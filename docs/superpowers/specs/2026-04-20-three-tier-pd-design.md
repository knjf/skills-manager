# Three-Tier Progressive Disclosure

**Date**: 2026-04-20
**Status**: Design
**Depends on**: `2026-04-20-pd-sync-wiring-design.md` (wiring complete, PR #28 merged)

## Problem

After PD sync wiring shipped, `hybrid` mode successfully reduces Claude Code's system-prompt cost by materializing essentials + pack routers only. But the routing experience has two defects:

1. **Router body noise**: `router_render` emits a markdown table of pack skills using each skill's *full original `description` field*. These descriptions were authored in isolation by skill vendors — they are verbose, repetitive across similar skills, and don't surface which skill to pick when several look similar (e.g. `perp-search` vs `autoresearch` vs `codex-deep-search`).

2. **Router description is a single flat field**: `packs.router_description` is the only signal Claude sees in the system prompt for a whole pack. To keep it trigger-rich we tend to cram keywords + domain + trigger phrases into one string. Claude Code's frontmatter supports a separate `when_to_use` field that shares the 1536-char cap — using both lets us cleanly split "what this pack is for" from "when to invoke it".

## What Research Confirmed

Authoritative findings from Claude Code docs (`https://code.claude.com/docs/en/skills.md`):

- `description` + `when_to_use` together cap at **1536 characters** per SKILL.md. Exceeding truncates.
- Session start scans SKILL.md frontmatter only; body loads when the skill is invoked. **Live-reload** on file change within a session.
- Skill selection is **100% LLM pattern-match** on descriptions in a flat list — no server-side ranking, no native pack/category concept.
- Router body is plain markdown; vault paths inside it are strings — Claude must explicitly `Read` them to pull the real skill in.
- Compaction may evict router bodies from older turns; re-invoking the router reloads cheaply.

These facts anchor the design:

- L1 (router description) **cannot be ultra-short** — it competes with individual essentials for the same LLM pattern-match. It needs keyword density.
- L2 (router body per-skill lines) **should be self-contained** — if Claude re-invokes the router later, it should still pick the right skill without having to Read every SKILL.md.
- L3 (full SKILL.md) is unchanged — it's the authoritative execution content.

## Goals

1. Introduce a clear three-tier progressive disclosure:
   - **L1** — pack-level: `description` + `when_to_use`, authored per pack, in system prompt on session start
   - **L2** — per-skill compressed line, authored per skill, rendered into router body at sync time
   - **L3** — full SKILL.md, unchanged, read via `Read` tool
2. L2 authoring data survives re-scans, scenario switches, vault updates. Stored in DB per skill.
3. L1 supports Claude Code's native `when_to_use` frontmatter field.
4. Fallback: when a skill has no L2 authored, router body falls back to the skill's original `description`. Rollout can be incremental.
5. CLI supports single-skill edits and bulk YAML import of L2 lines.
6. Backward-compatible: existing scenarios in `full` mode behave identically. `hybrid` scenarios with unfilled L2 still work via fallback.

## Non-Goals

- UI changes for editing L2 per skill (CLI + YAML import is sufficient for the author workflow; UI can land as a follow-up).
- Rewriting all 190 skills' L2 content as part of this spec — that is a separate data-entry task.
- Changing the L1 content of existing packs (marketing already has one). Content rewrites are data-entry follow-ups.
- LLM-generated L2 at runtime. L2 is human-authored and persisted.
- Retroactively limiting authored `description_router` length via schema — we'll document a target (~80 chars) but not enforce in DB.

## Architecture

### Three storage locations

```
packs table:
  router_description   TEXT  ← L1 "description" field (exists)
  router_when_to_use   TEXT  ← L1 "when_to_use" field (NEW)

skills table:
  description          TEXT  ← L3-referencing, original SKILL.md description (exists)
  description_router   TEXT  ← L2 compressed per-skill line (NEW)
```

### Rendering pipeline

Sync engine renders router SKILL.md at sync time:

```
render_router_skill_md(pack, skills, vault_root) ->

---
name: pack-<name>
description: <pack.router_description>
when_to_use: <pack.router_when_to_use>   # only emitted if set
---

# Pack: <name>

揀一個 skill，用 `Read` tool 讀對應 SKILL.md，跟住做。

| Skill | 用途 | 路徑 |
|---|---|---|
| <skill.name> | <skill.description_router OR skill.description> | <vault_path> |
| ... |
```

Key change: the middle column uses `description_router` when set, else falls back to `description`.

`when_to_use` frontmatter field is emitted only if `router_when_to_use` is `Some`. YAML frontmatter key-value escaping follows the existing `description` pattern (wrap in double quotes, escape internal quotes).

### Why not derive L2 from L3 at runtime?

Three reasons L2 is stored, not computed:
1. **Quality**: Human-authored differentiation ("this vs that") survives re-scans.
2. **Determinism**: Same skill always renders with the same L2 line — no LLM variance.
3. **Editability**: A bad router description gets corrected with one CLI call, not a re-scan.

## Data Model Changes

DB v11 migration:

```sql
ALTER TABLE packs ADD COLUMN router_when_to_use TEXT;
ALTER TABLE skills ADD COLUMN description_router TEXT;
```

Both columns are nullable. `NULL` means "not authored; use fallback".

`PackRecord` and `SkillRecord` structs gain the corresponding `Option<String>` fields. Serialization adds them with `#[serde(default)]` so older snapshots/exports don't break.

## Components Changed

### `crates/skills-manager-core/src/skill_store.rs`

- Add `router_when_to_use: Option<String>` to `PackRecord` struct. `map_pack_row`, `insert_pack`, `set_pack_router`, and tests updated.
- Add `description_router: Option<String>` to `SkillRecord` struct. `map_skill_row`, `insert_skill`, `update_skill`, and tests updated.
- New methods:
  - `set_pack_when_to_use(pack_id, Option<&str>) -> Result<()>`
  - `set_skill_description_router(skill_id, Option<&str>) -> Result<()>`
  - `bulk_set_skill_description_router(pairs: &[(String, Option<String>)]) -> Result<usize>` — accepts skill-name → description_router pairs for bulk import; returns count updated.

### `crates/skills-manager-core/src/router_render.rs`

- `render_router_skill_md`:
  - If `pack.router_when_to_use.is_some()`, emit `when_to_use: <escaped>` below `description:` in frontmatter.
  - Table middle column uses `skill.description_router.as_deref().unwrap_or(&skill.description)`.
- Existing tests extended; new tests cover:
  - Frontmatter includes `when_to_use` when set, omits when absent
  - Table uses `description_router` when skill has it, falls back to `description` when `None`
  - YAML escaping of internal quotes in `when_to_use`

### `crates/skills-manager-core/src/migrations.rs`

- Add `v11_add_three_tier_fields` migration. Schema version bumped to 11.

### `crates/skills-manager-cli/src/commands.rs`

- Extend `cmd_pack_set_router`:
  - New `--when-to-use <text>` option to write `router_when_to_use`
  - `--description` and `--when-to-use` can each be set independently
  - `--clear-when-to-use` flag to reset to NULL
- New `cmd_skill_set_router_desc(skill_name, description: Option<&str>)` — `None` clears the field.
- New `cmd_skill_import_router_descs(yaml_path)` — reads YAML, calls `bulk_set_skill_description_router`, prints summary (`N updated, M skipped`).

YAML format for bulk import:

```yaml
# Path: l2.yaml
# Each top-level key is a skill name; value is the description_router string.
# Missing entries leave the skill untouched. Use `null` to explicitly clear a field.

perp-search: "Perplexity single synthesized answer (pro search + reasoning)"
autoresearch: "Autonomous Karpathy-style multi-agent loop → cited report"
codex-deep-search: "Codex CLI follows links + cross-refs when others insufficient"
# ...
```

### `crates/skills-manager-cli/src/main.rs`

- Extend `PackAction::SetRouter` with `when_to_use: Option<String>` and `clear_when_to_use: bool` flags.
- New `SkillAction` subcommand group:
  - `SetRouterDesc { name: String, description: Option<String>, clear: bool }`
  - `ImportRouterDescs { file: PathBuf }`

Register `Commands::Skill { action: SkillAction }` and dispatch in `main()`.

### `src-tauri/src/commands/` (Tauri)

- `scenarios.rs` and `packs.rs`: update `PackDto` and `SkillDto` serializers to include the new fields so future UI work has data ready. No new Tauri IPC commands in this spec.

### Fallback behavior

When rendering a router table row:

```rust
let row_description = skill
    .description_router
    .as_deref()
    .filter(|s| !s.trim().is_empty())
    .unwrap_or(&skill.description);
```

Empty string treated as unset. Null → unset. This keeps hybrid mode working for every skill from day one, even before any L2 is authored.

## Error Handling

- YAML parse error in `import-router-descs` → abort with line:col context, no partial writes.
- Unknown skill name in YAML → warn (`skipped: <name>`), continue with remaining entries. Exit 0 unless zero skills updated.
- `set-router` with neither `--description` nor `--when-to-use` nor `--body` nor `--clear-*` → bail with clear message (unchanged from existing behavior).
- DB constraint errors surface as `anyhow::Error`.

## Testing

### Unit tests (skill_store)

- `pack_record_round_trips_router_when_to_use`
- `skill_record_round_trips_description_router`
- `set_skill_description_router_writes_and_clears`
- `bulk_set_skill_description_router_counts_updated`
- `bulk_set_skill_description_router_ignores_unknown_names`

### Unit tests (router_render)

- `frontmatter_emits_when_to_use_if_set`
- `frontmatter_omits_when_to_use_if_none`
- `table_uses_description_router_when_set`
- `table_falls_back_to_description_when_router_desc_none`
- `table_falls_back_to_description_when_router_desc_empty_string`
- `yaml_escapes_quotes_in_when_to_use`

### CLI integration tests

Extend `crates/skills-manager-cli/tests/pd_wiring.rs` (or new `three_tier.rs`):

- `pack_set_router_stores_when_to_use`
- `skill_set_router_desc_persists_and_clears`
- `import_router_descs_yaml_bulk_updates`
- `import_router_descs_yaml_skips_unknown_skills`
- `rendered_router_uses_description_router_after_set`
- `rendered_router_falls_back_to_description_when_unset`

### Manual end-to-end (acceptance gate)

1. `sm pack set-router marketing --description "<short L1>" --when-to-use "<trigger list>"`
2. `sm skill set-router-desc prd --description "Single-shot PRD authoring for software + AI features"`
3. `sm pack regen-all-routers`-equivalent (or re-switch scenario)
4. `cat ~/.claude/skills/pack-marketing/SKILL.md` — verify:
   - Frontmatter has both `description` and `when_to_use`
   - Table row for `prd` uses the new L2 text, not the original 120-char description
5. Bulk import:
   - Write a YAML with 5 skills' L2 lines
   - `sm skill import-router-descs ./l2-test.yaml`
   - Verify `skills.description_router` populated in DB (sqlite check)
6. Clear and re-sync; verify fallback works for unset skills.

## Rollout

1. Land this spec's code changes on `feat/three-tier-pd`.
2. Backward compat: `full` mode scenarios unaffected. `hybrid` mode scenarios continue to work; their router bodies render using original `description` for skills with no L2 authored.
3. Data entry (separate task): author 7 pack L1s + ~190 skill L2s.
4. E2E verification via new Claude Code session: confirm `pack-*` routers still trigger, and router body shows differentiated skill lines post data entry.

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| `when_to_use` YAML field misformatted → Claude Code rejects SKILL.md | Router renderer matches existing `description` escaping; unit test covers quote escaping. |
| Bulk YAML import partially fails; state is ambiguous | `bulk_set_skill_description_router` runs in a single transaction; either all rows commit or rollback on error. Exception: "unknown skill name" is a warn, not an abort. |
| 1536-char cap is exceeded for a pack's `description + when_to_use` | Runtime warning at `set-router` time if combined length > 1500 chars (soft warn, not error — avoids blocking a borderline case; Claude Code truncates gracefully). |
| Author writes L2 longer than router body practical width | No hard limit; documented target ~80 chars. Renderer does not wrap/truncate. |
| Skill renamed in vault, DB `description_router` orphaned under old name | Bulk import uses skill ids resolved from current names; rename handling is existing machinery. No new risk. |

## Open Questions

None at design time. Implementation may surface:
- Whether to expose `description_router` in the SkillDto for the UI immediately, or defer. Current spec emits it (no cost), UI consumption is future work.

## Spec Self-Review

- Placeholders: none.
- Internal consistency: L1/L2/L3 table, data model, rendering pipeline, CLI commands, and tests all reference the same field names (`router_when_to_use`, `description_router`).
- Scope: bounded — DB migration + renderer + CLI + tests. UI deferred. Content authoring deferred.
- Ambiguity: fallback rules (empty string treated as unset) explicit. `when_to_use` frontmatter emission conditional on `Some` (not empty string) — documented.
