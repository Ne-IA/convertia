import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";

// §6.4.6 unit (G15): the P2.121 on-mount native-file-drop subscription hook. Mock the §5.4 `events` façade so
// the hook runs under jsdom (no Tauri runtime) and we assert the subscribe-once + unlisten-on-unmount
// lifecycle without a real `onDragDropEvent`. The drop wiring itself is covered by `lib/ipc/events.test.ts`.
// [Build-Session-Entscheidung: P2.121]
const subscribeNativeDragDrop = vi.fn<(handlers?: unknown) => Promise<() => void>>();
vi.mock("../lib/ipc/events", () => ({
  subscribeNativeDragDrop: (handlers?: unknown) => subscribeNativeDragDrop(handlers),
}));

import { useNativeDragDrop } from "./useNativeDragDrop";

describe("useNativeDragDrop (P2.121 §5.4 on-mount native-drop subscription)", () => {
  beforeEach(() => {
    subscribeNativeDragDrop.mockReset();
    subscribeNativeDragDrop.mockResolvedValue(() => {});
  });

  it("subscribes exactly once on mount", () => {
    renderHook(() => {
      useNativeDragDrop();
    });
    expect(subscribeNativeDragDrop).toHaveBeenCalledTimes(1);
  });

  it("unlistens on unmount — the resolved cleanup is dropped", async () => {
    const cleanup = vi.fn();
    subscribeNativeDragDrop.mockResolvedValue(cleanup);
    const { unmount } = renderHook(() => {
      useNativeDragDrop();
    });
    await Promise.resolve(); // resolve the async subscribe so the cleanup is captured before unmount
    unmount();
    expect(cleanup).toHaveBeenCalledTimes(1);
  });
});
