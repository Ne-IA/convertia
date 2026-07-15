import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.2 Collecting screen — the role="status" region + the cancel button.
// Rendered INTO a `<main>` like the other per-screen a11y legs. [Build-Session-Entscheidung: P3.55]
vi.mock("../lib/ipc/events", () => ({ cancelIntakeCollect: () => Promise.resolve() }));

import { CollectingScreen } from "./CollectingScreen";

afterEach(cleanup);

describe("CollectingScreen — §5.6 a11y (G33a per-push target)", () => {
  it("renders with no axe violations (with a live count)", async () => {
    const { container } = render(
      <main>
        <CollectingScreen collectingId="c1" scanned={42} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("renders with no axe violations (indeterminate)", async () => {
    const { container } = render(
      <main>
        <CollectingScreen collectingId="c1" scanned={null} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });
});
