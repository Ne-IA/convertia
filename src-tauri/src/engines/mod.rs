//! `crate::engines` — the §3.2 engine registry + `Engine` trait + selection, the §1.7 generic
//! invocation lifecycle (spawn / progress / cancel / timeout / error-map), and the §3.5 per-engine
//! argument construction. Every spawn routes through `crate::isolation` and the §0.9 pool.
//!
//! P2.13 authors the §3.2 engine-seam descriptor TYPES here — `EngineId` / `EngineKind` /
//! `EngineDescriptor` (§0.6) — ahead of the registry / `trait Engine` / selection BEHAVIOUR, which is
//! filled by P4.1. The descriptor types are the seam vocabulary the P4.1 registry + the §0.9 pool + the
//! §7.2 `EngineHealth` contract key on.
//!
//! This module is ALSO the §0.7 home of the §7.2.3 C-return DTO cluster — the app-info / engine-health wire
//! types the C11 `get_app_info` / C12 `get_engine_health` handlers return: `Platform` (P2.132) and `AppInfo`
//! (P2.112) here, `EngineStatus` / `EngineHealth` at P2.110 / P2.111. They are homed here because they EMBED
//! the engine-layer leaves (`Platform` / `EngineId`) and so cannot sit in the tier-3 `domain` leaf (a §0.7
//! tier-3 → tier-2 edge is forbidden), `crate::ipc` is thin and DEFINES no DTOs (every C-return type is
//! imported there, never declared), and they are not the outcome-referencing lifecycle/result types
//! `crate::orchestrator` homes (§0.7 ‡). [Build-Session-Entscheidung: P2.112]
//!
//! P3.4 additionally homes the §1.7 invocation-dispatch cluster + its transitively-embedded §3.2.2 plan-seam
//! hull (the P3.4 ↔ P4.2/P4.3/P4.6 reconcile): the `EngineInvocation` envelope + `InvocationResult` (§1.7),
//! the `Invocation`/`EngineProgram`/`StdinPlan`/`TempPath`/`PlanError`/`ProgressModel` plan-seam types
//! (§3.2.2 — `ProbeOutput` authored at P4.2), and the `dispatch` fn (the exhaustive `EngineProgram` routing). All
//! are core-INTERNAL (no `serde`/`specta`): the §1.9 FSM maps `InvocationResult` onto the wire `ErrorKind`
//! at P3.46, so nothing in this cluster crosses the IPC door.

// [Build-Session-Entscheidung: P2.13] dead_code expect — the §3.2 seam descriptor types are authored as
// CONTRACTS before their consumers exist: the registry / `trait Engine` / selection is P4.1, the §0.9 pool
// reads `EngineDescriptor.serialised_only` then, and `EngineId`'s wire registration rides the §7.2
// `EngineHealth` (C12) consumer (a later P2 box). So `EngineId`/`EngineKind`/`EngineDescriptor` are dead in
// the PRODUCTION build until consumed; the cfg(test) tests below construct them, so the TEST build is
// dead-code-clean. P2.110/P2.111 added the §7.2.3 `EngineStatus` + `EngineHealth` wire DTOs; P2.113 wired the
// C12 `get_engine_health` return `Result<EngineHealth, IpcError>`, which REGISTERS the whole graph into
// `bindings.ts` — but its honest `Err` shell CONSTRUCTS neither, so they stay dead (fields never read) until
// the P4.45 startup probe assembles the real `Ok(EngineHealth)` (their wire-form tests below construct them,
// so the test build stays clean). The §3.2.2 `Platform` leaf (P2.132) + its `AppInfo` (P2.112) embedder are now LIVE:
// P2.98 wired the C11 `get_app_info` to assemble a real `Ok(AppInfo)` (`AppInfo::gather()` below), which
// constructs `Platform` via `current_platform()` (and `AppInfo` rides into `bindings.ts`); the P4
// `capabilities(platform)` consumers construct `Platform` further. `expect` (not `allow`) auto-flags the
// moment the remaining seam types' consumers land — matching `crate::domain`/`crate::outcome`/
// `crate::orchestrator`.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the §3.2 engine-seam descriptor types EngineId/EngineKind/EngineDescriptor + the §7.2.3 EngineStatus/EngineHealth wire DTOs (P2.110/P2.111) are dead in the production build until the P4.1 registry/trait/selection + the §0.9 pool + the P4.45 startup probe construct them. The C12 get_engine_health return (P2.113) REGISTERS EngineStatus/EngineHealth into bindings.ts via its Result<EngineHealth, IpcError> signature, but its honest Err shell constructs neither, so their fields stay unread (dead) until the P4.45 probe assembles the real Ok(EngineHealth). AppInfo (P2.112) + the §3.2.2 Platform leaf (P2.132) are now LIVE — P2.98's C11 get_app_info assembles a real Ok(AppInfo) (AppInfo::gather()), constructing Platform via current_platform(); the P4 capabilities(platform) consumers construct Platform further. The P3.4 §3.2.2 plan-seam hull (Invocation/EngineProgram/StdinPlan/TempPath/PlanError/ProgressModel) + the §1.7 EngineInvocation/InvocationResult + the dispatch fn — plus the P3.5 minimal Engine trait, the PlanOutcome return, and the NativeCsvTsvEngine impl — are authored ahead of their consumers: the P4.1 §3.2.3 registry constructs the native engine, P3.44/P3.45 extend the P3.43 dispatch InProcessNative arm (cooperative cancel / wall-clock timeout — P3.45 adds the bounded_lane wall-clock wrapper, dead until dispatch is a live root), and P4.13 rewrites the subprocess arms — so the dispatch fn + the plan-seam hull stay dead in the production build until the P3.46 conductor calls dispatch (the cfg(test) tests below construct + exercise them — the native engine's plan() is called there — so the test build is dead-code-clean). The P3.41 §3.5.6 native transform (csv_tsv_transform / transform_bytes / CsvTsvTarget / TransformError / delimiter_byte) + its P3.44 cooperative-cancel TransformStatus + run_native_csv_tsv are WIRED by the P3.43 dispatch InProcessNative arm onto crate::pool::run_in_core but STAY dead in the production build until the P3.46 conductor makes dispatch a live root: rustc does NOT propagate liveness through a dead-but-present caller to its callees (a pub fn in a private module of a bin crate is not itself a root), so the whole InProcessNative chain (dispatch -> run_native_csv_tsv -> the transform + run_in_core) is dead until then. The P3.42 §3.5.6 CSV-injection literal-preservation checker (assert_injection_cells_preserved / InjectionCellNotPreserved) is dead until the P3.62 G32 corpus binding calls it over the injection fixture. The P4.2-authored §3.2.2 ProbeOutput (the parsed §3.2.1 probe result) is dead until the P4.9 probe-then-encode sequencing constructs it and the P4.1 plan_encode consumer reads it; its cfg(test) shape test constructs + reads all four fields, keeping the test build dead-code-clean."
    )
)]

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use specta::Type;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::detection::{
    classify_delimiter, classify_encoding, Delimiter, DelimiterClass, MAX_HEADER_WINDOW,
};
use crate::domain::{
    Availability, DroppedItem, FormatId, JobId, Target, TargetId, UserFacingFormat,
};
use crate::outcome::ConversionErrorKind;
use crate::pool::{LaneError, Pool, NATIVE_CSV_TSV_TIMEOUT};

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

// ─── §3.2.2 engine-layer leaf types referenced by the `Engine` trait (defined here, §3.2 is owner) ──
// `Platform` is the SINGLE §3.2 leaf PULLED IN-PHASE to P2 (the rest — `Direction` / `EngineCapability` /
// `PatentDisposition` / the `SourceFmt`/`TargetFmt` aliases — stay in P4.3 with the `Engine` trait): the C11
// `AppInfo` contract embeds it (`AppInfo.platform: Platform`, §7.2.3 / P2.112), so it is authored here in
// `crate::engines` — its §3.2.2/§0.7 home, NOT the `crate::platform` OS-primitive shim (a false-friend
// name) — to keep the whole C1–C13 surface (and its G23 completeness gate P2.36) inside P2. From P4 the
// `Engine` trait's `capabilities(platform: Platform, …)` and the §3.4 patent disposition consume it; the
// dependency arrow runs Engine→Platform, so `Platform` has zero dependency on P4 and is freely authorable
// now (§3.2.2).

/// The running/target platform. Resolved at build/startup; drives both `capabilities()` and the §3.4
/// patent disposition (§3.2.2). One variant per shipped desktop OS — Windows / macOS / Linux (§1: one
/// artifact per platform; no mobile, web, or CLI build in v1).
///
/// [Build-Session-Entscheidung: P2.132] WIRE type — it rides `AppInfo.platform` into the C11 `get_app_info`
/// return (§7.2.3), so it derives `Serialize` + `Type`; it is exported into `bindings.ts` ONLY
/// TRANSITIVELY via that `AppInfo` embedder once C11 lands (P2.112/P2.34), with NO standalone
/// `collect_types![]` registration — the established defer-to-consumer pattern (`EngineId` via C12,
/// `ScanProgress`/`ConversionEvent` via their channels; `register_ipc_*_types` is only for the
/// consumer-less universal types). OUTBOUND-ONLY — no command TAKES a `Platform` arg (C11 takes `{}`), so
/// NO `Deserialize`, mirroring the outbound-only `EngineId`/`crate::orchestrator` wire types. `Copy` is free
/// for a fieldless enum and the §3.2.2 trait passes it BY VALUE (`capabilities(platform: Platform, …)`);
/// `PartialEq`/`Eq` for the §3.4 disposition branch + the wire-form test. NO `Hash` — nothing keys a map on
/// it (unlike `EngineId`, the §0.9 `HashMap<EngineId, bool>` key).
///
/// [Build-Session-Entscheidung: P2.132] WIRE FORM `camelCase` — the §0.6 wire default (`win`/`macOS`/
/// `linux`; 00-architecture §0.6 "camelCase on the wire") that `AppInfo` (its camelCase embedder) and every
/// §0.6/§7.2 DTO carry. NOT `EngineId`'s `lowercase` deviation — that existed ONLY to stop `camelCase`
/// mangling the FF-prefixed `FFmpeg`/`FFprobe` into `fFmpeg`/`fFprobe`; `Platform`'s variants have no such
/// hazard, so the clean §0.6 default applies (`MacOS` → `macOS`, the canonical Apple spelling).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Platform {
    /// Windows — the Windows desktop build (§1).
    Win,
    /// macOS — the macOS desktop build (§1; the universal `lipo`-both-slices artifact, §6).
    MacOS,
    /// Linux — the Linux desktop build (§1).
    Linux,
}

/// **`AppInfo`** — the C11 `get_app_info` return (§7.2.3; §0.4.1 references it, §5.9 About screen displays
/// it). The in-bundle About payload: app version, CI build id, running platform, and the §3.7
/// third-party-licenses / NOTICE text. NO network — every field is gathered in-process by the C11 handler
/// (P2.34): `version` from `app.package_info()` / `CARGO_PKG_VERSION`, `build_id` from the §6 CI build id
/// (deterministic dev fallback; the producer is P2.98), `platform` from the §3.2.2 `Platform` leaf, and
/// `third_party_notice` from the bundled §3.7 THIRD-PARTY-LICENSES.txt resource.
///
/// [Build-Session-Entscheidung: P2.112] WIRE struct — the §0.6 outbound-wire convention shared by every
/// §0.6/§7.2 DTO: `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]` + `#[serde(rename_all =
/// "camelCase")]` (cf. `PreflightVerdict`/`OutputPlanPreview`/`RunResult` in `crate::orchestrator`). NOT
/// `Copy` (it owns `String` fields). OUTBOUND-ONLY — C11 takes `{}` and no command takes an `AppInfo` arg,
/// so NO `Deserialize` (mirroring the outbound-only orchestrator result types). Registered into
/// `bindings.ts` TRANSITIVELY via the C11 return once P2.34 lands, with NO standalone `collect_types![]` —
/// the defer-to-consumer pattern its `Platform` field also rides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    /// The app semver version, e.g. `"1.0.0"` — `app.package_info().version` / `CARGO_PKG_VERSION` (§7.2.3).
    pub version: String,
    /// The §6 CI build identifier (deterministic dev fallback; producer P2.98) — wire key `buildId`.
    pub build_id: String,
    /// The running/target platform (§3.2.2) — rides as its own camelCase discriminant under wire key `platform`.
    pub platform: Platform,
    /// The bundled §3.7 THIRD-PARTY-LICENSES.txt contents for the §5.9 About screen — wire key `thirdPartyNotice`.
    pub third_party_notice: String,
}

/// The §6 CI build identifier for the §7.2.3 `AppInfo.build_id`, injected by `build.rs` as a `rustc-env`
/// (P2.98). Compile-time-guaranteed present (`env!`, never empty — §7.2.3 "neither field may silently ship
/// empty"): `<short-sha>-<run-id>` in a GitHub Actions build, the literal `"dev"` locally.
/// [Build-Session-Entscheidung: P2.98]
const BUILD_ID: &str = env!("CONVERTIA_BUILD_ID");

/// The bundled §3.7 third-party-licenses / NOTICE text for the §7.2.3 `AppInfo.third_party_notice`, embedded
/// at compile time from the canonical repo-root `THIRD-PARTY-LICENSES.txt`. [Build-Session-Entscheidung: P2.98]
/// `include_str!` (a compile-time embed IS "bundled", §7.2.3) of the §3.7/§6.3.2 GENERATED file — the release
/// step regenerates its CONTENTS from `engines.lock` + the SBOM, so C11 needs no code change when the
/// per-engine sections fill (P5-P7) / finalize (P10). **Ordering constraint:** because this is a compile-time
/// embed, the About/embedded copy is frozen at compile, so the release must ensure it matches the shipped §3.7
/// file — the constraint + its two fixes (assert embed == file in the §6.3.3 gate, or re-home the compile
/// after notice generation) are recorded on the owning release box P10.18. In P2 this is the committed
/// placeholder ("no bundled engines recorded yet" — the true state, no engines staged until P4+).
const THIRD_PARTY_NOTICE: &str = include_str!("../../../THIRD-PARTY-LICENSES.txt");

// [Build-Session-Entscheidung: P2.98] The running §3.2.2 platform, resolved from the compile target as a
// `const` per `cfg(target_os)` (§7.2.3; one artifact per OS, §1). A target outside the shipped three fails
// the build with a clear message, keeping the `Platform` enum and the buildable targets in lockstep.
#[cfg(target_os = "windows")]
const CURRENT_PLATFORM: Platform = Platform::Win;
#[cfg(target_os = "macos")]
const CURRENT_PLATFORM: Platform = Platform::MacOS;
#[cfg(target_os = "linux")]
const CURRENT_PLATFORM: Platform = Platform::Linux;
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
compile_error!(
    "§1/§3.2.2: ConvertIA ships only Windows / macOS / Linux — no Platform for this target_os"
);

/// The running §3.2.2 `Platform` (§7.2.3), resolved from the compile target. [Build-Session-Entscheidung: P2.98]
pub fn current_platform() -> Platform {
    CURRENT_PLATFORM
}

impl AppInfo {
    /// Assemble the real C11 `get_app_info` payload (§7.2.3, P2.98) — every field gathered in-process /
    /// in-bundle, NO network (§2.11): `version` from the crate `CARGO_PKG_VERSION`; `build_id` from the
    /// `build.rs` §6 producer; `platform` from the running target; `third_party_notice` from the bundled §3.7
    /// notice. [Build-Session-Entscheidung: P2.98] `version` via `CARGO_PKG_VERSION` is identical to
    /// `app.package_info().version` — `tauri.conf.json` omits `version`, so Tauri inherits it from `Cargo.toml`,
    /// and §7.6.2 offers either; reading it here keeps C11 `AppHandle`-free, so `get_app_info` stays a pure,
    /// unit-testable command (this crate ships no `tauri::test` mock harness by decision).
    pub fn gather() -> Self {
        AppInfo {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            build_id: BUILD_ID.to_owned(),
            platform: current_platform(),
            third_party_notice: THIRD_PARTY_NOTICE.to_owned(),
        }
    }
}

/// **`EngineStatus`** — one engine's row in the C12 `EngineHealth` return (§7.2.3; §0.4.1 C12 references
/// `EngineHealth`, which embeds `Vec<EngineStatus>`). The cached result of the §7.2.3 startup presence /
/// integrity / smoke probe for a single **registry-eligible** engine (FFmpeg, LibreOffice, Poppler, Pandoc,
/// ImageCore, NativeCsvTsv). The non-trait delegate/probe binaries get NO standalone row — `FFprobe` rolls
/// into `FFmpeg`, `ImageMagick` into `ImageCore` (§7.2.3); `NativeCsvTsv`'s row is SYNTHESIZED (always
/// available in-core), not produced by the presence loop. This box authors the TYPE; the startup probe that
/// POPULATES it (and the `EngineHealth` roll-up) is P4.
///
/// [Build-Session-Entscheidung: P2.110] WIRE struct — it rides `EngineHealth.engines` into the C12
/// `get_engine_health` return (§7.2.3), so it derives `Serialize` + `Type` (the no-`any` guarantee), with the
/// §0.6 `camelCase` wire default (`id`/`present`/`integrityOk`/`runnable`) shared by every §0.6/§7.2 DTO (cf.
/// `AppInfo`). NOT `Copy` — the §0.6 struct convention (cf. `EngineDescriptor`/`PreflightVerdict`: a §0.6
/// struct is not `Copy` even when every field is). OUTBOUND-ONLY — C12 takes `{}` and no command takes an
/// `EngineStatus` arg, so NO `Deserialize` (mirroring `AppInfo`/`EngineId`/the outbound orchestrator types).
/// Registration into `bindings.ts` is DEFERRED to the C12 `EngineHealth` consumer (P2.111/P2.113) — the
/// established P2.2-P2.12 defer-to-consumer pattern its `id: EngineId` field also rides; nothing CONSTRUCTS an
/// `EngineStatus` in production until the P4 startup probe, so it is dead in the production build until then
/// (the module-level dead-code expectation covers it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    /// Which engine this row describes (§0.6).
    pub id: EngineId,
    /// The engine binary resolved at its expected §3.3.1 path (the §7.2.3 out-of-band presence check).
    pub present: bool,
    /// The binary matched the build-time hash manifest (or the cheap warm size+magic check), §7.2.3 integrity
    /// — wire key `integrityOk`.
    pub integrity_ok: bool,
    /// The §7.2.3 smoke-probe result: `Some(true|false)` if the probe ran, `None` if it was skipped (the
    /// warm-launch fast path, or the macOS spawn deferred past the window). Wire: `true` / `false` / `null`.
    pub runnable: Option<bool>,
}

/// **`EngineHealth`** — the C12 `get_engine_health` return (§7.2.3; §0.4.1 C12 references it). The cached
/// result of the §7.2.3 startup presence / integrity / smoke probe over the whole engine set. It feeds §5.2
/// (disable / omit unavailable targets) and the §7.2.4 startup-fault surface: a missing / corrupt /
/// non-runnable **required** engine escalates to a §2.13 app-level fault (`EngineMissing` / `BundleDamaged`),
/// not a per-item failure. This box authors the TYPE; the startup probe that POPULATES it is P4.
///
/// [Build-Session-Entscheidung: P2.111] WIRE struct — the C12 return, so `Serialize` + `Type` (the no-`any`
/// guarantee) + the §0.6 `camelCase` wire default (`engines` / `unavailableTargets` / `allCriticalOk`) shared
/// by every §0.6/§7.2 DTO. NOT `Copy` (owns two `Vec`s). OUTBOUND-ONLY — C12 takes `{}` and no command takes
/// an `EngineHealth` arg, so NO `Deserialize` (mirroring `AppInfo`/`EngineStatus`/`EngineId`). Registration
/// into `bindings.ts` is DEFERRED to the C12 `get_engine_health` consumer (P2.113), which pulls the whole
/// graph (`EngineHealth` → `EngineStatus` → `EngineId`, + `TargetId`) into the export — the established
/// P2.2-P2.12 defer-to-consumer pattern; nothing CONSTRUCTS an `EngineHealth` in production until the P4
/// startup probe, so it is dead in the production build until then (the module-level dead-code expectation
/// covers it). It embeds `crate::domain::TargetId` (a tier-3 leaf) — a downward §0.7 tier-2 → tier-3 edge,
/// allowed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EngineHealth {
    /// One `EngineStatus` per **registry-eligible** engine — FFmpeg, LibreOffice, Poppler, Pandoc, ImageCore,
    /// NativeCsvTsv (§7.2.3). Two §7.2.3 `[DECIDED]` shaping rules govern this vector (the §7.2.3 spec is the
    /// authoritative home; recorded here as the contract the P4 probe must honor):
    ///
    /// - **Non-trait roll-up (P2.111.1):** the non-trait delegate / probe binaries — `FFprobe` and
    ///   `ImageMagick` (§0.6) — get **NO** standalone row. Their presence/integrity (checked by the §7.2.3
    ///   out-of-band binary loop) is **rolled into the owning engine's** `EngineStatus`: `FFprobe` → `FFmpeg`
    ///   (a missing/corrupt `ffprobe` makes FFmpeg's `runnable = Some(false)`, since no video job can probe),
    ///   `ImageMagick` → `ImageCore` (a missing BMP delegate makes ImageCore's `runnable = Some(false)`,
    ///   §7.2.3). Their `EngineId`s appear only in the §3.7 SBOM/NOTICE layer + that binary loop.
    /// - **NativeCsvTsv synthesized (P2.111.2):** `NativeCsvTsv` is `InProcessNative` (§3.5.6) — **not** in
    ///   the §3.3.1 binary list, so the §7.2.3 presence/integrity loop produces no row for it. Its
    ///   `EngineStatus` is **SYNTHESIZED** `{ present: true, integrity_ok: true, runnable: Some(true) }`
    ///   (always-available-in-core, pure-Rust, nothing to verify) and **appended after** the loop, never
    ///   produced from it.
    pub engines: Vec<EngineStatus>,
    /// The §3.4 patent-gapped targets unavailable on THIS platform (→ `PlatformUnavailable`, §2.8) — the §5.2
    /// disable/omit set. Wire key `unavailableTargets`. Populated from the §3.4 disposition matrix by P4.
    pub unavailable_targets: Vec<TargetId>,
    /// Derived — `true` iff every **required** engine is present + runnable (§7.2.3). A `false` here is what
    /// the §7.2.4 startup sequence escalates to a §2.13 app-level fault. Wire key `allCriticalOk`.
    pub all_critical_ok: bool,
}

