import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.3 DropZone's own attributable axe leg — the per-screen a11y split the
// App.a11y.test.tsx baseline anticipates ("each state screen adds its own axe assertions as it lands"). The
// DropZone renders INTO the §5.5 `<main>` workspace (App wraps it there), so the legs mount it in a `<main>`
// too — an unlandmarked fragment would false-positive axe's `region` best-practice rule. Mock the §5.1 IPC
// façade so each render stays hermetic under jsdom (no Tauri runtime). The vitest-axe `toHaveNoViolations`
// matcher is not used — its 0.1.0 .d.ts re-exports the matcher type-only, which verbatimModuleSyntax rejects —
// so the assertions read the axe() result directly (the App.a11y.test.tsx precedent). [Build-Session-Entscheidung: P3.54]
vi.mock("../lib/ipc/events", () => ({
  pickForIntake: () => Promise.resolve(),
}));

import { DropZone } from "./DropZone";

// Unmount between legs (vitest.a11y.config.ts does not set globals:true, so RTL auto-cleanup never registers).
afterEach(cleanup);

describe("DropZone — §5.6 a11y (G33a per-push target)", () => {
  it("renders with no axe ARIA/role/focus violations (Idle, enabled)", async () => {
    const { container } = render(
      <main>
        <DropZone />
      </main>,
    );
    const results = await axe(container);
    // Mapped to rule ids so a failure names the violated axe rule, not an opaque object dump.
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("renders with no axe violations while disabled (the state-9 / §5.8 inert form)", async () => {
    const { container } = render(
      <main>
        <DropZone disabled />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("names both intake actions accessibly (each action <button> carries a non-empty accessible name)", () => {
    const { getAllByRole } = render(
      <main>
        <DropZone />
      </main>,
    );
    const buttons = getAllByRole("button");
    expect(buttons).toHaveLength(2);
    for (const button of buttons) {
      expect(button.textContent?.trim()).not.toBe("");
    }
  });

  it("introduces no positive tabindex (roving-tabindex sanity, G33a)", () => {
    const { container } = render(
      <main>
        <DropZone />
      </main>,
    );
    const positive = Array.from(container.querySelectorAll("[tabindex]"))
      .map((element) => Number(element.getAttribute("tabindex")))
      .filter((value) => value > 0);
    expect(positive).toEqual([]);
  });
});
