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
//!    (`.convertia-<thisInstanceId>-<thisRunId>-*.part`, never a bare `*.part` glob, §2.6.2) — **P3.22
//!    (built below)**: `cleanup_item` removes one item's `.part` on the failure / out-of-disk / link-fallback
//!    exit paths; `cleanup_run` removes the run's OWN-prefix temps in every recorded `final_dir` then tears
//!    down the `run-<RunId>/` dir, returning the residue paths it could not remove. The `CleanupResidue`
//!    honesty leg — mapping that residue into `RunResult.cleanup_incomplete` (§2.6.4) — is **P3.25**.
//!  - `sweep_stale` — startup sweep, the held lock as the SOLE delete gate, non-blocking try-lock
//!    (§2.6.3) — **P3.23 (built below)**: globs `convertia/scratch/<*>.<*>/run-*` across all instance dirs,
//!    probes each `run-<RunId>/.lock` via the [`crate::platform`] non-blocking try-lock, and removes only DEAD
//!    dirs (free lock, or a stale lockless dir past the create-then-not-yet-locked grace window) — a live
//!    (held-lock) or just-starting run is left untouched. The opportunistic destination-resident `*.part`
//!    reclaim (§2.6.3 (b)) is **P3.24**.
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

use std::collections::BTreeSet;
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
    // [Test-Change: P3.22 - old-obsolete+new-correct, §2.6.2] flip to allow: P3.22's `cleanup_run` now calls
    // run_prefix (its own-prefix match), so rustc walks that dead-but-present caller and marks this callee
    // USED — the old dead-code EXPECTATION is obsolete and allow is correct (a lint-attribute flip, not a real
    // assertion change — the G70 signal FPs on the changed dead_code line).
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "P3.20 — now called by P3.22's `cleanup_run` own-prefix match (itself dead in production \
                      until the P3.74 run-lifecycle wiring); rustc walks that dead-but-present caller and \
                      marks this callee used, so `allow` (permissive) covers the transitive dead-ness through \
                      the P3 wiring window (the `create_in` pattern)."
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

/// The `run-` dir-name prefix of a per-run scratch dir (`run-<RunId>`, [`RunId::run_subdir_segment`] /
/// §2.14) — what the §2.6.3 sweep (`sweep_stale`, P3.23) matches under each `<InstanceId>.<pid>` instance dir.
const RUN_DIR_PREFIX: &str = "run-";
/// The per-run advisory-lock filename inside `run-<RunId>/` — the SINGLE home of the literal the writer
/// ([`RunScratch::acquire`], P3.21) and the §2.6.3 sweeper ([`sweep_stale`], P3.23) must agree on, so they
/// can never drift on which file carries the run's liveness lock (a drift would silently break the sweep's
/// held-lock delete gate).
const RUN_LOCK_FILE: &str = ".lock";
/// §2.6.3 create-then-not-yet-locked grace window: a **lockless** run dir younger than this is treated as a
/// just-starting run whose `.lock` is still absent (the tiny window between `mkdir run-<RunId>/` and the
/// lock-before-part acquire) and is LEFT for a subsequent sweep; older-and-lockless is a crash before that
/// step ⇒ dead ⇒ reclaimable. This window governs ONLY the not-held case — a dir with a HELD `.lock` is decided
/// by the held lock, the SOLE delete gate (§2.6.3), never by mtime. The **10 s** value is a build-session
/// choice — §2.6.3 specifies only "a short grace window / created in the last few seconds"; 10 s comfortably
/// covers the sub-millisecond `mkdir → open(.lock) → OS-lock` acquire window with ample margin for a slow /
/// loaded disk, while a stale crashed run (minutes/hours old by the next launch) is far past it and reclaimed.
/// [Build-Session-Entscheidung: P3.23]
const LOCKLESS_GRACE: std::time::Duration = std::time::Duration::from_secs(10);

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
            .open(dir.join(RUN_LOCK_FILE))?;
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

