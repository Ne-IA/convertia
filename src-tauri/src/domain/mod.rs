//! `crate::domain` — the §0.6 core domain model (tier-3 of the §0.7 module graph; depends on nothing).
//!
//! P1.9 lands only the §0.6 IDENTITY spine the module tree needs to compile and the §0.4.5 IPC
//! type-gen needs to mirror. The full §0.6 type set (the wire DTOs, `CollectedSet`, `UserFacingFormat`,
//! …) is the P2 pipeline-contract task. Identity POLICY (when each id is minted, its lifecycle) is
//! owned by §7.1; this module defines the types and their constructors (e.g. `InstanceId::mint`),
//! never the minting *policy* (when/lifecycle), which stays with §7.1.

// The §0.6 domain types are forward-declared here for the §0.4.5 type-gen + the tier-3 module graph:
// each is defined before its P2+ pipeline / IPC consumer, so each is dead in the PRODUCTION build
// until consumed (`InstanceId` is the exception — minted at startup, §7.1.2 / the P1.15 `setup` stage).
// `expect` (not `allow`) auto-flags the moment the module becomes fully consumed, so this annotation
// cannot silently outlive the scaffolding phase.
// [Build-Session-Entscheidung: P2.1/P2.2] Scoped to `not(test)`: every §0.6 type carries a cfg(test)
// unit test that references it, so the TEST build is dead-code-clean and needs no expectation; the
// expectation holds only for the PRODUCTION build, where the forward-declared types are genuinely dead
// (the scoping was introduced at P2.1, when the JobId alias-lock first referenced a forward-declared type).
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "§0.6 domain types are forward-declared (defined before their P2+ pipeline / IPC consumers), so each is dead in the production build until consumed; InstanceId is the exception (minted at startup, P1.15)."
    )
)]

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

// [Build-Session-Entscheidung: P1.9] one uniform derive set on every identity newtype. Serialize +
// Deserialize: RunId (C7 cancel_run arg), CollectedSetId (C3-C6 args) and CollectingId (C1/C13 args)
// cross the IPC boundary INBOUND (§0.4.1/§0.4.4); Eq + Hash: CollectedSetId keys the §0.4.4 State
// registry map. InstanceId/ItemId keep the same set for uniformity (benign — pure Uuid/u32 newtypes
// with no validation invariant a Deserialize could bypass). §0.6 marks the shown derives illustrative
// ("invariants are normative"), so the concrete set is this box's choice.

/// One per app launch (§7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct InstanceId(Uuid);

impl InstanceId {
    /// Mint the per-launch instance id — §7.1.2: a random **v4** UUID, created once in the §7.2.1
    /// `setup` stage (the P1.15 boot stage). Named `mint` (not `new`) per the §7.1 "minted"
    /// vocabulary and to avoid `clippy::new_without_default` — a random `Default` would be a
    /// surprising, non-deterministic default. [Build-Session-Entscheidung: P1.15]
    #[must_use]
    pub fn mint() -> Self {
        Self(Uuid::new_v4())
    }
}

/// One per `start_conversion` run (§0.4 C6 / §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct RunId(Uuid);

/// The frozen collected-set handle the C3–C6 commands resolve (§0.4 / §0.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct CollectedSetId(Uuid);

/// An ingest-scoped cancellation handle, minted by the frontend before a `RunId` exists (§0.4 C13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct CollectingId(Uuid);

/// Stable item index within a run (§0.6).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Type,
)]
pub struct ItemId(u32);

/// §1.7/§1.8 call it `JobId`; it IS the `ItemId` of the job's item (§0.6).
pub type JobId = ItemId;

