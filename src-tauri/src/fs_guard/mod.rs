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
//! `unsafe` in the core (§2.3.1 `[CORRECTED 2026-07-07]`). The §2.1.2/§2.3.3 create-only dir-relative PUBLISH
//! primitives split by OS `[re-cut by the P3.12 ruling, 2026-07-07]`: the **Unix** side (Linux `renameat2` /
//! macOS `renameatx_np`, P3.12/P3.13; the §2.14.3 copy fallback P3.17; the durability fsync P3.16/P3.18) rides
//! `rustix`'s SAFE API and lands HERE in `crate::fs_guard` with ZERO `unsafe` (the crate-root deny holds); the
//! **Windows** side (`NtSetInformationFile(FileRenameInformationEx)`, P3.14) is the FIRST — and only — RAW
//! per-OS handle FFI, homed in the single allow-listed `crate::platform` shim (§0.7) with `windows-sys`.
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
//!  - `publish_numbered(verified_parent, parent_dir, tmp, candidates)` — the §2.2.2 numbering ↔ no-clobber retry
//!    loop: drive `output_name`'s lazy candidates through the create-only publish, bumping `(n)` on each
//!    collision, capped at ~10 000 variants (§2.1.2 / §2.2) — **P3.15**.
//!  - `atomic_publish(verified_parent, parent_dir, tmp, candidates, same_volume_intermediate)` — the
//!    §2.1.1/§2.14.3 per-item write composite: step-3 `sync_all(tmp)` durability → the `publish_numbered` loop →
//!    step-6 parent-dir fsync on success (Unix; no-op Windows) — **P3.16**; with the §2.14.3 EXDEV cross-volume
//!    fallback (a cross-device publish → free-space re-check → copy-exactly-once into the caller-provided
//!    same-volume `.part` → exclusive publish) wired inside it — **P3.17**.
//!  - `location_status` / [`LocationStatus`] / [`LocationCache`] — the §2.7.2 per-location destination
//!    classifier (**P3.33**): ephemeral (temp-dir) → writable (exclusive-create + remove a `crate::run`-grammar
//!    probe) → FAT/exFAT no-atomic-publish (Unix), folding the `crate::platform` `is_ephemeral_output_dir` +
//!    `lacks_atomic_publish_primitive` heuristics into a `Writable`/`Divert(reason)` verdict, memoised per-dir
//!    within a run (a planning hint — the §2.1 publish re-checks at P3.36). `fs_guard` is a LEAF: the caller
//!    (§1.8/C4, P3.34+) passes the `crate::run::PublishTemp::probe_name` name in (that module owns the grammar).
//!  - `prepare_output_dir` / [`DestinationMode`] — the §2.7.1 destination-mode output-directory preparation
//!    (**P3.34**): beside-source (the source's own parent dir) or user-chosen-root subtree re-creation
//!    (create-only, ancestor-by-ancestor; a pre-existing dir tolerated, a non-dir collision / `..` traversal
//!    fails clearly), returning the DIRECTORY the §2.1 publish targets — never a pre-baked `final_path`. The
//!    deepest dir's link-safety verify (P3.9 `open_verified_parent_dir`) is taken at PUBLISH (P3.38), not here
//!    (§2.7.1 / §2.3.3 ordering); the §1.8 `OutputPlan` (P3.37) stores the returned dir as `final_dir`.
//!  - `resolve_divert_target` / [`DivertTarget`] — the §2.7.3 divert-ROOT resolution (**P3.35**): walk the
//!    caller-ordered candidate roots (the AppHandle-side `PathResolver` resolves the §2.7.3 Downloads /
//!    Documents roots plus any §2.7.1 user-chosen override and passes them IN — the LEAF is agnostic to the
//!    list-construction priority, which P3.36/P3.37 decide against §2.7.3), re-testing each via the §2.7.2
//!    `location_status` (reusing the run `LocationCache`); the first `Writable` is the divert root, else
//!    `Unavailable` (→ §2.8 `WriteFailed`, never a divert onto a purgeable / another-FAT volume). The §2.7.5
//!    full-chain re-checks on the diverted FINAL path are the late-divert (P3.36) / write-sequence (P3.38)
//!    concern, not here.
//!  - `is_write_divert_trigger` / `recheck_divert_free_space` / `publish_to_divert` — the §2.7.2/§2.7.5
//!    late-divert path (**P3.36**): the trigger classifier (a WRITABILITY publish failure — permission /
//!    read-only / device-gone / network — diverts; `OutOfDisk`/`PathTooLong`/`TooManyCollisions` do not), the
//!    §2.14.4 divert-volume free-space re-check, and the divert publish that re-runs the FULL chain (§2.3.3
//!    link-safety via `open_verified_parent_dir` → §2.14.4 free-space → §2.2.3 path-limit + §2.1 publish via
//!    `atomic_publish`) on the §2.7.3 divert target — "not a degraded path" (§2.7.5). The primary→trigger→divert
//!    orchestration is the §2.1.1 write-sequence (P3.38) concern; this box provides the pieces it wires.
//!  - `compute_output_plan` / [`OutputPlanError`] — the §1.8 per-job OutputPlan computation (**P3.37**): resolve
//!    the output DIRECTORY (Writable → `prepare_output_dir` beside/chosen subtree; Divert → the caller-resolved
//!    §2.7.3 divert root FLAT via a single-candidate `resolve_divert_target`, else `DivertUnavailable`) and
//!    assemble the directory-based [`crate::domain::OutputPlan`] (no pre-baked `final_path`; `publish_temp_dir =
//!    final_dir`, §2.14.1) the §2.1.1 write sequence (P3.38) / C4 plan_output body (P3.49) consume.

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

use crate::domain::{DivertReason, ItemId, OutputPlan};

// [Build-Session-Entscheidung: P2.74] The §2.3.1 resolved-identity TYPE only (option A "split IO-vs-pure",
// Co-Pilot 2026-06-30, owner-ratified): the `resolve_identity` FUNCTION that POPULATES it
// (`dunce::canonicalize` + the per-OS metadata read = IO, needs `dunce` and — on Windows — `winapi-util`
// for the safe `GetFileInformationByHandle` file-identity, both landed at P3.6) is wholly P3 — its
// contract-map entry at P3.1.1, its body at P3.6 — so there is NO half-built function shell and NO
// tagged-`Err` placeholder here (a placeholder with no honest value is the rejected quiet-stub, CLAUDE §5).
//
// Derive / identity choices (the §0.6 sibling types fix the house style):
//  - Core-INTERNAL: `FileIdentity` never crosses IPC — the §2.3.2 de-dup runs core-side and NO path
//    crosses the wire (§2.10.1 / the 2026-07-06 core-owned-paths ruling); the real `resolved_path` lives
//    OFF-WIRE in `FrozenCollectedSet.item_paths` (`ItemPaths`), not this identity — so it derives NO
//    `serde`/`specta`, only `Debug` + `Clone` (it owns a `PathBuf`, hence NOT `Copy`), the internal-type
//    set `FrozenCollectedSet` / `Batch` use.
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
// [Test-Change: P3.87 — old-obsolete+new-correct, §0.7] The P3.8-era not-in-test dead-code lint
// expectation on this fn is RETIRED: the crate-root `fuzz_api::fs_guard_is_safe_output` wrapper (the G48
// fuzz-entry surface) is a production caller since the bin+lib split, so the item is live in every build
// and the old expectation would be unfulfilled (a build error). The §2.1.1 per-item write sequence
// (P3.38) remains the pipeline consumer this verdict was declared for.
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
        reason = "§2.3.3 open_verified_parent_dir's verdict type (P3.9); LIVE from P3.48 (the conductor's §2.1.1 \
                  `publish_completed` opens the verified parent handle before the create-only publish). `allow` \
                  is retained gate-safely (the P3.7/P3.8 precedent). Exercised by open_verified_parent_dir_tests."
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
pub fn output_name(source: &Path, ext: &str) -> Option<OutputNameCandidates> {
    OutputNameCandidates::new(source, ext)
}

/// The §2.2.3 path-limit violation [`check_path_limit`] reports — which OS ceiling the resolved final path
/// would breach. The §2.1.1 write-sequence caller (P3.38) maps it to the §2.8 `ConversionErrorKind::PathTooLong`;
/// `crate::fs_guard` is a §0.7 tier-2 LEAF (it does NOT depend up on `crate::outcome`), so it returns its own
/// verdict here, never a `ConversionErrorKind`. Truncation is NEVER the escape hatch (§2.2.3 / SSOT).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathTooLong {
    /// A single path COMPONENT (a file / dir name) exceeds the 255-unit per-name ceiling — NTFS 255 UTF-16
    /// code units (Windows) / APFS + ext4 255 UTF-8 bytes (macOS / Linux), §2.2.3.
    Component,
    /// The TOTAL resolved path (+ its NUL terminator) exceeds the per-OS ceiling — Windows `MAX_PATH` 260
    /// (the conservative non-long-path-aware limit), macOS `PATH_MAX` 1024, Linux `PATH_MAX` 4096 (§2.2.3).
    Total,
}

/// The 255-unit per-COMPONENT ceiling (a single file / dir name) — NTFS UTF-16 code units / APFS + ext4 UTF-8
/// bytes; identical numeral on all three, measured in the per-OS unit by [`os_str_units`] (§2.2.3).
const MAX_COMPONENT_UNITS: usize = 255;

/// The per-OS TOTAL-path ceiling INCLUDING the NUL terminator (§2.2.3): the resolved path's units + 1 must fit
/// it. Windows `MAX_PATH` (the portable build does NOT assume the long-path-aware opt-in, §2.2.3); macOS /
/// Linux `PATH_MAX`. ConvertIA ships only Win / macOS / Linux (§1); the `all(unix, not(macos))` arm is Linux.
#[cfg(windows)]
const MAX_TOTAL_UNITS_INCL_NUL: usize = 260;
#[cfg(target_os = "macos")]
const MAX_TOTAL_UNITS_INCL_NUL: usize = 1024;
#[cfg(all(unix, not(target_os = "macos")))]
const MAX_TOTAL_UNITS_INCL_NUL: usize = 4096;

/// The length of `s` in the per-OS filesystem UNIT the §2.2.3 limits are stated in: **UTF-16 code units** on
/// Windows (NTFS / `MAX_PATH` count wide chars) and **UTF-8 bytes** on Unix (`NAME_MAX` / `PATH_MAX` count
/// bytes). Measuring in the wrong unit would mis-bound a multi-byte name (an emoji is 1 char but 2 UTF-16
/// units / 4 UTF-8 bytes), so the per-OS unit is load-bearing.
#[cfg(windows)]
fn os_str_units(s: &std::ffi::OsStr) -> usize {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().count()
}
#[cfg(unix)]
fn os_str_units(s: &std::ffi::OsStr) -> usize {
    use std::os::unix::ffi::OsStrExt;
    s.as_bytes().len()
}

/// §2.2.3: validate the **resolved final path** length against the per-OS ceilings BEFORE the §2.1 exclusive
/// create — each NORMAL component ≤ 255 units, and the whole path (+ NUL) ≤ the per-OS total. On breach it
/// returns [`PathTooLong`] (which the §2.1.1 caller maps to §2.8) — **truncation is NEVER the escape hatch**
/// (§2.2.3 / SSOT). Because it runs on the fully-resolved path INCLUDING any §2.7 divert, the divert path
/// enjoys the identical guarantee. On Windows the input is the **user-facing** (non-`\\?\`) resolved form
/// (the `dunce`-normalised §2.3.1 path) — the `\\?\` prefix is ConvertIA's internal syscall mitigation, not
/// what the user/Explorer must open (§2.2.3).
///
/// PURE (no FS access): it only MEASURES the path — `Path::components` and the per-OS unit count are pure, so
/// there is no panic surface here beyond arithmetic, which is `checked_add`-bounded (G4/G14). It appends
/// nothing; the caller feeds it each §2.2 candidate (base then `stem (n).ext`) and stops numbering when a
/// candidate would breach the limit. [Build-Session-Entscheidung: P3.11] the total ceiling is NUL-INCLUSIVE
/// (`units + 1 > LIMIT`), matching §2.2.3's "MAX_PATH 260 … (drive + dirs + name + NUL)" — a path that fills
/// the buffer to the ceiling leaves no room for the terminator the OS APIs require, so 259 wide chars is the
/// Windows usable max.
pub fn check_path_limit(final_path: &Path) -> Result<(), PathTooLong> {
    // Per-COMPONENT: every NORMAL component (a real file / dir name) ≤ 255 units. The Prefix (a Windows drive /
    // UNC root) and RootDir are not filenames and carry no 255-name ceiling, so only `Component::Normal` is
    // checked (§2.2.3).
    for component in final_path.components() {
        if let std::path::Component::Normal(name) = component {
            if os_str_units(name) > MAX_COMPONENT_UNITS {
                return Err(PathTooLong::Component);
            }
        }
    }

    // Per-TOTAL: the whole resolved path + its NUL terminator ≤ the per-OS ceiling. `checked_add(1)` guards the
    // (unreachable) `usize` overflow → treated as too long (a path that long is definitionally over-limit),
    // never a panic (G4/G14).
    let total_incl_nul = os_str_units(final_path.as_os_str()).checked_add(1);
    // Explicit `Some(_) | None` (never a `_` wildcard) — the crate root denies clippy::wildcard_enum_match_arm.
    match total_incl_nul {
        Some(units) if units <= MAX_TOTAL_UNITS_INCL_NUL => Ok(()),
        Some(_) | None => Err(PathTooLong::Total),
    }
}

/// §2.2.4 — the reserved DOS device names a Win32-namespace open aliases to the console/device rather than a
/// file (`CON`/`PRN`/`AUX`/`NUL` + the numbered `COM1`–`COM9` / `LPT1`–`LPT9`, matched case-insensitively).
/// Windows-only: on the other desktops these are ordinary, openable names (§2.2.3 running-OS scoping), so the
/// whole class is gated `cfg(windows)`.
#[cfg(windows)]
const RESERVED_DOS_DEVICE_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// The two §2.2.4 unopenable-name classes. Diagnostic only — the §2.8 `UnopenableOutputName` message names BOTH
/// causes (a reserved word, or a trailing dot/space), so the caller discards the class and keeps the offending
/// token; the enum is kept typed for the classifier's own unit tests + self-documentation.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(
    not(windows),
    allow(
        dead_code,
        reason = "constructed only by the cfg(windows) classifier body; on the other desktops \
                  reject_unopenable_windows_name is a const-Ok and never builds a variant (§2.2.3)"
    )
)]
pub enum UnopenableName {
    /// The first dot-segment (right-trimmed of trailing dots/spaces) is a reserved DOS device name.
    ReservedDevice,
    /// The final character is a dot or a space — silently stripped by the Win32 path layer (an alias onto a
    /// DIFFERENT name, §2.2.4).
    TrailingDotOrSpace,
}

/// **§2.2.4 the Windows-unopenable-name guard (P3.88)** — reject a ConvertIA-CONSTRUCTED output path component
/// (the §2.2.1 leaf candidate or a §2.7.1-recreated subtree directory, NEVER a user-chosen existing ancestor)
/// that Windows cannot treat as an ordinary file: its first dot-segment, right-trimmed of trailing dots/spaces,
/// is a reserved DOS device name (case-insensitive), OR its final character is a dot or a space. The caller
/// fails the item clearly NAMING the offending token — never an alias / rename / truncation (§2.2.1 forbids
/// decorating the stem). The SAME pre-publish validation seam as [`check_path_limit`] (§2.2.3).
///
/// **Windows-only** (§2.2.3 running-OS scoping — the names are legal + harmless on the other desktops, where a
/// `CON.tsv` opens fine): on `not(windows)` this is unconditionally `Ok`, so the guard is a compile-time no-op
/// off Windows. Pure + total, never a panic (G4/G14). [Build-Session-Entscheidung: P3.88]
#[cfg(windows)]
pub fn reject_unopenable_windows_name(component: &OsStr) -> Result<(), UnopenableName> {
    // Lossy is safe for the CLASSIFICATION decision: a reserved name / trailing dot/space is pure ASCII, so a
    // lossy replacement of any non-UTF-8 byte cannot forge OR mask a match (a `\u{FFFD}` is neither a dot/space
    // nor an ASCII device letter). The offending TOKEN the caller surfaces is taken from the real OsStr.
    let name = component.to_string_lossy();

    // (a) first dot-segment (the part before the first `.`), right-trimmed of trailing dots/spaces, equals a
    // reserved DOS device name — `CON`, `CON.csv`, `con` and `CON ` (a trailing-space stem) all alias CON. The
    // reserved cause is the PRIMARY one (§2.2.4 lists it first), so it is reported ahead of a coincidental
    // trailing dot/space (a name that is a reserved device WITH a trailing space is reported `ReservedDevice`).
    let first_segment = name.split('.').next().unwrap_or("");
    let trimmed = first_segment.trim_end_matches([' ', '.']);
    if RESERVED_DOS_DEVICE_NAMES
        .iter()
        .any(|reserved| trimmed.eq_ignore_ascii_case(reserved))
    {
        return Err(UnopenableName::ReservedDevice);
    }

    // (b) final character is a dot or a space → the Win32 path layer silently strips it (an alias onto a
    // DIFFERENT name). Checked on the RAW name, before any trim.
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(UnopenableName::TrailingDotOrSpace);
    }

    Ok(())
}

/// §2.2.4 running-OS scoping (P3.88): on the non-Windows desktops a reserved-word / trailing-dot name is an
/// ordinary openable file, so the guard is a const-`Ok` — the caller's reject arm is unreachable off Windows.
#[cfg(not(windows))]
pub fn reject_unopenable_windows_name(_component: &OsStr) -> Result<(), UnopenableName> {
    Ok(())
}

/// The §2.1.2 outcome of one no-replace publish attempt ([`publish_noreplace`]). Unix-only — the Windows
/// dir-handle publish (P3.14) has its own outcome (a `FileRenameInformationEx` NT-status), and the composite
/// `atomic_publish` (P3.15+) unifies them.
// [Build-Session-Entscheidung: P3.12] Gated `any(linux, macos)` — the SHIPPED unix desktops (§1) — NOT a bare
// `cfg(unix)`: `rustix::fs::renameat_with` is `cfg(any(apple, linux_kernel, redox))`, so a bare `cfg(unix)`
// would build-break a non-shipped unix (FreeBSD/illumos/…) of this public MIT repo with an unresolved import.
// (Distinct from the module's other `cfg(unix)` sites, which use std APIs available on ALL unix.)
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishAttempt {
    /// The exclusive no-replace rename succeeded — `leaf` now names the completed output; `tmp` was MOVED (no
    /// copy; a 0-byte `final` is never created, §2.1.2).
    Published,
    /// `leaf` already exists (`EEXIST`) — the no-replace rename did NOT clobber it (the SSOT never-harm
    /// guarantee); the §2.2.2 numbering loop (P3.15) re-picks the next `stem (n).ext` candidate, `tmp` untouched.
    NameTaken,
    /// The filesystem does not support the single-call no-replace primitive (`EINVAL` on Linux / `ENOTSUP` /
    /// `EOPNOTSUPP` on macOS — a FAT/exFAT-class volume) — the caller falls back to the §2.1.2 `link`+`unlink`
    /// primitive (P3.13); `tmp` untouched.
    Unsupported,
}

/// §2.1.2 the Unix single-call, create-only exclusive publish primitive (never a 0-byte `final`): atomically rename the
/// completed `tmp` onto `leaf` RELATIVE to the P3.9-verified parent dir handle, failing rather than replacing
/// if `leaf` already exists. rustix's `renameat_with(…, RenameFlags::NOREPLACE)` maps to Linux
/// `renameat2(RENAME_NOREPLACE)` and macOS `renameatx_np(RENAME_EXCL)` — BOTH box spellings behind ONE SAFE
/// dirfd-relative call, so this lands with NO `unsafe` in the core (the crate root `#![deny(unsafe_code)]`
/// holds; the P3.12 ruling rejected raw `libc`). Because the destination resolves through the VERIFIED handle
/// (not a re-parsed path string), the parent cannot be link-swapped between the §2.3.3 verify and this publish
/// (the §2.3.3 TOCTOU-closure); NOREPLACE means it either creates `leaf` fresh or fails `EEXIST` — it NEVER
/// clobbers an existing file (the SSOT never-harm guarantee).
///
/// **Errno mapping (no panic, G4/G14):** `EEXIST` → [`PublishAttempt::NameTaken`] (re-pick, P3.15);
/// `EINVAL`/`ENOTSUP`/`EOPNOTSUPP` → [`PublishAttempt::Unsupported`] (fall back to `link`+`unlink`, P3.13);
/// everything else — INCLUDING `EXDEV` (cross-volume, handled by the §2.14.3 copy fallback, P3.17) — surfaces
/// as a §2.8 `io::Error` the caller maps. [Build-Session-Entscheidung: P3.12] `tmp` is renamed FROM `CWD` (it
/// is a full path our own code produced, on the destination volume); only the DESTINATION side is
/// dirfd-relative — that is the TOCTOU-critical side (the source `tmp` is ours, not an attacker's).
// [Build-Session-Entscheidung: P3.12] `any(linux, macos)` (the shipped unix desktops), NOT a bare `cfg(unix)`
// — `rustix::fs::renameat_with` is `cfg(any(apple, linux_kernel, redox))`; a bare `cfg(unix)` would build-break
// a non-shipped unix (FreeBSD/…) for this public MIT repo. See the `PublishAttempt` note above.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn publish_noreplace(
    parent: &VerifiedParentDir,
    tmp: &Path,
    leaf: &std::ffi::OsStr,
) -> io::Result<PublishAttempt> {
    use rustix::fs::{renameat_with, RenameFlags, CWD};
    use rustix::io::Errno;
    // Rename (CWD, tmp) → (verified parent handle, leaf) with RENAME_NOREPLACE / RENAME_EXCL. The destination
    // dirfd is the pinned P3.9 handle (`&File: AsFd`), so a post-verify parent path swap cannot redirect it.
    match renameat_with(CWD, tmp, parent.dir_handle(), leaf, RenameFlags::NOREPLACE) {
        Ok(()) => Ok(PublishAttempt::Published),
        // `EEXIST`: `leaf` is taken — NOREPLACE refused to clobber it (no-harm); re-pick (§2.2.2, P3.15).
        Err(e) if e == Errno::EXIST => Ok(PublishAttempt::NameTaken),
        // `EINVAL` (Linux) / `ENOTSUP` / `EOPNOTSUPP` (macOS): the FS lacks the flag — fall back (§2.1.2, P3.13).
        Err(e) if e == Errno::INVAL || e == Errno::NOTSUP || e == Errno::OPNOTSUPP => {
            Ok(PublishAttempt::Unsupported)
        }
        // Anything else (incl. `EXDEV` cross-volume → §2.14.3 copy fallback, P3.17) is a genuine §2.8 error.
        Err(other) => Err(io::Error::from(other)),
    }
}

/// The §2.1.2 outcome of one `link`+`unlink` fallback publish attempt ([`publish_link_fallback`], P3.13).
/// Unix-only — the portable POSIX fallback reached when [`publish_noreplace`] returned
/// [`PublishAttempt::Unsupported`] (the destination FS lacks the single-call no-replace flag). Its own outcome
/// type (like [`PublishAttempt`], and — on Windows — the P3.14 `FileRenameInformationEx` NT-status), unified by
/// the composite `atomic_publish` (P3.15+). It differs from [`PublishAttempt`] by the [`Self::PublishedResidualTmp`]
/// arm: the link path has a §2.1.3 success-window sub-state the single-call path (which consumes `tmp`
/// atomically) does not.
// [Build-Session-Entscheidung: P3.13] Gated `any(linux, macos)` to MATCH the primitive it falls back FROM
// ([`publish_noreplace`], `any(linux, macos)` for the `renameat_with` cfg): the fallback is only ever reached
// when that returned `Unsupported`, and the P3.15 composite `atomic_publish` that chains them is the same
// shipped-desktops surface (§1). Unlike `renameat_with`, `rustix::fs::{linkat, unlink}` are available on ALL
// unix (`cfg(not(any(espidf, redox)))`), so this is a deliberate cluster-consistency gate, not a
// build-availability one (cf. the `PublishAttempt` cfg note above).
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkPublishAttempt {
    /// `link(tmp, leaf)` created `leaf` fresh AND `unlink(tmp)` then reaped the source — `leaf` names the
    /// completed output and NO residual `*.part` remains (the clean success path; never a 0-byte `final`).
    Published,
    /// `link(tmp, leaf)` created `leaf` fresh (complete + durable — Success, §2.1.3) BUT the subsequent
    /// `unlink(tmp)` FAILED — the leftover `tmp` `*.part` is the §2.1.3 link-form success-window residual,
    /// reclaimed (annotated as residue, NOT an item failure) by the §2.6.4 sweep (P3.25). The output IS
    /// published; the signal is advisory (the sweep reconciles the actual on-disk state).
    PublishedResidualTmp,
    /// `leaf` already exists (`link` → `EEXIST`) — it was NOT clobbered (the SSOT never-harm guarantee); the
    /// §2.2.2 numbering loop (P3.15) re-picks the next `stem (n).ext` candidate, `tmp` untouched. Also the NFS
    /// ambiguous-result case (§2.1.2): a retransmitted `link` RPC may report `EEXIST` though it committed — so
    /// re-pick, never assume success.
    NameTaken,
    /// The destination FS supports NEITHER the single-call no-replace primitive NOR hardlinks (`link` →
    /// `EPERM`/`ENOTSUP`/`EOPNOTSUPP` — the FAT/exFAT-class, §2.1.2 "third fallback"): there is no mechanised
    /// atomic no-clobber publish here, so the caller diverts at §2.7.2 (P3.18 `DivertReason::NoAtomicPublish`);
    /// `tmp` untouched.
    Unsupported,
}

/// §2.1.2 the Unix portable `link`+`unlink` fallback publish primitive (never a 0-byte `final`, so no empty
/// name a crash could leave behind): the create-only publish for a destination whose filesystem lacks the
/// single-call no-replace primitive ([`publish_noreplace`] returned [`PublishAttempt::Unsupported`]).
/// Hard-`link`s the completed `tmp`
/// onto `leaf` RELATIVE to the P3.9-verified parent dir handle (`linkat`, failing `EEXIST` rather than
/// clobbering an existing `leaf` — the SSOT never-harm no-clobber guarantee, one atomic create), then
/// `unlink`s the now-superfluous source `tmp`. rustix's `linkat`/`unlink` are SAFE, so this lands with NO
/// `unsafe` in the core (the crate root `#![deny(unsafe_code)]` holds; the P3.12 ruling rejected raw `libc`).
/// Because the DESTINATION resolves through the VERIFIED handle (not a re-parsed path string), the parent
/// cannot be link-swapped between the §2.3.3 verify and this publish (the §2.3.3 TOCTOU-closure); only the
/// source `tmp` side is path-based — it is our own file, not an attacker's (the same source-is-ours split as
/// [`publish_noreplace`]).
///
/// **The §2.1.3 success-window residual.** Unlike the single-call primitive (which consumes `tmp` atomically),
/// the link path has a brief window after `link` commits but before `unlink` where BOTH `leaf` and the `tmp`
/// `*.part` exist. `leaf` is already complete + durable (Success); if the `unlink` then fails, the leftover
/// `*.part` is a discardable, run-owned residual reclaimed by the §2.6.4 sweep — [`LinkPublishAttempt::PublishedResidualTmp`],
/// NOT an item failure (the output is published). The residual signal is advisory: the sweep reconciles the
/// actual on-disk state, so a benign already-removed `tmp` (e.g. a concurrent sweep) surfacing as a residual
/// signal is harmless.
///
/// **Errno mapping (no panic, G4/G14):** `link` `EEXIST` → [`LinkPublishAttempt::NameTaken`] (re-pick, P3.15;
/// ALSO the NFS ambiguous-result case — treat name-may-be-taken and re-pick, never assume success, §2.1.2);
/// `EPERM`/`ENOTSUP`/`EOPNOTSUPP` (the FS supports NEITHER no-replace rename NOR hardlinks — FAT/exFAT-class) →
/// [`LinkPublishAttempt::Unsupported`], the §2.7.2 divert trigger (P3.18); everything else — INCLUDING `EXDEV`
/// (cross-volume, handled by the §2.14.3 copy fallback, P3.17) — surfaces as a §2.8 `io::Error` the caller maps.
/// [Build-Session-Entscheidung: P3.13] `tmp` is `link`ed/`unlink`ed by path (from `CWD`) — it is a full path our
/// own code produced on the destination volume; only the DESTINATION side is dirfd-relative, the TOCTOU-critical
/// side (the source `tmp` is ours, not an attacker's), mirroring [`publish_noreplace`].
// [Build-Session-Entscheidung: P3.13] `any(linux, macos)` to match the cluster — see the `LinkPublishAttempt` note.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn publish_link_fallback(
    parent: &VerifiedParentDir,
    tmp: &Path,
    leaf: &std::ffi::OsStr,
) -> io::Result<LinkPublishAttempt> {
    use rustix::fs::{linkat, unlink, AtFlags, CWD};
    use rustix::io::Errno;
    // Hard-link (CWD, tmp) → (verified parent handle, leaf), no-follow (`tmp` is our own regular file). The
    // destination dirfd is the pinned P3.9 handle (`&File: AsFd`), so a post-verify parent path swap cannot
    // redirect the new link (§2.3.3 TOCTOU-closure). `link` fails `EEXIST` rather than clobbering `leaf`.
    match linkat(CWD, tmp, parent.dir_handle(), leaf, AtFlags::empty()) {
        Ok(()) => {
            // `leaf` is now a complete, durable name for the output (§2.1.3 Success). Reap the source `tmp`.
            // A failed `unlink` is NOT an item failure — `leaf` is published; the leftover `*.part` is the
            // §2.1.3 success-window residual, reclaimed (annotated, not failed) by the §2.6.4 sweep (P3.25).
            match unlink(tmp) {
                Ok(()) => Ok(LinkPublishAttempt::Published),
                Err(_) => Ok(LinkPublishAttempt::PublishedResidualTmp),
            }
        }
        // `EEXIST`: `leaf` is taken — `link` refused to clobber it (no-harm); re-pick (§2.2.2, P3.15). Also the
        // NFS ambiguous-result case: treat name-may-be-taken → re-pick (never assume success), §2.1.2.
        Err(e) if e == Errno::EXIST => Ok(LinkPublishAttempt::NameTaken),
        // `EPERM`/`ENOTSUP`/`EOPNOTSUPP`: the FS supports neither no-replace rename NOR hardlinks
        // (FAT/exFAT-class) — no mechanised atomic no-clobber publish here; the caller diverts at §2.7.2
        // (P3.18 `DivertReason::NoAtomicPublish`); `tmp` untouched.
        Err(e) if e == Errno::PERM || e == Errno::NOTSUP || e == Errno::OPNOTSUPP => {
            Ok(LinkPublishAttempt::Unsupported)
        }
        // Anything else (incl. `EXDEV` cross-volume → §2.14.3 copy fallback, P3.17) is a genuine §2.8 error.
        Err(other) => Err(io::Error::from(other)),
    }
}

