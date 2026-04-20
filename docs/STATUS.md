# Skills Manager — Status Report (2026-04-21)

**For**: a new admin taking over. Written as onboarding, not a changelog.

---

## TL;DR

Skills Manager is a Tauri + Rust desktop app managing 189+ AI agent skills across 7 agent tools (Claude Code, Cursor, Codex, etc.) via a central SQLite vault at `~/.skills-manager/`. CLI binary `sm` mirrors everything the GUI does.

**Recently shipped** (last 48 hours):
1. **PD Sync Wiring** (PR #28) — `disclosure_mode` now actually works end-to-end. Previously scenario mode was stored but ignored.
2. **Three-Tier Progressive Disclosure** (PR #29) — split pack router into three authorable tiers with Claude Code's native `when_to_use` frontmatter.
3. **Pilot content authoring** (local, this session) — `research` + `gstack` packs fully authored (L1 + L2 for 55 skills). Live-verified in session.

**Outstanding**:
- 5 more packs awaiting L1+L2 content (`agent-orchestration`, `browser-tools`, `design`, `knowledge`, `ops`)
- UI for editing the new `when_to_use` / `description_router` fields (backend ready)
- Lower-priority roadmap items (Dashboard per-agent view, Tray menu per-agent switch, My Skills retirement, Cursor copy fix)

**Fully operational, in dogfood**: Claude Code on `standard-marketing` scenario in `hybrid` disclosure mode. 20 entries in `~/.claude/skills/` (down from ~127 in full mode, ~84% token savings).

---

## Core concept: Three-tier Progressive Disclosure

Claude Code scans every `SKILL.md` in `~/.claude/skills/` at session start and injects each `description` into the system prompt. With 189 skills that costs ~15–20k tokens before the user does anything. Three-tier PD solves this.

```
~/.skills-manager/skills/           ← vault (source of truth, all 189 skills)
  ├── seo-audit/SKILL.md
  ├── perp-search/SKILL.md
  └── ... (unchanged)

~/.claude/skills/                   ← hybrid-mode contents (small):
  ├── find-skills/SKILL.md          ← essentials (full sync, in system prompt)
  ├── web-access/SKILL.md
  ├── pack-marketing/SKILL.md       ← pack routers (small description + skill table)
  ├── pack-research/SKILL.md
  └── ... (one per non-essential pack)
```

### The three tiers

| Tier | What it is | Where stored | When loaded |
|---|---|---|---|
| **L1** | Pack router `description` + `when_to_use` (native Claude Code frontmatter) | `packs.router_description`, `packs.router_when_to_use` | Session start → system prompt |
| **L2** | Per-skill compressed "which-to-pick" line | `skills.description_router` | When Claude invokes a pack router (markdown table with vault paths) |
| **L3** | Full SKILL.md body (original content) | Vault file `~/.skills-manager/skills/<name>/SKILL.md` | When Claude `Read`s the vault path |

### Disclosure modes (per scenario)

- `full` — materialize every skill (legacy behavior, no routers)
- `hybrid` — essential packs' skills materialized + non-essential packs surfaced via routers (the PD sweet spot)
- `router_only` — only routers, nothing materialized

Current production: `standard-marketing` scenario is `hybrid`. Others are `full`.

---

## How to use — CLI cheat sheet

All commands via `sm` at `/Users/jfkn/projects/skills-manager/target/debug/sm` (not yet reinstalled to `~/.local/bin/sm` — that's v0.1.0 and predates PR #28. Rebuild + replace when ready).

### Scenarios

```bash
sm list                          # All scenarios
sm current                       # Active scenario
sm switch <scenario>             # Switch all managed agents
sm switch <agent> <scenario>     # Switch one agent only
sm scenario set-mode <name> <full|hybrid|router_only>   # NEW in PR #28
```

### Packs

```bash
sm packs                         # List packs in active scenario
sm pack context <name>           # Show pack metadata + skill list
sm pack list-routers             # Router status per pack
sm pack set-router <name> \      # Author L1 for a pack
  --description "<domain sentence>" \
  --when-to-use "<trigger phrase list>"    # NEW in PR #29
sm pack set-essential <name> <true|false>  # Hybrid-mode inclusion
```

### Skills (NEW in PR #29)

```bash
sm skill set-router-desc <name> --description "<L2 line>"     # Single
sm skill set-router-desc <name> --clear                       # Clear
sm skill import-router-descs <file.yaml>                      # Bulk
```

YAML format (for bulk):

```yaml
skill-name-1: "Short L2 line"
skill-name-2: "Another L2 line"
unknown-skill: "ignored, counted as skipped"
```

### Agents

```bash
sm agents
sm agent info <agent>
sm agent add-pack <agent> <pack>        # Extra pack on top of scenario
sm agent remove-pack <agent> <pack>
```

---

## Verification checklist (how to know it's working)

1. **DB sanity**
   ```bash
   sqlite3 ~/.skills-manager/skills-manager.db \
     "SELECT name, disclosure_mode FROM scenarios;"
   ```
   → each scenario with its mode.

2. **Router content** (after setting L1 + L2 + syncing to hybrid)
   ```bash
   cat ~/.claude/skills/pack-marketing/SKILL.md
   ```
   → expect YAML frontmatter with `description:` AND `when_to_use:`, plus a markdown table with skill names + vault paths.

3. **Live reload**
   After `sm switch`, open a new Claude Code session. The system reminder at session start should list `pack-*` entries with the authored descriptions. If a pack has no L1 set, you'll see "Router for pack — description pending generation" — this is the placeholder.

4. **Test suite**
   ```bash
   cargo test --workspace
   ```
   → 277 tests should pass. If anything fails on main, that's a bug — file it.

---

## What's authored (pilot, 2026-04-21)

| Pack | L1 `description` | L1 `when_to_use` | L2 (skill lines) |
|---|---|---|---|
| `marketing` | ✅ (prior session) | ✅ (prior session) | 1/4 (`prd` only) |
| `research` | ✅ | ✅ | **10/10** |
| `gstack` | ✅ | ✅ | **45/45** |
| `agent-orchestration` | ❌ | ❌ | 0/7 |
| `browser-tools` | ❌ | ❌ | 0/7 |
| `design` | ❌ | ❌ | 0/10 |
| `knowledge` | ❌ | ❌ | 0/9 |
| `ops` | ❌ | ❌ | 0/4 |
| `base` | N/A (essential pack, no router) | N/A | 0/13 (not needed) |

Total authored: 55 of ~90 non-essential skills. 5 packs awaiting.

---

## What's not done (prioritized)

### P1 — Scale the pilot

Apply the same authoring pattern to the remaining 5 packs. Workflow:

1. `sm pack context <name>` — gather skill list
2. For skills with empty DB descriptions: `Read` their SKILL.md directly (find via `find ~/.skills-manager/skills -name SKILL.md`)
3. Draft L1 (`description` + `when_to_use`) + L2 (per-skill line, variable sizing 20–150 chars, emphasize "分叉" — i.e. contrast similar skills)
4. Write one YAML, one `sm pack set-router` per pack, one `sm skill import-router-descs`
5. Switch to hybrid and verify

Target budget per pack body: ~2000 tokens (all 7 packs well under).

Estimated effort: 2–3 hours per pack if done carefully with human review. Bulk-mode less.

### P2 — UI for new fields (Sub-project B)

Backend (DB columns + IPC DTOs) is ready. Frontend needs:
- `PacksView`: edit `when_to_use` per pack (textarea)
- `MySkills`: edit `description_router` per skill
- Bulk import YAML via a drag-drop / file picker
- Show which skills have L2 authored vs unset (badge)

Code pointers:
- `src/views/PacksView.tsx` — existing pack editor
- `src/views/MySkills.tsx` — existing skill editor
- `src-tauri/src/commands/skills.rs` (ManagedSkillDto) — DTO may need `description_router` field added for serialization
- `src-tauri/src/commands/scenarios.rs` (PackDto) — same for `router_when_to_use`

Effort: 1 day with tests.

### P3 — Auto-classification + auto L1/L2 generation (Sub-project C)

Deferred on purpose. Without good ground truth (lots of authored examples), auto-gen becomes self-echoing. Revisit after P1 gives us ~190 authored L2 lines to use as reference.

Infrastructure partially exists: `pack-router-gen` builtin skill, `sm pack gen-router` markers. Extend to also queue L2 suggestions. Human-in-the-loop review required.

### P4 — Lower-priority roadmap

- **Dashboard per-agent view** — current Dashboard shows global scenario; doesn't reflect per-agent assignment (after #9 merged).
- **Tray menu per-agent switch** — similar.
- **My Skills retirement** — low priority, evaluate after new pages validated.
- **Cursor copy_dir_recursive edge cases** — low priority.

See `PROGRESS.md` for the full list.

---

## Known gotchas

1. **`~/.local/bin/sm` is stale** (v0.1.0, from Apr 15). It predates PR #28 + #29 and lacks `scenario set-mode`, `pack set-essential`, `skill set-router-desc`, `skill import-router-descs`, `pack set-router --when-to-use`. To install fresh:
   ```bash
   cargo build -p skills-manager-cli --release
   cp target/release/sm ~/.local/bin/sm
   ```
   (Adjust to your install convention.)

2. **`sync_mode` setting is ignored by the new reconcile path.** The `Copy` option in Settings UI is a no-op during scenario sync — `reconcile_agent_dir` always uses `Symlink`. Documented tradeoff in PR #28. If someone needs copy mode for a specific scenario, the current code won't honor it. Separate fix.

3. **Dev server on main** may hot-reload during manual tests. If you edit DB while `cargo tauri dev` is running, the app sees changes but may not re-render immediately. Restart if confused.

4. **`fix-orphans`** imports skills the vault has but the DB doesn't know about. Useful after manual vault edits. Run it if `sm packs` shows weird counts.

5. **gstack skills** live at `~/.skills-manager/skills/gstack/.cursor/skills/gstack-<name>/SKILL.md` — not the flat `~/.skills-manager/skills/<name>/`. When authoring L2, read from the cursor path, not the vault placeholder.

---

## Test coverage

- **Workspace total**: 277 tests, 0 failures as of 2026-04-21
- **Core**: 249 unit tests (schema, sync engine, router render, store, disclosure)
- **CLI**: 8 integration tests (`tests/pd_wiring.rs`) — exercise full `sm` binary against temp HOME
- **Tauri**: 6 app-lib tests
- **Workspace**: 4 other

Run: `cargo test --workspace --no-fail-fast`

Both PRs used TDD via `superpowers:subagent-driven-development` — each task got spec-compliance review + code-quality review before merge.

---

## Recommended next action for the new admin

1. **Dogfood the pilot for a day.** Current scenario is `standard-marketing` hybrid with `pack-research` and `pack-gstack` authored. Use Claude Code normally. Watch for:
   - When you want a research / gstack command, does Claude invoke the right router?
   - Does Claude correctly pick the right skill from the router body (L2)?
   - Any misfires where the router description led Claude wrong? (Note these — they improve L1 authoring.)

2. **If pilot feels solid, scale up P1** — draft the remaining 5 packs, same workflow. Pilot content is the reference pattern for "分叉" style.

3. **If pilot feels shaky, iterate on L1/L2 language.** Tune `sm pack set-router` / `sm skill set-router-desc` commands and re-switch. Live-reload means changes take effect next session.

4. **Long-term**: P2 (UI) is user-facing convenience. P3 (automation) is force-multiplier once P1 is done.
