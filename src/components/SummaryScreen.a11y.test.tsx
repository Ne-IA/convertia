import { describe, it, expect, afterEach, vi } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.2 Summary screen — heading + ResultSummary + OpenActions + Convert
// more, rendered INTO a <main> like the other per-screen a11y legs. Mock the §5.1 events façade so the screen
// renders hermetically. [Build-Session-Entscheidung: P3.59]
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
  skipped: [],
  totalBytes: 10,
  rootsDisplay: ["/src"],
  encodingHint: null,
  delimiterHint: null,
  notes: [],
};

const result: RunResult = {
  collectedSetId: "cs1",
  runId: "r1",
  items: [{ item: 0, outputDisplay: "a.tsv", state: "succeeded", reason: null }],
  totals: { succeeded: 1, failed: 0, cancelled: 0, skipped: 0 },
  cleanupIncomplete: [],
  commonRootDisplay: "/src",
  divertRootDisplay: null,
  summaryLineDisplay: "All 1 files converted.",
};

afterEach(() => {
  cleanup();
  useAppStore.setState({ machine: { tag: "idle" } });
});

describe("SummaryScreen — §5.6 a11y (G33a per-push target)", () => {
  it("renders with no axe ARIA/role/focus violations", async () => {
    const { container } = render(
      <main>
        <SummaryScreen result={result} set={set} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("renders the split-divert form with no axe violations", async () => {
    const { container } = render(
      <main>
        <SummaryScreen result={{ ...result, divertRootDisplay: "/Downloads" }} set={set} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("introduces no positive tabindex (roving-tabindex sanity, G33a)", () => {
    const { container } = render(
      <main>
        <SummaryScreen result={result} set={set} />
      </main>,
    );
    const positive = Array.from(container.querySelectorAll("[tabindex]"))
      .map((element) => Number(element.getAttribute("tabindex")))
      .filter((value) => value > 0);
    expect(positive).toEqual([]);
  });
});