/// The §2.1.2 Windows outcome of one create-only publish ([`publish_rename_windows`], P3.14). Windows-only —
/// its own outcome type (like the Unix [`PublishAttempt`] / [`LinkPublishAttempt`]), unified by the composite
/// `atomic_publish` (P3.15+). The Windows create-only move consumes `tmp` atomically (no residual, unlike the
/// Unix `link`+`unlink` fallback), so there is no `PublishedResidualTmp`-style arm.
#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsPublishAttempt {
    /// The create-only move committed — `leaf` names the completed output; `tmp` was moved (never a 0-byte
    /// `final`).
    Published,
    /// `leaf` already exists — it was NOT clobbered (the SSOT never-harm guarantee); the §2.2.2 numbering loop
    /// (P3.15) re-picks, `tmp` untouched.
    NameTaken,
}

/// §2.1.2/§2.3.3 the Windows create-only publish primitive: move the completed `tmp` onto `leaf` relative to
/// the P3.9-verified parent dir handle via the platform FFI ([`crate::platform::rename_noreplace_at`]), with
/// the §2.1.2 bounded short-backoff AV-retry. The `unsafe` FFI is confined to `crate::platform` (the one
/// allow-listed unsafe surface, G29); this kernel primitive stays memory-safe (the crate root
/// `#![deny(unsafe_code)]` holds) and owns only the retry POLICY. The platform primitive's `TargetExists` →
/// [`WindowsPublishAttempt::NameTaken`] (the SSOT never-harm no-clobber guarantee; re-pick, P3.15); its
/// `Retryable` (a transient AV/indexer lock) is retried a small bounded number of times with a doubling
/// backoff, then surfaces as a §2.8 `WriteFailed` `io::Error`; any other (terminal) error surfaces immediately.
#[cfg(windows)]
pub fn publish_rename_windows(
    parent: &VerifiedParentDir,
    tmp: &Path,
    leaf: &std::ffi::OsStr,
) -> io::Result<WindowsPublishAttempt> {
    use crate::platform::{rename_noreplace_at, WindowsRenameOutcome};
    use std::os::windows::io::AsRawHandle;
    // §2.1.2 bounded AV-retry: a handful of short, doubling backoffs (a transient AV/indexer lock clears in
    // milliseconds), then give up to §2.8 WriteFailed — never an unbounded wait.
    const MAX_RETRIES: u32 = 5;
    const BACKOFF_START: std::time::Duration = std::time::Duration::from_millis(8);
    const BACKOFF_CAP: std::time::Duration = std::time::Duration::from_millis(64);
    let root = parent.dir_handle().as_raw_handle();
    let mut retries_left = MAX_RETRIES;
    let mut backoff = BACKOFF_START;
    loop {
        // A terminal error propagates immediately (`?`); the classified outcomes are matched below.
        match rename_noreplace_at(root, tmp, leaf)? {
            WindowsRenameOutcome::Renamed => return Ok(WindowsPublishAttempt::Published),
            WindowsRenameOutcome::TargetExists => return Ok(WindowsPublishAttempt::NameTaken),
            WindowsRenameOutcome::Retryable => {
                if retries_left == 0 {
                    // The transient AV/indexer lock persisted through every retry → §2.8 WriteFailed.
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "output publish blocked by a persistent lock (AV/indexer) after bounded retries",
                    ));
                }
                retries_left = retries_left.saturating_sub(1);
                std::thread::sleep(backoff);
                backoff = backoff.saturating_mul(2).min(BACKOFF_CAP);
            }
        }
    }
}

/// The unified outcome of ONE create-only publish attempt at a single candidate `leaf`, folding the per-OS
/// primitives (Unix [`PublishAttempt`]/[`LinkPublishAttempt`], Windows [`WindowsPublishAttempt`]) into the one
/// shape the §2.2.2 numbering loop ([`publish_numbered`]) drives. Private — the seed of the module-doc
/// `atomic_publish` single-attempt composite (P3.16 adds the §2.1.1 durability fsync, P3.17 the §2.14.3 EXDEV
/// cross-volume fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SinglePublish {
    /// Published — `residual_tmp` marks the §2.1.3 Unix `link`+`unlink` success-window sub-state (a `*.part`
    /// may remain for the §2.6.4 sweep); the single-call Unix path and the Windows path never set it.
    Published { residual_tmp: bool },
    /// `leaf` is taken — the no-clobber refusal (`EEXIST` / `ERROR_ALREADY_EXISTS`); the loop re-picks the
    /// next `stem (n).ext` candidate (§2.2.2), `tmp` untouched.
    NameTaken,
    /// The destination filesystem supports NEITHER the no-replace rename NOR hardlinks (FAT/exFAT-class, Unix
    /// only) — no mechanised atomic no-clobber publish here (§2.1.2 third fallback); the §2.7.2 divert trigger
    /// (P3.18). Windows never produces it (`MoveFileExW`-without-`REPLACE` is create-only on FAT/exFAT too).
    // Unix-only: only the `any(linux, macos)` publish_once statically constructs this arm, so on the Windows
    // `cfg(test)` leg (where the enum-level `not(test)` allow is inactive) it is never constructed — a variant
    // `allow(dead_code)` covers that platform-conditional dead-ness (harmless/no-op on the Unix legs, where the
    // link-fallback path constructs it). [Build-Session-Entscheidung: P3.15]
    #[allow(dead_code)]
    NoAtomicPublishSupport,
}

/// §2.1.2 one create-only publish attempt at `leaf`, rooted at the P3.9-verified parent dir handle (Unix): try
/// the single-call no-replace primitive ([`publish_noreplace`]) first, and on an FS that lacks the flag
/// ([`PublishAttempt::Unsupported`] — `EINVAL`/`ENOTSUP`) fall back to the portable `link`+`unlink`
/// ([`publish_link_fallback`], §2.1.2); only if THAT is also unsupported (FAT/exFAT — no hardlinks) is there no
/// atomic no-clobber publish here ([`SinglePublish::NoAtomicPublishSupport`], the §2.7.2 divert trigger). The
/// Unix seed of the module-doc `atomic_publish` per-OS composite (P3.16/P3.17 expand it). No panic (G4/G14) —
/// every branch is a structured value or the primitives' propagated `io::Error`. [Build-Session-Entscheidung: P3.15]
// [Build-Session-Entscheidung: P3.15] `any(linux, macos)` — the shipped unix desktops (§1) — matching the
// `publish_noreplace`/`publish_link_fallback` cfg it composes (both `any(linux, macos)`, gated so a non-shipped
// unix of this public MIT repo does not build-break on the `rustix::fs::renameat_with` import). See P3.12.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn publish_once(
    parent: &VerifiedParentDir,
    tmp: &Path,
    leaf: &std::ffi::OsStr,
) -> io::Result<SinglePublish> {
    match publish_noreplace(parent, tmp, leaf)? {
        PublishAttempt::Published => Ok(SinglePublish::Published {
            residual_tmp: false,
        }),
        PublishAttempt::NameTaken => Ok(SinglePublish::NameTaken),
        // The single-call no-replace flag is unsupported on this FS — fall back to the portable `link`+`unlink`
        // primitive (§2.1.2). This branch is reached only on a FAT/exFAT-class destination, which §2.7.2 diverts
        // up front (P3.18). Driving it needs such a filesystem MOUNTED, which is a privileged out-of-process act
        // no test may perform (`crate::isolation` is the sole sanctioned spawn site, G9 invariant (b) / G29) —
        // the bound P3.65 recorded. Its home is therefore the release-candidate verification on real removable
        // media (P11.25, DoD gate 8, which names USB FAT/exFAT explicitly). Everything the resulting VERDICT
        // feeds is covered by P3.65: the `fat_class_destination` fence drives `NoAtomicPublishSupport` through
        // the composite + the reactive late divert (`crate::orchestrator::cross_volume_e2e_tests`), and
        // `crate::platform`'s suite asserts the detector against a kernel-reported FAT mount where one exists.
        PublishAttempt::Unsupported => match publish_link_fallback(parent, tmp, leaf)? {
            LinkPublishAttempt::Published => Ok(SinglePublish::Published {
                residual_tmp: false,
            }),
            LinkPublishAttempt::PublishedResidualTmp => {
                Ok(SinglePublish::Published { residual_tmp: true })
            }
            LinkPublishAttempt::NameTaken => Ok(SinglePublish::NameTaken),
            // Neither no-replace rename NOR hardlinks (FAT/exFAT) — the §2.7.2 divert trigger (P3.18).
            LinkPublishAttempt::Unsupported => Ok(SinglePublish::NoAtomicPublishSupport),
        },
    }
}

/// §2.1.2 one create-only publish attempt at `leaf`, rooted at the P3.9-verified parent dir handle (Windows):
/// the `FileRenameInformationEx` no-replace move + its bounded AV-retry ([`publish_rename_windows`]). Windows
/// has NO FAT/exFAT divert (`MoveFileExW`-without-`REPLACE` is create-only there too, §2.1.2), so it yields only
/// `Published` / `NameTaken` (or a §2.8 `io::Error`), NEVER [`SinglePublish::NoAtomicPublishSupport`]. The
/// Windows seed of the module-doc `atomic_publish` composite. No panic (G4/G14). [Build-Session-Entscheidung: P3.15]
#[cfg(windows)]
fn publish_once(
    parent: &VerifiedParentDir,
    tmp: &Path,
    leaf: &std::ffi::OsStr,
) -> io::Result<SinglePublish> {
    match publish_rename_windows(parent, tmp, leaf)? {
        WindowsPublishAttempt::Published => Ok(SinglePublish::Published {
            residual_tmp: false,
        }),
        WindowsPublishAttempt::NameTaken => Ok(SinglePublish::NameTaken),
    }
}

/// The non-failure outcome of the §2.2.2 numbering ↔ no-clobber publish loop ([`publish_numbered`], P3.15) —
/// either the output was published (at the winning candidate name), or the destination cannot host an atomic
/// no-clobber publish and the caller must divert (§2.7.2). The hard failures (path-too-long, collision-cap, an
/// OS error) are the [`PublishError`] `Err` side. A §0.7 tier-2 LEAF verdict: `crate::fs_guard` does NOT depend
/// up on `crate::domain`'s `DivertReason`, so it returns its own outcome and the §2.1.1 write sequence (P3.38,
/// tier 1) maps [`Self::NoAtomicPublishSupport`] to a §2.7.2 `DivertReason::NoAtomicPublish` re-divert.
#[derive(Debug, PartialEq, Eq)]
pub enum PublishOutcome {
    /// The output was published — `leaf` is the winning candidate name (the base `stem.ext`, or the first free
    /// `stem (n).ext`) and the file now exists in the verified parent dir carrying `tmp`'s exact bytes.
    /// `residual_tmp` marks the §2.1.3 Unix `link`+`unlink` success-window sub-state (a `*.part` may remain for
    /// the §2.6.4 sweep, P3.25); the single-call Unix path and the Windows path never set it.
    Published { leaf: OsString, residual_tmp: bool },
    /// The destination filesystem supports NEITHER the no-replace rename NOR hardlinks (FAT/exFAT-class, Unix
    /// only) — no mechanised atomic no-clobber publish here (§2.1.2 third fallback). §2.7.2 detects this UP
    /// FRONT at `location_status` time and diverts BEFORE publish, so this is a defensive fall-through the
    /// §2.1.1 caller (P3.38) maps to a §2.7.2 `DivertReason::NoAtomicPublish` re-divert — the guarantee is
    /// never silently weakened. Windows never returns it (§2.1.2).
    NoAtomicPublishSupport,
}

/// The hard-failure verdict of the §2.2.2 numbering ↔ no-clobber publish loop ([`publish_numbered`], P3.15). A
/// §0.7 tier-2 LEAF verdict — the P3.48 conductor's §2.1.1 publish legs ([`crate::orchestrator`], tier 1) map it
/// to §2.8 `ConversionErrorKind` (`PathTooLong` / `TooManyCollisions` / `WriteFailed`); `crate::fs_guard` never
/// depends up on `crate::outcome`, so it returns its own verdict here. Not `PartialEq` (it carries an
/// `io::Error`) — callers `match` on it.
//
// [Test-Change: P3.48 — old-obsolete+new-correct, §2.2.3] `PublishError` is LIVE from P3.48 (the conductor's
// `map_publish_error` matches it), so its P3.15 wiring-window `allow(dead_code)` was shed like the sibling
// fs_guard publish primitives — EXCEPT the `PathTooLong(PathTooLong)` PAYLOAD (which §2.2.3 ceiling was
// breached): `publish_numbered` CONSTRUCTS it, but `map_publish_error` matches the VARIANT (`PathTooLong(_)`)
// and discards the payload, so the field is dead though the enum is live (the struct-with-unread-field case
// rustc does not flip on enum-construction alone). Retained for the §2.8/§7.5 diagnostic detail; the
// enum-scoped `allow(dead_code)` covers ONLY that unread field.
#[allow(dead_code)]
#[derive(Debug)]
pub enum PublishError {
    /// A candidate breached the §2.2.3 per-OS path limit (the base name, or the point at which appending
    /// `(n)` / the new extension would overflow) — fail clearly, NEVER truncate (§2.2.3 / SSOT). Maps to §2.8
    /// `PathTooLong`; carries which ceiling ([`PathTooLong`]) was breached.
    PathTooLong(PathTooLong),
    /// The ~10 000-variant no-clobber cap was exhausted — a degenerate destination directory already holding
    /// every candidate name (§2.1.2/§2.2). Maps to §2.8 `TooManyCollisions`.
    TooManyCollisions,
    /// The §2.14.3 EXDEV cross-volume fallback's pre-copy free-space re-check failed: `final`'s volume cannot
    /// host the ~output-sized intermediate the copy would place there (the ~2× destination-volume peak the
    /// §1.10/§2.14.4 preflight does NOT model — "never assume it fits", mirroring §2.7.2's late-divert). Maps to
    /// §2.8 `OutOfDisk`; the item fails clearly and the batch continues (§1.9). Only the cross-volume path
    /// produces it (the direct intra-volume publish never copies). [Build-Session-Entscheidung: P3.17]
    OutOfDisk,
    /// A genuine OS error from a publish attempt (a permission error). Maps to §2.8 `WriteFailed`. `EXDEV`
    /// (Unix) / `ERROR_NOT_SAME_DEVICE` (Windows) is NOT surfaced here — `atomic_publish` (P3.17) intercepts a
    /// cross-device publish failure and routes it to the §2.14.3 copy-into-dest-volume fallback, so an `Io` that
    /// escapes to the §2.1.1 caller (P3.38) is a real write failure, never a cross-volume one.
    Io(io::Error),
    /// §2.2.4 (Windows): the resolved LEAF candidate is a name Windows cannot open as an ordinary file (a
    /// reserved DOS device name in its first dot-segment, or a trailing dot/space) — [`reject_unopenable_windows_name`]
    /// refused it BEFORE the exclusive create. Carries the offending token (the leaf's lossy display) so the §2.8
    /// `UnopenableOutputName` message NAMES it (§2.2.4: "naming the offending token", never an alias/rename). Maps
    /// to §2.8 `UnopenableOutputName`. Windows-only — the guard is a const-`Ok` off Windows. [Build-Session-Entscheidung: P3.88]
    UnopenableName(String),
}

/// §2.2.2 the numbering ↔ no-clobber retry loop with an INJECTABLE attempt `cap` — the testable core of
/// [`publish_numbered`]. Drives `output_name`'s lazy `candidates` through the create-only dir-handle publish
/// ([`publish_once`], rooted at the P3.9-verified `parent`): validate each candidate's resolved final-path
/// length (§2.2.3), attempt the exclusive publish, and on the no-clobber refusal ([`SinglePublish::NameTaken`])
/// bump to the next `stem (n).ext` candidate — so §2.2 numbering and the absolute no-clobber guarantee are the
/// SAME loop (§2.2.2), decided by the kernel's exclusive create, never a stale directory scan. The §2.2.2
/// optional cheap `symlink_metadata` low-number pre-check is deliberately omitted — correctness rests solely on
/// the kernel's exclusive publish (the authority), and a pre-scan would add a TOCTOU-adjacent read for a
/// walking-skeleton optimisation that is not needed. [Build-Session-Entscheidung: P3.15]
///
/// `cap` bounds the total attempts; the public [`publish_numbered`] passes the ~10 000 production cap, and the
/// tests inject a small `cap` to exercise the `TooManyCollisions` exhaustion WITHOUT materialising ~10 000
/// files (a bound injected for testability, NOT a mock of the FS/publish under test — the real temp FS + real
/// publish primitive still run, test-strategy §0.1). `parent_dir` is the RESOLVED destination directory the
/// `parent` handle was opened from (§2.3.3): the per-candidate §2.2.3 check measures `parent_dir.join(leaf)`,
/// the user-facing resolved final path. No panic (G4/G14) — every failure is a structured `Err`, the `cap`
/// makes the loop total, and the counter is `saturating`. [Build-Session-Entscheidung: P3.15]
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn publish_numbered_capped(
    parent: &VerifiedParentDir,
    parent_dir: &Path,
    tmp: &Path,
    candidates: OutputNameCandidates,
    cap: u64,
) -> Result<PublishOutcome, PublishError> {
    let mut attempts: u64 = 0;
    for leaf in candidates {
        // §2.1.2/§2.2: the ~10 000-variant cap — a degenerate dir already holding every candidate fails
        // TooManyCollisions rather than looping unboundedly (never a panic, never a silent success).
        if attempts >= cap {
            return Err(PublishError::TooManyCollisions);
        }
        attempts = attempts.saturating_add(1);

        // §2.2.3: validate the resolved final-path length BEFORE the exclusive create — a candidate whose `(n)`
        // suffix / new extension would overflow the OS limit fails clearly, NEVER truncates (§2.2.3 / SSOT).
        // `parent_dir.join(leaf)` is the resolved final path (parent_dir is the verified destination dir).
        let final_path = parent_dir.join(&leaf);
        if let Err(too_long) = check_path_limit(&final_path) {
            return Err(PublishError::PathTooLong(too_long));
        }

        // §2.2.4 (Windows): reject a CONSTRUCTED leaf Windows cannot open as an ordinary file (a reserved DOS
        // device name in its first dot-segment, or a trailing dot/space) BEFORE the exclusive create — fail
        // clearly NAMING the token, never an alias/rename (§2.2.1). The SAME pre-publish seam as the length
        // check; a const-`Ok` no-op off Windows. The offending token is the leaf's lossy display.
        if reject_unopenable_windows_name(&leaf).is_err() {
            return Err(PublishError::UnopenableName(
                leaf.to_string_lossy().into_owned(),
            ));
        }

        // §2.1.2 fault-injection seam (P3.65) — `#[cfg(test)]` ONLY, so the not(test) PRODUCTION build is
        // byte-identical (this statement compiles out entirely). On a FAT/exFAT-class destination
        // `publish_once` returns `NoAtomicPublishSupport` (its no-replace rename AND `link()` both refuse,
        // §2.1.2's third fallback); MOUNTING such a filesystem is a privileged out-of-process act no test may
        // perform (`crate::isolation` is the sole sanctioned spawn site, G9 (b)/G29), so a test arms this
        // fence for ONE DIRECTORY instead and everything around it — the §2.2.3 path-limit check, the §2.2.2
        // numbering loop, the §2.7.2/§2.7.3 divert this verdict triggers, the §2.1 publish AT the divert
        // target — runs for real. Same class as the `location_status` verdict a test hands
        // `compute_output_plan` by value, and as the injected `avail_bytes`/`cap`: the PRECONDITION is
        // supplied, never the subject. Directory-scoped on purpose — a global switch would make the §2.7.3
        // divert TARGET refuse too, so the divert under test could never complete.
        // [Build-Session-Entscheidung: P3.65]
        #[cfg(test)]
        if fat_class_destination::armed_for(parent_dir) {
            return Ok(PublishOutcome::NoAtomicPublishSupport);
        }

        // §2.2.2: the kernel's exclusive create decides — NameTaken → the next candidate; Published → done;
        // NoAtomicPublishSupport → the §2.7.2 divert (P3.18). A genuine OS error is the §2.8 `Io` verdict.
        match publish_once(parent, tmp, &leaf).map_err(PublishError::Io)? {
            SinglePublish::Published { residual_tmp } => {
                return Ok(PublishOutcome::Published { leaf, residual_tmp });
            }
            SinglePublish::NameTaken => continue,
            SinglePublish::NoAtomicPublishSupport => {
                return Ok(PublishOutcome::NoAtomicPublishSupport);
            }
        }
    }
    // The lazy candidate iterator is exhausted only at the `u64` ceiling — unreachable in practice (the `cap`
    // fires far below), but treat it as TooManyCollisions so the loop is total (never a panic).
    Err(PublishError::TooManyCollisions)
}

/// §2.2.2 the numbering ↔ no-clobber retry loop: hand `output_name`'s lazy `candidates` to the create-only
/// dir-handle publish one at a time — bumping the `stem (n).ext` counter on each no-clobber collision — so §2.2
/// numbering and the absolute no-clobber guarantee are the SAME bounded loop (§2.1.2/§2.2.2), decided by the
/// kernel's exclusive create at the instant of publish, never a stale directory scan. Returns the winning
/// [`PublishOutcome::Published`] `{ leaf, residual_tmp }`, or [`PublishOutcome::NoAtomicPublishSupport`] on a
/// FAT/exFAT-class destination (Unix; the §2.7.2 divert trigger). Fails [`PublishError::PathTooLong`] when a
/// candidate would overflow the §2.2.3 OS path limit (never truncates), [`PublishError::TooManyCollisions`]
/// when the ~10 000-variant cap is exhausted, or [`PublishError::Io`] on a genuine OS error.
///
/// `parent` is the P3.9-verified, pinned parent-dir handle (the publish roots its dir-relative rename at it,
/// TOCTOU-closed, so the numbering retries all target the SAME verified directory); `parent_dir` is the
/// resolved destination directory it was opened from (for the §2.2.3 path-limit measurement); `tmp` is the
/// completed engine output to publish; `candidates` come from [`output_name`] (the caller maps a no-stem
/// `None` to §2.8 before calling). No panic (G4/G14) — this runs on untrusted destination paths outside the
/// §2.12 boundary; every failure is a structured `Err` and the cap makes the loop total.
///
/// **Caller contract (§2.2.3, for the P3.38 wiring):** `parent_dir` MUST be the SAME directory `parent` was
/// opened from — pass the literal `(parent_dir, parent)` pair from one [`open_verified_parent_dir`] call — and,
/// on Windows, the §2.3.1 `dunce`-normalised **non-`\\?\`** resolved form (the user-facing path
/// [`check_path_limit`] measures, §2.2.3), never the verbatim-`\\?\` `std::fs::canonicalize` output (which
/// over-counts the 4-char prefix). A mismatch fails SAFE (it can only over-reject on length, never admit an
/// over-limit path or misdirect the handle-relative publish), but the pair-from-one-call form is the contract.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn publish_numbered(
    parent: &VerifiedParentDir,
    parent_dir: &Path,
    tmp: &Path,
    candidates: OutputNameCandidates,
) -> Result<PublishOutcome, PublishError> {
    // The ~10 000-variant no-clobber cap (§2.1.2/§2.2): a degenerate destination dir already holding every
    // candidate name fails TooManyCollisions rather than looping unboundedly. Counts total attempts (the base
    // `stem.ext` + the numbered variants); the spec's "~10 000" realised as the round 10 000.
    // [Build-Session-Entscheidung: P3.15]
    const MAX_PUBLISH_CANDIDATES: u64 = 10_000;
    publish_numbered_capped(parent, parent_dir, tmp, candidates, MAX_PUBLISH_CANDIDATES)
}

/// §2.1.1 step 3 durability: fsync the completed `tmp`'s bytes to disk BEFORE the publish, so the atomic
/// name-update never exposes a durable NAME over unflushed DATA (atomic-name-update ≠ durable-data). Re-opens
/// `tmp` for WRITE — `sync_all` is `fsync` on Unix (works on any fd) but `FlushFileBuffers` on Windows REQUIRES
/// the `GENERIC_WRITE` access right, so a read-only handle would fail there — and `sync_all`s it; `write(true)`
/// (no `create`, no `truncate`) opens the EXISTING file without altering a byte, purely to obtain a flushable
/// handle. `fsync` is per-INODE, so it flushes whatever the (separate-process) engine wrote and closed, not
/// just this handle's writes. No panic (G4/G14): a missing/locked `tmp` is a clean `io::Error` the caller maps.
/// [Build-Session-Entscheidung: P3.16] the re-open form is used because the engine that wrote `tmp` is a
/// separate process (§3.5) whose write handle the core never holds; the in-core CSV engine's own handle is
/// already closed by the time the §2.1.1 sequence reaches step 3.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn sync_tmp_bytes(tmp: &Path) -> io::Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .open(tmp)?
        .sync_all()
}

/// §2.1.1 step 6 durability (Unix): after a successful publish, fsync `final`'s containing DIRECTORY so the new
/// dentry (the renamed/hard-linked `final` name) survives a crash — a rename/link is atomic but NOT durable
/// without the directory fsync (LWN/evanjones durability findings). The file BYTES are already durable via
/// [`sync_tmp_bytes`] (step 3; on the `link` path the new dentry shares that already-fsync'd inode, so only the
/// dentry needs this dir-fsync — §2.1.1). Takes the P3.9-verified parent dir handle the publish rooted its
/// rename at. No panic (G4/G14). [Build-Session-Entscheidung: P3.16]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn fsync_parent_dir(parent: &VerifiedParentDir) -> io::Result<()> {
    // `File::sync_all` on the pinned directory fd = `fsync(dirfd)`, which flushes the directory's dentries
    // (the new `final` name) to disk. The handle is the same one the publish rooted its dir-relative rename at.
    parent.dir_handle().sync_all()
}

/// §2.1.1 step 6 durability (Windows): a NO-OP. On Windows the new dentry's durability rests on NTFS metadata
/// journaling, not an explicit directory flush; `MOVEFILE_WRITE_THROUGH` on the create-only move is a
/// best-effort metadata flush (its documented effect is for the cross-volume copy-and-delete form), and the
/// §2.1.3 atomicity invariant does NOT depend on it (§2.1.1). The file bytes are still made durable by the
/// step-3 [`sync_tmp_bytes`] `FlushFileBuffers` as on Unix. [Build-Session-Entscheidung: P3.16]
#[cfg(windows)]
fn fsync_parent_dir(_parent: &VerifiedParentDir) -> io::Result<()> {
    Ok(())
}

/// §2.14.3: is `e` the CROSS-DEVICE publish failure the EXDEV fallback intercepts (as opposed to a genuine §2.8
/// write error)? Unix `EXDEV` from a cross-volume `renameat`/`linkat`; Windows `ERROR_NOT_SAME_DEVICE` from the
/// `crate::platform::rename_noreplace_at` NT move. Every OTHER `io::Error` is a real write failure the caller
/// surfaces unchanged, so a non-cross-device error can never silently trigger the (more expensive, though still
/// no-harm) copy fallback — the classifier is exact to keep the common path's §2.8 error mapping faithful.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn is_cross_device(e: &io::Error) -> bool {
    // The cross-device publish-failure errno, per OS — exactly one `let` compiles per target (the
    // `resolve_identity` per-OS-`let` idiom, no block-tail ambiguity). Unix: POSIX `EXDEV` = 18 (identical on
    // Linux + macOS `errno.h`), preserved through `io::Error::from(rustix::io::Errno)` by
    // `publish_noreplace`/`publish_link_fallback`. Windows: `ERROR_NOT_SAME_DEVICE` (17), the Win32 code
    // `RtlNtStatusToDosError` maps `STATUS_NOT_SAME_DEVICE` to inside `crate::platform::rename_noreplace_at`.
    // [Build-Session-Entscheidung: P3.17]
    #[cfg(unix)]
    let cross_device_errno: i32 = 18;
    #[cfg(windows)]
    let cross_device_errno: i32 = windows_sys::Win32::Foundation::ERROR_NOT_SAME_DEVICE as i32;
    e.raw_os_error() == Some(cross_device_errno)
}

