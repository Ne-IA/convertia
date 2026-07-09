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

use std::collections::BTreeMap;
use std::ffi::OsString;
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

    /// The per-instance scratch-root path SEGMENT — `<InstanceId>.<pid>` (§7.1.2 / §2.14): the central
    /// per-run scratch dir is `…/convertia/scratch/<InstanceId>.<pid>/run-<RunId>/`. The `pid` (the OS
    /// process id) is PASSED IN — this is a pure identity formatter, never an OS query — and is a
    /// human-readable LABEL / fast pre-filter ONLY, **never the liveness predicate**: liveness is the
    /// §2.6.3 advisory lock (PIDs are reused, so a PID alone is never a delete gate). The `.` separator
    /// and this shape are what the §2.6.3 startup-sweep glob `convertia/scratch/<*>.<*>/run-*` matches.
    /// The §2.14 path POLICY (the scratch base dir) + the §2.6 scratch lifecycle that assemble the full
    /// path are `crate::run` (P3.1.2); this fixes only the §7.1.2 identity embedded in it.
    /// [Build-Session-Entscheidung: P2.49]
    #[must_use]
    pub fn scratch_root_segment(self, pid: u32) -> String {
        format!("{}.{}", self.0, pid)
    }

    /// The wrapped v4 UUID (§7.1.2). `pub(crate)` — the P3.20 publish-temp naming model (`crate::run`)
    /// renders it into the `.convertia-<InstanceId>-…-.part` sibling name (§2.14.1) and the §2.6.3
    /// cross-instance sweep addresses the owning lock `…/scratch/<InstanceId>.*/…` by it. NOT a wire
    /// accessor — the IPC form stays the derived serde Uuid string; this is a crate-internal render of the
    /// identity `crate::run` assembles the path from (the P3.1.2 seam: domain fixes the embedded identity,
    /// `crate::run` assembles). [Build-Session-Entscheidung: P3.20]
    #[must_use]
    pub(crate) const fn as_uuid(self) -> Uuid {
        self.0
    }

    /// Reconstruct an `InstanceId` from a UUID PARSED out of a publish-temp / lock path (§2.6.1 / §2.6.3),
    /// the inverse of [`as_uuid`](Self::as_uuid). Unlike [`mint`](Self::mint) it does NOT generate a fresh
    /// v4 and does NOT re-assert v4-ness: it re-reads an identifier another (possibly foreign) run already
    /// minted — the owner of a sibling `.convertia-…​.part` — so any well-formed UUID is a valid
    /// reconstruction. `pub(crate)` — a crate-internal reconstructor for the P3.20 naming model, never a
    /// wire constructor. [Build-Session-Entscheidung: P3.20]
    #[must_use]
    pub(crate) const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// One per `start_conversion` run (§0.4 C6 / §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct RunId(Uuid);

impl RunId {
    /// Mint the per-run id — §7.1.2: a random **v4** UUID, minted **when C6 `start_conversion` ACCEPTS the
    /// batch** (the convert-begins point, §0.4.1 C6 / §0.4.4), **NOT at the §2.4 freeze** — the freeze yields
    /// the `CollectedSetId` (the pre-run identity), so a `RunId` (and thus the per-run scratch `run-<RunId>/`,
    /// §2.6.1) never exists before convert begins. This box (P2.48) fixes the mint POINT + adds the mechanism;
    /// the C6 BODY that calls it at accept is P3.46. Named `mint` (not `new`) per the §7.1 "minted" vocabulary
    /// and to avoid `clippy::new_without_default` (a random `Default` would be a surprising, non-deterministic
    /// default), mirroring `InstanceId::mint`. [Build-Session-Entscheidung: P2.48]
    #[must_use]
    pub fn mint() -> Self {
        Self(Uuid::new_v4())
    }

    /// The per-run scratch-subdir path SEGMENT — `run-<RunId>` (§7.1.2 / §2.14): the per-run working dir
    /// `…/<InstanceId>.<pid>/run-<RunId>/` under the per-instance scratch root. The literal `run-` prefix
    /// is what the §2.6.3 sweep glob `…/run-*` matches. The §2.14 path policy + the §2.6 scratch lifecycle
    /// that assemble the full path are `crate::run` (P3.1.2); this fixes only the §7.1.2 identity embedded.
    /// [Build-Session-Entscheidung: P2.49]
    #[must_use]
    pub fn run_subdir_segment(self) -> String {
        format!("run-{}", self.0)
    }

    /// The wrapped v4 UUID (§7.1.2), mirroring [`InstanceId::as_uuid`]. `pub(crate)` — the P3.20
    /// publish-temp naming model renders it into the `<RunId>` segment of
    /// `.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part` (§2.14.1) and the §2.6.3 sweep addresses the
    /// exact owning lock `…/run-<RunId>/.lock` by it. NOT a wire accessor. [Build-Session-Entscheidung: P3.20]
    #[must_use]
    pub(crate) const fn as_uuid(self) -> Uuid {
        self.0
    }

    /// Reconstruct a `RunId` from a UUID PARSED out of a publish-temp / lock path (§2.6.1 / §2.6.3), the
    /// inverse of [`as_uuid`](Self::as_uuid) and the mirror of [`InstanceId::from_uuid`]. Re-reads a
    /// possibly-foreign run's already-minted id (does NOT mint a fresh v4 / re-assert v4-ness); `pub(crate)`
    /// — a crate-internal reconstructor for the P3.20 naming model, never a wire constructor.
    /// [Build-Session-Entscheidung: P3.20]
    #[must_use]
    pub(crate) const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

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

impl ItemId {
    /// The §0.6-invariant-6 freeze constructor: an `ItemId` **IS** the zero-based positional INDEX of an item
    /// in the §1.1 de-duplicated frozen `Vec` of ALL dropped items (eligible + skipped alike, §2.4), assigned
    /// ONCE over the single id space. Named `from_index`, **NOT** `mint`: the sibling `InstanceId::mint` /
    /// `RunId::mint` are random-v4-UUID mints whose value is opaque, whereas an `ItemId` is a DETERMINISTIC
    /// position — so the name names the truth (an index, not a random mint) and keeps the two identity stories
    /// visibly distinct. `const` — a pure `u32` wrap, usable in const / test contexts. It cannot overflow (its
    /// argument already IS a `u32`); exhaustion of the single space is owned by [`ItemIdSpace::mint`], the one
    /// place ids are handed out — it advances a `u32` cursor with `checked_add`, so this design performs NO
    /// `usize → u32` narrowing anywhere (the cursor mints `u32`s directly, never indexes a `usize`-length `Vec`).
    /// [Build-Session-Entscheidung: P2.75]
    #[must_use]
    pub const fn from_index(index: u32) -> Self {
        Self(index)
    }

    /// The wrapped zero-based index (§0.6 invariant 6), the inverse of [`from_index`](Self::from_index).
    /// `pub(crate)` — the P3.20 publish-temp naming model renders it into the `<jobId>` segment of
    /// `.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part` (§2.6.1). NOT a wire accessor. `const` — a pure
    /// `u32` read, usable in const/test contexts like its `from_index` inverse. [Build-Session-Entscheidung: P3.20]
    #[must_use]
    pub(crate) const fn as_u32(self) -> u32 {
        self.0
    }
}

/// §1.7/§1.8 call it `JobId`; it IS the `ItemId` of the job's item (§0.6).
pub type JobId = ItemId;

/// The §0.6-invariant-6 single `ItemId` space (§1.1 / §2.4) — the ONE monotonic source that hands out each
/// `ItemId` **exactly once**, in strictly increasing order from `0`, never reset. At the §1.1 freeze both the
/// eligible (`DroppedItem`) and the skipped (`SkippedItem`) views mint from the SAME space, so their ids are
/// **id-disjoint by construction** and neither is ever re-indexed from 0 — the invariant made STRUCTURAL, not
/// conventional (there is no public way to write `ItemId(0)` twice or reset the cursor). PURE (tier-3): a
/// private `u32` cursor, no I/O and no `crate::outcome` reference, homed beside `ItemId`; the
/// `crate::orchestrator` freeze spine constructs one per freeze and mints across it (the sanctioned downward
/// `orchestrator → domain` edge). Core-INTERNAL — never crosses IPC, so no `serde`/`specta` (the internal-type
/// posture of `FrozenCollectedSet`). The de-dup FOLD that mints one id per first-seen survivor is P2.76; the
/// end-to-end wiring is the P3.49 spine. [Build-Session-Entscheidung: P2.75]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemIdSpace {
    /// The index the NEXT [`mint`](ItemIdSpace::mint) hands out; `None` once the space is EXHAUSTED (the last
    /// mint handed out `ItemId::from_index(u32::MAX)`), so `u32::MAX` IS a valid final id and only the
    /// FOLLOWING mint fails — never a silent `as u32` wrap.
    next: Option<u32>,
}

impl Default for ItemIdSpace {
    /// A fresh space, identical to [`ItemIdSpace::new`] — its first mint is `ItemId::from_index(0)`.
    fn default() -> Self {
        Self::new()
    }
}

impl ItemIdSpace {
    /// A fresh single id space whose first mint is `ItemId::from_index(0)` (§0.6 invariant 6). `const` so it can
    /// seed a `const` context; equal to [`ItemIdSpace::default`]. [Build-Session-Entscheidung: P2.75]
    #[must_use]
    pub const fn new() -> Self {
        Self { next: Some(0) }
    }

    /// Mint the NEXT `ItemId` over this single space and advance the cursor — the ONE way an id is assigned at
    /// the §1.1 freeze. The N-th mint yields `ItemId::from_index(N)` (order-preserving, contiguous from 0).
    /// Returns `Err(`[`ItemSpaceExhausted`]`)` — **never** a silent `as u32` wrap or a panic (the in-core
    /// no-panic discipline, G4/G14) — once the space is spent: `mint` reads the id at the current cursor THEN
    /// advances via `checked_add`, so `ItemId::from_index(u32::MAX)` is a valid FINAL id and only the FOLLOWING
    /// mint fails (the boundary the §1.10 resource bounds cap far below in practice; the code stays honest
    /// regardless). The P2.76 de-dup fold mints INSIDE its first-seen branch — so a dropped duplicate consumes
    /// no id — and propagates this `Err` so the freeze fails cleanly. [Build-Session-Entscheidung: P2.75]
    pub fn mint(&mut self) -> Result<ItemId, ItemSpaceExhausted> {
        match self.next {
            Some(index) => {
                let id = ItemId::from_index(index);
                self.next = index.checked_add(1);
                Ok(id)
            }
            None => Err(ItemSpaceExhausted),
        }
    }
}

/// The §0.6-invariant-6 `ItemId` space is EXHAUSTED — all `2^32` ids (`0..=u32::MAX`) have been minted, so no
/// further `ItemId` can be assigned over the single id space (§0.6). Surfaced honestly, never a silent `as u32`
/// wrap; the §1.10 resource bounds cap a real frozen set far below this, and the P3.49 freeze spine maps this to
/// the §1.1 fatal-ingest surface (the §2.8 taxonomy), never a panic. A fieldless marker (the only failure mode
/// is "ran out of `u32`"); core-INTERNAL, so no `serde`/`specta`. [Build-Session-Entscheidung: P2.75]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemSpaceExhausted;

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

/// Why a file or dropped root could not be read during intake — shared by the §1.2 detection read
/// (`DetectionOutcome::Unreadable`) and the §1.1 fatal walk-root stop (the dropped root itself
/// unreadable/gone, P2.68). Owned here; the §2.8 taxonomy projects these to a plain-language string. Distinct
/// from a conversion-time failure (that is the §2.8 `ConversionErrorKind`, mirrored as `ErrorKind` in P2.18).
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

// ─── §1.3 EmptyReport — the all-ineligible-drop report feeding the Empty { skipped } tally (P2.17) ──
/// The §1.3 all-ineligible-drop report (§1.2 / §0.6) — carries every dropped item's §1.2 `DetectionResult`
/// when `group()` (P3) finds NO eligible source, so the §1.3 `Empty(EmptyReport) → CollectedSet` projection
/// can pick the SPECIFIC variant (a lone `Unsupported` / lone `Uncertain`, else `Empty { skipped }` with the
/// per-item `SkipReason`s) instead of a reason-less empty. The per-item reasons come from the §1.2/§1.3
/// `DetectionOutcome::skip_reason` projection (P2.16) over these `outcomes`.
///
/// [Build-Session-Entscheidung: P2.17] INTERNAL type (the §1.3 `Grouping` intermediate maps onto the wire
/// `CollectedSet`, so this never crosses IPC — the same posture as `Batch` / `OutputPlan`), hence NO
/// `serde`/`specta`. Derives `Debug, Clone, PartialEq, Eq` (the internal-type set); NOT `Copy` (owns a
/// `Vec`). `Eq` holds (`DetectionResult` is `Eq`, P2.15). `pub outcomes` since `group()` (P3) constructs it
/// and the projection reads it — no validation invariant a private field would protect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyReport {
    /// Every dropped item's §1.2 detection result, in the §1.1 freeze order — all ineligible (there was no
    /// eligible source); the §1.3 projection reads these to build the per-item `SkippedItem` reasons.
    pub outcomes: Vec<DetectionResult>,
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

// ─── §0.6 SkippedItem / SkipReason — the id-disjoint ineligible-item view ────────
/// An item present in the drop but NOT eligible for the batch — unsupported / uncertain / empty /
/// unreadable at the §1.1 freeze (§0.6 / §1.3). Surfaced in the §1.4 confirm summary and the §1.12 run
/// summary so a bad item is never silently dropped. `item` is drawn from the SAME single id space as the
/// eligible `DroppedItem`s but is **id-DISJOINT** with them (§0.6 invariant 6 — the eligible
/// `members`/`items` and the `skipped` ids are never-re-indexed filtered VIEWS over one space, so a
/// `SkippedItem.item` can never collide with an eligible id). It stores a `SkipReason` (NOT an
/// `ErrorKind`): every `SkippedItem` comes from a detection-INELIGIBLE outcome, all of which have a
/// `SkipReason`, so the §1.12 `OutcomeMsg::Skipped` projection is a trivial copy (no undefined
/// `ErrorKind → SkipReason` reverse map at the boundary).
///
/// [Build-Session-Entscheidung: P2.5] Wire policy mirrors `DroppedItem` / the P2.2/P2.3/P2.15 §0.6
/// types: derives `specta::Type` + `Serialize`/`Deserialize` with `#[serde(rename_all = "camelCase")]`.
/// NOT `Copy` (owns a `PathBuf`); `PartialEq`+`Eq` back the round-trip + §6 property tests. No explicit
/// specta registration — auto-registers via its consuming command (the C1 `CollectedSet` return, P2.22).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SkippedItem {
    /// The §0.6 invariant-6 freeze-assigned id — id-disjoint with the eligible items over the single id
    /// space (never re-indexed from 0). Symmetric with `DroppedItem.item`.
    pub item: ItemId,
    /// The dropped path, for the §1.4 summary display.
    pub source: PathBuf,
    /// Why the item was skipped — a §0.6 `SkipReason`, NOT an `ErrorKind` (see the type doc).
    pub reason: SkipReason,
}

/// Why a dropped item was skipped — the four detection-INELIGIBLE §1.2 outcome classes (§0.6 / §1.3).
/// Carried on `SkippedItem.reason` as the canonical skip cause. The `DetectionOutcome → SkipReason`
/// projection is P2.16, and the ONE-WAY forward `SkipReason → ErrorKind` projection (the non-invertible
/// `Uncertain → Unrecognized`, §2.8.2) lives on the §1.12 helper (P2.20), never on this type. NOT
/// `ErrorKind`: a skip is a freeze-time ineligibility, distinct from a conversion-time failure.
///
/// [Build-Session-Entscheidung: P2.5] Mirrors the sibling fieldless wire enums (`ReadFailure` /
/// `Confidence`): `Copy` (fieldless) + the uniform `#[serde(rename_all = "camelCase")]` wire form
/// (`unsupportedType` / `uncertain` / `empty` / `unreadable`). No `Hash` (not a map key).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    /// A real type we identified but do not convert (the ineligible `DetectionOutcome::UnsupportedType`).
    UnsupportedType,
    /// Sniffed but contradictory / below threshold — we declined to guess (`DetectionOutcome::Uncertain`).
    Uncertain,
    /// 0-byte / no bytes to read (`DetectionOutcome::Empty`).
    Empty,
    /// Could not read the bytes at all (`DetectionOutcome::Unreadable`).
    Unreadable,
}

