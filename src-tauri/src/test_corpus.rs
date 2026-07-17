//! The §6.4.5 SINGLE-SOURCE corpus helper — the one place a test resolves a `tests/corpus/` fixture path.
//!
//! test-strategy §3 ("Corpus & fixtures (§6.4.5 · G24a)", :601) mandates "Tests reach the corpus through **one
//! helper** (no inline path duplication, no per-test re-listing)", and the P3.61 box restates it
//! ("single-source helper, no inline duplication"). Before P3.61 the only corpus-reach
//! path was a PRIVATE `tests_dir()` inside `crate::detection`'s `#[cfg(test)] mod kat_tests` (P3.30) — reachable
//! by that module alone, so the P3.61 sentinel test in `crate::engines` had no way to call it. Copy-pasting
//! `env!("CARGO_MANIFEST_DIR")/../tests` into a second module is exactly the inline duplication the rule
//! forbids, so the helper is promoted HERE and `kat_tests` now consumes it.
//!
//! [Build-Session-Entscheidung: P3.61] **Homed at the crate root, not in a §0.7 tier.** The §0.7 physical tree
//! decomposes the PRODUCT into dependency tiers; this module is `#[cfg(test)]`-only test infrastructure that
//! every tier's tests may reach, so tucking it inside one tier (`detection`, `engines`) would invert the
//! dependency direction the tiers exist to express — a sibling of `main.rs` states "cross-cutting, not a tier"
//! structurally. It adds a FILE, never a directory, so the §1a/§0.7 structure map (G69, which asserts the
//! DIRECTORY set) is untouched.
//!
//! Every path returned here resolves to a fixture that G24a has sha256-vouched: each `tests/corpus/` byte
//! carries a `manifest.toml` row (the bijection), so a test reaching a fixture through this helper is reading
//! integrity-pinned bytes rather than an ad-hoc file.

use std::path::{Path, PathBuf};

/// The workspace-root `tests/` dir. This crate's manifest dir is `src-tauri/`, so `tests/` is `../tests`.
pub fn tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests")
}

/// The §6.4.5 corpus root — `tests/corpus/`, whose every byte is manifest-rowed + sha256-vouched (G24a).
pub fn corpus_dir() -> PathBuf {
    tests_dir().join("corpus")
}

/// One §6.4.5 corpus fixture by its manifest-relative `path` (e.g. `"canonical.csv"`). The P3 corpus is FLAT at
/// the root (the P3.30 layout decision); the per-source-format tier P7.61 stages changes only what callers pass
/// here, since the argument IS the manifest `path`.
pub fn fixture(name: &str) -> PathBuf {
    corpus_dir().join(name)
}
