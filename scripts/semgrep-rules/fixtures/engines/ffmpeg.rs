// PLANTED-POSITIVE armed canary for G29 (engine-argv) — RESOLVED-SIDECAR-PATH spawns + SIBLING-ESCAPE
// shape (a hardened sibling next to an unhardened one), so the canary catches both the binary-literal
// silent-never-bind class AND the whole-fn over-suppression (fail-OPEN) class. DO NOT "fix". L(-1).
// engines/<engine>.rs matches the rule paths: include glob.
// rule (b4): convertia-ffmpeg-missing-protocol-whitelist
use std::process::Command;

fn run_ffmpeg(&self) {
    let _good = Command::new(self.engines.ffmpeg()).args(["-protocol_whitelist","file,pipe","-i","ok"]).env_clear();
    let _bad = Command::new(self.engines.ffmpeg()).arg("-i").arg("input.mkv").env_clear();  // no -protocol_whitelist -> b4 MUST fire
}
