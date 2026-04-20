---
name: sm-agents
description: Per-agent configuration in Skills Manager — list managed agents, inspect one agent's skill breakdown, assign different scenarios per agent, attach extra packs. Commands `sm agents`, `sm agent info`, `sm agent add-pack`, `sm agent remove-pack`. Use for 'give agent X a different scenario', 'add pack to agent', 'claude_code isn't loading skills', 'split agents across scenarios'.
---

# Skills Manager — Agents

Each agent (Claude Code, Cursor, Codex, etc.) can be on its own scenario, with optional extra packs layered on top.

## List agents

```bash
sm agents
```

Output: each managed agent + assigned scenario + pack count.

## Inspect one agent

```bash
sm agent info <agent-key>
```

Shows the agent's scenario, all packs (from scenario + extras), and skill breakdown.

Agent keys you'll see:
- `claude_code` — Claude Code (`~/.claude/skills/`)
- `cursor` — Cursor (`~/.cursor/skills/`)
- `codex` — OpenAI Codex CLI (`~/.codex/skills/`)
- `antigravity` — Google Antigravity
- `gemini_cli` — Gemini CLI
- `hermes` — Anthropic Hermes
- `openclaw` — OpenClaw

Only installed agents are managed.

## Assign a different scenario per agent

```bash
sm switch <agent-key> <scenario-name>
# e.g.
sm switch cursor minimal
sm switch claude_code standard-marketing
```

This lets claude_code have full marketing access while cursor stays minimal for focused coding.

## Attach an extra pack to one agent (on top of the scenario)

```bash
sm agent add-pack <agent-key> <pack-name>
# e.g.
sm agent add-pack codex sm   # give codex the sm pack even though its scenario doesn't include it
```

Remove:
```bash
sm agent remove-pack <agent-key> <pack-name>
```

## When an agent isn't loading skills

Check:

1. `sm agents` — is the agent listed as managed?
2. `sm agent info <key>` — does it have a scenario + packs?
3. `ls <agent-skills-dir>` — did the last sync populate the dir? Expected paths:
   - `~/.claude/skills/`
   - `~/.cursor/skills/`
   - `~/.codex/skills/`
4. `sm switch <key> <scenario>` — force re-sync.

See `sm-debug` for deeper troubleshooting.

## Related

- `sm-scenarios` — scenario management
- `sm-packs` — pack-level config
- `sm-debug` — agent sync troubleshooting
