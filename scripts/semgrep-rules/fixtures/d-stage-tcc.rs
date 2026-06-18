// PLANTED-POSITIVE armed canary for G29 — DELIBERATELY violates the named rule and MUST be flagged.
// SIBLING-ESCAPE shape: a correctly-hardened (fluent) builder sits next to an UNhardened one in the
// SAME fn. The rule MUST fire on the UNhardened sibling and NOT on the fluent-hardened one — so the
// canary turns RED if a not-inside ever over-suppresses across builders (the r2/r3 fail-OPEN class).
// DO NOT "fix". L(-1).
// rule (d): convertia-macos-command-missing-stage-for-tcc (no per-fn suppression — fires on every spawn)
use std::process::Command;

fn run_on_macos(input: &str) {
    stage_for_tcc(input);                                  // staging ONE path must NOT blanket-cover the spawn below
    let _ = Command::new(engine_bin()).arg("/some/path").env_clear();  // (d) MUST still fire here
}
