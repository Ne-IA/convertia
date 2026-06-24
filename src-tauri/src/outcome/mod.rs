//! `crate::outcome` — the single home of the §2.8 conversion-outcome taxonomy + message catalog and
//! the §2.9 lossy-disclosure catalog, mirrored onto the IPC wire as the §0.4.3 `IpcError` / `ErrorKind`
//! (the §0.7 tier module renamed from `error`; there is no `crate::error`).
//!
//! P1 established the module so the §0.7 tree compiles and the §06 drift mechanism has a home. P2.18
//! authors the §2.8.1 `ConversionErrorKind` taxonomy + its §0.4.3 `ErrorKind` wire alias here; the
//! `IpcError` shape (P2.19), the `OutcomeMsg` surfaced-string type (P2.20), and the §2.8.2 message
//! catalog land in their own P2 boxes (§02 owns the outcome strings).

// [Build-Session-Entscheidung: P2.18] The §2.8 taxonomy is forward-declared here before its P2.19/P2.20
// IPC consumers (`IpcError`/`OutcomeMsg`) register it, so `ConversionErrorKind` is dead in the PRODUCTION
// build until consumed; the cfg(test) anti-drift tests reference it, so the TEST build is dead-code-clean.
// `expect` (not `allow`) auto-flags the moment the taxonomy becomes consumed — matching `crate::domain`.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the §2.8.1 ConversionErrorKind taxonomy + the §0.4.3 ErrorKind alias are forward-declared before their P2.19/P2.20 IPC consumers register them, so they are dead in the production build until consumed."
    )
)]

use serde::Serialize;
use specta::Type;

// [Build-Session-Entscheidung: P2.18] Derive set: §0.4.3/§2.8.1 show the WIRE-required `Serialize` +
// `specta::Type` (so the kind mirrors to `bindings.ts` rather than `any`); `Debug, Clone, Copy, PartialEq,
// Eq` are added for testing + ergonomics, consistent with every fieldless §0.6 enum (e.g. `DivertReason`).
// NO `Deserialize`: `ErrorKind`/`IpcError` are OUTBOUND only (an `Err`/`ItemOutcome::Failed.error` return,
// §0.4.3), never deserialized from the WebView — so the kind has no inbound path. Registration in
// `collect_types![]` is DEFERRED to the consuming `IpcError`/`OutcomeMsg` (P2.19/P2.20), the established
// P2.2-P2.9 defer pattern (the no-`any` guarantee is the `Type` derive, not early registration).

