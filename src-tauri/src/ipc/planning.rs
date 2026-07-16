//! `crate::ipc::planning` — the §0.4.1 pre-convert planning command group (C2b / C3 / C4 / C5): the
//! target offer, the "will save to…" output plan, the destination picker, and the destination change +
//! re-validation (the §5.2 state-4 flow). P2.21 registers these as the §0.4.1 command-surface interface
//! shells; each command's full request/response contract + its `crate::orchestrator` delegation is authored
//! by its named fill-box. Thin by design (§0.7): validate, delegate, map onto the §0.4.3 `IpcError`.

// §0.4 / T10: unchecked arithmetic on an untrusted wire field must be a compile error in every IPC handler
// (the `crate::ipc` arithmetic-overflow deny cascades here; restated at the T10 boundary). The §1.10
// preflight estimates these handlers will carry are exactly the `width*height*bpp`-class arithmetic the
// deny guards. The shells below do no arithmetic; the deny bites the fill-bodies.
#![deny(clippy::arithmetic_side_effects)]

use std::path::PathBuf;

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::domain::{
    CollectedSetId, DestinationChoice, DestinationPicked, InitialDestination, InstanceId,
    OptionValues, ResolvedDestination, TargetId, TargetOffer,
};
use crate::engines::slice_target;
use crate::orchestrator::{
    plan_output_preview, resolve_persisted_destination, CollectedSetRegistry, DestinationRegistry,
    DestinationResolved, EquivKeyComputer, OutputPlanPreview,
};
use crate::outcome::{ConversionErrorKind, IpcError};
use crate::run::{PublishTemp, RerunLedger};

/// **C2b `pick_destination`** (§0.4.1) — the Rust-side `DialogExt` destination-folder picker. P2.24 authored the
/// typed §0.4.1 wire CONTRACT; **P3.80 RE-KEYS the return** to the id form — the
/// `{} -> Result<Option<DestinationPicked>, IpcError>` door — so the generated `bindings.ts` carries the id +
/// display pair, never a path. The picked folder is a *write* destination (never a source), so it can never harm
/// an original or read anything (§0.10 / §2.1 / §0.11 T2); and per the 2026-07-06 core-owned-paths ruling **no FS
/// path transits the WebView in either direction** — the handler mints a `DestinationId`, stores the picked
/// folder in the §0.4.4 `DestinationRegistry`, and returns `DestinationPicked { destination: DestinationId,
/// display: String }` (§2.10.1). The WebView carries the id into C5 `set_destination` (and C6) as
/// `DestinationChoice::ChosenRoot(id)`; the core resolves it back to the real `PathBuf`. `Ok(None)` = the user
/// cancelled — a clean no-op; the held C4/C5 destination is unchanged.
///
/// [Build-Session-Entscheidung: P2.24 → P3.80] **`Result<Option<DestinationPicked>, IpcError>` return — the §0.4
/// universal error-shape rule.** §0.4 "Error shape" is categorical: *every* command returns `Result<T, IpcError>`.
/// The §0.4.1 table's `Option<DestinationPicked>` output column is the SUCCESS type `T`, wrapped in
/// `Result<T, IpcError>` at the handler — exactly as C1's `CollectedSet` column maps to
/// `Result<CollectedSet, IpcError>`. So the three boundary outcomes are: `Ok(Some(picked))` = the user picked a
/// folder (registered, id + display returned); `Ok(None)` = the user cancelled (a clean no-op, the §5.4
/// cancelled-picker result); `Err(IpcError)` = the native dialog subsystem genuinely failed (a folder pick has no
/// *user-facing* failure, but the boundary still honours the universal Result shape rather than panicking across
/// it, §0.4 "No command ever panics across the boundary"). The wire/TS callsite is unchanged (`Result<T, E>`
/// renders as `__TAURI_INVOKE<T>` + a thrown `IpcError`, like C1).
///
/// [Build-Session-Entscheidung: P2.24 → P3.80 → P3.56] **WIRED — the native folder-pick body.** P2.24 authored
/// the wire signature; P3.80 re-keyed the return to `Option<DestinationPicked>` (the id form); P3.56 fills the
/// native `DialogExt` folder-pick BODY the DestinationBar "Change destination" affordance drives (C2b → C5;
/// P3.54 wired the C2a *intake* picker, a distinct path). The handler binds an `AppHandle` (a Tauri-injected
/// arg, NOT part of the §0.4.1 `{}` wire signature) to open the native picker + reach `State<DestinationRegistry>`;
/// it opens the folder dialog on a **dedicated blocking thread** (`spawn_blocking` + `blocking_pick_folder`, never
/// a synchronous `blocking_pick_*` on a Tokio worker, §1.1) so the runtime stays free — mirroring C2a's dialog
/// discipline. A dismissed dialog → `Ok(None)` (a clean no-op, the held C4/C5 destination unchanged, §5.4);
/// otherwise the picked folder is registered via the AppHandle-free `register_picked` (mints a `DestinationId`,
/// stores the root in `State<DestinationRegistry>`, returns `DestinationPicked { destination, display }` — the id
/// + lossy display, **no `PathBuf` on the wire**, §2.10.1). This is AppHandle-coupled boot-glue (§1.1a; G28
/// signature-exempt): the dialog open is source-scan-pinned, the registration is `register_picked` (unit-tested +
/// G27-counted). A native OS folder dialog is **not unit-testable** (it needs a real OS dialog — the §6.6
/// walkthrough + the P9 E2E flow exercise it), so the testable deliverable is `register_picked` + the typed
/// contract. `#[tauri::command]` (no `rename_all`): the wire signature takes no args (the `AppHandle` is injected).
#[tauri::command]
#[specta::specta]
pub async fn pick_destination(app: AppHandle) -> Result<Option<DestinationPicked>, IpcError> {
    // §1.1/§0.4.1 (P3.56): open the native folder picker on a DEDICATED BLOCKING THREAD (spawn_blocking), never a
    // synchronous blocking_pick_* on a Tokio worker — the same async-safety discipline C2a applies to its intake
    // picker + C4 to its FS probe (§1.1 "MUST NOT block a Tokio worker thread"). A spawn_blocking failure (the
    // dialog thread panicked — should-never-happen) surfaces as an InternalError, never a silent no-op.
    let dialog_app = app.clone();
    let picked: Option<FilePath> = tauri::async_runtime::spawn_blocking(move || {
        dialog_app.dialog().file().blocking_pick_folder()
    })
    .await
    .map_err(|_| IpcError {
        kind: ConversionErrorKind::InternalError,
        message: "Could not open the folder picker.".into(),
        path_display: None,
        residue_display: None,
    })?;
    // A dismissed dialog (or a non-path entry, defensively dropped) → the §5.4 clean no-op: nothing registered,
    // the held C4/C5 destination unchanged (`Ok(None)`, the §0.4.1 cancelled-pick result).
    let Some(path) = picked.and_then(|file| file.into_path().ok()) else {
        return Ok(None);
    };
    // §0.4.4 (P3.80 `register`): mint + register the picked root, returning the id + display the WebView carries
    // into C5 as `DestinationChoice::ChosenRoot(destination)` — the real `PathBuf` never crosses the wire (§2.10.1).
    let registry = app.state::<DestinationRegistry>();
    Ok(Some(register_picked(&registry, path)))
}

