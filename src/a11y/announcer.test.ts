import { describe, it, expect } from "vitest";

import { announce } from "./announcer";

// Section 5.6: announce() writes a message into a visually-hidden ARIA-live region of the requested
// priority, so assistive tech reads it. (WHEN each component announces is the P4/P8 wiring; this
// exercises only the helper's live-region mechanism.)
describe("announce (section 5.6 ARIA-live helper)", () => {
  it("writes the message into a polite ARIA-live region by default", () => {
    announce("polite announcement");
    const region = document.querySelector('[aria-live="polite"]');
    expect(region).not.toBeNull();
    expect(region?.getAttribute("role")).toBe("status");
    expect(region?.textContent).toBe("polite announcement");
  });

  it("uses an assertive ARIA-live region for assertive priority", () => {
    announce("assertive announcement", "assertive");
    const region = document.querySelector('[aria-live="assertive"]');
    expect(region).not.toBeNull();
    expect(region?.getAttribute("role")).toBe("alert");
    expect(region?.textContent).toBe("assertive announcement");
  });

  it("reuses the single region per priority rather than stacking nodes", () => {
    announce("first");
    announce("second");
    expect(document.querySelectorAll('[aria-live="polite"]')).toHaveLength(1);
    expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("second");
  });
});