// ─── §3.2.2 plan-seam hull + §1.7 dispatch envelope/result + the dispatch (P3.4) ──
// The §1.7 `EngineInvocation` envelope transitively embeds the §3.2.2 `Invocation` (via `plan`), and the
// dispatch matches `Invocation.program` (reading `Invocation.progress` is the §1.11 concern P4.8 wires) — so
// P3.4 authors the whole transitive hull here at its §3.2.2/§1.7 literal shape (the P3.4 ↔ P4.2/P4.3/P4.6
// reconcile — CLOSED at P4.2: the five P3.4 types verified against §3.2.2 verbatim, zero residual delta).
// `ProbeOutput` (P4.2-authored, below) is the §3.2.1 two-phase probe leg — P4-only, referenced by neither the
// envelope nor P3.5's `plan()`. All hull types are core-INTERNAL (no `serde`/`specta`): the §1.9 FSM maps
// `InvocationResult` onto the wire `ErrorKind` at P3.46, the ONE conversion. [Build-Session-Entscheidung: P3.4]

/// How the Rust core locates the bundled program to run for one [`Invocation`] (§3.2.2). Engines are spawned
/// Rust-side (§3.3.3), never via the WebView shell. `InProcessNative` is the ONLY non-subprocess variant —
/// ConvertIA's own MIT in-core CSV/TSV engine (§3.5.6); there is NO in-process path for any decoder of
/// untrusted third-party bytes (§2.12.4 absolute). §3.2.2 has **no `Subprocess` variant** — that name is the
/// §0.6 [`EngineKind`] (above); the two subprocess-class programs are `Sidecar` + `ResourceBin`.
///
/// [Build-Session-Entscheidung: P3.4] INTERNAL (a field of the internal [`Invocation`], never on the wire) —
/// `Debug, Clone, PartialEq, Eq`; NOT `Copy` (`ResourceBin.rel: PathBuf` is not `Copy`); no `serde`/`specta`
/// (mirroring the internal `EngineKind`/`EngineDescriptor`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineProgram {
    /// An `externalBin` sidecar (§3.3.1) resolved beside the app exe via `current_exe().parent()` (§3.3.3) —
    /// FFmpeg / FFprobe + the libvips image-worker (a separate short-lived subprocess, §3.5.5). The `EngineId`
    /// resolves the bare `<name>[.exe]` Tauri strips the staged triple to at bundle time.
    Sidecar(EngineId),
    /// A binary inside a bundled resources tree (§3.3.1), e.g. LibreOffice `soffice` — `engine` identifies it,
    /// `rel` is its path relative to the resources root.
    ResourceBin { engine: EngineId, rel: PathBuf },
    /// ConvertIA's own MIT in-core Rust engine — native CSV/TSV ONLY (§3.5.6). No spawn, no third-party native
    /// code; the one `EngineKind::InProcessNative` program.
    InProcessNative(EngineId),
}

/// How the engine's stdin is supplied (§3.2.2 / §3.5) — pandoc sometimes reads source bytes on stdin.
/// [Build-Session-Entscheidung: P3.4] INTERNAL, fieldless — `Debug, Clone, Copy, PartialEq, Eq`, no `serde`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdinPlan {
    /// The engine reads its input from a path argument (the common case).
    None,
    /// The core pipes the source bytes to the engine's stdin (§3.5).
    PipeBytes,
}

/// The per-invocation progress model (§3.2.2). Progress is a **per-invocation** property, NOT a per-engine
/// constant — the one video FFmpeg engine emits a `CoarseSpawnDone` probe `Invocation` and an
/// `FfmpegKeyValue` encode `Invocation` — so the §1.7 dispatch reads it from `Invocation.progress` and §1.11
/// normalises it (no `progress_model()` trait method).
///
/// [Build-Session-Entscheidung: P3.4] INTERNAL — `Debug, Clone, Copy, PartialEq, Eq` (every variant is
/// `Copy`), no `serde`. The per-variant stdout/stderr-handling dispatch is P4.8; P3's live value is
/// `InProcessFraction` (the native CSV/TSV self-reported fraction, §3.5.6, wired P3.43).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressModel {
    /// FFmpeg `-progress` key=value stream; the denominator is the ffprobe `duration_us` (video.md).
    FfmpegKeyValue { duration_us: u64 },
    /// The image-worker marshals libvips' eval-progress callback to stdout `progress=<0..100>` key=value lines
    /// across the worker's process boundary (§3.5.5), parsed by the §1.7 same line reader as `FfmpegKeyValue`.
    VipsStdout,
    /// LibreOffice / pandoc / poppler (and the video PROBE sub-invocation): 0% → spin → 100%, no streamed
    /// fraction — §1.7 dispatches it through the coarse spawn→done path, never the line reader.
    CoarseSpawnDone,
    /// The one in-process engine (`EngineProgram::InProcessNative`, the native CSV/TSV transform, §3.5.6): no
    /// stdout to line-read — it self-reports a real `bytes_processed / source_size` fraction per N-KB chunk
    /// (§1.11) over an in-process `mpsc::Sender<f32>` (the §1.7 `InProcessNative` sub-case, wired P3.43).
    InProcessFraction,
}

/// The §3.2.2 publish-temp the engine writes its output to — `tempfile::TempPath` (a path whose file is
/// deleted on drop, matching the §2.1 "path deleted on drop / never a placeholder" semantics). Picked by
/// `crate::run` inside the destination volume (§2.14.4) and owned by the §1.7 invocation; the §2.1 atomic
/// publish consumes it on item success, so drop is a no-op then. [Build-Session-Entscheidung: P3.4] the §3.2.2
/// named type — this box promotes `tempfile` dev→prod for it (already in `Cargo.lock`, no new package).
pub type TempPath = tempfile::TempPath;

/// The fully-constructed plan for one engine invocation (§3.2.2) — argv / cwd / env / stdin / progress-model /
/// output-temp, the single source of the spawn's shape. Built PURE by `Engine::plan()` (§3.2.2, P3.5), then
/// submitted to the §1.7 lifecycle wrapped in an [`EngineInvocation`]; §3.5 constructs `args`/`env` inside
/// `crate::isolation`. **`out_tmp` is populated by §1.7 at spawn time, never by `plan()`** (the 2026-07-07
/// plan-seam ruling): `Engine::plan()`/`plan_encode()` are Pure and construct the struct with `out_tmp: None`,
/// borrowing the temp only to embed its path in argv; §1.7 — the temp's owner (the §3.2.2 `TempPath`
/// lifecycle) — populates `out_tmp = Some(temp)` on the ENCODE invocation after the call returns. So the
/// SPAWN-TIME shape is `Some` for every encode (the §2.1 publish artifact) and `None` for a read-only
/// sub-invocation with no publish artifact — the video PROBE (`ffprobe`, §3.2.1), which stays `None` for its
/// whole leg; §1.7 atomic-publishes ONLY when `out_tmp.is_some()`.
///
/// [Build-Session-Entscheidung: P3.4] INTERNAL — no `serde`/`specta` (argv / env / a live `TempPath` are
/// core-only, never on the wire). Derives only `Debug`: `out_tmp` holds a `tempfile::TempPath`, which is
/// neither `Clone` nor `PartialEq` (it owns a unique on-disk temp deleted on drop — cloning/comparing it would
/// be wrong), so `Invocation` is moved, never cloned (the `crate::pool::Pool` precedent).
#[derive(Debug)]
pub struct Invocation {
    /// The resolved bundled program to run (§3.2.2).
    pub program: EngineProgram,
    /// The fully-constructed argument vector (§3.5), built inside `crate::isolation`.
    pub args: Vec<OsString>,
    /// The working directory — a per-run scratch dir (§2.14), or `None` to inherit.
    pub cwd: Option<PathBuf>,
    /// The isolated / minimal environment (§3.5 / §2.12) — never the inherited parent env.
    pub env: Vec<(OsString, OsString)>,
    /// How stdin is supplied (§3.5).
    pub stdin: StdinPlan,
    /// The per-invocation progress model (§1.11) the §1.7 dispatch reads.
    pub progress: ProgressModel,
    /// The publish-temp the engine writes to. **Constructed `None` at plan time and populated `Some(temp)` by
    /// §1.7 at spawn time** (the temp's owner; the 2026-07-07 plan-seam ruling) — so the spawn-time shape is
    /// `Some` for an encode, `None` for the read-only probe (§3.2.2); the §2.1 atomic publish consumes it on
    /// item success (drop is a no-op then). Typed with the §3.2.2 `TempPath` alias (= `tempfile::TempPath`) —
    /// the alias references an EXTERNAL type, so it does not trip the P2.19 within-module forward-declared-alias
    /// dead-code interaction.
    pub out_tmp: Option<TempPath>,
}

/// A PURE planning error (§3.2.2, no I/O): `Engine::plan()`/`plan_encode()` cannot build an [`Invocation`] for
/// this job (e.g. an option value out of range). The §1.7 lifecycle maps `kind` (a §2.8 [`ConversionErrorKind`],
/// typically `InternalError`/`UnsupportedPair`) onto the per-item outcome; distinct from a runtime failure.
///
/// [Build-Session-Entscheidung: P3.4] INTERNAL — `Debug, Clone, PartialEq, Eq`; NOT `Copy` (owns a `String`);
/// no `serde` (never on the wire — `kind` is projected onto the wire `ErrorKind` at the §1.9 boundary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanError {
    /// The §2.8.1 taxonomy kind this planning failure maps to (§3.2.2).
    pub kind: ConversionErrorKind,
    /// A short internal detail for the §7.5 log — NEVER surfaced raw to the user (SSOT *no stack traces*).
    pub detail: String,
}

/// The parsed result of a probe sub-invocation (§3.2.2, the §3.2.1 two-phase contract), produced by §1.7
/// from `ffprobe`'s stdout and handed to `plan_encode` (the P4.1 trait method) to finalise the encode
/// [`Invocation`]. Engine-layer-internal, like [`Invocation`]. `duration_us` becomes the
/// [`ProgressModel::FfmpegKeyValue`] denominator for the encode — PROVIDED here, never mutated onto a
/// pre-probe struct (§3.2.1's "no placeholder-then-mutate"). Video FFmpeg is the only v1 probe-requiring
/// engine; the shape is FFmpeg-shaped but the contract is generic.
///
/// [Build-Session-Entscheidung: P4.2] INTERNAL — `Debug, Clone, PartialEq, Eq`; NOT `Copy` (owns a `Vec`);
/// no `serde`/`specta` (never on the wire — it lives entirely between §1.7's probe parse and `plan_encode`),
/// mirroring the sibling [`PlanError`] derive set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOutput {
    /// Total media duration in microseconds — the §1.11 progress denominator (§3.2.2).
    pub duration_us: u64,
    /// The stream codecs — feeds the video.md remux-vs-reencode decision (§3.2.2).
    pub inner_codecs: Vec<String>,
    /// Display rotation in degrees, where flagged — feeds auto-orient (§3.2.2); `None` when unflagged.
    pub rotation_deg: Option<i32>,
    /// Flagged-interlaced — feeds the video.md deinterlace default (§3.2.2); `None` when unflagged.
    pub interlaced: Option<bool>,
}

/// What `Engine::plan()` produced — the §3.2.1 two-shape return, named at the type level (the 2026-07-07
/// plan-seam ruling). The discriminator §1.7 sequences on: under the `out_tmp` ownership contract every
/// plan-time [`Invocation`] constructs `out_tmp: None`, so `out_tmp.is_some()` cannot mark the probe.
/// Probe-ness is per-JOB, not per-engine (the same FFmpeg engine encodes audio single-step and probes video),
/// so it is NOT an [`EngineDescriptor`] flag — the engine names the shape on the value it returns.
///
/// [Build-Session-Entscheidung: P3.5] SOLE author (§3.2.2 owns the shape; the P3.5 minimal-trait box). INTERNAL
/// — no `serde`/`specta` (it wraps the core-only [`Invocation`], never on the wire). Derives only `Debug`:
/// [`Invocation`] is itself `Debug`-only (it owns a live `TempPath`), so `PlanOutcome` is moved, never cloned.
/// §1.7 matches it EXHAUSTIVELY (no `_ =>` catch-all — the §1.2/G29 dispatch-enum discipline the crate-root
/// `clippy::wildcard_enum_match_arm` deny enforces).
#[derive(Debug)]
pub enum PlanOutcome {
    /// A single-step engine's encode plan (the native CSV/TSV engine, and every image/office/pdf pair from P4
    /// on): §1.7 populates `out_tmp = Some(temp)` and dispatches it directly; `plan_encode` (a P4.1 trait
    /// method) is never called.
    Encode(Invocation),
    /// A probe-requiring engine's `ffprobe` sub-invocation (video FFmpeg, §3.2.1): `out_tmp` stays `None` for
    /// the whole probe leg (no publish artifact); §1.7 holds the temp, runs the probe, parses `ProbeOutput`,
    /// then calls `plan_encode`. No P3 engine produces it — the walking skeleton's one engine is single-step.
    Probe(Invocation),
}

/// The §1.7 dispatch ENVELOPE — NOT a second plan type. It wraps `(JobId, EngineId, Invocation,
/// CancellationToken)` and adds nothing the §3.2.2 [`Invocation`] already carries (no argv/cwd/env
/// re-declaration): the §1.7 lifecycle submits it to the §0.9 pool, dispatches on `plan.program`, and honours
/// `cancel` for the §1.7 group-kill / cooperative cancel.
///
/// [Build-Session-Entscheidung: P3.4] SOLE author of this §1.7 type (the P3.4 ↔ P4.6 reconcile; P4.6 is the
/// P4-side reconcile seat). INTERNAL — no `serde`; derives only `Debug` (embeds the `Debug`-only [`Invocation`]
/// + a `CancellationToken`, which is not `PartialEq`).
#[derive(Debug)]
pub struct EngineInvocation {
    /// The job this invocation runs (§0.6 `JobId` == the item's `ItemId`).
    pub job: JobId,
    /// The engine resolved for the job's pair (§3.2.3) — the §0.6 stable discriminant.
    pub engine: EngineId,
    /// The §3.2.2 plan artifact (program / args / cwd / env / stdin / progress / out_tmp).
    pub plan: Invocation,
    /// The §0.4.4 cancellation handle — tripped by C7 `cancel_run` (a cheap `Arc`-backed clone of the run's token).
    pub cancel: CancellationToken,
}

/// The terminal result of one §1.7 invocation (§1.7). `Failed` carries the Rust-internal §2.8
/// [`ConversionErrorKind`]; the orchestrator (`crate::orchestrator`, §0.7) maps it to the wire `ErrorKind` via
/// `ErrorKind::from(kind)` at the §1.9 Running→Failed transition (the identity under the §2.8.2 option-1
/// alias) and again at the §0.4.3 IPC boundary — one conversion.
///
/// [Build-Session-Entscheidung: P3.4] SOLE author of this §1.7 type. INTERNAL — no `serde`; `Debug, PartialEq,
/// Eq` (the caller matches/maps it, never clones — the `crate::pool::LaneError` precedent); `Succeeded` /
/// `Cancelled` are unit variants.
#[derive(Debug, PartialEq, Eq)]
pub enum InvocationResult {
    /// The invocation exited cleanly and its output verified (§1.7).
    Succeeded,
    /// The invocation failed — the §2.8 kind (spawn error / nonzero exit / hang / internal fault).
    Failed(ConversionErrorKind),
    /// The invocation was cancelled (user cancel → §1.7 group-kill / cooperative cancel).
    Cancelled,
}

/// The §1.7 dispatch — routes an [`EngineInvocation`] to its execution lane by `Invocation.program` and
/// returns the [`InvocationResult`]. The exhaustive match over [`EngineProgram`] is deny-gated (no `_ =>`
/// catch-all — the `clippy::wildcard_enum_match_arm` deny at the crate root, G4/G14/G29) so a future engine
/// program cannot be silently dropped.
///
/// **`on_progress`** is §1.7's per-fraction sink: the dispatch forwards every self-reported/parsed progress
/// fraction to it (P3.43 wires the `InProcessNative` lane's self-report; the subprocess lanes will feed the
/// same sink from their §3.5 line-reader at P4.13). It is a plain `f32` callback so `crate::engines` (a §0.7
/// tier-2 module) names **no** orchestrator-homed type: the tier-1 caller (the P3.46 conductor) supplies the
/// closure that wraps each fraction into the §0.4.2 `ItemProgress { runId, itemId, fraction, stage }` and
/// sends it over the channel — the fraction is §1.7's, the wire tick is the conductor's. `+ Send + 'static` so
/// §1.7 can move the sink into the concurrent progress-forwarding task (and the conductor can drive dispatch
/// on a spawned per-item task).
///
/// **P3 walking-skeleton state.** The `InProcessNative` lane is authored from P3.43: it runs the §3.5.6 native
/// CSV/TSV transform on `crate::pool::run_in_core` and forwards its self-reported fraction (P3.44 adds the
/// cooperative cancel, P3.45 the wall-clock timeout). It stays dead in the production build until the P3.46
/// conductor makes `dispatch` a live root (a `pub fn` in a private module of a bin crate is not itself a
/// root, and rustc does not propagate liveness to a dead fn's callees). The subprocess lanes are
/// unreachable-by-construction in the walking skeleton (no subprocess engine is registered — the registry +
/// engines land at P4.4) and still return the honest `InvocationResult::Failed(InternalError)` seam (§2.13,
/// the P2.25 unreachable-outcome precedent) until P4.13 routes them through `crate::isolation::run_confined`.
/// [Build-Session-Entscheidung: P3.4]
pub async fn dispatch(
    invocation: &EngineInvocation,
    pool: &Pool,
    on_progress: impl Fn(f32) + Send + 'static,
) -> InvocationResult {
    match &invocation.plan.program {
        // The one walking-skeleton lane — the native CSV/TSV engine (§3.5.6): run its transform on the §0.9
        // in-core permit lane, forward its self-reported fraction (P3.43), cooperatively poll the job's
        // cancellation token at each chunk boundary (P3.44), and bound it by the §0.9 native wall-clock timeout
        // (P3.45). [Build-Session-Entscheidung: P3.43]
        EngineProgram::InProcessNative(_) => {
            run_native_csv_tsv(
                &invocation.plan,
                &invocation.cancel,
                pool,
                on_progress,
                NATIVE_CSV_TSV_TIMEOUT,
            )
            .await
        }
        // Subprocess lanes — unreachable-by-construction in the P3 walking skeleton (no subprocess engine is
        // registered; the registry + engines land at P4.4). P4.13 authors crate::isolation::run_confined and
        // rewrites these arms to route through it; the honest InternalError seam holds meanwhile (§2.13, P2.25).
        EngineProgram::Sidecar(_) | EngineProgram::ResourceBin { .. } => {
            InvocationResult::Failed(ConversionErrorKind::InternalError)
        }
    }
}

