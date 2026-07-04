import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";

// §6.4.6 unit (G15) — §7.2.1 step 8 (P2.106.8): App is the terminal "hand to UI empty/idle state (§5.2)" step
// of the ordered startup sequence (the src-tauri `main()` spine, P2.106). After the Rust core reveals the
// window (step 6) and feeds launch intake (step 7), control passes to this React shell, which (a) renders the
// §5.2 `Idle` empty state — the `<main>` landmark — and (b) completes the readiness handshake via
// `useLaunchDrain` (C1 `drainPending` → core `mark_ready`, P2.60/P2.61). Mock the §5.8 IPC façade so the mount
// effect stays hermetic under jsdom (no Tauri runtime — the real Channel/invoke throws; the fix from the
// P1.35/ee362ce mount-side-effect note). The drain CALL contract is `lib/ipc/events.test.ts`; the hook's
// once-on-mount is `useLaunchDrain.test.tsx`; this pins the App-level STEP-8 contract (idle landmark + ready
// handshake fires). [Build-Session-Entscheidung: P2.106.8]
const drainPendingIntake = vi.fn(() => Promise.resolve({ empty: { skipped: [] } }));
vi.mock("./lib/ipc/events", () => ({
  drainPendingIntake: () => drainPendingIntake(),
}));

import { App } from "./App";

describe("App — §7.2.1 step 8 (hand to the §5.2 Idle UI)", () => {
  it("renders the §5.2 Idle `main` landmark and fires the readiness drain on mount", () => {
    const { container } = render(<App />);
    // §5.2 Idle empty-state: the `<main>` landmark boots (the step-8 handoff surface; the §5.7 reassurance copy
    // + the per-state screens land P3–P8, so the landmark is the P2 contract).
    expect(container.querySelector("main")).not.toBeNull();
    // §7.2.1 step 8 also completes the readiness handshake — the drain fires exactly once on mount (P2.60/P2.61).
    expect(drainPendingIntake).toHaveBeenCalledTimes(1);
  });
});
