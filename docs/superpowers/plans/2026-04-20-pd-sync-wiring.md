# PD Sync Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire `reconcile_agent_dir` into both CLI and Tauri sync paths so `disclosure_mode` actually controls what gets materialized into agent skills directories.

**Architecture:** Add owned-data store helpers (`get_packs_with_skills_for_*`), extend `resolve_desired_state`/`reconcile_agent_dir` with a per-skill exclusion set, replace per-skill loops in `sync_scenario`/`sync_agent`/`sync_agent_skills` with calls to `reconcile_agent_dir`, replace unsync paths with a new `unreconcile_agent_dir` that removes both skills and routers. Add CLI `set-mode` and `set-essential` subcommands so PD can be exercised entirely from the terminal.

**Tech Stack:** Rust (anyhow, rusqlite, tauri 2), CLI uses clap. Tests use Rust's built-in `#[test]` + `tempfile`. Repo layout: `crates/skills-manager-core/`, `crates/skills-manager-cli/`, `src-tauri/`.

**Spec:** `docs/superpowers/specs/2026-04-20-pd-sync-wiring-design.md`

---

## File Map

**Create:**
- `crates/skills-manager-cli/tests/pd_wiring.rs` — CLI integration tests for end-to-end disclosure behavior

**Modify:**
- `crates/skills-manager-core/src/skill_store.rs` — add `get_packs_with_skills_for_scenario`, `get_packs_with_skills_for_agent`
- `crates/skills-manager-core/src/sync_engine/disclosure.rs` — extend `resolve_desired_state` with `excluded_skills`
- `crates/skills-manager-core/src/sync_engine/mod.rs` — extend `reconcile_agent_dir` with `excluded_skills`; add `unreconcile_agent_dir`
- `crates/skills-manager-cli/src/commands.rs` — replace `sync_scenario`/`sync_agent`/`unsync_scenario` bodies; add `cmd_scenario_set_mode`, `cmd_pack_set_essential`
- `crates/skills-manager-cli/src/main.rs` — register `Scenario { SetMode { ... } }` + `PackAction::SetEssential`
- `src-tauri/src/commands/scenarios.rs` — replace `sync_agent_skills` and `unsync_agent_skills` bodies

---

## Task 1: Add `get_packs_with_skills_for_scenario` store helper

**Files:**
- Modify: `crates/skills-manager-core/src/skill_store.rs` (insert after line ~1566, near `get_packs_for_scenario`)

- [ ] **Step 1: Write the failing test**

Append to the existing tests module at the bottom of `skill_store.rs`:

```rust
    #[test]
    fn get_packs_with_skills_for_scenario_returns_pairs() {
        let store = test_store();
        // Two packs, two skills each, one scenario containing both packs.
        let now = 0i64;
        store
            .insert_pack(&PackRecord {
                id: "p-a".into(),
                name: "pack-a".into(),
                description: None,
                icon: None,
                color: None,
                sort_order: 0,
                created_at: now,
                updated_at: now,
                router_description: None,
                router_body: None,
                is_essential: false,
                router_updated_at: None,
            })
            .unwrap();
        store
            .insert_pack(&PackRecord {
                id: "p-b".into(),
                name: "pack-b".into(),
                description: None,
                icon: None,
                color: None,
                sort_order: 1,
                created_at: now,
                updated_at: now,
                router_description: None,
                router_body: None,
                is_essential: false,
                router_updated_at: None,
            })
            .unwrap();
        for (id, name) in [("s-a1", "a1"), ("s-a2", "a2"), ("s-b1", "b1")] {
            store
                .insert_skill(&minimal_skill(id, name))
                .unwrap();
        }
        store.add_skill_to_pack("p-a", "s-a1", 0).unwrap();
        store.add_skill_to_pack("p-a", "s-a2", 1).unwrap();
        store.add_skill_to_pack("p-b", "s-b1", 0).unwrap();

        let scenario = ScenarioRecord {
            id: "sc1".into(),
            name: "sc1".into(),
            description: None,
            icon: None,
            sort_order: 0,
            created_at: now,
            updated_at: now,
            disclosure_mode: DisclosureMode::Full,
        };
        store.insert_scenario(&scenario).unwrap();
        store.add_pack_to_scenario("sc1", "p-a").unwrap();
        store.add_pack_to_scenario("sc1", "p-b").unwrap();

        let pairs = store.get_packs_with_skills_for_scenario("sc1").unwrap();

        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].0.name, "pack-a");
        assert_eq!(pairs[0].1.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["a1", "a2"]);
        assert_eq!(pairs[1].0.name, "pack-b");
        assert_eq!(pairs[1].1.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["b1"]);
    }
```

If `test_store()` and `minimal_skill()` helpers don't exist in the test module, scan for the closest existing helpers (look near the top of the `tests` module — there are existing fixtures like `pack(...)` and `skill(...)`) and adapt names accordingly. Reuse, do not duplicate, fixtures.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p skills-manager-core get_packs_with_skills_for_scenario_returns_pairs --no-fail-fast`

Expected: FAIL with `no method named 'get_packs_with_skills_for_scenario'`

- [ ] **Step 3: Implement the method**

Insert immediately below `get_packs_for_scenario` (around line 1566 of `skill_store.rs`):