/// Register a C2b-picked folder into the §0.4.4 [`DestinationRegistry`] and build its [`DestinationPicked`] return
/// (§0.4.1, P3.56) — AppHandle-free so the native-dialog handler's registration half is unit-tested (the §1.1a
/// boot-glue split, mirroring C2a's `resolve_pick_outcome`). Computes the lossy display BEFORE `register` moves
/// the `PathBuf` in; the minted `DestinationId` resolves back to `path` in the registry (the C4/C5/C6
/// `resolve_choice` handle). [Build-Session-Entscheidung: P3.56]
fn register_picked(registry: &DestinationRegistry, path: PathBuf) -> DestinationPicked {
    // §2.10.1: the display-only lossy form for the "will save to …" line — computed before the path is moved into
    // the registry; never re-submittable as an input path (the WebView only ever names the id).
    let display = path.to_string_lossy().into_owned();
    let destination = registry.register(path);
    DestinationPicked {
        destination,
        display,
    }
}

/// **C3 `get_targets`** (§0.4.1) — a pure function of the detected source type to the offered `Vec<Target>` +
/// the one pre-highlighted default + per-target lossy/availability/options model (§1.5/§1.6); no engine spawned.
/// This box (P2.25) authors the typed §0.4.1 wire CONTRACT — `{ collectedSetId } -> Result<TargetOffer,
/// IpcError>` (the §0.4 universal error shape) — so the generated `bindings.ts` carries the C3 door, pulling the
/// whole `TargetOffer` graph (`Target` / `TargetId` / `OptionValues` / …) into the bindings.
///
/// [Build-Session-Entscheidung: P3.49] **WIRED.** The handler binds an `AppHandle` (a Tauri-injected arg, NOT
/// part of the §0.4.1 `{ collectedSetId }` wire signature) to reach the §0.4.4 `State<CollectedSetRegistry>`
/// (`.manage`d in main, P2.44) and dispatches to the AppHandle-free `resolve_targets` helper (the §1.1a
/// boot-glue split, mirroring C8's `resolve_run_summary`, unit-tested + G27-counted). `resolve_targets`
/// resolves the set, reads its detected `format`, and builds the §1.5 `TargetOffer` from the SHARED
/// `engines::slice_target` offer (the ONE source of the CSV↔TSV offer, the P3.48 `needs:` edge — no
/// synthesized `Target`); the single offered target IS the pre-highlighted default. An unresolvable
/// `collectedSetId` (expired / superseded / never registered) returns the §2.13 `Err(InternalError)` catch-all
/// (the §3.2 `PlanError` precedent) — the message is PROVISIONAL (the §2.8 catalog box owns the final
/// command-level wording), the `kind` spelled with the CONCRETE `ConversionErrorKind` not the `ErrorKind`
/// alias (the P2.19 convention).
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn get_targets(
    app: AppHandle,
    collected_set_id: CollectedSetId,
) -> Result<TargetOffer, IpcError> {
    let sets = app.state::<CollectedSetRegistry>();
    resolve_targets(&sets, collected_set_id)
}

/// **C4 `plan_output`** (§0.4.1) — computes the §1.8 output plan (resolved destination directory, per-location
/// divert preview §2.7, §2.5 re-run prompt, §1.10 pre-flight verdict) that drives the "will save to…" line
/// before convert. This box (P2.26) authors the typed §0.4.1 wire CONTRACT — `{ collectedSetId, target,
/// options, destination } -> Result<OutputPlanPreview, IpcError>` (the §0.4 universal error shape) — so the
/// generated `bindings.ts` carries the C4 door, pulling the `OutputPlanPreview` graph (`DivertReason` /
/// `RerunPrompt` / `PreflightVerdict` / …) into the bindings.
///
/// [Build-Session-Entscheidung: P3.49] **WIRED for the walking skeleton.** The handler binds an `AppHandle`
/// (Tauri-injected — the §0.4.1 wire signature stays `{ collectedSetId, target, options, destination }`) to
/// reach the §0.4.4 `State<CollectedSetRegistry>` + `State<DestinationRegistry>` + the §2.5
/// `State<EquivKeyComputer>` / `State<RerunLedger>` + the app `State<InstanceId>`; it resolves the wire
/// `ChosenRoot(DestinationId)` against the picked-roots registry (`resolve_choice`; an unknown id → the §0.4.3
/// refusal, P3.80) and dispatches the resolved `ResolvedDestination` to the AppHandle-free `resolve_output_plan`
/// helper, which resolves the set and delegates the §1.8 batch preview to `orchestrator::plan_output_preview`: the
/// representative "will save to…" directory + its §2.7.2 divert classification (`location_status`), the §2.5
/// PEEK-only re-run verdict (`compute_rerun_verdict`), and the §1.10 preflight verdict. The §1.10 verdict is
/// the **trivial §1.10-seam slice verdict** (the CSV→TSV footprint is negligible ⇒ `up_front_fail: None` by
/// construction); the real §1.10 estimator is P4.72, which SUPERSEDES it behind this same contract — so P3
/// must NOT build a real estimator here (a double-build). An unresolvable `collectedSetId` returns the §2.13
/// `Err(InternalError)` catch-all (provisional message, CONCRETE `ConversionErrorKind` — the P2.19 convention).
/// C4 is re-callable (debounced, §5.8): `resolve` is NON-evicting, so re-planning never consumes the set. The
/// §2.7.2 divert probe is blocking FS I/O, so the handler runs the whole preview on `spawn_blocking` — off the
/// async runtime, like C1's walk / C2a's dialog (§1.1 "MUST NOT block a Tokio worker thread").
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn plan_output(
    app: AppHandle,
    collected_set_id: CollectedSetId,
    target: TargetId,
    options: OptionValues,
    destination: DestinationChoice,
) -> Result<OutputPlanPreview, IpcError> {
    // §1.8/§2.7.2 (P3.49): the §2.7.2 divert classification (`location_status`) writes+removes a probe dotfile
    // and (Unix) `statfs`es the destination — genuine BLOCKING FS syscalls that can stall on a slow/unresponsive
    // destination (a network share, degraded media). So the C4 preview runs on a DEDICATED BLOCKING THREAD
    // (`spawn_blocking`), never a Tokio worker — the same async-safety discipline C1 applies to its walk and C2a
    // to its dialog (§1.1 "MUST NOT block a Tokio worker thread"), keeping the async runtime free for the
    // debounced re-calls (§5.8). `AppHandle` + the owned args move into the closure; State is re-resolved inside
    // (all five stores are `.manage()`d); the P3.80 `resolve_choice` adds a fallible step — an unknown
    // `ChosenRoot(DestinationId)` refuses with the §0.4.3 not-available `Err` before the preview runs. A
    // `JoinError` (the probe thread panicked — should-never-happen under the in-core no-panic policy) surfaces as
    // an InternalError, never a silent value. [Build-Session-Entscheidung: P3.49]
    match tauri::async_runtime::spawn_blocking(move || {
        let sets = app.state::<CollectedSetRegistry>();
        let computer = app.state::<EquivKeyComputer>();
        let ledger = app.state::<RerunLedger>();
        let destinations = app.state::<DestinationRegistry>();
        let instance = *app.state::<InstanceId>();
        // §0.4.4 (P3.80): resolve the wire `ChosenRoot(DestinationId)` against the picked-roots registry to its
        // real `PathBuf` — an unknown id is the §0.4.3 refusal (`not_available`). The pure §1.8 preview then
        // reads a resolved `ResolvedDestination`, never a registry lookup (the 2026-07-06 core-owned-paths split,
        // the C9 `open_path` id-resolution mirror). [Build-Session-Entscheidung: P3.80]
        let Some(resolved) = destinations.resolve_choice(&destination) else {
            return Err(not_available("Could not plan the output."));
        };
        resolve_output_plan(
            &sets,
            &computer,
            &ledger,
            instance,
            collected_set_id,
            target,
            &options,
            &resolved,
        )
    })
    .await
    {
        Ok(result) => result,
        Err(_join) => Err(not_available("Could not plan the output.")),
    }
}

