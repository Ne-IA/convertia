//! `crate::fs_guard` — the §2.0 no-harm kernel: atomic, exclusive, no-clobber publish on the resolved
//! real file, link-safety + resolved-identity, the frozen source set, path-limit handling and the
//! cross-volume strategy (§2.1 / §2.2 / §2.3 / §2.14). Every output flows through here; engines never
//! write the final file. A §0.7 tier-2 trust-kernel LEAF: it depends DOWN only (on `crate::domain`),
//! never up on IPC / orchestrator / the engine registry (§2.0 dependency direction). Unsafe-free — the
//! crate-root `#![deny(unsafe_code)]` (main.rs) covers it. The §2.3.1 resolved-IDENTITY read (P3.6) AND the
//! §2.3.3 TOCTOU-closed parent-dir-handle verify (P3.9) both need NO in-core FFI: the Windows
//! `(volumeSerialNumber, fileIndex)` — read from a PATH (P3.6, `winapi-util`'s `from_path_any`) or from the
//! ALREADY-OPEN dir handle (P3.9, `winapi_util::file::information(&dir_file)` — `GetFileInformationByHandle` on
//! the pinned handle, `winapi-util`'s `AsHandleRef for File`) — comes through `winapi-util`'s SAFE wrapper
//! (with `dunce` for the canonical-path `\\?\`-normalisation), and the Unix side is std `MetadataExt` — no
//! `unsafe` in the core (§2.3.1 `[CORRECTED 2026-07-07]`). The remaining RAW per-OS handle FFI — the
//! §2.1.2/§2.3.3 create-only dir-relative PUBLISH primitives (Linux `renameat2` / macOS `renameatx_np` /
//! Windows `NtSetInformationFile(FileRenameInformationEx)`, P3.12/P3.13/P3.14) — is homed in the single
//! allow-listed `crate::platform` shim (§0.7).
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
//!  - `is_safe_output` — write-target link-safety vs the frozen source set (§2.3.3) — **P3.8**, the path-based
//!    verdict; the **P3.9** `open_verified_parent_dir` below roots the *write* at a verified handle.
//!  - `open_verified_parent_dir(parent, frozen) -> ParentDirVerdict` — §2.3.3 open the parent dir as a PINNED
//!    handle + verify its identity (read FROM the handle) is not a frozen source (TOCTOU-closed) — **P3.9**;
//!    the `VerifiedParentDir` it returns is the handle the P3.12–P3.18 publish roots its dir-relative rename at.
//!  - `output_name` — verbatim-stem + `stem (n).ext` lazy no-clobber candidates (§2.2.1 / §2.10.1) — **P3.10**.
//!  - `check_path_limit` — per-OS component + total path-length validation, fail-never-truncate (§2.2.3) — **P3.11**.
//!  - `atomic_publish(verified_parent, tmp, leaf)` — per-OS create-only exclusive publish, rooted at the P3.9
//!    `VerifiedParentDir`, + the §2.14.3 cross-volume fallback (§2.1 / §2.3.3 / §2.14.3) — **P3.12-P3.18**.
//!  - `location_status` — per-location writability + ephemeral classification, cached per-dir (§2.7.2) — **P3.33**.

use std::ffi::OsString;
use std::fs::File;
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
// [Test-Change: P3.8 — old-obsolete+new-correct, §2.3.3] `expect`→`allow`: P3.8's `is_safe_output` field-reads
// `FileIdentity` and P3.7's `resolve_and_dedup` moves it, so the P2.74 DEAD assertion would error as
// unfulfilled under -D warnings — both consumers unwired until P3.38/P3.49; `allow` fits (cf. P2.63).
#[cfg_attr(
    not(test),
    allow(
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
// [Test-Change: P3.7 — old-obsolete+new-correct, §2.4.1] `expect`→`allow` (a production lint change, NOT a
// test suppression; cf. P2.63): P3.7's `crate::orchestrator::resolve_and_dedup` now references this fn, so the
// P3.6 assertion that it is DEAD would error as unfulfilled under -D warnings — but that consumer is itself
// unwired until P3.49, so the fn's dead-ness is ambiguous and `allow` (permissive) is the correct attribute.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.3.1 resolve_identity (the IO/FFI producer of FileIdentity, P3.6). Referenced by P3.7's \
                  `resolve_and_dedup` (still unwired until the P3.49 spine), so it is dead-at-runtime but no \
                  longer statically unused; the cfg(test) real-FS tests below exercise it."
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

/// The §2.3.3 write-target link-safety verdict — whether publishing to a candidate output path would land
/// on (clobber) a frozen SOURCE file, directly or through a symlink / junction / hardlink (§2.3). Returned
/// by [`is_safe_output`].
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.3.3 is_safe_output's verdict type (P3.8), constructed only by is_safe_output — itself \
                  unwired until the §2.1.1 write sequence (P3.38) — so its dead-ness is ambiguous during the \
                  P3 wiring window; `allow` (permissive) covers it. Exercised by the is_safe_output_tests."
    )
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputSafety {
    /// The write does not resolve onto any frozen source file — publishing here is safe (§2.3.3).
    Safe,
    /// The write would land on / through a frozen source FILE (a clobber — directly, or via a symlink /
    /// junction / hardlink) — the caller MUST divert (§2.7 / P3.34), never publish here (§2.3.3 rule 2).
    ResolvesOntoSource,
}

/// The shared §2.3.3 "resolves onto an original?" membership test: whether `id` resolves to the same file
/// as one of the frozen SOURCEs — by (dev, inode)/file-index identity ([`FileIdentity`]'s `Eq`, catching a hardlink
/// whose canonical path differs, §2.3.4) OR by canonical path (a source reached at exactly this resolved
/// path). Used by BOTH [`is_safe_output`] (the path-based verdict, P3.8) and [`open_verified_parent_dir`]
/// (the TOCTOU-closed handle verify, P3.9), so the two §2.3.3 checks apply one identical rule — a change to
/// "what counts as clobbering an original" cannot silently diverge between the path check and the handle check.
///
/// §2.3.3 states this membership test as resolved-identity / canonical-path EQUALITY (NOT an ancestor-path
/// prefix test): the frozen set holds FILES only (§0.6 invariant 4), so a candidate can be "inside the frozen
/// set" ONLY by resolving ONTO a source file (equality) — a literal path-PREFIX reject would reject the normal
/// beside-source case §2.3.3 explicitly permits ("NOT the container … beside-source is the normal, correct
/// case"). [Build-Session-Entscheidung: P3.9]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.3.3 shared membership helper — called only by is_safe_output (P3.8) + open_verified_parent_dir (P3.9), both unwired until the §2.1.1 write sequence (P3.12+ / P3.38); dead-at-runtime during the P3 wiring window, exercised by the in-module tests. `allow` (permissive) covers the ambiguous dead-ness (cf. OutputSafety)."
    )
)]
fn identity_matches_a_source(id: &FileIdentity, frozen_sources: &[FileIdentity]) -> bool {
    frozen_sources
        .iter()
        .any(|src| src == id || src.canonical_path == id.canonical_path)
}

