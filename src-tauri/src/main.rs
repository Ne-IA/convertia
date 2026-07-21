//! ConvertIA — the thin Tauri v2 host-binary entry (P3.87 bin+lib split, §0.7).
//!
//! The crate root is `src-tauri/src/lib.rs` (the `convertia_core` LIB target): every §0.7 tier module,
//! the crate lint policy, the §0.4.5 codegen seam and the app entry body live there, so the P0.4.3
//! in-core G48 fuzz targets can import the crate under test through a path dependency (a path dependency
//! resolves the LIB target only — the P3.73 lib-target precondition, fork ① of the 2026-07-21 ruling).
//! This file is the standard Tauri-v2 thin-bin shape: it delegates to `convertia_core::run()` and holds
//! no logic of its own (the §1.1a boot-glue source-scans concatenate lib.rs + this file, so the shim is
//! still inside the scanned production corpus).

// The crate-root lint policy (scripts/check-rust-lint-contract REQUIRED_ATTRS) is asserted at BOTH crate
// roots — lib.rs (the §0.7 tier modules + the app entry body) and this bin root — so the thin shim can
// never quietly grow un-linted logic.
#![deny(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::wildcard_enum_match_arm)]

fn main() -> tauri::Result<()> {
    convertia_core::run()
}
