---
name: sm-skills
description: Manage individual skills — author L2 `description_router` (per-skill "which to pick" line) via single edit or bulk YAML import. Commands `sm skill set-router-desc`, `sm skill import-router-descs`. Use for 'author L2', 'write router description for skill', 'import skill descriptions from YAML', 'bulk update skills'.
---

# Skills Manager — Skills

L2 (`description_router`) is a per-skill short line that appears in the pack router body's skill table. It tells Claude *which* skill to pick once it has routed into the pack.

## Set a single skill's L2

```bash
sm skill set-router-desc <skill-name> --description "<L2 line>"
```

Example:
```bash
sm skill set-router-desc perp-search \
  --description "Perplexity-style single synthesized answer. → Pick for 'just give me an answer with citations'."
```

Clear:
```bash
sm skill set-router-desc <skill-name> --clear
```

## Bulk import via YAML

Write a YAML file:

```yaml
# l2.yaml
perp-search: "Perplexity-style single synthesized answer."
autoresearch: "Autonomous multi-agent loop → cited report."
# missing-skill: reported as skipped, doesn't abort
another-skill: "..."
```

Import:
```bash
sm skill import-router-descs /path/to/l2.yaml
# Output: "Updated N skill(s), skipped M unknown name(s)."
```

Transaction-safe: all updates commit or none do. Unknown skill names are warnings, not errors.

## Writing good L2 lines

- **Target ~60–120 chars** for typical skills; go shorter for self-explanatory skills, longer (up to ~150 chars) for skills in a branching cluster that need differentiation.
- **"→ Pick for ..." markers** clarify intent when multiple similar skills exist. Example:
  - `perp-search: "Perplexity single answer. → Pick for 'give me an answer with citations'."`
  - `autoresearch: "Autonomous multi-agent loop → report. → Pick for hands-off deep work."`
- **Empty string or whitespace** counts as unset — renderer falls back to first sentence of the skill's original `description`.

## Related

- `sm-packs` — the pack that contains the skill (authored L1 affects how users arrive at the pack)
- `sm-authoring` — end-to-end workflow including how to differentiate similar skills
- `sm-debug` — when L2 isn't showing up
