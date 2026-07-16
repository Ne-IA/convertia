// src/components/ResultSummary.tsx — the §5.3 ResultSummary: the §1.12 end-of-batch outcome (P3.59).
//
// Renders the terminal `RunResult`: a per-item row for EVERY `ItemResult` (§1.12 projects the in-run outcomes
// AND every skip — the four pre-flight detection classes + the §2.5.3 re-run skip — into one uniform `items`
// list, so nothing the user dropped is silently dropped), each carrying its terminal status word, the §1.12
// output→source map, and its §2.8 reason line. Plus the fully-failed banner (§5.2 row 8: never a quiet "done").
//
// STRING OWNERSHIP (§5.7:799) — the load-bearing rule here: EVERY user-facing outcome string on this screen is
// §02-owned, core-resolved, and rendered VERBATIM; this component authors none of it. That holds at both levels:
//   - per item — `ItemResult.reason.text` (the §2.8/§2.9/§2.6.4 line `crate::outcome` already substituted);
//   - per batch — `RunResult.summaryLineDisplay` (the §2.8.2 situation row + the §2.6.4 "With residue" tail,
//     assembled core-side by `batch_summary_line`).
// Only the surrounding CHROME is §5.7's: the status word, the "Saved as …" frame, the reveal-residue label, and
// the §5.2 row-8 decision to dress a fully-failed batch as an alert banner. The 2026-07-16 P3.59 ruling wired
// that batch line onto the wire precisely because its absence had forced an earlier fill to author the copy in
// chrome — which the G1 dual review rejected against §5.7:799.
//
// RESIDUE (§2.6.4, three cases — residue NEVER rewrites an item's terminal STATE; §2.6.2:827 / §2.1.3:197
// "annotated, **not an item failure**"). What each case ships, and what this component does with it:
//   - case 1 (Succeeded + undeletable tmp) → stays `succeeded`; `reason` carries the §2.8.2 NON-failure
//     `OutcomeMsg::Residue` annotation, which already names {path} (the P3.59 ruling promoted §2.6.4:944's own
//     authored copy into the catalog and gave it this carrier);
//   - case 2 (Failed + uncleaned partial) → `Failed`, `reason` = the `cleanup_residue` FAILURE row, also
//     naming {path};
//   - case 3 (Cancelled + wedged temp) → stays `cancelled`, `reason` is `None` — §2.6.4 authors no per-item
//     case-3 sentence (its "With residue" tail is BATCH-level and rides the summary line above).
// So the residue LOCATION always arrives inside the §02 text (cases 1/2) or not at all (case 3); this component
// renders no path line of its own. The `cleanup_incomplete` membership — orthogonal to the terminal state — is
// what keys the §7.7 reveal link (C9 `Residue(ItemId)`), and it is case 3's whole per-item surface.
//
// SLICE SCOPE (P3.59): the CommandError slot + the wider §2.8 edge-state copy framework are P4.69 (which
// SUPERSEDES this slice renderer, and owns the reason-slot rule for an item carrying BOTH a §2.9 lossy note and
// a §2.6.4 case-1 annotation — unreachable here, the slice emits no `Lossy`); the §5.6 Summary priority
// focus-on-entry + the assertive outcome announcement are P4.70.3/P4.75; virtualising the results list rides
// its own P4 box; visual polish is P8. [Build-Session-Entscheidung: P3.59]
import { useCallback, useId } from "react";

import { openResultTarget } from "../lib/ipc/events";
import type { ItemId, ItemResult, RunResult } from "../lib/ipc/commands";
import { formatSavedAs, summaryStatusLabel } from "../strings/format";
import { ui } from "../strings/ui";

export interface ResultSummaryProps {
  /** The terminal §1.12 `RunResult` the machine's `summary` state carries (§5.3 props = `RunResult`). */
  readonly result: RunResult;
  /** The §1.12 output→SOURCE half of the map: each `ItemId`'s frozen display name, derived by the
   *  SummaryScreen from the threaded `CollectedSet` (§1.12:1425 "`item` keys the output→source mapping against
   *  the CollectedSet"). Covers the whole §0.6-invariant-6 id space — eligible items AND pre-flight skips. */
  readonly sources: ReadonlyMap<ItemId, string>;
}

/** One §1.12 row. `residueDisplay` is the item's `CleanupResidue.residueDisplay` when the run left residue for
 *  it (§2.6.4), independent of its terminal state. [Build-Session-Entscheidung: P3.59] */
interface RowProps {
  readonly item: ItemResult;
  readonly sourceDisplay: string;
  readonly residueDisplay: string | null;
}

