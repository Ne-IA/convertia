//! `crate::run` — the §2.6 per-run / per-instance scratch ownership + cleanup lifecycle, keyed on
//! `RunId` / `InstanceId` (§7.1): owned temp roots + names, the §2.6.3 startup orphan sweep, and
//! teardown. A §0.7 tier-2 trust-kernel LEAF: it depends DOWN only (on `crate::domain` for the
//! `RunId` / `InstanceId` / `ItemId` ids), never up on IPC / orchestrator / the engine registry (§2.0
//! dependency direction); it does NOT depend on `crate::fs_guard` to compile its root (the three
//! trust-kernel roots have no mutual dependency at scaffold time). Unsafe-free — the crate-root
//! `#![deny(unsafe_code)]` (main.rs) covers it; the §2.6.3 advisory-lock / try-lock FFI is homed in the
//! single allow-listed `crate::platform` shim (P3.21 / P3.23).
//!
//! ## P3.1.2 public-surface contract map — bodies authored by the named fill-boxes
//! [Build-Session-Entscheidung: P3.1.2] As in `crate::fs_guard` (P3.1.1), the surface is a documented
//! CONTRACT MAP, not callable bodies (the title's "function shells" = the public surface). Each
//! cleanup / sweep function does real filesystem work whose only honest value is the real one; a
//! permissive default would falsely claim "cleaned" / "swept", and a permissive `sweep_stale` could
//! remove a LIVE foreign temp (the §2.6.3 held-lock delete-gate the kernel exists to protect). No
//! run-owned temp even exists to clean ahead of the P3.20 naming model + the P3.21 lock-before-part
//! lifecycle, and no caller reaches these ahead of their fill-box (`cleanup_run` wires at P3.74, the
//! sweep at startup with P3.23). Signature AND body land together in each fill-box:
//!  - `cleanup_item` / `cleanup_run` — own-prefix-scoped cleanup on every exit path
//!    (`.convertia-<thisInstanceId>-<thisRunId>-*.part`, never a bare `*.part` glob, §2.6.2) — **P3.22**
//!    (the `CleanupResidue` honesty leg is **P3.25**).
//!  - `sweep_stale` — startup sweep, the held lock as the SOLE delete gate, non-blocking try-lock
//!    (§2.6.3) — **P3.23** (the opportunistic destination-resident `*.part` reclaim is **P3.24**).
//!  - the publish-temp naming + ownership model — [`PublishTemp`]
//!    (`.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part`, a dotfile SIBLING on `final`'s volume,
//!    §2.6.1 / §2.14.1 / §3.5.6) is **P3.20 (built below)**: `create_in` allocates the kind-1 publish temp,
//!    `run_prefix` is the §2.6.2 run-scoped own-prefix `cleanup_run` matches by (never a bare `*.part`
//!    glob), and `parse` reads a sibling's owning `(InstanceId, RunId, JobId)` back so the §2.6.3
//!    cross-instance reclaim can address its exact lock. The lock-before-part START ordering (mint
//!    `RunId` -> create `run-<RunId>/` -> OS-lock `.lock` -> only THEN the first `*.part`, the premise
//!    making "absent lock => dead => reclaimable" safe, §2.6.3) is **P3.21** — no ordering typestate is
//!    seated here yet, that shape is P3.21's own.

use std::ffi::OsStr;
use std::io;
use std::path::Path;

use tempfile::TempPath;
use uuid::Uuid;

use crate::domain::{InstanceId, JobId, RunId};

/// The shared `.convertia-` grammar prefix of every kind-1 publish temp name (§2.14.1). The leading `.`
/// makes it a dotfile so it does not clutter the destination directory's normal listing.
const PUBLISH_TEMP_PREFIX: &str = ".convertia-";
/// The shared `.part` grammar suffix (§2.14.1). The §2.1.2 atomic publish renames this away on success
/// (so drop is a no-op on the success path); a leftover `.part` is discardable residue (§2.6).
const PUBLISH_TEMP_SUFFIX: &str = ".part";
/// A hyphenated UUID is exactly 36 ASCII chars (`8-4-4-4-12`) — the fixed width [`PublishTemp::parse`]
/// splits the two UUID fields by, since the UUIDs' own internal hyphens make a naive `-`-split ambiguous.
const HYPHENATED_UUID_LEN: usize = 36;

