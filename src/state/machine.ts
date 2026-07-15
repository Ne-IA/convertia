// src/state/machine.ts — the §5.2 screen-state finite-state machine (the walking-skeleton SLICE subset, P3.53).
//
// A PURE reducer `transition(state, msg) → state` over the §5.2 slice states. Per the 2026-07-13 P3.53 ruling
// (option ①), the FULL slice machine lives HERE — the flow single-source-of-truth — so the §5.3 screens stay
// presentational ("presentational + wired to the store/machine"): they RENDER `state` and DISPATCH `Msg`s, and
// never hold transition logic. §5.2 is normative for the derivation ("Phase 3 derives the machine from these
// [state/transition tables], not the art"): every transition cell of the slice states is a reducer arm below.
//
// PURITY CONTRACT (the ruling's G1 constraint): the reducer performs NO command call and NO effect. The
// command-firing (C1 drain, C3 get_targets, C4 plan_output, C5 set_destination, C6 start_conversion, C7
// cancel_run) lives in the screen/hook wiring layer (P3.54+); it fires commands on user actions + feeds their
// RESULTS/EVENTS back as `Msg`s. The machine only SEQUENCES the user through the backend's facts (§5.2 "the
// backend is the source of truth; the machine only sequences the user through them").
//
// SLICE SCOPE (P3.53 → P4.80): states 1–10 + 12 (Idle/Collecting/Confirm/Targets+Destination/RerunPrompt/
// Converting[+7a Cancelling]/Summary/MixedDropRefusal/Unsupported/AppFault). NOT the slice: state 11
// `AppCloseRequested` (P4.67.1) and the full 7a button semantics (P4.67) — this box wires the 7a ARM
// (enter-on-cancel, second-Esc-ignored, → Summary(partial)); P4.80 completes all 12 states.
//
// [Build-Session-Entscheidung: P3.53] The `Msg` names are this fill's naming (the §5.2 tables + the P3.53
// [Decision]-note ~18-msg sketch are the faithful source). User-facing literals are NOT here — the machine is
// pure logic; the §5.3 screens own their `strings/ui.ts` copy (G57), so this module holds no display strings.
import type {
  AppFault,
  CollectedSet,
  CollectingId,
  DestinationChoice,
  DestinationResolved,
  OptionValues,
  OutputPlanPreview,
  RerunPrompt,
  RunId,
  RunResult,
  SkippedItem,
  TargetId,
  TargetOffer,
} from "../lib/ipc/commands";

// ─── payload sub-types (narrowed from the §0.6 wire unions) ──────────────────────────────────────────────

/** The §1.4 confirm-summary payload — the `single` arm of the §0.6 `CollectedSet` union (detected format +
 *  count + skipped + display roots). `NonNullable` strips the sibling arms' `single?: never`. */
export type SingleSet = NonNullable<CollectedSet["single"]>;

/** The §1.3 mixed-drop refusal payload — the per-format `[format, count]` tally for state 9. */
export type MixedFound = NonNullable<CollectedSet["mixed"]>["found"];

/** Why a lone/empty collection is not convertible (§1.2/§1.3) — the state-10 (`Unsupported`) render variant,
 *  projected from the non-`Single` `CollectedSet` arms (`unsupported`/`uncertain`/`empty`). A user-drop `Empty`
 *  lands here (nothing convertible); a PLAIN-launch `Empty` stays `Idle` (the state-1 launch nuance, below). */
export type UnsupportedReason =
  | { readonly kind: "unsupported"; readonly detected: string }
  | { readonly kind: "uncertain"; readonly note: string }
  | { readonly kind: "empty"; readonly skipped: SkippedItem[] };

/** The Targets + Destination held plan (§5.2 states 4/5, folded) — everything the FormatPicker + DestinationBar
 *  render and the Convert transition reads. `preview` is the last C4 `OutputPlanPreview` (or the C5-refreshed
 *  form); its `preflight.upFrontFail` disables Convert (§1.10) and its `rerun` gates the RerunPrompt branch
 *  (§2.5). [Build-Session-Entscheidung: P3.53] */
