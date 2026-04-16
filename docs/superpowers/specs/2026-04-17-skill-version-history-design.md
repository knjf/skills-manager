# Skill Version History & Diff — Design Spec

- **Status**: Draft
- **Author**: Jeff Kwan
- **Date**: 2026-04-17
- **Supersedes / Related**: builds on Phase 1 (core crate + skills schema v8)

## 1. Problem & Goal

Skills 會隨時間被 re-import、re-scan、pack update、local edit 改動，用戶依家冇方法睇返：
- 邊幾個版本曾經出現過
- 兩個版本之間嘅差異
- 幾時 import / 幾時改、來自邊個 source

**Goal**：加一個 History tab，俾用戶揀任意 skill → 睇佢所有版本 → 揀兩個版本 side-by-side diff（similar to GitHub），並可 restore 舊版本。

## 2. Scope

### In scope (MVP)

- SQLite-backed snapshot：每次 scan 偵測 `content_hash` 變化就保存一份 SKILL.md 主文 snapshot。
- LRU retention：每 skill 最多保 50 個版本。
- 新 **History** tab（左側 skill list，右側 version list + metadata panel + split-view diff）。
- Restore：揀舊版本寫回 central library 並 re-sync active scenario；restore 本身亦開新版本。
- 首次啟動 backfill 132 個現有 skills 成「version 1」snapshot。

### Out of scope (MVP)

- Diff 超過 SKILL.md 主文（frontmatter 分開顯示、附屬檔案 diff、整個 folder diff）。
- 其他 view（Dashboard / MySkills）嘅「View history」入口。
- 版本 search / blame / annotate。
- Export diff as `.patch` 文件。
- 跨 skill 比較。
- 鍵盤捷徑（phase 2 再計）。

## 3. Architecture Overview

```
┌─────────────────────── Frontend (React) ──────────────────────┐
│  HistoryView.tsx                                              │
│   ├─ SkillListPane   (search + list)                          │
│   ├─ MetadataPanel   (source / imported_at / updated_at...)   │
│   ├─ VersionListPane (timeline, 2 checkboxes max)             │
│   └─ DiffPane        (split view, react-diff-view)            │
└──────────────────────────────┬────────────────────────────────┘
                               │ Tauri IPC
┌──────────────────────────────▼────────────────────────────────┐
│  commands/history.rs                                          │
│   list_skills_with_history / list_versions / get_version /    │
│   diff_versions / restore_version                             │
└──────────────────────────────┬────────────────────────────────┘
                               │
┌──────────────────────────────▼────────────────────────────────┐
│  Core crate                                                   │
│   ├─ version_store.rs     (capture / list / get / restore)    │
│   ├─ diff.rs              (compute_diff via `similar` crate)  │
│   └─ Hooks: scanner, installer, pack_seeder → capture_version │
└──────────────────────────────┬────────────────────────────────┘
                               │
                    SQLite  skill_versions table
```

### Key decisions

- **Snapshot storage**：DB-only (Approach A). TEXT blob inline in `skill_versions.content`. 132 skills × ≤50 versions × ~5KB ≈ 33MB upper bound — acceptable for SQLite.
- **Diff engine**：Rust `similar` crate. 後端計好 structured hunks 俾前端 render，確保 CLI / web / future surfaces 共用邏輯。
- **Frontend diff view**：`react-diff-view` npm package (split mode)。
- **Capture location**：core crate，等 CLI 都自動 benefit（Phase 2 CLI 已存在）。

## 4. Data Model

### 4.1 Schema (Migration v8 → v9)

