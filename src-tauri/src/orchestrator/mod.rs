//! `crate::orchestrator` — the §1.9 batch / job-lifecycle conductor: it builds the queue, drives
//! `JobState`, holds the run registry + cancellation tokens (§0.4.4), and fans progress out to the
//! Channel. It sequences the guarantees / engines / detection layers; it owns none of their behaviour.
//!
//! The conducting BEHAVIOUR (queue construction at C6, the §1.9 transitions, the run-registry WIRING +
//! the cancellation flow) is filled by P3.46. This module homes the §0.6 outcome-referencing lifecycle/result types
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
//! run-cancellation-token store, P2.42; its register-at-C6 / cancel-at-C7 / drop-on-`RunFinished` WIRING is the
//! P3.46 conductor), its sibling the `RunResultStore` (the process-local terminal-`RunResult` retention for C8
//! re-serve, P2.43; no on-disk persistence per §7.4, its retain-at-`RunFinished` / evict-at-C6 / get-at-C8
//! WIRING likewise P3.46), the `CollectedSetRegistry` (the `CollectedSetId` → `FrozenCollectedSet` resolve
//! store, P2.44; so the bare-`collectedSetId` C3/C4/C5/C6 commands resolve the frozen detection result without
//! a second walk, its register-at-C1/C2a-freeze / resolve-at-C3/C4/C5 / take-at-C6 WIRING likewise P3.46), and
//! the `IngestRegistry` (the `CollectingId` → `CancellationToken` ingest-cancellation store, P2.45; the
//! one-phase-earlier sibling of `RunRegistry`, so C13 `cancel_ingest` can trip an in-flight C1/C2a walk — its
//! register-at-handler-entry / cancel-at-C13 / release-on-every-handler-exit WIRING is C1/C2a/C13 + P2.69-71).

// [Build-Session-Entscheidung: P2.10/P2.11/P2.12] dead_code expect — the lifecycle/DTO/result types homed
// here (Batch/ConversionJob/JobState, the C4/C5 DTOs, and the §1.12 RunResult/ItemResult/Totals/
// CleanupResidue/ItemOutcome) are authored as CONTRACTS before their consumers exist: the orchestrator
// queue/lifecycle BEHAVIOUR that constructs and drives them is P3.46, the DTO/result wire registration rides
// the C4/C5/C8 + RunFinished/ItemFinished consumers (later P2/P3 boxes). So each is dead in the PRODUCTION
// build until consumed; the cfg(test) tests below construct the full graphs, so the TEST build is
// dead-code-clean and needs no expectation. `expect` (not `allow`) auto-flags the moment the conductor
// consumes them — matching `crate::domain` / `crate::outcome`. Scoped to `not(test)` for that same reason.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the §0.6 lifecycle/DTO/result types homed here (Batch/ConversionJob/JobState P2.10, the C4/C5 DTOs P2.11, RunResult/ItemResult/Totals/CleanupResidue/ItemOutcome P2.12, the §0.4.2 ConversionEvent enum + its RunStarted/ItemStarted/ItemProgress/ItemFinished/BatchProgress payloads P2.37, and the four §0.4.4 State stores RunRegistry P2.42 + RunResultStore P2.43 + CollectedSetRegistry P2.44 + IngestRegistry P2.45) are authored as contracts before the P3.46 orchestrator behaviour + the C1/C2a/C3/C4/C5/C6/C7/C8/C13 + the C6 onProgress Channel<ConversionEvent> (P2.29) wire consumers construct/register/drive them, so their as-yet-unwired methods are dead in the production build until consumed (`RunRegistry::has_active_run` is already consumed by the §7.1.1 `converter_is_busy` from P2.55, with `register`/`cancel`/`finish` staying dead until P3.46)."
    )
)]

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use specta::Type;
use tokio_util::sync::CancellationToken;
use walkdir::WalkDir;

use crate::domain::{
    CollectedSet, CollectedSetId, CollectingId, DestinationChoice, DivertReason, DroppedItem,
    FrozenCollectedSet, IntakeOrigin, ItemId, ItemIdSpace, ItemSpaceExhausted, JobStage,
    OptionValues, OutputPlan, ReadFailure, RerunPrompt, RunId, SkipReason, Target, TargetId,
    UserFacingFormat,
};
use crate::fs_guard::FileIdentity;
use crate::outcome::{ConversionErrorKind, IpcError, OutcomeMsg};

/// One same-source conversion batch (§0.6 / §1.9) — the queue the orchestrator builds at C6
/// `start_conversion` from a frozen `CollectedSet::Single` and drives to a §1.12 summary. INTERNAL to the
/// pipeline: it is assembled and consumed core-side (the WebView sees the §1.12 `RunResult` projection,
/// never the `Batch` itself), so it is NOT a wire type and derives no `serde`/`specta` (mirroring the
/// P2.9 internal `OutputPlan`). The §0.6 invariants it carries BY SHAPE: exactly one whole-batch `target`
/// and one effective `options` (invariants 1+2 — single values, not per-item); a `Batch` exists only from
/// a `CollectedSet::Single` (invariant 3). The per-item enforcement (count == items.len(), frozen set,
/// `item == source.item`, stable `ItemId`) is property-tested in P2.14.
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
    /// Where the batch's outputs are written (§2.7) — beside-source or a chosen root.
    pub destination: DestinationChoice,
    /// The per-item jobs, in the deterministic collected/traversal order (§1.9 queue order). Carries BOTH
    /// the `Pending` eligible jobs AND the pre-flight `Skipped` jobs materialised at construction (§1.9),
    /// over the §0.6 single id space (so a `SkippedItem.item` never collides with an eligible `ItemId`).
    pub jobs: Vec<ConversionJob>,
}

/// One per-item conversion job within a `Batch` (§0.6 / §1.9). INTERNAL (not a wire type — the same
/// rationale as `Batch`).
///
/// [Build-Session-Entscheidung: P2.10] `Debug, Clone, PartialEq, Eq`; NOT `Copy` (its `source:
/// DroppedItem` owns `PathBuf`s). `Eq` holds — every field type is `Eq` (`OutputPlan` derives `Eq`, P2.9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionJob {
    /// The job's top-level key — the item's `ItemId`, DENORMALIZED from `source.item` for cheap
    /// addressing in the §1.9 lifecycle + the per-item progress/finished events without unwrapping
    /// `source` (§0.6; the same duplicate-for-cheap-access pattern as `count` beside `items.len()`).
    /// INVARIANT (§0.6): `item == source.item`, property-tested in P2.14.
    pub item: ItemId,
    /// The eligible source item this job converts (§0.6) — carries its frozen resolved path + detection.
    pub source: DroppedItem,
    /// The §1.9 lifecycle state — §1.9 owns the TRANSITIONS; this stores the current state.
    pub state: JobState,
    /// The §1.8-computed output plan, set before the write — `None` until §1.8 plans it (and for a
    /// pre-flight `Skipped` job, which never plans an output).
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
    /// maps the internal kind to the wire kind via `ErrorKind::from` in `crate::run` (§2.8.2).
    Failed(ConversionErrorKind),
    /// A detection-ineligible pre-flight item (§1.2/§1.3) — set at `Batch` construction, never queued,
    /// terminal (§1.9). Carries the §0.6 `SkipReason` copied directly from the `SkippedItem`.
    Skipped(SkipReason),
    /// User cancel; nothing written for it (§1.7/§1.11).
    Cancelled,
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
/// [Build-Session-Entscheidung: P2.11] `Serialize` + `Type`, NO `Deserialize` (embeds the Serialize-only
/// `PreflightVerdict`); NOT `Copy` (owns a `PathBuf`). camelCase wire form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OutputPlanPreview {
    /// The collected set this preview is for (the §0.4.4 registry key).
    pub set: CollectedSetId,
    /// The resolved destination DIRECTORY shown before convert (§1.8 / §2.7) — directory-based, never a
    /// pre-baked final file path (the numbered name is resolved at §2.1 write time).
    pub final_dir_preview: PathBuf,
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
/// [Build-Session-Entscheidung: P2.11] `Serialize` + `Type`, NO `Deserialize` (embeds the Serialize-only
/// `PreflightVerdict`); NOT `Copy`. camelCase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DestinationResolved {
    /// The (now chosen) destination (§0.6 / §2.7).
    pub destination: DestinationChoice,
    /// The recomputed per-location divert for the new destination (§2.7.2); `None` = no divert.
    pub diverted: Option<DivertReason>,
    /// The preflight RE-EVALUATED for the new destination volume (§2.14.4 free-space) so the UI's held C4
    /// verdict never goes stale (§1.8).
    pub preflight: PreflightVerdict,
    /// CARRIED THROUGH UNCHANGED from the C4 verdict — in v1 the §2.5 EquivKey has no destination
    /// component, so re-run is destination-INDEPENDENT (§2.5.1); C5 re-evaluates ONLY `preflight`, never
    /// recomputes `rerun`.
    pub rerun: Option<RerunPrompt>,
}

