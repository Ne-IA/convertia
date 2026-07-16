import { describe, it, expect } from "vitest";

import {
  divertNote,
  fill,
  formatBatchProgress,
  formatConfirmAnnouncement,
  formatConfirmCount,
  formatDisclosure,
  formatLabel,
  formatMixedFound,
  formatScanStatus,
  formatSkipBreakdown,
  formatSkipRow,
  formatSkipTally,
  formatWillSaveTo,
  skipReasonLabel,
} from "./format";
import type { SkipReason } from "../lib/ipc/commands";
import { ui } from "./ui";

// §6.4.6 unit (G15): the §5.7 confirm-gate string FORMATTERS (P3.55). ui.ts stays a flat English string map
// (ui.test.ts pins that); these pure functions compose its templates. Each leg asserts the SUBSTITUTED result
// so a template/placeholder drift reddens, and the §5.6 "one source" tie (announcement built from the same
// count/tally templates as the visible line) is pinned. [Build-Session-Entscheidung: P3.55]
describe("fill (template substitution)", () => {
  it("replaces every {token} with its var (all occurrences)", () => {
    expect(fill("{a} and {a} then {b}", { a: 1, b: "x" })).toBe("1 and 1 then x");
  });

  it("leaves an un-provided token literal (an authoring bug is visible, not silently blank)", () => {
    expect(fill("{a}-{missing}", { a: "z" })).toBe("z-{missing}");
  });
});

describe("formatLabel (source-format display token)", () => {
  it("uppercases the §0.6 UserFacingFormat for the confirm gate (csv → CSV, tsv → TSV)", () => {
    expect(formatLabel("csv")).toBe("CSV");
    expect(formatLabel("tsv")).toBe("TSV");
  });
});

describe("formatConfirmCount (§5.2 count line)", () => {
  it("uses the singular template for a count of 1", () => {
    expect(formatConfirmCount(1, "csv")).toBe("1 CSV file");
  });

  it("uses the plural template for a count > 1", () => {
    expect(formatConfirmCount(48, "csv")).toBe("48 CSV files");
  });
});

describe("formatSkipTally (§5.2/§1.4 passive tally)", () => {
  it("singular for 1 skipped, plural for many", () => {
    expect(formatSkipTally(1)).toBe("1 file wasn't recognized and will be skipped");
    expect(formatSkipTally(3)).toBe("3 files weren't recognized and will be skipped");
  });
});

describe("formatConfirmAnnouncement (§5.6 assertive summary)", () => {
  it("is the bare count line when nothing was skipped", () => {
    expect(formatConfirmAnnouncement(48, "csv", 0)).toBe("48 CSV files");
  });

  it("joins the count line and the skip tally with an em-dash when items were skipped (§5.6 pattern)", () => {
    expect(formatConfirmAnnouncement(48, "csv", 3)).toBe(
      "48 CSV files — 3 files weren't recognized and will be skipped",
    );
  });

  it("is built from the SAME templates as the visible count + tally (§5.6 one source, no divergence)", () => {
    // The announcement's parts are exactly the visible-line formatters — proving the single-source tie.
    expect(formatConfirmAnnouncement(2, "tsv", 5)).toBe(
      `${formatConfirmCount(2, "tsv")} — ${formatSkipTally(5)}`,
    );
  });
});

describe("formatScanStatus (§5.2 Collecting status)", () => {
  it("is the indeterminate fallback when no count has arrived (scanned === null)", () => {
    expect(formatScanStatus(null)).toBe(ui.collecting_indeterminate);
  });

  it("singular/plural on the throttled onScan count", () => {
    expect(formatScanStatus(1)).toBe("Scanning… 1 file so far");
    expect(formatScanStatus(42)).toBe("Scanning… 42 files so far");
  });
});

describe("formatDisclosure (§5.3 FileList Show/Hide N files)", () => {
  it("collapsed → Show; expanded → Hide; singular/plural on the total", () => {
    expect(formatDisclosure(1, false)).toBe("Show 1 file");
    expect(formatDisclosure(51, false)).toBe("Show 51 files");
    expect(formatDisclosure(1, true)).toBe("Hide 1 file");
    expect(formatDisclosure(51, true)).toBe("Hide 51 files");
  });
});

describe("skipReasonLabel (§5.7 confirm-gate SkipReason labels)", () => {
  it("maps every §0.6 SkipReason variant to a non-empty label (exhaustive)", () => {
    for (const reason of [
      "unsupportedType",
      "uncertain",
      "empty",
      "unreadable",
      "alreadyConverted",
    ] as const) {
      expect(skipReasonLabel(reason).trim()).not.toBe("");
    }
    expect(skipReasonLabel("unsupportedType")).toBe(ui.skip_reason_unsupported_type);
    expect(skipReasonLabel("unreadable")).toBe(ui.skip_reason_unreadable);
  });
});

