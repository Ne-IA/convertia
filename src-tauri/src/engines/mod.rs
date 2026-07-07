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
        reason = "the §3.2 engine-seam descriptor types EngineId/EngineKind/EngineDescriptor + the §7.2.3 EngineStatus/EngineHealth wire DTOs (P2.110/P2.111) are dead in the production build until the P4.1 registry/trait/selection + the §0.9 pool + the P4.45 startup probe construct them. The C12 get_engine_health return (P2.113) REGISTERS EngineStatus/EngineHealth into bindings.ts via its Result<EngineHealth, IpcError> signature, but its honest Err shell constructs neither, so their fields stay unread (dead) until the P4.45 probe assembles the real Ok(EngineHealth). AppInfo (P2.112) + the §3.2.2 Platform leaf (P2.132) are now LIVE — P2.98's C11 get_app_info assembles a real Ok(AppInfo) (AppInfo::gather()), constructing Platform via current_platform(); the P4 capabilities(platform) consumers construct Platform further. The P3.4 §3.2.2 plan-seam hull (Invocation/EngineProgram/StdinPlan/TempPath/PlanError/ProgressModel) + the §1.7 EngineInvocation/InvocationResult + the dispatch fn are authored ahead of their consumers — P3.5 constructs the first Invocation via Engine::plan(), P3.43-P3.45 rewrite the dispatch InProcessNative arm and P4.13 the subprocess arms — so they stay dead in the production build until then (the cfg(test) tests below construct + exercise them, so the test build is dead-code-clean)."
    )
)]

use std::ffi::OsString;
use std::path::PathBuf;

use serde::Serialize;
use specta::Type;
use tokio_util::sync::CancellationToken;

use crate::domain::{JobId, TargetId};
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
/// `crate::isolation`. `out_tmp` is `Some` for every ENCODE invocation (the §2.1 publish artifact) and `None`
/// for a read-only sub-invocation with no publish artifact — the video PROBE (`ffprobe`, §3.2.1): §1.7
/// atomic-publishes ONLY when `out_tmp.is_some()`.
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
    /// The publish-temp the engine writes to — `Some` for an encode, `None` for the read-only probe (§3.2.2);
    /// the §2.1 atomic publish consumes it on item success (drop is a no-op then). Typed with the §3.2.2
    /// `TempPath` alias (= `tempfile::TempPath`) — the alias references an EXTERNAL type, so it does not trip
    /// the P2.19 within-module forward-declared-alias dead-code interaction.
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
    // `out_tmp: None` (the read-only shape; an encode carries `Some(TempPath)`) — and reads every field so the
    // test build is dead-code-clean.
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
            "§3.2.2: the read-only shape carries out_tmp None; an encode Invocation carries Some(TempPath)"
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
}