/// The §1.7 `InProcessNative` lane (P3.43) — run the §3.5.6 native CSV/TSV transform on the §0.9 in-core
/// `spawn_blocking` permit lane ([`Pool::run_in_core`]) and forward its self-reported progress.
///
/// **Progress bridge (§1.7 InProcessNative sub-case).** Because this engine has no stdout to line-read, §1.7
/// hands the transform a bounded `mpsc::Sender<f32>` (`progress_tx`, capacity [`PROGRESS_CHANNEL_CAPACITY`])
/// captured inside the `run_in_core` closure; the synchronous transform calls `progress_tx.blocking_send` with
/// its `bytes_processed / source_size` fraction at each [`PROGRESS_CHUNK_BYTES`] chunk boundary (plus a final
/// `1.0`). §1.7 OWNS the matching `Receiver<f32>` in a concurrent forwarding task that hands each fraction to
/// `on_progress`; draining CONCURRENTLY with the blocking worker is what makes the bounded channel's
/// back-pressure a coalesce (a slow consumer parks the worker on a full buffer) rather than a deadlock. When
/// the transform ends it drops `progress_tx`; the forwarder drains to `None` and ends. A lane panic /
/// pool-closure ([`LaneError`]) is ONE item's `Failed(InternalError)`, never a pool-wide fault (§0.9 panic
/// isolation).
///
/// **Why the FORWARDER (not the lane) is `tokio::spawn`ed.** The `run_in_core` lane future is handed to
/// [`bounded_lane`], which awaits it under `tokio::time::timeout` — so on a §1.7 wall-clock timeout the lane
/// future is DROPPED, freeing its §0.9 permit at once (the permit-free-on-drop contract, [`Pool::run_in_core`])
/// while the blocking worker detaches. Spawning the WORKER instead would strand the permit until the abandoned
/// thread finished; spawning only the forwarder (the `rt` feature) lets the progress drain run concurrently
/// with the awaited worker without a `select!`. [`bounded_lane`] drains the forwarder on the within-bound path
/// and aborts it on timeout (so it does not linger waiting on the abandoned thread's `progress_tx`).
///
/// **Cooperative cancel + wall-clock timeout (P3.44 / P3.45).** The blocking closure polls a **child** of the
/// job's [`CancellationToken`] (`deadline_token`, cloned in as `poll_token`) at each chunk boundary. The child
/// trips on the user cancel (parent → child) — stopping the transform mid-stream ([`TransformStatus::Cancelled`]
/// → [`InvocationResult::Cancelled`]), the partial `out_tmp` discarded on drop with no §2.1 publish (§3.2.2) —
/// AND when [`bounded_lane`] trips it on a §1.7 wall-clock `timeout` expiry, so a non-wedged abandoned thread
/// bails at its next boundary WITHOUT the timeout cancelling the whole run. On expiry the item is
/// `Failed(EngineHang)` and the run CONTINUES (the §1.7 InProcessNative timeout sub-case / §2.12.4 bounded
/// in-core path; the wedged-uninterruptible-read residue parks in the pool's bounded headroom). The
/// `timeout` parameter is the §0.9-owned [`NATIVE_CSV_TSV_TIMEOUT`] (`dispatch` supplies it).
/// [Build-Session-Entscheidung: P3.43]
async fn run_native_csv_tsv(
    plan: &Invocation,
    cancel: &CancellationToken,
    pool: &Pool,
    on_progress: impl Fn(f32) + Send + 'static,
    timeout: Duration,
) -> InvocationResult {
    // The transform's two runtime params come from the plan's argv (§3.2.2 / NativeCsvTsvEngine::plan):
    // args[0] = the §2.3-resolved source path, args[1] = the output-format token. Index-free (`first`/`get`) —
    // a mis-built plan is an InternalError, never a panic (the in-core no-index/no-panic path, G4/G14).
    let Some(source) = plan.args.first().map(PathBuf::from) else {
        return InvocationResult::Failed(ConversionErrorKind::InternalError);
    };
    let Some(target) = plan
        .args
        .get(1)
        .and_then(|token| CsvTsvTarget::from_token(token))
    else {
        return InvocationResult::Failed(ConversionErrorKind::InternalError);
    };
    // §1.7 owns + populates `out_tmp` before dispatch (the 2026-07-07 plan-seam ruling); the transform WRITES
    // to it. A missing `out_tmp` on the encode invocation is a mis-wired lifecycle → InternalError.
    let Some(out_path) = plan.out_tmp.as_ref().map(|temp| temp.to_path_buf()) else {
        return InvocationResult::Failed(ConversionErrorKind::InternalError);
    };

    let (progress_tx, mut progress_rx) = mpsc::channel::<f32>(PROGRESS_CHANNEL_CAPACITY);
    // Forward every self-reported fraction to the sink until the transform drops `progress_tx` (recv → None).
    let forwarder = tokio::spawn(async move {
        while let Some(fraction) = progress_rx.recv().await {
            on_progress(fraction);
        }
    });

    // The blocking closure polls a CHILD of the job token (P3.44): it trips on the user cancel (parent →
    // child) AND when `bounded_lane` trips it on a §1.7 wall-clock timeout (P3.45) — the latter WITHOUT
    // cancelling the whole run (tripping the job token itself would). `deadline_token` stays in this frame for
    // `bounded_lane`; `poll_token` (a cheap Arc-sharing clone) crosses into the closure — CancellationToken is
    // Clone + Send + 'static, and the child shares the SAME cancellation state. [Build-Session-Entscheidung: P3.45]
    let deadline_token = cancel.child_token();
    let poll_token = deadline_token.clone();
    let lane = pool.run_in_core(move || -> Result<TransformStatus, TransformError> {
        // `create` opens the already-exclusively-created (`O_EXCL`, §2.14.1) publish temp for writing;
        // the §2.1 atomic publish CONSUMES it on success, so the engine only writes here (§3.2.2 TempPath).
        let out_file = std::fs::File::create(&out_path).map_err(TransformError::Write)?;
        // The sync loop self-reports through the bounded channel. A closed receiver (only if the forwarder
        // ended early) just stops the flow; in the success path the forwarder is live until the transform
        // drops progress_tx, so every send is delivered.
        let mut report = |fraction: f32| {
            let _ = progress_tx.blocking_send(fraction);
        };
        // The cooperative cancel/timeout poll (P3.44/P3.45): a `true` at a chunk boundary stops the transform.
        let mut should_cancel = || poll_token.is_cancelled();
        csv_tsv_transform(&source, target, out_file, &mut report, &mut should_cancel)
    });

    // Run the lane under the §1.7 wall-clock bound (P3.45): a lane that outruns `timeout` is abandoned (its
    // §0.9 permit freed on drop, the worker detached) → `Failed(EngineHang)`, the run continuing.
    bounded_lane(lane, forwarder, deadline_token, timeout).await
}

/// The terminal outcome of a §1.7 in-core lane before [`bounded_lane`] maps it to an [`InvocationResult`]: the
/// §3.5.6 transform's `Result<`[`TransformStatus`]`, `[`TransformError`]`>` wrapped in the §0.9 pool's
/// `Result<_, `[`LaneError`]`>` (a caught worker panic / closed pool). Named so the `bounded_lane` signature +
/// its tests avoid a `clippy::type_complexity` nesting. [Build-Session-Entscheidung: P3.45]
type LaneOutcome = Result<Result<TransformStatus, TransformError>, LaneError>;

/// Run one §1.7 in-core lane future under the §0.9 wall-clock timeout (P3.45), map its terminal outcome to an
/// [`InvocationResult`], and manage the progress `forwarder` — the §1.7 `InProcessNative` timeout sub-case.
/// Extracted from [`run_native_csv_tsv`] so the wall-clock mapping is unit-testable over a synthetic lane
/// (a never-completing `pending()` for the timeout arm, a `ready(..)` for each terminal arm) without a real hang:
///
/// - **Within the bound** (`timeout` returns `Ok`): drain the `forwarder` (so every buffered fraction reaches
///   the sink before returning), then map the lane outcome — [`TransformStatus::Completed`] →
///   [`InvocationResult::Succeeded`]; the cooperative [`TransformStatus::Cancelled`] (P3.44) →
///   [`InvocationResult::Cancelled`]; a §3.5.6 [`TransformError`] → its §2.8 [`ConversionErrorKind`]; a
///   [`LaneError`] (a caught worker panic / a closed pool, §0.9) → `InternalError` (ONE item's failure, never a
///   pool-wide fault).
/// - **On expiry** (`timeout` returns `Err(Elapsed)`): `tokio::time::timeout` has already DROPPED the lane
///   future, so its §0.9 permit is freed at once and the blocking worker detaches — the §1.7 "wedged-read
///   abandoned, not awaited" design (the thread parks in the pool's bounded headroom, §2.12.4). Trip the
///   cooperative poll (`deadline_token`, a child of the job token) so a still-progressing NON-wedged abandoned
///   thread bails at its next chunk boundary without touching the run, and abort the `forwarder` so it does not
///   linger waiting on the abandoned thread's `progress_tx`. The item is [`InvocationResult::Failed`] with
///   [`ConversionErrorKind::EngineHang`] and the run CONTINUES; a truly wedged uninterruptible read never
///   reaches a boundary — the accepted §1.7 residue, bounded by the pool's headroom.
async fn bounded_lane(
    lane: impl std::future::Future<Output = LaneOutcome>,
    forwarder: tokio::task::JoinHandle<()>,
    deadline_token: CancellationToken,
    timeout: Duration,
) -> InvocationResult {
    match tokio::time::timeout(timeout, lane).await {
        Ok(outcome) => {
            // The lane finished within the bound; drain the forwarder so every buffered fraction is delivered.
            // A forwarder panic (a panicking sink) is the caller's fault, not the lane's — its JoinError is ignored.
            let _ = forwarder.await;
            match outcome {
                Ok(Ok(TransformStatus::Completed)) => InvocationResult::Succeeded,
                // Cooperative cancel (P3.44): the transform stopped mid-stream; the caller drops the partial out_tmp.
                Ok(Ok(TransformStatus::Cancelled)) => InvocationResult::Cancelled,
                Ok(Err(error)) => InvocationResult::Failed(ConversionErrorKind::from(error)),
                Err(LaneError::Panicked | LaneError::PoolClosed) => {
                    InvocationResult::Failed(ConversionErrorKind::InternalError)
                }
            }
        }
        Err(_elapsed) => {
            // §1.7 wall-clock timeout: the lane future is already dropped (§0.9 permit freed, worker detached).
            // Best-effort cooperative stop for a non-wedged abandoned thread, then tear the forwarder down so it
            // does not linger on the abandoned thread's progress_tx.
            deadline_token.cancel();
            forwarder.abort();
            let _ = forwarder.await;
            InvocationResult::Failed(ConversionErrorKind::EngineHang)
        }
    }
}

// ─── §3.2 Engine trait (minimal walking-skeleton) + the native CSV/TSV engine (P3.5) ──
// P3.5 authors the §3.2.2 `Engine` registry-seam trait in its MINIMAL form — just `plan()` — together with the
// one walking-skeleton engine that impls it: the native CSV/TSV transform (§3.5.6). P4.1 EXPANDS the SAME trait
// (never a second one) to the full §3.2.2 surface — `descriptor()` / `capabilities()` / `plan_encode()` /
// `classify_failure()` — when the §3.2.3 registry + the subprocess engines land. [Build-Session-Entscheidung: P3.5]

/// A bundled conversion engine (§3.2.2) — one impl per engine binary/lib. The registry seam: §3.2.3 selection
/// resolves a job's `(source, target)` pair to one `Engine`, and §1.7 calls `plan()` to get the dispatch-ready
/// [`Invocation`]. **Minimal walking-skeleton surface (P3.5): `plan()` only.** P4.1 adds the `descriptor()` /
/// `capabilities()` / `plan_encode()` / `classify_failure()` methods to THIS trait (§3.2.2). `Send + Sync`
/// because the §3.2.3 registry stores engines behind a shared handle and §1.7 dispatches them across the §0.9
/// worker pool.
pub trait Engine: Send + Sync {
    /// Build the concrete, dispatch-ready plan for one job — **Pure: no I/O, no spawn** (§3.2.2). It only
    /// *describes* the invocation (program / argv / cwd / env / stdin / progress); §1.7 owns the actual
    /// spawn / cancel / timeout and populates `out_tmp` at spawn time.
    ///
    /// **Params are the job's tier-3 projection (the 2026-07-07 plan-seam ruling):** the §0.6 [`DroppedItem`]
    /// (detection + size) + [`TargetId`] + the effective read `input` path §1.7 hands in — NOT the tier-1
    /// orchestrator-homed `ConversionJob` (§0.7: `crate::engines` is tier 2 and cannot reference it). `input`
    /// is the §2.3-resolved source (or the §3.5.0 core-staged scratch copy from P4 on); argv embeds `input`,
    /// NEVER a path derived from `item`. `out_tmp` is BORROWED only so argv can embed its path — `plan()`
    /// constructs the returned [`Invocation`] with `out_tmp: None`; §1.7 owns the temp and populates
    /// `Some(temp)` on the ENCODE invocation after this call returns.
    ///
    /// Returns [`PlanOutcome::Encode`] (single-step) or [`PlanOutcome::Probe`] (a probe-requiring engine's
    /// `ffprobe` sub-invocation — §3.2.1) — the shape §1.7 sequences on. A pure planning failure (an option
    /// value out of range, an unexpected target) is a [`PlanError`] carrying its §2.8 kind.
    fn plan(
        &self,
        item: &DroppedItem,
        target: TargetId,
        input: &Path,
        out_tmp: &TempPath,
    ) -> Result<PlanOutcome, PlanError>;
}

/// ConvertIA's own MIT in-core CSV/TSV engine (§3.5.6) — the ONE `EngineProgram::InProcessNative` engine and
/// the single engine the P3 walking skeleton runs. It decodes NO third-party bytes (pure memory-safe Rust), so
/// it is the sole sanctioned in-core conversion path (§2.12.4 absolute). The §3.2.3 registry (P4.1) holds one
/// instance.
///
/// [Build-Session-Entscheidung: P3.5] a fieldless unit struct — the engine carries no per-instance state (the
/// transform's parameters come from the job via `plan()`), so there is nothing to store.
pub struct NativeCsvTsvEngine;

impl Engine for NativeCsvTsvEngine {
    /// Plan the native CSV↔TSV transform (§3.5.6). Pure: maps the chosen `target` to its output format token
    /// and builds the dispatch-ready [`Invocation`] — no I/O, no spawn. Single-step, so it always returns
    /// [`PlanOutcome::Encode`]; `plan_encode` (a P4.1 trait method) is never reached.
    ///
    /// **`args` carries the transform's two runtime parameters** [Build-Session-Entscheidung: P3.5]: the
    /// effective read `input` path (`args[0]`, embedded per the §3.2.2 ownership contract — the transform reads
    /// THIS path, never one derived from `item`) and the **target format token** (`args[1]` ∈ {`csv`, `tsv`},
    /// the canonical §0.6 lowercase name). The P3.41 streamed transform reads `args[0]` as the source path and
    /// `args[1]` as the output format, applying that format's RFC-4180 delimiter + re-quoting rules; the
    /// P3.43-P3.45 executor forwards the same `Invocation`. [Derived-Assumption: P3.5 — the in-core engine
    /// carries `input` in argv like every subprocess engine (§3.2.2 "argv embeds input"), since [`Invocation`]
    /// has no dedicated input field and the §1.7 dispatch envelope holds only the `Invocation`.]
    ///
    /// `item`/`out_tmp` are unused here: the source delimiter is detected at RUNTIME by the transform
    /// (P3.27/P3.28), not planned, and the output temp is read from the `Invocation.out_tmp` §1.7 populates —
    /// not embedded in this in-core engine's argv (unlike a subprocess engine, whose argv names its output path).
    fn plan(
        &self,
        _item: &DroppedItem,
        target: TargetId,
        input: &Path,
        _out_tmp: &TempPath,
    ) -> Result<PlanOutcome, PlanError> {
        // Map the chosen target FORMAT to its canonical token; the P3.41 transform applies that format's
        // RFC-4180 delimiter + re-quoting rules. CSV↔TSV only — the §3.2.3 registry routes no other pair to
        // this engine, so an unexpected target is an InternalError (a mis-routed selection), not a user fault.
        // Compared by value (TargetId is Copy + Eq) rather than matched, to stay off the crate-root
        // `clippy::wildcard_enum_match_arm` deny without spelling out every §0.6 FormatId variant.
        let target_token = if target == TargetId::Format(FormatId::Tsv) {
            "tsv"
        } else if target == TargetId::Format(FormatId::Csv) {
            "csv"
        } else {
            return Err(PlanError {
                kind: ConversionErrorKind::InternalError,
                detail: "native CSV/TSV engine planned for a non-CSV/TSV target".to_owned(),
            });
        };
        Ok(PlanOutcome::Encode(Invocation {
            program: EngineProgram::InProcessNative(EngineId::NativeCsvTsv),
            args: vec![input.as_os_str().to_owned(), OsString::from(target_token)],
            cwd: None,
            env: Vec::new(),
            stdin: StdinPlan::None,
            progress: ProgressModel::InProcessFraction,
            out_tmp: None,
        }))
    }
}

// ─── §1.5 the walking-skeleton target lookup — the SHARED `UserFacingFormat → Target` map (P3.48) ─────────
// [Build-Session-Entscheidung: P3.48] The §1.5 "source → offered target(s)" resolution, homed here in
// `crate::engines` per the ruling (2026-07-12 P3.48 secondary-scope ruling (1)): the C6 conductor validates
// its wire `TargetId` arg through `resolve_slice_target` + build_batch reads the full `Target` it returns
// (§0.6 invariant 1 — one Target per Batch), and P3.49's C3 `get_targets` REUSES `slice_target` (the
// `needs: P3.48` edge on P3.49 is already set) — ONE source of the offer, no synthesized `Target` (a `Target`
// carries `label`/`lossy`/`availability`/`options` — §0.6 data; faking them is the P3.47-class invention).
// The v1 walking-skeleton offer is the CSV↔TSV pair ONLY; P4.1's §3.2.3 registry supplies the full §04
// matrices then (this lookup stays the CSV/TSV slice's authority, reused, not re-derived).

/// The §1.5 offered target for a walking-skeleton source format — `Some(Target)` for the two slice formats
/// (`Csv → TSV`, `Tsv → CSV`, the §04 spreadsheets CSV↔TSV pair, the ONLY diagonal-free pair the P3 slice
/// converts), `None` for every other §0.6 `UserFacingFormat` (offered by the P4.1 registry, not here). The
/// returned `Target` is the COMPLETE §0.6 offer — `id`, the display `label` (`"TSV"`/`"CSV"`, §5-facing),
/// `lossy: None` (a delimiter re-write is not a §2.9 predictable-loss), `availability: Available` (CSV/TSV are
/// platform-universal, no §3.4 patent gap), and an empty `options` (§1.6 — the slice takes no per-conversion
/// option). Compared BY VALUE against the two format ids (an `if`-chain, NOT a `match` — a 46-variant
/// `UserFacingFormat` match would need a `_` arm the crate-root `clippy::wildcard_enum_match_arm` deny
/// forbids, mirroring `NativeCsvTsvEngine::plan`'s target dispatch). [Build-Session-Entscheidung: P3.48]
#[must_use]
pub fn slice_target(source: UserFacingFormat) -> Option<Target> {
    let id = if source == UserFacingFormat::Csv {
        TargetId::Format(FormatId::Tsv)
    } else if source == UserFacingFormat::Tsv {
        TargetId::Format(FormatId::Csv)
    } else {
        return None;
    };
    let label = if id == TargetId::Format(FormatId::Tsv) {
        "TSV"
    } else {
        "CSV"
    };
    Some(Target {
        id,
        label: label.to_owned(),
        lossy: None,
        availability: Availability::Available,
        options: Vec::new(),
    })
}

/// Validate + resolve a wire `TargetId` against the source's §1.5 offer (the C6 `start_conversion` +
/// C3-reuse path) — `Some(Target)` iff `requested` is exactly the source's offered target (so a batch is
/// built only for a genuinely-offered pair, §0.6 invariant 1), `None` for a source with no slice offer OR a
/// `requested` that is not its offered target (a defensive `UnsupportedPair`, which the UI never presents —
/// §0.4.1 C3/§1.5). Filters `slice_target` by identity, so it can never construct a `Target` for an
/// unoffered pair. [Build-Session-Entscheidung: P3.48]
#[must_use]
pub fn resolve_slice_target(source: UserFacingFormat, requested: TargetId) -> Option<Target> {
    slice_target(source).filter(|target| target.id == requested)
}

// ─── §3.5.6 native CSV/TSV streamed transform (P3.41) ──────────────────────────────────────────────────
// [Build-Session-Entscheidung: P3.41] The one in-core §2.12.4-sanctioned conversion body — pure memory-safe
// Rust, no third-party C/C++ decoder. It re-detects the source's encoding + delimiter at RUNTIME via
// `crate::detection` (P3.27/P3.28) — the P3.5 `plan()` contract ("the source delimiter is detected at RUNTIME
// by the transform"), which PRE-SANCTIONED this `engines`->`detection` edge in a committed box. It is a
// same-tier-2 acyclic CONSUME edge: `detection` never imports `engines` (engines strictly consumes detection's
// sniff, so they are NOT mutually-independent), the same class as the existing `engines`->`outcome` edge — so
// it is NOT the forbidden mutually-independent-SIBLING case the P3.38 `run`<->`fs_guard` ruling rejected (both
// are tier-2, so the "down" is by consume-direction, not a tier drop). Dead in the production build until the
// P3.43-P3.45 §1.7 InProcessNative
// lifecycle rewrites the dispatch arm to run it (the module dead_code expect); no-panic (the in-core
// detect/transform path, G4/G14).

