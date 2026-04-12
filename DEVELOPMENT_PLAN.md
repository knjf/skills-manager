# Skills Manager Fork — Development Plan

## Background

Fork of [xingkongliang/skills-manager](https://github.com/xingkongliang/skills-manager) (Rust + Tauri + React/TypeScript).

Forked to: [knjf/skills-manager](https://github.com/knjf/skills-manager)

Local path: `~/projects/skills-manager/`

### 點解要 Fork

原版 Skills Manager 有以下限制：
1. **場景 = 一個扁平 skill list** — 冇 "pack" 概念，加減 skills 要逐個操作
2. **唔管 plugin skills** — plugin 永遠全部載入，冇辦法 enable/disable
3. **冇 CLI** — 只有 GUI，AI agent 無法自動切換場景
4. **Per-agent toggle 存在但唔好用** — 要逐個 skill 逐個 agent 撥，冇批量操作

---

## 新功能規劃

### Feature 1: Skill Packs

**概念**：Pack = 一組相關 skills 嘅集合。場景由多個 packs 組成。

#### 例子

| Pack | Skills 數 | 內容 |
|------|----------|------|
| `base` | 15 | skill-retrieval, web-access, readwise-cli, opencli 等 |
| `gstack` | 37 | gstack 全套開發流程 |
| `agent-orchestration` | 7 | paseo*, paperclip |
| `browser-tools` | 8 | agent-browser, opencli-*, x-tweet-fetcher 等 |
| `research` | 5 | last30days, perp-search, autoresearch, follow-builders, feed-catchup |
| `design` | 10 | stitch-design, frontend-design, shadcn-ui 等 |
| `knowledge` | 7 | obsidian-cli, notebooklm, readwise-to-notebooklm 等 |
| `marketing` | 35 | 完整 marketing 套件 |
| `ops` | 5 | claude-code-router, yt-dlp 等 |

#### 場景 = Pack 組合

| Scenario | Packs |
|----------|-------|
| minimal | base |
| core | base + gstack |
| standard | base + gstack + agent-orchestration + browser-tools + research + ops |
| full-dev | standard + design + knowledge |
| standard-marketing | standard + marketing |
| full-dev-marketing | full-dev + marketing |
| everything | all packs |

#### DB Schema 改動

```sql
-- New table: packs
CREATE TABLE packs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    icon TEXT,
    color TEXT,              -- UI display color
    sort_order INTEGER DEFAULT 0,
    created_at INTEGER,
    updated_at INTEGER
);

-- New table: pack_skills (which skills belong to which pack)
CREATE TABLE pack_skills (
    pack_id TEXT NOT NULL REFERENCES packs(id) ON DELETE CASCADE,
    skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    PRIMARY KEY(pack_id, skill_id)
);

-- New table: scenario_packs (which packs are in which scenario)
CREATE TABLE scenario_packs (
    scenario_id TEXT NOT NULL REFERENCES scenarios(id) ON DELETE CASCADE,
    pack_id TEXT NOT NULL REFERENCES packs(id) ON DELETE CASCADE,
    sort_order INTEGER DEFAULT 0,
    PRIMARY KEY(scenario_id, pack_id)
);
```

場景嘅 skill list = union of all pack skills + 直接 assign 嘅 individual skills（向後兼容 scenario_skills table）。

#### UI 設計

- **Dashboard**：顯示所有 packs 作為 cards，每個 card 有 toggle
- **場景編輯**：drag & drop packs 入場景，或者 checkbox toggle
- **Pack 編輯**：drag & drop skills 入 pack
- **視覺化**：Pack 用顏色區分，場景顯示包含嘅 packs

---

### Feature 2: Plugin Management

**概念**：將 Claude Code plugins 納入 SM 管理，可以 per-scenario enable/disable。

#### 現狀

- Plugins 喺 `~/.claude/plugins/installed_plugins.json` 管理
- Plugin skills 喺 `~/.claude/plugins/cache/<plugin-name>/` 載入
- Plugin 有 scope：user / project / local
- 目前冇辦法 per-scenario 控制 plugin

#### 方案

```sql
-- New table: managed_plugins
CREATE TABLE managed_plugins (
    id TEXT PRIMARY KEY,
    plugin_key TEXT NOT NULL UNIQUE,    -- e.g., "compound-engineering@compound-engineering-plugin"
    display_name TEXT,
    scope TEXT,                         -- user / project / local
    install_path TEXT,
    skill_count INTEGER DEFAULT 0,
    agent_count INTEGER DEFAULT 0,
    token_estimate INTEGER DEFAULT 0,   -- estimated tokens consumed
    created_at INTEGER,
    updated_at INTEGER
);

-- New table: scenario_plugins (per-scenario plugin toggle)
CREATE TABLE scenario_plugins (
    scenario_id TEXT NOT NULL REFERENCES scenarios(id) ON DELETE CASCADE,
    plugin_id TEXT NOT NULL REFERENCES managed_plugins(id) ON DELETE CASCADE,
    enabled INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY(scenario_id, plugin_id)
);
```

#### Plugin enable/disable 機制

Option A（推薦）：修改 `installed_plugins.json` 嘅 scope 或加 `disabled` flag
Option B：場景切換時 rename plugin cache dir（`cache/plugin` → `cache/.disabled-plugin`）
Option C：用 Claude Code 嘅 blocklist.json

需要研究 Claude Code 點讀 plugin list，揀最穩嘅方案。

#### UI 設計

- **Settings → Plugins** tab：列出所有已安裝 plugins + token 估算
- **場景編輯**：每個場景可以 toggle plugins on/off
- **Token budget 顯示**：顯示場景嘅預計 token 消耗（skills + plugins + agents）

---

### Feature 3: Per-Agent Visual Toggle

**概念**：用 matrix view 顯示 skills × agents，一眼睇晒邊個 agent 有邊啲 skills。

#### UI 設計

```
                claude_code  cursor  codex  hermes  ...
┌─────────────┬────────────┬───────┬──────┬───────┐
│ base pack   │     ✓      │   ✓   │  ✓   │   ✓   │
│ gstack pack │     ✓      │   ✓   │  ✗   │   ✗   │
│ marketing   │     ✓      │   ✓   │  ✗   │   ✗   │
│ hermes-only │     ✗      │   ✗   │  ✗   │   ✓   │
└─────────────┴────────────┴───────┴──────┴───────┘
```

- Pack-level toggle：一次過 enable/disable 一個 pack 嘅所有 skills for 一個 agent
- Skill-level override：展開 pack 可以逐個 skill toggle
- Agent-level preset：一鍵 "enable all" / "disable all" for 一個 agent

#### 已有嘅 DB support

`scenario_skill_tools` table 已經支持 per-scenario per-skill per-agent toggle。新增：

```sql
-- New table: scenario_pack_tools (pack-level per-agent toggle)
CREATE TABLE scenario_pack_tools (
    scenario_id TEXT NOT NULL,
    pack_id TEXT NOT NULL,
    tool TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY(scenario_id, pack_id, tool)
);
```

---

### Feature 4: CLI Mode

**概念**：加 CLI 子命令，可以從 terminal 操作。

#### 方案

喺 Rust backend 加一個 CLI entry point，共用 core library（sync_engine, skill_store）。

```
skills-manager cli switch <scenario>
skills-manager cli list
skills-manager cli current
skills-manager cli packs [scenario]
skills-manager cli diff <a> <b>
skills-manager cli pack add <pack> <scenario>
skills-manager cli pack remove <pack> <scenario>
```

或者作為獨立 binary：`sm`（現有 shell script 嘅 Rust 版本）。

#### 實現路徑

1. 將 `src-tauri/src/core/` 拆成 standalone crate（`skills-manager-core`）
2. Tauri app 依賴呢個 crate
3. CLI binary 都依賴呢個 crate
4. 共用 DB access、sync logic、pack management

---

## 技術架構

### 現有結構

```
src-tauri/
  src/
    main.rs              # Tauri app entry
    lib.rs               # App setup
    commands/            # Tauri IPC commands
      scenarios.rs       # ← sync logic lives here
      skills.rs
      sync.rs
      tools.rs
      settings.rs
      ...
    core/                # Business logic
      sync_engine.rs     # Symlink/copy sync
      skill_store.rs     # SQLite operations
      tool_adapters.rs   # Agent directory config
      ...
src/                     # React frontend
  views/
    Dashboard.tsx
    MySkills.tsx
    Settings.tsx
    ...
  components/
    ...
```

### 目標結構

```
crates/
  skills-manager-core/   # NEW: standalone core library
    src/
      db.rs              # SQLite schema + migrations
      packs.rs           # Pack CRUD + resolution
      plugins.rs         # Plugin discovery + management
      scenarios.rs       # Scenario + pack composition
      sync.rs            # Sync engine (symlink/copy)
      tools.rs           # Agent adapter config
      cli.rs             # CLI command handlers
  skills-manager-cli/    # NEW: standalone CLI binary
    src/main.rs
src-tauri/               # Tauri app (depends on core)
  src/
    commands/            # Thin wrappers calling core
    ...
src/                     # React frontend
  views/
    PacksView.tsx        # NEW: pack management
    PluginsView.tsx      # NEW: plugin management
    MatrixView.tsx       # NEW: agent × skill matrix
    ...
```

---

## 執行策略

### 依賴圖

```
Phase 1 (core + packs) ← 硬性 blocker，必須先完成
    ├── Phase 2 (CLI)        ─┐
    ├── Phase 3 (plugins)     ├── 用 git worktree 並行開發
    └── Phase 4 (packs UI)   ─┘
                └── Phase 5 (matrix + plugin UI) ← 依賴 Phase 3 + 4
```

### 每個 Feature 嘅開發循環

```
brainstorming → writing-plans → [plan-eng-review if 架構性]
→ executing-plans (TDD: test → implement → verify)
→ simplify → review → git-commit-push-pr
```

Skills 詳細列表見 `CLAUDE.md` → "Development Workflow — Skills Toolkit"。

---

## 開發順序

### Phase 1: Core refactor + Packs（優先）
1. 拆 core library
2. 加 packs schema + migrations
3. 實現 pack CRUD
4. 場景改為 pack-based composition
5. 向後兼容：scenario_skills 仍然 work

### Phase 2: CLI（Phase 1 完成後，可與 3/4 並行）
6. 寫 CLI binary（用 clap）
7. switch / list / current / packs / diff commands
8. 替代現有 shell script `sm`

### Phase 3: Plugin management（Phase 1 完成後，可與 2/4 並行）
9. Plugin discovery（scan installed_plugins.json + plugin cache）
10. Plugin token estimation
11. Per-scenario plugin toggle
12. 研究 + 實現 plugin enable/disable 機制

### Phase 4: Frontend — Packs UI（Phase 1 完成後，可與 2/3 並行）
13. PacksView：pack CRUD
14. 場景編輯改為 pack toggle
15. Dashboard 加 pack cards

### Phase 5: Frontend — Matrix view + Plugin UI（Phase 3 + 4 完成後）
16. MatrixView：agent × pack/skill toggle matrix
17. PluginsView：plugin management
18. Token budget 顯示

---

## 環境設置

### Prerequisites
- Rust toolchain (`rustup`)
- Node.js + pnpm
- Tauri CLI: `cargo install tauri-cli`

### Build & Run
```bash
cd ~/projects/skills-manager
pnpm install
cargo tauri dev          # Dev mode with hot reload
cargo tauri build        # Production build
```

### Database
- Location: `~/.skills-manager/skills-manager.db`
- Migrations: `src-tauri/src/core/migrations.rs`
- Backup: `~/.skills-manager/skills-manager.db.bak-*`

---

## 參考資料

- [Upstream repo](https://github.com/xingkongliang/skills-manager)
- [Tauri docs](https://tauri.app/v2/)
- DB schema: see `src-tauri/src/core/migrations.rs`
- 現有 `sm` CLI script: `~/.local/bin/sm`
- Scenario plan: `~/.skills-manager/scenario-plan.md`
