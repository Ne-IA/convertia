//! ConvertIA core — the Tauri v2 host binary entry point.
//!
//! P1.6 establishes the minimal compiling entry so the workspace boots and the live Rust gates
//! (G3/G4/G14 format/lint/test, G29 unsafe-policy, G53 core-closure) act on a real crate root. The
//! Tauri `Builder` + the §7.2.1 startup spine are built in P1.13; the §0.7 logical-module roots
//! (`domain`/`outcome`/`ipc`/…) in P1.9–P1.11.

// §2.12 / G29: the MIT core decodes no untrusted bytes in-process, so it carries zero `unsafe` —
// denied at the crate root. The single allow-listed FFI shim is `crate::platform` (the OS-primitive
// module, added with its first syscall); the densest unsafe surface is the SEPARATE
// `convertia-imgworker` crate, not this one.
#![deny(unsafe_code)]
// §1.2 in-core no-panic policy (T1/T5; G4) + exhaustive-dispatch deny (no `_ =>` on the §0.6 dispatch
// enums; G4/G14) — crate-level lints asserted at the root (scripts/check-rust-lint-contract
// REQUIRED_ATTRS). The detect/IPC modules + the dispatch enums they govern are built in P1.9–P1.11/P2.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::wildcard_enum_match_arm)]

mod domain;

// [Build-Session-Entscheidung: P1.6] minimal entry — the real Tauri `Builder` is built in P1.13.
fn main() {}
