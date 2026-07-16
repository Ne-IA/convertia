import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.2 Converting screen (state 7 + the 7a Cancelling sub-state) — the state-7
// out-transition (the P3 screen-box wiring model: a rendered action MUST fire its command). Mock the §5.1 events
// façade (the C7 `cancel_run` round-trip); the store's live progress is seated so the ProgressList renders.
// [Build-Session-Entscheidung: P3.58]
const cancelConversionRun = vi.fn<(...args: unknown[]) => Promise<void>>();
vi.mock("../lib/ipc/events", () => ({
  cancelConversionRun: (...args: unknown[]) => cancelConversionRun(...args),
}));

import { ConvertingScreen } from "./ConvertingScreen";
import { useAppStore } from "../state/store";
import type { ItemRow } from "../state/store";

const row: ItemRow = { sourceDisplay: "/a.csv", status: "running", fraction: 0.5, reason: null };

afterEach(() => {
  cleanup();
  useAppStore.setState({ progress: {}, batchProgress: null });
});
beforeEach(() => {
  cancelConversionRun.mockReset();
  cancelConversionRun.mockResolvedValue(undefined);
  useAppStore.setState({ progress: { 0: row }, batchProgress: { done: 0, total: 1 } });
});

describe("ConvertingScreen — §5.2 Converting (states 7/7a)", () => {
  it("renders the heading + the ProgressList (aggregate + rows) + the Cancel button", () => {
    const { getByRole, getByText } = render(<ConvertingScreen runId="r1" cancelling={false} />);
    expect(getByRole("heading", { name: "Converting" })).not.toBeNull();
    expect(getByText("0 of 1 file done")).not.toBeNull();
    expect(getByText("/a.csv")).not.toBeNull();
    expect(getByRole("button", { name: "Cancel" })).not.toBeNull();
  });

  it("Cancel fires C7 cancel_run for the live runId (cancelConversionRun) — the §5.2/§5.8 round-trip", () => {
    const { getByRole } = render(<ConvertingScreen runId="r1" cancelling={false} />);
    fireEvent.click(getByRole("button", { name: "Cancel" }));
    expect(cancelConversionRun).toHaveBeenCalledWith("r1");
  });

  it("Esc fires the same C7 cancel_run (§5.10)", () => {
    render(<ConvertingScreen runId="r1" cancelling={false} />);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(cancelConversionRun).toHaveBeenCalledWith("r1");
  });

  it("unbinds the Esc listener on unmount (no cancel after leaving Converting)", () => {
    const { unmount } = render(<ConvertingScreen runId="r1" cancelling={false} />);
    unmount();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(cancelConversionRun).not.toHaveBeenCalled();
  });

  it("7a: while cancelling the Cancel button is disabled + labelled 'Cancelling…'", () => {
    const { getByRole } = render(<ConvertingScreen runId="r1" cancelling={true} />);
    const button = getByRole("button", { name: "Cancelling…" });
    expect((button as HTMLButtonElement).disabled).toBe(true);
  });

  it("7a: a SECOND Esc while cancelling is ignored — no double C7 (§5.2 row 7a)", () => {
    render(<ConvertingScreen runId="r1" cancelling={true} />);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(cancelConversionRun).not.toHaveBeenCalled();
  });

  it("guards a double-cancel SYNCHRONOUSLY — a rapid double Esc (before the 7a re-render) fires C7 once", () => {
    // §5.2 row 7a "no second cancel_run": the `cancelling` prop flips only after the optimistic dispatch
    // re-renders, so the synchronous `cancellingRef` must block the second Esc in that sub-render window (the
    // race the prop-only guard would miss). Both Escapes fire with `cancelling={false}` (no re-render between).
    render(<ConvertingScreen runId="r1" cancelling={false} />);
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.keyDown(document, { key: "Escape" });
    expect(cancelConversionRun).toHaveBeenCalledTimes(1);
  });

  it("guards a double-cancel SYNCHRONOUSLY — a rapid double Cancel click fires C7 once", () => {
    const { getByRole } = render(<ConvertingScreen runId="r1" cancelling={false} />);
    const button = getByRole("button", { name: "Cancel" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(cancelConversionRun).toHaveBeenCalledTimes(1);
  });
});
