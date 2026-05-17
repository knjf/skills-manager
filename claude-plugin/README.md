# Skills Manager — Claude Code Plugin

Packages the 7 `sm-*` meta-skills (`sm-overview`, `sm-packs`, `sm-skills`, `sm-agents`, `sm-scenarios`, `sm-authoring`, `sm-install`) as a Claude Code plugin so they are loaded **only** in projects where you opt in. `sm-debug` is intentionally kept in the global `~/.claude/skills/` directory with `disable-model-invocation: true` so `/sm-debug` is always available without spending context tokens.

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
│   └── sm-install/
└── README.md
```

The `skills/` entries are symlinks back to the vault (`~/.skills-manager/skills/sm-*`), so the vault remains the single source of truth — edits via `sm skill set-router …` propagate immediately.

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
- `~/.claude/skills/sm-debug/` — emergency debug skill, `disable-model-invocation: true`, manual `/sm-debug` only

## Migration helper

A future `sm migrate-to-plugin` subcommand will automate marketplace creation + global cleanup. For now, follow the manual steps in this README.
