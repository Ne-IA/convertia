//! `crate::outcome` — the single home of the §2.8 conversion-outcome taxonomy + message catalog and
//! the §2.9 lossy-disclosure catalog, mirrored onto the IPC wire as the §0.4.3 `IpcError` / `ErrorKind`
//! (the §0.7 tier module renamed from `error`; there is no `crate::error`).
//!
//! P1 established the module so the §0.7 tree compiles and the §06 drift mechanism has a home. P2.18
//! authored the §2.8.1 `ConversionErrorKind` taxonomy + its §0.4.3 `ErrorKind` wire alias here, P2.19 the
//! `IpcError` shape, P2.20 the `OutcomeMsg` surfaced-string type + the one-way `SkipReason → ErrorKind`
//! §1.12 projection helper, P2.39.1 the `AppFault` `app://fault` payload, and P2.73 the §1.1 turn-time
//! `ReadFailure → ErrorKind` projection helper.
//!
//! ## P3.1.3 reconcile — the taxonomy root is type-complete; the string TABLES + render seam remain
//! [Build-Session-Entscheidung: P3.1.3] The §2.8 taxonomy ROOT already homes the full item-/app-level
//! kind set + `ErrorKind` / `IpcError` / `AppFault` / `OutcomeMsg` + both projection helpers (above), so
//! this box authors NO new type or impl; it records the root as scaffolded and maps the string TABLES +
//! the surfacing leg owned by the scheduled boxes:
//!  - the §2.8.2 `ConversionErrorKind → canonical-English` message catalog — **P3.68 (built below):**
//!    `conversion_message_template` (the single-home 21-row table + `None` for the 4 kinds homed elsewhere),
//!    `conversion_failure` (the `{detected}`/`{platform}`/`{path}`-substituting `OutcomeMsg::Failure`
//!    producer), and the 5 batch-summary strings (`BatchSummary` + `WITH_RESIDUE_TAIL`).
//!  - the §2.9.1 `LossyKind → canonical-English` lossy-note catalog — **P3.69**.
//!  - the Running→Failed render seam turning an internal `ConversionErrorKind` into the surfaced
//!    `OutcomeMsg::Failure { text }` through the P3.68 catalog — **P3.46**.
//!
//! [Build-Session-Entscheidung: P3.1.3] The `From<ConversionErrorKind>` projection seam is VACUOUS under
//! the §2.8.2 option-1 alias (`pub type ErrorKind = ConversionErrorKind`, P2.18): `ErrorKind::from(kind)`
//! for `kind: ConversionErrorKind` is the std reflexive `From<T> for T` — already present, the identity;
//! a first-party `impl From<ConversionErrorKind> for ConversionErrorKind` conflicts with the std blanket
//! (E0119) and cannot be written. So NO `From` impl is authored here, and the P3.46.2 internal-to-wire
//! projection IS the identity — its real work is rendering the kind through the P3.68 catalog, not a type
//! conversion. (The `error_kind_is_the_conversion_error_kind_alias` test below pins the alias this rests on.)
//!
//! [Build-Session-Entscheidung: P3.1.3] `CleanupResidue` is NOT authored here. The
//! `CleanupResidue { item: ItemId, residue_path }` STRUCT is a §1.12 result-family type homed in
//! `crate::orchestrator` (tier 1, co-homed with `RunResult.cleanup_incomplete`, P2.12); `crate::outcome`
//! (tier 2) cannot reference a tier-1 type (§0.7), so its "string home" role is only the §2.8.2 catalog
//! ROW for the existing `ConversionErrorKind::CleanupResidue` variant + the §2.6.4 "With residue" tail —
//! both P3.68 strings, surfaced by P3.25.

// [Build-Session-Entscheidung: P2.18/P2.20/P2.73] The §2.8 wire-taxonomy (`ConversionErrorKind`/`ErrorKind`),
// the §0.4.3 `IpcError`, the `OutcomeMsg` surfaced line, the §1.12 `SkipReason → ErrorKind` helper, and the
// §1.1 turn-time `ReadFailure → ErrorKind` helper (P2.73) are all
// authored as CONTRACTS and registered for typegen (`collect_types![]`), but registration is a type-PARAMETER
// reference, not a construction — and no production path CONSTRUCTS an item-level outcome / calls the helper
// yet (the pipeline that emits them is P3/P4+). [P2.109] The one exception is the app-level `WebviewFault`
// `AppFault`: the §7.2.1 step-6 boot-fault seam in `main.rs` (`webview_init_fault`) now CONSTRUCTS it — the
// first production construction in this module — so `AppFault` + the `WebviewFault` variant are live; the
// item-level §2.8 taxonomy / IpcError / OutcomeMsg / helpers stay dead, which keeps this module-level
// expectation fulfilled. So the rest are dead in the PRODUCTION build until then; the cfg(test) anti-drift +
// wire-form tests reference them, so the TEST build is dead-code-clean. `expect` (not `allow`) auto-flags the
// moment the LAST covered item gains a production constructor/caller — matching `crate::domain`.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the item-level §2.8 taxonomy + IpcError + OutcomeMsg + the §1.12 SkipReason→ErrorKind and §1.1 turn-time ReadFailure→ErrorKind helpers are authored as contracts and registered for typegen, but no production path CONSTRUCTS an item-level outcome / calls either helper until the P3/P4+ pipeline, so they stay dead in the production build until then. (The app-level `WebviewFault` `AppFault` IS constructed by the P2.109 boot-fault seam in main.rs — the first production construction here — but the enumerated item-level items remain dead, keeping this expectation fulfilled.)"
    )
)]

use std::path::PathBuf;

use serde::Serialize;
use specta::Type;

use crate::domain::{LossyKind, ReadFailure, SkipReason};

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

