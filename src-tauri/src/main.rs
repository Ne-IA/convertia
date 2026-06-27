//! ConvertIA core — the Tauri v2 host binary entry point.
//!
//! P1.13 stands up the Tauri v2 `Builder` entrypoint on Tauri's managed multi-threaded tokio async
//! runtime (§0.4.0/§0.8/§0.9): the §0.4.5 tauri-specta codegen seam — `collect_commands!` carries the
//! C1..C13 §0.4.1 command surface from P2.21 (interface shells; `collect_events!` stays empty BY DECISION —
//! the §0.4.2 app:// events are RAW `app.emit`/`listen` events whose payloads register via `.types()` at
//! P2.39, and the P2.37 `ConversionEvent` Channel payload joins via C6/P2.29, neither via `collect_events!`),
//! plus the P1.25 standalone `.types(...)` registration of the §0.6 identity
//! newtypes so they never generate as `any` — the `invoke_handler`, and the `mount_events` setup hook. The §0.7
//! logical-module roots (`domain`/`outcome`/`ipc`/…) were scaffolded in P1.9–P1.11; the §7.2.1 ordered
//! startup spine is P2; the window MODEL is locked in P1.16 (the config-declared single `main` window;
//! the rendered frame is P1.23+P1.31); generating + committing `bindings.ts` from this builder is P1.26.

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

use crate::domain::{
    CollectedSetId, CollectingId, InstanceId, IntakePayload, ItemId, LossyKind, RunId,
};
use crate::outcome::{AppFault, IpcError, OutcomeMsg};

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

/// [Build-Session-Entscheidung: P2.8] The §2.8.2-mandated standalone wire-taxonomy registration. §2.8.2
/// (line 1261) EXPLICITLY requires `LossyKind` (with `OutcomeMsg`/`ConversionErrorKind`, both P2.18/P2.20)
/// derive `specta::Type` AND be registered in `collect_types![]` so `Target.lossy` / `OutcomeMsg.kind`
/// never generate as `any` in `bindings.ts`. `LossyKind` lands here at P2.8 (when it is authored); the
/// other two §2.8.2 types join when they are authored. This is a SPEC mandate — distinct from the other
/// P2 §0.6 wire types (e.g. `CollectedSet`/`Target`), whose registration is deferred to their C-command
/// consumer (the P2.2-P2.7 defer pattern). Kept as its own function (not folded into the identity set)
/// so the two registration RATIONALES — identity-spine vs §2.8.2-taxonomy-mandate — stay legible.
///
/// [Build-Session-Entscheidung: P2.19] `IpcError` (§0.4.3 — the single wire error shape every command
/// `Err` / `ItemOutcome::Failed.error` returns) joins here: §0.4.3 mandates it derive `specta::Type` AND
/// be registered in `collect_types![]`, and registering it pulls its `kind: ConversionErrorKind` field
/// into the export as a named type — satisfying the §2.8.2 `ConversionErrorKind` registration via its
/// consumer (the P2.18 defer-to-IpcError/OutcomeMsg decision).
///
/// [Build-Session-Entscheidung: P2.20] `OutcomeMsg` (§2.8.2 — the surfaced per-item `ItemResult.reason`
/// line) joins here: §2.8.2 (line 1261) mandates it derive `specta::Type` AND be registered so
/// `ItemResult.reason` mirrors as the named `OutcomeMsg`, not `any`; registering it pulls its referenced
/// `SkipReason` (`OutcomeMsg::Skipped.reason`) into the export as a named type (the `ConversionErrorKind`/
/// `LossyKind` it also references are already named via `IpcError` / the standalone `LossyKind`).
fn register_ipc_taxonomy_types(types: specta::Types) -> specta::Types {
    types
        .register::<LossyKind>()
        .register::<IpcError>()
        .register::<OutcomeMsg>()
}

/// [Build-Session-Entscheidung: P2.39] The §0.4.2 `app://` event-payload registration. The three app-wide
/// events (`app://fault` / `app://intake` / `app://close-requested`, named in `crate::ipc::events`) are RAW
/// `app.emit` / TS `listen` events (§0.4.2 "App-wide events — `app.emit` / TS `listen`"; §7.3.2 shows the
/// raw `window.emit` for close-requested), NOT tauri-specta `collect_events!` typed events: tauri-specta
/// rc.25's TS event codegen unconditionally emits a `makeEvent` helper with an `any`-typed `payload`
/// parameter (`tauri-specta-2.0.0-rc.25/src/lang/js_ts.rs` MAKE_EVENT_IMPL_TS), which would violate the platform's
/// hard no-`any` rule frozen on the generated `bindings.ts` (eslint `@typescript-eslint/no-explicit-any` /
/// G5, plus G8) — the SAME class of decision P2.22 made when it chose `ErrorHandlingMode::Throw` to avoid
/// the `any`-bearing `typedError` helper. So `collect_events![]` (below) stays EMPTY and THIS is the
/// "register in collect_types![]" the §0.4.2/§0.4.3 box-notes call for (tauri-specta v2 has no
/// `collect_types!` macro — `.types(register::<T>())` is its canonical equivalent): it exports each app://
/// payload as a NAMED `bindings.ts` type so the TS `listen(<name>)` side type-checks rather than mirroring
/// `any`. Two payload types register here — `AppFault` (app://fault, `crate::outcome`) and `IntakePayload`
/// (app://intake, `crate::domain`); `app://close-requested` carries `()` (§0.4.2), so it has NO payload type
/// to register. Kept as its own function (not folded into identity/taxonomy) so the three registration
/// RATIONALES — identity-spine vs §2.8.2-taxonomy-mandate vs §0.4.2-event-payload — stay legible.
fn register_ipc_event_types(types: specta::Types) -> specta::Types {
    types.register::<AppFault>().register::<IntakePayload>()
}

