// fuzz/fuzz_targets/csv_tsv.rs — the G48 in-core CSV/TSV native-engine target (P3.73).
//
// The §3.5.6 native CSV/TSV engine is the ONE sanctioned in-core decode path (`EngineProgram::
// InProcessNative`, it decodes no third-party C/C++ bytes), so it runs OUTSIDE a subprocess and a
// panic/OOM there lands in the core. Memory-safe Rust is not the same as panic/OOM-safe: gigabyte quoted
// fields, recursive quoting and interior NUL bytes are the adversarial shapes. This target feeds arbitrary
// bytes to both directions via `convertia_core::fuzz_api::csv_tsv_transform` (the byte-level `transform_bytes`
// entry into an in-memory sink). The invariant is "no panic; bounded output relative to input" (§6.4.2).
// Instrumented (ASAN on) on the Linux + macOS nightly legs; per-push it is the crate::fuzz_replay stable
// replay (P3.67). Targets import ONLY fuzz_api, P3.73.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    convertia_core::fuzz_api::csv_tsv_transform(data);
});