/// The ownership identity encoded in a kind-1 publish temp's NAME (§2.6.1 / §2.14.1): a uniquely-named
/// dotfile SIBLING of `final` in the destination directory,
/// `…/<dest_dir>/.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part`, on `final`'s volume by construction
/// (the §2.14.1 same-volume rule, so the §2.1.2 publish is a true intra-volume exclusive rename). Encoding
/// `(InstanceId, RunId, JobId)` in the name is what lets cleanup (§2.6.2 `cleanup_run`, P3.22; §2.6.3
/// opportunistic cross-instance reclaim, P3.24) (a) tell its OWN temps from a concurrent instance's and
/// (b) resolve the EXACT owning lock `…/scratch/<InstanceId>.*/run-<RunId>/.lock` from the filename alone —
/// **never** a bare `*.part` / `.convertia-*.part` glob (the §2.6.2 CRITICAL own-prefix scope, so a
/// concurrent foreign instance's LIVE temp is never deleted). The `<rand>` is `tempfile`'s
/// collision-avoiding random component (hyphen-free ASCII), owned by [`create_in`](Self::create_in), not
/// by this identity — two temps of the SAME `(instance, run, job)` still get distinct names.
///
/// The type + fields stay live via the `#[derive]`d `Debug`/`PartialEq` impls (which read every field);
/// only the inherent methods below are statically dead in the production build until their §2.1.1 /
/// §2.6.2 / §2.6.3 wiring lands (each carries its own auto-flagging dead-code lint-expectation —
/// P3.38/P3.43 for creation, P3.22/P3.24 for cleanup).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublishTemp {
    /// The owning launch instance (§7.1.2) — the §2.6.3 reclaim addresses `…/scratch/<InstanceId>.*/…`.
    instance: InstanceId,
    /// The owning run (§7.1.2) — the §2.6.3 reclaim addresses the exact `…/run-<RunId>/.lock`.
    run: RunId,
    /// The owning item/job (§0.6) — the §2.6.2 per-item cleanup identity.
    job: JobId,
}