/// How a set of paths entered intake (§0.6 / §7.8). Every source is routed through the single §7.8.1
/// funnel into the §1.1 intake state machine, so the §2.4 freeze + §1.3 one-batch rules apply
/// identically regardless of origin. `Drop`/`Picker` reach C1/C2a directly; only `LaunchArg` and
/// `SecondInstance` ever travel on the `app://intake` event (§0.4.2 / §7.8.1).
///
/// [Build-Session-Entscheidung: P2.2] `#[serde(rename_all = "camelCase")]` matches the established
/// §0.6 wire-enum casing (the sibling `ErrorKind`/`IpcError` wire types, §0.4.3): the variants
/// serialize as `drop`/`picker`/`launchArg`/`secondInstance`. `Serialize`+`Deserialize` because the
/// origin crosses IPC both inbound (the C1 `ingest_paths` arg, §0.4.1) and outbound (the `app://intake`
/// payload, §7.8.1); `Copy`/`Eq` are free for a fieldless enum. (`Hash` is omitted — not a map key.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum IntakeOrigin {
    /// Files dropped on the drop area — the §1.1 primary intake; reaches C1 `ingest_paths` directly.
    Drop,
    /// Files chosen via the OS file picker (C2a `pick_for_intake`); reaches C1 directly.
    Picker,
    /// Files passed at first launch (the desktop-entry `%F`/`%U` expansion, the Windows first-launch
    /// `argv`, or the macOS first-launch `RunEvent::Opened`), drained through the §7.8.1
    /// buffer-then-replay once the WebView is ready (§7.8).
    LaunchArg,
    /// Files handed to the already-running instance by a second launch — the §7.1.1 single-instance
    /// `argv`/cwd callback, or the macOS `RunEvent::Opened` while already running (§7.8).
    SecondInstance,
}

/// The single grouping key (§1.3): an individual user-facing format — NOT the six SSOT categories,
/// NOT codec subtypes (`Jpg != Png`, `Mp4 != Mov`). The enumeration IS the SSOT *What It Converts*
/// set; `04-formats/` owns each one's detection signature / targets / engine / options — this enum is
/// just the key. Two dropped items group into one batch iff their `UserFacingFormat` is equal (§1.3).
///
/// [Build-Session-Entscheidung: P2.3] `#[serde(rename_all = "camelCase")]` per the §0.6 "camelCase on
/// the wire" rule + the sibling `ErrorKind`/`IntakeOrigin` precedent (each variant lowercases its
/// leading letter: `jpg`/`png`/…/`threeGp`/…/`odp`). Derive set: `PartialEq`+`Eq`+`Hash` because this
/// is the §1.3 grouping/de-dup key; `Serialize`+`Deserialize`+`Type` because it crosses the wire both
/// ways (the `CollectedSet`/`DetectionOutcome` returns outbound and the `FormatId = UserFacingFormat`
/// C3+ target arg inbound, §0.6); `Copy` is free for a fieldless enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum UserFacingFormat {
    // Images (§04/images)
    Jpg,
    Png,
    Webp,
    Gif,
    Bmp,
    Tiff,
    Heic,
    Avif,
    Ico,
    Svg,
    // Audio (§04/audio)
    Mp3,
    Wav,
    Flac,
    Aac,
    M4a,
    Ogg,
    Opus,
    Wma,
    Aiff,
    Alac,
    // Video (§04/video)
    Mp4,
    Mov,
    Mkv,
    Webm,
    Avi,
    Wmv,
    Flv,
    Mpeg,
    M4v,
    ThreeGp,
    // Documents (§04/documents)
    Pdf,
    Docx,
    Doc,
    Odt,
    Rtf,
    Txt,
    Md,
    Html,
    // Spreadsheets (§04/spreadsheets)
    Xlsx,
    Xls,
    Ods,
    Csv,
    Tsv,
    // Presentations (§04/presentations)
    Pptx,
    Ppt,
    Odp,
}

// ─── §1.2 detection-result family `[DECIDED]` ───────────────────────────────────
// [Build-Session-Entscheidung: P2.15] `DetectionResult`/`DetectionOutcome`/`Confidence`/`ReadFailure`
// are authored together as the ONE §1.2 `[DECIDED]` type-family: `DetectionOutcome::Unreadable { reason:
// ReadFailure }` embeds `ReadFailure`, so a separate `ReadFailure` box would force the otherwise-fatal
// P2.15↔P2.17 needs-cycle (P2.17's `EmptyReport` embeds `DetectionResult`). §1.2 OWNS the family; §0.6
// references it (`DroppedItem.detected: DetectionOutcome`). Wire policy mirrors the P2.2/P2.3 §0.6 enums:
// each member derives `specta::Type` + `Serialize`/`Deserialize` and carries `#[serde(rename_all =
// "camelCase")]` so it mirrors to `bindings.ts` in the §0.6 camelCase wire form. The enum-level attribute
// renames the VARIANT names only — serde does NOT cascade it to a struct-variant's FIELDS, so each
// field-bearing variant repeats it (this is what camelCases `Uncertain.best_guess` → `bestGuess`).
// No specta-`Builder` registration is added here — the same choice P2.2/P2.3 made for `IntakeOrigin`/
// `UserFacingFormat`: no command references the family, so an explicit registration would emit it with no
// consumer; the family auto-registers when its consuming command (C1's `CollectedSet` return, P2.22) is
// wired. `Confidence`/`ReadFailure` are fieldless ⇒ `Copy`; `DetectionOutcome` carries a `String` and
// `DetectionResult` embeds it ⇒ neither is `Copy`. `PartialEq`+`Eq` back the round-trip + membership tests.

