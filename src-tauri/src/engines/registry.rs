//! `crate::engines::registry` — the §3.2 registry seam (§0.7 physical tree: `engines/registry.rs`,
//! "Engine trait + selection"): the full §3.2.2 `Engine` trait (expanded at P4.1 from the P3.5 minimal
//! `plan()`-only shell — the SAME trait, never a second one) and its two-shape [`PlanOutcome`] return
//! (P3.5-authored with the trait, moved beside it). The §3.2.3 registry construction + `select()` static
//! lookup join this file at P4.4. Everything here is core-INTERNAL (no `serde`/`specta`) and re-exported
//! through `crate::engines`, so the §0.7 tier-2 logical path its consumers import is unchanged by the
//! physical file split. [Build-Session-Entscheidung: P4.1]

use std::path::Path;
use std::process::ExitStatus;

use crate::domain::{DroppedItem, TargetId};
use crate::outcome::ConversionErrorKind;

use super::{
    EngineCapability, EngineDescriptor, EngineId, Invocation, PatentDisposition, PlanError,
    Platform, ProbeOutput, TempPath,
};

/// A bundled conversion engine (§3.2.2) — one impl per engine binary/lib. The registry seam: §3.2.3
/// selection resolves a job's `(source, target)` pair to one `Engine`, and §1.7 calls `plan()` to get the
/// dispatch-ready [`Invocation`]. The full §3.2.2 surface (P4.1): `id` / `descriptor` / `capabilities` /
/// `plan` / `plan_encode` / `classify_failure`. There is deliberately **NO `progress_model()` method**
/// (§3.2.2 `[DECIDED]`): progress is a PER-INVOCATION property, not a per-engine constant — the same video
/// FFmpeg engine emits a `CoarseSpawnDone` probe `Invocation` and an `FfmpegKeyValue` encode `Invocation`,
/// which a single static method cannot express; the §1.7 dispatch reads `Invocation.progress` and §1.11
/// normalises that. `Send + Sync` because the §3.2.3 registry stores engines behind a shared handle and
/// §1.7 dispatches them across the §0.9 worker pool.
pub trait Engine: Send + Sync {
    /// Stable id for logging / SBOM rows / the §3.2.3 registry (§3.2.2) — the §0.6 discriminant
    /// (`ffmpeg`, `libreoffice`, `vips`, …).
    fn id(&self) -> EngineId;

    /// The §0.6 capability descriptor for this engine, incl. `serialised_only` and `kind` (§3.2.2). The
    /// §0.9 pool reads `descriptor().serialised_only` from a job's resolved [`EngineId`] BEFORE spawn to
    /// decide whether to also acquire the engine's single-permit semaphore (LibreOffice) — the concrete
    /// `EngineId → serialised_only` data path §0.9 depends on (P4.5 wires it; without it the pool cannot
    /// get the flag from the §3.2.3 `(SourceFmt,TargetFmt) → EngineId` registry). Pure, const-ish (a
    /// static fact per engine).
    fn descriptor(&self) -> EngineDescriptor;

    /// What this engine can do *on this platform*, given the §3.4 patent disposition resolved at build
    /// time (§3.2.2) — the rows the §3.2.3 registry is populated from, and the source of the honest
    /// per-platform "unavailable here" (§2.8 `PlatformUnavailable`, the §5.2 disable/omit set).
    fn capabilities(
        &self,
        platform: Platform,
        patents: &PatentDisposition,
    ) -> Vec<EngineCapability>;

    /// Build the concrete, dispatch-ready plan for one job — **Pure: no I/O, no spawn** (§3.2.2). It only
    /// *describes* the invocation (program / argv / cwd / env / stdin / progress); §1.7 owns the actual
    /// spawn / cancel / timeout and populates `out_tmp` at spawn time.
    ///
    /// **Params are the job's tier-3 projection (the 2026-07-07 plan-seam ruling):** the §0.6
    /// [`DroppedItem`] (detection + size) + [`TargetId`] + the effective read `input` path §1.7 hands in —
    /// NOT the tier-1 orchestrator-homed `ConversionJob` (§0.7: `crate::engines` is tier 2 and cannot
    /// reference it). `input` is the §2.3-resolved source (or the §3.5.0 core-staged scratch copy from P4
    /// on); argv embeds `input`, NEVER a path derived from `item`. `out_tmp` is BORROWED only so argv can
    /// embed its path — `plan()` constructs the returned [`Invocation`] with `out_tmp: None`; §1.7 owns
    /// the temp and populates `Some(temp)` on the ENCODE invocation after this call returns (a by-value
    /// param would be dropped — file deleted — by a probe engine's `plan()` before `plan_encode` needs
    /// it; the borrow is what lets ONE signature serve both shapes, §3.2.2).
    ///
    /// Returns [`PlanOutcome::Encode`] (single-step) or [`PlanOutcome::Probe`] (a probe-requiring
    /// engine's `ffprobe` sub-invocation — §3.2.1) — the shape §1.7 sequences on. A pure planning
    /// failure (an option value out of range, an unexpected target) is a [`PlanError`] carrying its
    /// §2.8 kind.
    fn plan(
        &self,
        item: &DroppedItem,
        target: TargetId,
        input: &Path,
        out_tmp: &TempPath,
    ) -> Result<PlanOutcome, PlanError>;

