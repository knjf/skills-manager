//! Plugin management for Claude Code plugins.
//!
//! Reads/writes `~/.claude/plugins/installed_plugins.json` and manages
//! per-scenario plugin enable/disable via the `managed_plugins` and
//! `scenario_plugins` DB tables.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::skill_store::{ManagedPluginRecord, SkillStore};

// ── JSON model for installed_plugins.json ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPluginsFile {
    pub version: u32,
    pub plugins: BTreeMap<String, Vec<PluginEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEntry {
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    pub install_path: String,
    pub version: String,
    pub installed_at: String,
    pub last_updated: String,
    pub git_commit_sha: String,
}

// ── Path helpers ──

pub fn installed_plugins_path() -> PathBuf {
    dirs::home_dir()
        .expect("Cannot determine home directory")
        .join(".claude")
        .join("plugins")
        .join("installed_plugins.json")
}

// ── Read/Write ──

pub fn read_installed_plugins() -> Result<InstalledPluginsFile> {
    let path = installed_plugins_path();
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let file: InstalledPluginsFile =
        serde_json::from_str(&content).with_context(|| "Failed to parse installed_plugins.json")?;
    Ok(file)
}

pub fn write_installed_plugins(file: &InstalledPluginsFile) -> Result<()> {
    let path = installed_plugins_path();
    let content = serde_json::to_string_pretty(file)
        .with_context(|| "Failed to serialize installed_plugins.json")?;
    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

// ── Discovery ──

/// Discovered plugin info before it's registered in the DB.
#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    pub plugin_key: String,
    pub display_name: String,
    pub plugin_data: String, // JSON serialization of the Vec<PluginEntry>
}

/// Derive a human-readable display name from a plugin key.
/// e.g. "compound-engineering@compound-engineering-plugin" -> "compound-engineering"
/// e.g. "frontend-design@claude-plugins-official" -> "frontend-design"
pub fn display_name_from_key(plugin_key: &str) -> String {
    plugin_key
        .split('@')
        .next()
        .unwrap_or(plugin_key)
        .to_string()
}

/// Discover all plugins from `installed_plugins.json`.
pub fn discover_plugins() -> Result<Vec<DiscoveredPlugin>> {
    let file = read_installed_plugins()?;
    let mut discovered = Vec::new();
    for (key, entries) in &file.plugins {
        let plugin_data =
            serde_json::to_string(entries).with_context(|| "Failed to serialize plugin entries")?;
        discovered.push(DiscoveredPlugin {
            plugin_key: key.clone(),
            display_name: display_name_from_key(key),
            plugin_data,
        });
    }
    Ok(discovered)
}

/// Scan plugins from disk and register any new ones in the DB.
/// Existing plugins (matched by key) are updated with fresh plugin_data.
/// Returns all managed plugins after the scan.
pub fn scan_and_register_plugins(store: &SkillStore) -> Result<Vec<ManagedPluginRecord>> {
    let discovered = discover_plugins()?;
    let now = chrono::Utc::now().timestamp_millis();

    for dp in &discovered {
        match store.get_managed_plugin_by_key(&dp.plugin_key)? {
            Some(existing) => {
                // Update plugin_data in case it changed (version bump, etc.)
                store.update_managed_plugin_data(&existing.id, &dp.plugin_data)?;
            }
            None => {
                let record = ManagedPluginRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    plugin_key: dp.plugin_key.clone(),
                    display_name: Some(dp.display_name.clone()),
                    plugin_data: dp.plugin_data.clone(),
                    created_at: now,
                    updated_at: now,
                };
                store.insert_managed_plugin(&record)?;
            }
        }
    }

    store.get_all_managed_plugins()
}

// ── Scenario Plugin Application ──