/// One item's §1.2 detection verdict — the per-item output of the detection pass (§1.2 / §0.6).
/// `item` ties the verdict to the §0.6 single id space (the §2.4 freeze assigns one `ItemId` over ALL
/// dropped items — eligible + skipped — never re-indexed from 0); `outcome` is the canonical result.
/// `EmptyReport.outcomes: Vec<DetectionResult>` (§1.3, authored in P2.17) is what lets `group()` project
/// the SPECIFIC `CollectedSet` variant of an all-ineligible drop instead of a reason-less `Empty`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DetectionResult {
    /// The §0.6 id of the item this verdict is for.
    pub item: ItemId,
    /// The canonical §1.2 outcome for that item.
    pub outcome: DetectionOutcome,
}

/// The single canonical §1.2 detection outcome `[DECIDED]`. There is no separate
/// `DetectedFormat`/`DetectionConfidence` pair — the earlier 3-valued confidence enum and the
/// `Option<UserFacingFormat>` that collapsed Empty-vs-Unreadable are retired. An ineligible outcome
/// (`UnsupportedType`/`Uncertain`/`Empty`/`Unreadable`) is NEVER offered a target list and NEVER
/// silently extension-fallback-guessed (SSOT *Recognize files by content*); it is surfaced
/// eligible=false with the exact §2.8 plain-language string (the projection to a `SkipReason` is P2.16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DetectionOutcome {
    /// A supported v1 source type, with confidence. `dims` carries the header-derived raster
    /// width/height (JPEG SOF, PNG IHDR, …), read by the §1.2 bounded structural peek — `None` for a
    /// non-raster type or where the header lacks them. It is the input the §1.10 cheap per-pixel size
    /// estimate consumes, so the estimate never needs a decode.
    #[serde(rename_all = "camelCase")]
    Recognized {
        format: UserFacingFormat,
        confidence: Confidence,
        dims: Option<(u32, u32)>,
    },
    /// A real type we identified but do not convert (SSOT "can't convert this type — detected: X").
    /// `detected` carries the named type for the message.
    #[serde(rename_all = "camelCase")]
    UnsupportedType { detected: String },
    /// Sniffed but the signal is contradictory or below threshold — name the best guess (or that we
    /// can't tell) and decline clearly (SSOT). `Low` confidence never silently falls back to the
    /// extension; a genuinely ambiguous file lands here, not in `Recognized`.
    #[serde(rename_all = "camelCase")]
    Uncertain { best_guess: Option<String> },
    /// 0-byte / no bytes to read.
    Empty,
    /// Could not read the bytes at all — `reason` distinguishes gone / locked / permission / other.
    #[serde(rename_all = "camelCase")]
    Unreadable { reason: ReadFailure },
}

/// The §1.2 detection confidence — one name, two values, across §1.2 and §0.6 (the retired draft had a
/// 3-valued enum). `Low` is a first-class outcome on `Recognized`, NOT a silent extension fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    /// The signal is unambiguous.
    High,
    /// Recognized, but the signal is weak — surfaced honestly, never extension-guessed.
    Low,
}

/// Why a file's bytes could not be read at freeze/detect time (§1.2). Owned here; the §2.8 taxonomy
/// projects these to a plain-language string. Distinct from a conversion-time failure (that is the §2.8
/// `ConversionErrorKind`, mirrored as `ErrorKind` in P2.18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ReadFailure {
    /// Gone between drop and freeze (§2.4).
    NotFound,
    /// The OS denied the read.
    PermissionDenied,
    /// Exclusively locked by another process (esp. Windows).
    Locked,
    /// Any other OS read error.
    IoError,
}

