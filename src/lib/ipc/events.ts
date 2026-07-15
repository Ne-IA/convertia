// src/lib/ipc/events.ts — the §5.8 Channel + event-subscription helper home (§0.4.2 / §5.4).
//
// The SINGLE place the WebView wires `@tauri-apps/api` Channel / window-event APIs — the §5.1
// one-IPC-consumer discipline (only `src/lib/ipc/**` imports the Tauri IPC surface: `@tauri-apps/api` +
// any `@tauri-apps/plugin-*` package), enforced by the P1.36/G5 ESLint rule from the first commit. It is the named home for the hand-written subscription helpers
// authored as P2 lands the §0.4.2 event contract + the §1.1 intake flow: the §5.4 native
// `onDragDropEvent` intake wiring, the §5.8 `start_conversion` progress `Channel<ConversionEvent>`
// lifecycle, the §0.4.1 C1/C2a `onScan` `Channel<ScanProgress>` telemetry, and the three §0.4.2 `app://`
// listeners — wired incrementally as P2 lands each (see P2.61 / P2.120 / P2.121 below).
//
// P2.61 landed the FIRST hand-written helper: the §7.8.1 DRAIN (`drainPendingIntake`). P2.120 landed the async
// model: `subscribeAppEvents` (the three §0.4.2 `app://` listeners) + the `start_conversion`
// `Channel<ConversionEvent>` lifecycle (`startConversionRun`). P2.121 lands `subscribeNativeDragDrop` — the
// §5.4 native `onDragDropEvent` hover affordance. P2.124 adds the §5.8/§2.13.3 backend-disconnect fault seam
// (`ConversionRunHandlers.onRunFault`) on the `startConversionRun` lifecycle — a mid-run app-level fault routes
// to AppFault (state 12), never a per-item outcome.
// [Build-Session-Entscheidung: P3.77] The 2026-07-06 core-owned-path ruling moved the native DROP core-side
// (`WindowEvent::DragDrop` → the §7.8.1 funnel) and made `app://intake` a PAYLOAD-LESS nudge: the WebView no
// longer ingests a drop or carries intake paths — the `app://intake` listener + the mount both issue the C1
// DRAIN (`drainPendingIntake`), and `subscribeNativeDragDrop` keeps only the drag-active affordance (the
// retired `ingestFromDrop` / `ingestFromIntakeEvent` are tombstoned below). P3.81 is the post-screens
// consolidation/verification box (re-ordered after the P3.53-P3.60 screens by its 2026-07-12 ruling).
// [Build-Session-Entscheidung: P2.61]
import { Channel } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { useAppStore } from "../../state/store";
import {
  commands,
  type AppFault,
  type CollectedSet,
  type CollectedSetId,
  type ConversionEvent,
  type DestinationChoice,
  type OptionValues,
  type PickKind,
  type RerunDecision,
  type RunId,
  type ScanProgress,
  type TargetId,
} from "./commands";

/**
 * [Build-Session-Entscheidung: P2.61] The §7.8.1 DRAIN — call C1 `drain_intake`, consuming the Rust-side
 * `State<PendingIntake>` hand-off buffer (P2.58/P2.60) exactly once.
 * [Build-Session-Entscheidung: P3.77] After the 2026-07-06 core-owned-path ruling this is the drain BOTH intake
 * triggers issue (§7.8.1 "the frontend issues the drain on every `app://intake` nudge and once on root-shell
 * mount"): the root-shell mount (`useLaunchDrain`, P2.120) AND the payload-less `app://intake` nudge listener
 * (`subscribeAppEvents`, retiring the old payload-carrying `ingestFromIntakeEvent`). Each call drains the single
 * hand-off buffer once — for a first-launch/Open-with set that raced the listener, a second-instance launch, a
 * native drop, or a C2a-picked set (all handled core-side and stashed into the same buffer, P3.77/P3.78).
 * [Build-Session-Entscheidung: P3.78] It calls the args-less C1 `drain_intake { collectingId, onScan }` — the
 * WebView supplies no path / origin / drain flag; every call drains the core-side buffer (the P2.60
 * `drainPending: true` + empty-`paths` shape is retired with `ingest_paths`).
 *
 * - `collectingId` is a fresh §0.4.4 ingest-cancel handle (a drain is quick, but the contract requires one so
 *   C13 can cancel a long walk);
 * - `onScan` is the required §0.4.1 `Channel<ScanProgress>` (the C1 non-optional forced deviation) — a drain of
 *   a small buffer has little scan progress, so it is a bare unsubscribed sink here.
 *
 * Returns the §0.6 `CollectedSet`: a non-empty drain would drive the §5.2 `Collecting`/`Confirm` transition, an
 * empty drain (the ordinary first launch with no files) leaves the UI `Idle`. Through P3.78 the Rust §1.1 freeze
 * seam is a shell, so the result is `Empty` until P3.49 wires the real freeze. This helper is the drain TRIGGER;
 * feeding the result into the §5.2 machine — dispatching the `collected`/`startCollecting`/`scanTick` Msgs — is
 * the **§5.8 drain consumption** (§5.4 names it "the §5.8 C1 `drain_intake` consumption"), owned by the SCREEN
 * boxes that make each target state reachable (P3.55 Confirm is the first `collected`→Confirm consumer), NOT this
 * trigger. So the drained result is currently unconsumed by design; P3.55+ wire the dispatch.
 */
