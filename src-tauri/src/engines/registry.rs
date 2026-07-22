//! `crate::engines::registry` — the §3.2 registry seam (§0.7 physical tree: `engines/registry.rs`,
//! "Engine trait + selection"): the full §3.2.2 `Engine` trait (expanded at P4.1 from the P3.5 minimal
//! `plan()`-only shell — the SAME trait, never a second one) and its two-shape [`PlanOutcome`] return
//! (P3.5-authored with the trait, moved beside it). The §3.2.3 registry construction + `select()` static
//! lookup join this file at P4.4. Everything here is core-INTERNAL (no `serde`/`specta`) and re-exported
//! through `crate::engines`, so the §0.7 tier-2 logical path its consumers import is unchanged by the
//! physical file split. [Build-Session-Entscheidung: P4.1]

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::Path;
use std::process::ExitStatus;
use std::sync::{Arc, LazyLock};

use crate::domain::{DroppedItem, TargetId};
use crate::outcome::ConversionErrorKind;

use super::{
    current_platform, CodecPosture, Direction, EngineCapability, EngineDescriptor, EngineId,
    Invocation, NativeCsvTsvEngine, PatentDisposition, PlanError, Platform, ProbeOutput, SourceFmt,
    TargetFmt, TempPath,
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

// ─── §3.2.3 the single-owner pair registry + select() (P4.4) ──

/// The §3.2.3 engine registry — the startup-built static lookup `(SourceFmt, TargetFmt) → EngineId`
/// (selection is a **lookup, not a search**: the §04 files pre-assigned exactly one owner per pair) plus
/// the registered engines behind shared handles (§3.2.2's "registry of capability-declaring engines
/// behind one trait"). Built ONCE from every registered engine's `capabilities(platform, patents)` rows —
/// each engine already filters its rows by the resolved §3.4 disposition (§3.2.2), so the map IS the
/// platform-resolved view and a missing key is the §3.4 honest gap (§3.2.3's single legitimate `None`,
/// surfaced by the caller as the §2.8 `PlatformUnavailable`). **NO fallback engine chain** (§3.2.3):
/// single owner per pair, enforced fail-closed at build ([`RegistryBuildError`]).
///
/// [Derived-Assumption: P4.4 — §3.2.3's pseudo-signature `select(src, tgt, plat)` carries the platform,
/// but the registry is BUILT for the one running platform (`current_platform()`, resolved once at startup
/// together with the §3.4 disposition and consumed by the `capabilities()` filter), so the `plat` param
/// is absorbed by construction and `select` takes the pair alone; §3.2.3's select-time
/// `available_on(plat, patents)` filter is realised as that build-time capability filter — the same
/// predicate, applied once instead of per-lookup.]
/// [Build-Session-Entscheidung: P4.4] INTERNAL — core-side only, no `serde`/`specta`; one instance
/// behind the [`engine_registry`] static, never cloned.
pub struct EngineRegistry {
    /// The §3.2.3 pair lookup — single owner per `(source, target)` key.
    pairs: HashMap<(SourceFmt, TargetFmt), EngineId>,
    /// The registered engines by id — the shared handles a `select()` winner is resolved through.
    engines: HashMap<EngineId, Arc<dyn Engine>>,
}

/// Manual `Debug` — `dyn Engine` carries no `Debug` bound (the trait is a behaviour seam, §3.2.2), so
/// the registered engines render as their ids; the pair map renders in full. [Build-Session-Entscheidung: P4.4]
impl std::fmt::Debug for EngineRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineRegistry")
            .field("pairs", &self.pairs)
            .field("engines", &self.engines.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// A §3.2.3 registry-BUILD failure — a §3.2.1 invariant violated by the registered capability rows.
/// A mis-declared engine set is a build-time programming fault, never a user fault: the conductor
/// surfaces it as one item's `InternalError` (no panic — the crate no-panic policy).
/// [Build-Session-Entscheidung: P4.4] INTERNAL — `Debug, Clone, PartialEq, Eq`, no `serde`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryBuildError {
    /// Two capability rows resolved the same `(source, target)` cell — §3.2.1 single-engine-per-pair
    /// violated. The build FAILS whole (fail-closed): no silent first-wins masking of the second owner.
    DuplicatePair {
        /// The cell's source format.
        source: SourceFmt,
        /// The cell's target.
        target: TargetFmt,
        /// The engine that had already claimed the cell.
        first: EngineId,
        /// The engine whose row collided with it.
        second: EngineId,
    },
    /// A `Direction::Both` row carries a non-`Format` target: the reversed ordering is inexpressible (a
    /// §0.6 `TargetId::Op` has no source-side form), so the declared arrow cannot mean "both ways".
    UnflippableBothRow {
        /// The engine that declared the row.
        engine: EngineId,
        /// The non-reversible target it carried.
        target: TargetFmt,
    },
}

impl EngineRegistry {
    /// Build the §3.2.3 registry from the registered engine set for ONE resolved `(platform, patents)`
    /// pair — each engine's `capabilities()` filters its rows by that disposition (§3.2.2), and a
    /// `Direction::Both` row registers **both** orderings (the one declared arrow covers the §04 matrix's
    /// two cells — the P4.1 Derived-Assumption on the native engine's `CSV ↔ TSV` row, realised here).
    /// Fails closed on a §3.2.1 violation — an invalid set never becomes a half-usable registry.
    pub fn build(
        engines: Vec<Arc<dyn Engine>>,
        platform: Platform,
        patents: &PatentDisposition,
    ) -> Result<Self, RegistryBuildError> {
        let mut pairs: HashMap<(SourceFmt, TargetFmt), EngineId> = HashMap::new();
        let mut by_id: HashMap<EngineId, Arc<dyn Engine>> = HashMap::new();
        for engine in engines {
            let id = engine.id();
            for cell in engine.capabilities(platform, patents) {
                insert_single_owner(&mut pairs, cell.source, cell.target, id)?;
                if cell.direction == Direction::Both {
                    let TargetId::Format(reverse_source) = cell.target else {
                        return Err(RegistryBuildError::UnflippableBothRow {
                            engine: id,
                            target: cell.target,
                        });
                    };
                    insert_single_owner(
                        &mut pairs,
                        reverse_source,
                        TargetId::Format(cell.source),
                        id,
                    )?;
                }
            }
            by_id.insert(id, engine);
        }
        Ok(EngineRegistry {
            pairs,
            engines: by_id,
        })
    }

    /// The §3.2.3 static lookup — `Some(owner)` for a registered pair; `None` is the §3.4 honest gap
    /// (the caller surfaces it as the §2.8 `PlatformUnavailable`), the **single legitimate miss** — an
    /// in-scope license-clean pair always resolves (§3.2.3). No fallback chain.
    #[must_use]
    pub fn select(&self, source: SourceFmt, target: TargetFmt) -> Option<EngineId> {
        self.pairs.get(&(source, target)).copied()
    }

    /// Resolve a selected [`EngineId`] to its registered engine — the shared handle §1.7 calls `plan()`
    /// on. `None` for an id no registered engine carries (a caller inconsistency: `select()` only returns
    /// registered ids), answered honestly rather than panicking (the crate no-panic policy).
    #[must_use]
    pub fn engine(&self, id: EngineId) -> Option<&dyn Engine> {
        self.engines.get(&id).map(Arc::as_ref)
    }
}

/// Insert one `(source, target) → owner` cell, failing closed on a second owner — §3.2.1
/// single-engine-per-pair, with no first-wins masking.
fn insert_single_owner(
    pairs: &mut HashMap<(SourceFmt, TargetFmt), EngineId>,
    source: SourceFmt,
    target: TargetFmt,
    owner: EngineId,
) -> Result<(), RegistryBuildError> {
    match pairs.entry((source, target)) {
        Entry::Occupied(existing) => Err(RegistryBuildError::DuplicatePair {
            source,
            target,
            first: *existing.get(),
            second: owner,
        }),
        Entry::Vacant(slot) => {
            slot.insert(owner);
            Ok(())
        }
    }
}

/// The v1 registered-engine set the startup registry is built from — the walking skeleton registers the
/// native CSV/TSV engine alone; the P5–P7 engine adapters (image-worker, FFmpeg, LibreOffice, poppler,
/// pandoc) join this list at their staging boxes. [Build-Session-Entscheidung: P4.4]
fn registered_engines() -> Vec<Arc<dyn Engine>> {
    vec![Arc::new(NativeCsvTsvEngine)]
}

/// The startup-resolved §3.4 patent disposition. The HONEST current value is all-`Available`: every
/// §3.4-encumbered codec (HEVC / AAC / H.264) belongs to engine adapters that join the registry in P5–P7,
/// so today's registered set declares no encumbered capability row for a posture to gate — `Available` is
/// vacuously exact, not a stub. P4.40 (§3.4.4a) replaces this resolver with the `engines.lock` parse→map
/// flow, built before any `capabilities()` call and passed in — that box owns the re-cut.
/// [Build-Session-Entscheidung: P4.4]
fn resolved_patent_disposition() -> PatentDisposition {
    PatentDisposition {
        heic_hevc: CodecPosture::Available,
        aac: CodecPosture::Available,
        h264: CodecPosture::Available,
    }
}

/// The startup-built §3.2.3 registry — "built at startup", one per process, for the running platform +
/// the resolved §3.4 disposition. `Err` = a §3.2.1 invariant violation in the registered set (a
/// programming fault the conductor surfaces as `InternalError`; no panic). [Build-Session-Entscheidung:
/// P4.4] a `std::sync::LazyLock` (in-std since 1.80; MSRV 1.96): the registry is immutable after build
/// and every consumer — the conductor's select+plan step now, the §0.9 serialised-flag map at P4.5, the
/// C12 startup probe at P4.45 — reads the same instance.
static REGISTRY: LazyLock<Result<EngineRegistry, RegistryBuildError>> = LazyLock::new(|| {
    EngineRegistry::build(
        registered_engines(),
        current_platform(),
        &resolved_patent_disposition(),
    )
});

/// The process-wide §3.2.3 registry accessor — the conductor's §1.7 select+plan step reads it.
pub fn engine_registry() -> Result<&'static EngineRegistry, &'static RegistryBuildError> {
    REGISTRY.as_ref()
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use crate::domain::{CrossCatOp, FormatId, UserFacingFormat};
    use crate::engines::EngineKind;

    // A test-only rival claiming the native engine's (Csv → Tsv) cell — the §3.2.1 duplicate-owner leg.
    // The non-capability methods answer the honest InternalError shapes (never called by these tests).
    struct RivalCsvEngine;
    impl Engine for RivalCsvEngine {
        fn id(&self) -> EngineId {
            EngineId::Pandoc
        }
        fn descriptor(&self) -> EngineDescriptor {
            EngineDescriptor {
                id: EngineId::Pandoc,
                serialised_only: false,
                kind: EngineKind::Subprocess,
            }
        }
        fn capabilities(
            &self,
            _platform: Platform,
            _patents: &PatentDisposition,
        ) -> Vec<EngineCapability> {
            vec![EngineCapability {
                source: UserFacingFormat::Csv,
                target: TargetId::Format(FormatId::Tsv),
                direction: Direction::Encode,
            }]
        }
        fn plan(
            &self,
            _item: &DroppedItem,
            _target: TargetId,
            _input: &Path,
            _out_tmp: &TempPath,
        ) -> Result<PlanOutcome, PlanError> {
            Err(PlanError {
                kind: ConversionErrorKind::InternalError,
                detail: "test rival engine plans nothing".to_owned(),
            })
        }
        fn classify_failure(&self, _exit: ExitStatus, _stderr: &str) -> ConversionErrorKind {
            ConversionErrorKind::InternalError
        }
    }

    // A test-only engine declaring a Direction::Both row with a §0.6 Op target — the unflippable leg.
    struct OpBothEngine;
    impl Engine for OpBothEngine {
        fn id(&self) -> EngineId {
            EngineId::FFmpeg
        }
        fn descriptor(&self) -> EngineDescriptor {
            EngineDescriptor {
                id: EngineId::FFmpeg,
                serialised_only: false,
                kind: EngineKind::Subprocess,
            }
        }
        fn capabilities(
            &self,
            _platform: Platform,
            _patents: &PatentDisposition,
        ) -> Vec<EngineCapability> {
            vec![EngineCapability {
                source: UserFacingFormat::Csv,
                target: TargetId::Op(CrossCatOp::ExtractAudio),
                direction: Direction::Both,
            }]
        }
        fn plan(
            &self,
            _item: &DroppedItem,
            _target: TargetId,
            _input: &Path,
            _out_tmp: &TempPath,
        ) -> Result<PlanOutcome, PlanError> {
            Err(PlanError {
                kind: ConversionErrorKind::InternalError,
                detail: "test op-both engine plans nothing".to_owned(),
            })
        }
        fn classify_failure(&self, _exit: ExitStatus, _stderr: &str) -> ConversionErrorKind {
            ConversionErrorKind::InternalError
        }
    }

    // §6.4.1 unit (G15): the startup registry builds Ok and resolves BOTH slice orderings to the native
    // engine (the Both-row expansion covering §04/spreadsheets' two ✓(native) cells), and resolves the
    // winner to the engine handle §1.7 calls plan() on.
    #[test]
    fn startup_registry_resolves_both_slice_orderings_to_the_native_engine() {
        let registry = engine_registry()
            .expect("§3.2.3: the registered v1 set builds a valid single-owner registry");
        assert_eq!(
            registry.select(UserFacingFormat::Csv, TargetId::Format(FormatId::Tsv)),
            Some(EngineId::NativeCsvTsv)
        );
        assert_eq!(
            registry.select(UserFacingFormat::Tsv, TargetId::Format(FormatId::Csv)),
            Some(EngineId::NativeCsvTsv),
            "§3.2.3/§04: the one declared CSV ↔ TSV arrow registers the reverse ordering too"
        );
        let engine = registry
            .engine(EngineId::NativeCsvTsv)
            .expect("§3.2.3: a select() winner is always a registered engine");
        assert_eq!(engine.id(), EngineId::NativeCsvTsv);
    }

    // §6.4.1 unit (G15): select() misses honestly on an unregistered pair (§3.2.3's single legitimate
    // None — the caller surfaces §2.8 PlatformUnavailable) and engine() misses on an unregistered id.
    #[test]
    fn select_and_engine_miss_honestly_on_unregistered_keys() {
        let registry = engine_registry()
            .expect("§3.2.3: the registered v1 set builds a valid single-owner registry");
        assert_eq!(
            registry.select(UserFacingFormat::Csv, TargetId::Format(FormatId::Webp)),
            None
        );
        assert_eq!(
            registry.select(UserFacingFormat::Webp, TargetId::Format(FormatId::Csv)),
            None
        );
        assert!(registry.engine(EngineId::FFmpeg).is_none());
    }

    // §6.4.1 unit (G15): a second owner for an already-owned cell fails the BUILD — §3.2.1
    // single-engine-per-pair, fail-closed, no first-wins masking.
    #[test]
    fn registry_build_rejects_a_second_owner_for_a_pair() {
        let err = EngineRegistry::build(
            vec![Arc::new(NativeCsvTsvEngine), Arc::new(RivalCsvEngine)],
            Platform::Linux,
            &resolved_patent_disposition(),
        )
        .expect_err("§3.2.1: two owners for one (source, target) cell must fail the build");
        assert_eq!(
            err,
            RegistryBuildError::DuplicatePair {
                source: UserFacingFormat::Csv,
                target: TargetId::Format(FormatId::Tsv),
                first: EngineId::NativeCsvTsv,
                second: EngineId::Pandoc,
            }
        );
    }

    // §6.4.1 unit (G15): a Direction::Both row whose target is a §0.6 Op has no reversible ordering —
    // the build fails closed rather than half-registering the arrow.
    #[test]
    fn registry_build_rejects_an_unflippable_both_row() {
        let err = EngineRegistry::build(
            vec![Arc::new(OpBothEngine)],
            Platform::Win,
            &resolved_patent_disposition(),
        )
        .expect_err("§3.2.2/§0.6: a Both row cannot carry an Op target");
        assert_eq!(
            err,
            RegistryBuildError::UnflippableBothRow {
                engine: EngineId::FFmpeg,
                target: TargetId::Op(CrossCatOp::ExtractAudio),
            }
        );
    }
}
