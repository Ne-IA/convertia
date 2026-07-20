//! The SINGLE-SOURCE volume-capability helper — the one place a test asks the environment "is there a second
//! PHYSICAL volume here?" (§2.14.3) and "is a FAT/exFAT-class filesystem mounted here?" (§2.1.2/§2.7.2).
//!
//! Two §2 guarantees are only observable across a filesystem BOUNDARY the process cannot create for itself:
//! the §2.14.3 `EXDEV` cross-volume fallback needs `tmp` and `final` on genuinely different volumes, and the
//! §2.1.2 third-fallback (neither no-replace rename NOR hardlinks) needs a FAT/exFAT-class destination.
//! Mounting either is a privileged, out-of-process act — and `crate::isolation` is the sole sanctioned
//! `process::Command::new` site (G9 invariant (b) / G29), so a test may never spawn `mount`/`hdiutil`/`subst`
//! to manufacture one. This module therefore reports what the host already offers as an `Option`.
//!
//! **The declared expectation — a skip must never be able to hide a whole suite (test-strategy §0.2).** A
//! live capability probe with no declared expectation is the pattern test-strategy §0.2 rejects for the G38
//! decoder-golden ("absent-from-golden → legitimate skip; present-in-golden-but-absent-from-binary → hard
//! fail"). The analogue here is [`REQUIRE_SECOND_VOLUME_ENV`]: on a leg where a second volume is KNOWN to
//! exist, set `CONVERTIA_REQUIRE_SECOND_VOLUME=1` and [`second_volume_dir`] PANICS instead of returning `None`
//! — so a runner that silently loses its second volume reds the build instead of quietly turning the
//! cross-volume suite into a no-op. Unset, it returns `None` and the caller takes a commented skip (the P3.6
//! guarded-early-`return` precedent — never `#[ignore]`).
//!
//! **Discovery never sweeps for volumes.** [`candidate_roots`] is a short, per-OS list, not a scan: macOS has
//! NO discovery at all (every `/Volumes/<name>` entry is a user's attached medium), Linux uses the system
//! shared-memory tmpfs mounts, and Windows uses `D:\` alone with a MAPPED NETWORK drive excluded. Stated
//! exactly: a macOS attached medium (a USB stick, a DMG, a backup disk) and a Windows network share are out of
//! discovery's reach entirely; what discovery CAN still reach is the LOCAL volume holding `D:` — fixed or
//! removable, so an external disk that took that letter does count. It receives exactly one auto-removed temp
//! dir — the same act every test already performs on the crate root and the OS temp dir — and no user data is
//! read or touched. Anything beyond that is an explicit operator act via [`SECOND_VOLUME_ENV`]. (The FAT side
//! is stricter still: [`fat_class_mount`] returns a mount point to CLASSIFY and is never written to at all.)
//!
//! [Build-Session-Entscheidung: P3.65] **Homed at the crate root, not in a §0.7 tier**, exactly like the
//! §6.4.5 corpus helper [`crate::test_corpus`]: it is `#[cfg(test)]`-only test infrastructure that both
//! `crate::fs_guard` (the §2.1/§2.14.3 publish primitives) and `crate::orchestrator` (the §2.1.1 write
//! sequence) reach, so homing it inside either tier would invert the dependency direction the tiers express,
//! and copying it into both is the inline duplication the single-source-helper rule forbids (test-strategy
//! §3). It adds a FILE, never a directory, so the §1a/§0.7 structure map (G69 asserts the DIRECTORY set) is
//! untouched.
//!
//! **What each helper does NOT promise.** [`second_volume_dir`] returns a directory on a different volume —
//! that volume's filesystem TYPE is whatever the host mounted there, so it proves the `EXDEV` boundary, never
//! the FAT/exFAT capability gap. [`fat_class_mount`] is the converse: it names a FAT/exFAT mount point read
//! from the kernel's own mount table, so a detector assertion against it is independent of the detector under
//! test (asking `crate::platform::lacks_atomic_publish_primitive` to find its own subject would be circular
//! and would assert nothing).

use std::path::{Path, PathBuf};

