// Unit tests for RouterEditor.
//
// NOTE: This repo does not currently ship a JS test runner (vitest/jest) or
// @testing-library/react in package.json. These tests are written to the
// Vitest + @testing-library/react API for a future test harness. The file
// is excluded from the production `tsc -b` build via the Vite project config.
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { RouterEditor } from "../RouterEditor";

describe("RouterEditor", () => {
  it("disables Save when description is empty", () => {
    render(
      <RouterEditor packId="p1" initial={{ description: "" }} onSave={vi.fn()} />,
    );
    expect(screen.getByRole("button", { name: /save/i })).toBeDisabled();
  });

  it("calls onSave with trimmed description + body + null whenToUse", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <RouterEditor packId="p1" initial={{ description: "" }} onSave={onSave} />,
    );
    fireEvent.change(screen.getByLabelText(/router description/i), {
      target: { value: "  hello  " },
    });
    fireEvent.click(screen.getByRole("button", { name: /save/i }));
    expect(onSave).toHaveBeenCalledWith({
      description: "hello",
      body: null,
      whenToUse: null,
    });
  });

  it("converts empty body + empty whenToUse to null", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <RouterEditor
        packId="p1"
        initial={{ description: "desc", body: "", whenToUse: "" }}
        onSave={onSave}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /save/i }));
    expect(onSave).toHaveBeenCalledWith({
      description: "desc",
      body: null,
      whenToUse: null,
    });
  });

  it("renders Generate button when onGenerate provided", () => {
    render(
      <RouterEditor
        packId="p1"
        initial={{ description: "d" }}
        onSave={vi.fn()}
        onGenerate={vi.fn()}
      />,
    );
    expect(
      screen.getByRole("button", { name: /generate with claude code/i }),
    ).toBeInTheDocument();
  });

  // ── new tests for when_to_use ──

  it("renders when_to_use textarea with initial value", () => {
    render(
      <RouterEditor
        packId="p1"
        initial={{ description: "d", body: null, whenToUse: "use when X" }}
        onSave={vi.fn()}
      />,
    );
    expect(screen.getByLabelText(/when to use/i)).toHaveValue("use when X");
  });

  it("calls onSave with trimmed whenToUse when edited", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <RouterEditor
        packId="p1"
        initial={{ description: "d", body: null, whenToUse: null }}
        onSave={onSave}
      />,
    );
    fireEvent.change(screen.getByLabelText(/when to use/i), {
      target: { value: "  trigger text  " },
    });
    fireEvent.click(screen.getByRole("button", { name: /save/i }));
    expect(onSave).toHaveBeenCalledWith({
      description: "d",
      body: null,
      whenToUse: "trigger text",
    });
  });

  it("shows combined char count for description + whenToUse", () => {
    render(
      <RouterEditor
        packId="p1"
        initial={{
          description: "a".repeat(100),
          body: null,
          whenToUse: "b".repeat(50),
        }}
        onSave={vi.fn()}
      />,
    );
    expect(screen.getByTestId("char-counter")).toHaveTextContent("150");
  });

  it("warns yellow at 1400–1536 chars", () => {
    render(
      <RouterEditor
        packId="p1"
        initial={{ description: "a".repeat(1450), whenToUse: "" }}
        onSave={vi.fn()}
      />,
    );
    expect(screen.getByTestId("char-counter").className).toContain(
      "text-yellow-600",
    );
  });

  it("warns red above 1536 chars and disables Save", () => {
    render(
      <RouterEditor
        packId="p1"
        initial={{ description: "a".repeat(1600), whenToUse: "" }}
        onSave={vi.fn()}
      />,
    );
    expect(screen.getByTestId("char-counter").className).toContain(
      "text-red-600",
    );
    expect(screen.getByRole("button", { name: /save/i })).toBeDisabled();
  });
});
