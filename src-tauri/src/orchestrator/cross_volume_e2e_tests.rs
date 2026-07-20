//! §2.1.2/§2.7.2/§2.14.3 (G31) the FAT/exFAT-divert + cross-volume integration case (P3.65) — the §2.1.1
//! per-item write sequence ([`publish_written_temp`](super::publish_written_temp)) driven over a real temp FS
//! for the two destination shapes the beside-source happy path never reaches: a destination that cannot host
//! an atomic no-clobber publish at all (§2.1.2 third fallback / §2.7.2 `DivertReason::NoAtomicPublish`), and a
//! publish temp that lands on a different VOLUME than `final` (§2.14.3 `EXDEV`). Nothing is mocked — the real
//! `crate::run` temp ownership, the real `crate::fs_guard` publish primitives, a real filesystem, and — where
//! the host offers one — a real second volume (test-strategy §0.1).
//!
//! **Two preconditions are supplied; nothing under test is.** Mounting a FAT/exFAT filesystem is a privileged,
//! out-of-process act, and `crate::isolation` is the sole sanctioned `process::Command::new` site (G9
//! invariant (b) / G29), so no test may provision that substrate. Both entry points into the §2.7.2 divert are
//! therefore reached by supplying the PRECONDITION — the same injectable-value idiom
//! `publish_numbered_capped`'s `cap` and `publish_cross_volume_checked`'s `avail_bytes` already use:
//!
//! - the **PROACTIVE** (§1.8 planning) route — the `LocationStatus` verdict is handed to
//!   [`compute_output_plan`](crate::fs_guard::compute_output_plan), which takes it by value;
//! - the **REACTIVE** (§2.1.2 publish-time) route — the `crate::fs_guard::fat_class_destination` `#[cfg(test)]`
//!   fence makes ONE directory answer `NoAtomicPublishSupport`, the sibling of the P3.19.1 `kill_after_sync`
//!   fence and compiled out of production identically.
//!
//! Everything downstream of either verdict is real: the §2.7.3 target resolution, the §2.2 naming, the §2.3.3
//! link-safety re-check, the §2.1 exclusive publish AT the divert target, the §2.14.3 cross-volume copy and
//! the §2.6.2 cleanup. What the DETECTOR does with a real filesystem is asserted separately — against a
//! kernel-reported FAT/exFAT mount by `crate::platform`'s suite wherever a host offers one, and on the
//! magic/name values directly (the P3.18 ruling).
//!
//! **The honest environment bound (recorded, never a silent skip — the test-strategy §1.7
//! acknowledged-gap doctrine).** What no host here can
//! drive is the errno mapping INSIDE `publish_noreplace`/`publish_link_fallback` (`EINVAL`/`ENOTSUP` from the
//! no-replace rename AND `EPERM`/`ENOTSUP` from `link()`), because that needs the filesystem itself. Its home
//! is the release-candidate verification on real removable media (P11.25, DoD gate 8). The cross-volume legs
//! that need a second volume skip where the host has none — and a leg that KNOWS it has one arms
//! `CONVERTIA_REQUIRE_SECOND_VOLUME=1` so the absence is a hard failure rather than a silent no-op
//! ([`crate::test_volumes`]). [Build-Session-Entscheidung: P3.65]
//!
//! Reuses the [`run_conversion_tests`](super::run_conversion_tests) `pub(super)` harness for every
//! non-ephemeral directory, so the §2.7.2 placement rule has one home. [Build-Session-Entscheidung: P3.65]

use super::run_conversion_tests::non_ephemeral_source_dir;
use super::*;
use crate::domain::InstanceId;

/// A live run handle plus a real source file and its frozen identity — the inputs the §2.1.1 write sequence
/// threads. The destination dirs differ per test, so they are NOT part of the fixture.
struct Fixture {
    _scratch_base: tempfile::TempDir,
    scratch: RunScratch,
    _src_dir: tempfile::TempDir,
    source: PathBuf,
    frozen: Vec<FileIdentity>,
    cache: LocationCache,
    instance: InstanceId,
}