describe("formatSkipRow (§5.3 skipped-row reason line)", () => {
  it("wraps the bare label in the 'Skipped — {reason}' frame when no detected name (§5.6 textual, not colour)", () => {
    expect(formatSkipRow("unreadable", null)).toBe("Skipped — Couldn't read this file");
  });

  it("appends the retained detected-type name when the item carried one (§0.6 detectedDisplay)", () => {
    expect(formatSkipRow("unsupportedType", "PDF")).toBe(
      "Skipped — Unsupported type — detected: PDF",
    );
  });
});

describe("formatWillSaveTo (§5.3 will-save-to line)", () => {
  it("wraps the C4 plan's finalDirDisplay in the chrome frame", () => {
    expect(formatWillSaveTo("C:/Users/me/Downloads")).toBe("Will save to C:/Users/me/Downloads");
  });
});

describe("divertNote (§2.7.2 per-location divert)", () => {
  it("maps every §0.6 DivertReason variant to a non-empty note (exhaustive)", () => {
    for (const reason of ["unwritable", "ephemeral", "noAtomicPublish"] as const) {
      expect(divertNote(reason).trim()).not.toBe("");
    }
    expect(divertNote("unwritable")).toBe(ui.destination_divert_unwritable);
    expect(divertNote("ephemeral")).toBe(ui.destination_divert_ephemeral);
    expect(divertNote("noAtomicPublish")).toBe(ui.destination_divert_no_atomic_publish);
  });
});

describe("formatBatchProgress (§5.2/§1.11 Converting aggregate line, P3.58)", () => {
  it("uses the singular template for a single-file batch (total === 1)", () => {
    expect(formatBatchProgress(0, 1)).toBe("0 of 1 file done");
    expect(formatBatchProgress(1, 1)).toBe("1 of 1 file done");
  });

  it("uses the plural template for a multi-file batch and substitutes done + total", () => {
    expect(formatBatchProgress(1, 2)).toBe("1 of 2 files done");
    expect(formatBatchProgress(3, 10)).toBe("3 of 10 files done");
  });
});

describe("formatMixedFound (§5.2 row-9 formats-found line, P3.60)", () => {
  it("renders the §5.2 row-9 example verbatim — every format with its count, uppercased", () => {
    expect(
      formatMixedFound([
        ["jpg", 30],
        ["png", 12],
        ["pdf", 3],
      ]),
    ).toBe("Found 30 JPG, 12 PNG, 3 PDF");
  });

  it("keeps the WIRE's order — the UI does not re-rank the core's refusal tally", () => {
    expect(
      formatMixedFound([
        ["pdf", 3],
        ["jpg", 30],
      ]),
    ).toBe("Found 3 PDF, 30 JPG");
  });

  it("handles the minimum mixed drop (two formats — one is not a mixed drop at all)", () => {
    expect(
      formatMixedFound([
        ["csv", 1],
        ["tsv", 1],
      ]),
    ).toBe("Found 1 CSV, 1 TSV");
  });
});

describe("formatSkipBreakdown (§5.2 row-10 Empty per-reason tally, P3.60)", () => {
  const skip = (reason: SkipReason): { reason: SkipReason } => ({ reason });

  it("groups by SkipReason and renders the §5.2 row-10 sentence", () => {
    expect(
      formatSkipBreakdown([
        skip("unreadable"),
        skip("unreadable"),
        skip("unreadable"),
        skip("unsupportedType"),
        skip("unsupportedType"),
      ]),
    ).toBe("5 files, none convertible (3 unreadable, 2 unsupported)");
  });

  it("uses the singular template for a one-file drop", () => {
    expect(formatSkipBreakdown([skip("unreadable")])).toBe(
      "1 file, none convertible (1 unreadable)",
    );
  });

  it("the breakdown counts always sum to the stated total", () => {
    const skipped = [skip("unreadable"), skip("empty"), skip("uncertain"), skip("unreadable")];
    const line = formatSkipBreakdown(skipped) ?? "";
    expect(line).toBe("4 files, none convertible (2 unreadable, 1 empty, 1 unrecognized)");
    const summed = [...line.matchAll(/(\d+) [a-z]/g)]
      .slice(1)
      .reduce((total, [, n]) => total + Number(n), 0);
    expect(summed).toBe(skipped.length);
  });

  it("returns null for the all-hidden Empty { skipped: [] } — the §5.2 'plain copy, no tally' case", () => {
    expect(formatSkipBreakdown([])).toBeNull();
  });

  it("covers every §0.6 SkipReason with a non-empty lowercase word (the exhaustive switch)", () => {
    const reasons: SkipReason[] = [
      "unsupportedType",
      "uncertain",
      "empty",
      "unreadable",
      "alreadyConverted",
    ];
    for (const reason of reasons) {
      const line = formatSkipBreakdown([skip(reason)]) ?? "";
      const word = /\(1 (.+)\)/.exec(line)?.[1] ?? "";
      expect(word.trim()).not.toBe("");
      expect(word).toBe(word.toLowerCase());
    }
  });
});
