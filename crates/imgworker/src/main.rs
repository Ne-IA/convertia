//! convertia-imgworker — the isolated libvips/libheif/librsvg image-worker process (spec §3.5.5).
//!
//! P1.6 reserves this as a compile-only workspace member (`fn main`) so the workspace graph carries
//! BOTH first-party crates the G29 `#![deny(unsafe_code)]`-at-every-root check and the G53
//! core-must-not-link-imgworker-libs rule address from P1. The C-FFI module (`src/ffi.rs`, the one
//! G29 allow-listed unsafe surface) and the libvips/libheif image pipeline are built in P4/P5.

// G29: deny unsafe at the crate root; the single allow-listed FFI module `src/ffi.rs` is added in P4.
#![deny(unsafe_code)]

fn main() {}
