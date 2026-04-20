# Progressive Disclosure — Sync Wiring

**Date**: 2026-04-20
**Status**: Design
**Depends on**: `2026-04-19-progressive-disclosure-design.md` (schema + engine merged 2026-04-19)

## Problem

Progressive Disclosure (PD) was merged 2026-04-19 (PR #25). The schema, the disclosure-mode-aware sync engine (`reconcile_agent_dir`), the router renderer (`router_render::render_router_skill_md`), the CLI router-management subcommands (`sm pack set-router`, `list-routers`, etc.), the builtin `pack-router-gen` skill, and the frontend (`DisclosureModeSelect`, `TokensSavedWidget`, Sidebar mode badge) all shipped.

But the production sync paths bypass `reconcile_agent_dir`. Both:

- CLI `sync_scenario` / `sync_agent` (`crates/skills-manager-cli/src/commands.rs:772, 814`)
- Tauri `sync_agent_skills` (`src-tauri/src/commands/scenarios.rs:430`)

iterate the flat skill list returned by `get_effective_skills_for_scenario` / `_agent` and call `sync_skill` per skill. They never read the scenario's `disclosure_mode` and never call `reconcile_agent_dir`.

**Observed**: Setting a scenario to `hybrid` and running `sm switch claude_code <scenario>` still materializes every skill. No `pack-*` router files appear in `~/.claude/skills/`. The PD feature is invisible in real use.

**Discovered**: 2026-04-20, while drafting router descriptions for the marketing pack as part of a separate "data entry" task — the absence of materialized routers exposed the missing wiring.

## Goals

1. `disclosure_mode` on a scenario actually controls what gets materialized into the agent skills directory.
2. `hybrid` mode produces: essential-pack skills materialized + one `pack-<name>/SKILL.md` router per non-essential pack.
3. `router_only` mode produces: only routers, no materialized skills.
4. `full` mode behaves exactly as today (zero behavioral regression for current users).
5. Per-skill tool toggles (the existing `enabled_tools_for_scenario_skill` filter) continue to work in all three modes.
6. Switching out of a hybrid/router_only scenario cleanly removes router directories.
7. CLI exposes a `set-mode` command so PD can be exercised end-to-end without the GUI.
8. After wiring, marking `base` pack as essential and switching to a hybrid scenario produces a visible, testable result: essentials in `~/.claude/skills/`, routers for everything else.

## Non-Goals

- Frontend changes. UI already has `DisclosureModeSelect` and supporting components; once the backend honors disclosure_mode, the UI works as-is.
- Writing router descriptions for the 8 packs that currently show `<not generated>`. Tracked separately; will be done after wiring is verified.
- Reorganizing pack taxonomy or assigning more skills to packs.
- New essential-pack policy beyond the single `base → essential` flip needed for the e2e demo.
- Mid-session skill hot-swap. Claude Code scans `~/.claude/skills/` once at session start; PD effects take hold next session.

## Architecture

### Current (broken) sync flow

```
sm switch <scenario>
  → cmd_switch
    → sync_scenario(store, scenario_id, adapters, configured_mode)
      → for each adapter:
          for each skill in get_effective_skills_for_scenario(scenario_id):
            sync_skill(source, target, SyncMode::Symlink|Copy)   ← per-skill, no disclosure
```

### Target sync flow

```
sm switch <scenario>
  → cmd_switch
    → sync_scenario(store, scenario_id, adapters, configured_mode)
      → mode = store.get_scenario(scenario_id).disclosure_mode
      → for each adapter:
          packs_with_skills = store.get_packs_with_skills_for_scenario(scenario_id)
          excluded = collect_per_skill_excluded(store, scenario_id, adapter.key)
          reconcile_agent_dir(
            agent_skills_dir,
            packs_with_skills,
            mode,
            vault_root,
            excluded,        ← new parameter
          )
```

`reconcile_agent_dir` already does the right thing internally (calls `resolve_desired_state(mode)` then materializes/renders/cleans). The fix is at the call sites.

### Per-skill tool toggle integration

`reconcile_agent_dir` currently has no notion of a per-skill exclude set. Today's CLI/Tauri loops filter inside the loop. We add an `excluded_skills: &HashSet<String>` parameter (skill names) and `resolve_desired_state` filters skills before emitting them. Routers are unaffected by per-skill toggles (router lists every pack skill regardless).

### Owned vs borrowed PackWithSkills

`disclosure::PackWithSkills<'a>` holds borrowed references. Building it from store results requires the source `Vec<PackRecord>` and `Vec<SkillRecord>` to outlive the call. We restructure store helpers to return owned data (`Vec<(PackRecord, Vec<SkillRecord>)>`) and the call sites construct borrowed `PackWithSkills` slices for the engine. No engine signature change beyond the new `excluded_skills` parameter.

## Data Model Changes

None. All required schema (`scenarios.disclosure_mode`, `packs.is_essential`, `packs.router_description`, `packs.router_body`) shipped in DB v9.

One configuration change applied as part of the spec (one-line):

- Mark `base` pack as `is_essential = 1`. Done via CLI flag (added in this spec) or a one-shot SQL on first run if the column is unset.

## Components Changed

### `crates/skills-manager-core/src/skill_store.rs`

Add two methods:

- `get_packs_with_skills_for_scenario(scenario_id) -> Result<Vec<(PackRecord, Vec<SkillRecord>)>>`
  - Reuses `get_packs_for_scenario` + `get_skills_for_pack` per pack.
- `get_packs_with_skills_for_agent(tool_key) -> Result<Vec<(PackRecord, Vec<SkillRecord>)>>`
  - Combines the agent's base scenario packs + extra packs (from `agent_extra_packs`).
  - Deduplicates by pack id.

Add CLI-facing helper:

- `set_pack_essential(pack_name, essential: bool)` (already exists in some form? if not, add).

### `crates/skills-manager-core/src/sync_engine/disclosure.rs`

Modify `resolve_desired_state` signature to accept an `excluded_skills: &HashSet<String>` and filter skills before emitting `EntryKind::Skill`. Routers are not filtered by this set.

### `crates/skills-manager-core/src/sync_engine/mod.rs`

Modify `reconcile_agent_dir` to take `excluded_skills: &HashSet<String>` and pass through to `resolve_desired_state`.

### `crates/skills-manager-cli/src/commands.rs`

- Replace `sync_scenario` body to use `reconcile_agent_dir`.
- Replace `sync_agent` body similarly. For per-agent sync, use `get_packs_with_skills_for_agent` and the agent's effective scenario id (for `disclosure_mode` lookup).
- Replace `unsync_scenario` body to call a new `unreconcile_agent_dir` (removes all SM-managed entries by `is_sm_managed` heuristic).
- Add `cmd_set_disclosure_mode(scenario_name, mode)` — wraps `store.set_scenario_disclosure_mode`.
- Add `cmd_set_pack_essential(pack_name, essential)` — wraps store helper.

### `crates/skills-manager-cli/src/main.rs`

Register two new subcommand groups:

- `sm scenario set-mode <name> <full|hybrid|router_only>`
- `sm pack set-essential <name> <true|false>`

(Promote existing top-level scenario actions under a `scenario` subcommand if symmetry helps; otherwise free-standing.)

### `src-tauri/src/commands/scenarios.rs`

- Replace `sync_agent_skills` body to use `reconcile_agent_dir`.
  - Compute excluded set: for each skill in pack's skills, check `get_enabled_tools_for_scenario_skill(scenario_id, skill_id)`; if adapter.key not in enabled, add skill.name to excluded set.
  - Insert `targets` table records only for skills that were actually materialized (skip routers — routers tracked via filesystem heuristic).
- Replace `unsync_agent_skills` body to call new `unreconcile_agent_dir` so router dirs get removed too.

### `crates/skills-manager-core/src/sync_engine/mod.rs` (one more)

Add `unreconcile_agent_dir(agent_skills_dir) -> Result<usize>` — scans dir, removes everything `is_sm_managed` says yes to, returns count.

## Error Handling

- Missing pack referenced in scenario_packs → log warning, skip.
- Invalid disclosure_mode in DB → already errors at parse (existing `DisclosureMode::parse`).
- Per-skill toggle defaults missing → existing `ensure_scenario_skill_tool_defaults` covers it; preserved at call site.
- Router write failure (disk full / permission) → propagate as `anyhow::Error`; partial state acceptable (idempotent re-sync recovers).

## Testing

### Unit tests (extend existing)

In `skill_store.rs` test module:
- `get_packs_with_skills_for_scenario_returns_pack_skill_pairs`
- `get_packs_with_skills_for_agent_includes_extra_packs`
- `get_packs_with_skills_for_agent_dedupes_overlapping_packs`

In `disclosure.rs` test module:
- `excluded_skills_filtered_in_full_mode`
- `excluded_skills_filtered_in_hybrid_mode`
- `excluded_skills_does_not_affect_routers`

In `sync_engine/mod.rs` test module:
- `reconcile_with_excluded_skills_skips_them`
- `unreconcile_removes_routers_and_symlinks`
- `unreconcile_leaves_native_skills_alone`

### CLI integration tests

New `crates/skills-manager-cli/tests/pd_wiring.rs`:
- `switch_to_hybrid_creates_pack_routers`
- `switch_from_hybrid_to_full_removes_routers`
- `set_mode_persists_to_db`
- `set_essential_persists_to_db`
- `hybrid_with_per_skill_toggle_excludes_skill_but_keeps_router`

Setup: temp `HOME`, seeded DB with one essential pack + two non-essential packs + a fake vault.

### Manual end-to-end (acceptance gate)

1. `sm pack set-essential base true`
2. `sm pack set-router marketing --description "<draft from earlier session>"`
3. `sm scenario set-mode standard-marketing hybrid`
4. `sm switch claude_code standard-marketing`
5. `ls ~/.claude/skills/` — expect `base` skills (symlinks) + `pack-marketing/`, `pack-gstack/`, etc.
6. `cat ~/.claude/skills/pack-marketing/SKILL.md` — expect frontmatter with our description + auto-rendered skill table.
7. `sm switch claude_code everything` — expect all `pack-*` directories gone, full skill set materialized.
8. Open new Claude Code session in unrelated project — confirm router description appears in skills list, not the 4 individual marketing skill descriptions.

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Per-skill tool toggles silently break in hybrid | Dedicated unit + CLI integration test (`hybrid_with_per_skill_toggle_excludes_skill_but_keeps_router`) |
| Existing `targets` table records become stale (routers not tracked) | `unreconcile_agent_dir` uses filesystem `is_sm_managed` heuristic, independent of `targets` table; targets remain skill-only |
| Idempotent re-sync clobbers a user-edited router file | Documented: routers under `pack-*/` are SM-managed, edits should go through `sm pack set-router`. Sync engine already only rewrites if content differs. |
| Borrowed `PackWithSkills<'a>` lifetime issues at call sites | Store returns owned tuples; call sites build `Vec<PackWithSkills>` from local owned data |
| `reconcile_agent_dir` signature change breaks downstream callers | No production callers today — only tests. Update test sites in same PR. |
| Frontend assumes Full-mode behavior in some computed display (e.g. skill counts) | Skim `MatrixView`, `MySkills`, `Sidebar` after backend lands; any cosmetic mismatch is a separate follow-up, not a wiring blocker |

## Rollout

1. Implement + test on a feature branch (`feat/pd-sync-wiring`).
2. Manual e2e walkthrough on dev machine before opening PR.
3. PR includes screenshots of `~/.claude/skills/` before and after, and `cat` output of one router file.
4. Backwards-compat verified: any scenario currently in `full` mode (every existing scenario today) behaves identically.
5. Post-merge: run the deferred "fill 8 router descriptions" data-entry task.

## Open Questions

None at design time. Implementation may surface:
- Whether `is_sm_managed` heuristic is reliable enough across mixed user states (older agent dirs may have stale entries from prior SM versions). If false negatives appear in `unreconcile`, we extend the heuristic.

## Spec Self-Review

- Placeholders: none.
- Internal consistency: architecture diagram, components-changed list, and tests all reference the same call points.
- Scope: bounded — backend wiring only, plus the single `base → essential` flip required to make the e2e demo visible. Frontend, router descriptions, and broader pack taxonomy decisions are explicitly deferred.
- Ambiguity: `set-mode` and `set-essential` CLI shape is specified verbatim above; implementation may pick `sm scenario set-mode` vs `sm set-mode` based on what reads naturally — both are acceptable.
