import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.2 Summary screen (state 8) — the state-8 out-transition (the P3 screen-box wiring
// model: a rendered action MUST fire its command) + the §1.12 output→source resolution over the threaded frozen
// set. Mock the §5.1 events façade (the C9 shell-out); the store drives the real machine.
// [Build-Session-Entscheidung: P3.59]
vi.mock("../lib/ipc/events", () => ({ openResultTarget: () => Promise.resolve() }));

import { SummaryScreen } from "./SummaryScreen";
import { useAppStore } from "../state/store";
import type { SingleSet } from "../state/machine";
import type { RunResult } from "../lib/ipc/commands";

const set: SingleSet = {
  id: "cs1",
  instance: "i1",
  format: "csv",
  items: [
    {
      item: 0,
      displayName: "a.csv",
      relPathDisplay: null,
      sizeBytes: 10,
      detected: { recognized: { format: "csv", confidence: "high", dims: null } },
    },
  ],
  count: 1,
  skipped: [{ item: 1, sourceDisplay: "weird.bin", detectedDisplay: null, reason: "unreadable" }],
  totalBytes: 10,
  rootsDisplay: ["/src"],
  encodingHint: null,
  delimiterHint: null,
  notes: [],
};

const result: RunResult = {
  collectedSetId: "cs1",
  runId: "r1",
  items: [
    { item: 0, outputDisplay: "a.tsv", state: "succeeded", reason: null },
    {
      item: 1,
      outputDisplay: null,
      state: { skipped: "unreadable" },
      reason: {
        type: "skipped",
        data: { reason: "unreadable", text: "We couldn't read this file." },
      },
    },
  ],
  totals: { succeeded: 1, failed: 0, cancelled: 0, skipped: 1 },
  cleanupIncomplete: [],
  commonRootDisplay: "/src",
  divertRootDisplay: null,
  summaryLineDisplay: "All 1 files converted.",
};

afterEach(() => {
  cleanup();
  useAppStore.setState({ machine: { tag: "idle" } });
});
beforeEach(() => {
  useAppStore.setState({ machine: { tag: "summary", result, set } });
});

describe("SummaryScreen — §5.2 Summary (state 8)", () => {
  it("renders the heading + the ResultSummary rows + the OpenActions + Convert more", () => {
    const { getByRole, getByText } = render(<SummaryScreen result={result} set={set} />);
    expect(getByRole("heading", { name: "Results" })).not.toBeNull();
    expect(getByText("a.csv")).not.toBeNull();
    expect(getByRole("button", { name: "Open folder" })).not.toBeNull();
    expect(getByRole("button", { name: "Convert more" })).not.toBeNull();
  });

  it("names an ELIGIBLE item's source from the frozen set's displayName (§1.12 output→source map)", () => {
    const { getByText } = render(<SummaryScreen result={result} set={set} />);
    expect(getByText("a.csv")).not.toBeNull();
    expect(getByText("Saved as a.tsv")).not.toBeNull();
  });

  it("names a PRE-FLIGHT SKIP's source from the frozen set's skipped view — the live progress map has no row for it", () => {
    // §0.4.2: a pre-flight skip emits no `ItemStarted`, so the store's `progress` map could never name it; the
    // frozen set's `skipped` view spans the rest of the §0.6-invariant-6 id space (the P3.59 derivation).
    const { getByText } = render(<SummaryScreen result={result} set={set} />);
    expect(getByText("weird.bin")).not.toBeNull();
  });

  it("'Convert more' dispatches convertMore → the machine returns to Idle (§5.2 row 8)", () => {
    const { getByRole } = render(<SummaryScreen result={result} set={set} />);
    fireEvent.click(getByRole("button", { name: "Convert more" }));
    expect(useAppStore.getState().machine).toEqual({ tag: "idle" });
  });

  it("renders NO DropZone — Summary binds no single-chord picker (§5.3 [DECIDED])", () => {
    const { queryByRole } = render(<SummaryScreen result={result} set={set} />);
    expect(queryByRole("button", { name: /Drop files here/ })).toBeNull();
  });

  it("renders the split-divert TWO-button rendering when the run diverted (§2.7.3 → §7.7.1)", () => {
    const diverted: RunResult = { ...result, divertRootDisplay: "/Downloads" };
    const { getByRole, getByText } = render(<SummaryScreen result={diverted} set={set} />);
    expect(getByRole("button", { name: "Open source folder" })).not.toBeNull();
    expect(getByRole("button", { name: "Open saved-to folder" })).not.toBeNull();
    expect(getByText("Some files were saved to /Downloads")).not.toBeNull();
  });
});