impl PublishTemp {
    /// Bind the `(InstanceId, RunId, JobId)` an in-flight kind-1 publish temp is owned by (§2.6.1): the
    /// per-launch `InstanceId` singleton (§7.1.2), the per-run `RunId` mint (C6 accept, §7.1.2), and the
    /// item's `JobId` (= `ItemId`, §0.6). [Build-Session-Entscheidung: P3.20]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "P3.20 — constructed by the §2.1.1 write sequence (P3.38) / §3.5.6 native-engine \
                      out_tmp (P3.43) just before `create_in`; unused in production until then."
        )
    )]
    #[must_use]
    pub fn new(instance: InstanceId, run: RunId, job: JobId) -> Self {
        Self { instance, run, job }
    }

    /// Create the §2.14.1 kind-1 publish temp: a `.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part`
    /// dotfile SIBLING inside `dest_dir`. Placing it in the destination directory itself puts it on
    /// `final`'s volume BY CONSTRUCTION, so the §2.1.2 publish is a true intra-volume exclusive rename
    /// (§2.14.1 same-volume rule). `tempfile` creates the file EXCLUSIVELY (`O_EXCL`) with a fresh random
    /// component, `0o600` on POSIX (owner-only — a per-run temp may briefly hold decoded bytes, §2.14.1
    /// mode bits) / under the per-user profile ACL on Windows, and returns a [`TempPath`] (deleted on
    /// drop). The engine writes to it and the §2.1 atomic publish CONSUMES it on success (the rename moves
    /// it to `final`), so drop is a no-op on the success path and the discard-on-drop covers only the
    /// cancel/fail path (§3.5.6 / §2.6.2). `dest_dir` MUST be the already-writability-verified destination
    /// — §2.7.2 has diverted a non-writable one BEFORE §2.14 places the temp. [Build-Session-Entscheidung: P3.20]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "P3.20 — the production caller is the §2.1.1 write sequence pick-temp step (P3.38) + \
                      the §3.5.6 native-engine out_tmp (P3.43); unused in production until that wiring lands."
        )
    )]
    pub fn create_in(&self, dest_dir: &Path) -> io::Result<TempPath> {
        // The tempfile prefix is the full per-job own-anchor `.convertia-<InstanceId>-<RunId>-<jobId>-`;
        // tempfile appends its hyphen-free random component and the `.part` suffix, yielding exactly the
        // §2.14.1 name `.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part`.
        let prefix = format!(
            "{PUBLISH_TEMP_PREFIX}{}-{}-{}-",
            self.instance.as_uuid(),
            self.run.as_uuid(),
            self.job.as_u32()
        );
        let file = tempfile::Builder::new()
            .prefix(&prefix)
            .suffix(PUBLISH_TEMP_SUFFIX)
            .tempfile_in(dest_dir)?;
        Ok(file.into_temp_path())
    }

    /// The §2.6.2 run-scoped own-prefix `.convertia-<InstanceId>-<RunId>-` — every `<jobId>` publish temp
    /// of ONE run shares it. `cleanup_run` (P3.22) removes a run's own temps by matching this prefix + the
    /// `.part` suffix in each RECORDED `final_dir`, **never** a bare `*.part` / `.convertia-*.part` glob
    /// (which would delete a concurrent foreign instance's LIVE temp — the §2.6.2 CRITICAL rule). It is
    /// job-INDEPENDENT (an associated fn of `(instance, run)`, not `&self`) because run-end cleanup spans
    /// every job of the run. [Build-Session-Entscheidung: P3.20]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "P3.20 — the production caller is `cleanup_run`'s own-prefix match (P3.22); unused in \
                      production until then."
        )
    )]
    #[must_use]
    pub fn run_prefix(instance: InstanceId, run: RunId) -> String {
        format!(
            "{PUBLISH_TEMP_PREFIX}{}-{}-",
            instance.as_uuid(),
            run.as_uuid()
        )
    }

    /// Parse a publish-temp FILE NAME back into the `(InstanceId, RunId, JobId)` it is owned by, `None` if
    /// it is not a well-formed run publish temp (`.convertia-<uuid>-<uuid>-<u32>-<rand>.part`). This is the
    /// §2.6.1 "resolve ownership from the name alone": the §2.6.3 cross-instance opportunistic reclaim
    /// (P3.24) reads a sibling `.convertia-*.part`'s owning `(InstanceId, RunId)` from here to address its
    /// exact lock `…/scratch/<InstanceId>.*/run-<RunId>/.lock` — a HELD lock ⇒ live ⇒ keep; free/absent ⇒
    /// dead ⇒ reclaim. **Panic-free** (the crate no-panic deny, G4): every step is a fallible
    /// `Option`/`Result` short-circuit, so a hostile or foreign sibling name yields `None`, never a panic
    /// — the §2.6.2 "non-matching ⇒ never our delete" safety. Fixed-width UUID fields disambiguate the
    /// UUIDs' own internal hyphens; the `<jobId>-<rand>` tail splits on its FIRST `-` (the `<jobId>` is
    /// digits-only, so it carries none). The `<rand>` is discarded — ownership IS the triple.
    /// [Build-Session-Entscheidung: P3.20]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "P3.20 — the production caller is the §2.6.3 cross-instance opportunistic reclaim \
                      (P3.24) that reads a sibling `.part`'s owning (InstanceId, RunId); unused in \
                      production until then."
        )
    )]
    #[must_use]
    pub fn parse(file_name: &OsStr) -> Option<Self> {
        let name = file_name.to_str()?;
        let body = name
            .strip_prefix(PUBLISH_TEMP_PREFIX)?
            .strip_suffix(PUBLISH_TEMP_SUFFIX)?;
        let instance_str = body.get(..HYPHENATED_UUID_LEN)?;
        let rest = body.get(HYPHENATED_UUID_LEN..)?.strip_prefix('-')?;
        let run_str = rest.get(..HYPHENATED_UUID_LEN)?;
        let tail = rest.get(HYPHENATED_UUID_LEN..)?.strip_prefix('-')?;
        // `<jobId>` is ASCII digits (no `-`), so the FIRST `-` is the jobId/rand boundary even though the
        // hyphen-free `<rand>` follows; `split_once` discards the rand.
        let (job_str, _rand) = tail.split_once('-')?;
        let instance = InstanceId::from_uuid(Uuid::parse_str(instance_str).ok()?);
        let run = RunId::from_uuid(Uuid::parse_str(run_str).ok()?);
        let job = JobId::from_index(job_str.parse::<u32>().ok()?);
        Some(Self { instance, run, job })
    }

    /// The owning launch instance (§2.6.1) — the §2.6.3 reclaim addresses `…/scratch/<InstanceId>.*/…` by
    /// it. [Build-Session-Entscheidung: P3.20]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "P3.20 — read by the §2.6.3 cross-instance reclaim's lock addressing (P3.24); unused \
                      in production until then."
        )
    )]
    #[must_use]
    pub fn instance(&self) -> InstanceId {
        self.instance
    }

    /// The owning run (§2.6.1) — the §2.6.3 reclaim addresses the exact `…/run-<RunId>/.lock` by it.
    /// [Build-Session-Entscheidung: P3.20]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "P3.20 — read by the §2.6.3 cross-instance reclaim's lock addressing (P3.24); unused \
                      in production until then."
        )
    )]
    #[must_use]
    pub fn run(&self) -> RunId {
        self.run
    }

    /// The owning item/job (§0.6) — the §2.6.2 per-item cleanup identity. [Build-Session-Entscheidung: P3.20]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "P3.20 — read by the §2.6.2 per-item cleanup identity (P3.22); unused in production \
                      until then."
        )
    )]
    #[must_use]
    pub fn job(&self) -> JobId {
        self.job
    }
}

