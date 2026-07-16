// src/components/RerunPrompt.tsx — the §5.3 RerunPrompt: the §2.5 re-run decision modal (state 6) (P3.57).
//
// The one batch-level §2.5 re-run interstitial, rendered as a focus-trapped `role="alertdialog"` (§5.6 WCAG
// 4.1.2) — the RerunScreen (P3.57) overlays it on the inert Targets/Destination (state 4/5). Presentational +
// wired to the machine via the three §5.3 callbacks: **Skip (default, focused)** / **Make a fresh copy** /
// **Cancel** — Skip / Make-a-fresh-copy proceed to Converting (the parent fires C6 with the chosen
// `RerunDecision`); Cancel (also **Esc**, §5.10) returns to Destination with the held plan intact (§5.2 row 6).
//
// SLICE SCOPE (P3.57). The DECIDED §5.6(f) v1 copy is COUNT-FREE — a fixed heading ("Already converted with
// these settings", the alertdialog's `aria-labelledby` name) + a fixed body ("You already converted these with
// the same settings."), NOT the `equivalentCount`; that datum rides the machine's state-6 `rerun` payload (built
// P3.53) and is reserved for a future count-aware copy, so this presentational slice takes only the three §5.3
// callbacks. This box lands the box-required focus-trap + default-focus-on-Skip + Esc-cancel; the §5.6(c) global-
// accelerator suppression while state 6 is open (a focus-trap governs Tab only) and the precise focus-restore-to-
// Convert-button on close are P4.70.4 (the P3.56 DestinationBar/FormatPicker slice-scope precedent).
// [Build-Session-Entscheidung: P3.57]
import { useEffect, useId, useRef, type KeyboardEvent } from "react";

import { ui } from "../strings/ui";

export interface RerunPromptProps {
  /** §2.5: the user chose Skip (the safe default) — the parent fires C6 with `RerunDecision::Skip`. */
  readonly onSkip: () => void;
  /** §2.5: the user chose Make a fresh copy — the parent fires C6 with `RerunDecision::FreshCopy`. */
  readonly onFreshCopy: () => void;
  /** §5.2 row 6: cancel (button or Esc) — return to the inert Targets/Destination with the held plan intact. */
  readonly onCancel: () => void;
}

/** The §5.3 RerunPrompt (§2.5 re-run decision modal, slice renderer). [Build-Session-Entscheidung: P3.57] */
export function RerunPrompt({ onSkip, onFreshCopy, onCancel }: RerunPromptProps) {
  const headingId = useId();
  const bodyId = useId();
  const dialogRef = useRef<HTMLDivElement>(null);
  const skipRef = useRef<HTMLButtonElement>(null);

  // §5.6/§5.10: default focus lands on Skip (the safe default) on entry — so Enter/Space takes the safe path.
  useEffect(() => {
    skipRef.current?.focus();
  }, []);

  // §5.6/§5.10: Esc cancels (→ back to Destination); Tab/Shift+Tab is TRAPPED within the dialog's controls (the
  // §5.6 focus-trap; it governs Tab only — the §5.6(c) global-accelerator suppression is the reducer/keymap's,
  // P4.70.4). The handler sits on the dialog so a focused button's bubbled keydown reaches it.
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    if (event.key === "Escape") {
      event.preventDefault();
      onCancel();
      return;
    }
    if (event.key !== "Tab") {
      return;
    }
    const buttons = dialogRef.current?.querySelectorAll<HTMLButtonElement>("button");
    if (buttons === undefined || buttons.length === 0) {
      return;
    }
    const first = buttons[0];
    const last = buttons[buttons.length - 1];
    const active = document.activeElement;
    if (event.shiftKey && active === first) {
      // Shift+Tab off the first control wraps to the last (§5.6 focus-trap).
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && active === last) {
      // Tab off the last control wraps to the first (§5.6 focus-trap).
      event.preventDefault();
      first?.focus();
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
      <div
        ref={dialogRef}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={headingId}
        aria-describedby={bodyId}
        onKeyDown={onKeyDown}
        className="flex max-w-md flex-col gap-4 rounded-lg border border-border bg-surface-raised p-6 shadow-lg"
      >
        <h2 id={headingId} className="text-xl font-semibold text-text">
          {ui.rerun_heading}
        </h2>
        <p id={bodyId} className="text-base text-text">
          {ui.rerun_body}
        </p>
        <div className="flex flex-wrap gap-3">
          <button
            ref={skipRef}
            type="button"
            onClick={onSkip}
            className="rounded-md bg-accent px-4 py-2 text-base font-medium text-accent-contrast"
          >
            {ui.rerun_skip}
          </button>
          <button
            type="button"
            onClick={onFreshCopy}
            className="rounded-md border border-border px-4 py-2 text-base text-text"
          >
            {ui.rerun_fresh_copy}
          </button>
          <button
            type="button"
            onClick={onCancel}
            className="rounded-md border border-border px-4 py-2 text-base text-text"
          >
            {ui.rerun_cancel}
          </button>
        </div>
      </div>
    </div>
  );
}