/// The §2.3.3 no-harm guard: does publishing to `final_path` resolve onto a frozen SOURCE file (SSOT —
/// "never write onto an original")? The pipeline calls this before §2.1's exclusive publish and, on
/// [`OutputSafety::ResolvesOntoSource`], DIVERTS (§2.7) instead of writing. Because it compares the RESOLVED
/// real identity (§2.3.1), an output path that is a symlink resolving back onto a source, and a hardlink
/// whose inode is a source, are both caught where a raw path-string compare misses them (§2.3.4).
///
/// `frozen_sources` are the resolved [`FileIdentity`] of the frozen source FILES (§2.3.2 / §2.4.1 — the P3.7
/// de-dup survivors); the frozen set holds FILES only (§0.6 invariant 4), so landing beside-source INSIDE a
/// dropped folder is the normal case and is NOT rejected — the guard is "would this write resolve onto an
/// ORIGINAL FILE?", never "is this path under a dropped folder?" (§2.3.3). Path/identity EQUALITY with a
/// source is the reject; ANCESTOR containment (a source sitting under the output directory) is not.
///
/// **Fallible (`io::Result`), never a panic** (G4/G14 — this runs on untrusted candidate paths): a genuine
/// resolve failure is a clean `Err` the §2.8 caller maps, NEVER silently treated as Safe. The COMMON case is
/// that `final_path` does not exist yet (§2.1 picks a non-existent name): `canonicalize` fails `NotFound` (the
/// absent leaf) — or `NotADirectory` when a parent component is itself a FILE (an output-dir symlink onto a
/// source file) — and ONLY those two kinds take the §2.3.3 fallback that resolves the PARENT (a non-existent
/// leaf cannot itself be a link, but its parent can be a symlink into a source tree). Any OTHER resolve
/// failure — an interior-NUL path (`InvalidInput`; the G48 "never `Ok` on a null-byte path" T7+T2a contract),
/// a permission error — is surfaced as `Err`, never `Ok(Safe)`. The parent is presumed to EXIST here (§2.7.1
/// create-only ancestor creation runs first for a chosen-root subtree); a missing parent is `Err`. A source
/// AT the would-be path would mean the leaf EXISTS → the existing-target branch handles it, so the fallback
/// needs only the parent-resolves-onto-a-source check. (On a case-insensitive FS a case-variant of a source
/// resolves to the source's real inode and is caught by that existing-target identity check — no residual gap.)
///
/// P3.8 is the path-based verdict; the §2.3.3 TOCTOU-closed dir-handle publish (P3.9) roots the *write* at a
/// verified parent handle so the parent cannot be swapped between this check and the publish — this function
/// yields the verdict, that primitive enforces it atomically.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "§2.3.3 is_safe_output (P3.8) — the write-target link-safety verdict. Its production caller \
                  is the §2.1.1 per-item write sequence (P3.38), which diverts on ResolvesOntoSource; dead in \
                  the production build pending that wiring, exercised by the in-module is_safe_output_tests \
                  below."
    )
)]
pub fn is_safe_output(
    final_path: &Path,
    frozen_sources: &[FileIdentity],
) -> io::Result<OutputSafety> {
    // Whether ANY frozen source resolves to the same file as `id` — the shared §2.3.3 membership test
    // ([`identity_matches_a_source`]), extracted so this path-based verdict and the P3.9 TOCTOU-closed
    // `open_verified_parent_dir` handle verify apply the IDENTICAL "resolves onto an original?" rule and
    // cannot drift. [Build-Session-Entscheidung: P3.9]
    let matches_a_source = |id: &FileIdentity| identity_matches_a_source(id, frozen_sources);

    // §2.3.3 step 1: resolve the target. `final_path` normally does NOT exist yet, so `canonicalize` fails
    // (NotFound — the absent leaf; or NotADirectory — a parent component is a FILE, e.g. an output-dir
    // symlink onto a source file) and the sanctioned fallback resolves the PARENT.
    match resolve_identity(final_path) {
        Ok(target) => {
            // The (rare) already-existing target: its resolved identity is authoritative — a hardlink to a
            // source shares the (dev, inode) even with a different canonical path (§2.3.4), a symlink is
            // followed onto the source it points at, and an existing NON-source file is Safe (§2.2 no-clobber
            // numbering, NOT this guard, reacts to a pre-existing non-source name).
            Ok(if matches_a_source(&target) {
                OutputSafety::ResolvesOntoSource
            } else {
                OutputSafety::Safe
            })
        }
        // §2.3.3 fallback — ONLY for "the leaf does not resolve as a file": NotFound (the absent leaf, the
        // normal §2.1 case) or NotADirectory (a parent component is a FILE — e.g. an output-dir symlink onto a
        // source file, the ENOTDIR that must reach the parent check). [Build-Session-Entscheidung: P3.8 — gate
        // the fallback on exactly these two kinds so an interior-NUL path (InvalidInput) / permission error is
        // surfaced as `Err`, never `Ok(Safe)`: the G48 "never Ok on a null-byte path" (T7+T2a) contract + the
        // no-harm default that an unresolvable target is never silently safe.]
        Err(final_err)
            if matches!(
                final_err.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            // Resolve the PARENT (which must exist, §2.7.1). The non-existent leaf cannot itself be a link, so
            // the only §2.3.3 risk is the PARENT resolving onto a source FILE (an output-dir symlink pointing
            // back at a source). A NORMAL directory that merely CONTAINS sources is a distinct (dev, inode)/
            // path from any source FILE, so beside-source writing there is correctly Safe (§0.6 invariant 4 —
            // the frozen set holds files, not the container). [Build-Session-Entscheidung: P3.8 — the §2.3.3
            // "resolved-parent + leaf == source" case is omitted as provably unreachable HERE: a source at the
            // would-be path means the leaf EXISTS, which lands in the Ok(target) branch above, never here.] If
            // the parent ALSO cannot be resolved, surface the ORIGINAL error (§2.8), never a silent Safe.
            match final_path.parent() {
                Some(parent) => match resolve_identity(parent) {
                    Ok(parent_id) => Ok(if matches_a_source(&parent_id) {
                        OutputSafety::ResolvesOntoSource
                    } else {
                        OutputSafety::Safe
                    }),
                    Err(_) => Err(final_err),
                },
                None => Err(final_err),
            }
        }
        // Any OTHER resolve failure (InvalidInput — an interior-NUL / dangerous path; PermissionDenied; …) is a
        // genuine error the §2.8 caller maps — NEVER Ok(Safe) (the G48 null-byte contract + no-harm default).
        Err(final_err) => Err(final_err),
    }
}

/// The §2.3.3 verdict of [`open_verified_parent_dir`] — either the verified, pinned parent-directory handle
/// the write is rooted at, or the divert signal (mirrors [`OutputSafety`], but carries the handle on the safe
/// path).
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.3.3 open_verified_parent_dir's verdict type (P3.9), constructed only by that fn — itself \
                  unwired until the P3.12–P3.18 dir-relative publish / the §2.1.1 write sequence (P3.38) — so \
                  its dead-ness is ambiguous during the P3 wiring window; `allow` (permissive) covers it (cf. \
                  OutputSafety). Exercised by open_verified_parent_dir_tests."
    )
)]
#[derive(Debug)]
pub enum ParentDirVerdict {
    /// The parent directory opened, is a real directory, and its resolved identity is NOT a frozen source —
    /// the [`VerifiedParentDir`] handle the §2.1.2 create-only publish (P3.12–P3.14) roots its dir-relative
    /// rename at (§2.3.3). Because the identity was read FROM this open handle, a subsequent path swap of the
    /// parent cannot redirect the write.
    Verified(VerifiedParentDir),
    /// The opened parent resolves onto a frozen SOURCE file (a clobber — directly or via a symlink / junction /
    /// hardlink) — the caller MUST divert (§2.7 / P3.34), never publish here (§2.3.3 rule 2).
    ResolvesOntoSource,
}