export interface Planned {
  /** The frozen §1.4 collected set (its §0.4.4 id + confirm summary) — carried forward from Confirm so the
   *  §5.2 row-4 Back returns to Confirm PRESERVING the frozen set (§5.2 "Back preserves the frozen set"). Its
   *  `id` is the handle the C4/C5/C6 commands resolve. */
  readonly set: SingleSet;
  /** The C3 offered targets + the one pre-highlighted default (§1.5). */
  readonly offer: TargetOffer;
  /** The currently-selected target (the offer's default until the user changes it). */
  readonly selected: TargetId;
  /** The effective whole-batch options (§1.6) — empty for the CSV→TSV slice. */
  readonly options: OptionValues;
  /** The chosen destination — the id-keyed wire form (`ChosenRoot(DestinationId)` / `BesideSource`, P3.80). */
  readonly destination: DestinationChoice;
  /** The last C4/C5 plan preview: the "will save to…" line + divert + §1.10 preflight + §2.5 rerun verdict. */
  readonly preview: OutputPlanPreview;
}

// ─── the slice State (a §5.2 discriminated union) ────────────────────────────────────────────────────────

/** The §5.2 screen-state value the store holds — one variant per slice state, each carrying exactly the data
 *  that state renders (§5.2 "each state is a discriminated union variant carrying exactly the data that state
 *  needs"). Discriminated by `tag`; the reducer is exhaustive over it. [Build-Session-Entscheidung: P3.53] */
export type State =
  /** State 1 — the empty drop-or-browse invitation (no data). */
  | { readonly tag: "idle" }
  /** State 2 — the backend is freezing/detecting the dropped/picked/launch set; `scanned` is the throttled
   *  §0.4.2 `onScan` live count (`null` = no count yet → the indeterminate "looking at your files…" copy). */
  | {
      readonly tag: "collecting";
      readonly collectingId: CollectingId;
      readonly scanned: number | null;
    }
  /** State 3 — the mandatory §1.4 confirm gate over a single-format collected set. */
  | { readonly tag: "confirm"; readonly set: SingleSet }
  /** States 4/5 (folded) — the FormatPicker + DestinationBar over the held `Planned`. */
  | { readonly tag: "targets"; readonly plan: Planned }
  /** State 6 — the one batch-level §2.5 re-run interstitial over the held plan (entered ONLY from a C4 `rerun`). */
  | { readonly tag: "rerunPrompt"; readonly plan: Planned; readonly rerun: RerunPrompt }
  /** State 7 (+7a) — the live conversion run; `cancelling` is the §5.2 7a `Converting (Cancelling…)` sub-state. */
  | { readonly tag: "converting"; readonly runId: RunId; readonly cancelling: boolean }
  /** State 8 — the §1.12 end-of-batch summary. */
  | { readonly tag: "summary"; readonly result: RunResult }
  /** State 9 — the §1.3 hard mixed-format pre-flight refusal. */
  | { readonly tag: "mixedDropRefusal"; readonly found: MixedFound }
  /** State 10 — the §1.2 unsupported/uncertain/empty pre-flight outcome. */
  | { readonly tag: "unsupported"; readonly reason: UnsupportedReason }
  /** State 12 — the app-level fault surface (§2.13.3), reachable from ANY state via the `appFault` wildcard. */
  | { readonly tag: "appFault"; readonly fault: AppFault };

// ─── the Msg union (user actions + inbound §5.8 IPC results/events) ──────────────────────────────────────

/** The reducer's input — user actions AND inbound §5.8 IPC results/events (§5.2 "transitions are driven by
 *  user actions and by inbound IPC results/events"). NO msg is a command call — the wiring layer fires the
 *  command and feeds its result back as a `Msg`. [Build-Session-Entscheidung: P3.53] */