/// §2.6.2 item-exit cleanup: explicitly remove ONE item's kind-1 publish temp on an item-level exit path.
/// **Item failure** (engine error / corrupt input), **out-of-disk mid-write** (the partial `tmp`), and the
/// **`link`+`unlink` success fallback** (the hardlink published `tmp→final`, so the `*.part` original is
/// removed) all reduce to "remove this item's `.part`". The **single-call success path does NOT call this**
/// (the atomic rename already CONSUMED the temp — nothing remains, §2.6.2 "Item success" row). Consuming the
/// [`TempPath`] and removing it EXPLICITLY — never a silent drop, which swallows the `io::Error` — is what
/// lets the §2.6.4 honesty leg (P3.25) surface a `CleanupResidue`: an `Err` here means the `.part` residue
/// remains at the caller-held path (a lock held by AV software, a permission flip), so the caller reports the
/// item honestly rather than as a clean success. An already-absent temp (`NotFound`) is a **clean, idempotent
/// success** — a double call or an externally-vanished temp is not a spurious failure. Panic-free (the crate
/// no-panic deny, G4): every step is a fallible `Result` short-circuit. [Build-Session-Entscheidung: P3.22]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "P3.22 — the item-exit cleanup entry; the production caller is the §2.1.1 write sequence \
                  (item failure / out-of-disk / link-fallback success, P3.38) and the §3.5.6 native-engine \
                  out_tmp path (P3.43) — unused in the production build until that wiring lands."
    )
)]
pub fn cleanup_item(tmp: TempPath) -> io::Result<()> {
    match tmp.close() {
        Ok(()) => Ok(()),
        // Already gone (a double cleanup, or an externally-removed temp) is a clean, idempotent success —
        // there is no residue to report (§2.6.4).
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// §2.6.2 run-end cleanup: at run end (any reason) remove this run's OWN leftover kind-1 publish temps in
/// every RECORDED `final_dir`, then tear down the central `run-<RunId>/` scratch dir.
///
/// **CRITICAL — own-prefix scope, never a bare `*.part` glob (§2.6.2 "Run end" row):** a recorded `final_dir`
/// can be SHARED with a concurrent foreign instance writing beside the same sources, so this removes **only**
/// files whose name carries this run's exact own prefix `.convertia-<thisInstanceId>-<thisRunId>-` plus the
/// `.part` suffix. A foreign instance's / foreign run's live in-progress `.part` (a different `InstanceId` or
/// `RunId`) is NEVER matched, honouring the SSOT *"cleanup never removes another instance's in-progress
/// file"*. A non-matching `.convertia-*.part` is left **untouched** here — its dead-foreign opportunistic
/// reclaim under the §2.6.3 held-lock guard is P3.24, not run-end.
///
/// `recorded_final_dirs` is the union of every DISTINCT `final_dir` an output actually landed in this run —
/// incl. §2.7.2 late-divert targets and §2.14.3 cross-volume intermediates, which can sit in dirs that are
/// neither a drop root nor the chosen destination. Recording it per written item is the caller's job (the
/// P3.74 run-lifecycle teardown); this fn only CONSUMES the set.
///
/// Run-end consumes the [`RunScratch`], **releasing its held `.lock` BEFORE** the `run-<RunId>/` tree is
/// removed — on Windows a still-open handle inside a dir blocks its recursive delete, and releasing the lock
/// first also lets a concurrent sweeper (§2.6.3, P3.23) see this run as dead. Returns the **residue**: the
/// paths whose removal FAILED — never a silent clean success; the §2.6.4 honesty leg (P3.25) maps these into
/// `CleanupResidue`. An empty return = a fully clean run-end. Panic-free (the crate no-panic deny, G4).
/// [Build-Session-Entscheidung: P3.22]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "P3.22 — the run-end cleanup entry; the production caller is the P3.74 run-lifecycle \
                  teardown — unused in the production build until that wiring lands."
    )
)]
pub fn cleanup_run(scratch: RunScratch, recorded_final_dirs: &BTreeSet<PathBuf>) -> Vec<PathBuf> {
    let RunScratch {
        instance,
        run,
        dir,
        _lock: lock,
    } = scratch;
    let own_prefix = PublishTemp::run_prefix(instance, run);
    let mut residue = Vec::new();

    // (1) Remove this run's OWN publish temps in every recorded destination dir — own-prefix scoped, never a
    // bare `*.part` glob (§2.6.2 CRITICAL). The run is still lock-held here; the destination temps live in
    // the destination dirs (not the scratch dir), so this is unaffected by the lock, which step (2) releases.
    for final_dir in recorded_final_dirs {
        remove_own_temps_in(final_dir, &own_prefix, &mut residue);
    }

    // (2) Release the held advisory lock, THEN remove the now-discardable central `run-<RunId>/` tree. The
    // lock `File` MUST drop before the delete: on Windows a still-open handle inside the dir blocks the
    // recursive remove; releasing it first also lets a concurrent sweeper (§2.6.3) see the run as dead.
    drop(lock);
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => {}
        // Already gone (a crash-recovery sweep raced us, or a prior partial teardown) is a clean success.
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(_) => residue.push(dir),
    }
    residue
}

/// Remove every kind-1 publish temp in `dir` whose name carries this run's exact `own_prefix`
/// (`.convertia-<InstanceId>-<RunId>-`) AND the `.part` suffix — the §2.6.2 own-prefix match. A foreign
/// instance's / foreign run's `.part` never matches this prefix, so it is never removed here (the SSOT
/// foreign-file safety). A removal that FAILS (a permission flip, a held lock) is recorded in `residue`; an
/// already-absent entry is skipped (idempotent).
///
/// **Cleanup-honesty on an un-enumerable dir (§2.6.4):** a `NotFound` `dir` is genuinely gone — its contents
/// went with it, so there is nothing to reclaim and nothing to report. But a `dir` that exists yet cannot be
/// LISTED (a permission flip, a read-only volume that went away) may still hold an own `.part` that is now
/// undeletable-because-unlistable, and a per-entry enumeration error means an own `.part` could be silently
/// skipped — §2.6.3 sanctions only a *crash* or a *wedged cancel* as silent lingering carve-outs, not this,
/// so both are surfaced by pushing `dir` itself into `residue` (never a silent clean success), mirroring the
/// `cleanup_item` / `remove_dir_all` `NotFound`-vs-other split. Panic-free (the crate no-panic deny, G4).
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "P3.22 — the run-end own-prefix removal helper; its only caller is `cleanup_run` (itself \
                  dead in production until the P3.74 wiring), which rustc walks as a present caller and marks \
                  this callee used, so `allow` (permissive) covers the transitive dead-ness through the P3 \
                  wiring window (the `PublishTemp::create_in` pattern)."
    )
)]
fn remove_own_temps_in(dir: &Path, own_prefix: &str, residue: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        // Genuinely gone — no contents to reclaim, nothing to report.
        Err(e) if e.kind() == io::ErrorKind::NotFound => return,
        // Exists but un-listable (permission flip / read-only volume gone): an own `.part` may linger here,
        // undeletable-because-unlistable — surface the dir as residue (§2.6.4), never a silent clean success.
        Err(_) => {
            residue.push(dir.to_path_buf());
            return;
        }
    };
    let mut enumeration_incomplete = false;
    for entry in entries {
        let Ok(entry) = entry else {
            // A per-entry read error: we cannot read this entry's name to tell whether it is ours, so an own
            // `.part` could be silently skipped — flag the dir as residue once, after the loop.
            enumeration_incomplete = true;
            continue;
        };
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if name.starts_with(own_prefix) && name.ends_with(PUBLISH_TEMP_SUFFIX) {
            let path = entry.path();
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(_) => residue.push(path),
            }
        }
    }
    if enumeration_incomplete {
        residue.push(dir.to_path_buf());
    }
}