// ─── §0.4.3 IpcError — the single wire error shape every command Err returns (P2.19) ──
/// The §0.4.3 authoritative error shape — every command's `Err` and every `ItemOutcome::Failed.error` is
/// this ONE shape (§0.4.3 / §2.8). Homed in `crate::outcome` (the §2.8 taxonomy → §0.4.3 IpcError mirror
/// module, §0.7). OUTBOUND-ONLY (a `Result` `Err` / `ItemOutcome::Failed.error` return, never deserialized
/// from the WebView) — so `Serialize` + `Type`, NO `Deserialize` (mirroring the outbound-only
/// `ConversionErrorKind`/`ErrorKind`, P2.18). `message` is the §2.8.2 pre-localised plain English string
/// (NEVER a stack trace / raw engine stderr, SSOT *no stack traces*); the §2.8 message CATALOG that produces
/// it is `conversion_message_template` / `conversion_failure` below (P3.68).
///
/// [Build-Session-Entscheidung: P2.19] `kind` is typed with the CONCRETE `ConversionErrorKind`, NOT the
/// §0.4.3-named `ErrorKind` ALIAS (`pub type ErrorKind = ConversionErrorKind`, P2.18) — the SAME type, but
/// referencing the forward-declared alias from this (production-dead-until-consumed) struct trips the rustc
/// dead-code-EXPECTATION/alias interaction with this module's forward-declaration suppression; the concrete
/// spelling avoids it (the P2.10 `JobState::Failed` / P2.9 `OutputPlan.job` precedent). specta resolves the
/// alias to the concrete type regardless, so the mirrored wire/bindings type is `ConversionErrorKind`
/// either way.
///
/// [Build-Session-Entscheidung: P2.19] Registered in the P1.25 type registry (§0.4.3 / §2.8: "both
/// IpcError and ErrorKind derive specta::Type and are registered in collect_types![]") so
/// `ItemOutcome::Failed.error` + every command `Err` mirror to `bindings.ts` as the named `IpcError`
/// rather than `any`; registering `IpcError` pulls its referenced `ConversionErrorKind` into the export as
/// a named type too (the §2.8.2 deferred-to-its-consumer registration, P2.18). Derive set: `Serialize` +
/// `Type` (the §0.4.3 wire-required pair) + `Debug, Clone, PartialEq, Eq` (ergonomics + the serialize-pin
/// test); NOT `Copy` (owns a `String` + two `PathBuf`s); NO `Deserialize` (outbound-only). camelCase wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    /// The stable machine code from the §2.8 taxonomy — drives the UI branching + i18n.
    pub kind: ConversionErrorKind,
    /// The §2.8.2 pre-localised plain-language English message; NEVER a stack trace / raw engine stderr.
    pub message: String,
    /// The optional path the error concerns (for the §1.12 summary's output→source map).
    pub path: Option<PathBuf>,
    /// The optional residue location when §2.6 cleanup could not complete — so the item is never reported
    /// as a clean success.
    pub residue: Option<PathBuf>,
}

// ─── §0.4.2 AppFault — the app://fault event payload (§2.13 app-level fault) (P2.39.1) ──
/// The `app://fault` event payload (§0.4.2 / §2.13.1 / §2.13.3) — the **app-level** fault the §2.13.3
/// single calm screen renders: a startup engine-missing escalation, a WebView core disconnect (§5.8), a
/// damaged bundle. It is categorically distinct from a per-item `IpcError`: an app-level fault means the
/// WHOLE APP can't function (the §2.13.1 "App-level" class), not one item failing — so it is surfaced via
/// the §0.4.2 `app://fault` `app.emit` event (a Rust→WebView signal the §2.13.3 / §5.8 screen listens for),
/// NEVER as a §1.12 per-item summary row.
///
/// OUTBOUND-ONLY: the core `app.emit`s it Rust→WebView and the WebView `listen`s — it is never deserialized
/// core-side — so `Serialize` + `Type`, NO `Deserialize` (the identical outbound-only derive choice as the
/// sibling wire / event payloads `IpcError` (§0.4.3 above) and `ConversionEvent` / `ScanProgress` (§0.4.2)).
/// camelCase wire. Derive set mirrors `IpcError`'s: `Debug, Clone, PartialEq, Eq` (ergonomics + the
/// serialize-pin test) + `Serialize, Type`; NOT `Copy` (owns a `String`).
///
/// [Build-Session-Entscheidung: P2.39.1] `kind` is typed with the CONCRETE `ConversionErrorKind`, NOT the
/// §0.4.3 `ErrorKind` alias (`pub type ErrorKind = ConversionErrorKind`) — the SAME type, but referencing
/// the forward-declared alias from this production-dead-until-emitted struct trips the rustc
/// dead-code-EXPECTATION/alias interaction this module's `not(test)` forward-declaration dead-code
/// suppression relies on (the identical P2.19 `IpcError.kind` decision; specta resolves the alias to the
/// same wire type
/// regardless). Only the three §2.13 app-level variants {`EngineMissing`, `WebviewFault`, `BundleDamaged`}
/// ever travel on this event — a §2.13 RUNTIME invariant, NOT a type constraint (mirroring
/// `IntakePayload.origin`, typed as the full `IntakeOrigin` though only `LaunchArg`/`SecondInstance` ride
/// `app://intake`). `message` is the §2.13.3 pre-localised, plain-English, trace-free calm line (NEVER a
/// stack trace / raw engine stderr, SSOT *no stack traces*); the §2.13.3 / §7.2 strings that fill it are a
/// later box.
///
/// [Build-Session-Entscheidung: P2.39.1] Homed in `crate::outcome` (tier 2), NOT `crate::domain` (the
/// tier-3 leaf): it references `ConversionErrorKind`, which lives here, and a leaf type cannot depend on a
/// higher tier (§0.7). It is NOT an orchestrator lifecycle/result type (the §0.7 ‡ rule that homed
/// `ConversionEvent` at tier 1), so tier 2 — its lowest valid home, beside the `ConversionErrorKind` it
/// carries — is correct.
///
/// [Build-Session-Entscheidung: P2.39.1] The "register in collect_types![]" the §0.4.3 box-note calls for is
/// `main.rs`'s `register_ipc_event_types` `.types(register::<AppFault>())` (tauri-specta v2 has no
/// `collect_types!` macro). `app://fault` is a RAW `app.emit` / TS `listen` event (§0.4.2), NOT a
/// `collect_events!` typed event: tauri-specta rc.25's TS event codegen unconditionally emits a `makeEvent`
/// helper with an `any`-typed `payload` parameter, which would violate the no-`any` rule frozen on the
/// generated `bindings.ts` (G5/G8) — the same reason P2.22 chose `ErrorHandlingMode::Throw` over the
/// `any`-bearing `typedError` helper. The
/// `.types()` registration still exports `AppFault` as a NAMED `bindings.ts` type so `listen('app://fault')`
/// type-checks rather than mirroring `any`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppFault {
    /// The app-level fault kind — only {`EngineMissing`, `WebviewFault`, `BundleDamaged`} per §2.13 (a
    /// RUNTIME invariant; the field type is the full mirror enum, see the struct doc).
    pub kind: ConversionErrorKind,
    /// The §2.13.3 pre-localised, plain-English, trace-free calm message.
    pub message: String,
}