/// [Build-Session-Entscheidung: P1.25/P1.26] The single tauri-specta `Builder` — the ONE source the
/// generated `src/lib/ipc/bindings.ts` is produced from (the `bindings_codegen` export test, driven by
/// `cargo run -p xtask -- codegen`, §0.4.5) AND the runtime invoke/event registry (`main`). Sharing one
/// constructor is what guarantees the generated TS surface can never drift from the registered Rust
/// surface. The C1..C13 §0.4.1 command set is registered from P2.21 (interface shells, each filled by its
/// per-command fill-box). [Reconcile: P2.39] `collect_events![]` (below) stays EMPTY BY DECISION: the §0.4.2
/// app:// events (`app://fault`/`intake`/`close-requested`) are RAW `app.emit` / TS `listen` events whose
/// payload types register via `register_ipc_event_types` (`.types()`), NOT tauri-specta typed events — a
/// `collect_events!` entry would force an `any`-bearing `makeEvent` helper into `bindings.ts` (P2.39, see
/// `register_ipc_event_types`). The run-telemetry `ConversionEvent` (authored P2.37) is a CHANNEL payload,
/// NOT a `collect_events!` event, so it joins `bindings.ts` via C6's `onProgress: Channel<ConversionEvent>`
/// arg (P2.29) — the deferred-to-consumer pattern (like `ScanProgress` via C1), NOT registered here.
///
/// §0.6 standalone-type registration: the §0.6 identity newtypes are still referenced by no command
/// signature (the P2.21 shells are argument-/return-free), so without an explicit registration tauri-specta
/// omits them from `bindings.ts` (and a future C-arg reaching an un-registered one would emit `any` —
/// §0.4.5 / the §0.6 "in collect_types![] or the drift check emits `any`" line). tauri-specta v2 has NO
/// `collect_types!` macro; its canonical equivalent is `specta::Types::default().register::<T>()` chained
/// per type, handed to `Builder::types(&types)` — so the five P1.9 identity types emit as named TS types
/// from the first `bindings.ts` commit (the C1–C13 args that USE them arrive with the per-command fill-boxes).
fn ipc_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        // §0.4.1 command surface (P2.21): the C1–C13 handlers, one-file-per-command-group (§0.7),
        // registered as interface shells — each command's full request/response contract + orchestrator
        // delegation is authored by its named fill-box (see `crate::ipc`); the closed-set completeness +
        // drift gate over this list is P2.36 (G23). `collect_events![]` (below) stays empty BY DECISION
        // (P2.39): the §0.4.2 app:// events are RAW `app.emit`/TS `listen` events whose payload types register
        // via `register_ipc_event_types` (a `collect_events!` entry would force an `any`-bearing `makeEvent`
        // helper into bindings.ts). The C6 run-telemetry `ConversionEvent` (P2.37) is a Channel payload joining
        // via C6's `onProgress` arg (P2.29), NOT a collect_events! event.
        .commands(tauri_specta::collect_commands![
            // intake (§0.4.1 C1 / C2a / C13)
            crate::ipc::intake::ingest_paths,
            crate::ipc::intake::pick_for_intake,
            crate::ipc::intake::cancel_ingest,
            // planning (§0.4.1 C2b / C3 / C4 / C5)
            crate::ipc::planning::pick_destination,
            crate::ipc::planning::get_targets,
            crate::ipc::planning::plan_output,
            crate::ipc::planning::set_destination,
            // conversion run lifecycle (§0.4.1 C6 / C7 / C8)
            crate::ipc::conversion::start_conversion,
            crate::ipc::conversion::cancel_run,
            crate::ipc::conversion::get_run_summary,
            // system / info (§0.4.1 C9 / C10 / C11 / C12)
            crate::ipc::system::open_path,
            crate::ipc::system::open_project_page,
            crate::ipc::system::get_app_info,
            crate::ipc::system::get_engine_health,
        ])
        // [Build-Session-Entscheidung: P2.39] Empty BY DECISION — the §0.4.2 app:// events are RAW
        // `app.emit`/TS `listen` events registered via `.types()` below (`register_ipc_event_types`), never
        // `collect_events!` typed events (which would force an `any`-bearing `makeEvent` helper into
        // bindings.ts — the P2.22 Throw-to-avoid-any precedent applied to events).
        .events(tauri_specta::collect_events![])
        .types(&register_ipc_event_types(register_ipc_taxonomy_types(
            register_ipc_identity_types(specta::Types::default()),
        )))
}

