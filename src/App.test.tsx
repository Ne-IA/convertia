import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "@testing-library/react";

// §6.4.6 unit (G15) — §7.2.1 step 8 (P2.106.8): App is the terminal "hand to UI empty/idle state (§5.2)" step
// of the ordered startup sequence (the src-tauri `main()` spine, P2.106). After the Rust core reveals the
// window (step 6) and feeds launch intake (step 7), control passes to this React shell, which (a) renders the
// §5.2 `Idle` empty state — the `<main>` landmark — and (b) completes the readiness handshake via
// `useLaunchDrain` (C1 `drainPending` → core `mark_ready`, P2.60/P2.61). Mock the §5.8 IPC façade so the mount
// effect stays hermetic under jsdom (no Tauri runtime — the real Channel/invoke throws; the fix from the
// P1.35/ee362ce mount-side-effect note). The drain CALL contract is `lib/ipc/events.test.ts`; the hook's
// gate/once semantics are `useLaunchDrain.test.tsx`; this pins the App-level STEP-8 contract (idle landmark +
// ready handshake fires, gated on listener-registration COMPLETION). [Build-Session-Entscheidung: P2.106.8]
const drainPendingIntake = vi.fn<() => Promise<unknown>>();
const subscribeAppEvents = vi.fn<() => Promise<() => void>>();
const subscribeNativeDragDrop = vi.fn<() => Promise<() => void>>();
vi.mock("./lib/ipc/events", () => ({
  drainPendingIntake: () => drainPendingIntake(),
  subscribeAppEvents: () => subscribeAppEvents(),
  subscribeNativeDragDrop: () => subscribeNativeDragDrop(),
}));

import { App } from "./App";

// Drain enough microtask turns that a settled subscribe propagates through useAppEvents' ready deferred into
// useLaunchDrain's gate and the drain invoke lands (three chained `.then` hops; eight turns is safe margin).
async function flushMicrotasks(): Promise<void> {
  for (let i = 0; i < 8; i += 1) {
    await Promise.resolve();
  }
}

describe("App — §7.2.1 step 8 (hand to the §5.2 Idle UI)", () => {
  beforeEach(() => {
    drainPendingIntake.mockReset();
    subscribeAppEvents.mockReset();
    subscribeNativeDragDrop.mockReset();
    drainPendingIntake.mockResolvedValue({ empty: { skipped: [] } });
    subscribeAppEvents.mockResolvedValue(() => {});
    subscribeNativeDragDrop.mockResolvedValue(() => {});
  });

  it("renders the §5.2 Idle `main` landmark and fires all three §5.4/§5.8 mount effects", async () => {
    const { container } = render(<App />);
    // §5.2 Idle empty-state: the `<main>` landmark boots (the step-8 handoff surface; the §5.7 reassurance copy
    // + the per-state screens land P3–P8, so the landmark is the P2 contract).
    expect(container.querySelector("main")).not.toBeNull();
    // §5.4/§5.8: App subscribes the three `app://` listeners (P2.120) + the native file-drop (P2.121) directly
    // on mount, and — once the subscription settles — fires the §7.8.1 readiness drain (P2.60/P2.61), each
    // exactly once.
    expect(subscribeAppEvents).toHaveBeenCalledTimes(1);
    expect(subscribeNativeDragDrop).toHaveBeenCalledTimes(1);
    await flushMicrotasks();
    expect(drainPendingIntake).toHaveBeenCalledTimes(1);
  });

  // [Test-Change: P2.137 — old-obsolete+new-correct, §7.8.1] The former invocation-ORDER pin
  // (subscribeAppEvents called before drainPendingIntake in the same synchronous flush) is obsolete: §7.8.1
  // mandates the drain fire "later than listener-registration, so it closes the race" (07-app-shell.md
  // §7.8.1), and mount order alone still let the drain's C1 invoke overtake the three PENDING async `listen`
  // registrations — the core flipped `FrontendReady` while the WebView listeners may not exist, so a second
  // launch in that window was emitted into an unregistered listener and dropped. The correct pin is
  // COMPLETION granularity: no drain while the subscribe is pending; exactly one once it settles.
  it("gates the §7.8.1 drain on listener-registration COMPLETION — no drain while the subscribe is pending", async () => {
    let resolveSubscribe: (cleanup: () => void) => void = () => undefined;
    subscribeAppEvents.mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          resolveSubscribe = resolve;
        }),
    );
    render(<App />);
    await flushMicrotasks();
    // The three `listen` registrations are still PENDING: the drain C1 invoke must not have been issued
    // (this is exactly the §7.8.1 unregistered-listener window the gate closes).
    expect(subscribeAppEvents).toHaveBeenCalledTimes(1);
    expect(drainPendingIntake).not.toHaveBeenCalled();
    resolveSubscribe(() => {});
    await flushMicrotasks();
    expect(drainPendingIntake).toHaveBeenCalledTimes(1);
  });

  it("still drains exactly once when the subscribe REJECTS — the buffered set is never stranded", async () => {
    // The drain's buffered set returns via the C1 command RESPONSE, not via an event, so draining after a
    // failed subscribe still loses nothing (§7.8.1) — the reject leg opens the same gate, exactly once.
    subscribeAppEvents.mockRejectedValue(new Error("listen failed"));
    render(<App />);
    await flushMicrotasks();
    expect(drainPendingIntake).toHaveBeenCalledTimes(1);
    // [Test-Change: P2.137 — old-obsolete+new-correct, §7.8.1] the synchronous invocation-ORDER pin that
    // closed this file is superseded by the two COMPLETION-granularity tests above (order alone never
    // closed the §7.8.1 race — see the tag block above the gating test).
  });
});