/// §2.14.3 the EXDEV cross-volume fallback CORE, with an INJECTABLE `avail_bytes` free-space value — the
/// testable seat (mirroring `publish_numbered_capped`'s injectable `cap`; the real path reads
/// [`crate::platform::available_bytes`], the tests inject a value so both sides of the free-space gate are
/// deterministic without a constrained FS). Given the completed engine output `cross_volume_tmp` sitting on a
/// DIFFERENT volume than `final` (the direct intra-volume publish already failed cross-device), preserve
/// atomicity WITHIN `final`'s volume by copying it into a same-volume intermediate and exclusively publishing
/// THAT. The §2.14.3 order:
///
/// - **(c) pre-copy free-space re-check** — the copy makes the output's bytes exist a SECOND time on `final`'s
///   volume (peak ~2× output), which the §1.10/§2.14.4 preflight does NOT model; if `avail_bytes` (the caller's
///   [`crate::platform::available_bytes`] read of `final`'s volume) is below the intermediate's size (≈ the
///   `cross_volume_tmp` byte length), fail [`PublishError::OutOfDisk`] BEFORE writing a byte ("never assume it
///   fits", §2.7.2).
/// - **(d) copy EXACTLY ONCE** — `std::fs::copy(cross_volume_tmp, same_volume_intermediate)` places the bytes on
///   `final`'s volume. `same_volume_intermediate` is the caller-provided (P3.38, via `crate::run`) run-owned
///   `.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part` sibling of `final` — a NAMED, swept home (§2.6.3),
///   never an anonymous `$TMPDIR` temp. `fs_guard` is a §0.7 tier-2 LEAF (it does not know the `crate::run`
///   naming), so the intermediate PATH is passed in, not minted here.
/// - **sync** the copied intermediate durable ([`sync_tmp_bytes`], §2.1.1 step 3).
/// - **(e) publish** the intermediate → `final` via the §2.2.2 numbering ↔ no-clobber loop
///   ([`publish_numbered`]). The COPY is OUTSIDE this loop, so a name collision re-renames the SAME
///   already-copied intermediate to the next `stem (n).ext` — the expensive cross-volume copy happens EXACTLY
///   ONCE, only the cheap intra-volume exclusive-rename loops (§2.14.3).
/// - **(f) dir-fsync** the destination directory on success ([`fsync_parent_dir`], Unix; Windows no-op).
///
/// `cross_volume_tmp` is COPIED, never moved — it stays on its own volume for the §2.6 run-scope cleanup
/// (`crate::run::cleanup_item` reclaims it by the recorded `final_dir` set); only the same-volume intermediate is
/// consumed by the publish. No panic (G4/G14) — every failure is a structured `Err`. [Build-Session-Entscheidung: P3.17]
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn publish_cross_volume_checked(
    parent: &VerifiedParentDir,
    parent_dir: &Path,
    cross_volume_tmp: &Path,
    candidates: OutputNameCandidates,
    same_volume_intermediate: &Path,
    avail_bytes: u64,
) -> Result<PublishOutcome, PublishError> {
    // (c) §2.14.3 pre-copy free-space re-check: the intermediate is ≈ the output size (= the cross-volume tmp's
    // byte length). Fail OutOfDisk BEFORE writing a byte if `final`'s volume cannot host it (never assume it
    // fits, §2.7.2). A genuine `metadata` failure on our own tmp is a §2.8 `Io`.
    let need = std::fs::metadata(cross_volume_tmp)
        .map_err(PublishError::Io)?
        .len();
    if avail_bytes < need {
        return Err(PublishError::OutOfDisk);
    }
    // (d) §2.14.3 copy EXACTLY ONCE — place the output's bytes on `final`'s volume in the caller's run-owned
    // `.part` intermediate. `std::fs::copy` is the documented `EXDEV` remedy (copy into the destination volume,
    // then rename); the intermediate carries a fresh unique `.part` name (§2.14.1), so this never clobbers user
    // data. On the cross-device path the source `cross_volume_tmp` is LEFT for the §2.6 cleanup (copied, not
    // moved).
    std::fs::copy(cross_volume_tmp, same_volume_intermediate).map_err(PublishError::Io)?;
    // §2.1.1 step 3: the copied intermediate's bytes are durable BEFORE the rename.
    sync_tmp_bytes(same_volume_intermediate).map_err(PublishError::Io)?;
    // (e) §2.14.3/§2.2.2 publish the SAME already-copied intermediate through the numbering loop — a collision
    // re-renames it to the next candidate (copy-exactly-once; only the cheap intra-volume rename loops).
    let outcome = publish_numbered(parent, parent_dir, same_volume_intermediate, candidates)?;
    // (f) §2.1.1 step 6: on a successful publish, fsync the containing dir so the new dentry is crash-durable
    // (Unix; a no-op on Windows). NoAtomicPublishSupport created no `final`, so there is no dentry to flush.
    if matches!(outcome, PublishOutcome::Published { .. }) {
        fsync_parent_dir(parent).map_err(PublishError::Io)?;
    }
    Ok(outcome)
}

/// §2.1.1/§2.14.3 the atomic, durable, no-clobber publish — the per-output-item write composite: make the
/// completed `tmp`'s bytes durable (step 3, [`sync_tmp_bytes`]), publish it through the §2.2.2 numbering ↔
/// no-clobber loop (step 5, [`publish_numbered`]), and on a successful publish fsync the containing directory so
/// the new dentry is durable (step 6, [`fsync_parent_dir`] — Unix; a no-op on Windows). This is the module-doc
/// `atomic_publish` composite, with the §2.14.3 EXDEV cross-volume fallback (P3.17) wired inside it.
///
/// **The §2.14.3 cross-volume fallback (P3.17).** In the rare case where `tmp` did not land on `final`'s volume
/// (§2.14.1's same-volume placement could not be guaranteed — a quirky mount, a scratch dir the destination FS
/// disallows), the direct publish fails CROSS-DEVICE (`EXDEV` / `ERROR_NOT_SAME_DEVICE`, [`is_cross_device`]).
/// `atomic_publish` intercepts exactly that, reads `final`'s volume free space
/// ([`crate::platform::available_bytes`]), and hands off to [`publish_cross_volume_checked`], which re-checks
/// free space, copies `tmp` into `same_volume_intermediate` EXACTLY ONCE, and exclusively publishes THAT within
/// `final`'s volume. Callers see the same [`PublishOutcome`] / [`PublishError`] whether the publish went direct
/// or via the fallback (§2.14.3 "callers never see the distinction"). **On the fallback path `tmp` is COPIED,
/// not moved — it is left on its own volume for the §2.6 run-scope cleanup** (unlike the direct path, whose
/// rename consumes `tmp`); this is why P3.38 records every `final_dir` (incl. cross-volume) for `cleanup_run`.
///
/// Returns the [`PublishOutcome`] / [`PublishError`] of the (direct or fallback) numbering loop — the durability
/// steps add no new outcome, only `io::Error`s (surfaced as [`PublishError::Io`] → §2.8 `WriteFailed`), plus the
/// fallback's [`PublishError::OutOfDisk`] (→ §2.8 `OutOfDisk`) when `final`'s volume can't host the ~output-sized
/// copy. On [`PublishOutcome::NoAtomicPublishSupport`] (the FAT/exFAT §2.7.2 divert signal) NO dentry was
/// created, so the dir-fsync is correctly skipped. **Atomicity + no-clobber come SOLELY from the create-only
/// exclusive publish (§2.1.2/§2.1.3); the fsyncs add DURABILITY, never atomicity, and the cross-volume path's
/// ONLY rename is intra-volume + exclusive** (the copy is never a cross-volume rename, §2.14.3) — a lost fsync
/// degrades to "the write may not survive a power cut", never to a clobber or a truncated `final`.
///
/// `parent` / `parent_dir` / `tmp` / `candidates` carry the same meaning + the same caller contract as
/// [`publish_numbered`] (the `(parent_dir, parent)` pair from one [`open_verified_parent_dir`] call, Windows
/// non-`\\?\` form). `same_volume_intermediate` is the caller-provided (P3.38, via `crate::run`) run-owned
/// `.convertia-…-.part` sibling of `final` used ONLY on the §2.14.3 fallback path (a cheap path the caller
/// always computes; unused — never created — on the common direct path). No panic (G4/G14) — every failure is a
/// structured `Err`.
///
/// **Caller contract (§2.1.1 step 7, for the P3.38 write sequence):** a [`PublishError::Io`] from the STEP-3
/// sync means `final` was never created (the publish never ran — nothing on disk to reconcile), but an `Io`
/// from the STEP-6 dir-fsync means the publish ALREADY SUCCEEDED (`final` EXISTS, only its dentry-durability is
/// uncertain). These two are indistinguishable from the return value, so P3.38's step-7 handling must NOT
/// assume `final` is absent on an `Io` error — it reconciles residues via the §2.6 sweep, never a blind remove,
/// and a step-6 failure never means the original was harmed (no-clobber held; only crash-durability degraded).
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn atomic_publish(
    parent: &VerifiedParentDir,
    parent_dir: &Path,
    tmp: &Path,
    candidates: OutputNameCandidates,
    same_volume_intermediate: &Path,
) -> Result<PublishOutcome, PublishError> {
    // §2.1.1 step 3: the bytes are durable BEFORE the rename — a name-update is atomic but not durable-data.
    sync_tmp_bytes(tmp).map_err(PublishError::Io)?;
    // §2.1.3 fault-injection seam (P3.19.1) — `#[cfg(test)]` ONLY, so the not(test) PRODUCTION build is
    // byte-identical (this statement compiles out entirely). When a test arms `kill_after_sync`, the publish
    // "dies" HERE — in the post-`sync_all`-pre-rename window — so the crash/power-loss two-state-invariant test
    // can inspect the on-disk state (§2.1.3 state-2: a durable but UNPUBLISHED `*.part`, no `final`). No real
    // process can be killed at this exact instant from outside, so the seam is the only way to prove the
    // invariant dynamically (unlike the §2.1.2 AV-retry, which a real lock triggers). [Build-Session-Entscheidung: P3.19.1]
    #[cfg(test)]
    if kill_after_sync::armed() {
        return Err(PublishError::Io(io::Error::new(
            io::ErrorKind::Interrupted,
            "P3.19.1 fault-injection: killed in the post-sync-pre-rename window",
        )));
    }
    // The §2.14.3 EXDEV fallback re-runs the numbering loop over a same-volume COPY, so keep a candidate clone
    // for it: the direct attempt below consumes `candidates`, and on a cross-device failure it errored at the
    // FIRST attempt (a cross-volume source fails EXDEV regardless of the target name), so the fallback needs a
    // FRESH iterator. Cheap — two `OsString`s + a counter. [Build-Session-Entscheidung: P3.17]
    let fallback_candidates = candidates.clone();
    // §2.1.1 step 5: try the DIRECT intra-volume numbering ↔ no-clobber publish (P3.15) — the common path
    // (`tmp` is on `final`'s volume, §2.14.1).
    match publish_numbered(parent, parent_dir, tmp, candidates) {
        Ok(outcome) => {
            // §2.1.1 step 6: on a successful direct publish, fsync the containing dir (Unix; no-op Windows).
            if matches!(outcome, PublishOutcome::Published { .. }) {
                fsync_parent_dir(parent).map_err(PublishError::Io)?;
            }
            Ok(outcome)
        }
        // §2.14.3: the direct publish hit a CROSS-DEVICE failure (`tmp` landed on a different volume than
        // `final` — the rare case §2.14.1's same-volume placement could not guarantee). Fall back to
        // copy-into-`final`'s-volume-then-exclusive-publish. Read `final`'s volume free space HERE — lazily, so
        // there is NO free-space read on the common direct path — and hand it to the testable fallback core.
        Err(PublishError::Io(e)) if is_cross_device(&e) => {
            let avail = crate::platform::available_bytes(parent_dir).map_err(PublishError::Io)?;
            publish_cross_volume_checked(
                parent,
                parent_dir,
                tmp,
                fallback_candidates,
                same_volume_intermediate,
                avail,
            )
        }
        // Any other publish error (PathTooLong / TooManyCollisions / a genuine non-cross-device Io) surfaces
        // unchanged — the §2.14.3 fallback fires ONLY on a real cross-device failure.
        Err(other) => Err(other),
    }
}

/// The §2.7.2 per-location destination classification verdict ([`location_status`], P3.33): whether a
/// candidate output directory can hold a conversion RESULT, or must **divert** (with the §0.6 reason). The
/// §1.8/C4 destination planning reads it — `Writable` → publish beside-source / under the chosen root;
/// `Divert(r)` → the §2.7.3 divert-target resolution (P3.35) with `r` ∈ {`Ephemeral`, `Unwritable`,
/// `NoAtomicPublish`}. A planning HINT, never a commitment — the real §2.1 publish re-checks (P3.36 late
/// divert on a post-probe writability flip). [Build-Session-Entscheidung: P3.33]
///
/// No `dead_code` attribute: it is `location_status`'s return type + `LocationCache`'s stored verdict, so
/// rustc marks it USED even while its §1.8/C4 reader (P3.34+) is unbuilt (unlike `location_status` /
/// `LocationCache`, which are statically unreferenced in the production build until then).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationStatus {
    /// The dir accepts a create AND is neither ephemeral nor atomic-publish-incapable — publish here.
    Writable,
    /// The dir must be diverted (§2.7.2), carrying the §0.6 `DivertReason` (`Ephemeral`/`Unwritable`/
    /// `NoAtomicPublish`) the §2.7.3 resolution + the §1.12 summary read.
    Divert(DivertReason),
}

/// **The §2.7.2 per-location writability & ephemerality classifier (P3.33)** — classify a candidate output
/// `dir` into a [`LocationStatus`] the §1.8/C4 planning uses to decide beside-source-vs-divert. Three tests,
/// short-circuiting in order (cheapest + most-specific first):
///
/// 1. **Ephemeral** (§2.7.2, `crate::platform::is_ephemeral_output_dir`): under a known OS temp root the OS
///    may silently purge? → `Divert(Ephemeral)`, WITHOUT writing a probe (no probe residue left in a temp dir).
/// 2. **Writable** (§2.7.2): does the dir accept a create? Exclusive-create the caller-supplied `probe_name`
///    (`.convertia-<InstanceId>-probe-<rand>.part`, the `crate::run` grammar — this §0.7 tier-2 LEAF module
///    PERFORMS the create, `crate::run` OWNS the name, the P3.18 decision) then remove it. ANY create failure
///    (PermissionDenied / ReadOnly / network / gone) → `Divert(Unwritable)`. A create SUCCESS whose remove
///    FAILS is still **writable** (the create is the test); the leftover probe residue is reclaimed by the
///    §2.6.3 `InstanceId`-liveness sweep (P3.24), never a divert.
/// 3. **No atomic publish** (§2.7.2, Unix-only, `crate::platform::lacks_atomic_publish_primitive`): a
///    FAT/exFAT-class FS that accepts a create yet offers no atomic no-clobber publish → `Divert(NoAtomicPublish)`.
///    A `statfs` read error is treated as NOT FAT-class (the reactive §2.1.2 publish-time backstop catches a
///    missed one — the P3.18 "list-miss honesty"), never an error. Windows FAT is a true create-only move
///    (§2.1.2), so its detector is a no-op → never diverted here.
///
/// **Infallible** — every failure maps to a `LocationStatus`, never an `Err`: the caller reads a definitive
/// verdict. Panic-free (the crate no-panic deny, G4/G14) — the probe create/remove and the `statfs` are all
/// fallible ops whose errors map to a verdict. `fs_guard` is a §0.7 tier-2 LEAF: the `crate::run`-grammar
/// `probe_name` is passed IN (never a `crate::run` dependency here). [Build-Session-Entscheidung: P3.33]
// [Test-Change: P3.49 — old-obsolete+new-correct, §1.8] the P3.33 `allow(dead_code)` is removed: the §1.8/C4
// `plan_output` preview (`crate::orchestrator::plan_output_preview` → C4 `plan_output`) is now a LIVE production
// caller of `location_status`, so the "dead until P3.34+" annotation is obsolete. The `LocationCache` memo + its
// `new`/`classify` (P3.33) are ALSO live in production — the P3.48 C6 conductor (`run_conversion`) threads the
// cache per-item — so their stale "dead until P3.34+" attributes are swept in this same commit (a P3.48-era
// miss corrected here; a production lint cleanup, not a test suppression).
pub fn location_status(dir: &Path, probe_name: &OsStr) -> LocationStatus {
    // 1. Ephemeral first — short-circuit BEFORE the writable probe so no probe residue is written into a temp
    //    dir the OS may purge, and it is the cheapest + most-specific classification (§2.7.2).
    if crate::platform::is_ephemeral_output_dir(dir) {
        return LocationStatus::Divert(DivertReason::Ephemeral);
    }
    // 2. Writable probe: exclusive-create the pre-RunId probe then remove it (§2.7.2). ANY create failure means
    //    "the dir does not accept a create" → unwritable (a UUID-rand-collision `AlreadyExists` is astronomically
    //    unlikely, and a spurious divert is SAFE — it never loses data). The file handle is dropped BEFORE the
    //    remove (Windows cannot remove an open file). A cleanup (remove) failure leaves the residue for the
    //    §2.6.3 sweep and is NOT a divert — the create succeeded, which is the test.
    //    [Build-Session-Entscheidung: P3.33] §2.7.2's diagnostic "the failure is logged locally (§7.5)" is
    //    DEFERRED — `fs_guard` is a §0.7 non-logging tier-2 leaf (no `log::`/`tracing::` here), and the residue
    //    is NAMED (`-probe-` grammar) for the §2.6.3 InstanceId-liveness sweep (P3.24), which IS the observable
    //    reclaim. The load-bearing §2.7.2 half — "still writable, never a divert" — is honored here; the §7.5
    //    diagnostic log, if wanted, is the §1.8/C4 caller's (P3.34+), not this leaf's. §2.7.2 records this.
    let probe_path = dir.join(probe_name);
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe_path)
    {
        Ok(file) => {
            drop(file);
            let _ = std::fs::remove_file(&probe_path);
        }
        Err(_) => return LocationStatus::Divert(DivertReason::Unwritable),
    }
    // 3. FAT/exFAT-class no-atomic-publish (Unix-only; a Windows no-op). A `statfs` error ⇒ treat as NOT
    //    FAT-class (the reactive §2.1.2 backstop covers a missed one — the P3.18 list-miss honesty).
    if crate::platform::lacks_atomic_publish_primitive(dir).unwrap_or(false) {
        return LocationStatus::Divert(DivertReason::NoAtomicPublish);
    }
    LocationStatus::Writable
}

/// A per-directory memo of [`location_status`] verdicts within a run — the §2.7.2 "cache per-directory"
/// planning hint: a 10 000-file batch dropping into ONE folder probes it ONCE, not 10 000 times. Keyed by the
/// candidate output dir PATH (a hint, so two aliased paths to one dir get separate entries — harmless: the
/// real §2.1 publish P3.36 re-checks). Owned by the §1.8/C4 planning pass and threaded across it.
/// [Build-Session-Entscheidung: P3.33]
// [Test-Change: P3.49 — old-obsolete+new-correct, §1.8] the P3.33 `allow(dead_code)` on `LocationCache` + its
// `new`/`classify` (below) is removed: the P3.48 C6 conductor (`crate::orchestrator::run_conversion`) threads
// the cache per-item in production, so they are LIVE — the "dead until P3.34+" annotation is a P3.48-era miss
// corrected here (a production lint cleanup, not a test suppression).
#[derive(Debug, Default)]
pub struct LocationCache {
    seen: HashMap<PathBuf, LocationStatus>,
}

impl LocationCache {
    /// A fresh empty cache. [Build-Session-Entscheidung: P3.33]
    #[must_use]
    pub fn new() -> Self {
        Self {
            seen: HashMap::new(),
        }
    }

    /// Classify `dir` (§2.7.2), MEMOISED per-dir. On a cache HIT the stored verdict is returned and NO probe
    /// runs; on a MISS `probe_name()` supplies a fresh `crate::run`-grammar probe name — a `FnOnce`, so it is
    /// built ONLY on a real probe, which keeps `fs_guard` a LEAF (the caller wires it to
    /// `crate::run::PublishTemp::probe_name`) — [`location_status`] classifies, and the verdict is cached.
    /// [Build-Session-Entscheidung: P3.33]
    pub fn classify(
        &mut self,
        dir: &Path,
        probe_name: impl FnOnce() -> OsString,
    ) -> LocationStatus {
        if let Some(&status) = self.seen.get(dir) {
            return status;
        }
        let status = location_status(dir, &probe_name());
        self.seen.insert(dir.to_path_buf(), status);
        status
    }
}

/// The §2.7.1 resolved destination MODE for one source's output directory — the leaf-local, path-resolved form
/// the §1.8/C4 planning (P3.37) hands [`prepare_output_dir`] after combining the user's wire
/// [`crate::domain::DestinationChoice`] with the §2.4 freeze-derived common root. `fs_guard` is a §0.7 tier-2
/// LEAF: the wire `DestinationChoice` carries only the chosen root (NO common root — that is a freeze-derived
/// value, §2.7.1), so the subtree re-creation needs BOTH, and this leaf-local mode carries them borrowed. NOT a
/// wire type: no `serde`/`specta`, plain borrowed `&Path`s (so it is `Copy`). [Build-Session-Entscheidung: P3.34]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "P3.34 — the §2.7.1 resolved destination-mode input to `prepare_output_dir`; constructed only \
                  by the §1.8/C4 destination planning (P3.37, which combines the wire DestinationChoice with the \
                  §2.4 freeze common root), so it is dead-at-runtime during the P3 wiring window; `allow` \
                  (permissive) covers the ambiguous dead-ness (cf. OutputSafety). Exercised by prepare_output_dir_tests."
    )
)]
#[derive(Debug, Clone, Copy)]
pub enum DestinationMode<'a> {
    /// Beside source (the §2.7.1 default): the output goes in the source's OWN parent directory, so a
    /// folder-drop's layout is preserved for free (each output sits next to its origin) with no path computation
    /// beyond the source's parent — and NO directory is created (the parent already exists).
    BesideSource,
    /// User-chosen root (§2.7.1): re-create the dropped-root-relative subtree under `root`. For a `source` at
    /// `common_root/sub/dir/file.ext`, the output directory is `root/sub/dir` — the relative subtree is re-created
    /// (never flattened).
    ChosenRoot {
        /// The single user-chosen destination root `D` — assumed to EXIST (the picker returns an existing
        /// folder); only the subtree ancestors UNDER it are created (§2.7.1).
        root: &'a Path,
        /// The §2.4 freeze-derived common root (the deepest directory containing all frozen sources) the source's
        /// relative subtree is taken against. `source` and `common_root` MUST be in the SAME path representation
        /// (the caller passes the user-facing dropped-root pair so the re-created layout matches what the user
        /// dropped, §2.7.1).
        common_root: &'a Path,
    },
}

/// §2.7.1 create-only ancestor step: create ONE subtree directory `dir`, tolerating a pre-existing DIRECTORY but
/// never a non-directory. `create_dir` (NOT `create_dir_all`) is create-only — it does not silently accept an
/// existing NON-directory occupying the name. On `AlreadyExists` the entry is re-checked: `metadata`
/// (symlink-FOLLOWING) `is_dir()` ⇒ continue (a real dir OR a symlink-to-dir — the symlink-ancestor-into-a-
/// source-tree redirect is caught by the full-final-dir §2.3.3 link-safety at publish, P3.9 on the deepest dir,
/// §2.7.1); otherwise a clear `NotADirectory` failure (a regular file / symlink-to-file occupies the name — the
/// item fails per §2.8, batch continues, never a silent overwrite/divert-around). A dangling symlink surfaces as
/// the `?` metadata `Err`. Any other create error fails the item. Panic-free (G4/G14): every FS op maps to
/// `io::Result`. [Build-Session-Entscheidung: P3.34]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "P3.34 — the create-only ancestor helper, called only by `prepare_output_dir` (itself unwired \
                  until the §1.8/C4 planning, P3.37); dead-at-runtime through the P3 wiring window, exercised by \
                  prepare_output_dir_tests. `allow` (permissive) covers the transitive dead-ness."
    )
)]
fn create_subtree_dir(dir: &Path) -> io::Result<()> {
    match std::fs::create_dir(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            // §2.7.1: an existing ancestor must be a DIRECTORY. `metadata` follows a symlink, so a
            // symlink-to-FILE / regular file → `is_dir() == false` → the clear NotADirectory failure; a real dir /
            // symlink-to-dir → continue. A dangling symlink surfaces as the `?` metadata `Err` (a clear per-item
            // failure), never a silent success.
            if std::fs::metadata(dir)?.is_dir() {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    "§2.7.1: a non-directory already occupies an output-subtree ancestor path",
                ))
            }
        }
        Err(other) => Err(other),
    }
}

/// The fallible outcome of [`prepare_output_dir`] (P3.34, extended at P3.88) — either a §2.7.1 directory-
/// preparation IO failure (a non-directory ancestor collision / a `..`-traversal / a create error / a source
/// outside the common root — all `Io`, the pre-P3.88 sole outcome), or the §2.2.4 Windows-unopenable-name reject
/// on a RE-CREATED subtree directory (`UnopenableName`, carrying the offending token). `fs_guard` is a §0.7
/// tier-2 LEAF, so this is its OWN error (never a §1.8 [`OutputPlanError`] / §2.8 `ConversionErrorKind` — the
/// §1.8 [`compute_output_plan`] maps it). [Build-Session-Entscheidung: P3.88]
#[derive(Debug)]
pub enum PrepareOutputDirError {
    /// A §2.7.1 IO failure (the pre-P3.88 sole outcome) — the caller maps it to a `WriteFailed`/`Io`-class §2.8 kind.
    Io(io::Error),
    /// §2.2.4 (Windows): a CONSTRUCTED subtree directory is a name Windows cannot open (a reserved DOS device name
    /// in its first dot-segment, or a trailing dot/space) — carries the offending token for the §2.8
    /// `UnopenableOutputName` message. Windows-only (the classifier is a const-`Ok` off Windows).
    UnopenableName(String),
}

impl From<io::Error> for PrepareOutputDirError {
    /// Lets the §2.7.1 body keep its `?` on `io::Result` steps — every io failure is the `Io` class.
    fn from(e: io::Error) -> Self {
        PrepareOutputDirError::Io(e)
    }
}

/// **§2.7.1 destination-mode output-directory preparation (P3.34)** — resolve (and, for the chosen-root case,
/// create-only re-create) the final output DIRECTORY for one `source` under `mode`, returning the directory the
/// §2.1 exclusive publish will target. This is DIRECTORY-only (never a `final_path` — the exact leaf name + §2.2
/// no-clobber numbering is resolved at write time on the resolved real file, §2.1.2 / P3.15): the §1.8
/// `OutputPlan` (P3.37) stores this as `final_dir`, and the §2.1.1 write sequence (P3.38) opens + link-safety-
/// verifies it via [`open_verified_parent_dir`] (P3.9) BEFORE the leaf publish (the §2.7.1 / §2.3.3 ordering —
/// the full-final-dir link-safety on the DEEPEST dir, taken at publish, not here).
///
/// - **[`DestinationMode::BesideSource`]** (default, §2.7.1): the output directory is the source's own parent —
///   the folder layout is preserved for free and NO directory is created (the parent already exists). `Err`
///   (InvalidInput) only if `source` has no parent (a root path — a frozen source file never is).
/// - **[`DestinationMode::ChosenRoot`]** (§2.7.1): re-create the dropped-root-relative subtree under `root`. The
///   source's path relative to `common_root` (`sub/dir/file.ext`) has its directory part (`sub/dir`) re-created
///   under `root` ancestor-by-ancestor (shallowest first), each step create-only via [`create_subtree_dir`] (a
///   pre-existing directory is tolerated; a non-directory collision fails clearly). The returned directory is
///   `root/sub/dir` (never flattened). A source directly under `common_root` yields `root` itself (no ancestor to
///   create). `Err(Io/InvalidInput)` if `source` is not under `common_root`, or a non-normal component (a `..` /
///   absolute anomaly) appears in the relative subpath — never re-created through (the no-harm / anti-traversal
///   default: a dropped-root-relative subtree can never escape the chosen root). **§2.2.4 (P3.88):** each
///   CONSTRUCTED subtree directory is rejected BEFORE creation if Windows cannot open it (a reserved DOS device
///   name / trailing dot-space) → `Err(`[`PrepareOutputDirError::UnopenableName`]`)` naming the token; the
///   user-chosen `root`/`common_root` ancestors are NEVER checked (only re-created names).
///
/// **Fallible ([`PrepareOutputDirError`]), never a panic** (G4/G14 — this runs on untrusted paths outside the
/// §2.12 boundary): a strip/create failure (`Io`) or a §2.2.4 unopenable-name reject (`UnopenableName`) is a
/// clean `Err` the §2.8 caller maps to a per-item failure (batch continues, §1.9), NEVER a partial silently-wrong
/// tree. `fs_guard` is a §0.7 tier-2 LEAF — it returns its OWN error, never a `ConversionErrorKind` (the §1.8
/// [`compute_output_plan`] maps it). [Build-Session-Entscheidung: P3.34, P3.88]
pub fn prepare_output_dir(
    source: &Path,
    mode: DestinationMode,
) -> Result<PathBuf, PrepareOutputDirError> {
    match mode {
        DestinationMode::BesideSource => source.parent().map(Path::to_path_buf).ok_or_else(|| {
            PrepareOutputDirError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "§2.7.1: beside-source output needs the source's parent directory, but source has none (a root path)",
            ))
        }),
        DestinationMode::ChosenRoot { root, common_root } => {
            // The source's path relative to the freeze common root — `sub/dir/file.ext` (§2.7.1). `strip_prefix`
            // fails only if `source` is not under `common_root` (a caller contract violation — the common root
            // contains all frozen sources by construction, §2.4); fail the item clearly, never re-create a wrong
            // tree.
            let rel = source.strip_prefix(common_root).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "§2.7.1: source is not under the frozen common root — cannot re-create its relative subtree",
                )
            })?;
            // The DIRECTORY part of the relative path (`sub/dir`); a source directly under `common_root` has an
            // empty relative directory → the final directory is `root` itself (no ancestor to create).
            let rel_dir = rel.parent().unwrap_or_else(|| Path::new(""));
            let mut final_dir = root.to_path_buf();
            for component in rel_dir.components() {
                // Enumerate every `Component` variant (the crate root denies clippy::wildcard_enum_match_arm):
                // only a NORMAL name extends the subtree; any Prefix/RootDir/CurDir/ParentDir in a supposedly
                // dropped-root-relative subpath is an absolute / `..` traversal anomaly — fail clearly, never
                // create through it (§2.7.1 / the no-harm default).
                match component {
                    std::path::Component::Normal(name) => {
                        // §2.2.4 (Windows): reject a CONSTRUCTED subtree directory Windows cannot open (a reserved
                        // DOS device name / a trailing dot-space) BEFORE creating it — fail clearly NAMING the
                        // token, never create an aliased directory (§2.2.1). ONLY these re-created names are
                        // checked; the user-chosen `root` + `common_root` ancestors are NEVER (§2.2.4). A const-`Ok`
                        // no-op off Windows.
                        if reject_unopenable_windows_name(name).is_err() {
                            return Err(PrepareOutputDirError::UnopenableName(
                                name.to_string_lossy().into_owned(),
                            ));
                        }
                        final_dir.push(name);
                        // Create-only, ancestor-by-ancestor (shallowest first): each missing ancestor is made
                        // create-only; a pre-existing directory is tolerated; a non-directory collision fails the
                        // item (§2.7.1).
                        create_subtree_dir(&final_dir)?;
                    }
                    std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
                    | std::path::Component::CurDir
                    | std::path::Component::ParentDir => {
                        return Err(PrepareOutputDirError::Io(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "§2.7.1: unexpected non-normal component in the dropped-root-relative subtree path",
                        )));
                    }
                }
            }
            Ok(final_dir)
        }
    }
}