export type Msg =
  /** User dropped/picked/launched → the intake walk begins (Idle → Collecting). Carries the frontend-minted
   *  ingest-cancel handle (§0.4.4) C13 targets. */
  | { readonly type: "startCollecting"; readonly collectingId: CollectingId }
  /** The intake picker was cancelled — a clean no-op that STAYS `Idle` (§5.2 state-1; its OWN Msg, never the
   *  `collected(Empty)` arm, which is the Unsupported(10) path — the P3.53 ruling's distinct-Msg constraint). */
  | { readonly type: "pickerCancelled" }
  /** A throttled §0.4.2 `onScan` tick — updates the Collecting live count. */
  | { readonly type: "scanTick"; readonly scanned: number }
  /** The C1 `drain_intake` result (§0.4.1 / §1.3 grouping). Routed by arm: Single → Confirm; Mixed →
   *  MixedDropRefusal; Unsupported/Uncertain → Unsupported; Empty → Unsupported (from Collecting) OR stays
   *  Idle (from the plain-launch mount-drain). */
  | { readonly type: "collected"; readonly set: CollectedSet }
  /** The Collecting cancel-collect control (Esc, §5.10) — discards the partial set → Idle. */
  | { readonly type: "cancelCollect" }
  /** The C3 `get_targets` + eager C4 `plan_output` resolved on the 3→4 transition (§5.8 call-timing) — enter
   *  Targets with the offer + initial plan + initial destination. */
  | {
      readonly type: "targetsReady";
      readonly offer: TargetOffer;
      readonly plan: OutputPlanPreview;
      readonly destination: DestinationChoice;
    }
  /** Cancel the pre-run wizard back to Idle (§5.2/§5.10) — the Confirm Esc (row 3) AND the Targets/Destination
   *  Ctrl/⌘+N "cancel back to Idle" (§5.10 row 1180 — no temp written yet, so nothing to clean). Distinct from
   *  the Targets `back` (→ Confirm, preserving the frozen set). */
  | { readonly type: "cancel" }
  /** The user selected a different target tile — updates `selected` (the wiring re-fires C4 → `planResolved`). */
  | { readonly type: "selectTarget"; readonly target: TargetId }
  /** A C4 `plan_output` re-plan result (target/option/destination change, debounced §5.8) — refreshes the preview. */
  | { readonly type: "planResolved"; readonly plan: OutputPlanPreview }
  /** A C5 `set_destination` result — the user changed the destination; refresh destination + preview (the §2.5
   *  `rerun` is destination-INDEPENDENT and carried through unchanged, §2.5.1). */
  | { readonly type: "destinationResolved"; readonly resolved: DestinationResolved }
  /** The user pressed Convert. With a C4 `rerun` verdict → RerunPrompt; without → a no-op (the wiring fires C6,
   *  and `runStarted` drives Converting). */
  | { readonly type: "convert" }
  /** The Targets→Confirm Back (Ctrl/⌘+Backspace / Back button, §5.10) — preserves the frozen set. */
  | { readonly type: "back" }
  /** Esc on the RerunPrompt (§5.2 row 6) — return to Targets with the held plan intact. */
  | { readonly type: "rerunCancel" }
  /** C6 `start_conversion` returned the minted `RunId` (§0.4.1) — enter Converting. */
  | { readonly type: "runStarted"; readonly runId: RunId }
  /** The terminal §0.4.2 `RunFinished(RunResult)` (or the C8 re-fetch) — every job reached a terminal state → Summary. */
  | { readonly type: "runFinished"; readonly result: RunResult }
  /** The Cancel button during Converting (C7 `cancel_run`, §5.8) — enter the 7a `Cancelling…` sub-state; a
   *  SECOND cancel/Esc while cancelling is IGNORED (§5.2 row 7a). */
  | { readonly type: "cancelRun" }
  /** "Convert more" on Summary → Idle. */
  | { readonly type: "convertMore" }
  /** Dismiss a pre-flight refusal/notice (MixedDropRefusal / Unsupported) → Idle. */
  | { readonly type: "dismiss" }
  /** A fresh single-format drop/pick onto the MixedDropRefusal screen's active DropZone → Collecting (§5.2 row 9). */
  | { readonly type: "redrop"; readonly collectingId: CollectingId }
  /** "Start over" (Ctrl/⌘+N) on AppFault → Idle. */
  | { readonly type: "startOver" }
  /** The global §0.4.2 `app://fault` — routes to AppFault from ANY state (the §5.2 state-12 WILDCARD edge). */
  | { readonly type: "appFault"; readonly fault: AppFault }
  /** A CONVERTING-scoped §5.8/§2.13.3 backend disconnect — the run's `ConversionEvent` Channel goes silent or
   *  the C7 cancel round-trip drops — an app-level fault in place of Summary → AppFault (handled ONLY in
   *  Converting, below). A PRE-Converting C6 `start_conversion` OPAQUE reject (from Targets/RerunPrompt, §5.8
   *  `startConversionRun.onRunFault`) instead dispatches the `appFault` WILDCARD (→ AppFault from any state); a
   *  STRUCTURED §0.4.3 `IpcError` pre-run reject is the §5.3 `CommandError` inline slot (§5.2 state-12), not a fault. */
  | { readonly type: "runFault"; readonly fault: AppFault };

// ─── construction ────────────────────────────────────────────────────────────────────────────────────────

