// fuzz/fuzz_targets/fs_guard_is_safe_output.rs — the G48 `fs_guard::is_safe_output` target (P3.73).
//
// §2.3.3 write-target link-safety runs over PATHs derived from untrusted input, in-core. This target feeds
// the raw libFuzzer bytes to `convertia_core::fuzz_api`, which owns the byte→OS-path conversion and calls
// the real `is_safe_output` with an empty frozen-source set (the fuzzed dimension is the PATH, not the
// membership test). The invariant is "no panic on arbitrary bytes; and NEVER `Ok(Safe)` on an
// UNRESOLVABLE target" — is_safe_output is the §2.3.3 no-clobber-onto-source verdict (resolved-identity
// equality with a frozen source FILE), NOT a dangerous-Windows-path validator, so with an empty frozen set
// it correctly returns `Ok(Safe)` for a merely-drive-relative path whose parent resolves; its bound-firing
// seed is therefore `nul_output_path` (an interior-NUL OUTPUT path → `InvalidInput`, the non-fallback reject
// arm → `Err`, never a silent Safe). The Windows dangerous-PATH classes (device/reserved/drive-relative/
// UNC/trailing) are the `resolve_identity` target's — their fixtures live under `.../fs_guard_resolve_identity/`
// (the 2026-07-21 P3.73 P0 ruling: the P0.4.3 gate prose had mis-attributed those classes to this fn).
// (§6.4.2). Instrumented (ASAN on) on the Linux + macOS nightly legs; per-push it is the crate::fuzz_replay
// stable replay (P3.67). Targets import ONLY fuzz_api, P3.73.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    convertia_core::fuzz_api::fs_guard_is_safe_output(data);
});
