import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.3 RerunPrompt — the §2.5 re-run decision modal (state 6). Purely presentational, so
// the three §5.3 callbacks are `vi.fn`s; this suite pins the DECIDED §5.6(f) copy, the three controls, the
// default-focus-on-Skip landing, the alertdialog accessible name (§5.6 WCAG 4.1.2), the Esc-cancel, and the
// §5.6 focus-trap. The command wiring (Skip/FreshCopy → C6, Cancel → rerunCancel) is RerunScreen.test.tsx's.
// [Build-Session-Entscheidung: P3.57]
import { RerunPrompt } from "./RerunPrompt";

afterEach(cleanup);

function renderPrompt() {
  const onSkip = vi.fn();
  const onFreshCopy = vi.fn();
  const onCancel = vi.fn();
  const utils = render(
    <RerunPrompt onSkip={onSkip} onFreshCopy={onFreshCopy} onCancel={onCancel} />,
  );
  return { ...utils, onSkip, onFreshCopy, onCancel };
}

describe("RerunPrompt — §5.3 / §2.5 re-run decision modal (state 6)", () => {
  it("renders the DECIDED §5.6(f) heading + body and the three §5.2 row-6 controls", () => {
    const { getByText, getByRole } = renderPrompt();
    expect(getByText("Already converted with these settings")).not.toBeNull();
    expect(getByText("You already converted these with the same settings.")).not.toBeNull();
    expect(getByRole("button", { name: "Skip" })).not.toBeNull();
    expect(getByRole("button", { name: "Make a fresh copy" })).not.toBeNull();
    expect(getByRole("button", { name: "Cancel" })).not.toBeNull();
  });

  it("is a role=alertdialog named by its heading (§5.6 WCAG 4.1.2 accessible name)", () => {
    const { getByRole } = renderPrompt();
    // testing-library resolves the accessible name via aria-labelledby → the heading text.
    expect(
      getByRole("alertdialog", { name: "Already converted with these settings" }),
    ).not.toBeNull();
  });

  it("lands default focus on Skip (the safe default, §5.6/§5.10)", () => {
    const { getByRole } = renderPrompt();
    expect(document.activeElement).toBe(getByRole("button", { name: "Skip" }));
  });

  it("Skip fires onSkip (only)", () => {
    const { getByRole, onSkip, onFreshCopy, onCancel } = renderPrompt();
    fireEvent.click(getByRole("button", { name: "Skip" }));
    expect(onSkip).toHaveBeenCalledTimes(1);
    expect(onFreshCopy).not.toHaveBeenCalled();
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("Make a fresh copy fires onFreshCopy (only)", () => {
    const { getByRole, onSkip, onFreshCopy, onCancel } = renderPrompt();
    fireEvent.click(getByRole("button", { name: "Make a fresh copy" }));
    expect(onFreshCopy).toHaveBeenCalledTimes(1);
    expect(onSkip).not.toHaveBeenCalled();
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("Cancel fires onCancel (§5.2 row 6 → back to Destination)", () => {
    const { getByRole, onCancel } = renderPrompt();
    fireEvent.click(getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("Esc fires onCancel (§5.10 — the same back-to-Destination exit)", () => {
    const { getByRole, onCancel } = renderPrompt();
    fireEvent.keyDown(getByRole("alertdialog"), { key: "Escape" });
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("traps Tab within the dialog — Tab off the last control wraps to the first (§5.6)", () => {
    const { getByRole } = renderPrompt();
    const cancel = getByRole("button", { name: "Cancel" });
    const skip = getByRole("button", { name: "Skip" });
    cancel.focus();
    fireEvent.keyDown(cancel, { key: "Tab" });
    expect(document.activeElement).toBe(skip);
  });

  it("traps Shift+Tab within the dialog — Shift+Tab off the first control wraps to the last (§5.6)", () => {
    const { getByRole } = renderPrompt();
    const skip = getByRole("button", { name: "Skip" });
    const cancel = getByRole("button", { name: "Cancel" });
    skip.focus();
    fireEvent.keyDown(skip, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(cancel);
  });
});