/** The §5.2 initial machine state — `Idle` for a plain launch (the store-init default). A launch-WITH-files
 *  (§7.8.1 Open-with/argv) instead makes the initial state `Collecting` via {@link launchCollectingState} at
 *  store init; the plain-launch mount-drain (§7.8.1) resolves from `Idle` — an `Empty` result STAYS `Idle`
 *  (never Unsupported), a non-empty launch set advances exactly like a drop (the `idle + collected` arm below).
 *  [Build-Session-Entscheidung: P3.53] */
export function initialState(): State {
  return { tag: "idle" };
}

/** The §5.2 launch-with-files initial state (§7.8.1) — the store inits to THIS (not `Idle`) when the app was
 *  launched with files, so the first screen is `Collecting` while the mount-drain freezes the launch set (not
 *  the empty `Idle` invitation). The `collectingId` is the mount-drain's ingest-cancel handle. This is the
 *  §5.2-normative "initial state is Collecting when launched with files" made buildable; the launch-vs-plain
 *  decision is the store-init/launch-drain wiring's (P3.54+). [Build-Session-Entscheidung: P3.53] */
export function launchCollectingState(collectingId: CollectingId): State {
  return { tag: "collecting", collectingId, scanned: null };
}

/** Map a §0.6 `CollectedSet` result onto its target state (§1.3/§1.2 routing). `emptyStaysIdle` distinguishes
 *  the plain-launch mount-drain (from `Idle`: an `Empty` result is "no files launched" → stays `Idle`) from a
 *  user drop/pick (from `Collecting`: an `Empty` result is "nothing convertible" → Unsupported(10), §5.2 row 2).
 *  The non-`Single` arms are narrowed by presence (the §0.6 `& { …?: never }` exclusivity encoding).
 *  [Build-Session-Entscheidung: P3.53] */
function collectedToState(set: CollectedSet, current: State, emptyStaysIdle: boolean): State {
  if (set.single !== undefined) {
    return { tag: "confirm", set: set.single };
  }
  if (set.mixed !== undefined) {
    return { tag: "mixedDropRefusal", found: set.mixed.found };
  }
  if (set.unsupported !== undefined) {
    return {
      tag: "unsupported",
      reason: { kind: "unsupported", detected: set.unsupported.detected },
    };
  }
  if (set.uncertain !== undefined) {
    return { tag: "unsupported", reason: { kind: "uncertain", note: set.uncertain.note } };
  }
  if (set.empty !== undefined) {
    return emptyStaysIdle
      ? { tag: "idle" }
      : { tag: "unsupported", reason: { kind: "empty", skipped: set.empty.skipped } };
  }
  // Every §0.6 `CollectedSet` arm is handled above; an unrecognised shape is a no-op (defensive — the backend
  // is the source of truth, so the machine never fabricates a transition, §5.2). Returns the current state.
  return current;
}

// ─── the reducer ─────────────────────────────────────────────────────────────────────────────────────────

/** The §5.2 pure transition function — `(state, msg) → state`, exhaustive over the state × slice-msg space with
 *  the sole documented wildcard being `app://fault → AppFault` (the §5.2 state-12 global edge, handled FIRST so
 *  it fires from every state). Every other transition is state-specific; an invalid (state, msg) pair is a
 *  no-op that returns the state unchanged (the FSM ignores a transition its current state does not define — the
 *  backend, not the machine, is the source of truth). No side effect, no command call (the P3.53 purity
 *  constraint). [Build-Session-Entscheidung: P3.53] */
export function transition(state: State, msg: Msg): State {
  // §5.2 WILDCARD: an app-level `app://fault` routes to AppFault from ANY state (the global state-12 edge). Kept
  // FIRST + as the ONLY catch-all so it is unconditional; every other arm is state-specific below.
  if (msg.type === "appFault") {
    return { tag: "appFault", fault: msg.fault };
  }
  switch (state.tag) {
    case "idle":
      return fromIdle(state, msg);
    case "collecting":
      return fromCollecting(state, msg);
    case "confirm":
      return fromConfirm(state, msg);
    case "targets":
      return fromTargets(state, msg);
    case "rerunPrompt":
      return fromRerunPrompt(state, msg);
    case "converting":
      return fromConverting(state, msg);
    case "summary":
      return fromSummary(state, msg);
    case "mixedDropRefusal":
      return fromMixedDropRefusal(state, msg);
    case "unsupported":
      return fromUnsupported(state, msg);
    case "appFault":
      return fromAppFault(state, msg);
    default:
      return assertNever(state);
  }
}

