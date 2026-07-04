import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";

// §6.4.6 unit (G15): the P2.120 on-mount `app://` subscription hook. Mock the §5.8 `events` façade so the hook
// runs under jsdom (no Tauri runtime) and we assert the subscribe-once + unlisten-on-unmount lifecycle without
// a real `listen`. The listener wiring itself is covered by `lib/ipc/events.test.ts`.
// [Build-Session-Entscheidung: P2.120]
const subscribeAppEvents = vi.fn<(handlers?: unknown) => Promise<() => void>>();
vi.mock("../lib/ipc/events", () => ({
  subscribeAppEvents: (handlers?: unknown) => subscribeAppEvents(handlers),
}));

import { useAppEvents } from "./useAppEvents";

describe("useAppEvents (P2.120 §5.8 on-mount app:// subscription)", () => {
  beforeEach(() => {
    subscribeAppEvents.mockReset();
    subscribeAppEvents.mockResolvedValue(() => {});
  });

  it("subscribes exactly once on mount", () => {
    renderHook(() => {
      useAppEvents();
    });
    expect(subscribeAppEvents).toHaveBeenCalledTimes(1);
  });

  it("unlistens on unmount — the resolved cleanup is dropped", async () => {
    const cleanup = vi.fn();
    subscribeAppEvents.mockResolvedValue(cleanup);
    const { unmount } = renderHook(() => {
      useAppEvents();
    });
    await Promise.resolve(); // resolve the async subscribe so the cleanup is captured before unmount
    unmount();
    expect(cleanup).toHaveBeenCalledTimes(1);
  });
});