// ─── §1.2/§1.3 DetectionOutcome → SkipReason projection (ineligible-outcome → skip cause, P2.16) ──
/// [Build-Session-Entscheidung: P2.16] The §1.2/§1.3 projection is a METHOD on `DetectionOutcome` returning
/// `Option<SkipReason>` — NOT a `From`/`TryFrom` impl: the map is a total function over all five outcomes but
/// is partial onto `SkipReason` (the eligible `Recognized` outcome has no image), so `Option` models
/// "eligible ⇒ no skip reason" cleanly where `From` would need a panic and `TryFrom` an error type for the
/// eligible case. §1.3 `group()` (P3) calls it to fill `SkippedItem.reason` when building the
/// `CollectedSet::Single.skipped` / `Empty { skipped }` views; the eligible `Recognized` outcome becomes a
/// batch MEMBER, never a `SkippedItem`. The four INELIGIBLE outcomes project by IDENTICAL name (the §0.6
/// `SkipReason` set is exactly those four), so the projection cannot silently mis-map. This is the §1.2-side
/// projection; the inverse, one-way `SkipReason → ErrorKind` lives on the separate §1.12 helper (P2.20).
impl DetectionOutcome {
    /// Project this §1.2 detection outcome to its §0.6 `SkipReason` (§1.3) — `None` for the eligible
    /// `Recognized` outcome (a batch member, never skipped), `Some(reason)` for each ineligible outcome,
    /// by identical name (`UnsupportedType`/`Uncertain`/`Empty`/`Unreadable`).
    #[must_use]
    pub fn skip_reason(&self) -> Option<SkipReason> {
        match self {
            DetectionOutcome::Recognized { .. } => None,
            DetectionOutcome::UnsupportedType { .. } => Some(SkipReason::UnsupportedType),
            DetectionOutcome::Uncertain { .. } => Some(SkipReason::Uncertain),
            DetectionOutcome::Empty => Some(SkipReason::Empty),
            DetectionOutcome::Unreadable { .. } => Some(SkipReason::Unreadable),
        }
    }
}

// ─── §0.6 CollectedSet — the frozen batch candidate (C1/C2a return + §1.4 confirm shape) ──
/// The frozen collected-set the C1 `ingest_paths` / C2a `pick_for_intake` commands return and the §1.4 /
/// §5.2 confirm gate renders (§0.6 / §1.1 / §1.4). `Single` carries the FULL confirm-summary field set,
/// so the wire type IS the §1.4 `CollectedSummary` (unified — the mandatory confirm gate gets a real IPC
/// path); the §0.4.4 collected-set registry stores this payload + its roots keyed by `CollectedSetId`
/// for C3–C6 to resolve. The five variants are the §1.3 grouping outcomes: exactly one eligible format
/// (`Single`), 2+ eligible formats (`Mixed` → pre-flight refusal), a lone real-but-unsupported /
/// lone-uncertain item (`Unsupported` / `Uncertain`), or nothing eligible (`Empty`, carrying the
/// per-item skip reasons so §5.2 state-10 is specific, not reason-less).
///
/// [Build-Session-Entscheidung: P2.6] Wire policy mirrors the P2.2/P2.3/P2.15/P2.4/P2.5 §0.6 types:
/// derives `specta::Type` + `Serialize`/`Deserialize`; externally-tagged with `#[serde(rename_all =
/// "camelCase")]` at the enum level (variant tags `single`/`mixed`/`unsupported`/`uncertain`/`empty`) AND
/// repeated on every field-bearing variant (serde does NOT cascade the enum-level rename to a
/// struct-variant's FIELDS, so `Single` needs it for `total_bytes`/`encoding_hint`/`delimiter_hint` →
/// `totalBytes`/`encodingHint`/`delimiterHint`). NOT `Copy` (owns `Vec`/`String`/`PathBuf`);
/// `PartialEq`+`Eq` back the round-trip tests. No explicit specta registration here — the WHOLE
/// CollectedSet graph (`DroppedItem`/`SkippedItem`/`CollectedNote`/…) auto-registers together via its C1
/// consumer (P2.22), the established defer pattern; deriving `specta::Type` is what guarantees it mirrors
/// to `bindings.ts` as a NAMED type (never `any`) once consumed, so an early registration would only emit
/// a consumer-less type and churn `bindings.ts` ahead of its command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CollectedSet {
    /// Exactly one eligible user-facing format across all readable items → a batch. Carries the full §1.4
    /// confirm-summary field set (it IS the `CollectedSummary` wire shape). `items` is the eligible
    /// filtered view + `skipped` the id-disjoint ineligible view over the §0.6-invariant-6 single id
    /// space; the §1.4 confirm-summary FIELDS are COMPUTED in P3.27/P3.28 — this box homes the wire TYPE.
    #[serde(rename_all = "camelCase")]
    Single {
        id: CollectedSetId,
        instance: InstanceId,
        format: UserFacingFormat,
        items: Vec<DroppedItem>,
        /// Shown in the confirm gate (§1.4). INVARIANT (§0.6): `count == items.len()`, set once at the
        /// §1.1 freeze; kept separate so a wire consumer reading the tally never walks a 10k-file Vec (the
        /// §6 property test asserts the equality so the duplication cannot drift).
        count: usize,
        skipped: Vec<SkippedItem>,
        /// Size hint for the §1.10 pre-flight (§1.4).
        total_bytes: u64,
        /// The dropped root(s) → §2.7 subtree + open-folder.
        roots: Vec<PathBuf>,
        /// A detection-derived hint, e.g. CSV detected "Windows-1252" (per §04).
        encoding_hint: Option<String>,
        /// A detection-derived hint, e.g. CSV/TSV detected ";" (per §04).
        delimiter_hint: Option<String>,
        /// The §1.4-owned structural-peek notes (>1 sheet, animated source, …), PRODUCED by §1.2's
        /// bounded peek — not invented here.
        notes: Vec<CollectedNote>,
    },
    /// Two or more distinct eligible source formats → the §1.3 hard pre-flight refusal; `found` lists
    /// each format with its count for the refusal message.
    #[serde(rename_all = "camelCase")]
    Mixed {
        found: Vec<(UserFacingFormat, usize)>,
    },
    /// A lone item that is a real type we identified but do not convert (§1.2); `detected` names it.
    #[serde(rename_all = "camelCase")]
    Unsupported { detected: String },
    /// A lone item we could not classify with confidence (§1.2); `note` carries the can't-tell text.
    #[serde(rename_all = "camelCase")]
    Uncertain { note: String },
    /// Nothing eligible. `skipped` carries the per-item skip reasons (§1.3 projection from
    /// `EmptyReport.outcomes`) so §5.2 state-10 shows "N files, none convertible (M unreadable, …)"
    /// instead of a reason-less empty; `vec![]` for the genuinely-zero-items case (cancelled dialog /
    /// drained `PendingIntake` / all files hidden-filtered).
    #[serde(rename_all = "camelCase")]
    Empty { skipped: Vec<SkippedItem> },
}

/// A §1.4-owned structural-peek note surfaced in the §1.4 confirm summary (`CollectedSet::Single.notes`),
/// PRODUCED by §1.2's bounded structural peek (step 4) — spreadsheets.md / images.md / audio.md own the
/// per-format peek, §1.2 owns running it. The `kind` is a stable discriminant → the §5 label catalogue
/// (§2.10); any value (sheet count, encoding, …) rides `detail`, NOT the variant. Never a pre-localised
/// sentence (§5 localises the `kind`).
///
/// [Build-Session-Entscheidung: P2.6] Same wire policy as the sibling §0.6 types: derives `specta::Type`,
/// `Serialize`, `Deserialize` and `#[serde(rename_all = "camelCase")]`; NOT `Copy` (owns an
/// `Option<String>`). Registration is deferred to the C1 consumer (P2.22) with the rest of the
/// CollectedSet graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CollectedNote {
    /// The stable discriminant → the §5 label catalogue (§2.10).
    pub kind: CollectedNoteKind,
    /// The optional value (e.g. "3 sheets", "Windows-1252").
    pub detail: Option<String>,
}

/// The stable §1.4 note discriminant. The four typed variants each have a declared §1.2-step-4 producer;
/// `Other` is a RESERVED forward-compatible catch-all emitted by no current (v1) engine — it carries its
/// value in `CollectedNote.detail` and is never silently dropped.
///
/// [Build-Session-Entscheidung: P2.6] Fieldless wire enum like `SkipReason` / `ReadFailure`: `Copy` +
/// `#[serde(rename_all = "camelCase")]` (`multipleSheets` / `animatedSource` / `multiSizeIcon` /
/// `embeddedCoverArt` / `other`). No `Hash` (not a map key).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CollectedNoteKind {
    /// spreadsheets.md: the source holds >1 sheet, only one is exported.
    MultipleSheets,
    /// images.md: an animated source converted to a still target flattens.
    AnimatedSource,
    /// images.md: an ICO source holds >1 size.
    MultiSizeIcon,
    /// audio.md: cover art present.
    EmbeddedCoverArt,
    /// Reserved forward-compatible catch-all — no v1 producer; the value rides `detail`.
    Other,
}

/// The §0.4.4 collected-set registry's stored value — the **frozen projection of a
/// `CollectedSet::Single`** the core retains so the bare-`collectedSetId` C3 `get_targets` /
/// C4 `plan_output` / C5 `set_destination` / C6 `start_conversion` commands resolve back to the
/// detected format, the frozen `items`, the dropped `roots`, and the `skipped` view **without a
/// second walk or re-detection** (§0.4.4). It carries the **FULL `Single` payload** (every field —
/// §0.4.4 "store this payload + its roots"): C3 reads `format`, C4/C5 plan against `roots`, C6
/// rebuilds the `Batch` from `items` (and §2.7 needs `roots` for subtree re-creation); the
/// size/hints/notes are retained so a post-reload confirm re-render (§1.4) stays faithful.
///
/// Only a `CollectedSet::Single` yields a registrable entry — `Mixed`/`Unsupported`/`Uncertain`/
/// `Empty` are terminal pre-flight states with no resolvable `CollectedSetId` (§0.4.4 / §0.6
/// invariant 3), so the projection is fallible:
/// [`from_collected`](FrozenCollectedSet::from_collected) returns `Some` ONLY for a `Single`. The
/// store that holds these keyed by `CollectedSetId` is the `crate::orchestrator::CollectedSetRegistry`
/// (the §0.4.4 State store, P2.44) — a downward `orchestrator`→`domain` edge, like the `RunRegistry`'s
/// `RunId` key.
///
/// [Build-Session-Entscheidung: P2.44] Core-INTERNAL (NOT a wire type) — it never crosses IPC: C3–C6
/// resolve it core-side and return their OWN §0.6 DTOs (`TargetOffer`/`OutputPlanPreview`/…), the
/// WebView never sees a `FrozenCollectedSet`. So it derives NO `serde`/`specta` — only `Debug, Clone,
/// PartialEq, Eq` (the internal-type set, like the orchestrator-internal `Batch`/`OutputPlan`); NOT
/// `Copy` (owns `Vec`/`String`/`PathBuf` fields). Every field type is `Eq`, backing the projection test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenCollectedSet {
    /// The set's handle — the §0.4.4 registry key (also the C3–C6 `collectedSetId` argument). Kept
    /// inside the value (mirroring the `Single` payload) so the registry insert is self-keyed off `id`.
    pub id: CollectedSetId,
    /// The instance that froze this set (§7.1.2) — carried through from `Single`.
    pub instance: InstanceId,
    /// The single eligible user-facing source format (§1.3 grouping key) — what C3 `get_targets` reads.
    pub format: UserFacingFormat,
    /// The frozen eligible items (§2.4) — what C6 rebuilds the `Batch` from (no second walk).
    pub items: Vec<DroppedItem>,
    /// The eligible-item tally (§1.4) — `count == items.len()` (the `Single` invariant, carried through).
    pub count: usize,
    /// The id-disjoint ineligible (pre-flight `Skipped`) view (§0.6 invariant 6) — projected into the
    /// §1.12 summary; retained so C6 can carry the skips forward.
    pub skipped: Vec<SkippedItem>,
    /// The §1.10 pre-flight size hint (§1.4) — retained for the C4/C5 estimate.
    pub total_bytes: u64,
    /// The dropped root(s) (§2.7) — what C4/C5 plan against + the §2.7 subtree / open-folder anchor.
    pub roots: Vec<PathBuf>,
    /// A detection-derived encoding hint (e.g. CSV "Windows-1252", per §04) — retained for re-render.
    pub encoding_hint: Option<String>,
    /// A detection-derived delimiter hint (e.g. CSV/TSV ";", per §04) — retained for re-render.
    pub delimiter_hint: Option<String>,
    /// The §1.4 structural-peek notes — retained for a post-reload confirm re-render (§1.4).
    pub notes: Vec<CollectedNote>,
}

impl FrozenCollectedSet {
    /// Project a `CollectedSet` into a registrable `FrozenCollectedSet` (§0.4.4) — `Some` ONLY for a
    /// `Single` (the only outcome with a resolvable `CollectedSetId`, §0.6 invariant 3); `None` for the
    /// terminal `Mixed`/`Unsupported`/`Uncertain`/`Empty` pre-flight states, which the registry never
    /// stores. Clones the `Single` payload (the wire copy C1/C2a returns is serialized then dropped;
    /// this retained copy out-lives it). The **exhaustive `Single { .. }` destructure (NO `..`)** makes
    /// a new `Single` field a COMPILE error here, so the frozen projection can never silently drift from
    /// the `Single` payload it mirrors. [Build-Session-Entscheidung: P2.44]
    #[must_use]
    pub fn from_collected(set: &CollectedSet) -> Option<Self> {
        match set {
            CollectedSet::Single {
                id,
                instance,
                format,
                items,
                count,
                skipped,
                total_bytes,
                roots,
                encoding_hint,
                delimiter_hint,
                notes,
            } => Some(Self {
                id: *id,
                instance: *instance,
                format: *format,
                items: items.clone(),
                count: *count,
                skipped: skipped.clone(),
                total_bytes: *total_bytes,
                roots: roots.clone(),
                encoding_hint: encoding_hint.clone(),
                delimiter_hint: delimiter_hint.clone(),
                notes: notes.clone(),
            }),
            CollectedSet::Mixed { .. }
            | CollectedSet::Unsupported { .. }
            | CollectedSet::Uncertain { .. }
            | CollectedSet::Empty { .. } => None,
        }
    }
}

// ─── §0.6 wire DTOs for the C-commands + app:// hand-off (§0.4.1 / §0.4.2) ───────
// [Build-Session-Entscheidung: P2.7] The §0.6 "Intake & detection" wire-DTO group. Each derives
// `specta::Type` + camelCase per the §0.6 wire convention so it mirrors to `bindings.ts` as a named type;
// registration is deferred to the consuming command/event (C2a/C9/app://intake/C1-onScan, P2.21+), the
// established P2.2–P2.6 defer pattern. DIRECTION drives the derive set: the INBOUND command-arg enums
// (`PickKind`/`OpenKind`) derive `Serialize`+`Deserialize` (round-trippable, fieldless → `Copy`); the
// app:// event payload (`IntakePayload`) follows the round-trippable struct pattern (`Serialize`+
// `Deserialize`, like `DroppedItem`); the Channel payload (`ScanProgress`) is OUTBOUND-ONLY per its §0.6
// literal (`#[derive(Clone, Serialize, specta::Type)]`) — `Serialize` without `Deserialize`, since the
// frontend RECEIVES but never sends it.

/// The C2a `pick_for_intake` `kind` arg (§0.4.1) — pick files or a folder. Inbound (WebView → Rust).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PickKind {
    /// Pick one or more files.
    Files,
    /// Pick a folder (recursively collected at the §1.1 freeze).
    Folder,
}

