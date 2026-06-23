import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { axe } from "vitest-axe";

// G33a self-test (P1.62.5): the armed-canary that proves the §6.4.6a vitest-axe jsdom leg is
// ENFORCING, not vacuously green. A deliberately-INVALID ARIA role is rendered; axe MUST report >= 1
// violation -- so a real ARIA/role regression in a product component can never slip through a
// silently-disarmed leg (the jsdom-axe analogue of a gate's planted-positive G24 self-test). It runs
// in the dedicated G33a leg (vitest.a11y.config.ts / `pnpm test:a11y`), so the same vitest-axe
// machinery that enforces also proves it detects, every run. The companion POSITIVE assertion -- the
// real App tree has ZERO violations -- lives in App.a11y.test.tsx. The matcher is asserted on the
// axe() result directly (not toHaveNoViolations), the same way App.a11y.test.tsx does, because
// vitest-axe 0.1.0 re-exports the matcher type-only under verbatimModuleSyntax.
// [Build-Session-Entscheidung: P1.62.5]
describe("G33a armed-canary", () => {
  it("axe DETECTS a planted invalid ARIA role (the jsdom-axe leg is armed)", async () => {
    // role="not-a-valid-role" is not a defined ARIA role -> axe-core's `aria-roles` rule flags it.
    // No child text (it would trip react/jsx-no-literals, and axe flags the invalid role regardless
    // of content); a stable id keeps the node well-formed so ONLY the planted role is the violation.
    const { container } = render(<div id="g33a-planted" role="not-a-valid-role" />);
    const results = await axe(container);
    expect(results.violations.length).toBeGreaterThan(0);
  });
});
