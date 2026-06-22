//! ConvertIA core — the Tauri v2 host binary entry point.
//!
//! P1.13 stands up the Tauri v2 `Builder` entrypoint on Tauri's managed multi-threaded tokio async
//! runtime (§0.4.0/§0.8/§0.9): the §0.4.5 tauri-specta codegen seam — `collect_commands!`/
//! `collect_events!` empty until the C1..C13 commands + E-series events of P2, plus the P1.25
//! standalone `.types(...)` registration of the §0.6 identity newtypes so they never generate as
//! `any` — the empty-but-present `invoke_handler`, and the `mount_events` setup hook. The §0.7
//! logical-module roots (`domain`/`outcome`/`ipc`/…) were scaffolded in P1.9–P1.11; the §7.2.1 ordered
//! startup spine is P2; the window frame is P1.16; generating + committing `bindings.ts` from this
//! builder is P1.26.

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

use std::path::PathBuf;

use tauri::Manager;

use crate::domain::{CollectedSetId, CollectingId, InstanceId, ItemId, RunId};

/// [Build-Session-Entscheidung: P1.15] The §7.2.1 step-2 boot context: the per-launch identity plus
/// the three resolved base dirs, held as the §7.1.2 app-managed singleton (`app.manage`). Its home is
/// the binary root for P1 because the §7.2.1 ordered startup-sequence MODULE is the P2 app-shell spine
/// — §0.7 has no app-shell module yet and P1 must not add an unsanctioned folder (CLAUDE §1a / G69);
/// P2 relocates this type into that spine. The fields are read by the P2 ordered spine + later phases
/// (none of the dirs is created here — directory creation is §7.2.1 step 5).
#[derive(Debug)]
#[expect(
    dead_code,
    reason = "§7.2.1 step-2 boot context; fields read by the P2 ordered startup spine + later phases (P1.15)"
)]
struct StartupContext {
    /// §7.1.2 per-launch identity — a random v4, minted once in `setup`.
    instance_id: InstanceId,
    /// §7.2.1 step-2 config base dir (resolved, NOT created here).
    config_dir: PathBuf,
    /// §7.2.1 step-2 local-data / scratch base dir (§2.14; resolved, NOT created here).
    scratch_base_dir: PathBuf,
    /// §7.2.1 step-2 log base dir (§7.5; resolved, NOT created here).
    log_dir: PathBuf,
}

/// [Build-Session-Entscheidung: P1.25] The single registration of the §0.6 standalone identity types
/// into a tauri-specta/specta type collection (§0.4.5). Used by BOTH the live `Builder::types` call
/// and the P1.25 type-gen test, so the registered set and its test assertion cannot drift. The C1–C13
/// command/event signatures that reference these types (and thus pull them in automatically) are P2;
/// until then this explicit registration is what keeps them out of `any` in `bindings.ts` (P1.26).
fn register_ipc_identity_types(types: specta::Types) -> specta::Types {
    types
        .register::<InstanceId>()
        .register::<RunId>()
        .register::<CollectedSetId>()
        .register::<CollectingId>()
        .register::<ItemId>()
}

/// [Build-Session-Entscheidung: P1.25/P1.26] The single tauri-specta `Builder` — the ONE source the
/// generated `src/lib/ipc/bindings.ts` is produced from (the `bindings_codegen` export test, driven by
/// `cargo run -p xtask -- codegen`, §0.4.5) AND the runtime invoke/event registry (`main`). Sharing one
/// constructor is what guarantees the generated TS surface can never drift from the registered Rust
/// surface. The command + event sets are EMPTY in P1 — the C1..C13 commands and the E-series events are
/// authored in P2.
///
/// §0.6 standalone-type registration: with the command set empty the §0.6 identity newtypes are
/// referenced by no command signature, so without an explicit registration tauri-specta omits them from
/// `bindings.ts` (and a future C-arg reaching an un-registered one would emit `any` — §0.4.5 / the §0.6
/// "in collect_types![] or the drift check emits `any`" line). tauri-specta v2 has NO `collect_types!`
/// macro; its canonical equivalent is `specta::Types::default().register::<T>()` chained per type, handed
/// to `Builder::types(&types)` — so the five P1.9 identity types emit as named TS types from the first
/// `bindings.ts` commit (the C1–C13 args that USE them are P2).
fn ipc_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![])
        .events(tauri_specta::collect_events![])
        .types(&register_ipc_identity_types(specta::Types::default()))
}

