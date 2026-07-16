// src/strings/ui.ts -- the flat English UI-chrome string table (section 5.7).
//
// The single home for UI-CHROME strings (empty-state copy, confirm-gate labels, button text, About
// text, the mixed-drop refusal phrasing). Conversion-OUTCOME strings (failure section 2.8, lossy
// section 2.9) are owned by section 02 and pulled in verbatim, never re-homed here.
//
// v1 is English-only with NO i18n runtime (SSOT Principle 11 / section 5.7 / 6.10): this table is
// consumed directly behind named keys -- the "localization boundary" is a future-proofing
// convention, not a v1 capability, and no locale-switch framework is a dependency. G57
// (check-english-only) asserts every key resolves to a non-empty English value and that
// idle_reassurance carries its exact section 5.7 [DECIDED] text (the section 6.10 drift check).
//
// Component-specific chrome strings join this table as their components land (P3-P8); P1 seeds it
// with the one section 5.7 [DECIDED]-pinned key. [Build-Session-Entscheidung: P1.37]
export const ui = {
  // The section 5.2 Idle empty-state offline/privacy reassurance line -- a section 5.7 [DECIDED]
  // fixed string (the SSOT "Local, private & offline" promise). This is its SINGLE home: P8.17 only
  // references it for the Idle screen, never re-defines it; the exact text is drift-checked by G57.
  idle_reassurance: "All conversion happens locally, on your machine — nothing is ever uploaded.",

  // The section 5.3 DropZone (the section 5.2 Idle intake surface, P3.54). The primary drop-or-browse
  // surface labels the click-to-browse action (a native file drop is handled core-side, section 5.4 --
  // the label speaks to the mouse-drop and the click/keyboard browse equally); the secondary link is the
  // "choose a folder" affordance (both invoke C2a pick_for_intake, section 0.4.1). [Build-Session-Entscheidung: P3.54]
  dropzone_prompt: "Drop files here, or click to choose files",
  dropzone_choose_folder: "Or choose a folder",

  // ── P3.55 ──────────────────────────────────────────────────────────────────────────────────────────
  // The section 5.2 Collecting (state 2) indeterminate + throttled-count status region (section 5.6.1
  // role="status" landing element). Placeholders substituted by strings/format.ts (ui.ts stays a flat
  // English string map -- ui.test.ts pins every value to a non-empty string, so no function lives here).
  // [Build-Session-Entscheidung: P3.55]
  collecting_indeterminate: "Looking at your files…",
  collecting_scanning_one: "Scanning… 1 file so far",
  collecting_scanning_many: "Scanning… {count} files so far",
  collecting_cancel: "Cancel",

  // The section 5.2 Confirm gate (state 3) BatchSummary. The count line "{n} {FORMAT} files" is the section
  // 5.6 assertive summary source -- the SAME string drives the visible line and the SR announcement (one
  // source, no divergence): strings/format.ts joins the count with the skip tally for the announcement.
  // Singular/plural per section 5.6. [Build-Session-Entscheidung: P3.55]
  confirm_count_one: "1 {format} file",
  confirm_count_many: "{count} {format} files",
  // The passive skip tally (section 5.2 / section 1.4) -- always shown when >=1 item was skipped, never
  // blocks confirm, never silent. Wording matches the section 5.6 assertive pattern verbatim.
  confirm_skip_tally_one: "1 file wasn't recognized and will be skipped",
  confirm_skip_tally_many: "{count} files weren't recognized and will be skipped",
  // The Confirm gate actions (section 5.10: Enter proceeds to Targets, Esc cancels back to Idle).
  confirm_continue: "Continue",
  confirm_cancel: "Cancel",

  // The section 5.3 FileList disclosure ("Show N files", section 5.10) + its skipped-row reason labels.
  // N is the total listed count (eligible + skipped rows). Singular/plural per section 5.6.
  // [Build-Session-Entscheidung: P3.55]
  filelist_show_one: "Show 1 file",
  filelist_show_many: "Show {count} files",
  filelist_hide_one: "Hide 1 file",
  filelist_hide_many: "Hide {count} files",
  // The accessible name of the expanded, keyboard-focusable (arrow/Page-scroll) virtualised list region (§5.6
  // — a scrollable region with non-focusable rows must be focusable so a keyboard/AT user can scroll it).
  filelist_region_label: "Collected files",
  // A skipped row's reason line (section 5.3 "skipped rows visually marked with their reason"). The word
  // "Skipped" makes the state TEXTUAL, not colour-alone (section 5.6 -- nothing critical by colour alone).
  filelist_skip_row: "Skipped — {reason}",
  // The confirm-gate skip-reason labels (section 5.7: confirm-gate labels are UI chrome, owned here) -- the
  // short per-SkipReason names; the full section 2.8 outcome sentences are the section 02-owned Summary
  // strings (state 8), not re-homed here. The four detection-ineligible classes appear at the freeze;
  // "alreadyConverted" is a C6 re-run skip (never a confirm-gate SkippedItem) but is mapped for the
  // exhaustive SkipReason switch. [Build-Session-Entscheidung: P3.55]
  skip_reason_unsupported_type: "Unsupported type",
  skip_reason_uncertain: "Couldn't identify this file",
  skip_reason_empty: "Empty file",
  skip_reason_unreadable: "Couldn't read this file",
  skip_reason_already_converted: "Already converted",
  // Appends the retained detected-type name when the skipped item carried one (section 0.6 detectedDisplay).
  skip_reason_detected: "{label} — detected: {detected}",

  // ── P3.56 ──────────────────────────────────────────────────────────────────────────────────────────
  // The section 5.3 FormatPicker (the section 5.2 Targets state 4) heading. The offered target tiles carry
  // their own backend-supplied label (section 0.6 Target.label, from C3 -- never re-homed here); this is only
  // the group's contextual heading. [Build-Session-Entscheidung: P3.56]
  formatpicker_heading: "Convert to",

  // The section 5.3 DestinationBar (state 5). "Will save to {dir}" renders the C4 plan's finalDirDisplay (a
  // core-produced lossy display string, section 2.10.1 -- never a re-submittable path); the Change/Convert
  // button labels. Chrome (section 5.7: destination + button text owned here); the plan itself is section
  // 1.8/2.7-owned. [Build-Session-Entscheidung: P3.56]
  destination_will_save_to: "Will save to {dir}",
  destination_change: "Change destination",
  destination_convert: "Convert",
  // The section 5.2 row-4 "Back button" (Targets -> Confirm, preserving the frozen set) the machine's `back`
  // arm references. The section 5.10 Ctrl/Cmd+Backspace accelerator that also drives it is P4.70.3 (keyboard a11y).
  targets_back: "Back",

  // The section 2.7.2 per-location divert note -- shown under the will-save-to line when the C4 plan diverted
  // (OutputPlanPreview.diverted is Some). Chrome (section 5.7 line 825: "divert noted", string owned here); it
  // explains WHY the output moved to the shown safe folder. One line per section 0.6 DivertReason variant.
  // [Build-Session-Entscheidung: P3.56]
  destination_divert_unwritable:
    "The original folder can't be written to, so it's being saved here instead.",
  destination_divert_ephemeral:
    "The original folder is temporary, so it's being saved here instead.",
  destination_divert_no_atomic_publish:
    "The original folder can't safely store the result, so it's being saved here instead.",

  // The section 5.8:926 persisted-destination FALLBACK note -- shown when the C14 get_initial_destination hand-off
  // reported the saved lastDestinationMode path failed re-validation (gone/read-only/ephemeral) and fell back to
  // beside-source. Chrome (section 5.7:825, string owned here); surfaced EVEN when beside-source is writable (only
  // the resolver knows the fallback happened -- the G1 Opus-P2 adoption). [Build-Session-Entscheidung: P3.56]
  destination_persisted_fallback:
    "Your saved destination folder isn't available, so files will be saved beside each source.",

  // ── P3.57 ──────────────────────────────────────────────────────────────────────────────────────────
  // The section 5.3 RerunPrompt (the section 5.2 RerunPrompt state 6) -- the one batch-level section 2.5 re-run
  // decision modal. The heading + body are the two distinct section 5.6(f) [DECIDED] strings (a short heading +
  // a sentence body, intentionally not identical); the heading is the alertdialog's accessible name (WCAG 4.1.2,
  // aria-labelledby). The three control labels are the section 5.2 row-6 / section 5.6.1 decided controls:
  // Skip (the safe default) / Make a fresh copy / Cancel. Chrome (section 5.7: button + dialog text owned here);
  // the section 2.5 re-run LOGIC is section 02-owned. [Build-Session-Entscheidung: P3.57]
  rerun_heading: "Already converted with these settings",
  rerun_body: "You already converted these with the same settings.",
  rerun_skip: "Skip",
  rerun_fresh_copy: "Make a fresh copy",
  rerun_cancel: "Cancel",

  // ── P3.58 ──────────────────────────────────────────────────────────────────────────────────────────
  // The section 5.2 Converting screen (state 7 + the 7a Cancelling sub-state) — the ProgressList aggregate
  // bar + per-item row status labels + the Cancel affordance. The per-item Failed row also shows the item's
  // verbatim section 2.8 reason string (backend-rendered IpcError.message, section 5.7 -- section 02-owned,
  // NOT re-homed here); these are the surrounding chrome labels only. The aggregate "{done} of {total}" line
  // is composed in strings/format.ts (singular/plural on total). [Build-Session-Entscheidung: P3.58]
  converting_heading: "Converting",
  converting_cancel: "Cancel",
  converting_cancelling: "Cancelling…",
  converting_aggregate_one: "{done} of 1 file done",
  converting_aggregate_many: "{done} of {total} files done",
  converting_status_running: "Converting…",
  converting_status_succeeded: "Done",
  converting_status_failed: "Failed",
  converting_status_cancelled: "Cancelled",
  converting_status_skipped: "Skipped",

  // ── P3.59 ──────────────────────────────────────────────────────────────────────────────────────────
  // The section 5.2 Summary screen (state 8) — the ResultSummary + OpenActions CHROME (section 5.7: button
  // text + screen copy owned here). The per-item OUTCOME lines are NOT here: a row renders its
  // core-supplied OutcomeMsg.text verbatim (section 5.7:800 -- section 02-owned, never paraphrased), and the
  // section 2.8.2 BATCH-level summary line (all / partial / all-failed / cancelled + the section 2.6.4
  // with-residue tail) is NOT here either: the 2026-07-16 P3.59 ruling wired the core's existing
  // batch_summary_line onto RunResult.summaryLineDisplay, and the Summary renders that VERBATIM -- so the
  // fully-failed banner is a section 5.2 row-8 PRESENTATION of a section 02-owned string, not a chrome
  // paraphrase of it (the pre-ruling fill authored one here and the G1 dual review rejected it against
  // section 5.7:799). [Build-Session-Entscheidung: P3.59]
  summary_heading: "Results",
  // The per-row outcome chrome: the status word (textual, never colour-alone -- section 5.6) + the
  // output->source mapping line (section 1.12 / SSOT How It Feels 7: every output maps back to its source).
  summary_status_succeeded: "Done",
  summary_status_failed: "Failed",
  summary_status_cancelled: "Cancelled",
  summary_status_skipped: "Skipped",
  summary_saved_as: "Saved as {output}",
  // The section 7.7 reveal affordance for a section 2.6.4 residue row (C9 Residue(ItemId)). The residue
  // LOCATION is NOT chrome: it arrives already rendered inside the item's section 02-owned reason line
  // (case 1 = the section 2.8.2 residue annotation, case 2 = the cleanup_residue row) and is shown verbatim,
  // so only this button label is owned here.
  summary_reveal_residue: "Reveal residue",
  // The section 5.3 OpenActions labels (section 5.3 [DECIDED]: real string entries, not placeholders) --
  // the single common-root button, or the split-divert PAIR + its connector line when the run diverted
  // (section 1.12 divertRootDisplay is Some). Backed by C9 open_path by OpenTarget id (section 7.7).
  // [Build-Session-Entscheidung: P3.59] section 5.3's [DECIDED] enumerates these keys unprefixed
  // (open_folder / open_source_folder / open_saved_to_folder / saved_to_connector). They ship
  // screen-prefixed, per this table's established convention (converting_status_* / confirm_count_* /
  // filelist_*) -- naming is the fill's call (roles section 4 "NOT escalation"), and the [DECIDED]'s
  // substance is honoured: these are REAL entries sharing the section 5.7 localization boundary, not
  // schematic bracket placeholders. open_file is legitimately absent -- the single-output "Open file"
  // button is P4.68's (this box's OpenActions clause omits it).
  summary_open_folder: "Open folder",
  summary_open_source_folder: "Open source folder",
  summary_open_saved_to_folder: "Open saved-to folder",
  summary_saved_to_connector: "Some files were saved to {dir}",
  // The section 5.2 row-8 "Convert more" -> Idle (also Ctrl/Cmd+N, section 5.10; the accelerator is P4.70.3).
  summary_convert_more: "Convert more",
} as const;
