// src/components/ConvertingScreen.tsx — the §5.2 Converting screen (state 7 + the 7a Cancelling sub-state) (P3.58).
//
// Composes the §5.3 ProgressList (per-item rows + aggregate bar) over the store's live §5.8 progress, and OWNS
// the state-7 out-transitions (the P3 screen-box wiring model — a rendered action MUST fire its command):
//   - Cancel (button or Esc, §5.10) → C7 `cancel_run` via `cancelConversionRun` → the 7a `Converting (Cancelling…)`
//     sub-state (the optimistic `cancelRun` dispatch), then the backend's terminal `RunFinished` (partial) drives
//     Converting → Summary (the §5.8 run lifecycle in events.ts wires that transition out of Converting).
// 7a semantics (§5.2 row 7a): while `cancelling` the Cancel button is DISABLED with a "Cancelling…" label and a
// SECOND Cancel/Esc is ignored (the `cancellingRef` synchronous guard) — no double `cancel_run`, no quit-confirm
// here. The live per-item progress + aggregate come from the store (P3.58 reducer). SLICE SCOPE: the full §5.6
// keyboard/focus a11y (role=progressbar refinement, focus order) is P4.70.3; the QuitConfirm (11) overlay is
// P4.67.1; the §5.2-row-7 ConvertingNote worst-case-lossy banner is P4.65, the LowMemoryNote is the §1.10 P4
// engine's surface, the "current-item label" is the P8 visual pass; visual polish is P8. [Build-Session-Entscheidung: P3.58]
import { useCallback, useEffect, useRef } from "react";

import { cancelConversionRun } from "../lib/ipc/events";
import type { RunId } from "../lib/ipc/commands";
import { useAppStore } from "../state/store";
import { ui } from "../strings/ui";

import { ProgressList } from "./ProgressList";

export interface ConvertingScreenProps {
  /** The §0.4.1 `RunId` of the live run (from the machine's `converting` state) — the C7 cancel target. */
  readonly runId: RunId;
  /** §5.2 row 7a: `true` once Cancel fired (the `Converting (Cancelling…)` sub-state) — disables Cancel + ignores a second Esc. */
  readonly cancelling: boolean;
}

/** The §5.2 Converting screen (states 7/7a). [Build-Session-Entscheidung: P3.58] */
export function ConvertingScreen({ runId, cancelling }: ConvertingScreenProps) {
  // §5.8 live progress — the reduced per-item rows + the aggregate. Each tick mints a new object, so this
  // re-renders on progress; the §1.10 per-row selector-granularity for a 1000-row virtualised list is P4.70.3.
  const rows = useAppStore((state) => state.progress);
  const batchProgress = useAppStore((state) => state.batchProgress);
  // §5.2 row 7a "no second cancel_run" (§5.8): the `cancelling` PROP flips only AFTER the optimistic dispatch
  // re-renders, so a fast second Esc (OS key-repeat) in that sub-render window would slip past a prop-only check.
  // A synchronous ref closes it — the established `convertingRef` pattern (RerunScreen/TargetsScreen/ConfirmScreen).
  // The component remounts per run, so the ref is fresh each run.
  const cancellingRef = useRef(false);

  const onCancel = useCallback((): void => {
    // §5.2 row 7a: once a cancel has fired (ref, synchronous) OR the machine is already in 7a (prop), a second
    // Cancel/Esc is IGNORED — no double C7 `cancel_run`. The machine's `cancelRun` arm also no-ops a second, but
    // the ref stops the redundant IPC round-trip in the sub-render window a prop-only check would miss.
    if (cancellingRef.current || cancelling) {
      return;
    }
    cancellingRef.current = true;
    // §5.8: optimistic 7a entry + C7 `cancel_run` (the façade dispatches `cancelRun` then trips the token). A C7
    // rejection surfaces via the §7.5.1 global bridge (the `cancelIntakeCollect` fire-and-forget precedent).
    void cancelConversionRun(runId);
  }, [cancelling, runId]);

  // §5.10: Esc cancels the run (→ 7a); while cancelling it is inert (the `onCancel` guard). Document-level so a
  // focused ProgressList row's Esc reaches it (the ConfirmScreen precedent).
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key === "Escape") {
        event.preventDefault();
        onCancel();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [onCancel]);

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6 p-8">
      <h2 className="text-xl font-semibold text-text">{ui.converting_heading}</h2>
      <ProgressList rows={rows} batchProgress={batchProgress} />
      <button
        type="button"
        onClick={onCancel}
        disabled={cancelling}
        className="self-start rounded-md border border-border px-4 py-2 text-base text-text disabled:cursor-not-allowed disabled:opacity-50"
      >
        {cancelling ? ui.converting_cancelling : ui.converting_cancel}
      </button>
    </div>
  );
}
