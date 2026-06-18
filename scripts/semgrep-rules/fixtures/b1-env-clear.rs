// PLANTED-POSITIVE armed canary for G29 — DELIBERATELY violates the named rule and MUST be flagged.
// SIBLING-ESCAPE shape: a correctly-hardened (fluent) builder sits next to an UNhardened one in the
// SAME fn. The rule MUST fire on the UNhardened sibling and NOT on the fluent-hardened one — so the
// canary turns RED if a not-inside ever over-suppresses across builders (the r2/r3 fail-OPEN class).
// DO NOT "fix". L(-1).
// rule (b1): convertia-command-missing-env-clear
use std::process::Command;

fn spawn_engines() {
    let _hardened = Command::new(probe_bin()).env_clear().arg("--version");  // fluent-scrubbed sibling -> suppressed
    let _ = Command::new(engine_bin()).arg("-i").arg("input.mp4");           // UNSCRUBBED -> b1 MUST fire
}