fn main() -> tauri::Result<()> {
    // §0.4.5 IPC seam: the shared `ipc_specta_builder()` is BOTH the runtime invoke/event registry and
    // the single source the generated `bindings.ts` is produced from (no drift between them). Empty
    // command/event sets in P1 — the C1..C13 commands + the E-series events are authored in P2.
    let builder = ipc_specta_builder();

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
            // §0.4.5 IPC event-channel mount (the P1.13 tauri-specta seam).
            builder.mount_events(app);

            // ── §7.2.1 startup stages the bootable empty window needs ─────────────────────
            // [Build-Session-Entscheidung: P1.15] P1 lands ONLY the compile-and-boot stages,
            // NOT the §7.2.1 ordered spine: stage 1 (single-instance guard) is already real via
            // the P1.14 plugin; the §7.2.1 step ORDER plus the engine-presence / exec-permission /
            // scratch-reclaim / launch-intake / WebView-fault stages are the P2 startup-sequence
            // cluster (later phases fill the bodies). Landed here: stage 2 + the stage-6 slot.

            // Stage 2 — establish the per-launch InstanceId (§7.1.2: random v4) and resolve the
            // three base dirs via app.path() (§7.2.1 step 2: config / local-data scratch §2.14 /
            // log §7.5). NO directory is created here (creation is §7.2.1 step 5). Each call below
            // touches only local uuid + filesystem primitives, so the boot path opens no socket
            // (§7.2.2; proven by the P1.15.1 boot-invariant test + the G29 first-party rule (g)).
            let startup = StartupContext {
                instance_id: InstanceId::mint(),
                config_dir: app.path().app_config_dir()?,
                scratch_base_dir: app.path().app_local_data_dir()?,
                log_dir: app.path().app_log_dir()?,
            };
            app.manage(startup);

            // Stage 6 — window-create slot. The §7.3.1 main window + the empty WebView frame are
            // the P1.16 deliverable; this named slot is where it lands. Intentionally empty here.

            Ok(())
        })
        .run(tauri::generate_context!())
}

#[cfg(test)]
mod boot_invariants {
    //! §7.2.2 boot invariant — the startup path opens no socket. The §6.7.1 Lane-A compensating
    //! guard (cargo-test plane) for the Lane-B-only egress gate (§2.11.4 / §7.2.2), pairing with the
    //! P0 G29 first-party no-socket rule (g) at the source plane. [Build-Session-Entscheidung: P1.15.1]

    /// The production boot source = this binary's `main.rs` up to the `#[cfg(test)]` boundary, so the
    /// sentinel needles declared in THIS module can never self-match the `include_str!` scan.
    fn production_boot_source() -> &'static str {
        let full = include_str!("main.rs");
        // Take the production prefix before this module's `#[cfg(test)]` attribute (the first such
        // marker), or the whole file if absent. `split_once` avoids the impossible-`None` dead
        // fallback an `unwrap_or` would carry (`str::split(..).next()` is always `Some`).
        full.split_once("#[cfg(test)]")
            .map_or(full, |(prefix, _)| prefix)
    }

    // §7.2.2 / §6.4.1 unit (G15): a structural assertion that the production boot path references no
    // network primitive — the cargo-test companion to the G29 source rule (g), scoped to §7.2.2.
    #[test]
    fn boot_path_opens_no_socket() {
        let src = production_boot_source();
        // Needles assembled by `concat!` so the forbidden substrings never appear literally in this
        // test file (which `include_str!` would otherwise self-match through the production scan).
        let net_primitives = [
            concat!("std", "::", "net"),
            concat!("tokio", "::", "net"),
            "TcpStream",
            "TcpListener",
            "UdpSocket",
            "reqwest",
            "ureq",
            concat!("hyper", "::"),
        ];
        for primitive in net_primitives {
            assert!(
                !src.contains(primitive),
                "§7.2.2 zero-startup-network violated: the boot path references `{primitive}` — \
                 the startup sequence must open no socket (pairs with G29 rule (g))"
            );
        }
    }
}

#[cfg(test)]
mod ipc_typegen {
    //! §0.4.5 / §0.6: the IPC type-gen seam registers the §0.6 identity types so the generated
    //! `bindings.ts` (P1.26) carries them as named TS types, never `any`. [Build-Session-Entscheidung: P1.25]
    use super::*;

    // §6.4.1 unit (G15): the five §0.6 identity newtypes register into the type collection (so none
    // emits `any` in bindings.ts), pinned to the exact resolved set so a dropped/added registration
    // reddens the build. Verified by read-back against the §0.8-pinned specta.
    #[test]
    fn identity_types_registered_for_typegen() {
        let types = register_ipc_identity_types(specta::Types::default());
        // 6 = the 5 identity newtypes (InstanceId/RunId/CollectedSetId/CollectingId/ItemId) registered
        // as NAMED types + the single shared `Uuid` named type they pull in: with specta's `uuid`
        // feature (§0.8) `Uuid` is itself a NAMED TS type, referenced by the four Uuid-newtypes and so
        // registered exactly once; `ItemId`'s `u32` field inlines as a number primitive (not named).
        assert_eq!(
            types.len(),
            6,
            "the five §0.6 identity types + their shared Uuid named type must register (§0.4.5/§0.6)"
        );
    }
}