```sql
CREATE TABLE skill_versions (
    id                  TEXT PRIMARY KEY,           -- uuid v4
    skill_id            TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    version_no          INTEGER NOT NULL,           -- 1,2,3... per skill
    content             TEXT NOT NULL,              -- full SKILL.md text
    content_hash        TEXT NOT NULL,              -- sha256
    byte_size           INTEGER NOT NULL,
    captured_at         INTEGER NOT NULL,           -- unix seconds
    trigger             TEXT NOT NULL,              -- 'scan' | 'import' | 'backfill' | 'restore'
    source_type         TEXT NOT NULL,              -- snapshot of skills.source_type
    source_ref          TEXT,                       -- snapshot
    source_ref_resolved TEXT,                       -- snapshot (e.g. commit SHA)
    UNIQUE(skill_id, version_no)
);

CREATE INDEX idx_skill_versions_skill_captured
    ON skill_versions(skill_id, captured_at DESC);
```

### 4.2 Design rationale

| 欄位 | 原因 |
|------|------|
| `version_no` | 穩定人類友善序號 (v1, v2...) |
| `content` (TEXT inline) | Approach A：~33MB total upper bound，SQLite TEXT BLOB 合適 |
| 唔加 `UNIQUE(skill_id, content_hash)` | Restore 舊版本希望體現為新一行歷史，就算 content 重複舊 snapshot。Dedup 靠 application-level：只同 **latest** version 嘅 hash 比（`capture_version` early-return）。保護唔俾連續兩次 scan 影重複，但容許非連續重複。 |
| `trigger` | Audit trail；UI 可顯示 badge |
| `source_*` snapshot | Restore 時可知舊版本當年嘅 source |
| `ON DELETE CASCADE` | Skill 刪除時 versions 一併清（跟現有 FK pattern） |

### 4.3 LRU eviction

每次成功 `capture_version` 後：

```sql
DELETE FROM skill_versions
 WHERE skill_id = ?1
   AND id NOT IN (
     SELECT id FROM skill_versions
      WHERE skill_id = ?1
      ORDER BY version_no DESC
      LIMIT 50
   );
```

Retention 值由 `settings` table `version_retention` key 讀取（default 50），容後 UI 可改。

## 5. Backend API

### 5.1 Core：`crates/skills-manager-core/src/version_store.rs`

```rust
pub struct VersionRecord {
    pub id: String,
    pub skill_id: String,
    pub version_no: i64,
    pub content_hash: String,
    pub byte_size: i64,
    pub captured_at: i64,
    pub trigger: String,                   // "scan" | "import" | "backfill" | "restore"
    pub source_type: String,
    pub source_ref: Option<String>,
    pub source_ref_resolved: Option<String>,
}

pub struct VersionContent {
    pub record: VersionRecord,
    pub content: String,
}

pub enum CaptureTrigger { Scan, Import, Backfill, Restore }

impl SkillStore {
    /// Insert new version if content_hash differs from the *latest* version
    /// (only compares against latest, not full history — allows restore of
    /// older content to create a new row).
    /// Returns Some(VersionRecord) if captured, None if no-op.
    /// Enforces LRU retention after insert.
    pub fn capture_version(
        &self, skill_id: &str, content: &str, trigger: CaptureTrigger,
    ) -> Result<Option<VersionRecord>>;

    pub fn list_versions(&self, skill_id: &str) -> Result<Vec<VersionRecord>>;
    pub fn get_version(&self, version_id: &str) -> Result<VersionContent>;
    pub fn latest_version(&self, skill_id: &str) -> Result<Option<VersionRecord>>;

    pub fn restore_version(&self, version_id: &str) -> Result<VersionContent>;

    pub fn backfill_initial_versions(&self) -> Result<usize>;
}
```

### 5.2 Core：`crates/skills-manager-core/src/diff.rs`

```rust
pub struct DiffHunk { pub header: String, pub lines: Vec<DiffLine> }

pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    pub text: String,
}

pub enum DiffLineKind { Context, Added, Removed }

pub fn compute_diff(old: &str, new: &str, context: usize) -> Vec<DiffHunk>;
```

Pure Rust via `similar` crate (~2.x). No WASM / JS fallback needed — structured hunks travel via IPC。

### 5.3 Capture hooks

每個「可能改動 central library skill content」嘅路徑都要 call `capture_version`：

