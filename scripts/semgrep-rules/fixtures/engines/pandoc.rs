// PLANTED-POSITIVE armed canary for G29 (engine-argv) — RESOLVED-SIDECAR-PATH spawns + SIBLING-ESCAPE
// shape (a hardened sibling next to an unhardened one), so the canary catches both the binary-literal
// silent-never-bind class AND the whole-fn over-suppression (fail-OPEN) class. DO NOT "fix". L(-1).
// engines/<engine>.rs matches the rule paths: include glob.
// rule (b2): convertia-pandoc-unsafe-resource-path — absolute --resource-path literal
use std::process::Command;

fn run_pandoc(&self, scratch: &str) {
    let _good = Command::new(self.engines.pandoc()).arg("--resource-path").arg(scratch).env_clear();   // scratch var -> NOT flagged
    let _bad = Command::new(self.engines.pandoc()).arg("--sandbox").arg("--resource-path").arg("/etc/passwd").env_clear();  // absolute -> b2 MUST fire
}
