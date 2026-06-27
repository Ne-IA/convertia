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
use crate::orchestrator::ConversionEvent;
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

/// **C7 `cancel_run`** (§0.4.1) — trips the §0.4.4 cancellation token for the run (finished items kept, the
/// in-progress item discarded cleanly, §2.1/§2.6). Registered as the §0.4.1 interface shell (P2.21); the
/// full `{ runId } -> ()` contract is authored by P2.30. [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn cancel_run() {}

/// **C8 `get_run_summary`** (§0.4.1) — the idempotent re-fetch of the retained §1.12 `RunResult` (also
/// delivered as the terminal `RunFinished` event), e.g. after a WebView reload. Registered as the §0.4.1
/// interface shell (P2.21); the full `{ runId } -> RunResult` contract is authored by P2.31.
/// [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn get_run_summary() {}

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
