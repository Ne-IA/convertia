//! ConvertIA core — the Tauri v2 host binary entry point.
//!
//! P1.13 stands up the Tauri v2 `Builder` entrypoint on Tauri's managed multi-threaded tokio async
//! runtime (§0.4.0/§0.8/§0.9): the §0.4.5 tauri-specta codegen seam (empty `collect_commands!`/
//! `collect_events!` — the C1..C13 commands and the E-series events are authored in P2), the
//! empty-but-present `invoke_handler`, and the `mount_events` setup hook. The §0.7 logical-module
//! roots (`domain`/`outcome`/`ipc`/…) were scaffolded in P1.9–P1.11; the §7.2.1 ordered startup spine
//! is P2; the window frame is P1.16; `bindings.ts` codegen is the `cargo xtask` of P1.25/P1.26.

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

mod detection;
mod domain;
mod engines;
mod fs_guard;
mod ipc;
mod isolation;
mod orchestrator;
mod outcome;
mod platform;
mod pool;
mod run;

fn main() -> tauri::Result<()> {
    // §0.4.5 IPC codegen seam: the tauri-specta `Builder` is the single source `bindings.ts` is
    // generated from (the `cargo xtask codegen` bin, P1.26) AND the runtime invoke/event registry.
    // The command + event sets are EMPTY in P1 — the C1..C13 commands and the E-series events are
    // authored in P2; `collect_types!` (so the §0.6 identity types do not generate as `any`) is added
    // with the P1.25 tauri-specta-builder + export wiring.
    let builder = tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![])
        .events(tauri_specta::collect_events![]);

    // [Build-Session-Entscheidung: P1.13] Async runtime: ConvertIA runs on Tauri v2's own managed
    // multi-threaded tokio runtime — `tauri::async_runtime`'s default builds a `tokio::runtime::
    // Runtime::new()` (multi-thread), exactly the §0.8/§0.9 "tokio (multi-thread)" choice. We
    // deliberately do NOT wrap `main` in `#[tokio::main]` (a Tauri-v2 anti-pattern: a second runtime
    // nested around the blocking event loop) nor re-register one via `async_runtime::set` (it would
    // only duplicate Tauri's identical default and add a runtime-lifetime burden, for no functional
    // gain). tokio is pinned transitively in `Cargo.lock` (§0.8 "exact") and is added as a DIRECT
    // dependency where the first in-crate `async` command code calls into it (P2).
    tauri::Builder::default()
        // [Build-Session-Entscheidung: P1.14] §0.8 plugin wiring (registration only; the handlers
        // that USE these plugins are P2). single-instance is registered FIRST (§7.1) so a second
        // launch is intercepted before the other plugins init; ConvertIA is desktop-only so no
        // `#[cfg(desktop)]` guard is needed, and the callback is empty — the §7.8 launch-arg /
        // second-instance intake hand-off (forward_launch_intake + the PendingIntake buffer) is P2.
        // dialog + opener are called Rust-side (DialogExt/OpenerExt) so they take NO WebView grant
        // (§0.10); store/log get store:default/log:default in capabilities/main.json (P1.21).
        .plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {}))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_log::Builder::new().build())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
}
