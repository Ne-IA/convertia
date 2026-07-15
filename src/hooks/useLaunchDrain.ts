// src/hooks/useLaunchDrain.ts — the §7.8.1 root-shell-mount first-launch drain trigger (P2.61).
import { useEffect, useRef } from "react";

import { consumeMountDrain } from "../lib/ipc/events";

/**
 * [Build-Session-Entscheidung: P2.61] The §7.8.1 root-shell-mount drain trigger — call C1 `drain_intake`
 * (via `drainPendingIntake`, §5.8; P3.78 — every call drains) to replay any launch-with-files (Open-with / argv)
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
 * RESULT (§5.2): [Build-Session-Entscheidung: P3.55] the drain now CONSUMES — `consumeMountDrain` (events.ts)
 * drains once and routes the `CollectedSet` into the §5.2 machine FROM Idle (the launch-vs-nudge asymmetry: a
 * plain-launch `Empty` STAYS Idle via the machine's `emptyStaysIdle=true` arm; a launch-with-files set advances
 * exactly like a drop). Through P3.78 the Rust §1.1 freeze seam is a shell so the result is `Empty` → `Idle`;
 * P3.49 wires the real freeze so a launch-with-files reaches `Confirm`. The drain is best-effort; a genuine C1
 * failure re-throws to the §7.5.1 global frontend-error bridge (like every intake trigger). `void` marks the
 * fire-and-forget trigger intent.
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
      void consumeMountDrain();
    };
    // Both legs open the gate (see the doc comment); a settled promise fires `fire` on a microtask, so the
    // drain is ALWAYS issued after the registrations settled — never in the same synchronous flush.
    void eventsReady.then(fire, fire);
    return () => {
      cancelled = true;
    };
  }, [eventsReady]);
}
