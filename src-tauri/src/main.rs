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
mod log_redact;
mod orchestrator;
mod outcome;
mod platform;
mod pool;
mod prefs;
mod run;

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_log::{log::LevelFilter, RotationStrategy, Target, TargetKind};

use crate::domain::{
    CollectedSetId, CollectingId, InstanceId, IntakePayload, ItemId, LossyKind, RunId,
};
use crate::outcome::{AppFault, ConversionErrorKind, IpcError, OutcomeMsg};

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

/// [Build-Session-Entscheidung: P2.80] The §0.4.2 `app://close-requested` payload. The event carries no data
/// (§0.4.2 "payload ()") — it is a pure signal to the §5.2 confirm UI. It is emitted as a `Serialize + Clone`
/// unit struct (the §7.3.2-sanctioned alternative to `serde_json::Value::Null`, chosen because `serde_json`
/// is a DEV-only dependency here while `serde` is in the production closure): serde serialises a unit struct
/// to JSON `null` — the SAME wire form as `Value::Null`, and NOT the bare `()` literal §7.3.2 rules out. It is
/// deliberately NOT `specta::Type` and NOT registered in `.types()`, so `bindings.ts` gains no type for it,
/// keeping the §0.4.2 / P2.41 closed-set contract that `app://close-requested` carries `()` (no payload type)
/// intact.
#[derive(Clone, serde::Serialize)]
struct CloseRequestedSignal;

/// [Build-Session-Entscheidung: P2.79/P2.80] §7.3.2 the window-lifecycle event handler — the Tauri v2 two-arg
/// `Builder::on_window_event(|window, event|)` hook (registered in `main()`) delegates every `WindowEvent`
/// here with the resolved `&AppHandle`, and this fn intercepts `CloseRequested`. When a conversion run is in
/// flight it (1) calls `api.prevent_close()` so a mid-run window close cannot end the app ungracefully and
/// truncate the in-flight output — the §7.3.3 "the core blocks the close" guarantee (the SSOT never-harm
/// origin) — and (2) emits `app://close-requested` (via the `crate::ipc::events` constant, never a re-spelled
/// literal — plan-lint check 28) so the §5.2 WebView confirm UI renders (§7.3.3). The core owns the busy
/// decision; the JS side only renders, avoiding the §7.3.2-warned split-brain "is it converting?" check. It
/// reuses the ONE §7.1.1 run-busy predicate (`launch_intake::converter_is_busy`, the §1.9 `RunRegistry` read)
/// — the spec mandates the launch refuse-busy gate and the close guard share the SAME predicate (§7.3.2). An
/// idle converter is not busy, so the close proceeds and the app quits immediately (§7.3.3). Other
/// `WindowEvent` variants are not intercepted (the single `main` window needs no per-event handling).
/// AppHandle-coupled boot-glue (the §1.1a boot-stage pattern — not `tauri::test`-mockable here; the routing is
/// source-scan-pinned + the §6.4.6 window-close E2E leg exercises it, and the `AppHandle` signature makes it
/// G28 diff-floor-exempt). `main()`'s closure is a thin delegation line (the established `.setup`/`.plugin`
/// main-body pattern), so the interceptor logic lives in this exempt fn, not in `main()`.
fn dispatch_window_event(app: &tauri::AppHandle, event: &tauri::WindowEvent) {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        if launch_intake::converter_is_busy(app) {
            api.prevent_close();
            app.emit(
                crate::ipc::events::APP_CLOSE_REQUESTED,
                CloseRequestedSignal,
            )
            .ok();
        }
    }
}

/// [Build-Session-Entscheidung: P2.81] §7.3.2 the `App::run` run-event handler — registered on the BUILT
/// `App` (`.build(ctx)?.run(|app, event| dispatch_run_event(app, &event))`, NOT the `Builder`) and owning the
/// two §7.3.2 run-lifecycle events. `RunEvent::ExitRequested` is the last chance to `api.prevent_exit()` — the
/// §7.3.3 quit-while-converting guard's QUIT leg (busy-gated, never unconditional; user/OS quits only — a
/// programmatic `app.exit(code)` passes so a confirmed-quit flow can exit; the window-close path is guarded
/// by `dispatch_window_event`'s §7.3.2 `CloseRequested` `prevent_close`, and both legs share the ONE §7.3.2
/// busy predicate + the ONE `app://close-requested` confirm signal — the P2.137 sweep closed the quit leg
/// the macOS app-menu Quit / Cmd+Q path needs, which never raises a per-window `CloseRequested`).
/// `RunEvent::Exit` is the final cleanup point: flush the plugin logger's buffered records before exit (via
/// tauri-plugin-log's re-exported `log`). The best-effort scratch cleanup call joins at P3.74 (= the §2.6
/// `cleanup_run` path, §7.3.2) — `crate::run::cleanup_run` is the P3 §2.6 kernel and nothing run-owned is
/// created to clean at this box. `RunEvent` is an external `#[non_exhaustive]` enum whose known-variant set is
/// PLATFORM-DEPENDENT (`Opened`/`Reopen`/`SceneRequested` are `#[cfg]`-gated to Apple/mobile targets), so an
/// exhaustive listing would be platform-fragile (clippy would demand a different variant set per OS); the
/// item-level `#[allow(clippy::wildcard_enum_match_arm)]` is the gate-sanctioned per-item escape (it does NOT
/// disqualify the crate-root deny — check-rust-lint-contract's item-allow allowance), so every other run
/// event is a no-op. On the Apple/mobile targets it ALSO matches the `#[cfg]`-gated `RunEvent::Opened` — the
/// macOS Open-with hook (§7.8.1): its `file://` urls route through the SAME §7.8.1 funnel (`handle_opened`,
/// P2.56) so the §7.1.1 refuse-busy gate + the §1.1 freeze apply, never bypassing the primary gate. `app` is
/// used unconditionally since P2.137 (the `ExitRequested` busy read), so the former Win/Linux
/// `allow(unused_variables)` shim is gone. AppHandle-coupled boot-glue (the §1.1a boot-stage pattern — not
/// `tauri::test`-mockable here; source-scan-pinned + the §6.4.6 E2E leg; the `AppHandle` signature makes it
/// G28 diff-floor-exempt).
#[allow(clippy::wildcard_enum_match_arm)]
fn dispatch_run_event(app: &tauri::AppHandle, event: &tauri::RunEvent) {
    match event {
        // §7.3.2/§7.3.3 the QUIT-path busy guard — the "last chance to `api.prevent_exit()`": a mid-run quit
        // that never traverses the window-close path (macOS app-menu Quit / Cmd+Q raises `ExitRequested`
        // directly, with no per-window `CloseRequested`) is blocked and routed to the SAME §5.2 confirm
        // signal as the window-close guard — the one §7.3.2 busy predicate, the one `app://close-requested`
        // event. A PROGRAMMATIC exit (`app.exit(code)` → `code: Some(..)`) is never blocked: that is the
        // sanctioned exit a confirmed-quit flow uses (its §5.2 QuitConfirm edge is the P4.67 box), so the
        // guard applies only to the OS/user quit request (`code: None`). An idle converter is not busy, so
        // the quit proceeds immediately (§7.3.3). [Build-Session-Entscheidung: P2.137]
        tauri::RunEvent::ExitRequested { api, code, .. } if code.is_none() => {
            if launch_intake::converter_is_busy(app) {
                api.prevent_exit();
                app.emit(
                    crate::ipc::events::APP_CLOSE_REQUESTED,
                    CloseRequestedSignal,
                )
                .ok();
            }
        }
        // §7.3.2 the final cleanup point — flush the plugin logger before the process exits.
        tauri::RunEvent::Exit => {
            tauri_plugin_log::log::logger().flush();
        }
        // §7.8.1 macOS Open-with: `RunEvent::Opened` is an Apple/mobile-target `#[cfg]`-gated variant (absent
        // on Win/Linux, so the arm carries the SAME cfg — an unconditional arm would not compile there). Route
        // the `file://` urls through the SAME §7.8.1 funnel (`handle_opened`, P2.56) so the §7.1.1 refuse-busy
        // gate + the §1.1 freeze apply — a mid-conversion Open-with is refused, never bypassing the primary gate.
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
        tauri::RunEvent::Opened { urls } => launch_intake::handle_opened(app, urls),
        _ => {}
    }
}

/// [Build-Session-Entscheidung: P2.90] §7.5.2 the persistence target: the OS-specific log DIRECTORY,
/// resolved per-OS by Tauri's `app_log_dir()`. `tauri-plugin-log` resolves `TargetKind::LogDir` at runtime
/// via `app_handle.path().app_log_dir()` (tauri-plugin-log 2.8.0 `lib.rs:628`), so this hard-codes NO path —
/// the location tracks Tauri, not a fragile literal. Per-OS: Windows `%LOCALAPPDATA%\dev.ne-ia.convertia\logs`,
/// macOS `~/Library/Logs/dev.ne-ia.convertia`, Linux `~/.config/dev.ne-ia.convertia/logs`. The Linux
/// **config-dir** resolution (Tauri's `app_log_dir()` resolves via `$XDG_CONFIG_HOME`, deviating from strict
/// XDG `$XDG_STATE_HOME`) and its rationale are the authoritative §7.5.2 `[DECIDED]` note — recorded there
/// once, not duplicated here (one home per fact). `file_name: None` keeps the plugin's default log file name.
/// `log_dir_target_resolves_via_app_log_dir` pins the `LogDir`-default-name choice (a regression to a
/// hard-coded `Folder { path }` or a custom name — either of which would break the per-OS `app_log_dir()`
/// resolution — fails it).
fn log_dir_target() -> TargetKind {
    TargetKind::LogDir { file_name: None }
}

/// [Build-Session-Entscheidung: P2.89] §7.5.2 log targets — the persistence + dev-console target set for
/// `tauri-plugin-log`, made explicit as a pure, coverage-counted function so the §3 zero-egress control is
/// TESTED, not implicit. The ONLY persistence target is the OS log directory (`log_dir_target()`, the §7.5.2
/// primary rotating on-disk record, resolved per-OS via `app_log_dir()` — see that fn); `Stderr` is added in
/// dev builds only (§7.5.2 "stderr in dev"). It deliberately omits every non-local / arbitrary sink
/// `TargetKind` offers — `Webview` (§7.5.2: the webview console is NOT a persistence target), `Dispatch` (an
/// arbitrary `fern` sink that could route off the machine), `Folder`, `Stdout` — so NO network sink can ever
/// reach the log (§7.5.1/§2.11 no-telemetry, §0.10 allowlist). `log_targets_are_local_only` pins the
/// whitelist. Rotation (`max_file_size`/`KeepOne`, §7.5.2) is applied in `log_plugin()`; this function
/// returns the target KINDS only (the level + the rotation cap live in `log_plugin()`).
fn log_targets() -> Vec<TargetKind> {
    // Release builds write the on-disk file exclusively (no console); dev builds add `Stderr`. Two cfg-gated
    // bindings (not a `mut` + conditional push) so neither profile trips clippy `unused_mut`.
    #[cfg(debug_assertions)]
    let targets = vec![log_dir_target(), TargetKind::Stderr];
    #[cfg(not(debug_assertions))]
    let targets = vec![log_dir_target()];
    targets
}

/// [Build-Session-Entscheidung: P2.91] §7.5.2 `[DECIDED]` the rotating-file size cap: the single log file is
/// bounded at 5 MB (bytes). Paired with `RotationStrategy::KeepOne`, whose rotation arm is `fs::remove_file`
/// (it DELETES the old file, not renames it to a dated backup like `KeepAll`/`KeepSome`), so on reaching the
/// cap the on-disk maximum stays ~1x this value — the "leave nothing behind / no system pollution" budget.
/// The `KeepOne == fs::remove_file` ≈1x-footprint audit against the pinned plugin version lives in
/// §7.5.2's `Audit trail` (the concrete audit vs the `tauri-plugin-log` 2.8.0 pin, P2.92).
/// `log_max_file_size_is_the_spec_cap` pins the value to §7.5.2's `5_000_000`.
const LOG_MAX_FILE_SIZE_BYTES: u128 = 5_000_000;

/// [Build-Session-Entscheidung: P2.89] §7.5.1/§7.5.2 the configured `tauri-plugin-log` plugin: the local
/// on-disk rotating file (+ dev `Stderr`) from `log_targets()`, default level `info`, and NO default `Stdout`
/// target. Thin glue over the `log_targets()` zero-egress control: `.targets(...)` fully REPLACES the
/// plugin's default target set (`[Stdout, LogDir]`) with exactly `log_targets()`, and `.clear_targets()` is
/// kept ahead of it as the plugin's documented "ignore the defaults" idiom (a belt-and-suspenders marker so no
/// default `Stdout` sink can leak in even if a future change makes `.targets()` append-style); then the level
/// and the §7.5.2 rotation cap (5 MB / `KeepOne`, see `LOG_MAX_FILE_SIZE_BYTES`) are pinned. `info` is the
/// §7.5.3 `info`/`warn` default that captures the structural diagnostic facts §7.5.4/§6.5 depend on.
/// [Build-Session-Entscheidung: P2.94] the GLOBAL level stays `info`, and a `convertia_core`-scoped `Debug`
/// `level_for` ceiling is added as the §7.5.3 verbose gateway: this crate's `debug!` records pass the plugin
/// filter (dependency `debug!` never does — no wry/tao noise / third-party path leak), but only APPEAR once
/// `resolve_log_verbosity` raises the runtime `log::max_level` to `Debug` at startup iff verbose is on. NOT
/// `AppHandle`-coupled (no `&AppHandle` in the signature) → NOT the P2.135 G28
/// boot-glue exemption → its lines COUNT in the diff floor, so `log_plugin_builds` executes it. `.build()`
/// only constructs the plugin descriptor — the global logger is installed by the plugin's `setup` hook at app
/// init, not by this call — so calling this outside `main()` (the test) installs no logger and is
/// side-effect-free.
fn log_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_log::Builder::new()
        .clear_targets()
        .targets(log_targets().into_iter().map(Target::new))
        .level(LevelFilter::Info)
        // [Build-Session-Entscheidung: P2.94] §7.5.3 verbose gateway — grant THIS crate a `Debug` ceiling
        // (`module_path!()` at the crate root == "convertia_core", a fern PREFIX match over
        // `convertia_core::*`; using `module_path!()` not a literal so a crate rename cannot silently
        // un-scope it). The global level stays `info` above, so dependency `debug!` (wry/tao/…) never reaches
        // the file — no noise, no third-party path leak. This ceiling only lets this crate's `debug!` records
        // PASS the plugin's own filter; they still only APPEAR when the runtime `log::max_level` is raised to
        // `Debug`, which `resolve_log_verbosity` does once at startup iff verbose. Necessary because
        // `set_max_level(Debug)` ALONE emits nothing — the plugin's fern filter drops sub-global records
        // regardless of the macro-level gate.
        .level_for(module_path!(), LevelFilter::Debug)
        // [Build-Session-Entscheidung: P2.91] §7.5.2 rotation: cap the single file at 5 MB and KEEP ONE — the
        // `KeepOne` arm deletes (fs::remove_file) rather than renaming to a dated backup, so the on-disk
        // footprint stays ~1x the cap (the ≈1x source-audit vs the pinned plugin version lives in §7.5.2, P2.92).
        .max_file_size(LOG_MAX_FILE_SIZE_BYTES)
        .rotation_strategy(RotationStrategy::KeepOne)
        .build()
}

/// [Build-Session-Entscheidung: P2.94] §7.5.3 the `--verbose` launch-switch predicate — `true` iff argv
/// carries the `--verbose` diagnostic flag (the launch-flag half of the §7.5.3 verbose opt-in; the other
/// half is the persisted `verboseLog` pref, §7.4). Pure over the passed argv (the `args_os` read lives in
/// the boot-glue `resolve_log_verbosity`), so the switch detection is unit-tested in isolation. Matches the
/// EXACT `--verbose` token — `parse_path_args` already classifies it as a launch switch, never an ingestable
/// path (P2.54.1) — and a bare `-v` / other spelling is deliberately NOT accepted (one canonical flag).
fn argv_has_verbose(argv: &[String]) -> bool {
    argv.iter().any(|token| token == "--verbose")
}

