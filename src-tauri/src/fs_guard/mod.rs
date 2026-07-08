//! `crate::fs_guard` — the §2.0 no-harm kernel: atomic, exclusive, no-clobber publish on the resolved
//! real file, link-safety + resolved-identity, the frozen source set, path-limit handling and the
//! cross-volume strategy (§2.1 / §2.2 / §2.3 / §2.14). Every output flows through here; engines never
//! write the final file. A §0.7 tier-2 trust-kernel LEAF: it depends DOWN only (on `crate::domain`),
//! never up on IPC / orchestrator / the engine registry (§2.0 dependency direction). Unsafe-free — the
//! crate-root `#![deny(unsafe_code)]` (main.rs) covers it. The §2.3.1 resolved-IDENTITY read (P3.6) needs
//! NO in-core FFI: the Windows `(volumeSerialNumber, fileIndex)` comes through `winapi-util`'s SAFE
//! `GetFileInformationByHandle` wrapper (with `dunce` for the canonical-path `\\?\`-normalisation) — no
//! `unsafe` in the core (§2.3.1 `[CORRECTED 2026-07-07]`). The remaining per-OS handle FFI — the §2.3.3/§2.1
//! publish dir-handle primitive (P3.9) — is homed in the single allow-listed `crate::platform` shim (§0.7).
//!
//! P2.74 lands the pure §2.3.1 resolved-identity TYPE (`FileIdentity`) — the §2.3.2 de-dup key (below).
//!
//! ## P3.1.1 public-surface contract map — bodies authored by the named fill-boxes
//! [Build-Session-Entscheidung: P3.1.1] The §2.0 kernel's public functions are declared here as a
//! documented CONTRACT MAP — the title's "function shells" are the public SURFACE, not callable bodies.
//! No honest minimal value exists for any of them, so each signature AND body land together in its named
//! fill-box below (the P2.74 ruling — author the `FileIdentity` TYPE, never a `resolve_identity`
//! half-body — applied across the whole surface; and `crate::isolation` / `crate::pool` likewise carry
//! only a documented root at scaffold time, their own interface landing at P3.2 / P3.3). It costs no
//! compile-time reach: every P3 caller `needs:` the function's own fill-box,
//! never P3.1.
//!  - `resolve_identity(path) -> io::Result<FileIdentity>` — canonicalize + per-OS `(dev, inode)` /
//!    file-index identity (§2.3.1 / §2.3.4) — **P3.6** (the TYPE is P2.74).
//!  - `is_safe_output` — write-target link-safety vs the frozen source set (§2.3.3) — **P3.8**, over the
//!    **P3.9** TOCTOU-closed parent-dir-handle primitive.
//!  - `output_name` — verbatim-stem + `stem (n).ext` lazy no-clobber candidates (§2.2.1 / §2.10.1) — **P3.10**.
//!  - `check_path_limit` — per-OS component + total path-length validation, fail-never-truncate (§2.2.3) — **P3.11**.
//!  - `atomic_publish(parent_handle, tmp, leaf)` — per-OS create-only exclusive publish + the §2.14.3
//!    cross-volume fallback (§2.1 / §2.3.3 / §2.14.3) — **P3.9 / P3.12-P3.18**.
//!  - `location_status` — per-location writability + ephemeral classification, cached per-dir (§2.7.2) — **P3.33**.

use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

