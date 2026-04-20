use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Each builtin skill is embedded at compile time. `(skill_name, SKILL.md content)`.
const BUILTIN_SKILLS: &[(&str, &str)] = &[
    (
        "pack-router-gen",
        include_str!("../assets/builtin-skills/pack-router-gen/SKILL.md"),
    ),
    (
        "sm-overview",
        include_str!("../assets/builtin-skills/sm-overview/SKILL.md"),
    ),
    (
        "sm-scenarios",
        include_str!("../assets/builtin-skills/sm-scenarios/SKILL.md"),
    ),
    (
        "sm-packs",
        include_str!("../assets/builtin-skills/sm-packs/SKILL.md"),
    ),
    (
        "sm-skills",
        include_str!("../assets/builtin-skills/sm-skills/SKILL.md"),
    ),
    (
        "sm-authoring",
        include_str!("../assets/builtin-skills/sm-authoring/SKILL.md"),
    ),
    (
        "sm-debug",
        include_str!("../assets/builtin-skills/sm-debug/SKILL.md"),
    ),
    (
        "sm-agents",
        include_str!("../assets/builtin-skills/sm-agents/SKILL.md"),
    ),
    (
        "sm-install",
        include_str!("../assets/builtin-skills/sm-install/SKILL.md"),
    ),
];

pub fn install_builtin_skills(vault_root: &Path) -> Result<()> {
    for (name, content) in BUILTIN_SKILLS {
        let dir = vault_root.join(name);
        fs::create_dir_all(&dir).with_context(|| format!("create {name} dir"))?;
        let path = dir.join("SKILL.md");
        fs::write(&path, content).with_context(|| format!("write {name}/SKILL.md"))?;
    }
    Ok(())
}

/// Names of the builtin sm-* skills (for seeder to reference).
pub const SM_SKILL_NAMES: &[&str] = &[
    "sm-overview",
    "sm-scenarios",
    "sm-packs",
    "sm-skills",
    "sm-authoring",
    "sm-debug",
    "sm-agents",
    "sm-install",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_pack_router_gen_skill() {
        let tmp = tempfile::tempdir().unwrap();
        install_builtin_skills(tmp.path()).unwrap();
        let p = tmp.path().join("pack-router-gen/SKILL.md");
        assert!(p.exists(), "SKILL.md file should be written");
        let content = fs::read_to_string(&p).unwrap();
        assert!(
            content.contains("name: pack-router-gen"),
            "frontmatter name present"
        );
        assert!(
            content.contains("sm pack set-router"),
            "CLI instructions present"
        );
    }

    #[test]
    fn install_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        install_builtin_skills(tmp.path()).unwrap();
        install_builtin_skills(tmp.path()).unwrap();
        let p = tmp.path().join("pack-router-gen/SKILL.md");
        assert!(p.exists());
    }

    #[test]
    fn installs_all_eight_sm_skills() {
        let tmp = tempfile::tempdir().unwrap();
        install_builtin_skills(tmp.path()).unwrap();
        for name in [
            "sm-overview",
            "sm-scenarios",
            "sm-packs",
            "sm-skills",
            "sm-authoring",
            "sm-debug",
            "sm-agents",
            "sm-install",
        ] {
            let p = tmp.path().join(name).join("SKILL.md");
            assert!(p.exists(), "{name}/SKILL.md should be written");
            let content = fs::read_to_string(&p).unwrap();
            assert!(
                content.contains(&format!("name: {name}")),
                "{name} frontmatter missing name field"
            );
        }
    }
}
