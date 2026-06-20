//! convertia-imgworker — the isolated libvips/libheif/librsvg image-worker process (spec §3.5.5).
//!
//! P1.6 reserved this as a compile-only workspace member so the workspace graph carries BOTH
//! first-party crates the G29 `#![deny(unsafe_code)]`-at-every-root check and the G53
//! core-must-not-link-imgworker-libs rule address from P1. P1.8 adds the single allow-listed FFI
//! module (`ffi`); the libvips/libheif/librsvg `extern "C"` bindings + the image pipeline are
//! built in P4/P5.

// §2.12.4 / G29: every third-party C/C++ decoder is confined to this isolated worker; within it
// `unsafe` is denied at the crate root and permitted ONLY in the one allow-listed FFI module `ffi`.
#![deny(unsafe_code)]

mod ffi;

fn main() {}
