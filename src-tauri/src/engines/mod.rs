//! `crate::engines` — the §3.2 engine registry + `Engine` trait + selection, the §1.7 generic
//! invocation lifecycle (spawn / progress / cancel / timeout / error-map), and the §3.5 per-engine
//! argument construction. Every spawn routes through `crate::isolation` and the §0.9 pool.
//!
//! P2.13 authors the §3.2 engine-seam descriptor TYPES here — `EngineId` / `EngineKind` /
//! `EngineDescriptor` (§0.6) — ahead of the registry / `trait Engine` / selection BEHAVIOUR, which is
//! filled by P4.1. The descriptor types are the seam vocabulary the P4.1 registry + the §0.9 pool + the
//! §7.2 `EngineHealth` contract key on.

// [Build-Session-Entscheidung: P2.13] dead_code expect — the §3.2 seam descriptor types are authored as
// CONTRACTS before their consumers exist: the registry / `trait Engine` / selection is P4.1, the §0.9 pool
// reads `EngineDescriptor.serialised_only` then, and `EngineId`'s wire registration rides the §7.2
// `EngineHealth` (C12) consumer (a later P2 box). So each is dead in the PRODUCTION build until consumed;
// the cfg(test) tests below construct them, so the TEST build is dead-code-clean. `expect` (not `allow`)
// auto-flags the moment a consumer lands — matching `crate::domain`/`crate::outcome`/`crate::orchestrator`.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the §3.2 engine-seam descriptor types EngineId/EngineKind/EngineDescriptor are authored as contracts before the P4.1 registry/trait/selection + the §0.9 pool + the §7.2 EngineHealth (C12) wire consumer construct/register them, so they are dead in the production build until consumed."
    )
)]

use serde::Serialize;
use specta::Type;

/// The stable engine discriminant (§0.6 / §3.2) — used in logging / SBOM rows (§3.7), the §3.2.3
/// `(SourceFmt,TargetFmt) → EngineId` registry, the §0.9 pool's `HashMap<EngineId, bool>` serialised-flag
/// map, and the §7.2 `EngineHealth` presence-check. One variant per bundled engine; Ghostscript is NOT
/// shipped v1 (§3.1).
///
/// **Two variants are NON-TRAIT** (no `EngineProgram`, no §3.2.3 registry entry, no `trait Engine` impl) —
/// they exist as an `EngineId` ONLY for SBOM/NOTICE attribution (§3.7), the §7.2 `EngineHealth` presence
/// check, and (for `FFprobe`) the sidecar-path resolver:
/// - `ImageMagick` is a bundled DELEGATE inside the image-worker (libvips `magicksave`/`magickload` for
///   BMP+ICO, §3.5.5), NOT a registry-eligible engine: no `(source,target)` pair maps to it (BMP/ICO route
///   through `ImageCore` = the image-worker). Its presence here prevents a spurious `Engine` impl / row.
/// - `FFprobe` is the video two-phase PROBE binary (`binaries/ffprobe`, §3.3.1), spawned as the §3.5.1
///   probe sub-invocation OF the FFmpeg engine (the FFmpeg `trait Engine` impl owns the pair + returns the
///   ffprobe `Invocation`); its `EngineId` exists so the sidecar-path resolver can locate `binaries/ffprobe`
///   (distinct from `binaries/ffmpeg`) and for SBOM + the §7.2 presence-check.
///
/// [Build-Session-Entscheidung: P2.13] WIRE type — it rides `EngineStatus.id` inside the C12 `EngineHealth`
/// return (§7.2), so it derives `Serialize` + `Type`; OUTBOUND-ONLY (no command takes an `EngineId` arg —
/// C12 takes `{}`), so NO `Deserialize` (mirroring the outbound-only `crate::outcome`/`crate::orchestrator`
/// wire types). `Hash` because §0.9 keys a `HashMap<EngineId, bool>` on it (cf. `UserFacingFormat`, also a
/// registry key); `Copy` is free for a fieldless enum. Registration in `collect_types![]` is DEFERRED to
/// the §7.2 `EngineHealth` (C12) consumer, the established P2.2-P2.12 defer pattern.
///
/// [Derived-Assumption: P2.13 — the wire form is `rename_all = "lowercase"` (`ffmpeg`/`ffprobe`/
/// `libreoffice`/…), derived from the §3.2 `Engine::id()` doc examples ("ffmpeg", "libreoffice", "vips");
/// `camelCase` (the other §0.6 enums' rule) would mangle the FF-prefixed variants to `fFmpeg`/`fFprobe`, so
/// lowercase is both spec-faithful and clean for a stable discriminant.]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum EngineId {
    /// FFmpeg — the audio/video engine (§3.5.1); sidecar `binaries/ffmpeg`.
    FFmpeg,
    /// FFprobe — the §3.5.1 probe binary (`binaries/ffprobe`). NON-TRAIT (see above).
    FFprobe,
    /// LibreOffice headless — the office engine (§3.5.2); `serialised_only` (§0.9).
    LibreOffice,
    /// poppler — the PDF text/image engine (§3.5.3).
    Poppler,
    /// pandoc — the markup engine (§3.5.4).
    Pandoc,
    /// ImageMagick — NON-TRAIT delegate inside the image-worker (§3.5.5; see above).
    ImageMagick,
    /// The libvips image-worker (`convertia-imgworker`, §3.5.5) — the registry-eligible image engine.
    ImageCore,
    /// ConvertIA's own MIT in-core CSV/TSV engine (§3.5.6) — `InProcessNative`, no sidecar.
    NativeCsvTsv,
}

