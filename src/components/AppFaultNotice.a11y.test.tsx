import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { axe } from "vitest-axe";

// §6.4.6a / G33a a11y (per-push): the §5.3 AppFaultNotice (state 12) — an assertive-heading full-screen STATE
// (NOT an alertdialog, §5.7:840) + the single focusable Start-over action, rendered INTO a `<main>` like the
// other per-screen a11y legs. [Build-Session-Entscheidung: P3.60]
import { AppFaultNotice } from "./AppFaultNotice";
import type { AppFault } from "../lib/ipc/commands";

const fault: AppFault = {
  kind: "bundleDamaged",
  message:
    "ConvertIA can't start because part of the app appears to be missing or damaged. Try downloading it again from the official releases page.",
};

afterEach(cleanup);

describe("AppFaultNotice — §5.6 a11y (G33a per-push target)", () => {
  it("renders with no axe ARIA/role/focus violations", async () => {
    const { container } = render(
      <main>
        <AppFaultNotice fault={fault} onStartOver={() => undefined} />
      </main>,
    );
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });

  it("introduces no positive tabindex (roving-tabindex sanity, G33a)", () => {
    const { container } = render(
      <main>
        <AppFaultNotice fault={fault} onStartOver={() => undefined} />
      </main>,
    );
    const positive = Array.from(container.querySelectorAll("[tabindex]"))
      .map((element) => Number(element.getAttribute("tabindex")))
      .filter((value) => value > 0);
    expect(positive).toEqual([]);
  });
});
