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
        reason = "the §0.6 lifecycle/DTO/result types homed here (Batch/ConversionJob/JobState P2.10, the C4/C5 DTOs P2.11, RunResult/ItemResult/Totals/CleanupResidue/ItemOutcome P2.12, the §0.4.2 ConversionEvent enum + its RunStarted/ItemStarted/ItemProgress/ItemFinished/BatchProgress payloads P2.37, and the four §0.4.4 State stores RunRegistry P2.42 + RunResultStore P2.43 + CollectedSetRegistry P2.44 + IngestRegistry P2.45) are authored as contracts before the P3.46 orchestrator behaviour + the C1/C2a/C3/C4/C5/C6/C7/C8/C13 + the C6 onProgress Channel<ConversionEvent> (P2.29) wire consumers construct/register/drive them, so their as-yet-unwired methods are dead in the production build until consumed (`RunRegistry::has_active_run` is already consumed by the §7.1.1 `converter_is_busy` from P2.55, with `register`/`cancel`/`finish` staying dead until P3.46). The P3.25 §2.6.4 cleanup-honesty leg (ResidueRecord/ResidueDisposition/residue_item_reason/split_residue_records/append_residue_tail) is likewise dead until the P3.50 §1.12 projection + the P3.38 write-sequence consume it. The P3.39 §2.5.1 EquivKeyComputer (compute_equiv_key) is likewise dead until the P3.40 C4 plan_output re-run wiring resolves its managed State + calls it."
    )
)]