// ─── §0.6 DroppedItem — one eligible item in the §1.1-frozen collected set ───────
/// One eligible item in the §1.1-frozen collected set — the per-item record the pipeline carries
/// from freeze through conversion (§0.6 / §1.2). It is a wire type: it reaches the WebView as
/// `CollectedSet::Single.items` (P2.6), but on the wire `raw_path` is **DISPLAY-ONLY** — the §5.3
/// BatchSummary derives sample basenames from the first few `items[].raw_path`, and the WebView
/// NEVER re-submits it as intake. The only intake funnels are C1 (paths the native drop/launch
/// gave) and C2a (paths the Rust-opened picker gave), both Rust-side; a frozen set's `raw_path`
/// travelling back for display does not let the WebView feed an arbitrary path into a conversion
/// (the §0.6 `raw_path` SCOPE `[DECIDED]` note). The §2.4 freeze de-duplicates by RESOLVED IDENTITY
/// on `resolved_path` (owned by §2.3), so two paths reaching one real file are one `DroppedItem`.
///
/// [Build-Session-Entscheidung: P2.4] Wire policy mirrors the P2.2/P2.3/P2.15 §0.6 types: derives
/// `specta::Type` + `Serialize`/`Deserialize` with `#[serde(rename_all = "camelCase")]` so it mirrors
/// to `bindings.ts` in the §0.6 camelCase wire form (`raw_path` → `rawPath`, `resolved_path` →
/// `resolvedPath`, `size_bytes` → `sizeBytes`). NOT `Copy` (it owns two `PathBuf`s + a `String`-bearing
/// `DetectionOutcome`); NOT `Hash` (it is not a map key — the de-dup is by resolved identity on
/// `resolved_path`, §2.3, not by hashing the whole record). `PartialEq`+`Eq` back the round-trip + the
/// §6 property tests (`DetectionOutcome` is `Eq`, so the struct is). No explicit specta-`Builder`
/// registration here — the same choice P2.15 made: the type auto-registers when its consuming command
/// (C1's `CollectedSet` return, P2.22) is wired, so an early registration would emit it with no consumer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DroppedItem {
    /// The §0.6 invariant-6 freeze-assigned id over the SINGLE id space (eligible + skipped). `items`
    /// is a filtered VIEW that is NEVER re-indexed from 0, so each `DroppedItem` carries its own
    /// `ItemId` (its position in `items` is NOT its id). Symmetric with `SkippedItem.item` (P2.5);
    /// `ConversionJob.item` denormalizes it (P2.10).
    pub item: ItemId,
    /// The path as the OS handed it at drop/pick time. DISPLAY-ONLY on the wire (see the type doc).
    pub raw_path: PathBuf,
    /// The symlink/junction/alias-resolved real path (§2.3) — the identity the §2.4 freeze
    /// de-duplicates on and the path the engine is ultimately pointed at.
    pub resolved_path: PathBuf,
    /// Size in bytes of the resolved file, recorded at the §2.4 freeze.
    pub size_bytes: u64,
    /// The single canonical §1.2 detection verdict for this item — §1.2 OWNS the type (P2.15), §0.6
    /// references it. NOT a separate `DetectedFormat` (that earlier name is retired).
    pub detected: DetectionOutcome,
}

#[cfg(test)]
mod tests {
    use super::*;

    // §6.4.1 unit (G15): the §7.1.2 InstanceId minting contract — a fresh, non-nil v4 per launch.
    #[test]
    fn instance_id_mint_is_unique_nonnil_v4() {
        let a = InstanceId::mint();
        let b = InstanceId::mint();
        assert_ne!(a, b, "each launch mints a distinct InstanceId (§7.1.2)");
        assert_ne!(
            a.0,
            Uuid::nil(),
            "a minted InstanceId is never the nil UUID"
        );
        assert_eq!(
            a.0.get_version_num(),
            4,
            "§7.1.2: InstanceId is a v4 (random) UUID"
        );
    }