#[cfg(test)]
mod publish_temp_tests {
    use super::*;

    // §6.4.1 unit (G15) / §2.14.1: `create_in` lands the kind-1 publish temp as a dotfile SIBLING inside
    // the destination dir (same volume by construction) whose name is exactly
    // `.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part` — the real-temp-FS creation read back (never a
    // mocked FS, test-strategy §0.1).
    #[test]
    fn create_in_places_a_named_part_sibling_in_the_destination_dir() {
        let dir = tempfile::tempdir().expect("a real temp destination dir");
        let instance = InstanceId::mint();
        let run = RunId::mint();
        let job = JobId::from_index(7);
        let temp = PublishTemp::new(instance, run, job)
            .create_in(dir.path())
            .expect("create the publish temp");

        assert_eq!(
            temp.parent(),
            Some(dir.path()),
            "§2.14.1: the publish temp is a SIBLING inside the destination dir (on final's volume)"
        );
        assert!(
            temp.exists(),
            "§3.5.6: tempfile exclusively CREATED the .part file the engine writes to"
        );
        let name = temp
            .file_name()
            .and_then(|n| n.to_str())
            .expect("the temp has a UTF-8 file name");
        assert!(
            name.starts_with(&format!("{}7-", PublishTemp::run_prefix(instance, run))),
            "§2.14.1: name is `.convertia-<InstanceId>-<RunId>-<jobId>-…`, got {name}"
        );
        assert!(
            name.ends_with(PUBLISH_TEMP_SUFFIX),
            "§2.14.1: the publish temp carries the `.part` suffix, got {name}"
        );
    }

    // §6.4.1 unit (G15) / §2.6.1: the name round-trips — parsing a created temp's name recovers its exact
    // (InstanceId, RunId, JobId) ownership triple, so the §2.6.3 cross-instance reclaim can read a
    // sibling's owner back from the filename alone.
    #[test]
    fn parse_round_trips_the_ownership_triple() {
        let dir = tempfile::tempdir().expect("a real temp destination dir");
        let owner = PublishTemp::new(InstanceId::mint(), RunId::mint(), JobId::from_index(42));
        let temp = owner
            .create_in(dir.path())
            .expect("create the publish temp");
        let name = temp.file_name().expect("the temp has a file name");

        let parsed = PublishTemp::parse(name).expect("a well-formed publish-temp name parses");
        assert_eq!(
            parsed, owner,
            "§2.6.1: parse recovers the exact (InstanceId, RunId, JobId) the name encodes"
        );
        assert_eq!(
            parsed.instance(),
            owner.instance(),
            "owning instance recovered"
        );
        assert_eq!(parsed.run(), owner.run(), "owning run recovered");
        assert_eq!(parsed.job(), JobId::from_index(42), "owning job recovered");
    }

    // §6.4.1 unit (G15) / §2.14.1: the `<rand>` component makes two temps of the SAME (instance, run, job)
    // distinct — a second in-flight item never collides onto the first's `.part` (exclusive-create + rand).
    #[test]
    fn same_owner_two_temps_get_distinct_names() {
        let dir = tempfile::tempdir().expect("a real temp destination dir");
        let owner = PublishTemp::new(InstanceId::mint(), RunId::mint(), JobId::from_index(0));
        let a = owner.create_in(dir.path()).expect("first temp");
        let b = owner.create_in(dir.path()).expect("second temp");
        assert_ne!(
            a.file_name(),
            b.file_name(),
            "§2.14.1: the random component gives each publish temp a distinct name (no collision)"
        );
    }

