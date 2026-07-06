// src/hooks/useLaunchDrain.ts — the §7.8.1 root-shell-mount first-launch drain trigger (P2.61).
import { useEffect, useRef } from "react";

import { drainPendingIntake } from "../lib/ipc/events";

/**
 * [Build-Session-Entscheidung: P2.61] The §7.8.1 root-shell-mount drain trigger — re-call C1 with
 * `drainPending: true` (via `drainPendingIntake`, §5.8) to replay any launch-with-files (Open-with / argv)
 * that was buffered core-side before the WebView's `app://intake` listener existed (§7.8.1 buffer-then-replay;
 * the Rust path is P2.58/P2.60). Drained EXACTLY ONCE per mount (the `drained` ref survives re-renders and a
 * changed gate identity) — a fresh `collectingId` is minted per call, and `PendingIntake` is consumed once, so
 * a duplicate fire would be a wasted no-op drain anyway.
 *
 * [Build-Session-Entscheidung: P2.137] GATING (the §7.8.1 listener race): §7.8.1 mandates the drain fire
 * "later than listener-registration, so it closes the race" (07-app-shell.md §7.8.1). Mount ORDER alone —
 * `useAppEvents()` above this hook in App.tsx — is NOT completion: the three `listen` registrations are
 * async, so a same-flush drain issues its C1 invoke first, the core flips `FrontendReady` while the
 * `app://intake` listener may not be registered, and a second launch in that window is emitted into an
 * unregistered listener and dropped. The hook therefore takes `eventsReady` — `useAppEvents`'
 * registration-completion promise — and drains only after it SETTLES. It drains on BOTH the fulfil and the
 * reject leg: the drain's buffered set returns via the C1 command RESPONSE, not via an event, so draining
 * after a failed subscribe still loses nothing — the buffer is never left stranded core-side. The `cancelled`
 * guard drops a drain whose gate settles only after unmount.
 *
 * RESULT (§5.2): a non-empty drained `CollectedSet` drives the `Collecting` transition (P3.53's state machine
 * consumes it, exactly like a drop); an empty drain leaves the UI `Idle`. During P2 the Rust §1.1 freeze
 * seam is a shell so the result is always `Empty` → `Idle` (the trigger fires correctly; the `Collecting`
 * consumption lands with the state machine). The drain is best-effort and cannot reject under the current C1
 * handler (it returns `Ok(Empty)`); a genuine failure once C1 can fail (P3.49+) routes to the §2.13/§5.8
 * app-fault surface wired by P2.124. `void` marks the fire-and-forget trigger intent.
 */
export function useLaunchDrain(eventsReady: Promise<void>): void {
  const drained = useRef(false);
  useEffect(() => {
    let cancelled = false;
    const fire = (): void => {
      if (cancelled || drained.current) {
        return;
      }
      drained.current = true;
      void drainPendingIntake();
    };
    // Both legs open the gate (see the doc comment); a settled promise fires `fire` on a microtask, so the
    // drain is ALWAYS issued after the registrations settled — never in the same synchronous flush.
    void eventsReady.then(fire, fire);
    return () => {
      cancelled = true;
    };
  }, [eventsReady]);
}
