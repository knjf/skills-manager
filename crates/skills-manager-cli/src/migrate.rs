use anyhow::{bail, Context, Result};
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};

/// Skills the migration moves into the plugin.
/// `sm-debug` is intentionally excluded so it remains globally available.
const PLUGIN_SKILLS: &[&str] = &[
    "sm-overview",
    "sm-packs",
    "sm-skills",
    "sm-agents",
    "sm-scenarios",
    "sm-authoring",
    "sm-install",
];

const MARKETPLACE_REL: &str = ".claude/marketplaces/sm-local";
const CLAUDE_SKILLS_REL: &str = ".claude/skills";

const MARKETPLACE_JSON: &str = r#"{
  "name": "sm-local",
  "owner": {
    "name": "Skills Manager"
  },
  "plugins": [
    {
      "name": "sm",
      "source": "./sm-plugin",
      "description": "Skills Manager meta-skills (sm-overview, sm-packs, sm-skills, sm-agents, sm-scenarios, sm-authoring, sm-install). Enable per-project to manage skills via the sm CLI.",
      "version": "0.1.0"
    }
  ]
}
"#;

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    CreateMarketplaceJson(PathBuf),
    CreatePluginSymlink { link: PathBuf, target: PathBuf },
    BackupSkill { from: PathBuf, to: PathBuf },
}

pub struct Plan {
    pub plugin_src: PathBuf,
    pub marketplace_dir: PathBuf,
    pub backup_dir: PathBuf,
    pub actions: Vec<Action>,
}

pub fn run(apply: bool, plugin_dir: Option<&Path>) -> Result<()> {
    let home = dirs::home_dir().context("cannot determine home directory")?;
    let plugin_src = resolve_plugin_dir(plugin_dir)?;
    let marketplace_dir = home.join(MARKETPLACE_REL);
    let claude_skills = home.join(CLAUDE_SKILLS_REL);
    let backup_dir = claude_skills.join(format!("_backup-{}", Local::now().format("%Y%m%d")));

    let plan = build_plan(&plugin_src, &marketplace_dir, &claude_skills, &backup_dir);

    print_plan(&plan);

    if plan.actions.is_empty() {
        println!("\nNothing to do — already migrated.");
        return Ok(());
    }

    if !apply {
        println!("\nDry run. Re-run with --apply to execute.");
        return Ok(());
    }

    execute(&plan)?;
    print_followups(&plan);
    Ok(())
}

fn resolve_plugin_dir(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        return validate_plugin_dir(p);
    }
    let cwd = std::env::current_dir().context("cannot read current directory")?;
    let candidate = cwd.join("claude-plugin");
    if candidate
        .join(".claude-plugin")
        .join("plugin.json")
        .exists()
    {
        return validate_plugin_dir(&candidate);
    }
    bail!(
        "could not locate plugin source. Pass --plugin-dir <path> or run from a directory \
         containing claude-plugin/.claude-plugin/plugin.json"
    )
}

fn validate_plugin_dir(p: &Path) -> Result<PathBuf> {
    let manifest = p.join(".claude-plugin").join("plugin.json");
    if !manifest.exists() {
        bail!("plugin source missing manifest: {}", manifest.display());
    }
    p.canonicalize()
        .with_context(|| format!("cannot canonicalize plugin source {}", p.display()))
}

fn build_plan(
    plugin_src: &Path,
    marketplace_dir: &Path,
    claude_skills: &Path,
    backup_dir: &Path,
) -> Plan {
    let mut actions = Vec::new();

    let marketplace_json = marketplace_dir
        .join(".claude-plugin")
        .join("marketplace.json");
    if !marketplace_json.exists() {
        actions.push(Action::CreateMarketplaceJson(marketplace_json));
    }

    let plugin_symlink = marketplace_dir.join("sm-plugin");
    if plugin_symlink.symlink_metadata().is_err() {
        actions.push(Action::CreatePluginSymlink {
            link: plugin_symlink,
            target: plugin_src.to_path_buf(),
        });
    }

    for skill in PLUGIN_SKILLS {
        let from = claude_skills.join(skill);
        if from.symlink_metadata().is_ok() {
            let to = backup_dir.join(skill);
            actions.push(Action::BackupSkill { from, to });
        }
    }

    Plan {
        plugin_src: plugin_src.to_path_buf(),
        marketplace_dir: marketplace_dir.to_path_buf(),
        backup_dir: backup_dir.to_path_buf(),
        actions,
    }
}

fn print_plan(plan: &Plan) {
    println!("Plugin source: {}", plan.plugin_src.display());
    println!("Marketplace:   {}", plan.marketplace_dir.display());
    println!("Backup dir:    {}", plan.backup_dir.display());
    println!();

    if plan.actions.is_empty() {
        println!("(no actions)");
        return;
    }

    println!("Actions:");
    for action in &plan.actions {
        match action {
            Action::CreateMarketplaceJson(path) => {
                println!("  + write {}", path.display());
            }
            Action::CreatePluginSymlink { link, target } => {
                println!("  + symlink {} -> {}", link.display(), target.display());
            }
            Action::BackupSkill { from, to } => {
                println!("  ~ mv {} -> {}", from.display(), to.display());
            }
        }
    }
}

fn execute(plan: &Plan) -> Result<()> {
    for action in &plan.actions {
        match action {
            Action::CreateMarketplaceJson(path) => {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("mkdir {}", parent.display()))?;
                }
                fs::write(path, MARKETPLACE_JSON)
                    .with_context(|| format!("write {}", path.display()))?;
            }
            Action::CreatePluginSymlink { link, target } => {
                if let Some(parent) = link.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("mkdir {}", parent.display()))?;
                }
                std::os::unix::fs::symlink(target, link).with_context(|| {
                    format!("symlink {} -> {}", link.display(), target.display())
                })?;
            }
            Action::BackupSkill { from, to } => {
                if let Some(parent) = to.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("mkdir {}", parent.display()))?;
                }
                fs::rename(from, to)
                    .with_context(|| format!("mv {} -> {}", from.display(), to.display()))?;
            }
        }
    }
    Ok(())
}