/// The C9 `open_path` `kind` arg (§0.4.1 / §7.7) — how to surface an output path. Inbound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum OpenKind {
    /// Open the containing folder.
    Folder,
    /// Open the file itself in its default app.
    File,
    /// Reveal the file within its folder (highlight it).
    RevealInFolder,
}

/// The `app://intake` hand-off payload (§0.4.2 / §7.8.1) — the launch-arg / second-instance paths drained
/// through the §7.8.1 buffer-then-replay once the WebView is ready. `origin` is typed as the full
/// `IntakeOrigin`, but only `LaunchArg` | `SecondInstance` ever travel on this event (`Drop`/`Picker`
/// reach C1/C2a directly) — a §7.8.1 runtime invariant, not a type constraint.
///
/// [Build-Session-Entscheidung: P2.7] Follows the round-trippable struct pattern (`Serialize`+
/// `Deserialize`, like `DroppedItem`); NOT `Copy` (owns a `Vec<PathBuf>`). camelCase wire form.
///
/// [Build-Session-Entscheidung: P2.39] Registered EXPLICITLY in `main.rs`'s `register_ipc_event_types`
/// `.types()` chain now that its consumer — the `app://intake` event (§0.4.2 / §7.8.1) — is authored.
/// The P2.7 "deferred to its consuming command/event" note assumed an auto-pull, but `app://intake` is a
/// RAW `app.emit` / TS `listen` event (§0.4.2), not a command arg / `collect_events!` typed event, so it
/// does NOT auto-pull `IntakePayload` into `bindings.ts` — the explicit `.types()` registration is what
/// keeps `listen('app://intake')` typed against the named `IntakePayload` rather than `any` (the same
/// reason `collect_events!` is avoided — see `register_ipc_event_types`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IntakePayload {
    /// The paths handed in (already resolved by the §7.8.1 funnel; frozen at C1).
    pub paths: Vec<PathBuf>,
    /// How the set entered intake — only `LaunchArg` | `SecondInstance` on this event (see the type doc).
    pub origin: IntakeOrigin,
}

/// The C1 `ingest_paths` `onScan` Channel payload (§0.4.2) — a throttled (~2/s, coalesced) live count of
/// files seen during the §1.1 recursive walk + §1.2 detection, so the §5.2 Collecting state can show
/// "Scanning… N files so far". Best-effort, monotonic, dies with the C1 call.
///
/// [Build-Session-Entscheidung: P2.7] Honors the §0.6 literal's deliberate OUTBOUND-ONLY derive set
/// (`#[derive(Clone, Serialize, specta::Type)]`): the frontend RECEIVES this Channel payload but never
/// sends it, so no `Deserialize` (and no `PartialEq`/`Eq` — the contract is the serialized form, not a
/// round-trip; `Debug` is a benign ergonomic add). `specta::Type` is MANDATORY (§0.6: a
/// `Channel<ScanProgress>` without it is `any` in `bindings.ts`). camelCase for module uniformity (a
/// no-op on the single-word `scanned`).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    /// The throttled, monotonic count of files seen so far.
    pub scanned: u32,
}

// ─── §1.6 OptionDecl family — the generic per-(source,target) option model (P2.8.1) ──
// [Build-Session-Entscheidung: P2.8] The §1.6-owned option-declaration model. Each derives `specta::Type`
// + camelCase; NOT explicitly registered — deferred to the C3 `get_targets` consumer (P2.25), the
// established P2.2-P2.7 defer pattern (`Target.options: Vec<OptionDecl>` auto-registers the family then).
// Types owning `String`/`Vec` are not `Copy`; the fieldless `Surface`/`Unit` are `Copy`. `OptionKey`
// derives `Ord` (it is the `OptionValues` BTreeMap key + the §2.5 EquivKey). `OptionKey`/`LabelKey` are
// transparent `String` newtypes (serde serializes a 1-tuple struct as its inner value → a bare string),
// with a `pub` field since the §1.6 registry (P5-P7) constructs them from known slugs (no validation
// invariant a public field could bypass).

/// A UI surface tier for an option (§1.6) — Basic (materially changes a normal result) vs Advanced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Surface {
    /// The few switches that materially change a normal user's result.
    Basic,
    /// Power-user knobs, hidden by default.
    Advanced,
}

/// Display unit for an `IntRange` option — purely for the §5 label, not semantic (§1.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Unit {
    Percent,
    Kbps,
    Px,
    Dpi,
    Fps,
}

/// A stable machine key for an option (e.g. "quality", "fps", "lossless"), §1.6. Used as the
/// `OptionValues` BTreeMap key and in the §2.5 EquivKey canonicalisation, so it is a stable ASCII slug,
/// never a UI label. Derives `Ord` for its BTreeMap-key role; serializes transparently as a bare string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type)]
pub struct OptionKey(pub String);

/// A UI-chrome label key (§1.6 / §5 / §2.10) — §5 resolves it to a localised string. NOT a user-facing
/// string itself; keeps the domain model i18n-free (§2.8/§2.9 own surfaced strings). Bare-string wire form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct LabelKey(pub String);

/// A named preset choice inside an `Enum` option (e.g. MP3 "High"/"Standard"/"Small"), §1.6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EnumChoice {
    /// The stable id stored in `OptionValue::Enum` (never localised).
    pub value: String,
    /// The §5 UI-chrome label for the choice.
    pub label: LabelKey,
}

/// The shape of an option control (§1.6). Externally tagged; the payload carries the bounds/choices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum OptionKind {
    /// A bounded integer (quality / CRF / compression level) with a range + optional display unit.
    #[serde(rename_all = "camelCase")]
    IntRange {
        min: i64,
        max: i64,
        step: i64,
        unit: Option<Unit>,
    },
    /// A small named preset set mapping to engine flags.
    #[serde(rename_all = "camelCase")]
    Enum { choices: Vec<EnumChoice> },
    /// A boolean toggle (lossless on/off, progressive, BOM).
    Toggle,
    /// A pixel/size value (SVG width, GIF width).
    #[serde(rename_all = "camelCase")]
    Size { min: u32, max: u32 },
    /// A colour (flatten background) — picker; default usually white.
    Color,
}

/// One concrete, fully-resolved option value (§1.6). INVARIANT (§1.6): every variant is JSON-serialisable
/// and round-trips through the §2.5 canonical form; no floats (no NaN/Inf), colours as `#RRGGBB(AA)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum OptionValue {
    /// An `IntRange` / `Size` resolved value.
    Int(i64),
    /// A `Toggle` value.
    Bool(bool),
    /// The chosen `EnumChoice.value` (the stable id, not the label).
    Enum(String),
    /// A `#RRGGBB` / `#RRGGBBAA` colour.
    Color(String),
}

/// A declared option for a (source, target) pair (§1.6), supplied by the registry (concrete values in
/// 04-formats). The pipeline renders/collects these generically; the §1.4 options panel (P4.64) renders
/// it and P5-P7 register concrete declarations against it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OptionDecl {
    /// The stable machine key.
    pub key: OptionKey,
    /// The §5 UI-chrome label key (§2.10).
    pub label: LabelKey,
    /// Basic vs Advanced surface tier.
    pub surface: Surface,
    /// The control shape + bounds/choices.
    pub kind: OptionKind,
    /// The no-decision default (from 04-formats).
    pub default: OptionValue,
}

// ─── §2.9 LossyKind — the predictable-loss catalog discriminant (P2.8.2) ─────────
/// The predictable-loss kind keyed by the §2.9.1 catalog (the canonical English note lives in §2.9; this
/// is the ONE canonical name). Carried by `Target.lossy: Option<LossyKind>` (the §1.5 offer-time SINGLE
/// marker) and `OutcomeMsg::Lossy { kind }` (§2.8, P2.20). The §2.9.2 CO-APPLYING set (2-3 kinds rendered
/// together at §5.7) is a SEPARATE render-time computation (P4.65), NOT this single offer marker — §1.5
/// owns the wire field as `Option<LossyKind>`, §2.9.2/§5.7 own the rendered set (the box-note-flagged
/// §1.5-vs-§2.9.2 distinction, surfaced for owner escalation and confirmed an offer-vs-render layering).
///
/// [Derived-Assumption: P2.8 — LossyKind wire form is snake_case (`image_lossy_codec`), derived from the
/// §2.9.1 catalog + the 04-formats cross-references (images/spreadsheets/documents/presentations/audio),
/// which all name the kind in snake_case as a stable cross-referenced catalog key. §0.4.3's camelCase rule
/// governs FIELD names; LossyKind is a fieldless discriminant enum, so its snake_case is a per-catalog
/// discriminant casing, not a §0.4.3 deviation.]
///
/// [Build-Session-Entscheidung: P2.8] Registered standalone in the P1.25 type registry — §2.8.2 (line
/// 1261) EXPLICITLY mandates LossyKind (with OutcomeMsg/ConversionErrorKind) derive `specta::Type` + be
/// registered in `collect_types![]` so `Target.lossy` / `OutcomeMsg.kind` never generate as `any`. Derives
/// both `Serialize` + `Deserialize` (Copy, fieldless) so it round-trips AND embeds in the round-trippable
/// `Target`; the §2.8 sibling enums are Serialize-only, but LossyKind's embedding in a `Deserialize`
/// `Target` requires `Deserialize` here. Variant order matches the §2.9.1 catalog (audio_downmix last).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum LossyKind {
    /// `→ JPG/WEBP(lossy)/HEIC/AVIF` from any source (images.md).
    ImageLossyCodec,
    /// `→ GIF` 256-colour reduction (images.md).
    ImagePalette,
    /// `→ ICO` multi-size icon assembly (images.md).
    ImageDownscale,
    /// alpha source `→ JPG/BMP` transparency flatten (images.md).
    ImageAlphaFlatten,
    /// animated source `→` still target (images.md).
    ImageAnimationFlatten,
    /// `SVG → raster` (images.md).
    ImageSvgRaster,
    /// `DOCX/DOC/ODT/RTF/MD → PDF` and `XLSX/XLS/ODS → PDF` reflow (documents.md / spreadsheets.md).
    DocPdfReflow,
    /// `PDF → TXT` (documents.md).
    DocPdfToText,
    /// `HTML → PDF` (documents.md).
    DocHtmlRender,
    /// `* → TXT` from rich sources (documents.md).
    DocToText,
    /// `* → MD/RTF` from rich sources (documents.md).
    DocSimplified,
    /// `XLSX/XLS/ODS → CSV/TSV` (spreadsheets.md).
    SheetToDelimited,
    /// `* → XLS` legacy format (spreadsheets.md).
    XlsLegacyLimits,
    /// `CSV/TSV → workbook/CSV` non-Unicode encoding (spreadsheets.md).
    TextEncodingNarrowed,
    /// `PPTX/PPT/ODP → PDF` (presentations.md).
    SlidesToPdfFlatten,
    /// ODF↔MS office round-trip + slide re-layout (presentations.md).
    OfficeRoundtripApprox,
    /// `PPTX → PPT` legacy downgrade (presentations.md).
    PptxToPptLegacy,
    /// `→ MP3/AAC/M4A/OGG/OPUS` (audio.md).
    AudioLossyTarget,
    /// lossy source `→` lossy target (audio.md).
    AudioTranscode,
    /// lossy source `→` lossless target (audio.md).
    AudioLossyOrigin,
    /// >16-bit source `→` default 16-bit WAV/AIFF (audio.md).
    AudioBitdepth,
    /// `→ AAC`, partly WAV/AIFF — tags dropped (audio.md).
    AudioTagsDropped,
    /// re-encode disposition (video.md / cross-cat).
    VideoReencode,
    /// WEBM(alpha) `→ MP4/H.264` (video.md).
    VideoAlphaLost,
    /// image/ASS subs `→ MP4` (video.md).
    VideoSubsDropped,
    /// `video → GIF` cross-category, unconditional (cross-category.md).
    VideoToGif,
    /// surround forced to stereo by codec (rare; audio.md).
    AudioDownmix,
}

// ─── §0.6 target scalar/alias layer (the leaf vocabulary, P2.8.3) ────────────────
// [Build-Session-Entscheidung: P2.8] The §0.6 scalar/alias leaf types the P2.8.4 composites key on. Each
// derives specta::Type + camelCase; NOT explicitly registered — deferred to the C3 consumer (P2.25), the
// P2.2-P2.7 defer pattern. Fieldless TargetId/CrossCatOp are Copy; Availability owns a String (not Copy).

/// The offered-target identity (§0.6 / §1.5): a format target or a cross-category operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TargetId {
    /// A format target (e.g. `Format(Webp)`).
    Format(FormatId),
    /// A cross-category operation (`ExtractAudio` | `ToGif`).
    Op(CrossCatOp),
}

/// A format target IS a user-facing format (§0.6) — the alias ties the §1.5 target vocabulary to the
/// single §1.3 grouping key.
pub type FormatId = UserFacingFormat;

/// The closed set of cross-category operations (§0.6 / cross-category.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CrossCatOp {
    /// Extract the audio track from a video.
    ExtractAudio,
    /// Render to an animated GIF.
    ToGif,
}

/// A target's per-platform availability (§0.6 / §3.4 patent disposition, resolved per platform).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Availability {
    /// Offered on this platform.
    Available,
    /// Honestly unavailable here (§3.4 / §5.2) — `reason` names why.
    #[serde(rename_all = "camelCase")]
    Unavailable { reason: String },
}

// ─── §0.6 target composite layer (Target / TargetOffer / OptionValues, P2.8.4) ───
// [Build-Session-Entscheidung: P2.8] The §0.6 composites that compose the scalars + the option/lossy
// families. Each derives specta::Type + camelCase; NOT explicitly registered — deferred to the C3
// `get_targets` consumer (P2.25), which returns `TargetOffer` and auto-registers the whole graph then.

/// An offered output choice for a source (§0.6 / §1.5). `lossy` is the §1.5 offer-time SINGLE
/// predictable-loss marker (`Option<LossyKind>`, ≤1); the §2.9.2 co-applying render-set (2-3 kinds) is a
/// SEPARATE render-time computation (P4.65), not this field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    /// The target identity (e.g. `Format(Webp)` | `Op(ExtractAudio)` | `Op(ToGif)`).
    pub id: TargetId,
    /// The display label.
    pub label: String,
    /// The §1.5 offer-time single predictable-loss marker (§2.9 catalog key; the string lives in §2.9).
    pub lossy: Option<LossyKind>,
    /// Per-platform availability (from §3.4).
    pub availability: Availability,
    /// The §1.6 declared options model (concrete values in 04-formats).
    pub options: Vec<OptionDecl>,
}

/// The C3 `get_targets` return (§0.6 / §1.5) — the offered targets for a collected set plus the
/// exactly-one pre-highlighted default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TargetOffer {
    /// The collected set these targets are offered for.
    pub set: CollectedSetId,
    /// The offered targets.
    pub targets: Vec<Target>,
    /// Exactly ONE pre-highlighted default (§1.5).
    pub default_target: TargetId,
}

/// The effective, fully-defaulted-plus-overrides option set for a batch (§0.6; == §1.6 `EffectiveOptions`).
/// The ONE wire/domain name for the resolved values, keyed by the stable `OptionKey`. Serializes
/// transparently as its inner map (a JSON object keyed by the `OptionKey` slug strings).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct OptionValues(pub BTreeMap<OptionKey, OptionValue>);

// ─── §0.6 destination / output-plan layer (DestinationChoice / OutputPlan / DivertReason, P2.9) ───
// [Build-Session-Entscheidung: P2.9] The §0.6 destination + per-job output-plan vocabulary. `DestinationChoice`
// (the C4/C5/C6 inbound `destination` arg, §0.4.1) and `DivertReason` (carried by the P2.11 wire DTOs
// `OutputPlanPreview`/`DestinationResolved`) are WIRE types: each derives `specta::Type` + camelCase so it
// mirrors to `bindings.ts` once its consumer registers it — NOT explicitly registered here, the established
// P2.2-P2.8 defer pattern (the consuming command/DTO auto-registers the graph: C4/C5 at P2.26/P2.27, the
// `OutputPlanPreview`/`DestinationResolved` DTOs at P2.11). The persisted `lastDestinationMode` string form
// (`"beside-source"`/`"<path>"`, §5/§7.4) is a SEPARATE frontend-side store representation mapped to this enum
// JS-side, NOT this type's wire form — so the uniform camelCase externally-tagged convention applies here.
// `OutputPlan` is the EXCEPTION: it is an INTERNAL plan type (computed by §1.8, consumed by §2.1/§2.14 — never a
// command return; the wire shows `OutputPlanPreview`/`DestinationResolved` instead, §0.6) and it holds `OsString`
// `base_name`/`extension` that MUST preserve the source's exact OS-native bytes (§2.2 base-name-kept). `OsString`
// has no cross-platform-stable JSON form — which is precisely why the plan stays off the wire — so it derives only
// `Debug, Clone, PartialEq, Eq` (no `Serialize`/`Deserialize`/`Type`), unlike the wire types above.

