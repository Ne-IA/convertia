// src/strings/format.ts — the §5.7 UI-chrome string FORMATTERS (the composition layer over `ui`).
//
// `strings/ui.ts` is a FLAT English string map (ui.test.ts pins every value to a non-empty string, so no
// function can live there); the dynamic §5.2/§5.3 confirm-gate strings ("48 CSV files", the skip tally, the
// "Scanning… N files" status, the skipped-row reason) are composed HERE by pure functions that read the `ui`
// templates and substitute their `{placeholder}` tokens. This keeps the single-source rule (§5.6 "the same
// string drives the visible line and the SR announcement — one source, no divergence") — the visible
// BatchSummary count line and the assertive announcement are BOTH built from `confirm_count_*` here — while
// leaving `ui` a lint-/G57-clean flat table. No JSX, no user-facing literal outside `ui`; the `—`/`·` glue is
// section-owned punctuation, not translatable copy. [Build-Session-Entscheidung: P3.55]
import type { DivertReason, JobState, SkipReason, UserFacingFormat } from "../lib/ipc/commands";

import { ui } from "./ui";

/** Substitute every `{key}` token in `template` with `vars[key]` (all occurrences). The template + its tokens
 *  are authored together in `ui`, so a missing key is an authoring bug, not a runtime branch. */
export function fill(template: string, vars: Readonly<Record<string, string | number>>): string {
  return template.replace(/\{(\w+)\}/g, (_match, key: string) =>
    key in vars ? String(vars[key]) : `{${key}}`,
  );
}

/** The §5.2 source-format display token for the confirm gate — the uppercased §0.6 `UserFacingFormat` (the
 *  walking-skeleton slice is CSV→TSV: `"csv"`→"CSV", `"tsv"`→"TSV"). A richer per-format label map (e.g.
 *  `threeGp`→"3GP", `md`→"Markdown") rides each format's own phase (P5–P7) when that format's UI lands;
 *  uppercasing is correct for the slice and the common case. [Build-Session-Entscheidung: P3.55] */
export function formatLabel(format: UserFacingFormat): string {
  return format.toUpperCase();
}

/** The §5.2 confirm-gate count line ("48 CSV files" / "1 CSV file") — the §5.6 assertive-summary source.
 *  Singular/plural per §5.6 (`confirm_count_one`/`confirm_count_many`). */
export function formatConfirmCount(count: number, format: UserFacingFormat): string {
  const template = count === 1 ? ui.confirm_count_one : ui.confirm_count_many;
  return fill(template, { count, format: formatLabel(format) });
}

/** The §5.2/§1.4 passive skip tally ("3 files weren't recognized and will be skipped"). Only rendered when
 *  `skippedCount >= 1` (the caller gates on non-empty `skipped`, so this always receives a positive count). */
export function formatSkipTally(skippedCount: number): string {
  const template = skippedCount === 1 ? ui.confirm_skip_tally_one : ui.confirm_skip_tally_many;
  return fill(template, { count: skippedCount });
}

/** The §5.6 assertive confirm-gate announcement: the count line, joined with the skip tally by ` — ` when any
 *  item was skipped (the exact §5.6 pattern "{n} {FORMAT} files — {m} file(s) weren't recognized…"). Built
 *  from the SAME `ui` templates as the visible line, so the two never diverge. */
export function formatConfirmAnnouncement(
  count: number,
  format: UserFacingFormat,
  skippedCount: number,
): string {
  const summary = formatConfirmCount(count, format);
  return skippedCount > 0 ? `${summary} — ${formatSkipTally(skippedCount)}` : summary;
}

/** The §5.2 Collecting status text — the indeterminate "Looking at your files…" until a throttled `onScan`
 *  count arrives, then "Scanning… N files so far". `scanned === null` = no count yet (§5.2 state 2). */
export function formatScanStatus(scanned: number | null): string {
  if (scanned === null) {
    return ui.collecting_indeterminate;
  }
  const template = scanned === 1 ? ui.collecting_scanning_one : ui.collecting_scanning_many;
  return fill(template, { count: scanned });
}

/** The §5.3 FileList disclosure label — "Show N files" collapsed, "Hide N files" expanded (§5.10). `total` is
 *  the count of listed rows (eligible + skipped). Singular/plural per §5.6. */
export function formatDisclosure(total: number, expanded: boolean): string {
  if (expanded) {
    return fill(total === 1 ? ui.filelist_hide_one : ui.filelist_hide_many, { count: total });
  }
  return fill(total === 1 ? ui.filelist_show_one : ui.filelist_show_many, { count: total });
}

