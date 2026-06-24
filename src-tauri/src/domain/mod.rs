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
}