    /// Two-phase encode plan `[DECIDED §3.2.1]` (§3.2.2). Called by §1.7 ONLY for an engine whose
    /// `plan()` returned [`PlanOutcome::Probe`]: §1.7 runs the probe, parses its stdout into
    /// [`ProbeOutput`], then calls this to finalise the encode [`Invocation`] (constructed with
    /// `out_tmp: None` like every plan-time `Invocation`; §1.7 then populates `out_tmp = Some(temp)` —
    /// the ownership contract on `plan()` above). The progress denominator (`duration_us`) is taken FROM
    /// `probe` here — never mutated onto a previously-returned struct (§3.2.1). Pure (no I/O, no spawn).
    ///
    /// The default impl is the §3.2.2 single-step-engine seam: §1.7 only calls `plan_encode` after a
    /// `Probe`, so a single-step engine never reaches it — reaching it is a mis-sequenced lifecycle,
    /// answered with the spec's `InternalError` [`PlanError`] and the spec's detail string.
    fn plan_encode(
        &self,
        _item: &DroppedItem,
        _target: TargetId,
        _input: &Path,
        _out_tmp: &TempPath,
        _probe: &ProbeOutput,
    ) -> Result<Invocation, PlanError> {
        Err(PlanError {
            kind: ConversionErrorKind::InternalError,
            detail: "engine has no probe/encode two-phase plan".into(),
        })
    }

    /// Map this engine's exit code + stderr into the §2.8 error taxonomy (§3.2.2). Returns the §2.8-owned
    /// [`ConversionErrorKind`] — NOT a separate "FailureKind" (that name is dropped; §2.8 is the single
    /// owner of the failure-kind set). The wire `ErrorKind` (§0.4.3) is its projection at the §1.9
    /// boundary; the §06 drift check keeps the two byte-identical for ALL variants.
    fn classify_failure(&self, exit: ExitStatus, stderr: &str) -> ConversionErrorKind;
}

/// What `Engine::plan()` produced — the §3.2.1 two-shape return, named at the type level (the 2026-07-07
/// plan-seam ruling). The discriminator §1.7 sequences on: under the `out_tmp` ownership contract every
/// plan-time [`Invocation`] constructs `out_tmp: None`, so `out_tmp.is_some()` cannot mark the probe.
/// Probe-ness is per-JOB, not per-engine (the same FFmpeg engine encodes audio single-step and probes video),
/// so it is NOT an [`EngineDescriptor`] flag — the engine names the shape on the value it returns.
///
/// [Build-Session-Entscheidung: P3.5] SOLE author (§3.2.2 owns the shape; the P3.5 minimal-trait box). INTERNAL
/// — no `serde`/`specta` (it wraps the core-only [`Invocation`], never on the wire). Derives only `Debug`:
/// [`Invocation`] is itself `Debug`-only (it owns a live `TempPath`), so `PlanOutcome` is moved, never cloned.
/// §1.7 matches it EXHAUSTIVELY (no `_ =>` catch-all — the §1.2/G29 dispatch-enum discipline the crate-root
/// `clippy::wildcard_enum_match_arm` deny enforces). Moved beside the trait at P4.1 (the §0.7 file split).
#[derive(Debug)]
pub enum PlanOutcome {
    /// A single-step engine's encode plan (the native CSV/TSV engine, and every image/office/pdf pair from P4
    /// on): §1.7 populates `out_tmp = Some(temp)` and dispatches it directly; `plan_encode` is never called.
    Encode(Invocation),
    /// A probe-requiring engine's `ffprobe` sub-invocation (video FFmpeg, §3.2.1): `out_tmp` stays `None` for
    /// the whole probe leg (no publish artifact); §1.7 holds the temp, runs the probe, parses `ProbeOutput`,
    /// then calls `plan_encode`. No P3 engine produces it — the walking skeleton's one engine is single-step.
    Probe(Invocation),
}