// [Build-Session-Entscheidung: P2.74] The §2.3.1 resolved-identity TYPE only (option A "split IO-vs-pure",
// Co-Pilot 2026-06-30, owner-ratified): the `resolve_identity` FUNCTION that POPULATES it
// (`dunce::canonicalize` + the per-OS metadata read = IO, needs `dunce` and — on Windows — `winapi-util`
// for the safe `GetFileInformationByHandle` file-identity, both landed at P3.6) is wholly P3 — its
// contract-map entry at P3.1.1, its body at P3.6 — so there is NO half-built function shell and NO
// tagged-`Err` placeholder here (a placeholder with no honest value is the rejected quiet-stub, CLAUDE §5).
//
// Derive / identity choices (the §0.6 sibling types fix the house style):
//  - Core-INTERNAL: `FileIdentity` never crosses IPC — the §2.3.2 de-dup runs core-side and the wire
//    carries `DroppedItem.resolved_path` (§0.6), not this identity — so it derives NO `serde`/`specta`,
//    only `Debug` + `Clone` (it owns a `PathBuf`, hence NOT `Copy`), the internal-type set
//    `FrozenCollectedSet` / `Batch` use.
//  - `dev_or_volserial` / `inode_or_fileindex` are both `u64`: Unix `st_dev`/`st_ino` are `u64`; the Windows
//    `winapi-util` accessors `volume_serial_number()` / `file_index()` are BOTH already `u64` (0.1.11, no
//    widening — the std `MetadataExt` `Option<u32>`/`Option<u64>` forms are the nightly-gated ones we do NOT
//    use, §2.3.1). One platform-agnostic representation, so the TYPE carries no `cfg` — only its P3 producer
//    reads the per-OS metadata.
//  - `pub` fields, no constructor: a plain data record with no validation invariant (like `DroppedItem` /
//    `FrozenCollectedSet`), so P3's `resolve_identity` builds it by struct literal and the P2.76 pure
//    de-dup fold + the §6.4.1 unit tests construct `FileIdentity` values directly.
//  - `PartialEq`/`Eq`/`Hash` are HAND-WRITTEN over `(dev_or_volserial, inode_or_fileindex)` ONLY, NOT
//    `#[derive]`d over all three fields: §2.3.2 makes the (dev, inode)/file-index identity — "NOT the path
//    string" — the de-dup key, and §2.3.4 shows a HARDLINK is two distinct paths over ONE inode that
//    `canonicalize` cannot collapse (no link to follow), so the two `FileIdentity` values carry DIFFERENT
//    `canonical_path` but the SAME identity and MUST compare / hash equal to collapse to one frozen member
//    (§2.3.2 "converted once"). A blind `#[derive(Eq, Hash)]` would fold `canonical_path` into the key and
//    silently break hardlink de-dup. `canonical_path` is the §2.3.1 fast pre-filter + the §2.3.2 first-seen
//    representative path (and the §2.3.3 prefix-containment input), carried in the value, OUT of the key.

/// The §2.3.1 canonical resolved identity of a path — `{ canonical_path, dev_or_volserial,
/// inode_or_fileindex }`, the key the §2.4 frozen source set de-duplicates on (§2.3.2) and `is_safe_output`
/// (§2.3.3) compares against. Two paths are the **same resolved file iff the device+inode identity matches**
/// (authoritative); the canonical path is a fast pre-filter and the retained first-seen representative, NOT
/// part of the identity (the §2.3.4 hardlink case: same inode, different path). Produced by
/// `fs_guard::resolve_identity` — the IO/FFI canonicalize + per-OS metadata read, authored in P3.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "§2.3.1 FileIdentity is forward-declared at P2.74 (the pure de-dup-key type); its production producer `resolve_identity` (IO/FFI) and its first consumer (the P2.76 de-dup fold, wired into the P3.49 spine) are P3, so it is dead in the production build until those land."
    )
)]
#[derive(Debug, Clone)]
pub struct FileIdentity {
    /// The canonicalized path (`std::fs::canonicalize`, normalised to the most-compatible non-UNC form via
    /// `dunce::canonicalize` on Windows, §2.3.1). The §2.3.1 **fast pre-filter** + the §2.3.2 first-seen
    /// **representative** path + the §2.3.3 prefix-containment input — NOT part of the identity key (a
    /// hardlink shares an inode but not a canonical path, §2.3.4).
    pub canonical_path: PathBuf,
    /// The volume identity: Unix `st_dev` (`MetadataExt::dev`) / Windows `volumeSerialNumber`
    /// (`winapi-util`'s `volume_serial_number()`, already `u64`). Half of the authoritative (dev, inode) identity —
    /// disambiguates two files that share an inode NUMBER across different volumes (§2.3.1).
    pub dev_or_volserial: u64,
    /// The file identity within its volume: Unix `st_ino` (`MetadataExt::ino`) / Windows file index
    /// (`file_index()`). With `dev_or_volserial` this is the authoritative "same file?" key that catches
    /// **hardlinks** (everywhere) and **junctions** (Windows) `canonicalize` alone misses (§2.3.1 / §2.3.4).
    pub inode_or_fileindex: u64,
}

