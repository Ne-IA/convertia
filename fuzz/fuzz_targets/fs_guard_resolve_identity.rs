// fuzz/fuzz_targets/fs_guard_resolve_identity.rs — the G48 `fs_guard::resolve_identity` target (P3.73).
//
// §2.3.1 identity resolution runs over PATHs that ultimately derive from untrusted WebView/OS drag-drop
// input, in-core. This target feeds the raw libFuzzer bytes to `convertia_core::fuzz_api`, which owns the
// byte→OS-path conversion (`bytes_to_path` — BYTE-FAITHFUL on Unix so an overlong / interior-NUL / symlink
// path is not rewritten before it reaches the guard) and calls the real `resolve_identity`. The invariant
// is "no panic on arbitrary bytes; structured `Err` on the hostile classes, never `Ok` on a null-byte path"
// (T7+T2a, §6.4.2). The committed `fuzz/corpus/fs_guard_resolve_identity/` seed corpus carries the
// deterministic bound-firing fixtures: nul_path, path_max_plus_1, and the five Windows dangerous-path class
// seeds (device / reserved / drive-relative / UNC / trailing) re-homed here by the 2026-07-21 P3.73 P0
// ruling — those hostile-PATH classes are the untrusted-path fn's, not is_safe_output's. Instrumented (ASAN on) on the Linux +
// macOS nightly legs; per-push it is the crate::fuzz_replay stable replay (P3.67). Targets import ONLY
// fuzz_api (never a `crate::fs_guard` / `std::path` path), P3.73.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    convertia_core::fuzz_api::fs_guard_resolve_identity(data);
});
