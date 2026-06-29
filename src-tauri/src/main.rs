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

/// [Build-Session-Entscheidung: P1.15] The §7.2.1 step-2 boot context: the three resolved base dirs
/// (config / local-data scratch / log), held as app-managed state (`app.manage`). Its home is the
/// binary root for P1 because the §7.2.1 ordered startup-sequence MODULE is the P2 app-shell spine —
/// §0.7 has no app-shell module yet and P1 must not add an unsanctioned folder (CLAUDE §1a / G69); P2
/// relocates this type into that spine. The fields are read by the P2 ordered spine + subsequent phases
/// (none of the dirs is created here — directory creation is §7.2.1 step 5). The per-launch InstanceId
/// is NOT a field here: P2.47 promoted it to its own §7.1.2 app-managed singleton (`State<InstanceId>`).
#[derive(Debug)]
#[expect(
    dead_code,
    reason = "§7.2.1 step-2 boot context — the three resolved base dirs (config/scratch/log); read by the P2 ordered startup spine + later phases (P1.15). The per-launch InstanceId is NO LONGER a field here: P2.47 promoted it to its own app-managed singleton (§7.1.2 'app-managed singleton via app.manage(...)')."
)]
struct StartupContext {
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
/// handler, and `app.emit`). P2.40 authored the §0.4.2 / §7.8.1 `app://intake` IDLE-path-only RULE as a
/// pure disposition; [Build-Session-Entscheidung: P2.54] adds the `forward_launch_intake` funnel that
/// CONSUMES it (every launch-time path source routes here) + the §7.8.1 `parse_path_args` /
/// `forward_launch_argv` argv classifier. The per-OS live callers — the single-instance callback (P2.52),
/// the macOS `RunEvent::Opened` handler (P2.56), the first-launch argv reader (P2.57) — wire the funnel
/// from P2.52 onward.
mod launch_intake {
    // [Test-Change: P2.52 — old-obsolete+new-correct, §7.1.1] G70 flags the REMOVED module dead-code lint
    // attribute (the `#![cfg_attr(not(test), …)]` above) as a "removed assertion" — a false positive (a LINT
    // attribute, never a test assertion); the dead-code expectation is OBSOLETE now the §7.1.1 callback makes
    // the module live, so removing it is CORRECT (keeping it errors "expectation unfulfilled"). No test changed.
    // [Build-Session-Entscheidung: P2.52] The §7.1.1 single-instance callback in `main()` (below) is now the
    // funnel's FIRST live caller — `forward_launch_argv` -> `forward_launch_intake` -> the predicate fns +
    // `parse_path_args` + `intake_disposition` are all reachable from production — so the whole module is LIVE
    // and the former module-level `#![cfg_attr(not(test), …)]` dead-code suppression is REMOVED (with nothing
    // dead, that `dead_code` expectation would flip to "expectation unfulfilled" under `-D warnings`). The
    // `converter_is_busy` predicate now reads the real §1.9 run-state via the RunRegistry (P2.55) and the
    // `PendingIntake` buffer arm is real (P2.58); the remaining fill is P2.59 (the `frontend_ready` ready-flag
    // branch). Until P3 populates runs the registry is empty (not busy), so an idle launch set routes — via
    // `frontend_ready`'s shell `false` — to the real buffer, never lost.

    use std::path::PathBuf;

    use tauri::{AppHandle, Emitter, Manager};

    use crate::domain::{IntakeOrigin, IntakePayload};
    use crate::ipc::events;
    use crate::orchestrator::{PendingIntake, RunRegistry};

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

    /// [Build-Session-Entscheidung: P2.54] The §7.8.1 single funnel — EVERY launch-time path source (the
    /// §7.1.1 single-instance argv callback, the macOS `RunEvent::Opened` handler, the first-launch argv
    /// reader) routes here, so the §7.1.1 refuse-busy gate and the §7.8.1 first-launch buffer-replay are
    /// enforced ONCE, not duplicated per hook. The disposition is the pure `intake_disposition` rule (P2.40,
    /// already truth-table-tested), NOT a re-inlined copy of the illustrative §7.8.1 if/else: the funnel
    /// resolves the two run-time predicates against the `AppHandle` and dispatches on the result.
    fn forward_launch_intake(app: &AppHandle, paths: Vec<PathBuf>, origin: IntakeOrigin) {
        // §7.8.1: an empty path set (a bare relaunch with no files) is a no-op.
        if paths.is_empty() {
            return;
        }
        match intake_disposition(converter_is_busy(app), frontend_ready(app)) {
            // §7.1.1 PRIMARY refuse-busy: a mid-run second launch / Open-with is dropped core-side — no
            // `app://intake` emit, no `PendingIntake` buffer — so the §2.4 frozen set is never mutated mid-run.
            IntakeDisposition::Drop => {}
            // Idle + the WebView listener is ready: emit the §0.4.2 `app://intake` event (via the
            // `crate::ipc::events` constant, never a re-spelled literal — plan-lint check 28) so the UI
            // mirrors a drop (§5.2/§1.1). The payload is `{ paths, origin }` so the frontend re-calls C1
            // with the right `IntakeOrigin`.
            IntakeDisposition::Emit => {
                app.emit(events::APP_INTAKE, IntakePayload { paths, origin })
                    .ok();
            }
            // Idle but the WebView is not-yet-ready (the §7.8.1 first-launch listener race): stash the
            // paths + origin for the drain-on-mount replay.
            IntakeDisposition::Buffer => buffer_pending_intake(app, paths, origin),
        }
    }

