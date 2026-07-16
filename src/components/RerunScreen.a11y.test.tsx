import { describe, it, expect, afterEach, vi } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.2 RerunPrompt screen (state 6) — the alertdialog modal composed over the
// inert Targets/Destination backdrop, rendered INTO a `<main>` like the other per-screen a11y legs. Mock the §5.1
// events façade so the backdrop TargetsScreen renders hermetically (its actions fire only on user input, unused
// here). The backdrop is `inert` (no `aria-hidden`), so its focusable controls are out of tab order without
// tripping the axe `aria-hidden-focus` rule. [Build-Session-Entscheidung: P3.57]
vi.mock("../lib/ipc/events", () => ({
  replanOutput: () => Promise.resolve(),
  pickAndSetDestination: () => Promise.resolve(),
  runConversion: () => Promise.resolve(),
}));

import { RerunScreen } from "./RerunScreen";
import type { Planned, SingleSet } from "../state/machine";

const singleSet: SingleSet = {
  id: "cs1",
  instance: "inst-1",
  format: "csv",
  items: [],
  count: 1,
  skipped: [],
  totalBytes: 10,
  rootsDisplay: ["/drop"],
  encodingHint: null,
  delimiterHint: null,
  notes: [],
};

const plan: Planned = {
  set: singleSet,
  offer: {
    set: "cs1",
    targets: [
      { id: { format: "tsv" }, label: "TSV", lossy: null, availability: "available", options: [] },
    ],
    defaultTarget: { format: "tsv" },
  },
  selected: { format: "tsv" },
  options: {},
  destination: "besideSource",
  preview: {
    set: "cs1",
    finalDirDisplay: "/drop",
    diverted: null,
    rerun: { equivalentCount: 2 },
    preflight: { estTotalOutputBytes: 0, estTotalScratchBytes: 0, upFrontFail: null },
  },
  persistedFallback: false,
};

afterEach(cleanup);

describe("RerunScreen — §5.6 a11y (G33a per-push target)", () => {
  it("renders with no axe ARIA/role/focus violations (modal over the inert backdrop)", async () => {
    const { container } = render(
      <main>
        <RerunScreen plan={plan} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("introduces no positive tabindex (roving-tabindex sanity, G33a)", () => {
    const { container } = render(
      <main>
        <RerunScreen plan={plan} />
      </main>,
    );
    const positive = Array.from(container.querySelectorAll("[tabindex]"))
      .map((element) => Number(element.getAttribute("tabindex")))
      .filter((value) => value > 0);
    expect(positive).toEqual([]);
  });
});
