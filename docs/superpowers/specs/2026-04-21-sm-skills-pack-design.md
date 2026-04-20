# Skills Manager Skills Pack

**Date**: 2026-04-21
**Status**: Design
**Branch**: `feat/sm-skills-pack` (to create)
**Context**: Skills Manager is going public. User base will be both technical (CLI-comfortable) and non-technical (GUI-only). A dedicated skills pack for Skills Manager itself is a force multiplier — every Claude Code user who installs `sm` gets an AI that understands how to operate the tool.

## Problem

After PRs #28 and #29 shipped three-tier Progressive Disclosure + seven packs of L1/L2 content, Skills Manager has a rich surface area:

- 7 non-essential packs + 1 essential (base)
- 3 disclosure modes (full / hybrid / router_only)
- Per-agent scenario assignment + extra packs
- Pack authoring workflow (L1 + L2 with "分叉" differentiation)
- CLI + GUI + DB, all coherent via `reconcile_agent_dir`

A new user — especially a non-technical one — has no low-friction path to learn this. Options today are:

1. Read CLAUDE.md / PROGRESS.md / STATUS.md (high effort, requires tech literacy)
2. Trial-and-error the GUI (misses the conceptual layer)
3. Ask their Claude Code session, which only knows generic context

Option 3 is the wedge. If we author a skills pack specifically for using Skills Manager, every Claude Code session of every user automatically becomes `sm`-aware. Skills are read on-demand, so the token cost is near-zero at baseline; when the user asks "how do I add a skill to marketing pack?", Claude reads the relevant skill and walks them through it.

This aligns with the **agent-native architecture** principle: anything a user can do manually, an agent should be able to do on their behalf.

## Goals

1. **8 skills** covering the core operating surface of Skills Manager (scenarios, packs, skills, agents, install, debug, authoring workflow, overview).
2. **One pack** (`sm`) containing those 8 skills, with authored L1 (`description` + `when_to_use`) and L2 (per-skill `description_router`) following the patterns established in the pilot.
3. **Builtin installation** — pack content embedded in the Rust binary and auto-installed to the vault on app startup, so users get the pack without any opt-in step.
4. **Zero content duplication** — SKILL.md body is the single source of truth per skill.
5. **Discoverable** — default scenarios (at least `everything`, possibly `standard`) include the `sm` pack so `sm` is available from day one on a fresh install.
6. **Maintainable** — when `sm` CLI adds commands in future, updating a skill is a one-line change in a repo file; next `cargo build` ships the updated version to all users.

## Non-Goals

- Writing skills that document *every* CLI subcommand exhaustively. Focus on common user workflows.
- Teaching Rust / TS / Tauri internals. These skills are for *users of sm*, not contributors.
- Auto-generating skill content from code analysis. Human-authored for quality.
- Tutorial / onboarding flow in the GUI (separate future project).
- Video walkthroughs / screenshots (could ship as supporting files later).

## Architecture

### Skill catalog

| Skill | Purpose |
|---|---|
| `sm-overview` | Concepts: vault, scenarios, packs, agents, three-tier PD, disclosure modes, where data lives |
| `sm-scenarios` | `sm list` / `sm current` / `sm switch` / `sm scenario set-mode` — list, switch, change disclosure mode |
| `sm-packs` | `sm packs` / `sm pack context` / `sm pack set-router` (incl. `--when-to-use`) / `sm pack set-essential` |
| `sm-skills` | `sm skill set-router-desc` (single) + `sm skill import-router-descs` (YAML bulk); skill-level mutations |
| `sm-authoring` | End-to-end authoring workflow: gather context → draft L1/L2 → review → bulk import → verify. Teaches the "分叉" differentiation pattern with concrete examples |
| `sm-debug` | Common failure modes: router not rendering, trigger not hitting, stale CLI binary, sync_mode ignored; verification checklist |
| `sm-agents` | `sm agents` / `sm agent info` / `sm agent add-pack` / `sm agent remove-pack` — per-agent scenario + extras |
| `sm-install` | Fresh install, upgrade path, vault path, DB backup, orphan cleanup (`sm fix-orphans`) |

