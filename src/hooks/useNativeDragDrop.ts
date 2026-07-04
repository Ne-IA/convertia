// src/hooks/useNativeDragDrop.ts — the §5.4 native file-drop subscription on root-shell mount (P2.121).
import { useEffect } from "react";

import { subscribeNativeDragDrop, type NativeDragDropHandlers } from "../lib/ipc/events";

/**
 * [Build-Session-Entscheidung: P2.121] Subscribe the §5.4 native window `onDragDropEvent` on root-shell mount
 * — the hover affordance (`onDragActiveChange`, UNSET in P2) + `drop` → C1 `ingest_paths` (live). It is
 * INDEPENDENT of the §7.8.1 launch-drain ordering (a native drop is a live user action, never a buffered
 * launch path), so unlike `useAppEvents` it carries no before-`useLaunchDrain` constraint.
 *
 * `subscribeNativeDragDrop` is async (Tauri `onDragDropEvent` returns a Promise), so the effect tracks the
 * resolved unlisten and drops it on unmount — even when an unmount beats the subscribe (the `cancelled`
 * guard). `handlers` is stable in P2 (`App` passes none → runs once); a caller passing an inline object
 * re-subscribes on change (correct `useEffect` dependency semantics). Imports only the `src/lib/ipc/events`
 * façade — never `@tauri-apps/*` directly — so the P1.36/G5 one-IPC-consumer rule holds.
 */
export function useNativeDragDrop(handlers?: NativeDragDropHandlers): void {
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void subscribeNativeDragDrop(handlers).then((cleanup) => {
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