/// Identity is the **(dev, inode)/file-index pair only** — §2.3.2: "identity, NOT the path string, is the
/// de-dup key". Excluding `canonical_path` is what collapses a hardlink (two paths, one inode, §2.3.4) to a
/// single frozen member.
impl PartialEq for FileIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.dev_or_volserial == other.dev_or_volserial
            && self.inode_or_fileindex == other.inode_or_fileindex
    }
}

impl Eq for FileIdentity {}

/// Consistent with `PartialEq` (§2.3.2): hashes ONLY the `(dev, inode)` identity `eq` compares (never
/// `canonical_path`), so `a == b ⇒ hash(a) == hash(b)` and two hardlinked paths land in one
/// `HashSet`/`HashMap` slot — the §2.4 frozen-set "keyed by `FileIdentity`" de-dup.
impl Hash for FileIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.dev_or_volserial.hash(state);
        self.inode_or_fileindex.hash(state);
    }
}

/// Resolve a path to its §2.3.1 canonical [`FileIdentity`] — the load-bearing "same file?" key the §2.4
/// frozen set de-duplicates on (§2.3.2) and `is_safe_output` (§2.3.3) compares against. This is the IO/FFI
/// producer for the pure P2.74 type; it fills the P3.1.1 contract-map slot.
///
/// **Fallible (`io::Result`), never a panic.** `canonicalize` fails on a path that does not exist, so a
/// missing SOURCE is a clean `Err` the §2.8 caller maps — NEVER `unwrap`/`expect`/`panic` (this runs on
/// untrusted paths OUTSIDE the §2.12 isolation boundary, where a stray panic is an in-core DoS; the no-panic
/// policy G4/G14 forbids it here). The §2.3.2 "retry on the parent when the path does not exist" is the
/// §2.3.3 OUTPUT-target path's concern (`is_safe_output`, P3.8) — NOT this: a frozen source exists at drop,
/// so a missing one is honestly `Err`.
///
/// - **`canonical_path`:** `dunce::canonicalize` — `std::fs::canonicalize` (resolves symlinks + `.`/`..`)
///   with the Windows verbatim-`\\?\`-UNC form normalised to the most-compatible non-UNC form so two paths
///   differing only by that prefix compare equal (§2.3.1). Off-Windows `dunce::canonicalize` is a
///   `std::fs::canonicalize` passthrough, so the call is uniform (no `cfg`).
/// - **`(dev_or_volserial, inode_or_fileindex)`** — the authoritative identity, read per OS from `path`
///   (`std::fs::metadata` / `winapi-util` both follow a symlink to the SAME resolved real file, so the pair
///   is identical whether read from `path` or the canonical path):
///   - **Unix / macOS (`cfg(unix)`):** `std::fs::metadata` + `MetadataExt::dev()`/`ino()` (both `u64`,
///     stable). A **hardlink** shares the `(dev, ino)` pair `canonicalize` cannot collapse — no link to
///     follow, §2.3.4.
///   - **Windows (`cfg(windows)`):** the `(volumeSerialNumber, fileIndex)` from `GetFileInformationByHandle`
///     via **`winapi-util`**'s SAFE wrapper — `Handle::from_path_any` (opens with backup semantics; follows
///     reparse points to the resolved real file) then `file::information(&handle)` →
///     `volume_serial_number()`/`file_index()` (both `u64`). **No `unsafe` in the core** — the FFI lives
///     inside the audited crate. The std `MetadataExt` equivalents are nightly-gated (`windows_by_handle`,
///     rust-lang #63010 — unavailable on the pinned stable toolchain), and `same-file` exposes no Windows
///     identity numbers, so `winapi-util` is the direct dependency (§2.3.1 `[CORRECTED 2026-07-07]`). Catches
///     hardlinks + junctions `canonicalize` misses (§2.3.4).
///
/// [Build-Session-Entscheidung: P3.6] the identity is read from `path` (not from `canonical_path`): both
/// `std::fs::metadata` and `Handle::from_path_any` follow to the same real file, so the pair is identical
/// either way, and reading `path` mirrors what the §6.4.1 per-OS mutant-killer tests read directly.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "§2.3.1 resolve_identity (the IO/FFI producer of FileIdentity) is authored at P3.6; its first PRODUCTION consumer is the P3.7 resolved-identity de-dup fold / the P3.49 spine, so it is dead in the production build until then (the cfg(test) real-FS tests below exercise it — the test build is dead-code-clean)."
    )
)]
pub fn resolve_identity(path: &Path) -> io::Result<FileIdentity> {
    // `dunce::canonicalize` is uniform: on Windows it strips the verbatim `\\?\` UNC prefix; off-Windows it
    // is a `std::fs::canonicalize` passthrough (dunce 1.0.5 lib.rs) — one call, no `cfg` (§2.3.1).
    let canonical_path = dunce::canonicalize(path)?;

    // The authoritative (dev/volume-serial, inode/file-index) identity, read per OS. Exactly one `let`
    // compiles per target (unix vs windows) — ConvertIA ships only Win/macOS/Linux (§1), and macOS is
    // `cfg(unix)`. Both readers follow a symlink to the resolved real file; a hardlink shares the pair.
    #[cfg(unix)]
    let (dev_or_volserial, inode_or_fileindex) = {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::metadata(path)?;
        (meta.dev(), meta.ino())
    };
    #[cfg(windows)]
    let (dev_or_volserial, inode_or_fileindex) = {
        // winapi-util centralizes the `GetFileInformationByHandle` FFI behind a SAFE API — no `unsafe` in the
        // core (the crate-root `#![deny(unsafe_code)]` holds). `from_path_any` opens with
        // FILE_FLAG_BACKUP_SEMANTICS (a directory path works too) and follows reparse points → the resolved
        // real file (§2.3.4). Both accessors are plain `u64` (winapi-util 0.1.11 file.rs).
        let handle = winapi_util::Handle::from_path_any(path)?;
        let info = winapi_util::file::information(&handle)?;
        (info.volume_serial_number(), info.file_index())
    };

    Ok(FileIdentity {
        canonical_path,
        dev_or_volserial,
        inode_or_fileindex,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Build a `FileIdentity` tersely — the §6.4.1 unit level constructs values directly (no IO; the
    /// `resolve_identity` that reads real metadata is P3).
    fn id(path: &str, dev: u64, inode: u64) -> FileIdentity {
        FileIdentity {
            canonical_path: PathBuf::from(path),
            dev_or_volserial: dev,
            inode_or_fileindex: inode,
        }
    }

    // §6.4.1 unit (G15) / §2.3.2 / §2.3.4: a HARDLINK is two distinct paths over ONE (dev, inode) — the
    // canonical paths differ (`canonicalize` cannot follow a hardlink, §2.3.4) yet the resolved identity is
    // the SAME, so the two `FileIdentity` values MUST compare equal AND collapse to one slot in a
    // `FileIdentity`-keyed set (§2.3.2 "converted once" — the §2.4 frozen-set de-dup). This is the
    // load-bearing reason `Eq`/`Hash` exclude `canonical_path`; a `#[derive]` over all three fields fails it.
    #[test]
    fn hardlink_same_inode_different_path_is_one_identity() {
        let a = id("/data/photo.jpg", 66, 1234);
        let b = id("/data/backup/photo-link.jpg", 66, 1234); // hardlink: different path, same (dev, inode)
        assert_eq!(
            a, b,
            "§2.3.2/§2.3.4: same (dev, inode) ⇒ same resolved file, regardless of canonical path"
        );
        let set: HashSet<FileIdentity> = [a, b].into_iter().collect();
        assert_eq!(
            set.len(),
            1,
            "§2.4: a FileIdentity-keyed set collapses a hardlink to one member (Eq + Hash agree)"
        );
    }

    // §6.4.1 unit (G15) / §2.3.1: distinct inodes on the same volume are distinct files — the de-dup key
    // must NOT collapse them (two genuinely different files in one dropped folder convert separately).
    #[test]
    fn different_inode_same_volume_is_distinct() {
        let a = id("/data/one.jpg", 66, 1234);
        let b = id("/data/two.jpg", 66, 5678);
        assert_ne!(a, b, "§2.3.1: different inode ⇒ different file");
        let set: HashSet<FileIdentity> = [a, b].into_iter().collect();
        assert_eq!(
            set.len(),
            2,
            "§2.4: two distinct identities stay two set members"
        );
    }

    // §6.4.1 unit (G15) / §2.3.1: an inode NUMBER is only unique within a volume, so two files that share an
    // inode number across DIFFERENT volumes (dev/volume-serial) are different files — the `dev_or_volserial`
    // half of the key disambiguates them (the reason identity is the PAIR, not the inode alone).
    #[test]
    fn same_inode_different_volume_is_distinct() {
        let a = id("/mnt/usb/photo.jpg", 66, 1234);
        let b = id("/photo.jpg", 99, 1234); // same inode number, different volume
        assert_ne!(
            a, b,
            "§2.3.1: equal inode across different volumes ⇒ different files (dev disambiguates)"
        );
        let set: HashSet<FileIdentity> = [a, b].into_iter().collect();
        assert_eq!(
            set.len(),
            2,
            "§2.4: cross-volume inode collision stays two members"
        );
    }

    // §6.4.1 unit (G15) / §2.3.1/§2.3.2: `canonical_path` is the retained first-seen REPRESENTATIVE — carried
    // in the value and readable — but is OUT of the identity key, so two values that differ ONLY by
    // canonical path (the hardlink shape) are still equal. Locks both halves of the design: the path is kept,
    // and it does not influence equality.
    #[test]
    fn canonical_path_is_retained_but_excluded_from_identity() {
        let a = id("/data/first-seen.jpg", 66, 1234);
        assert_eq!(
            a.canonical_path,
            PathBuf::from("/data/first-seen.jpg"),
            "§2.3.2: the canonical path is retained as the first-seen representative"
        );
        let other_path = id("/data/second-path.jpg", 66, 1234);
        assert_eq!(
            a, other_path,
            "§2.3.1: canonical path is a pre-filter/representative, NOT part of the identity key"
        );
    }

    // ── P3.6: real-FS resolve_identity (§2.3.1/§2.3.4) — never mock the FS under test (test-strategy §0.1) ──

    // §2.3.1 (G15): a real file resolves to a stable, deterministic identity — the Ok path + the per-OS
    // metadata read of whichever `resolve_identity` body compiled on this CI leg.
    #[test]
    fn real_file_resolves_to_a_stable_identity() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let p = dir.path().join("photo.jpg");
        std::fs::write(&p, b"bytes").expect("write the file");
        let a = resolve_identity(&p).expect("resolve once");
        let b = resolve_identity(&p).expect("resolve twice");
        assert_eq!(
            a, b,
            "§2.3.1: resolving the same file twice yields the same identity (deterministic)"
        );
        assert!(
            a.canonical_path.ends_with("photo.jpg"),
            "§2.3.1: the canonical path names the resolved file"
        );
    }

    // §2.3.2/§2.3.4 (G15/G31): a HARDLINK — two names, one (dev, inode)/file-index — resolves to ONE identity
    // (the §2.4 de-dup collapses it) YET the two canonical paths DIFFER, because `canonicalize` cannot follow
    // a hardlink (no link to follow, §2.3.4). The real-FS proof the synthetic
    // `hardlink_same_inode_different_path_is_one_identity` unit can only assert by construction.
    #[test]
    fn hardlink_yields_same_identity_but_different_canonical_path() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let original = dir.path().join("original.txt");
        std::fs::write(&original, b"payload").expect("write the original");
        let link = dir.path().join("hardlink.txt");
        // A no-hardlink volume (FAT/exFAT, §2.3.4) reports Unsupported/PermissionDenied — skip only THAT; any
        // OTHER error (transient I/O, a real bug) must fail loudly, never vacuous-pass (this is the sole
        // real-FS proof of the §2.3.2 hardlink de-dup-collapse). Real temp dirs are NTFS/ext4/APFS, so the
        // skip does not fire in practice. [Build-Session-Entscheidung: P3.6]
        let linked = std::fs::hard_link(&original, &link);
        if matches!(&linked, Err(e) if matches!(e.kind(), std::io::ErrorKind::Unsupported | std::io::ErrorKind::PermissionDenied))
        {
            return;
        }
        linked.expect("create the hardlink (a non-unsupported error is a real failure)");
        let a = resolve_identity(&original).expect("resolve the original");
        let b = resolve_identity(&link).expect("resolve the hardlink");
        assert_eq!(
            a, b,
            "§2.3.2/§2.3.4: a hardlink shares the (dev, inode)/file-index identity — de-dups to one member"
        );
        assert_ne!(
            a.canonical_path, b.canonical_path,
            "§2.3.4: canonicalize cannot follow a hardlink, so the two names keep distinct canonical paths"
        );
    }

    // §2.3.1 (G15): two genuinely distinct files in one dir have distinct identities — the de-dup must NOT
    // collapse them. Kills a constant-identity mutant on the metadata read.
    #[test]
    fn two_distinct_files_have_distinct_identities() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let one = dir.path().join("one.jpg");
        let two = dir.path().join("two.jpg");
        std::fs::write(&one, b"a").expect("write one");
        std::fs::write(&two, b"b").expect("write two");
        let a = resolve_identity(&one).expect("resolve one");
        let b = resolve_identity(&two).expect("resolve two");
        assert_ne!(
            a, b,
            "§2.3.1: two distinct files have distinct (dev, inode)/file-index identities"
        );
    }

    // §2.8 (G15): a non-existent SOURCE is a clean Err (canonicalize fails), NEVER a panic — the no-panic
    // policy on this in-core untrusted-path surface (G4/G14). Doubly-missing (no parent) so it is Err
    // regardless of any parent-retry (that retry is `is_safe_output`/§2.3.3/P3.8, not `resolve_identity`).
    #[test]
    fn nonexistent_path_is_err() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let missing = dir.path().join("no_parent").join("no_file");
        assert!(
            resolve_identity(&missing).is_err(),
            "§2.8: a missing source path is a clean Err, never a panic (the caller maps it)"
        );
    }
}