/// **C5 `set_destination`** (§0.4.1) — re-validates writability/divert and re-evaluates the
/// destination-dependent §2.14.4 pre-flight when the user changes the destination, carrying the §2.5 re-run
/// verdict through UNCHANGED (§2.5.1 — the v1 EquivKey has no destination component, so C5 never recomputes
/// `rerun`). This box (P2.27) authors the typed §0.4.1 wire CONTRACT — `{ collectedSetId, target, options,
/// destination } -> Result<DestinationResolved, IpcError>` (the §0.4 universal error shape; the SAME request
/// payload as C4 `plan_output`, the C4/C5 byte-identical-payload pair) — so the generated `bindings.ts` carries
/// the C5 door, pulling the `DestinationResolved` graph into the bindings.
///
/// [Build-Session-Entscheidung: P2.27 → P3.56] **WIRED — the destination-change re-validation body.** P2.27
/// authored the typed contract; P3.56 fills the §1.8/§2.14.4 body the DestinationBar "Change destination" flow
/// drives (C2b `pick_destination` → C5). Same AppHandle-coupled boot-glue pattern as C4 `plan_output` (§1.1a; G28
/// signature-exempt): the handler binds an `AppHandle` (Tauri-injected — the §0.4.1 wire signature stays
/// `{ collectedSetId, target, options, destination }`, the C4/C5 byte-identical payload pair) to reach the
/// §0.4.4 `State<CollectedSetRegistry>` + `State<DestinationRegistry>` + the §2.5 `State<EquivKeyComputer>` /
/// `State<RerunLedger>` + the app `State<InstanceId>`; it resolves the wire `ChosenRoot(DestinationId)` against
/// the picked-roots registry (`resolve_choice`; an unknown id → the §0.4.3 refusal) and dispatches to the
/// AppHandle-free `resolve_destination_change` helper (unit-tested + G27-counted). The re-validation runs on a
/// **dedicated blocking thread** (`spawn_blocking`) — the §2.7.2 `location_status` probe is blocking FS I/O, so
/// the async runtime stays free for the debounced re-calls, exactly as C4 does. `resolve_destination_change`
/// reuses `orchestrator::plan_output_preview` (the ONE §1.8 preview machinery — the refreshed "will save to …"
/// dir, the §2.7.2 divert, the §2.14.4-re-evaluated preflight) and maps it onto the C5 `DestinationResolved`
/// echo. **`rerun` carried through unchanged (§2.5.1):** the v1 §2.5 EquivKey has NO destination component, so the
/// re-run verdict is destination-INDEPENDENT — recomputing it via the PEEK-only `plan_output_preview` yields the
/// identical value C4 held, which is exactly "carried through unchanged" (a fresh peek is idempotent, never a
/// double-record). An unresolvable `collectedSetId` returns the §2.13 `Err(InternalError)` catch-all (provisional
/// message, CONCRETE `ConversionErrorKind` — the P2.19 convention). The C4/C5 lifecycle asymmetry (C4 re-callable;
/// C5 owns the destination; C4 never overrides C5) is the §0.4.1 caller-passed-destination contract (P2.28) — C5
/// echoes the caller's `destination` back, holding no server-side destination store.
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn set_destination(
    app: AppHandle,
    collected_set_id: CollectedSetId,
    target: TargetId,
    options: OptionValues,
    destination: DestinationChoice,
) -> Result<DestinationResolved, IpcError> {
    // §1.8/§2.14.4 (P3.56): re-validate the destination-dependent divert + preflight on a DEDICATED BLOCKING
    // THREAD (`location_status` is blocking FS I/O), the same async-safety discipline C4 applies to its preview
    // (§1.1 "MUST NOT block a Tokio worker thread"), keeping the runtime free for the debounced re-calls (§5.8).
    // `AppHandle` + the owned args move into the closure; State is re-resolved inside. The wire
    // `ChosenRoot(DestinationId)` resolves against the picked-roots registry FIRST — an unknown id refuses with the
    // §0.4.3 not-available `Err` before the preview runs (P3.80 `resolve_choice`). A `JoinError` (the probe thread
    // panicked — should-never-happen under the in-core no-panic policy) surfaces as an InternalError, never a
    // silent value. [Build-Session-Entscheidung: P3.56]
    match tauri::async_runtime::spawn_blocking(move || {
        let sets = app.state::<CollectedSetRegistry>();
        let computer = app.state::<EquivKeyComputer>();
        let ledger = app.state::<RerunLedger>();
        let destinations = app.state::<DestinationRegistry>();
        let instance = *app.state::<InstanceId>();
        let Some(resolved) = destinations.resolve_choice(&destination) else {
            return Err(not_available("Could not update the destination."));
        };
        resolve_destination_change(
            &sets,
            &computer,
            &ledger,
            instance,
            collected_set_id,
            target,
            &options,
            destination,
            &resolved,
        )
    })
    .await
    {
        Ok(result) => result,
        Err(_join) => Err(not_available("Could not update the destination.")),
    }
}

/// **C14 `get_initial_destination`** (§0.4.1, P3.56) — the returning-user DestinationBar initial-state query the
/// frontend's Confirm→Targets advance runs (§5.8:918) BEFORE the first C4 `plan_output`. Resolves the persisted
/// §7.4.1 `lastDestinationMode` CORE-side into a structural [`InitialDestination`] (`BesideSource` / `ChosenRoot` /
/// `Fallback`) the frontend maps onto C4's first `destination` argument — keeping §0.6's 2-variant
/// `DestinationChoice` permanently (no `Last` variant, no C4 mirror-back; the P3.80 hand-off form). The
/// re-validation FALLBACK is distinguished STRUCTURALLY from a plain beside-source pref so the §5.8:926 passive
/// fallback note surfaces even when beside-source is writable (the G1 Opus-P2 adoption).
///
/// [Build-Session-Entscheidung: P3.56] Naming = this box's fill decision (the `get_*` query convention, cf.
/// `get_targets`). AppHandle-coupled boot-glue (§1.1a; G28 signature-exempt): the handler binds an `AppHandle` (a
/// Tauri-injected arg, NOT part of the §0.4.1 `{}` wire signature) to read the prefs store + reach the §0.4.4
/// `State<DestinationRegistry>` + the app `State<InstanceId>` (the §2.6.3 probe name); it runs on a **dedicated
/// blocking thread** (`spawn_blocking`) — `prefs::load` reads `settings.json` and the resolver's §2.7.2
/// `location_status` re-validation is blocking FS I/O — then dispatches to the AppHandle-free
/// `orchestrator::resolve_persisted_destination` (unit-tested + G27-counted). The resolver never fails (the
/// beside-source/fallback IS a value), so the only `Err` is a `JoinError` (the probe thread panicked —
/// should-never-happen under the in-core no-panic policy) → the §2.13 InternalError catch-all, never a silent
/// value. NO path outbound: a re-validated `ChosenPath` is registered in the `DestinationRegistry` and only its
/// id + display cross the wire (§2.10.1), exactly as C2b does.
#[tauri::command]
#[specta::specta]
pub async fn get_initial_destination(app: AppHandle) -> Result<InitialDestination, IpcError> {
    // §5.8:918/§7.4.1 (P3.56): read the persisted `lastDestinationMode` + re-validate it on a DEDICATED BLOCKING
    // THREAD (`prefs::load` opens `settings.json`; the resolver's §2.7.2 `location_status` probe is blocking FS
    // I/O), the same async-safety discipline C4 applies to its preview (§1.1 "MUST NOT block a Tokio worker").
    // `AppHandle` moves into the closure; State is re-resolved inside. A `JoinError` (should-never-happen) surfaces
    // as an InternalError, never a silent value. [Build-Session-Entscheidung: P3.56]
    match tauri::async_runtime::spawn_blocking(move || {
        let prefs = crate::prefs::load(&app);
        let registry = app.state::<DestinationRegistry>();
        let instance = *app.state::<InstanceId>();
        let probe = PublishTemp::probe_name(instance);
        resolve_persisted_destination(&prefs.last_destination_mode, &registry, &probe)
    })
    .await
    {
        Ok(result) => Ok(result),
        Err(_join) => Err(not_available("Could not resolve the saved destination.")),
    }
}

