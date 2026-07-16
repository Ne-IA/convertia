import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.3 RerunPrompt modal — the focus-trapped `role="alertdialog"` with its
// heading-derived accessible name (aria-labelledby) + body description (aria-describedby) + the three controls,
// rendered INTO a `<main>` like the other per-screen a11y legs. Callbacks are no-op `vi.fn`s (presentational).
// [Build-Session-Entscheidung: P3.57]
import { RerunPrompt } from "./RerunPrompt";

afterEach(cleanup);

describe("RerunPrompt — §5.6 a11y (G33a per-push target)", () => {
  it("renders with no axe ARIA/role/focus violations", async () => {
    const { container } = render(
      <main>
        <RerunPrompt onSkip={vi.fn()} onFreshCopy={vi.fn()} onCancel={vi.fn()} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("introduces no positive tabindex (roving-tabindex sanity, G33a)", () => {
    const { container } = render(
      <main>
        <RerunPrompt onSkip={vi.fn()} onFreshCopy={vi.fn()} onCancel={vi.fn()} />
      </main>,
    );
    const positive = Array.from(container.querySelectorAll("[tabindex]"))
      .map((element) => Number(element.getAttribute("tabindex")))
      .filter((value) => value > 0);
    expect(positive).toEqual([]);
  });
});