/// [Build-Session-Entscheidung: P2.40] The §7.8.1 launch-intake logic, homed with the Tauri host (§0.7:
/// `main.rs` homes the launch glue — the single-instance callback (§7.1.1), the macOS `RunEvent::Opened`
/// handler, and `app.emit`). This box authors the §0.4.2 / §7.8.1 `app://intake` IDLE-path-only RULE as a
/// pure disposition; the `forward_launch_intake` funnel + the per-OS argv / `RunEvent::Opened` wiring that
/// CONSUME it join this home at P2.54+.
mod launch_intake {
    // The disposition rule is dead in the PRODUCTION build until the funnel wires it (the `Drop` arm at the
    // P2.55 refuse-busy gate, the `Emit`/`Buffer` arms at the P2.59 ready-flag branch); the cfg(test)
    // truth-table tests reference it, so the TEST build is dead-code-clean. `expect` (not `allow`) auto-flags
    // the moment the funnel consumes it — matching `crate::ipc::events` / `crate::domain` / `crate::outcome`.
    #![cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the §7.8.1 IDLE-path-only disposition rule is consumed by the forward_launch_intake funnel (the Drop arm at P2.55, the Emit/Buffer arms at P2.59), so it is dead in the production build until then."
        )
    )]

    /// [Build-Session-Entscheidung: P2.40] The §0.4.2 / §7.8.1 `app://intake` disposition — the THREE
    /// outcomes a launch-time path set can take. Encoding it as an enum makes the IDLE-path-only rule
    /// STRUCTURAL: a `busy` state maps ONLY to `Drop` (see `intake_disposition`), so the funnel can never
    /// emit/buffer ingestable paths mid-run — the §2.4 freeze + the §0.4.2 "never emits `app://intake` with
    /// ingestable paths mid-run" contract become a property of the type, not a convention.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum IntakeDisposition {
        /// §7.1.1 refuse-busy: a run is in flight — DROP the launch paths core-side (no `app://intake`
        /// emit, no `PendingIntake` buffer), so the §2.4 frozen set is never mutated mid-run. Enforced in
        /// the funnel at P2.55.
        Drop,
        /// Idle + the WebView `app://intake` listener is ready — emit `app://intake` so the frontend mirrors
        /// a drop (§5.2/§1.1). Wired at P2.59.
        Emit,
        /// Idle but the WebView is not-yet-ready (the §7.8.1 first-launch listener race) — stash the paths in
        /// `PendingIntake` for the drain-on-mount replay (P2.58/P2.60). Wired at P2.59.
        Buffer,
    }

    /// [Build-Session-Entscheidung: P2.40] The §0.4.2 / §7.8.1 IDLE-path-only decision. `busy` is tested
    /// FIRST and short-circuits to `Drop` regardless of `frontend_ready` — that ordering is what encodes
    /// "never emit ingestable paths mid-run": this fn can return `Emit`/`Buffer` ONLY when `!busy`. The
    /// `busy` / `frontend_ready` predicates are resolved by the funnel against the `AppHandle`
    /// (`converter_is_busy`, §1.9 run-state, wired P2.55; the WebView-ready flag, §7.8.1, wired P2.59) and
    /// PASSED IN, so the rule itself is pure and unit-tested in isolation.
    pub(crate) fn intake_disposition(busy: bool, frontend_ready: bool) -> IntakeDisposition {
        if busy {
            IntakeDisposition::Drop
        } else if frontend_ready {
            IntakeDisposition::Emit
        } else {
            IntakeDisposition::Buffer
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        // §6.4.1 unit (G15): the §0.4.2 / §7.8.1 `app://intake` IDLE-path-only rule (P2.40) — the full
        // busy x ready truth table. The load-bearing rows are the two busy ones: busy yields `Drop` in BOTH
        // ready states (the "never emit ingestable paths mid-run" invariant — a mid-run launch / Open-with
        // is dropped core-side, never emitted or buffered, §7.1.1 / §2.4).
        #[test]
        fn idle_path_only_rule_truth_table() {
            assert_eq!(
                intake_disposition(true, true),
                IntakeDisposition::Drop,
                "§7.1.1: busy drops even when the frontend is ready (never emit ingestable paths mid-run)"
            );
            assert_eq!(
                intake_disposition(true, false),
                IntakeDisposition::Drop,
                "§7.1.1: busy drops even when the frontend is not ready (no buffer mid-run either)"
            );
            assert_eq!(
                intake_disposition(false, true),
                IntakeDisposition::Emit,
                "§7.8.1: idle + ready emits app://intake"
            );
            assert_eq!(
                intake_disposition(false, false),
                IntakeDisposition::Buffer,
                "§7.8.1: idle + not-ready buffers (the first-launch listener race)"
            );
        }

        // §6.4.1 unit (G15): the IDLE-path-only INVARIANT made explicit — for a busy converter the
        // disposition is `Drop` for EVERY readiness value, so the funnel can never emit/buffer ingestable
        // paths mid-run by construction (§0.4.2 "never emits app://intake with ingestable paths mid-run").
        #[test]
        fn busy_never_emits_or_buffers() {
            for ready in [true, false] {
                assert_eq!(
                    intake_disposition(true, ready),
                    IntakeDisposition::Drop,
                    "§0.4.2/§7.1.1: a busy converter always drops launch paths (ready={ready})"
                );
            }
        }
    }
}

