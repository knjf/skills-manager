# Three-Tier UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the GUI gap for three-tier Progressive Disclosure — expose `router_when_to_use` and `description_router` through the Tauri desktop app so non-technical users can author L1/L2 without the CLI.

**Architecture:** Add 2 additive Tauri IPC commands wrapping the core `SkillStore` setters. Extend `ManagedSkillDto` + TS types. Extend `RouterEditor` with a `when_to_use` textarea + combined char counter. Add an L2 editor section + sibling list to `SkillDetailPanel`. Add coverage badges + a filter to `PacksView` and `MySkills`. No DB/schema changes — all back-end work shipped in PR #29.

**Tech Stack:** Rust (tauri 2, anyhow, serde), TypeScript + React (Vite + Tailwind), Vitest for frontend tests, existing Tauri command + DTO patterns.

**Spec:** `docs/superpowers/specs/2026-04-21-three-tier-ui-design.md`

---

## File Map

**Modify:**
- `src-tauri/src/commands/packs.rs` — add `set_pack_when_to_use` Tauri command
- `src-tauri/src/commands/skills.rs` — add `set_skill_description_router` command + `description_router` field in `ManagedSkillDto` and all constructors
- `src-tauri/src/lib.rs` — register 2 new commands in `invoke_handler`
- `src/lib/tauri.ts` — add `router_when_to_use` to `PackRecord` TS type; add `description_router` to `ManagedSkill` TS type; add 2 wrapper functions
- `src/components/RouterEditor.tsx` — add `whenToUse` prop + textarea + combined char counter
- `src/views/PacksView.tsx` — pass `whenToUse` through, call new IPC on save, render L1/L2 coverage badges on each pack card
- `src/components/SkillDetailPanel.tsx` — add L2 editor section + sibling list; accept new `sisterSkills` prop + `onSelectSibling` callback
- `src/views/MySkills.tsx` — compute sibling skills per pack, pass to `SkillDetailPanel`, render L2 indicator on each row, add "show only unset" filter

**Create:**
- `src/components/__tests__/RouterEditor.test.tsx` — Vitest tests for extended component
- `src/components/__tests__/SkillDetailPanel.test.tsx` — tests for L2 editor + sibling list

---

## Task 1: Add `set_pack_when_to_use` Tauri command

**Files:**
- Modify: `src-tauri/src/commands/packs.rs`
- Modify: `src-tauri/src/lib.rs` (command registration)

- [ ] **Step 1: Read the existing `set_pack_router` command to mirror its shape**

Run: `grep -A 12 'pub async fn set_pack_router' src-tauri/src/commands/packs.rs`

Expected: a `#[tauri::command]` async function taking `pack_id: String`, optional fields, `store: State<'_, Arc<SkillStore>>`, returning `Result<(), AppError>`, calling `SkillStore::set_pack_router` inside `spawn_blocking`.

- [ ] **Step 2: Add new command**

Append to `src-tauri/src/commands/packs.rs` (after `set_pack_router` function — around line 190):

```rust
#[tauri::command]
pub async fn set_pack_when_to_use(
    pack_id: String,
    text: Option<String>,
    store: State<'_, Arc<SkillStore>>,
) -> Result<(), AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        store
            .set_pack_when_to_use(&pack_id, text.as_deref())
            .map_err(AppError::db)
    })
    .await?
}
```

- [ ] **Step 3: Register the command**

In `src-tauri/src/lib.rs`, find the `tauri::generate_handler![...]` list (around line 418). Find the line `commands::packs::set_pack_router,` and add immediately after:

```rust
            commands::packs::set_pack_when_to_use,
```

- [ ] **Step 4: Build**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

Expected: clean build. Fix any import errors (likely `State` + `Arc` + `AppError` are already imported at top of `packs.rs` — check).

- [ ] **Step 5: Run workspace tests**

Run: `cargo test --workspace --no-fail-fast`

