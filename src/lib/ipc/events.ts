// src/lib/ipc/events.ts — the §5.8 Channel + event-subscription helper home (§0.4.2 / §5.4).
//
// The SINGLE place the WebView wires `@tauri-apps/api` Channel / window-event APIs — the §5.1
// one-IPC-consumer discipline (only `src/lib/ipc/**` imports the Tauri IPC surface: `@tauri-apps/api` +
// any `@tauri-apps/plugin-*` package), enforced by the P1.36/G5 ESLint rule from the first commit. It is the named home for the hand-written subscription helpers
// authored as P2 lands the §0.4.2 event contract + the §1.1 intake flow: the §5.4 native
// `onDragDropEvent` intake wiring, the §5.8 `start_conversion` progress `Channel<ConversionEvent>`
// lifecycle, the §0.4.1 C1/C2a `onScan` `Channel<ScanProgress>` telemetry, and the three §0.4.2 `app://`
// listeners — all wired by P2.120's frontend async model.
//
// P2.61 landed the FIRST hand-written helper: the §7.8.1 first-launch DRAIN (`drainPendingIntake`). P2.120
// lands the async model: `subscribeAppEvents` (the three §0.4.2 `app://` listeners, on-mount) + the
// `start_conversion` `Channel<ConversionEvent>` lifecycle (`startConversionRun` → the store's
// `applyConvertEvent`). The §5.4 native `onDragDropEvent` hover affordance is P2.121.
// [Build-Session-Entscheidung: P2.61]
import { Channel } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { useAppStore } from "../../state/store";
import {
  commands,
  type AppFault,
  type CollectedSet,
  type CollectedSetId,
  type ConversionEvent,
  type DestinationChoice,
  type IntakePayload,
  type OptionValues,
  type RerunDecision,
  type RunId,
  type ScanProgress,
  type TargetId,
} from "./commands";

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

// ─── P2.120: the §5.8 frontend async model (Channel<ConversionEvent> lifecycle + the three app:// listeners) ───

/**
 * [Build-Session-Entscheidung: P2.120] The §5.8 `start_conversion` progress lifecycle: create the run-scoped,
 * ordered `Channel<ConversionEvent>`, route every event into the store's `applyConvertEvent` reducer (§5.8
 * `ch.onmessage = (m) => store.applyConvertEvent(m)` — the per-item `progress` map + the
 * `pendingVideoReencodeNote` keep/clear), then fire C6 `start_conversion` (§0.4.1), which returns quickly with
 * the minted `RunId` while progress streams over the Channel (the §5.8 "respond immediately, stream the rest"
 * posture). The Channel dies with the run (§0.4.2) — a fresh one is minted per call. This wires the
 * Channel→store path; the CALLER (the §5.2 Convert transition, P3.53) invokes it once the batch/target/
 * destination are chosen (mirrors `drainPendingIntake`, whose consumption also lands with the state machine).
 */
export async function startConversionRun(
  collectedSetId: CollectedSetId,
  target: TargetId,
  options: OptionValues,
  destination: DestinationChoice,
  rerunDecision: RerunDecision,
): Promise<RunId> {
  const onProgress = new Channel<ConversionEvent>();
  onProgress.onmessage = (event) => {
    useAppStore.getState().applyConvertEvent(event);
  };
  return commands.startConversion(
    collectedSetId,
    target,
    options,
    destination,
    rerunDecision,
    onProgress,
  );
}

/**
 * [Build-Session-Entscheidung: P2.120] The `app://intake` handler (§0.4.2 / §7.8.1): the OS handed the running
 * (idle) instance new paths via a second-instance launch / Open-with, so re-enter intake by calling C1
 * `ingest_paths` with the event's `{ paths, origin }`, a fresh §0.4.4 `collectingId`, the §0.4.1 `onScan`
 * `Channel<ScanProgress>` (non-optional — a bare sink here; the "Scanning… N" display subscribes it when the
 * §5.2 Collecting screen lands, P3.53), and `drainPending: null` (a LIVE intake, distinct from the
 * first-launch `drainPendingIntake`). Returns the frozen `CollectedSet`; the §5.2 `Collecting` transition that
 * consumes it is the P3.53 machine's, exactly like a drop (mirrors `drainPendingIntake`).
 */
export async function ingestFromIntakeEvent(payload: IntakePayload): Promise<CollectedSet> {
  const collectingId = crypto.randomUUID();
  const onScan = new Channel<ScanProgress>();
  return commands.ingestPaths(payload.paths, payload.origin, collectingId, null, onScan);
}

/**
 * [Build-Session-Entscheidung: P2.120] Optional typed handlers for the two §0.4.2 `app://` events whose §5.8
 * intent is a §5.2 finite-state-machine transition that does not exist until P3.53. They are UNSET in P2.120 —
 * a §5.8-mandated on-mount registration seam with named fillers on record (typed optional props, G8-clean):
 * `app://fault`'s dispatch → FSM state 12 is the P3.53 reducer (the channel-death fault SOURCE is P2.124; the
 * `AppFaultNotice` render is the P3.60 slice → P8.19.1 copy; the `app://fault` EMIT + `PendingFault` buffer is
 * P4), and `app://close-requested`'s → QuitConfirm (state 11) body is P4.67.1. `app://intake` is NOT a prop —
 * it is handled internally (→ C1 `ingest_paths`), live from P2.120.
 */
export interface AppEventHandlers {
  /** `app://fault` (§2.13.3): an app-level fault (`AppFault`). The dispatch-to-FSM-state-12 body is P3.53. */
  readonly onFault?: (fault: AppFault) => void;
  /** `app://close-requested` (§7.3.3): the quit-while-converting confirm. QuitConfirm (state 11) body = P4.67.1. */
  readonly onCloseRequested?: () => void;
}

/**
 * [Build-Session-Entscheidung: P2.120] Subscribe the THREE §0.4.2 `app://` events on mount (§5.8 "the three
 * app-wide events are subscribed on mount of the root shell", 05-ui-ux.md §5.8) — the ONLY `app.emit`/`listen`
 * events (the closed set G23 + plan-lint check 28 assert). Returns a single cleanup dropping all three
 * listeners. `useAppEvents` calls this BEFORE `useLaunchDrain` so a buffered first-launch set (§7.8.1) replays
 * only after the `app://intake` listener exists (the listener-before-drain race, P2.61):
 * - `app://intake` → C1 `ingest_paths` (`ingestFromIntakeEvent`), live from P2.120 — it ingests
 *   unconditionally here; the §5.8 defence-in-depth "ignore-unless-Idle/Summary + BusyNotice" guard on a
 *   leaked mid-run event is the P3.53 FSM's (only `idle` exists in P2, and the §7.1 single-instance callback
 *   is the authoritative primary refuse-busy gate);
 * - `app://fault` → `handlers.onFault?.(payload)` (UNSET in P2 — see {@link AppEventHandlers});
 * - `app://close-requested` → `handlers.onCloseRequested?.()` (unit payload, no DTO; UNSET in P2).
 */
export async function subscribeAppEvents(handlers: AppEventHandlers = {}): Promise<() => void> {
  const unlisteners = await Promise.all([
    listen<IntakePayload>("app://intake", (event) => {
      void ingestFromIntakeEvent(event.payload);
    }),
    listen<AppFault>("app://fault", (event) => {
      handlers.onFault?.(event.payload);
    }),
    listen("app://close-requested", () => {
      handlers.onCloseRequested?.();
    }),
  ]);
  return () => {
    for (const unlisten of unlisteners) {
      unlisten();
    }
  };
}
