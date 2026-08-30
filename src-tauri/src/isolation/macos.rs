//! `crate::isolation::macos` — the §3.5.0 / §7.2.6 **macOS TCC source-staging** slice.
//!
//! On macOS the dropped source frequently sits in a TCC-protected location (Desktop / Documents /
//! Downloads / a removable volume). §7.2.6 fact 2 forbids relying on the responsible-process chain holding
//! for a spawned engine, so §3.5.0 makes the CORE — which already holds the grant, having read the path
//! during the §1.1 freeze / detection — the only process that ever *first* reads such a path: it copies the
//! source into per-job **kind-2 scratch** (§2.14.2) and hands the engine that path instead. The absolute is
//! **read-side only**: the §2.1 publish still writes a `.part` inside the destination dir, and a TCC denial
//! there fails that one item per §2.8 (§3.5.0's own scope note).
//!
//! ## Why this file exists at all — the P4.85 homing contract
//! `check-sast`'s `misplaced_macos_cfg` leg requires every `target_os = "macos"`-conditional slice of the
//! isolation tree to live HERE, so the `paths:`-scoped G29 rule (d) — which keys on the literal standalone
//! `stage_for_tcc` call preceding a `Command::new` — can see it. The module is therefore declared
//! UNCONDITIONALLY in `isolation/mod.rs`: a `#[cfg(target_os = "macos")] mod macos;` there would itself be a
//! mac-cfg outside this file and would fail that very check. The per-item `#[cfg]` below is the established
//! `crate::platform` pattern (per-item attributes, never a module-level `#![cfg]`).
//!
//! ## Why the MECHANICS are portable and only the POLICY is gated
//! [Build-Session-Entscheidung: P4.24] §3.5.0 makes the staging macOS-only, but nothing about *copying a
//! file into the run's scratch dir* is macOS-specific — only the decision to do it is. Splitting it that way
//! buys real coverage: [`stage_source_into_scratch`] compiles and is regression-tested on **all three** CI
//! legs and on a developer's own machine, while the untestable-off-mac surface shrinks to the one-line
//! `stage_for_tcc` delegation. The alternative — a module-level `#![cfg(target_os = "macos")]` — would
//! have made the whole slice invisible to every non-mac build, i.e. unverifiable by the Build-Loop host and
//! by two of the three gate-tooling legs. Correctness that only one CI leg can check is worth less than the
//! same correctness three legs check.
//!
//! (`stage_for_tcc` is named throughout as a plain code span, never an intra-doc link: it is cfg-gated OFF
//! every non-macOS doc build, so a link would be unresolvable there and would redden the CI-only G74
//! rustdoc leg on two of the three OSes — the same cfg-gated-item convention `crate::platform` states for
//! its own Windows-gated items. Caught locally this time by running that CI-only leg before the push, which
//! is exactly what P4.22 did not do.)
//!
//! ## Lifecycle — free by construction, not by a cleanup call
//! The staged copy is written INSIDE `run-<RunId>/`, so §2.14.2's two lifecycle guarantees hold without any
//! code here: the §2.6.2 run-scope `cleanup_run` removes it unconditionally on every normal / cancel / error
//! exit, and a crash between staging and spawn leaves it reclaimable by the §2.6.3 next-launch sweep
//! (absent lock ⇒ dead ⇒ reclaimable). Taking a `&RunScratch` — the handle that only exists once the
//! `run-<RunId>/.lock` is held — makes §2.14.2's "created AFTER the run-lock" ordering STRUCTURAL rather
//! than a convention, exactly as `RunScratch::publish_temp` does for the kind-1 `.part`.

use std::io;
use std::path::{Path, PathBuf};

use crate::domain::JobId;
use crate::run::RunScratch;

/// The staged-source filename prefix — `src-<jobId>[.<ext>]` inside `run-<RunId>/`.
///
/// **The source's BASENAME is deliberately not reused** [Build-Session-Entscheidung: P4.24]: a `JobId` is
/// unique within the run (§0.6 invariant 6 — it IS the item's index), so this is collision-free by
/// construction, it cannot trip over a non-UTF-8 or absurdly long user filename (§2.10.1), and it keeps the
/// user's filename out of the scratch tree. Nothing downstream needs the staged basename: the OUTPUT name
/// is computed from the SOURCE path by §2.2, and P7.11 explicitly discovers LibreOffice output by
/// unique-empty-outdir snapshot-diff rather than by source-basename match.
const STAGED_SOURCE_PREFIX: &str = "src-";