/// A parent directory opened as a PINNED OS handle whose resolved identity has been verified NOT to be a
/// frozen source (§2.3.3) — the TOCTOU-closing root the §2.1.2 dir-handle-relative create-only publish
/// (P3.12–P3.14) renames the leaf against. The handle is pinned to the directory inode at open, so a
/// post-open path swap of the parent (to a symlink into a source tree) cannot redirect the publish: the rename
/// resolves `leaf` THROUGH this handle, not by re-parsing a path string (§2.3.3 "Parent-directory safety is
/// made atomic via a directory-handle, not a path").
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.3.3 the verified parent-dir handle (P3.9), constructed only by open_verified_parent_dir; \
                  its consumer is the P3.12–P3.18 dir-relative publish (unwired until then), so it is \
                  dead-at-runtime in the P3 wiring window; `allow` (permissive) covers the ambiguous dead-ness \
                  (cf. OutputSafety). Exercised by open_verified_parent_dir_tests."
    )
)]
#[derive(Debug)]
pub struct VerifiedParentDir {
    /// The open directory handle — Unix `File::open` (a directory opens read-only; the is-a-directory check on
    /// the OPEN handle is the `O_DIRECTORY`-equivalent) / Windows `FILE_FLAG_BACKUP_SEMANTICS` (required to open
    /// a directory handle). The P3.12–P3.14 publish primitives rename `leaf` RELATIVE to this handle
    /// (`renameat2`/`renameatx_np` on its `as_raw_fd()`; `FileRenameInformationEx` with its `RootDirectory`), so
    /// the parent cannot be link-redirected between the verify and the write (§2.3.3).
    handle: File,
}

impl VerifiedParentDir {
    /// The pinned directory handle the §2.1.2 dir-relative create-only publish (P3.12–P3.14) roots its rename
    /// at: on Unix the caller reads `as_raw_fd()` for `renameat2(…, newdirfd, leaf, RENAME_NOREPLACE)`; on
    /// Windows `as_raw_handle()` for `FILE_RENAME_INFORMATION_EX.RootDirectory` (§2.3.3).
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "§2.3.3 accessor for the P3.12–P3.18 dir-relative publish (unwired until then); \
                      dead-at-runtime in the P3 wiring window, exercised by open_verified_parent_dir_tests; \
                      `allow` covers the ambiguous dead-ness (cf. the VerifiedParentDir it belongs to)."
        )
    )]
    pub fn dir_handle(&self) -> &File {
        &self.handle
    }
}

/// The §2.3.3 TOCTOU-closed parent-directory-handle safety primitive: open `parent` as a PINNED directory
/// handle, then verify — from the OPEN handle, not a re-resolved path — that its resolved identity is not a
/// frozen SOURCE. Returns the verified handle ([`ParentDirVerdict::Verified`]) the §2.1.2 create-only publish
/// (P3.12–P3.14) renames the leaf against, or the divert signal ([`ParentDirVerdict::ResolvesOntoSource`]).
///
/// This closes the parent-swap TOCTOU that a path-only [`is_safe_output`] (P3.8) leaves open (§2.3.3
/// "Parent-directory safety is made atomic via a directory-handle, not a path"): even if `parent` is swapped
/// to a symlink into a source tree AFTER this call, the handle stays pinned to the directory inode opened
/// here, and the publish renames `leaf` THROUGH this handle — so the write lands in the verified directory,
/// never a redirected one.
///
/// **Fallible (`io::Result`), never a panic** (G4/G14 — this runs on untrusted candidate paths OUTSIDE the
/// §2.12 boundary): a genuine failure is a clean `Err` the §2.8 caller maps, NEVER silently treated as
/// verified. Specifically —
/// - `parent` does not exist / cannot be `stat`'d → `Err` (e.g. `NotFound`).
/// - `parent` is not a directory — a regular file, a **FIFO / device / socket**, or a symlink resolving onto
///   any of those (incl. onto a source FILE) → `Err(NotADirectory)`. A fast `stat` TYPE PRE-CHECK rejects it
///   BEFORE the open, so `File::open` — which on a Unix FIFO blocks indefinitely waiting for a writer (an
///   in-core DoS an adversary could plant at the parent path, worse than the panic the no-panic policy forbids)
///   — is never reached on a non-directory; the fstat on the OPEN handle then re-verifies dir-ness.
/// - an interior-NUL / dangerous path → `Err(InvalidInput)` (the G48 "never `Ok` on a null-byte path" T7+T2a
///   contract): `std` rejects it before the FS is touched.
///
/// The type pre-check is a `stat` (never blocks), so a **pre-existing** FIFO / device parent can never hang the
/// open. A narrow residual remains only if `parent` is swapped dir→FIFO in the µs window between the pre-check
/// `stat` and the open — a negligible local-race liveness edge (a hang, recoverable), NOT a no-harm violation:
/// the identity (the no-harm-critical key) is still read from the pinned handle. A fully race-free open would
/// need `O_DIRECTORY|O_NONBLOCK` via a `libc` edge — deliberately avoided to keep this module std-only.
///
/// The identity used for the verify is read FROM the pinned handle (Unix `fstat` via `MetadataExt`; Windows
/// `GetFileInformationByHandle` on the open handle via `winapi_util::file::information`), so it is the
/// authoritative "same file?" key AT open time — the TOCTOU-closing property. `canonical_path` is the
/// best-effort representative (re-resolved via `dunce::canonicalize`); a swap between open and that resolve can
/// only make the canonical-path leg over-match (→ divert, the no-harm direction), never under-match, so safety
/// rests on the handle-read identity. [Build-Session-Entscheidung: P3.9]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "§2.3.3 open_verified_parent_dir (P3.9) — the TOCTOU-closed parent-dir-handle primitive. Its \
                  production caller is the §2.1.1 write sequence (P3.38) via the P3.12–P3.18 publish; statically \
                  unused in the production build until that wiring lands (`expect` auto-flags the moment it \
                  does), exercised by the in-module open_verified_parent_dir_tests."
    )
)]
pub fn open_verified_parent_dir(
    parent: &Path,
    frozen_sources: &[FileIdentity],
) -> io::Result<ParentDirVerdict> {
    // §2.3.3 — FAST TYPE PRE-CHECK before opening: `stat` the resolved target (follows symlinks, does NOT
    // open, so it never blocks). This rejects a non-directory parent — critically a **FIFO / device / socket**
    // — up front, so the subsequent open cannot hang: `File::open` on a Unix FIFO blocks indefinitely waiting
    // for a writer (an in-core DoS an adversary could plant at the parent path). A `stat` never blocks; a NUL
    // path / missing parent still surface here as `InvalidInput` / `NotFound`. The authoritative dir-verify is
    // still the fstat on the OPEN handle below — this pre-check is LIVENESS, not the trust boundary; the
    // no-harm-critical identity is read from the pinned handle. [Build-Session-Entscheidung: P3.9]
    if !std::fs::metadata(parent)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            "§2.3.3: the output parent path is not a directory (pre-open type check)",
        ));
    }

    // §2.3.3 step 1 — open the parent dir handle FIRST, pinning the directory inode. Any open failure
    // (NotFound; InvalidInput on an interior-NUL path — the G48 T7+T2a "never Ok on a null-byte path" contract;
    // PermissionDenied) surfaces as `Err`, never a silent verify. Per-OS cfg'd let-blocks (the resolve_identity
    // idiom): exactly one compiles per target.
    #[cfg(unix)]
    let handle = {
        // Unix: `File::open` opens a directory read-only; a symlink is FOLLOWED to the resolved real dir
        // (§2.3.4), matching `resolve_identity`. Dir-ness is enforced by the is-a-directory check on the OPEN
        // handle below — the `O_DIRECTORY`-equivalent, without pulling in `libc` for the arch-specific flag
        // constant. [Build-Session-Entscheidung: P3.9]
        File::open(parent)?
    };
    #[cfg(windows)]
    let handle = {
        use std::os::windows::fs::OpenOptionsExt;
        // Windows: a directory handle requires FILE_FLAG_BACKUP_SEMANTICS (0x02000000) — `File::open` alone
        // fails on a directory. This is the same flag `winapi-util`'s `from_path_any` opens directories with
        // (Win32 CreateFile docs). [Build-Session-Entscheidung: P3.9]
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(parent)?
    };

    // §2.3.3 — the handle must be a DIRECTORY. `metadata()` reads the OPEN handle (fstat / dir-handle
    // GetFileInformationByHandle), so this reflects the pinned inode, never a re-resolved path. A non-dir
    // handle (a file, or a symlink resolving onto a source FILE) → `NotADirectory` (the §2.8 divert-or-fail
    // path), mirroring `is_safe_output`'s NotADirectory fallback — the "output dir symlinked onto a source
    // file" reject, closed here at OPEN time.
    let meta = handle.metadata()?;
    if !meta.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            "§2.3.3: the output parent path is not a directory",
        ));
    }

    // §2.3.3 step 2 — build the handle's identity FROM the pinned handle (the TOCTOU-closing read) and verify
    // it is not a frozen source. `canonical_path` is the best-effort representative; the authoritative
    // "same file?" key is the (dev, inode)/file-index read from the handle.
    let canonical_path = dunce::canonicalize(parent)?;
    #[cfg(unix)]
    let (dev_or_volserial, inode_or_fileindex) = {
        use std::os::unix::fs::MetadataExt;
        // `fstat` on the open fd (`handle.metadata()` above) — the identity of the pinned directory.
        (meta.dev(), meta.ino())
    };
    #[cfg(windows)]
    let (dev_or_volserial, inode_or_fileindex) = {
        // GetFileInformationByHandle on the ALREADY-OPEN dir handle via `winapi-util`'s SAFE `AsHandleRef for
        // File` wrapper — no re-open (a fresh `from_path_any` would reintroduce the parent-swap TOCTOU), no
        // `unsafe` in the core. Both accessors are `u64` (winapi-util 0.1.11).
        let info = winapi_util::file::information(&handle)?;
        (info.volume_serial_number(), info.file_index())
    };
    let identity = FileIdentity {
        canonical_path,
        dev_or_volserial,
        inode_or_fileindex,
    };

    // On reject → divert (§2.7); on pass → hand back the verified, pinned handle for the P3.12–P3.18 publish.
    if identity_matches_a_source(&identity, frozen_sources) {
        Ok(ParentDirVerdict::ResolvesOntoSource)
    } else {
        Ok(ParentDirVerdict::Verified(VerifiedParentDir { handle }))
    }
}