/** The short confirm-gate label for a §0.6 `SkipReason` (§5.7: confirm-gate labels are UI chrome, owned in
 *  `ui`). Exhaustive over the five variants — the four detection-ineligible classes seen at the confirm gate
 *  plus `alreadyConverted` (a C6 re-run skip, never a confirm-gate `SkippedItem`, mapped for exhaustiveness). */
export function skipReasonLabel(reason: SkipReason): string {
  switch (reason) {
    case "unsupportedType":
      return ui.skip_reason_unsupported_type;
    case "uncertain":
      return ui.skip_reason_uncertain;
    case "empty":
      return ui.skip_reason_empty;
    case "unreadable":
      return ui.skip_reason_unreadable;
    case "alreadyConverted":
      return ui.skip_reason_already_converted;
    default:
      return assertNever(reason);
  }
}

/** A skipped row's full reason line (§5.3): the short label, with the retained detected-type name appended
 *  when the item carried one (§0.6 `detectedDisplay`, e.g. "Unsupported type — detected: PDF"), wrapped in the
 *  "Skipped — {reason}" frame so the skipped state is TEXTUAL (§5.6: nothing critical by colour alone). */
export function formatSkipRow(reason: SkipReason, detectedDisplay: string | null): string {
  const label = skipReasonLabel(reason);
  const reasonText =
    detectedDisplay !== null
      ? fill(ui.skip_reason_detected, { label, detected: detectedDisplay })
      : label;
  return fill(ui.filelist_skip_row, { reason: reasonText });
}

/** The §5.3 DestinationBar "will save to …" line — the C4 plan's `finalDirDisplay` (a core-produced lossy
 *  display string, §2.10.1) wrapped in the chrome frame. [Build-Session-Entscheidung: P3.56] */
export function formatWillSaveTo(finalDirDisplay: string): string {
  return fill(ui.destination_will_save_to, { dir: finalDirDisplay });
}

/** The §2.7.2 per-location divert note for a §0.6 `DivertReason` (§5.3/§5.7 chrome) — shown under the
 *  will-save-to line when the C4 plan diverted. Exhaustive over the three variants. [Build-Session-Entscheidung: P3.56] */
export function divertNote(reason: DivertReason): string {
  switch (reason) {
    case "unwritable":
      return ui.destination_divert_unwritable;
    case "ephemeral":
      return ui.destination_divert_ephemeral;
    case "noAtomicPublish":
      return ui.destination_divert_no_atomic_publish;
    default:
      return assertNever(reason);
  }
}

/** The §5.2/§1.11 Converting aggregate line ("1 of 2 files done" / "0 of 1 file done") — the queued-only
 *  `done`/`total` from the §0.4.2 `BatchProgress` (`total == RunStarted.total_items`, pre-flight skips excluded,
 *  §0.4.2). Singular/plural on `total` (`converting_aggregate_one`/`_many`). [Build-Session-Entscheidung: P3.58] */
export function formatBatchProgress(done: number, total: number): string {
  const template = total === 1 ? ui.converting_aggregate_one : ui.converting_aggregate_many;
  return fill(template, { done, total });
}

/** The §5.2 Summary (state 8) per-row status word for a §0.6 terminal `JobState` — TEXTUAL, so the outcome is
 *  never carried by colour alone (§5.6). GENUINELY exhaustive: every arm is discriminated until `state` narrows
 *  to `never`, so a new `JobState` variant fails to compile here rather than silently rendering a wrong word.
 *  The §2.8 REASON line beside this word is the core-supplied `OutcomeMsg.text`, rendered verbatim (§5.7).
 *
 *  The two NON-terminal variants (`pending`/`running`) are **unrepresentable in a §1.12 `RunResult`** ("at
 *  `RunFinished` always a terminal variant", §0.6) — so they THROW rather than render a label: a `RunResult`
 *  carrying one is a backend contract violation, and inventing an outcome for it would breach §5.2's
 *  "never fabricates per-item outcomes for items it never heard back about". [Build-Session-Entscheidung: P3.59] */
export function summaryStatusLabel(state: JobState): string {
  if (state === "succeeded") {
    return ui.summary_status_succeeded;
  }
  if (state === "cancelled") {
    return ui.summary_status_cancelled;
  }
  if (state === "pending" || state === "running") {
    throw new Error(`non-terminal JobState in a terminal RunResult (§0.6/§1.12): ${state}`);
  }
  if (state.skipped !== undefined) {
    return ui.summary_status_skipped;
  }
  if (state.failed !== undefined) {
    return ui.summary_status_failed;
  }
  return assertNever(state);
}

