---
name: sm-overview
description: Skills Manager (sm) concept reference — vault, scenarios, packs, agents, three-tier Progressive Disclosure, disclosure modes. Use when user is new to sm or asks 'what is sm', 'how does this work', 'where is X stored', 'what's a scenario', 'what's a pack', 'what's L1/L2/L3'.
---

# Skills Manager — Overview

Skills Manager (`sm`) manages AI agent skills across tools (Claude Code, Cursor, Codex, etc.) from a central SQLite-backed vault. The CLI is `sm`; a Tauri desktop GUI exposes the same operations.

## Key concepts

**Vault** (`~/.skills-manager/`): single source of truth.
- `skills/` — every skill directory (L3 content), plus builtin helpers (sm-*, pack-router-gen).
- `skills-manager.db` — SQLite DB (packs, scenarios, agent configs, disclosure modes, L1/L2 content).

**Scenario**: a named bundle of packs. Each agent can be on a different scenario. Examples: `minimal`, `standard`, `everything`.

**Pack**: a themed collection of skills. Examples: `gstack`, `marketing`, `research`, `sm`.

**Agent**: a tool that runs Claude (`claude_code`, `cursor`, `codex`, etc.). Each agent has a scenario assignment plus optional extra packs.

**Three-tier Progressive Disclosure** (reduces system-prompt token cost):
- **L1** — pack `router_description` + `when_to_use` frontmatter, loaded into system prompt. Claude uses this to pick which pack to invoke.
- **L2** — per-skill `description_router` in the pack router body. Claude uses this AFTER invoking the pack, to pick a specific skill.
- **L3** — actual `SKILL.md` body, loaded when Claude `Read`s the vault path.

**Disclosure modes** (per scenario):
- `full` — materialize every skill directly (no routers). Legacy, high token cost.
- `hybrid` — essential-pack skills materialized; non-essential packs surfaced via routers. Default.
- `router_only` — only routers, nothing materialized.

## Where data lives

- `~/.skills-manager/skills-manager.db` — main DB
- `~/.skills-manager/skills/<name>/SKILL.md` — skill content (L3)
- `~/.skills-manager/skills/sm-*/` — builtin `sm` pack skills
- `~/.claude/skills/` — materialized view for claude_code (symlinks + auto-generated `pack-*` router dirs)
- `~/.cursor/skills/`, `~/.codex/skills/`, etc. — other agents' views

## Next steps

- Switch scenarios → see `sm-scenarios`
- Author a pack's L1 → see `sm-packs`
- Author a skill's L2 → see `sm-skills`
- End-to-end authoring workflow → see `sm-authoring`
- Something broken → see `sm-debug`
- Install / upgrade / backup → see `sm-install`
- Per-agent config → see `sm-agents`
