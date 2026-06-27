//! `crate::ipc::conversion` — the §0.4.1 run-lifecycle command group (C6 / C7 / C8): start a run, cancel a
//! run, and re-fetch a run summary. P2.21 registered these as the §0.4.1 command-surface interface shells;
//! C6 `start_conversion`'s typed request/response CONTRACT is authored by P2.29 (this file), C7's by P2.30,
//! and C8's by P2.31. Each command's `crate::orchestrator` delegation BODY is its own named fill-box (the C6
//! queue build / §1.9 run lifecycle / §0.9 worker spawn / `ConversionEvent` emission is P3.46). Thin by
//! design (§0.7): the handler validates, delegates, and maps the `Result` onto the §0.4.3 `IpcError`.

// §0.4 / T10: unchecked arithmetic on an untrusted wire field must be a compile error in every IPC handler
// (the `crate::ipc` arithmetic-overflow deny cascades here; restated at the T10 boundary). The C6 contract
// handler below + the remaining C7/C8 shells do no arithmetic; the deny bites the fill-bodies (the C6 queue
// body P3.46, C7 P2.30, C8 P2.31).
#![deny(clippy::arithmetic_side_effects)]

use tauri::ipc::Channel;

use crate::domain::{
    CollectedSetId, DestinationChoice, OptionValues, RerunDecision, RunId, TargetId,
};
use crate::orchestrator::{ConversionEvent, RunResult};
use crate::outcome::{ConversionErrorKind, IpcError};

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
/// - `destination` — **AUTHORITATIVE** (§0.4.1 C6 `[DECIDED]`): C4/C5 are plan/preview + revalidation only,
///   there is no separate server-side destination store — the value the UI passes here (the last C5-resolved
///   destination) is what the run writes to.
/// - `rerun_decision` — the §0.6 `RerunDecision` (the user's answer to a C4 `RerunPrompt`, §2.5): `Skip` (the
///   safe default — no new output for equivalent items) or `FreshCopy` (fresh numbered copies).
/// - `on_progress` — the run-telemetry `Channel<ConversionEvent>` (§0.4.2): ordered, run-scoped (dies with
///   the run — no cross-run leak, §1.11). Like C1's `onScan` it is **non-optional** (tauri's `Channel<T>` is
///   `!Deserialize`, so `Option<Channel<T>>` cannot be a command arg — the same forced shape the C1
///   `ingest_paths` handler documents; the §0.4.1 C6 row already specifies it non-optional).
///
/// [Build-Session-Entscheidung: P2.29] **Shell returns `Err(IpcError{ kind: InternalError })` — the same
/// owner-approved interface-shell pattern as C3/C4/C5 (P2.25/P2.26/P2.27).** `RunId` is a non-nil v4-UUID
/// newtype with no zero value (and no public constructor), so unlike C1/C2a (`CollectedSet::Empty`) / C2b
/// (`Ok(None)`) there is no `Ok(empty)` to return — and fabricating an `Ok(RunId)` would be a LIE (it would
/// claim a run started when nothing was enqueued and no `ConversionEvent` will ever fire on the Channel),
/// exactly the fabricated handling the interface-shell pattern forbids (CLAUDE §5). Until the §0.4.4
/// collected-set registry (P2.44) + the §1.9 queue / §0.9 workers land (P3.46), **no** `collectedSetId`
/// resolves — so the shell's honest result is exactly the `Err` the real body returns for an unresolvable id:
/// `Err(IpcError{ kind: ConversionErrorKind::InternalError, … })` (§2.13 catch-all; the §3.2 `PlanError`
/// `plan_encode` precedent C3/C4/C5 cite). The named fill-boxes own the rest: (a) the §2.8 catalog box owns
/// the FINAL message — the string below is a PROVISIONAL neutral English one — and must add a COMMAND-level
/// string (the §2.8 catalog is item-scoped); (b) the §0.4.4 registry resolve + the §1.9 queue build / §0.9
/// worker spawn / `ConversionEvent` emission + the §0.6 SUCCESS path (the minted `RunId`) belong to the body
/// box (P3.46), and the RunId mint-point (at C6 accept, NOT the §2.4 freeze — §7.1.2) is fixed by P2.48;
/// (c) `kind` is the CONCRETE `ConversionErrorKind`, not the `ErrorKind` alias (the P2.19 convention).
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn start_conversion(
    collected_set_id: CollectedSetId,
    target: TargetId,
    options: OptionValues,
    destination: DestinationChoice,
    rerun_decision: RerunDecision,
    on_progress: Channel<ConversionEvent>,
) -> Result<RunId, IpcError> {
    let _ = (
        collected_set_id,
        target,
        options,
        destination,
        rerun_decision,
        on_progress,
    );
    Err(IpcError {
        kind: ConversionErrorKind::InternalError,
        message: "Could not start the conversion.".into(),
        path: None,
        residue: None,
    })
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
/// [Build-Session-Entscheidung: P2.30] **Shell returns `Ok(())` — the genuine no-op-cancel outcome, the
/// C1/C2a (`CollectedSet::Empty`) / C2b (`Ok(None)`) "zero-valued result" branch of the interface-shell
/// pattern, NOT the C3/C4/C5/C6 `Err(InternalError)` branch.** The split is principled: C3/C4/C5/C6 return
/// `Err` because their success type (`TargetOffer`/`OutputPlanPreview`/`DestinationResolved`/`RunId`) has **no
/// zero value**, so a pre-registry shell cannot honestly produce one. C7's success type is `()`, which **does**
/// have a zero value, and `cancel_run` is an **idempotent fire-and-forget side-effect**: it trips a token and
/// returns — `tokio_util` `CancellationToken::cancel()` on an unheld/already-cancelled token is a harmless
/// no-op, and a cancel of an already-finished run is the desired end-state ("not running" ⇒ effectively
/// cancelled, §0.4.4). So tripping *no* token (the shell has no run registry — P2.42 — yet) is genuinely
/// `Ok(())`, NOT a fabricated success: it claims nothing positive happened (unlike a fabricated C6 `Ok(RunId)`,
/// which would lie that a run started). The kill *outcome* is never C7's return — it is reported async via the
/// `ConversionEvent`/`RunResult` (§1.7/§1.12). The real registry resolve + `.cancel()` wiring lands at P2.42
/// (the `RunId` token registry) / P3.52 (the C7 cancel-wiring to the §1.1/P3.44 cooperative cancel); the
/// contract is unchanged by it (cancel stays `Ok(())`).
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn cancel_run(run_id: RunId) -> Result<(), IpcError> {
    let _ = run_id;
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
/// [Build-Session-Entscheidung: P2.31] **Shell returns `Err(IpcError{ kind: InternalError })` — the C3/C4/C5/
/// C6 interface-shell pattern (success type has no zero value), NOT the C7 `Ok(())` no-op branch.** `RunResult`
/// carries a real summary (items / totals / roots) and has **no zero value**, so — like `TargetOffer`/
/// `OutputPlanPreview`/`DestinationResolved`/`RunId`, unlike C7's `()` — there is no `Ok(empty)` to return, and
/// fabricating one would invent a summary for a run that never happened (CLAUDE §5). Until the §0.4.4
/// `RunResult` retention registry (P2.43) holds a terminal result, **no** `runId` resolves — so the shell's
/// honest result is exactly the `Err` the real body returns for an unresolvable / not-yet-finished id:
/// `Err(IpcError{ kind: ConversionErrorKind::InternalError, … })` (§2.13 catch-all; the §3.2 `PlanError`
/// precedent C3/C4/C5 cite). The named fill-boxes own the rest: (a) the §2.8 catalog box owns the FINAL message
/// — the string below is a PROVISIONAL neutral English one — and must add a COMMAND-level string (the §2.8
/// catalog is item-scoped); (b) the §0.4.4 retention resolve + the §1.12 `RunResult` projection (incl. the
/// pre-flight-skip projection + the batch-summary string) + the §0.6 SUCCESS path belong to the body box P3.50;
/// (c) `kind` is the CONCRETE `ConversionErrorKind`, not the `ErrorKind` alias (the P2.19 convention).
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn get_run_summary(run_id: RunId) -> Result<RunResult, IpcError> {
    let _ = run_id;
    Err(IpcError {
        kind: ConversionErrorKind::InternalError,
        message: "Could not retrieve the conversion summary.".into(),
        path: None,
        residue: None,
    })
}

#[cfg(test)]
mod c6_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C6 `start_conversion` typed CONTRACT (P2.29) — same interface-shell
    //! pattern as C3/C4/C5: the handler carries its typed `{ collectedSetId, target, options, destination,
    //! rerunDecision, onProgress } -> Result<RunId, IpcError>` signature, so the P2.21 all-shells
    //! `block_on(start_conversion())` invocation in `crate::ipc` (mod.rs) is REPLACED here by C6's own
    //! typed-contract test (the fill-box transition the P2.21 note schedules). The shell returns the genuine
    //! pre-registry `Err(InternalError)`; SHAPE is asserted, NOT the provisional message (owned by the §2.8
    //! catalog box). The §0.4.4 registry resolve + the §1.9 queue build / §0.9 worker spawn / `ConversionEvent`
    //! emission + the minted `RunId` SUCCESS path land at P3.46. [Build-Session-Entscheidung: P2.29]
    use super::*;
    use tauri::async_runtime::block_on;

    /// A `CollectedSetId` for the contract call — minted through its PUBLIC bare-uuid `Deserialize` wire form
    /// (the frontend mints the id, §0.4.4), mirroring the `c3_contract`/`c4_contract`/`c5_contract` helpers.
    fn collected_set_id() -> CollectedSetId {
        serde_json::from_str(r#""77777777-7777-4777-8777-777777777777""#)
            .expect("CollectedSetId deserializes from a uuid string")
    }

    // §6.4.1 unit (G15): the C6 contract is invocable with its full §0.4.1 typed arg set ({ collectedSetId,
    // target, options, destination, rerunDecision, the non-optional onProgress Channel }) and returns a
    // `Result<RunId, IpcError>` (the §0.4 universal error shape). `on_progress` is a real
    // `Channel::new(|_| Ok(()))` — the non-optional contract (there is no `None` arm; the same `Channel<T>`
    // `!Deserialize` forced shape C1 documents). The shell has no §0.4.4 registry / §1.9 queue yet (P2.44 /
    // P3.46), so it returns the genuine pre-registry `Err(InternalError)`. SHAPE asserted (kind ==
    // InternalError), NOT the provisional message (owned by the §2.8 catalog box); P3.46 replaces the shell
    // with the real resolve → queue build → minted RunId.
    #[test]
    fn c6_start_conversion_contract_is_invocable_and_typed() {
        use crate::domain::FormatId;
        use std::collections::BTreeMap;
        let out = block_on(start_conversion(
            collected_set_id(),
            TargetId::Format(FormatId::Png),
            OptionValues(BTreeMap::new()),
            DestinationChoice::BesideSource,
            RerunDecision::Skip,
            Channel::new(|_| Ok(())),
        ));
        let err = out.expect_err(
            "§0.4.1/§0.4: the C6 shell has no registry/queue yet (P2.44/P3.46), so it returns the genuine \
             pre-registry Err(InternalError); the typed Result<RunId, IpcError> signature is the P2.29 deliverable",
        );
        assert_eq!(
            err.kind,
            ConversionErrorKind::InternalError,
            "§2.13: the unresolvable-set shell outcome is the InternalError catch-all — SHAPE asserted, NOT \
             the provisional message (the §2.8 catalog box owns the final string)"
        );
    }
}

#[cfg(test)]
mod c7_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C7 `cancel_run` typed CONTRACT (P2.30). The handler now carries its typed
    //! `{ runId } -> Result<(), IpcError>` signature, so the P2.21 all-shells `block_on(cancel_run())`
    //! invocation in `crate::ipc` (mod.rs) is REPLACED here by C7's own typed-contract test (the fill-box
    //! transition the P2.21 note schedules). The shell returns the genuine idempotent no-op-cancel `Ok(())`;
    //! the §0.4.4 token registry resolve + `.cancel()` land at P2.42 / P3.46. [Build-Session-Entscheidung: P2.30]
    use super::*;
    use tauri::async_runtime::block_on;

    /// A `RunId` for the contract call — minted through its PUBLIC bare-uuid `Deserialize` wire form (the
    /// frontend holds the C6-minted id, §0.4.4), mirroring the `c6_contract` helper.
    fn run_id() -> RunId {
        serde_json::from_str(r#""88888888-8888-4888-8888-888888888888""#)
            .expect("RunId deserializes from a uuid string")
    }

    // §6.4.1 unit (G15): the C7 contract is invocable with its §0.4.1 typed `runId` arg and returns a
    // `Result<(), IpcError>` (the §0.4 universal error shape). The shell trips no token yet (no run registry —
    // P2.42), so it returns the genuine idempotent no-op-cancel `Ok(())` (a cancel of a non-existent/finished
    // run is the desired "not running" end-state, §0.4.4); P3.46 wires the real registry resolve + `.cancel()`.
    #[test]
    fn c7_cancel_run_contract_is_invocable_and_typed() {
        let out = block_on(cancel_run(run_id()));
        assert_eq!(
            out,
            Ok(()),
            "§0.4.1/§0.4: the C7 contract shell trips no token yet (the §0.4.4 registry is P2.42), so it \
             returns the genuine idempotent no-op-cancel Ok(()); the typed Result<(), IpcError> signature is \
             the P2.30 deliverable"
        );
    }
}

#[cfg(test)]
mod c8_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C8 `get_run_summary` typed CONTRACT (P2.31) — same interface-shell pattern
    //! as C3/C4/C5/C6: the handler carries its typed `{ runId } -> Result<RunResult, IpcError>` signature, so
    //! the P2.21 all-shells `block_on(get_run_summary())` invocation in `crate::ipc` (mod.rs) is REPLACED here
    //! by C8's own typed-contract test. The shell returns the genuine pre-retention `Err(InternalError)`; SHAPE
    //! is asserted, NOT the provisional message (owned by the §2.8 catalog box). The §0.4.4 retention resolve +
    //! the §1.12 RunResult projection land at P2.43 / P3.50. [Build-Session-Entscheidung: P2.31]
    use super::*;
    use tauri::async_runtime::block_on;

    /// A `RunId` for the contract call — minted through its PUBLIC bare-uuid `Deserialize` wire form (the
    /// frontend holds the C6-minted id, §0.4.4), mirroring the `c6_contract`/`c7_contract` helpers.
    fn run_id() -> RunId {
        serde_json::from_str(r#""99999999-9999-4999-8999-999999999999""#)
            .expect("RunId deserializes from a uuid string")
    }

    // §6.4.1 unit (G15): the C8 contract is invocable with its §0.4.1 typed `runId` arg and returns a
    // `Result<RunResult, IpcError>` (the §0.4 universal error shape). The shell has no §0.4.4 retention registry
    // yet (P2.43), so it returns the genuine pre-retention `Err(InternalError)` — the same Err the real body
    // returns for an unresolvable / not-yet-finished id. SHAPE asserted (kind == InternalError), NOT the
    // provisional message (owned by the §2.8 catalog box); P3.50 replaces the shell with the real resolve →
    // §1.12 RunResult projection.
    #[test]
    fn c8_get_run_summary_contract_is_invocable_and_typed() {
        let out = block_on(get_run_summary(run_id()));
        let err = out.expect_err(
            "§0.4.1/§0.4: the C8 shell has no retention registry yet (P2.43), so it returns the genuine \
             pre-retention Err(InternalError); the typed Result<RunResult, IpcError> signature is the P2.31 deliverable",
        );
        assert_eq!(
            err.kind,
            ConversionErrorKind::InternalError,
            "§2.13: the unresolvable-run shell outcome is the InternalError catch-all — SHAPE asserted, NOT \
             the provisional message (the §2.8 catalog box owns the final string)"
        );
    }
}
