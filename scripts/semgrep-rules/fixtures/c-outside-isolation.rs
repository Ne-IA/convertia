// PLANTED-POSITIVE armed canary for G29 — this file DELIBERATELY violates the named rule and
// MUST be flagged by it (the SAST self-test prelude asserts it). DO NOT "fix" it. This dir is L(-1).
// rule (c): convertia-command-outside-isolation (this file is NOT under isolation/)
use std::process::Command;

fn spawn_from_wrong_module() {
    let _c = Command::new("ffmpeg").env_clear();
}
