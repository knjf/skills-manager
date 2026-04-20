---
name: sm-authoring
description: End-to-end L1 + L2 authoring workflow for a pack — gather context, draft, review, bulk import, verify. Includes the "分叉" (branching differentiation) theory for similar skills. Use when user asks 'how do I author this pack', 'how to write L2', 'how to differentiate similar skills', 'onboarding a new pack'.
---

# Skills Manager — Pack Authoring Workflow

End-to-end recipe for populating a pack's L1 + L2 content.

## 0. Understand the goal

- **L1** (pack level) = pack `router_description` + `router_when_to_use`. Shown in system prompt. Claude uses this to pick the pack.
- **L2** (per-skill) = `description_router`. Shown in pack router body. Claude uses this to pick a skill within the pack.

## 1. Gather context

```bash
sm pack context <pack-name>
```

For skills with empty DB `description`, read the actual SKILL.md:
```bash
find ~/.skills-manager/skills -path "*<skill-name>*/SKILL.md"
cat <path>  # read first ~30 lines to see frontmatter + intro
```

Some skills live in nested dirs (e.g. `~/.skills-manager/skills/gstack/.cursor/skills/gstack-browse/SKILL.md`). Search broadly.

## 2. Draft L1 + L2 in one YAML file

Structure:

```yaml
# pack-name: <name>
# Pack L1 (commit via CLI after review)
pack_description: "<2–3 sentences covering the pack's capabilities>"
pack_when_to_use: "Use when user says '<trigger>', '<trigger>', ..."

# Per-skill L2 (import via sm skill import-router-descs)
skill-name-1: "<60–120 char line — what it does + → Pick for ...>"
skill-name-2: "..."
```

## 3. Differentiate similar skills ("分叉")

If two or more skills in the pack look alike, use **→ Pick for ...** markers to force distinction:

```yaml
perp-search: "Perplexity-style single synthesized answer. → Pick for 'give me an answer with citations'."
autoresearch: "Autonomous multi-agent loop → cited report. → Pick for hands-off deep work."
codex-deep-search: "Codex CLI that follows links + cross-refs. → Pick when agent-reach returned too little."
```

Variable sizing:
- Self-evident skill → 20–40 chars (`"Readwise library via local CLI."`)
- Typical → 60–100 chars
- Branching cluster → up to ~150 chars to fit the → Pick clause

## 4. Show the draft, get user review

Before committing anything, display the YAML in conversation and ask user to approve or edit. Critical: authoring happens once; fixing later requires re-running commands.

## 5. Commit L1 + L2

```bash
sm pack set-router <pack-name> \
  --description "<pack_description from YAML>" \
  --when-to-use "<pack_when_to_use from YAML>"

sm skill import-router-descs /path/to/l2.yaml
```

Expected output: `Updated N skill(s), skipped 0 unknown name(s).`

## 6. Verify

```bash
# Switch to a scenario that includes the pack in hybrid mode
sm scenario set-mode <scenario> hybrid
sm switch claude_code <scenario>

# Read the rendered router
cat ~/.claude/skills/pack-<pack-name>/SKILL.md
```

Expect:
- Frontmatter has both `description:` and `when_to_use:`
- Table column uses authored L2 lines (not vendor original descriptions)

## 7. Live-test

Open a NEW Claude Code session in any project. Ask a question that should route through the pack. Observe whether Claude invokes the pack and picks the correct skill.

## Related

- `sm-packs` — set-router command details
- `sm-skills` — set-router-desc + import-router-descs command details
- `sm-debug` — troubleshooting