fn print_followups(plan: &Plan) {
    println!("\nDone. Next steps:");
    println!(
        "  claude plugin marketplace add {}",
        plan.marketplace_dir.display()
    );
    println!("  claude plugin install sm@sm-local");
    println!("  claude plugin disable sm@sm-local --scope user");
    println!();
    println!("To enable in a project:");
    println!("  cd <project> && claude plugin enable sm@sm-local --scope project");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    fn fake_plugin_src(dir: &Path) -> PathBuf {
        let p = dir.join("claude-plugin");
        fs::create_dir_all(p.join(".claude-plugin")).unwrap();
        fs::write(p.join(".claude-plugin/plugin.json"), "{}").unwrap();
        p
    }

    #[test]
    fn plan_on_fresh_machine_has_all_three_kinds_of_actions() {
        let tmp = TempDir::new().unwrap();
        let plugin_src = fake_plugin_src(tmp.path());
        let marketplace = tmp.path().join("market");
        let claude_skills = tmp.path().join("skills");
        fs::create_dir_all(&claude_skills).unwrap();
        for s in PLUGIN_SKILLS {
            symlink("/nonexistent", claude_skills.join(s)).unwrap();
        }
        // sm-debug must NOT be moved
        symlink("/nonexistent", claude_skills.join("sm-debug")).unwrap();

        let backup = claude_skills.join("_backup-test");
        let plan = build_plan(&plugin_src, &marketplace, &claude_skills, &backup);

        let mut backup_count = 0;
        let mut has_marketplace = false;
        let mut has_symlink = false;
        for a in &plan.actions {
            match a {
                Action::CreateMarketplaceJson(_) => has_marketplace = true,
                Action::CreatePluginSymlink { .. } => has_symlink = true,
                Action::BackupSkill { from, .. } => {
                    assert!(
                        !from.ends_with("sm-debug"),
                        "sm-debug must not be backed up"
                    );
                    backup_count += 1;
                }
            }
        }
        assert!(has_marketplace, "marketplace.json action missing");
        assert!(has_symlink, "plugin symlink action missing");
        assert_eq!(backup_count, PLUGIN_SKILLS.len());
    }

    #[test]
    fn plan_is_idempotent_when_already_migrated() {
        let tmp = TempDir::new().unwrap();
        let plugin_src = fake_plugin_src(tmp.path());
        let marketplace = tmp.path().join("market");
        let claude_skills = tmp.path().join("skills");

        // Pre-create marketplace.json + plugin symlink + no sm-* skills in claude_skills
        fs::create_dir_all(marketplace.join(".claude-plugin")).unwrap();
        fs::write(
            marketplace.join(".claude-plugin/marketplace.json"),
            MARKETPLACE_JSON,
        )
        .unwrap();
        symlink(&plugin_src, marketplace.join("sm-plugin")).unwrap();
        fs::create_dir_all(&claude_skills).unwrap();

        let backup = claude_skills.join("_backup-test");
        let plan = build_plan(&plugin_src, &marketplace, &claude_skills, &backup);
        assert!(
            plan.actions.is_empty(),
            "expected no actions, got: {:?}",
            plan.actions
        );
    }

    #[test]
    fn plan_skips_marketplace_creation_when_already_present_but_still_backs_up_skills() {
        let tmp = TempDir::new().unwrap();
        let plugin_src = fake_plugin_src(tmp.path());
        let marketplace = tmp.path().join("market");
        let claude_skills = tmp.path().join("skills");

        fs::create_dir_all(marketplace.join(".claude-plugin")).unwrap();
        fs::write(
            marketplace.join(".claude-plugin/marketplace.json"),
            MARKETPLACE_JSON,
        )
        .unwrap();
        symlink(&plugin_src, marketplace.join("sm-plugin")).unwrap();
        fs::create_dir_all(&claude_skills).unwrap();
        symlink("/nonexistent", claude_skills.join("sm-overview")).unwrap();

        let backup = claude_skills.join("_backup-test");
        let plan = build_plan(&plugin_src, &marketplace, &claude_skills, &backup);

        assert_eq!(plan.actions.len(), 1);
        matches!(plan.actions[0], Action::BackupSkill { .. });
    }

    #[test]
    fn execute_creates_marketplace_and_backs_up_skills() {
        let tmp = TempDir::new().unwrap();
        let plugin_src = fake_plugin_src(tmp.path());
        let marketplace = tmp.path().join("market");
        let claude_skills = tmp.path().join("skills");
        fs::create_dir_all(&claude_skills).unwrap();
        for s in PLUGIN_SKILLS {
            symlink("/nonexistent", claude_skills.join(s)).unwrap();
        }

        let backup = claude_skills.join("_backup-test");
        let plan = build_plan(&plugin_src, &marketplace, &claude_skills, &backup);
        execute(&plan).unwrap();

        assert!(marketplace.join(".claude-plugin/marketplace.json").exists());
        assert!(marketplace.join("sm-plugin").symlink_metadata().is_ok());
        for s in PLUGIN_SKILLS {
            assert!(
                !claude_skills.join(s).symlink_metadata().is_ok(),
                "{s} should have been moved out"
            );
            assert!(
                backup.join(s).symlink_metadata().is_ok(),
                "{s} missing in backup"
            );
        }
    }
}
