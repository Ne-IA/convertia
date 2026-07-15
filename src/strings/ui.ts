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
} as const;
