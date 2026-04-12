# Skills Manager Fork — Development Progress

## Overall Status

```
Phase 1 ✅ → Phase 2 📝 → Phase 3 📝 → Phase 4 📝 → Phase 5 📝
```

✅ = merged to main | 📝 = PR open for review

---

## Phase 1: Core Crate Extraction + Skill Packs ✅

**Completed:** 2026-04-13
**PR:** knjf/skills-manager#1 (merged)
**Branch:** phase1/core-extraction-packs → main

### What was done
- Extracted 18 core modules into `crates/skills-manager-core/` library crate
- Created Cargo workspace (core crate + Tauri app)
- Feature-gated `error.rs` tauri/tokio deps (`tauri-compat`, `tokio-compat`)
- Added Skill Packs: DB migration v4→v5, pack CRUD, effective skill resolution
- Pack-aware scenario sync (`remove_skill_from_scenario` checks pack membership)
- 12 new Tauri IPC commands for pack management
- 146 tests pass, core builds standalone

### Deferred items
- Default pack seeding (needs UI or CLI to be useful)
- 3 reorder methods (reorder_packs, reorder_pack_skills, reorder_scenario_packs) — needed when Phase 4 UI is built

### Key files
- `crates/skills-manager-core/src/skill_store.rs` — PackRecord, pack CRUD, effective skill resolution
- `crates/skills-manager-core/src/migrations.rs` — v4→v5, PACKS_SCHEMA_DDL constant
- `src-tauri/src/commands/packs.rs` — 12 Tauri IPC wrappers
- `docs/superpowers/specs/2026-04-13-phase1-core-extraction-packs-design.md` — design spec
- `docs/superpowers/plans/2026-04-13-phase1-core-extraction-packs.md` — implementation plan

---

## Phase 2: CLI Binary 📝

**Completed:** 2026-04-13
**PR:** knjf/skills-manager#3 (open for review)
**Branch:** phase2/cli-binary

### What was done
- Rust CLI binary `sm` using `clap` derive API at `crates/skills-manager-cli/`
- Commands: list, current, switch, skills, diff, packs, pack add/remove
- Uses `get_effective_skills_for_scenario()` for pack-aware operations
- Sync logic matches shell script behavior (symlink/copy per agent)
- Install: `cp target/release/sm ~/.local/bin/sm`

### Known issue
- Cursor copy mode warns on skills with internal symlinks (pre-existing core crate issue)

### Key files
- `crates/skills-manager-cli/src/main.rs` — clap structs + entry point
- `crates/skills-manager-cli/src/commands.rs` — command implementations
- `docs/superpowers/specs/2026-04-13-phase2-cli-design.md`

---

## Phase 3: Plugin Management 📝

**Completed:** 2026-04-13
**PR:** knjf/skills-manager#4 (open for review)
**Branch:** phase3/plugin-management

### What was done
- DB migration v5→v6: `managed_plugins` + `scenario_plugins` tables
- New core module: `crates/skills-manager-core/src/plugins.rs` — discovery, apply, restore
- Plugin discovery reads `~/.claude/plugins/installed_plugins.json`
- Per-scenario enable/disable by manipulating the JSON manifest
- 4 new Tauri IPC commands
- Scenario switch integration: applies plugin state automatically
- 162 tests pass (16 new plugin tests)

### Key design decisions
- Never touches plugin cache directories — only manipulates JSON
- Plugins default to enabled (backward compatible)
- Full plugin entry JSON saved for restore

### Key files
- `crates/skills-manager-core/src/plugins.rs` — plugin management core
- `src-tauri/src/commands/plugins.rs` — Tauri IPC wrappers

---

## Phase 4: Packs UI 📝

**Completed:** 2026-04-13
**PR:** knjf/skills-manager#5 (open for review)
**Branch:** phase4/packs-ui

### What was done
- New view: `src/views/PacksView.tsx` — pack CRUD with card grid + detail view
- PackDialog: create/edit with icon picker (10 icons) + color picker (8 colors)
- AddSkillsDialog: multi-select searchable skill assignment
- Sidebar "Packs" nav item added
- TypeScript interfaces + 11 API wrappers in `tauri.ts`

### Key files
- `src/views/PacksView.tsx` — main packs page (530 lines)
- `src/lib/packIcons.tsx` — icon/color definitions

---

## Phase 5: Matrix View + Plugin UI 📝

**Completed:** 2026-04-13
**PR:** knjf/skills-manager#6 (open for review)
**Branch:** phase5/matrix-plugin-ui

### What was done
- **MatrixView** (`src/views/MatrixView.tsx`) — agent × pack grid with expand/collapse
  - Pack-level bulk toggles (green/amber/grey status)
  - Skill-level individual toggles
  - Column-level "toggle all" headers
- **PluginsView** (`src/views/PluginsView.tsx`) — plugin list with per-scenario toggles
  - Graceful fallback when Phase 3 backend not merged (shows placeholder)
  - Search, scan, scope badges
- Routes + sidebar items + i18n keys added

### Dependencies
- MatrixView: works with Phase 1 APIs (on main)
- PluginsView: requires Phase 3 merge for full functionality

---

## Development Workflow

每個 Phase 嘅流程：
```
brainstorming → writing-plans → [plan-eng-review]
→ subagent-driven-development (TDD)
→ simplify → review → git-commit-push-pr
```

Skills toolkit 詳見 `CLAUDE.md` → "Development Workflow — Skills Toolkit"

## References

- Design specs: `docs/superpowers/specs/`
- Implementation plans: `docs/superpowers/plans/`
- Development plan: `DEVELOPMENT_PLAN.md`
- Project instructions: `CLAUDE.md`