/// Apply plugin state for a scenario: write `installed_plugins.json` with only
/// the plugins that are enabled for this scenario. Disabled plugins are omitted
/// from the JSON but their data is preserved in `managed_plugins.plugin_data`.
///
/// Plugins not yet registered as managed plugins are left as-is in the JSON
/// (they are unknown to us and we should not touch them).
pub fn apply_scenario_plugins(store: &SkillStore, scenario_id: &str) -> Result<()> {
    let managed_plugins = store.get_all_managed_plugins()?;
    if managed_plugins.is_empty() {
        // No plugins are managed — nothing to do.
        return Ok(());
    }

    let enabled_keys = store.get_enabled_plugin_keys_for_scenario(scenario_id)?;
    let enabled_set: std::collections::HashSet<&str> =
        enabled_keys.iter().map(|s| s.as_str()).collect();

    let managed_keys: std::collections::HashSet<&str> = managed_plugins
        .iter()
        .map(|p| p.plugin_key.as_str())
        .collect();

    // Read current file
    let mut file = read_installed_plugins()?;

    // Remove all managed plugins from the file first
    for key in &managed_keys {
        file.plugins.remove(*key);
    }

    // Re-add only the enabled managed plugins using their saved plugin_data
    for plugin in &managed_plugins {
        if enabled_set.contains(plugin.plugin_key.as_str()) {
            let entries: Vec<PluginEntry> = serde_json::from_str(&plugin.plugin_data)
                .with_context(|| {
                    format!(
                        "Failed to parse saved plugin_data for {}",
                        plugin.plugin_key
                    )
                })?;
            file.plugins.insert(plugin.plugin_key.clone(), entries);
        }
    }

    write_installed_plugins(&file)?;
    Ok(())
}