| 位置 | Trigger |
|------|---------|
| `scanner.rs::scan_central_library()` 發現 hash 變 | `Scan` |
| `installer.rs::install_git_skill()` / `install_skillssh_skill()` | `Import` |
| `pack_seeder.rs` 寫 skill | `Import` |
| `commands/history.rs::restore_version` | `Restore` |
| Migration backfill (startup job) | `Backfill` |

`capture_version` idempotent：相同 hash 返 `None`，call 多次無副作用。

### 5.4 Tauri commands：`src-tauri/src/commands/history.rs`

```rust
#[tauri::command] list_skills_with_history() -> Vec<SkillHistorySummary>
#[tauri::command] list_versions(skill_id: String) -> Vec<VersionRecord>
#[tauri::command] get_version(version_id: String) -> VersionContent
#[tauri::command] diff_versions(old: String, new: String) -> Vec<DiffHunk>
#[tauri::command] restore_version(version_id: String) -> RestoreResult
```

`SkillHistorySummary`：skill metadata (id / name / source_type / source_ref) + `version_count` + `latest_captured_at` + `first_imported_at`。

`restore_version` 流程：

1. 讀目標 version content。
2. 寫入 central library SKILL.md。
3. `capture_version(..., Restore)`：restore 本身亦開新版本，歷史永唔倒退。
4. Trigger active scenario re-sync（重用 existing sync 邏輯）。
5. 返新 hash / 新 version_no。

### 5.5 Dependencies

- Cargo: `similar = "2"` (pure Rust, small)
- npm: `react-diff-view` + `diff-match-patch` (peer)

## 6. Frontend

### 6.1 Routing + Sidebar

`src/App.tsx` router 加 `/history`；sidebar 加 `History` item（icon：`History` from lucide-react）。

### 6.2 Layout

```
┌───────────────┬──────────────────────────────────────────┐
│ Skill list    │  Metadata strip                          │
│               │  name · source badge · 32 versions ·     │
│ [search]      │  imported 2026-03-02 · updated 2h ago    │
│               ├──────────────────────────────────────────┤
│ ▸ foo         │  Version list (newest → oldest)          │
│ ▸ bar         │  ☑ v32  hash:a1b2  2h ago   scan         │
│   baz         │  ☐ v31  hash:9f7c  Mon      scan         │
│   ...         │  ☐ v30  hash:5e2d  Sun      restore      │
│               │  ☑ v29  hash:3c91  Apr 10   import (git) │
│ 132 skills    │  [Restore older version…]                │
│               ├──────────────────────────────────────────┤
│               │  Split diff                              │
│               │  v29 (Apr 10) │ v32 (2h ago)             │
│               │  ─────────────┼──────────                 │
│               │  - old line   │ + new line               │
└───────────────┴──────────────────────────────────────────┘
```

### 6.3 Components (`src/views/HistoryView.tsx` + children)

| Component | 職責 |
|-----------|------|
| `SkillListPane` | Fuzzy search；sort by latest_captured_at DESC；source-type badge |
| `MetadataPanel` | source icon + ref；version count；imported/updated timestamps；latest hash |
| `VersionListPane` | Row = checkbox + v# + short hash + relative time + trigger badge；max 2 checked；third check evicts earliest |
| `DiffPane` | Needs 2 selections；split mode；empty state 提示；大 diff 用 virtual scroll |

### 6.4 Interactions

- **Default**：揀 skill 時自動 check 最新 + 第二新。
- **Restore**：揀單一非最新版本 → `Restore this version` 按鈕啟用 → confirm modal 警告「會寫入 central library 並 re-sync active scenario」→ 成功後新 `v_max+1 · restore` row 出現。
- **Empty states**：
  - `0 versions`：「呢個 skill 未有歷史紀錄」（理論上 backfill 後唔會出現）。
  - `1 version`：只顯示 metadata + 「未有可對比嘅版本」。
- **Live refresh**：listen `app-files-changed` → debounced `refreshAppData` (reuse AppContext pattern)。

### 6.5 i18n

`src/i18n/{en,zh,zh-TW}.json` 新增 `history.*` keys，跟現有 pattern。