fn main() -> tauri::Result<()> {
    // §0.4.5 IPC seam: the shared `ipc_specta_builder()` is BOTH the runtime invoke/event registry and
    // the single source the generated `bindings.ts` is produced from (no drift between them). The C1..C13
    // §0.4.1 command surface is registered from P2.21 (interface shells); the run-telemetry `ConversionEvent`
    // (P2.37) joins `bindings.ts` via C6's Channel arg (P2.29); the §0.4.2 app:// event payloads (`AppFault`/
    // `IntakePayload`) register via `register_ipc_event_types` (`.types()`) at P2.39, as RAW `app.emit`/
    // `listen` events — `collect_events![]` stays empty (a typed event would force an `any` into bindings.ts).
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

            // Stage 6 — window-create slot. [Build-Session-Entscheidung: P1.16] §7.3.1 LOCKS the single
            // `main` window as CONFIG-DECLARED (`tauri.conf.json -> app.windows[main]`, P1.19): Tauri
            // auto-creates + shows it at startup ("created by Tauri at startup", §7.3.1), so the core
            // adds no programmatic window-builder call here. This slot is therefore empty BY DESIGN, not
            // unfinished; the §7.3.1 model is asserted structurally by the `window_model` test below. The
            // loaded React frame arrives with P1.23 (`index.html`) + P1.31 (the React mount); the
            // rendered-frame headed E2E is P9.

            Ok(())
        })
        .run(tauri::generate_context!())
}

#[cfg(test)]
mod boot_invariants {
    //! §7.2.2 boot invariant — the startup path opens no socket. The §6.7.1 Lane-A compensating
    //! guard (cargo-test plane) for the Lane-B-only egress gate (§2.11.4 / §7.2.2), pairing with the
    //! P0 G29 first-party no-socket rule (g) at the source plane. [Build-Session-Entscheidung: P1.15.1]

    /// The production boot source = this binary's `main.rs` up to the first `#[cfg(test)]` boundary, so
    /// sentinel needles declared in a test module can never self-match the `include_str!` scan.
    /// `pub(super)`: SHARED with the §7.3.1 `window_model` no-programmatic-window-builder scan (P1.16).
    pub(super) fn production_boot_source() -> &'static str {
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

    // §6.4.1 unit (G15): the §2.8.2 / §0.4.3 wire-taxonomy registration. §2.8.2 requires `LossyKind` +
    // `OutcomeMsg` and §0.4.3 requires `IpcError` be registered in collect_types![] so `Target.lossy` /
    // `ItemResult.reason` / every command `Err` never emit `any`. The registered set is PINNED BY NAME (not a
    // bare count) so a dropped / added / renamed registration reddens the build with a legible diff.
    // [Test-Change: P2.20 — old-obsolete+new-correct, §2.8.2] old: the P2.19 set lacked `OutcomeMsg`; new
    // (verified by read-back of the registered NAMES): P2.20 adds `.register::<OutcomeMsg>()`, which also pulls
    // in `SkipReason` (`OutcomeMsg::Skipped.reason`) as a named type — `ConversionErrorKind`/`LossyKind`/
    // `PathBuf`/`String` were already named via `IpcError` / the standalone `LossyKind`. Net add: OutcomeMsg +
    // SkipReason (PathBuf/String render inline as TS `string` but specta tracks them as named map entries,
    // exactly as `Uuid` is for the identity set's count of 6, §0.4.3).
    #[test]
    fn taxonomy_types_registered_for_typegen() {
        let types = register_ipc_taxonomy_types(specta::Types::default());
        let mut names: Vec<String> = types
            .into_unsorted_iter()
            .map(|n| n.name.to_string())
            .collect();
        names.sort();
        assert_eq!(
            names.join(","),
            "ConversionErrorKind,IpcError,LossyKind,OutcomeMsg,PathBuf,SkipReason,String",
            "§2.8.2/§0.4.3: the wire-taxonomy registration is LossyKind + IpcError + OutcomeMsg + the named \
             types they pull in (ConversionErrorKind / SkipReason / PathBuf / String)"
        );
    }