// ─── §2.8.2 OutcomeMsg — the surfaced per-item outcome line (P2.20) ──
/// The §2.8.2 surfaced per-item outcome — the *resolved, ready-to-show* line for one item, carried by the
/// §0.6 `ItemResult.reason: Option<OutcomeMsg>` (which rides the `RunFinished` Channel payload + the C8
/// return, §0.4.2/§1.12). It is **either** a §2.8 failure, a §2.9 lossy note, **or** a §1.1/§1.3 pre-flight
/// skip — three distinct variants so a consumer pattern-matching `OutcomeMsg` can tell skip from fail WITHOUT
/// also reading `ItemResult.state` (§0.6 keeps `Skipped`/`Failed` distinct, §1.12 `Totals` counts them
/// separately — "must not be conflated"). Each variant carries the stable discriminant (`kind`/`reason`) so
/// §5 may re-localise (§2.10) AND the resolved English `text` (the §2.8.2 catalog row / §2.9.1 note with its
/// `{x}` substitutions already applied), so the §5.3 Summary needs no second lookup.
///
/// [Build-Session-Entscheidung: P2.20] OUTBOUND-ONLY (it crosses the boundary inside the outbound
/// `RunResult`/`ItemResult`, never deserialized from the WebView) — `Serialize` + `Type` (the §2.8.2
/// wire-required pair so `ItemResult.reason` mirrors as the named `OutcomeMsg`, not `any`) + `Debug, Clone,
/// PartialEq, Eq` (ergonomics + the serialize-pin tests); NOT `Copy` (owns a `String` per variant); NO
/// `Deserialize` (outbound-only, mirroring `IpcError`/`ConversionErrorKind`). Adjacently tagged
/// (`tag = "type", content = "data"`) so each variant is a discriminated `{ type, data }` object on the wire.
/// Registered in the P1.25 type registry (§2.8.2 line 1261 mandate), which pulls its referenced `SkipReason`
/// (+ the already-registered `ConversionErrorKind`/`LossyKind`) into the export as named types. `Failure.kind`
/// is spelled with the CONCRETE `ConversionErrorKind`, NOT the `ErrorKind` alias — mirroring the P2.19
/// `IpcError.kind` decision (referencing the forward-declared alias from a production-dead item trips the
/// rustc dead-code-expectation/alias interaction; specta resolves the alias to the same wire type regardless).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase", tag = "type", content = "data")]
pub enum OutcomeMsg {
    /// A §2.8 conversion FAILURE (the item entered the queue and failed) — `kind` is the §2.8.1 taxonomy code,
    /// `text` the §2.8.2 catalog row with its substitutions applied.
    Failure {
        kind: ConversionErrorKind,
        text: String,
    },
    /// A §2.9 predictable-LOSS note on an otherwise-successful conversion — `kind` is the §2.9.1 catalog key,
    /// `text` the §2.9.1 note.
    Lossy { kind: LossyKind, text: String },
    /// A §1.1/§1.3 pre-flight SKIP (a detection-ineligible item that never entered the queue, projected into
    /// `RunResult.items` at run-end, §1.12) — `reason` is the §0.6 `SkipReason`. A skip rides THIS skip-shaped
    /// variant, NOT `Failure`, so skip ≠ fail at the type level (§1.12).
    Skipped { reason: SkipReason, text: String },
}

// ─── §1.12 forward projection helper — SkipReason → ErrorKind (one-way, non-inverted) ──
/// The §1.12 / §0.6 forward projection of a §0.6 `SkipReason` onto its §2.8.1 `ErrorKind` (== the concrete
/// `ConversionErrorKind`). This is the ONE-WAY, non-invertible conversion the spec sanctions (§0.6 line 733 /
/// §1.12): it is applied ONLY when a `Skipped` item must ALSO surface an `ErrorKind`-shaped display string —
/// never to turn a skip into a failure (the `OutcomeMsg::Skipped` variant keeps skip ≠ fail; §1.12 "must not
/// be conflated"). There is deliberately NO reverse `ErrorKind → SkipReason` map: `Uncertain → Unrecognized`
/// is non-invertible (there is no `ErrorKind::Uncertain`), so the projection only ever runs forward.
///
/// [Build-Session-Entscheidung: P2.20] A NAMED helper, NOT a blanket `From<SkipReason> for ErrorKind` impl —
/// an ambient `.into()` would make turning a skip into a failure-kind trivially available everywhere, blurring
/// the §1.12 skip ≠ fail boundary the type system exists to keep. The explicit function keeps the one
/// sanctioned forward projection greppable and intentional. The match is non-wildcard, so a new `SkipReason`
/// variant fails to compile here — the helper is its own compile-time total-ness guard against the §0.6
/// `SkipReason` set. Returns the concrete `ConversionErrorKind` (the `ErrorKind` alias's underlying type), the
/// spelling consistent with `OutcomeMsg::Failure.kind` / `IpcError.kind`.
pub fn skip_reason_to_error_kind(reason: SkipReason) -> ConversionErrorKind {
    match reason {
        SkipReason::UnsupportedType => ConversionErrorKind::UnsupportedType,
        // The non-invertible one (§1.12): a freeze-time "couldn't confidently classify" maps to the
        // conversion-time "couldn't tell what kind of file this is" — there is no `ErrorKind::Uncertain`.
        SkipReason::Uncertain => ConversionErrorKind::Unrecognized,
        SkipReason::Empty => ConversionErrorKind::Empty,
        SkipReason::Unreadable => ConversionErrorKind::Unreadable,
    }
}