/// The temp-dir prefix every discovered second-volume scratch dir carries — a run-visible marker naming the
/// box that mints it, so a leftover after a killed test run is attributable at a glance.
const SECOND_VOLUME_PREFIX: &str = "convertia-p365-xvol-";

/// Operator-declared directory on another volume, used ahead of any discovery. The door for a host whose
/// second volume is removable / networked / otherwise not safe to guess at (macOS and Windows in practice).
pub const SECOND_VOLUME_ENV: &str = "CONVERTIA_TEST_SECOND_VOLUME";

/// Set to `1` on a leg where a second volume is KNOWN to exist: a missing one then PANICS instead of skipping,
/// so the cross-volume suite cannot silently degrade to a no-op (the test-strategy §0.2 declared-expectation
/// discipline).
pub const REQUIRE_SECOND_VOLUME_ENV: &str = "CONVERTIA_REQUIRE_SECOND_VOLUME";

/// The §2.3.1 volume identity of `path` (`st_dev` on Unix, the NTFS volume serial on Windows) — the same
/// number `fs_guard::resolve_identity` reads for the frozen-set identity, so "different volume" here means
/// exactly what §2.14.1 means by it. `None` when the path cannot be resolved at all.
pub fn volume_of(path: &Path) -> Option<u64> {
    crate::fs_guard::resolve_identity(path)
        .ok()
        .map(|id| id.dev_or_volserial)
}

/// A writable scratch directory on a volume GENUINELY different from `anchor`'s — the §2.14.3 `EXDEV`
/// substrate. Candidates are the operator-declared [`SECOND_VOLUME_ENV`] dir first, then the system-owned
/// scratch roots of [`candidate_roots`]; each is accepted only after the minted temp dir's [`volume_of`] is
/// READ BACK and found to differ from `anchor`'s, so a bind/`subst` alias of the same volume is rejected
/// rather than silently producing a same-volume "cross-volume" test.
///
/// `None` = this host offers no second volume, so the caller skips its cross-volume leg with a comment naming
/// what went unexercised — UNLESS [`REQUIRE_SECOND_VOLUME_ENV`] is set, in which case the absence is a hard
/// failure (a leg that declared it has one must not quietly stop testing).
pub fn second_volume_dir(anchor: &Path) -> Option<tempfile::TempDir> {
    let found = discover_second_volume_dir(anchor);
    assert!(
        found.is_some() || !require_second_volume(),
        "{REQUIRE_SECOND_VOLUME_ENV} is set, so this leg declares a second volume — none was found, which \
         would silently turn the whole §2.14.3 cross-volume suite into a no-op (test-strategy §0.2). Point \
         {SECOND_VOLUME_ENV} at a directory on another volume, or unset the requirement."
    );
    found
}

/// Is the declared-expectation guard armed on this leg?
fn require_second_volume() -> bool {
    std::env::var_os(REQUIRE_SECOND_VOLUME_ENV).is_some_and(|value| value == "1")
}

/// The discovery half of [`second_volume_dir`] (kept separate so the declared-expectation guard reads as one
/// assertion over its result).
fn discover_second_volume_dir(anchor: &Path) -> Option<tempfile::TempDir> {
    let anchor_volume = volume_of(anchor)?;
    let declared = std::env::var_os(SECOND_VOLUME_ENV).map(PathBuf::from);
    for root in declared.into_iter().chain(candidate_roots()) {
        // Screen the ROOT first, so a candidate that is unreadable or sits on the anchor's own volume costs
        // nothing and — the point — no scratch dir is ever created on a volume this helper will not return.
        if volume_of(&root).is_none_or(|volume| volume == anchor_volume) {
            continue;
        }
        // A root that refuses a create (a read-only mount, a restricted drive root) is simply not a
        // candidate — try the next one. The dir is dropped (removed) when it is not the one we keep.
        let Ok(dir) = tempfile::Builder::new()
            .prefix(SECOND_VOLUME_PREFIX)
            .tempdir_in(&root)
        else {
            continue;
        };
        // Re-read the identity of the DIR itself: a root can host a nested mount, and a Windows `subst`/bind
        // alias resolves to the real volume — so the returned dir, not its root, is what must differ.
        if volume_of(dir.path()).is_some_and(|volume| volume != anchor_volume) {
            return Some(dir);
        }
    }
    None
}

