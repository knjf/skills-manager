# Phase 2: CLI Binary Design Spec

## Goal

Replace the 222-line zsh script at `~/.local/bin/sm` with a compiled Rust binary that reuses the `skills-manager-core` crate. The binary must replicate all existing commands and add pack-aware operations.

## Commands

| Command | Aliases | Description |
|---------|---------|-------------|
| `sm list` | `sm ls` | List all scenarios with skill counts, mark active |
| `sm current` | `sm c` | Show active scenario name + skill count |
| `sm switch <name>` | `sm sw <name>` | Switch scenario: unsync old, sync new, update DB |
| `sm skills [name]` | `sm sk [name]` | List skills in a scenario (default: active) |
| `sm diff <a> <b>` | `sm d <a> <b>` | Compare two scenarios |
| `sm packs [name]` | | List packs in a scenario (default: active) |
| `sm pack add <pack> <scenario>` | | Add a pack to a scenario |
| `sm pack remove <pack> <scenario>` | | Remove a pack from a scenario |
| `sm help` | `sm --help` | Usage info (handled by clap) |

## Architecture

Single crate at `crates/skills-manager-cli/` with binary name `sm`.

```
crates/skills-manager-cli/
  Cargo.toml
  src/
    main.rs      -- clap derive structs + entry point
    commands.rs  -- command implementations
```

## Key Design Decisions

1. **Output format**: Human-readable, matching the shell script style (arrows for active, indented lists, skill counts).

2. **Sync approach**: The CLI uses the same `sync_engine` and `tool_adapters` as the Tauri app. For the CLI, we simplify: we use `enabled_installed_adapters()` (not per-skill tool toggles) since the shell script doesn't have per-skill-per-tool granularity. This keeps behavior identical to the current shell script.

3. **Pack-aware sync**: Use `get_effective_skills_for_scenario()` for all skill listings and sync operations (packs + direct skills, deduped).

4. **Unsync strategy**: For each enabled adapter, scan its skills_dir and remove entries that point to `~/.skills-manager/skills/` (symlinks) or whose name matches a known central skill (copies for Cursor). This matches the shell script behavior and avoids reliance on the `skill_targets` table which may be stale if the CLI and GUI interleave.

   **Update**: After review, using the DB `skill_targets` table is actually the right approach since:
   - The Tauri app already populates it
   - It avoids false positives from scanning
   - The CLI will also populate it during sync
   
   Fallback: if `skill_targets` is empty for the old scenario, use filesystem scanning as the shell script does.

5. **Scenario lookup**: Find scenarios by name (case-insensitive match for usability).

6. **Error handling**: anyhow for errors, human-readable messages on stderr, exit code 1 on error.

## Dependencies

- `clap` (derive) for argument parsing
- `skills-manager-core` (path dep) for all DB/sync/adapter logic
- `anyhow` for error handling

## Installation

After `cargo build --release -p skills-manager-cli`:
```
cp target/release/sm ~/.local/bin/sm
```

Or during development: `cargo run -p skills-manager-cli -- <args>`
