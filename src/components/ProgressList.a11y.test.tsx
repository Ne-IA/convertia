import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.3 ProgressList — the native <progress> bars (implicit role=progressbar)
// each labelled by their row / the aggregate line, rendered INTO a <main> like the other per-screen a11y legs.
// A mix of a running row (determinate + indeterminate bar) and terminal rows exercises the row variants.
// [Build-Session-Entscheidung: P3.58]
import { ProgressList } from "./ProgressList";
import type { ItemRow } from "../state/store";

const rows: Readonly<Record<number, ItemRow>> = {
  0: { sourceDisplay: "/a.csv", status: "succeeded", fraction: 1, reason: null },
  1: { sourceDisplay: "/b.csv", status: "running", fraction: 0.5, reason: null },
  2: { sourceDisplay: "/c.csv", status: "running", fraction: null, reason: null },
  3: {
    sourceDisplay: "/d.csv",
    status: "failed",
    fraction: 0.2,
    reason: "Couldn't read this file.",
  },
};

afterEach(cleanup);

describe("ProgressList — §5.6 a11y (G33a per-push target)", () => {
  it("renders with no axe ARIA/role/focus violations (labelled progressbars)", async () => {
    const { container } = render(
      <main>
        <ProgressList rows={rows} batchProgress={{ done: 1, total: 4 }} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("introduces no positive tabindex (roving-tabindex sanity, G33a)", () => {
    const { container } = render(
      <main>
        <ProgressList rows={rows} batchProgress={{ done: 1, total: 4 }} />
      </main>,
    );
    const positive = Array.from(container.querySelectorAll("[tabindex]"))
      .map((element) => Number(element.getAttribute("tabindex")))
      .filter((value) => value > 0);
    expect(positive).toEqual([]);
  });
});