// §6.4.1 real-FS (G15/G31): the Unix/macOS `resolve_identity` branch — canonicalize FOLLOWS a symlink, and
// the identity is the std `MetadataExt` (dev, ino). TWO STACKED cfg attributes (`#[cfg(test)]` then
// `#[cfg(unix)]`) — NOT the compound `#[cfg(all(test, unix))]` — so clippy's allow-expect-in-tests recognises
// the test context (the P1.17 compound-cfg trap; else the tests' `expect` calls trip clippy::expect_used,
// reddening the ubuntu/macOS legs).
#[cfg(test)]
#[cfg(unix)]
mod unix_realfs_tests {
    use super::*;

    // §2.3.4: canonicalize FOLLOWS a symlink — link and target resolve to ONE identity AND one canonical
    // path (the follow-symlink counterpart to the can't-follow-hardlink test).
    #[test]
    fn symlink_resolves_to_its_target_identity() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let target = dir.path().join("target.txt");
        std::fs::write(&target, b"payload").expect("write the target");
        let link = dir.path().join("link.txt");
        std::os::unix::fs::symlink(&target, &link).expect("create a unix symlink");
        let a = resolve_identity(&link).expect("resolve the symlink");
        let b = resolve_identity(&target).expect("resolve the target");
        assert_eq!(
            a, b,
            "§2.3.4: canonicalize follows a symlink — link and target resolve to one identity"
        );
        assert_eq!(
            a.canonical_path, b.canonical_path,
            "§2.3.1: a followed symlink shares the target's canonical path"
        );
    }

    // §2.3.1 (mutant-killer, cargo-mutants target): the resolved identity equals the directly-read (dev, ino)
    // of the same file — kills a swapped-field / constant-return mutant on the unix branch.
    #[test]
    fn unix_identity_matches_directly_read_dev_and_ino() {
        use std::os::unix::fs::MetadataExt;
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let p = dir.path().join("file.bin");
        std::fs::write(&p, b"x").expect("write the file");
        let id = resolve_identity(&p).expect("resolve the file");
        let meta = std::fs::metadata(&p).expect("read metadata directly");
        assert_eq!(
            id.dev_or_volserial,
            meta.dev(),
            "§2.3.1: dev_or_volserial is the Unix st_dev (MetadataExt::dev)"
        );
        assert_eq!(
            id.inode_or_fileindex,
            meta.ino(),
            "§2.3.1: inode_or_fileindex is the Unix st_ino (MetadataExt::ino)"
        );
    }
}