Expected: all pass (no new tests yet — core `set_pack_when_to_use` is already tested in DB v11 PR).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/packs.rs src-tauri/src/lib.rs
git commit -m "feat(tauri): set_pack_when_to_use IPC command"
```

---

## Task 2: Add `set_skill_description_router` command + extend `ManagedSkillDto`

**Files:**
- Modify: `src-tauri/src/commands/skills.rs` — add field + command + update all mappers
- Modify: `src-tauri/src/lib.rs` — register command

- [ ] **Step 1: Add `description_router` to `ManagedSkillDto`**

In `src-tauri/src/commands/skills.rs`, find the struct (around line 26). Change:

```rust
#[derive(Debug, Serialize)]
pub struct ManagedSkillDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub source_ref_resolved: Option<String>,
    pub source_subpath: Option<String>,
    pub source_branch: Option<String>,
    pub source_revision: Option<String>,
    pub remote_revision: Option<String>,
    pub update_status: String,
    pub last_checked_at: Option<i64>,
    pub last_check_error: Option<String>,
    pub central_path: String,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub status: String,
    pub targets: Vec<TargetDto>,
    pub scenario_ids: Vec<String>,
    pub tags: Vec<String>,
}
```

to (add `description_router` at the end):

```rust
#[derive(Debug, Serialize)]
pub struct ManagedSkillDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub source_ref_resolved: Option<String>,
    pub source_subpath: Option<String>,
    pub source_branch: Option<String>,
    pub source_revision: Option<String>,
    pub remote_revision: Option<String>,
    pub update_status: String,
    pub last_checked_at: Option<i64>,
    pub last_check_error: Option<String>,
    pub central_path: String,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub status: String,
    pub targets: Vec<TargetDto>,
    pub scenario_ids: Vec<String>,
    pub tags: Vec<String>,
    pub description_router: Option<String>,
}
```

- [ ] **Step 2: Update every ManagedSkillDto constructor**

Run: `grep -n 'ManagedSkillDto {' src-tauri/src/commands/skills.rs`

For each match, add `description_router: skill.description_router,` (or `.clone()` where skill is borrowed) to the struct literal. Most call sites will have a `SkillRecord` called `skill` or `s` in scope — use that.

Example change in one location (around line 1091 per earlier grep):
Before:
```rust
ManagedSkillDto {
    ...
    description: skill.description,
    ...
    tags,
}
```
After:
```rust
ManagedSkillDto {
    ...
    description: skill.description,
    ...
    tags,
    description_router: skill.description_router,
}
```

For sites that don't have a `SkillRecord` in scope (if any), set `description_router: None`.

- [ ] **Step 3: Add `set_skill_description_router` command**

Append to `src-tauri/src/commands/skills.rs` (anywhere after the struct defs, before the `#[cfg(test)]` block):

```rust
#[tauri::command]
pub async fn set_skill_description_router(
    skill_id: String,
    text: Option<String>,
    store: State<'_, Arc<SkillStore>>,
) -> Result<(), AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        store
            .set_skill_description_router(&skill_id, text.as_deref())
            .map_err(AppError::db)
    })
    .await?
}
```

If `State`, `Arc`, `AppError` aren't already imported at the top of the file, add the imports. Pattern: check `use` statements at top.

- [ ] **Step 4: Register command in lib.rs**

In `src-tauri/src/lib.rs` invoke_handler, find the skills commands section (search for `commands::skills::`). Add:

```rust
            commands::skills::set_skill_description_router,
```

- [ ] **Step 5: Build**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

Expected: clean. Missed constructor will fail compile — fix until green.

- [ ] **Step 6: Run workspace tests**

Run: `cargo test --workspace --no-fail-fast`

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands/skills.rs src-tauri/src/lib.rs
git commit -m "feat(tauri): ManagedSkillDto.description_router + set_skill_description_router IPC"
```

---

## Task 3: TypeScript types + IPC wrappers

**Files:**
- Modify: `src/lib/tauri.ts`

- [ ] **Step 1: Extend `PackRecord` TS interface**

Find `export interface PackRecord` (around line 531). Add `router_when_to_use` after `router_body`:

```typescript
export interface PackRecord {
  id: string;
  name: string;
  description: string | null;
  icon: string | null;
  color: string | null;
  sort_order: number;
  created_at: number;
  updated_at: number;
  router_description: string | null;
  router_body: string | null;
  router_when_to_use: string | null;
  is_essential: boolean;
  router_updated_at: number | null;
}
```

- [ ] **Step 2: Extend `ManagedSkill` TS interface**

Find `export interface ManagedSkill` (around line 23). Add `description_router` at the end:

```typescript
export interface ManagedSkill {
  id: string;
  name: string;
  description: string | null;
  // ... (existing fields)
  scenario_ids: string[];
  tags: string[];
  description_router: string | null;
}
```

- [ ] **Step 3: Add IPC wrapper functions**

Append to `src/lib/tauri.ts` (near other pack/skill wrapper functions):

```typescript
export const setPackWhenToUse = (packId: string, text: string | null) =>
  invoke<void>("set_pack_when_to_use", { packId, text });

export const setSkillDescriptionRouter = (skillId: string, text: string | null) =>
  invoke<void>("set_skill_description_router", { skillId, text });
```

Note: Tauri's `invoke` auto-converts camelCase TS args to snake_case Rust args, so `packId` maps to `pack_id`.

- [ ] **Step 4: TypeScript type-check**

Run: `pnpm run typecheck` (or `pnpm tsc --noEmit` if that's the script name; check `package.json`).

Expected: no new type errors.

If there are compilation errors in other files referencing `ManagedSkill` or `PackRecord` missing the new fields — fix by adding the field in the referenced constructor, defaulting to `null`.

- [ ] **Step 5: Commit**

```bash
git add src/lib/tauri.ts
git commit -m "feat(ts): Pack.router_when_to_use + ManagedSkill.description_router + IPC wrappers"
```

---

## Task 4: Extend `RouterEditor` with `whenToUse` textarea

**Files:**
- Modify: `src/components/RouterEditor.tsx`
- Create: `src/components/__tests__/RouterEditor.test.tsx`

- [ ] **Step 1: Write failing tests**

Create `src/components/__tests__/RouterEditor.test.tsx`:

```typescript
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { RouterEditor } from "../RouterEditor";

