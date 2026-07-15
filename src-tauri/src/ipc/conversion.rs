//! `crate::ipc::conversion` — the §0.4.1 run-lifecycle command group (C6 / C7 / C8): start a run, cancel a
//! run, and re-fetch a run summary. P2.21 registered these as the §0.4.1 command-surface interface shells;
//! C6 `start_conversion`'s typed request/response CONTRACT is authored by P2.29 (this file), C7's by P2.30,
//! and C8's by P2.31. Each command's `crate::orchestrator` delegation BODY is its own named fill-box (the C6
//! run conductor — the §1.9 batch build / lifecycle / §0.9 dispatch / `ConversionEvent` emission — is P3.48;
//! C8's §1.12 re-serve is P3.50; C7's cancel wiring is P3.52). Thin by design (§0.7): the handler validates,
//! delegates, and maps the `Result` onto the §0.4.3 `IpcError`.

// §0.4 / T10: unchecked arithmetic on an untrusted wire field must be a compile error in every IPC handler
// (the `crate::ipc` arithmetic-overflow deny cascades here; restated at the T10 boundary). The C6 conductor
// body (P3.48) + the C7 `cancel_run` handler (P3.52) do no arithmetic in this file; the deny bites any future fill-body.
#![deny(clippy::arithmetic_side_effects)]

use std::path::PathBuf;
use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tokio_util::sync::CancellationToken;

use crate::domain::{
    CollectedSetId, DestinationChoice, InstanceId, OptionValues, RerunDecision, RunId, TargetId,
};
use crate::engines::resolve_slice_target;
use crate::orchestrator::{
    build_batch, run_conversion, Batch, CollectedSetRegistry, ConversionEvent, DestinationRegistry,
    EquivKeyComputer, RegisteredSet, RunRegistry, RunResult, RunResultStore,
};
use crate::outcome::{ConversionErrorKind, IpcError};
use crate::pool::Pool;
use crate::run::{RerunLedger, RunScratch};

/// **C6 `start_conversion`** (§0.4.1) — mints the run's `RunId`, builds + enqueues the §1.9 batch from the
/// frozen collected set, spawns the §0.9 workers, and streams `ConversionEvent`s over the handed `onProgress`
/// Channel (the §0.4.2 E-series — `RunStarted` → per-item `ItemStarted`/`ItemProgress`/`ItemFinished` +
/// `BatchProgress` → terminal `RunFinished`); it returns immediately with the `RunId` (the run proceeds async,
/// the Channel carries all telemetry, §1.11). This box (P2.29) authors the typed §0.4.1 wire CONTRACT — the
/// `{ collectedSetId, target, options, destination, rerunDecision, onProgress } -> Result<RunId, IpcError>`
/// door (the §0.4 universal error shape) — so the generated `bindings.ts` mirrors the C6 surface and, **for
/// the first time, pulls the whole `ConversionEvent` graph onto the wire** (the P2.37 enum + its `RunStarted`/
/// `ItemStarted`/`ItemProgress`/`ItemFinished`/`BatchProgress` payloads + the `RunFinished` `RunResult` graph)
/// via the `onProgress` arg — the §0.6 defer-registration-to-the-consumer pattern (the `crate::orchestrator`
/// §0.4.2 note), exactly the `ScanProgress`-via-C1 precedent.
///
/// - `collected_set_id` — the frozen §0.4.4 collected-set handle (§2.4) the run is built from; resolved
///   against the §0.4.4 registry (P2.44) to the `CollectedSet::Single` whose items become the queue.
/// - `target` — the one whole-batch `TargetId` (§0.6 invariant 1 — one Target per Batch, never per item).
/// - `options` — the effective whole-batch `OptionValues` (§0.6 invariant 2 / §2.5).
/// - `destination` — **AUTHORITATIVE** (§0.4.1 C6 `[DECIDED]`): C4/C5 are plan/preview + revalidation only, the
///   value the UI passes here (the last C5-resolved destination) is what the run writes to. It names
///   beside-source or WHICH picked root by id (`ChosenRoot(DestinationId)`); `start_run` resolves the id against
///   the §0.4.4 `DestinationRegistry` to its real `PathBuf` (`resolve_choice`; an unknown id is the §0.4.3
///   refusal) BEFORE building the batch, so the run drives a resolved `ResolvedDestination` — the path never
///   crosses the wire (the 2026-07-06 core-owned-paths split).
/// - `rerun_decision` — the §0.6 `RerunDecision` (the user's answer to a C4 `RerunPrompt`, §2.5): `Skip` (the
///   safe default — no new output for equivalent items) or `FreshCopy` (fresh numbered copies).
/// - `on_progress` — the run-telemetry `Channel<ConversionEvent>` (§0.4.2): ordered, run-scoped (dies with
///   the run — no cross-run leak, §1.11). Like C1's `onScan` it is **non-optional** (tauri's `Channel<T>` is
///   `!Deserialize`, so `Option<Channel<T>>` cannot be a command arg — the same forced shape the C1
///   `drain_intake` handler documents; the §0.4.1 C6 row already specifies it non-optional).
///
/// [Build-Session-Entscheidung: P3.48 — the C6 body fill] The handler is a THIN AppHandle door (§0.7): it
/// injects the `AppHandle` (a `#[tauri::command]` special arg) and delegates to [`start_run`], which resolves
/// the §0.4.4 managed State + the §2.14 base paths, does the SYNC run setup, and SPAWNS the async run — so it
/// returns immediately with the minted `RunId` (§1.11; the `onProgress` Channel carries all telemetry). The
/// glue is AppHandle-coupled boot-glue (the §1.1a pattern — this crate ships no `tauri::test` mock BY
/// DECISION, so the State-resolution + spawn are SOURCE-SCAN-pinned, not `tauri::test`-executed; the PURE
/// conductor `crate::orchestrator::run_conversion` is unit-tested directly over a directly-registered frozen
/// set). `on_progress` stays the non-optional run Channel (tauri's `Channel<T>` is `!Deserialize`, so
/// `Option<Channel<T>>` cannot be a command arg — the C1 `onScan` forced shape). Its `AppHandle` signature
/// makes the handler + `start_run` + `run_conversion_spawned` G28 diff-floor-exempt (the §1.1a boot-glue
/// exemption).
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn start_conversion(
    collected_set_id: CollectedSetId,
    target: TargetId,
    options: OptionValues,
    destination: DestinationChoice,
    rerun_decision: RerunDecision,
    on_progress: Channel<ConversionEvent>,
    app: AppHandle,
) -> Result<RunId, IpcError> {
    start_run(
        &app,
        collected_set_id,
        target,
        options,
        destination,
        rerun_decision,
        on_progress,
    )
}