    /// [Build-Session-Entscheidung: P2.54.1] The §7.8.1 `argv` classifier — split launch FLAG tokens from
    /// file-PATH tokens. `argv[0]` (the program path) is skipped; the §7.5.3 `--verbose` diagnostic switch
    /// and any `-`/`--`-prefixed token are launch switches (never ingestable paths); a relative path
    /// resolves against the launching `cwd`. The §1.1 freeze re-validates (canonicalises / resolve-identity
    /// / detects) every returned path, so this is CLASSIFICATION, not a trust boundary — but the flag-vs-path
    /// split + the cwd-relative resolution are genuinely homed here. `PathBuf` is platform-aware, so the
    /// Win-vs-Linux separator / argv conventions are handled by construction.
    fn parse_path_args(argv: &[String], cwd: &str) -> Vec<PathBuf> {
        argv.iter()
            .skip(1)
            .filter(|tok| !is_launch_switch(tok))
            .map(|tok| resolve_launch_path(tok, cwd))
            .collect()
    }

    /// [Build-Session-Entscheidung: P2.54.1] A launch switch (never an ingestable path): the §7.5.3
    /// `--verbose` flag and any `-`/`--`-prefixed token. A launcher / desktop-entry passes a file as an
    /// OS-expanded path (absolute, or a plain relative name), never as a `-`-leading bare argument, so the
    /// leading-`-` test cleanly separates switches from paths.
    fn is_launch_switch(tok: &str) -> bool {
        tok.starts_with('-')
    }

    /// [Build-Session-Entscheidung: P2.54.1] Resolve a launch path token: an absolute path is kept as-is; a
    /// relative one is joined onto the launching `cwd` (§7.8.1). The §1.1 freeze canonicalises later, so a
    /// plain join is the classification step. `PathBuf::is_absolute` is platform-aware (Win drive-letter vs
    /// POSIX root), matching the native argv each OS delivers.
    fn resolve_launch_path(tok: &str, cwd: &str) -> PathBuf {
        let path = PathBuf::from(tok);
        if path.is_absolute() {
            path
        } else {
            PathBuf::from(cwd).join(path)
        }
    }

    /// [Build-Session-Entscheidung: P2.54.1] The §7.8.1 `forward_launch_argv` wrapper — classify `argv` into
    /// paths (`parse_path_args`) and route them through the single funnel. The §7.1.1 single-instance
    /// callback (P2.52) and the Win/Linux first-launch argv reader (P2.57) both forward through this.
    /// `pub(super)` [P2.52]: the funnel's argv door for `main()`'s single-instance callback (the first live caller).
    pub(super) fn forward_launch_argv(
        app: &AppHandle,
        argv: &[String],
        cwd: &str,
        origin: IntakeOrigin,
    ) {
        forward_launch_intake(app, parse_path_args(argv, cwd), origin);
    }

    /// [Build-Session-Entscheidung: P2.52] The §7.1.1 single-instance second-launch HANDLER — re-focus the
    /// `main` window (the OS bringing ConvertIA forward IS the PRIMARY "busy" signal, §7.1.1) + forward the
    /// second launch's argv through the §7.8.1 funnel as `SecondInstance`. The funnel owns the §7.1.1
    /// refuse-busy gate + the emit/buffer disposition (a mid-run second launch is dropped, never merged into
    /// the §2.4 frozen set), so nothing is emitted here. Homed as a NAMED `&AppHandle` fn (NOT an inline
    /// `init` closure): the boot-stage pattern keeps AppHandle-coupled glue in named fns — `main()` passes it
    /// to `tauri_plugin_single_instance::init` directly, and the P2.135 boot-glue G28 exemption (which is
    /// AppHandle-fn-SIGNATURE-based) then covers it (an inline closure is NOT a fn, so it would not be exempt).
    pub(super) fn on_second_instance(app: &AppHandle, argv: Vec<String>, cwd: String) {
        let _ = app.get_webview_window("main").map(|w| {
            let _ = w.set_focus();
        });
        forward_launch_argv(app, &argv, &cwd, IntakeOrigin::SecondInstance);
    }

    /// [Build-Session-Entscheidung: P2.55] The §7.1.1 PRIMARY refuse-busy predicate — reads the real §1.9
    /// run-level state (the same predicate §7.3.2's close-requested guard uses): a conversion run is in flight
    /// iff the `RunRegistry` (crate::orchestrator) holds an active token (registered at C6, dropped at
    /// `RunFinished`; a cancelling run stays busy until `finish`). When busy, `intake_disposition` returns
    /// `Drop`, so the funnel refuses a mid-run second-launch / Open-with — no `app://intake` emit, no
    /// `PendingIntake` buffer — and the §2.4 frozen set is never mutated mid-run. AppHandle-coupled boot-glue
    /// (§1.1a; G28 signature-exempt; the source-scan pins the RunRegistry query). `app.state::<RunRegistry>()`
    /// is infallible by construction (registered in main()'s Builder chain). The runs are POPULATED by the
    /// P3.46 conductor (register-at-C6 / finish-at-RunFinished), so until P3 the registry is empty → not busy →
    /// the funnel's idle-flow is open (the buffer P2.58 made real catches an idle-and-not-ready set) — the
    /// owner-confirmed P2.58-before-P2.55 order in effect.
    fn converter_is_busy(app: &AppHandle) -> bool {
        app.state::<RunRegistry>().has_active_run()
    }

    /// [Build-Session-Entscheidung: P2.54] INTERFACE SHELL for the §7.8.1 WebView-ready flag — the real
    /// wiring is box P2.59 (the ready-flag branch). The fail-SAFE default is `false`: an unwired ready-check
    /// reports NOT-ready, so `intake_disposition` returns `Buffer` rather than `Emit` — the funnel never
    /// emits `app://intake` into a listener that may not exist (a dropped first-launch event, the §7.8.1
    /// race). Buffered paths are stashed in `PendingIntake` (P2.58, real) and drained by C1 `drainPending`
    /// (P2.60). REACHED from P2.55: with the run registry empty (pre-P3) `converter_is_busy` is `false`, so an
    /// idle launch set reaches this shell → `Buffer` → the real `PendingIntake` stash (never lost); P2.59
    /// replaces this `false` with the live WebView-ready flag so a ready listener gets `Emit` instead.
    fn frontend_ready(_app: &AppHandle) -> bool {
        false
    }