/// Where a batch's outputs are written (§0.6 / §2.7.1) — the C4/C5/C6 `destination` argument (§0.4.1).
/// WebView-held, with no server-side store (§0.11 T2a): the no-harm machinery, not path provenance, is the bound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DestinationChoice {
    /// Beside each source in place — the §2.7.1 default; folder layout is preserved for free and per-location
    /// divert (§2.7.2) still applies to any unwritable/ephemeral source.
    BesideSource,
    /// A single user-chosen root under which the dropped-selection-relative subtree is re-created (§2.7.1, not
    /// flattened). A re-validated HINT, never a guarantee — §2.7.2 / §7.4.1 re-check writability + divert at use time.
    ChosenRoot(PathBuf),
}

/// Why a single source's output was diverted away from its intended location (§0.6 / §2.7.2). Carried by the
/// P2.11 wire DTOs (`OutputPlanPreview`/`DestinationResolved`); on `OutputPlan`, `None` = beside-source (no divert).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DivertReason {
    /// The intended location could not be written — read-only USB / network share / restricted folder (§2.7.2).
    Unwritable,
    /// The intended location is a known-ephemeral OS temp place the OS may silently purge (§2.7.2) — writing a
    /// result there would lose the user's output.
    Ephemeral,
    /// The destination filesystem accepts a create but offers NO atomic create-only no-clobber publish primitive
    /// (FAT/exFAT-class: neither `RENAME_NOREPLACE`-class no-replace rename NOR hardlinks). Unix-only — Windows'
    /// `MoveFileExW` is create-only on FAT/exFAT (§2.7.2 / §2.14.2).
    NoAtomicPublish,
}

/// The per-job output plan (§0.6; §1.8 computes it, §2.1/§2.14 consume it). DIRECTORY-BASED by design: the exact
/// final name + no-clobber numbering is resolved LAZILY at write time on the RESOLVED real file (§2.1 exclusive
/// create) — there is deliberately NO pre-baked `final_path`/`temp_path` (a pre-numbered path would reintroduce the
/// §2.1.2 TOCTOU race). Internal-only (not a wire type) — see the section note above for why it carries no serde/specta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputPlan {
    /// The job this plan is for — the item's `ItemId` (§0.6 names this the `JobId` alias, `pub type JobId = ItemId`;
    /// the sibling §0.6 `ConversionJob.item` is likewise spelled `ItemId`). [Build-Session-Entscheidung: P2.9] spelled
    /// as the underlying `ItemId` rather than the `JobId` alias: it is the SAME type, and `OutputPlan` is the alias's
    /// first PRODUCTION user — referencing the (otherwise-dead) `JobId` alias here trips a rustc dead-code
    /// lint-expectation interaction with this module's forward-declaration suppression (type aliases have incomplete
    /// dead-code lint-expectation support), which using the concrete type avoids with no semantic change.
    pub job: ItemId,
    /// The resolved output directory — beside-source OR a §2.7 divert target.
    pub final_dir: PathBuf,
    /// `Some(reason)` if this item's location was diverted (§2.7.2); `None` = beside-source.
    pub diverted: Option<DivertReason>,
    /// The SOURCE base name, kept exactly (§2.2) — OS-native bytes preserved.
    pub base_name: OsString,
    /// The extension from the chosen TARGET (§2.2).
    pub extension: OsString,
    /// Where the kind-1 publish temp (`*.part`) lives — a uniquely-named sibling DOTFILE inside `final_dir`, on the
    /// SAME volume as `final_dir` by construction, so the §2.1 publish is a true intra-volume atomic rename. EQUALS
    /// `final_dir` in v1 (§2.14.1). (The kind-2 engine-working scratch root, §2.14.2, may be on another volume and is
    /// NOT carried here.)
    pub publish_temp_dir: PathBuf,
}

// ─── §0.6 JobStage — the coarse per-item progress stage (P2.10) ───────────────────
/// The coarse per-item progress stage (§0.6), carried by the §0.4.2 `ItemProgress` Channel event; §1.11
/// owns the per-engine semantics, this is the shared/wire enum NAME. Homed in `crate::domain` (the tier-3
/// leaf) because it references NO `crate::outcome` type (§0.7 ‡, P2.10) — unlike its sibling lifecycle
/// types `Batch`/`ConversionJob`/`JobState`, which reference the §2.8 kind and so are homed in
/// `crate::orchestrator` (tier 1).
///
/// [Build-Session-Entscheidung: P2.10] A WIRE enum: derives `specta::Type` (so `ItemProgress.stage`
/// mirrors to `bindings.ts` as a named type, never `any`) + `Serialize` with `#[serde(rename_all =
/// "camelCase")]` (`spawning`/`decoding`/`encoding`/`writing`). OUTBOUND-ONLY — the `ItemProgress` Channel
/// event is sent Rust→WebView and never deserialized inbound, so NO `Deserialize` (mirroring the
/// outbound-only `ScanProgress` (P2.7) + `ConversionErrorKind` (P2.18) derive choice). `Copy` (fieldless).
/// Registration is DEFERRED to the C6 `ConversionEvent`/`ItemProgress` consumer (P2.37), the established
/// P2.2-P2.9 defer pattern (the no-`any` guarantee is the `Type` derive, not an early consumer-less
/// registration that would churn `bindings.ts` ahead of its event).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum JobStage {
    /// The engine subprocess is being spawned (§1.7/§2.12).
    Spawning,
    /// The source is being decoded.
    Decoding,
    /// The target is being encoded.
    Encoding,
    /// The output is being written + atomically published (§2.1).
    Writing,
}

// ─── §0.6 re-run DTOs — the §2.5 batch-level prompt + decision (P2.11) ─────────────
// [Build-Session-Entscheidung: P2.11] The two genuinely outcome-FREE §0.6 command-return DTOs of the §2.5
// re-run flow stay in `crate::domain` (the tier-3 leaf): `RerunPrompt` (outbound prompt data) and
// `RerunDecision` (the C6 inbound choice). Their outcome-referencing siblings — `OutputPlanPreview` /
// `DestinationResolved` (embed `PreflightVerdict` → transitively reference `crate::outcome`) — are homed in
// `crate::orchestrator` (§0.7 ‡ "directly OR transitively"; the orchestrator previews embed these two via a
// downward `orchestrator`→`domain` edge). Each derives `specta::Type` + camelCase; registration rides
// the consuming command (C4/C6, P2.26/P2.29), the established P2.2-P2.9 defer pattern.

/// The one batch-level §2.5 re-run prompt's data (§0.6 / §2.5) — surfaced once per batch when the
/// in-session ledger detects an equivalent prior run (same resolved source + target + effective settings,
/// §2.5.1). OUTBOUND-ONLY: it is carried inside the C4/C5 `OutputPlanPreview` / `DestinationResolved`
/// returns (Rust→WebView), never sent inbound — so `Serialize` + `Type` with NO `Deserialize` (mirroring
/// the outbound-only `ScanProgress` (P2.7) derive choice). The user's RESPONSE is the separate inbound
/// `RerunDecision`.
///
/// [Build-Session-Entscheidung: P2.11] NOT `Copy` (the established struct convention, like `ScanProgress`);
/// `PartialEq` + `Eq` back the embedding `OutputPlanPreview` / `DestinationResolved` equality + the
/// serialize pin. camelCase renames `equivalent_count` → `equivalentCount`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RerunPrompt {
    /// How many items in the batch are flagged equivalent to a prior in-session run (§2.5).
    pub equivalent_count: usize,
}

