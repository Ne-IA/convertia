// src/hooks/useLaunchDrain.ts — the §7.8.1 root-shell-mount first-launch drain trigger (P2.61).
import { useEffect } from "react";

import { drainPendingIntake } from "../lib/ipc/events";

/**
 * [Build-Session-Entscheidung: P2.61] The §7.8.1 root-shell-mount drain trigger — on mount, re-call C1 with
 * `drainPending: true` (via `drainPendingIntake`, §5.8) to replay any launch-with-files (Open-with / argv)
 * that was buffered core-side before the WebView's `app://intake` listener existed (§7.8.1 buffer-then-replay;
 * the Rust path is P2.58/P2.60). Mounted EXACTLY ONCE (empty dep array) — a fresh `collectingId` is minted
 * per call, and `PendingIntake` is consumed once, so a duplicate fire would be a wasted no-op drain anyway.
 *
 * ORDERING (the listener race, §7.8.1): this trigger MUST run AFTER the `app://intake` listener is
 * registered. The three §5.8 `app://` listeners are wired by P2.120, which places its listener-registration
 * effect BEFORE `useLaunchDrain()` in the root shell (App.tsx). Until P2.120, no listener exists yet — the
 * drain alone replays the buffered first-launch set, which is the only launch path that can reach the WebView
 * before the app is interactive (the running-instance second-launch / Open-with paths the listener catches
 * cannot fire pre-mount).
 *
 * RESULT (§5.2): a non-empty drained `CollectedSet` drives the `Collecting` transition (P3.53's state machine
 * consumes it, exactly like a drop); an empty drain leaves the UI `Idle`. During P2 the Rust §1.1 freeze
 * seam is a shell so the result is always `Empty` → `Idle` (the trigger fires correctly; the `Collecting`
 * consumption lands with the state machine). The drain is best-effort and cannot reject under the current C1
 * handler (it returns `Ok(Empty)`); a genuine failure once C1 can fail (P3.49+) routes to the §2.13/§5.8
 * app-fault surface wired by P2.124. `void` marks the fire-and-forget trigger intent.
 */
export function useLaunchDrain(): void {
  useEffect(() => {
    void drainPendingIntake();
  }, []);
}