// ─── §1.1 turn-time read-failure → ErrorKind (the intake-Skipped vs turn-Failed non-conflation, P2.73) ──
/// The §1.1 / §2.8 **turn-time** projection of a `ReadFailure` onto its §2.8.1 `ErrorKind` (== the concrete
/// `ConversionErrorKind`) — the FAILURE half of the §1.1 zero-byte/unreadable classification. A file that was
/// READABLE at the §2.4 freeze but is **unreadable/gone WHEN ITS TURN COMES** mid-run is a per-item
/// **`Failed`** counted in the §1.12 `failed` total (§1.9 mid-run skip): now-missing (`NotFound`) →
/// `Gone`; now-unreadable (permission / exclusive lock / other IO) → `Unreadable`.
///
/// **This is NOT the intake-time path** — §1.1 "these are different totals and must not be conflated". A read
/// failure observed AT INTAKE is a pre-flight **Skip**: it lands in `DetectionOutcome::Unreadable { reason }`,
/// projected by `DetectionOutcome::skip_reason` (P2.16) to `SkipReason::Unreadable` (a `JobState::Skipped`,
/// never queued, counted in the §1.12 `skipped` total). The SAME underlying `ReadFailure` therefore
/// classifies to a SKIP at intake and a FAILURE at turn-time. The range is exactly `{Gone, Unreadable}`,
/// NEVER `Empty`: a 0-byte file is an INTAKE-only zero-byte skip (`DetectionOutcome::Empty` →
/// `SkipReason::Empty`), never a turn-time read failure (the item was non-empty + readable at the freeze).
///
/// [Build-Session-Entscheidung: P2.73] A NAMED helper, NOT a `From<ReadFailure> for ConversionErrorKind`
/// impl — the symmetric counterpart of the P2.20 `skip_reason_to_error_kind` decision: an ambient `.into()`
/// would make turning a read condition into a failure-kind trivially available everywhere (incl. an
/// intake-side caller that must instead produce a `SkipReason`), blurring the §1.1 skip ≠ fail boundary. The
/// explicit fn keeps the one sanctioned turn-time projection greppable + intentional, and the non-wildcard
/// match makes a new `ReadFailure` variant force an explicit turn-time classification rather than silently
/// defaulting. Returns the concrete `ConversionErrorKind` (the `ErrorKind` alias's underlying type), the
/// spelling consistent with `skip_reason_to_error_kind` / `IpcError.kind`.
pub fn read_failure_to_error_kind(failure: ReadFailure) -> ConversionErrorKind {
    match failure {
        // Present at the freeze, now MISSING (moved / deleted / removed media) — §2.8 `Gone`.
        ReadFailure::NotFound => ConversionErrorKind::Gone,
        // Present at the freeze, now UNREADABLE (permission denied / exclusive lock / other IO) — §2.8
        // `Unreadable`.
        ReadFailure::PermissionDenied | ReadFailure::Locked | ReadFailure::IoError => {
            ConversionErrorKind::Unreadable
        }
    }
}