Each skill is a single `SKILL.md` file under `~/.skills-manager/skills/sm-<name>/`. No supporting files in v1; can add `references/*.md` later if content exceeds a comfortable length.

### Pack structure

```
Pack: sm
- Description: "Skills Manager (sm) usage — scenarios / packs / skills / agents / authoring / debug. The CLI is at ~/.local/bin/sm or project target/debug/sm."
- When_to_use: "Use when user asks about 'sm', 'skills manager', 'skill pack', 'router description', 'disclosure mode', 'when_to_use', 'description_router', 'L1/L2/L3', 'how do I author', 'how to switch scenario', 'CLI', 'vault'."
- Is_essential: true (so skills are materialized in hybrid mode, not hidden behind a router)

Skills (8): sm-overview, sm-scenarios, sm-packs, sm-skills, sm-authoring, sm-debug, sm-agents, sm-install
```

**Why essential, not a router**: the operative pack is small (~8 skills, ~40k bytes total), and we want the individual skill `description` fields in the system prompt at all times so Claude can pick the right one without a routing round-trip. Users asking "how do I switch scenarios" should route directly to `sm-scenarios`, not via a `pack-sm` router.

### Installation mechanism

Follow the same pattern as the existing `pack-router-gen` builtin skill.

**In `crates/skills-manager-core/assets/builtin-skills/`**: add 8 subdirectories `sm-<name>/` each containing `SKILL.md`. Already has `pack-router-gen/` as the reference pattern.

**In `crates/skills-manager-core/src/builtin_skills.rs`** (`install_builtin_skills` function): copy the new 8 directories to `<vault>/sm-<name>/` alongside the existing `pack-router-gen/`. Idempotent: overwrite only if content differs.

**In `crates/skills-manager-core/src/pack_seeder.rs`**: extend the default pack seeding to register the `sm` pack with its 8 skills + set `is_essential = true`. The seeder already handles `base`/`gstack`/etc.; adding `sm` follows the same pattern.

**Scenario membership**: add `sm` pack to the `everything`, `standard`, `standard-marketing`, `full-dev`, `full-dev-marketing` scenarios as a seeded default. `minimal` and `core` can skip it (those are explicitly stripped-down).

### DB migration

**DB v12**: optional — if `is_essential = true` is the right default for the `sm` pack, and the seeder can set it on first install, no new schema is needed. Re-use DB v11 schema.

However, existing databases won't re-run seeds on upgrade. We need a **one-shot upgrade migration**: on startup, if `sm` pack doesn't exist in `packs` table, seed it. This can go in `pack_seeder.rs` as an idempotent seed step (not a DB schema migration). Call on every app startup.

### Content authoring

Each SKILL.md follows the standard Claude Code skill format:

```markdown
---
name: sm-<topic>
description: "<one-sentence summary + keywords so the Skill tool can pattern-match>"
---

# <Title>

## When to use
...

## Commands / workflow
...

## Examples
...

## Related
- `sm-<sibling>` — <when to use instead>
```

The `description` in each skill's frontmatter is the **L3-level authored content for system prompt** (since pack is essential, L1 router body doesn't come into play). It should front-load keywords and use-cases.

Per-skill authoring targets:
- `description` (frontmatter): 200–400 chars, front-loaded keywords
- Body: 300–2000 chars depending on topic complexity
- No nested skills; flat markdown

L2 `description_router` field on each skill: authored for consistency with other packs, even though the `sm` pack is essential (defensive — if user flips pack to non-essential, router body still shows good content).

### File layout

```
crates/skills-manager-core/assets/builtin-skills/
├── pack-router-gen/              ← existing
│   └── SKILL.md
├── sm-overview/                  ← NEW
│   └── SKILL.md
├── sm-scenarios/                 ← NEW
│   └── SKILL.md
├── sm-packs/                     ← NEW
│   └── SKILL.md
├── sm-skills/                    ← NEW
│   └── SKILL.md
├── sm-authoring/                 ← NEW
│   └── SKILL.md
├── sm-debug/                     ← NEW
│   └── SKILL.md
├── sm-agents/                    ← NEW
│   └── SKILL.md
└── sm-install/                   ← NEW
    └── SKILL.md
```