## 7. Migration & Backfill

**Schema migration v8 → v9** 喺 `migrations.rs`：純 `CREATE TABLE` + `CREATE INDEX`。

**Data backfill** 喺 app startup（同 `pack_seeder.rs` 類似 pattern）：

1. 檢測 `PRAGMA user_version = 9 AND SELECT COUNT(*) FROM skill_versions = 0`。
2. Background thread：loop 所有 skills，讀 central library 對應 SKILL.md，`capture_version(..., Backfill)`。
3. 失敗 skill（檔案 missing / 讀錯）記 log，唔 block startup。
4. UI 喺 History tab 初次 render 時顯示「Backfilling history…」直到 job 完成。

## 8. Testing

### 8.1 Rust unit tests

| 測試 | 驗證 |
|------|------|
| `capture_version_inserts_new_row` | 新 hash 插入，`version_no = 1` |
| `capture_version_dedups_same_hash` | 同 hash 返 `None`，DB 冇新 row |
| `capture_version_increments_version_no` | 連續唔同 content → v1, v2, v3 |
| `lru_eviction_at_50` | 插第 51 後最舊嗰版被 delete |
| `skill_delete_cascades_versions` | Delete skill 後 versions 清 |
| `backfill_creates_v1_for_all_skills` | N skills → N rows trigger=`backfill` |
| `restore_version_creates_new_version` | Restore v5（非 latest）→ 新 v_max+1 with same content_hash as v5；dedup 只 check latest，所以成功 insert |
| `restore_latest_is_noop` | Restore 目前已係 latest 嘅版本 → `capture_version` 返 None，唔改 central library |
| `compute_diff_basic` | add/remove/context 正確 |
| `compute_diff_identical_returns_no_hunks` | 同內容 → empty |
| `migrations::v8_to_v9_creates_table` | 舊 DB upgrade 後 table 存在 |

### 8.2 Integration test (src-tauri)

- Fixture skill → scan → edit content → scan → `list_versions` 返 2 rows trigger=`scan`。
- Restore v1 → central library 檔案內容符合 v1 snapshot。

### 8.3 Frontend

- `HistoryView` 核心 flow（選 skill / 選 2 版 / render diff）vitest + RTL。
- gstack `/qa` browser smoke 喺 release gate 跑（唔納入 unit test run）。

### 8.4 TDD discipline

跟 `superpowers:test-driven-development`：每個 phase failing test 先行，implement 到 green。

## 9. Rollout Phases

| Phase | 內容 | Gate |
|-------|------|------|
| 1 | Core `version_store` + `diff` + migration v9 + backfill job | Rust tests pass；舊 DB 升級後 132 rows |
| 2 | Wire capture hooks (scanner / installer / pack_seeder) | 手動 edit → re-scan → DB v2 |
| 3 | Tauri commands | dev console 可 call 所有 commands |
| 4 | Frontend HistoryView + sidebar + i18n | Manual walkthrough：揀 skill → diff 顯示 |
| 5 | Restore 流程 | Manual：restore v1 → central 改 → active scenario sync |
| 6 | Polish + QA | gstack `/qa` browser smoke pass |

每 phase 用 `superpowers:verification-before-completion` gate。Phase 1 完成可獨立 PR，後續合併 / 分拆於 `writing-plans` 決定。

## 10. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| Backfill 132 skills × 讀檔 拖慢 startup | Background thread；UI loading state；失敗 skip + log |
| Restore 同 scan race | Restore 用 transaction；scan idempotent |
| 大 skill diff 慢 | `similar` 提供 deadline option；前端 React.lazy + suspense |
| User 困惑「restore 又開新版」 | Confirm modal 明確寫：「保留原版歷史，建新版本指向呢個 content」 |
| Restore 用戶揀咗 latest（等於 no-op） | `restore_version` 先 check；若 target == latest 則返「already latest」結果，UI disable Restore button when latest selected |

## 11. Open Questions

無。所有 brainstorming 決定已鎖定喺以上各 section。