/// Copy `source` into `scratch`'s kind-2 run dir and return the staged path (§3.5.0 step 1).
///
/// **The extension IS preserved** [Build-Session-Entscheidung: P4.24] — this is the one part of the staged
/// name that is load-bearing rather than cosmetic. The §3.5.2 LibreOffice shape passes NO `--infilter`
/// (`soffice --headless … --convert-to <ext>:<Filter> --outdir <scratch> <input>`), so LO picks the import
/// filter itself and reads the input's extension in doing so [Corrected by P4.24-r2 — the earlier wording
/// also claimed pandoc infers its input format from the name; it does not, because the §3.5.4 shape
/// `pandoc -f <in-fmt> -t <out-fmt> …` ALWAYS passes an explicit `-f`. LibreOffice alone carries the
/// engine-specific half of this rationale]. FFmpeg and poppler sniff content and would not care either, but
/// staging must be engine-agnostic — it runs before the §3.2.3 registry's choice is relevant to this layer
/// — so the safe direction is to carry the extension across for every engine. A source with no extension
/// stages without one.
///
/// **PORTABLE on purpose** — see the module doc. It is the whole mechanism; `stage_for_tcc` adds only the
/// §3.5.0 macOS policy on top, so every property that can be regressed off-mac is regressed off-mac.
///
/// Returns the plain `io::Result`: mapping a failure onto the §2.8 taxonomy is the §1.7 caller's, not this
/// layer's (the same division `RunScratch::publish_temp` keeps).
#[allow(
    dead_code,
    reason = "P4.24 — the §3.5.0 staging mechanism, authored with its macOS entry point. Its production \
              caller is P4.25 (hand engines the staged scratch path), so on Windows/Linux — where \
              `stage_for_tcc` does not exist at all — it is dead in every production build. On macOS it is \
              LIVE even before P4.25, because `stage_for_tcc` below bears a dead_code lint-level attribute \
              and rustc ROOTS such an item, propagating liveness to what it calls. That target-dependent \
              split is exactly why this is `allow` and never `expect`: an `expect` would go UNFULFILLED on \
              macOS, the one target this Windows host cannot compile. The crate::platform precedent \
              (platform/mod.rs:948) uses `allow` for the same cross-target reason."
)]
fn stage_source_into_scratch(
    scratch: &RunScratch,
    job: JobId,
    source: &Path,
) -> io::Result<PathBuf> {
    // §2.12.4 NO-HANG, the pre-open regular-file check. `source` is an untrusted, user-writable path and
    // `fs::copy` OPENS it; std's own directory guard runs AFTER that open, so it does not prevent the block.
    // A FIFO/pipe/device here would park the copy forever IN-CORE — before any spawn, so §1.7's subprocess
    // timeouts never see it. This is `orchestrator::detect_candidate`'s P3.49 pre-check applied to the second
    // untrusted read path, with the same owner-accepted residual: the µs window between THIS stat and THIS
    // open, inherent to any stat-then-open and not a full closure. [Build-Session-Entscheidung: P4.24]
    if !std::fs::metadata(source)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "§2.12.4: the staging source is not a regular file",
        ));
    }
    let mut name = String::from(STAGED_SOURCE_PREFIX);
    name.push_str(&job.as_u32().to_string());
    let mut staged = scratch.dir().join(name);
    if let Some(extension) = source.extension() {
        staged.set_extension(extension);
    }
    std::fs::copy(source, &staged)?;
    Ok(staged)
}

/// Test-only crate-visible door onto [`stage_source_into_scratch`] for `crate::run`'s §6.4.2 kill-fence
/// property test [Build-Session-Entscheidung: P4.24].
///
/// The mechanism itself is deliberately MODULE-PRIVATE: §3.5.0 scopes staging to macOS and §2.14.4 puts the
/// kind-2 staged-source term at 0 on Windows/Linux, so an off-macOS production caller would silently add an
/// unmodelled footprint the §1.10 preflight does not account for. Module privacy closes that structurally
/// rather than by convention — and `#[cfg(test)]` here means no production build can reach this door either.
/// The property test cannot simply live in this module: it must drive `crate::run`'s `sweep_stale_within`,
/// which is private there (the public `sweep_stale` applies the 10 s `LOCKLESS_GRACE` and would KEEP a
/// just-created run dir), and it belongs beside its sweep siblings in any case.
#[cfg(test)]
pub(crate) fn stage_source_into_scratch_for_tests(
    scratch: &RunScratch,
    job: JobId,
    source: &Path,
) -> io::Result<PathBuf> {
    stage_source_into_scratch(scratch, job, source)
}

