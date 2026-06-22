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
export function App() {
  return <main />;
}
