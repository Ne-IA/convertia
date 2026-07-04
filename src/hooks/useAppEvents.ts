// src/hooks/useAppEvents.ts — the §5.8 root-shell-mount app-event subscription (P2.120).
import { useEffect } from "react";

import { subscribeAppEvents, type AppEventHandlers } from "../lib/ipc/events";

/**
 * [Build-Session-Entscheidung: P2.120] Subscribe the three §0.4.2 `app://` events on root-shell mount
 * (§5.8 "the three app-wide events are subscribed on mount of the root shell", 05-ui-ux.md §5.8). Placed in
 * `App.tsx` BEFORE `useLaunchDrain()` so the `app://intake` listener exists before the §7.8.1 first-launch
 * drain replays a buffered set (the listener-before-drain race, P2.61).
 *
 * `subscribeAppEvents` is async (Tauri `listen` returns a Promise), so the effect tracks the resolved cleanup
 * and unlistens on unmount — dropping the listeners even when an unmount beats the subscribe (the `cancelled`
 * guard). `handlers` is stable in P2 (`App` passes none, so the effect runs once); a caller passing an inline
 * object re-subscribes on change, the correct `useEffect` dependency semantics. This hook imports only the
 * `src/lib/ipc/events` façade — never `@tauri-apps/*` directly — so the P1.36/G5 one-IPC-consumer rule holds.
 */
export function useAppEvents(handlers?: AppEventHandlers): void {
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void subscribeAppEvents(handlers).then((cleanup) => {
      if (cancelled) {
        cleanup();
      } else {
        unlisten = cleanup;
      }
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [handlers]);
}
