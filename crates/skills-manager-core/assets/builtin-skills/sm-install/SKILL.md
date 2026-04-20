---
name: sm-install
description: Install, upgrade, backup, restore Skills Manager. Fresh install path, rebuild CLI from source, DB backup/restore location, vault migration. Use for 'install sm', 'upgrade sm', 'back up my vault', 'where is my DB', 'migrate to new machine', 'something broke after upgrade'.
---

# Skills Manager — Install / Upgrade / Backup

## Fresh install (from source)

```bash
git clone https://github.com/knjf/skills-manager.git
cd skills-manager
pnpm install
cargo build -p skills-manager-cli --release

# Install binary
cp target/release/sm ~/.local/bin/sm
# Or: sudo cp target/release/sm /usr/local/bin/sm
```

First run of `sm list` creates:
- `~/.skills-manager/` (vault)
- `~/.skills-manager/skills-manager.db` (DB with seeded default packs + scenarios)
- `~/.skills-manager/skills/pack-router-gen/`, `~/.skills-manager/skills/sm-*/` (builtin helper skills)

## Install the Tauri desktop app (optional)

```bash
cargo tauri build
# Binary at: src-tauri/target/release/bundle/macos/skills-manager.app
```

Or run in dev mode:
```bash
pnpm tauri:dev
```

Desktop app operates on the same DB + vault as the CLI.

## Upgrade

Pull latest + rebuild:
```bash
git pull
cargo build -p skills-manager-cli --release
cp target/release/sm ~/.local/bin/sm
```

On next `sm` invocation:
- DB migrations auto-run (schema upgrades)
- Builtin skills (`sm-*`, `pack-router-gen`) auto-install / overwrite from embedded assets
- `sm` pack is idempotently ensured in DB if missing (fresh install and upgrade alike)

## Back up the vault

```bash
# Small enough to copy directly
tar czf ~/sm-backup-$(date +%Y%m%d).tar.gz ~/.skills-manager
```

Restore:
```bash
tar xzf ~/sm-backup-YYYYMMDD.tar.gz -C ~/
```

## Back up just the DB

```bash
cp ~/.skills-manager/skills-manager.db ~/.skills-manager/skills-manager.db.bak
# Or with sqlite
sqlite3 ~/.skills-manager/skills-manager.db ".backup ~/sm-db-$(date +%Y%m%d).db"
```

## Migrate to another machine

1. `tar czf sm.tar.gz ~/.skills-manager` on source machine.
2. Copy the tarball to target.
3. Extract: `tar xzf sm.tar.gz -C ~/`.
4. Install `sm` binary on target (build from same repo revision for compatibility).
5. Verify: `sm list` should show your scenarios.

## Clean reinstall (destructive)

```bash
# Only if you want to throw away all data
rm -rf ~/.skills-manager
sm list   # Re-creates everything with defaults
```

## Orphan recovery (vault has SKILL.md but DB doesn't know)

```bash
sm fix-orphans
```

## Related

- `sm-overview` — where data lives
- `sm-debug` — upgrade broke something