/// The §2.2.1 output-name candidate generator ([`output_name`]): a LAZY iterator over the output file NAMES
/// for one (source, target-extension), yielding the base `stem.ext` first, then `stem (1).ext`,
/// `stem (2).ext`, … The §2.2.2 exclusive-publish loop (P3.15) consumes these one at a time — trying each with
/// the create-only dir-handle publish until one wins — so numbering is decided by the kernel's exclusive
/// create, NEVER by a directory-list max+1 pre-scan (itself a TOCTOU race, §2.2.2). It yields NAMES only (an
/// `OsString` file name); the destination directory is prepended by the §1.8 `OutputPlan` (P3.37).
///
/// The stem is the source's `file_stem` taken **byte-for-byte** (`OsString`, no `to_string_lossy` — §2.10.1:
/// operations are `OsStr`-lossless, so emoji / CJK / RTL / non-UTF-8 names survive), and the extension is the
/// TARGET's canonical lowercase extension regardless of the source's true-vs-claimed extension (§2.2.1). The
/// numbering suffix is the SSOT **space-paren** shape (a space, then `(n)`, then the extension) — never
/// `stem_1` / `stem-1` / a hash. Only ASCII digits are appended, so byte-preservation of the stem holds.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.2.1 output_name's candidate-iterator type (P3.10), constructed only by `output_name` — \
                  whose consumer is the §2.2.2 numbering ↔ exclusive-publish loop (P3.15) / the §1.8 OutputPlan \
                  (P3.37) — so it is dead-at-runtime during the P3 wiring window; `allow` (permissive) covers \
                  the ambiguous dead-ness (cf. OutputSafety). Exercised by output_name_tests."
    )
)]
#[derive(Debug, Clone)]
pub struct OutputNameCandidates {
    /// The source's verbatim `file_stem` (byte-for-byte `OsString`, §2.10.1) — the invariant part of every
    /// candidate.
    stem: OsString,
    /// The TARGET's canonical lowercase extension (bare, no leading dot — e.g. `tsv` / `csv` / `webp`, §2.2.1).
    ext: OsString,
    /// The next numbering index to yield: `Some(0)` → the base `stem.ext`; `Some(n)` (n ≥ 1) → `stem (n).ext`;
    /// `None` → exhausted at the `u64` ceiling (unreachable — the §2.2.2 publish loop caps retries far below).
    next_n: Option<u64>,
}

impl OutputNameCandidates {
    /// §2.2.1: build the candidate generator from the SOURCE path (its verbatim `file_stem`, §2.10.1) and the
    /// TARGET's bare canonical lowercase extension. Returns `None` iff `source` has no file name / stem (a
    /// `.` / `..` / root path) — the §2.8 caller maps that; a frozen SOURCE always has a stem, so `None` is the
    /// "not a real file path" edge, never a normal outcome. [Build-Session-Entscheidung: P3.10] `ext` is the
    /// bare canonical extension (no leading dot); the caller passes the format registry's value.
    fn new(source: &Path, ext: &str) -> Option<Self> {
        // `file_stem` is a PURE path operation (no FS access): `photo.jpg` → `photo`; the multi-dot
        // `my.report.final.docx` → `my.report.final` (only the LAST extension is dropped, §2.2.1); the dotfile
        // `.bashrc` → `.bashrc`. Taken as an `OsString` — no lossy conversion (§2.10.1).
        let stem = source.file_stem()?.to_os_string();
        Some(Self {
            stem,
            ext: OsString::from(ext),
            next_n: Some(0),
        })
    }
}

impl Iterator for OutputNameCandidates {
    type Item = OsString;

    fn next(&mut self) -> Option<OsString> {
        let n = self.next_n?;
        // Build the candidate for `n` on the verbatim stem — `OsString::push` appends without any lossy
        // re-encode, and the digits / separators are ASCII, so the stem's exact bytes are preserved (§2.10.1).
        let mut name = self.stem.clone();
        if n == 0 {
            // The base candidate `stem.ext` (the §2.5 re-run / same-format case collides here and the publish
            // loop falls through to the numbered variants below, §2.2.1).
            name.push(".");
            name.push(&self.ext);
        } else {
            // The SSOT space-paren numbered variant `stem (n).ext` (never `stem_n` / `stem-n` / a hash, §2.2.1).
            name.push(" (");
            name.push(n.to_string());
            name.push(").");
            name.push(&self.ext);
        }
        // Advance; `checked_add` stops at the `u64` ceiling rather than overflow-panic (the in-core no-panic
        // policy, G4/G14) — unreachable in practice (the §2.2.2 publish loop caps retries far below).
        self.next_n = n.checked_add(1);
        Some(name)
    }
}