/// The §3.5.0 / §7.2.6 **macOS** TCC staging entry — the load-bearing name.
///
/// `stage_for_tcc` is not a free naming choice: build-gates G29 rule (d), test-strategy's macOS-T11
/// first-accessor row and the P7.16 / P7.21 / P7.28 boxes all name this exact function, and the P4.85
/// refinement keys its `paths:`-scoped Semgrep rule on the literal standalone call. Renaming it silently
/// voids the T11 control (§0.11).
///
/// It deliberately adds no mechanism of its own — the whole point of the split is that the mechanism is
/// portable and tested everywhere ([`stage_source_into_scratch`]), so what is macOS-gated is only §3.5.0's
/// POLICY: *on this platform, the engine must never be the first process to touch the source*.
#[cfg(target_os = "macos")]
#[allow(
    dead_code,
    reason = "P4.24 — the §3.5.0 macOS staging entry, authored ahead of its P4.25 caller (which hands \
              engines the staged path in place of the raw protected one). The allowance is UNCONDITIONAL, \
              not `cfg_attr(not(test), …)`: this item exists ONLY on macOS, and `cargo clippy --all-targets` \
              on the macos-14 CI leg builds the lib TEST target, where a `not(test)` allowance expands to \
              nothing — `pub(crate)` is not a dead_code root in a lib crate, so the `not(test)` form would \
              have reddened the one leg this Windows host cannot compile (probed: `cargo check --target \
              aarch64-apple-darwin` needs an Apple `cc` this host has none of). `allow` rather than `expect` \
              for the same host reason and per the crate::platform cross-target precedent \
              (platform/mod.rs:948), whose `any(not(test), target_os = …)` guard is the same idea: allow \
              unconditionally on the target where the item is dead under BOTH cfgs."
)]
pub(crate) fn stage_for_tcc(
    scratch: &RunScratch,
    job: JobId,
    source: &Path,
) -> io::Result<PathBuf> {
    stage_source_into_scratch(scratch, job, source)
}

#[cfg(test)]
mod macos_staging_tests {
    use super::*;
    use crate::domain::{InstanceId, RunId};

    /// A real locked `RunScratch` over a temp base — the same shape `crate::run`'s own tests use, so the
    /// staged copy really does land inside a run dir that `cleanup_run` owns.
    fn scratch(base: &Path) -> RunScratch {
        RunScratch::acquire(base, InstanceId::mint(), std::process::id(), RunId::mint())
            .expect("§2.6: the per-run scratch acquires")
    }

    // §6.4.1 unit (G15): §3.5.0 step 1 — the source is COPIED into the run's kind-2 scratch, byte for byte,
    // and the original is untouched (the never-harm-the-original absolute applies to the staging read path
    // as much as to the publish: staging must never move or modify the user's file).
    #[test]
    fn it_copies_the_source_into_the_run_scratch_leaving_the_original_intact() {
        let base = tempfile::tempdir().expect("a scratch base");
        let src_dir = tempfile::tempdir().expect("a source dir");
        let source = src_dir.path().join("holiday.csv");
        std::fs::write(&source, b"a,b\n1,2\n").expect("write the source");
        let scratch = scratch(base.path());

        let staged = stage_source_into_scratch(&scratch, JobId::from_index(7), &source)
            .expect("§3.5.0: staging copies the source into kind-2 scratch");

        assert_eq!(
            std::fs::read(&staged).expect("the staged copy is readable"),
            b"a,b\n1,2\n",
            "§3.5.0: the staged copy is byte-identical to the source"
        );
        assert!(
            source.exists(),
            "§2.0 never-harm-the-original: staging COPIES, it never moves the user's file"
        );
        assert_eq!(
            std::fs::read(&source).expect("the original is readable"),
            b"a,b\n1,2\n",
            "… and leaves its bytes untouched"
        );
    }

