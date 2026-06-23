import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { axe } from "vitest-axe";

import { App } from "./App";

// Section 6.4.6a / G33a: the mounted React shell renders with no axe ARIA/role/focus violations
// under jsdom. The vitest-axe toHaveNoViolations matcher is not used -- its 0.1.0 .d.ts re-exports
// the matcher type-only, which verbatimModuleSyntax rejects -- so we assert on the axe() result's
// violations directly (mapped to rule ids for a readable failure). Per-state screens add their own
// axe assertions as they land (P3-P8). [Build-Session-Entscheidung: P1.35]
describe("App", () => {
  it("renders with no axe accessibility violations", async () => {
    const { container } = render(<App />);
    const results = await axe(container);
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });
});