/// The §2.8.1 conversion-outcome taxonomy — the single owner of the failure-kind set (§2.8 owns the set +
/// their §2.8.2 strings; §0.4 owns the wire shape). Every engine / FS / detection failure maps to exactly
/// one variant — there is no "other/unknown" that leaks a raw error (an unmapped fault becomes
/// `InternalError`, §2.13). The §0.4.3 `ErrorKind` is its byte-identical wire mirror; see the `ErrorKind`
/// alias below (§2.8.2 option 1: one enum, nothing to drift). Outbound-only (no `Deserialize`, see note above).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ConversionErrorKind {
    // ── item-level (one source file failed; the batch continues, §1.9) ──
    /// Decoded but structurally invalid / truncated mid-stream.
    Corrupt,
    /// 0-byte or no decodable content.
    Empty,
    /// Detection cannot identify the type at all (§1.2 uncertain/conflicting).
    Unrecognized,
    /// Recognised but not an in-scope source (§1.2 "detected: X").
    UnsupportedType,
    /// In-scope source, but the target is not offered (defensive; the UI prevents it).
    UnsupportedPair,
    /// Present at freeze, now unreadable: permission denied / exclusive lock.
    Unreadable,
    /// Present at freeze, now missing: moved / deleted / removed media.
    Gone,
    /// Encrypted / DRM source (PDF password, FairPlay, PlaysForSure) — ConvertIA never prompts/cracks.
    PasswordProtected,
    /// Extract-audio asked of a source with no audio stream (cross-category.md / audio.md).
    NoAudioTrack,
    /// Exceeds the §1.10 "too big" ceiling (pre-flight or mid-run).
    TooBig,
    /// `ENOSPC` while writing (§2.6 cleans the partial).
    OutOfDisk,
    /// The output write/publish failed for a non-space reason (permission / IO at the destination, §2.1/§2.7).
    WriteFailed,
    /// §2.2.3 — the name/extension would exceed the OS path limit (never truncated).
    PathTooLong,
    /// §2.1.2/§2.2 — the ~10,000-variant no-clobber cap was exhausted (a degenerate directory).
    TooManyCollisions,
    /// Subprocess killed by signal / nonzero abnormal exit (§1.7/§2.12).
    EngineCrash,
    /// Exceeded the §1.7 timeout, killed (§2.12).
    EngineHang,
    /// Subprocess clean nonzero exit with classifiable stderr (§3.5).
    EngineError,
    /// Patent-gapped on this platform (§3.4) — honest "unavailable here".
    PlatformUnavailable,
    /// macOS Gatekeeper quarantined a bundled engine sidecar so it can't spawn (§7.2.3) — distinct from
    /// `EngineMissing`/`BundleDamaged`.
    QuarantinedByOs,
    /// The item failed AND its partial couldn't be removed (§2.6.4) — the only kind that names a residue path.
    CleanupResidue,
    /// Catch-all for an unexpected internal fault (§2.13); no trace shown.
    InternalError,
    // ── run/app-level (§2.13); surfaced via `app://fault`, not a per-item row ──
    /// A required bundled engine is absent / unrunnable at startup (§7.2).
    EngineMissing,
    /// The WebView core disconnected / failed to load (§2.13/§5.8).
    WebviewFault,
    /// The app bundle / resources failed their integrity check (§7.2).
    BundleDamaged,
    // ── pre-flight (NOT carried as an IpcError; mirror-only for drift-lock) ──
    /// More than one source format in one drop — the §1.3 pre-flight refusal. Has NO IpcError producer:
    /// it is the `CollectedSet::Mixed` SUCCESS return from C1 (§0.6) driving the §5.2 state-9 refusal.
    /// Listed here ONLY to keep the enum byte-identical to the §0.4.3 wire mirror (no §2.13 producer).
    MixedDrop,
}

/// The §0.4.3 wire mirror of the §2.8.1 taxonomy. [Build-Session-Entscheidung: P2.18] §2.8.2 option 1
/// (the PREFERRED mechanism): `ErrorKind` is a `type` alias for `ConversionErrorKind` — one enum, no
/// second list to drift, the wire mirror IS the same type. The distinct-enum + `static_assertions` path
/// (§2.8.2 option 2) is only needed when the wire enum must OMIT an internal-only variant; here the wire
/// enum carries every variant (incl. the run/app-level kinds + `MixedDrop`), so the alias is both viable
/// and strictly simpler — no `static_assertions` dependency. The remaining enum↔§2.8-catalog drift is
/// locked by the anti-drift test below (exhaustive-match variant-count + per-variant wire-name pins).
pub type ErrorKind = ConversionErrorKind;

#[cfg(test)]
mod tests {
    use super::*;

    // §6.4.1 unit (G15/G23): the §2.8.1 ↔ §0.4.3 byte-identical wire mirror (P2.18.3 anti-drift). Pins
    // every variant's exact camelCase wire string (a renamed/added/removed variant changes a pin) AND the
    // total count == 25 (21 item-level + 3 run/app-level + MixedDrop). The companion exhaustive match
    // (`conversion_error_kind_exhaustive`) is the COMPILE-TIME half: a variant added without a row in this
    // array fails to compile there, so the array can never silently fall behind the enum. ErrorKind/
    // ConversionErrorKind are outbound-only (no Deserialize), so this is a serialize pin, not a round-trip.
    #[test]
    fn conversion_error_kind_wire_names_byte_identical_to_catalog() {
        let all: [(ConversionErrorKind, &str); 25] = [
            (ConversionErrorKind::Corrupt, "corrupt"),
            (ConversionErrorKind::Empty, "empty"),
            (ConversionErrorKind::Unrecognized, "unrecognized"),
            (ConversionErrorKind::UnsupportedType, "unsupportedType"),
            (ConversionErrorKind::UnsupportedPair, "unsupportedPair"),
            (ConversionErrorKind::Unreadable, "unreadable"),
            (ConversionErrorKind::Gone, "gone"),
            (ConversionErrorKind::PasswordProtected, "passwordProtected"),
            (ConversionErrorKind::NoAudioTrack, "noAudioTrack"),
            (ConversionErrorKind::TooBig, "tooBig"),
            (ConversionErrorKind::OutOfDisk, "outOfDisk"),
            (ConversionErrorKind::WriteFailed, "writeFailed"),
            (ConversionErrorKind::PathTooLong, "pathTooLong"),
            (ConversionErrorKind::TooManyCollisions, "tooManyCollisions"),
            (ConversionErrorKind::EngineCrash, "engineCrash"),
            (ConversionErrorKind::EngineHang, "engineHang"),
            (ConversionErrorKind::EngineError, "engineError"),
            (
                ConversionErrorKind::PlatformUnavailable,
                "platformUnavailable",
            ),
            (ConversionErrorKind::QuarantinedByOs, "quarantinedByOs"),
            (ConversionErrorKind::CleanupResidue, "cleanupResidue"),
            (ConversionErrorKind::InternalError, "internalError"),
            (ConversionErrorKind::EngineMissing, "engineMissing"),
            (ConversionErrorKind::WebviewFault, "webviewFault"),
            (ConversionErrorKind::BundleDamaged, "bundleDamaged"),
            (ConversionErrorKind::MixedDrop, "mixedDrop"),
        ];
        assert_eq!(
            all.len(),
            25,
            "§2.8.1: the taxonomy is exactly 25 kinds (21 item-level + 3 run/app-level + MixedDrop)"
        );
        for (kind, wire) in all {
            assert_eq!(
                serde_json::to_string(&kind).expect("ConversionErrorKind serializes"),
                format!("\"{wire}\""),
                "§2.8/§0.4.3: each kind serializes to its byte-identical camelCase wire name"
            );
        }
    }

