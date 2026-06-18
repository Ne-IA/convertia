// PLANTED-POSITIVE armed canary for G29 (engine-argv) — RESOLVED-SIDECAR-PATH spawns + SIBLING-ESCAPE
// shape (a hardened sibling next to an unhardened one), so the canary catches both the binary-literal
// silent-never-bind class AND the whole-fn over-suppression (fail-OPEN) class. DO NOT "fix". L(-1).
// engines/<engine>.rs matches the rule paths: include glob.
// rule (b5): convertia-imgworker-missing-magick-configure-path
use std::process::Command;

fn run_image(&self, policy: &str) {
    let _good = Command::new(self.engines.magick()).env("MAGICK_CONFIGURE_PATH", policy).arg("convert").env_clear();
    let _bad = Command::new(self.engines.magick()).arg("convert").env_clear();  // no MAGICK_CONFIGURE_PATH -> b5 MUST fire
}