/// The §3.5.6 output format the native transform writes — its RFC-4180 field delimiter. Parsed from the plan's
/// `args[1]` token (`csv`/`tsv`, `NativeCsvTsvEngine::plan`, P3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsvTsvTarget {
    /// Comma-delimited output.
    Csv,
    /// Tab-delimited output.
    Tsv,
}

impl CsvTsvTarget {
    /// The target's field delimiter byte — `,` for CSV, `\t` for TSV.
    const fn delimiter(self) -> u8 {
        match self {
            CsvTsvTarget::Csv => b',',
            CsvTsvTarget::Tsv => b'\t',
        }
    }

    /// Parse the plan's `args[1]` output-format token (`NativeCsvTsvEngine::plan`, P3.5) — `Some` for the two
    /// canonical §0.6 lowercase tokens, `None` for any other (a mis-routed selection → the §1.7 executor's
    /// `InternalError`).
    pub fn from_token(token: &std::ffi::OsStr) -> Option<Self> {
        match token.to_str() {
            Some("csv") => Some(CsvTsvTarget::Csv),
            Some("tsv") => Some(CsvTsvTarget::Tsv),
            _ => None,
        }
    }
}

/// A §3.5.6 native-transform failure — mapped to the §2.8 [`ConversionErrorKind`] by the §1.7 executor
/// (P3.43-P3.45). [Build-Session-Entscheidung: P3.41]
#[derive(Debug)]
pub enum TransformError {
    /// The source is not decodable text (`classify_encoding` declined — a binary / UTF-32 / NUL-bearing input).
    /// The §3.2.3 registry routes only a Recognized CSV/TSV here, so this means the file changed since intake
    /// (or an intake edge) — the §2.10.2 "not text" case.
    NotText,
    /// A mixed / invalid byte sequence in the detected encoding (§2.10.2 "fail clearly, never emit mojibake") —
    /// or the defensive catch for an unexpected `csv` reader fault (the parse loop; not reached in practice, as
    /// the `ByteRecord` + `flexible` reader over an in-memory source parses permissively).
    Malformed,
    /// The source's delimiter is not consistently detectable (`classify_delimiter` → `Ambiguous`) — a
    /// structurally-inconsistent input the transform cannot re-quote faithfully.
    AmbiguousDelimiter,
    /// The source could not be read (an I/O failure at read time — vanished / permission).
    Read(io::Error),
    /// The output temp could not be written (an I/O failure — out of disk, etc.).
    Write(io::Error),
}

impl From<TransformError> for ConversionErrorKind {
    fn from(error: TransformError) -> Self {
        match error {
            // §2.10.2: a not-text / invalid-bytes / structurally-inconsistent input is a Corrupt source — the
            // transform never emits mojibake or a mis-quoted output.
            TransformError::NotText
            | TransformError::Malformed
            | TransformError::AmbiguousDelimiter => ConversionErrorKind::Corrupt,
            // §1.1 turn-time read failure: a source frozen at intake can vanish or lock by convert time —
            // now-missing (`NotFound`) → `Gone`; permission / lock / other IO → `Unreadable`, matching the
            // `outcome::read_failure_to_error_kind` split (the §1.1 invariant).
            TransformError::Read(error) => {
                if error.kind() == io::ErrorKind::NotFound {
                    ConversionErrorKind::Gone
                } else {
                    ConversionErrorKind::Unreadable
                }
            }
            TransformError::Write(_) => ConversionErrorKind::WriteFailed,
        }
    }
}

/// The terminal state of a §3.5.6 native transform pass (P3.44): the pass ran to the end, or the cooperative
/// §1.7 cancel poll stopped it at a chunk boundary. A cancel is NOT a [`TransformError`] (it is no failure) —
/// the §1.7 executor maps it to `InvocationResult::Cancelled`, the "cleanly discards the one in progress"
/// guarantee reached cooperatively (§1.7 InProcessNative sub-case), with the partial `out_tmp` discarded on
/// drop and no §2.1 atomic publish. [Build-Session-Entscheidung: P3.44]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformStatus {
    /// Every record was written and the writer flushed — the item Succeeded.
    Completed,
    /// The cooperative cancel poll fired at a chunk boundary; the pass stopped mid-stream. The `out_tmp` holds
    /// a partial, un-published `.part` temp discarded on drop (no atomic publish runs, §2.1/§3.2.2).
    Cancelled,
}

impl TransformError {
    /// The underlying I/O error for a read/write failure — the §7.5 diagnostic-log detail the §1.7 executor
    /// (P3.43-P3.45) records alongside the surfaced §2.8 kind (which carries no raw detail — SSOT *no stack
    /// traces*). `None` for the content failures (NotText / Malformed / AmbiguousDelimiter), which have no I/O
    /// source. [Build-Session-Entscheidung: P3.41]
    pub fn io_source(&self) -> Option<&io::Error> {
        match self {
            TransformError::Read(error) | TransformError::Write(error) => Some(error),
            TransformError::NotText
            | TransformError::Malformed
            | TransformError::AmbiguousDelimiter => None,
        }
    }
}

/// The N-KB chunk granularity for the §1.7 `InProcessNative` self-reported progress (§1.11): the native
/// CSV/TSV transform emits one `bytes_processed / source_size` fraction each time it crosses a
/// `PROGRESS_CHUNK_BYTES` boundary (P3.44's cooperative cancel will poll the token at the SAME granularity).
/// N = 100 KiB, so this value is ALSO the §1.7 "sub-100-KB → single 1.0 tick" gate: a source whose DECODED
/// text is smaller than one chunk crosses no boundary and emits only the final completion tick,
/// wire-indistinguishable from `CoarseSpawnDone` (the fraction, boundary + gate all share the decoded-text
/// unit — for the dominant UTF-8 case decoded text == source bytes). [Build-Session-Entscheidung: P3.43]
const PROGRESS_CHUNK_BYTES: usize = 100 * 1024;

/// The bounded-channel capacity for the §1.7 `InProcessNative` progress bridge (the InProcessNative sub-case):
/// the transform's `progress_tx.blocking_send` fractions cross from the blocking worker to §1.7's async
/// `Receiver` through this bounded `mpsc` channel, so a slow consumer applies natural back-pressure (the
/// blocking worker parks on a full buffer; fractions coalesce, memory stays bounded) rather than growing
/// unboundedly. Small — the native engine emits few ticks and the async drain keeps up. [Build-Session-Entscheidung: P3.43]
const PROGRESS_CHANNEL_CAPACITY: usize = 16;

