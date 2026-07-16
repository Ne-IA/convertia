import { describe, it, expect, afterEach, vi } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.3 OpenActions — rendered INTO a <main> like the other per-screen a11y
// legs. Mock the §5.1 events façade so the component renders hermetically (C9 fires only on user input, unused
// here). Both renderings are covered: the single common-root button and the split-divert pair.
// [Build-Session-Entscheidung: P3.59]
vi.mock("../lib/ipc/events", () => ({ openResultTarget: () => Promise.resolve() }));

import { OpenActions } from "./OpenActions";

afterEach(cleanup);

describe("OpenActions — §5.6 a11y (G33a per-push target)", () => {
  it("renders the single-button (no divert) form with no axe ARIA/role/focus violations", async () => {
    const { container } = render(
      <main>
        <OpenActions commonRootDisplay="/src" divertRootDisplay={null} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("renders the split-divert TWO-button form with no axe violations", async () => {
    const { container } = render(
      <main>
        <OpenActions commonRootDisplay="/src" divertRootDisplay="/Downloads" />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("gives each split-divert button a DISTINCT visible accessible name (§5.3 [DECIDED] labels)", () => {
    // §5.3: the two buttons must be told apart by name alone — "Open folder" twice would strand an SR user.
    const { getAllByRole } = render(
      <main>
        <OpenActions commonRootDisplay="/src" divertRootDisplay="/Downloads" />
      </main>,
    );
    const names = getAllByRole("button").map((button) => button.textContent);
    expect(names).toEqual(["Open source folder", "Open saved-to folder"]);
    expect(new Set(names).size).toBe(names.length);
  });

  it("introduces no positive tabindex (roving-tabindex sanity, G33a)", () => {
    const { container } = render(
      <main>
        <OpenActions commonRootDisplay="/src" divertRootDisplay="/Downloads" />
      </main>,
    );
    const positive = Array.from(container.querySelectorAll("[tabindex]"))
      .map((element) => Number(element.getAttribute("tabindex")))
      .filter((value) => value > 0);
    expect(positive).toEqual([]);
  });
});