/// The §0.4.3 "collected set not resolvable" `IpcError` (P3.49) — the honest result when a `collectedSetId`
/// does not resolve in the §0.4.4 registry (expired / superseded / never registered). `InternalError` is the
/// §2.13 catch-all (the §3.2 `PlanError` precedent); the §2.8 message catalog owns the FINAL command-level
/// wording, so `message` is a PROVISIONAL neutral English string, `kind` the CONCRETE `ConversionErrorKind`
/// (the P2.19 convention). [Build-Session-Entscheidung: P3.49]
fn not_available(message: &str) -> IpcError {
    IpcError {
        kind: ConversionErrorKind::InternalError,
        message: message.to_owned(),
        path_display: None,
        residue_display: None,
    }
}

/// The C3 `get_targets` resolve LOGIC (§1.5, P3.49) — AppHandle-free so it is unit-tested with a real registry
/// (the §1.1a boot-glue split, mirroring C8's `resolve_run_summary`). Resolve the set (`None` → the §0.4.3
/// not-available `Err`), read its detected `format`, and build the §1.5 `TargetOffer` from the SHARED
/// `engines::slice_target` offer (the ONE source of the CSV↔TSV offer, the P3.48 `needs:` edge — no
/// synthesized `Target`); the single offered target IS the pre-highlighted default. [Build-Session-Entscheidung: P3.49]
fn resolve_targets(
    sets: &CollectedSetRegistry,
    collected_set_id: CollectedSetId,
) -> Result<TargetOffer, IpcError> {
    let Some(set) = sets.resolve(collected_set_id) else {
        return Err(not_available("Could not prepare conversion options."));
    };
    let Some(target) = slice_target(set.frozen.format) else {
        // The registered set's format has no offered target (a non-CSV/TSV format — unreachable while the slice
        // offer is CSV↔TSV; P5–P7 grow the registry). The honest not-available result, kept total (no panic).
        return Err(not_available("Could not prepare conversion options."));
    };
    let default_target = target.id;
    Ok(TargetOffer {
        set: collected_set_id,
        targets: vec![target],
        default_target,
    })
}

/// The C4 `plan_output` resolve LOGIC (§1.8, P3.49) — AppHandle-free so it is unit-tested. Resolve the set
/// (NON-evicting, so C4 stays re-callable/debounced; `None` → the §0.4.3 not-available `Err`) and delegate the
/// §1.8 batch preview to `orchestrator::plan_output_preview`. [Build-Session-Entscheidung: P3.49]
#[allow(clippy::too_many_arguments)] // each arg is a distinct, documented C4 planning input (the C8 State-inject precedent)
fn resolve_output_plan(
    sets: &CollectedSetRegistry,
    computer: &EquivKeyComputer,
    ledger: &RerunLedger,
    instance: InstanceId,
    collected_set_id: CollectedSetId,
    target: TargetId,
    options: &OptionValues,
    destination: &ResolvedDestination,
) -> Result<OutputPlanPreview, IpcError> {
    let Some(set) = sets.resolve(collected_set_id) else {
        return Err(not_available("Could not plan the output."));
    };
    Ok(plan_output_preview(
        &set,
        target,
        options,
        destination,
        instance,
        computer,
        ledger,
    ))
}

/// The C5 `set_destination` resolve LOGIC (§1.8/§2.14.4, P3.56) — AppHandle-free so it is unit-tested, mirroring
/// `resolve_output_plan`. Resolve the set (`None` → the §0.4.3 not-available `Err`), re-run the ONE §1.8 preview
/// machinery (`orchestrator::plan_output_preview` — the refreshed "will save to …" dir, the §2.7.2 divert, the
/// §2.14.4-re-evaluated preflight) for the new `resolved` destination, and map it onto the C5 `DestinationResolved`
/// echo: `destination` is the wire choice echoed back (the caller-passed §0.4.1 destination, P2.28); `rerun` is
/// **carried through unchanged** — the v1 §2.5 EquivKey is destination-INDEPENDENT (§2.5.1), so the PEEK-only
/// recompute yields the identical verdict C4 held (idempotent, never a double-record). [Build-Session-Entscheidung: P3.56]
#[allow(clippy::too_many_arguments)] // each arg is a distinct, documented C5 re-validation input (the C4 resolve_output_plan precedent)
fn resolve_destination_change(
    sets: &CollectedSetRegistry,
    computer: &EquivKeyComputer,
    ledger: &RerunLedger,
    instance: InstanceId,
    collected_set_id: CollectedSetId,
    target: TargetId,
    options: &OptionValues,
    destination: DestinationChoice,
    resolved: &ResolvedDestination,
) -> Result<DestinationResolved, IpcError> {
    let Some(set) = sets.resolve(collected_set_id) else {
        return Err(not_available("Could not update the destination."));
    };
    let preview = plan_output_preview(&set, target, options, resolved, instance, computer, ledger);
    Ok(DestinationResolved {
        // §0.4.1 (P2.28): C5 echoes the caller's destination choice back — no server-side destination store.
        destination,
        final_dir_display: preview.final_dir_display,
        diverted: preview.diverted,
        preflight: preview.preflight,
        // §2.5.1: destination-INDEPENDENT, so the fresh PEEK == the C4-held verdict (carried through unchanged).
        rerun: preview.rerun,
    })
}

#[cfg(test)]
mod support {
    //! Shared §6.4.1 (G15) test support for the C3/C4 resolve tests (P3.49): freeze a real one-CSV drop
    //! through the §1.1 `ingest` funnel and register it — the honest way to seat a resolvable Single set
    //! (test-strategy §0.1: a real FS, no hand-built wire type) — plus the production source scan the
    //! AppHandle-coupled handlers (G28 signature-exempt) are pinned by. [Build-Session-Entscheidung: P3.49]
    use std::path::Path;

