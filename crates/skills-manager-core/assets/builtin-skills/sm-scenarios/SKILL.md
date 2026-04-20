---
name: sm-scenarios
description: Manage sm scenarios — list, show active, switch active, set disclosure mode. Commands `sm list`, `sm current`, `sm switch`, `sm scenario set-mode`. Use when user asks 'switch scenario', 'enable hybrid mode', 'what scenario am I on', 'list scenarios', 'change disclosure mode'.
---

# Skills Manager — Scenarios

A scenario is a named bundle of packs assigned to one or more agents. Switching a scenario re-materializes the agent's skill directory.

## List scenarios

```bash
sm list
```

Output: each scenario + skill count + `>` marker for active.

## Show active

```bash
sm current
```

## Switch

Global (all managed agents):
```bash
sm switch <scenario-name>
```

Per-agent:
```bash
sm switch <agent-key> <scenario-name>
# e.g.
sm switch claude_code standard-marketing
```

Agents come from `sm agents`. Typical keys: `claude_code`, `cursor`, `codex`, `antigravity`, `gemini_cli`, `hermes`, `openclaw`.

## Set disclosure mode

```bash
sm scenario set-mode <scenario-name> <full|hybrid|router_only>
# e.g.
sm scenario set-mode standard-marketing hybrid
```

Effect: next `sm switch` syncs according to the chosen mode.

- `full` — every skill materialized directly (legacy, high token cost).
- `hybrid` — essential pack's skills materialized; non-essential packs surfaced via router SKILL.md files. Recommended.
- `router_only` — only routers, no materialized skills.

## Typical workflow

1. `sm list` — see what's available
2. `sm scenario set-mode <name> hybrid` — enable hybrid for token savings
3. `sm switch <agent> <name>` — switch that agent
4. Verify: `ls ~/.claude/skills/` (or relevant agent's skill dir)

## Related

- `sm-overview` — concept primer
- `sm-packs` — author pack-level L1 content
- `sm-agents` — per-agent scenario assignment
- `sm-debug` — when switching doesn't take effect
