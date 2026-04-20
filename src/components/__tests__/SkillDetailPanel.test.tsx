// Staged Vitest tests for SkillDetailPanel (L2 editor + sibling list).
// NOTE: vitest not installed; these are written for future test harness.
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
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
  it("renders description_router textarea with current value when onSaveDescriptionRouter provided", () => {
    render(
      <SkillDetailPanel
        skill={mockSkill}
        onClose={vi.fn()}
        onSaveDescriptionRouter={vi.fn()}
      />
    );
    const textarea = screen.getByLabelText(/router description \(l2\)/i);
    expect(textarea).toHaveValue("Short L2 line");
  });

  it("does not render L2 editor when onSaveDescriptionRouter is omitted", () => {
    render(<SkillDetailPanel skill={mockSkill} onClose={vi.fn()} />);
    expect(screen.queryByLabelText(/router description \(l2\)/i)).toBeNull();
  });

  it("calls onSaveDescriptionRouter with new trimmed text", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <SkillDetailPanel
        skill={mockSkill}
        onClose={vi.fn()}
        onSaveDescriptionRouter={onSave}
      />
    );
    const textarea = screen.getByLabelText(/router description \(l2\)/i);
    fireEvent.change(textarea, { target: { value: "  New L2 line  " } });
    fireEvent.click(screen.getByRole("button", { name: /save router description/i }));
    await new Promise((r) => setTimeout(r, 0));
    expect(onSave).toHaveBeenCalledWith("s1", "New L2 line");
  });

  it("calls onSaveDescriptionRouter with null when textarea cleared", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <SkillDetailPanel
        skill={mockSkill}
        onClose={vi.fn()}
        onSaveDescriptionRouter={onSave}
      />
    );
    const textarea = screen.getByLabelText(/router description \(l2\)/i);
    fireEvent.change(textarea, { target: { value: "   " } });
    fireEvent.click(screen.getByRole("button", { name: /save router description/i }));
    await new Promise((r) => setTimeout(r, 0));
    expect(onSave).toHaveBeenCalledWith("s1", null);
  });

  it("renders sibling list when sisterSkills provided", () => {
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
    expect(screen.getByText(/sibling-a/)).toBeInTheDocument();
    expect(screen.getByText(/A's L2/)).toBeInTheDocument();
    expect(screen.getByText(/sibling-b/)).toBeInTheDocument();
    expect(screen.getByText(/no L2 authored/i)).toBeInTheDocument();
  });

  it("calls onSelectSibling when a sibling is clicked", () => {
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
    fireEvent.click(screen.getByText(/sibling-a/));
    expect(onSelectSibling).toHaveBeenCalledWith("s2");
  });
});