// §6.4.1 real-FS (G15/G31): the Windows `resolve_identity` branch — the dunce non-UNC canonical form + the
// winapi-util (volume_serial, file_index) identity. Stacked `#[cfg(test)]`+`#[cfg(windows)]` (the P1.17 trap).
#[cfg(test)]
#[cfg(windows)]
mod windows_realfs_tests {
    use super::*;

    // §2.3.1: the canonical form is dunce-normalised to the most-compatible non-UNC form — no verbatim `\\?\`
    // prefix. UN-SKIPPABLE, so it anchors the Windows dunce branch floor (G27).
    #[test]
    fn canonical_path_has_no_verbatim_unc_prefix() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let p = dir.path().join("file.bin");
        std::fs::write(&p, b"x").expect("write the file");
        let id = resolve_identity(&p).expect("resolve the file");
        assert!(
            !id.canonical_path.to_string_lossy().starts_with(r"\\?\"),
            "§2.3.1: dunce normalises the Windows canonical form to the non-UNC form (no verbatim prefix)"
        );
    }

    // §2.3.4: a Windows symlink is followed to the target identity. Symlink creation needs the
    // SeCreateSymbolicLink privilege (Developer Mode / elevation); an UNPRIVILEGED runner errors with 1314
    // (ERROR_PRIVILEGE_NOT_HELD → PermissionDenied) → skip gracefully. So on an unprivileged Windows CI leg
    // this follow-a-reparse-point proof is NOT exercised — the winapi-util `from_path_any` follow behaviour is
    // still verified from the crate source AND on every Unix leg (the unix symlink test), and the identity
    // READ is CI-proven unskippably by the hardlink + `windows_identity_matches_*` tests; the unskippable
    // privileged/junction Windows reparse-follow proof is owned by the §6.6 human walkthrough + the P3
    // phase-end hardening sweep (test-strategy §11). [Build-Session-Entscheidung: P3.6]
    #[test]
    fn symlink_resolves_to_target_identity_or_skips_unprivileged() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let target = dir.path().join("target.txt");
        std::fs::write(&target, b"payload").expect("write the target");
        let link = dir.path().join("link.txt");
        let made = std::os::windows::fs::symlink_file(&target, &link);
        // Single `if matches!` (no nested-if → no clippy::collapsible_if): an unprivileged runner skips.
        if matches!(&made, Err(e) if e.raw_os_error() == Some(1314) || e.kind() == std::io::ErrorKind::PermissionDenied)
        {
            return;
        }
        made.expect("create the test symlink (a non-privilege error is a real failure)");
        let a = resolve_identity(&link).expect("resolve the symlink");
        let b = resolve_identity(&target).expect("resolve the target");
        assert_eq!(
            a, b,
            "§2.3.4: a Windows symlink is followed (winapi-util from_path_any) to the target identity"
        );
    }

    // §2.3.1 (mutant-killer, cargo-mutants target): the resolved identity equals the directly-read winapi-util
    // (volume_serial, file_index) of the same handle — kills swapped file-index high/low-word or wrong-field
    // mutants on the Windows branch.
    #[test]
    fn windows_identity_matches_directly_read_volume_serial_and_file_index() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let p = dir.path().join("file.bin");
        std::fs::write(&p, b"x").expect("write the file");
        let id = resolve_identity(&p).expect("resolve the file");
        let handle = winapi_util::Handle::from_path_any(&p).expect("open a handle");
        let info = winapi_util::file::information(&handle).expect("read file information");
        assert_eq!(
            id.dev_or_volserial,
            info.volume_serial_number(),
            "§2.3.1: dev_or_volserial is the Windows volumeSerialNumber"
        );
        assert_eq!(
            id.inode_or_fileindex,
            info.file_index(),
            "§2.3.1: inode_or_fileindex is the Windows fileIndex"
        );
    }
}