/// The §2.13 not-startable `IpcError` — the §0.4.3 shape every C6 refusal maps to (an unresolvable
/// `collectedSetId`, an unoffered target, or a missing managed store / scratch failure). `kind` is the
/// concrete `ConversionErrorKind` (the P2.19 convention); the message is a calm, trace-free command-level line
/// (§2.13 — the §2.8 catalog is item-scoped, so the run-start command carries its own string).
/// [Build-Session-Entscheidung: P3.48]
fn not_startable() -> IpcError {
    IpcError {
        kind: ConversionErrorKind::InternalError,
        message: "Could not start the conversion.".into(),
        path_display: None,
        residue_display: None,
    }
}

/// The C6 run SETUP (§0.4.1 / §0.4.4, P3.48) — resolve + evict the frozen collected set (§0.4.4 "evicted when
/// its run starts"), validate the §1.5 target and build the §1.9 `Batch`, mint the `RunId` + set up its
/// §0.4.4 registries (evict the prior `RunResult`, register a fresh cancellation token), acquire the §2.14
/// per-run scratch (lock-before-part), resolve the §2.7.3 divert root, and SPAWN the async run — returning the
/// `RunId` immediately (§1.11). AppHandle-coupled boot-glue (the §1.1a pattern; G28 signature-exempt,
/// source-scan-pinned). [Build-Session-Entscheidung: P3.48]
fn start_run(
    app: &AppHandle,
    collected_set_id: CollectedSetId,
    target: TargetId,
    options: OptionValues,
    destination: DestinationChoice,
    rerun_decision: RerunDecision,
    on_progress: Channel<ConversionEvent>,
) -> Result<RunId, IpcError> {
    // §0.4.4: resolve + EVICT the frozen collected set (C6 takes it out of the registry as its run begins —
    // the `Batch` becomes the sole post-C6 carrier, incl. the pre-flight skips, §1.9). An unknown / superseded
    // id is refused (the WebView cannot name a set the freeze did not register). `take` is EARLY BY DESIGN — it
    // is the atomic serialization gate for two concurrent C6s on the SAME set (the first evicts, the second
    // finds nothing and is refused), so it must NOT be moved after the fallible scratch/path IO below (a `take`
    // via a non-evicting `resolve` would let both run — a double-conversion regression). The accepted cost: a
    // transient scratch/path IO failure below has already evicted the set, so the user re-drops — recoverable,
    // corrupts no persistent state (unlike the run-token/`RunResult` mutations, which are moved past that IO).
    let collected_sets = app
        .try_state::<CollectedSetRegistry>()
        .ok_or_else(not_startable)?;
    // §0.4.4 (P3.80): resolve the wire `DestinationChoice`'s `ChosenRoot(DestinationId)` against the picked-roots
    // registry to its real `PathBuf` (`resolve_choice`) — an unknown id is the §0.4.3 refusal (`not_startable`).
    // Done BEFORE the eviction gate so a bad destination id NEVER evicts the frozen set: a pure in-memory registry
    // read cannot affect the concurrent-C6 `take` serialization, and refusing here (the set stays registered) lets
    // the user retry without re-dropping. The pure §1.8/§2.7 conductor then drives a resolved
    // `ResolvedDestination`, never a registry lookup (the 2026-07-06 core-owned-paths split, the C9 `open_path`
    // id-resolution mirror). [Build-Session-Entscheidung: P3.80]
    let destinations = app
        .try_state::<DestinationRegistry>()
        .ok_or_else(not_startable)?;
    let resolved_destination = destinations
        .resolve_choice(&destination)
        .ok_or_else(not_startable)?;
    let registered = collected_sets
        .take(collected_set_id)
        .ok_or_else(not_startable)?;

    // §1.5: validate + resolve the wire `TargetId` to the full `Target` (refuse an unoffered pair, §0.6 inv 1 —
    // the UI never presents one). §1.9: build the batch (eligible `Pending` jobs + pre-flight `Skipped` records).
    let full_target =
        resolve_slice_target(registered.frozen.format, target).ok_or_else(not_startable)?;
    let batch = build_batch(
        &registered.frozen,
        full_target,
        options,
        resolved_destination,
    );

    // Mint the run id (at C6 accept, NOT the §2.4 freeze — §7.1.2).
    let run_id = RunId::mint();

    // §2.14 scratch: acquire the per-run scratch under `app_local_data_dir()` (lock-before-part, §2.6.3), and
    // resolve the §2.7.3 divert root — the user Downloads (then Documents, then the scratch base as a last
    // resort) — AppHandle-side via Tauri's PathResolver (the P3.35 "candidates resolved AppHandle-side"). These
    // are ALL the fallible steps, done BEFORE any registry mutation so a failure here leaves the §0.4.4 stores
    // untouched.
    let instance = *app.try_state::<InstanceId>().ok_or_else(not_startable)?;
    let scratch_base = app
        .path()
        .app_local_data_dir()
        .map_err(|_| not_startable())?;
    let scratch = RunScratch::acquire(&scratch_base, instance, std::process::id(), run_id)
        .map_err(|_| not_startable())?;
    let divert_root = app
        .path()
        .download_dir()
        .or_else(|_| app.path().document_dir())
        .unwrap_or_else(|_| scratch_base.clone());

    // §0.4.4 registry setup — the LAST mutations before the infallible spawn. Resolve BOTH stores first, then
    // evict the prior retained `RunResult` (§0.4.4 "until a new run starts") and register a fresh cancellation
    // token. `register` MUST be the final fallible-free step so no post-register early-return can leak the token:
    // a leaked token leaves `RunRegistry::has_active_run` permanently true, wedging `§7.1.1 converter_is_busy` /
    // the §7.3 close-guard for the whole process lifetime (the G1 dual-review blocker — the scratch/path IO
    // above formerly ran AFTER `register`, so a transient disk/permission failure bricked the convert path).
    let results_store = app
        .try_state::<RunResultStore>()
        .ok_or_else(not_startable)?;
    let runs = app.try_state::<RunRegistry>().ok_or_else(not_startable)?;
    results_store.evict();
    let token = runs.register(run_id);

    // §1.11: spawn the async run + return the `RunId` immediately (the Channel carries all telemetry). The
    // spawned task re-resolves the managed State via the cloned AppHandle (the handler's `State` guards cannot
    // cross the `'static` spawn boundary).
    let app_for_run = app.clone();
    tauri::async_runtime::spawn(async move {
        run_conversion_spawned(
            app_for_run,
            batch,
            registered,
            run_id,
            token,
            scratch,
            instance,
            divert_root,
            rerun_decision,
            on_progress,
        )
        .await;
    });
    Ok(run_id)
}

