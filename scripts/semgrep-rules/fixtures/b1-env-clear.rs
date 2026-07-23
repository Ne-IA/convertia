// PLANTED-POSITIVE armed canary for G29 — DELIBERATELY violates the named rule and MUST be flagged.
// SIBLING-ESCAPE shape: a correctly-hardened (fluent) builder sits next to an UNhardened one in the
// SAME fn. The rule MUST fire on the UNhardened sibling and NOT on the fluent-hardened one — so the
// canary turns RED if a not-inside ever over-suppresses across builders (the r2/r3 fail-OPEN class).
// P4.85 refinement pins: the ADJACENT split-builder scrub-FIRST pair is clean (`sast-clean` markers —
// the check-sast prelude FAILS if a marked line fires, so a broken suppression is armed, not hoped);
// a GAPPED split, a wrong-receiver scrub, and a sibling spawn AFTER a clean pair all still fire —
// each pinned per-line via `sast-must-fire` markers (the prelude FAILS if a pinned positive stops
// firing: the fail-OPEN direction the set-level canary cannot see).
// DO NOT "fix". L(-1).
// rule (b1): convertia-command-missing-env-clear
use std::process::Command;

fn spawn_engines() {
    let _hardened = Command::new(probe_bin()).env_clear().arg("--version");  // fluent-scrubbed sibling -> suppressed
    let _ = Command::new(engine_bin()).arg("-i").arg("input.mp4");           // UNSCRUBBED -> MUST fire // sast-must-fire: convertia-command-missing-env-clear
}

fn split_builder_scrub_first_is_clean() {
    let mut ok = Command::new(probe_bin()); // sast-clean: convertia-command-missing-env-clear
    ok.env_clear();
    ok.arg("--version");
    let _ = Command::new(engine_bin()).arg("-i");    // sibling AFTER the clean pair // sast-must-fire: convertia-command-missing-env-clear
}

fn split_builder_with_a_gap_still_fires() {
    let mut gapped = Command::new(engine_bin());     // GAPPED split, over-strict by design // sast-must-fire: convertia-command-missing-env-clear
    gapped.arg("-i");
    gapped.env_clear();
}

fn split_builder_wrong_receiver_still_fires() {
    let mut other = make_other();
    let mut bad = Command::new(engine_bin());        // scrubs a DIFFERENT receiver ($C unification) // sast-must-fire: convertia-command-missing-env-clear
    other.env_clear();
}

fn qualified_split_scrub_first_is_clean() {
    let mut q = std::process::Command::new(probe_bin()); // sast-clean: convertia-command-missing-env-clear
    q.env_clear();
}
