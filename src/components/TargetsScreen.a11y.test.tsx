import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.2 Targets/Destination screen's own axe leg. Mock the §5.1 events façade
// so the render stays hermetic (no Tauri runtime under jsdom); the axe run is over the composed FormatPicker +
// DestinationBar + Back tree in a `<main>`. [Build-Session-Entscheidung: P3.56]
vi.mock("../lib/ipc/events", () => ({
  replanOutput: () => Promise.resolve(),
  pickAndSetDestination: () => Promise.resolve(),
  runConversion: () => Promise.resolve(),
}));

import { TargetsScreen } from "./TargetsScreen";
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
    rerun: null,
    preflight: { estTotalOutputBytes: 0, estTotalScratchBytes: 0, upFrontFail: null },
  },
  persistedFallback: false,
};

afterEach(cleanup);

describe("TargetsScreen — §5.6 a11y (G33a per-push target)", () => {
  it("renders with no axe violations", async () => {
    const { container } = render(
      <main>
        <TargetsScreen plan={plan} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });
});