/// The spawned async run (P3.48) — re-resolve the §0.4.4 managed State inside the `'static` task (the
/// handler's `State` guards can't cross the spawn) and drive the pure `crate::orchestrator::run_conversion`.
/// A missing managed store is a boot-wiring bug (the `boot_invariants` source-scan pins all of
/// Pool/RerunLedger/EquivKeyComputer/RunResultStore/RunRegistry as `.manage`d), so it drops the run token so a
/// later run is not blocked and returns without emitting `RunFinished` — unreachable in a correctly-wired app.
/// AppHandle-coupled boot-glue (§1.1a; G28 signature-exempt). [Build-Session-Entscheidung: P3.48]
#[allow(clippy::too_many_arguments)]
async fn run_conversion_spawned(
    app: AppHandle,
    batch: Batch,
    registered: Arc<RegisteredSet>,
    run_id: RunId,
    token: CancellationToken,
    scratch: RunScratch,
    instance: InstanceId,
    divert_root: PathBuf,
    rerun_decision: RerunDecision,
    on_progress: Channel<ConversionEvent>,
) {
    let (Some(pool), Some(ledger), Some(equiv), Some(results), Some(runs)) = (
        app.try_state::<Pool>(),
        app.try_state::<RerunLedger>(),
        app.try_state::<EquivKeyComputer>(),
        app.try_state::<RunResultStore>(),
        app.try_state::<RunRegistry>(),
    ) else {
        if let Some(runs) = app.try_state::<RunRegistry>() {
            runs.finish(run_id);
        }
        return;
    };
    run_conversion(
        batch,
        registered.as_ref(),
        run_id,
        token,
        scratch,
        instance,
        divert_root,
        rerun_decision,
        pool.inner(),
        ledger.inner(),
        equiv.inner(),
        results.inner(),
        runs.inner(),
        &on_progress,
    )
    .await;
}

