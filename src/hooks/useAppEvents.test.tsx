import { StrictMode } from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";

// §6.4.6 unit (G15): the P2.120 on-mount `app://` subscription hook. Mock the §5.8 `events` façade so the hook
// runs under jsdom (no Tauri runtime) and we pin the subscribe-once + unlisten-on-unmount lifecycle plus the
// P2.137 registration-completion promise (the §7.8.1 drain gate) without a real `listen`. The listener wiring
// itself is covered by `lib/ipc/events.test.ts`. [Build-Session-Entscheidung: P2.120]
const subscribeAppEvents = vi.fn<(handlers?: unknown) => Promise<() => void>>();
vi.mock("../lib/ipc/events", () => ({
  subscribeAppEvents: (handlers?: unknown) => subscribeAppEvents(handlers),
}));

import { useAppEvents } from "./useAppEvents";

async function flushMicrotasks(): Promise<void> {
  for (let i = 0; i < 8; i += 1) {
    await Promise.resolve();
  }
}

describe("useAppEvents (P2.120 §5.8 on-mount app:// subscription)", () => {
  beforeEach(() => {
    subscribeAppEvents.mockReset();
    subscribeAppEvents.mockResolvedValue(() => {});
  });

  it("subscribes exactly once on mount", () => {
    renderHook(() => useAppEvents());
    expect(subscribeAppEvents).toHaveBeenCalledTimes(1);
  });

  it("unlistens on unmount — the resolved cleanup is dropped", async () => {
    const cleanup = vi.fn();
    subscribeAppEvents.mockResolvedValue(cleanup);
    const { unmount } = renderHook(() => useAppEvents());
    await Promise.resolve(); // resolve the async subscribe so the cleanup is captured before unmount
    unmount();
    expect(cleanup).toHaveBeenCalledTimes(1);
  });

  // P2.137: the unmount-beats-subscribe branch (the `cancelled` guard) — a LATE-resolving subscription's
  // listeners are dropped the moment they materialise, so nothing leaks past the unmounted shell.
  it("drops a late-resolving subscription's listeners when unmount beats the subscribe (the cancelled guard)", async () => {
    const cleanup = vi.fn();
    let resolveSubscribe: (c: () => void) => void = () => undefined;
    subscribeAppEvents.mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          resolveSubscribe = resolve;
        }),
    );
    const { unmount } = renderHook(() => useAppEvents());
    unmount(); // BEFORE the subscribe resolves
    expect(cleanup).not.toHaveBeenCalled();
    resolveSubscribe(cleanup);
    await flushMicrotasks();
    expect(cleanup).toHaveBeenCalledTimes(1);
  });

  // ─── P2.137: the returned registration-completion promise (the §7.8.1 drain gate) ───

  it("returns a promise that stays PENDING until the subscribe fulfils, then fulfils (§7.8.1 gate)", async () => {
    let resolveSubscribe: (c: () => void) => void = () => undefined;
    subscribeAppEvents.mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          resolveSubscribe = resolve;
        }),
    );
    const { result } = renderHook(() => useAppEvents());
    const settled = vi.fn();
    void result.current.then(settled);
    await flushMicrotasks();
    expect(settled).not.toHaveBeenCalled();
    resolveSubscribe(() => {});
    await flushMicrotasks();
    expect(settled).toHaveBeenCalledTimes(1);
  });

  it("FULFILS (never rejects) when the subscribe REJECTS — the drain gate still opens (§7.8.1)", async () => {
    // The reject leg settles the same promise: the drain's buffered set returns via the C1 command RESPONSE,
    // not via an event, so a gate that stayed pending on a failed subscribe would strand the buffer core-side.
    subscribeAppEvents.mockRejectedValue(new Error("listen failed"));
    const { result } = renderHook(() => useAppEvents());
    const fulfilled = vi.fn();
    const rejected = vi.fn();
    void result.current.then(fulfilled, rejected);
    await flushMicrotasks();
    expect(fulfilled).toHaveBeenCalledTimes(1);
    expect(rejected).not.toHaveBeenCalled();
  });

  it("returns a referentially STABLE promise across re-renders (the per-mount gate identity)", () => {
    const { result, rerender } = renderHook(() => useAppEvents());
    const first = result.current;
    rerender();
    expect(result.current).toBe(first);
  });

  // P2.137 (G1 review finding): under dev StrictMode the effect double-runs — TWO subscribe attempts share
  // the per-mount gate deferred. The FIRST (torn-down) attempt settling must NOT open the §7.8.1 drain
  // gate: its listeners are already dropped, so an early open re-creates the exact unregistered-listener
  // race the gate exists to close. Only the SURVIVING attempt's settle opens it.
  it("StrictMode double-mount: only the SURVIVING attempt's settle opens the gate", async () => {
    const resolvers: Array<(c: () => void) => void> = [];
    subscribeAppEvents.mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          resolvers.push(resolve);
        }),
    );
    const { result } = renderHook(() => useAppEvents(), { wrapper: StrictMode });
    expect(subscribeAppEvents).toHaveBeenCalledTimes(2);
    const settled = vi.fn();
    void result.current.then(settled);
    const firstCleanup = vi.fn();
    resolvers[0]?.(firstCleanup); // the cancelled first pass settles…
    await flushMicrotasks();
    expect(firstCleanup).toHaveBeenCalledTimes(1); // …its listeners are dropped…
    expect(settled).not.toHaveBeenCalled(); // …and the gate stays SHUT
    resolvers[1]?.(() => {}); // the surviving attempt settles…
    await flushMicrotasks();
    expect(settled).toHaveBeenCalledTimes(1); // …and opens the gate
  });
});