/** State 1 `Idle` — `startCollecting` begins a user drop/pick; `pickerCancelled` STAYS Idle (its own no-op Msg,
 *  never the Empty arm); `collected` handles the plain-launch mount-drain (§7.8.1 — an Empty result stays Idle,
 *  a non-empty launch set advances). */
function fromIdle(state: State & { tag: "idle" }, msg: Msg): State {
  switch (msg.type) {
    case "startCollecting":
      return { tag: "collecting", collectingId: msg.collectingId, scanned: null };
    case "pickerCancelled":
      // §5.2 state-1: a cancelled picker is a clean no-op — never a `collected(Empty)` (the Unsupported path).
      return state;
    case "collected":
      // The §7.8.1 launch mount-drain resolves from Idle: Empty → stays Idle (no files launched); non-empty
      // advances exactly like a drop. `emptyStaysIdle = true` is the state-1 vs state-2 distinction.
      return collectedToState(msg.set, state, true);
    default:
      return state;
  }
}

/** State 2 `Collecting` — `scanTick` updates the live count; `collected` routes to Confirm/Mixed/Unsupported
 *  (a user drop's Empty → Unsupported, §5.2 row 2); `cancelCollect` (Esc) discards the partial → Idle. */
function fromCollecting(state: State & { tag: "collecting" }, msg: Msg): State {
  switch (msg.type) {
    case "scanTick":
      return { ...state, scanned: msg.scanned };
    case "collected":
      // A user drop/pick: an Empty result means "nothing convertible" → Unsupported(10), not Idle.
      return collectedToState(msg.set, state, false);
    case "cancelCollect":
      return { tag: "idle" };
    default:
      return state;
  }
}

/** State 3 `Confirm` — `targetsReady` (C3+eager-C4 resolved) advances to Targets with the initial plan;
 *  `cancel` returns to Idle. */
function fromConfirm(state: State & { tag: "confirm" }, msg: Msg): State {
  switch (msg.type) {
    case "targetsReady":
      return {
        tag: "targets",
        plan: {
          set: state.set,
          offer: msg.offer,
          selected: msg.offer.defaultTarget,
          options: {},
          destination: msg.destination,
          preview: msg.plan,
        },
      };
    case "cancel":
      return { tag: "idle" };
    default:
      return state;
  }
}

/** States 4/5 `Targets`+`Destination` — `selectTarget` updates the selection; `planResolved`/`destinationResolved`
 *  refresh the preview (C4/C5, §5.8); `convert` branches to RerunPrompt iff the plan carries a §2.5 `rerun`
 *  verdict (else a no-op — the wiring fires C6 and `runStarted` drives Converting); `runStarted` enters
 *  Converting (the no-rerun path); `back` returns to Confirm preserving the set (§5.2 row-4 Back). */
function fromTargets(state: State & { tag: "targets" }, msg: Msg): State {
  switch (msg.type) {
    case "selectTarget":
      return { ...state, plan: { ...state.plan, selected: msg.target } };
    case "planResolved":
      return { ...state, plan: { ...state.plan, preview: msg.plan } };
    case "destinationResolved":
      // §2.5.1: C5 re-evaluates the destination-dependent preview but CARRIES `rerun` THROUGH UNCHANGED (the v1
      // EquivKey is destination-independent), so the refreshed preview keeps the held C4 `rerun`.
      return {
        ...state,
        plan: {
          ...state.plan,
          destination: msg.resolved.destination,
          preview: {
            ...state.plan.preview,
            finalDirDisplay: msg.resolved.finalDirDisplay,
            diverted: msg.resolved.diverted,
            preflight: msg.resolved.preflight,
          },
        },
      };
    case "convert":
      // §2.5 / §5.2 state-6: a C4 `rerun` verdict shows the RerunPrompt BEFORE convert; no verdict → a no-op
      // here (the wiring fires C6 directly, and `runStarted` below drives Converting).
      return state.plan.preview.rerun !== null
        ? { tag: "rerunPrompt", plan: state.plan, rerun: state.plan.preview.rerun }
        : state;
    case "runStarted":
      return { tag: "converting", runId: msg.runId, cancelling: false };
    case "back":
      // §5.2 row-4 Back: return to the Confirm gate PRESERVING the frozen set (distinct from Ctrl+N → Idle) —
      // the `SingleSet` threaded through `Planned` from Confirm is the preserved set, re-rendered verbatim.
      return { tag: "confirm", set: state.plan.set };
    case "cancel":
      // §5.10 row 1180: Ctrl/⌘+N in Targets/Destination (4/5) is a "cancel back to Idle" escape — no temp is
      // written yet, so nothing to clean (distinct from `back`, which preserves the set for Confirm). It shares
      // the Confirm-Esc `cancel` Msg (both abandon the pre-run wizard → Idle); the key→Msg map is the keymap's
      // (P3.54). RerunPrompt (6) is deliberately NOT bound to Ctrl/⌘+N (§5.10 row 1180 omits it) — its escape is
      // Esc → `rerunCancel` → Targets.
      return { tag: "idle" };
    default:
      return state;
  }
}

