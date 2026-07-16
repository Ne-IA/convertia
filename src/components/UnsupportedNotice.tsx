// src/components/UnsupportedNotice.tsx — the §5.3 UnsupportedNotice: the state-10 intake-refusal notice (P3.60).
//
// The §5.2 state-10 pre-flight outcome — a full-screen STATE, **not** a modal (§5.7:840: entered by a drop, no
// trigger to restore; assertive heading, no `role="alertdialog"`, no focus trap). FOUR §5.3 variants, each with
// its own copy path, so the `Empty` "nothing here I can convert" branch is never overlooked despite the
// component's unsupported-leaning name. It OWNS the state-10 out-transition (the P3 screen-box wiring model):
// **Dismiss** (the button, or **Esc**/**Enter** per §5.10:1231/:1241) → the machine's `dismiss` Msg → `Idle`.
//
// STRINGS: chrome WRAPPERS around §02-supplied payloads (§5.7:824's "§2.8 / here" split). The UI authors the
// four variant lines + the tally frame; the PAYLOADS ride inside verbatim — `{detected}` is detection's own
// retained type name (`CollectedSet::Unsupported.detected`) and the `Uncertain` note is the §1.2 can't-tell text
// (`CollectedSet::Uncertain.note`), rendered as its own calm secondary line so the payload is **never dropped**
// (§5.2 row 10). No §2.8.2 catalog row is re-authored here: the per-item conversion-outcome table is the §1.12
// Summary's surface, not this collection-level pre-flight screen.
//
// SLICE SCOPE (P3.60): the §5.3:307 `[DECIDED]` focus-on-entry (the Dismiss BUTTON, **not** the heading — a
// `tabindex=-1` heading is a no-op for Enter in most browsers, so focusing it would strand the keyboard user)
// ships here; the focus landing on the DropZone in the NEW `Idle` after dismiss (§5.10:1231) is the Idle
// DropZone's own focus-on-entry, P4.70.1's contract. Visual polish is P8; the copy refinement is P8.19.1.
// [Build-Session-Entscheidung: P3.60]
import { useEffect, useRef } from "react";

import type { SkippedItem } from "../lib/ipc/commands";
import type { UnsupportedReason } from "../state/machine";
import { useAppStore } from "../state/store";
import { fill, formatSkipBreakdown } from "../strings/format";
import { ui } from "../strings/ui";

export interface UnsupportedNoticeProps {
  /** The §5.2 state-10 payload from the machine — the §1.2/§1.3 projection of the non-`Single` `CollectedSet`
   *  arms (`unsupported` / `uncertain` / `empty`). The §5.3 fourth variant (`Unreadable`) is DERIVED from it —
   *  see {@link resolveVariant}. */
  readonly reason: UnsupportedReason;
}

/** The four §5.3 render variants + the chrome line each resolves to, plus the optional §02-supplied secondary
 *  payload line and the §5.2 row-10 Empty tally. [Build-Session-Entscheidung: P3.60] */
interface ResolvedVariant {
  /** The variant's chrome heading line. */
  readonly heading: string;
  /** The §1.2 `Uncertain.note` — a §02-supplied calm secondary line, rendered VERBATIM; `null` for the others. */
  readonly note: string | null;
  /** The §5.2 row-10 per-reason tally for the `Empty` case; `null` when there is none (incl. the all-hidden
   *  `Empty { skipped: [] }` case, which renders "the plain copy, no tally"). */
  readonly tally: string | null;
}

/** Resolve the machine's three-arm §5.2 payload onto the FOUR §5.3 render variants.
 *
 *  [Derived-Assumption: P3.60 — the §5.3:307 `Unreadable` variant has no wire arm of its own; it is derived from
 *  an `Empty` whose skips are ALL `unreadable`. Source: §5.2 row 10, which enumerates state-10's entry conditions
 *  as "`CollectedSet::Unsupported` … or `Uncertain` … **or every collected item was unreadable/gone**, **or** a
 *  `CollectedSet::Empty { skipped }`" — the third condition is not a distinct arm (the §0.6 `CollectedSet` union
 *  has none, and the machine's `UnsupportedReason` mirrors that with three kinds), so an all-unreadable `Empty` IS
 *  that condition. §5.3:307 then requires it render its own "couldn't read these files" copy rather than the
 *  generic Empty line. The freeze maps an unreadable/gone item to `SkipReason::unreadable` (§0.6 — there is no
 *  `gone` SkipReason), so that reason set is the test.] */
export function resolveVariant(reason: UnsupportedReason): ResolvedVariant {
  if (reason.kind === "unsupported") {
    return {
      heading: fill(ui.unsupported_heading_unsupported, { detected: reason.detected }),
      note: null,
      tally: null,
    };
  }
  if (reason.kind === "uncertain") {
    // §5.2 row 10 / §5.3:307: the §1.2 reason rides as a calm secondary line so the payload is never dropped.
    return { heading: ui.unsupported_heading_uncertain, note: reason.note, tally: null };
  }
  const { skipped } = reason;
  return {
    heading: isAllUnreadable(skipped)
      ? ui.unsupported_heading_unreadable
      : ui.unsupported_heading_empty,
    note: null,
    tally: formatSkipBreakdown(skipped),
  };
}

/** True iff the §5.2 row-10 "every collected item was unreadable/gone" condition holds — a NON-empty skip set
 *  whose every reason is `unreadable`. An `Empty { skipped: [] }` (the all-hidden drop) is NOT unreadable: it is
 *  the plain "nothing here I can convert" case, so the vacuous-truth arm is excluded deliberately. */
function isAllUnreadable(skipped: readonly SkippedItem[]): boolean {
  return skipped.length > 0 && skipped.every((item) => item.reason === "unreadable");
}

/** The §5.3 UnsupportedNotice (§5.2 state 10). [Build-Session-Entscheidung: P3.60] */
export function UnsupportedNotice({ reason }: UnsupportedNoticeProps) {
  const dispatch = useAppStore((state) => state.dispatch);
  const dismissRef = useRef<HTMLButtonElement>(null);
  const { heading, note, tally } = resolveVariant(reason);

  // §5.3:307 `[DECIDED]` focus-on-entry: the Dismiss BUTTON (so Enter activates it, §5.10:1241) — never the
  // heading, which is announced via its own live region without needing focus.
  useEffect(() => {
    dismissRef.current?.focus();
  }, []);

  // §5.10:1231: Esc dismisses → Idle. Enter needs no binding — focus is on Dismiss, so native <button>
  // activation already satisfies §5.10:1241's "Enter → dismiss".
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
      {/* §5.6:716 / §5.6.1 row 10: the heading carries `aria-live="assertive"` AND `tabindex="-1"` — a
          PROGRAMMATIC focus target only ("announced, not focused"; focus lands on Dismiss, since focusing a
          heading would make Enter a no-op). It is the one of the three screens whose §5.6.1 row mandates the
          attribute; rows 9 and 12 do not. */}
      <h2 tabIndex={-1} aria-live="assertive" className="text-xl font-semibold text-text">
        {heading}
      </h2>
      {/* The §1.2 can't-tell text — §02-supplied, rendered VERBATIM (§5.7), never paraphrased. */}
      {note !== null ? <p className="text-base text-text">{note}</p> : null}
      {tally !== null ? <p className="text-base text-text-muted">{tally}</p> : null}
      <button
        ref={dismissRef}
        type="button"
        onClick={() => dispatch({ type: "dismiss" })}
        className="self-start rounded-md border border-border px-4 py-2 text-base text-text"
      >
        {ui.unsupported_dismiss}
      </button>
    </div>
  );
}
