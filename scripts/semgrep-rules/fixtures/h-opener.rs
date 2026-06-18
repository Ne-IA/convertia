// PLANTED-POSITIVE armed canary for G29 — this file DELIBERATELY violates the named rule and
// MUST be flagged by it (the SAST self-test prelude asserts it). DO NOT "fix" it. This dir is L(-1).
// rule (h): convertia-opener-egress-non-constant (non-constant URL)
fn open_link(url: String) {
    opener::open(url);
}
