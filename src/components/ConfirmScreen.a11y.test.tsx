import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.2 Confirm-gate screen — the summary heading, the Continue/Cancel
// actions, and the FileList disclosure, in DOM order (§5.6.1 traversal). Rendered INTO a `<main>` like the
// other per-screen a11y legs. Mock the events façade + announcer for a hermetic render. [Build-Session-Entscheidung: P3.55]
vi.mock("../lib/ipc/events", () => ({ advanceToTargets: () => Promise.resolve() }));
vi.mock("../a11y/announcer", () => ({ announce: () => undefined }));

import { ConfirmScreen } from "./ConfirmScreen";
import type { SingleSet } from "../state/machine";

const set: SingleSet = {
  id: "cs1",
  instance: "inst-1",
  format: "csv",
  items: [],
  count: 3,
  skipped: [
    { item: 1, sourceDisplay: "notes.pdf", detectedDisplay: "PDF", reason: "unsupportedType" },
  ],
  totalBytes: 100,
  rootsDisplay: ["C:/drop"],
  encodingHint: null,
  delimiterHint: null,
  notes: [],
};

afterEach(cleanup);

describe("ConfirmScreen — §5.6 a11y (G33a per-push target)", () => {
  it("renders with no axe ARIA/role/focus violations", async () => {
    const { container } = render(
      <main>
        <ConfirmScreen set={set} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("introduces no positive tabindex (roving-tabindex sanity, G33a)", () => {
    const { container } = render(
      <main>
        <ConfirmScreen set={set} />
      </main>,
    );
    const positive = Array.from(container.querySelectorAll("[tabindex]"))
      .map((element) => Number(element.getAttribute("tabindex")))
      .filter((value) => value > 0);
    expect(positive).toEqual([]);
  });
});
