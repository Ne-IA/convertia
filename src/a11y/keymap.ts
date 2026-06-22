// src/a11y/keymap.ts -- the section 5.10 canonical accelerator table (single source).
//
// The single home for in-app keyboard CHORD accelerators (section 5.10); per-component code
// references an entry here instead of re-declaring a chord, so the section 5.10 "single source" rule
// holds from the first component. The modifier is CmdOrCtrl: Cmd (metaKey) on macOS, Ctrl elsewhere
// (section 5.10). v1 binds NO OS-global hotkeys -- every accelerator is in-app, active only while the
// window is focused (section 5.10 policy). The per-state CONTEXTUAL Esc/Enter semantics (the section
// 5.10 decision-gate table) are reducer-level, not chords, so they live with the section 5.2 machine,
// not here. Components add their handlers/bindings against these chords as they land (P5-P10).
// [Build-Session-Entscheidung: P1.40]

export interface Accelerator {
  /** The KeyboardEvent.key value, matched case-insensitively (e.g. "o", "n", "Enter", "Backspace", ".", "F1"). */
  readonly key: string;
  /** Requires the CmdOrCtrl modifier: Cmd (metaKey) on macOS, Ctrl elsewhere. */
  readonly cmdOrCtrl?: boolean;
  /** Requires Shift. */
  readonly shift?: boolean;
}

// The section 5.10 canonical chord accelerators. The name is the action; the value is the chord.
export const keymap = {
  openFilePicker: { key: "o", cmdOrCtrl: true }, // Ctrl/Cmd+O -- Idle
  chooseFolder: { key: "o", cmdOrCtrl: true, shift: true }, // Ctrl/Cmd+Shift+O -- Idle
  toggleAdvancedOptions: { key: ".", cmdOrCtrl: true }, // Ctrl/Cmd+. -- Targets
  changeDestination: { key: "d", cmdOrCtrl: true }, // Ctrl/Cmd+D -- Targets/Destination
  convert: { key: "Enter", cmdOrCtrl: true }, // Ctrl/Cmd+Enter -- Targets/Destination
  openOutputFolder: { key: "f", cmdOrCtrl: true, shift: true }, // Ctrl/Cmd+Shift+F -- Summary
  openOutputFile: { key: "Enter", cmdOrCtrl: true, shift: true }, // Ctrl/Cmd+Shift+Enter -- Summary
  backToConfirm: { key: "Backspace", cmdOrCtrl: true }, // Ctrl/Cmd+Backspace -- Targets/Destination
  startOver: { key: "n", cmdOrCtrl: true }, // Ctrl/Cmd+N -- Targets/Destination/Summary/AppFault
  about: { key: "F1" }, // F1 -- any
} as const satisfies Record<string, Accelerator>;

const isMac = typeof navigator !== "undefined" && /Mac|iP(hone|ad|od)/.test(navigator.platform);

// True iff `event` matches `accelerator`, resolving CmdOrCtrl per platform (Cmd on macOS, Ctrl
// elsewhere) and rejecting the wrong primary modifier or a stray Alt. Used by the per-component /
// reducer key handlers (P5-P10).
export function matchesAccelerator(event: KeyboardEvent, accelerator: Accelerator): boolean {
  const cmdOrCtrlDown = isMac ? event.metaKey : event.ctrlKey;
  const wrongPrimaryDown = isMac ? event.ctrlKey : event.metaKey;
  return (
    event.key.toLowerCase() === accelerator.key.toLowerCase() &&
    cmdOrCtrlDown === Boolean(accelerator.cmdOrCtrl) &&
    event.shiftKey === Boolean(accelerator.shift) &&
    !wrongPrimaryDown &&
    !event.altKey
  );
}
