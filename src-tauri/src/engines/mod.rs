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
//! (§3.2.2 — `ProbeOutput` stays P4.2), and the `dispatch` fn (the exhaustive `EngineProgram` routing). All
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
        reason = "the §3.2 engine-seam descriptor types EngineId/EngineKind/EngineDescriptor + the §7.2.3 EngineStatus/EngineHealth wire DTOs (P2.110/P2.111) are dead in the production build until the P4.1 registry/trait/selection + the §0.9 pool + the P4.45 startup probe construct them. The C12 get_engine_health return (P2.113) REGISTERS EngineStatus/EngineHealth into bindings.ts via its Result<EngineHealth, IpcError> signature, but its honest Err shell constructs neither, so their fields stay unread (dead) until the P4.45 probe assembles the real Ok(EngineHealth). AppInfo (P2.112) + the §3.2.2 Platform leaf (P2.132) are now LIVE — P2.98's C11 get_app_info assembles a real Ok(AppInfo) (AppInfo::gather()), constructing Platform via current_platform(); the P4 capabilities(platform) consumers construct Platform further. The P3.4 §3.2.2 plan-seam hull (Invocation/EngineProgram/StdinPlan/TempPath/PlanError/ProgressModel) + the §1.7 EngineInvocation/InvocationResult + the dispatch fn — plus the P3.5 minimal Engine trait, the PlanOutcome return, and the NativeCsvTsvEngine impl — are authored ahead of their consumers: the P4.1 §3.2.3 registry constructs the native engine, P3.41 runs its planned transform, P3.43-P3.45 rewrite the dispatch InProcessNative arm, and P4.13 the subprocess arms — so they stay dead in the production build until then (the cfg(test) tests below construct + exercise them — the native engine's plan() is called there — so the test build is dead-code-clean). The P3.41 §3.5.6 native transform (csv_tsv_transform / transform_bytes / CsvTsvTarget / TransformError / delimiter_byte) is likewise dead until the P3.43-P3.45 dispatch-arm rewrite runs it on crate::pool::run_in_core."
    )
)]

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use specta::Type;
use tokio_util::sync::CancellationToken;

