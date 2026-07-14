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

use crate::domain::{
    CollectedSetId, DestinationChoice, InstanceId, OptionValues, TargetId, TargetOffer,
};
use crate::engines::slice_target;
use crate::orchestrator::{
    plan_output_preview, CollectedSetRegistry, DestinationResolved, EquivKeyComputer,
    OutputPlanPreview,
};
use crate::outcome::{ConversionErrorKind, IpcError};
use crate::run::RerunLedger;

/// **C2b `pick_destination`** (§0.4.1) — the Rust-side `DialogExt` destination-folder picker. This box (P2.24)
/// authors the typed §0.4.1 wire CONTRACT — the `{} -> Result<Option<PathBuf>, IpcError>` door — so the
/// generated `bindings.ts` carries the C2b surface. Unlike the C2a intake picker, the **one chosen `PathBuf` it
/// returns legitimately transits the WebView** into C5 `set_destination` (and then C6): it is a *write*
/// destination, not a source path, so it can never harm an original or read anything (§0.10 / §2.1 / §0.11 T2).
/// `Ok(None)` = the user cancelled — a clean no-op; the held C4/C5 destination is unchanged.
///
/// [Build-Session-Entscheidung: P2.24] **`Result<Option<PathBuf>, IpcError>` return — the §0.4 universal
/// error-shape rule.** §0.4 "Error shape" is categorical: *every* command returns `Result<T, IpcError>`. The
/// §0.4.1 table's `Option<PathBuf>` output column is the SUCCESS type `T`, wrapped in `Result<T, IpcError>` at
/// the handler — exactly as C1's `CollectedSet` column maps to `Result<CollectedSet, IpcError>`. So the three
/// boundary outcomes are: `Ok(Some(path))` = the user picked a folder; `Ok(None)` = the user cancelled (a clean
/// no-op, the §5.4 cancelled-picker result); `Err(IpcError)` = the native dialog subsystem genuinely failed (a
/// folder pick has no *user-facing* failure, but the boundary still honours the universal Result shape rather
/// than panicking across it, §0.4 "No command ever panics across the boundary"). The wire/TS callsite is
/// unchanged (`Result<T, E>` renders as `__TAURI_INVOKE<T>` + a thrown `IpcError`, like C1).
///
/// [Build-Session-Entscheidung: P2.24] **Interface-shell body — the typed CONTRACT is the deliverable.**
/// P2.24 authors the §0.4.1 wire signature; the native `DialogExt` folder-pick BODY (`app.dialog().file()
/// .pick_folder(..)`, opened async/`spawn_blocking` so it never blocks a Tokio worker, §7 app-shell) is wired
/// end-to-end at P3.56 — the DestinationBar, whose "Change destination" affordance drives C2b → C5 (P3.54 wires
/// the C2a *intake* picker, a distinct path; C2b is the *destination* picker). A native OS folder dialog is
/// **not unit-testable** (it needs a real OS dialog /
/// user interaction — the §6.6 walkthrough + the P9 E2E flow exercise it), so the testable P2 deliverable is
/// the typed contract; the shell returns `Ok(None)` — the genuine cancelled/no-pick result. This is the
/// sanctioned compile-time interface-shell pattern (CLAUDE §5 / the P3 `crate::isolation` shells P4 expands),
/// not a quiet deferral.
#[tauri::command]
#[specta::specta]
pub async fn pick_destination() -> Result<Option<PathBuf>, IpcError> {
    Ok(None)
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
/// reach the §0.4.4 `State<CollectedSetRegistry>` + the §2.5 `State<EquivKeyComputer>` / `State<RerunLedger>`
/// + the app `State<InstanceId>`, and dispatches to the AppHandle-free `resolve_output_plan` helper, which
/// resolves the set and delegates the §1.8 batch preview to `orchestrator::plan_output_preview`: the
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
    // (infallible — all four stores are `.manage()`d). A `JoinError` (the probe thread panicked —
    // should-never-happen under the in-core no-panic policy) surfaces as an InternalError, never a silent value.
    // [Build-Session-Entscheidung: P3.49]
    match tauri::async_runtime::spawn_blocking(move || {
        let sets = app.state::<CollectedSetRegistry>();
        let computer = app.state::<EquivKeyComputer>();
        let ledger = app.state::<RerunLedger>();
        let instance = *app.state::<InstanceId>();
        resolve_output_plan(
            &sets,
            &computer,
            &ledger,
            instance,
            collected_set_id,
            target,
            &options,
            &destination,
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
/// [Build-Session-Entscheidung: P2.27] **Shell returns `Err(IpcError{ kind: InternalError })` — the same
/// owner-approved interface-shell pattern as C3/C4.** `DestinationResolved` has no zero value (it carries a
/// re-evaluated `PreflightVerdict`), so there is no `Ok(empty)`; the genuine pre-registry outcome (the §0.4.4
/// registry, P2.44, does not exist) is the `Err` the real body returns for an unresolvable id: `Err(IpcError{
/// kind: ConversionErrorKind::InternalError, … })` (§2.13 catch-all; the §3.2 `PlanError` precedent). The named
/// fill-boxes own the rest: (a) the §2.8 catalog box owns the FINAL message (the string below is provisional) +
/// must add a COMMAND-level string (the §2.8 catalog is item-scoped); (b) the §0.4.4 registry resolve + the
/// §1.8/§2.14.4 destination-change re-validation (re-eval pre-flight, carry `rerun` through) + the §0.6 SUCCESS
/// path belong to the body box (P2.44+) — the C4/C5 lifecycle asymmetry (C4 re-callable; C5 owns the
/// destination; C4 never overrides C5) is enforced by P2.28; (c) `kind` is the CONCRETE `ConversionErrorKind`,
/// not the `ErrorKind` alias (the P2.19 convention).
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn set_destination(
    collected_set_id: CollectedSetId,
    target: TargetId,
    options: OptionValues,
    destination: DestinationChoice,
) -> Result<DestinationResolved, IpcError> {
    let _ = (collected_set_id, target, options, destination);
    Err(IpcError {
        kind: ConversionErrorKind::InternalError,
        message: "Could not update the destination.".into(),
        path_display: None,
        residue_display: None,
    })
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
    destination: &DestinationChoice,
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
    //! §6.4.1 unit (G15): the §0.4.1 C2b `pick_destination` typed CONTRACT (P2.24). Mirrors the C1/C2a
    //! `*_contract` tests — the handler now carries its typed `-> Result<Option<PathBuf>, IpcError>` signature
    //! (the §0.4 universal error shape), so the P2.21 all-shells `block_on(pick_destination())` invocation in
    //! `crate::ipc` (mod.rs) is REPLACED here by C2b's own typed-contract test (the fill-box transition the
    //! P2.21 note schedules). The native folder-dialog body is not unit-testable (it needs a real OS dialog) and
    //! lands at P3.56 (the DestinationBar "Change destination" path); this asserts the typed contract returns
    //! the cancelled/no-pick `Ok(None)`. [Build-Session-Entscheidung: P2.24]
    use super::*;
    use tauri::async_runtime::block_on;

    // §6.4.1 unit (G15): the C2b contract is invocable and returns `Result<Option<PathBuf>, IpcError>` (the wire
    // door this box authors, in the §0.4 universal error shape). The shell opens no dialog yet (the DialogExt
    // body is P3.56, the DestinationBar "Change destination" path), so it returns `Ok(None)` — which is ALSO the
    // contract's genuine cancelled-dialog result (§0.4.1: `Ok(None)` = the user cancelled); P3.56 replaces it
    // with the real folder pick whose `Ok(Some(path))` carries into C5, and an `Err(IpcError)` for a genuine
    // dialog-subsystem failure.
    #[test]
    fn c2b_pick_destination_contract_is_invocable_and_typed() {
        let out: Result<Option<PathBuf>, IpcError> = block_on(pick_destination());
        assert_eq!(
            out,
            Ok(None),
            "§0.4.1/§0.4: the C2b contract shell opens no dialog yet (the DialogExt body is P3.56), so it \
             returns Ok(None) — also the §5.4 cancelled-pick result; the typed Result<Option<PathBuf>, \
             IpcError> signature (the §0.4 universal error shape) is the P2.24 deliverable"
        );
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
            &DestinationChoice::BesideSource,
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
            &DestinationChoice::ChosenRoot(chosen_dir.path().to_path_buf()),
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
            &DestinationChoice::BesideSource,
        )
        .expect_err("§2.13: an unresolvable set id yields the not-available Err");
        assert_eq!(
            err.kind,
            ConversionErrorKind::InternalError,
            "§2.13: the unresolvable-set outcome is the InternalError catch-all"
        );
    }

    // §6.4.1 unit (G15): the C4 handler is AppHandle-coupled boot-glue (§1.1a; G28-exempt) — a source-scan pins
    // it binds an `AppHandle`, resolves the four States (the `state::<InstanceId>()` needle is call-specific),
    // and DISPATCHES via `resolve_output_plan`. Needles `concat!`-assembled (self-match avoidance).
    #[test]
    fn plan_output_handler_binds_apphandle_and_dispatches_via_the_helper() {
        let src = production_planning_source();
        for needle in [
            concat!("pub async fn plan_", "output("),
            concat!("app: App", "Handle"),
            concat!("spawn_", "blocking(move"),
            concat!("state::<CollectedSet", "Registry>()"),
            concat!("state::<EquivKey", "Computer>()"),
            concat!("state::<Rerun", "Ledger>()"),
            concat!("state::<Instance", "Id>()"),
            concat!("resolve_output_", "plan("),
        ] {
            assert!(
                src.contains(needle),
                "§0.4.1/§1.8: the C4 plan_output handler must bind an AppHandle, resolve the four States, and \
                 dispatch via resolve_output_plan (missing `{needle}`)"
            );
        }
    }
}

#[cfg(test)]
mod c5_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C5 `set_destination` typed CONTRACT (P2.27) — same interface-shell pattern
    //! as C3/C4: the handler carries its typed `{ collectedSetId, target, options, destination } ->
    //! Result<DestinationResolved, IpcError>` signature (the SAME request payload as C4), so the P2.21
    //! all-shells `block_on(set_destination())` invocation in `crate::ipc` (mod.rs) moves here. The shell
    //! returns the genuine pre-registry `Err(InternalError)`; SHAPE is asserted, NOT the provisional message
    //! (owned by the §2.8 catalog box). [Build-Session-Entscheidung: P2.27]
    use super::*;
    use tauri::async_runtime::block_on;

    fn collected_set_id() -> CollectedSetId {
        serde_json::from_str(r#""66666666-6666-4666-8666-666666666666""#)
            .expect("CollectedSetId deserializes from a uuid string")
    }

    // §6.4.1 unit (G15): the C5 contract is invocable with its full §0.4.1 typed arg set ({ collectedSetId,
    // target, options, destination }) and returns a `Result<DestinationResolved, IpcError>` (the §0.4 universal
    // error shape). The shell has no §0.4.4 registry (P2.44), so it returns the genuine pre-registry
    // `Err(InternalError)`. SHAPE asserted (kind == InternalError), NOT the provisional message (owned by the
    // §2.8 catalog box); P2.44+ replaces the shell with the real §1.8/§2.14.4 destination re-validation.
    #[test]
    fn c5_set_destination_contract_is_invocable_and_typed() {
        use crate::domain::FormatId;
        use std::collections::BTreeMap;
        let out = block_on(set_destination(
            collected_set_id(),
            TargetId::Format(FormatId::Png),
            OptionValues(BTreeMap::new()),
            DestinationChoice::BesideSource,
        ));
        let err = out.expect_err(
            "§0.4.1/§0.4: the C5 shell has no registry (P2.44), so it returns the genuine pre-registry \
             Err(InternalError); the typed Result<DestinationResolved, IpcError> signature is the P2.27 deliverable",
        );
        assert_eq!(
            err.kind,
            ConversionErrorKind::InternalError,
            "§2.13: the unresolvable-set shell outcome is the InternalError catch-all — SHAPE asserted, NOT \
             the provisional message (the §2.8 catalog box owns the final string)"
        );
    }
}
