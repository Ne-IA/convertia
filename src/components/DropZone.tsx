// src/components/DropZone.tsx — the §5.3 DropZone: the §5.2 Idle intake surface (P3.54).
//
// The primary intake surface — the drop-or-browse SURFACE is itself the §5.4/§5.6.1 `role="button"` element
// (a native <button>, so Enter/Space activate it and its visible label is its accessible name), plus a
// secondary "choose a folder" button and the §5.10 Idle accelerators Ctrl/⌘+O (files) / Ctrl/⌘+Shift+O
// (folder). Presentational + wired to the §5.1 IPC façade only (§5.3): it fires C2a through `pickForIntake`
// (src/lib/ipc/events) and holds NO path — the native file drop is handled CORE-side (WindowEvent::DragDrop →
// the §7.8.1 funnel, P3.77), so the surface's drag-over affordance is DOM-drag-event styling ONLY, never a
// path source (§5.4). C2a's picked set completes via the payload-less app://intake nudge → the §5.8 C1 drain
// CONSUMPTION (owned by the screen boxes, NOT this trigger box — see below), and a cancelled picker is a clean
// core-side no-op that stays Idle (§5.4). App renders it in the Idle (1) state; P3.60 reuses it as the state-9
// MixedDropRefusal re-drop surface.
//
// SCOPE (P3.54 = the intake TRIGGER, not the machine consumption). This box's refs are §5.3 (component) / §5.4
// (native-drop boundary) / §0.4.1 (C2a): it builds the DropZone and FIRES C2a. Driving the §5.2 machine from the
// drained set — dispatching `startCollecting`/`scanTick`/`collected` — is the **§5.8 drain consumption** (§5.4
// line 432 names it "the §5.8 C1 drain_intake consumption"), which rides with the SCREEN boxes that make each
// target state reachable (P3.55 Confirm is the first `collected`→Confirm consumer); it is deliberately NOT wired
// here. So after P3.54 the picker opens and the nudge→drain runs, but the machine advances when P3.55+ land.
// [Build-Session-Entscheidung: P3.54]
import { useEffect, useState, type DragEvent } from "react";

import { keymap, matchesAccelerator } from "../a11y/keymap";
import { pickForIntake } from "../lib/ipc/events";
import { ui } from "../strings/ui";

export interface DropZoneProps {
  /** The §5.8 disabled-while-Converting guard (§5.3). Inert (false) in Idle (1) and the state-9 re-drop — the
   *  two states the DropZone renders — so it defaults false; P3.60 reuses the component in state 9. When set,
   *  every intake action (click, folder, the §5.10 accelerators) is a no-op. */
  readonly disabled?: boolean;
}

/**
 * The §5.3 DropZone. `dragActive` is DOM-drag-event visual state (§5.4/§5.5 lift), never a path source (the drop
 * itself is handled core-side, §7.8.1); firing C2a is fire-and-forget — the picked set returns via the nudge→drain.
 */
export function DropZone({ disabled = false }: DropZoneProps) {
  const [dragActive, setDragActive] = useState(false);

  // §5.10 Idle accelerators: Ctrl/⌘+O → files, Ctrl/⌘+Shift+O → folder. `matchesAccelerator` resolves the chord
  // per platform (Cmd on macOS, Ctrl elsewhere) and disambiguates by Shift (openFilePicker has no Shift;
  // chooseFolder requires it). They bind while the DropZone is mounted + enabled: in P3.54 the DropZone is
  // mounted ONLY in Idle (App renders it for `tag === "idle"`), so the §5.10 Idle-only scope holds by mounting.
  // When P3.60 reuses this component as the state-9 re-drop surface, state 9 gets Enter/Space on the focused
  // surface ONLY (never the global chords, §5.10), so that reuse must gate this binding off.
  useEffect(() => {
    if (disabled) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent): void => {
      if (matchesAccelerator(event, keymap.openFilePicker)) {
        event.preventDefault();
        void pickForIntake("files");
      } else if (matchesAccelerator(event, keymap.chooseFolder)) {
        event.preventDefault();
        void pickForIntake("folder");
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [disabled]);

  // §5.4 drag-over affordance — DOM drag events ONLY, on the surface <button>, never a path source. The surface
  // holds only its text label (no interactive children), so a plain enter→lift / leave→clear is flicker-free.
  // `preventDefault` on enter/over/drop stops the WebView navigating to a dropped file; the DOM drop merely
  // clears the affordance and NEVER ingests (the Rust WindowEvent::DragDrop handler already funnelled the paths
  // core-side, P3.77 — a WebView ingest would double it). Whether these DOM events fire on the real platform
  // WebView with `dragDropEnabled: true` is the §5.4 premise, validated live by the §6.4.6 headed E2E (P9).
  const onDragEnter = (event: DragEvent<HTMLButtonElement>): void => {
    event.preventDefault();
    if (!disabled) {
      setDragActive(true);
    }
  };
  const onDragOver = (event: DragEvent<HTMLButtonElement>): void => {
    // Required for the drop event to fire at all and to stop the WebView opening the dropped file (§5.4).
    event.preventDefault();
    if (!disabled) {
      setDragActive(true);
    }
  };
  const onDragLeave = (): void => {
    setDragActive(false);
  };
  const onDrop = (event: DragEvent<HTMLButtonElement>): void => {
    event.preventDefault();
    setDragActive(false);
  };

  const surfaceClasses = [
    "w-full rounded-lg border-2 border-dashed p-10 text-center text-text transition-colors",
    "disabled:cursor-not-allowed disabled:opacity-50",
    dragActive ? "border-accent bg-surface-raised" : "border-border bg-surface",
  ].join(" ");

  return (
    <div className="flex flex-col items-center gap-3">
      <button
        type="button"
        className={surfaceClasses}
        data-drag-active={dragActive ? "true" : "false"}
        onClick={() => void pickForIntake("files")}
        onDragEnter={onDragEnter}
        onDragOver={onDragOver}
        onDragLeave={onDragLeave}
        onDrop={onDrop}
        disabled={disabled}
      >
        {ui.dropzone_prompt}
      </button>
      <button
        type="button"
        className="text-accent underline disabled:cursor-not-allowed disabled:opacity-50"
        onClick={() => void pickForIntake("folder")}
        disabled={disabled}
      >
        {ui.dropzone_choose_folder}
      </button>
    </div>
  );
}
