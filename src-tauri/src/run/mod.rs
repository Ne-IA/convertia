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
//!    cross-instance reclaim can address its exact lock (`create_in` is module-private — the run `.part`
//!    is minted ONLY through the P3.21 [`RunScratch`] lock gate, below). The lock-before-part START
//!    ordering (mint `RunId` -> create `run-<RunId>/` -> OS-lock `.lock` -> only THEN the first `*.part`,
//!    the premise making "absent lock => dead => reclaimable" safe, §2.6.3) is **P3.21 (built below)**:
//!    the [`RunScratch`] typestate SEATS it — `acquire` creates the `0o700` run dir and takes the HELD
//!    exclusive lock, and its `publish_temp` is the SOLE lock-after `.part` mint, so a `.part` is
//!    structurally unreachable before the lock is held.

use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

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
    // [Test-Change: P3.21 - old-obsolete+new-correct, §2.6.3] flip to allow: P3.21's dead-walked
    // RunScratch::publish_temp now marks this P3.20 callee used, so the old dead-code attribute is obsolete
    // and allow is correct (this is a lint-attribute flip, not a real assertion change — the G70 signal FPs
    // on the removed dead_code line).
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "P3.20 — now called by the P3.21 RunScratch::publish_temp (the lock-after mint entry), \
                      itself dead in production until the §2.1.1 write sequence (P3.38) / §3.5.6 \
                      native-engine out_tmp (P3.43) wire it; rustc walks that dead-but-present caller and \
                      marks this callee USED, so `allow` (permissive) covers the transitive dead-ness \
                      through the P3 wiring window (the platform WindowsRenameOutcome pattern)."
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
    // [Test-Change: P3.21 - old-obsolete+new-correct, §2.6.3] flip to allow: P3.21's dead-walked
    // RunScratch::publish_temp now marks this P3.20 callee used, so the old dead-code attribute is obsolete
    // and allow is correct (a lint-attribute flip, not a real assertion change — the G70 signal FPs on the
    // removed dead_code line).
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "P3.20 — now called by the P3.21 RunScratch::publish_temp (the lock-after mint entry), \
                      itself dead in production until the §2.1.1 write sequence (P3.38) / §3.5.6 \
                      native-engine out_tmp (P3.43) wire it; rustc walks that dead-but-present caller and \
                      marks this callee USED, so `allow` (permissive) covers the transitive dead-ness \
                      through the P3 wiring window (the platform WindowsRenameOutcome pattern)."
        )
    )]
    fn create_in(&self, dest_dir: &Path) -> io::Result<TempPath> {
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

/// The literal `convertia/` namespace + `scratch/` component the per-run scratch dir lives under, below
/// the §2.14 scratch base (`app_local_data_dir()`): the full run dir is
/// `<scratch_base>/convertia/scratch/<InstanceId>.<pid>/run-<RunId>/`, and the §2.6.3 startup sweep (P3.23)
/// globs exactly `convertia/scratch/<*>.<*>/run-*` under the same base — so these two constants are the
/// single home of the shared literal the assembler (here) and the sweeper (P3.23) must agree on.
const SCRATCH_NAMESPACE: &str = "convertia";
const SCRATCH_SUBDIR: &str = "scratch";

/// §2.6.3 run-lifecycle: a LIVE, LOCK-HELD run's scratch home — the structural encoding of the
/// **lock-before-part ordering**. Constructing one ([`acquire`](Self::acquire)) performs the run-start
/// sequence strictly in order: (1) create the per-run scratch dir
/// `<scratch_base>/convertia/scratch/<InstanceId>.<pid>/run-<RunId>/` (`0o700` owner-only on POSIX,
/// §2.14.1), (2) create + open `run-<RunId>/.lock`, (3) take a HELD exclusive advisory lock on it
/// ([`crate::platform::acquire_exclusive_lock`]). Because a kind-1 publish temp can be minted ONLY through
/// [`publish_temp`](Self::publish_temp) on an already-constructed `RunScratch`, the §2.6.3 invariant "the
/// `run-<RunId>/.lock` exists and is HELD before the run writes its first `.part`" is made **structural**,
/// not conventional — the premise that makes "absent lock ⇒ dead ⇒ reclaimable" SAFE (a live in-progress
/// `.part` can never coexist with an absent lock, so a concurrent sweeper that finds the lock held keeps
/// the live `.part`, §2.6.3). The lock is held for the run's whole lifetime and released when the
/// `RunScratch` is dropped (its `.lock` `File` closes). A §6 property-test target.
///
/// Unlike [`PublishTemp`], this type carries NO field-reading derives (it owns a live `File`), so it is
/// genuinely dead in the production build until its C6-accept run-start wiring lands (P3.46 / the §2.1.1
/// write sequence P3.38) — the struct + its methods are forward-declared dead through the P3-wiring
/// window, each carrying its own dead-code lint attribute.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "P3.21 run-lifecycle lock-before-part typestate; the runtime caller is the C6-accept run \
                  start (P3.46) / the §2.1.1 write sequence (P3.38). The struct is constructed only by its \
                  own `acquire` (itself dead in production until then), and rustc walks that dead-but-present \
                  constructor and marks the struct USED, so a dead_code EXPECTATION would be unfulfilled; \
                  `allow` (permissive) covers the transitive dead-ness through the P3 wiring window (the \
                  platform WindowsRenameOutcome pattern)."
    )
)]
pub struct RunScratch {
    /// The owning launch instance (§7.1.2) — stamped into every publish temp's §2.6.1 ownership.
    instance: InstanceId,
    /// The owning run (§7.1.2) — stamped into every publish temp's §2.6.1 ownership.
    run: RunId,
    /// The per-run scratch dir `…/run-<RunId>/` (the kind-2 working-file root, §2.14.2; removed by the
    /// §2.6.2 run-end `cleanup_run`, P3.22).
    dir: PathBuf,
    /// The held `run-<RunId>/.lock` handle — kept OPEN for the run's lifetime; dropping it releases the OS
    /// advisory lock (§2.6.3). An RAII guard: its VALUE is the OS-held lock, so the field is intentionally
    /// never read (holding the handle open is the whole effect).
    #[cfg_attr(
        test,
        allow(
            dead_code,
            reason = "§2.6.3 RAII lock guard: the `.lock` File is held OPEN for the run's lifetime so the OS \
                      advisory lock stays taken (released when this field drops); its effect is the held \
                      lock, so the handle itself is intentionally never read."
        )
    )]
    _lock: File,
}

