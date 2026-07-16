import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, cleanup, fireEvent } from "@testing-library/react";

// §6.4.6 unit (G15): the §5.3 ResultSummary — the §1.12 end-of-batch outcome. Mock the §5.1 events façade (the
// C9 reveal-residue round-trip). The load-bearing assertions are the §2.6.4 THREE-CASE residue contract (the
// item's terminal state is never rewritten by residue — §2.6.2:827 / §2.1.3:197 "annotated, not an item
// failure") and the §5.7:800 verbatim-reason rule. [Build-Session-Entscheidung: P3.59]
const openResultTarget = vi.fn<(...args: unknown[]) => Promise<void>>();
vi.mock("../lib/ipc/events", () => ({
  openResultTarget: (...args: unknown[]) => openResultTarget(...args),
}));

import { ResultSummary } from "./ResultSummary";
import type { ItemId, ItemResult, RunResult } from "../lib/ipc/commands";

const sources = new Map<ItemId, string>([
  [0, "a.csv"],
  [1, "b.csv"],
  [2, "c.csv"],
]);

const succeeded = (item: ItemId, output: string): ItemResult => ({
  item,
  outputDisplay: output,
  state: "succeeded",
  reason: null,
});

const failed = (item: ItemId, text: string): ItemResult => ({
  item,
  outputDisplay: null,
  state: { failed: "engineError" },
  reason: { type: "failure", data: { kind: "engineError", text } },
});

const result = (items: ItemResult[], over: Partial<RunResult> = {}): RunResult => {
  const totals = { succeeded: 0, failed: 0, cancelled: 0, skipped: 0 };
  for (const item of items) {
    if (item.state === "succeeded") totals.succeeded += 1;
    else if (item.state === "cancelled") totals.cancelled += 1;
    else if (typeof item.state === "object" && item.state.skipped !== undefined)
      totals.skipped += 1;
    else totals.failed += 1;
  }
  return {
    collectedSetId: "cs1",
    runId: "r1",
    items,
    totals,
    cleanupIncomplete: [],
    commonRootDisplay: "/src",
    divertRootDisplay: null,
    // The core-assembled §2.8.2 line (§1.12). Fixtures set it explicitly per case; the ASSEMBLY is pinned
    // Rust-side (`run_result_wire_summary_line_matches_the_assembler`) — here we pin that the UI renders it
    // verbatim and never authors copy of its own.
    summaryLineDisplay: "All 1 files converted.",
    ...over,
  };
};

afterEach(cleanup);
beforeEach(() => {
  openResultTarget.mockReset();
  openResultTarget.mockResolvedValue(undefined);
});