/// §2.6.3 liveness of a run dir's `.lock`, as read by the non-blocking try-lock probe ([`probe_lock`]). The
/// three-state split (vs a bare bool) is a build-session decomposition so [`sweep_verdict`] can apply the
/// §2.6.3 (b) grace window to BOTH the `Free` and `Absent` not-held cases. [Build-Session-Entscheidung: P3.23]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "P3.23 — the §2.6.3 sweep's liveness enum; constructed only by `probe_lock` and read only by \
                  `sweep_verdict`, both reached solely from `sweep_stale` (dead in production until the §7.2 \
                  startup wiring), which rustc walks as present callers marking these variants used — `allow` \
                  covers the transitive dead-ness through the P3 wiring window (the `create_in` pattern)."
    )
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LockState {
    /// `.lock` opened, the non-blocking exclusive acquire was REFUSED — a live owner holds it (LIVE).
    Held,
    /// `.lock` opened, the non-blocking acquire SUCCEEDED (free/stale) — the owning run is DEAD.
    Free,
    /// `.lock` is ABSENT (never created, or the run crashed before the lock-before-part step).
    Absent,
}

/// §2.6.3 per-run-dir sweep decision — the output of the pure [`sweep_verdict`] rule.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "P3.23 — the §2.6.3 sweep verdict enum; produced by `sweep_verdict` and consumed by \
                  `sweep_stale` (dead in production until the §7.2 startup wiring), which rustc walks as a \
                  present caller marking these variants used — `allow` covers the transitive dead-ness \
                  through the P3 wiring window."
    )
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SweepVerdict {
    /// Keep the dir — a live owner (held lock), a just-starting run (lockless within grace), or an
    /// un-probeable dir (never delete on a guess).
    Keep,
    /// Reclaim the dir — a dead run (free lock) or a crashed-before-lock stale lockless dir.
    Remove,
}

/// §2.6.3 PURE liveness decision (no I/O, so exhaustively unit-testable): the **held lock is the SOLE delete
/// gate** — a `Held` lock ⇒ `Keep`, independent of mtime. Whenever the lock is **not held** — the `.lock` is
/// `Free` (acquirable) OR `Absent` — the **create-then-not-yet-locked grace window (§2.6.3 (b))** applies,
/// because a young dir may be a run mid-`acquire`: its `.lock` file just created but **still unlocked** (⇒
/// `Free`), or the dir created but its `.lock` **still absent** (⇒ `Absent`). Both must be kept while young —
/// so a dir younger than `grace` ⇒ `Keep`, and only a STALE (mtime past `grace`) not-held dir ⇒ `Remove`,
/// reclaimed on a subsequent sweep (§2.6.3: "a lockless very-recent dir is left for next time"). A `None`
/// `dir_age` (unreadable mtime / clock skew) is a conservative `Keep` — mtime is never a delete gate on its
/// own (§2.6.3), so an unknown age never removes. [Build-Session-Entscheidung: P3.23]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "P3.23 — the pure §2.6.3 sweep rule; its only caller is `sweep_stale` (dead in production \
                  until the §7.2 startup wiring), which rustc walks as a present caller marking this used — \
                  `allow` covers the transitive dead-ness through the P3 wiring window."
    )
)]
fn sweep_verdict(
    lock: LockState,
    dir_age: Option<std::time::Duration>,
    grace: std::time::Duration,
) -> SweepVerdict {
    match lock {
        // A HELD lock ⇒ live ⇒ keep — the SOLE delete gate (§2.6.3), independent of mtime.
        LockState::Held => SweepVerdict::Keep,
        // No HELD lock (`.lock` FREE/acquirable, or ABSENT): potentially a dead/crashed run, BUT a YOUNG dir
        // may be a run mid-`acquire` (the `.lock` just created but still unlocked ⇒ `Free`, or still absent
        // ⇒ `Absent`). The §2.6.3 (b) grace window keeps a young such dir and reclaims only a STALE one.
        LockState::Free | LockState::Absent => match dir_age {
            Some(age) if age < grace => SweepVerdict::Keep,
            Some(_) => SweepVerdict::Remove,
            None => SweepVerdict::Keep,
        },
    }
}