    // §6.4.1 unit (G15): the §0.4.2 app:// event-payload registration (P2.39). `register_ipc_event_types`
    // puts the two raw-event payload types (AppFault §crate::outcome, IntakePayload §crate::domain) into the
    // type collection so `listen('app://fault')` / `listen('app://intake')` mirror NAMED types, not `any`
    // (`app://close-requested` carries `()`, so it has no payload type to register). PINNED BY NAME (not a
    // bare count) so a dropped / added / renamed registration reddens with a legible diff. The referenced
    // types are pulled in named too (verified by read-back of the registered names): `AppFault.kind` →
    // ConversionErrorKind, `AppFault.message` → String, `IntakePayload.origin` → IntakeOrigin,
    // `IntakePayload.paths: Vec<PathBuf>` → both the `Vec` container AND `PathBuf` element (specta tracks each
    // as a named map entry, exactly as `Uuid` is for the identity set's count and `String`/`PathBuf` for the
    // taxonomy set — the container `Vec` renders as TS `string[]`, not an `export type Vec`).
    #[test]
    fn event_payload_types_registered_for_typegen() {
        let types = register_ipc_event_types(specta::Types::default());
        let mut names: Vec<String> = types
            .into_unsorted_iter()
            .map(|n| n.name.to_string())
            .collect();
        names.sort();
        assert_eq!(
            names.join(","),
            "AppFault,ConversionErrorKind,IntakeOrigin,IntakePayload,PathBuf,String,Vec",
            "§0.4.2: the app:// event-payload registration is AppFault + IntakePayload + the named types \
             they pull in (ConversionErrorKind / IntakeOrigin / PathBuf / String / the Vec container)"
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
    use tauri_specta::ErrorHandlingMode;

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
        // [Build-Session-Entscheidung: P2.22] §0.4.5 BigInt export policy. `specta-typescript` FORBIDS
        // exporting BigInt-style Rust scalars (`u64`/`usize`/`i64`/`u128`/`i128`) by default — a JS-f64
        // safe-integer guard (ceiling 2^53). The §0.6 `CollectedSet` graph is the FIRST exported type to
        // carry such fields (P2.22 is its first consumer), so the export must declare a policy.
        // `dangerously_cast_bigints_to_number()` (the tauri-specta builder method) casts every bigint-style
        // field to TS `number`. It is an EXPORT-only concern kept HERE at the dev-only codegen call — NOT in
        // the shared `ipc_specta_builder()` (there is one export path; the G19 regen uses exactly this call).
        // SAFE because EVERY wire bigint is a CAPPED count / byte-size / option-range far below 2^53:
        // `DroppedItem.size_bytes`, `CollectedSet.{count,total_bytes}` / `Mixed.found`,
        // `RerunPrompt.equivalent_count`, `PreflightVerdict.est_total_*_bytes`, the §1.6 OptionKind
        // min/max/step + `OptionValue::Int` (sizes are `u64` precisely BECAUSE a single file may exceed
        // 4 GB). The §2.5 `EquivKey` (a hash-style full-range value) is core-internal and NEVER on the wire;
        // there is no hash / checksum / timestamp / full-range id wire-bigint. FORWARD-GUARD: a FUTURE
        // wire-DTO field holding a full-range 64-bit value MUST override per-field with
        // `#[specta(type = String)]` + a lossless `#[serde(with = …)]`, never ride this global cast (§0.4.5).
        // [Build-Session-Entscheidung: P2.22] §0.4.5 no-`any` in the GENERATED bindings drives the error mode.
        // C1 is the first command to return a typed `Result<_, IpcError>`. tauri-specta's DEFAULT `Result`
        // error mode wraps each such command in a `typedError<T, E>` runtime helper that is UNAVOIDABLY
        // `any`-bearing — the default casts the caught rejection `e as any`, and a custom `typed_error_impl`
        // only trades that for tauri-specta's generated `_assertTypedErrorFollowsContract: … => Promise<any>`
        // check. Either form violates the platform's hard no-`any` rule (CLAUDE §5 / §0.4.5), which is ENFORCED
        // on the generated `bindings.ts` (eslint `@typescript-eslint/no-explicit-any`, frozen by G5; plus G8).
        // So the codegen selects `ErrorHandlingMode::Throw`: command wrappers return `Promise<T>` directly and
        // the §0.4.3 `IpcError` is the THROWN rejection value (the §5.8 frontend `await commands.X(…)` examples
        // carry no `{ status }` assumption, so this is spec-compatible). No `typedError` helper, no assertion,
        // no `any`. Export-only concern, kept at the codegen call (like the bigint cast), not the shared builder.
        ipc_specta_builder()
            .dangerously_cast_bigints_to_number()
            .error_handling(ErrorHandlingMode::Throw)
            .export(Typescript::default(), TRACKED_BINDINGS_PATH)
            .expect("§0.4.5 bindings.ts codegen export failed");
        // [Build-Session-Entscheidung: P2.8] `specta_typescript` emits TRAILING WHITESPACE on multi-line
        // union types (`"a" | ` per line — first exposed by the P2.8 `LossyKind` enum; the prior identity
        // types were single-line aliases) and a platform-dependent EOL. Normalise the generated artifact
        // to the repo editorconfig (LF, no trailing whitespace, exactly one final newline) so the codegen
        // output passes G52 deterministically — `bindings.ts` is `.prettierignore`d (G13 skips it), so the
        // generator is its own formatter. Idempotent: re-running codegen yields byte-identical output, so
        // the G19 regenerate-and-diff drift gate stays clean.
        let raw = std::fs::read_to_string(TRACKED_BINDINGS_PATH)
            .expect("re-read the freshly-exported bindings.ts for normalisation");
        let body = raw
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n");
        let normalised = format!("{}\n", body.trim_end());
        std::fs::write(TRACKED_BINDINGS_PATH, normalised)
            .expect("write the normalised bindings.ts");
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
        // [Build-Session-Entscheidung: P2.8] The codegen normalises its output to the repo editorconfig
        // (the `regenerate_committed_bindings` post-process): assert no line carries trailing whitespace
        // and the artifact ends with exactly one final newline, so a codegen-normalisation regression is
        // caught hermetically at L2 (the §0.2 read-the-output-back discipline) before G52 sees it at push.
        for (n, line) in ts.lines().enumerate() {
            assert_eq!(
                line.trim_end(),
                line,
                "committed bindings.ts line {} carries trailing whitespace — the §0.4.5 codegen must normalise it",
                n + 1
            );
        }
        assert!(
            ts.ends_with('\n') && !ts.ends_with("\n\n"),
            "the committed bindings.ts must end with exactly one final newline (editorconfig / G52)"
        );
    }

    // §6.4.1 unit (G15): the §0.4.1 command SURFACE registered at P2.21. The C1–C13 handlers are registered
    // as interface shells on the shared `ipc_specta_builder()`, so the committed bindings.ts (the frontend's
    // only IPC door, §0.7) must expose all 14 commands. Read the committed artifact back (the §0.2
    // read-the-output-back discipline applied to the IPC surface) and assert the `commands` export plus each
    // canonical Tauri command id — so a dropped/renamed registration reddens L2 BEFORE the P2.36 closed-set
    // drift gate (G23) sees it at push. (C1–C13 = 14 commands: §0.4.1's C2 splits into C2a `pick_for_intake`
    // + C2b `pick_destination`.) PINNED BY NAME (not a bare count) so a drop/rename gives a legible diff.
    #[test]
    fn committed_bindings_expose_the_c1_c13_command_surface() {
        let ts = std::fs::read_to_string(TRACKED_BINDINGS_PATH).expect(
            "the committed src/lib/ipc/bindings.ts must exist — regenerate it via the xtask codegen task",
        );
        assert!(
            ts.contains("export const commands"),
            "the committed bindings.ts must expose the `commands` IPC surface (§0.4.1 / P2.21)"
        );
        // The canonical Tauri command ids = the snake_case `invoke(...)` names = the registered Rust fn
        // names, one per §0.4.1 row C1..C13. The double-quoted form matches only the generated `invoke`
        // call, never the back-ticked command name inside a doc comment.
        for cmd in [
            "ingest_paths",      // C1
            "pick_for_intake",   // C2a
            "pick_destination",  // C2b
            "get_targets",       // C3
            "plan_output",       // C4
            "set_destination",   // C5
            "start_conversion",  // C6
            "cancel_run",        // C7
            "get_run_summary",   // C8
            "open_path",         // C9
            "open_project_page", // C10
            "get_app_info",      // C11
            "get_engine_health", // C12
            "cancel_ingest",     // C13
        ] {
            assert!(
                ts.contains(&format!("\"{cmd}\"")),
                "the §0.4.1 command `{cmd}` must be registered in the committed bindings.ts (the P2.21 surface)"
            );
        }
    }

    // §6.4.1 unit (G15): the §0.4.1 closed-set IPC-surface drift gate's GOLDEN (P2.36). `plan-lint` check 12
    // (the L2 `doc12_ipc_surface_drift`) diffs the registered `#[tauri::command]` fn set in `src-tauri/src`
    // against the committed `src-tauri/ipc-commands.golden`, flagging any SPURIOUS WebView-reachable command
    // (a registered fn absent from the golden — the `names - want` direction). This test pins the OTHER
    // direction at the core level: the golden lists EXACTLY the §0.4.1 C1–C13 command set (the same 14 fn names
    // the surface test above pins in the bindings + `collect_commands!` registers), so a golden that silently
    // drifts (a missing entry — which would drop a real command from check 12's `want` set — or an extra /
    // typo'd entry) reddens L1/L2 here. Together with check 12 (registered ⊆ golden) + the surface test
    // (bindings ⊇ C1–C13), the IPC surface is asserted complete + drift-free (no extra, no missing).
    // [Build-Session-Entscheidung: P2.36]
    #[test]
    fn golden_lists_exactly_the_c1_c13_command_surface() {
        let golden = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/ipc-commands.golden"))
            .expect("src-tauri/ipc-commands.golden must exist — the P2.36 closed-set IPC-surface drift golden");
        let listed: std::collections::BTreeSet<&str> = golden.split_whitespace().collect();
        let expected: std::collections::BTreeSet<&str> = [
            "ingest_paths",      // C1
            "pick_for_intake",   // C2a
            "pick_destination",  // C2b
            "get_targets",       // C3
            "plan_output",       // C4
            "set_destination",   // C5
            "start_conversion",  // C6
            "cancel_run",        // C7
            "get_run_summary",   // C8
            "open_path",         // C9
            "open_project_page", // C10
            "get_app_info",      // C11
            "get_engine_health", // C12
            "cancel_ingest",     // C13
        ]
        .into_iter()
        .collect();
        assert_eq!(
            listed, expected,
            "§0.4.1: the committed ipc-commands.golden must list EXACTLY the 14 C1–C13 command fn names — \
             plan-lint check 12 diffs the registered #[tauri::command] set against it, so a drifted golden \
             would silently weaken the closed-set IPC-surface gate (P2.36)"
        );
    }

    // §6.4.1 unit (G15): the §0.4.2 app:// event PAYLOADS (P2.39) mirror to the committed bindings.ts as
    // named types so the TS `listen(<name>)` side type-checks rather than `any` — the raw-event registration
    // via `register_ipc_event_types` (`collect_events!` is avoided; it would force an `any`-bearing
    // `makeEvent` helper, the P2.22 Throw-to-avoid-any precedent). app://fault → AppFault, app://intake →
    // IntakePayload (app://close-requested carries `()`, no payload type). Read the committed artifact back
    // (the §0.2 read-the-output-back discipline). The CLOSED-SET "exactly three, no fourth" assertion is
    // P2.41/G23's; this pins the two payload types are PRESENT + named.
    #[test]
    fn committed_bindings_expose_the_app_event_payloads() {
        let ts = std::fs::read_to_string(TRACKED_BINDINGS_PATH).expect(
            "the committed src/lib/ipc/bindings.ts must exist — regenerate it via the xtask codegen task",
        );
        for ty in ["AppFault", "IntakePayload"] {
            assert!(
                ts.contains(&format!("export type {ty}")),
                "the §0.4.2 app:// event payload `{ty}` must be a named `export type` in the committed bindings.ts (P2.39)"
            );
        }
    }
}

#[cfg(test)]
mod window_model {
    //! §7.3.1 window model — the single `main` window is CONFIG-DECLARED in `tauri.conf.json`
    //! (`app.windows[main]`, P1.19), created + shown by Tauri at startup; the core adds no programmatic
    //! window-builder call. This structural L1/L2 test (no display) LOCKS the §7.3.1 model: exactly one
    //! window labeled `main`, a sensible default size, not fullscreen, no secondary window, no tray. The
    //! rendered frame is P1.23+P1.31; the headed-E2E is P9. [Build-Session-Entscheidung: P1.16]
    use serde_json::Value;

