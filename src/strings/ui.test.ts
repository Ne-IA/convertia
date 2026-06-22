import { describe, it, expect } from "vitest";

import { ui } from "./ui";

// Section 5.7 / 6.10 Principle 11 by construction: the UI-chrome strings are a flat English table
// consumed DIRECTLY -- there is no i18n runtime, no locale negotiation, no per-locale message
// catalogue. The dependency-/import-level i18n ban is enforced by G57 (check-english-only's
// package.json + src import scans and the eslint no-restricted-imports rule, both armed); this test
// locks in the "consumed directly as a flat string map" half at the unit level. [Build-Session-Entscheidung: P1.38]
describe("Principle 11 -- English-only UI, no i18n runtime", () => {
  it("consumes ui as a flat string map by direct key, not a locale-aware lookup", () => {
    // A plain object -- NOT a t()/i18n() lookup function and NOT a locale-keyed catalogue.
    expect(typeof ui).toBe("object");
    const entries = Object.entries(ui);
    expect(entries.length).toBeGreaterThan(0);
    for (const [key, value] of entries) {
      // Each value is the final English string, reached by a plain key with no locale parameter.
      expect(typeof value, `${key} must be a string`).toBe("string");
      expect(value.trim(), `${key} must be non-empty`).not.toBe("");
    }
  });
});