// ── §0.4.1 C4/C5 lifecycle asymmetry invariant (P2.28) ───────────────────────────────────────────
// C4 `plan_output` and C5 `set_destination` take BYTE-IDENTICAL request payloads
// ({ collectedSetId, target, options, destination }), so their signatures alone cannot distinguish them.
// §0.4.1 ("C4 vs C5 — byte-identical payloads, different contract [DECIDED]") resolves the difference NOT
// by a one-shot guard but BY LIFECYCLE — the rule this module's behaviour (P3.46) + the C4/C5 body boxes
// (P2.44+) honor:
//   1. C4 is RE-CALLABLE at any point in state 4 (eager on the 3→4 transition, then debounced ~150 ms on
//      any target/option change, §5.8) — there is NO "fires exactly once" constraint.
//   2. C5 OWNS the destination: once the user changes it (a C5 on a `collectedSetId`), a subsequent C4 on
//      that same set CARRIES the C5-resolved destination in its `destination: DestinationChoice` argument
//      (caller-passed) and NEVER resets it. There is NO server-side destination store — the destination is
//      authoritative as the C6 argument (§0.4.1 C6 [DECIDED]); the "re-apply the retained C5 destination if
//      C4 arrives carrying a stale default" (§0.4.1) is a P3.46 runtime stale-default REPAIR, NOT a P2 state
//      structure.
//   3. C4 COMPUTES `rerun` (§2.5 equivalence) + the §1.10 `preflight`; C5 NEVER recomputes `rerun` (the v1
//      EquivKey is destination-independent, §2.5.1) — it CARRIES C4's `rerun` THROUGH UNCHANGED and
//      re-evaluates ONLY the destination-volume `preflight` (§2.14.4). This is the ONLY ordering rule.
//
// [Build-Session-Entscheidung: P2.28] Structural + documented layer authored HERE; runtime asserts at P3.46.
// The orchestrator BEHAVIOUR that enforces these at runtime (the re-callable C4 plan, the C5 destination
// authority, the computed-vs-carried-through `rerun`) is the P3.46 conductor + the C4/C5 body boxes (P2.44+).
// P2.28 encodes the two layers that exist at the contract stage: (i) this documented lifecycle invariant the
// P3.46 conductor + the body boxes honor, and (ii) the structural ENABLERS the DTO shapes above already
// guarantee — pinned by the `c4_c5_asymmetry_structural_enablers` test: `DestinationResolved` CARRIES a
// `destination` (C5 owns it) while `OutputPlanPreview` carries only a `final_dir_preview` PREVIEW and NO
// `DestinationChoice` field (C4 does not own the choice), and both carry the SAME `rerun: Option<RerunPrompt>`
// type (so C5 carries C4's `rerun` through unchanged). This is the same contract-here / behaviour-at-P3.46
// split as the C1–C6 shells, NOT a stub: the structure makes the lifecycle rule TYPE-POSSIBLE; P3.46 adds the
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
/// [Build-Session-Entscheidung: P2.12] `Serialize` + `Type` (wire), NO `Deserialize`; NOT `Copy` (owns
/// `Vec`/`PathBuf` fields). camelCase wire form (`collectedSetId`/`runId`/`cleanupIncomplete`/`commonRoot`/
/// `divertRoot`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    /// The frozen collected-set this run summarises — `Batch.id` IS a `CollectedSetId` (§1.12), tying the
    /// summary back to its §0.4.4 collected-set registry entry.
    pub collected_set_id: CollectedSetId,
    /// The run this summary is for (§7.1) — minted at C6 `start_conversion`.
    pub run_id: RunId,
    /// Per-item outcome + output→source mapping (§1.12). INCLUDES the freeze-time pre-flight `SkippedItem`s
    /// (`CollectedSet::Single.skipped`) projected as `ItemResult { state: Skipped(reason), output: None,
    /// reason: Some(OutcomeMsg::Skipped{ reason, .. }) }` — the skip rides the skip-shaped `OutcomeMsg`
    /// variant (§2.8), NOT `Failure`, so skip ≠ fail at the type level (§1.12); counted in `totals.skipped`.
    pub items: Vec<ItemResult>,
    /// The succeeded / failed / cancelled / skipped tally (§1.12).
    pub totals: Totals,
    /// The §2.6 cleanup-incomplete warnings — items whose partial could not be removed, so the run is never
    /// reported as a clean success (§2.6.4). Empty when every cleanup completed.
    pub cleanup_incomplete: Vec<CleanupResidue>,
    /// The "open folder" target for the BESIDE-SOURCE outputs — the dropped-selection common ancestor
    /// (§2.7 / §7.7.3).
    pub common_root: PathBuf,
    /// `Some(Downloads/Documents/chosen)` when ANY item was diverted (§2.7.3) — a single `PathBuf` cannot
    /// carry both the beside-source and divert roots, so the divert root is its own field; `None` when no
    /// item diverted. Both are §7.7.3 open-folder targets; per-item diverted outputs are also reachable via
    /// `ItemResult.output` (C9 `open_path`, `kind = RevealInFolder`).
    pub divert_root: Option<PathBuf>,
}

/// One per-item row of the §1.12 summary (§0.6) — its source path (for output→source mapping), its terminal
/// `JobState`, the output path (`Some` only when `Succeeded`), and the resolved surfaced line.
///
/// [Build-Session-Entscheidung: P2.12] `Serialize` + `Type` (wire — embedded in `RunResult`), NO
/// `Deserialize`; NOT `Copy` (owns `PathBuf`/`OutcomeMsg`). camelCase. `state: JobState` is what forces
/// `JobState` to be a wire type (see its doc) — the summary's per-item state, distinct from the live
/// `ItemFinished`'s `ItemOutcome`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    /// The source path this row is for — the output→source mapping anchor (SSOT *How It Feels* 7).
    pub source: PathBuf,
    /// The terminal §1.9 lifecycle state for this item (§0.6) — at `RunFinished` always a terminal variant
    /// (`Succeeded`/`Failed`/`Skipped`/`Cancelled`).
    pub state: JobState,
    /// The published output path — `Some(..)` ONLY when `state == Succeeded` (§1.12); `None` otherwise.
    pub output: Option<PathBuf>,
    /// The resolved, ready-to-show §2.8 failure / §2.9 lossy / §1.1 skip line (§2.8.2 `OutcomeMsg`); `None`
    /// for a plain success with no lossy note.
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
    /// Pre-flight detection-ineligible items projected into the summary (§1.3/§1.12) — never `failed`.
    pub skipped: u32,
}

impl Totals {
    /// The total item count — the sum of the four tallies (§1.12). Not stored; derived from the parts.
    pub fn total(&self) -> u32 {
        self.succeeded
            .saturating_add(self.failed)
            .saturating_add(self.cancelled)
            .saturating_add(self.skipped)
    }

    /// The §1.12 "all failed" condition (`failed == total && total > 0`) — DERIVED, never stored. A
    /// fully-failed batch is surfaced as an explicit failure, never a quiet finish (SSOT *Fail clearly*).
    pub fn all_failed(&self) -> bool {
        let total = self.total();
        total > 0 && self.failed == total
    }
}

/// A §2.6.4 cleanup-incomplete warning (§0.6) — one item whose partial could not be removed, naming WHERE
/// the residue may remain so the summary never reports a clean success (§2.6 / §1.12).
///
/// [Build-Session-Entscheidung: P2.12] `Serialize` + `Type` (wire — embedded in `RunResult`), NO
/// `Deserialize`; NOT `Copy` (owns a `PathBuf`). camelCase (`residuePath`). `item: ItemId` is the downward
/// `orchestrator`→`domain` edge that co-homing this leaf here introduces (allowed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResidue {
    /// The item whose cleanup did not complete (§2.6.4) — the stable §0.6 `ItemId`.
    pub item: ItemId,
    /// Where the residue may remain (§2.6.4) — the only place the summary names a residue path.
    pub residue_path: PathBuf,
}