// ─── §2.8.2 the conversion-outcome message catalog — the single home of the canonical English strings ──
/// The §2.8.2 canonical-English message TEMPLATE for a conversion-outcome kind — the raw string with any
/// `{detected}` / `{platform}` / `{path}` slot still literal. This is the **single home** of the §2.8.2
/// strings (§2.8 owns the set): `crate::orchestrator` (P3.46) maps an `ErrorKind` into it, `crate::run` (P3.25)
/// reads the `CleanupResidue` row, §1.12 (P3.50) reads it for the summary projection, and the UI (P4.69/P8.19)
/// render the resolved text verbatim — no consumer ever re-authors a string. Tone: plain, calm, never blaming,
/// never technical (SSOT *Fail clearly*); English-only (G57).
///
/// Returns `None` for the four `ConversionErrorKind` variants §2.8.2 does NOT home: the three §2.13 app-level
/// faults (`EngineMissing` / `WebviewFault` / `BundleDamaged`) render via the §2.13.3 `app://fault` catalog,
/// and `MixedDrop` is the §1.3 pre-flight refusal surfaced by the §5.2 UI — each a different home, so this
/// per-item conversion-outcome table returns `None` rather than duplicating them (one string, one home). The
/// match is EXHAUSTIVE (G4/G14): a new `ConversionErrorKind` variant forces a compile-time decision here.
/// [Build-Session-Entscheidung: P3.68]
pub fn conversion_message_template(kind: ConversionErrorKind) -> Option<&'static str> {
    let text = match kind {
        ConversionErrorKind::Corrupt => "This file looks damaged and couldn't be converted.",
        ConversionErrorKind::Empty => "This file is empty — there's nothing to convert.",
        ConversionErrorKind::Unrecognized => {
            "ConvertIA couldn't tell what kind of file this is, so it can't convert it."
        }
        ConversionErrorKind::UnsupportedType => {
            "ConvertIA can't convert this type of file — it looks like {detected}."
        }
        ConversionErrorKind::UnsupportedPair => "That conversion isn't available.",
        ConversionErrorKind::Unreadable => {
            "ConvertIA couldn't open this file — it may be in use by another program, or you don't have permission to read it."
        }
        ConversionErrorKind::Gone => {
            "This file is no longer there — it may have been moved, renamed, or its drive removed."
        }
        ConversionErrorKind::PasswordProtected => {
            "This file is password-protected or copy-protected, so ConvertIA can't read it."
        }
        ConversionErrorKind::NoAudioTrack => "This file has no audio to extract.",
        ConversionErrorKind::TooBig => {
            "This file is too large for ConvertIA to convert on this computer."
        }
        ConversionErrorKind::OutOfDisk => {
            "There isn't enough free disk space to finish this conversion."
        }
        ConversionErrorKind::WriteFailed => {
            "ConvertIA couldn't save the converted file to that location."
        }
        ConversionErrorKind::PathTooLong => {
            "The output name would be too long for this system, so this file was skipped. Try a shorter folder or file name."
        }
        ConversionErrorKind::TooManyCollisions => {
            "There are already too many files with this name in that folder, so this one couldn't be saved. Try a different folder."
        }
        ConversionErrorKind::EngineCrash => {
            "Something went wrong while converting this file, so it was skipped."
        }
        ConversionErrorKind::EngineHang => "This file took too long to convert and was stopped.",
        ConversionErrorKind::EngineError => "ConvertIA couldn't convert this file.",
        ConversionErrorKind::PlatformUnavailable => {
            "This conversion isn't available on {platform} because the required format support can't be included here."
        }
        ConversionErrorKind::QuarantinedByOs => {
            "macOS is blocking one of ConvertIA's built-in tools with a security check. Open System Settings → Privacy & Security and choose \"Open Anyway\", then try again."
        }
        ConversionErrorKind::CleanupResidue => {
            "This file couldn't be converted, and a temporary file may remain at {path}."
        }
        ConversionErrorKind::InternalError => {
            "Something unexpected went wrong, so this file was skipped. The rest of your files will continue."
        }
        // Homed elsewhere — not a §2.8.2 per-item conversion-outcome string (one string, one home):
        // {EngineMissing, WebviewFault, BundleDamaged} render via the §2.13.3 app://fault catalog, MixedDrop
        // via the §5.2 pre-flight UI. This per-item table returns None rather than duplicating them.
        ConversionErrorKind::EngineMissing
        | ConversionErrorKind::WebviewFault
        | ConversionErrorKind::BundleDamaged
        | ConversionErrorKind::MixedDrop => return None,
    };
    Some(text)
}

/// Build the §2.8.2 [`OutcomeMsg::Failure`] for a conversion-outcome `kind`, filling the kind's single `{x}`
/// slot from `arg`: the friendly detected type for `UnsupportedType`, the platform name for
/// `PlatformUnavailable`, the residue path display for `CleanupResidue` — ignored (pass `""`) for the majority
/// with no slot. Returns `None` for a kind §2.8.2 does not home (see [`conversion_message_template`]).
/// Panic-free (a single `str::replace`, no formatting fallibility). Substitutes ONLY the one slot the template
/// carries — never a chain of three replaces — so `arg`'s own content can never be re-scanned into a second
/// substitution even if it happens to contain another slot token (a user residue path literally reading
/// `{platform}`, say); `str::replace` never re-matches its own output. [Build-Session-Entscheidung: P3.68]
pub fn conversion_failure(kind: ConversionErrorKind, arg: &str) -> Option<OutcomeMsg> {
    let template = conversion_message_template(kind)?;
    let text = if template.contains("{detected}") {
        template.replace("{detected}", arg)
    } else if template.contains("{platform}") {
        template.replace("{platform}", arg)
    } else if template.contains("{path}") {
        template.replace("{path}", arg)
    } else {
        // No slot in this template — `arg` is ignored (the majority of kinds).
        template.to_owned()
    };
    Some(OutcomeMsg::Failure { kind, text })
}

/// The §2.8.2 batch-level summary situations — the run-end line §1.12 assembles from the run `Totals`.
/// Fieldful so the `{n}` / `{ok}` / `{fail}` counts are typed, not stringly-substituted. The "With residue"
/// tail is [`WITH_RESIDUE_TAIL`] (appended, not a situation). Derive set matches the file convention (`Debug,
/// Clone, Copy, PartialEq, Eq` — all-`usize` fields, so `Copy` is free); NO wire derives — this is a
/// core-internal string source assembled into an already-wire `RunResult` line, not itself a wire type.
/// [Build-Session-Entscheidung: P3.68]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchSummary {
    /// "All {n} files converted."
    AllSucceeded { n: usize },
    /// "{ok} of {n} files converted. {fail} couldn't be converted — see details."
    Partial { ok: usize, n: usize, fail: usize },
    /// "None of the {n} files could be converted." — an explicit failure, never a quiet finish (SSOT).
    AllFailed { n: usize },
    /// "Stopped. {ok} files were already converted and kept; the rest were not started."
    Cancelled { ok: usize },
}

impl BatchSummary {
    /// The §2.8.2 canonical-English summary line for this situation, counts substituted (English-only, G57).
    /// [Build-Session-Entscheidung: P3.68]
    #[must_use]
    pub fn text(&self) -> String {
        match *self {
            BatchSummary::AllSucceeded { n } => format!("All {n} files converted."),
            BatchSummary::Partial { ok, n, fail } => {
                format!("{ok} of {n} files converted. {fail} couldn't be converted — see details.")
            }
            BatchSummary::AllFailed { n } => format!("None of the {n} files could be converted."),
            BatchSummary::Cancelled { ok } => {
                format!("Stopped. {ok} files were already converted and kept; the rest were not started.")
            }
        }
    }
}

/// The §2.8.2 "With residue" tail — appended (after a space) to a [`BatchSummary`] line when temporary files
/// may remain (§2.6.4). Its own `const` so the §1.12 assembler and the §2.6.4 residue path (P3.25) share the
/// one string. [Build-Session-Entscheidung: P3.68]
pub const WITH_RESIDUE_TAIL: &str = "Some temporary files may remain — see details.";

#[cfg(test)]
mod tests {
    use super::*;

