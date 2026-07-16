// src/components/MixedDropRefusal.tsx — the §5.3 MixedDropRefusal: the §1.3 hard pre-flight refusal (state 9) (P3.60).
//
// The §1.3 mixed-drop refusal: a full-screen STATE, **not** a modal (§5.7:840 — entered by a drop, with no
// trigger to restore; assertive heading, no `role="alertdialog"`, no focus trap). It lists the formats found +
// their counts and refuses the batch whole — there is deliberately **no** "just convert the JPGs" subset
// affordance in v1 (§5.2 row 9, parked).
//
// It OWNS the state-9 out-transitions (the P3 screen-box wiring model — a rendered action MUST fire its command):
//   - **re-drop** (the PRIMARY action): the §5.3 `[DECIDED]` active `DropZone`, so a fresh single-format drop/pick
//     goes straight back to `Collecting` WITHOUT a Dismiss-to-Idle round-trip. It is the SAME DropZone component
//     with its §5.8 disabled-while-Converting guard inert (state 9 is pre-flight — nothing is converting), and
//     with the §5.10:1211 global chords gated OFF (`bindGlobalAccelerators={false}`): the global Ctrl/⌘+O binds in
//     `Idle` ONLY, while state 9 re-drops via Enter/Space on the focused surface (native <button> activation).
//     The C2a picker it fires completes through the §7.8.1 funnel → the payload-less `app://intake` nudge → the
//     §5.8 drain, which routes state 9 through the machine's `redrop` arm (events.ts `consumeIntakeNudge`).
//   - **Dismiss** (the secondary action, also **Esc** per §5.10:1232) → the machine's `dismiss` Msg → `Idle`.
//
// STRINGS: chrome in full. §5.7:803 names "the mixed-drop refusal phrasing" as a UI-owned string and §5.7:823
// gives the row's owner as "here (chrome)"; `crate::outcome`'s one-string-one-home comment likewise refuses
// §2.8.2 homing for `MixedDrop` ("via the §5.2 pre-flight UI"), so this screen renders no §02 body. The
// found-formats line is composed from the wire's own `[format, count]` tally (§0.6), never re-ranked here.
//
// SLICE SCOPE (P3.60): the §5.3:306 `[DECIDED]` focus-on-entry (the re-drop DropZone, NOT the heading — a
// `tabindex=-1` heading would make Enter a no-op) ships here; the focus landing in the NEW `Idle` after Dismiss
// (§5.10:1232) is the Idle DropZone's own focus-on-entry, P4.70.1's contract. Visual polish is P8; the copy
// refinement is P8.19.1. [Build-Session-Entscheidung: P3.60]
import { useEffect } from "react";

import type { MixedFound } from "../state/machine";
import { useAppStore } from "../state/store";
import { formatMixedFound } from "../strings/format";
import { ui } from "../strings/ui";

import { DropZone } from "./DropZone";

export interface MixedDropRefusalProps {
  /** The §1.3 per-format `[UserFacingFormat, count]` tally from the machine's state-9 payload (§0.6
   *  `CollectedSet::Mixed.found`) — the formats-found line's source. */
  readonly found: MixedFound;
}

/** The §5.3 MixedDropRefusal (§5.2 state 9). [Build-Session-Entscheidung: P3.60] */
export function MixedDropRefusal({ found }: MixedDropRefusalProps) {
  const dispatch = useAppStore((state) => state.dispatch);

  // §5.10:1232: Esc is the secondary Dismiss → Idle (state 9 is a full-screen state, not a modal, so this is a
  // document-level binding — the CollectingScreen/ConfirmScreen Esc precedent — not a dialog-scoped handler).
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key === "Escape") {
        event.preventDefault();
        dispatch({ type: "dismiss" });
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [dispatch]);

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6 p-8">
      {/* §5.3:306: announced via its own assertive live region on entry — NOT focused (the focus goes to the
          re-drop DropZone, the primary action). */}
      <h2 aria-live="assertive" className="text-xl font-semibold text-text">
        {ui.mixed_heading}
      </h2>
      <p className="text-base text-text">{formatMixedFound(found)}</p>
      <p className="text-base text-text-muted">{ui.mixed_body}</p>
      <DropZone autoFocus bindGlobalAccelerators={false} />
      <button
        type="button"
        onClick={() => dispatch({ type: "dismiss" })}
        className="self-start rounded-md border border-border px-4 py-2 text-base text-text"
      >
        {ui.mixed_dismiss}
      </button>
    </div>
  );
}
