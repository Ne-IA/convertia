//! `crate::orchestrator` — the §1.9 batch / job-lifecycle conductor: it builds the queue, drives
//! `JobState`, holds the run registry + cancellation tokens (§0.4.4), and fans progress out to the
//! Channel. It sequences the guarantees / engines / detection layers; it owns none of their behaviour.
//!
//! The conducting BEHAVIOUR (queue construction at C6, the §1.9 transitions, the run registry +
//! cancellation) is filled by P3.46. This module homes the §0.6 outcome-referencing lifecycle/result types
//! it assembles — `Batch`/`ConversionJob`/`JobState` (P2.10), the C4/C5 command-return DTOs
//! `PreflightVerdict`/`OutputPlanPreview`/`DestinationResolved` (P2.11), the §1.12 result types
//! `RunResult`/`ItemResult`/`Totals`/`CleanupResidue`/`ItemOutcome` (P2.12), and the §0.4.2 `ConversionEvent`
//! run-telemetry enum + its payloads `RunStarted`/`ItemStarted`/`ItemProgress`/`ItemFinished`/`BatchProgress`
//! (P2.37) — at tier 1, ABOVE the tier-3
//! `crate::domain` leaf, because each references `crate::outcome` (the §2.8 kind / `OutcomeMsg` / `IpcError`)
//! directly or transitively. Homing them here keeps the §0.6 `domain` ↔ `outcome` type cycle broken and
//! `crate::domain` a pure leaf (the §0.7 ‡ note, the owner-decided P2.10 tier-finalisation). The sibling
//! `JobStage` (no outcome ref) stays in `crate::domain`.

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
        reason = "the §0.6 lifecycle/DTO/result types homed here (Batch/ConversionJob/JobState P2.10, the C4/C5 DTOs P2.11, RunResult/ItemResult/Totals/CleanupResidue/ItemOutcome P2.12, the §0.4.2 ConversionEvent enum + its RunStarted/ItemStarted/ItemProgress/ItemFinished/BatchProgress payloads P2.37) are authored as contracts before the P3.46 orchestrator behaviour + the C4/C5/C8 + the C6 onProgress Channel<ConversionEvent> (P2.29) wire consumers construct/register them, so they are dead in the production build until consumed."
    )
)]

use std::path::PathBuf;

use serde::Serialize;
use specta::Type;

use crate::domain::{
    CollectedSetId, DestinationChoice, DivertReason, DroppedItem, ItemId, JobStage, OptionValues,
    OutputPlan, RerunPrompt, RunId, SkipReason, Target, TargetId, UserFacingFormat,
};
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        Availability, Confidence, DetectionOutcome, DivertReason, RerunPrompt, TargetId,
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
}
