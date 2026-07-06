// src/hooks/useAppEvents.ts — the §5.8 root-shell-mount app-event subscription (P2.120).
import { useEffect, useRef } from "react";

import { subscribeAppEvents, type AppEventHandlers } from "../lib/ipc/events";

/** The per-mount deferred behind {@link useAppEvents}' returned registration-completion promise. */
interface ReadyDeferred {
  readonly promise: Promise<void>;
  readonly resolve: () => void;
}

function createReadyDeferred(): ReadyDeferred {
  let resolve: () => void = () => undefined;
  const promise = new Promise<void>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

/**
 * [Build-Session-Entscheidung: P2.120] Subscribe the three §0.4.2 `app://` events on root-shell mount
 * (§5.8 "the three app-wide events are subscribed on mount of the root shell", 05-ui-ux.md §5.8).
 *
 * [Build-Session-Entscheidung: P2.137] Returns the registration-COMPLETION promise: a per-mount-stable
 * `Promise<void>` that FULFILS once the first subscribe attempt SETTLES — on BOTH the fulfil and the reject
 * leg (it never rejects itself, so an unconsumed return can never become an unhandled rejection). §7.8.1
 * mandates the first-launch drain fire "later than listener-registration, so it closes the race"
 * (07-app-shell.md §7.8.1) — and mount ORDER alone is not that: `subscribeAppEvents` is async (Tauri `listen`
 * returns a Promise), so a drain fired in the same synchronous effect flush issues its C1 invoke BEFORE the
 * three `listen` registrations resolve, the core flips `FrontendReady` while the WebView listeners may not be
 * registered, and a second launch in that window is emitted into an unregistered listener and dropped.
 * `useLaunchDrain` therefore gates on THIS promise (App.tsx wires the two). Settling on the reject leg is
 * deliberate: the drain's buffered set returns via the C1 command RESPONSE, not via an event, so draining
 * after a failed subscribe still loses nothing — the buffer must never be left stranded core-side.
 *
 * `subscribeAppEvents` is async, so the effect tracks the resolved cleanup and unlistens on unmount —
 * dropping the listeners even when an unmount beats the subscribe (the `cancelled` guard). `handlers` is
 * stable in P2 (`App` passes none, so the effect runs once); a caller passing an inline object re-subscribes
 * on change, the correct `useEffect` dependency semantics — the returned promise stays the FIRST attempt's
 * (a promise settles once; re-resolving is a no-op). This hook imports only the `src/lib/ipc/events` façade —
 * never `@tauri-apps/*` directly — so the P1.36/G5 one-IPC-consumer rule holds.
 */
export function useAppEvents(handlers?: AppEventHandlers): Promise<void> {
  // Lazy ref init: one deferred per mount, created on first render and stable across re-renders — the
  // identity the drain gate depends on. [Build-Session-Entscheidung: P2.137]
  const readyRef = useRef<ReadyDeferred | null>(null);
  readyRef.current ??= createReadyDeferred();
  const ready = readyRef.current;
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void subscribeAppEvents(handlers).then(
      (cleanup) => {
        if (cancelled) {
          // A cancelled attempt (unmount / the dev-StrictMode first pass) drops its listeners AND must
          // NOT open the §7.8.1 drain gate: StrictMode double-mounts share this per-mount deferred, and
          // the first (torn-down) attempt settling early would re-open the exact unregistered-listener
          // race the gate exists to close — only the SURVIVING attempt's settle opens it.
          // [Build-Session-Entscheidung: P2.137]
          cleanup();
          return;
        }
        unlisten = cleanup;
        ready.resolve();
      },
      () => {
        if (cancelled) {
          // A cancelled attempt's failure is equally not the surviving attempt's signal (see above).
          return;
        }
        // Registration failed — still mark the attempt SETTLED so the §7.8.1 drain gate opens (see the
        // doc comment: the drained set returns via the C1 response, so nothing is lost).
        ready.resolve();
      },
    );
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [handlers, ready]);
  return ready.promise;
}
