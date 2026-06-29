// src/lib/ipc/events.ts — the §5.8 Channel + event-subscription helper home (§0.4.2 / §5.4).
//
// The SINGLE place the WebView wires `@tauri-apps/api` Channel / window-event APIs — the §5.1 hard rule
// "only `src/lib/ipc/**` imports `@tauri-apps/api`", the one-IPC-consumer discipline the P1.36 ESLint
// rule enforces from the first commit. It is the named home for the hand-written subscription helpers
// authored as P2 lands the §0.4.2 event contract + the §1.1 intake flow: the §5.4 native
// `onDragDropEvent` intake wiring, the §5.8 `start_conversion` progress `Channel<ConversionEvent>`
// lifecycle, the §0.4.1 C1/C2a `onScan` `Channel<ScanProgress>` telemetry, and the three §0.4.2 `app://`
// listeners — all wired by P2.120's frontend async model.
//
// P2.61 lands the FIRST hand-written helper: the §7.8.1 first-launch DRAIN (`drainPendingIntake`). The
// three `app://` event listeners + the live `Channel<ConversionEvent>` / `onScan` lifecycle are P2.120.
// [Build-Session-Entscheidung: P2.61]
import { Channel } from "@tauri-apps/api/core";

import { commands, type CollectedSet, type ScanProgress } from "./commands";

/**
 * [Build-Session-Entscheidung: P2.61] The §7.8.1 first-launch DRAIN — re-call C1 `ingest_paths` with
 * `drainPending: true` and NO paths, consuming the Rust-side `State<PendingIntake>` first-launch buffer
 * (P2.58/P2.60) exactly once. This closes the §7.8.1 listener race: a launch-with-files (Open-with / argv)
 * that arrived BEFORE the WebView registered its `app://intake` listener was buffered core-side, and is
 * replayed HERE on root-shell mount (`useLaunchDrain`, fired AFTER the listener registration — P2.120).
 *
 * The Rust handler (P2.60) marks the frontend ready + drains the buffer using its STORED origin, so:
 * - `paths: []` — a drain ignores any passed paths (`drainPending` ⊻ paths, §0.4.1 C1 mutual exclusivity);
 * - `origin: "launchArg"` is IGNORED by the drain (the buffer's stored origin wins, §7.8.1) — passed as the
 *   semantically-apt default (the typical first-launch origin), never relied on;
 * - `collectingId` is a fresh §0.4.4 ingest-cancel handle (a drain is quick, but the contract requires one);
 * - `onScan` is the required §0.4.1 `Channel<ScanProgress>` (the C1 non-optional forced deviation) — a drain
 *   has no scan progress, so it is a bare unsubscribed sink.
 *
 * Returns the §0.6 `CollectedSet`: a non-empty drain → the §5.2 `Collecting` transition (P3.53's state
 * machine consumes the result, exactly like a drop); an empty drain (the ordinary first launch with no
 * files) → `CollectedSet::Empty` and the UI stays `Idle`. During P2 the Rust §1.1 freeze seam is a shell,
 * so the result is always `Empty` until P3.49 wires the real freeze — the drain TRIGGER is this box's
 * deliverable, the result CONSUMPTION lands with the state machine.
 */
export async function drainPendingIntake(): Promise<CollectedSet> {
  const collectingId = crypto.randomUUID();
  const onScan = new Channel<ScanProgress>();
  return commands.ingestPaths([], "launchArg", collectingId, true, onScan);
}
