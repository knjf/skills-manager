# Skills Manager Fork — Development Progress

## Overall Status

```
Phase 1 ✅ → Phase 2 ✅ → Phase 3 ✅ → Phase 4 ✅ → Phase 5 ✅
Per-Agent ✅ → Matrix Fix ✅ → Native Skills 🔄 → Pack Seeding ✅ → Progressive Disclosure ✅ → Dashboard ⬜ → Tray Menu ⬜
```

✅ = merged | 🔄 = in progress | ⬜ = planned

---

## Completed Phases

### Phase 1: Core Crate Extraction + Skill Packs ✅
- **PR:** #1 (merged 2026-04-13)
- Core crate at `crates/skills-manager-core/`, DB v5, pack CRUD, effective skill resolution

### Phase 2: CLI Binary ✅
- **PR:** #3 (merged 2026-04-13)
- `sm` CLI replacing shell script, pack-aware, installed at `~/.local/bin/sm`

### Phase 3: Plugin Management ✅
- **PR:** #4 (merged 2026-04-13)
- DB v6, plugin discovery, per-scenario enable/disable via `installed_plugins.json`

### Phase 4: Packs UI ✅
- **PR:** #5 (merged 2026-04-13)
- PacksView with CRUD, icon/color picker, skill assignment

### Phase 5: Matrix View + Plugin UI ✅
- **PR:** #6 (merged 2026-04-13)
- MatrixView (agent × pack grid), PluginsView with per-scenario toggles

### Matrix Architecture Fix ✅
- **PR:** #8 (merged 2026-04-14)
- Fixed toggle flash/revert for pack-inherited skills
- MatrixView shows all effective skills + ungrouped section
- Fixed `copy_dir_recursive` symlink handling for Cursor

### Per-Agent Scenario Assignment ✅
- **PR:** #9 (merged 2026-04-15)
- DB v7: `agent_configs` + `agent_extra_packs` tables
- Each agent independently has base scenario + extra packs
- Agent Detail page, Sidebar AGENTS section
- CLI: `sm agents`, `sm switch <agent> <scenario>`, `sm agent add-pack/remove-pack`

---

## Current Iteration: Polish + Features

### Agent Native Skills Management 🔄
**Status:** Starting
**Goal:** Identify and manage agent-native skills (pre-installed by agent, not SM). Show in Agent Detail page. Prevent SM from overwriting native skills.

### Default Pack Seeding ✅
**Status:** Merged (subsumed by Progressive Disclosure)
**Goal:** Seed 132 skills into 9 packs (base, gstack, marketing, etc.) on first run

### Progressive Disclosure ✅ (partial — sync wiring incomplete)
**PR:** (pending) **Status:** Merged 2026-04-19
**Goal:** Reduce Claude Code system-prompt tokens ~85% via file-based pack routers + Read-on-demand.
**Changes:** DB v9 migration, 16-pack taxonomy, router_render + disclosure sync engine, pack-router-gen builtin skill, CLI subcommands, Tauri IPC, Frontend (PacksView / ScenariosView / MatrixView / Sidebar / Dashboard).
**Subsumed:** Default Pack Seeding.

### Three-Tier UI ✅
**Status:** Complete (PR pending) **Date:** 2026-04-21
**Goal:** Close GUI gap for three-tier PD — edit `router_when_to_use` + `description_router` from Tauri app so non-technical users don't need the CLI.
**Changes:**
- 2 additive Tauri IPC commands: `set_pack_when_to_use`, `set_skill_description_router`
- `ManagedSkillDto` gains `description_router` field; TS `PackRecord` gains `router_when_to_use`, TS `ManagedSkill` gains `description_router`
- New TS wrappers `setPackWhenToUse`, `setSkillDescriptionRouter`
- `RouterEditor` supports `whenToUse` textarea + combined 1536-char counter (red/yellow/green)
- `PacksView` saves via both `set_pack_router` + `set_pack_when_to_use`, renders L1 coverage pill per pack card (green / yellow / gray)
- `SkillDetailPanel` gains L2 editor section + sibling-skill list section (gracefully hidden when sisterSkills is empty)
- `MySkills` shows L2 ✓/— badge per row (grid + list views) + "Show only skills without L2" filter; wires `setSkillDescriptionRouter` on save
- 2 new staged Vitest test files (RouterEditor 9 tests, SkillDetailPanel 6 tests)
**Known limitation:** sibling list is currently empty because MySkills doesn't load pack-skill relations. Graceful fallback (section hidden). Future work: load pack membership in MySkills to populate sibling list for the 分叉 helper.
**Verified:** Rust tests all pass (275+), `pnpm tsc --noEmit` clean, `pnpm run build` clean. Visual e2e requires `pnpm tauri:dev` — performed separately.

