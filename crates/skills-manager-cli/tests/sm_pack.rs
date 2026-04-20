//! End-to-end test for the builtin sm pack.

use std::path::PathBuf;
use std::process::Command;

fn sm_bin() -> PathBuf {
    let mut p = std::env::current_exe().unwrap();
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    p.join("sm")
}

fn run_sm(home: &std::path::Path, args: &[&str]) -> (bool, String, String) {
    let output = Command::new(sm_bin())
        .args(args)
        .env("HOME", home)
        .output()
        .expect("failed to run sm");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Seed a minimal DB with a scenario and the 8 sm-* skills in the vault.
fn seed_fresh(home: &std::path::Path) {
    use skills_manager_core::skill_store::{
        DisclosureMode, ScenarioRecord, SkillRecord, SkillStore,
    };

    std::fs::create_dir_all(home.join(".skills-manager/skills")).unwrap();
    let db_path = home.join(".skills-manager/skills-manager.db");
    let store = SkillStore::new(&db_path).unwrap();

    // Simulate install_builtin_skills having copied sm-* dirs into the vault.
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
        let dir = home.join(".skills-manager/skills").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: test\n---\n"),
        )
        .unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        store
            .insert_skill(&SkillRecord {
                id: format!("id-{name}"),
                name: name.to_string(),
                description: Some(format!("{name} test desc")),
                source_type: "local".into(),
                source_ref: None,
                source_ref_resolved: None,
                source_subpath: None,
                source_branch: None,
                source_revision: None,
                remote_revision: None,
                central_path: dir.to_string_lossy().into_owned(),
                content_hash: None,
                enabled: true,
                created_at: now,
                updated_at: now,
                status: "active".into(),
                update_status: "idle".into(),
                last_checked_at: None,
                last_check_error: None,
                description_router: None,
            })
            .unwrap();
    }

    let scenario = ScenarioRecord {
        id: "sc-test".into(),
        name: "test-scenario".into(),
        description: None,
        icon: None,
        sort_order: 0,
        created_at: 0,
        updated_at: 0,
        disclosure_mode: DisclosureMode::Full,
    };
    store.insert_scenario(&scenario).unwrap();
    store.set_active_scenario("sc-test").unwrap();
    store.set_agent_scenario("claude_code", "sc-test").unwrap();

    std::fs::create_dir_all(home.join(".claude/skills")).unwrap();
}

#[test]
fn sm_pack_seeded_idempotently_on_sm_invocation() {
    let tmp = tempfile::tempdir().unwrap();
    seed_fresh(tmp.path());

    // First run: ensure_sm_pack_installed should seed the pack.
    let (ok, _, err) = run_sm(tmp.path(), &["list"]);
    assert!(ok, "first sm list failed: {err}");

    let db_path = tmp.path().join(".skills-manager/skills-manager.db");
    {
        let store = skills_manager_core::skill_store::SkillStore::new(&db_path).unwrap();
        let packs = store.get_all_packs().unwrap();
        let sm_pack = packs
            .iter()
            .find(|p| p.name == "sm")
            .expect("sm pack missing");
        assert!(sm_pack.is_essential);
        assert!(sm_pack.router_description.is_some());
        assert!(sm_pack.router_when_to_use.is_some());

        let skills = store.get_skills_for_pack(&sm_pack.id).unwrap();
        assert_eq!(skills.len(), 8, "sm pack should have 8 skills");
        for s in &skills {
            assert!(
                s.description_router.is_some(),
                "skill {} missing L2",
                s.name
            );
        }
    } // store dropped here — SQLite lock released

    // Second run: should be a no-op.
    let (ok2, _, _) = run_sm(tmp.path(), &["list"]);
    assert!(ok2);

    let store2 = skills_manager_core::skill_store::SkillStore::new(&db_path).unwrap();
    let packs2 = store2.get_all_packs().unwrap();
    assert_eq!(packs2.iter().filter(|p| p.name == "sm").count(), 1);
}