/// §2.6.3 non-blocking liveness probe of a run dir's `.lock`: open it, attempt a NON-BLOCKING exclusive
/// acquire via the [`crate::platform`] try-lock, and return the [`LockState`] — dropping the file handle (and
/// any momentarily-taken lock) before returning, so the sweep can then remove a dead dir (on Windows an open
/// handle inside the dir would block its delete). `NotFound` ⇒ `Absent`; any other open error, or a probe I/O
/// error, maps conservatively to `Held` (keep — liveness could not be established, and the held lock is the
/// sole delete gate). Panic-free (crate no-panic deny, G4). [Build-Session-Entscheidung: P3.23]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "P3.23 — the §2.6.3 sweep's per-dir lock probe; its only caller is `sweep_stale` (dead in \
                  production until the §7.2 startup wiring), which rustc walks as a present caller marking \
                  this used — `allow` covers the transitive dead-ness through the P3 wiring window."
    )
)]
fn probe_lock(lock_path: &Path) -> LockState {
    let file = match OpenOptions::new().read(true).write(true).open(lock_path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return LockState::Absent,
        // Cannot open the lock (a permission flip, etc.): liveness is unknown, so keep (never delete blind).
        Err(_) => return LockState::Held,
    };
    match crate::platform::try_acquire_exclusive_lock(&file) {
        Ok(true) => LockState::Free,  // acquired ⇒ dead
        Ok(false) => LockState::Held, // refused ⇒ live
        Err(_) => LockState::Held,    // probe I/O error ⇒ conservative keep
    }
    // `file` drops here: releases any acquired lock + closes the handle so a subsequent `remove_dir_all` can
    // delete the run dir on Windows (where a still-open handle inside the dir blocks the recursive remove).
}

/// The age of `dir` (now − mtime), or `None` if the mtime is unreadable or in the future (a clock skew ⇒
/// unknown ⇒ conservative keep). Used ONLY for the §2.6.3 lockless grace window — never as a delete gate for a
/// dir that has a `.lock`. Panic-free (crate no-panic deny, G4): every step is a fallible `Option`.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "P3.23 — the §2.6.3 lockless-grace mtime read; its only caller is `sweep_stale` (dead in \
                  production until the §7.2 startup wiring), which rustc walks as a present caller marking \
                  this used — `allow` covers the transitive dead-ness through the P3 wiring window."
    )
)]
fn dir_age(dir: &Path) -> Option<std::time::Duration> {
    let mtime = std::fs::metadata(dir).ok()?.modified().ok()?;
    std::time::SystemTime::now().duration_since(mtime).ok()
}

/// §2.6.3 startup sweep: reclaim the discardable central `run-<RunId>/` scratch dirs of DEAD prior runs across
/// ALL instance dirs — the held lock is the SOLE delete gate (never mtime/PID alone). Globs
/// `<scratch_base>/convertia/scratch/<*>.<*>/run-*` (every `<InstanceId>.<pid>` dir, so a crashed FOREIGN
/// instance's stale runs are reclaimed too), probes each `run-<RunId>/.lock` with the non-blocking try-lock
/// ([`probe_lock`]), and removes a dir only on a [`SweepVerdict::Remove`] ([`sweep_verdict`]): a **not-held**
/// dir (`.lock` free or absent) whose mtime is **past** the create-then-not-yet-locked grace window (a
/// dead/crashed run). A HELD lock (live run) and a **just-created not-held** dir (a run mid-`acquire`) are
/// LEFT UNTOUCHED — the sweep never races a just-starting run, never hangs on a live one (the try-lock is
/// non-blocking). Best-effort + panic-free (crate no-panic deny, G4): an unreadable scratch root / instance
/// dir / run dir is skipped, never a crash. Returns the run dirs actually REMOVED (for the §7.2 startup
/// caller's observability). The destination-resident `*.part` reclaim (§2.6.3 (b), a different location
/// entirely) is P3.24, not this central-scratch sweep. [Build-Session-Entscheidung: P3.23]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "P3.23 — the §2.6.3 startup sweep entry; the production caller is the §7.2 startup sequence \
                  wiring — unused in the production build until that lands."
    )
)]
pub fn sweep_stale(scratch_base: &Path) -> Vec<PathBuf> {
    sweep_stale_within(scratch_base, LOCKLESS_GRACE)
}