## Components Changed

### `crates/skills-manager-core/assets/builtin-skills/sm-*/SKILL.md`

Eight new files. Content authored to match the patterns established in our pilot work (front-loaded keywords, clear "when to use" section, concrete CLI examples with expected output, "Related" cross-references).

### `crates/skills-manager-core/src/builtin_skills.rs`

Add 8 new directory names to the list of builtin skills the installer copies. Pattern identical to existing `pack-router-gen` handling.

### `crates/skills-manager-core/src/pack_seeder.rs`

Extend `seed_default_packs`:

1. Create `sm` pack (name, description, icon, color, `is_essential = true`).
2. Associate the 8 `sm-<name>` skills with the pack via `add_skill_to_pack`.
3. Set L1: `router_description` + `router_when_to_use` on the pack.
4. Set L2: `description_router` on each of the 8 skills (via a hardcoded mapping in the seeder).
5. Add `sm` pack to the appropriate default scenarios' `scenario_packs`.

Idempotent: skip if pack already exists.

### (Optional) `src-tauri/src/lib.rs`

Ensure `ensure_central_repo()` is called on app startup so the builtin skills + pack get installed on first run and on upgrade. This is already wired (existing behavior). No change needed unless we discover a gap.

## Content Authoring — Draft L1 and Per-Skill Highlights

These will be finalized during implementation. Below are seed drafts to anchor review.

**Pack `sm`**:
- `router_description`: "Skills Manager (sm) usage reference. Concepts (vault / scenarios / packs / agents / three-tier PD), all CLI commands, authoring L1+L2 content, per-agent config, debugging, install/upgrade."
- `router_when_to_use`: "Use when user asks about 'sm', 'skills manager', 'skill pack', 'router description', 'when_to_use', 'description_router', 'L1/L2', 'how to author', 'how to switch scenario', 'hybrid mode', 'disclosure mode', 'CLI', 'vault'."

**Per-skill `description` (frontmatter) seeds**:

- `sm-overview`: "Skills Manager concept map — vault, scenarios, packs, agents, three-tier Progressive Disclosure, disclosure modes (full / hybrid / router_only). Use when user is new to sm or asks 'what is this', 'how does sm work', 'where is X stored'."
- `sm-scenarios`: "Manage sm scenarios: list, switch active, set disclosure mode. Use for 'switch scenario', 'enable hybrid mode', 'what scenario am I on', 'create a new scenario'."
- `sm-packs`: "Manage sm packs: list, inspect, author L1 router (`description` + `when_to_use`), mark essential. Use for 'set up a pack', 'change pack router', 'mark pack essential', 'add skill to pack'."
- `sm-skills`: "Manage individual skills: set L2 `description_router` per skill, bulk YAML import. Use for 'author L2', 'write router description for skill', 'import skill descriptions', 'bulk update skills'."
- `sm-authoring`: "End-to-end L1+L2 authoring workflow: gather → draft → review → import → verify, with 分叉 (branching) differentiation theory. Use when user asks 'how do I author this pack', 'how to write L2', 'how to differentiate similar skills'."
- `sm-debug`: "Troubleshoot sm: router not rendering, trigger not hitting, stale binary, sync_mode ignored. Use for 'router isn't showing', 'trigger isn't working', 'something isn't syncing', 'sm upgrade broken'."
- `sm-agents`: "Per-agent scenario + extra packs. Use for 'give agent X a different scenario', 'add pack to agent', 'claude_code isn't loading skills', 'split agents'."
- `sm-install`: "Install, upgrade, backup, restore sm. Use for 'install sm', 'upgrade sm', 'back up my vault', 'where is my DB', 'migrate to new machine'."

## Testing

### Unit tests (Rust, `builtin_skills.rs` + `pack_seeder.rs`)