/// **C7 `cancel_run`** (§0.4.1) — trips the `RunId`-indexed §0.4.4 cancellation token (`.cancel()` on the
/// run-registry `CancellationToken`, P2.42): finished items are kept, the in-progress item is discarded
/// cleanly (§2.1/§2.6), and the forceful engine kill is §1.7's mechanism (cooperative at the orchestrator
/// level, forceful at the engine level). This box (P2.30) authors the typed §0.4.1 wire CONTRACT — the
/// `{ runId } -> Result<(), IpcError>` door (the §0.4 universal error shape) — so the generated `bindings.ts`
/// mirrors the C7 surface.
///
/// - `run_id` — the §0.4.4 `RunId` (minted at C6) whose cancellation token to trip.
///
/// [Build-Session-Entscheidung: P3.52] **The `.cancel()` trip is WIRED** (no longer the P2.30 `Ok(())` shell).
/// The handler binds an `AppHandle` (Tauri-injected — the §0.4.1 wire signature stays `{ runId }`) to reach the
/// §0.4.4 `RunRegistry` (`.manage`d in main, P2.42) and **trips the run's cancellation token** via
/// `RunRegistry::cancel(run_id)` — the exact mirror of the C13 `cancel_ingest` wiring (P2.71). The cancel EFFECT
/// is then observed by the in-flight conversion: the §1.7 / P3.44 cooperative cancel keeps already-finished
/// items and discards the in-progress one cleanly (§2.1/§2.6); the forceful engine kill is §1.7's mechanism.
/// C7's own return never carries the effect — the kill *outcome* is reported async via the
/// `ConversionEvent`/`RunResult` (§1.7/§1.12). **Idempotent `Ok(())` `[DECIDED]`:** a cancel of an unknown /
/// already-finished run finds no live token (`RunRegistry::cancel` returns `false`) — the genuine "not running"
/// end-state (§0.4.4), the C13 `cancel_ingest` mirror — so the result is ALWAYS `Ok(())` (the §0.4.1 C7
/// idempotent contract), NEVER an `Err`. This handler is AppHandle-coupled boot-glue (§1.1a; G28
/// signature-exempt): the trip LOGIC is `RunRegistry::cancel` (unit-tested at P2.42, + the end-to-end
/// token-trip chain proven in the `c7_contract` tests here), the WIRING source-scan-pinned.
/// `app.state::<RunRegistry>()` is infallible by construction (the registry is `.manage`d in main()'s Builder
/// chain before the event loop, P2.42 — no panic under the `crate::ipc` clippy::panic deny).
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn cancel_run(app: AppHandle, run_id: RunId) -> Result<(), IpcError> {
    // §0.4.4/§1.1 (P3.52): trip the run's cancellation token so the in-flight conversion observes the cancel —
    // the §1.7/P3.44 cooperative cancel keeps already-finished items and discards the in-progress one cleanly
    // (§2.1/§2.6). Idempotent: a cancel of an unknown/already-finished run finds no token (cancel returns false)
    // — the genuine "not running" no-op (§0.4.4) — so the result is ALWAYS Ok(()) (the §0.4.1 C7 contract),
    // never an error. The kill OUTCOME is reported async via the ConversionEvent/RunResult (§1.7/§1.12).
    let _ = app.state::<RunRegistry>().cancel(run_id);
    Ok(())
}

/// **C8 `get_run_summary`** (§0.4.1) — the idempotent re-fetch of the retained §1.12 `RunResult` (the
/// end-of-batch summary: per-item outcome + output→source map + residue warnings + the open-folder roots),
/// also delivered once as the terminal `ConversionEvent::RunFinished`. C8 re-serves it from the §0.4.4
/// run-registry retention (the `RunResult` is held process-local until the next run starts or the app exits,
/// P2.43 — distinct from the cancellation token, which is dropped on `RunFinished`); the re-serve covers a
/// fresh listener attaching *after* the run has terminated (e.g. a WebView reload — v1 does not claim macOS
/// reload-mid-stream resilience, §0.4.4). This box (P2.31) authors the typed §0.4.1 wire CONTRACT — the
/// `{ runId } -> Result<RunResult, IpcError>` door (the §0.4 universal error shape) — so the generated
/// `bindings.ts` carries the C8 surface (the whole `RunResult` graph already mirrored via C6's `RunFinished`,
/// P2.29).
///
/// - `run_id` — the §0.4.4 `RunId` (minted at C6) whose retained summary to re-serve.
///
/// [Build-Session-Entscheidung: P3.50 — the C8 body fill] The handler is THIN (§0.7): it resolves the retained
/// §1.12 `RunResult` from the §0.4.4 `State<RunResultStore>` (P2.43) and maps the `Option` onto the §0.4.3
/// `IpcError` — the whole logic lives in the pure [`resolve_run_summary`] so it is unit-tested WITHOUT a
/// `tauri::test` mock (this crate ships none BY DECISION), while the handler's `State` injection is
/// source-scan-pinned (the C1/C13 pattern). `Some` (a run's terminal summary is retained AND its `run_id`
/// matches) → `Ok(RunResult)`; `None` (no run retained, or a different / superseded run's id) → the honest
/// `Err(IpcError{ InternalError })` — exactly the pre-P3.50 shell outcome for an unresolvable / not-yet-finished
/// id (§2.13 catch-all). The `RunResult` graph is DISPLAY-ONLY (§2.10.1); the real roots + per-item output/
/// residue `PathBuf`s stay in the store's off-wire `RunResultPaths` (C9 resolves its `OpenTarget` there, P3.79).
/// `kind` stays the CONCRETE `ConversionErrorKind` (the P2.19 convention). The store is `.manage`d in `main()`'s
/// Builder chain (added with this box), mirroring the sibling `RunRegistry`; the retain-at-`RunFinished` /
/// evict-at-C6 lifecycle that POPULATES it is the P3.48 conductor — until then the store is empty and C8
/// honestly returns `Err`, the walking-skeleton state.
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn get_run_summary(
    run_id: RunId,
    store: State<'_, RunResultStore>,
) -> Result<RunResult, IpcError> {
    resolve_run_summary(store.inner(), run_id)
}