    use tauri::ipc::{Channel, InvokeResponseBody};
    use tokio_util::sync::CancellationToken;

    use crate::domain::{CollectedSetId, InstanceId, IntakeOrigin, ScanProgress};
    use crate::orchestrator::{ingest, CollectedSetRegistry};

    /// The production prefix of `planning.rs` — everything before the FIRST `#[cfg(test)]` module — so a
    /// needle declared in a test can never self-match. `concat!`-assembled so the literal `#[cfg(test)]` does
    /// not appear in this test source.
    pub fn production_planning_source() -> &'static str {
        let full = include_str!("planning.rs");
        full.split_once(concat!("#[cfg", "(test)]"))
            .map_or(full, |(prefix, _)| prefix)
    }

    /// A non-ephemeral temp dir under the crate source root — `location_status` classifies an OS-temp dir
    /// `Ephemeral` FIRST (so a plain `tempfile::tempdir()` would falsely divert the C4 preview), so the C4
    /// success path needs a non-ephemeral base (mirroring the fs_guard `location_status_tests` helper). `None`
    /// on the pathological env where the crate root is itself under an OS temp root (a clean skip, never a
    /// false pass). Real FS — never mocked (test-strategy §0.1).
    pub fn non_ephemeral_tempdir() -> Option<tempfile::TempDir> {
        let dir = tempfile::Builder::new()
            .prefix("convertia-planning-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .expect("create a temp dir in the crate source root");
        (!crate::platform::is_ephemeral_output_dir(dir.path())).then_some(dir)
    }

    /// Freeze a real one-CSV drop (in `dir`) through the §1.1 `ingest` funnel and register it, returning its
    /// `CollectedSetId`. A discarding scan Channel + a fresh cancel token — the drain never depends on them.
    pub fn register_one_csv(sets: &CollectedSetRegistry, dir: &Path) -> CollectedSetId {
        let csv = dir.join("data.csv");
        std::fs::write(&csv, b"a,b\n1,2\n").expect("write the CSV source");
        let discard: Channel<ScanProgress> = Channel::new(|_body: InvokeResponseBody| Ok(()));
        let result = ingest(
            vec![csv],
            IntakeOrigin::Drop,
            &CancellationToken::new(),
            &discard,
            InstanceId::mint(),
        );
        let registrable = result
            .registrable
            .expect("a lone CSV freezes a registrable Single");
        let id = registrable.frozen.id;
        sets.register(registrable);
        id
    }
}

#[cfg(test)]
mod c2b_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C2b `pick_destination` — the native folder-pick, WIRED (P3.56). Same
    //! AppHandle-coupled boot-glue pattern as C3/C4 (§1.1a; G28-exempt): the handler binds an `AppHandle` (to open
    //! the native `DialogExt` folder dialog + reach `State<DestinationRegistry>`), so it is NOT cargo-test-invocable,
    //! and the native OS dialog is not unit-testable either (it needs a real dialog — the §6.6 walkthrough + the P9
    //! E2E flow exercise it). Its testable half — the AppHandle-free `register_picked` registration — is unit-tested
    //! here with a real registry (the minted id read BACK to its path), and the handler WIRING (open the dialog +
    //! register via the helper) is source-scan-pinned. The §0.4.1 typed wire surface stays asserted by the
    //! bindings.ts golden (`bindings_codegen` in main.rs). [Build-Session-Entscheidung: P3.56]
    //!
    //! [Test-Change: P3.56 — old-obsolete+new-correct, §0.4.1] the P2.24 `block_on(pick_destination())` contract
    //! test is OBSOLETE — the handler now binds an `AppHandle` (not constructible in a cargo test), and the shell's
    //! unconditional `Ok(None)` is superseded by the real native folder pick. It is REPLACED by the `register_picked`
    //! unit test (a picked path → a `DestinationPicked` whose minted id RESOLVES BACK to the path + the display
    //! form — read back, not "it compiles") + the handler source-scan — the sanctioned boot-glue stratification (the
    //! C3/C4 P3.49 precedent), NOT a dropped assertion.
    use super::support::production_planning_source;
    use super::*;
    use crate::orchestrator::DestinationRegistry;

    // §6.4.1 unit (G15) / §0.4.1: `register_picked` mints a resolvable id + the lossy display for a picked folder —
    // the `DestinationPicked` pair the WebView carries into C5 (`ChosenRoot(destination)`). Read BACK: the minted
    // `DestinationId` resolves to the registered path in a real registry (not "it compiles"), and the display is the
    // path's lossy form (§2.10.1). Real registry, no mock (test-strategy §0.1).
    #[test]
    fn register_picked_mints_a_resolvable_id_and_the_display() {
        let registry = DestinationRegistry::default();
        // A unique real dir via `tempfile` (not `std::env::temp_dir()`, the SAST-flagged shared-temp
        // pattern) — `register_picked` only stores + resolves the path, so the dir is never written to.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("convertia-c2b-picked-root");
        let picked = register_picked(&registry, path.clone());
        assert_eq!(
            picked.display,
            path.to_string_lossy(),
            "§2.10.1: the DestinationPicked display is the picked folder's lossy form (the 'will save to …' line)"
        );
        assert_eq!(
            registry.resolve(picked.destination),
            Some(path),
            "§0.4.4: the minted DestinationId resolves BACK to the registered picked root (the C5/C6 ChosenRoot handle)"
        );
    }

    // §6.4.1 unit (G15): the C2b handler is AppHandle-coupled boot-glue (§1.1a; G28-exempt) — a source-scan pins it
    // binds an `AppHandle`, opens the native folder dialog on a blocking thread, resolves `State<DestinationRegistry>`,
    // and REGISTERS via `register_picked`. Needles `concat!`-assembled (self-match avoidance).
    #[test]
    fn pick_destination_handler_opens_the_folder_dialog_and_registers_via_the_helper() {
        let src = production_planning_source();
        for needle in [
            concat!("pub async fn pick_", "destination("),
            concat!("app: App", "Handle"),
            concat!("spawn_", "blocking(move"),
            concat!("blocking_pick_", "folder()"),
            concat!("state::<Destination", "Registry>()"),
            concat!("register_", "picked(&registry, path)"),
        ] {
            assert!(
                src.contains(needle),
                "§0.4.1/§1.1a: the C2b pick_destination handler must bind an AppHandle, open the folder dialog on a \
                 blocking thread, resolve the DestinationRegistry, and register via register_picked (missing `{needle}`)"
            );
        }
    }
}

