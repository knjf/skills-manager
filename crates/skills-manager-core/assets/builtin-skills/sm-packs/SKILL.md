---
name: sm-packs
description: Manage sm packs — list packs in scenario, inspect pack contents, author L1 router (description + when_to_use), mark pack essential. Commands `sm packs`, `sm pack context`, `sm pack set-router`, `sm pack set-essential`. Use for 'set up a pack', 'change pack router description', 'mark pack essential', 'what's in pack X', 'add when_to_use to pack'.
---

# Skills Manager — Packs

A pack groups related skills under a theme. Each pack has L1 metadata (`router_description` + `router_when_to_use`) that goes into the system prompt in hybrid mode.

## List packs

In active scenario:
```bash
sm packs
```

In a specific scenario:
```bash
sm packs <scenario-name>
```

## Inspect a pack

```bash
sm pack context <pack-name>
```

Shows pack metadata + full list of skills with their DB descriptions.

## List routers (L1 status)

```bash
sm pack list-routers
```

Shows which packs have `router_description` authored vs `<not generated>`.

## Author L1 — description + when_to_use

```bash
sm pack set-router <pack-name> \
  --description "<domain summary>" \
  --when-to-use "<trigger phrase list>"
```

Example:
```bash
sm pack set-router marketing \
  --description "Marketing + CRO + SEO + copy + PRD + internal comms + docs." \
  --when-to-use "Use when user says 'marketing', 'SEO', 'CRO', 'PRD', 'internal comms', 'documentation'."
```

Both fields combined cap at 1536 chars (Claude Code native frontmatter limit).

Clear a field:
```bash
sm pack set-router <pack-name> --clear-when-to-use
```

## Author pack body (optional, overrides auto-rendered skill table)

```bash
sm pack set-router <pack-name> --body <file-path>
```

## Mark a pack essential

Essential packs are materialized directly in hybrid mode (not behind a router).

```bash
sm pack set-essential <pack-name> true
sm pack set-essential <pack-name> false
```

## Add / remove skill ↔ pack

```bash
sm pack add <pack-name> <skill-name>
sm pack remove <pack-name> <skill-name>
```

## Related

- `sm-skills` — per-skill L2 authoring
- `sm-authoring` — end-to-end workflow
- `sm-scenarios` — which scenarios include this pack