/** State 6 `RerunPrompt` — `runStarted` (the wiring fired C6 with the chosen `RerunDecision`) enters Converting;
 *  `rerunCancel` (Esc) returns to Targets with the held plan intact (§5.2 row 6). */
function fromRerunPrompt(state: State & { tag: "rerunPrompt" }, msg: Msg): State {
  switch (msg.type) {
    case "runStarted":
      return { tag: "converting", runId: msg.runId, cancelling: false };
    case "rerunCancel":
      return { tag: "targets", plan: state.plan };
    default:
      return state;
  }
}

/** State 7 (+7a) `Converting` — `runFinished` (all jobs terminal) → Summary; `cancelRun` enters the 7a
 *  `Cancelling…` sub-state (a SECOND cancel while already cancelling is IGNORED, §5.2 row 7a); `runFault` → an
 *  app-level fault in place of Summary (§5.8). The live per-item progress is the store's `reduceConvertEvent`
 *  (§0.4.2), NOT a machine field. */
function fromConverting(state: State & { tag: "converting" }, msg: Msg): State {
  switch (msg.type) {
    case "runFinished":
      // §1.9: every job reached a terminal state → the §1.12 Summary (a partial run — post-cancel — lands here too).
      return { tag: "summary", result: msg.result };
    case "cancelRun":
      // §5.2 row 7a: enter Cancelling on the first cancel; a second cancel/Esc while cancelling is a no-op
      // (no double-cancel, the button is disabled). `already cancelling → return state` is that ignore.
      return state.cancelling ? state : { ...state, cancelling: true };
    case "runFault":
      return { tag: "appFault", fault: msg.fault };
    default:
      return state;
  }
}

/** State 8 `Summary` — `convertMore` → Idle (§5.2 row 8; the OpenActions stay available on the rendered screen). */
function fromSummary(state: State & { tag: "summary" }, msg: Msg): State {
  switch (msg.type) {
    case "convertMore":
      return { tag: "idle" };
    default:
      return state;
  }
}

/** State 9 `MixedDropRefusal` — `redrop` (a fresh single-format drop/pick onto the refusal screen's active
 *  DropZone) → Collecting; `dismiss` → Idle (§5.2 row 9). */
function fromMixedDropRefusal(state: State & { tag: "mixedDropRefusal" }, msg: Msg): State {
  switch (msg.type) {
    case "redrop":
      return { tag: "collecting", collectingId: msg.collectingId, scanned: null };
    case "dismiss":
      return { tag: "idle" };
    default:
      return state;
  }
}

/** State 10 `Unsupported` — `dismiss` → Idle (§5.2 row 10). */
function fromUnsupported(state: State & { tag: "unsupported" }, msg: Msg): State {
  switch (msg.type) {
    case "dismiss":
      return { tag: "idle" };
    default:
      return state;
  }
}

/** State 12 `AppFault` — `startOver` (Ctrl/⌘+N) → Idle (§5.2 row 12). */
function fromAppFault(state: State & { tag: "appFault" }, msg: Msg): State {
  switch (msg.type) {
    case "startOver":
      return { tag: "idle" };
    default:
      return state;
  }
}

/** Exhaustiveness guard — a NEW `State` variant reaching {@link transition}'s switch fails to compile
 *  (`state: never`), so the slice reducer can never silently drop a state. Unreachable by construction (the
 *  §5.2 slice states are closed here; P4.80 adds the remaining states + this guard flags the gap). */
function assertNever(state: never): never {
  throw new Error(`unhandled slice State variant: ${JSON.stringify(state)}`);
}