    // §6.4.1 unit (G15): the §1.1 turn-time `ReadFailure → ErrorKind` projection (P2.73) — a file readable at
    // the §2.4 freeze but unreadable/gone WHEN ITS TURN COMES mid-run is a per-item `Failed` (§1.9):
    // now-missing (`NotFound`) → `Gone`; now-unreadable (permission / lock / other IO) → `Unreadable`. The
    // turn-time range is exactly `{Gone, Unreadable}`, NEVER `Empty` (emptiness is an intake-only zero-byte
    // skip, §1.1). The non-wildcard match makes a new `ReadFailure` variant force a turn-time classification.
    #[test]
    fn read_failure_to_error_kind_classifies_turn_time_failures() {
        assert_eq!(
            read_failure_to_error_kind(ReadFailure::NotFound),
            ConversionErrorKind::Gone,
            "§1.1/§2.8: a frozen file now MISSING at its turn is Failed(Gone)"
        );
        for failure in [
            ReadFailure::PermissionDenied,
            ReadFailure::Locked,
            ReadFailure::IoError,
        ] {
            assert_eq!(
                read_failure_to_error_kind(failure),
                ConversionErrorKind::Unreadable,
                "§1.1/§2.8: a frozen file now UNREADABLE at its turn is Failed(Unreadable)"
            );
        }
        for failure in [
            ReadFailure::NotFound,
            ReadFailure::PermissionDenied,
            ReadFailure::Locked,
            ReadFailure::IoError,
        ] {
            assert_ne!(
                read_failure_to_error_kind(failure),
                ConversionErrorKind::Empty,
                "§1.1: emptiness is an intake-only zero-byte skip — never a turn-time read-failure kind"
            );
        }
    }

    // §6.4.1 unit (G15): the §1.1 "must not be conflated" invariant — the SAME underlying read condition is a
    // pre-flight SKIP at intake but a per-item FAILURE at turn-time. At intake a read failure lands in
    // `DetectionOutcome::Unreadable` → `skip_reason` → `Some(SkipReason::Unreadable)` (a `JobState::Skipped`,
    // never queued, §1.12 `skipped` total); the SAME `ReadFailure` at turn-time → `read_failure_to_error_kind`
    // → a `ConversionErrorKind` (a `JobState::Failed`, §1.12 `failed` total). They are different result TYPES
    // (`SkipReason` vs `ConversionErrorKind`), so the two §1.12 totals are structurally non-conflatable.
    #[test]
    fn intake_read_failure_is_a_skip_distinct_from_the_turn_time_failure() {
        use crate::domain::DetectionOutcome;
        for failure in [
            ReadFailure::NotFound,
            ReadFailure::PermissionDenied,
            ReadFailure::Locked,
            ReadFailure::IoError,
        ] {
            assert_eq!(
                DetectionOutcome::Unreadable { reason: failure }.skip_reason(),
                Some(SkipReason::Unreadable),
                "§1.1: an intake-time read failure is Skipped(Unreadable) — pre-flight, never queued"
            );
            assert!(
                matches!(
                    read_failure_to_error_kind(failure),
                    ConversionErrorKind::Gone | ConversionErrorKind::Unreadable
                ),
                "§1.1: the SAME read failure at turn-time is Failed(Gone|Unreadable), not a skip — the two §1.12 totals must not be conflated"
            );
        }
    }

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

    // §6.4.1 unit (G15): the §0.4.3 `IpcError` wire shape (P2.19) — the single error shape every command
    // `Err` / `ItemOutcome::Failed.error` returns, in its camelCase wire form (kind/message/path/residue).
    // OUTBOUND-ONLY (no `Deserialize`), so a SERIALIZE pin, not a round-trip. `path` Some / `residue` None
    // exercises both `Option<PathBuf>` renderings; `kind` carries a §2.8 taxonomy code.
    #[test]
    fn ipc_error_wire_form_is_camelcase() {
        let err = IpcError {
            kind: ConversionErrorKind::WriteFailed,
            message: "Could not write the output file.".to_owned(),
            path: Some(PathBuf::from("/out/report.pdf")),
            residue: None,
        };
        assert_eq!(
            serde_json::to_string(&err).expect("IpcError serializes"),
            r#"{"kind":"writeFailed","message":"Could not write the output file.","path":"/out/report.pdf","residue":null}"#,
            "§0.4.3: IpcError is the single camelCase wire error shape (kind/message/path/residue)"
        );
    }

    // §6.4.1 unit (G15): the §2.8.2 `OutcomeMsg` wire form (P2.20) — the surfaced per-item line carried by
    // §0.6 `ItemResult.reason`, adjacently tagged (`type`/`data`) camelCase. OUTBOUND-ONLY, so a SERIALIZE
    // pin (not a round-trip). One case per variant pins (1) the variant tag, (2) the embedded discriminant's
    // wire casing — `ConversionErrorKind` camelCase, `LossyKind` snake_case (its §2.9.1-catalog casing),
    // `SkipReason` camelCase — and (3) that a skip rides the `skipped` tag, NOT `failure` (§1.12 skip ≠ fail).
    #[test]
    fn outcome_msg_wire_form_is_adjacently_tagged_camelcase() {
        let failure = OutcomeMsg::Failure {
            kind: ConversionErrorKind::WriteFailed,
            text: "ConvertIA couldn't save the converted file to that location.".to_owned(),
        };
        assert_eq!(
            serde_json::to_string(&failure).expect("OutcomeMsg::Failure serializes"),
            r#"{"type":"failure","data":{"kind":"writeFailed","text":"ConvertIA couldn't save the converted file to that location."}}"#,
            "§2.8.2: Failure rides the `failure` tag with a camelCase ConversionErrorKind code"
        );

        let lossy = OutcomeMsg::Lossy {
            kind: LossyKind::ImageLossyCodec,
            text: "Some quality is lost saving to this format.".to_owned(),
        };
        assert_eq!(
            serde_json::to_string(&lossy).expect("OutcomeMsg::Lossy serializes"),
            r#"{"type":"lossy","data":{"kind":"image_lossy_codec","text":"Some quality is lost saving to this format."}}"#,
            "§2.8.2/§2.9: Lossy rides the `lossy` tag with a snake_case LossyKind catalog key"
        );

        let skipped = OutcomeMsg::Skipped {
            reason: SkipReason::Uncertain,
            text: "ConvertIA couldn't tell what kind of file this is, so it can't convert it."
                .to_owned(),
        };
        assert_eq!(
            serde_json::to_string(&skipped).expect("OutcomeMsg::Skipped serializes"),
            r#"{"type":"skipped","data":{"reason":"uncertain","text":"ConvertIA couldn't tell what kind of file this is, so it can't convert it."}}"#,
            "§1.12: a pre-flight skip rides the `skipped` tag (NOT `failure`), carrying a SkipReason"
        );
    }

