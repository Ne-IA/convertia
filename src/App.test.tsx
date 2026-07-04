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
const subscribeAppEvents = vi.fn(() => Promise.resolve(() => {}));
const subscribeNativeDragDrop = vi.fn(() => Promise.resolve(() => {}));
vi.mock("./lib/ipc/events", () => ({
  drainPendingIntake: () => drainPendingIntake(),
  subscribeAppEvents: () => subscribeAppEvents(),
  subscribeNativeDragDrop: () => subscribeNativeDragDrop(),
}));

import { App } from "./App";

describe("App — §7.2.1 step 8 (hand to the §5.2 Idle UI)", () => {
  it("renders the §5.2 Idle `main` landmark and fires all three §5.4/§5.8 mount effects", () => {
    const { container } = render(<App />);
    // §5.2 Idle empty-state: the `<main>` landmark boots (the step-8 handoff surface; the §5.7 reassurance copy
    // + the per-state screens land P3–P8, so the landmark is the P2 contract).
    expect(container.querySelector("main")).not.toBeNull();
    // §5.4/§5.8: App subscribes the three `app://` listeners (P2.120) + the native file-drop (P2.121), and
    // fires the §7.8.1 readiness drain (P2.60/P2.61) — each exactly once on mount.
    expect(subscribeAppEvents).toHaveBeenCalledTimes(1);
    expect(subscribeNativeDragDrop).toHaveBeenCalledTimes(1);
    expect(drainPendingIntake).toHaveBeenCalledTimes(1);
    // …and in THAT order: the listener MUST register before the drain (the §7.8.1 listener-before-drain race,
    // P2.61). Pin the RELATIVE call order so a future swap of the two mount hooks in App.tsx reddens this test.
    // Both fired exactly once (asserted above), so the recorded orders are real (the `?? 0` never triggers).
    const subscribeOrder = subscribeAppEvents.mock.invocationCallOrder[0] ?? 0;
    const drainOrder = drainPendingIntake.mock.invocationCallOrder[0] ?? 0;
    expect(subscribeOrder).toBeLessThan(drainOrder);
  });
});