    // §6.4.1 unit (G15): lock the §0.6 `JobId = ItemId` alias contract. §1.7/§1.8 call the running
    // job's id "JobId"; §0.6 fixes it as `pub type JobId = ItemId` — it IS the ItemId of the job's
    // item, an ALIAS, not a distinct newtype. The `coerce` identity below moves a `JobId` into an
    // `ItemId` with NO conversion, so it compiles ONLY while the two name the same type: a future
    // split of `JobId` into its own newtype fails to compile here, forcing a §0.6-conscious decision
    // rather than a silent divergence of the wire type (the project's anti-drift "lock the contract"
    // discipline, cf. the P2.18.3 variant-count lock). [Build-Session-Entscheidung: P2.1]
    #[test]
    fn jobid_compiles_as_itemid_alias() {
        fn coerce(id: JobId) -> ItemId {
            id
        }
        let item = ItemId(7);
        assert_eq!(
            coerce(item),
            item,
            "§0.6: JobId IS ItemId (the alias contract)"
        );
    }

    // §6.4.1 unit (G15): the §0.6/§7.8 `IntakeOrigin` wire enum — all four origins exist and serialize
    // in the §0.4.3 camelCase wire form. A serialize→deserialize round-trip locks the wire casing so a
    // silent rename can't break the frontend's `IntakeOrigin` handling (Drop/Picker reach C1/C2a;
    // LaunchArg/SecondInstance also ride the `app://intake` event, §7.8.1).
    #[test]
    fn intake_origin_wire_form_is_camelcase_and_roundtrips() {
        for (origin, wire) in [
            (IntakeOrigin::Drop, "\"drop\""),
            (IntakeOrigin::Picker, "\"picker\""),
            (IntakeOrigin::LaunchArg, "\"launchArg\""),
            (IntakeOrigin::SecondInstance, "\"secondInstance\""),
        ] {
            let json = serde_json::to_string(&origin).expect("IntakeOrigin serializes");
            assert_eq!(json, wire, "§0.4.3: IntakeOrigin wire casing is camelCase");
            let back: IntakeOrigin =
                serde_json::from_str(&json).expect("IntakeOrigin round-trips from its wire form");
            assert_eq!(
                back, origin,
                "§7.8: IntakeOrigin round-trips through its wire form"
            );
        }
    }