    // §6.4.1 unit (G15): the §1.12 / §0.6 forward `SkipReason → ErrorKind` projection (P2.20). Pins the four
    // one-way mappings — including the explicitly non-invertible `Uncertain → Unrecognized` (§1.12). The
    // helper's own non-wildcard match is the COMPILE-TIME total-ness guard (a new SkipReason variant without a
    // mapping fails to compile there), so the projection can never silently fall behind the §0.6 SkipReason set.
    #[test]
    fn skip_reason_projects_forward_to_error_kind() {
        assert_eq!(
            skip_reason_to_error_kind(SkipReason::UnsupportedType),
            ConversionErrorKind::UnsupportedType
        );
        assert_eq!(
            skip_reason_to_error_kind(SkipReason::Uncertain),
            ConversionErrorKind::Unrecognized,
            "§1.12: the non-invertible mapping — Uncertain has no same-named ErrorKind"
        );
        assert_eq!(
            skip_reason_to_error_kind(SkipReason::Empty),
            ConversionErrorKind::Empty
        );
        assert_eq!(
            skip_reason_to_error_kind(SkipReason::Unreadable),
            ConversionErrorKind::Unreadable
        );
    }

    // §6.4.1 unit (G15): the §0.4.2 / §2.13 `AppFault` wire shape (P2.39.1) — the app://fault event payload,
    // camelCase `{ kind, message }`. OUTBOUND-ONLY (no `Deserialize`), so a SERIALIZE pin, not a round-trip.
    // Iterates the THREE §2.13 app-level `kind` variants the event ever carries ({EngineMissing, WebviewFault,
    // BundleDamaged}) so each one's camelCase wire string is locked inside the AppFault envelope (a rename of
    // an app-level variant changes a pin) — the runtime "only these three" invariant made checkable here.
    #[test]
    fn app_fault_wire_form_is_camelcase() {
        for (kind, wire_kind) in [
            (ConversionErrorKind::EngineMissing, "engineMissing"),
            (ConversionErrorKind::WebviewFault, "webviewFault"),
            (ConversionErrorKind::BundleDamaged, "bundleDamaged"),
        ] {
            let fault = AppFault {
                kind,
                message: "ConvertIA can't start because part of the app appears to be missing."
                    .to_owned(),
            };
            assert_eq!(
                serde_json::to_string(&fault).expect("AppFault serializes"),
                format!(
                    r#"{{"kind":"{wire_kind}","message":"ConvertIA can't start because part of the app appears to be missing."}}"#
                ),
                "§0.4.2/§2.13: AppFault is the camelCase app://fault payload ({{ kind, message }})"
            );
        }
    }

    // §6.4.1 unit (G15) / §2.8.2 / G23 completeness: EVERY ConversionErrorKind is homed — the 21 §2.8.2
    // conversion-outcome kinds each carry a non-empty catalog row, and the 4 non-conversion kinds
    // ({EngineMissing, WebviewFault, BundleDamaged} → §2.13.3 app-fault; MixedDrop → §5.2 pre-flight) return
    // None (homed elsewhere — one string, one home), NOT an unhomed kind. The exhaustive match in
    // conversion_message_template is the compile-time guard; this asserts the current 25 are correctly split.
    #[test]
    fn every_conversion_kind_is_homed() {
        use ConversionErrorKind as K;
        let conversion = [
            K::Corrupt,
            K::Empty,
            K::Unrecognized,
            K::UnsupportedType,
            K::UnsupportedPair,
            K::Unreadable,
            K::Gone,
            K::PasswordProtected,
            K::NoAudioTrack,
            K::TooBig,
            K::OutOfDisk,
            K::WriteFailed,
            K::PathTooLong,
            K::TooManyCollisions,
            K::EngineCrash,
            K::EngineHang,
            K::EngineError,
            K::PlatformUnavailable,
            K::QuarantinedByOs,
            K::CleanupResidue,
            K::InternalError,
        ];
        assert_eq!(
            conversion.len(),
            21,
            "§2.8.2 defines 21 conversion-outcome rows"
        );
        for kind in conversion {
            let row = conversion_message_template(kind);
            assert!(
                matches!(row, Some(s) if !s.is_empty()),
                "§2.8.2: {kind:?} must have a non-empty catalog row, got {row:?}"
            );
        }
        for kind in [
            K::EngineMissing,
            K::WebviewFault,
            K::BundleDamaged,
            K::MixedDrop,
        ] {
            assert_eq!(
                conversion_message_template(kind),
                None,
                "§2.8.2: {kind:?} is homed elsewhere (§2.13.3 / §5.2), not in this per-item catalog"
            );
        }
    }

