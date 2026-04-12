# Phase 1: Core Crate Extraction + Skill Packs

## Overview

Phase 1 splits into two sequential sub-phases:

- **1A — Core Crate Extraction**: Extract 18 modules from `src-tauri/src/core/` into a standalone `skills-manager-core` library crate. Refactor with feature-gated Tauri/Tokio deps in error.rs, plus CI/tooling updates.
- **1B — Skill Packs**: Add packs schema, CRUD, and pack-based scenario composition on top of the extracted crate.

1A must complete (all tests pass, app runs) before 1B begins.

---

## Phase 1A: Core Crate Extraction

### Goal

Make the core business logic reusable outside Tauri (for CLI in Phase 2, and potentially other consumers) by extracting it into a standalone Rust library crate within a Cargo workspace.

### Workspace Structure

```
skills-manager/                    # workspace root
├── Cargo.toml                     # [workspace] members
├── crates/
│   └── skills-manager-core/
│       ├── Cargo.toml             # library crate
│       └── src/
│           ├── lib.rs             # pub mod declarations + re-exports
│           ├── central_repo.rs
│           ├── content_hash.rs
│           ├── crypto.rs
│           ├── error.rs
│           ├── git_backup.rs
│           ├── git_fetcher.rs
│           ├── installer.rs
│           ├── migrations.rs
│           ├── project_scanner.rs
│           ├── scanner.rs
│           ├── skill_metadata.rs
│           ├── skill_store.rs
│           ├── skillsmp_api.rs
│           ├── skillssh_api.rs
│           ├── sync_engine.rs
│           ├── tool_adapters.rs
│           └── install_cancel.rs
├── src-tauri/
│   ├── Cargo.toml                 # depends on skills-manager-core
│   └── src/
│       ├── core/
│       │   ├── mod.rs             # re-export bridge
│       │   └── file_watcher.rs    # stays (tauri::AppHandle dependency)
│       ├── commands/              # unchanged
│       ├── lib.rs
│       └── main.rs
```

### Module Classification

**Move to core crate (18 modules):**

| Module | Responsibility | Internal Dependencies |
|--------|---------------|----------------------|
| `central_repo.rs` | Path management (~/.skills-manager/) | `dirs` |
| `content_hash.rs` | SHA256 directory hashing | `sha2`, `walkdir` |
| `crypto.rs` | AES-256-GCM encrypt/decrypt | `aes-gcm`, `rand` |
| `error.rs` | AppError type + ErrorKind enum | none |
| `git_backup.rs` | Git operations on skills dir | `git2`, `central_repo` |
| `git_fetcher.rs` | Clone/pull git repos | `git2`, `central_repo` |
| `installer.rs` | Install from local/git sources | `central_repo`, `content_hash`, `skill_metadata` |
| `migrations.rs` | SQLite schema versioning | `rusqlite` |
| `project_scanner.rs` | Scan projects for skills | `skill_metadata` |
| `scanner.rs` | Discover skills in tool dirs | `skill_metadata`, `tool_adapters` |
| `skill_metadata.rs` | Parse SKILL.md frontmatter | `serde_yaml`, `regex` |
| `skill_store.rs` | All SQLite CRUD operations | `crypto`, `migrations`, `rusqlite` |
| `skillsmp_api.rs` | SkillsMasterPlan API client | `reqwest`, `serde` |
| `skillssh_api.rs` | SkillsShelf API client | `reqwest`, `serde` |
| `sync_engine.rs` | Symlink/copy sync logic | none (standalone) |
| `tool_adapters.rs` | Agent directory config | `skill_store` (read-only) |
| `install_cancel.rs` | Async cancellation registry | `tokio` (no Tauri dependency) |

**Stay in Tauri app (1 module):**

| Module | Reason |
|--------|--------|
| `file_watcher.rs` | Uses `tauri::AppHandle` for event emission |

### Re-export Bridge

`src-tauri/src/core/mod.rs` becomes:

```rust
pub use skills_manager_core::*;

// Tauri-dependent module stays local
pub mod file_watcher;
```

This means all `commands/*.rs` files keep their existing `use crate::core::` imports unchanged.

### Dependency Split

**Core crate Cargo.toml dependencies:**
- `rusqlite` (bundled), `anyhow`, `serde`, `serde_json`, `serde_yaml`
- `chrono`, `uuid`, `regex`, `semver`, `hex`
- `sha2`, `aes-gcm`, `rand` (crypto)
- `git2` (vendored-openssl)
- `reqwest` (blocking, json), `urlencoding`
- `walkdir`, `tempfile`, `zip`, `image`
- `dirs`, `log`