/// The §2.6.3 sweep body, parameterised on the grace window so a test can drive the reclaim path with a
/// zero/large `grace` without fragile directory-mtime aging. `sweep_stale` calls it with [`LOCKLESS_GRACE`];
/// the logic is otherwise identical. [Build-Session-Entscheidung: P3.23]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "P3.23 — the §2.6.3 sweep body; its only caller is `sweep_stale` (dead in production until \
                  the §7.2 startup wiring), which rustc walks as a present caller marking this used — `allow` \
                  covers the transitive dead-ness through the P3 wiring window."
    )
)]
fn sweep_stale_within(scratch_base: &Path, grace: std::time::Duration) -> Vec<PathBuf> {
    let scratch_root = scratch_base.join(SCRATCH_NAMESPACE).join(SCRATCH_SUBDIR);
    let mut removed = Vec::new();
    // No scratch root yet (first-ever run) or an unreadable one — nothing to sweep.
    let Ok(instance_dirs) = std::fs::read_dir(&scratch_root) else {
        return removed;
    };
    for instance_entry in instance_dirs.flatten() {
        // A non-dir entry / an unreadable instance dir yields no run dirs — skip it.
        let Ok(run_dirs) = std::fs::read_dir(instance_entry.path()) else {
            continue;
        };
        for run_entry in run_dirs.flatten() {
            let name = run_entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if !name.starts_with(RUN_DIR_PREFIX) {
                continue;
            }
            let run_dir = run_entry.path();
            let verdict = sweep_verdict(
                probe_lock(&run_dir.join(RUN_LOCK_FILE)),
                dir_age(&run_dir),
                grace,
            );
            // A stray `run-*` FILE (not a dir) cannot be `remove_dir_all`'d, so `.is_ok()` naturally skips it.
            if verdict == SweepVerdict::Remove && std::fs::remove_dir_all(&run_dir).is_ok() {
                removed.push(run_dir);
            }
        }
    }
    removed
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

#[cfg(test)]
mod cleanup_tests {
    use super::*;

    /// Plant a real file named `name` in `dir` (real temp FS, never mocked — test-strategy §0.1), returning
    /// its path. Used to seat own-prefix and foreign `.part` siblings the cleanup must (or must not) remove.
    fn plant(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"partial output bytes").expect("plant a file");
        path
    }

    // §6.4.3 real-FS (G31) / §2.6.2 "Item failure"/"Out-of-disk": `cleanup_item` removes the item's kind-1
    // publish temp (the failure / out-of-disk / link-fallback exit paths all reduce to "remove this .part"),
    // verified by reading the REAL temp FS back — the temp is GONE.
    #[test]
    fn cleanup_item_removes_the_items_publish_temp() {
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let dest = tempfile::tempdir().expect("a real destination dir");
        let scratch = RunScratch::acquire(
            base.path(),
            InstanceId::mint(),
            std::process::id(),
            RunId::mint(),
        )
        .expect("acquire the locked run scratch");
        let temp = scratch
            .publish_temp(dest.path(), JobId::from_index(3))
            .expect("mint the publish temp");
        let path = temp.to_path_buf();
        assert!(
            path.exists(),
            "the minted publish temp exists before cleanup"
        );

        cleanup_item(temp).expect("§2.6.2: cleanup removes the item's publish temp");
        assert!(
            !path.exists(),
            "§2.6.2 item-exit: the item's `.part` is gone after cleanup_item"
        );
    }

    // §6.4.3 real-FS (G31) / §2.6.4: `cleanup_item` is idempotent — an already-vanished temp (a double call,
    // or an externally-removed temp) is a clean success, never a spurious failure.
    #[test]
    fn cleanup_item_is_idempotent_when_the_temp_already_vanished() {
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let dest = tempfile::tempdir().expect("a real destination dir");
        let scratch = RunScratch::acquire(
            base.path(),
            InstanceId::mint(),
            std::process::id(),
            RunId::mint(),
        )
        .expect("acquire");
        let temp = scratch
            .publish_temp(dest.path(), JobId::from_index(0))
            .expect("mint the publish temp");
        let path = temp.to_path_buf();
        std::fs::remove_file(&path).expect("externally remove the temp before cleanup");

        cleanup_item(temp)
            .expect("§2.6.4: an already-absent temp is a clean, idempotent success, not a failure");
        assert!(!path.exists(), "the temp remains gone");
    }

    // §6.4.3 real-FS (G31) / §2.6.2 "Run end": `cleanup_run` removes this run's own-prefix temps in EVERY
    // recorded `final_dir` (incl. a second dir, modelling a late-divert / cross-volume target), and reports
    // no residue on a clean run-end. Read the real FS back — all own temps GONE.
    #[test]
    fn cleanup_run_removes_own_prefix_temps_in_every_recorded_dir() {
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let dest_a = tempfile::tempdir().expect("recorded final_dir A");
        let dest_b =
            tempfile::tempdir().expect("recorded final_dir B (a divert/cross-volume target)");
        let instance = InstanceId::mint();
        let run = RunId::mint();
        let scratch =
            RunScratch::acquire(base.path(), instance, std::process::id(), run).expect("acquire");
        let prefix = PublishTemp::run_prefix(instance, run);
        let own = [
            plant(dest_a.path(), &format!("{prefix}1-r1.part")),
            plant(dest_a.path(), &format!("{prefix}2-r2.part")),
            plant(dest_b.path(), &format!("{prefix}3-r3.part")),
        ];

        let mut recorded = BTreeSet::new();
        recorded.insert(dest_a.path().to_path_buf());
        recorded.insert(dest_b.path().to_path_buf());
        let residue = cleanup_run(scratch, &recorded);

        assert!(
            residue.is_empty(),
            "§2.6.4: a clean run-end reports no residue, got {residue:?}"
        );
        for p in &own {
            assert!(
                !p.exists(),
                "§2.6.2 run-end: this run's own-prefix temp is removed: {p:?}"
            );
        }
    }

    // §6.4.3 real-FS (G31) / §2.6.2 CRITICAL own-prefix scope: `cleanup_run` NEVER removes a concurrent
    // foreign instance's live `.part`, a foreign RUN's `.part` (same instance, different run), or a plain
    // user file — only this run's own-prefix temps in the SHARED dir. This is the SSOT "cleanup never removes
    // another instance's in-progress file" — proven by reading the real FS back (never a bare `*.part` glob).
    #[test]
    fn cleanup_run_never_removes_a_foreign_or_unrelated_part() {
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let shared =
            tempfile::tempdir().expect("a dest dir shared with a concurrent foreign instance");
        let instance = InstanceId::mint();
        let run = RunId::mint();
        let scratch =
            RunScratch::acquire(base.path(), instance, std::process::id(), run).expect("acquire");

        let ours = plant(
            shared.path(),
            &format!("{}5-mine.part", PublishTemp::run_prefix(instance, run)),
        );
        // A concurrent FOREIGN INSTANCE's live in-progress `.part` in the SAME dir (different InstanceId+RunId).
        let foreign_instance = plant(
            shared.path(),
            &format!(
                "{}5-theirs.part",
                PublishTemp::run_prefix(InstanceId::mint(), RunId::mint())
            ),
        );
        // A foreign RUN of the SAME instance (a different run of us / another live run) — still not our run.
        let foreign_run = plant(
            shared.path(),
            &format!(
                "{}5-otherrun.part",
                PublishTemp::run_prefix(instance, RunId::mint())
            ),
        );
        // A real user file that merely happens to sit beside the temps — never a `.part`, never touched.
        let user_file = plant(shared.path(), "vacation.jpg");

        let mut recorded = BTreeSet::new();
        recorded.insert(shared.path().to_path_buf());
        let residue = cleanup_run(scratch, &recorded);

        assert!(residue.is_empty(), "clean run-end, got residue {residue:?}");
        assert!(!ours.exists(), "§2.6.2: our own-prefix temp IS removed");
        assert!(
            foreign_instance.exists(),
            "SSOT: a concurrent foreign INSTANCE's live `.part` is NEVER removed (own-prefix scope, not a bare `*.part` glob)"
        );
        assert!(
            foreign_run.exists(),
            "§2.6.2: a foreign RUN's `.part` (same instance, different run) is not ours — kept"
        );
        assert!(
            user_file.exists(),
            "a non-`.part` user file is never touched"
        );
    }

    // §6.4.3 real-FS (G31) / §2.6.2 "Run end": `cleanup_run` tears down the central `run-<RunId>/` scratch
    // dir and releases the held lock (the remove_dir_all SUCCEEDS, which on Windows requires the `.lock`
    // handle already closed) — proven by reading the real FS back: the run dir is gone.
    #[test]
    fn cleanup_run_removes_the_run_scratch_dir() {
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let scratch = RunScratch::acquire(
            base.path(),
            InstanceId::mint(),
            std::process::id(),
            RunId::mint(),
        )
        .expect("acquire");
        let run_dir = scratch.dir().to_path_buf();
        assert!(
            run_dir.is_dir(),
            "the run scratch dir exists before cleanup"
        );

        let residue = cleanup_run(scratch, &BTreeSet::new());
        assert!(residue.is_empty(), "clean run-end, got residue {residue:?}");
        assert!(
            !run_dir.exists(),
            "§2.6.2 run-end: the central run-<RunId>/ scratch dir is torn down (lock released first)"
        );
    }

    // §6.4.3 real-FS (G31) / §2.6.4: an own-prefix entry that CANNOT be removed is surfaced as residue —
    // never a silent clean success. Forced deterministically, cross-platform, and root-safe by planting the
    // own-prefix name as a DIRECTORY: `std::fs::remove_file` refuses a directory (`EISDIR` on POSIX /
    // `ERROR_ACCESS_DENIED` on Windows) on every OS and even as root, standing in for the §2.6.4 real cases
    // (a lock held by AV software, a read-only scratch, a permission flip) without a root-fragile chmod.
    #[test]
    fn cleanup_run_reports_an_unremovable_own_temp_as_residue() {
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let dest = tempfile::tempdir().expect("a real destination dir");
        let instance = InstanceId::mint();
        let run = RunId::mint();
        let scratch =
            RunScratch::acquire(base.path(), instance, std::process::id(), run).expect("acquire");
        // An own-prefix entry `remove_file` cannot remove — it is a directory, not a file.
        let stuck = dest.path().join(format!(
            "{}1-x.part",
            PublishTemp::run_prefix(instance, run)
        ));
        std::fs::create_dir(&stuck).expect("plant an own-prefix entry as a directory");

        let mut recorded = BTreeSet::new();
        recorded.insert(dest.path().to_path_buf());
        let residue = cleanup_run(scratch, &recorded);

        assert_eq!(
            residue,
            vec![stuck.clone()],
            "§2.6.4: an own entry that could not be removed is surfaced as residue, never a silent clean success"
        );
        assert!(
            stuck.exists(),
            "the un-removable residue really remains on disk"
        );
    }

    // §6.4.3 real-FS (G31) / §2.6.4: a recorded `final_dir` that exists but CANNOT be enumerated (a
    // permission flip / a read-only volume that went away) may still hide an own `.part` — so it is surfaced
    // as residue, never a silent clean success. Forced deterministically, cross-platform, and root-safe by
    // recording a path that is a FILE, not a directory: `std::fs::read_dir` fails with a non-`NotFound` error
    // (`ENOTDIR` on POSIX / `ERROR_DIRECTORY` on Windows) on every OS and even as root.
    #[test]
    fn cleanup_run_surfaces_an_unlistable_recorded_dir_as_residue() {
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let host = tempfile::tempdir().expect("a real host dir");
        let scratch = RunScratch::acquire(
            base.path(),
            InstanceId::mint(),
            std::process::id(),
            RunId::mint(),
        )
        .expect("acquire");
        // A recorded "final_dir" that is actually a FILE — read_dir on it fails (not NotFound).
        let not_a_dir = host.path().join("this-is-a-file-not-a-dir");
        std::fs::write(&not_a_dir, b"x").expect("plant a file where a dir is recorded");

        let mut recorded = BTreeSet::new();
        recorded.insert(not_a_dir.clone());
        let residue = cleanup_run(scratch, &recorded);

        assert_eq!(
            residue,
            vec![not_a_dir.clone()],
            "§2.6.4: a recorded dir that could not be enumerated is surfaced as residue, never a silent clean success"
        );
    }

    // §6.4.3 real-FS (G31) / §2.6.2: a recorded `final_dir` that is genuinely ABSENT (NotFound) at run-end —
    // its contents went with it — has nothing to reclaim and nothing to report: a clean, empty residue. This
    // is the `NotFound`-vs-other split's clean side (distinct from the un-listable case above).
    #[test]
    fn cleanup_run_treats_an_absent_recorded_dir_as_clean() {
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let host = tempfile::tempdir().expect("a real host dir");
        let scratch = RunScratch::acquire(
            base.path(),
            InstanceId::mint(),
            std::process::id(),
            RunId::mint(),
        )
        .expect("acquire");
        let absent = host.path().join("never-created-subdir");

        let mut recorded = BTreeSet::new();
        recorded.insert(absent);
        let residue = cleanup_run(scratch, &recorded);

        assert!(
            residue.is_empty(),
            "§2.6.2: a genuinely-absent recorded dir has nothing to reclaim — no residue, got {residue:?}"
        );
    }
}