    // §6.4.1 unit (G15) / §2.8.2: PIN every one of the 21 catalog rows to its EXACT canonical-English string
    // (templates carry the literal `{x}` slot for the 3 substituting kinds). This is an independent second
    // transcription of the §2.8.2 table, so a single-character/word mutation to any catalog string (the whole
    // deliverable of this box) fails here — closing the mutation-survival gap the non-empty check leaves.
    #[test]
    fn catalog_rows_match_the_exact_canonical_english() {
        use ConversionErrorKind as K;
        let expected: [(ConversionErrorKind, &str); 21] = [
            (K::Corrupt, "This file looks damaged and couldn't be converted."),
            (K::Empty, "This file is empty — there's nothing to convert."),
            (
                K::Unrecognized,
                "ConvertIA couldn't tell what kind of file this is, so it can't convert it.",
            ),
            (
                K::UnsupportedType,
                "ConvertIA can't convert this type of file — it looks like {detected}.",
            ),
            (K::UnsupportedPair, "That conversion isn't available."),
            (
                K::Unreadable,
                "ConvertIA couldn't open this file — it may be in use by another program, or you don't have permission to read it.",
            ),
            (
                K::Gone,
                "This file is no longer there — it may have been moved, renamed, or its drive removed.",
            ),
            (
                K::PasswordProtected,
                "This file is password-protected or copy-protected, so ConvertIA can't read it.",
            ),
            (K::NoAudioTrack, "This file has no audio to extract."),
            (
                K::TooBig,
                "This file is too large for ConvertIA to convert on this computer.",
            ),
            (
                K::OutOfDisk,
                "There isn't enough free disk space to finish this conversion.",
            ),
            (
                K::WriteFailed,
                "ConvertIA couldn't save the converted file to that location.",
            ),
            (
                K::PathTooLong,
                "The output name would be too long for this system, so this file was skipped. Try a shorter folder or file name.",
            ),
            (
                K::TooManyCollisions,
                "There are already too many files with this name in that folder, so this one couldn't be saved. Try a different folder.",
            ),
            (
                K::EngineCrash,
                "Something went wrong while converting this file, so it was skipped.",
            ),
            (
                K::EngineHang,
                "This file took too long to convert and was stopped.",
            ),
            (K::EngineError, "ConvertIA couldn't convert this file."),
            (
                K::PlatformUnavailable,
                "This conversion isn't available on {platform} because the required format support can't be included here.",
            ),
            (
                K::QuarantinedByOs,
                "macOS is blocking one of ConvertIA's built-in tools with a security check. Open System Settings → Privacy & Security and choose \"Open Anyway\", then try again.",
            ),
            (
                K::CleanupResidue,
                "This file couldn't be converted, and a temporary file may remain at {path}.",
            ),
            (
                K::InternalError,
                "Something unexpected went wrong, so this file was skipped. The rest of your files will continue.",
            ),
        ];
        for (kind, text) in expected {
            assert_eq!(
                conversion_message_template(kind),
                Some(text),
                "§2.8.2: {kind:?} must match its exact canonical-English row"
            );
        }
    }

    // §6.4.1 unit (G15) / §2.8.2: the three substituting kinds fill their single `{x}` slot from `arg`
    // (pinned to the exact substituted string — proving no slot leaks and the wiring is applied).
    #[test]
    fn conversion_failure_substitutes_the_single_slot() {
        assert_eq!(
            conversion_failure(ConversionErrorKind::UnsupportedType, "a ZIP archive"),
            Some(OutcomeMsg::Failure {
                kind: ConversionErrorKind::UnsupportedType,
                text: "ConvertIA can't convert this type of file — it looks like a ZIP archive."
                    .to_owned(),
            }),
            "§2.8.2: {{detected}} is substituted"
        );
        assert_eq!(
            conversion_failure(ConversionErrorKind::PlatformUnavailable, "Linux"),
            Some(OutcomeMsg::Failure {
                kind: ConversionErrorKind::PlatformUnavailable,
                text: "This conversion isn't available on Linux because the required format support can't be included here.".to_owned(),
            }),
            "§2.8.2: {{platform}} is substituted"
        );
        assert_eq!(
            conversion_failure(ConversionErrorKind::CleanupResidue, "C:/out/file.tmp"),
            Some(OutcomeMsg::Failure {
                kind: ConversionErrorKind::CleanupResidue,
                text: "This file couldn't be converted, and a temporary file may remain at C:/out/file.tmp."
                    .to_owned(),
            }),
            "§2.6.4/§2.8.2: {{path}} is substituted — the only failure that names a residue path"
        );
    }

    // §6.4.1 unit (G15) / §2.8.2: a kind with NO slot ignores `arg` (verbatim), and a non-§2.8.2 kind
    // yields None (homed elsewhere).
    #[test]
    fn conversion_failure_verbatim_for_plain_kinds_and_none_for_non_conversion() {
        assert_eq!(
            conversion_failure(ConversionErrorKind::Corrupt, "ignored"),
            Some(OutcomeMsg::Failure {
                kind: ConversionErrorKind::Corrupt,
                text: "This file looks damaged and couldn't be converted.".to_owned(),
            }),
            "§2.8.2: a slot-free kind renders verbatim, ignoring arg"
        );
        assert_eq!(
            conversion_failure(ConversionErrorKind::EngineMissing, "x"),
            None,
            "§2.8.2: a non-conversion kind is not produced as a per-item OutcomeMsg::Failure"
        );
    }

    // §6.4.1 unit (G15) / §2.8.2: the five batch-level summary strings (§1.12 assembles them) + the residue
    // tail, pinned to their exact canonical English with counts substituted.
    #[test]
    fn batch_summary_strings_are_canonical() {
        assert_eq!(
            BatchSummary::AllSucceeded { n: 5 }.text(),
            "All 5 files converted."
        );
        assert_eq!(
            BatchSummary::Partial {
                ok: 3,
                n: 5,
                fail: 2
            }
            .text(),
            "3 of 5 files converted. 2 couldn't be converted — see details."
        );
        assert_eq!(
            BatchSummary::AllFailed { n: 4 }.text(),
            "None of the 4 files could be converted."
        );
        assert_eq!(
            BatchSummary::Cancelled { ok: 2 }.text(),
            "Stopped. 2 files were already converted and kept; the rest were not started."
        );
        assert_eq!(
            WITH_RESIDUE_TAIL,
            "Some temporary files may remain — see details."
        );
    }
}
