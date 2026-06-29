import { describe, it, expect, vi, beforeEach } from "vitest";

// §6.4.6 unit (G15): the §7.8.1 first-launch drain helper (P2.61). Mock the §0.4.5 IPC transport — the
// generated `bindings.ts` (re-exported by ./commands) calls `invoke` from @tauri-apps/api/core, and the
// drain constructs a `Channel` — so the C1 `ingest_paths` wrapper runs with no Tauri runtime and we read
// back the EXACT arguments the drain sends. [Build-Session-Entscheidung: P2.61]
const invoke = vi.fn<(cmd: string, args: Record<string, unknown>) => Promise<unknown>>();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args: Record<string, unknown>) => invoke(cmd, args),
  Channel: class {},
}));

import { drainPendingIntake } from "./events";

describe("drainPendingIntake (§7.8.1 first-launch drain)", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue({ empty: { skipped: [] } });
  });

  it("re-calls C1 ingest_paths with no paths + drainPending true + a fresh collectingId", async () => {
    await drainPendingIntake();
    expect(invoke).toHaveBeenCalledTimes(1);
    // §7.8.1 / §0.4.1: a drain sends empty paths (drainPending ⊻ paths) + drainPending=true; the origin is
    // ignored by the Rust drain (stored origin wins) but passed as the launchArg placeholder.
    expect(invoke).toHaveBeenCalledWith(
      "ingest_paths",
      expect.objectContaining({
        paths: [],
        origin: "launchArg",
        drainPending: true,
        collectingId: expect.any(String),
      }),
    );
  });

  it("mints a fresh collectingId per drain (no reuse across calls)", async () => {
    await drainPendingIntake();
    await drainPendingIntake();
    const ids = invoke.mock.calls.map((call) => call[1].collectingId);
    expect(new Set(ids).size).toBe(2);
  });
});