    // §6.4.1 unit (G15): `UserFacingFormat` IS the §0.6 SSOT *What It Converts* set (the §1.3 grouping
    // key). This locks (a) the §0.4.3 camelCase wire form of every variant via a serialize→deserialize
    // round-trip, and (b) the set membership in BOTH directions — a REMOVED variant fails to compile in
    // `all` below, and an ADDED variant fails to compile in the no-wildcard `exhaustive` match — so the
    // SSOT set cannot silently drift away from §0.6.
    #[test]
    fn user_facing_format_is_the_ssot_set_with_camelcase_wire() {
        use UserFacingFormat as F;
        let all: &[(UserFacingFormat, &str)] = &[
            (F::Jpg, "jpg"),
            (F::Png, "png"),
            (F::Webp, "webp"),
            (F::Gif, "gif"),
            (F::Bmp, "bmp"),
            (F::Tiff, "tiff"),
            (F::Heic, "heic"),
            (F::Avif, "avif"),
            (F::Ico, "ico"),
            (F::Svg, "svg"),
            (F::Mp3, "mp3"),
            (F::Wav, "wav"),
            (F::Flac, "flac"),
            (F::Aac, "aac"),
            (F::M4a, "m4a"),
            (F::Ogg, "ogg"),
            (F::Opus, "opus"),
            (F::Wma, "wma"),
            (F::Aiff, "aiff"),
            (F::Alac, "alac"),
            (F::Mp4, "mp4"),
            (F::Mov, "mov"),
            (F::Mkv, "mkv"),
            (F::Webm, "webm"),
            (F::Avi, "avi"),
            (F::Wmv, "wmv"),
            (F::Flv, "flv"),
            (F::Mpeg, "mpeg"),
            (F::M4v, "m4v"),
            (F::ThreeGp, "threeGp"),
            (F::Pdf, "pdf"),
            (F::Docx, "docx"),
            (F::Doc, "doc"),
            (F::Odt, "odt"),
            (F::Rtf, "rtf"),
            (F::Txt, "txt"),
            (F::Md, "md"),
            (F::Html, "html"),
            (F::Xlsx, "xlsx"),
            (F::Xls, "xls"),
            (F::Ods, "ods"),
            (F::Csv, "csv"),
            (F::Tsv, "tsv"),
            (F::Pptx, "pptx"),
            (F::Ppt, "ppt"),
            (F::Odp, "odp"),
        ];
        assert_eq!(
            all.len(),
            46,
            "§0.6: the SSOT set is 46 formats (10 image + 10 audio + 10 video + 8 doc + 5 sheet + 3 slide)"
        );
        for (fmt, wire) in all {
            let json = serde_json::to_string(fmt).expect("UserFacingFormat serializes");
            assert_eq!(
                json,
                format!("\"{wire}\""),
                "§0.4.3: {fmt:?} wire form must be camelCase `{wire}`"
            );
            let back: UserFacingFormat = serde_json::from_str(&json)
                .expect("UserFacingFormat round-trips from its wire form");
            assert_eq!(
                back, *fmt,
                "§0.6: {fmt:?} round-trips through its wire form"
            );
        }

        // Compiler-enforced membership (the ADD direction): a variant added to the enum without a row
        // in `all` fails to compile here — no wildcard arm (the crate also denies
        // wildcard_enum_match_arm), so the match is non-exhaustive until the new variant is listed.
        fn exhaustive(f: UserFacingFormat) {
            match f {
                F::Jpg
                | F::Png
                | F::Webp
                | F::Gif
                | F::Bmp
                | F::Tiff
                | F::Heic
                | F::Avif
                | F::Ico
                | F::Svg
                | F::Mp3
                | F::Wav
                | F::Flac
                | F::Aac
                | F::M4a
                | F::Ogg
                | F::Opus
                | F::Wma
                | F::Aiff
                | F::Alac
                | F::Mp4
                | F::Mov
                | F::Mkv
                | F::Webm
                | F::Avi
                | F::Wmv
                | F::Flv
                | F::Mpeg
                | F::M4v
                | F::ThreeGp
                | F::Pdf
                | F::Docx
                | F::Doc
                | F::Odt
                | F::Rtf
                | F::Txt
                | F::Md
                | F::Html
                | F::Xlsx
                | F::Xls
                | F::Ods
                | F::Csv
                | F::Tsv
                | F::Pptx
                | F::Ppt
                | F::Odp => {}
            }
        }
        exhaustive(F::Jpg);
    }

    // §6.4.1 unit (G15): the §1.2 `ReadFailure` wire enum — every freeze/detect read-failure reason
    // exists and serializes in the §0.4.3 camelCase wire form, locked by a serialize→deserialize
    // round-trip (a silent rename would break the §2.8 projection + the frontend handling). The
    // no-wildcard `exhaustive` arm locks set MEMBERSHIP: an added/removed variant fails to compile.
    #[test]
    fn read_failure_wire_form_is_camelcase_and_roundtrips() {
        for (reason, wire) in [
            (ReadFailure::NotFound, "\"notFound\""),
            (ReadFailure::PermissionDenied, "\"permissionDenied\""),
            (ReadFailure::Locked, "\"locked\""),
            (ReadFailure::IoError, "\"ioError\""),
        ] {
            let json = serde_json::to_string(&reason).expect("ReadFailure serializes");
            assert_eq!(json, wire, "§0.4.3: ReadFailure wire casing is camelCase");
            let back: ReadFailure =
                serde_json::from_str(&json).expect("ReadFailure round-trips from its wire form");
            assert_eq!(
                back, reason,
                "§1.2: ReadFailure round-trips through its wire form"
            );
        }
        fn exhaustive(r: ReadFailure) {
            match r {
                ReadFailure::NotFound
                | ReadFailure::PermissionDenied
                | ReadFailure::Locked
                | ReadFailure::IoError => {}
            }
        }
        exhaustive(ReadFailure::NotFound);
    }