### Skills Manager Skills Pack ✅
**Status:** Complete (PR pending) **Date:** 2026-04-21
**Goal:** Ship a builtin `sm` pack (8 skills, essential) that teaches Claude Code how to use Skills Manager itself. Any user with `sm` installed gets AI-guided operation via Claude sessions.
**Changes:**
- 8 new `SKILL.md` assets under `crates/skills-manager-core/assets/builtin-skills/sm-*/` — sm-overview / scenarios / packs / skills / authoring / debug / agents / install
- `builtin_skills.rs` refactored — `BUILTIN_SKILLS` slice embeds all 9 skills (1 pre-existing + 8 new) via `include_str!`; installer loops uniformly; added `SM_SKILL_NAMES` public const
- `pack_seeder::ensure_sm_pack()` — fully idempotent: creates `sm` pack (essential) + L1 `router_description` + `router_when_to_use` + inserts missing DB skill records for 8 sm-* names + links into every scenario + sets per-skill L2 `description_router`. Safe to call every startup
- `central_repo::ensure_sm_pack_installed()` wrapper called from CLI `open_store()` and Tauri app startup — ensures vault + DB + pack all in sync on every invocation
- CLI `open_store()` now calls `ensure_central_repo()` first (CLI didn't previously, only Tauri did) so the installer runs for CLI users too
- 1 new CLI integration test (`tests/sm_pack.rs`), 2 new pack_seeder unit tests
**Verified end-to-end:**
- `sm list` triggers installer + seeder: 8 sm-* dirs appear in `~/.skills-manager/skills/`, DB has sm pack (essential, L1 set, when_to_use set), 8 skills linked with L2 each, pack present in all 7 default scenarios
- `sm switch claude_code standard-marketing` materializes all 8 sm-* skills into `~/.claude/skills/` (essential pack → direct sync)
- Idempotent: second invocation is a no-op
- Workspace tests: 275 pass (266 core + 9 CLI integration), 0 failures

### Pack Content Authoring ✅
**Status:** Complete **Date:** 2026-04-21
**Goal:** Populate L1 (`router_description` + `router_when_to_use`) and L2 (per-skill `description_router`) for all 8 non-essential packs so hybrid mode sessions surface triggerable router descriptions.
**Done:**
- Pilot (2026-04-21 AM): `research` pack (L1 + 10 skills L2) + `gstack` pack (L1 + 45 skills L2). Live-verified.
- Scale-up (2026-04-21 PM): `agent-orchestration` (L1 + 7), `browser-tools` (L1 + 7), `design` (L1 + 10), `knowledge` (L1 + 6 unique), `ops` (L1 + 4). 34 skills bulk-imported via YAML.
- **All 7 non-essential packs now have complete L1+L2 content.** Marketing pack still has partial L2 (1/4 — authored during three-tier PD spec). Base pack is essential, no router needed.
**Verified:** Live Claude Code session surfaced all 7 pack-* routers with their L1 description + when_to_use on next sync. Router bodies use authored L2 lines with → Pick for... markers on branching clusters. Skills with no L2 fall back to first-sentence of original description.

### Three-Tier Progressive Disclosure ✅
**Status:** Complete (PR pending) **Date:** 2026-04-20
**Goal:** Split PD into three storage tiers so routers carry authored per-skill differentiation and Claude Code's native `when_to_use` frontmatter field is populated.
**Changes:** DB v11 (two nullable columns: `packs.router_when_to_use`, `skills.description_router`). `router_render` emits `when_to_use` in frontmatter and prefers `description_router` in the table with fallback to first-sentence of `description`. CLI `pack set-router --when-to-use / --clear-when-to-use` + new `sm skill set-router-desc` + `sm skill import-router-descs` (YAML bulk, transaction-safe). 4 new CLI integration tests.
**Verified end-to-end:** Set `when_to_use` on `marketing` pack + `description_router` on `prd`. Switched hybrid mode. Rendered `~/.claude/skills/pack-marketing/SKILL.md` contains both frontmatter fields and the custom L2 row text (prd shows "Single-shot PRD authoring — exec summary, user stories, risks." instead of 120-char vendor original). Bulk YAML import updated 3 skills + skipped 1 unknown as expected. Claude Code session surfaced `when_to_use` appended to the description in the skills listing — proving native `when_to_use` integration works.

### PD Sync Wiring ✅
**Status:** Complete (PR pending) **Date:** 2026-04-20
**Discovered:** While drafting router descriptions for marketing pack, found `reconcile_agent_dir` (the disclosure-mode-aware sync) was only invoked from unit tests. Production sync path (`sync_scenario` in CLI, Tauri sync command) bypassed disclosure mode and always materialized every skill (Full mode behaviour).
**Done:** Wired `reconcile_agent_dir` + `unreconcile_agent_dir` into both CLI (`sync_scenario`, `sync_agent`, `unsync_scenario`) and Tauri (`sync_agent_skills`, `unsync_agent_skills`). Added store helpers `get_packs_with_skills_for_scenario`/`_for_agent`. Added per-skill exclusion threading through `resolve_desired_state` for tool-toggle compatibility. Added CLI `sm scenario set-mode` + `sm pack set-essential`. 4 new integration tests in `crates/skills-manager-cli/tests/pd_wiring.rs`. Total 259 tests passing.
**Verified end-to-end:** `sm pack set-essential base true` → `sm scenario set-mode standard-marketing hybrid` → `sm switch claude_code standard-marketing` produces 17 essential skill symlinks + 7 `pack-*` router dirs in `~/.claude/skills/`. `pack-marketing/SKILL.md` correctly shows our router description + auto-rendered skill table with vault paths. Switching back to `everything` (full mode) removes all `pack-*` dirs. New Claude Code sessions immediately see the router descriptions instead of individual skill descriptions — confirmed via system reminder showing 7 `pack-*` entries during the e2e walkthrough.

### Dashboard Update ⬜
**Status:** Planned
**Goal:** Show per-agent status instead of single global scenario

### Tray Menu Update ⬜
**Status:** Planned
**Goal:** Per-agent quick switch in tray menu

### My Skills Retirement ⬜
**Status:** Planned (low priority)
**Goal:** Evaluate after new pages are validated

### Cursor Copy Fix ⬜
**Status:** Planned (low priority)
**Goal:** Further improve copy_dir_recursive edge cases

---

## References

- Design specs: `docs/superpowers/specs/`
- Implementation plans: `docs/superpowers/plans/`
- Development plan: `DEVELOPMENT_PLAN.md`
- Project instructions: `CLAUDE.md`
