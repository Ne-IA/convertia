import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.2 Confirm-gate screen — the state-3 transitions (Confirm → C3+C4 via
// advanceToTargets; Cancel/Esc → cancel → Idle) + focus-on-entry. Mock the §5.1 events façade (the C3+C4
// advance) + the announcer (BatchSummary calls it); the §5.2 machine is the REAL store so the cancel →
// transition is exercised end-to-end (the P3 screen-box wiring model: a rendered action button MUST fire its
// command / dispatch a Msg). [Build-Session-Entscheidung: P3.55]
const advanceToTargets = vi.fn<(collectedSetId: string) => Promise<void>>();
vi.mock("../lib/ipc/events", () => ({
  advanceToTargets: (collectedSetId: string) => advanceToTargets(collectedSetId),
}));
vi.mock("../a11y/announcer", () => ({ announce: () => undefined }));

import { ConfirmScreen } from "./ConfirmScreen";
import { useAppStore } from "../state/store";
import type { SingleSet } from "../state/machine";

// A minimal §1.4 CollectedSet::Single. `items: []` sidesteps constructing a full DroppedItem (with a
// DetectionOutcome) — this suite pins the TRANSITIONS, not the row rendering (that is FileList.test.tsx). The
// display counts need not satisfy the §0.6 count==items.len invariant here (it is a backend invariant).
const set: SingleSet = {
  id: "cs1",
  instance: "inst-1",
  format: "csv",
  items: [],
  count: 3,
  skipped: [],
  totalBytes: 100,
  rootsDisplay: ["C:/drop"],
  encodingHint: null,
  delimiterHint: null,
  notes: [],
};

afterEach(cleanup);
beforeEach(() => {
  advanceToTargets.mockReset();
  advanceToTargets.mockResolvedValue(undefined);
  useAppStore.setState({ machine: { tag: "confirm", set } });
});

describe("ConfirmScreen — §5.2 Confirm gate (state 3)", () => {
  it("focuses the Continue button on entry (§5.6.1 state-3 landing)", () => {
    const { getByRole } = render(<ConfirmScreen set={set} />);
    expect(document.activeElement).toBe(getByRole("button", { name: "Continue" }));
  });

  it("renders the BatchSummary count + the FileList disclosure (composition)", () => {
    const { getByText, getByRole } = render(<ConfirmScreen set={set} />);
    expect(getByText("3 CSV files")).not.toBeNull();
    expect(getByRole("button", { name: "Show 0 files" })).not.toBeNull();
  });

  it("Continue fires C3+C4 via advanceToTargets(set.id) — the §5.8 3→4 advance", () => {
    const { getByRole } = render(<ConfirmScreen set={set} />);
    fireEvent.click(getByRole("button", { name: "Continue" }));
    expect(advanceToTargets).toHaveBeenCalledWith("cs1");
  });

  it("guards a double-advance — a rapid double Continue fires C3+C4 once", () => {
    const { getByRole } = render(<ConfirmScreen set={set} />);
    const button = getByRole("button", { name: "Continue" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(advanceToTargets).toHaveBeenCalledTimes(1);
  });

  it("Cancel dispatches `cancel` → Idle (§5.2 row 3), firing no command", () => {
    const { getByRole } = render(<ConfirmScreen set={set} />);
    fireEvent.click(getByRole("button", { name: "Cancel" }));
    expect(useAppStore.getState().machine.tag).toBe("idle");
    expect(advanceToTargets).not.toHaveBeenCalled();
  });

  it("Esc cancels the batch back to Idle (§5.10)", () => {
    render(<ConfirmScreen set={set} />);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(useAppStore.getState().machine.tag).toBe("idle");
  });

  it("unbinds the Esc listener on unmount (no cancel after leaving Confirm)", () => {
    const { unmount } = render(<ConfirmScreen set={set} />);
    unmount();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(useAppStore.getState().machine.tag).toBe("confirm");
  });
});