/// The §2.7.3 divert-target resolution verdict ([`resolve_divert_target`], P3.35): either the writable divert
/// ROOT the item's output diverts to, or the signal that NO candidate is usable. `fs_guard` is a §0.7 tier-2
/// LEAF — it returns its own verdict, never a `ConversionErrorKind` (the caller maps [`Self::Unavailable`] to
/// the §2.8 `WriteFailed`). [Build-Session-Entscheidung: P3.35]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "P3.35 — resolve_divert_target's verdict type; constructed only by that fn (itself unwired \
                  until the late-divert P3.36 / the §1.8 OutputPlan divert leg P3.37), so it is dead-at-runtime \
                  through the P3 wiring window; `allow` (permissive) covers the ambiguous dead-ness (cf. \
                  OutputSafety). Exercised by resolve_divert_target_tests."
    )
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DivertTarget {
    /// A candidate divert root passed the §2.7.2 re-test (`Writable` — not ephemeral / unwritable / FAT-exFAT):
    /// the item's output diverts here (§2.7.3). The §2.7.5 full-chain re-checks (`is_safe_output` / path-limit /
    /// free-space) on the diverted FINAL path run at write time (P3.36 late-divert / P3.38), not here.
    Resolved(PathBuf),
    /// NO candidate divert root is usable — every one is ephemeral / unwritable / atomic-publish-incapable. The
    /// item fails clearly with `WriteFailed` (§2.8), never diverting an output onto a purgeable / another-FAT
    /// volume (§2.7.3).
    Unavailable,
}

/// **§2.7.3 divert-target resolution (P3.35)** — pick the divert ROOT for an unwritable / ephemeral /
/// `NoAtomicPublish` source's output by re-testing an ordered list of candidate roots. The FIRST candidate the
/// §2.7.2 [`location_status`] classifies `Writable` (not ephemeral / unwritable / FAT-exFAT — the P3.18 test
/// folded in) is the divert root; if NONE qualifies, [`DivertTarget::Unavailable`] (→ §2.8 `WriteFailed`, never
/// a divert onto a purgeable / another-FAT volume, §2.7.3).
///
/// `candidates` is passed IN by the AppHandle-side caller (the C4 boundary, P3.36/P3.37), which resolves the
/// §2.7.3 roots via Tauri's `PathResolver` (`download_dir()` / `document_dir()`) plus any §2.7.1 user-chosen
/// override — this §0.7 tier-2 LEAF cannot reach `PathResolver`, exactly as it takes the `crate::run`
/// probe-name grammar in (P3.33) and the freeze common root in (P3.34). **This box is AGNOSTIC to how the list
/// is BUILT:** the exact §2.7.3 candidate PRIORITY + semantics (does a user-chosen root *replace* the
/// Downloads/Documents fallback or merely lead it; is "falling back to Documents if Downloads is *absent*"
/// keyed on absence vs unwritability) is the caller's construction decision, made against §2.7.3 when
/// P3.36/P3.37 build the list — `resolve_divert_target` walks whatever ordered list it is given and returns the
/// first `Writable`, so a single-element list yields the strict "one target, else WriteFailed" behavior and a
/// multi-element list cascades, per the list the caller supplies. [Build-Session-Entscheidung: P3.35]
///
/// The re-test reuses the run's [`LocationCache`] (`&mut`), so a divert root already probed this run (e.g.
/// Downloads probed for an earlier diverted item in the same batch) is classified ONCE — the §2.7.2 "cache
/// per-directory" hint applied to divert roots too. `probe_name` supplies a FRESH `crate::run`-grammar probe
/// name per real probe (a `Fn`, invoked once per candidate that misses the cache), keeping this a LEAF. The
/// §2.7.5 full-chain re-checks on the diverted FINAL path (link-safety / path-limit / free-space) are the
/// late-divert (P3.36) / write-sequence (P3.38) concern — this box resolves the divert ROOT only.
///
/// **Infallible** — every candidate failure maps to a verdict, never an `Err`; panic-free (the crate no-panic
/// deny, G4/G14). [Build-Session-Entscheidung: P3.35]
#[must_use = "the DivertTarget verdict decides WHERE (or whether) the output diverts — dropping it would \
              divert blindly or skip the §2.8 WriteFailed on an unusable target"]
pub fn resolve_divert_target(
    candidates: &[PathBuf],
    cache: &mut LocationCache,
    probe_name: impl Fn() -> OsString,
) -> DivertTarget {
    for candidate in candidates {
        // §2.7.2 re-test each candidate divert root in order; the FIRST Writable wins (the §2.7.3 user-chosen →
        // Downloads → Documents priority is the caller's ordering). A `Divert(_)` verdict (ephemeral /
        // unwritable / FAT-exFAT) skips this candidate — never divert onto a purgeable / another-FAT volume
        // (§2.7.3). `&probe_name` (a `&impl Fn` is a valid `FnOnce`) builds a fresh `crate::run`-grammar probe
        // name only on a real (cache-miss) probe — `LocationCache::classify` wants a `FnOnce` — keeping this a LEAF.
        if let LocationStatus::Writable = cache.classify(candidate, &probe_name) {
            return DivertTarget::Resolved(candidate.clone());
        }
    }
    // §2.7.3: no candidate is usable → the item fails clearly (WriteFailed, §2.8), never a silent bad divert.
    DivertTarget::Unavailable
}

/// **§2.7.2 late-divert trigger classification (P3.36)** — is this failed §2.1 publish a WRITABILITY failure
/// (the destination became unwritable *after* the cached §2.7.2 probe — USB pulled / share dropped / permission
/// flip), which the late-divert rescues by re-publishing to the §2.7.3 divert target? A NON-writability error
/// is **not** a divert trigger (it fails per §2.8): [`PublishError::OutOfDisk`] (§2.7.2 names it explicitly),
/// [`PublishError::PathTooLong`] and [`PublishError::TooManyCollisions`] are structural, and an
/// [`PublishError::Io`] that is not a writability kind is a genuine write failure surfaced unchanged. The
/// writability kinds are §2.7.2's ("`PermissionDenied`, `ReadOnlyFilesystem`, network errors") plus the
/// device-gone `NotFound` (a pulled USB / removed share leaves the pinned parent handle's directory absent).
///
/// **Conservative by design, and SAFE in both directions (no-harm holds regardless):** a writability kind this
/// set MISSES merely fails the item `WriteFailed` instead of rescuing it (a missed rescue, never data loss);
/// a kind it OVER-includes at most attempts a divert that itself fails → `WriteFailed` anyway. So the set errs
/// toward the documented common cases (§2.7.2:writable-probe classification) without risking a wrong outcome.
/// [Build-Session-Entscheidung: P3.36]
// [Test-Change: P3.48 — old-obsolete+new-correct, §2.7.2] LIVE from P3.48: the conductor's §2.1.1
// `publish_completed` calls `is_write_divert_trigger` on a primary-publish writability failure (the late-divert
// decision). The P3.38 `write_item` that formerly called it was re-cut into `publish_written_temp`; `allow` is
// retained gate-safely (the P3.7/P3.8 precedent).
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.7.2 is_write_divert_trigger (P3.36) — the late-divert trigger classifier; LIVE from P3.48 \
                  (the conductor's §2.1.1 `publish_completed` classifies a writability publish failure). `allow` \
                  is retained gate-safely (the P3.7/P3.8 precedent). Exercised by the late_divert_tests."
    )
)]
pub fn is_write_divert_trigger(err: &PublishError) -> bool {
    match err {
        // §2.7.2: "a non-writability error — e.g. OutOfDisk — is not a divert trigger." PathTooLong /
        // TooManyCollisions / UnopenableName are likewise structural (a shorter/less-degenerate/differently-named
        // candidate, not a writability problem) — a §2.7.3 divert to another volume would carry the SAME
        // unopenable name and fail identically (§2.2.4: "the divert path is re-checked identically"). Never divert.
        PublishError::OutOfDisk
        | PublishError::PathTooLong(_)
        | PublishError::TooManyCollisions
        | PublishError::UnopenableName(_) => false,
        // A genuine OS write error: a late-divert trigger IFF its kind says the DESTINATION became unwritable.
        // `io::ErrorKind` is `#[non_exhaustive]`, so `matches!` (whitelist → `false` for anything else) is the
        // exhaustiveness-safe form (no wildcard arm; a future kind defaults to NOT-a-trigger, the safe direction).
        PublishError::Io(e) => matches!(
            e.kind(),
            io::ErrorKind::PermissionDenied            // permission flip (EACCES / EPERM / an ACL change)
                | io::ErrorKind::ReadOnlyFilesystem    // the mount / share flipped read-only (EROFS)
                | io::ErrorKind::NotFound              // the destination dir / device vanished (USB pulled)
                | io::ErrorKind::NetworkUnreachable    // a network share dropped …
                | io::ErrorKind::HostUnreachable
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
        ),
    }
}

/// §2.14.4 free-space re-check on the DIVERT target's volume (P3.36). The up-front §1.10 preflight verified
/// free space on the ORIGINAL beside-source volume; a late-divert lands the output on a DIFFERENT volume
/// (Downloads/Documents), so its free space is re-checked here against the output size (≈ the completed `tmp`'s
/// byte length) BEFORE the publish — "never assume it fits because the original volume did" (§2.7.2). This runs
/// on BOTH the same- and cross-volume divert (the §2.14.3 cross-volume path inside [`atomic_publish`] has its
/// own pre-copy check, but a same-volume divert would otherwise skip §2.14.4 entirely). Below the need →
/// [`PublishError::OutOfDisk`] (→ §2.8 `OutOfDisk`; the one item fails, the batch continues, §1.9).
/// [Build-Session-Entscheidung: P3.36]
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn recheck_divert_free_space(divert_dir: &Path, needed_bytes: u64) -> Result<(), PublishError> {
    let avail = crate::platform::available_bytes(divert_dir).map_err(PublishError::Io)?;
    if avail < needed_bytes {
        Err(PublishError::OutOfDisk)
    } else {
        Ok(())
    }
}

/// **§2.7.2/§2.7.5 the late-divert publish (P3.36)** — re-run the FULL safety chain on the §2.7.3 divert target
/// `divert_dir` and publish the completed `tmp` there, after the primary beside-source §2.1 publish failed for
/// a writability reason ([`is_write_divert_trigger`]). "Not a degraded path" (§2.7.5 / SSOT Principle-5): every
/// guarantee the beside-source publish runs, runs here too — because the up-front §2.2.3 path-limit and §2.14.4
/// free-space verdicts were computed for the ORIGINAL volume + path, which differ from the divert's:
///
/// 1. **§2.3.3 link-safety** — [`open_verified_parent_dir`] opens `divert_dir` as a TOCTOU-closed pinned
///    handle and verifies it does not resolve onto a frozen source (a `ResolvesOntoSource` divert target fails
///    the item — the divert never publishes onto an original).
/// 2. **§2.14.4 free-space** — [`recheck_divert_free_space`] re-checks the divert VOLUME against the output size
///    (≈ `tmp`'s bytes); a shortfall fails `OutOfDisk`.
/// 3. **§2.2.3 path-limit + §2.1 publish** — [`atomic_publish`] runs the §2.2.2 numbering loop (which re-checks
///    the per-OS path limit against the divert's full absolute path per candidate) + the create-only exclusive
///    publish (with the §2.14.3 cross-volume fallback — a late-divert `tmp` on the ORIGINAL volume publishes
///    cross-volume onto the divert), returning [`PublishOutcome`].
///
/// The divert target was already §2.7.2-re-tested writable by [`resolve_divert_target`] (P3.35) before this
/// call; this re-runs the WRITE-time chain the planning-hint probe cannot stand in for. Any leg failing fails
/// the ONE item clearly (§2.8 `WriteFailed`/`OutOfDisk`/`PathTooLong`) and the batch continues (§1.9); the
/// caller (P3.38) does NOT re-divert a failed divert (one divert per item, §2.7.3). No panic (G4/G14).
/// `same_volume_intermediate` is the run-owned §2.14.3 `.part` sibling of the divert final on the DIVERT volume
/// (passed in — the LEAF does not mint the `crate::run` name). [Build-Session-Entscheidung: P3.36]
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
// [Test-Change: P3.48 — old-obsolete+new-correct, §2.7.5] LIVE from P3.48: the conductor's §2.1.1
// `divert_completed` (← `publish_completed`) calls `publish_to_divert` after a writability publish failure /
// FAT-exFAT NoAtomicPublishSupport. The P3.38 `write_item` that formerly called it was re-cut into
// `publish_written_temp`; `allow` is retained gate-safely (the P3.7/P3.8 precedent).
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.7.2/§2.7.5 publish_to_divert (P3.36) — the late-divert publish; LIVE from P3.48 (the \
                  conductor's §2.1.1 `divert_completed` re-publishes to the §2.7.3 divert target). `allow` is \
                  retained gate-safely (the P3.7/P3.8 precedent). Exercised by the late_divert_tests."
    )
)]
pub fn publish_to_divert(
    divert_dir: &Path,
    frozen_sources: &[FileIdentity],
    source: &Path,
    ext: &str,
    tmp: &Path,
    same_volume_intermediate: &Path,
) -> Result<PublishOutcome, PublishError> {
    // 1. §2.3.3 link-safety on the divert dir — a TOCTOU-closed pinned handle whose resolved identity is not a
    //    frozen source (§2.7.5's identical link-safety on the divert path — the divert never publishes onto an
    //    original). A `ResolvesOntoSource` divert target fails the item (→ §2.8 WriteFailed).
    let verified =
        match open_verified_parent_dir(divert_dir, frozen_sources).map_err(PublishError::Io)? {
            ParentDirVerdict::Verified(v) => v,
            ParentDirVerdict::ResolvesOntoSource => {
                return Err(PublishError::Io(io::Error::other(
                "§2.7.5: the divert target resolves onto a frozen source — cannot publish there",
            )));
            }
        };
    // 2. §2.14.4 free-space re-check against the DIVERT volume (the output ≈ the completed tmp's byte length).
    let output_size = std::fs::metadata(tmp).map_err(PublishError::Io)?.len();
    recheck_divert_free_space(divert_dir, output_size)?;
    // 3. §2.2 candidates + §2.1 publish — the numbering loop re-checks the per-OS path limit against the divert's
    //    full absolute path per candidate (P3.15), and atomic_publish handles the §2.14.3 cross-volume copy.
    let candidates = output_name(source, ext).ok_or_else(|| {
        PublishError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "§2.2.1: the source has no file stem — cannot name the divert output",
        ))
    })?;
    atomic_publish(
        &verified,
        divert_dir,
        tmp,
        candidates,
        same_volume_intermediate,
    )
}

/// The §1.8 [`compute_output_plan`] failure verdict (P3.37) — a `crate::fs_guard` §0.7 tier-2 LEAF error the
/// §1.8/C4 caller (P3.49) maps to §2.8. `crate::fs_guard` never depends up on `crate::outcome`, so it returns
/// this, never a `ConversionErrorKind`. [Build-Session-Entscheidung: P3.37]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "P3.37 — compute_output_plan's failure verdict; constructed only by that fn (itself unwired \
                  until the §1.8/C4 plan_output body P3.49 / the §2.1.1 write sequence P3.38), so it is \
                  dead-at-runtime through the P3 wiring window; `allow` (permissive) covers the ambiguous \
                  dead-ness (cf. OutputSafety). Exercised by compute_output_plan_tests."
    )
)]
#[derive(Debug)]
pub enum OutputPlanError {
    /// A §2.7.1 directory-preparation failure from [`prepare_output_dir`] (a non-directory collision /
    /// `..`-traversal / a create error — the §2.3.3 link-safety on the deepest dir runs at the P3.9 publish, NOT
    /// here, so a symlink-to-dir ancestor is tolerated at this stage) — the item fails per §2.8 (a
    /// `WriteFailed`/`Io`-class kind).
    Io(io::Error),
    /// §2.7.3: no usable divert target — the diverted location's resolved §2.7.3 root is itself
    /// ephemeral / unwritable / FAT-exFAT ([`resolve_divert_target`] → [`DivertTarget::Unavailable`]). The item
    /// fails clearly `WriteFailed` (§2.8), never a divert onto a purgeable / another-FAT volume.
    DivertUnavailable,
    /// §2.2.4 (Windows): a CONSTRUCTED chosen-root subtree directory is a name Windows cannot open (a reserved
    /// DOS device name / a trailing dot-space) — [`prepare_output_dir`] refused it BEFORE creation. Carries the
    /// offending token so the §2.8 `UnopenableOutputName` message NAMES it (§2.2.4). The subtree analog of the
    /// leaf-side [`PublishError::UnopenableName`] (§2.1.1 publish). Windows-only. [Build-Session-Entscheidung: P3.88]
    UnopenableName(String),
}

/// **§1.8 the per-job OutputPlan computation (P3.37)** — resolve one job's output DIRECTORY (beside-source /
/// chosen-root subtree / §2.7.3 divert) and assemble the directory-based [`OutputPlan`] the §2.1.1 write
/// sequence (P3.38) consumes, BEFORE any write. **Directory-based by design:** there is NO pre-baked
/// `final_path` — the exact leaf name + §2.2 no-clobber numbering is resolved LAZILY at write time on the
/// resolved real file (§2.1.2 / P3.15; a pre-numbered path would reintroduce the §2.1.2 TOCTOU race) — and no
/// `crosses_volume` field (EXDEV is detected reactively at publish, §2.14.3).
///
/// `location` is this destination's §2.7.2 classification (computed at C4 via `location_status`, P3.33, and
/// passed IN — this LEAF does not re-probe the primary location):
/// - **[`LocationStatus::Writable`]** → `final_dir` = [`prepare_output_dir`]`(source, mode)` (the §2.7.1
///   beside-source parent OR the chosen-root subtree re-creation, create-only), `diverted = None`.
/// - **[`LocationStatus::Divert`]`(reason)`** → the output diverts FLAT into the §2.7.3 divert root (§2.7.4 — no
///   subtree re-creation on the divert path). `divert_root` is the caller-resolved §2.7.3 target (the §2.7.1
///   user-chosen destination override, else Downloads, else Documents — resolved AppHandle-side via Tauri's
///   `PathResolver` and passed IN, exactly as P3.35 takes candidates in and P3.34 the common root). A
///   single-candidate [`resolve_divert_target`] re-tests it (§2.7.2 — the strict §2.7.3 "one resolved target,
///   else WriteFailed" reading): `Resolved(dir)` → `final_dir = dir`, `diverted = Some(reason)`; `Unavailable`
///   → [`OutputPlanError::DivertUnavailable`].
///
/// `base_name` is the source's verbatim `file_stem` (§2.2 / §2.10.1 — OS-native bytes preserved; a stemless
/// source fails clearly, never a panic); `extension` is the chosen target's bare canonical extension;
/// `publish_temp_dir` EQUALS `final_dir` (v1, §2.14.1 — the `*.part` is a uniquely-named sibling dotfile inside
/// `final_dir`, on the same volume by construction, so the §2.1 publish is a true intra-volume atomic rename).
/// No panic (G4/G14). [Build-Session-Entscheidung: P3.37]
// [Build-Session-Entscheidung: P3.37] `#[allow(clippy::too_many_arguments)]`: the eight inputs are each a
// DISTINCT, documented planning input — the per-job WHAT (job / source / extension / mode / location /
// divert_root) plus the shared run CONTEXT (the `LocationCache` + the `crate::run` probe-name factory the LEAF
// takes in). A mechanical bundle struct would group them without semantic value (and re-introduce the borrow
// lifetimes the borrowed args already carry), so the explicit signature is the clearer surface here.
#[allow(clippy::too_many_arguments)]
// `expect`→`allow`: the P3.48 C6 conductor (`crate::orchestrator::convert_item`) is now a LIVE production
// caller of `compute_output_plan` (per item, before the pick-temp → dispatch → publish legs), so the P3.37
// `expect(dead_code)` flips to "unfulfilled". The §1.8/C4 `plan_output` body (P3.49) named in the old reason
// remains a FUTURE additional caller; `expect`→`allow` IN PLACE (a removed multi-line `expect(` is untaggable
// in G70's ±6 window, the P3.7/P3.8 precedent). [Test-Change: P3.48 — old-obsolete+new-correct, §1.8]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§1.8 compute_output_plan (P3.37) — the per-job OutputPlan computation; LIVE from P3.48 (the \
                  C6 conductor `convert_item` computes the plan per item). The §1.8/C4 plan_output body (P3.49) \
                  is a future additional caller. `allow` (not removal) keeps the expect→allow diff G70-safe \
                  (the P3.7/P3.8 precedent); the compute_output_plan_tests still exercise it."
    )
)]
pub fn compute_output_plan(
    job: ItemId,
    source: &Path,
    extension: &str,
    mode: DestinationMode,
    location: LocationStatus,
    divert_root: &Path,
    cache: &mut LocationCache,
    probe_name: impl Fn() -> OsString,
) -> Result<OutputPlan, OutputPlanError> {
    // §2.2 / §2.10.1: the SOURCE base name, kept verbatim (OS-native bytes). A frozen source always has a stem;
    // a stemless path (`.` / `..` / a root) fails clearly, never a panic (G4/G14).
    let base_name = source
        .file_stem()
        .ok_or_else(|| {
            OutputPlanError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "§2.2.1: the source has no file stem — cannot name the output",
            ))
        })?
        .to_os_string();

    let (final_dir, diverted) = match location {
        // §2.7.1: the location accepts a write → beside-source parent OR the chosen-root subtree (create-only).
        LocationStatus::Writable => (
            // Map the §0.7 tier-2 leaf error onto the §1.8 taxonomy: a §2.7.1 IO failure → `Io`; a §2.2.4
            // unopenable re-created subtree dir → `UnopenableName` (carrying the token) → §2.8 `UnopenableOutputName`.
            prepare_output_dir(source, mode).map_err(|e| match e {
                PrepareOutputDirError::Io(io) => OutputPlanError::Io(io),
                PrepareOutputDirError::UnopenableName(token) => {
                    OutputPlanError::UnopenableName(token)
                }
            })?,
            None,
        ),
        // §2.7.3: the location diverts → the output goes FLAT into the caller-resolved §2.7.3 divert root
        // (§2.7.4, no subtree re-creation). A single-candidate resolve_divert_target re-tests it (§2.7.2 — the
        // strict "one resolved target else WriteFailed" reading); Unavailable → the item fails clearly.
        LocationStatus::Divert(reason) => {
            let candidates = [divert_root.to_path_buf()];
            match resolve_divert_target(&candidates, cache, probe_name) {
                DivertTarget::Resolved(dir) => (dir, Some(reason)),
                DivertTarget::Unavailable => return Err(OutputPlanError::DivertUnavailable),
            }
        }
    };

    // §2.14.1: the kind-1 publish temp lives in `final_dir` (v1) — a same-volume sibling dotfile, so the §2.1
    // publish is a true intra-volume atomic rename. (The kind-2 engine scratch, §2.14.2, may be elsewhere and is
    // NOT carried here.)
    let publish_temp_dir = final_dir.clone();
    Ok(OutputPlan {
        job,
        final_dir,
        diverted,
        base_name,
        extension: OsString::from(extension),
        publish_temp_dir,
    })
}

#[cfg(test)]
mod location_status_tests {
    use super::*;

