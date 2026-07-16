import { describe, it, expect, afterEach, vi } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.2 Converting screen — heading + ProgressList + Cancel, rendered INTO a
// <main> like the other per-screen a11y legs. Mock the §5.1 events façade so the screen renders hermetically
// (Cancel fires only on user input, unused here); seat the store's live progress. [Build-Session-Entscheidung: P3.58]
vi.mock("../lib/ipc/events", () => ({ cancelConversionRun: () => Promise.resolve() }));

import { ConvertingScreen } from "./ConvertingScreen";
import { useAppStore } from "../state/store";
import type { ItemRow } from "../state/store";

const row: ItemRow = { sourceDisplay: "/a.csv", status: "running", fraction: 0.5, reason: null };

afterEach(() => {
  cleanup();
  useAppStore.setState({ progress: {}, batchProgress: null });
});

describe("ConvertingScreen — §5.6 a11y (G33a per-push target)", () => {
  it("renders with no axe ARIA/role/focus violations", async () => {
    useAppStore.setState({ progress: { 0: row }, batchProgress: { done: 0, total: 1 } });
    const { container } = render(
      <main>
        <ConvertingScreen runId="r1" cancelling={false} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("introduces no positive tabindex (roving-tabindex sanity, G33a)", () => {
    useAppStore.setState({ progress: { 0: row }, batchProgress: { done: 0, total: 1 } });
    const { container } = render(
      <main>
        <ConvertingScreen runId="r1" cancelling={false} />
      </main>,
    );
    const positive = Array.from(container.querySelectorAll("[tabindex]"))
      .map((element) => Number(element.getAttribute("tabindex")))
      .filter((value) => value > 0);
    expect(positive).toEqual([]);
  });

  it("renders the 7a Cancelling… (disabled Cancel) state with no axe violations", async () => {
    useAppStore.setState({ progress: { 0: row }, batchProgress: { done: 0, total: 1 } });
    const { container } = render(
      <main>
        <ConvertingScreen runId="r1" cancelling={true} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });
});