    /// The committed `src-tauri/tauri.conf.json` — the single home of the §7.3.1 window model (P1.19),
    /// resolved from this crate's compile-time manifest dir so it is independent of the process CWD (the
    /// same CWD-independent pattern as `bindings_codegen::TRACKED_BINDINGS_PATH`).
    const TAURI_CONF_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json");

    /// Parse the committed config. A malformed/absent file fails here loudly (the file's parse-validity
    /// is independently guarded by the G47 CSP/capability lint + the live `generate_context!` in `main`;
    /// this test owns the §7.3.1 MODEL content those do not assert).
    fn tauri_conf() -> Value {
        let raw = std::fs::read_to_string(TAURI_CONF_PATH).expect(
            "the committed src-tauri/tauri.conf.json must exist — the §7.3.1 window model home (P1.19)",
        );
        serde_json::from_str(&raw).expect("tauri.conf.json must be valid JSON (§7.3.1 / G47)")
    }

    // §6.4.1 unit (G15): the §7.3.1 config-declared model — exactly ONE window, labeled `main`, with a
    // sensible default size, not fullscreen (a foreground windowed tool; §7.3.1 "opens at a sensible
    // default size each launch"). "No secondary windows in v1" = the array holds exactly one entry (the
    // About screen is an in-app route, §5.9, not an OS window).
    #[test]
    fn single_main_window_with_default_size() {
        let conf = tauri_conf();
        let windows = conf["app"]["windows"]
            .as_array()
            .expect("§7.3.1: `app.windows` must be a declared array");
        assert_eq!(
            windows.len(),
            1,
            "§7.3.1: exactly one main window in v1 — no secondary windows (About is an in-app route, §5.9)"
        );
        let main = windows
            .first()
            .expect("§7.3.1: the single declared `main` window");
        assert_eq!(
            main["label"], "main",
            "§7.3.1: the single window is labeled `main` (referenced by the §7.1.1 focus hand-off + §5)"
        );
        assert!(
            main["width"].as_f64().is_some_and(|w| w > 0.0),
            "§7.3.1/§7.4: a sensible default window width must be declared (opens at a default size each launch)"
        );
        assert!(
            main["height"].as_f64().is_some_and(|h| h > 0.0),
            "§7.3.1/§7.4: a sensible default window height must be declared (opens at a default size each launch)"
        );
        assert_ne!(
            main["fullscreen"],
            Value::Bool(true),
            "§7.3.1: the foreground tool opens windowed at a default size, never fullscreen"
        );
    }

