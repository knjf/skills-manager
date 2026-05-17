# Skills Manager — Claude Code Plugin

Packages the eight `sm-*` meta-skills (`sm-overview`, `sm-packs`, `sm-skills`, `sm-agents`, `sm-scenarios`, `sm-authoring`, `sm-install`, `sm-debug`) as a Claude Code plugin so they are loaded **only** in projects where you opt in. A lightweight `sm-router` skill stays in the global `~/.claude/skills/` directory as a permanent entry point — it advertises the toolkit and offers first-aid checks before the user enables the plugin.

The `sm` Rust CLI in `~/.local/bin/sm` is unaffected — it remains callable from every project regardless of plugin state.

## Layout

```
claude-plugin/
├── .claude-plugin/plugin.json     # name=sm, version=0.1.0
├── skills/                        # symlinks into ~/.skills-manager/skills/
│   ├── sm-overview/
│   ├── sm-packs/
│   ├── sm-skills/
│   ├── sm-agents/
│   ├── sm-scenarios/
│   ├── sm-authoring/
│   ├── sm-install/
│   └── sm-debug/
└── README.md
```

The `skills/` entries are symlinks back to the vault (`~/.skills-manager/skills/sm-*`), so the vault remains the single source of truth — edits via `sm skill set-router …` propagate immediately.

## marketplace.json gotchas

Two non-obvious rules from the Claude Code marketplace schema (`https://json.schemastore.org/claude-code-marketplace.json`):

1. `plugins[].source` accepts **either** a relative path string (`"./sm-plugin"`) **or** an object whose discriminator is `source` (not `type`) — valid values: `npm`, `url`, `github`, `git-subdir`. There is **no `local` object form** and absolute paths are rejected.
2. Relative paths are resolved against the marketplace **directory** (e.g. `~/.claude/marketplaces/sm-local/`), not the `.claude-plugin/marketplace.json` file. So `"./sm-plugin"` resolves to `~/.claude/marketplaces/sm-local/sm-plugin`, which is the symlink to the in-repo `claude-plugin/`.

## One-time install

```bash
# Add the local marketplace (catalog at ~/.claude/marketplaces/sm-local/.claude-plugin/marketplace.json)
claude plugin marketplace add ~/.claude/marketplaces/sm-local/

# Install the plugin (defaults to user scope, enabled globally)
claude plugin install sm@sm-local

# Disable globally so it is off by default everywhere
claude plugin disable sm@sm-local --scope user
```

Restart Claude Code after install so the marketplace + skill listing refreshes.

## Per-project enable

```bash
# From inside the project you want sm meta-skills in:
claude plugin enable sm@sm-local --scope project
# Restart the Claude Code session in that project.
```

Verify with `/context` — the seven `sm-*` skills should appear. Disable again with `claude plugin disable sm@sm-local --scope project`.

## What stays global

- `~/.local/bin/sm` — the Rust CLI (always available)
- `~/.skills-manager/` — the vault (DB + skill content, single source of truth)
- `~/.claude/skills/sm-router/` — single global skill that advertises the toolkit and runs first-aid checks before pointing the user at the plugin

## Migration helper

Run `sm migrate-to-plugin` from the repo root for an idempotent dry-run, then `sm migrate-to-plugin --apply` to execute. The command:

1. Creates `~/.claude/marketplaces/sm-local/.claude-plugin/marketplace.json` (if missing)
2. Symlinks `~/.claude/marketplaces/sm-local/sm-plugin -> <repo>/claude-plugin` (if missing)
3. Moves any pre-existing `~/.claude/skills/sm-*` symlinks (except `sm-debug`) into `~/.claude/skills/_backup-YYYYMMDD/`

After it finishes, run the printed `claude plugin marketplace add` / `install` / `disable --scope user` commands. Re-running the migration is safe — it skips already-completed steps.