#[cfg(test)]
mod sweep_tests {
    use super::*;
    use std::time::Duration;

    fn scratch_root(base: &Path) -> PathBuf {
        base.join(SCRATCH_NAMESPACE).join(SCRATCH_SUBDIR)
    }

    /// Plant an EMPTY per-run scratch dir `…/convertia/scratch/<instance>.<pid>/run-<run>/` (no `.lock`),
    /// returning its path — the shape the §2.6.3 sweep globs. Real temp FS, never mocked (test-strategy §0.1).
    fn plant_run_dir(base: &Path, instance: InstanceId, pid: u32, run: RunId) -> PathBuf {
        let dir = scratch_root(base)
            .join(instance.scratch_root_segment(pid))
            .join(run.run_subdir_segment());
        std::fs::create_dir_all(&dir).expect("plant a run dir");
        dir
    }

    // §6.4.1 unit (G15) / §2.6.3: the pure verdict rule — the HELD lock is the SOLE delete gate (keep, any
    // age); every NOT-HELD case (`Free` OR `Absent`) is governed by the create-then-not-yet-locked grace
    // window (young ⇒ keep, stale ⇒ remove, unknown age ⇒ conservative keep). Both not-held cases behave
    // identically — the fix for the "`.lock` created but still unlocked" race (a young `Free` must be kept).
    #[test]
    fn sweep_verdict_covers_every_liveness_branch() {
        let grace = Duration::from_secs(10);
        let young = Some(Duration::from_secs(1));
        let stale = Some(Duration::from_secs(100));
        // A HELD lock keeps the dir regardless of age — the SOLE delete gate.
        assert_eq!(
            sweep_verdict(LockState::Held, stale, grace),
            SweepVerdict::Keep,
            "§2.6.3: a HELD lock ⇒ live ⇒ keep (mtime irrelevant, even when stale)"
        );
        // Both NOT-HELD states go through the grace window identically.
        for lock in [LockState::Free, LockState::Absent] {
            assert_eq!(
                sweep_verdict(lock, young, grace),
                SweepVerdict::Keep,
                "§2.6.3 (b): not-held + young ⇒ a run mid-acquire ⇒ keep ({lock:?})"
            );
            assert_eq!(
                sweep_verdict(lock, stale, grace),
                SweepVerdict::Remove,
                "§2.6.3: not-held + stale (past grace) ⇒ dead ⇒ reclaim ({lock:?})"
            );
            assert_eq!(
                sweep_verdict(lock, None, grace),
                SweepVerdict::Keep,
                "§2.6.3: not-held + unreadable mtime ⇒ conservative keep, mtime never a delete gate ({lock:?})"
            );
        }
    }