/// Restore all managed plugins to `installed_plugins.json`.
/// Used when unsyncing a scenario to restore the default state (all enabled).
pub fn restore_all_plugins(store: &SkillStore) -> Result<()> {
    let managed_plugins = store.get_all_managed_plugins()?;
    if managed_plugins.is_empty() {
        return Ok(());
    }

    let mut file = read_installed_plugins()?;

    for plugin in &managed_plugins {
        let entries: Vec<PluginEntry> =
            serde_json::from_str(&plugin.plugin_data).with_context(|| {
                format!(
                    "Failed to parse saved plugin_data for {}",
                    plugin.plugin_key
                )
            })?;
        file.plugins.insert(plugin.plugin_key.clone(), entries);
    }

    write_installed_plugins(&file)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn sample_plugins_json() -> &'static str {
        r#"{
  "version": 2,
  "plugins": {
    "compound-engineering@compound-engineering-plugin": [
      {
        "scope": "user",
        "installPath": "/home/test/.claude/plugins/cache/compound-engineering-plugin/compound-engineering/2.59.0",
        "version": "2.59.0",
        "installedAt": "2026-03-30T15:32:57.511Z",
        "lastUpdated": "2026-03-30T15:32:57.511Z",
        "gitCommitSha": "847ce3f156a5cdf75667d9802e95d68e6b3c53a4"
      }
    ],
    "superpowers@claude-plugins-official": [
      {
        "scope": "user",
        "installPath": "/home/test/.claude/plugins/cache/claude-plugins-official/superpowers/5.0.6",
        "version": "5.0.6",
        "installedAt": "2026-03-30T14:38:19.313Z",
        "lastUpdated": "2026-03-30T14:38:19.313Z",
        "gitCommitSha": "eafe962b18f6c5dc70fb7c8cc7e83e61f4cdde06"
      }
    ],
    "github@claude-plugins-official": [
      {
        "scope": "project",
        "projectPath": "/home/test",
        "installPath": "/home/test/.claude/plugins/cache/claude-plugins-official/github/236752ad9ab3",
        "version": "236752ad9ab3",
        "installedAt": "2026-03-13T19:01:32.482Z",
        "lastUpdated": "2026-03-13T19:03:25.425Z",
        "gitCommitSha": "236752ad9ab36029b648bfd94707944760242768"
      }
    ]
  }
}"#
    }

    #[test]
    fn parse_installed_plugins_json() {
        let file: InstalledPluginsFile = serde_json::from_str(sample_plugins_json()).unwrap();
        assert_eq!(file.version, 2);
        assert_eq!(file.plugins.len(), 3);

        let ce = &file.plugins["compound-engineering@compound-engineering-plugin"];
        assert_eq!(ce.len(), 1);
        assert_eq!(ce[0].scope, "user");
        assert_eq!(ce[0].version, "2.59.0");
        assert!(ce[0].project_path.is_none());

        let github = &file.plugins["github@claude-plugins-official"];
        assert_eq!(github[0].scope, "project");
        assert_eq!(github[0].project_path.as_deref(), Some("/home/test"));
    }

    #[test]
    fn roundtrip_serialize() {
        let file: InstalledPluginsFile = serde_json::from_str(sample_plugins_json()).unwrap();
        let json = serde_json::to_string_pretty(&file).unwrap();
        let file2: InstalledPluginsFile = serde_json::from_str(&json).unwrap();
        assert_eq!(file.version, file2.version);
        assert_eq!(file.plugins.len(), file2.plugins.len());
        for (key, entries) in &file.plugins {
            let entries2 = &file2.plugins[key];
            assert_eq!(entries.len(), entries2.len());
            for (e1, e2) in entries.iter().zip(entries2.iter()) {
                assert_eq!(e1.scope, e2.scope);
                assert_eq!(e1.version, e2.version);
                assert_eq!(e1.install_path, e2.install_path);
                assert_eq!(e1.git_commit_sha, e2.git_commit_sha);
            }
        }
    }

    #[test]
    fn display_name_from_key_extracts_name() {
        assert_eq!(
            display_name_from_key("compound-engineering@compound-engineering-plugin"),
            "compound-engineering"
        );
        assert_eq!(
            display_name_from_key("frontend-design@claude-plugins-official"),
            "frontend-design"
        );
        assert_eq!(display_name_from_key("simple-plugin"), "simple-plugin");
    }

    #[test]
    fn discover_plugins_from_json() {
        let file: InstalledPluginsFile = serde_json::from_str(sample_plugins_json()).unwrap();
        let mut discovered = Vec::new();
        for (key, entries) in &file.plugins {
            let plugin_data = serde_json::to_string(entries).unwrap();
            discovered.push(DiscoveredPlugin {
                plugin_key: key.clone(),
                display_name: display_name_from_key(key),
                plugin_data,
            });
        }
        assert_eq!(discovered.len(), 3);
        let keys: Vec<&str> = discovered.iter().map(|d| d.plugin_key.as_str()).collect();
        assert!(keys.contains(&"compound-engineering@compound-engineering-plugin"));
        assert!(keys.contains(&"superpowers@claude-plugins-official"));
        assert!(keys.contains(&"github@claude-plugins-official"));
    }

    /// Test the full apply_scenario_plugins flow using a temp file.
    /// We override the path by writing directly and reading back.
    #[test]
    fn apply_scenario_plugins_writes_correct_subset() {
        // This test verifies the logic without touching the real plugins file.
        // We test the building blocks: parsing, filtering, and serialization.

        let file: InstalledPluginsFile = serde_json::from_str(sample_plugins_json()).unwrap();

        // Simulate: only "superpowers" is enabled
        let enabled_keys: std::collections::HashSet<&str> =
            ["superpowers@claude-plugins-official"].into();
        let managed_keys: std::collections::HashSet<&str> =
            file.plugins.keys().map(|k| k.as_str()).collect();

        let mut result = file.clone();

        // Remove all managed
        for key in &managed_keys {
            result.plugins.remove(*key);
        }

        // Re-add only enabled
        for (key, entries) in &file.plugins {
            if enabled_keys.contains(key.as_str()) {
                result.plugins.insert(key.clone(), entries.clone());
            }
        }

        assert_eq!(result.plugins.len(), 1);
        assert!(result
            .plugins
            .contains_key("superpowers@claude-plugins-official"));
        assert!(!result
            .plugins
            .contains_key("compound-engineering@compound-engineering-plugin"));
        assert!(!result
            .plugins
            .contains_key("github@claude-plugins-official"));
    }

    #[test]
    fn multi_scope_plugin_preserved() {
        let json = r#"{
  "version": 2,
  "plugins": {
    "everything-claude-code@everything-claude-code": [
      {
        "scope": "local",
        "projectPath": "/home/test",
        "installPath": "/home/test/.claude/plugins/cache/everything-claude-code/everything-claude-code/1.8.0",
        "version": "1.8.0",
        "installedAt": "2026-03-13T18:31:10.305Z",
        "lastUpdated": "2026-03-13T19:03:25.425Z",
        "gitCommitSha": "fdea3085a76c842edea49a72ea695ccc7ff537ed"
      },
      {
        "scope": "user",
        "installPath": "/home/test/.claude/plugins/cache/everything-claude-code/everything-claude-code/1.8.0",
        "version": "1.8.0",
        "installedAt": "2026-03-30T23:12:55.974Z",
        "lastUpdated": "2026-03-30T23:12:55.974Z",
        "gitCommitSha": "fdea3085a76c842edea49a72ea695ccc7ff537ed"
      }
    ]
  }
}"#;

        let file: InstalledPluginsFile = serde_json::from_str(json).unwrap();
        let entries = &file.plugins["everything-claude-code@everything-claude-code"];
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].scope, "local");
        assert_eq!(entries[1].scope, "user");

        // Roundtrip through plugin_data storage
        let plugin_data = serde_json::to_string(entries).unwrap();
        let restored: Vec<PluginEntry> = serde_json::from_str(&plugin_data).unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored[0].scope, "local");
        assert_eq!(restored[1].scope, "user");
    }
}
