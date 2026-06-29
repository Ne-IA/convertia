import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";
import { axe } from "vitest-axe";

// P2.61: App now fires the §7.8.1 first-launch drain on mount (`useLaunchDrain`). Mock the §5.8 IPC helper so
// this a11y render stays hermetic — jsdom has no Tauri runtime, so the real `Channel`/`invoke` throws. The
// drain CALL is covered by `lib/ipc/events.test.ts`; here App just renders + the mount effect runs harmlessly.
vi.mock("./lib/ipc/events", () => ({
  drainPendingIntake: () => Promise.resolve({ empty: { skipped: [] } }),
}));

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
