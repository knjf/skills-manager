# Phase 1: Core Crate Extraction + Skill Packs

## Overview

Phase 1 splits into two sequential sub-phases:

- **1A — Core Crate Extraction**: Extract 17 modules from `src-tauri/src/core/` into a standalone `skills-manager-core` library crate. Pure refactor, zero functionality changes.
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
│           └── tool_adapters.rs
├── src-tauri/
│   ├── Cargo.toml                 # depends on skills-manager-core
│   └── src/
│       ├── core/
│       │   ├── mod.rs             # re-export bridge
│       │   ├── file_watcher.rs    # stays (tauri::AppHandle dependency)
│       │   └── install_cancel.rs  # stays (async coordination)
│       ├── commands/              # unchanged
│       ├── lib.rs
│       └── main.rs
```

### Module Classification

**Move to core crate (17 modules):**

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

**Stay in Tauri app (2 modules):**

| Module | Reason |
|--------|--------|
| `file_watcher.rs` | Uses `tauri::AppHandle` for event emission |
| `install_cancel.rs` | Async cancellation tied to Tauri task lifecycle |

### Re-export Bridge

`src-tauri/src/core/mod.rs` becomes:

```rust
pub use skills_manager_core::*;

// Tauri-dependent modules stay local
pub mod file_watcher;
pub mod install_cancel;
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

### Internal Import Changes

Files moving to the core crate need import path updates:
- `use super::crypto` → `use crate::crypto`
- `use super::migrations` → `use crate::migrations`
- Other `super::` and `crate::core::` references → `crate::` (since modules are now at crate root)

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

### Changes to `commands/scenarios.rs`

`sync_scenario_skills()` and `unsync_scenario_skills()` updated to use effective skill resolution (packs + direct skills) instead of just `scenario_skills`.

### Default Pack Definitions

On first run after migration, seed the following packs from existing scenario data:

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

Seeding is best-effort based on existing skill names. Users can adjust after migration.

### Backward Compatibility

- Existing `scenario_skills` rows remain valid and contribute to effective skill lists
- Scenarios without any packs work exactly as before
- No data loss on migration — packs are additive tables

### Verification Criteria

Phase 1B is complete when:
1. Migration v4→v5 runs cleanly on existing database
2. Pack CRUD operations work (create, assign skills, assign to scenarios)
3. Effective skill resolution returns correct union of pack + direct skills
4. Scenario switching still syncs all effective skills correctly
5. Existing scenarios without packs continue to work unchanged
6. Default packs are seeded on first run
7. All new functions have unit tests

---

## Out of Scope

These are explicitly deferred to later phases:
- CLI binary (Phase 2)
- Plugin management (Phase 3)
- Packs UI / frontend changes (Phase 4)
- Matrix view (Phase 5)
- Semantic reorganization of core crate internals (can be done anytime, not blocking)