impl RunScratch {
    /// §2.6.3 run start (the lock-before-part ordering): assemble the per-run scratch dir under
    /// `scratch_base` (the §2.14 `app_local_data_dir()` base), create it `0o700` on POSIX (§2.14.1),
    /// create + open `run-<RunId>/.lock`, and take the HELD exclusive advisory lock on it — returning a
    /// `RunScratch` that PROVES the lock is held. `pid` is the current process id (a §7.1.2 human-readable
    /// label in the dir name, never the liveness gate — that is the held lock). Any I/O failure (dir
    /// create, lock open, lock acquire) is a structured `Err`, never a panic. [Build-Session-Entscheidung: P3.21]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "P3.21 run-start lock-before-part sequence; the production caller is the C6-accept run \
                      start (P3.46) / the §2.1.1 write sequence (P3.38) — unused in the production build \
                      until that wiring lands."
        )
    )]
    pub fn acquire(
        scratch_base: &Path,
        instance: InstanceId,
        pid: u32,
        run: RunId,
    ) -> io::Result<Self> {
        let dir = scratch_base
            .join(SCRATCH_NAMESPACE)
            .join(SCRATCH_SUBDIR)
            .join(instance.scratch_root_segment(pid))
            .join(run.run_subdir_segment());
        // (1) Create the per-run scratch dir tree. On POSIX it is owner-only `0o700` (§2.14.1: a per-run
        // scratch must never be world-readable in a shared data dir); `recursive` tolerates a pre-existing
        // `convertia/scratch/<InstanceId>.<pid>/` from an earlier run of this instance, and the fresh
        // unique `run-<RunId>/` leaf is newly created at the restricted mode. On Windows the scratch lives
        // under the per-user profile, whose default ACL is the §2.14.1 equivalent (the DACL leg is P3.71.2).
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&dir)?;
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir_all(&dir)?;
        }
        // (2) Create/open the `.lock`, then (3) take the HELD exclusive lock BEFORE any `.part` is written
        // (the §2.6.3 lock-before-part ordering). The `File` is retained so the lock lives for the run.
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join(".lock"))?;
        crate::platform::acquire_exclusive_lock(&lock)?;
        Ok(Self {
            instance,
            run,
            dir,
            _lock: lock,
        })
    }

    /// The per-run scratch dir `…/run-<RunId>/` (the §2.14.2 kind-2 engine-working-file root; the §2.6.2
    /// run-end `cleanup_run` removes it). [Build-Session-Entscheidung: P3.21]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "P3.21 — the kind-2 scratch-dir accessor; read by §2.14.2 engine working-file placement \
                      + the §2.6.2 run-end cleanup (P3.22) — unused in production until then."
        )
    )]
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Mint a kind-1 publish temp for `job` in `dest_dir` — the ONLY way to create one, so it provably
    /// happens AFTER the run lock is held (the §2.6.3 lock-before-part ordering, made structural).
    /// Delegates to the P3.20 naming model ([`PublishTemp::create_in`]), stamping this run's §2.6.1
    /// ownership. [Build-Session-Entscheidung: P3.21]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "P3.21 — the sole lock-after publish-temp entry; the production caller is the §2.1.1 \
                      write sequence (P3.38) / §3.5.6 native-engine out_tmp (P3.43) — unused in production \
                      until then."
        )
    )]
    pub fn publish_temp(&self, dest_dir: &Path, job: JobId) -> io::Result<TempPath> {
        PublishTemp::new(self.instance, self.run, job).create_in(dest_dir)
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

#[cfg(test)]
mod run_scratch_tests {
    use super::*;

    // §6.4.3 real-FS (G15/G31) / §2.6.3: run start acquires a locked scratch — the per-run dir + `.lock`
    // are created under `convertia/scratch/<InstanceId>.<pid>/run-<RunId>/`, and ONLY the locked scratch
    // can mint a kind-1 publish temp (lock-before-part made structural), which lands in the DESTINATION
    // dir owned by (instance, run, job). Real temp FS, never mocked (test-strategy §0.1).
    #[test]
    fn acquire_creates_a_locked_scratch_and_mints_owned_publish_temps() {
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let dest = tempfile::tempdir().expect("a real destination dir");
        let instance = InstanceId::mint();
        let run = RunId::mint();
        let scratch = RunScratch::acquire(base.path(), instance, std::process::id(), run)
            .expect("run start acquires the locked scratch");

        assert!(
            scratch.dir().is_dir(),
            "§2.6.3: the per-run scratch dir was created"
        );
        assert!(
            scratch.dir().join(".lock").is_file(),
            "§2.6.3: run-<RunId>/.lock was created"
        );
        assert_eq!(
            scratch.dir().file_name().and_then(|n| n.to_str()),
            Some(run.run_subdir_segment().as_str()),
            "§2.14: the scratch-dir leaf is exactly run-<RunId>"
        );
        // The scratch dir sits under the shared `convertia/scratch/` namespace (what the P3.23 sweep globs).
        assert!(
            scratch
                .dir()
                .starts_with(base.path().join(SCRATCH_NAMESPACE).join(SCRATCH_SUBDIR)),
            "§2.6.3: the run dir is under <base>/convertia/scratch/ (the sweep glob's root)"
        );

        // Lock-before-part, structural: `publish_temp` is the only way to mint a `.part`, so it provably
        // runs AFTER the lock is held. The temp lands in the DESTINATION dir (§2.14.1), not the scratch dir.
        let temp = scratch
            .publish_temp(dest.path(), JobId::from_index(9))
            .expect("a locked run mints a publish temp");
        assert_eq!(
            temp.parent(),
            Some(dest.path()),
            "§2.14.1: the publish temp is a sibling in the destination dir, not the scratch dir"
        );
        let owner = PublishTemp::parse(temp.file_name().expect("the temp has a name"))
            .expect("the minted temp name parses");
        assert_eq!(
            owner,
            PublishTemp::new(instance, run, JobId::from_index(9)),
            "§2.6.1: the minted publish temp carries this run's exact ownership"
        );
    }

    // §6.4.3 real-FS (G15/G31) / §2.14.1: the per-run scratch dir is owner-only `0o700` on POSIX — a
    // per-run scratch that may hold decoded engine working bytes is never world-readable. (The full
    // temp-ownership suite incl. the Windows DACL leg is P3.71.)
    #[cfg(unix)]
    #[test]
    fn run_dir_is_owner_only_0o700_on_posix() {
        use std::os::unix::fs::PermissionsExt;
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let scratch = RunScratch::acquire(
            base.path(),
            InstanceId::mint(),
            std::process::id(),
            RunId::mint(),
        )
        .expect("acquire");
        let mode = std::fs::metadata(scratch.dir())
            .expect("stat the run dir")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o700,
            "§2.14.1: the per-run scratch dir is owner-only 0o700 on POSIX"
        );
    }

    // §6.4.3 real-FS (G15/G31) / §2.6.3: the run's exclusive advisory lock is ACTUALLY HELD for the run's
    // lifetime and RELEASED on drop — proven with a safe `rustix` non-blocking probe from a second file
    // description (flock conflicts across descriptions of the same file, even in one process). This is the
    // "held ⇒ live ⇒ keep; absent/free ⇒ dead ⇒ reclaimable" premise the §2.6.3 sweep (P3.23) rests on.
    // (The Windows held-lock semantics are exercised by the P3.23 sweep's non-blocking try-lock + the §6
    // property test — `crate::run` carries no `unsafe`, so it cannot probe `LockFileEx` directly here.)
    #[cfg(unix)]
    #[test]
    fn the_exclusive_lock_is_held_then_released_on_drop() {
        use rustix::fs::{flock, FlockOperation};
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let scratch = RunScratch::acquire(
            base.path(),
            InstanceId::mint(),
            std::process::id(),
            RunId::mint(),
        )
        .expect("acquire");
        let probe = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(scratch.dir().join(".lock"))
            .expect("re-open the .lock as an independent description");

        assert!(
            flock(&probe, FlockOperation::NonBlockingLockExclusive).is_err(),
            "§2.6.3: the run's exclusive lock is HELD — a second non-blocking acquire is refused"
        );

        drop(scratch);
        assert!(
            flock(&probe, FlockOperation::NonBlockingLockExclusive).is_ok(),
            "§2.6.3: dropping the run releases the lock (absent/free ⇒ dead ⇒ reclaimable)"
        );
    }
}