impl Fixture {
    /// `None` when the crate source root is itself under an OS temp root — the pathological environment in
    /// which no realistic non-ephemeral source placement exists (a clean skip, never a false pass).
    fn new(source_bytes: &[u8]) -> Option<Self> {
        let scratch_base = tempfile::tempdir().expect("a real scratch base dir");
        let instance = InstanceId::mint();
        let scratch = RunScratch::acquire(
            scratch_base.path(),
            instance,
            std::process::id(),
            RunId::mint(),
        )
        .expect("acquire the run scratch (lock held)");
        let src_dir = non_ephemeral_source_dir()?;
        let source = src_dir.path().join("data.csv");
        std::fs::write(&source, source_bytes).expect("write the source file");
        let frozen =
            vec![crate::fs_guard::resolve_identity(&source).expect("resolve the source identity")];
        Some(Self {
            _scratch_base: scratch_base,
            scratch,
            _src_dir: src_dir,
            source,
            frozen,
            cache: LocationCache::new(),
            instance,
        })
    }

    /// The §2.7.2 writability-probe name factory in the REAL `crate::run` grammar
    /// (`.convertia-<InstanceId>-probe-<rand>.part`), exactly as the C6 conductor builds it — so a probe
    /// residue this run may leave is the one the §2.6.3 InstanceId-liveness sweep reclaims, and no two tests
    /// can collide on a shared literal name.
    fn probe(&self) -> impl Fn() -> OsString + Copy {
        let instance = self.instance;
        move || crate::run::PublishTemp::probe_name(instance)
    }
}