/// §2.2.1 the output-name candidate generator: given a SOURCE path and the TARGET's bare canonical lowercase
/// extension, yield the LAZY [`OutputNameCandidates`] the §2.2.2 exclusive-publish loop (P3.15) consumes —
/// `stem.ext`, then `stem (1).ext`, `stem (2).ext`, … on the verbatim source stem (§2.10.1). Returns `None`
/// iff `source` has no file stem (a `.` / `..` / root path); a frozen source always has one.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "§2.2.1 output_name (P3.10) — the lazy candidate generator. Its production consumer is the \
                  §2.2.2 numbering ↔ exclusive-publish loop (P3.15) / the §1.8 OutputPlan (P3.37); statically \
                  unused in the production build until that wiring lands (`expect` auto-flags the moment it \
                  does), exercised by the in-module output_name_tests."
    )
)]
pub fn output_name(source: &Path, ext: &str) -> Option<OutputNameCandidates> {
    OutputNameCandidates::new(source, ext)
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

#[cfg(test)]
mod is_safe_output_tests {
    //! §6.4.1/§6.4.3 real-FS (G15/G31/G48) for the §2.3.3 write-target link-safety verdict
    //! ([`is_safe_output`], P3.8) — the no-harm guard that never lets a conversion write onto an original
    //! source (SSOT). Never mock the FS under test (test-strategy §0.1): real temp files / hardlinks, driving
    //! the real `resolve_identity` (P3.6). The identity (NOT path-string) comparison is what catches a
    //! hardlink to a source a naive path compare misses (§2.3.4). The symlink legs are the unix module below.
    use super::*;

    /// The frozen source identities for a set of real paths (the P3.7 de-dup produces these; here built
    /// directly from real files).
    fn sources(paths: &[&Path]) -> Vec<FileIdentity> {
        paths
            .iter()
            .map(|p| resolve_identity(p).expect("resolve a source"))
            .collect()
    }

    // §2.3.3 (G15/G31): the NORMAL case — a new output beside a source, inside the same (dropped) folder, is
    // SAFE. The frozen set holds the source FILE, not the container dir, so writing a sibling is not a clobber.
    #[test]
    fn a_new_output_beside_a_source_is_safe() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let src = dir.path().join("data.csv");
        std::fs::write(&src, b"a,b\n1,2\n").expect("write the source");
        let frozen = sources(&[&src]);
        let out = dir.path().join("data.tsv"); // a non-existent sibling output name
        assert_eq!(
            is_safe_output(&out, &frozen).expect("resolve the parent"),
            OutputSafety::Safe,
            "§2.3.3: writing a new file beside a source (same folder) is the normal, safe case"
        );
    }

    // §2.3.3 rule 2 (G15/G31): writing directly ONTO an existing source path is rejected (a clobber).
    #[test]
    fn writing_onto_an_existing_source_is_rejected() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let src = dir.path().join("original.csv");
        std::fs::write(&src, b"payload").expect("write the source");
        let frozen = sources(&[&src]);
        assert_eq!(
            is_safe_output(&src, &frozen).expect("resolve the existing target"),
            OutputSafety::ResolvesOntoSource,
            "§2.3.3: the output path IS a frozen source — reject, never clobber"
        );
    }

    // §2.3.2/§2.3.4 (G15/G31): a HARDLINK to a source (same (dev, inode), DIFFERENT path) is rejected — the
    // identity check catches it where a path-string compare would not. The headline no-harm proof for P3.8.
    #[test]
    fn a_hardlink_to_a_source_is_rejected() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let src = dir.path().join("source.csv");
        std::fs::write(&src, b"payload").expect("write the source");
        let link = dir.path().join("elsewhere.csv");
        // FAT/exFAT (no hardlinks, §2.3.4) → Unsupported/PermissionDenied → skip only THAT; a real temp dir
        // is NTFS/ext4/APFS, so the skip does not fire in practice. [Build-Session-Entscheidung: P3.8 — mirror
        // the P3.6/P3.7 hardlink-test skip guard.]
        let linked = std::fs::hard_link(&src, &link);
        if matches!(&linked, Err(e) if matches!(e.kind(), std::io::ErrorKind::Unsupported | std::io::ErrorKind::PermissionDenied))
        {
            return;
        }
        linked.expect("create the hardlink (a non-unsupported error is a real failure)");
        let frozen = sources(&[&src]); // only the ORIGINAL path is frozen
        assert_eq!(
            is_safe_output(&link, &frozen).expect("resolve the hardlink"),
            OutputSafety::ResolvesOntoSource,
            "§2.3.4: a hardlink shares the source's (dev, inode) — writing onto it clobbers the original"
        );
    }

    // §2.3.3 (G15/G31): a genuinely unrelated new output (a different file, under no source) is SAFE — the
    // over-reject control.
    #[test]
    fn an_unrelated_new_output_is_safe() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let src = dir.path().join("source.csv");
        std::fs::write(&src, b"x").expect("write the source");
        let frozen = sources(&[&src]);
        let other = tempfile::tempdir().expect("a second, unrelated temp dir");
        let out = other.path().join("output.tsv");
        assert_eq!(
            is_safe_output(&out, &frozen).expect("resolve the parent"),
            OutputSafety::Safe,
            "§2.3.3: an output unrelated to any source is safe"
        );
    }

    // §2.3.3 (G15): with NO frozen sources, every resolvable output is Safe (nothing to clobber) — the empty
    // set is a valid input, not a special case.
    #[test]
    fn no_frozen_sources_means_every_output_is_safe() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let out = dir.path().join("out.tsv");
        assert_eq!(
            is_safe_output(&out, &[]).expect("resolve the parent"),
            OutputSafety::Safe,
            "§2.3.3: no frozen sources → nothing to clobber → Safe"
        );
    }

    // §2.8/G4/G14 (G15/G48): a target whose PARENT does not exist is a clean Err, never a panic — the no-harm
    // default (an unresolvable target is surfaced, not silently Safe). Exercises the fallible resolve on this
    // in-core untrusted-path surface (the G48 no-panic contract).
    #[test]
    fn a_missing_parent_directory_is_err_not_a_panic() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let missing = dir.path().join("no_such_dir").join("out.tsv"); // parent does not exist
        assert!(
            is_safe_output(&missing, &[]).is_err(),
            "§2.8: an output whose parent cannot be resolved is a clean Err, never a panic"
        );
    }

    // §2.3.3 (G15/G31): an EXISTING output file that is NOT a frozen source is Safe — §2.2 no-clobber
    // numbering (not this guard) reacts to a pre-existing non-source name. Exercises the Ok(target)-not-a-
    // source arm (a mutant returning ResolvesOntoSource there would otherwise survive the suite).
    #[test]
    fn an_existing_non_source_output_is_safe() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let src = dir.path().join("source.csv");
        std::fs::write(&src, b"payload").expect("write the source");
        let frozen = sources(&[&src]);
        let existing_output = dir.path().join("leftover.tsv");
        std::fs::write(&existing_output, b"a prior, non-source file")
            .expect("write a pre-existing output");
        assert_eq!(
            is_safe_output(&existing_output, &frozen).expect("resolve the existing target"),
            OutputSafety::Safe,
            "§2.3.3: an existing output that is not a frozen source is Safe (no-clobber numbering handles it)"
        );
    }

    // §2.8/G48 (G15): an interior-NUL output path is a clean Err, NEVER Ok(Safe) — the G48 "never Ok on a
    // null-byte path" (T7+T2a) contract. `std` rejects an interior NUL (`InvalidInput`) BEFORE touching the
    // FS, and the fallback is gated to NotFound/NotADirectory so `is_safe_output` surfaces it rather than
    // resolving the (real) parent to a false Safe.
    #[test]
    fn a_null_byte_output_path_is_err_never_safe() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        // A NUL-bearing leaf UNDER a real, resolvable parent — the fallback must NOT swallow this to Safe.
        let nul_path = dir.path().join("a\0b.tsv");
        assert!(
            is_safe_output(&nul_path, &[]).is_err(),
            "§2.8/G48: an interior-NUL output path is a clean Err, never Ok(Safe) (T7+T2a)"
        );
    }
}

// §6.4.3 real-FS unix (G15/G31): the symlink legs of §2.3.3 ([`is_safe_output`], P3.8). TWO STACKED cfg attrs
// (`#[cfg(test)]` then `#[cfg(unix)]`), NOT `all(test, unix)` — the P1.17 compound-cfg clippy::expect_used
// trap. Windows symlink creation needs privilege (fs_guard's resolve_identity tests gate that leg); the
// cross-platform hardlink test above proves the identity-based reject on every OS.
#[cfg(test)]
#[cfg(unix)]
mod is_safe_output_unix_tests {
    use super::*;

    fn sources(paths: &[&Path]) -> Vec<FileIdentity> {
        paths
            .iter()
            .map(|p| resolve_identity(p).expect("resolve a source"))
            .collect()
    }