    // The COMPILE-TIME variant-count lock (the established dependency-free exhaustive-match pattern, cf.
    // `crate::domain`'s `*_exhaustive` helpers). Adding or removing a `ConversionErrorKind` variant without
    // updating this match fails to compile — so the wire-name array above can never silently drift from the
    // enum. (§2.8.2 option 1 means there is ONE enum; this guards it against the §2.8.1/§0.4.3 catalog.)
    fn conversion_error_kind_exhaustive(k: &ConversionErrorKind) {
        match k {
            ConversionErrorKind::Corrupt
            | ConversionErrorKind::Empty
            | ConversionErrorKind::Unrecognized
            | ConversionErrorKind::UnsupportedType
            | ConversionErrorKind::UnsupportedPair
            | ConversionErrorKind::Unreadable
            | ConversionErrorKind::Gone
            | ConversionErrorKind::PasswordProtected
            | ConversionErrorKind::NoAudioTrack
            | ConversionErrorKind::TooBig
            | ConversionErrorKind::OutOfDisk
            | ConversionErrorKind::WriteFailed
            | ConversionErrorKind::PathTooLong
            | ConversionErrorKind::TooManyCollisions
            | ConversionErrorKind::EngineCrash
            | ConversionErrorKind::EngineHang
            | ConversionErrorKind::EngineError
            | ConversionErrorKind::PlatformUnavailable
            | ConversionErrorKind::QuarantinedByOs
            | ConversionErrorKind::CleanupResidue
            | ConversionErrorKind::InternalError
            | ConversionErrorKind::EngineMissing
            | ConversionErrorKind::WebviewFault
            | ConversionErrorKind::BundleDamaged
            | ConversionErrorKind::MixedDrop => {}
        }
    }

    #[test]
    fn conversion_error_kind_exhaustive_match_is_exercised() {
        conversion_error_kind_exhaustive(&ConversionErrorKind::InternalError);
    }

    // §0.4.3/§2.8.2 option 1: `ErrorKind` IS `ConversionErrorKind` (the wire mirror is the same type, so
    // nothing can drift). The `coerce` identity moves an `ErrorKind` into a `ConversionErrorKind` with NO
    // conversion, so it compiles ONLY while the alias holds — a future split into a distinct wire enum fails
    // to compile here, forcing a §2.8.2-conscious decision rather than a silent divergence (the project's
    // "lock the contract" discipline, cf. `crate::domain`'s `jobid_compiles_as_itemid_alias`).
    #[test]
    fn error_kind_is_the_conversion_error_kind_alias() {
        fn coerce(k: ErrorKind) -> ConversionErrorKind {
            k
        }
        assert_eq!(
            coerce(ErrorKind::InternalError),
            ConversionErrorKind::InternalError,
            "§2.8.2: ErrorKind is the ConversionErrorKind alias (the wire mirror is the same type)"
        );
    }
}