/** The §1.12 output→source mapping line for a succeeded row — "Saved as {output}" over the core-supplied
 *  `ItemResult.outputDisplay` (a lossy display string, §2.10.1; never a re-submittable path). The SOURCE half of
 *  the map is the row's own heading (the frozen set's `displayName`), so the pair reads source → output (SSOT
 *  *How It Feels* 7). [Build-Session-Entscheidung: P3.59] */
export function formatSavedAs(outputDisplay: string): string {
  return fill(ui.summary_saved_as, { output: outputDisplay });
}

/** The §5.3 split-divert connector line — "Some files were saved to {dir}" over the §1.12
 *  `RunResult.divertRootDisplay`, explaining WHY the run has two open-folder buttons (§2.7.3/§7.7.1).
 *  [Build-Session-Entscheidung: P3.59] */
export function formatSavedToConnector(divertRootDisplay: string): string {
  return fill(ui.summary_saved_to_connector, { dir: divertRootDisplay });
}

/** The §5.2 row-9 formats-found line ("Found 30 JPG, 12 PNG, 3 PDF") — the §1.3 mixed-drop tally, one entry per
 *  distinct eligible format over the wire's `[UserFacingFormat, count]` pairs, joined by ", " in the wire's own
 *  order (the core produced it; the UI does not re-rank the refusal, §5.2 "the backend is the source of truth
 *  for facts"). The `, ` glue is section-owned punctuation, not translatable copy (this module's header).
 *  [Build-Session-Entscheidung: P3.60] */
export function formatMixedFound(found: readonly (readonly [UserFacingFormat, number])[]): string {
  const list = found
    .map(([format, count]) => fill(ui.mixed_found_entry, { count, format: formatLabel(format) }))
    .join(", ");
  return fill(ui.mixed_found, { list });
}

/** The §5.2 row-10 Empty per-reason tally ("5 files, none convertible (3 unreadable, 2 unsupported)") — grouped
 *  client-side from `CollectedSet::Empty.skipped` on the §0.6 `SkipReason` (§5.3 "derived client-side"). Entries
 *  keep first-seen order (the wire's), and the total is the skipped count, so the sentence's "N files" and its
 *  breakdown always sum. Returns `null` for an empty list — the §5.2 all-hidden `Empty { skipped: [] }` case,
 *  which renders "the plain copy, no tally". [Build-Session-Entscheidung: P3.60] */
export function formatSkipBreakdown(
  skipped: readonly { readonly reason: SkipReason }[],
): string | null {
  if (skipped.length === 0) {
    return null;
  }
  const counts = new Map<SkipReason, number>();
  for (const { reason } of skipped) {
    counts.set(reason, (counts.get(reason) ?? 0) + 1);
  }
  const breakdown = [...counts]
    .map(([reason, count]) =>
      fill(ui.unsupported_tally_entry, { count, reason: tallyReasonWord(reason) }),
    )
    .join(", ");
  const template = skipped.length === 1 ? ui.unsupported_tally_one : ui.unsupported_tally_many;
  return fill(template, { count: skipped.length, breakdown });
}

/** The SHORT lowercase counted-noun for a §0.6 `SkipReason` inside the {@link formatSkipBreakdown} sentence
 *  ("3 unreadable") — distinct from {@link skipReasonLabel}'s capitalised confirm-gate ROW label. Exhaustive
 *  over the five variants (`alreadyConverted` is a C6 re-run skip, never a freeze-time `SkippedItem`, mapped for
 *  the exhaustive switch — the `skipReasonLabel` precedent). [Build-Session-Entscheidung: P3.60] */
function tallyReasonWord(reason: SkipReason): string {
  switch (reason) {
    case "unsupportedType":
      return ui.unsupported_tally_reason_unsupported_type;
    case "uncertain":
      return ui.unsupported_tally_reason_uncertain;
    case "empty":
      return ui.unsupported_tally_reason_empty;
    case "unreadable":
      return ui.unsupported_tally_reason_unreadable;
    case "alreadyConverted":
      return ui.unsupported_tally_reason_already_converted;
    default:
      return assertNever(reason);
  }
}

/** Exhaustiveness guard: a new variant reaching an exhaustive switch ({@link skipReasonLabel} /
 *  {@link divertNote} / {@link tallyReasonWord}) fails to compile (`value: never`), so a label can never be
 *  silently missing. Unreachable by construction. */
function assertNever(value: never): never {
  throw new Error(`unhandled union variant: ${String(value)}`);
}