/// The `.part` sibling names in `dir` — the §2.6.2 leftover-temp assertion (a clean publish leaves none).
fn part_files(dir: &Path) -> Vec<OsString> {
    std::fs::read_dir(dir)
        .expect("read the dir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
        .filter(|name| name.to_string_lossy().contains(".part"))
        .collect()
}

// §2.7.2/§2.7.3 (G31) THE PROACTIVE ROUTE — a FAT/exFAT-CLASS DESTINATION DIVERTS TO THE HARDLINK-CAPABLE
// TARGET AND THE FULL §2.1 CHAIN HOLDS THERE. Given the §2.7.2 `NoAtomicPublish` verdict for the beside-source
// location, §1.8 planning resolves the §2.7.3 divert root instead, records the REASON on the plan (so the
// §1.12 summary and the §5 divert note can name it), and the §2.1.1 write sequence publishes there through the
// real exclusive publish — output present and byte-exact, the source untouched (G32(a)), no residue, the
// outcome flagged `diverted`.
//
// Run on EVERY platform, deliberately: §2.7.2 makes the PRODUCTION of this verdict Unix-only (Windows' NT
// create-only rename is a true no-clobber publish on FAT/exFAT too — the sibling Windows test below pins
// that), but everything the verdict FEEDS — the §2.7.3 target resolution, the §2.2 naming, the §2.3.3
// link-safety re-check, the §2.1 exclusive publish, the §2.6.2 cleanup — is OS-independent and rests on a
// DIFFERENT publish primitive per OS. Gating this to Unix would leave the Windows and macOS legs of that chain
// unexercised and the case unverifiable on a Windows workstation. [Build-Session-Entscheidung: P3.65]
#[test]
fn a_no_atomic_publish_destination_diverts_and_the_full_chain_holds_at_the_divert_target() {
    let Some(mut f) = Fixture::new(b"a,b\n1,2\n") else {
        return; // the crate root is itself ephemeral — no realistic non-ephemeral source placement.
    };
    let Some(divert_root) = non_ephemeral_source_dir() else {
        return; // same pathological environment — no valid non-ephemeral divert target.
    };
    let source_before = std::fs::read(&f.source).expect("read the source before the publish");
    // Hoisted: the probe factory borrows the fixture, and the same call also takes `&mut f.cache`.
    let probe = f.probe();

    // §1.8/§2.7.3: the FAT/exFAT-class verdict for the beside-source location routes the plan to the divert
    // root (flat, no subtree — §2.7.4) and records `DivertReason::NoAtomicPublish` on the plan.
    let plan = compute_output_plan(
        ItemId::from_index(0),
        &f.source,
        "tsv",
        DestinationMode::BesideSource,
        LocationStatus::Divert(DivertReason::NoAtomicPublish),
        divert_root.path(),
        &mut f.cache,
        probe,
    )
    .expect("§2.7.3: the writable divert root resolves");
    assert_eq!(
        plan.final_dir,
        divert_root.path(),
        "§2.7.3: a destination that cannot host an atomic no-clobber publish diverts to the resolved target"
    );
    assert_eq!(
        plan.diverted,
        Some(DivertReason::NoAtomicPublish),
        "§2.7.2/§0.6: the plan carries the NoAtomicPublish reason (the §1.12 summary + §5 note read it)"
    );

    // §2.1.1 steps 1-2 conductor-side (pick the temp, the engine writes into it), then the publish legs.
    let tmp = f
        .scratch
        .publish_temp(&plan.publish_temp_dir, plan.job)
        .expect("pick the publish temp");
    std::fs::write(&*tmp, b"a\tb\n1\t2\n").expect("the engine writes the output into the temp");
    let out = publish_written_temp(
        &plan,
        &f.source,
        &f.frozen,
        &[divert_root.path().to_path_buf()],
        &f.scratch,
        &mut f.cache,
        probe,
        tmp,
    );

    let output = divert_root.path().join("data.tsv");
    assert_eq!(
        out.disposition,
        WriteDisposition::Published {
            output: output.clone()
        },
        "§2.7.5: the full §2.1 publish chain runs at the divert target — not a degraded path"
    );
    assert!(
        out.diverted,
        "§1.12: the item reports as diverted (it drives RunResult.divert_root_display)"
    );
    assert_eq!(
        std::fs::read(&output).expect("read the diverted output"),
        b"a\tb\n1\t2\n",
        "§2.1.1: the diverted output carries exactly the bytes the engine seam wrote"
    );
    assert_eq!(
        std::fs::read(&f.source).expect("read the source after the publish"),
        source_before,
        "G32(a): the source file is byte-identical — a divert never harms the original"
    );
    assert!(
        out.residue.is_none(),
        "§2.6.4: the publish consumed its temp and reported no residue, got {:?}",
        out.residue
    );
}

// §2.7.5/§2.2.2 (G31) THE DIVERT PATH KEEPS THE NO-CLOBBER GUARANTEE: an unrelated file already at the target
// name in the divert root is NEVER overwritten — the exclusive publish numbers away to `stem (1).ext` exactly
// as it does beside-source, proving §2.7.5's "there is no code path where a divert skips a guarantee" on the
// §2.7.2 NoAtomicPublish trigger specifically. All-platform for the reason its sibling above states.
#[test]
fn a_collision_at_the_no_atomic_publish_divert_target_numbers_away_without_clobbering() {
    let Some(mut f) = Fixture::new(b"x\n") else {
        return; // the crate root is itself ephemeral (see the sibling test's note).
    };
    let Some(divert_root) = non_ephemeral_source_dir() else {
        return;
    };
    let taken = divert_root.path().join("data.tsv");
    std::fs::write(&taken, b"PRE-EXISTING must survive").expect("seed the collision");
    // Hoisted: the probe factory borrows the fixture, and the same call also takes `&mut f.cache`.
    let probe = f.probe();

    let plan = compute_output_plan(
        ItemId::from_index(0),
        &f.source,
        "tsv",
        DestinationMode::BesideSource,
        LocationStatus::Divert(DivertReason::NoAtomicPublish),
        divert_root.path(),
        &mut f.cache,
        probe,
    )
    .expect("§2.7.3: the writable divert root resolves");
    let tmp = f
        .scratch
        .publish_temp(&plan.publish_temp_dir, plan.job)
        .expect("pick the publish temp");
    std::fs::write(&*tmp, b"fresh").expect("the engine writes the output into the temp");
    let out = publish_written_temp(
        &plan,
        &f.source,
        &f.frozen,
        &[divert_root.path().to_path_buf()],
        &f.scratch,
        &mut f.cache,
        probe,
        tmp,
    );

    assert_eq!(
        out.disposition,
        WriteDisposition::Published {
            output: divert_root.path().join("data (1).tsv")
        },
        "§2.7.5/§2.2.2: the divert publish numbers away to the first free variant"
    );
    assert_eq!(
        std::fs::read(&taken).expect("read the pre-existing file"),
        b"PRE-EXISTING must survive",
        "§2.1 no-clobber: the pre-existing file at the divert target is byte-identical — never overwritten"
    );
}

// §2.1.2/§2.7.2 (G31) THE REACTIVE ROUTE — the arm that had NO coverage at all before this box. The §1.8 plan
// says "publish beside the source" (the §2.7.2 statfs heuristic did not fire — the P3.18 "list-miss" case the
// reactive arm exists to backstop), the destination then refuses at PUBLISH time, and `publish_completed`'s
// `Ok(NoAtomicPublishSupport) => divert_completed` arm rescues the completed output to the §2.7.3 target
// through the full safety chain. The beside-source dir is armed via the `crate::fs_guard::fat_class_destination`
// fence; the divert target is a sibling directory the fence does NOT cover, so its publish is entirely real.
#[test]
fn a_reactive_no_atomic_publish_verdict_late_diverts_the_completed_output() {
    let Some(mut f) = Fixture::new(b"a,b\n1,2\n") else {
        return; // the crate root is itself ephemeral (see the first test's note).
    };
    let (Some(beside), Some(divert_root)) =
        (non_ephemeral_source_dir(), non_ephemeral_source_dir())
    else {
        return;
    };
    let source_before = std::fs::read(&f.source).expect("read the source before the publish");
    // Hoisted: the probe factory borrows the fixture, and the same call also takes `&mut f.cache`.
    let probe = f.probe();
    // A plain beside-source plan — planning saw nothing wrong with this destination (`diverted: None`).
    let plan = OutputPlan {
        job: ItemId::from_index(0),
        final_dir: beside.path().to_path_buf(),
        diverted: None,
        base_name: OsString::from("data"),
        extension: OsString::from("tsv"),
        publish_temp_dir: beside.path().to_path_buf(),
    };
    let tmp = f
        .scratch
        .publish_temp(&plan.publish_temp_dir, plan.job)
        .expect("pick the publish temp");
    let tmp_path = tmp.to_path_buf();
    std::fs::write(&*tmp, b"a\tb\n1\t2\n").expect("the engine writes the output into the temp");

    // The destination refuses an atomic no-clobber publish only at publish time (§2.1.2 third fallback).
    let _fat = crate::fs_guard::fat_class_destination::Armed::arm(beside.path());
    let out = publish_written_temp(
        &plan,
        &f.source,
        &f.frozen,
        &[divert_root.path().to_path_buf()],
        &f.scratch,
        &mut f.cache,
        probe,
        tmp,
    );

    let output = divert_root.path().join("data.tsv");
    assert_eq!(
        out.disposition,
        WriteDisposition::Published {
            output: output.clone()
        },
        "§2.7.2: a publish-time NoAtomicPublishSupport late-diverts the completed output to the §2.7.3 target"
    );
    assert!(
        out.diverted,
        "§1.12: a late divert reports as diverted even though the PLAN was beside-source"
    );
    assert_eq!(
        std::fs::read(&output).expect("read the diverted output"),
        b"a\tb\n1\t2\n",
        "§2.7.5: the rescued output is byte-exact — the divert is not a degraded path"
    );
    assert!(
        !beside.path().join("data.tsv").exists(),
        "§2.1.2: nothing was published at the atomic-publish-incapable destination"
    );
    assert_eq!(
        std::fs::read(&f.source).expect("read the source after the publish"),
        source_before,
        "G32(a): the source file is byte-identical across the late divert"
    );
    assert!(
        out.residue.is_none() && !tmp_path.exists(),
        "§2.6.2: the publish temp is cleaned on the divert path and no residue is reported, got {:?}",
        out.residue
    );
    assert!(
        part_files(beside.path()).is_empty(),
        "§2.6.2: no `.part` is left at the refusing destination, found {:?}",
        part_files(beside.path())
    );
}

// §2.1.2/§2.7.2/§2.14.3 (G31) THE FULL LIVE COMPOSITION — the reactive refusal AND a cross-volume divert in one
// pass, which is how the two mechanisms actually meet in production: the completed temp sits on the refusing
// destination's volume, the §2.7.3 target is on ANOTHER volume, so `divert_completed`'s publish crosses volumes
// and takes the §2.14.3 copy-into-the-destination-volume fallback. Requires a real second volume; skips where
// the host has none (or hard-fails first, under `CONVERTIA_REQUIRE_SECOND_VOLUME`).
#[test]
fn a_reactive_late_divert_across_a_real_volume_boundary_publishes_and_leaves_no_residue() {
    let Some(mut f) = Fixture::new(b"a,b\n1,2\n") else {
        return; // the crate root is itself ephemeral (see the first test's note).
    };
    let Some(beside) = non_ephemeral_source_dir() else {
        return;
    };
    let Some(divert_root) = crate::test_volumes::second_volume_dir(beside.path()) else {
        // No second volume on this host, so a divert cannot cross one. The same-volume late divert above and
        // `crate::fs_guard::real_cross_volume_tests` cover the two halves separately.
        return;
    };
    let source_before = std::fs::read(&f.source).expect("read the source before the publish");
    // Hoisted: the probe factory borrows the fixture, and the same call also takes `&mut f.cache`.
    let probe = f.probe();
    let plan = OutputPlan {
        job: ItemId::from_index(0),
        final_dir: beside.path().to_path_buf(),
        diverted: None,
        base_name: OsString::from("data"),
        extension: OsString::from("tsv"),
        publish_temp_dir: beside.path().to_path_buf(),
    };
    let tmp = f
        .scratch
        .publish_temp(&plan.publish_temp_dir, plan.job)
        .expect("pick the publish temp");
    let tmp_path = tmp.to_path_buf();
    std::fs::write(&*tmp, b"a\tb\n1\t2\n").expect("the engine writes the output into the temp");

    let _fat = crate::fs_guard::fat_class_destination::Armed::arm(beside.path());
    let out = publish_written_temp(
        &plan,
        &f.source,
        &f.frozen,
        &[divert_root.path().to_path_buf()],
        &f.scratch,
        &mut f.cache,
        probe,
        tmp,
    );

    let output = divert_root.path().join("data.tsv");
    assert_eq!(
        out.disposition,
        WriteDisposition::Published {
            output: output.clone()
        },
        "§2.14.3: the cross-volume divert publishes — the caller sees an ordinary Published"
    );
    assert!(out.diverted, "§1.12: the item reports as diverted");
    assert_eq!(
        std::fs::read(&output).expect("read the diverted output"),
        b"a\tb\n1\t2\n",
        "§2.14.3: the output copied across the volume boundary is byte-exact"
    );
    assert_eq!(
        std::fs::read(&f.source).expect("read the source after the publish"),
        source_before,
        "G32(a): the source file is byte-identical across a cross-volume divert"
    );
    // §2.6.2: BOTH temps are cleaned — the one on the refusing volume (which the §2.14.3 copy deliberately
    // leaves behind rather than consuming) and the same-volume intermediate the rename did consume.
    assert!(
        out.residue.is_none() && !tmp_path.exists(),
        "§2.6.2: the cross-volume temp is explicitly cleaned and no residue is reported, got {:?}",
        out.residue
    );
    assert!(
        part_files(divert_root.path()).is_empty(),
        "§2.6.2: no `.part` remains beside the published output, found {:?}",
        part_files(divert_root.path())
    );
}

// §2.14.3 (G31) THE DEFENSIVE ARM — the §2.14.3 step-2 shape ("the engine is told to write its output to that
// other-volume scratch"), driven through the §2.1.1 write sequence with the publish temp on a real second
// volume.
//
// Stated plainly, because the framing matters: v1 PLANNING never produces this. `compute_output_plan` sets
// `publish_temp_dir = final_dir` unconditionally (§2.14.1's same-volume sibling, pinned by
// `fs_guard::compute_output_plan_tests`), and P3.37 deliberately carries no `crosses_volume` field — EXDEV is
// "detected reactively at publish". So this test hand-builds the plan to reach the DEFENSIVE cross-device arm
// from the DIRECT-publish side; the route production actually takes is the cross-volume DIVERT above. Both are
// worth pinning: the arm exists, it is reachable, and its §2.6.2 cleanup duty differs from the same-volume path
// (the copy leaves the source temp behind rather than consuming it). [Build-Session-Entscheidung: P3.65]
#[test]
fn the_write_sequence_publishes_across_a_real_volume_boundary_and_cleans_the_cross_volume_temp() {
    let Some(mut f) = Fixture::new(b"a,b\n1,2\n") else {
        return; // the crate root is itself ephemeral (see the first test's note).
    };
    let Some(dest) = non_ephemeral_source_dir() else {
        return;
    };
    let Some(other) = crate::test_volumes::second_volume_dir(dest.path()) else {
        // This host mounts no second volume, so the OS never returns a cross-device error and the §2.14.3
        // fallback cannot be driven here. Its primitives stay covered on every platform by `crate::fs_guard`'s
        // injected-`avail_bytes` tests; the routing arm is covered wherever a second volume exists.
        return;
    };
    let source_before = std::fs::read(&f.source).expect("read the source before the publish");
    // Hoisted: the probe factory borrows the fixture, and the same call also takes `&mut f.cache`.
    let probe = f.probe();

    let plan = OutputPlan {
        job: ItemId::from_index(0),
        final_dir: dest.path().to_path_buf(),
        diverted: None,
        base_name: OsString::from("data"),
        extension: OsString::from("tsv"),
        publish_temp_dir: other.path().to_path_buf(),
    };
    let tmp = f
        .scratch
        .publish_temp(&plan.publish_temp_dir, plan.job)
        .expect("pick the publish temp on the other volume");
    let tmp_path = tmp.to_path_buf();
    std::fs::write(&*tmp, b"a\tb\n1\t2\n").expect("the engine writes the output into the temp");
    let out = publish_written_temp(
        &plan,
        &f.source,
        &f.frozen,
        &[],
        &f.scratch,
        &mut f.cache,
        probe,
        tmp,
    );

    let output = dest.path().join("data.tsv");
    assert_eq!(
        out.disposition,
        WriteDisposition::Published {
            output: output.clone()
        },
        "§2.14.3: the caller sees an ordinary Published — the cross-volume fallback is invisible above fs_guard"
    );
    assert!(
        !out.diverted,
        "§2.14.3: a cross-volume publish is not a §2.7 divert — the destination was honoured"
    );
    assert_eq!(
        std::fs::read(&output).expect("read the published output"),
        b"a\tb\n1\t2\n",
        "§2.14.3: the published file carries the bytes copied across the volume boundary"
    );
    assert_eq!(
        std::fs::read(&f.source).expect("read the source after the publish"),
        source_before,
        "G32(a): the source file is byte-identical across the cross-volume publish"
    );
    assert!(
        out.residue.is_none(),
        "§2.6.4: a clean cross-volume publish reports no residue, got {:?}",
        out.residue
    );
    assert!(
        !tmp_path.exists(),
        "§2.6.2: the cross-volume temp the copy LEFT on its own volume is explicitly cleaned (it is not \
         consumed by the rename, unlike a same-volume publish)"
    );
    assert!(
        part_files(dest.path()).is_empty(),
        "§2.6.2: the same-volume intermediate was consumed and no `.part` remains beside `final`, found {:?}",
        part_files(dest.path())
    );
}

// §2.1.2/§2.7.2 (G31) WINDOWS KEEPS THE GUARANTEE BY CONSTRUCTION — a Windows destination is NEVER diverted for
// `NoAtomicPublish`, because the create-only NT rename is a true no-clobber publish on every Windows
// filesystem, so the §2.7.2 detector is a constant `false` there. Asserted through the COMPOSED
// `location_status` verdict on a real writable dir in its falsifiable form (`Writable`, not merely "anything
// but NoAtomicPublish"), so a Windows leg that ever started diverting would red here.
#[cfg(windows)]
#[test]
fn a_windows_destination_is_never_diverted_for_no_atomic_publish() {
    let Some(dest) = non_ephemeral_source_dir() else {
        return; // the crate root is itself ephemeral (see the first test's note).
    };
    let probe_name = crate::run::PublishTemp::probe_name(InstanceId::mint());
    assert_eq!(
        location_status(dest.path(), &probe_name),
        LocationStatus::Writable,
        "§2.7.2: a writable non-ephemeral Windows destination classifies Writable — the §2.1.2 create-only NT \
         rename is a true no-clobber publish on every Windows filesystem, so NoAtomicPublish never fires"
    );
}