export async function drainPendingIntake(): Promise<CollectedSet> {
  const collectingId = crypto.randomUUID();
  const onScan = new Channel<ScanProgress>();
  return commands.drainIntake(collectingId, onScan);
}

/**
 * [Build-Session-Entscheidung: P3.54] Fire the §0.4.1 C2a `pick_for_intake` intake picker — the §5.3 DropZone's
 * click-to-browse (`kind: "files"`) / choose-folder (`kind: "folder"`) action. C2a opens the native
 * files/folder dialog Rust-side via `DialogExt` (no `dialog:allow-open` WebView grant, §0.10/§5.4) and returns
 * `()`: the picked set is routed core-side through the same §7.8.1 funnel every intake source uses →
 * `State<PendingIntake>` → the payload-less `app://intake` nudge (§0.4.2), and the WebView completes the intake
 * via the C1 drain ({@link drainPendingIntake}, wired on the nudge by {@link subscribeAppEvents}). So NO raw FS
 * path ever reaches the WebView, and this call's own resolution carries nothing to act on. A cancelled dialog is
 * a clean core-side no-op — nothing buffered, no nudge; the UI stays Idle (§5.4).
 *
 * Fire-and-forget: the §5.3 DropZone `void`s it (the completion is the nudge→drain, never this promise). A
 * genuine dialog-subsystem `Err(IpcError)` (rare — a folder/file pick has no *user-facing* failure) surfaces
 * through the §7.5.1 global frontend-error bridge as a structural record (`installFrontendErrorLog`), exactly as
 * an unhandled {@link drainPendingIntake} rejection would; its user-visible §5.3 `CommandError` inline surface is
 * a state-screen concern (§5.8), not this intake-trigger box.
 */
export async function pickForIntake(kind: PickKind): Promise<void> {
  await commands.pickForIntake(kind);
}

// ─── P2.120: the §5.8 frontend async model (Channel<ConversionEvent> lifecycle + the three app:// listeners) ───

/**
 * [Build-Session-Entscheidung: P2.124] The §5.8/§2.13.3 backend-disconnect fault seam for the conversion-run
 * lifecycle. `onRunFault` fires when a mid-run backend disconnect is detected — an **app-level** fault (§2.13.1:
 * the core panicked, the IPC channel dropped), NOT a per-item `ItemFinished{Failed}` — so the UI can route to
 * the AppFault surface (state 12) and stop the run **without fabricating outcomes for items it never heard back
 * about** (§5.8). It is a DISTINCT fault from the `app://fault` `AppFault` DTO ({@link AppEventHandlers.onFault}),
 * whose kinds ({EngineMissing, WebviewFault, BundleDamaged}) are the §7.2 STARTUP faults. UNSET in P2: the P3.53
 * FSM supplies it (dispatch → state 12 / `AppFaultNotice`, P3.60→P8) and adds the §5.8 mid-run **channel-silence**
 * watchdog (the per-engine wall-clock watchdog is P3.44/P4.12); this box wires the SEAM + the buildable-now
 * `start_conversion`-rejection detection SOURCE.
 */
export interface ConversionRunHandlers {
  /** §5.8/§2.13.3 mid-run backend-disconnect → AppFault (state 12). UNSET in P2 (P3.53 supplies it). */
  readonly onRunFault?: () => void;
}

