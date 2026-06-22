//! ConvertIA build script — the Tauri v2 build-time setup (P1.12).
//!
//! `tauri_build::build()` is mandatory for every Tauri app: it reads `tauri.conf.json`, generates the
//! permission/capability + context artifacts the runtime `tauri::generate_context!()` consumes
//! (P1.13), and re-runs when the conf / capabilities change. §0.4.0 / §0.4.5.

// §2.12 / G29: deny unsafe at this build-script crate root (first-party); build.rs has no FFI surface.
#![deny(unsafe_code)]

// [Build-Session-Entscheidung: P1.12] The §0.4.5 tauri-specta `bindings.ts` codegen is NOT driven from
// this build script: it is owned by the xtask `codegen` task — `cargo run -p xtask -- codegen` (wired
// P1.26, registered with the G19 generated-drift check at P1.28) — emitting the single tracked
// `src/lib/ipc/bindings.ts`. This build script is the §0.4.5 build-time generation seam — kept to
// `tauri_build::build()` alone because ConvertIA drives codegen through xtask (one command, one tracked
// artifact), not a build-script hook.
fn main() {
    tauri_build::build();
}
