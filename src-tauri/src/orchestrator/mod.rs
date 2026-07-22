//! `crate::orchestrator` — the §1.9 batch / job-lifecycle conductor: it builds the queue, drives
//! `JobState`, holds the run registry + cancellation tokens (§0.4.4), and fans progress out to the
//! Channel. It sequences the guarantees / engines / detection layers; it owns none of their behaviour.
//!
//! The conducting BEHAVIOUR (queue construction at C6, the run-registry register/finish WIRING, the
//! `ConversionEvent` fan-out) is the P3.48 C6 conductor, composing the §1.9 transition primitives P3.46
//! authored (the cancellation-DISPATCH leg is the C7 `cancel_run` handler, wired P3.52). This module homes the §0.6 outcome-referencing lifecycle/result types
//! it assembles — `Batch`/`ConversionJob`/`JobState` (P2.10), the C4/C5 command-return DTOs
//! `PreflightVerdict`/`OutputPlanPreview`/`DestinationResolved` (P2.11), the §1.12 result types
//! `RunResult`/`ItemResult`/`Totals`/`CleanupResidue`/`ItemOutcome` (P2.12), and the §0.4.2 `ConversionEvent`
//! run-telemetry enum + its payloads `RunStarted`/`ItemStarted`/`ItemProgress`/`ItemFinished`/`BatchProgress`
//! (P2.37) — at tier 1, ABOVE the tier-3
//! `crate::domain` leaf, because each references `crate::outcome` (the §2.8 kind / `OutcomeMsg` / `IpcError`)
//! directly or transitively. Homing them here keeps the §0.6 `domain` ↔ `outcome` type cycle broken and
//! `crate::domain` a pure leaf (the §0.7 ‡ note, the owner-decided P2.10 tier-finalisation). The sibling
//! `JobStage` (no outcome ref) stays in `crate::domain`.
//!
//! It also homes the four §0.4.4 orchestrator-State stores (per §0.7, under the §0.7 "(§0.4.4)" umbrella) —
//! distinct from the outcome-referencing types above: the `RunRegistry` (the `RunId` → `CancellationToken`
//! run-cancellation-token store, P2.42; its register-at-C6 + drop-on-`RunFinished` WIRING is the P3.48
//! conductor + handler, its cancel-at-C7 the `cancel_run` handler wired P3.52), its sibling the `RunResultStore` (the
//! process-local terminal-`RunResult` retention for C8 re-serve, P2.43; no on-disk persistence per §7.4, its
//! retain-at-`RunFinished` + evict-at-C6 WIRING the P3.48 conductor + handler, its get-at-C8 the P3.50
//! `get_run_summary`), the `CollectedSetRegistry` (the `CollectedSetId` → `RegisteredSet` resolve
//! store — the domain `FrozenCollectedSet` + the §2.3 identity-evidence table, P2.44 / P3.40; so the
//! bare-`collectedSetId` C3/C4/C5/C6 commands resolve the frozen detection result without
//! a second walk, its take-at-C6 WIRING the P3.48 handler — its register-at-C1/C2a-freeze + resolve-at-C3/C4/C5 the P3.49 ingest/planning bodies), and
//! the `IngestRegistry` (the `CollectingId` → `CancellationToken` ingest-cancellation store, P2.45; the
//! one-phase-earlier sibling of `RunRegistry`, so C13 `cancel_ingest` can trip an in-flight C1/C2a walk — its
//! register-at-handler-entry / cancel-at-C13 / release-on-every-handler-exit WIRING is C1/C2a/C13 + P2.69-71).

// [Build-Session-Entscheidung: P2.10/P2.11/P2.12] dead_code expect — the lifecycle/DTO/result types homed
// here (Batch/ConversionJob/JobState, the C4/C5 DTOs, and the §1.12 RunResult/ItemResult/Totals/
// CleanupResidue/ItemOutcome) were authored as CONTRACTS before their consumers existed: the orchestrator
// queue/lifecycle BEHAVIOUR that constructs and drives them is the P3.48 C6 conductor (composing the §1.9
// FSM primitives P3.46 authored), and the DTO/result wire registration rides the C4/C5/C8 +
// RunFinished/ItemFinished consumers. P3.48 now consumes MOST of them into the live run (the reason string
// below tracks what still stays dead); the cfg(test) tests construct the full graphs, so the TEST build is
// dead-code-clean and needs no expectation. `expect` (not `allow`) auto-flags the moment the LAST dead item
// is consumed — matching `crate::domain` / `crate::outcome`. Scoped to `not(test)` for that same reason.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the §0.6 lifecycle/DTO/result types homed here (Batch/ConversionJob/JobState P2.10, the C4/C5 DTOs P2.11, RunResult/ItemResult/Totals/CleanupResidue/ItemOutcome P2.12, the §0.4.2 ConversionEvent enum + its RunStarted/ItemStarted/ItemProgress/ItemFinished/BatchProgress payloads P2.37, and the four §0.4.4 State stores RunRegistry P2.42 + RunResultStore P2.43 + CollectedSetRegistry P2.44 + IngestRegistry P2.45) were authored as contracts ahead of their wire consumers. The P3.48 C6 run conductor + its start_conversion handler now compose MOST of them into the live run. LIVE via P3.48: the §1.9 FSM advance + queue_order (+ state_is_queued), the P3.47 build_batch, the P3.50 project_run_result, CollectedSetRegistry::take + RunResultStore::{evict, retain} + RunRegistry::{register, finish} + crate::run::RunScratch::acquire, and the P3.39 EquivKeyComputer::compute_equiv_key (the §2.5 applier + the per-success RerunLedger record). Already live before P3.48: RunRegistry::has_active_run (the §7.1.1 converter_is_busy, P2.55) and RunResultStore::get (the C8 get_run_summary handler via resolve_run_summary, P3.50). LIVE via P3.49 (the C1 drain_intake / C3 get_targets / C4 plan_output walking-skeleton wiring): the §1.1/§2.4.1 ingest funnel spine (walk_intake_roots + resolve_and_dedup/dedup_by_identity + freeze_snapshot + the §1.3 group() projection) via the C1 drain, CollectedSetRegistry::{register, resolve} (the C1 freeze register + the C3/C4 resolve), and the P3.40 compute_rerun_verdict (its first production caller — the C4 plan_output_preview re-run verdict). STILL dead in the production build until its own wiring lands: the §2.8 project_outcome (P3.46.2 — the conductor maps its own ItemRunOutcome onto the terminal JobEvent INLINE, so this InvocationResult projection has no production caller yet); any P3.25 §2.6.4 residue helper project_run_result does not reach. LIVE via P3.59 (the 2026-07-16 ruling wiring the §1.12 batch line onto the wire): the §2.8.2 batch-summary renderer batch_summary_line (its FIRST production caller — project_run_result assembles RunResult.summary_line_display from it) + batch_summary/append_residue_tail transitively, and crate::outcome::residue_annotation (the promoted §2.8.2 case-1 row) via residue_item_reason's Succeeded arm. Reading a dead fn does not make it live — a dead-fn reference is not a root — so this pure lifecycle/projection graph stayed dead until the P3.48 conductor made it reachable from the C6 command root; the expectation stays fulfilled while ANY of the above is still unwired."
    )
)]

use std::collections::hash_map::RandomState;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::hash::{BuildHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use tempfile::TempPath;
use tokio_util::sync::CancellationToken;
use walkdir::WalkDir;

use crate::domain::{
    CollectedSet, CollectedSetId, CollectingId, DestinationChoice, DestinationId,
    DestinationPicked, DetectionOutcome, DivertReason, DroppedItem, FrozenCollectedSet,
    InitialDestination, InstanceId, IntakeOrigin, ItemId, ItemIdSpace, ItemPaths,
    ItemSpaceExhausted, JobSource, JobStage, OptionValues, OutputPlan, ReadFailure, RerunDecision,
    RerunPrompt, ResolvedDestination, RunId, ScanProgress, SkipReason, SkippedItem, Target,
    TargetId, UserFacingFormat,
};
use crate::engines::{
    dispatch, Engine, EngineId, EngineInvocation, InvocationResult, NativeCsvTsvEngine, PlanOutcome,
};
use crate::fs_guard::{
    atomic_publish, compute_output_plan, is_write_divert_trigger, location_status,
    open_verified_parent_dir, output_name, publish_to_divert, resolve_divert_target,
    DestinationMode, DivertTarget, FileIdentity, LocationCache, LocationStatus, OutputPlanError,
    ParentDirVerdict, PublishError, PublishOutcome,
};
use crate::outcome::{conversion_failure, ConversionErrorKind, IpcError, OutcomeMsg};
use crate::pool::Pool;
use crate::prefs::LastDestinationMode;
use crate::run::{cleanup_item, cleanup_run, EquivKey, PublishTemp, RerunLedger, RunScratch};

/// One same-source conversion batch (§0.6 / §1.9) — the queue the orchestrator builds at C6
/// `start_conversion` from a frozen `CollectedSet::Single` and drives to a §1.12 summary. INTERNAL to the
/// pipeline: it is assembled and consumed core-side (the WebView sees the §1.12 `RunResult` projection,
/// never the `Batch` itself), so it is NOT a wire type and derives no `serde`/`specta` (mirroring the
/// P2.9 internal `OutputPlan`). The §0.6 invariants it carries BY SHAPE: exactly one whole-batch `target`
/// and one effective `options` (invariants 1+2 — single values, not per-item); a `Batch` exists only from
/// a `CollectedSet::Single` (invariant 3). The per-item enforcement (count == items.len(), frozen set,
/// `item == source.item()`, stable `ItemId`) is property-tested in P2.14.
///
/// [Build-Session-Entscheidung: P2.10] Derive set `Debug, Clone, PartialEq, Eq` — the internal-type set
/// (no `serde`/`specta`, like `OutputPlan`); NOT `Copy` (it owns `Vec`/`String`-bearing fields). `Eq`
/// backs the P2.14 property suite + the construction tests; every field type is `Eq`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch {
    /// The frozen collected-set this batch was built from — `Batch.id` IS the source `CollectedSetId`
    /// (§1.12), tying the run back to its §0.4.4 collected-set registry entry.
    pub id: CollectedSetId,
    /// The single eligible user-facing source format (§1.3 grouping key) — shared by every job.
    pub source_format: UserFacingFormat,
    /// The one chosen target applied to the WHOLE batch (§0.6 invariant 1) — a single `Target`, never a
    /// per-item choice; the single-value field SHAPE is what enforces the invariant.
    pub target: Target,
    /// The one effective, fully-resolved option set for the whole batch (§0.6 invariant 2 / §2.5).
    pub options: OptionValues,
    /// Where the batch's outputs are written (§2.7) — beside-source or a chosen root, **already resolved**
    /// core-side ([`ResolvedDestination`]): C6 `start_run` resolves the wire `DestinationChoice`'s
    /// `ChosenRoot(DestinationId)` against the §0.4.4 `DestinationRegistry` before `build_batch`, so the pure
    /// §1.8/§2.7 legs read a real `PathBuf`, never do a registry lookup (the 2026-07-06 core-owned-paths split).
    pub destination: ResolvedDestination,
    /// The per-item jobs, in the deterministic collected/traversal order (§1.9 queue order). Carries BOTH
    /// the `Pending` eligible jobs AND the pre-flight `Skipped` jobs materialised at construction (§1.9),
    /// over the §0.6 single id space (so a `SkippedItem.item` never collides with an eligible `ItemId`).
    pub jobs: Vec<ConversionJob>,
}

/// One per-item conversion job within a `Batch` (§0.6 / §1.9). INTERNAL (not a wire type — the same
/// rationale as `Batch`).
///
/// [Build-Session-Entscheidung: P2.10 → P3.47] `Debug, Clone, PartialEq, Eq`; NOT `Copy` (its `source:
/// JobSource` owns `String`/`PathBuf`-bearing records). `Eq` holds — every field type is `Eq` (`JobSource`
/// derives `Eq`, P3.47; `OutputPlan` derives `Eq`, P2.9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionJob {
    /// The job's top-level key — the item's `ItemId`, DENORMALIZED from `source.item()` for cheap
    /// addressing in the §1.9 lifecycle + the per-item progress/finished events without unwrapping
    /// `source` (§0.6; the same duplicate-for-cheap-access pattern as `count` beside `items.len()`).
    /// INVARIANT (§0.6): `item == source.item()` — UNIFORM over both `JobSource` arms (P3.47),
    /// property-tested in P2.14.
    pub item: ItemId,
    /// The job's own frozen source record (§0.6 `JobSource`, P3.47) — the `Eligible(DroppedItem)` arm for a
    /// queued conversion, or the `Skipped(SkippedItem)` arm for a pre-flight-ineligible non-queue job (which
    /// carries the COMPLETE skip record so the `Batch` is the sole post-C6 carrier once the §0.4.4 registry
    /// is evicted). Coupling invariant (§0.6, P2.14): `source is Skipped(_) ⟺ state is JobState::Skipped(_)`.
    pub source: JobSource,
    /// The §1.9 lifecycle state — §1.9 owns the TRANSITIONS; this stores the current state.
    pub state: JobState,
    /// The §1.8-computed output plan, set before the write — `None` until §1.8 plans it (and always `None`
    /// for a pre-flight `Skipped` job, which never plans an output).
    pub plan: Option<OutputPlan>,
}

/// The §1.9 job-lifecycle state (§0.6) — §1.9 owns the TRANSITIONS between these variants; this is the
/// canonical state TYPE the orchestrator stores on each `ConversionJob` AND surfaces per-item in the §1.12
/// `RunResult.items[].state` summary. `Failed` carries the §2.8 kind, NOT a full `IpcError` (the wire
/// `IpcError` is assembled from the kind + path + message at the §1.12 projection — storing just the kind
/// keeps `JobState` cheap and serde-stable, §0.6).
///
/// [Build-Session-Entscheidung: P2.12] `JobState` IS a WIRE type — it derives `Serialize` + `specta::Type`
/// (added here, correcting the P2.10 "internal, not a wire type" note). The spec puts it on the wire: §0.6
/// `ItemResult.state: JobState` is carried inside `RunResult`, which §0.4.2 emits as `RunFinished(RunResult)`
/// and the C8 return, and §0.6's own JobState comment calls it "serde-stable". This is DISTINCT from the LIVE
/// per-item `ItemFinished` event, which carries the richer terminal `ItemOutcome` projection — the §1.12
/// summary's per-item state is `JobState`, the live terminal event is `ItemOutcome`; BOTH cross the wire,
/// for two different surfaces (P2.10's note conflated them). OUTBOUND-ONLY (no `Deserialize` — it is only
/// ever sent Rust→WebView in the summary, never deserialized), mirroring the §2.8 kinds it carries.
/// Externally tagged with `#[serde(rename_all = "camelCase")]` (the §0.6 wire-enum convention, cf.
/// `DetectionOutcome`/`CollectedSet`): unit variants serialize as `"pending"`…`"cancelled"`, the newtype
/// variants as `{"failed":"corrupt"}` / `{"skipped":"empty"}`.
///
/// [Build-Session-Entscheidung: P2.10] `Failed` is spelled with the CONCRETE `crate::outcome::
/// ConversionErrorKind`, NOT the §0.6/§1.9-named `ErrorKind` ALIAS (`pub type ErrorKind =
/// ConversionErrorKind`, P2.18) — it is the SAME type, but referencing the still-forward-declared
/// `ErrorKind` alias from this (production-dead) type trips the rustc dead-code lint-EXPECTATION
/// interaction with `crate::outcome`'s forward-declaration suppression (type aliases have incomplete
/// dead-code-expectation support); the concrete spelling avoids it with no semantic change — exactly the
/// P2.9 `OutputPlan.job: ItemId`-not-`JobId` resolution. specta resolves the alias to the same wire type.
///
/// [Build-Session-Entscheidung: P2.10/P2.12] `Debug, Clone, Copy, PartialEq, Eq` — `Copy` because both
/// payloads (`ConversionErrorKind` + `SkipReason`) are `Copy` fieldless enums, so the state is a cheap value
/// to move through the lifecycle; PLUS `Serialize` + `Type` (P2.12, the wire pair above). Variant order
/// matches §0.6 exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum JobState {
    /// Queued, not started (§1.9).
    Pending,
    /// The engine has been invoked (§1.7).
    Running,
    /// Output verified + atomically published (§2.1).
    Succeeded,
    /// A named §2.8 failure kind; nothing was written for it (§2.1). The §1.9 Running→Failed transition
    /// maps the internal kind to the wire kind via `ErrorKind::from` in `crate::orchestrator` (§2.8.2).
    Failed(ConversionErrorKind),
    /// A skipped item — terminal, never queued, no live events (§1.9). Either a detection-ineligible pre-flight
    /// item (§1.2/§1.3, its `SkipReason` copied from the frozen `SkippedItem` at `build_batch`) OR the §2.5.3
    /// re-run skip (`Skipped(AlreadyConverted)`, the P3.48 ruling — assigned by the C6 conductor's §2.5 applier
    /// on a ledger-hit item that keeps its real `Eligible` `DroppedItem`; the refined P3.47 coupling).
    Skipped(SkipReason),
    /// User cancel; nothing written for it (§1.7/§1.11).
    Cancelled,
}

// ─── §1.9 job/batch lifecycle FSM (P3.46.1) + the Running→Failed projection (P3.46.2) ──
// [Build-Session-Entscheidung: P3.46] The §1.9 conducting BEHAVIOUR the module doc names: the pure transition
// graph over `JobState` + the deterministic §1.9 queue order (P3.46.1), and the §2.8 Running→Failed projection
// (P3.46.2) mapping a §1.7 `InvocationResult` onto the terminal event + its §2.8.2 message. Both are PURE (no
// I/O, no spawn) — the P3.48 conductor composes them: `queue_order` → `advance(_, Started)` → dispatch →
// `project_outcome` → `advance(_, event)` — so they stay dead in the production build until that conductor makes
// them a live root (the module dead_code expect). Homed in `crate::orchestrator` per §0.7 (the tier-1 §1.9
// lifecycle owner) — the 2026-07-11 reconciliation of §1.9's "crate::run" mis-attribution (the P3.46 [Decision]
// note); the projection composes `InvocationResult` (tier-2 `engines`), `JobState` (tier-1) and `crate::outcome`
// (tier-2), a legal downward fan the tier-2 scratch/cleanup `run` leaf could not host. No taxonomy in the FSM:
// the internal-kind→wire-kind projection is `project_outcome`, so "a wrong transition fails in P3.46.1, a
// missing catalog entry in P3.46.2" (the sub-box boundary).

/// A §1.9 lifecycle EVENT driving a QUEUED job from one [`JobState`] to the next (§1.9). [`JobState::Skipped`]
/// is set at `Batch` construction (P3.47) and is NOT an event target — a pre-flight-skipped job never enters
/// the queue and never transitions. `Failed` carries the concrete [`ConversionErrorKind`] (== the §2.8
/// `ErrorKind` alias) the §2.8 projection ([`project_outcome`]) produced — the FSM applies it as a pure state
/// move, it does not project it. [Build-Session-Entscheidung: P3.46] INTERNAL — `Debug, Clone, Copy, PartialEq,
/// Eq` (`Copy`: its `ConversionErrorKind` payload is a `Copy` fieldless enum), NO `serde`/`specta` (the FSM
/// input is core-only; the wire carries `JobState`/`OutcomeMsg`, not the event).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobEvent {
    /// The engine was invoked (§1.7) — `Pending → Running`.
    Started,
    /// Output verified + atomically published (§2.1) — `Running → Succeeded`.
    Succeeded,
    /// A named §2.8 failure — `Running → Failed(kind)`; nothing was written (§2.1). The kind is projected from
    /// the §1.7 `InvocationResult` by [`project_outcome`] (P3.46.2), never here.
    Failed(ConversionErrorKind),
    /// User cancel (§1.7/§1.11) — `Running → Cancelled`; nothing was written.
    Cancelled,
}

/// An illegal §1.9 transition — a conductor bug (a terminal or pre-flight-`Skipped` state re-driven, or an
/// out-of-order event such as `Succeeded` before `Started`). Surfaced as a structured `Err` from [`advance`],
/// NEVER a panic (the crate-root `clippy::panic`/`unwrap_used`/`expect_used` deny holds on this path too — an
/// invalid transition is a control-flow bug to diagnose, not a crash). [Build-Session-Entscheidung: P3.46]
/// INTERNAL — `Debug, Clone, Copy, PartialEq, Eq` (both fields are `Copy`), no `serde`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IllegalTransition {
    /// The state the illegal event was applied to.
    pub from: JobState,
    /// The event that is not valid from `from`.
    pub event: JobEvent,
}

/// Apply a §1.9 lifecycle [`JobEvent`] to a [`JobState`], enforcing the §1.9 transition graph (P3.46.1) — the
/// pure FSM the module doc names, no I/O, no taxonomy/serialization:
///
/// ```text
/// Pending ──Started──▶ Running ─┬─Succeeded──▶ Succeeded
///                               ├─Failed(k)──▶ Failed(k)
///                               └─Cancelled─▶ Cancelled
/// Skipped(_)  +  Succeeded / Failed(_) / Cancelled : terminal — accept NO event
/// ```
///
/// Every other `(state, event)` pair is an [`IllegalTransition`] `Err` (a conductor bug), never a panic. The
/// match is EXHAUSTIVE over [`JobState`] (and over [`JobEvent`] within each non-terminal state) — no `_`
/// wildcard, so a new `JobState`/`JobEvent` variant forces a compile-time transition decision (the crate-root
/// `clippy::wildcard_enum_match_arm` deny, G4/G14). [Build-Session-Entscheidung: P3.46]
pub fn advance(state: JobState, event: JobEvent) -> Result<JobState, IllegalTransition> {
    let next = match state {
        JobState::Pending => match event {
            JobEvent::Started => Some(JobState::Running),
            JobEvent::Succeeded | JobEvent::Failed(_) | JobEvent::Cancelled => None,
        },
        JobState::Running => match event {
            JobEvent::Succeeded => Some(JobState::Succeeded),
            JobEvent::Failed(kind) => Some(JobState::Failed(kind)),
            JobEvent::Cancelled => Some(JobState::Cancelled),
            JobEvent::Started => None,
        },
        // Terminal (§1.9): a published / failed / cancelled job and a pre-flight `Skipped` job accept no event.
        JobState::Succeeded | JobState::Failed(_) | JobState::Skipped(_) | JobState::Cancelled => {
            None
        }
    };
    next.ok_or(IllegalTransition { from: state, event })
}

/// `true` iff a [`JobState`] entered (or can enter) the §1.9 `Pending` queue — every state EXCEPT the
/// pre-flight [`JobState::Skipped`] (set at `Batch` construction, never queued, §1.9). The `Batch.jobs` list
/// carries BOTH the queued jobs AND the materialised pre-flight-`Skipped` records (P3.47), so [`queue_order`]
/// filters on this to yield only the queued items. Exhaustive over [`JobState`] (no `_`, G4/G14).
/// [Build-Session-Entscheidung: P3.46]
fn state_is_queued(state: JobState) -> bool {
    match state {
        JobState::Pending
        | JobState::Running
        | JobState::Succeeded
        | JobState::Failed(_)
        | JobState::Cancelled => true,
        JobState::Skipped(_) => false,
    }
}

/// The §1.9 deterministic queue order (P3.46.1): the `Batch`'s QUEUED jobs — the eligible ones that entered the
/// `Pending` queue, i.e. every job EXCEPT the pre-flight `Skipped` records (P3.47) — yielded in the frozen
/// collected/traversal order (`Batch.jobs` order, the §1.1 depth-first order) with NO reordering (§1.9 "[REC] no
/// priority/size reordering in v1"). The order is the `Batch.jobs` order verbatim, not a re-sort — the
/// determinism the §1.11 progress bar + the §1.12 summary read predictably.
///
/// **Dispatch-eligibility (for the P3.48 conductor):** this yields EVERY non-`Skipped` job — including one
/// already `Running`/terminal — so it is the stable §1.11/§1.12 denominator + traversal order, NOT a
/// dispatch-ready filter. The P3.48 conductor selects the `Pending` subset (via [`advance`]`(_, Started)`,
/// which returns [`IllegalTransition`] for a non-`Pending` job by design) before dispatching an item; it does
/// not drive `Started` off this iterator blindly. [Build-Session-Entscheidung: P3.46]
pub fn queue_order(batch: &Batch) -> impl Iterator<Item = &ConversionJob> {
    batch.jobs.iter().filter(|job| state_is_queued(job.state))
}

/// The §2.8 projection of a §1.7 [`InvocationResult`] (P3.46.2) — the §1.9 terminal [`JobEvent`] the FSM
/// ([`advance`]) applies, plus the per-item §2.8.2 [`OutcomeMsg`] a Running→Failed outcome carries (into
/// `ItemResult.reason` / the live `ItemFinished`, P3.50/P3.48). `message` is `Some` ONLY for `Failed`; a
/// `Succeeded`/`Cancelled` item carries no per-item failure message. [Build-Session-Entscheidung: P3.46]
/// INTERNAL — `Debug, Clone, PartialEq, Eq`, NOT `Copy` (`OutcomeMsg` owns a `String`), no `serde` (the pairing
/// is core-only; its `OutcomeMsg` is separately the wire type).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalProjection {
    /// The §1.9 terminal event [`advance`] applies to move the `Running` job to its terminal [`JobState`].
    pub event: JobEvent,
    /// The rendered §2.8.2 per-item message — `Some(OutcomeMsg::Failure)` for a Running→Failed outcome, `None`
    /// for `Succeeded`/`Cancelled`.
    pub message: Option<OutcomeMsg>,
}

/// Project one §1.7 [`InvocationResult`] onto its §1.9 terminal [`JobEvent`] + (for a failure) its §2.8.2
/// per-item message (P3.46.2):
///
/// - `Succeeded` → `(`[`JobEvent::Succeeded`]`, None)` — the §2.1 verified/published item; no per-item message.
/// - `Cancelled` → `(`[`JobEvent::Cancelled`]`, None)` — the cooperatively-cancelled item; no per-item message.
/// - `Failed(kind)` → `(`[`JobEvent::Failed`]`(kind), Some(OutcomeMsg::Failure))` — the Running→Failed leg.
///
/// **The internal-kind → wire-kind map is the IDENTITY** (§2.8.2 option-1): `ErrorKind` is `pub type ErrorKind
/// = ConversionErrorKind` (P2.18), so the §1.9 spec's `ErrorKind::from(kind)` is the std reflexive `From<T> for
/// T` — `crate::outcome` owns no such `From` (E0119) and none can be written; the internal kind IS the wire
/// kind (reconciled at P3.1.3). So the projection passes `kind` through verbatim (an explicit `ErrorKind::from`
/// would be a `clippy::useless_conversion` no-op) — [`JobState::Failed`] already stores the concrete
/// `ConversionErrorKind` (P2.10). The substantive work is the MESSAGE: [`conversion_failure`] renders the
/// §2.8.2 catalog row (P3.68) with **`arg = ""`** — every kind reachable from a Running→Failed `InvocationResult`
/// (Corrupt / Gone / Unreadable / WriteFailed / EngineHang / EngineCrash / EngineError / InternalError …) is a
/// per-item conversion-outcome kind with NO substitution slot. The §2.2.4 `UnopenableOutputName` (P3.88) is the
/// FIRST slotted kind that IS a Running→Failed outcome, but it arises on the §2.1.1 publish / §1.8 output-plan
/// path (`map_publish_error` / `compute_output_plan`), NOT this engine-`InvocationResult` projection — the
/// conductor's INLINE render (`failure_message` + `item_base_reason`) takes its `name_arg`; the other slotted
/// kinds (UnsupportedType `{detected}`, PlatformUnavailable `{platform}`, CleanupResidue `{path}`) stay
/// pre-flight / app-level, never a Running→Failed outcome. **Forward constraint (a P4 `classify_failure()`
/// consumer):** since `InvocationResult::Failed` is untyped over the full `ConversionErrorKind` set, and THIS
/// projection still renders `arg = ""`, a P4 engine's `classify_failure()` MUST NOT route a slotted kind through
/// this path with the empty `arg` (it would render an empty slot, e.g. "…it looks like ."), or must extend this
/// projection to supply the slot's `arg` — P3.88 already did exactly that on the conductor's inline path (so the
/// "first slotted Running→Failed kind" arrived earlier than this P4 note anticipated); the `arg = ""` correctness
/// here remains a slice invariant, not a compiler-checked one. A kind §2.8.2 homes elsewhere (a mis-route →
/// [`conversion_failure`] `None`) falls back to the always-available `InternalError` row so a failed item is
/// never message-less. Exhaustive over [`InvocationResult`] (no `_`, G4/G14). [Build-Session-Entscheidung: P3.46, P3.88]
pub fn project_outcome(result: InvocationResult) -> TerminalProjection {
    match result {
        InvocationResult::Succeeded => TerminalProjection {
            event: JobEvent::Succeeded,
            message: None,
        },
        InvocationResult::Cancelled => TerminalProjection {
            event: JobEvent::Cancelled,
            message: None,
        },
        InvocationResult::Failed(kind) => {
            // Identity map (§2.8.2 alias): the internal kind IS the wire kind — passed through, not
            // `.from()`-converted (a useless-conversion no-op). The message is the substantive projection.
            let message = conversion_failure(kind, "")
                .or_else(|| conversion_failure(ConversionErrorKind::InternalError, ""));
            TerminalProjection {
                event: JobEvent::Failed(kind),
                message,
            }
        }
    }
}

/// Construct the §1.9 [`Batch`] from a frozen `CollectedSet::Single` at C6 (`start_conversion`) — the
/// materialisation the §1.9 "Batch construction projects pre-flight skips as non-queue `Skipped` records
/// `[DECIDED]`" anchor names (P3.47). For every eligible `DroppedItem` in `frozen.items` it creates a
/// `Pending` job ([`JobSource::Eligible`], `plan: None` until §1.8 plans it); for every `SkippedItem` in
/// `frozen.skipped` it creates a terminal `Skipped(reason)` job ([`JobSource::Skipped`], the `SkipReason`
/// **copied directly** from `SkippedItem.reason`) — set **at construction**, never queued, never
/// transitioned, receiving **no** `Channel` events (§0.4.2). This is the single anchor that prevents a skip
/// from being lost when the §0.4.4 collected-set registry is evicted at C6: the `Batch` becomes the sole
/// post-C6 carrier of the COMPLETE skip record.
///
/// The jobs are emitted in the deterministic §1.1 collected/traversal order over the §0.6-invariant-6
/// single id space — `jobs.sort_by_key(|job| job.item)`. The freeze assigns each `ItemId` in traversal
/// order over the merged eligible+skipped set (§0.6 invariant 6), so **id order IS traversal order**, and a
/// `SkippedItem.item` can never collide with an eligible `ItemId` — sorting by id therefore INTERLEAVES the
/// eligible `Pending` jobs and the `Skipped` records back into the original drop order, so the §1.11
/// progress bar + the §1.12 summary read predictably (`[REC]` no priority/size reordering in v1).
///
/// Every job upholds the §0.6 invariants by construction: `item == source.item()` (denormalized uniformly
/// from whichever arm), and the coupling `source is Skipped(_) ⟺ state is JobState::Skipped(_)` (an eligible
/// item is `Eligible`+`Pending`, a skipped item is `Skipped`+`Skipped(reason)`). PURE (no I/O, no spawn):
/// the whole-batch `target`/`options`/`destination` are the C6 command arguments the P3.48 conductor
/// supplies (after resolving the frozen set from the §0.4.4 registry), and this fn stays dead in the
/// production build until that conductor calls it (the module `dead_code` expect).
/// [Build-Session-Entscheidung: P3.47]
pub fn build_batch(
    frozen: &FrozenCollectedSet,
    target: Target,
    options: OptionValues,
    destination: ResolvedDestination,
) -> Batch {
    let mut jobs: Vec<ConversionJob> =
        Vec::with_capacity(frozen.items.len() + frozen.skipped.len());
    // Eligible items → queued `Pending` jobs (JobSource::Eligible; plan None until §1.8).
    for dropped in &frozen.items {
        let source = JobSource::Eligible(dropped.clone());
        jobs.push(ConversionJob {
            item: source.item(),
            source,
            state: JobState::Pending,
            plan: None,
        });
    }
    // Pre-flight-skipped items → terminal `Skipped(reason)` records materialised at construction
    // (JobSource::Skipped; the reason copied directly from SkippedItem.reason; never queued, never planned).
    for skipped in &frozen.skipped {
        let reason = skipped.reason;
        let source = JobSource::Skipped(skipped.clone());
        jobs.push(ConversionJob {
            item: source.item(),
            source,
            state: JobState::Skipped(reason),
            plan: None,
        });
    }
    // Deterministic §1.1 traversal order over the single id space (id order == traversal order, §0.6 inv 6):
    // interleave the eligible + skipped jobs back into drop order.
    jobs.sort_by_key(|job| job.item);
    Batch {
        id: frozen.id,
        source_format: frozen.format,
        target,
        options,
        destination,
        jobs,
    }
}

// ─── §0.6 command-return DTOs — the §1.8/§1.10/§2.5 C4/C5 preview shapes (P2.11) ──
// [Build-Session-Entscheidung: P2.11] The §0.6 "Command return DTOs" group homed in `crate::orchestrator`
// (tier 1) because each references `crate::outcome` — `PreflightVerdict` DIRECTLY (`up_front_fail:
// Option<ErrorKind>`), `OutputPlanPreview`/`DestinationResolved` TRANSITIVELY (they embed `preflight:
// PreflightVerdict`). Per the §0.7 ‡ rule "directly OR transitively → orchestrator, never domain" (the
// owner-finalised P2.11 homing, 7ee293b). They embed the outcome-free `RerunPrompt`/`DivertReason`/
// `DestinationChoice` from `crate::domain` via a downward `orchestrator`→`domain` edge (allowed). Each is
// a WIRE type (derives `specta::Type` + camelCase) but `Serialize`-ONLY: the embedded
// `Option<ConversionErrorKind>` is itself outbound-only (no `Deserialize`, P2.18), which cascades — and a
// command RETURN is sent Rust→WebView, never deserialized in Rust anyway. Registration rides the
// C4/C5 consumers (P2.26/P2.27), the established P2.2-P2.9 defer pattern. `up_front_fail` spells the
// CONCRETE `ConversionErrorKind` (not the `ErrorKind` alias) against the rustc dead-code-expectation/alias
// trap — the P2.10 `JobState::Failed` precedent.

/// The §1.10 resource pre-flight verdict surfaced before convert (§0.6 / §1.10) — the size/space estimate
/// plus any whole-batch up-front "too big" / "won't fit" fail. Carried by the C4/C5 `OutputPlanPreview` /
/// `DestinationResolved` returns. Homed in `crate::orchestrator` (it references `crate::outcome` DIRECTLY
/// via `up_front_fail`).
///
/// [Build-Session-Entscheidung: P2.11] `Serialize` + `Type` (a wire type), NO `Deserialize` (its
/// `up_front_fail: Option<ConversionErrorKind>` is outbound-only — `ConversionErrorKind`/`ErrorKind` have
/// no `Deserialize`, P2.18); NOT `Copy` (struct convention). `up_front_fail` spells the concrete
/// `ConversionErrorKind` (the `ErrorKind` alias's underlying type) against the P2.10 alias trap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PreflightVerdict {
    /// Estimated total output + kind-1 publish-temp footprint across the batch (§1.10 / §2.14.1).
    pub est_total_output_bytes: u64,
    /// Estimated total kind-2 engine-working scratch footprint across the batch (§1.10 / §2.14.2).
    pub est_total_scratch_bytes: u64,
    /// `Some(TooBig | OutOfDisk)` ONLY for the WHOLE-BATCH doomed case (the §5.2 disable-Convert-wholesale
    /// flag — per-physical-volume / aggregate, §1.10). A PER-ITEM too-big / out-of-disk is NOT carried here
    /// — it is enforced at write time as that item's `Failed(TooBig|OutOfDisk)` while the batch continues
    /// (§1.10 / §1.11). `None` = the batch is not up-front doomed.
    pub up_front_fail: Option<ConversionErrorKind>,
}

/// The C4 `plan_output` return (§0.6 / §1.8) — drives the "will save to…" line shown before convert. Homed
/// in `crate::orchestrator` (it embeds `preflight: PreflightVerdict` → transitively references
/// `crate::outcome`, §0.7 ‡).
///
/// [Build-Session-Entscheidung: P2.11 → P3.76] `Serialize` + `Type`, NO `Deserialize` (embeds the
/// Serialize-only `PreflightVerdict`); NOT `Copy` (owns a `String`). camelCase wire form. P3.76 re-types
/// the directory PREVIEW field from `PathBuf` to a lossy display `String` (`final_dir_display`) — no
/// `PathBuf` crosses the wire (§2.10.1 / 2026-07-06 ruling); the real per-item dirs are computed core-side
/// by §1.8 and never leave the core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OutputPlanPreview {
    /// The collected set this preview is for (the §0.4.4 registry key).
    pub set: CollectedSetId,
    /// The resolved destination DIRECTORY shown before convert (§1.8 / §2.7) as a core-produced lossy
    /// display `String` (last-step `to_string_lossy`, §2.10.1) — directory-based, never a pre-baked final
    /// file path (the numbered name is resolved at §2.1 write time), never a re-submittable path.
    pub final_dir_display: String,
    /// `Some(reason)` if a per-location divert was previewed (§2.7.2); `None` = beside-source / no divert.
    pub diverted: Option<DivertReason>,
    /// `Some(..)` if the §2.5 in-session ledger detected an equivalent prior run (the one batch-level
    /// prompt's data); `None` = no re-run prompt.
    pub rerun: Option<RerunPrompt>,
    /// The §1.10 size/space estimate + any up-front whole-batch fail.
    pub preflight: PreflightVerdict,
}

/// The C5 `set_destination` return (§0.6 / §1.8 / §2.14.4) — the re-validated destination after the user
/// changes it. Homed in `crate::orchestrator` (embeds `preflight: PreflightVerdict` → transitively
/// references `crate::outcome`, §0.7 ‡).
///
/// [Build-Session-Entscheidung: P2.11 → P3.76] `Serialize` + `Type`, NO `Deserialize` (embeds the
/// Serialize-only `PreflightVerdict`); NOT `Copy`. camelCase. P3.76 ADDS the `final_dir_display` lossy
/// display `String` (mirroring `OutputPlanPreview.final_dir_display`, §2.10.1) so the refreshed
/// "will save to …" line has a display projection with no `PathBuf` on the wire. (`destination:
/// DestinationChoice` is the id-keyed wire form — `ChosenRoot(DestinationId)` since the P3.80 re-key landed —
/// the C5 echo of the choice; the core-resolved `ResolvedDestination` (a real `PathBuf`) never crosses the wire.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DestinationResolved {
    /// The (now chosen) destination the C5 echo carries back (§0.6 / §2.7) — the id-keyed wire
    /// `DestinationChoice` (`ChosenRoot(DestinationId)`, the P3.80 re-key); the core resolves the id to its real
    /// `PathBuf` against the §0.4.4 `DestinationRegistry` at C6, never on the wire.
    pub destination: DestinationChoice,
    /// The refreshed display-only "will save to …" form for the new destination (mirrors
    /// `OutputPlanPreview.final_dir_display`, last-step `to_string_lossy`, §2.10.1) [DECIDED 2026-07-06].
    pub final_dir_display: String,
    /// The recomputed per-location divert for the new destination (§2.7.2); `None` = no divert.
    pub diverted: Option<DivertReason>,
    /// The preflight RE-EVALUATED for the new destination volume (§2.14.4 free-space) so the UI's held C4
    /// verdict never goes stale (§1.8).
    pub preflight: PreflightVerdict,
    /// CARRIED THROUGH UNCHANGED from the C4 verdict — in v1 the §2.5 EquivKey has no destination
    /// component, so re-run is destination-INDEPENDENT (§2.5.1). C5's `resolve_destination_change` re-runs the
    /// ONE §1.8 preview machinery (`plan_output_preview`), whose PEEK-only re-run recompute therefore yields the
    /// IDENTICAL value the C4 verdict held — "carried through unchanged" BY CONSTRUCTION (an idempotent peek,
    /// never a divergent re-decision), not a separate skip-the-recompute path. Only `preflight` actually changes
    /// with the destination (§2.14.4). [Build-Session-Entscheidung: P3.56 — the resolver-reuse nuance]
    pub rerun: Option<RerunPrompt>,
}

// ── §0.4.1 C4/C5 lifecycle asymmetry invariant (P2.28) ───────────────────────────────────────────
// C4 `plan_output` and C5 `set_destination` take BYTE-IDENTICAL request payloads
// ({ collectedSetId, target, options, destination }), so their signatures alone cannot distinguish them.
// §0.4.1 ("C4 vs C5 — byte-identical payloads, different contract [DECIDED]") resolves the difference NOT
// by a one-shot guard but BY LIFECYCLE — the rule this module's behaviour (P3.48) + the C4/C5 body boxes
// (P2.44+) honor:
//   1. C4 is RE-CALLABLE at any point in state 4 (eager on the 3→4 transition, then debounced ~150 ms on
//      any target/option change, §5.8) — there is NO "fires exactly once" constraint.
//   2. C5 OWNS the destination: once the user changes it (a C5 on a `collectedSetId`), a subsequent C4 on
//      that same set CARRIES the C5-resolved destination in its `destination: DestinationChoice` argument
//      (caller-passed) and NEVER resets it. There is NO server-side destination store — the destination is
//      authoritative as the C6 argument (§0.4.1 C6 [DECIDED]); the "re-apply the retained C5 destination if
//      C4 arrives carrying a stale default" (§0.4.1) is a P3.49 runtime stale-default REPAIR, NOT a P2 state
//      structure.
//   3. C4 COMPUTES `rerun` (§2.5 equivalence) + the §1.10 `preflight`; C5 NEVER recomputes `rerun` (the v1
//      EquivKey is destination-independent, §2.5.1) — it CARRIES C4's `rerun` THROUGH UNCHANGED and
//      re-evaluates ONLY the destination-volume `preflight` (§2.14.4). This is the ONLY ordering rule.
//
// [Build-Session-Entscheidung: P2.28] Structural + documented layer authored HERE; runtime asserts at P3.48/P3.49.
// The orchestrator BEHAVIOUR that enforces these at runtime (the re-callable C4 plan, the C5 destination
// authority, the computed-vs-carried-through `rerun`) is the P3.48 conductor + the C4/C5 body boxes (P2.44+).
// P2.28 encodes the two layers that exist at the contract stage: (i) this documented lifecycle invariant the
// P3.48 conductor + the body boxes honor, and (ii) the structural ENABLERS the DTO shapes above already
// guarantee — pinned by the `c4_c5_asymmetry_structural_enablers` test: `DestinationResolved` CARRIES a
// `destination` (C5 owns it) while `OutputPlanPreview` carries only a `final_dir_display` PREVIEW and NO
// `DestinationChoice` field (C4 does not own the choice), and both carry the SAME `rerun: Option<RerunPrompt>`
// type (so C5 carries C4's `rerun` through unchanged). This is the same contract-here / behaviour-at-P3.48/P3.49
// split as the C1–C6 shells, NOT a stub: the structure makes the lifecycle rule TYPE-POSSIBLE; P3.48/P3.49 adds the
// runtime enforcement.

// ─── §1.12 result types — the end-of-batch RunResult + the live ItemFinished outcome (P2.12) ──
// [Build-Session-Entscheidung: P2.12] The §0.6 §1.12 result family homed in `crate::orchestrator` (tier 1,
// "which §1.12 computes & references by name"): `RunResult`/`ItemResult`/`ItemOutcome` reference
// `crate::outcome` (`OutcomeMsg`/`IpcError`) + the local `JobState` → orchestrator per the §0.7 ‡ rule. The
// pure `Totals`/`CleanupResidue` (counts / §2.6 residue info, no outcome ref) are CO-HOMED here with
// `RunResult` for result-family cohesion (the §0.7 ‡ box-note sanctions either domain or orchestrator —
// `RunResult` embeds both, so co-homing keeps the family together via a downward `orchestrator`→`domain`
// edge for `CleanupResidue.item: ItemId`; routine loop choice, not a cycle decision). All are WIRE types
// (`RunResult` rides §0.4.2 `RunFinished` + the C8 return; `ItemOutcome` rides §0.4.2 `ItemFinished`):
// `Serialize` + `Type`, NO `Deserialize` (outbound-only — a §0.4.2 event payload / command return is sent
// Rust→WebView, never deserialized in Rust; the embedded outbound-only `OutcomeMsg`/`IpcError`/`JobState`
// cascade this anyway). Registration rides the §0.4.2 event / C8 consumers, the established P2.2-P2.11
// defer pattern. camelCase wire form throughout.

/// The §1.12 end-of-batch summary (§0.6) — emitted as §0.4.2 `RunFinished(RunResult)` when every job has
/// left `Pending`/`Running`, and idempotently re-served by C8 `get_run_summary` after a WebView reload
/// (§0.4.4 run-registry retention). It is the §5.3 `ResultSummary`'s single source: per-item outcome +
/// output→source map + residue warnings + the open-folder roots.
///
/// [Build-Session-Entscheidung: P2.12 → P3.76] `Serialize` + `Type` (wire), NO `Deserialize`; NOT `Copy`
/// (owns `Vec`/`String` fields). camelCase wire form (`collectedSetId`/`runId`/`cleanupIncomplete`/
/// `commonRootDisplay`/`divertRootDisplay`). P3.76 re-types the two root fields from `PathBuf`/
/// `Option<PathBuf>` to lossy display `String`s — **no `PathBuf` crosses the wire** (§2.10.1 / 2026-07-06
/// ruling); the REAL common/divert roots (and the per-item output/residue `PathBuf`s) live in the
/// `RunResultStore` OFF-WIRE table (`RunResultPaths`), which C9 resolves its `OpenTarget` against (P3.79).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    /// The frozen collected-set this run summarises — `Batch.id` IS a `CollectedSetId` (§1.12), tying the
    /// summary back to its §0.4.4 collected-set registry entry.
    pub collected_set_id: CollectedSetId,
    /// The run this summary is for (§7.1) — minted at C6 `start_conversion`.
    pub run_id: RunId,
    /// Per-item outcome + output→source mapping (§1.12). INCLUDES every skip — the freeze-time pre-flight
    /// `SkippedItem`s (`CollectedSet::Single.skipped`, the four detection reasons) AND the §2.5.3 re-run skip
    /// (`Skipped(AlreadyConverted)`, assigned by the C6 applier over an item that keeps its `Eligible`
    /// `DroppedItem`) — each projected as `ItemResult { item, output_display: None, state: Skipped(reason),
    /// reason: Some(OutcomeMsg::Skipped{ reason, .. }) }`; the skip rides the skip-shaped `OutcomeMsg` variant
    /// (§2.8), NOT `Failure`, so skip ≠ fail at the type level (§1.12); counted in `totals.skipped`.
    pub items: Vec<ItemResult>,
    /// The succeeded / failed / cancelled / skipped tally (§1.12).
    pub totals: Totals,
    /// The §2.6 cleanup-incomplete warnings — items whose partial could not be removed, so the run is never
    /// reported as a clean success (§2.6.4). Empty when every cleanup completed.
    pub cleanup_incomplete: Vec<CleanupResidue>,
    /// The display-only "open folder" LABEL for the BESIDE-SOURCE outputs — the dropped-selection common
    /// ancestor (§2.7 / §7.7.3) as a lossy display `String` (last-step `to_string_lossy`, §2.10.1)
    /// [DECIDED 2026-07-06]. The REAL root `PathBuf` lives in the `RunResultStore` off-wire table
    /// (`RunResultPaths.common_root`), opened via C9 `OpenTarget::CommonRoot`.
    pub common_root_display: String,
    /// `Some(display)` when ANY item was diverted (§2.7.3) — a single field cannot carry both the
    /// beside-source and divert roots, so the divert root is its own display field; `None` when no item
    /// diverted. Both REAL roots are §7.7.3 open-folder targets resolved core-side (C9
    /// `OpenTarget::DivertRoot`, from `RunResultPaths.divert_root`); a per-item diverted output is reachable
    /// via C9 `OpenTarget::Item(ItemId)` (its real path in `RunResultPaths.item_outputs`).
    pub divert_root_display: Option<String>,
    /// The §1.12 batch-level SUMMARY LINE, core-assembled + ready to show: the §2.8.2 situation row for this
    /// run's `totals` ("All {n} files converted." / "{ok} of {n} files converted. …" / "None of the {n} files
    /// could be converted." / "Stopped. …") with the §2.6.4 "With residue" tail appended iff
    /// `cleanup_incomplete` is non-empty — i.e. exactly [`batch_summary_line`]'s output. The §5.3 Summary
    /// renders it VERBATIM; the fully-failed case is what §5.2 row 8 dresses as a clear failure banner
    /// ("never a quiet done"), but the STRING is §02's, not chrome.
    ///
    /// [Build-Session-Entscheidung: P3.59] Named `summary_line_display` per the sibling `*_display` convention
    /// (the wire-facing, core-rendered projection a consumer shows as-is). It is a plain `String`, not an
    /// `Option`: §2.8.2's situation table is TOTAL over `Totals` (`batch_summary` always classifies), so every
    /// run has a line — an `Option` would invent an "no summary" state the spec does not have. This field is
    /// what the 2026-07-16 P3.59 ruling wired: [`batch_summary_line`] was built at P3.50 but had NO production
    /// caller, and that wire gap is exactly why the pre-ruling fill authored a chrome banner string against
    /// §5.7:799 (the G1 NOGO).
    pub summary_line_display: String,
}

/// One per-item row of the §1.12 summary (§0.6) — its `ItemId` (the output→source mapping anchor; the
/// source is named for display via the `CollectedSet`'s `DroppedItem.display_name`), its terminal
/// `JobState`, the display-only output form (`Some` only when `Succeeded`), and the resolved surfaced line.
///
/// [Build-Session-Entscheidung: P2.12 → P3.76] `Serialize` + `Type` (wire — embedded in `RunResult`), NO
/// `Deserialize`; NOT `Copy` (owns `String`/`OutcomeMsg`). camelCase. `state: JobState` is what forces
/// `JobState` to be a wire type (see its doc) — the summary's per-item state, distinct from the live
/// `ItemFinished`'s `ItemOutcome`. P3.76 retires the two path fields off the wire (2026-07-06 ruling,
/// §2.10.1): `source: PathBuf` → `item: ItemId` (display via `DroppedItem.display_name`; real paths in
/// `FrozenCollectedSet`/`RunResultStore`) and `output: Option<PathBuf>` → `output_display: Option<String>`
/// (the real output `PathBuf` is `RunResultStore`-side, opened via C9 `OpenTarget::Item(item)`). The
/// `state`/`reason` pair is unchanged (`OutcomeMsg` carries kind + text, never a path).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    /// The ID-keyed output→source mapping anchor (§1.12) — the source is named for display via the
    /// `CollectedSet`'s `DroppedItem.display_name`; the REAL paths live in `FrozenCollectedSet` /
    /// `RunResultStore` (§0.4.4), where C9 resolves `OpenTarget::Item(item)`.
    pub item: ItemId,
    /// The display-only lossy form of the published output (last-step `to_string_lossy`, §2.10.1) —
    /// `Some(..)` ONLY when `state == Succeeded` (§1.12); `None` otherwise. The REAL output `PathBuf` is
    /// `RunResultStore`-side, opened via C9 `OpenTarget::Item(item)`.
    pub output_display: Option<String>,
    /// The terminal §1.9 lifecycle state for this item (§0.6) — at `RunFinished` always a terminal variant
    /// (`Succeeded`/`Failed`/`Skipped`/`Cancelled`).
    pub state: JobState,
    /// The resolved, ready-to-show §2.8 failure / §2.9 lossy / §1.1 skip line, **or the §2.6.4 case-1 residue
    /// annotation** on an otherwise-successful item (§2.8.2 `OutcomeMsg` — the non-failure `Residue` variant the
    /// P3.59 ruling added, in the `Lossy` shape). `None` for a plain success with no lossy note AND no residue,
    /// and for a §2.6.4 case-3 (cancelled-with-residue) item — §2.6.4 authors no per-item case-3 sentence, so
    /// its surface is the structural `cleanup_incomplete` entry + the batch-level tail on
    /// [`RunResult::summary_line_display`] alone.
    pub reason: Option<OutcomeMsg>,
}

/// The §1.12 per-outcome tally (§0.6). The "all failed" condition is DERIVED (`all_failed()`), never a
/// stored field — SSOT *Fail clearly*: a fully-failed batch is an explicit failure, not a quiet finish.
///
/// [Build-Session-Entscheidung: P2.12] `Serialize` + `Type` (wire — embedded in `RunResult`), NO
/// `Deserialize`; NOT `Copy` (the §0.6 struct convention, cf. `PreflightVerdict`). camelCase. The
/// `total()`/`all_failed()` helpers home the §1.12 derived condition so it is computed once, never
/// re-derived inconsistently (and never stored as a field).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    /// Items that converted + published successfully (§2.1).
    pub succeeded: u32,
    /// Items that failed with a named §2.8 kind (the batch continued, §1.9).
    pub failed: u32,
    /// Items discarded by user cancel (§1.7/§1.11) — finished-before-cancel items stay in `succeeded`.
    pub cancelled: u32,
    /// Skipped items projected into the summary (§1.3/§1.12) — never `failed`: the pre-flight
    /// detection-ineligible skips AND the §2.5.3 re-run skip (`Skipped(AlreadyConverted)`, the P3.48 ruling).
    pub skipped: u32,
}

impl Totals {
    /// The total item count — the sum of the four tallies (§1.12). Not stored; derived from the parts.
    /// Returns `u64`: the sum of four `u32`s is always exact in `u64`, so the derived §1.12 condition can
    /// never be distorted by saturation (a saturated "total" equal to `failed` would make `all_failed` lie
    /// at the `u32` ceiling — the silent-saturation class `ItemIdSpace`'s `checked_add` discipline rejects).
    /// Derived-helper-only (never serialized), so the wire shape is unchanged.
    /// [Build-Session-Entscheidung: P2.137]
    pub fn total(&self) -> u64 {
        u64::from(self.succeeded)
            + u64::from(self.failed)
            + u64::from(self.cancelled)
            + u64::from(self.skipped)
    }

    /// The §1.12 "all failed" condition (`failed == total && total > 0`) — DERIVED, never stored. A
    /// fully-failed batch is surfaced as an explicit failure, never a quiet finish (SSOT *Fail clearly*).
    pub fn all_failed(&self) -> bool {
        let total = self.total();
        total > 0 && u64::from(self.failed) == total
    }
}

/// A §2.6.4 cleanup-incomplete warning (§0.6) — one item whose partial could not be removed, naming WHERE
/// the residue may remain so the summary never reports a clean success (§2.6 / §1.12).
///
/// [Build-Session-Entscheidung: P2.12 → P3.76] `Serialize` + `Type` (wire — embedded in `RunResult`), NO
/// `Deserialize`; NOT `Copy` (owns a `String`). camelCase (`residueDisplay`). `item: ItemId` is the
/// downward `orchestrator`→`domain` edge that co-homing this leaf here introduces (allowed). P3.76 re-types
/// `residue_path: PathBuf` → the display-only `residue_display: String` (2026-07-06 ruling, §2.10.1); the
/// real residue `PathBuf` stays core-side in the `RunResultStore` off-wire table (`RunResultPaths.
/// item_residues`), revealed via C9 `OpenTarget::Residue(item)` (P3.79).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResidue {
    /// The item whose cleanup did not complete (§2.6.4) — the stable §0.6 `ItemId` (also the off-wire
    /// `RunResultPaths.item_residues` key where the real residue `PathBuf` lives).
    pub item: ItemId,
    /// The display-only lossy form of where the residue may remain (§2.6.4, last-step `to_string_lossy`,
    /// §2.10.1) — the only place the summary names a residue; never a re-submittable path.
    pub residue_display: String,
}

/// The terminal per-item outcome carried by the LIVE §0.4.2 `ItemFinished` event (§0.6) — the richer
/// terminal projection the UI applies as each item finishes, distinct from the summary's `JobState`.
/// `Failed` carries the full §0.4.3 `IpcError` (kind + message + path/residue DISPLAY) the live row needs;
/// `Succeeded` the display-only output form; `Skipped` the §0.6 `SkipReason`; `Cancelled` is payload-free.
///
/// [Build-Session-Entscheidung: P2.12 → P3.76] `Serialize` + `Type` (wire — the `ItemFinished` payload), NO
/// `Deserialize` (outbound-only — embeds the outbound-only `IpcError`); NOT `Copy` (`Failed` owns an
/// `IpcError` with `String`s). Externally tagged with `#[serde(rename_all = "camelCase")]` (the §0.6
/// wire-enum convention) + per-struct-variant `rename_all` (serde does not cascade the enum-level rename to
/// a variant's fields, so `Succeeded`'s `output_display` → `outputDisplay` needs its own, cf.
/// `CollectedSet`). Variant order matches §0.6 exactly. P3.76 re-types `Succeeded { output_path: PathBuf }`
/// → `Succeeded { output_display: String }` — no `PathBuf` on the wire (2026-07-06 ruling, §2.10.1); the
/// real output `PathBuf` lives in `RunResultStore` (`RunResultPaths.item_outputs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ItemOutcome {
    /// Converted + atomically published (§2.1) — carries the display-only output form (last-step
    /// `to_string_lossy`, §2.10.1); the real output `PathBuf` is `RunResultStore`-side.
    #[serde(rename_all = "camelCase")]
    Succeeded { output_display: String },
    /// A named §2.8 failure — carries the full §0.4.3 `IpcError` the live row renders.
    #[serde(rename_all = "camelCase")]
    Failed { error: IpcError },
    /// A skipped item — carries the §0.6 `SkipReason` (skip ≠ fail): a pre-flight detection-ineligible item
    /// (§1.2/§1.3) OR the §2.5.3 re-run skip (`AlreadyConverted`, the P3.48 ruling). Reserved for the §1.12
    /// terminal-projection path (no live `ItemFinished{Skipped}` is emitted — §0.4.2).
    #[serde(rename_all = "camelCase")]
    Skipped { reason: SkipReason },
    /// User-cancelled; nothing written (§1.7/§1.11). Not an `ErrorKind` (§0.4.3 note) — payload-free.
    Cancelled,
}

// ─── §2.6.4 cleanup-failure honesty — the `CleanupResidue` surfacing leg (P3.25) ─────────────────────
// [Build-Session-Entscheidung: P3.25] Homed HERE in crate::orchestrator (the §1.12 result family), NOT in
// crate::run: this leg PRODUCES the orchestrator wire type `CleanupResidue` and records the real residue
// `PathBuf` for `RunResultPaths.item_residues`, but `crate::run` is a §0.7 tier-2 domain-only LEAF (it
// depends DOWN only on crate::domain, never UP on orchestrator, run/mod.rs §0.7 note) — so the residue→
// `RunResult` mapping cannot live there without an illegal upward edge. The division of labour: `crate::run`'s
// `cleanup_item`/`cleanup_run` (P3.22) surface the RAW residue paths on the cleanup exit paths; THIS leg maps
// each into the honest §1.12 projection — the wire `CleanupResidue` (display-only) + the off-wire real
// `PathBuf` + the per-item §2.8.2 reason override + the "With residue" batch tail. The §2.8.2 strings it reads
// (the `CleanupResidue` catalog row + `WITH_RESIDUE_TAIL`) are crate::outcome's (P3.68). It is a pure primitive
// here — the P3.50 §1.12 run-end projection and the P3.38 write-sequence CALL it, so it is dead in the
// production build until then (the module-level dead_code expect covers it); unit-tested in
// `cleanup_honesty_tests`. (Per the conflict order spec §0.7 > code doc-comment, this corrects the stale
// forward-reference in `crate::outcome`'s catalog doc, which said `crate::run` reads the row — fixed in the
// same commit, DoD (b) / G68.)

/// One item's §2.6.4 cleanup-incomplete record — the item + the REAL residue `PathBuf` (retained for
/// [`RunResultPaths::item_residues`], the C9 `OpenTarget::Residue(item)` reveal target). The wire
/// [`CleanupResidue`] (display-only, folded into [`RunResult::cleanup_incomplete`]) is DERIVED on demand via
/// [`ResidueRecord::warning`] — NOT stored — so the SINGLE source of truth is the real path and the wire
/// display can never desync from the off-wire path it names (§2.10.1 / 2026-07-06 core-owned-paths ruling).
///
/// [Build-Session-Entscheidung: P3.25] Core-INTERNAL (no `serde`/`specta`) — it holds the off-wire real
/// `PathBuf`; only its DERIVED `warning` half ever crosses IPC. `Debug, Clone, PartialEq, Eq` (the
/// internal-type set, like [`RunResultPaths`]); NOT `Copy` (owns a `PathBuf`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidueRecord {
    /// The item whose cleanup did not complete (§2.6.4) — the [`RunResultPaths::item_residues`] key and the
    /// derived wire [`CleanupResidue::item`].
    pub item: ItemId,
    /// The REAL residue `PathBuf` — the SINGLE source of both the off-wire `item_residues` entry AND (via
    /// [`ResidueRecord::warning`]) the wire `residue_display`. Never crosses the wire directly (§2.10.1).
    pub real_path: PathBuf,
}

impl ResidueRecord {
    /// Record one item whose cleanup did not complete (§2.6.4), retaining the byte-verbatim real residue path
    /// as the single source of truth (the wire display is derived, not stored — see [`ResidueRecord::warning`]).
    /// [Build-Session-Entscheidung: P3.25]
    pub fn new(item: ItemId, residue: PathBuf) -> Self {
        Self {
            item,
            real_path: residue,
        }
    }

    /// The DERIVED wire [`CleanupResidue`] for this record — `residue_display` is the §2.10.1 last-step
    /// `to_string_lossy` of the real residue path (the ONLY place the summary names a residue, never a
    /// re-submittable path). Derived on demand from `real_path`, so the wire display can NEVER desync from the
    /// off-wire path it names. Panic-free (a single lossy projection, no fallibility).
    /// [Build-Session-Entscheidung: P3.25]
    pub fn warning(&self) -> CleanupResidue {
        CleanupResidue {
            item: self.item,
            residue_display: self.real_path.to_string_lossy().into_owned(),
        }
    }
}

/// The §2.6.4 terminal disposition of an item whose cleanup left residue — which of §2.6.4's three cases
/// applies, so the §1.12 projection carries the item HONESTLY (never a silent clean success). The residue
/// itself is surfaced IDENTICALLY in all three cases (a [`ResidueRecord`] folded into `cleanup_incomplete` +
/// the "With residue" batch tail); the disposition governs only the per-item `reason` OVERRIDE
/// ([`residue_item_reason`]):
///
/// - `Succeeded` (case 1): the item stays `Succeeded` and its reason carries the §2.8.2 case-1
///   [`crate::outcome::OutcomeMsg::Residue`] annotation — [Build-Session-Entscheidung: P3.59] the row is
///   quoted here VERBATIM from the §2.8.2 catalog ("Converted — a temporary file may remain at {path}."),
///   so its "temporary" is §02's own product wording about a leftover FILE, not a G8 deferral marker
///   about this code; the string may not be reworded (§5.7:799 — one string, one home)
///   — a NON-failure note in the `Lossy` shape, so the success stands while the summary still says residue may
///   remain and WHERE (§5.7:830). **[Test-Change: P3.59 — old-obsolete+new-correct, §2.6.4]** this arm
///   SUPERSEDES the P3.25 `Succeeded => None` rule, per the 2026-07-16 P3.59 Co-Pilot ruling. OLD OBSOLETE:
///   P3.25's rationale ("neither adopts the §2.8.2 `CleanupResidue` *failure* string") was CORRECT against the
///   failure-worded row — and stays correct: case 1 still does not adopt it. But it left §2.6.4:944's OWN
///   already-authored case-1 annotation **carrier-less**, and that gap is what forced the pre-ruling fill to
///   author a chrome string against §5.7:799's "the UI must not paraphrase" (the G1 NOGO). NEW CORRECT: §2.6.4
///   case 1 authors that sentence normatively and §2.8.2 now homes it (spec > code); the note rides a
///   non-failure variant, so the "not a failure string" invariant P3.25 protected is preserved intact.
/// - `Cancelled` (case 3): **RATIFIED exactly as built** — the residue does NOT rewrite the per-item reason
///   (`None`). §2.6.4 authors no per-item case-3 sentence: its complete per-item surface is the STRUCTURAL
///   `CleanupResidue` annotation (the rendered `residue_display` + the C9 reveal link), and the "With residue"
///   tail is BATCH-level (§2.8.2 02:1266/:1274) — routing it per-item would double-render it against the
///   `RunResult` summary line, and its pathless "see details" wording cannot satisfy §5.7:830's "with where
///   residue remains" anyway. `state` (not the message) is what distinguishes a stopped cancel.
/// - `Failed` (case 2): **unchanged as built** — the item is reported `Failed` WITH the combined §2.8.2
///   `CleanupResidue` message ("This file couldn't be converted, and a temporary file may remain at {path}.")
///   — never a clean success.
///
/// [Build-Session-Entscheidung: P3.25] The three variants mirror the item's terminal `JobState` (§1.9) — the
/// state the residue attaches to — so a `ResidueDisposition::Failed` names exactly the `JobState::Failed` case.
/// `Debug, Clone, Copy, PartialEq, Eq` (a fieldless enum, so `Copy` is free); NO wire derives (core-internal —
/// it drives the projection, never crosses IPC). The exhaustive match in [`residue_item_reason`] (G4/G14) makes
/// a fourth §2.6.4 case a COMPILE-TIME decision, so the honesty rule can never silently fall behind the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidueDisposition {
    /// §2.6.4 case 1 — the output published but its temp couldn't be removed; the item stays `Succeeded`.
    Succeeded,
    /// §2.6.4 case 2 — the item failed AND its partial couldn't be cleaned; reported `Failed` with residue.
    Failed,
    /// §2.6.4 case 3 — a cancelled item's publish temp survived the §1.7 group-kill wait; stays `Cancelled`.
    Cancelled,
}

/// The per-item §2.8.2 `reason` a residue imposes for the given §2.6.4 disposition, read from the
/// crate::outcome catalog (P3.68/P3.59): the combined `CleanupResidue` FAILURE message for `Failed` (never a
/// clean success — case 2), the NON-failure [`crate::outcome::OutcomeMsg::Residue`] annotation for `Succeeded`
/// (case 1 — the success stands, with where residue remains), and `None` for `Cancelled` (case 3 — the
/// terminal `state` carries the meaning; the residue is surfaced structurally via `cleanup_incomplete` + the
/// batch tail). In NO case does the residue rewrite the item's terminal `JobState` — see [`ResidueDisposition`]
/// for the per-case rationale + the P3.59 supersession of P3.25's `Succeeded => None` arm. `residue_display`
/// is the same §2.10.1 display string carried in the item's [`ResidueRecord::warning`], substituted into the
/// row's `{path}` slot. Because the §2.8.2 `CleanupResidue` row IS homed ([`crate::outcome::conversion_failure`]
/// returns `Some` for it) `Failed` always yields `Some`, and [`crate::outcome::residue_annotation`] is
/// infallible, so `Succeeded` always yields `Some`; the exhaustive match forces a fourth §2.6.4 case to decide
/// its reason explicitly. Panic-free. [Build-Session-Entscheidung: P3.25 → P3.59]
pub fn residue_item_reason(
    disposition: ResidueDisposition,
    residue_display: &str,
) -> Option<OutcomeMsg> {
    match disposition {
        ResidueDisposition::Failed => {
            crate::outcome::conversion_failure(ConversionErrorKind::CleanupResidue, residue_display)
        }
        // §2.6.4 case 1 (P3.59, superseding P3.25's `None`): the promoted §2.8.2 non-failure annotation.
        ResidueDisposition::Succeeded => Some(crate::outcome::residue_annotation(residue_display)),
        // §2.6.4 case 3 (RATIFIED as built): no per-item sentence exists to carry — the tail is batch-level.
        ResidueDisposition::Cancelled => None,
    }
}

/// Split a run's §2.6.4 residue records into the §1.12 projection's two honest halves: the WIRE
/// `Vec<CleanupResidue>` for [`RunResult::cleanup_incomplete`] and the OFF-WIRE `BTreeMap<ItemId, PathBuf>` for
/// [`RunResultPaths::item_residues`] — the ONE place the §2.10.1 wire↔off-wire split is applied, so the P3.50
/// projection cannot leak a residue path onto the wire. Order-preserving for the wire list (it mirrors what the
/// user sees); a repeated `ItemId` keeps the LAST real path in the off-wire map while retaining EVERY warning.
/// Panic-free. [Build-Session-Entscheidung: P3.25]
pub fn split_residue_records(
    records: Vec<ResidueRecord>,
) -> (Vec<CleanupResidue>, BTreeMap<ItemId, PathBuf>) {
    let mut warnings = Vec::with_capacity(records.len());
    let mut real_paths = BTreeMap::new();
    for record in records {
        warnings.push(record.warning());
        real_paths.insert(record.item, record.real_path);
    }
    (warnings, real_paths)
}

/// Append the §2.8.2 "With residue" tail to a §1.12 batch-summary line iff the run left any residue
/// (`has_residue` = `cleanup_incomplete` non-empty) — the run-level honesty that "temporary files may remain"
/// is always stated, never dropped (§2.6.4, esp. the case-3 wedged-cancel gap). Reads crate::outcome's
/// [`WITH_RESIDUE_TAIL`](crate::outcome::WITH_RESIDUE_TAIL) (P3.68); a residue-free run returns the line
/// verbatim. Panic-free. [Build-Session-Entscheidung: P3.25]
pub fn append_residue_tail(summary_line: String, has_residue: bool) -> String {
    if has_residue {
        format!("{summary_line} {}", crate::outcome::WITH_RESIDUE_TAIL)
    } else {
        summary_line
    }
}

// ─── §1.12 end-of-batch RunResult projection + batch-summary classifier (P3.50) ──────────────────────
// [Build-Session-Entscheidung: P3.50] The §1.12 run-end projection the box names: map a TERMINAL `Batch`
// (every job left Pending/Running) + its per-item outputs/residues/roots onto the wire `RunResult`
// (display-only, §2.10.1) PLUS its off-wire `RunResultPaths` (real paths for the C9 OpenTarget resolution).
// PURE (no I/O, no spawn) — the P3.48 conductor supplies the artifacts (the per-item published output paths,
// the §2.6.4 residue records, the §2.7 roots) and emits the result as `RunFinished`; C8 `get_run_summary`
// re-serves the retained wire half. Homed in `crate::orchestrator` (tier 1, the §1.12 result-family owner,
// §0.7): it composes the §1.9 `Batch`/`JobState` + the §2.6.4 residue helpers (P3.25) + the §2.8.2 catalog
// (`crate::outcome`, P3.68/P3.50). LIVE since P3.48 — the C6 conductor calls it (see the module `reason=`).

/// The §2.6.4 residue DISPOSITION an item's terminal §1.9 `JobState` implies (P3.50) — which of the three
/// §2.6.4 cases a residue on this item is, so the per-item reason is HONEST: `Failed` adopts the combined
/// §2.8.2 `CleanupResidue` FAILURE message, `Succeeded` the §2.8.2 non-failure `Residue` ANNOTATION (P3.59 —
/// superseding P3.25's no-reason grouping of the two non-failure cases), and `Cancelled` no reason at all
/// (§2.6.4 authors none; its terminal state carries the meaning). See [`ResidueDisposition`]. `None` for a state
/// that cannot carry run-end residue (`Pending`/`Running` never terminate here; a pre-flight `Skipped` never
/// ran, so it has no temp). Exhaustive over `JobState` (no `_`, G4/G14). [Build-Session-Entscheidung: P3.50]
fn residue_disposition_of(state: JobState) -> Option<ResidueDisposition> {
    match state {
        JobState::Succeeded => Some(ResidueDisposition::Succeeded),
        JobState::Failed(_) => Some(ResidueDisposition::Failed),
        JobState::Cancelled => Some(ResidueDisposition::Cancelled),
        JobState::Pending | JobState::Running | JobState::Skipped(_) => None,
    }
}

/// The base per-item §2.8.2 `reason` for a terminal job (before any §2.6.4 residue override) — the resolved,
/// ready-to-show line (`ItemResult.reason`, §0.6): a `Failed(kind)` renders its §2.8.2 catalog row, a
/// `Skipped(reason)` renders its `OutcomeMsg::Skipped` line, and a plain `Succeeded`/`Cancelled` carries none.
/// TWO skip shapes: a pre-flight DETECTION skip (`JobSource::Skipped`) passes its retained
/// `SkippedItem.detected_display` into `skipped_message` (the `{detected}` substitution, P3.50); the §2.5.3
/// re-run skip `Skipped(AlreadyConverted)` (`JobSource::Eligible`) has NO `detected_display` (`None`) and its
/// line renders DIRECTLY (the P3.48 ruling — never via the `SkipReason→ErrorKind` bridge). The coupling
/// `source is Skipped(_) ⟺ state is Skipped(<detection reason>)` (P3.47) guarantees the detected name is
/// present exactly when it is needed. [Build-Session-Entscheidung: P3.50 → P3.48]
fn item_base_reason(job: &ConversionJob, name_arg: Option<&str>) -> Option<OutcomeMsg> {
    match job.state {
        JobState::Skipped(reason) => {
            // The retained detected-type name rides the JobSource::Skipped arm (coupling-guaranteed present).
            let detected = match &job.source {
                JobSource::Skipped(skipped) => skipped.detected_display.as_deref(),
                JobSource::Eligible(_) => None,
            };
            Some(crate::outcome::skipped_message(reason, detected))
        }
        // §2.2.4 (P3.88): `name_arg` fills the `UnopenableOutputName` `{name}` slot with the offending token so
        // the terminal reason NAMES it, matching the live message; `None` (every other Failed kind is a
        // no-substitution row) renders the full string with the empty `arg`. §2.8 / §1.12 (P3.75 sweep): a
        // mis-homed app-level kind ({EngineMissing, WebviewFault, BundleDamaged, MixedDrop}) has no §2.8.2 row →
        // `conversion_failure` returns `None`; the same `InternalError` fallback the live `failure_message` /
        // `project_outcome` siblings carry keeps this TERMINAL projection never-message-less too (a failed item
        // is never message-less — the two projections of one item must agree).
        JobState::Failed(kind) => crate::outcome::conversion_failure(kind, name_arg.unwrap_or(""))
            .or_else(|| crate::outcome::conversion_failure(ConversionErrorKind::InternalError, "")),
        JobState::Succeeded | JobState::Cancelled | JobState::Pending | JobState::Running => None,
    }
}

/// Project a TERMINAL `Batch` onto the §1.12 wire `RunResult` (display-only) + its off-wire `RunResultPaths`
/// (P3.50). For every job it emits one `ItemResult { item, output_display, state, reason }`: `output_display`
/// is the published output's lossy display ONLY for a `Succeeded` item (§1.12, §2.10.1), and `reason` is the
/// §2.8.2 line — a §2.6.4 residue supplies it per the item's case (P3.59): `Failed` the combined
/// `CleanupResidue` FAILURE message (never a clean success, case 2), `Succeeded` the NON-failure `Residue`
/// annotation (case 1), `Cancelled` none (case 3 — the base line, i.e. `None`); otherwise the base line
/// ([`item_base_reason`]). The residue never rewrites the item's terminal `state`. Pre-flight SKIPS
/// (the `JobSource::Skipped` jobs materialised at C6, P3.47) ride through identically — `Skipped(reason)`
/// state, `output_display: None`, an `OutcomeMsg::Skipped` reason — counted in `Totals.skipped`, NEVER
/// `failed` (skip ≠ fail, §1.12). The §2.6.4 residues split into the wire `cleanup_incomplete` + the off-wire
/// `item_residues` ([`split_residue_records`], P3.25), so the run is never reported a clean success while a
/// temp may remain. The real roots + per-item output/residue `PathBuf`s ride the off-wire `RunResultPaths`
/// (C9 resolves its `OpenTarget` against it, P3.79); the wire carries only their `to_string_lossy` displays
/// (§2.10.1 / 2026-07-06 ruling). §2.2.4 (P3.88): `failed_name_args` supplies the per-item offending token for an
/// `UnopenableOutputName` failure so the terminal reason NAMES it (empty for a run with none). PURE — the P3.48
/// conductor supplies `item_outputs`/`failed_name_args`/`residues`/roots from the live run. [Build-Session-Entscheidung: P3.50, P3.88]
pub fn project_run_result(
    batch: &Batch,
    run_id: RunId,
    item_outputs: &BTreeMap<ItemId, PathBuf>,
    failed_name_args: &BTreeMap<ItemId, String>,
    residues: Vec<ResidueRecord>,
    common_root: PathBuf,
    divert_root: Option<PathBuf>,
) -> (RunResult, RunResultPaths) {
    // §2.6.4 honesty split: the wire warnings (order-preserving) + the off-wire real residue paths (P3.25).
    let (cleanup_incomplete, item_residues) = split_residue_records(residues);
    // Per-item residue-display lookup for the §2.6.4 Failed-with-residue reason override.
    let residue_displays: BTreeMap<ItemId, &str> = cleanup_incomplete
        .iter()
        .map(|warning| (warning.item, warning.residue_display.as_str()))
        .collect();

    let mut items = Vec::with_capacity(batch.jobs.len());
    for job in &batch.jobs {
        // §1.12: the published output display is carried ONLY for a Succeeded item.
        let output_display = match job.state {
            JobState::Succeeded => item_outputs
                .get(&job.item)
                .map(|path| path.to_string_lossy().into_owned()),
            JobState::Failed(_)
            | JobState::Skipped(_)
            | JobState::Cancelled
            | JobState::Pending
            | JobState::Running => None,
        };
        // §2.2.4 (P3.88): pass the item's offending token (present only for an `UnopenableOutputName` failure)
        // so the base reason NAMES it, matching the live message.
        let base_reason =
            item_base_reason(job, failed_name_args.get(&job.item).map(String::as_str));
        // §2.6.4 (P3.59): a residue supplies the item's reason per its case — Failed the combined
        // CleanupResidue FAILURE message (never a clean success, case 2); Succeeded the NON-failure Residue
        // ANNOTATION (case 1 — the success stands, with where residue remains); Cancelled nothing (case 3 —
        // `residue_item_reason` yields None, so `.or(base_reason)` falls through and the structural
        // cleanup_incomplete entry + the batch tail carry it). In no case is the terminal `state` rewritten.
        // PRECEDENCE: a residue reason SHORT-CIRCUITS `.or(base_reason)`. Reachable only for case 1 (the other
        // two dispositions have no base reason to lose: `item_base_reason` is None for Succeeded/Cancelled),
        // and unreachable even there in the P3 slice, which emits no §2.9 Lossy. The rule for an item carrying
        // BOTH a lossy note and a case-1 annotation is P4.69's named forward point (the 2026-07-16 ruling's
        // Forward clause) — it is a consequence of that ruling's carrier choice, not a silent drop here.
        let reason = match (
            residue_displays.get(&job.item).copied(),
            residue_disposition_of(job.state),
        ) {
            (Some(residue_display), Some(disposition)) => {
                residue_item_reason(disposition, residue_display).or(base_reason)
            }
            _ => base_reason,
        };
        items.push(ItemResult {
            item: job.item,
            output_display,
            state: job.state,
            reason,
        });
    }

    let totals = Totals {
        succeeded: tally(batch, |state| matches!(state, JobState::Succeeded)),
        failed: tally(batch, |state| matches!(state, JobState::Failed(_))),
        cancelled: tally(batch, |state| matches!(state, JobState::Cancelled)),
        skipped: tally(batch, |state| matches!(state, JobState::Skipped(_))),
    };

    // Compute the display forms BEFORE moving the real roots into the off-wire table (§2.10.1).
    let common_root_display = common_root.to_string_lossy().into_owned();
    let divert_root_display = divert_root
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned());
    let paths = RunResultPaths {
        common_root,
        divert_root,
        item_outputs: item_outputs.clone(),
        item_residues,
    };
    // §1.12 (P3.59): assemble the §2.8.2 batch line + its §2.6.4 tail HERE, in the core — the one place that
    // holds both halves (`totals` + whether any residue survived). Computed BEFORE `cleanup_incomplete` moves
    // into the struct. `batch_summary_line` is the P3.50 build; this is its first production caller.
    let summary_line_display = batch_summary_line(&totals, !cleanup_incomplete.is_empty());
    let result = RunResult {
        collected_set_id: batch.id,
        run_id,
        items,
        totals,
        cleanup_incomplete,
        common_root_display,
        divert_root_display,
        summary_line_display,
    };
    (result, paths)
}

/// Count the batch's jobs whose terminal `JobState` matches `pred` (§1.12 `Totals`), returning a `u32`. The
/// count is `usize` (bounded by `batch.jobs.len()`); the `u32::try_from` cannot realistically saturate (the
/// §0.6 single id space is `u32`, so a batch holds at most `u32::MAX + 1` jobs), and the `u32::MAX` cap keeps
/// it panic-free at the impossible boundary rather than truncating. [Build-Session-Entscheidung: P3.50]
fn tally(batch: &Batch, pred: impl Fn(&JobState) -> bool) -> u32 {
    u32::try_from(batch.jobs.iter().filter(|job| pred(&job.state)).count()).unwrap_or(u32::MAX)
}

/// Classify a run's §1.12 `Totals` into its §2.8.2 [`BatchSummary`] situation (P3.50). The headline reflects
/// the CONVERSION disposition over the ATTEMPTED items (`succeeded + failed`); pre-flight SKIPS are excluded
/// from the headline `{n}` — they never entered the queue and are not a conversion outcome, so they appear
/// only in `RunResult.items` + `Totals.skipped` (the skip ≠ fail posture, §1.12), never inflating "All {n}
/// converted". A user cancel DOMINATES: a run with any `cancelled` item is `Stopped.` (its `{ok}` = the
/// finished-before-cancel successes). Otherwise: no failures → `AllSucceeded`; no successes → `AllFailed`
/// (an explicit failure, SSOT *Fail clearly*, never a quiet finish); a mix → `Partial`.
/// [Build-Session-Entscheidung: P3.50 — {n} = attempted (succeeded + failed), skips excluded from the headline]
#[must_use]
pub fn batch_summary(totals: &Totals) -> crate::outcome::BatchSummary {
    use crate::outcome::BatchSummary;
    // u32 -> usize is a widening on every supported (32-/64-bit) target — never truncates.
    let ok = totals.succeeded as usize;
    let fail = totals.failed as usize;
    if totals.cancelled > 0 {
        // A user cancel dominates the headline (§2.8.2 Cancelled): finished-before-cancel items are kept.
        BatchSummary::Cancelled { ok }
    } else {
        // The headline is over the attempted (succeeded + failed) items; pre-flight skips are excluded.
        let n = ok.saturating_add(fail);
        if fail == 0 {
            BatchSummary::AllSucceeded { n }
        } else if ok == 0 {
            BatchSummary::AllFailed { n }
        } else {
            BatchSummary::Partial { ok, n, fail }
        }
    }
}

/// The full §1.12 batch-level summary LINE (P3.50) — the [`batch_summary`] situation's §2.8.2 text with the
/// "With residue" tail appended iff the run left any residue (`has_residue` = `RunResult.cleanup_incomplete`
/// non-empty). This is the one place the §2.8.2 batch line + the §2.6.4 honesty tail are assembled; the §5.3
/// Summary UI (or the `RunFinished` emitter) renders it from `RunResult.totals` + `cleanup_incomplete`.
/// [Build-Session-Entscheidung: P3.50]
#[must_use]
pub fn batch_summary_line(totals: &Totals, has_residue: bool) -> String {
    append_residue_tail(batch_summary(totals).text(), has_residue)
}

// ─── §2.1.1 per-item PUBLISH LEGS (P3.38 → re-cut P3.48) ──────────────────────────────────────────────
// [Build-Session-Entscheidung: P3.38 → P3.48] Homed HERE in crate::orchestrator (tier 1) per the 2026-07-07
// home ruling (§0.7 > the plan-cluster heading): the sequence COMPOSES `crate::run` (temp/cleanup) +
// `crate::fs_guard` (publish/divert) + the engine step — and ONLY the tier-1 orchestrator may compose all
// three (`fs_guard` depends DOWN only; `run`/`fs_guard` are mutually-independent siblings, so a deliberate
// `run`→`fs_guard` edge is rejected, §0.7). The §2.1.1 steps 3-6 (sync → resolve-late → exclusive publish →
// dir-fsync + the §2.14.3 EXDEV fallback + the §2.2.2 numbering ↔ no-clobber loop) all live INSIDE
// `fs_guard::atomic_publish` (P3.15/P3.16/P3.17); [`publish_written_temp`] wires those + step 7
// (`run::cleanup_item`, P3.22) around an ALREADY-WRITTEN, non-empty-verified publish temp.
//
// **The P3.48 composition re-cut (the 2026-07-12 ruling — option ②: pick-temp → await dispatch → publish
// legs).** P3.38's original `write_item` bundled all seven steps around a SYNCHRONOUS engine-write `FnOnce`
// seam; P3.48 RE-CUT it because the §1.7 dispatch is `async` (spawn/kill/cancel) and a synchronous `FnOnce`
// seam can never host it (the ruling's rationale (b): option ① — the `write_item` spine — dies at P4). So the
// per-item sequence is composed TIER-1 in the [`crate::orchestrator`] conductor (`run_conversion`, §1.9):
// step 1 (`RunScratch::publish_temp`) is conductor-side, step 2 (the write) is the awaited
// `engines::dispatch`, the §1.7 non-empty exit-verification runs on the conductor's `Succeeded` path, and
// ONLY on `InvocationResult::Succeeded` does the conductor call [`publish_written_temp`] for steps 3-7 (a
// `Cancelled`/`Failed` invocation drops the temp + projects directly, never reaching the publish legs —
// §2.1.1 step 2's `[CLARIFIED 2026-07-12]`). Every guarantee-bearing piece (sync, resolve-late, exclusive
// publish, dir-fsync, cleanup-on-error, late-divert) stays LIVE on the run path — the re-cut moved the seam,
// it did not deaden any leg.
//
// The §2.7.2/§2.7.5 LATE-DIVERT is composed here (not merely surfaced): a §2.1.1 sequence that failed an item on
// a mid-write writability flip / FAT-exFAT `NoAtomicPublishSupport` instead of diverting would DEGRADE the §2.7.5
// "not a degraded path" guarantee (SSOT Principle-5). P3.17/P3.35/P3.36 authored `atomic_publish` /
// `resolve_divert_target` / `is_write_divert_trigger` / `publish_to_divert` with THIS leg named as their
// production caller (P3.35/P3.36 are earlier `[x]` boxes, so no explicit `needs:` edge is required — the §04
// divert primitives are already built). One divert per item (§2.7.3) — a failed divert is terminal, never
// re-diverted. The whole surface goes LIVE when the P3.48 conductor makes `start_conversion` a live root.

/// The terminal disposition of one §2.1.1 per-item publish ([`publish_written_temp`]) — the output published
/// (the real path, retained core-side for `RunResultPaths.item_outputs` + the §1.12 display projection) or a
/// named §2.8 failure (one item failed, the batch continues, §1.9). Core-INTERNAL (holds the real output
/// `PathBuf`, never a wire type); the P3.48 conductor maps it onto the wire `JobState`/`ItemOutcome`.
///
/// [Build-Session-Entscheidung: P3.38] `Debug, Clone, PartialEq, Eq` — the internal-type set (no `serde`/
/// `specta`, like `OutputPlan`/`ResidueRecord`); NOT `Copy` (owns a `PathBuf`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteDisposition {
    /// Converted + atomically published (§2.1) — the real output path (the §2.2 no-clobber name resolved LAZILY
    /// at write time, `final_dir.join(leaf)`), retained core-side for `RunResultPaths.item_outputs`.
    Published { output: PathBuf },
    /// A named §2.8 failure — one item failed, the batch continues (§1.9). The `kind` is the §2.8 taxonomy code
    /// the FSM renders via the P3.68 catalog.
    Failed { kind: ConversionErrorKind },
}

/// The full result of one §2.1.1 per-item publish ([`publish_written_temp`]) — the terminal
/// [`WriteDisposition`], whether the output DIVERTED (§2.7.3, so the run's `divert_root_display` is set), and
/// any §2.6.4 cleanup residue (a temp that could not be removed, so the item is never reported as a silent
/// clean success — the P3.25 honesty leg). Core-INTERNAL (a [`ResidueRecord`] holds the off-wire real
/// `PathBuf`). The P3.48 conductor also maps a `Failed`-with-residue into the live `ItemFinished` `IpcError`
/// (kind + `residue_display`) via `write_outcome_to_run` + `failure_message`.
///
/// [Build-Session-Entscheidung: P3.38] `Debug, Clone, PartialEq, Eq`; NOT `Copy` (embeds a `WriteDisposition` +
/// an `Option<ResidueRecord>`, both owning `PathBuf`s). The §1.12 projection maps `(disposition, residue)` onto
/// the §2.6.4 [`ResidueDisposition`] (a `Published` → case 1; a `Failed` → case 2) via [`residue_item_reason`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteOutcome {
    /// Published(real path) or Failed(§2.8 kind).
    pub disposition: WriteDisposition,
    /// `true` when the output landed on the §2.7.3 DIVERT root — a proactively-diverted plan (`OutputPlan.
    /// diverted.is_some()`) that published, OR a write-time late-divert. Drives `RunResult.divert_root_display`.
    /// `false` on a beside-source publish or any failure.
    pub diverted: bool,
    /// `Some` when §2.6 cleanup left a temp behind (§2.6.4) — recorded for `cleanup_incomplete` + the off-wire
    /// residue table so the summary never reports a clean success. `None` when every temp was removed.
    pub residue: Option<ResidueRecord>,
    /// §2.2.4 (P3.88): the offending CONSTRUCTED token when `disposition` is `Failed(UnopenableOutputName)` — the
    /// leaf name Windows cannot open (a reserved DOS device / trailing dot-space), threaded so the §2.8 message
    /// NAMES it (§2.2.4). `None` for every other outcome. `write_outcome_to_run` rides it onto
    /// `ItemRunOutcome::Failed.name_arg`; a non-`Failed`/non-unopenable outcome leaves it `None`.
    pub name_arg: Option<String>,
}

impl WriteOutcome {
    /// A failure with no temp to reconcile (a pre-write step failed before any `.part` existed) — no residue.
    /// [Build-Session-Entscheidung: P3.38]
    fn failed(kind: ConversionErrorKind) -> Self {
        Self {
            disposition: WriteDisposition::Failed { kind },
            diverted: false,
            residue: None,
            name_arg: None,
        }
    }
}

/// The borrowed per-item inputs the §2.1.1 write sequence threads through its steps (a bundle, not eight
/// re-passed args) — the per-job plan/source/ext + the frozen-source identities (§2.3.3 link-safety) + the
/// §2.7.3 divert candidate roots + the live run handle (§2.6 temp ownership). The mutable `LocationCache` and the
/// `crate::run`-grammar `probe_name` factory (both needed only on the divert path) travel separately.
/// [Build-Session-Entscheidung: P3.38]
struct WriteInputs<'a> {
    plan: &'a OutputPlan,
    /// The §2.3-resolved read path (the output base name comes from its verbatim stem, §2.2.1; §1.7 hands it in).
    source: &'a Path,
    /// The target's bare canonical extension (`tsv`/`csv`) — ASCII by construction (§04).
    ext: &'a str,
    /// The frozen source-file identities (§2.4) the §2.3.3 link-safety re-check runs against.
    frozen_sources: &'a [FileIdentity],
    /// The §2.7.3 divert candidate roots (caller-resolved AppHandle-side, the P3.35 contract), tried in order.
    divert_candidates: &'a [PathBuf],
    /// The live run handle (§2.6.3 lock-before-part) — mints the run-owned `.part` temps + names the cleanup.
    scratch: &'a RunScratch,
}

/// **§2.1.1 the per-item PUBLISH LEGS (P3.38 → re-cut P3.48)** — sync → resolve-late → publish → dir-fsync →
/// cleanup-on-error, with the §2.7.2/§2.7.5 late-divert, over the ALREADY-WRITTEN, non-empty-verified publish
/// `tmp` the §1.9 conductor (`run_conversion`) hands in. Consumes the §1.8 [`OutputPlan`] (P3.37), producing
/// the terminal [`WriteOutcome`] the conductor maps onto the `ItemFinished` `ItemOutcome` + the §1.12
/// `RunResult` projection.
///
/// **What the conductor did BEFORE this (the P3.48 re-cut, ruling option ②):** §2.1.1 step 1 — picked the
/// publish temp on `final`'s volume ([`RunScratch::publish_temp`], P3.20); step 2 — the §1.7 `engines::dispatch`
/// WROTE the engine's output into `tmp` (never `final`, §3.5); and the §1.7 exit-verification GATED this call —
/// it runs ONLY on `InvocationResult::Succeeded` with a temp that exists and is non-empty (an empty/vanished
/// output is a §2.8 `Empty`/`InternalError` the conductor fails BEFORE reaching here; a `Failed`/`Cancelled`
/// invocation never reaches here). So this fn is steps 3-7: **(3-6)** sync → resolve-late + §2.3.3 link-safety →
/// the §2.2.2 numbering ↔ no-clobber exclusive publish → dir-fsync, all inside [`atomic_publish`]
/// (P3.15/P3.16, with the §2.14.3 EXDEV fallback P3.17); **(7)** on any error in 3-6 remove `tmp` — `final` was
/// never created ([`cleanup_item`], P3.22).
///
/// **Late-divert (§2.7.2/§2.7.5).** A `ResolvesOntoSource` parent (§2.3.3), a FAT/exFAT `NoAtomicPublishSupport`
/// (§2.7.2, Unix), or a writability publish failure ([`is_write_divert_trigger`] — USB pulled / share dropped /
/// permission flip) routes the completed `tmp` to the §2.7.3 divert target ([`resolve_divert_target`] →
/// [`publish_to_divert`]) — the FULL safety chain, not a degraded path (§2.7.5). ONE divert per item (§2.7.3): a
/// plan already diverted, or a failed divert, is terminal (§2.8 `WriteFailed`), never re-diverted.
///
/// A non-UTF-8 target extension is an internal fault, never user-facing — the temp is removed (it is already
/// written by this point, unlike the pre-re-cut `write_item` where the ext check preceded the temp pick) and
/// the item fails `InternalError`. No panic (the crate no-panic deny, G4/G14) — every failure is a structured
/// [`WriteOutcome`]; the source bytes are never touched (the no-harm G32(a) invariant the tests assert).
/// [Build-Session-Entscheidung: P3.38 → P3.48]
#[allow(clippy::too_many_arguments)]
// [Build-Session-Entscheidung: P3.38 → P3.48] Each arg is a DISTINCT §2.1.1 input (the plan/source/frozen-set/
// divert roots/run handle/cache/probe-name factory/the written temp) — the `compute_output_plan` (P3.37)
// precedent; a mechanical bundle struct would group them without semantic value. The re-cut REPLACED the old
// `engine_write: impl FnOnce` seam with `tmp: TempPath` (the conductor's already-written publish temp).
pub fn publish_written_temp(
    plan: &OutputPlan,
    source: &Path,
    frozen_sources: &[FileIdentity],
    divert_candidates: &[PathBuf],
    scratch: &RunScratch,
    cache: &mut LocationCache,
    probe_name: impl Fn() -> OsString,
    tmp: TempPath,
) -> WriteOutcome {
    let item = plan.job;
    // The target's canonical extension is ASCII (§04); a non-UTF-8 ext is an internal fault, never user-facing.
    // The temp is ALREADY written here (the re-cut moved the pick/write conductor-side), so a failing ext check
    // removes it (never a silent leftover) — unlike the pre-re-cut `write_item`, whose ext check preceded step 1.
    let Some(ext) = plan.extension.to_str() else {
        return fail_cleanup(item, [tmp], ConversionErrorKind::InternalError);
    };
    let inputs = WriteInputs {
        plan,
        source,
        ext,
        frozen_sources,
        divert_candidates,
        scratch,
    };

    // §2.1.1 steps 3-6 (+ the §2.7.2/§2.7.5 late-divert) over the conductor's written, non-empty-verified temp.
    publish_completed(&inputs, cache, &probe_name, tmp)
}

/// §2.1.1 steps 4-6 over the completed `tmp`, with the §2.7 divert routing (P3.38). The §2.3.3 link-safety +
/// no-clobber decision is resolved AS LATE AS POSSIBLE (immediately before the create, to shrink the TOCTOU
/// window); [`atomic_publish`] then runs steps 3/5/6 (sync → exclusive publish → dir-fsync + the §2.14.3
/// cross-volume fallback). [Build-Session-Entscheidung: P3.38]
fn publish_completed(
    inputs: &WriteInputs,
    cache: &mut LocationCache,
    probe_name: &impl Fn() -> OsString,
    tmp: TempPath,
) -> WriteOutcome {
    let item = inputs.plan.job;
    let final_dir = &inputs.plan.final_dir;
    // §2.1.1 step 4: resolve the no-clobber decision + §2.3.3 link-safety AS LATE AS POSSIBLE — open `final_dir`
    // as a TOCTOU-closed pinned handle whose resolved identity is not a frozen source (P3.9).
    let parent = match open_verified_parent_dir(final_dir, inputs.frozen_sources) {
        Ok(ParentDirVerdict::Verified(parent)) => parent,
        // §2.3.3 rule 2: `final_dir` resolves onto a frozen source → never publish there → §2.7 divert (P3.8).
        Ok(ParentDirVerdict::ResolvesOntoSource) => {
            return divert_completed(inputs, cache, probe_name, tmp);
        }
        // The parent could not be opened / verified (gone / not a directory) → §2.8 WriteFailed.
        Err(_) => return fail_cleanup(item, [tmp], ConversionErrorKind::WriteFailed),
    };
    // The §2.14.3 EXDEV intermediate — a run-owned same-volume `.part` sibling of `final`, COMPUTED NAME-ONLY
    // (never created here). `atomic_publish` materialises it (via `std::fs::copy`) ONLY on the cross-volume
    // fallback; on the common same-volume publish — §2.14.1, where `tmp` IS a same-volume sibling of `final`, so
    // the rename is intra-volume and EXDEV cannot fire — it stays a bare name. Name-only BY DECISION: an EAGER
    // mint in `final_dir` would (a) FAIL, pre-empting the §2.7.2 writability-flip late-divert below, if `final`'s
    // dir flipped read-only DURING the seconds-long engine write, and (b) waste a create+unlink per item. A rare
    // cross-volume-crash materialisation is §2.6.3-sweep-reclaimable via its run-grammar name.
    let intermediate = inputs.scratch.publish_temp_path(final_dir, item);
    // §2.2.1: the lazy candidate names from the SOURCE stem + the target ext (a frozen source always has a stem —
    // a stemless source is an internal fault here, never a panic).
    let Some(candidates) = output_name(inputs.source, inputs.ext) else {
        return fail_cleanup(item, [tmp], ConversionErrorKind::InternalError);
    };
    // §2.1.1 steps 3/5/6: sync → the §2.2.2 numbering ↔ no-clobber exclusive publish → dir-fsync (all inside
    // atomic_publish, with the §2.14.3 cross-volume fallback).
    match atomic_publish(&parent, final_dir, tmp.as_ref(), candidates, &intermediate) {
        // Published beside-source — the same-volume publish renamed `tmp` onto `final` (the name-only
        // `intermediate` was never materialised), so `tmp` is the sole leftover (already consumed → `cleanup_item`
        // NotFound → Ok). Clean it explicitly (§2.6.2).
        Ok(PublishOutcome::Published { leaf, .. }) => finish_published(
            item,
            final_dir.join(leaf),
            inputs.plan.diverted.is_some(),
            [tmp],
        ),
        // §2.7.2 FAT/exFAT (Unix): no create-only atomic publish here → divert (one divert per item). The
        // name-only `intermediate` was never created, so there is nothing to clean.
        Ok(PublishOutcome::NoAtomicPublishSupport) => {
            divert_completed(inputs, cache, probe_name, tmp)
        }
        // §2.7.2 late-divert on a writability flip (USB pulled / share dropped / permission flip after the cached
        // probe); any other publish error maps straight to §2.8 (never a divert, §2.7.2). The name-only
        // `intermediate` was never created on this failed same-volume publish, so only `tmp` needs cleaning.
        Err(err) => {
            if inputs.plan.diverted.is_none() && is_write_divert_trigger(&err) {
                divert_completed(inputs, cache, probe_name, tmp)
            } else {
                let (kind, name_arg) = map_publish_error(&err);
                fail_cleanup_named(item, [tmp], kind, name_arg)
            }
        }
    }
}

/// §2.7.2/§2.7.5 the late-divert (P3.38) — resolve the §2.7.3 divert ROOT ([`resolve_divert_target`], P3.35) and
/// re-publish the completed `tmp` there through the FULL safety chain ([`publish_to_divert`], P3.36). ONE divert
/// per item (§2.7.3): a plan already diverted, or a failed divert, is terminal `WriteFailed` — never re-diverted.
/// [Build-Session-Entscheidung: P3.38]
fn divert_completed(
    inputs: &WriteInputs,
    cache: &mut LocationCache,
    probe_name: &impl Fn() -> OsString,
    tmp: TempPath,
) -> WriteOutcome {
    let item = inputs.plan.job;
    // §2.7.3 one divert per item: a plan whose beside-source location ALREADY diverted at C4 does not divert a
    // second time at write — a further failure is terminal (§2.8 WriteFailed).
    if inputs.plan.diverted.is_some() {
        return fail_cleanup(item, [tmp], ConversionErrorKind::WriteFailed);
    }
    // §2.7.3 resolve the divert ROOT — the first §2.7.2-writable candidate; none usable → §2.8 WriteFailed
    // (never divert onto a purgeable / another-FAT volume).
    let divert_dir = match resolve_divert_target(inputs.divert_candidates, cache, probe_name) {
        DivertTarget::Resolved(dir) => dir,
        DivertTarget::Unavailable => {
            return fail_cleanup(item, [tmp], ConversionErrorKind::WriteFailed)
        }
    };
    // The §2.14.3 intermediate for the divert publish — a run-owned `.part` sibling on the DIVERT volume. The
    // divert `tmp` sits on the ORIGINAL volume, so a divert onto a DIFFERENT volume crosses volumes and USES it
    // (the divert `tmp` is `std::fs::copy`'d into it). Minted REAL (not name-only): `divert_dir` was just
    // §2.7.2-verified WRITABLE by `resolve_divert_target`, so the mint cannot block, and a real [`TempPath`] is
    // cleaned by `cleanup_item` on every divert exit (§2.6.2) — the cross-volume case DOES create a file here.
    let Ok(intermediate) = inputs.scratch.publish_temp(&divert_dir, item) else {
        return fail_cleanup(item, [tmp], ConversionErrorKind::WriteFailed);
    };
    // §2.7.2/§2.7.5 the late-divert publish — re-runs the FULL safety chain (link-safety + §2.14.4 free-space +
    // §2.2.2 numbering + the exclusive publish, incl. the §2.14.3 cross-volume copy) on the divert target. NOT a
    // degraded path (§2.7.5).
    match publish_to_divert(
        &divert_dir,
        inputs.frozen_sources,
        inputs.source,
        inputs.ext,
        tmp.as_ref(),
        intermediate.as_ref(),
    ) {
        // Diverted output published — on the cross-volume path `tmp` was COPIED (left on the original volume) and
        // `intermediate` renamed onto `final`; on a same-volume divert `tmp` was renamed and `intermediate` is an
        // unused 0-byte sibling. Either way clean BOTH leftovers explicitly (§2.6.2). `diverted = true` (§2.7.3).
        Ok(PublishOutcome::Published { leaf, .. }) => {
            finish_published(item, divert_dir.join(leaf), true, [tmp, intermediate])
        }
        // The divert target is ALSO atomic-publish-incapable — `resolve_divert_target` already excludes those,
        // so this is defensive; never a second divert (§2.7.3) → §2.8 WriteFailed. Clean both temps (§2.6.2).
        Ok(PublishOutcome::NoAtomicPublishSupport) => {
            fail_cleanup(item, [tmp, intermediate], ConversionErrorKind::WriteFailed)
        }
        // A divert leg failed (link-safety / free-space / publish) — the ONE item fails clearly; no re-divert.
        // Clean both temps (§2.6.2) so a divert-volume leftover surfaces, never a silent drop. §2.2.4 is
        // re-checked identically on the divert path, so a divert-side unopenable leaf carries its token too.
        Err(err) => {
            let (kind, name_arg) = map_publish_error(&err);
            fail_cleanup_named(item, [tmp, intermediate], kind, name_arg)
        }
    }
}

/// Map a `crate::fs_guard` [`PublishError`] to its §2.8 [`ConversionErrorKind`] — the tier-1 boundary where the
/// leaf verdict becomes the wire taxonomy (§2.8; `crate::fs_guard` never depends up on `crate::outcome`). A
/// generic `Io` is a non-space destination write failure (§2.1/§2.7). [Build-Session-Entscheidung: P3.38]
fn map_publish_error(err: &PublishError) -> (ConversionErrorKind, Option<String>) {
    match err {
        PublishError::PathTooLong(_) => (ConversionErrorKind::PathTooLong, None),
        PublishError::TooManyCollisions => (ConversionErrorKind::TooManyCollisions, None),
        PublishError::OutOfDisk => (ConversionErrorKind::OutOfDisk, None),
        PublishError::Io(_) => (ConversionErrorKind::WriteFailed, None),
        // §2.2.4 (Windows): the leaf is a name Windows cannot open — the §2.8 `UnopenableOutputName` message
        // NAMES the offending token (the second tuple slot), never an alias/rename. Off Windows the guard is a
        // const-`Ok`, so this arm is unreachable there.
        PublishError::UnopenableName(token) => (
            ConversionErrorKind::UnopenableOutputName,
            Some(token.clone()),
        ),
    }
}

/// §2.1.1 step 7 — a failed item removes EVERY temp it minted ([`cleanup_item`], P3.22); a removal failure is
/// surfaced as a §2.6.4 [`ResidueRecord`] (case 2, `Failed`-with-residue) so the item is never a silent clean
/// success. `temps` is ALL the item's real [`TempPath`]s (the publish temp + any divert intermediate) — passing
/// them explicitly rather than dropping them is what lets a genuine removal failure surface (§2.6.4), never a
/// silent delete-on-drop. [Build-Session-Entscheidung: P3.38]
fn fail_cleanup(
    item: ItemId,
    temps: impl IntoIterator<Item = TempPath>,
    kind: ConversionErrorKind,
) -> WriteOutcome {
    fail_cleanup_named(item, temps, kind, None)
}

/// §2.1.1 step 7 with the §2.2.4 offending token (P3.88) — the [`fail_cleanup`] variant that threads a
/// `name_arg` (the leaf name Windows cannot open) onto [`WriteOutcome::name_arg`], so the §2.8
/// `UnopenableOutputName` message NAMES it. Every other failure passes `None` via [`fail_cleanup`]. The token
/// rides `write_outcome_to_run` → `ItemRunOutcome::Failed.name_arg` → the live/terminal render.
fn fail_cleanup_named(
    item: ItemId,
    temps: impl IntoIterator<Item = TempPath>,
    kind: ConversionErrorKind,
    name_arg: Option<String>,
) -> WriteOutcome {
    WriteOutcome {
        disposition: WriteDisposition::Failed { kind },
        diverted: false,
        residue: cleanup_leftovers(item, temps),
        name_arg,
    }
}

/// A successful publish — clean EVERY leftover temp explicitly (§2.6.2) and record any §2.6.4 residue (case 1,
/// `Succeeded`-with-residue). `temps` carries every real [`TempPath`] the item minted (the publish temp + any
/// divert intermediate): the one the publish renamed is already gone (`cleanup_item` idempotent-NotFound → Ok),
/// the rest are removed. [Build-Session-Entscheidung: P3.38]
fn finish_published(
    item: ItemId,
    output: PathBuf,
    diverted: bool,
    temps: impl IntoIterator<Item = TempPath>,
) -> WriteOutcome {
    WriteOutcome {
        disposition: WriteDisposition::Published { output },
        diverted,
        residue: cleanup_leftovers(item, temps),
        name_arg: None,
    }
}

/// §2.6.2/§2.6.4: remove each leftover publish-temp EXPLICITLY (never a silent delete-on-drop — a real removal
/// failure must surface, §2.6.4). The temp the publish renamed is already gone (`cleanup_item` treats NotFound as
/// idempotent success); a genuine failure is recorded as the item's [`ResidueRecord`] so it is never a silent
/// clean success. Returns the FIRST unremovable path. Panic-free. [Build-Session-Entscheidung: P3.38]
fn cleanup_leftovers(
    item: ItemId,
    leftovers: impl IntoIterator<Item = TempPath>,
) -> Option<ResidueRecord> {
    let mut residue = None;
    for leftover in leftovers {
        let path = leftover.to_path_buf();
        if cleanup_item(leftover).is_err() && residue.is_none() {
            residue = Some(ResidueRecord::new(item, path));
        }
    }
    residue
}

// ─── §0.4.2 ConversionEvent — the C6 run-telemetry Channel<ConversionEvent> payload (P2.37) ──────────
// [Build-Session-Entscheidung: P2.37] Homed in crate::orchestrator (ConversionEvent references RunResult +
// ItemOutcome → outcome-referencing, §0.7 ‡), co-homed with the §1.12 result types it carries. OUTBOUND-ONLY
// wire types — `Serialize` + `specta::Type`, NO `Deserialize` (a Channel payload is only ever sent
// Rust→WebView) — the SAME derive set as the sibling Channel payload `ScanProgress` (§0.6). camelCase wire.
//
// REGISTRATION — deferred-to-consumer, NOT here. ConversionEvent is a CHANNEL payload, NOT an app.emit
// event: the §0.4.2 app:// events (app://fault/intake/close-requested) are RAW `app.emit`/`listen` events
// whose payloads register via main.rs's `register_ipc_event_types` (`.types()`) at P2.39 (NOT collect_events!,
// which would force an `any`-bearing `makeEvent` helper into bindings.ts) — distinct from the C6 onProgress
// Channel stream. ConversionEvent is NOT added to main.rs's register_ipc_*_types chain (that chain holds the
// §2.8.2-mandated universal types IpcError/OutcomeMsg/LossyKind + the §0.4.2 raw-app:// event payloads
// AppFault/IntakePayload — a Channel payload belongs in neither; ScanProgress is absent from it too).
// ConversionEvent + its 5 payloads + the whole
// RunResult graph it carries via RunFinished JOIN bindings.ts at C6 (P2.29), when start_conversion registers
// its `onProgress: Channel<ConversionEvent>` arg — exactly the ScanProgress-via-C1 deferred-to-consumer
// pattern (bindings.ts P2.6/P2.15, which guards against consumer-less early registration). So P2.37 authors
// the TYPES; P2.29 pulls them onto the wire. They are dead in the production build (the module-level
// dead-code lint covers them) + exercised by the wire-form tests below.

/// The §0.4.2 run-telemetry event — the adjacently-tagged (`{ type, data }`) enum streamed over the C6
/// `start_conversion` `onProgress: Channel<ConversionEvent>` (§0.4.2 / §1.11): ordered, throughput-friendly,
/// run-scoped (dies with the run — no cross-run leak). `RunFinished` carries the §1.12 `RunResult` (mirrors C8).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase", tag = "type", content = "data")]
pub enum ConversionEvent {
    /// Batch accepted; the queue is built (§1.9).
    RunStarted(RunStarted),
    /// An item left `Pending` for `Running` (§1.9).
    ItemStarted(ItemStarted),
    /// Real per-item progress (§1.11 — never an indeterminate spinner).
    ItemProgress(ItemProgress),
    /// Terminal per item (§1.9).
    ItemFinished(ItemFinished),
    /// Aggregate queue progress for the batch bar (§1.11).
    BatchProgress(BatchProgress),
    /// Terminal for the run — the full §1.12 `RunResult` (mirrors C8).
    RunFinished(RunResult),
}

/// `RunStarted` (§0.4.2) — the batch was accepted and the §1.9 queue built.
///
/// [Build-Session-Entscheidung: P2.37.1] **`total_items` = QUEUED (eligible) items only.** It equals the §1.3
/// `CollectedSet::Single.count` (i.e. `items.len()` — NOT the internal `Grouping::Single.members`, never on the
/// §0.6 wire), EXCLUDING pre-flight-skipped items (§1.1/§1.3, which never enter the §1.9 queue). It is the
/// `BatchProgress.total` denominator (P2.37.3), so a skipped item never holds the bar below 100% — skips are
/// reconciled only at the §1.12 Summary. The "= count" equality is a §1.9 RUNTIME emission rule the P3.48
/// conductor enforces when it builds the queue; P2.37.1 fixes the FIELD + its documented denominator contract.
///
/// [Build-Session-Entscheidung: P2.37.2] **`will_reencode` is a non-optional `bool`, always definite.** A
/// conservative source-container→target worst-case flag (§2.9.2 — re-encode *possible* ⇒ `true`), decided from
/// the (source-container, target) pair BEFORE any `ffprobe` (inner codecs unknown at emit). The §2.9.2 emission
/// rule is that the core ALWAYS emits a definite value — `false` for non-video / non-applicable batches, never
/// omitted — so the Rust field is a plain `bool` (NOT `Option<bool>`) and the generated wire type is a
/// non-optional `willReencode: boolean` with no third `undefined` state. The real per-item disposition is
/// resolved at convert-time (§3.5); the §1.12 summary reflects the actual outcome.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunStarted {
    /// The run this telemetry belongs to (§0.4.4).
    pub run_id: RunId,
    /// QUEUED (eligible) items only — the P2.37.1 denominator (see the struct doc).
    pub total_items: u32,
    /// The conservative worst-case re-encode flag — always a definite `bool` (P2.37.2).
    pub will_reencode: bool,
}

/// `ItemStarted` (§0.4.2) — an item left `Pending` for `Running` (§1.9).
///
/// [Build-Session-Entscheidung: P3.76] `source_path: PathBuf` → the display-only `source_display: String`
/// (last-step `to_string_lossy`, §2.10.1) — no `PathBuf` crosses the wire (2026-07-06 ruling); the real
/// resolved source path stays core-side in `FrozenCollectedSet.item_paths` (keyed by `item_id`).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ItemStarted {
    pub run_id: RunId,
    pub item_id: ItemId,
    /// The core-produced lossy DISPLAY of the item's source being converted (§2.4 frozen resolved source,
    /// last-step `to_string_lossy`, §2.10.1) — display-only, never a re-submittable path.
    pub source_display: String,
    /// The whole-batch target (§0.6 invariant 1) this item converts to.
    pub target: TargetId,
}

/// `ItemProgress` (§0.4.2) — real per-item progress (§1.11; SSOT *not an indeterminate spinner*).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ItemProgress {
    pub run_id: RunId,
    pub item_id: ItemId,
    /// `0.0..=1.0`; `None` ONLY where truly indeterminate (LibreOffice, §1.11 — the frontend synthesises a
    /// staged determinate-looking bar from `stage` there). A NON-FINITE `Some` (NaN/±∞ — never a valid
    /// §1.11 fraction) has no JSON number form and collapses on the wire to the SAME `null` as the
    /// deliberate indeterminate `None` — the fail-safe pinned as contract by the §6.4.1 serialize test
    /// below. [Build-Session-Entscheidung: P2.137]
    pub fraction: Option<f32>,
    /// The §0.6/§1.11 coarse stage (`Spawning | Decoding | Encoding | Writing`).
    pub stage: JobStage,
}

/// `ItemFinished` (§0.4.2) — terminal per item (§1.9). Carries the §0.6 `ItemOutcome` projection.
///
/// [Build-Session-Entscheidung: P2.37.4] **Pre-flight-skip emission policy — no LIVE `ItemFinished{Skipped}`.**
/// Pre-flight-skipped items (§1.1/§1.3 — detection-ineligible; they never enter the §1.9 queue) are NOT emitted
/// as a live `ItemFinished` carrying `ItemOutcome::Skipped`; they appear ONLY in the terminal
/// `RunFinished → RunResult.items` projection (§1.12). `ItemOutcome::Skipped` is RESERVED for that terminal
/// path (it is not dead wire code — it carries the projected pre-flight skips + any mid-run cooperative skip),
/// so the conductor emits no live `ItemStarted`/`ItemFinished{Skipped}` for a freeze-time skip. The
/// `ItemFinished.outcome` field structurally CAN carry `Skipped` (the SAME shared `ItemOutcome` type as the
/// terminal `RunResult.items`), so the policy is a §1.9/§1.12 RUNTIME emission rule the P3.48 conductor honors,
/// NOT a type-level prohibition; P2.37.4 fixes the documented policy + the structural enabler.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ItemFinished {
    pub run_id: RunId,
    pub item_id: ItemId,
    /// The terminal §0.6 outcome (`Succeeded | Failed | Skipped | Cancelled`).
    pub outcome: ItemOutcome,
}

/// `BatchProgress` (§0.4.2) — aggregate queue progress for the batch bar (§1.11).
///
/// [Build-Session-Entscheidung: P2.37.3] **`total` == `RunStarted.total_items` (queued-only) invariant.**
/// `total` counts ONLY items that entered the §1.9 queue — the SAME queued-eligible denominator as
/// `RunStarted.total_items` (P2.37.1), EXCLUDING pre-flight-skipped items. If `total` counted dropped-but-
/// skipped items the bar could never reach 100%; skips are reconciled only at the §1.12 Summary. The equality
/// `BatchProgress.total == RunStarted.total_items` is a §1.11 RUNTIME emission invariant the P3.48 conductor
/// holds (both read the same `CollectedSet::Single.count`); P2.37.3 fixes the shared-`u32`-denominator field +
/// its documented invariant. `done` is the completed-item numerator (also queued-only).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BatchProgress {
    pub run_id: RunId,
    /// Completed queued items (the numerator) — pre-flight skips excluded.
    pub done: u32,
    /// QUEUED (eligible) items only — equals `RunStarted.total_items` (P2.37.3).
    pub total: u32,
}

// ─── §1.9 the C6 run conductor — pick-temp → await dispatch → publish legs (P3.48) ────────────────────
// [Build-Session-Entscheidung: P3.48] The tier-1 conductor the §1.9 module doc names — it COMPOSES the pure
// pieces (queue_order → advance → engines::dispatch → the §2.1.1 publish legs → advance → project_run_result)
// into the live C6 `start_conversion` run, streaming the §0.4.2 `ConversionEvent`s over the run Channel. The
// 2026-07-12 ruling's option ② composition is per-item: pick-temp (`RunScratch::publish_temp`) → AWAIT
// `engines::dispatch` (the §2.1.1 step-2 write, inherently async for a subprocess engine) → on `Succeeded`,
// the §1.7 non-empty exit-verification + the §2.1.1 publish legs (`publish_written_temp`); a `Failed`/
// `Cancelled` invocation drops the temp + projects directly, never reaching the publish legs. `run_conversion`
// takes PLAIN values/refs (no AppHandle), so it is unit-tested over a DIRECTLY-registered frozen set (the full
// C1→C6→summary E2E is P3.49/P3.63) — the AppHandle State-resolution + spawn is the thin C6 handler
// (`crate::ipc::conversion`, the boot-glue half of the build-vs-wire split).

/// The per-item terminal disposition the conductor's async convert ([`convert_item`]) produces — richer than
/// the publish-leg [`WriteOutcome`] because it additionally carries the §1.7 `Cancelled` arm (a cancelled item
/// never reaches the publish legs, so [`WriteOutcome`], produced only by the publish legs, cannot represent
/// it). The conductor maps it onto the §1.9 terminal [`JobEvent`] + the live §0.4.2 `ItemFinished`
/// [`ItemOutcome`] + the §1.12 projection inputs. [Build-Session-Entscheidung: P3.48]
enum ItemRunOutcome {
    /// Converted + atomically published (§2.1) — the real output `PathBuf` (retained for
    /// `RunResultPaths.item_outputs`), whether it §2.7.3-diverted, and any §2.6.4 cleanup residue.
    Published {
        output: PathBuf,
        diverted: bool,
        residue: Option<ResidueRecord>,
    },
    /// A named §2.8 failure (one item failed, the batch continues, §1.9) + any §2.6.4 residue.
    Failed {
        kind: ConversionErrorKind,
        residue: Option<ResidueRecord>,
        /// §2.2.4 (P3.88): the offending CONSTRUCTED token when `kind` is `UnopenableOutputName` — the leaf/
        /// subtree name Windows cannot open, threaded so BOTH the live `ItemFinished` `IpcError.message` and the
        /// terminal `RunResult.items[].reason` NAME it (§2.2.4 "naming the offending token"; the first slotted
        /// kind reachable on the Running→Failed path — see the `project_outcome` forward-constraint note). `None`
        /// for every non-slotted failure (the vast majority), rendered with the empty `arg`.
        name_arg: Option<String>,
    },
    /// User-cancelled (§1.7/§1.11) — the partial `out_tmp` was dropped (§3.2.2), nothing published.
    Cancelled,
}

/// Map a publish-leg [`WriteOutcome`] (P3.38) onto the conductor's richer [`ItemRunOutcome`] — a `Published`
/// disposition carries the real output + divert flag + residue; a `Failed` disposition carries the §2.8 kind +
/// residue. (A `WriteOutcome` never carries `Cancelled` — the publish legs run only on `Succeeded`.)
/// [Build-Session-Entscheidung: P3.48]
fn write_outcome_to_run(outcome: WriteOutcome) -> ItemRunOutcome {
    match outcome.disposition {
        WriteDisposition::Published { output } => ItemRunOutcome::Published {
            output,
            diverted: outcome.diverted,
            residue: outcome.residue,
        },
        WriteDisposition::Failed { kind } => ItemRunOutcome::Failed {
            kind,
            residue: outcome.residue,
            // §2.2.4 (P3.88): ride the offending token (set only for a `Failed(UnopenableOutputName)`, else
            // `None`) onto the conductor's outcome for the live/terminal §2.8 render.
            name_arg: outcome.name_arg,
        },
    }
}

/// The §2.8.2 catalog message for a Running→Failed item's live `ItemFinished` [`IpcError`] (§0.4.3) — the
/// substituted per-item failure line for `kind`, falling back to the always-homed `InternalError` row so a
/// failed item is never message-less. `arg` fills the row's substitution slot: **almost** every kind a conductor
/// per-item write produces (WriteFailed / Empty / InternalError / OutOfDisk / PathTooLong / TooManyCollisions /
/// EngineHang / … + the §3.5.6 transform kinds) is a NO-substitution row rendered with `arg = ""`. The **one
/// §2.2.4 exception (P3.88)** is `UnopenableOutputName`, the FIRST slotted kind reachable on the Running→Failed
/// path — its `{name}` slot is filled with the offending CONSTRUCTED token (the `project_outcome`
/// forward-constraint note: "a slotted kind … must extend this projection to supply the slot's arg"). The other
/// slotted kinds (UnsupportedType `{detected}` / PlatformUnavailable `{platform}`) stay pre-flight/app-level,
/// never a Running→Failed outcome; a residue rides `IpcError.residue_display`, NOT a combined CleanupResidue
/// message — that combination is the §1.12 SUMMARY-projection's job, §2.6.4 / P3.50). Exhaustive over
/// [`OutcomeMsg`] (no `_`, G4/G14). [Build-Session-Entscheidung: P3.48, P3.88]
fn failure_message(kind: ConversionErrorKind, arg: &str) -> String {
    let rendered = conversion_failure(kind, arg)
        .or_else(|| conversion_failure(ConversionErrorKind::InternalError, ""));
    match rendered {
        Some(OutcomeMsg::Failure { text, .. })
        | Some(OutcomeMsg::Lossy { text, .. })
        | Some(OutcomeMsg::Skipped { text, .. })
        // Unreachable by construction — `conversion_failure` only ever builds `Failure` (the `Residue` arm is
        // minted solely by `crate::outcome::residue_annotation`, §2.6.4 case 1) — but the text-extraction is
        // uniform across every variant, so the arm needs no special case. [Build-Session-Entscheidung: P3.59]
        | Some(OutcomeMsg::Residue { text }) => text,
        None => "Something unexpected went wrong, so this file couldn't be converted.".to_owned(),
    }
}

/// The §1.12 `RunResult.common_root` — the "open folder" target's real root (§2.7.4 / §7.7.3). For a
/// chosen-root batch it is the chosen root; for the beside-source default it is the DEEPEST directory
/// containing every §2.4 frozen dropped root (so "open folder" lands on the enclosing folder the outputs sit
/// beside). An empty root set (a defensive degenerate) yields an empty path. [Build-Session-Entscheidung: P3.48]
fn common_ancestor(roots: &[PathBuf], destination: &ResolvedDestination) -> PathBuf {
    match destination {
        ResolvedDestination::ChosenRoot(root) => root.clone(),
        ResolvedDestination::BesideSource => source_common_root(roots),
    }
}

/// The §2.7.1 SOURCE freeze common root — the deepest directory containing every §2.4 frozen dropped root. It
/// is the base a chosen-root subtree is taken against (`fs_guard::prepare_output_dir` does
/// `source.strip_prefix(source_common_root)` to re-create the relative subtree under the chosen root, §2.7.1).
/// **Destination-INDEPENDENT** — unlike [`common_ancestor`], whose `ChosenRoot` arm returns the open-folder
/// target D (the chosen root itself): feeding D as the strip base would fail `strip_prefix` for every source
/// (sources are never under the destination), so the two values MUST NOT be conflated. Empty root set → empty
/// path. [Build-Session-Entscheidung: P3.48]
fn source_common_root(roots: &[PathBuf]) -> PathBuf {
    let mut iter = roots.iter();
    let Some(first) = iter.next() else {
        return PathBuf::new();
    };
    let mut common = first.clone();
    for root in iter {
        common = deepest_common_prefix(&common, root);
    }
    common
}

/// The longest shared leading path prefix of two paths (component-wise) — the §2.7.4 common-ancestor fold.
/// [Build-Session-Entscheidung: P3.48]
fn deepest_common_prefix(a: &Path, b: &Path) -> PathBuf {
    let mut common = PathBuf::new();
    for (ca, cb) in a.components().zip(b.components()) {
        if ca == cb {
            common.push(ca);
        } else {
            break;
        }
    }
    common
}

/// The chosen target's canonical output extension for the walking-skeleton slice (`tsv`/`csv`) — mirrors
/// `NativeCsvTsvEngine::plan`'s target-token map (`None` for a mis-routed non-CSV/TSV target — an
/// `InternalError`, never a user fault; the UI never offers one, §1.5). Compared by value against the two
/// format ids (not a `match` with a `_` arm — the crate-root `clippy::wildcard_enum_match_arm` deny).
/// [Build-Session-Entscheidung: P3.48]
fn slice_extension(target: TargetId) -> Option<&'static str> {
    if target == TargetId::Format(UserFacingFormat::Tsv) {
        Some("tsv")
    } else if target == TargetId::Format(UserFacingFormat::Csv) {
        Some("csv")
    } else {
        None
    }
}

/// **§1.7 exit & output verification (P3.48 — RELOCATED from `write_item` onto the conductor's Succeeded
/// path).** After the §1.7 dispatch reports `Succeeded`, the reclaimed publish `tmp` must exist and be
/// non-empty for the §2.1.1 publish legs to run: `Ok(tmp)` iff the temp is non-empty (§1.7 "success ONLY if
/// the expected temp output exists and is non-empty"); a present-but-0-byte output is a §2.8 `Empty` failure
/// (never a clean success of an empty file); a VANISHED temp broke the engine's non-empty contract → §2.13
/// `InternalError`. On failure the temp is cleaned ([`fail_cleanup`] — §2.6.4 honest, a removal failure
/// surfaces a `CleanupResidue`) and the [`WriteOutcome`] returned as `Err`. Extracted so the empty/vanished
/// verification is unit-testable without driving the async engine. [Build-Session-Entscheidung: P3.48]
fn verify_encode_output(item: ItemId, tmp: TempPath) -> Result<TempPath, WriteOutcome> {
    match std::fs::metadata(&*tmp) {
        Ok(meta) if meta.len() > 0 => Ok(tmp),
        Ok(_) => Err(fail_cleanup(item, [tmp], ConversionErrorKind::Empty)),
        Err(_) => Err(fail_cleanup(
            item,
            [tmp],
            ConversionErrorKind::InternalError,
        )),
    }
}

/// The per-item async convert (§2.1.1 — the P3.48 ruling option ②: pick-temp → await dispatch → publish legs).
/// Computes the §1.8 [`OutputPlan`], picks the publish temp (§2.14.1), builds the §1.7 invocation
/// (`NativeCsvTsvEngine::plan` + the §1.7-owned `out_tmp`), AWAITs `engines::dispatch` (the step-2 write, with
/// the §1.11 progress ticks + the §0.4.4 cancel token), and on `Succeeded` runs the §1.7 non-empty exit
/// verification + the §2.1.1 publish legs ([`publish_written_temp`]); a `Failed`/`Cancelled` invocation drops
/// the temp (§3.2.2) and projects directly. No panic (the crate no-panic deny) — every failure is a structured
/// [`ItemRunOutcome`]; the source bytes are never touched (the no-harm G32(a) invariant).
/// [Build-Session-Entscheidung: P3.48]
#[allow(clippy::too_many_arguments)]
async fn convert_item(
    dropped: &DroppedItem,
    source: &Path,
    target: TargetId,
    frozen_sources: &[FileIdentity],
    divert_root: Option<&Path>,
    destination: &ResolvedDestination,
    source_common_root: &Path,
    scratch: &RunScratch,
    cache: &mut LocationCache,
    probe_name: impl Fn() -> OsString + Copy,
    pool: &Pool,
    cancel: CancellationToken,
    run_id: RunId,
    item: ItemId,
    on_progress: &Channel<ConversionEvent>,
) -> ItemRunOutcome {
    // The §1.5 slice output extension (the handler resolved a slice target, so this is `Some` on the live
    // path; a mis-routed non-CSV/TSV target fails the item InternalError before any temp is picked).
    let Some(ext) = slice_extension(target) else {
        return ItemRunOutcome::Failed {
            kind: ConversionErrorKind::InternalError,
            residue: None,
            name_arg: None,
        };
    };

    // §1.8 output planning (P3.37): resolve this item's output dir (beside-source parent / chosen-root subtree
    // / §2.7.3 divert). `intended_dir` is the §2.7.2 location probe target — the beside-source parent, or the
    // chosen root (an existing folder; the subtree is created under it by `compute_output_plan`).
    let (mode, intended_dir): (DestinationMode, &Path) = match destination {
        ResolvedDestination::BesideSource => {
            let Some(parent) = source.parent() else {
                return ItemRunOutcome::Failed {
                    kind: ConversionErrorKind::InternalError,
                    residue: None,
                    name_arg: None,
                };
            };
            (DestinationMode::BesideSource, parent)
        }
        ResolvedDestination::ChosenRoot(root) => (
            DestinationMode::ChosenRoot {
                root,
                common_root: source_common_root,
            },
            root.as_path(),
        ),
    };
    let location = cache.classify(intended_dir, probe_name);
    let plan = match compute_output_plan(
        item,
        source,
        ext,
        mode,
        location,
        divert_root,
        cache,
        probe_name,
    ) {
        Ok(plan) => plan,
        // §2.2.4 (P3.88): a re-created chosen-root subtree directory Windows cannot open → fail clearly
        // `UnopenableOutputName` NAMING the token (the subtree analog of the leaf-side publish reject), never a
        // generic write failure — so the §2.8 message tells the user WHICH constructed component is the problem.
        Err(OutputPlanError::UnopenableName(token)) => {
            return ItemRunOutcome::Failed {
                kind: ConversionErrorKind::UnopenableOutputName,
                residue: None,
                name_arg: Some(token),
            }
        }
        // §2.7.3 no usable divert target / §2.7.1 dir-prep failure → the item fails clearly (§2.8 WriteFailed).
        Err(OutputPlanError::DivertUnavailable | OutputPlanError::Io(_)) => {
            return ItemRunOutcome::Failed {
                kind: ConversionErrorKind::WriteFailed,
                residue: None,
                name_arg: None,
            }
        }
    };

    // §2.1.1 step 1: pick the publish temp on `final`'s volume (P3.20) — the run-owned `.convertia-…-.part`
    // sibling. A create failure (permission / IO at the destination) fails the item clearly; nothing written.
    let Ok(tmp) = scratch.publish_temp(&plan.publish_temp_dir, item) else {
        return ItemRunOutcome::Failed {
            kind: ConversionErrorKind::WriteFailed,
            residue: None,
            name_arg: None,
        };
    };

    // Build the §1.7 invocation: `NativeCsvTsvEngine::plan()` is PURE (§3.2.2) and returns `out_tmp: None`;
    // §1.7 (this tier-1 conductor) OWNS + populates `out_tmp = Some(tmp)` on the ENCODE invocation (the
    // 2026-07-07 plan-seam ruling). A pure PlanError → its §2.8 kind, cleaning the just-picked temp.
    let plan_outcome = match NativeCsvTsvEngine.plan(dropped, target, source, &tmp) {
        Ok(plan_outcome) => plan_outcome,
        Err(err) => return write_outcome_to_run(fail_cleanup(item, [tmp], err.kind)),
    };
    // The slice engine is single-step: `plan()` always returns `Encode`. A `Probe` would be a mis-routed plan.
    let PlanOutcome::Encode(mut invocation) = plan_outcome else {
        return write_outcome_to_run(fail_cleanup(
            item,
            [tmp],
            ConversionErrorKind::InternalError,
        ));
    };
    invocation.out_tmp = Some(tmp);
    let mut envelope = EngineInvocation {
        job: item,
        engine: EngineId::NativeCsvTsv,
        plan: invocation,
        cancel,
    };

    // §2.1.1 step 2: the awaited §1.7 dispatch WRITES the engine output into `out_tmp` (never `final`, §3.5).
    // The progress sink wraps each self-reported §1.11 InProcessFraction into a §0.4.2 `ItemProgress` tick over
    // the run Channel. [Build-Session-Entscheidung: P3.48] Stage = `Encoding` — the CSV/TSV transform reads and
    // writes as one bounded in-core pass (§3.5.6), so its single self-reported fraction stream reports the
    // dominant produce-the-output work; the §1.11 InProcessFraction basis carries the real fraction (§1.11).
    let progress_channel = on_progress.clone();
    let on_fraction = move |fraction: f32| {
        progress_channel
            .send(ConversionEvent::ItemProgress(ItemProgress {
                run_id,
                item_id: item,
                fraction: Some(fraction),
                stage: JobStage::Encoding,
            }))
            .ok();
    };
    let result = dispatch(&envelope, pool, on_fraction).await;

    match result {
        // §1.7 verified success → reclaim the written temp (dispatch BORROWED the envelope, so the conductor
        // still owns `out_tmp`), run the §1.7 non-empty exit verification, then the §2.1.1 publish legs.
        InvocationResult::Succeeded => {
            let Some(tmp) = envelope.plan.out_tmp.take() else {
                return ItemRunOutcome::Failed {
                    kind: ConversionErrorKind::InternalError,
                    residue: None,
                    name_arg: None,
                };
            };
            // §1.7 exit & output verification (RELOCATED onto the Succeeded path — the P3.48 re-cut): the temp
            // must exist and be non-empty or the item fails (§2.8 `Empty` / §2.13 `InternalError`) before publish.
            let tmp = match verify_encode_output(item, tmp) {
                Ok(tmp) => tmp,
                Err(outcome) => return write_outcome_to_run(outcome),
            };
            // §2.1.1 steps 3-7 (+ the §2.7.2/§2.7.5 late-divert) over the verified temp. The §2.7.3 late-divert
            // candidate is the resolved divert root (the same one `compute_output_plan` used), or an EMPTY set
            // when neither Downloads nor Documents resolved — an empty set makes `resolve_divert_target` yield
            // `Unavailable` → the late-divert fails clearly with `WriteFailed`, never a hidden-app-dir write (§2.7.3).
            let divert_candidates: Vec<PathBuf> =
                divert_root.map(Path::to_path_buf).into_iter().collect();
            let outcome = publish_written_temp(
                &plan,
                source,
                frozen_sources,
                &divert_candidates,
                scratch,
                cache,
                probe_name,
                tmp,
            );
            write_outcome_to_run(outcome)
        }
        // §1.7 failure → the engine wrote a partial (or nothing) into `out_tmp`; clean it EXPLICITLY (§2.6.4
        // honesty — a removal failure surfaces as a `CleanupResidue`, never a silent delete-on-drop) and fail.
        InvocationResult::Failed(kind) => {
            let outcome = match envelope.plan.out_tmp.take() {
                Some(tmp) => fail_cleanup(item, [tmp], kind),
                None => WriteOutcome::failed(kind),
            };
            write_outcome_to_run(outcome)
        }
        // §1.7 InProcessNative cancel → drop the partial `out_tmp` (deleted on drop, §3.2.2), nothing published.
        // [Build-Session-Entscheidung: P3.48] Drop (not `cleanup_item`) matches the §1.7 InProcessNative cancel
        // contract ("drops the out_tmp TempPath"); the §2.6.4 deferred-residue case is the SUBPROCESS
        // group-kill timeout (P4), not this in-core cooperative cancel.
        InvocationResult::Cancelled => {
            drop(envelope.plan.out_tmp.take());
            ItemRunOutcome::Cancelled
        }
    }
}

/// The run-end finalisation (P3.48): project the terminal `Batch` onto the §1.12 wire `RunResult` +
/// off-wire `RunResultPaths`, emit the terminal §0.4.2 `RunFinished`, RETAIN it for the C8 re-serve
/// (§0.4.4), drop the run's cancellation token (`runs.finish` — the terminal `RunResult` out-lives it), and
/// run the §2.6.2 run-end cleanup over the recorded final dirs. **The run-end cleanup residue is deliberately
/// not re-surfaced** (see the drop at the call site): §2.6.4 honest-residue reporting is WHOLLY per-item (its
/// three cases each ride the item via `ItemRunOutcome` above) and §1.12 `RunResult` carries NO run-level
/// residue slot (`CleanupResidue` is an `ItemOutcome`, §1.12), so there is nowhere to project it — and both
/// residue kinds `cleanup_run` can yield are already backstopped: a lingering destination `.part` by the
/// per-item §2.6.4 leg, an un-removable central `run-<RunId>/` scratch tree by the §2.6.3 next-launch orphan
/// sweep. [Build-Session-Entscheidung: P3.48]
#[allow(clippy::too_many_arguments)]
fn finish_run(
    batch: Batch,
    run_id: RunId,
    results: &RunResultStore,
    runs: &RunRegistry,
    scratch: RunScratch,
    on_progress: &Channel<ConversionEvent>,
    common_root: PathBuf,
    item_outputs: BTreeMap<ItemId, PathBuf>,
    failed_name_args: BTreeMap<ItemId, String>,
    residues: Vec<ResidueRecord>,
    recorded_final_dirs: BTreeSet<PathBuf>,
    any_diverted: bool,
    divert_root: Option<PathBuf>,
) {
    // §1.12 projection → the wire `RunResult` (display-only) + the off-wire `RunResultPaths`. `divert_root` is
    // carried only when an item actually diverted (§2.7.4 — the second "open Downloads" affordance); a diverted
    // item implies the root resolved, so `filter` keeps the `Some` in that case and is `None` otherwise.
    let divert_root_opt = divert_root.filter(|_| any_diverted);
    let (result, paths) = project_run_result(
        &batch,
        run_id,
        &item_outputs,
        &failed_name_args,
        residues,
        common_root,
        divert_root_opt,
    );
    // §0.4.2 terminal `RunFinished` (mirrors C8) → retain for the C8 re-serve → drop the token → §2.6.2 cleanup.
    on_progress
        .send(ConversionEvent::RunFinished(result.clone()))
        .ok();
    results.retain(result, paths);
    runs.finish(run_id);
    // §2.6.2 run-end cleanup. Its residue is deliberately DROPPED, not re-surfaced (see the doc comment): the
    // §2.6.4 honesty is per-item (already threaded above) and §1.12 `RunResult` has no run-level residue field
    // — a lingering destination `.part` is the per-item §2.6.4 case, an un-removable scratch tree is reclaimed
    // by the §2.6.3 next-launch sweep, so nothing remains to report here.
    drop(cleanup_run(scratch, &recorded_final_dirs));
}

/// **The §1.9 C6 run conductor (P3.48)** — the tier-1 composition that drives a `Batch` to its §1.12
/// `RunResult`, streaming the §0.4.2 `ConversionEvent`s over `on_progress`. Applies the §2.5 `RerunDecision`
/// (the pre-pass), emits `RunStarted`, dispatches every eligible `Pending` job SEQUENTIALLY (the P3 slice = one
/// worker; the §0.9 concurrency degree is P4), records each success's §2.5.1 ledger key, and projects the
/// terminal batch onto the retained `RunResult` (+ its off-wire paths) for C8 re-serve. Takes PLAIN
/// values/refs (no AppHandle), so it is unit-testable over a directly-registered frozen set; the AppHandle
/// State-resolution + spawn is the thin C6 handler. [Build-Session-Entscheidung: P3.48]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_conversion(
    mut batch: Batch,
    frozen: &RegisteredSet,
    run_id: RunId,
    token: CancellationToken,
    scratch: RunScratch,
    instance: InstanceId,
    divert_root: Option<PathBuf>,
    rerun_decision: RerunDecision,
    pool: &Pool,
    ledger: &RerunLedger,
    equiv: &EquivKeyComputer,
    results: &RunResultStore,
    runs: &RunRegistry,
    on_progress: &Channel<ConversionEvent>,
) {
    let target_id = batch.target.id;
    let options = batch.options.clone();
    let destination = batch.destination.clone();

    // §2.5 RerunDecision applier (the C6-construction pre-pass). A `Skip`+seen eligible item is a re-run the
    // user chose not to re-produce (§2.5 "no new output for equivalent items"): the P3.48 rerun-skip ruling
    // assigns it `JobState::Skipped(SkipReason::AlreadyConverted)` — it KEEPS its real `Eligible` `DroppedItem`
    // (no `SkippedItem` fabricated; §1.4 stays detection-only) but becomes terminal-`Skipped`, so it rides the
    // EXISTING skip machinery end-to-end: excluded from `queue_order`/`total_items` (never dispatched, no live
    // events, §0.4.2), yet projected into the §1.12 summary as `ItemResult{ state: Skipped(AlreadyConverted),
    // output_display: None, reason: OutcomeMsg::Skipped }` counted in `Totals.skipped` (§1.12 "skipped items
    // appear as a distinct outcome, not a failure"). [Build-Session-Entscheidung: P3.48 — the applier arm; the
    // JobState mapping itself is the Co-Pilot ruling `034a451` (NOT loop-decidable — §2.5 + §1.12 + §0.6 were
    // not jointly satisfiable by any existing state, so the ruling added the `AlreadyConverted` variant).] The
    // refined P3.47 coupling: `source is JobSource::Skipped(_) ⟺ state is Skipped(<detection reason>)`;
    // `Skipped(AlreadyConverted)` ⟹ source `Eligible`. On a FIRST run (empty ledger) NOTHING is seen, so no item
    // is reassigned; the applier converts every eligible item + records its §2.5.1 key on success.
    // `ledger.has_seen` reflects a PRIOR in-session run (§2.5.2 signal 1); a within-batch duplicate is unseen in
    // this pre-pass → both convert → §2.2 silent numbering (the §2.5.2 concurrent-identical-batch edge, accepted).
    if rerun_decision == RerunDecision::Skip {
        for job in &mut batch.jobs {
            let seen =
                match &job.source {
                    JobSource::Eligible(dropped) => frozen
                        .identities
                        .get(&dropped.item)
                        .is_some_and(|identity| {
                            ledger.has_seen(equiv.compute_equiv_key(identity, target_id, &options))
                        }),
                    // A pre-flight `Skipped` job is never a re-run candidate (it never converts).
                    JobSource::Skipped(_) => false,
                };
            if seen {
                job.state = JobState::Skipped(SkipReason::AlreadyConverted);
            }
        }
    }

    // §0.4.2 `RunStarted` — `total_items` = QUEUED (eligible) count only (excludes pre-flight skips); the
    // `BatchProgress.total` denominator. CSV/TSV is never re-encode → `will_reencode: false` (§2.9.2).
    let total = u32::try_from(queue_order(&batch).count()).unwrap_or(u32::MAX);
    on_progress
        .send(ConversionEvent::RunStarted(RunStarted {
            run_id,
            total_items: total,
            will_reencode: false,
        }))
        .ok();

    // The §2.3.3 link-safety comparison set = every frozen source identity (§2.3 unqualified "any source in
    // the frozen set", eligible AND skipped). The §2.7.4 open-folder root. The §2.6.1-stamped probe-name
    // factory (closes over this run's `instance` — a Copy closure). The §2.6.2 recorded final dirs.
    let frozen_sources: Vec<FileIdentity> = frozen.identities.values().cloned().collect();
    // Two DISTINCT roots (do NOT conflate — the P3.48 G1-review fix): `common_root` is the §1.12/§2.7.4
    // open-folder target (for `ChosenRoot` it is the chosen root D), while `source_root` is the §2.7.1 SOURCE
    // freeze common root the chosen-root subtree is stripped against (`fs_guard::prepare_output_dir` does
    // `source.strip_prefix(source_root)`). They coincide for `BesideSource` but DIFFER for `ChosenRoot` (D vs
    // the source side); `convert_item` needs the source side, `finish_run` the open-folder side.
    let common_root = common_ancestor(&frozen.frozen.roots, &destination);
    let source_root = source_common_root(&frozen.frozen.roots);
    let probe_name = move || crate::run::PublishTemp::probe_name(instance);
    let mut cache = LocationCache::new();
    let mut item_outputs: BTreeMap<ItemId, PathBuf> = BTreeMap::new();
    let mut residues: Vec<ResidueRecord> = Vec::new();
    let mut recorded_final_dirs: BTreeSet<PathBuf> = BTreeSet::new();
    let mut any_diverted = false;
    // §2.2.4 (P3.88): the per-item offending token for an `UnopenableOutputName` failure, collected here so the
    // TERMINAL `RunResult.items[].reason` NAMES it too (the live `ItemFinished` message already does). Empty for
    // a run with no unopenable-name failure.
    let mut failed_name_args: BTreeMap<ItemId, String> = BTreeMap::new();
    let mut done: u32 = 0;

    // §1.9 cancel-semantics NOTE (a P3.48 G1-review finding; the C7 `cancel_run` token trip is now wired, P3.52):
    // this loop does NOT poll `token.is_cancelled()` before dequeuing the next Pending job. It relies on each
    // dispatched item's OWN cooperative cancel. NO-HARM holds regardless of WHEN the token trips (including
    // mid-batch, now that C7 can trip it) — but NOT because every dispatched item returns `Cancelled` without
    // publishing (the P3.75 sweep corrected this): the §1.7 in-core CSV/TSV transform polls the token only at
    // N-KB chunk boundaries, so a SUB-CHUNK item (the common small-file case) never observes the trip, completes,
    // and PUBLISHES a fresh output. That fresh output still harms no original — no-harm rests on no-clobber +
    // frozen-at-drop + the §2.1.2 atomic create-only publish (which can ONLY create a NEW file, never
    // touch/overwrite a source), independent of cancel timing; the cancel-before-publish path is one mechanism,
    // not the guarantee. The §1.9 "stop dequeuing Pending" refinement (leave un-started jobs un-dispatched + give
    // them a terminal summary state) is a SEPARATE §1.9 optimization — it would end the wasted small-item work
    // above, but is NOT a no-harm requirement. It also needs a §1.9 decision the FSM does not sanction here
    // (`advance` returns None for a direct Pending→Cancelled, by design — a clean stop-dequeue needs a
    // summary-state ruling for un-started jobs), so it is deliberately OUT of P3.52's token-trip-wiring scope. Its
    // home is the P4.11 §1.7 kill↔cleanup box (measured against the full §1.7 cancel enumeration, incl. §1.7's
    // "stop dequeuing Pending"); the spec §1.7 mandate is unchanged, so nothing is dropped here — the run still
    // terminates with every job in a valid terminal state (a dispatched large item via Pending→Running→Cancelled;
    // a dispatched sub-chunk item via Pending→Running→Succeeded). [Build-Session-Entscheidung: P3.52]
    for i in 0..batch.jobs.len() {
        // Only eligible `Pending` jobs are dispatched; pre-flight `Skipped` jobs are terminal at construction
        // (never queued, no Channel events — §0.4.2/§1.9). `queue_order`'s `state_is_queued` == this filter.
        if !matches!(batch.jobs[i].state, JobState::Pending) {
            continue;
        }
        let item = batch.jobs[i].item;
        // The eligible source record + its §2.3-resolved real path (the §0.4.4 off-wire `item_paths` table).
        let Some(dropped) = batch.jobs[i].source.eligible().cloned() else {
            continue;
        };
        let Some(source) = frozen
            .frozen
            .item_paths
            .get(&item)
            .map(|paths| paths.resolved_path.clone())
        else {
            // No resolved path retained for a `Pending` item → a mis-built frozen set → fail clearly (no
            // panic). Move it Pending → Running → Failed so the §1.12 projection reports a terminal state.
            if let Ok(running) = advance(batch.jobs[i].state, JobEvent::Started) {
                batch.jobs[i].state = advance(
                    running,
                    JobEvent::Failed(ConversionErrorKind::InternalError),
                )
                .unwrap_or(running);
            }
            continue;
        };
        let source_display = dropped.display_name.clone();

        // §1.9 Pending → Running (§1.7 engine invoked). Emit §0.4.2 `ItemStarted`.
        if let Ok(next) = advance(batch.jobs[i].state, JobEvent::Started) {
            batch.jobs[i].state = next;
        }
        on_progress
            .send(ConversionEvent::ItemStarted(ItemStarted {
                run_id,
                item_id: item,
                source_display,
                target: target_id,
            }))
            .ok();

        // The per-item convert (pick-temp → await dispatch → verify → publish/fail/cancel).
        let outcome = convert_item(
            &dropped,
            &source,
            target_id,
            &frozen_sources,
            divert_root.as_deref(),
            &destination,
            &source_root,
            &scratch,
            &mut cache,
            probe_name,
            pool,
            token.clone(),
            run_id,
            item,
            on_progress,
        )
        .await;

        // Map the per-item outcome onto the §1.9 terminal event + the live §0.4.2 `ItemOutcome` + the §1.12
        // projection inputs (output path / residue / divert flag), recording the §2.5.1 ledger key on success.
        let (event, item_outcome) = match outcome {
            ItemRunOutcome::Published {
                output,
                diverted,
                residue,
            } => {
                let output_display = output.to_string_lossy().into_owned();
                if let Some(dir) = output.parent() {
                    recorded_final_dirs.insert(dir.to_path_buf());
                }
                item_outputs.insert(item, output);
                any_diverted |= diverted;
                if let Some(record) = residue {
                    if let Some(dir) = record.real_path.parent() {
                        recorded_final_dirs.insert(dir.to_path_buf());
                    }
                    residues.push(record);
                }
                // §2.5.2 ledger: record this exact conversion's key so a subsequent identical in-session run is
                // detected (§2.5.1 — identity, not path).
                if let Some(identity) = frozen.identities.get(&item) {
                    ledger.record(equiv.compute_equiv_key(identity, target_id, &options));
                }
                (
                    JobEvent::Succeeded,
                    ItemOutcome::Succeeded { output_display },
                )
            }
            ItemRunOutcome::Failed {
                kind,
                residue,
                name_arg,
            } => {
                let residue_display = residue.as_ref().map(|record| {
                    if let Some(dir) = record.real_path.parent() {
                        recorded_final_dirs.insert(dir.to_path_buf());
                    }
                    record.real_path.to_string_lossy().into_owned()
                });
                if let Some(record) = residue {
                    residues.push(record);
                }
                // §2.2.4 (P3.88): the live message NAMES the offending token; collect it so the terminal
                // `RunResult` reason names it too. `None` for every non-slotted failure → the empty `arg`.
                if let Some(token) = &name_arg {
                    failed_name_args.insert(item, token.clone());
                }
                let error = IpcError {
                    kind,
                    message: failure_message(kind, name_arg.as_deref().unwrap_or("")),
                    path_display: None,
                    residue_display,
                };
                (JobEvent::Failed(kind), ItemOutcome::Failed { error })
            }
            ItemRunOutcome::Cancelled => (JobEvent::Cancelled, ItemOutcome::Cancelled),
        };

        // §1.9 Running → terminal. Emit §0.4.2 `ItemFinished` + `BatchProgress` (queued-only denominator).
        if let Ok(next) = advance(batch.jobs[i].state, event) {
            batch.jobs[i].state = next;
        }
        on_progress
            .send(ConversionEvent::ItemFinished(ItemFinished {
                run_id,
                item_id: item,
                outcome: item_outcome,
            }))
            .ok();
        done = done.saturating_add(1);
        on_progress
            .send(ConversionEvent::BatchProgress(BatchProgress {
                run_id,
                done,
                total,
            }))
            .ok();
    }

    finish_run(
        batch,
        run_id,
        results,
        runs,
        scratch,
        on_progress,
        common_root,
        item_outputs,
        failed_name_args,
        residues,
        recorded_final_dirs,
        any_diverted,
        divert_root,
    );
}

// ─── §0.4.4 run registry — the RunId → CancellationToken token store (P2.42) ─────────────────────────
// [Build-Session-Entscheidung: P2.42] The §0.4.4 cancellation-token registry, homed in crate::orchestrator
// per §0.7 ("orchestrator homes run registry + cancellation"). It owns the token's IDENTITY + LIFECYCLE
// only (created in C6, tripped by C7, dropped on RunFinished) — §0.4.4 explicitly scopes THIS section to
// identity/lifecycle: the §1.7 invocation layer wires the token to the engine subprocess for the
// process-group kill, and cancellation is cooperative at the orchestrator level + forceful at the engine
// level (reconciled by §1.7, built in P3/P4). Like the sibling lifecycle/result types, this is a CONTRACT
// authored before its consumer — but PARTLY consumed from P2.55: `has_active_run` is the §7.1.1 refuse-busy
// predicate `converter_is_busy` reads, and the `.manage(RunRegistry)` registration lives in main()'s Builder
// chain (P2.55). The C6/RunFinished/C7 token WIRING is now LIVE — `register` (the P3.48 C6 `start_conversion`
// handler) + `finish` (the P3.48 conductor's run-end + the spawn-error path) + `cancel` (the C7 `cancel_run`
// handler tripping the real registry `.cancel()`, P3.52) — so no `RunRegistry` method is dead any more. The
// retained terminal RunResult (so C8 re-serves after a WebView reload,
// §0.4.4) is a SEPARATE store — the P2.43 box — NOT this token registry.

/// The §0.4.4 run registry — maps each in-flight `RunId` to its `tokio_util::sync::CancellationToken`. Held
/// as a Tauri app-managed `State` (the `.manage` is P2.55; the register/finish wiring is the P3.48 conductor
/// and handler, the cancel wiring the C7 `cancel_run` handler, P3.52). The token's three §0.4.4 lifecycle
/// points are this type's three methods: [`register`](RunRegistry::register) at C6 `start_conversion` (mint +
/// store a fresh token), [`cancel`](RunRegistry::cancel) at C7 `cancel_run` (trip it — cooperative at the
/// orchestrator level), and [`finish`](RunRegistry::finish) on `RunFinished` (drop it; the run's terminal
/// `RunResult` out-lives the token in the separate P2.43 retention store). Interior-mutable behind a `Mutex`
/// so the shared `&RunRegistry` (the `State` form) serves concurrent C6/C7 handlers; every critical section
/// is a whole-map op that never holds the guard across an `.await`, so a plain `std::sync::Mutex` (not an
/// async lock) is correct.
///
/// [Build-Session-Entscheidung: P2.42] `Default`-constructed empty (the app-startup form); `Debug` for parity
/// with the sibling orchestrator state. NOT a wire type (no `serde`/`specta`) — it is pure core-internal
/// State that never crosses IPC (the WebView drives cancellation through the C7 command, never sees a token).
#[derive(Debug, Default)]
pub struct RunRegistry {
    /// The active `RunId` → `CancellationToken` map. A `RunId` is a unique per-run v4 (§7.1.2), inserted once
    /// at C6 and removed once on `RunFinished`, so no key ever legitimately collides.
    tokens: Mutex<HashMap<RunId, CancellationToken>>,
}

impl RunRegistry {
    /// Lock the token map, recovering the guard from a poisoned lock rather than propagating the panic. The
    /// critical sections are infallible whole-map ops that never panic (the in-core no-panic discipline,
    /// G4/G14), so a poisoned lock is unreachable in practice; recovering keeps the registry usable AND avoids
    /// an `unwrap`/`expect` on the no-panic path. [Build-Session-Entscheidung: P2.42]
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<RunId, CancellationToken>> {
        self.tokens
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Mint a fresh `CancellationToken` for `run_id` and store it (C6 `start_conversion`, §0.4.4). Returns a
    /// clone for the conductor to hand to the run's workers: a `CancellationToken` is a cheap `Arc`-backed
    /// handle, so the stored copy and the returned copy share ONE cancellation state (a `cancel` on either —
    /// or via [`cancel`](RunRegistry::cancel) — trips all). A `RunId` is unique per run (§7.1.2), so this
    /// never overwrites a live entry; a collision could only be a programming error and would merely drop the
    /// stale token.
    pub fn register(&self, run_id: RunId) -> CancellationToken {
        let token = CancellationToken::new();
        self.lock().insert(run_id, token.clone());
        token
    }

    /// Trip the `run_id`'s token (C7 `cancel_run`, §0.4.4) — cooperative cancellation at the orchestrator
    /// level. Returns `true` if a live token was found and tripped, `false` if `run_id` is unknown or already
    /// finished — the §0.4.1 C7 idempotent no-op-cancel case (cancelling a completed/absent run is a clean
    /// no-op, never an error). The token is left IN the map until [`finish`](RunRegistry::finish) (a worker may
    /// still observe the cancel before its run reaches `RunFinished`). The token is cloned out before the
    /// guard drops, so `.cancel()` runs without holding the lock.
    pub fn cancel(&self, run_id: RunId) -> bool {
        let token = self.lock().get(&run_id).cloned();
        match token {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    /// Drop the `run_id`'s token on `RunFinished` (§0.4.4) — the run is terminal, so its token is no longer
    /// needed. This is NOT a cancel — an outstanding worker clone is left un-cancelled (a normal finish), and
    /// the run's terminal `RunResult` out-lives the token in the separate P2.43 retention store (§0.4.4).
    pub fn finish(&self, run_id: RunId) {
        self.lock().remove(&run_id);
    }

    /// True iff a conversion run is in flight — the §1.9 run-level "Running" state the §7.1.1/§7.3.2
    /// refuse-busy gate reads (`converter_is_busy`). A run's token is present from C6 `start_conversion` to
    /// `RunFinished`, INCLUDING while a C7 cancel winds it down ([`cancel`](RunRegistry::cancel) trips the
    /// token but leaves it until [`finish`](RunRegistry::finish)), so a cancelling-but-not-finished run still
    /// reports busy — correct for refuse-busy (the §2.4 frozen set must not take new intake until the run is
    /// fully terminal). The runs are POPULATED by the P3.48 conductor; until then the registry is empty, so
    /// this is `false` (not busy) and the funnel's idle-flow is open. [Build-Session-Entscheidung: P2.55]
    pub fn has_active_run(&self) -> bool {
        !self.lock().is_empty()
    }
}

// ─── §0.4.4 RunResult retention — the process-local C8-re-serve store (P2.43) ─────────────────────────
// [Build-Session-Entscheidung: P2.43] The §0.4.4 run-registry retention, homed in crate::orchestrator
// alongside the sibling RunRegistry (the token store, P2.42) — both §0.4.4 orchestrator State (§0.7). This
// is the SEPARATE store the P2.42 RunRegistry doc points at: "the cancellation token is dropped on
// RunFinished" drops only the TOKEN; the terminal RunResult OUT-LIVES the token here so C8 get_run_summary
// can idempotently re-serve the summary after a WebView reload (the exact §0.4.4 case C8 names). Retention is
// IN-MEMORY + process-local, NO on-disk persistence — consistent with §7.4. The retained result lives "until
// a new run starts or the app exits" (§0.4.4): a new run's start (C6) evicts the prior result, RunFinished
// retains the new one, and a process exit drops the store. Like the sibling state, this is a CONTRACT before
// its consumer: the retain-at-RunFinished + evict-at-C6 WIRING is now LIVE (the P3.48 conductor + handler)
// and get-at-C8 via the P3.50 `get_run_summary`; only `paths` (the P3.51 C9 leg) stays dead in the
// production build (covered by the module-level dead_code expect).

/// The §0.4.4 OFF-WIRE path table retained alongside a terminal `RunResult` (2026-07-06 core-owned-paths
/// ruling, §2.10.1) — the real `PathBuf`s the wire `RunResult` shed. The wire `RunResult` carries only
/// display strings (`common_root_display`/`divert_root_display`, each `ItemResult.output_display`, each
/// `CleanupResidue.residue_display`); the REAL paths live HERE, and C9 resolves its `OpenTarget` against
/// this table (P3.79): the roots for `CommonRoot`/`DivertRoot`, `item_outputs` for `Item(ItemId)`,
/// `item_residues` for `Residue(ItemId)`. Core-INTERNAL: NO `serde`/`specta` (it never crosses IPC — the
/// same store posture as `RunResultStore` itself). [Build-Session-Entscheidung: P3.76]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResultPaths {
    /// The REAL beside-source common-ancestor root (§2.7 / §7.7.3) — C9 `OpenTarget::CommonRoot`.
    pub common_root: PathBuf,
    /// The REAL divert root when any item diverted (§2.7.3), else `None` — C9 `OpenTarget::DivertRoot`.
    pub divert_root: Option<PathBuf>,
    /// Each succeeded item's REAL published output `PathBuf` (§2.1), keyed by `ItemId` — the C9
    /// `OpenTarget::Item(item)` file-launch membership target.
    pub item_outputs: BTreeMap<ItemId, PathBuf>,
    /// Each cleanup-incomplete item's REAL residue `PathBuf` (§2.6.4), keyed by `ItemId` — the C9
    /// `OpenTarget::Residue(item)` reveal target.
    pub item_residues: BTreeMap<ItemId, PathBuf>,
}

/// The retained terminal run — the wire `RunResult` (display strings) PLUS its off-wire `RunResultPaths`
/// (real paths). `RunResultStore` holds at most one (the latest run's, §0.4.4). Core-internal, no serde.
/// [Build-Session-Entscheidung: P3.76]
#[derive(Debug)]
struct RetainedRun {
    result: RunResult,
    paths: RunResultPaths,
}

/// The §0.4.4 RunResult retention — the process-local, in-memory store of the most-recent terminal
/// `RunResult` **plus its off-wire `RunResultPaths`**, kept so C8 `get_run_summary` can idempotently
/// re-serve the §1.12 summary after a WebView reload and so C9 `open_path` can resolve its `OpenTarget`
/// against the real paths (P3.79). Holds AT MOST ONE run (the latest): [`retain`](RunResultStore::retain)
/// on `RunFinished` stores it, [`evict`](RunResultStore::evict) on a new run's start (C6) clears the prior
/// one (the §0.4.4 "until a new run starts" eviction), [`get`](RunResultStore::get) serves the wire result
/// back to C8 (matched by `RunId` so a stale/other run is never served for the wrong id), and
/// [`current_paths`](RunResultStore::current_paths) serves the off-wire paths of the current run to C9 (which
/// carries no `RunId` on its wire, §7.7.2). NO on-disk persistence (§7.4) — the store
/// is dropped on process exit. Interior-mutable behind a `Mutex` (the `State` form serves concurrent
/// C6/C8/C9 handlers); the critical sections never hold the guard across an `.await`, so a `std::sync::Mutex`
/// is correct.
///
/// [Build-Session-Entscheidung: P2.43 → P3.76] `Default`-constructed empty; `Debug` for parity with the
/// sibling state. NOT a wire type (no `serde`/`specta`) — the `RunResult` it holds IS a wire type, but the
/// STORE (and its off-wire `RunResultPaths`) is pure core-internal State that never crosses IPC.
#[derive(Debug, Default)]
pub struct RunResultStore {
    /// The retained terminal run (wire result + off-wire paths, the latest run's), or `None` between an
    /// `evict` and the next `retain`. A single slot, not a per-`RunId` map: §0.4.4 retains only until the
    /// NEXT run starts.
    retained: Mutex<Option<RetainedRun>>,
}

impl RunResultStore {
    /// Lock the slot, recovering a poisoned guard rather than propagating the panic — the in-core no-panic
    /// discipline (G4/G14: no `unwrap`/`expect`/`panic`), sound because the critical sections never panic.
    /// [Build-Session-Entscheidung: P2.43]
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<RetainedRun>> {
        self.retained
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Retain the run's terminal summary + its off-wire real paths (`RunFinished`, §0.4.4) — supersedes any
    /// prior retained run (only the latest is kept). After this, C8 `get(result.run_id)` re-serves the wire
    /// summary and C9 `current_paths()` resolves its `OpenTarget` against the real paths (P3.51).
    /// [Build-Session-Entscheidung: P3.76] `paths` is the sibling off-wire `RunResultPaths` (the real roots +
    /// per-item output/residue `PathBuf`s the display-only wire `RunResult` shed, §2.10.1).
    pub fn retain(&self, result: RunResult, paths: RunResultPaths) {
        *self.lock() = Some(RetainedRun { result, paths });
    }

    /// Re-serve the retained wire summary for `run_id` (C8 `get_run_summary`, §0.4.4) — returns a clone iff a
    /// run is retained AND its `run_id` matches (so a superseded/other run's id never serves the wrong
    /// summary). `None` = no retained run, or it belongs to a different run (the C8 caller maps that to its
    /// §0.4.3 not-available error). The result is cloned out, so the guard is not held across the return.
    pub fn get(&self, run_id: RunId) -> Option<RunResult> {
        self.lock()
            .as_ref()
            .filter(|retained| retained.result.run_id == run_id)
            .map(|retained| retained.result.clone())
    }

    /// Re-serve the CURRENT (single latest) run's off-wire `RunResultPaths` — the C9 `open_path` accessor
    /// (§0.4.4 / §7.7.2, P3.51). It takes NO `RunId` (unlike the run-id-keyed [`get`](Self::get)): the §0.4.1 C9
    /// wire is id-keyed on the OPEN TARGET (`OpenTarget`), never on the run, and the §0.4.4 store holds AT MOST
    /// ONE run (the latest terminal one), so "the current run" is unambiguous (§7.7.2 "resolves it against the
    /// current run's `RunResultStore`"). `None` = no terminal run retained (→ the C9 §7.7.3 refusal). The paths
    /// are cloned out, so the guard is not held across the return. This is the C9 accessor — the run-id-keyed
    /// P3.76 `paths(run_id)` sibling is removed with P3.51, since the C9 wire carries no `run_id` (a run-id form
    /// would never be reachable). [Build-Session-Entscheidung: P3.51]
    pub fn current_paths(&self) -> Option<RunResultPaths> {
        self.lock().as_ref().map(|retained| retained.paths.clone())
    }

    /// Evict the retained run when a new run starts (C6, §0.4.4 "until a new run starts") — so a stale prior
    /// summary/paths are never re-served once the next run is in flight. Idempotent: evicting an already-empty
    /// store is a no-op.
    pub fn evict(&self) {
        *self.lock() = None;
    }
}

// ─── §0.4.4 collected-set registry — the CollectedSetId → FrozenCollectedSet resolve store (P2.44) ────
// [Build-Session-Entscheidung: P2.44] The THIRD §0.4.4 orchestrator-State store (after RunRegistry P2.42 +
// RunResultStore P2.43), homed here under the same §0.7 "(§0.4.4)" umbrella RunResultStore set the precedent
// for — a §0.4.4 State store added to orchestrator with NO §0.7/§1a structural edit (§0.7 attributes "(§0.4.4)"
// State to orchestrator; it enumerates the outcome-referencing TYPES, not every store, so a third store under
// the umbrella needs no fingerprint re-bless). It holds the `RegisteredSet` composite (the frozen
// `CollectedSet::Single` payload — a crate::domain FrozenCollectedSet, a downward orchestrator→domain edge like
// RunRegistry's RunId key — PLUS the §2.3 identity-evidence table, P3.40) keyed by CollectedSetId, so the
// bare-`collectedSetId` C3/C4/C5/C6 commands resolve back to the detected format / frozen items / roots /
// skipped / identities WITHOUT a second walk or re-detection (§0.4.4). Like the sibling stores it is
// a CONTRACT before its consumer: the take-at-C6 WIRING is now LIVE (the P3.48 handler `start_run` resolves
// + evicts the set); the register-at-C1/C2a-freeze + resolve-at-C3/C4/C5 legs are the P3.49 C-command
// bodies, still dead in the production build until then (covered by the module-level dead_code expect).

/// The §0.4.4 registered collected-set record — the orchestrator composite the [`CollectedSetRegistry`]
/// holds (the P3.40 frozen-model-identity ruling / §0.4.4 `[CLARIFIED]`): the domain [`FrozenCollectedSet`]
/// PLUS the §2.3 **identity evidence** — an `ItemId → FileIdentity` table over every RESOLVED survivor. The
/// evidence CANNOT be a field on the tier-3 `domain` `FrozenCollectedSet`: `FileIdentity` is a tier-2
/// `fs_guard` type, so embedding it would be an upward §0.7 edge — it rides here in the tier-1 orchestrator
/// value instead, so the "registered VALUE as a whole" carries everything §0.4.4 enumerates. The identities
/// key the §2.5.1 EquivKey `source_identity` (P3.39 — "identity, not path") and the §2.3.3 write-time
/// comparison set (the FULL table — §2.3's unqualified "any source in the frozen set", eligible AND skipped).
///
/// [Build-Session-Entscheidung: P3.40] Core-INTERNAL — no `serde`/`specta` (never crosses IPC; C3–C6 return
/// their own §0.6 DTOs). `Debug` for the store's parity; `PartialEq`+`Eq` back the registry lifecycle tests
/// (`FileIdentity`'s manual `Eq` over `(dev, inode)` composes). The registering fill (P3.49/P3.78) builds it
/// from the `FrozenSnapshot` the freeze retains; dead in the production build until then (the module-level
/// dead_code expect).
#[derive(Debug, PartialEq, Eq)]
pub struct RegisteredSet {
    /// The domain frozen set (the `CollectedSet::Single` projection) — the wire-facing content C3/C4/C5/C6
    /// resolve. A tier-3 leaf, so it holds NO `FileIdentity` (§0.7).
    pub frozen: FrozenCollectedSet,
    /// The §2.3 identity evidence (§0.4.4): `ItemId → FileIdentity` over every RESOLVED survivor — the
    /// eligible members AND the detect-ineligible skips alike (§0.6 invariant 6), keyed over the single id
    /// space. A walk/resolve-FAILURE skip has NO entry (its `resolve_identity` failed — a physical fact; an
    /// existing such file is still protected by the §2.1 exclusive-create like any pre-existing file).
    pub identities: BTreeMap<ItemId, FileIdentity>,
}

/// The §0.4.4 collected-set registry — maps each frozen `CollectedSetId` to its [`RegisteredSet`] (the
/// `CollectedSet::Single` payload PLUS the §2.3 identity evidence, P3.40), held as a Tauri app-managed
/// `State` so the bare-`collectedSetId` C3
/// `get_targets` / C4 `plan_output` / C5 `set_destination` / C6 `start_conversion` commands resolve back to the
/// frozen detection result without a second walk or re-detection (§0.4.4). Its §0.4.4 lifecycle is this type's
/// methods: [`register`](CollectedSetRegistry::register) on a C1/C2a freeze (store the `Single` projection,
/// SUPERSEDING any prior un-run set — §0.4.4 "a new C1/C2a supersedes it"),
/// [`resolve`](CollectedSetRegistry::resolve) at C3/C4/C5 (a non-evicting read), and
/// [`take`](CollectedSetRegistry::take) at C6 `start_conversion` (resolve + evict in one op — §0.4.4 "evicted
/// when its run starts: C6 hands the frozen items to the Batch"). A process exit drops the store (no on-disk
/// persistence, §7.4).
///
/// SUPERSEDE = at most one live entry: a new freeze clears any prior un-run set, so the registry never
/// accumulates stale entries (§2.4.3 "a subsequent drop starts a NEW frozen set, never mutates an in-flight one";
/// the in-flight set was already `take`n out by C6). The id-keyed map (not a single slot) is the §0.4.4 literal
/// — it lets `resolve`/`take` reject a stale/superseded `collectedSetId` (→ `None` → the C-command's §0.4.3
/// not-available error), never serving the wrong set for a mismatched id. Interior-mutable behind a `Mutex`
/// (the `State` form serves concurrent C1/C3/C4/C5/C6 handlers); every critical section is a whole-map op that
/// never holds the guard across an `.await`, so a plain `std::sync::Mutex` (not an async lock) is correct.
///
/// [Build-Session-Entscheidung: P2.44 → P3.40] Stores `Arc<RegisteredSet>` (not a bare value): C4 is
/// re-callable / debounced (~150 ms, §5.8) so the frozen set — a potentially-large `items` Vec + its
/// identities table — is READ MANY times per freeze; an `Arc` makes each `resolve`/`take` an O(1) handle
/// clone instead of an O(n) deep copy (the read-many extension of the cheap-`CancellationToken`-clone the
/// RunRegistry already relies on). `Default`-constructed empty; `Debug` for parity with the sibling stores.
/// NOT a wire type (no `serde`/`specta`) — pure core-internal State that never crosses IPC (C3–C6 return
/// their own §0.6 DTOs). P3.40 widened the stored value from the bare `FrozenCollectedSet` to the
/// [`RegisteredSet`] composite so the §0.4.4-mandated §2.3 identity evidence rides with it (§0.7-tier-clean).
#[derive(Debug, Default)]
pub struct CollectedSetRegistry {
    /// The live `CollectedSetId` → registered-set map. At most one entry (the current un-run set): `register`
    /// supersedes any prior, `take` (C6) removes it, a process exit drops the store.
    sets: Mutex<HashMap<CollectedSetId, Arc<RegisteredSet>>>,
}

impl CollectedSetRegistry {
    /// Lock the set map, recovering the guard from a poisoned lock rather than propagating the panic — the
    /// in-core no-panic discipline (G4/G14: no `unwrap`/`expect`/`panic`), sound because the critical
    /// sections are infallible whole-map ops that never panic. [Build-Session-Entscheidung: P2.44]
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<CollectedSetId, Arc<RegisteredSet>>> {
        self.sets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Register the frozen set on a C1/C2a freeze (§0.4.4) — SUPERSEDES any prior un-run set (§0.4.4 "a new
    /// C1/C2a supersedes it" + §2.4.3 "a subsequent drop starts a new frozen set"), so at most one entry is ever
    /// live. Keyed by the set's own `id` (`set.frozen.id`). After this, C3/C4/C5 `resolve(id)` and C6 `take(id)`
    /// see it. The `set` is the [`RegisteredSet`] composite (the frozen set + the §2.3 identity evidence, P3.40).
    pub fn register(&self, set: RegisteredSet) {
        let id = set.frozen.id;
        let mut sets = self.lock();
        sets.clear();
        sets.insert(id, Arc::new(set));
    }

    /// Resolve a `collectedSetId` to its registered set (C3/C4/C5, §0.4.4) — a NON-evicting read (C3/C4/C5 may
    /// each fire repeatedly; C4 is debounced-re-callable, §5.8). Returns the `Arc` clone iff `id` is the live
    /// set; `None` if `id` is unknown or was superseded (→ the C-command's §0.4.3 not-available error). The
    /// `Arc` is cloned out before the guard drops, so the lock is not held across the return. C4 reads
    /// `.identities[item]` off the resolved composite for the §2.5 re-run verdict (P3.40).
    pub fn resolve(&self, id: CollectedSetId) -> Option<Arc<RegisteredSet>> {
        self.lock().get(&id).map(Arc::clone)
    }

    /// Resolve AND evict the `collectedSetId` (C6 `start_conversion`, §0.4.4 "evicted when its run starts — C6
    /// hands the frozen items to the Batch") — one op so the set leaves the registry exactly as its run
    /// begins, never lingering to be re-run. Returns the `Arc` iff `id` was live; `None` otherwise (an unknown
    /// / already-superseded id → the C6 §0.4.3 not-available error).
    pub fn take(&self, id: CollectedSetId) -> Option<Arc<RegisteredSet>> {
        self.lock().remove(&id)
    }
}

// ─── §0.4.4 picked-destination registry — the DestinationId → PathBuf session store (P3.76) ────────────
// [Build-Session-Entscheidung: P3.76] The FIFTH §0.4.4 orchestrator-State store — the session-scoped
// picked-roots registry the 2026-07-06 core-owned-paths ruling introduces so a C2b-picked destination PATH
// never crosses the wire: C2b mints a DestinationId, stores the Rust-picked folder here, and returns the id
// (+ a display string); C4/C5/C6 resolve DestinationChoice::ChosenRoot(id) core-side against it (§0.4.4).
// Homed here under the same §0.7 "(§0.4.4)" State umbrella as the four sibling stores (no §0.7/§1a
// structural edit — the P2.43/P2.44/P2.45 precedent). Unlike the SUPERSEDING CollectedSetRegistry, this
// ACCUMULATES: §0.4.4 "entries live for the app session (they survive across collected sets, so switching
// batches never forces a re-pick) and die at app exit; nothing is persisted (§7.4)". Like the sibling
// stores it is a CONTRACT before its consumer: P3.80 wired the C4/C6 `resolve_choice` resolution + the
// `.manage` registration (so `resolve`/`resolve_choice` are LIVE); **P3.56 wires the remaining two —
// `register` (the C2b `pick_destination` real-pick body via `register_picked`) and
// `resolve_persisted_destination` (the C14 `get_initial_destination` hand-off) — so both are now LIVE in the
// production build.** The module-level `not(test)` dead_code expect stays fulfilled by the OTHER still-unwired
// items it covers (the §2.8 `project_outcome` / batch-summary renderer / §2.6.4 residue helpers — see its reason).

/// The §0.4.4 picked-destination registry — maps each C2b-minted `DestinationId` to its Rust-picked root
/// `PathBuf`, held as a Tauri app-managed `State` so a picked destination PATH never crosses the wire
/// (2026-07-06 core-owned-paths ruling, §2.10.1): C2b `pick_destination` mints an id + stores the folder
/// ([`register`](DestinationRegistry::register)); C4/C5/C6 resolve `DestinationChoice::ChosenRoot(id)`
/// against it ([`resolve`](DestinationRegistry::resolve)) — an unknown id is refused as a §0.4.3 error, so
/// the WebView can only *select among* user-picked roots, never name a path. SESSION-SCOPED: entries
/// accumulate and survive across collected sets (switching batches never forces a re-pick, §0.4.4) and die
/// at app exit — nothing is persisted (§7.4). Interior-mutable behind a `Mutex` (the `State` form serves
/// concurrent C2b/C4/C5/C6 handlers); every critical section is a whole-map op that never holds the guard
/// across an `.await`, so a plain `std::sync::Mutex` is correct.
///
/// [Build-Session-Entscheidung: P3.76 → P3.56] `Default`-constructed empty; `Debug` for parity with the sibling
/// stores. NOT a wire type (no `serde`/`specta`) — pure core-internal State that never crosses IPC (the
/// wire carries only the `DestinationId` + the C2b display string, §0.6). P3.80 wired the C4/C6
/// `resolve_choice` resolution + the `.manage` registration; **P3.56 makes `register` LIVE** in the production
/// build (the C2b `pick_destination` real-pick body registers a picked root via `register_picked`). The
/// module-level `not(test)` dead_code expect stays fulfilled by the other still-unwired items it covers (the §2.8
/// `project_outcome` / batch-summary renderer / §2.6.4 residue helpers — see its reason string).
#[derive(Debug, Default)]
pub struct DestinationRegistry {
    /// The session's `DestinationId` → picked-root map. ACCUMULATES across the session (unlike the
    /// superseding `CollectedSetRegistry`): every successful C2b pick adds an entry; a process exit drops
    /// the store; nothing is persisted (§7.4).
    roots: Mutex<HashMap<DestinationId, PathBuf>>,
}

impl DestinationRegistry {
    /// Lock the root map, recovering the guard from a poisoned lock rather than propagating the panic — the
    /// in-core no-panic discipline (G4/G14: no `unwrap`/`expect`/`panic`), sound because the critical
    /// sections are infallible whole-map ops that never panic. [Build-Session-Entscheidung: P3.76]
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<DestinationId, PathBuf>> {
        self.roots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Register a C2b-picked destination root (§0.4.4) — mints a fresh `DestinationId`, stores `path` against
    /// it, and returns the id (which C2b returns to the WebView paired with a display string; the PATH never
    /// crosses the wire). ACCUMULATES: prior entries are retained so switching batches never forces a re-pick
    /// (§0.4.4). [Build-Session-Entscheidung: P3.76]
    pub fn register(&self, path: PathBuf) -> DestinationId {
        let id = DestinationId::mint();
        self.lock().insert(id, path);
        id
    }

    /// Resolve a `DestinationId` to its picked-root `PathBuf` (C4/C5/C6 `ChosenRoot(id)`, §0.4.4) — returns a
    /// clone iff `id` is a live picked root; `None` if unknown (→ the C-command's §0.4.3 not-available /
    /// refusal error — the WebView cannot name a path the user never picked). The `PathBuf` is cloned out
    /// before the guard drops, so the lock is not held across the return. [Build-Session-Entscheidung: P3.76]
    pub fn resolve(&self, id: DestinationId) -> Option<PathBuf> {
        self.lock().get(&id).cloned()
    }

    /// Resolve a wire [`DestinationChoice`] to its core [`ResolvedDestination`] — the single fallible step the
    /// C4/C6 handlers run at the IPC boundary (§0.4.4). `BesideSource` maps through unchanged; a
    /// `ChosenRoot(id)` resolves the id to its picked-root `PathBuf` via [`resolve`](DestinationRegistry::resolve).
    /// `None` ONLY when a `ChosenRoot(id)` names an unknown id — the §0.4.3 refusal the C4/C6 handler maps to its
    /// not-available `IpcError` (the WebView cannot name a root the user never picked); `BesideSource` never
    /// fails. The pure §1.8/§2.7 legs then consume the `ResolvedDestination`, so no registry lookup ever reaches
    /// the planning/convert path (the 2026-07-06 core-owned-paths split — the C9 `resolve_open_target`
    /// id-resolution mirror). [Build-Session-Entscheidung: P3.80]
    pub fn resolve_choice(&self, choice: &DestinationChoice) -> Option<ResolvedDestination> {
        match choice {
            DestinationChoice::BesideSource => Some(ResolvedDestination::BesideSource),
            DestinationChoice::ChosenRoot(id) => {
                self.resolve(*id).map(ResolvedDestination::ChosenRoot)
            }
        }
    }
}

/// **§7.4 persisted-last resolver (leg 3, P3.80; return re-cut P3.56)** — load + validate the §7.4.1
/// `lastDestinationMode` pref core-side into the §0.4.4 [`DestinationRegistry`], yielding the 3-way
/// [`InitialDestination`] the C14 handler returns and the frontend maps onto C4's first destination (its
/// `ChosenRoot` arm carries the [`DestinationPicked`] id + display; the WebView never touches the stored path —
/// the 2026-07-06 core-owned-paths ruling / the P2.88 `[Superseded]` re-cut). Given the P2.85-built [`LastDestinationMode`] union
/// (`BesideSource | ChosenPath(PathBuf)` — the §7.4.1 `"beside-source" | "<absolute path>"` VALUE; the
/// §7.4.1-key-name-vs-§5.8-value "mode-vs-path" reading is a non-finding, the ruling's steelman: the
/// `ChosenPath` arm's value IS the path):
/// - `BesideSource` → [`InitialDestination::BesideSource`] (the §2.7.1 default, nothing registered, NO fallback);
/// - `ChosenPath(path)` → re-validate the path as WRITABLE via the §2.7.2 [`location_status`] machinery
///   (§7.4.1 "always re-validated as writable at use time"): a `Writable` verdict mints + registers a
///   `DestinationId` and returns [`InitialDestination::ChosenRoot`]`(DestinationPicked{ destination, display })`;
///   a `Divert(_)` verdict (the path is gone / read-only / ephemeral / no-atomic-publish) →
///   [`InitialDestination::Fallback`] (nothing registered — the §2.7 per-location fallback, §5.8:926 "falls back
///   to beside-source"), so a stale pref never reaches the no-harm machinery unchecked. **`Fallback` is
///   STRUCTURALLY distinct from `BesideSource`** so the §5.8 passive fallback note surfaces even when beside-source
///   itself is writable (the G1 Opus-P2 adoption — only this resolver knows the path failed re-validation).
///
/// PURE + directly testable: it takes the pref VALUE + a real `&DestinationRegistry` + the caller's `crate::run`
/// grammar probe name (passed IN — `fs_guard`'s `location_status` never depends on `crate::run`), performing NO
/// `AppHandle` / `prefs::load` read itself. **LIVE via P3.56:** its AppHandle-coupled consumer is the C14
/// `get_initial_destination` handler (`crate::ipc::planning`) — the `prefs::load(app).last_destination_mode` +
/// `State<InstanceId>` (probe) + `State<DestinationRegistry>` read the frontend's `advanceToTargets` hand-off runs
/// at the Confirm→Targets advance (§5.8:918); the module-level `not(test)` `dead_code` expect stays fulfilled by
/// the other still-dead items it covers. [Build-Session-Entscheidung: P3.80 → P3.56]
pub fn resolve_persisted_destination(
    last: &LastDestinationMode,
    registry: &DestinationRegistry,
    probe_name: &OsStr,
) -> InitialDestination {
    let LastDestinationMode::ChosenPath(path) = last else {
        // §7.4.1 `"beside-source"` sentinel → the §2.7.1 default; nothing registered, NO fallback (a plain pref).
        return InitialDestination::BesideSource;
    };
    match location_status(path, probe_name) {
        LocationStatus::Writable => {
            let display = path.to_string_lossy().into_owned();
            let destination = registry.register(path.clone());
            InitialDestination::ChosenRoot(DestinationPicked {
                destination,
                display,
            })
        }
        // §7.4.1/§2.7/§5.8: a gone / read-only / ephemeral persisted path → the beside-source FALLBACK, nothing
        // registered. The `Fallback` fact (STRUCTURALLY distinct from the plain `BesideSource` above) drives the
        // §5.8:926 passive fallback note the P3.56 DestinationBar renders (§5.7:825 chrome) — surfaced EVEN when
        // beside-source itself is writable (only this resolver knows the persisted path failed re-validation).
        LocationStatus::Divert(_) => InitialDestination::Fallback,
    }
}

// ─── §0.4.4 ingest registry — the CollectingId → CancellationToken ingest-cancellation store (P2.45) ──
// [Build-Session-Entscheidung: P2.45] The FOURTH §0.4.4 orchestrator-State store, the one-phase-EARLIER
// sibling of the RunRegistry (P2.42): same RunId-token shape, but keyed by the frontend-generated
// CollectingId (§0.4.4 / §1.1) so C13 cancel_ingest can trip an IN-FLIGHT C1 walk / C2a pick before its
// long await resolves. Homed here under the same §0.7 "(§0.4.4) cancellation" umbrella as the RunRegistry
// (no §0.7/§1a structural edit — the P2.43/P2.44 precedent). Like the sibling stores it is a CONTRACT
// before its consumer, but is now CONSUMED (live): the register-at-handler-entry (the C1 `drain_intake` walk
// start, §1.1 — the C2a picker walks nothing now, §0.4.1, so only C1 registers) / cancel-at-C13 /
// release-on-EVERY-handler-exit-branch WIRING is LIVE — the C1 drain register-guard (P3.49) + the C13
// `cancel_ingest` trip (P2.71) + the §1.1 walk-loop poll (P2.69) — so this store is no longer dead.

/// The §0.4.4 ingest registry — maps each in-flight `CollectingId` to its `tokio_util::sync::
/// CancellationToken`, so C13 `cancel_ingest` can cooperatively cancel an in-flight C1 walk / C2a pick
/// (§0.4.4 / §1.1). The one-phase-EARLIER sibling of [`RunRegistry`]: the `CollectingId` is minted by the
/// frontend and handed to C1/C2a as an argument (so a C13 can target the ingest before any `RunId`
/// exists). Held as a Tauri app-managed `State` (the wiring is the C1/C2a/C13 handlers). Its §0.4.4
/// lifecycle is this type's three methods: [`register`](IngestRegistry::register) at handler entry (C1 at
/// the start of the walk; C2a *before* the native dialog opens, §1.1, so a C13 during the modal is
/// honoured), [`cancel`](IngestRegistry::cancel) at C13 `cancel_ingest` (trip it — cooperative), and
/// [`release`](IngestRegistry::release) on **EVERY** handler exit branch (the normal walk-completes return,
/// the C13-tripped return, AND the C2a cancelled-dialog → `CollectedSet::Empty` return — the spec's
/// "the handler drops it explicitly, no token leak", §0.4.4). Interior-mutable behind a `Mutex` (the
/// `State` form serves concurrent C1/C2a/C13 handlers); every critical section is a whole-map op that never
/// holds the guard across an `.await`, so a plain `std::sync::Mutex` (not an async lock) is correct.
///
/// [Build-Session-Entscheidung: P2.45] `Default`-constructed empty; `Debug` for parity with the sibling
/// stores. NOT a wire type (no `serde`/`specta`) — pure core-internal State that never crosses IPC (the
/// WebView drives ingest cancellation through the C13 command + the minted `CollectingId`, never sees a
/// token). The exit-drop method is named `release` (not the `RunRegistry`'s `finish`) because it fires on
/// EVERY exit branch — cancelled / errored / completed alike — which "finish" would mis-describe.
#[derive(Debug, Default)]
pub struct IngestRegistry {
    /// The active `CollectingId` → `CancellationToken` map. A `CollectingId` is a frontend-minted per-ingest
    /// v4 (§0.4.4), registered once at handler entry and removed once on the handler's exit, so no key ever
    /// legitimately collides.
    tokens: Mutex<HashMap<CollectingId, CancellationToken>>,
}

impl IngestRegistry {
    /// Lock the token map, recovering the guard from a poisoned lock rather than propagating the panic — the
    /// in-core no-panic discipline (G4/G14: no `unwrap`/`expect`/`panic`), sound because the critical
    /// sections are infallible whole-map ops that never panic. [Build-Session-Entscheidung: P2.45]
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<CollectingId, CancellationToken>> {
        self.tokens
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Register a fresh `CancellationToken` for `collecting_id` at handler entry (C1 walk start / C2a before
    /// the dialog opens, §0.4.4 / §1.1) and store it. Returns a clone for the walk/pick to poll: a
    /// `CancellationToken` is a cheap `Arc`-backed handle, so the stored copy and the returned copy share ONE
    /// cancellation state (a C13 `cancel` trips all). A `CollectingId` is unique per ingest (§0.4.4), so this
    /// never overwrites a live entry; a collision could only be a programming error and would merely drop the
    /// stale token.
    pub fn register(&self, collecting_id: CollectingId) -> CancellationToken {
        let token = CancellationToken::new();
        self.lock().insert(collecting_id, token.clone());
        token
    }

    /// Trip the `collecting_id`'s token (C13 `cancel_ingest`, §0.4.4) — cooperative cancellation of the
    /// in-flight ingest. Returns `true` if a live token was found and tripped, `false` if `collecting_id` is
    /// unknown or already released — the §0.4.1 C13 idempotent no-op-cancel case (cancelling a
    /// completed/absent ingest is a clean no-op the C13 handler maps to `Ok(())`, never an error). The token
    /// is left IN the map until [`release`](IngestRegistry::release) (the walk may still observe the cancel
    /// before the handler exits). The token is cloned out before the guard drops, so `.cancel()` runs without
    /// holding the lock.
    pub fn cancel(&self, collecting_id: CollectingId) -> bool {
        let token = self.lock().get(&collecting_id).cloned();
        match token {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    /// Drop the `collecting_id`'s token on a handler exit (§0.4.4) — called on **EVERY** exit branch of the
    /// C1/C2a handler (normal walk-completes, C13-tripped, and the C2a cancelled-dialog → `Empty` return,
    /// where the walk loop that would otherwise drop it never runs), so no token leaks. This is NOT a cancel
    /// — an outstanding poll clone is left un-cancelled on the normal-completion branch. Idempotent:
    /// releasing an unknown / already-released ingest is a no-op.
    pub fn release(&self, collecting_id: CollectingId) {
        self.lock().remove(&collecting_id);
    }

    /// Register `collecting_id`'s token (as [`register`](IngestRegistry::register)) and return an **RAII
    /// guard** that [`release`](IngestRegistry::release)s it on **Drop** — the §1.1 "token drop on EVERY C2a
    /// exit branch" realized **by construction** (P2.70). The C2a `pick_for_intake` handler binds the guard
    /// **before opening the native dialog**, so every return path — the picked-and-funnelled branch, the
    /// dialog-cancelled → `Empty` branch, the C13-tripped → `Empty` branch, and any `?` early-return — drops
    /// the guard and de-registers the token; no branch can leak it (the §1.1 hazard: the walk loop that
    /// normally drops the token never runs on a cancelled dialog). Registering before the modal also keeps
    /// C13 honest: a `cancel_ingest` arriving while the dialog is up trips this token, which the handler reads
    /// post-dialog via [`IngestGuard::is_cancelled`]. [Build-Session-Entscheidung: P2.70]
    pub fn register_guard(&self, collecting_id: CollectingId) -> IngestGuard<'_> {
        let token = self.register(collecting_id);
        IngestGuard {
            registry: self,
            collecting_id,
            token,
        }
    }
}

/// An RAII registration guard for an ingest-scoped `CollectingId` token (§1.1, P2.70) — returned by
/// [`IngestRegistry::register_guard`]. Its [`Drop`] calls [`IngestRegistry::release`], so the §1.1 "drop on
/// EVERY C2a exit branch" rule holds **structurally**: whichever way the C2a handler returns (picked,
/// dialog-cancelled, C13-tripped, or an error `?`), the guard drops and the token is de-registered — no leak.
/// It borrows the registry (not an `AppHandle`), so the guard's drop-on-every-branch behaviour is unit-tested
/// against a real `IngestRegistry` with no Tauri runtime. [`is_cancelled`](IngestGuard::is_cancelled) exposes
/// the token's state for the §1.1 post-dialog check.
pub struct IngestGuard<'r> {
    registry: &'r IngestRegistry,
    collecting_id: CollectingId,
    token: CancellationToken,
}

impl IngestGuard<'_> {
    /// Whether a C13 `cancel_ingest` tripped this ingest's token while the C2a dialog was up (the §1.1
    /// post-dialog check — on a trip the handler abandons the picked paths and yields `CollectedSet::Empty`
    /// rather than walking them). A LIVE check: until P2.71 wires C13's `.cancel()` nothing trips the token,
    /// so this reads `false` — a reachable-by-construction real read, not an inert no-op (no hole).
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// The raw ingest-scoped cancellation token, polled cooperatively by the §1.1 C1 `drain_intake` walk +
    /// §1.2 detection loop (`ingest`, P3.49): the walk hands `&guard.token()` to `walk_intake_roots` and
    /// re-checks it between detected files, so a C13 `cancel_ingest` trip stops the in-flight walk and the
    /// partial, not-yet-frozen set is discarded (§1.1). Distinct from [`is_cancelled`](Self::is_cancelled),
    /// the one-shot boolean the C2a post-dialog check read: the walk needs the token itself to thread into
    /// `walk_intake_roots`. [Build-Session-Entscheidung: P3.49]
    pub fn token(&self) -> &CancellationToken {
        &self.token
    }
}

impl Drop for IngestGuard<'_> {
    fn drop(&mut self) {
        // §1.1 (P2.70): de-register the ingest token on the C2a handler's exit — fires on every return path
        // (Rust drops the guard regardless of which branch returns), so the token can never leak.
        self.registry.release(self.collecting_id);
    }
}

// ─── §2.5.1 EquivKey computation — the re-run equivalence key computer (P3.39) ────────────────────────────
// [Build-Session-Entscheidung: P3.39] The §2.5.1 EquivKey COMPUTATION homes here in the tier-1 orchestrator
// (the P3.38 prevention-sweep ruling): the key folds a `fs_guard::FileIdentity` + the §0.6 `TargetId` /
// `OptionValues`, and the orchestrator already holds all three (the frozen set carries the identities, C4/C6
// carry target + settings), so computing it here needs no `run`->`fs_guard` sibling edge (§0.7 forbids it).
// It hands only the finished hash DOWN to the tier-2 `crate::run` ledger as an opaque `EquivKey`. An
// app-managed singleton like the §0.4.4 stores above — it needs NO §0.7/§1a structural edit (§0.7 attributes
// the app-managed State to orchestrator; the P2.43/P2.44/P2.45/P2.58 precedent). `compute_equiv_key` is now
// LIVE — the P3.48 C6 conductor resolves the managed `State<EquivKeyComputer>` and folds each item's re-run
// key in the §2.5 applier (`has_seen`) + the per-success `RerunLedger::record`; its sibling
// `compute_rerun_verdict` (the C4 `plan_output` VERDICT) is the SECOND consumer, still dead until the P3.49 C4
// wiring (the module-level dead_code expect stays fulfilled while it is unwired).

/// The §2.5.1 re-run equivalence-key computer — holds the ONE process-lifetime `BuildHasher` so two computes
/// of the same `(source, target, effective settings)` in a session agree on the resulting `u64` (§2.5.2: the
/// seed-stable compute-side hasher, NEVER a fresh `RandomState` per call — two computes of the same job must
/// agree). The ledger is session-only (§2.5.2 cleared-on-quit, §7.4 persists nothing), so NO cross-run /
/// cross-version hash stability is needed — a per-process random seed is sufficient and this instance is held
/// for the process lifetime as the app-managed `State<EquivKeyComputer>`. It folds a `fs_guard::FileIdentity`
/// (its manual `Hash` covers the `(dev, inode)` identity only, NOT the path — §2.5.1 "identity, not path") +
/// the §0.6 `TargetId` + `OptionValues` (the `BTreeMap` hashes in sorted-key order, §2.5.1's order-independent
/// canonical form), then hands the finished hash to `crate::run` as an opaque `EquivKey`. Core-internal — no
/// `serde`/`specta` (`RandomState` is not serialisable and the key never crosses IPC, the P2.22 forward-guard).
#[derive(Debug, Default)]
pub struct EquivKeyComputer {
    /// The held, seed-stable `BuildHasher`. `RandomState::default()` seeds ONCE at construction (app start);
    /// every `compute_equiv_key` builds a fresh `Hasher` off this SAME seed, so identical inputs fold to the
    /// same `u64` within the process (§2.5.2). The `run`-side `HashSet<EquivKey>`'s own bucket-hasher is a
    /// separate, independent `RandomState` — the two never need to agree (§2.5.2).
    hasher: RandomState,
}

impl EquivKeyComputer {
    /// Compute the §2.5.1 `EquivKey` for one `(source, target, effective settings)` conversion — the fold
    /// `hash(source_identity, target_format, effective_settings_canon)`. `settings` MUST already be the §1.6
    /// fully-defaulted-plus-overrides set (the caller resolves defaults; §2.5.1 "effective_settings_canon");
    /// its `BTreeMap` hashes in sorted-key order so "left everything default" twice yields the same key. The
    /// three components are folded into ONE hasher in a FIXED field order (§2.5.1); a `u64` collision's only
    /// consequence is one spurious RerunPrompt (a hint dialog — §2.1's exclusive-create makes data loss
    /// impossible regardless). Returns the opaque key for `crate::run::RerunLedger::{has_seen, record}`.
    pub fn compute_equiv_key(
        &self,
        source: &FileIdentity,
        target: TargetId,
        settings: &OptionValues,
    ) -> EquivKey {
        let mut hasher = self.hasher.build_hasher();
        source.hash(&mut hasher);
        target.hash(&mut hasher);
        settings.hash(&mut hasher);
        EquivKey::from_hash(hasher.finish())
    }
}

/// Compute the §2.5 batch-level re-run verdict for a planned conversion (C4 `plan_output`, P3.40) — the
/// `OutputPlanPreview.rerun` field. For each **eligible** frozen member, fold its freeze-RETAINED §2.3
/// `source_identity` (`set.identities[item]`, the P3.40 evidence) + the `target` + the effective `settings`
/// into an `EquivKey` (P3.39) and query the in-session `ledger`; the batch-level prompt fires (`Some`) iff
/// **any** eligible item is an equivalent prior in-session run (§2.5.2 signal 1 — the sole v1 firing
/// authority), carrying the `equivalent_count`. Only the eligible `items` are candidates — a `skipped` item
/// is not converted, so it can never be a re-run (its identity is retained only for the §2.3.3 comparison
/// set, not this verdict).
///
/// **§2.5.3 never-overwrite fallback (inherent, not a branch):** an item the ledger has NOT seen — a new
/// session (empty ledger), or a prior output renamed/moved so equivalence can't be determined — simply is
/// not counted, so no prompt fires for it and it falls through to §2.2 silent next-free-variant numbering at
/// publish. The failure mode is a harmless extra numbered copy, NEVER an overwrite (which §2.1's
/// exclusive-create makes impossible regardless, §2.5.3). An eligible item that (defensively) has no retained
/// identity is likewise treated as "not equivalent" — a re-run is only ever ASSERTED on positive evidence.
///
/// [Build-Session-Entscheidung: P3.40] A free function (it composes two orchestrator-State singletons — the
/// `EquivKeyComputer` compute-side hasher and the `RerunLedger` store — over a resolved `RegisteredSet`);
/// C4 (P3.49) resolves the composite from the `CollectedSetRegistry`, calls this, and seats the result in
/// `OutputPlanPreview.rerun`. LIVE since P3.49 — the C4 `plan_output_preview` is its first production caller
/// (see the module `reason=`).
pub fn compute_rerun_verdict(
    set: &RegisteredSet,
    target: TargetId,
    settings: &OptionValues,
    computer: &EquivKeyComputer,
    ledger: &RerunLedger,
) -> Option<RerunPrompt> {
    let equivalent_count = set
        .frozen
        .items
        .iter()
        .filter(|item| {
            set.identities.get(&item.item).is_some_and(|identity| {
                ledger.has_seen(computer.compute_equiv_key(identity, target, settings))
            })
        })
        .count();
    (equivalent_count > 0).then_some(RerunPrompt { equivalent_count })
}

// ─── §7.8.1 intake buffer — the PendingIntake stash/drain store (P2.58; single hand-off buffer P3.77) ─────
// [Build-Session-Entscheidung: P2.58] The §7.8.1 intake buffer, homed here under the same §0.7 State
// umbrella as the four §0.4.4 sibling stores (RunRegistry/RunResultStore/CollectedSetRegistry/IngestRegistry)
// — a launch-intake State store added to orchestrator needs NO §0.7/§1a structural edit (§0.7 attributes the
// app-managed State to orchestrator + enumerates the outcome-referencing TYPES, not every store; the
// P2.43/P2.44/P2.45 precedent). It is the single-slot sibling of `RunResultStore` (a `Mutex<Option<…>>`).
// [Build-Session-Entscheidung: P3.77/P3.78] The 2026-07-06 core-owned-path ruling makes this the SINGLE hand-off
// buffer for EVERY non-busy intake origin — the native drop, launch-arg, second-instance, Open-with, AND the C2a
// picker's picked set (§7.8.1): the §7.8.1 intake funnel (`stash_pending_intake`, main.rs) ALWAYS `stash`es the
// set here (the former `Emit` arm — a payload-carrying `app://intake` emit — is retired with `IntakePayload`; the
// nudge is now payload-less), and the C1 `drain_intake` drain (P2.60/P3.78) TAKEs it and freezes it (§1.1). The
// C2a picker now joins this same funnel (P3.78): it stashes its picked set here via `forward_launch_intake` and
// the C1 drain collects it, exactly like every other origin — no source-specific freeze path exists. It differs
// from the §0.4.4 run/ingest stores only in WHO drives it — the intake glue writes, the C1 drain reads (via
// `take_marking_ready`, live since P2.60) — but the State-store shape + the contract-before-consumer discipline
// are identical.

/// One buffered §7.8.1 intake — a path set + its §0.6 `IntakeOrigin`. Stashed by the intake funnel (every
/// non-busy launch/drop origin — drop, launch-arg, second-instance, P3.77; and the C2a picker, P3.78),
/// drained once by the C1 drain (P2.60).
/// NOT a wire type: the C1 drain returns a `CollectedSet` (§0.4.1), never this buffer (pure core-internal
/// State). [Build-Session-Entscheidung: P2.58]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferedLaunchIntake {
    /// The intake paths (already `parse_path_args`-classified for a launch/second-instance set, §7.8.1),
    /// accumulated across any repeat stash before the drain (no-loss — see [`PendingIntake::stash`]).
    pub paths: Vec<PathBuf>,
    /// The §0.6 origin of the FIRST stash before this drain (typically `LaunchArg`/`Drop`; §7.8.1 "its stored
    /// origin"). A subsequent stash before the drain accumulates its paths but keeps this origin — the §1.1
    /// freeze re-validates every path and is origin-agnostic, so one stored origin for the merged set is
    /// correct.
    pub origin: IntakeOrigin,
}

/// The §7.8.1 intake buffer (`State<PendingIntake>`) — holds at most one un-drained
/// [`BufferedLaunchIntake`]. The single-slot sibling of [`RunResultStore`]: the intake funnel `stash`es every
/// non-busy launch/drop intake set here (drop, launch-arg, second-instance — the single hand-off buffer, P3.77;
/// and the C2a picker since P3.78), and the C1 drain (P2.60/P3.78) drains it exactly once per call. Held as a Tauri app-managed `State`
/// (registered in `main()`'s Builder chain). Interior-mutable behind a `Mutex` (the `State` form is shared
/// across the intake hooks + the C1 handler); the critical sections are infallible slot ops that never hold
/// the guard across an `.await`, so a `std::sync::Mutex` is correct.
///
/// [Build-Session-Entscheidung: P2.58] `Default`-constructed empty; `Debug` for parity with the sibling
/// stores. NOT a wire type (no `serde`/`specta`) — pure core-internal State that never crosses IPC (the C1
/// drain returns a `CollectedSet`, §0.4.1).
#[derive(Debug, Default)]
pub struct PendingIntake {
    /// The single buffered intake set, or `None` when nothing is pending (never stashed, or already drained).
    pending: Mutex<Option<BufferedLaunchIntake>>,
}

impl PendingIntake {
    /// Lock the slot, recovering a poisoned guard rather than propagating the panic — the in-core no-panic
    /// discipline (G4/G14: no `unwrap`/`expect`/`panic`), sound because the critical sections never panic.
    /// [Build-Session-Entscheidung: P2.58]
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<BufferedLaunchIntake>> {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Stash an intake set into the §7.8.1 buffer (every non-busy launch/drop origin — drop, launch-arg,
    /// second-instance, P3.77; and the C2a picker, P3.78). NO-LOSS on a repeat stash before the drain: a second intake before the drain
    /// APPENDS its paths to the pending set rather than superseding it — superseding would drop the earlier
    /// set's paths, the very loss this buffer exists to prevent (§7.8.1) — and keeps the FIRST origin (§7.8.1
    /// "its stored origin"). The funnel only reaches this with a non-empty `paths` (it returns early on empty,
    /// §7.8.1), so no empty-stash guard is needed. [Build-Session-Entscheidung: P2.58]
    ///
    /// **The §7.8.1 no-loss ordering (the stash-vs-drain interleaving, P3.77):** the 2026-07-06 core-owned-path
    /// ruling collapsed the former Emit-vs-Buffer split into ALWAYS-stash + a payload-less nudge, so the
    /// `RouteToEmit` re-route and the under-lock ready re-check are structurally unnecessary — this op simply
    /// buffers. Two rules replace the former lock dance: the funnel **stashes BEFORE it reads the ready flag**,
    /// and the drain **marks ready BEFORE it takes**
    /// ([`take_marking_ready`](PendingIntake::take_marking_ready)). So every intake set is either stashed
    /// strictly before the drain's take (the drain observes it) or its stash precedes a `is_ready()` read that
    /// sees `true` (the mark happened in the earlier drain) → the funnel nudges → the next drain consumes it;
    /// the worst outcome of any interleaving is a harmless empty drain, never a stranded or double-ingested
    /// set. Proven by `state_stores::stash_after_drain_stays_buffered_for_the_nudge_drain` (+ the two-thread
    /// stress leg). [Build-Session-Entscheidung: P3.77]
    pub fn stash(&self, paths: Vec<PathBuf>, origin: IntakeOrigin) {
        let mut slot = self.lock();
        match slot.as_mut() {
            Some(buffered) => buffered.paths.extend(paths),
            None => *slot = Some(BufferedLaunchIntake { paths, origin }),
        }
    }

    /// Mark the frontend ready AND take the buffered intake set — the C1 drain's two cohesive effects (P2.60,
    /// §7.8.1 "consumes `PendingIntake` exactly once") ordered under the pending-slot `Mutex`. Mark-BEFORE-take
    /// inside the lock is the §7.8.1 no-loss ordering's second rule (P3.77 — see [`stash`](PendingIntake::stash)):
    /// an intake whose `stash` serializes after this section is followed by a funnel `is_ready()` read that
    /// observes `ready == true` (the mark), so the funnel nudges and the next drain consumes it — nothing is
    /// stranded. Returns `None` when nothing is pending (the ordinary drain with no files → C1 returns
    /// `CollectedSet::Empty`, §0.4.1). Idempotent: a second drain is `None`. [Build-Session-Entscheidung: P2.137]
    pub fn take_marking_ready(&self, ready: &FrontendReady) -> Option<BufferedLaunchIntake> {
        let mut slot = self.lock();
        ready.mark_ready();
        slot.take()
    }
}

// ─── §7.8.1 WebView-ready flag — the FrontendReady nudge gate (P2.59; nudge-vs-silent P3.77) ──────────────
// [Build-Session-Entscheidung: P2.59] The §7.8.1 / §0.4.2 WebView-ready flag, homed here under the same §0.7
// State umbrella as the intake sibling PendingIntake (P2.58) + the four §0.4.4 stores — a launch-intake
// State store added to orchestrator needs NO §0.7/§1a structural edit (the P2.58 precedent: §0.7 attributes the
// app-managed State to orchestrator, not every store). It records whether the WebView's `app://intake` listener
// is registered. [Build-Session-Entscheidung: P3.77] After the 2026-07-06 core-owned-path ruling the funnel
// ALWAYS stashes into `PendingIntake` and reads this flag ONLY to decide whether to NUDGE: the §7.8.1 intake
// funnel reads it (`frontend_ready`, main.rs) AFTER the stash to emit a payload-less `app://intake` nudge iff
// ready (not-ready → no nudge; the root-shell mount drains the stash once regardless, §5.8), and the C1 drain
// (P2.60 — on root-shell mount, AFTER the listener registers) marks it ready. MONOTONIC false→true: the `main`
// window lives for the whole session (§7.3.1 closing-quits) so the listener never un-registers, hence the flag
// never resets — an `AtomicBool` is the right tool (no Mutex/poison handling; the reader needs only the
// published boolean, no data is gated behind it). Both methods are LIVE: `mark_ready` is driven by the C1
// `drain_intake` handler via the fused [`PendingIntake::take_marking_ready`]
// (`crate::ipc::intake::drain_to_collected_set` calls it — every drain call is the §7.8.1 readiness signal),
// and `is_ready` is read by the §7.8.1 funnel's `frontend_ready` (P2.59, main.rs) after the stash.

/// The §7.8.1 WebView-ready flag (`State<FrontendReady>`) — `true` once the frontend has registered its
/// `app://intake` listener and run the C1 drain (P2.60) on root-shell mount. The §7.8.1 intake funnel reads it
/// (`frontend_ready`, main.rs) AFTER the stash to decide whether to nudge: an intake set arriving BEFORE the
/// listener exists (the first-launch race, §7.8.1) is still buffered into [`PendingIntake`] (the funnel always
/// stashes, P3.77) — the flag only gates the payload-less nudge, and the mount drain collects the stash either
/// way. Held as a Tauri app-managed `State` (registered in `main()`'s Builder chain, so the funnel's
/// `frontend_ready` resolve is infallible by construction). A monotonic false→true flag, so an `AtomicBool`
/// (no `Mutex`/poison handling) is the right shape.
///
/// [Build-Session-Entscheidung: P2.59] `Default`-constructed `false` (not-ready at app start — the funnel's
/// fail-safe default: no nudge is emitted until the frontend proves its listener exists; the stash is still
/// taken by the mount drain); `Debug` for parity with the sibling State stores. NOT a wire type (no
/// `serde`/`specta`) — pure core-internal State that never crosses IPC.
#[derive(Debug, Default)]
pub struct FrontendReady {
    /// `true` once the WebView's `app://intake` listener is live (set by the C1 drain, P2.60).
    ready: AtomicBool,
}

impl FrontendReady {
    /// Mark the frontend ready — the WebView has registered its `app://intake` listener and is draining
    /// `PendingIntake` (the C1 drain on root-shell mount, P2.60). Monotonic: once set it never clears (the
    /// `main` window lives for the whole session, §7.3.1), so a repeat call is a harmless no-op. `Release`
    /// publishes the write so a subsequent `is_ready` `Acquire` observes it. [Build-Session-Entscheidung: P2.59]
    pub fn mark_ready(&self) {
        self.ready.store(true, Ordering::Release);
    }

    /// Read the §7.8.1 ready flag — `true` once [`mark_ready`](FrontendReady::mark_ready) has fired. The §7.8.1
    /// intake funnel's `frontend_ready` predicate (main.rs) reads this AFTER the stash to decide whether to emit
    /// the payload-less `app://intake` nudge (ready → nudge; not-ready → no nudge, the mount drain collects the
    /// stash, P3.77). `Acquire` pairs with `mark_ready`'s `Release`. [Build-Session-Entscheidung: P2.59]
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
}

/// The §1.1 / §2.4 **intake-freeze funnel** — the single, exhaustive freeze point every intake entry
/// point routes through (SSOT *Never harm the original*). [Build-Session-Entscheidung: P3.78] All §1.1 entry
/// points reduce to this one Rust function via the C1 `drain_intake` drain: every origin (the native drop,
/// launch-arg / second-instance / Open-with, and the C2a `pick_for_intake` picked set — origin stamped
/// `Picker` by the C2a handler, §1.1) funnels core-side into the §7.8.1 `PendingIntake` buffer, and the C1
/// drain (`drain_to_collected_set`) hands the drained set here with its stored origin — so the §2.4 freeze
/// and the §1.3 one-batch rule are enforced ONCE here, never duplicated per entry point. It builds the
/// frozen source set **eagerly and once, before any conversion** (§2.4.1) and projects it to the §0.6
/// `CollectedSet` the §1.4 confirm gate renders.
///
/// Homed in `crate::orchestrator` — §0.7's "the §01 pipeline conductor: builds the queue … sequences the
/// guarantees / engines / detection layers": the freeze funnel BUILDS the frozen set that becomes that
/// queue, so it is the conductor's first act, not a new architectural responsibility — the same placement
/// (no §0.7 tree edit) the §7.8.1 `PendingIntake` / `FrontendReady` intake machinery already took here
/// (P2.58 / P2.59). [Derived-Assumption: P2.62 — funnel homed in `crate::orchestrator`, anchored to §0.7's
/// orchestrator-as-§01-conductor role + the P2.58/P2.59 §7.8.1-in-orchestrator precedent; §0.7 is a
/// responsibility map, not an exhaustive per-§ enumeration, so this needs no §0.7 edit]
///
/// **The §2.4.1 freeze spine — built eagerly and once, then immutable for the run:**
/// 1. **Walk / expand** — a dropped/picked folder is enumerated recursively in Rust (the WebView cannot
///    list directories, §0.4); hidden/system files are filtered at freeze time → P2.64 (recursion) /
///    P2.65 (ignore list) / P2.66 (dropped-root retention).
/// 2. **Detect** — each candidate is classified by content (§1.2); an `Unreadable`/`Empty` item is
///    skipped without aborting the walk → P3 (the §1.2 detection framework) / P2.67 (per-item skip) /
///    P2.73 (intake-time `Empty`/`Unreadable` = pre-flight `Skipped`).
/// 3. **Resolve identity + de-dup** — each entry is reduced to its §2.3 resolved identity and
///    de-duplicated (§2.3.2 / §2.4.1) → P2.74 (the pure `FileIdentity` resolved-identity type) / P3 (the IO/FFI
///    `resolve_identity` producer that yields each identity — P3.1.1 surface / P3.6 body) / P2.76 (the pure de-dup
///    fold over those identities, [`dedup_by_identity`]) / P3.7 (the real-FS composition that resolves each
///    candidate then folds — [`resolve_and_dedup`], the spine's step-3 call).
/// 4. **Partition + assign `ItemId` + materialise** — the resolved + de-duped survivors are partitioned by
///    detection verdict (eligible `DroppedItem` / pre-flight `SkippedItem`, P2.73) and each is assigned one
///    `ItemId` over the single id space (eligible + skipped, never re-indexed, §0.6 invariant 6), then the
///    whole set is materialised EAGERLY and ONCE into the immutable [`FrozenSnapshot`] → P2.75 (the id space) /
///    P3.32 (the [`freeze_snapshot`] freeze-point that assembles it).
/// 5. **Group** the frozen snapshot into the §0.6 `CollectedSet` variant (`Single` / `Mixed` /
///    `Unsupported` / `Uncertain` / `Empty`, §1.3) → P3.49 (`group()`).
///
/// The §2.4 idle-vs-in-flight gating is **upstream-delegated, NOT a wrapper around this funnel** (Reading B,
/// P2.72): the in-flight **refuse-busy** is owned by the §7.1.1 PRIMARY `forward_launch_intake` funnel
/// (P2.55 — it DROPS a mid-run launch-intake before any freeze: no `app://intake` emit, no buffer) + the §5.8
/// UI defence-in-depth, so a busy launch-intake never reaches this freeze; the IDLE "freeze a NEW set" is the
/// [`register`](CollectedSetRegistry::register) supersede (P2.44) at the C1/C2a freeze, and "never
/// mutate/merge a frozen one" is structural (§2.4.3 + the register-supersede + this funnel building a fresh
/// snapshot each call). P2.72 ASSERTS that delegation (the orchestrator `freeze_gating_contract` tests); it
/// adds NO core-side freeze gate around `ingest`. The `CollectingId` cooperative-cancel poll the walk runs is P2.69.
///
/// [Build-Session-Entscheidung: P2.62] **Interface-shell body — the SINGLE FUNNEL is the deliverable.**
/// This box establishes the one canonical freeze funnel + its §2.4.1 spine; the stages are the named,
/// scheduled fill-boxes above (the sanctioned compile-time interface-shell pattern, CLAUDE §5 / the P3
/// `crate::isolation` shells P4 expands — NOT a quiet deferral). While the walk + detection + grouping
/// remain unbuilt the spine collects nothing, so the funnel returns the §0.6 zero-collection
/// `CollectedSet::Empty { skipped: [] }` — the genuine result for an input that yields no eligible source
/// (and the same zero-collection result the C1 `drain_intake` drain returns for an empty/raced buffer,
/// §0.4.1 / §5.4). [Test-Change: P2.63 — old-obsolete+new-correct, §1.1] the P2.62 per-fn dead-code lint
/// attribute on `ingest` is removed now the funnel is LIVE — [Build-Session-Entscheidung: P3.78] its
/// production caller is the C1 `drain_intake` drain (`drain_to_collected_set`, which freezes the drained
/// `PendingIntake` buffer here with its stored origin); keeping the attribute would error "unfulfilled
/// expectation" under -D warnings (a production lint change, not a test suppression). C1 `drain_intake` wires
/// it end-to-end at P3.49 (the CSV→TSV walking skeleton); the funnel returns the zero-collection `Empty` for
/// every input until its §2.4.1 spine stages land (P2.64 / P3). (Pre-P3.78 the C2a picker called `ingest`
/// directly; it now funnels through `forward_launch_intake` → `PendingIntake` → the C1 drain, §7.8.1.)
#[must_use]
pub fn ingest(
    paths: Vec<PathBuf>,
    origin: IntakeOrigin,
    cancel: &CancellationToken,
    on_scan: &Channel<ScanProgress>,
    instance: InstanceId,
) -> IngestResult {
    // The §1.1 funnel origin is informational — the §1.3 projection keys off DETECTION, not origin (every
    // origin funnels uniformly since P3.78). [Build-Session-Entscheidung: P3.49]
    let _ = origin;

    // Step 1 (§1.1, P2.64): the recursive walk. A tripped C13 token → discard the partial, not-yet-frozen set
    // (the §0.6 zero-collection `Empty`, §1.1); a fatal walk-root (the dropped root itself gone/unreadable) →
    // surface the root as one Unreadable skip through the freeze (§1.1 "intake-time unreadable = Skipped").
    let IntakeWalk {
        files,
        roots,
        skipped: walk_skips,
    } = match walk_intake_roots(&paths, cancel) {
        Ok(walk) => walk,
        Err(WalkAbort::Cancelled) => return empty_ingest_result(),
        Err(WalkAbort::FatalRoot(fatal)) => return fatal_root_ingest_result(fatal, instance),
    };

    // Step 2 (§1.2, P3.26–P3.29): detect each walked file → a `DetectedCandidate`, polling the ingest cancel
    // token between files (§1.1 cooperative cancel) and emitting the throttled §0.4.2 scan count. The
    // representative §1.4 encoding/delimiter hints are recomputed from the FIRST eligible item's header (they
    // are NOT carried on `DetectionOutcome` — `Recognized` holds only format/confidence/dims, §1.2).
    let mut candidates: Vec<DetectedCandidate> = Vec::with_capacity(files.len());
    let mut hints: Option<SliceHints> = None;
    let mut throttle = ScanThrottle::new();
    let mut scanned: u32 = 0;
    for path in &files {
        if cancel.is_cancelled() {
            return empty_ingest_result();
        }
        scanned = scanned.saturating_add(1);
        throttle.tick(on_scan, scanned);
        let (detected, header, size_bytes) = detect_candidate(path);
        if hints.is_none() && matches!(detected, DetectionOutcome::Recognized { .. }) {
            hints = Some(SliceHints::from_header(&header));
        }
        candidates.push(DetectedCandidate {
            raw_path: path.clone(),
            detected,
            size_bytes,
            rel_path_display: rel_path_for(path, &roots),
        });
    }
    throttle.finish(on_scan, scanned);

    // Step 3 (§2.4.1, P3.7/P3.32): resolve identity + de-dup + freeze into the immutable snapshot.
    let snapshot = match freeze_snapshot(candidates, walk_skips, roots) {
        Ok(snapshot) => snapshot,
        // `ItemSpaceExhausted` is the > u32::MAX-items overflow — astronomically unreachable (§1.10 caps
        // cardinality far lower), handled without a panic (§0.7 in-core no-panic policy): collect nothing.
        Err(ItemSpaceExhausted) => return empty_ingest_result(),
    };

    // Step 4 (§1.3, P3.49): project the frozen snapshot into the wire `CollectedSet` + the registrable set.
    group(snapshot, instance, hints.unwrap_or_default())
}

/// The §0.6 zero-collection ingest result (`CollectedSet::Empty { skipped: [] }`, nothing registrable) — the
/// honest result of a cancelled walk (the partial set is discarded, §1.1) or the unreachable id-space
/// overflow. [Build-Session-Entscheidung: P3.49]
fn empty_ingest_result() -> IngestResult {
    IngestResult {
        collected: CollectedSet::Empty {
            skipped: Vec::new(),
        },
        registrable: None,
    }
}

/// §1.1 fatal-walk-root projection (P3.49): the dropped root itself was gone/unreadable, so the walk sank the
/// ingest before any candidate was collected (`walk_intake_roots` abandons any earlier roots). Per §1.1
/// "intake-time unreadable = Skipped", the root is frozen as a single Unreadable skip → the §1.3
/// `Empty { skipped: [root] }` view, so the reason is surfaced rather than dropped (SSOT *Fail clearly*). The
/// gone-vs-denied `ReadFailure` nuance collapses to `SkipReason::Unreadable` (the `SkippedItem` model carries
/// no `ReadFailure` slot — the same collapse the per-item `record_unreadable` makes).
/// [Build-Session-Entscheidung: P3.49]
fn fatal_root_ingest_result(fatal: FatalWalkRoot, instance: InstanceId) -> IngestResult {
    let FatalWalkRoot { root, cause: _ } = fatal;
    let root_skip = WalkSkip {
        path: root.clone(),
        reason: SkipReason::Unreadable,
    };
    match freeze_snapshot(Vec::new(), vec![root_skip], vec![root]) {
        Ok(snapshot) => group(snapshot, instance, SliceHints::default()),
        Err(ItemSpaceExhausted) => empty_ingest_result(),
    }
}

/// Detect one §1.1 walked file (§1.2): stat it (recording `size_bytes` at the §2.4 freeze), read the bounded
/// §1.2 header window, and classify. A pre-open regular-file check — a FIFO/pipe/device that raced the walk's
/// own `is_file` check would BLOCK `File::open` (an in-core hang/DoS, §2.12.4) — and any stat/open/read failure
/// map to `DetectionOutcome::Unreadable` rather than propagating (§1.1: "a bad file never sinks the ingest").
/// Returns `(outcome, header, size_bytes)`; the header is returned for the caller's representative-hint
/// recompute and is empty on a read failure. This is the only untrusted-byte touch, bounded + memory-safe,
/// kept in-core (§1.2 / §2.12.4). **Residual TOCTOU (owner-accepted, the P3.9 precedent):** a NARROW window
/// remains if the file is swapped regular→FIFO in the µs between THIS `metadata()` pre-check and THIS
/// `File::open()` — the identical residual `fs_guard::open_verified_parent_dir`'s stat-precheck names and
/// accepts; the pre-check closes the common case (the walk's already-stale verdict), the residual is inherent
/// to any stat-then-open and is not a full closure. [Build-Session-Entscheidung: P3.49]
fn detect_candidate(path: &Path) -> (DetectionOutcome, Vec<u8>, u64) {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            return (
                DetectionOutcome::Unreadable {
                    reason: classify_read_failure(&error),
                },
                Vec::new(),
                0,
            );
        }
    };
    // A non-regular file (a FIFO/pipe/device that replaced the file after the walk's `is_file` check) is NEVER
    // opened — a blocking FIFO open is an in-core hang (§2.12.4 no-hang). It surfaces as an Unreadable skip.
    if !metadata.is_file() {
        return (
            DetectionOutcome::Unreadable {
                reason: ReadFailure::IoError,
            },
            Vec::new(),
            metadata.len(),
        );
    }
    let size_bytes = metadata.len();
    let header = match std::fs::File::open(path).and_then(crate::detection::read_header) {
        Ok(header) => header,
        Err(error) => {
            return (
                DetectionOutcome::Unreadable {
                    reason: classify_read_failure(&error),
                },
                Vec::new(),
                size_bytes,
            );
        }
    };
    (crate::detection::detect(&header), header, size_bytes)
}

/// Map a `std::io::Error` from stat/open/read to the §0.6 [`ReadFailure`] taxonomy `DetectionOutcome::Unreadable`
/// carries — the same gone-vs-denied mapping [`classify_walk_root_failure`] makes for `walkdir::Error`: a
/// missing file → `NotFound` ("gone"), a denied read → `PermissionDenied`, anything else → `IoError`. `Locked`
/// is NOT produced at intake (a sharing violation has no portable `io::ErrorKind`, and `Locked` is a
/// conversion-time §2.8 outcome), so intake maps only the gone-vs-denied distinction. An if-chain (not a
/// `match`) because `io::ErrorKind` is `#[non_exhaustive]` — a `_`-wildcard `match` trips the crate-root
/// `clippy::wildcard_enum_match_arm` deny (the engines-module convention). [Build-Session-Entscheidung: P3.49]
fn classify_read_failure(error: &std::io::Error) -> ReadFailure {
    let kind = error.kind();
    if kind == std::io::ErrorKind::NotFound {
        ReadFailure::NotFound
    } else if kind == std::io::ErrorKind::PermissionDenied {
        ReadFailure::PermissionDenied
    } else {
        ReadFailure::IoError
    }
}

/// The §2.7 root-relative subpath preview for a walked file (`DroppedItem.rel_path_display`, §1.4): the
/// DEEPEST dropped root that is an ancestor of `file`, stripped. `None` for a top-level dropped FILE (its root
/// IS the file — no subtree) and for the unreached case of a file under no dropped root. A lossy §2.10.1
/// display string, never re-submitted as input. [Build-Session-Entscheidung: P3.49]
fn rel_path_for(file: &Path, roots: &[PathBuf]) -> Option<String> {
    let root = roots
        .iter()
        .filter(|root| file.starts_with(root))
        .max_by_key(|root| root.components().count())?;
    let rel = file.strip_prefix(root).ok()?;
    if rel.as_os_str().is_empty() {
        None
    } else {
        Some(rel.to_string_lossy().into_owned())
    }
}

/// The §0.4.2 `ScanProgress` telemetry throttle (P3.49): a best-effort ~2/s coalescing emitter for the §5.2
/// *Collecting* "Scanning… N files" count during the §1.1 walk. Emits on the first tick, then at most once per
/// [`SCAN_EMIT_INTERVAL`], plus a final emit of the true total. A dead listener (`Channel::send` `Err`) is
/// non-fatal — the walk never depends on the frontend receiving telemetry (best-effort, monotonic, dies with
/// the C1 call, §0.4.2). [Build-Session-Entscheidung: P3.49]
struct ScanThrottle {
    last_emit: Option<Instant>,
}

/// The ≈2/s coalescing interval for [`ScanThrottle`] (§0.4.2 "≈2/s").
const SCAN_EMIT_INTERVAL: Duration = Duration::from_millis(500);

impl ScanThrottle {
    fn new() -> Self {
        Self { last_emit: None }
    }

    /// Emit the running `scanned` count if the throttle interval has elapsed (or on the first tick).
    fn tick(&mut self, on_scan: &Channel<ScanProgress>, scanned: u32) {
        let now = Instant::now();
        let due = match self.last_emit {
            None => true,
            Some(last) => now.duration_since(last) >= SCAN_EMIT_INTERVAL,
        };
        if due {
            // Best-effort: a dead listener (`Err`) is ignored — telemetry never blocks the walk.
            let _ = on_scan.send(ScanProgress { scanned });
            self.last_emit = Some(now);
        }
    }

    /// Emit the final total once the walk completes (always — so the last "Scanning… N" reflects the true count).
    fn finish(&self, on_scan: &Channel<ScanProgress>, scanned: u32) {
        let _ = on_scan.send(ScanProgress { scanned });
    }
}

/// The §1.4 detection-derived encoding/delimiter hints for a `CollectedSet::Single`, recomputed from a
/// REPRESENTATIVE (the first eligible) item's header — they are NOT carried on `DetectionOutcome` (§1.2). Both
/// `None` for a UTF-8 comma-CSV (no hint to surface) or for a non-`Single` collection. [Build-Session-Entscheidung: P3.49]
#[derive(Debug, Default, PartialEq, Eq)]
struct SliceHints {
    encoding: Option<String>,
    delimiter: Option<String>,
}

impl SliceHints {
    /// Recompute the §1.4 hints from a header via the §1.2 classifiers. `ext_hint: None` reproduces the
    /// extension-free classification `detect` already used, so the delimiter hint agrees with the recognized
    /// format (a real extension could flip a genuine comma/tab tie). A non-text header yields no hints (unreached
    /// for a `Recognized` CSV/TSV, which reached `Recognized` only because `classify_encoding` returned `Some`).
    fn from_header(header: &[u8]) -> Self {
        let Some(encoding) = crate::detection::classify_encoding(header) else {
            return Self::default();
        };
        let delimiter_class = crate::detection::classify_delimiter(header, encoding, None);
        Self {
            encoding: crate::detection::encoding_hint(encoding),
            delimiter: crate::detection::delimiter_hint(delimiter_class),
        }
    }
}

/// §1.3 batch grouping (P3.49): project the frozen snapshot into the §0.6 wire `CollectedSet` + the
/// registrable `RegisteredSet`. Exactly one eligible source format → `Single` (the ONLY registrable outcome,
/// §0.4.4); two or more distinct eligible formats → the §1.3 hard pre-flight refusal `Mixed { found }` (no
/// partial conversion); zero eligible → the §1.3 Empty-projection over the skips ([`empty_projection`]). The
/// `CollectedSet::Single` fields the snapshot does not carry are computed here: a fresh `CollectedSetId`, the
/// `instance`, the single `format`, `count`/`total_bytes`, the lossy `roots_display`, and the §1.4
/// `encoding_hint`/`delimiter_hint` (`hints`); `notes` is empty (CSV/TSV has no §1.4 structural-peek producer —
/// that arrives with P5–P7). [Build-Session-Entscheidung: P3.49]
fn group(snapshot: FrozenSnapshot, instance: InstanceId, hints: SliceHints) -> IngestResult {
    let FrozenSnapshot {
        items,
        skipped,
        item_paths,
        identities,
        roots,
    } = snapshot;

    // Distinct eligible source formats in first-seen order, with per-format counts (§1.3 grouping key). Every
    // `items` member is `Recognized` by the freeze partition; the `if let` keeps the fold total without a panic.
    let mut found: Vec<(UserFacingFormat, usize)> = Vec::new();
    for item in &items {
        if let DetectionOutcome::Recognized { format, .. } = item.detected {
            match found.iter_mut().find(|(seen, _)| *seen == format) {
                Some(entry) => entry.1 = entry.1.saturating_add(1),
                None => found.push((format, 1)),
            }
        }
    }

    match found.len() {
        // Zero eligible source → the §1.3 Empty-projection (lone Unsupported/Uncertain specificity, else Empty).
        0 => IngestResult {
            collected: empty_projection(skipped),
            registrable: None,
        },
        // Exactly one eligible format → `Single` (the registrable collection).
        1 => {
            let Some(&(format, _)) = found.first() else {
                // Unreachable (len checked == 1); the §0.7 no-panic totality guard, never an unwrap.
                return IngestResult {
                    collected: CollectedSet::Empty { skipped },
                    registrable: None,
                };
            };
            let count = items.len();
            let total_bytes = items.iter().map(|item| item.size_bytes).sum();
            let roots_display = roots
                .iter()
                .map(|root| root.to_string_lossy().into_owned())
                .collect();
            let single = CollectedSet::Single {
                id: CollectedSetId::mint(),
                instance,
                format,
                items,
                count,
                skipped,
                total_bytes,
                roots_display,
                encoding_hint: hints.encoding,
                delimiter_hint: hints.delimiter,
                notes: Vec::new(),
            };
            // `from_collected` returns `Some` only for `Single`, so this is `Some` by construction; `.map`
            // avoids a dead no-panic branch. The real off-wire `roots` + `item_paths` are moved into the frozen
            // set (the wire `roots_display` above is the lossy display; §2.10.1).
            let registrable = FrozenCollectedSet::from_collected(&single, roots, item_paths)
                .map(move |frozen| RegisteredSet { frozen, identities });
            IngestResult {
                collected: single,
                registrable,
            }
        }
        // Two or more distinct eligible formats → the §1.3 hard pre-flight refusal.
        _ => IngestResult {
            collected: CollectedSet::Mixed { found },
            registrable: None,
        },
    }
}

/// The §1.3 all-ineligible projection: a LONE `UnsupportedType` skip → `Unsupported { detected }` (SSOT
/// principle 6 "detected: X"); a LONE `Uncertain` skip → `Uncertain { note }` (the §1.2 best-guess text, or
/// empty when detection could not even guess); anything else (zero skips, or 2+ ineligibles of mixed kinds) →
/// the generic `Empty { skipped }` carrying every per-item reason (§1.3 "the reasons are no longer lost when
/// 2+ ineligible items collapse to Empty"). [Build-Session-Entscheidung: P3.49]
fn empty_projection(skipped: Vec<SkippedItem>) -> CollectedSet {
    if let [only] = skipped.as_slice() {
        match only.reason {
            SkipReason::UnsupportedType => {
                return CollectedSet::Unsupported {
                    detected: only.detected_display.clone().unwrap_or_default(),
                };
            }
            SkipReason::Uncertain => {
                return CollectedSet::Uncertain {
                    note: only.detected_display.clone().unwrap_or_default(),
                };
            }
            SkipReason::Empty | SkipReason::Unreadable | SkipReason::AlreadyConverted => {}
        }
    }
    CollectedSet::Empty { skipped }
}

/// The §1.1 `ingest` funnel result (P3.49): the wire `CollectedSet` the C1 `drain_intake` drain returns (the
/// §1.3 [`group`] projection of the frozen snapshot) PLUS the `registrable` `RegisteredSet` the C1 handler
/// registers into the §0.4.4 `CollectedSetRegistry` for a `Single` collection (`None` for every non-`Single`
/// outcome — only a `Single` set is offered targets / planned / run, so only it is registered). The split
/// keeps `ingest` a pure `crate::orchestrator` funnel (no `AppHandle`/State) while the AppHandle-coupled C1
/// handler owns the managed-registry mutation, performed LAST — after the whole fallible funnel resolves — so
/// a mid-funnel early-return can never leave a half-registered set (the §1.1a boot-glue split; the
/// mutate-registries-last discipline). INTERNAL: it wraps the tier-1 `RegisteredSet`, so it derives no
/// `serde`/`specta`. [Build-Session-Entscheidung: P3.49]
#[derive(Debug, PartialEq, Eq)]
pub struct IngestResult {
    /// The §0.6 wire union the C1 drain returns (the §1.3 `group()` projection of the frozen snapshot).
    pub collected: CollectedSet,
    /// `Some` for a `Single` collection (the registrable frozen set the C3/C4/C6 commands resolve by
    /// `CollectedSetId`); `None` for `Mixed`/`Unsupported`/`Uncertain`/`Empty` (nothing to register).
    pub registrable: Option<RegisteredSet>,
}

/// The representative "will save to …" directory for the §1.8 / C4 batch preview (P3.49). For `ChosenRoot`
/// the chosen root is shown as-is (the per-item subtree is re-created at C6 write time, not here — a preview
/// writes nothing). For `BesideSource` the representative is the PARENT of the first eligible source file
/// (§2.7.1 — output lands beside its source), NOT `source_common_root` (which for a lone dropped FILE root is
/// the file itself, not a directory — probing it as a dir would falsely divert). Falls back to the §2.7.1
/// source common root only if the first item has no retained resolved path (a mis-built set — never for a real
/// freeze). No filesystem side effect. [Build-Session-Entscheidung: P3.49]
fn preview_final_dir(frozen: &FrozenCollectedSet, destination: &ResolvedDestination) -> PathBuf {
    match destination {
        ResolvedDestination::ChosenRoot(root) => root.clone(),
        ResolvedDestination::BesideSource => frozen
            .items
            .first()
            .and_then(|item| frozen.item_paths.get(&item.item))
            .and_then(|paths| paths.resolved_path.parent())
            .map_or_else(|| source_common_root(&frozen.roots), Path::to_path_buf),
    }
}

/// §1.8 / C4 `plan_output` (P3.49): compute the batch-level [`OutputPlanPreview`] for a registered set — the
/// "will save to …" directory, its §2.7.2 divert classification, the §2.5 re-run verdict, and the §1.10
/// preflight verdict — the plan/preview the §5.2 Targets/Destination screens render BEFORE convert (eager on
/// the `3→4` transition, debounced re-callable on target/option/destination change; §0.4.1 C4). Reads only —
/// no ledger record, no registry eviction — so it is safe to re-call.
///
/// - `final_dir`/`diverted`: the representative destination directory ([`preview_final_dir`]: the beside-source
///   PARENT of the first eligible source for `BesideSource`, the chosen root for `ChosenRoot`), classified once
///   via [`location_status`] (§2.7.2 — the eager planning HINT; the §2.1 at-write re-check is the authority).
///   `final_dir_display` is the lossy §2.10.1 display — no `PathBuf` crosses the wire.
/// - `rerun`: [`compute_rerun_verdict`] over the set's §2.3 identities (PEEK-only, §2.5).
/// - `preflight`: the §1.10-seam walking-skeleton trivial verdict — the CSV→TSV footprint is negligible, so
///   `up_front_fail: None` by construction and `est_total_output_bytes` is the frozen size; the real §1.10
///   estimator is P4.72, which SUPERSEDES this behind the same C4 contract (the §1.10-seam slice-verdict note),
///   so P3 must NOT build a real estimator here (a double-build). [Build-Session-Entscheidung: P3.49]
#[must_use]
pub fn plan_output_preview(
    set: &RegisteredSet,
    target: TargetId,
    options: &OptionValues,
    destination: &ResolvedDestination,
    instance: InstanceId,
    computer: &EquivKeyComputer,
    ledger: &RerunLedger,
) -> OutputPlanPreview {
    let final_dir = preview_final_dir(&set.frozen, destination);
    let diverted = match location_status(&final_dir, &PublishTemp::probe_name(instance)) {
        LocationStatus::Writable => None,
        LocationStatus::Divert(reason) => Some(reason),
    };
    OutputPlanPreview {
        set: set.frozen.id,
        final_dir_display: final_dir.to_string_lossy().into_owned(),
        diverted,
        rerun: compute_rerun_verdict(set, target, options, computer, ledger),
        preflight: PreflightVerdict {
            est_total_output_bytes: set.frozen.total_bytes,
            est_total_scratch_bytes: 0,
            up_front_fail: None,
        },
    }
}

/// The §2.4.1 freeze-spine step-1 intake-walk result (P2.66): the flat candidate file list
/// ([`walk_intake_roots`], P2.64) PLUS the **dropped root(s) retained VERBATIM** for §2.7
/// (relative-subtree re-creation + the "open folder" common-root anchor). §2.7 owns the common-root /
/// relative-subtree COMPUTATION; this is plain §1.1 retention — the roots are carried through the walk so
/// the P3.49 ingest funnel can freeze them onto `CollectedSet::Single.roots` (§0.6).
/// [Build-Session-Entscheidung: P2.66]
// [Test-Change: P3.49 — old-obsolete+new-correct, §2.4.1] the P2.66 dead-code lint attribute is removed — the
// `ingest` funnel spine (this item's production reader) is now LIVE (P3.49), so the dead-code lint is obsolete
// (a production lint removal, not a test suppression).
struct IntakeWalk {
    /// The flat candidate file paths (§1.1), depth-first deterministic order ([`walk_intake_roots`]).
    files: Vec<PathBuf>,
    /// The dropped root(s) retained verbatim (§1.1/§2.7) — the §2.7 subtree / "open folder" anchor.
    roots: Vec<PathBuf>,
    /// Per-item walk-level READ failures recorded mid-walk (P2.67) — an `Unreadable` item the walk could not
    /// classify (a vanished/denied discovered entry, an unreadable subdir, a dangling/denied symlink). KEPT,
    /// not silently dropped, so the §1.4 summary accounts for it (§1.1); the walk CONTINUES past each. The
    /// §1.2 detection skips (`Empty`/`UnsupportedType`/`Uncertain`) join here when detection lands (P3); the
    /// §0.6 `ItemId` is assigned at the freeze (P2.75) — turning each [`WalkSkip`] into a §0.6 `SkippedItem`.
    skipped: Vec<WalkSkip>,
}

/// A raw per-item skip recorded DURING the §1.1 walk (P2.67) — PRE-`ItemId` (the freeze assigns the §0.6
/// `ItemId` over the single id space at P2.75, turning this into a §0.6 `SkippedItem`). P2.67 records only the
/// walk-level `Unreadable` read failures (a vanished/denied entry, an unreadable subdir, a dangling symlink);
/// the §1.2 detection skips (`Empty`/`UnsupportedType`/`Uncertain`) join the [`IntakeWalk::skipped`] list at P3.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WalkSkip {
    /// The path of the unreadable item (for the §1.4 summary display).
    path: PathBuf,
    /// The §0.6 skip cause — `Unreadable` for every walk-level read failure P2.67 records.
    reason: SkipReason,
}

/// A FATAL §1.1 walk-root error (P2.68) — the dropped/picked ROOT itself could not be read (a DEPTH-0
/// `walkdir` error: the root is gone, or its directory cannot be listed), as opposed to a per-item read
/// failure DISCOVERED *inside* a root (a depth > 0 error → an `Unreadable` [`WalkSkip`], P2.67, the walk
/// CONTINUES). Per §1.1 the walk is STOPPED **only** by a C13 cancel (P2.69) or this fatal walk-root error:
/// "a single bad file inside a thousand-file folder never sinks the whole ingest", but a bad ROOT does — so
/// [`walk_intake_roots`] yields this in the `Err(`[`WalkAbort`]`::FatalRoot)` arm, never a skipped row. The
/// P3.49 freeze spine maps it to the §1.1 fatal-ingest surface; `cause` reuses the §0.6 [`ReadFailure`]
/// taxonomy so that surfacing distinguishes "gone" (`NotFound`) from "unreadable"
/// (`PermissionDenied`/`IoError`). It needs no dead-code suppression attribute: its derived impls make it
/// "used" in the non-test build (the same reason its sibling [`WalkSkip`] needs none), so the only pending
/// wiring is its production caller — the P3.49 ingest funnel that maps it to the fatal surface.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FatalWalkRoot {
    /// The dropped/picked root that could not be read (its lossy display feeds the §1.1 fatal-ingest skip).
    root: PathBuf,
    /// Why the root could not be read — the §0.6 [`ReadFailure`] taxonomy (gone vs denied/io). Retained as a
    /// diagnostic record (`derive(Debug)`); the P3.49 fatal-ingest surfacing does NOT distinguish it —
    /// `fatal_root_ingest_result` collapses every fatal root to `SkipReason::Unreadable` through the freeze,
    /// because the §0.6 `SkippedItem` model carries no `ReadFailure` slot (the same collapse the per-item
    /// `record_unreadable` makes for a mid-walk read failure).
    cause: ReadFailure,
}

/// Why the §1.1 intake walk did NOT produce a freezable set — the `Err` of [`walk_intake_roots`]
/// (P2.68/P2.69). Both arms mean "stop; there is nothing to freeze", so the P3.49 freeze spine never freezes a
/// partial set; it maps each arm to its §1.1 surface: a `FatalRoot` to the fatal-ingest message, a `Cancelled`
/// to the silent return-to-Idle / `CollectedSet::Empty`. A normal walk is the `Ok([`IntakeWalk`])` arm. It
/// needs no dead-code suppression attribute: its derived impls make it "used" in the non-test build (the
/// [`FatalWalkRoot`] / [`WalkSkip`] precedent); the only pending wiring is its production reader — the P3.49
/// ingest funnel.
#[derive(Debug, Clone, PartialEq, Eq)]
enum WalkAbort {
    /// The ingest was cooperatively CANCELLED mid-walk — C13 `cancel_ingest` (§0.4.1) tripped the
    /// `CollectingId` token the walk polls (§1.1, P2.69). The partial, not-yet-frozen set is discarded; there
    /// is NO cleanup obligation (nothing is written during the walk).
    Cancelled,
    /// The dropped/picked ROOT itself was unreadable/gone — a fatal walk-root error (§1.1, P2.68), distinct
    /// from the P2.67 per-item `Unreadable` skip (which continues the walk).
    FatalRoot(FatalWalkRoot),
}

/// **Step 1 of the §2.4.1 freeze spine (P2.64)** — expand the dropped/picked intake roots into a flat,
/// **depth-first** list of candidate file paths (the input the §1.2 detection stage classifies). The WebView
/// cannot enumerate a directory (§0.4), so a dropped/picked FOLDER is walked recursively in Rust; a dropped
/// FILE is yielded directly. The [`ingest`] funnel's spine consumes this at P3.49 (the CSV→TSV walking
/// skeleton); P2.64 authors it as the named step-1 primitive — the sanctioned compile-time interface-shell
/// (CLAUDE §5): a complete, tested unit whose only pending step is its spine wiring.
///
/// **§1.1 walk rules owned here (recursion P2.64 + the hidden/system filter P2.65):**
/// - **Depth-first** traversal (`walkdir`, §0.8) — the §1.9 queue / §2.7 open-folder order reads it.
/// - **Symlinked directories are NOT traversed** (`follow_links(false)`) — loop-safety against a symlink
///   cycle (the existing T7 link-redirection class); a discovered symlinked DIR is neither descended NOR a
///   candidate. The resolved-identity de-dup that handles file-level link aliasing is §2.3 (P2.74/P2.76),
///   NOT here, so a symlinked FILE IS yielded as a §2.3-resolvable candidate (the T7 input-side-symlink case).
/// - **Deterministic order** (`sort_by_file_name`) — a stable, OS-readdir-independent order so the §1.9
///   "deterministic collected/traversal order" + §2.5 re-run equivalence hold across platforms (test-strategy
///   §7 determinism). [Build-Session-Entscheidung: P2.64 — sort the walk; walkdir's native order is
///   filesystem-dependent, which would make the §1.9 queue order non-reproducible.]
/// - **Hidden/system files skipped** (`filter_entry`, P2.65) — a DISCOVERED entry that is a dotfile (name
///   begins `.`, all platforms), one of the fixed sentinels (`.DS_Store`/`Thumbs.db`/`desktop.ini`), or
///   carries a Windows hidden/system file-attribute is pruned: a hidden DIRECTORY is not descended AND a
///   hidden FILE is skipped (§1.1, SSOT *How It Feels* 2). The dropped ROOT (`depth() == 0`, a hidden file OR
///   folder dropped directly) is EXEMPT — the user chose it explicitly, and the §1.1 ignore-list governs
///   RECURSION (a directly-dropped hidden folder is still expanded; only its DISCOVERED entries are filtered).
///   See `name_is_hidden_or_sentinel` + `windows_attr_hidden`.
///
/// **Owned here additionally (P2.66):** the dropped root(s) are RETAINED verbatim on the returned
/// [`IntakeWalk`] for §2.7 (relative-subtree re-creation + the "open folder" common-root anchor — §2.7 owns
/// that computation; this is plain retention).
///
/// **Per-item read failures RECORDED, never silently dropped (P2.67):** a `walkdir` traversal error at
/// **depth > 0** (an unreadable discovered subdir/entry — "a denied read, a file that vanished") or a
/// dangling/denied symlink (its target stat fails) is recorded as an `Unreadable` [`WalkSkip`] on
/// [`IntakeWalk::skipped`] and the walk **CONTINUES** (§1.1: a per-item failure never aborts the ingest — one
/// bad entry never sinks a thousand-file folder). The §1.2 detection-time skips
/// (`Empty`/`UnsupportedType`/`Uncertain`) and the §0.6 `ItemId` join at P3 / the freeze (P2.75) — P2.67 owns
/// the walk-level `Unreadable` half only. **A 0-byte file is NOT skipped here:** `Empty` is a §1.2 detection
/// outcome (detection reads the bytes, a 0-byte file yields none → `Empty`, the §1.1 "Zero-byte at intake"
/// subsection / P2.73), so 0-byte files stay candidates at the walk and are classified at detect (P3).
/// [Build-Session-Entscheidung: P2.67 — record only the walk-level `Unreadable` read failures; `Empty` and
/// content-ineligibility defer to §1.2 detection (P3) and the `ItemId` to the freeze (P2.75).]
///
/// **Two walk-stopping outcomes OWNED here**, both returned in the `Err` arm ([`WalkAbort`]) so the P3.49
/// freeze spine never freezes a partial set (§1.1: "the walk is stopped only by a C13 cancel or a fatal
/// walk-root error … a single bad file never sinks the whole ingest"):
/// - **Fatal walk-root stop (P2.68):** a DEPTH-0 `walkdir` error — the dropped/picked ROOT itself unreadable
///   or gone — STOPS the walk (`Err(WalkAbort::FatalRoot(`[`FatalWalkRoot`]`))`), distinct from the P2.67
///   per-item `Unreadable` skip that CONTINUES. A bad ROOT does sink the ingest. Across multiple dropped
///   roots the FIRST fatal root (input order) stops the whole walk; its already-collected candidates are
///   discarded. [Derived-Assumption: P2.68 — multi-root: the first fatal root stops the WHOLE walk, from §1.1
///   "the walk is stopped … a single bad file never sinks the ingest" (a root is not a per-item skip).]
/// - **Cooperative cancellation (P2.69):** the loop polls the ingest-scoped `cancel` token each entry; when
///   C13 `cancel_ingest` trips it via the `CollectingId` (§1.1/§0.4.1, registered by the `IngestRegistry` at
///   handler entry), the walk STOPS and returns `Err(WalkAbort::Cancelled)`, discarding the partial,
///   not-yet-frozen set — NO cleanup obligation (nothing is written during the walk). The §1.2 detection-loop
///   poll joins at P3 (detection is unbuilt); P2.69 owns the walk-loop poll.
///
/// [Build-Session-Entscheidung: P2.64 — a symlink's TARGET type is classified by one link-following
/// `std::fs::metadata` stat (a type check, NOT a content read and NOT §2.3 identity resolution): a
/// symlinked-file is kept, a symlinked-dir excluded, a dangling/denied target skipped — anchored to §1.1
/// ("symlinked dirs not traversed … file-level aliasing handled by §2.3") + the T7 corpus expectation. A
/// dropped symlinked-dir ROOT is followed (walkdir's `follow_root_links` default) — the user chose it
/// explicitly; the no-traversal rule defends against cycles DISCOVERED mid-walk.]
// [Test-Change: P3.49 — old-obsolete+new-correct, §2.4.1] the P2.64 dead-code lint attribute is removed — the
// `ingest` funnel spine (this walk's production caller) is now LIVE (P3.49), so the dead-code lint is obsolete
// (a production lint removal, not a test suppression).
fn walk_intake_roots(
    roots: &[PathBuf],
    cancel: &CancellationToken,
) -> Result<IntakeWalk, WalkAbort> {
    let mut candidates = Vec::new();
    let mut skipped = Vec::new();
    for root in roots {
        // `follow_links(false)`: a symlinked subdirectory is never descended (loop-safety). `sort_by_file_name`:
        // a deterministic per-directory order (§1.9/§2.5). `filter_entry` (P2.65): a §1.1 hidden/system
        // DISCOVERED entry is pruned — a hidden DIR is not descended AND a hidden FILE is skipped — but the
        // dropped ROOT (`depth() == 0`) is EXEMPT (the user chose it explicitly; the ignore-list governs
        // RECURSION, §1.1). A dropped FILE root walks as a single entry.
        let walk = WalkDir::new(root)
            .follow_links(false)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|entry| entry.depth() == 0 || !entry_is_hidden_or_system(entry));
        for entry in walk {
            // P2.69: poll the ingest-scoped cancellation token each entry — C13 `cancel_ingest` (§0.4.1) trips
            // it via the `CollectingId`, and the §1.1 walk stops COOPERATIVELY, discarding the partial,
            // not-yet-frozen set (the in-progress `candidates`/`skipped` are dropped on return) — there is NO
            // cleanup obligation, nothing is written during the walk. The §1.2 detection-loop poll joins at P3.
            if cancel.is_cancelled() {
                return Err(WalkAbort::Cancelled);
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    // P2.68: a DEPTH-0 error is the dropped ROOT itself unreadable/gone — a FATAL walk-root
                    // error that STOPS the walk (§1.1), distinct from the P2.67 per-item skip that CONTINUES.
                    // It abandons any candidates collected from earlier roots (the multi-root short-circuit) —
                    // a bad root sinks the ingest, a bad file never does. `cause` records the gone-vs-denied
                    // §0.6 `ReadFailure` (a `Debug` diagnostic); the P3.49 fatal-ingest surfacing collapses it to
                    // one generic `SkipReason::Unreadable` skip (the `SkippedItem` model has no `ReadFailure` slot).
                    if err.depth() == 0 {
                        return Err(WalkAbort::FatalRoot(FatalWalkRoot {
                            root: root.clone(),
                            cause: classify_walk_root_failure(&err),
                        }));
                    }
                    // P2.67: a depth > 0 error is a DISCOVERED entry/subdir that could not be read — record it
                    // `Unreadable` + CONTINUE (one bad entry never sinks a thousand-file folder).
                    record_unreadable(&mut skipped, err.path());
                    continue;
                }
            };
            let file_type = entry.file_type();
            let is_candidate = if file_type.is_symlink() {
                // Not descended (above). A target FILE is a §2.3-resolvable candidate (T7); a target DIR is
                // excluded; a target whose stat FAILS (dangling/denied — "a file that vanished") is an
                // `Unreadable` skip (P2.67), recorded + the walk continues. The stat is a type check, not a read.
                match std::fs::metadata(entry.path()) {
                    Ok(target) => target.is_file(),
                    Err(_) => {
                        record_unreadable(&mut skipped, Some(entry.path()));
                        false
                    }
                }
            } else {
                // A real directory is descended by `walkdir` (not itself a candidate); a real file is a candidate.
                file_type.is_file()
            };
            if is_candidate {
                candidates.push(entry.into_path());
            }
        }
    }
    // §1.1 (P2.66): retain the dropped root(s) VERBATIM alongside the flat candidate list — the §2.7
    // subtree / "open folder" anchor (§2.7 computes the common root; this is plain §1.1 retention). Every
    // dropped root is kept, in input order, regardless of how many candidates it yielded (an empty folder
    // still anchors "open folder"). `skipped` carries the §1.1 per-item Unreadable read failures (P2.67).
    // Reaching here means no root was fatally unreadable/gone (P2.68 returns early on a DEPTH-0 error) and no
    // cancel was observed (P2.69 returns early on a tripped token) — the walk completed into a freezable set.
    Ok(IntakeWalk {
        files: candidates,
        roots: roots.to_vec(),
        skipped,
    })
}

/// Classify a DEPTH-0 [`walk_intake_roots`] `walkdir` error (the dropped/picked root itself unreadable/gone,
/// P2.68) into the §0.6 [`ReadFailure`] taxonomy: a missing root → `NotFound` ("gone"), a denied read →
/// `PermissionDenied`, anything else → `IoError`. The §0.6 `Locked` variant is **not** produced here — a
/// directory-root sharing violation has no portable `io::ErrorKind`, and `Locked` is a conversion-time (§2.8)
/// outcome, not an intake one; so the walk-root classifier maps only the gone-vs-denied distinction onto the
/// `FatalWalkRoot.cause` diagnostic record. (The P3.49 intake surfacing does not distinguish it — it collapses
/// every fatal root to a generic `SkipReason::Unreadable` skip, the `SkippedItem` model carrying no slot for it.)
/// [Build-Session-Entscheidung: P2.68 — map `io::ErrorKind` {NotFound, PermissionDenied} to the matching
/// `ReadFailure`, everything else to `IoError`; `Locked` is conversion-time only (§2.8).]
fn classify_walk_root_failure(err: &walkdir::Error) -> ReadFailure {
    match err.io_error().map(std::io::Error::kind) {
        Some(std::io::ErrorKind::NotFound) => ReadFailure::NotFound,
        Some(std::io::ErrorKind::PermissionDenied) => ReadFailure::PermissionDenied,
        _ => ReadFailure::IoError,
    }
}

/// Record a §1.1 walk-level `Unreadable` skip (P2.67) for `path` if one is present — the shared recorder both
/// the walkdir-traversal-error arm and the symlink-stat-failure arm of [`walk_intake_roots`] call, so a
/// per-item read failure is KEPT (not silently dropped) and the §1.4 summary accounts for it. A `None` path
/// (a `walkdir` error with no associated path) has no item to attribute and is dropped.
fn record_unreadable(skipped: &mut Vec<WalkSkip>, path: Option<&std::path::Path>) {
    if let Some(path) = path {
        skipped.push(WalkSkip {
            path: path.to_path_buf(),
            reason: SkipReason::Unreadable,
        });
    }
}

/// The §1.1 fixed hidden/system-file sentinels (P2.65) — the NON-dotfile platform junk a folder walk skips
/// by NAME. Dotfiles (any name beginning `.`, all platforms) are matched by rule, not by this list;
/// `.DS_Store` is itself a dotfile but kept here for spec fidelity (§1.1 enumerates it). A FIXED constant —
/// not user-config in v1 (§1.1 `[REC]`).
const IGNORED_SENTINEL_NAMES: &[&str] = &[".DS_Store", "Thumbs.db", "desktop.ini"];

/// Whether a walked entry's NAME is hidden/system per §1.1 (P2.65): a **dotfile** (its name begins `.`, all
/// platforms) or one of the fixed [`IGNORED_SENTINEL_NAMES`] (case-insensitive — Windows filesystems
/// case-fold). The leading-`.` test reads the OS-encoded first byte (`.` is ASCII, a single byte in
/// UTF-8/WTF-8) so a non-UTF-8 name classifies without a lossy round-trip; the sentinel match lossily folds
/// case (the sentinels are pure ASCII, so a real match is exact and a non-UTF-8 name simply never equals one).
fn name_is_hidden_or_sentinel(name: &std::ffi::OsStr) -> bool {
    if name.as_encoded_bytes().first() == Some(&b'.') {
        return true;
    }
    let lossy = name.to_string_lossy();
    IGNORED_SENTINEL_NAMES
        .iter()
        .any(|sentinel| lossy.eq_ignore_ascii_case(sentinel))
}

/// Whether a DISCOVERED walked entry is hidden/system per §1.1 (P2.65) — by NAME (cross-platform) OR, on
/// Windows, by the hidden/system file-ATTRIBUTE. The dropped ROOT is exempted by the caller (the user chose
/// it explicitly); this classifies discovered entries so a hidden directory is pruned (not descended) and a
/// hidden file is skipped.
fn entry_is_hidden_or_system(entry: &walkdir::DirEntry) -> bool {
    name_is_hidden_or_sentinel(entry.file_name()) || windows_attr_hidden(entry)
}

/// The Windows hidden/system file-ATTRIBUTE leg of [`entry_is_hidden_or_system`] (§1.1, P2.65) — reads the
/// entry's own `file_attributes()` (`walkdir` honours `follow_links(false)`, so it is the entry's word, not a
/// target's) and tests the HIDDEN/SYSTEM bits. Thin platform glue (cf. the §2.14/§7.7 platform shims); a
/// metadata error is treated as not-attribute-hidden (the entry is still classified by name above).
#[cfg(windows)]
fn windows_attr_hidden(entry: &walkdir::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x0000_0002;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x0000_0004;
    entry
        .metadata()
        .map(|meta| meta.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0)
        .unwrap_or(false)
}

/// Non-Windows: there is no hidden/system file-attribute, so the NAME rule (dotfiles + sentinels) is the whole
/// §1.1 hidden-file policy — this is the constant-false OR-leg of [`entry_is_hidden_or_system`].
#[cfg(not(windows))]
fn windows_attr_hidden(_entry: &walkdir::DirEntry) -> bool {
    false
}

/// One first-seen survivor of the §2.3.2 de-duplicated frozen set (P2.76) — the typed output row of
/// [`dedup_by_identity`]. Carries its freeze-assigned [`ItemId`] (§0.6 invariant 6 — one per SURVIVOR,
/// contiguous over the survivors, a dropped duplicate consumes none), the RETAINED first-seen
/// [`FileIdentity`] (§2.3.2: identity is the de-dup key; `identity.canonical_path` is the first-seen
/// representative path the P3.49 spine projects onto the off-wire `FrozenCollectedSet.item_paths[item].
/// resolved_path` (an `ItemPaths`, §2.10.1 — no path crosses the wire; the wire `DroppedItem` carries only
/// `display_name`), and the identity itself feeds §2.3.3 `is_safe_output`), and the abstract per-candidate
/// `payload` the spine threads through
/// un-inspected (detection is P3, so P2.76 never constructs a §0.6 `DroppedItem`/`SkippedItem`).
///
/// [Build-Session-Entscheidung: P2.76] Derives `Debug` ONLY — NOT `PartialEq`/`Eq`: those would leak a
/// `P: PartialEq/Eq` bound onto every consumer, and a whole-struct `Eq` would be MISLEADING (`FileIdentity`'s
/// `Eq` ignores `canonical_path`, so two rows with different first-seen paths but the same identity would
/// compare equal). The §6.4.1 tests assert on the fields (`.id` / `.identity.canonical_path` / `.payload`)
/// individually instead. Core-INTERNAL (never crosses IPC) → no `serde`/`specta`.
#[derive(Debug)]
struct DedupedMember<P> {
    /// The freeze-assigned §0.6 `ItemId` — one per survivor (§0.6 invariant 6).
    id: ItemId,
    /// The retained first-seen §2.3.1 resolved identity (§2.3.2) — its `canonical_path` is the first-seen
    /// representative path.
    identity: FileIdentity,
    /// The abstract per-candidate payload the P3.49 spine threads through (a `PathBuf` for the CSV→TSV
    /// walking skeleton, or a richer detected candidate) — never inspected by the fold.
    payload: P,
}

/// **Step 3 of the §2.4.1 freeze spine (P2.76)** — the PURE §2.3.2 resolved-identity de-dup fold. Over the
/// walk candidates ALREADY paired with their resolved [`FileIdentity`] (§2.3.1; the IO/FFI `resolve_identity`
/// that PRODUCES each identity is WHOLLY P3 and FEEDS this fold via the P3.49 spine — the fold performs NO
/// I/O and is unit-tested with `FileIdentity` values directly), keep the FIRST-SEEN member per identity
/// (§2.3.2) and drop every subsequent duplicate (a file reached via two paths, or a hardlink pair, collapses to
/// ONE member → converted once, SSOT), minting exactly one [`ItemId`] per SURVIVOR over the single id space
/// (§0.6 invariant 6 — a dropped duplicate consumes NO id). ORDER-preserving: survivors keep the walk's
/// first-seen order, so their ids are contiguous from the cursor. `Err(`[`ItemSpaceExhausted`]`)` is
/// `?`-propagated (never a panic/wrap, G4/G14); mapping it to the §1.1 fatal-ingest surface is the P3.49
/// spine's job. The [`ItemIdSpace`] is passed by `&mut` (NOT owned): §0.6 invariant 6 is ONE space across
/// the eligible survivors AND the §1.1 skips, so the P3.49 assembly owns the single space and threads it
/// through — the skip ids (for `WalkSkip`s, which have no resolvable identity and are therefore NOT de-duped
/// here) mint from the same cursor after this fold. [Build-Session-Entscheidung: P2.76]
fn dedup_by_identity<P>(
    candidates: Vec<(FileIdentity, P)>,
    ids: &mut ItemIdSpace,
) -> Result<Vec<DedupedMember<P>>, ItemSpaceExhausted> {
    let mut seen: HashSet<FileIdentity> = HashSet::with_capacity(candidates.len());
    let mut survivors: Vec<DedupedMember<P>> = Vec::with_capacity(candidates.len());
    for (identity, payload) in candidates {
        // §2.3.2: identity — NOT the path string — is the de-dup key. `FileIdentity`'s hand-written Eq/Hash
        // key ONLY on (dev, inode)/file-index, so a hardlink (same inode, different `canonical_path`)
        // collapses here; the FIRST-seen candidate's identity is retained (its `canonical_path` = the §2.3.2
        // first-seen representative path). `insert` clones the identity for the set key; the original moves
        // into the survivor on the first-seen branch (or is dropped with a duplicate).
        if seen.insert(identity.clone()) {
            // First sighting of this resolved file → a SURVIVOR: mint one id over the single space. A repeat
            // sighting takes the implicit `else` (does nothing) and mints NOTHING, consuming no id (§0.6
            // invariant 6). The `?` propagates `ItemSpaceExhausted` without a panic (G4/G14).
            let id = ids.mint()?;
            survivors.push(DedupedMember {
                id,
                identity,
                payload,
            });
        }
    }
    Ok(survivors)
}

/// The §2.4.1 freeze-spine step-3 output (P3.7) — the real-FS resolved-identity de-dup over walk candidate
/// PATHS. Carries the first-seen SURVIVORS (the P2.76 [`dedup_by_identity`] fold's rows, ids minted over the
/// threaded space) PLUS the `unresolved` per-item read failures (a candidate whose `resolve_identity` failed —
/// it vanished / became unreadable between the §1.1 walk and this resolve step). The `unresolved` rows are
/// §1.1 `Unreadable` [`WalkSkip`]s WITHOUT an `ItemId` — the P3.49 spine mints their ids from the same cursor
/// after the survivors (the P2.76 `&mut ItemIdSpace` contract) — so this step never silently drops a candidate
/// (§1.1: recorded, never dropped) and never lets a single vanished file sink the ingest.
#[derive(Debug)]
struct ResolvedDedup<P> {
    /// The de-duplicated first-seen survivors ([`dedup_by_identity`], P2.76) — one per resolved file, ids
    /// minted `0..` over the threaded space in first-seen order (§0.6 invariant 6).
    survivors: Vec<DedupedMember<P>>,
    /// The candidates whose `resolve_identity` failed — §1.1 `Unreadable` [`WalkSkip`]s, PRE-`ItemId` (the
    /// P3.49 spine mints their ids after the survivors). Empty in the normal all-resolvable case.
    unresolved: Vec<WalkSkip>,
}

/// **Step 3 of the §2.4.1 freeze spine, the real-FS half (P3.7)** — resolve each walk candidate PATH to its
/// §2.3.1 [`FileIdentity`] via the IO/FFI [`crate::fs_guard::resolve_identity`] (P3.6), then de-duplicate by
/// identity through the pure P2.76 [`dedup_by_identity`] fold. This is the box's real-FS integration — the
/// hardlink / two-paths-to-one-inode collapse to ONE first-seen survivor (§2.3.2 "converted once", SSOT) that
/// a synthetic-`FileIdentity` unit cannot exercise, keyed on the authoritative (dev, inode)/file-index
/// identity `canonicalize` alone misses (§2.3.4).
///
/// The per-candidate `payload` `P` is threaded un-inspected onto the survivor (a `PathBuf` for the CSV→TSV
/// walking skeleton, or a richer detected candidate) — the fold keys ONLY on identity. `ids: &mut ItemIdSpace`
/// is threaded, not owned: §0.6 invariant 6 is ONE space across the survivors AND the §1.1 skips, so the P3.49
/// spine owns the space and mints the `unresolved` skip ids from the same cursor after this step (the P2.76
/// contract). `Err(`[`ItemSpaceExhausted`]`)` is `?`-propagated, never a panic (G4/G14); the P3.49 spine maps
/// it to the §1.1 fatal-ingest surface.
///
/// A candidate whose `resolve_identity` FAILS is surfaced on `unresolved` as an `Unreadable` [`WalkSkip`] and
/// the de-dup proceeds over the resolvable rest — see the `Err` arm.
fn resolve_and_dedup<P>(
    candidates: Vec<(PathBuf, P)>,
    ids: &mut ItemIdSpace,
) -> Result<ResolvedDedup<P>, ItemSpaceExhausted> {
    let mut resolved: Vec<(FileIdentity, P)> = Vec::with_capacity(candidates.len());
    let mut unresolved: Vec<WalkSkip> = Vec::new();
    for (path, payload) in candidates {
        match crate::fs_guard::resolve_identity(&path) {
            Ok(identity) => resolved.push((identity, payload)),
            // §2.3.1 `resolve_identity` is fallible: a source that existed at the §1.1 walk can vanish/lock
            // between walk and freeze (TOCTOU), so a failure here is a per-item read failure, NOT a fatal
            // ingest error — it is recorded and the de-dup continues (never a panic, G4/G14; never silently
            // dropped, §1.1). [Derived-Assumption: P3.7 — a resolve-time read failure maps to the §1.1
            //  `Unreadable` `WalkSkip` class P2.67 records for a walk-level read failure, anchored to §1.1
            //  ("a per-item failure is recorded, never dropped, and the walk continues — one bad file never
            //  sinks the ingest") + §2.3.1 ("a missing source is a clean `Err` the caller maps").]
            // [Build-Session-Entscheidung: P3.7 — reuse the module's `WalkSkip` / `SkipReason::Unreadable` for
            //  a resolve-time failure rather than a parallel type; both are pre-`ItemId` per-item `Unreadable`
            //  read failures the freeze accounts for identically (§1.4 summary).]
            Err(_) => unresolved.push(WalkSkip {
                path,
                reason: SkipReason::Unreadable,
            }),
        }
    }
    // §2.3.2 first-seen de-dup over the resolved identities (P2.76): a hardlink / symlink pair collapses to
    // one member, ids minted over the shared space; the `?` propagates `ItemSpaceExhausted` (P3.49 maps it,
    // and its u32-ceiling source is unit-tested at `ItemIdSpace::mint`, P2.75 — no fold-level ceiling seam).
    let survivors = dedup_by_identity(resolved, ids)?;
    Ok(ResolvedDedup {
        survivors,
        unresolved,
    })
}

/// A single **detected** §1.1 walk candidate feeding the §2.4.1 freeze ([`freeze_snapshot`], P3.32) — one
/// file the §1.1 walk (P2.64) yielded and §1.2 detection (P3.26–P3.29) classified, PRE-freeze: no `ItemId`
/// yet, not yet resolved to a `FileIdentity` (§2.3) nor de-duplicated (§2.3.2). The freeze folds it in —
/// resolve + de-dup its `raw_path` (P3.7), mint its single-space `ItemId` (P2.75), and PARTITION it by the
/// P2.73 intake rule: a `Recognized` verdict becomes an eligible §0.6 `DroppedItem`, an ineligible one
/// (`Empty`/`Unreadable`/`UnsupportedType`/`Uncertain`) a pre-flight `SkippedItem`.
///
/// [Build-Session-Entscheidung: P3.32] `size_bytes` + `rel_path_display` are freeze INPUTS (read/computed
/// UPSTREAM at the P3.49 walk/detect read — which already stats the file and holds the §2.7 root context),
/// NOT re-derived here: the freeze does no second stat and owns no §2.7 root logic; it merely RECORDS them
/// into the frozen `DroppedItem` (`DroppedItem.size_bytes` doc: "recorded at the §2.4 freeze" = the value the
/// freeze lands in the item). The lossy §2.10.1 `display_name` (basename) / `source_display` (path)
/// projections, by contrast, ARE produced here from `raw_path` (the freeze is the core-side birthplace of the
/// wire DTOs). The freeze owns the STRUCTURAL snapshot (dedup + classification + the single id space), not the
/// §2.7 subtree / §1.10 sizing.
struct DetectedCandidate {
    /// The path as the OS handed it at drop/pick (§2.10.1) — the resolve input + the off-wire
    /// `ItemPaths.raw_path`, and the source of the lossy `display_name` / `source_display`.
    raw_path: PathBuf,
    /// The single §1.2 detection verdict (P3.26–P3.29) — the freeze partition key (via `skip_reason`, P2.16).
    detected: DetectionOutcome,
    /// The resolved file size in bytes (read upstream at the detect stat) — recorded into `DroppedItem.size_bytes`.
    size_bytes: u64,
    /// The §2.7 root-relative subpath preview for a folder-drop member (upstream; `None` for a top-level item).
    rel_path_display: Option<String>,
}

/// The §2.4.1 **frozen snapshot** ([`freeze_snapshot`], P3.32) — the eager, once-materialised, IMMUTABLE
/// image of the drop the run iterates and NEVER re-derives (the structural T8 no-self-feeding defence,
/// §2.4.2 defence 1: the walk already happened and produced a fixed list, so a file written into a source
/// folder AFTER the freeze is simply not in it). Carries the §0.6-invariant-6 SINGLE id space split into two
/// id-DISJOINT views — the eligible `items` (`DroppedItem`) and the ineligible `skipped` (`SkippedItem`) —
/// plus the OFF-WIRE per-item `item_paths` table (§0.4.4 / §2.10.1, keyed by `ItemId` over BOTH views) and
/// the retained dropped `roots` (§1.1 / §2.7, P2.66). The P3.49 ingest spine calls this, then projects the
/// snapshot through §1.3 `group()` into the wire `CollectedSet` and registers a `FrozenCollectedSet` (P3.76)
/// — this box homes the snapshot primitive; P3.49 wires it.
// [Test-Change: P3.49 — old-obsolete+new-correct, §2.4.1] the P3.32 dead-code lint attribute is removed — the
// `ingest` funnel spine (this snapshot's production reader) is now LIVE (P3.49), so the dead-code lint is
// obsolete (a production lint removal, not a test suppression).
#[derive(Debug)]
struct FrozenSnapshot {
    /// The eligible frozen members (§2.4) — the immutable `Vec<DroppedItem>` the run iterates, in first-seen
    /// (walk) order, ids drawn from the single space (§0.6 invariant 6).
    items: Vec<DroppedItem>,
    /// The id-disjoint ineligible view (§0.6 invariant 6) — the pre-flight `Skipped`s: the detect-ineligible
    /// (`Empty`/`Unreadable`/`UnsupportedType`/`Uncertain`) survivors AND the §1.1 read-failure skips
    /// (walk-level P2.67 + resolve-time P3.7 `unresolved`), the read-failure ids minted AFTER the survivors.
    skipped: Vec<SkippedItem>,
    /// The §0.4.4 OFF-WIRE per-item path table (§2.10.1) — the real `raw_path`/`resolved_path` keyed by
    /// `ItemId` over the single space, so BOTH the eligible and the skipped items resolve their real path.
    item_paths: BTreeMap<ItemId, ItemPaths>,
    /// The §2.3 identity evidence RETAINED at the freeze (P3.40 / §0.4.4 `[CLARIFIED]`) — the `(dev, inode)`
    /// `FileIdentity` keyed by `ItemId` over every RESOLVED survivor: the eligible members AND the
    /// detect-ineligible skips alike (both exit the same §2.3 resolve+de-dup pass WITH an identity, §0.6
    /// invariant 6). A walk/resolve-FAILURE skip (its `resolve_identity` failed) has NO entry — a physical
    /// fact, not a scoping choice. Retained so the §2.5.1 EquivKey folds `source_identity` (identity, NOT the
    /// §2.3.2 representative path — the hardlink/two-paths match, P3.39) and the §2.3.3 write-time comparison
    /// set draws from the FULL table (§2.3's unqualified "any source in the frozen set"). Homed here in the
    /// tier-1 orchestrator, NEVER on the tier-3 `domain` `FrozenCollectedSet` (`FileIdentity` is a tier-2
    /// `fs_guard` type — embedding it would be an upward §0.7 edge). [Build-Session-Entscheidung: P3.40]
    identities: BTreeMap<ItemId, FileIdentity>,
    /// The dropped root(s) retained VERBATIM (§1.1 / §2.7, P2.66) — the §2.7 subtree / open-folder anchor.
    roots: Vec<PathBuf>,
}

/// **The §2.4.1 freeze-point (P3.32)** — materialise the §1.1 walk's DETECTED candidates EAGERLY and ONCE
/// into an IMMUTABLE [`FrozenSnapshot`] (§2.4). This is the structural T8 no-self-feeding defence: the drop
/// becomes a fixed `Vec` here and the run iterates that snapshot, NEVER re-reading the directory (§2.4.2
/// defence 1), so an output landing in a source folder afterward is invisible to the run.
///
/// The freeze folds the three freeze-spine steps this box owns the ASSEMBLY of (the walk P2.64 and detection
/// P3.26–P3.29 run UPSTREAM; the end-to-end `ingest` wiring is P3.49):
/// 1. **Resolve + de-dup (P3.7 / P2.76).** Each candidate's `raw_path` is resolved to its §2.3.1
///    `FileIdentity` and the set de-duplicated by identity ([`resolve_and_dedup`]) — a hardlink / two-paths
///    pair collapses to ONE first-seen survivor ("converted once", SSOT §2.3.2). One `ItemId` is minted per
///    SURVIVOR over the single space (§0.6 invariant 6); a candidate whose resolve FAILS (vanished/locked
///    between walk and freeze, TOCTOU) is surfaced `unresolved` as a §1.1 `Unreadable` skip, never dropped.
/// 2. **Classify (P2.73 / P2.16).** Each survivor is PARTITIONED by its detection verdict's
///    [`skip_reason`](DetectionOutcome::skip_reason): a `Recognized` verdict (`None`) becomes an eligible
///    `DroppedItem`; an ineligible one (`Empty`/`Unreadable`/`UnsupportedType`/`Uncertain`) a pre-flight
///    `SkippedItem` — the intake-time `Skipped` half of the P2.73 intake-`Skipped` vs turn-time-`Failed`
///    non-conflation (a turn-time gone/unreadable is a `Failed`, not a `Skipped`).
/// 3. **Materialise.** The eligible `items`, the ineligible `skipped` (the §1.1 read-failure skips —
///    walk-level P2.67 then resolve-time `unresolved` — minted AFTER the survivors from the SAME cursor, the
///    P2.76 single-space contract), the OFF-WIRE `item_paths` pair for every id, the retained per-RESOLVED-
///    survivor `identities` (the §2.3 evidence — every survivor but no read-failure skip, P3.40), and `roots`.
///
/// Fallible only on `ItemSpaceExhausted` (`?`-propagated, never a panic — G4/G14; the §1.10 bounds cap a real
/// frozen set far below `2^32`); the P3.49 spine maps it to the §1.1 fatal-ingest surface. The lossy §2.10.1
/// `display_name` (basename) / `source_display` (path) projections are produced here from `raw_path`; the §2.7
/// `rel_path_display` + the `size_bytes` are carried through from the candidate (see [`DetectedCandidate`]).
// [Test-Change: P3.49 — old-obsolete+new-correct, §2.4.1] the P3.32 dead-code lint attribute is removed — the
// `ingest` funnel spine (this freeze-point's production caller) is now LIVE (P3.49), so the dead-code lint is
// obsolete (a production lint removal, not a test suppression).
fn freeze_snapshot(
    candidates: Vec<DetectedCandidate>,
    walk_skips: Vec<WalkSkip>,
    roots: Vec<PathBuf>,
) -> Result<FrozenSnapshot, ItemSpaceExhausted> {
    // §0.6 invariant 6: ONE monotonic id space across the eligible members AND every skip (never re-indexed),
    // constructed once per freeze and threaded so the two views are id-disjoint BY CONSTRUCTION (P2.75).
    let mut ids = ItemIdSpace::new();

    // Step 1 (P3.7 / P2.76): resolve each candidate to its §2.3.1 FileIdentity and de-dup by identity, minting
    // one survivor id per first-seen resolved file over the single space. The `DetectedCandidate` rides as the
    // threaded payload (its `raw_path` is cloned in as the resolve target; the payload keeps its own copy for
    // `ItemPaths.raw_path` + the §2.10.1 display). A resolve failure lands on `unresolved` (§1.1 `Unreadable`).
    let resolve_input: Vec<(PathBuf, DetectedCandidate)> = candidates
        .into_iter()
        .map(|candidate| (candidate.raw_path.clone(), candidate))
        .collect();
    let ResolvedDedup {
        survivors,
        unresolved,
    } = resolve_and_dedup(resolve_input, &mut ids)?;

    let mut items: Vec<DroppedItem> = Vec::with_capacity(survivors.len());
    let mut skipped: Vec<SkippedItem> = Vec::new();
    let mut item_paths: BTreeMap<ItemId, ItemPaths> = BTreeMap::new();
    // §2.3 identity evidence retained per RESOLVED survivor (P3.40) — populated in the survivor loop only
    // (a walk/resolve-FAILURE skip below has no identity). The §0.4.4 identity-evidence mandate.
    let mut identities: BTreeMap<ItemId, FileIdentity> = BTreeMap::new();

    // Step 2 (P2.73 / P2.16): PARTITION each first-seen survivor by its detection verdict. The survivor already
    // carries its single-space `id` (minted in step 1) and its §2.3.1 `identity` (canonical resolved path).
    for DedupedMember {
        id,
        identity,
        payload,
    } in survivors
    {
        let DetectedCandidate {
            raw_path,
            detected,
            size_bytes,
            rel_path_display,
        } = payload;
        // §0.4.4 / §2.10.1 off-wire path pair: `raw` = the as-dropped path, `resolved` = the §2.3.2 canonical
        // REPRESENTATIVE (the §1.7 engine target). Keyed by the item's id over the single space so BOTH views
        // resolve. The `(dev, inode)` identity is NOT the path — it is retained separately below (§2.3.1).
        item_paths.insert(
            id,
            ItemPaths {
                raw_path: raw_path.clone(),
                resolved_path: identity.canonical_path.clone(),
            },
        );
        // §2.3 identity evidence (P3.40 / §0.4.4): RETAIN the resolved `(dev, inode)` `FileIdentity` keyed by
        // the survivor's id — the §2.5.1 EquivKey `source_identity` (P3.39) + the §2.3.3 comparison set. Every
        // survivor (eligible OR detect-ineligible skip) has one; only the read-failure skips below do not.
        identities.insert(id, identity);
        match detected.skip_reason() {
            // `Recognized` → an eligible frozen member. The lossy §2.10.1 display basename is produced here.
            None => items.push(DroppedItem {
                item: id,
                display_name: display_basename(&raw_path),
                rel_path_display,
                size_bytes,
                detected,
            }),
            // Ineligible (`Empty`/`Unreadable`/`UnsupportedType`/`Uncertain`) → a pre-flight `Skipped` (the
            // P2.73 intake half; a turn-time read failure is a `Failed`, not this).
            Some(reason) => skipped.push(SkippedItem {
                item: id,
                source_display: raw_path.to_string_lossy().into_owned(),
                // RETAIN the friendly detected-type name from detection's own output (SSOT-6 / P3.50):
                // Some for UnsupportedType, the named best_guess for Uncertain, None otherwise — kept
                // through the freeze rather than discarded (RETENTION, not invention).
                detected_display: detected.detected_display(),
                reason,
            }),
        }
    }

    // Step 3 (§1.1): the read-failure skips — the walk-level `Unreadable`s (P2.67) then the resolve-time
    // `unresolved` (P3.7) — have NO resolvable identity, so they mint their ids AFTER the survivors from the
    // same cursor (the P2.76 single-space contract) and their off-wire pair uses the dropped path for both
    // sides. Recorded, never dropped (§1.1: one bad file never sinks the ingest).
    // [Build-Session-Entscheidung: P3.32 — order the read-failure skips walk-level THEN resolve-time
    //  (pipeline-discovery order); §0.6 invariant 6 constrains only disjoint-contiguous ids, not the skip order.]
    for WalkSkip { path, reason } in walk_skips.into_iter().chain(unresolved) {
        let id = ids.mint()?;
        item_paths.insert(
            id,
            ItemPaths {
                raw_path: path.clone(),
                resolved_path: path.clone(),
            },
        );
        skipped.push(SkippedItem {
            item: id,
            source_display: path.to_string_lossy().into_owned(),
            // A read-failure skip has NO DetectionOutcome (it failed before/at detection) — no type name.
            detected_display: None,
            reason,
        });
    }

    Ok(FrozenSnapshot {
        items,
        skipped,
        item_paths,
        identities,
        roots,
    })
}

/// The lossy §2.10.1 DISPLAY basename for a `DroppedItem.display_name` (last-step `to_string_lossy`) — the
/// file's own name, produced core-side at the §2.4 freeze. Falls back to the whole lossy path for the
/// nameless-tail case (a path ending in `..` / a bare root) so the display is never empty; a real dropped
/// file always has a `file_name`. [Build-Session-Entscheidung: P3.32]
fn display_basename(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| path.to_string_lossy(), std::ffi::OsStr::to_string_lossy)
        .into_owned()
}

#[cfg(test)]
mod resolve_dedup_realfs_tests {
    //! §6.4.1/§6.4.3 real-FS (G15/G31) for the §2.4.1 freeze-spine step-3 REAL-FS resolved-identity de-dup
    //! ([`resolve_and_dedup`], P3.7). Never mock the FS under test (test-strategy §0.1): these drive the real
    //! `resolve_identity` (P3.6) over real temp files / hardlinks — the hardlink two-paths→one-member collapse
    //! the synthetic-`FileIdentity` `dedup_tests` unit (P2.76) can only assert by construction, proven here on
    //! a real filesystem (§2.3.2 "converted once", SSOT). The symlink-follow half is the unix module below.
    use super::*;

    // §2.3.2/§2.3.4 (G15/G31): the HEADLINE real-FS proof — two names over ONE (dev, inode)/file-index (a
    // hardlink) collapse to ONE first-seen survivor, keyed on the identity `canonicalize` alone misses. The
    // FIRST-seen path + payload are retained; exactly one id is minted; nothing is unresolved.
    #[test]
    fn hardlink_two_real_paths_collapse_to_one_first_seen_survivor() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let original = dir.path().join("original.csv");
        std::fs::write(&original, b"a,b\n1,2\n").expect("write the original");
        let link = dir.path().join("backup-link.csv");
        // A no-hardlink volume (FAT/exFAT, §2.3.4) reports Unsupported/PermissionDenied — skip only THAT; any
        // other error is a real failure (this is the sole real-FS proof of the §2.3.2 hardlink de-dup collapse
        // at the composition level). Real temp dirs are NTFS/ext4/APFS, so the skip does not fire in practice.
        // [Build-Session-Entscheidung: P3.7 — mirror fs_guard's own hardlink-test skip guard.]
        let linked = std::fs::hard_link(&original, &link);
        if matches!(&linked, Err(e) if matches!(e.kind(), std::io::ErrorKind::Unsupported | std::io::ErrorKind::PermissionDenied))
        {
            return;
        }
        linked.expect("create the hardlink (a non-unsupported error is a real failure)");

        let mut ids = ItemIdSpace::new();
        let candidates = vec![
            (original.clone(), "first"),
            (link, "second"), // hardlink: same (dev, inode), different path
        ];
        let out = resolve_and_dedup(candidates, &mut ids).expect("space not exhausted");
        assert_eq!(
            out.survivors.len(),
            1,
            "§2.3.2: two real paths to one inode collapse to one first-seen survivor"
        );
        assert_eq!(
            out.survivors[0].payload, "first",
            "§2.3.2: the FIRST-seen candidate (original) is the retained survivor, not the hardlink"
        );
        assert_eq!(
            out.survivors[0].id,
            ItemId::from_index(0),
            "§0.6 inv-6: the sole survivor gets id 0"
        );
        assert!(
            out.unresolved.is_empty(),
            "both real paths resolved — nothing is unresolved"
        );
        // §2.3.2: the retained representative's canonical path is the ORIGINAL's resolved path, not the
        // hardlink's (canonicalize cannot follow a hardlink, §2.3.4) — first-seen wins.
        let original_id =
            crate::fs_guard::resolve_identity(&original).expect("resolve the original directly");
        assert_eq!(
            out.survivors[0].identity.canonical_path, original_id.canonical_path,
            "§2.3.2: the retained representative is the first-seen (original) resolved path"
        );
    }

    // §2.3.1 (G15/G31): two genuinely distinct real files do NOT collapse — 2 survivors, ids 0,1, nothing
    // unresolved. The over-collapse control (kills a mutant keying on something coarser than the identity).
    #[test]
    fn two_distinct_real_files_both_survive() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let one = dir.path().join("one.csv");
        let two = dir.path().join("two.csv");
        std::fs::write(&one, b"x").expect("write one");
        std::fs::write(&two, b"y").expect("write two");
        let mut ids = ItemIdSpace::new();
        let out = resolve_and_dedup(vec![(one, "one"), (two, "two")], &mut ids)
            .expect("space not exhausted");
        assert_eq!(
            out.survivors.len(),
            2,
            "§2.3.1: two distinct real files both survive (no over-collapse)"
        );
        let got_ids: Vec<ItemId> = out.survivors.iter().map(|m| m.id).collect();
        assert_eq!(
            got_ids,
            vec![ItemId::from_index(0), ItemId::from_index(1)],
            "§0.6 inv-6: two survivors get ids 0,1"
        );
        assert!(out.unresolved.is_empty(), "both files resolved");
    }

    // §2.8/§1.1 (G15/G31): a candidate that does NOT exist (vanished between the §1.1 walk and this resolve
    // step) surfaces as an `Unreadable` skip on `unresolved` — never a panic (G4/G14) and never silently
    // dropped (§1.1) — while the resolvable candidate still survives and mints its id. The Err arm end-to-end.
    #[test]
    fn a_vanished_candidate_surfaces_as_unresolved_not_a_survivor() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let real = dir.path().join("real.csv");
        std::fs::write(&real, b"z").expect("write the real file");
        // Doubly-missing (no parent) so `resolve_identity`'s canonicalize is Err regardless of any retry.
        let missing = dir.path().join("gone").join("missing.csv");
        let mut ids = ItemIdSpace::new();
        let out = resolve_and_dedup(vec![(real, "real"), (missing.clone(), "missing")], &mut ids)
            .expect("space not exhausted");
        assert_eq!(
            out.survivors.len(),
            1,
            "only the resolvable candidate survives the resolve step"
        );
        assert_eq!(out.survivors[0].payload, "real");
        assert_eq!(
            out.unresolved.len(),
            1,
            "§1.1: the vanished candidate is recorded, never silently dropped"
        );
        assert_eq!(
            out.unresolved[0].path, missing,
            "§1.1: the unresolved skip carries the failed candidate's path"
        );
        assert_eq!(
            out.unresolved[0].reason,
            SkipReason::Unreadable,
            "§2.3.1/§1.1: a resolve-time read failure is an Unreadable skip"
        );
        // §0.6 inv-6: only the survivor consumed an id — the shared space's next mint is 1 (the vanished
        // candidate minted nothing; the P3.49 spine assigns the skip's id from here).
        assert_eq!(
            ids.mint().expect("space not exhausted"),
            ItemId::from_index(1),
            "§0.6 inv-6: only the survivor consumed an id (next mint is 1)"
        );
    }
}

// §6.4.3 real-FS unix (G15/G31): the symlink-follow half of the §2.4.1 step-3 de-dup ([`resolve_and_dedup`],
// P3.7) — a symlink + its target collapse to one survivor (canonicalize FOLLOWS a symlink, §2.3.4). TWO
// STACKED cfg attributes (`#[cfg(test)]` then `#[cfg(unix)]`), NOT the compound `#[cfg(all(test, unix))]` —
// the P1.17 trap (else the tests' `expect` calls trip clippy::expect_used on the ubuntu/macOS legs). Windows
// symlink creation needs the SeCreateSymbolicLink privilege (fs_guard gates that leg); the cross-platform
// hardlink test above proves real-FS collapse on every OS.
#[cfg(test)]
#[cfg(unix)]
mod resolve_dedup_unix_realfs_tests {
    use super::*;

    // §2.3.4: a symlink and its target resolve to ONE identity (canonicalize follows the link), so the two
    // dropped paths collapse to one first-seen survivor — the follow-symlink counterpart to the hardlink test.
    #[test]
    fn symlink_and_target_collapse_to_one_first_seen_survivor() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let target = dir.path().join("target.csv");
        std::fs::write(&target, b"a\n").expect("write the target");
        let link = dir.path().join("alias.csv");
        std::os::unix::fs::symlink(&target, &link).expect("create a unix symlink");
        let mut ids = ItemIdSpace::new();
        // Target seen FIRST, then the symlink — both canonicalize to the target's real path (§2.3.4).
        let out = resolve_and_dedup(vec![(target, "target"), (link, "link")], &mut ids)
            .expect("space not exhausted");
        assert_eq!(
            out.survivors.len(),
            1,
            "§2.3.4: a symlink and its target resolve to one identity — one survivor"
        );
        assert_eq!(
            out.survivors[0].payload, "target",
            "§2.3.2: the first-seen (target) is retained, not the alias"
        );
        assert!(out.unresolved.is_empty(), "both paths resolved");
    }
}

#[cfg(test)]
mod freeze_tests {
    //! §6.4.1 unit (G15) + §6.4.2 property (G16) for the §2.4.1 freeze-point ([`freeze_snapshot`], P3.32) —
    //! the eager, once, IMMUTABLE snapshot materialisation (the structural T8 no-self-feeding defence,
    //! §2.4.2 defence 1). Real-FS, never mocked (test-strategy §0.1): the freeze folds the REAL
    //! `resolve_and_dedup` (P3.7) over real temp files, so its resolve/de-dup half runs against a real
    //! filesystem; the P2.73 partition + the single-id-space assembly (§0.6 invariant 6) are asserted on the
    //! resulting snapshot. The pure id-space composition (dedup × mint × skip) is additionally property-tested
    //! at the fold level in `mod tests`
    //! (`prop_dedup_and_skip_minting_compose_over_one_contiguous_id_space`, P2.137); the property here covers
    //! the FREEZE's end-to-end partition + `item_paths` completeness over a generated detected-outcome mix.
    use super::*;
    use crate::domain::Confidence;
    use proptest::prelude::*;
    use proptest::test_runner::{RngAlgorithm, TestRng, TestRunner};
    use std::collections::BTreeSet;

    /// The eligible detection verdict (a recognized CSV) — projects to `None` skip reason ⇒ a `DroppedItem`.
    fn recognized() -> DetectionOutcome {
        DetectionOutcome::Recognized {
            format: UserFacingFormat::Csv,
            confidence: Confidence::High,
            dims: None,
        }
    }

    /// Write a real temp file `name` in `dir` and return a `DetectedCandidate` for it with the given verdict
    /// (size 4, no §2.7 rel-path). Real-FS so the freeze's `resolve_identity` fold has a real inode to key on.
    fn write_candidate(dir: &Path, name: &str, detected: DetectionOutcome) -> DetectedCandidate {
        let raw_path = dir.join(name);
        std::fs::write(&raw_path, b"data").expect("write a real temp source");
        DetectedCandidate {
            raw_path,
            detected,
            size_bytes: 4,
            rel_path_display: None,
        }
    }

    // §6.4.1 (G15): the empty freeze — no candidates, no walk skips → an empty snapshot; the dropped roots are
    // still retained VERBATIM (§1.1/§2.7, P2.66) so "open folder" anchors even when nothing is eligible.
    #[test]
    fn an_empty_intake_freezes_an_empty_snapshot_but_retains_the_roots() {
        let roots = vec![PathBuf::from("/drop/one"), PathBuf::from("/drop/two")];
        let snap =
            freeze_snapshot(vec![], vec![], roots.clone()).expect("no ids minted, no exhaustion");
        assert!(snap.items.is_empty(), "no candidates → no eligible members");
        assert!(
            snap.skipped.is_empty(),
            "no candidates + no walk skips → no skips"
        );
        assert!(
            snap.item_paths.is_empty(),
            "no items → an empty off-wire path table"
        );
        assert_eq!(
            snap.roots, roots,
            "§1.1/§2.7 (P2.66): the dropped roots are retained VERBATIM in input order, even when nothing is eligible"
        );
    }

    // §6.4.1 (G15): a recognized candidate → an eligible frozen member carrying id 0, its lossy §2.10.1 display
    // basename, the carried-through §2.7 rel-path + size, and its §2.3 resolved identity in the off-wire table.
    #[test]
    fn a_recognized_candidate_becomes_an_eligible_member_with_its_resolved_identity() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let raw = dir.path().join("data.csv");
        std::fs::write(&raw, b"a,b\n1,2\n").expect("write the source");
        let candidate = DetectedCandidate {
            raw_path: raw.clone(),
            detected: recognized(),
            size_bytes: 8,
            rel_path_display: Some("sub/data.csv".to_string()),
        };
        let snap = freeze_snapshot(vec![candidate], vec![], vec![dir.path().to_path_buf()])
            .expect("no exhaustion");
        assert_eq!(snap.items.len(), 1, "the recognized candidate is eligible");
        assert!(snap.skipped.is_empty(), "nothing ineligible → no skips");
        let item = &snap.items[0];
        assert_eq!(
            item.item,
            ItemId::from_index(0),
            "§0.6 inv-6: the sole eligible member gets id 0"
        );
        assert_eq!(
            item.display_name, "data.csv",
            "§2.10.1: display_name is the lossy basename, produced core-side at the freeze"
        );
        assert_eq!(
            item.rel_path_display.as_deref(),
            Some("sub/data.csv"),
            "the §2.7 subtree preview is carried through the freeze"
        );
        assert_eq!(
            item.size_bytes, 8,
            "the upstream-read size is recorded verbatim"
        );
        assert!(matches!(item.detected, DetectionOutcome::Recognized { .. }));
        let paths = snap
            .item_paths
            .get(&item.item)
            .expect("§0.4.4: every eligible id has an off-wire path entry");
        assert_eq!(paths.raw_path, raw, "raw_path = the as-dropped path");
        let resolved = crate::fs_guard::resolve_identity(&raw)
            .expect("resolve the source directly")
            .canonical_path;
        assert_eq!(
            paths.resolved_path, resolved,
            "§2.3.2: resolved_path = the canonical representative path (the §1.7 engine target; the (dev, inode) identity is retained separately)"
        );
    }

    // §6.4.1 (G15) · §0.4.4 identity-evidence mandate (P3.40): the freeze RETAINS the resolved (dev, inode)
    // FileIdentity for every RESOLVED survivor — the eligible member AND the detect-ineligible skip alike (§0.6
    // invariant 6) — and its canonical path is exactly the off-wire §2.3.2 representative the item_paths pair
    // carries (a clone, not a move). A walk/resolve-FAILURE skip has NO identity (its resolve_identity failed).
    #[test]
    fn the_freeze_retains_an_identity_for_every_resolved_survivor() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let eligible = write_candidate(dir.path(), "data.csv", recognized());
        let ineligible = write_candidate(
            dir.path(),
            "note.xyz",
            DetectionOutcome::UnsupportedType {
                detected: "XYZ".to_string(),
            },
        );
        let raw_eligible = eligible.raw_path.clone();
        // A read-failure walk skip — never resolved, so it carries no identity.
        let walk_skip = WalkSkip {
            path: dir.path().join("vanished.dat"),
            reason: SkipReason::Unreadable,
        };
        let snap = freeze_snapshot(vec![eligible, ineligible], vec![walk_skip], vec![])
            .expect("no exhaustion");

        // Both RESOLVED survivors — the eligible member AND the detect-ineligible skip — carry a retained identity.
        assert_eq!(
            snap.identities.len(),
            2,
            "§0.4.4/§0.6 inv-6: every RESOLVED survivor (eligible + detect-ineligible skip) has a retained identity"
        );
        let eligible_id = snap.items[0].item;
        let unsupported_id = snap
            .skipped
            .iter()
            .find(|s| s.reason == SkipReason::UnsupportedType)
            .expect("the detect-ineligible skip is recorded")
            .item;
        assert!(
            snap.identities.contains_key(&eligible_id),
            "the eligible member's §2.3 identity is retained"
        );
        assert!(
            snap.identities.contains_key(&unsupported_id),
            "§0.6 inv-6: the detect-ineligible skip's identity is ALSO retained (the §2.3.3 comparison set is the full table)"
        );

        // The retained identity is the REAL resolved (dev, inode), and its canonical path IS the representative.
        let real = crate::fs_guard::resolve_identity(&raw_eligible)
            .expect("resolve the eligible source directly");
        assert_eq!(
            snap.identities.get(&eligible_id),
            Some(&real),
            "§2.3.1: the retained identity is the resolved (dev, inode) identity"
        );
        assert_eq!(
            snap.identities[&eligible_id].canonical_path,
            snap.item_paths[&eligible_id].resolved_path,
            "the retained identity's canonical path IS the off-wire §2.3.2 representative (a clone, not divergent values)"
        );

        // A walk/resolve-FAILURE skip has an off-wire path pair but NO identity.
        let walk_id = snap
            .skipped
            .iter()
            .find(|s| s.reason == SkipReason::Unreadable)
            .expect("the read-failure skip is recorded")
            .item;
        assert!(
            snap.item_paths.contains_key(&walk_id),
            "the read-failure skip still has an off-wire path pair"
        );
        assert!(
            !snap.identities.contains_key(&walk_id),
            "§0.4.4: a walk/resolve-FAILURE skip has NO retained identity (its resolve_identity failed — a physical fact)"
        );
    }

    // §6.4.1 (G15) · P2.73 intake half: an intake-time `Empty` / `Unreadable` verdict is a pre-flight
    // `Skipped`, NEVER an eligible member (distinct from a turn-time gone/unreadable, which is a `Failed`).
    #[test]
    fn intake_empty_and_unreadable_verdicts_are_skipped_never_members() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let empty = write_candidate(dir.path(), "empty.dat", DetectionOutcome::Empty);
        let unreadable = write_candidate(
            dir.path(),
            "denied.dat",
            DetectionOutcome::Unreadable {
                reason: ReadFailure::PermissionDenied,
            },
        );
        let snap = freeze_snapshot(vec![empty, unreadable], vec![], vec![]).expect("no exhaustion");
        assert!(
            snap.items.is_empty(),
            "P2.73: an intake `Empty`/`Unreadable` verdict is a pre-flight `Skipped`, never eligible"
        );
        let reasons: Vec<SkipReason> = snap.skipped.iter().map(|s| s.reason).collect();
        assert_eq!(
            reasons,
            vec![SkipReason::Empty, SkipReason::Unreadable],
            "the §1.2 verdict projects to the matching `SkipReason` (P2.16), in first-seen order"
        );
    }

    // §6.4.1 (G15): the other two ineligible verdicts (`UnsupportedType`/`Uncertain`) are likewise skipped.
    #[test]
    fn unsupported_and_uncertain_verdicts_are_skipped() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let unsupported = write_candidate(
            dir.path(),
            "movie.xyz",
            DetectionOutcome::UnsupportedType {
                detected: "XYZ".to_string(),
            },
        );
        let uncertain = write_candidate(
            dir.path(),
            "mystery.bin",
            DetectionOutcome::Uncertain { best_guess: None },
        );
        let snap =
            freeze_snapshot(vec![unsupported, uncertain], vec![], vec![]).expect("no exhaustion");
        assert!(snap.items.is_empty(), "neither is a real convertible type");
        let reasons: Vec<SkipReason> = snap.skipped.iter().map(|s| s.reason).collect();
        assert_eq!(
            reasons,
            vec![SkipReason::UnsupportedType, SkipReason::Uncertain]
        );
    }

    // §6.4.1 (G15) · §0.6 invariant 6: a mixed intake shares ONE id space — eligible members and skips keep
    // their first-seen ids (id-disjoint, never re-indexed from 0), and `item_paths` keys exactly that space.
    #[test]
    fn a_mixed_intake_shares_one_disjoint_contiguous_id_space() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        // First-seen order: recognized, empty, recognized, unsupported → ids 0,1,2,3.
        let candidates = vec![
            write_candidate(dir.path(), "a.csv", recognized()),
            write_candidate(dir.path(), "b.dat", DetectionOutcome::Empty),
            write_candidate(dir.path(), "c.csv", recognized()),
            write_candidate(
                dir.path(),
                "d.xyz",
                DetectionOutcome::UnsupportedType {
                    detected: "XYZ".to_string(),
                },
            ),
        ];
        let snap = freeze_snapshot(candidates, vec![], vec![]).expect("no exhaustion");
        assert_eq!(
            snap.items.iter().map(|d| d.item).collect::<Vec<_>>(),
            vec![ItemId::from_index(0), ItemId::from_index(2)],
            "the eligible members keep their first-seen ids (0 and 2)"
        );
        assert_eq!(
            snap.skipped.iter().map(|s| s.item).collect::<Vec<_>>(),
            vec![ItemId::from_index(1), ItemId::from_index(3)],
            "the skips keep their first-seen ids (1 and 3), id-disjoint from the members"
        );
        let all: BTreeSet<ItemId> = snap
            .items
            .iter()
            .map(|d| d.item)
            .chain(snap.skipped.iter().map(|s| s.item))
            .collect();
        let expected: BTreeSet<ItemId> = (0u32..4).map(ItemId::from_index).collect();
        assert_eq!(
            all, expected,
            "§0.6 inv-6: eligible ⊎ skipped = the contiguous single id space 0..N"
        );
        assert_eq!(
            snap.item_paths.keys().copied().collect::<BTreeSet<_>>(),
            expected,
            "§0.4.4: item_paths covers EXACTLY every id (eligible + skipped)"
        );
    }

    // §6.4.1 (G15) · §1.1: the read-failure skips — a walk-level `Unreadable` (P2.67) then a resolve-time
    // `unresolved` (a candidate whose path vanished before the freeze, P3.7) — mint their ids AFTER the
    // survivors, walk-level before resolve-time; an unresolved item's off-wire pair is raw == resolved.
    #[test]
    fn read_failures_mint_ids_after_the_survivors() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let good = write_candidate(dir.path(), "ok.csv", recognized()); // survivor → id 0
        let gone = dir.path().join("gone.csv"); // never written → resolve fails → unresolved
        let vanished = DetectedCandidate {
            raw_path: gone.clone(),
            detected: recognized(),
            size_bytes: 1,
            rel_path_display: None,
        };
        let walk_skip = WalkSkip {
            path: dir.path().join("denied-subdir"),
            reason: SkipReason::Unreadable,
        };
        let snap =
            freeze_snapshot(vec![good, vanished], vec![walk_skip], vec![]).expect("no exhaustion");
        assert_eq!(
            snap.items.len(),
            1,
            "only the resolvable recognized file is an eligible member"
        );
        assert_eq!(snap.items[0].item, ItemId::from_index(0));
        assert_eq!(
            snap.skipped.iter().map(|s| s.item).collect::<Vec<_>>(),
            vec![ItemId::from_index(1), ItemId::from_index(2)],
            "§1.1: the read-failure skips mint AFTER the survivor — walk-level (id 1) before resolve-time (id 2)"
        );
        assert!(
            snap.skipped
                .iter()
                .all(|s| s.reason == SkipReason::Unreadable),
            "a walk-level or resolve-time read failure is `Unreadable`"
        );
        let vanished_paths = snap
            .item_paths
            .get(&ItemId::from_index(2))
            .expect("the unresolved item still has an off-wire path entry");
        assert_eq!(
            vanished_paths.raw_path, vanished_paths.resolved_path,
            "an unresolved item has no canonical identity — raw == resolved (the dropped path)"
        );
        assert_eq!(vanished_paths.raw_path, gone);
    }

    // §6.4.1 (G15) · §2.3.2 / T8 at the freeze: two paths to ONE inode (a hardlink) collapse to ONE frozen
    // member ("converted once", SSOT) — the real-FS collapse a synthetic-`FileIdentity` unit can only assert
    // by construction. Skipped only on a no-hardlink volume (FAT/exFAT, §2.3.4).
    #[test]
    fn hardlinked_candidates_collapse_to_one_frozen_member() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let original = dir.path().join("original.csv");
        std::fs::write(&original, b"a,b\n").expect("write the original");
        let link = dir.path().join("hardlink.csv");
        let linked = std::fs::hard_link(&original, &link);
        if matches!(&linked, Err(e) if matches!(e.kind(), std::io::ErrorKind::Unsupported | std::io::ErrorKind::PermissionDenied))
        {
            return; // FAT/exFAT — no hardlinks; the collapse is asserted on a linking volume only.
        }
        linked.expect("create the hardlink (a non-unsupported error is a real failure)");
        let candidates = vec![
            DetectedCandidate {
                raw_path: original,
                detected: recognized(),
                size_bytes: 4,
                rel_path_display: None,
            },
            DetectedCandidate {
                raw_path: link,
                detected: recognized(),
                size_bytes: 4,
                rel_path_display: None,
            },
        ];
        let snap = freeze_snapshot(candidates, vec![], vec![]).expect("no exhaustion");
        assert_eq!(
            snap.items.len(),
            1,
            "§2.3.2/T8: two paths to one inode collapse to ONE frozen member (converted once)"
        );
        assert_eq!(
            snap.items[0].display_name, "original.csv",
            "§2.3.2: the FIRST-seen path is the retained member, not the hardlink"
        );
        assert_eq!(
            snap.item_paths.len(),
            1,
            "the dropped duplicate consumes no id and no path entry"
        );
    }

    // §6.4.1 (G15): `size_bytes` is the upstream-read value RECORDED at the freeze, not a re-stat — the freeze
    // does no second `metadata` call (the file is 2 bytes on disk; the frozen item reports the declared 4096).
    #[test]
    fn the_recorded_size_is_the_upstream_value_not_a_refreeze_stat() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let raw = dir.path().join("small.csv");
        std::fs::write(&raw, b"ab").expect("write a 2-byte file");
        let candidate = DetectedCandidate {
            raw_path: raw,
            detected: recognized(),
            size_bytes: 4096,
            rel_path_display: None,
        };
        let snap = freeze_snapshot(vec![candidate], vec![], vec![]).expect("no exhaustion");
        assert_eq!(
            snap.items[0].size_bytes, 4096,
            "the freeze records the upstream-read size verbatim — it does NOT re-stat (which would read 2)"
        );
    }

    /// The §0.6-invariant property-test case-count floor (test-strategy §1.3: above proptest's 256 default).
    /// [Build-Session-Entscheidung: P3.32]
    const FREEZE_CASES: u32 = 512;

    /// A PINNED-SEED runner (test-strategy §1.3 / G16): the `proptest!` macro seeds its forward run from
    /// ENTROPY, so drive a `TestRunner` with a `deterministic_rng` for a reproducible 512-case exploration —
    /// a failure reproduces deterministically and is NEVER retried-to-pass (§7). Local to this module (the
    /// sibling `mod tests` `pinned_runner` is module-private). [Build-Session-Entscheidung: P3.32]
    fn pinned_runner() -> TestRunner {
        TestRunner::new_with_rng(
            ProptestConfig::with_cases(FREEZE_CASES),
            TestRng::deterministic_rng(RngAlgorithm::ChaCha),
        )
    }

    // §6.4.2 property (G16) · §2.4.1 / §0.6 invariant 6: the FREEZE's end-to-end partition + single-id-space
    // completeness over an arbitrary detected-outcome mix + walk-skip count, on real DISTINCT temp files (so
    // every candidate resolves — the de-dup collapse is the hardlink unit above; here N distinct files → N
    // survivors). For any mix: every candidate lands in EXACTLY ONE view by its `skip_reason` (Recognized →
    // items, ineligible → skipped), the walk-skips join `skipped`, the two views' ids are DISJOINT and cover
    // EXACTLY the contiguous `0..(N + M)`, and `item_paths` keys that same space.
    #[test]
    fn prop_freeze_partitions_a_mixed_intake_over_one_contiguous_id_space() {
        // 0 = Recognized (eligible); 1..=4 = the four ineligible verdicts.
        fn verdict(choice: u8) -> DetectionOutcome {
            match choice % 5 {
                0 => DetectionOutcome::Recognized {
                    format: UserFacingFormat::Csv,
                    confidence: Confidence::High,
                    dims: None,
                },
                1 => DetectionOutcome::Empty,
                2 => DetectionOutcome::Unreadable {
                    reason: ReadFailure::PermissionDenied,
                },
                3 => DetectionOutcome::UnsupportedType {
                    detected: "XYZ".to_string(),
                },
                _ => DetectionOutcome::Uncertain { best_guess: None },
            }
        }
        pinned_runner()
            .run(
                &(prop::collection::vec(0u8..5, 0..6usize), 0..4usize),
                |(choices, walk_skip_count)| {
                    let dir = tempfile::tempdir().expect("create a real temp dir");
                    let candidates: Vec<DetectedCandidate> = choices
                        .iter()
                        .enumerate()
                        .map(|(i, &choice)| {
                            let raw_path = dir.path().join(format!("f{i}.dat"));
                            std::fs::write(&raw_path, b"x")
                                .expect("write a distinct real temp source");
                            DetectedCandidate {
                                raw_path,
                                detected: verdict(choice),
                                size_bytes: 1,
                                rel_path_display: None,
                            }
                        })
                        .collect();
                    let walk_skips: Vec<WalkSkip> = (0..walk_skip_count)
                        .map(|j| WalkSkip {
                            path: dir.path().join(format!("skip{j}")),
                            reason: SkipReason::Unreadable,
                        })
                        .collect();
                    let eligible = choices.iter().filter(|&&c| c % 5 == 0).count();
                    let n = choices.len();
                    let m = walk_skip_count;
                    let snap = freeze_snapshot(candidates, walk_skips, vec![])
                        .expect("at most 10 ids — nowhere near the u32 ceiling, no exhaustion");
                    prop_assert_eq!(
                        snap.items.len(),
                        eligible,
                        "every Recognized candidate is an eligible member — nothing else"
                    );
                    prop_assert_eq!(
                        snap.skipped.len(),
                        (n - eligible) + m,
                        "every ineligible candidate + every walk-skip is a `Skipped`"
                    );
                    let member_ids: BTreeSet<ItemId> = snap.items.iter().map(|d| d.item).collect();
                    let skip_ids: BTreeSet<ItemId> = snap.skipped.iter().map(|s| s.item).collect();
                    prop_assert!(
                        member_ids.is_disjoint(&skip_ids),
                        "§0.6 inv-6: the eligible and skipped id views never collide (one shared space)"
                    );
                    let all: BTreeSet<ItemId> = member_ids.union(&skip_ids).copied().collect();
                    let expected: BTreeSet<ItemId> = (0..u32::try_from(n + m).expect("n + m < u32::MAX"))
                        .map(ItemId::from_index)
                        .collect();
                    prop_assert_eq!(
                        all,
                        expected.clone(),
                        "§0.6 inv-6: eligible ⊎ skipped = the contiguous 0..(N+M)"
                    );
                    prop_assert_eq!(
                        snap.item_paths.keys().copied().collect::<BTreeSet<_>>(),
                        expected,
                        "§0.4.4: item_paths keys EXACTLY the single id space (both views resolve)"
                    );
                    Ok(())
                },
            )
            .expect("the pinned 512-case exploration holds the §2.4.1 freeze partition + §0.6 invariant 6");
    }
}

#[cfg(test)]
mod dedup_tests {
    //! §6.4.1 unit (G15) for the §2.4.1 freeze-spine step-3 §2.3.2 resolved-identity de-dup fold
    //! ([`dedup_by_identity`], P2.76), driven with `FileIdentity` values built DIRECTLY (the IO/FFI
    //! `resolve_identity` that produces them is P3; test-strategy §0.1 — the thing under test is the pure
    //! de-dup + mint logic, not the filesystem).
    //!
    //! Exhaustion (`ItemSpaceExhausted`): the fold `?`-propagates it with NO fold-specific error logic — a
    //! bare `?`, no branch/cleanup (the partial `survivors` Vec is simply dropped on early return). The
    //! ceiling→`Err` behaviour is owned + unit-tested at its source, `ItemIdSpace::mint`
    //! (`domain::tests::item_id_space_reports_exhaustion_at_the_u32_ceiling`, P2.75); `ItemIdSpace` exposes
    //! no near-ceiling constructor, so a fold-level ceiling test would need a test-only seam into the P2.75
    //! type — deliberately NOT added (the successful `mint()?` path is exercised by every test below).
    use super::*;

    /// Build a `FileIdentity` tersely — mirrors fs_guard's own test helper (the fold keys on `(dev, inode)`,
    /// so the path is free to vary to model a hardlink).
    fn fid(path: &str, dev: u64, inode: u64) -> FileIdentity {
        FileIdentity {
            canonical_path: PathBuf::from(path),
            dev_or_volserial: dev,
            inode_or_fileindex: inode,
        }
    }

    /// §2.3.2 / §2.3.4: a HARDLINK is two paths over ONE `(dev, inode)` — same identity, different
    /// `canonical_path` — so the group collapses to ONE first-seen member (SSOT "converted once"), retaining
    /// the FIRST path, and only ONE id is minted. (Different paths + one identity also proves the key is the
    /// identity, not the path string.)
    #[test]
    fn hardlink_two_paths_collapse_to_one_first_seen_member() {
        let mut ids = ItemIdSpace::new();
        let candidates = vec![
            (fid("/data/photo.jpg", 66, 1234), "first"),
            (fid("/data/backup/photo-link.jpg", 66, 1234), "second"), // hardlink: same (dev, inode)
        ];
        let survivors = dedup_by_identity(candidates, &mut ids).expect("space not exhausted");
        assert_eq!(
            survivors.len(),
            1,
            "§2.3.2: two paths to one resolved file collapse to one member"
        );
        assert_eq!(
            survivors[0].identity.canonical_path,
            PathBuf::from("/data/photo.jpg"),
            "§2.3.2: the FIRST-seen path is the retained representative"
        );
        assert_eq!(
            survivors[0].id,
            ItemId::from_index(0),
            "§0.6 inv-6: the sole survivor gets id 0"
        );
    }

    /// §2.3.2 (the retention half, isolated): the same identity reached via path A THEN path B retains A —
    /// the first-seen `canonical_path` — not B.
    #[test]
    fn first_seen_representative_is_retained() {
        let mut ids = ItemIdSpace::new();
        let candidates = vec![
            (fid("/first-seen", 66, 7), "A"),
            (fid("/second-path", 66, 7), "B"),
        ];
        let survivors = dedup_by_identity(candidates, &mut ids).expect("space not exhausted");
        assert_eq!(survivors.len(), 1, "same identity → one member");
        assert_eq!(
            survivors[0].identity.canonical_path,
            PathBuf::from("/first-seen"),
            "§2.3.2: the retained representative is the FIRST-seen path, not the repeat sighting"
        );
    }

    /// §0.6 invariant 6: a dropped duplicate consumes NO id — `[A, dup(A), B]` yields 2 survivors with ids
    /// `[0, 1]` (B is 1, NOT 2), and the shared space's cursor advanced exactly twice (its next mint is 2),
    /// so the P3.49 skip ids continue at 2.
    #[test]
    fn duplicate_consumes_no_id() {
        let mut ids = ItemIdSpace::new();
        let candidates = vec![
            (fid("/a", 66, 1), "A"),
            (fid("/a-link", 66, 1), "dupA"), // duplicate of A (same identity)
            (fid("/b", 66, 2), "B"),
        ];
        let survivors = dedup_by_identity(candidates, &mut ids).expect("space not exhausted");
        let got: Vec<ItemId> = survivors.iter().map(|m| m.id).collect();
        assert_eq!(
            got,
            vec![ItemId::from_index(0), ItemId::from_index(1)],
            "§0.6 inv-6: B is id 1, not 2 — the duplicate consumed no id"
        );
        assert_eq!(
            ids.mint().expect("space not exhausted"),
            ItemId::from_index(2),
            "§0.6 inv-6: the shared space advanced exactly twice (skip ids continue at 2)"
        );
    }

    /// The fold preserves first-seen order — `[C, A, B]` (distinct, deliberately unsorted) yields survivors
    /// C, A, B with ids 0, 1, 2; the fold never reorders.
    #[test]
    fn order_preserving_over_survivors() {
        let mut ids = ItemIdSpace::new();
        let candidates = vec![
            (fid("/c", 66, 3), "C"),
            (fid("/a", 66, 1), "A"),
            (fid("/b", 66, 2), "B"),
        ];
        let survivors = dedup_by_identity(candidates, &mut ids).expect("space not exhausted");
        let payloads: Vec<&str> = survivors.iter().map(|m| m.payload).collect();
        assert_eq!(
            payloads,
            vec!["C", "A", "B"],
            "the fold preserves first-seen order, never sorts"
        );
        let got_ids: Vec<ItemId> = survivors.iter().map(|m| m.id).collect();
        assert_eq!(
            got_ids,
            (0u32..3).map(ItemId::from_index).collect::<Vec<_>>(),
            "§0.6 inv-6: ids are contiguous 0,1,2 in survivor order"
        );
    }

    /// §2.3.1: distinct identities all survive — including a same-INODE-different-DEV pair (proves `dev`
    /// disambiguates, no over-collapse; mirrors fs_guard's `same_inode_different_volume_is_distinct`). N
    /// distinct → N survivors, ids `0..N`.
    #[test]
    fn distinct_identities_all_survive() {
        let mut ids = ItemIdSpace::new();
        let candidates = vec![
            (fid("/x", 66, 1), "x"),
            (fid("/y", 66, 2), "y"),
            (fid("/z", 99, 1), "z"), // same inode 1 as /x but different dev → a DISTINCT file
        ];
        let survivors = dedup_by_identity(candidates, &mut ids).expect("space not exhausted");
        assert_eq!(
            survivors.len(),
            3,
            "§2.3.1: same inode across different volumes is NOT a duplicate (dev disambiguates)"
        );
        let got_ids: Vec<ItemId> = survivors.iter().map(|m| m.id).collect();
        assert_eq!(
            got_ids,
            (0u32..3).map(ItemId::from_index).collect::<Vec<_>>(),
            "§0.6 inv-6: three survivors get ids 0,1,2"
        );
    }

    /// An empty candidate list yields no survivors and does not touch the shared space (its next mint is
    /// still 0) — a dropped/cancelled/all-duplicate intake mints nothing.
    #[test]
    fn empty_input_yields_no_survivors() {
        let mut ids = ItemIdSpace::new();
        let survivors = dedup_by_identity(Vec::<(FileIdentity, &str)>::new(), &mut ids)
            .expect("space not exhausted");
        assert!(survivors.is_empty(), "no candidates → no survivors");
        assert_eq!(
            ids.mint().expect("space not exhausted"),
            ItemId::from_index(0),
            "§0.6 inv-6: an empty fold mints nothing — the space is untouched (next mint is 0)"
        );
    }
}

#[cfg(test)]
mod walk_tests {
    //! §6.4.1 unit (G15) for the §2.4.1 freeze-spine step-1 walk (P2.64 recursion + P2.65 hidden/system
    //! filter), run against a REAL temp filesystem (test-strategy §0.1: never mock the thing under test — the
    //! recursion + filtering ARE what these boxes prove, so they walk real directories, never an in-memory
    //! fake). Covers: a dropped file root, a flat dir, depth-first recursion across nested dirs, deterministic
    //! sorted order, an empty dir, multiple roots, the §1.1 hidden/system filter (dotfiles + the fixed
    //! sentinels filtered, a hidden directory not descended, the directly-dropped-root exemption — P2.65), the
    //! §1.1 per-item read-failure recording (a clean tree records no skips; (unix) a dangling symlink + an
    //! unreadable subdir → `Unreadable` `WalkSkip`, walk continues — P2.67), the §1.1 FATAL walk-root stop (a
    //! gone/unreadable dropped ROOT → `Err(WalkAbort::FatalRoot)`, distinct from the per-item skip; multi-root
    //! short-circuit — P2.68), the §1.1 cooperative-cancel poll (a tripped `CollectingId` token →
    //! `Err(WalkAbort::Cancelled)`, discarding the partial set — P2.69), and (unix) the
    //! symlinked-dir-not-traversed + symlinked-file-IS-a-candidate rules.
    //! [Build-Session-Entscheidung: P2.64/P2.65/P2.67/P2.68/P2.69]
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// Create an empty file at `dir/rel`, materialising any parent directories — the corpus builder for the
    /// real-FS walk tests.
    fn touch(dir: &std::path::Path, rel: &str) -> PathBuf {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(&p, b"").expect("write fixture file");
        p
    }

    /// Walk roots that are all readable, with a fresh (never-cancelled) token, unwrapping the
    /// `Result<IntakeWalk, WalkAbort>` — the success-path call the non-abort tests use so the abort arms
    /// (`FatalRoot` P2.68 / `Cancelled` P2.69) are asserted only by the tests that mean to.
    /// [Test-Change: P2.69 — old-obsolete+new-correct, §1.1] the prior single-arg
    /// `walk_intake_roots(roots).expect("walk succeeds …")` is obsolete because P2.69 added the `cancel`
    /// parameter; the new call passes a fresh un-cancelled token and the success assertion is UNCHANGED in
    /// meaning (readable roots + a never-cancelled token always complete — the suite stays green), so this
    /// adapts the signature, it does not relax the check.
    fn walk_ok(roots: &[PathBuf]) -> IntakeWalk {
        walk_intake_roots(roots, &CancellationToken::new())
            .expect("walk succeeds — every dropped root here is readable, token never cancelled")
    }

    fn file_names(paths: &[PathBuf]) -> Vec<std::ffi::OsString> {
        paths
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_owned()))
            .collect()
    }

    // §6.4.1: a dropped FILE root is a candidate directly (no folder to walk) — `walkdir` yields a file path
    // as a single entry (§1.1 "a dropped FILE is yielded directly").
    #[test]
    fn walk_yields_a_dropped_file_root_directly() {
        let tmp = tempdir().expect("tempdir");
        let file = touch(tmp.path(), "lonely.csv");
        let got = walk_ok(std::slice::from_ref(&file)).files;
        assert_eq!(
            got,
            vec![file],
            "§1.1: a dropped FILE root is a candidate directly, no recursion"
        );
    }

    // §6.4.1: a flat folder yields every file it directly contains.
    #[test]
    fn walk_yields_every_file_in_a_flat_dir() {
        let tmp = tempdir().expect("tempdir");
        let a = touch(tmp.path(), "a.csv");
        let b = touch(tmp.path(), "b.txt");
        let got = walk_ok(&[tmp.path().to_path_buf()]).files;
        assert!(got.contains(&a) && got.contains(&b), "both files collected");
        assert_eq!(got.len(), 2, "only the two files, no directory entries");
    }

    // §6.4.1/§1.9: recursion reaches files at every depth; directory entries themselves are NOT candidates;
    // and the GLOBAL cross-directory order is deterministic. Depth-first + per-directory sort means the `a/`
    // subtree (`a` < `top.csv`) is fully emitted before `top.csv`, and within it the `b/` subtree
    // (`b` < `mid.csv`) before `a/mid.csv` — so the exact sequence is `a/b/deep.csv`, `a/mid.csv`, `top.csv`.
    // Asserting the exact ordered vec pins the cross-directory interleaving, not just membership (closes the
    // flat-only-order gap). [Build-Session-Entscheidung: P2.64]
    #[test]
    fn walk_recurses_into_nested_directories_in_deterministic_global_order() {
        let tmp = tempdir().expect("tempdir");
        let root = tmp.path();
        let top = touch(root, "top.csv");
        let mid = touch(root, "a/mid.csv");
        let deep = touch(root, "a/b/deep.csv");
        let got = walk_ok(&[root.to_path_buf()]).files;
        assert_eq!(
            got,
            vec![deep, mid, top],
            "§1.9: depth-first, per-directory-sorted — a/b/deep.csv, then a/mid.csv, then top.csv; only the \
             3 files (directory entries are descended, not yielded)"
        );
    }

    // §1.9/§2.5: the walk order is DETERMINISTIC (sorted by file name) — reproducible across runs, never the
    // platform's filesystem readdir order. Two runs over the same tree give the identical, sorted order.
    #[test]
    fn walk_yields_a_deterministic_sorted_order() {
        let tmp = tempdir().expect("tempdir");
        let root = tmp.path();
        for name in ["zebra.csv", "alpha.csv", "mango.csv"] {
            touch(root, name);
        }
        let first = walk_ok(&[root.to_path_buf()]).files;
        let second = walk_ok(&[root.to_path_buf()]).files;
        assert_eq!(
            first, second,
            "§2.5: the walk order is reproducible across runs"
        );
        assert_eq!(
            file_names(&first),
            ["alpha.csv", "mango.csv", "zebra.csv"]
                .into_iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>(),
            "§1.9: a deterministic, file-name-sorted traversal order"
        );
    }

    // §6.4.1: an empty folder yields no candidates (not an error, just nothing).
    #[test]
    fn walk_of_an_empty_dir_yields_nothing() {
        let tmp = tempdir().expect("tempdir");
        let got = walk_ok(&[tmp.path().to_path_buf()]).files;
        assert!(got.is_empty(), "an empty folder yields no candidates");
    }

    // §1.1: candidates from EVERY dropped root are combined into the one frozen candidate list, in the
    // dropped-root INPUT order (root a's candidates, then root b's) — a deterministic combination, not just
    // a set. [Build-Session-Entscheidung: P2.64]
    #[test]
    fn walk_collects_across_multiple_roots_in_input_order() {
        let a = tempdir().expect("tempdir a");
        let b = tempdir().expect("tempdir b");
        let fa = touch(a.path(), "from_a.csv");
        let fb = touch(b.path(), "sub/from_b.csv");
        let got = walk_ok(&[a.path().to_path_buf(), b.path().to_path_buf()]).files;
        assert_eq!(
            got,
            vec![fa, fb],
            "§1.1: candidates from every root, combined in dropped-root input order (a before b)"
        );
    }

    // §1.1 (P2.66): the dropped root(s) are RETAINED verbatim on the walk result for §2.7 (relative-subtree
    // re-creation + the "open folder" common root). §2.7 owns the common-root computation; the walk carries
    // EVERY dropped root through, in input order, regardless of how many candidates it yielded — an empty
    // dropped folder is still retained (it anchors "open folder"). [Build-Session-Entscheidung: P2.66]
    #[test]
    fn walk_retains_every_dropped_root_verbatim_in_input_order() {
        let a = tempdir().expect("tempdir a");
        let b = tempdir().expect("tempdir b (empty - yields no candidates)");
        touch(a.path(), "x.csv");
        let roots = vec![a.path().to_path_buf(), b.path().to_path_buf()];
        let got = walk_ok(&roots);
        assert_eq!(got.files.len(), 1, "only a's one file is a candidate");
        assert_eq!(
            got.roots, roots,
            "§2.7: every dropped root retained verbatim in input order - including the EMPTY folder b (the \
             open-folder / subtree anchor is independent of yield)"
        );
    }

    // §1.1 (P2.66): a dropped FILE root is retained too - its containing folder is the §2.7 "open folder"
    // target; the retained root is the file path itself (verbatim), §2.7 derives the folder. The file is
    // ALSO a candidate (the P2.64 direct-file rule), so `files` and `roots` are distinct projections.
    #[test]
    fn walk_retains_a_dropped_file_root_verbatim() {
        let tmp = tempdir().expect("tempdir");
        let file = touch(tmp.path(), "lonely.csv");
        let got = walk_ok(std::slice::from_ref(&file));
        assert_eq!(
            got.files,
            vec![file.clone()],
            "§1.1: the dropped file is a candidate"
        );
        assert_eq!(
            got.roots,
            vec![file],
            "§2.7: the dropped FILE root is retained verbatim"
        );
    }

    // §1.1 (P2.65): a DISCOVERED hidden file (a dotfile) is now FILTERED from the walk — the §1.1 ignore
    // constant is active. The dotfile sits in `sub/` so it is a discovered entry (not the exempt root); a
    // normal sibling in the same dir is still collected, proving the filter is selective, not a blanket drop.
    // [Test-Change: P2.65 — old-obsolete+new-correct, §1.1] the P2.64 `walk_does_not_yet_filter_hidden_files`
    // expectation (a dotfile IS a candidate) is OBSOLETE: it deliberately pinned the P2.64<->P2.65 scope
    // boundary while the ignore filter was unbuilt. P2.65 activates the §1.1 fixed ignore constant, so the new
    // correct expectation — verified by reading back the walk result (the dotfile absent, the normal sibling
    // present) — is that a discovered dotfile is filtered.
    #[test]
    fn walk_filters_a_discovered_hidden_dotfile() {
        let tmp = tempdir().expect("tempdir");
        let dot = touch(tmp.path(), "sub/.hidden.csv");
        let normal = touch(tmp.path(), "sub/data.csv");
        let got = walk_ok(&[tmp.path().to_path_buf()]).files;
        assert!(
            !got.contains(&dot),
            "§1.1: a discovered dotfile is filtered (the P2.65 ignore constant)"
        );
        assert!(
            got.contains(&normal),
            "a normal sibling file is still collected"
        );
    }

    // §1.1 (P2.65): the fixed NON-dotfile platform sentinels (`.DS_Store` is also a dotfile, but
    // `Thumbs.db`/`desktop.ini` are not) are filtered by NAME, case-insensitively; a normal file is kept.
    #[test]
    fn walk_filters_the_platform_sentinels() {
        let tmp = tempdir().expect("tempdir");
        let thumbs = touch(tmp.path(), "Thumbs.db");
        let desktop = touch(tmp.path(), "DESKTOP.INI"); // case-insensitive match
        let ds_store = touch(tmp.path(), ".DS_Store");
        let keep = touch(tmp.path(), "report.csv");
        let got = walk_ok(&[tmp.path().to_path_buf()]).files;
        for junk in [&thumbs, &desktop, &ds_store] {
            assert!(
                !got.contains(junk),
                "§1.1: the platform sentinel is filtered: {junk:?}"
            );
        }
        assert_eq!(
            got,
            vec![keep],
            "only the normal file survives the §1.1 sentinel filter"
        );
    }

    // §1.1 (P2.65): a hidden DIRECTORY is PRUNED — `filter_entry` declines to descend it, so its contents
    // never reach the candidate list (a hidden dir like `.git` is junk, never walked); a normal dir IS walked.
    #[test]
    fn walk_does_not_descend_a_hidden_directory() {
        let tmp = tempdir().expect("tempdir");
        let buried = touch(tmp.path(), ".git/config.csv"); // inside a hidden dir — must be pruned
        let visible = touch(tmp.path(), "data/keep.csv");
        let got = walk_ok(&[tmp.path().to_path_buf()]).files;
        assert!(
            !got.contains(&buried),
            "§1.1: a hidden directory is not descended, so its files never become candidates"
        );
        assert_eq!(
            got,
            vec![visible],
            "the file under the normal directory is collected"
        );
    }

    // §1.1 (P2.65): the dropped ROOT is EXEMPT from the ignore filter — a user who explicitly drops a hidden
    // file gets it converted (the ignore-list governs RECURSION, not the explicit choice). depth()==0 exempt.
    // [Build-Session-Entscheidung: P2.65]
    #[test]
    fn walk_keeps_a_directly_dropped_hidden_file_root() {
        let tmp = tempdir().expect("tempdir");
        let dropped = touch(tmp.path(), ".hidden.csv");
        let got = walk_ok(std::slice::from_ref(&dropped)).files;
        assert_eq!(
            got,
            vec![dropped],
            "§1.1: a directly-dropped hidden file ROOT is kept (the user chose it; depth-0 exemption)"
        );
    }

    // §1.1 (P2.65): the depth-0 ROOT exemption applies to a directly-dropped hidden DIRECTORY too — a user who
    // drops `.hidden_dir/` explicitly gets its (non-hidden) contents walked; the ignore-list governs the
    // DISCOVERED entries inside, not the explicit root choice. Completes the file-OR-folder root exemption.
    // [Build-Session-Entscheidung: P2.65]
    #[test]
    fn walk_descends_a_directly_dropped_hidden_dir_root() {
        let tmp = tempdir().expect("tempdir");
        let hidden_root = tmp.path().join(".hidden_dir");
        let keep = touch(&hidden_root, "keep.csv");
        let got = walk_ok(std::slice::from_ref(&hidden_root)).files;
        assert_eq!(
            got,
            vec![keep],
            "§1.1: a directly-dropped hidden DIRECTORY root is descended (depth-0 exemption); its normal \
             contents are walked"
        );
    }

    // §6.4.1: the pure §1.1 name classifier (P2.65) — dotfiles + the fixed sentinels (case-insensitive) are
    // hidden; a normal name is not. Cross-platform (the Windows file-ATTRIBUTE leg is thin platform glue,
    // exercised on a real Windows host).
    #[test]
    fn name_is_hidden_or_sentinel_classifies_dotfiles_and_sentinels() {
        use std::ffi::OsStr;
        for hidden in [
            ".hidden.csv",
            ".DS_Store",
            ".git",
            "Thumbs.db",
            "thumbs.DB",
            "desktop.ini",
        ] {
            assert!(
                name_is_hidden_or_sentinel(OsStr::new(hidden)),
                "§1.1: `{hidden}` is hidden/system"
            );
        }
        for visible in ["report.csv", "data.txt", "thumbs.csv", "my.desktop.ini.csv"] {
            assert!(
                !name_is_hidden_or_sentinel(OsStr::new(visible)),
                "§1.1: `{visible}` is a normal name"
            );
        }
    }

    // §1.1 (unix): a symlinked DIRECTORY is NOT traversed (loop-safety, T7), while a symlinked FILE IS a
    // §2.3-resolvable candidate. The symlinked dir points OUTSIDE the walked tree, so its target's file is
    // reachable ONLY via the link — if the walk descended it, `secret.csv` would appear; it must not.
    #[cfg(unix)]
    #[test]
    fn walk_skips_a_symlinked_dir_but_keeps_a_symlinked_file() {
        use std::os::unix::fs::symlink;
        let tmp = tempdir().expect("walked-root tempdir");
        let ext = tempdir().expect("symlink-target tempdir (outside the walk)");
        let root = tmp.path();

        let nested = touch(root, "dir/a.csv"); // a real file — must be found
        fs::write(ext.path().join("secret.csv"), b"").expect("write the out-of-tree target");
        symlink(ext.path(), root.join("dir/link_to_ext")).expect("symlink a dir → ext");
        let file_target = touch(root, "files/real.csv");
        symlink(&file_target, root.join("dir/link_to_file.csv")).expect("symlink a file");

        let got = walk_ok(&[root.to_path_buf()]).files;

        assert!(got.contains(&nested), "the real nested file is walked");
        assert!(
            !got.iter().any(|p| p.ends_with("secret.csv")),
            "§1.1/T7: a symlinked directory is NOT traversed — its target's files stay unreachable via the link"
        );
        assert!(
            file_names(&got).contains(&std::ffi::OsString::from("link_to_file.csv")),
            "§1.1: a symlinked FILE IS a candidate (file-level aliasing resolved by §2.3 later)"
        );
    }

    // §1.1 (P2.67, unix): a DANGLING symlink (its target stat fails — "a file that vanished") is RECORDED as
    // an `Unreadable` skip and the walk CONTINUES; it is NOT a candidate, and the good sibling is still
    // collected.
    // [Test-Change: P2.67 — old-obsolete+new-correct, §1.1] the P2.64 expectation (the dangling symlink
    // silently excluded, only `.files` asserted) is OBSOLETE: P2.67 changes the behaviour from a silent skip to
    // a RECORDED `Unreadable` skip. The new expectation — `.files` still excludes it (preserved) AND `.skipped`
    // records it as `Unreadable` — is verified by reading back the walk result.
    #[cfg(unix)]
    #[test]
    fn walk_records_a_dangling_symlink_as_unreadable_and_continues() {
        use std::os::unix::fs::symlink;
        let tmp = tempdir().expect("tempdir");
        let root = tmp.path();
        let good = touch(root, "good.csv");
        let dangling = root.join("dangling.csv");
        symlink(root.join("nonexistent-target"), root.join("dangling.csv"))
            .expect("dangling symlink");
        let result = walk_ok(&[root.to_path_buf()]);
        assert_eq!(
            result.files,
            vec![good],
            "the dangling symlink is not a candidate; the good file is still collected (the walk continued)"
        );
        assert_eq!(
            result.skipped,
            vec![WalkSkip {
                path: dangling,
                reason: SkipReason::Unreadable,
            }],
            "§1.1/P2.67: a dangling symlink (its target stat fails) is RECORDED as an Unreadable skip, not \
             silently dropped"
        );
    }

    // §6.4.1 (P2.67): a clean, fully-readable tree records NO skips — `skipped` is empty when every entry is
    // readable (the recording fires only on a real per-item read failure, never on ordinary entries).
    #[test]
    fn walk_of_a_clean_tree_records_no_skips() {
        let tmp = tempdir().expect("tempdir");
        touch(tmp.path(), "a.csv");
        touch(tmp.path(), "sub/b.csv");
        let result = walk_ok(&[tmp.path().to_path_buf()]);
        assert!(
            result.skipped.is_empty(),
            "a fully-readable tree records no Unreadable skips"
        );
        assert_eq!(result.files.len(), 2, "both readable files collected");
    }

    // §1.1 (P2.67, unix): an unreadable DISCOVERED subdirectory (a `walkdir` traversal error at depth > 0) is
    // recorded as an `Unreadable` skip and the walk CONTINUES collecting other entries — a denied folder never
    // sinks the whole ingest. Skipped when the process can read it anyway (running as root, where DAC is
    // bypassed) rather than asserting a condition the environment cannot produce.
    #[cfg(unix)]
    #[test]
    fn walk_records_an_unreadable_subdir_and_continues() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempdir().expect("tempdir");
        let root = tmp.path();
        let ok = touch(root, "ok.csv");
        let denied = root.join("denied");
        fs::create_dir(&denied).expect("mkdir denied");
        touch(&denied, "secret.csv");
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o000)).expect("chmod 000");
        let can_read_anyway = fs::read_dir(&denied).is_ok();
        let result = walk_ok(&[root.to_path_buf()]);
        // restore perms so the tempdir cleanup can remove the subtree
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o755)).ok();
        if can_read_anyway {
            return; // DAC bypassed (e.g. root) — the denied-read path cannot be exercised here
        }
        assert!(
            result.files.contains(&ok),
            "the readable sibling is still collected — the walk continued past the denied subdir"
        );
        assert!(
            !result.files.iter().any(|p| p.ends_with("secret.csv")),
            "the unreadable subdir's content is not collected"
        );
        assert!(
            result
                .skipped
                .iter()
                .any(|skip| skip.reason == SkipReason::Unreadable && skip.path.ends_with("denied")),
            "§1.1/P2.67: an unreadable discovered subdir is recorded as Unreadable, not silently dropped"
        );
    }

    // §1.1 (P2.68): a dropped ROOT that is GONE (does not exist) is a FATAL walk-root error — the walk STOPS
    // with `Err(FatalWalkRoot)`, NOT a per-item skip (which would continue and return `Ok`). The carried
    // `root` is the offending dropped root verbatim and `cause` is `NotFound` ("gone") so the §1.1
    // fatal-ingest message (P3.49) can say the dropped folder/file is gone.
    #[test]
    fn walk_stops_fatally_when_a_dropped_root_is_gone() {
        let tmp = tempdir().expect("tempdir");
        let gone = tmp.path().join("never-existed");
        assert_eq!(
            walk_intake_roots(std::slice::from_ref(&gone), &CancellationToken::new()).err(),
            Some(WalkAbort::FatalRoot(FatalWalkRoot {
                root: gone,
                cause: ReadFailure::NotFound,
            })),
            "§1.1/P2.68: a gone dropped root STOPS the walk fatally (NotFound) — an Err, never an Ok with a \
             per-item skip"
        );
    }

    // §1.1 (P2.68): across MULTIPLE dropped roots the FIRST fatal root (input order) STOPS the whole walk and
    // the candidates already collected from an earlier readable root are DISCARDED — the `Err` is the abort,
    // never a partial `Ok`. This is the sharp contrast with the P2.67 per-item skip (which returns `Ok` + a
    // skipped row): a bad FILE never sinks the ingest, a bad ROOT does.
    #[test]
    fn a_fatal_root_stops_the_whole_walk_discarding_earlier_candidates() {
        let readable = tempdir().expect("readable root tempdir");
        touch(readable.path(), "kept.csv");
        let gone = readable.path().join("never-existed");
        let roots = vec![readable.path().to_path_buf(), gone.clone()];
        // The whole `Result` is `Err` — NOT an `Ok` carrying the readable root's `kept.csv` (which is what a
        // per-item skip would leave). The fatal root (b) is reported; the earlier root's candidates discarded.
        assert_eq!(
            walk_intake_roots(&roots, &CancellationToken::new()).err(),
            Some(WalkAbort::FatalRoot(FatalWalkRoot {
                root: gone,
                cause: ReadFailure::NotFound,
            })),
            "§1.1/P2.68: the first fatal root STOPS the whole walk — an Err, not a partial Ok with the \
             readable root's candidates (a per-item skip would have continued)"
        );
    }

    // §1.1 (P2.68, unix): a dropped ROOT directory that cannot be LISTED (chmod 000) is a FATAL walk-root
    // error (`PermissionDenied`) that STOPS the walk — unlike the P2.67 unreadable DISCOVERED subdir
    // (depth > 0), which is a per-item skip that continues. Skipped when the process can read it anyway
    // (running as root, where DAC is bypassed) rather than asserting a condition the environment cannot
    // produce — the same guard the P2.67 unreadable-subdir test uses.
    #[cfg(unix)]
    #[test]
    fn walk_stops_fatally_when_a_dropped_root_is_unreadable() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempdir().expect("tempdir");
        let denied = tmp.path().join("denied_root");
        fs::create_dir(&denied).expect("mkdir denied_root");
        touch(&denied, "inside.csv");
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o000)).expect("chmod 000");
        let can_read_anyway = fs::read_dir(&denied).is_ok();
        let result = walk_intake_roots(std::slice::from_ref(&denied), &CancellationToken::new());
        // restore perms so the tempdir cleanup can remove the subtree
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o755)).ok();
        if can_read_anyway {
            return; // DAC bypassed (e.g. root) — the denied-read path cannot be exercised here
        }
        assert_eq!(
            result.err(),
            Some(WalkAbort::FatalRoot(FatalWalkRoot {
                root: denied,
                cause: ReadFailure::PermissionDenied,
            })),
            "§1.1/P2.68: an unreadable dropped ROOT stops the walk fatally (PermissionDenied), not a per-item \
             skip"
        );
    }

    // §1.1 (P2.69): the walk polls the ingest-scoped `CollectingId` token and STOPS cooperatively when C13
    // `cancel_ingest` has tripped it — returning `WalkAbort::Cancelled` and discarding the partial,
    // not-yet-frozen set (no cleanup obligation, nothing is written during the walk). Pre-cancelling the
    // token exercises the poll deterministically (it trips on the first entry); the contrast call with a
    // fresh token over the SAME populated dir proves the token IS the cause — that dir collects both files
    // normally, so the cancelled run's empty Err is the poll discarding the set, not an empty dir.
    #[test]
    fn a_cancelled_ingest_token_stops_the_walk_and_discards_the_partial_set() {
        let tmp = tempdir().expect("tempdir");
        touch(tmp.path(), "a.csv");
        touch(tmp.path(), "b.csv");
        let root = [tmp.path().to_path_buf()];

        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert_eq!(
            walk_intake_roots(&root, &cancelled).err(),
            Some(WalkAbort::Cancelled),
            "§1.1/P2.69: a tripped CollectingId token stops the walk (Cancelled) and discards the partial set \
             — an Err, never an Ok with a partial candidate list"
        );

        // Contrast: the SAME populated dir with a fresh (un-cancelled) token completes with both files — so
        // the cancelled run above stopped because of the poll, not because the dir was empty.
        let fresh = CancellationToken::new();
        let walk = walk_intake_roots(&root, &fresh)
            .expect("a fresh token never trips — the walk completes");
        assert_eq!(
            walk.files.len(),
            2,
            "§1.1/P2.69: an un-cancelled token collects normally (the poll does not false-trip)"
        );
    }

    // §6.4.1 unit (P2.67): the shared `record_unreadable` recorder's OBSERVABLE contract, driven directly —
    // a present path is RECORDED as a §1.1 `Unreadable` `WalkSkip` (kept for the §1.4 summary, never
    // silently dropped) and a `None` path (a walkdir error with no associated path) records NOTHING (no
    // item to attribute). Both walk arms that call it are exercised on a real filesystem above; this pins
    // the push itself, so an accidentally-inert recorder is caught on every platform.
    // [Build-Session-Entscheidung: P2.137]
    #[test]
    fn record_unreadable_records_a_present_path_and_drops_a_pathless_error() {
        let mut skipped: Vec<WalkSkip> = Vec::new();
        record_unreadable(&mut skipped, Some(std::path::Path::new("locked.csv")));
        assert_eq!(
            skipped,
            vec![WalkSkip {
                path: PathBuf::from("locked.csv"),
                reason: SkipReason::Unreadable,
            }],
            "§1.1/P2.67: a per-item read failure is RECORDED as an Unreadable skip"
        );
        record_unreadable(&mut skipped, None);
        assert_eq!(
            skipped.len(),
            1,
            "§1.1/P2.67: a path-less walkdir error has no item to attribute — nothing is recorded"
        );
    }

    // §1.1 (P2.68, windows): the PermissionDenied FATAL-root mapping on Windows — a dropped ROOT directory
    // whose LISTING is ACL-denied stops the walk fatally with `ReadFailure::PermissionDenied`, never the
    // `IoError` fallback (the unix sibling P2.68 unreadable-root test proves
    // the same arm via a chmod-000 root). std exposes no ACL editing and subprocess use is confined to
    // `crate::isolation` (G29 rule (c)), so the test walks the volume's `System Volume Information`
    // directory — present on every NTFS system drive and list-denied to every non-SYSTEM principal by
    // default — GUARDED like the unix DAC-bypass guard: it asserts only when the probe read actually
    // reports PermissionDenied (a SYSTEM-account runner, or a drive without the folder, cannot produce the
    // condition and returns early). [Build-Session-Entscheidung: P2.137]
    #[cfg(windows)]
    #[test]
    fn walk_stops_fatally_with_permission_denied_on_a_list_denied_root() {
        let denied = PathBuf::from(r"C:\System Volume Information");
        match fs::read_dir(&denied) {
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {}
            Ok(_) | Err(_) => return, // privileged runner / absent folder — the condition is not producible
        }
        assert_eq!(
            walk_intake_roots(std::slice::from_ref(&denied), &CancellationToken::new()).err(),
            Some(WalkAbort::FatalRoot(FatalWalkRoot {
                root: denied,
                cause: ReadFailure::PermissionDenied,
            })),
            "§1.1/P2.68: a list-denied dropped ROOT maps io PermissionDenied → \
             ReadFailure::PermissionDenied (the specific arm, never the IoError fallback)"
        );
    }

    // §1.1 (P2.65, windows): the Windows hidden/system file-ATTRIBUTE leg of the ignore filter — a TRUTH
    // TABLE over the real FILE_ATTRIBUTE_HIDDEN (0x2) / FILE_ATTRIBUTE_SYSTEM (0x4) bits (the constants
    // `windows_attr_hidden` tests), on real files created WITH those attributes via std's
    // `OpenOptionsExt::attributes`: neither → false (a plain new file carries OTHER bits, e.g. ARCHIVE,
    // which must not trip the mask), hidden-only → true, system-only → true, both → true. Each row kills a
    // distinct fault class: constant-true (the neither row), constant-false (the hidden/system rows), and a
    // broken HIDDEN|SYSTEM mask (a zeroed mask fails the single-bit rows).
    // [Build-Session-Entscheidung: P2.137]
    #[cfg(windows)]
    #[test]
    fn windows_attr_hidden_truth_table_over_the_attribute_bits() {
        use std::os::windows::fs::OpenOptionsExt;
        // Mirrors the fn-local constants of `windows_attr_hidden` (§1.1/P2.65).
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x0000_0002;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x0000_0004;
        let tmp = tempdir().expect("tempdir");
        let entry_for = |name: &str, attrs: u32| -> walkdir::DirEntry {
            let p = tmp.path().join(name);
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            if attrs != 0 {
                options.attributes(attrs);
            }
            options
                .open(&p)
                .expect("create the attributed fixture file");
            WalkDir::new(&p)
                .into_iter()
                .next()
                .expect("a file root yields exactly one entry")
                .expect("the fixture entry is readable")
        };
        let rows: [(&str, u32, bool); 4] = [
            ("plain.csv", 0, false),
            ("hidden.csv", FILE_ATTRIBUTE_HIDDEN, true),
            ("system.csv", FILE_ATTRIBUTE_SYSTEM, true),
            (
                "both.csv",
                FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM,
                true,
            ),
        ];
        for (name, attrs, expected) in rows {
            assert_eq!(
                windows_attr_hidden(&entry_for(name, attrs)),
                expected,
                "§1.1/P2.65: attribute bits {attrs:#06x} on `{name}` classify as hidden/system = {expected}"
            );
        }
    }
}

// ─── §1.1/§2.4 freeze idle-vs-in-flight GATING CONTRACT (P2.72) ────────────────────────────────────────
// The §2.4 freeze-gating contract is **upstream-delegated, not a core-side wrapper** (Reading B — the
// Co-Pilot 2026-06-30 scope DECISION on P2.72): §7.1.1 names exactly TWO refuse-busy layers — the PRIMARY
// `forward_launch_intake` funnel (P2.55, which DROPS a mid-run launch-intake before any freeze) + the §5.8
// UI defence-in-depth — so the orchestrator freeze (`ingest`) carries NO core-side busy gate; a third one
// would be over-build (and would conflate "busy" with "nothing", both projecting to `CollectedSet::Empty`).
// This module ASSERTS that contract from the orchestrator side; the delegation-DOC half is the `ingest` /
// `CollectedSetRegistry::register` doc-comments. Its three legs + the structural Reading-B anchor:
//   1. IDLE → a freeze starts a NEW set: `CollectedSetRegistry::register` SUPERSEDES the prior un-run set
//      (§2.4.3: a subsequent drop starts a new frozen set, never mutating an in-flight one; §0.4.4).
//   2. NEVER mutate/merge: a second freeze REPLACES — at most one live set, the new set's content is the
//      freeze's OWN, never a merge of the prior (§2.4.3, structural via `register`'s clear-then-insert).
//   3. The busy launch-intake is refused UPSTREAM (this freeze is never reached): the §7.1.1 PRIMARY rule
//      `crate::launch_intake::intake_disposition` returns `Drop` for a busy converter in EVERY readiness
//      state — no emit, no buffer — so no UI re-call / drain ever routes paths back into this freeze.
//   + STRUCTURAL Reading-B anchor: `ingest`'s signature takes only `paths`+`origin` — no run-state / `busy`
//     parameter — so a core-side freeze gate is impossible by construction (a drift would fail to compile).
// [Build-Session-Entscheidung: P2.72] A DEDICATED, self-contained contract module (its own minimal id /
// `frozen` helpers mirror the `crate::orchestrator::tests` registry helpers) so the §2.4 freeze-gating
// contract reads standalone and its name is a stable, rename-safe MODULE anchor for the `ingest` /
// P8.1.1 delegation doc-comments (the .rs-comment module-anchoring convention).
#[cfg(test)]
mod freeze_gating_contract {
    use super::*;
    use crate::domain::InstanceId;

    /// A `CollectedSetId` from its public bare-uuid `Deserialize` wire form (the inner `Uuid` is private to
    /// `crate::domain`; minting is §1.1/§7.1's, not a back-door constructor) — the
    /// `crate::orchestrator::tests` id-helper precedent, kept local so this contract module is self-contained.
    /// [Build-Session-Entscheidung: P2.72]
    fn set_id(uuid: &str) -> CollectedSetId {
        serde_json::from_str(&format!("\"{uuid}\""))
            .expect("CollectedSetId deserializes from a uuid string")
    }
    fn instance() -> InstanceId {
        serde_json::from_str(r#""44444444-4444-4444-8444-444444444444""#)
            .expect("InstanceId deserializes from a uuid string")
    }
    /// A minimal `RegisteredSet` carrying `id` + a content-distinguishing `count`/`total_bytes` on its inner
    /// frozen set, so the never-merge leg asserts the resolved latest set is the freeze's OWN content (not a
    /// merge of a prior). The `identities` table is empty here — these legs exercise the id/supersede
    /// lifecycle, not the P3.40 identity evidence (that is `rerun_verdict_tests`).
    fn frozen(id: CollectedSetId, count: usize, total_bytes: u64) -> RegisteredSet {
        RegisteredSet {
            frozen: FrozenCollectedSet {
                id,
                instance: instance(),
                format: UserFacingFormat::Csv,
                items: vec![],
                count,
                skipped: vec![],
                total_bytes,
                roots: vec![],
                encoding_hint: None,
                delimiter_hint: None,
                notes: vec![],
                item_paths: BTreeMap::new(),
            },
            identities: BTreeMap::new(),
        }
    }

    // Leg 1 — §6.4.1 unit (G15): an IDLE freeze starts a NEW set. A second `register` (a subsequent idle drop,
    // §1.1) SUPERSEDES the prior un-run set, never mutating it (§2.4.3 / §0.4.4): the superseded id no longer
    // resolves; the latest freeze is the one live set.
    #[test]
    fn idle_freeze_supersedes_the_prior_un_run_set() {
        let reg = CollectedSetRegistry::default();
        let prior = set_id("11111111-1111-4111-8111-111111111111");
        let next = set_id("22222222-2222-4222-8222-222222222222");
        reg.register(frozen(prior, 3, 30));
        reg.register(frozen(next, 5, 50));
        assert!(
            reg.resolve(prior).is_none(),
            "§1.1/§2.4.3: an idle freeze starts a NEW frozen set — the prior un-run set is superseded, never mutated"
        );
        assert_eq!(
            reg.resolve(next).map(|s| s.frozen.count),
            Some(5),
            "§0.4.4: the latest freeze is the one live set (at most one un-run set)"
        );
    }

    // Leg 2 — §6.4.1 unit (G15): a freeze REPLACES, it never merges. The resolved latest set is the freeze's
    // OWN content (count/total_bytes), never a merge of a prior set; and a same-id re-freeze replaces rather
    // than accumulates (§2.4.3, structural via `register`'s clear-then-insert).
    #[test]
    fn freeze_replaces_content_never_merges() {
        let reg = CollectedSetRegistry::default();
        let a = set_id("11111111-1111-4111-8111-111111111111");
        let b = set_id("22222222-2222-4222-8222-222222222222");
        reg.register(frozen(a, 3, 30));
        reg.register(frozen(b, 5, 50));
        let live = reg.resolve(b).expect("the latest freeze resolves");
        assert_eq!(
            (live.frozen.count, live.frozen.total_bytes),
            (5, 50),
            "§2.4.3: the new frozen set is the freeze's OWN content — never a merge of the prior set (a merge would be count 8 / bytes 80)"
        );
        // A same-id re-freeze (a re-drop minting the same logical id) REPLACES the snapshot, never accumulates.
        reg.register(frozen(b, 7, 70));
        assert_eq!(
            reg.resolve(b).map(|s| (s.frozen.count, s.frozen.total_bytes)),
            Some((7, 70)),
            "§2.4.3: a re-freeze of the same id replaces the snapshot (clear-then-insert), never accumulates onto it"
        );
    }

    // Leg 3 — §6.4.1 unit (G15): the busy launch-intake is refused UPSTREAM, so this freeze is never reached.
    // The §7.1.1 PRIMARY rule `intake_disposition` (the funnel reads it, P2.55) returns `Drop` for a busy
    // converter — `Drop` neither stashes into `PendingIntake` nor nudges `app://intake`, so the intake never
    // routes paths back into the orchestrator freeze (`ingest`) mid-run. This asserts the DELEGATION SEAM
    // (refuse-busy is upstream, not in the freeze); the full disposition truth table is `crate::launch_intake::tests`
    // (the pure rule's home). [Build-Session-Entscheidung: P2.72] The contract test reaches the `pub(crate)`
    // upstream rule so the "freeze never reached" delegation is asserted end-to-end, not only documented.
    // the readiness loop is obsolete — `intake_disposition` no longer takes `frontend_ready` (the nudge is a
    // separate post-stash read, P3.77), so busy → Drop depends on `busy` alone; the delegation-seam assertion
    // is the surviving invariant. [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1]
    #[test]
    fn busy_launch_intake_is_refused_upstream_so_the_freeze_is_never_reached() {
        use crate::launch_intake::{intake_disposition, IntakeDisposition};
        assert_eq!(
            intake_disposition(true),
            IntakeDisposition::Drop,
            "§7.1.1/§2.4: a busy converter DROPS the launch-intake upstream — no stash, no nudge, so the orchestrator freeze is never reached mid-run"
        );
    }

    // Structural Reading-B anchor — §6.4.1 unit (G15): there is NO core-side freeze gate, BY CONSTRUCTION.
    // `ingest`'s signature takes only the drop inputs (paths + origin) + the §1.1 walk plumbing (the ingest
    // cancel token, the §0.4.2 scan Channel, the app instance); it carries NO run-state / `busy` / `&RunRegistry`
    // parameter, so it CANNOT refuse-busy — the refusal is necessarily upstream (Leg 3). A drift that bolted a
    // core-side gate onto the freeze (an added `busy` / `&RunRegistry` parameter) would fail this fn-pointer
    // coercion to compile — the signature pin is the structural guard the doc-comments delegate to.
    // [Test-Change: P3.49 — old-obsolete+new-correct, §1.1] the pin grows to the P3.49 walk signature (the
    // ingest cancel token / `on_scan` Channel / `instance` + the `IngestResult` return); the "no busy /
    // &RunRegistry param" assertion is preserved (the added params are walk plumbing, not a run-state gate).
    #[test]
    fn ingest_freeze_carries_no_core_side_busy_gate() {
        let _freeze: fn(
            Vec<PathBuf>,
            IntakeOrigin,
            &CancellationToken,
            &Channel<ScanProgress>,
            InstanceId,
        ) -> IngestResult = ingest;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        Availability, Confidence, DetectionOutcome, DivertReason, InstanceId, RerunPrompt, TargetId,
    };
    use proptest::prelude::*;
    use proptest::test_runner::{RngAlgorithm, TestRng, TestRunner};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use tauri::ipc::InvokeResponseBody;

    // The §0.6 id newtypes' inner fields are PRIVATE to `crate::domain` and their MINTING is owned by
    // §1.1/§7.1 (P2.75 assigns `ItemId` at the freeze; the ids are not minted here) — so these
    // cross-module tests construct the ids via their PUBLIC `Deserialize` wire contract (each is a
    // transparent newtype: `ItemId` from a bare number, `CollectedSetId` from a uuid string), NOT a
    // back-door constructor that would pre-empt the minting policy. [Build-Session-Entscheidung: P2.10]
    fn item_id(n: u32) -> ItemId {
        serde_json::from_str(&n.to_string())
            .expect("ItemId deserializes from its bare-number wire form")
    }
    fn collected_set_id() -> CollectedSetId {
        serde_json::from_str(r#""00000000-0000-4000-8000-000000000000""#)
            .expect("CollectedSetId deserializes from a uuid string")
    }
    fn run_id() -> RunId {
        serde_json::from_str(r#""11111111-1111-4111-8111-111111111111""#)
            .expect("RunId deserializes from a uuid string")
    }
    /// A second, distinct `RunId` — for the §0.4.4 independent-token registry test (two live runs).
    fn run_id_other() -> RunId {
        serde_json::from_str(r#""22222222-2222-4222-8222-222222222222""#)
            .expect("RunId deserializes from a uuid string")
    }

    /// A minimal eligible source item of the given §1.2 recognized `format` — the format-generative sibling
    /// of `dropped_item` for the P2.137 grouping property. [Build-Session-Entscheidung: P2.137]
    fn dropped_item_with(id: u32, format: UserFacingFormat) -> DroppedItem {
        DroppedItem {
            item: item_id(id),
            display_name: "data.csv".to_string(),
            rel_path_display: None,
            size_bytes: 12,
            detected: DetectionOutcome::Recognized {
                format,
                confidence: Confidence::High,
                dims: None,
            },
        }
    }

    /// A minimal eligible CSV source item, for the job/batch shape tests.
    fn dropped_item(id: u32) -> DroppedItem {
        dropped_item_with(id, UserFacingFormat::Csv)
    }

    /// A minimal pre-flight-skipped source item at `id` with the given §0.6 `SkipReason` — the skip-arm
    /// sibling of `dropped_item`, for the `JobSource::Skipped` fixtures (P3.47). `detected_display` defaults
    /// to `None`; [`skipped_item_detected`] sets it for the P3.50 skip-message projection tests.
    fn skipped_item(id: u32, reason: SkipReason) -> SkippedItem {
        SkippedItem {
            item: item_id(id),
            source_display: "skipped.bin".to_string(),
            detected_display: None,
            reason,
        }
    }

    /// A pre-flight-skipped source item carrying a retained `detected_display` (§0.6, P3.50) — for the §1.12
    /// skip-message projection tests where the retained detected-type name feeds the §2.8.2 `{detected}` slot.
    fn skipped_item_detected(id: u32, reason: SkipReason, detected: Option<&str>) -> SkippedItem {
        SkippedItem {
            item: item_id(id),
            source_display: "skipped.bin".to_string(),
            detected_display: detected.map(str::to_owned),
            reason,
        }
    }

    /// A minimal CSV→TSV target, for the batch shape test.
    fn sample_target() -> Target {
        Target {
            id: TargetId::Format(UserFacingFormat::Tsv),
            label: "TSV".to_owned(),
            lossy: None,
            availability: Availability::Available,
            options: vec![],
        }
    }

    // ─── P3.46 §1.9 lifecycle FSM + Running→Failed projection tests ──

    /// A `ConversionJob` in the given §1.9 [`JobState`], for the FSM + queue-order tests.
    /// [Test-Change: P3.47 — old-obsolete+new-correct, §0.6] `source` is now the `JobSource` sum, not a bare
    /// `DroppedItem` (the old field form no longer compiles). The fixture mirrors the §0.6 coupling invariant
    /// — a `Skipped` state gets the `JobSource::Skipped` arm (its `SkipReason` matching the state), every
    /// other state the `Eligible` arm — so each `job_in` job is well-formed, and denormalizes `item` via the
    /// new `source.item()` accessor. The exhaustive match (no `_`, the crate `wildcard_enum_match_arm` deny)
    /// forces a conscious arm for a future `JobState` variant.
    fn job_in(id: u32, state: JobState) -> ConversionJob {
        let source = match state {
            JobState::Skipped(reason) => JobSource::Skipped(skipped_item(id, reason)),
            JobState::Pending
            | JobState::Running
            | JobState::Succeeded
            | JobState::Failed(_)
            | JobState::Cancelled => JobSource::Eligible(dropped_item(id)),
        };
        ConversionJob {
            item: source.item(),
            source,
            state,
            plan: None,
        }
    }

    /// A `Batch` over the given jobs (in their given, frozen order), for the queue-order tests.
    fn batch_of(jobs: Vec<ConversionJob>) -> Batch {
        Batch {
            id: collected_set_id(),
            source_format: UserFacingFormat::Csv,
            target: sample_target(),
            options: OptionValues(BTreeMap::new()),
            destination: ResolvedDestination::BesideSource,
            jobs,
        }
    }

    /// A minimal frozen `CollectedSet::Single` projection over the given eligible + skipped items, for the C6
    /// `build_batch` tests (P3.47). Only the fields `build_batch` reads matter (`id`/`format`/`items`/
    /// `skipped`); the rest carry benign frozen defaults. `count == items.len()` (the `Single` invariant,
    /// carried through).
    fn frozen_of(items: Vec<DroppedItem>, skipped: Vec<SkippedItem>) -> FrozenCollectedSet {
        let count = items.len();
        FrozenCollectedSet {
            id: collected_set_id(),
            instance: instance_id(),
            format: UserFacingFormat::Csv,
            items,
            count,
            skipped,
            total_bytes: 0,
            roots: vec![],
            encoding_hint: None,
            delimiter_hint: None,
            notes: vec![],
            item_paths: BTreeMap::new(),
        }
    }

    // §6.4.1 unit (G15): the P3.46.1 §1.9 FSM drives every VALID transition — Pending --Started--> Running,
    // Running --{Succeeded|Failed(kind)|Cancelled}--> the matching terminal.
    #[test]
    fn advance_drives_the_valid_1_9_transitions() {
        let kind = ConversionErrorKind::Corrupt;
        assert_eq!(
            advance(JobState::Pending, JobEvent::Started),
            Ok(JobState::Running),
            "§1.9: Pending --Started--> Running"
        );
        assert_eq!(
            advance(JobState::Running, JobEvent::Succeeded),
            Ok(JobState::Succeeded),
            "§1.9: Running --Succeeded--> Succeeded"
        );
        assert_eq!(
            advance(JobState::Running, JobEvent::Failed(kind)),
            Ok(JobState::Failed(kind)),
            "§1.9: Running --Failed(kind)--> Failed(kind) (the kind carried through unchanged)"
        );
        assert_eq!(
            advance(JobState::Running, JobEvent::Cancelled),
            Ok(JobState::Cancelled),
            "§1.9: Running --Cancelled--> Cancelled"
        );
    }

    // §6.4.1 unit (G15): the P3.46.1 FSM REJECTS every illegal §1.9 transition as a structured Err, never a
    // panic (the crate-root clippy::panic/unwrap_used deny) — a terminal (incl. pre-flight Skipped) accepts no
    // event, Pending accepts only Started, Running accepts no second Started.
    #[test]
    fn advance_rejects_every_illegal_1_9_transition_without_a_panic() {
        let kind = ConversionErrorKind::EngineHang;
        let terminals = [
            JobState::Succeeded,
            JobState::Failed(kind),
            JobState::Skipped(SkipReason::Empty),
            JobState::Cancelled,
        ];
        let events = [
            JobEvent::Started,
            JobEvent::Succeeded,
            JobEvent::Failed(kind),
            JobEvent::Cancelled,
        ];
        // Every terminal state (a published/failed/cancelled job AND a pre-flight Skipped) rejects every event.
        for &from in &terminals {
            for &event in &events {
                assert_eq!(
                    advance(from, event),
                    Err(IllegalTransition { from, event }),
                    "§1.9: {from:?} is terminal and rejects {event:?}"
                );
            }
        }
        // Pending accepts ONLY Started — every terminal event is illegal from Pending.
        for &event in &[
            JobEvent::Succeeded,
            JobEvent::Failed(kind),
            JobEvent::Cancelled,
        ] {
            assert_eq!(
                advance(JobState::Pending, event),
                Err(IllegalTransition {
                    from: JobState::Pending,
                    event
                }),
                "§1.9: Pending rejects {event:?} (only Started is valid)"
            );
        }
        // Running rejects a second Started (out-of-order).
        assert_eq!(
            advance(JobState::Running, JobEvent::Started),
            Err(IllegalTransition {
                from: JobState::Running,
                event: JobEvent::Started
            }),
            "§1.9: Running rejects a second Started"
        );
    }

    // §6.4.1 unit (G15): the P3.46.1 deterministic queue order yields ONLY the non-Skipped jobs, in the frozen
    // Batch.jobs order, with NO reordering — the §1.9 "Skipped never enters the queue" + "no priority/size
    // reordering" contract, over an interleaved Pending/Skipped batch.
    #[test]
    fn queue_order_yields_the_queued_jobs_in_batch_order_excluding_skipped() {
        let batch = batch_of(vec![
            job_in(0, JobState::Pending),
            job_in(1, JobState::Skipped(SkipReason::UnsupportedType)),
            job_in(2, JobState::Pending),
            job_in(3, JobState::Skipped(SkipReason::Empty)),
            job_in(4, JobState::Pending),
        ]);
        let queued: Vec<ItemId> = queue_order(&batch).map(|job| job.item).collect();
        assert_eq!(
            queued,
            vec![item_id(0), item_id(2), item_id(4)],
            "§1.9: queue_order yields only the non-Skipped jobs, in Batch.jobs order (Skipped excluded, no reordering)"
        );
    }

    // §6.4.1 unit (G15): the queue carries every state EXCEPT the pre-flight Skipped — an in-flight Running or a
    // completed terminal job stays in the queue (it entered it); only a pre-flight Skipped record is excluded.
    #[test]
    fn queue_order_includes_every_non_skipped_state() {
        let kind = ConversionErrorKind::Corrupt;
        let batch = batch_of(vec![
            job_in(0, JobState::Pending),
            job_in(1, JobState::Running),
            job_in(2, JobState::Succeeded),
            job_in(3, JobState::Failed(kind)),
            job_in(4, JobState::Cancelled),
            job_in(5, JobState::Skipped(SkipReason::Uncertain)),
        ]);
        let queued: Vec<ItemId> = queue_order(&batch).map(|job| job.item).collect();
        assert_eq!(
            queued,
            vec![item_id(0), item_id(1), item_id(2), item_id(3), item_id(4)],
            "§1.9: every state except the pre-flight Skipped is in the queue (they entered it)"
        );
    }

    // §6.4.1 unit (G15): the P3.46.2 projection maps a Succeeded / Cancelled outcome to its terminal event with
    // NO per-item failure message.
    #[test]
    fn project_outcome_maps_succeeded_and_cancelled_with_no_message() {
        assert_eq!(
            project_outcome(InvocationResult::Succeeded),
            TerminalProjection {
                event: JobEvent::Succeeded,
                message: None
            },
            "§2.8: a succeeded item carries no per-item failure message"
        );
        assert_eq!(
            project_outcome(InvocationResult::Cancelled),
            TerminalProjection {
                event: JobEvent::Cancelled,
                message: None
            },
            "§2.8: a cancelled item carries no per-item failure message"
        );
    }

    // §6.4.1 unit (G15): the P3.46.2 Running→Failed projection carries the internal kind through as the wire
    // kind (the §2.8.2 alias IDENTITY) and renders the §2.8.2 catalog OutcomeMsg::Failure (P3.68) for every
    // Running→Failed kind the P3-slice native lane produces (all no-substitution-slot → arg = "").
    #[test]
    fn project_outcome_maps_a_failed_outcome_to_its_kind_identity_and_2_8_2_message() {
        for kind in [
            ConversionErrorKind::Corrupt,
            ConversionErrorKind::Gone,
            ConversionErrorKind::Unreadable,
            ConversionErrorKind::WriteFailed,
            ConversionErrorKind::EngineHang,
            ConversionErrorKind::InternalError,
        ] {
            let projected = project_outcome(InvocationResult::Failed(kind));
            assert_eq!(
                projected.event,
                JobEvent::Failed(kind),
                "§2.8.2: the internal kind IS the wire kind (identity) — carried through unchanged"
            );
            assert_eq!(
                projected.message,
                conversion_failure(kind, ""),
                "§2.8.2: the per-item message is the P3.68 catalog OutcomeMsg::Failure for the kind"
            );
            assert!(
                projected.message.is_some(),
                "§2.8.2: every P3-slice Running→Failed kind renders a message (not vacuously None)"
            );
        }
    }

    // §6.4.1 unit (G15): the P3.46.2 mis-route fallback — a kind §2.8.2 homes ELSEWHERE (an app-level fault like
    // EngineMissing, for which conversion_failure returns None) must never leave a failed item message-less; it
    // falls back to the always-available InternalError row.
    #[test]
    fn project_outcome_falls_back_to_internal_error_for_a_non_per_item_kind() {
        assert_eq!(
            conversion_failure(ConversionErrorKind::EngineMissing, ""),
            None,
            "§2.8.2: EngineMissing is a §2.13 app-level fault, not a per-item conversion row"
        );
        let projected =
            project_outcome(InvocationResult::Failed(ConversionErrorKind::EngineMissing));
        assert_eq!(
            projected.event,
            JobEvent::Failed(ConversionErrorKind::EngineMissing),
            "the terminal event still carries the (mis-routed) kind"
        );
        assert_eq!(
            projected.message,
            conversion_failure(ConversionErrorKind::InternalError, ""),
            "§2.8.2: a non-per-item kind falls back to the InternalError message — a failed item is never message-less"
        );
        assert!(
            projected.message.is_some(),
            "the fallback always renders a message"
        );
    }

    // §2.8 / §1.12 (P3.75 sweep): the TERMINAL `item_base_reason` projection mirrors the live
    // `project_outcome` / `failure_message` InternalError fallback — a per-item `Failed` carrying a mis-homed
    // app-level kind ({EngineMissing, WebviewFault, BundleDamaged, MixedDrop}, none of which has a §2.8.2 row)
    // is NEVER message-less in the summary either, so the live `ItemFinished` message and the terminal
    // `RunResult` reason of one item always agree. Before the fix the terminal arm had no fallback → `None`.
    #[test]
    fn item_base_reason_falls_back_to_internal_error_for_a_non_per_item_kind() {
        for kind in [
            ConversionErrorKind::EngineMissing,
            ConversionErrorKind::WebviewFault,
            ConversionErrorKind::BundleDamaged,
            ConversionErrorKind::MixedDrop,
        ] {
            assert_eq!(
                conversion_failure(kind, ""),
                None,
                "§2.8.2: {kind:?} is a §2.13 app-level fault with no per-item row"
            );
            let job = job_in(0, JobState::Failed(kind));
            let reason = item_base_reason(&job, None);
            assert_eq!(
                reason,
                conversion_failure(ConversionErrorKind::InternalError, ""),
                "§2.8/§1.12: a mis-homed app-level kind falls back to InternalError — the terminal reason is never message-less"
            );
            assert!(
                reason.is_some(),
                "the terminal projection always renders a message for {kind:?}"
            );
        }
    }

    // A `FrozenSnapshot` from explicit eligible + skipped views — for the §1.3 `group()` projection unit tests
    // (constructed directly to control the skip reasons the Empty-projection keys off; the identities/item_paths
    // are empty because `group()`'s projection does not read them). [Build-Session-Entscheidung: P3.49]
    fn frozen_snapshot(items: Vec<DroppedItem>, skipped: Vec<SkippedItem>) -> FrozenSnapshot {
        FrozenSnapshot {
            items,
            skipped,
            item_paths: BTreeMap::new(),
            identities: BTreeMap::new(),
            roots: vec![PathBuf::from("/drop")],
        }
    }

    // A discarding §0.4.2 scan Channel — `ingest` never depends on the frontend receiving telemetry.
    fn discard_scan() -> Channel<ScanProgress> {
        Channel::new(|_body: InvokeResponseBody| Ok(()))
    }

    // §6.4.1 unit (G15) / §1.1/§2.4: the `ingest` freeze funnel is ORIGIN-INDEPENDENT — every intake origin
    // funnels the same way (P3.78), so the §1.3 projection keys off DETECTION, not origin. An EMPTY intake set
    // (no roots to walk) yields the zero-collection `Empty` for every origin. [Test-Change: P3.49 —
    // old-obsolete+new-correct, §1.1] the P2.62 assertion that a NON-empty set also returns `Empty` (the `ingest`
    // interface shell) is OBSOLETE — the funnel now freezes real sets (see `ingest_of_one_csv_*` below); the
    // origin-independence + empty-set contract survives, the compile-time variant lock stays.
    #[test]
    fn ingest_funnel_is_origin_independent_and_empty_for_an_empty_set() {
        // Compile-time variant lock (the established `exhaustive`-match pattern): a new `IntakeOrigin` variant
        // breaks this match, forcing the `all` array below to grow with it, so the test can never silently miss
        // a new origin. [Build-Session-Entscheidung: P2.62]
        fn exhaustive(o: IntakeOrigin) {
            match o {
                IntakeOrigin::Drop
                | IntakeOrigin::Picker
                | IntakeOrigin::LaunchArg
                | IntakeOrigin::SecondInstance => {}
            }
        }
        let all = [
            IntakeOrigin::Drop,
            IntakeOrigin::Picker,
            IntakeOrigin::LaunchArg,
            IntakeOrigin::SecondInstance,
        ];
        for origin in all {
            exhaustive(origin);
            let result = ingest(
                Vec::new(),
                origin,
                &CancellationToken::new(),
                &discard_scan(),
                InstanceId::mint(),
            );
            assert_eq!(
                result.collected,
                CollectedSet::Empty {
                    skipped: Vec::new(),
                },
                "§1.1/§2.4: an empty intake set (no roots) is the zero-collection Empty for every origin"
            );
            assert!(
                result.registrable.is_none(),
                "§0.4.4: a zero-collection funnel registers nothing"
            );
        }
    }

    // §6.4.1 unit (G15) / §1.3: exactly one eligible source format → a registrable `Single`.
    #[test]
    fn group_of_one_format_is_a_registrable_single() {
        let snapshot = frozen_snapshot(
            vec![
                dropped_item_with(0, UserFacingFormat::Csv),
                dropped_item_with(1, UserFacingFormat::Csv),
            ],
            Vec::new(),
        );
        let result = group(snapshot, InstanceId::mint(), SliceHints::default());
        assert!(
            matches!(
                &result.collected,
                CollectedSet::Single {
                    format: UserFacingFormat::Csv,
                    count: 2,
                    ..
                }
            ),
            "§1.3: one eligible format across the members → Single (CSV, count 2), got {:?}",
            result.collected
        );
        assert!(
            result.registrable.is_some(),
            "§0.4.4: a Single collection is the ONE registrable outcome"
        );
    }

    // §6.4.1 unit (G15) / §1.3: two or more distinct eligible formats → the hard `Mixed` refusal (no partial
    // conversion, no registration).
    #[test]
    fn group_of_two_formats_is_a_mixed_refusal() {
        let snapshot = frozen_snapshot(
            vec![
                dropped_item_with(0, UserFacingFormat::Csv),
                dropped_item_with(1, UserFacingFormat::Tsv),
            ],
            Vec::new(),
        );
        let result = group(snapshot, InstanceId::mint(), SliceHints::default());
        assert!(
            result.registrable.is_none(),
            "§1.3: a Mixed refusal registers nothing"
        );
        assert!(
            matches!(&result.collected, CollectedSet::Mixed { .. }),
            "§1.3: two distinct eligible formats → Mixed, got {:?}",
            result.collected
        );
        if let CollectedSet::Mixed { found } = &result.collected {
            assert_eq!(
                found.len(),
                2,
                "§1.3: the refusal names both found formats (with counts)"
            );
        }
    }

    // §6.4.1 unit (G15) / §1.3: a LONE `UnsupportedType` skip → the specific `Unsupported { detected }` (the
    // SSOT-6 "detected: X" surface), NOT a generic Empty.
    #[test]
    fn group_of_a_lone_unsupported_skip_is_the_specific_unsupported() {
        let snapshot = frozen_snapshot(
            Vec::new(),
            vec![skipped_item_detected(
                0,
                SkipReason::UnsupportedType,
                Some("HEIC"),
            )],
        );
        let result = group(snapshot, InstanceId::mint(), SliceHints::default());
        assert_eq!(
            result.collected,
            CollectedSet::Unsupported {
                detected: "HEIC".to_string(),
            },
            "§1.3: a lone UnsupportedType skip → the specific Unsupported carrying the detected type"
        );
        assert!(result.registrable.is_none());
    }

    // §6.4.1 unit (G15) / §1.3: a LONE `Uncertain` skip → the specific `Uncertain { note }` (from the best-guess).
    #[test]
    fn group_of_a_lone_uncertain_skip_is_the_specific_uncertain() {
        let snapshot = frozen_snapshot(
            Vec::new(),
            vec![skipped_item_detected(
                0,
                SkipReason::Uncertain,
                Some("maybe RTF"),
            )],
        );
        let result = group(snapshot, InstanceId::mint(), SliceHints::default());
        assert_eq!(
            result.collected,
            CollectedSet::Uncertain {
                note: "maybe RTF".to_string(),
            },
            "§1.3: a lone Uncertain skip → the specific Uncertain note from the §1.2 best-guess"
        );
    }

    // §6.4.1 unit (G15) / §1.3: a lone skip that is NEITHER Unsupported nor Uncertain (e.g. Unreadable) → the
    // generic `Empty { skipped }` carrying the reason (no lone-specificity for those kinds).
    #[test]
    fn group_of_a_lone_unreadable_skip_is_the_generic_empty() {
        let snapshot = frozen_snapshot(Vec::new(), vec![skipped_item(0, SkipReason::Unreadable)]);
        let result = group(snapshot, InstanceId::mint(), SliceHints::default());
        assert!(
            matches!(&result.collected, CollectedSet::Empty { .. }),
            "§1.3: a lone Unreadable (not Unsupported/Uncertain) → the generic Empty, got {:?}",
            result.collected
        );
        if let CollectedSet::Empty { skipped } = &result.collected {
            assert_eq!(
                skipped.len(),
                1,
                "§1.3: the generic Empty carries the per-item reason (not discarded)"
            );
        }
    }

    // §6.4.1 unit (G15) / §1.3: 2+ ineligibles of mixed kinds → the generic `Empty { skipped }` carrying EVERY
    // reason (no lone-Unsupported/Uncertain collapse — the reasons are no longer lost).
    #[test]
    fn group_of_multiple_ineligibles_is_the_generic_empty_carrying_all_reasons() {
        let snapshot = frozen_snapshot(
            Vec::new(),
            vec![
                skipped_item_detected(0, SkipReason::UnsupportedType, Some("HEIC")),
                skipped_item(1, SkipReason::Uncertain),
            ],
        );
        let result = group(snapshot, InstanceId::mint(), SliceHints::default());
        assert!(
            matches!(&result.collected, CollectedSet::Empty { .. }),
            "§1.3: 2+ ineligibles of mixed kinds → the generic Empty, got {:?}",
            result.collected
        );
        if let CollectedSet::Empty { skipped } = &result.collected {
            assert_eq!(
                skipped.len(),
                2,
                "§1.3: the generic Empty carries every per-item reason (no lone-specificity collapse)"
            );
        }
    }

    // §6.4.1 unit (G15) / §1.3: genuinely zero items and zero skips → the bare zero-collection `Empty`.
    #[test]
    fn group_of_nothing_is_the_bare_empty() {
        let result = group(
            frozen_snapshot(Vec::new(), Vec::new()),
            InstanceId::mint(),
            SliceHints::default(),
        );
        assert_eq!(
            result.collected,
            CollectedSet::Empty {
                skipped: Vec::new(),
            },
            "§1.3: zero items + zero skips → the bare zero-collection Empty"
        );
        assert!(result.registrable.is_none());
    }

    // §6.4.1 real-FS (G15) / §1.1/§1.3: a lone CSV drop freezes end-to-end into a registrable `Single` (CSV,
    // count 1, nonzero bytes) — the real walk → detect → freeze → group spine (test-strategy §0.1: no mocks).
    #[test]
    fn ingest_of_one_csv_freezes_a_registrable_single() {
        let dir = tempfile::tempdir().expect("temp dir");
        let csv = dir.path().join("data.csv");
        std::fs::write(&csv, b"a,b,c\n1,2,3\n").expect("write the CSV source");
        let result = ingest(
            vec![csv],
            IntakeOrigin::Drop,
            &CancellationToken::new(),
            &discard_scan(),
            InstanceId::mint(),
        );
        assert!(
            matches!(
                &result.collected,
                CollectedSet::Single {
                    format: UserFacingFormat::Csv,
                    count: 1,
                    total_bytes,
                    ..
                } if *total_bytes > 0
            ),
            "§1.1/§1.3: a lone CSV drop freezes a Single (CSV, count 1, nonzero bytes), got {:?}",
            result.collected
        );
        assert!(
            result.registrable.is_some(),
            "§0.4.4: the frozen Single is registrable"
        );
    }

    // §6.4.1 real-FS (G15) / §1.3: CSV + TSV are two distinct eligible formats → the hard `Mixed` refusal.
    #[test]
    fn ingest_of_csv_and_tsv_is_a_mixed_refusal() {
        let dir = tempfile::tempdir().expect("temp dir");
        let csv = dir.path().join("a.csv");
        let tsv = dir.path().join("b.tsv");
        std::fs::write(&csv, b"a,b\n1,2\n").expect("write the CSV source");
        std::fs::write(&tsv, b"a\tb\n1\t2\n").expect("write the TSV source");
        let result = ingest(
            vec![csv, tsv],
            IntakeOrigin::Drop,
            &CancellationToken::new(),
            &discard_scan(),
            InstanceId::mint(),
        );
        assert!(
            matches!(&result.collected, CollectedSet::Mixed { .. }),
            "§1.3: CSV + TSV are two distinct eligible formats → Mixed, got {:?}",
            result.collected
        );
        assert!(result.registrable.is_none());
    }

    // §6.4.1 real-FS (G15) / §1.4: a semicolon-delimited CSV is detected as CSV, and the §1.4 delimiter hint is
    // recomputed from the representative header as ";".
    #[test]
    fn ingest_surfaces_the_semicolon_delimiter_hint() {
        let dir = tempfile::tempdir().expect("temp dir");
        let csv = dir.path().join("data.csv");
        std::fs::write(&csv, b"a;b;c\n1;2;3\n").expect("write the semicolon CSV");
        let result = ingest(
            vec![csv],
            IntakeOrigin::Drop,
            &CancellationToken::new(),
            &discard_scan(),
            InstanceId::mint(),
        );
        assert!(
            matches!(&result.collected, CollectedSet::Single { .. }),
            "§1.3: a semicolon-CSV is a Single, got {:?}",
            result.collected
        );
        if let CollectedSet::Single { delimiter_hint, .. } = &result.collected {
            assert_eq!(
                delimiter_hint.as_deref(),
                Some(";"),
                "§1.4: a semicolon-CSV surfaces the ';' delimiter hint (recomputed from the representative header)"
            );
        }
    }

    // §6.4.1 real-FS (G15) / §1.1: a PRE-TRIPPED ingest token discards the partial, not-yet-frozen set → the
    // zero-collection Empty (the cooperative-cancel contract C13 drives).
    #[test]
    fn ingest_of_a_cancelled_token_discards_the_partial() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("data.csv"), b"a,b\n1,2\n").expect("write the CSV source");
        let cancel = CancellationToken::new();
        cancel.cancel();
        let result = ingest(
            vec![dir.path().to_path_buf()],
            IntakeOrigin::Drop,
            &cancel,
            &discard_scan(),
            InstanceId::mint(),
        );
        assert_eq!(
            result.collected,
            CollectedSet::Empty {
                skipped: Vec::new(),
            },
            "§1.1: a tripped ingest token discards the partial set → the zero-collection Empty"
        );
        assert!(result.registrable.is_none());
    }

    // §6.4.1 real-FS (G15) / §1.1: a fatal walk-root (a dropped root that does not exist) sinks the ingest →
    // the root surfaces as ONE Unreadable skip through the freeze (intake-time unreadable = Skipped), never a
    // panic.
    #[test]
    fn ingest_of_a_fatal_walk_root_surfaces_the_root_as_an_unreadable_skip() {
        let dir = tempfile::tempdir().expect("temp dir");
        let missing = dir.path().join("nonexistent-dropped-root");
        let result = ingest(
            vec![missing],
            IntakeOrigin::Drop,
            &CancellationToken::new(),
            &discard_scan(),
            InstanceId::mint(),
        );
        assert!(result.registrable.is_none());
        assert!(
            matches!(&result.collected, CollectedSet::Empty { .. }),
            "§1.1: a fatal walk-root sinks the ingest → Empty, got {:?}",
            result.collected
        );
        if let CollectedSet::Empty { skipped } = &result.collected {
            assert_eq!(
                skipped.len(),
                1,
                "§1.1: the fatal root surfaces as exactly ONE Unreadable skip"
            );
            assert_eq!(
                skipped.first().map(|item| item.reason),
                Some(SkipReason::Unreadable),
                "§1.1: the fatal root's skip reason is Unreadable (SSOT Fail clearly — the reason is not dropped)"
            );
        }
    }

    // §6.4.1 real-FS (G15) / §1.1/§1.3/§1.4: a mixed drop of ONE eligible CSV + ONE ineligible (0-byte) file in
    // the SAME folder freezes through the REAL funnel to a `Single { count: 1, skipped: [the 0-byte Empty skip] }`
    // — the walk → detect(Empty) → freeze single-id-space merge → §1.3 `group()` `Single{skipped}` composition
    // seam, read back end-to-end (not a hand-built FrozenSnapshot).
    #[test]
    fn ingest_of_one_csv_plus_one_empty_file_is_a_single_carrying_the_skip() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("data.csv"), b"a,b\n1,2\n").expect("write the CSV source");
        std::fs::write(dir.path().join("empty.dat"), b"").expect("write the 0-byte file");
        let result = ingest(
            vec![dir.path().to_path_buf()],
            IntakeOrigin::Drop,
            &CancellationToken::new(),
            &discard_scan(),
            InstanceId::mint(),
        );
        assert!(
            result.registrable.is_some(),
            "§0.4.4: one eligible format (CSV) → a registrable Single, even with an ineligible sibling"
        );
        assert!(
            matches!(
                &result.collected,
                CollectedSet::Single {
                    format: UserFacingFormat::Csv,
                    count: 1,
                    ..
                }
            ),
            "§1.3: one eligible CSV among a mixed drop → Single (CSV, count 1), got {:?}",
            result.collected
        );
        if let CollectedSet::Single { skipped, .. } = &result.collected {
            assert_eq!(
                skipped.len(),
                1,
                "§1.4: the ineligible 0-byte file rides the Single's skipped list (not the eligible count)"
            );
            assert_eq!(
                skipped.first().map(|item| item.reason),
                Some(SkipReason::Empty),
                "§1.2: the 0-byte file is detected Empty and surfaced as an Empty skip"
            );
        }
    }

    // §6.4.1 real-FS (G15) / §0.4.2: the scan throttle emits the final total once the walk completes (a
    // best-effort monotonic count for the §5.2 Collecting state).
    #[test]
    fn ingest_emits_the_final_scan_count() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("a.csv"), b"x,y\n1,2\n").expect("write");
        std::fs::write(dir.path().join("b.csv"), b"x,y\n3,4\n").expect("write");
        let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let channel = Channel::new(move |body: InvokeResponseBody| {
            if let InvokeResponseBody::Json(json) = body {
                sink.lock().expect("scan sink lock").push(json);
            }
            Ok(())
        });
        let _ = ingest(
            vec![dir.path().to_path_buf()],
            IntakeOrigin::Drop,
            &CancellationToken::new(),
            &channel,
            InstanceId::mint(),
        );
        let emitted = seen.lock().expect("scan sink lock").clone();
        assert!(
            emitted.iter().any(|json| json.contains("\"scanned\":2")),
            "§0.4.2: the throttle emits the final total (scanned:2) once the walk completes, got {emitted:?}"
        );
    }

    // §6.4.1 unit (G15): the §0.6/§1.9 `JobState` is exactly the SIX lifecycle states, in the §0.6 order.
    // The `exhaustive` match is the COMPILE-TIME variant lock (the established dependency-free pattern, cf.
    // `crate::outcome`'s `conversion_error_kind_exhaustive`): adding/removing a variant without updating
    // it fails to compile, so the lifecycle set can never silently drift from §0.6. The payload assertions
    // pin that `Failed` carries the §2.8 kind and `Skipped` the §0.6 `SkipReason`.
    #[test]
    fn job_state_is_the_six_lifecycle_states() {
        fn exhaustive(s: JobState) {
            match s {
                JobState::Pending
                | JobState::Running
                | JobState::Succeeded
                | JobState::Failed(_)
                | JobState::Skipped(_)
                | JobState::Cancelled => {}
            }
        }
        let all = [
            JobState::Pending,
            JobState::Running,
            JobState::Succeeded,
            JobState::Failed(ConversionErrorKind::Corrupt),
            JobState::Skipped(SkipReason::Empty),
            JobState::Cancelled,
        ];
        assert_eq!(
            all.len(),
            6,
            "§0.6/§1.9: JobState has exactly six lifecycle states"
        );
        for s in all {
            exhaustive(s);
        }
        assert_eq!(
            JobState::Failed(ConversionErrorKind::Gone),
            JobState::Failed(ConversionErrorKind::Gone),
            "§0.6: Failed carries the §2.8 kind (the ConversionErrorKind that ErrorKind aliases)"
        );
        assert_ne!(
            JobState::Skipped(SkipReason::Empty),
            JobState::Skipped(SkipReason::Unreadable),
            "§0.6: Skipped carries the distinguishing SkipReason"
        );
    }

    // §6.4.1 unit (G15): the §0.6 `ConversionJob.item == source.item()` denormalization — the job's
    // top-level key IS its source item's id (cheap addressing without unwrapping `source`). This box
    // authors the TYPE so the relationship is expressible + correct; the orchestrator-ALWAYS-enforces-it
    // property is P2.14.
    // [Test-Change: P3.47 — old-obsolete+new-correct, §0.6] `source` is a `JobSource` sum now; the
    // denormalization reads the uniform `source.item()` accessor (the old `source.item` field access no
    // longer compiles), exercised over the `Eligible` arm here (the `Skipped` arm is covered in P2.14).
    #[test]
    fn conversion_job_denormalizes_its_source_item() {
        let source = JobSource::Eligible(dropped_item(3));
        let job = ConversionJob {
            item: source.item(),
            source: source.clone(),
            state: JobState::Pending,
            plan: None,
        };
        assert_eq!(
            job.item,
            job.source.item(),
            "§0.6: ConversionJob.item is denormalized from source.item()"
        );
    }

    // §6.4.1 unit (G15): a `Batch` carries ONE whole-batch `Target` (§0.6 invariant 1, enforced by the
    // single-value field SHAPE) over its jobs. Constructs the full `Batch → ConversionJob → JobSource →
    // DroppedItem` graph so the P2.10/P3.47 types are exercised (and the test build is dead-code-clean); the
    // per-item invariant ENFORCEMENT (count/frozen/stable-id) is the P2.14 property suite.
    // [Test-Change: P3.47 — old-obsolete+new-correct, §0.6] `source` is wrapped in `JobSource::Eligible`
    // (the old bare `DroppedItem` form no longer compiles); the shape assertions below are unchanged.
    #[test]
    fn batch_holds_one_target_over_its_jobs() {
        let source = JobSource::Eligible(dropped_item(0));
        let job = ConversionJob {
            item: source.item(),
            source,
            state: JobState::Pending,
            plan: None,
        };
        let batch = Batch {
            id: collected_set_id(),
            source_format: UserFacingFormat::Csv,
            target: sample_target(),
            options: OptionValues(BTreeMap::new()),
            destination: ResolvedDestination::BesideSource,
            jobs: vec![job],
        };
        assert_eq!(
            batch.jobs.len(),
            1,
            "the constructed batch carries its one job"
        );
        assert_eq!(
            batch.target.id,
            TargetId::Format(UserFacingFormat::Tsv),
            "§0.6 invariant 1: a single whole-batch Target (the field shape enforces it)"
        );
        assert_eq!(
            batch.source_format,
            UserFacingFormat::Csv,
            "the batch's single eligible source format (§1.3 grouping key)"
        );
    }

    // §6.4.1 unit (G15): the P3.47 §1.9 C6 `build_batch` materialisation — from a frozen `CollectedSet::
    // Single` with INTERLEAVED eligible (ids 0/2/4) + skipped (ids 1/3) items, it builds `Batch.jobs`
    // carrying BOTH kinds in the deterministic §1.1 traversal order (id-sorted), with each eligible item a
    // `Pending`/`JobSource::Eligible` job and each skipped item a terminal `Skipped(reason)`/`JobSource::
    // Skipped` job whose reason is copied from the `SkippedItem` — and `queue_order` then yields ONLY the
    // eligible (Pending) ids. The "skips survive C6" anchor: a skip stored only in the (now-evicted) registry
    // is materialised into the Batch.
    #[test]
    fn build_batch_materialises_eligible_pending_and_preflight_skipped_records_in_order() {
        // Eligible ids 0/2/4 and skipped ids 1/3 — deliberately supplied out of interleave so the id-sort
        // (the §1.1 traversal order over the single id space) is exercised, not the input order.
        let items = vec![dropped_item(0), dropped_item(2), dropped_item(4)];
        let skipped = vec![
            skipped_item(1, SkipReason::UnsupportedType),
            skipped_item(3, SkipReason::Empty),
        ];
        let frozen = frozen_of(items, skipped);
        let batch = build_batch(
            &frozen,
            sample_target(),
            OptionValues(BTreeMap::new()),
            ResolvedDestination::BesideSource,
        );

        // The batch carries its whole-batch fields from the frozen set + the C6 args.
        assert_eq!(
            batch.id, frozen.id,
            "§1.12: Batch.id IS the source CollectedSetId"
        );
        assert_eq!(
            batch.source_format,
            UserFacingFormat::Csv,
            "§1.3: the single eligible source format is carried from the frozen set"
        );

        // Every eligible + every skipped item became exactly one job, in id (= §1.1 traversal) order.
        let states: Vec<(ItemId, JobState)> =
            batch.jobs.iter().map(|job| (job.item, job.state)).collect();
        assert_eq!(
            states,
            vec![
                (item_id(0), JobState::Pending),
                (item_id(1), JobState::Skipped(SkipReason::UnsupportedType)),
                (item_id(2), JobState::Pending),
                (item_id(3), JobState::Skipped(SkipReason::Empty)),
                (item_id(4), JobState::Pending),
            ],
            "§1.9: build_batch materialises eligible Pending + pre-flight Skipped(reason) jobs, interleaved \
             into the deterministic id/traversal order (the skip reason copied from the SkippedItem)"
        );

        // The source arm mirrors the state (the coupling invariant) for every job, and the plan is None.
        for job in &batch.jobs {
            let is_skipped_source = matches!(job.source, JobSource::Skipped(_));
            let is_skipped_state = matches!(job.state, JobState::Skipped(_));
            assert_eq!(
                is_skipped_source, is_skipped_state,
                "§0.6 coupling: source is Skipped(_) ⟺ state is JobState::Skipped(_)"
            );
            assert_eq!(
                job.item,
                job.source.item(),
                "§0.6: the job key is denormalized uniformly from source.item()"
            );
            assert!(
                job.plan.is_none(),
                "§1.9: no job is planned at construction (§1.8 plans eligible jobs subsequently; a skip never plans)"
            );
        }

        // The queue is exactly the eligible (Pending) ids — the Skipped records never enter it (§1.9).
        let queued: Vec<ItemId> = queue_order(&batch).map(|job| job.item).collect();
        assert_eq!(
            queued,
            vec![item_id(0), item_id(2), item_id(4)],
            "§1.9: queue_order yields only the eligible Pending jobs; the pre-flight Skipped records are non-queue"
        );
    }

    /// Extract the resolved `text` of an `ItemResult.reason`, whichever `OutcomeMsg` variant
    /// (§2.8/§2.9/§1.12/§2.6.4). [Test-Change: P3.59 — old-obsolete+new-correct, §2.6.4] gains the `Residue`
    /// arm: the 2026-07-16 ruling added the §2.6.4 case-1 variant, and this helper is text-extraction only —
    /// it is variant-agnostic by contract, so the arm joins the existing or-pattern rather than branching. No
    /// assertion changed; the exhaustive match (G4/G14) is what forced the update.
    fn reason_text(reason: &Option<OutcomeMsg>) -> Option<String> {
        match reason {
            Some(OutcomeMsg::Failure { text, .. })
            | Some(OutcomeMsg::Skipped { text, .. })
            | Some(OutcomeMsg::Lossy { text, .. })
            | Some(OutcomeMsg::Residue { text }) => Some(text.clone()),
            None => None,
        }
    }

    // §6.4.1 unit (G15): the P3.50 §1.12 run-end projection — a TERMINAL batch with one of each disposition
    // (Succeeded 0 / Failed 1 / pre-flight-Skipped 2 / Cancelled 3) + a published output + a §2.6.4 residue maps
    // onto the wire `RunResult` (display-only) + the off-wire `RunResultPaths` (real paths). Asserts: the
    // per-disposition Totals (the SKIP counted in `skipped`, NEVER `failed`); `output_display` only for the
    // Succeeded item; the Failed-WITH-residue item's reason is the combined §2.6.4 CleanupResidue message (never
    // a clean success); the skip's reason is `OutcomeMsg::Skipped` naming the retained detected type (SSOT-6);
    // the residue folds into `cleanup_incomplete` (wire) + `item_residues` (off-wire); the real paths ride
    // `RunResultPaths`, the wire carries only their displays.
    #[test]
    fn project_run_result_maps_terminal_jobs_to_the_1_12_summary() {
        let kind = ConversionErrorKind::Corrupt;
        // The skip (item 2) is an UnsupportedType with a RETAINED detected name (P3.50 ruling).
        let skip_job = ConversionJob {
            item: item_id(2),
            source: JobSource::Skipped(skipped_item_detected(
                2,
                SkipReason::UnsupportedType,
                Some("a ZIP archive"),
            )),
            state: JobState::Skipped(SkipReason::UnsupportedType),
            plan: None,
        };
        let batch = batch_of(vec![
            job_in(0, JobState::Succeeded),
            job_in(1, JobState::Failed(kind)),
            skip_job,
            job_in(3, JobState::Cancelled),
        ]);
        let outputs: BTreeMap<ItemId, PathBuf> =
            BTreeMap::from([(item_id(0), PathBuf::from("out/a.tsv"))]);
        let residues = vec![ResidueRecord::new(item_id(1), PathBuf::from("tmp/b.part"))];
        // No §2.2.4 unopenable-name failure in this fixture → an empty per-item token map.
        let failed_name_args: BTreeMap<ItemId, String> = BTreeMap::new();
        let (result, paths) = project_run_result(
            &batch,
            run_id(),
            &outputs,
            &failed_name_args,
            residues,
            PathBuf::from("root"),
            None,
        );

        // §1.12 Totals — the skip is counted in `skipped`, NEVER `failed` (skip ≠ fail).
        assert_eq!(
            result.totals,
            Totals {
                succeeded: 1,
                failed: 1,
                cancelled: 1,
                skipped: 1,
            },
            "§1.12: one of each disposition; the pre-flight skip counts in `skipped`, never `failed`"
        );
        assert_eq!(
            result.run_id,
            run_id(),
            "§1.12: the summary carries its run id"
        );
        assert_eq!(result.items.len(), 4, "one ItemResult per job");

        // Item 0: Succeeded → output_display Some(display), no failure reason.
        let i0 = &result.items[0];
        assert_eq!(i0.state, JobState::Succeeded);
        assert_eq!(
            i0.output_display.as_deref(),
            Some("out/a.tsv"),
            "§1.12: a Succeeded item carries its published output display"
        );
        assert!(
            i0.reason.is_none(),
            "a plain success has no failure/lossy reason"
        );

        // Item 1: Failed WITH residue → the reason is OVERRIDDEN to the §2.6.4 CleanupResidue message naming
        // the residue path (never a clean success); no output_display.
        let i1 = &result.items[1];
        assert_eq!(i1.state, JobState::Failed(kind));
        assert!(i1.output_display.is_none(), "a Failed item has no output");
        let i1_reason =
            reason_text(&i1.reason).expect("a Failed-with-residue item carries a reason");
        assert!(
            i1_reason.contains("tmp/b.part"),
            "§2.6.4: the Failed-with-residue reason names WHERE the residue may remain (never a clean success)"
        );

        // Item 2: the pre-flight skip → OutcomeMsg::Skipped naming the retained detected type (SSOT-6).
        let i2 = &result.items[2];
        assert_eq!(i2.state, JobState::Skipped(SkipReason::UnsupportedType));
        assert!(i2.output_display.is_none(), "a skip has no output");
        assert!(
            matches!(i2.reason, Some(OutcomeMsg::Skipped { .. })),
            "§1.12: the skip reason rides OutcomeMsg::Skipped (never Failure)"
        );
        let i2_reason = reason_text(&i2.reason).expect("a skip carries a reason");
        assert!(
            i2_reason.contains("a ZIP archive"),
            "SSOT-6: the skip line names the retained detected type (detected: X)"
        );

        // §2.6.4 honesty split: item 1's residue in the wire `cleanup_incomplete` + the off-wire real path.
        assert_eq!(
            result.cleanup_incomplete.len(),
            1,
            "the one residue is surfaced"
        );
        assert_eq!(result.cleanup_incomplete[0].item, item_id(1));
        assert_eq!(
            paths.item_outputs.get(&item_id(0)),
            Some(&PathBuf::from("out/a.tsv")),
            "the real output PathBuf rides RunResultPaths (off-wire, §2.10.1)"
        );
        assert_eq!(
            paths.item_residues.get(&item_id(1)),
            Some(&PathBuf::from("tmp/b.part")),
            "the real residue PathBuf rides RunResultPaths (C9 Residue(item) target, §2.10.1)"
        );
        assert_eq!(
            result.common_root_display, "root",
            "§2.10.1: the wire carries only the common-root DISPLAY; the real root is in RunResultPaths"
        );
        assert_eq!(paths.common_root, PathBuf::from("root"));
        assert!(
            result.divert_root_display.is_none(),
            "no item diverted → no divert-root display"
        );
    }

    // §2.2.4 (P3.88): the live `ItemFinished` message for an `UnopenableOutputName` failure fills the `{name}`
    // slot with the offending token (the conductor's INLINE render); a no-substitution kind ignores the arg.
    #[test]
    fn failure_message_fills_the_unopenable_name_slot() {
        let msg = failure_message(ConversionErrorKind::UnopenableOutputName, "CON.tsv");
        assert!(
            msg.contains("CON.tsv") && !msg.contains("{name}"),
            "§2.2.4: the live message NAMES the token with the slot filled, got: {msg}"
        );
        // A no-substitution kind renders its full string regardless of the (empty) arg.
        let write_failed = failure_message(ConversionErrorKind::WriteFailed, "");
        assert!(
            !write_failed.is_empty() && !write_failed.contains('{'),
            "§2.8.2: a no-substitution kind renders its full string: {write_failed}"
        );
    }

    // §2.2.4 (P3.88): a TERMINAL `Failed(UnopenableOutputName)` job renders its `RunResult` reason NAMING the
    // offending token from `failed_name_args` — the terminal reason matches the live message (never a literal
    // `{name}`). A token-less run (empty map) is unaffected. NOTE: `failed_name_args` is hand-populated here (not
    // derived from a live `run_conversion`) BY NECESSITY: an end-to-end conductor run that actually PRODUCES an
    // `UnopenableOutputName` needs a source whose §2.2.1 verbatim stem constructs a reserved output (e.g. a
    // `CON.csv` source → `CON.tsv` leaf), and such a source CANNOT EXIST on the Windows test host — that
    // unrepresentable-on-Windows source is the very reason §2.2.4 exists (the names arrive from Unix media /
    // network shares). So the collection glue in `run_conversion` (`if let Some(token) = &name_arg { … insert … }`)
    // is covered by construction (trivial, and the map key `item` equals the terminal `job.item` lookup key), not
    // by an E2E — a documented, unavoidable coverage seam, not a green-by-omission.
    #[test]
    fn project_run_result_names_the_unopenable_token_in_the_terminal_reason() {
        let batch = batch_of(vec![job_in(
            0,
            JobState::Failed(ConversionErrorKind::UnopenableOutputName),
        )]);
        let outputs: BTreeMap<ItemId, PathBuf> = BTreeMap::new();
        let failed_name_args: BTreeMap<ItemId, String> =
            BTreeMap::from([(item_id(0), "CON.tsv".to_owned())]);
        let (result, _paths) = project_run_result(
            &batch,
            run_id(),
            &outputs,
            &failed_name_args,
            Vec::new(),
            PathBuf::from("root"),
            None,
        );
        let reason =
            reason_text(&result.items[0].reason).expect("a failed item carries a §2.8 reason");
        assert!(
            reason.contains("CON.tsv"),
            "§2.2.4: the terminal reason NAMES the offending token, got: {reason}"
        );
        assert!(
            !reason.contains("{name}"),
            "§2.2.4: the {{name}} slot is filled, never left literal in the terminal reason"
        );
    }

    // §6.4.1 unit (G15): the P3.50 §1.12 batch-summary classifier — the headline reflects the ATTEMPTED items
    // (succeeded + failed); pre-flight SKIPS are EXCLUDED from `{n}` (skip ≠ fail); any cancel DOMINATES; a
    // no-success run is an explicit AllFailed (SSOT Fail clearly). `batch_summary_line` appends the §2.6.4
    // "With residue" tail iff the run left residue.
    #[test]
    fn batch_summary_classifies_over_attempted_items_excluding_skips() {
        use crate::outcome::BatchSummary;
        // All attempted succeeded — the 2 skips do NOT inflate the headline `{n}` (n == succeeded).
        assert_eq!(
            batch_summary(&Totals {
                succeeded: 3,
                failed: 0,
                cancelled: 0,
                skipped: 2,
            }),
            BatchSummary::AllSucceeded { n: 3 },
            "§1.12: AllSucceeded over attempted only — skips excluded from {{n}}"
        );
        // Every attempted item failed → an explicit failure (never a quiet finish).
        assert_eq!(
            batch_summary(&Totals {
                succeeded: 0,
                failed: 4,
                cancelled: 0,
                skipped: 1,
            }),
            BatchSummary::AllFailed { n: 4 },
            "§1.12/SSOT: a no-success run is an explicit AllFailed"
        );
        // A mix → Partial { ok, n = ok + fail, fail }.
        assert_eq!(
            batch_summary(&Totals {
                succeeded: 2,
                failed: 1,
                cancelled: 0,
                skipped: 1,
            }),
            BatchSummary::Partial {
                ok: 2,
                n: 3,
                fail: 1,
            },
            "§1.12: Partial with n = attempted (ok + fail)"
        );
        // Any cancel DOMINATES the headline → Stopped, ok = the finished-before-cancel successes.
        assert_eq!(
            batch_summary(&Totals {
                succeeded: 2,
                failed: 1,
                cancelled: 3,
                skipped: 0,
            }),
            BatchSummary::Cancelled { ok: 2 },
            "§2.8.2: a cancelled run is Stopped; ok = the kept successes"
        );
        // batch_summary_line appends the §2.6.4 "With residue" tail iff the run left residue.
        let totals = Totals {
            succeeded: 1,
            failed: 0,
            cancelled: 0,
            skipped: 0,
        };
        assert!(
            !batch_summary_line(&totals, false).contains("temporary files may remain"),
            "no residue → no tail"
        );
        let with_tail = batch_summary_line(&totals, true);
        assert!(
            with_tail.starts_with("All 1 files converted."),
            "the batch line comes first"
        );
        assert!(
            with_tail.contains("temporary files may remain"),
            "§2.6.4: the With-residue tail is appended when the run left residue"
        );
    }

    // §6.4.1 unit (G15): the §0.6/§1.10 `PreflightVerdict` wire form (P2.11) — the resource-preflight
    // verdict in its camelCase wire shape, with `up_front_fail` None (not doomed) and Some(TooBig)
    // (whole-batch doomed). A SERIALIZE pin (PreflightVerdict is outbound-only — no round-trip).
    #[test]
    fn preflight_verdict_wire_form_is_camelcase() {
        let ok = PreflightVerdict {
            est_total_output_bytes: 2048,
            est_total_scratch_bytes: 512,
            up_front_fail: None,
        };
        assert_eq!(
            serde_json::to_string(&ok).expect("PreflightVerdict serializes"),
            r#"{"estTotalOutputBytes":2048,"estTotalScratchBytes":512,"upFrontFail":null}"#,
            "§1.10: PreflightVerdict mirrors to camelCase; None = not up-front doomed"
        );
        let doomed = PreflightVerdict {
            est_total_output_bytes: 0,
            est_total_scratch_bytes: 0,
            up_front_fail: Some(ConversionErrorKind::TooBig),
        };
        assert_eq!(
            serde_json::to_string(&doomed).expect("PreflightVerdict serializes"),
            r#"{"estTotalOutputBytes":0,"estTotalScratchBytes":0,"upFrontFail":"tooBig"}"#,
            "§1.10/§5.2: a whole-batch doomed verdict carries Some(TooBig) (the disable-Convert flag)"
        );
    }

    // §6.4.1 unit (G15): the §0.6/§1.8 `OutputPlanPreview` wire form (P2.11) — the C4 `plan_output` return,
    // the full nested camelCase graph (set / finalDirDisplay / diverted / rerun / preflight). A SERIALIZE
    // pin (the embedded `PreflightVerdict` is outbound-only, so `OutputPlanPreview` does not round-trip).
    // `finalDirDisplay` is a lossy display string (2026-07-06 ruling, §2.10.1 — no `PathBuf` on the wire).
    #[test]
    fn output_plan_preview_wire_form_is_camelcase() {
        let preview = OutputPlanPreview {
            set: collected_set_id(),
            final_dir_display: "/dest".to_string(),
            diverted: Some(DivertReason::Unwritable),
            rerun: Some(RerunPrompt {
                equivalent_count: 2,
            }),
            preflight: PreflightVerdict {
                est_total_output_bytes: 1024,
                est_total_scratch_bytes: 256,
                up_front_fail: None,
            },
        };
        assert_eq!(
            serde_json::to_string(&preview).expect("OutputPlanPreview serializes"),
            r#"{"set":"00000000-0000-4000-8000-000000000000","finalDirDisplay":"/dest","diverted":"unwritable","rerun":{"equivalentCount":2},"preflight":{"estTotalOutputBytes":1024,"estTotalScratchBytes":256,"upFrontFail":null}}"#,
            "§1.8: OutputPlanPreview is the C4 'will save to…' graph in camelCase"
        );
    }

    // §6.4.1 unit (G15): the §0.6/§1.8/§2.14.4 `DestinationResolved` wire form (P2.11) — the C5
    // `set_destination` return; `preflight` RE-EVALUATED, `rerun` carried through unchanged (§2.5.1
    // destination-independent). A SERIALIZE pin.
    #[test]
    fn destination_resolved_wire_form_is_camelcase() {
        let resolved = DestinationResolved {
            destination: DestinationChoice::BesideSource,
            final_dir_display: "/dest".to_string(),
            diverted: None,
            preflight: PreflightVerdict {
                est_total_output_bytes: 4096,
                est_total_scratch_bytes: 0,
                up_front_fail: None,
            },
            rerun: None,
        };
        assert_eq!(
            serde_json::to_string(&resolved).expect("DestinationResolved serializes"),
            r#"{"destination":"besideSource","finalDirDisplay":"/dest","diverted":null,"preflight":{"estTotalOutputBytes":4096,"estTotalScratchBytes":0,"upFrontFail":null},"rerun":null}"#,
            "§1.8/§2.14.4: DestinationResolved re-validates the destination; rerun carried through (§2.5.1)"
        );
    }

    // §6.4.1 unit (G15): the §0.4.1 C4/C5 lifecycle-asymmetry STRUCTURAL ENABLERS (P2.28). The runtime
    // enforcement (C4 re-callable, C5 destination authority, C4 computes `rerun` while C5 carries it through)
    // is the P3.48 conductor + the C4/C5 body boxes; this test pins the layer the DTO shapes guarantee NOW —
    // the structure that makes the §0.4.1 "by lifecycle" rule TYPE-POSSIBLE (see the invariant block above the
    // §1.12 section). [Build-Session-Entscheidung: P2.28]
    #[test]
    fn c4_c5_asymmetry_structural_enablers() {
        let preflight = PreflightVerdict {
            est_total_output_bytes: 0,
            est_total_scratch_bytes: 0,
            up_front_fail: None,
        };

        // (2) C5 OWNS the destination: `DestinationResolved` CARRIES a `destination: DestinationChoice`.
        let resolved = DestinationResolved {
            destination: DestinationChoice::BesideSource,
            final_dir_display: "/dest".to_string(),
            diverted: None,
            preflight: preflight.clone(),
            rerun: Some(RerunPrompt {
                equivalent_count: 1,
            }),
        };
        assert!(
            matches!(resolved.destination, DestinationChoice::BesideSource),
            "§0.4.1: C5 owns the destination — DestinationResolved carries a DestinationChoice"
        );

        // C4 does NOT own the destination choice: `OutputPlanPreview` carries a `final_dir_display` PREVIEW
        // and NO `DestinationChoice` field. This EXHAUSTIVE literal (all 5 fields, no `..`) PINS the field
        // set — adding a `destination` field to `OutputPlanPreview` would make this fail to compile, so "C4
        // has no settable destination" is gate-enforced here, not just prose (§0.4.1 "C4 never overrides C5").
        let preview = OutputPlanPreview {
            set: collected_set_id(),
            final_dir_display: "/dest".to_string(),
            diverted: None,
            rerun: Some(RerunPrompt {
                equivalent_count: 1,
            }),
            preflight: preflight.clone(),
        };
        let _: &String = &preview.final_dir_display; // C4 shows a directory PREVIEW, never a settable destination

        // (3) C5 CARRIES C4's `rerun` THROUGH UNCHANGED (§2.5.1): both DTOs carry the SAME
        // `rerun: Option<RerunPrompt>` type, so the C4 value assigns verbatim into the C5 return.
        let carried_from_c4: Option<RerunPrompt> = preview.rerun.clone();
        let resolved_carrying = DestinationResolved {
            destination: DestinationChoice::BesideSource,
            final_dir_display: "/dest".to_string(),
            diverted: None,
            preflight: preflight.clone(),
            rerun: carried_from_c4,
        };
        assert_eq!(
            resolved_carrying.rerun, preview.rerun,
            "§2.5.1: C5 re-evaluates only preflight and carries C4's rerun through unchanged (same \
             Option<RerunPrompt> type)"
        );

        // both C4 and C5 returns carry the §1.10 `preflight: PreflightVerdict` (C4 computes it, C5 re-evaluates
        // it for the new destination volume, §2.14.4) — the same type on both sides of the asymmetry.
        assert_eq!(
            preview.preflight, resolved.preflight,
            "§1.8/§1.10: both the C4 and C5 returns carry a PreflightVerdict (C5 re-evaluates it, §2.14.4)"
        );
    }

    // §6.4.1 unit (G15): the §0.4.2 `ConversionEvent::RunStarted` wire form (P2.37) — the adjacently-tagged
    // ({ type, data }) camelCase Channel payload. Also pins P2.37.2: `will_reencode` is a non-optional `bool`
    // serialised in BOTH the false and true case (never omitted, never `undefined` — the §2.9.2 always-definite
    // emission rule). A SERIALIZE pin (the event types are outbound-only, like ScanProgress — no round-trip).
    #[test]
    fn conversion_event_run_started_wire_form_and_definite_willreencode() {
        let started = ConversionEvent::RunStarted(RunStarted {
            run_id: run_id(),
            total_items: 3,
            will_reencode: false,
        });
        assert_eq!(
            serde_json::to_string(&started).expect("ConversionEvent::RunStarted serializes"),
            r#"{"type":"runStarted","data":{"runId":"11111111-1111-4111-8111-111111111111","totalItems":3,"willReencode":false}}"#,
            "§0.4.2: ConversionEvent is adjacently tagged camelCase; RunStarted carries runId/totalItems/willReencode"
        );
        let reencode = ConversionEvent::RunStarted(RunStarted {
            run_id: run_id(),
            total_items: 1,
            will_reencode: true,
        });
        assert!(
            serde_json::to_string(&reencode)
                .expect("serializes")
                .contains(r#""willReencode":true"#),
            "§2.9.2 / P2.37.2: willReencode is a definite non-optional bool — present in BOTH the false and true case"
        );
    }

    // §6.4.1 unit (G15): the remaining §0.4.2 payloads in the adjacently-tagged `ConversionEvent` (P2.37),
    // plus the P2.37.1/P2.37.3 queued-only-denominator + P2.37.4 skip-carriability structural enablers. Asserts
    // via `serde_json::Value` (the nested TargetId/JobStage/ItemOutcome wire forms have their OWN pins — this
    // pins the §0.4.2 envelope shape + field names, not those nested forms).
    #[test]
    fn conversion_event_item_and_batch_wire_forms() {
        use crate::domain::{FormatId, JobStage};

        // ItemStarted — runId / itemId / sourceDisplay / target, adjacently tagged camelCase.
        let item_started = ConversionEvent::ItemStarted(ItemStarted {
            run_id: run_id(),
            item_id: item_id(1),
            source_display: "/in/a.csv".to_string(),
            target: TargetId::Format(FormatId::Tsv),
        });
        let v = serde_json::to_value(&item_started).expect("ItemStarted serializes");
        assert_eq!(v["type"], "itemStarted", "§0.4.2: adjacent tag");
        assert_eq!(
            v["data"]["sourceDisplay"], "/in/a.csv",
            "§0.4.2: camelCase sourceDisplay (a lossy display string, §2.10.1)"
        );
        assert_eq!(v["data"]["itemId"], 1, "§0.4.2: camelCase itemId");

        // ItemProgress — `fraction: None` (the §1.11 truly-indeterminate LibreOffice case) + a JobStage.
        let item_progress = ConversionEvent::ItemProgress(ItemProgress {
            run_id: run_id(),
            item_id: item_id(1),
            fraction: None,
            stage: JobStage::Encoding,
        });
        let v = serde_json::to_value(&item_progress).expect("ItemProgress serializes");
        assert_eq!(v["type"], "itemProgress");
        assert!(
            v["data"]["fraction"].is_null(),
            "§1.11: fraction is None where truly indeterminate"
        );

        // P2.37.4 structural enabler: ItemFinished CAN carry `ItemOutcome::Skipped` (the SAME shared type as the
        // terminal `RunResult.items`). The POLICY — no LIVE ItemFinished{Skipped} for a pre-flight skip — is the
        // P3.48 runtime emission rule documented on ItemFinished; this asserts only the structural carriability.
        let item_finished = ConversionEvent::ItemFinished(ItemFinished {
            run_id: run_id(),
            item_id: item_id(2),
            outcome: ItemOutcome::Skipped {
                reason: SkipReason::Empty,
            },
        });
        let v = serde_json::to_value(&item_finished).expect("ItemFinished serializes");
        assert_eq!(v["type"], "itemFinished");
        assert!(
            !v["data"]["outcome"].is_null(),
            "P2.37.4: ItemFinished structurally carries an ItemOutcome (here Skipped — the shared terminal type; \
             the no-live-emit policy is the P3.48 runtime rule)"
        );

        // P2.37.1 + P2.37.3: BatchProgress.total and RunStarted.total_items are the SAME queued-only u32
        // denominator. The RUNTIME equality is a P3.48 invariant; here both carry the same N on the wire.
        let n: u32 = 5;
        let started = RunStarted {
            run_id: run_id(),
            total_items: n,
            will_reencode: false,
        };
        let batch = ConversionEvent::BatchProgress(BatchProgress {
            run_id: run_id(),
            done: 2,
            total: n,
        });
        let bv = serde_json::to_value(&batch).expect("BatchProgress serializes");
        assert_eq!(bv["type"], "batchProgress");
        assert_eq!(
            bv["data"]["total"].as_u64(),
            Some(u64::from(started.total_items)),
            "P2.37.1 / P2.37.3: BatchProgress.total uses the SAME queued-only denominator as RunStarted.total_items"
        );
        assert_eq!(
            bv["data"]["done"], 2,
            "§1.11: done is the completed-item numerator (queued-only)"
        );
    }

    // §6.4.1 unit (G15): a NON-FINITE `ItemProgress.fraction` (NaN / ±∞ — a malformed engine-progress
    // computation, never a valid §1.11 value) serializes to the SAME wire `null` as the DELIBERATE
    // indeterminate `None` (JSON has no NaN/Infinity number form; serde_json emits `null` for a non-finite
    // float). Pinned as the §0.4.2 wire contract: the fail-safe collapse means the WebView can never receive
    // a NaN/∞ that would poison the §1.11 bar arithmetic — it sees the indeterminate form instead.
    // [Build-Session-Entscheidung: P2.137]
    #[test]
    fn item_progress_non_finite_fraction_serializes_as_null() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let progress = ItemProgress {
                run_id: run_id(),
                item_id: item_id(1),
                fraction: Some(bad),
                stage: JobStage::Encoding,
            };
            let v = serde_json::to_value(&progress).expect("ItemProgress serializes");
            assert!(
                v["fraction"].is_null(),
                "§0.4.2/§1.11: a non-finite fraction ({bad}) collapses to the indeterminate wire null"
            );
        }
    }

    // §6.4.1 unit (G15): the §0.4.2 `ConversionEvent::RunFinished` variant (P2.37) wraps the §1.12 `RunResult`
    // (it mirrors C8) — the terminal run event. A minimal RunResult here; the full RunResult wire form has its
    // own pin (`run_result_wire_form_is_camelcase`). Also exercises the RunFinished variant in the test build.
    #[test]
    fn conversion_event_run_finished_wraps_run_result() {
        let run = RunResult {
            collected_set_id: collected_set_id(),
            run_id: run_id(),
            items: Vec::new(),
            totals: Totals {
                succeeded: 0,
                failed: 0,
                cancelled: 0,
                skipped: 0,
            },
            cleanup_incomplete: Vec::new(),
            // Irrelevant to this variant-wrapping pin; the line itself is pinned by
            // `batch_summary_line_*` + the wire-form pin. [Build-Session-Entscheidung: P3.59]
            summary_line_display: String::new(),
            common_root_display: "/src".to_string(),
            divert_root_display: None,
        };
        let finished = ConversionEvent::RunFinished(run);
        let v = serde_json::to_value(&finished).expect("ConversionEvent::RunFinished serializes");
        assert_eq!(
            v["type"], "runFinished",
            "§0.4.2: RunFinished is the terminal run event (mirrors C8)"
        );
        assert_eq!(
            v["data"]["collectedSetId"], "00000000-0000-4000-8000-000000000000",
            "§0.4.2/§1.12: RunFinished carries the full RunResult"
        );
    }

    // §6.4.1 unit (G15): the §0.6/§1.9 `JobState` WIRE form (P2.12 — JobState is now a wire type, the §1.12
    // summary's per-item state). Externally tagged camelCase: unit variants as bare strings, the newtype
    // variants as `{"failed":<kind>}` / `{"skipped":<reason>}`. A SERIALIZE pin (JobState is outbound-only).
    #[test]
    fn job_state_wire_form_is_externally_tagged_camelcase() {
        let cases: [(JobState, &str); 6] = [
            (JobState::Pending, r#""pending""#),
            (JobState::Running, r#""running""#),
            (JobState::Succeeded, r#""succeeded""#),
            (
                JobState::Failed(ConversionErrorKind::Corrupt),
                r#"{"failed":"corrupt"}"#,
            ),
            (
                JobState::Skipped(SkipReason::Empty),
                r#"{"skipped":"empty"}"#,
            ),
            (JobState::Cancelled, r#""cancelled""#),
        ];
        for (state, wire) in cases {
            assert_eq!(
                serde_json::to_string(&state).expect("JobState serializes"),
                wire,
                "§0.6/§1.12: JobState mirrors externally-tagged camelCase on the wire"
            );
        }
    }

    // §6.4.1 unit (G15): the §0.6/§0.4.2 `ItemOutcome` WIRE form (P2.12 → P3.76) — the live `ItemFinished`
    // payload, externally tagged camelCase; `Succeeded`'s `output_display` → `outputDisplay` (per-variant
    // rename), `Failed` carries the full §0.4.3 IpcError (`pathDisplay`/`residueDisplay`), `Cancelled` is
    // payload-free. A SERIALIZE pin (outbound-only). No `PathBuf` on the wire (2026-07-06 ruling, §2.10.1).
    #[test]
    fn item_outcome_wire_form_is_externally_tagged_camelcase() {
        let succeeded = ItemOutcome::Succeeded {
            output_display: "/out/data.tsv".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&succeeded).expect("ItemOutcome::Succeeded serializes"),
            r#"{"succeeded":{"outputDisplay":"/out/data.tsv"}}"#,
            "§0.4.2: Succeeded carries the published outputDisplay (a lossy display string, §2.10.1)"
        );
        let failed = ItemOutcome::Failed {
            error: IpcError {
                kind: ConversionErrorKind::EngineError,
                message: "ConvertIA couldn't convert this file.".to_owned(),
                path_display: Some("/src/bad.csv".to_string()),
                residue_display: None,
            },
        };
        assert_eq!(
            serde_json::to_string(&failed).expect("ItemOutcome::Failed serializes"),
            r#"{"failed":{"error":{"kind":"engineError","message":"ConvertIA couldn't convert this file.","pathDisplay":"/src/bad.csv","residueDisplay":null}}}"#,
            "§0.4.2/§0.4.3: Failed carries the full IpcError"
        );
        let skipped = ItemOutcome::Skipped {
            reason: SkipReason::Uncertain,
        };
        assert_eq!(
            serde_json::to_string(&skipped).expect("ItemOutcome::Skipped serializes"),
            r#"{"skipped":{"reason":"uncertain"}}"#,
            "§0.6: Skipped carries the SkipReason (skip ≠ fail)"
        );
        assert_eq!(
            serde_json::to_string(&ItemOutcome::Cancelled)
                .expect("ItemOutcome::Cancelled serializes"),
            r#""cancelled""#,
            "§0.4.3 note: Cancelled is payload-free, not an ErrorKind"
        );
    }

    // §6.4.1 unit (G15): `ItemOutcome` is EXACTLY the four §0.6 terminal outcomes — the no-wildcard
    // `exhaustive` match is the COMPILE-TIME membership lock (the established dependency-free pattern, cf.
    // the `JobState` six-state lock above / the domain per-enum `exhaustive` fns): a variant added or
    // removed without updating it fails to compile, so the closed live-wire enum can never silently drift
    // from §0.6. The runtime leg pins the four externally-tagged wire TAGS as an ordered set, complementing
    // the per-variant payload pins above. [Build-Session-Entscheidung: P2.137]
    #[test]
    fn item_outcome_is_the_four_terminal_variants() {
        fn exhaustive(o: &ItemOutcome) {
            match o {
                ItemOutcome::Succeeded { .. }
                | ItemOutcome::Failed { .. }
                | ItemOutcome::Skipped { .. }
                | ItemOutcome::Cancelled => {}
            }
        }
        let all = [
            ItemOutcome::Succeeded {
                output_display: "/out/data.tsv".to_string(),
            },
            ItemOutcome::Failed {
                error: IpcError {
                    kind: ConversionErrorKind::EngineError,
                    message: "ConvertIA couldn't convert this file.".to_owned(),
                    path_display: None,
                    residue_display: None,
                },
            },
            ItemOutcome::Skipped {
                reason: SkipReason::Empty,
            },
            ItemOutcome::Cancelled,
        ];
        let tags: Vec<String> = all
            .iter()
            .map(|outcome| {
                exhaustive(outcome);
                // The externally-tagged §0.6 wire form is a single-key object (payload variants) or a bare
                // string (Cancelled); any other JSON shape yields an empty tag the assertion below rejects.
                match serde_json::to_value(outcome).expect("ItemOutcome serializes") {
                    serde_json::Value::Object(map) => {
                        map.keys().cloned().collect::<Vec<_>>().join(",")
                    }
                    serde_json::Value::String(tag) => tag,
                    serde_json::Value::Null
                    | serde_json::Value::Bool(_)
                    | serde_json::Value::Number(_)
                    | serde_json::Value::Array(_) => String::new(),
                }
            })
            .collect();
        assert_eq!(
            tags,
            ["succeeded", "failed", "skipped", "cancelled"],
            "§0.6/§0.4.2: exactly the four terminal wire tags, in §0.6 order"
        );
    }

    // §6.4.1 unit (G15): the §1.12 `Totals` wire form + the DERIVED `all_failed()`/`total()` (P2.12) — the
    // "all failed" condition is `failed == total && total > 0`, NEVER a stored field (SSOT *Fail clearly*).
    #[test]
    fn totals_wire_form_and_derived_all_failed() {
        let mixed = Totals {
            succeeded: 1,
            failed: 2,
            cancelled: 0,
            skipped: 1,
        };
        assert_eq!(
            serde_json::to_string(&mixed).expect("Totals serializes"),
            r#"{"succeeded":1,"failed":2,"cancelled":0,"skipped":1}"#,
            "§1.12: Totals is the four camelCase tallies"
        );
        assert_eq!(mixed.total(), 4, "§1.12: total() sums the four tallies");
        assert!(
            !mixed.all_failed(),
            "§1.12: a partial batch is not all-failed"
        );
        let all_failed = Totals {
            succeeded: 0,
            failed: 3,
            cancelled: 0,
            skipped: 0,
        };
        assert!(
            all_failed.all_failed(),
            "§1.12: failed == total (>0) is the all-failed condition"
        );
        let empty = Totals {
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            skipped: 0,
        };
        assert!(
            !empty.all_failed(),
            "§1.12: total == 0 is NOT all-failed (the total > 0 guard)"
        );
    }

    // §6.4.1 unit (G15): `total()` is EXACT `u64` arithmetic at the `u32` ceiling (the P2.137 widened
    // return): `succeeded: 1, failed: u32::MAX` sums PAST `u32::MAX` without saturation, and `all_failed()`
    // stays false (`failed != total`). A `u32`/saturating total would freeze at `u32::MAX == failed` here
    // and make the derived §1.12 all-failed condition lie at the ceiling — the silent-saturation class the
    // `ItemIdSpace` checked_add discipline rejects (see `Totals::total`'s doc).
    // [Build-Session-Entscheidung: P2.137]
    #[test]
    fn totals_total_is_exact_u64_at_the_u32_boundary() {
        let boundary = Totals {
            succeeded: 1,
            failed: u32::MAX,
            cancelled: 0,
            skipped: 0,
        };
        assert_eq!(
            boundary.total(),
            u64::from(u32::MAX) + 1,
            "§1.12: total() sums the four u32 tallies exactly, past the u32 ceiling"
        );
        assert!(
            !boundary.all_failed(),
            "§1.12: one success among u32::MAX failures is NOT all-failed (no saturated-total lie)"
        );
    }

    // §6.4.1 unit (G15): the §1.12 `RunResult` wire form (P2.12 → P3.76) — the full nested camelCase graph the
    // §5.3 Summary renders, exercising `ItemResult` (a Succeeded row + a pre-flight Skipped row whose `reason`
    // rides the adjacently-tagged `OutcomeMsg::Skipped`), `Totals`, `CleanupResidue`, and `divertRootDisplay`
    // Some(..). A SERIALIZE pin (RunResult is outbound-only — the §0.4.2 RunFinished payload / C8 return). Every
    // path field is a lossy DISPLAY string (2026-07-06 ruling, §2.10.1 — the real paths live off-wire in
    // `RunResultStore`); `ItemResult` is ID-keyed (`item`), the output→source mapping via `DroppedItem.display_name`.
    #[test]
    fn run_result_wire_form_is_camelcase() {
        let run = RunResult {
            collected_set_id: collected_set_id(),
            run_id: run_id(),
            items: vec![
                ItemResult {
                    item: item_id(0),
                    output_display: Some("/src/data.tsv".to_string()),
                    state: JobState::Succeeded,
                    reason: None,
                },
                ItemResult {
                    item: item_id(1),
                    output_display: None,
                    state: JobState::Skipped(SkipReason::Uncertain),
                    reason: Some(OutcomeMsg::Skipped {
                        reason: SkipReason::Uncertain,
                        text: "ConvertIA couldn't tell what kind of file this is, so it can't convert it."
                            .to_owned(),
                    }),
                },
            ],
            totals: Totals {
                succeeded: 1,
                failed: 0,
                cancelled: 0,
                skipped: 1,
            },
            cleanup_incomplete: vec![CleanupResidue {
                item: item_id(2),
                residue_display: "/src/.data.tsv.part".to_string(),
            }],
            common_root_display: "/src".to_string(),
            divert_root_display: Some("/Downloads".to_string()),
            // [Test-Change: P3.59 — old-obsolete+new-correct, §1.12] The wire GAINED this field by the
            // 2026-07-16 P3.59 ruling (the §2.8.2 batch line, core-assembled, finally reaching the wire from
            // the P3.50 `batch_summary_line`), so the pre-P3.59 expected JSON below is obsolete BY THE WIRE
            // CONTRACT — a pin that omitted it would assert a shape the core no longer emits. NEW CORRECT: the
            // value is exactly what `batch_summary_line` produces for THIS fixture's inputs (verified by the
            // `run_result_wire_summary_line_matches_the_assembler` read-back below, not by eyeballing): totals
            // 1 succeeded / 0 failed / 0 cancelled ⇒ `AllSucceeded { n: 1 }` ⇒ "All 1 files converted.", plus
            // the §2.6.4 tail because `cleanup_incomplete` is non-empty. Nothing else in the pin moved.
            summary_line_display: "All 1 files converted. Some temporary files may remain — see details."
                .to_string(),
        };
        assert_eq!(
            serde_json::to_string(&run).expect("RunResult serializes"),
            concat!(
                r#"{"collectedSetId":"00000000-0000-4000-8000-000000000000","#,
                r#""runId":"11111111-1111-4111-8111-111111111111","#,
                r#""items":[{"item":0,"outputDisplay":"/src/data.tsv","state":"succeeded","reason":null},"#,
                r#"{"item":1,"outputDisplay":null,"state":{"skipped":"uncertain"},"#,
                r#""reason":{"type":"skipped","data":{"reason":"uncertain","text":"ConvertIA couldn't tell what kind of file this is, so it can't convert it."}}}],"#,
                r#""totals":{"succeeded":1,"failed":0,"cancelled":0,"skipped":1},"#,
                r#""cleanupIncomplete":[{"item":2,"residueDisplay":"/src/.data.tsv.part"}],"#,
                r#""commonRootDisplay":"/src","divertRootDisplay":"/Downloads","#,
                r#""summaryLineDisplay":"All 1 files converted. Some temporary files may remain — see details."}"#
            ),
            "§1.12: RunResult is the end-of-batch summary graph in camelCase (pre-flight skip rides \
             OutcomeMsg::Skipped, not Failure — skip ≠ fail; the §2.8.2 batch line rides summaryLineDisplay)"
        );
    }

    // §6.4.1 unit (G15): the wire pin above hardcodes its `summaryLineDisplay` literal, so this reads the value
    // BACK from the real assembler for the SAME inputs — the §0.2 read-back bar (never "it's green now"). If
    // `batch_summary`/`append_residue_tail`/the §2.8.2 rows ever change, this fails rather than letting the pin
    // silently assert a string the core would no longer produce. [Build-Session-Entscheidung: P3.59]
    #[test]
    fn run_result_wire_summary_line_matches_the_assembler() {
        let totals = Totals {
            succeeded: 1,
            failed: 0,
            cancelled: 0,
            skipped: 1,
        };
        assert_eq!(
            batch_summary_line(&totals, true),
            "All 1 files converted. Some temporary files may remain — see details.",
            "§1.12/§2.8.2: the wire pin's summaryLineDisplay literal IS the assembler's output for its own \
             fixture (totals + a non-empty cleanup_incomplete)"
        );
    }

    // ─── P2.14 · §0.6-invariant property tests (§6.4.2 / G16) ────────────────────────────────────────────
    // The §6.4.2 property level (test-strategy §1.3) for the §0.6 normative invariants carried by the
    // orchestrator lifecycle types `Batch` / `ConversionJob`. Each asserts an invariant over a WIDE generated
    // input space, complementing the example-based unit tests above. All three G16 / test-strategy §1.3
    // determinism knobs are set: case-count floor 512 (> the 256 default); a PINNED CI seed — `pinned_runner()`
    // drives a `TestRunner` with a `deterministic_rng` so the 512 cases are identical every run, locally and in
    // CI (the `proptest!` macro seeds from ENTROPY and CANNOT pin the forward seed — only an already-found
    // counterexample); a failure is NEVER retried-to-pass (the pinned seed reproduces it deterministically,
    // test-strategy §1.3 / §7). `Strategy`-combinator automatic shrinking, no hand-rolled `Shrink` impls.
    // Instances are built by canonical `Batch` / `ConversionJob` constructors that model the §1.9 queue
    // construction; the LIVE-path enforcement (the real P3.48 orchestrator builder over a real run) is the P3
    // integration leg (test-strategy §1.1 / §6 — the data-structure leg is here, the live-path leg is there).
    // [Build-Session-Entscheidung: P2.14] case-count floor 512 + a `deterministic_rng`-pinned seed; ids built
    // via the orchestrator-test `item_id` serde helper (the `ItemId` field is private to `crate::domain`, so
    // the cross-module test mints it through its public bare-number wire form, never a back-door past the
    // §1.1/§7.1 minting policy).

    /// The §0.6-invariant property-test case-count floor (test-strategy §1.3: above proptest's 256 default).
    const P2_14_CASES: u32 = 512;

    /// The §1.2 recognized format of a (test) job source — for the §1.3 one-format-per-batch grouping check.
    /// [Test-Change: P3.47 — old-obsolete+new-correct, §0.6] takes `&JobSource` now (the P2.137 callers read
    /// `&job.source`, which is a `JobSource` sum after P3.47); a `Skipped` arm has no recognized format
    /// (`None`), and the `Eligible` arm projects its `DroppedItem.detected` exactly as before.
    fn recognized_format(source: &JobSource) -> Option<UserFacingFormat> {
        // Exhaustive over both axes (the crate denies `clippy::wildcard_enum_match_arm`, so no `_` arm) — a
        // future `JobSource`/`DetectionOutcome` variant forces a conscious decision here, never a silent `None`.
        match source {
            JobSource::Skipped(_) => None,
            JobSource::Eligible(d) => match &d.detected {
                DetectionOutcome::Recognized { format, .. } => Some(*format),
                DetectionOutcome::UnsupportedType { .. }
                | DetectionOutcome::Uncertain { .. }
                | DetectionOutcome::Empty
                | DetectionOutcome::Unreadable { .. } => None,
            },
        }
    }

    /// A PINNED-SEED proptest runner (test-strategy §1.3 / G16: "a pinned CI seed"). The `proptest!` macro
    /// seeds its forward run from ENTROPY (only an already-found counterexample is pinned, via the
    /// `proptest-regressions/` file), so to make the 512-case exploration identical on every run — locally and
    /// in CI, so a failure is reproducible and NEVER retried-to-pass (test-strategy §1.3 / §7) — the
    /// §0.6-invariant properties drive a `TestRunner` with a `deterministic_rng`. [Build-Session-Entscheidung: P2.14]
    fn pinned_runner() -> TestRunner {
        TestRunner::new_with_rng(
            ProptestConfig::with_cases(P2_14_CASES),
            TestRng::deterministic_rng(RngAlgorithm::ChaCha),
        )
    }

    /// A small cross-category palette of §0.6 formats — the GENERATIVE format axis for the grouping property
    /// below (any two distinct entries make the §1.3 grouping key falsifiable; the full 46-variant set
    /// MEMBERSHIP is `crate::domain`'s own compile-time lock and is not re-policed here).
    /// [Build-Session-Entscheidung: P2.137]
    const P2_137_FORMAT_PALETTE: [UserFacingFormat; 8] = [
        UserFacingFormat::Csv,
        UserFacingFormat::Tsv,
        UserFacingFormat::Png,
        UserFacingFormat::Jpg,
        UserFacingFormat::Mp3,
        UserFacingFormat::Mp4,
        UserFacingFormat::Pdf,
        UserFacingFormat::Docx,
    ];

    /// §0.6 invariant 1 (one-Target-per-Batch) + the §1.3 single-format grouping — GENERATIVE over the
    /// format axis: for ANY generated batch format and job count, a batch built to the grouping rule has
    /// every job's recognized source format equal to the single whole-batch `source_format` (and carries ONE
    /// whole-batch `target` — `ConversionJob` has no target field, so the single value governs by shape).
    /// The invariant has TEETH: a planted format-INTRUDER job (a different generated format) is DETECTABLE
    /// via `recognized_format(&job.source) != Some(batch.source_format)`, so the grouping key is a real
    /// constraint a §1.3-violating queue would fail — not a fixture echo.
    /// [Test-Change: P2.137 — old-obsolete+new-correct, §0.6] the prior generator varied only the job COUNT
    /// over a hardcoded-Csv fixture, so no generated input could falsify the format equality (a fixture
    /// tautology). The new axes (an arbitrary palette format + a planted intruder of a guaranteed-different
    /// format) make both directions falsifiable, verified against the §0.6 inv-1 / §1.3 grouping contract.
    #[test]
    fn prop_batch_is_one_target_and_one_source_format_over_arbitrary_jobs() {
        let palette_len = P2_137_FORMAT_PALETTE.len();
        pinned_runner()
            .run(&(0..palette_len, 0..palette_len - 1, 0usize..64), |(fi, off, n)| {
                let batch_format = P2_137_FORMAT_PALETTE[fi];
                // Always a DIFFERENT palette entry: offsets 1..len around the ring from fi, so the
                // intruder's format never equals the batch format (no filtering, fully deterministic).
                let intruder_format = P2_137_FORMAT_PALETTE[(fi + 1 + off) % palette_len];
                let jobs: Vec<ConversionJob> = (0..n)
                    .map(|i| {
                        let id = u32::try_from(i).expect("n < 64 fits u32");
                        // [Test-Change: P3.47 — old-obsolete+new-correct, §0.6] `source` wraps the
                        // `DroppedItem` in `JobSource::Eligible` (the old bare form no longer compiles); the
                        // §1.3 grouping semantics are unchanged (recognized_format reads the eligible arm).
                        ConversionJob {
                            item: item_id(id),
                            source: JobSource::Eligible(dropped_item_with(id, batch_format)),
                            state: JobState::Pending,
                            plan: None,
                        }
                    })
                    .collect();
                let mut batch = Batch {
                    id: collected_set_id(),
                    source_format: batch_format,
                    target: sample_target(),
                    options: OptionValues(BTreeMap::new()),
                    destination: ResolvedDestination::BesideSource,
                    jobs,
                };
                prop_assert_eq!(batch.jobs.len(), n, "the batch carries exactly its n constructed jobs");
                prop_assert_eq!(
                    batch.target.id,
                    TargetId::Format(UserFacingFormat::Tsv),
                    "§0.6 inv-1: a single whole-batch Target governs every job (no per-job target field)"
                );
                for job in &batch.jobs {
                    prop_assert_eq!(
                        recognized_format(&job.source),
                        Some(batch.source_format),
                        "§1.3: every job in the batch shares the single whole-batch source format"
                    );
                }
                // [Test-Change: P2.137 — old-obsolete+new-correct, §0.6] teeth: plant a format-intruder
                // job and prove the grouping key DETECTS it (the old fixture-only generator could not
                // falsify the format equality; the still-true shape assertions above are retained verbatim).
                let intruder_id = u32::try_from(n).expect("n < 64 fits u32");
                batch.jobs.push(ConversionJob {
                    item: item_id(intruder_id),
                    source: JobSource::Eligible(dropped_item_with(intruder_id, intruder_format)),
                    state: JobState::Pending,
                    plan: None,
                });
                prop_assert_eq!(
                    batch
                        .jobs
                        .iter()
                        .filter(|job| recognized_format(&job.source) != Some(batch.source_format))
                        .count(),
                    1,
                    "§1.3 teeth: exactly the planted intruder violates the single-format grouping — the \
                     key discriminates, it is not a fixture echo"
                );
                Ok(())
            })
            .expect("the pinned 512-case exploration holds the §0.6 inv-1 / §1.3 grouping invariant");
    }

    /// §0.6 "`ConversionJob.item == source.item()`": the job's top-level key is DENORMALIZED from its source
    /// item's id (cheap addressing without unwrapping `source`). Holds for ANY generated source id AND
    /// UNIFORMLY over BOTH `JobSource` arms (the P3.47 strengthening — an eligible `DroppedItem` and a
    /// pre-flight `SkippedItem` are addressed identically, no queued-only carve-out); the teeth assertion
    /// shows a deliberately-mismatched item is detectable, so the equality is a real constraint, not a
    /// vacuous `x == x`.
    /// [Test-Change: P3.47 — old-obsolete+new-correct, §0.6] the prior form asserted `item == source.item`
    /// (the P2.10 bare `DroppedItem` field) over the eligible arm only; the field is obsolete (`source` is a
    /// `JobSource` sum now) and the invariant is STRENGTHENED to the uniform `item == source.item()` accessor
    /// generated over BOTH arms — verified against the §0.6 `JobSource` ruling (both arms carry `item`).
    #[test]
    fn prop_conversion_job_item_equals_source_item() {
        pinned_runner()
            .run(&(any::<u32>(), any::<bool>()), |(id, skipped)| {
                // Generate BOTH arms: an eligible DroppedItem or a pre-flight SkippedItem at the same id.
                let source = if skipped {
                    JobSource::Skipped(skipped_item(id, SkipReason::Unreadable))
                } else {
                    JobSource::Eligible(dropped_item(id))
                };
                let state = if skipped {
                    JobState::Skipped(SkipReason::Unreadable)
                } else {
                    JobState::Pending
                };
                let job = ConversionJob {
                    item: source.item(),
                    source: source.clone(),
                    state,
                    plan: None,
                };
                prop_assert_eq!(
                    job.item,
                    job.source.item(),
                    "§0.6: ConversionJob.item == source.item() (uniform over both JobSource arms)"
                );
                prop_assert_eq!(
                    job.item,
                    item_id(id),
                    "the denormalized key tracks the generated source id, whichever arm"
                );
                // teeth: a job whose item is NOT its source's id is detectably inconsistent — `wrapping_add(1)`
                // never equals `id` for any u32, so the denormalization invariant discriminates correct from wrong.
                let mismatched = ConversionJob {
                    item: item_id(id.wrapping_add(1)),
                    source,
                    state,
                    plan: None,
                };
                prop_assert_ne!(
                    mismatched.item,
                    mismatched.source.item(),
                    "a mismatched item IS detectable — the denormalization invariant is not vacuous"
                );
                Ok(())
            })
            .unwrap();
    }

    /// §0.6 coupling invariant (P3.47, REFINED by the P3.48 rerun-skip ruling) over the REAL C6 constructor:
    /// for a `build_batch` result from ANY generated eligible+skipped mix, EVERY job satisfies `source is
    /// Skipped(_) ⟺ state is JobState::Skipped(<detection reason>)` — an eligible item is `Eligible`+`Pending`,
    /// a skipped item is `Skipped`+`Skipped(reason)` with the reason COPIED from the `SkippedItem`, the jobs are
    /// in deterministic id/traversal order (eligible even ids + skipped odd ids are interleaved BACK by the
    /// sort), and `item == source.item()` holds uniformly. TEETH: a hand-built job whose source arm and state
    /// disagree IS detectable, so the ⟺ is a real constraint, not a fixture echo.
    ///
    /// [Test-Change: P3.48 — old-obsolete+new-correct, §0.6] The P3.48 ruling REFINED the coupling: the C6 §2.5
    /// applier may assign an ELIGIBLE item `Skipped(AlreadyConverted)` (source stays `Eligible`), so the bare
    /// `source is Skipped(_) ⟺ state is Skipped(_)` no longer holds POST-applier. But `build_batch` (this
    /// constructor, PRE-applier) is unaffected — it NEVER mints `AlreadyConverted` (only the §1.1 freeze's four
    /// detection reasons reach it), so the bare ⟺ still holds over its output AND every `Skipped` state here
    /// carries a DETECTION reason. The added assertion pins the freeze-never-mints-`AlreadyConverted` invariant
    /// (construction-only, type-unenforced); the POST-applier `Skipped(AlreadyConverted) ⟹ Eligible` refinement
    /// is covered by `run_conversion_tests::rerun_skip_marks_a_seen_item_already_converted_but_fresh_copy_converts_it`.
    #[test]
    fn prop_build_batch_couples_source_arm_with_job_state() {
        const REASONS: [SkipReason; 4] = [
            SkipReason::UnsupportedType,
            SkipReason::Uncertain,
            SkipReason::Empty,
            SkipReason::Unreadable,
        ];
        pinned_runner()
            .run(&(0u32..32, 0u32..32), |(n_elig, n_skip)| {
                // Disjoint ids over ONE space: eligible get EVEN ids, skipped get ODD ids — so build_batch's
                // id-sort has real interleaving work (it pushes all eligible, then all skipped, then sorts).
                let items: Vec<DroppedItem> = (0..n_elig).map(|i| dropped_item(2 * i)).collect();
                let skipped: Vec<SkippedItem> = (0..n_skip)
                    .enumerate()
                    .map(|(idx, i)| skipped_item(2 * i + 1, REASONS[idx % 4]))
                    .collect();
                let frozen = frozen_of(items, skipped);
                let batch = build_batch(
                    &frozen,
                    sample_target(),
                    OptionValues(BTreeMap::new()),
                    ResolvedDestination::BesideSource,
                );

                let total = usize::try_from(n_elig + n_skip).expect("n_elig + n_skip < 64 fits usize");
                prop_assert_eq!(
                    batch.jobs.len(),
                    total,
                    "build_batch materialises exactly one job per eligible + skipped item"
                );

                // Jobs are in ascending id (§1.1 traversal) order — the interleaved evens+odds re-sorted.
                let ids: Vec<ItemId> = batch.jobs.iter().map(|job| job.item).collect();
                let mut sorted = ids.clone();
                sorted.sort_unstable();
                prop_assert_eq!(
                    ids,
                    sorted,
                    "§1.9: build_batch jobs are in deterministic id/traversal order over the single id space"
                );

                for job in &batch.jobs {
                    // The coupling invariant, both directions.
                    let skipped_source = matches!(job.source, JobSource::Skipped(_));
                    let skipped_state = matches!(job.state, JobState::Skipped(_));
                    prop_assert_eq!(
                        skipped_source,
                        skipped_state,
                        "§0.6 coupling: source is Skipped(_) ⟺ state is JobState::Skipped(_)"
                    );
                    // [Test-Change: P3.48] The FREEZE / `build_batch` NEVER mints `AlreadyConverted` — that is
                    // the C6 §2.5 applier's assignment (over an ELIGIBLE item, post-construction). Every skip
                    // here carries a DETECTION reason (the refined coupling's pre-applier half).
                    prop_assert!(
                        !matches!(job.state, JobState::Skipped(SkipReason::AlreadyConverted)),
                        "the freeze/build_batch never mints AlreadyConverted (the §2.5 re-run skip is the C6 applier's, P3.48)"
                    );
                    // Uniform denormalization + never-planned-at-construction, over both arms.
                    prop_assert_eq!(
                        job.item,
                        job.source.item(),
                        "§0.6: item == source.item() over both arms"
                    );
                    prop_assert!(
                        job.plan.is_none(),
                        "§1.9: no job is planned at construction (a skip never plans; §1.8 plans eligible subsequently)"
                    );
                    // Each arm's concrete state (the match is exhaustive over the 2-variant JobSource, no `_`).
                    match &job.source {
                        JobSource::Eligible(_) => prop_assert_eq!(
                            job.state,
                            JobState::Pending,
                            "an eligible item materialises as a Pending job"
                        ),
                        JobSource::Skipped(s) => prop_assert_eq!(
                            job.state,
                            JobState::Skipped(s.reason),
                            "a skipped item materialises as Skipped(reason), the reason copied from the SkippedItem"
                        ),
                    }
                }

                // Teeth: a job whose source arm and state DISAGREE is detectable by the ⟺ check (an Eligible
                // source paired with a Skipped state) — proving the coupling assertion above is not vacuous.
                let bad = ConversionJob {
                    item: item_id(0),
                    source: JobSource::Eligible(dropped_item(0)),
                    state: JobState::Skipped(SkipReason::Empty),
                    plan: None,
                };
                prop_assert_ne!(
                    matches!(bad.source, JobSource::Skipped(_)),
                    matches!(bad.state, JobState::Skipped(_)),
                    "a coupling-violating job (Eligible source + Skipped state) IS detectable — the ⟺ is not vacuous"
                );
                Ok(())
            })
            .unwrap();
    }

    /// §0.6 invariant 6 — the dedup×mint×skip COMPOSITION over ONE shared id space (§2.3.2 + §1.1): run the
    /// REAL P2.76 `dedup_by_identity` fold over an arbitrary eligible/ineligible candidate mix (identity
    /// classes WITH repeats — 8 classes over up to 64 candidates force duplicates), then mint one id per
    /// ineligible item from the SAME `ItemIdSpace` cursor — the documented P3.49 assembly order (survivors
    /// first, then the §1.1 skips). Over any generated mix: survivor ids and skip ids are DISJOINT, together
    /// they cover EXACTLY the contiguous `0..(survivors + skips)` space, one survivor exists per DISTINCT
    /// eligible identity class (each duplicate consumed NO id), and the cursor's next mint is exactly
    /// `survivors + skips` — the property-level composition the `dedup_tests` single-example units pin
    /// pointwise. [Build-Session-Entscheidung: P2.137]
    #[test]
    fn prop_dedup_and_skip_minting_compose_over_one_contiguous_id_space() {
        use std::collections::BTreeSet;
        // One §2.3.1 identity per CLASS index — the fold keys on (dev, inode), so equal class indexes model
        // a duplicate (hardlink / re-reached file) and distinct indexes distinct resolved files.
        fn class_fid(class: u8) -> FileIdentity {
            FileIdentity {
                canonical_path: PathBuf::from(format!("/gen/class-{class}.csv")),
                dev_or_volserial: 77,
                inode_or_fileindex: u64::from(class),
            }
        }
        pinned_runner()
            .run(
                &prop::collection::vec((0u8..8, any::<bool>()), 0..64usize),
                |entries| {
                    let mut ids = ItemIdSpace::new();
                    let eligible: Vec<(FileIdentity, usize)> = entries
                        .iter()
                        .enumerate()
                        .filter(|(_, (_, eligible))| *eligible)
                        .map(|(pos, (class, _))| (class_fid(*class), pos))
                        .collect();
                    let survivors = dedup_by_identity(eligible, &mut ids)
                        .expect("at most 64 candidates never exhaust the u32 id space");
                    // One id per generated INELIGIBLE item, from the SAME shared cursor (assembly order).
                    let skip_ids: BTreeSet<ItemId> = entries
                        .iter()
                        .filter(|(_, eligible)| !eligible)
                        .map(|_| {
                            ids.mint()
                                .expect("at most 64 skip mints never exhaust the u32 id space")
                        })
                        .collect();
                    let survivor_ids: BTreeSet<ItemId> = survivors.iter().map(|m| m.id).collect();
                    prop_assert!(
                        survivor_ids.is_disjoint(&skip_ids),
                        "§0.6 inv-6: survivor and skip ids never collide (one shared space)"
                    );
                    let minted = survivor_ids.len() + skip_ids.len();
                    let covered: BTreeSet<ItemId> =
                        survivor_ids.union(&skip_ids).copied().collect();
                    let expected: BTreeSet<ItemId> = (0..minted)
                        .map(|i| item_id(u32::try_from(i).expect("minted < 64 fits u32")))
                        .collect();
                    prop_assert_eq!(
                        covered,
                        expected,
                        "§0.6 inv-6: the two views together cover exactly the contiguous 0..(survivors+skips)"
                    );
                    let distinct_eligible_classes = entries
                        .iter()
                        .filter(|(_, eligible)| *eligible)
                        .map(|(class, _)| *class)
                        .collect::<BTreeSet<u8>>()
                        .len();
                    prop_assert_eq!(
                        survivors.len(),
                        distinct_eligible_classes,
                        "§2.3.2: one survivor per distinct identity class — each duplicate consumed NO id"
                    );
                    let next = ids
                        .mint()
                        .expect("the space is nowhere near the u32 ceiling here");
                    prop_assert_eq!(
                        next,
                        item_id(u32::try_from(minted).expect("minted < 64 fits u32")),
                        "§0.6 inv-6: the shared cursor advanced once per survivor + skip, never for a duplicate"
                    );
                    Ok(())
                },
            )
            .expect("the pinned 512-case exploration holds §0.6 invariant 6 across the composition");
    }

    // §6.4.1 unit (G15): the §0.4.4 run-registry lifecycle (P2.42). `register` mints a LIVE token, `cancel`
    // trips it + reports found, an unknown/finished `cancel` is the §0.4.1 C7 idempotent no-op, `finish`
    // drops WITHOUT cancelling, and distinct runs hold independent tokens. No tokio runtime needed —
    // `CancellationToken::new`/`cancel`/`is_cancelled` are synchronous atomic ops (only `.cancelled().await`
    // would need a runtime, and nothing here awaits).
    #[test]
    fn run_registry_register_yields_a_live_token() {
        let reg = RunRegistry::default();
        let token = reg.register(run_id());
        assert!(
            !token.is_cancelled(),
            "§0.4.4: a freshly registered run's token is live (not cancelled)"
        );
    }

    #[test]
    fn run_registry_cancel_trips_the_registered_token_and_reports_found() {
        let reg = RunRegistry::default();
        let token = reg.register(run_id());
        assert!(
            reg.cancel(run_id()),
            "§0.4.4: cancelling a registered run reports the token was found"
        );
        assert!(
            token.is_cancelled(),
            "§0.4.4: cancel trips the run's token — the stored copy and the handed-out clone share one state"
        );
    }

    #[test]
    fn run_registry_cancel_unknown_run_is_the_idempotent_no_op() {
        let reg = RunRegistry::default();
        assert!(
            !reg.cancel(run_id()),
            "§0.4.1/§0.4.4: cancelling an unknown / already-finished run is a clean no-op returning false (C7 idempotent)"
        );
    }

    #[test]
    fn run_registry_finish_drops_the_token_without_cancelling() {
        let reg = RunRegistry::default();
        let token = reg.register(run_id());
        reg.finish(run_id());
        assert!(
            !token.is_cancelled(),
            "§0.4.4: finish (RunFinished) drops the registry entry but never cancels — a normal finish leaves an outstanding worker clone live"
        );
        assert!(
            !reg.cancel(run_id()),
            "§0.4.4: after finish the run is no longer registered, so a later cancel is a no-op"
        );
    }

    // §6.4.1 unit (G15): the §7.1.1/§7.3.2 refuse-busy predicate (P2.55) — `has_active_run` is the §1.9
    // "Running" signal `converter_is_busy` reads: false when empty (idle / the pre-P3 default), true once a
    // run is registered (C6), and STILL true after a C7 cancel until `finish` (a cancelling run stays busy),
    // then false after `finish` (RunFinished).
    #[test]
    fn run_registry_has_active_run_tracks_the_running_window() {
        let reg = RunRegistry::default();
        assert!(
            !reg.has_active_run(),
            "§7.1.1: an empty registry is not busy (idle — the pre-P3 default → idle-flow open)"
        );
        let _token = reg.register(run_id());
        assert!(
            reg.has_active_run(),
            "§7.1.1: a registered run (C6) makes the converter busy → the funnel refuses mid-run intake"
        );
        reg.cancel(run_id());
        assert!(
            reg.has_active_run(),
            "§7.1.1: a cancelling-but-not-finished run is STILL busy (the token lingers until finish)"
        );
        reg.finish(run_id());
        assert!(
            !reg.has_active_run(),
            "§7.1.1: after finish (RunFinished) the run is terminal → not busy again"
        );
    }

    #[test]
    fn run_registry_distinct_runs_have_independent_tokens() {
        let reg = RunRegistry::default();
        let a = reg.register(run_id());
        let b = reg.register(run_id_other());
        assert!(
            reg.cancel(run_id()),
            "run A is registered, so its cancel is found"
        );
        assert!(a.is_cancelled(), "§0.4.4: cancelling run A trips A's token");
        assert!(
            !b.is_cancelled(),
            "§0.4.4: run A's cancel does not touch run B's independent token"
        );
    }

    /// A minimal §1.12 `RunResult` for the §0.4.4 retention tests — one succeeded item, no residue. Its path
    /// fields are display strings (2026-07-06 ruling); the real paths are the sibling `sample_run_paths`.
    fn sample_run_result(rid: RunId) -> RunResult {
        RunResult {
            collected_set_id: collected_set_id(),
            run_id: rid,
            items: vec![],
            totals: Totals {
                succeeded: 1,
                failed: 0,
                cancelled: 0,
                skipped: 0,
            },
            cleanup_incomplete: vec![],
            // Irrelevant to the retention tests (they exercise store lifetime, not the projection).
            // [Build-Session-Entscheidung: P3.59]
            summary_line_display: String::new(),
            common_root_display: "/out".to_string(),
            divert_root_display: None,
        }
    }

    /// The off-wire `RunResultPaths` sibling of `sample_run_result` — the REAL common root the wire result's
    /// `common_root_display` shed (2026-07-06 ruling, §2.10.1); no per-item outputs/residues (the retention
    /// tests exercise the wire re-serve + the `paths()` re-serve, not C9 membership).
    /// [Build-Session-Entscheidung: P3.76]
    fn sample_run_paths() -> RunResultPaths {
        RunResultPaths {
            common_root: PathBuf::from("/out"),
            divert_root: None,
            item_outputs: BTreeMap::new(),
            item_residues: BTreeMap::new(),
        }
    }

    // §6.4.1 unit (G15): the §0.4.4 RunResult-retention lifecycle (P2.43). `retain` stores the terminal
    // summary; `get` re-serves it for the matching `RunId` (the C8 idempotent re-fetch); an empty / mismatched
    // `get` is `None`; `retain` supersedes the prior result (only the latest is kept — §0.4.4 "until a new run
    // starts"); and `evict` clears it (the new-run-start eviction). `RunResult` is owned data, no runtime.
    #[test]
    fn run_result_store_retain_then_get_matching_id_returns_the_result() {
        let store = RunResultStore::default();
        let result = sample_run_result(run_id());
        store.retain(result.clone(), sample_run_paths());
        assert_eq!(
            store.get(run_id()),
            Some(result),
            "§0.4.4: a retained terminal RunResult is re-served to C8 for its own RunId"
        );
    }

    // §6.4.1 unit (G15): the §0.4.4 OFF-WIRE `RunResultPaths` re-serve (P3.51) — `retain` stores the real paths
    // alongside the wire result; `current_paths` re-serves the CURRENT run's paths (the C9 `OpenTarget`
    // resolution source, §7.7.2), and on an empty store or after `evict` it is `None` (the C9 §7.7.3 refusal).
    // This is the off-wire half of the retention contract the display-only wire `RunResult` depends on (§2.10.1).
    // [Test-Change: P3.51 — old-obsolete+new-correct, §7.7.2] the P3.76 run-id-keyed `paths(run_id)` accessor is
    // REMOVED with P3.51 (the C9 wire carries no `run_id`, §7.7.2/§0.4.1 — a run-id form is unreachable), so this
    // test re-cuts onto the live `current_paths`; its RunId-MISMATCH assertion is obsolete (the run-id-match
    // guard stays covered by the sibling `get(run_id)` test above), while the retain-then-serve and
    // evict-clears-paths legs are RETAINED over the live accessor (read-back proof, test-strategy §0.2).
    #[test]
    fn run_result_store_current_paths_re_serves_the_off_wire_paths() {
        let store = RunResultStore::default();
        assert_eq!(
            store.current_paths(),
            None,
            "§0.4.4: an empty store has no off-wire paths to serve (the C9 §7.7.3 refusal)"
        );
        store.retain(sample_run_result(run_id()), sample_run_paths());
        assert_eq!(
            store.current_paths(),
            Some(sample_run_paths()),
            "§0.4.4/§7.7.3: the current run's off-wire RunResultPaths is re-served to C9"
        );
        store.evict();
        assert_eq!(
            store.current_paths(),
            None,
            "§0.4.4: evict clears the off-wire paths too (no stale real path survives a new run start)"
        );
    }

    #[test]
    fn run_result_store_get_on_empty_is_none() {
        let store = RunResultStore::default();
        assert_eq!(
            store.get(run_id()),
            None,
            "§0.4.4: an empty store re-serves nothing (the C8 caller maps None to its §0.4.3 not-available error)"
        );
    }

    #[test]
    fn run_result_store_get_mismatched_id_is_none() {
        let store = RunResultStore::default();
        store.retain(sample_run_result(run_id()), sample_run_paths());
        assert_eq!(
            store.get(run_id_other()),
            None,
            "§0.4.4: a retained result is NEVER served for a different run's id (the RunId match guards it)"
        );
    }

    #[test]
    fn run_result_store_retain_supersedes_the_prior_result() {
        let store = RunResultStore::default();
        store.retain(sample_run_result(run_id()), sample_run_paths());
        store.retain(sample_run_result(run_id_other()), sample_run_paths());
        assert_eq!(
            store.get(run_id()),
            None,
            "§0.4.4: only the latest run's result is retained — the superseded prior id no longer resolves"
        );
        assert_eq!(
            store.get(run_id_other()),
            Some(sample_run_result(run_id_other())),
            "§0.4.4: the latest retained result is the one re-served"
        );
    }

    #[test]
    fn run_result_store_evict_clears_the_retained_result() {
        let store = RunResultStore::default();
        store.retain(sample_run_result(run_id()), sample_run_paths());
        store.evict();
        assert_eq!(
            store.get(run_id()),
            None,
            "§0.4.4: evict (a new run starting) clears the retained result so a stale summary is not re-served"
        );
    }

    // §6.4.1 unit (G15): the §0.4.4 picked-destination registry lifecycle (P3.76) — `register` mints a
    // `DestinationId` + stores the picked root (the PATH never crosses the wire, §2.10.1); `resolve` re-serves
    // it (C4/C5/C6 `ChosenRoot(id)`), an unknown id is `None` (the WebView cannot name a path it never picked);
    // and — unlike the SUPERSEDING `CollectedSetRegistry` — it ACCUMULATES: a second pick does not evict the
    // first (§0.4.4 "entries survive across collected sets, so switching batches never forces a re-pick"). The
    // C2b register + C4/C5/C6 resolve wiring is P3.80; this pins the store contract now (the sibling of the
    // RunRegistry / RunResultStore / CollectedSetRegistry lifecycle pins).
    #[test]
    fn destination_registry_register_resolve_and_accumulate() {
        let reg = DestinationRegistry::default();
        let a = reg.register(PathBuf::from("/home/me/Documents"));
        let b = reg.register(PathBuf::from("/home/me/Downloads"));
        assert_ne!(
            a, b,
            "§0.4.4: each pick mints a fresh, distinct DestinationId"
        );
        assert_eq!(
            reg.resolve(a),
            Some(PathBuf::from("/home/me/Documents")),
            "§0.4.4: a picked root resolves back to its real PathBuf (C4/C5/C6 ChosenRoot(id) → the real path)"
        );
        assert_eq!(
            reg.resolve(b),
            Some(PathBuf::from("/home/me/Downloads")),
            "§0.4.4: the second pick ACCUMULATES — registering b did not evict a (switching batches never re-picks)"
        );
        assert_eq!(
            reg.resolve(DestinationId::mint()),
            None,
            "§0.4.4/§0.4.3: an unknown id resolves to None — the WebView cannot name a path the user never picked"
        );
    }

    // §6.4.1 unit (G15): the §0.4.4 wire→core `resolve_choice` (P3.80) — the single fallible C4/C6 boundary step
    // that turns a wire `DestinationChoice` into a core `ResolvedDestination`. `BesideSource` maps through (never
    // fails, never a registry lookup); a `ChosenRoot(registered id)` resolves to its real `PathBuf`; a
    // `ChosenRoot(unknown id)` is the §0.4.3 refusal (`None`) the C4/C6 handler maps to its not-available
    // `IpcError`. Read back over a real registry (test-strategy §0.2), never mocked — the C9 `resolve_open_target`
    // id-resolution mirror.
    #[test]
    fn resolve_choice_maps_beside_source_and_resolves_or_refuses_a_chosen_id() {
        let reg = DestinationRegistry::default();
        assert_eq!(
            reg.resolve_choice(&DestinationChoice::BesideSource),
            Some(ResolvedDestination::BesideSource),
            "§0.4.4: BesideSource maps through to the resolved beside-source (never a registry lookup, never fails)"
        );
        let id = reg.register(PathBuf::from("/home/me/Exports"));
        assert_eq!(
            reg.resolve_choice(&DestinationChoice::ChosenRoot(id)),
            Some(ResolvedDestination::ChosenRoot(PathBuf::from(
                "/home/me/Exports"
            ))),
            "§0.4.4: a ChosenRoot(registered id) resolves to its real picked-root PathBuf"
        );
        assert_eq!(
            reg.resolve_choice(&DestinationChoice::ChosenRoot(DestinationId::mint())),
            None,
            "§0.4.4/§0.4.3: a ChosenRoot(unknown id) is the refusal (None) → the C4/C6 not-available IpcError"
        );
    }

    // §6.4.1 unit (G15): the §7.4 persisted-last resolver (leg 3, P3.80) — read the §7.4.1 `lastDestinationMode`
    // pref VALUE, re-validate a stored path as writable (§2.7.2 location_status), and load it into the registry.
    // `BesideSource` → None (the §2.7.1 default, nothing registered); a WRITABLE `ChosenPath` mints + registers a
    // DestinationId and yields the DestinationPicked; a GONE/read-only `ChosenPath` falls back to beside-source
    // (None), nothing registered (§7.4.1 "re-validated at use time" / §5.8 fallback). Real FS + a real registry,
    // never mocked (test-strategy §0.1/§0.2).
    // [Test-Change: P3.56 — old-obsolete+new-correct, §5.8] the resolver's return type is re-cut from
    // `Option<DestinationPicked>` to the 3-way `InitialDestination` (Co-Pilot ruling item 2, 7f73553): the two
    // `None` cases (a plain beside-source pref vs a re-validation FALLBACK) MUST be STRUCTURALLY distinct so the
    // §5.8:926 passive fallback note surfaces even when beside-source is writable (the G1 Opus-P2 adoption). The
    // old `None`/`Some(picked)` assertions are obsolete against the new type; the new `BesideSource`/`ChosenRoot`/
    // `Fallback` assertions verify the SAME behaviours (sentinel→default, writable→registered+read-back, gone→fall-back).
    #[test]
    fn resolve_persisted_destination_registers_a_writable_path_else_falls_back_beside_source() {
        let probe = PublishTemp::probe_name(instance_id());

        // §7.4.1: a beside-source pref → the plain §2.7.1 default (NOT a fallback), nothing registered (no probe).
        let reg = DestinationRegistry::default();
        assert_eq!(
            resolve_persisted_destination(&LastDestinationMode::BesideSource, &reg, &probe),
            InitialDestination::BesideSource,
            "§7.4.1: a beside-source pref → InitialDestination::BesideSource (the plain default, nothing registered)"
        );

        // A real, non-ephemeral WRITABLE dir → ChosenRoot(DestinationPicked) whose id resolves back to it.
        let base = tempfile::Builder::new()
            .prefix("convertia-persisted-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .expect("create a temp dir in the crate source root");
        if crate::platform::is_ephemeral_output_dir(base.path()) {
            return; // pathological: the crate root sits under an OS temp root → location_status would divert; a clean skip.
        }
        let last = LastDestinationMode::ChosenPath(base.path().to_path_buf());
        // [Test-Change: P3.56 — old-obsolete+new-correct, §5.8] the writable-arm assertion is re-cut from the old
        // `Option::expect` to a match→`expect` over the 3-way return (`ChosenRoot(picked)` replaces `Some(picked)`).
        // Extract via match → Option → `expect` (no `panic!` on the in-core path; the §1.1 no-panic discipline).
        // Arms enumerated (no `_`) — `#![deny(clippy::wildcard_enum_match_arm)]` on the dispatch enums.
        let picked = match resolve_persisted_destination(&last, &reg, &probe) {
            InitialDestination::ChosenRoot(picked) => Some(picked),
            InitialDestination::BesideSource | InitialDestination::Fallback => None,
        }
        .expect(
            "§7.4.1: a writable persisted path → InitialDestination::ChosenRoot(DestinationPicked)",
        );
        assert_eq!(
            picked.display,
            base.path().to_string_lossy().into_owned(),
            "§2.10.1: the display is the lossy form of the picked folder"
        );
        assert_eq!(
            reg.resolve(picked.destination),
            Some(base.path().to_path_buf()),
            "§7.4.1: the yielded id resolves to the registered real path (loaded core-side into the registry)"
        );

        // A GONE path (a non-existent subdir) → location_status Divert(Unwritable) → InitialDestination::Fallback,
        // nothing registered (a stale pref never reaches the no-harm machinery unchecked, §7.4.1/§5.8).
        let reg2 = DestinationRegistry::default();
        let gone = LastDestinationMode::ChosenPath(base.path().join("does-not-exist"));
        assert_eq!(
            resolve_persisted_destination(&gone, &reg2, &probe),
            InitialDestination::Fallback,
            "§7.4.1/§5.8: a gone/read-only persisted path → InitialDestination::Fallback (the structural fallback fact, nothing registered)"
        );
    }

    /// A second, distinct `CollectedSetId` — for the §0.4.4 collected-set-registry stale/supersede tests.
    fn collected_set_id_other() -> CollectedSetId {
        serde_json::from_str(r#""33333333-3333-4333-8333-333333333333""#)
            .expect("CollectedSetId deserializes from a uuid string")
    }
    /// A test `InstanceId` — the frozen set's owning instance (§7.1.2). Built via its public wire form, like
    /// the id helpers above (the inner `Uuid` is private to `crate::domain`).
    fn instance_id() -> InstanceId {
        serde_json::from_str(r#""44444444-4444-4444-8444-444444444444""#)
            .expect("InstanceId deserializes from a uuid string")
    }
    /// A minimal `RegisteredSet` carrying `id` — empty frozen payload + empty identities, since the §0.4.4
    /// registry's register/resolve/take/supersede lifecycle is content-agnostic (the full-payload projection
    /// is tested in `crate::domain::tests::frozen_collected_set_projects_only_single_with_full_payload`; the
    /// P3.40 identity evidence in `rerun_verdict_tests`).
    fn frozen_set(id: CollectedSetId) -> RegisteredSet {
        RegisteredSet {
            frozen: FrozenCollectedSet {
                id,
                instance: instance_id(),
                format: UserFacingFormat::Csv,
                items: vec![],
                count: 0,
                skipped: vec![],
                total_bytes: 0,
                roots: vec![],
                encoding_hint: None,
                delimiter_hint: None,
                notes: vec![],
                item_paths: BTreeMap::new(),
            },
            identities: BTreeMap::new(),
        }
    }

    // §6.4.1 unit (G15): the §0.4.4 collected-set-registry lifecycle (P2.44). `register` stores the frozen
    // set (keyed by its own id); `resolve` re-serves it for the matching `collectedSetId` WITHOUT evicting
    // (C3/C4/C5 may each fire repeatedly); `take` (C6) resolves AND evicts in one op; `register` supersedes
    // any prior un-run set (at most one live entry, §2.4.3); and a stale/mismatched id never resolves nor
    // evicts the live set (the §0.4.3 not-available guard). `FrozenCollectedSet` is owned data, no runtime.
    #[test]
    fn collected_set_registry_register_then_resolve_returns_the_frozen_set() {
        let reg = CollectedSetRegistry::default();
        let id = collected_set_id();
        reg.register(frozen_set(id));
        assert_eq!(
            reg.resolve(id).as_deref(),
            Some(&frozen_set(id)),
            "§0.4.4: a registered frozen set resolves for its own collectedSetId (C3/C4/C5/C6 read it)"
        );
    }

    #[test]
    fn collected_set_registry_resolve_unknown_id_is_none() {
        let reg = CollectedSetRegistry::default();
        assert!(
            reg.resolve(collected_set_id()).is_none(),
            "§0.4.4: an unknown collectedSetId resolves to None (the C-command maps it to its §0.4.3 not-available error)"
        );
    }

    #[test]
    fn collected_set_registry_resolve_does_not_evict() {
        let reg = CollectedSetRegistry::default();
        let id = collected_set_id();
        reg.register(frozen_set(id));
        assert!(reg.resolve(id).is_some(), "first resolve sees the set");
        assert!(
            reg.resolve(id).is_some(),
            "§0.4.4: resolve is a NON-evicting read — C3/C4/C5 may each fire repeatedly (C4 is debounced-re-callable, §5.8)"
        );
    }

    #[test]
    fn collected_set_registry_take_resolves_and_evicts() {
        let reg = CollectedSetRegistry::default();
        let id = collected_set_id();
        reg.register(frozen_set(id));
        assert_eq!(
            reg.take(id).as_deref(),
            Some(&frozen_set(id)),
            "§0.4.4: take (C6 start_conversion) resolves the frozen set the Batch is built from"
        );
        assert!(
            reg.resolve(id).is_none(),
            "§0.4.4: take EVICTS — once the run starts the set leaves the registry, never lingering to be re-run"
        );
    }

    #[test]
    fn collected_set_registry_register_supersedes_prior() {
        let reg = CollectedSetRegistry::default();
        reg.register(frozen_set(collected_set_id()));
        reg.register(frozen_set(collected_set_id_other()));
        assert!(
            reg.resolve(collected_set_id()).is_none(),
            "§0.4.4/§2.4.3: a new C1/C2a freeze supersedes the prior un-run set — the superseded id no longer resolves"
        );
        assert!(
            reg.resolve(collected_set_id_other()).is_some(),
            "§0.4.4: the latest frozen set is the one resolved (at most one live entry)"
        );
    }

    #[test]
    fn collected_set_registry_take_mismatched_id_does_not_evict_the_live_set() {
        let reg = CollectedSetRegistry::default();
        let live = collected_set_id();
        reg.register(frozen_set(live));
        assert!(
            reg.take(collected_set_id_other()).is_none(),
            "§0.4.4: taking an unknown / superseded id is a clean None, never the wrong set"
        );
        assert!(
            reg.resolve(live).is_some(),
            "§0.4.4: a mismatched take leaves the live set untouched (the id-keyed map guards it)"
        );
    }

    /// A `CollectingId` for the §0.4.4 ingest-registry tests — built via its public bare-uuid wire form
    /// (the inner `Uuid` is private to `crate::domain`), like the sibling id helpers.
    fn collecting_id() -> CollectingId {
        serde_json::from_str(r#""55555555-5555-4555-8555-555555555555""#)
            .expect("CollectingId deserializes from a uuid string")
    }
    /// A second, distinct `CollectingId` — for the independent-token test (two in-flight ingests).
    fn collecting_id_other() -> CollectingId {
        serde_json::from_str(r#""66666666-6666-4666-8666-666666666666""#)
            .expect("CollectingId deserializes from a uuid string")
    }

    // §6.4.1 unit (G15): the §0.4.4 ingest-registry lifecycle (P2.45) — the one-phase-earlier sibling of the
    // run registry. `register` mints a LIVE token, `cancel` (C13) trips it + reports found, an unknown/released
    // `cancel` is the §0.4.1 C13 idempotent no-op (→ Ok(()) at the handler), `release` drops WITHOUT cancelling
    // and is idempotent on EVERY exit branch (incl. the C2a cancelled-dialog branch where the walk never ran),
    // and distinct ingests hold independent tokens. No tokio runtime needed — `CancellationToken::new`/`cancel`/
    // `is_cancelled` are synchronous atomic ops (only `.cancelled().await` would need a runtime).
    #[test]
    fn ingest_registry_register_yields_a_live_token() {
        let reg = IngestRegistry::default();
        let token = reg.register(collecting_id());
        assert!(
            !token.is_cancelled(),
            "§0.4.4: a freshly registered ingest's token is live (not cancelled)"
        );
    }

    #[test]
    fn ingest_registry_cancel_trips_the_registered_token_and_reports_found() {
        let reg = IngestRegistry::default();
        let token = reg.register(collecting_id());
        assert!(
            reg.cancel(collecting_id()),
            "§0.4.4: C13 cancelling a registered ingest reports the token was found"
        );
        assert!(
            token.is_cancelled(),
            "§0.4.4: cancel trips the ingest's token — the stored copy and the handed-out poll clone share one state"
        );
    }

    #[test]
    fn ingest_registry_cancel_unknown_ingest_is_the_idempotent_no_op() {
        let reg = IngestRegistry::default();
        assert!(
            !reg.cancel(collecting_id()),
            "§0.4.1/§0.4.4: C13 cancelling an unknown / already-released ingest is a clean no-op returning false (the handler maps it to Ok(()))"
        );
    }

    #[test]
    fn ingest_registry_release_drops_the_token_without_cancelling() {
        let reg = IngestRegistry::default();
        let token = reg.register(collecting_id());
        reg.release(collecting_id());
        assert!(
            !token.is_cancelled(),
            "§0.4.4: release (a handler exit branch) drops the registry entry but never cancels — the normal walk-completes branch leaves the poll clone live"
        );
        assert!(
            !reg.cancel(collecting_id()),
            "§0.4.4: after release the ingest is no longer registered, so a later C13 cancel is a no-op"
        );
    }

    #[test]
    fn ingest_registry_release_on_every_exit_branch_is_idempotent() {
        let reg = IngestRegistry::default();
        // The C2a cancelled-dialog → Empty branch: the handler explicitly releases even though the walk loop
        // never ran. Releasing an id that was registered-then-not-walked, and double-releasing, are no-ops.
        reg.register(collecting_id());
        reg.release(collecting_id());
        reg.release(collecting_id()); // double release (e.g. cancel-then-exit) — idempotent
        reg.release(collecting_id_other()); // releasing a never-registered id — idempotent
        assert!(
            !reg.cancel(collecting_id()),
            "§0.4.4: release on every exit branch is idempotent — no token leak, no panic, the entry is gone"
        );
    }

    #[test]
    fn ingest_registry_distinct_ingests_have_independent_tokens() {
        let reg = IngestRegistry::default();
        let a = reg.register(collecting_id());
        let b = reg.register(collecting_id_other());
        assert!(
            reg.cancel(collecting_id()),
            "ingest A is registered, so its C13 cancel is found"
        );
        assert!(
            a.is_cancelled(),
            "§0.4.4: cancelling ingest A trips A's token"
        );
        assert!(
            !b.is_cancelled(),
            "§0.4.4: ingest A's cancel does not touch ingest B's independent token"
        );
    }

    // §6.4.1 unit (G15): the §1.1 C2a RAII guard (P2.70) — `register_guard` registers a LIVE token (a C13
    // cancel finds + trips it) AND the guard exposes the trip for the §1.1 post-dialog check
    // (`is_cancelled()` becomes true once C13 trips the token while the dialog is up). No tokio runtime
    // (synchronous atomic ops). [Build-Session-Entscheidung: P2.70]
    #[test]
    fn ingest_guard_registers_a_live_token_and_exposes_cancellation() {
        let reg = IngestRegistry::default();
        let guard = reg.register_guard(collecting_id());
        assert!(
            !guard.is_cancelled(),
            "§1.1/P2.70: a freshly registered guard's token is live — the post-dialog check sees no C13 yet"
        );
        assert!(
            reg.cancel(collecting_id()),
            "§1.1/P2.70: register_guard truly registered the token, so a C13 cancel_ingest finds + trips it"
        );
        assert!(
            guard.is_cancelled(),
            "§1.1/P2.70: the guard observes the C13 trip — the §1.1 post-dialog check then abandons the pick (Empty)"
        );
    }

    // §6.4.1 unit (G15): the §1.1 "drop on EVERY C2a exit branch" rule realized BY RAII (P2.70) — dropping the
    // guard (any return path) de-registers the token, so a later C13 cancel finds nothing (released, NOT
    // leaked). Borrowing the registry (not an AppHandle) is what makes this drop behaviour testable with no
    // Tauri runtime. [Build-Session-Entscheidung: P2.70]
    #[test]
    fn ingest_guard_releases_the_token_on_drop() {
        let reg = IngestRegistry::default();
        {
            let _guard = reg.register_guard(collecting_id());
            // guard alive here; it de-registers when the block ends (mirroring a C2a handler return).
        }
        assert!(
            !reg.cancel(collecting_id()),
            "§1.1/P2.70: the guard's Drop released the token on exit — a later C13 cancel finds nothing (no leak)"
        );
    }

    // ── §7.8.1 PendingIntake first-launch buffer (P2.58) ──────────────────────────────────────────────
    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    // [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1] The P2.137 `stash_or_route`/`RouteToEmit` fused
    // no-loss closure collapsed into a plain [`PendingIntake::stash`] + the two-rule no-loss ordering (stash
    // BEFORE the funnel's ready-read; mark-ready BEFORE the drain's take) when the 2026-07-06 core-owned-path
    // ruling retired the payload-carrying `Emit` arm (there is no live emit to re-route to). The assertions
    // below are the SAME §7.8.1 contracts (consume-once, stored-origin, no-loss accumulation) driven through
    // the simplified API (`stash` unconditionally buffers; the drain still fuses mark-ready + take).

    // §6.4.1 unit (G15): the §7.8.1 stash→drain round-trip — a buffered intake set is taken back with its
    // paths + the stored origin, and the slot is cleared (the C1 drain consume-once, §7.8.1); the drain marks
    // the frontend ready in the same fused step (P2.137).
    #[test]
    fn pending_intake_stash_then_take_returns_the_set_and_clears() {
        let buf = PendingIntake::default();
        let ready = FrontendReady::default();
        buf.stash(paths(&["a.png", "b.jpg"]), IntakeOrigin::LaunchArg);
        let drained = buf
            .take_marking_ready(&ready)
            .expect("§7.8.1: a stashed set is drained back");
        assert_eq!(
            drained.paths,
            paths(&["a.png", "b.jpg"]),
            "§7.8.1: the drained paths are exactly the stashed set"
        );
        assert_eq!(
            drained.origin,
            IntakeOrigin::LaunchArg,
            "§7.8.1: the drain carries the stored origin (typically LaunchArg)"
        );
        assert!(
            ready.is_ready(),
            "§7.8.1/P2.137: the fused drain marked the frontend ready in the same critical section"
        );
        assert!(
            buf.take_marking_ready(&ready).is_none(),
            "§7.8.1: the drain consumes exactly once — a second take is None (idempotent)"
        );
    }

    // §6.4.1 unit (G15): an un-stashed buffer drains to None — the ordinary first launch with no files,
    // which C1 maps to CollectedSet::Empty (§0.4.1 / §7.8.1) — and still marks ready (the drain call IS the
    // §7.8.1 readiness signal, files or not).
    #[test]
    fn pending_intake_empty_take_is_none() {
        let buf = PendingIntake::default();
        let ready = FrontendReady::default();
        assert!(
            buf.take_marking_ready(&ready).is_none(),
            "§7.8.1: a never-stashed buffer drains to None (first launch, no files)"
        );
        assert!(
            ready.is_ready(),
            "§7.8.1: the empty drain still marks ready — readiness is the drain's signal, not the files'"
        );
    }

    // §6.4.1 unit (G15): NO-LOSS on a repeat stash before the drain — a second intake before the drain
    // APPENDS its paths (never supersedes, which would drop the earlier set's paths) and keeps the FIRST
    // origin (§7.8.1 "its stored origin"). This is the path-loss-avoidance property the single hand-off buffer
    // rests on (every buffered intake set is preserved until the drain). [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1]
    #[test]
    fn pending_intake_repeat_stash_accumulates_paths_keeps_first_origin() {
        let buf = PendingIntake::default();
        let ready = FrontendReady::default();
        buf.stash(paths(&["first.png"]), IntakeOrigin::LaunchArg);
        buf.stash(
            paths(&["second.jpg", "third.gif"]),
            IntakeOrigin::SecondInstance,
        );
        let drained = buf
            .take_marking_ready(&ready)
            .expect("§7.8.1: the accumulated set is drained back");
        assert_eq!(
            drained.paths,
            paths(&["first.png", "second.jpg", "third.gif"]),
            "§7.8.1: a repeat stash APPENDS its paths (no-loss), never supersedes the earlier launch's set"
        );
        assert_eq!(
            drained.origin,
            IntakeOrigin::LaunchArg,
            "§7.8.1: the FIRST stash's origin is kept across an accumulating second stash"
        );
    }

    // §6.4.1 unit (G15): the §7.8.1 no-loss ordering (P3.77) — the exact interleaving the two-rule ordering
    // closes: the C1 drain runs its fused mark-ready + take (finding nothing), and only THEN does the funnel
    // reach the stash. Under the retired `Emit`-arm model a plain stash here would strand the set (ready is
    // monotonic, the drain fires once per mount). Now the funnel ALWAYS stashes (the set is buffered, never
    // dropped) and — because the drain already marked ready — its post-stash `is_ready()` read sees `true`, so
    // it NUDGES, and the nudge-triggered drain retrieves the buffered set. [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1]
    #[test]
    fn stash_after_drain_stays_buffered_for_the_nudge_drain() {
        let buf = PendingIntake::default();
        let ready = FrontendReady::default();
        // [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1] the old not-ready `precondition` assert is
        // obsolete — the funnel no longer snapshots readiness up front (the disposition is `busy`-only now).
        // The C1 drain interleaves before the stash lands: fused mark-ready + take (nothing pending yet).
        assert!(buf.take_marking_ready(&ready).is_none());
        assert!(
            ready.is_ready(),
            "§7.8.1: the drain marked the frontend ready in the same fused step"
        );
        // The funnel now stashes the late set — unconditionally buffered (never dropped, the P3.77 no-loss rule).
        // [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1] the old drain-check-after-stash order + the
        // retired `stash_or_route`→`RouteToEmit` re-route collapse into this plain `stash`; same no-loss meaning.
        buf.stash(paths(&["late.png"]), IntakeOrigin::LaunchArg);
        // Because ready is already `true`, the funnel's post-stash `is_ready()` read nudges → a fresh drain,
        // which retrieves the buffered set. Nothing is stranded.
        assert!(
            ready.is_ready(),
            "§7.8.1: ready stays true after the stash, so the funnel emits the app://intake nudge (P3.77)"
        );
        let drained = buf.take_marking_ready(&ready).expect(
            "§7.8.1: the late stash is retrieved by the nudge-triggered drain (never stranded)",
        );
        assert_eq!(
            drained.paths,
            paths(&["late.png"]),
            "§7.8.1: the buffered set carries the full stashed payload"
        );
        // [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1] the old `set.origin` (off the retired
        // `RouteToEmit(set)`) is now `drained.origin` off the buffered take — the same origin-preservation check.
        assert_eq!(drained.origin, IntakeOrigin::LaunchArg);
        assert!(
            buf.take_marking_ready(&ready).is_none(),
            "§7.8.1: the drain consumed the buffer exactly once (nothing remains)"
        );
    }

    // §6.4.2 stress leg (G15; bounded, deterministic INVARIANT — not timing-dependent asserts): under a real
    // two-thread race between the funnel `stash` and the C1 drain, the §7.8.1 no-loss invariant always holds —
    // the raced set is EITHER taken by the concurrent drain OR still buffered for a late one; it is never
    // stranded. [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1] the `stash_or_route`/`RouteToEmit`
    // re-route collapsed into a plain `stash`, so there is no second outcome branch — `stash` always buffers,
    // and the two-rule ordering (funnel nudges after the stash iff ready) closes the race at the funnel, not
    // here; this leg proves the buffer level never loses the set.
    #[test]
    fn stash_vs_drain_race_never_strands_a_set() {
        for _ in 0..100 {
            let buf = std::sync::Arc::new(PendingIntake::default());
            let ready = std::sync::Arc::new(FrontendReady::default());
            let b1 = std::sync::Arc::clone(&buf);
            let stasher = std::thread::spawn(move || {
                b1.stash(vec![PathBuf::from("race.png")], IntakeOrigin::LaunchArg);
            });
            let (b2, r2) = (std::sync::Arc::clone(&buf), std::sync::Arc::clone(&ready));
            let drainer = std::thread::spawn(move || b2.take_marking_ready(&r2));
            stasher
                .join()
                .expect("§7.8.1: the stasher thread must not panic");
            let drained = drainer
                .join()
                .expect("§7.8.1: the drainer thread must not panic");
            // [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1] the join `.expect` messages shed their
            // P2.137 tag (the stash is a plain `stash` now, no `StashOutcome` bound); same thread-panic checks.
            // The residue a LATE drain (serialized after the stash) would still find:
            let residue = buf.take_marking_ready(&ready);
            // [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1] the old `match stash_outcome { Stashed =>
            // assert!(observed…), RouteToEmit(set) => assert_eq!(set.paths…) + assert!(…) }` collapses to two
            // no-loss invariants (no `StashOutcome` → one branch): the RouteToEmit paths-check + re-route assert
            // reduce to an observed assert + a no-duplicate assert.
            assert!(
                drained.is_some() || residue.is_some(),
                "§7.8.1 no-loss: the raced set is observed by the concurrent drain or remains buffered for \
                 a late one — never stranded"
            );
            assert!(
                !(drained.is_some() && residue.is_some()),
                "§7.8.1: the single-slot buffer hands the set to exactly one take, never both (no duplicate)"
            );
        }
    }

    // ── §7.8.1 FrontendReady WebView-ready flag (P2.59) ────────────────────────────────────────────────

    // §6.4.1 unit (G15): the §7.8.1 ready flag starts NOT-ready — the funnel's fail-safe default (a launch set
    // is buffered, never emitted, until the frontend proves its `app://intake` listener exists, §7.8.1).
    #[test]
    fn frontend_ready_defaults_to_not_ready() {
        let flag = FrontendReady::default();
        assert!(
            !flag.is_ready(),
            "§7.8.1: FrontendReady starts not-ready (the funnel buffers until the listener is proven)"
        );
    }

    // §6.4.1 unit (G15): mark_ready flips the flag false→true — once the WebView registers its listener (the C1
    // drain, P2.60) the funnel starts NUDGING `app://intake` after its stash instead of staying silent (§7.8.1;
    // the funnel always stashes, P3.77).
    #[test]
    fn frontend_ready_mark_ready_sets_ready() {
        let flag = FrontendReady::default();
        flag.mark_ready();
        assert!(
            flag.is_ready(),
            "§7.8.1: mark_ready makes the funnel nudge app://intake after the stash instead of staying silent"
        );
    }

    // §6.4.1 unit (G15): mark_ready is MONOTONIC + idempotent — the `main` window lives the whole session
    // (§7.3.1), so the listener never un-registers; a repeat mark stays ready (never resets to buffering).
    #[test]
    fn frontend_ready_mark_ready_is_idempotent() {
        let flag = FrontendReady::default();
        flag.mark_ready();
        flag.mark_ready();
        assert!(
            flag.is_ready(),
            "§7.8.1: the ready flag is monotonic — a repeat mark_ready keeps it ready (never resets)"
        );
    }
}

#[cfg(test)]
mod cleanup_honesty_tests {
    //! §6.4.1 unit (G15) for the P3.25 §2.6.4 cleanup-failure honesty leg — the `CleanupResidue` surfacing
    //! that guarantees a cleanup which could not complete is NEVER a silent clean success. This is pure
    //! projection logic (no FS, no engine); the real cleanup-on-fault fault-injection is the G31 hosted
    //! integration assertion (P3.71's temp-ownership box). Here we pin the §2.10.1 wire↔off-wire split, the
    //! three §2.6.4 dispositions, and the §2.8.2 "With residue" batch tail.
    use super::*;
    use std::path::PathBuf;

    // §6.4.1 (G15): `ResidueRecord::new` projects the display string (§2.10.1 last-step `to_string_lossy`) onto
    // the wire warning while RETAINING the real `PathBuf` byte-verbatim off-wire (the 2026-07-06 core-owned-
    // paths split) — the real path is never mutated into its lossy display.
    #[test]
    fn residue_record_projects_display_and_retains_real_path() {
        let item = ItemId::from_index(0);
        let residue = PathBuf::from("/src/.convertia-i-r-j.tsv.part");
        let record = ResidueRecord::new(item, residue.clone());

        assert_eq!(
            record.warning(),
            CleanupResidue {
                item,
                residue_display: residue.to_string_lossy().into_owned(),
            },
            "§2.6.4/§2.10.1: the DERIVED wire CleanupResidue carries the item + the last-step lossy display of the residue"
        );
        assert_eq!(
            record.real_path, residue,
            "§2.10.1: the real residue PathBuf is retained byte-verbatim off-wire (RunResultPaths.item_residues), never re-derived from the lossy display"
        );
    }

    // §6.4.1 (G15): the §2.10.1 lossy-vs-real distinction is REAL, not vacuous — a non-UTF-8 residue path keeps
    // its exact bytes in `real_path` while `residue_display` is the U+FFFD-replaced lossy form. Unix-only (only
    // there can a `PathBuf` hold non-UTF-8 bytes portably); the cross-platform contract (display =
    // `to_string_lossy`, real = verbatim) is already pinned by the test above.
    #[cfg(unix)]
    #[test]
    fn residue_record_display_is_lossy_while_real_path_is_byte_exact() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let raw_bytes: &[u8] = b"/src/.conv-\xFF\xFE.part";
        let residue = PathBuf::from(OsStr::from_bytes(raw_bytes));
        let record = ResidueRecord::new(ItemId::from_index(3), residue.clone());

        assert_eq!(
            record.real_path.as_os_str().as_bytes(),
            raw_bytes,
            "§2.10.1: the invalid bytes survive verbatim in the real residue path"
        );
        assert!(
            record.warning().residue_display.contains('\u{FFFD}'),
            "§2.10.1: the DERIVED display is the LOSSY projection (invalid bytes → U+FFFD), distinct from the byte-exact real path"
        );
    }

    // §6.4.1 (G15): the §2.6.4 THREE-CASE honesty — each disposition gets the reason §2.6.4 authors for it, and
    // NONE of them rewrites the item's terminal state: `Failed` (case 2) the combined §2.8.2 `CleanupResidue`
    // FAILURE message ("never a clean success"); `Succeeded` (case 1) the §2.8.2 NON-failure `Residue`
    // annotation (the success stands, with where residue remains — §5.7:830); `Cancelled` (case 3) NO reason
    // (§2.6.4 authors no per-item case-3 sentence; its surface is the structural `cleanup_incomplete` entry +
    // the BATCH-level tail). The machine-checkable "never a silent clean success" guard.
    //
    // [Test-Change: P3.59 — old-obsolete+new-correct, §2.6.4] The `Succeeded` arm's expectation flipped
    // `None` → `Some(Residue{..})`, per the 2026-07-16 P3.59 Co-Pilot ruling.
    // (1) OLD OBSOLETE: the P3.25 expectation encoded "case 1 imposes no reason override". Its rationale — that
    //     case 1 must not adopt the §2.8.2 CleanupResidue *failure* string — was CORRECT and still holds (the
    //     new arm is a NON-failure variant carrying no `ConversionErrorKind`). What it got wrong is that it
    //     left §2.6.4:944's OWN authored case-1 sentence — [Build-Session-Entscheidung: P3.59] quoted
    //     verbatim from the spec ("converted — a temporary file may remain at <path>"), so its
    //     "temporary" is §02 product copy about a leftover FILE, not a G8 deferral about this test —
    //     with no carrier at all, which is what forced the pre-ruling P3.59 fill to author a chrome paraphrase
    //     against §5.7:799 — the defect the G1 NOGO surfaced. The ruling promoted that sentence into §2.8.2 and
    //     gave it the `OutcomeMsg::Residue` slot; spec > code, so the expectation is obsolete.
    // (2) NEW CORRECT: verified against §2.6.4 case 1 (the success stands + the summary carries the annotation)
    //     and §5.7:830 ("not a clean success WITH WHERE residue remains"), and by READ-BACK below — the arm is
    //     compared to `crate::outcome::residue_annotation(display)` (the catalog's own output, never a
    //     re-hardcoded string), with the {path} substitution and the non-failure shape both asserted.
    // The `Cancelled` (case 3) and `Failed` (case 2) expectations are UNCHANGED — RATIFIED as built.
    #[test]
    fn residue_item_reason_carries_each_2_6_4_case_without_rewriting_the_state() {
        let display = "/src/.residue.tsv.part";

        let failed = residue_item_reason(ResidueDisposition::Failed, display);
        assert_eq!(
            failed,
            crate::outcome::conversion_failure(ConversionErrorKind::CleanupResidue, display),
            "§2.6.4 case 2: Failed yields exactly the §2.8.2 CleanupResidue catalog message — no re-authored string"
        );
        assert!(
            matches!(
                &failed,
                Some(OutcomeMsg::Failure { kind: ConversionErrorKind::CleanupResidue, text })
                    if text.contains(display) && !text.contains("{path}")
            ),
            "§2.6.4/§2.8.2: a failed-with-residue item is Failed(CleanupResidue) with the {{path}} slot substituted — never a clean success"
        );

        let succeeded = residue_item_reason(ResidueDisposition::Succeeded, display);
        assert_eq!(
            succeeded,
            Some(crate::outcome::residue_annotation(display)),
            "§2.6.4 case 1: a success-with-residue item carries the §2.8.2 NON-failure residue annotation \
             from the catalog — no re-authored string (P3.59, superseding P3.25's None)"
        );
        assert!(
            matches!(
                &succeeded,
                Some(OutcomeMsg::Residue { text })
                    if text.contains(display) && !text.contains("{path}")
            ),
            "§2.6.4/§2.8.2: case 1 is a Residue annotation with the {{path}} slot substituted — the item's \
             success STANDS (§5.7:830 'with where residue remains'), never downgraded to a failure"
        );
        assert!(
            !matches!(&succeeded, Some(OutcomeMsg::Failure { .. })),
            "§2.6.4 case 1: the annotation is NOT a Failure — residue never rewrites the terminal state \
             (§2.6.2/§2.1.3 'annotated, not an item failure'); this is what P3.25's rationale protected"
        );

        assert_eq!(
            residue_item_reason(ResidueDisposition::Cancelled, display),
            None,
            "§2.6.4 case 3: a cancelled-with-residue item stays Cancelled — §2.6.4 authors no per-item \
             sentence for it (the With-residue tail is BATCH-level, §2.8.2); RATIFIED as built"
        );
    }

    // §6.4.1 (G15): `split_residue_records` is the ONE §2.10.1 wire↔off-wire fork — the wire list mirrors the
    // records in order (what the user sees), the off-wire map keys each item to its REAL `PathBuf`.
    #[test]
    fn split_residue_records_forks_wire_and_off_wire() {
        let item0 = ItemId::from_index(0);
        let item1 = ItemId::from_index(1);
        let path0 = PathBuf::from("/a/.x.part");
        let path1 = PathBuf::from("/b/.y.part");
        let records = vec![
            ResidueRecord::new(item0, path0.clone()),
            ResidueRecord::new(item1, path1.clone()),
        ];

        let (warnings, real_paths) = split_residue_records(records);

        assert_eq!(
            warnings,
            vec![
                CleanupResidue {
                    item: item0,
                    residue_display: path0.to_string_lossy().into_owned(),
                },
                CleanupResidue {
                    item: item1,
                    residue_display: path1.to_string_lossy().into_owned(),
                },
            ],
            "§1.12: cleanup_incomplete mirrors the records in order (display-only)"
        );
        assert_eq!(
            real_paths.get(&item0),
            Some(&path0),
            "§0.4.4: item_residues keys the real residue PathBuf for the C9 reveal"
        );
        assert_eq!(
            real_paths.get(&item1),
            Some(&path1),
            "§0.4.4: item_residues keys the real residue PathBuf for the C9 reveal"
        );
        assert_eq!(
            real_paths.len(),
            2,
            "§0.4.4: one off-wire entry per distinct item"
        );
    }

    // §6.4.1 (G15): a repeated `ItemId` keeps the LAST real path off-wire while retaining EVERY warning (the
    // wire list is what the user sees; the C9 reveal resolves to the latest recorded residue for that item).
    #[test]
    fn split_residue_records_dedups_off_wire_but_keeps_every_warning() {
        let item = ItemId::from_index(7);
        let first = PathBuf::from("/a/.first.part");
        let last = PathBuf::from("/a/.last.part");
        let records = vec![
            ResidueRecord::new(item, first.clone()),
            ResidueRecord::new(item, last.clone()),
        ];

        let (warnings, real_paths) = split_residue_records(records);

        assert_eq!(
            warnings.len(),
            2,
            "§1.12: every residue warning is retained on the wire list"
        );
        assert_eq!(
            real_paths.len(),
            1,
            "§0.4.4: the off-wire map is keyed by ItemId — one entry per item"
        );
        assert_eq!(
            real_paths.get(&item),
            Some(&last),
            "§0.4.4: the LAST recorded real residue path wins for the reveal"
        );
    }

    // §6.4.1 (G15): the §2.8.2 "With residue" tail is appended IFF the run left residue — the run-level honesty
    // that residue may remain (esp. the §2.6.4 case-3 wedged-cancel gap). A residue-free run's line is returned
    // verbatim (no spurious tail).
    #[test]
    fn append_residue_tail_fires_only_with_residue() {
        let base = crate::outcome::BatchSummary::Cancelled { ok: 2 }.text();

        let with = append_residue_tail(base.clone(), true);
        assert_eq!(
            with,
            format!("{base} {}", crate::outcome::WITH_RESIDUE_TAIL),
            "§2.6.4 case 3 / §2.8.2: a cancelled run with a wedged-cancel residue gets the 'With residue' tail"
        );
        assert!(
            with.ends_with(crate::outcome::WITH_RESIDUE_TAIL),
            "§2.8.2: the tail is the final clause of the summary line"
        );

        assert_eq!(
            append_residue_tail(base.clone(), false),
            base,
            "§2.8.2: a residue-free run's summary line is unchanged — no spurious tail"
        );
    }
}

#[cfg(test)]
mod write_sequence_tests {
    //! §6.4.1 unit + real-FS integration (G15/G32(a)) for the §2.1.1 per-item PUBLISH LEGS (P3.38 → re-cut
    //! P3.48) — the composition of `crate::run` (temp/cleanup) + `crate::fs_guard` (publish/divert) over an
    //! ALREADY-WRITTEN publish temp (what the P3.48 conductor's `engines::dispatch` produces on the run path).
    //!
    //! [Test-Change: P3.48 — old-obsolete+new-correct, §2.1.1] The P3.48 ruling RE-CUT `write_item`: step 1
    //! (pick-temp) + step 2 (the engine write) moved conductor-side (`crate::orchestrator::convert_item`), the
    //! §1.7 non-empty exit-verification moved to the extracted [`verify_encode_output`], and steps 3-7 became
    //! [`publish_written_temp`]. So the P3.38 tests migrate onto the pieces they now exercise: the publish-leg
    //! tests PRE-WRITE the publish temp (`RunScratch::publish_temp` + `fs::write`, the two conductor-side steps)
    //! then call [`publish_written_temp`] (steps 3-7 verbatim — the assertions are UNCHANGED, verified against
    //! the same real FS); the empty/vanished tests target [`verify_encode_output`]; the engine-failure cleanup
    //! targets [`fail_cleanup`]. The ONE dropped case (the old `a_missing_publish_temp_dir_...` step-1-before-
    //! engine test) is obsolete BY CONSTRUCTION post-re-cut — the conductor picks the temp only AFTER
    //! `compute_output_plan`'s §2.7.2 `location_status` screen, which DIVERTS an unwritable dir rather than
    //! reaching a failing pick-temp, so "publish_temp_dir missing → WriteFailed before the engine" no longer
    //! arises on the run path (the `RunScratch::publish_temp` Err is tested in `crate::run`; the defensive
    //! Err→WriteFailed mapping lives in `convert_item`). The end-to-end real-engine CSV→TSV run + the §0.4.2
    //! event stream + the §1.12 projection are the `run_conversion_tests` below (the G31 output-validity + the
    //! conductor path the ruling names); this module keeps the engine-agnostic publish-leg invariants.
    use super::*;
    use crate::domain::InstanceId;
    use crate::fs_guard::{resolve_identity, PathTooLong};

    /// A real-FS fixture: a live run handle, a real source file (with its frozen identity), and a writable
    /// destination dir — every leg exercises the real primitives, nothing mocked (test-strategy §0.1).
    struct Fixture {
        _scratch_base: tempfile::TempDir,
        scratch: RunScratch,
        _src_dir: tempfile::TempDir,
        source: PathBuf,
        frozen: Vec<FileIdentity>,
        dest: tempfile::TempDir,
        cache: LocationCache,
    }

    impl Fixture {
        fn new(source_bytes: &[u8]) -> Self {
            let scratch_base = tempfile::tempdir().expect("a real scratch base dir");
            let scratch = RunScratch::acquire(
                scratch_base.path(),
                InstanceId::mint(),
                std::process::id(),
                RunId::mint(),
            )
            .expect("acquire the run scratch (lock held)");
            let src_dir = tempfile::tempdir().expect("a real source dir");
            let source = src_dir.path().join("data.csv");
            std::fs::write(&source, source_bytes).expect("write the source file");
            let frozen = vec![resolve_identity(&source).expect("resolve the source identity")];
            let dest = tempfile::tempdir().expect("a real destination dir");
            Self {
                _scratch_base: scratch_base,
                scratch,
                _src_dir: src_dir,
                source,
                frozen,
                dest,
                cache: LocationCache::new(),
            }
        }

        /// A beside-source plan writing into `dir` (both `final_dir` and the same-volume `publish_temp_dir`).
        fn plan_in(&self, dir: &Path) -> OutputPlan {
            OutputPlan {
                job: ItemId::from_index(0),
                final_dir: dir.to_path_buf(),
                diverted: None,
                base_name: OsString::from("data"),
                extension: OsString::from("tsv"),
                publish_temp_dir: dir.to_path_buf(),
            }
        }
    }

    /// A `crate::run`-shaped probe name for the §2.7.2 writability probe (created + removed by `location_status`).
    fn probe() -> impl Fn() -> OsString {
        || OsString::from(".convertia-test-probe.part")
    }

    /// The `.part` sibling names in `dir` — the leftover-temp assertion (a clean publish leaves none).
    fn part_files(dir: &Path) -> Vec<OsString> {
        std::fs::read_dir(dir)
            .expect("read the dir")
            .filter_map(Result::ok)
            .map(|e| e.file_name())
            .filter(|n| n.to_string_lossy().contains(".part"))
            .collect()
    }

    /// A writable, NON-ephemeral temp dir (under the crate source root, not `%TEMP%`) — a valid §2.7.3 divert
    /// TARGET, since `resolve_divert_target` rejects an ephemeral candidate (§2.7.2). `None` when the crate root
    /// is itself under an OS temp root (a clean skip, never a false pass — the fs_guard `non_ephemeral_tempdir`
    /// pattern). Real FS (test-strategy §0.1).
    fn non_ephemeral_dir() -> Option<tempfile::TempDir> {
        let dir = tempfile::Builder::new()
            .prefix("convertia-p338-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .expect("create a temp dir in the crate source root");
        (!crate::platform::is_ephemeral_output_dir(dir.path())).then_some(dir)
    }

    #[test]
    fn a_completed_write_publishes_beside_source_and_leaves_the_source_byte_identical() {
        let mut f = Fixture::new(b"a,b\n1,2\n");
        let before = std::fs::read(&f.source).expect("read source before");
        let plan = f.plan_in(f.dest.path());

        // [Test-Change: P3.48 — old-obsolete+new-correct, §2.1.1] pre-write the publish temp (the conductor's
        // pick-temp + the engine write) then run the §2.1.1 publish legs; the assertions are UNCHANGED.
        let tmp = f
            .scratch
            .publish_temp(&plan.publish_temp_dir, plan.job)
            .expect("pick the publish temp (step 1, conductor-side)");
        std::fs::write(&*tmp, b"a\tb\n1\t2\n").expect("the engine writes the output into the temp");
        let out = publish_written_temp(
            &plan,
            &f.source,
            &f.frozen,
            &[],
            &f.scratch,
            &mut f.cache,
            probe(),
            tmp,
        );

        // §2.1: published at the beside-source name `data.tsv`, carrying the written bytes.
        let output = f.dest.path().join("data.tsv");
        assert_eq!(
            out.disposition,
            WriteDisposition::Published {
                output: output.clone()
            }
        );
        assert_eq!(
            std::fs::read(&output).expect("read output"),
            b"a\tb\n1\t2\n",
            "the published file is exactly what the engine seam wrote"
        );
        assert!(!out.diverted, "a beside-source publish is not diverted");
        assert!(out.residue.is_none(), "a clean publish leaves no residue");
        // G32(a): the source bytes are untouched (no-harm).
        assert_eq!(
            std::fs::read(&f.source).expect("read source after"),
            before,
            "G32(a): the source file is byte-identical after the conversion"
        );
        // §2.6.2: no leftover `.part` — the renamed publish temp is cleaned (idempotent NotFound) and the
        // name-only EXDEV intermediate was never created on this same-volume publish.
        assert!(
            part_files(f.dest.path()).is_empty(),
            "no `.part` residue expected, found {:?}",
            part_files(f.dest.path())
        );
    }

    #[test]
    fn a_name_collision_publishes_the_next_numbered_variant_without_clobbering() {
        let mut f = Fixture::new(b"x\n");
        // A file already at the target name — the no-clobber publish must NEVER overwrite it.
        std::fs::write(f.dest.path().join("data.tsv"), b"pre-existing")
            .expect("seed the collision");
        let plan = f.plan_in(f.dest.path());

        // [Test-Change: P3.48 — old-obsolete+new-correct, §2.2.2] pre-write the publish temp, then publish.
        let tmp = f
            .scratch
            .publish_temp(&plan.publish_temp_dir, plan.job)
            .expect("pick the publish temp");
        std::fs::write(&*tmp, b"fresh").expect("write the output into the temp");
        let out = publish_written_temp(
            &plan,
            &f.source,
            &f.frozen,
            &[],
            &f.scratch,
            &mut f.cache,
            probe(),
            tmp,
        );

        // §2.2.2: the collision bumps to the space-paren numbered variant, never a replace.
        let output = f.dest.path().join("data (1).tsv");
        assert_eq!(
            out.disposition,
            WriteDisposition::Published {
                output: output.clone()
            }
        );
        assert_eq!(std::fs::read(&output).expect("read output"), b"fresh");
        assert_eq!(
            std::fs::read(f.dest.path().join("data.tsv")).expect("read the pre-existing file"),
            b"pre-existing",
            "no-clobber: the pre-existing file is untouched"
        );
    }

    #[test]
    fn an_engine_error_cleans_the_temp_and_never_creates_final() {
        // [Test-Change: P3.48 — old-obsolete+new-correct, §2.1.1] the engine-write step is conductor-side now
        // (`convert_item`'s dispatch `Failed(kind)` arm calls `fail_cleanup(item, [tmp], kind)`), so this
        // migrates onto that cleanup mechanic directly: a written partial temp is REMOVED (§2.6.2 / step 7),
        // the item fails with the engine's kind, no residue, and `final` was never created (the publish legs
        // never ran). The end-to-end real-engine failure is `run_conversion_tests` below.
        let f = Fixture::new(b"src\n");
        let before = std::fs::read(&f.source).expect("read source before");
        let plan = f.plan_in(f.dest.path());

        // The engine wrote a partial into the temp, then §1.7 dispatch reported `Failed(Corrupt)`.
        let tmp = f
            .scratch
            .publish_temp(&plan.publish_temp_dir, plan.job)
            .expect("pick the publish temp");
        std::fs::write(&*tmp, b"partial").expect("the engine wrote a partial before failing");
        let out = fail_cleanup(plan.job, [tmp], ConversionErrorKind::Corrupt);

        assert_eq!(
            out.disposition,
            WriteDisposition::Failed {
                kind: ConversionErrorKind::Corrupt
            }
        );
        assert!(
            out.residue.is_none(),
            "the temp was removed cleanly — no residue"
        );
        assert!(
            !f.dest.path().join("data.tsv").exists(),
            "no `final` on an engine failure (the publish legs never ran)"
        );
        assert!(
            part_files(f.dest.path()).is_empty(),
            "the partial temp is removed on failure, found {:?}",
            part_files(f.dest.path())
        );
        assert_eq!(
            std::fs::read(&f.source).expect("read source after"),
            before,
            "G32(a): a failed item never touches the source"
        );
    }

    #[test]
    fn an_empty_output_is_a_failure_not_a_clean_success() {
        // [Test-Change: P3.48 — old-obsolete+new-correct, §1.7] the §1.7 non-empty exit-verification moved out
        // of `write_item` onto the conductor's Succeeded path, extracted as `verify_encode_output`; this
        // migrates onto it — a present-but-0-byte publish temp is a §2.8 `Empty` failure, never a clean success.
        let f = Fixture::new(b"src\n");
        let plan = f.plan_in(f.dest.path());

        // A picked-but-never-written temp is 0-byte (the "success exit, empty output" case).
        let tmp = f
            .scratch
            .publish_temp(&plan.publish_temp_dir, plan.job)
            .expect("pick the publish temp");
        let out = verify_encode_output(plan.job, tmp)
            .expect_err("§1.7: a 0-byte output fails verification, never a clean success");

        assert_eq!(
            out.disposition,
            WriteDisposition::Failed {
                kind: ConversionErrorKind::Empty
            },
            "§1.7: a success exit with zero output is a §2.8 Empty failure, never a clean success"
        );
        assert!(
            !f.dest.path().join("data.tsv").exists(),
            "no `final` for an empty output (the publish legs never ran)"
        );
    }

    #[test]
    fn a_vanished_output_after_success_is_an_internal_error() {
        // [Test-Change: P3.48 — old-obsolete+new-correct, §1.7] `verify_encode_output` on a VANISHED temp (the
        // engine reported success but its output is gone — an internal contract violation) → §2.13 InternalError.
        let f = Fixture::new(b"src\n");
        let plan = f.plan_in(f.dest.path());

        // [Test-Change: P3.48 — old-obsolete+new-correct, §1.7] the old closure-based seam's
        // `remove_file(tmp).expect("mid-seam")` (inside `write_item`'s `FnOnce`) is obsolete — `write_item` was
        // re-cut, so the vanish is now staged DIRECTLY here (pick the temp, remove it, then verify).
        let tmp = f
            .scratch
            .publish_temp(&plan.publish_temp_dir, plan.job)
            .expect("pick the publish temp");
        std::fs::remove_file(&*tmp)
            .expect("remove the temp — the output vanished after a 'successful' exit");
        let out = verify_encode_output(plan.job, tmp)
            .expect_err("§1.7: a vanished output fails verification");

        assert_eq!(
            out.disposition,
            WriteDisposition::Failed {
                kind: ConversionErrorKind::InternalError
            },
            "§1.7: a vanished output after a 'successful' exit is an internal fault"
        );
    }

    // [Test-Change: P3.48 — old-obsolete+new-correct, §2.6.2] the old closure-based
    // `a_missing_publish_temp_dir_fails_write_failed_before_the_engine_runs` test is RE-CUT: post-re-cut the
    // temp is picked conductor-side, so its `write_item(bad_publish_temp_dir)` pre-engine WriteFailed check +
    // its `assert!(!ran)` engine-never-ran closure assertion are obsolete (the `RunScratch::publish_temp` Err is
    // tested in `crate::run`; the `convert_item` Err→WriteFailed mapping is exercised in `run_conversion_tests`).
    // The WriteFailed OUTCOME assertion is RETAINED here on `publish_written_temp` and STRENGTHENED to also pin
    // the §2.6.2 publish-temp cleanup on a failed publish (the old test could not — its temp was the seam's).
    #[test]
    fn a_failed_publish_is_write_failed_and_cleans_its_publish_temp() {
        let mut f = Fixture::new(b"src\n");
        // A file-as-final-dir → the §2.3.3 parent-handle open rejects it → §2.8 WriteFailed (the reliably-driven
        // publish failure; the sibling `a_final_dir_...` pins the unrelated-file no-harm on the same trigger).
        let file_final = f.dest.path().join("i-am-a-file");
        std::fs::write(&file_final, b"not a dir").expect("create the file");
        let plan = OutputPlan {
            job: ItemId::from_index(0),
            final_dir: file_final,
            diverted: None,
            base_name: OsString::from("data"),
            extension: OsString::from("tsv"),
            publish_temp_dir: f.dest.path().to_path_buf(),
        };
        let tmp = f
            .scratch
            .publish_temp(&plan.publish_temp_dir, plan.job)
            .expect("pick the publish temp");
        std::fs::write(&*tmp, b"out").expect("write the output into the temp");
        let tmp_path = tmp.to_path_buf();
        let out = publish_written_temp(
            &plan,
            &f.source,
            &f.frozen,
            &[],
            &f.scratch,
            &mut f.cache,
            probe(),
            tmp,
        );

        assert_eq!(
            out.disposition,
            WriteDisposition::Failed {
                kind: ConversionErrorKind::WriteFailed
            }
        );
        assert!(
            !tmp_path.exists(),
            "§2.6.2: a failed publish cleans its own publish temp — no leftover .part, no residue"
        );
        assert!(
            out.residue.is_none(),
            "§2.6.4: a clean temp removal surfaces no residue on the failure"
        );
    }

    #[test]
    fn a_final_dir_that_is_a_file_fails_write_failed() {
        let mut f = Fixture::new(b"src\n");
        // `final_dir` is a real FILE, not a directory — the §2.3.3 parent-handle open rejects it.
        let file_final = f.dest.path().join("i-am-a-file");
        std::fs::write(&file_final, b"not a dir").expect("create the file");
        let plan = OutputPlan {
            job: ItemId::from_index(0),
            final_dir: file_final.clone(),
            diverted: None,
            base_name: OsString::from("data"),
            extension: OsString::from("tsv"),
            publish_temp_dir: f.dest.path().to_path_buf(),
        };

        // [Test-Change: P3.48 — old-obsolete+new-correct, §2.3.3] pre-write the publish temp, then publish; the
        // §2.3.3 parent-handle open rejects the file-as-final-dir → §2.8 WriteFailed.
        let tmp = f
            .scratch
            .publish_temp(&plan.publish_temp_dir, plan.job)
            .expect("pick the publish temp");
        std::fs::write(&*tmp, b"out").expect("write the output into the temp");
        let out = publish_written_temp(
            &plan,
            &f.source,
            &f.frozen,
            &[],
            &f.scratch,
            &mut f.cache,
            probe(),
            tmp,
        );

        assert_eq!(
            out.disposition,
            WriteDisposition::Failed {
                kind: ConversionErrorKind::WriteFailed
            }
        );
        assert_eq!(
            std::fs::read(&file_final).expect("the file is untouched"),
            b"not a dir",
            "a failed publish never harms an unrelated file at the final path"
        );
    }

    #[test]
    fn map_publish_error_maps_each_leaf_verdict_to_its_taxonomy_kind() {
        // The tier-1 leaf-verdict -> §2.8 boundary (the hard-to-drive publish errors, covered directly). The
        // second tuple slot is the §2.2.4 offending token — `None` for every non-unopenable verdict.
        assert_eq!(
            map_publish_error(&PublishError::PathTooLong(PathTooLong::Total)),
            (ConversionErrorKind::PathTooLong, None)
        );
        assert_eq!(
            map_publish_error(&PublishError::TooManyCollisions),
            (ConversionErrorKind::TooManyCollisions, None)
        );
        assert_eq!(
            map_publish_error(&PublishError::OutOfDisk),
            (ConversionErrorKind::OutOfDisk, None)
        );
        assert_eq!(
            map_publish_error(&PublishError::Io(std::io::Error::other("write failed"))),
            (ConversionErrorKind::WriteFailed, None)
        );
        // §2.2.4 (P3.88): the unopenable-name verdict maps to `UnopenableOutputName` AND carries the offending
        // token (the second slot), so the §2.8 message can NAME it.
        assert_eq!(
            map_publish_error(&PublishError::UnopenableName("CON.tsv".to_owned())),
            (
                ConversionErrorKind::UnopenableOutputName,
                Some("CON.tsv".to_owned())
            )
        );
    }

    #[test]
    fn a_parent_resolving_onto_a_frozen_source_diverts_and_never_publishes_onto_an_original() {
        let mut f = Fixture::new(b"a,b\n");
        // Freeze the DESTINATION dir's own identity into the frozen set — so the §2.3.3 parent-handle verify
        // sees `final_dir` resolve ONTO a frozen source and must divert (never publish onto an original). This
        // drives the ResolvesOntoSource → late-divert wiring on EVERY platform (no read-only-dir permissions).
        let frozen = vec![resolve_identity(f.dest.path()).expect("resolve the dest dir identity")];
        let Some(divert) = non_ephemeral_dir() else {
            return; // the crate root is itself ephemeral — no valid divert target to test against.
        };
        let plan = f.plan_in(f.dest.path());

        // [Test-Change: P3.48 — old-obsolete+new-correct, §2.3.3] pre-write the publish temp, then publish; the
        // §2.3.3 parent verify sees `final_dir` resolve onto a frozen source → the §2.7 late-divert.
        let tmp = f
            .scratch
            .publish_temp(&plan.publish_temp_dir, plan.job)
            .expect("pick the publish temp");
        std::fs::write(&*tmp, b"a\tb\n").expect("write the output into the temp");
        let divert_candidates = [divert.path().to_path_buf()];
        let out = publish_written_temp(
            &plan,
            &f.source,
            &frozen,
            &divert_candidates,
            &f.scratch,
            &mut f.cache,
            probe(),
            tmp,
        );

        // §2.3.3/§2.7: the output diverted to the (empty) divert root under the base name.
        let output = divert.path().join("data.tsv");
        assert_eq!(
            out.disposition,
            WriteDisposition::Published {
                output: output.clone()
            },
            "a resolves-onto-source parent diverts rather than publishing onto the original"
        );
        assert!(out.diverted, "§2.7.3: the diverted flag is set");
        assert_eq!(
            std::fs::read(&output).expect("read the diverted output"),
            b"a\tb\n"
        );
        assert!(
            !f.dest.path().join("data.tsv").exists(),
            "§2.3.3: nothing was published into the frozen (resolves-onto-source) location"
        );
    }

    // -- §2.7.2/§2.7.5 late-divert (Unix — a read-only `final_dir` injects the writability flip; a 0o500 dir
    // blocks writes only for a non-root user, so the divert-trigger tests self-skip under root). --

    #[cfg(unix)]
    fn running_as_root() -> bool {
        use std::os::unix::fs::PermissionsExt;
        // Probe: a 0o500 dir a non-root user cannot write into; root ignores the mode -> we are root, skip.
        let probe_dir = tempfile::tempdir().expect("a probe dir");
        std::fs::set_permissions(probe_dir.path(), std::fs::Permissions::from_mode(0o500))
            .expect("chmod the probe dir read-only");
        let root = std::fs::write(probe_dir.path().join("w"), b"").is_ok();
        std::fs::set_permissions(probe_dir.path(), std::fs::Permissions::from_mode(0o700)).ok();
        root
    }

    #[cfg(unix)]
    #[test]
    fn a_writability_flip_late_diverts_to_the_divert_root() {
        use std::os::unix::fs::PermissionsExt;
        if running_as_root() {
            return; // 0o500 does not block root — the writability trigger cannot fire.
        }
        let mut f = Fixture::new(b"a,b\n");
        let before = std::fs::read(&f.source).expect("read source before");
        // Temp lands in a WRITABLE dir; `final_dir` is a separate dir we flip read-only -> the publish EACCES.
        let temp_vol = tempfile::tempdir().expect("a writable temp-volume dir");
        let readonly_final = tempfile::tempdir().expect("the (soon read-only) final dir");
        let Some(divert) = non_ephemeral_dir() else {
            return; // the crate root is itself ephemeral — no valid divert target to test against.
        };
        let plan = OutputPlan {
            job: ItemId::from_index(0),
            final_dir: readonly_final.path().to_path_buf(),
            diverted: None,
            base_name: OsString::from("data"),
            extension: OsString::from("tsv"),
            publish_temp_dir: temp_vol.path().to_path_buf(),
        };
        std::fs::set_permissions(
            readonly_final.path(),
            std::fs::Permissions::from_mode(0o500),
        )
        .expect("flip the final dir read-only");

        // [Test-Change: P3.48 — old-obsolete+new-correct, §2.7.2] pre-write the publish temp (in the WRITABLE
        // temp-volume dir, unaffected by the final-dir flip) then publish; the read-only `final_dir` EACCES on
        // the exclusive publish → the §2.7.2/§2.7.5 late-divert to the divert root.
        let tmp = f
            .scratch
            .publish_temp(&plan.publish_temp_dir, plan.job)
            .expect("pick the publish temp (in the writable temp-volume dir)");
        std::fs::write(&*tmp, b"a\tb\n").expect("write the output into the temp");
        let divert_candidates = [divert.path().to_path_buf()];
        let out = publish_written_temp(
            &plan,
            &f.source,
            &f.frozen,
            &divert_candidates,
            &f.scratch,
            &mut f.cache,
            probe(),
            tmp,
        );

        // Restore perms so the tempdir can clean itself up.
        std::fs::set_permissions(
            readonly_final.path(),
            std::fs::Permissions::from_mode(0o700),
        )
        .ok();

        // §2.7.3: the output diverted under the divert root (empty → the base `data.tsv` name).
        let output = divert.path().join("data.tsv");
        assert_eq!(
            out.disposition,
            WriteDisposition::Published {
                output: output.clone()
            },
            "the output diverted to the divert root"
        );
        assert!(
            out.diverted,
            "§2.7.3: the diverted flag is set for the run's divert_root_display"
        );
        assert_eq!(
            std::fs::read(&output).expect("read the diverted output"),
            b"a\tb\n"
        );
        assert!(
            !readonly_final.path().join("data.tsv").exists(),
            "§2.7.5: nothing was published into the unwritable original location"
        );
        assert_eq!(
            std::fs::read(&f.source).expect("read source after"),
            before,
            "G32(a): a diverted conversion still never touches the source"
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_already_diverted_plan_does_not_re_divert_on_a_second_writability_failure() {
        use std::os::unix::fs::PermissionsExt;
        if running_as_root() {
            return;
        }
        let mut f = Fixture::new(b"a,b\n");
        let temp_vol = tempfile::tempdir().expect("a writable temp-volume dir");
        let readonly_final = tempfile::tempdir().expect("the (soon read-only) final dir");
        let divert = tempfile::tempdir().expect("a writable candidate (must NOT be used)");
        // The plan was ALREADY diverted at C4 — a second write-time failure is terminal (one divert per item).
        let plan = OutputPlan {
            job: ItemId::from_index(0),
            final_dir: readonly_final.path().to_path_buf(),
            diverted: Some(DivertReason::Unwritable),
            base_name: OsString::from("data"),
            extension: OsString::from("tsv"),
            publish_temp_dir: temp_vol.path().to_path_buf(),
        };
        std::fs::set_permissions(
            readonly_final.path(),
            std::fs::Permissions::from_mode(0o500),
        )
        .expect("flip the final dir read-only");

        // [Test-Change: P3.48 — old-obsolete+new-correct, §2.7.3] pre-write the publish temp then publish; an
        // ALREADY-diverted plan (§2.7.3 one-divert-per-item) does not divert a second time on the read-only
        // final → terminal WriteFailed, the divert candidate never touched.
        let tmp = f
            .scratch
            .publish_temp(&plan.publish_temp_dir, plan.job)
            .expect("pick the publish temp");
        std::fs::write(&*tmp, b"a\tb\n").expect("write the output into the temp");
        let divert_candidates = [divert.path().to_path_buf()];
        let out = publish_written_temp(
            &plan,
            &f.source,
            &f.frozen,
            &divert_candidates,
            &f.scratch,
            &mut f.cache,
            probe(),
            tmp,
        );

        std::fs::set_permissions(
            readonly_final.path(),
            std::fs::Permissions::from_mode(0o700),
        )
        .ok();

        assert_eq!(
            out.disposition,
            WriteDisposition::Failed {
                kind: ConversionErrorKind::WriteFailed
            },
            "§2.7.3: an already-diverted plan does not divert a second time"
        );
        assert!(
            part_files(divert.path()).is_empty() && !divert.path().join("data.tsv").exists(),
            "the divert candidate was never written — no re-divert"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_writability_flip_with_no_usable_divert_target_fails_write_failed() {
        use std::os::unix::fs::PermissionsExt;
        if running_as_root() {
            return;
        }
        let mut f = Fixture::new(b"a,b\n");
        let temp_vol = tempfile::tempdir().expect("a writable temp-volume dir");
        let readonly_final = tempfile::tempdir().expect("the (soon read-only) final dir");
        let plan = OutputPlan {
            job: ItemId::from_index(0),
            final_dir: readonly_final.path().to_path_buf(),
            diverted: None,
            base_name: OsString::from("data"),
            extension: OsString::from("tsv"),
            publish_temp_dir: temp_vol.path().to_path_buf(),
        };
        std::fs::set_permissions(
            readonly_final.path(),
            std::fs::Permissions::from_mode(0o500),
        )
        .expect("flip the final dir read-only");

        // [Test-Change: P3.48 — old-obsolete+new-correct, §2.7.3] pre-write the publish temp then publish with
        // NO divert candidates → the read-only final EACCES late-diverts, `resolve_divert_target([])` yields
        // Unavailable → §2.8 WriteFailed (never a bad divert).
        let tmp = f
            .scratch
            .publish_temp(&plan.publish_temp_dir, plan.job)
            .expect("pick the publish temp");
        std::fs::write(&*tmp, b"a\tb\n").expect("write the output into the temp");
        let out = publish_written_temp(
            &plan,
            &f.source,
            &f.frozen,
            &[],
            &f.scratch,
            &mut f.cache,
            probe(),
            tmp,
        );

        std::fs::set_permissions(
            readonly_final.path(),
            std::fs::Permissions::from_mode(0o700),
        )
        .ok();

        assert_eq!(
            out.disposition,
            WriteDisposition::Failed {
                kind: ConversionErrorKind::WriteFailed
            },
            "§2.7.3: no usable divert target -> the item fails clearly, never a bad divert"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_extension_is_an_internal_error() {
        use std::os::unix::ffi::OsStringExt;
        let mut f = Fixture::new(b"src\n");
        let plan = OutputPlan {
            job: ItemId::from_index(0),
            final_dir: f.dest.path().to_path_buf(),
            diverted: None,
            base_name: OsString::from("data"),
            extension: OsString::from_vec(vec![0xff, 0xfe]), // invalid UTF-8
            publish_temp_dir: f.dest.path().to_path_buf(),
        };

        // [Test-Change: P3.48 — old-obsolete+new-correct, §2.1.1] pre-write the publish temp then publish; the
        // non-UTF-8 extension fails `InternalError` — and (post-re-cut) the ALREADY-written temp is CLEANED (the
        // ext check now runs after the write, so it removes the temp rather than returning before any exists).
        let tmp = f
            .scratch
            .publish_temp(&plan.publish_temp_dir, plan.job)
            .expect("pick the publish temp");
        std::fs::write(&*tmp, b"out").expect("write the output into the temp");
        let out = publish_written_temp(
            &plan,
            &f.source,
            &f.frozen,
            &[],
            &f.scratch,
            &mut f.cache,
            probe(),
            tmp,
        );

        assert_eq!(
            out.disposition,
            WriteDisposition::Failed {
                kind: ConversionErrorKind::InternalError
            },
            "a non-UTF-8 target extension is an internal fault (never a user-facing case)"
        );
        assert!(
            part_files(f.dest.path()).is_empty(),
            "the already-written temp is cleaned on the ext-None path, found {:?}",
            part_files(f.dest.path())
        );
    }
}

#[cfg(test)]
mod run_conversion_e2e_tests;

#[cfg(test)]
mod link_safety_e2e_tests;

#[cfg(test)]
mod cross_volume_e2e_tests;

#[cfg(test)]
mod run_conversion_tests {
    //! §6.4.1 unit + §6.4.3 per-pair integration (G15/G31/G32(a)) for the P3.48 C6 run conductor — the REAL
    //! native CSV↔TSV engine driven end-to-end over a DIRECTLY-registered frozen set (test-strategy §0.1: the
    //! conversion IS the product, so the engine + FS are real, never mocked; the full C1→C6→summary E2E is
    //! P3.49/P3.63). Pins: the output-VALIDITY bar (a real field-parse read-back of the published TSV + the
    //! §0.2 CSV-injection literal-preservation check, G31), the no-harm G32(a) source-unchanged invariant, the
    //! §1.12 `RunResult` projection + `Totals`, the §0.4.4 `RunResultStore` retention (C8 re-serve), the §1.9
    //! pre-flight-skip projection (no live events), the §2.5 `RerunDecision` applier, and the §0.4.2 event
    //! stream. [Build-Session-Entscheidung: P3.48]
    //!
    //! The record-building + conductor-driving helpers below are `pub(super)` so the sibling P3.63
    //! [`run_conversion_e2e_tests`](super::run_conversion_e2e_tests) suite — which lives in a
    //! `_TEST_PATH_RE`-matching file (the G23 conversion-command→test home) — reuses the SAME C6-path
    //! harness rather than duplicating it. [Build-Session-Entscheidung: P3.63]
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use tauri::ipc::{Channel, InvokeResponseBody};

    use super::*;
    use crate::domain::{
        Availability, Confidence, DetectionOutcome, InstanceId, ItemId, ItemPaths,
    };
    use crate::fs_guard::resolve_identity;

    /// A stable `CollectedSetId` for the frozen set (`CollectedSetId` has no `mint`; minted through its public
    /// bare-uuid `Deserialize` wire form, mirroring the C6/C8 contract test helpers).
    fn a_collected_set_id() -> CollectedSetId {
        serde_json::from_str(r#""55555555-5555-4555-8555-555555555555""#)
            .expect("CollectedSetId deserializes from a uuid string")
    }

    /// A NON-ephemeral source directory (under the crate source root, not `%TEMP%`) — REALISTIC user-source
    /// placement (Downloads/Documents/Desktop). A plain `tempfile::tempdir()` lives under the OS temp root,
    /// which the conductor's §2.7.2 `location_status` correctly classifies `Ephemeral` → DIVERT (a result there
    /// could be silently purged), so a beside-source publish must run from a non-ephemeral dir. `None` on the
    /// pathological env where the crate root is itself under an OS temp root (a clean skip, never a false pass —
    /// the `write_sequence_tests` / `location_status_tests` `non_ephemeral_tempdir` pattern). Real FS (§0.1).
    pub(super) fn non_ephemeral_source_dir() -> Option<tempfile::TempDir> {
        let dir = tempfile::Builder::new()
            .prefix("convertia-p348-run-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .expect("create a temp dir in the crate source root");
        (!crate::platform::is_ephemeral_output_dir(dir.path())).then_some(dir)
    }

    /// The §1.5 CSV→TSV target the conductor writes (mirrors `engines::slice_target(Csv)`).
    fn tsv_target() -> Target {
        Target {
            id: TargetId::Format(UserFacingFormat::Tsv),
            label: "TSV".to_owned(),
            lossy: None,
            availability: Availability::Available,
            options: Vec::new(),
        }
    }

    /// A REAL eligible CSV source file at `dir/name` (id `n`) + its `DroppedItem` / off-wire `ItemPaths` /
    /// §2.3 `FileIdentity` — the frozen record the §1.1 freeze would build (test-strategy §0.1: a real file).
    pub(super) fn eligible(
        dir: &Path,
        name: &str,
        n: u32,
        bytes: &[u8],
    ) -> (DroppedItem, ItemPaths, FileIdentity) {
        let source = dir.join(name);
        std::fs::write(&source, bytes).expect("write the real source file");
        let item = ItemId::from_index(n);
        let dropped = DroppedItem {
            item,
            display_name: name.to_owned(),
            rel_path_display: None,
            size_bytes: bytes.len() as u64,
            detected: DetectionOutcome::Recognized {
                format: UserFacingFormat::Csv,
                confidence: Confidence::High,
                dims: None,
            },
        };
        let paths = ItemPaths {
            raw_path: source.clone(),
            resolved_path: source.clone(),
        };
        let identity = resolve_identity(&source).expect("resolve the source identity");
        (dropped, paths, identity)
    }

    /// A pre-flight-skipped item at id `n` (unsupported at freeze) — no `item_paths`/identity (never converted).
    fn skipped(n: u32, reason: SkipReason) -> SkippedItem {
        SkippedItem {
            item: ItemId::from_index(n),
            source_display: format!("skipped-{n}.bin"),
            detected_display: None,
            reason,
        }
    }

    /// Assemble a `RegisteredSet` (frozen set + §2.3 identity table) from real eligible items + skipped records,
    /// with the source `dir` as the §2.4 dropped root (the §2.7.4 beside-source open-folder anchor).
    pub(super) fn registered(
        dir: &Path,
        eligibles: Vec<(DroppedItem, ItemPaths, FileIdentity)>,
        skips: Vec<SkippedItem>,
    ) -> RegisteredSet {
        let mut items = Vec::new();
        let mut item_paths: BTreeMap<ItemId, ItemPaths> = BTreeMap::new();
        let mut identities: BTreeMap<ItemId, FileIdentity> = BTreeMap::new();
        for (dropped, paths, identity) in eligibles {
            identities.insert(dropped.item, identity);
            item_paths.insert(dropped.item, paths);
            items.push(dropped);
        }
        let count = items.len();
        let frozen = FrozenCollectedSet {
            id: a_collected_set_id(),
            instance: InstanceId::mint(),
            format: UserFacingFormat::Csv,
            items,
            count,
            skipped: skips,
            total_bytes: 0,
            roots: vec![dir.to_path_buf()],
            encoding_hint: None,
            delimiter_hint: None,
            notes: Vec::new(),
            item_paths,
        };
        RegisteredSet { frozen, identities }
    }

    /// A capturing run Channel — records each sent `ConversionEvent`'s serialized JSON (the outbound wire form;
    /// `ConversionEvent` is Serialize-only, so the test asserts on the JSON, never a deserialized value).
    pub(super) fn capture_channel() -> (Channel<ConversionEvent>, Arc<Mutex<Vec<String>>>) {
        let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&events);
        let channel = Channel::new(move |body: InvokeResponseBody| {
            if let InvokeResponseBody::Json(json) = body {
                sink.lock().expect("event sink lock").push(json);
            }
            Ok(())
        });
        (channel, events)
    }

    /// Parse a delimited output (split lines by `\n`, fields by `delim`) into rows of cells — a real field
    /// read-back of the produced file (test-strategy §0.2: not a byte-blob, the cells are parsed + asserted).
    fn parse_rows(bytes: &[u8], delim: char) -> Vec<Vec<String>> {
        String::from_utf8_lossy(bytes)
            .lines()
            .map(|line| line.split(delim).map(str::to_owned).collect())
            .collect()
    }

    /// The stores + scratch a run needs — freshly constructed per test (nothing shared, nothing mocked).
    pub(super) struct Deps {
        _scratch_base: tempfile::TempDir,
        instance: InstanceId,
        scratch_base: PathBuf,
        pool: Pool,
        ledger: RerunLedger,
        equiv: EquivKeyComputer,
        pub(super) results: RunResultStore,
        runs: RunRegistry,
    }

    pub(super) fn deps() -> Deps {
        let scratch_base = tempfile::tempdir().expect("scratch base dir");
        Deps {
            scratch_base: scratch_base.path().to_path_buf(),
            _scratch_base: scratch_base,
            instance: InstanceId::mint(),
            pool: Pool::new(),
            ledger: RerunLedger::default(),
            equiv: EquivKeyComputer::default(),
            results: RunResultStore::default(),
            runs: RunRegistry::default(),
        }
    }

    /// Acquire a fresh scratch + run the conductor over `registered` with the given decision, into `dest_dir`
    /// as the §2.7.3 divert root (beside-source runs never use it). Returns the run's `RunId`.
    pub(super) async fn run(
        d: &Deps,
        registered: &RegisteredSet,
        rerun: RerunDecision,
        divert_root: &Path,
        channel: &Channel<ConversionEvent>,
    ) -> RunId {
        run_with_token(
            d,
            registered,
            rerun,
            divert_root,
            channel,
            CancellationToken::new(),
            ResolvedDestination::BesideSource,
        )
        .await
    }

    /// `run` with an explicit run token + destination — the Cancelled-arm test hands in a PRE-cancelled token
    /// (§1.7 cooperative cancel → `InvocationResult::Cancelled`); the ChosenRoot test hands in a
    /// `DestinationChoice::ChosenRoot` to exercise the §2.7.1 subtree path.
    #[allow(clippy::too_many_arguments)]
    async fn run_with_token(
        d: &Deps,
        registered: &RegisteredSet,
        rerun: RerunDecision,
        divert_root: &Path,
        channel: &Channel<ConversionEvent>,
        token: CancellationToken,
        destination: ResolvedDestination,
    ) -> RunId {
        let batch = build_batch(
            &registered.frozen,
            tsv_target(),
            OptionValues(BTreeMap::new()),
            destination,
        );
        let run_id = RunId::mint();
        let scratch = RunScratch::acquire(&d.scratch_base, d.instance, std::process::id(), run_id)
            .expect("acquire the run scratch");
        run_conversion(
            batch,
            registered,
            run_id,
            token,
            scratch,
            d.instance,
            Some(divert_root.to_path_buf()),
            rerun,
            &d.pool,
            &d.ledger,
            &d.equiv,
            &d.results,
            &d.runs,
            channel,
        )
        .await;
        run_id
    }

    // §6.4.3 integration (G31/G32(a)): a real CSV→TSV conversion publishes a VALID tab-delimited TSV beside the
    // source (field-parsed read-back + a §0.2 CSV-injection literal-preservation check), never touches the
    // source (no-harm), and the §1.12 summary is retained for the C8 re-serve with one Succeeded item.
    #[tokio::test]
    async fn converts_csv_to_tsv_reads_back_valid_preserves_injection_and_retains_the_summary() {
        let Some(src_dir) = non_ephemeral_source_dir() else {
            return; // the crate root is itself ephemeral — no realistic non-ephemeral source dir to test.
        };
        let d = deps();
        // A leading `=` cell (a CSV-injection payload) must survive as LITERAL text (§0.2 non-execution).
        let source_bytes = b"=cmd(),b\n1,2\n";
        let (dropped, paths, identity) = eligible(src_dir.path(), "data.csv", 0, source_bytes);
        let source = paths.resolved_path.clone();
        let set = registered(src_dir.path(), vec![(dropped, paths, identity)], Vec::new());
        let (channel, _events) = capture_channel();

        let run_id = run(&d, &set, RerunDecision::FreshCopy, src_dir.path(), &channel).await;

        // Output validity (G31): a tab-delimited TSV published beside the source, field-parsed to the same cells.
        let output = src_dir.path().join("data.tsv");
        let out = std::fs::read(&output).expect("the TSV output was published beside the source");
        let rows = parse_rows(&out, '\t');
        assert_eq!(
            rows,
            vec![
                vec!["=cmd()".to_owned(), "b".to_owned()],
                vec!["1".to_owned(), "2".to_owned()],
            ],
            "G31: the CSV converted to a tab-delimited TSV, the leading `=` cell preserved LITERALLY (§0.2 CSV-injection non-execution)"
        );
        assert!(
            !out.contains(&b','),
            "the TSV output carries no comma delimiter (the CSV became tab-delimited)"
        );
        // No-harm (G32(a)): the source is byte-identical.
        assert_eq!(
            std::fs::read(&source).expect("read source after"),
            source_bytes,
            "G32(a): the source file is byte-identical after the conversion"
        );
        // §1.12 projection + §0.4.4 retention: C8 re-serves the summary; one item Succeeded.
        let result = d
            .results
            .get(run_id)
            .expect("the terminal RunResult is retained for the C8 re-serve");
        assert_eq!(
            result.totals,
            Totals {
                succeeded: 1,
                failed: 0,
                cancelled: 0,
                skipped: 0,
            }
        );
        assert_eq!(result.items.len(), 1, "one item in the summary");
        assert!(
            matches!(result.items[0].state, JobState::Succeeded),
            "the converted item is Succeeded"
        );
    }

    // §6.4.1 (G15): the §0.4.2 event stream is emitted in order — RunStarted first, then per-item
    // ItemStarted → ItemFinished (with at least one BatchProgress), terminal RunFinished last. Asserted on the
    // serialized JSON `type` tags (ConversionEvent is Serialize-only).
    #[tokio::test]
    async fn emits_the_0_4_2_event_stream_in_order() {
        let Some(src_dir) = non_ephemeral_source_dir() else {
            return; // the crate root is itself ephemeral — no realistic non-ephemeral source dir to test.
        };
        let d = deps();
        let (dropped, paths, identity) = eligible(src_dir.path(), "data.csv", 0, b"a,b\n1,2\n");
        let set = registered(src_dir.path(), vec![(dropped, paths, identity)], Vec::new());
        let (channel, events) = capture_channel();

        run(&d, &set, RerunDecision::FreshCopy, src_dir.path(), &channel).await;

        let events = events.lock().expect("event sink lock").clone();
        let tag = |needle: &str| events.iter().position(|e| e.contains(needle));
        let run_started = tag(r#""type":"runStarted""#).expect("a RunStarted event");
        let item_started = tag(r#""type":"itemStarted""#).expect("an ItemStarted event");
        let item_finished = tag(r#""type":"itemFinished""#).expect("an ItemFinished event");
        let batch_progress = tag(r#""type":"batchProgress""#).expect("a BatchProgress event");
        let run_finished = tag(r#""type":"runFinished""#).expect("a RunFinished event");
        assert_eq!(run_started, 0, "§0.4.2: RunStarted is the FIRST event");
        assert_eq!(
            run_finished,
            events.len() - 1,
            "§0.4.2: RunFinished is the terminal (LAST) event"
        );
        assert!(
            run_started < item_started && item_started < item_finished,
            "§1.9: per-item ItemStarted precedes ItemFinished, both after RunStarted"
        );
        assert!(
            item_finished <= batch_progress && batch_progress < run_finished,
            "§1.11: BatchProgress follows the item's finish, before the terminal RunFinished"
        );
    }

    // §6.4.1 (G15) / §1.9 / §1.12: a pre-flight-skipped item never enters the queue (excluded from
    // `total_items`) and emits NO live ItemStarted/ItemFinished, but IS projected into `RunResult.items` +
    // `Totals.skipped` (§0.4.2 pre-flight-skip emission policy; §1.12 "pre-flight skips ARE in RunResult.items").
    #[tokio::test]
    async fn projects_a_preflight_skip_into_the_summary_without_live_events() {
        let Some(src_dir) = non_ephemeral_source_dir() else {
            return; // the crate root is itself ephemeral — no realistic non-ephemeral source dir to test.
        };
        let d = deps();
        let (dropped, paths, identity) = eligible(src_dir.path(), "data.csv", 0, b"a,b\n1,2\n");
        let set = registered(
            src_dir.path(),
            vec![(dropped, paths, identity)],
            vec![skipped(1, SkipReason::UnsupportedType)],
        );
        let (channel, events) = capture_channel();

        let run_id = run(&d, &set, RerunDecision::FreshCopy, src_dir.path(), &channel).await;

        // total_items counts ONLY the queued eligible item (the skip is excluded).
        let events = events.lock().expect("event sink lock").clone();
        let run_started = events
            .iter()
            .find(|e| e.contains(r#""type":"runStarted""#))
            .expect("a RunStarted event");
        assert!(
            run_started.contains(r#""totalItems":1"#),
            "§0.4.2: total_items counts only the queued eligible item, not the pre-flight skip"
        );
        // Exactly ONE live ItemFinished (the eligible item) — the skip emits none.
        let finishes = events
            .iter()
            .filter(|e| e.contains(r#""type":"itemFinished""#))
            .count();
        assert_eq!(
            finishes, 1,
            "§0.4.2: the pre-flight skip emits NO live ItemFinished — only the eligible item does"
        );
        // But the summary carries BOTH — the eligible Succeeded + the skip, counted in Totals.skipped.
        let result = d.results.get(run_id).expect("the RunResult is retained");
        assert_eq!(
            result.totals,
            Totals {
                succeeded: 1,
                failed: 0,
                cancelled: 0,
                skipped: 1,
            }
        );
        assert_eq!(
            result.items.len(),
            2,
            "§1.12: the summary carries the eligible item AND the pre-flight skip"
        );
        assert!(
            result
                .items
                .iter()
                .any(|item| matches!(item.state, JobState::Skipped(SkipReason::UnsupportedType))),
            "§1.12: the pre-flight skip is projected with its SkipReason"
        );
    }

    // §6.4.1 (G15) / §2.5: the RerunDecision applier (the P3.48 rerun-skip ruling `034a451`) — under `Skip`, a
    // SEEN (ledgered) equivalent item is assigned `Skipped(AlreadyConverted)`: no output, excluded from the
    // queued `total_items`, no live events, BUT projected into the §1.12 summary as a distinct `Skipped` outcome
    // (`Totals.skipped`, the direct §2.8.2 line) — NOT dropped. Under `FreshCopy`, the SAME seen item converts.
    // [Build-Session-Entscheidung: P3.48 — the applier arm over the ruling's `AlreadyConverted` variant]
    #[tokio::test]
    async fn rerun_skip_marks_a_seen_item_already_converted_but_fresh_copy_converts_it() {
        let Some(src_dir) = non_ephemeral_source_dir() else {
            return; // the crate root is itself ephemeral — no realistic non-ephemeral source dir to test.
        };
        let d = deps();
        let (dropped, paths, identity) = eligible(src_dir.path(), "data.csv", 0, b"a,b\n1,2\n");
        let item = dropped.item;
        let set = registered(src_dir.path(), vec![(dropped, paths, identity)], Vec::new());

        // Pre-seed the §2.5.2 ledger with THIS item's §2.5.1 key (a prior in-session run of the same pair).
        let key = d.equiv.compute_equiv_key(
            &set.identities[&item],
            tsv_target().id,
            &OptionValues(BTreeMap::new()),
        );
        d.ledger.record(key);

        // Under Skip, the seen item is `Skipped(AlreadyConverted)` — no output, total_items 0, no live events,
        // but IN the summary as a distinct skip (§1.12 "never a failure").
        let (skip_channel, skip_events) = capture_channel();
        let skip_run = run(&d, &set, RerunDecision::Skip, src_dir.path(), &skip_channel).await;
        assert!(
            !src_dir.path().join("data.tsv").exists(),
            "§2.5: a Skip re-run produces no new output for the equivalent item"
        );
        let skip_events = skip_events.lock().expect("lock").clone();
        assert!(
            skip_events
                .iter()
                .find(|e| e.contains(r#""type":"runStarted""#))
                .expect("RunStarted")
                .contains(r#""totalItems":0"#),
            "§2.5: the AlreadyConverted item is excluded from the queued count (never dispatched)"
        );
        assert!(
            !skip_events
                .iter()
                .any(|e| e.contains(r#""type":"itemFinished""#)),
            "§0.4.2: a re-run skip emits NO live ItemFinished (terminal at construction)"
        );
        let skip_result = d.results.get(skip_run).expect("the run retains a summary");
        assert_eq!(
            skip_result.totals,
            Totals {
                succeeded: 0,
                failed: 0,
                cancelled: 0,
                skipped: 1,
            },
            "§1.12: the re-run skip is counted in Totals.skipped, never dropped, never failed"
        );
        assert_eq!(
            skip_result.items.len(),
            1,
            "§1.12: the item IS in the summary"
        );
        assert!(
            matches!(
                skip_result.items[0].state,
                JobState::Skipped(SkipReason::AlreadyConverted)
            ),
            "§1.9: the re-run skip is state Skipped(AlreadyConverted)"
        );
        assert!(
            matches!(
                &skip_result.items[0].reason,
                Some(OutcomeMsg::Skipped { reason: SkipReason::AlreadyConverted, text })
                    if text.contains("already converted")
            ),
            "§2.8.2: the reason is the direct AlreadyConverted skip line"
        );

        // Under FreshCopy, the SAME seen item converts (a fresh numbered copy, §2.5).
        let (fresh_channel, _fresh_events) = capture_channel();
        let fresh_run = run(
            &d,
            &set,
            RerunDecision::FreshCopy,
            src_dir.path(),
            &fresh_channel,
        )
        .await;
        assert!(
            src_dir.path().join("data.tsv").exists(),
            "§2.5: FreshCopy re-produces the output for the equivalent item"
        );
        assert_eq!(
            d.results
                .get(fresh_run)
                .expect("the FreshCopy run summary")
                .totals
                .succeeded,
            1,
            "§2.5: FreshCopy converts the seen item"
        );
    }

    // §6.4.1 (G15) / §1.9: a source that is gone/unreadable when its turn comes fails THAT item cleanly (the
    // batch continues), projected `Failed` in the summary with a live `ItemFinished{Failed}` — no output.
    #[tokio::test]
    async fn a_missing_source_fails_the_item_cleanly() {
        let Some(src_dir) = non_ephemeral_source_dir() else {
            return; // the crate root is itself ephemeral — no realistic non-ephemeral source dir to test.
        };
        let d = deps();
        // Build the eligible item, then DELETE its source so the engine cannot read it at convert time.
        let (dropped, paths, identity) = eligible(src_dir.path(), "data.csv", 0, b"a,b\n1,2\n");
        let source = paths.resolved_path.clone();
        let set = registered(src_dir.path(), vec![(dropped, paths, identity)], Vec::new());
        std::fs::remove_file(&source).expect("remove the source before the run (it went away)");
        let (channel, events) = capture_channel();

        let run_id = run(&d, &set, RerunDecision::FreshCopy, src_dir.path(), &channel).await;

        assert!(
            !src_dir.path().join("data.tsv").exists(),
            "no output for a failed item"
        );
        let result = d.results.get(run_id).expect("the RunResult is retained");
        assert_eq!(
            result.totals.failed, 1,
            "§1.9: the unreadable source fails THAT item (the batch continues)"
        );
        assert_eq!(result.totals.succeeded, 0);
        assert!(
            matches!(result.items[0].state, JobState::Failed(_)),
            "§1.12: the failed item is projected with a §2.8 kind"
        );
        let events = events.lock().expect("lock").clone();
        assert!(
            events
                .iter()
                .any(|e| e.contains(r#""type":"itemFinished""#) && e.contains(r#""failed""#)),
            "§0.4.2: a live ItemFinished carries the Failed outcome"
        );
    }

    // §6.4.1 (G15) / §1.7 / §2.1: a run whose token is already tripped cancels the in-flight item at the
    // native lane's first chunk-boundary poll (§1.7 cooperative cancel). The §2.1 atomic publish NEVER runs on
    // the cancel path, so NO output survives beside the source (the partial temp drops), the source is
    // UNTOUCHED (no-harm G32(a)), and the §1.12 summary counts the item `Cancelled` (never a clean success),
    // retained for the C8 re-serve with a live `ItemFinished{Cancelled}`.
    #[tokio::test]
    async fn a_cancelled_run_publishes_no_output_leaves_the_source_intact_and_projects_cancelled() {
        let Some(src_dir) = non_ephemeral_source_dir() else {
            return; // the crate root is itself ephemeral — no realistic non-ephemeral source dir to test.
        };
        let d = deps();
        // A MULTI-chunk source (> 3×100 KB `PROGRESS_CHUNK_BYTES`) so the transform reaches a chunk boundary
        // where the cancel poll fires (a tiny source could complete in one pass before the first poll).
        let mut source_bytes = Vec::new();
        while source_bytes.len() < 300 * 1024 {
            source_bytes.extend_from_slice(b"a,b,c\n");
        }
        let (dropped, paths, identity) = eligible(src_dir.path(), "big.csv", 0, &source_bytes);
        let source = paths.resolved_path.clone();
        let set = registered(src_dir.path(), vec![(dropped, paths, identity)], Vec::new());
        let (channel, events) = capture_channel();

        // A PRE-cancelled run token: the native lane's first chunk-boundary poll observes it → Cancelled.
        let token = CancellationToken::new();
        token.cancel();
        let run_id = run_with_token(
            &d,
            &set,
            RerunDecision::FreshCopy,
            src_dir.path(),
            &channel,
            token,
            ResolvedDestination::BesideSource,
        )
        .await;

        // §2.1: the atomic publish never runs on the cancel path — no output beside the source.
        assert!(
            !src_dir.path().join("big.tsv").exists(),
            "§2.1/§1.7: a cancelled item publishes NO output (the partial temp is dropped, never promoted)"
        );
        // No-harm (G32(a)): the source is read-only to the conductor and survives byte-for-byte.
        assert_eq!(
            std::fs::read(&source).expect("the source is untouched by a cancelled run"),
            source_bytes,
            "§2.0 no-harm: a cancelled run never mutates the source"
        );
        // §1.12: the item is projected `Cancelled`, counted in `Totals.cancelled`, never succeeded/failed.
        let result = d
            .results
            .get(run_id)
            .expect("the RunResult is retained for C8 re-serve");
        assert_eq!(
            result.totals,
            Totals {
                succeeded: 0,
                failed: 0,
                cancelled: 1,
                skipped: 0,
            },
            "§1.12: the cancelled item is counted in Totals.cancelled, never a clean success"
        );
        assert!(
            matches!(result.items[0].state, JobState::Cancelled),
            "§1.9: the cancelled item's terminal state is Cancelled"
        );
        assert!(
            result.items[0].output_display.is_none(),
            "§1.12: a cancelled item names no output path"
        );
        // §0.4.2: unlike a pre-flight skip, a cancelled item DID dispatch, so it emits a live terminal event.
        let events = events.lock().expect("lock").clone();
        assert!(
            events
                .iter()
                .any(|e| e.contains(r#""type":"itemFinished""#) && e.contains(r#""cancelled""#)),
            "§0.4.2: a live ItemFinished carries the Cancelled outcome"
        );
    }

    // §6.4.3 (G31/G32(a)) / §2.7.1: a `ChosenRoot` run RE-CREATES the source's relative subtree UNDER the
    // chosen root (a source at `<src>/nested/data.csv` publishes to `<dest>/nested/data.tsv`), never beside the
    // source. Regression guard for the P3.48 G1-review fix: the conductor must strip the source path against the
    // SOURCE freeze common root, NOT the open-folder root (the chosen root D) — feeding D would fail
    // `strip_prefix` (a source is never under the destination) → `WriteFailed` for every ChosenRoot item.
    #[tokio::test]
    async fn a_chosen_root_run_recreates_the_source_subtree_under_the_chosen_root() {
        let Some(src_dir) = non_ephemeral_source_dir() else {
            return; // the crate root is itself ephemeral — no realistic non-ephemeral source dir to test.
        };
        let Some(dest_dir) = non_ephemeral_source_dir() else {
            return;
        };
        let d = deps();
        // A source NESTED under the freeze root: `<src>/nested/data.csv` (the freeze root is `<src>`).
        std::fs::create_dir(src_dir.path().join("nested"))
            .expect("create the nested source subdir");
        let (dropped, paths, identity) =
            eligible(src_dir.path(), "nested/data.csv", 0, b"a,b\n1,2\n");
        let set = registered(src_dir.path(), vec![(dropped, paths, identity)], Vec::new());
        let (channel, _events) = capture_channel();

        // Run with a CHOSEN-ROOT destination (the chosen root D = `<dest>`), fresh token.
        let run_id = run_with_token(
            &d,
            &set,
            RerunDecision::FreshCopy,
            dest_dir.path(),
            &channel,
            CancellationToken::new(),
            ResolvedDestination::ChosenRoot(dest_dir.path().to_path_buf()),
        )
        .await;

        // §2.7.1: the output re-creates the source subtree UNDER the chosen root → `<dest>/nested/data.tsv`.
        let output = dest_dir.path().join("nested").join("data.tsv");
        let out = std::fs::read(&output).expect(
            "§2.7.1: the ChosenRoot output re-creates the source subtree under the chosen root",
        );
        assert_eq!(
            parse_rows(&out, '\t'),
            vec![
                vec!["a".to_owned(), "b".to_owned()],
                vec!["1".to_owned(), "2".to_owned()],
            ],
            "output validity (G31): the ChosenRoot TSV is a valid tab-delimited read-back"
        );
        // No-harm + no beside-source output on a ChosenRoot run.
        assert!(
            src_dir.path().join("nested").join("data.csv").is_file(),
            "no-harm: the source is untouched"
        );
        assert!(
            !src_dir.path().join("nested").join("data.tsv").exists(),
            "§2.7.1: a ChosenRoot run publishes under the chosen root, never beside the source"
        );
        let result = d.results.get(run_id).expect("the RunResult is retained");
        assert_eq!(
            result.totals.succeeded, 1,
            "§1.12: the ChosenRoot item succeeded (not WriteFailed on a mis-fed strip base)"
        );
    }
}

#[cfg(test)]
mod equiv_key_tests {
    //! §6.4.1 unit + §6.4.2 property (G15/G16) for the P3.39 §2.5.1 EquivKey computation and its end-to-end
    //! firing through the `crate::run` §2.5.2 ledger. The COMPUTE half lives here (folding a `FileIdentity` +
    //! `TargetId` + `OptionValues`); the STORAGE half is `crate::run::RerunLedger` (unit-tested there). We
    //! pin the load-bearing §2.5.2 invariant — two computes of the same job AGREE (a held, seed-stable
    //! `BuildHasher`, never a fresh `RandomState` per call) — as a pinned-seed property, plus the §2.5.1
    //! folding facts (identity-not-path; target/settings/source sensitivity; the `Op` cross-cat target), and
    //! the end-to-end "second identical drop this session fires; a changed target does not".
    use super::*;
    use crate::domain::{CrossCatOp, OptionKey, OptionValue};
    use proptest::prelude::*;
    use proptest::test_runner::{RngAlgorithm, TestRng, TestRunner};

    /// A `FileIdentity` with a chosen `(dev, inode)` identity and path — the identity `Hash` covers only
    /// `(dev, inode)` (§2.5.1 "identity, not path"), so the path is free to vary independently in the tests.
    fn identity(dev: u64, inode: u64, path: &str) -> FileIdentity {
        FileIdentity {
            canonical_path: PathBuf::from(path),
            dev_or_volserial: dev,
            inode_or_fileindex: inode,
        }
    }

    /// An `OptionValues` from `(key, int)` pairs — the effective (fully-defaulted) settings the §2.5.1 fold
    /// canonicalises via the inner `BTreeMap`'s sorted-key iteration.
    fn settings(pairs: &[(&str, i64)]) -> OptionValues {
        OptionValues(
            pairs
                .iter()
                .map(|(k, v)| (OptionKey((*k).to_string()), OptionValue::Int(*v)))
                .collect(),
        )
    }

    /// A PINNED-SEED runner (test-strategy §1.3 / G16): drive a `TestRunner` with a `deterministic_rng` so
    /// the determinism property reproduces exactly and is never retried-to-pass (§7). Module-private, like
    /// the sibling `freeze_tests` runner. [Build-Session-Entscheidung: P3.39]
    fn pinned_runner() -> TestRunner {
        TestRunner::new_with_rng(
            ProptestConfig::with_cases(512),
            TestRng::deterministic_rng(RngAlgorithm::ChaCha),
        )
    }

    #[test]
    fn same_inputs_yield_the_same_key() {
        // §2.5.2 (the load-bearing invariant): two computes of the same (source, target, settings) through
        // the SAME held BuildHasher agree — the premise the whole re-run signal rests on.
        let computer = EquivKeyComputer::default();
        let id = identity(1, 2, "/vol/a.csv");
        let target = TargetId::Format(UserFacingFormat::Webp);
        let opts = settings(&[("quality", 80), ("lossless", 0)]);
        assert_eq!(
            computer.compute_equiv_key(&id, target, &opts),
            computer.compute_equiv_key(&id, target, &opts),
            "§2.5.2: identical inputs fold to the same EquivKey (held seed-stable hasher)"
        );
    }

    #[test]
    fn settings_are_order_independent() {
        // §2.5.1: the effective settings canonicalise order-independently (the BTreeMap's sorted-key form),
        // so the same key/value set supplied in a different order yields the same EquivKey.
        let computer = EquivKeyComputer::default();
        let id = identity(1, 2, "/vol/a.csv");
        let target = TargetId::Format(UserFacingFormat::Webp);
        let forward = settings(&[("aaa", 1), ("bbb", 2), ("ccc", 3)]);
        let shuffled = settings(&[("ccc", 3), ("aaa", 1), ("bbb", 2)]);
        assert_eq!(
            computer.compute_equiv_key(&id, target, &forward),
            computer.compute_equiv_key(&id, target, &shuffled),
            "§2.5.1: settings canonicalise order-independently (sorted-key BTreeMap)"
        );
    }

    #[test]
    fn source_identity_ignores_the_path() {
        // §2.5.1: source IDENTITY (not path) keys the fold — the same (dev, inode) reached via a different
        // path still matches (FileIdentity's Hash covers only (dev, inode), §2.3.1/§2.3.4).
        let computer = EquivKeyComputer::default();
        let target = TargetId::Format(UserFacingFormat::Webp);
        let opts = settings(&[("quality", 80)]);
        let via_a = identity(7, 9, "/vol/one/name.csv");
        let via_b = identity(7, 9, "/vol/other/hardlink.csv");
        assert_eq!(
            computer.compute_equiv_key(&via_a, target, &opts),
            computer.compute_equiv_key(&via_b, target, &opts),
            "§2.5.1: a re-run reached via a different but same-file path still folds to the same key"
        );
    }

    #[test]
    fn a_different_target_folds_to_a_different_key() {
        // §2.5.1: changing the target is a NEW conversion — the target component folds into the key.
        let computer = EquivKeyComputer::default();
        let id = identity(1, 2, "/vol/a.csv");
        let opts = settings(&[("quality", 80)]);
        assert_ne!(
            computer.compute_equiv_key(&id, TargetId::Format(UserFacingFormat::Webp), &opts),
            computer.compute_equiv_key(&id, TargetId::Format(UserFacingFormat::Png), &opts),
            "§2.5.1: a different target format is a different EquivKey"
        );
    }

    #[test]
    fn a_cross_category_op_target_folds() {
        // §2.5.1: the `TargetId::Op(CrossCatOp)` arm folds too (covers CrossCatOp's Hash) — ToGif vs
        // ExtractAudio are distinct conversions of the same source.
        let computer = EquivKeyComputer::default();
        let id = identity(1, 2, "/vol/clip.mp4");
        let opts = settings(&[]);
        assert_ne!(
            computer.compute_equiv_key(&id, TargetId::Op(CrossCatOp::ToGif), &opts),
            computer.compute_equiv_key(&id, TargetId::Op(CrossCatOp::ExtractAudio), &opts),
            "§2.5.1: distinct cross-category ops fold to distinct EquivKeys"
        );
    }

    #[test]
    fn a_different_setting_value_folds_to_a_different_key() {
        // §2.5.1: changing an effective setting is a NEW conversion — settings fold into the key.
        let computer = EquivKeyComputer::default();
        let id = identity(1, 2, "/vol/a.csv");
        let target = TargetId::Format(UserFacingFormat::Webp);
        assert_ne!(
            computer.compute_equiv_key(&id, target, &settings(&[("quality", 80)])),
            computer.compute_equiv_key(&id, target, &settings(&[("quality", 90)])),
            "§2.5.1: a different effective setting is a different EquivKey"
        );
    }

    #[test]
    fn a_different_source_folds_to_a_different_key() {
        // §2.5.1: a different source identity ((dev, inode)) is a different conversion.
        let computer = EquivKeyComputer::default();
        let target = TargetId::Format(UserFacingFormat::Webp);
        let opts = settings(&[("quality", 80)]);
        assert_ne!(
            computer.compute_equiv_key(&identity(1, 2, "/vol/a.csv"), target, &opts),
            computer.compute_equiv_key(&identity(1, 3, "/vol/b.csv"), target, &opts),
            "§2.5.1: a different source identity is a different EquivKey"
        );
    }

    #[test]
    fn compute_then_ledger_fires_on_the_second_identical_drop() {
        // §2.5.2 end-to-end (compute + storage): compute a conversion's key, record it, then re-compute the
        // SAME conversion — the ledger has seen it (the second identical drop this session fires the prompt).
        // A DIFFERENT target folds to a key the ledger has NOT seen (a changed conversion never falsely fires).
        let computer = EquivKeyComputer::default();
        let ledger = RerunLedger::default();
        let id = identity(1, 2, "/vol/a.csv");
        let opts = settings(&[("quality", 80)]);

        let first =
            computer.compute_equiv_key(&id, TargetId::Format(UserFacingFormat::Webp), &opts);
        ledger.record(first);

        let again =
            computer.compute_equiv_key(&id, TargetId::Format(UserFacingFormat::Webp), &opts);
        assert!(
            ledger.has_seen(again),
            "§2.5.2: the re-computed key of an identical conversion hits the in-session ledger"
        );

        let changed =
            computer.compute_equiv_key(&id, TargetId::Format(UserFacingFormat::Png), &opts);
        assert!(
            !ledger.has_seen(changed),
            "§2.5.2/§2.5.1: a changed target folds to an unseen key — a different conversion does not fire"
        );
    }

    #[test]
    fn prop_compute_is_deterministic() {
        // §2.5.2 property (G16): for ANY (source identity, target, effective settings), two computes through
        // one held hasher agree — the pinned-seed guard against a reseeding-per-call regression (§2.5.2 warns
        // "never a fresh RandomState per call").
        let computer = EquivKeyComputer::default();
        pinned_runner()
            .run(
                &(
                    any::<u64>(),
                    any::<u64>(),
                    0u8..4,
                    prop::collection::vec((0u8..8, any::<i64>()), 0..6),
                ),
                |(dev, inode, target_choice, opt_pairs)| {
                    let id = identity(dev, inode, "/vol/prop.dat");
                    let target = match target_choice % 4 {
                        0 => TargetId::Format(UserFacingFormat::Webp),
                        1 => TargetId::Format(UserFacingFormat::Png),
                        2 => TargetId::Op(CrossCatOp::ToGif),
                        _ => TargetId::Op(CrossCatOp::ExtractAudio),
                    };
                    let opts = OptionValues(
                        opt_pairs
                            .iter()
                            .map(|(k, v)| (OptionKey(format!("k{k}")), OptionValue::Int(*v)))
                            .collect(),
                    );
                    prop_assert_eq!(
                        computer.compute_equiv_key(&id, target, &opts),
                        computer.compute_equiv_key(&id, target, &opts),
                        "§2.5.2: identical inputs always fold to the same EquivKey"
                    );
                    Ok(())
                },
            )
            .expect("the determinism property holds for every pinned-seed case");
    }
}

#[cfg(test)]
mod rerun_verdict_tests {
    //! §6.4.1 unit (G15) for the P3.40 §2.5 batch re-run verdict `compute_rerun_verdict` — the
    //! `OutputPlanPreview.rerun` computation over the freeze-retained identities + the P3.39
    //! `EquivKeyComputer`/`RerunLedger`. Pins: the prompt fires (`Some`) iff an ELIGIBLE item's retained
    //! identity folds to a key the in-session ledger has seen; the §2.5.3 fallback (an unseen item → not
    //! counted → no prompt → §2.2 numbering); only eligible items count (a skipped item's identity never
    //! fires); and a missing identity is not-equivalent (a re-run is asserted only on positive evidence).
    use super::*;
    use crate::domain::{Confidence, InstanceId};

    /// A `FileIdentity` with a chosen `(dev, inode)` — the §2.5.1 EquivKey source component.
    fn identity(dev: u64, inode: u64) -> FileIdentity {
        FileIdentity {
            canonical_path: PathBuf::from(format!("/vol/{dev}-{inode}.csv")),
            dev_or_volserial: dev,
            inode_or_fileindex: inode,
        }
    }

    /// An eligible `DroppedItem` at `id` — a recognized CSV (the `detected` verdict is irrelevant to the
    /// verdict, which keys on the id + the identities table).
    fn eligible_item(id: u32) -> DroppedItem {
        DroppedItem {
            item: ItemId::from_index(id),
            display_name: format!("f{id}.csv"),
            rel_path_display: None,
            size_bytes: 1,
            detected: DetectionOutcome::Recognized {
                format: UserFacingFormat::Csv,
                confidence: Confidence::High,
                dims: None,
            },
        }
    }

    fn set_id() -> CollectedSetId {
        serde_json::from_str(r#""77777777-7777-4777-8777-777777777777""#)
            .expect("CollectedSetId deserializes from a uuid string")
    }
    fn instance() -> InstanceId {
        serde_json::from_str(r#""88888888-8888-4888-8888-888888888888""#)
            .expect("InstanceId deserializes from a uuid string")
    }

    /// A `RegisteredSet` with the given eligible `items` + `identities` table (the verdict iterates only the
    /// eligible `items`, so no skipped members are needed to drive it).
    fn registered(
        items: Vec<DroppedItem>,
        identities: BTreeMap<ItemId, FileIdentity>,
    ) -> RegisteredSet {
        let count = items.len();
        RegisteredSet {
            frozen: FrozenCollectedSet {
                id: set_id(),
                instance: instance(),
                format: UserFacingFormat::Csv,
                items,
                count,
                skipped: vec![],
                total_bytes: 0,
                roots: vec![],
                encoding_hint: None,
                delimiter_hint: None,
                notes: vec![],
                item_paths: BTreeMap::new(),
            },
            identities,
        }
    }

    fn webp() -> TargetId {
        TargetId::Format(UserFacingFormat::Webp)
    }
    fn no_settings() -> OptionValues {
        OptionValues(BTreeMap::new())
    }

    #[test]
    fn verdict_fires_on_a_recorded_eligible_item() {
        let computer = EquivKeyComputer::default();
        let ledger = RerunLedger::default();
        let id = identity(1, 2);
        let set = registered(
            vec![eligible_item(0)],
            BTreeMap::from([(ItemId::from_index(0), id.clone())]),
        );
        // Record this exact conversion this session.
        ledger.record(computer.compute_equiv_key(&id, webp(), &no_settings()));
        assert_eq!(
            compute_rerun_verdict(&set, webp(), &no_settings(), &computer, &ledger),
            Some(RerunPrompt {
                equivalent_count: 1
            }),
            "§2.5.2: the eligible item's retained identity folds to a seen key → the batch prompt fires"
        );
    }

    #[test]
    fn verdict_is_none_on_a_fresh_ledger() {
        let computer = EquivKeyComputer::default();
        let ledger = RerunLedger::default();
        let id = identity(1, 2);
        let set = registered(
            vec![eligible_item(0)],
            BTreeMap::from([(ItemId::from_index(0), id)]),
        );
        assert_eq!(
            compute_rerun_verdict(&set, webp(), &no_settings(), &computer, &ledger),
            None,
            "§2.5.3: a fresh session (empty ledger) determines no equivalence → no prompt → §2.2 numbering"
        );
    }

    #[test]
    fn verdict_counts_only_the_equivalent_items() {
        let computer = EquivKeyComputer::default();
        let ledger = RerunLedger::default();
        let a = identity(1, 2);
        let b = identity(3, 4);
        let set = registered(
            vec![eligible_item(0), eligible_item(1)],
            BTreeMap::from([
                (ItemId::from_index(0), a.clone()),
                (ItemId::from_index(1), b),
            ]),
        );
        // Record only item 0's key.
        ledger.record(computer.compute_equiv_key(&a, webp(), &no_settings()));
        assert_eq!(
            compute_rerun_verdict(&set, webp(), &no_settings(), &computer, &ledger),
            Some(RerunPrompt {
                equivalent_count: 1
            }),
            "§2.5.2: exactly the one recorded eligible item is counted equivalent (the other is unseen)"
        );
    }

    #[test]
    fn verdict_ignores_a_skipped_items_identity() {
        // The identities table also carries detect-ineligible skips (§0.6 inv-6), but the verdict iterates only
        // the ELIGIBLE `items` — a skipped item is not converted, so it can never be a re-run.
        let computer = EquivKeyComputer::default();
        let ledger = RerunLedger::default();
        let eligible = identity(1, 2);
        let skipped = identity(3, 4);
        let set = registered(
            vec![eligible_item(0)],
            // id 0 = the eligible member (NOT recorded); id 1 = a skipped identity (recorded), NOT in `items`.
            BTreeMap::from([
                (ItemId::from_index(0), eligible),
                (ItemId::from_index(1), skipped.clone()),
            ]),
        );
        ledger.record(computer.compute_equiv_key(&skipped, webp(), &no_settings()));
        assert_eq!(
            compute_rerun_verdict(&set, webp(), &no_settings(), &computer, &ledger),
            None,
            "§2.5: a recorded SKIPPED-item identity never fires the verdict — only eligible items are candidates"
        );
    }

    #[test]
    fn verdict_treats_a_missing_identity_as_not_equivalent() {
        // Defensive: an eligible item with no retained identity is treated as not-equivalent — a re-run is
        // asserted only on positive evidence (a resolved survivor always HAS one; this guards the lookup).
        let computer = EquivKeyComputer::default();
        let ledger = RerunLedger::default();
        let set = registered(vec![eligible_item(0)], BTreeMap::new());
        // Even a recorded key for some identity cannot fire an item that has no identity to fold.
        ledger.record(computer.compute_equiv_key(&identity(1, 2), webp(), &no_settings()));
        assert_eq!(
            compute_rerun_verdict(&set, webp(), &no_settings(), &computer, &ledger),
            None,
            "§2.5: an eligible item with no retained identity is not-equivalent (evidence-only firing)"
        );
    }
}