    // A real temp dir under the crate source root (`CARGO_MANIFEST_DIR`) — a NON-ephemeral writable base, so
    // the §2.7.2 writable / unwritable / FAT legs are reachable. A plain `tempfile::tempdir()` lives under the
    // OS temp root, which `location_status` classifies `Ephemeral` FIRST (short-circuiting those legs), so the
    // non-ephemeral legs need a non-temp base. `None` on the pathological env where the crate root is itself
    // under an OS temp root (a clean skip, never a false pass). Real FS — never mocked (test-strategy §0.1).
    fn non_ephemeral_tempdir() -> Option<tempfile::TempDir> {
        let dir = tempfile::Builder::new()
            .prefix("convertia-locstat-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .expect("create a temp dir in the crate source root");
        (!crate::platform::is_ephemeral_output_dir(dir.path())).then_some(dir)
    }

    // §6.4.1 real-FS (G15) / §2.7.2: a writable, non-ephemeral, non-FAT dir → `Writable`, and the throwaway
    // writability probe is REMOVED (no residue in the destination).
    #[test]
    fn a_writable_non_ephemeral_dir_is_writable_and_leaves_no_probe() {
        let Some(dir) = non_ephemeral_tempdir() else {
            return;
        };
        let probe = OsStr::new(".convertia-locstat-probe-writable.part");
        assert_eq!(
            location_status(dir.path(), probe),
            LocationStatus::Writable,
            "§2.7.2: a writable, non-ephemeral, non-FAT dir accepts a create → Writable"
        );
        assert!(
            !dir.path().join(probe).exists(),
            "§2.7.2: the throwaway probe is removed on the writable path — no residue in the destination"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.2: a dir under the OS temp root → `Divert(Ephemeral)`, and NO probe is
    // written — ephemeral short-circuits BEFORE the writable probe, so a purgeable temp dir gets no residue.
    #[test]
    fn a_temp_dir_diverts_ephemeral_without_probing() {
        let dir = tempfile::tempdir().expect("a real temp dir under the OS temp root");
        let probe = OsStr::new(".convertia-locstat-probe-ephemeral.part");
        assert_eq!(
            location_status(dir.path(), probe),
            LocationStatus::Divert(DivertReason::Ephemeral),
            "§2.7.2: a dir under the OS temp root diverts (Ephemeral) — a result there could be silently purged"
        );
        assert!(
            !dir.path().join(probe).exists(),
            "§2.7.2: ephemeral short-circuits BEFORE the writable probe — no probe residue in a temp dir"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.2: a read-only non-ephemeral dir does NOT accept a create → `Divert(Unwritable)`.
    // Mirrors the publish tests' read-only pattern: skip where the platform/FS won't enforce read-only (root, a
    // permissive FS), restoring writability so the TempDir cleanup succeeds.
    #[cfg(unix)]
    #[test]
    fn a_read_only_dir_diverts_unwritable() {
        use std::os::unix::fs::PermissionsExt;
        let Some(dir) = non_ephemeral_tempdir() else {
            return;
        };
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o500))
            .expect("make the dir read-only (r-x, no write)");
        // Skip where read-only is not enforced (e.g. running as root) — a create would still succeed.
        if std::fs::File::create(dir.path().join(".probe-check")).is_ok() {
            let _ = std::fs::remove_file(dir.path().join(".probe-check"));
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
                .expect("restore writability for cleanup");
            return;
        }
        let verdict = location_status(dir.path(), OsStr::new(".convertia-locstat-probe-ro.part"));
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
            .expect("restore writability so the TempDir cleanup succeeds");
        assert_eq!(
            verdict,
            LocationStatus::Divert(DivertReason::Unwritable),
            "§2.7.2: a dir that does not accept a create → Divert(Unwritable)"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.2: the per-dir cache MEMOISES — a second `classify` of the same dir returns
    // the stored verdict WITHOUT re-probing (the `FnOnce` probe-name factory runs exactly ONCE), so a
    // 10 000-file batch dropping into one folder probes it once. Both calls return the same verdict.
    #[test]
    fn classify_memoises_per_dir_and_probes_once() {
        use std::cell::Cell;
        let Some(dir) = non_ephemeral_tempdir() else {
            return;
        };
        let mut cache = LocationCache::new();
        let calls = Cell::new(0usize);
        let first = cache.classify(dir.path(), || {
            calls.set(calls.get() + 1);
            OsString::from(".convertia-loccache-probe-a.part")
        });
        let second = cache.classify(dir.path(), || {
            calls.set(calls.get() + 1);
            OsString::from(".convertia-loccache-probe-b.part")
        });
        assert_eq!(
            first, second,
            "§2.7.2: the cache returns the same verdict for the same dir"
        );
        assert_eq!(
            first,
            LocationStatus::Writable,
            "§2.7.2: a writable non-ephemeral dir classifies Writable"
        );
        assert_eq!(
            calls.get(),
            1,
            "§2.7.2: the second classify is a cache HIT — the probe-name factory ran exactly once (no re-probe)"
        );
    }
}

// §2.2.4 (P3.88) — the Windows-unopenable-name guard: the classifier + its leaf (publish) and subtree
// (prepare_output_dir) wiring. Windows-ONLY: off Windows the guard is a compile-time const-`Ok`, so there is
// nothing to reject and the whole class is `cfg(all(test, windows))`.
// Two STACKED cfg attrs (not `all(test, windows)`): clippy's expect-used/unwrap-used test allowance only
// recognises a STANDALONE `#[cfg(test)]` gate, so a compound `all(test, windows)` would wrongly deny the tests'
// fail-fast unwrapping. Stacking `#[cfg(test)]` + `#[cfg(windows)]` compiles to the same gate while keeping the allow.
#[cfg(test)]
#[cfg(windows)]
mod unopenable_windows_name_tests {
    use super::*;

    fn verified(dir: &Path) -> VerifiedParentDir {
        match open_verified_parent_dir(dir, &[]).expect("open the dest dir") {
            ParentDirVerdict::Verified(v) => Some(v),
            ParentDirVerdict::ResolvesOntoSource => None,
        }
        .expect("a real dir with an empty frozen set verifies")
    }

    // §2.2.4 (a): the first dot-segment (right-trimmed of trailing dots/spaces) is a reserved DOS device name —
    // bare, with any extension, case-insensitive, the numbered COM/LPT families, and even with a coincidental
    // trailing space (the reserved cause is the PRIMARY one, reported ahead of a trailing dot/space).
    #[test]
    fn reserved_device_in_first_dot_segment_is_rejected() {
        for token in [
            "CON",
            "con",
            "CON.tsv",
            "nul.csv",
            "AUX",
            "PRN.txt",
            "COM1",
            "com9.tsv",
            "LPT1",
            "LPT9.md",
            "CON ",
            "NUL.",
            "aux.csv.bak",
        ] {
            assert_eq!(
                reject_unopenable_windows_name(OsStr::new(token)),
                Err(UnopenableName::ReservedDevice),
                "§2.2.4: `{token}` aliases a reserved DOS device and must be rejected",
            );
        }
    }

    // §2.2.4 (b): the final character is a dot or a space — silently stripped by the Win32 path layer (an alias
    // onto a DIFFERENT name). The stem is NOT a reserved device (else it would be class (a)).
    #[test]
    fn trailing_dot_or_space_is_rejected() {
        for token in ["evil.txt.", "evil.txt ", "report.", "data.csv "] {
            assert_eq!(
                reject_unopenable_windows_name(OsStr::new(token)),
                Err(UnopenableName::TrailingDotOrSpace),
                "§2.2.4: `{token}` ends in a dot/space → an alias onto a different name, rejected",
            );
        }
    }

    // A reserved name aliases a device ONLY when it IS the first dot-segment — never as a substring, a longer
    // word, a subsequent dot-segment, or a non-existent `COM0`; a non-reserved trailing-free name is openable.
    #[test]
    fn ordinary_openable_names_are_accepted() {
        for token in [
            "CONSOLE.txt",
            "CONVERT.csv",
            "report.tsv",
            "NUL2.csv",
            "COM0.txt",
            "COMB.md",
            "data.con.csv",
            "my.CON.txt",
            "aCON.csv",
            "lpt.csv",
            "data.tsv",
        ] {
            assert_eq!(
                reject_unopenable_windows_name(OsStr::new(token)),
                Ok(()),
                "§2.2.4: `{token}` is an ordinary openable name, not a device alias",
            );
        }
    }

    // §2.1.1 LEAF wiring (P3.88): the publish seam surfaces a reserved CONSTRUCTED leaf as
    // `PublishError::UnopenableName` NAMING the token, BEFORE the exclusive create — so nothing is written.
    #[test]
    fn publish_rejects_a_reserved_leaf_naming_the_token() {
        let dir = tempfile::tempdir().expect("a real temp dir");
        let parent = verified(dir.path());
        let tmp = dir.path().join(".convertia-out.part");
        std::fs::write(&tmp, b"converted bytes").expect("write the tmp");
        // A source named `CON.csv` yields the constructed leaf `CON.tsv` — a reserved DOS device alias (§2.2.4).
        let candidates = output_name(Path::new("CON.csv"), "tsv").expect("a real path has a stem");
        let err = publish_numbered_capped(&parent, dir.path(), &tmp, candidates, 10_000)
            .expect_err(
                "§2.2.4: a constructed `CON.tsv` leaf must be rejected on Windows, never created",
            );
        assert!(
            matches!(&err, PublishError::UnopenableName(token) if token == "CON.tsv"),
            "§2.2.4: the publish reject NAMES the offending token (`CON.tsv`), got {err:?}",
        );
        assert!(
            !dir.path().join("CON.tsv").exists(),
            "§2.2.4: nothing was written — no aliased `CON.tsv` file exists in the destination",
        );
    }

    // §2.7.1 SUBTREE wiring (P3.88): the chosen-root subtree recreation rejects a CONSTRUCTED `CON` directory
    // BEFORE creating it → `PrepareOutputDirError::UnopenableName` naming the token, and no `CON` dir is left.
    // The source PATH need not exist — `prepare_output_dir` does pure path arithmetic (strip_prefix + the
    // component walk) and the reject fires BEFORE any create (we could not `create_dir("CON")` on Windows anyway
    // — that IS the hazard §2.2.4 guards).
    #[test]
    fn prepare_output_dir_rejects_a_reserved_subtree_component() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let common = base.path().join("drop");
        let dest = base.path().join("out");
        std::fs::create_dir(&common).expect("create the common root");
        std::fs::create_dir(&dest).expect("create the chosen destination root");
        let source = common.join("CON").join("file.csv");
        let err = prepare_output_dir(
            &source,
            DestinationMode::ChosenRoot {
                root: &dest,
                common_root: &common,
            },
        )
        .expect_err("§2.2.4: a re-created `CON` subtree directory must be rejected on Windows");
        assert!(
            matches!(&err, PrepareOutputDirError::UnopenableName(token) if token == "CON"),
            "§2.2.4: the subtree reject NAMES the offending token (`CON`), got {err:?}",
        );
        assert!(
            !dest.join("CON").exists(),
            "§2.2.4: the aliased `CON` directory was NEVER created under the chosen root",
        );
    }
}

#[cfg(test)]
mod prepare_output_dir_tests {
    use super::*;

    // §6.4.1 real-FS (G15) / §2.7.1: beside-source resolves to the source's OWN parent directory (folder layout
    // preserved for free) and creates NOTHING (the parent already exists).
    #[test]
    fn beside_source_returns_the_source_parent() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let sub = base.path().join("sub");
        std::fs::create_dir(&sub).expect("create the source's parent dir");
        let source = sub.join("file.csv");
        let final_dir = prepare_output_dir(&source, DestinationMode::BesideSource)
            .expect("beside-source resolves to the source parent");
        assert_eq!(
            final_dir, sub,
            "§2.7.1: beside-source output goes in the source's own parent directory"
        );
    }

    // §6.4.1 (G15) / §2.7.1: beside-source on a path with NO parent (an empty/root path — a frozen source file
    // never is) fails clearly (InvalidInput), never a panic (G4/G14).
    #[test]
    fn beside_source_with_no_parent_is_invalid_input() {
        let err = prepare_output_dir(Path::new(""), DestinationMode::BesideSource)
            .expect_err("an empty path has no parent → Err");
        // [Test-Change: P3.88 — old-obsolete+new-correct, §2.7.1] prepare_output_dir now returns
        // PrepareOutputDirError (not io::Error), so err.kind() → matches! on its Io arm — same io kind asserted.
        assert!(
            matches!(&err, PrepareOutputDirError::Io(e) if e.kind() == io::ErrorKind::InvalidInput),
            "§2.7.1: beside-source with no parent directory is a clear InvalidInput failure"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.1: chosen-root re-creates the dropped-root-relative subtree under D
    // (`D/sub/deep`, never flattened), ancestor-by-ancestor, and BOTH intermediate dirs exist on disk.
    #[test]
    fn chosen_root_recreates_the_relative_subtree() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let common = base.path().join("src");
        std::fs::create_dir(&common).expect("create the freeze common root");
        let dest = base.path().join("dest");
        std::fs::create_dir(&dest).expect("create the chosen destination root");
        // source at common/sub/deep/file.csv → final_dir = dest/sub/deep
        let source = common.join("sub").join("deep").join("file.csv");
        let final_dir = prepare_output_dir(
            &source,
            DestinationMode::ChosenRoot {
                root: &dest,
                common_root: &common,
            },
        )
        .expect("chosen-root re-creates the subtree");
        assert_eq!(
            final_dir,
            dest.join("sub").join("deep"),
            "§2.7.1: the relative subtree sub/deep is re-created under the chosen root (never flattened)"
        );
        assert!(
            dest.join("sub").is_dir() && dest.join("sub").join("deep").is_dir(),
            "§2.7.1: each ancestor (sub, then sub/deep) was created ancestor-by-ancestor on disk"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.1: a source DIRECTLY under the common root has an empty relative directory →
    // the final directory is the chosen root ITSELF (no ancestor to create).
    #[test]
    fn chosen_root_source_directly_under_common_root_is_the_root_itself() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let common = base.path().join("src");
        std::fs::create_dir(&common).expect("create the common root");
        let dest = base.path().join("dest");
        std::fs::create_dir(&dest).expect("create the chosen root");
        let source = common.join("file.csv");
        let final_dir = prepare_output_dir(
            &source,
            DestinationMode::ChosenRoot {
                root: &dest,
                common_root: &common,
            },
        )
        .expect("a top-level source resolves to the chosen root itself");
        assert_eq!(
            final_dir, dest,
            "§2.7.1: a source directly under the common root outputs at the chosen root itself"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.1: create-only TOLERATES a pre-existing directory ancestor (a second file in
    // the same subtree, or a re-run) — AlreadyExists on a real dir continues, idempotently.
    #[test]
    fn chosen_root_tolerates_a_preexisting_subtree_directory() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let common = base.path().join("src");
        std::fs::create_dir(&common).expect("create the common root");
        let dest = base.path().join("dest");
        std::fs::create_dir(&dest).expect("create the chosen root");
        // Pre-create dest/sub so the first ancestor already exists (a prior file wrote the same subtree).
        std::fs::create_dir(dest.join("sub")).expect("pre-create the first subtree ancestor");
        let source = common.join("sub").join("deep").join("file.csv");
        let final_dir = prepare_output_dir(
            &source,
            DestinationMode::ChosenRoot {
                root: &dest,
                common_root: &common,
            },
        )
        .expect("create-only tolerates a pre-existing directory ancestor");
        assert_eq!(final_dir, dest.join("sub").join("deep"));
        assert!(
            dest.join("sub").join("deep").is_dir(),
            "§2.7.1: the deeper ancestor is still created when a shallower one pre-exists (create-only, not a failure)"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.1: a NON-directory occupying an ancestor path fails CLEARLY (NotADirectory),
    // never silently overwriting or diverting around it — and the occupying file is left byte-untouched (no-harm).
    #[test]
    fn chosen_root_non_directory_ancestor_collision_fails_clearly() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let common = base.path().join("src");
        std::fs::create_dir(&common).expect("create the common root");
        let dest = base.path().join("dest");
        std::fs::create_dir(&dest).expect("create the chosen root");
        // A regular FILE occupies dest/sub — the ancestor path the subtree needs as a directory.
        let occupier = dest.join("sub");
        std::fs::write(&occupier, b"i am a file, not a directory")
            .expect("plant a non-dir at the ancestor path");
        let source = common.join("sub").join("deep").join("file.csv");
        let err = prepare_output_dir(
            &source,
            DestinationMode::ChosenRoot {
                root: &dest,
                common_root: &common,
            },
        )
        .expect_err("a non-directory ancestor collision fails");
        // [Test-Change: P3.88 — old-obsolete+new-correct, §2.7.1] prepare_output_dir now returns
        // PrepareOutputDirError (not io::Error), so err.kind() → matches! on its Io arm — same io kind asserted.
        assert!(
            matches!(&err, PrepareOutputDirError::Io(e) if e.kind() == io::ErrorKind::NotADirectory),
            "§2.7.1: a non-directory occupying a subtree ancestor fails clearly, never a silent overwrite"
        );
        assert_eq!(
            std::fs::read(&occupier).expect("the occupying file is still readable"),
            b"i am a file, not a directory",
            "§2.7.1 no-harm: the file occupying the ancestor path is left byte-untouched"
        );
        assert!(
            !dest.join("sub").join("deep").exists(),
            "§2.7.1: no deeper subtree was created through/around the non-directory collision"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.1: a `source` NOT under the common root fails clearly (InvalidInput) — the
    // relative subtree cannot be taken, so the item fails rather than re-create a wrong tree.
    #[test]
    fn chosen_root_source_not_under_common_root_is_invalid_input() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let common = base.path().join("src");
        std::fs::create_dir(&common).expect("create the common root");
        let dest = base.path().join("dest");
        std::fs::create_dir(&dest).expect("create the chosen root");
        let source = base.path().join("elsewhere").join("file.csv"); // NOT under `common`
        let err = prepare_output_dir(
            &source,
            DestinationMode::ChosenRoot {
                root: &dest,
                common_root: &common,
            },
        )
        .expect_err("a source outside the common root cannot be re-created");
        // [Test-Change: P3.88 — old-obsolete+new-correct, §2.7.1] prepare_output_dir now returns
        // PrepareOutputDirError (not io::Error), so err.kind() → matches! on its Io arm — same io kind asserted.
        assert!(
            matches!(&err, PrepareOutputDirError::Io(e) if e.kind() == io::ErrorKind::InvalidInput),
            "§2.7.1: a source not under the common root is a clear InvalidInput failure"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.1: a `..` in a (non-canonical) source's relative subpath is REJECTED
    // (InvalidInput), never re-created through — a dropped-root-relative subtree can never escape the chosen
    // root D via traversal (the no-harm / anti-zip-slip default). Nothing is created outside D.
    #[test]
    fn chosen_root_parent_dir_traversal_in_subpath_is_rejected() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let common = base.path().join("src");
        std::fs::create_dir(&common).expect("create the common root");
        let dest = base.path().join("dest");
        std::fs::create_dir(&dest).expect("create the chosen root");
        // A NON-canonical source lexically under `common` but containing `..`: strip_prefix(common) yields
        // `../escape/file.csv`, whose directory part `../escape` carries a ParentDir component (§2.7.1 reject).
        let source = common.join("..").join("escape").join("file.csv");
        let err = prepare_output_dir(
            &source,
            DestinationMode::ChosenRoot {
                root: &dest,
                common_root: &common,
            },
        )
        .expect_err("a `..` traversal in the relative subpath is rejected");
        // [Test-Change: P3.88 — old-obsolete+new-correct, §2.7.1] prepare_output_dir now returns
        // PrepareOutputDirError (not io::Error), so err.kind() → matches! on its Io arm — same io kind asserted.
        assert!(
            matches!(&err, PrepareOutputDirError::Io(e) if e.kind() == io::ErrorKind::InvalidInput),
            "§2.7.1: a non-normal (`..`) component in the relative subtree is rejected, never created through"
        );
        assert!(
            !base.path().join("escape").exists(),
            "§2.7.1 no-harm: the traversal target was never created outside the chosen root"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.1: a create error OTHER than AlreadyExists (a MISSING chosen root D) is
    // propagated as a clear per-item failure (never a panic, G4/G14).
    #[test]
    fn chosen_root_missing_destination_root_propagates_error() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let common = base.path().join("src");
        std::fs::create_dir(&common).expect("create the common root");
        // `dest` is NOT created — the first create_dir under it fails (parent missing).
        let dest = base.path().join("does-not-exist");
        let source = common.join("sub").join("file.csv");
        let err = prepare_output_dir(
            &source,
            DestinationMode::ChosenRoot {
                root: &dest,
                common_root: &common,
            },
        )
        .expect_err("a missing destination root cannot hold the subtree");
        // [Test-Change: P3.88 — old-obsolete+new-correct, §2.7.1] prepare_output_dir now returns
        // PrepareOutputDirError (not io::Error), so err.kind() → matches! on its Io arm — same io kind asserted.
        assert!(
            matches!(&err, PrepareOutputDirError::Io(e) if e.kind() == io::ErrorKind::NotFound),
            "§2.7.1: creating a subtree under a missing root propagates the underlying NotFound, not a panic"
        );
    }
}

// Two STACKED cfg attributes (`#[cfg(test)]` + `#[cfg(unix)]`), NOT `#[cfg(all(test, unix))]`: the compound
// form hides the standalone `#[cfg(test)]` the clippy allow-`expect_used`-in-tests config keys off, reddening
// `-D warnings` (the P1.17 lesson); mirrors this module's existing `*_unix_tests` modules.
#[cfg(test)]
#[cfg(unix)]
mod prepare_output_dir_unix_tests {
    use super::*;

    // §6.4.1 real-FS (G15) / §2.7.1: a symlink-to-DIRECTORY ancestor is FOLLOWED (metadata follows the link →
    // `is_dir() == true`), so create-only TOLERATES it and creates the deeper subtree THROUGH it — the
    // "symlink-to-dir → continue" half of `create_subtree_dir`'s documented behavior, proven with a REAL
    // symlink (the ancestor-into-a-source-tree redirect is caught by the full-final-dir §2.3.3 link-safety at
    // publish, P3.9 — NOT rejected here). A regression to `symlink_metadata` (a plausible "hardening" swap)
    // would start rejecting a legitimate symlink-to-dir ancestor and this test would catch it.
    #[test]
    fn chosen_root_tolerates_a_symlink_to_directory_ancestor() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let common = base.path().join("src");
        std::fs::create_dir(&common).expect("create the common root");
        let dest = base.path().join("dest");
        std::fs::create_dir(&dest).expect("create the chosen root");
        // A REAL directory + a symlink `dest/sub` → that real dir occupies the first ancestor name.
        let real_target = base.path().join("real_target");
        std::fs::create_dir(&real_target).expect("create the symlink's real target dir");
        std::os::unix::fs::symlink(&real_target, dest.join("sub"))
            .expect("symlink the first subtree ancestor onto a real dir");
        // source at common/sub/deep/file.csv → final_dir = dest/sub/deep, created THROUGH the symlink.
        let source = common.join("sub").join("deep").join("file.csv");
        let final_dir = prepare_output_dir(
            &source,
            DestinationMode::ChosenRoot {
                root: &dest,
                common_root: &common,
            },
        )
        .expect("create-only follows a symlink-to-dir ancestor and continues");
        assert_eq!(final_dir, dest.join("sub").join("deep"));
        assert!(
            real_target.join("deep").is_dir(),
            "§2.7.1: a symlink-to-directory ancestor is followed (not rejected); the deeper subtree is created through it"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.1: a symlink-to-FILE ancestor is REJECTED (metadata follows the link →
    // `is_dir() == false` → NotADirectory), exactly like a regular file — the "symlink-to-file → fail clearly"
    // half of the documented behavior. The file behind the symlink is left byte-untouched (no-harm).
    #[test]
    fn chosen_root_symlink_to_file_ancestor_fails_not_a_directory() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let common = base.path().join("src");
        std::fs::create_dir(&common).expect("create the common root");
        let dest = base.path().join("dest");
        std::fs::create_dir(&dest).expect("create the chosen root");
        // A real FILE + a symlink `dest/sub` → that file occupies the first ancestor name.
        let real_file = base.path().join("real_file.csv");
        std::fs::write(&real_file, b"payload").expect("write the symlink's real target file");
        std::os::unix::fs::symlink(&real_file, dest.join("sub"))
            .expect("symlink the first subtree ancestor onto a file");
        let source = common.join("sub").join("deep").join("file.csv");
        let err = prepare_output_dir(
            &source,
            DestinationMode::ChosenRoot {
                root: &dest,
                common_root: &common,
            },
        )
        .expect_err("a symlink-to-file ancestor is not a directory → Err");
        // [Test-Change: P3.88 — old-obsolete+new-correct, §2.7.1] prepare_output_dir now returns
        // PrepareOutputDirError (not io::Error), so err.kind() → matches! on its Io arm — same io kind asserted.
        assert!(
            matches!(&err, PrepareOutputDirError::Io(e) if e.kind() == io::ErrorKind::NotADirectory),
            "§2.7.1: a symlink-to-FILE occupying a subtree ancestor fails clearly (NotADirectory), like a regular file"
        );
        assert_eq!(
            std::fs::read(&real_file).expect("the symlink's target file is still readable"),
            b"payload",
            "§2.7.1 no-harm: the file behind the symlinked ancestor is left byte-untouched"
        );
    }
}

#[cfg(test)]
mod resolve_divert_target_tests {
    use super::*;

    // A non-ephemeral writable base dir under the crate source root — a plain OS temp dir classifies Ephemeral
    // (see location_status_tests), which would divert. `None` on the pathological env where the crate root is
    // itself under an OS temp root (a clean skip, never a false pass). Real FS — never mocked (test-strategy §0.1).
    fn non_ephemeral_dir() -> Option<tempfile::TempDir> {
        let dir = tempfile::Builder::new()
            .prefix("convertia-divert-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .expect("create a temp dir in the crate source root");
        (!crate::platform::is_ephemeral_output_dir(dir.path())).then_some(dir)
    }

    // A fresh `crate::run`-grammar probe name per call (the `<rand>` distinguishes each real probe).
    fn fresh_probe() -> OsString {
        use std::cell::Cell;
        thread_local! {
            static N: Cell<u64> = const { Cell::new(0) };
        }
        let n = N.with(|c| {
            let v = c.get();
            c.set(v + 1);
            v
        });
        OsString::from(format!(".convertia-divert-probe-{n}.part"))
    }

    // §6.4.1 real-FS (G15) / §2.7.3: the FIRST writable candidate wins (the caller's user-chosen → Downloads →
    // Documents priority) — a subsequent writable candidate is never reached.
    #[test]
    fn picks_the_first_writable_candidate() {
        let Some(a) = non_ephemeral_dir() else {
            return;
        };
        let Some(b) = non_ephemeral_dir() else {
            return;
        };
        let mut cache = LocationCache::new();
        let target = resolve_divert_target(
            &[a.path().to_path_buf(), b.path().to_path_buf()],
            &mut cache,
            fresh_probe,
        );
        assert_eq!(
            target,
            DivertTarget::Resolved(a.path().to_path_buf()),
            "§2.7.3: the first writable candidate is the divert root (caller priority order)"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.3: an ephemeral (OS-temp) candidate is SKIPPED; the next writable candidate is
    // the divert root — never divert an output into a place the OS may purge.
    #[test]
    fn skips_an_ephemeral_candidate_and_takes_the_next_writable() {
        let ephemeral = tempfile::tempdir().expect("a real OS-temp dir (classified Ephemeral)");
        let Some(good) = non_ephemeral_dir() else {
            return;
        };
        let mut cache = LocationCache::new();
        let target = resolve_divert_target(
            &[ephemeral.path().to_path_buf(), good.path().to_path_buf()],
            &mut cache,
            fresh_probe,
        );
        assert_eq!(
            target,
            DivertTarget::Resolved(good.path().to_path_buf()),
            "§2.7.3: an ephemeral candidate is skipped; the next writable one is chosen"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.3: when EVERY candidate is unusable (all ephemeral), resolution is Unavailable
    // → the caller fails the item WriteFailed (§2.8), never a bad divert onto a purgeable volume.
    #[test]
    fn all_unusable_candidates_are_unavailable() {
        let e1 = tempfile::tempdir().expect("an OS-temp dir (Ephemeral)");
        let e2 = tempfile::tempdir().expect("another OS-temp dir (Ephemeral)");
        let mut cache = LocationCache::new();
        let target = resolve_divert_target(
            &[e1.path().to_path_buf(), e2.path().to_path_buf()],
            &mut cache,
            fresh_probe,
        );
        assert_eq!(
            target,
            DivertTarget::Unavailable,
            "§2.7.3: no usable candidate → Unavailable (→ §2.8 WriteFailed), never divert onto a purgeable volume"
        );
    }

    // §6.4.1 (G15) / §2.7.3: an empty candidate list resolves Unavailable — defensive (the caller always
    // supplies at least Downloads/Documents, but an empty list must not panic, G4/G14).
    #[test]
    fn empty_candidate_list_is_unavailable() {
        let mut cache = LocationCache::new();
        assert_eq!(
            resolve_divert_target(&[], &mut cache, fresh_probe),
            DivertTarget::Unavailable,
            "§2.7.3: no candidates → Unavailable, never a panic"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.2+§2.7.3: the run LocationCache is reused ACROSS resolve calls — the real
    // batch scenario (Downloads probed once for the whole batch, then reused per diverted item). Two separate
    // resolve_divert_target calls sharing ONE cache + the SAME candidate probe the dir exactly ONCE: the second
    // call is a cache HIT (no re-probe). (A single call with a repeated candidate would return on the first
    // Writable and never reach the second — so the reuse is proven across CALLS, not within one list.)
    #[test]
    fn reuses_the_run_cache_across_calls_probing_a_shared_candidate_once() {
        use std::cell::Cell;
        let Some(good) = non_ephemeral_dir() else {
            return;
        };
        let mut cache = LocationCache::new();
        let calls = Cell::new(0usize);
        let probe = || {
            calls.set(calls.get() + 1);
            OsString::from(format!(".convertia-divert-probe-r{}.part", calls.get()))
        };
        let first = resolve_divert_target(&[good.path().to_path_buf()], &mut cache, probe);
        let second = resolve_divert_target(&[good.path().to_path_buf()], &mut cache, probe);
        assert_eq!(first, DivertTarget::Resolved(good.path().to_path_buf()));
        assert_eq!(
            second, first,
            "§2.7.3: the shared candidate resolves to the same divert root on both calls"
        );
        assert_eq!(
            calls.get(),
            1,
            "§2.7.2: the SECOND resolve is a cache HIT on the shared dir — probed exactly once across the batch"
        );
    }

    // §6.4.1 real-FS (G15, unix) / §2.7.3: a read-only (unwritable) candidate is SKIPPED; the next writable one
    // is chosen — never divert onto an unwritable root. Mirrors location_status_tests' read-only pattern (skip
    // where the platform won't enforce read-only, restoring writability so the TempDir cleanup succeeds).
    #[cfg(unix)]
    #[test]
    fn skips_an_unwritable_candidate_and_takes_the_next_writable() {
        use std::os::unix::fs::PermissionsExt;
        let Some(ro) = non_ephemeral_dir() else {
            return;
        };
        let Some(good) = non_ephemeral_dir() else {
            return;
        };
        std::fs::set_permissions(ro.path(), std::fs::Permissions::from_mode(0o500))
            .expect("make the first candidate read-only (r-x, no write)");
        // Skip where read-only is not enforced (e.g. running as root) — a create would still succeed.
        if std::fs::File::create(ro.path().join(".probe-check")).is_ok() {
            let _ = std::fs::remove_file(ro.path().join(".probe-check"));
            std::fs::set_permissions(ro.path(), std::fs::Permissions::from_mode(0o700))
                .expect("restore writability for cleanup");
            return;
        }
        let mut cache = LocationCache::new();
        let target = resolve_divert_target(
            &[ro.path().to_path_buf(), good.path().to_path_buf()],
            &mut cache,
            fresh_probe,
        );
        std::fs::set_permissions(ro.path(), std::fs::Permissions::from_mode(0o700))
            .expect("restore writability so the TempDir cleanup succeeds");
        assert_eq!(
            target,
            DivertTarget::Resolved(good.path().to_path_buf()),
            "§2.7.3: an unwritable candidate is skipped; the next writable one is chosen"
        );
    }
}

// The publish half (recheck_divert_free_space / publish_to_divert) is `cfg(any(linux, macos, windows))` (it
// calls atomic_publish / available_bytes), so the tests are gated to the shipped desktops too — the same
// two-stacked-`cfg` module form as publish_numbered_tests / atomic_publish_tests.
#[cfg(test)]
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod late_divert_tests {
    use super::*;

    // §6.4.1 unit (G15) / §2.7.2: OutOfDisk / PathTooLong / TooManyCollisions are NOT writability failures →
    // never a late-divert trigger (they fail per §2.8, §2.7.2 names OutOfDisk explicitly).
    #[test]
    fn structural_and_capacity_errors_are_not_divert_triggers() {
        assert!(
            !is_write_divert_trigger(&PublishError::OutOfDisk),
            "§2.7.2: OutOfDisk is explicitly NOT a divert trigger"
        );
        assert!(
            !is_write_divert_trigger(&PublishError::TooManyCollisions),
            "§2.7.2: a degenerate-directory collision cap is structural, not writability — no divert"
        );
        assert!(
            !is_write_divert_trigger(&PublishError::PathTooLong(PathTooLong::Total)),
            "§2.7.2: a path-limit breach is structural, not writability — no divert"
        );
        assert!(
            !is_write_divert_trigger(&PublishError::UnopenableName("CON.tsv".to_owned())),
            "§2.2.4/§2.7.2: an unopenable name is structural (the divert would carry the SAME name and fail \
             identically), not writability — no divert"
        );
    }

    // §6.4.1 unit (G15) / §2.7.2: an Io publish failure of a WRITABILITY kind (permission / read-only /
    // device-gone / network) IS a late-divert trigger; a non-writability Io kind is surfaced unchanged (no divert).
    #[test]
    fn writability_io_kinds_trigger_divert_others_do_not() {
        for kind in [
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::ReadOnlyFilesystem,
            io::ErrorKind::NotFound,
            io::ErrorKind::NetworkUnreachable,
            io::ErrorKind::HostUnreachable,
            io::ErrorKind::ConnectionReset,
            io::ErrorKind::ConnectionAborted,
        ] {
            assert!(
                is_write_divert_trigger(&PublishError::Io(io::Error::from(kind))),
                "§2.7.2: {kind:?} is a writability failure → a late-divert trigger"
            );
        }
        for kind in [
            io::ErrorKind::InvalidData,
            io::ErrorKind::InvalidInput,
            io::ErrorKind::Other,
        ] {
            assert!(
                !is_write_divert_trigger(&PublishError::Io(io::Error::from(kind))),
                "§2.7.2: {kind:?} is not a writability failure → surfaced unchanged, no divert"
            );
        }
    }

    // §6.4.1 real-FS (G15) / §2.14.4: the divert free-space re-check passes when the volume can host the output
    // (needed = 0 always fits) and fails OutOfDisk when it cannot (u64::MAX never fits a real volume).
    #[test]
    fn free_space_recheck_passes_when_it_fits_and_fails_when_it_does_not() {
        let dir = tempfile::tempdir().expect("a real temp dir on a real volume");
        recheck_divert_free_space(dir.path(), 0).expect("§2.14.4: a zero-byte need always fits");
        assert!(
            matches!(
                recheck_divert_free_space(dir.path(), u64::MAX),
                Err(PublishError::OutOfDisk)
            ),
            "§2.14.4: an impossible need (u64::MAX) fails OutOfDisk — never assume it fits"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.5: the late-divert publishes the completed tmp into a writable divert dir
    // (same volume → a direct move) — the output lands under the target base name and tmp is consumed.
    #[test]
    fn publish_to_divert_lands_the_output_in_a_writable_divert_dir() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let divert = base.path().join("divert");
        std::fs::create_dir(&divert).expect("create the divert target dir");
        // tmp on the SAME volume as the divert (both under `base`) → a direct intra-volume publish (no copy).
        let tmp = base.path().join("out.part");
        std::fs::write(&tmp, b"a\tb\tc\n").expect("write the completed engine output");
        let intermediate = divert.join(".convertia-int.part"); // used only on the cross-volume path (not here)
        let source = Path::new("data.csv");
        let outcome = publish_to_divert(&divert, &[], source, "tsv", &tmp, &intermediate)
            .expect("§2.7.5: the late-divert publishes into a writable divert dir");
        assert!(
            matches!(outcome, PublishOutcome::Published { .. }),
            "§2.7.5: the divert publish reports Published"
        );
        assert!(
            divert.join("data.tsv").is_file(),
            "§2.7.5: the output landed under the source base name + target extension in the divert dir"
        );
        assert_eq!(
            std::fs::read(divert.join("data.tsv")).expect("read the published divert output"),
            b"a\tb\tc\n",
            "§2.7.5 no-harm: the completed tmp's EXACT bytes landed in the divert output (content, not just existence)"
        );
        assert!(
            !tmp.exists(),
            "§2.1.2: the same-volume publish MOVED tmp (no 0-byte final, no leftover tmp)"
        );
    }

    // §6.4.1 real-FS (G15) / §2.7.5: a divert target that resolves onto a frozen source is REFUSED (the divert
    // never publishes onto an original) — synthesised by making the divert dir's OWN identity a "frozen source".
    #[test]
    fn publish_to_divert_refuses_a_target_that_resolves_onto_a_source() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let divert = base.path().join("divert");
        std::fs::create_dir(&divert).expect("create the divert dir");
        // Treat the divert dir's own resolved identity as a frozen source → open_verified matches → refuse.
        let dir_identity = resolve_identity(&divert).expect("resolve the divert dir's identity");
        let tmp = base.path().join("out.part");
        std::fs::write(&tmp, b"payload").expect("write tmp");
        let intermediate = divert.join(".convertia-int.part");
        let err = publish_to_divert(
            &divert,
            std::slice::from_ref(&dir_identity),
            Path::new("data.csv"),
            "tsv",
            &tmp,
            &intermediate,
        )
        .expect_err("a divert target resolving onto a frozen source must be refused");
        assert!(
            matches!(err, PublishError::Io(_)),
            "§2.7.5: a divert-onto-source is refused as a write failure (→ §2.8 WriteFailed), never published"
        );
        assert!(
            !divert.join("data.tsv").exists(),
            "§2.7.5 no-harm: nothing was published when the divert target resolved onto a source"
        );
    }

    // §6.4.1 real-FS (G15) / §2.2.1: a source with no file stem fails clearly (InvalidInput), never a panic
    // (G4/G14) — reached after the divert dir opened + free-space passed.
    #[test]
    fn publish_to_divert_with_a_stemless_source_is_invalid_input() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let divert = base.path().join("divert");
        std::fs::create_dir(&divert).expect("create the divert dir");
        let tmp = base.path().join("out.part");
        std::fs::write(&tmp, b"payload").expect("write tmp");
        let intermediate = divert.join(".convertia-int.part");
        let err = publish_to_divert(&divert, &[], Path::new(""), "tsv", &tmp, &intermediate)
            .expect_err("a stemless source cannot name the divert output");
        assert!(
            matches!(&err, PublishError::Io(e) if e.kind() == io::ErrorKind::InvalidInput),
            "§2.2.1: a source with no file stem fails InvalidInput, never a panic"
        );
    }

    // §6.4.1 real-FS (G15): a missing tmp (a bug/race — the completed output vanished) fails clearly as an Io
    // error, never a panic (G4/G14) — the free-space re-check reads tmp's size, so a gone tmp surfaces there.
    #[test]
    fn publish_to_divert_with_a_missing_tmp_is_an_io_error() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let divert = base.path().join("divert");
        std::fs::create_dir(&divert).expect("create the divert dir");
        let tmp = base.path().join("does-not-exist.part"); // never created
        let intermediate = divert.join(".convertia-int.part");
        let err = publish_to_divert(
            &divert,
            &[],
            Path::new("data.csv"),
            "tsv",
            &tmp,
            &intermediate,
        )
        .expect_err("a missing tmp cannot be published");
        assert!(
            matches!(err, PublishError::Io(_)),
            "§2.8: a missing completed output surfaces as a clear Io write failure, never a panic"
        );
    }
}

#[cfg(test)]
mod compute_output_plan_tests {
    use super::*;

    // A non-ephemeral writable base dir under the crate source root (a plain OS temp dir classifies Ephemeral,
    // which resolve_divert_target would refuse). `None` on the pathological env where the crate root is itself
    // under an OS temp root. Real FS — never mocked (test-strategy §0.1).
    fn non_ephemeral_dir() -> Option<tempfile::TempDir> {
        let dir = tempfile::Builder::new()
            .prefix("convertia-plan-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .expect("create a temp dir in the crate source root");
        (!crate::platform::is_ephemeral_output_dir(dir.path())).then_some(dir)
    }

    fn probe() -> OsString {
        OsString::from(".convertia-plan-probe.part")
    }

    // §6.4.1 real-FS (G15) / §1.8+§2.7.1: a WRITABLE beside-source location → final_dir = the source's parent,
    // diverted None, publish_temp_dir == final_dir, base_name = the source stem, extension = the target ext.
    #[test]
    fn beside_source_writable_plans_the_source_parent() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let src_dir = base.path().join("src");
        std::fs::create_dir(&src_dir).expect("create the source's parent");
        let source = src_dir.join("data.csv");
        let mut cache = LocationCache::new();
        let plan = compute_output_plan(
            ItemId::from_index(0),
            &source,
            "tsv",
            DestinationMode::BesideSource,
            LocationStatus::Writable,
            base.path(), // divert_root — unused on the writable path
            &mut cache,
            probe,
        )
        .expect("§1.8: a writable beside-source location plans successfully");
        assert_eq!(plan.job, ItemId::from_index(0));
        assert_eq!(
            plan.final_dir, src_dir,
            "§2.7.1: beside-source final_dir is the source's parent"
        );
        assert_eq!(
            plan.diverted, None,
            "§2.7: a writable location is not diverted"
        );
        assert_eq!(
            plan.base_name,
            OsString::from("data"),
            "§2.2: base_name is the source stem"
        );
        assert_eq!(
            plan.extension,
            OsString::from("tsv"),
            "§2.2: extension is the chosen target ext"
        );
        assert_eq!(
            plan.publish_temp_dir, plan.final_dir,
            "§2.14.1: publish_temp_dir == final_dir in v1"
        );
    }

    // §6.4.1 real-FS (G15) / §1.8+§2.7.1: a WRITABLE chosen-root location re-creates the dropped-root-relative
    // subtree under D → final_dir = D/sub (created on disk), diverted None.
    #[test]
    fn chosen_root_writable_recreates_the_subtree() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let common = base.path().join("common");
        std::fs::create_dir(&common).expect("create the common root");
        let dest = base.path().join("dest");
        std::fs::create_dir(&dest).expect("create the chosen root");
        let source = common.join("sub").join("data.csv");
        let mut cache = LocationCache::new();
        let plan = compute_output_plan(
            ItemId::from_index(1),
            &source,
            "tsv",
            DestinationMode::ChosenRoot {
                root: &dest,
                common_root: &common,
            },
            LocationStatus::Writable,
            base.path(),
            &mut cache,
            probe,
        )
        .expect("§1.8: a writable chosen-root location plans successfully");
        assert_eq!(
            plan.final_dir,
            dest.join("sub"),
            "§2.7.1: the relative subtree is re-created under the chosen root D"
        );
        assert!(
            dest.join("sub").is_dir(),
            "§2.7.1: the subtree ancestor was created on disk"
        );
        assert_eq!(plan.diverted, None);
    }

    // §6.4.1 real-FS (G15) / §1.8+§2.7.3+§2.7.4: a DIVERTED location → final_dir = the (writable) divert root
    // FLAT (no subtree re-creation), diverted = Some(reason).
    #[test]
    fn diverted_location_plans_the_divert_root_flat() {
        let Some(divert) = non_ephemeral_dir() else {
            return;
        };
        let base = tempfile::tempdir().expect("a real temp dir");
        // The source's ORIGINAL (now-unwritable) location — irrelevant to the divert final_dir (flattened).
        let source = base.path().join("deep").join("data.csv");
        let mut cache = LocationCache::new();
        let plan = compute_output_plan(
            ItemId::from_index(2),
            &source,
            "tsv",
            DestinationMode::BesideSource,
            LocationStatus::Divert(DivertReason::Unwritable),
            divert.path(),
            &mut cache,
            probe,
        )
        .expect("§1.8: a diverted location plans to the writable divert root");
        assert_eq!(
            plan.final_dir,
            divert.path(),
            "§2.7.3/§2.7.4: the output diverts FLAT into the divert root (no subtree)"
        );
        assert_eq!(
            plan.diverted,
            Some(DivertReason::Unwritable),
            "§2.7.2: the divert reason is carried on the plan"
        );
        assert_eq!(plan.base_name, OsString::from("data"));
        assert_eq!(plan.publish_temp_dir, plan.final_dir);
    }

    // §6.4.1 real-FS (G15) / §2.7.3: a diverted location whose resolved divert root is ALSO unusable (ephemeral)
    // → DivertUnavailable (→ §2.8 WriteFailed), never a bad divert onto a purgeable volume.
    #[test]
    fn diverted_location_with_an_unusable_divert_root_is_unavailable() {
        let ephemeral = tempfile::tempdir().expect("an OS-temp dir (an ephemeral divert root)");
        let base = tempfile::tempdir().expect("a real temp dir");
        let source = base.path().join("data.csv");
        let mut cache = LocationCache::new();
        let err = compute_output_plan(
            ItemId::from_index(3),
            &source,
            "tsv",
            DestinationMode::BesideSource,
            LocationStatus::Divert(DivertReason::Unwritable),
            ephemeral.path(), // itself ephemeral → resolve_divert_target returns Unavailable
            &mut cache,
            probe,
        )
        .expect_err("§2.7.3: an unusable divert root cannot plan");
        assert!(
            matches!(err, OutputPlanError::DivertUnavailable),
            "§2.7.3: no usable divert target → DivertUnavailable (→ §2.8 WriteFailed), never a bad divert"
        );
    }

    // §6.4.1 (G15) / §2.2.1: a stemless source fails clearly (Io InvalidInput) BEFORE any location work, never a
    // panic (G4/G14).
    #[test]
    fn stemless_source_is_invalid_input() {
        let base = tempfile::tempdir().expect("a real temp dir");
        let mut cache = LocationCache::new();
        let err = compute_output_plan(
            ItemId::from_index(4),
            Path::new(""),
            "tsv",
            DestinationMode::BesideSource,
            LocationStatus::Writable,
            base.path(),
            &mut cache,
            probe,
        )
        .expect_err("a stemless source cannot name the output");
        assert!(
            matches!(&err, OutputPlanError::Io(e) if e.kind() == io::ErrorKind::InvalidInput),
            "§2.2.1: a stemless source fails Io(InvalidInput), never a panic"
        );
    }
}

/// The §2.1.2 FAT/exFAT-class destination fault-injection fence (P3.65) — the sibling of [`kill_after_sync`]
/// for the OTHER §2 precondition no test can create: a filesystem offering neither a no-replace rename nor
/// hardlinks. `#[cfg(test)]`-only, so the production build is byte-identical and this widens NO runtime
/// surface; `pub(crate)` (unlike `kill_after_sync`, whose users are all inside `crate::fs_guard`) because the
/// consumer under test is TIER-1 — `crate::orchestrator`'s reactive `Ok(NoAtomicPublishSupport) =>
/// divert_completed` arm, which had no coverage at all before this box.
/// [Build-Session-Entscheidung: P3.65]
#[cfg(test)]
pub(crate) mod fat_class_destination {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};

    thread_local! {
        static ARMED_DIR: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    }

    /// True iff the current thread has an [`Armed`] guard live for `parent_dir` (or an ancestor of it) — read
    /// by the [`super::publish_numbered_capped`] `#[cfg(test)]` fence.
    pub(crate) fn armed_for(parent_dir: &Path) -> bool {
        ARMED_DIR.with(|armed| {
            armed
                .borrow()
                .as_deref()
                .is_some_and(|dir| parent_dir.starts_with(dir))
        })
    }

    /// RAII: make `dir` (and anything beneath it) behave as a FAT/exFAT-class destination for the current
    /// thread; **restore the previous armed state on drop** (even on a test panic), so no armed state leaks
    /// past the test's scope. Thread-local, so a parallel test on another thread is unaffected.
    ///
    /// The guard carries the state it displaced rather than clearing to `None`, so NESTING is sound: this
    /// fence is `pub(crate)` and armable from `crate::orchestrator`, so an inner guard's drop must not
    /// silently disarm an outer one that is still in scope. [Build-Session-Entscheidung: P3.65]
    pub(crate) struct Armed {
        previous: Option<PathBuf>,
    }

    impl Armed {
        #[must_use = "the returned Armed guard restores the FAT/exFAT fence on Drop; discarding it (a bare \
                      `Armed::arm(dir);` or `let _ = …`) would disarm IMMEDIATELY, silently defeating the \
                      fault injection — bind it to a named `_fat` for the intended scope"]
        pub(crate) fn arm(dir: &Path) -> Self {
            let previous = ARMED_DIR.with(|armed| armed.borrow_mut().replace(dir.to_path_buf()));
            Armed { previous }
        }
    }

    impl Drop for Armed {
        fn drop(&mut self) {
            let previous = self.previous.take();
            ARMED_DIR.with(|armed| *armed.borrow_mut() = previous);
        }
    }
}

/// §2.1.3 crash/power-loss fault-injection kill switch (P3.19.1) — `#[cfg(test)]` ONLY. A **thread-local** flag
/// [`atomic_publish`] checks in the post-`sync_all`-pre-rename window; when armed, the publish "dies" there so
/// the two-state-invariant test can inspect the on-disk state. Thread-local (cargo runs each `#[test]` on its
/// own thread, so an armed kill never leaks into a parallel test) + a RAII [`Armed`] guard that disarms on drop
/// (so an early test panic still disarms). The not(test) build has NEITHER this module NOR the fence — the
/// production `atomic_publish` is byte-identical. [Build-Session-Entscheidung: P3.19.1]
#[cfg(test)]
mod kill_after_sync {
    use std::cell::Cell;

    thread_local! {
        static ARMED: Cell<bool> = const { Cell::new(false) };
    }

    /// True iff the current thread has an [`Armed`] kill guard live — read by the [`super::atomic_publish`]
    /// `#[cfg(test)]` fence.
    pub(super) fn armed() -> bool {
        ARMED.with(Cell::get)
    }

    /// RAII: arm the post-`sync_all`-pre-rename kill for the current thread; **disarm on drop** (even on a test
    /// panic), so no armed state can leak past the test's scope.
    pub(super) struct Armed;

    impl Armed {
        #[must_use = "the returned Armed guard disarms the kill switch on Drop; discarding it (a bare \
                      `Armed::arm();` or `let _ = …`) would disarm IMMEDIATELY, silently defeating the \
                      fault injection — bind it to a named `_kill` for the intended scope"]
        pub(super) fn arm() -> Self {
            ARMED.with(|a| a.set(true));
            Armed
        }
    }

    impl Drop for Armed {
        fn drop(&mut self) {
            ARMED.with(|a| a.set(false));
        }
    }
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

    // §2.3.1 / G48 (the 2026-07-21 P3.73 P0 ruling, item 5): every Windows dangerous-PATH class MUST resolve
    // to `Err` (never `Ok`) from `resolve_identity` — the fn the P0 ruling re-homed the `win_*` fuzz fixtures
    // to. This makes the G48 REQUIRED_FIXTURES "MUST return Err on Windows (via resolve_identity)" contract
    // RUNTIME-true (not merely fixture-byte-shape-true) and mutation-proof: a mutant that lets any class
    // through would flip an `is_err()` here. The five representative literals mirror the committed
    // `fuzz/corpus/fs_guard_resolve_identity/win_*` seeds; each is a non-existent / syntactically-invalid path
    // so it Errs via `InvalidInput` (the device namespace) or `NotFound` (the rest) — the specific kind is not
    // asserted (it is OS-error-dependent), only the load-bearing "never Ok" verdict. [Build-Session-Entscheidung: P3.73]
    #[test]
    fn every_windows_dangerous_path_class_resolves_to_err() {
        for (class, literal) in [
            ("device namespace", r"\\.\NUL"),
            ("reserved DOS name", "CON.jpg"),
            ("drive-relative", "C:relative-no-backslash.txt"),
            ("UNC share", r"\\server\share\payload.txt"),
            ("trailing dot", "trailing-dot.txt."),
        ] {
            assert!(
                resolve_identity(Path::new(literal)).is_err(),
                "§2.3.1/G48: resolve_identity must return Err for the Windows {class} class ({literal:?}), never Ok"
            );
        }
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

#[cfg(test)]
mod check_path_limit_tests {
    //! §6.4.1 unit (G15/G48) for the §2.2.3 per-OS path-limit gate ([`check_path_limit`], P3.11): the 255-unit
    //! per-component ceiling + the per-OS NUL-inclusive total ceiling, fail-never-truncate. Pure logic, no FS.
    //! The per-OS UNIT (UTF-16 vs bytes) is exercised by whichever `os_str_units` branch this CI leg compiles;
    //! the bytes-not-chars distinction is pinned by the unix module below.
    use super::*;

    // §2.2.3 (G15): a normal short path is within every per-OS limit → Ok.
    #[test]
    fn a_normal_path_is_within_limits() {
        assert_eq!(
            check_path_limit(Path::new("dir/subdir/photo.webp")),
            Ok(()),
            "§2.2.3: a normal short path is within every per-OS limit"
        );
    }

    // §2.2.3 (G15): the empty path has no components and zero length → Ok (a degenerate but valid input).
    #[test]
    fn an_empty_path_is_ok() {
        assert_eq!(
            check_path_limit(Path::new("")),
            Ok(()),
            "§2.2.3: the empty path breaches no limit"
        );
    }

    // §2.2.3 (G15): a component of EXACTLY 255 units fits; 256 breaches the per-name ceiling →
    // PathTooLong::Component (the boundary — kills an off-by-one mutant). ASCII, so 1 char == 1 unit on both OS.
    #[test]
    fn the_component_ceiling_is_255_units() {
        assert_eq!(
            check_path_limit(Path::new(&"a".repeat(255))),
            Ok(()),
            "§2.2.3: a 255-unit component fits the per-name ceiling"
        );
        assert_eq!(
            check_path_limit(Path::new(&"a".repeat(256))),
            Err(PathTooLong::Component),
            "§2.2.3: a 256-unit component breaches the 255 per-name ceiling → PathTooLong::Component"
        );
    }

    // §2.2.3 (G15/G48): a path far over the per-OS total ceiling → PathTooLong::Total. Many 1-unit components
    // (each under the component ceiling), so it is the TOTAL check that fires — truncation is never the escape.
    #[test]
    fn a_path_far_over_the_total_ceiling_is_too_long() {
        let long = "a/".repeat(MAX_TOTAL_UNITS_INCL_NUL); // ~2× the ceiling; each component "a" is 1 unit
        assert_eq!(
            check_path_limit(Path::new(&long)),
            Err(PathTooLong::Total),
            "§2.2.3: a path over the per-OS total ceiling fails PathTooLong::Total (never truncated)"
        );
    }

    /// An ASCII path of EXACTLY `n` chars (so `os_str_units == n` on every OS), every component ≤ 200 (< 255),
    /// so a LENGTH test exercises the TOTAL check, not the component check.
    fn ascii_path_of_len(n: usize) -> String {
        let mut s = String::with_capacity(n);
        while s.len() < n {
            if !s.is_empty() {
                s.push('/');
                if s.len() == n {
                    break;
                }
            }
            let seg = (n - s.len()).min(200);
            for _ in 0..seg {
                s.push('a');
            }
        }
        s
    }

    // §2.2.3 (G15): the total ceiling is NUL-INCLUSIVE — a path of exactly `MAX-1` units fits (units + NUL ==
    // MAX) → Ok, and `MAX` units breaches it (units + NUL == MAX+1) → Total. Proves the `+ 1` NUL term
    // ([Build-Session-Entscheidung: P3.11]); kills a mutant that drops it.
    #[test]
    fn the_total_ceiling_is_nul_inclusive() {
        let at_limit = ascii_path_of_len(MAX_TOTAL_UNITS_INCL_NUL - 1);
        assert_eq!(
            at_limit.len(),
            MAX_TOTAL_UNITS_INCL_NUL - 1,
            "precondition: the built path is exactly MAX-1 ASCII chars"
        );
        assert_eq!(
            check_path_limit(Path::new(&at_limit)),
            Ok(()),
            "§2.2.3: a path of MAX-1 units fits (units + NUL == the per-OS ceiling)"
        );
        let over = ascii_path_of_len(MAX_TOTAL_UNITS_INCL_NUL);
        assert_eq!(
            over.len(),
            MAX_TOTAL_UNITS_INCL_NUL,
            "precondition: the built path is exactly MAX ASCII chars"
        );
        assert_eq!(
            check_path_limit(Path::new(&over)),
            Err(PathTooLong::Total),
            "§2.2.3: a path of MAX units breaches the NUL-inclusive ceiling → PathTooLong::Total"
        );
    }
}

// §6.4.1 unix (G15): the §2.2.3 component ceiling is measured in BYTES on Unix (NAME_MAX), not chars — a
// multi-byte name that is few CHARS but many BYTES is bounded correctly. TWO STACKED cfg attrs (`#[cfg(test)]`
// then `#[cfg(unix)]`), NOT `all(test, unix)` — the P1.17 compound-cfg clippy::expect_used trap.
#[cfg(test)]
#[cfg(unix)]
mod check_path_limit_unix_tests {
    use super::*;

    // §2.2.3 (G15): 63 emoji = 63 chars but 252 BYTES (≤255) → fits; 64 emoji = 64 chars but 256 BYTES (>255)
    // → Component. Proves the ceiling counts BYTES, not chars (a char-count would pass both).
    #[test]
    fn the_component_ceiling_is_bytes_not_chars_on_unix() {
        let within = "🎉".repeat(63); // 63 chars, 252 bytes
        assert_eq!(
            within.len(),
            252,
            "precondition: 63 emoji is 252 UTF-8 bytes"
        );
        assert_eq!(
            check_path_limit(Path::new(&within)),
            Ok(()),
            "§2.2.3: 252 bytes (63 emoji) is within the 255-BYTE per-component ceiling"
        );
        let over = "🎉".repeat(64); // 64 chars, 256 bytes
        assert_eq!(over.len(), 256, "precondition: 64 emoji is 256 UTF-8 bytes");
        assert_eq!(
            check_path_limit(Path::new(&over)),
            Err(PathTooLong::Component),
            "§2.2.3: 256 bytes (64 emoji) breaches the 255-BYTE ceiling — measured in bytes, not chars"
        );
    }
}

// §6.4.1 windows (G15): the §2.2.3 component ceiling is measured in UTF-16 CODE UNITS on Windows (NTFS /
// MAX_PATH count wide chars), not chars and not WTF-8 bytes — a supplementary-plane char is 2 UTF-16 units.
// TWO STACKED cfg attrs (`#[cfg(test)]` then `#[cfg(windows)]`) — the P1.17 compound-cfg trap sibling.
#[cfg(test)]
#[cfg(windows)]
mod check_path_limit_windows_tests {
    use super::*;

    // §2.2.3 (G15): '🎉' (U+1F389, supplementary plane) = 2 UTF-16 code units (a surrogate pair), 4 UTF-8
    // bytes, 1 char. 127 emoji = 254 UTF-16 units (≤255) → Ok — proves it is NOT counting the 508 WTF-8 bytes
    // (a byte-count would breach). 128 emoji = 256 UTF-16 units (>255) → Component — proves it is NOT counting
    // the 128 chars (a char-count would fit). Together they pin UTF-16-unit counting, not chars, not bytes.
    #[test]
    fn the_component_ceiling_is_utf16_code_units_on_windows() {
        let within = "🎉".repeat(127); // 127 chars, 254 UTF-16 units, 508 bytes
        assert_eq!(
            check_path_limit(Path::new(&within)),
            Ok(()),
            "§2.2.3: 254 UTF-16 units (127 emoji) fits — NOT the 508 bytes a byte-count would see"
        );
        let over = "🎉".repeat(128); // 128 chars, 256 UTF-16 units
        assert_eq!(
            check_path_limit(Path::new(&over)),
            Err(PathTooLong::Component),
            "§2.2.3: 256 UTF-16 units (128 emoji) breaches 255 — measured in UTF-16 units, not chars"
        );
    }
}

// §6.4.1/§6.4.3 real-FS unix (G15/G31) for the §2.1.2 single-call no-replace publish primitive
// ([`publish_noreplace`], P3.12). Never mock the FS under test (test-strategy §0.1): a REAL temp dir + a REAL
// rustix rename. TWO STACKED cfg attrs (`#[cfg(test)]` then the `any(linux, macos)` predicate matching the
// primitive's own cfg) — NOT a compound `all(test, …)` (the P1.17 compound-cfg trap). The `Unsupported`
// (EINVAL/ENOTSUP) arm is driven by the P3.13 link+unlink fallback; reaching it from a REAL filesystem needs a
// mounted FAT/exFAT-class volume, whose home is the P11.25 release-candidate verification on removable media
// (the environment bound P3.65 recorded — see the `publish_once` note).
#[cfg(test)]
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod publish_noreplace_tests {
    use super::*;
    use std::ffi::OsStr;

    /// The P3.9 verified parent handle for `dir` (empty frozen set → always Verified); binds via
    /// match→Option→`expect` (never a hard-fail macro the deferral gate flags).
    fn verified(dir: &Path) -> VerifiedParentDir {
        match open_verified_parent_dir(dir, &[]).expect("open the dest dir") {
            ParentDirVerdict::Verified(v) => Some(v),
            ParentDirVerdict::ResolvesOntoSource => None,
        }
        .expect("a real dir with an empty frozen set verifies")
    }

    // §2.1.2 (G15/G31): a fresh `leaf` publishes — the tmp is renamed onto `leaf` (create-only), the content
    // lands byte-exact in the verified parent dir, and the tmp is GONE (moved, not copied — no residual, never
    // a 0-byte `final`).
    #[test]
    fn a_fresh_leaf_publishes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let tmp = dir.path().join("out.part");
        std::fs::write(&tmp, b"converted bytes").expect("write the tmp");
        let outcome = publish_noreplace(&parent, &tmp, OsStr::new("out.tsv")).expect("publish");
        assert_eq!(
            outcome,
            PublishAttempt::Published,
            "§2.1.2: a fresh leaf publishes"
        );
        assert_eq!(
            std::fs::read(dir.path().join("out.tsv")).expect("read the published file"),
            b"converted bytes",
            "§2.1.2: the leaf is published relative to the verified parent dir, carrying the tmp's exact bytes"
        );
        assert!(
            !tmp.exists(),
            "§2.1.2: the tmp was renamed (moved), never left behind"
        );
    }

    // §2.1.2 THE NO-HARM PROOF (G15/G31): publishing onto an EXISTING `leaf` returns NameTaken and NEVER
    // clobbers it — the existing file is byte-identical afterward, and the tmp is untouched (the §2.2.2 loop
    // re-picks the next candidate). The SSOT never-harm guarantee at the publish primitive.
    #[test]
    fn a_collision_never_clobbers_the_existing_target() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let existing = dir.path().join("taken.tsv");
        std::fs::write(&existing, b"PRE-EXISTING must survive").expect("write the existing target");
        let tmp = dir.path().join("out.part");
        std::fs::write(&tmp, b"new bytes").expect("write the tmp");
        let outcome =
            publish_noreplace(&parent, &tmp, OsStr::new("taken.tsv")).expect("publish attempt");
        assert_eq!(
            outcome,
            PublishAttempt::NameTaken,
            "§2.1.2: an existing leaf is NameTaken (EEXIST), never replaced"
        );
        assert_eq!(
            std::fs::read(&existing).expect("read the existing target"),
            b"PRE-EXISTING must survive",
            "§2.1.2 no-harm: the existing target is byte-identical — the no-replace rename NEVER clobbered it"
        );
        assert_eq!(
            std::fs::read(&tmp).expect("read the tmp"),
            b"new bytes",
            "§2.1.2: the tmp is untouched on collision (the §2.2.2 loop re-picks the next candidate)"
        );
    }
}

// §6.4.1/§6.4.3 real-FS unix (G15/G31) for the §2.1.2 `link`+`unlink` fallback publish primitive
// ([`publish_link_fallback`], P3.13) + its §2.1.3 success-window residual handling. Never mock the FS under
// test (test-strategy §0.1): a REAL temp dir + a REAL rustix `linkat`/`unlink`. TWO STACKED cfg attrs
// (`#[cfg(test)]` then the `any(linux, macos)` predicate matching the primitive's own cfg) — NOT a compound
// `all(test, …)` (the P1.17 compound-cfg trap). The `Unsupported` (EPERM/ENOTSUP on a FAT/exFAT-class volume
// that lacks hardlinks) arm needs such a volume MOUNTED, so its home is the P11.25 release-candidate
// verification on removable media (the environment bound P3.65 recorded — see the `publish_once` note).
#[cfg(test)]
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod publish_link_fallback_tests {
    use super::*;
    use std::ffi::OsStr;
    use std::os::unix::fs::PermissionsExt;

    /// The P3.9 verified parent handle for `dir` (empty frozen set → always Verified); binds via
    /// match→Option→`expect` (never a hard-fail macro the deferral gate flags).
    fn verified(dir: &Path) -> VerifiedParentDir {
        match open_verified_parent_dir(dir, &[]).expect("open the dest dir") {
            ParentDirVerdict::Verified(v) => Some(v),
            ParentDirVerdict::ResolvesOntoSource => None,
        }
        .expect("a real dir with an empty frozen set verifies")
    }

    /// Restore owner rwx on `dir` so the `TempDir` Drop can `remove_dir_all` it (the read-only dir we set to
    /// force the unlink failure blocks removal). An explicit `0o700` mode — clippy forbids the ambiguous
    /// `set_readonly(false)`. Best-effort: a restore failure must not itself abort the test's cleanup.
    fn restore_writable(dir: &Path) {
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }

    // §2.1.2 (G15/G31): the fallback publishes a fresh `leaf` by hard-linking the tmp onto it, then reaps the
    // tmp — `leaf` carries the exact bytes and NO residual `*.part` remains (the clean success path, no §2.1.3
    // success-window leftover).
    #[test]
    fn a_fresh_leaf_publishes_via_link_and_reaps_the_tmp() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let tmp = dir.path().join("out.part");
        std::fs::write(&tmp, b"converted bytes").expect("write the tmp");
        let outcome = publish_link_fallback(&parent, &tmp, OsStr::new("out.tsv")).expect("publish");
        assert_eq!(
            outcome,
            LinkPublishAttempt::Published,
            "§2.1.2: a fresh leaf publishes via link+unlink"
        );
        assert_eq!(
            std::fs::read(dir.path().join("out.tsv")).expect("read the published file"),
            b"converted bytes",
            "§2.1.2: the leaf is hard-linked relative to the verified parent dir, carrying the tmp's exact bytes"
        );
        assert!(
            !tmp.exists(),
            "§2.1.2: the source tmp is reaped (unlinked) on the clean path — no residual"
        );
    }

    // §2.1.2 THE NO-HARM PROOF (G15/G31): `link` onto an EXISTING `leaf` returns NameTaken (EEXIST) and NEVER
    // clobbers it — the existing file is byte-identical afterward, the tmp is untouched (the §2.2.2 loop
    // re-picks). The SSOT never-harm guarantee via the portable fallback, matching the single-call primitive.
    #[test]
    fn a_collision_never_clobbers_the_existing_target_via_link() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let existing = dir.path().join("taken.tsv");
        std::fs::write(&existing, b"PRE-EXISTING must survive").expect("write the existing target");
        let tmp = dir.path().join("out.part");
        std::fs::write(&tmp, b"new bytes").expect("write the tmp");
        let outcome =
            publish_link_fallback(&parent, &tmp, OsStr::new("taken.tsv")).expect("publish attempt");
        assert_eq!(
            outcome,
            LinkPublishAttempt::NameTaken,
            "§2.1.2: an existing leaf is NameTaken (EEXIST), never replaced"
        );
        assert_eq!(
            std::fs::read(&existing).expect("read the existing target"),
            b"PRE-EXISTING must survive",
            "§2.1.2 no-harm: the existing target is byte-identical — `link` NEVER clobbered it"
        );
        assert_eq!(
            std::fs::read(&tmp).expect("read the tmp"),
            b"new bytes",
            "§2.1.2: the tmp is untouched on collision (the §2.2.2 loop re-picks the next candidate)"
        );
    }

    // §2.1.3 THE SUCCESS-WINDOW RESIDUAL (G15/G31): when `link` commits but `unlink(tmp)` then FAILS, the
    // publish is STILL a success — `leaf` is complete + durable — and the leftover tmp `*.part` is signalled as
    // a residual for the §2.6.4 sweep (PublishedResidualTmp), NOT returned as an item failure. Forced on a real
    // FS by placing the tmp in its OWN read-only dir (so removing it is denied) while `link` INTO the writable
    // dest dir still succeeds; in production the tmp is a sibling of `leaf` (§2.1.1) — the residual DECISION
    // exercised is identical.
    #[test]
    fn a_failed_unlink_leaves_a_residual_part_not_an_item_failure() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        // The tmp lives in its OWN subdir; making that subdir read-only denies `unlink(tmp)` (unlink needs
        // write on the tmp's PARENT dir) while `link` INTO the writable dest dir still succeeds (both are on
        // the one temp volume, so the hardlink is intra-volume).
        let src_dir = dir.path().join("src");
        std::fs::create_dir(&src_dir).expect("create the tmp's own dir");
        let tmp = src_dir.join("out.part");
        std::fs::write(&tmp, b"converted bytes").expect("write the tmp");
        let readonly = {
            let mut perms = std::fs::metadata(&src_dir)
                .expect("stat the src dir")
                .permissions();
            perms.set_readonly(true);
            perms
        };
        std::fs::set_permissions(&src_dir, readonly).expect("make the tmp's dir read-only");
        // Skip ONLY where the read-only chmod is not enforced against us (running as root, or a permission-less
        // FS): probe by trying to create a file in the now-read-only dir; if that succeeds we cannot force the
        // unlink failure this leg needs, so restore + skip (never a false pass — the same 'skip only THAT'
        // discipline as the hardlink-unsupported-volume test). Linux/macOS CI runners run non-root, so the
        // residual leg is exercised there.
        if std::fs::File::create(src_dir.join(".probe")).is_ok() {
            let _ = std::fs::remove_file(src_dir.join(".probe"));
            restore_writable(&src_dir);
            return;
        }
        let outcome = publish_link_fallback(&parent, &tmp, OsStr::new("out.tsv"));
        // Restore write BEFORE asserting so the TempDir Drop can clean up even if an assertion fails.
        restore_writable(&src_dir);
        let outcome = outcome.expect("publish (a failed reap is NOT an error)");
        assert_eq!(
            outcome,
            LinkPublishAttempt::PublishedResidualTmp,
            "§2.1.3: link committed but unlink failed → PublishedResidualTmp (success + a residual), not a failure"
        );
        assert_eq!(
            std::fs::read(dir.path().join("out.tsv")).expect("read the published file"),
            b"converted bytes",
            "§2.1.3: the leaf is complete + durable even though the reap failed"
        );
        assert!(
            tmp.exists(),
            "§2.1.3: the residual *.part remains for the §2.6.4 sweep to reclaim"
        );
    }
}

// §6.4.1/§6.4.3 real-FS Windows (G15/G31) for the §2.1.2/§2.3.3 Windows create-only publish primitive
// ([`publish_rename_windows`], P3.14) — the FileRenameInformationEx no-replace move + its §2.1.2 bounded AV-retry.
// Never mock the FS under test (test-strategy §0.1): a REAL temp dir + the REAL `NtSetInformationFile`
// FFI (via crate::platform). TWO STACKED cfg attrs (`#[cfg(test)]` then `#[cfg(windows)]`) — NOT a compound
// `all(test, windows)` (the P1.17 compound-cfg clippy `is_cfg_test` trap).
#[cfg(test)]
#[cfg(windows)]
mod publish_rename_windows_tests {
    use super::*;
    use std::ffi::OsStr;

    /// The P3.9 verified parent handle for `dir` (empty frozen set → always Verified); binds via
    /// match→Option→`expect` (never a hard-fail macro the deferral gate flags).
    fn verified(dir: &Path) -> VerifiedParentDir {
        match open_verified_parent_dir(dir, &[]).expect("open the dest dir") {
            ParentDirVerdict::Verified(v) => Some(v),
            ParentDirVerdict::ResolvesOntoSource => None,
        }
        .expect("a real dir with an empty frozen set verifies")
    }

    // §2.1.2 (G15/G31): a fresh `leaf` publishes — the create-only move renames the tmp onto `leaf` relative to
    // the verified parent handle, the bytes land exact, and the tmp is GONE (moved, no residual — Windows
    // consumes tmp atomically, never a 0-byte final).
    #[test]
    fn a_fresh_leaf_publishes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let tmp = dir.path().join("out.part");
        std::fs::write(&tmp, b"converted bytes").expect("write the tmp");
        let outcome =
            publish_rename_windows(&parent, &tmp, OsStr::new("out.tsv")).expect("publish");
        assert_eq!(
            outcome,
            WindowsPublishAttempt::Published,
            "§2.1.2: a fresh leaf publishes"
        );
        assert_eq!(
            std::fs::read(dir.path().join("out.tsv")).expect("read the published file"),
            b"converted bytes",
            "§2.1.2: the leaf is published relative to the verified parent dir, carrying the tmp's exact bytes"
        );
        assert!(
            !tmp.exists(),
            "§2.1.2: the tmp was moved (create-only), never left behind"
        );
    }

    // §2.1.2 THE NO-HARM PROOF (G15/G31): publishing onto an EXISTING `leaf` returns NameTaken and NEVER
    // clobbers it — the existing file is byte-identical afterward, the tmp is untouched (the §2.2.2 loop
    // re-picks). The SSOT never-harm guarantee at the Windows publish primitive.
    #[test]
    fn a_collision_never_clobbers_the_existing_target() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let existing = dir.path().join("taken.tsv");
        std::fs::write(&existing, b"PRE-EXISTING must survive").expect("write the existing target");
        let tmp = dir.path().join("out.part");
        std::fs::write(&tmp, b"new bytes").expect("write the tmp");
        let outcome = publish_rename_windows(&parent, &tmp, OsStr::new("taken.tsv"))
            .expect("publish attempt");
        assert_eq!(
            outcome,
            WindowsPublishAttempt::NameTaken,
            "§2.1.2: an existing leaf is NameTaken (ERROR_ALREADY_EXISTS), never replaced"
        );
        assert_eq!(
            std::fs::read(&existing).expect("read the existing target"),
            b"PRE-EXISTING must survive",
            "§2.1.2 no-harm: the existing target is byte-identical — the no-replace move NEVER clobbered it"
        );
        assert_eq!(
            std::fs::read(&tmp).expect("read the tmp"),
            b"new bytes",
            "§2.1.2: the tmp is untouched on collision (the §2.2.2 loop re-picks the next candidate)"
        );
    }

    // §2.1.2/§2.8 AV-RETRY EXHAUSTION (G15/G31): a PERSISTENT lock on the tmp (a second handle NOT sharing
    // DELETE, exactly as an AV scanner / indexer would hold) makes every DELETE-access open raise
    // SHARING_VIOLATION; the bounded AV-retry loop exhausts and surfaces a §2.8 WriteFailed `Err` — NOT a
    // panic, NOT a clobber, and the original tmp is untouched.
    #[test]
    fn a_persistent_lock_exhausts_the_av_retry_to_writefailed() {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let tmp = dir.path().join("out.part");
        std::fs::write(&tmp, b"converted bytes").expect("write the tmp");
        // Hold the tmp open WITHOUT FILE_SHARE_DELETE → the publish's DELETE-access open persistently hits
        // ERROR_SHARING_VIOLATION (never released), so the bounded AV-retry exhausts to WriteFailed.
        let blocker = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&tmp)
            .expect("hold a no-delete-share handle on the tmp");
        let result = publish_rename_windows(&parent, &tmp, OsStr::new("out.tsv"));
        drop(blocker);
        assert!(
            result.is_err(),
            "§2.1.2/§2.8: a persistent SHARING_VIOLATION exhausts the bounded AV-retry → WriteFailed Err, not a panic"
        );
        assert!(
            !dir.path().join("out.tsv").exists(),
            "§2.1.2 no-harm: no leaf was published on the failed path"
        );
        assert_eq!(
            std::fs::read(&tmp).expect("read the tmp"),
            b"converted bytes",
            "§2.1.2: the tmp (our source) is byte-identical when the publish fails"
        );
    }

    // §2.1.2 AV-RETRY RECOVERY (G15/G31): a lock that CLEARS mid-retry (a background thread releasing the
    // no-delete-share handle well within the ~184ms bounded-retry budget) lets an early retry fail but a
    // subsequent one SUCCEED — the core value of the bounded-retry design. Asserting Published never spuriously
    // fails
    // (it holds whichever attempt wins), so this is a real-FS exercise of the recovery path, not a flaky race.
    #[test]
    fn the_av_retry_recovers_when_the_lock_clears_mid_retry() {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let tmp = dir.path().join("out.part");
        std::fs::write(&tmp, b"converted bytes").expect("write the tmp");
        // Held at the call's start (acquired synchronously here), then released ~40ms in — before the bounded
        // budget exhausts — so the early attempts hit SHARING_VIOLATION and a subsequent one succeeds.
        let blocker = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&tmp)
            .expect("hold a no-delete-share handle on the tmp");
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(40));
            drop(blocker);
        });
        let outcome = publish_rename_windows(&parent, &tmp, OsStr::new("out.tsv"))
            .expect("the publish RECOVERS once the transient lock clears (not an Err)");
        releaser.join().expect("the releaser thread");
        assert_eq!(
            outcome,
            WindowsPublishAttempt::Published,
            "§2.1.2: the bounded AV-retry RECOVERS to a successful publish once the transient lock clears"
        );
        assert_eq!(
            std::fs::read(dir.path().join("out.tsv")).expect("read the published file"),
            b"converted bytes",
            "§2.1.2: the recovered publish carries the tmp's exact bytes"
        );
        assert!(
            !tmp.exists(),
            "§2.1.2: the tmp was moved by the recovered publish"
        );
    }
}

// §6.4.1/§6.4.3 real-FS (G15/G31) for the §2.2.2 numbering ↔ no-clobber publish loop ([`publish_numbered`],
// P3.15) — the whole drop→publish naming/no-clobber contract driven end-to-end over a REAL temp dir + the REAL
// per-OS create-only publish (never mock the FS/publish under test, test-strategy §0.1). Runs on every shipped
// platform (the loop dispatches per-OS via publish_once, so the happy path + numbering assertions hold on
// Win/macOS/Linux alike). TWO STACKED cfg attrs (`#[cfg(test)]` then the shipped-platforms predicate) — NOT a
// compound `all(test, …)` (the P1.17 compound-cfg clippy::expect_used trap). The Unix-only FAT/exFAT
// `NoAtomicPublishSupport` arm needs a FAT/exFAT volume that lacks hardlinks MOUNTED, so — like the underlying
// publish_noreplace/publish_link_fallback `Unsupported` arms — its home is the P11.25 release-candidate
// verification on removable media (the environment bound P3.65 recorded — see the `publish_once` note); what
// the verdict FEEDS is covered by `crate::orchestrator::cross_volume_e2e_tests`.
#[cfg(test)]
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod publish_numbered_tests {
    use super::*;

    /// The P3.9 verified parent handle for `dir` (empty frozen set → always Verified); binds via
    /// match→Option→`expect` (never a hard-fail macro the deferral gate flags).
    fn verified(dir: &Path) -> VerifiedParentDir {
        match open_verified_parent_dir(dir, &[]).expect("open the dest dir") {
            ParentDirVerdict::Verified(v) => Some(v),
            ParentDirVerdict::ResolvesOntoSource => None,
        }
        .expect("a real dir with an empty frozen set verifies")
    }

    // §2.2.1/§2.2.2 (G15/G31): a FREE base name publishes UNNUMBERED at `stem.ext` — the tmp is moved (gone),
    // the published file carries its exact bytes, and no residual (the single-call Unix / Windows path).
    #[test]
    fn a_free_base_name_publishes_unnumbered() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let tmp = dir.path().join(".convertia-tmp.part");
        std::fs::write(&tmp, b"converted bytes").expect("write the tmp");
        let candidates = output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem");
        let outcome = publish_numbered(&parent, dir.path(), &tmp, candidates).expect("publish");
        assert_eq!(
            outcome,
            PublishOutcome::Published {
                leaf: OsString::from("data.tsv"),
                residual_tmp: false,
            },
            "§2.2.1/§2.2.2: a free base name publishes unnumbered at stem.ext, no residual"
        );
        assert_eq!(
            std::fs::read(dir.path().join("data.tsv")).expect("read the published file"),
            b"converted bytes",
            "§2.2.2: the published file carries the tmp's exact bytes"
        );
        assert!(
            !tmp.exists(),
            "§2.1.2: the tmp was moved (published), never left behind"
        );
    }

    // §2.2.2 THE NO-HARM + NUMBERING PROOF (G15/G31): when the base name is already TAKEN, the loop numbers away
    // to the first free `stem (1).ext` and NEVER clobbers the pre-existing file — the taken file is
    // byte-identical afterward, the numbered output carries the tmp's bytes. The headline P3.15 assertion.
    #[test]
    fn a_collision_numbers_away_and_never_clobbers() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let taken = dir.path().join("data.tsv");
        std::fs::write(&taken, b"PRE-EXISTING must survive").expect("write the taken base name");
        let tmp = dir.path().join(".convertia-tmp.part");
        std::fs::write(&tmp, b"new bytes").expect("write the tmp");
        let candidates = output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem");
        let outcome = publish_numbered(&parent, dir.path(), &tmp, candidates).expect("publish");
        assert_eq!(
            outcome,
            PublishOutcome::Published {
                leaf: OsString::from("data (1).tsv"),
                residual_tmp: false,
            },
            "§2.2.2: the taken base name numbers away to the first free stem (1).ext"
        );
        assert_eq!(
            std::fs::read(&taken).expect("read the pre-existing base name"),
            b"PRE-EXISTING must survive",
            "§2.2.2 no-harm: the pre-existing base name is byte-identical — the no-replace publish NEVER clobbered it"
        );
        assert_eq!(
            std::fs::read(dir.path().join("data (1).tsv")).expect("read the numbered output"),
            b"new bytes",
            "§2.2.2: the output lands at the numbered candidate carrying the tmp's exact bytes"
        );
        assert!(
            !tmp.exists(),
            "§2.1.2: the tmp was moved on the winning publish"
        );
    }

    // §2.2.2 (G15/G31): with the base + first two numbered names ALL taken, the loop advances to the first free
    // number `stem (3).ext` — the numbering is driven by the kernel's exclusive create, one candidate at a time.
    #[test]
    fn multiple_collisions_pick_the_first_free_number() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        for name in ["data.tsv", "data (1).tsv", "data (2).tsv"] {
            std::fs::write(dir.path().join(name), b"taken").expect("write a taken name");
        }
        let tmp = dir.path().join(".convertia-tmp.part");
        std::fs::write(&tmp, b"new bytes").expect("write the tmp");
        let candidates = output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem");
        let outcome = publish_numbered(&parent, dir.path(), &tmp, candidates).expect("publish");
        assert_eq!(
            outcome,
            PublishOutcome::Published {
                leaf: OsString::from("data (3).tsv"),
                residual_tmp: false,
            },
            "§2.2.2: with base + (1) + (2) taken, the loop publishes at the first free (3)"
        );
        assert!(
            dir.path().join("data (3).tsv").exists(),
            "§2.2.2: the output published at the first free number"
        );
    }