    // §6.4.1 unit (G15): the §1.2 `Confidence` enum — the one confidence type (High/Low), camelCase on
    // the wire and round-tripped; the no-wildcard `exhaustive` arm locks the two-value membership so a
    // re-introduction of the retired 3-valued enum fails to compile here.
    #[test]
    fn confidence_wire_form_is_camelcase_and_roundtrips() {
        for (confidence, wire) in [(Confidence::High, "\"high\""), (Confidence::Low, "\"low\"")] {
            let json = serde_json::to_string(&confidence).expect("Confidence serializes");
            assert_eq!(json, wire, "§0.4.3: Confidence wire casing is camelCase");
            let back: Confidence =
                serde_json::from_str(&json).expect("Confidence round-trips from its wire form");
            assert_eq!(
                back, confidence,
                "§1.2: Confidence round-trips through its wire form"
            );
        }
        fn exhaustive(c: Confidence) {
            match c {
                Confidence::High | Confidence::Low => {}
            }
        }
        exhaustive(Confidence::High);
    }

    // §6.4.1 unit (G15): the §1.2 `DetectionOutcome` family — assert the §0.4.3 EXTERNALLY-TAGGED
    // camelCase wire form of every variant (incl. the nested `bestGuess` field-rename, the `dims`
    // tuple→array, and the `dims: None` → `null` case), each round-tripped. The no-wildcard `exhaustive`
    // arm locks variant MEMBERSHIP so an added/removed variant fails to compile (the project's anti-drift
    // "lock the contract" discipline, cf. the `UserFacingFormat` set lock above).
    #[test]
    fn detection_outcome_wire_forms_and_membership() {
        // Recognized — `dims: Some` serializes as a 2-element JSON array (the §1.10 size-estimate input).
        let recognized = DetectionOutcome::Recognized {
            format: UserFacingFormat::Jpg,
            confidence: Confidence::High,
            dims: Some((640, 480)),
        };
        assert_eq!(
            serde_json::to_string(&recognized).expect("Recognized serializes"),
            r#"{"recognized":{"format":"jpg","confidence":"high","dims":[640,480]}}"#,
            "§0.4.3: Recognized is externally-tagged camelCase with a tuple `dims` array"
        );
        // dims: None → JSON null (a non-raster or header-less Recognized).
        let recognized_no_dims = DetectionOutcome::Recognized {
            format: UserFacingFormat::Txt,
            confidence: Confidence::Low,
            dims: None,
        };
        assert_eq!(
            serde_json::to_string(&recognized_no_dims).expect("Recognized(None dims) serializes"),
            r#"{"recognized":{"format":"txt","confidence":"low","dims":null}}"#,
            "§1.2: a non-raster Recognized carries dims=null"
        );
        let unsupported = DetectionOutcome::UnsupportedType {
            detected: "PostScript".to_owned(),
        };
        assert_eq!(
            serde_json::to_string(&unsupported).expect("UnsupportedType serializes"),
            r#"{"unsupportedType":{"detected":"PostScript"}}"#,
            "§0.4.3: UnsupportedType names the detected type"
        );
        // Uncertain — the one multi-word field: `best_guess` MUST camelCase to `bestGuess` on the wire.
        let uncertain = DetectionOutcome::Uncertain {
            best_guess: Some("maybe a tiff".to_owned()),
        };
        assert_eq!(
            serde_json::to_string(&uncertain).expect("Uncertain serializes"),
            r#"{"uncertain":{"bestGuess":"maybe a tiff"}}"#,
            "§0.6: the `best_guess` field camelCases to `bestGuess` on the wire"
        );
        // Empty — a fieldless variant serializes as a bare tag string (externally tagged).
        assert_eq!(
            serde_json::to_string(&DetectionOutcome::Empty).expect("Empty serializes"),
            r#""empty""#,
            "§1.2: the fieldless Empty variant is a bare camelCase tag"
        );
        let unreadable = DetectionOutcome::Unreadable {
            reason: ReadFailure::Locked,
        };
        assert_eq!(
            serde_json::to_string(&unreadable).expect("Unreadable serializes"),
            r#"{"unreadable":{"reason":"locked"}}"#,
            "§1.2: Unreadable carries its ReadFailure reason"
        );

        // Round-trip every representative variant (locks deserialize ↔ serialize symmetry).
        for outcome in [
            recognized,
            recognized_no_dims,
            unsupported,
            uncertain,
            DetectionOutcome::Empty,
            unreadable,
        ] {
            let json = serde_json::to_string(&outcome).expect("DetectionOutcome serializes");
            let back: DetectionOutcome =
                serde_json::from_str(&json).expect("DetectionOutcome round-trips");
            assert_eq!(
                back, outcome,
                "§1.2: DetectionOutcome round-trips through its wire form"
            );
        }

        // Compiler-enforced membership: no wildcard arm (the crate denies wildcard_enum_match_arm), so a
        // variant added without an arm here fails to compile rather than silently widening the contract.
        fn exhaustive(o: &DetectionOutcome) {
            match o {
                DetectionOutcome::Recognized { .. }
                | DetectionOutcome::UnsupportedType { .. }
                | DetectionOutcome::Uncertain { .. }
                | DetectionOutcome::Empty
                | DetectionOutcome::Unreadable { .. } => {}
            }
        }
        exhaustive(&DetectionOutcome::Empty);
    }