    // §2.3.3 rule 2: an output that IS a symlink onto a source is rejected — `canonicalize` follows the link
    // to the source's identity (§2.3.4). Exercises the existing-target reject branch via a followed symlink.
    #[test]
    fn an_output_symlink_onto_a_source_is_rejected() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let src = dir.path().join("source.csv");
        std::fs::write(&src, b"payload").expect("write the source");
        let alias = dir.path().join("alias.csv");
        std::os::unix::fs::symlink(&src, &alias).expect("create a unix symlink");
        let frozen = sources(&[&src]);
        assert_eq!(
            is_safe_output(&alias, &frozen).expect("resolve the symlink"),
            OutputSafety::ResolvesOntoSource,
            "§2.3.4: an output symlink is followed onto the source it points at — reject"
        );
    }

    // §2.3.3 "the output dir is a symlink that resolves back onto a source file": a NON-existent leaf under a
    // parent that symlinks onto a source FILE is rejected — exercises the fallback's parent-resolves-onto-a-
    // source branch (the `final_path` resolve fails, the parent resolves onto the source's identity).
    #[test]
    fn a_new_leaf_under_a_parent_symlinked_onto_a_source_is_rejected() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let src = dir.path().join("source.csv");
        std::fs::write(&src, b"payload").expect("write the source");
        // `bad_parent` is a symlink pointing at the source FILE; a would-be leaf "under" it resolves the
        // parent onto the source.
        let bad_parent = dir.path().join("bad_parent");
        std::os::unix::fs::symlink(&src, &bad_parent)
            .expect("symlink a dir-name onto a source file");
        let frozen = sources(&[&src]);
        let out = bad_parent.join("new.tsv"); // non-existent leaf under a symlink-to-a-file parent
        assert_eq!(
            is_safe_output(&out, &frozen).expect("resolve the parent"),
            OutputSafety::ResolvesOntoSource,
            "§2.3.3: an output-dir path resolving onto a source file is rejected (fallback parent check)"
        );
    }
}

#[cfg(test)]
mod open_verified_parent_dir_tests {
    //! §6.4.1/§6.4.3 real-FS (G15/G31/G48) for the §2.3.3 TOCTOU-closed parent-dir-handle primitive
    //! ([`open_verified_parent_dir`], P3.9) — opens the parent as a PINNED handle and verifies its identity
    //! (read FROM the handle) is not a frozen source. Never mock the FS under test (test-strategy §0.1): real
    //! temp dirs / files, driving the real per-OS open + handle identity read. The symlink legs are the unix
    //! module below (Windows symlink creation needs privilege — the P3.6 resolve_identity tests gate that leg).
    use super::*;

    /// The frozen source identities for a set of real paths (the P3.7 de-dup produces these; here built
    /// directly from real files).
    fn sources(paths: &[&Path]) -> Vec<FileIdentity> {
        paths
            .iter()
            .map(|p| resolve_identity(p).expect("resolve a source"))
            .collect()
    }

    // §2.3.3 (G15/G31): the NORMAL case — a real directory with an empty frozen set opens and VERIFIES, and
    // the returned handle is a genuine OPEN directory (metadata read back from the pinned handle). Also the
    // no-frozen-sources leg (nothing to clobber).
    #[test]
    fn a_real_directory_opens_and_verifies() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let verdict = open_verified_parent_dir(dir.path(), &[]).expect("open the parent dir");
        assert!(
            matches!(&verdict, ParentDirVerdict::Verified(_)),
            "§2.3.3: an empty frozen set verifies the parent (nothing to clobber)"
        );
        if let ParentDirVerdict::Verified(verified) = &verdict {
            assert!(
                verified
                    .dir_handle()
                    .metadata()
                    .expect("stat the pinned dir handle")
                    .is_dir(),
                "§2.3.3: the verified handle is an OPEN directory (the publish roots its rename at it)"
            );
        }
    }

    // §2.3.3 (G15/G31): the beside-source NORMAL case — a directory that CONTAINS a frozen source FILE is
    // VERIFIED, not rejected. The frozen set holds FILES (§0.6 invariant 4), so landing beside-source inside
    // the container is the correct case (the guard is "resolves onto an original FILE?", not "under a folder
    // holding sources?"). The load-bearing over-reject control for P3.9.
    #[test]
    fn a_directory_containing_a_source_is_verified_not_rejected() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let src = dir.path().join("data.csv");
        std::fs::write(&src, b"a,b\n1,2\n").expect("write the source file inside the dir");
        let frozen = sources(&[&src]);
        let verdict = open_verified_parent_dir(dir.path(), &frozen).expect("open the parent dir");
        assert!(
            matches!(verdict, ParentDirVerdict::Verified(_)),
            "§2.3.3: a directory that merely CONTAINS a source is verified — beside-source is the normal case"
        );
    }

    // §2.3.3 (G15/G31): the verify FIRES — a parent directory whose OWN resolved identity is in the frozen set
    // is rejected to ResolvesOntoSource (kills an always-Verified mutant). In production the frozen set holds
    // FILES, so this is unreachable for a real directory; the test exercises the verification branch by
    // construction (the dir's own identity used as a synthetic "source").
    #[test]
    fn a_parent_whose_identity_is_a_frozen_source_is_rejected() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let dir_id = resolve_identity(dir.path()).expect("resolve the dir's own identity");
        let verdict = open_verified_parent_dir(dir.path(), &[dir_id]).expect("open the parent dir");
        assert!(
            matches!(verdict, ParentDirVerdict::ResolvesOntoSource),
            "§2.3.3: a parent whose own identity IS a frozen source is rejected — the handle verify fires"
        );
    }

    // §2.8/G4/G14 (G15/G48): a non-existent parent is a clean Err, never a panic — the open fails and is
    // surfaced (never a silent verify).
    #[test]
    fn a_nonexistent_parent_is_err() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let missing = dir.path().join("no_such_dir");
        assert!(
            open_verified_parent_dir(&missing, &[]).is_err(),
            "§2.8: a parent that cannot be opened is a clean Err (the §2.8 caller maps it), never a panic"
        );
    }

    // §2.3.3/§2.8 (G15/G31): a plain FILE used as the parent is rejected to Err(NotADirectory) — opening it
    // yields a file handle, and the is-a-directory check on the OPEN handle rejects it (the ENOTDIR case the
    // path check reaches via its NotADirectory fallback). The "parent resolves onto a source file" reject,
    // closed at OPEN time by the handle approach.
    #[test]
    fn a_file_used_as_a_parent_is_not_a_directory_err() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let file = dir.path().join("iam_a_file.bin");
        std::fs::write(&file, b"x").expect("write a plain file");
        let err = open_verified_parent_dir(&file, &[]).expect_err("a file parent must be an Err");
        assert_eq!(
            err.kind(),
            io::ErrorKind::NotADirectory,
            "§2.3.3: a non-directory parent handle is rejected as NotADirectory"
        );
    }

    // §2.8/G48 (G15): an interior-NUL parent path is a clean Err, NEVER a panic and never Ok — the G48 "never
    // Ok on a null-byte path" (T7+T2a) contract. `std` rejects the interior NUL (InvalidInput) at open, before
    // the FS is touched.
    #[test]
    fn a_null_byte_parent_path_is_err_never_a_panic() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let nul_path = dir.path().join("a\0b");
        assert!(
            open_verified_parent_dir(&nul_path, &[]).is_err(),
            "§2.8/G48: an interior-NUL parent path is a clean Err, never Ok / never a panic (T7+T2a)"
        );
    }
}

