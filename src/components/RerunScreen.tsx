// src/components/RerunScreen.tsx — the §5.2 RerunPrompt screen (state 6) (P3.57).
//
// The App-router composition for state 6: the §5.3 RerunPrompt modal overlaid on the still-mounted-but-INERT
// Targets/Destination (states 4/5) — the underlying screen stays rendered (made `inert`) so cancel/Esc returns
// to it, and this screen OWNS the state-6 out-transitions (the P3 screen-box wiring model — a rendered action
// MUST fire its command):
//   - Skip → C6 `runConversion(…, "skip")` → `runStarted` → Converting (state 7);
//   - Make a fresh copy → C6 `runConversion(…, "freshCopy")` → `runStarted` → Converting;
//   - Cancel (also Esc, on the RerunPrompt) → dispatch `rerunCancel` → Targets, held plan intact (§5.2 row 6) —
//     but INERT once a decision is committed (see the `convertingRef` guard below), so a pending `runStarted`
//     cannot teleport into Converting.
// The held plan lives in the machine's state-6 payload (P3.53), so the return keeps it regardless of the backdrop
// remount; the precise §5.6 focus-restore-to-the-Convert-button on close (+ the §5.6(c) accelerator suppression)
// is P4.70.4 (the P3.56 DestinationBar slice-scope precedent). A C6 rejection re-throws to the §7.5.1 global
// frontend-error bridge (the TargetsScreen convert precedent — the §5.3 CommandError inline slot rides P4.69).
// [Build-Session-Entscheidung: P3.57]
import { useRef } from "react";

import type { RerunDecision } from "../lib/ipc/commands";
import { runConversion } from "../lib/ipc/events";
import type { Planned } from "../state/machine";
import { useAppStore } from "../state/store";

import { RerunPrompt } from "./RerunPrompt";
import { TargetsScreen } from "./TargetsScreen";

export interface RerunScreenProps {
  /** The §5.2 state-6 held plan (carried from Targets) — the RerunPrompt's C6 args + the inert backdrop's props. */
  readonly plan: Planned;
}

/** The §5.2 RerunPrompt screen (state 6). [Build-Session-Entscheidung: P3.57] */
export function RerunScreen({ plan }: RerunScreenProps) {
  const dispatch = useAppStore((state) => state.dispatch);
  // Guards a double-convert across BOTH decision buttons: Skip or Make-a-fresh-copy fires C6 then `runStarted`
  // unmounts this screen, but a fast second click before that would fire C6 twice (two runs). A ref (not
  // `disabled`) keeps the buttons interactive; it resets on a C6 rejection (the run never started) so the user
  // can retry.
  const convertingRef = useRef(false);

  const runWith = (decision: RerunDecision): void => {
    if (convertingRef.current) {
      return;
    }
    convertingRef.current = true;
    // §2.5/§5.8: C6 with the chosen `RerunDecision` → `runStarted` → Converting. On rejection reset the guard +
    // re-throw to the §7.5.1 global frontend-error bridge (the TargetsScreen convert precedent).
    void runConversion(plan.set.id, plan.selected, plan.options, plan.destination, decision).catch(
      (error: unknown) => {
        convertingRef.current = false;
        throw error;
      },
    );
  };

  const onCancel = (): void => {
    // §5.2 row 6 "Cancel → back to Destination, held plan intact, NO conversion" — but ONLY while no decision is
    // committed. Once Skip/Make-a-fresh-copy has fired C6 (`convertingRef` set), the run is committed: cancelling
    // would return to Targets and let the still-pending `runStarted` land in `fromTargets` and teleport into
    // Converting, breaking that "no conversion" contract. So while a decision is in flight, Cancel/Esc is inert
    // (the run is starting; Converting — with its own Cancel — is the next screen). It re-enables if C6 rejects
    // (the `runWith` `.catch` resets the guard). Staying in `rerunPrompt` lets the pending `runStarted` resolve
    // correctly via `fromRerunPrompt` → Converting.
    if (convertingRef.current) {
      return;
    }
    dispatch({ type: "rerunCancel" });
  };

  return (
    <>
      {/* §5.3: the still-mounted-but-INERT Targets/Destination backdrop — rendered (not blank) behind the modal
          and made `inert` so it is out of tab order + non-interactive while the alertdialog is up (§5.6). */}
      <div inert>
        <TargetsScreen plan={plan} />
      </div>
      <RerunPrompt
        onSkip={() => runWith("skip")}
        onFreshCopy={() => runWith("freshCopy")}
        onCancel={onCancel}
      />
    </>
  );
}
