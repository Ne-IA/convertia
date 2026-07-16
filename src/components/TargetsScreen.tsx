// src/components/TargetsScreen.tsx — the §5.2 Targets + Destination screen (states 4/5, folded) (P3.56).
//
// Composes the §5.3 FormatPicker (target tiles) + DestinationBar (will-save-to + Change + Convert) over the held
// `Planned`, and OWNS the state-4/5 out-transitions (the P3 screen-box wiring model — a rendered action MUST fire
// its command):
//   - tile select → `selectTarget` (optimistic highlight) + C4 re-plan (`replanOutput`);
//   - Change → C2b `pick_destination` → C5 `set_destination` (`pickAndSetDestination`; a cancelled pick is a no-op);
//   - Convert → a §2.5 rerun verdict shows the RerunPrompt (dispatch `convert`, state 6) else fires C6
//     (`runConversion`, decision `skip`) → `runStarted` → Converting (state 7);
//   - Back → Confirm preserving the frozen set (`back`, §5.2 row 4).
// A C3/C4/C5/C6 rejection re-throws to the §7.5.1 global frontend-error bridge (the ConfirmScreen `advanceToTargets`
// precedent) — the §5.3 `CommandError` inline slot rides P4.69, and the §5.10 accelerators (Ctrl/⌘+N cancel-to-Idle
// / Ctrl/⌘+Backspace back) + full keyboard/focus a11y ride P4.70.3. The screens the transitions reach that are not
// yet built (RerunPrompt P3.57, Converting P3.58) render as an empty workspace until their box lands — never a
// dead button, because the transition INTO each is wired here. [Build-Session-Entscheidung: P3.56]
import { useRef } from "react";

import { pickAndSetDestination, replanOutput, runConversion } from "../lib/ipc/events";
import type { TargetId } from "../lib/ipc/commands";
import type { Planned } from "../state/machine";
import { useAppStore } from "../state/store";
import { ui } from "../strings/ui";

import { DestinationBar } from "./DestinationBar";
import { FormatPicker } from "./FormatPicker";

export interface TargetsScreenProps {
  /** The §5.2 states-4/5 held plan (offer + selected target + options + destination + the last C4 preview). */
  readonly plan: Planned;
}

/**
 * The §5.2 Targets + Destination screen. [Build-Session-Entscheidung: P3.56]
 */
export function TargetsScreen({ plan }: TargetsScreenProps) {
  const dispatch = useAppStore((state) => state.dispatch);
  const setId = plan.set.id;
  // Guards a double-convert: firing C6 then dispatching `runStarted` unmounts this screen, but a fast second
  // Convert before that would fire C6 twice (two runs). A ref (not `disabled`) keeps the button interactive; it
  // resets on a C6 rejection (the run never started) and on remount (rerunCancel → Targets is a fresh instance).
  const convertingRef = useRef(false);

  const onSelect = (target: TargetId): void => {
    // §5.8: optimistic tile highlight (immediate), then C4 re-plan for the new target (refreshes the preview).
    dispatch({ type: "selectTarget", target });
    void replanOutput(setId, target, plan.options, plan.destination);
  };

  const onChangeDestination = (): void => {
    // §5.4/§5.8: C2b `pick_destination` → C5 `set_destination` → `destinationResolved`; a cancelled pick is a no-op.
    void pickAndSetDestination(setId, plan.selected, plan.options);
  };

  const onConvert = (): void => {
    if (convertingRef.current) {
      return;
    }
    if (plan.preview.rerun !== null) {
      // §2.5 / §5.2 state 6: an equivalent prior run → the RerunPrompt (P3.57 renders it) BEFORE convert. No C6
      // fires here (P3.57 fires it with the chosen RerunDecision); the dispatch unmounts this screen.
      dispatch({ type: "convert" });
      return;
    }
    // No rerun → fire C6 directly (decision `skip` is moot with no equivalent items, §2.5) → `runStarted` → Converting.
    convertingRef.current = true;
    void runConversion(setId, plan.selected, plan.options, plan.destination, "skip").catch(
      (error: unknown) => {
        // The run never started — reset the guard so the user can retry; re-throw to the §7.5.1 global bridge.
        convertingRef.current = false;
        throw error;
      },
    );
  };

  const onBack = (): void => {
    // [Build-Session-Entscheidung: P3.57] Prevention-sweep sibling of the RerunScreen Cancel guard: once the
    // no-rerun Convert has fired C6 (`convertingRef` set), the run is COMMITTED — a Back here returns to Confirm,
    // where a pending `runStarted` is SILENTLY DROPPED (`fromConfirm` has no `runStarted` arm) and the UI would
    // sit at the Confirm gate while the run is live. So while a convert is in flight, Back is inert (Converting —
    // driven by `runStarted` — is the next screen). It re-enables if C6 rejects (the `.catch` above resets the
    // guard); the rerun path never sets `convertingRef` here (it dispatches `convert` and returns), so Back stays
    // live there. Same defect class fixed at both C6-firing sites (§5.2 row 4 / row 6).
    if (convertingRef.current) {
      return;
    }
    dispatch({ type: "back" });
  };

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6 p-8">
      <FormatPicker targets={plan.offer.targets} selected={plan.selected} onSelect={onSelect} />
      <DestinationBar
        preview={plan.preview}
        persistedFallback={plan.persistedFallback}
        onChangeDestination={onChangeDestination}
        onConvert={onConvert}
      />
      <button type="button" onClick={onBack} className="self-start text-accent underline">
        {ui.targets_back}
      </button>
    </div>
  );
}