    // §6.4.1 unit (G15): `DetectionResult` pairs a §0.6 `ItemId` with its §1.2 outcome and round-trips on
    // the wire (the type `EmptyReport.outcomes` carries, P2.17). The `ItemId` newtype inlines as a bare
    // number and the struct fields `item`/`outcome` are camelCase.
    #[test]
    fn detection_result_pairs_item_with_outcome_and_roundtrips() {
        let result = DetectionResult {
            item: ItemId(3),
            outcome: DetectionOutcome::Recognized {
                format: UserFacingFormat::Png,
                confidence: Confidence::High,
                dims: Some((1, 1)),
            },
        };
        let json = serde_json::to_string(&result).expect("DetectionResult serializes");
        assert_eq!(
            json,
            r#"{"item":3,"outcome":{"recognized":{"format":"png","confidence":"high","dims":[1,1]}}}"#,
            "§1.2/§0.6: DetectionResult is {{ item, outcome }} in camelCase wire form"
        );
        let back: DetectionResult =
            serde_json::from_str(&json).expect("DetectionResult round-trips");
        assert_eq!(
            back, result,
            "§1.2: DetectionResult round-trips through its wire form"
        );
    }

    // §6.4.1 unit (G15): the §0.6 `DroppedItem` record — the per-item frozen-set entry. Locks (a) the
    // §0.4.3 camelCase wire form of all five fields (`item`/`rawPath`/`resolvedPath`/`sizeBytes`/
    // `detected`) via a serialize→deserialize round-trip, and (b) the invariant-6 `item: ItemId` field's
    // presence (the §0.6 contradiction-fix field — every eligible DroppedItem carries its own id over the
    // single id space, never its position in `items`). The struct literal is itself the compile-time
    // field-set lock: a removed/renamed field fails to build here. Bare filenames (no path separators)
    // keep the `PathBuf` wire form platform-independent — a `C:\…` path would serialize differently on
    // Windows, making the exact-JSON assertion non-portable.
    #[test]
    fn dropped_item_wire_form_is_camelcase_and_roundtrips() {
        let dropped = DroppedItem {
            item: ItemId(3),
            raw_path: PathBuf::from("holiday.jpg"),
            resolved_path: PathBuf::from("holiday.jpg"),
            size_bytes: 2048,
            detected: DetectionOutcome::Recognized {
                format: UserFacingFormat::Jpg,
                confidence: Confidence::High,
                dims: Some((640, 480)),
            },
        };
        let json = serde_json::to_string(&dropped).expect("DroppedItem serializes");
        assert_eq!(
            json,
            r#"{"item":3,"rawPath":"holiday.jpg","resolvedPath":"holiday.jpg","sizeBytes":2048,"detected":{"recognized":{"format":"jpg","confidence":"high","dims":[640,480]}}}"#,
            "§0.4.3/§0.6: DroppedItem is {{ item, rawPath, resolvedPath, sizeBytes, detected }} in camelCase wire form, item carrying the invariant-6 ItemId"
        );
        let back: DroppedItem = serde_json::from_str(&json).expect("DroppedItem round-trips");
        assert_eq!(
            back, dropped,
            "§0.6: DroppedItem round-trips through its wire form"
        );
    }
}