describe("RouterEditor", () => {
  it("renders when_to_use textarea", () => {
    render(
      <RouterEditor
        packId="p1"
        initial={{ description: "d", body: null, whenToUse: "use when X" }}
        onSave={vi.fn()}
      />
    );
    expect(screen.getByLabelText(/when.to.use/i)).toHaveValue("use when X");
  });

  it("calls onSave with description + body + whenToUse", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <RouterEditor
        packId="p1"
        initial={{ description: "d", body: null, whenToUse: null }}
        onSave={onSave}
      />
    );
    fireEvent.change(screen.getByLabelText(/when.to.use/i), {
      target: { value: "trigger text" },
    });
    fireEvent.click(screen.getByRole("button", { name: /save/i }));
    await new Promise((r) => setTimeout(r, 0));
    expect(onSave).toHaveBeenCalledWith({
      description: "d",
      body: null,
      whenToUse: "trigger text",
    });
  });

  it("saves null whenToUse when textarea is empty", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <RouterEditor
        packId="p1"
        initial={{ description: "d", body: null, whenToUse: "existing" }}
        onSave={onSave}
      />
    );
    fireEvent.change(screen.getByLabelText(/when.to.use/i), {
      target: { value: "" },
    });
    fireEvent.click(screen.getByRole("button", { name: /save/i }));
    await new Promise((r) => setTimeout(r, 0));
    expect(onSave).toHaveBeenCalledWith({
      description: "d",
      body: null,
      whenToUse: null,
    });
  });

  it("shows combined char count for description + whenToUse", () => {
    render(
      <RouterEditor
        packId="p1"
        initial={{ description: "a".repeat(100), body: null, whenToUse: "b".repeat(50) }}
        onSave={vi.fn()}
      />
    );
    expect(screen.getByTestId("char-counter")).toHaveTextContent("150");
  });
});
```

- [ ] **Step 2: Run tests — expect failure**

Run: `pnpm vitest run src/components/__tests__/RouterEditor.test.tsx`

Expected: FAIL — no `when_to_use` input, no new save shape.

- [ ] **Step 3: Replace `src/components/RouterEditor.tsx` content**

```typescript
import { useState } from "react";

type Initial = {
  description: string;
  body?: string | null;
  whenToUse?: string | null;
};

type Props = {
  packId: string;
  initial: Initial;
  onSave: (v: {
    description: string;
    body: string | null;
    whenToUse: string | null;
  }) => Promise<void>;
  onGenerate?: () => void;
  onPreview?: () => void;
};

