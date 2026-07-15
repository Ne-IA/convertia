import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.3 BatchSummary's own axe leg. Renders INTO a `<main>` (the App wraps
// state screens there) so the region best-practice rule is satisfied. Mock the announcer so the render stays
// hermetic (jsdom has no live-region assistive tech to drive). [Build-Session-Entscheidung: P3.55]
vi.mock("../a11y/announcer", () => ({ announce: () => undefined }));

import { BatchSummary } from "./BatchSummary";

afterEach(cleanup);

describe("BatchSummary — §5.6 a11y (G33a per-push target)", () => {
  it("renders with no axe violations (with the skip tally)", async () => {
    const { container } = render(
      <main>
        <BatchSummary count={48} format="csv" skippedCount={3} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("renders with no axe violations (clean drop, no tally)", async () => {
    const { container } = render(
      <main>
        <BatchSummary count={1} format="tsv" skippedCount={0} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });
});
