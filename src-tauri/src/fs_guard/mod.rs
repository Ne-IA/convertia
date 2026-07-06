//! `crate::fs_guard` — the §2.0 no-harm kernel: atomic, exclusive, no-clobber publish on the resolved
//! real file, link-safety + resolved-identity, the frozen source set, path-limit handling and the
//! cross-volume strategy (§2.1 / §2.2 / §2.3 / §2.14). Every output flows through here; engines never
//! write the final file. A §0.7 tier-2 trust-kernel LEAF: it depends DOWN only (on `crate::domain`),
//! never up on IPC / orchestrator / the engine registry (§2.0 dependency direction). Unsafe-free — the
//! crate-root `#![deny(unsafe_code)]` (main.rs) covers it; the per-OS handle FFI the identity/publish
//! primitives require is homed in the single allow-listed `crate::platform` shim (§0.7 / P3.6 / P3.9).
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
use std::path::PathBuf;

// [Build-Session-Entscheidung: P2.74] The §2.3.1 resolved-identity TYPE only (option A "split IO-vs-pure",
// Co-Pilot 2026-06-30, owner-ratified): the `resolve_identity` FUNCTION that POPULATES it
// (`std::fs::canonicalize` + `dunce::canonicalize` + the per-OS metadata read = IO/FFI, needs `dunce`) is
// wholly P3 — its contract-map entry at P3.1.1, its body at P3.6 — so there is NO half-built function shell and NO
// tagged-`Err` placeholder here (a placeholder with no honest value is the rejected quiet-stub, CLAUDE §5).
//
// Derive / identity choices (the §0.6 sibling types fix the house style):
//  - Core-INTERNAL: `FileIdentity` never crosses IPC — the §2.3.2 de-dup runs core-side and the wire
//    carries `DroppedItem.resolved_path` (§0.6), not this identity — so it derives NO `serde`/`specta`,
//    only `Debug` + `Clone` (it owns a `PathBuf`, hence NOT `Copy`), the internal-type set
//    `FrozenCollectedSet` / `Batch` use.
//  - `dev_or_volserial` / `inode_or_fileindex` are both `u64`: Unix `st_dev`/`st_ino` are `u64`; Windows
//    `volume_serial_number()` is `u32` (widened) and `file_index()` is already `u64`. One platform-agnostic
//    representation, so the TYPE carries no `cfg` — only its P3 producer reads the per-OS metadata.
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
    /// (`volume_serial_number()`, widened from `u32`). Half of the authoritative (dev, inode) identity —
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
}