/// [Build-Session-Entscheidung: P2.94] §7.5.3 resolve the verbose log level ONCE at startup — the §7.5.3
/// diagnostic opt-in. Verbose is on iff the persisted `verboseLog` pref (§7.4) is `true` OR the `--verbose`
/// launch flag is present; when on, the runtime `log::set_max_level` is raised to `Debug` (else `Info`),
/// which is what makes the `convertia_core`-scoped `Debug` ceiling in `log_plugin` actually emit this
/// crate's `debug!` §7.5.4 verbose-diagnostic records. Called from `.setup()` — the first point the
/// AppHandle is live, so `prefs::load` can resolve the config dir — because the log plugin is registered on
/// the Builder BEFORE any AppHandle exists, so the persisted pref cannot be read at the plugin's own
/// plugin-init `set_max_level`; this setup-stage read is the §7.5.3 "resolve the verbose level once at
/// startup". A mid-session About-toggle only persists the new value — `setup` runs once — so it takes effect
/// on the NEXT launch (§7.5.3 / §5.9 "applies after restart"). Best-effort: `prefs::load` never fails
/// (§7.4.2), so an unreadable store simply yields the `false` default. AppHandle-coupled boot-glue (the
/// §1.1a boot-stage pattern — not `tauri::test`-mockable; the decision is the pure unit-tested
/// `argv_has_verbose` + the §7.4.2-tested `prefs::load`, so only this thin plumbing is un-executed; the
/// `AppHandle` signature makes it G28 diff-floor-exempt, and it is signature- + wiring-source-scan-pinned).
fn resolve_log_verbosity(app: &tauri::AppHandle) {
    // [Build-Session-Entscheidung: P2.94] G29/SAST per-finding suppression — the vendored rule
    // `rust.lang.security.args-os.args-os` ("don't rely on `args_os` for SECURITY") does NOT apply here:
    // argv is read ONLY to detect the presence of the `--verbose` diagnostic switch (a bool), never for a
    // security decision; a non-UTF8 arg is compared lossily and simply won't equal `--verbose`. The bare
    // marker carries only the rule-id (semgrep parses the rest of a `nosemgrep:` line as comma-separated
    // rule-ids), so the rationale stays on these separate lines.
    // nosemgrep: rust.lang.security.args-os.args-os
    let argv: Vec<String> = std::env::args_os()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let verbose = verbose_opt_in(crate::prefs::load(app).verbose_log, argv_has_verbose(&argv));
    tauri_plugin_log::log::set_max_level(if verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    });
}

/// §7.5.3 the verbose opt-in DECISION — a pure rule extracted from the boot glue (the §1.1a
/// decision-pure/glue-thin split): verbose logging is on when the persisted §5.9 About toggle OR the
/// `--verbose` launch switch asks for it (either source alone suffices; the OR is the §7.5.3 contract, so it
/// is truth-table-pinned rather than living un-executed inside the G28-exempt `resolve_log_verbosity`
/// plumbing). [Build-Session-Entscheidung: P2.137]
fn verbose_opt_in(pref_verbose: bool, argv_verbose: bool) -> bool {
    pref_verbose || argv_verbose
}

// ── §7.2.1 ordered startup spine (steps 3–6 + the §2.13.3 fault presentation, P2.106) ──────────────────────
// These are the AppHandle-coupled boot-glue fns the `main()` `setup` closure sequences into the §7.2.1 order.
// The three readiness SLOTs (steps 3–5) return Ok now — their bodies are the P4 engine + §2.6 layer — so the
// window is revealed (step 6) on the ready path; when the bodies land, a failing step yields an app-level
// `AppFault` the setup match routes to `present_startup_fault` (§2.13.3), leaving the window hidden. Every fn
// here carries an `AppHandle` in its signature (the P2.135 G28 boot-glue exemption is signature-based) and is
// covered by the boot-stage pattern (source-scan + signature pins, not `tauri::test` execution — §1.1a),
// because this crate ships no `tauri::test` mock harness.

/// [Build-Session-Entscheidung: P2.106.3] §7.2.1 step 3 — the §7.2.3 engine presence + integrity verification
/// SLOT. The verifier BODY — iterate the §3.3.1 externalBin binary list (`ffmpeg`/`ffprobe`/`soffice`/
/// `pdftotext`/`pandoc`/`convertia-imgworker`, the bare runtime names + `.exe` on Windows), resolve each under
/// the resource dir, and hash-on-first-launch against the bundled manifest with the cheap size/magic warm
/// check (§7.2.3) — lands with the P4 engine layer; a missing/corrupt REQUIRED engine becomes an
/// `EngineMissing`/`BundleDamaged` `AppFault` (§2.13) routed to `present_startup_fault`. Returning `Ok(())`
/// wires the readiness gate structurally without asserting engines P2 has not staged. AppHandle-coupled
/// boot-glue (§1.1a; signature-pinned + G28-exempt).
fn verify_engine_presence(_app: &AppHandle) -> Result<(), AppFault> {
    Ok(())
}

/// [Build-Session-Entscheidung: P2.106.4] §7.2.1 step 4 — the §7.2.4 executable-permission setup SLOT for the
/// portable build. The BODY — on macOS/Linux ensure each bundled engine binary carries the execute bit
/// (idempotent `+x`, §7.2.4; a no-op on Windows, where sidecar `.exe`s run as-is) — lands with the P4 engine
/// layer; a permission failure is an app-level `AppFault`. Returning `Ok(())` wires the ordered gate; the
/// engines it would `chmod` are staged in P4. AppHandle-coupled boot-glue (§1.1a; signature-pinned + G28-exempt).
fn ensure_engine_permissions(_app: &AppHandle) -> Result<(), AppFault> {
    Ok(())
}

/// [Build-Session-Entscheidung: P2.106.5] §7.2.1 step 5 — the §7.2.5 scratch + log dir creation SLOT with the
/// §7.1.2 per-instance root, plus the §2.6 orphan-reclaim SLOT (remove a previous crashed run's residue, keyed
/// by the §2.6.3 held-lock liveness predicate so a concurrent instance's live temp is never touched). The BODY
/// — create the per-instance scratch root + log dir on first need and reclaim orphaned roots — is owned by the
/// §2.6 kernel (P3) + the P4 engine layer; NO directory is created here (§7.2.1 step 2 only RESOLVED the base
/// paths). Returning `Ok(())` wires the ordered gate. AppHandle-coupled boot-glue (§1.1a; signature-pinned +
/// G28-exempt).
fn prepare_scratch_and_log(_app: &AppHandle) -> Result<(), AppFault> {
    Ok(())
}

/// [Build-Session-Entscheidung: P2.106] §7.2.1 the readiness gate — steps 3 → 4 → 5 in order, short-circuiting
/// on the first `AppFault` via `?`: engine presence + integrity (3), executable-permission setup (4), scratch +
/// log creation + orphan reclaim (5). The `main` window is revealed (step 6) ONLY when this returns `Ok` — the
/// §7.2.1 "the window is only shown once they succeed" contract — so a hard startup fault is presented as a
/// clean §2.13 screen, never a half-broken UI. Each step is a SLOT returning `Ok` now (bodies P3/P4), so the
/// gate passes; when the bodies land, a failing step yields the app-level fault the `setup` match presents.
/// AppHandle-coupled boot-glue (§1.1a; signature-pinned + G28-exempt).
fn readiness_checks(app: &AppHandle) -> Result<(), AppFault> {
    verify_engine_presence(app)?;
    ensure_engine_permissions(app)?;
    prepare_scratch_and_log(app)?;
    Ok(())
}

/// [Build-Session-Entscheidung: P2.106.6/P2.109] §7.2.1 step 6 — reveal the config-declared single `main`
/// window (P1.16/P1.19, created HIDDEN via `visible: false` in `tauri.conf.json`) now that the readiness
/// steps 3–5 have passed. Showing it only here is the §7.2.1 "the window is only shown once they succeed"
/// guarantee — a hard startup fault renders as a clean §2.13 fault screen, never a half-broken window.
/// Resolving the window here is ALSO the §0.3.1/§7.2.1 WebView-init fault observation point:
/// `get_webview_window("main")` returning `None` means the OS WebView runtime could NOT create the view (a
/// missing/old macOS WKWebView / Linux WebKitGTK; the Windows WebView2-absent case fails before the core runs
/// and is the §0.3.1 honest exception, not this). [Build-Session-Entscheidung: P2.109] the `None` arm surfaces
/// that as the §2.13 app-level `WebviewFault` (`webview_init_fault`) routed to `present_startup_fault`
/// (§2.13.3) — a broken WebView cannot render an `app://fault` screen, so its presentation is NATIVE (body
/// P4); this box builds the detection + routing seam. This is NOT a programmatic window builder — the window
/// is config-declared (§7.3.1); this only shows the already-created one. [Build-Session-Entscheidung: P2.109]
/// This fn returns `()` (internal routing, NOT a short-circuit signal — so `main()`'s setup is unchanged +
/// G28-clean, not the `and_then` variant that would add uncovered lines to `main`), so the setup's step-7
/// launch-intake feed (`forward_first_launch_argv`) still runs after a `None`/`WebviewFault`. That feed is
/// INERT on this path — `frontend_ready` is `false` (no window ⇒ `mark_ready` is never called), so
/// `intake_disposition` resolves to `Buffer` → `PendingIntake`, which is then never drained: no panic, no
/// path loss. Whether a fatal boot fault should SKIP step 7 is a P4 decision that travels with the native
/// presentation body (which owns what the app does after a `WebviewFault`). AppHandle-coupled boot-glue (§1.1a;
/// signature-pinned + G28-exempt).
fn reveal_main_window(app: &AppHandle) {
    match app.get_webview_window("main") {
        // §7.2.1 step 6 ready path: the WebView was created — show the config-declared `main` window.
        Some(window) => {
            window.show().ok();
        }
        // §0.3.1/§2.13 WebView-init fault: no `main` WebView exists (missing/old WKWebView / WebKitGTK), so
        // route the app-level `WebviewFault` to the §2.13.3 presentation. An `app://fault`→WebView emit is
        // impossible here (there is no WebView), so the NATIVE presentation body (P4) owns HOW; P2.109 owns
        // the detection + route.
        None => present_startup_fault(app, webview_init_fault()),
    }
}

/// [Build-Session-Entscheidung: P2.109] §7.2.1 step 6 / §0.3.1 — construct the §2.13 app-level `WebviewFault`
/// for a WebView-init failure (`get_webview_window("main") == None`: the OS WebView runtime — macOS WKWebView
/// / Linux WebKitGTK — could not create the view). It is an `AppFault` (§2.13.1 "the app can't function"),
/// NOT a per-item `IpcError`. PURE (no `AppHandle`) so it is unit-tested in isolation and its lines COUNT in
/// the G28 diff floor — it is NOT the §1.1a boot-glue exemption (only the AppHandle-coupled `reveal_main_window`
/// caller is). `kind` is the CONCRETE `ConversionErrorKind::WebviewFault`, never the §0.4.3 `ErrorKind` alias
/// (the P2.39.1 dead-code-expectation/alias reason). `message` is the §2.13.3 pre-localised, plain-English,
/// trace-free calm line pointing at the releases page (the §2.13.3 "download it again … official releases
/// page" pattern; §0.3.1 pins the supported-OS/WebView floor); §7.2 owns the app-level startup strings and the
/// NATIVE presentation of this line is the P4 body (a broken WebView cannot render an `app://fault` screen).
fn webview_init_fault() -> AppFault {
    AppFault {
        kind: ConversionErrorKind::WebviewFault,
        message: "ConvertIA couldn't start its window because your system's web view component is missing or out of date. See the official releases page for the supported systems."
            .to_owned(),
    }
}

