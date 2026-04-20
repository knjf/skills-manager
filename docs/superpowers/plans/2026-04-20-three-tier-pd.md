# Three-Tier Progressive Disclosure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split Progressive Disclosure into three storage tiers (pack-level L1 with `when_to_use`, per-skill L2 compressed lines, L3 original SKILL.md) backed by DB columns, so router bodies carry authored per-skill "which-to-pick" differentiation instead of verbose original descriptions.

**Architecture:** Add two nullable DB columns (`packs.router_when_to_use`, `skills.description_router`) via migration v11. Update `PackRecord`/`SkillRecord` structs, SELECT/INSERT queries, and the `map_*_row` helpers. Change `render_router_skill_md` to emit `when_to_use` in frontmatter and use `description_router` in the table column with fallback to `description`. Extend CLI: `pack set-router` gains `--when-to-use`, new `skill set-router-desc` for single edits and `skill import-router-descs` for bulk YAML import.

**Tech Stack:** Rust (anyhow, rusqlite, clap, serde_yaml for YAML import). Existing skill-store / sync-engine / CLI architecture unchanged.

**Spec:** `docs/superpowers/specs/2026-04-20-three-tier-pd-design.md`

---

## File Map

**Modify:**
- `crates/skills-manager-core/src/migrations.rs` — add v10→v11 migration, bump `LATEST_VERSION`
- `crates/skills-manager-core/src/skill_store.rs`:
  - `PackRecord` struct (add `router_when_to_use`)
  - `SkillRecord` struct (add `description_router`)
  - `map_pack_row`, `map_skill_row`
  - `insert_skill` INSERT statement + params
  - All `SELECT` statements pulling packs/skills: `get_all_packs`, `get_pack_by_id`, `get_packs_for_scenario`, `get_agent_extra_packs`, `get_all_skills`, `get_skill`, `get_skills_for_pack`, `get_effective_skills_for_scenario`, `get_effective_skills_for_agent` (both queries)
  - Add `set_pack_when_to_use`, `set_skill_description_router`, `bulk_set_skill_description_router`
  - Fixture helpers: `insert_test_skill` (pack_tests module and any copies)
- `crates/skills-manager-core/src/router_render.rs` — emit `when_to_use`, fall back for skill row
- `crates/skills-manager-cli/src/main.rs` — extend `PackAction::SetRouter` flags, add `SkillAction`
- `crates/skills-manager-cli/src/commands.rs` — update `cmd_pack_set_router`, add `cmd_skill_set_router_desc` + `cmd_skill_import_router_descs`
- `crates/skills-manager-cli/Cargo.toml` — add `serde_yaml` to dependencies
- `crates/skills-manager-cli/tests/pd_wiring.rs` OR new `tests/three_tier.rs` — integration tests

**Create:**
- (none — all changes are modifications)

---

## Task 1: DB v11 migration

**Files:**
- Modify: `crates/skills-manager-core/src/migrations.rs`

- [ ] **Step 1: Write the failing test**

Append to the test module at the bottom of `migrations.rs` (search for `v9_to_v10_creates_skill_versions_table` to find the module — pattern-match its location, append a sibling test):

```rust
    #[test]
    fn v10_to_v11_adds_three_tier_columns() {
        let conn = Connection::open_in_memory().unwrap();
        // Boot to v10 first.
        for v in 0..10 {
            conn.execute_batch("BEGIN EXCLUSIVE").unwrap();
            migrate_step(&conn, v).unwrap();
            conn.pragma_update(None, "user_version", v + 1).unwrap();
            conn.execute_batch("COMMIT").unwrap();
        }

        // Run v10 → v11.
        conn.execute_batch("BEGIN EXCLUSIVE").unwrap();
        migrate_step(&conn, 10).unwrap();
        conn.pragma_update(None, "user_version", 11u32).unwrap();
        conn.execute_batch("COMMIT").unwrap();

        // Verify both columns exist.
        let pack_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(packs)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(pack_cols.iter().any(|c| c == "router_when_to_use"),
            "packs.router_when_to_use missing; got columns: {pack_cols:?}");

        let skill_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(skills)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(skill_cols.iter().any(|c| c == "description_router"),
            "skills.description_router missing; got columns: {skill_cols:?}");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p skills-manager-core --lib migrations::tests::v10_to_v11_adds_three_tier_columns --no-fail-fast`

Expected: FAIL — `migrate_step` called with version 10 hits the `_ => bail!("unknown migration version: 10")` branch.

- [ ] **Step 3: Add the migration function and register it**

At the top of `migrations.rs`, bump the version constant:

```rust
const LATEST_VERSION: u32 = 11;
```

Add the new migration function near the other `migrate_vN_to_vM` functions (after `migrate_v9_to_v10`):

```rust
/// v10 → v11: Add three-tier progressive disclosure fields.
/// `packs.router_when_to_use` — native Claude Code frontmatter field.
/// `skills.description_router` — per-skill compressed "which-to-pick" line for router body.
fn migrate_v10_to_v11(conn: &Connection) -> Result<()> {
    add_column_if_missing(conn, "packs", "router_when_to_use", "TEXT")?;
    add_column_if_missing(conn, "skills", "description_router", "TEXT")?;
    Ok(())
}
```

In `migrate_step`, add a dispatch arm:

```rust
        9 => migrate_v9_to_v10(conn),
        10 => migrate_v10_to_v11(conn),
        _ => bail!("unknown migration version: {from_version}"),
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p skills-manager-core --lib migrations::tests::v10_to_v11_adds_three_tier_columns --no-fail-fast`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/skills-manager-core/src/migrations.rs
git commit -m "feat(core): DB v11 — add router_when_to_use + description_router columns"
```

---

## Task 2: Extend `PackRecord` with `router_when_to_use`

**Files:**
- Modify: `crates/skills-manager-core/src/skill_store.rs`

This task adds the new Option<String> field to `PackRecord`, updates the row mapper, updates every SELECT that reads packs, and updates every struct literal that constructs a PackRecord.

- [ ] **Step 1: Add field to `PackRecord` struct (line 65)**

Change:

```rust
pub struct PackRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
    pub router_description: Option<String>,
    pub router_body: Option<String>,
    pub is_essential: bool,
    pub router_updated_at: Option<i64>,
}
```

to:

```rust
pub struct PackRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
    pub router_description: Option<String>,
    pub router_body: Option<String>,
    pub is_essential: bool,
    pub router_updated_at: Option<i64>,
    pub router_when_to_use: Option<String>,
}
```

- [ ] **Step 2: Update `map_pack_row` to read the new column**

Find `fn map_pack_row` (around line 1981). Change:

```rust
fn map_pack_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PackRecord> {
    Ok(PackRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        icon: row.get(3)?,
        color: row.get(4)?,
        sort_order: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        router_description: row.get(8)?,
        router_body: row.get(9)?,
        is_essential: row.get::<_, i64>(10)? != 0,
        router_updated_at: row.get(11)?,
    })
}
```

to:

```rust
fn map_pack_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PackRecord> {
    Ok(PackRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        icon: row.get(3)?,
        color: row.get(4)?,
        sort_order: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        router_description: row.get(8)?,
        router_body: row.get(9)?,
        is_essential: row.get::<_, i64>(10)? != 0,
        router_updated_at: row.get(11)?,
        router_when_to_use: row.get(12)?,
    })
}
```

- [ ] **Step 3: Update every SELECT that reads packs to include the new column**

Search for SELECTs that reference `router_description, router_body`:

Run: `grep -n 'router_description, router_body' crates/skills-manager-core/src/skill_store.rs`

At each match, extend the column list from:

```
router_description, router_body, is_essential, router_updated_at
```

to:

```
router_description, router_body, is_essential, router_updated_at, router_when_to_use
```

Known sites (verify by grep — exact line numbers may drift):
- `get_all_packs` SELECT
- `get_pack_by_id` SELECT
- `get_packs_for_scenario` SELECT
- `get_agent_extra_packs` SELECT

- [ ] **Step 4: Update struct literals that construct `PackRecord`**

Run: `grep -n 'PackRecord {' crates/skills-manager-core/src/skill_store.rs crates/skills-manager-core/src/router_render.rs crates/skills-manager-core/src/sync_engine/disclosure.rs crates/skills-manager-core/src/sync_engine/mod.rs`

For each struct literal (mostly in test code), add `router_when_to_use: None,` to the literal. Example — in `router_render.rs` test helper `fn pack(...)`:

```rust
        PackRecord {
            id: format!("p-{name}"),
            name: name.into(),
            description: None,
            icon: None,
            color: None,
            sort_order: 0,
            created_at: 0,
            updated_at: 0,
            router_description: router_desc.map(str::to_string),
            router_body: None,
            is_essential: false,
            router_updated_at: None,
            router_when_to_use: None,
        }
```

Apply the same addition to every `PackRecord { ... }` literal found.

- [ ] **Step 5: Write test for round-trip**

Append to the `pack_tests` module (search for `mod pack_tests` and place near other pack round-trip tests):

```rust
    #[test]
    fn pack_record_round_trips_router_when_to_use() {
        let (store, _tmp) = test_store();
        store.insert_pack("p-x", "pack-x", None, None, None).unwrap();
        // Directly write via SQL since set_pack_when_to_use arrives in Task 3.
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE packs SET router_when_to_use = ?1 WHERE id = 'p-x'",
                ["Trigger when user says X"],
            ).unwrap();
        }
        let packs = store.get_all_packs().unwrap();
        let px = packs.iter().find(|p| p.id == "p-x").expect("pack-x missing");
        assert_eq!(px.router_when_to_use.as_deref(), Some("Trigger when user says X"));
    }
