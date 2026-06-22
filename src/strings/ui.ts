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
} as const;