    // §6.4.1 unit (G15): the staged copy lands INSIDE `run-<RunId>/`, which is the whole lifecycle argument
    // of §2.14.2 — the §2.6.2 run-scope cleanup and the §2.6.3 next-launch sweep both reclaim it with no
    // code of ours, so a cancel or a crash between staging and spawn strands nothing. Asserting the
    // CONTAINMENT is asserting that guarantee; asserting a cleanup call would only assert our own plumbing.
    #[test]
    fn the_staged_copy_lives_inside_the_run_dir_so_the_run_cleanup_reclaims_it() {
        let base = tempfile::tempdir().expect("a scratch base");
        let src_dir = tempfile::tempdir().expect("a source dir");
        let source = src_dir.path().join("a.txt");
        std::fs::write(&source, b"x").expect("write the source");
        let scratch = scratch(base.path());

        let staged = stage_source_into_scratch(&scratch, JobId::from_index(0), &source)
            .expect("§3.5.0: staging succeeds");

        assert_eq!(
            staged.parent(),
            Some(scratch.dir()),
            "§2.14.2: the staged source is a kind-2 file directly under `run-<RunId>/`"
        );
    }

    // §6.4.1 unit (G15): the extension is carried across (the §3.5.2 LibreOffice shape passes no
    // `--infilter`, so LO reads the name in choosing its import filter) while the user's BASENAME is not (a
    // `JobId` is unique within the run, so the name is collision-free without ever touching a
    // possibly-non-UTF-8 filename, §2.10.1).
    #[test]
    fn the_staged_name_keeps_the_extension_but_not_the_user_basename() {
        let base = tempfile::tempdir().expect("a scratch base");
        let src_dir = tempfile::tempdir().expect("a source dir");
        let source = src_dir.path().join("Quarterly Report (final).docx");
        std::fs::write(&source, b"d").expect("write the source");
        let scratch = scratch(base.path());

        let staged = stage_source_into_scratch(&scratch, JobId::from_index(12), &source)
            .expect("§3.5.0: staging succeeds");

        assert_eq!(
            staged.file_name().and_then(std::ffi::OsStr::to_str),
            Some("src-12.docx"),
            "§3.5.0: `src-<jobId>.<ext>` — the extension engines infer from is kept, the user's basename is not"
        );
    }

    // §6.4.1 unit (G15): an extension-less source stages without inventing one — a fabricated extension
    // would actively MISLEAD the very filter inference the previous test preserves it for.
    #[test]
    fn an_extension_less_source_stages_without_one() {
        let base = tempfile::tempdir().expect("a scratch base");
        let src_dir = tempfile::tempdir().expect("a source dir");
        let source = src_dir.path().join("README");
        std::fs::write(&source, b"r").expect("write the source");
        let scratch = scratch(base.path());

        let staged = stage_source_into_scratch(&scratch, JobId::from_index(3), &source)
            .expect("§3.5.0: staging succeeds");

        assert_eq!(
            staged.file_name().and_then(std::ffi::OsStr::to_str),
            Some("src-3"),
            "§3.5.0: no extension in, no extension out"
        );
    }

    // §6.4.x (G15): distinct items never collide — §2.14.2 notes up to the §0.9 concurrency degree of staged
    // copies coexist, so two in-flight jobs staging at once must land on different paths. The `JobId` is the
    // §0.6-invariant-6 index, which is what makes this true by construction rather than by chance.
    #[test]
    fn distinct_jobs_stage_to_distinct_paths_even_from_the_same_source() {
        let base = tempfile::tempdir().expect("a scratch base");
        let src_dir = tempfile::tempdir().expect("a source dir");
        let source = src_dir.path().join("shared.png");
        std::fs::write(&source, b"p").expect("write the source");
        let scratch = scratch(base.path());

        let first = stage_source_into_scratch(&scratch, JobId::from_index(1), &source)
            .expect("§3.5.0: the first item stages");
        let second = stage_source_into_scratch(&scratch, JobId::from_index(2), &source)
            .expect("§3.5.0: the second item stages");

        assert_ne!(
            first, second,
            "§2.14.2: concurrent staged sources must not collide — the JobId separates them"
        );
        assert!(
            first.exists() && second.exists(),
            "… and both really exist at once, which is the case §2.14.2's peak-concurrent footprint counts"
        );
    }

