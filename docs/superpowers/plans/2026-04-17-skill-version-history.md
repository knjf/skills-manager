# Skill Version History & Diff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a History tab that lets users view every past version of each skill, diff any two side-by-side, and restore older versions — all backed by a DB snapshot captured whenever the central library content hash changes.

**Architecture:** SQLite `skill_versions` table stores full SKILL.md text per snapshot (Approach A). Core crate owns capture / diff / restore. Tauri commands expose read-only listing + diff + restore. React `HistoryView` uses `react-diff-view` for split-view rendering. LRU retention at 50 versions per skill. App startup backfills existing skills as v1.

**Tech Stack:** Rust (`similar` crate for diff), SQLite/rusqlite, Tauri 2, React + TypeScript + Tailwind, `react-diff-view` + `diff-match-patch`.

**Spec:** [2026-04-17-skill-version-history-design.md](../specs/2026-04-17-skill-version-history-design.md)

---

## File Structure

**Create:**
- `crates/skills-manager-core/src/version_store.rs` — types + capture_version + LRU + list/get/restore + backfill
- `crates/skills-manager-core/src/diff.rs` — compute_diff via `similar`
- `src-tauri/src/commands/history.rs` — Tauri IPC commands
- `src/views/HistoryView.tsx` — main page
- `src/views/history/SkillListPane.tsx`
- `src/views/history/MetadataPanel.tsx`
- `src/views/history/VersionListPane.tsx`
- `src/views/history/DiffPane.tsx`
- `src/views/history/RestoreConfirmDialog.tsx`

**Modify:**
- `crates/skills-manager-core/Cargo.toml` (+`similar`)
- `crates/skills-manager-core/src/migrations.rs` (v9 schema)
- `crates/skills-manager-core/src/lib.rs` (module exports)
- `crates/skills-manager-core/src/installer.rs` (capture hook)
- `crates/skills-manager-core/src/pack_seeder.rs` (capture hook)
- `crates/skills-manager-core/src/central_repo.rs` (`rescan_library` helper) — if absent, add to `version_store.rs`
- `src-tauri/src/commands/mod.rs` (register)
- `src-tauri/src/lib.rs` (register commands + startup backfill)
- `src-tauri/src/core/file_watcher.rs` (auto rescan central on event)
- `src/App.tsx` (route + sidebar)
- `src/lib/tauri.ts` (IPC wrappers + types)
- `src/i18n/en.json`, `src/i18n/zh.json`, `src/i18n/zh-TW.json`
- `package.json` (+`react-diff-view`, +`diff-match-patch`)

---

## Phase 1 — Core data layer

### Task 1: Add `similar` crate + migration v9 schema

**Files:**
- Modify: `crates/skills-manager-core/Cargo.toml`
- Modify: `crates/skills-manager-core/src/migrations.rs`

- [ ] **Step 1: Add Cargo dep**

Edit `crates/skills-manager-core/Cargo.toml`, add under `[dependencies]`:

```toml
similar = "2"
```

- [ ] **Step 2: Bump LATEST_VERSION to 9 and add migration step**

Edit `crates/skills-manager-core/src/migrations.rs`:

Change line `const LATEST_VERSION: u32 = 8;` → `const LATEST_VERSION: u32 = 9;`

In `migrate_step`, add `8 => migrate_v8_to_v9(conn),` before the `_ => bail!` arm.

Add the migration function at the bottom of the migrations section:

```rust
/// v8 → v9: Add skill_versions table for version history.
fn migrate_v8_to_v9(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS skill_versions (
            id                  TEXT PRIMARY KEY,
            skill_id            TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
            version_no          INTEGER NOT NULL,
            content             TEXT NOT NULL,
            content_hash        TEXT NOT NULL,
            byte_size           INTEGER NOT NULL,
            captured_at         INTEGER NOT NULL,
            trigger             TEXT NOT NULL,
            source_type         TEXT NOT NULL,
            source_ref          TEXT,
            source_ref_resolved TEXT,
            UNIQUE(skill_id, version_no)
        );
        CREATE INDEX IF NOT EXISTS idx_skill_versions_skill_captured
            ON skill_versions(skill_id, captured_at DESC);
        ",
    )?;
    Ok(())
}
```

Also mirror the `CREATE TABLE` (without the `IF NOT EXISTS`) into the initial `migrate_v0_to_v1` DDL so new databases start with it. Place it after the existing `scenario_skill_tools` block inside the big execute_batch string in `migrate_v0_to_v1`.

- [ ] **Step 3: Write migration test**

Add to the bottom of `migrations.rs` `#[cfg(test)] mod tests` block:

```rust
#[test]
fn v8_to_v9_creates_skill_versions_table() {
    use rusqlite::Connection;
    let conn = Connection::open_in_memory().unwrap();
    // Simulate pre-v9 DB: create skills table and set user_version=8
    conn.execute_batch(
        "CREATE TABLE skills (id TEXT PRIMARY KEY, name TEXT NOT NULL);
         PRAGMA user_version = 8;",
    )
    .unwrap();

    run_migrations(&conn).unwrap();

    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 9);

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='skill_versions'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}
```

- [ ] **Step 4: Run tests**

```bash
cd crates/skills-manager-core && cargo test migrations::tests::v8_to_v9 -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/skills-manager-core/Cargo.toml crates/skills-manager-core/src/migrations.rs
git commit -m "feat(core): add skill_versions schema (v9 migration)"
```

---

### Task 2: VersionRecord types + `capture_version` with LRU

**Files:**
- Create: `crates/skills-manager-core/src/version_store.rs`
- Modify: `crates/skills-manager-core/src/lib.rs` (pub mod version_store + re-exports)
- Modify: `crates/skills-manager-core/src/skill_store.rs` (pub impl methods delegating to version_store)

- [ ] **Step 1: Create `version_store.rs` skeleton with types**

Create `crates/skills-manager-core/src/version_store.rs`:

```rust
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
```

- [ ] **Step 2: Wire module into lib.rs**

Edit `crates/skills-manager-core/src/lib.rs`:

Add `pub mod version_store;` alphabetically (after `pub mod tool_adapters;` is fine).

Add re-export after the existing `pub use skill_store::{...};` block:

```rust
pub use version_store::{CaptureTrigger, VersionContent, VersionRecord};
```

- [ ] **Step 3: Write failing test for capture_version**

Add to `version_store.rs`:

```rust
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

    fn insert_skill(store: &SkillStore, id: &str) {
        let skill = SkillRecord {
            id: id.to_string(),
            name: id.to_string(),
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
            enabled: Some(1),
            created_at: Some(0),
            updated_at: Some(0),
            status: Some("ok".to_string()),
            update_status: Some("unknown".to_string()),
            last_checked_at: None,
            last_check_error: None,
        };
        store.insert_skill(&skill).unwrap();
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
        // Newest first
        assert_eq!(versions[0].version_no, 55);
        // Oldest remaining is 55-50+1 = 6
        assert_eq!(versions[DEFAULT_RETENTION - 1].version_no, 6);
    }

    #[test]
    fn skill_delete_cascades_versions() {
        let (_tmp, store) = make_store();
        insert_skill(&store, "s1");
        store
            .capture_version("s1", "A", CaptureTrigger::Scan)
            .unwrap();

        store.delete_skill("s1").unwrap();

        let versions = store.list_versions("s1").unwrap();
        assert!(versions.is_empty());
    }
}
```

- [ ] **Step 4: Run tests to verify they fail**

```bash
cd crates/skills-manager-core && cargo test version_store::tests -- --nocapture
```

Expected: FAIL (methods not implemented yet).

- [ ] **Step 5: Implement capture_version + list_versions + LRU on SkillStore**

Append to `version_store.rs`:

```rust
impl SkillStore {
    pub fn capture_version(
        &self,
        skill_id: &str,
        content: &str,
        trigger: CaptureTrigger,
    ) -> Result<Option<VersionRecord>> {
        use sha2::{Digest, Sha256};

        let hash = format!("{:x}", Sha256::digest(content.as_bytes()));

        let latest = self.latest_version(skill_id)?;
        if let Some(ref latest) = latest {
            if latest.content_hash == hash {
                return Ok(None);
            }
        }

        // Fetch source metadata from skills row
        let conn = self.conn();
        let (source_type, source_ref, source_ref_resolved): (
            String,
            Option<String>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT source_type, source_ref, source_ref_resolved FROM skills WHERE id = ?1",
                params![skill_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
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

        // LRU eviction
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
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
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
```

Note: `SkillStore::conn()` is assumed to expose the underlying `&Connection`. If it doesn't exist, instead access the private field via a helper method in `skill_store.rs`:

Add to `skill_store.rs` `impl SkillStore` block:

```rust
pub(crate) fn conn(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
    self.conn.lock().expect("skill store mutex poisoned")
}
```

(Adjust signature to match the existing private accessor pattern — check `skill_store.rs` line 160-250 for how other methods access `self.conn`.)

- [ ] **Step 6: Run tests to verify they pass**

```bash
cd crates/skills-manager-core && cargo test version_store::tests -- --nocapture
```

Expected: 5 tests PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/skills-manager-core/src/version_store.rs \
        crates/skills-manager-core/src/lib.rs \
        crates/skills-manager-core/src/skill_store.rs