/// [Build-Session-Entscheidung: P2.106.3/P2.109] §2.13.3 the app-level startup-fault presentation — the
/// mechanism-INDEPENDENT entry point every startup fault routes through. It records the fault to the local log
/// (§7.5; the §2.13 app-level fault is trace-free — the §2.13.3 `AppFault.message` is a pre-localised,
/// trace-free calm line and `kind` is an enum, so this log line is redaction-safe) — a real action, never a
/// silent drop.
///
/// [Build-Session-Entscheidung: P2.109] **How the fault is PRESENTED splits by the WebView's own health
/// (§7.2.1 / §2.13.3 design-of-record):** a readiness fault (steps 3–5) — `EngineMissing` / `BundleDamaged` —
/// leaves the WebView healthy, so it is emitted over the §0.4.2 `app://fault` event to the §5.8 WebView screen,
/// with a `PendingFault` buffer closing the first-frame race (emitting before the §5.8 listener is registered
/// would lose it); a `WebviewFault` (step 6, the WebView itself failed to init) makes an `app://fault`→WebView
/// emit impossible, so it presents on a NATIVE surface. BOTH presentation bodies are P4 — this shell records to
/// the log only; `_app` is the handle they emit/buffer/native-present through. Reached from TWO routes: the
/// readiness-gate `Err` arm (steps 3–5, runtime-dead until P4 fills those SLOT bodies — they return `Ok`) AND
/// `reveal_main_window`'s `None` arm (the `WebviewFault` path, P2.109 — the sole `AppFault` CONSTRUCTED in
/// production, fired only when the OS WebView runtime genuinely fails to init, which is not reproducible under
/// `cargo test`). AppHandle-coupled boot-glue (§1.1a; signature-pinned + G28-exempt).
fn present_startup_fault(_app: &AppHandle, fault: AppFault) {
    tauri_plugin_log::log::error!(
        "§2.13 app-level startup fault [{:?}]: {}",
        fault.kind,
        fault.message
    );
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
    // `converter_is_busy` predicate now reads the real §1.9 run-state via the RunRegistry (P2.55), the
    // `PendingIntake` buffer arm is real (P2.58), and the `frontend_ready` ready-flag now reads the real
    // `State<FrontendReady>` (P2.59). Until P3 populates runs the registry is empty (not busy), and until P2.60
    // wires the C1 `drainPending` drain (which calls `mark_ready`) the flag stays `false`, so an idle launch set
    // routes — via `frontend_ready`'s not-ready flag — to the real buffer, never lost.

    use std::path::PathBuf;

    use tauri::{AppHandle, Emitter, Manager};

    use crate::domain::{IntakeOrigin, IntakePayload};
    use crate::ipc::events;
    use crate::orchestrator::{FrontendReady, PendingIntake, RunRegistry};

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
        /// a drop (§5.2/§1.1). Selected once the real `frontend_ready` flag (`State<FrontendReady>`, P2.59) is set.
        Emit,
        /// Idle but the WebView is not-yet-ready (the §7.8.1 first-launch listener race) — stash the paths in
        /// `PendingIntake` for the drain-on-mount replay (P2.58/P2.60). The default until `frontend_ready` is set (P2.59).
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
            // Idle + the WebView listener is ready: emit the §0.4.2 `app://intake` event so the UI mirrors
            // a drop (§5.2/§1.1).
            IntakeDisposition::Emit => emit_intake(app, paths, origin),
            // Idle but the WebView is not-yet-ready (the §7.8.1 first-launch listener race): stash the
            // paths + origin for the drain-on-mount replay — or, when the C1 drain won the race between
            // the disposition snapshot above and the stash (the §7.8.1 no-loss closure, P2.137:
            // `stash_or_route` re-checks readiness under the pending-slot lock), take the set back and
            // emit it live instead — nothing is ever stranded.
            IntakeDisposition::Buffer => {
                if let crate::orchestrator::StashOutcome::RouteToEmit(set) =
                    buffer_pending_intake(app, paths, origin)
                {
                    emit_intake(app, set.paths, set.origin);
                }
            }
        }
    }

    /// The single §0.4.2 `app://intake` emit site — the payload is `{ paths, origin }` so the frontend
    /// re-calls C1 with the right `IntakeOrigin` (§5.2/§1.1); the event name via the `crate::ipc::events`
    /// constant, never a re-spelled literal (plan-lint check 28). Extracted so the emit surface has exactly
    /// ONE production `emit(events::APP_INTAKE` call site (both funnel arms route through it; the
    /// `launch_intake::tests` cardinality scan pins that closed surface — a second emit site would bypass
    /// the §7.1.1 refuse-busy gate). AppHandle-coupled boot-glue (§1.1a; G28 signature-exempt).
    /// [Build-Session-Entscheidung: P2.137]
    fn emit_intake(app: &AppHandle, paths: Vec<PathBuf>, origin: IntakeOrigin) {
        app.emit(events::APP_INTAKE, IntakePayload { paths, origin })
            .ok();
    }

    /// [Build-Session-Entscheidung: P2.54.1] The §7.8.1 `argv` classifier — split launch FLAG tokens from
    /// file-PATH tokens. `argv[0]` (the program path) is skipped; the §7.5.3 `--verbose` diagnostic switch
    /// and any `-`/`--`-prefixed token are launch switches (never ingestable paths); a relative path
    /// resolves against the launching `cwd`. The §1.1 freeze re-validates (canonicalises / resolve-identity
    /// / detects) every returned path, so this is CLASSIFICATION, not a trust boundary — but the flag-vs-path
    /// split + the cwd-relative resolution are genuinely homed here. `PathBuf` is platform-aware, so the
    /// Win-vs-Linux separator / argv conventions are handled by construction.
    fn parse_path_args(argv: &[String], cwd: &str) -> Vec<PathBuf> {
        // An EMPTY token (a buggy wrapper script / desktop-entry quoting artifact, e.g. `convertia ""`) is
        // dropped: cwd-joining it would resolve to the launching DIRECTORY itself, silently turning the
        // user's whole cwd into a recursively-walked ingest root (§7.8.1 classifies, it never invents a
        // path the user did not name). [Build-Session-Entscheidung: P2.137]
        argv.iter()
            .skip(1)
            .filter(|tok| !tok.is_empty() && !is_launch_switch(tok))
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
    ///
    /// [Build-Session-Entscheidung: P2.137] A **Windows drive-relative** token (`C:file.txt` — a prefix but
    /// no root, so `is_absolute() == false`) passes through effectively UNCHANGED: `Path::join` with a
    /// prefix-bearing right side REPLACES the base (documented `std::path::PathBuf::push` semantics), so the
    /// token resolves against the primary instance's per-drive cwd at freeze time, where a missing file
    /// fails clearly (§2.8) — classification never rewrites a drive-qualified token the user passed.
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

    /// [Build-Session-Entscheidung: P2.56] The §7.8.1 launch-origin decision for the macOS Open-with hook — a
    /// pure rule extracted beside the AppHandle-coupled handler (the §1.1a boot-stage pattern): a first-launch
    /// Open-with (the WebView listener is not ready → the set is buffered + drained, §7.8.1) is `LaunchArg`; a
    /// while-running Open-with is `SecondInstance`. `handle_opened` resolves `frontend_ready(app)` and passes it
    /// in, so the rule is pure + unit-tested in isolation. NO `dead_code` attr is needed: `handle_opened`'s body
    /// has a call site to it, which rustc counts as a use even though `handle_opened` is itself dead on
    /// Win/Linux (its `App::run` Opened arm is `#[cfg]`-gated to the Apple targets, P2.82) — so only
    /// `handle_opened` is flagged there, not this.
    fn launch_origin(frontend_ready: bool) -> IntakeOrigin {
        if frontend_ready {
            IntakeOrigin::SecondInstance
        } else {
            IntakeOrigin::LaunchArg
        }
    }

    /// [Build-Session-Entscheidung: P2.56] §7.8.1 the macOS Open-with HANDLER — converts the `RunEvent::Opened`
    /// `file://` URLs via `Url::to_file_path()` and routes them through the SAME §7.8.1 `forward_launch_intake`
    /// funnel as the argv / single-instance paths, with the origin resolved by readiness (`launch_origin`). So
    /// the §7.1.1 refuse-busy gate + the §1.1 freeze apply identically (a mid-conversion Open-with is refused,
    /// never merged into the §2.4 frozen set — without this a macOS Open-with would BYPASS the PRIMARY gate,
    /// since it never goes through the argv callback there). Takes the already-extracted `urls` (cfg-free —
    /// `tauri::Url` exists on every target), so the Apple/Android-target gating of the `RunEvent::Opened`
    /// variant lives on the P2.82 `App::run` ARM that calls this, NOT here.
    /// AppHandle-coupled boot-glue (§1.1a; G28 signature-exempt; the routing is source-scan-pinned, the runtime
    /// is the §6.4.6 macOS E2E smoke leg). Wired by the P2.82 `App::run` Opened arm (itself `#[cfg]`-gated to
    /// the Apple/mobile targets), so live there + in the test build (the signature pin); the `dead_code`
    /// expectation is therefore scoped to `not(test)` AND the non-Apple targets, where the Opened arm is
    /// compiled out.
    ///
    /// [Build-Session-Entscheidung: P2.56.1] **`RunEvent::Opened` is an Apple/Android-target variant (Tauri-v2
    /// API fact).** In Tauri v2 it is a `#[cfg(any(target_os = "macos", target_os = "ios", target_os =
    /// "android"))]` enum VARIANT — it does NOT exist on Windows/Linux, so launch intake there rests on the
    /// argv / single-instance path (§7.8.1). Of ConvertIA's shipped DESKTOP triples (macOS/Linux/Windows;
    /// CLAUDE.md §1, no mobile build) it is therefore reachable ONLY on macOS. The P2.82 `App::run` ARM that
    /// matches it is cfg-gated accordingly, while the `App::run` handler *registration* stays unconditional
    /// (one funnel): where the variant is absent the Opened arm is compiled out — a no-op for Open-with, not a
    /// second intake path. (This handler is cfg-free because it takes the extracted `urls`, never the variant.)
    ///
    /// [Build-Session-Entscheidung: P2.56.2] **NOT `tauri-plugin-deep-link` / `on_open_url`.** That plugin
    /// handles custom-scheme deep links (`myapp://…`), a DIFFERENT OS intent; it does NOT fire for the
    /// Open-with / open-documents AppleEvent that delivers `file://` URLs, so using it for file intake would
    /// silently never trigger. ConvertIA registers NO URL scheme (§7.8.2 negative), so `on_open_url` is
    /// irrelevant — the open-documents AppleEvent surfaced as `RunEvent::Opened` is the sole macOS file-open
    /// mechanism.
    #[cfg_attr(
        all(
            not(test),
            not(any(target_os = "macos", target_os = "ios", target_os = "android"))
        ),
        expect(
            dead_code,
            reason = "the macOS Open-with handler — dead ONLY on Win/Linux, where the P2.82 App::run Opened arm that wires it is compiled out (the arm + the RunEvent::Opened variant are #[cfg]-gated to the Apple/mobile targets, where handle_opened is live); the test build uses it via the signature pin"
        )
    )]
    pub(super) fn handle_opened(app: &AppHandle, urls: &[tauri::Url]) {
        forward_launch_intake(
            app,
            opened_urls_to_paths(urls),
            launch_origin(frontend_ready(app)),
        );
    }

    /// §7.8.1 the Open-with URL→path conversion — the pure rule extracted beside `handle_opened` (the §1.1a
    /// pure-rule/glue split, the `launch_origin` sibling): each `file://` URL converts via
    /// `Url::to_file_path()` — the spec's own prescribed `filter_map` form (§7.8.1) — and an unconvertible
    /// URL (non-`file` scheme, a host-bearing form with no local mapping) is dropped: ConvertIA registers NO
    /// URL scheme (§7.8.2 negative), so a non-file URL can only be a stray AppleEvent, never a supported
    /// intake. Percent-encoding is decoded by `to_file_path` (unit-pinned). NO `dead_code` attr is needed:
    /// `handle_opened`'s body calls it, which rustc counts as a use even where `handle_opened` is itself
    /// dead (the `launch_origin` precedent). [Build-Session-Entscheidung: P2.137]
    fn opened_urls_to_paths(urls: &[tauri::Url]) -> Vec<PathBuf> {
        urls.iter().filter_map(|u| u.to_file_path().ok()).collect()
    }

    /// [Build-Session-Entscheidung: P2.57] §7.8.1 the FIRST-launch argv reader — at THIS instance's own launch
    /// (Windows `argv` / Linux `%F`/`%U` desktop-entry expansion → `argv`; the §7.1.1 single-instance callback
    /// covers SECOND launches, this covers the FIRST), read `std::env::args_os` + the launching cwd and route
    /// them through `forward_launch_argv` as `LaunchArg`. Reads `args_os` (NOT `args()`, which PANICS on a
    /// non-UTF8 arg — the in-core no-panic policy); a non-UTF8 arg is `to_string_lossy`'d (the §1.1 freeze
    /// re-validates the path + fails it clearly if the lossy form does not resolve, never a crash). `cwd` from
    /// `current_dir()` (lossy; on error → "" so absolute args still resolve, relative ones fail clearly).
    /// Unconditional (no per-OS `#[cfg]`): on macOS argv carries no file args (its launch files arrive via
    /// `RunEvent::Opened`, P2.56), so `parse_path_args` yields none and the funnel is a no-op there.
    /// AppHandle-coupled boot-glue (§1.1a; G28 signature-exempt); LIVE from `setup`, so the routing is
    /// source-scan-pinned + the §1.6 launch-with-files E2E exercises it. (The §7.2.1 ordered spine P2.106
    /// subsequently homes this call as step 7; here it is the launch-intake stage in `setup`.)
    pub(super) fn forward_first_launch_argv(app: &AppHandle) {
        // [Build-Session-Entscheidung: P2.57] G29/SAST per-finding suppression — the vendored audit rule
        // `rust.lang.security.args-os.args-os` ("don't rely on `args_os` for SECURITY") does NOT apply here:
        // `argv` is read ONLY as the §7.8.1 launch-FILE source — `parse_path_args` SKIPS `argv[0]` (the only
        // spoofable element the rule warns about) and the §1.1 freeze (canonicalise / resolve-identity /
        // existence / detection) re-validates every path before any decode. No security decision is made on
        // `argv`. The bare marker below carries only the rule-id (semgrep parses the rest of a `nosemgrep:`
        // line as comma-separated rule-ids, so the rationale stays on these separate lines).
        // nosemgrep: rust.lang.security.args-os.args-os
        let argv: Vec<String> = std::env::args_os()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        forward_launch_argv(app, &argv, &cwd, IntakeOrigin::LaunchArg);
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
    ///
    /// [Build-Session-Entscheidung: P2.79] Widened to `pub(super)` so the crate-root §7.3.2
    /// `dispatch_window_event` close guard reuses this ONE run-busy predicate — the spec mandates the launch
    /// refuse-busy gate (§7.1.1) and the close-requested guard (§7.3.2) share the SAME predicate, rather than
    /// each re-reading the `RunRegistry`. Visibility-only change; the `has_active_run` read is unchanged.
    pub(super) fn converter_is_busy(app: &AppHandle) -> bool {
        app.state::<RunRegistry>().has_active_run()
    }

    /// [Build-Session-Entscheidung: P2.59] The §7.8.1 WebView-ready predicate — reads the real
    /// `State<FrontendReady>` flag (`crate::orchestrator`, P2.59): `true` once the frontend has registered its
    /// `app://intake` listener and run the C1 `drainPending` drain on root-shell mount (P2.60 calls
    /// `mark_ready`). When ready, `intake_disposition` returns `Emit` (the funnel emits `app://intake`); when
    /// not-ready it returns `Buffer`, so a first-launch set arriving before the listener exists is stashed in
    /// `PendingIntake` (P2.58) instead of emitted into a listener that would drop it (the §7.8.1 race). The flag
    /// is `false` at app start (the fail-safe default — buffer, never emit, until the listener is proven), so
    /// while the run registry is empty (pre-P3) `converter_is_busy` is `false` and an idle launch set still
    /// routes to the real `PendingIntake` buffer until P2.60 wires the `mark_ready` drain — this box makes the
    /// `Emit` arm reachable. AppHandle-coupled boot-glue (§1.1a; G28 signature-exempt; the source-scan pins the
    /// `FrontendReady` query). `app.state::<FrontendReady>()` is infallible by construction (registered in
    /// main()'s Builder chain).
    fn frontend_ready(app: &AppHandle) -> bool {
        app.state::<FrontendReady>().is_ready()
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
    fn buffer_pending_intake(
        app: &AppHandle,
        paths: Vec<PathBuf>,
        origin: IntakeOrigin,
    ) -> crate::orchestrator::StashOutcome {
        let pending = app.state::<PendingIntake>();
        let ready = app.state::<FrontendReady>();
        pending.stash_or_route(ready.inner(), paths, origin)
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
            // [Test-Change: P2.137 — old-obsolete+new-correct, §7.8.1] the Buffer arm now returns the fused
            // no-loss `StashOutcome` (stash-or-route), so the signature pin follows the new return type.
            let _buffer: fn(
                &AppHandle,
                Vec<PathBuf>,
                IntakeOrigin,
            ) -> crate::orchestrator::StashOutcome = buffer_pending_intake;
            let _opened: fn(&AppHandle, &[tauri::Url]) = handle_opened;
            let _origin: fn(bool) -> IntakeOrigin = launch_origin;
            let _argvread: fn(&AppHandle) = forward_first_launch_argv;
        }

        // §6.4.1 unit (G15): the §7.1.1 single-instance second-launch HANDLER (P2.52). `on_second_instance`
        // (in `launch_intake`, covered by `production_boot_source`) re-focuses `main` + forwards the argv as
        // `SecondInstance`; `main()` (covered by `production_main_body`) WIRES it into the single-instance
        // `init`. Both are AppHandle-coupled boot-glue (not execution-testable; the boot-stage pattern), so
        // source scans pin them — with FULL-CALL-SITE needles (the `c2a_contract` / `verbose_gateway`
        // discipline), because the P2.52 bare tokens went ALIASED as later boxes landed: bare
        // `IntakeOrigin::SecondInstance` also matches `launch_origin` (P2.56) and bare
        // `get_webview_window("main")` also matches `reveal_main_window` (P2.106.6), so a wrong-origin or
        // deleted-forward regression stayed green. [Test-Change: P2.137 — old-obsolete+new-correct, §7.1.1]
        // the old needles' uniqueness claim was stale; the full-call forms below match only
        // `on_second_instance`'s body / the `init` registration. [Build-Session-Entscheidung: P2.52]
        #[test]
        fn single_instance_handler_refocuses_forwards_and_is_wired() {
            let handler = crate::boot_invariants::production_boot_source();
            for needle in [
                concat!("w.set_", "focus()"), // re-focus: unique to on_second_instance's .map(|w| …) body
                concat!(
                    "forward_launch_",
                    "argv(app, &argv, &cwd, IntakeOrigin::SecondInstance)"
                ), // the FULL forward call — a wrong-origin regression cannot alias via launch_origin
            ] {
                assert!(
                    handler.contains(needle),
                    "§7.1.1: on_second_instance must re-focus `main` + forward the argv as SecondInstance (missing `{needle}`)"
                );
            }
            let main_body = crate::boot_invariants::production_main_body();
            assert!(
                main_body.contains(concat!("tauri_plugin_single_", "instance::init(")),
                "§7.1.1: main() must register the single-instance plugin"
            );
            assert!(
                main_body.contains(concat!("launch_intake::on_second_", "instance,")),
                "§7.1.1: main() must pass on_second_instance as the init handler (the fn-item code form, \
                 not a doc mention)"
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

        // §6.4.1 unit (G15, P2.137): an EMPTY argv token (a buggy wrapper script / desktop-entry quoting
        // artifact, `convertia ""`) is DROPPED — cwd-joining it would resolve to the launching DIRECTORY
        // itself and silently turn the user's whole cwd into a recursively-walked ingest root (§1.1 folder
        // recursion). The classifier never invents a path the user did not name (§7.8.1).
        #[test]
        fn parse_path_args_drops_empty_tokens() {
            let cwd = "base";
            let argv = vec![
                "convertia".to_string(),
                String::new(),
                "keep.txt".to_string(),
            ];
            let parsed = parse_path_args(&argv, cwd);
            assert_eq!(
                parsed,
                vec![PathBuf::from(cwd).join("keep.txt")],
                "§7.8.1: an empty token yields NO path (never the launching cwd itself)"
            );
            assert!(
                !parsed.contains(&PathBuf::from(cwd)),
                "§7.8.1: the bare launching cwd never appears as an ingestable path"
            );
        }

        // §6.4.1 unit (G15, P2.137): the documented Windows drive-relative posture — a `C:file.txt` token
        // (prefix, no root → not absolute) passes through effectively UNCHANGED (`Path::join` with a
        // prefix-bearing right side REPLACES the base, documented std semantics): it resolves against the
        // per-drive cwd at freeze time, where a missing file fails clearly (§2.8); classification never
        // rewrites a drive-qualified token.
        #[cfg(windows)]
        #[test]
        fn parse_path_args_passes_drive_relative_tokens_through() {
            let argv = vec!["convertia".to_string(), "C:file.txt".to_string()];
            assert_eq!(
                parse_path_args(&argv, "base"),
                vec![PathBuf::from("C:file.txt")],
                "§7.8.1: a drive-relative token passes through unchanged (the documented posture)"
            );
        }

        /// The §6.4.2/G16 case-count floor + pinned-seed runner for this module's classifier property (the
        /// P2.14 / P2.126 / P2.127 discipline). [Build-Session-Entscheidung: P2.137]
        fn pinned_classifier_runner() -> proptest::test_runner::TestRunner {
            use proptest::test_runner::{RngAlgorithm, TestRng, TestRunner};
            TestRunner::new_with_rng(
                proptest::prelude::ProptestConfig::with_cases(512),
                TestRng::deterministic_rng(RngAlgorithm::ChaCha),
            )
        }

        // §6.4.2 property (G16, P2.137): the §7.8.1 classifier invariants over arbitrary token vectors —
        // every returned path is exactly the cwd-join of a non-switch, non-empty token after argv[0]
        // (relative tokens; the absolute/drive-relative arms have their own example pins), the returned
        // count equals the count of such tokens, and the bare launching cwd never appears (the empty-token
        // guard). Pinned seed, 512 cases (test-strategy §1.3).
        #[test]
        fn parse_path_args_classification_invariants_hold_for_arbitrary_argv() {
            use proptest::prelude::*;
            let token = prop_oneof![
                Just(String::new()),                                  // the empty-token artifact
                "-{1,2}[a-z]{1,6}".prop_map(|s| s),                   // launch switches
                "[a-z][a-z0-9]{0,7}(\\.[a-z]{1,3})?".prop_map(|s| s), // relative path names
            ];
            pinned_classifier_runner()
                .run(&proptest::collection::vec(token, 0..12), |tokens| {
                    let cwd = "prop-base";
                    let mut argv = vec!["convertia".to_string()];
                    argv.extend(tokens.iter().cloned());
                    let parsed = parse_path_args(&argv, cwd);
                    let expected: Vec<PathBuf> = tokens
                        .iter()
                        .filter(|t| !t.is_empty() && !t.starts_with('-'))
                        .map(|t| PathBuf::from(cwd).join(t))
                        .collect();
                    prop_assert_eq!(
                        &parsed,
                        &expected,
                        "§7.8.1: exactly the non-switch, non-empty tokens classify as paths"
                    );
                    prop_assert!(
                        !parsed.contains(&PathBuf::from(cwd)),
                        "§7.8.1: the bare launching cwd is never an ingestable path"
                    );
                    Ok(())
                })
                .expect("§7.8.1: the classifier property must hold (pinned seed, 512 cases)");
        }

        // §6.4.1 unit (G15, P2.137): the §7.8.1 empty-SET no-op guard is WIRED — the funnel returns early on
        // an empty path set, so a bare flags-only relaunch never occupies the PendingIntake slot (whose
        // `stash_or_route` waives its own empty guard on this one) and never emits a spurious `app://intake`
        // against the §0.4.2 row semantics.
        #[test]
        fn funnel_returns_early_on_an_empty_path_set() {
            let src = crate::boot_invariants::production_boot_source();
            assert!(
                src.contains(concat!("if paths.is_", "empty() {")),
                "§7.8.1: forward_launch_intake must no-op an empty path set BEFORE the disposition dispatch"
            );
        }

        // §6.4.1 unit (G15, P2.137): the §0.4.2 `app://intake` emit surface is CLOSED — exactly ONE
        // production emit call site exists (`emit_intake`; both funnel arms route through it), so a future
        // box adding a second emit — which would bypass the §7.1.1 refuse-busy gate and the §2.4 freeze
        // protection — reds here. Walks every crate source file, each stripped at its first test-module
        // boundary (the established per-file production-prefix discipline); the needles carry the leading
        // `.` of the method call so `emit_intake`'s own doc prose never aliases.
        #[test]
        fn app_intake_has_exactly_one_production_emit_site() {
            fn collect_production_sources(dir: &std::path::Path, out: &mut String) {
                let entries = std::fs::read_dir(dir).expect("crate src dir is readable");
                for entry in entries {
                    let path = entry.expect("dir entry is readable").path();
                    if path.is_dir() {
                        collect_production_sources(&path, out);
                    } else if path.extension().is_some_and(|e| e == "rs") {
                        let full = std::fs::read_to_string(&path).expect("source file is readable");
                        let prefix = full
                            .split_once(concat!("#[cfg", "(test)]"))
                            .map_or(full.as_str(), |(p, _)| p);
                        out.push_str(prefix);
                    }
                }
            }
            let mut src = String::new();
            collect_production_sources(
                std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src")),
                &mut src,
            );
            let count = src.matches(concat!(".emit(events::APP_", "INTAKE")).count()
                + src
                    .matches(concat!(".emit(crate::ipc::events::APP_", "INTAKE"))
                    .count();
            assert_eq!(
                count, 1,
                "§0.4.2/§7.1.1: exactly ONE production app://intake emit site (emit_intake) — a second \
                 site would bypass the refuse-busy gate + the §2.4 freeze protection"
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
            // [Test-Change: P2.137 — old-obsolete+new-correct, §7.8.1] the P2.58 `.stash(` call form was
            // fused into `stash_or_route` (the no-loss closure); the needle follows the new sole call form.
            assert!(
                buffer_src.contains(concat!(".stash_or_route(", "ready.inner(), paths, origin)")),
                "§7.8.1: buffer_pending_intake must stash-or-route the launch set + origin under the fused \
                 no-loss protocol (P2.137; not the P2.54 no-op shell)"
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

        // §6.4.1 unit (G15): the §7.8.1 WebView-ready predicate is WIRED to the real State<FrontendReady> flag
        // (P2.59) — not the P2.54 fail-safe `false` shell. `frontend_ready` is AppHandle-coupled boot-glue (the
        // §1.1a boot-stage pattern — not cargo-test execution-testable; the mark/is_ready LOGIC is unit-tested on
        // `crate::orchestrator::FrontendReady`), so a source-scan pins it resolves the managed FrontendReady +
        // reads is_ready; a second scan pins `main()` REGISTERS it (else the resolve would fail). Needles
        // `concat!`-assembled (the established self-match avoidance). [Build-Session-Entscheidung: P2.59]
        #[test]
        fn ready_predicate_reads_the_managed_frontend_ready_flag() {
            let boot_src = crate::boot_invariants::production_boot_source();
            assert!(
                boot_src.contains(concat!("state::<Frontend", "Ready>()")),
                "§7.8.1: frontend_ready must resolve the managed State<FrontendReady> (P2.59)"
            );
            assert!(
                boot_src.contains(concat!(".is_", "ready()")),
                "§7.8.1: frontend_ready must read the real ready flag, not the P2.54 fail-safe `false`"
            );
            let main_src = crate::boot_invariants::production_main_body();
            assert!(
                main_src.contains(concat!(
                    ".manage(crate::orchestrator::Frontend",
                    "Ready::default())"
                )),
                "§7.8.1: main() must register the State<FrontendReady> so the frontend_ready resolve cannot fail (P2.59)"
            );
        }

        // §6.4.1 unit (G15): main() REGISTERS the §0.4.4 IngestRegistry (P2.70) so the C2a `pick_for_intake`
        // handler's `app.state::<IngestRegistry>()` resolve cannot fail — the store its RAII guard registers
        // the `CollectingId` token in BEFORE opening the native dialog (§1.1). A source-scan on main()'s body
        // (the `.manage` is boot-glue, not cargo-test-runnable; the registry LOGIC is unit-tested on
        // `crate::orchestrator::IngestRegistry`). Needle `concat!`-assembled (self-match avoidance).
        // [Build-Session-Entscheidung: P2.70]
        #[test]
        fn main_registers_the_managed_ingest_registry() {
            let main_src = crate::boot_invariants::production_main_body();
            assert!(
                main_src.contains(concat!(
                    ".manage(crate::orchestrator::Ingest",
                    "Registry::default())"
                )),
                "§0.4.4/§1.1: main() must register the State<IngestRegistry> so the C2a register_guard resolve cannot fail (P2.70)"
            );
        }

        // §6.4.1 unit (G15): the §7.8.1 launch-origin decision (P2.56) — a first-launch Open-with (WebView not
        // ready) is LaunchArg (buffered + drained); a while-running Open-with is SecondInstance. The pure rule
        // beside the macOS Open-with glue (the §1.1a boot-stage pattern), unit-tested in isolation.
        #[test]
        fn launch_origin_maps_readiness_to_intake_origin() {
            assert_eq!(
                launch_origin(false),
                IntakeOrigin::LaunchArg,
                "§7.8.1: a first-launch Open-with (frontend not ready) is LaunchArg (buffered + drained)"
            );
            assert_eq!(
                launch_origin(true),
                IntakeOrigin::SecondInstance,
                "§7.8.1: a while-running Open-with (frontend ready) is SecondInstance"
            );
        }

        // §6.4.1 unit (G15): the macOS Open-with handler logic (P2.56). `handle_opened` is AppHandle-coupled
        // boot-glue (the §1.1a boot-stage pattern — its runtime is the §6.4.6 macOS E2E smoke leg, not
        // cargo-test; it is wired into the App::run Opened arm by P2.82), so a source-scan pins it converts
        // the urls via the extracted pure `opened_urls_to_paths` rule (unit-tested below, P2.137) and routes
        // through forward_launch_intake with the launch_origin-resolved origin. Needles concat!-assembled.
        // [Test-Change: P2.137 — old-obsolete+new-correct, §7.8.1] the inline `to_file_path()` filter moved
        // into the extracted `opened_urls_to_paths` (the §1.1a pure-rule split); the needles follow the new
        // sole call form, and the CONVERSION semantics are now behaviourally unit-tested, not needle-only.
        // [Build-Session-Entscheidung: P2.56]
        #[test]
        fn opened_handler_routes_urls_through_the_funnel() {
            let src = crate::boot_invariants::production_boot_source();
            for needle in [
                concat!("opened_urls_to_", "paths(urls)"),
                concat!("launch_origin(frontend_", "ready(app)),"),
            ] {
                assert!(
                    src.contains(needle),
                    "§7.8.1: handle_opened must convert Open-with urls → paths and route through the funnel (missing `{needle}`)"
                );
            }
        }

        // §6.4.1 unit (G15, P2.137): the §7.8.1 Open-with URL→path conversion — behavioural tests on the
        // extracted pure rule (`opened_urls_to_paths`): a percent-encoded `file://` URL decodes to the real
        // path, a non-`file` scheme is DROPPED while its siblings survive (§7.8.2 registers no URL scheme),
        // and on Windows the `file://host/share` UNC form maps to the UNC path.
        #[test]
        fn opened_urls_convert_percent_encoding_and_drop_non_file_schemes() {
            // A drive-letter file URL converts on EVERY target (Windows `to_file_path` requires the drive;
            // on POSIX it maps to the literal `/C:/…` path) — one fixture, both platforms.
            let urls = [
                tauri::Url::parse("file:///C:/tmp/with%20space.csv").expect("test URL parses"),
                tauri::Url::parse("https://example.invalid/not-a-file.csv")
                    .expect("test URL parses"),
            ];
            let paths = opened_urls_to_paths(&urls);
            assert_eq!(
                paths.len(),
                1,
                "§7.8.1/§7.8.2: the non-file scheme is dropped, the file URL survives"
            );
            assert!(
                paths[0].to_string_lossy().ends_with("with space.csv"),
                "§7.8.1: percent-encoding is decoded by Url::to_file_path (got {:?})",
                paths[0]
            );
        }

        #[cfg(windows)]
        #[test]
        fn opened_urls_map_host_bearing_file_urls_to_unc_on_windows() {
            let urls = [tauri::Url::parse("file://server/share/doc.csv").expect("test URL parses")];
            let paths = opened_urls_to_paths(&urls);
            assert_eq!(
                paths,
                vec![PathBuf::from(r"\\server\share\doc.csv")],
                "§7.8.1: a host-bearing file URL maps to the UNC form on Windows"
            );
        }

        // §6.4.1 unit (G15): the §7.8.1 first-launch argv reader (P2.57). forward_first_launch_argv is
        // AppHandle-coupled boot-glue (the §1.1a boot-stage pattern — runtime is the §6.4.6 launch-with-files
        // E2E, not cargo-test), so a source-scan pins: it reads std::env::args_os (NOT the panic-on-non-UTF8
        // args()) and routes through forward_launch_argv as LaunchArg; and setup() CALLS it (the launch-intake
        // stage). Needles concat!-assembled. [Build-Session-Entscheidung: P2.57]
        #[test]
        fn first_launch_argv_reader_routes_through_the_funnel() {
            let src = crate::boot_invariants::production_boot_source();
            for needle in [
                concat!("env::args_", "os()"),
                concat!(
                    "forward_launch_",
                    "argv(app, &argv, &cwd, IntakeOrigin::Launch",
                    "Arg)"
                ),
            ] {
                assert!(
                    src.contains(needle),
                    "§7.8.1: forward_first_launch_argv must read args_os + route as LaunchArg (missing `{needle}`)"
                );
            }
            let main_src = crate::boot_invariants::production_main_body();
            assert!(
                main_src.contains(concat!("forward_first_launch_", "argv(app.handle())")),
                "§7.8.1: setup must call forward_first_launch_argv at first launch (the launch-intake stage, P2.57)"
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
        // [Build-Session-Entscheidung: P2.89] §7.5.1/§7.5.2 logging — the configured plugin (local rotating
        // file + dev stderr, level `info`, no `Stdout`/network sink) from `log_plugin()`. The target
        // whitelist + level live in that helper (`log_targets()` is the pure, tested §3 zero-egress control);
        // this chain line is the single production registration site.
        .plugin(log_plugin())
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
        // [Build-Session-Entscheidung: P2.59] §7.8.1 WebView-ready flag — register the State<FrontendReady>
        // (crate::orchestrator) so the §7.8.1 funnel's `frontend_ready` predicate can read it. `false` at startup
        // (buffer, never emit, until the frontend proves its `app://intake` listener) → the C1 `drainPending`
        // drain (P2.60) flips it ready on root-shell mount via `mark_ready`. Builder chain (compile-time Default,
        // before the event loop) → the `frontend_ready` resolve is infallible by construction (no panic under the
        // crate-root clippy::panic deny). Its live consumer — the P2 launch funnel — exists now.
        .manage(crate::orchestrator::FrontendReady::default())
        // [Build-Session-Entscheidung: P2.70] §0.4.4 ingest-cancellation store — register the IngestRegistry
        // (the CollectingId → token store, P2.45) so the C2a pick_for_intake handler can register the
        // CollectingId token via its RAII guard BEFORE opening the native dialog (§1.1), and C13 cancel_ingest
        // can trip it (P2.71). Builder chain (compile-time Default, before the event loop) → the handler's
        // app.state::<IngestRegistry>() resolve is infallible by construction (no panic under the crate-root
        // clippy::panic deny). Its first live consumer — the P2.70 C2a dialog handler — exists now.
        .manage(crate::orchestrator::IngestRegistry::default())
        .invoke_handler(builder.invoke_handler())
        // [Build-Session-Entscheidung: P2.79] §7.3.2 window-lifecycle hook — the Tauri v2 two-arg
        // `(&Window, &WindowEvent)` `on_window_event` closure. Thin BY DESIGN (a Builder-chain delegation
        // line, the established `.setup`/`.plugin` main-body pattern) — the interceptor logic lives in the
        // AppHandle-signatured `dispatch_window_event`, which is G28 diff-floor-exempt + source-scan-pinned.
        // It resolves the run-level `&AppHandle` (`window.app_handle()`, §1.9) and forwards the event so a
        // mid-conversion `CloseRequested` is blocked (`prevent_close`) and the §5.2 confirm UI is signalled via
        // the `app://close-requested` event (§7.3.2/§7.3.3, never-harm).
        .on_window_event(|window, event| dispatch_window_event(window.app_handle(), event))
        .setup(move |app| {
            // §0.4.5 IPC event-channel mount (the P1.13 tauri-specta seam).
            builder.mount_events(app);

            // [Build-Session-Entscheidung: P2.94] §7.5.3 resolve the verbose log level ONCE at startup
            // (`verboseLog` pref || `--verbose` flag → raise `log::max_level` to `Debug`; else `Info`). Read
            // here in setup — the first point the AppHandle is live, so `prefs::load` can resolve the config
            // dir — because the log plugin is registered on the Builder before any AppHandle exists (§7.5.3).
            // A mid-session About-toggle takes effect on the next launch (setup runs once). No `debug!` fires
            // in the boot path before this line, so the tiny window after the plugin's own plugin-init
            // set_max_level (which this overrides) records nothing verbose unintentionally.
            resolve_log_verbosity(app.handle());

            // ── §7.2.1 ordered startup sequence — the app-shell spine (steps 1–8, P2.106) ──────────────
            // [Build-Session-Entscheidung: P2.106] P2.106 establishes the §7.2.1 order over the boot stages
            // P1/P2 landed. Step 1 (single-instance guard) is registered FIRST on the Builder above (§7.1.1,
            // P1.14/P2.51/P2.52) so it wins before any window. Steps 2–7 run here in order; the window is
            // revealed (step 6) ONLY after the readiness steps 3–5 succeed (§7.2.1), so a hard startup fault
            // renders as a clean §2.13 fault screen, never a half-broken UI; step 8 hands to the §5.2 Idle UI
            // (the React root shell, P2.106.8). The step 3–5 readiness bodies are SLOTs the P4 engine + §2.6
            // layer fill; a step fault is presented by `present_startup_fault` (the §2.13.3 mechanism, body
            // P2.109/P4). The `mount_events` + `resolve_log_verbosity` calls above are the tauri-specta IPC
            // seam + the §7.5.3 log level — infra the ordered §7.2.1 steps run after, not numbered steps.

            // §7.2.1 step 2 — establish the per-launch InstanceId as an app-managed SINGLETON (§7.1.2: a
            // random v4, the spec's "app-managed singleton via app.manage(...)"; process-local — never
            // persisted, never networked, §2.11) and resolve the three base dirs via app.path() (config /
            // local-data scratch §2.14 / log §7.5). NO directory is created here — directory creation is
            // §7.2.1 step 5 (`prepare_scratch_and_log`). Each call below touches only local uuid + filesystem
            // primitives, so the boot path opens no socket (§7.2.2; G29 first-party rule (g) backstops the
            // whole tree; the boot-invariant test covers the top-of-file import surface).
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

            // §7.2.1 steps 3–7 — the readiness gate, the window reveal, and the launch-intake feed, in order.
            // The three readiness SLOTs (engine presence + integrity 3 / exec-perm 4 / scratch + log + orphan
            // reclaim 5) return `Ok` now (bodies P3/P4), so the ready path is taken; when P4 fills them a
            // failing step yields an app-level `AppFault` the `Err` arm presents (§2.13.3) with the window
            // still hidden. [Build-Session-Entscheidung: P2.106]
            match readiness_checks(app.handle()) {
                // §7.2.1 steps 3–5 passed → step 6: reveal the config-declared single `main` window (created
                // HIDDEN via `visible: false`, P1.16/P1.19) — the "shown only after 3–5 succeed" rule (§7.2.1),
                // so a hard fault is a clean §2.13 screen, never a half-broken UI — then step 7: feed THIS
                // instance's first-launch argv (Windows / Linux `%F`/`%U`) through the §7.8.1 funnel as
                // `LaunchArg` (P2.57; on macOS argv carries no file args — Open-with arrives via
                // `RunEvent::Opened`, P2.56 — so a no-op there). §7.2.1 step 8 (hand to the §5.2 Idle UI) is
                // then the WebView's: the React root shell (App.tsx) renders Idle + registers ready via the C1
                // `drainPending` `mark_ready` handshake (P2.60/P2.61/P2.106.8).
                Ok(()) => {
                    reveal_main_window(app.handle());
                    launch_intake::forward_first_launch_argv(app.handle());
                }
                // A readiness step faulted → present the §2.13.3 app-level startup fault (body P2.109/P4) and
                // leave the window hidden — a clean fault screen, never a half-broken UI (§7.2.1). Runtime-dead
                // until P4 fills the step-3–5 bodies (they return `Ok`), so no fault is lost meanwhile.
                Err(fault) => present_startup_fault(app.handle(), fault),
            }

            Ok(())
        })
        // [Build-Session-Entscheidung: P2.81] §7.3.2 the App::run event-loop handler is registered on the
        // BUILT `App` (`.build(ctx)?.run(...)`), NOT the `Builder` — the run-event closure is an `App` method.
        // Thin BY DESIGN (a main-body delegation line, the established `.setup`/`.on_window_event` pattern);
        // the run-event logic lives in the AppHandle-signatured `dispatch_run_event` (G28 diff-floor-exempt +
        // source-scan-pinned).
        .build(tauri::generate_context!())?
        .run(|app, event| dispatch_run_event(app, &event));

    Ok(())
}

#[cfg(test)]
mod boot_invariants {
    //! §7.2.2 boot invariant — the startup path opens no socket (`boot_path_opens_no_socket`, P1.15.1) and,
    //! now that the §7.2.1 ordered spine registers the full plugin set, the boot shell registers no
    //! network-capable plugin/client (`boot_shell_registers_no_network_plugin_or_client`, P2.107). The §6.7.1
    //! Lane-A compensating guard (cargo-test plane) for the Lane-B-only egress gate (§2.11.4 / §7.2.2), pairing
    //! with the P0 G29 first-party no-socket rule (g) at the source plane + the G18 `cargo-deny` dependency
    //! bans. [Build-Session-Entscheidung: P1.15.1]

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

    /// EVERYTHING the §7.2.1 boot path transitively EXECUTES, for the §7.2.2 zero-startup-network scans:
    /// `all_production_source()` PLUS the production prefixes of the module files the setup path reaches —
    /// `prefs.rs` (P2.94: `resolve_log_verbosity` → `crate::prefs::load` runs at boot). Without this, a
    /// socket opened inside a boot-reached module passed the ENTIRE cargo-test plane (the G29 SAST companion
    /// is CI-only), under-delivering the module docstring's "the startup path opens no socket" claim.
    /// Per-file production-prefix split (the established discipline). [Build-Session-Entscheidung: P2.137]
    pub(super) fn boot_reachable_source() -> String {
        let prefs = include_str!("prefs.rs");
        let prefs_prefix = prefs
            .split_once(concat!("#[cfg", "(test)]"))
            .map_or(prefs, |(prefix, _)| prefix);
        format!("{}\n{}", all_production_source(), prefs_prefix)
    }

    // §7.2.2 / §6.4.1 unit (G15): a structural assertion that the production boot path references no
    // network primitive — the cargo-test companion to the G29 source rule (g), scoped to §7.2.2. Scans
    // `all_production_source()` (prefix + `main()` body) because the §7.2.1 startup spine lives in `main()`,
    // which `production_boot_source()` alone does not reach (P2.54 — fixes the pre-existing P2.40 blindness).
    // [Test-Change: P2.137 — old-obsolete+new-correct, §7.2.2] corpus widened from `all_production_source()`
    // (main.rs only) to `boot_reachable_source()` (+ the boot-executed `prefs.rs` production prefix) — the
    // old corpus was blind to a socket in a boot-reached module; strictly stricter, same needles.
    #[test]
    fn boot_path_opens_no_socket() {
        let src = boot_reachable_source();
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

    // §7.2.2 / §2.11.1 (T9a) / §6.4.1 unit (G15): the SHELL adds ZERO startup network — the plugin/client
    // half that `boot_path_opens_no_socket` (the socket-primitive scan above) and `no_updater_posture` (the
    // updater) do not cover. Now that the §7.2.1 ordered spine (P2.106) registers the full Builder plugin
    // set, assert the boot shell registers NO network-CAPABLE Tauri plugin and pulls in NO broader
    // HTTP-client crate. This closes a real gap: `.plugin(tauri_plugin_http::init())` registers an
    // IPC-accessible HTTP client (CLAUDE §5 anti-pattern / G18) yet contains no `reqwest` literal, so the
    // socket scan above would miss it. The load-bearing enforcers stay the G18 `cargo-deny [bans]`
    // (dependency level) + the §0.10 CSP (WebView fetch) + G29 rule (g); this is their cargo-test-plane
    // companion scoped to the boot shell — the same defense-in-depth shape as `boot_path_opens_no_socket` /
    // `no_updater_posture`. Needles `concat!`-assembled so the tokens are not literals in this scanned
    // production file. [Build-Session-Entscheidung: P2.107]
    // [Test-Change: P2.137 — old-obsolete+new-correct, §7.2.2] corpus widened to `boot_reachable_source()`
    // (see `boot_path_opens_no_socket` above) — strictly stricter, same needles.
    #[test]
    fn boot_shell_registers_no_network_plugin_or_client() {
        let src = boot_reachable_source();
        let network_surfaces = [
            concat!("tauri_plugin_", "http"), // wraps reqwest, registers an IPC-accessible HTTP client
            concat!("tauri_plugin_", "upload"), // network upload/download plugin
            concat!("tauri_plugin_", "websocket"), // websocket client plugin
            concat!("tauri_plugin_", "oauth"), // binds a LOCAL HTTP server to catch OAuth redirects (same no-literal gap as http)
            concat!("isahc", "::"),            // libcurl-backed HTTP client
            concat!("curl", "::"),             // libcurl bindings
            concat!("surf", "::"),             // async HTTP client
            concat!("attohttpc", "::"),        // blocking HTTP client
            concat!("awc", "::"),              // the actix-web HTTP client
            concat!("tungstenite", "::"), // websocket client — the substring also catches `tokio_tungstenite::`
        ];
        for surface in network_surfaces {
            assert!(
                !src.contains(surface),
                "§7.2.2 zero-startup-network violated: the boot shell references `{surface}` — a \
                 network-capable plugin/client the shell must not register (pairs with G18 bans + the §0.10 CSP)"
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
    // [Test-Change: P3.76 — old-obsolete+new-correct, §2.10.1/§0.4.3] old: the set carried `PathBuf` (named via
    // `IpcError`'s former `path`/`residue: Option<PathBuf>` fields); new (verified by read-back of the
    // registered NAMES below): P3.76 re-types those fields to `path_display`/`residue_display: Option<String>`
    // (the 2026-07-06 core-owned-paths ruling — no `PathBuf` crosses the wire, §2.10.1), so `IpcError` no
    // longer references `PathBuf` and it drops out of the registered set. `String` remains (the display
    // fields + `message`). Net removal: `PathBuf` — a correct consequence of the re-typing, not a dropped
    // registration.
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
            "ConversionErrorKind,IpcError,LossyKind,OutcomeMsg,SkipReason,String",
            "§2.8.2/§0.4.3: the wire-taxonomy registration is LossyKind + IpcError + OutcomeMsg + the named \
             types they pull in (ConversionErrorKind / SkipReason / String — no PathBuf after the P3.76 \
             display-string re-typing, §2.10.1)"
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

    // §6.4.1 unit (G15): §7.2.1 step 6 — the `main` window is declared HIDDEN (`visible: false`) so the core
    // reveals it (via `reveal_main_window`, P2.106.6) ONLY after the readiness steps 3–5 succeed ("the window
    // is only shown once they succeed", §7.2.1). A regression dropping `visible: false` would show a
    // half-broken window before readiness — exactly the §7.2.1 anti-pattern. [Build-Session-Entscheidung: P2.106.6]
    #[test]
    fn main_window_declared_hidden_until_readiness() {
        let conf = tauri_conf();
        let main = conf["app"]["windows"]
            .as_array()
            .and_then(|windows| windows.first())
            .expect("§7.3.1: the single declared `main` window");
        assert_eq!(
            main["visible"],
            Value::Bool(false),
            "§7.2.1 step 6: the `main` window must be declared `visible: false` — shown only after readiness (steps 3–5, P2.106.6)"
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
mod window_lifecycle {
    //! §7.3.2 window lifecycle — the Tauri v2 `Builder::on_window_event` `CloseRequested` interceptor
    //! (P2.79). When a conversion run is in flight the core blocks the window close (`prevent_close`) so an
    //! ungraceful end can never truncate the in-flight output (§7.3.3; the SSOT never-harm origin). This is
    //! AppHandle-coupled boot-glue: this crate ships no `tauri::test` mock harness (the boot-stage pattern,
    //! test-strategy §1.1a), so the wiring is pinned by a SOURCE SCAN of the production source (the runtime
    //! is the §6.4.6 window-close E2E leg), not cargo-test execution. Reusing the ONE §7.1.1 run-busy
    //! predicate is the §7.3.2 mandate ("the same predicate the close guard uses"); the `has_active_run`
    //! logic itself is unit-tested on `crate::orchestrator::RunRegistry`. [Build-Session-Entscheidung: P2.79]

    // §6.4.1 unit (G15): `main()` WIRES the §7.3.2 `on_window_event` hook and delegates to the named
    // `dispatch_window_event` with the resolved `&AppHandle` (the thin-main-closure pattern). Scans
    // `all_production_source()` (prefix + `main()` body, every `cfg(test)` module excluded so these needles
    // can never self-match). Needles `concat!`-assembled (the established self-match avoidance).
    #[test]
    fn main_wires_the_on_window_event_close_hook() {
        let src = super::boot_invariants::all_production_source();
        assert!(
            src.contains(concat!(".on_window_", "event(|window, event|")),
            "§7.3.2: main() must register the Tauri v2 on_window_event hook (the close-requested interceptor)"
        );
        assert!(
            src.contains(concat!(
                "dispatch_window_",
                "event(window.app_handle(), event)"
            )),
            "§7.3.2: the on_window_event closure must resolve the run-level &AppHandle and delegate to dispatch_window_event"
        );
    }

    // §6.4.1 unit (G15): `dispatch_window_event` intercepts `CloseRequested` and, when a run is in flight,
    // blocks the close via the ONE shared §7.1.1 run-busy predicate (§7.3.2 "the same predicate") — the
    // §7.3.3 core "blocks the close" guarantee. Source-scan (AppHandle-coupled boot-glue). Needles
    // `concat!`-assembled.
    #[test]
    fn close_requested_guard_prevents_close_when_busy() {
        let src = super::boot_invariants::all_production_source();
        for needle in [
            concat!("WindowEvent::Close", "Requested { api, .. }"), // intercepts the §7.3.2 close event
            concat!("converter_is_", "busy(app)"), // reuses the ONE §7.1.1 run-busy predicate (§7.3.2)
            concat!("api.prevent_", "close()"),    // §7.3.3: the core blocks the close mid-run
        ] {
            assert!(
                src.contains(needle),
                "§7.3.2/§7.3.3: dispatch_window_event must block the close when busy (missing `{needle}`)"
            );
        }
    }

    // §6.4.1 unit (G15): when busy, `dispatch_window_event` ALSO emits the §0.4.2 `app://close-requested`
    // signal so the §5.2 WebView confirm UI renders (§7.3.3). The event name comes from the
    // `crate::ipc::events` constant (never a re-spelled literal — plan-lint check 28), and the payload is the
    // `CloseRequestedSignal` unit struct (serialises to JSON `null` — the §7.3.2 null-payload form, not `()`).
    // Source-scan (AppHandle-coupled boot-glue); needles `concat!`-assembled (self-match avoidance).
    #[test]
    fn close_requested_emits_the_confirm_signal_via_the_events_constant() {
        let src = super::boot_invariants::all_production_source();
        for needle in [
            concat!("crate::ipc::events::APP_CLOSE_", "REQUESTED,"), // emits via the events constant, not a literal
            concat!("CloseRequested", "Signal,"), // the null-payload unit struct (§7.3.2), not the bare `()`
        ] {
            assert!(
                src.contains(needle),
                "§7.3.2/§0.4.2: dispatch_window_event must emit app://close-requested with the null-payload signal (missing `{needle}`)"
            );
        }
    }

    // §6.4.1 unit (G15, P2.137): the §7.3.2 wire form is EXECUTED, not comment-only — `CloseRequestedSignal`
    // is a unit struct precisely because serde serialises it to JSON `null` (the §7.3.2-sanctioned form; a
    // braced `struct CloseRequestedSignal {}` would silently ship `{}` instead, with every needle green).
    // The one piece of this emit that IS cargo-testable (pure serde, no AppHandle); `serde_json` is the
    // established dev-dependency (the P2.80 doc comment names exactly this check).
    #[test]
    fn close_requested_signal_serialises_to_json_null() {
        let wire = serde_json::to_value(super::CloseRequestedSignal)
            .expect("§7.3.2: the close-requested signal serialises");
        assert_eq!(
            wire,
            serde_json::Value::Null,
            "§7.3.2/§0.4.2: app://close-requested carries JSON null (the unit-struct wire form), never {{}}"
        );
    }
}

#[cfg(test)]
mod run_lifecycle {
    //! §7.3.2 app run-event lifecycle — the `App::run` handler registered on the BUILT `App`
    //! (`.build(ctx)?.run(|app, event| dispatch_run_event(app, &event))`, NOT the `Builder`), P2.81. It owns
    //! `RunEvent::ExitRequested` (the last `prevent_exit` chance, the §7.3.3 quit-guard hook site) and
    //! `RunEvent::Exit` (the final cleanup point — flush the plugin logger; the §2.6 best-effort scratch
    //! cleanup call joins at P3.74). `RunEvent` is external `#[non_exhaustive]`, so the `_ =>` arm is
    //! mandatory (the item-level `#[allow(clippy::wildcard_enum_match_arm)]` is the gate-sanctioned escape,
    //! the known-variant set being platform-dependent). AppHandle-coupled boot-glue: the wiring is
    //! source-scan-pinned (this crate ships no `tauri::test` mock harness — the boot-stage pattern,
    //! test-strategy §1.1a), the runtime is the §6.4.6 E2E leg. [Build-Session-Entscheidung: P2.81]

    // §6.4.1 unit (G15): main() registers the run-event handler on the BUILT App (the `.build(ctx)?.run(...)`
    // refactor) and delegates to the named `dispatch_run_event`. Scans `all_production_source()` (prefix +
    // main body, every `cfg(test)` module excluded so these needles can never self-match). Needles
    // `concat!`-assembled (the established self-match avoidance).
    #[test]
    fn main_runs_the_event_handler_on_the_built_app() {
        let src = super::boot_invariants::all_production_source();
        for needle in [
            concat!(".build(tauri::generate_", "context!())?"), // handler on the BUILT App, not the Builder
            concat!(".run(|app, event| dispatch_run_", "event(app, &event))"), // the App::run closure delegates to the named exempt fn
        ] {
            assert!(
                src.contains(needle),
                "§7.3.2: main() must register the App::run handler on the built App + delegate (missing `{needle}`)"
            );
        }
    }

    // §6.4.1 unit (G15): `dispatch_run_event` wires the two §7.3.2 arms — `ExitRequested` (the busy-gated
    // §7.3.3 QUIT guard: `prevent_exit` + the `app://close-requested` confirm signal, sharing the ONE
    // §7.3.2 busy predicate with the window-close guard; user/OS quits only — a programmatic
    // `app.exit(code)` is never blocked) + `Exit` (flush the plugin logger, the final cleanup point) — plus
    // the mandatory non-exhaustive wildcard via the item-level allow. Source-scan (AppHandle-coupled
    // boot-glue). Needles `concat!`-assembled.
    // [Test-Change: P2.137 — old-obsolete+new-correct, §7.3.2/§7.3.3] the old needle pinned the EMPTY
    // `ExitRequested { .. } => {}` arm — implementation-mirroring, and stale against §7.3.2's own
    // "last chance to `api.prevent_exit()`" + §7.3.3's block-quit-while-busy mandate (the macOS app-menu
    // Quit / Cmd+Q path never raises a per-window CloseRequested, so the window-close guard alone
    // under-delivered). The new needles pin the spec contract: the busy predicate, the prevent, the signal.
    #[test]
    fn run_event_handler_wires_exit_requested_hook_and_exit_flush() {
        let src = super::boot_invariants::all_production_source();
        for needle in [
            concat!(
                "RunEvent::Exit",
                "Requested { api, code, .. } if code.is_none()"
            ), // the §7.3.3 QUIT leg (user/OS quits only)
            concat!("api.prevent_", "exit();"), // the busy-gated block (§7.3.2 "last chance")
            concat!("RunEvent::Exit ", "=> {"), // the final cleanup point arm (§7.3.2)
            concat!("log::logger().", "flush()"), // flush the plugin logger before exit (§7.5)
            concat!("allow(clippy::wildcard_enum_", "match_arm)"), // the gate-sanctioned per-item escape
        ] {
            assert!(
                src.contains(needle),
                "§7.3.2/§7.3.3: dispatch_run_event must wire the busy-gated ExitRequested quit guard + Exit(flush) + the item allow (missing `{needle}`)"
            );
        }
    }

    // §6.4.1 unit (G15): the §7.8.1 macOS Open-with arm (P2.82) — `dispatch_run_event` routes the `#[cfg]`-gated
    // `RunEvent::Opened` through the SAME §7.8.1 funnel via `handle_opened` (P2.56), so the §7.1.1 refuse-busy
    // gate + the §1.1 freeze apply to a macOS Open-with too. The arm is `#[cfg]`-gated to the Apple/mobile
    // targets (compiled out on Win/Linux) — but `include_str!` reads the raw TEXT, so this pins the wiring on
    // EVERY platform; the arm's compile-correctness is the macos-14 compile-sanity leg. Needles
    // `concat!`-assembled (self-match avoidance).
    #[test]
    fn run_event_handler_routes_opened_through_the_funnel() {
        let src = super::boot_invariants::all_production_source();
        for needle in [
            concat!("RunEvent::Open", "ed { urls }"), // the macOS Open-with arm (§7.8.1)
            concat!("launch_intake::handle_open", "ed(app, urls)"), // routes through the P2.56 funnel handler
        ] {
            assert!(
                src.contains(needle),
                "§7.8.1: dispatch_run_event must route RunEvent::Opened through handle_opened (missing `{needle}`)"
            );
        }
    }
}

#[cfg(test)]
mod quit_while_converting {
    //! §7.3.3 the quit-while-converting contract — CLASSIFIED **contract-only** (the P2.81-review Co-Pilot
    //! flag). §7.3.3 mandates that quitting while a run is in flight is "the SAME code path as an in-UI
    //! Cancel": confirm → cancel the in-flight run (the §1.7 mechanism, surfaced via C7 `cancel_run`) → the
    //! §2.6 cleanup → exit; the idle state quits immediately with no prompt. The cancel + cleanup MECHANISM is
    //! the P3 §2.6/§1.7 kernel (C7 `cancel_run` body P3.52, §1.7 P3.44, cleanup P3.22), and §7.3.3 forbids a
    //! SEPARATE implementation — so this box authors NO core cancel/cleanup and NO `cleanup_run` call
    //! (contract-only, NOT deferred-invocation: no new P3 wiring box either, since the runtime is the frontend
    //! §5.2 confirm → the shared C7 `cancel_run` → the window close → the §7.3.2 `RunEvent::Exit` sweep, P3.74,
    //! reusing pieces that already exist as shells/hooks). It ASSERTS the contract-by-construction:
    //!   (1) idle quits immediately — EVERY close/quit prevent is busy-gated (`dispatch_window_event`'s
    //!       `prevent_close` sits inside the `converter_is_busy` branch, P2.79; the `RunEvent::ExitRequested`
    //!       quit leg's `prevent_exit` sits inside the SAME busy branch, P2.137 — and a programmatic
    //!       `app.exit(code)` is never blocked), so an idle close/quit is never blocked;
    //!   (2) a busy quit HANDS OFF to the §5.2 confirm UI (the `app://close-requested` emit, P2.80) rather than
    //!       inlining a cancel — the actual cancel is the shared C7 `cancel_run` the frontend calls on confirm;
    //!   (3) the core lifecycle inlines NO cancel/kill (delegated to C7), the "same path as in-UI Cancel"
    //!       forward-guard.
    //! Source-scan (AppHandle-coupled boot-glue; test-strategy §1.1a). [Build-Session-Entscheidung: P2.83]

    // §6.4.1 unit (G15): §7.3.3 "the idle state quits immediately" — EVERY close/quit prevent is busy-gated.
    // Both prevents (`prevent_close` on the window path, P2.79; `prevent_exit` on the quit leg, P2.137) sit
    // inside a `converter_is_busy` branch, so when the converter is idle the branches are skipped → the
    // close/quit proceeds immediately (no prompt). The CARDINALITY pin (exactly two busy branches, exactly
    // two prevents) makes an added UN-gated prevent red this test, not just a removed one. Scans
    // `all_production_source()`; needles `concat!`-assembled.
    // [Test-Change: P2.137 — old-obsolete+new-correct, §7.3.2/§7.3.3] the old single-needle contains() was
    // written against the one-prevent P2.79 world; P2.137 added the spec-mandated quit leg, so the pin is
    // now a two-leg cardinality assertion.
    #[test]
    fn idle_quits_immediately_every_prevent_is_busy_gated() {
        let src = super::boot_invariants::all_production_source();
        // [Test-Change: P2.137 — old-obsolete+new-correct, §7.3.2/§7.3.3] (see the test doc above)
        let busy_branches = src
            .matches(concat!("if launch_intake::converter_is_", "busy(app) {"))
            .count();
        assert_eq!(
            busy_branches, 2,
            "§7.3.3: exactly the two close/quit prevents (window close + quit leg) gate on converter_is_busy"
        );
        assert_eq!(
            src.matches(concat!("api.prevent_", "close();")).count(),
            1,
            "§7.3.2: exactly one prevent_close — inside the busy-gated window-close branch"
        );
        assert_eq!(
            src.matches(concat!("api.prevent_", "exit();")).count(),
            1,
            "§7.3.2/§7.3.3: exactly one prevent_exit — inside the busy-gated quit-leg branch (P2.137)"
        );
    }

    // §6.4.1 unit (G15, P2.137): the two guard legs emit the SAME §5.2 confirm signal — exactly TWO
    // `CloseRequestedSignal` emit sites exist (window-close, P2.80; quit leg, P2.137), so a leg silently
    // dropping its hand-off (or a third, un-reviewed emit site appearing) reds here. Needle
    // `concat!`-assembled (the struct name in prose would otherwise alias).
    #[test]
    fn both_guard_legs_emit_the_one_confirm_signal() {
        let src = super::boot_invariants::all_production_source();
        assert_eq!(
            src.matches(concat!("CloseRequested", "Signal,")).count(),
            2,
            "§7.3.2/§7.3.3: exactly two emit sites hand off to the §5.2 confirm UI — the window-close leg and the quit leg"
        );
    }

    // §6.4.1 unit (G15): §7.3.3 "the same code path as an in-UI Cancel" — a busy quit does NOT inline a core
    // cancel/kill; it HANDS OFF to the §5.2 confirm UI (the `app://close-requested` emit, P2.80), and the
    // actual cancel is the shared C7 `cancel_run` (§0.4.1) the frontend calls on confirm. Assert: the busy
    // path emits the confirm event, AND the core lifecycle authors NO inline cancellation-token trip — the
    // cancel machinery lives in `crate::ipc::conversion` (C7) / `crate::orchestrator`, never the main.rs
    // lifecycle (`all_production_source()` is main.rs only). Needles `concat!`-assembled.
    #[test]
    fn busy_quit_hands_off_to_the_confirm_ui_and_inlines_no_cancel() {
        let src = super::boot_invariants::all_production_source();
        assert!(
            src.contains(concat!("crate::ipc::events::APP_CLOSE_", "REQUESTED")),
            "§7.3.3: a busy quit hands off to the §5.2 confirm UI via the app://close-requested emit (P2.80)"
        );
        assert!(
            !src.contains(concat!(".can", "cel(")),
            "§7.3.3: the quit path must delegate the cancel to the shared C7 cancel_run (the in-UI-Cancel path), not inline a cancel in the lifecycle"
        );
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

#[cfg(test)]
mod log_config {
    use super::{
        argv_has_verbose, log_dir_target, log_plugin, log_targets, resolve_log_verbosity,
        verbose_opt_in, LOG_MAX_FILE_SIZE_BYTES,
    };
    use tauri_plugin_log::TargetKind;

    // §7.5.2 (P2.91): the rotating-file cap is pinned to the spec's 5 MB (bytes). With KeepOne the on-disk
    // footprint stays ~1x this (the plugin deletes rather than renames on rotation); the ≈1x source audit
    // lives in §7.5.2's `Audit trail` (the concrete audit vs the 2.8.0 pin, P2.92). This pins the value so a
    // drift away from §7.5.2's 5_000_000 fails until the spec is updated too.
    #[test]
    fn log_max_file_size_is_the_spec_cap() {
        assert_eq!(
            LOG_MAX_FILE_SIZE_BYTES, 5_000_000,
            "§7.5.2: the log rotation cap must be 5 MB (5_000_000 bytes)"
        );
    }

    // §7.5.2 (P2.90): the persistence target is the plugin's `LogDir` with the DEFAULT file name, whose
    // runtime path the plugin resolves via Tauri's `app_handle.path().app_log_dir()` (tauri-plugin-log 2.8.0
    // lib.rs:628) — the per-OS §7.5.2 location incl. the Linux config-dir deviation. Pinning
    // `LogDir { file_name: None }` (NOT a hard-coded `Folder { path }`, NOT a custom file name) guarantees the
    // per-OS resolution stays Tauri's `app_log_dir()`, not a literal that could drift from §7.5.2.
    #[test]
    fn log_dir_target_resolves_via_app_log_dir() {
        assert!(
            matches!(log_dir_target(), TargetKind::LogDir { file_name: None }),
            "§7.5.2: the log dir must be the app_log_dir()-resolved LogDir target (default name), not a hard-coded path"
        );
    }

    // §7.5.2 / §3 zero-egress: the log target set is LOCAL-ONLY — exactly one on-disk persistence target
    // (`LogDir`) plus, in dev only, `Stderr`; NEVER `Webview`/`Dispatch`/`Folder`/`Stdout`, so no network
    // sink can ever reach the log (§7.5.1/§2.11 no-telemetry). This is the explicit test of the P2.89
    // zero-egress control — the reason `log_targets` is a pure, coverage-counted function.
    #[test]
    fn log_targets_are_local_only() {
        let targets = log_targets();

        // exactly one persistence target: the OS log directory (the §7.5.2 primary rotating record).
        let log_dirs = targets
            .iter()
            .filter(|t| matches!(t, TargetKind::LogDir { .. }))
            .count();
        assert_eq!(
            log_dirs, 1,
            "§7.5.2: the on-disk LogDir must be the single persistence target"
        );

        // every target is on the local-only whitelist — no off-machine / arbitrary sink is EVER present.
        for t in &targets {
            assert!(
                matches!(t, TargetKind::LogDir { .. } | TargetKind::Stderr),
                "§7.5.1/§2.11: only LogDir (persist) + dev Stderr are allowed — never Webview/Dispatch/Folder/Stdout (no network sink)"
            );
        }

        // stderr is a DEV-ONLY console aid — a release build writes the on-disk file exclusively.
        let has_stderr = targets.iter().any(|t| matches!(t, TargetKind::Stderr));
        #[cfg(debug_assertions)]
        assert!(has_stderr, "§7.5.2: dev builds add stderr");
        #[cfg(not(debug_assertions))]
        assert!(
            !has_stderr,
            "§7.5.2: release builds must not write to stderr"
        );
    }

    // The `log_plugin()` glue is NOT AppHandle-coupled (no P2.135 G28 exemption), so its lines count in the
    // diff floor and must be EXECUTED. `.build()` only constructs the plugin descriptor (the global logger is
    // installed by the plugin's `setup` hook at app init, not by this call), so building it here installs no
    // logger and is side-effect-free — safe to call outside `main()`.
    #[test]
    fn log_plugin_builds() {
        let _plugin = log_plugin();
    }

    // §6.4.1 unit (G15): the §7.5.3 `--verbose` launch-switch predicate (P2.94) — verbose iff the EXACT
    // `--verbose` token is in argv; a bare filename, another flag, an empty argv, or a near-miss spelling
    // (`-v` / `--Verbose`) is NOT verbose (one canonical flag). This is the launch-flag half of the verbose
    // opt-in; the pref half is the §7.4.2-tested `verboseLog`.
    #[test]
    fn argv_has_verbose_matches_only_the_exact_flag() {
        assert!(argv_has_verbose(&[
            "convertia".to_string(),
            "--verbose".to_string(),
            "photo.png".to_string(),
        ]));
        assert!(!argv_has_verbose(&[
            "convertia".to_string(),
            "photo.png".to_string()
        ]));
        assert!(!argv_has_verbose(&[
            "convertia".to_string(),
            "-v".to_string()
        ]));
        assert!(!argv_has_verbose(&[
            "convertia".to_string(),
            "--Verbose".to_string()
        ]));
        let empty: [String; 0] = [];
        assert!(!argv_has_verbose(&empty));
    }

    // §6.4.1 unit (G15): boot-stage signature pin (test-strategy §1.1a) — `resolve_log_verbosity` is
    // `AppHandle`-coupled (it reads `prefs::load` + `args_os` and sets `log::max_level`; no `tauri::test`
    // mock harness by decision), so it is verified by its fn-pointer SIGNATURE here + the §1.6 E2E run, not
    // cargo-test execution — G28 exempts its body from the diff floor by this same `&AppHandle` signature.
    #[test]
    fn resolve_log_verbosity_has_its_boot_glue_signature() {
        let _pinned: fn(&tauri::AppHandle) = resolve_log_verbosity;
    }

    // §6.4.1 unit (G15): the §7.5.3 verbose GATEWAY is wired end to end (P2.94). `log_plugin` grants this
    // crate a `Debug` `level_for` ceiling (scoped so dependency `debug!` never reaches the file), and main()'s
    // setup CALLS `resolve_log_verbosity`, which raises the runtime `log::max_level` via `set_max_level`. Both
    // the ceiling and the setup call are AppHandle-adjacent boot glue not executable under cargo-test, so
    // source-scan-pinned (needles `concat!`-assembled to avoid self-match). A regression dropping the ceiling
    // or the setup call would silently make verbose mode a no-op — this catches it.
    #[test]
    fn verbose_gateway_is_wired() {
        let boot = crate::boot_invariants::production_boot_source();
        // Full call-site needles (NOT bare tokens): the short `level_for` / `set_max_level(` tokens ALSO
        // appear in the nearby doc/inline COMMENTS in this source range, so a partial revert that dropped the
        // real call but kept its comment would leave a bare-token scan falsely green — the exact regression
        // this guard must fail on. These strings occur ONLY at the real call sites; `concat!`-split to avoid
        // self-match against this test's own source.
        assert!(
            boot.contains(concat!(".level_", "for(module_path!(), LevelFilter::Debug)")),
            "§7.5.3: log_plugin must grant this crate a Debug level_for ceiling (the verbose gateway)"
        );
        assert!(
            boot.contains(concat!("set_max_", "level(if verbose")),
            "§7.5.3: resolve_log_verbosity must raise the runtime log max level from the verbose flag"
        );
        assert!(
            crate::boot_invariants::production_main_body()
                .contains(concat!("resolve_log_", "verbosity(app.handle())")),
            "§7.5.3: main()'s setup must call resolve_log_verbosity (read-once-at-startup)"
        );
        // P2.137: the opt-in DECISION is the pure `verbose_opt_in` rule (truth-table-tested below), and the
        // glue must actually derive `verbose` through it — a regression re-inlining a (possibly wrong)
        // composition drops this call form.
        assert!(
            boot.contains(concat!(
                "verbose_opt_",
                "in(crate::prefs::load(app).verbose_log, argv_has_verbose(&argv))"
            )),
            "§7.5.3: resolve_log_verbosity must derive the flag via the pure verbose_opt_in rule (P2.137)"
        );
    }

    // §6.4.1 unit (G15, P2.137): the §7.5.3 verbose opt-in DECISION — the full truth table of the pure
    // `verbose_opt_in` rule (either source alone suffices; only both-off stays default). Extracted from the
    // G28-exempt boot glue precisely so this OR is EXECUTED, not needle-only: an `||`→`&&` regression (which
    // silently breaks the §5.9 About toggle's next-launch effect and the §7.5.3 reproduction path) reds the
    // second and third rows.
    #[test]
    fn verbose_opt_in_truth_table() {
        assert!(
            !verbose_opt_in(false, false),
            "§7.5.3: both off → default level"
        );
        assert!(
            verbose_opt_in(true, false),
            "§7.5.3: the persisted §5.9 toggle alone enables verbose"
        );
        assert!(
            verbose_opt_in(false, true),
            "§7.5.3: the --verbose launch switch alone enables verbose"
        );
        assert!(verbose_opt_in(true, true), "§7.5.3: both on is verbose");
    }

    // §6.4.1 unit (G15, P2.137): the §7.5.1/§7.5.2 log-plugin CONFIG CHAIN is wired — the pure ingredients
    // (`log_targets`, `LOG_MAX_FILE_SIZE_BYTES`) are value-tested elsewhere, but nothing forced the builder
    // to CONSUME them: dropping `.targets(...)` reinstates the plugin's default Stdout sink, `.level(Info)`→
    // `Debug` lets third-party debug records (which can carry full user paths) reach the file at verbose,
    // and a dropped cap/rotation silently unwires the §7.5.2 disk bound — all previously green. Full
    // call-site needles over the boot source (the `verbose_gateway_is_wired` discipline).
    #[test]
    fn log_plugin_config_chain_is_wired() {
        let boot = crate::boot_invariants::production_boot_source();
        for needle in [
            concat!(".clear_", "targets()"), // drop the plugin default target set first (§7.5.1)
            concat!(".targets(log_", "targets("), // …then install exactly the local-only set (§7.5.1)
            concat!(".level(LevelFilter::", "Info)"), // the GLOBAL Info ceiling (third-party debug! stays out, §7.5.3)
            concat!(".max_file_size(LOG_MAX_FILE_SIZE_", "BYTES)"), // the §7.5.2 size cap is CONSUMED, not just defined
            concat!(".rotation_strategy(RotationStrategy::", "KeepOne)"), // the §7.5.2 ~1x disk bound
        ] {
            assert!(
                boot.contains(needle),
                "§7.5.1/§7.5.2: log_plugin's builder chain must consume the pinned config (missing `{needle}`)"
            );
        }
    }
}

#[cfg(test)]
mod log_redaction_gate {
    //! §6.4.2 (G31/G15): the §7.5.3 log-redaction PROPERTY GATE (P2.127) — the *behavioural* proof
    //! `crate::log_redact` names ("a secret-looking path stem is absent from the log output ... the separate
    //! P2.127 property gate"), and the P2 activation target of the P0.5.9 §7.5 log-redaction home
    //! (test-strategy §6 / §1.3: "a known secret-looking path stem fed through the logger is absent from the
    //! log — no file contents, no full paths").
    //!
    //! **Fed through the real `log` facade.** The redaction is applied at the CALL SITE — a user file path
    //! routes through [`RedactedPath`] (basename only) at the default `info!`/`warn!` level, and full paths
    //! surface only at the verbose `debug!` level via `p.display()` (§7.5.3 / §7.5.4). The configured
    //! `tauri-plugin-log` logger (`log_plugin`) applies NO format-level redaction — it is a verified
    //! pass-through — so the emitted record's content IS what the call site rendered. This gate drives the
    //! SAME `log` facade the app uses (`tauri_plugin_log::log`) through a capturing sink + the SAME runtime
    //! level gate `resolve_log_verbosity` flips (`log::set_max_level`), so it exercises the actual
    //! default(`info`/`warn`)-vs-verbose(`debug`) filter + the `RedactedPath` door + the `p.display` verbose
    //! form end to end. The plugin's own on-disk file target is `AppHandle`-coupled (no `tauri::test` mock by
    //! decision, §1.1a) — its real file output is the §6.6 walkthrough / §1.6 E2E; this proves the redaction
    //! the configured logger's call sites produce, which is the whole of the record content it writes.
    //!
    //! [Build-Session-Entscheidung: P2.127] Homed beside `log_config` (the §7.5 logging test home); the log
    //! facade is driven through a single test-scoped capturing `log::Log` (no other test in this crate
    //! installs a logger, so the one install Ok's and the buffer captures only this gate's records); the
    //! `RedactedPath` breadth is a pinned-seed 512-case proptest (the P2.14 / P2.126 G16 runner).
    //!
    //! [Derived-Assumption: P2.127 — WHAT THIS GATE PROVES, PRECISELY. The DEFAULT `info!`/`warn!` sites render
    //! a user path through `RedactedPath` (basename), and that door has NO verbose branch, so a default-level
    //! site stays basename-only at EVERY runtime level — PROVEN here: flipping verbose never makes a default
    //! site leak the stem. VERBOSE adds separate `debug!` `p.display` sites that disclose full paths (§7.5.4) —
    //! the opted-in reproducibility trade — PROVEN here against a diagnostic scratch path. Derived from §7.5.3
    //! (privacy by default, full paths on opt-in) + `crate::log_redact` (the renderer split), not picked
    //! arbitrarily.
    //! **OPEN — NOT closed by this gate, flagged for P4:** §7.5.4 verbose ALSO logs the exact engine argv,
    //! which on Win/Linux carries the user's RESOLVED SOURCE path (staging the source into scratch before the
    //! engine sees it is only the macOS T11 case, §3.5.0/§7.2.6). So once the P4 §7.5.4 argv-diagnostic sites
    //! land, a secret-shaped DIRECTORY component of a user source path COULD surface at verbose unless that P4
    //! site redacts the argv paths (or relies on staging). This gate does NOT prove that away — P4's argv-log
    //! site owns it. And the box's "a secret is never logged at any level" is the SEPARATE structural fact that
    //! ConvertIA has no credential-VALUE log site at all — an absence of a code path, not a value this
    //! path-redaction gate can feed through the logger and assert on; it is therefore out of this gate's
    //! behavioural scope (§6.7.3 offline-egress + the absence of any credential-logging site carry it).]

    use crate::log_redact::RedactedPath;
    use proptest::prelude::*;
    use proptest::test_runner::{RngAlgorithm, TestRng, TestRunner};
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use tauri_plugin_log::log::{self, LevelFilter};

    /// A test-scoped capturing `log::Log`: records each emitted record's rendered `[LEVEL] message`, so the
    /// gate can read exactly what the configured logger's call sites write. This is the ONE logger installed in
    /// this crate's test binary — no production log site runs under `cargo test` (§1.1a boot glue) and no other
    /// test installs a logger; a future `main.rs` test that must assert on log output has to REUSE this fixture
    /// rather than install a second logger (there is one `log::Log` per process).
    struct CaptureLogger {
        lines: Mutex<Vec<String>>,
    }

    static CAPTURE: CaptureLogger = CaptureLogger {
        lines: Mutex::new(Vec::new()),
    };

    impl log::Log for CaptureLogger {
        fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
            // The macro's `max_level` gate already filtered by level before this is reached; accept the rest
            // so the capture reflects exactly what passed the runtime level filter.
            true
        }
        fn log(&self, record: &log::Record<'_>) {
            if let Ok(mut lines) = self.lines.lock() {
                lines.push(format!("[{}] {}", record.level(), record.args()));
            }
        }
        fn flush(&self) {}
    }

    /// Install the capturing logger (idempotent — one `log::Log` per process; the first call Ok's).
    fn install_capture() {
        let _ = log::set_logger(&CAPTURE);
    }
    /// Clear the capture buffer before a capture window.
    fn reset() {
        CAPTURE.lines.lock().expect("capture buffer lock").clear();
    }
    /// Read the captured records as one string.
    fn dump() -> String {
        CAPTURE
            .lines
            .lock()
            .expect("capture buffer lock")
            .join("\n")
    }

    /// A minisign-secret-key-SHAPED directory stem, assembled at RUNTIME from harmless word fragments so no
    /// gitleaks-matching literal (G2) sits in this source (the boot-scan `concat!`-avoids-self-match idiom).
    /// Synthetic, never a real key: `RW` + a ≥180-char base64-charset body, so it matches the `.gitleaks.toml`
    /// minisign shape (`RW[A-Za-z0-9+/]{180,}`) as a RUNTIME value (the source holds only the short fragments),
    /// standing in for a long, opaque, sensitive-looking path component the §7.5.3 default door must strip.
    fn secret_shaped_stem() -> String {
        // 6x a 36-char letters-only fragment (⊂ the base64 charset) = 216 chars after `RW` (≥180); the source
        // literals `"RW"` and the fragment never adjoin, so no minisign-shaped literal exists to trip G2.
        format!("RW{}", "SyntheticBase64ShapedBodyNotARealKey".repeat(6))
    }

    // §6.4.2 (G31/G15): the core §7.5.3 behavioural proof, fed through the real `log` facade. A user file
    // path carrying a secret-shaped directory stem is logged through the DEFAULT door (`RedactedPath`) and a
    // ConvertIA diagnostic path through the VERBOSE `debug!` form; the runtime level gate is the real
    // `log::set_max_level` (what `resolve_log_verbosity` sets). All level transitions live in this ONE
    // function so the global level + shared buffer are mutated sequentially, never raced.
    #[test]
    fn secret_looking_path_stem_is_absent_when_fed_through_the_logger() {
        install_capture();

        let secret = secret_shaped_stem();
        // A USER file path whose directory carries the secret-shaped stem — only its basename may ever surface.
        let user_path = Path::new("home")
            .join("alice")
            .join(&secret)
            .join("vacation.jpg");
        // A ConvertIA DIAGNOSTIC path (§7.5.4 scratch/temp) — its full path is the verbose reproducibility
        // disclosure; it carries no user secret.
        let scratch_path = Path::new("tmp")
            .join("convertia")
            .join("run-abcdef")
            .join("job-0.part");

        // ── DEFAULT level (info/warn), verbose OFF ──────────────────────────────────────────────────────
        reset();
        log::set_max_level(LevelFilter::Info);
        // a default-level user-path log site routes through the basename-only door
        log::warn!("could not write output {}", RedactedPath::new(&user_path));
        // a verbose §7.5.4 diagnostic site — MUST be filtered out at the default level
        log::debug!("scratch path {}", scratch_path.display());
        let default_out = dump();

        assert!(
            default_out.contains("vacation.jpg"),
            "§7.5.3: the basename is logged at the default level"
        );
        assert!(
            !default_out.contains(&secret),
            "§7.5.3/§2.11: the secret-shaped path stem is ABSENT from the default-level log output"
        );
        assert!(
            !default_out.contains("alice"),
            "§7.5.3: no directory component (no full path) surfaces at the default level"
        );
        assert!(
            !default_out.contains("run-abcdef"),
            "§7.5.3: a verbose debug! diagnostic site is filtered out at the default level"
        );

        // ── VERBOSE level (debug): the disclosed §7.5.3 reproducibility trade ────────────────────────────
        reset();
        log::set_max_level(LevelFilter::Debug);
        // the verbose diagnostic site now fires and DISCLOSES the full diagnostic path (the §7.5.3 trade)
        log::debug!("scratch path {}", scratch_path.display());
        // the default door is level-independent — a user path via RedactedPath is STILL basename-only
        log::warn!("could not write output {}", RedactedPath::new(&user_path));
        let verbose_out = dump();

        assert!(
            verbose_out.contains("vacation.jpg"),
            "verbose still logs the basename via the default door"
        );
        assert!(
            verbose_out.contains("run-abcdef"),
            "§7.5.3/§7.5.4: verbose DISCLOSES the full diagnostic path (the reproducibility trade)"
        );
        assert!(
            !verbose_out.contains(&secret),
            "§7.5.3: the RedactedPath default door is level-INDEPENDENT — a default-level info!/warn! site \
             stays basename-only even when verbose raises the runtime level, so flipping verbose never makes \
             the DEFAULT sites leak the stem (verbose's disclosure is the separate debug! p.display sites)"
        );

        // leave the global level at the benign default so no subsequent reader inherits verbose
        log::set_max_level(LevelFilter::Info);
    }

    /// The §0.6-invariant property-test case-count floor (test-strategy §1.3 / G16: above proptest's 256
    /// default; the P2.14 / P2.126 floor). [Build-Session-Entscheidung: P2.127]
    const P2_127_CASES: u32 = 512;

    /// A PINNED-SEED proptest runner (test-strategy §1.3 / G16) — replicates the P2.14 runner so the 512-case
    /// exploration is identical every run and a counterexample is reproducible, never retried-to-pass (§7).
    /// [Build-Session-Entscheidung: P2.127]
    fn pinned_runner() -> TestRunner {
        TestRunner::new_with_rng(
            ProptestConfig::with_cases(P2_127_CASES),
            TestRng::deterministic_rng(RngAlgorithm::ChaCha),
        )
    }

    // §6.4.2 (G31/G15): the PROPERTY-gate breadth — over an arbitrary user FILE path (its directory chain
    // includes arbitrary long, opaque, secret-shaped components), the §7.5.3 default door renders EXACTLY
    // the basename, so no DIRECTORY-CHAIN stem (secret-shaped or not) can survive it. The proven domain is
    // file-leaf inputs — the door's call-site contract (§7.5.3); a directory-LEAF input renders its own
    // final component (the documented single-component disclosure, pinned by the sibling property below —
    // P2.137 closed the former dir-leaf domain hole in this gate's claim). The wide-input complement to the
    // fixed facade-driven test above and the example-based `crate::log_redact` unit tests.
    #[test]
    fn the_default_door_renders_exactly_the_basename_for_any_directory_chain() {
        pinned_runner()
            .run(
                &(
                    // directory components: 1..6 long alnum tokens (covers secret-shaped opaque stems)
                    prop::collection::vec("[A-Za-z0-9_-]{1,40}", 1..6),
                    // a `name.ext` basename (a real final component, so file_name() is Some)
                    "[A-Za-z0-9_-]{1,24}\\.[a-z]{1,5}",
                ),
                |(dirs, basename)| {
                    let mut path = PathBuf::new();
                    for d in &dirs {
                        path.push(d);
                    }
                    path.push(&basename);
                    // EXACT basename: if the rendered form equals only the basename, no directory content
                    // (secret-shaped or otherwise) can have leaked — the strongest form of "no full path".
                    prop_assert_eq!(
                        RedactedPath::new(&path).to_string(),
                        basename.clone(),
                        "§7.5.3: the default door renders exactly the basename, never a directory stem"
                    );
                    Ok(())
                },
            )
            .unwrap();
    }

    // §6.4.2 (G31/G15, P2.137): the DIRECTORY-LEAF domain the file-leaf property above cannot reach — for a
    // trailing-separator / directory path, `Path::file_name` yields the LAST directory component, so the
    // door renders AT MOST that one component and NEVER an earlier chain element (the pinned
    // single-component-disclosure contract, decided + documented in `crate::log_redact` — call sites pass
    // FILE paths per §7.5.3; a stray directory input discloses one leaf name, not the chain). The former
    // gate claim silently excluded this input class from its generative space.
    #[test]
    fn the_default_door_renders_at_most_the_final_component_for_directory_leaf_inputs() {
        pinned_runner()
            .run(
                &prop::collection::vec("[A-Za-z0-9_-]{1,40}", 2..6),
                |dirs| {
                    // Build a trailing-separator directory path (`a/b/c/`) — the dir-leaf input class.
                    let joined = format!("{}/", dirs.join("/"));
                    let rendered = RedactedPath::new(Path::new(&joined)).to_string();
                    let leaf = dirs.last().expect("2..6 components").clone();
                    // EXACT leaf equality IS the whole property: the rendered form being precisely the
                    // final component proves no earlier chain element survived (a substring check would
                    // false-positive when an earlier component is a substring of the leaf, e.g. `-` ⊂ `-a`).
                    prop_assert_eq!(
                        &rendered,
                        &leaf,
                        "§7.5.3: a directory-leaf input renders exactly its final component, never the chain"
                    );
                    Ok(())
                },
            )
            .expect("§7.5.3: the dir-leaf property must hold (pinned seed, 512 cases)");
    }
}

#[cfg(test)]
mod startup_spine {
    //! §7.2.1 ordered startup sequence — the app-shell spine (P2.106). Steps 3–5 are readiness SLOTs
    //! (`Result<(), AppFault>`, `Ok` now — bodies P3/P4), step 6 reveals the config-declared window ONLY after
    //! 3–5 succeed, step 7 feeds the launch intake, step 8 hands to the §5.2 Idle UI. AppHandle-coupled
    //! boot-glue: this crate ships no `tauri::test` mock harness (the boot-stage pattern, test-strategy §1.1a),
    //! so the spine is pinned by SIGNATURE coercion (a drift fails to compile) + a SOURCE SCAN of the
    //! production `setup` ORDER (the runtime is the §1.6 launch-with-files E2E + the §6.4.6 window-shown leg).
    //! [Build-Session-Entscheidung: P2.106]
    use tauri::AppHandle;

    use crate::outcome::{AppFault, ConversionErrorKind};

    // §6.4.1 unit (G15): the §7.2.1 step 3–6 spine fns exist with their spec'd signatures — the three readiness
    // SLOTs + the gate return `Result<(), AppFault>` (a step fault is app-level, §2.13), `reveal_main_window`
    // takes the `&AppHandle` (step 6), `present_startup_fault` takes the `&AppHandle` + the `AppFault` it
    // presents (§2.13.3). A signature drift fails to compile — the boot-stage signature pin (these fns are not
    // `tauri::test`-executed). [Build-Session-Entscheidung: P2.106]
    #[test]
    fn startup_spine_fns_have_their_spec_signatures() {
        let _verify: fn(&AppHandle) -> Result<(), AppFault> = super::verify_engine_presence;
        let _perm: fn(&AppHandle) -> Result<(), AppFault> = super::ensure_engine_permissions;
        let _scratch: fn(&AppHandle) -> Result<(), AppFault> = super::prepare_scratch_and_log;
        let _gate: fn(&AppHandle) -> Result<(), AppFault> = super::readiness_checks;
        let _reveal: fn(&AppHandle) = super::reveal_main_window;
        let _present: fn(&AppHandle, AppFault) = super::present_startup_fault;
    }

    // §6.4.1 unit (G15): the §7.2.1 readiness gate chains steps 3 → 4 → 5 in order (short-circuiting on the
    // first `AppFault` via `?`). Scans `all_production_source()` (prefix + main body, every `cfg(test)` module
    // excluded so these needles can never self-match). Needles `concat!`-assembled (self-match avoidance).
    #[test]
    fn readiness_gate_chains_steps_3_4_5_in_order() {
        let src = crate::boot_invariants::all_production_source();
        let step3 = src
            .find(concat!("verify_engine_", "presence(app)?"))
            .expect("§7.2.1: the readiness gate must run step 3 (engine presence + integrity)");
        let step4 = src
            .find(concat!("ensure_engine_", "permissions(app)?"))
            .expect("§7.2.1: the readiness gate must run step 4 (executable-permission setup)");
        let step5 = src
            .find(concat!("prepare_scratch_and_", "log(app)?"))
            .expect(
            "§7.2.1: the readiness gate must run step 5 (scratch + log creation + orphan reclaim)",
        );
        assert!(
            step3 < step4 && step4 < step5,
            "§7.2.1: the readiness gate must chain steps 3 → 4 → 5 in that order"
        );
    }

    // §6.4.1 unit (G15): the §7.2.1 window-reveal ordering — `setup` reveals the window (step 6) and feeds the
    // launch intake (step 7) ONLY on the `Ok` arm of the readiness gate (steps 3–5), so the window is "shown
    // only after 3–5 succeed" (§7.2.1); a readiness fault routes to `present_startup_fault` (§2.13.3) with the
    // window still hidden. Source-scan over `all_production_source()` (AppHandle-coupled boot-glue). Needles
    // `concat!`-assembled. The `.find` offsets all land in `main()`'s body (the fn DEFS carry `(app:
    // &AppHandle)`, only the setup CALL sites carry `(app.handle())`), so the ordering is over the setup calls.
    #[test]
    fn window_revealed_only_after_readiness_then_intake_fed() {
        let src = crate::boot_invariants::all_production_source();
        let gate = src
            .find(concat!("match readiness_", "checks(app.handle())"))
            .expect("§7.2.1: setup must gate on the readiness checks (steps 3–5)");
        let reveal = src
            .find(concat!("reveal_main_", "window(app.handle())"))
            .expect("§7.2.1 step 6: setup must reveal the window after readiness");
        let intake = src
            .find(concat!("forward_first_launch_", "argv(app.handle())"))
            .expect("§7.2.1 step 7: setup must feed the launch intake after the window reveal");
        let present = src
            .find(concat!("present_startup_", "fault(app.handle(), fault)"))
            .expect("§2.13.3: a readiness fault must route to present_startup_fault");
        assert!(
            gate < reveal && reveal < intake,
            "§7.2.1: the window reveal (step 6) + intake feed (step 7) must come AFTER the readiness gate, in order"
        );
        assert!(
            gate < present,
            "§2.13.3: the fault-presentation arm is part of the readiness match (window stays hidden on a fault)"
        );
    }

    // §6.4.1 unit (G15, P2.137): §7.2.1 step 1 — the single-instance guard is "registered FIRST": its
    // `.plugin(` registration is the FIRST plugin registration in `main()`'s Builder chain (a registration
    // behind another plugin would open a window in which a second launch's argv is not forwarded). The old
    // pins covered only steps 3–7; steps 1–2 were order-asserted nowhere.
    #[test]
    fn single_instance_plugin_is_registered_first() {
        let main_body = crate::boot_invariants::production_main_body();
        let first_plugin = main_body
            .find(".plugin(")
            .expect("§7.2.1 step 1: main() registers plugins");
        let single_instance = main_body
            .find(concat!(".plugin(tauri_plugin_single_", "instance::init("))
            .expect("§7.2.1 step 1: main() registers the single-instance plugin");
        assert_eq!(
            first_plugin, single_instance,
            "§7.2.1 step 1: the single-instance guard must be the FIRST plugin registration"
        );
    }

    // §6.4.1 unit (G15, P2.137): §7.2.1 step 2 — the InstanceId mint + StartupContext resolution happen
    // BEFORE the step 3–5 readiness gate (their consumers resolve them as managed State; a hoisted gate
    // faults at boot the moment the P4-filled `prepare_scratch_and_log` body reads them, §7.2.5/§7.1.2).
    #[test]
    fn instance_identity_and_startup_context_precede_the_readiness_gate() {
        let main_body = crate::boot_invariants::production_main_body();
        let mint = main_body
            .find(concat!("app.manage(InstanceId::", "mint())"))
            .expect("§7.2.1 step 2: setup must mint + manage the InstanceId");
        let ctx = main_body
            .find(concat!("let startup = Startup", "Context {"))
            .expect("§7.2.1 step 2: setup must resolve the StartupContext");
        let gate = main_body
            .find(concat!("match readiness_", "checks(app.handle())"))
            .expect("§7.2.1: setup must gate on the readiness checks");
        assert!(
            mint < gate && ctx < gate,
            "§7.2.1: step 2 (identity + context) must precede the step 3–5 readiness gate"
        );
    }

    // §6.4.1 unit (G15, P2.137): §7.2.1 step 6/7 EXCLUSIVITY — the window reveal and the intake feed each
    // have exactly ONE call site (the `Ok` arm of the readiness match). The order pin above proves only
    // FIRST-occurrence order; this cardinality pin excludes a second reveal on the `Err` arm / after the
    // match (the "half-broken UI" §7.2.1 anti-pattern: a window revealed on a readiness fault). The P4
    // fault-presentation body may legitimately add a fault-path window show — that change updates this pin
    // with its own [Test-Change] rationale, which is exactly the review moment the pin forces.
    #[test]
    fn reveal_and_intake_feed_have_exactly_one_call_site_each() {
        let src = crate::boot_invariants::all_production_source();
        assert_eq!(
            src.matches(concat!("reveal_main_", "window(app.handle())"))
                .count(),
            1,
            "§7.2.1 step 6: exactly one reveal call site — the readiness Ok arm (never the fault path)"
        );
        assert_eq!(
            src.matches(concat!("forward_first_launch_", "argv(app.handle())"))
                .count(),
            1,
            "§7.2.1 step 7: exactly one intake-feed call site — the readiness Ok arm"
        );
    }

    // §6.4.1 unit (G15): §7.2.1 step 6 — `reveal_main_window` SHOWS the config-declared `main` window (never a
    // programmatic builder; `no_programmatic_window_builder` in `window_model` guards the negative, and
    // `main_window_declared_hidden_until_readiness` pins the `visible: false` config). Pins that its body
    // resolves the `main` window and calls `.show()`. Needles `concat!`-assembled (self-match avoidance).
    #[test]
    fn reveal_main_window_shows_the_config_declared_window() {
        let src = crate::boot_invariants::all_production_source();
        for needle in [
            concat!("get_webview_", "window(\"main\")"), // resolves the config-declared window (§7.3.1)
            concat!(".show", "()"), // shows it — step 6 reveal, unique to reveal_main_window
        ] {
            assert!(
                src.contains(needle),
                "§7.2.1 step 6: reveal_main_window must show the config-declared `main` window (missing `{needle}`)"
            );
        }
    }

    // §6.4.1 unit (G15): §7.2.1 step 6 / §0.3.1 — the WebView-init fault constructor builds the §2.13 app-level
    // `WebviewFault` with a calm, trace-free message (§2.13.3). PURE (no `AppHandle`), so — unlike the
    // AppHandle-coupled boot glue — it IS executed here and its lines count in the G28 diff floor. A WebView-init
    // FAILURE itself cannot be forced under `cargo test` (no Tauri runtime — the §1.6 E2E / §6.6 walkthrough owns
    // that); this pins the fault VALUE the step-6 `None` arm routes to `present_startup_fault`.
    // [Build-Session-Entscheidung: P2.109]
    #[test]
    fn webview_init_fault_is_a_trace_free_webviewfault() {
        let fault = super::webview_init_fault();
        assert_eq!(
            fault.kind,
            ConversionErrorKind::WebviewFault,
            "§2.13/§7.2.1 step 6: a WebView-init failure is the app-level WebviewFault"
        );
        assert!(
            !fault.message.is_empty(),
            "§2.13.3: the app-level fault carries a calm, plain user message"
        );
        // §2.13.3 trace-free: a calm line, never a stack trace / panic dump (SSOT *no stack traces*). Needles
        // `concat!`-assembled so scanning THIS assertion's source can never self-match.
        assert!(
            !fault.message.contains("panic") && !fault.message.contains(concat!("thread", " '")),
            "§2.13.3: the app-level fault message is trace-free"
        );
    }

    // §6.4.1 unit (G15): §7.2.1 step 6 / §0.3.1 — `reveal_main_window`'s `None` arm (no `main` WebView: a
    // missing/old WKWebView / WebKitGTK) routes the app-level `WebviewFault` to `present_startup_fault`. The
    // AppHandle-coupled reveal is not `tauri::test`-executed (the boot-stage pattern §1.1a), so this source-scan
    // pins the wiring: the `None` arm constructs the fault (`webview_init_fault()`) and hands it to
    // `present_startup_fault`. Needle `concat!`-assembled (self-match avoidance). [Build-Session-Entscheidung: P2.109]
    #[test]
    fn reveal_routes_missing_webview_to_a_webviewfault() {
        let src = crate::boot_invariants::all_production_source();
        assert!(
            src.contains(concat!(
                "present_startup_",
                "fault(app, webview_init_",
                "fault())"
            )),
            "§7.2.1 step 6 / §2.13: reveal_main_window's None arm must route a WebviewFault to present_startup_fault"
        );
    }
}