/// The C6 `start_conversion` re-run decision (§0.6 / §2.5) — the user's answer to the `RerunPrompt`.
/// INBOUND (WebView → Rust, a C6 input), so it derives `Deserialize`. `Skip` is the SAFE DEFAULT (produce
/// no new output for the equivalent items); `FreshCopy` makes fresh numbered copies (§2.5). Any change to
/// target/settings is a new conversion using ordinary numbering, not a re-run decision.
///
/// [Build-Session-Entscheidung: P2.11] Round-trippable (`Serialize` + `Deserialize`) because it crosses IPC
/// inbound (the C6 arg); `Copy` (fieldless enum); camelCase wire form (`skip` / `freshCopy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RerunDecision {
    /// The safe default — produce no new output for the equivalent items (§2.5).
    Skip,
    /// Make fresh numbered copies of the equivalent items (§2.5).
    FreshCopy,
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use proptest::test_runner::{RngAlgorithm, TestRng, TestRunner};
    use std::collections::BTreeSet;

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

    // §6.4.1 unit (G15): the §7.1.2 RunId minting contract — a fresh, non-nil v4 per run. The mint POINT is
    // C6 `start_conversion` accept (NOT the §2.4 freeze, which yields the CollectedSetId — §0.4.4); this box
    // (P2.48) fixes the point + adds the mechanism, the at-C6-accept call site is P3.46. Mirrors the
    // InstanceId mint test.
    #[test]
    fn run_id_mint_is_unique_nonnil_v4() {
        let a = RunId::mint();
        let b = RunId::mint();
        assert_ne!(a, b, "each run mints a distinct RunId (§7.1.2)");
        assert_ne!(a.0, Uuid::nil(), "a minted RunId is never the nil UUID");
        assert_eq!(
            a.0.get_version_num(),
            4,
            "§7.1.2: RunId is a v4 (random) UUID"
        );
    }

    // §6.4.1 unit (G15): the §7.1.2/§2.14 scratch-root identity SEGMENTS (P2.49) — the per-instance root is
    // <InstanceId>.<pid> (the PID a human-readable LABEL, never the liveness gate — §2.6.3) and the per-run
    // subdir is run-<RunId>; both shapes are exactly what the §2.6.3 startup-sweep glob
    // `convertia/scratch/<*>.<*>/run-*` matches. The §2.14 path assembly + the §2.6 scratch lifecycle are
    // crate::run (P3.1.2); this pins the identity embedding. The pid is PASSED IN (a pure formatter).
    #[test]
    fn scratch_root_and_run_subdir_identity_segments() {
        assert_eq!(
            InstanceId(Uuid::nil()).scratch_root_segment(12345),
            "00000000-0000-0000-0000-000000000000.12345",
            "§7.1.2/§2.14: the per-instance scratch root is <InstanceId>.<pid> (matches the §2.6.3 glob <*>.<*>)"
        );
        assert_eq!(
            RunId(Uuid::nil()).run_subdir_segment(),
            "run-00000000-0000-0000-0000-000000000000",
            "§7.1.2/§2.14: the per-run subdir is run-<RunId> (matches the §2.6.3 glob run-*)"
        );
    }

    // §6.4.1 unit (G15): the P3.20 crate-internal id accessors/reconstructors the publish-temp naming
    // model composes (§2.6.1 / §2.14.1) — `as_uuid`/`from_uuid` (InstanceId, RunId) and `as_u32` (ItemId)
    // are exact inverses, so a `(InstanceId, RunId, JobId)` triple survives a render→parse round-trip
    // (the `.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part` ownership encoding `crate::run` reads back
    // to resolve the §2.6.3 owning lock). `from_uuid` does NOT re-assert v4-ness — it reconstructs a
    // possibly-foreign, arbitrary UUID (here the nil UUID, which is not v4) verbatim.
    #[test]
    fn publish_temp_id_accessors_round_trip() {
        let inst = InstanceId::mint();
        assert_eq!(
            InstanceId::from_uuid(inst.as_uuid()),
            inst,
            "§2.6.1: InstanceId survives as_uuid → from_uuid unchanged"
        );
        let run = RunId::mint();
        assert_eq!(
            RunId::from_uuid(run.as_uuid()),
            run,
            "§2.6.1: RunId survives as_uuid → from_uuid unchanged"
        );
        assert_eq!(
            ItemId::from_index(ItemId::from_index(4_294_967_295).as_u32()),
            ItemId::from_index(u32::MAX),
            "§0.6: ItemId survives from_index → as_u32 unchanged, incl. the u32::MAX boundary"
        );
        // `from_uuid` reconstructs an arbitrary (non-v4, here nil) UUID verbatim — it re-reads a foreign
        // owner, it does not mint (§2.6.1): the reconstruction is exactly the parsed identifier.
        assert_eq!(
            InstanceId::from_uuid(Uuid::nil()).as_uuid(),
            Uuid::nil(),
            "§2.6.1: from_uuid re-reads a foreign/non-v4 identifier verbatim (no v4 re-assertion)"
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

    // §6.4.1 unit (G15): the §0.6 `SkipReason` enum — the four detection-ineligible skip classes, each
    // serializing in the §0.4.3 camelCase wire form (`unsupportedType`/`uncertain`/`empty`/`unreadable`),
    // locked by a serialize→deserialize round-trip. The no-wildcard `exhaustive` arm locks MEMBERSHIP: an
    // added/removed variant fails to compile here (the anti-drift "lock the contract" discipline).
    #[test]
    fn skip_reason_wire_form_is_camelcase_and_roundtrips() {
        for (reason, wire) in [
            (SkipReason::UnsupportedType, "\"unsupportedType\""),
            (SkipReason::Uncertain, "\"uncertain\""),
            (SkipReason::Empty, "\"empty\""),
            (SkipReason::Unreadable, "\"unreadable\""),
        ] {
            let json = serde_json::to_string(&reason).expect("SkipReason serializes");
            assert_eq!(json, wire, "§0.4.3: SkipReason wire casing is camelCase");
            let back: SkipReason =
                serde_json::from_str(&json).expect("SkipReason round-trips from its wire form");
            assert_eq!(
                back, reason,
                "§0.6: SkipReason round-trips through its wire form"
            );
        }
        fn exhaustive(r: SkipReason) {
            match r {
                SkipReason::UnsupportedType
                | SkipReason::Uncertain
                | SkipReason::Empty
                | SkipReason::Unreadable => {}
            }
        }
        exhaustive(SkipReason::Empty);
    }

    // §6.4.1 unit (G15): the §1.2/§1.3 `DetectionOutcome → SkipReason` projection (P2.16) — the eligible
    // `Recognized` outcome has NO skip reason (`None`, it is a batch member), and each of the four INELIGIBLE
    // outcomes projects to its identically-named §0.6 `SkipReason`. Exercises all five variants, so adding a
    // `DetectionOutcome` variant (which would break the exhaustive match in `skip_reason`) is caught here too.
    #[test]
    fn detection_outcome_projects_to_skip_reason() {
        // eligible: a Recognized outcome (High OR Low confidence) is a batch member, never skipped.
        assert_eq!(
            DetectionOutcome::Recognized {
                format: UserFacingFormat::Csv,
                confidence: Confidence::High,
                dims: None,
            }
            .skip_reason(),
            None,
            "§1.3: an eligible High-confidence Recognized outcome has no skip reason"
        );
        assert_eq!(
            DetectionOutcome::Recognized {
                format: UserFacingFormat::Png,
                confidence: Confidence::Low,
                dims: Some((16, 16)),
            }
            .skip_reason(),
            None,
            "§1.2: a Low-confidence Recognized is still eligible (Low is a first-class Recognized, not a skip)"
        );
        // ineligible: each projects to its identically-named SkipReason.
        assert_eq!(
            DetectionOutcome::UnsupportedType {
                detected: "PostScript".to_owned(),
            }
            .skip_reason(),
            Some(SkipReason::UnsupportedType),
            "§1.2/§1.3: UnsupportedType → SkipReason::UnsupportedType (by name)"
        );
        assert_eq!(
            DetectionOutcome::Uncertain { best_guess: None }.skip_reason(),
            Some(SkipReason::Uncertain),
            "§1.2/§1.3: Uncertain → SkipReason::Uncertain (by name)"
        );
        assert_eq!(
            DetectionOutcome::Empty.skip_reason(),
            Some(SkipReason::Empty),
            "§1.2/§1.3: Empty → SkipReason::Empty (by name)"
        );
        assert_eq!(
            DetectionOutcome::Unreadable {
                reason: ReadFailure::NotFound,
            }
            .skip_reason(),
            Some(SkipReason::Unreadable),
            "§1.2/§1.3: Unreadable → SkipReason::Unreadable (by name)"
        );
    }

    // §6.4.1 unit (G15): the §1.3/§0.6 `EmptyReport` contract (P2.17) — the all-ineligible-drop report
    // carries every item's §1.2 `DetectionResult` (in freeze order) so the §1.3 projection can build the
    // per-item skip reasons. INTERNAL type (no wire form): construction + field-access + the all-ineligible
    // precondition (each outcome projects to a `SkipReason` via P2.16's `skip_reason`), no serde round-trip.
    #[test]
    fn empty_report_carries_the_per_item_detection_outcomes() {
        let report = EmptyReport {
            outcomes: vec![
                DetectionResult {
                    item: ItemId(0),
                    outcome: DetectionOutcome::Empty,
                },
                DetectionResult {
                    item: ItemId(1),
                    outcome: DetectionOutcome::Unreadable {
                        reason: ReadFailure::PermissionDenied,
                    },
                },
            ],
        };
        assert_eq!(
            report.outcomes.len(),
            2,
            "§1.3: EmptyReport holds every dropped item's detection result"
        );
        assert_eq!(
            report.outcomes.first().map(|r| r.item),
            Some(ItemId(0)),
            "§0.6: the report preserves the single-id-space ids in freeze order"
        );
        // every carried outcome is ineligible (the all-ineligible-drop precondition) → each projects to a
        // `SkipReason` (P2.16), which the §1.3 projection reads to build the `Empty { skipped }` tally.
        for r in &report.outcomes {
            assert!(
                r.outcome.skip_reason().is_some(),
                "§1.3: every EmptyReport outcome is ineligible, so it projects to a SkipReason"
            );
        }
    }

    // §6.4.1 unit (G15): the §0.6 `SkippedItem` record — the id-disjoint ineligible-item view. Locks the
    // §0.4.3 camelCase wire form of all three fields (`item`/`source`/`reason`) + a serialize→deserialize
    // round-trip; the struct literal is the compile-time field-set lock. A bare filename keeps the
    // `PathBuf` wire form platform-independent (no Windows backslash divergence).
    #[test]
    fn skipped_item_wire_form_is_camelcase_and_roundtrips() {
        let skipped = SkippedItem {
            item: ItemId(5),
            source: PathBuf::from("notes.xyz"),
            reason: SkipReason::UnsupportedType,
        };
        let json = serde_json::to_string(&skipped).expect("SkippedItem serializes");
        assert_eq!(
            json, r#"{"item":5,"source":"notes.xyz","reason":"unsupportedType"}"#,
            "§0.4.3/§0.6: SkippedItem is {{ item, source, reason }} in camelCase wire form"
        );
        let back: SkippedItem = serde_json::from_str(&json).expect("SkippedItem round-trips");
        assert_eq!(
            back, skipped,
            "§0.6: SkippedItem round-trips through its wire form"
        );
    }

    // §6.4.1 unit (G15): the §1.4 `CollectedNoteKind` discriminant — the four typed producers + the
    // reserved `Other`, each serializing in the §0.4.3 camelCase wire form, round-tripped. The no-wildcard
    // `exhaustive` arm locks MEMBERSHIP (an added/removed variant fails to compile).
    #[test]
    fn collected_note_kind_wire_form_is_camelcase_and_roundtrips() {
        for (kind, wire) in [
            (CollectedNoteKind::MultipleSheets, "\"multipleSheets\""),
            (CollectedNoteKind::AnimatedSource, "\"animatedSource\""),
            (CollectedNoteKind::MultiSizeIcon, "\"multiSizeIcon\""),
            (CollectedNoteKind::EmbeddedCoverArt, "\"embeddedCoverArt\""),
            (CollectedNoteKind::Other, "\"other\""),
        ] {
            let json = serde_json::to_string(&kind).expect("CollectedNoteKind serializes");
            assert_eq!(
                json, wire,
                "§0.4.3: CollectedNoteKind wire casing is camelCase"
            );
            let back: CollectedNoteKind = serde_json::from_str(&json)
                .expect("CollectedNoteKind round-trips from its wire form");
            assert_eq!(
                back, kind,
                "§1.4: CollectedNoteKind round-trips through its wire form"
            );
        }
        fn exhaustive(k: CollectedNoteKind) {
            match k {
                CollectedNoteKind::MultipleSheets
                | CollectedNoteKind::AnimatedSource
                | CollectedNoteKind::MultiSizeIcon
                | CollectedNoteKind::EmbeddedCoverArt
                | CollectedNoteKind::Other => {}
            }
        }
        exhaustive(CollectedNoteKind::Other);
    }

    // §6.4.1 unit (G15): the §1.4 `CollectedNote` record — { kind, detail } in camelCase, with both the
    // `detail: Some` and `detail: None` (→ JSON null) cases round-tripped.
    #[test]
    fn collected_note_wire_form_is_camelcase_and_roundtrips() {
        let note = CollectedNote {
            kind: CollectedNoteKind::MultipleSheets,
            detail: Some("3 sheets".to_owned()),
        };
        assert_eq!(
            serde_json::to_string(&note).expect("CollectedNote serializes"),
            r#"{"kind":"multipleSheets","detail":"3 sheets"}"#,
            "§1.4: CollectedNote is {{ kind, detail }} in camelCase wire form"
        );
        let bare = CollectedNote {
            kind: CollectedNoteKind::AnimatedSource,
            detail: None,
        };
        assert_eq!(
            serde_json::to_string(&bare).expect("CollectedNote(None) serializes"),
            r#"{"kind":"animatedSource","detail":null}"#,
            "§1.4: a value-less note carries detail=null"
        );
        for n in [note, bare] {
            let json = serde_json::to_string(&n).expect("CollectedNote serializes");
            let back: CollectedNote =
                serde_json::from_str(&json).expect("CollectedNote round-trips");
            assert_eq!(
                back, n,
                "§1.4: CollectedNote round-trips through its wire form"
            );
        }
    }

    // §6.4.1 unit (G15): the §0.6 `CollectedSet` enum — the C1/C2a return + §1.4 confirm shape. The
    // `Single` variant locks the FULL confirm-summary wire shape incl. the camelCase
    // `totalBytes`/`encodingHint`/`delimiterHint` field renames (serde does NOT cascade the enum-level
    // rename to struct-variant fields, so the per-variant attr is load-bearing) and the externally-tagged
    // `{"single":{…}}` form embedding a DroppedItem/SkippedItem/CollectedNote; the four simpler variants
    // lock their own externally-tagged forms (incl. the Mixed tuple → `[fmt, count]` array). Every variant
    // round-trips, and the no-wildcard `exhaustive` arm locks variant MEMBERSHIP. `Uuid::nil()` keeps the
    // id fields deterministic.
    #[test]
    fn collected_set_wire_forms_and_membership() {
        let single = CollectedSet::Single {
            id: CollectedSetId(Uuid::nil()),
            instance: InstanceId(Uuid::nil()),
            format: UserFacingFormat::Csv,
            items: vec![DroppedItem {
                item: ItemId(0),
                raw_path: PathBuf::from("data.csv"),
                resolved_path: PathBuf::from("data.csv"),
                size_bytes: 2048,
                detected: DetectionOutcome::Recognized {
                    format: UserFacingFormat::Csv,
                    confidence: Confidence::High,
                    dims: None,
                },
            }],
            count: 1,
            skipped: vec![SkippedItem {
                item: ItemId(1),
                source: PathBuf::from("notes.xyz"),
                reason: SkipReason::UnsupportedType,
            }],
            total_bytes: 2048,
            roots: vec![PathBuf::from("folder")],
            encoding_hint: Some("Windows-1252".to_owned()),
            delimiter_hint: Some(";".to_owned()),
            notes: vec![CollectedNote {
                kind: CollectedNoteKind::MultipleSheets,
                detail: Some("3 sheets".to_owned()),
            }],
        };
        assert_eq!(
            serde_json::to_string(&single).expect("Single serializes"),
            r#"{"single":{"id":"00000000-0000-0000-0000-000000000000","instance":"00000000-0000-0000-0000-000000000000","format":"csv","items":[{"item":0,"rawPath":"data.csv","resolvedPath":"data.csv","sizeBytes":2048,"detected":{"recognized":{"format":"csv","confidence":"high","dims":null}}}],"count":1,"skipped":[{"item":1,"source":"notes.xyz","reason":"unsupportedType"}],"totalBytes":2048,"roots":["folder"],"encodingHint":"Windows-1252","delimiterHint":";","notes":[{"kind":"multipleSheets","detail":"3 sheets"}]}}"#,
            "§0.4.3/§0.6/§1.4: CollectedSet::Single is the full externally-tagged camelCase confirm-summary wire shape"
        );
        let mixed = CollectedSet::Mixed {
            found: vec![(UserFacingFormat::Jpg, 3), (UserFacingFormat::Png, 2)],
        };
        assert_eq!(
            serde_json::to_string(&mixed).expect("Mixed serializes"),
            r#"{"mixed":{"found":[["jpg",3],["png",2]]}}"#,
            "§1.3: Mixed lists each found (format, count) as a [tag, n] array"
        );
        let unsupported = CollectedSet::Unsupported {
            detected: "PostScript".to_owned(),
        };
        assert_eq!(
            serde_json::to_string(&unsupported).expect("Unsupported serializes"),
            r#"{"unsupported":{"detected":"PostScript"}}"#,
            "§1.2: Unsupported names the detected type"
        );
        let uncertain = CollectedSet::Uncertain {
            note: "could be tiff or raw".to_owned(),
        };
        assert_eq!(
            serde_json::to_string(&uncertain).expect("Uncertain serializes"),
            r#"{"uncertain":{"note":"could be tiff or raw"}}"#,
            "§1.2: Uncertain carries the can't-tell note"
        );
        let empty = CollectedSet::Empty { skipped: vec![] };
        assert_eq!(
            serde_json::to_string(&empty).expect("Empty serializes"),
            r#"{"empty":{"skipped":[]}}"#,
            "§1.3: a genuinely-zero-items Empty carries an empty skipped vec"
        );

        for set in [single, mixed, unsupported, uncertain, empty] {
            let json = serde_json::to_string(&set).expect("CollectedSet serializes");
            let back: CollectedSet = serde_json::from_str(&json).expect("CollectedSet round-trips");
            assert_eq!(
                back, set,
                "§0.6: CollectedSet round-trips through its wire form"
            );
        }

        // Compiler-enforced membership: no wildcard arm (the crate denies wildcard_enum_match_arm).
        fn exhaustive(s: &CollectedSet) {
            match s {
                CollectedSet::Single { .. }
                | CollectedSet::Mixed { .. }
                | CollectedSet::Unsupported { .. }
                | CollectedSet::Uncertain { .. }
                | CollectedSet::Empty { .. } => {}
            }
        }
        exhaustive(&CollectedSet::Empty { skipped: vec![] });
    }

    // §6.4.1 unit (G15): the §0.4.4 `FrozenCollectedSet::from_collected` projection (P2.44). A `Single`
    // projects to `Some` carrying EVERY payload field verbatim (the registry stores the full Single
    // payload so C3 reads `format`, C4/C5 plan against `roots`, C6 rebuilds from `items` — §0.4.4); each
    // of the four terminal pre-flight outcomes (Mixed/Unsupported/Uncertain/Empty) projects to `None`
    // (no resolvable CollectedSetId — §0.6 invariant 3 / §0.4.4 "only a Single yields a resolvable id").
    // The non-trivial values in EVERY field (2 items, a skip, both hints Some, a note, populated roots)
    // prove the projection copies each field, not just the easy ones.
    #[test]
    fn frozen_collected_set_projects_only_single_with_full_payload() {
        let items = vec![
            DroppedItem {
                item: ItemId(0),
                raw_path: PathBuf::from("a.csv"),
                resolved_path: PathBuf::from("/abs/a.csv"),
                size_bytes: 2048,
                detected: DetectionOutcome::Recognized {
                    format: UserFacingFormat::Csv,
                    confidence: Confidence::High,
                    dims: None,
                },
            },
            DroppedItem {
                item: ItemId(1),
                raw_path: PathBuf::from("b.csv"),
                resolved_path: PathBuf::from("/abs/b.csv"),
                size_bytes: 4096,
                detected: DetectionOutcome::Recognized {
                    format: UserFacingFormat::Csv,
                    confidence: Confidence::High,
                    dims: None,
                },
            },
        ];
        let skipped = vec![SkippedItem {
            item: ItemId(2),
            source: PathBuf::from("notes.xyz"),
            reason: SkipReason::UnsupportedType,
        }];
        let roots = vec![PathBuf::from("/abs")];
        let notes = vec![CollectedNote {
            kind: CollectedNoteKind::MultipleSheets,
            detail: Some("3 sheets".to_owned()),
        }];
        let single = CollectedSet::Single {
            id: CollectedSetId(Uuid::nil()),
            instance: InstanceId(Uuid::nil()),
            format: UserFacingFormat::Csv,
            items: items.clone(),
            count: 2,
            skipped: skipped.clone(),
            total_bytes: 6144,
            roots: roots.clone(),
            encoding_hint: Some("Windows-1252".to_owned()),
            delimiter_hint: Some(";".to_owned()),
            notes: notes.clone(),
        };

        let frozen = FrozenCollectedSet::from_collected(&single)
            .expect("§0.4.4: a CollectedSet::Single projects to a FrozenCollectedSet");
        assert_eq!(frozen.id, CollectedSetId(Uuid::nil()), "id carried through");
        assert_eq!(
            frozen.instance,
            InstanceId(Uuid::nil()),
            "instance carried through"
        );
        assert_eq!(
            frozen.format,
            UserFacingFormat::Csv,
            "§0.4.4: format carried (C3 get_targets reads it)"
        );
        assert_eq!(
            frozen.items, items,
            "§0.4.4: frozen items carried (C6 rebuilds the Batch from them)"
        );
        assert_eq!(
            frozen.count, 2,
            "count carried (== items.len(), the Single invariant)"
        );
        assert_eq!(
            frozen.skipped, skipped,
            "§0.6: the id-disjoint skipped view carried"
        );
        assert_eq!(frozen.total_bytes, 6144, "§1.10: the size hint carried");
        assert_eq!(
            frozen.roots, roots,
            "§2.7: the dropped roots carried (C4/C5 plan against them)"
        );
        assert_eq!(
            frozen.encoding_hint.as_deref(),
            Some("Windows-1252"),
            "the encoding hint carried"
        );
        assert_eq!(
            frozen.delimiter_hint.as_deref(),
            Some(";"),
            "the delimiter hint carried"
        );
        assert_eq!(
            frozen.notes, notes,
            "§1.4: the structural-peek notes carried"
        );

        // The four terminal pre-flight outcomes are NOT registrable — no resolvable CollectedSetId.
        for terminal in [
            CollectedSet::Mixed {
                found: vec![(UserFacingFormat::Jpg, 2), (UserFacingFormat::Png, 1)],
            },
            CollectedSet::Unsupported {
                detected: "PostScript".to_owned(),
            },
            CollectedSet::Uncertain {
                note: "could be tiff or raw".to_owned(),
            },
            CollectedSet::Empty { skipped: vec![] },
        ] {
            assert!(
                FrozenCollectedSet::from_collected(&terminal).is_none(),
                "§0.4.4/§0.6 invariant 3: a non-Single outcome has no resolvable CollectedSetId, so it is never frozen"
            );
        }
    }

    // §6.4.1 unit (G15): the C2a `PickKind` arg — Files/Folder in the §0.4.3 camelCase wire form,
    // round-tripped; the no-wildcard `exhaustive` arm locks membership.
    #[test]
    fn pick_kind_wire_form_is_camelcase_and_roundtrips() {
        for (kind, wire) in [
            (PickKind::Files, "\"files\""),
            (PickKind::Folder, "\"folder\""),
        ] {
            let json = serde_json::to_string(&kind).expect("PickKind serializes");
            assert_eq!(json, wire, "§0.4.1: PickKind wire casing is camelCase");
            let back: PickKind =
                serde_json::from_str(&json).expect("PickKind round-trips from its wire form");
            assert_eq!(
                back, kind,
                "§0.6: PickKind round-trips through its wire form"
            );
        }
        fn exhaustive(k: PickKind) {
            match k {
                PickKind::Files | PickKind::Folder => {}
            }
        }
        exhaustive(PickKind::Files);
    }

    // §6.4.1 unit (G15): the C9 `OpenKind` arg — Folder/File/RevealInFolder in camelCase (`revealInFolder`
    // is the multi-word lock), round-tripped; the no-wildcard `exhaustive` arm locks membership.
    #[test]
    fn open_kind_wire_form_is_camelcase_and_roundtrips() {
        for (kind, wire) in [
            (OpenKind::Folder, "\"folder\""),
            (OpenKind::File, "\"file\""),
            (OpenKind::RevealInFolder, "\"revealInFolder\""),
        ] {
            let json = serde_json::to_string(&kind).expect("OpenKind serializes");
            assert_eq!(json, wire, "§0.4.1: OpenKind wire casing is camelCase");
            let back: OpenKind =
                serde_json::from_str(&json).expect("OpenKind round-trips from its wire form");
            assert_eq!(
                back, kind,
                "§7.7: OpenKind round-trips through its wire form"
            );
        }
        fn exhaustive(k: OpenKind) {
            match k {
                OpenKind::Folder | OpenKind::File | OpenKind::RevealInFolder => {}
            }
        }
        exhaustive(OpenKind::File);
    }

    // §6.4.1 unit (G15): the app://intake `IntakePayload` — { paths, origin } in camelCase wire form
    // (origin reusing the §0.6 `IntakeOrigin` camelCase, e.g. `launchArg`), round-tripped.
    #[test]
    fn intake_payload_wire_form_is_camelcase_and_roundtrips() {
        let payload = IntakePayload {
            paths: vec![PathBuf::from("a.jpg"), PathBuf::from("b.png")],
            origin: IntakeOrigin::LaunchArg,
        };
        let json = serde_json::to_string(&payload).expect("IntakePayload serializes");
        assert_eq!(
            json, r#"{"paths":["a.jpg","b.png"],"origin":"launchArg"}"#,
            "§0.4.2/§7.8.1: IntakePayload is {{ paths, origin }} in camelCase wire form"
        );
        let back: IntakePayload = serde_json::from_str(&json).expect("IntakePayload round-trips");
        assert_eq!(
            back, payload,
            "§7.8.1: IntakePayload round-trips through its wire form"
        );
    }

    // §6.4.1 unit (G15): the C1 onScan `ScanProgress` Channel payload — { scanned } wire form. It is
    // OUTBOUND-ONLY (Serialize, no Deserialize per the §0.6 literal), so this locks the SERIALIZED form,
    // not a round-trip — the frontend receives this throttled live count but never sends it back.
    #[test]
    fn scan_progress_serializes_to_scanned_count() {
        let json =
            serde_json::to_string(&ScanProgress { scanned: 42 }).expect("ScanProgress serializes");
        assert_eq!(
            json, r#"{"scanned":42}"#,
            "§0.4.2: ScanProgress is {{ scanned }} on the wire (the throttled live count)"
        );
    }

    // §6.4.1 unit (G15): the §2.9 `LossyKind` catalog discriminant — every one of the 27 §2.9.1 kinds
    // serializes in the SNAKE_CASE wire form the catalog + the 04-formats cross-refs name (NOT camelCase —
    // §0.4.3 governs field names, this is a fieldless catalog-key enum), round-tripped. The no-wildcard
    // `exhaustive` arm locks variant MEMBERSHIP: a kind added/removed (or a 04 matrix flag pointing at a
    // missing kind) fails to compile here. Order matches the §2.9.1 catalog.
    #[test]
    fn lossy_kind_snake_case_wire_and_membership() {
        let all: &[(LossyKind, &str)] = &[
            (LossyKind::ImageLossyCodec, "image_lossy_codec"),
            (LossyKind::ImagePalette, "image_palette"),
            (LossyKind::ImageDownscale, "image_downscale"),
            (LossyKind::ImageAlphaFlatten, "image_alpha_flatten"),
            (LossyKind::ImageAnimationFlatten, "image_animation_flatten"),
            (LossyKind::ImageSvgRaster, "image_svg_raster"),
            (LossyKind::DocPdfReflow, "doc_pdf_reflow"),
            (LossyKind::DocPdfToText, "doc_pdf_to_text"),
            (LossyKind::DocHtmlRender, "doc_html_render"),
            (LossyKind::DocToText, "doc_to_text"),
            (LossyKind::DocSimplified, "doc_simplified"),
            (LossyKind::SheetToDelimited, "sheet_to_delimited"),
            (LossyKind::XlsLegacyLimits, "xls_legacy_limits"),
            (LossyKind::TextEncodingNarrowed, "text_encoding_narrowed"),
            (LossyKind::SlidesToPdfFlatten, "slides_to_pdf_flatten"),
            (LossyKind::OfficeRoundtripApprox, "office_roundtrip_approx"),
            (LossyKind::PptxToPptLegacy, "pptx_to_ppt_legacy"),
            (LossyKind::AudioLossyTarget, "audio_lossy_target"),
            (LossyKind::AudioTranscode, "audio_transcode"),
            (LossyKind::AudioLossyOrigin, "audio_lossy_origin"),
            (LossyKind::AudioBitdepth, "audio_bitdepth"),
            (LossyKind::AudioTagsDropped, "audio_tags_dropped"),
            (LossyKind::VideoReencode, "video_reencode"),
            (LossyKind::VideoAlphaLost, "video_alpha_lost"),
            (LossyKind::VideoSubsDropped, "video_subs_dropped"),
            (LossyKind::VideoToGif, "video_to_gif"),
            (LossyKind::AudioDownmix, "audio_downmix"),
        ];
        assert_eq!(all.len(), 27, "§2.9.1: the LossyKind catalog has 27 kinds");
        for (kind, wire) in all {
            let json = serde_json::to_string(kind).expect("LossyKind serializes");
            assert_eq!(
                json,
                format!("\"{wire}\""),
                "§2.9.1: {kind:?} wire form must be snake_case `{wire}`"
            );
            let back: LossyKind =
                serde_json::from_str(&json).expect("LossyKind round-trips from its wire form");
            assert_eq!(
                back, *kind,
                "§2.9: {kind:?} round-trips through its wire form"
            );
        }
        // Compiler-enforced membership (no wildcard arm): a variant add/remove fails to compile here.
        fn exhaustive(k: LossyKind) {
            match k {
                LossyKind::ImageLossyCodec
                | LossyKind::ImagePalette
                | LossyKind::ImageDownscale
                | LossyKind::ImageAlphaFlatten
                | LossyKind::ImageAnimationFlatten
                | LossyKind::ImageSvgRaster
                | LossyKind::DocPdfReflow
                | LossyKind::DocPdfToText
                | LossyKind::DocHtmlRender
                | LossyKind::DocToText
                | LossyKind::DocSimplified
                | LossyKind::SheetToDelimited
                | LossyKind::XlsLegacyLimits
                | LossyKind::TextEncodingNarrowed
                | LossyKind::SlidesToPdfFlatten
                | LossyKind::OfficeRoundtripApprox
                | LossyKind::PptxToPptLegacy
                | LossyKind::AudioLossyTarget
                | LossyKind::AudioTranscode
                | LossyKind::AudioLossyOrigin
                | LossyKind::AudioBitdepth
                | LossyKind::AudioTagsDropped
                | LossyKind::VideoReencode
                | LossyKind::VideoAlphaLost
                | LossyKind::VideoSubsDropped
                | LossyKind::VideoToGif
                | LossyKind::AudioDownmix => {}
            }
        }
        exhaustive(LossyKind::ImageLossyCodec);
    }

    // §6.4.1 unit (G15): the §0.6 target scalar/alias layer — TargetId (externally-tagged Format/Op),
    // CrossCatOp, Availability — in camelCase wire form, round-tripped, with no-wildcard membership locks.
    #[test]
    fn target_scalars_wire_forms_and_membership() {
        // TargetId — externally tagged; Format wraps a FormatId (= UserFacingFormat), Op a CrossCatOp.
        for (id, wire) in [
            (
                TargetId::Format(UserFacingFormat::Webp),
                r#"{"format":"webp"}"#,
            ),
            (
                TargetId::Op(CrossCatOp::ExtractAudio),
                r#"{"op":"extractAudio"}"#,
            ),
            (TargetId::Op(CrossCatOp::ToGif), r#"{"op":"toGif"}"#),
        ] {
            let json = serde_json::to_string(&id).expect("TargetId serializes");
            assert_eq!(json, wire, "§0.6: TargetId externally-tagged camelCase");
            let back: TargetId = serde_json::from_str(&json).expect("TargetId round-trips");
            assert_eq!(back, id, "§0.6: TargetId round-trips");
        }
        fn target_id_exhaustive(t: &TargetId) {
            match t {
                TargetId::Format(_) | TargetId::Op(_) => {}
            }
        }
        target_id_exhaustive(&TargetId::Op(CrossCatOp::ToGif));
        fn cross_cat_exhaustive(o: CrossCatOp) {
            match o {
                CrossCatOp::ExtractAudio | CrossCatOp::ToGif => {}
            }
        }
        cross_cat_exhaustive(CrossCatOp::ExtractAudio);

        // Availability — unit `Available` is a bare tag; `Unavailable { reason }` is externally tagged.
        assert_eq!(
            serde_json::to_string(&Availability::Available).expect("Available serializes"),
            r#""available""#,
            "§0.6/§3.4: Available is a bare camelCase tag"
        );
        let unavail = Availability::Unavailable {
            reason: "patent-gapped on this platform".to_owned(),
        };
        assert_eq!(
            serde_json::to_string(&unavail).expect("Unavailable serializes"),
            r#"{"unavailable":{"reason":"patent-gapped on this platform"}}"#,
            "§0.6/§3.4: Unavailable carries its reason"
        );
        let back: Availability =
            serde_json::from_str(r#"{"unavailable":{"reason":"x"}}"#).expect("round-trips");
        assert_eq!(
            back,
            Availability::Unavailable {
                reason: "x".to_owned()
            },
            "§3.4: Availability round-trips"
        );
        fn availability_exhaustive(a: &Availability) {
            match a {
                Availability::Available | Availability::Unavailable { .. } => {}
            }
        }
        availability_exhaustive(&Availability::Available);
    }

    // §6.4.1 unit (G15): the §1.6 option model — OptionKind (all 5 control shapes, externally-tagged
    // camelCase incl. the multi-word `intRange` + the nested `IntRange` fields + the `Enum` EnumChoice),
    // OptionValue (all 4 value shapes), Surface, Unit — each round-tripped, with no-wildcard membership
    // locks. This references the OptionKey/LabelKey/EnumChoice/Unit/Surface leaves.
    #[test]
    fn option_model_wire_forms_and_membership() {
        // OptionKind variants.
        let int_range = OptionKind::IntRange {
            min: 0,
            max: 100,
            step: 1,
            unit: Some(Unit::Percent),
        };
        assert_eq!(
            serde_json::to_string(&int_range).expect("IntRange serializes"),
            r#"{"intRange":{"min":0,"max":100,"step":1,"unit":"percent"}}"#,
            "§1.6: OptionKind::IntRange is externally-tagged camelCase with a nested unit"
        );
        let enum_kind = OptionKind::Enum {
            choices: vec![EnumChoice {
                value: "high".to_owned(),
                label: LabelKey("opt.mp3.high".to_owned()),
            }],
        };
        assert_eq!(
            serde_json::to_string(&enum_kind).expect("Enum serializes"),
            r#"{"enum":{"choices":[{"value":"high","label":"opt.mp3.high"}]}}"#,
            "§1.6: OptionKind::Enum carries EnumChoice {{ value, label }} (LabelKey transparent)"
        );
        assert_eq!(
            serde_json::to_string(&OptionKind::Toggle).expect("Toggle serializes"),
            r#""toggle""#,
            "§1.6: a fieldless OptionKind variant is a bare camelCase tag"
        );
        assert_eq!(
            serde_json::to_string(&OptionKind::Size { min: 16, max: 512 })
                .expect("Size serializes"),
            r#"{"size":{"min":16,"max":512}}"#,
            "§1.6: OptionKind::Size carries the pixel bounds"
        );
        for kind in [int_range, enum_kind, OptionKind::Toggle, OptionKind::Color] {
            let json = serde_json::to_string(&kind).expect("OptionKind serializes");
            let back: OptionKind = serde_json::from_str(&json).expect("OptionKind round-trips");
            assert_eq!(back, kind, "§1.6: OptionKind round-trips");
        }
        fn option_kind_exhaustive(k: &OptionKind) {
            match k {
                OptionKind::IntRange { .. }
                | OptionKind::Enum { .. }
                | OptionKind::Toggle
                | OptionKind::Size { .. }
                | OptionKind::Color => {}
            }
        }
        option_kind_exhaustive(&OptionKind::Color);

        // OptionValue variants.
        for (val, wire) in [
            (OptionValue::Int(80), r#"{"int":80}"#),
            (OptionValue::Bool(true), r#"{"bool":true}"#),
            (OptionValue::Enum("high".to_owned()), r#"{"enum":"high"}"#),
            (
                OptionValue::Color("#ffffff".to_owned()),
                r##"{"color":"#ffffff"}"##,
            ),
        ] {
            let json = serde_json::to_string(&val).expect("OptionValue serializes");
            assert_eq!(json, wire, "§1.6: OptionValue externally-tagged camelCase");
            let back: OptionValue = serde_json::from_str(&json).expect("OptionValue round-trips");
            assert_eq!(back, val, "§1.6: OptionValue round-trips");
        }
        fn option_value_exhaustive(v: &OptionValue) {
            match v {
                OptionValue::Int(_)
                | OptionValue::Bool(_)
                | OptionValue::Enum(_)
                | OptionValue::Color(_) => {}
            }
        }
        option_value_exhaustive(&OptionValue::Bool(false));

        // Surface + Unit wire forms + membership.
        for (s, wire) in [
            (Surface::Basic, "\"basic\""),
            (Surface::Advanced, "\"advanced\""),
        ] {
            assert_eq!(
                serde_json::to_string(&s).expect("Surface serializes"),
                wire,
                "§1.6: Surface camelCase"
            );
        }
        fn surface_exhaustive(s: Surface) {
            match s {
                Surface::Basic | Surface::Advanced => {}
            }
        }
        surface_exhaustive(Surface::Basic);
        for (u, wire) in [
            (Unit::Percent, "\"percent\""),
            (Unit::Kbps, "\"kbps\""),
            (Unit::Px, "\"px\""),
            (Unit::Dpi, "\"dpi\""),
            (Unit::Fps, "\"fps\""),
        ] {
            assert_eq!(
                serde_json::to_string(&u).expect("Unit serializes"),
                wire,
                "§1.6: Unit camelCase"
            );
        }
        fn unit_exhaustive(u: Unit) {
            match u {
                Unit::Percent | Unit::Kbps | Unit::Px | Unit::Dpi | Unit::Fps => {}
            }
        }
        unit_exhaustive(Unit::Px);
    }

    // §6.4.1 unit (G15): the §0.6 composite layer — a full `TargetOffer` (embedding a `Target` with its
    // `lossy`/`availability`/`options: Vec<OptionDecl>`, the offer-time SINGLE `Option<LossyKind>` marker)
    // and `OptionValues` (the BTreeMap keyed by `OptionKey` slugs). Locks the exact externally-tagged
    // camelCase wire shape (incl. `defaultTarget`) + round-trips. `Uuid::nil()` keeps `set` deterministic.
    #[test]
    fn target_offer_option_values_composite_wire_forms() {
        let decl = OptionDecl {
            key: OptionKey("quality".to_owned()),
            label: LabelKey("opt.quality".to_owned()),
            surface: Surface::Basic,
            kind: OptionKind::IntRange {
                min: 0,
                max: 100,
                step: 1,
                unit: Some(Unit::Percent),
            },
            default: OptionValue::Int(80),
        };
        let target = Target {
            id: TargetId::Format(UserFacingFormat::Webp),
            label: "WebP".to_owned(),
            lossy: Some(LossyKind::ImageLossyCodec),
            availability: Availability::Available,
            options: vec![decl],
        };
        let offer = TargetOffer {
            set: CollectedSetId(Uuid::nil()),
            targets: vec![target],
            default_target: TargetId::Format(UserFacingFormat::Webp),
        };
        assert_eq!(
            serde_json::to_string(&offer).expect("TargetOffer serializes"),
            r#"{"set":"00000000-0000-0000-0000-000000000000","targets":[{"id":{"format":"webp"},"label":"WebP","lossy":"image_lossy_codec","availability":"available","options":[{"key":"quality","label":"opt.quality","surface":"basic","kind":{"intRange":{"min":0,"max":100,"step":1,"unit":"percent"}},"default":{"int":80}}]}],"defaultTarget":{"format":"webp"}}"#,
            "§0.6/§1.5: TargetOffer is the full externally-tagged camelCase target graph with defaultTarget"
        );
        let back: TargetOffer = serde_json::from_str(&serde_json::to_string(&offer).expect("ser"))
            .expect("round-trips");
        assert_eq!(
            back, offer,
            "§0.6: TargetOffer round-trips through its wire form"
        );

        // OptionValues — a transparent newtype over BTreeMap; BTreeMap orders keys (`lossless` < `quality`).
        let mut map: BTreeMap<OptionKey, OptionValue> = BTreeMap::new();
        map.insert(OptionKey("quality".to_owned()), OptionValue::Int(80));
        map.insert(OptionKey("lossless".to_owned()), OptionValue::Bool(true));
        let values = OptionValues(map);
        assert_eq!(
            serde_json::to_string(&values).expect("OptionValues serializes"),
            r#"{"lossless":{"bool":true},"quality":{"int":80}}"#,
            "§0.6/§1.6: OptionValues is a JSON object keyed by the OptionKey slugs, BTreeMap-ordered"
        );
        let back: OptionValues =
            serde_json::from_str(r#"{"lossless":{"bool":true},"quality":{"int":80}}"#)
                .expect("OptionValues round-trips");
        assert_eq!(
            back, values,
            "§1.6: OptionValues round-trips through its wire form"
        );
    }

    // §6.4.1 unit (G15): the §0.6 destination / output-plan layer (P2.9). Locks the externally-tagged camelCase
    // WIRE forms of `DestinationChoice` (the C4/C5/C6 arg, §0.4.1) and `DivertReason` (the §2.7.2 divert
    // classification carried by the P2.11 DTOs) + round-trips both, and exercises the INTERNAL `OutputPlan`
    // (Debug/Clone/Eq, the directory-based no-`final_path` shape, §1.8/§2.14.1). `OutputPlan` is deliberately
    // NOT serialized — its `OsString` base_name/extension have no cross-platform JSON form (§0.6 / the section
    // note) — so the test asserts its construction + value identity, never a wire shape.
    #[test]
    fn destination_output_plan_layer_wire_and_shape() {
        // DestinationChoice — externally-tagged camelCase: BesideSource is a bare tag, ChosenRoot wraps the path.
        assert_eq!(
            serde_json::to_string(&DestinationChoice::BesideSource)
                .expect("BesideSource serializes"),
            r#""besideSource""#,
            "§2.7.1: BesideSource is the bare camelCase tag (the default destination)"
        );
        let chosen = DestinationChoice::ChosenRoot(PathBuf::from("/dest"));
        assert_eq!(
            serde_json::to_string(&chosen).expect("ChosenRoot serializes"),
            r#"{"chosenRoot":"/dest"}"#,
            "§2.7.1: ChosenRoot carries the chosen root path (externally-tagged camelCase)"
        );
        for dc in [DestinationChoice::BesideSource, chosen.clone()] {
            let json = serde_json::to_string(&dc).expect("DestinationChoice serializes");
            let back: DestinationChoice =
                serde_json::from_str(&json).expect("DestinationChoice round-trips");
            assert_eq!(
                back, dc,
                "§0.6: DestinationChoice round-trips through its wire form"
            );
        }
        fn destination_choice_exhaustive(d: &DestinationChoice) {
            match d {
                DestinationChoice::BesideSource | DestinationChoice::ChosenRoot(_) => {}
            }
        }
        destination_choice_exhaustive(&chosen);

        // DivertReason — all three §2.7.2 variants in their camelCase wire form, round-tripped.
        for (reason, wire) in [
            (DivertReason::Unwritable, r#""unwritable""#),
            (DivertReason::Ephemeral, r#""ephemeral""#),
            (DivertReason::NoAtomicPublish, r#""noAtomicPublish""#),
        ] {
            assert_eq!(
                serde_json::to_string(&reason).expect("DivertReason serializes"),
                wire,
                "§2.7.2: DivertReason is a bare camelCase tag"
            );
            let back: DivertReason = serde_json::from_str(wire).expect("DivertReason round-trips");
            assert_eq!(back, reason, "§0.6: DivertReason round-trips");
        }
        fn divert_reason_exhaustive(r: DivertReason) {
            match r {
                DivertReason::Unwritable
                | DivertReason::Ephemeral
                | DivertReason::NoAtomicPublish => {}
            }
        }
        divert_reason_exhaustive(DivertReason::Unwritable);

        // OutputPlan — the internal directory-based plan: Clone + Eq, OsString base-name/extension kept exactly,
        // publish_temp_dir == final_dir in v1 (the §2.14.1 same-volume sibling-dotfile rule). No wire assertion
        // (OsString has no cross-platform JSON form, §0.6 / the section note). `job` is the item's ItemId.
        let plan = OutputPlan {
            job: ItemId(0),
            final_dir: PathBuf::from("/dest/sub"),
            diverted: Some(DivertReason::Unwritable),
            base_name: OsString::from("report"),
            extension: OsString::from("pdf"),
            publish_temp_dir: PathBuf::from("/dest/sub"),
        };
        assert_eq!(plan.clone(), plan, "§0.6: OutputPlan is Clone + Eq");
        assert_eq!(
            plan.publish_temp_dir, plan.final_dir,
            "§2.14.1: in v1 the publish temp is a sibling inside final_dir (same volume)"
        );
        assert_eq!(
            plan.base_name,
            OsString::from("report"),
            "§2.2: the source base name is kept exactly"
        );
        assert_eq!(plan.diverted, Some(DivertReason::Unwritable));
        let beside = OutputPlan {
            diverted: None,
            ..plan.clone()
        };
        assert_eq!(beside.diverted, None, "§0.6: None diverted = beside-source");
    }

    // §6.4.1 unit (G15): the §0.6/§0.4.2 `JobStage` wire enum (P2.10) — the four coarse progress stages
    // carried by `ItemProgress.stage`, each in its camelCase wire form. JobStage is OUTBOUND-ONLY (no
    // `Deserialize`), so this is a SERIALIZE pin (like `ConversionErrorKind`'s), not a round-trip. The
    // `exhaustive` match is the COMPILE-TIME variant lock: a stage added/removed without updating it fails
    // to compile, so the wire-name pins can never silently fall behind the enum.
    #[test]
    fn job_stage_wire_form_is_camelcase() {
        for (stage, wire) in [
            (JobStage::Spawning, r#""spawning""#),
            (JobStage::Decoding, r#""decoding""#),
            (JobStage::Encoding, r#""encoding""#),
            (JobStage::Writing, r#""writing""#),
        ] {
            assert_eq!(
                serde_json::to_string(&stage).expect("JobStage serializes"),
                wire,
                "§0.4.2/§1.11: JobStage mirrors to its camelCase wire name (carried by ItemProgress)"
            );
        }
        fn job_stage_exhaustive(s: JobStage) {
            match s {
                JobStage::Spawning
                | JobStage::Decoding
                | JobStage::Encoding
                | JobStage::Writing => {}
            }
        }
        job_stage_exhaustive(JobStage::Writing);
    }

    // §6.4.1 unit (G15): the §0.6/§2.5 `RerunPrompt` + `RerunDecision` wire forms (P2.11). `RerunPrompt` is
    // outbound-only (carried in the C4/C5 previews) → a serialize pin (camelCase `equivalentCount`).
    // `RerunDecision` is the C6 INBOUND choice → round-trips (`skip`/`freshCopy`) + a compile-time variant
    // lock so the closed set can't silently drift from §2.5.
    #[test]
    fn rerun_prompt_and_decision_wire_forms() {
        assert_eq!(
            serde_json::to_string(&RerunPrompt {
                equivalent_count: 3
            })
            .expect("RerunPrompt serializes"),
            r#"{"equivalentCount":3}"#,
            "§2.5: RerunPrompt carries the equivalent-item count in camelCase"
        );
        for (decision, wire) in [
            (RerunDecision::Skip, r#""skip""#),
            (RerunDecision::FreshCopy, r#""freshCopy""#),
        ] {
            assert_eq!(
                serde_json::to_string(&decision).expect("RerunDecision serializes"),
                wire,
                "§2.5: RerunDecision is a bare camelCase tag (skip = safe default)"
            );
            let back: RerunDecision =
                serde_json::from_str(wire).expect("RerunDecision round-trips");
            assert_eq!(
                back, decision,
                "§0.6: RerunDecision round-trips (the C6 inbound arg)"
            );
        }
        fn rerun_decision_exhaustive(d: RerunDecision) {
            match d {
                RerunDecision::Skip | RerunDecision::FreshCopy => {}
            }
        }
        rerun_decision_exhaustive(RerunDecision::Skip);
    }

    // ─── P2.14 · §0.6-invariant property tests (§6.4.2 / G16) ────────────────────────────────────────────
    // The §6.4.2 property level (test-strategy §1.3) for the §0.6 normative invariants carried by the
    // `crate::domain` types. Each asserts an invariant over a WIDE generated input space, complementing the
    // example-based unit tests above. All three G16 / test-strategy §1.3 determinism knobs are set:
    //   * case-count floor 512 (> proptest's thin default of 256), via `ProptestConfig::with_cases`;
    //   * a PINNED CI seed — `pinned_runner()` drives a `TestRunner` with a `deterministic_rng`, so the 512
    //     cases are identical on every run, locally and in CI (the `proptest!` macro seeds from ENTROPY, so it
    //     CANNOT pin the forward seed — only an already-found counterexample; hence the explicit runner);
    //   * a failure is NEVER retried-to-pass — the pinned seed reproduces any counterexample deterministically
    //     (test-strategy §1.3 / §7). `Strategy`-combinator (macro-free) automatic shrinking, no hand-rolled
    //     `Shrink` impls (the §P0.5 / G16 rule).
    // Instances are built by canonical constructors that model the §1.1 freeze / §1.8 plan; the LIVE-path
    // enforcement (the real P3 freeze/plan over a real filesystem) is the P3 G31 integration leg
    // (test-strategy §1.1 / §6 — the data-structure leg is here, the live-path leg is there).
    //
    // [Build-Session-Entscheidung: P2.14] case-count floor 512 + a `deterministic_rng`-pinned seed; co-located
    // with the per-type unit tests (the established module layout); test ids built via the in-module
    // `ItemId(n)` tuple constructor (the field is private to `crate::domain` but visible to this child test
    // module — the sibling `jobid_compiles_as_itemid_alias` test uses it identically), a TEST fixture, never a
    // back-door past the §1.1/§7.1 minting policy.

    /// The §0.6-invariant property-test case-count floor (test-strategy §1.3: above proptest's 256 default).
    const P2_14_CASES: u32 = 512;

    fn prop_collected_set_id() -> CollectedSetId {
        serde_json::from_str(r#""00000000-0000-4000-8000-000000000000""#)
            .expect("CollectedSetId deserializes from a uuid string")
    }
    fn prop_instance_id() -> InstanceId {
        serde_json::from_str(r#""22222222-2222-4222-8222-222222222222""#)
            .expect("InstanceId deserializes from a uuid string")
    }
    /// A minimal eligible CSV `DroppedItem` carrying the §0.6 single-id-space id `id`.
    fn prop_dropped_item(id: u32) -> DroppedItem {
        DroppedItem {
            item: ItemId(id),
            raw_path: PathBuf::from("data.csv"),
            resolved_path: PathBuf::from("data.csv"),
            size_bytes: 0,
            detected: DetectionOutcome::Recognized {
                format: UserFacingFormat::Csv,
                confidence: Confidence::High,
                dims: None,
            },
        }
    }
    /// A minimal ineligible `SkippedItem` carrying the §0.6 single-id-space id `id`.
    fn prop_skipped_item(id: u32) -> SkippedItem {
        SkippedItem {
            item: ItemId(id),
            source: PathBuf::from("mystery.bin"),
            reason: SkipReason::Unreadable,
        }
    }
    /// The §1.1-freeze stand-in: build a `CollectedSet::Single` from an eligible-item snapshot, setting the
    /// confirm tally `count := items.len()` exactly as the freeze does (the LIVE freeze is P3).
    fn prop_freeze_single(items: Vec<DroppedItem>) -> CollectedSet {
        let count = items.len();
        CollectedSet::Single {
            id: prop_collected_set_id(),
            instance: prop_instance_id(),
            format: UserFacingFormat::Csv,
            items,
            count,
            skipped: vec![],
            total_bytes: 0,
            roots: vec![],
            encoding_hint: None,
            delimiter_hint: None,
            notes: vec![],
        }
    }

    /// Extract `(count, items)` from the `CollectedSet::Single` that `prop_freeze_single` always builds — an
    /// EXHAUSTIVE match (the crate denies `clippy::wildcard_enum_match_arm`, so no `_` arm); the non-`Single`
    /// arm is never taken by these tests and returns `None`, which the caller treats as a hard failure.
    fn single_count_items(set: &CollectedSet) -> Option<(usize, &[DroppedItem])> {
        match set {
            CollectedSet::Single { count, items, .. } => Some((*count, items.as_slice())),
            CollectedSet::Mixed { .. }
            | CollectedSet::Unsupported { .. }
            | CollectedSet::Uncertain { .. }
            | CollectedSet::Empty { .. } => None,
        }
    }

    /// A PINNED-SEED proptest runner (test-strategy §1.3 / G16: "a pinned CI seed"). The `proptest!` macro
    /// seeds its forward run from ENTROPY (only an already-found counterexample is pinned, via the
    /// `proptest-regressions/` file), so to make the 512-case exploration itself identical on every run —
    /// locally and in CI, so a property failure is reproducible and NEVER retried-to-pass (test-strategy §1.3
    /// / §7) — the §0.6-invariant properties drive a `TestRunner` with a `deterministic_rng` directly.
    /// [Build-Session-Entscheidung: P2.14]
    fn pinned_runner() -> TestRunner {
        TestRunner::new_with_rng(
            ProptestConfig::with_cases(P2_14_CASES),
            TestRng::deterministic_rng(RngAlgorithm::ChaCha),
        )
    }

    /// The §1.8 / §2.14.1-v1 output-plan stand-in: place the kind-1 publish temp in `final_dir` (a sibling
    /// dotfile on the SAME volume) so the §2.1 publish is an intra-volume atomic rename — the construction
    /// discipline P3/P4's real §1.8 plan builder must hold (the LIVE plan is P3). [Build-Session-Entscheidung: P2.14]
    fn prop_plan_for(final_dir: PathBuf, base: &str) -> OutputPlan {
        OutputPlan {
            job: ItemId(0),
            publish_temp_dir: final_dir.clone(), // §2.14.1 v1: the publish temp shares final_dir's volume
            final_dir,
            diverted: None,
            base_name: OsString::from(base),
            extension: OsString::from("tsv"),
        }
    }

    /// §0.6 "stable `ItemId`": the id is a TRANSPARENT `u32` on the wire and round-trips byte-stably for EVERY
    /// `u32` (not just the example ids the unit tests pin) — a future `#[serde(...)]` change that broke the
    /// transparent form, or a non-`u32` re-spelling, is caught across the whole value range.
    #[test]
    fn prop_item_id_is_a_stable_transparent_u32_on_the_wire() {
        pinned_runner()
            .run(&any::<u32>(), |n| {
                let id = ItemId(n);
                let wire = serde_json::to_string(&id).expect("ItemId serializes");
                let back: ItemId = serde_json::from_str(&wire).expect("ItemId deserializes");
                prop_assert_eq!(back, id, "§0.6: ItemId is stable across a wire round-trip");
                prop_assert_eq!(
                    wire,
                    n.to_string(),
                    "§0.6: ItemId is a transparent bare-u32 on the wire"
                );
                Ok(())
            })
            .unwrap();
    }

    /// §0.6 invariant 6 (the single id space): the §1.1 freeze assigns one `ItemId` per dropped item from ONE
    /// monotonic space (the global drop position), then filters into the eligible `items` view and the
    /// id-disjoint `skipped` view WITHOUT re-indexing either from 0. Over any eligibility pattern the two
    /// id-sets are disjoint and together cover exactly `0..N` — a re-indexed `items` (ids `0..k`) would
    /// collide with `skipped` and fail here.
    #[test]
    fn prop_single_id_space_is_disjoint_and_never_reindexed() {
        pinned_runner()
            .run(&prop::collection::vec(any::<bool>(), 0..64usize), |flags| {
                let mut items: Vec<DroppedItem> = Vec::new();
                let mut skipped: Vec<SkippedItem> = Vec::new();
                for (idx, &eligible) in flags.iter().enumerate() {
                    let id = u32::try_from(idx).expect("idx < 64 fits u32");
                    if eligible {
                        items.push(prop_dropped_item(id));
                    } else {
                        skipped.push(prop_skipped_item(id));
                    }
                }
                let item_ids: BTreeSet<ItemId> = items.iter().map(|d| d.item).collect();
                let skip_ids: BTreeSet<ItemId> = skipped.iter().map(|s| s.item).collect();
                prop_assert!(
                    item_ids.is_disjoint(&skip_ids),
                    "§0.6 inv-6: the eligible and skipped ids never collide (one shared id space)"
                );
                let covered: BTreeSet<ItemId> = item_ids.union(&skip_ids).copied().collect();
                let whole_space: BTreeSet<ItemId> = (0..flags.len())
                    .map(|i| ItemId(u32::try_from(i).expect("i < 64 fits u32")))
                    .collect();
                prop_assert_eq!(
                    covered, whole_space,
                    "§0.6 inv-6: the two views cover the single 0..N space, never re-indexed from 0"
                );
                Ok(())
            })
            .unwrap();
    }

    // ─── §0.6 invariant-6 ItemId assignment: from_index + the single ItemIdSpace (P2.75) ───
    // These lock the PRODUCTION id-source the P2.76 de-dup fold / P3.49 spine mint over. The disjoint/covers-0..N
    // VIEW invariant is proven by `prop_single_id_space_is_disjoint_and_never_reindexed` (above) over ids-by-index;
    // these prove `ItemIdSpace::mint` PRODUCES exactly that 0,1,2,… space — so the composition (freeze wires the
    // minter to the views) is proven by the two together, and the wired fold itself is P2.76's test (not re-tested
    // here — additive, so the existing property test is left untouched, no test-change). [Build-Session-Entscheidung: P2.75]

    /// §6.4.1 unit (G15) / §0.6 invariant 6: `ItemId::from_index(n)` IS the item at index `n` — identical to the
    /// in-crate `ItemId(n)` and to the transparent bare-`u32` wire form, across the boundary values (0, 1, MAX).
    /// Locks that the freeze constructor introduces no offset / re-mapping.
    #[test]
    fn item_id_from_index_is_the_transparent_index() {
        for n in [0u32, 1, 2, 41, u32::MAX] {
            assert_eq!(
                ItemId::from_index(n),
                ItemId(n),
                "§0.6: from_index(n) is the id at index n"
            );
            assert_eq!(
                serde_json::to_string(&ItemId::from_index(n)).expect("ItemId serializes"),
                n.to_string(),
                "§0.6: from_index yields the transparent bare-u32 wire form"
            );
        }
    }

    /// §6.4.1 unit (G15): `ItemId::from_index` is `const` — usable in a `const` context. Locks const-ness so a
    /// subsequent refactor to a non-const body (e.g. adding a runtime check) is a compile break, not a silent loss.
    #[test]
    fn item_id_from_index_is_const() {
        const ID: ItemId = ItemId::from_index(7);
        assert_eq!(
            ID,
            ItemId(7),
            "§0.6: a const from_index equals the id at index 7"
        );
    }

    /// §6.4.1 unit (G15) / §0.6 invariant 6: a fresh `ItemIdSpace` (via `new()` AND `default()`) first-mints
    /// `ItemId::from_index(0)` — the single id space always starts at 0, never re-indexed; `new() == default()`.
    #[test]
    fn item_id_space_new_and_default_start_at_zero() {
        assert_eq!(
            ItemIdSpace::new(),
            ItemIdSpace::default(),
            "§0.6: new() and default() are the same fresh space"
        );
        let mut space = ItemIdSpace::new();
        assert_eq!(
            space.mint(),
            Ok(ItemId::from_index(0)),
            "§0.6 inv-6: the first mint is index 0"
        );
        let mut default_space = ItemIdSpace::default();
        assert_eq!(
            default_space.mint(),
            Ok(ItemId::from_index(0)),
            "§0.6 inv-6: default() also first-mints 0"
        );
    }

    /// §6.4.1 unit (G15) / §0.6 invariant 6: consecutive mints yield `0, 1, 2, …` — strictly increasing,
    /// contiguous, from 0 (order-preserving + never re-indexed). This is the property the P2.76 fold relies on to
    /// give each first-seen survivor its stable freeze index.
    #[test]
    fn item_id_space_mints_monotonic_contiguous_from_zero() {
        let mut space = ItemIdSpace::new();
        let minted: Vec<ItemId> = (0..5)
            .map(|_| space.mint().expect("a fresh space is not exhausted"))
            .collect();
        let expected: Vec<ItemId> = (0u32..5).map(ItemId::from_index).collect();
        assert_eq!(minted, expected, "§0.6 inv-6: mints are 0,1,2,3,4 in order");
    }

    /// §6.4.1 unit (G15) / §0.6 invariant 6 (assign-once): N mints from one space produce N DISTINCT ids — no id
    /// is ever handed out twice, so the eligible/skipped views drawn from this space can never collide.
    #[test]
    fn item_id_space_mints_no_duplicate_ids() {
        let mut space = ItemIdSpace::new();
        let n = 1000usize;
        let ids: BTreeSet<ItemId> = (0..n)
            .map(|_| space.mint().expect("a fresh space is not exhausted"))
            .collect();
        assert_eq!(
            ids.len(),
            n,
            "§0.6 inv-6: each of N mints is unique (assign-once over one space)"
        );
    }

    /// §6.4.1 unit (G15) / §0.6 invariant 6 (no-panic honesty): `u32::MAX` IS a valid FINAL id — the mint at the
    /// ceiling hands out `from_index(u32::MAX)`, and only the FOLLOWING mint fails with `ItemSpaceExhausted`,
    /// NEVER a silent `as u32` wrap (which would alias item 2^32 onto id 0 and break per-item addressing). The
    /// ceiling is reached by constructing a space at the boundary directly (an in-crate `#[cfg(test)]` fixture),
    /// not by 4e9 iterations. This is the mint-then-`checked_add` ordering leg — an increment-then-return
    /// ordering would silently make `from_index(u32::MAX)` unreachable (an off-by-one capacity loss).
    #[test]
    fn item_id_space_reports_exhaustion_at_the_u32_ceiling() {
        let mut space = ItemIdSpace {
            next: Some(u32::MAX),
        };
        assert_eq!(
            space.mint(),
            Ok(ItemId::from_index(u32::MAX)),
            "§0.6: u32::MAX is a valid final id (handed out, not skipped)"
        );
        assert_eq!(
            space.mint(),
            Err(ItemSpaceExhausted),
            "§0.6: the FOLLOWING mint is exhausted, never a silent wrap"
        );
        assert_eq!(
            space.mint(),
            Err(ItemSpaceExhausted),
            "§0.6: exhaustion is stable (stays Err)"
        );
    }

    /// §0.6 "`count == items.len()`": the §1.1 freeze sets the confirm tally `count` to `items.len()`, so a
    /// wire consumer reading the tally never walks a 10k-file Vec. Holds for any frozen length; the tally also
    /// equals the INDEPENDENTLY-generated count `n`, so a freeze that sourced `count` from a stale value would
    /// fail.
    #[test]
    fn prop_collected_single_count_equals_items_len() {
        pinned_runner()
            .run(&(0usize..256), |n| {
                let items: Vec<DroppedItem> = (0..n)
                    .map(|i| prop_dropped_item(u32::try_from(i).expect("i < 256 fits u32")))
                    .collect();
                let set = prop_freeze_single(items);
                let (count, items) =
                    single_count_items(&set).expect("the freeze yields CollectedSet::Single");
                prop_assert_eq!(count, items.len(), "§0.6: count == items.len()");
                prop_assert_eq!(
                    count,
                    n,
                    "§0.6: the freeze tally equals the frozen item count"
                );
                Ok(())
            })
            .unwrap();
    }

    /// §2.4 "frozen `items`" (data-structure leg): the freeze is a PURE, deterministic function of its input
    /// snapshot — two freezes of the same snapshot are EQUAL — and that equality is DISCRIMINATING, so the
    /// determinism assertion is a real constraint: a set frozen from a snapshot that GREW after the freeze
    /// (a file appearing late, §2.4) compares UNEQUAL, and a set frozen from a snapshot whose reachable item
    /// was MUTATED in place compares UNEQUAL too — the equality inspects the owned item payloads, not just
    /// the length. Together: the frozen value provably cannot co-vary with post-snapshot changes to its
    /// source. The live-path leg (the freeze ignoring real on-disk changes) is the P3 G31 integration test
    /// (test-strategy §1.1 / §6).
    /// [Test-Change: P2.137 — old-obsolete+new-correct, §0.6] the prior ownership leg asserted that pushing
    /// into the SEPARATE source vec leaves the frozen set's length unchanged — a Rust move-semantics
    /// tautology no generated input could falsify (the two vecs are distinct values by construction). The
    /// new legs are falsifiable: each `prop_assert_ne` fails the moment `CollectedSet`'s equality (or the
    /// freeze) stops discriminating post-snapshot growth/mutation, verified against the §2.4/§0.6
    /// frozen-set contract.
    #[test]
    fn prop_frozen_items_are_an_owned_snapshot() {
        pinned_runner()
            .run(&(0usize..48), |n| {
                let source: Vec<DroppedItem> = (0..n)
                    .map(|i| prop_dropped_item(u32::try_from(i).expect("i < 48 fits u32")))
                    .collect();
                // [Test-Change: P2.137 — old-obsolete+new-correct, §0.6] (legs reordered/teethed; see doc)
                // the still-valid P2.14 shape leg, retained verbatim: the freeze yields Single with
                // exactly the n frozen items and a tracking count (§0.6/§2.4).
                let frozen = prop_freeze_single(source.clone());
                let (count, items) =
                    single_count_items(&frozen).expect("the freeze yields CollectedSet::Single");
                prop_assert_eq!(
                    items.len(),
                    n,
                    "§2.4: the frozen snapshot holds exactly the n frozen items"
                );
                prop_assert_eq!(count, n, "§0.6: count tracks the frozen snapshot");
                // [Test-Change: P2.137 — old-obsolete+new-correct, §0.6] the removed leg below this point
                // was the move-semantics tautology (push into the SEPARATE source vec, then compare
                // lengths); the replacement legs are falsifiable.
                // the freeze is deterministic: two freezes of the same snapshot are equal (no injected
                // timestamp / per-freeze state that would break §2.5 re-run equivalence downstream).
                prop_assert_eq!(
                    prop_freeze_single(source.clone()),
                    frozen,
                    "§2.4: the freeze is a deterministic pure function of its input snapshot"
                );
                // teeth 1: post-snapshot GROWTH is detectable — a file appearing after the snapshot freezes
                // to a DIFFERENT set (count + items diverge), so the equality above is no vacuous `x == x`.
                // [Test-Change: P2.137 — old-obsolete+new-correct, §0.6]
                let mut grown = source.clone();
                grown.push(prop_dropped_item(9999));
                prop_assert_ne!(
                    prop_freeze_single(source.clone()),
                    prop_freeze_single(grown),
                    "§2.4/§0.6: a snapshot grown post-freeze freezes to a DETECTABLY different set"
                );
                // teeth 2: an IN-PLACE mutation of a reachable item is detectable — the equality inspects
                // the owned item payloads (an empty snapshot has no item to mutate; teeth 1 covers n == 0).
                if !source.is_empty() {
                    let mut mutated = source.clone();
                    let first = mutated
                        .first_mut()
                        .expect("a non-empty snapshot has a first item");
                    first.size_bytes = first.size_bytes.wrapping_add(1);
                    prop_assert_ne!(
                        prop_freeze_single(source.clone()),
                        prop_freeze_single(mutated),
                        "§2.4/§0.6: a mutated reachable item yields a DETECTABLY different frozen set"
                    );
                }
                Ok(())
            })
            .expect("the pinned 512-case exploration holds the §2.4 frozen-snapshot invariants");
    }

    /// §2.14.1 v1 "same-volume publish-temp": the §1.8 output plan (`prop_plan_for`, the §2.14.1-v1
    /// construction discipline) places the kind-1 publish temp in `final_dir`, on the SAME volume, so the §2.1
    /// publish is a true intra-volume atomic rename. `publish_temp_dir == final_dir` for any destination
    /// directory; the teeth test below shows a cross-dir temp IS detectable (the equality is a real
    /// constraint, not `x == x`).
    #[test]
    fn prop_output_plan_publish_temp_is_the_same_dir_as_final() {
        let dir_re =
            proptest::string::string_regex("[a-z][a-z0-9_/]{0,23}").expect("valid dir regex");
        let base_re = proptest::string::string_regex("[a-z]{1,8}").expect("valid base regex");
        pinned_runner()
            .run(&(dir_re, base_re), |(dir, base)| {
                let final_dir = PathBuf::from(format!("/{dir}"));
                let plan = prop_plan_for(final_dir.clone(), &base);
                prop_assert_eq!(
                    &plan.publish_temp_dir, &plan.final_dir,
                    "§2.14.1 v1: publish_temp_dir EQUALS final_dir (the publish is an intra-volume rename)"
                );
                prop_assert_eq!(
                    &plan.publish_temp_dir, &final_dir,
                    "the publish temp is the generated destination dir, not a fixed off-volume scratch"
                );
                Ok(())
            })
            .unwrap();
    }

    // §0.6 teeth (non-proptest): the property assertions above are NOT vacuous `x == x` — a deliberately
    // corrupted instance is DETECTABLE, so each equality is a real constraint on the §1.1 freeze / §1.8 plan.
    #[test]
    fn count_equals_items_len_invariant_has_teeth() {
        let corrupt = CollectedSet::Single {
            id: prop_collected_set_id(),
            instance: prop_instance_id(),
            format: UserFacingFormat::Csv,
            items: vec![prop_dropped_item(0)],
            count: 2,
            skipped: vec![],
            total_bytes: 0,
            roots: vec![],
            encoding_hint: None,
            delimiter_hint: None,
            notes: vec![],
        };
        let (count, items) =
            single_count_items(&corrupt).expect("constructed as CollectedSet::Single");
        assert_ne!(
            count,
            items.len(),
            "a corrupted count IS detectable — count == items.len() is a real constraint, not a tautology"
        );
    }

    #[test]
    fn same_volume_publish_temp_invariant_has_teeth() {
        let plan = OutputPlan {
            job: ItemId(0),
            final_dir: PathBuf::from("/dest"),
            diverted: None,
            base_name: OsString::from("data"),
            extension: OsString::from("tsv"),
            publish_temp_dir: PathBuf::from("/scratch"),
        };
        assert_ne!(
            plan.publish_temp_dir, plan.final_dir,
            "a cross-dir publish temp IS detectable — the same-volume invariant is a real §2.14.1 constraint"
        );
    }
}