#[cfg(test)]
mod c3_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C3 `get_targets` — the §1.5 target offer, WIRED (P3.49). The handler binds
    //! an `AppHandle` to reach `State<CollectedSetRegistry>`, so it is AppHandle-coupled boot-glue (the §1.1a
    //! pattern — NOT cargo-test-invocable; G28 signature-exempt): its resolve LOGIC lives in the AppHandle-free
    //! `resolve_targets` helper, unit-tested here with a real registry + a real freeze; the handler's WIRING
    //! (resolve the State + dispatch via the helper) is source-scan-pinned. The §0.4.1 typed wire surface stays
    //! asserted by the bindings.ts golden (`bindings_codegen` in main.rs). [Build-Session-Entscheidung: P3.49]
    //!
    //! [Test-Change: P3.49 — old-obsolete+new-correct, §1.5] the P2.25 `block_on(get_targets(id))` contract
    //! test is OBSOLETE — the handler now binds an `AppHandle` (not constructible in a cargo test), and the
    //! shell's unconditional `Err(InternalError)` is superseded by the real §1.5 resolve. It is REPLACED by the
    //! `resolve_targets` unit tests (a registered CSV set → the TSV-default `TargetOffer`; an unresolvable id →
    //! the `Err(InternalError)` catch-all — read back, not "it compiles") + the handler source-scan — the
    //! sanctioned boot-glue stratification (the C8 `resolve_run_summary` precedent), NOT a dropped assertion.
    use super::support::{production_planning_source, register_one_csv};
    use super::*;
    use crate::domain::FormatId;

    /// A `CollectedSetId` for the unresolvable-id test — its PUBLIC bare-uuid wire form, mirroring the sibling
    /// contract helpers.
    fn collected_set_id() -> CollectedSetId {
        serde_json::from_str(r#""44444444-4444-4444-8444-444444444444""#)
            .expect("CollectedSetId deserializes from a uuid string")
    }

    // §6.4.1 real-FS (G15) / §1.5: a registered CSV set resolves the CSV↔TSV slice offer — exactly one target,
    // which is also the pre-highlighted default (TSV for a CSV source). Read back from the SHARED
    // `engines::slice_target` offer, no synthesized Target (the P3.48 single-source rule).
    #[test]
    fn resolve_targets_offers_the_tsv_default_for_a_registered_csv_set() {
        let dir = tempfile::tempdir().expect("temp dir");
        let sets = CollectedSetRegistry::default();
        let id = register_one_csv(&sets, dir.path());
        let offer =
            resolve_targets(&sets, id).expect("a registered CSV set resolves a TargetOffer");
        assert_eq!(
            offer.set, id,
            "§1.5: the offer names the resolved collected set"
        );
        assert_eq!(
            offer.default_target,
            TargetId::Format(FormatId::Tsv),
            "§1.5: the CSV slice's single pre-highlighted default is TSV"
        );
        assert_eq!(
            offer.targets.len(),
            1,
            "§1.5: exactly one target is offered for the CSV↔TSV slice"
        );
        let target = offer
            .targets
            .first()
            .expect("§1.5: the slice offers one target");
        assert_eq!(
            target.id,
            TargetId::Format(FormatId::Tsv),
            "§1.5: the one offered target is TSV (the single offer IS the default)"
        );
    }

    // §6.4.1 unit (G15) / §2.13: an unresolvable `collectedSetId` (empty registry — expired/superseded/never
    // registered) is the InternalError catch-all — SHAPE asserted (kind), NOT the provisional message (owned
    // by the §2.8 catalog box).
    #[test]
    fn resolve_targets_of_an_unresolvable_id_is_the_internalerror_catch_all() {
        let sets = CollectedSetRegistry::default();
        let err = resolve_targets(&sets, collected_set_id())
            .expect_err("§2.13: an unresolvable set id yields the not-available Err");
        assert_eq!(
            err.kind,
            ConversionErrorKind::InternalError,
            "§2.13: the unresolvable-set outcome is the InternalError catch-all"
        );
    }

    // §6.4.1 unit (G15): the C3 handler is AppHandle-coupled boot-glue (§1.1a; G28-exempt) — a source-scan pins
    // it binds an `AppHandle`, resolves `State<CollectedSetRegistry>`, and DISPATCHES via `resolve_targets` (the
    // `&sets, collected_set_id` needle carries the call-site args so it matches the CALL, not the def). Needles
    // `concat!`-assembled (self-match avoidance).
    #[test]
    fn get_targets_handler_binds_apphandle_and_dispatches_via_the_helper() {
        let src = production_planning_source();
        for needle in [
            concat!("pub async fn get_", "targets("),
            concat!("app: App", "Handle"),
            concat!("state::<CollectedSet", "Registry>()"),
            concat!("resolve_", "targets(&sets, collected_set_id)"),
        ] {
            assert!(
                src.contains(needle),
                "§0.4.1/§1.5: the C3 get_targets handler must bind an AppHandle, resolve the CollectedSetRegistry, \
                 and dispatch via resolve_targets (missing `{needle}`)"
            );
        }
    }
}