git commit -m "feat(core): capture_version + LRU + list_versions"
```

---

### Task 3: `get_version` + `restore_version`

**Files:**
- Modify: `crates/skills-manager-core/src/version_store.rs`

- [ ] **Step 1: Write failing test**

Append to `version_store.rs` `mod tests`:

```rust
#[test]
fn get_version_returns_content() {
    let (_tmp, store) = make_store();
    insert_skill(&store, "s1");
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
    insert_skill(&store, "s1");
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
    insert_skill(&store, "s1");
    let v1 = store
        .capture_version("s1", "A", CaptureTrigger::Scan)
        .unwrap()
        .unwrap();

    // Restoring the only (latest) version should not create a new row
    let _ = store.restore_version(&v1.id).unwrap();
    let versions = store.list_versions("s1").unwrap();
    assert_eq!(versions.len(), 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd crates/skills-manager-core && cargo test version_store::tests::get_version -- --nocapture
cd crates/skills-manager-core && cargo test version_store::tests::restore -- --nocapture
```

Expected: FAIL (methods not defined).

- [ ] **Step 3: Implement get_version + restore_version**

Add to `impl SkillStore` in `version_store.rs`:

```rust
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

/// Copies the specified version's content into a fresh snapshot (if it differs
/// from latest) and returns the full VersionContent for callers to persist to
/// the central library.
pub fn restore_version(&self, version_id: &str) -> Result<VersionContent> {
    let target = self.get_version(version_id)?;
    let _ = self.capture_version(
        &target.record.skill_id,
        &target.content,
        CaptureTrigger::Restore,
    )?;
    Ok(target)
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd crates/skills-manager-core && cargo test version_store::tests -- --nocapture
```

Expected: all version_store tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/skills-manager-core/src/version_store.rs
git commit -m "feat(core): get_version + restore_version"
```

---

### Task 4: `backfill_initial_versions`

**Files:**
- Modify: `crates/skills-manager-core/src/version_store.rs`

- [ ] **Step 1: Write failing test**

Append to `mod tests`:

```rust
#[test]
fn backfill_creates_v1_for_skills_with_readable_content() {
    use std::fs;
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test.db");
    let store = SkillStore::new(&db_path).unwrap();

    // Create two skills on disk at their central_paths
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
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test.db");
    let store = SkillStore::new(&db_path).unwrap();

    // Skill row exists but central_path doesn't resolve to a real file
    let mut rec = sample_skill_record("ghost");
    rec.central_path = tmp.path().join("nowhere/ghost").to_string_lossy().to_string();
    store.insert_skill(&rec).unwrap();

    let n = store.backfill_initial_versions().unwrap();
    assert_eq!(n, 0);
}

fn sample_skill_record(id: &str) -> crate::skill_store::SkillRecord {
    crate::skill_store::SkillRecord {
        id: id.to_string(),
        name: id.to_string(),
        description: None,
        source_type: "local".to_string(),
        source_ref: None,
        source_ref_resolved: None,
        source_subpath: None,
        source_branch: None,
        source_revision: None,
        remote_revision: None,
        central_path: String::new(),
        content_hash: None,
        enabled: Some(1),
        created_at: Some(0),
        updated_at: Some(0),
        status: Some("ok".to_string()),
        update_status: Some("unknown".to_string()),
        last_checked_at: None,
        last_check_error: None,
    }
}
```

Remove the duplicate `insert_skill` helper at the top of tests — consolidate into `sample_skill_record`. Update earlier tests to use `store.insert_skill(&sample_skill_record(id))` instead of the old `insert_skill(&store, id)` helper.

- [ ] **Step 2: Run tests to verify failure**

```bash
cd crates/skills-manager-core && cargo test version_store::tests::backfill -- --nocapture
```

Expected: FAIL — `backfill_initial_versions` not defined.

- [ ] **Step 3: Implement backfill**

Add to `impl SkillStore` in `version_store.rs`:

```rust
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
        let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };

    let mut captured = 0usize;
    for (skill_id, central_path) in skill_ids {
        let skill_md = std::path::Path::new(&central_path).join("SKILL.md");
        match std::fs::read_to_string(&skill_md) {
            Ok(content) => match self.capture_version(&skill_id, &content, CaptureTrigger::Backfill) {
                Ok(Some(_)) => captured += 1,
                Ok(None) => {} // impossible (no prior versions), but safe
                Err(err) => log::warn!("backfill failed for skill {skill_id}: {err}"),
            },
            Err(err) => {
                log::info!("backfill skipped {skill_id} ({}): {err}", skill_md.display());
            }
        }
    }
    Ok(captured)
}
```

- [ ] **Step 4: Run all core tests**

```bash
cd crates/skills-manager-core && cargo test -- --nocapture
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/skills-manager-core/src/version_store.rs
git commit -m "feat(core): backfill_initial_versions from central library"
```

---

### Task 5: `diff.rs` — compute_diff via `similar`

**Files:**
- Create: `crates/skills-manager-core/src/diff.rs`
- Modify: `crates/skills-manager-core/src/lib.rs`

- [ ] **Step 1: Create `diff.rs`**

Create `crates/skills-manager-core/src/diff.rs`:

```rust
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

/// Compute unified hunks between two texts.
/// `context` = number of unchanged lines around each change (e.g. 3).
pub fn compute_diff(old: &str, new: &str, context: usize) -> Vec<DiffHunk> {
    let diff = TextDiff::from_lines(old, new);
    let mut hunks: Vec<DiffHunk> = Vec::new();

    for group in diff.grouped_ops(context) {
        if group.is_empty() {
            continue;
        }

        let first = group.first().unwrap();
        let last = group.last().unwrap();
        let (old_start, old_end) = (first.old_range().start, last.old_range().end);
        let (new_start, new_end) = (first.new_range().start, last.new_range().end);

        let header = format!(
            "@@ -{},{} +{},{} @@",
            old_start + 1,
            old_end - old_start,
            new_start + 1,
            new_end - new_start,
        );

        let mut lines: Vec<DiffLine> = Vec::new();
        for op in group {
            for change in diff.iter_inline_changes(&op) {
                let kind = match change.tag() {
                    ChangeTag::Equal => DiffLineKind::Context,
                    ChangeTag::Insert => DiffLineKind::Added,
                    ChangeTag::Delete => DiffLineKind::Removed,
                };
                let text: String = change
                    .iter_strings_lossy()
                    .map(|(_, s)| s.into_owned())
                    .collect();
                lines.push(DiffLine {
                    kind,
                    old_no: change.old_index().map(|i| (i + 1) as u32),
                    new_no: change.new_index().map(|i| (i + 1) as u32),
                    text,
                });
            }
        }

        hunks.push(DiffHunk { header, lines });
    }

    hunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_inputs_yield_no_hunks() {
        let hunks = compute_diff("same\ntext\n", "same\ntext\n", 3);
        assert!(hunks.is_empty());
    }

    #[test]
    fn simple_addition_produces_one_hunk() {
        let hunks = compute_diff("a\nb\n", "a\nb\nc\n", 3);
        assert_eq!(hunks.len(), 1);
        assert!(hunks[0]
            .lines
            .iter()
            .any(|l| l.kind == DiffLineKind::Added && l.text.trim() == "c"));
    }

    #[test]
    fn replacement_shows_both_removed_and_added() {
        let hunks = compute_diff("hello\n", "world\n", 3);
        assert_eq!(hunks.len(), 1);
        let kinds: Vec<_> = hunks[0].lines.iter().map(|l| &l.kind).collect();
        assert!(kinds.contains(&&DiffLineKind::Removed));
        assert!(kinds.contains(&&DiffLineKind::Added));
    }
}
```

- [ ] **Step 2: Wire into lib.rs**

Edit `crates/skills-manager-core/src/lib.rs`:

Add `pub mod diff;` alphabetically.

Add re-export:

```rust
pub use diff::{compute_diff, DiffHunk, DiffLine, DiffLineKind};
```

- [ ] **Step 3: Run tests**

```bash
cd crates/skills-manager-core && cargo test diff::tests -- --nocapture
```

Expected: 3 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/skills-manager-core/src/diff.rs crates/skills-manager-core/src/lib.rs
git commit -m "feat(core): compute_diff via similar crate"
```

---

## Phase 2 — Capture hooks + central rescan

### Task 6: Hook capture_version into installer + pack_seeder

**Files:**
- Modify: `crates/skills-manager-core/src/installer.rs`
- Modify: `crates/skills-manager-core/src/pack_seeder.rs`

- [ ] **Step 1: Hook `installer.rs`**

Find the location where installer has finished writing the skill to central library and has a `content_hash`. Based on existing code (near the `content_hash::hash_directory(destination)?` call around line 153), add immediately after the `store.insert_skill` / `store.update_skill` call:

```rust
let skill_md = destination.join("SKILL.md");
if let Ok(content) = std::fs::read_to_string(&skill_md) {
    if let Err(err) = store.capture_version(
        &skill.id,
        &content,
        crate::version_store::CaptureTrigger::Import,
    ) {
        log::warn!("failed to capture version after install for {}: {err}", skill.id);
    }
}
```

Add `use crate::version_store::CaptureTrigger;` at the top if not present.

- [ ] **Step 2: Hook `pack_seeder.rs`**

Similarly, find the spot in `pack_seeder.rs` where each pack skill has been materialized into central library (search for `insert_skill` / `update_skill`). Add the same `capture_version(..., CaptureTrigger::Import)` call after the skill row is persisted. Use `log::warn!` on error; never fail the seed operation.

- [ ] **Step 3: Write integration smoke test (optional but recommended)**

If a convenient fixture exists in `installer.rs` tests (e.g. around line 278 which already creates a SKILL.md), add:

```rust
#[test]
fn install_captures_initial_version() {
    // Use existing installer test scaffolding — after install,
    // assert store.list_versions(&skill_id) has one row with trigger='import'.
}
```

Sketch only — adapt to the existing installer test harness. If not easy, skip and rely on manual verification in Step 5.

- [ ] **Step 4: Run tests**

```bash
cd crates/skills-manager-core && cargo test -- --nocapture
```

Expected: all tests PASS. Existing installer tests should still pass since capture is best-effort.

- [ ] **Step 5: Commit**

```bash
git add crates/skills-manager-core/src/installer.rs crates/skills-manager-core/src/pack_seeder.rs
git commit -m "feat(core): capture_version hooks in installer and pack_seeder"
```

---

### Task 7: `rescan_central_library` + file_watcher integration

**Files:**
- Modify: `crates/skills-manager-core/src/version_store.rs`
- Modify: `src-tauri/src/core/file_watcher.rs`

- [ ] **Step 1: Write failing test**

Append to `version_store.rs mod tests`:

```rust
#[test]
fn rescan_detects_external_edit_and_captures_new_version() {
    use std::fs;
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test.db");
    let store = SkillStore::new(&db_path).unwrap();

    let central = tmp.path().join("skills/s1");
    fs::create_dir_all(&central).unwrap();
    fs::write(central.join("SKILL.md"), "v1 content\n").unwrap();

    let mut rec = sample_skill_record("s1");
    rec.central_path = central.to_string_lossy().to_string();
    store.insert_skill(&rec).unwrap();
    store.backfill_initial_versions().unwrap();

    // User edits the file externally
    fs::write(central.join("SKILL.md"), "v2 content\n").unwrap();

    let captured = store.rescan_central_library().unwrap();
    assert_eq!(captured, 1);

    let versions = store.list_versions("s1").unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0].version_no, 2);
    assert_eq!(versions[0].trigger, "scan");
}
```

- [ ] **Step 2: Run test to verify failure**

```bash
cd crates/skills-manager-core && cargo test version_store::tests::rescan -- --nocapture
```

Expected: FAIL.

- [ ] **Step 3: Implement `rescan_central_library`**

Add to `impl SkillStore` in `version_store.rs`:

```rust
/// Re-read every skill's SKILL.md and capture a new version if content changed.
/// Returns number of skills that produced a new version.
pub fn rescan_central_library(&self) -> Result<usize> {
    let skills: Vec<(String, String)> = {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id, central_path FROM skills")?;
        stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };

    let mut captured = 0usize;
    for (skill_id, central_path) in skills {
        let skill_md = std::path::Path::new(&central_path).join("SKILL.md");
        match std::fs::read_to_string(&skill_md) {
            Ok(content) => match self.capture_version(&skill_id, &content, CaptureTrigger::Scan) {
                Ok(Some(_)) => captured += 1,
                Ok(None) => {}
                Err(err) => log::warn!("rescan capture failed for {skill_id}: {err}"),
            },
            Err(_) => {
                // skill removed or unreadable — ignore
            }
        }
    }
    Ok(captured)
}
```

- [ ] **Step 4: Hook file_watcher to trigger rescan**

Edit `src-tauri/src/core/file_watcher.rs` — inside the event-handling match arm where `APP_FS_CHANGED_EVENT` is emitted, also call rescan:

```rust
Ok(Ok(event)) => {
    if !should_emit(&event) || last_emit.elapsed() < WATCH_EMIT_DEBOUNCE {
        continue;
    }

    // Best-effort: rescan central library to capture any external edits.
    let store_clone = Arc::clone(&store);
    std::thread::spawn(move || {
        if let Err(err) = store_clone.rescan_central_library() {
            log::debug!("rescan_central_library after fs event failed: {err}");
        }
    });

    if let Err(err) = app.emit(APP_FS_CHANGED_EVENT, ()) {
        log::debug!("Failed to emit app-files-changed: {err}");
    } else {
        last_emit = Instant::now();
    }
}
```

- [ ] **Step 5: Run tests**

```bash
cd crates/skills-manager-core && cargo test -- --nocapture
cargo check --workspace
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/skills-manager-core/src/version_store.rs src-tauri/src/core/file_watcher.rs
git commit -m "feat(core): rescan_central_library + file_watcher auto-capture"
```

---

## Phase 3 — Tauri commands

### Task 8: `commands/history.rs` read-only commands

**Files:**
- Create: `src-tauri/src/commands/history.rs`
- Modify: `src-tauri/src/commands/mod.rs`

- [ ] **Step 1: Create the commands module**

Create `src-tauri/src/commands/history.rs`:

```rust
use serde::Serialize;
use skills_manager_core::diff::{compute_diff, DiffHunk};
use skills_manager_core::version_store::{VersionContent, VersionRecord};
use tauri::State;

use crate::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct SkillHistorySummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub source_ref_resolved: Option<String>,
    pub content_hash: Option<String>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub version_count: i64,
    pub latest_captured_at: Option<i64>,
}

