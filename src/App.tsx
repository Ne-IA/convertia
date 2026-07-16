// src/App.tsx — the top-level screen-state router SHELL (§5.1 / §5.2).
//
// P1 lands ONLY the router seam + a minimal mounted Idle screen: the empty window the §5.2
// `Idle` state shows before any file is dropped. Everything below is a named, scheduled box —
// NOT a quiet deferral:
//   - the §5.2 finite-state machine (the 12-state discriminated union + reducer) is the
//     separate `state/machine.ts` artifact (P3.53 slice subset → P4.78 all 12 states);
//   - the per-state screens (DropZone, BatchSummary, FormatPicker, ProgressList, …) are P3–P8;
//   - the §5.7 `idle_reassurance` copy ("All conversion happens locally, …") is owned by
//     `strings/ui.ts` (P1.37) and rendered into the Idle empty-state by P8.17 — so no text is
//     hardcoded here.
// This component renders the `<main>` landmark + the §5.2 screen router over the store's machine state — the
// P1 phase end-state assembled by P1.31 (this mount) + P1.23 (index.html) + P1.16 (window model). P3.54 wired
// the first router arm — the Idle (1) `DropZone` (§5.3); P3.55 added the Collecting (2) + Confirm (3) arms (the
// §5.8 consumption seam drives Idle → Collecting → Confirm); P3.56 added the Targets+Destination (4/5) arm;
// P3.57–P3.59 the RerunPrompt (6) / Converting (7/7a) / Summary (8) arms; P3.60 the pre-flight refusal + fault
// arms (9/10/12), which complete the P3.53 slice machine's state set and make the router exhaustive over it.
// [Build-Session-Entscheidung: P1.31] [Build-Session-Entscheidung: P3.54] [Build-Session-Entscheidung: P3.55] [Build-Session-Entscheidung: P3.56] [Build-Session-Entscheidung: P3.60]
//
// [Build-Session-Entscheidung: P2.137] P2.61 wired the §7.8.1 root-shell-mount first-launch drain trigger
// (`useLaunchDrain`); P2.120 added `useAppEvents()` — the three §5.8 `app://` listener registrations. P2.137
// hardened the drain gate from mount ORDER to registration COMPLETION: §7.8.1 mandates the drain fire "later
// than listener-registration, so it closes the race" (07-app-shell.md §7.8.1), and order alone let the drain's
// synchronous-flush C1 invoke overtake the three async `listen` registrations — the core would flip
// `FrontendReady` into a window where a second launch is emitted into an unregistered listener and dropped.
// `useAppEvents()` returns the per-mount registration-completion promise; `useLaunchDrain(eventsReady)` drains
// only once it SETTLES (both legs — the drained set returns via the C1 response, so a failed subscribe loses
// nothing). P2.121 adds `useNativeDragDrop()` (the §5.4 native file-drop) — independent of the drain gate (a
// live drop is never a buffered launch path).
//
// This root render IS §7.2.1 step 8 — "hand to UI empty/idle state (§5.2)": the terminal step of the ordered
// startup sequence (src-tauri `main()`'s spine, P2.106). After the Rust core reveals the window (step 6) and
// feeds launch intake (step 7), control passes to this React shell, which renders the §5.2 `Idle` empty state
// (the `<main>` landmark; the §5.7 reassurance copy + the 12-state screens land P3–P8) AND completes the
// readiness handshake — `useLaunchDrain` calls C1 `drain_intake` (P3.78 — every call drains), which flips the
// core `FrontendReady` flag via `mark_ready` (P2.60) so buffered launch paths replay. [Build-Session-Entscheidung: P2.106.8]
import type { ReactElement } from "react";

import { AppFaultNotice } from "./components/AppFaultNotice";
import { CollectingScreen } from "./components/CollectingScreen";
import { ConfirmScreen } from "./components/ConfirmScreen";
import { ConvertingScreen } from "./components/ConvertingScreen";
import { DropZone } from "./components/DropZone";
import { MixedDropRefusal } from "./components/MixedDropRefusal";
import { RerunScreen } from "./components/RerunScreen";
import { SummaryScreen } from "./components/SummaryScreen";
import { TargetsScreen } from "./components/TargetsScreen";
import { UnsupportedNotice } from "./components/UnsupportedNotice";
import { useAppEvents } from "./hooks/useAppEvents";
import { useLaunchDrain } from "./hooks/useLaunchDrain";
import { useNativeDragDrop } from "./hooks/useNativeDragDrop";
import { consumeAppFault, type AppEventHandlers } from "./lib/ipc/events";
import { useAppStore, type Msg, type State } from "./state/store";