    // §6.4.1 unit (G15) / §2.6.2: the run-scoped own-prefix is job-INDEPENDENT and instance+run-SPECIFIC —
    // every job of one run shares `.convertia-<InstanceId>-<RunId>-`, and a different run has a different
    // prefix, so `cleanup_run`'s own-prefix match never spans a foreign run's temps.
    #[test]
    fn run_prefix_is_shared_across_jobs_and_specific_per_run() {
        let dir = tempfile::tempdir().expect("a real temp destination dir");
        let instance = InstanceId::mint();
        let run = RunId::mint();
        let prefix = PublishTemp::run_prefix(instance, run);

        for job in [0u32, 1, 4_294_967_295] {
            let temp = PublishTemp::new(instance, run, JobId::from_index(job))
                .create_in(dir.path())
                .expect("create the publish temp");
            let name = temp
                .file_name()
                .and_then(|n| n.to_str())
                .expect("UTF-8 name");
            assert!(
                name.starts_with(&prefix),
                "§2.6.2: every job of the run shares the run own-prefix, got {name}"
            );
        }
        let other_run = RunId::mint();
        assert_ne!(
            PublishTemp::run_prefix(instance, other_run),
            prefix,
            "§2.6.2: a different run has a different own-prefix (cleanup never spans a foreign run)"
        );
    }

    // §6.4.1 unit (G15) / §2.6.2: `parse` REJECTS anything that is not a well-formed run publish temp — a
    // plain output name, the ad-hoc `.convertia-tmp.part` the fs_guard tests use, a bad UUID, a missing
    // `.part`, and a truncated body — so a hostile/foreign sibling name never resolves to a bogus owner
    // (the "non-matching ⇒ never our delete" safety). Panic-free by construction (crate no-panic deny).
    #[test]
    fn parse_rejects_malformed_and_foreign_names() {
        use std::ffi::OsStr;
        let real = {
            let dir = tempfile::tempdir().expect("temp dir");
            let temp = PublishTemp::new(InstanceId::mint(), RunId::mint(), JobId::from_index(3))
                .create_in(dir.path())
                .expect("create");
            temp.file_name()
                .and_then(|n| n.to_str())
                .expect("UTF-8 name")
                .to_owned()
        };
        // A valid publish-temp name with its `.part` suffix dropped — a well-formed head that must still
        // NOT parse (the suffix is part of the grammar), built without any string indexing.
        let head_without_suffix = real
            .strip_suffix(PUBLISH_TEMP_SUFFIX)
            .expect("a created temp name ends with the .part suffix")
            .to_owned();
        for bad in [
            "data.tsv",                                // not a publish temp at all
            ".convertia-tmp.part", // the fs_guard ad-hoc test temp — NOT a real owner
            ".convertia-not-a-uuid-here-x-7-abc.part", // ill-formed UUID fields
            &head_without_suffix,  // valid head, `.part` dropped
            ".convertia-.part",    // empty body
            "",                    // empty name
        ] {
            assert!(
                PublishTemp::parse(OsStr::new(bad)).is_none(),
                "§2.6.2: a non-conforming name must NOT parse to an owner: {bad:?}"
            );
        }
    }

    // §6.4.1 unit (G15) / §2.14.1: on POSIX the kind-1 publish temp is created owner-only `0o600` — a
    // per-run temp that may briefly hold decoded bytes is never world-readable in a shared dir. (The full
    // temp-ownership security invariant suite, incl. the Windows DACL leg, is P3.71.)
    #[cfg(unix)]
    #[test]
    fn create_in_is_owner_only_0o600_on_posix() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("a real temp destination dir");
        let temp = PublishTemp::new(InstanceId::mint(), RunId::mint(), JobId::from_index(1))
            .create_in(dir.path())
            .expect("create the publish temp");
        let mode = std::fs::metadata(&temp)
            .expect("stat the publish temp")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o600,
            "§2.14.1: the kind-1 publish temp is owner-only 0o600 on POSIX"
        );
    }
}