/**
 * [Build-Session-Entscheidung: P2.120/P2.124] The §5.8 `start_conversion` progress lifecycle: create the
 * run-scoped, ordered `Channel<ConversionEvent>`, route every event into the store's `applyConvertEvent` reducer
 * (§5.8 — the per-item `progress` map + the `pendingVideoReencodeNote` keep/clear), then fire C6
 * `start_conversion` (§0.4.1), which returns quickly with the minted `RunId` while progress streams over the
 * Channel (the §5.8 "respond immediately, stream the rest" posture). The Channel dies with the run (§0.4.2) — a
 * fresh one is minted per call.
 *
 * **P2.124 fault wiring:** the `start_conversion` rejection is CLASSIFIED (§5.8 "command Promise rejects
 * **unexpectedly**"). A structured §0.4.3 `IpcError` (Throw-mode surfaces a business `Err(IpcError)` as a
 * rejection — a documented, EXPECTED error, e.g. a stale `CollectedSetId`) is **not** app-level: it re-throws
 * unsignalled for the caller to route (the §5.3 `CommandError`, P3.53). An OPAQUE (non-`IpcError`) rejection is
 * the "core panic / IPC drop" case → `onRunFault` (→ AppFault, state 12). Either way the rejection is
 * **re-thrown** — the seam NOTIFIES, it never swallows the failure. The CALLER (the §5.2 Convert transition,
 * P3.53) invokes this once the batch/target/destination are chosen and supplies `onRunFault` (mirrors
 * `drainPendingIntake`, whose consumption also lands with the state machine).
 */
export async function startConversionRun(
  collectedSetId: CollectedSetId,
  target: TargetId,
  options: OptionValues,
  destination: DestinationChoice,
  rerunDecision: RerunDecision,
  handlers: ConversionRunHandlers = {},
): Promise<RunId> {
  const onProgress = new Channel<ConversionEvent>();
  onProgress.onmessage = (event) => {
    useAppStore.getState().applyConvertEvent(event);
  };
  try {
    return await commands.startConversion(
      collectedSetId,
      target,
      options,
      destination,
      rerunDecision,
      onProgress,
    );
  } catch (fault) {
    // §5.8: only an UNEXPECTED rejection is an app-level fault. A structured §0.4.3 `IpcError` (Throw-mode
    // surfaces a business `Err(IpcError)` as a rejection) is the DOCUMENTED error contract, NOT a disconnect,
    // so it re-throws unsignalled for the caller to route (the §5.3 `CommandError`, P3.53). An OPAQUE
    // (non-`IpcError`) rejection is the "core panic / IPC drop" case → `onRunFault` (AppFault, state 12).
    if (!isIpcError(fault)) {
      handlers.onRunFault?.();
    }
    throw fault;
  }
}

/**
 * [Build-Session-Entscheidung: P2.124] Discriminate a structured §0.4.3 `IpcError` (the documented business
 * error contract — Throw-mode throws a Rust `Err(IpcError)` as this shape) from an OPAQUE transport/panic
 * rejection: only the latter is the §5.8 "rejects unexpectedly (core panic, IPC drop)" app-level fault. An
 * `IpcError` always carries a string `kind` (the §2.8 taxonomy code); a transport error is a bare `Error` /
 * string with none, so the shape check is the sound, minimal discriminator (an unknown shape falls through to
 * the app-fault path — the safe default, never a silent misroute).
 */
function isIpcError(value: unknown): boolean {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    typeof (value as { kind: unknown }).kind === "string"
  );
}

// [Build-Session-Entscheidung: P3.77] The old payload-carrying `app://intake` handler (`ingestFromIntakeEvent`,
// P2.120) is RETIRED with `IntakePayload`: the 2026-07-06 core-owned-path ruling makes `app://intake` a
// PAYLOAD-LESS nudge (no path crosses the wire), so the listener issues the DRAIN (`drainPendingIntake`) — the
// paths were stashed core-side in `PendingIntake` and drain via the C1 response (§7.8.1). The second-instance /
// Open-with paths a `LIVE intake` used to carry are now buffered core-side exactly like a first-launch set.