/// Run the §3.5.6 native CSV/TSV transform (P3.41): read `source`, re-detect its encoding + delimiter, and
/// stream it to `out` at the `target` delimiter with RFC-4180 re-quoting, self-reporting progress via
/// `on_progress` (P3.43).
///
/// **§3.5.6 record pass:** the source is read into memory + decoded to UTF-8 (no BOM), then each RFC-4180
/// record is parsed at the source delimiter and re-written at the target delimiter — the `csv` writer quotes
/// only fields containing the new delimiter / a quote / a newline (RFC-4180 `QuoteStyle::Necessary`), so every
/// field's VALUE is preserved byte-for-byte (incl. a leading `= + - @` — the CSV-injection-safe literal
/// preservation, §3.5.6, bound by G32 at P3.42). Output line terminator = LF (`\n`)
/// [Build-Session-Entscheidung: P3.41] — deterministic + cross-platform (the P3.61 `sha256` determinism
/// sub-assertion), never the RFC-4180 CRLF.
///
/// **Progress (§1.7/§1.11 InProcessFraction, P3.43):** the read is **whole-file-buffered** (the §1.10 preflight
/// bounds the size), so the `bytes_processed / source_size` progress fraction is derived from the `csv` reader's
/// decoded-text position — a faithful 0→1 proxy for source-byte progress (exact at both endpoints, monotonic,
/// since processing is linear in both). `on_progress` is called with that fraction each time the reader crosses
/// a [`PROGRESS_CHUNK_BYTES`] boundary, plus a final `1.0` completion tick; a source whose decoded text is below
/// one chunk crosses no boundary and emits ONLY the final `1.0` (§1.7 "sub-100-KB → single tick"). `on_progress`
/// fires only on the
/// success path — a failed OR cancelled transform surfaces no completion tick. And `source` MUST be a
/// regular file: the FIFO / blocking-read pre-open type-check is the P3.49 read-path wiring's job (§2.12.4),
/// and the wall-clock / wedged-read time bound is P3.45 — this pass owns neither.
///
/// **Cooperative cancel (§1.7 InProcessNative sub-case, P3.44):** `should_cancel` is polled at **every chunk
/// boundary** — the same granularity as the progress tick. On a `true` poll the pass stops mid-stream and
/// returns [`TransformStatus::Cancelled`]; the caller drops the partial `out_tmp` (deleted on drop, §3.2.2)
/// and reports `Cancelled` with no §2.1 publish. A completed pass returns [`TransformStatus::Completed`].
pub fn csv_tsv_transform(
    source: &Path,
    target: CsvTsvTarget,
    out: impl Write,
    on_progress: &mut impl FnMut(f32),
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<TransformStatus, TransformError> {
    let bytes = std::fs::read(source).map_err(TransformError::Read)?;
    transform_bytes(&bytes, target, out, on_progress, should_cancel)
}

/// The pure byte→byte core of [`csv_tsv_transform`] (source bytes in, transformed bytes out) — the transform
/// LOGIC, separated from the file read so it is unit-testable over byte literals. Self-reports `bytes_processed
/// / source_size` progress through `on_progress` (P3.43) and polls `should_cancel` at each chunk boundary
/// (P3.44); see [`csv_tsv_transform`] for the fraction basis + the cooperative-cancel contract.
/// `pub(crate)` since P3.87: the crate-root `fuzz_api::csv_tsv_transform` wrapper drives exactly this
/// byte-level entry (the G48 fuzz surface — untrusted bytes, no file read), crate-internal only.
/// [Build-Session-Entscheidung: P3.41]
pub(crate) fn transform_bytes(
    bytes: &[u8],
    target: CsvTsvTarget,
    out: impl Write,
    on_progress: &mut impl FnMut(f32),
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<TransformStatus, TransformError> {
    // Re-detect over the SAME §1.2 bounded header window intake used (`classify_encoding`/`classify_delimiter`
    // sample <= MAX_HEADER_WINDOW), so the transform's re-detection matches the freeze's Recognized verdict.
    // Index-FREE (`get(..).unwrap_or`) — the same defense-in-depth §2.12.4 groups this in-core untrusted-byte
    // transform with the `crate::detection` sniffs: a short source (< the window) uses the whole buffer.
    let header = bytes.get(..MAX_HEADER_WINDOW).unwrap_or(bytes);
    let encoding = classify_encoding(header).ok_or(TransformError::NotText)?;

    // Decode to UTF-8 with the detected encoding. `decode` handles + strips the BOM; `had_errors` is true iff a
    // malformed sequence was replaced with U+FFFD — §2.10.2 "fail clearly, never emit mojibake".
    let (text, _, had_errors) = encoding.decode(bytes);
    if had_errors {
        return Err(TransformError::Malformed);
    }

    let source_delimiter = match classify_delimiter(header, encoding, None) {
        DelimiterClass::Detected(delimiter) => delimiter_byte(delimiter),
        DelimiterClass::Ambiguous => return Err(TransformError::AmbiguousDelimiter),
    };

    // RFC-4180 read at the source delimiter → write at the target delimiter. `flexible(true)` on BOTH tolerates
    // a ragged field count (a real-world CSV edge, spreadsheets.md) rather than erroring; `has_headers(false)`
    // treats every record uniformly (the header row is data to re-delimit, not a schema). `ByteRecord`
    // preserves field VALUE bytes exactly (the decode above already produced valid UTF-8).
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(source_delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut writer = csv::WriterBuilder::new()
        .delimiter(target.delimiter())
        .terminator(csv::Terminator::Any(b'\n'))
        .quote_style(csv::QuoteStyle::Necessary)
        .flexible(true)
        .from_writer(out);

    // §1.7/§1.11 self-reported progress (P3.43): emit `bytes_processed / source_size` each time the reader
    // crosses a PROGRESS_CHUNK_BYTES boundary. The fraction, the boundary, AND the small-input gate are all
    // measured on the DECODED-TEXT byte position/length (`text_len`) — a faithful 0→1 proxy for source-byte
    // progress (identical for the dominant UTF-8 case, monotonic + endpoint-exact otherwise; and processing
    // time is proportional to the decoded text, not the raw source, so the gate belongs on `text_len`, NOT
    // `bytes.len()`, or a shrinking/expanding encoding would mis-gate — §1.11 "real progress, working not
    // hung"). `report_chunks` gates the intermediate ticks: a sub-chunk decoded text crosses no boundary →
    // only the final 1.0 below (§1.7 "sub-100-KB → single tick"). Ticks are gated `< text_len` (position) and
    // `< 1.0` (value) so the sole 1.0 emitted is the final completion tick, never a duplicate at EOF.
    let text_len = text.len() as u64;
    let report_chunks = text_len >= PROGRESS_CHUNK_BYTES as u64;
    let mut next_boundary = PROGRESS_CHUNK_BYTES as u64;

    let mut record = csv::ByteRecord::new();
    loop {
        // The byte-level invalid-bytes failure is already handled above (`had_errors` → Malformed). The `csv`
        // reader itself parses PERMISSIVELY here (a `ByteRecord` never re-validates UTF-8, and `flexible(true)`
        // suppresses the unequal-field-count error over an in-memory source that cannot I/O-fail), so its `Err`
        // arm is a DEFENSIVE catch for an unexpected reader fault (mapped to `Malformed`), not reached in
        // practice. A write error is an out_tmp I/O failure. Either way the pass stops with no partial publish
        // (the §2.1 temp is discarded on drop).
        match reader.read_byte_record(&mut record) {
            Ok(true) => {
                writer
                    .write_byte_record(&record)
                    .map_err(|error| TransformError::Write(io::Error::other(error)))?;
                if report_chunks {
                    let position = reader.position().byte();
                    if position >= next_boundary && position < text_len {
                        // `< 1.0` guards the rare case where an intermediate rounds up to exactly 1.0f32
                        // (text_len > ~16 MiB with a boundary a few bytes before EOF) — it must never pre-empt
                        // the sole final 1.0. The boundary still advances past `position` either way.
                        let fraction = (position as f64 / text_len as f64) as f32;
                        if fraction < 1.0 {
                            on_progress(fraction);
                        }
                        while next_boundary <= position {
                            next_boundary =
                                next_boundary.saturating_add(PROGRESS_CHUNK_BYTES as u64);
                        }
                        // Cooperative cancel (§1.7 InProcessNative sub-case, P3.44): poll at the SAME chunk
                        // boundary as progress. On cancel, stop mid-stream and return Cancelled — the caller
                        // drops the partial out_tmp (§3.2.2) and runs no §2.1 publish. No final 1.0 tick fires
                        // (the item is Cancelled, not done). A sub-chunk source crosses no boundary, so it is
                        // effectively instant and completes before a cancel is polled.
                        if should_cancel() {
                            return Ok(TransformStatus::Cancelled);
                        }
                    }
                }
            }
            Ok(false) => break,
            Err(_) => return Err(TransformError::Malformed),
        }
    }
    writer.flush().map_err(TransformError::Write)?;
    // The completion tick (§1.11): the sole 1.0, and — for a sub-chunk source — the only tick emitted.
    on_progress(1.0);
    Ok(TransformStatus::Completed)
}

/// The literal delimiter byte a [`Delimiter`] splits on — the source delimiter for the `csv` reader (all four
/// §1.2 candidates are ASCII). [Build-Session-Entscheidung: P3.41]
const fn delimiter_byte(delimiter: Delimiter) -> u8 {
    match delimiter {
        Delimiter::Comma => b',',
        Delimiter::Semicolon => b';',
        Delimiter::Tab => b'\t',
        Delimiter::Pipe => b'|',
    }
}

// ─── §3.5.6 CSV-injection literal-preservation rule — the G31/G32 reader-side check (P3.42) ─────────────
// [Build-Session-Entscheidung: P3.42] The §3.5.6 "leading `= + - @` stay literal text" guarantee is already
// satisfied BY CONSTRUCTION by the P3.41 transform (`ByteRecord` preserves field bytes, and RFC-4180
// re-quoting keeps a delimiter/quote/newline-bearing injection cell one field) — the transform NEVER prefixes
// or mangles an injection cell (the §3.5.6 rule is literal PRESERVATION, NOT OWASP `'`-prefix neutralisation,
// which would alter data + break no-harm). This box makes that rule an ASSERTABLE, reusable READER-SIDE
// primitive: the behaviour the G31 per-format structural-reader clause specifies ("the corpus's leading
// `=`/`+`/`@` injection cells preserved literally as text", build-gates §6) and G32's (b) output-validity leg
// reuses, bound over the §6.4.5 corpus by P3.62 (`needs:` P3.61's injection fixture + this checker). Governed
// BY G31 (+ G32's (b) reuse) — it does NOT author a new gate; the `· G31 G32` markers name the gates this rule
// feeds. Dead in the production build until the P3.62 corpus binding calls it (the module dead_code expect);
// the `transform_tests` exercise it now.

/// A §3.5.6 CSV-injection literal-preservation violation (P3.42): an expected injection cell — a leading
/// `= + - @` field value — that did NOT survive as a literal field value in the transform OUTPUT.
#[derive(Debug, PartialEq, Eq)]
pub struct InjectionCellNotPreserved {
    /// The source injection cell (a field value) that is absent or mangled in the output.
    pub cell: Vec<u8>,
}

/// Assert the §3.5.6 CSV-injection literal-preservation RULE on a transform OUTPUT — the reader-side rule the
/// G31 per-format structural-reader clause specifies (reused by G32's (b) output-validity leg), bound over the
/// §6.4.5 corpus by P3.62: read `output` with a real RFC-4180 reader at `target_delimiter` and verify each
/// `injection_cell` (a known source `= + - @`-leading value) re-appears as a LITERAL field value — the exact
/// bytes, as ONE field: never split by the new delimiter, merged, re-quoted-away, prefixed, or otherwise
/// re-interpreted ("CSV-injection non-execution on the OUTPUT side", §3.5.6). Reading back with a REAL parser
/// (never a bare field-count parity) is the G31/G32 semantic. Returns `Err` naming the FIRST cell not preserved.
///
/// This is a PRESENCE check (position-independent — the cell survives as SOME literal field), sound because
/// P3.62 composes it with G31's own parseability + `output != input` + size-plausibility legs, and the caller
/// passes distinctive known corpus cells. The P3.41 transform satisfies the rule by construction; this box
/// makes it an assertable primitive, and P3.62 binds it over the injection fixture (P3.61).
/// [Build-Session-Entscheidung: P3.42]
pub fn assert_injection_cells_preserved(
    output: &[u8],
    target_delimiter: u8,
    injection_cells: &[&[u8]],
) -> Result<(), InjectionCellNotPreserved> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(target_delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(output);
    // Collect every output field's EXACT bytes. A read error means the output is not parseable as RFC-4180 (a
    // separate G31 output-validity failure); here it simply stops collection, so any not-yet-seen injection
    // cell surfaces below as a violation.
    let mut fields: Vec<Vec<u8>> = Vec::new();
    let mut record = csv::ByteRecord::new();
    while reader.read_byte_record(&mut record).unwrap_or(false) {
        fields.extend(record.iter().map(|field| field.to_vec()));
    }
    for &cell in injection_cells {
        if !fields.iter().any(|field| field.as_slice() == cell) {
            return Err(InjectionCellNotPreserved {
                cell: cell.to_vec(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::domain::{Confidence, DetectionOutcome, ItemId};

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

    // §6.4.1 unit (G15): the §3.2.2 `Platform` WIRE form (P2.132) — the leaf rides `AppInfo.platform` in
    // the C11 `get_app_info` return (§7.2.3). Pinned to its camelCase wire string per variant (the §0.6
    // "camelCase on the wire" default its `AppInfo` embedder carries); the count == 3 + the exhaustive
    // match below lock the set against §3.2.2 drift. A SERIALIZE pin (Platform is outbound-only — no
    // round-trip).
    #[test]
    fn platform_wire_form_is_camel_case() {
        let all: [(Platform, &str); 3] = [
            (Platform::Win, "win"),
            (Platform::MacOS, "macOS"),
            (Platform::Linux, "linux"),
        ];
        assert_eq!(
            all.len(),
            3,
            "§3.2.2: Platform is exactly the three shipped desktop OSes (no mobile/web/CLI build in v1)"
        );
        for (platform, wire) in all {
            assert_eq!(
                serde_json::to_string(&platform).expect("Platform serializes"),
                format!("\"{wire}\""),
                "§0.6/§3.2.2: each Platform serializes to its camelCase wire discriminant"
            );
        }
    }

    // The COMPILE-TIME variant lock (the established dependency-free exhaustive-match pattern, cf.
    // `engine_id_exhaustive`): adding/removing a `Platform` variant without updating this match fails to
    // compile, so the wire-form array above can never silently drift from §3.2.2.
    fn platform_exhaustive(platform: &Platform) {
        match platform {
            Platform::Win | Platform::MacOS | Platform::Linux => {}
        }
    }

    #[test]
    fn platform_exhaustive_match_is_exercised() {
        platform_exhaustive(&Platform::MacOS);
    }

    // §6.4.1 unit (G15): the §7.2.3 `AppInfo` WIRE form (P2.112) — the C11 `get_app_info` return. Pins the
    // camelCase field keys (version / buildId / platform / thirdPartyNotice) + the nested `Platform`
    // discriminant, the §0.6 "camelCase on the wire" convention every §0.6/§7.2 DTO carries; asserts the
    // snake_case keys are ABSENT (only camelCase reaches the wire). A SERIALIZE pin (AppInfo is
    // outbound-only — no round-trip); constructing the full 4-field struct keeps the TEST build
    // dead-code-clean and locks the field set (a field add/remove breaks this constructor at compile time).
    #[test]
    fn app_info_wire_form_is_camelcase() {
        let info = AppInfo {
            version: "1.0.0".to_owned(),
            build_id: "ci-0000000".to_owned(),
            platform: Platform::MacOS,
            third_party_notice: "Third-party licenses.".to_owned(),
        };
        let json = serde_json::to_value(&info).expect("AppInfo serializes");
        assert_eq!(json["version"], "1.0.0", "§7.2.3: version rides verbatim");
        assert_eq!(
            json["buildId"], "ci-0000000",
            "§0.6: build_id → camelCase buildId on the wire"
        );
        assert_eq!(
            json["platform"], "macOS",
            "§3.2.2: the nested Platform rides as its own camelCase discriminant"
        );
        assert_eq!(
            json["thirdPartyNotice"], "Third-party licenses.",
            "§0.6: third_party_notice → camelCase thirdPartyNotice on the wire"
        );
        assert!(
            json.get("build_id").is_none() && json.get("third_party_notice").is_none(),
            "§0.6: snake_case keys are NOT on the wire — camelCase only"
        );
    }

    // §6.4.1 unit (G15): the §3.2.2 `current_platform()` producer (P2.98) resolves the running `Platform` from
    // the compile target — the value that rides `AppInfo.platform` in the C11 `get_app_info` return (§7.2.3).
    // Runs on all three native CI legs (§6.4.4), pinning the per-OS cfg→variant mapping.
    #[test]
    fn current_platform_matches_the_compile_target() {
        let expected = if cfg!(target_os = "windows") {
            Platform::Win
        } else if cfg!(target_os = "macos") {
            Platform::MacOS
        } else {
            Platform::Linux
        };
        assert_eq!(
            current_platform(),
            expected,
            "§7.2.3/§3.2.2: current_platform() reflects the compile target (one artifact per OS, §1)"
        );
    }

    // §6.4.1 unit (G15): the §7.2.3 `AppInfo::gather()` producer (P2.98) assembles the real C11 payload from
    // in-process / in-bundle sources — the RELEASE-BLOCKING version + build_id (neither may ship empty) plus
    // the running platform and the bundled §3.7 notice. Read-back proof (test-strategy §0.2): the four fields
    // carry real values, not an empty shell.
    #[test]
    fn gather_assembles_the_real_appinfo_from_in_bundle_sources() {
        let info = AppInfo::gather();
        assert_eq!(
            info.version,
            env!("CARGO_PKG_VERSION"),
            "§7.2.3: version is the crate CARGO_PKG_VERSION (== app.package_info().version)"
        );
        assert!(
            !info.build_id.is_empty(),
            "§7.2.3: build_id is the §6 build.rs producer, never empty (the \"dev\" fallback locally)"
        );
        assert_eq!(
            info.platform,
            current_platform(),
            "§7.2.3: platform is the running compile target"
        );
        assert!(
            info.third_party_notice.contains("ConvertIA"),
            "§3.7: the bundled THIRD-PARTY-LICENSES.txt is embedded into thirdPartyNotice"
        );
    }

    // §6.4.1 unit (G15): the §7.2.3 `EngineStatus` WIRE form (P2.110) — one engine's row in the C12
    // `EngineHealth` return. Pins the camelCase field keys (id / present / integrityOk / runnable) + the
    // nested `EngineId` discriminant + the `runnable: Option<bool>` wire (Some → bool, None → null), the §0.6
    // "camelCase on the wire" convention every §0.6/§7.2 DTO carries; asserts the snake_case `integrity_ok`
    // key is ABSENT. A SERIALIZE pin (EngineStatus is outbound-only — no round-trip); constructing the full
    // 4-field struct keeps the TEST build dead-code-clean and locks the field set (a field add/remove breaks
    // this constructor at compile time).
    #[test]
    fn engine_status_wire_form_is_camelcase() {
        let probed = EngineStatus {
            id: EngineId::FFmpeg,
            present: true,
            integrity_ok: true,
            runnable: Some(false),
        };
        let json = serde_json::to_value(&probed).expect("EngineStatus serializes");
        assert_eq!(
            json["id"], "ffmpeg",
            "§0.6/§3.2: the nested EngineId rides as its lowercase discriminant"
        );
        assert_eq!(json["present"], true, "§7.2.3: present rides verbatim");
        assert_eq!(
            json["integrityOk"], true,
            "§0.6: integrity_ok → camelCase integrityOk on the wire"
        );
        assert_eq!(
            json["runnable"], false,
            "§7.2.3: runnable Some(false) → false on the wire (the probe ran)"
        );
        assert!(
            json.get("integrity_ok").is_none(),
            "§0.6: snake_case integrity_ok is NOT on the wire — camelCase only"
        );

        // §7.2.3: a skipped smoke probe → runnable None → JSON null, distinct from Some(false).
        let skipped = EngineStatus {
            id: EngineId::LibreOffice,
            present: true,
            integrity_ok: true,
            runnable: None,
        };
        let json = serde_json::to_value(&skipped).expect("EngineStatus serializes");
        assert!(
            json["runnable"].is_null(),
            "§7.2.3: runnable None (probe skipped) → null on the wire, distinct from Some(false)"
        );

        // §7.2.3: the negative/`Some(true)` arm — a missing engine whose smoke probe ran and passed the
        // binary check but is not runnable is impossible, but the field combination pins that `false` bools
        // ride as bare `false` and `runnable: Some(true)` rides as bare `true` (the passthrough arms the two
        // cases above don't cover).
        let missing = EngineStatus {
            id: EngineId::Poppler,
            present: false,
            integrity_ok: false,
            runnable: Some(true),
        };
        let json = serde_json::to_value(&missing).expect("EngineStatus serializes");
        assert_eq!(
            json["present"], false,
            "§7.2.3: present false rides as bare false"
        );
        assert_eq!(
            json["integrityOk"], false,
            "§7.2.3: integrity_ok false rides as bare false under the camelCase key"
        );
        assert_eq!(
            json["runnable"], true,
            "§7.2.3: runnable Some(true) → true on the wire"
        );
    }

    // §6.4.1 unit (G15): the §7.2.3 `EngineHealth` WIRE form (P2.111) — the C12 get_engine_health return.
    // Pins the camelCase field keys (engines / unavailableTargets / allCriticalOk) + the nested EngineStatus
    // rows + the nested externally-tagged TargetId, the §0.6 "camelCase on the wire" convention; asserts the
    // snake_case keys are ABSENT. Also exercises the §7.2.3 `[DECIDED]` NativeCsvTsv-synthesized row shape
    // (P2.111.2: `{ present: true, integrity_ok: true, runnable: Some(true) }`). A SERIALIZE pin
    // (EngineHealth is outbound-only — no round-trip); constructing the full struct locks the field set at
    // compile time (a field add/remove breaks this constructor).
    #[test]
    fn engine_health_wire_form_is_camelcase() {
        use crate::domain::UserFacingFormat;

        let health = EngineHealth {
            engines: vec![
                EngineStatus {
                    id: EngineId::FFmpeg,
                    present: true,
                    integrity_ok: true,
                    runnable: Some(true),
                },
                // §7.2.3/P2.111.2: the synthesized NativeCsvTsv always-available row.
                EngineStatus {
                    id: EngineId::NativeCsvTsv,
                    present: true,
                    integrity_ok: true,
                    runnable: Some(true),
                },
            ],
            unavailable_targets: vec![TargetId::Format(UserFacingFormat::Webp)],
            all_critical_ok: true,
        };
        let json = serde_json::to_value(&health).expect("EngineHealth serializes");
        assert_eq!(
            json["engines"][0]["id"], "ffmpeg",
            "§7.2.3: engines[] carries the per-engine EngineStatus rows"
        );
        assert_eq!(
            json["engines"][1]["id"], "nativecsvtsv",
            "§7.2.3/P2.111.2: the synthesized NativeCsvTsv row rides in engines[]"
        );
        assert_eq!(
            json["engines"][1]["runnable"], true,
            "§7.2.3/P2.111.2: the synthesized NativeCsvTsv row is always-available (runnable Some(true))"
        );
        assert_eq!(
            json["unavailableTargets"][0]["format"], "webp",
            "§0.6: unavailable_targets → camelCase unavailableTargets, each an externally-tagged TargetId"
        );
        assert_eq!(
            json["allCriticalOk"], true,
            "§0.6: all_critical_ok → camelCase allCriticalOk on the wire"
        );
        assert!(
            json.get("unavailable_targets").is_none() && json.get("all_critical_ok").is_none(),
            "§0.6: snake_case keys are NOT on the wire — camelCase only"
        );
    }

    // ─── P3.4: §3.2.2 plan-seam hull + §1.7 dispatch envelope/result + the dispatch ──
    //
    // The not(test) module dead-code expectation does NOT cover cfg(test), so a never-read field/variant would
    // red the TEST build under -D warnings — these tests read every field of every hull type (directly, or via
    // a derived `PartialEq` that reads all fields), so the test build stays dead-code-clean while the hull
    // remains dead in the production build until P3.5/P3.43-46/P4.13 construct + wire it.

    // A canonical InProcessNative native-CSV/TSV `Invocation` — every field set (read by
    // `invocation_holds_the_seven_plan_seam_fields`).
    fn native_csv_invocation() -> Invocation {
        Invocation {
            program: EngineProgram::InProcessNative(EngineId::NativeCsvTsv),
            args: vec![OsString::from("--delimiter"), OsString::from("tab")],
            cwd: Some(PathBuf::from("scratch/run-0")),
            env: vec![(OsString::from("LC_ALL"), OsString::from("C"))],
            stdin: StdinPlan::None,
            progress: ProgressModel::InProcessFraction,
            out_tmp: None,
        }
    }

    // Wrap an arbitrary `EngineProgram` in a full `EngineInvocation` for the dispatch tests.
    fn engine_invocation(program: EngineProgram) -> EngineInvocation {
        EngineInvocation {
            job: JobId::from_index(0),
            engine: EngineId::NativeCsvTsv,
            plan: Invocation {
                program,
                args: Vec::new(),
                cwd: None,
                env: Vec::new(),
                stdin: StdinPlan::None,
                progress: ProgressModel::InProcessFraction,
                out_tmp: None,
            },
            cancel: CancellationToken::new(),
        }
    }

    // §6.4.1 unit (G15): the §3.2.2 `Invocation` holds its seven plan-seam fields (P3.4). Pins the §3.2.2
    // shape — InProcessNative program, argv, scratch cwd, isolated env, no-stdin, self-reported progress, and
    // `out_tmp: None` (every plan-time Invocation constructs None; §1.7 populates Some(temp) at spawn time for
    // an encode — the 2026-07-07 plan-seam ruling) — and reads every field so the test build is dead-code-clean.
    #[test]
    fn invocation_holds_the_seven_plan_seam_fields() {
        let inv = native_csv_invocation();
        assert!(
            matches!(
                inv.program,
                EngineProgram::InProcessNative(EngineId::NativeCsvTsv)
            ),
            "§3.2.2: the native CSV/TSV plan carries the InProcessNative program"
        );
        assert_eq!(
            inv.args,
            vec![OsString::from("--delimiter"), OsString::from("tab")]
        );
        assert_eq!(inv.cwd, Some(PathBuf::from("scratch/run-0")));
        assert_eq!(
            inv.env,
            vec![(OsString::from("LC_ALL"), OsString::from("C"))]
        );
        assert_eq!(inv.stdin, StdinPlan::None);
        assert_eq!(inv.progress, ProgressModel::InProcessFraction);
        assert!(
            inv.out_tmp.is_none(),
            "§3.2.2: every plan-time Invocation constructs out_tmp None; §1.7 populates Some(temp) at spawn time for an encode (the 2026-07-07 plan-seam ruling)"
        );
    }

    // §6.4.1 unit (G15): the §3.2.2 `EngineProgram` models exactly the three program classes (P3.4) — the two
    // subprocess-class programs (`Sidecar` externalBin, `ResourceBin` inside the resources tree) + the one
    // `InProcessNative`. The equality comparisons read the inner `EngineId`/`rel` via the derived `PartialEq`.
    // There is NO `Subprocess` variant (that name is the §0.6 `EngineKind`).
    #[test]
    fn engine_program_models_the_three_program_classes() {
        assert_eq!(
            EngineProgram::Sidecar(EngineId::FFmpeg),
            EngineProgram::Sidecar(EngineId::FFmpeg)
        );
        assert_eq!(
            EngineProgram::ResourceBin {
                engine: EngineId::LibreOffice,
                rel: PathBuf::from("program/soffice"),
            },
            EngineProgram::ResourceBin {
                engine: EngineId::LibreOffice,
                rel: PathBuf::from("program/soffice"),
            },
            "§3.2.2: ResourceBin carries its owning EngineId + the resources-relative path"
        );
        assert!(matches!(
            EngineProgram::InProcessNative(EngineId::NativeCsvTsv),
            EngineProgram::InProcessNative(EngineId::NativeCsvTsv)
        ));
        assert_ne!(
            EngineProgram::Sidecar(EngineId::FFmpeg),
            EngineProgram::InProcessNative(EngineId::FFmpeg),
            "§3.2.2: the program CLASS is part of the identity (Sidecar != InProcessNative for one EngineId)"
        );
    }

    // §6.4.1 unit (G15): the §3.2.2 `ProgressModel` carries its four per-invocation variants (P3.4).
    // Comparing two `FfmpegKeyValue` values reads the `duration_us` field (the §1.11 denominator); the four
    // variants are pairwise distinct.
    #[test]
    fn progress_model_carries_all_four_variants() {
        assert_ne!(
            ProgressModel::FfmpegKeyValue { duration_us: 1 },
            ProgressModel::FfmpegKeyValue { duration_us: 2 },
            "§3.2.2: duration_us is part of the FfmpegKeyValue identity (the §1.11 progress denominator)"
        );
        let variants = [
            ProgressModel::FfmpegKeyValue { duration_us: 0 },
            ProgressModel::VipsStdout,
            ProgressModel::CoarseSpawnDone,
            ProgressModel::InProcessFraction,
        ];
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                assert_eq!(
                    i == j,
                    a == b,
                    "§3.2.2: the four ProgressModel variants are pairwise distinct"
                );
            }
        }
    }

    // §6.4.1 unit (G15): the §3.2.2 `StdinPlan` has exactly the path-arg (`None`) and pipe-bytes cases (P3.4).
    #[test]
    fn stdin_plan_has_none_and_pipe_bytes() {
        assert_ne!(
            StdinPlan::None,
            StdinPlan::PipeBytes,
            "§3.5: reading a path arg (None) is distinct from piping source bytes to stdin (pandoc)"
        );
    }

    // §6.4.1 unit (G15): the §3.2.2 `PlanError` carries a §2.8 kind + an internal detail (P3.4). The
    // equality comparison reads both fields via the derived `PartialEq`.
    #[test]
    fn plan_error_carries_its_kind_and_detail() {
        assert_eq!(
            PlanError {
                kind: ConversionErrorKind::UnsupportedPair,
                detail: "no engine for this pair".to_owned(),
            },
            PlanError {
                kind: ConversionErrorKind::UnsupportedPair,
                detail: "no engine for this pair".to_owned(),
            },
            "§3.2.2: a plan error maps a planning failure to its §2.8 kind + an internal detail string"
        );
    }

    // §6.4.1 unit (G15): the §1.7 `EngineInvocation` wraps `(JobId, EngineId, Invocation, CancellationToken)`
    // and adds nothing the §3.2.2 Invocation already carries (P3.4). Reads every field, and exercises the
    // §0.4.4 cancel handle (un-cancelled → tripped).
    #[test]
    fn engine_invocation_wraps_job_engine_plan_and_cancel() {
        let invocation = engine_invocation(EngineProgram::InProcessNative(EngineId::NativeCsvTsv));
        assert_eq!(
            invocation.job,
            JobId::from_index(0),
            "§1.7: the envelope carries the job's ItemId (§0.6 JobId == ItemId)"
        );
        assert_eq!(
            invocation.engine,
            EngineId::NativeCsvTsv,
            "§1.7: and the resolved EngineId for the pair"
        );
        assert!(
            matches!(
                invocation.plan.program,
                EngineProgram::InProcessNative(EngineId::NativeCsvTsv)
            ),
            "§1.7: the envelope wraps the §3.2.2 Invocation (no argv/cwd/env re-declaration)"
        );
        assert!(
            !invocation.cancel.is_cancelled(),
            "§0.4.4: a fresh cancel token starts un-cancelled"
        );
        invocation.cancel.cancel();
        assert!(
            invocation.cancel.is_cancelled(),
            "§0.4.4: tripping the token cancels the invocation (the C7 cancel_run path)"
        );
    }

    // §6.4.1 unit (G15): the §1.7 `InvocationResult` has the three terminal variants (P3.4); `Failed` carries
    // the Rust-internal §2.8 `ConversionErrorKind`.
    #[test]
    fn invocation_result_has_succeeded_failed_and_cancelled() {
        assert_eq!(InvocationResult::Succeeded, InvocationResult::Succeeded);
        assert_eq!(InvocationResult::Cancelled, InvocationResult::Cancelled);
        assert_eq!(
            InvocationResult::Failed(ConversionErrorKind::EngineCrash),
            InvocationResult::Failed(ConversionErrorKind::EngineCrash),
            "§1.7: Failed carries the §2.8 kind the §1.9 FSM maps to the wire ErrorKind at P3.46"
        );
        assert_ne!(
            InvocationResult::Failed(ConversionErrorKind::EngineCrash),
            InvocationResult::Failed(ConversionErrorKind::EngineHang),
            "§1.7: the carried kind is part of the Failed identity"
        );
        assert_ne!(InvocationResult::Succeeded, InvocationResult::Cancelled);
    }

    // §6.4.1 unit (G15): the §1.7 dispatch — the P3 walking-skeleton contract. The subprocess lanes stay
    // unwired (no subprocess engine is registered; P4.13 routes them through run_confined), so the exhaustive
    // `EngineProgram` match still returns the honest `Failed(InternalError)` seam (§2.13, P2.25) for them.
    // [Test-Change: P3.43 — old-obsolete+new-correct, §1.7] the InProcessNative case was REMOVED from this
    // seam test: P3.43 wires that arm to the real native CSV/TSV lane on crate::pool::run_in_core, so its old
    // "InternalError seam" expectation is obsolete (the arm now succeeds — asserted by the tests below); the
    // two subprocess arms keep the seam until P4.13. dispatch is now `async` + takes the pool + progress sink.
    #[tokio::test]
    async fn dispatch_returns_the_honest_internal_error_seam_for_the_unwired_subprocess_lanes() {
        let pool = Pool::with_degree(1);
        for program in [
            EngineProgram::Sidecar(EngineId::FFmpeg),
            EngineProgram::ResourceBin {
                engine: EngineId::LibreOffice,
                rel: PathBuf::from("program/soffice"),
            },
        ] {
            let invocation = engine_invocation(program);
            assert_eq!(
                dispatch(&invocation, &pool, |_| {}).await,
                InvocationResult::Failed(ConversionErrorKind::InternalError),
                "§1.7/§2.13: the unwired subprocess lanes return the honest InternalError seam (P4.13 wires them)"
            );
        }
    }

    // Build an InProcessNative `EngineInvocation` for the native CSV/TSV lane: `args = [source, target-token]`
    // (NativeCsvTsvEngine::plan's shape, §3.2.2) and a real publish `out_tmp` the transform writes to.
    fn native_lane_invocation(
        source: &Path,
        target_token: &str,
        out_tmp: TempPath,
    ) -> EngineInvocation {
        EngineInvocation {
            job: JobId::from_index(0),
            engine: EngineId::NativeCsvTsv,
            plan: Invocation {
                program: EngineProgram::InProcessNative(EngineId::NativeCsvTsv),
                args: vec![source.as_os_str().to_owned(), OsString::from(target_token)],
                cwd: None,
                env: Vec::new(),
                stdin: StdinPlan::None,
                progress: ProgressModel::InProcessFraction,
                out_tmp: Some(out_tmp),
            },
            cancel: CancellationToken::new(),
        }
    }

    // §6.4.1 unit (G15) + §0.1 real-FS: the P3.43 §1.7 InProcessNative lane runs the real §3.5.6 transform on
    // crate::pool::run_in_core, writes the TSV output to out_tmp, returns Succeeded, and forwards the
    // self-reported progress — here a single 1.0 completion tick for the sub-100-KB source (§1.7/§1.11).
    #[tokio::test]
    async fn dispatch_runs_the_native_lane_writes_the_output_and_forwards_the_completion_tick() {
        use std::sync::{Arc, Mutex};

        let dir = tempfile::tempdir().expect("temp dir");
        let source = dir.path().join("data.csv");
        std::fs::write(&source, b"a,b\n1,2\n").expect("write source");
        let out_temp = tempfile::Builder::new()
            .tempfile_in(dir.path())
            .expect("out temp")
            .into_temp_path();
        let out_path = out_temp.to_path_buf();
        let invocation = native_lane_invocation(&source, "tsv", out_temp);

        let ticks = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&ticks);
        let result = dispatch(&invocation, &Pool::with_degree(1), move |fraction| {
            sink.lock().expect("tick lock").push(fraction);
        })
        .await;

        assert_eq!(
            result,
            InvocationResult::Succeeded,
            "§1.7: the native CSV→TSV lane completes successfully"
        );
        assert_eq!(
            std::fs::read(&out_path).expect("read output"),
            b"a\tb\n1\t2\n",
            "§3.5.6: the transform wrote the TSV output to out_tmp"
        );
        assert_eq!(
            ticks.lock().expect("tick lock").as_slice(),
            &[1.0],
            "§1.7/§1.11: a sub-100-KB source emits a single 1.0 completion tick"
        );
    }

    // §6.4.1 unit (G15): a mis-wired InProcessNative plan (no out_tmp / an unknown target token) fails cleanly
    // as Failed(InternalError) — index-free, never a panic (the in-core no-panic path, G4/G14).
    #[tokio::test]
    async fn dispatch_fails_the_native_lane_cleanly_on_a_mis_wired_plan() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source = dir.path().join("data.csv");
        std::fs::write(&source, b"a,b\n1,2\n").expect("write source");

        // (a) no out_tmp on the encode invocation — a mis-wired §1.7 lifecycle.
        let mut no_out_tmp = native_lane_invocation(
            &source,
            "tsv",
            tempfile::Builder::new()
                .tempfile_in(dir.path())
                .expect("out temp")
                .into_temp_path(),
        );
        no_out_tmp.plan.out_tmp = None;
        assert_eq!(
            dispatch(&no_out_tmp, &Pool::with_degree(1), |_| {}).await,
            InvocationResult::Failed(ConversionErrorKind::InternalError),
            "§1.7: a native encode invocation with no out_tmp is a mis-wired lifecycle → InternalError"
        );

        // (b) an unknown target token — a mis-routed selection (CsvTsvTarget::from_token → None).
        let bad_token = native_lane_invocation(
            &source,
            "xlsx",
            tempfile::Builder::new()
                .tempfile_in(dir.path())
                .expect("out temp")
                .into_temp_path(),
        );
        assert_eq!(
            dispatch(&bad_token, &Pool::with_degree(1), |_| {}).await,
            InvocationResult::Failed(ConversionErrorKind::InternalError),
            "§1.7: a non-CSV/TSV target token is a mis-routed selection → InternalError"
        );

        // (c) empty argv — no source path at args[0] (a mis-built plan). Index-free (`first`) → InternalError.
        let mut no_source = native_lane_invocation(
            &source,
            "tsv",
            tempfile::Builder::new()
                .tempfile_in(dir.path())
                .expect("out temp")
                .into_temp_path(),
        );
        no_source.plan.args.clear();
        assert_eq!(
            dispatch(&no_source, &Pool::with_degree(1), |_| {}).await,
            InvocationResult::Failed(ConversionErrorKind::InternalError),
            "§1.7: a plan with no source arg is mis-built → InternalError (index-free, no panic)"
        );
    }

    // §6.4.1 unit (G15) + §0.1 real-FS: the native lane maps a real transform FAILURE to its §2.8 kind through
    // crate::pool::run_in_core — the run_in_core `Ok(Err(TransformError))` → `Failed(from)` arm, exercised
    // end-to-end (spawn → transform → classify). An ambiguous-delimiter single-column source (§2.10.2) →
    // Corrupt, with no partial output published and no panic.
    #[tokio::test]
    async fn dispatch_maps_a_native_transform_failure_to_its_conversion_kind() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source = dir.path().join("ambiguous.csv");
        std::fs::write(&source, b"alpha\nbeta\ngamma\n").expect("write source");
        let invocation = native_lane_invocation(
            &source,
            "tsv",
            tempfile::Builder::new()
                .tempfile_in(dir.path())
                .expect("out temp")
                .into_temp_path(),
        );
        assert_eq!(
            dispatch(&invocation, &Pool::with_degree(1), |_| {}).await,
            InvocationResult::Failed(ConversionErrorKind::Corrupt),
            "§1.7/§2.8: an ambiguous-delimiter source fails the transform → Failed(Corrupt), never a panic"
        );
    }

    // §6.4.1 unit (G15) + §0.1 real-FS: the P3.44 §1.7 cooperative cancel through the dispatch lane. A
    // PRE-cancelled token stops the native transform at the first chunk boundary → InvocationResult::Cancelled
    // — the "cleanly discards the one in progress" guarantee reached cooperatively (no kill step, §1.7).
    #[tokio::test]
    async fn dispatch_cancels_the_native_lane_cooperatively() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source = dir.path().join("big.csv");
        // A multi-chunk source so the transform reaches a chunk boundary (where the cancel poll fires).
        let mut bytes = Vec::new();
        while bytes.len() < PROGRESS_CHUNK_BYTES * 3 {
            bytes.extend_from_slice(b"a,b,c\n");
        }
        std::fs::write(&source, &bytes).expect("write source");
        let invocation = native_lane_invocation(
            &source,
            "tsv",
            tempfile::Builder::new()
                .tempfile_in(dir.path())
                .expect("out temp")
                .into_temp_path(),
        );
        // Cancel BEFORE dispatch: the token is already tripped, so the first chunk-boundary poll observes it.
        invocation.cancel.cancel();
        assert_eq!(
            dispatch(&invocation, &Pool::with_degree(1), |_| {}).await,
            InvocationResult::Cancelled,
            "§1.7: a cancelled token stops the native lane cooperatively → Cancelled"
        );
    }

    // §6.4.1 unit (G15) + §0.1 real-FS: the P3.44 §2.1 "no partial leftover" guarantee END-TO-END. A cancelled
    // native lane writes a partial out_tmp, but the §2.1 atomic publish NEVER runs on the cancel path, so
    // dropping the un-consumed invocation (which owns the `TempPath`) deletes the partial `.part` temp (§3.2.2)
    // — the file at the output path never survives. (The pre-dispatch token check + the batch-level end-to-end
    // no-leftover assertion are the P3.46 conductor's, which owns the invocation lifecycle.)
    #[tokio::test]
    async fn a_cancelled_native_lane_leaves_no_partial_output_after_the_invocation_drops() {
        let dir = tempfile::tempdir().expect("temp dir");
        let source = dir.path().join("big.csv");
        let mut bytes = Vec::new();
        while bytes.len() < PROGRESS_CHUNK_BYTES * 3 {
            bytes.extend_from_slice(b"a,b,c\n");
        }
        std::fs::write(&source, &bytes).expect("write source");
        let out_temp = tempfile::Builder::new()
            .tempfile_in(dir.path())
            .expect("out temp")
            .into_temp_path();
        let out_path = out_temp.to_path_buf();
        let invocation = native_lane_invocation(&source, "tsv", out_temp);
        invocation.cancel.cancel();

        let result = dispatch(&invocation, &Pool::with_degree(1), |_| {}).await;
        assert_eq!(
            result,
            InvocationResult::Cancelled,
            "the cancelled lane returns Cancelled"
        );
        // The partial temp is still present while the invocation (holding the TempPath) is alive — the §2.1
        // publish did NOT run, so nothing was promoted to a final path.
        assert!(
            out_path.exists(),
            "the un-published partial .part temp exists until the owning invocation drops"
        );
        // Dropping the un-consumed invocation drops the TempPath → the partial temp is deleted (§3.2.2/§2.1).
        drop(invocation);
        assert!(
            !out_path.exists(),
            "§2.1: dropping the un-published invocation deletes the partial temp — no leftover survives"
        );
    }

    // §6.4.2 bound-firing (G16): the §0.9 TIMEOUT-SENTINEL over the REAL transform + the REAL §0.9 lane —
    // `tests/corpus/expansion_sentinel.csv` (P3.61). §0.9:1633 asks for "a deterministic input / a
    // `#[cfg(test)]` sidecar that reliably exceeds the budget or stalls without progress" so the §1.7 reap is
    // "test-covered, not prose"; `NATIVE_CSV_TSV_TIMEOUT`'s own doc names P3.61 as this sentinel's author.
    //
    // WHY A FIXTURE AND NOT ANOTHER `pending()` LANE: the sibling below already covers the mapping over a
    // synthetic lane. This one proves the reap over the code a real file actually drives — `csv_tsv_transform`
    // reading real bytes, on `Pool::run_in_core`, under `bounded_lane` — composed exactly as
    // `run_native_csv_tsv` composes it.
    //
    // WHY IT IS DETERMINISTIC (no stopwatch, no margin): the 120s production bound can never fire on SIZE — the
    // transform is a linear whole-file-buffered re-encode, so its real trigger is a stall, not a big file
    // (`NATIVE_CSV_TSV_TIMEOUT`'s doc says exactly this). So the sentinel STRUCTURALLY stalls: `should_cancel`
    // blocks on a channel whose sender this test holds, so the lane CANNOT complete inside any bound — the
    // `pending()` determinism argument, applied to the real transform. Dropping the sender at test end unblocks
    // the parked worker, so the abandoned thread exits rather than leaking.
    //
    // WHY THE FIXTURE'S SIZE IS LOAD-BEARING: `transform_bytes` gates `should_cancel` behind
    // `report_chunks = text_len >= PROGRESS_CHUNK_BYTES`, so a sub-100-KiB source is never polled and has NO
    // stall point at all. The sentinel is sized past that gate (106510 B, ASCII ⇒ decoded len == byte len; the
    // boundary is first reached by record 3200 at position 102414, with 128 records to spare). The control leg
    // below pins that this size is what arms it: the SAME stall closure over the 58-byte canonical fixture is
    // never polled, so that lane completes — a fixture-inertness tripwire that goes red the moment a shrunk
    // sentinel (or a raised PROGRESS_CHUNK_BYTES) stops crossing the gate.
    //
    // Stated precisely (the G1 opus P3): the ASSERTION is bound-independent — the parked lane can never
    // complete, so `EngineHang` is the only reachable outcome. What the pair of legs cannot separate is
    // ATTRIBUTION on a pathologically slow runner: `spawn_blocking` scheduling alone could outrun the 50 ms
    // bound, so a hypothetically-inert sentinel might still report EngineHang for an unrelated reason. The
    // control leg closes that gap from the other side (an inert fixture completes), which is why both legs
    // exist rather than one.
    #[tokio::test]
    async fn the_timeout_sentinel_fixture_stalls_at_its_chunk_boundary_and_the_wall_clock_bound_reaps_it(
    ) {
        let pool = Pool::with_degree(1);
        let out_dir = tempfile::tempdir().expect("a temp dir for the sentinel's out_tmp");
        let out_path = out_dir.path().join("sentinel.tsv");
        let source = crate::test_corpus::fixture("expansion_sentinel.csv");

        // The §1.7 job token: the run. `bounded_lane` trips only its CHILD on expiry — the run must survive.
        let job = CancellationToken::new();
        let deadline_token = job.child_token();

        // The structural stall: `recv()` parks until this test drops `stall_tx`. Held across the whole await,
        // so the lane cannot finish inside the bound no matter how fast the machine is.
        let (stall_tx, stall_rx) = std::sync::mpsc::channel::<()>();
        let lane = pool.run_in_core(move || -> Result<TransformStatus, TransformError> {
            let out_file = std::fs::File::create(&out_path).map_err(TransformError::Write)?;
            let mut report = |_fraction: f32| {};
            let mut should_cancel = || {
                // Parks at the fixture's first chunk boundary — which its size guarantees is reached.
                let _ = stall_rx.recv();
                false
            };
            csv_tsv_transform(
                &source,
                CsvTsvTarget::Tsv,
                out_file,
                &mut report,
                &mut should_cancel,
            )
        });
        let forwarder = tokio::spawn(async {});

        let result = bounded_lane(lane, forwarder, deadline_token, Duration::from_millis(50)).await;

        assert_eq!(
            result,
            InvocationResult::Failed(ConversionErrorKind::EngineHang),
            "§0.9/§1.7: the sentinel stalls without progress, so the wall-clock bound reaps it to EngineHang"
        );
        assert!(
            !job.is_cancelled(),
            "§1.7: the reap trips only the CHILD deadline token — the RUN continues (P3.45)"
        );
        drop(stall_tx); // unpark the abandoned worker so it exits instead of leaking
    }

    // The sentinel's fixture-inertness tripwire (G16): the SAME structural stall over a sub-chunk fixture is
    // never polled, so the lane completes well inside the bound. This is what proves the sentinel's SIZE is the
    // thing arming it — without this leg, a future shrink of the sentinel (or a raise of PROGRESS_CHUNK_BYTES)
    // would silently turn it into a file that is loaded and then ignored.
    #[tokio::test]
    async fn a_sub_chunk_fixture_is_never_polled_so_the_same_stall_cannot_arm_the_bound() {
        let pool = Pool::with_degree(1);
        let out_dir = tempfile::tempdir().expect("a temp dir for the control's out_tmp");
        let out_path = out_dir.path().join("control.tsv");
        let source = crate::test_corpus::fixture("canonical.csv");

        let job = CancellationToken::new();
        let deadline_token = job.child_token();

        let (stall_tx, stall_rx) = std::sync::mpsc::channel::<()>();
        let lane = pool.run_in_core(move || -> Result<TransformStatus, TransformError> {
            let out_file = std::fs::File::create(&out_path).map_err(TransformError::Write)?;
            let mut report = |_fraction: f32| {};
            let mut should_cancel = || {
                let _ = stall_rx.recv();
                false
            };
            csv_tsv_transform(
                &source,
                CsvTsvTarget::Tsv,
                out_file,
                &mut report,
                &mut should_cancel,
            )
        });
        let forwarder = tokio::spawn(async {});

        // A generous bound: the point is that the lane finishes, not that it races.
        let result = bounded_lane(lane, forwarder, deadline_token, Duration::from_secs(30)).await;

        assert_eq!(
            result,
            InvocationResult::Succeeded,
            "a sub-PROGRESS_CHUNK_BYTES source crosses no boundary, so `should_cancel` never runs and the \
             identical stall closure cannot arm the bound — the sentinel's SIZE is what makes it a sentinel"
        );
        drop(stall_tx);
    }

    // §6.4.1 unit (G15): the P3.45 §1.7 wall-clock TIMEOUT arm of `bounded_lane`. A never-completing lane (the
    // wedged-uninterruptible-read model, §2.12.4) cannot resolve, so the wall-clock bound alone decides the
    // outcome → Failed(EngineHang) — the run continuing — and the timeout TRIPS the cooperative-cancel poll
    // (the child deadline token) so a non-wedged abandoned thread would bail at its next boundary. Deterministic:
    // a `pending()` lane can never win the race, so the short real bound always elapses (no flake).
    #[tokio::test]
    async fn bounded_lane_abandons_a_wedged_lane_to_engine_hang_and_trips_the_cooperative_poll() {
        let deadline_token = CancellationToken::new();
        let forwarder = tokio::spawn(async {});
        let wedged = std::future::pending::<LaneOutcome>();
        let result = bounded_lane(
            wedged,
            forwarder,
            deadline_token.clone(),
            Duration::from_millis(50),
        )
        .await;
        assert_eq!(
            result,
            InvocationResult::Failed(ConversionErrorKind::EngineHang),
            "§1.7: a lane that outruns its wall-clock bound is abandoned → Failed(EngineHang), the run continuing"
        );
        assert!(
            deadline_token.is_cancelled(),
            "§1.7: the wall-clock timeout trips the cooperative-cancel poll (the child deadline token)"
        );
    }

    // §6.4.1 unit (G15): the P3.45 WITHIN-bound arm of `bounded_lane` maps every terminal lane outcome and
    // leaves the cooperative poll UN-tripped (no timeout fired) — Completed→Succeeded, the cooperative
    // Cancelled→Cancelled (P3.44), a §3.5.6 TransformError→its §2.8 kind, and a §0.9 LaneError→InternalError
    // (one item's failure, never a pool-wide fault). A `ready(..)` lane resolves before the generous bound.
    #[tokio::test]
    async fn bounded_lane_maps_each_within_bound_outcome_without_tripping_the_poll() {
        let generous = Duration::from_secs(30);
        let cases: Vec<(LaneOutcome, InvocationResult)> = vec![
            (
                Ok(Ok(TransformStatus::Completed)),
                InvocationResult::Succeeded,
            ),
            (
                Ok(Ok(TransformStatus::Cancelled)),
                InvocationResult::Cancelled,
            ),
            (
                Ok(Err(TransformError::AmbiguousDelimiter)),
                InvocationResult::Failed(ConversionErrorKind::Corrupt),
            ),
            (
                Err(LaneError::Panicked),
                InvocationResult::Failed(ConversionErrorKind::InternalError),
            ),
            (
                Err(LaneError::PoolClosed),
                InvocationResult::Failed(ConversionErrorKind::InternalError),
            ),
        ];
        for (outcome, want) in cases.into_iter() {
            let deadline_token = CancellationToken::new();
            let forwarder = tokio::spawn(async {});
            let got = bounded_lane(
                std::future::ready(outcome),
                forwarder,
                deadline_token.clone(),
                generous,
            )
            .await;
            assert_eq!(
                got, want,
                "§1.7: the within-bound lane outcome maps to its InvocationResult"
            );
            assert!(
                !deadline_token.is_cancelled(),
                "§1.7: no wall-clock timeout fired, so the cooperative poll is not tripped"
            );
        }
    }

    // §6.4.1 unit (G15): the P3.45 bounded-pool-headroom leg (the Decision note ② §1.7 AND/OR first leg). A real
    // `run_in_core` lane whose worker BLOCKS forever (the wedged-uninterruptible-read model) is abandoned at the
    // wall-clock deadline; because `run_in_core` frees its §0.9 permit ON DROP (P3.3), the detached worker parks
    // in the pool's headroom holding NO permit — so a fresh lane on the SAME degree-1 pool still runs (the run
    // CONTINUES, the pool is not starved). Deterministic: the worker never completes, so the short real bound
    // alone fires the timeout; a std channel the test controls releases the parked worker at teardown.
    #[tokio::test]
    async fn a_timed_out_lane_frees_its_permit_so_the_pool_is_not_starved() {
        let pool = Pool::with_degree(1);
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        // A lane whose worker blocks on a never-sent channel until teardown (models a wedged read holding the
        // single degree-1 permit's worker slot).
        let wedged = pool.run_in_core(move || {
            let _ = release_rx.recv();
        });
        let timed_out = tokio::time::timeout(Duration::from_millis(50), wedged).await;
        assert!(
            timed_out.is_err(),
            "§1.7: the wedged lane outruns the wall-clock bound and is abandoned"
        );
        // The permit was freed on drop despite the still-parked worker (bounded-pool-headroom): a fresh lane
        // acquires the single degree-1 permit and runs to completion. The fresh lane is itself wall-clock-bounded
        // so a permit-free-on-drop REGRESSION fails FAST (a clean red) instead of hanging CI forever on a lane
        // that can never acquire the starved permit — the generous bound never bites on the passing path (the
        // trivial closure finishes in microseconds).
        let recovered = tokio::time::timeout(Duration::from_secs(30), pool.run_in_core(|| 7_u32))
            .await
            .expect("§1.7/§0.9: a fresh lane must acquire the freed permit within the bound — a permit-free regression would otherwise hang here")
            .expect("§1.7/§0.9: the abandoned lane freed its permit — the pool is not starved, the run continues");
        assert_eq!(recovered, 7);
        // Release the parked worker so it exits cleanly at teardown (no blocked thread leaks beyond the test).
        drop(release_tx);
    }

    // ─── P4.2: §3.2.2 `ProbeOutput` — the §3.2.1 two-phase probe result ──

    // §6.4.1 unit (G15): the §3.2.2 `ProbeOutput` holds the four parsed probe fields (P4.2) — the typed
    // result §1.7 parses from ffprobe stdout and hands to `plan_encode` (P4.1). Reads every field so the
    // test build is dead-code-clean; `duration_us` is the §1.11 FfmpegKeyValue denominator PROVIDED here,
    // never mutated onto a pre-probe struct (§3.2.1).
    #[test]
    fn probe_output_holds_the_four_probe_fields() {
        let probe = ProbeOutput {
            duration_us: 90_000_000,
            inner_codecs: vec!["h264".to_owned(), "aac".to_owned()],
            rotation_deg: Some(90),
            interlaced: Some(false),
        };
        assert_eq!(
            probe.duration_us, 90_000_000,
            "§3.2.2: duration_us carries the probed media duration — the §1.11 progress denominator"
        );
        assert_eq!(
            probe.inner_codecs,
            vec!["h264".to_owned(), "aac".to_owned()],
            "§3.2.2: inner_codecs carries the stream codecs for the remux-vs-reencode decision"
        );
        assert_eq!(probe.rotation_deg, Some(90));
        assert_eq!(probe.interlaced, Some(false));
    }

    // §6.4.1 unit (G15): the two optional probe facts are honestly absent (`None`) when the probed streams
    // carry no rotation/interlace flag (§3.2.2) — the minimal shape a flag-less source produces; distinct
    // from the flagged shape via the derived `PartialEq` (which reads all four fields).
    #[test]
    fn probe_output_optional_fields_model_unflagged_streams() {
        let unflagged = ProbeOutput {
            duration_us: 1,
            inner_codecs: vec!["pcm_s16le".to_owned()],
            rotation_deg: None,
            interlaced: None,
        };
        assert_eq!(unflagged.rotation_deg, None);
        assert_eq!(unflagged.interlaced, None);
        assert_ne!(
            unflagged,
            ProbeOutput {
                rotation_deg: Some(0),
                ..unflagged.clone()
            },
            "§3.2.2: an absent rotation flag (None) is distinct from an explicit 0° rotation"
        );
    }

    // ─── P3.5: the §3.2 Engine trait (minimal) + the native CSV/TSV engine's plan() ──

    // A minimal eligible CSV `DroppedItem` for the native-engine plan() tests. plan() ignores `item` (the
    // source delimiter is detected at RUNTIME by the transform, not planned), so any well-formed item serves.
    fn csv_dropped_item() -> DroppedItem {
        DroppedItem {
            item: ItemId::from_index(0),
            display_name: "data.csv".to_string(),
            rel_path_display: None,
            size_bytes: 12,
            detected: DetectionOutcome::Recognized {
                format: FormatId::Csv,
                confidence: Confidence::High,
                dims: None,
            },
        }
    }

    // A throwaway publish-temp for the plan() tests. plan() ignores `out_tmp` (the native engine reads the temp
    // §1.7 populates onto `Invocation.out_tmp`, not its argv), so any live TempPath serves; it is deleted on
    // drop at the end of the test. Rooted in the system temp dir here (a test-only convenience — production
    // picks it in the destination volume, §2.14.4).
    fn throwaway_temp_path() -> TempPath {
        tempfile::NamedTempFile::new()
            .expect("create a temp file for the plan() test")
            .into_temp_path()
    }

    // §6.4.1 unit (G15): the P3.5 native CSV/TSV `Engine::plan()` — Pure, maps a Tsv target to a single-step
    // encode Invocation carrying the InProcessNative program, self-reported InProcessFraction progress, no cwd/
    // env/stdin (an in-core engine spawns nothing), out_tmp None (§1.7 populates at spawn time), and args
    // [input, "tsv"] (the §3.5.6 transform's two runtime params). A Pure, no-I/O logic test (test-strategy §10.1).
    #[test]
    fn native_engine_plans_a_tsv_target_as_a_single_step_encode() {
        let engine = NativeCsvTsvEngine;
        let item = csv_dropped_item();
        let temp = throwaway_temp_path();
        let input = Path::new("/data/report.csv");

        let outcome = engine
            .plan(&item, TargetId::Format(FormatId::Tsv), input, &temp)
            .expect("native CSV/TSV plan() succeeds for a TSV target");
        let inv = match outcome {
            PlanOutcome::Encode(inv) => inv,
            // unreachable-by-construction: the single-step native engine plans a single encode and never a
            // probe (§3.2.1) — reaching this arm is a real bug. Allowed in #[cfg(test)] (CLAUDE.md anti-patterns).
            // [Build-Session-Entscheidung: P3.5]
            PlanOutcome::Probe(_) => {
                unreachable!(
                    "§3.2.2: the single-step native CSV/TSV engine returns Encode, never Probe"
                )
            }
        };

        assert!(
            matches!(
                inv.program,
                EngineProgram::InProcessNative(EngineId::NativeCsvTsv)
            ),
            "§3.5.6: the native engine's program is InProcessNative(NativeCsvTsv)"
        );
        assert_eq!(
            inv.progress,
            ProgressModel::InProcessFraction,
            "§3.2.2/§3.5.6: it self-reports a bytes_processed/source_size fraction"
        );
        assert!(
            inv.out_tmp.is_none(),
            "§3.2.2: plan() constructs out_tmp None; §1.7 populates Some(temp) at spawn time"
        );
        assert_eq!(
            inv.stdin,
            StdinPlan::None,
            "§3.5.6: the native engine reads the input path, never stdin"
        );
        assert_eq!(
            inv.cwd, None,
            "§3.5.6: an in-core engine spawns no subprocess, so it needs no working directory"
        );
        assert!(
            inv.env.is_empty(),
            "§3.5.6: an in-core engine spawns no subprocess, so it carries no env"
        );
        assert_eq!(
            inv.args,
            vec![OsString::from("/data/report.csv"), OsString::from("tsv")],
            "§3.2.2/§3.5.6: args carry the embedded input path + the target format token"
        );
    }

    // §6.4.1 unit (G15): the P3.5 native `plan()` maps a Csv target to the args token "csv", and REJECTS any
    // non-CSV/TSV target with an InternalError PlanError — a mis-routed §3.2.3 selection (the registry never
    // sends a non-CSV/TSV pair here), a bug rather than a user fault.
    #[test]
    fn native_engine_plans_csv_and_rejects_a_foreign_target() {
        let engine = NativeCsvTsvEngine;
        let item = csv_dropped_item();
        let temp = throwaway_temp_path();
        let input = Path::new("/data/report.tsv");

        let outcome = engine
            .plan(&item, TargetId::Format(FormatId::Csv), input, &temp)
            .expect("native plan() succeeds for a CSV target");
        let inv = match outcome {
            PlanOutcome::Encode(inv) => inv,
            // unreachable-by-construction (see the TSV test); allowed in #[cfg(test)].
            // [Build-Session-Entscheidung: P3.5]
            PlanOutcome::Probe(_) => {
                unreachable!(
                    "§3.2.2: the single-step native CSV/TSV engine returns Encode, never Probe"
                )
            }
        };
        assert_eq!(
            inv.args,
            vec![OsString::from("/data/report.tsv"), OsString::from("csv")],
            "§3.5.6: a CSV target sets the format token to \"csv\""
        );

        // A foreign target (an image format) is a mis-routed selection → an InternalError PlanError. `.err()`
        // extracts the error without requiring PlanOutcome to be PartialEq (it wraps a live TempPath).
        let rejected = engine.plan(&item, TargetId::Format(FormatId::Webp), input, &temp);
        assert_eq!(
            rejected.err(),
            Some(PlanError {
                kind: ConversionErrorKind::InternalError,
                detail: "native CSV/TSV engine planned for a non-CSV/TSV target".to_owned(),
            }),
            "§3.2.2: a non-CSV/TSV target yields an InternalError PlanError, not a wrong Invocation"
        );
    }

    // §6.4.1 unit (G15): the P3.5 `PlanOutcome` names both plan shapes — Encode (single-step) and Probe (the
    // §3.2.1 ffprobe sub-invocation). Constructing + reading both keeps the test build dead-code-clean; no P3
    // engine returns Probe, so it is dead in the production build (the module-level dead-code expectation
    // covers it).
    #[test]
    fn plan_outcome_names_the_encode_and_probe_shapes() {
        let shapes = [
            PlanOutcome::Encode(native_csv_invocation()),
            PlanOutcome::Probe(native_csv_invocation()),
        ];
        for shape in shapes {
            // Both variants wrap the plan Invocation; read its program via an or-pattern (exhaustive, no
            // wildcard) so both variants and the wrapped field are exercised.
            let program = match shape {
                PlanOutcome::Encode(inv) | PlanOutcome::Probe(inv) => inv.program,
            };
            assert!(
                matches!(
                    program,
                    EngineProgram::InProcessNative(EngineId::NativeCsvTsv)
                ),
                "§3.2.2: both PlanOutcome shapes wrap the plan Invocation"
            );
        }
    }
}

#[cfg(test)]
mod transform_tests {
    //! §6.4.1 unit (G15) for the P3.41 §3.5.6 native CSV/TSV streamed transform. Exercises `transform_bytes`
    //! (the byte->byte core) over crafted inputs + `csv_tsv_transform` over a real temp file. Pins: both
    //! directions (CSV<->TSV); RFC-4180 re-quoting when a field contains the NEW delimiter / a quote / a
    //! newline; CSV-injection literal preservation (leading `= + - @` unchanged); non-UTF-8 -> UTF-8
    //! transcode; BOM stripping; the §2.10.2 fail-clearly on invalid bytes; an ambiguous delimiter -> error;
    //! LF output; determinism; and the `from_token` / error-mapping contracts. (The output-VALIDITY corpus
    //! bar G31/G32 binds these to real reader-read-back at P3.61-P3.63.)
    use super::*;

    /// Run `transform_bytes` and return the produced output bytes (the common test shape). Progress is
    /// discarded here (the content assertions do not depend on it) — the P3.43 progress contract is asserted by
    /// its own `transform_reports_*` tests below via [`transform_collecting_ticks`].
    fn transform(bytes: &[u8], target: CsvTsvTarget) -> Result<Vec<u8>, TransformError> {
        let mut out = Vec::new();
        let status = transform_bytes(bytes, target, &mut out, &mut |_| {}, &mut || false)?;
        assert_eq!(
            status,
            TransformStatus::Completed,
            "the never-cancelling transform runs to completion"
        );
        Ok(out)
    }

    /// Run `transform_bytes`, returning both the output bytes AND the ordered progress fractions `on_progress`
    /// received — the P3.43 self-reported-progress test shape (never cancels).
    fn transform_collecting_ticks(
        bytes: &[u8],
        target: CsvTsvTarget,
    ) -> Result<(Vec<u8>, Vec<f32>), TransformError> {
        let mut out = Vec::new();
        let mut ticks = Vec::new();
        let status = transform_bytes(
            bytes,
            target,
            &mut out,
            &mut |fraction| ticks.push(fraction),
            &mut || false,
        )?;
        assert_eq!(
            status,
            TransformStatus::Completed,
            "the never-cancelling transform runs to completion"
        );
        Ok((out, ticks))
    }

    #[test]
    fn csv_to_tsv_swaps_the_delimiter() {
        let out = transform(b"a,b,c\n1,2,3\n", CsvTsvTarget::Tsv).expect("valid CSV transforms");
        assert_eq!(
            out, b"a\tb\tc\n1\t2\t3\n",
            "§3.5.6: comma source -> tab-delimited output, LF terminator"
        );
    }

    // §3.5.6 / spreadsheets.md "CSV → TSV not lossy" edge cases (P3.75 sweep): a GENUINELY-empty line (zero
    // bytes between two terminators — no fields at all) is NOT an RFC-4180 record and is DROPPED, exactly as
    // every mainstream CSV reader does; but a line with ANY content — here a whitespace-only field — IS a
    // record and is preserved. Pins the disclosed blank-line normalisation so it is reviewed, not accidental.
    #[test]
    fn a_genuinely_empty_line_is_dropped_but_a_content_line_is_kept() {
        let dropped = transform(b"a,b\n\nc,d\n", CsvTsvTarget::Tsv).expect("transforms");
        assert_eq!(
            dropped, b"a\tb\nc\td\n",
            "§3.5.6: a zero-field blank line is not a record and is dropped (universal CSV convention)"
        );
        let kept = transform(b"a,b\n \nc,d\n", CsvTsvTarget::Tsv).expect("transforms");
        assert_eq!(
            kept, b"a\tb\n \nc\td\n",
            "§3.5.6: a whitespace-only line has content → it IS a one-field record and is preserved"
        );
    }

    #[test]
    fn tsv_to_csv_swaps_the_delimiter() {
        let out =
            transform(b"a\tb\tc\n1\t2\t3\n", CsvTsvTarget::Csv).expect("valid TSV transforms");
        assert_eq!(
            out, b"a,b,c\n1,2,3\n",
            "§3.5.6: tab source -> comma-delimited output"
        );
    }

    #[test]
    fn a_field_containing_the_new_delimiter_is_rfc4180_requoted() {
        // A comma-CSV field `b\tc` contains a TAB; converting to TSV the tab is the NEW delimiter, so the field
        // must be RFC-4180 quoted to stay one field.
        let out = transform(b"h1,h2,h3\na,b\tc,d\n", CsvTsvTarget::Tsv).expect("transforms");
        assert_eq!(
            out, b"h1\th2\th3\na\t\"b\tc\"\td\n",
            "§3.5.6: a field containing the NEW (tab) delimiter is re-quoted"
        );
    }

    #[test]
    fn a_field_with_an_embedded_quote_is_requoted_and_doubled() {
        let out = transform(b"col1,col2\n\"a\"\"b\",c\n", CsvTsvTarget::Tsv).expect("transforms");
        assert_eq!(
            out, b"col1\tcol2\n\"a\"\"b\"\tc\n",
            "§3.5.6: a field with an embedded quote is re-quoted, the quote doubled"
        );
    }

    #[test]
    fn a_field_with_an_embedded_newline_is_requoted() {
        let out = transform(b"col1,col2\n\"p\nq\",z\n", CsvTsvTarget::Tsv).expect("transforms");
        assert_eq!(
            out, b"col1\tcol2\n\"p\nq\"\tz\n",
            "§3.5.6: a field with an embedded newline is re-quoted"
        );
    }

    #[test]
    fn a_plain_field_is_never_quoted() {
        let out = transform(b"a,bcd,e\n1,2,3\n", CsvTsvTarget::Tsv).expect("transforms");
        assert_eq!(
            out, b"a\tbcd\te\n1\t2\t3\n",
            "§3.5.6: a plain field (no delimiter/quote/newline) is written bare (QuoteStyle::Necessary)"
        );
    }

    #[test]
    fn leading_formula_chars_are_preserved_literally() {
        // §3.5.6 CSV-injection-safe: a leading `= + - @` field stays LITERAL text — the transform never
        // prefixes or mangles it, and (having no delimiter/quote/newline) it is written bare, its value
        // byte-for-byte. The G32 output-validity reader binds this literal-preservation at P3.42.
        let out = transform(b"=1+1,+2,-3,@cmd\nx,y,z,w\n", CsvTsvTarget::Tsv).expect("transforms");
        assert_eq!(
            out, b"=1+1\t+2\t-3\t@cmd\nx\ty\tz\tw\n",
            "§3.5.6: leading = + - @ stay literal (never re-interpreted / prefixed)"
        );
    }

    #[test]
    fn non_utf8_source_is_transcoded_to_utf8() {
        // A Windows-1252 source (0xE9 = e-acute) -> detected as a single-byte codepage (not valid UTF-8) ->
        // decoded -> UTF-8 output (e-acute = 0xC3 0xA9), §2.10.2.
        let out = transform(b"nom,ville\ncaf\xE9,paris\n", CsvTsvTarget::Tsv).expect("transcodes");
        assert_eq!(
            out,
            "nom\tville\ncafé\tparis\n".as_bytes(),
            "§2.10.2: a Windows-1252 source is transcoded to UTF-8"
        );
    }

    #[test]
    fn a_utf8_bom_is_stripped() {
        // A UTF-8 BOM (EF BB BF) is authoritative for encoding + stripped from the output (§2.10.2 no-BOM).
        let out = transform(b"\xEF\xBB\xBFa,b\n1,2\n", CsvTsvTarget::Tsv).expect("transforms");
        assert_eq!(
            out, b"a\tb\n1\t2\n",
            "§2.10.2: the UTF-8 BOM is stripped (output UTF-8, no BOM)"
        );
    }

    #[test]
    fn invalid_bytes_fail_clearly_never_mojibake() {
        // A source whose header (first MAX_HEADER_WINDOW bytes) is valid UTF-8 CSV but whose BODY carries an
        // invalid UTF-8 byte (0xFF): detected UTF-8 from the header, then `decode` flags had_errors ->
        // Malformed (§2.10.2 "fail clearly, never emit mojibake") — NOT a silent U+FFFD replacement.
        let mut bytes = b"a,b\n".repeat(MAX_HEADER_WINDOW / 4); // >= MAX_HEADER_WINDOW valid UTF-8
        bytes.extend_from_slice(b"x,\xFF\n"); // invalid UTF-8 in the body
        let err = transform(&bytes, CsvTsvTarget::Tsv).expect_err("invalid UTF-8 fails");
        assert!(
            matches!(err, TransformError::Malformed),
            "§2.10.2: invalid bytes -> Malformed, never mojibake"
        );
    }

    #[test]
    fn an_ambiguous_delimiter_fails() {
        // A single-column source with no consistent multi-field delimiter -> classify_delimiter Ambiguous ->
        // the transform declines (it cannot re-quote a structure it cannot parse). Such a file is Uncertain at
        // intake and never routed here; the transform guards defensively.
        let err =
            transform(b"alpha\nbeta\ngamma\n", CsvTsvTarget::Tsv).expect_err("ambiguous fails");
        assert!(
            matches!(err, TransformError::AmbiguousDelimiter),
            "an undetectable delimiter -> AmbiguousDelimiter"
        );
    }

    #[test]
    fn output_uses_lf_not_crlf() {
        let out = transform(b"a,b\n1,2\n", CsvTsvTarget::Tsv).expect("transforms");
        assert!(
            !out.contains(&b'\r'),
            "[P3.41]: the output line terminator is LF, never the RFC-4180 CRLF"
        );
    }

    #[test]
    fn the_transform_is_deterministic() {
        let input = b"a,b\tc,d\n\"x\"\"y\",1,2\n";
        let first = transform(input, CsvTsvTarget::Tsv).expect("transforms");
        let second = transform(input, CsvTsvTarget::Tsv).expect("transforms");
        assert_eq!(
            first, second,
            "§3.5.6 / P3.61: the transform is deterministic (sha256(out1) == sha256(out2))"
        );
    }

    #[test]
    fn csv_tsv_transform_reads_a_real_file() {
        // The path wrapper over a real temp file (real-FS, test-strategy §0.1) — the same core, read from disk.
        let dir = tempfile::tempdir().expect("temp dir");
        let src = dir.path().join("data.csv");
        std::fs::write(&src, b"a,b\n1,2\n").expect("write source");
        let mut out = Vec::new();
        // [Test-Change: P3.43 — old-obsolete+new-correct, §1.7] on_progress (P3.43) + should_cancel (P3.44) args added: the old call form is obsolete, the new call correct, success check unchanged; fmt wrapped the call so G70 --diff over-reads the old line — no expectation relaxed, no regression hidden.
        csv_tsv_transform(&src, CsvTsvTarget::Tsv, &mut out, &mut |_| {}, &mut || {
            false
        })
        .expect("transforms a real file");
        assert_eq!(
            out, b"a\tb\n1\t2\n",
            "§3.5.6: the path wrapper reads + transforms a real source file"
        );
    }

    // §6.4.1 unit (G15): the P3.43 §1.7/§1.11 self-reported progress. A source below one PROGRESS_CHUNK_BYTES
    // chunk crosses no boundary, so the transform emits ONLY the final 1.0 completion tick (§1.7 "sub-100-KB
    // input → single tick", wire-indistinguishable from CoarseSpawnDone).
    #[test]
    fn transform_reports_only_a_final_completion_tick_for_a_sub_chunk_source() {
        let (_out, ticks) =
            transform_collecting_ticks(b"a,b\n1,2\n", CsvTsvTarget::Tsv).expect("transforms");
        assert_eq!(
            ticks.as_slice(),
            &[1.0],
            "§1.7/§1.11: a sub-chunk source emits ONLY the final 1.0 tick"
        );
    }

    // §6.4.1 unit (G15): the P3.43 progress fraction basis. A source spanning several PROGRESS_CHUNK_BYTES
    // chunks emits intermediate `bytes_processed / source_size` ticks — monotonically non-decreasing, each in
    // [0,1) — followed by the sole final 1.0 (§1.7/§1.11).
    #[test]
    fn transform_reports_monotonic_fractions_ending_in_one_for_a_multi_chunk_source() {
        // A CSV a few chunks wide, so the reader crosses several boundaries. Every row is well-formed so the
        // record pass never short-circuits.
        let mut source = Vec::new();
        let mut row = 0u32;
        while source.len() < PROGRESS_CHUNK_BYTES * 3 {
            source.extend_from_slice(format!("{row},value-{row},{row}\n").as_bytes());
            row = row.wrapping_add(1);
        }
        let (out, ticks) =
            transform_collecting_ticks(&source, CsvTsvTarget::Tsv).expect("transforms");
        assert!(!out.is_empty(), "the multi-chunk source produced output");
        assert!(
            ticks.len() >= 2,
            "§1.11: a multi-chunk source emits intermediate ticks plus the final 1.0: {ticks:?}"
        );
        assert_eq!(
            ticks.last().copied(),
            Some(1.0),
            "§1.11: the final tick is the completion 1.0"
        );
        for pair in ticks.windows(2) {
            assert!(
                pair[0] <= pair[1],
                "§1.11: fractions are monotonically non-decreasing: {ticks:?}"
            );
        }
        for &fraction in &ticks[..ticks.len() - 1] {
            assert!(
                (0.0..1.0).contains(&fraction),
                "§1.11: each intermediate fraction is in [0,1): {fraction}"
            );
        }
    }

    // §6.4.1 unit (G15): the P3.43 progress gate is on the DECODED text, NOT the raw source bytes — the
    // discriminating case. A Latin-1 / Windows-1252 source of high-range bytes (0xE9 = 'é' → the 2-byte UTF-8
    // U+00E9) EXPANDS on decode, so a source that is BELOW one chunk on disk but whose DECODED text exceeds one
    // chunk must still report intermediate progress. A source-byte gate would see `< 1 chunk` → single tick
    // (the regression this pins); the `text_len` gate emits real intermediates (§1.11 "working, not hung").
    #[test]
    fn transform_reports_progress_when_a_sub_chunk_source_decodes_past_a_chunk() {
        let mut source = Vec::new();
        // ~75 KiB source (< one 100 KiB chunk) of mostly high-range bytes → the decoded UTF-8 exceeds one chunk.
        while source.len() < PROGRESS_CHUNK_BYTES * 3 / 4 {
            source.extend([0xE9u8; 20]);
            source.push(b',');
            source.extend([0xE9u8; 20]);
            source.push(b'\n');
        }
        assert!(
            source.len() < PROGRESS_CHUNK_BYTES,
            "the source is deliberately below one chunk on disk ({} bytes)",
            source.len()
        );
        let (out, ticks) = transform_collecting_ticks(&source, CsvTsvTarget::Tsv)
            .expect("transforms a high-range single-byte source");
        assert!(!out.is_empty(), "the sub-chunk source produced output");
        assert!(
            ticks.len() >= 2,
            "§1.11: a sub-chunk SOURCE whose decoded text spans chunks still emits intermediate ticks: {ticks:?}"
        );
        assert_eq!(
            ticks.last().copied(),
            Some(1.0),
            "§1.11: the final tick is the completion 1.0"
        );
    }

    // §6.4.1 unit (G15): the P3.44 cooperative cancel. A `should_cancel` firing at the first chunk boundary
    // stops the transform MID-STREAM → TransformStatus::Cancelled, a partial (< full) output, and NO final
    // 1.0 completion tick (§1.7 InProcessNative cancel: "cleanly discards the one in progress").
    #[test]
    fn transform_stops_mid_stream_and_reports_cancelled_when_the_poll_fires() {
        // A multi-chunk source so a boundary is crossed (the poll granularity, PROGRESS_CHUNK_BYTES).
        let mut source = Vec::new();
        while source.len() < PROGRESS_CHUNK_BYTES * 3 {
            source.extend_from_slice(b"a,b,c\n");
        }
        let full = transform(&source, CsvTsvTarget::Tsv).expect("the full transform completes");

        let mut out = Vec::new();
        let mut ticks = Vec::new();
        let status = transform_bytes(
            &source,
            CsvTsvTarget::Tsv,
            &mut out,
            &mut |fraction| ticks.push(fraction),
            &mut || true,
        )
        .expect("a cancelled transform is not an error");
        assert_eq!(
            status,
            TransformStatus::Cancelled,
            "§1.7: a firing cancel poll stops the transform mid-stream → Cancelled"
        );
        assert!(
            out.len() < full.len(),
            "§1.7: the cancelled output is partial (stopped mid-stream): {} < {}",
            out.len(),
            full.len()
        );
        assert_ne!(
            ticks.last().copied(),
            Some(1.0),
            "§1.11: no final 1.0 completion tick fires on cancel: {ticks:?}"
        );
    }

    // §6.4.1 unit (G15): the P3.44 cancel is polled ONLY at chunk boundaries (§1.7 InProcessNative sub-case). A
    // sub-chunk source crosses no boundary, so an always-cancelling poll is NEVER reached — the near-instant
    // pass Completes (the §1.7 "cancelling keeps the files already finished" semantics: a tiny file finishes
    // before a cancel could be observed).
    #[test]
    fn a_sub_chunk_transform_completes_even_when_the_poll_would_cancel() {
        let mut out = Vec::new();
        let status = transform_bytes(
            b"a,b\n1,2\n",
            CsvTsvTarget::Tsv,
            &mut out,
            &mut |_| {},
            &mut || true,
        )
        .expect("transforms");
        assert_eq!(
            status,
            TransformStatus::Completed,
            "§1.7: a sub-chunk source crosses no boundary, so the always-true cancel poll is never reached"
        );
        assert_eq!(
            out, b"a\tb\n1\t2\n",
            "the sub-chunk source transformed fully"
        );
    }

    #[test]
    fn from_token_parses_the_two_canonical_tokens() {
        use std::ffi::OsStr;
        assert_eq!(
            CsvTsvTarget::from_token(OsStr::new("csv")),
            Some(CsvTsvTarget::Csv)
        );
        assert_eq!(
            CsvTsvTarget::from_token(OsStr::new("tsv")),
            Some(CsvTsvTarget::Tsv)
        );
        assert_eq!(
            CsvTsvTarget::from_token(OsStr::new("xlsx")),
            None,
            "a non-CSV/TSV token -> None (a mis-routed selection)"
        );
    }

    #[test]
    fn transform_error_maps_to_the_conversion_kind() {
        assert_eq!(
            ConversionErrorKind::from(TransformError::Malformed),
            ConversionErrorKind::Corrupt
        );
        assert_eq!(
            ConversionErrorKind::from(TransformError::NotText),
            ConversionErrorKind::Corrupt
        );
        assert_eq!(
            ConversionErrorKind::from(TransformError::AmbiguousDelimiter),
            ConversionErrorKind::Corrupt
        );
        // §1.1 turn-time read failure: a now-missing source (NotFound) → Gone; permission / lock / other IO →
        // Unreadable (the outcome::read_failure_to_error_kind split).
        assert_eq!(
            ConversionErrorKind::from(TransformError::Read(io::Error::from(
                io::ErrorKind::NotFound
            ))),
            ConversionErrorKind::Gone
        );
        assert_eq!(
            ConversionErrorKind::from(TransformError::Read(io::Error::other("x"))),
            ConversionErrorKind::Unreadable
        );
        assert_eq!(
            ConversionErrorKind::from(TransformError::Write(io::Error::other("x"))),
            ConversionErrorKind::WriteFailed
        );
    }

    #[test]
    fn a_missing_source_is_a_read_error_carrying_the_io_detail() {
        // The path wrapper surfaces a real read failure as `Read(io::Error)`, and `io_source` exposes the io
        // detail for the §7.5 log (the P3.43-P3.45 executor records it). A missing file → NotFound.
        let missing = Path::new("this-convertia-source-does-not-exist.csv");
        let err = csv_tsv_transform(
            missing,
            CsvTsvTarget::Tsv,
            Vec::new(),
            &mut |_| {},
            &mut || false,
        )
        .expect_err("a missing source fails");
        assert!(
            matches!(err, TransformError::Read(_)),
            "a missing source is a Read error"
        );
        assert_eq!(
            err.io_source().map(io::Error::kind),
            Some(io::ErrorKind::NotFound),
            "the missing-file read error carries its NotFound io::Error detail (for the §7.5 log)"
        );
        assert_eq!(
            ConversionErrorKind::from(err),
            ConversionErrorKind::Gone,
            "§1.1: a turn-time-vanished source (NotFound) maps to Gone, not Unreadable"
        );
    }

    #[test]
    fn io_source_is_present_for_io_errors_and_absent_for_content_errors() {
        assert!(
            TransformError::Write(io::Error::other("x"))
                .io_source()
                .is_some(),
            "a write failure carries its io::Error source (for the §7.5 log)"
        );
        assert!(
            TransformError::Malformed.io_source().is_none(),
            "a content failure (Malformed) has no io source"
        );
    }

    // ─── P3.42 §3.5.6 CSV-injection literal-preservation (the G32 reader-side rule) ───────────────────────

    #[test]
    fn output_preserves_leading_formula_injection_cells() {
        // §3.5.6: the four leading `= + - @` cells survive as LITERAL field values when the output is read
        // back with a real RFC-4180 reader (the G32 rule, P3.42) — CSV-injection non-execution on the output.
        let out =
            transform(b"=1+1,+2,-3,@SUM(A1)\nx,y,z,w\n", CsvTsvTarget::Tsv).expect("transforms");
        assert_injection_cells_preserved(&out, b'\t', &[b"=1+1", b"+2", b"-3", b"@SUM(A1)"])
            .expect("all four leading = + - @ cells survive as literal field values");
    }

    #[test]
    fn a_requoted_injection_cell_is_still_preserved() {
        // An injection cell containing the TARGET delimiter (a tab) is RFC-4180 re-quoted to stay ONE field,
        // and still reads back as the literal cell value (the re-quote does not mangle it).
        let out = transform(b"h1,h2\n=a\tb,plain\n", CsvTsvTarget::Tsv).expect("transforms");
        assert_injection_cells_preserved(&out, b'\t', &[b"=a\tb"])
            .expect("a re-quoted injection cell survives as one literal field");
    }

    #[test]
    fn injection_cells_survive_both_directions() {
        let out = transform(b"=x\t@y\n1\t2\n", CsvTsvTarget::Csv).expect("transforms");
        assert_injection_cells_preserved(&out, b',', &[b"=x", b"@y"])
            .expect("injection cells survive TSV→CSV too");
    }

    #[test]
    fn the_injection_checker_catches_a_mangled_output() {
        // Planted-positive (non-vacuity): a hand-crafted TSV output where the `=1+1` cell was SPLIT (a stray
        // tab injected mid-cell) reads back as `=1` / `+1`, NOT a literal `=1+1` field → the checker flags it.
        let mangled = b"=1\t+1\tok\n";
        assert_eq!(
            assert_injection_cells_preserved(mangled, b'\t', &[b"=1+1"]),
            Err(InjectionCellNotPreserved {
                cell: b"=1+1".to_vec()
            }),
            "a split / mangled injection cell is caught — the checker is not vacuous"
        );
    }
}

#[cfg(test)]
mod csv_tsv_corpus_binding {
    //! §6.4.3 per-pair corpus binding (P3.62) — the FIRST binding of the G31 output-validity readers +
    //! the G32(a) source-unchanged and G32(c) determinism invariants to REAL CSV/TSV corpus data (the
    //! §6.4.5 P3.61 fixtures). The `transform_tests` module above is the §6.4.1 UNIT level (G15) over
    //! crafted byte literals; this module is the corpus-driven reader binding the §6.4.3 output-validity
    //! bar (G31) + §2.5/G32 invariants specify: the produced output is read back by the REAL RFC-4180
    //! `csv` reader (never a magic-sniff / bare field-count), the source bytes are proven byte-identical
    //! before/after (no-harm), and the transform is deterministic. The invariant homes are P0.5.5/P0.5.6
    //! (test-strategy §0.2/§1.4/§2); this box activates them for the native pair, mirroring the P4.59
    //! runner's `needs: P0.5.6` activation pattern for every subsequent engine.
    //!
    //! [Build-Session-Entscheidung: P3.62] The binding drives the ENGINE transform (`csv_tsv_transform` +
    //! the P3.42 `assert_injection_cells_preserved` reader) — the natural level for the OUTPUT-VALIDITY
    //! readers, whose primitives live here (the module dead-code note names P3.62 as the injection
    //! checker's first caller). The FULL drop→…→publish→summary vertical slice + no-clobber + the §6.5
    //! ledger is the SEPARATE P3.63 runner box; the source-unchanged proof at this level (the transform
    //! reads the source, never writes it) is verified against a temp COPY so a committed corpus fixture
    //! can never be mutated by the test.
    use super::*;
    use crate::test_corpus::fixture;

    /// The convertible CSV-source corpus fixtures — each backs the `CSV → TSV` pair (manifest `covers`).
    const CSV_TO_TSV: &[&str] = &[
        "canonical.csv",
        "cp1252.csv",
        "quoted_fields.csv",
        "injection.csv",
        "cjk_rtl.csv",
        "ragged_zero.csv",
        "expansion_sentinel.csv",
        "semicolon_decimal.csv",
        "pipe.csv",
        "utf8_bom.csv",
        "utf16le_bom.csv",
        "utf16be_bom.csv",
    ];
    /// The convertible TSV-source corpus fixtures — each backs the `TSV → CSV` pair (manifest `covers`).
    /// `tsv_as_csv.csv` is a `.csv`-named file that is CONTENT-detected as TSV (§04 CSV rule "content over
    /// name"), so it is a genuine TSV source of the reverse pair.
    const TSV_TO_CSV: &[&str] = &["canonical.tsv", "quoted_fields.tsv", "tsv_as_csv.csv"];

    /// The §3.5.6 CSV-injection cells authored in `injection.csv` (its formula column) — the leading
    /// `= + - @` four-token set plus the classic payload. The G31/G32 output-side check asserts every one
    /// survives as a LITERAL field (CSV-injection non-execution on the output side, never `'`-neutralised).
    const INJECTION_CELLS: &[&[u8]] = &[
        b"=1+1",
        b"+1-1",
        b"-2+3",
        b"@SUM(A1)",
        b"=cmd|' /c calc'!A0",
    ];

    /// Stage a corpus fixture as a temp COPY (never the committed corpus file) and return the live temp dir
    /// (the caller keeps `_dir` in scope so it is not deleted) + the staged source path. EVERY test drives
    /// the transform against this copy — the success path and the decline path (`ambiguous.csv`) alike — so
    /// the G32(a) source-unchanged proof is honest AND no test can ever mutate a committed fixture.
    fn stage(name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("create a temp source dir");
        let staged = dir.path().join(name);
        let original = std::fs::read(fixture(name)).expect("read the corpus fixture");
        std::fs::write(&staged, &original).expect("stage the source copy");
        (dir, staged)
    }

    /// Convert a corpus fixture through the native transform (staged via [`stage`]), returning
    /// `(source_before, source_after, output)` for the source-unchanged + output-validity assertions.
    fn convert(name: &str, target: CsvTsvTarget) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let (_dir, staged) = stage(name);
        let before = std::fs::read(&staged).expect("read the staged source before");
        let mut output = Vec::new();
        let status = csv_tsv_transform(&staged, target, &mut output, &mut |_| {}, &mut || false)
            .expect("the native transform succeeds on a well-formed corpus source");
        assert_eq!(
            status,
            TransformStatus::Completed,
            "{name}: a never-cancelling transform runs to completion"
        );
        let after = std::fs::read(&staged).expect("read the staged source after");
        (before, after, output)
    }

    /// Read `output` back with the REAL RFC-4180 `csv` reader at `delimiter` (the G31 "real structural
    /// reader" bar — NOT a magic re-detect / bare field-count parity, which pass on mis-quoted or
    /// embedded-newline output that is unparseable). Returns the record count; `expect` fails the test if
    /// the output is not valid RFC-4180.
    fn record_count(output: &[u8], delimiter: u8) -> usize {
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(false)
            .flexible(true)
            .from_reader(output);
        let mut record = csv::ByteRecord::new();
        let mut records = 0usize;
        while reader
            .read_byte_record(&mut record)
            .expect("the transform output parses as valid RFC-4180")
        {
            records = records.saturating_add(1);
        }
        records
    }

    /// The G31 output-validity bar over one produced output (§6.4.3, reused by G32's (b) leg): parseable by
    /// the real RFC-4180 reader (≥1 record), non-empty, `output != input` (no silent passthrough), and
    /// size-plausible.
    fn assert_output_valid(name: &str, input: &[u8], output: &[u8], delimiter: u8) {
        assert!(
            !output.is_empty(),
            "{name}: the output is non-empty (not an empty/stub file)"
        );
        assert!(
            record_count(output, delimiter) > 0,
            "{name}: the RFC-4180 reader decodes at least one record"
        );
        // G31 sub-assertion (2): where `src_format != tgt_format` the output must differ from the input — a
        // delimiter swap changes the bytes for any multi-column source (every convertible fixture here holds
        // at least one delimiter), so a byte-identical output would be a silent passthrough of the source.
        assert_ne!(
            output, input,
            "{name}: output != input (no silent passthrough of the source bytes)"
        );
        // G31 sub-assertion (1) — size-plausibility. A CSV↔TSV transform re-encodes to UTF-8 (no BOM) and
        // swaps one delimiter, so the output stays within a narrow factor of the source: UTF-16→UTF-8 roughly
        // halves (~0.53×), a Windows-1252 source whose bytes expand to multi-byte UTF-8 can grow (~1.15×), and
        // RFC-4180 re-quoting adds only a bounded per-field overhead. The [0.25×, 4×] band bounds every corpus
        // fixture with margin while still catching a truncated stub (too small) or a runaway (too large).
        // Integer cross-multiplication avoids float rounding. [Build-Session-Entscheidung: P3.62]
        assert!(
            output.len().saturating_mul(4) >= input.len(),
            "{name}: output {} is not implausibly small vs input {}",
            output.len(),
            input.len()
        );
        assert!(
            output.len() <= input.len().saturating_mul(4),
            "{name}: output {} is not implausibly large vs input {}",
            output.len(),
            input.len()
        );
    }

    #[test]
    fn csv_to_tsv_source_unchanged_and_output_valid() {
        for &name in CSV_TO_TSV {
            let (before, after, output) = convert(name, CsvTsvTarget::Tsv);
            // G32(a) SOURCE-UNCHANGED (the no-harm proof, T2/T7). Byte equality is the sha256-equality
            // semantic — equal bytes ⟺ equal sha256, with no collision risk.
            assert_eq!(before, after, "{name}: SOURCE-UNCHANGED (G32(a) no-harm)");
            assert_output_valid(name, &before, &output, b'\t');
        }
    }

    #[test]
    fn tsv_to_csv_source_unchanged_and_output_valid() {
        for &name in TSV_TO_CSV {
            let (before, after, output) = convert(name, CsvTsvTarget::Csv);
            assert_eq!(before, after, "{name}: SOURCE-UNCHANGED (G32(a) no-harm)");
            assert_output_valid(name, &before, &output, b',');
        }
    }

    #[test]
    fn injection_cells_survive_literally_in_the_output() {
        // The G31/G32 CSV-injection output-side check, bound over the §6.4.5 injection fixture (P3.61) — the
        // binding the P3.42 checker's dead-code note names P3.62 as the caller of. Scope: CSV→TSV only —
        // P3.61's set (its 2026-07-17 scope ruling) authored no `injection.tsv`, so the reverse direction has
        // no corpus binding; both directions share the delimiter-parametrised `transform_bytes`, so the gap is
        // low-risk. An `injection.tsv` fixture would extend the check to TSV→CSV.
        let (_, _, output) = convert("injection.csv", CsvTsvTarget::Tsv);
        assert_injection_cells_preserved(&output, b'\t', INJECTION_CELLS)
            .expect("§3.5.6 CSV-injection cells survive as literal fields in the TSV output");
    }

    #[test]
    fn conversion_is_deterministic() {
        // G32(c) determinism — same source + settings twice → byte-identical output (== sha256(out1) ==
        // sha256(out2)). Asserted for BOTH native output-format categories (TSV output AND CSV output) — the
        // ≥1-pair-per-output-category determinism floor for the in-core CSV/TSV engine. The LF terminator +
        // deterministic quoting make the transform reproducible, so §2.5 re-run-equivalence rests on a real
        // property and no embedded timestamp / uninitialised padding can leak into an offline app's output.
        for (name, target) in [
            ("canonical.csv", CsvTsvTarget::Tsv),
            ("canonical.tsv", CsvTsvTarget::Csv),
        ] {
            let (_, _, first) = convert(name, target);
            let (_, _, second) = convert(name, target);
            assert_eq!(first, second, "{name}: deterministic output (G32(c))");
        }
    }

    #[test]
    fn ambiguous_source_declines_and_backs_no_pair() {
        // `ambiguous.csv` has no consistent delimiter, so the transform declines with `AmbiguousDelimiter`
        // rather than emitting a mis-quoted output — which is why it carries NO manifest `covers` (it backs
        // no conversion pair). This guards the corpus partition: the convertible-fixture lists above are the
        // pair-backers, and this decline is principled, not an accidental omission.
        let (_dir, staged) = stage("ambiguous.csv");
        let before = std::fs::read(&staged).expect("read the staged source before");
        let mut output = Vec::new();
        let error = csv_tsv_transform(
            &staged,
            CsvTsvTarget::Tsv,
            &mut output,
            &mut |_| {},
            &mut || false,
        )
        .expect_err("ambiguous.csv has no consistent delimiter to re-quote faithfully");
        assert!(
            matches!(error, TransformError::AmbiguousDelimiter),
            "ambiguous.csv declines to AmbiguousDelimiter, got {error:?}"
        );
        // No-harm holds even on the decline path — the transform reads the source, never writes it.
        let after = std::fs::read(&staged).expect("read the staged source after");
        assert_eq!(
            before, after,
            "ambiguous.csv: SOURCE-UNCHANGED on the decline path"
        );
    }
}