// §6.4.3 real-FS unix (G15/G31): the symlink legs of §2.3.3's parent-dir-handle primitive
// ([`open_verified_parent_dir`], P3.9). TWO STACKED cfg attrs (`#[cfg(test)]` then `#[cfg(unix)]`), NOT
// `all(test, unix)` — the P1.17 compound-cfg clippy::expect_used trap. Windows symlink creation needs
// privilege (fs_guard's resolve_identity tests gate that leg), so the follow-symlink legs are unix-homed;
// the cross-platform NotADirectory + reject tests above cover the non-symlink cases on every OS.
#[cfg(test)]
#[cfg(unix)]
mod open_verified_parent_dir_unix_tests {
    use super::*;

    // §2.3.4: a parent that is a symlink to a real DIRECTORY is FOLLOWED — the handle pins the resolved target
    // dir and it verifies (the resolved dir is not a source). The follow-symlink counterpart to the
    // reject-a-symlink-onto-a-file case below (canonicalize + File::open both follow the link, §2.3.4).
    #[test]
    fn a_parent_symlink_to_a_directory_is_followed_and_verified() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let real_dir = dir.path().join("real_out");
        std::fs::create_dir(&real_dir).expect("create the real target dir");
        let link = dir.path().join("link_out");
        std::os::unix::fs::symlink(&real_dir, &link).expect("symlink a dir name onto the real dir");
        let verdict = open_verified_parent_dir(&link, &[]).expect("open through the symlink");
        assert!(
            matches!(verdict, ParentDirVerdict::Verified(_)),
            "§2.3.4: a parent symlink to a directory is followed to the resolved dir and verified"
        );
    }

    // §2.3.3: a parent that is a symlink onto a SOURCE FILE is rejected as NotADirectory — opening it follows
    // the link to the file, and the is-a-directory check on the OPEN handle rejects it. Exactly the "the output
    // dir is a symlink that resolves back onto a source file" case (§2.3.3), closed at OPEN time — so it is
    // rejected regardless of the frozen set (the structural is_dir gate fires before the identity verify).
    #[test]
    fn a_parent_symlink_onto_a_source_file_is_not_a_directory_err() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let src = dir.path().join("source.csv");
        std::fs::write(&src, b"payload").expect("write the source file");
        let bad_parent = dir.path().join("bad_parent");
        std::os::unix::fs::symlink(&src, &bad_parent)
            .expect("symlink a dir name onto a source file");
        let err = open_verified_parent_dir(&bad_parent, &[])
            .expect_err("a parent symlinked onto a file must be an Err");
        assert_eq!(
            err.kind(),
            io::ErrorKind::NotADirectory,
            "§2.3.3: a parent resolving onto a source FILE is rejected (NotADirectory), never published into"
        );
    }

    // §2.3.3 THE HEADLINE TOCTOU-CLOSING PROPERTY (G15/G31): the pinned handle's identity SURVIVES a post-open
    // path swap of `parent`. Open through a symlink to a real dir, then re-point the symlink at a DECOY dir; the
    // handle still reports the ORIGINAL dir's inode (it was pinned to that inode AT open), while the PATH now
    // resolves to the decoy — so the P3.12–P3.18 publish, rooted at this handle, can never be redirected by a
    // swap. Deterministic (no thread race — the OS pins the handle at open time). This proves by TEST what the
    // path-based `is_safe_output` (P3.8) cannot: the central claim of the primitive, not just an assertion.
    #[test]
    fn the_pinned_handle_survives_a_post_open_parent_path_swap() {
        use std::os::unix::fs::MetadataExt;
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let real_dir = dir.path().join("real");
        let decoy_dir = dir.path().join("decoy");
        std::fs::create_dir(&real_dir).expect("create the real dir");
        std::fs::create_dir(&decoy_dir).expect("create the decoy dir");
        let link = dir.path().join("parent_link");
        std::os::unix::fs::symlink(&real_dir, &link).expect("symlink parent → real dir");

        // Bind the owned verified handle; a non-Verified verdict fails the test explicitly via `expect`
        // (test-allowed) rather than a hard-fail macro the deferral gate flags.
        let verified = match open_verified_parent_dir(&link, &[]).expect("open through the symlink") {
            ParentDirVerdict::Verified(v) => Some(v),
            ParentDirVerdict::ResolvesOntoSource => None,
        }
        .expect("precondition: a real dir with an empty frozen set verifies, never ResolvesOntoSource");

        let real_id = resolve_identity(&real_dir).expect("resolve the real dir");
        let decoy_id = resolve_identity(&decoy_dir).expect("resolve the decoy dir");

        // Swap the PATH after open: re-point the symlink at the decoy dir.
        std::fs::remove_file(&link).expect("remove the original symlink");
        std::os::unix::fs::symlink(&decoy_dir, &link).expect("re-point the symlink → decoy dir");

        // The PATH now resolves to the decoy — the post-open swap took effect (the precondition).
        let path_now = resolve_identity(&link).expect("resolve the swapped path");
        assert_eq!(
            (path_now.dev_or_volserial, path_now.inode_or_fileindex),
            (decoy_id.dev_or_volserial, decoy_id.inode_or_fileindex),
            "precondition: the post-open path swap took effect (the symlink now resolves to the decoy)"
        );

        // …but the PINNED HANDLE still reports the ORIGINAL dir's inode — the TOCTOU-closing property.
        let handle_now = {
            let m = verified
                .dir_handle()
                .metadata()
                .expect("fstat the pinned handle after the swap");
            (m.dev(), m.ino())
        };
        assert_eq!(
            handle_now,
            (real_id.dev_or_volserial, real_id.inode_or_fileindex),
            "§2.3.3: the pinned handle keeps the ORIGINAL dir's identity across a post-open path swap"
        );
        assert_ne!(
            handle_now,
            (decoy_id.dev_or_volserial, decoy_id.inode_or_fileindex),
            "§2.3.3: the handle is NOT redirected to the decoy the swapped path now points at"
        );
    }
}

#[cfg(test)]
mod output_name_tests {
    //! §6.4.1 unit (G15) for the §2.2.1 output-name candidate generator ([`output_name`], P3.10). Pure logic,
    //! no FS: the verbatim byte-preserving stem (§2.10.1), the SSOT space-paren numbering shape, and the
    //! target-extension-wins rule — the naming contract made executable. The `for ANY stem` byte-exactness
    //! invariant is the sibling property module.
    use super::*;

    /// The first `count` candidate NAMES of `output_name(source, ext)`.
    fn first_candidates(source: &str, ext: &str, count: usize) -> Vec<OsString> {
        output_name(Path::new(source), ext)
            .expect("a real file path has a stem")
            .take(count)
            .collect()
    }

    // §2.2.1 (G15): base + the first numbered candidates — `photo.jpg` → webp yields `photo.webp`, then the
    // SSOT space-paren `photo (1).webp`, `photo (2).webp`.
    #[test]
    fn base_then_space_paren_numbering() {
        assert_eq!(
            first_candidates("photo.jpg", "webp", 3),
            vec![
                OsString::from("photo.webp"),
                OsString::from("photo (1).webp"),
                OsString::from("photo (2).webp"),
            ],
            "§2.2.1: base `stem.ext`, then the SSOT space-paren `stem (n).ext` (never `_n`/`-n`/a hash)"
        );
    }

    // §2.2.1 (G15): the extension is the TARGET's regardless of the source's true-vs-claimed extension — a
    // misnamed `.jpg`-that-is-PNG converted to webp → `misnamed.webp`.
    #[test]
    fn extension_is_the_target_not_the_source() {
        assert_eq!(
            first_candidates("misnamed.jpg", "webp", 1),
            vec![OsString::from("misnamed.webp")],
            "§2.2.1: the output extension is the target's canonical ext, never the source's"
        );
    }

    // §2.2.1/§2.10.1 (G15): a MULTI-DOT stem is preserved verbatim — only the LAST extension is replaced
    // (`my.report.final.docx` → `my.report.final.pdf`), never split at the first dot.
    #[test]
    fn multi_dot_stem_preserved_verbatim() {
        assert_eq!(
            first_candidates("my.report.final.docx", "pdf", 2),
            vec![
                OsString::from("my.report.final.pdf"),
                OsString::from("my.report.final (1).pdf"),
            ],
            "§2.2.1: the multi-dot stem is kept whole; only the last extension changes"
        );
    }