use crate::detection::{
    classify_delimiter, classify_encoding, Delimiter, DelimiterClass, MAX_HEADER_WINDOW,
};
use crate::domain::{DroppedItem, FormatId, JobId, TargetId};
use crate::outcome::ConversionErrorKind;

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
// reconcile). `ProbeOutput` stays P4.2 — the §3.2.1 two-phase probe leg is P4-only, referenced by neither the
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
/// [`ConversionErrorKind`]; the orchestrator (`crate::run`) maps it to the wire `ErrorKind` via
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
/// **P3 walking-skeleton state (the honest seam).** Every arm returns the honest
/// `InvocationResult::Failed(InternalError)` (§2.13, the P2.25 unreachable-outcome precedent): no execution
/// lane is wired at P3.4. The `InProcessNative` lane's native §1.7 lifecycle is authored across P3.43
/// (self-reported progress) / P3.44 (cooperative cancel) / P3.45 (wall-clock timeout), which rewrite that arm
/// to run the CSV/TSV transform on `crate::pool::run_in_core`; the subprocess lanes are
/// unreachable-by-construction in the walking skeleton (no subprocess engine is registered — the registry +
/// engines land at P4.4) and P4.13 rewrites them to route through `crate::isolation::run_confined`.
/// [Build-Session-Entscheidung: P3.4]
#[must_use]
pub fn dispatch(invocation: &EngineInvocation) -> InvocationResult {
    match &invocation.plan.program {
        // The one walking-skeleton lane — the native CSV/TSV engine (§3.5.6). P3.43/P3.44/P3.45 rewrite this
        // arm to run the §1.7 InProcessNative lifecycle on crate::pool::run_in_core; the honest InternalError
        // seam holds meanwhile (§2.13, P2.25).
        EngineProgram::InProcessNative(_) => {
            InvocationResult::Failed(ConversionErrorKind::InternalError)
        }
        // Subprocess lanes — unreachable-by-construction in the P3 walking skeleton (no subprocess engine is
        // registered; the registry + engines land at P4.4). P4.13 authors crate::isolation::run_confined and
        // rewrites these arms to route through it; the honest InternalError seam holds meanwhile (§2.13, P2.25).
        EngineProgram::Sidecar(_) | EngineProgram::ResourceBin { .. } => {
            InvocationResult::Failed(ConversionErrorKind::InternalError)
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

/// Run the §3.5.6 native CSV/TSV transform (P3.41): read `source`, re-detect its encoding + delimiter, and
/// stream it to `out` at the `target` delimiter with RFC-4180 re-quoting.
///
/// **§3.5.6 record pass:** the source is read into memory + decoded to UTF-8 (no BOM), then each RFC-4180
/// record is parsed at the source delimiter and re-written at the target delimiter — the `csv` writer quotes
/// only fields containing the new delimiter / a quote / a newline (RFC-4180 `QuoteStyle::Necessary`), so every
/// field's VALUE is preserved byte-for-byte (incl. a leading `= + - @` — the CSV-injection-safe literal
/// preservation, §3.5.6, bound by G32 at P3.42). Output line terminator = LF (`\n`)
/// [Build-Session-Entscheidung: P3.41] — deterministic + cross-platform (the P3.61 `sha256` determinism
/// sub-assertion), never the RFC-4180 CRLF.
///
/// The read is **whole-file-buffered** here, not byte-streamed: the §1.10 preflight bounds the size, and the
/// byte-STREAMED read + the §1.7 `bytes_processed / source_size` progress seam thread through at P3.43. And
/// `source` MUST be a regular file: the FIFO / blocking-read pre-open type-check is the P3.49 read-path
/// wiring's job (§2.12.4), and the wall-clock / wedged-read time bound is P3.45 — this pass owns none of them.
pub fn csv_tsv_transform(
    source: &Path,
    target: CsvTsvTarget,
    out: impl Write,
) -> Result<(), TransformError> {
    let bytes = std::fs::read(source).map_err(TransformError::Read)?;
    transform_bytes(&bytes, target, out)
}

/// The pure byte→byte core of [`csv_tsv_transform`] (source bytes in, transformed bytes out) — the transform
/// LOGIC, separated from the file read so it is unit-testable over byte literals. [Build-Session-Entscheidung: P3.41]
fn transform_bytes(
    bytes: &[u8],
    target: CsvTsvTarget,
    out: impl Write,
) -> Result<(), TransformError> {
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

    let mut record = csv::ByteRecord::new();
    loop {
        // The byte-level invalid-bytes failure is already handled above (`had_errors` → Malformed). The `csv`
        // reader itself parses PERMISSIVELY here (a `ByteRecord` never re-validates UTF-8, and `flexible(true)`
        // suppresses the unequal-field-count error over an in-memory source that cannot I/O-fail), so its `Err`
        // arm is a DEFENSIVE catch for an unexpected reader fault (mapped to `Malformed`), not reached in
        // practice. A write error is an out_tmp I/O failure. Either way the pass stops with no partial publish
        // (the §2.1 temp is discarded on drop).
        match reader.read_byte_record(&mut record) {
            Ok(true) => writer
                .write_byte_record(&record)
                .map_err(|e| TransformError::Write(io::Error::other(e)))?,
            Ok(false) => break,
            Err(_) => return Err(TransformError::Malformed),
        }
    }
    writer.flush().map_err(TransformError::Write)?;
    Ok(())
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

    // §6.4.1 unit (G15): the §1.7 dispatch — the P3 walking-skeleton contract. No execution lane is wired at
    // P3.4, so the exhaustive `EngineProgram` match returns the honest `Failed(InternalError)` seam (§2.13,
    // P2.25) for EVERY program. P3.43/P3.44/P3.45 rewrite the InProcessNative arm to run the native CSV/TSV
    // §1.7 lifecycle on crate::pool::run_in_core; P4.13 rewrites the subprocess arms via run_confined — each
    // updates this test alongside the code. This locks the exhaustive routing + the current seam value.
    #[test]
    fn dispatch_returns_the_honest_internal_error_seam_for_every_program() {
        for program in [
            EngineProgram::InProcessNative(EngineId::NativeCsvTsv),
            EngineProgram::Sidecar(EngineId::FFmpeg),
            EngineProgram::ResourceBin {
                engine: EngineId::LibreOffice,
                rel: PathBuf::from("program/soffice"),
            },
        ] {
            let invocation = engine_invocation(program);
            assert_eq!(
                dispatch(&invocation),
                InvocationResult::Failed(ConversionErrorKind::InternalError),
                "§1.7/§2.13: the P3.4 dispatch returns the honest InternalError seam for every program (no lane wired yet)"
            );
        }
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

    /// Run `transform_bytes` and return the produced output bytes (the common test shape).
    fn transform(bytes: &[u8], target: CsvTsvTarget) -> Result<Vec<u8>, TransformError> {
        let mut out = Vec::new();
        transform_bytes(bytes, target, &mut out)?;
        Ok(out)
    }

    #[test]
    fn csv_to_tsv_swaps_the_delimiter() {
        let out = transform(b"a,b,c\n1,2,3\n", CsvTsvTarget::Tsv).expect("valid CSV transforms");
        assert_eq!(
            out, b"a\tb\tc\n1\t2\t3\n",
            "§3.5.6: comma source -> tab-delimited output, LF terminator"
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
        csv_tsv_transform(&src, CsvTsvTarget::Tsv, &mut out).expect("transforms a real file");
        assert_eq!(
            out, b"a\tb\n1\t2\n",
            "§3.5.6: the path wrapper reads + transforms a real source file"
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
        let err = csv_tsv_transform(missing, CsvTsvTarget::Tsv, Vec::new())
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
}
