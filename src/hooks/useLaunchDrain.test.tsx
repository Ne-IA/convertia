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
// [Test-Change: P3.55 — old-obsolete+new-correct, §5.8] The mount trigger now calls the CONSUMING
// `consumeMountDrain` (drain + route the `CollectedSet` into the §5.2 machine) rather than the bare
// `drainPendingIntake`; the gate/once/cancelled semantics under test are unchanged — only the mocked façade
// name is (the mount consumes now). The consumption itself is covered in `events.test.ts`.
const consumeMountDrain = vi.fn<() => Promise<void>>();
vi.mock("../lib/ipc/events", () => ({
  consumeMountDrain: () => consumeMountDrain(),
}));

import { useLaunchDrain } from "./useLaunchDrain";

async function flushMicrotasks(): Promise<void> {
  for (let i = 0; i < 8; i += 1) {
    await Promise.resolve();
  }
}

describe("useLaunchDrain (§7.8.1 completion-gated first-launch drain trigger)", () => {
  beforeEach(() => {
    consumeMountDrain.mockReset();
    consumeMountDrain.mockResolvedValue(undefined);
  });

  // [Test-Change: P3.55 — old-obsolete+new-correct, §5.8] Each `expect(drainPendingIntake)` below became
  // `expect(consumeMountDrain)` — the mount trigger now calls the CONSUMING façade (drain + route into the §5.2
  // machine), not the bare drain. The gate/once/cancelled semantics are unchanged; the tags mark the rename.
  it("does NOT drain while the gate promise is pending (the §7.8.1 unregistered-listener window)", async () => {
    const gate = new Promise<void>(() => undefined); // never settles
    renderHook(() => {
      useLaunchDrain(gate);
    });
    await flushMicrotasks();
    // [Test-Change: P3.55 — old-obsolete+new-correct, §5.8]
    expect(consumeMountDrain).not.toHaveBeenCalled();
  });

  it("fires the drain exactly once after the gate FULFILS", async () => {
    let resolveGate: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => {
      resolveGate = resolve;
    });
    renderHook(() => {
      useLaunchDrain(gate);
    });
    // [Test-Change: P3.55 — old-obsolete+new-correct, §5.8]
    expect(consumeMountDrain).not.toHaveBeenCalled();
    resolveGate();
    await flushMicrotasks();
    expect(consumeMountDrain).toHaveBeenCalledTimes(1);
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
    // [Test-Change: P3.55 — old-obsolete+new-correct, §5.8]
    expect(consumeMountDrain).toHaveBeenCalledTimes(1);
  });

  it("does not re-fire on re-render (drained once per mount)", async () => {
    const gate = Promise.resolve();
    const { rerender } = renderHook(() => {
      useLaunchDrain(gate);
    });
    await flushMicrotasks();
    rerender();
    await flushMicrotasks();
    // [Test-Change: P3.55 — old-obsolete+new-correct, §5.8]
    expect(consumeMountDrain).toHaveBeenCalledTimes(1);
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
    // [Test-Change: P3.55 — old-obsolete+new-correct, §5.8]
    expect(consumeMountDrain).toHaveBeenCalledTimes(1);
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
    // [Test-Change: P3.55 — old-obsolete+new-correct, §5.8]
    expect(consumeMountDrain).not.toHaveBeenCalled();
  });
});
