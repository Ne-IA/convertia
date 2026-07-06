import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";

// §6.4.6 unit (G15): the §7.8.1 root-shell-mount drain trigger (P2.61). Mock the §5.8 façade helper so the
// hook is tested in isolation from the IPC transport — the drain CALL contract is covered by
// `events.test.ts`; here we pin the P2.137 completion gate (drain only after the passed registration promise
// SETTLES, on both legs), the drained-once-per-mount semantics, and the cancelled guard.
// [Build-Session-Entscheidung: P2.61]
//
// [Test-Change: P2.137 — old-obsolete+new-correct, §7.8.1] The former fires-synchronously-on-mount
// expectation is obsolete: §7.8.1 mandates the drain fire "later than listener-registration, so it closes
// the race" (07-app-shell.md §7.8.1), so the hook signature gained the `eventsReady` gate and the drain is
// asserted AFTER the gate settles — never in the mount's synchronous flush.
const drainPendingIntake = vi.fn<() => Promise<unknown>>();
vi.mock("../lib/ipc/events", () => ({
  drainPendingIntake: () => drainPendingIntake(),
}));

import { useLaunchDrain } from "./useLaunchDrain";

async function flushMicrotasks(): Promise<void> {
  for (let i = 0; i < 8; i += 1) {
    await Promise.resolve();
  }
}

describe("useLaunchDrain (§7.8.1 completion-gated first-launch drain trigger)", () => {
  beforeEach(() => {
    drainPendingIntake.mockReset();
    drainPendingIntake.mockResolvedValue({ empty: { skipped: [] } });
  });

  it("does NOT drain while the gate promise is pending (the §7.8.1 unregistered-listener window)", async () => {
    const gate = new Promise<void>(() => undefined); // never settles
    renderHook(() => {
      useLaunchDrain(gate);
    });
    await flushMicrotasks();
    expect(drainPendingIntake).not.toHaveBeenCalled();
  });

  it("fires the drain exactly once after the gate FULFILS", async () => {
    let resolveGate: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => {
      resolveGate = resolve;
    });
    renderHook(() => {
      useLaunchDrain(gate);
    });
    expect(drainPendingIntake).not.toHaveBeenCalled();
    resolveGate();
    await flushMicrotasks();
    expect(drainPendingIntake).toHaveBeenCalledTimes(1);
  });

  it("fires the drain on the gate's REJECT leg too — the buffered set is never stranded (§7.8.1)", async () => {
    // The drain's buffered set returns via the C1 command RESPONSE, not via an event, so draining after a
    // failed listener registration still loses nothing — both legs open the gate.
    let rejectGate: (reason: unknown) => void = () => undefined;
    const gate = new Promise<void>((_resolve, reject) => {
      rejectGate = reject;
    });
    renderHook(() => {
      useLaunchDrain(gate);
    });
    rejectGate(new Error("listen failed"));
    await flushMicrotasks();
    expect(drainPendingIntake).toHaveBeenCalledTimes(1);
  });

  it("does not re-fire on re-render (drained once per mount)", async () => {
    const gate = Promise.resolve();
    const { rerender } = renderHook(() => {
      useLaunchDrain(gate);
    });
    await flushMicrotasks();
    rerender();
    await flushMicrotasks();
    expect(drainPendingIntake).toHaveBeenCalledTimes(1);
  });

  it("does not re-fire when a NEW gate identity arrives on re-render (the drained ref)", async () => {
    // A changed `eventsReady` identity re-runs the effect (correct dependency semantics), but PendingIntake
    // is consumed once per mount — the `drained` ref blocks the duplicate.
    const { rerender } = renderHook(
      ({ gate }: { gate: Promise<void> }) => {
        useLaunchDrain(gate);
      },
      { initialProps: { gate: Promise.resolve() } },
    );
    await flushMicrotasks();
    rerender({ gate: Promise.resolve() });
    await flushMicrotasks();
    expect(drainPendingIntake).toHaveBeenCalledTimes(1);
  });

  it("does not drain when unmount beats the gate settle (the cancelled guard)", async () => {
    let resolveGate: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => {
      resolveGate = resolve;
    });
    const { unmount } = renderHook(() => {
      useLaunchDrain(gate);
    });
    unmount(); // BEFORE the gate settles
    resolveGate();
    await flushMicrotasks();
    expect(drainPendingIntake).not.toHaveBeenCalled();
  });
});