    // §7.3.1 [REC]: no tray icon / no background-agent mode in v1 — ConvertIA is a foreground tool,
    // closing the window quits the app (the §7.3.3 path). A declared `app.trayIcon` would model a tray
    // resident — the §7.3.1 anti-pattern ("closer to an installed service").
    #[test]
    fn no_tray_icon_declared() {
        let conf = tauri_conf();
        assert!(
            conf["app"].get("trayIcon").is_none(),
            "§7.3.1: no tray icon in v1 — a foreground tool, closing the window quits it (not a tray resident)"
        );
    }

    // §7.3.1: the single window is "created by Tauri at startup" from config — the core adds NO
    // programmatic window builder. Scan the production boot source (the shared `boot_invariants` helper,
    // truncated at the first `#[cfg(test)]`, so these needles can never self-match) for the Tauri v2
    // programmatic window-creation constructors. Needles assembled by `concat!` for the same
    // self-match-avoidance as the `boot_invariants` net-primitive scan.
    #[test]
    fn no_programmatic_window_builder() {
        let src = super::boot_invariants::production_boot_source();
        let builder_ctors = [
            concat!("Window", "Builder"), // WebviewWindowBuilder / WindowBuilder (the builder types)
            concat!("Webview", "Builder"), // WebviewBuilder (the lower-level builder type)
            concat!("WebviewWindow", "::", "builder"), // WebviewWindow::builder() (the method ctor)
        ];
        for ctor in builder_ctors {
            assert!(
                !src.contains(ctor),
                "§7.3.1: the `main` window is config-declared (P1.19) — the core must add no programmatic `{ctor}` window creation"
            );
        }
    }
}

#[cfg(test)]
mod no_updater_posture {
    //! §7.6.1: the Tauri updater is explicitly absent — "its absence is the implementation". This
    //! structural test asserts the no-updater posture BY CONSTRUCTION at the Rust level: the resolved
    //! dependency graph carries no `tauri-plugin-updater` (direct or transitive), and the Builder
    //! registers no updater plugin. It is the cargo-test-plane / co-located companion behind the live
    //! enforcers — the G18 `cargo-deny` ban (deny.toml denies `tauri-plugin-updater`) and the G47 config
    //! lint (no `updater`/pubkey block, no `createUpdaterArtifacts` in tauri.conf.json) — the same
    //! defense-in-depth shape as P1.15.1's `boot_invariants` standing in for the Lane-B egress gate. The
    //! config side (tauri.conf.json) is G47's; this owns the Cargo-graph + Builder side §7.6.1 names but
    //! G47 (a conf-parse gate) cannot reach. [Build-Session-Entscheidung: P1.18]