    // §2.1.2/§2.2 (G15/G31): exhausting the collision cap fails `TooManyCollisions` (never an unbounded loop,
    // never a clobber). Uses the injectable-cap core with cap=3 + three taken names, so the exhaustion is
    // deterministic WITHOUT materialising ~10 000 files (the real temp FS + real publish still run).
    #[test]
    fn the_collision_cap_is_bounded_to_too_many_collisions() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        for name in ["data.tsv", "data (1).tsv", "data (2).tsv"] {
            std::fs::write(dir.path().join(name), b"taken").expect("write a taken name");
        }
        let tmp = dir.path().join(".convertia-tmp.part");
        std::fs::write(&tmp, b"new bytes").expect("write the tmp");
        let candidates = output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem");
        let err = publish_numbered_capped(&parent, dir.path(), &tmp, candidates, 3)
            .expect_err("all candidates within the cap are taken → TooManyCollisions");
        assert!(
            matches!(err, PublishError::TooManyCollisions),
            "§2.1.2/§2.2: exhausting the (injected) collision cap fails TooManyCollisions, never loops unboundedly"
        );
        assert_eq!(
            std::fs::read(dir.path().join("data.tsv")).expect("read a taken name"),
            b"taken",
            "§2.2.2 no-harm: a collision-cap failure never clobbered any existing file"
        );
        assert_eq!(
            std::fs::read(&tmp).expect("read the tmp"),
            b"new bytes",
            "§2.1.2: the tmp (our source) is untouched on the failed loop"
        );
    }

    // §2.2.3 (G15/G31): a candidate whose name COMPONENT exceeds the 255-unit per-name ceiling fails
    // `PathTooLong::Component` — fail clearly, NEVER truncate (§2.2.3 / SSOT). A 256-unit ASCII stem → the leaf
    // `<256>.tsv` is a 260-unit component (1 char == 1 UTF-16 unit == 1 byte for ASCII, so it breaches on every
    // OS), rejected BEFORE any publish attempt (nothing published, the tmp untouched).
    #[test]
    fn an_over_limit_candidate_fails_path_too_long_never_truncates() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let tmp = dir.path().join(".convertia-tmp.part");
        std::fs::write(&tmp, b"bytes").expect("write the tmp");
        let long_stem = "a".repeat(256);
        let source = format!("{long_stem}.csv");
        let candidates = output_name(Path::new(&source), "tsv").expect("a real path has a stem");
        let err = publish_numbered(&parent, dir.path(), &tmp, candidates)
            .expect_err("an over-limit candidate fails clearly, never truncates");
        assert!(
            matches!(err, PublishError::PathTooLong(PathTooLong::Component)),
            "§2.2.3: a candidate whose component exceeds 255 units fails PathTooLong::Component, never truncated"
        );
        assert!(
            !dir.path().join(format!("{long_stem}.tsv")).exists(),
            "§2.2.3: nothing was published on a path-limit failure"
        );
        assert!(
            tmp.exists(),
            "§2.2.3: the tmp is untouched when the candidate is rejected pre-publish"
        );
    }

    // §2.8/G4/G14 (G15/G31): a genuine OS error during publish (here a MISSING `tmp` source) surfaces as a
    // structured `PublishError::Io` — NEVER a panic (this runs on untrusted destination paths outside the §2.12
    // boundary) and NEVER a silent success. The §2.1.1 caller (P3.38) maps it to §2.8 `WriteFailed`.
    #[test]
    fn an_os_error_during_publish_surfaces_as_io_never_a_panic() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        // A tmp source that does NOT exist — the publish primitive hits a genuine OS error (missing source).
        let missing_tmp = dir.path().join(".convertia-does-not-exist.part");
        let candidates = output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem");
        let err = publish_numbered(&parent, dir.path(), &missing_tmp, candidates)
            .expect_err("a missing tmp source cannot publish → a clean Io error, never a panic");
        assert!(
            matches!(err, PublishError::Io(_)),
            "§2.8: a missing publish source surfaces as PublishError::Io (never PathTooLong / TooManyCollisions)"
        );
        if let PublishError::Io(e) = &err {
            assert_eq!(
                e.kind(),
                io::ErrorKind::NotFound,
                "§2.8: the OS error is surfaced faithfully — a missing source is NotFound (read from the Io payload)"
            );
        }
        assert!(
            !dir.path().join("data.tsv").exists(),
            "§2.1.2 no-harm: nothing was published on the OS-error path"
        );
    }
}

