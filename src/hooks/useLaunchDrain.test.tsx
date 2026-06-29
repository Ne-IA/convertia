import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";

// §6.4.6 unit (G15): the §7.8.1 root-shell-mount drain trigger (P2.61). Mock the §5.8 façade helper so the
// hook is tested in isolation from the IPC transport — the drain CALL contract is covered by
// `events.test.ts`; here we pin that the trigger FIRES exactly once on mount and never re-fires on re-render
// (the empty-deps mount-once contract closing the §7.8.1 listener race). [Build-Session-Entscheidung: P2.61]
const drainPendingIntake = vi.fn<() => Promise<unknown>>();
vi.mock("../lib/ipc/events", () => ({
  drainPendingIntake: () => drainPendingIntake(),
}));

import { useLaunchDrain } from "./useLaunchDrain";

describe("useLaunchDrain (§7.8.1 root-shell-mount drain trigger)", () => {
  beforeEach(() => {
    drainPendingIntake.mockReset();
    drainPendingIntake.mockResolvedValue({ empty: { skipped: [] } });
  });

  it("fires the first-launch drain exactly once on mount", () => {
    renderHook(() => useLaunchDrain());
    expect(drainPendingIntake).toHaveBeenCalledTimes(1);
  });

  it("does not re-fire on re-render (mount-once, empty deps)", () => {
    const { rerender } = renderHook(() => useLaunchDrain());
    rerender();
    expect(drainPendingIntake).toHaveBeenCalledTimes(1);
  });
});