```

- [ ] **Step 6: Run all skill_store tests to confirm nothing broke**

Run: `cargo test -p skills-manager-core --lib skill_store:: --no-fail-fast`

Expected: all tests pass including the new one.

- [ ] **Step 7: Commit**

```bash
git add crates/skills-manager-core/src/skill_store.rs crates/skills-manager-core/src/router_render.rs crates/skills-manager-core/src/sync_engine/disclosure.rs crates/skills-manager-core/src/sync_engine/mod.rs
git commit -m "feat(core): PackRecord carries router_when_to_use"
```

---

## Task 3: Extend `SkillRecord` with `description_router`

**Files:**
- Modify: `crates/skills-manager-core/src/skill_store.rs`
- Modify: test fixture files that construct `SkillRecord` literals

- [ ] **Step 1: Add field to `SkillRecord` struct (around line 18)**

Change the struct to add `description_router` at the end (after `last_check_error`):

```rust
pub struct SkillRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub source_ref_resolved: Option<String>,
    pub source_subpath: Option<String>,
    pub source_branch: Option<String>,
    pub source_revision: Option<String>,
    pub remote_revision: Option<String>,
    pub central_path: String,
    pub content_hash: Option<String>,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub status: String,
    pub update_status: String,
    pub last_checked_at: Option<i64>,
    pub last_check_error: Option<String>,
    pub description_router: Option<String>,
}
```

- [ ] **Step 2: Update `map_skill_row` (around line 1998)**

Change:

```rust
fn map_skill_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SkillRecord> {
    Ok(SkillRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        source_type: row.get(3)?,
        source_ref: row.get(4)?,
        source_ref_resolved: row.get(5)?,
        source_subpath: row.get(6)?,
        source_branch: row.get(7)?,
        source_revision: row.get(8)?,
        remote_revision: row.get(9)?,
        central_path: row.get(10)?,
        content_hash: row.get(11)?,
        enabled: row.get::<_, i32>(12)? != 0,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        status: row.get(15)?,
        update_status: row.get(16)?,
        last_checked_at: row.get(17)?,
        last_check_error: row.get(18)?,
    })
}
```

to add the new field at index 19:

```rust
fn map_skill_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SkillRecord> {
    Ok(SkillRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        source_type: row.get(3)?,
        source_ref: row.get(4)?,
        source_ref_resolved: row.get(5)?,
        source_subpath: row.get(6)?,
        source_branch: row.get(7)?,
        source_revision: row.get(8)?,
        remote_revision: row.get(9)?,
        central_path: row.get(10)?,
        content_hash: row.get(11)?,
        enabled: row.get::<_, i32>(12)? != 0,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        status: row.get(15)?,
        update_status: row.get(16)?,
        last_checked_at: row.get(17)?,
        last_check_error: row.get(18)?,
        description_router: row.get(19)?,
    })
}
```

- [ ] **Step 3: Update every SELECT that reads skills to include the new column**

Run: `grep -n 'last_checked_at, last_check_error' crates/skills-manager-core/src/skill_store.rs`

At each match, extend the SELECT column list from:

```
... last_checked_at, last_check_error
```

to:

```
... last_checked_at, last_check_error, description_router
```

Known sites:
- `get_all_skills`
- `get_skill` (singular)
- `get_skills_for_pack`
- `get_effective_skills_for_scenario`
- `get_effective_skills_for_agent`

Any SELECT the grep surfaces must be updated. If a site does `SELECT s.id, s.name, ..., s.last_check_error FROM skills s ...`, prefix the new column appropriately (e.g. `s.description_router`).

- [ ] **Step 4: Update `insert_skill` INSERT statement (around line 207)**

Change:

```rust
    pub fn insert_skill(&self, skill: &SkillRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO skills (
                id, name, description, source_type, source_ref, source_ref_resolved, source_subpath,
                source_branch, source_revision, remote_revision, central_path, content_hash, enabled,
                created_at, updated_at, status, update_status, last_checked_at, last_check_error
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            params![
                skill.id,
                skill.name,
                skill.description,
                skill.source_type,
                skill.source_ref,
                skill.source_ref_resolved,
                skill.source_subpath,
                skill.source_branch,
                skill.source_revision,
                skill.remote_revision,
                skill.central_path,
                skill.content_hash,
                skill.enabled,
                skill.created_at,
                skill.updated_at,
                skill.status,
                skill.update_status,
                skill.last_checked_at,
                skill.last_check_error,
            ],
        )?;
        Ok(())
    }
```

to:

```rust
    pub fn insert_skill(&self, skill: &SkillRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO skills (
                id, name, description, source_type, source_ref, source_ref_resolved, source_subpath,
                source_branch, source_revision, remote_revision, central_path, content_hash, enabled,
                created_at, updated_at, status, update_status, last_checked_at, last_check_error,
                description_router
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                skill.id,
                skill.name,
                skill.description,
                skill.source_type,
                skill.source_ref,
                skill.source_ref_resolved,
                skill.source_subpath,
                skill.source_branch,
                skill.source_revision,
                skill.remote_revision,
                skill.central_path,
                skill.content_hash,
                skill.enabled,
                skill.created_at,
                skill.updated_at,
                skill.status,
                skill.update_status,
                skill.last_checked_at,
                skill.last_check_error,
                skill.description_router,
            ],
        )?;
        Ok(())
    }
```

- [ ] **Step 5: Update every `SkillRecord { ... }` struct literal**

Run: `grep -rn 'SkillRecord {' crates --include='*.rs'`

For each match, add `description_router: None,` to the literal. There are many sites in test code (at least in: `skill_store.rs` test fixtures, `router_render.rs`, `sync_engine/disclosure.rs`, `sync_engine/mod.rs`, and `crates/skills-manager-cli/tests/pd_wiring.rs`).

Example — the `insert_test_skill` fixture in `pack_tests` module (line ~2035):

```rust
    fn insert_test_skill(store: &SkillStore, id: &str, name: &str) -> SkillRecord {
        let rec = SkillRecord {
            id: id.to_string(),
            name: name.to_string(),
            description: None,
            source_type: "local".to_string(),
            source_ref: None,
            source_ref_resolved: None,
            source_subpath: None,
            source_branch: None,
            source_revision: None,
            remote_revision: None,
            central_path: format!("/tmp/skills/{id}"),
            content_hash: None,
            enabled: true,
            created_at: 1000,
            updated_at: 1000,
            status: "ok".to_string(),
            update_status: "unknown".to_string(),
            last_checked_at: None,
            last_check_error: None,
            description_router: None,
        };
        store.insert_skill(&rec).unwrap();
        rec
    }
```

Apply the same addition to **every** `SkillRecord { ... }` literal the grep surfaces.

- [ ] **Step 6: Write round-trip test**

Append to the `pack_tests` module:

```rust
    #[test]
    fn skill_record_round_trips_description_router() {
        let (store, _tmp) = test_store();
        insert_test_skill(&store, "s-x", "skill-x");
        // Directly write via SQL since set_skill_description_router arrives in Task 4.
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE skills SET description_router = ?1 WHERE id = 's-x'",
                ["Short router line"],
            ).unwrap();
        }
        let all = store.get_all_skills().unwrap();
        let sx = all.iter().find(|s| s.id == "s-x").expect("s-x missing");
        assert_eq!(sx.description_router.as_deref(), Some("Short router line"));
    }
```

- [ ] **Step 7: Build and run all core tests**

Run: `cargo build -p skills-manager-core`

Expected: clean build. Any compile error is a missed struct-literal site — find and fix.

Run: `cargo test -p skills-manager-core --lib --no-fail-fast`

Expected: all tests pass.

- [ ] **Step 8: Build CLI and workspace to catch external literal sites**

Run: `cargo build --workspace`

Expected: clean build. Any compile error points to a `SkillRecord { ... }` literal outside the core crate that needs `description_router: None,` added. The prime suspect is `crates/skills-manager-cli/tests/pd_wiring.rs`'s `mk_skill` closure. Update and rebuild until clean.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat(core): SkillRecord carries description_router"
```