describe("ResultSummary — §5.3 / §1.12", () => {
  it("renders one row per item, naming its SOURCE from the frozen set (the §1.12 output→source map)", () => {
    const { getByText } = render(
      <ResultSummary
        result={result([succeeded(0, "a.tsv"), succeeded(1, "b.tsv")])}
        sources={sources}
      />,
    );
    expect(getByText("a.csv")).not.toBeNull();
    expect(getByText("b.csv")).not.toBeNull();
  });

  it("maps each succeeded output back to its source ('Saved as …', SSOT How It Feels 7)", () => {
    const { getByText } = render(
      <ResultSummary result={result([succeeded(0, "a.tsv")])} sources={sources} />,
    );
    expect(getByText("Saved as a.tsv")).not.toBeNull();
  });

  it("renders a failed item's §2.8 reason VERBATIM — the core-supplied text, never paraphrased (§5.7:800)", () => {
    const text = "ConvertIA couldn't convert this file.";
    const { getByText } = render(
      <ResultSummary result={result([failed(0, text)])} sources={sources} />,
    );
    expect(getByText(text)).not.toBeNull();
  });

  it("renders NO output line for a non-succeeded item (§1.12: outputDisplay is Some only on success)", () => {
    const { queryByText } = render(
      <ResultSummary result={result([failed(0, "nope")])} sources={sources} />,
    );
    expect(queryByText(/Saved as/)).toBeNull();
  });

  describe("the §1.12 batch summary line + the fully-failed banner (§5.2 row 8 — never a quiet 'done')", () => {
    // [Test-Change: P3.59 — old-obsolete+new-correct, §1.12] These expectations moved from the chrome literal
    // "No files were converted" to the core's `summaryLineDisplay`. OLD OBSOLETE: the 2026-07-16 P3.59 ruling
    // wired `batch_summary_line` onto the wire and REMOVED the chrome banner string — §2.8.2 owns the copy
    // (§5.7:799), so asserting a UI-authored literal now asserts a §5.7 violation. NEW CORRECT: verified vs
    // §2.8.2 (the "All failed" row is the §02 string) + §5.2 row 8 (the UI owns the BANNER, not the words);
    // the line's assembly is read back Rust-side, and rendering it verbatim is pinned here.
    it("renders the core's §2.8.2 line VERBATIM as an alert banner when EVERY item failed", () => {
      const run = result([failed(0, "x"), failed(1, "y")], {
        summaryLineDisplay: "None of the 2 files could be converted.",
      });
      const { getByRole } = render(<ResultSummary result={run} sources={sources} />);
      expect(getByRole("alert").textContent).toBe("None of the 2 files could be converted.");
    });

    it("renders the line WITHOUT the alert banner on a partial run (one success is not a fully-failed batch)", () => {
      const run = result([succeeded(0, "a.tsv"), failed(1, "y")], {
        summaryLineDisplay: "1 of 2 files converted. 1 couldn't be converted — see details.",
      });
      const { queryByRole, getByText } = render(<ResultSummary result={run} sources={sources} />);
      expect(queryByRole("alert")).toBeNull();
      expect(
        getByText("1 of 2 files converted. 1 couldn't be converted — see details."),
      ).not.toBeNull();
    });

    it("renders NO banner on an EMPTY run (total === 0 — the derived guard, not a div-by-zero 'all failed')", () => {
      const { queryByRole } = render(<ResultSummary result={result([])} sources={sources} />);
      expect(queryByRole("alert")).toBeNull();
    });

    it("banners a run where every ATTEMPTED item failed but a pre-flight SKIP is present (§2.8.2 AllFailed)", () => {
      // The regression this pins: the banner predicate must mirror the branch of `batch_summary` that PRODUCED
      // the line (AllFailed is scoped to the ATTEMPTED items — P3.50 excludes pre-flight skips from its {n}),
      // NOT §1.12's `failed == total` literal over all four tallies. Under the literal, {failed:2, skipped:1}
      // reads 2 !== 3 → no banner, so the core's "None of the 2 files could be converted." would render as calm
      // body text: a total failure shown as a quiet finish (§5.2 row 8 / SSOT *Fail clearly*), and an SR user
      // would lose the assertive announcement. Every other banner fixture here uses skipped: 0, so this case is
      // the only thing standing between that predicate and a silent regression.
      const skipped: ItemResult = {
        item: 2,
        outputDisplay: null,
        state: { skipped: "unsupportedType" },
        reason: { type: "skipped", data: { reason: "unsupportedType", text: "Unsupported type." } },
      };
      const run = result([failed(0, "x"), failed(1, "y"), skipped], {
        summaryLineDisplay: "None of the 2 files could be converted.",
      });
      expect(run.totals).toEqual({ succeeded: 0, failed: 2, cancelled: 0, skipped: 1 });
      const { getByRole } = render(<ResultSummary result={run} sources={sources} />);
      expect(getByRole("alert").textContent).toBe("None of the 2 files could be converted.");
    });

    it("renders NO banner on a CANCELLED run even with zero successes (§2.8.2: a cancel dominates the headline)", () => {
      // `batch_summary` classifies ANY cancelled item as `Cancelled` ("Stopped. …"), never AllFailed — so a
      // stopped run is not dressed as a total failure, however its attempted items ended.
      const cancelled: ItemResult = {
        item: 2,
        outputDisplay: null,
        state: "cancelled",
        reason: null,
      };
      const run = result([failed(0, "x"), cancelled], {
        summaryLineDisplay:
          "Stopped. 0 files were already converted and kept; the rest were not started.",
      });
      const { queryByRole, getByText } = render(<ResultSummary result={run} sources={sources} />);
      expect(queryByRole("alert")).toBeNull();
      expect(
        getByText("Stopped. 0 files were already converted and kept; the rest were not started."),
      ).not.toBeNull();
    });

    it("authors NO batch copy of its own — the rendered line is exactly what the wire carried (§5.7:799)", () => {
      // A sentinel the §2.8.2 catalog would never produce: if the component ever paraphrased/derived the line
      // instead of rendering the wire field, this fails.
      const run = result([failed(0, "x")], { summaryLineDisplay: "SENTINEL-LINE-FROM-CORE" });
      const { getByRole } = render(<ResultSummary result={run} sources={sources} />);
      expect(getByRole("alert").textContent).toBe("SENTINEL-LINE-FROM-CORE");
    });
  });

  describe("§2.6.4 residue — the item's terminal state is NEVER rewritten (three cases)", () => {
    // §2.6.4 case 1 as the core actually ships it (P3.59 ruling): state stays `succeeded` and the reason
    // carries the §2.8.2 NON-failure residue annotation, already naming {path}.
    const residueAnnotation = (item: ItemId, path: string): ItemResult => ({
      item,
      outputDisplay: "a.tsv",
      state: "succeeded",
      reason: {
        type: "residue",
        data: { text: `Converted — a temporary file may remain at ${path}.` },
      },
    });

    it("case 1: a SUCCEEDED item with residue stays Done — not downgraded to Failed (§2.6.2:827)", () => {
      const run = result([residueAnnotation(0, "/tmp/a.part")], {
        cleanupIncomplete: [{ item: 0, residueDisplay: "/tmp/a.part" }],
      });
      const { getByText, queryByText } = render(<ResultSummary result={run} sources={sources} />);
      expect(getByText("Done")).not.toBeNull();
      expect(queryByText("Failed")).toBeNull();
      // The §2.8.2 annotation is rendered VERBATIM — it says the success stands AND where residue remains.
      expect(getByText("Converted — a temporary file may remain at /tmp/a.part.")).not.toBeNull();
    });

    it("case 1: the residue line is the core's §02-owned text — the UI adds NO chrome path line of its own", () => {
      // §5.7:799: the UI must not author/paraphrase a §2.8 string. Pinning the exact rendered text set means a
      // re-introduced chrome frame ("A temporary file may remain at …") fails here.
      const run = result([residueAnnotation(0, "/tmp/a.part")], {
        cleanupIncomplete: [{ item: 0, residueDisplay: "/tmp/a.part" }],
      });
      const { container, queryByText } = render(<ResultSummary result={run} sources={sources} />);
      expect(queryByText(/^A temporary file may remain at/)).toBeNull();
      // The path appears exactly ONCE in the row (the pre-ruling fill rendered it twice — reason + chrome note).
      const occurrences = (container.textContent ?? "").split("/tmp/a.part").length - 1;
      expect(occurrences).toBe(1);
    });

    it("case 1: a succeeded-with-residue item is never announced as a clean success — the reveal link is shown", () => {
      const run = result([residueAnnotation(0, "/tmp/a.part")], {
        cleanupIncomplete: [{ item: 0, residueDisplay: "/tmp/a.part" }],
      });
      const { getByRole } = render(<ResultSummary result={run} sources={sources} />);
      // The reveal affordance is the §7.7 half of "says residue may remain AND where".
      expect(getByRole("button", { name: "Reveal residue" })).not.toBeNull();
    });

    it("case 2: a FAILED item with residue keeps its core-supplied §2.8.2 CleanupResidue line verbatim", () => {
      const text =
        "This file couldn't be converted, and a temporary file may remain at /tmp/b.part.";
      const items: ItemResult[] = [
        {
          item: 1,
          outputDisplay: null,
          state: { failed: "cleanupResidue" },
          reason: { type: "failure", data: { kind: "cleanupResidue", text } },
        },
      ];
      const run = result(items, {
        cleanupIncomplete: [{ item: 1, residueDisplay: "/tmp/b.part" }],
      });
      const { container, getByText } = render(<ResultSummary result={run} sources={sources} />);
      expect(getByText(text)).not.toBeNull();
      expect(getByText("Failed")).not.toBeNull();
      // The cleanup_residue row ALREADY names {path}; the UI adds no second path line (the pre-ruling fill's
      // chrome note rendered it twice — the G1 Sonnet P2 defect this resolves).
      expect((container.textContent ?? "").split("/tmp/b.part").length - 1).toBe(1);
    });

    it("case 3: a CANCELLED item with residue stays Cancelled, reason null — its surface is the reveal link", () => {
      // §2.6.4 authors NO per-item case-3 sentence (the With-residue tail is BATCH-level, §2.8.2), so the core
      // ships `reason: null` and the per-item surface is exactly the structural annotation + the C9 reveal.
      const items: ItemResult[] = [
        { item: 2, outputDisplay: null, state: "cancelled", reason: null },
      ];
      const run = result(items, {
        cleanupIncomplete: [{ item: 2, residueDisplay: "/tmp/c.part" }],
      });
      const { getByRole, getByText, queryByText } = render(
        <ResultSummary result={run} sources={sources} />,
      );
      expect(getByText("Cancelled")).not.toBeNull();
      expect(queryByText("Failed")).toBeNull();
      expect(getByRole("button", { name: "Reveal residue" })).not.toBeNull();
    });

    it("renders no residue note / reveal link for an item with no residue", () => {
      const { queryByRole, queryByText } = render(
        <ResultSummary result={result([succeeded(0, "a.tsv")])} sources={sources} />,
      );
      expect(queryByText(/temporary file may remain/)).toBeNull();
      expect(queryByRole("button", { name: "Reveal residue" })).toBeNull();
    });

    it("the reveal link fires C9 with the Residue(ItemId) id — never a path (§7.7.2)", () => {
      const run = result([succeeded(0, "a.tsv")], {
        cleanupIncomplete: [{ item: 0, residueDisplay: "/tmp/a.part" }],
      });
      const { getByRole } = render(<ResultSummary result={run} sources={sources} />);
      fireEvent.click(getByRole("button", { name: "Reveal residue" }));
      expect(openResultTarget).toHaveBeenCalledWith({ residue: 0 });
    });
  });

  it("renders a §1.12-projected pre-flight SKIP as a skip (never a failure) with its verbatim reason", () => {
    const text = "We couldn't tell what this file is.";
    const items: ItemResult[] = [
      {
        item: 2,
        outputDisplay: null,
        state: { skipped: "uncertain" },
        reason: { type: "skipped", data: { reason: "uncertain", text } },
      },
    ];
    const { getByText, queryByText } = render(
      <ResultSummary result={result(items)} sources={sources} />,
    );
    expect(getByText("Skipped")).not.toBeNull();
    expect(getByText(text)).not.toBeNull();
    expect(queryByText("Failed")).toBeNull();
  });
});
