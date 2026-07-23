// PLANTED-POSITIVE armed canary for G29 — DELIBERATELY violates the named rule and MUST be flagged.
// P4.85 refined rule (d): paths-scoped to the macOS isolation module; this fixture file is
// explicitly bound in the rule's `paths: include` (the engines/-fixture precedent) so the armed
// canary can fire. PER-PATH pins: a bare staging call does NOT cover a later spawn; staging ONE
// path does NOT cover a spawn arg'ing a DIFFERENT one ($S unification); any gap between staging
// and spawn/build breaks the suppression (the r2/r3 fail-OPEN region class stays closed). The
// staged shapes are pinned clean via `sast-clean` markers, and every MUST-fire positive is pinned
// per-line via `sast-must-fire` markers — the check-sast prelude FAILS if a marked-clean line
// fires OR a pinned positive stops firing, so both directions are armed, not hoped.
// DO NOT "fix". L(-1).
// rule (d): convertia-macos-command-missing-stage-for-tcc
use std::process::Command;

fn run_on_macos(input: &str) {
    stage_for_tcc(input);                                              // a bare staging CALL must NOT blanket-cover the spawn below
    let _ = Command::new(engine_bin()).arg("/some/path").env_clear();  // spawned arg is NOT the staged result // sast-must-fire: convertia-macos-command-missing-stage-for-tcc
}

fn staged_inline_is_clean(input: &str) {
    let _ = Command::new(engine_bin()).env_clear().arg(stage_for_tcc(input)); // sast-clean: convertia-macos-command-missing-stage-for-tcc
}

fn staged_adjacent_split_is_clean(input: &str) {
    let staged = stage_for_tcc(input);
    let _ = Command::new(engine_bin()).env_clear().arg(&staged); // sast-clean: convertia-macos-command-missing-stage-for-tcc
}

fn staged_then_built_split_is_clean(input: &str) {
    let staged = stage_for_tcc(input)?;
    let mut cmd = Command::new(engine_bin()); // sast-clean: convertia-macos-command-missing-stage-for-tcc
    cmd.env_clear();
    cmd.arg(&staged);
}

fn staged_but_different_path_still_fires(input: &str, other: &str) {
    let staged = stage_for_tcc(input);
    let _ = Command::new(engine_bin()).env_clear().arg(other);   // `other` is not the staged binding ($S unification) // sast-must-fire: convertia-macos-command-missing-stage-for-tcc
}

fn staged_with_a_gap_still_fires(input: &str) {
    let staged = stage_for_tcc(input);
    log_stage(&staged);
    let _ = Command::new(engine_bin()).env_clear().arg(&staged); // a statement between staging and spawn breaks adjacency // sast-must-fire: convertia-macos-command-missing-stage-for-tcc
}
