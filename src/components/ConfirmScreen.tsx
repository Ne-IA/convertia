// src/components/ConfirmScreen.tsx — the §5.2 Confirm gate (state 3) screen (P3.55).
//
// The mandatory pre-convert gate (§1.4): composes the §5.3 BatchSummary (count + skip tally) and the §5.3
// FileList (the "Show N files" per-item detail), and OWNS the state-3 transitions (the P3 screen-box wiring
// model — a screen box wires the transitions its state reaches, and a rendered action button MUST fire its
// command): **Confirm** (Enter / the Continue button) fires C3 `get_targets` + the eager C4 `plan_output`
// → dispatches `targetsReady` (state 3 → 4, via the {@link advanceToTargets} façade); **Cancel** (Esc / the
// Cancel button) dispatches `cancel` → Idle (§5.2 row 3 / §5.10). Focus lands on the Continue button on entry
// (§5.6.1 state-3 landing); DOM order is summary → Confirm → Cancel → FileList (§5.6.1 traversal). The
// targets/destination SCREEN this advances to is P3.56; App renders it once P3.56 lands, so Confirm advances
// into a real (if not-yet-rendered) machine state, never a dead button. [Build-Session-Entscheidung: P3.55]
import { useEffect, useRef } from "react";

import { advanceToTargets } from "../lib/ipc/events";
import type { SingleSet } from "../state/machine";
import { useAppStore } from "../state/store";
import { ui } from "../strings/ui";

import { BatchSummary } from "./BatchSummary";
import { FileList } from "./FileList";

export interface ConfirmScreenProps {
  /** The frozen §1.4 collected set (`CollectedSet::Single`) this gate confirms — its `id` is the C3/C4 handle. */
  readonly set: SingleSet;
}

/**
 * The §5.2 Confirm-gate screen. [Build-Session-Entscheidung: P3.55]
 */
export function ConfirmScreen({ set }: ConfirmScreenProps) {
  const dispatch = useAppStore((state) => state.dispatch);
  const confirmRef = useRef<HTMLButtonElement>(null);
  // Guards a double-advance: C3+C4 resolve then dispatch `targetsReady` (unmounting this screen), but a fast
  // second click before that would fire C3+C4 twice. A ref (not `disabled`) keeps the button focused (§5.6.1).
  const advancingRef = useRef(false);

  // §5.6.1 state-3 focus-on-entry: focus the primary Continue button so Enter proceeds (§5.10).
  useEffect(() => {
    confirmRef.current?.focus();
  }, []);

  // §5.10: Esc cancels the batch back to Idle (global-in-Confirm; the native buttons handle Enter/Space on
  // whichever control is focused — Continue proceeds, the FileList disclosure toggles, §5.10).
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key === "Escape") {
        event.preventDefault();
        dispatch({ type: "cancel" });
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [dispatch]);

  const onConfirm = (): void => {
    if (advancingRef.current) {
      return;
    }
    advancingRef.current = true;
    // §5.8 3→4: C3 `get_targets` + the eager C4 `plan_output` (beside-source for the slice) → `targetsReady`
    // (advanceToTargets dispatches it on success). On rejection it re-throws to the §7.5.1 global
    // frontend-error bridge and leaves the machine in Confirm (the user retries); the full pre-run
    // CommandError inline slot rides P3.56 (its Targets/Destination screen). [Build-Session-Entscheidung: P3.55]
    void advanceToTargets(set.id).catch((error: unknown) => {
      advancingRef.current = false;
      throw error;
    });
  };

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6 p-8">
      <BatchSummary count={set.count} format={set.format} skippedCount={set.skipped.length} />
      <div className="flex gap-3">
        <button
          ref={confirmRef}
          type="button"
          onClick={onConfirm}
          className="rounded-md bg-accent px-4 py-2 text-base font-medium text-accent-contrast"
        >
          {ui.confirm_continue}
        </button>
        <button
          type="button"
          onClick={() => dispatch({ type: "cancel" })}
          className="rounded-md border border-border px-4 py-2 text-base text-text"
        >
          {ui.confirm_cancel}
        </button>
      </div>
      <FileList items={set.items} skipped={set.skipped} />
    </div>
  );
}