/// The pure C8 resolve (§0.4.4 / §1.12, P3.50) — re-serve the retained wire `RunResult` for `run_id`, mapping
/// the store's `Option` onto the §0.4.3 `IpcError`. Separated from the `#[tauri::command]` handler so the
/// mapping is directly unit-testable over a real `RunResultStore` (no `tauri::test` mock). `None` → the
/// InternalError not-available result (an unresolvable / not-yet-finished / superseded id, §2.13).
/// [Build-Session-Entscheidung: P3.50]
fn resolve_run_summary(store: &RunResultStore, run_id: RunId) -> Result<RunResult, IpcError> {
    store.get(run_id).ok_or_else(|| IpcError {
        kind: ConversionErrorKind::InternalError,
        message: "Could not retrieve the conversion summary.".into(),
        path_display: None,
        residue_display: None,
    })
}

#[cfg(test)]
mod c6_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C6 `start_conversion` BODY (P3.48 — replacing the P2.29 shell). C6 is
    //! AppHandle-coupled boot-glue: the handler injects the `AppHandle`, delegates to `start_run` (which
    //! resolves the §0.4.4 managed State + the §2.14 paths, builds the §1.9 batch, and SPAWNS the async run),
    //! and this crate ships no `tauri::test` mock BY DECISION (the C1/C8 pattern), so the handler's wiring is
    //! SOURCE-SCAN-pinned here; the PURE conductor `crate::orchestrator::run_conversion` is unit-tested
    //! DIRECTLY over a directly-registered frozen set (the `run_conversion_tests` module in
    //! `crate::orchestrator`). [Build-Session-Entscheidung: P3.48]
    //!
    //! [Test-Change: P3.48 — old-obsolete+new-correct, §0.4.1] this module's P2.29 shell contract test (which
    //! called `start_conversion` directly via `block_on` and asserted the shell's genuine pre-registry
    //! `Err(InternalError)`) is SUPERSEDED: the shell no longer exists (P3.48 built the real body + added the
    //! `AppHandle` arg, so the old 6-arg `block_on(start_conversion(...))` cannot compile and its `Err`
    //! expectation is obsolete), and the handler is now AppHandle-coupled (not `block_on`-callable without a
    //! Tauri runtime). The new source-scan pins verify the handler delegates to `start_run` + spawns
    //! `run_conversion` — the same shell→body transition as the sibling C8 contract module. (Comment cites the
    //! MODULE, never a deleted `#[test] fn` name — the G73 rs-test-refs module-anchor discipline.)

    // [Test-Change: P3.48 — old-obsolete+new-correct, §0.4.1] the deleted P2.29 shell helper's
    // `.expect("CollectedSetId deserializes…")` is obsolete — the shell (and its uuid-mint helper) is gone,
    // replaced by this source-scan helper (the module `//!` note carries the full rationale).
    /// The production prefix of `conversion.rs` (everything before the FIRST `#[cfg(test)]`), so a needle
    /// declared in this test can never self-match — the per-module copy of the `c7_contract`/`c8_contract`
    /// helper (each contract module keeps its own copy, the established per-module test-helper pattern).
    fn production_conversion_source() -> &'static str {
        let full = include_str!("conversion.rs");
        full.split_once(concat!("#[cfg", "(test)]"))
            .map_or(full, |(prefix, _)| prefix)
    }

    // §6.4.1 unit (G15): the C6 handler is THIN — it injects the `AppHandle` and delegates to `start_run`,
    // which resolves the §0.4.4 managed State, builds the batch, and SPAWNS the pure `run_conversion` conductor
    // (returning the `RunId` immediately, §1.11). AppHandle-coupled boot-glue (not cargo-test-runnable; the
    // PURE conductor is unit-tested in `crate::orchestrator::run_conversion_tests`), so a source-scan pins the
    // wiring. Needles `concat!`-split so the scan never matches its own text.
    #[test]
    fn c6_handler_injects_the_app_handle_and_delegates_to_start_run() {
        let src = production_conversion_source();
        assert!(
            src.contains(concat!("app: App", "Handle")),
            "§0.4.1: the C6 handler injects the AppHandle (the P3.48 body)"
        );
        // [Test-Change: P3.48 — old-obsolete+new-correct, §0.4.1] replaces the deleted P2.29 shell test's
        // `assert_eq!(err.kind, InternalError)` (+ its `expect_err`): the shell's pre-registry Err is gone (the
        // real body spawns the run); the outcome is asserted by these source-scan pins.
        assert!(
            src.contains(concat!("start_", "run(")),
            "§0.4.1/§0.7: the C6 handler delegates to start_run (resolve State + build batch + spawn)"
        );
        assert!(
            src.contains(concat!("async_runtime::", "spawn(")),
            "§1.11: start_run spawns the async run + returns the RunId immediately"
        );
        assert!(
            src.contains(concat!("run_", "conversion(")),
            "§1.9: the spawned task drives the pure crate::orchestrator::run_conversion conductor"
        );
        // §0.4.4 (P3.80): start_run resolves the wire ChosenRoot(DestinationId) against the DestinationRegistry
        // (resolve_choice) BEFORE build_batch — an unknown id is the §0.4.3 refusal; the pure conductor drives a
        // resolved ResolvedDestination, never a registry lookup (the 2026-07-06 core-owned-paths split). Needle
        // concat!-split (self-match avoidance; the call form lives only in the production prefix).
        assert!(
            src.contains(concat!("resolve_", "choice(&destination)")),
            "§0.4.4 (P3.80): start_run resolves ChosenRoot(DestinationId) via DestinationRegistry::resolve_choice \
             (an unknown id → the §0.4.3 refusal), then build_batch drives a resolved ResolvedDestination"
        );
        // §0.4.4 (P3.80, G1 Sonnet-P2 order-check): the destination resolution must run BEFORE the `take`
        // eviction gate — presence alone is insufficient, because the load-bearing property is the ORDER: a bad
        // destination id must refuse WITHOUT evicting the frozen set (so the user retries without re-dropping). A
        // silent reorder would regress that with no other gate catching it, so pin the order via `src.find()`
        // index comparison (the `main.rs` boot-order-test pattern). Needles `concat!`-split (self-match avoidance).
        let resolve_at = src
            .find(concat!("resolve_", "choice(&destination)"))
            .expect("start_run calls resolve_choice");
        let take_at = src
            .find(concat!(".take(collected_set", "_id)"))
            .expect("start_run calls take on the CollectedSetRegistry");
        assert!(
            resolve_at < take_at,
            "§0.4.4 (P3.80): resolve_choice must precede the `take` eviction gate so an unknown destination id is \
             refused BEFORE the frozen set is evicted (no evict-on-refusal; the user retries without re-dropping)"
        );
    }
}

