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
// the first router arm — the Idle (1) `DropZone` (§5.3); P3.55 adds the Collecting (2) + Confirm (3) arms (the
// §5.8 consumption seam drives Idle → Collecting → Confirm). The remaining slice screens (Targets P3.56 …
// fault screens P3.60) add their arms as they land, and a not-yet-built state renders the empty `<main>`
// workspace until its box lands. [Build-Session-Entscheidung: P1.31] [Build-Session-Entscheidung: P3.54] [Build-Session-Entscheidung: P3.55]
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

import { CollectingScreen } from "./components/CollectingScreen";
import { ConfirmScreen } from "./components/ConfirmScreen";
import { DropZone } from "./components/DropZone";
import { useAppEvents } from "./hooks/useAppEvents";
import { useLaunchDrain } from "./hooks/useLaunchDrain";
import { useNativeDragDrop } from "./hooks/useNativeDragDrop";
import { useAppStore, type State } from "./state/store";

// §5.2 screen router: map the current machine state to its screen. P3.54 landed the Idle (1) arm; P3.55 adds
// the Collecting (2) + Confirm (3) arms (the §5.8 consumption seam drives Idle → Collecting → Confirm). The
// remaining slice states (Targets P3.56 … fault screens P3.60) render null until their box lands — never a dead
// button, because the transition INTO each state is wired by the box that first reaches it (the P3 screen-box
// wiring model). [Build-Session-Entscheidung: P3.55]
function screenFor(machine: State): ReactElement | null {
  switch (machine.tag) {
    case "idle":
      return <DropZone />;
    case "collecting":
      return <CollectingScreen collectingId={machine.collectingId} scanned={machine.scanned} />;
    case "confirm":
      return <ConfirmScreen set={machine.set} />;
    default:
      return null;
  }
}

export function App() {
  const eventsReady = useAppEvents();
  useNativeDragDrop();
  useLaunchDrain(eventsReady);
  // §5.1 selector granularity: subscribe to the whole machine state (each §5.2 transition mints a new object,
  // so this re-renders exactly on a screen change). [Build-Session-Entscheidung: P3.55]
  const machine = useAppStore((state) => state.machine);
  return <main>{screenFor(machine)}</main>;
}
