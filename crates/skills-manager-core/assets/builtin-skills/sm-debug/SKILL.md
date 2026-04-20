---
name: sm-debug
description: Troubleshoot Skills Manager — router not rendering, trigger not hitting, stale CLI binary, sync_mode ignored, missing L2 in table. Verification checklist. Use for 'router isn't showing', 'trigger isn't working', 'something isn't syncing', 'sm command not found', 'my pack doesn't appear'.
---

# Skills Manager — Debug

Common failure modes and verification steps.

## `sm` command does nothing / wrong output

Check which `sm` binary is on PATH:

```bash
which sm
sm --version
```

If version < what you expected, the installed binary is stale. Rebuild:

```bash
cd <skills-manager-repo>
cargo build -p skills-manager-cli --release
cp target/release/sm ~/.local/bin/sm  # or your install location
```

## Router SKILL.md not appearing in `~/.claude/skills/`

Check scenario disclosure mode:

```bash
sqlite3 ~/.skills-manager/skills-manager.db \
  "SELECT name, disclosure_mode FROM scenarios;"
```

Router dirs only appear when scenario is `hybrid` or `router_only`. In `full`, skills are materialized directly (no routers).

Check active scenario + re-sync:

```bash
sm current
sm switch claude_code <scenario>   # force re-sync
ls ~/.claude/skills/ | grep '^pack-'
```

## Trigger isn't hitting — Claude doesn't invoke the pack

Check what's in the router's frontmatter:

```bash
head -5 ~/.claude/skills/pack-<name>/SKILL.md
```

Expect: both `description:` and `when_to_use:` populated. If missing, author them:

```bash
sm pack set-router <name> --description "..." --when-to-use "..."
sm switch claude_code <scenario>   # re-sync
```

Open a FRESH Claude Code session. Claude Code scans skills at session start; in-session sessions won't re-load the router frontmatter from system prompt (live-reload of the SKILL.md body does work when Claude re-invokes the Skill tool, but initial trigger description is cached for the session).

## L2 row shows original vendor description instead of authored L2

Check if `description_router` is set in DB:

```bash
sqlite3 ~/.skills-manager/skills-manager.db \
  "SELECT name, description_router FROM skills WHERE name='<skill>';"
```

If NULL → L2 not authored. Author it:

```bash
sm skill set-router-desc <skill> --description "..."
sm switch claude_code <scenario>   # re-render router
```

If set but the router body still shows old text, the router file may be stale. Force resync:

```bash
sm switch claude_code <any-other-scenario>
sm switch claude_code <target-scenario>
```

## Orphan skills (vault has SKILL.md but DB doesn't know)

```bash
sm fix-orphans
```

Imports orphan skills from vault to DB so they can be assigned to packs.

## `sync_mode` setting (copy vs symlink) is ignored

Known issue: the three-tier PD sync engine (`reconcile_agent_dir`) always uses symlinks regardless of the `sync_mode` user setting. The Copy option in Settings UI is currently a no-op during scenario sync. If you need true copies, use the legacy per-skill sync path (not exposed by default) or file an issue.

## Full verification checklist

```bash
# 1. DB has expected scenarios + disclosure modes
sqlite3 ~/.skills-manager/skills-manager.db \
  "SELECT name, disclosure_mode FROM scenarios;"

# 2. Pack has L1 + L2 content
sqlite3 ~/.skills-manager/skills-manager.db \
  "SELECT name, router_description IS NOT NULL AS has_l1,
          router_when_to_use IS NOT NULL AS has_wtu
   FROM packs WHERE name='<pack>';"

sqlite3 ~/.skills-manager/skills-manager.db \
  "SELECT name, description_router IS NOT NULL AS has_l2 FROM skills
   WHERE id IN (SELECT skill_id FROM pack_skills
                WHERE pack_id=(SELECT id FROM packs WHERE name='<pack>'));"

# 3. Agent dir populated
ls ~/.claude/skills/ | grep '^pack-'
cat ~/.claude/skills/pack-<name>/SKILL.md

# 4. Tests pass
cargo test --workspace --no-fail-fast
```

## Related

- `sm-overview` — concepts behind the data model
- `sm-scenarios` — switching and mode changes
- `sm-install` — fresh install / upgrade path
