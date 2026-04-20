# Three-Tier UI — Edit Fields for `when_to_use` + `description_router`

**Date**: 2026-04-21
**Status**: Design
**Branch**: `feat/three-tier-ui`
**Depends on**: PRs #28, #29, #30 (all merged). Backend tiers L1 + L2 + L3 wiring complete; authoring currently CLI-only.

## Problem

Three-tier Progressive Disclosure ships with backend + CLI complete:
- Pack `router_description` + new `router_when_to_use`
- Skill `description_router` per pack-member skill

But the Tauri GUI only edits the old `router_description` (+ optional `body`) via `RouterEditor`. Users have no GUI path to:
1. Edit `router_when_to_use` per pack
2. Edit `description_router` per skill
3. See which packs/skills already have L1/L2 authored

For a public-facing tool aimed at both technical (CLI) and non-technical (GUI-only) users, GUI gaps force non-technical users into the CLI. P2's mandate is to close those gaps.

## Goals

1. Pack L1 authoring covers both `description` AND `when_to_use` in one cohesive edit flow.
2. Skill L2 (`description_router`) editable per skill in `SkillDetailPanel`.
3. When editing skill L2, the panel shows sibling skills' current L2 inline — encourages "分叉" differentiation (Option C from brainstorm).
4. Visual coverage badges: PacksView shows L1 + L2 coverage per pack; MySkills shows L2 ✓/✗ per skill with filter.
5. Zero disruption to existing edit flows (description + body still work; current callers don't break).

## Non-Goals

- Bulk YAML import UI. CLI (`sm skill import-router-descs`) covers this use case; GUI is for point edits.
- Changing the sidebar, scenarios view, matrix view, agent detail, or other unrelated pages.
- New DB schema — reuse DB v11 columns.
- Offline / background validation of char caps — surface soft warnings only.
- Export current L1/L2 as YAML from GUI — CLI follow-up if needed.

## Architecture

### Data flow (tier → storage → DTO → component)

```
DB (v11 cols)                IPC DTO                TS type                Component
─────────────────            ───────                ───────                ─────────
packs.router_description  → PackDto.router_description → Pack.router_description → RouterEditor (existing)
packs.router_body         → PackDto.router_body         → Pack.router_body         → RouterEditor (existing)
packs.router_when_to_use  → PackDto.router_when_to_use  → Pack.router_when_to_use  → RouterEditor (NEW section)
skills.description_router → ManagedSkillDto.description_router → ManagedSkill.description_router → SkillDetailPanel (NEW section)
```

### New Tauri IPC commands

Two additive commands (follow `set_pack_essential` pattern — one field, one command):

- `set_pack_when_to_use(pack_id: String, text: Option<String>) -> Result<(), AppError>`
  Wraps `store.set_pack_when_to_use`. `None` clears field.

- `set_skill_description_router(skill_id: String, text: Option<String>) -> Result<(), AppError>`
  Wraps `store.set_skill_description_router`. `None` clears field.

Existing `set_pack_router(pack_id, description, body)` stays unchanged.

### DTO updates

**`src-tauri/src/commands/scenarios.rs`** (or wherever `PackDto` lives):
- `PackDto` gains `router_when_to_use: Option<String>`

**`src-tauri/src/commands/skills.rs`**:
- `ManagedSkillDto` gains `description_router: Option<String>`

Both DTOs serialize via `serde::Serialize` (already derived).

### TS type updates (`src/lib/tauri.ts`)

- `Pack.router_when_to_use: string | null`
- `ManagedSkill.description_router: string | null`
- Two new wrapper functions: `setPackWhenToUse(packId, text)`, `setSkillDescriptionRouter(skillId, text)`

### Component changes

#### `RouterEditor` (extend)

Props expand:
```typescript
type Initial = {
  description: string;
  body?: string | null;
  whenToUse?: string | null;   // NEW
};

type Props = {
  packId: string;
  initial: Initial;
  onSave: (v: { description: string; body: string | null; whenToUse: string | null }) => Promise<void>;
  onGenerate?: () => void;
  onPreview?: () => void;
};
```

UI layout (top to bottom):
1. Router description textarea (existing, 3 rows)
2. Combined char counter — counts `description + when_to_use`, target 150–1400 chars, red ≥1400
3. When-to-use textarea (NEW, 2 rows, placeholder: "Use when user says '...', '...'")
4. Body textarea (existing, 8 rows, optional)
5. Save button (commits all 3 fields via new + existing IPC calls)

Save behavior:
- `onSave` in PacksView invokes `set_pack_router(pack_id, description, body)` for description+body (existing)
- Then `set_pack_when_to_use(pack_id, when_to_use || null)` for the new field
- If both succeed → reload pack data; if either fails → show error, keep form dirty

#### `SkillDetailPanel` (extend)

Add new section below existing metadata (before content tabs). Collapsible like the agent-tool-toggles section:

```
┌─ Router description (L2) ────────────────────┐
│ [textarea: description_router]               │
│ [char count: 47 chars (target 20–150)]       │
│ [Save] [Clear]                               │
└──────────────────────────────────────────────┘

┌─ Sibling skills in pack-marketing ───────────┐
│ • prd: "Single-shot PRD authoring..."        │
│ • marketing: "Marketing skills router..."    │
│ • internal-comms: (no L2 authored)           │
│ (read-only, click to jump to that skill)     │
└──────────────────────────────────────────────┘
```

Data source for sibling list:
- `SkillDetailPanel` receives `skill: ManagedSkill`
- Computes pack membership: iterate `skill.scenario_ids` is no-go (that's scenarios, not packs) — need to join via pack. New prop `sisterSkills?: Array<{ name: string; description_router: string | null }>` passed from MySkills.
- MySkills precomputes siblings for the currently-open skill (requires knowing pack membership — existing code already resolves packs for skills; if not, add via `get_packs_for_skill(skill_id)` IPC).

**Alternative (simpler)**: rather than adding new IPC, MySkills can filter its own skill list by shared pack. Since `ManagedSkill` has pack-membership info via existing infrastructure, just pass sibling names from parent.

**Decision**: MySkills filters its existing skill list by pack membership and passes `sisterSkills` prop to `SkillDetailPanel`. Zero new IPC needed for sibling list.

#### `PacksView` (extend)

Each pack card:
- Existing: name, icon, color, skill count
- NEW: L1 status pill — `L1 ✓` (green) if both `router_description` + `router_when_to_use` set, `L1 partial` (yellow) if only one, `L1 —` (gray) if neither
- NEW: L2 coverage — `L2 8/10` (text) showing how many of pack's skills have `description_router` set

Badge computation: client-side, derived from pack + skill data already loaded.

#### `MySkills` (extend)

Each skill row:
- NEW: small `L2 ✓` or `L2 —` indicator (green dot or gray dash)
- NEW: filter dropdown/checkbox — "Show only skills without L2"

Also in `SkillDetailPanel`: shows the sibling list as described above.

## Error Handling

- IPC failures → show error toast via existing toast infrastructure; keep form state (don't clear user's typed text).
- Save button disabled while saving; re-enabled after completion.
- Char count: soft warning ≥1400 chars; block save at 1536 (hard cap).
- Empty `description_router` saved as `None` (clears field) — matches backend semantics.

## Testing

### Frontend unit tests (Vitest + Testing Library)

**RouterEditor**:
- `renders_when_to_use_textarea`
- `calls_onSave_with_both_description_and_when_to_use`
- `shows_combined_char_counter`
- `clears_when_to_use_when_input_emptied`

**SkillDetailPanel**:
- `renders_description_router_textarea_when_skill_has_value`
- `saves_description_router_via_callback`
- `shows_sibling_list_when_sister_skills_prop_set`
- `sibling_clicking_triggers_onSelectSibling_callback`

**PacksView coverage badges**:
- `shows_L1_green_pill_when_both_fields_set`
- `shows_L2_coverage_ratio_correctly`

**MySkills filter**:
- `filter_only_unset_hides_skills_with_L2`

### Tauri command tests

`src-tauri` has existing command tests. Add:
- `set_pack_when_to_use_command_persists_and_clears`
- `set_skill_description_router_command_persists_and_clears`

Both wrap the already-tested `SkillStore` methods; testing is light (command layer passes through).

### Manual end-to-end (acceptance gate)

1. `pnpm tauri:dev` — launch GUI
2. Navigate to PacksView → click `marketing` pack
3. Scroll to Router Editor → edit `when_to_use` text → Save
4. Verify DB: `sqlite3 ~/.skills-manager/skills-manager.db "SELECT name, router_when_to_use FROM packs WHERE name='marketing';"` shows new text.
5. Navigate to MySkills → click `prd` skill → Skill Detail Panel opens
6. Edit `description_router` → Save
7. Verify DB: `SELECT name, description_router FROM skills WHERE name='prd';` shows new text.
8. Verify sibling list shows other marketing skills.
9. PacksView badges reflect authored state.
10. MySkills filter "unset only" hides authored skills.
11. Run `cargo tauri build` → app bundles cleanly.

## Rollout

1. Implement + test on `feat/three-tier-ui`.
2. E2E on dev machine.
3. PR.
4. After merge: public-ready. All three tiers editable via GUI + CLI.

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| `set_pack_router` callers (existing PacksView) need to also call `set_pack_when_to_use` → two round trips per save | Accept — cost is trivial (local SQLite). UX shows a single "Saving..." spinner. |
| `SkillDetailPanel` is modal, adding more sections makes it scrollable/cluttered | Use collapsible sections (pattern already exists for agent-tool-toggles). Sibling list collapsed by default. |
| MySkills pack-membership lookup: not all `ManagedSkill` instances know their pack list | Use existing `packs` data from the MySkills page context; compute client-side. No new IPC. |
| Badges introduce visual noise on PacksView | Keep pills subtle (small text, muted colors for "unset" state). |
| Char counter: which field's cap? | Cap applies to combined `description + when_to_use` (1536 per Claude Code docs). Counter shows combined total. |

## Open Questions

None at design time. Implementation may surface:
- Whether sibling list should click-navigate to that skill or just display (start with click-to-navigate for discovery; easy to remove if confusing).
- Whether MySkills already shows pack membership per skill; if not, minor query extension.

## Spec Self-Review

- **Placeholders**: none.
- **Consistency**: IPC commands + DTO fields + TS types + components all reference the same field names (`router_when_to_use`, `description_router`).
- **Scope**: bounded — extends 2 components, adds 2 IPC, no DB/schema changes, no new top-level views.
- **Ambiguity**: resolved — sibling list uses prop passed from parent (no new IPC); `when_to_use` commits via separate additive IPC (not breaking existing `set_pack_router`); bulk import explicitly deferred.