    /// The committed workspace `Cargo.lock` (repo root), resolved from this crate's compile-time
    /// manifest dir so it is CWD-independent (the same pattern as `bindings_codegen::TRACKED_BINDINGS_PATH`).
    const CARGO_LOCK_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../Cargo.lock");

    // §7.6.1 / §6.4.1 unit (G15): `tauri-plugin-updater` is in the resolved dependency graph NOWHERE
    // (direct or transitive) — its absence is the implementation. Scan the committed lock for a
    // `[[package]]` of that name; the machine-generated `name = "…"` form is unambiguous (no
    // false-positive from prose). The cargo-test-plane companion to the G18 cargo-deny ban (which fires
    // only when cargo-deny actually runs). Needle via `concat!` so the token is not a literal in this
    // scanned production file.
    #[test]
    fn no_updater_crate_in_dependency_graph() {
        let lock = std::fs::read_to_string(CARGO_LOCK_PATH)
            .expect("the committed Cargo.lock must exist — the §7.6.1 no-updater posture");
        let needle = concat!("name = \"tauri-plugin-", "updater\"");
        assert!(
            !lock.contains(needle),
            "§7.6.1: `tauri-plugin-updater` must be in the dependency graph nowhere — its absence is the implementation (denied by the G18 cargo-deny ban)"
        );
    }

    // §7.6.1 / §6.4.1 unit (G15): the Builder registers no updater plugin. Scan the production boot
    // source (the shared `boot_invariants` helper, truncated at the first `#[cfg(test)]`, so this
    // needle can never self-match) for any updater plugin reference — `tauri_plugin_updater` contains
    // `plugin_updater`, so the one needle catches the crate path and any `.plugin(...)` registration.
    // Needle via `concat!` (the established self-match-avoidance).
    #[test]
    fn builder_registers_no_updater_plugin() {
        let src = super::boot_invariants::production_boot_source();
        let needle = concat!("plugin_", "updater");
        assert!(
            !src.contains(needle),
            "§7.6.1: the Builder must register no updater plugin (`{needle}`) — the updater is explicitly absent"
        );
    }
}
