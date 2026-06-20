//! xtask — ConvertIA developer task runner (the `cargo xtask` pattern).
//!
//! P1.6.2 reserves this as a compile-only workspace member so the G19 generated-drift check can bind
//! to a concrete `cargo xtask codegen` command (wired in P1.28) rather than a guessed invocation. The
//! codegen + coverage subcommands are added by their consuming boxes (P1.26 / P1.54).

// G29: deny unsafe at the crate root (xtask is first-party). No FFI surface here.
#![deny(unsafe_code)]

fn main() {}