    // §6.4.3 real-FS (G31) / §2.6.3: a DEAD run — its `run-<RunId>/.lock` exists but is UNHELD (a crashed run's
    // lock is released on process exit) and its dir is PAST the grace window — is reclaimed: the non-blocking
    // probe acquires the free lock ⇒ Free ⇒ (stale) ⇒ Remove. Driven with `grace = ZERO` so the just-planted
    // dir counts as stale WITHOUT fragile directory-mtime aging. Read the real FS back: the run dir is gone.
    #[test]
    fn sweep_removes_a_dead_run_with_a_free_lock() {
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let dead = plant_run_dir(base.path(), InstanceId::mint(), 4242, RunId::mint());
        // An UNHELD `.lock` file (a dead run left it behind, lock released on exit).
        std::fs::write(dead.join(RUN_LOCK_FILE), b"").expect("plant an unheld .lock");

        let removed = sweep_stale_within(base.path(), Duration::ZERO);

        assert!(
            !dead.exists(),
            "§2.6.3: a dead run's scratch dir (free lock, past grace) is reclaimed"
        );
        assert_eq!(removed, vec![dead], "§2.6.3: the reclaimed dir is reported");
    }

    // §6.4.3 real-FS (G31) / §2.6.3 CRITICAL: a LIVE run holding its `.lock` is NEVER swept — the non-blocking
    // probe is REFUSED (would-block) ⇒ Held ⇒ keep. Driven with `grace = ZERO` to prove the HELD LOCK ALONE
    // (not the grace window) is the delete gate: even when mtime offers no protection, the live run survives.
    // Real held lock via RunScratch::acquire; read the real FS back.
    #[test]
    fn sweep_never_removes_a_live_run_even_at_zero_grace() {
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let live = RunScratch::acquire(
            base.path(),
            InstanceId::mint(),
            std::process::id(),
            RunId::mint(),
        )
        .expect("acquire a live locked run");
        let live_dir = live.dir().to_path_buf();

        let removed = sweep_stale_within(base.path(), Duration::ZERO);

        assert!(
            live_dir.is_dir(),
            "§2.6.3: a live run holding its lock is NEVER swept (held lock is the SOLE gate, even at zero grace)"
        );
        assert!(
            removed.is_empty(),
            "§2.6.3: nothing is reclaimed while the only run is live, got {removed:?}"
        );
        drop(live); // release the lock only after the sweep has run
    }