/// How an engine runs (§0.6) — mirrors §3.2's `EngineProgram` at the domain level. Every third-party engine
/// (FFmpeg / LibreOffice / poppler / pandoc / ImageMagick + the libvips image-worker) is a `Subprocess`;
/// ONLY ConvertIA's own MIT native CSV/TSV engine (§3.5.6) is `InProcessNative` — there is NO in-process
/// path for any third-party decoder of untrusted bytes (§2.12.4 absolute). The name `InProcessNative` is
/// identical to §3.2 `EngineProgram::InProcessNative` (one canonical name; the earlier `InCoreNative`
/// spelling is retired).
///
/// [Build-Session-Entscheidung: P2.13] INTERNAL (a field of the internal `EngineDescriptor`; never on the
/// wire) — `Debug, Clone, Copy, PartialEq, Eq` (`Copy`, fieldless), NO `serde`/`specta` (mirroring the
/// internal `crate::orchestrator` `Batch`/`ConversionJob`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    /// Spawned as an isolated subprocess (§2.12) — every third-party engine.
    Subprocess,
    /// ConvertIA's own in-core MIT Rust engine (§3.5.6 native CSV/TSV) — no spawn, no third-party bytes.
    InProcessNative,
}

/// The §0.6 / §3.2 capability descriptor for one engine — NOT a process and NOT the §3.2 `trait Engine`
/// (the registry seam). The name is `EngineDescriptor` precisely to avoid colliding with that trait. The
/// §3.2 `Engine::descriptor()` returns it; the §0.9 pool reads `descriptor().serialised_only` from a job's
/// resolved `EngineId` BEFORE spawn to decide whether to also acquire the engine's single-permit semaphore
/// (LibreOffice). It is the concrete `EngineId → serialised_only` data path §0.9 depends on.
///
/// [Build-Session-Entscheidung: P2.13] INTERNAL (the registry/pool read it core-side; never on the wire) —
/// `Debug, Clone, PartialEq, Eq`, NOT `Copy` (the §0.6 struct convention, cf. `PreflightVerdict`/`Batch`),
/// even though every field is `Copy`; no `serde`/`specta`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineDescriptor {
    /// Which engine this describes (§0.6).
    pub id: EngineId,
    /// `true` for an engine the §0.9 pool must run one-at-a-time (LibreOffice headless) — the pool holds a
    /// dedicated single-permit semaphore for it (§0.9).
    pub serialised_only: bool,
    /// Whether the engine runs as a `Subprocess` or `InProcessNative` (§0.6 / §3.2).
    pub kind: EngineKind,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // §6.4.1 unit (G15): the §0.6/§3.2 `EngineId` WIRE form (P2.13) — the stable discriminant rides
    // `EngineStatus.id` in the C12 `EngineHealth` return (§7.2). Pinned to its lowercase wire string per
    // variant (the §3.2 `id()` "ffmpeg"/"libreoffice" convention); the count == 8 + the exhaustive match
    // below lock the set against §0.6 drift. A SERIALIZE pin (EngineId is outbound-only — no round-trip).
    #[test]
    fn engine_id_wire_form_is_lowercase() {
        let all: [(EngineId, &str); 8] = [
            (EngineId::FFmpeg, "ffmpeg"),
            (EngineId::FFprobe, "ffprobe"),
            (EngineId::LibreOffice, "libreoffice"),
            (EngineId::Poppler, "poppler"),
            (EngineId::Pandoc, "pandoc"),
            (EngineId::ImageMagick, "imagemagick"),
            (EngineId::ImageCore, "imagecore"),
            (EngineId::NativeCsvTsv, "nativecsvtsv"),
        ];
        assert_eq!(
            all.len(),
            8,
            "§0.6: EngineId is exactly the eight bundled-engine discriminants (Ghostscript not shipped v1)"
        );
        for (id, wire) in all {
            assert_eq!(
                serde_json::to_string(&id).expect("EngineId serializes"),
                format!("\"{wire}\""),
                "§0.6/§3.2: each EngineId serializes to its lowercase wire discriminant"
            );
        }
    }

    // The COMPILE-TIME variant lock (the established dependency-free exhaustive-match pattern, cf.
    // `crate::outcome`'s `conversion_error_kind_exhaustive`): adding/removing an `EngineId` variant without
    // updating this match fails to compile, so the wire-form array above can never silently drift from §0.6.
    fn engine_id_exhaustive(id: &EngineId) {
        match id {
            EngineId::FFmpeg
            | EngineId::FFprobe
            | EngineId::LibreOffice
            | EngineId::Poppler
            | EngineId::Pandoc
            | EngineId::ImageMagick
            | EngineId::ImageCore
            | EngineId::NativeCsvTsv => {}
        }
    }

    #[test]
    fn engine_id_exhaustive_match_is_exercised() {
        engine_id_exhaustive(&EngineId::ImageCore);
    }

    // §6.4.1 unit (G15): `EngineId` is usable as the §0.9 `HashMap<EngineId, bool>` serialised-flag key
    // (the Hash derive's contract) — the path the pool reads `serialised_only` through. Pins that distinct
    // ids are distinct keys and a lookup returns the stored flag.
    #[test]
    fn engine_id_keys_a_serialised_flag_map() {
        let mut serialised: HashMap<EngineId, bool> = HashMap::new();
        serialised.insert(EngineId::LibreOffice, true);
        serialised.insert(EngineId::FFmpeg, false);
        assert_eq!(serialised.get(&EngineId::LibreOffice), Some(&true));
        assert_eq!(serialised.get(&EngineId::FFmpeg), Some(&false));
        assert_eq!(
            serialised.get(&EngineId::Poppler),
            None,
            "§0.9: an unregistered EngineId is absent from the serialised-flag map"
        );
    }

    // §6.4.1 unit (G15): the §0.6/§3.2 `EngineDescriptor` holds its `EngineId` + `serialised_only` +
    // `EngineKind` (P2.13) — exercises the internal descriptor + `EngineKind` so the test build is
    // dead-code-clean. LibreOffice is the `serialised_only` Subprocess; the native CSV/TSV engine is the
    // sole `InProcessNative`.
    #[test]
    fn engine_descriptor_holds_id_serialised_flag_and_kind() {
        let office = EngineDescriptor {
            id: EngineId::LibreOffice,
            serialised_only: true,
            kind: EngineKind::Subprocess,
        };
        assert_eq!(office.id, EngineId::LibreOffice);
        assert!(
            office.serialised_only,
            "§0.9: LibreOffice is the serialised_only engine"
        );
        assert_eq!(office.kind, EngineKind::Subprocess);

        let csv = EngineDescriptor {
            id: EngineId::NativeCsvTsv,
            serialised_only: false,
            kind: EngineKind::InProcessNative,
        };
        assert_eq!(
            csv.kind,
            EngineKind::InProcessNative,
            "§3.5.6/§2.12.4: the native CSV/TSV engine is the sole InProcessNative"
        );
        assert!(!csv.serialised_only);
    }
}