**Tauri app Cargo.toml changes:**
- Add: `skills-manager-core = { path = "../crates/skills-manager-core" }`
- Remove: dependencies that moved to core (avoid duplication)
- Keep: `tauri`, `tauri-plugin-*`, `tokio`, `notify`

### `error.rs` — Tauri/Tokio Dependency

`error.rs` has `impl From<tauri::Error>` and `impl From<tokio::task::JoinError>`. These are gated behind optional features:

```toml
# crates/skills-manager-core/Cargo.toml
[features]
default = []
tauri = ["dep:tauri"]
tokio = ["dep:tokio"]
```

```rust
// error.rs
#[cfg(feature = "tokio")]
impl From<tokio::task::JoinError> for AppError { ... }

#[cfg(feature = "tauri")]
impl From<tauri::Error> for AppError { ... }
```

Tauri app enables both features. CLI enables only `tokio` (if needed). Core crate stays framework-agnostic by default.

### Internal Import Changes

Files moving to the core crate need import path updates:
- `use super::crypto` → `use crate::crypto`
- `use super::migrations` → `use crate::migrations`
- Other `super::` and `crate::core::` references → `crate::` (since modules are now at crate root)

### CI/Tooling Updates

Workspace restructure requires updating:
- `package.json` scripts — Tauri CLI paths
- `.github/workflows/release.yml` — manifest path, cache keys
- Cargo lockfile placement (workspace root)

### Verification Criteria

Phase 1A is complete when:
1. `cargo build -p skills-manager-core` succeeds — core crate compiles independently
2. `cargo test` passes — all existing tests (in both crates) pass
3. `cargo tauri dev` works — app starts, scenario switching functions correctly
4. Diff contains only file moves and import path changes — zero logic modifications

---

## Phase 1B: Skill Packs

### Goal

Introduce "packs" as an organizational layer between individual skills and scenarios. A pack is a named group of related skills. A scenario becomes a composition of packs (plus optional individual skills for backward compatibility).

### Database Schema (Migration v4 → v5)

```sql
-- Skill packs: named groups of related skills
CREATE TABLE packs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    icon TEXT,
    color TEXT,
    sort_order INTEGER DEFAULT 0,
    created_at INTEGER,
    updated_at INTEGER
);

-- Which skills belong to which pack
CREATE TABLE pack_skills (
    pack_id TEXT NOT NULL REFERENCES packs(id) ON DELETE CASCADE,
    skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    sort_order INTEGER DEFAULT 0,
    PRIMARY KEY(pack_id, skill_id)
);

-- Which packs are in which scenario
CREATE TABLE scenario_packs (
    scenario_id TEXT NOT NULL REFERENCES scenarios(id) ON DELETE CASCADE,
    pack_id TEXT NOT NULL REFERENCES packs(id) ON DELETE CASCADE,
    sort_order INTEGER DEFAULT 0,
    PRIMARY KEY(scenario_id, pack_id)
);
```

### Pack Composition Logic

A scenario's effective skill list is computed as:

```
effective_skills(scenario) = 
    UNION(skills in each pack assigned to scenario)
    UNION(individual skills in scenario_skills table)
```

The `scenario_skills` table continues to work for backward compatibility. Existing scenarios with direct skill assignments keep functioning. Packs are purely additive.

### Additions to `skill_store.rs`

All pack persistence lives in `skill_store.rs`, consistent with the existing pattern (all SQLite operations in one place):
- `insert_pack()`, `get_all_packs()`, `get_pack_by_id()`, `update_pack()`, `delete_pack()`
- `add_skill_to_pack()`, `remove_skill_from_pack()`, `get_skills_for_pack()`
- `add_pack_to_scenario()`, `remove_pack_from_scenario()`, `get_packs_for_scenario()`
- `reorder_packs()`, `reorder_pack_skills()`, `reorder_scenario_packs()`
- `get_effective_skills_for_scenario()` — returns deduplicated union of pack skills + direct scenario_skills

### New Tauri Command Module: `commands/packs.rs`

Thin IPC wrappers around `skill_store` pack functions. Registered in `lib.rs`.

### Effective Skill Ordering

A scenario's effective skill list is ordered as:
1. Packs in `scenario_packs.sort_order` — within each pack, skills in `pack_skills.sort_order`
2. Direct skills from `scenario_skills.sort_order` appended after all pack skills
3. Duplicates removed (first occurrence wins)

