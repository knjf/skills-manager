# Overnight Execution Plan — Ship PD + Clean House

**Start:** 2026-04-20 ~04:50 local
**Goal on wake:** PR #68 merged to main; repo clean; app boots cleanly; Version History branch rebased; follow-up tickets filed.

**Decisions (delegated to me):**
- Q2 → **Option A** (ship PD first, then rebase Version History to v10)
- Q3 → Clean worktrees after PR #68 merges
- Q4 → File GitHub issues for follow-ups (proper repo hygiene)

**Cannot do (need user):** Interactive GUI testing. I will verify via build + headless run + DB schema assertions. UI regressions that only appear at runtime (layout breakage, missing event handlers) must be caught later by human.

---

## Phase 1 — Verify app boots cleanly from clean DB state

Root cause of last boot failure: DB had `user_version=9` but column missing (corrupted intermediate state from earlier dev iteration). I reset to v8 — next launch will re-run migration.

- [x] 1.1 Confirm DB state: `user_version=8`, no `disclosure_mode` column
- [x] 1.2 `cargo build --workspace` clean
- [x] 1.3 Launch Tauri app in background for ~20s, capture stdout, kill cleanly; confirm no panic + migration line appears
- [x] 1.4 Re-inspect DB: `user_version=9`, `disclosure_mode` column exists, default `'full'` on existing scenarios
- [x] 1.5 Run all 6 scenarios default present; `essential` pack `is_essential=1`
- [x] 1.6 Vite production build: `pnpm run build` clean (no TS errors blocking)
- [x] 1.7 Full workspace test: `cargo test --workspace` green

## Phase 2 — Fix any bugs surfaced

- [x] 2.1 Re-run `pnpm exec tsc -b --noEmit`; fix any remaining errors introduced by PD
- [x] 2.2 If 1.3 panicked: diagnose, fix, loop back
- [x] 2.3 Commit + push any fixes to `docs/skill-pack-taxonomy`

## Phase 3 — Merge PR #68

- [x] 3.1 Final `cargo test --workspace` + `pnpm exec tsc -b --noEmit` green
- [x] 3.2 Update PR #68 body with smoke-test evidence + known limitations
- [x] 3.3 `gh pr merge 68 --squash --delete-branch` (or `--merge` if squash undesired)
- [x] 3.4 `git checkout main && git pull origin main && git log -1`

## Phase 4 — Clean up agent worktrees + branches

- [x] 4.1 List all agent worktrees (`.claude/worktrees/agent-*`)
- [x] 4.2 `git worktree remove --force <path>` each one
- [x] 4.3 Delete local branches: `worktree-agent-*` and `feat/pd-*`
- [x] 4.4 `git remote prune origin`
- [x] 4.5 Verify `git worktree list` shows only main + superconductor worktree

## Phase 5 — Rebase Skill Version History onto new main (bumps DB v9→v10)

- [x] 5.1 Enter `/Users/jfkn/projects/skills-manager` worktree; verify branch `feat/skill-version-history`
- [x] 5.2 Inspect commits: migration file, schema changes, claimed DB version
- [x] 5.3 `git fetch origin && git rebase origin/main` (pulls PD + bumps base)
- [x] 5.4 Resolve conflicts:
  - [x] `migrations.rs`: skill-version-history had its own v8→v9 migration. Rename to v9→v10 (bump LATEST_VERSION to 10).
  - [x] Other: address case-by-case
- [x] 5.5 `cargo test --workspace` green on rebased branch
- [x] 5.6 Push with lease: `git push --force-with-lease origin feat/skill-version-history`
- [x] 5.7 Open PR (title + body): `gh pr create --base main` → WAIT for user approval before merging (scope judgment — user hasn't reviewed this feature yet)

## Phase 6 — File follow-up GitHub issues

Each issue title + body. Leave for user to prioritize.

- [x] 6.1 Issue: "Frontend test harness (vitest) not wired — RouterEditor tests authored but can't run"
- [x] 6.2 Issue: "Agent Native Skills marker integration with `is_sm_managed` heuristic (PD sync engine)"
- [x] 6.3 Issue: "regen-all-routers N+1 store-open perf"
- [x] 6.4 Issue: "PluginsView.tsx pre-existing TS errors (ManagedPlugin.name missing)"
- [x] 6.5 Issue: "Scenario editor token estimate uses placeholder 0/0 counts"
- [x] 6.6 Issue: "Tokens-saved widget on Dashboard needs real is_essential + skill_count fields in TS Pack type"
- [x] 6.7 Issue: "Optional `description_short` field for UI tooltips (PD future work)"

## Phase 7 — Write wake-up report

- [x] 7.1 Summarize what was done + what's left at `docs/superpowers/plans/2026-04-20-wake-up-report.md`
- [x] 7.2 Include: merged PR #68 SHA, new main tip, skill-version-history PR URL (if opened), GitHub issue URLs, any known-open bugs
- [x] 7.3 Commit report + push

---

## Safety rails

- **Never force-push to main.** Always work on feature branches; use `gh pr merge` for integration.
- **Never skip hooks** (no `--no-verify`).
- If any Phase 1 check fails with a real bug: fix, re-verify, proceed. Don't merge a broken PR.
- If Phase 5 rebase conflicts are large/unclear: abort rebase, leave branch untouched, file issue for user to resolve manually.
- If `cargo test` regresses between Phase 3.1 and 3.3: don't merge.

## Blocker handling

| Symptom | Action |
|---|---|
| Tauri panics on startup after reset | Check migration logs; if v8→v9 fn is wrong, fix; if DB path mis-resolved, investigate `central_repo::db_path()` |
| TS errors block Vite build | Revert or fix per-error |
| Skill version history rebase impossible | Abort, file "Rebase skill-version-history onto main" issue, leave branch |
| PR #68 merge fails due to protection rules | Document in wake-up report; don't force |