    // §6.4.3 real-FS (G31) / §2.6.3 (b) — the create-then-not-yet-locked RACE regression: a run that has
    // created its `.lock` FILE but has NOT OS-locked it (the window inside RunScratch::acquire between
    // open(.lock) and the flock/LockFileEx) must NOT be reclaimed by a concurrent startup sweep. The probe
    // sees the `.lock` as FREE (acquirable), but the dir is YOUNG ⇒ the grace window KEEPS it. Real public
    // sweep_stale (the live 10 s grace). Without the fix (unconditional Free ⇒ Remove) this run is destroyed
    // mid-start.
    #[test]
    fn sweep_never_removes_a_just_starting_run_with_an_unlocked_lock() {
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let starting = plant_run_dir(base.path(), InstanceId::mint(), 9, RunId::mint());
        // The `.lock` file exists but is NOT locked — the mid-acquire window.
        std::fs::write(starting.join(RUN_LOCK_FILE), b"")
            .expect("plant an unlocked .lock (mid-acquire)");

        let removed = sweep_stale(base.path());

        assert!(
            starting.is_dir(),
            "§2.6.3 (b): a just-starting run whose `.lock` is created but still unlocked is KEPT (grace), never reclaimed mid-acquire"
        );
        assert!(removed.is_empty(), "nothing reclaimed, got {removed:?}");
    }

    // §6.4.3 real-FS (G31) / §2.6.3 create-then-not-yet-locked window: a LOCKLESS run dir that was JUST
    // created (mtime within the grace window) is a just-starting run whose `.lock` is still absent — it is
    // LEFT UNTOUCHED, never raced. Real public sweep_stale (live grace). Read the real FS back.
    #[test]
    fn sweep_keeps_a_just_created_lockless_run_dir() {
        let base = tempfile::tempdir().expect("a real scratch base dir");
        // Freshly created, no `.lock` — its mtime is well within LOCKLESS_GRACE.
        let fresh = plant_run_dir(base.path(), InstanceId::mint(), 7, RunId::mint());

        let removed = sweep_stale(base.path());

        assert!(
            fresh.is_dir(),
            "§2.6.3: a just-created lockless run dir is left for a subsequent sweep (grace window), not deleted"
        );
        assert!(removed.is_empty(), "nothing reclaimed, got {removed:?}");
    }

    // §6.4.3 real-FS (G31) / §2.6.3: the sweep globs across ALL `<InstanceId>.<pid>` instance dirs — a crashed
    // FOREIGN instance's dead runs are reclaimed too. Two dead runs under two DISTINCT instances are both
    // removed (driven with `grace = ZERO` so the just-planted dead dirs count as stale). Read the real FS back.
    #[test]
    fn sweep_reclaims_dead_runs_across_all_instance_dirs() {
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let dead_a = plant_run_dir(base.path(), InstanceId::mint(), 100, RunId::mint());
        std::fs::write(dead_a.join(RUN_LOCK_FILE), b"").expect("unheld .lock A");
        let dead_b = plant_run_dir(base.path(), InstanceId::mint(), 200, RunId::mint());
        std::fs::write(dead_b.join(RUN_LOCK_FILE), b"").expect("unheld .lock B");

        let removed = sweep_stale_within(base.path(), Duration::ZERO);

        assert!(!dead_a.exists(), "§2.6.3: instance A's dead run reclaimed");
        assert!(
            !dead_b.exists(),
            "§2.6.3: instance B's dead run reclaimed (cross-instance glob)"
        );
        assert_eq!(removed.len(), 2, "both dead runs reported, got {removed:?}");
    }

    // §6.4.3 real-FS (G31) / §2.6.3: robustness — a NON-`run-` entry under an instance dir is ignored, and a
    // sweep over an ABSENT scratch root is a clean no-op (no panic, empty result). Panic-free (G4).
    #[test]
    fn sweep_ignores_non_run_entries_and_an_absent_root() {
        // Absent scratch root (first-ever run): clean no-op.
        let empty_base = tempfile::tempdir().expect("a base with no scratch root yet");
        assert!(
            sweep_stale(empty_base.path()).is_empty(),
            "§2.6.3: an absent scratch root sweeps to nothing, no panic"
        );

        // A non-`run-` sibling under an instance dir is left untouched.
        let base = tempfile::tempdir().expect("a real scratch base dir");
        let instance_dir =
            scratch_root(base.path()).join(InstanceId::mint().scratch_root_segment(5));
        std::fs::create_dir_all(&instance_dir).expect("instance dir");
        let not_a_run = instance_dir.join("not-a-run-dir");
        std::fs::create_dir(&not_a_run).expect("a non-run- sibling");

        let removed = sweep_stale(base.path());

        assert!(
            not_a_run.is_dir(),
            "§2.6.3: a non-`run-` entry is never swept"
        );
        assert!(removed.is_empty(), "nothing reclaimed, got {removed:?}");
    }
}
