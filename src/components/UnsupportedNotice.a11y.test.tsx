import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.3 UnsupportedNotice (state 10) — an assertive-heading full-screen
// STATE (NOT an alertdialog, §5.7:840) + a focusable Dismiss, rendered INTO a `<main>` like the other
// per-screen a11y legs. Each of the four §5.3 variants is scanned: they differ in the rendered subtree (the
// Uncertain note line, the Empty tally line), so one variant's clean scan does not cover the others.
// [Build-Session-Entscheidung: P3.60]
import { UnsupportedNotice } from "./UnsupportedNotice";
import type { UnsupportedReason } from "../state/machine";

const variants: readonly (readonly [string, UnsupportedReason])[] = [
  ["Unsupported", { kind: "unsupported", detected: "PDF" }],
  ["Uncertain", { kind: "uncertain", note: "The header and the tail disagree." }],
  [
    "Unreadable",
    {
      kind: "empty",
      skipped: [
        { item: 0, sourceDisplay: "a.bin", detectedDisplay: null, reason: "unreadable" },
        { item: 1, sourceDisplay: "b.bin", detectedDisplay: null, reason: "unreadable" },
      ],
    },
  ],
  [
    "Empty",
    {
      kind: "empty",
      skipped: [
        { item: 0, sourceDisplay: "a.bin", detectedDisplay: null, reason: "unsupportedType" },
      ],
    },
  ],
];

afterEach(cleanup);

describe("UnsupportedNotice — §5.6 a11y (G33a per-push target)", () => {
  it.each(variants)("renders the %s variant with no axe violations", async (_name, reason) => {
    const { container } = render(
      <main>
        <UnsupportedNotice reason={reason} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("introduces no positive tabindex (roving-tabindex sanity, G33a)", () => {
    const { container } = render(
      <main>
        <UnsupportedNotice reason={{ kind: "unsupported", detected: "PDF" }} />
      </main>,
    );
    const positive = Array.from(container.querySelectorAll("[tabindex]"))
      .map((element) => Number(element.getAttribute("tabindex")))
      .filter((value) => value > 0);
    expect(positive).toEqual([]);
  });
});
