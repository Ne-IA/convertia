//! xtask — ConvertIA developer task runner (the `cargo xtask` pattern).
//!
//! P1.6.2 reserved this as a compile-only workspace member so the §0.4.5 codegen + §6.7.1 coverage bins
//! have a home and the G19 generated-drift check can bind to a concrete invocation (wired in P1.28)
//! rather than a guessed one. P1.26 adds the `codegen` task; line/diff coverage (G27/G28) is measured
//! directly by `cargo-llvm-cov` in CI (P1.54), NOT an xtask task — a future coverage task here would be
//! the §6.4.3a corpus-coverage bin (`check-corpus-coverage`, P3+), the §6.7.1 coverage home §0.7 reserves.
//!
//! Invoke from the workspace as `cargo run -p xtask -- <task>` (a `cargo xtask <task>` cargo-alias is an
//! optional developer-ergonomics layer, not required for the task to run). Tasks:
//!   * `codegen` — regenerate the single tracked `src/lib/ipc/bindings.ts` from the convertia-core
//!     tauri-specta builder (§0.4.5). [Build-Session-Entscheidung: P1.26]

// G29: deny unsafe at the crate root (xtask is first-party). No FFI surface here. The §1.2 in-core
// no-panic policy (G4) deliberately does NOT apply to xtask (a dev tool, never bundled), so this CLI
// uses ordinary process spawning + stderr diagnostics.
#![deny(unsafe_code)]

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("codegen") => codegen(),
        Some(other) => {
            eprintln!("xtask: unknown task `{other}` (known tasks: codegen)");
            ExitCode::from(2)
        }
        None => {
            eprintln!("xtask: no task given (usage: cargo run -p xtask -- codegen)");
            ExitCode::from(2)
        }
    }
}

/// §0.4.5: regenerate the single tracked `src/lib/ipc/bindings.ts` by running convertia-core's
/// `regenerate_committed_bindings` codegen test (the tauri-specta export from the SHARED
/// `ipc_specta_builder()`, so the generated TS surface cannot drift from the registered Rust surface).
/// The export is homed as an `#[ignore]`d test so the hermetic `cargo test` suite never mutates a
/// tracked source; this task is the single producer the G19 drift check binds to in P1.28.
/// [Build-Session-Entscheidung: P1.26]
fn codegen() -> ExitCode {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .args([
            "test",
            "-p",
            "convertia-core",
            "--bin",
            "convertia-core",
            "regenerate_committed_bindings",
            "--",
            "--ignored",
        ])
        .status();
    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => {
            eprintln!(
                "xtask codegen: bindings.ts regeneration failed (cargo test exit {:?})",
                s.code()
            );
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("xtask codegen: could not spawn cargo to run the codegen test: {e}");
            ExitCode::FAILURE
        }
    }
}
