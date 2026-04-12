# Phase 3: Plugin Management Design Spec

## Problem

Claude Code plugins are always-on — all 11 installed plugins load every session. Users need
per-scenario control to disable heavy or irrelevant plugins.

## Mechanism

Manipulate `~/.claude/plugins/installed_plugins.json` directly:
- **Disable**: Remove plugin entry from JSON, save full entry in our DB for restore.
- **Enable**: Restore saved entry back to JSON.
- Never touch plugin cache directories.

## JSON Format

```json
{
  "version": 2,
  "plugins": {
    "plugin-key": [
      {
        "scope": "user|project|local",
        "projectPath": "/optional/path",  // only for project/local scope
        "installPath": "/path/to/cache/...",
        "version": "...",
        "installedAt": "ISO8601",
        "lastUpdated": "ISO8601",
        "gitCommitSha": "..."
      }
    ]
  }
}
```

A plugin key like `compound-engineering@compound-engineering-plugin` can have multiple scope
entries. We treat the entire array as one unit.

## DB Schema (Migration v5 -> v6)

```sql
CREATE TABLE managed_plugins (
    id TEXT PRIMARY KEY,
    plugin_key TEXT NOT NULL UNIQUE,
    display_name TEXT,
    plugin_data TEXT NOT NULL,  -- full JSON array from installed_plugins.json
    created_at INTEGER,
    updated_at INTEGER
);

CREATE TABLE scenario_plugins (
    scenario_id TEXT NOT NULL REFERENCES scenarios(id) ON DELETE CASCADE,
    plugin_id TEXT NOT NULL REFERENCES managed_plugins(id) ON DELETE CASCADE,
    enabled INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY(scenario_id, plugin_id)
);
```

## Core Module: `plugins.rs`

### Types

- `InstalledPluginsFile` — serde model for `installed_plugins.json`
- `PluginEntry` — single scope entry within a plugin
- `ManagedPluginRecord` — DB record

### Functions

1. `installed_plugins_path()` -> PathBuf
2. `read_installed_plugins()` -> Result<InstalledPluginsFile>
3. `write_installed_plugins(file: &InstalledPluginsFile)` -> Result<()>
4. `discover_plugins()` -> Result<Vec<discovered plugin info>>
5. `apply_scenario_plugins(store, scenario_id)` -> Result<()>
   - Read current `installed_plugins.json`
   - Get enabled plugin keys for scenario
   - Write back JSON with only enabled plugins
   - Disabled plugins' data is already saved in `managed_plugins.plugin_data`

### SkillStore Methods

1. `insert_managed_plugin(record)` -> Result<()>
2. `get_all_managed_plugins()` -> Result<Vec<ManagedPluginRecord>>
3. `get_managed_plugin_by_key(key)` -> Result<Option<ManagedPluginRecord>>
4. `update_managed_plugin_data(id, plugin_data)` -> Result<()>
5. `delete_managed_plugin(id)` -> Result<()>
6. `set_scenario_plugin_enabled(scenario_id, plugin_id, enabled)` -> Result<()>
7. `get_scenario_plugins(scenario_id)` -> Result<Vec<(ManagedPluginRecord, bool)>>
8. `get_enabled_plugin_keys_for_scenario(scenario_id)` -> Result<Vec<String>>

### Default Behavior

When a scenario has no `scenario_plugins` rows for a given plugin, that plugin is
considered **enabled** (backward compatible). Only explicit `enabled = 0` disables.

## Scenario Switch Integration

In `sync_scenario_skills()` and `unsync_scenario_skills()`:
- After syncing/unsyncing skills, call `apply_scenario_plugins(store, scenario_id)`.

## Tauri Commands

- `get_managed_plugins()` -> Vec<ManagedPluginRecord>
- `scan_plugins()` -> Vec<ManagedPluginRecord>  (discover + register)
- `get_scenario_plugins(scenario_id)` -> Vec<{plugin, enabled}>
- `set_scenario_plugin_enabled(scenario_id, plugin_id, enabled)` -> ()

## Implementation Order

1. Migration v5->v6 (new tables)
2. `ManagedPluginRecord` type + DB CRUD in `skill_store.rs`
3. `plugins.rs` core module (discovery, read/write JSON, apply)
4. Integration into scenario switch
5. Tauri commands
6. Tests throughout