    // §2.2.1/§2.5 (G15): the SAME-FORMAT re-encode case — `photo.jpg` → jpg yields `photo.jpg` first (which
    // collides with the source in the §2.2.2 publish loop, then numbers away to `photo (1).jpg`), never
    // overwriting the original.
    #[test]
    fn same_format_yields_base_then_numbered() {
        assert_eq!(
            first_candidates("photo.jpg", "jpg", 2),
            vec![OsString::from("photo.jpg"), OsString::from("photo (1).jpg")],
            "§2.2.1: same-format re-encode still starts at `stem.ext`; the publish loop numbers away from the source"
        );
    }

    // §2.10.1 (G15): a DOTFILE (`.bashrc`) has no trailing extension, so its whole name is the stem —
    // `.bashrc` → txt yields `.bashrc.txt`, `.bashrc (1).txt`.
    #[test]
    fn dotfile_whole_name_is_the_stem() {
        assert_eq!(
            first_candidates(".bashrc", "txt", 2),
            vec![
                OsString::from(".bashrc.txt"),
                OsString::from(".bashrc (1).txt"),
            ],
            "§2.10.1: a leading-dot file has no extension — the whole name is the stem"
        );
    }

    // §2.10.1 (G15): a Unicode / emoji / RTL stem survives byte-for-byte — no transliteration / ASCII-fold /
    // emoji-strip (the §2.10.1 verbatim-preservation invariant, the reason operations stay `OsStr`-lossless).
    #[test]
    fn unicode_emoji_rtl_stem_preserved() {
        assert_eq!(
            first_candidates("café_مرحبا_🎉.png", "webp", 2),
            vec![
                OsString::from("café_مرحبا_🎉.webp"),
                OsString::from("café_مرحبا_🎉 (1).webp"),
            ],
            "§2.10.1: Unicode/emoji/RTL stems are preserved verbatim (no fold/strip)"
        );
    }

    // §2.2.1 (G15): a path with no file stem (`..`) → None (the §2.8 caller maps it); never a panic.
    #[test]
    fn a_path_without_a_stem_is_none() {
        assert!(
            output_name(Path::new(".."), "txt").is_none(),
            "§2.2.1: a path with no file stem yields None (the §2.8 caller maps it), never a panic"
        );
    }

    // §2.2.2 (G15): the candidates are DISTINCT and LAZY-unbounded — the first 50 are all different (the
    // publish loop can retry as far as it needs, no directory pre-scan).
    #[test]
    fn candidates_are_distinct() {
        use std::collections::HashSet;
        let got: HashSet<OsString> = first_candidates("data.csv", "tsv", 50)
            .into_iter()
            .collect();
        assert_eq!(
            got.len(),
            50,
            "§2.2.2: every candidate in the lazy sequence is distinct"
        );
    }

    // §2.2.1 (G15): a name that is ONLY an extension-looking token (`mp4`, no dot at all) keeps the WHOLE name
    // as the stem (`file_stem` of a dotless name is the whole name, the dotfile code path) — `mp4` → webp
    // yields `mp4.webp`, `mp4 (1).webp`. The §2.2.1 "names that are only an extension-looking token" case.
    #[test]
    fn extension_looking_token_name_keeps_whole_name_as_stem() {
        assert_eq!(
            first_candidates("mp4", "webp", 2),
            vec![OsString::from("mp4.webp"), OsString::from("mp4 (1).webp")],
            "§2.2.1: a dotless extension-looking name is preserved whole as the stem"
        );
    }
}

#[cfg(test)]
mod output_name_property_tests {
    //! §6.4.2 property (G16) — the §2.2.1/§2.10.1 byte-preservation + shape invariants over adversarial stems
    //! (arbitrary Unicode: emoji, RTL, spaces, dots), shrinking mandatory (proptest, test-strategy §1.3).
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // §2.10.1: for ANY stem + canonical extension, the BASE candidate is EXACTLY `stem.ext` byte-for-byte
        // (the stem's bytes are preserved; only `.ext` is appended) and the n-th is EXACTLY `stem (n).ext`.
        // The stem is recovered from a `stem.srcext` source; the prop_assume filters the (rare) file_stem
        // edge inputs (trailing-dot / dot-only), keeping the property about the CANDIDATE SHAPE, not
        // file_stem's own contract (which the unit tests pin).
        #[test]
        fn candidate_is_exactly_stem_dot_ext_byte_for_byte(
            stem in any::<String>().prop_filter(
                "no path separator / NUL / empty",
                |s| !s.is_empty() && !s.contains('/') && !s.contains('\\') && !s.contains('\0'),
            ),
            ext in "[a-z0-9]{1,6}",
            n in 1u64..1000,
        ) {
            let source = format!("{stem}.srcext");
            prop_assume!(Path::new(&source).file_stem() == Some(std::ffi::OsStr::new(&stem)));

            let candidates = output_name(Path::new(&source), &ext).expect("a `stem.srcext` path has a stem");

            let base = candidates.clone().next().expect("the base candidate");
            let mut expected_base = OsString::from(&stem);
            expected_base.push(".");
            expected_base.push(&ext);
            prop_assert_eq!(base, expected_base, "the base candidate is exactly `stem.ext` byte-for-byte");

            let nth = candidates.take((n as usize) + 1).last().expect("the n-th candidate");
            let mut expected_nth = OsString::from(&stem);
            expected_nth.push(" (");
            expected_nth.push(n.to_string());
            expected_nth.push(").");
            expected_nth.push(&ext);
            prop_assert_eq!(nth, expected_nth, "the n-th candidate is exactly `stem (n).ext` byte-for-byte");
        }
    }
}

// §6.4.1 real-bytes unix (G15): the §2.10.1 "Unix paths are arbitrary bytes — preserved exactly" leg. A
// non-UTF-8 stem is the ONE §2.10.1 category the UTF-8 `String` unit tests + the `any::<String>()` proptest
// structurally cannot reach (a Rust `String` is always valid UTF-8), yet it is exactly what the fn doc +
// §2.10.1 promise "survives". Mirrors the log_redact.rs non-UTF-8 precedent. TWO STACKED cfg attrs
// (`#[cfg(test)]` then `#[cfg(unix)]`), NOT `all(test, unix)` — the P1.17 compound-cfg clippy::expect_used trap.
#[cfg(test)]
#[cfg(unix)]
mod output_name_unix_tests {
    use super::*;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    // §2.10.1 (G15): a NON-UTF-8 stem (`a\xff\xfez`, bytes that are never valid UTF-8) is preserved
    // byte-for-byte in both the base and numbered candidates — no lossy re-encode, no U+FFFD substitution.
    #[test]
    fn a_non_utf8_unix_stem_is_preserved_byte_for_byte() {
        // Source `a\xff\xfez.src`: the stem `a\xff\xfez` is not valid UTF-8 (0xFF/0xFE never appear in UTF-8).
        let source = Path::new(OsStr::from_bytes(b"a\xff\xfez.src"));
        let mut candidates =
            output_name(source, "webp").expect("a non-UTF-8 source still has a file_stem");

        let base = candidates.next().expect("the base candidate");
        assert_eq!(
            base.as_bytes(),
            &b"a\xff\xfez.webp"[..],
            "§2.10.1: the non-UTF-8 stem bytes are preserved verbatim in the base candidate"
        );
        let numbered = candidates.next().expect("the first numbered candidate");
        assert_eq!(
            numbered.as_bytes(),
            &b"a\xff\xfez (1).webp"[..],
            "§2.10.1: the non-UTF-8 stem bytes are preserved verbatim in the numbered candidate"
        );
    }
}