// §6.4.1/§6.4.3 real-FS (G15/G31) for the §2.1.1 atomic/durable/no-clobber publish composite ([`atomic_publish`]
// + the [`sync_tmp_bytes`] step-3 / [`fsync_parent_dir`] step-6 durability primitives, P3.16). Never mock the FS
// under test (test-strategy §0.1): a REAL temp dir + the REAL fsync/publish. The composite's crash-DURABILITY
// (that the fsync'd state actually survives a power loss) is the §2.1.3 fault-injection test (P3.19.1, kill in
// the post-`sync_all`-pre-rename window) — here we prove the sequence RUNS correctly (bytes published, no-harm
// held, the durability steps neither corrupt nor fail on the happy path). Runs on every shipped platform. TWO
// STACKED cfg attrs (`#[cfg(test)]` then the shipped-platforms predicate) — NOT a compound `all(test, …)` (the
// P1.17 compound-cfg clippy::expect_used trap).
#[cfg(test)]
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod atomic_publish_tests {
    use super::*;

    /// The P3.9 verified parent handle for `dir` (empty frozen set → always Verified); binds via
    /// match→Option→`expect` (never a hard-fail macro the deferral gate flags).
    fn verified(dir: &Path) -> VerifiedParentDir {
        match open_verified_parent_dir(dir, &[]).expect("open the dest dir") {
            ParentDirVerdict::Verified(v) => Some(v),
            ParentDirVerdict::ResolvesOntoSource => None,
        }
        .expect("a real dir with an empty frozen set verifies")
    }

    // §2.1.1 (G15/G31): the whole composite publishes a fresh name — step-3 sync_all, the numbering publish, and
    // step-6 dir-fsync all succeed, the file lands byte-exact, and the tmp is moved (gone). The happy-path proof
    // that the durability steps neither break nor corrupt the publish.
    #[test]
    fn atomic_publish_publishes_a_fresh_name_durably() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let tmp = dir.path().join(".convertia-tmp.part");
        std::fs::write(&tmp, b"converted bytes").expect("write the tmp");
        let candidates = output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem");
        // [Test-Change: P3.17 — old-obsolete+new-correct, §2.14.3] mechanical call-arity update: the 4-arg
        // atomic_publish call is obsolete (P3.17 added the `same_volume_intermediate` 5th param, §2.14.3); the
        // 5-arg form is correct. The Published-outcome EXPECTATION asserted below is UNCHANGED — no assertion
        // was relaxed/removed; the intermediate arg is unused on this direct (non-cross-device) path.
        let outcome = atomic_publish(
            &parent,
            dir.path(),
            &tmp,
            candidates,
            &dir.path().join(".convertia-xvol.part"),
        )
        .expect("publish");
        assert_eq!(
            outcome,
            PublishOutcome::Published {
                leaf: OsString::from("data.tsv"),
                residual_tmp: false,
            },
            "§2.1.1: the durable composite publishes a free base name at stem.ext"
        );
        assert_eq!(
            std::fs::read(dir.path().join("data.tsv")).expect("read the published file"),
            b"converted bytes",
            "§2.1.1: the published file carries the tmp's exact bytes after the durability sequence"
        );
        assert!(
            !tmp.exists(),
            "§2.1.2: the tmp was moved (published), never left behind"
        );
    }

    // §2.1.1/§2.2.2 THE NO-HARM PROOF THROUGH THE COMPOSITE (G15/G31): a taken base name numbers away and the
    // pre-existing file is byte-identical — the durability wrapper does not weaken the no-clobber guarantee.
    #[test]
    fn atomic_publish_numbers_away_and_never_clobbers() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let taken = dir.path().join("data.tsv");
        std::fs::write(&taken, b"PRE-EXISTING must survive").expect("write the taken base name");
        let tmp = dir.path().join(".convertia-tmp.part");
        std::fs::write(&tmp, b"new bytes").expect("write the tmp");
        let candidates = output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem");
        // [Test-Change: P3.17 — old-obsolete+new-correct, §2.14.3] mechanical call-arity update: the 4-arg
        // atomic_publish call is obsolete (P3.17 added the `same_volume_intermediate` 5th param, §2.14.3); the
        // 5-arg form is correct. The Published-outcome EXPECTATION asserted below is UNCHANGED — no assertion
        // was relaxed/removed; the intermediate arg is unused on this direct (non-cross-device) path.
        let outcome = atomic_publish(
            &parent,
            dir.path(),
            &tmp,
            candidates,
            &dir.path().join(".convertia-xvol.part"),
        )
        .expect("publish");
        assert_eq!(
            outcome,
            PublishOutcome::Published {
                leaf: OsString::from("data (1).tsv"),
                residual_tmp: false,
            },
            "§2.2.2: the composite numbers away from the taken base to the first free stem (1).ext"
        );
        assert_eq!(
            std::fs::read(&taken).expect("read the pre-existing base name"),
            b"PRE-EXISTING must survive",
            "§2.2.2 no-harm: the pre-existing file is byte-identical — the durable composite NEVER clobbered it"
        );
    }

    // §2.1.3 (G15/G31) THE CRASH / POWER-LOSS TWO-STATE INVARIANT — a kill in the post-`sync_all`-pre-rename
    // window (all shipped OS). With the P3.19.1 `#[cfg(test)]` kill-fence ARMED, atomic_publish "dies" right
    // after the durability sync, before the rename (a simulated crash), and the on-disk state is EXACTLY §2.1.3
    // state-2: `final` does NOT exist and the durable tmp (*.part) remains (a discardable run-owned artifact) —
    // NEVER a truncated/0-byte `final`. Disarming and re-publishing then reaches §2.1.3 state-1 (`final` springs
    // into existence COMPLETE at the atomic rename), so the invariant holds across the boundary. A real process
    // cannot be killed at this exact instant from outside, so the `#[cfg(test)]` fence is the only way to prove
    // it dynamically (unlike the §2.1.2 AV-retry, which a real lock triggers). [Build-Session-Entscheidung: P3.19.1]
    #[test]
    fn a_kill_between_sync_and_rename_leaves_no_final_only_a_discardable_part() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let tmp = dir.path().join(".convertia-out.part");
        std::fs::write(&tmp, b"converted bytes").expect("write the tmp");
        let final_name = dir.path().join("data.tsv");

        // ── §2.1.3 STATE-2: a crash in the post-sync-pre-rename window ──
        {
            let _kill = kill_after_sync::Armed::arm(); // RAII: disarmed on scope exit (even on a panic)
            let candidates =
                output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem");
            let killed = atomic_publish(
                &parent,
                dir.path(),
                &tmp,
                candidates,
                &dir.path().join(".convertia-xvol.part"),
            );
            assert!(
                killed.is_err(),
                "§2.1.3: the armed fence returns in the post-sync-pre-rename window (nothing published)"
            );
        }
        // §2.1.3: NO `final` exists — the rename never committed, so there is never a truncated/0-byte `final`.
        assert!(
            !final_name.exists(),
            "§2.1.3 state-2: after a kill before the rename NO `final` exists — never a truncated/0-byte final"
        );
        // The durable tmp (*.part) remains, byte-identical — a discardable run-owned artifact (§2.6 reclaims it).
        assert_eq!(
            std::fs::read(&tmp).expect("read the tmp"),
            b"converted bytes",
            "§2.1.3 state-2: the durable *.part remains byte-identical after the kill (unpublished, discardable)"
        );

        // ── §2.1.3 STATE-1: the un-armed publish completes atomically ──
        let candidates = output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem");
        let outcome = atomic_publish(
            &parent,
            dir.path(),
            &tmp,
            candidates,
            &dir.path().join(".convertia-xvol.part"),
        )
        .expect("§2.1.3: the un-armed publish completes");
        assert!(
            matches!(outcome, PublishOutcome::Published { .. }),
            "§2.1.3 state-1: with the fence disarmed the publish reaches state-1 (final complete)"
        );
        assert_eq!(
            std::fs::read(&final_name).expect("read the final"),
            b"converted bytes",
            "§2.1.3 state-1: `final` sprang into existence COMPLETE — the two-state invariant holds end to end"
        );
    }

    // §2.1.3 NO-HARM UNDER A CRASH (G31 source-unchanged leg): a kill in the post-sync-pre-rename window NEVER
    // touches a PRE-EXISTING file at the target name — the crash lands before the (no-clobber) rename is even
    // attempted, so a same-named original is byte-identical afterwards (never harmed, never clobbered).
    #[test]
    fn a_kill_between_sync_and_rename_never_touches_a_pre_existing_target() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let pre_existing = dir.path().join("data.tsv");
        std::fs::write(&pre_existing, b"PRE-EXISTING must survive the crash")
            .expect("write the original");
        let tmp = dir.path().join(".convertia-out.part");
        std::fs::write(&tmp, b"converted bytes").expect("write the tmp");

        let _kill = kill_after_sync::Armed::arm();
        let candidates = output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem");
        let killed = atomic_publish(
            &parent,
            dir.path(),
            &tmp,
            candidates,
            &dir.path().join(".convertia-xvol.part"),
        );
        assert!(
            killed.is_err(),
            "§2.1.3: the kill returns before any rename is attempted"
        );
        assert_eq!(
            std::fs::read(&pre_existing).expect("read the pre-existing target"),
            b"PRE-EXISTING must survive the crash",
            "§2.1.3 no-harm: a same-named original is byte-identical after a crash before the rename — never touched"
        );
    }

    // §2.1.1 step 3 (G15/G31): sync_tmp_bytes fsyncs a real tmp AND preserves its content byte-for-byte — the
    // `write(true)` (no `truncate`) re-open obtains a flushable handle without altering a byte. Kills a mutant
    // that opened with truncate (which would zero the file the engine just wrote — a silent data-loss bug).
    #[test]
    fn sync_tmp_bytes_fsyncs_and_preserves_content() {
        let dir = tempfile::tempdir().expect("temp dir");
        let tmp = dir.path().join(".convertia-tmp.part");
        std::fs::write(&tmp, b"engine-written bytes").expect("write the tmp");
        sync_tmp_bytes(&tmp).expect("§2.1.1: fsync a real completed tmp");
        assert_eq!(
            std::fs::read(&tmp).expect("read the tmp back"),
            b"engine-written bytes",
            "§2.1.1: sync_tmp_bytes obtains a flushable handle WITHOUT truncating — content is byte-identical"
        );
    }

    // §2.8/G4/G14 (G15): sync_tmp_bytes on a MISSING tmp is a clean Err (NotFound), never a panic — the re-open
    // fails and is surfaced (this in-core durability step runs outside the §2.12 boundary).
    #[test]
    fn sync_tmp_bytes_on_a_missing_file_is_err() {
        let dir = tempfile::tempdir().expect("temp dir");
        let missing = dir.path().join(".convertia-missing.part");
        let err = sync_tmp_bytes(&missing).expect_err("a missing tmp cannot be fsync'd");
        assert_eq!(
            err.kind(),
            io::ErrorKind::NotFound,
            "§2.8: a missing tmp is a clean NotFound Err, never a panic"
        );
    }

    // §2.1.1 step 6 (G15/G31): fsync_parent_dir succeeds on the real P3.9-verified dir handle — the Unix
    // fsync(dirfd) flushes the directory's dentries; on Windows it is the documented no-op (NTFS journaling).
    // Ok on every shipped platform.
    #[test]
    fn fsync_parent_dir_on_a_verified_dir_is_ok() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        fsync_parent_dir(&parent).expect(
            "§2.1.1: fsync the verified parent dir handle (Unix real fsync / Windows no-op)",
        );
    }

    // §2.1.1/§2.8 (G15/G31): the durability sequence is ORDERED sync-BEFORE-publish — a missing tmp fails at the
    // step-3 sync_all with a clean Io(NotFound), and NOTHING is published (no leaf created, the loop never ran).
    // Proves step 3 gates the publish (a non-durable/absent source never reaches the exclusive create).
    #[test]
    fn atomic_publish_on_a_missing_tmp_fails_at_the_sync_step() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let missing_tmp = dir.path().join(".convertia-does-not-exist.part");
        let candidates = output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem");
        let err = atomic_publish(
            &parent,
            dir.path(),
            &missing_tmp,
            candidates,
            &dir.path().join(".convertia-xvol.part"),
        )
        .expect_err("a missing tmp fails at the step-3 sync, before any publish");
        assert!(
            matches!(err, PublishError::Io(_)),
            "§2.8: a missing tmp surfaces as PublishError::Io (the step-3 sync failure)"
        );
        if let PublishError::Io(e) = &err {
            assert_eq!(
                e.kind(),
                io::ErrorKind::NotFound,
                "§2.8: the step-3 sync failure is surfaced faithfully (NotFound)"
            );
        }
        assert!(
            !dir.path().join("data.tsv").exists(),
            "§2.1.1: sync runs BEFORE publish — a failed sync publishes NOTHING (no leaf created)"
        );
    }

    // §2.14.3 (G15): the cross-device classifier recognises the per-OS cross-volume errno (Unix `EXDEV` = 18 /
    // Windows `ERROR_NOT_SAME_DEVICE` = 17) and NOTHING else — so ONLY a real cross-device publish failure
    // triggers the §2.14.3 copy fallback, and a genuine write error (permission, NotFound) surfaces unchanged
    // as §2.8. Kills a mutant that classified every `io::Error` as cross-volume (which would copy-fallback on a
    // permission error) or none (which would never take the fallback).
    #[test]
    fn is_cross_device_recognises_only_the_cross_volume_errno() {
        #[cfg(unix)]
        let cross = io::Error::from_raw_os_error(18); // POSIX EXDEV (Linux + macOS)
        #[cfg(windows)]
        let cross = io::Error::from_raw_os_error(17); // ERROR_NOT_SAME_DEVICE
        assert!(
            is_cross_device(&cross),
            "§2.14.3: the per-OS cross-device errno is classified as cross-volume"
        );
        assert!(
            !is_cross_device(&io::Error::from(io::ErrorKind::PermissionDenied)),
            "§2.14.3: a permission error is a real §2.8 write failure, never the copy-fallback trigger"
        );
        assert!(
            !is_cross_device(&io::Error::from(io::ErrorKind::NotFound)),
            "§2.14.3: a NotFound (no raw_os_error) is not a cross-device failure"
        );
    }

    // §2.14.3 (G15/G31): the cross-volume fallback core copies the (simulated cross-volume) tmp into the
    // same-volume intermediate, publishes THAT to the fresh base name byte-exact, CONSUMES the intermediate, and
    // leaves the SOURCE tmp UNTOUCHED (copied, not moved — copy-exactly-once; the source stays on its own volume
    // for §2.6 cleanup). `avail_bytes` is injected ABOVE the need so the free-space gate passes (the
    // `publish_numbered_capped` injectable-value idiom; the real temp FS + real publish primitive still run).
    #[test]
    fn cross_volume_publishes_a_copy_and_leaves_the_source() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let src = dir.path().join(".convertia-src-xvol.part");
        std::fs::write(&src, b"cross-volume bytes").expect("write the (cross-volume) source tmp");
        let intermediate = dir.path().join(".convertia-intermediate.part");
        let candidates = output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem");
        let outcome = publish_cross_volume_checked(
            &parent,
            dir.path(),
            &src,
            candidates,
            &intermediate,
            u64::MAX, // free space far above the need → the §2.14.3 (c) gate passes
        )
        .expect("§2.14.3: the cross-volume fallback publishes");
        assert_eq!(
            outcome,
            PublishOutcome::Published {
                leaf: OsString::from("data.tsv"),
                residual_tmp: false,
            },
            "§2.14.3: the copied intermediate publishes at the fresh base name"
        );
        assert_eq!(
            std::fs::read(dir.path().join("data.tsv")).expect("read the published file"),
            b"cross-volume bytes",
            "§2.14.3: the published file carries the source tmp's exact bytes (copied across the volume once)"
        );
        assert!(
            !intermediate.exists(),
            "§2.14.3: the same-volume intermediate was consumed by the exclusive rename"
        );
        assert_eq!(
            std::fs::read(&src).expect("read the source tmp"),
            b"cross-volume bytes",
            "§2.14.3 copy-exactly-once: the source tmp is COPIED not moved — left untouched for §2.6 cleanup"
        );
    }

    // §2.14.3/§2.2.2 (G15/G31): a taken base name numbers away on the same-volume intermediate — the COPY
    // happens ONCE (outside the numbering loop), then the cheap intra-volume rename re-targets the SAME
    // intermediate to data (1).tsv. The pre-existing file is byte-identical (no-harm) and the source tmp is
    // untouched — proving the expensive cross-volume copy does not re-run per numbering attempt (§2.14.3).
    #[test]
    fn cross_volume_numbers_away_copying_once_and_never_clobbers() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let taken = dir.path().join("data.tsv");
        std::fs::write(&taken, b"PRE-EXISTING must survive").expect("write the taken base name");
        let src = dir.path().join(".convertia-src-xvol.part");
        std::fs::write(&src, b"new xvol bytes").expect("write the (cross-volume) source tmp");
        let intermediate = dir.path().join(".convertia-intermediate.part");
        let candidates = output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem");
        let outcome = publish_cross_volume_checked(
            &parent,
            dir.path(),
            &src,
            candidates,
            &intermediate,
            u64::MAX,
        )
        .expect("publish");
        assert_eq!(
            outcome,
            PublishOutcome::Published {
                leaf: OsString::from("data (1).tsv"),
                residual_tmp: false,
            },
            "§2.14.3/§2.2.2: the fallback numbers away from the taken base to the first free stem (1).ext"
        );
        assert_eq!(
            std::fs::read(&taken).expect("read the pre-existing base name"),
            b"PRE-EXISTING must survive",
            "§2.2.2 no-harm: the pre-existing file is byte-identical — the cross-volume publish NEVER clobbered it"
        );
        assert_eq!(
            std::fs::read(dir.path().join("data (1).tsv")).expect("read the numbered output"),
            b"new xvol bytes",
            "§2.14.3: the numbered output carries the copied bytes"
        );
        assert_eq!(
            std::fs::read(&src).expect("read the source tmp"),
            b"new xvol bytes",
            "§2.14.3 copy-exactly-once: the source is untouched even when the publish numbers away"
        );
    }

    // §2.14.3 step (c) / §2.8 (G15/G31): the pre-copy free-space re-check FAILS OutOfDisk when `final`'s volume
    // can't host the ~output-sized intermediate — BEFORE writing a byte (`avail_bytes` injected BELOW the need).
    // No intermediate is created, nothing is published, and the source tmp is untouched — "never assume it fits"
    // (§2.7.2), fail clearly. Kills a mutant that copied first and space-checked second (a wasted ~output write).
    #[test]
    fn cross_volume_out_of_disk_fails_before_the_copy() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let src = dir.path().join(".convertia-src-xvol.part");
        std::fs::write(&src, b"a non-empty output that will not fit")
            .expect("write the source tmp");
        let intermediate = dir.path().join(".convertia-intermediate.part");
        let candidates = output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem");
        let err = publish_cross_volume_checked(
            &parent,
            dir.path(),
            &src,
            candidates,
            &intermediate,
            0, // zero free space on `final`'s volume → the intermediate cannot fit
        )
        .expect_err("§2.14.3: the pre-copy free-space re-check fails OutOfDisk when it won't fit");
        assert!(
            matches!(err, PublishError::OutOfDisk),
            "§2.8: the cross-volume free-space re-check maps to OutOfDisk (not a generic Io)"
        );
        assert!(
            !intermediate.exists(),
            "§2.14.3: OutOfDisk fires BEFORE the copy — no intermediate byte was written"
        );
        assert!(
            !dir.path().join("data.tsv").exists(),
            "§2.14.3: nothing was published on the OutOfDisk path"
        );
        assert_eq!(
            std::fs::read(&src).expect("read the source tmp"),
            b"a non-empty output that will not fit",
            "§2.14.3: the source tmp is untouched when the free-space gate refuses"
        );
    }

    // §2.2.3/§2.8/§2.14.3 (G15): a §2.2.3 path-limit breach from the direct publish flows through
    // `atomic_publish`'s non-cross-device Err arm UNCHANGED — the §2.14.3 fallback fires ONLY on a real
    // cross-device failure, NEVER on PathTooLong (or any other §2.8 write error), and it never creates the
    // cross-volume intermediate on such a path. (The cross-device routing arm itself needs a genuine 2-volume
    // setup; it is driven on a real volume boundary by the `real_cross_volume_tests` module below, P3.65.)
    #[test]
    fn atomic_publish_passes_through_a_non_crossdevice_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let tmp = dir.path().join(".convertia-tmp.part");
        std::fs::write(&tmp, b"bytes").expect("write the tmp");
        // A 300-char stem → the base candidate breaches the 255-unit per-component limit (§2.2.3).
        let source = format!("{}.csv", "a".repeat(300));
        let intermediate = dir.path().join(".convertia-xvol.part");
        let candidates = output_name(Path::new(&source), "tsv").expect("a real path has a stem");
        let err = atomic_publish(&parent, dir.path(), &tmp, candidates, &intermediate)
            .expect_err("a §2.2.3 path-limit breach is a hard failure");
        assert!(
            matches!(err, PublishError::PathTooLong(_)),
            "§2.8: a §2.2.3 path-limit breach flows through atomic_publish unchanged (not swallowed by the EXDEV arm)"
        );
        assert!(
            !intermediate.exists(),
            "§2.14.3: the cross-volume intermediate is never created on a non-cross-device error"
        );
    }

    // §2.1.2/§2.7.2 (G15/G31, P3.65): a FAT/exFAT-class destination is a VERDICT, not an error — the composite
    // reports `NoAtomicPublishSupport` (the §2.7.2 divert signal) while creating NO `final`, leaving the
    // completed tmp intact for the divert to re-publish, and creating no cross-volume intermediate. The
    // §2.1.1 step-6 dir-fsync is correctly skipped (no dentry was made). Its tier-1 consumer — the reactive
    // late-divert — is `crate::orchestrator::cross_volume_e2e_tests`.
    //
    // HONEST READING OF WHAT EACH ASSERTION BUYS: the fence returns BEFORE `publish_once`, so "no `final`",
    // "tmp intact" and "no intermediate" are fence-IMPLIED — they pin that the fence models a real FAT
    // destination faithfully (on vfat the chain refuses with no FS side effect either), not a §2.1.2 property.
    // The LOAD-BEARING assertion is the first one: `atomic_publish` PROPAGATES the leaf verdict as an `Ok`
    // outcome rather than an `Err`, and (per its own contract) skips the step-6 dir-fsync on it — production
    // logic the §2.7.2 divert routing depends on. [Build-Session-Entscheidung: P3.65]
    #[test]
    fn a_fat_class_destination_reports_no_atomic_publish_support_and_creates_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let parent = verified(dir.path());
        let tmp = dir.path().join(".convertia-out.part");
        std::fs::write(&tmp, b"converted bytes").expect("write the tmp");
        let intermediate = dir.path().join(".convertia-xvol.part");
        let candidates = output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem");

        let _fat = fat_class_destination::Armed::arm(dir.path()); // RAII: disarmed on scope exit
        let outcome = atomic_publish(&parent, dir.path(), &tmp, candidates, &intermediate)
            .expect("§2.7.2: an atomic-publish-incapable destination is a verdict, never an Err");

        assert_eq!(
            outcome,
            PublishOutcome::NoAtomicPublishSupport,
            "§2.1.2 third fallback: neither the no-replace rename nor `link` can publish here"
        );
        assert!(
            !dir.path().join("data.tsv").exists(),
            "§2.7.2: nothing is published on an atomic-publish-incapable destination"
        );
        assert_eq!(
            std::fs::read(&tmp).expect("read the tmp"),
            b"converted bytes",
            "§2.7.2: the completed tmp is intact — the §2.7.3 divert re-publishes THIS output elsewhere"
        );
        assert!(
            !intermediate.exists(),
            "§2.14.3: no cross-volume intermediate is created on the divert-signal path"
        );
    }

    // §2.7.3 (G15, P3.65) THE FENCE IS DIRECTORY-SCOPED — the premise the whole reactive-divert case rests on:
    // an armed destination refuses, while a SIBLING directory (standing in for the §2.7.3 divert target on a
    // hardlink-capable volume) publishes for real in the same call scope. A global switch would make the
    // divert target refuse too, so the divert could never complete and the case would prove nothing.
    #[test]
    fn the_fat_class_fence_is_directory_scoped_so_a_sibling_destination_still_publishes() {
        let base = tempfile::tempdir().expect("temp dir");
        let fat_dir = base.path().join("fat-class");
        let ok_dir = base.path().join("hardlink-capable");
        std::fs::create_dir(&fat_dir).expect("create the armed dir");
        std::fs::create_dir(&ok_dir).expect("create the sibling dir");
        let tmp = ok_dir.join(".convertia-out.part");
        std::fs::write(&tmp, b"converted bytes").expect("write the tmp");

        let _fat = fat_class_destination::Armed::arm(&fat_dir);
        assert!(
            fat_class_destination::armed_for(&fat_dir.join("nested")),
            "the fence covers the armed dir and everything beneath it"
        );
        assert!(
            !fat_class_destination::armed_for(&ok_dir),
            "the fence does NOT cover a sibling dir — the §2.7.3 divert target keeps publishing for real"
        );
        let outcome = atomic_publish(
            &verified(&ok_dir),
            &ok_dir,
            &tmp,
            output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem"),
            &ok_dir.join(".convertia-xvol.part"),
        )
        .expect("the un-armed sibling publishes");
        assert_eq!(
            outcome,
            PublishOutcome::Published {
                leaf: OsString::from("data.tsv"),
                residual_tmp: false,
            },
            "§2.7.3: the divert target publishes normally while another dir is atomic-publish-incapable"
        );
    }
}