/**
 * [Build-Session-Entscheidung: P2.120] Optional typed handlers for the two §0.4.2 `app://` events whose §5.8
 * intent is a §5.2 finite-state-machine transition that does not exist until P3.53. They are UNSET in P2.120 —
 * a §5.8-mandated on-mount registration seam with named fillers on record (typed optional props, G8-clean):
 * `app://fault`'s dispatch → FSM state 12 is the P3.53 reducer (the channel-death fault SOURCE is P2.124; the
 * `AppFaultNotice` render is the P3.60 slice → P8.19.1 copy; the `app://fault` EMIT + `PendingFault` buffer is
 * P4), and `app://close-requested`'s → QuitConfirm (state 11) body is P4.67.1. `app://intake` is NOT a prop —
 * it is handled internally (→ the C1 `drain_intake` drain via `drainPendingIntake`), live from P2.120.
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
 * - `app://intake` → the payload-LESS DRAIN (`drainPendingIntake`, P3.77): the 2026-07-06 core-owned-path
 *   ruling made this event a pure "come and drain `PendingIntake`" nudge (no payload), so the listener issues
 *   the same drain as the mount does — the paths were stashed core-side and drain via the C1 response. It drains
 *   unconditionally here; the §5.8 defence-in-depth "ignore-unless-Idle/Summary + BusyNotice" guard on a leaked
 *   mid-run nudge is the P3.53 FSM's (only `idle` exists in P2, and the §7.1 single-instance callback is the
 *   authoritative primary refuse-busy gate — the funnel drops a mid-run intake before it ever nudges);
 * - `app://fault` → `handlers.onFault?.(payload)` (UNSET in P2 — see {@link AppEventHandlers});
 * - `app://close-requested` → `handlers.onCloseRequested?.()` (unit payload, no DTO; UNSET in P2).
 */
export async function subscribeAppEvents(handlers: AppEventHandlers = {}): Promise<() => void> {
  const unlisteners = await Promise.all([
    listen("app://intake", () => {
      void drainPendingIntake();
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

// ─── P2.121: the §5.4 native drag-active affordance (Tauri's window onDragDropEvent; the DROP itself is core-side, P3.77) ───

/**
 * [Build-Session-Entscheidung: P2.121] The optional drag-active visual callback. `subscribeNativeDragDrop`
 * fires it `true` on `enter`/`over` and `false` on `leave`/`drop` (§5.4 "visual affordance only"). It is UNSET
 * in P2.121 — the §5.3 `DropZone` component that consumes the `dragActive` flag is P3+, so this is a typed
 * seam (like the {@link AppEventHandlers} fault/close callbacks), not a `dragActive` store field: per the
 * §5.1-store-shape discipline a store field is homed by the P1.31.2 shell, not ad-hoc-minted mid-phase.
 */
export interface NativeDragDropHandlers {
  readonly onDragActiveChange?: (active: boolean) => void;
}

/**
 * [Build-Session-Entscheidung: P2.121] Wire the §5.4 native drag-active affordance via Tauri v2's window
 * `onDragDropEvent`. `enter`/`over` → drag-active `true`, `leave`/`drop` → `false` (the hover affordance, routed
 * to the UNSET-in-P2 `onDragActiveChange` seam — its §5.3 `DropZone` consumer is P3+).
 *
 * [Build-Session-Entscheidung: P3.77] The `drop` no longer INGESTS from the WebView — the 2026-07-06
 * core-owned-path ruling moved the native drop CORE-SIDE (`WindowEvent::DragDrop` → the §7.8.1 funnel →
 * `PendingIntake` → the payload-less `app://intake` nudge → the C1 drain), so ingesting here too would
 * DOUBLE-INGEST one drop. The `drop` phase now only clears the drag-active affordance; the dropped paths never
 * enter the WebView (the §0.4.0 boundary fact). This Tauri-`onDragDropEvent` affordance is the interim; the
 * §7.8.1 "drag-over styling from DOM drag events only" DropZone replaces it at P3.54, and P3.81 retires this
 * interim listener (its post-screens hand-off completion, the 2026-07-12 re-ordering ruling).
 * This is the ONLY place `onDragDropEvent` is wired (§0.7). Returns the unlisten.
 */
export async function subscribeNativeDragDrop(
  handlers: NativeDragDropHandlers = {},
): Promise<() => void> {
  return getCurrentWindow().onDragDropEvent((event) => {
    switch (event.payload.type) {
      case "enter":
      case "over":
        handlers.onDragActiveChange?.(true);
        break;
      case "leave":
      case "drop":
        // §5.4/§7.8.1 (P3.77): the drop is handled CORE-SIDE — clear the affordance only, never ingest here
        // (a WebView ingest would double-ingest the drop the Rust `WindowEvent::DragDrop` handler already took).
        handlers.onDragActiveChange?.(false);
        break;
    }
  });
}

// [Build-Session-Entscheidung: P3.77] The old WebView drop→C1 handler (`ingestFromDrop`, P2.121) is RETIRED:
// the native drop is handled core-side (`WindowEvent::DragDrop` → the §7.8.1 funnel), so the WebView no longer
// ingests a drop — it would double-ingest. The drop's paths route through `PendingIntake` + the payload-less
// `app://intake` nudge + the C1 drain (`drainPendingIntake`), exactly like every other intake origin.
