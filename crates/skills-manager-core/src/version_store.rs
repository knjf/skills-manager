use anyhow::{Context, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::skill_store::SkillStore;

pub const DEFAULT_RETENTION: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CaptureTrigger {
    Scan,
    Import,
    Backfill,
    Restore,
}

impl CaptureTrigger {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Scan => "scan",
            Self::Import => "import",
            Self::Backfill => "backfill",
            Self::Restore => "restore",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionRecord {
    pub id: String,
    pub skill_id: String,
    pub version_no: i64,
    pub content_hash: String,
    pub byte_size: i64,
    pub captured_at: i64,
    pub trigger: String,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub source_ref_resolved: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionContent {
    pub record: VersionRecord,
    pub content: String,
}

impl SkillStore {
    pub fn capture_version(
        &self,
        skill_id: &str,
        content: &str,
        trigger: CaptureTrigger,
    ) -> Result<Option<VersionRecord>> {
        use sha2::{Digest, Sha256};

        let hash = format!("{:x}", Sha256::digest(content.as_bytes()));

        // Acquire the guard once for the entire operation — Mutex is not re-entrant,
        // so we cannot call self.latest_version() (which would also lock) from here.
        let conn = self.conn();

        // Inline latest-version query.
        let latest = conn
            .query_row(
                "SELECT id, skill_id, version_no, content_hash, byte_size, captured_at,
                        trigger, source_type, source_ref, source_ref_resolved
                   FROM skill_versions
                  WHERE skill_id = ?1
                  ORDER BY version_no DESC
                  LIMIT 1",
                params![skill_id],
                map_version_row,
            )
            .optional()?;

        if let Some(ref latest) = latest {
            if latest.content_hash == hash {
                return Ok(None);
            }
        }

        let (source_type, source_ref, source_ref_resolved): (
            String,
            Option<String>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT source_type, source_ref, source_ref_resolved FROM skills WHERE id = ?1",
                params![skill_id],
                |row: &rusqlite::Row<'_>| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .with_context(|| format!("skill {skill_id} not found"))?;

        let next_version_no = latest.as_ref().map(|v| v.version_no).unwrap_or(0) + 1;
        let id = uuid::Uuid::new_v4().to_string();
        let captured_at = chrono::Utc::now().timestamp();
        let byte_size = content.len() as i64;

        conn.execute(
            "INSERT INTO skill_versions (
                id, skill_id, version_no, content, content_hash, byte_size,
                captured_at, trigger, source_type, source_ref, source_ref_resolved
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                id,
                skill_id,
                next_version_no,
                content,
                hash,
                byte_size,
                captured_at,
                trigger.as_str(),
                source_type,
                source_ref,
                source_ref_resolved,
            ],
        )?;

        // LRU eviction: keep newest DEFAULT_RETENTION versions.
        conn.execute(
            "DELETE FROM skill_versions
              WHERE skill_id = ?1
                AND id NOT IN (
                    SELECT id FROM skill_versions
                     WHERE skill_id = ?1
                     ORDER BY version_no DESC
                     LIMIT ?2
                )",
            params![skill_id, DEFAULT_RETENTION as i64],
        )?;

        Ok(Some(VersionRecord {
            id,
            skill_id: skill_id.to_string(),
            version_no: next_version_no,
            content_hash: hash,
            byte_size,
            captured_at,
            trigger: trigger.as_str().to_string(),
            source_type,
            source_ref,
            source_ref_resolved,
        }))
    }

    pub fn latest_version(&self, skill_id: &str) -> Result<Option<VersionRecord>> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, skill_id, version_no, content_hash, byte_size, captured_at,
                    trigger, source_type, source_ref, source_ref_resolved
               FROM skill_versions
              WHERE skill_id = ?1
              ORDER BY version_no DESC
              LIMIT 1",
            params![skill_id],
            map_version_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_versions(&self, skill_id: &str) -> Result<Vec<VersionRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, skill_id, version_no, content_hash, byte_size, captured_at,
                    trigger, source_type, source_ref, source_ref_resolved
               FROM skill_versions
              WHERE skill_id = ?1
              ORDER BY version_no DESC",
        )?;
        let rows = stmt.query_map(params![skill_id], map_version_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_version(&self, version_id: &str) -> Result<VersionContent> {
        let conn = self.conn();
        let (record, content): (VersionRecord, String) = conn.query_row(
            "SELECT id, skill_id, version_no, content_hash, byte_size, captured_at,
                    trigger, source_type, source_ref, source_ref_resolved, content
               FROM skill_versions
              WHERE id = ?1",
            params![version_id],
            |row| {
                let rec = VersionRecord {
                    id: row.get(0)?,
                    skill_id: row.get(1)?,
                    version_no: row.get(2)?,
                    content_hash: row.get(3)?,
                    byte_size: row.get(4)?,
                    captured_at: row.get(5)?,
                    trigger: row.get(6)?,
                    source_type: row.get(7)?,
                    source_ref: row.get(8)?,
                    source_ref_resolved: row.get(9)?,
                };
                let content: String = row.get(10)?;
                Ok((rec, content))
            },
        )?;
        Ok(VersionContent { record, content })
    }

    /// Copies the specified version's content into a fresh snapshot (if it
    /// differs from latest) and returns the full VersionContent for callers
    /// to persist to the central library. If restoring the latest, capture is
    /// a no-op but the original VersionContent is still returned unchanged.
    pub fn restore_version(&self, version_id: &str) -> Result<VersionContent> {
        let target = self.get_version(version_id)?;
        let _ = self.capture_version(
            &target.record.skill_id,
            &target.content,
            CaptureTrigger::Restore,
        )?;
        Ok(target)
    }

    /// Re-read every skill's SKILL.md and capture a new version if content changed.
    /// Returns number of skills that produced a new version.
    pub fn rescan_central_library(&self) -> Result<usize> {
        let skills: Vec<(String, String)> = {
            let conn = self.conn();
            let mut stmt = conn.prepare("SELECT id, central_path FROM skills")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            let result = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            result
        };

        let mut captured = 0usize;
        for (skill_id, central_path) in skills {
            let skill_md = std::path::Path::new(&central_path).join("SKILL.md");
            match std::fs::read_to_string(&skill_md) {
                Ok(content) => {
                    match self.capture_version(&skill_id, &content, CaptureTrigger::Scan) {
                        Ok(Some(_)) => captured += 1,
                        Ok(None) => {}
                        Err(err) => log::warn!("rescan capture failed for {skill_id}: {err}"),
                    }
                }
                Err(_) => {
                    // skill dir removed or unreadable — ignore silently
                }
            }
        }
        Ok(captured)
    }

    /// Backfill: for each skill with no versions yet AND a readable SKILL.md,
    /// capture v1 with trigger='backfill'. Failures are logged, not fatal.
    /// Returns number of skills successfully backfilled.
    pub fn backfill_initial_versions(&self) -> Result<usize> {
        let skill_ids: Vec<(String, String)> = {
            let conn = self.conn();
            let mut stmt = conn.prepare(
                "SELECT s.id, s.central_path
                   FROM skills s
                  WHERE NOT EXISTS (
                      SELECT 1 FROM skill_versions v WHERE v.skill_id = s.id
                  )",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let mut captured = 0usize;
        for (skill_id, central_path) in skill_ids {
            let skill_md = std::path::Path::new(&central_path).join("SKILL.md");
            match std::fs::read_to_string(&skill_md) {
                Ok(content) => {
                    match self.capture_version(&skill_id, &content, CaptureTrigger::Backfill) {
                        Ok(Some(_)) => captured += 1,
                        Ok(None) => log::warn!(
                            "backfill: capture_version returned None for {skill_id} \
                             (hash collision or concurrent write — skipping)"
                        ),
                        Err(err) => log::warn!("backfill failed for skill {skill_id}: {err}"),
                    }
                }
                Err(err) => {
                    log::info!(
                        "backfill skipped {skill_id} ({}): {err}",
                        skill_md.display()
                    );
                }
            }
        }
        Ok(captured)
    }
}

fn map_version_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<VersionRecord> {
    Ok(VersionRecord {
        id: row.get(0)?,
        skill_id: row.get(1)?,
        version_no: row.get(2)?,
        content_hash: row.get(3)?,
        byte_size: row.get(4)?,
        captured_at: row.get(5)?,
        trigger: row.get(6)?,
        source_type: row.get(7)?,
        source_ref: row.get(8)?,
        source_ref_resolved: row.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill_store::{SkillRecord, SkillStore};
    use tempfile::tempdir;

    fn make_store() -> (tempfile::TempDir, SkillStore) {
        let tmp = tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let store = SkillStore::new(&db_path).unwrap();
        (tmp, store)
    }

    fn sample_skill_record(id: &str) -> SkillRecord {
        SkillRecord {
            id: id.to_string(),
            name: format!("skill-{id}"),
            description: None,
            source_type: "local".to_string(),
            source_ref: None,
            source_ref_resolved: None,
            source_subpath: None,
            source_branch: None,
            source_revision: None,
            remote_revision: None,
            central_path: format!("/central/{id}"),
            content_hash: None,
            enabled: true,
            created_at: 0,
            updated_at: 0,
            status: "ok".to_string(),
            update_status: "unknown".to_string(),
            last_checked_at: None,
            last_check_error: None,
        }
    }

    fn insert_skill(store: &SkillStore, id: &str) {
        store.insert_skill(&sample_skill_record(id)).unwrap();
    }

    #[test]
    fn capture_version_inserts_new_row() {
        let (_tmp, store) = make_store();
        insert_skill(&store, "s1");

        let result = store
            .capture_version("s1", "hello world", CaptureTrigger::Scan)
            .unwrap();
        let rec = result.expect("expected captured version");

        assert_eq!(rec.skill_id, "s1");
        assert_eq!(rec.version_no, 1);
        assert_eq!(rec.byte_size, "hello world".len() as i64);
        assert_eq!(rec.trigger, "scan");
    }

    #[test]
    fn capture_version_dedups_against_latest_only() {
        let (_tmp, store) = make_store();
        insert_skill(&store, "s1");

        store
            .capture_version("s1", "A", CaptureTrigger::Scan)
            .unwrap();
        let again = store
            .capture_version("s1", "A", CaptureTrigger::Scan)
            .unwrap();
        assert!(again.is_none(), "same hash as latest should no-op");

        store
            .capture_version("s1", "B", CaptureTrigger::Scan)
            .unwrap();
        let back_to_a = store
            .capture_version("s1", "A", CaptureTrigger::Restore)
            .unwrap();
        assert!(
            back_to_a.is_some(),
            "content matching an older (non-latest) version should capture"
        );
        assert_eq!(back_to_a.unwrap().version_no, 3);
    }

    #[test]
    fn capture_version_increments_version_no() {
        let (_tmp, store) = make_store();
        insert_skill(&store, "s1");

        let a = store
            .capture_version("s1", "A", CaptureTrigger::Scan)
            .unwrap()
            .unwrap();
        let b = store
            .capture_version("s1", "B", CaptureTrigger::Scan)
            .unwrap()
            .unwrap();
        let c = store
            .capture_version("s1", "C", CaptureTrigger::Scan)
            .unwrap()
            .unwrap();

        assert_eq!((a.version_no, b.version_no, c.version_no), (1, 2, 3));
    }

    #[test]
    fn lru_eviction_keeps_newest_n() {
        let (_tmp, store) = make_store();
        insert_skill(&store, "s1");

        for i in 0..55 {
            store
                .capture_version("s1", &format!("v{i}"), CaptureTrigger::Scan)
                .unwrap();
        }

        let versions = store.list_versions("s1").unwrap();
        assert_eq!(versions.len(), DEFAULT_RETENTION);
        assert_eq!(versions[0].version_no, 55);
        assert_eq!(versions[DEFAULT_RETENTION - 1].version_no, 6);
    }

    #[test]
    fn skill_delete_cascades_versions() {
        let (_tmp, store) = make_store();
        insert_skill(&store, "s1");
        store
            .capture_version("s1", "A", CaptureTrigger::Scan)
            .unwrap();

        // SkillStore::new sets PRAGMA foreign_keys=ON, so ON DELETE CASCADE works.
        store.delete_skill("s1").unwrap();

        let versions = store.list_versions("s1").unwrap();
        assert!(versions.is_empty());
    }

    #[test]
    fn get_version_returns_content() {
        let (_tmp, store) = make_store();
        store.insert_skill(&sample_skill_record("s1")).unwrap();
        let rec = store
            .capture_version("s1", "hello", CaptureTrigger::Scan)
            .unwrap()
            .unwrap();

        let fetched = store.get_version(&rec.id).unwrap();
        assert_eq!(fetched.content, "hello");
        assert_eq!(fetched.record.version_no, 1);
    }

    #[test]
    fn restore_older_version_captures_new_row() {
        let (_tmp, store) = make_store();
        store.insert_skill(&sample_skill_record("s1")).unwrap();
        let v1 = store
            .capture_version("s1", "A", CaptureTrigger::Scan)
            .unwrap()
            .unwrap();
        store
            .capture_version("s1", "B", CaptureTrigger::Scan)
            .unwrap();

        let result = store.restore_version(&v1.id).unwrap();
        assert_eq!(result.content, "A");

        let versions = store.list_versions("s1").unwrap();
        // Newest first: v3 restore, v2 B, v1 A
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].version_no, 3);
        assert_eq!(versions[0].trigger, "restore");
        assert_eq!(versions[0].content_hash, v1.content_hash);
    }

    #[test]
    fn restore_latest_is_noop() {
        let (_tmp, store) = make_store();
        store.insert_skill(&sample_skill_record("s1")).unwrap();
        let v1 = store
            .capture_version("s1", "A", CaptureTrigger::Scan)
            .unwrap()
            .unwrap();

        let _ = store.restore_version(&v1.id).unwrap();
        let versions = store.list_versions("s1").unwrap();
        assert_eq!(versions.len(), 1);
    }

    #[test]
    fn backfill_creates_v1_for_skills_with_readable_content() {
        use std::fs;
        let (tmp, store) = make_store();

        let central_a = tmp.path().join("skills/a");
        fs::create_dir_all(&central_a).unwrap();
        fs::write(central_a.join("SKILL.md"), "---\nname: a\n---\nbody A\n").unwrap();

        let central_b = tmp.path().join("skills/b");
        fs::create_dir_all(&central_b).unwrap();
        fs::write(central_b.join("SKILL.md"), "---\nname: b\n---\nbody B\n").unwrap();

        let mut rec_a = sample_skill_record("a");
        rec_a.central_path = central_a.to_string_lossy().to_string();
        store.insert_skill(&rec_a).unwrap();

        let mut rec_b = sample_skill_record("b");
        rec_b.central_path = central_b.to_string_lossy().to_string();
        store.insert_skill(&rec_b).unwrap();

        let n = store.backfill_initial_versions().unwrap();
        assert_eq!(n, 2);

        assert_eq!(store.list_versions("a").unwrap().len(), 1);
        assert_eq!(store.list_versions("b").unwrap().len(), 1);
        assert_eq!(store.list_versions("a").unwrap()[0].trigger, "backfill");
    }

    #[test]
    fn backfill_skips_skills_without_readable_content() {
        let (tmp, store) = make_store();

        let mut rec = sample_skill_record("ghost");
        rec.central_path = tmp
            .path()
            .join("nowhere/ghost")
            .to_string_lossy()
            .to_string();
        store.insert_skill(&rec).unwrap();

        let n = store.backfill_initial_versions().unwrap();
        assert_eq!(n, 0);
        assert!(store.list_versions("ghost").unwrap().is_empty());
    }

    #[test]
    fn backfill_is_idempotent_for_skills_with_existing_versions() {
        let (tmp, store) = make_store();
        use std::fs;
        let central = tmp.path().join("skills/s");
        fs::create_dir_all(&central).unwrap();
        fs::write(central.join("SKILL.md"), "content\n").unwrap();

        let mut rec = sample_skill_record("s");
        rec.central_path = central.to_string_lossy().to_string();
        store.insert_skill(&rec).unwrap();

        // First backfill captures v1
        assert_eq!(store.backfill_initial_versions().unwrap(), 1);
        // Second call should be a no-op — skill already has versions
        assert_eq!(store.backfill_initial_versions().unwrap(), 0);
        assert_eq!(store.list_versions("s").unwrap().len(), 1);
    }

    #[test]
    fn rescan_detects_external_edit_and_captures_new_version() {
        use std::fs;
        let (tmp, store) = make_store();

        let central = tmp.path().join("skills/s1");
        fs::create_dir_all(&central).unwrap();
        fs::write(central.join("SKILL.md"), "v1 content\n").unwrap();

        let mut rec = sample_skill_record("s1");
        rec.central_path = central.to_string_lossy().to_string();
        store.insert_skill(&rec).unwrap();
        store.backfill_initial_versions().unwrap();

        // External edit
        fs::write(central.join("SKILL.md"), "v2 content\n").unwrap();

        let captured = store.rescan_central_library().unwrap();
        assert_eq!(captured, 1);

        let versions = store.list_versions("s1").unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version_no, 2);
        assert_eq!(versions[0].trigger, "scan");
    }

    #[test]
    fn rescan_is_noop_when_no_content_changed() {
        use std::fs;
        let (tmp, store) = make_store();

        let central = tmp.path().join("skills/s1");
        fs::create_dir_all(&central).unwrap();
        fs::write(central.join("SKILL.md"), "stable content\n").unwrap();

        let mut rec = sample_skill_record("s1");
        rec.central_path = central.to_string_lossy().to_string();
        store.insert_skill(&rec).unwrap();
        store.backfill_initial_versions().unwrap();

        assert_eq!(store.rescan_central_library().unwrap(), 0);
        assert_eq!(store.list_versions("s1").unwrap().len(), 1);
    }
}