#[cfg(test)]
mod bindings_codegen {
    //! §0.4.5 IPC type-gen: the single tracked `src/lib/ipc/bindings.ts` (the frontend's only IPC door,
    //! §0.7) is generated from the SAME `ipc_specta_builder()` the runtime uses, so the emitted TS
    //! surface can never drift from the registered Rust surface. `regenerate_committed_bindings` is the
    //! on-demand codegen ACTION (driven by `cargo run -p xtask -- codegen`); the hermetic
    //! `committed_bindings_are_nonempty_and_typed` test reads the committed artifact back and proves it
    //! is non-empty + typed (no `any`). Committed-file freshness is the G19 drift gate's job (regenerate +
    //! `git diff --exit-code`, registered in P1.28). [Build-Session-Entscheidung: P1.26]
    use super::*;
    use specta_typescript::Typescript;

    /// The single tracked path (§0.7): `<repo>/src/lib/ipc/bindings.ts`, resolved from this crate's
    /// compile-time manifest dir (`<repo>/src-tauri`) so it is independent of the process CWD.
    const TRACKED_BINDINGS_PATH: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/ipc/bindings.ts");

    // §0.4.5 codegen ACTION (not a behavioural assertion): regenerate the committed bindings.ts in place
    // from the shared builder. `#[ignore]`d so the hermetic `cargo test` suite (G15) never mutates a
    // tracked source file — it runs ON DEMAND via `cargo run -p xtask -- codegen` (`-- --ignored`), and
    // committed-file freshness is the G19 drift gate's job (regenerate + `git diff --exit-code`, P1.28).
    // [Test-Change: P1.26 — new-test:codegen side-effect run on demand via the xtask codegen task, never in the hermetic suite, §0.4.5]
    #[test]
    #[ignore = "codegen action; run via `cargo run -p xtask -- codegen`, not the hermetic suite (§0.4.5)"]
    fn regenerate_committed_bindings() {
        ipc_specta_builder()
            .export(Typescript::default(), TRACKED_BINDINGS_PATH)
            .expect("§0.4.5 bindings.ts codegen export failed");
    }

    // §6.4.1 unit (G15): the committed generated bindings.ts (the real shipped artifact) is non-empty
    // and TYPED — read it back (the §0.2 read-the-output-back discipline applied to codegen) and assert
    // it exposes the `export`-ed surface (the §0.4.5 / G19 ts-bindings non-empty sanity), declares the
    // five §0.6 identity newtypes as named `export type` declarations, and carries no `any` escape.
    // Hermetic: a read-only check of the tracked file — NO shared-OS-temp write (`std::env::temp_dir` is
    // the §2.14.1 shared-temp anti-pattern the vendored G29 `temp-dir` lint forbids). The export CALL
    // itself is exercised by `regenerate_committed_bindings` (the xtask codegen task) and, from P1.28, by
    // the G19 regenerate-and-diff drift gate.
    #[test]
    fn committed_bindings_are_nonempty_and_typed() {
        // [Test-Change: P1.26 — old-obsolete+new-correct, §0.4.5 (the prior temp-export form tripped the vendored G29 `temp-dir` lint; reading the committed artifact back is the correct hermetic check)]
        let ts = std::fs::read_to_string(TRACKED_BINDINGS_PATH).expect(
            "the committed src/lib/ipc/bindings.ts must exist — regenerate it via the xtask codegen task",
        );

        assert!(
            !ts.trim().is_empty(),
            "the committed bindings.ts must be non-empty (§0.4.5 / the G19 non-empty sanity)"
        );
        assert!(
            ts.contains("export"),
            "the committed bindings.ts must expose the `export`-ed IPC surface (§0.4.5 / the G19 ts-bindings validator)"
        );
        for ty in [
            "InstanceId",
            "RunId",
            "CollectedSetId",
            "CollectingId",
            "ItemId",
        ] {
            assert!(
                ts.contains(&format!("export type {ty}")),
                "the §0.6 identity newtype `{ty}` must be declared as a named `export type` in the committed bindings.ts (§0.4.5/§0.6)"
            );
        }
        // No IPC type may degrade to the TS `any` escape (§0.4.5 / CLAUDE §5 "no any"). The needle is
        // assembled by `concat!` so the forbidden token never appears literally in this scanned
        // production file — the same self-match-avoidance the `boot_invariants` test uses (G8).
        let any_escape = concat!(":", " any");
        assert!(
            !ts.contains(any_escape),
            "no IPC type may generate as the TS `any` escape — the §0.6 types must stay named (§0.4.5)"
        );
    }
}