#[cfg(test)]
mod c4_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C4 `plan_output` — the §1.8 output-plan preview, WIRED (P3.49). Same
    //! AppHandle-coupled boot-glue pattern as C3 (§1.1a; G28-exempt): the resolve LOGIC is the AppHandle-free
    //! `resolve_output_plan` helper (unit-tested with a real registry + a real freeze + a real FS probe), the
    //! handler WIRING is source-scan-pinned. [Build-Session-Entscheidung: P3.49]
    //!
    //! [Test-Change: P3.49 — old-obsolete+new-correct, §1.8] the P2.26 `block_on(plan_output(..))` contract
    //! test is OBSOLETE (the handler now binds an `AppHandle`; the shell's `Err` is superseded by the real §1.8
    //! preview). REPLACED by the `resolve_output_plan` unit tests (a registered CSV set → the beside-source
    //! `OutputPlanPreview` read back; an unresolvable id → the InternalError catch-all) + the handler source-scan.
    use std::collections::BTreeMap;

    use super::support::{non_ephemeral_tempdir, production_planning_source, register_one_csv};
    use super::*;
    use crate::domain::FormatId;

    fn collected_set_id() -> CollectedSetId {
        serde_json::from_str(r#""55555555-5555-4555-8555-555555555555""#)
            .expect("CollectedSetId deserializes from a uuid string")
    }

    fn no_options() -> OptionValues {
        OptionValues(BTreeMap::new())
    }

    // §6.4.1 real-FS (G15) / §1.8: a registered CSV set previews its beside-source output plan — the set id, a
    // non-empty "will save to" directory, NO divert (a non-ephemeral writable source dir), NO re-run prompt (a
    // first run, empty ledger), and the §1.10-seam trivial verdict (never up-front doomed). Read back from the real
    // `plan_output_preview` + a real `location_status` probe (test-strategy §0.1/§0.2).
    #[test]
    fn resolve_output_plan_previews_the_beside_source_plan_for_a_registered_csv_set() {
        let Some(dir) = non_ephemeral_tempdir() else {
            // The crate root is itself under an OS temp root — `location_status` would classify it Ephemeral, so
            // the "no divert" assertion is unreachable here. A clean skip (the fs_guard `location_status_tests`
            // precedent), never a false pass.
            return;
        };
        let sets = CollectedSetRegistry::default();
        let equiv = EquivKeyComputer::default();
        let ledger = RerunLedger::default();
        let id = register_one_csv(&sets, dir.path());
        let preview = resolve_output_plan(
            &sets,
            &equiv,
            &ledger,
            InstanceId::mint(),
            id,
            TargetId::Format(FormatId::Tsv),
            &no_options(),
            &ResolvedDestination::BesideSource,
        )
        .expect("a registered CSV set resolves an OutputPlanPreview");
        assert_eq!(
            preview.set, id,
            "§1.8: the preview names the resolved collected set"
        );
        assert_eq!(
            preview.diverted, None,
            "§2.7.2: a writable, non-ephemeral beside-source destination is not diverted"
        );
        assert_eq!(
            preview.rerun, None,
            "§2.5: a first run (empty ledger) has no equivalent prior run → no re-run prompt"
        );
        assert_eq!(
            preview.preflight.up_front_fail, None,
            "§1.10-seam: the CSV/TSV slice is never up-front doomed (the trivial slice verdict; the real estimator is P4.72)"
        );
        assert!(
            !preview.final_dir_display.is_empty(),
            "§1.8: the 'will save to' directory is shown (a non-empty lossy display)"
        );
    }

    // §6.4.1 real-FS (G15) / §1.8: a ChosenRoot destination previews the CHOSEN directory as the "will save to"
    // line (not the source's parent) — the `preview_final_dir` ChosenRoot branch, distinct from the
    // BesideSource case above.
    #[test]
    fn resolve_output_plan_previews_the_chosen_root_for_a_chosen_destination() {
        let Some(source_dir) = non_ephemeral_tempdir() else {
            return; // crate root under an OS temp root — a clean skip (the fs_guard precedent).
        };
        let Some(chosen_dir) = non_ephemeral_tempdir() else {
            return;
        };
        let sets = CollectedSetRegistry::default();
        let equiv = EquivKeyComputer::default();
        let ledger = RerunLedger::default();
        let id = register_one_csv(&sets, source_dir.path());
        let preview = resolve_output_plan(
            &sets,
            &equiv,
            &ledger,
            InstanceId::mint(),
            id,
            TargetId::Format(FormatId::Tsv),
            &no_options(),
            &ResolvedDestination::ChosenRoot(chosen_dir.path().to_path_buf()),
        )
        .expect("a registered CSV set with a chosen root resolves an OutputPlanPreview");
        assert_eq!(
            preview.diverted, None,
            "§2.7.2: a writable, non-ephemeral chosen root is not diverted"
        );
        assert_eq!(
            preview.final_dir_display,
            chosen_dir.path().to_string_lossy().into_owned(),
            "§1.8: a ChosenRoot destination previews the CHOSEN directory as the 'will save to' line"
        );
    }

    // §6.4.1 unit (G15) / §2.13: an unresolvable `collectedSetId` is the InternalError catch-all (SHAPE, not the
    // provisional message).
    #[test]
    fn resolve_output_plan_of_an_unresolvable_id_is_the_internalerror_catch_all() {
        let sets = CollectedSetRegistry::default();
        let equiv = EquivKeyComputer::default();
        let ledger = RerunLedger::default();
        let err = resolve_output_plan(
            &sets,
            &equiv,
            &ledger,
            InstanceId::mint(),
            collected_set_id(),
            TargetId::Format(FormatId::Tsv),
            &no_options(),
            &ResolvedDestination::BesideSource,
        )
        .expect_err("§2.13: an unresolvable set id yields the not-available Err");
        assert_eq!(
            err.kind,
            ConversionErrorKind::InternalError,
            "§2.13: the unresolvable-set outcome is the InternalError catch-all"
        );
    }

    // §6.4.1 unit (G15): the C4 handler is AppHandle-coupled boot-glue (§1.1a; G28-exempt) — a source-scan pins
    // it binds an `AppHandle`, resolves the FIVE States (incl. the P3.80 `DestinationRegistry`; the
    // `state::<InstanceId>()` needle is call-specific), resolves the wire ChosenRoot(DestinationId) via
    // `resolve_choice` (§0.4.4/§0.4.3), and DISPATCHES via `resolve_output_plan`. Needles `concat!`-assembled
    // (self-match avoidance).
    #[test]
    fn plan_output_handler_binds_apphandle_and_dispatches_via_the_helper() {
        let src = production_planning_source();
        for needle in [
            concat!("pub async fn plan_", "output("),
            concat!("app: App", "Handle"),
            concat!("spawn_", "blocking(move"),
            concat!("state::<CollectedSet", "Registry>()"),
            concat!("state::<Destination", "Registry>()"),
            concat!("state::<EquivKey", "Computer>()"),
            concat!("state::<Rerun", "Ledger>()"),
            concat!("state::<Instance", "Id>()"),
            concat!("resolve_", "choice(&destination)"),
            concat!("resolve_output_", "plan("),
        ] {
            assert!(
                src.contains(needle),
                "§0.4.1/§1.8/§0.4.4: the C4 plan_output handler must bind an AppHandle, resolve the five States \
                 (incl. DestinationRegistry), resolve the destination id (resolve_choice), and dispatch via \
                 resolve_output_plan (missing `{needle}`)"
            );
        }
    }
}

