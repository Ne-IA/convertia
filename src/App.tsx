// src/App.tsx — the top-level screen-state router SHELL (§5.1 / §5.2).
//
// P1 lands ONLY the router seam + a minimal mounted Idle screen: the empty window the §5.2
// `Idle` state shows before any file is dropped. Everything below is a named, scheduled box —
// NOT a quiet deferral:
//   - the §5.2 finite-state machine (the 12-state discriminated union + reducer) is the
//     separate `state/machine.ts` artifact (P3.53 slice subset → P4.80 all 12 states);
//   - the per-state screens (DropZone, BatchSummary, FormatPicker, ProgressList, …) are P3–P8;
//   - the §5.7 `idle_reassurance` copy ("All conversion happens locally, …") is owned by
//     `strings/ui.ts` (P1.37) and rendered into the Idle empty-state by P8.17 — so no text is
//     hardcoded here.
// This component renders only the `<main>` landmark so the empty ConvertIA window boots — the
// P1 phase end-state assembled by P1.31 (this mount) + P1.23 (index.html) + P1.16 (window
// model). The machine-state switch that selects a screen is wired when `state/machine.ts`
// lands (P3.53). [Build-Session-Entscheidung: P1.31]
//
// P2.61 wired the §7.8.1 root-shell-mount first-launch drain trigger (`useLaunchDrain`); P2.120 adds
// `useAppEvents()` — the three §5.8 `app://` listener registrations — ABOVE it, because the drain must run
// AFTER the `app://intake` listener exists (the §7.8.1 listener race). ORDERING is load-bearing: keep
// `useAppEvents()` before `useLaunchDrain()`. P2.121 adds `useNativeDragDrop()` (the §5.4 native file-drop) —
// order-independent of the drain (a live drop is never a buffered launch path). [Build-Session-Entscheidung: P2.121]
//
// This root render IS §7.2.1 step 8 — "hand to UI empty/idle state (§5.2)": the terminal step of the ordered
// startup sequence (src-tauri `main()`'s spine, P2.106). After the Rust core reveals the window (step 6) and
// feeds launch intake (step 7), control passes to this React shell, which renders the §5.2 `Idle` empty state
// (the `<main>` landmark; the §5.7 reassurance copy + the 12-state screens land P3–P8) AND completes the
// readiness handshake — `useLaunchDrain` re-calls C1 `drainPending`, which flips the core `FrontendReady`
// flag via `mark_ready` (P2.60) so buffered launch paths replay. [Build-Session-Entscheidung: P2.106.8]
import { useAppEvents } from "./hooks/useAppEvents";
import { useLaunchDrain } from "./hooks/useLaunchDrain";
import { useNativeDragDrop } from "./hooks/useNativeDragDrop";

export function App() {
  useAppEvents();
  useNativeDragDrop();
  useLaunchDrain();
  return <main />;
}