---

## Task 4: Store setters for the new fields

**Files:**
- Modify: `crates/skills-manager-core/src/skill_store.rs`

- [ ] **Step 1: Write failing tests**

Append to the `pack_tests` module:

```rust
    #[test]
    fn set_pack_when_to_use_writes_and_clears() {
        let (store, _tmp) = test_store();
        store.insert_pack("p-a", "pack-a", None, None, None).unwrap();

        store.set_pack_when_to_use("p-a", Some("trigger text")).unwrap();
        let p = store.get_pack_by_id("p-a").unwrap().unwrap();
        assert_eq!(p.router_when_to_use.as_deref(), Some("trigger text"));

        store.set_pack_when_to_use("p-a", None).unwrap();
        let p = store.get_pack_by_id("p-a").unwrap().unwrap();
        assert_eq!(p.router_when_to_use, None);
    }

    #[test]
    fn set_pack_when_to_use_errors_when_missing() {
        let (store, _tmp) = test_store();
        assert!(store.set_pack_when_to_use("p-nope", Some("x")).is_err());
    }

    #[test]
    fn set_skill_description_router_writes_and_clears() {
        let (store, _tmp) = test_store();
        insert_test_skill(&store, "s-a", "skill-a");

        store.set_skill_description_router("s-a", Some("short")).unwrap();
        let s = store.get_skill("s-a").unwrap().unwrap();
        assert_eq!(s.description_router.as_deref(), Some("short"));

        store.set_skill_description_router("s-a", None).unwrap();
        let s = store.get_skill("s-a").unwrap().unwrap();
        assert_eq!(s.description_router, None);
    }

    #[test]
    fn set_skill_description_router_errors_when_missing() {
        let (store, _tmp) = test_store();
        assert!(store.set_skill_description_router("s-nope", Some("x")).is_err());
    }

    #[test]
    fn bulk_set_skill_description_router_atomic() {
        let (store, _tmp) = test_store();
        insert_test_skill(&store, "s-a", "skill-a");
        insert_test_skill(&store, "s-b", "skill-b");

        let updates = vec![
            ("skill-a".to_string(), Some("A short".to_string())),
            ("skill-b".to_string(), Some("B short".to_string())),
            ("skill-missing".to_string(), Some("ignored".to_string())),
        ];
        let report = store.bulk_set_skill_description_router(&updates).unwrap();
        assert_eq!(report.updated, 2);
        assert_eq!(report.skipped, 1);
        assert_eq!(
            store.get_skill("s-a").unwrap().unwrap().description_router.as_deref(),
            Some("A short")
        );
        assert_eq!(
            store.get_skill("s-b").unwrap().unwrap().description_router.as_deref(),
            Some("B short")
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p skills-manager-core --lib skill_store::pack_tests::set_pack_when_to_use_writes_and_clears skill_store::pack_tests::set_skill_description_router_writes_and_clears skill_store::pack_tests::bulk_set_skill_description_router_atomic --no-fail-fast`

Expected: FAIL — methods undefined.

- [ ] **Step 3: Implement `set_pack_when_to_use`**

Place after `set_pack_essential` (around line 1471):

```rust
    pub fn set_pack_when_to_use(&self, pack_id: &str, text: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE packs SET router_when_to_use = ?2, updated_at = ?3 WHERE id = ?1",
            params![pack_id, text, chrono::Utc::now().timestamp_millis()],
        )?;
        if n == 0 {
            anyhow::bail!("pack {pack_id} not found");
        }
        Ok(())
    }
```

- [ ] **Step 4: Implement `set_skill_description_router`**

Place near the other skill update methods (search for `update_skill_source_metadata` around line 294 and place in that vicinity):

```rust
    pub fn set_skill_description_router(
        &self,
        skill_id: &str,
        text: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE skills SET description_router = ?2, updated_at = ?3 WHERE id = ?1",
            params![skill_id, text, chrono::Utc::now().timestamp_millis()],
        )?;
        if n == 0 {
            anyhow::bail!("skill {skill_id} not found");
        }
        Ok(())
    }
```

- [ ] **Step 5: Implement `bulk_set_skill_description_router`**

Define the report struct near the top of `skill_store.rs` (after the existing structs, around line 130):

```rust
/// Result of a bulk description_router update.
#[derive(Debug, Default, Clone, Serialize)]
pub struct BulkRouterDescReport {
    pub updated: usize,
    pub skipped: usize,
}
```

Then add the method near `set_skill_description_router`:

```rust
    /// Bulk-update `description_router` for multiple skills keyed by name.
    /// Unknown skill names are counted in `skipped`; all writes run in one transaction.
    pub fn bulk_set_skill_description_router(
        &self,
        updates: &[(String, Option<String>)],
    ) -> Result<BulkRouterDescReport> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().timestamp_millis();
        let mut report = BulkRouterDescReport::default();
        for (name, text) in updates {
            let n = tx.execute(
                "UPDATE skills SET description_router = ?2, updated_at = ?3 WHERE name = ?1",
                params![name, text, now],
            )?;
            if n == 0 {
                report.skipped += 1;
            } else {
                report.updated += n;
            }
        }
        tx.commit()?;
        Ok(report)
    }
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p skills-manager-core --lib skill_store::pack_tests --no-fail-fast`

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add crates/skills-manager-core/src/skill_store.rs
git commit -m "feat(core): setters for router_when_to_use + description_router (single + bulk)"
```

---

## Task 5: Update `router_render` to use the new fields

**Files:**
- Modify: `crates/skills-manager-core/src/router_render.rs`

- [ ] **Step 1: Write failing tests**

Append to the `tests` module in `router_render.rs`:

```rust
    #[test]
    fn frontmatter_emits_when_to_use_when_set() {
        let mut p = pack("mkt", Some("Marketing domain"));
        p.router_when_to_use = Some("Use when user mentions SEO, CRO, PRD".into());
        let out = render_router_skill_md(&p, &[], &PathBuf::from("/v"));
        assert!(
            out.contains("when_to_use: Use when user mentions SEO, CRO, PRD"),
            "when_to_use missing; got:\n{out}"
        );
    }

    #[test]
    fn frontmatter_omits_when_to_use_when_none() {
        let p = pack("mkt", Some("Marketing domain"));
        let out = render_router_skill_md(&p, &[], &PathBuf::from("/v"));
        assert!(!out.contains("when_to_use:"), "when_to_use should not appear; got:\n{out}");
    }

    #[test]
    fn table_uses_description_router_when_set() {
        let p = pack("mkt", Some("desc"));
        let mut s = skill("seo-audit", "Full long original description with lots of words.");
        s.description_router = Some("Short router line".into());
        let out = render_router_skill_md(&p, &[s], &PathBuf::from("/v"));
        assert!(
            out.contains("| `seo-audit` | Short router line | `/v/seo-audit/SKILL.md` |"),
            "expected short router line; got:\n{out}"
        );
    }

    #[test]
    fn table_falls_back_to_description_when_router_desc_none() {
        let p = pack("mkt", Some("desc"));
        let s = skill("seo-audit", "First sentence. Second sentence.");
        let out = render_router_skill_md(&p, &[s], &PathBuf::from("/v"));
        assert!(
            out.contains("| `seo-audit` | First sentence | `/v/seo-audit/SKILL.md` |"),
            "expected fallback to first-sentence truncation; got:\n{out}"
        );
    }

    #[test]
    fn table_falls_back_when_router_desc_is_empty_string() {
        let p = pack("mkt", Some("desc"));
        let mut s = skill("x", "Original desc.");
        s.description_router = Some("   ".into()); // whitespace-only
        let out = render_router_skill_md(&p, &[s], &PathBuf::from("/v"));
        assert!(out.contains("| `x` | Original desc"));
    }

    #[test]
    fn yaml_escapes_quotes_in_when_to_use() {
        let mut p = pack("x", Some("d"));
        p.router_when_to_use = Some("Use when: \"quoted trigger\"".into());
        let out = render_router_skill_md(&p, &[], &PathBuf::from("/v"));
        assert!(
            out.contains("when_to_use: \"Use when: \\\"quoted trigger\\\"\""),
            "expected escaped quotes; got:\n{out}"
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p skills-manager-core --lib router_render::tests --no-fail-fast`

Expected: FAIL — render function doesn't emit `when_to_use` and uses `description` unconditionally.

- [ ] **Step 3: Update `render_router_skill_md` and `auto_render_body`**

Change `render_router_skill_md` to emit `when_to_use` when set:

```rust
pub fn render_router_skill_md(
    pack: &PackRecord,
    skills: &[SkillRecord],
    vault_root: &Path,
) -> String {
    let desc = pack
        .router_description
        .as_deref()
        .unwrap_or("Router for pack — description pending generation.");
    let body = pack
        .router_body
        .clone()
        .unwrap_or_else(|| auto_render_body(pack, skills, vault_root));

    let mut frontmatter = format!(
        "---\nname: pack-{}\ndescription: {}\n",
        pack.name,
        escape_yaml_scalar(desc),
    );
    if let Some(when) = pack.router_when_to_use.as_deref().filter(|s| !s.trim().is_empty()) {
        frontmatter.push_str(&format!("when_to_use: {}\n", escape_yaml_scalar(when)));
    }
    frontmatter.push_str("---\n\n");
    format!("{}{}\n", frontmatter, body)
}
```

Change the row-rendering in `auto_render_body` to prefer `description_router`:

```rust
fn auto_render_body(pack: &PackRecord, skills: &[SkillRecord], vault_root: &Path) -> String {
    let mut out = format!(
        "# Pack: {}\n\n\
        揀一個 skill，用 `Read` tool 讀對應 SKILL.md，跟住做。\n\n\
        | Skill | 用途 | 路徑 |\n|---|---|---|\n",
        pack.name,
    );
    for s in skills {
        let summary = skill_row_summary(s);
        out.push_str(&format!(
            "| `{}` | {} | `{}/{}/SKILL.md` |\n",
            s.name,
            summary,
            vault_root.display(),
            s.name,
        ));
    }
    out
}

/// Pick the per-row summary: prefer `description_router` when set and non-empty,
/// else fall back to first sentence of `description`, else empty.
fn skill_row_summary(s: &SkillRecord) -> String {
    if let Some(r) = s.description_router.as_deref().map(str::trim).filter(|r| !r.is_empty()) {
        return r.to_string();
    }
    s.description
        .as_deref()
        .unwrap_or("")
        .split_terminator(['.', '。'])
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p skills-manager-core --lib router_render:: --no-fail-fast`

Expected: all pass (existing + new).

- [ ] **Step 5: Commit**

```bash
git add crates/skills-manager-core/src/router_render.rs
git commit -m "feat(core): router_render emits when_to_use + uses description_router"
```

---

## Task 6: Extend CLI `pack set-router` with `--when-to-use`

**Files:**
- Modify: `crates/skills-manager-cli/src/main.rs`
- Modify: `crates/skills-manager-cli/src/commands.rs`

- [ ] **Step 1: Update clap enum in `main.rs`**

Find `PackAction::SetRouter` (search for `SetRouter` in `main.rs`). It currently has `description: Option<String>` and `body: Option<PathBuf>`. Add two new fields:

```rust
    /// Set or update a pack's router description/body
    SetRouter {
        /// Pack name
        name: String,
        /// New router description (single-line summary)
        #[arg(long)]
        description: Option<String>,
        /// Path to a file whose contents become the router body
        #[arg(long)]
        body: Option<std::path::PathBuf>,
        /// Trigger / when-to-use text (native Claude Code frontmatter field)
        #[arg(long = "when-to-use")]
        when_to_use: Option<String>,
        /// Clear the when-to-use field (set to NULL)
        #[arg(long = "clear-when-to-use")]
        clear_when_to_use: bool,
    },
```

In the `match cli.command { ... }` arm for `Commands::Pack` -> `PackAction::SetRouter`, change the dispatch to pass the new args:

```rust
            PackAction::SetRouter {
                name,
                description,
                body,
                when_to_use,
                clear_when_to_use,
            } => commands::cmd_pack_set_router(
                &name,
                description.as_deref(),
                body.as_deref(),
                when_to_use.as_deref(),
                clear_when_to_use,
            ),
```

- [ ] **Step 2: Update `cmd_pack_set_router` in `commands.rs`**

Find the function (around line 330). Replace its body with:

```rust
pub fn cmd_pack_set_router(
    name: &str,
    description: Option<&str>,
    body_file: Option<&std::path::Path>,
    when_to_use: Option<&str>,
    clear_when_to_use: bool,
) -> Result<()> {
    if description.is_none()
        && body_file.is_none()
        && when_to_use.is_none()
        && !clear_when_to_use
    {
        anyhow::bail!(
            "set-router requires at least one of --description, --body, --when-to-use, --clear-when-to-use"
        );
    }
    if when_to_use.is_some() && clear_when_to_use {
        anyhow::bail!("cannot pass both --when-to-use and --clear-when-to-use");
    }
    let store = open_store()?;
    let pack = find_pack_by_name(&store, name)?;
    let ts = chrono::Utc::now().timestamp();

    // description / body path (existing semantics)
    if description.is_some() || body_file.is_some() {
        let body = body_file.map(std::fs::read_to_string).transpose()?;
        store.set_pack_router(&pack.id, description, body.as_deref(), ts)?;
    }

    // when-to-use path (new)
    if let Some(w) = when_to_use {
        store.set_pack_when_to_use(&pack.id, Some(w))?;
    } else if clear_when_to_use {
        store.set_pack_when_to_use(&pack.id, None)?;
    }

    println!("Router updated for pack '{}'.", pack.name);
    Ok(())
}
```

- [ ] **Step 3: Build and smoke test**

Run: `cargo build -p skills-manager-cli && ./target/debug/sm pack set-router --help`

Expected: help output includes `--when-to-use <WHEN_TO_USE>` and `--clear-when-to-use`.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace --no-fail-fast`

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/skills-manager-cli/src/main.rs crates/skills-manager-cli/src/commands.rs
git commit -m "feat(cli): pack set-router --when-to-use + --clear-when-to-use"
```

---

## Task 7: Add CLI `sm skill set-router-desc` + `sm skill import-router-descs`

**Files:**
- Modify: `crates/skills-manager-cli/Cargo.toml`
- Modify: `crates/skills-manager-cli/src/main.rs`
- Modify: `crates/skills-manager-cli/src/commands.rs`

- [ ] **Step 1: Add `serde_yaml` dependency**

In `crates/skills-manager-cli/Cargo.toml`, add to `[dependencies]` (check if already present — if so, skip):

```toml
serde_yaml = "0.9"
```

- [ ] **Step 2: Add `Commands::Skill { action: SkillAction }` to `main.rs`**

In the `Commands` enum (near `Pack { ... }`), add:

```rust
    /// Manage individual skills (router description, bulk import)
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
```

After the `PackAction` enum (or `AgentAction`), add a new enum:

```rust
#[derive(Subcommand)]
enum SkillAction {
    /// Set or clear a skill's router description (L2 per-skill line)
    SetRouterDesc {
        /// Skill name
        name: String,
        /// Router description text (the L2 short line)
        #[arg(long)]
        description: Option<String>,
        /// Clear the router description (set to NULL)
        #[arg(long = "clear")]
        clear: bool,
    },
    /// Bulk-import router descriptions from a YAML file
    /// (top-level keys are skill names, values are strings or null)
    ImportRouterDescs {
        /// Path to YAML file
        file: std::path::PathBuf,
    },
}
```

In the `main()` match block, add dispatch:

```rust
        Commands::Skill { action } => match action {
            SkillAction::SetRouterDesc { name, description, clear } => {
                commands::cmd_skill_set_router_desc(&name, description.as_deref(), clear)
            }
            SkillAction::ImportRouterDescs { file } => {
                commands::cmd_skill_import_router_descs(&file)
            }
        },
```

- [ ] **Step 3: Add handlers in `commands.rs`**

Append near the existing pack helpers (e.g. after `cmd_pack_set_essential` added in Task 8 of the previous feature):

```rust
pub fn cmd_skill_set_router_desc(
    name: &str,
    description: Option<&str>,
    clear: bool,
) -> Result<()> {
    if description.is_none() && !clear {
        anyhow::bail!("skill set-router-desc requires --description <text> or --clear");
    }
    if description.is_some() && clear {
        anyhow::bail!("cannot pass both --description and --clear");
    }
    let store = open_store()?;
    let skill = store
        .get_skill_by_name(name)?
        .ok_or_else(|| anyhow::anyhow!("skill '{}' not found", name))?;
    let text = if clear { None } else { description };
    store.set_skill_description_router(&skill.id, text)?;
    match text {
        Some(_) => println!("Router description set for skill '{}'.", skill.name),
        None => println!("Router description cleared for skill '{}'.", skill.name),
    }
    Ok(())
}

pub fn cmd_skill_import_router_descs(path: &std::path::Path) -> Result<()> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let parsed: std::collections::BTreeMap<String, Option<String>> = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse YAML at {}", path.display()))?;
    if parsed.is_empty() {
        anyhow::bail!("no entries in {}", path.display());
    }
    let updates: Vec<(String, Option<String>)> = parsed.into_iter().collect();
    let store = open_store()?;
    let report = store.bulk_set_skill_description_router(&updates)?;
    println!(
        "Updated {} skill(s), skipped {} unknown name(s).",
        report.updated, report.skipped,
    );
    Ok(())
}
```

**Important**: `get_skill_by_name` may not exist. Verify with `grep -n 'fn get_skill_by_name\|fn get_skill\b' crates/skills-manager-core/src/skill_store.rs`. If absent, use the existing lookup pattern: `store.get_all_skills()?.into_iter().find(|s| s.name == name)`. Prefer that fallback rather than adding a new store method inside this task.

- [ ] **Step 4: Build and smoke test**

Run: `cargo build -p skills-manager-cli`

Expected: clean build.

Run: `./target/debug/sm skill --help && ./target/debug/sm skill set-router-desc --help && ./target/debug/sm skill import-router-descs --help`

Expected: all three help outputs render cleanly.

- [ ] **Step 5: Run workspace tests**

Run: `cargo test --workspace --no-fail-fast`

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/skills-manager-cli/Cargo.toml crates/skills-manager-cli/src/main.rs crates/skills-manager-cli/src/commands.rs
git commit -m "feat(cli): skill set-router-desc + import-router-descs (YAML bulk)"
```

---

## Task 8: CLI integration tests

**Files:**
- Modify: `crates/skills-manager-cli/tests/pd_wiring.rs` (extend) OR create `crates/skills-manager-cli/tests/three_tier.rs`

Choose the extension path — append to `pd_wiring.rs` to reuse `seed_test_state` and `run_sm`.

- [ ] **Step 1: Write integration tests**

Append to `crates/skills-manager-cli/tests/pd_wiring.rs`:

```rust
#[test]
fn pack_set_router_stores_when_to_use() {
    let tmp = tempfile::tempdir().unwrap();
    seed_test_state(tmp.path());

    let (ok, out, err) = run_sm(
        tmp.path(),
        &[
            "pack", "set-router", "marketing",
            "--description", "Marketing domain",
            "--when-to-use", "Trigger when user mentions SEO / CRO / PRD",
        ],
    );
    assert!(ok, "set-router failed: {err}\n{out}");

    // Switch to hybrid + sync to observe rendered router SKILL.md
    run_sm(tmp.path(), &["scenario", "set-mode", "test-scenario", "hybrid"]);
    let (ok, _, err) = run_sm(tmp.path(), &["switch", "claude_code", "test-scenario"]);
    assert!(ok, "switch failed: {err}");

    let router_md = tmp.path().join(".claude/skills/pack-marketing/SKILL.md");
    let content = std::fs::read_to_string(&router_md).unwrap();
    assert!(content.contains("description: Marketing domain"), "got:\n{content}");
    assert!(
        content.contains("when_to_use: Trigger when user mentions SEO / CRO / PRD"),
        "when_to_use missing; got:\n{content}"
    );
}

#[test]
fn skill_set_router_desc_shows_in_router_body() {
    let tmp = tempfile::tempdir().unwrap();
    seed_test_state(tmp.path());

    // Set a short router line for the marketing-pack skill.
    let (ok, _, err) = run_sm(
        tmp.path(),
        &["skill", "set-router-desc", "mkt-skill", "--description", "Pick for marketing work"],
    );
    assert!(ok, "set-router-desc failed: {err}");

    // Switch hybrid + sync.
    run_sm(tmp.path(), &["scenario", "set-mode", "test-scenario", "hybrid"]);
    run_sm(tmp.path(), &["switch", "claude_code", "test-scenario"]);

    let router_md = tmp.path().join(".claude/skills/pack-marketing/SKILL.md");
    let content = std::fs::read_to_string(&router_md).unwrap();
    assert!(
        content.contains("| `mkt-skill` | Pick for marketing work |"),
        "row should use new router line; got:\n{content}"
    );
}

#[test]
fn import_router_descs_yaml_bulk_updates() {
    let tmp = tempfile::tempdir().unwrap();
    seed_test_state(tmp.path());

    let yaml_path = tmp.path().join("l2.yaml");
    std::fs::write(
        &yaml_path,
        "mkt-skill: \"bulk-set line\"\nbase-skill: \"base line\"\nnon-existent: \"ignored\"\n",
    ).unwrap();

    let (ok, out, err) = run_sm(
        tmp.path(),
        &["skill", "import-router-descs", yaml_path.to_str().unwrap()],
    );
    assert!(ok, "import failed: {err}\n{out}");
    assert!(out.contains("Updated 2 skill(s)"), "got: {out}");
    assert!(out.contains("skipped 1"), "got: {out}");
}

#[test]
fn rendered_router_falls_back_when_description_router_unset() {
    let tmp = tempfile::tempdir().unwrap();
    seed_test_state(tmp.path());

    // Don't set any description_router — switch to hybrid and render.
    run_sm(tmp.path(), &["scenario", "set-mode", "test-scenario", "hybrid"]);
    run_sm(tmp.path(), &["switch", "claude_code", "test-scenario"]);

    let router_md = tmp.path().join(".claude/skills/pack-marketing/SKILL.md");
    let content = std::fs::read_to_string(&router_md).unwrap();
    // seed_test_state's mkt-skill inserts with description=None, so first-sentence
    // fallback yields empty — the row still renders.
    assert!(content.contains("| `mkt-skill` |"), "row missing; got:\n{content}");
}
```

**Note**: `seed_test_state` inserts `mk_skill` with `description: None`. That makes the fallback test's assertion thin (empty summary). If the SkillRecord construction in `seed_test_state` ends up setting `description`, tighten the fallback assertion to check the first-sentence truncation.

- [ ] **Step 2: Run the new integration tests**

Run: `cargo test -p skills-manager-cli --test pd_wiring pack_set_router_stores_when_to_use skill_set_router_desc_shows_in_router_body import_router_descs_yaml_bulk_updates rendered_router_falls_back_when_description_router_unset --no-fail-fast`

Expected: all four pass.

- [ ] **Step 3: Run full workspace tests**

Run: `cargo test --workspace --no-fail-fast`

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add crates/skills-manager-cli/tests/pd_wiring.rs
git commit -m "test(cli): integration tests for three-tier PD (L1 when_to_use + L2 bulk import)"
```

---

## Task 9: Manual e2e acceptance + PROGRESS update

**Files:** None modified as part of the acceptance. Updates `PROGRESS.md` at the end.

- [ ] **Step 1: Build fresh**

Run: `cargo build -p skills-manager-cli && cargo build --manifest-path src-tauri/Cargo.toml`

Expected: clean.

- [ ] **Step 2: Write `when_to_use` for the marketing pack on the real DB**

Run:

```bash
./target/debug/sm pack set-router marketing \
  --when-to-use 'Use when user mentions "SEO", "CRO", "PRD", "marketing copy", "launch", "internal comms", "documentation".'
```

Expected: `Router updated for pack 'marketing'.`

Verify via:

```bash
sqlite3 ~/.skills-manager/skills-manager.db "SELECT name, router_when_to_use FROM packs WHERE name='marketing';"
```

- [ ] **Step 3: Write one `description_router` on a real skill**

Run:

```bash
./target/debug/sm skill set-router-desc prd --description "Single-shot PRD authoring — exec summary, user stories, risks."
```

Verify:

```bash
sqlite3 ~/.skills-manager/skills-manager.db "SELECT name, description_router FROM skills WHERE name='prd';"
```

- [ ] **Step 4: Resync to materialize the router**

Run:

```bash
./target/debug/sm scenario set-mode standard-marketing hybrid
./target/debug/sm switch claude_code standard-marketing
cat ~/.claude/skills/pack-marketing/SKILL.md
```

Expected:
- Frontmatter contains `description: ...` AND `when_to_use: Use when user mentions "SEO", "CRO", "PRD", ...`
- Table row for `prd` shows `Single-shot PRD authoring — exec summary, user stories, risks.` (not the original 120-char vendor description)
- Rows for skills without `description_router` set still appear using first-sentence fallback

- [ ] **Step 5: Bulk import smoke test**

Write a tiny test YAML:

```bash
cat > /tmp/l2-smoke.yaml <<'EOF'
marketing: "Marketing skills router — CRO / SEO / copy / ads / email / pricing / launch."
internal-comms: "Internal comms templates — status reports / leadership updates / FAQs / incidents."
documentation-writer: "Diátaxis-style technical docs — tutorial / how-to / reference / explanation."
not-a-real-skill: "ignored"
EOF

./target/debug/sm skill import-router-descs /tmp/l2-smoke.yaml
```

Expected: `Updated 3 skill(s), skipped 1 unknown name(s).`

Verify all three skills now have `description_router` populated via `sqlite3`.

- [ ] **Step 6: Restore the active scenario**

Run:

```bash
./target/debug/sm switch claude_code everything
```

Expected: switch reports success; `ls ~/.claude/skills/ | grep '^pack-' | wc -l` returns 0 (all routers removed).

- [ ] **Step 7: Update PROGRESS.md**

Edit `PROGRESS.md`. Under "Current Iteration", add a new completed entry:

```markdown
### Three-Tier Progressive Disclosure ✅
**Status:** Complete (PR pending) **Date:** 2026-04-20
**Goal:** Split PD into three storage tiers so routers carry authored per-skill differentiation and Claude Code's native `when_to_use` frontmatter field is populated.
**Changes:** DB v11 (two new nullable columns), router_render emits `when_to_use` + prefers `description_router`, CLI `pack set-router --when-to-use` + `sm skill set-router-desc` + `sm skill import-router-descs` (YAML bulk), 4 new CLI integration tests.
**Verified:** Manual e2e — set `when_to_use` on marketing pack + `description_router` on `prd`, synced hybrid-mode scenario, observed rendered SKILL.md contains both fields and the custom L2 row text. Bulk YAML import updates + skips correctly.
```

Commit:

```bash
git add PROGRESS.md
git commit -m "docs: three-tier PD complete"
```

---

## Self-Review

**Spec coverage** — every Goal in the spec has at least one task:

| Spec Goal | Task |
|---|---|
| L1/L2/L3 tier separation (storage + rendering) | Tasks 1–5 |
| L2 authoring data persists and survives re-scans | Tasks 3, 4 |
| L1 emits `when_to_use` frontmatter | Task 5 |
| Fallback: L2 unset → use `description` | Task 5 (plus Task 8 integration test) |
| CLI single-skill edit | Task 7 (`cmd_skill_set_router_desc`) |
| CLI bulk YAML import | Task 7 (`cmd_skill_import_router_descs`) |
| Backward-compat (full mode identical, hybrid with no L2 still works) | Task 5 fallback + Task 8 fallback test + Task 9 manual e2e |
| DB migration v11, nullable columns | Task 1 |
| Transaction-safe bulk import | Task 4 (`bulk_set_skill_description_router`) |

**Placeholder scan**: No "TBD", "TODO", "fill in details". Two notes ask the engineer to verify method existence (`get_skill_by_name`) or struct-field names via grep before editing; these are legitimate context-checks given the codebase may evolve between plan-write and execution.

**Type consistency**:
- `router_when_to_use: Option<String>` — used in spec + struct + setter + renderer + CLI + tests
- `description_router: Option<String>` — same
- `BulkRouterDescReport { updated: usize, skipped: usize }` — defined in Task 4, used in Task 7
- Empty-string-is-unset rule: `router_render` (Task 5) + CLI tests (Task 8) both check this

**Decomposition**: 9 focused tasks. Task 3 is the largest (struct field propagates across many SELECT queries + struct literals) — engineer is warned to iterate build-fixes until workspace compiles clean. Tasks 1, 4, 5, 7, 8 are straightforward TDD. Task 9 is acceptance only.
