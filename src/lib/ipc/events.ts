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

import type { Msg, State } from "../../state/machine";
import { useAppStore } from "../../state/store";
import {
  commands,
  type AppFault,
  type CollectedSet,
  type CollectedSetId,
  type CollectingId,
  type ConversionEvent,
  type DestinationChoice,
  type InitialDestination,
  type OpenTarget,
  type OptionValues,
  type OutputPlanPreview,
  type PickKind,
  type RerunDecision,
  type RunId,
  type ScanProgress,
  type TargetId,
  type TargetOffer,
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
 * boxes that make each target state reachable (P3.55 Confirm is the first `collected`→Confirm consumer).
 * [Build-Session-Entscheidung: P3.55] The consumption is now wired: {@link consumeMountDrain} (the root-shell
 * mount-drain, routing from Idle) and {@link consumeIntakeNudge} (the `app://intake` nudge, entering
 * Collecting) call this trigger + dispatch the returned `CollectedSet` into the §5.2 machine.
 */
export async function drainPendingIntake(): Promise<CollectedSet> {
  const collectingId = crypto.randomUUID();
  const onScan = new Channel<ScanProgress>();
  return commands.drainIntake(collectingId, onScan);
}

/**
 * [Build-Session-Entscheidung: P3.55] The §5.8 MOUNT-drain consumption (§7.8.1): drain `PendingIntake` once on
 * root-shell mount and route the `CollectedSet` into the §5.2 machine FROM Idle (the `useLaunchDrain` caller).
 * The launch-vs-nudge ASYMMETRY (memory `p3-screen-box-wiring-model`): the mount-drain dispatches `collected`
 * DIRECTLY from Idle (no `startCollecting`), so a plain-launch `Empty` STAYS Idle (the machine's `fromIdle`
 * `emptyStaysIdle=true` arm) rather than mis-routing to Unsupported — while a launch-with-files set advances
 * exactly like a drop. The launch walk therefore does NOT transit the Collecting indicator; that is the nudge
 * path's ({@link consumeIntakeNudge}).
 */
export async function consumeMountDrain(): Promise<void> {
  const set = await drainPendingIntake();
  useAppStore.getState().dispatch({ type: "collected", set });
}

/**
 * [Build-Session-Entscheidung: P3.60] Map the machine's CURRENT state onto the §5.2 Msg that enters `Collecting`
 * for a fresh intake, or `null` when this state cannot take one (the §5.4 not-drainable case). The two drainable
 * slice states have DISTINCT entry Msgs — the §5.2 tables give each its own edge, and the P3.53 machine mirrors
 * that: `Idle` (1) → `startCollecting` (the row-1 drop/pick edge), `MixedDropRefusal` (9) → `redrop` (the row-9
 * "re-drop → `Collecting`" edge the §5.3 active DropZone drives, WITHOUT a Dismiss-to-Idle round-trip). Both
 * carry the freshly-minted `collectingId` and land in `Collecting`, so the drain below is identical for either.
 */
function intakeEntryMsg(state: State, collectingId: CollectingId): Msg | null {
  if (state.tag === "idle") {
    return { type: "startCollecting", collectingId };
  }
  if (state.tag === "mixedDropRefusal") {
    return { type: "redrop", collectingId };
  }
  return null;
}

/**
 * [Build-Session-Entscheidung: P3.55] The §5.8/§5.4 NUDGE consumption: on the payload-less `app://intake`
 * nudge, if the machine can take fresh intake, enter Collecting and drain WITH scan telemetry, then route the
 * result (a drop's `Empty` → Unsupported, the machine's `fromCollecting` `emptyStaysIdle=false` arm). The
 * `collectingId` is minted HERE and dispatched via the state's entry Msg BEFORE the drain, so C13
 * `cancel_ingest` (via {@link cancelIntakeCollect}) can name the in-flight walk while it awaits (§1.1); `onScan`
 * drives the §5.2 "Scanning… N files" count via `scanTick`.
 *
 * SLICE POLICY: the drainable states are `Idle` (1) and — since P3.60 — `MixedDropRefusal` (9), i.e. exactly the
 * states that RENDER a `DropZone` (§5.3:295) and that the P3.53 machine gives a `Collecting` entry edge
 * ({@link intakeEntryMsg}). The REST of the §5.4 fresh-intake set (a nudge in Confirm/Targets/Destination/
 * Summary/state-10 re-drains) stays unwired: the §5.2 machine has no entry arm from those states, which requires
 * the P4.78 machine completion — so a nudge there is a no-op HERE and the core-side `PendingIntake` buffer is
 * PRESERVED (§7.8.1 no-loss accumulation), consumed by a subsequent drainable-state drain. The
 * non-drainable-state `BusyNotice` also lands with P4.
 *
 * STALE-WALK GUARD: the late `scanTick`/`collected` dispatches fire only if the machine is STILL this walk's
 * `Collecting` (same `collectingId`). Otherwise a cancel-then-immediately-redrop — where drain-1 resolves AFTER
 * the redrop entered a NEW walk (or a cancel returned to Idle) — would route drain-1's STALE set into the newer
 * walk's state (a `Collecting`→Unsupported/Confirm misroute). The guard drops the stale result; the fresh walk
 * keeps its state (Sonnet review, P3.55).
 */
export async function consumeIntakeNudge(): Promise<void> {
  const collectingId = crypto.randomUUID();
  const entry = intakeEntryMsg(useAppStore.getState().machine, collectingId);
  if (entry === null) {
    return;
  }
  useAppStore.getState().dispatch(entry);
  const isThisWalk = (): boolean => {
    const machine = useAppStore.getState().machine;
    return machine.tag === "collecting" && machine.collectingId === collectingId;
  };
  const onScan = new Channel<ScanProgress>();
  onScan.onmessage = (message) => {
    if (isThisWalk()) {
      useAppStore.getState().dispatch({ type: "scanTick", scanned: message.scanned });
    }
  };
  const set = await commands.drainIntake(collectingId, onScan);
  if (isThisWalk()) {
    useAppStore.getState().dispatch({ type: "collected", set });
  }
}

/**
 * [Build-Session-Entscheidung: P3.60] The §5.8 `app://fault` CONSUMPTION: route an app-level §2.13 fault into the
 * §5.2 machine's `appFault` WILDCARD (→ AppFault, state 12, from ANY state — the §5.2:262-69 global edge). This
 * is the seam {@link AppEventHandlers.onFault} reserved and the ONLY runtime entry into state 12 in P3 (the
 * DTO-less run-path entry is P4.50's, per the 2026-07-16 P3.60 ruling).
 *
 * The `AppFault` is passed through UNTOUCHED — its `message` is the §2.13.3/§7.2-owned calm line the
 * `AppFaultNotice` renders verbatim (§5.7:799), so nothing is re-authored, re-classified or dropped on the way.
 * Homed here beside {@link consumeIntakeNudge}/{@link consumeMountDrain}: every §5.8 event consumption dispatches
 * from this façade, so the store write stays inside `src/lib/ipc/**` (the §5.1 one-IPC-consumer discipline).
 */
export function consumeAppFault(fault: AppFault): void {
  useAppStore.getState().dispatch({ type: "appFault", fault });
}

/**
 * [Build-Session-Entscheidung: P3.55] The §5.2/§5.10 Collecting cancel-collect: optimistically advance the
 * machine to Idle (`cancelCollect`), then trip the ingest-scoped §0.4.4 token via C13 `cancel_ingest` so the
 * in-flight C1 walk discards its partial unfrozen set (§1.1). The `cancelCollect` dispatch FIRST is deliberate:
 * a C13-tripped C1 returns `CollectedSet::Empty` (§1.1), and the still-in-flight {@link consumeIntakeNudge}
 * `collected(Empty)` then routes from the now-Idle machine (STAYS Idle) instead of `Collecting`→Unsupported.
 * Fire-and-forget: a C13 rejection surfaces through the §7.5.1 global frontend-error bridge (§5.4).
 */
export async function cancelIntakeCollect(collectingId: CollectingId): Promise<void> {
  useAppStore.getState().dispatch({ type: "cancelCollect" });
  await commands.cancelIngest(collectingId);
}

/**
 * [Build-Session-Entscheidung: P3.55 → P3.56] The §5.8 Confirm → Targets (3 → 4) advance: run the §5.8:918
 * persisted-destination HAND-OFF (C14 `get_initial_destination`) to resolve the returning user's initial
 * destination CORE-side, then fire C3 `get_targets` + the eager C4 `plan_output` (with the pre-highlighted default
 * target + the resolved first-call destination), then dispatch `targetsReady` so the machine enters `Targets`.
 *
 * [Build-Session-Entscheidung: P3.56] **Persisted-destination hand-off (Co-Pilot ruling item 2, 7f73553):** the
 * P3.80 resolver's consumer is now WIRED — C14 resolves the saved §7.4.1 `lastDestinationMode` into a structural
 * `InitialDestination` ({@link mapInitialDestination} maps it): a re-validated `ChosenRoot` → the ordinary
 * `ChosenRoot(DestinationId)` first-call destination (no path on the wire, §2.10.1, keeping §0.6's 2-variant
 * `DestinationChoice` — no `Last` variant, no C4 mirror-back); a `Fallback` → beside-source + the §5.8:926
 * fallback-note fact; a plain `BesideSource` → the §2.7.1 default (no note). The fact rides `targetsReady` into
 * `Planned.persistedFallback` (the DestinationBar renders the passive §5.7:825 chrome note).
 *
 * A rejection (a stale `CollectedSetId` §0.4.3 `IpcError`, or an opaque core panic) RE-THROWS unhandled to the
 * §7.5.1 global frontend-error bridge and LEAVES the machine in Confirm (the user retries the gate). The full
 * §5.2 state-12 pre-run routing — the §5.3 `CommandError` inline slot (IpcError) / the AppFault wildcard
 * (opaque) — rides P4.69/P3.60, which build those surfaces (the CommandError slot lives in the Targets screen).
 */
export async function advanceToTargets(collectedSetId: CollectedSetId): Promise<void> {
  const initial = await commands.getInitialDestination();
  const { destination, persistedFallback } = mapInitialDestination(initial);
  const offer: TargetOffer = await commands.getTargets(collectedSetId);
  const plan: OutputPlanPreview = await commands.planOutput(
    collectedSetId,
    offer.defaultTarget,
    {},
    destination,
  );
  useAppStore
    .getState()
    .dispatch({ type: "targetsReady", offer, plan, destination, persistedFallback });
}

/**
 * [Build-Session-Entscheidung: P3.56] Map the C14 `InitialDestination` hand-off onto the FIRST C4 `(destination,
 * persistedFallback)` pair (§5.8:918): `chosenRoot` → the ordinary `ChosenRoot(DestinationId)` wire choice (the
 * WebView carries only the id, never the path); `fallback` → beside-source + the §5.8:926 fallback-note fact;
 * `besideSource` → the plain §2.7.1 default (no note). Pure — unit-tested over all three arms.
 */
function mapInitialDestination(initial: InitialDestination): {
  destination: DestinationChoice;
  persistedFallback: boolean;
} {
  if (initial === "besideSource") {
    return { destination: "besideSource", persistedFallback: false };
  }
  if (initial === "fallback") {
    return { destination: "besideSource", persistedFallback: true };
  }
  return { destination: { chosenRoot: initial.chosenRoot.destination }, persistedFallback: false };
}

/**
 * [Build-Session-Entscheidung: P3.56] The §5.8 Targets re-plan: fire C4 `plan_output` for the currently-held
 * (set, target, options, destination) and dispatch `planResolved` so the DestinationBar's "will save to …" line
 * + the §1.10 preflight + the §2.5 rerun verdict refresh (the §5.2 state-4 `selectTarget` re-plan). The
 * FormatPicker dispatches `selectTarget` OPTIMISTICALLY (an immediate tile highlight), then calls this to refresh
 * the preview for the new target — the machine's `planResolved` arm folds it into the held plan. For the CSV→TSV
 * slice there is one target, so a re-plan is a no-op refresh; the wiring is general (P5–P7 grow the offer). A
 * rejection RE-THROWS to the §7.5.1 global frontend-error bridge (the `advanceToTargets` precedent) — the machine
 * stays in Targets; the §5.3 `CommandError` inline slot for a C4 reject rides P4.69.
 */
export async function replanOutput(
  collectedSetId: CollectedSetId,
  target: TargetId,
  options: OptionValues,
  destination: DestinationChoice,
): Promise<void> {
  const plan: OutputPlanPreview = await commands.planOutput(
    collectedSetId,
    target,
    options,
    destination,
  );
  useAppStore.getState().dispatch({ type: "planResolved", plan });
}

/**
 * [Build-Session-Entscheidung: P3.56] The §5.4/§5.8 "Change destination" flow: fire C2b `pick_destination` (the
 * native Rust-side `DialogExt` folder dialog — no `dialog:allow-open` WebView grant, §0.10/§5.4), then, on a real
 * pick, fire C5 `set_destination` with the picked root as `DestinationChoice::ChosenRoot(destination)` and dispatch
 * `destinationResolved` so the held plan's destination + the re-validated "will save to …" / divert / §2.14.4
 * preflight refresh. C2b returns the id-keyed `DestinationPicked` (id + display, **no path on the wire**, §2.10.1);
 * the WebView carries only the id. A **cancelled** dialog (`null`) is a clean no-op — the held destination is
 * unchanged (§5.4), no C5 fires. A C2b/C5 rejection RE-THROWS to the §7.5.1 global frontend-error bridge (the
 * `advanceToTargets` precedent); the §5.3 `CommandError` inline slot for a C4/C5 reject rides P4.69.
 */
export async function pickAndSetDestination(
  collectedSetId: CollectedSetId,
  target: TargetId,
  options: OptionValues,
): Promise<void> {
  const picked = await commands.pickDestination();
  if (picked === null) {
    // §5.4: a dismissed folder dialog is a clean no-op — nothing changes, no C5 round-trip.
    return;
  }
  const resolved = await commands.setDestination(collectedSetId, target, options, {
    chosenRoot: picked.destination,
  });
  useAppStore.getState().dispatch({ type: "destinationResolved", resolved });
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
 * whose kinds ({EngineMissing, WebviewFault, BundleDamaged}) are the §7.2 STARTUP faults.
 *
 * [Build-Session-Entscheidung: P3.60] **SUPPLIER = P4.50** (re-cut from "the P3.53 FSM" per the 2026-07-16 P3.60
 * ruling, which assigns the state-12 run-path leg to P4.50): P4.50 supplies `onRunFault` and adds the §5.8 mid-run
 * **channel-silence** watchdog (the per-engine wall-clock watchdog is P3.44/P4.12). P3.60 deliberately does NOT
 * wire it — this fault carries no `AppFault` DTO (by construction the core is dead and can author nothing), so
 * supplying it under P3's non-null `runFault { fault: AppFault }` typing would force the frontend to SYNTHESIZE a
 * wire fact (§5.2: the backend is the source of truth for facts); P4.50 re-cuts that typing and owns the class's
 * copy. The P2.124 `start_conversion`-rejection detection SOURCE below stays live, firing into the
 * optional-absent handler as a documented no-op until then. (The `app://fault` RENDER chain is unaffected:
 * dispatch → state 12 / `AppFaultNotice` is P3.60→P8.)
 */
export interface ConversionRunHandlers {
  /** §5.8/§2.13.3 mid-run backend-disconnect → AppFault (state 12). UNSET through P3 (P4.50 supplies it — the
   *  2026-07-16 P3.60 ruling; see the interface doc). */
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
 * **re-thrown** — the seam NOTIFIES, it never swallows the failure. The CALLER (the §5.2 Convert transition) is
 * live from P3.56/P3.57 ({@link runConversion}), which invokes this once the batch/target/destination are chosen;
 * `onRunFault` itself is supplied by **P4.50**, not the caller ([Build-Session-Entscheidung: P3.60] — re-cut from
 * "the §5.2 Convert transition, P3.53" per the 2026-07-16 P3.60 ruling; see {@link ConversionRunHandlers}).
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
    if (event.type === "runFinished") {
      // [Build-Session-Entscheidung: P3.58] §5.8: the run reached its terminal §1.12 `RunResult` (a partial run
      // on cancel included) → drive the machine Converting (7/7a) → Summary (8). The store reducer holds no
      // `RunResult`; the machine carries it (P3.53 `runFinished` arm). This is the transition OUT of Converting
      // the P3.58 box wires; the Summary SCREEN that renders `machine.result` is P3.59.
      useAppStore.getState().dispatch({ type: "runFinished", result: event.data });
    }
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

/**
 * [Build-Session-Entscheidung: P3.56] The §5.2/§5.8 Convert transition: fire C6 `start_conversion` (via
 * {@link startConversionRun} — the run-scoped `Channel<ConversionEvent>` lifecycle) and dispatch `runStarted`
 * with the minted `RunId`, entering `Converting` (state 7). Called by the DestinationBar's Convert button on the
 * **no-rerun** path (`rerunDecision = "skip"` — no equivalent prior run exists, so the decision is moot; §2.5),
 * and by the §2.5 RerunPrompt (P3.57) with the user's `Skip`/`FreshCopy` choice on the rerun path.
 *
 * Fault handling follows the `advanceToTargets` precedent — no `onRunFault` handler is supplied here, so a C6
 * rejection (an opaque core/IPC drop OR a structured §0.4.3 `IpcError`) RE-THROWS to the §7.5.1 global
 * frontend-error bridge and leaves the machine in Targets (the user retries). The §5.2 fault routing rides the
 * boxes that build those surfaces: the `appFault` wildcard for an opaque pre-run C6 drop is **P4.50**'s, which
 * supplies `onRunFault` ([Build-Session-Entscheidung: P3.60] — re-cut from "P3.60 AppFault" per the 2026-07-16
 * P3.60 ruling, which leaves this DTO-less leg to P4.50; see {@link ConversionRunHandlers}), and the §5.3
 * `CommandError` slot for a structured reject is P4.69's. The SEAM is already live
 * ({@link ConversionRunHandlers}).
 */
export async function runConversion(
  collectedSetId: CollectedSetId,
  target: TargetId,
  options: OptionValues,
  destination: DestinationChoice,
  rerunDecision: RerunDecision,
): Promise<void> {
  const runId = await startConversionRun(
    collectedSetId,
    target,
    options,
    destination,
    rerunDecision,
  );
  useAppStore.getState().dispatch({ type: "runStarted", runId });
}

/**
 * [Build-Session-Entscheidung: P3.58] The §5.2 row-7a / §5.8 Cancel-run round-trip: optimistically enter the 7a
 * `Converting (Cancelling…)` sub-state (dispatch `cancelRun`), then trip the §0.4.4 cancellation token via C7
 * `cancel_run` (idempotent `Ok(())`, §0.4.1 — never a business `Err`). The backend then keeps finished items +
 * discards the in-flight one (§1.7/§2.6) and emits the terminal `RunFinished` (partial) → the
 * {@link startConversionRun} onmessage drives the machine Converting → Summary. The optimistic dispatch FIRST
 * mirrors {@link cancelIntakeCollect} (the C13 precedent), so a second Cancel/Esc while already in 7a is a no-op
 * (the machine's `cancelRun` arm ignores it). A C7 rejection (opaque transport only) surfaces via the §7.5.1
 * global frontend-error bridge (the fire-and-forget caller `void`s it, the `cancelIntakeCollect` precedent).
 */
export async function cancelConversionRun(runId: RunId): Promise<void> {
  useAppStore.getState().dispatch({ type: "cancelRun" });
  await commands.cancelRun(runId);
}

/**
 * [Build-Session-Entscheidung: P3.59] The §7.7 open-finished-output shell-out: fire C9 `open_path { target }`
 * with a run-scoped `OpenTarget` ID — the §5.3 OpenActions "Open folder" (`"commonRoot"`) / the split-divert
 * "Open saved-to folder" (`"divertRoot"`) / the ResultSummary reveal-residue link (`{ residue: itemId }`). This
 * is C9's FIRST frontend consumer: P3.79 re-keyed the command to `OpenTarget` and P3.51 wired the live Rust
 * handler, but no button called it until this box (the P3.79 note's "their first live frontend consumers are the
 * P3.56/P3.59 screens" — a rendered action MUST fire its command).
 *
 * The WebView names **an id, never a path** (the 2026-07-06 core-owned-paths ruling, §2.10.1): the core resolves
 * the target against `State<RunResultStore>` to its OWN recorded `PathBuf`, so membership IS the resolution
 * (§7.7.2) — there is no WebView path to validate, canonicalize or race. An unresolvable target (a mid-run call,
 * an undiverted `divertRoot`, a residue-free item) is the §7.7.3 refusal, logged core-side.
 *
 * Fire-and-forget: the caller `void`s it (opening a folder has no UI result to await, and the §5.3 OpenActions
 * stay available). A C9 rejection surfaces through the §7.5.1 global frontend-error bridge — the
 * {@link cancelIntakeCollect} / {@link pickForIntake} fire-and-forget precedent; a failed shell-out is not an
 * app-level fault (the run's outputs are published either way, §7.7).
 */
export async function openResultTarget(target: OpenTarget): Promise<void> {
  await commands.openPath(target);
}

// [Build-Session-Entscheidung: P3.77] The old payload-carrying `app://intake` handler (`ingestFromIntakeEvent`,
// P2.120) is RETIRED with `IntakePayload`: the 2026-07-06 core-owned-path ruling makes `app://intake` a
// PAYLOAD-LESS nudge (no path crosses the wire), so the listener issues the DRAIN (`drainPendingIntake`) — the
// paths were stashed core-side in `PendingIntake` and drain via the C1 response (§7.8.1). The second-instance /
// Open-with paths a `LIVE intake` used to carry are now buffered core-side exactly like a first-launch set.

/**
 * [Build-Session-Entscheidung: P2.120] Optional typed handlers for the two §0.4.2 `app://` events whose §5.8
 * intent is a §5.2 finite-state-machine transition that did not exist in P2 — a §5.8-mandated on-mount
 * registration seam with named fillers on record (typed optional props, G8-clean).
 *
 * [Build-Session-Entscheidung: P3.60] `onFault` is now SUPPLIED: App passes {@link consumeAppFault}, which
 * dispatches the §5.2 `appFault` wildcard → state 12, whose `AppFaultNotice` renders the DTO's §2.13.3/§7.2-owned
 * `message` verbatim (the P3.60 slice → P8.19.1 chrome copy). The `app://fault` EMIT + `PendingFault` buffer that
 * lights this path up in production is the P4 readiness body (`main.rs present_startup_fault`: both presentation
 * bodies are P4); the DTO-less mid-run channel-death SOURCE is P2.124's, whose handler is P4.50's
 * ({@link ConversionRunHandlers}). `onCloseRequested`'s → QuitConfirm (state 11) body remains P4.67.1.
 * `app://intake` is NOT a prop — it is handled internally (→ the §5.8 nudge consumption {@link consumeIntakeNudge}).
 */
export interface AppEventHandlers {
  /** `app://fault` (§2.13.3): an app-level fault (`AppFault`) → the §5.2 state-12 wildcard. Supplied from P3.60
   *  by {@link consumeAppFault}. */
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
 * - `app://intake` → the §5.8 nudge CONSUMPTION ({@link consumeIntakeNudge}, P3.55; the P3.77 bare
 *   `drainPendingIntake` trigger is replaced): the 2026-07-06 core-owned-path ruling made this event a pure
 *   "come and drain `PendingIntake`" nudge (no payload), and P3.55 wires its consumption — enter Collecting,
 *   drain (paths stashed core-side, returned via the C1 response), route the `CollectedSet` into the §5.2
 *   machine. The §5.4 "ignore-unless-drainable + preserve-the-buffer" guard now lives IN `consumeIntakeNudge`
 *   (slice: Idle (1) + the state-9 re-drop, i.e. the DropZone-rendering states, P3.60; the REST of the
 *   fresh-intake set + the `BusyNotice` land with P4), and the §7.1 single-instance callback stays the
 *   authoritative primary refuse-busy gate (the funnel drops a mid-run intake before it ever nudges);
 * - `app://fault` → `handlers.onFault?.(payload)` (supplied from P3.60 by {@link consumeAppFault} → the §5.2
 *   state-12 wildcard — see {@link AppEventHandlers});
 * - `app://close-requested` → `handlers.onCloseRequested?.()` (unit payload, no DTO; UNSET through P3 — P4.67.1).
 */
export async function subscribeAppEvents(handlers: AppEventHandlers = {}): Promise<() => void> {
  const unlisteners = await Promise.all([
    listen("app://intake", () => {
      // [Build-Session-Entscheidung: P3.55] The nudge CONSUMPTION: enter Collecting + drain + route the result
      // into the §5.2 machine ({@link consumeIntakeNudge}), replacing the P3.77 bare `drainPendingIntake()`
      // trigger. The §5.4 not-drainable-state guard lives IN `consumeIntakeNudge` (slice: Idle + the state-9
      // re-drop, P3.60).
      void consumeIntakeNudge();
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