export function RouterEditor({
  initial,
  onSave,
  onGenerate,
  onPreview,
}: Props) {
  const [desc, setDesc] = useState(initial.description);
  const [body, setBody] = useState(initial.body ?? "");
  const [whenToUse, setWhenToUse] = useState(initial.whenToUse ?? "");

  const combinedLen = desc.length + whenToUse.length;
  const color =
    combinedLen <= 1400
      ? "text-green-600"
      : combinedLen <= 1536
      ? "text-yellow-600"
      : "text-red-600";
  const canSave = desc.trim().length > 0 && combinedLen <= 1536;

  return (
    <div className="space-y-3">
      <label className="block">
        <span className="text-sm font-medium">Router description</span>
        <textarea
          className="w-full border rounded p-2 font-mono text-sm"
          rows={3}
          value={desc}
          onChange={(e) => setDesc(e.target.value)}
          aria-label="Router description"
        />
      </label>

      <label className="block">
        <span className="text-sm font-medium">When to use (trigger phrases)</span>
        <textarea
          className="w-full border rounded p-2 font-mono text-sm"
          rows={2}
          value={whenToUse}
          onChange={(e) => setWhenToUse(e.target.value)}
          aria-label="When to use"
          placeholder="Use when user says '...', '...'"
        />
      </label>

      <div data-testid="char-counter" className={`text-xs ${color}`}>
        {combinedLen} / 1536 chars (description + when_to_use)
      </div>

      <label className="block">
        <span className="text-sm font-medium">
          Body (optional — leave empty for auto-render)
        </span>
        <textarea
          className="w-full border rounded p-2 font-mono text-sm"
          rows={8}
          value={body}
          onChange={(e) => setBody(e.target.value)}
          aria-label="Router body"
        />
      </label>

      <div className="flex gap-2">
        <button
          type="button"
          className="px-3 py-1 bg-blue-600 text-white rounded disabled:opacity-50"
          disabled={!canSave}
          onClick={() =>
            onSave({
              description: desc.trim(),
              body: body.trim() || null,
              whenToUse: whenToUse.trim() || null,
            })
          }
        >
          Save
        </button>
        {onGenerate && (
          <button
            type="button"
            className="px-3 py-1 border rounded"
            onClick={onGenerate}
          >
            Generate with Claude Code
          </button>
        )}
        {onPreview && (
          <button
            type="button"
            className="px-3 py-1 border rounded"
            onClick={onPreview}
          >
            Preview Sync Output
          </button>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Run tests — expect PASS**

Run: `pnpm vitest run src/components/__tests__/RouterEditor.test.tsx`

Expected: all 4 tests pass.

- [ ] **Step 5: TypeScript type-check**

Run: `pnpm run typecheck`

Expected: errors in `PacksView.tsx` where the old `onSave` signature was `{ description, body }` but now it's `{ description, body, whenToUse }`. Fix in Task 5.

- [ ] **Step 6: Commit**

```bash
git add src/components/RouterEditor.tsx src/components/__tests__/RouterEditor.test.tsx
git commit -m "feat(ui): RouterEditor supports when_to_use"
```

---

## Task 5: Update `PacksView` — wire new field + save path + coverage badges

**Files:**
- Modify: `src/views/PacksView.tsx`

- [ ] **Step 1: Find the existing `RouterEditor` call site**

Run: `grep -n 'RouterEditor' src/views/PacksView.tsx`

Expected: call around line 550. Read the surrounding 20 lines to see how `onSave` is currently implemented and how pack data flows.

- [ ] **Step 2: Update the `RouterEditor` call to pass `whenToUse`**

Find the `<RouterEditor` call (around line 550). Change the `initial` prop to include `whenToUse` and the `onSave` to call both IPCs:

Before (approximately):
```tsx
<RouterEditor
  packId={pack.id}
  initial={{
    description: pack.router_description ?? "",
    body: pack.router_body ?? null,
  }}
  onSave={async ({ description, body }) => {
    await invoke("set_pack_router", {
      packId: pack.id,
      description,
      body,
    });
    // existing: reload / toast
  }}
/>
```

After:
```tsx
<RouterEditor
  packId={pack.id}
  initial={{
    description: pack.router_description ?? "",
    body: pack.router_body ?? null,
    whenToUse: pack.router_when_to_use ?? null,
  }}
  onSave={async ({ description, body, whenToUse }) => {
    await invoke("set_pack_router", {
      packId: pack.id,
      description,
      body,
    });
    await invoke("set_pack_when_to_use", {
      packId: pack.id,
      text: whenToUse,
    });
    // existing: reload / toast (unchanged)
  }}
/>
```

Keep whatever reload/toast logic exists after the invoke calls.

- [ ] **Step 3: Add L1/L2 coverage badges to pack cards**

Find the pack card rendering (look for the outer `<div>` that renders each pack — search for `pack.name` or `pack.icon`). Near where the pack's name is displayed, add a row of pill badges:

```tsx
{/* inside the pack card JSX, below the pack name */}
<div className="flex gap-1 text-xs mt-1">
  <span
    className={`px-1.5 py-0.5 rounded ${
      pack.router_description && pack.router_when_to_use
        ? "bg-green-100 text-green-700"
        : pack.router_description || pack.router_when_to_use
        ? "bg-yellow-100 text-yellow-700"
        : "bg-gray-100 text-gray-500"
    }`}
  >
    L1 {pack.router_description && pack.router_when_to_use
      ? "✓"
      : pack.router_description || pack.router_when_to_use
      ? "partial"
      : "—"}
  </span>
  <span className="px-1.5 py-0.5 rounded bg-blue-50 text-blue-700">
    L2 {l2Coverage(pack)}
  </span>
</div>
```

Helper `l2Coverage(pack)` — add near top of `PacksView.tsx` or inside the component:

```tsx
function l2Coverage(pack: PackRecord & { skills?: Array<{ description_router: string | null }> }): string {
  const skills = pack.skills ?? [];
  if (skills.length === 0) return "0/0";
  const set = skills.filter((s) => !!s.description_router).length;
  return `${set}/${skills.length}`;
}
```

If the pack data object passed to the card doesn't carry skills, derive coverage from the main skill list instead:

```tsx
// Alternative (if pack doesn't have embedded skills): compute in parent, pass as prop
const coverage = useMemo(() => {
  // For each pack, count skills whose description_router is non-null.
  // Requires access to the full skill list — typically available in PacksView scope.
  const byPack = new Map<string, { total: number; set: number }>();
  for (const pack of allPacks) {
    byPack.set(pack.id, { total: 0, set: 0 });
  }
  for (const skill of allSkills) {
    for (const pid of skill.pack_ids ?? []) {
      const c = byPack.get(pid);
      if (c) {
        c.total += 1;
        if (skill.description_router) c.set += 1;
      }
    }
  }
  return byPack;
}, [allPacks, allSkills]);
```

**NOTE**: `skill.pack_ids` may or may not exist on the TS type. If not, inspect `PackSkillRecord` and the existing code to see how pack→skill membership is tracked in the PacksView state. Reuse the existing structure rather than adding a new IPC.

Practical compromise: if unclear where to get L2 coverage cheaply, SHIP L1 badge only in this task, and add L2 badge in a small follow-up once the data plumbing is obvious.

**Decision for this plan**: Ship L1 badge only. Mark L2 coverage badge as a follow-up sub-task at end of this task if the data path isn't obvious from existing PacksView state. Don't block the PR on it.

- [ ] **Step 4: TypeScript type-check**

Run: `pnpm run typecheck`

Expected: clean.

- [ ] **Step 5: Visual smoke test**

Run: `pnpm tauri:dev`

Wait for GUI to launch. Navigate to PacksView. Click a pack. Verify:
- Router editor now shows "When to use" textarea with pack's existing value (if any)
- Editing + Save updates DB (verify via `sqlite3 ~/.skills-manager/skills-manager.db "SELECT name, router_when_to_use FROM packs WHERE name='<edited>';"`)
- Pack cards show L1 pill (green if both fields set, yellow if one, gray if neither)

Kill the dev server (Ctrl+C) when done.

- [ ] **Step 6: Commit**

```bash
git add src/views/PacksView.tsx
git commit -m "feat(ui): PacksView saves when_to_use + shows L1 coverage badge"
```

---

## Task 6: Extend `SkillDetailPanel` — L2 editor + sibling list

**Files:**
- Modify: `src/components/SkillDetailPanel.tsx`
- Create: `src/components/__tests__/SkillDetailPanel.test.tsx`

- [ ] **Step 1: Update Props + write failing tests**

Read the existing `Props` interface in `SkillDetailPanel.tsx` (around line 28–34). Extend with:

```typescript
interface Props {
  skill: ManagedSkill | null;
  onClose: () => void;
  toolToggles?: SkillToolToggle[] | null;
  togglingTool?: string | null;
  onToggleTool?: (tool: string, enabled: boolean) => void;
  sisterSkills?: Array<{ id: string; name: string; description_router: string | null }>;
  onSaveDescriptionRouter?: (skillId: string, text: string | null) => Promise<void>;
  onSelectSibling?: (skillId: string) => void;
}
```

Create `src/components/__tests__/SkillDetailPanel.test.tsx`:

```typescript
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SkillDetailPanel } from "../SkillDetailPanel";
import type { ManagedSkill } from "../../lib/tauri";

const mockSkill: ManagedSkill = {
  id: "s1",
  name: "test-skill",
  description: "Test",
  source_type: "local",
  source_ref: null,
  source_ref_resolved: null,
  source_subpath: null,
  source_branch: null,
  source_revision: null,
  remote_revision: null,
  update_status: "idle",
  last_checked_at: null,
  last_check_error: null,
  central_path: "/tmp/test-skill",
  enabled: true,
  created_at: 0,
  updated_at: 0,
  status: "active",
  targets: [],
  scenario_ids: [],
  tags: [],
  description_router: "Short L2 line",
};

describe("SkillDetailPanel", () => {
  it("renders description_router textarea with current value", () => {
    render(<SkillDetailPanel skill={mockSkill} onClose={vi.fn()} />);
    const textarea = screen.getByLabelText(/router description/i);
    expect(textarea).toHaveValue("Short L2 line");
  });

  it("calls onSaveDescriptionRouter with new text", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <SkillDetailPanel
        skill={mockSkill}
        onClose={vi.fn()}
        onSaveDescriptionRouter={onSave}
      />
    );
    const textarea = screen.getByLabelText(/router description/i);
    fireEvent.change(textarea, { target: { value: "New L2 line" } });
    fireEvent.click(screen.getByRole("button", { name: /save router description/i }));
    await new Promise((r) => setTimeout(r, 0));
    expect(onSave).toHaveBeenCalledWith("s1", "New L2 line");
  });

  it("renders sibling list when sisterSkills is provided", () => {
    render(
      <SkillDetailPanel
        skill={mockSkill}
        onClose={vi.fn()}
        sisterSkills={[
          { id: "s2", name: "sibling-a", description_router: "A's L2" },
          { id: "s3", name: "sibling-b", description_router: null },
        ]}
      />
    );
    expect(screen.getByText(/sibling-a/i)).toBeInTheDocument();
    expect(screen.getByText(/A's L2/)).toBeInTheDocument();
    expect(screen.getByText(/sibling-b/i)).toBeInTheDocument();
    expect(screen.getByText(/no L2 authored/i)).toBeInTheDocument();
  });

  it("clicking sibling calls onSelectSibling", () => {
    const onSelectSibling = vi.fn();
    render(
      <SkillDetailPanel
        skill={mockSkill}
        onClose={vi.fn()}
        sisterSkills={[
          { id: "s2", name: "sibling-a", description_router: "A's L2" },
        ]}
        onSelectSibling={onSelectSibling}
      />
    );
    fireEvent.click(screen.getByText(/sibling-a/i));
    expect(onSelectSibling).toHaveBeenCalledWith("s2");
  });
});
```

- [ ] **Step 2: Run tests — expect FAIL**

Run: `pnpm vitest run src/components/__tests__/SkillDetailPanel.test.tsx`

Expected: FAIL — no L2 section, no sibling list.

- [ ] **Step 3: Add L2 editor + sibling list to `SkillDetailPanel.tsx`**

Inside the `SkillDetailPanel` function body, after the existing state hooks + before the returned JSX's final closing, add local state:

```typescript
const [routerDescDraft, setRouterDescDraft] = useState(
  skill?.description_router ?? ""
);
const [savingRouterDesc, setSavingRouterDesc] = useState(false);

// Reset draft when skill changes
useEffect(() => {
  setRouterDescDraft(skill?.description_router ?? "");
}, [skill?.id, skill?.description_router]);
```

In the rendered JSX (somewhere visible in the modal — inside the scrollable content area, BEFORE the document content tabs), insert:

```tsx
{skill && onSaveDescriptionRouter && (
  <section className="border rounded p-3 mb-4 bg-gray-50">
    <div className="flex items-center justify-between mb-2">
      <h3 className="text-sm font-semibold">Router description (L2)</h3>
      <span className="text-xs text-gray-500">
        {routerDescDraft.length} chars
      </span>
    </div>
    <textarea
      className="w-full border rounded p-2 font-mono text-sm"
      rows={2}
      value={routerDescDraft}
      onChange={(e) => setRouterDescDraft(e.target.value)}
      aria-label="Router description (L2)"
      placeholder="Short per-skill line shown in pack router body. e.g. 'Perplexity single answer. → Pick for ...'"
    />
    <div className="flex gap-2 mt-2">
      <button
        type="button"
        className="px-3 py-1 bg-blue-600 text-white rounded text-sm disabled:opacity-50"
        disabled={savingRouterDesc}
        onClick={async () => {
          if (!skill) return;
          setSavingRouterDesc(true);
          try {
            await onSaveDescriptionRouter(
              skill.id,
              routerDescDraft.trim() || null
            );
          } finally {
            setSavingRouterDesc(false);
          }
        }}
      >
        Save router description
      </button>
      <button
        type="button"
        className="px-3 py-1 border rounded text-sm"
        onClick={() => setRouterDescDraft("")}
      >
        Clear
      </button>
    </div>
  </section>
)}

{sisterSkills && sisterSkills.length > 0 && (
  <section className="border rounded p-3 mb-4">
    <h3 className="text-sm font-semibold mb-2">
      Sibling skills in this pack
    </h3>
    <ul className="space-y-1 text-sm">
      {sisterSkills.map((sib) => (
        <li key={sib.id}>
          <button
            type="button"
            className="text-left hover:underline"
            onClick={() => onSelectSibling?.(sib.id)}
          >
            <span className="font-mono">{sib.name}</span>
            {": "}
            {sib.description_router ? (
              <span className="text-gray-700">{sib.description_router}</span>
            ) : (
              <span className="text-gray-400 italic">(no L2 authored)</span>
            )}
          </button>
        </li>
      ))}
    </ul>
  </section>
)}
```

- [ ] **Step 4: Run tests — expect PASS**

Run: `pnpm vitest run src/components/__tests__/SkillDetailPanel.test.tsx`

Expected: all 4 tests pass.

- [ ] **Step 5: TypeScript type-check**

Run: `pnpm run typecheck`

Expected: errors in `MySkills.tsx` where `SkillDetailPanel` is rendered without the new optional props. The props are optional so compile should still pass; if an error surfaces, the fix is in Task 7.

- [ ] **Step 6: Commit**

```bash
git add src/components/SkillDetailPanel.tsx src/components/__tests__/SkillDetailPanel.test.tsx
git commit -m "feat(ui): SkillDetailPanel — L2 editor + sibling list"
```

---

## Task 7: Update `MySkills` — wire L2 save + compute siblings + L2 filter

**Files:**
- Modify: `src/views/MySkills.tsx`

- [ ] **Step 1: Find the `SkillDetailPanel` render location**

Run: `grep -n 'SkillDetailPanel\|selectedSkill' src/views/MySkills.tsx | head -10`

Look at the surrounding 20 lines for both the `SkillDetailPanel` render (around line 1477) and the skill-list rendering to understand state shape.

- [ ] **Step 2: Wire `onSaveDescriptionRouter` + compute `sisterSkills`**

Find the `<SkillDetailPanel` JSX. Extend props:

```tsx
<SkillDetailPanel
  skill={selectedSkill}
  onClose={() => setSelectedSkill(null)}
  toolToggles={toolToggles}
  togglingTool={togglingTool}
  onToggleTool={handleToggleTool}
  sisterSkills={computeSiblings(selectedSkill, allSkills, packs)}
  onSaveDescriptionRouter={async (skillId, text) => {
    await setSkillDescriptionRouter(skillId, text);
    // Reload the skill after save; exact code depends on existing state shape.
    await reloadSkills();
  }}
  onSelectSibling={(skillId) => {
    const next = allSkills.find((s) => s.id === skillId);
    if (next) setSelectedSkill(next);
  }}
/>
```

Add the `computeSiblings` helper function near top of the file:

```tsx
function computeSiblings(
  current: ManagedSkill | null,
  allSkills: ManagedSkill[],
  packs: PackRecord[] | PackWithSkills[],
): Array<{ id: string; name: string; description_router: string | null }> {
  if (!current) return [];
  // Find packs that include this skill, then all skills in those packs.
  // Exact lookup depends on data shape — inspect existing packs state:
  // if `packs` is PackRecord[], we need a separate pack_skills relation;
  // if packs include skill lists, iterate directly.
  // Fall back: return empty list if relation isn't cheaply available.
  const siblingIds = new Set<string>();
  for (const pack of packs) {
    const skillsInPack = (pack as any).skills ?? (pack as any).skill_ids ?? [];
    const isMember = skillsInPack.some(
      (s: any) => (typeof s === "string" ? s : s.id) === current.id
    );
    if (!isMember) continue;
    for (const s of skillsInPack) {
      const id = typeof s === "string" ? s : s.id;
      if (id !== current.id) siblingIds.add(id);
    }
  }
  return allSkills
    .filter((s) => siblingIds.has(s.id))
    .map((s) => ({
      id: s.id,
      name: s.name,
      description_router: s.description_router,
    }));
}
```

**Adaptation note**: before committing, verify the exact shape of `packs` in MySkills state. Use whatever relation structure is already loaded. If MySkills doesn't load pack→skill relations today, add `getPacksForSkill(skill_id)` Tauri command or just pass an empty `sisterSkills` and fix later (graceful fallback — `SkillDetailPanel` omits the sibling section when list is empty).

Run `grep -n 'packs\|PackRecord' src/views/MySkills.tsx | head -15` to confirm what state MySkills holds.

- [ ] **Step 3: Add L2 indicator + "show only unset" filter**

Find the skills-list rendering. Beside (or within) each skill's row, add:

```tsx
<span
  className={`px-1.5 py-0.5 rounded text-xs ml-2 ${
    skill.description_router
      ? "bg-green-100 text-green-700"
      : "bg-gray-100 text-gray-500"
  }`}
  title={skill.description_router ?? "No L2 authored"}
>
  L2 {skill.description_router ? "✓" : "—"}
</span>
```

Add a filter toggle above the list:

```tsx
const [onlyUnsetL2, setOnlyUnsetL2] = useState(false);

// in render:
<label className="flex items-center gap-2 text-sm">
  <input
    type="checkbox"
    checked={onlyUnsetL2}
    onChange={(e) => setOnlyUnsetL2(e.target.checked)}
  />
  Show only skills without L2
</label>
```

Apply filter in the list rendering. Find where the skill list is filtered (likely there's already a search/filter pipeline) and add:

```tsx
const visibleSkills = skills.filter((s) => {
  if (onlyUnsetL2 && s.description_router) return false;
  // ...existing filters
  return true;
});
```

Plug into the existing filter chain rather than duplicating. If the exact chain is unclear, add a fresh filter step after the existing ones.

- [ ] **Step 4: TypeScript type-check**

Run: `pnpm run typecheck`

Expected: clean.

- [ ] **Step 5: Visual smoke test**

Run: `pnpm tauri:dev`. Navigate to MySkills. Verify:
- Each skill row shows L2 ✓ or L2 — pill
- "Show only skills without L2" checkbox filters correctly
- Click a skill → detail panel opens → L2 editor section visible
- Edit + save → verify DB: `sqlite3 ~/.skills-manager/skills-manager.db "SELECT name, description_router FROM skills WHERE name='<edited>';"`
- Sibling list renders below L2 editor (if the skill belongs to a pack)

- [ ] **Step 6: Commit**

```bash
git add src/views/MySkills.tsx
git commit -m "feat(ui): MySkills — L2 badge, filter, save + sibling panel"
```

---

## Task 8: Manual e2e + PROGRESS

**Files:**
- Modify: `PROGRESS.md`

- [ ] **Step 1: Full workspace tests**

Run: `cargo test --workspace --no-fail-fast && pnpm vitest run --reporter verbose`

Expected: all Rust + TS tests pass.

- [ ] **Step 2: Tauri build**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

Expected: clean build.

- [ ] **Step 3: Full manual e2e**

Run: `pnpm tauri:dev`

Walk through the following user flow:

1. Navigate to PacksView. Pick a pack (e.g. `marketing`). Expand Router Editor.
   - ✅ See existing `router_description` populated
   - ✅ See new "When to use" textarea populated (or empty if not set)
   - Edit `when_to_use` → Save
   - Verify DB with SQL: `sqlite3 ~/.skills-manager/skills-manager.db "SELECT router_when_to_use FROM packs WHERE name='marketing';"`
2. PacksView card grid:
   - ✅ Green L1 ✓ pill on packs with both `router_description` + `router_when_to_use` set
   - ✅ Yellow L1 partial on packs with one set
   - ✅ Gray L1 — on packs with neither
3. Navigate to MySkills. Tick "Show only skills without L2".
   - ✅ List filters to only unset skills
4. Click a skill (one that's in a pack). Detail panel opens.
   - ✅ "Router description (L2)" section visible
   - ✅ Textarea prefilled with existing L2 (or empty)
   - ✅ Sibling list shows other skills in the same pack with their L2 previews
   - Edit L2 → Save. Verify DB.
   - Click a sibling → detail panel switches to that skill.
5. Sync + render test (CLI):
   ```bash
   sm switch claude_code standard-marketing
   cat ~/.claude/skills/pack-marketing/SKILL.md
   ```
   - ✅ Frontmatter `when_to_use:` reflects edit from step 1
   - ✅ Table row for edited skill reflects new L2 from step 4

- [ ] **Step 4: Update PROGRESS.md**

Edit `PROGRESS.md`. Add entry near the top of "Current Iteration":

```markdown
### Three-Tier UI ✅
**Status:** Complete (PR pending) **Date:** 2026-04-21
**Goal:** Close GUI gap for three-tier PD — edit `router_when_to_use` + `description_router` from Tauri app.
**Changes:**
- 2 additive Tauri IPC commands (`set_pack_when_to_use`, `set_skill_description_router`)
- `ManagedSkillDto` + TS `PackRecord`/`ManagedSkill` types gain new fields
- `RouterEditor` supports `when_to_use` textarea + combined 1536-char counter
- `PacksView` — L1 coverage pill per pack card; save path includes when_to_use
- `SkillDetailPanel` — L2 editor section + sibling skill list with click-to-navigate
- `MySkills` — L2 ✓/— indicator per row, "show only unset" filter, sibling computation
- New Vitest tests: RouterEditor (4 cases) + SkillDetailPanel (4 cases)
- No DB/schema changes
**Verified end-to-end:** Full GUI edit flow works for L1 + L2. Sibling list aids "分叉" authoring. CLI output reflects all GUI edits.
```

Commit:

```bash
git add PROGRESS.md
git commit -m "docs: three-tier UI complete"
```

---

## Self-Review

**Spec coverage:**

| Spec goal | Task |
|---|---|
| Pack L1 `when_to_use` editable in GUI | Tasks 1 + 3 + 4 + 5 |
| Skill L2 `description_router` editable | Tasks 2 + 3 + 6 + 7 |
| Sibling-skill helper (Option C) | Tasks 6 + 7 |
| Coverage badges | Task 5 (L1) + Task 7 (L2 indicator + filter) |
| No new IPC for sibling list | Task 7 (computed client-side from existing state) |
| Additive IPC (no breaking changes) | Tasks 1 + 2 |
| No DB/schema change | Confirmed — no migration tasks |
| Bulk YAML import skipped | Confirmed — no task |
| Tests: unit for components + manual e2e | Tasks 4, 6 (Vitest); Task 8 (e2e) |

All goals mapped.

**Placeholder scan**: no "TBD"/"TODO"/"implement later". Task 5 Step 3 has a decision point ("Ship L1 badge only in this task") — this is a scope tradeoff documented in place, not a placeholder. Task 7 Step 2 has an adaptation note ("verify exact shape of packs before committing") — this is a context-check, not a placeholder.

**Type consistency**:
- `set_pack_when_to_use(packId, text)` IPC (Task 1) ↔ `setPackWhenToUse` wrapper (Task 3) ↔ RouterEditor save path (Task 5). Consistent.
- `set_skill_description_router(skillId, text)` IPC (Task 2) ↔ `setSkillDescriptionRouter` wrapper (Task 3) ↔ SkillDetailPanel save (Tasks 6 + 7). Consistent.
- `ManagedSkill.description_router` (Task 3) ↔ mockSkill in Task 6 test ↔ sibling list computation (Task 7). Consistent.
- `RouterEditor` Props shape (`{ description, body, whenToUse }`) (Task 4) ↔ PacksView call (Task 5). Consistent.
- `SkillDetailPanel` new props (`sisterSkills`, `onSaveDescriptionRouter`, `onSelectSibling`) (Task 6) ↔ MySkills usage (Task 7). Consistent.

**Decomposition**: 8 tasks. Tasks 1–3 are backend + types (foundation). Task 4 is component A (RouterEditor) with tests. Task 5 wires A into PacksView + badges. Task 6 is component C (SkillDetailPanel L2 + siblings) with tests. Task 7 wires C into MySkills + filter. Task 8 is acceptance + docs. Each task produces a working state.