- `test_builtin_sm_skills_copied_to_vault` — verify all 8 SKILL.md files land in vault on first install
- `test_builtin_sm_skills_idempotent` — second install call doesn't duplicate or corrupt
- `test_seed_sm_pack_creates_pack_and_skills` — verify pack exists with 8 skill links + `is_essential = true`
- `test_seed_sm_pack_idempotent` — second seed call is a no-op
- `test_seed_sm_pack_sets_l1_and_l2` — `router_description`, `router_when_to_use`, per-skill `description_router` all populated

### Integration test (`crates/skills-manager-cli/tests/sm_pack.rs`)

- `sm_pack_seeded_on_fresh_install` — fresh temp HOME, run `sm list` (or similar triggering seed), verify DB has `sm` pack with 8 skills
- `sm_pack_skills_appear_in_claude_code_skills_dir_in_hybrid_mode` — switch to hybrid scenario, verify `~/.claude/skills/sm-*/SKILL.md` files exist

### Manual end-to-end acceptance

1. Fresh `cargo build`. Delete `~/.skills-manager/` (move aside; this is destructive for the tester's data — use a temp HOME or snapshot).
2. Run `sm list` to trigger setup.
3. Verify `~/.skills-manager/skills/sm-*/` directories exist (8).
4. Verify `sqlite3 ~/.skills-manager/skills-manager.db "SELECT name, is_essential FROM packs WHERE name='sm';"` returns `sm|1`.
5. Verify `sm pack context sm` shows 8 skills with `description_router` populated.
6. `sm scenario set-mode everything hybrid; sm switch claude_code everything`.
7. Verify `ls ~/.claude/skills/sm-*/SKILL.md` — 8 files present (essential pack → materialized in hybrid mode).
8. Open a new Claude Code session. Ask: "how do I add a skill to a pack?" → expect Claude to invoke `sm-packs` or `sm-skills` skill.

## Rollout

1. Author the 8 SKILL.md files (longest part of implementation).
2. Wire builtin installation + seeding.
3. Tests.
4. Manual e2e on a temp HOME (not the dev machine's real vault).
5. Update PROGRESS.md.
6. PR.
7. After merge, next session: Phase 2 is `feat/three-tier-ui` (UI for new fields, as originally planned).

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Existing installs don't re-seed on upgrade, so existing users won't get the `sm` pack | Seeder is idempotent and runs on every startup — presence check + add if missing. Handles both fresh + upgrade paths. |
| Skill content is wrong / out-of-date as `sm` CLI evolves | Skills live in the repo. Future PRs that change CLI behavior must update the corresponding skill. Consider a doc-lint step that cross-references CLI `--help` output against skill content in a follow-up. |
| 8 skills in system prompt (essential pack) adds ~2–3k tokens per session | Accept — the pack is purposefully essential so `sm` help is always available. Trade-off is intentional. Users who don't want it can toggle `is_essential=false` per scenario via `sm pack set-essential sm false`. |
| Skills overlap with existing docs (CLAUDE.md, PROGRESS.md, STATUS.md) | Skills are the single source of truth for *usage*. CLAUDE.md/PROGRESS.md/STATUS.md remain project contributor docs. Minor overlap acceptable. |
| Default scenarios (fresh install) don't include `sm` pack | Seeder adds `sm` to `everything`, `standard`, `standard-marketing`, `full-dev`, `full-dev-marketing`. Excluded from `minimal` and `core` by design. |

## Open Questions

None at design time. Implementation may surface:
- Exact token cost of the pack at runtime (measure after implementation).
- Whether the `sm-install` skill should bundle a generated install script or point to README (lean toward README reference).

## Spec Self-Review

- **Placeholders**: none. Seed L1/L2 drafts shown are starting points; the implementation task authors the full SKILL.md bodies.
- **Internal consistency**: pack config, installation mechanism, seeder changes, and testing all reference the same 8 skills and the same `sm` pack id.
- **Scope**: bounded. No code in other modules besides `builtin_skills.rs` + `pack_seeder.rs` + the SKILL.md asset files. UI deferred.
- **Ambiguity**: `is_essential = true` is the default choice, explicit. L2 authoring is a defensive measure (in case user flips pack to non-essential later), explicit.