This is a single SQL query using `UNION ALL` with explicit ordering, not N+1:

```sql
SELECT DISTINCT s.* FROM (
    SELECT s.*, sp.sort_order * 10000 + ps.sort_order AS effective_order
    FROM skills s
    JOIN pack_skills ps ON ps.skill_id = s.id
    JOIN scenario_packs sp ON sp.pack_id = ps.pack_id
    WHERE sp.scenario_id = ?
    UNION ALL
    SELECT s.*, 99999000 + ss.sort_order AS effective_order
    FROM skills s
    JOIN scenario_skills ss ON ss.skill_id = s.id
    WHERE ss.scenario_id = ?
) s
ORDER BY effective_order
```

### Changes to `commands/scenarios.rs`

- `sync_scenario_skills()` and `unsync_scenario_skills()` updated to use `get_effective_skills_for_scenario()` instead of just `scenario_skills`
- `remove_skill_from_scenario()` — after removing the direct row, check if the skill is still in the effective list via packs. If yes, do NOT unsync.

### Default Pack Definitions

Migration v4→v5 seeds both packs AND scenario_packs:
1. Create pack definitions with skills assigned via `pack_skills`
2. Assign `scenario_packs` based on existing `scenario_skills` data — if a scenario contains most skills from a pack, assign that pack and remove the matching direct `scenario_skills` rows
3. Remaining unmatched skills stay as direct `scenario_skills`

This ensures upgrade is behavior-preserving: the effective skill list for each scenario is identical before and after migration.

| Pack | Description |
|------|-------------|
| `base` | Core utility skills (skill-retrieval, web-access, etc.) |
| `gstack` | Full gstack development workflow |
| `agent-orchestration` | Paseo, Paperclip agent coordination |
| `browser-tools` | Agent-browser, opencli, x-tweet-fetcher |
| `research` | Deep research and content discovery |
| `design` | Stitch, frontend-design, shadcn-ui |
| `knowledge` | Obsidian, NotebookLM, Readwise |
| `marketing` | Full marketing suite |
| `ops` | claude-code-router, system utilities |

### Backward Compatibility

- Existing `scenario_skills` rows remain valid and contribute to effective skill lists
- Scenarios without any packs work exactly as before
- No data loss on migration — packs are additive tables
- Effective skill list is identical before and after migration (verified by seeding logic)

### Verification Criteria

Phase 1B is complete when:
1. Migration v4→v5 runs cleanly on existing database
2. Pack CRUD operations work (create, assign skills, assign to scenarios)
3. Effective skill resolution returns correct ordered, deduplicated union
4. Scenario switching still syncs all effective skills correctly
5. Existing scenarios without packs continue to work unchanged
6. Default packs are seeded on first run with scenario_packs assigned
7. Effective skill list is identical before and after migration
8. remove_skill_from_scenario does NOT unsync pack-inherited skills
9. Orphaned skill_ids in pack_skills (deleted skills) are handled gracefully
10. All new functions have unit tests (see test plan below)

### Required Tests (Phase 1B)

Migration tests:
- Fresh DB creates packs, pack_skills, scenario_packs tables
- v4→v5 migration on existing DB with data
- Foreign key cascade: delete pack → pack_skills rows removed

Pack CRUD tests:
- insert_pack / get_all_packs / get_pack_by_id / update_pack / delete_pack
- add_skill_to_pack / remove_skill_from_pack / get_skills_for_pack
- add_pack_to_scenario / remove_pack_from_scenario / get_packs_for_scenario
- reorder_packs / reorder_pack_skills / reorder_scenario_packs

Effective skill resolution tests:
- Scenario with only packs → returns pack skills in order
- Scenario with only direct skills → backward compat
- Scenario with packs + direct skills → ordered union, packs first
- Duplicate skill in two packs → deduplicated (first wins)
- Scenario with no packs and no direct skills → empty
- Orphaned skill_id in pack_skills → gracefully excluded

Scenario sync tests:
- remove_skill_from_scenario: skill still in pack → NOT unsynced
- remove_skill_from_scenario: skill NOT in pack → unsynced
- sync_scenario_skills with packs → all effective skills synced

---

## Out of Scope

These are explicitly deferred to later phases:
- CLI binary (Phase 2)
- Plugin management (Phase 3)
- Packs UI / frontend changes (Phase 4)
- Matrix view (Phase 5)
- Semantic reorganization of core crate internals (can be done anytime, not blocking)