// §6.4.3 real-FS integration (G15/G31) for the §2.14.3 EXDEV path driven over a GENUINE VOLUME BOUNDARY, and
// the §2.1.3 two-state invariant proven with the temp on a real second volume (P3.65).
//
// The `atomic_publish_tests` above reach the §2.14.3 fallback CORE directly (`publish_cross_volume_checked`
// with an injected `avail_bytes` — the `publish_numbered_capped` injectable-value idiom, real FS + real
// publish primitive, nothing mocked). What they structurally cannot reach is the ROUTING arm that gets there:
// `atomic_publish`'s `Err(Io(e)) if is_cross_device(&e)` match, which fires only when the OS itself refuses
// the rename `EXDEV`/`ERROR_NOT_SAME_DEVICE`. That needs two real volumes, which no in-process call can
// create — mounting one is privileged and out-of-process, and `crate::isolation` is the sole sanctioned
// `process::Command::new` site (G9 invariant (b) / G29), so these tests DISCOVER a second volume the host
// already offers ([`crate::test_volumes`]) and take a clean, commented skip where there is none (the P3.6
// guarded-early-`return` precedent — never a vacuous pass, never `#[ignore]`). TWO STACKED cfg attrs (the
// P1.17 compound-cfg trap). [Build-Session-Entscheidung: P3.65]
#[cfg(test)]
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod real_cross_volume_tests {
    use super::*;

    /// The P3.9 verified parent handle for `dir` (empty frozen set → always Verified).
    fn verified(dir: &Path) -> VerifiedParentDir {
        match open_verified_parent_dir(dir, &[]).expect("open the dest dir") {
            ParentDirVerdict::Verified(v) => Some(v),
            ParentDirVerdict::ResolvesOntoSource => None,
        }
        .expect("a real dir with an empty frozen set verifies")
    }

    /// A `(destination, other-volume)` pair whose two dirs sit on GENUINELY different volumes — the §2.14.3
    /// substrate. `None` = this host mounts no second volume, so the caller skips.
    fn volume_pair() -> Option<(tempfile::TempDir, tempfile::TempDir)> {
        let dest = tempfile::tempdir().expect("temp dir");
        let other = crate::test_volumes::second_volume_dir(dest.path())?;
        Some((dest, other))
    }

    /// The §2.2.1 candidate names for `data.csv` → `.tsv` (a fresh iterator per publish attempt).
    fn data_tsv_candidates() -> OutputNameCandidates {
        output_name(Path::new("data.csv"), "tsv").expect("a real path has a stem")
    }

    // §2.14.3 (G15/G31) THE CROSS-DEVICE ROUTING ARM ON A REAL VOLUME BOUNDARY: a `tmp` sitting on a different
    // volume than `final` makes the DIRECT publish fail cross-device, `atomic_publish` classifies it
    // (`is_cross_device`), reads `final`'s volume free space, and hands off to the copy fallback — which
    // publishes a same-volume COPY at the fresh base name while LEAVING the cross-volume `tmp` untouched
    // (copy-exactly-once; the source stays on its own volume for the §2.6 run-scope cleanup) and CONSUMING the
    // same-volume intermediate. This is the arm no injected value can reach.
    #[test]
    fn a_real_cross_device_publish_copies_once_and_leaves_the_cross_volume_tmp() {
        let Some((dest, other)) = volume_pair() else {
            // This host mounts no second volume, so the OS never returns EXDEV and the routing arm cannot be
            // driven here. The fallback CORE it routes into (copy-exactly-once, the free-space gate, the
            // numbering loop) stays covered on every platform by `atomic_publish_tests` above.
            return;
        };
        let parent = verified(dest.path());
        let tmp = other.path().join(".convertia-out.part");
        std::fs::write(&tmp, b"cross-volume bytes").expect("write the tmp on the other volume");
        let intermediate = dest.path().join(".convertia-xvol.part");

        let outcome = atomic_publish(
            &parent,
            dest.path(),
            &tmp,
            data_tsv_candidates(),
            &intermediate,
        )
        .expect("§2.14.3: the cross-device failure routes into the copy fallback and publishes");

        assert_eq!(
            outcome,
            PublishOutcome::Published {
                leaf: OsString::from("data.tsv"),
                residual_tmp: false,
            },
            "§2.14.3: the cross-volume publish lands at the fresh base name (callers see the same outcome as a direct publish)"
        );
        assert_eq!(
            std::fs::read(dest.path().join("data.tsv")).expect("read the published file"),
            b"cross-volume bytes",
            "§2.14.3: the published file carries the cross-volume tmp's exact bytes"
        );
        assert_eq!(
            std::fs::read(&tmp).expect("read the cross-volume tmp"),
            b"cross-volume bytes",
            "§2.14.3 copy-exactly-once: the cross-volume tmp is COPIED not moved — left on its own volume for the §2.6 cleanup"
        );
        assert!(
            !intermediate.exists(),
            "§2.14.3: the same-volume intermediate was consumed by the intra-volume exclusive rename"
        );
    }

    // §2.14.3/§2.2.2 (G15/G31) COPY-EXACTLY-ONCE UNDER A REAL NUMBERING COLLISION: with the base name taken,
    // the fallback re-renames the SAME already-copied same-volume intermediate to `data (1).tsv` — the
    // expensive cross-volume copy does NOT re-run per attempt. The pre-existing file is byte-identical
    // (no-harm through the fallback) and the cross-volume tmp is still untouched.
    #[test]
    fn a_real_cross_device_publish_numbers_away_without_recopying_or_clobbering() {
        let Some((dest, other)) = volume_pair() else {
            return; // no second volume on this host (see the sibling test's note).
        };
        let parent = verified(dest.path());
        let taken = dest.path().join("data.tsv");
        std::fs::write(&taken, b"PRE-EXISTING must survive").expect("seed the taken base name");
        let tmp = other.path().join(".convertia-out.part");
        std::fs::write(&tmp, b"new xvol bytes").expect("write the tmp on the other volume");
        let intermediate = dest.path().join(".convertia-xvol.part");

        let outcome = atomic_publish(
            &parent,
            dest.path(),
            &tmp,
            data_tsv_candidates(),
            &intermediate,
        )
        .expect("publish");

        assert_eq!(
            outcome,
            PublishOutcome::Published {
                leaf: OsString::from("data (1).tsv"),
                residual_tmp: false,
            },
            "§2.14.3/§2.2.2: the cross-volume publish numbers away to the first free stem (1).ext"
        );
        assert_eq!(
            std::fs::read(&taken).expect("read the pre-existing base name"),
            b"PRE-EXISTING must survive",
            "§2.2.2 no-harm: the pre-existing file is byte-identical — the cross-volume publish NEVER clobbered it"
        );
        assert_eq!(
            std::fs::read(dest.path().join("data (1).tsv")).expect("read the numbered output"),
            b"new xvol bytes",
            "§2.14.3: the numbered output carries the copied bytes"
        );
        assert_eq!(
            std::fs::read(&tmp).expect("read the cross-volume tmp"),
            b"new xvol bytes",
            "§2.14.3 copy-exactly-once: the numbering retry re-renamed the intermediate — the cross-volume source was not re-copied"
        );
    }

    // §2.14.3 step (c) / §2.8 (G15/G31) THE FREE-SPACE RE-CHECK FIRING ON THE LIVE PATH: with `final`'s volume
    // unable to host a second copy of the output, `atomic_publish`'s live
    // `crate::platform::available_bytes` read feeds the pre-copy gate, which fails `OutOfDisk` BEFORE writing a
    // byte — no intermediate, nothing published, the source untouched ("never assume it fits", §2.7.2).
    //
    // The ORIENTATION is deliberately flipped versus the sibling tests (`final` on the discovered second
    // volume, the tmp on the anchor volume): the gate needs a destination whose free space a SPARSE
    // pre-sized file can exceed cheaply, and the discovered volume is the smaller of the two on a stock host.
    // Unix-gated because `set_len` is a sparse expansion on the Unix filesystems here, whereas a Windows
    // `SetEndOfFile` allocates — so a Windows run would either really consume the space or fail the `set_len`;
    // BOTH sides of this gate are proven deterministically on EVERY platform by
    // `atomic_publish_tests`' injected-`avail_bytes` pair (one case each side of the gate), so what
    // is Unix-only here is the LIVE `available_bytes`-fed firing, not the gate's coverage.
    // [Build-Session-Entscheidung: P3.65]
    #[cfg(unix)]
    #[test]
    fn a_real_cross_device_out_of_disk_fires_before_the_copy() {
        let Some((anchor, other)) = volume_pair() else {
            return; // no second volume on this host (see the first test's note).
        };
        let dest = other.path();
        let parent = verified(dest);
        let avail = crate::platform::available_bytes(dest)
            .expect("read the destination volume's free space");
        let tmp = anchor.path().join(".convertia-oversized.part");
        let file = std::fs::File::create(&tmp).expect("create the oversized tmp");
        // A SPARSE pre-sized file: `metadata().len()` (what the §2.14.3 gate compares) exceeds the
        // destination's free space without any bytes being written. A filesystem that materialises the
        // expansion instead refuses it — skip rather than fill the disk.
        //
        // The MARGIN is deliberately enormous — at least DOUBLE the free space, and never under 1 GiB — not a
        // token overshoot. `atomic_publish` re-reads `available_bytes` independently, and `/dev/shm`-class
        // destinations breathe with every process that maps or releases shared memory: if free space grew past
        // `need` between the two reads, the gate would NOT fire and the fallback would copy an ~`avail`-sized
        // file into the destination volume until `ENOSPC` — a host-harming outcome, not a mere flake. A gap
        // this size cannot be closed by ordinary drift. [Build-Session-Entscheidung: P3.65]
        let need = avail.saturating_mul(2).max(1 << 30);
        if file.set_len(need).is_err() {
            return;
        }
        drop(file);
        let intermediate = dest.join(".convertia-xvol.part");

        let err = atomic_publish(&parent, dest, &tmp, data_tsv_candidates(), &intermediate)
            .expect_err("§2.14.3: the live free-space re-check refuses a copy that cannot fit");

        assert!(
            matches!(err, PublishError::OutOfDisk),
            "§2.8: the live cross-volume free-space re-check maps to OutOfDisk (not a generic Io), got {err:?}"
        );
        assert!(
            !intermediate.exists(),
            "§2.14.3: OutOfDisk fires BEFORE the copy — no intermediate byte was written"
        );
        assert!(
            !dest.join("data.tsv").exists(),
            "§2.14.3: nothing was published on the OutOfDisk path"
        );
        assert!(
            tmp.exists(),
            "§2.14.3: the source tmp is untouched when the free-space gate refuses"
        );
    }

    // §2.1.3 (G15/G31) THE CRASH / POWER-LOSS TWO-STATE INVARIANT ACROSS A REAL VOLUME BOUNDARY: the sibling
    // `atomic_publish_tests` proof runs with `tmp` and `final` on ONE volume; this one arms the same P3.19.1
    // kill fence with the durable `tmp` on a REAL SECOND VOLUME, so §2.1.3's closing clause ("the only rename
    // is still intra-volume and exclusive, so the same two-state invariant holds") is proven where it is
    // actually claimed. State-2: no `final`, the durable `*.part` remains on ITS OWN volume. State-1 (fence
    // disarmed): `final` springs into existence complete via the §2.14.3 copy fallback, with the cross-volume
    // `tmp` still present (copied, not moved) — the third §2.1.3 state (a truncated/0-byte `final`) never
    // exists on either side.
    #[test]
    fn the_two_state_invariant_holds_with_the_tmp_on_a_real_second_volume() {
        let Some((dest, other)) = volume_pair() else {
            return; // no second volume on this host (see the first test's note).
        };
        let parent = verified(dest.path());
        let tmp = other.path().join(".convertia-out.part");
        std::fs::write(&tmp, b"converted bytes").expect("write the tmp on the other volume");
        let final_name = dest.path().join("data.tsv");
        let intermediate = dest.path().join(".convertia-xvol.part");

        // ── §2.1.3 STATE-2: a crash in the post-sync-pre-rename window, tmp on the other volume ──
        {
            let _kill = kill_after_sync::Armed::arm(); // RAII: disarmed on scope exit (even on a panic)
            let killed = atomic_publish(
                &parent,
                dest.path(),
                &tmp,
                data_tsv_candidates(),
                &intermediate,
            );
            assert!(
                killed.is_err(),
                "§2.1.3: the armed fence returns in the post-sync-pre-rename window (nothing published)"
            );
        }
        assert!(
            !final_name.exists(),
            "§2.1.3 state-2: after a kill before the rename NO `final` exists — never a truncated/0-byte final"
        );
        assert!(
            !intermediate.exists(),
            "§2.1.3 state-2: the kill precedes the cross-volume copy — no intermediate was created either"
        );
        assert_eq!(
            std::fs::read(&tmp).expect("read the tmp"),
            b"converted bytes",
            "§2.1.3 state-2: the durable *.part remains byte-identical ON ITS OWN VOLUME (unpublished, discardable)"
        );

        // ── §2.1.3 STATE-1: the un-armed publish completes atomically via the §2.14.3 fallback ──
        let outcome = atomic_publish(
            &parent,
            dest.path(),
            &tmp,
            data_tsv_candidates(),
            &intermediate,
        )
        .expect("§2.1.3: the un-armed cross-volume publish completes");
        assert!(
            matches!(outcome, PublishOutcome::Published { .. }),
            "§2.1.3 state-1: with the fence disarmed the cross-volume publish reaches state-1"
        );
        assert_eq!(
            std::fs::read(&final_name).expect("read the final"),
            b"converted bytes",
            "§2.1.3 state-1: `final` sprang into existence COMPLETE — the two-state invariant holds across the volume boundary"
        );
        assert!(
            tmp.exists(),
            "§2.14.3: the cross-volume tmp survives the successful publish (copied, not moved)"
        );
    }
}