// [Build-Session-Entscheidung: P3.60] The §5.8 `app://` handler set, MODULE-LEVEL so its identity is stable
// across renders: `useAppEvents` keys its subscribe effect on `handlers` (the correct dependency semantics —
// P2.137), so an inline object here would re-subscribe the three listeners on EVERY render and re-open the
// §7.8.1 registration race the drain gate exists to close. `onFault` routes `app://fault` → the §5.2 `appFault`
// wildcard → state 12 (P3.60); `onCloseRequested` (state 11 / QuitConfirm) is P4.67.1's and stays unset.
const APP_EVENT_HANDLERS: AppEventHandlers = { onFault: consumeAppFault };

// §5.2 screen router: map the current machine state to its screen. P3.54 landed the Idle (1) arm; P3.55 added
// the Collecting (2) + Confirm (3) arms; P3.56 added the Targets+Destination (4/5) arm; P3.57 added the
// RerunPrompt (6) arm; P3.58 added the Converting (7/7a) arm (the live ProgressList + Cancel); P3.59 added the
// Summary (8) arm (the §1.12 ResultSummary + the §7.7 OpenActions); P3.60 adds the pre-flight refusal + fault
// arms — MixedDropRefusal (9), Unsupported (10) and AppFault (12) — COMPLETING the P3.53 slice machine's state
// set, so the router is now exhaustive over it (a new state fails to compile, `machine: never`) and the P3-era
// null fallback is retired. State 11 (AppCloseRequested) joins with the P4.78 machine completion + P4.67.1.
// Each arm's INBOUND transition is wired by the box that first reaches it (the P3 screen-box wiring model), so
// no arm is a dead screen.
// [Build-Session-Entscheidung: P3.55] [Build-Session-Entscheidung: P3.56] [Build-Session-Entscheidung: P3.57] [Build-Session-Entscheidung: P3.58] [Build-Session-Entscheidung: P3.59] [Build-Session-Entscheidung: P3.60]
function screenFor(machine: State, dispatch: (msg: Msg) => void): ReactElement {
  switch (machine.tag) {
    case "idle":
      return <DropZone />;
    case "collecting":
      return <CollectingScreen collectingId={machine.collectingId} scanned={machine.scanned} />;
    case "confirm":
      return <ConfirmScreen set={machine.set} />;
    case "targets":
      return <TargetsScreen plan={machine.plan} />;
    case "rerunPrompt":
      return <RerunScreen plan={machine.plan} />;
    case "converting":
      return <ConvertingScreen runId={machine.runId} cancelling={machine.cancelling} />;
    case "summary":
      return <SummaryScreen result={machine.result} set={machine.set} />;
    case "mixedDropRefusal":
      return <MixedDropRefusal found={machine.found} />;
    case "unsupported":
      return <UnsupportedNotice reason={machine.reason} />;
    case "appFault":
      return (
        <AppFaultNotice fault={machine.fault} onStartOver={() => dispatch({ type: "startOver" })} />
      );
    default:
      return assertNever(machine);
  }
}

/** Exhaustiveness guard — a NEW slice `State` variant reaching {@link screenFor} fails to compile
 *  (`machine: never`), so a state can never silently render a blank `<main>`. Unreachable by construction (the
 *  P3.53 slice states are closed and all ten now have an arm); it mirrors the machine's own reducer guard.
 *  [Build-Session-Entscheidung: P3.60] */
function assertNever(machine: never): never {
  throw new Error(`unhandled slice State variant: ${JSON.stringify(machine)}`);
}

export function App() {
  const eventsReady = useAppEvents(APP_EVENT_HANDLERS);
  useNativeDragDrop();
  useLaunchDrain(eventsReady);
  // §5.1 selector granularity: subscribe to the whole machine state (each §5.2 transition mints a new object,
  // so this re-renders exactly on a screen change). [Build-Session-Entscheidung: P3.55]
  const machine = useAppStore((state) => state.machine);
  // [Build-Session-Entscheidung: P3.60] `dispatch` is a stable store action (never re-created), so selecting it
  // adds no re-render; the router needs it for the ONE §5.3 slice component whose prop contract declares a
  // callback (AppFaultNotice's `onStartOver`, §5.3:309) — its two sibling state screens declare none and
  // dispatch internally, per their own §5.3 rows.
  const dispatch = useAppStore((state) => state.dispatch);
  return <main>{screenFor(machine, dispatch)}</main>;
}