    // §6.4.1 unit (G15): a missing source surfaces as a plain `io::Error` rather than a panic — the §1.7
    // caller (P4.25) is what maps it onto the §2.8 taxonomy, so this layer must fail honestly and quietly.
    // The in-core no-panic policy applies here exactly as on the detect/fs_guard path.
    #[test]
    fn a_missing_source_is_an_io_error_not_a_panic() {
        let base = tempfile::tempdir().expect("a scratch base");
        let src_dir = tempfile::tempdir().expect("a source dir");
        let scratch = scratch(base.path());

        let staged = stage_source_into_scratch(
            &scratch,
            JobId::from_index(4),
            &src_dir.path().join("not-there.jpg"),
        );

        assert!(
            staged.is_err(),
            "§3.5.0/§2.8: a source that cannot be read fails the ITEM, honestly and without a panic"
        );
    }
    // §6.4.1 unit (G15) / §2.12.4: a NON-REGULAR source is an honest `Err`, never a blocking open. The
    // dangerous member of that class is a Unix FIFO — `fs::copy` would park in-core forever on it — but the
    // branch under test is `!metadata.is_file()`, which a FIFO, a device, a socket and a DIRECTORY all take.
    // The directory is the one non-regular kind that exists on all three target platforms, so it drives the
    // guard portably; a FIFO reaches the identical early return one line further down.
    #[test]
    fn a_non_regular_source_is_refused_before_the_open_rather_than_blocking() {
        let base = tempfile::tempdir().expect("a scratch base");
        let src_dir = tempfile::tempdir().expect("a source dir");
        let scratch = scratch(base.path());

        let refused = stage_source_into_scratch(&scratch, JobId::from_index(3), src_dir.path())
            .expect_err("§2.12.4: a non-regular staging source is refused, not opened");

        assert_eq!(
            refused.kind(),
            io::ErrorKind::InvalidInput,
            "§2.12.4: the refusal is an honest InvalidInput the §1.7 caller can map onto §2.8, not a hang \
             and not a panic"
        );
        // The kind ALONE would be vacuous on Linux/macOS: std's `fs::copy` opens first and only THEN checks
        // `is_file()`, returning the identical `InvalidInput` for a directory — so deleting the pre-check
        // would leave the assertion above green on exactly the two platforms where the FIFO hang is real
        // (on Windows `CopyFileExW` yields `PermissionDenied` instead, so only that leg would have caught
        // it). Matching our own §-tagged message is what proves the PRE-open guard fired. A real FIFO
        // cannot be built here — `#![deny(unsafe_code)]` rules out `libc::mkfifo` and the repo has no
        // precedent for one — but a FIFO takes this same `!is_file()` branch. [Build-Session-Entscheidung: P4.24]
        assert!(
            refused.to_string().contains("§2.12.4"),
            "§2.12.4: … and it is OUR pre-open refusal, not std's identical post-open one; got {refused}"
        );
        let residue: Vec<_> = std::fs::read_dir(scratch.dir())
            .expect("the run dir is readable")
            .filter_map(|entry| Some(entry.ok()?.file_name().to_string_lossy().into_owned()))
            .filter(|name| name.starts_with(STAGED_SOURCE_PREFIX))
            .collect();
        assert!(
            residue.is_empty(),
            "§2.14.2: a refused staging leaves NO staged file behind — the run dir holds only the \
             §2.6 `.lock` the scratch created for itself, never a half-made `src-*`; found \
             {residue:?}"
        );
    }

    // §6.4.1 unit (G15), macOS-only: the §3.5.0 entry point really is a pure delegation — it adds POLICY,
    // not mechanism. This is the leg that makes the PORTABLE/GATED split verifiable on its own target rather
    // than only argued: with it, the macos-14 CI leg covers every line of this module. It cannot run on the
    // Build-Loop's Windows host (the macOS target does not compile here at all), so its first execution IS
    // its validation, on that leg. [Build-Session-Entscheidung: P4.24]
    #[cfg(target_os = "macos")]
    #[test]
    fn the_macos_tcc_entry_is_a_pure_delegation_to_the_portable_mechanism() {
        let base = tempfile::tempdir().expect("a scratch base");
        let src_dir = tempfile::tempdir().expect("a source dir");
        let source = src_dir.path().join("protected.csv");
        std::fs::write(&source, b"x,y\n8,9\n").expect("write the source");
        let scratch = scratch(base.path());

        let staged = stage_for_tcc(&scratch, JobId::from_index(4), &source)
            .expect("§3.5.0: the macOS entry stages the source");

        assert_eq!(
            staged,
            scratch.dir().join("src-4.csv"),
            "§3.5.0: the entry produces the mechanism's own `src-<jobId>[.<ext>]` path, adding no naming of \
             its own"
        );
        assert_eq!(
            std::fs::read(&staged).expect("read the staged copy"),
            b"x,y\n8,9\n",
            "§3.5.0: … with the mechanism's byte-exact copy semantics"
        );
        assert!(
            source.exists(),
            "§2.0: … and the TCC-protected original is untouched"
        );
    }
}