#[cfg(test)]
mod c7_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C7 `cancel_run` `.cancel()` trip wiring (P3.52). The handler binds an
    //! `AppHandle` (to reach the §0.4.4 `RunRegistry` and trip the run's token), so it is AppHandle-coupled
    //! boot-glue (the §1.1a pattern — NOT cargo-test-invocable; this crate ships no `tauri::test` mock BY
    //! DECISION, G28 signature-exempt). The trip LOGIC is `RunRegistry::cancel` (unit-tested at P2.42); this
    //! module pins the handler WIRING by source-scan + proves the END-TO-END token-trip chain at the
    //! registry/token level (no runtime) — the exact mirror of the C13 `c13_contract` (P2.71).
    //! [Build-Session-Entscheidung: P3.52]
    //!
    //! [Test-Change: P3.52 — old-obsolete+new-correct, §0.4.1] the P2.30 shell contract test
    //! (`block_on(cancel_run(run_id()))` → the idempotent no-op `Ok(())`) AND the P2.137 `c7_shell_body_binds_...`
    //! source-scan are OBSOLETE: the shell no longer exists — the handler now binds an `AppHandle` (uninvocable
    //! under `cargo test`, §1.1a) and TRIPS the token via `RunRegistry::cancel`. They are REPLACED by the
    //! end-to-end token-trip chain (register → cancel → is_cancelled) — what C7 does and what the §1.7/P3.44
    //! cooperative cancel observes — plus the handler source-scan, exactly as the P2.137 `c7_shell_body_...`
    //! comment scheduled ("superseded by a behavioural token-tripped check at the fill-box"). STRICTLY STRONGER
    //! (the shell asserted only a no-op `Ok(())`; this asserts the real trip), NOT a dropped assertion.
    use super::*;

    /// A `RunId` for the registry-level end-to-end test — its PUBLIC bare-uuid `Deserialize` wire form (the
    /// frontend holds the C6-minted id, §0.4.4), mirroring the `c6_contract` helper.
    /// [Test-Change: P3.52 — old-obsolete+new-correct, §0.4.1] this helper is retained; the P2.30 block_on shell
    /// contract test + the P2.137 shell-body source-scan it fed are superseded by the token-trip chain below.
    fn run_id() -> RunId {
        serde_json::from_str(r#""88888888-8888-4888-8888-888888888888""#)
            .expect("RunId deserializes from a uuid string")
    }

    // [Test-Change: P3.52 — old-obsolete+new-correct, §0.4.1] this end-to-end token-trip chain REPLACES the
    // obsolete P2.30 `block_on(cancel_run)` shell contract test (the shell is gone — the handler is now
    // AppHandle-coupled, uninvocable under cargo test): STRICTLY STRONGER (it asserts the real trip, not Ok(())).
    // §6.4.1 unit (G15): the END-TO-END C7-tripped→observed chain (P3.52), proven at the registry/token level
    // with NO Tauri runtime — the C13 `c13_contract` mirror. The C6 conductor registered the run's token at
    // dispatch (RunRegistry::register, P2.42/P3.48); a C7 cancel trips it (`RunRegistry::cancel` — the wiring
    // this handler adds); the in-flight conversion's §1.7/P3.44 cooperative-cancel poll then reads the tripped
    // token and winds the run down (already-finished items kept, in-progress discarded cleanly, §2.1/§2.6). An
    // unknown/finished run trips nothing (cancel returns false) — the idempotent §0.4.1 no-op.
    #[test]
    fn c7_cancel_trips_the_run_token_so_the_in_flight_conversion_observes_it() {
        // [Test-Change: P3.52 — old-obsolete+new-correct, §0.4.1] this real token-trip chain supersedes the
        // obsolete P2.30 `block_on(cancel_run)` shell assert_eq (the shell is gone — the handler is AppHandle-coupled).
        let registry = RunRegistry::default();
        // The C6 conductor registered the run's cancellation token at dispatch (P2.42/P3.48):
        let token = registry.register(run_id());
        // C7 cancel_run trips it (the wiring P3.52 adds: app.state::<RunRegistry>().cancel):
        assert!(
            registry.cancel(run_id()),
            "§0.4.4: C7 finds the in-flight run's token and trips it"
        );
        // The in-flight conversion's cooperative-cancel poll (§1.7/P3.44) now reads the tripped token and winds
        // the run down (kept-finished / discarded-in-progress). Cancellation-token clones share state, so the
        // token the conductor holds observes the trip.
        assert!(
            token.is_cancelled(),
            "§1.7: the in-flight conversion observes the C7 trip (its cooperative-cancel poll reads the token)"
        );
        // Idempotent: cancelling a finished/unknown run finds no live token — the §0.4.1 no-op.
        registry.finish(run_id());
        assert!(
            !registry.cancel(run_id()),
            "§0.4.1: a cancel of a finished/unknown run trips nothing (RunRegistry::cancel returns false) — the idempotent no-op"
        );
    }

    /// The production prefix of `conversion.rs` (everything before the FIRST `#[cfg(test)]`), so a needle
    /// declared in this test can never self-match — the per-module copy of the system.rs `c10_contract` helper.
    fn production_conversion_source() -> &'static str {
        let full = include_str!("conversion.rs");
        full.split_once(concat!("#[cfg", "(test)]"))
            .map_or(full, |(prefix, _)| prefix)
    }

    // [Test-Change: P3.52 — old-obsolete+new-correct, §0.4.1] this handler source-scan REPLACES the obsolete
    // P2.137 `c7_shell_body_binds_run_id` scan (the shell body it pinned is gone), exactly as that test's own
    // comment scheduled ("superseded by a behavioural token-tripped check at the fill-box").
    // §6.4.1 unit (G15): the C7 handler binds an AppHandle and TRIPS the token via `RunRegistry::cancel` (the
    // P3.52 wiring), no longer the P2.30 `let _ = run_id; Ok(())` shell. Source-scan (AppHandle boot-glue, not
    // cargo-test-runnable; the cancel LOGIC is unit-tested on `RunRegistry` at P2.42 + the chain above). Needles
    // `concat!`-assembled (self-match avoidance); the literal call forms appear only in the handler body.
    #[test]
    fn c7_handler_trips_the_run_token_via_the_registry() {
        // [Test-Change: P3.52 — old-obsolete+new-correct, §0.4.1] this wired-handler scan supersedes the
        // obsolete P2.137 `c7_shell_body_binds_run_id` shell-body scan (the `let _ = run_id; Ok(())` shell is gone).
        let src = production_conversion_source();
        for needle in [
            concat!("cancel_run(app: App", "Handle"),
            concat!("state::<Run", "Registry>().cancel(run_id)"),
        ] {
            assert!(
                src.contains(needle),
                "§0.4.4/§1.1: cancel_run must bind an AppHandle and trip the token via RunRegistry::cancel (missing `{needle}`)"
            );
        }
    }
}

