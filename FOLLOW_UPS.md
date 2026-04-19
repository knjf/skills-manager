# Progressive Disclosure — Follow-ups

Tracked in this file because GitHub issues are disabled on the repo. Delete an entry when it lands. Each entry is self-contained.

---

## 1. Frontend test harness (vitest + @testing-library/react)

**Context:** PD work authored `src/components/__tests__/RouterEditor.test.tsx` in Vitest style, but the repo has no JS test framework installed — test never ran.

**What to add:**
- `vitest` dev dep
- `@testing-library/react` + `@testing-library/jest-dom`
- `jsdom` environment
- `"test"` script in `package.json`
- `vitest.config.ts` with jsdom + React plugin

**Already-wired tests waiting to execute:** `RouterEditor.test.tsx`

---

## 2. Integrate Agent Native Skills marker with `is_sm_managed`

**Context:** Sync engine's `is_sm_managed` in `crates/skills-manager-core/src/sync_engine/mod.rs` uses a heuristic (symlink into `.skills-manager/skills/` OR `pack-*` dir with router marker). Works for SM writes but fragile.

**Fix when the Agent Native Skills phase (🔄 in PROGRESS.md) lands:**
- `is_sm_managed` consults the explicit native-skills marker first
- Current heuristic kept as fallback for pre-marker DBs
- Unit tests: real symlink, `pack-` router dir, native marked skill, user-authored without marker

---

## 3. `regen-all-routers` N+1 store opens

**Context:** `cmd_pack_regen_all_routers` in `crates/skills-manager-cli/src/commands.rs` calls `cmd_pack_gen_router(&pack.name)` per pack. Each iteration re-opens store + re-queries `get_all_packs`.

**Fix:**
1. Open store once
2. Load non-essential packs in one query
3. Extract `cmd_pack_gen_router` body into `write_pending_marker_for(pack: &PackRecord, root: &Path)` helper
4. Iterate with already-loaded list

Outcome: N+1 → O(N).

---

## 4. Scenario editor token estimate: real counts

**Context:** `TokenEstimateBadge` in MySkills.tsx + Dashboard widget currently passed `essentialSkillCount=0`, `nonEssentialPackCount=0` placeholders. The Scenario TS type doesn't expose per-pack skill counts or `is_essential` flags to the frontend.

**Fix:**
1. Extend backend `ScenarioDto` (or add `scenario_meta` command) to return: sum of essential-pack skill counts, count of non-essential packs
2. Update TS `Scenario` type to carry these
3. Pass real values in `MySkills.tsx` (line ~854) and `Dashboard.tsx` tokens-saved widget

---

## 5. `description_short` field for Skills (UI tooltips, PD future)

**Motivation:**
- UI tooltips could use 20-word hooks instead of 200-400 char descriptions
- Pack routers could embed short versions to keep bodies tight

**Scope when picked up:**
- `description_short TEXT` column on `skills` (v10→v11 migration)
- Tauri command + TS type extension
- Optional LLM population via a new `skill-desc-shortener` builtin skill (same pattern as `pack-router-gen`)
- `auto_render_body` in `router_render` prefers short when present

Not a priority — only if UI actually needs it.

---

## 6. Dashboard tokens-saved widget data fields

**Context:** Task 21's widget at `src/components/TokensSavedWidget.tsx` renders `— current / — baseline` placeholders because `PackRecord` TS type (pre-PD) didn't expose `is_essential` + per-pack skill counts.

**Fix:** overlaps with #4. When fixing scenario editor counts, update `Pack` TS type to carry these fields so dashboard renders real numbers.

---

## 7. `PluginsView.tsx` type mismatch (resolved inline but leaves brittle fallback)

`PluginsView.tsx` originally used `plugin.name` which doesn't exist on `ManagedPlugin` (the type has `display_name`). Fixed inline during PD smoke-test to use `display_name`. Review:
- Confirm `ManagedPlugin.display_name` is always populated (backend) so the `|| "Plugin"` fallback is defensive only
- Consider renaming the field `name` → `display_name` consistently if user-facing

---

## 8. Document: how to force-reseed packs to v9 taxonomy

`seed_default_packs` is skip-if-exists. Existing DBs keep their 9 old packs after PD migration. To get the 16-pack taxonomy + `is_essential` flags + disclosure modes, users must run **`sm seed-packs --force`**.

**Action:** add a section to `docs/superpowers/specs/2026-04-19-progressive-disclosure-design.md` or `CLAUDE.md` with a one-liner:

> After upgrading to DB v9, run `sm seed-packs --force` to adopt the new 16-pack taxonomy. Existing scenarios keep their names; pack memberships get replaced.

Warn users that this destroys any custom `scenario_packs` assignments they may have set up.