use std::collections::hash_map::RandomState;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsString;
use std::hash::{BuildHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use specta::Type;
use tempfile::TempPath;
use tokio_util::sync::CancellationToken;
use walkdir::WalkDir;

use crate::domain::{
    CollectedSet, CollectedSetId, CollectingId, DestinationChoice, DestinationId, DetectionOutcome,
    DivertReason, DroppedItem, FrozenCollectedSet, IntakeOrigin, ItemId, ItemIdSpace, ItemPaths,
    ItemSpaceExhausted, JobStage, OptionValues, OutputPlan, ReadFailure, RerunPrompt, RunId,
    SkipReason, SkippedItem, Target, TargetId, UserFacingFormat,
};
use crate::fs_guard::{
    atomic_publish, is_write_divert_trigger, open_verified_parent_dir, output_name,
    publish_to_divert, resolve_divert_target, DivertTarget, FileIdentity, LocationCache,
    ParentDirVerdict, PublishError, PublishOutcome,
};
use crate::outcome::{ConversionErrorKind, IpcError, OutcomeMsg};
use crate::run::{cleanup_item, EquivKey, RunScratch};

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
/// DestinationChoice` still carries a raw `ChosenRoot(PathBuf)` until P3.80 re-keys it to a
/// `DestinationId` — the phased P3.76→P3.80 split; this box owns only the display projections.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DestinationResolved {
    /// The (now chosen) destination (§0.6 / §2.7). Re-keyed to `ChosenRoot(DestinationId)` at P3.80.
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
// `destination` (C5 owns it) while `OutputPlanPreview` carries only a `final_dir_display` PREVIEW and NO
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
    /// Per-item outcome + output→source mapping (§1.12). INCLUDES the freeze-time pre-flight `SkippedItem`s
    /// (`CollectedSet::Single.skipped`) projected as `ItemResult { item, output_display: None,
    /// state: Skipped(reason), reason: Some(OutcomeMsg::Skipped{ reason, .. }) }` — the skip rides the skip-shaped `OutcomeMsg`
    /// variant (§2.8), NOT `Failure`, so skip ≠ fail at the type level (§1.12); counted in `totals.skipped`.
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
    /// A pre-flight detection-ineligible item (§1.2/§1.3) — carries the §0.6 `SkipReason` (skip ≠ fail).
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
/// - `Succeeded` (case 1) and `Cancelled` (case 3): the item's terminal `state` already carries the meaning,
///   so the residue does NOT rewrite the per-item reason — it is surfaced via `cleanup_incomplete` (+ the tail)
///   alone. The two coincide in the reason override **by design, not by accident**: `state` (not the message)
///   is what distinguishes a kept success from a stopped cancel, and neither is a failure, so neither adopts
///   the §2.8.2 `CleanupResidue` *failure* string.
/// - `Failed` (case 2): the item is reported `Failed` WITH the combined §2.8.2 `CleanupResidue` message
///   ("This file couldn't be converted, and a temporary file may remain at {path}.") — never a clean success.
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

/// The per-item §2.8.2 `reason` OVERRIDE a residue imposes for the given §2.6.4 disposition, read from the
/// crate::outcome catalog (P3.68): the combined `CleanupResidue` message for `Failed` (never a clean
/// success — §2.6.4 case 2), and `None` for `Succeeded`/`Cancelled` (the terminal `state` carries the meaning;
/// the residue is surfaced via `cleanup_incomplete` + the batch tail). `residue_display` is the same §2.10.1
/// display string carried in the item's [`ResidueRecord::warning`], substituted into the row's `{path}` slot.
/// Because the §2.8.2 `CleanupResidue` row IS homed ([`crate::outcome::conversion_failure`] returns `Some` for
/// it), `Failed` always yields `Some`; the exhaustive match forces a fourth §2.6.4 case to decide its reason
/// explicitly. Panic-free. [Build-Session-Entscheidung: P3.25]
pub fn residue_item_reason(
    disposition: ResidueDisposition,
    residue_display: &str,
) -> Option<OutcomeMsg> {
    match disposition {
        ResidueDisposition::Failed => {
            crate::outcome::conversion_failure(ConversionErrorKind::CleanupResidue, residue_display)
        }
        ResidueDisposition::Succeeded | ResidueDisposition::Cancelled => None,
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

// ─── §2.1.1 per-item write sequence (P3.38) ──────────────────────────────────────────────────────────
// [Build-Session-Entscheidung: P3.38] Homed HERE in crate::orchestrator (tier 1) per the 2026-07-07 home
// ruling (§0.7 > the plan-cluster heading): the sequence COMPOSES `crate::run` (temp/cleanup) + `crate::fs_guard`
// (publish/divert) + the engine step — three tier-2 leaves + the engine seam — and ONLY the tier-1 orchestrator
// may compose all three (`fs_guard` depends DOWN only; `run`/`fs_guard` are mutually-independent siblings, so a
// deliberate `run`→`fs_guard` edge is rejected, §0.7). The §2.1.1 steps 3-6 (sync → resolve-late → exclusive
// publish → dir-fsync + the §2.14.3 EXDEV fallback + the §2.2.2 numbering ↔ no-clobber loop) all live INSIDE
// `fs_guard::atomic_publish` (P3.15/P3.16/P3.17); this box wires step 1 (`run::publish_temp`, P3.20), step 2 (the
// engine seam), the §1.7 exit-verification, and step 7 (`run::cleanup_item`, P3.22) around it.
//
// The engine step is a CALLABLE SEAM (a `FnOnce` writing into the temp) — the real native CSV/TSV engine binds
// at P3.41/P3.48, so `needs:` correctly excludes P3.41. Because the seam is honest-now (a caller supplies the
// bytes), the G32(a) source-unchanged leg binds with THIS box; the G31 output-validity structural readers bind
// when the real engine + corpus land (P3.41 → P3.62/P3.63), per the home ruling.
//
// The §2.7.2/§2.7.5 LATE-DIVERT is composed here (not merely surfaced): a §2.1.1 sequence that failed an item on
// a mid-write writability flip / FAT-exFAT `NoAtomicPublishSupport` instead of diverting would DEGRADE the §2.7.5
// "not a degraded path" guarantee (SSOT Principle-5). P3.17/P3.35/P3.36 authored `atomic_publish` /
// `resolve_divert_target` / `is_write_divert_trigger` / `publish_to_divert` with THIS box named as their
// production caller (P3.35/P3.36 are earlier `[x]` boxes, so no explicit `needs:` edge is required — the §04
// divert primitives are already built). One divert per item (§2.7.3) — a failed divert is terminal, never
// re-diverted. The whole surface is dead in the production build until the P3.46/P3.48 conductor calls
// `write_item` (the module-level `not(test)` dead_code expect covers it).

/// The terminal disposition of one §2.1.1 per-item write ([`write_item`]) — the output published (the real
/// path, retained core-side for `RunResultPaths.item_outputs` + the §1.12 display projection) or a named §2.8
/// failure (one item failed, the batch continues, §1.9). Core-INTERNAL (holds the real output `PathBuf`, never a
/// wire type); the §1.9 FSM (P3.46) projects it onto the wire `JobState`/`ItemOutcome`.
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

/// The full result of one §2.1.1 per-item write ([`write_item`]) — the terminal [`WriteDisposition`], whether the
/// output DIVERTED (§2.7.3, so the run's `divert_root_display` is set), and any §2.6.4 cleanup residue (a temp
/// that could not be removed, so the item is never reported as a silent clean success — the P3.25 honesty leg).
/// Core-INTERNAL (a [`ResidueRecord`] holds the off-wire real `PathBuf`).
///
/// [Build-Session-Entscheidung: P3.38] `Debug, Clone, PartialEq, Eq`; NOT `Copy` (embeds a `WriteDisposition` +
/// an `Option<ResidueRecord>`, both owning `PathBuf`s). The §1.9 FSM (P3.46) maps `(disposition, residue)` onto
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
}

impl WriteOutcome {
    /// A failure with no temp to reconcile (a pre-write step failed before any `.part` existed) — no residue.
    /// [Build-Session-Entscheidung: P3.38]
    fn failed(kind: ConversionErrorKind) -> Self {
        Self {
            disposition: WriteDisposition::Failed { kind },
            diverted: false,
            residue: None,
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

/// **§2.1.1 the per-item write sequence (P3.38)** — pick-temp → engine-writes → sync → resolve-late → publish →
/// dir-fsync → cleanup-on-error, with the §2.7.2/§2.7.5 late-divert. Consumes the §1.8 [`OutputPlan`] (P3.37) and
/// an engine-write SEAM (`engine_write`, a `FnOnce` the real native CSV/TSV engine replaces at P3.41/P3.48),
/// producing the terminal [`WriteOutcome`] the §1.9 FSM (P3.46) projects onto `JobState`/`ItemOutcome`.
///
/// The seven §2.1.1 steps: **(1)** pick the publish temp on `final`'s volume ([`RunScratch::publish_temp`], P3.20);
/// **(2)** the engine writes into `tmp` (the seam — never `final`, §3.5); **(3-6)** sync → resolve-late + §2.3.3
/// link-safety → the §2.2.2 numbering ↔ no-clobber exclusive publish → dir-fsync, all inside
/// [`atomic_publish`] (P3.15/P3.16, with the §2.14.3 EXDEV fallback P3.17); **(7)** on any error in 3-6 remove
/// `tmp` — `final` was never created ([`cleanup_item`], P3.22). The §1.7 EXIT-VERIFICATION gates step 3: success
/// ONLY if the temp output exists and is non-empty (a 0-byte output is a §2.8 `Empty` failure, never a clean
/// success of an empty file).
///
/// **Late-divert (§2.7.2/§2.7.5).** A `ResolvesOntoSource` parent (§2.3.3), a FAT/exFAT `NoAtomicPublishSupport`
/// (§2.7.2, Unix), or a writability publish failure ([`is_write_divert_trigger`] — USB pulled / share dropped /
/// permission flip) routes the completed `tmp` to the §2.7.3 divert target ([`resolve_divert_target`] →
/// [`publish_to_divert`]) — the FULL safety chain, not a degraded path (§2.7.5). ONE divert per item (§2.7.3): a
/// plan already diverted, or a failed divert, is terminal (§2.8 `WriteFailed`), never re-diverted.
///
/// No panic (the crate no-panic deny, G4/G14) — every failure is a structured [`WriteOutcome`]; the source bytes
/// are never touched (the no-harm G32(a) invariant the tests assert). [Build-Session-Entscheidung: P3.38]
#[allow(clippy::too_many_arguments)]
// [Build-Session-Entscheidung: P3.38] Each arg is a DISTINCT §2.1.1 input (the plan/source/frozen-set/divert
// roots/run handle/cache/probe-name factory/engine seam) — the `compute_output_plan` (P3.37) precedent; a
// mechanical bundle struct would group them without semantic value (and the two closures cannot live in one).
pub fn write_item(
    plan: &OutputPlan,
    source: &Path,
    frozen_sources: &[FileIdentity],
    divert_candidates: &[PathBuf],
    scratch: &RunScratch,
    cache: &mut LocationCache,
    probe_name: impl Fn() -> OsString,
    engine_write: impl FnOnce(&Path) -> Result<(), ConversionErrorKind>,
) -> WriteOutcome {
    let item = plan.job;
    // The target's canonical extension is ASCII (§04); a non-UTF-8 ext is an internal fault, never user-facing.
    let Some(ext) = plan.extension.to_str() else {
        return WriteOutcome::failed(ConversionErrorKind::InternalError);
    };
    let inputs = WriteInputs {
        plan,
        source,
        ext,
        frozen_sources,
        divert_candidates,
        scratch,
    };

    // §2.1.1 step 1: pick the publish temp (P3.20) — the run-owned `.convertia-…-.part` sibling on `final`'s
    // volume (§2.14.1). A create failure (permission / IO at the destination) fails the item clearly; nothing
    // was written.
    let Ok(tmp) = scratch.publish_temp(&plan.publish_temp_dir, item) else {
        return WriteOutcome::failed(ConversionErrorKind::WriteFailed);
    };

    // §2.1.1 step 2: the engine writes into `tmp` (the callable seam — never `final`, §3.5; the real native
    // CSV/TSV engine binds P3.41, dispatch wiring P3.48). On engine failure: §2.1.1 step 7 removes `tmp`
    // (`final` was never created).
    if let Err(kind) = engine_write(tmp.as_ref()) {
        return fail_cleanup(item, [tmp], kind);
    }

    // §1.7 exit & output verification: success ONLY if the temp output exists and is non-empty — a "success exit
    // but empty/zero output" is a §2.8 failure, never a clean success of an empty file.
    match std::fs::metadata(&*tmp) {
        Ok(meta) if meta.len() > 0 => {}
        // Present but 0-byte → §2.8 `Empty` (§1.7).
        Ok(_) => return fail_cleanup(item, [tmp], ConversionErrorKind::Empty),
        // Gone after a "successful" seam → the engine broke its non-empty-output contract → §2.13 InternalError.
        Err(_) => return fail_cleanup(item, [tmp], ConversionErrorKind::InternalError),
    }

    // §2.1.1 steps 3-6 (+ the §2.7.2/§2.7.5 late-divert).
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
                fail_cleanup(item, [tmp], map_publish_error(&err))
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
        // Clean both temps (§2.6.2) so a divert-volume leftover surfaces, never a silent drop.
        Err(err) => fail_cleanup(item, [tmp, intermediate], map_publish_error(&err)),
    }
}

/// Map a `crate::fs_guard` [`PublishError`] to its §2.8 [`ConversionErrorKind`] — the tier-1 boundary where the
/// leaf verdict becomes the wire taxonomy (§2.8; `crate::fs_guard` never depends up on `crate::outcome`). A
/// generic `Io` is a non-space destination write failure (§2.1/§2.7). [Build-Session-Entscheidung: P3.38]
fn map_publish_error(err: &PublishError) -> ConversionErrorKind {
    match err {
        PublishError::PathTooLong(_) => ConversionErrorKind::PathTooLong,
        PublishError::TooManyCollisions => ConversionErrorKind::TooManyCollisions,
        PublishError::OutOfDisk => ConversionErrorKind::OutOfDisk,
        PublishError::Io(_) => ConversionErrorKind::WriteFailed,
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
    WriteOutcome {
        disposition: WriteDisposition::Failed { kind },
        diverted: false,
        residue: cleanup_leftovers(item, temps),
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
/// back to C8, and [`paths`](RunResultStore::paths) serves the off-wire paths to C9 — each matched by
/// `RunId` so a stale/other run is never served for the wrong id. NO on-disk persistence (§7.4) — the store
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
    /// summary and C9 `paths(result.run_id)` resolves its `OpenTarget` against the real paths (P3.79).
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

    /// Re-serve the retained OFF-WIRE `RunResultPaths` for `run_id` (C9 `open_path`, §0.4.4 / §7.7.3, P3.79)
    /// — the real roots + per-item output/residue `PathBuf`s the wire `RunResult` shed (§2.10.1). Returns a
    /// clone iff a run is retained AND its `run_id` matches; `None` otherwise (→ the C9 §7.7.3 refusal). The
    /// paths are cloned out, so the guard is not held across the return. [Build-Session-Entscheidung: P3.76]
    pub fn paths(&self, run_id: RunId) -> Option<RunResultPaths> {
        self.lock()
            .as_ref()
            .filter(|retained| retained.result.run_id == run_id)
            .map(|retained| retained.paths.clone())
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

// ─── §0.4.4 picked-destination registry — the DestinationId → PathBuf session store (P3.76) ────────────
// [Build-Session-Entscheidung: P3.76] The FIFTH §0.4.4 orchestrator-State store — the session-scoped
// picked-roots registry the 2026-07-06 core-owned-paths ruling introduces so a C2b-picked destination PATH
// never crosses the wire: C2b mints a DestinationId, stores the Rust-picked folder here, and returns the id
// (+ a display string); C4/C5/C6 resolve DestinationChoice::ChosenRoot(id) core-side against it (§0.4.4).
// Homed here under the same §0.7 "(§0.4.4)" State umbrella as the four sibling stores (no §0.7/§1a
// structural edit — the P2.43/P2.44/P2.45 precedent). Unlike the SUPERSEDING CollectedSetRegistry, this
// ACCUMULATES: §0.4.4 "entries live for the app session (they survive across collected sets, so switching
// batches never forces a re-pick) and die at app exit; nothing is persisted (§7.4)". Like the sibling
// stores it is a CONTRACT before its consumer: the C2b register + the C4/C5/C6 resolve + the `.manage`
// registration are the P3.80 destination-legs box, so it is dead in the production build until then
// (covered by the module-level dead_code expect).

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
/// [Build-Session-Entscheidung: P3.76] `Default`-constructed empty; `Debug` for parity with the sibling
/// stores. NOT a wire type (no `serde`/`specta`) — pure core-internal State that never crosses IPC (the
/// wire carries only the `DestinationId` + the C2b display string, §0.6). The C2b register + C4/C5/C6
/// resolve WIRING is the P3.80 box, so it is dead in the production build until then (the module-level
/// dead_code expect covers it, the P2.44 `CollectedSetRegistry` precedent).
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

// ─── §2.5.1 EquivKey computation — the re-run equivalence key computer (P3.39) ────────────────────────────
// [Build-Session-Entscheidung: P3.39] The §2.5.1 EquivKey COMPUTATION homes here in the tier-1 orchestrator
// (the P3.38 prevention-sweep ruling): the key folds a `fs_guard::FileIdentity` + the §0.6 `TargetId` /
// `OptionValues`, and the orchestrator already holds all three (the frozen set carries the identities, C4/C6
// carry target + settings), so computing it here needs no `run`->`fs_guard` sibling edge (§0.7 forbids it).
// It hands only the finished hash DOWN to the tier-2 `crate::run` ledger as an opaque `EquivKey`. An
// app-managed singleton like the §0.4.4 stores above — it needs NO §0.7/§1a structural edit (§0.7 attributes
// the app-managed State to orchestrator; the P2.43/P2.44/P2.45/P2.58 precedent). Contract before consumer:
// the C4 `plan_output` path (P3.40) resolves the managed `State<EquivKeyComputer>` and calls
// `compute_equiv_key`, so it is dead in the production build until then (the module-level dead_code expect).

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferedLaunchIntake {
    /// The launch-time paths (already `parse_path_args`-classified, §7.8.1), accumulated across any repeat
    /// stash in the same not-ready window (no-loss — see [`PendingIntake::stash_or_route`]).
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

    /// Stash a launch set from the §7.8.1 `Buffer` arm (idle + WebView not-yet-ready) — UNLESS the frontend
    /// proved ready in the meantime, in which case the set is handed BACK for the `Emit` arm
    /// ([`StashOutcome::RouteToEmit`]). NO-LOSS on a repeat stash before the drain: a second launch event in
    /// the same not-ready window APPENDS its paths to the pending set rather than superseding it —
    /// superseding would drop the earlier launch's paths, the very loss this buffer exists to prevent
    /// (§7.8.1) — and keeps the FIRST origin (§7.8.1 "its stored origin"). The funnel only reaches this with
    /// a non-empty `paths` (it returns early on empty, §7.8.1), so no empty-stash guard is needed.
    /// [Build-Session-Entscheidung: P2.58]
    ///
    /// **The §7.8.1 no-loss closure (the stash-vs-drain interleaving, P2.137):** the funnel's ready-read
    /// (`intake_disposition`'s snapshot) and this stash are two steps, so the C1 drain
    /// ([`take_marking_ready`](PendingIntake::take_marking_ready)) can run BETWEEN them — mark-ready +
    /// take(`None`) — after which a plain stash would strand the set for the whole session (`FrontendReady`
    /// is monotonic and the frontend drains once per mount). Both critical sections therefore serialize on
    /// the SAME pending-slot `Mutex`, and this op RE-CHECKS the ready flag under that lock: every launch set
    /// is either stashed strictly before the drain's fused mark+take (so the drain observes it) or re-routed
    /// to a live `app://intake` emit. Proven by `state_stores::stash_after_drain_reroutes_to_emit` (+ the
    /// two-thread stress leg). [Build-Session-Entscheidung: P2.137]
    pub fn stash_or_route(
        &self,
        ready: &FrontendReady,
        paths: Vec<PathBuf>,
        origin: IntakeOrigin,
    ) -> StashOutcome {
        let mut slot = self.lock();
        if ready.is_ready() {
            return StashOutcome::RouteToEmit(BufferedLaunchIntake { paths, origin });
        }
        match slot.as_mut() {
            Some(buffered) => buffered.paths.extend(paths),
            None => *slot = Some(BufferedLaunchIntake { paths, origin }),
        }
        StashOutcome::Stashed
    }

    /// Mark the frontend ready AND take the buffered launch set — the C1 `drainPending` drain's two cohesive
    /// effects (P2.60, §7.8.1 "consumes `PendingIntake` exactly once") FUSED under the pending-slot `Mutex`
    /// so no [`stash_or_route`](PendingIntake::stash_or_route) can land between them (the §7.8.1 no-loss
    /// closure — see `stash_or_route`; P2.137). Mark-BEFORE-take inside the lock: a stash serialized after
    /// this section observes `ready == true` and re-routes to the `Emit` arm. Returns `None` when nothing is
    /// pending (the ordinary first launch with no files → C1 returns `CollectedSet::Empty`, §0.4.1).
    /// Idempotent: a second drain is `None`. [Build-Session-Entscheidung: P2.137]
    pub fn take_marking_ready(&self, ready: &FrontendReady) -> Option<BufferedLaunchIntake> {
        let mut slot = self.lock();
        ready.mark_ready();
        slot.take()
    }
}

/// The §7.8.1 `Buffer`-arm outcome of [`PendingIntake::stash_or_route`] — either the set is buffered for the
/// C1 drain replay, or the drain already ran (`FrontendReady` flipped between the funnel's disposition
/// snapshot and the stash) and the set is handed back so the caller emits `app://intake` instead: nothing is
/// ever stranded (§7.8.1 "a launch-with-files is never lost"). Core-internal (not a wire type — no
/// `serde`/`specta`). [Build-Session-Entscheidung: P2.137]
#[derive(Debug, PartialEq, Eq)]
pub enum StashOutcome {
    /// The set is buffered; the C1 `drainPending` drain will consume it (§7.8.1).
    Stashed,
    /// The drain already consumed the buffer and marked the frontend ready — the set is handed back for a
    /// live `app://intake` emit (the §7.8.1 no-loss re-route).
    RouteToEmit(BufferedLaunchIntake),
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
// methods are LIVE: `mark_ready` is driven by the C1 `drainPending` handler via the fused
// [`PendingIntake::take_marking_ready`] (P2.137; `crate::ipc::intake::resolve_intake_source` calls it —
// the drain call is the §7.8.1 root-shell-mount readiness signal), and `is_ready` is read by the §7.8.1
// funnel's `frontend_ready` (P2.59, main.rs) plus the stash's under-lock re-check (`stash_or_route`).

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
    // `FileIdentity`) + de-dup (P2.76) → freeze/materialise the immutable snapshot + assign
    // `ItemId` (P3.32 [`freeze_snapshot`] / P2.75) → group (P3.49). While those stages are unwired here the
    // frozen snapshot is empty, so the §1.3 projection of a no-eligible-source freeze is the §0.6
    // zero-collection `Empty` (§0.4.1 / §5.4).
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
// [Test-Change: P3.32 — old-obsolete+new-correct, §2.4.1] `expect`→`allow` (a production lint change, NOT a
// test suppression; cf. P3.7): P3.32's `freeze_snapshot` (below) now destructures this survivor row, so the
// P2.76 assertion that it is DEAD would error as unfulfilled — but `freeze_snapshot` is itself unwired until
// P3.49, so the row's dead-ness is ambiguous and `allow` (permissive) is the correct attribute.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "P2.76's §2.3.2 de-dup fold `dedup_by_identity` yields `DedupedMember<P>` (the id + \
                  retained FileIdentity + payload survivor row). Referenced by P3.32's `freeze_snapshot` \
                  (which resolves+de-dups, folds each survivor in, then projects it into a §0.6 DroppedItem / \
                  SkippedItem), still unwired until the P3.49 spine — so it is dead-at-runtime but no longer \
                  statically unused; the in-module dedup_tests construct it directly."
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
// [Test-Change: P3.7 — old-obsolete+new-correct, §2.4.1] `expect`→`allow` (a production lint change, NOT a
// test suppression; cf. P2.63): P3.7's `resolve_and_dedup` (below) now references this fold, so the P2.76
// assertion that it is DEAD would error as unfulfilled under -D warnings — but `resolve_and_dedup` is itself
// unwired until P3.49, so the fold's dead-ness is ambiguous and `allow` (permissive) is the correct attribute.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "The §2.4.1 freeze-spine step-3 resolved-identity de-dup fold (P2.76). Referenced by P3.7's \
                  `resolve_and_dedup` (still unwired until the P3.49 spine), so it is dead-at-runtime but no \
                  longer statically unused; the in-module dedup_tests exercise it directly."
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

/// The §2.4.1 freeze-spine step-3 output (P3.7) — the real-FS resolved-identity de-dup over walk candidate
/// PATHS. Carries the first-seen SURVIVORS (the P2.76 [`dedup_by_identity`] fold's rows, ids minted over the
/// threaded space) PLUS the `unresolved` per-item read failures (a candidate whose `resolve_identity` failed —
/// it vanished / became unreadable between the §1.1 walk and this resolve step). The `unresolved` rows are
/// §1.1 `Unreadable` [`WalkSkip`]s WITHOUT an `ItemId` — the P3.49 spine mints their ids from the same cursor
/// after the survivors (the P2.76 `&mut ItemIdSpace` contract) — so this step never silently drops a candidate
/// (§1.1: recorded, never dropped) and never lets a single vanished file sink the ingest.
// [Test-Change: P3.32 — old-obsolete+new-correct, §2.4.1] `expect`→`allow` (a production lint change, NOT a
// test suppression; cf. P3.7): P3.32's `freeze_snapshot` (below) now destructures this result (`survivors` /
// `unresolved`), so the P3.7 assertion that it is DEAD would error as unfulfilled — but `freeze_snapshot` is
// itself unwired until P3.49, so its dead-ness is ambiguous and `allow` (permissive) is correct.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "P3.7's real-FS resolve+de-dup step yields `ResolvedDedup<P>` (the first-seen survivors + the \
                  unresolved read-failure skips). Referenced by P3.32's `freeze_snapshot` (which materialises \
                  the §2.4.1 frozen snapshot from it), still unwired until the P3.49 spine — so it is \
                  dead-at-runtime but no longer statically unused; the in-module resolve_dedup tests construct \
                  it directly."
    )
)]
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
// [Test-Change: P3.32 — old-obsolete+new-correct, §2.4.1] `expect`→`allow` (a production lint change, NOT a
// test suppression; cf. P3.7): P3.32's `freeze_snapshot` (below) now CALLS this step, so the P3.7 assertion
// that it is DEAD would error as unfulfilled — but `freeze_snapshot` is itself unwired until P3.49, so its
// dead-ness is ambiguous and `allow` (permissive) is correct.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "P3.7 authors the §2.4.1 freeze-spine step-3 real-FS resolve+de-dup step. Called by P3.32's \
                  `freeze_snapshot` (the §2.4.1 freeze-point that materialises the frozen snapshot from its \
                  survivors), still unwired until the P3.49 spine — so it is dead-at-runtime but no longer \
                  statically unused; the in-module resolve_dedup tests exercise it directly."
    )
)]
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "P3.32 authors the §2.4.1 frozen snapshot (the eager immutable Vec<DroppedItem> \
                  materialisation). Its production reader is the `ingest` funnel's spine, wired at P3.49 (the \
                  CSV→TSV walking skeleton — which projects it through §1.3 group() into the wire CollectedSet); \
                  its fields are read only by the in-module freeze_tests until then, so it is dead in the \
                  production build — the same interface-shell attribute IntakeWalk (P2.66) / ResolvedDedup (P3.7) \
                  carry."
    )
)]
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
///    P2.76 single-space contract), the OFF-WIRE `item_paths` pair for every id, and the retained `roots`.
///
/// Fallible only on `ItemSpaceExhausted` (`?`-propagated, never a panic — G4/G14; the §1.10 bounds cap a real
/// frozen set far below `2^32`); the P3.49 spine maps it to the §1.1 fatal-ingest surface. The lossy §2.10.1
/// `display_name` (basename) / `source_display` (path) projections are produced here from `raw_path`; the §2.7
/// `rel_path_display` + the `size_bytes` are carried through from the candidate (see [`DetectedCandidate`]).
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "P3.32 authors the §2.4.1 freeze-point primitive. Its production caller is the `ingest` \
                  funnel's spine, wired at P3.49 (the CSV→TSV walking skeleton); dead in the production build \
                  pending that wiring, exercised by the in-module freeze_tests below — the same interface-shell \
                  attribute walk_intake_roots (P2.64) / resolve_and_dedup (P3.7) carry."
    )
)]
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
        // §0.4.4 / §2.10.1 off-wire path pair: `raw` = the as-dropped path, `resolved` = the §2.3 canonical
        // identity (the §1.7 engine target). Keyed by the item's id over the single space so BOTH views resolve.
        item_paths.insert(
            id,
            ItemPaths {
                raw_path: raw_path.clone(),
                resolved_path: identity.canonical_path,
            },
        );
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
            reason,
        });
    }

    Ok(FrozenSnapshot {
        items,
        skipped,
        item_paths,
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
            "§2.3: resolved_path = the canonical identity path (the §1.7 engine target)"
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
            item_paths: BTreeMap::new(),
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
                r#""commonRootDisplay":"/src","divertRootDisplay":"/Downloads"}"#
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
                        ConversionJob {
                            item: item_id(id),
                            source: dropped_item_with(id, batch_format),
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
                // [Test-Change: P2.137 — old-obsolete+new-correct, §0.6] teeth: plant a format-intruder
                // job and prove the grouping key DETECTS it (the old fixture-only generator could not
                // falsify the format equality; the still-true shape assertions above are retained verbatim).
                let intruder_id = u32::try_from(n).expect("n < 64 fits u32");
                batch.jobs.push(ConversionJob {
                    item: item_id(intruder_id),
                    source: dropped_item_with(intruder_id, intruder_format),
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

    // §6.4.1 unit (G15): the §0.4.4 OFF-WIRE `RunResultPaths` re-serve (P3.76) — `retain` stores the real
    // paths alongside the wire result; `paths` re-serves them for the matching `RunId` (the C9 `OpenTarget`
    // resolution source, P3.79), and mismatched/empty is `None` (the C9 §7.7.3 refusal). This is the off-wire
    // half of the retention contract the display-only wire `RunResult` depends on (§2.10.1).
    #[test]
    fn run_result_store_paths_re_serves_the_off_wire_paths_for_matching_id() {
        let store = RunResultStore::default();
        store.retain(sample_run_result(run_id()), sample_run_paths());
        assert_eq!(
            store.paths(run_id()),
            Some(sample_run_paths()),
            "§0.4.4/§7.7.3: the off-wire RunResultPaths is re-served to C9 for its own RunId"
        );
        assert_eq!(
            store.paths(run_id_other()),
            None,
            "§0.4.4: the off-wire paths are NEVER served for a different run's id (the RunId match guards it)"
        );
        store.evict();
        assert_eq!(
            store.paths(run_id()),
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
            item_paths: BTreeMap::new(),
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

    // [Test-Change: P2.137 — old-obsolete+new-correct, §7.8.1] The P2.58 `stash`/`take` pair was fused into
    // `stash_or_route`/`take_marking_ready` (the §7.8.1 no-loss closure: both critical sections share the
    // pending-slot Mutex and the stash re-checks readiness under it) — the P2.58 assertions below are the
    // SAME contracts driven through the fused API (a not-ready stash routes to `Stashed`; the drain fuses
    // mark-ready + take), verified against §7.8.1's consume-once + stored-origin + no-loss prose.

    // §6.4.1 unit (G15): the §7.8.1 stash→drain round-trip — a buffered launch set is taken back with its
    // paths + the stored origin, and the slot is cleared (the C1 drainPending consume-once, §7.8.1); the
    // drain marks the frontend ready in the same fused step (P2.137).
    #[test]
    fn pending_intake_stash_then_take_returns_the_set_and_clears() {
        let buf = PendingIntake::default();
        // [Test-Change: P2.137 — old-obsolete+new-correct, §7.8.1] (fused-API rewrite; see block header)
        let ready = FrontendReady::default();
        assert_eq!(
            buf.stash_or_route(&ready, paths(&["a.png", "b.jpg"]), IntakeOrigin::LaunchArg),
            StashOutcome::Stashed,
            "§7.8.1: a not-ready stash buffers (the Buffer arm's normal case)"
        );
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

    // §6.4.1 unit (G15): NO-LOSS on a repeat stash before the drain — a second launch event in the same
    // not-ready window APPENDS its paths (never supersedes, which would drop the earlier launch's paths) and
    // keeps the FIRST origin (§7.8.1 "its stored origin"). This is the property the path-loss-avoidance the
    // owner-confirmed P2.58-before-P2.55 order rests on (every reachable launch set is preserved).
    #[test]
    fn pending_intake_repeat_stash_accumulates_paths_keeps_first_origin() {
        let buf = PendingIntake::default();
        let ready = FrontendReady::default();
        buf.stash_or_route(&ready, paths(&["first.png"]), IntakeOrigin::LaunchArg);
        buf.stash_or_route(
            &ready,
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

    // §6.4.1 unit (G15): the §7.8.1 no-loss closure, deterministic interleaving (P2.137) — the exact TOCTOU
    // the fused API exists to close: the funnel snapshots ready=false (its `intake_disposition` read), the C1
    // drain then runs its fused mark-ready+take (finding nothing), and only THEN does the funnel's Buffer arm
    // reach the stash. A plain stash would strand the set for the session (ready is monotonic, the drain
    // fires once per mount); `stash_or_route` re-checks readiness under the pending-slot lock and hands the
    // set BACK for a live emit instead.
    #[test]
    fn stash_after_drain_reroutes_to_emit() {
        let buf = PendingIntake::default();
        let ready = FrontendReady::default();
        // The funnel's disposition snapshot: idle + not-ready → it WILL pick the Buffer arm.
        assert!(
            !ready.is_ready(),
            "precondition: the snapshot read not-ready"
        );
        // The drain interleaves before the stash lands: fused mark-ready + take (nothing pending yet).
        assert!(buf.take_marking_ready(&ready).is_none());
        // The funnel's stale Buffer arm now stashes — and MUST be re-routed, not stranded.
        let outcome = buf.stash_or_route(&ready, paths(&["late.png"]), IntakeOrigin::LaunchArg);
        assert!(
            matches!(outcome, StashOutcome::RouteToEmit(_)),
            "§7.8.1/P2.137: a stash after the drain must RE-ROUTE to Emit — a plain Stashed here is the \
             stranded-set loss the fused API exists to prevent"
        );
        let StashOutcome::RouteToEmit(set) = outcome else {
            return; // proven RouteToEmit by the assert above
        };
        assert_eq!(
            set.paths,
            paths(&["late.png"]),
            "§7.8.1/P2.137: the re-routed set carries the full stale-stash payload"
        );
        assert_eq!(set.origin, IntakeOrigin::LaunchArg);
        assert!(
            buf.take_marking_ready(&ready).is_none(),
            "§7.8.1: nothing may remain buffered after the re-route (the set went to the Emit arm)"
        );
    }

    // §6.4.2 stress leg (G15; bounded, deterministic INVARIANT — not timing-dependent asserts): under a real
    // two-thread race between the Buffer-arm stash and the C1 drain, every outcome pair satisfies the §7.8.1
    // no-loss invariant — `Stashed` implies the drain observed the set (its take returned it), and a drain
    // that took nothing implies the stash re-routed to Emit. No interleaving may strand a set.
    #[test]
    fn stash_vs_drain_race_never_strands_a_set() {
        for _ in 0..100 {
            let buf = std::sync::Arc::new(PendingIntake::default());
            let ready = std::sync::Arc::new(FrontendReady::default());
            let (b1, r1) = (std::sync::Arc::clone(&buf), std::sync::Arc::clone(&ready));
            let stasher = std::thread::spawn(move || {
                b1.stash_or_route(
                    &r1,
                    vec![PathBuf::from("race.png")],
                    IntakeOrigin::LaunchArg,
                )
            });
            let (b2, r2) = (std::sync::Arc::clone(&buf), std::sync::Arc::clone(&ready));
            let drainer = std::thread::spawn(move || b2.take_marking_ready(&r2));
            let stash_outcome = stasher
                .join()
                .expect("§7.8.1/P2.137: the stasher thread must not panic");
            let drained = drainer
                .join()
                .expect("§7.8.1/P2.137: the drainer thread must not panic");
            // The residue a LATE drain (serialized after the stash) would still find:
            let residue = buf.take_marking_ready(&ready);
            let observed = drained.is_some() || residue.is_some();
            match stash_outcome {
                StashOutcome::Stashed => assert!(
                    observed,
                    "§7.8.1/P2.137 no-loss: a Stashed set must be observable by a drain (never stranded)"
                ),
                StashOutcome::RouteToEmit(set) => {
                    assert_eq!(set.paths, vec![PathBuf::from("race.png")]);
                    assert!(
                        drained.is_none() && residue.is_none(),
                        "§7.8.1/P2.137: a re-routed set is emitted, never ALSO buffered (no duplicate)"
                    );
                }
            }
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

    // §6.4.1 (G15): the §2.6.4 THREE-CASE honesty — only `Failed` (case 2) rewrites the per-item reason to
    // the combined §2.8.2 `CleanupResidue` message ("never a clean success"); `Succeeded` (case 1) and
    // `Cancelled` (case 3) impose NO reason override (the terminal state carries the meaning; the residue rides
    // `cleanup_incomplete` + the tail). The machine-checkable "never a silent clean success" guard.
    #[test]
    fn residue_item_reason_rewrites_only_the_failure_case() {
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

        assert_eq!(
            residue_item_reason(ResidueDisposition::Succeeded, display),
            None,
            "§2.6.4 case 1: a success-with-residue item keeps its Succeeded state — no failure reason override"
        );
        assert_eq!(
            residue_item_reason(ResidueDisposition::Cancelled, display),
            None,
            "§2.6.4 case 3: a cancelled-with-residue item stays Cancelled — no failure reason override"
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
    //! §6.4.1 unit + real-FS integration (G15/G32(a)) for the P3.38 §2.1.1 per-item write sequence — the
    //! composition of `crate::run` (temp/cleanup) + `crate::fs_guard` (publish/divert) + an engine-write SEAM.
    //! The engine is a caller-supplied `FnOnce` (the real native CSV/TSV engine binds P3.41/P3.48), so these
    //! run against a REAL temp filesystem (test-strategy §0.1 — never mock the FS under test); the G31
    //! output-VALIDITY structural readers bind with the real engine + corpus (P3.62/P3.63). Here we pin: the
    //! atomic beside-source publish + the no-harm G32(a) source-unchanged invariant, no-clobber numbering, the
    //! §1.7 exit-verification (empty / vanished output), step-7 cleanup-on-error, the §2.7.2/§2.7.5 late-divert
    //! wiring, and the one-divert-per-item rule.
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

        let out = write_item(
            &plan,
            &f.source,
            &f.frozen,
            &[],
            &f.scratch,
            &mut f.cache,
            probe(),
            |tmp: &Path| {
                std::fs::write(tmp, b"a\tb\n1\t2\n").map_err(|_| ConversionErrorKind::WriteFailed)
            },
        );

        // §2.1: published at the beside-source name `data.tsv`, carrying the seam's bytes.
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

        let out = write_item(
            &plan,
            &f.source,
            &f.frozen,
            &[],
            &f.scratch,
            &mut f.cache,
            probe(),
            |tmp: &Path| {
                std::fs::write(tmp, b"fresh").map_err(|_| ConversionErrorKind::WriteFailed)
            },
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
    fn an_engine_error_fails_the_item_removes_the_temp_and_never_creates_final() {
        let mut f = Fixture::new(b"src\n");
        let before = std::fs::read(&f.source).expect("read source before");
        let plan = f.plan_in(f.dest.path());

        let out = write_item(
            &plan,
            &f.source,
            &f.frozen,
            &[],
            &f.scratch,
            &mut f.cache,
            probe(),
            |_tmp: &Path| Err(ConversionErrorKind::Corrupt),
        );

        // §2.1.1 step 7: the item fails with the engine's kind; `final` was never created.
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
            "no `final` on an engine failure"
        );
        assert!(
            part_files(f.dest.path()).is_empty(),
            "the temp is removed on failure, found {:?}",
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
        let mut f = Fixture::new(b"src\n");
        let plan = f.plan_in(f.dest.path());

        // The seam "succeeds" but writes nothing — the §1.7 exit-verification rejects the empty output.
        let out = write_item(
            &plan,
            &f.source,
            &f.frozen,
            &[],
            &f.scratch,
            &mut f.cache,
            probe(),
            |_tmp: &Path| Ok(()),
        );

        assert_eq!(
            out.disposition,
            WriteDisposition::Failed {
                kind: ConversionErrorKind::Empty
            },
            "§1.7: a success exit with zero output is a §2.8 Empty failure, never a clean success"
        );
        assert!(
            !f.dest.path().join("data.tsv").exists(),
            "no `final` for an empty output"
        );
    }

    #[test]
    fn a_vanished_output_after_a_successful_seam_is_an_internal_error() {
        let mut f = Fixture::new(b"src\n");
        let plan = f.plan_in(f.dest.path());

        // The seam removes its own temp then reports success — an internal contract violation (§1.7).
        let out = write_item(
            &plan,
            &f.source,
            &f.frozen,
            &[],
            &f.scratch,
            &mut f.cache,
            probe(),
            |tmp: &Path| {
                std::fs::remove_file(tmp).expect("remove the temp mid-seam");
                Ok(())
            },
        );

        assert_eq!(
            out.disposition,
            WriteDisposition::Failed {
                kind: ConversionErrorKind::InternalError
            },
            "§1.7: a vanished output after a 'successful' seam is an internal fault"
        );
    }

    #[test]
    fn a_missing_publish_temp_dir_fails_write_failed_before_the_engine_runs() {
        let mut f = Fixture::new(b"src\n");
        // `publish_temp_dir` points at a non-existent directory → step 1 cannot mint the temp.
        let plan = OutputPlan {
            job: ItemId::from_index(0),
            final_dir: f.dest.path().to_path_buf(),
            diverted: None,
            base_name: OsString::from("data"),
            extension: OsString::from("tsv"),
            publish_temp_dir: f.dest.path().join("does-not-exist"),
        };
        let mut ran = false;

        let out = write_item(
            &plan,
            &f.source,
            &f.frozen,
            &[],
            &f.scratch,
            &mut f.cache,
            probe(),
            |_tmp: &Path| {
                ran = true;
                Ok(())
            },
        );

        assert_eq!(
            out.disposition,
            WriteDisposition::Failed {
                kind: ConversionErrorKind::WriteFailed
            }
        );
        assert!(
            !ran,
            "the engine seam never runs when the publish temp cannot be created"
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

        let out = write_item(
            &plan,
            &f.source,
            &f.frozen,
            &[],
            &f.scratch,
            &mut f.cache,
            probe(),
            |tmp: &Path| std::fs::write(tmp, b"out").map_err(|_| ConversionErrorKind::WriteFailed),
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
        // The tier-1 leaf-verdict -> §2.8 boundary (the hard-to-drive publish errors, covered directly).
        assert_eq!(
            map_publish_error(&PublishError::PathTooLong(PathTooLong::Total)),
            ConversionErrorKind::PathTooLong
        );
        assert_eq!(
            map_publish_error(&PublishError::TooManyCollisions),
            ConversionErrorKind::TooManyCollisions
        );
        assert_eq!(
            map_publish_error(&PublishError::OutOfDisk),
            ConversionErrorKind::OutOfDisk
        );
        assert_eq!(
            map_publish_error(&PublishError::Io(std::io::Error::other("write failed"))),
            ConversionErrorKind::WriteFailed
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

        let out = write_item(
            &plan,
            &f.source,
            &frozen,
            &[divert.path().to_path_buf()],
            &f.scratch,
            &mut f.cache,
            probe(),
            |tmp: &Path| {
                std::fs::write(tmp, b"a\tb\n").map_err(|_| ConversionErrorKind::WriteFailed)
            },
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

        let out = write_item(
            &plan,
            &f.source,
            &f.frozen,
            &[divert.path().to_path_buf()],
            &f.scratch,
            &mut f.cache,
            probe(),
            |tmp: &Path| {
                std::fs::write(tmp, b"a\tb\n").map_err(|_| ConversionErrorKind::WriteFailed)
            },
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

        let out = write_item(
            &plan,
            &f.source,
            &f.frozen,
            &[divert.path().to_path_buf()],
            &f.scratch,
            &mut f.cache,
            probe(),
            |tmp: &Path| {
                std::fs::write(tmp, b"a\tb\n").map_err(|_| ConversionErrorKind::WriteFailed)
            },
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

        // No divert candidates at all -> resolve_divert_target yields Unavailable -> §2.8 WriteFailed.
        let out = write_item(
            &plan,
            &f.source,
            &f.frozen,
            &[],
            &f.scratch,
            &mut f.cache,
            probe(),
            |tmp: &Path| {
                std::fs::write(tmp, b"a\tb\n").map_err(|_| ConversionErrorKind::WriteFailed)
            },
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

        let out = write_item(
            &plan,
            &f.source,
            &f.frozen,
            &[],
            &f.scratch,
            &mut f.cache,
            probe(),
            |tmp: &Path| std::fs::write(tmp, b"out").map_err(|_| ConversionErrorKind::WriteFailed),
        );

        assert_eq!(
            out.disposition,
            WriteDisposition::Failed {
                kind: ConversionErrorKind::InternalError
            },
            "a non-UTF-8 target extension is an internal fault (never a user-facing case)"
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
    use crate::run::RerunLedger;
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
