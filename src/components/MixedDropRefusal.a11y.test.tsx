import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.3 MixedDropRefusal (state 9) — an assertive-heading full-screen STATE
// (NOT an alertdialog, §5.7:840) composing the active re-drop DropZone, rendered INTO a `<main>` like the other
// per-screen a11y legs. Mock the §5.1 events façade (the composed DropZone fires C2a through it).
// [Build-Session-Entscheidung: P3.60]
vi.mock("../lib/ipc/events", () => ({ pickForIntake: () => Promise.resolve() }));

import { MixedDropRefusal } from "./MixedDropRefusal";
import type { MixedFound } from "../state/machine";

const found: MixedFound = [
  ["jpg", 30],
  ["png", 12],
];

afterEach(cleanup);

describe("MixedDropRefusal — §5.6 a11y (G33a per-push target)", () => {
  it("renders with no axe ARIA/role/focus violations", async () => {
    const { container } = render(
      <main>
        <MixedDropRefusal found={found} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("introduces no positive tabindex (roving-tabindex sanity, G33a)", () => {
    const { container } = render(
      <main>
        <MixedDropRefusal found={found} />
      </main>,
    );
    const positive = Array.from(container.querySelectorAll("[tabindex]"))
      .map((element) => Number(element.getAttribute("tabindex")))
      .filter((value) => value > 0);
    expect(positive).toEqual([]);
  });
});
