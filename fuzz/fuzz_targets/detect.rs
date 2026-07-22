// fuzz/fuzz_targets/detect.rs — the G48 `detect` in-core untrusted-byte target (P3.73).
//
// §1.2 content sniffing runs IN-CORE, outside the §2.12 isolation boundary (it is the first code to touch
// untrusted bytes), so a panic/OOM/UB there lands in the trust kernel (security principle 9). This target
// feeds arbitrary bytes to the full sniff chain via `convertia_core::fuzz_api::detect` — the ONLY door a
// fuzz target uses (never a `crate::detection::*` tier path, P3.73). The invariant is "no panic / abort /
// OOM on arbitrary input" (§6.4.2); the byte→result mapping is fuzz_api's, so this file stays a thin body.
// Instrumented on the Linux (full ASAN) + macOS (sanitizer-less coverage-guided - the upstream aarch64-apple-darwin ASAN breakage, G48 row + gate-status 2026-07-22; the per-run canary re-arms) nightly legs; per-push it is the crate::fuzz_replay
// stable-toolchain replay of the committed corpus/crashes (P3.67).
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    convertia_core::fuzz_api::detect(data);
});