#[cfg(test)]
mod c8_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C8 `get_run_summary` BODY (P3.50 — replacing the P2.31 shell). C8 is the
    //! idempotent §1.12 re-fetch: the pure [`resolve_run_summary`] re-serves the retained `RunResult` from the
    //! §0.4.4 `RunResultStore` (P2.43), mapping the `Option` onto the §0.4.3 `IpcError`. This crate ships no
    //! `tauri::test` mock BY DECISION (the C1/C13 pattern), so the mapping is unit-tested DIRECTLY over a real
    //! `RunResultStore`, and the handler's `State<RunResultStore>` injection is SOURCE-SCAN-pinned (the
    //! `main()`-registers-the-store scan lives in `boot_invariants`, beside the sibling `RunRegistry` scan).
    //! [Build-Session-Entscheidung: P3.50]
    //!
    //! [Test-Change: P3.50 — old-obsolete+new-correct, §0.4.1] this module's P2.31 shell contract test (which
    //! asserted the shell's genuine pre-retention `Err(InternalError)`) is SUPERSEDED here: the shell it
    //! exercised no longer exists (P3.50 built the real body), so its expectation is obsolete; the new tests are
    //! STRICTLY STRONGER — the InternalError leg is retained (an empty/mismatched store) and JOINED by the
    //! retained-run re-serve leg, verified against real `RunResultStore` semantics (`retain` → `get`). No
    //! assertion is weakened; the shell→body supersession is the same tagged transition as the sibling C6
    //! contract module. (Comment cites the MODULE, never a deleted `#[test] fn` name — the G73 rs-test-refs
    //! module-anchor discipline.)
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::orchestrator::{RunResultPaths, Totals};

    /// A `RunId` for the resolve tests — minted through its PUBLIC bare-uuid `Deserialize` wire form (the
    /// frontend holds the C6-minted id, §0.4.4), mirroring the `c6_contract`/`c7_contract` helpers.
    fn run_id() -> RunId {
        serde_json::from_str(r#""99999999-9999-4999-8999-999999999999""#)
            .expect("RunId deserializes from a uuid string")
    }
    /// A second, DISTINCT `RunId` — for the §0.4.4 "a non-matching id never serves the wrong summary" leg.
    fn other_run_id() -> RunId {
        serde_json::from_str(r#""88888888-8888-4888-8888-888888888888""#)
            .expect("RunId deserializes from a uuid string")
    }
    fn collected_set_id() -> CollectedSetId {
        serde_json::from_str(r#""99999999-9999-4999-8999-999999999998""#)
            .expect("CollectedSetId deserializes from a uuid string")
    }
    /// A minimal terminal `RunResult` + its off-wire `RunResultPaths` for the given run — enough to exercise
    /// the C8 re-serve (the §1.12 PROJECTION content is tested in `crate::orchestrator`; here we test the
    /// store re-serve + the Option → IpcError mapping).
    fn a_retained_run(run: RunId) -> (RunResult, RunResultPaths) {
        let result = RunResult {
            collected_set_id: collected_set_id(),
            run_id: run,
            items: vec![],
            totals: Totals {
                succeeded: 0,
                failed: 0,
                cancelled: 0,
                skipped: 0,
            },
            cleanup_incomplete: vec![],
            common_root_display: "root".to_string(),
            divert_root_display: None,
        };
        let paths = RunResultPaths {
            common_root: PathBuf::from("root"),
            divert_root: None,
            item_outputs: BTreeMap::new(),
            item_residues: BTreeMap::new(),
        };
        (result, paths)
    }

    // §6.4.1 unit (G15): the C8 resolve maps the §0.4.4 store `Option` onto the §0.4.3 `IpcError` — an empty
    // store (no run retained) yields the honest `Err(InternalError)` (the same shell outcome for an
    // unresolvable id), a retained run re-serves its `RunResult` verbatim (the idempotent §1.12 re-fetch), and
    // a NON-matching run_id never serves the wrong summary (→ Err). Tested over a real `RunResultStore`, no mock.
    #[test]
    fn c8_resolve_re_serves_the_retained_summary_else_internal_error() {
        let store = RunResultStore::default();
        // No retained run → the honest not-available result.
        assert_eq!(
            resolve_run_summary(&store, run_id())
                .expect_err("an empty store has no summary to re-serve")
                .kind,
            ConversionErrorKind::InternalError,
            "§2.13: an unresolvable run (nothing retained) is the InternalError catch-all"
        );
        // Retain a terminal summary → C8 re-serves it verbatim (idempotent, §0.4.1/§1.12).
        let (result, paths) = a_retained_run(run_id());
        store.retain(result.clone(), paths);
        assert_eq!(
            resolve_run_summary(&store, run_id()),
            Ok(result),
            "§0.4.1/§1.12: C8 re-serves the retained RunResult for its run_id (the idempotent re-fetch)"
        );
        // A DIFFERENT run's id never serves the wrong summary (a superseded / other run → Err).
        assert_eq!(
            resolve_run_summary(&store, other_run_id())
                .expect_err("a non-matching run_id resolves nothing")
                .kind,
            ConversionErrorKind::InternalError,
            "§0.4.4: a non-matching run_id resolves to the not-available result, never the wrong summary"
        );
    }

    // §6.4.1 unit (G15): the C8 handler WIRING is source-scan-pinned (the C1/C13 no-mock pattern) — it injects
    // the managed `State<RunResultStore>` and delegates to the pure `resolve_run_summary`, so it is the real
    // re-serve, NOT the P2.31 `Err`-shell. Needles `concat!`-split to avoid the scan matching its own text.
    #[test]
    fn c8_handler_injects_the_store_and_delegates_to_the_pure_resolve() {
        let src = include_str!("conversion.rs");
        assert!(
            src.contains(concat!("store: State<'_, Run", "ResultStore>")),
            "§0.4.4: the C8 handler injects the managed State<RunResultStore> (P3.50)"
        );
        assert!(
            src.contains(concat!("resolve_run_summary(store.", "inner(), run_id)")),
            "§0.4.1/§0.7: the C8 handler is THIN — it delegates to the pure resolve_run_summary"
        );
    }
}