#[tauri::command]
pub fn list_skills_with_history(
    state: State<'_, AppState>,
) -> Result<Vec<SkillHistorySummary>, String> {
    let store = state.store();
    let skills = store.list_skills().map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(skills.len());
    for s in skills {
        let versions = store.list_versions(&s.id).map_err(|e| e.to_string())?;
        let latest_captured_at = versions.first().map(|v| v.captured_at);
        out.push(SkillHistorySummary {
            id: s.id,
            name: s.name,
            description: s.description,
            source_type: s.source_type,
            source_ref: s.source_ref,
            source_ref_resolved: s.source_ref_resolved,
            content_hash: s.content_hash,
            created_at: s.created_at,
            updated_at: s.updated_at,
            version_count: versions.len() as i64,
            latest_captured_at,
        });
    }
    // Sort: most recently captured first, then by name
    out.sort_by(|a, b| {
        b.latest_captured_at
            .cmp(&a.latest_captured_at)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(out)
}

#[tauri::command]
pub fn list_versions(
    state: State<'_, AppState>,
    skill_id: String,
) -> Result<Vec<VersionRecord>, String> {
    state
        .store()
        .list_versions(&skill_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_version(
    state: State<'_, AppState>,
    version_id: String,
) -> Result<VersionContent, String> {
    state
        .store()
        .get_version(&version_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn diff_versions(
    state: State<'_, AppState>,
    old_id: String,
    new_id: String,
) -> Result<Vec<DiffHunk>, String> {
    let store = state.store();
    let old = store.get_version(&old_id).map_err(|e| e.to_string())?;
    let new = store.get_version(&new_id).map_err(|e| e.to_string())?;
    Ok(compute_diff(&old.content, &new.content, 3))
}
```

Replace `state.store()` with the actual accessor used elsewhere in `src-tauri/src/commands/` (check any existing command file, e.g. `scan.rs`, for the pattern). If `AppState` lives elsewhere, adjust the import path.

- [ ] **Step 2: Register module**

Edit `src-tauri/src/commands/mod.rs`:

Add `pub mod history;` alongside other `pub mod` declarations.

- [ ] **Step 3: Smoke-check compilation**

```bash
cd src-tauri && cargo check
```

Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/history.rs src-tauri/src/commands/mod.rs
git commit -m "feat(tauri): history read commands (list / get / diff)"
```

---

### Task 9: `restore_version` command

**Files:**
- Modify: `src-tauri/src/commands/history.rs`

- [ ] **Step 1: Implement restore command**

Append to `history.rs`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct RestoreResult {
    pub skill_id: String,
    pub new_version_no: Option<i64>,
    pub no_op: bool,
    pub message: String,
}

#[tauri::command]
pub fn restore_version(
    state: State<'_, AppState>,
    version_id: String,
) -> Result<RestoreResult, String> {
    let store = state.store();
    let target = store.get_version(&version_id).map_err(|e| e.to_string())?;

    // If the target version is already the latest, short-circuit
    if let Some(latest) = store
        .latest_version(&target.record.skill_id)
        .map_err(|e| e.to_string())?
    {
        if latest.id == version_id {
            return Ok(RestoreResult {
                skill_id: target.record.skill_id,
                new_version_no: None,
                no_op: true,
                message: "target is already the latest version".to_string(),
            });
        }
    }

    // Resolve central_path to write the SKILL.md
    let skill = store
        .get_skill(&target.record.skill_id)
        .map_err(|e| e.to_string())?;
    let skill_md = std::path::Path::new(&skill.central_path).join("SKILL.md");

    std::fs::write(&skill_md, &target.content)
        .map_err(|e| format!("failed to write {}: {e}", skill_md.display()))?;

    // Capture as a new version with trigger=restore
    let new_version = store
        .restore_version(&version_id)
        .map_err(|e| e.to_string())?;

    // Trigger a sync of the active scenario so all agents receive the restored content.
    // Reuse existing sync command if available; otherwise log a hint.
    if let Err(err) = crate::commands::scenarios::sync_current_scenario_internal(&state) {
        log::warn!("scenario sync after restore failed: {err}");
    }

    Ok(RestoreResult {
        skill_id: new_version.record.skill_id,
        new_version_no: Some(new_version.record.version_no),
        no_op: false,
        message: "restored".to_string(),
    })
}
```

Notes for implementer:
- `store.get_skill(id)` — verify the exact method name in `skill_store.rs` (may be `get_skill_by_id` or similar).
- `crate::commands::scenarios::sync_current_scenario_internal` — replace with the actual sync entry point exposed in `commands/scenarios.rs`. If only a Tauri command exists, add a thin `pub(crate) fn` wrapper that takes `&AppState` and calls the same core logic, then use that here.

- [ ] **Step 2: Compile check**

```bash
cd src-tauri && cargo check
```

Expected: clean build. Fix any signature mismatches flagged by the compiler.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/history.rs
# (possibly also src-tauri/src/commands/scenarios.rs if the internal wrapper was added)
git commit -m "feat(tauri): restore_version command + post-restore sync"
```

---

### Task 10: Register commands + startup backfill trigger

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Register all history commands**

Edit `src-tauri/src/lib.rs` — find the `.invoke_handler(tauri::generate_handler![...])` call and append:

```rust
commands::history::list_skills_with_history,
commands::history::list_versions,
commands::history::get_version,
commands::history::diff_versions,
commands::history::restore_version,
```

- [ ] **Step 2: Trigger backfill on app startup**

In `src-tauri/src/lib.rs`, locate the `setup(|app| { ... })` block (or equivalent startup hook where `SkillStore` is initialized). After `SkillStore` is ready, spawn a background thread:

```rust
let store_for_backfill = app_state.store_arc();
std::thread::spawn(move || {
    match store_for_backfill.backfill_initial_versions() {
        Ok(n) if n > 0 => log::info!("backfilled {n} initial skill versions"),
        Ok(_) => {}
        Err(err) => log::warn!("initial skill version backfill failed: {err}"),
    }
});
```

Replace `store_arc()` with the actual method returning `Arc<SkillStore>`. If none, add one.

- [ ] **Step 3: Manual smoke test**

```bash
pnpm tauri dev
```

Expected: app launches, logs show `"backfilled N initial skill versions"` on first run after migration. Subsequent launches show no message (idempotent because `backfill_initial_versions` skips skills that already have versions).

Verify via:

```bash
sqlite3 ~/.skills-manager/skills-manager.db "SELECT COUNT(*) FROM skill_versions;"
```

Expected: matches skill count (up to 132).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(tauri): register history commands + startup backfill"
```

---

## Phase 4 — Frontend

### Task 11: npm deps + IPC wrappers + TS types

**Files:**
- Modify: `package.json`
- Modify: `src/lib/tauri.ts`
- Create: `src/types/history.ts`

- [ ] **Step 1: Install deps**

```bash
pnpm add react-diff-view diff-match-patch
pnpm add -D @types/diff-match-patch
```

- [ ] **Step 2: Create types**

Create `src/types/history.ts`:

```ts
export interface SkillHistorySummary {
  id: string;
  name: string;
  description: string | null;
  source_type: string;
  source_ref: string | null;
  source_ref_resolved: string | null;
  content_hash: string | null;
  created_at: number | null;
  updated_at: number | null;
  version_count: number;
  latest_captured_at: number | null;
}

export interface VersionRecord {
  id: string;
  skill_id: string;
  version_no: number;
  content_hash: string;
  byte_size: number;
  captured_at: number;
  trigger: "scan" | "import" | "backfill" | "restore";
  source_type: string;
  source_ref: string | null;
  source_ref_resolved: string | null;
}

export interface VersionContent {
  record: VersionRecord;
  content: string;
}

export type DiffLineKind = "Context" | "Added" | "Removed";

export interface DiffLine {
  kind: DiffLineKind;
  old_no: number | null;
  new_no: number | null;
  text: string;
}

export interface DiffHunk {
  header: string;
  lines: DiffLine[];
}

export interface RestoreResult {
  skill_id: string;
  new_version_no: number | null;
  no_op: boolean;
  message: string;
}
```

- [ ] **Step 3: Add IPC wrappers**

Edit `src/lib/tauri.ts` — append:

```ts
import type {
  DiffHunk,
  RestoreResult,
  SkillHistorySummary,
  VersionContent,
  VersionRecord,
} from "../types/history";

export const history = {
  listSkills: () => invoke<SkillHistorySummary[]>("list_skills_with_history"),
  listVersions: (skillId: string) =>
    invoke<VersionRecord[]>("list_versions", { skillId }),
  getVersion: (versionId: string) =>
    invoke<VersionContent>("get_version", { versionId }),
  diffVersions: (oldId: string, newId: string) =>
    invoke<DiffHunk[]>("diff_versions", { oldId, newId }),
  restoreVersion: (versionId: string) =>
    invoke<RestoreResult>("restore_version", { versionId }),
};
```

Match the casing convention the rest of the file uses (the codebase may use `snake_case` arg keys directly — check one existing wrapper first).

- [ ] **Step 4: Typecheck**

```bash
pnpm run lint
```

Expected: no new errors.

- [ ] **Step 5: Commit**

```bash
git add package.json pnpm-lock.yaml src/types/history.ts src/lib/tauri.ts
git commit -m "feat(ui): history types + IPC wrappers + react-diff-view dep"
```

---

### Task 12: Sidebar route + HistoryView shell + SkillListPane

**Files:**
- Modify: `src/App.tsx`
- Create: `src/views/HistoryView.tsx`
- Create: `src/views/history/SkillListPane.tsx`

- [ ] **Step 1: Add sidebar + route**

Edit `src/App.tsx`:
- Import `History` icon from `lucide-react`.
- Add a nav entry: `{ path: "/history", label: t("history.tab"), icon: History }` (check existing nav structure — mirror it).
- Add a route: `<Route path="/history" element={<HistoryView />} />`.

- [ ] **Step 2: Create HistoryView shell**

Create `src/views/HistoryView.tsx`:

```tsx
import { useEffect, useState } from "react";
import { history as api } from "../lib/tauri";
import type { SkillHistorySummary, VersionRecord } from "../types/history";
import { SkillListPane } from "./history/SkillListPane";

export function HistoryView() {
  const [skills, setSkills] = useState<SkillHistorySummary[]>([]);
  const [selectedSkillId, setSelectedSkillId] = useState<string | null>(null);
  const [versions, setVersions] = useState<VersionRecord[]>([]);

  useEffect(() => {
    api.listSkills().then(setSkills).catch(console.error);
  }, []);

  useEffect(() => {
    if (!selectedSkillId) return;
    api.listVersions(selectedSkillId).then(setVersions).catch(console.error);
  }, [selectedSkillId]);

  const selectedSkill = skills.find((s) => s.id === selectedSkillId) ?? null;

  return (
    <div className="flex h-full">
      <SkillListPane
        skills={skills}
        selectedId={selectedSkillId}
        onSelect={setSelectedSkillId}
      />
      <div className="flex-1 flex flex-col p-4">
        {!selectedSkill ? (
          <div className="text-gray-500">Select a skill to view its history.</div>
        ) : (
          <>
            <div className="text-lg font-semibold">{selectedSkill.name}</div>
            <div className="text-sm text-gray-500">
              {versions.length} versions
            </div>
          </>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Create SkillListPane**

Create `src/views/history/SkillListPane.tsx`:

```tsx
import { useState, useMemo } from "react";
import type { SkillHistorySummary } from "../../types/history";

interface Props {
  skills: SkillHistorySummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}

export function SkillListPane({ skills, selectedId, onSelect }: Props) {
  const [query, setQuery] = useState("");
  const filtered = useMemo(() => {
    const q = query.toLowerCase();
    return skills.filter((s) => s.name.toLowerCase().includes(q));
  }, [skills, query]);

  return (
    <div className="w-72 border-r h-full flex flex-col">
      <input
        type="text"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search skills…"
        className="m-2 px-2 py-1 border rounded"
      />
      <ul className="flex-1 overflow-y-auto">
        {filtered.map((s) => (
          <li
            key={s.id}
            onClick={() => onSelect(s.id)}
            className={`px-3 py-2 cursor-pointer hover:bg-gray-100 ${
              s.id === selectedId ? "bg-blue-50" : ""
            }`}
          >
            <div className="font-medium">{s.name}</div>
            <div className="text-xs text-gray-500">
              {s.source_type} · {s.version_count} versions
            </div>
          </li>
        ))}
      </ul>
      <div className="text-xs text-gray-400 p-2 border-t">
        {skills.length} skills total
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Manual smoke test**

```bash
pnpm tauri dev
```

Click the History tab, confirm the skill list populates and search narrows the list.

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx src/views/HistoryView.tsx src/views/history/SkillListPane.tsx
git commit -m "feat(ui): HistoryView shell + SkillListPane"
```

---

### Task 13: MetadataPanel + VersionListPane (2-selection logic)

**Files:**
- Create: `src/views/history/MetadataPanel.tsx`
- Create: `src/views/history/VersionListPane.tsx`
- Modify: `src/views/HistoryView.tsx`

- [ ] **Step 1: Create MetadataPanel**

Create `src/views/history/MetadataPanel.tsx`:

```tsx
import type { SkillHistorySummary } from "../../types/history";

export function MetadataPanel({ skill }: { skill: SkillHistorySummary }) {
  const fmt = (ts: number | null) =>
    ts ? new Date(ts * 1000).toLocaleString() : "—";
  return (
    <div className="border-b p-3 text-sm">
      <div className="flex items-center gap-2">
        <span className="font-semibold text-base">{skill.name}</span>
        <span className="text-xs uppercase px-2 py-0.5 bg-gray-200 rounded">
          {skill.source_type}
        </span>
      </div>
      {skill.source_ref && (
        <div className="text-xs text-gray-600 truncate">{skill.source_ref}</div>
      )}
      <div className="mt-1 flex gap-4 text-xs text-gray-500">
        <span>{skill.version_count} versions</span>
        <span>first seen: {fmt(skill.created_at)}</span>
        <span>last update: {fmt(skill.updated_at)}</span>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Create VersionListPane with 2-selection logic**

Create `src/views/history/VersionListPane.tsx`:

```tsx
import type { VersionRecord } from "../../types/history";

interface Props {
  versions: VersionRecord[];
  selectedIds: [string | null, string | null]; // [older, newer]
  onToggle: (id: string) => void;
}

export function VersionListPane({ versions, selectedIds, onToggle }: Props) {
  const checked = (id: string) =>
    selectedIds[0] === id || selectedIds[1] === id;

  const relTime = (ts: number) => {
    const diff = Date.now() / 1000 - ts;
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    return `${Math.floor(diff / 86400)}d ago`;
  };

  return (
    <div className="flex-1 overflow-y-auto border-b">
      <table className="w-full text-sm">
        <thead className="bg-gray-50 sticky top-0">
          <tr>
            <th className="w-8"></th>
            <th className="text-left px-2">Version</th>
            <th className="text-left px-2">Hash</th>
            <th className="text-left px-2">Captured</th>
            <th className="text-left px-2">Trigger</th>
          </tr>
        </thead>
        <tbody>
          {versions.map((v) => (
            <tr
              key={v.id}
              className={`border-t hover:bg-gray-50 ${
                checked(v.id) ? "bg-blue-50" : ""
              }`}
            >
              <td className="text-center">
                <input
                  type="checkbox"
                  checked={checked(v.id)}
                  onChange={() => onToggle(v.id)}
                />
              </td>
              <td className="px-2">v{v.version_no}</td>
              <td className="px-2 font-mono text-xs">
                {v.content_hash.slice(0, 8)}
              </td>
              <td className="px-2 text-gray-600">{relTime(v.captured_at)}</td>
              <td className="px-2">
                <span className="text-xs uppercase px-1.5 py-0.5 bg-gray-200 rounded">
                  {v.trigger}
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
```

- [ ] **Step 3: Wire selection logic in HistoryView**

Edit `src/views/HistoryView.tsx`:

Replace the right-side placeholder with:

```tsx
import { MetadataPanel } from "./history/MetadataPanel";
import { VersionListPane } from "./history/VersionListPane";

// Inside HistoryView():
const [selectedVersions, setSelectedVersions] = useState<
  [string | null, string | null]
>([null, null]);

// Auto-select newest two when versions load
useEffect(() => {
  if (versions.length >= 2) {
    setSelectedVersions([versions[1].id, versions[0].id]);
  } else if (versions.length === 1) {
    setSelectedVersions([null, versions[0].id]);
  } else {
    setSelectedVersions([null, null]);
  }
}, [versions]);

const toggleVersion = (id: string) => {
  setSelectedVersions((prev) => {
    if (prev[0] === id) return [null, prev[1]];
    if (prev[1] === id) return [prev[0], null];
    // Fill older slot first, then newer, evicting older of the two when full
    if (prev[0] === null) return [id, prev[1]];
    if (prev[1] === null) return [prev[0], id];
    return [prev[1], id]; // evict older
  });
};

// In JSX, replace the "Select a skill…" block:
{!selectedSkill ? (
  <div className="text-gray-500 p-4">Select a skill to view its history.</div>
) : (
  <>
    <MetadataPanel skill={selectedSkill} />
    <VersionListPane
      versions={versions}
      selectedIds={selectedVersions}
      onToggle={toggleVersion}
    />
    <div className="p-4 text-sm text-gray-500">
      {selectedVersions[0] && selectedVersions[1]
        ? "Diff goes here (Task 14)"
        : "Select two versions to compare."}
    </div>
  </>
)}
```

- [ ] **Step 4: Manual test**

Run `pnpm tauri dev`. Select a skill with >=2 versions (use manual edit + wait for watcher if needed); confirm auto-selection of newest two works and checkbox toggle evicts correctly.

- [ ] **Step 5: Commit**

```bash
git add src/views/history/MetadataPanel.tsx \
        src/views/history/VersionListPane.tsx \
        src/views/HistoryView.tsx
git commit -m "feat(ui): MetadataPanel + VersionListPane with 2-selection"
```

---

### Task 14: DiffPane using react-diff-view

**Files:**
- Create: `src/views/history/DiffPane.tsx`
- Modify: `src/views/HistoryView.tsx`

- [ ] **Step 1: Create DiffPane**

Create `src/views/history/DiffPane.tsx`:

```tsx
import { useEffect, useState } from "react";
import { Diff, Hunk, parseDiff } from "react-diff-view";
import "react-diff-view/style/index.css";

import { history as api } from "../../lib/tauri";
import type { DiffHunk, VersionContent } from "../../types/history";

interface Props {
  oldVersionId: string;
  newVersionId: string;
}

// Convert our DiffHunk[] to the unified text format that react-diff-view parses.
function toUnifiedText(hunks: DiffHunk[], oldName: string, newName: string): string {
  const header = `--- ${oldName}\n+++ ${newName}\n`;
  const body = hunks
    .map((h) => {
      const lines = h.lines
        .map((l) => {
          const prefix =
            l.kind === "Added" ? "+" : l.kind === "Removed" ? "-" : " ";
          // Strip trailing newline since patch format expects line-by-line
          return `${prefix}${l.text.replace(/\n$/, "")}`;
        })
        .join("\n");
      return `${h.header}\n${lines}`;
    })
    .join("\n");
  return `${header}${body}\n`;
}

export function DiffPane({ oldVersionId, newVersionId }: Props) {
  const [oldV, setOldV] = useState<VersionContent | null>(null);
  const [newV, setNewV] = useState<VersionContent | null>(null);
  const [hunks, setHunks] = useState<DiffHunk[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    setLoading(true);
    Promise.all([
      api.getVersion(oldVersionId),
      api.getVersion(newVersionId),
      api.diffVersions(oldVersionId, newVersionId),
    ])
      .then(([o, n, h]) => {
        setOldV(o);
        setNewV(n);
        setHunks(h);
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [oldVersionId, newVersionId]);

  if (loading) return <div className="p-4 text-gray-500">Loading diff…</div>;
  if (!oldV || !newV) return null;
  if (hunks.length === 0)
    return <div className="p-4 text-gray-500">Versions are identical.</div>;

  const unified = toUnifiedText(
    hunks,
    `v${oldV.record.version_no}`,
    `v${newV.record.version_no}`,
  );
  const files = parseDiff(unified);

  return (
    <div className="flex-1 overflow-auto p-2">
      {files.map((file, i) => (
        <Diff key={i} viewType="split" diffType={file.type} hunks={file.hunks}>
          {(parsedHunks) =>
            parsedHunks.map((h) => <Hunk key={h.content} hunk={h} />)
          }
        </Diff>
      ))}
    </div>
  );
}
```

- [ ] **Step 2: Wire DiffPane into HistoryView**

Edit `src/views/HistoryView.tsx`:

Replace the placeholder text block with:

```tsx
import { DiffPane } from "./history/DiffPane";

// ...
{selectedVersions[0] && selectedVersions[1] ? (
  <DiffPane
    oldVersionId={selectedVersions[0]}
    newVersionId={selectedVersions[1]}
  />
) : (
  <div className="p-4 text-gray-500">Select two versions to compare.</div>
)}
```

- [ ] **Step 3: Manual test**

```bash
pnpm tauri dev
```

Pick a skill, edit its central library SKILL.md, wait for watcher; two versions should render split diff correctly.

- [ ] **Step 4: Commit**

```bash
git add src/views/history/DiffPane.tsx src/views/HistoryView.tsx
git commit -m "feat(ui): split-view diff using react-diff-view"
```

---

## Phase 5 — Restore

### Task 15: RestoreConfirmDialog + wire-up

**Files:**
- Create: `src/views/history/RestoreConfirmDialog.tsx`
- Modify: `src/views/HistoryView.tsx`

- [ ] **Step 1: Create dialog**

Create `src/views/history/RestoreConfirmDialog.tsx`:

```tsx
import type { VersionRecord } from "../../types/history";

interface Props {
  version: VersionRecord;
  open: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function RestoreConfirmDialog({ version, open, onConfirm, onCancel }: Props) {
  if (!open) return null;
  return (
    <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50">
      <div className="bg-white rounded shadow-lg p-6 max-w-md">
        <h2 className="text-lg font-semibold mb-2">Restore version v{version.version_no}?</h2>
        <p className="text-sm text-gray-700 mb-4">
          This will write the content of v{version.version_no} back to the central
          library and re-sync the active scenario to all agents. The existing
          history is preserved — a new version is created pointing at this
          content.
        </p>
        <div className="flex justify-end gap-2">
          <button
            onClick={onCancel}
            className="px-3 py-1 border rounded"
          >Cancel</button>
          <button
            onClick={onConfirm}
            className="px-3 py-1 bg-blue-600 text-white rounded"
          >Restore</button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Wire in HistoryView**

Edit `src/views/HistoryView.tsx`:

```tsx
import { RestoreConfirmDialog } from "./history/RestoreConfirmDialog";

// Add state:
const [restoreTarget, setRestoreTarget] = useState<VersionRecord | null>(null);

// Add a "Restore" button next to the version list. Enabled only when exactly one
// non-latest version is selected.
const selectedVersion = versions.find(
  (v) =>
    (v.id === selectedVersions[0] && selectedVersions[1] === null) ||
    (v.id === selectedVersions[1] && selectedVersions[0] === null),
);
const latestVersion = versions[0];
const canRestore =
  selectedVersion && latestVersion && selectedVersion.id !== latestVersion.id;

// In the JSX, below VersionListPane:
<div className="px-3 py-2 border-b flex justify-end">
  <button
    disabled={!canRestore}
    onClick={() => selectedVersion && setRestoreTarget(selectedVersion)}
    className="px-3 py-1 rounded border text-sm disabled:opacity-40"
  >
    Restore this version
  </button>
</div>

{restoreTarget && (
  <RestoreConfirmDialog
    version={restoreTarget}
    open={true}
    onCancel={() => setRestoreTarget(null)}
    onConfirm={async () => {
      try {
        await api.restoreVersion(restoreTarget.id);
        // Refresh versions
        const next = await api.listVersions(selectedSkillId!);
        setVersions(next);
      } catch (err) {
        console.error("restore failed", err);
      } finally {
        setRestoreTarget(null);
      }
    }}
  />
)}
```

- [ ] **Step 3: Manual test**

```bash
pnpm tauri dev
```

Pick a skill with ≥2 versions, select a single non-latest version, click Restore, confirm. Expected: new version appears at top of list with trigger `restore`, central library SKILL.md has restored content, agents receive it via sync.

- [ ] **Step 4: Commit**

```bash
git add src/views/history/RestoreConfirmDialog.tsx src/views/HistoryView.tsx
git commit -m "feat(ui): restore version flow with confirm dialog"
```

---

## Phase 6 — i18n + polish

### Task 16: i18n keys + empty states + final QA

**Files:**
- Modify: `src/i18n/en.json`
- Modify: `src/i18n/zh.json`
- Modify: `src/i18n/zh-TW.json`
- Modify: HistoryView + subcomponents to use `t()` instead of hardcoded strings

- [ ] **Step 1: Add i18n keys**

For each locale file, add a `history` section mirroring existing conventions:

```jsonc
"history": {
  "tab": "History",
  "searchPlaceholder": "Search skills…",
  "selectSkill": "Select a skill to view its history.",
  "noVersions": "No history recorded for this skill yet.",
  "oneVersion": "Only one version exists — nothing to compare.",
  "selectTwo": "Select two versions to compare.",
  "identical": "Versions are identical.",
  "loadingDiff": "Loading diff…",
  "restore": "Restore this version",
  "restoreTitle": "Restore version v{version}?",
  "restoreBody": "This will write the content of v{version} back to the central library and re-sync the active scenario.",
  "cancel": "Cancel",
  "confirmRestore": "Restore",
  "versionCount": "{count} versions",
  "skillsTotal": "{count} skills total",
  "firstSeen": "first seen",
  "lastUpdate": "last update",
  "trigger": {
    "scan": "scan",
    "import": "import",
    "backfill": "backfill",
    "restore": "restore"
  }
}
```

Provide translations for `zh.json` (simplified) and `zh-TW.json` (traditional). Match the tone of existing keys.

- [ ] **Step 2: Replace hardcoded strings**

Use the existing `useTranslation` hook (check any other view for the pattern). Replace literal strings in HistoryView and its children.

- [ ] **Step 3: Handle empty states**

In HistoryView, render:
- `history.selectSkill` when `!selectedSkill`
- `history.noVersions` when `versions.length === 0`
- `history.oneVersion` when `versions.length === 1`
- `history.selectTwo` when both selections not complete

- [ ] **Step 4: Final manual QA walkthrough**

Run `pnpm tauri dev` and step through:

1. History tab loads all skills, sorted by most recently captured first.
2. Search narrows the skill list.
3. Clicking a skill auto-selects the two newest versions.
4. Diff renders split-view with correct colors.
5. Toggling a third checkbox evicts the older-of-two.
6. Metadata panel shows source type, refs, timestamps, version count.
7. Editing a skill's central SKILL.md externally triggers a new `scan` version within a few seconds.
8. Restore button is disabled for latest, enabled for older-single-selection.
9. Restore dialog confirmation writes SKILL.md + creates `restore` version + re-syncs agents.
10. Language switch (en/zh/zh-TW) updates every string.

- [ ] **Step 5: Commit**

```bash
git add src/i18n/ src/views/HistoryView.tsx src/views/history/
git commit -m "feat(ui): history i18n + empty states + final polish"
```

- [ ] **Step 6: Open PR**

```bash
git push -u origin <branch>
gh pr create --title "feat: skill version history & diff" --body "$(cat <<'EOF'
## Summary
- Adds History tab with side-by-side diff between any two skill versions
- Captures a snapshot whenever central library content hash changes (LRU 50)
- Backfills existing skills on first launch
- Restore flow writes older version back to central library + re-syncs agents

## Test plan
- [ ] Core Rust tests pass (`cargo test` in workspace)
- [ ] Startup backfill creates one version per existing skill
- [ ] External edit to SKILL.md creates a new `scan` version
- [ ] Two-version split diff renders correctly
- [ ] Restore older version creates new `restore` version + updates central + re-syncs

Spec: docs/superpowers/specs/2026-04-17-skill-version-history-design.md
Plan: docs/superpowers/plans/2026-04-17-skill-version-history.md
EOF
)"
```

---

## Self-Review Checklist

- [x] Every spec section maps to one or more tasks (schema → T1; capture → T2; list/get/restore → T3; backfill → T4; diff → T5; hooks → T6; rescan → T7; IPC read → T8; IPC restore → T9; startup + registration → T10; UI shell → T11/T12; version selection → T13; diff UI → T14; restore UI → T15; i18n + empty states → T16).
- [x] No placeholders (TBD/TODO) inside tasks.
- [x] All types referenced (`VersionRecord`, `VersionContent`, `DiffHunk`, `CaptureTrigger`, `SkillHistorySummary`, `RestoreResult`) are defined in earlier tasks before use.
- [x] Exact method names consistent across tasks: `capture_version`, `list_versions`, `get_version`, `latest_version`, `restore_version`, `backfill_initial_versions`, `rescan_central_library`.
- [x] Frontend types mirror Rust enum casing (`"Context" | "Added" | "Removed"` matches serde default for enum variant names — verified implicit in serde behavior).

**Watch-out during execution:** Task 2 Step 5 assumes `SkillStore::conn()` accessor; if the existing private field pattern differs, add the accessor in `skill_store.rs` first. Task 9 Step 1 assumes a re-usable sync entry point; if only a Tauri command exists, create a `pub(crate)` wrapper first.