#[cfg(test)]
mod c5_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C5 `set_destination` — the §1.8/§2.14.4 destination-change re-validation,
    //! WIRED (P3.56). Same AppHandle-coupled boot-glue pattern as C4 (§1.1a; G28-exempt): the resolve LOGIC is the
    //! AppHandle-free `resolve_destination_change` helper (unit-tested with a real registry + a real freeze + a real
    //! FS probe), the handler WIRING is source-scan-pinned. [Build-Session-Entscheidung: P3.56]
    //!
    //! [Test-Change: P3.56 — old-obsolete+new-correct, §1.8] the P2.27 `block_on(set_destination(..))` contract test
    //! is OBSOLETE (the handler now binds an `AppHandle`; the shell's unconditional `Err(InternalError)` is superseded
    //! by the real §1.8/§2.14.4 re-validation). REPLACED by the `resolve_destination_change` unit tests (a registered
    //! CSV set → the DestinationResolved echo read back; a ChosenRoot echo; an unresolvable id → the InternalError
    //! catch-all) + the handler source-scan — the boot-glue stratification (the C4 P3.49 precedent), NOT a dropped
    //! assertion.
    use std::collections::BTreeMap;

    use super::support::{non_ephemeral_tempdir, production_planning_source, register_one_csv};
    use super::*;
    use crate::domain::FormatId;
    use crate::orchestrator::DestinationRegistry;

    fn collected_set_id() -> CollectedSetId {
        serde_json::from_str(r#""66666666-6666-4666-8666-666666666666""#)
            .expect("CollectedSetId deserializes from a uuid string")
    }

    fn no_options() -> OptionValues {
        OptionValues(BTreeMap::new())
    }

    // §6.4.1 real-FS (G15) / §1.8/§2.7.2: a registered CSV set re-validates a beside-source destination change — the
    // DestinationResolved ECHOES the (beside-source) choice, a non-empty "will save to" dir, NO divert (writable,
    // non-ephemeral), NO re-run (first run, empty ledger, carried through), and the §1.10-seam trivial verdict. Read
    // back from the real `plan_output_preview` + a real `location_status` probe (test-strategy §0.1/§0.2).
    #[test]
    fn resolve_destination_change_echoes_beside_source_and_re_validates() {
        let Some(dir) = non_ephemeral_tempdir() else {
            // crate root under an OS temp root — `location_status` would classify it Ephemeral, so the "no divert"
            // assertion is unreachable. A clean skip (the fs_guard `location_status_tests` precedent).
            return;
        };
        let sets = CollectedSetRegistry::default();
        let equiv = EquivKeyComputer::default();
        let ledger = RerunLedger::default();
        let id = register_one_csv(&sets, dir.path());
        let resolved = resolve_destination_change(
            &sets,
            &equiv,
            &ledger,
            InstanceId::mint(),
            id,
            TargetId::Format(FormatId::Tsv),
            &no_options(),
            DestinationChoice::BesideSource,
            &ResolvedDestination::BesideSource,
        )
        .expect("a registered CSV set re-validates a DestinationResolved");
        assert_eq!(
            resolved.destination,
            DestinationChoice::BesideSource,
            "§0.4.1: C5 echoes the caller's destination choice back (no server-side destination store)"
        );
        assert_eq!(
            resolved.diverted, None,
            "§2.7.2: a writable, non-ephemeral beside-source destination is not diverted"
        );
        assert_eq!(
            resolved.rerun, None,
            "§2.5.1: a first run (empty ledger) carries through no re-run verdict"
        );
        assert_eq!(
            resolved.preflight.up_front_fail, None,
            "§1.10-seam: the CSV/TSV slice is never up-front doomed (the trivial slice verdict)"
        );
        assert!(
            !resolved.final_dir_display.is_empty(),
            "§1.8: the re-validated 'will save to' directory is shown (a non-empty lossy display)"
        );
    }

    // §6.4.1 real-FS (G15) / §1.8: a ChosenRoot destination change re-validates the CHOSEN directory as the "will
    // save to" line AND echoes the ChosenRoot choice — the C5 destination-owns leg (distinct from BesideSource). The
    // id is minted through a real `DestinationRegistry` (as C2b does), resolved via `resolve_choice`.
    #[test]
    fn resolve_destination_change_echoes_and_previews_a_chosen_root() {
        let Some(source_dir) = non_ephemeral_tempdir() else {
            return; // crate root under an OS temp root — a clean skip (the fs_guard precedent).
        };
        let Some(chosen_dir) = non_ephemeral_tempdir() else {
            return;
        };
        let sets = CollectedSetRegistry::default();
        let equiv = EquivKeyComputer::default();
        let ledger = RerunLedger::default();
        let destinations = DestinationRegistry::default();
        let id = register_one_csv(&sets, source_dir.path());
        let dest_id = destinations.register(chosen_dir.path().to_path_buf());
        let choice = DestinationChoice::ChosenRoot(dest_id);
        let resolved_dest = destinations
            .resolve_choice(&choice)
            .expect("the just-registered ChosenRoot id resolves");
        let resolved = resolve_destination_change(
            &sets,
            &equiv,
            &ledger,
            InstanceId::mint(),
            id,
            TargetId::Format(FormatId::Tsv),
            &no_options(),
            choice.clone(),
            &resolved_dest,
        )
        .expect("a ChosenRoot destination change re-validates a DestinationResolved");
        assert_eq!(
            resolved.destination, choice,
            "§0.4.1: C5 echoes the ChosenRoot(DestinationId) choice back verbatim"
        );
        assert_eq!(
            resolved.final_dir_display,
            chosen_dir.path().to_string_lossy(),
            "§1.8: a ChosenRoot destination re-validates the CHOSEN directory as the 'will save to' line"
        );
    }

    // §6.4.1 unit (G15) / §2.13: an unresolvable `collectedSetId` is the InternalError catch-all (SHAPE, not the
    // provisional message owned by the §2.8 catalog box).
    #[test]
    fn resolve_destination_change_of_an_unresolvable_id_is_the_internalerror_catch_all() {
        let sets = CollectedSetRegistry::default();
        let equiv = EquivKeyComputer::default();
        let ledger = RerunLedger::default();
        let err = resolve_destination_change(
            &sets,
            &equiv,
            &ledger,
            InstanceId::mint(),
            collected_set_id(),
            TargetId::Format(FormatId::Tsv),
            &no_options(),
            DestinationChoice::BesideSource,
            &ResolvedDestination::BesideSource,
        )
        .expect_err("§2.13: an unresolvable set id yields the not-available Err");
        assert_eq!(
            err.kind,
            ConversionErrorKind::InternalError,
            "§2.13: the unresolvable-set outcome is the InternalError catch-all"
        );
    }

    // §6.4.1 unit (G15): the C5 handler is AppHandle-coupled boot-glue (§1.1a; G28-exempt) — a source-scan pins it
    // binds an `AppHandle`, resolves the FIVE States (incl. the DestinationRegistry), resolves the wire
    // ChosenRoot(DestinationId) via `resolve_choice` (§0.4.4/§0.4.3), and DISPATCHES via `resolve_destination_change`.
    // Needles `concat!`-assembled (self-match avoidance).
    #[test]
    fn set_destination_handler_binds_apphandle_and_dispatches_via_the_helper() {
        let src = production_planning_source();
        for needle in [
            concat!("pub async fn set_", "destination("),
            concat!("app: App", "Handle"),
            concat!("spawn_", "blocking(move"),
            concat!("state::<CollectedSet", "Registry>()"),
            concat!("state::<Destination", "Registry>()"),
            concat!("resolve_", "choice(&destination)"),
            concat!("resolve_destination_", "change("),
        ] {
            assert!(
                src.contains(needle),
                "§0.4.1/§1.8/§0.4.4: the C5 set_destination handler must bind an AppHandle, resolve the States (incl. \
                 DestinationRegistry), resolve the destination id (resolve_choice), and dispatch via \
                 resolve_destination_change (missing `{needle}`)"
            );
        }
    }
}

#[cfg(test)]
mod c14_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C14 `get_initial_destination` — the persisted-destination hand-off query,
    //! WIRED (P3.56). Same AppHandle-coupled boot-glue pattern as C3/C4/C5 (§1.1a; G28-exempt): the handler binds
    //! an `AppHandle` (to read the prefs store + reach `State<DestinationRegistry>` / `State<InstanceId>`), so it is
    //! NOT cargo-test-invocable; its resolve LOGIC is the AppHandle-free
    //! `orchestrator::resolve_persisted_destination` (unit-tested in `crate::orchestrator` with a real registry + a
    //! real FS probe — the 3-way `InitialDestination` outcome), the handler WIRING is source-scan-pinned. The
    //! §0.4.1 typed wire surface stays asserted by the bindings.ts golden (`bindings_codegen` in main.rs).
    //! [Build-Session-Entscheidung: P3.56]
    use super::support::production_planning_source;

    // §6.4.1 unit (G15): the C14 handler is AppHandle-coupled boot-glue (§1.1a; G28-exempt) — a source-scan pins it
    // binds an `AppHandle`, reads the prefs (`prefs::load`), resolves `State<DestinationRegistry>` + the probe
    // (`InstanceId` → `probe_name`) on a blocking thread, and DISPATCHES via `resolve_persisted_destination`.
    // Needles `concat!`-assembled (self-match avoidance).
    #[test]
    fn get_initial_destination_handler_reads_prefs_and_dispatches_via_the_resolver() {
        let src = production_planning_source();
        for needle in [
            concat!("pub async fn get_initial_", "destination("),
            concat!("app: App", "Handle"),
            concat!("spawn_", "blocking(move"),
            concat!("prefs::", "load(&app)"),
            concat!("state::<Destination", "Registry>()"),
            concat!("probe_", "name(instance)"),
            concat!("resolve_persisted_", "destination("),
        ] {
            assert!(
                src.contains(needle),
                "§0.4.1/§5.8: the C14 get_initial_destination handler must bind an AppHandle, read the prefs, resolve \
                 the DestinationRegistry + probe on a blocking thread, and dispatch via resolve_persisted_destination \
                 (missing `{needle}`)"
            );
        }
    }
}
