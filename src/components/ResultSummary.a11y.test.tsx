import { describe, it, expect, afterEach, vi } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.3 ResultSummary — rendered INTO a <main> like the other per-screen
// a11y legs. Mock the §5.1 events façade (the C9 reveal-residue call fires only on user input, unused here).
// The §5.6 Summary priority focus-on-entry + the assertive outcome announcement are P4.70.3/P4.75; this leg
// pins the structural ARIA the slice ships. [Build-Session-Entscheidung: P3.59]
vi.mock("../lib/ipc/events", () => ({ openResultTarget: () => Promise.resolve() }));

import { ResultSummary } from "./ResultSummary";
import type { ItemId, ItemResult, RunResult } from "../lib/ipc/commands";

const sources = new Map<ItemId, string>([
  [0, "a.csv"],
  [1, "b.csv"],
]);

const items: ItemResult[] = [
  { item: 0, outputDisplay: "a.tsv", state: "succeeded", reason: null },
  {
    item: 1,
    outputDisplay: null,
    state: { failed: "engineError" },
    reason: {
      type: "failure",
      data: { kind: "engineError", text: "ConvertIA couldn't convert this file." },
    },
  },
];

const result: RunResult = {
  collectedSetId: "cs1",
  runId: "r1",
  items,
  totals: { succeeded: 1, failed: 1, cancelled: 0, skipped: 0 },
  cleanupIncomplete: [],
  commonRootDisplay: "/src",
  divertRootDisplay: null,
  summaryLineDisplay: "1 of 2 files converted. 1 couldn't be converted — see details.",
};

afterEach(cleanup);

describe("ResultSummary — §5.6 a11y (G33a per-push target)", () => {
  it("renders a mixed (success + failure) run with no axe ARIA/role/focus violations", async () => {
    const { container } = render(
      <main>
        <ResultSummary result={result} sources={sources} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("renders a residue row (with its reveal button) with no axe violations", async () => {
    const withResidue: RunResult = {
      ...result,
      cleanupIncomplete: [{ item: 0, residueDisplay: "/tmp/a.part" }],
    };
    const { container } = render(
      <main>
        <ResultSummary result={withResidue} sources={sources} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("renders the fully-failed banner with no axe violations (role=alert is announced, not focused)", async () => {
    const allFailed: RunResult = {
      ...result,
      items: [items[1] as ItemResult],
      totals: { succeeded: 0, failed: 1, cancelled: 0, skipped: 0 },
    };
    const { container, getByRole } = render(
      <main>
        <ResultSummary result={allFailed} sources={sources} />
      </main>,
    );
    expect(getByRole("alert")).not.toBeNull();
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("introduces no positive tabindex (roving-tabindex sanity, G33a)", () => {
    const { container } = render(
      <main>
        <ResultSummary result={result} sources={sources} />
      </main>,
    );
    const positive = Array.from(container.querySelectorAll("[tabindex]"))
      .map((element) => Number(element.getAttribute("tabindex")))
      .filter((value) => value > 0);
    expect(positive).toEqual([]);
  });
});