```rust
    /// Returns each scenario pack paired with its full skill list.
    /// Used by the disclosure-mode-aware sync to decide what gets materialized.
    pub fn get_packs_with_skills_for_scenario(
        &self,
        scenario_id: &str,
    ) -> Result<Vec<(PackRecord, Vec<SkillRecord>)>> {
        let packs = self.get_packs_for_scenario(scenario_id)?;
        let mut out = Vec::with_capacity(packs.len());
        for pack in packs {
            let skills = self.get_skills_for_pack(&pack.id)?;
            out.push((pack, skills));
        }
        Ok(out)
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p skills-manager-core get_packs_with_skills_for_scenario_returns_pairs --no-fail-fast`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/skills-manager-core/src/skill_store.rs
git commit -m "feat(core): add get_packs_with_skills_for_scenario helper"
```

---

## Task 2: Add `get_packs_with_skills_for_agent` store helper

**Files:**
- Modify: `crates/skills-manager-core/src/skill_store.rs` (insert after the method added in Task 1)

- [ ] **Step 1: Write the failing test**

Append to the tests module:

```rust
    #[test]
    fn get_packs_with_skills_for_agent_includes_extra_packs_and_dedupes() {
        let store = test_store();
        let now = 0i64;
        // Three packs.
        for (id, name, sort) in [("p-base", "base", 0), ("p-extra", "extra", 1), ("p-shared", "shared", 2)] {
            store.insert_pack(&PackRecord {
                id: id.into(),
                name: name.into(),
                description: None,
                icon: None,
                color: None,
                sort_order: sort,
                created_at: now,
                updated_at: now,
                router_description: None,
                router_body: None,
                is_essential: false,
                router_updated_at: None,
            }).unwrap();
        }
        // One skill per pack.
        for (sid, sname) in [("sk-base", "base-skill"), ("sk-extra", "extra-skill"), ("sk-shared", "shared-skill")] {
            store.insert_skill(&minimal_skill(sid, sname)).unwrap();
        }
        store.add_skill_to_pack("p-base", "sk-base", 0).unwrap();
        store.add_skill_to_pack("p-extra", "sk-extra", 0).unwrap();
        store.add_skill_to_pack("p-shared", "sk-shared", 0).unwrap();

        // Scenario contains base + shared.
        let scenario = ScenarioRecord {
            id: "sc1".into(), name: "sc1".into(), description: None, icon: None,
            sort_order: 0, created_at: now, updated_at: now,
            disclosure_mode: DisclosureMode::Full,
        };
        store.insert_scenario(&scenario).unwrap();
        store.add_pack_to_scenario("sc1", "p-base").unwrap();
        store.add_pack_to_scenario("sc1", "p-shared").unwrap();

        // Agent assigned to that scenario, with extra packs (extra + shared — shared overlaps).
        store.set_agent_scenario("claude_code", "sc1").unwrap();
        store.add_agent_extra_pack("claude_code", "p-extra").unwrap();
        store.add_agent_extra_pack("claude_code", "p-shared").unwrap();

        let pairs = store.get_packs_with_skills_for_agent("claude_code").unwrap();
        let pack_names: Vec<_> = pairs.iter().map(|(p, _)| p.name.as_str()).collect();

        // base + shared (from scenario), then extra (from extras). Shared appears once.
        assert_eq!(pack_names, vec!["base", "shared", "extra"]);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p skills-manager-core get_packs_with_skills_for_agent_includes_extra_packs_and_dedupes --no-fail-fast`

Expected: FAIL with `no method named 'get_packs_with_skills_for_agent'`

- [ ] **Step 3: Implement the method**

Insert immediately below the method added in Task 1:

```rust
    /// Returns each pack effectively assigned to an agent (scenario packs +
    /// agent extra packs), paired with its full skill list. Deduplicates by
    /// pack id, preserving scenario-pack order first, then extras.
    pub fn get_packs_with_skills_for_agent(
        &self,
        tool_key: &str,
    ) -> Result<Vec<(PackRecord, Vec<SkillRecord>)>> {
        use std::collections::HashSet;

        let agent_config = self.get_agent_config(tool_key)?;
        let scenario_id = match agent_config.and_then(|c| c.scenario_id) {
            Some(id) => id,
            None => return Ok(Vec::new()),
        };

        let scenario_packs = self.get_packs_for_scenario(&scenario_id)?;
        let extra_packs = self.get_agent_extra_packs(tool_key)?;

        let mut seen: HashSet<String> = HashSet::new();
        let mut combined: Vec<PackRecord> = Vec::with_capacity(scenario_packs.len() + extra_packs.len());
        for p in scenario_packs.into_iter().chain(extra_packs.into_iter()) {
            if seen.insert(p.id.clone()) {
                combined.push(p);
            }
        }

        let mut out = Vec::with_capacity(combined.len());
        for pack in combined {
            let skills = self.get_skills_for_pack(&pack.id)?;
            out.push((pack, skills));
        }
        Ok(out)
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p skills-manager-core get_packs_with_skills_for_agent_includes_extra_packs_and_dedupes --no-fail-fast`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/skills-manager-core/src/skill_store.rs
git commit -m "feat(core): add get_packs_with_skills_for_agent helper"
```

---

## Task 3: Extend `resolve_desired_state` with `excluded_skills`

**Files:**
- Modify: `crates/skills-manager-core/src/sync_engine/disclosure.rs`

- [ ] **Step 1: Write the failing test**

Append to the existing `mod tests` block in `disclosure.rs` (after the existing `pack`/`skill` helpers):

```rust
    #[test]
    fn excluded_skills_filtered_in_full_mode() {
        use std::collections::HashSet;
        let p = pack("p1", false);
        let skills = vec![skill("alpha"), skill("beta")];
        let packs = vec![PackWithSkills { pack: &p, skills: &skills }];
        let mut excluded = HashSet::new();
        excluded.insert("beta".to_string());

        let entries = resolve_desired_state(
            std::path::Path::new("/cc"),
            &packs,
            DisclosureMode::Full,
            &excluded,
        );

        let names: Vec<_> = entries.iter().map(|e| e.target_path.to_string_lossy().to_string()).collect();
        assert!(names.contains(&"/cc/alpha".to_string()));
        assert!(!names.contains(&"/cc/beta".to_string()));
    }

    #[test]
    fn excluded_skills_does_not_affect_routers_in_hybrid() {
        use std::collections::HashSet;
        let p = pack("mkt", false);
        let skills = vec![skill("alpha"), skill("beta")];
        let packs = vec![PackWithSkills { pack: &p, skills: &skills }];
        let mut excluded = HashSet::new();
        excluded.insert("alpha".to_string());
        excluded.insert("beta".to_string());

        let entries = resolve_desired_state(
            std::path::Path::new("/cc"),
            &packs,
            DisclosureMode::Hybrid,
            &excluded,
        );

        // Pack is non-essential and we're in hybrid: skills are not materialized
        // (vault-only), so excluded set is irrelevant for skills here.
        // But the router MUST still be emitted.
        let paths: Vec<_> = entries.iter().map(|e| e.target_path.to_string_lossy().to_string()).collect();
        assert_eq!(paths, vec!["/cc/pack-mkt".to_string()]);
    }
```

Note: existing tests in this module call `resolve_desired_state(dir, packs, mode)` with 3 args. They must be updated in Step 3 below.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p skills-manager-core --lib disclosure:: --no-fail-fast`

Expected: FAIL — both new tests fail to compile (function takes 3 args, given 4) AND existing tests will also fail to compile after the upcoming signature change.

- [ ] **Step 3: Update the function signature and update all callers in this file**

In `disclosure.rs`, change `resolve_desired_state` to:

```rust
pub fn resolve_desired_state(
    agent_skills_dir: &Path,
    packs: &[PackWithSkills<'_>],
    mode: DisclosureMode,
    excluded_skills: &std::collections::HashSet<String>,
) -> Vec<DesiredEntry> {
    let mut out = Vec::new();
    for p in packs {
        let materialize = match mode {
            DisclosureMode::Full => true,
            DisclosureMode::Hybrid => p.pack.is_essential,
            DisclosureMode::RouterOnly => false,
        };
        if materialize {
            for s in p.skills {
                if excluded_skills.contains(&s.name) {
                    continue;
                }
                out.push(DesiredEntry {
                    target_path: agent_skills_dir.join(&s.name),
                    kind: EntryKind::Skill { skill_name: s.name.clone() },
                });
            }
        }
        if mode != DisclosureMode::Full && !p.pack.is_essential {
            out.push(DesiredEntry {
                target_path: agent_skills_dir.join(format!("pack-{}", p.pack.name)),
                kind: EntryKind::Router { pack_name: p.pack.name.clone() },
            });
        }
    }
    out
}
```

Then update the existing tests in this module (`full_mode_materializes_everything_no_routers`, `hybrid_mode_keeps_essential_skills_and_emits_routers_for_domain`, `router_only_emits_only_routers_for_non_essential`) to pass `&HashSet::new()` as the fourth argument. Add `use std::collections::HashSet;` at the top of the test module if not already present.

Example update for one of the existing tests:

```rust
        let entries = resolve_desired_state(
            std::path::Path::new("/cc"),
            &packs,
            DisclosureMode::Full,
            &HashSet::new(),
        );
```

- [ ] **Step 4: Run all disclosure tests to verify they pass**

Run: `cargo test -p skills-manager-core --lib disclosure:: --no-fail-fast`

Expected: PASS for all tests in `disclosure::tests` (existing + 2 new).

- [ ] **Step 5: Commit**

```bash
git add crates/skills-manager-core/src/sync_engine/disclosure.rs
git commit -m "feat(core): add per-skill exclusion to resolve_desired_state"
```

---

## Task 4: Extend `reconcile_agent_dir` with `excluded_skills` + add `unreconcile_agent_dir`

**Files:**
- Modify: `crates/skills-manager-core/src/sync_engine/mod.rs`

- [ ] **Step 1: Write the failing tests**

Append to the existing `mod tests` block in `sync_engine/mod.rs`:

```rust
    #[test]
    fn reconcile_with_excluded_skills_skips_them() {
        use std::collections::HashSet;
        let tmp = tempdir().unwrap();
        let agent_dir = tmp.path().join("agent");
        let vault_root = tmp.path().join("vault");
        fs::create_dir_all(vault_root.join("alpha")).unwrap();
        fs::create_dir_all(vault_root.join("beta")).unwrap();
        fs::write(vault_root.join("alpha/SKILL.md"), "alpha-content").unwrap();
        fs::write(vault_root.join("beta/SKILL.md"), "beta-content").unwrap();

        let p = PackRecord {
            id: "p-ess".into(), name: "ess".into(), description: None,
            icon: None, color: None, sort_order: 0, created_at: 0, updated_at: 0,
            router_description: None, router_body: None,
            is_essential: true, router_updated_at: None,
        };
        let skills = vec![
            SkillRecord {
                id: "alpha".into(), name: "alpha".into(), description: None,
                source_type: "local".into(), source_ref: None, source_ref_resolved: None,
                source_subpath: None, source_branch: None, source_revision: None,
                remote_revision: None, central_path: vault_root.join("alpha").to_string_lossy().into(),
                content_hash: None, enabled: true, created_at: 0, updated_at: 0,
                status: "active".into(), update_status: "idle".into(),
                last_checked_at: None, last_check_error: None,
            },
            SkillRecord {
                id: "beta".into(), name: "beta".into(), description: None,
                source_type: "local".into(), source_ref: None, source_ref_resolved: None,
                source_subpath: None, source_branch: None, source_revision: None,
                remote_revision: None, central_path: vault_root.join("beta").to_string_lossy().into(),
                content_hash: None, enabled: true, created_at: 0, updated_at: 0,
                status: "active".into(), update_status: "idle".into(),
                last_checked_at: None, last_check_error: None,
            },
        ];
        let packs = vec![disclosure::PackWithSkills { pack: &p, skills: &skills }];

        let mut excluded = HashSet::new();
        excluded.insert("beta".to_string());

        let report = reconcile_agent_dir(
            &agent_dir,
            &packs,
            DisclosureMode::Full,
            &vault_root,
            &excluded,
        ).unwrap();

        assert!(agent_dir.join("alpha").exists());
        assert!(!agent_dir.join("beta").exists());
        assert_eq!(report.added, 1);
    }

    #[test]
    fn unreconcile_removes_routers_and_symlinks_but_leaves_native() {
        let tmp = tempdir().unwrap();
        let agent_dir = tmp.path().join("agent");
        let vault_root = tmp.path().join("vault");
        fs::create_dir_all(&agent_dir).unwrap();
        fs::create_dir_all(vault_root.join("real-skill")).unwrap();
        fs::write(vault_root.join("real-skill/SKILL.md"), "x").unwrap();

        // 1. SM-managed symlink into vault.
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            vault_root.join("real-skill"),
            agent_dir.join("real-skill"),
        ).unwrap();

        // 2. SM-managed router dir with marker.
        let router_dir = agent_dir.join("pack-marketing");
        fs::create_dir_all(&router_dir).unwrap();
        fs::write(router_dir.join("SKILL.md"), "---\nname: pack-marketing\n---\n# Pack: marketing\n").unwrap();

        // 3. Native skill — plain dir, not a symlink, no router marker.
        let native = agent_dir.join("native-skill");
        fs::create_dir_all(&native).unwrap();
        fs::write(native.join("SKILL.md"), "i was here first").unwrap();

        let removed = unreconcile_agent_dir(&agent_dir).unwrap();

        assert_eq!(removed, 2);
        assert!(!agent_dir.join("real-skill").exists());
        assert!(!agent_dir.join("pack-marketing").exists());
        assert!(agent_dir.join("native-skill").exists());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p skills-manager-core --lib sync_engine::tests::reconcile_with_excluded_skills_skips_them sync_engine::tests::unreconcile_removes_routers_and_symlinks_but_leaves_native --no-fail-fast`

Expected: FAIL — both compile errors (signature mismatch / `unreconcile_agent_dir` missing).

- [ ] **Step 3: Update `reconcile_agent_dir` signature and update existing tests**

Change `reconcile_agent_dir` in `sync_engine/mod.rs`:

```rust
pub fn reconcile_agent_dir(
    agent_skills_dir: &Path,
    packs: &[disclosure::PackWithSkills<'_>],
    mode: crate::skill_store::DisclosureMode,
    vault_root: &Path,
    excluded_skills: &std::collections::HashSet<String>,
) -> Result<ReconcileReport> {
    use disclosure::{resolve_desired_state, EntryKind};
    use std::collections::HashSet;
    use std::fs;

    fs::create_dir_all(agent_skills_dir)
        .with_context(|| format!("create agent skills dir {}", agent_skills_dir.display()))?;

    let desired = resolve_desired_state(agent_skills_dir, packs, mode, excluded_skills);
    let desired_paths: HashSet<_> = desired.iter().map(|e| e.target_path.clone()).collect();
    let mut report = ReconcileReport::default();

    // (rest of the function body unchanged from current implementation)
```

Keep the rest of the body identical to the existing implementation. The only change is the new parameter and the new fourth arg passed to `resolve_desired_state`.

Update existing tests in `sync_engine::tests` that call `reconcile_agent_dir` (search for all occurrences) to pass `&HashSet::new()` as the fifth argument. There are four such tests at lines ~498, ~555, ~595, ~613 of the current file.

Example update:

```rust
        reconcile_agent_dir(&agent_dir, &packs, DisclosureMode::Hybrid, &vault_root, &HashSet::new()).unwrap();
```

Add `use std::collections::HashSet;` to the test module imports if not already present.

- [ ] **Step 4: Add `unreconcile_agent_dir`**

Insert immediately after `reconcile_agent_dir` in `sync_engine/mod.rs` (before the existing `is_sm_managed` helper):

```rust
/// Remove every SM-managed entry from an agent's skills directory.
/// Used when unsyncing a scenario; complements `reconcile_agent_dir`.
/// Returns the number of entries removed. Native (non-SM) entries are left alone.
pub fn unreconcile_agent_dir(agent_skills_dir: &Path) -> Result<usize> {
    use std::fs;
    if !agent_skills_dir.exists() {
        return Ok(0);
    }
    let mut removed = 0;
    for entry in fs::read_dir(agent_skills_dir)? {
        let entry = entry?;
        let p = entry.path();
        if !is_sm_managed(&p)? {
            continue;
        }
        if p.is_dir() && !p.is_symlink() {
            fs::remove_dir_all(&p)?;
        } else {
            fs::remove_file(&p)?;
        }
        removed += 1;
    }
    Ok(removed)
}
```

- [ ] **Step 5: Run all sync_engine tests to verify they pass**

Run: `cargo test -p skills-manager-core --lib sync_engine:: --no-fail-fast`

Expected: PASS for all sync_engine tests (existing + 2 new).

- [ ] **Step 6: Commit**

```bash
git add crates/skills-manager-core/src/sync_engine/mod.rs
git commit -m "feat(core): exclusion set for reconcile + add unreconcile_agent_dir"
```

---

## Task 5: Wire `reconcile_agent_dir` into CLI `sync_scenario`

**Files:**
- Modify: `crates/skills-manager-cli/src/commands.rs` (function `sync_scenario`, lines ~772-810)

- [ ] **Step 1: Replace the `sync_scenario` body**

Replace the existing `sync_scenario` function with:

```rust
fn sync_scenario(
    store: &SkillStore,
    scenario_id: &str,
    adapters: &[tool_adapters::ToolAdapter],
    _configured_mode: Option<&str>,
) -> Result<Vec<(String, usize)>> {
    let scenario = store
        .get_scenario(scenario_id)?
        .with_context(|| format!("scenario '{}' not found", scenario_id))?;
    let packs_with_skills = store.get_packs_with_skills_for_scenario(scenario_id)?;
    let vault_root = central_repo::skills_dir();

    let mut results = Vec::new();
    for adapter in adapters {
        let skills_dir = adapter.skills_dir();

        // Build excluded set for this adapter from per-skill tool toggles.
        let mut excluded: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (_pack, skills) in &packs_with_skills {
            for skill in skills {
                let adapter_keys = vec![adapter.key.clone()];
                store.ensure_scenario_skill_tool_defaults(scenario_id, &skill.id, &adapter_keys)?;
                let enabled = store.get_enabled_tools_for_scenario_skill(scenario_id, &skill.id)?;
                if !enabled.contains(&adapter.key) {
                    excluded.insert(skill.name.clone());
                }
            }
        }

        let pack_views: Vec<sync_engine::disclosure::PackWithSkills> = packs_with_skills
            .iter()
            .map(|(p, s)| sync_engine::disclosure::PackWithSkills { pack: p, skills: s.as_slice() })
            .collect();

        let report = sync_engine::reconcile_agent_dir(
            &skills_dir,
            &pack_views,
            scenario.disclosure_mode,
            &vault_root,
            &excluded,
        )?;
        results.push((adapter.display_name.clone(), report.added));
    }

    Ok(results)
}
```

Note: `_configured_mode` is no longer used here (sync_skill always uses Symlink under reconcile). Keep the parameter in the signature for now to avoid touching the callers; rename to `_configured_mode` to silence the unused warning.

- [ ] **Step 2: Build to verify it compiles**

Run: `cargo build -p skills-manager-cli`

Expected: Build succeeds. If there are import errors:
- Add `use anyhow::Context;` if not already imported
- The `sync_engine::disclosure` module is already public from sync_engine

If `store.get_scenario` returns `Result<ScenarioRecord>` instead of `Result<Option<ScenarioRecord>>`, drop the `with_context(...)` call and just use `?`. (Check the actual signature in `skill_store.rs:840`.)

- [ ] **Step 3: Run all CLI tests to verify nothing broke**

Run: `cargo test -p skills-manager-cli --no-fail-fast`

Expected: All existing tests still pass (this is a refactor of internal sync; integration tests will be added in Task 9).

- [ ] **Step 4: Commit**

```bash
git add crates/skills-manager-cli/src/commands.rs
git commit -m "feat(cli): sync_scenario uses reconcile_agent_dir"
```

---

## Task 6: Wire `reconcile_agent_dir` into CLI `sync_agent`

**Files:**
- Modify: `crates/skills-manager-cli/src/commands.rs` (function `sync_agent`, lines ~814-852)

- [ ] **Step 1: Replace the `sync_agent` body**

Replace the existing `sync_agent` function with:

```rust
fn sync_agent(
    store: &SkillStore,
    tool_key: &str,
    adapters: &[tool_adapters::ToolAdapter],
    _configured_mode: Option<&str>,
) -> Result<Vec<(String, usize)>> {
    let agent_config = store
        .get_agent_config(tool_key)?
        .with_context(|| format!("agent '{}' has no config", tool_key))?;
    let scenario_id = match agent_config.scenario_id {
        Some(id) => id,
        None => return Ok(Vec::new()),
    };
    let scenario = store
        .get_scenario(&scenario_id)?
        .with_context(|| format!("scenario '{}' not found", scenario_id))?;
    let packs_with_skills = store.get_packs_with_skills_for_agent(tool_key)?;
    let vault_root = central_repo::skills_dir();

    let mut results = Vec::new();
    for adapter in adapters {
        let skills_dir = adapter.skills_dir();

        let mut excluded: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (_pack, skills) in &packs_with_skills {
            for skill in skills {
                let adapter_keys = vec![adapter.key.clone()];
                store.ensure_scenario_skill_tool_defaults(&scenario_id, &skill.id, &adapter_keys)?;
                let enabled = store.get_enabled_tools_for_scenario_skill(&scenario_id, &skill.id)?;
                if !enabled.contains(&adapter.key) {
                    excluded.insert(skill.name.clone());
                }
            }
        }

        let pack_views: Vec<sync_engine::disclosure::PackWithSkills> = packs_with_skills
            .iter()
            .map(|(p, s)| sync_engine::disclosure::PackWithSkills { pack: p, skills: s.as_slice() })
            .collect();

        let report = sync_engine::reconcile_agent_dir(
            &skills_dir,
            &pack_views,
            scenario.disclosure_mode,
            &vault_root,
            &excluded,
        )?;
        results.push((adapter.display_name.clone(), report.added));
    }

    Ok(results)
}
```

- [ ] **Step 2: Build to verify it compiles**

Run: `cargo build -p skills-manager-cli`

Expected: Build succeeds.

- [ ] **Step 3: Run CLI tests**

Run: `cargo test -p skills-manager-cli --no-fail-fast`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/skills-manager-cli/src/commands.rs
git commit -m "feat(cli): sync_agent uses reconcile_agent_dir"
```

---

## Task 7: Replace CLI `unsync_scenario` with `unreconcile_agent_dir`

**Files:**
- Modify: `crates/skills-manager-cli/src/commands.rs` (function `unsync_scenario`, lines ~721-768)

- [ ] **Step 1: Replace the `unsync_scenario` body**

Replace the existing `unsync_scenario` function with:

```rust
fn unsync_scenario(
    _store: &SkillStore,
    _scenario_id: &str,
    adapters: &[tool_adapters::ToolAdapter],
    _configured_mode: Option<&str>,
) -> Result<()> {
    for adapter in adapters {
        let skills_dir = adapter.skills_dir();
        if !skills_dir.exists() {
            continue;
        }
        sync_engine::unreconcile_agent_dir(&skills_dir)?;
    }
    Ok(())
}
```

The previous logic walked the directory and matched on copy/symlink mode. `unreconcile_agent_dir` uses the `is_sm_managed` heuristic which already handles both symlinks and `pack-*` router dirs.

- [ ] **Step 2: Build to verify it compiles**

Run: `cargo build -p skills-manager-cli`

Expected: Build succeeds. Some imports may be unused (`std::collections::HashSet` perhaps); follow compiler warnings.

- [ ] **Step 3: Run CLI tests**

Run: `cargo test -p skills-manager-cli --no-fail-fast`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/skills-manager-cli/src/commands.rs
git commit -m "feat(cli): unsync_scenario uses unreconcile_agent_dir"
```

---

## Task 8: Add CLI `scenario set-mode` and `pack set-essential` commands

**Files:**
- Modify: `crates/skills-manager-cli/src/main.rs`
- Modify: `crates/skills-manager-cli/src/commands.rs`

- [ ] **Step 1: Add new subcommand variants in `main.rs`**

In the `Commands` enum (after `Pack { ... }` around line 60), add:

```rust
    /// Manage scenarios (set disclosure mode)
    Scenario {
        #[command(subcommand)]
        action: ScenarioAction,
    },
```

In the `PackAction` enum (after `EvalRouters` around line 140), add:

```rust
    /// Mark a pack as essential (loaded in hybrid mode) or not
    SetEssential {
        /// Pack name
        name: String,
        /// "true" to mark essential, "false" to unmark
        value: String,
    },
```

After the `AgentAction` enum, add a new enum:

```rust
#[derive(Subcommand)]
enum ScenarioAction {
    /// Set the disclosure mode for a scenario
    SetMode {
        /// Scenario name
        name: String,
        /// Disclosure mode: full | hybrid | router_only
        mode: String,
    },
}
```

In the `match cli.command { ... }` block in `main()`, add:

```rust
        Commands::Scenario { action } => match action {
            ScenarioAction::SetMode { name, mode } => commands::cmd_scenario_set_mode(&name, &mode),
        },
```

In the `match action { ... }` block under `Commands::Pack`, add:

```rust
            PackAction::SetEssential { name, value } => commands::cmd_pack_set_essential(&name, &value),
```

- [ ] **Step 2: Add the two command handlers in `commands.rs`**

Append to `commands.rs` (near the other pack/scenario helpers):

```rust
pub fn cmd_scenario_set_mode(name: &str, mode: &str) -> Result<()> {
    let store = open_store()?;
    let scenario = find_scenario_by_name(&store, name)?;
    // Validate mode parses cleanly.
    let _parsed = skills_manager_core::skill_store::DisclosureMode::parse(mode)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    store.set_scenario_disclosure_mode(&scenario.id, mode)?;
    println!("Scenario '{}' disclosure mode set to '{}'.", scenario.name, mode);
    Ok(())
}

pub fn cmd_pack_set_essential(name: &str, value: &str) -> Result<()> {
    let store = open_store()?;
    let pack = find_pack_by_name(&store, name)?;
    let essential = match value.to_lowercase().as_str() {
        "true" | "yes" | "1" => true,
        "false" | "no" | "0" => false,
        other => anyhow::bail!("invalid value '{}': expected true|false", other),
    };
    store.set_pack_essential(&pack.id, essential)?;
    println!(
        "Pack '{}' is_essential set to {}.",
        pack.name, essential
    );
    Ok(())
}
```

If `DisclosureMode::parse` is not publicly re-exported, the import path may need adjustment. Verify: `grep -n 'pub fn parse' crates/skills-manager-core/src/skill_store.rs`. The function is `impl DisclosureMode { pub fn parse(...) -> Result<...> }` so the call above is correct.

- [ ] **Step 3: Build and verify CLI shows the new commands**

Run: `cargo build -p skills-manager-cli && ./target/debug/sm scenario --help && ./target/debug/sm pack set-essential --help`

Expected: Both new subcommands appear in help output without errors.

- [ ] **Step 4: Commit**

```bash
git add crates/skills-manager-cli/src/main.rs crates/skills-manager-cli/src/commands.rs
git commit -m "feat(cli): add scenario set-mode and pack set-essential"
```

---

## Task 9: CLI integration tests for end-to-end PD behavior

**Files:**
- Create: `crates/skills-manager-cli/tests/pd_wiring.rs`

This test exercises the binary end-to-end against a temporary HOME so it touches the real DB code path but no user data.

- [ ] **Step 1: Write the integration tests**

Create `crates/skills-manager-cli/tests/pd_wiring.rs`:

```rust
//! End-to-end PD wiring integration tests.
//! These spawn the `sm` binary against a temporary HOME so we exercise the
//! real DB and sync paths without touching user data.

use std::path::PathBuf;
use std::process::Command;

fn sm_bin() -> PathBuf {
    // `cargo test` puts integration test binaries next to the binaries they test.
    let mut p = std::env::current_exe().unwrap();
    p.pop(); // drop the test binary name
    if p.ends_with("deps") {
        p.pop();
    }
    p.join("sm")
}

fn run_sm(home: &std::path::Path, args: &[&str]) -> (bool, String, String) {
    let output = Command::new(sm_bin())
        .args(args)
        .env("HOME", home)
        .env("SM_DB_DIR", home.join(".skills-manager"))
        .output()
        .expect("failed to run sm");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Build a minimal seeded skills-manager DB + vault under `home`.
/// Returns once the DB is populated with: 1 essential pack ("base") with 1 skill,
/// 1 non-essential pack ("marketing") with 1 skill, 1 scenario containing both,
/// 1 managed claude_code agent assigned to that scenario.
fn seed_test_state(home: &std::path::Path) {
    use skills_manager_core::skill_store::{
        DisclosureMode, PackRecord, ScenarioRecord, SkillRecord, SkillStore,
    };
    use skills_manager_core::AgentConfigRecord;

    std::fs::create_dir_all(home.join(".skills-manager/skills")).unwrap();
    let db_path = home.join(".skills-manager/skills-manager.db");
    let store = SkillStore::new(&db_path).unwrap();

    let now = chrono::Utc::now().timestamp_millis();

    // Vault: two skill dirs.
    for name in ["base-skill", "mkt-skill"] {
        let dir = home.join(".skills-manager/skills").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), format!("---\nname: {name}\ndescription: x\n---\n")).unwrap();
    }

    let mk_skill = |id: &str, name: &str| SkillRecord {
        id: id.into(), name: name.into(), description: None,
        source_type: "local".into(), source_ref: None, source_ref_resolved: None,
        source_subpath: None, source_branch: None, source_revision: None,
        remote_revision: None,
        central_path: home.join(".skills-manager/skills").join(name).to_string_lossy().into(),
        content_hash: None, enabled: true, created_at: now, updated_at: now,
        status: "active".into(), update_status: "idle".into(),
        last_checked_at: None, last_check_error: None,
    };
    store.insert_skill(&mk_skill("sk-base", "base-skill")).unwrap();
    store.insert_skill(&mk_skill("sk-mkt", "mkt-skill")).unwrap();

    let mk_pack = |id: &str, name: &str, essential: bool, sort: i32| PackRecord {
        id: id.into(), name: name.into(), description: None,
        icon: None, color: None, sort_order: sort,
        created_at: now, updated_at: now,
        router_description: Some(format!("Router description for {name}")),
        router_body: None, is_essential: essential, router_updated_at: Some(now),
    };
    store.insert_pack(&mk_pack("p-base", "base", true, 0)).unwrap();
    store.insert_pack(&mk_pack("p-mkt", "marketing", false, 1)).unwrap();
    store.add_skill_to_pack("p-base", "sk-base", 0).unwrap();
    store.add_skill_to_pack("p-mkt", "sk-mkt", 0).unwrap();

    let scenario = ScenarioRecord {
        id: "sc-test".into(), name: "test-scenario".into(),
        description: None, icon: None, sort_order: 0,
        created_at: now, updated_at: now,
        disclosure_mode: DisclosureMode::Full,
    };
    store.insert_scenario(&scenario).unwrap();
    store.add_pack_to_scenario("sc-test", "p-base").unwrap();
    store.add_pack_to_scenario("sc-test", "p-mkt").unwrap();
    store.set_active_scenario("sc-test").unwrap();

    // Mark the claude_code agent managed and assigned to the scenario.
    store.set_agent_scenario("claude_code", "sc-test").unwrap();
    let _ = store.set_agent_managed("claude_code", true);

    // Pre-create the agent skills dir so reconcile has a target.
    std::fs::create_dir_all(home.join(".claude/skills")).unwrap();
}

#[test]
fn set_mode_persists_to_db() {
    let tmp = tempfile::tempdir().unwrap();
    seed_test_state(tmp.path());

    let (ok, out, err) = run_sm(tmp.path(), &["scenario", "set-mode", "test-scenario", "hybrid"]);
    assert!(ok, "set-mode failed: {err}\n{out}");
    assert!(out.contains("hybrid"));
}

#[test]
fn switch_to_hybrid_creates_pack_routers() {
    let tmp = tempfile::tempdir().unwrap();
    seed_test_state(tmp.path());

    let (ok, _, err) = run_sm(tmp.path(), &["scenario", "set-mode", "test-scenario", "hybrid"]);
    assert!(ok, "set-mode failed: {err}");

    let (ok, _, err) = run_sm(tmp.path(), &["switch", "claude_code", "test-scenario"]);
    assert!(ok, "switch failed: {err}");

    let claude_skills = tmp.path().join(".claude/skills");
    assert!(claude_skills.join("base-skill").exists(), "essential skill should be materialized");
    assert!(claude_skills.join("pack-marketing/SKILL.md").exists(), "router should be created");
    let router = std::fs::read_to_string(claude_skills.join("pack-marketing/SKILL.md")).unwrap();
    assert!(router.contains("Router description for marketing"));
    assert!(!claude_skills.join("mkt-skill").exists(), "non-essential skill should NOT be materialized in hybrid");
}

#[test]
fn switch_from_hybrid_to_full_removes_routers() {
    let tmp = tempfile::tempdir().unwrap();
    seed_test_state(tmp.path());

    run_sm(tmp.path(), &["scenario", "set-mode", "test-scenario", "hybrid"]);
    run_sm(tmp.path(), &["switch", "claude_code", "test-scenario"]);
    assert!(tmp.path().join(".claude/skills/pack-marketing").exists());

    run_sm(tmp.path(), &["scenario", "set-mode", "test-scenario", "full"]);
    let (ok, _, err) = run_sm(tmp.path(), &["switch", "claude_code", "test-scenario"]);
    assert!(ok, "switch back to full failed: {err}");

    assert!(!tmp.path().join(".claude/skills/pack-marketing").exists(),
        "router should be removed after switching back to full");
    assert!(tmp.path().join(".claude/skills/base-skill").exists());
    assert!(tmp.path().join(".claude/skills/mkt-skill").exists(),
        "non-essential skill should now be materialized in full mode");
}

#[test]
fn set_essential_persists_to_db() {
    let tmp = tempfile::tempdir().unwrap();
    seed_test_state(tmp.path());

    let (ok, out, err) = run_sm(tmp.path(), &["pack", "set-essential", "marketing", "true"]);
    assert!(ok, "set-essential failed: {err}");
    assert!(out.contains("true"));
}
```

Add the test deps to `crates/skills-manager-cli/Cargo.toml` under `[dev-dependencies]` if not already present:

```toml
[dev-dependencies]
tempfile = "3"
chrono = "0.4"
skills-manager-core = { path = "../skills-manager-core" }
```

(Run `grep -A 5 "dev-dependencies" crates/skills-manager-cli/Cargo.toml` first; only add what's missing.)

The integration tests use `SM_DB_DIR` to redirect the DB location. **Verify this env var is supported** by `central_repo::base_dir()`:

Run: `grep -n 'SM_DB_DIR\|env::var' crates/skills-manager-core/src/central_repo.rs`

If `SM_DB_DIR` is NOT honored, modify `central_repo::base_dir()` to check for it before the platform default, OR change the test to set `HOME` only and rely on it. The simplest check: see what `base_dir()` returns under `HOME=tmp_dir` and adjust the test to expect `<tmp>/.skills-manager/...`.

- [ ] **Step 2: Run the integration tests to verify they fail or pass as expected**

Run: `cargo test -p skills-manager-cli --test pd_wiring --no-fail-fast`

Expected: PASS for all four tests. If any test fails because `SM_DB_DIR` isn't honored, fix the central_repo to honor it (one-line change) and re-run.

- [ ] **Step 3: Commit**

```bash
git add crates/skills-manager-cli/tests/pd_wiring.rs crates/skills-manager-cli/Cargo.toml
git commit -m "test(cli): integration tests for PD sync wiring"
```

If `central_repo.rs` was modified, include it in the same commit:

```bash
git add crates/skills-manager-core/src/central_repo.rs
git commit --amend --no-edit
```

---

## Task 10: Wire `reconcile_agent_dir` into Tauri `sync_agent_skills`

**Files:**
- Modify: `src-tauri/src/commands/scenarios.rs` (function `sync_agent_skills`, lines 430-501)

- [ ] **Step 1: Replace the `sync_agent_skills` body**

Replace the existing function with:

```rust
pub(crate) fn sync_agent_skills(
    store: &SkillStore,
    tool_key: &str,
) -> Result<(), AppError> {
    let adapter = match tool_adapters::find_adapter_with_store(store, tool_key) {
        Some(a) if a.is_installed() => a,
        _ => return Ok(()),
    };

    let agent_config = store.get_agent_config(tool_key).map_err(AppError::db)?;
    let scenario_id = match agent_config.and_then(|c| c.scenario_id) {
        Some(id) => id,
        None => return Ok(()),
    };
    let scenario = match store.get_scenario(&scenario_id).map_err(AppError::db)? {
        Some(s) => s,
        None => return Ok(()),
    };

    let packs_with_skills = store
        .get_packs_with_skills_for_agent(tool_key)
        .map_err(AppError::db)?;
    let vault_root = crate::core::central_repo::skills_dir();

    // Build excluded set from per-skill tool toggles.
    let mut excluded: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (_pack, skills) in &packs_with_skills {
        for skill in skills {
            let adapter_keys = vec![adapter.key.clone()];
            store
                .ensure_scenario_skill_tool_defaults(&scenario_id, &skill.id, &adapter_keys)
                .map_err(AppError::db)?;
            let enabled = store
                .get_enabled_tools_for_scenario_skill(&scenario_id, &skill.id)
                .map_err(AppError::db)?;
            if !enabled.contains(&adapter.key) {
                excluded.insert(skill.name.clone());
            }
        }
    }

    let pack_views: Vec<sync_engine::disclosure::PackWithSkills> = packs_with_skills
        .iter()
        .map(|(p, s)| sync_engine::disclosure::PackWithSkills { pack: p, skills: s.as_slice() })
        .collect();

    let report = sync_engine::reconcile_agent_dir(
        &adapter.skills_dir(),
        &pack_views,
        scenario.disclosure_mode,
        &vault_root,
        &excluded,
    )
    .map_err(AppError::other)?;

    log::info!(
        "Synced {} entries to {} (mode: {:?})",
        report.added,
        adapter.display_name,
        scenario.disclosure_mode
    );
    Ok(())
}
```

Note: The previous implementation also recorded each materialized skill into the `targets` table (`store.insert_target(...)`). With reconcile, that per-skill bookkeeping no longer reflects what was actually written (routers aren't in `targets`). The targets table is used by `unsync_agent_skills` and possibly UI displays. If needed, we can re-add target tracking inside reconcile, but for this task we drop it — `unreconcile_agent_dir` doesn't need targets table (uses filesystem heuristic). Audit follow-up: check if any Tauri command reads `get_all_targets` for UI display — if yes, file as a separate cleanup.

If `AppError` doesn't have a `::other` constructor, search for the right variant:

Run: `grep -n 'pub fn\|pub enum AppError\|impl AppError' src-tauri/src/error.rs src-tauri/src/lib.rs 2>/dev/null`

Use the appropriate constructor (likely `AppError::from(e.to_string())` or similar).

- [ ] **Step 2: Build to verify it compiles**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

Expected: Build succeeds. Resolve any import errors:
- `use crate::core::sync_engine` likely already at file top
- May need `use std::collections::HashSet;`

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/scenarios.rs
git commit -m "feat(tauri): sync_agent_skills uses reconcile_agent_dir"
```

---

## Task 11: Wire `unreconcile_agent_dir` into Tauri `unsync_agent_skills`

**Files:**
- Modify: `src-tauri/src/commands/scenarios.rs` (function `unsync_agent_skills`, lines 504-527)

- [ ] **Step 1: Replace the `unsync_agent_skills` body**

Replace with:

```rust
pub(crate) fn unsync_agent_skills(
    store: &SkillStore,
    tool_key: &str,
) -> Result<(), AppError> {
    let adapter = match tool_adapters::find_adapter_with_store(store, tool_key) {
        Some(a) => a,
        None => return Ok(()),
    };
    let skills_dir = adapter.skills_dir();
    if skills_dir.exists() {
        sync_engine::unreconcile_agent_dir(&skills_dir).map_err(AppError::other)?;
    }
    // Also clear stale targets table rows for this tool.
    let all_targets = store.get_all_targets().map_err(AppError::db)?;
    for target in &all_targets {
        if target.tool != tool_key {
            continue;
        }
        if let Err(e) = store.delete_target(&target.skill_id, &target.tool) {
            log::warn!("Failed to delete target record: {e}");
        }
    }
    Ok(())
}
```

Same caveat as Task 10 about `AppError::other` — adjust to whatever variant exists.

- [ ] **Step 2: Build to verify it compiles**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/scenarios.rs
git commit -m "feat(tauri): unsync_agent_skills uses unreconcile_agent_dir"
```

---

## Task 12: Manual end-to-end acceptance walkthrough

**Files:** None modified. This is a verification step.

- [ ] **Step 1: Build everything fresh**

Run: `cargo build -p skills-manager-cli && cargo build --manifest-path src-tauri/Cargo.toml`

Expected: Both build cleanly.

- [ ] **Step 2: Run the full test suite**

Run: `cargo test --workspace --no-fail-fast`

Expected: All tests pass.

- [ ] **Step 3: Mark `base` pack essential**

Run: `./target/debug/sm pack set-essential base true`

Expected: `Pack 'base' is_essential set to true.`

Verify: `sqlite3 ~/.skills-manager/skills-manager.db "SELECT name, is_essential FROM packs;"` shows `base|1`, others `0`.

- [ ] **Step 4: Set scenario to hybrid**

Run: `./target/debug/sm scenario set-mode standard-marketing hybrid`

Expected: `Scenario 'standard-marketing' disclosure mode set to 'hybrid'.`

- [ ] **Step 5: Switch claude_code to that scenario**

Run: `./target/debug/sm switch claude_code standard-marketing`

Expected: switch reports succeed.

- [ ] **Step 6: Verify materialized state**

Run:

```bash
ls ~/.claude/skills/ | grep -E '^pack-|^base-' | head -20
cat ~/.claude/skills/pack-marketing/SKILL.md
```

Expected:
- Multiple `pack-*` directories exist (one per non-essential pack assigned to standard-marketing)
- `base` pack's skills are materialized (symlinks)
- `pack-marketing/SKILL.md` shows frontmatter with the marketing router description set in the previous session, plus an auto-rendered table of marketing skills with vault paths
- Non-essential pack skills (e.g. individual marketing skills) are NOT present as direct entries

- [ ] **Step 7: Switch back to `everything` (full mode)**

Run: `./target/debug/sm switch claude_code everything`

Expected: switch reports succeed.

Verify:

```bash
ls ~/.claude/skills/ | grep '^pack-' | wc -l
```

Expected: `0` — all `pack-*` router dirs removed.

- [ ] **Step 8: Update PROGRESS.md to mark wiring complete**

Edit `PROGRESS.md`, change:

```
### PD Sync Wiring 🔄
**Status:** Starting (2026-04-20)
```

to:

```
### PD Sync Wiring ✅
**Status:** Merged (PR pending)
**Completed:** 2026-04-20
```

Commit:

```bash
git add PROGRESS.md
git commit -m "docs: PD sync wiring complete"
```

- [ ] **Step 9: Optional — verify in a fresh Claude Code session**

Open a new terminal in any project, run `claude`, and ask a marketing-flavored question (e.g. "draft a PRD for a new feature"). Confirm:
- The `pack-marketing` router description appears in the available-skills list (rather than the four individual marketing skill descriptions)
- Invoking the router returns the auto-rendered skill table with vault paths
- Reading one of the vault paths returns the real skill content

If this works, PD is fully wired end-to-end.

---

## Self-Review

**Spec coverage check** — every Goal in the spec has a task:

1. ✅ disclosure_mode controls materialization → Tasks 5, 6, 10
2. ✅ hybrid produces essential skills + routers → Task 4 (engine), validated in Task 9 (integration test) and Task 12 (manual e2e)
3. ✅ router_only produces routers only → Task 4 logic intact (uses existing engine), validated in Task 9 by extension
4. ✅ full mode unchanged → No engine logic change for full path; Task 9 includes round-trip test
5. ✅ per-skill tool toggles preserved → Task 3 (engine), Tasks 5/6/10 (call sites build excluded set)
6. ✅ unsync removes router dirs → Task 4 (`unreconcile_agent_dir`), Tasks 7/11 (call sites)
7. ✅ CLI `set-mode` exposed → Task 8
8. ✅ `base` pack essential + visible e2e → Task 8 adds `set-essential`, Task 12 step 3 marks it, Task 12 step 6 verifies

**Placeholder scan**: No "TBD", "TODO", or "implement later" in tasks. Two notes describe verification activities the engineer must perform during implementation (`grep` for `AppError` constructors, check `SM_DB_DIR` support); these are not placeholders but justified context-checks because the surrounding code may have evolved.

**Type consistency**:
- `get_packs_with_skills_for_scenario`/`_for_agent` return `Vec<(PackRecord, Vec<SkillRecord>)>` — used consistently in Tasks 5, 6, 10
- `excluded_skills: &HashSet<String>` (skill names, not ids) — consistent across Tasks 3, 4, 5, 6, 10
- `reconcile_agent_dir` and `unreconcile_agent_dir` signatures defined in Task 4, called identically downstream
- `DisclosureMode::parse` used in Task 8 — verified to exist

**Decomposition check**: Each task is self-contained, has its own commit, and the build stays green between tasks. Tasks 1–4 are pure additions (no caller changes yet). Tasks 5–7 update CLI sync paths in three commits so a regression can be bisected. Tasks 10–11 update Tauri sync paths (separate from CLI). Task 9 is the integration-test gate before touching Tauri. Task 12 is acceptance-only.