/// The terminal per-item outcome carried by the LIVE §0.4.2 `ItemFinished` event (§0.6) — the richer
/// terminal projection the UI applies as each item finishes, distinct from the summary's `JobState`.
/// `Failed` carries the full §0.4.3 `IpcError` (kind + message + path + residue) the live row needs;
/// `Succeeded` the published output path; `Skipped` the §0.6 `SkipReason`; `Cancelled` is payload-free.
///
/// [Build-Session-Entscheidung: P2.12] `Serialize` + `Type` (wire — the `ItemFinished` payload), NO
/// `Deserialize` (outbound-only — embeds the outbound-only `IpcError`); NOT `Copy` (`Failed` owns an
/// `IpcError` with `String`/`PathBuf`). Externally tagged with `#[serde(rename_all = "camelCase")]` (the
/// §0.6 wire-enum convention) + per-struct-variant `rename_all` (serde does not cascade the enum-level
/// rename to a variant's fields, so `Succeeded`'s `output_path` → `outputPath` needs its own, cf.
/// `CollectedSet`). Variant order matches §0.6 exactly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ItemOutcome {
    /// Converted + atomically published (§2.1) — carries the final output path.
    #[serde(rename_all = "camelCase")]
    Succeeded { output_path: PathBuf },
    /// A named §2.8 failure — carries the full §0.4.3 `IpcError` the live row renders.
    #[serde(rename_all = "camelCase")]
    Failed { error: IpcError },
    /// A pre-flight detection-ineligible item (§1.2/§1.3) — carries the §0.6 `SkipReason` (skip ≠ fail).
    #[serde(rename_all = "camelCase")]
    Skipped { reason: SkipReason },
    /// User-cancelled; nothing written (§1.7/§1.11). Not an `ErrorKind` (§0.4.3 note) — payload-free.
    Cancelled,
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
/// reconciled only at the §1.12 Summary. The "= count" equality is a §1.9 RUNTIME emission rule the P3.46
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
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ItemStarted {
    pub run_id: RunId,
    pub item_id: ItemId,
    /// The frozen resolved source path being converted (§2.4).
    pub source_path: PathBuf,
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
    /// staged determinate-looking bar from `stage` there).
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
/// terminal `RunResult.items`), so the policy is a §1.9/§1.12 RUNTIME emission rule the P3.46 conductor honors,
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
/// `BatchProgress.total == RunStarted.total_items` is a §1.11 RUNTIME emission invariant the P3.46 conductor
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

// ─── §0.4.4 run registry — the RunId → CancellationToken token store (P2.42) ─────────────────────────
// [Build-Session-Entscheidung: P2.42] The §0.4.4 cancellation-token registry, homed in crate::orchestrator
// per §0.7 ("orchestrator homes run registry + cancellation"). It owns the token's IDENTITY + LIFECYCLE
// only (created in C6, tripped by C7, dropped on RunFinished) — §0.4.4 explicitly scopes THIS section to
// identity/lifecycle: the §1.7 invocation layer wires the token to the engine subprocess for the
// process-group kill, and cancellation is cooperative at the orchestrator level + forceful at the engine
// level (reconciled by §1.7, built in P3/P4). Like the sibling lifecycle/result types, this is a CONTRACT
// authored before its consumer — but PARTLY consumed from P2.55: `has_active_run` is the §7.1.1 refuse-busy
// predicate `converter_is_busy` reads, and the `.manage(RunRegistry)` registration lives in main()'s Builder
// chain (P2.55). The C6/C7/RunFinished token WIRING (the conductor's register/cancel/finish calls) is the
// P3.46 behaviour, so those three methods stay dead in the production build until then (covered by the
// module-level dead_code expect). The retained terminal RunResult (so C8 re-serves after a WebView reload,
// §0.4.4) is a SEPARATE store — the P2.43 box — NOT this token registry.

/// The §0.4.4 run registry — maps each in-flight `RunId` to its `tokio_util::sync::CancellationToken`. Held
/// as a Tauri app-managed `State` (the `.manage` is P2.55; the register/cancel/finish wiring is the P3.46
/// conductor). The token's three §0.4.4 lifecycle
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
    /// fully terminal). The runs are POPULATED by the P3.46 conductor; until then the registry is empty, so
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
// its consumer: the retain-at-RunFinished / evict-at-C6 / get-at-C8 WIRING is the P3.46 behaviour (C8's
// success path, the P2.31 shell note), so it is dead in the production build until then (covered by the
// module-level dead_code expect).

/// The §0.4.4 RunResult retention — the process-local, in-memory store of the most-recent terminal
/// `RunResult`, kept so C8 `get_run_summary` can idempotently re-serve the §1.12 summary after a WebView
/// reload. Holds AT MOST ONE result (the latest run's): [`retain`](RunResultStore::retain) on `RunFinished`
/// stores it, [`evict`](RunResultStore::evict) on a new run's start (C6) clears the prior one (the §0.4.4
/// "until a new run starts" eviction), and [`get`](RunResultStore::get) serves it back to C8 — matched by
/// `RunId` so a stale/other run's result is never served for the wrong id. NO on-disk persistence (§7.4) — the
/// store is dropped on process exit. Interior-mutable behind a `Mutex` (the `State` form serves concurrent
/// C6/C8 handlers); the critical sections never hold the guard across an `.await`, so a `std::sync::Mutex` is
/// correct.
///
/// [Build-Session-Entscheidung: P2.43] `Default`-constructed empty; `Debug` for parity with the sibling
/// state. NOT a wire type (no `serde`/`specta`) — the `RunResult` it holds IS a wire type, but the STORE is
/// pure core-internal State (C8 returns the resolved `RunResult`; the store itself never crosses IPC).
#[derive(Debug, Default)]
pub struct RunResultStore {
    /// The retained terminal `RunResult` (the latest run's), or `None` between an `evict` and the next
    /// `retain`. A single slot, not a per-`RunId` map: §0.4.4 retains only until the NEXT run starts.
    result: Mutex<Option<RunResult>>,
}

impl RunResultStore {
    /// Lock the slot, recovering a poisoned guard rather than propagating the panic — the in-core no-panic
    /// discipline (G4/G14: no `unwrap`/`expect`/`panic`), sound because the critical sections never panic.
    /// [Build-Session-Entscheidung: P2.43]
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<RunResult>> {
        self.result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Retain `result` as the run's terminal summary (`RunFinished`, §0.4.4) — supersedes any prior retained
    /// result (only the latest run's is kept). After this, C8 `get_run_summary(result.run_id)` re-serves it.
    pub fn retain(&self, result: RunResult) {
        *self.lock() = Some(result);
    }

    /// Re-serve the retained summary for `run_id` (C8 `get_run_summary`, §0.4.4) — returns a clone iff a
    /// result is retained AND its `run_id` matches (so a superseded/other run's id never serves the wrong
    /// summary). `None` = no retained result, or it belongs to a different run (the C8 caller maps that to its
    /// §0.4.3 not-available error). The result is cloned out, so the guard is not held across the return.
    pub fn get(&self, run_id: RunId) -> Option<RunResult> {
        self.lock()
            .as_ref()
            .filter(|result| result.run_id == run_id)
            .cloned()
    }

    /// Evict the retained result when a new run starts (C6, §0.4.4 "until a new run starts") — so a stale
    /// prior summary is never re-served once the next run is in flight. Idempotent: evicting an already-empty
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
// the umbrella needs no fingerprint re-bless). It holds the frozen `CollectedSet::Single` payload (a
// crate::domain FrozenCollectedSet, a downward orchestrator→domain edge like RunRegistry's RunId key) keyed by
// CollectedSetId, so the bare-`collectedSetId` C3/C4/C5/C6 commands resolve back to the detected format /
// frozen items / roots / skipped WITHOUT a second walk or re-detection (§0.4.4). Like the sibling stores it is
// a CONTRACT before its consumer: the register-at-C1/C2a-freeze / resolve-at-C3/C4/C5 / take-at-C6 WIRING is
// the P3.46 conductor + the C-command bodies, so it is dead in the production build until then (covered by the
// module-level dead_code expect).

/// The §0.4.4 collected-set registry — maps each frozen `CollectedSetId` to its `FrozenCollectedSet` (the
/// `CollectedSet::Single` payload), held as a Tauri app-managed `State` so the bare-`collectedSetId` C3
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
/// [Build-Session-Entscheidung: P2.44] Stores `Arc<FrozenCollectedSet>` (not a bare value): C4 is re-callable /
/// debounced (~150 ms, §5.8) so the frozen set — a potentially-large `items` Vec — is READ MANY times per
/// freeze; an `Arc` makes each `resolve`/`take` an O(1) handle clone instead of an O(n) deep copy (the
/// read-many extension of the cheap-`CancellationToken`-clone the RunRegistry already relies on).
/// `Default`-constructed empty; `Debug` for parity with the sibling stores. NOT a wire type (no
/// `serde`/`specta`) — pure core-internal State that never crosses IPC (C3–C6 return their own §0.6 DTOs).
#[derive(Debug, Default)]
pub struct CollectedSetRegistry {
    /// The live `CollectedSetId` → frozen-set map. At most one entry (the current un-run set): `register`
    /// supersedes any prior, `take` (C6) removes it, a process exit drops the store.
    sets: Mutex<HashMap<CollectedSetId, Arc<FrozenCollectedSet>>>,
}

impl CollectedSetRegistry {
    /// Lock the set map, recovering the guard from a poisoned lock rather than propagating the panic — the
    /// in-core no-panic discipline (G4/G14: no `unwrap`/`expect`/`panic`), sound because the critical
    /// sections are infallible whole-map ops that never panic. [Build-Session-Entscheidung: P2.44]
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<CollectedSetId, Arc<FrozenCollectedSet>>> {
        self.sets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Register the frozen set on a C1/C2a freeze (§0.4.4) — SUPERSEDES any prior un-run set (§0.4.4 "a new
    /// C1/C2a supersedes it" + §2.4.3 "a subsequent drop starts a new frozen set"), so at most one entry is ever
    /// live. Keyed by the set's own `id`. After this, C3/C4/C5 `resolve(id)` and C6 `take(id)` see it.
    pub fn register(&self, frozen: FrozenCollectedSet) {
        let id = frozen.id;
        let mut sets = self.lock();
        sets.clear();
        sets.insert(id, Arc::new(frozen));
    }

    /// Resolve a `collectedSetId` to its frozen set (C3/C4/C5, §0.4.4) — a NON-evicting read (C3/C4/C5 may
    /// each fire repeatedly; C4 is debounced-re-callable, §5.8). Returns the `Arc` clone iff `id` is the live
    /// set; `None` if `id` is unknown or was superseded (→ the C-command's §0.4.3 not-available error). The
    /// `Arc` is cloned out before the guard drops, so the lock is not held across the return.
    pub fn resolve(&self, id: CollectedSetId) -> Option<Arc<FrozenCollectedSet>> {
        self.lock().get(&id).map(Arc::clone)
    }

    /// Resolve AND evict the `collectedSetId` (C6 `start_conversion`, §0.4.4 "evicted when its run starts — C6
    /// hands the frozen items to the Batch") — one op so the set leaves the registry exactly as its run
    /// begins, never lingering to be re-run. Returns the `Arc` iff `id` was live; `None` otherwise (an unknown
    /// / already-superseded id → the C6 §0.4.3 not-available error).
    pub fn take(&self, id: CollectedSetId) -> Option<Arc<FrozenCollectedSet>> {
        self.lock().remove(&id)
    }
}

// ─── §0.4.4 ingest registry — the CollectingId → CancellationToken ingest-cancellation store (P2.45) ──
// [Build-Session-Entscheidung: P2.45] The FOURTH §0.4.4 orchestrator-State store, the one-phase-EARLIER
// sibling of the RunRegistry (P2.42): same RunId-token shape, but keyed by the frontend-generated
// CollectingId (§0.4.4 / §1.1) so C13 cancel_ingest can trip an IN-FLIGHT C1 walk / C2a pick before its
// long await resolves. Homed here under the same §0.7 "(§0.4.4) cancellation" umbrella as the RunRegistry
// (no §0.7/§1a structural edit — the P2.43/P2.44 precedent). Like the sibling stores it is a CONTRACT
// before its consumer: the register-at-handler-entry (C1 walk start / C2a BEFORE the dialog opens, §1.1) /
// cancel-at-C13 / release-on-EVERY-handler-exit-branch WIRING is the C1/C2a/C13 handler bodies + the
// walk-loop poll (P2.69/P2.70/P2.71) — the C13 shell (P2.35) already trips no token pending this store —
// so it is dead in the production build until consumed (covered by the module-level dead_code expect).

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
}

impl Drop for IngestGuard<'_> {
    fn drop(&mut self) {
        // §1.1 (P2.70): de-register the ingest token on the C2a handler's exit — fires on every return path
        // (Rust drops the guard regardless of which branch returns), so the token can never leak.
        self.registry.release(self.collecting_id);
    }
}

// ─── §7.8.1 first-launch intake buffer — the PendingIntake stash/drain store (P2.58) ─────────────────────
// [Build-Session-Entscheidung: P2.58] The §7.8.1 first-launch buffer, homed here under the same §0.7 State
// umbrella as the four §0.4.4 sibling stores (RunRegistry/RunResultStore/CollectedSetRegistry/IngestRegistry)
// — a launch-intake State store added to orchestrator needs NO §0.7/§1a structural edit (§0.7 attributes the
// app-managed State to orchestrator + enumerates the outcome-referencing TYPES, not every store; the
// P2.43/P2.44/P2.45 precedent). It is the single-slot sibling of `RunResultStore` (a `Mutex<Option<…>>`): the
// §7.8.1 launch funnel's `Buffer` arm (`buffer_pending_intake`, main.rs) STASHES the idle-and-not-ready launch
// set here when the WebView's `app://intake` listener is not yet ready (the first-launch listener race), and
// the C1 `drainPending` path (P2.60) TAKEs it once on root-shell mount and freezes it (§1.1). It differs from
// the §0.4.4 run/ingest stores only in WHO drives it — the launch glue writes, C1 reads — but the State-store
// shape + the contract-before-consumer discipline (the `take` reader is dead in the production build until
// P2.60 wires C1, covered by the module-level dead_code expect) are identical.

/// One buffered §7.8.1 first-launch intake — a launch path set + its §0.6 `IntakeOrigin`. Stashed by the
/// launch funnel's `Buffer` arm when the WebView is not yet ready, drained once by C1 `drainPending` (P2.60).
/// NOT a wire type: the C1 drain returns a `CollectedSet` (§0.4.1), never this buffer (pure core-internal
/// State). [Build-Session-Entscheidung: P2.58]
#[derive(Debug, Clone)]
pub struct BufferedLaunchIntake {
    /// The launch-time paths (already `parse_path_args`-classified, §7.8.1), accumulated across any repeat
    /// stash in the same not-ready window (no-loss — see [`PendingIntake::stash`]).
    pub paths: Vec<PathBuf>,
    /// The §0.6 origin of the FIRST stash in this not-ready window (typically `LaunchArg`; §7.8.1 "its stored
    /// origin"). A subsequent stash in the same window accumulates its paths but keeps this origin — the §1.1
    /// freeze re-validates every path and is origin-agnostic, so one stored origin for the merged set is
    /// correct.
    pub origin: IntakeOrigin,
}

/// The §7.8.1 first-launch buffer (`State<PendingIntake>`) — holds at most one un-drained
/// [`BufferedLaunchIntake`]. The single-slot sibling of [`RunResultStore`]: the launch funnel's `Buffer` arm
/// stashes here when the WebView's `app://intake` listener is not yet ready (the first-launch listener race,
/// §7.8.1), and C1 `drainPending` (P2.60) drains it exactly once on root-shell mount. Held as a Tauri
/// app-managed `State` (registered in `main()`'s Builder chain). Interior-mutable behind a `Mutex` (the
/// `State` form is shared across the launch hooks + the C1 handler); the critical sections are infallible
/// slot ops that never hold the guard across an `.await`, so a `std::sync::Mutex` is correct.
///
/// [Build-Session-Entscheidung: P2.58] `Default`-constructed empty; `Debug` for parity with the sibling
/// stores. NOT a wire type (no `serde`/`specta`) — pure core-internal State that never crosses IPC (the C1
/// drain returns a `CollectedSet`, §0.4.1).
#[derive(Debug, Default)]
pub struct PendingIntake {
    /// The single buffered launch set, or `None` when nothing is pending (never stashed, or already drained).
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

    /// Stash a launch set from the §7.8.1 `Buffer` arm (idle + WebView not-yet-ready). NO-LOSS on a repeat
    /// stash before the drain: a second launch event in the same not-ready window APPENDS its paths to the
    /// pending set rather than superseding it — superseding would drop the earlier launch's paths, the very
    /// loss this buffer exists to prevent (§7.8.1) — and keeps the FIRST origin (§7.8.1 "its stored origin").
    /// The funnel only reaches this with a non-empty `paths` (it returns early on empty, §7.8.1), so no
    /// empty-stash guard is needed. [Build-Session-Entscheidung: P2.58]
    pub fn stash(&self, paths: Vec<PathBuf>, origin: IntakeOrigin) {
        let mut slot = self.lock();
        match slot.as_mut() {
            Some(buffered) => buffered.paths.extend(paths),
            None => *slot = Some(BufferedLaunchIntake { paths, origin }),
        }
    }

    /// Take the buffered launch set, clearing the slot — the C1 `drainPending` consume-once drain (P2.60,
    /// §7.8.1 "consumes `PendingIntake` exactly once"). Returns `None` when nothing is pending (the ordinary
    /// first launch with no files → C1 returns `CollectedSet::Empty`, §0.4.1). Idempotent: a second drain is
    /// `None`. [Build-Session-Entscheidung: P2.58]
    pub fn take(&self) -> Option<BufferedLaunchIntake> {
        self.lock().take()
    }
}

// ─── §7.8.1 WebView-ready flag — the FrontendReady emit-vs-buffer gate (P2.59) ────────────────────────────
// [Build-Session-Entscheidung: P2.59] The §7.8.1 / §0.4.2 WebView-ready flag, homed here under the same §0.7
// State umbrella as the launch-intake sibling PendingIntake (P2.58) + the four §0.4.4 stores — a launch-intake
// State store added to orchestrator needs NO §0.7/§1a structural edit (the P2.58 precedent: §0.7 attributes the
// app-managed State to orchestrator, not every store). It records whether the WebView's `app://intake` listener
// is registered: the §7.8.1 launch funnel reads it (`frontend_ready`, main.rs) to choose the `Emit` arm (ready →
// emit `app://intake`) versus the `Buffer` arm (not-ready → stash into PendingIntake, the §7.8.1 first-launch
// listener race), and the C1 `drainPending` path (P2.60 — on root-shell mount, AFTER the listener registers)
// marks it ready. MONOTONIC false→true: the `main` window lives for the whole session (§7.3.1 closing-quits) so
// the listener never un-registers, hence the flag never resets — an `AtomicBool` is the right tool (no
// Mutex/poison handling; the reader needs only the published boolean, no data is gated behind it). Both
// methods are LIVE: `mark_ready` is called by the C1 `drainPending` handler (`crate::ipc::intake::
// resolve_intake_source`, P2.60 — the drain call is the §7.8.1 root-shell-mount readiness signal), and
// `is_ready` is read by the §7.8.1 funnel's `frontend_ready` (P2.59, main.rs).

/// The §7.8.1 WebView-ready flag (`State<FrontendReady>`) — `true` once the frontend has registered its
/// `app://intake` listener and run the C1 `drainPending` drain (P2.60) on root-shell mount. The §7.8.1 launch
/// funnel reads it (`frontend_ready`, main.rs) to pick Emit-vs-Buffer: a launch set arriving BEFORE the
/// listener exists (the first-launch race, §7.8.1) is buffered into [`PendingIntake`] rather than emitted into
/// a listener that would drop it. Held as a Tauri app-managed `State` (registered in `main()`'s Builder chain,
/// so the funnel's `frontend_ready` resolve is infallible by construction). A monotonic false→true flag, so an
/// `AtomicBool` (no `Mutex`/poison handling) is the right shape.
///
/// [Build-Session-Entscheidung: P2.59] `Default`-constructed `false` (not-ready at app start — the funnel's
/// fail-safe default: a launch set is buffered, never emitted, until the frontend proves its listener exists);
/// `Debug` for parity with the sibling State stores. NOT a wire type (no `serde`/`specta`) — pure core-internal
/// State that never crosses IPC.
#[derive(Debug, Default)]
pub struct FrontendReady {
    /// `true` once the WebView's `app://intake` listener is live (set by the C1 `drainPending` drain, P2.60).
    ready: AtomicBool,
}

impl FrontendReady {
    /// Mark the frontend ready — the WebView has registered its `app://intake` listener and is draining
    /// `PendingIntake` (the C1 `drainPending` path on root-shell mount, P2.60). Monotonic: once set it never
    /// clears (the `main` window lives for the whole session, §7.3.1), so a repeat call is a harmless no-op.
    /// `Release` publishes the write so a subsequent `is_ready` `Acquire` observes it.
    /// [Build-Session-Entscheidung: P2.59]
    pub fn mark_ready(&self) {
        self.ready.store(true, Ordering::Release);
    }

    /// Read the §7.8.1 ready flag — `true` once [`mark_ready`](FrontendReady::mark_ready) has fired. The §7.8.1
    /// launch funnel's `frontend_ready` predicate (main.rs) reads this to choose the `Emit` arm (ready) versus
    /// the `Buffer` arm (not-ready, the first-launch race). `Acquire` pairs with `mark_ready`'s `Release`.
    /// [Build-Session-Entscheidung: P2.59]
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
}

/// The §1.1 / §2.4 **intake-freeze funnel** — the single, exhaustive freeze point every intake entry
/// point routes through (SSOT *Never harm the original*). All five §1.1 entry points reduce to this one
/// Rust function: the C1 `ingest_paths` drop / launch-arg / second-instance set, and the C2a
/// `pick_for_intake` picked set (origin stamped `Picker` by the C2a handler, §1.1) — so the §2.4 freeze
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
///    `resolve_identity` producer that yields each identity — P3.1.1 shell / P3.6 body) / P2.76 (the pure de-dup
///    fold over those identities, [`dedup_by_identity`]).
/// 4. **Assign `ItemId`** over the single id space (eligible + skipped, never re-indexed, §0.6
///    invariant 6) → P2.75.
/// 5. **Group** the frozen snapshot into the §0.6 `CollectedSet` variant (`Single` / `Mixed` /
///    `Unsupported` / `Uncertain` / `Empty`, §1.3) → P3 (`group()`).
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
/// (and the same zero-collection result the C1 / C2a shells already return for an empty/cancelled intake,
/// §0.4.1 / §5.4). [Test-Change: P2.63 — old-obsolete+new-correct, §1.1] the P2.62 per-fn dead-code lint
/// attribute on `ingest` is removed now the funnel is LIVE — its first production caller is the C2a
/// `pick_for_intake` picker (P2.63, which stamps `Picker` and funnels its picked set here); keeping the
/// attribute would error "unfulfilled expectation" under -D warnings (a production lint change, not a test
/// suppression). The C1 `ingest_paths` handler wires it end-to-end at P3.49 (the CSV→TSV walking skeleton);
/// the funnel returns the zero-collection `Empty` for every input until its §2.4.1 spine stages land (P2.64 / P3).
#[must_use]
pub fn ingest(paths: Vec<PathBuf>, origin: IntakeOrigin) -> CollectedSet {
    // §2.4.1 freeze spine: walk (P2.64) → detect (P3) → resolve-identity (P3, produces the P2.74
    // `FileIdentity`) + de-dup (P2.76) → assign
    // `ItemId` (P2.75) → group (P3). While those stages are unbuilt the frozen snapshot is empty, so the
    // §1.3 projection of a no-eligible-source freeze is the §0.6 zero-collection `Empty` (§0.4.1 / §5.4).
    let _ = (paths, origin);
    CollectedSet::Empty {
        skipped: Vec::new(),
    }
}

/// The §2.4.1 freeze-spine step-1 intake-walk result (P2.66): the flat candidate file list
/// ([`walk_intake_roots`], P2.64) PLUS the **dropped root(s) retained VERBATIM** for §2.7
/// (relative-subtree re-creation + the "open folder" common-root anchor). §2.7 owns the common-root /
/// relative-subtree COMPUTATION; this is plain §1.1 retention — the roots are carried through the walk so
/// the P3.49 ingest funnel can freeze them onto `CollectedSet::Single.roots` (§0.6).
/// [Build-Session-Entscheidung: P2.66]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "P2.66 retains the dropped roots + P2.67 the per-item Unreadable skips on the §2.4.1 \
                  freeze-spine step-1 result; produced by walk_intake_roots, frozen onto the §0.6 CollectedSet \
                  by the ingest funnel at P3.49 — dead in the production build pending that wiring, read by \
                  the in-module walk_tests below."
    )
)]
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
    /// The dropped/picked root that could not be read (for the §1.1 fatal-ingest message).
    root: PathBuf,
    /// Why the root could not be read — the §0.6 [`ReadFailure`] taxonomy reused here (gone vs denied/io), so
    /// the P3.49 surfacing distinguishes "gone" (`NotFound`) from "unreadable".
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "P2.64 authors the §2.4.1 freeze-spine step-1 walk primitive (the §1.1 recursive folder \
                  enumeration). Its production caller is the `ingest` funnel, consumed at P3.49 (the CSV→TSV \
                  walking skeleton); dead in the production build pending that wiring, exercised by the \
                  in-module walk_tests below — the same per-fn interface-shell attribute `ingest` itself \
                  carried (P2.62) before P2.63 consumed it."
    )
)]
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
                    // a bad root sinks the ingest, a bad file never does. `cause` carries the gone-vs-unreadable
                    // §0.6 `ReadFailure` for the P3.49 fatal-ingest message.
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
/// outcome, not an intake one; so the walk-root maps only the gone-vs-unreadable distinction the §1.1
/// fatal-ingest message needs.
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
/// representative path the P3.49 spine projects onto `DroppedItem.resolved_path`, §0.6, and the identity
/// itself feeds §2.3.3 `is_safe_output`), and the abstract per-candidate `payload` the spine threads through
/// un-inspected (detection is P3, so P2.76 never constructs a §0.6 `DroppedItem`/`SkippedItem`).
///
/// [Build-Session-Entscheidung: P2.76] Derives `Debug` ONLY — NOT `PartialEq`/`Eq`: those would leak a
/// `P: PartialEq/Eq` bound onto every consumer, and a whole-struct `Eq` would be MISLEADING (`FileIdentity`'s
/// `Eq` ignores `canonical_path`, so two rows with different first-seen paths but the same identity would
/// compare equal). The §6.4.1 tests assert on the fields (`.id` / `.identity.canonical_path` / `.payload`)
/// individually instead. Core-INTERNAL (never crosses IPC) → no `serde`/`specta`.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "P2.76's §2.3.2 de-dup fold `dedup_by_identity` yields `DedupedMember<P>` (the id + \
                  retained FileIdentity + payload survivor row). Its production reader is the `ingest` \
                  freeze funnel's spine, wired at P3.49 (which resolves each candidate's FileIdentity, folds \
                  it in, then projects each survivor into a §0.6 DroppedItem); dead in the production build \
                  pending that wiring, constructed by the in-module dedup_tests below."
    )
)]
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "P2.76 authors the §2.4.1 freeze-spine step-3 resolved-identity de-dup fold. Its production \
                  caller is the `ingest` funnel's spine, wired at P3.49 (the CSV→TSV walking skeleton); dead \
                  in the production build pending that wiring, exercised by the in-module dedup_tests below — \
                  the same per-fn interface-shell attribute `walk_intake_roots` (P2.64) carries."
    )
)]
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
    /// A minimal `FrozenCollectedSet` carrying `id` + a content-distinguishing `count`/`total_bytes`, so the
    /// never-merge leg asserts the resolved latest set is the freeze's OWN content (not a merge of a prior).
    fn frozen(id: CollectedSetId, count: usize, total_bytes: u64) -> FrozenCollectedSet {
        FrozenCollectedSet {
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
            reg.resolve(next).map(|s| s.count),
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
            (live.count, live.total_bytes),
            (5, 50),
            "§2.4.3: the new frozen set is the freeze's OWN content — never a merge of the prior set (a merge would be count 8 / bytes 80)"
        );
        // A same-id re-freeze (a re-drop minting the same logical id) REPLACES the snapshot, never accumulates.
        reg.register(frozen(b, 7, 70));
        assert_eq!(
            reg.resolve(b).map(|s| (s.count, s.total_bytes)),
            Some((7, 70)),
            "§2.4.3: a re-freeze of the same id replaces the snapshot (clear-then-insert), never accumulates onto it"
        );
    }

    // Leg 3 — §6.4.1 unit (G15): the busy launch-intake is refused UPSTREAM, so this freeze is never reached.
    // The §7.1.1 PRIMARY rule `intake_disposition` (the funnel reads it, P2.55) returns `Drop` for a busy
    // converter in EVERY readiness state — `Drop` emits no `app://intake` and buffers nothing, so neither the
    // ready re-call (Emit) nor the first-launch drain (Buffer) ever routes paths back into the orchestrator
    // freeze (`ingest`). This asserts the DELEGATION SEAM (refuse-busy is upstream, not in the freeze); the
    // full busy x ready truth table is `crate::launch_intake::tests` (the pure rule's home).
    // [Build-Session-Entscheidung: P2.72] The contract test reaches the `pub(crate)` upstream rule so the
    // "freeze never reached" delegation is asserted end-to-end, not only documented.
    #[test]
    fn busy_launch_intake_is_refused_upstream_so_the_freeze_is_never_reached() {
        use crate::launch_intake::{intake_disposition, IntakeDisposition};
        for ready in [true, false] {
            assert_eq!(
                intake_disposition(true, ready),
                IntakeDisposition::Drop,
                "§7.1.1/§2.4: a busy converter DROPS the launch-intake upstream (ready={ready}) — no emit, no buffer, so the orchestrator freeze is never reached mid-run"
            );
        }
    }

    // Structural Reading-B anchor — §6.4.1 unit (G15): there is NO core-side freeze gate, BY CONSTRUCTION.
    // `ingest`'s signature takes only the paths + origin; it carries no run-state / `busy` parameter, so it
    // CANNOT refuse-busy — the refusal is necessarily upstream (Leg 3). A drift that bolted a core-side gate
    // onto the freeze (an added `busy` / `&RunRegistry` parameter) would fail this fn-pointer coercion to
    // compile — the signature pin is the structural guard the doc-comments delegate to.
    #[test]
    fn ingest_freeze_carries_no_core_side_busy_gate() {
        let _freeze: fn(Vec<PathBuf>, IntakeOrigin) -> CollectedSet = ingest;
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

    /// A minimal eligible CSV source item, for the job/batch shape tests.
    fn dropped_item(id: u32) -> DroppedItem {
        DroppedItem {
            item: item_id(id),
            raw_path: PathBuf::from("data.csv"),
            resolved_path: PathBuf::from("data.csv"),
            size_bytes: 12,
            detected: DetectionOutcome::Recognized {
                format: UserFacingFormat::Csv,
                confidence: Confidence::High,
                dims: None,
            },
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

    // §6.4.1 unit (G15): the §1.1/§2.4 `ingest` freeze funnel (P2.62) is the single, exhaustive freeze
    // point every intake origin routes through, returning the §0.6 `CollectedSet`. While the §2.4.1 spine
    // stages are unbuilt (walk P2.64 / detect P3 / de-dup P2.76 / group P3) the funnel collects nothing,
    // so it returns the genuine zero-collection `CollectedSet::Empty { skipped: [] }` for EVERY origin and
    // for both an empty and a non-empty path set — the seam contract, not an origin- or input-specific
    // branch. This pins the §2.4.1 "all five entry points → one funnel" shape: one fn, every
    // `IntakeOrigin`, one zero-collection contract.
    #[test]
    fn ingest_funnel_returns_zero_collection_for_every_origin() {
        // Compile-time variant lock (the established `exhaustive`-match pattern, cf.
        // `job_state_is_the_six_lifecycle_states`): a new `IntakeOrigin` variant breaks this match,
        // forcing the `all` array below — the funnel's "all five entry points" coverage — to grow with it,
        // so the test can never silently miss a new origin. [Build-Session-Entscheidung: P2.62]
        fn exhaustive(o: IntakeOrigin) {
            match o {
                IntakeOrigin::Drop
                | IntakeOrigin::Picker
                | IntakeOrigin::LaunchArg
                | IntakeOrigin::SecondInstance => {}
            }
        }
        let zero = CollectedSet::Empty {
            skipped: Vec::new(),
        };
        let all = [
            IntakeOrigin::Drop,
            IntakeOrigin::Picker,
            IntakeOrigin::LaunchArg,
            IntakeOrigin::SecondInstance,
        ];
        for origin in all {
            exhaustive(origin);
            assert_eq!(
                ingest(Vec::new(), origin),
                zero,
                "§1.1/§2.4: an empty intake set yields the zero-collection Empty for every origin"
            );
            assert_eq!(
                ingest(
                    vec![PathBuf::from("/drop/data.csv"), PathBuf::from("/drop/pic.png")],
                    origin,
                ),
                zero,
                "§1.1/§2.4: the §2.4.1 spine collects nothing until the walk/detect/group fills land — the \
                 zero-collection Empty, not an origin- or input-specific CollectedSet"
            );
        }
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

    // §6.4.1 unit (G15): the §0.6 `ConversionJob.item == source.item` denormalization — the job's
    // top-level key IS its source item's id (cheap addressing without unwrapping `source`). This box
    // authors the TYPE so the relationship is expressible + correct; the orchestrator-ALWAYS-enforces-it
    // property is P2.14.
    #[test]
    fn conversion_job_denormalizes_its_source_item() {
        let source = dropped_item(3);
        let job = ConversionJob {
            item: source.item,
            source: source.clone(),
            state: JobState::Pending,
            plan: None,
        };
        assert_eq!(
            job.item, job.source.item,
            "§0.6: ConversionJob.item is denormalized from source.item"
        );
    }

    // §6.4.1 unit (G15): a `Batch` carries ONE whole-batch `Target` (§0.6 invariant 1, enforced by the
    // single-value field SHAPE) over its jobs. Constructs the full `Batch → ConversionJob → DroppedItem`
    // graph so the P2.10 types are exercised (and the test build is dead-code-clean); the per-item
    // invariant ENFORCEMENT (count/frozen/stable-id) is the P2.14 property suite.
    #[test]
    fn batch_holds_one_target_over_its_jobs() {
        let source = dropped_item(0);
        let job = ConversionJob {
            item: source.item,
            source,
            state: JobState::Pending,
            plan: None,
        };
        let batch = Batch {
            id: collected_set_id(),
            source_format: UserFacingFormat::Csv,
            target: sample_target(),
            options: OptionValues(BTreeMap::new()),
            destination: DestinationChoice::BesideSource,
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
    // the full nested camelCase graph (set / finalDirPreview / diverted / rerun / preflight). A SERIALIZE
    // pin (the embedded `PreflightVerdict` is outbound-only, so `OutputPlanPreview` does not round-trip).
    #[test]
    fn output_plan_preview_wire_form_is_camelcase() {
        let preview = OutputPlanPreview {
            set: collected_set_id(),
            final_dir_preview: PathBuf::from("/dest"),
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
            r#"{"set":"00000000-0000-4000-8000-000000000000","finalDirPreview":"/dest","diverted":"unwritable","rerun":{"equivalentCount":2},"preflight":{"estTotalOutputBytes":1024,"estTotalScratchBytes":256,"upFrontFail":null}}"#,
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
            r#"{"destination":"besideSource","diverted":null,"preflight":{"estTotalOutputBytes":4096,"estTotalScratchBytes":0,"upFrontFail":null},"rerun":null}"#,
            "§1.8/§2.14.4: DestinationResolved re-validates the destination; rerun carried through (§2.5.1)"
        );
    }

    // §6.4.1 unit (G15): the §0.4.1 C4/C5 lifecycle-asymmetry STRUCTURAL ENABLERS (P2.28). The runtime
    // enforcement (C4 re-callable, C5 destination authority, C4 computes `rerun` while C5 carries it through)
    // is the P3.46 conductor + the C4/C5 body boxes; this test pins the layer the DTO shapes guarantee NOW —
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

        // C4 does NOT own the destination choice: `OutputPlanPreview` carries a `final_dir_preview` PREVIEW
        // and NO `DestinationChoice` field. This EXHAUSTIVE literal (all 5 fields, no `..`) PINS the field
        // set — adding a `destination` field to `OutputPlanPreview` would make this fail to compile, so "C4
        // has no settable destination" is gate-enforced here, not just prose (§0.4.1 "C4 never overrides C5").
        let preview = OutputPlanPreview {
            set: collected_set_id(),
            final_dir_preview: PathBuf::from("/dest"),
            diverted: None,
            rerun: Some(RerunPrompt {
                equivalent_count: 1,
            }),
            preflight: preflight.clone(),
        };
        let _: &PathBuf = &preview.final_dir_preview; // C4 shows a directory PREVIEW, never a settable destination

        // (3) C5 CARRIES C4's `rerun` THROUGH UNCHANGED (§2.5.1): both DTOs carry the SAME
        // `rerun: Option<RerunPrompt>` type, so the C4 value assigns verbatim into the C5 return.
        let carried_from_c4: Option<RerunPrompt> = preview.rerun.clone();
        let resolved_carrying = DestinationResolved {
            destination: DestinationChoice::BesideSource,
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

        // ItemStarted — runId / itemId / sourcePath / target, adjacently tagged camelCase.
        let item_started = ConversionEvent::ItemStarted(ItemStarted {
            run_id: run_id(),
            item_id: item_id(1),
            source_path: PathBuf::from("/in/a.csv"),
            target: TargetId::Format(FormatId::Tsv),
        });
        let v = serde_json::to_value(&item_started).expect("ItemStarted serializes");
        assert_eq!(v["type"], "itemStarted", "§0.4.2: adjacent tag");
        assert_eq!(
            v["data"]["sourcePath"], "/in/a.csv",
            "§0.4.2: camelCase sourcePath"
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
        // P3.46 runtime emission rule documented on ItemFinished; this asserts only the structural carriability.
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
             the no-live-emit policy is the P3.46 runtime rule)"
        );

        // P2.37.1 + P2.37.3: BatchProgress.total and RunStarted.total_items are the SAME queued-only u32
        // denominator. The RUNTIME equality is a P3.46 invariant; here both carry the same N on the wire.
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
            common_root: PathBuf::from("/src"),
            divert_root: None,
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

    // §6.4.1 unit (G15): the §0.6/§0.4.2 `ItemOutcome` WIRE form (P2.12) — the live `ItemFinished` payload,
    // externally tagged camelCase; `Succeeded`'s `output_path` → `outputPath` (per-variant rename), `Failed`
    // carries the full §0.4.3 IpcError, `Cancelled` is payload-free. A SERIALIZE pin (outbound-only).
    #[test]
    fn item_outcome_wire_form_is_externally_tagged_camelcase() {
        let succeeded = ItemOutcome::Succeeded {
            output_path: PathBuf::from("/out/data.tsv"),
        };
        assert_eq!(
            serde_json::to_string(&succeeded).expect("ItemOutcome::Succeeded serializes"),
            r#"{"succeeded":{"outputPath":"/out/data.tsv"}}"#,
            "§0.4.2: Succeeded carries the published outputPath"
        );
        let failed = ItemOutcome::Failed {
            error: IpcError {
                kind: ConversionErrorKind::EngineError,
                message: "ConvertIA couldn't convert this file.".to_owned(),
                path: Some(PathBuf::from("/src/bad.csv")),
                residue: None,
            },
        };
        assert_eq!(
            serde_json::to_string(&failed).expect("ItemOutcome::Failed serializes"),
            r#"{"failed":{"error":{"kind":"engineError","message":"ConvertIA couldn't convert this file.","path":"/src/bad.csv","residue":null}}}"#,
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

    // §6.4.1 unit (G15): the §1.12 `RunResult` wire form (P2.12) — the full nested camelCase graph the §5.3
    // Summary renders, exercising `ItemResult` (a Succeeded row + a pre-flight Skipped row whose `reason`
    // rides the adjacently-tagged `OutcomeMsg::Skipped`), `Totals`, `CleanupResidue`, and `divertRoot`
    // Some(..). A SERIALIZE pin (RunResult is outbound-only — the §0.4.2 RunFinished payload / C8 return).
    #[test]
    fn run_result_wire_form_is_camelcase() {
        let run = RunResult {
            collected_set_id: collected_set_id(),
            run_id: run_id(),
            items: vec![
                ItemResult {
                    source: PathBuf::from("/src/data.csv"),
                    state: JobState::Succeeded,
                    output: Some(PathBuf::from("/src/data.tsv")),
                    reason: None,
                },
                ItemResult {
                    source: PathBuf::from("/src/mystery.bin"),
                    state: JobState::Skipped(SkipReason::Uncertain),
                    output: None,
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
                residue_path: PathBuf::from("/src/.data.tsv.part"),
            }],
            common_root: PathBuf::from("/src"),
            divert_root: Some(PathBuf::from("/Downloads")),
        };
        assert_eq!(
            serde_json::to_string(&run).expect("RunResult serializes"),
            concat!(
                r#"{"collectedSetId":"00000000-0000-4000-8000-000000000000","#,
                r#""runId":"11111111-1111-4111-8111-111111111111","#,
                r#""items":[{"source":"/src/data.csv","state":"succeeded","output":"/src/data.tsv","reason":null},"#,
                r#"{"source":"/src/mystery.bin","state":{"skipped":"uncertain"},"output":null,"#,
                r#""reason":{"type":"skipped","data":{"reason":"uncertain","text":"ConvertIA couldn't tell what kind of file this is, so it can't convert it."}}}],"#,
                r#""totals":{"succeeded":1,"failed":0,"cancelled":0,"skipped":1},"#,
                r#""cleanupIncomplete":[{"item":2,"residuePath":"/src/.data.tsv.part"}],"#,
                r#""commonRoot":"/src","divertRoot":"/Downloads"}"#
            ),
            "§1.12: RunResult is the end-of-batch summary graph in camelCase (pre-flight skip rides \
             OutcomeMsg::Skipped, not Failure — skip ≠ fail)"
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
    // construction; the LIVE-path enforcement (the real P3.46 orchestrator builder over a real run) is the P3
    // integration leg (test-strategy §1.1 / §6 — the data-structure leg is here, the live-path leg is there).
    // [Build-Session-Entscheidung: P2.14] case-count floor 512 + a `deterministic_rng`-pinned seed; ids built
    // via the orchestrator-test `item_id` serde helper (the `ItemId` field is private to `crate::domain`, so
    // the cross-module test mints it through its public bare-number wire form, never a back-door past the
    // §1.1/§7.1 minting policy).

    /// The §0.6-invariant property-test case-count floor (test-strategy §1.3: above proptest's 256 default).
    const P2_14_CASES: u32 = 512;

    /// The §1.2 recognized format of a (test) source item — for the §1.3 one-format-per-batch grouping check.
    fn recognized_format(d: &DroppedItem) -> Option<UserFacingFormat> {
        // Exhaustive (the crate denies `clippy::wildcard_enum_match_arm`, so no `_` arm) — a future
        // `DetectionOutcome` variant forces a conscious decision here rather than silently mapping to `None`.
        match &d.detected {
            DetectionOutcome::Recognized { format, .. } => Some(*format),
            DetectionOutcome::UnsupportedType { .. }
            | DetectionOutcome::Uncertain { .. }
            | DetectionOutcome::Empty
            | DetectionOutcome::Unreadable { .. } => None,
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

    /// §0.6 invariant 1 (one-Target-per-Batch) + the §1.3 single-format grouping: a `Batch` carries ONE
    /// whole-batch `target` (a single value, never per-item) over an arbitrary number of jobs, and EVERY job's
    /// source is that one `source_format` — the grouping key is whole-batch. `ConversionJob` has no `target`
    /// field, so the only target in effect is `batch.target`; this locks the shape against a future
    /// per-job-target regression and asserts the one-format grouping over any job count.
    #[test]
    fn prop_batch_is_one_target_and_one_source_format_over_arbitrary_jobs() {
        pinned_runner()
            .run(&(0usize..64), |n| {
                let jobs: Vec<ConversionJob> = (0..n)
                    .map(|i| {
                        let id = u32::try_from(i).expect("n < 64 fits u32");
                        ConversionJob {
                            item: item_id(id),
                            source: dropped_item(id),
                            state: JobState::Pending,
                            plan: None,
                        }
                    })
                    .collect();
                let batch = Batch {
                    id: collected_set_id(),
                    source_format: UserFacingFormat::Csv,
                    target: sample_target(),
                    options: OptionValues(BTreeMap::new()),
                    destination: DestinationChoice::BesideSource,
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
                Ok(())
            })
            .unwrap();
    }

    /// §0.6 "`ConversionJob.item == source.item`": the job's top-level key is DENORMALIZED from its source
    /// item's id (cheap addressing without unwrapping `source`). Holds for ANY generated source id; the teeth
    /// assertion shows a deliberately-mismatched item is detectable, so the equality is a real constraint, not
    /// a vacuous `x == x`.
    #[test]
    fn prop_conversion_job_item_equals_source_item() {
        pinned_runner()
            .run(&any::<u32>(), |id| {
                let source = dropped_item(id);
                let job = ConversionJob {
                    item: source.item,
                    source: source.clone(),
                    state: JobState::Pending,
                    plan: None,
                };
                prop_assert_eq!(
                    job.item,
                    job.source.item,
                    "§0.6: ConversionJob.item == source.item"
                );
                prop_assert_eq!(
                    job.item,
                    item_id(id),
                    "the denormalized key tracks the generated source id"
                );
                // teeth: a job whose item is NOT its source's id is detectably inconsistent — `wrapping_add(1)`
                // never equals `id` for any u32, so the denormalization invariant discriminates correct from wrong.
                let mismatched = ConversionJob {
                    item: item_id(id.wrapping_add(1)),
                    source,
                    state: JobState::Pending,
                    plan: None,
                };
                prop_assert_ne!(
                    mismatched.item, mismatched.source.item,
                    "a mismatched item IS detectable — the denormalization invariant is not vacuous"
                );
                Ok(())
            })
            .unwrap();
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

    /// A minimal §1.12 `RunResult` for the §0.4.4 retention tests — one succeeded item, no residue.
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
            common_root: PathBuf::from("/out"),
            divert_root: None,
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
        store.retain(result.clone());
        assert_eq!(
            store.get(run_id()),
            Some(result),
            "§0.4.4: a retained terminal RunResult is re-served to C8 for its own RunId"
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
        store.retain(sample_run_result(run_id()));
        assert_eq!(
            store.get(run_id_other()),
            None,
            "§0.4.4: a retained result is NEVER served for a different run's id (the RunId match guards it)"
        );
    }

    #[test]
    fn run_result_store_retain_supersedes_the_prior_result() {
        let store = RunResultStore::default();
        store.retain(sample_run_result(run_id()));
        store.retain(sample_run_result(run_id_other()));
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
        store.retain(sample_run_result(run_id()));
        store.evict();
        assert_eq!(
            store.get(run_id()),
            None,
            "§0.4.4: evict (a new run starting) clears the retained result so a stale summary is not re-served"
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
    /// A minimal `FrozenCollectedSet` carrying `id` — empty payload, since the §0.4.4 registry's
    /// register/resolve/take/supersede lifecycle is content-agnostic (the full-payload projection is tested
    /// in `crate::domain::tests::frozen_collected_set_projects_only_single_with_full_payload`).
    fn frozen_set(id: CollectedSetId) -> FrozenCollectedSet {
        FrozenCollectedSet {
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

    // §6.4.1 unit (G15): the §7.8.1 stash→drain round-trip — a buffered launch set is taken back with its
    // paths + the stored origin, and the slot is cleared (the C1 drainPending consume-once, §7.8.1).
    #[test]
    fn pending_intake_stash_then_take_returns_the_set_and_clears() {
        let buf = PendingIntake::default();
        buf.stash(paths(&["a.png", "b.jpg"]), IntakeOrigin::LaunchArg);
        let drained = buf.take().expect("§7.8.1: a stashed set is drained back");
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
            buf.take().is_none(),
            "§7.8.1: the drain consumes exactly once — a second take is None (idempotent)"
        );
    }

    // §6.4.1 unit (G15): an un-stashed buffer drains to None — the ordinary first launch with no files,
    // which C1 maps to CollectedSet::Empty (§0.4.1 / §7.8.1).
    #[test]
    fn pending_intake_empty_take_is_none() {
        let buf = PendingIntake::default();
        assert!(
            buf.take().is_none(),
            "§7.8.1: a never-stashed buffer drains to None (first launch, no files)"
        );
    }

    // §6.4.1 unit (G15): NO-LOSS on a repeat stash before the drain — a second launch event in the same
    // not-ready window APPENDS its paths (never supersedes, which would drop the earlier launch's paths) and
    // keeps the FIRST origin (§7.8.1 "its stored origin"). This is the property the path-loss-avoidance the
    // owner-confirmed P2.58-before-P2.55 order rests on (every reachable launch set is preserved).
    #[test]
    fn pending_intake_repeat_stash_accumulates_paths_keeps_first_origin() {
        let buf = PendingIntake::default();
        buf.stash(paths(&["first.png"]), IntakeOrigin::LaunchArg);
        buf.stash(
            paths(&["second.jpg", "third.gif"]),
            IntakeOrigin::SecondInstance,
        );
        let drained = buf
            .take()
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
    // drainPending drain, P2.60) the funnel switches from the Buffer arm to the Emit arm (§7.8.1).
    #[test]
    fn frontend_ready_mark_ready_sets_ready() {
        let flag = FrontendReady::default();
        flag.mark_ready();
        assert!(
            flag.is_ready(),
            "§7.8.1: mark_ready makes the funnel emit app://intake instead of buffering"
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
