//! End-to-end PD wiring integration tests.
//! Spawns the `sm` binary against a temporary HOME.

use std::path::PathBuf;
use std::process::Command;

fn sm_bin() -> PathBuf {
    // `cargo test` integration binaries live in target/debug/deps;
    // the binary they test is one level up.
    let mut p = std::env::current_exe().expect("current_exe");
    p.pop(); // drop test binary name
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

/// Build a minimal seeded skills-manager DB + vault under `home`.
fn seed_test_state(home: &std::path::Path) {
    use skills_manager_core::skill_store::{
        DisclosureMode, ScenarioRecord, SkillRecord, SkillStore,
    };

    std::fs::create_dir_all(home.join(".skills-manager/skills")).unwrap();
    let db_path = home.join(".skills-manager/skills-manager.db");
    let store = SkillStore::new(&db_path).unwrap();

    let now = chrono::Utc::now().timestamp_millis();

    // Vault: two skill dirs with minimal SKILL.md.
    for name in ["base-skill", "mkt-skill"] {
        let dir = home.join(".skills-manager/skills").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: x\n---\n"),
        )
        .unwrap();
    }

    let mk_skill = |id: &str, name: &str| SkillRecord {
        id: id.into(),
        name: name.into(),
        description: None,
        source_type: "local".into(),
        source_ref: None,
        source_ref_resolved: None,
        source_subpath: None,
        source_branch: None,
        source_revision: None,
        remote_revision: None,
        central_path: home
            .join(".skills-manager/skills")
            .join(name)
            .to_string_lossy()
            .into(),
        content_hash: None,
        enabled: true,
        created_at: now,
        updated_at: now,
        status: "active".into(),
        update_status: "idle".into(),
        last_checked_at: None,
        last_check_error: None,
    };
    store
        .insert_skill(&mk_skill("sk-base", "base-skill"))
        .unwrap();
    store
        .insert_skill(&mk_skill("sk-mkt", "mkt-skill"))
        .unwrap();

    // Two packs (use convenience API: insert_pack(id, name, desc, icon, color)).
    store
        .insert_pack("p-base", "base", None, None, None)
        .unwrap();
    store
        .insert_pack("p-mkt", "marketing", None, None, None)
        .unwrap();
    store.set_pack_essential("p-base", true).unwrap();
    store
        .set_pack_router("p-mkt", Some("Router description for marketing"), None, now)
        .unwrap();

    store.add_skill_to_pack("p-base", "sk-base").unwrap();
    store.add_skill_to_pack("p-mkt", "sk-mkt").unwrap();

    let scenario = ScenarioRecord {
        id: "sc-test".into(),
        name: "test-scenario".into(),
        description: None,
        icon: None,
        sort_order: 0,
        created_at: now,
        updated_at: now,
        disclosure_mode: DisclosureMode::Full,
    };
    store.insert_scenario(&scenario).unwrap();
    store.add_pack_to_scenario("sc-test", "p-base").unwrap();
    store.add_pack_to_scenario("sc-test", "p-mkt").unwrap();
    store.set_active_scenario("sc-test").unwrap();

    store.set_agent_scenario("claude_code", "sc-test").unwrap();
    let _ = store.set_agent_managed("claude_code", true);

    // Pre-create agent dirs so adapter detects "installed" and has a sync target.
    std::fs::create_dir_all(home.join(".claude/skills")).unwrap();
}

#[test]
fn set_mode_persists_to_db() {
    let tmp = tempfile::tempdir().unwrap();
    seed_test_state(tmp.path());

    let (ok, out, err) = run_sm(
        tmp.path(),
        &["scenario", "set-mode", "test-scenario", "hybrid"],
    );
    assert!(ok, "set-mode failed: {err}\n{out}");
    assert!(out.contains("hybrid"), "out was: {out}");
}

#[test]
fn set_essential_persists_to_db() {
    let tmp = tempfile::tempdir().unwrap();
    seed_test_state(tmp.path());

    let (ok, out, err) = run_sm(tmp.path(), &["pack", "set-essential", "marketing", "true"]);
    assert!(ok, "set-essential failed: {err}\n{out}");
    assert!(out.contains("true"), "out was: {out}");
}

#[test]
fn switch_to_hybrid_creates_pack_routers() {
    let tmp = tempfile::tempdir().unwrap();
    seed_test_state(tmp.path());

    let (ok, _, err) = run_sm(
        tmp.path(),
        &["scenario", "set-mode", "test-scenario", "hybrid"],
    );
    assert!(ok, "set-mode failed: {err}");

    let (ok, _, err) = run_sm(tmp.path(), &["switch", "claude_code", "test-scenario"]);
    assert!(ok, "switch failed: {err}");

    let claude_skills = tmp.path().join(".claude/skills");
    assert!(
        claude_skills.join("base-skill").exists(),
        "essential skill should be materialized"
    );
    let router_md = claude_skills.join("pack-marketing/SKILL.md");
    assert!(
        router_md.exists(),
        "router should be created at {}",
        router_md.display()
    );
    let router = std::fs::read_to_string(&router_md).unwrap();
    assert!(
        router.contains("Router description for marketing"),
        "router body did not include description; got:\n{router}"
    );
    assert!(
        !claude_skills.join("mkt-skill").exists(),
        "non-essential skill should NOT be materialized in hybrid"
    );
}

#[test]
fn switch_from_hybrid_to_full_removes_routers() {
    let tmp = tempfile::tempdir().unwrap();
    seed_test_state(tmp.path());

    run_sm(
        tmp.path(),
        &["scenario", "set-mode", "test-scenario", "hybrid"],
    );
    run_sm(tmp.path(), &["switch", "claude_code", "test-scenario"]);
    assert!(
        tmp.path().join(".claude/skills/pack-marketing").exists(),
        "router should exist after hybrid switch"
    );

    run_sm(
        tmp.path(),
        &["scenario", "set-mode", "test-scenario", "full"],
    );
    let (ok, _, err) = run_sm(tmp.path(), &["switch", "claude_code", "test-scenario"]);
    assert!(ok, "switch back to full failed: {err}");

    assert!(
        !tmp.path().join(".claude/skills/pack-marketing").exists(),
        "router should be removed after switching back to full"
    );
    assert!(tmp.path().join(".claude/skills/base-skill").exists());
    assert!(
        tmp.path().join(".claude/skills/mkt-skill").exists(),
        "non-essential skill should now be materialized in full mode"
    );
}