    /// [Build-Session-Entscheidung: P2.58] The §7.8.1 first-launch `Buffer` arm — stash the idle-and-not-ready
    /// launch set into the app-managed `State<PendingIntake>` (`crate::orchestrator`) for the C1 `drainPending`
    /// replay (P2.60), closing the §7.8.1 first-launch listener race. AppHandle-coupled boot-glue (the §1.1a
    /// boot-stage pattern — not cargo-test execution-testable; the stash LOGIC is unit-tested on
    /// `PendingIntake`, this WIRING is source-scan-pinned + G28 boot-glue-exempt). `app.state::<PendingIntake>()`
    /// is infallible by construction: `main()` registers the buffer in the Builder chain BEFORE the event loop
    /// (so before any single-instance callback), so the resolve cannot fail at runtime — no panic despite the
    /// crate-root `clippy::panic` deny. REACHED from P2.55: with the run registry empty (pre-P3)
    /// `converter_is_busy` is `false`, so an idle-and-not-ready launch set now routes into this real stash —
    /// the owner-confirmed order landed this buffer (P2.58) BEFORE P2.55 opened idle-flow, so no idle set
    /// ever routed into a no-op.
    fn buffer_pending_intake(app: &AppHandle, paths: Vec<PathBuf>, origin: IntakeOrigin) {
        app.state::<PendingIntake>().stash(paths, origin);
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

        // §6.4.1 unit (G15): the §7.8.1 funnel + classifier exist with their spec'd signatures. The
        // AppHandle-coupled fns (the funnel, the wrapper, the three predicate/buffer shells) are not
        // call-tested — this crate has no `tauri::test` mock harness (the boot-stage pattern) — so this
        // pins their SIGNATURES via fn-pointer coercion: a signature drift fails to compile. (From P2.52 the
        // module is LIVE via the single-instance callback in `main()`, so this is now purely a signature pin —
        // the former dead-code-suppression role ended when P2.52 removed that module suppression.)
        // [Build-Session-Entscheidung: P2.54]
        #[test]
        fn launch_funnel_items_have_their_spec_signatures() {
            let _intake: fn(&AppHandle, Vec<PathBuf>, IntakeOrigin) = forward_launch_intake;
            let _argv: fn(&AppHandle, &[String], &str, IntakeOrigin) = forward_launch_argv;
            let _busy: fn(&AppHandle) -> bool = converter_is_busy;
            let _ready: fn(&AppHandle) -> bool = frontend_ready;
            let _buffer: fn(&AppHandle, Vec<PathBuf>, IntakeOrigin) = buffer_pending_intake;
        }

        // §6.4.1 unit (G15): the §7.1.1 single-instance second-launch HANDLER (P2.52). `on_second_instance`
        // (in `launch_intake`, covered by `production_boot_source`) re-focuses `main` + forwards the argv as
        // `SecondInstance`; `main()` (covered by `production_main_body`) WIRES it into the single-instance
        // `init`. Both are AppHandle-coupled boot-glue (not execution-testable; the boot-stage pattern), so
        // source scans pin them. The re-focus + `SecondInstance` needles appear only in `on_second_instance`'s
        // body (`forward_launch_argv`'s signature carries `origin: IntakeOrigin`, not `::SecondInstance`), and
        // the `on_second_instance` def lives in `launch_intake` (outside `production_main_body` = main()-only),
        // so the wiring needle matches only the `init` call site — non-blind. Needles `concat!`-assembled, and
        // every test module is excluded from both helpers. [Build-Session-Entscheidung: P2.52]
        #[test]
        fn single_instance_handler_refocuses_forwards_and_is_wired() {
            let handler = crate::boot_invariants::production_boot_source();
            for needle in [
                concat!("get_webview_", "window(\"main\")"), // re-focus: looks up the main window
                concat!("set_", "focus"), // re-focuses it (the §7.1.1 primary busy signal)
                concat!("IntakeOrigin::Second", "Instance"), // forwards as SecondInstance (§7.8 / §0.6)
            ] {
                assert!(
                    handler.contains(needle),
                    "§7.1.1: on_second_instance must re-focus `main` + forward the argv as SecondInstance (missing `{needle}`)"
                );
            }
            assert!(
                crate::boot_invariants::production_main_body()
                    .contains(concat!("on_second_", "instance")),
                "§7.1.1: main() must wire on_second_instance into the single-instance init"
            );
        }

        // §6.4.1 unit (G15): the §7.8.1 funnel DISPATCHES via the pure P2.40 `intake_disposition` rule
        // (resolving both predicates against the AppHandle), NOT a re-inlined copy of the illustrative
        // §7.8.1 if/else — the DRY property the owner confirmed. Scan the production source (the shared
        // `boot_invariants` helper, truncated before the FIRST `cfg(test)` module = launch_intake's own
        // tests, so the needle can never self-match this file); the call substring does not appear in
        // `intake_disposition`'s own `fn` definition. Needle `concat!`-assembled (the established
        // self-match-avoidance). [Build-Session-Entscheidung: P2.54]
        #[test]
        fn funnel_dispatches_via_the_pure_disposition_rule() {
            let src = crate::boot_invariants::production_boot_source();
            let dispatch = concat!("intake_disposition(converter_is_", "busy(app)");
            assert!(
                src.contains(dispatch),
                "§7.8.1: forward_launch_intake must dispatch via the pure intake_disposition rule (P2.40), \
                 resolving the predicates against the AppHandle — not a re-inlined if/else"
            );
        }

        // §6.4.1 unit (G15): the §7.8.1 `parse_path_args` classifier (P2.54.1) — argv[0] skipped, relative
        // paths cwd-resolved. Platform-robust: relative tokens only (PathBuf join is deterministic per
        // platform), so the assertion holds identically on Win/Linux/macOS.
        #[test]
        fn parse_path_args_skips_argv0_and_resolves_relative() {
            let cwd = "base-dir";
            let argv = vec![
                "convertia".to_string(),
                "a.txt".to_string(),
                "sub/b.png".to_string(),
            ];
            assert_eq!(
                parse_path_args(&argv, cwd),
                vec![
                    PathBuf::from(cwd).join("a.txt"),
                    PathBuf::from(cwd).join("sub/b.png"),
                ],
                "§7.8.1: argv[0] is skipped and relative paths resolve against the launching cwd"
            );
        }

        // §6.4.1 unit (G15): the §7.5.3 `--verbose` diagnostic switch and any `-`/`--`-prefixed token are
        // launch SWITCHES, never ingestable paths — stripped before the cwd resolution.
        #[test]
        fn parse_path_args_strips_launch_switches() {
            let cwd = "base";
            let argv = vec![
                "convertia".to_string(),
                "--verbose".to_string(),
                "-x".to_string(),
                "keep.txt".to_string(),
            ];
            assert_eq!(
                parse_path_args(&argv, cwd),
                vec![PathBuf::from(cwd).join("keep.txt")],
                "§7.5.3/§7.8.1: --verbose and any -/-- launch switch is stripped, never an ingestable path"
            );
        }

        // §6.4.1 unit (G15): empty / argv0-only / flags-only inputs yield NO paths (no panic, no spurious
        // path). The empty-argv case also exercises `skip(1)` on an empty slice (no underflow).
        #[test]
        fn parse_path_args_yields_no_paths_for_empty_or_flags_only() {
            let cwd = "base";
            let empty: Vec<String> = Vec::new();
            assert!(
                parse_path_args(&empty, cwd).is_empty(),
                "empty argv → no paths"
            );
            assert!(
                parse_path_args(&["convertia".to_string()], cwd).is_empty(),
                "argv0-only → no paths (the program path is never ingested)"
            );
            let flags_only = vec!["convertia".to_string(), "--verbose".to_string()];
            assert!(
                parse_path_args(&flags_only, cwd).is_empty(),
                "flags-only → no paths"
            );
        }

        // §6.4.1 unit (G15): an absolute launch path is kept as-is (NOT joined onto cwd); the relative case
        // is covered above. `PathBuf::is_absolute` is platform-aware, so the absolute fixture is
        // `cfg!`-selected to a native absolute path for the build target.
        #[test]
        fn parse_path_args_keeps_absolute_paths_unjoined() {
            let cwd = "base";
            let abs = if cfg!(windows) {
                "C:\\abs\\f.txt"
            } else {
                "/abs/f.txt"
            };
            let argv = vec!["convertia".to_string(), abs.to_string()];
            assert_eq!(
                parse_path_args(&argv, cwd),
                vec![PathBuf::from(abs)],
                "§7.8.1: an absolute launch path is kept as-is, never joined onto cwd"
            );
        }

        // §6.4.1 unit (G15): the §7.8.1 `Buffer` arm is WIRED into the `State<PendingIntake>` stash (P2.58) —
        // not the P2.54 no-op shell. `buffer_pending_intake` is AppHandle-coupled boot-glue (the §1.1a
        // boot-stage pattern — not cargo-test execution-testable; the stash/take LOGIC is unit-tested on
        // `crate::orchestrator::PendingIntake`), so a source-scan pins it resolves the managed buffer + stashes;
        // a second scan pins `main()` REGISTERS the buffer (else the resolve would fail). Needles
        // `concat!`-assembled (the established self-match avoidance). [Build-Session-Entscheidung: P2.58]
        #[test]
        fn buffer_arm_stashes_into_the_managed_pending_intake() {
            let buffer_src = crate::boot_invariants::production_boot_source();
            assert!(
                buffer_src.contains(concat!("state::<Pending", "Intake>()")),
                "§7.8.1: buffer_pending_intake must resolve the managed State<PendingIntake> (P2.58)"
            );
            assert!(
                buffer_src.contains(concat!(".stash(", "paths, origin)")),
                "§7.8.1: buffer_pending_intake must stash the launch set + origin (not the P2.54 no-op shell)"
            );
            let main_src = crate::boot_invariants::production_main_body();
            assert!(
                main_src.contains(concat!(".manage(crate::orchestrator::Pending", "Intake::default())")),
                "§7.8.1: main() must register the State<PendingIntake> so the buffer-arm resolve cannot fail (P2.58)"
            );
        }

        // §6.4.1 unit (G15): the §7.1.1 refuse-busy predicate is WIRED to the real §1.9 run-state (P2.55) — not
        // the P2.54 fail-safe `true` shell. `converter_is_busy` is AppHandle-coupled boot-glue (the §1.1a
        // boot-stage pattern — not cargo-test execution-testable; the `has_active_run` LOGIC is unit-tested on
        // `crate::orchestrator::RunRegistry`), so a source-scan pins it resolves the managed RunRegistry +
        // queries it; a second scan pins `main()` REGISTERS it. Needles `concat!`-assembled. [Build-Session-Entscheidung: P2.55]
        #[test]
        fn busy_gate_reads_the_managed_run_registry() {
            let buffer_src = crate::boot_invariants::production_boot_source();
            assert!(
                buffer_src.contains(concat!("state::<Run", "Registry>()")),
                "§7.1.1: converter_is_busy must resolve the managed State<RunRegistry> (P2.55)"
            );
            assert!(
                buffer_src.contains(concat!(".has_active_", "run()")),
                "§7.1.1: converter_is_busy must read the real §1.9 run-state, not the P2.54 fail-safe `true`"
            );
            let main_src = crate::boot_invariants::production_main_body();
            assert!(
                main_src.contains(concat!(".manage(crate::orchestrator::Run", "Registry::default())")),
                "§7.1.1: main() must register the State<RunRegistry> so the busy-gate resolve cannot fail (P2.55)"
            );
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
        // [Build-Session-Entscheidung: P1.14] §0.8 plugin wiring. single-instance is registered FIRST (§7.1)
        // so a second launch is intercepted before the other plugins init; ConvertIA is desktop-only so no
        // `#[cfg(desktop)]` guard is needed. [Build-Session-Entscheidung: P2.52] the callback is now WIRED
        // (was empty in P1.14): it re-focuses the `main` window + forwards the second launch's argv through
        // the §7.8.1 funnel as `SecondInstance` (the §7.1.1 hand-off). dialog + opener are called Rust-side
        // (DialogExt/OpenerExt) so they take NO WebView grant (§0.10); store/log get store:default/log:default
        // in capabilities/main.json (P1.21).
        //
        // [Build-Session-Entscheidung: P2.51] §7.1.1 single-instance LOCK SCOPE — per-OS-user, NOT
        // machine-global. The PLUGIN owns the lock (this box adds no locking logic), so the scope is a
        // property of `tauri-plugin-single-instance`'s per-platform mechanism, documented here in-core:
        // Windows = a per-Session `CreateMutexW`, Linux = the session `D-Bus` — both PER-OS-USER (a second
        // launch by the SAME user reaches THIS instance; two different logged-in OS users each get their own,
        // acceptable since their §2.14 scratch + output locations are user-scoped anyway). macOS is the SOLE
        // gap: the plugin hard-codes its socket at world-writable `/tmp/{id}_si.sock` (machine-global), so the
        // per-OS-user scope is NOT achievable there — the accepted v1 limitation recorded as §0.11 threat
        // class T13 (the macOS PRIMARY single-instance path is the §7.8 AppleEvent, unaffected; the /tmp
        // socket covers only direct-binary re-exec, the least-mature leg). v1 adds NO per-user-`$TMPDIR`
        // macOS socket — T13 records that heavier path as the one not chosen. The per-platform scope mapping
        // is pinned structurally by the `single_instance_lock_scope` test module below.
        // [Build-Session-Entscheidung: P2.52] §7.1.1 second-launch hand-off → the named
        // `launch_intake::on_second_instance` handler (re-focus `main` + forward the second launch's argv
        // through the §7.8.1 funnel as SecondInstance; the funnel owns the refuse-busy gate + emit/buffer).
        // Passed as a fn ITEM, not an inline closure, so the AppHandle-coupled boot-glue lives in a NAMED fn
        // — signature-pinned + G28-exempt (the P2.135 boot-glue exemption is AppHandle-fn-SIGNATURE-based; an
        // inline closure would not be a fn and so would not be exempt).
        .plugin(tauri_plugin_single_instance::init(
            launch_intake::on_second_instance,
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_log::Builder::new().build())
        // [Build-Session-Entscheidung: P2.58] §7.8.1 first-launch intake buffer — register the
        // State<PendingIntake> in the Builder chain (compile-time Default, so registered BEFORE the event
        // loop / any single-instance callback: the funnel's Buffer arm resolve is then infallible by
        // construction, no panic under the crate-root clippy::panic deny). Writer = the launch funnel
        // (buffer_pending_intake); reader = C1 drainPending (P2.60). Ahead of the P3.46-registered run stores
        // because its live consumer — the P2 launch funnel — exists now.
        .manage(crate::orchestrator::PendingIntake::default())
        // [Build-Session-Entscheidung: P2.55] §7.1.1 refuse-busy run-state — register the RunRegistry (the
        // §0.4.4 run-cancellation-token store, P2.42) so converter_is_busy can read it (the SAME registry the
        // P3.46 conductor populates at C6 / drains at RunFinished — this box owns the .manage, P3 wires the
        // register/finish calls). Builder chain (compile-time Default, before the event loop) → the busy-gate
        // resolve is infallible by construction.
        .manage(crate::orchestrator::RunRegistry::default())
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

            // Stage 2 — establish the per-launch InstanceId as an app-managed SINGLETON (§7.1.2: a
            // random v4, the spec's "app-managed singleton via app.manage(...)"; process-local — never
            // persisted, never networked, §2.11) and resolve the three base dirs via app.path() (§7.2.1
            // step 2: config / local-data scratch §2.14 / log §7.5). NO directory is created here
            // (creation is §7.2.1 step 5). Each call below touches only local uuid + filesystem
            // primitives, so the boot path opens no socket (§7.2.2; G29 first-party rule (g) backstops
            // the whole tree; the boot-invariant test covers the top-of-file import surface).
            //
            // [Build-Session-Entscheidung: P2.47] InstanceId is its OWN managed singleton (resolved as
            // State<InstanceId> by the §2.14 scratch-naming / §2.6 cleanup consumers), NOT a
            // StartupContext field — P1.15 bundled it in the boot-context scaffold; P2.47 promotes it to
            // the §7.1.2 standalone form. The mint (random v4) is unit-tested in crate::domain; this
            // establishment is pinned by `instance_identity` below.
            app.manage(InstanceId::mint());
            let startup = StartupContext {
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

    /// The production prefix = this binary's `main.rs` up to the FIRST `cfg(test)` module, so sentinel
    /// needles declared in a test module can never self-match the `include_str!` scan. NOTE: since P2.40
    /// that first `cfg(test)` module is `launch_intake`'s — BEFORE `main()` — so this prefix does NOT reach
    /// `main()`; a scan needing `main()` uses `all_production_source()` (below). `pub(super)`: SHARED with
    /// the §7.3.1 `window_model` scan (P1.16) + `all_production_source()`.
    pub(super) fn production_boot_source() -> &'static str {
        let full = include_str!("main.rs");
        // Take the production prefix before the FIRST `cfg(test)` marker in the file, or the whole file if
        // absent. `split_once` avoids the impossible-`None` dead fallback an `unwrap_or` would carry
        // (`str::split(..).next()` is always `Some`).
        full.split_once("#[cfg(test)]")
            .map_or(full, |(prefix, _)| prefix)
    }

    /// `main()`'s body — from the `main()` definition to the FIRST `cfg(test)` module after it (this one).
    /// `production_boot_source()` stops inside `launch_intake` BEFORE `main()` (P2.40), so it never reaches
    /// `main()`'s Builder chain / §7.2.1 startup spine; this slice does. `pub(super)`: SHARED with
    /// `all_production_source()` + the §7.1.1 `single_instance_lock_scope` registration scan (P2.51).
    /// [Build-Session-Entscheidung: P2.54]
    pub(super) fn production_main_body() -> &'static str {
        let full = include_str!("main.rs");
        let after_main = full.split_once("fn main()").map_or("", |(_, rest)| rest);
        after_main
            .split_once("#[cfg(test)]")
            .map_or(after_main, |(prefix, _)| prefix)
    }

    /// ALL production code a source-scan invariant must cover, every `cfg(test)` module excluded so
    /// sentinel needles never self-match: the pre-test-module prefix PLUS `main()`'s body. Needed because
    /// `production_boot_source()` alone stops at `launch_intake`'s test module — BEFORE `main()` — so a scan
    /// over it is blind to `main()`'s Builder chain / startup spine (every item after `main()` is a
    /// `cfg(test)` module, so prefix + main-body = all production code). [Build-Session-Entscheidung: P2.54]
    /// — fixes a pre-existing P2.40 blindness in the `main()`-targeting scans (`boot_path_opens_no_socket`,
    /// `no_programmatic_window_builder`, `builder_registers_no_updater_plugin`), which scanned only the prefix.
    pub(super) fn all_production_source() -> String {
        format!("{}\n{}", production_boot_source(), production_main_body())
    }

    // §7.2.2 / §6.4.1 unit (G15): a structural assertion that the production boot path references no
    // network primitive — the cargo-test companion to the G29 source rule (g), scoped to §7.2.2. Scans
    // `all_production_source()` (prefix + `main()` body) because the §7.2.1 startup spine lives in `main()`,
    // which `production_boot_source()` alone does not reach (P2.54 — fixes the pre-existing P2.40 blindness).
    #[test]
    fn boot_path_opens_no_socket() {
        let src = all_production_source();
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
mod instance_identity {
    //! §6.4.1 unit (G15): §7.1.2 — the per-launch `InstanceId` is established as an app-managed SINGLETON
    //! (a random v4; process-local, never persisted/networked, §2.11). The §7.2.1-step-2 boot fact is
    //! asserted at the SOURCE plane: the `setup` closure is not live-unit-testable here (this crate has no
    //! `tauri::test` mock harness — the established boot-stage pattern, cf. `boot_invariants` / `window_model`).
    //! The random-v4 MINT itself is unit-tested in `crate::domain` (`instance_id_mint_is_unique_nonnil_v4`);
    //! never-NETWORKED is `boot_invariants::boot_path_opens_no_socket` + the whole-tree G29 rule (g);
    //! never-PERSISTED is structural — the id is minted fresh each launch (random), never loaded from a
    //! store/disk (no load / deserialize-from-persistence constructor exists). This module pins the SINGLETON
    //! ESTABLISHMENT in `main()`'s `setup`, which `production_boot_source()` (truncated before the FIRST
    //! `#[cfg(test)]`, ~line 244) does NOT reach — so it scans the FULL `main.rs`. [Build-Session-Entscheidung: P2.47]

    // §7.1.2 / §2.11: the boot path mints the per-launch InstanceId (random v4) and hands it to
    // app.manage, so the §2.14 scratch-naming / §2.6 cleanup consumers resolve it as State<InstanceId> — a
    // standalone singleton, NOT a StartupContext field (P2.47 promoted it from the P1.15 scaffold).
    #[test]
    fn instance_id_minted_and_managed_as_a_singleton_in_setup() {
        // Scan the WHOLE source (not production_boot_source(), which truncates before the setup closure).
        // The needles are concat!-assembled — and the assert MESSAGES deliberately avoid the literal tokens —
        // so the literal never appears in this test file (which include_str! would otherwise self-match,
        // including via a message: a green-but-blind trap).
        let src = include_str!("main.rs");
        let mint = concat!("InstanceId", "::mint()");
        let managed_singleton = concat!(".manage(", "InstanceId", "::mint())");
        assert!(
            src.contains(mint),
            "§7.1.2: the boot path must mint the per-launch InstanceId as a random v4 (the mint constructor)"
        );
        assert!(
            src.contains(managed_singleton),
            "§7.1.2: the minted InstanceId must be established as an app-managed singleton (handed to \
             app.manage) — so the §2.14/§2.6 consumers resolve it as State<InstanceId>, not via StartupContext"
        );
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
mod app_event_closed_set {
    //! §6.4.1 unit (G15): the §0.4.2 app:// event CLOSED-SET + payload-registration cross-check (P2.41 — the
    //! loose-`G23` closed-set gate's IN-CORE half). This is the §0.4.1-command analog of the Rust golden test
    //! `bindings_codegen::golden_lists_exactly_the_c1_c13_command_surface` that `plan-lint` check 12 pairs
    //! with: there, the L2 source scan (check 12) proves "no spurious registered command"; here, the L2 source
    //! scan (`plan-lint` check 28 `app-event-surface-drift`, build-gates §6) proves no fourth app:// literal
    //! exists anywhere in `src-tauri/src` and that every such literal lives only in `crate::ipc::events`, and
    //! THIS cross-check pins the in-core side a text scan cannot reach: each §0.4.2 event's payload type is
    //! authored and `.types()`-registered (via `register_ipc_event_types`), so the TS `listen(...)` side
    //! mirrors a NAMED type, never the TS `any` escape (§0.4.2/§0.4.5, the no-`any` rule G5/G8).
    //!
    //! The set is keyed by the `crate::ipc::events` constants, NEVER by re-spelled app:// string literals —
    //! check 28 forbids those outside `crate::ipc::events`, and the constants' literal VALUES are pinned there
    //! by `crate::ipc::app_event_names` (P2.39), which this leans on. The close-requested event carries `()`
    //! (§0.4.2 row), so it has — correctly — no payload type to register. Also leans on
    //! `ipc_typegen::event_payload_types_registered_for_typegen` (the full registration name-list, P2.39).
    //! [Build-Session-Entscheidung: P2.41]
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    // §6.4.1 unit (G15): the §0.4.2 app:// event surface is the closed set {fault, intake, close-requested},
    // each event's payload type authored + registered so none mirrors as the TS `any` escape. Pairs with
    // plan-lint check 28 (the "no fourth literal" source scan) — this is the in-core registration side.
    #[test]
    fn app_event_closed_set_binds_each_event_to_its_registered_payload() {
        use crate::ipc::events;

        // The §0.4.2 closed app:// event set bound to its payload type, KEYED BY the `crate::ipc::events`
        // constants (referencing them pins they are PRESENT — a removed/renamed constant fails to compile),
        // NOT by re-spelled app:// literals (plan-lint check 28 forbids those here; their literal values are
        // pinned by `app_event_names` in ipc/mod.rs). `None` = the event carries `()`, so it has no payload.
        let closed_set: BTreeMap<&str, Option<&str>> = [
            (events::APP_FAULT, Some("AppFault")), // §2.13 app-level fault
            (events::APP_INTAKE, Some("IntakePayload")), // §7.8.1 idle launch-arg / second-instance hand-off
            (events::APP_CLOSE_REQUESTED, None), // §7.3.2 mid-run close intercept — payload `()`
        ]
        .into_iter()
        .collect();

        // (A) Exactly three DISTINCT §0.4.2 events (the BTreeMap dedups by the constant value, so two
        // constants sharing a value would also drop the count). The literal-value pin is `app_event_names`;
        // the "no fourth literal in src-tauri/src" pin is check 28; this is the in-core cardinality leg.
        assert_eq!(
            closed_set.len(),
            3,
            "§0.4.2: the closed app:// event set is exactly three events {{fault, intake, close-requested}} \
             (P2.41/G23, paired with plan-lint check 28's source scan)"
        );

        // (B) The payload-BEARING events carry EXACTLY {AppFault, IntakePayload}; close-requested carries `()`
        // (None). A fourth payload-bearing event, a dropped payload, or close-requested sprouting a payload all
        // redden here — the in-core "each event with its authored payload type" half of the box invariant.
        let closed_payloads: BTreeSet<&str> = closed_set.values().copied().flatten().collect();
        let expected_payloads: BTreeSet<&str> = ["AppFault", "IntakePayload"].into_iter().collect();
        assert_eq!(
            closed_payloads, expected_payloads,
            "§0.4.2: exactly two of the three app:// events carry a payload type (AppFault for fault, \
             IntakePayload for intake); close-requested carries `()` (P2.41/G23)"
        );

        // (C) The side a source scan cannot do: each payload-bearing event's type is authored +
        // `.types()`-registered via the REAL `register_ipc_event_types`, so no app:// payload mirrors as the TS
        // `any` escape (§0.4.2/§0.4.5). `register_ipc_event_types` registers the named payloads AND pulls in
        // their field types, so its set is a SUPERSET — the load-bearing direction is closed-payloads ⊆ registered.
        let registered: BTreeSet<String> = register_ipc_event_types(specta::Types::default())
            .into_unsorted_iter()
            .map(|n| n.name.to_string())
            .collect();
        for ty in &expected_payloads {
            assert!(
                registered.contains(*ty),
                "§0.4.2: the app:// event payload `{ty}` must be authored + `.types()`-registered \
                 (register_ipc_event_types) so the listen side mirrors a named type, never `any` (P2.41)"
            );
        }
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
    // programmatic window builder. Scan `all_production_source()` (the shared `boot_invariants` helper:
    // the pre-test-module prefix + `main()`'s body, every test module excluded so these needles can never
    // self-match) for the Tauri v2 programmatic window-creation constructors. `main()`'s Builder/setup is
    // where such a call would live, and `production_boot_source()` alone does not reach it (P2.54 — fixes
    // the pre-existing P2.40 blindness). Needles `concat!`-assembled for self-match-avoidance.
    #[test]
    fn no_programmatic_window_builder() {
        let src = super::boot_invariants::all_production_source();
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

    // §7.6.1 / §6.4.1 unit (G15): the Builder registers no updater plugin. Scan `all_production_source()`
    // (the shared `boot_invariants` helper: the pre-test-module prefix + `main()`'s body, every test module
    // excluded so this needle can never self-match) for any updater plugin reference — `tauri_plugin_updater`
    // contains `plugin_updater`, so the one needle catches the crate path and any `.plugin(...)` registration.
    // The `.plugin(...)` chain lives in `main()`, which `production_boot_source()` alone does not reach (P2.54
    // — fixes the pre-existing P2.40 blindness). Needle via `concat!` (self-match-avoidance).
    #[test]
    fn builder_registers_no_updater_plugin() {
        let src = super::boot_invariants::all_production_source();
        let needle = concat!("plugin_", "updater");
        assert!(
            !src.contains(needle),
            "§7.6.1: the Builder must register no updater plugin (`{needle}`) — the updater is explicitly absent"
        );
    }
}

#[cfg(test)]
mod single_instance_lock_scope {
    //! §6.4.1 unit (G15): §7.1.1 — the single-instance LOCK SCOPE per platform. The scope is owned by
    //! `tauri-plugin-single-instance` (this box adds no locking logic), so this is a STRUCTURAL encoding of
    //! the per-platform fact, not a live test of the plugin: Windows (a per-Session `CreateMutexW`) + Linux
    //! (the session `D-Bus`) are PER-OS-USER; macOS is MACHINE-GLOBAL (the plugin's world-writable
    //! `/tmp/{id}_si.sock`) — the accepted v1 limitation, §0.11 threat class T13. The mapping is pinned as a
    //! truth table (checkable on every CI leg), a current-target consistency assertion (each native §6.4.4
    //! leg re-checks its own scope via `cfg!`), and a source scan that the production registration site
    //! documents the scope in-core — so the §7.1.1 / T13 note cannot silently drop. The §7.1.1 mechanism is
    //! the plugin's; this module makes the per-platform scope a checkable fact, not just prose.
    //! [Build-Session-Entscheidung: P2.51]

    /// The §7.1.1 single-instance lock scope a platform's `tauri-plugin-single-instance` mechanism yields.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum LockScope {
        /// Per-OS-user: a second launch by the SAME user reaches the primary instance; two different
        /// logged-in OS users each get their own. Windows (a per-Session `CreateMutexW`) + Linux (the
        /// session `D-Bus`). §7.1.1.
        PerOsUser,
        /// Machine-global: the lock is shared across ALL logged-in users on the machine. macOS only — the
        /// plugin hard-codes its socket at world-writable `/tmp/{id}_si.sock`. The accepted v1 limitation,
        /// §0.11 threat class T13.
        MachineGlobal,
    }

    /// The §7.1.1 per-platform lock-scope mapping for ConvertIA's three shipped desktop targets (§0.8).
    /// `None` = a target ConvertIA does not ship (unclassified) — a future target must be classified here
    /// explicitly, never silently defaulted to one scope or the other.
    fn lock_scope_for_os(target_os: &str) -> Option<LockScope> {
        match target_os {
            "macos" => Some(LockScope::MachineGlobal),
            "windows" | "linux" => Some(LockScope::PerOsUser),
            _ => None,
        }
    }

    // §6.4.1 unit (G15): the full §7.1.1 per-platform scope truth table over the three shipped targets. The
    // load-bearing row is macOS = MachineGlobal (the T13 limitation) versus Windows / Linux = PerOsUser.
    #[test]
    fn per_platform_lock_scope_truth_table() {
        assert_eq!(
            lock_scope_for_os("windows"),
            Some(LockScope::PerOsUser),
            "§7.1.1: Windows single-instance is a per-Session CreateMutexW — per-OS-user"
        );
        assert_eq!(
            lock_scope_for_os("linux"),
            Some(LockScope::PerOsUser),
            "§7.1.1: Linux single-instance is the session D-Bus — per-OS-user"
        );
        assert_eq!(
            lock_scope_for_os("macos"),
            Some(LockScope::MachineGlobal),
            "§7.1.1 / §0.11 T13: macOS single-instance is the machine-global /tmp socket (accepted limitation)"
        );
    }

    // §6.4.1 unit (G15): the BUILT target is always one of the three shipped, classified scopes, and the
    // table lookup for the current OS agrees with the `cfg!`-derived arm — so each native CI leg (§6.4.4)
    // re-checks its own platform's scope, and an unclassified target (table `None`) reddens here.
    #[test]
    fn current_target_scope_is_classified_and_matches_cfg() {
        let by_cfg = if cfg!(target_os = "macos") {
            LockScope::MachineGlobal
        } else {
            // The only other shipped desktop targets are Windows + Linux, both per-OS-user (§7.1.1).
            LockScope::PerOsUser
        };
        assert_eq!(
            lock_scope_for_os(std::env::consts::OS),
            Some(by_cfg),
            "§7.1.1: the built target `{}` must be a classified single-instance lock scope matching its `cfg!` arm",
            std::env::consts::OS
        );
    }

    // §6.4.1 unit (G15): the production registration site (main()'s Builder chain) DOCUMENTS the per-platform
    // lock scope in-core — the §7.1.1 / T13 note is this box's deliverable, so a source scan pins it cannot
    // silently drop. Scans the shared `crate::boot_invariants::production_main_body()` (main() only, every
    // test module excluded — the `production_main_body` helper was promoted to `boot_invariants` at P2.54 so
    // the `main()`-targeting scans share one source), so these needles — `concat!`-assembled for the same
    // self-match-avoidance as `boot_invariants` — are never matched against this test file: a deleted
    // production comment reddens here, it does not pass blindly.
    #[test]
    fn registration_site_documents_the_per_platform_scope() {
        let src = crate::boot_invariants::production_main_body();
        let needles = [
            concat!("T", "13"), // the §0.11 accepted-limitation threat class (macOS machine-global)
            concat!("machine", "-global"), // the macOS scope characterisation
            concat!("per-OS-", "user"), // the Windows / Linux scope characterisation
            concat!("CreateMutex", "W"), // the Windows per-Session mechanism
            concat!("D-", "Bus"), // the Linux session mechanism
        ];
        for needle in needles {
            assert!(
                src.contains(needle),
                "§7.1.1: the single-instance registration site must document the per-platform lock scope (missing `{needle}`)"
            );
        }
    }
}