/// Linux discovery roots: the POSIX shared-memory tmpfs mounts. They are system-owned scratch (never a user's
/// removable medium), ordinarily a filesystem separate from both the workspace and `/tmp`, and — incidentally,
/// not by design — absent from the §2.7.2 ephemeral-root list, so a destination there is a valid
/// `location_status` subject.
#[cfg(target_os = "linux")]
fn candidate_roots() -> Vec<PathBuf> {
    vec![PathBuf::from("/dev/shm"), PathBuf::from("/run/shm")]
}

/// macOS has no system-owned second scratch volume to discover safely: every `/Volumes/<name>` entry is a
/// user's attached medium (a USB stick, a DMG, a network share, a backup disk) that a test must not write to
/// on a guess. So macOS reaches a second volume ONLY through the operator-declared [`SECOND_VOLUME_ENV`].
#[cfg(target_os = "macos")]
fn candidate_roots() -> Vec<PathBuf> {
    Vec::new()
}

/// Windows discovery root: `D:\` alone — the conventional secondary LOCAL disk (and the CI runners' data
/// disk). The `A:`..`Z:` sweep is deliberately NOT done: it can stall on a letter with no medium or on a dead
/// network mapping, and it would write into whatever removable or networked drive answered first.
///
/// A MAPPED NETWORK drive that happened to take the letter is excluded WITHOUT any FFI (`GetDriveTypeW` would
/// need `unsafe`, which the crate root denies outside the one allow-listed module): canonicalizing a mapped
/// drive yields its UNC form — `dunce` strips the verbatim prefix only for a `VerbatimDisk`, so the value here
/// is `\\?\UNC\server\share\…` — and a LOCAL volume canonicalizes to a plain `D:\…`. The screen is therefore
/// "starts with two backslashes", which both the verbatim and the simplified UNC spellings satisfy and a local
/// disk path never does. An unreadable or absent `D:` resolves to the same exclusion (fail-closed). What
/// remains reachable by discovery is a LOCAL volume — fixed or removable — which receives one auto-removed
/// temp dir and nothing else; any other letter is the operator's call via [`SECOND_VOLUME_ENV`].
#[cfg(windows)]
fn candidate_roots() -> Vec<PathBuf> {
    let root = PathBuf::from("D:\\");
    let is_remote_or_unreadable = dunce::canonicalize(&root)
        .map(|resolved| resolved.as_os_str().to_string_lossy().starts_with(r"\\"))
        .unwrap_or(true);
    if is_remote_or_unreadable {
        Vec::new()
    } else {
        vec![root]
    }
}

/// A mount point whose filesystem the KERNEL reports as FAT/exFAT-class, read from `/proc/self/mounts` —
/// the §2.1.2/§2.7.2 substrate. The type comes from the mount table, NOT from
/// `crate::platform::lacks_atomic_publish_primitive`, so an assertion against this path is an independent
/// check of that detector rather than a restatement of it.
///
/// Read-only: this returns a mount point to CLASSIFY, never a directory to write into — a test must not
/// deposit files on whatever removable medium the host happens to have attached. `None` on a host with no
/// such mount (the ordinary case, including the CI runners), which the caller skips with a comment.
#[cfg(target_os = "linux")]
pub fn fat_class_mount() -> Option<PathBuf> {
    let mounts = std::fs::read_to_string("/proc/self/mounts").ok()?;
    mounts.lines().find_map(|line| {
        // `<device> <mount point> <fs type> <options> …`, whitespace-separated.
        let mut fields = line.split_ascii_whitespace();
        let _device = fields.next()?;
        let mount_point = fields.next()?;
        let fs_type = fields.next()?;
        // The kernel escapes space/tab/newline/backslash in a mount point as `\0NN`; a path carrying an
        // escape is skipped rather than mis-parsed (an ordinary FAT mount point carries none).
        if mount_point.contains('\\') || !matches!(fs_type, "vfat" | "exfat" | "msdos") {
            return None;
        }
        let path = PathBuf::from(mount_point);
        path.is_dir().then_some(path)
    })
}