function ResultRow({ item, sourceDisplay, residueDisplay }: RowProps) {
  const labelId = useId();
  // Only a genuine §2.8 FAILURE gets the alarm token — a §2.9 lossy note, a §2.6.4 case-1 residue annotation
  // and a §1.12 skip are all calm, non-failure lines (§1.12 "skip ≠ fail"; §2.6.4 "the success stands").
  const isFailure = item.reason?.type === "failure";
  // §7.7: reveal the recorded residue location by ID (C9 `Residue(ItemId)` → `reveal_item_in_dir`, never a
  // launch). Fire-and-forget — a refusal/rejection surfaces via the §7.5.1 bridge (the OpenActions precedent).
  const revealResidue = useCallback((): void => {
    void openResultTarget({ residue: item.item });
  }, [item.item]);

  return (
    <li className="flex flex-col gap-1" aria-labelledby={labelId}>
      {/* The SOURCE half of the §1.12 map — the frozen set's display name for this `ItemId` (§2.10.1 lossy). */}
      <span id={labelId} className="text-base text-text">
        {sourceDisplay}
      </span>
      {/* Textual status, never colour-alone (§5.6). */}
      <span className="text-sm text-text-muted">{summaryStatusLabel(item.state)}</span>
      {/* The OUTPUT half — present iff the item succeeded (§1.12: `outputDisplay` is `Some` only then). */}
      {item.outputDisplay !== null ? (
        <span className="text-sm text-text-muted">{formatSavedAs(item.outputDisplay)}</span>
      ) : null}
      {/* The §2.8/§2.9/§2.6.4 reason line — core-resolved, rendered VERBATIM (§5.7:799), never paraphrased.
          This ONE line covers every outcome incl. residue: a §2.6.4 case-1 item carries the §2.8.2 residue
          ANNOTATION (`OutcomeMsg::Residue` — the success stands) and a case-2 item the `cleanup_residue`
          FAILURE row; both already name {path}, so the UI adds no second path line of its own (which is also
          why a case-2 row no longer renders the location twice — the pre-ruling chrome note did). A skip's
          line is styled neutrally: skip ≠ fail (§1.12). */}
      {item.reason !== null ? (
        <p className={isFailure ? "text-sm text-danger" : "text-sm text-text-muted"}>
          {item.reason.data.text}
        </p>
      ) : null}
      {/* §7.7: the reveal affordance for a residue row — keyed off `cleanup_incomplete` membership (which spans
          ALL THREE §2.6.4 cases, incl. case 3, whose per-item surface is exactly this structural annotation +
          the batch-level tail; §2.6.4 authors no case-3 sentence, so its `reason` is legitimately null). */}
      {residueDisplay !== null ? (
        <button
          type="button"
          onClick={revealResidue}
          className="self-start rounded-md border border-border px-2 py-1 text-sm text-text"
        >
          {ui.summary_reveal_residue}
        </button>
      ) : null}
    </li>
  );
}

/** The §5.3 ResultSummary (slice renderer). [Build-Session-Entscheidung: P3.59] */
export function ResultSummary({ result, sources }: ResultSummaryProps) {
  const { totals } = result;
  // Whether to DRESS the §2.8.2 line as a failure banner (§5.2 row 8) — derived, never a stored field.
  //
  // [Build-Session-Entscheidung: P3.59] This mirrors the branch of `crate::orchestrator::batch_summary` that
  // PRODUCED the line being dressed (`cancelled == 0 && fail > 0 && ok == 0` → `BatchSummary::AllFailed`), NOT
  // §1.12's `Totals`-level "all failed" literal (`failed == total` over all FOUR tallies). The two are
  // different predicates on purpose and MUST NOT be swapped here: §2.8.2's AllFailed row is scoped to the
  // ATTEMPTED items — P3.50 deliberately excludes pre-flight skips from its `{n}` headline, since a skip never
  // entered the queue and is not a conversion outcome. Coupling §1.12's literal to §2.8.2's string silently
  // breaks the guarantee this banner exists for: a run of `{succeeded: 0, failed: 2, skipped: 1}` is AllFailed
  // to the core ("None of the 2 files could be converted.") while the literal reads 2 !== 3 → no banner, i.e. a
  // total failure rendered as calm body text — the "never a quiet done" violation (§5.2 row 8 / SSOT *Fail
  // clearly*). The predicate must match the classifier whose sentence it is dressing.
  const allFailed = totals.cancelled === 0 && totals.failed > 0 && totals.succeeded === 0;
  // §2.6.4: residue is keyed by `ItemId` and is orthogonal to the item's terminal state — a Succeeded, Failed
  // or Cancelled item can each carry one.
  const residues = new Map<ItemId, string>(
    result.cleanupIncomplete.map((residue) => [residue.item, residue.residueDisplay]),
  );

  return (
    <div className="flex flex-col gap-4">
      {/* The §1.12 batch-level summary line — core-assembled (`batch_summary_line` → the wire's
          `summaryLineDisplay`) and rendered VERBATIM: the §2.8.2 situation row for this run's totals + the
          §2.6.4 "With residue" tail when any residue survived. §5.2 row 8 / §5.7:831: a FULLY-FAILED batch is
          dressed as a clear failure banner (`role="alert"` + the failure token), never a quiet "done" — but the
          PRESENTATION is this component's and the STRING is §02's, so the UI never authors the copy (the
          pre-ruling chrome banner is removed; the 2026-07-16 P3.59 ruling). */}
      {allFailed ? (
        <p role="alert" className="text-base font-semibold text-danger">
          {result.summaryLineDisplay}
        </p>
      ) : (
        <p className="text-base text-text">{result.summaryLineDisplay}</p>
      )}
      <ul className="flex flex-col gap-3">
        {result.items.map((item) => (
          <ResultRow
            key={item.item}
            item={item}
            // A `RunResult` item is always a member of the frozen set (§0.6 invariant 6), so the lookup resolves;
            // the `?? ""` is the never-taken defensive arm — an unnamed row rather than a fabricated name.
            sourceDisplay={sources.get(item.item) ?? ""}
            residueDisplay={residues.get(item.item) ?? null}
          />
        ))}
      </ul>
    </div>
  );
}
