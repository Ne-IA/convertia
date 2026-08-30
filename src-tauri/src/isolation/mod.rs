//! `crate::isolation` — the §2.12 decoder-isolation wrapper every engine SUBPROCESS spawn routes through,
//! and the SOLE sanctioned `process::Command::new` site in the codebase — the concrete spawn primitive is
//! `tokio::process::Command` (the async spawn the §2.12 confined runner awaits under the §0.4.4 cancel
//! token): the G9 repo-invariant (b) scopes its qualified `process::Command::new` grep to this module, and the G29 spawn rule excludes
//! `**/isolation/**` from the spawn-outside-isolation ban — keeping every spawn inside this module is what
//! makes those two gates honest. A §0.7 tier-2 module: the §1.7 invocation lifecycle CALLS it and §3.5
//! builds the engine args INSIDE it; it depends DOWN only, never up on IPC / orchestrator / the engine
//! registry. Unsafe-free — the crate-root `#![deny(unsafe_code)]` (main.rs) covers it; the §2.12.3
//! privilege-drop tier reaches its per-OS confinement through SAFE wrapper crates (`process-wrap`
//! group-kill / Job-Object teardown) and through SAFE `crate::platform` shims (the best-effort Linux
//! seccomp / Landlock / net-namespace legs, P4.15; the Windows intermediate-integrity write confinement +
//! own-Job-Object legs, P4.17), so this module adds NO FFI and NO `unsafe`; the confined-spawn entry
//! [`run_confined`] below is the P4.13-authored cheap-tier body.
//!
//! ## The confined-spawn entry (P3.2 contract map → P4.13-authored body)
//! [Build-Session-Entscheidung: P3.2] This root was a documented CONTRACT MAP through P3 (as in
//! `crate::fs_guard` / `crate::run`, P3.1.1 / P3.1.2) — no honest non-spawn body existed, and the entry
//! could not even be typed before `crate::engines` authored `EngineInvocation`/`InvocationResult` (P3.4).
//! **P4.13 expanded the map into the real entry below** (never a spawn stub rebuilt later):
//!  - [`run_confined`]`(inv: &EngineInvocation, program: &Path, on_progress: impl Fn(f32)) -> ConfinedRun` —
//!    the §1.7 confined-spawn entry every SUBPROCESS engine invocation routes through: the §2.12.1 OS process
//!    boundary + the §2.12.3 cheap-tier floor (P4.13) + the §1.7 per-`ProgressModel` stdout/stderr handling
//!    (P4.8 — streaming line-reader → `on_progress`, `CoarseSpawnDone` stdout buffered, stderr captured in
//!    full; returned in the [`crate::engines::ConfinedRun`] outcome) + the §1.7 whole-group spawn/kill
//!    (**P4.10** — `process-wrap` Job-Object / process-group teardown of the engine AND its descendants,
//!    e.g. `soffice` → `soffice.bin`) + the §1.7 step-2 **TIMEOUT-BOUNDED confirm-wait** on the cancel arm
//!    (**P4.11** — after the group-kill, wait up to [`crate::pool::GROUP_CONFIRM_WAIT`] for the OS to reap,
//!    then return regardless, so a wedged descendant cannot hang the cancel/quit path; the deferred-reclaim
//!    `CleanupResidue` tail is the tier-1 conductor's, which owns the temp). **P4.12** added this entry's two
//!    exit-handling halves while leaving it the pure confinement primitive: (a) the crash-vs-clean group-kill
//!    decision — a CLEAN completed wait stands the [`GroupKillGuard`] down, a CRASH (non-zero) completed wait
//!    leaves it armed so the Drop backstop tears the doomed tree down; and (b) surfacing the raw `ExitStatus`
//!    in [`crate::engines::ConfinedRun::exit`] so the §1.7 `run_subprocess` lane (engines-side) can refine the
//!    `EngineCrash` floor via the §3.5 `classify_failure` seam. The §1.7 no-progress / wall-clock watchdog
//!    itself lives engines-side in `bounded_confined_run` (shared by `run_subprocess` + `run_probe_then_encode`;
//!    it drops THIS future on a hang, so the Drop backstop is the kill), NOT here. **P4.14** delivered the
//!    §2.12.3 dynamic-loader-injection env STRIP (the [`is_loader_injection_var`] filter on the constructed
//!    env — `LD_PRELOAD`/`LD_LIBRARY_PATH`/`DYLD_*`, §0.11 T3a). The remaining layers land on THIS entry at
//!    their boxes: the per-OS privilege-drop legs at **P4.15** (Linux, three `pre_exec` legs) and **P4.17**
//!    (Windows, the `WindowsConfinement` `post_spawn` wrapper — a code span: cfg-gated off non-Windows doc builds) — **P4.16** (macOS) is DECIDED cheap-tier
//!    only, no leg attaches (Co-Pilot ruling 2026-07-25) — and the achieved-tier record
//!    into `privilege-drop-coverage.toml` at **P4.18**. It never runs the §2.1 publish — that is
//!    `crate::fs_guard`, invoked by the §1.7 lifecycle after a `Succeeded` return; the §0.9 pool permit is
//!    acquired one layer up (§1.7). `program` is the RESOLVED absolute binary path — the
//!    `EngineProgram → path` resolution is P4.32's (`current_exe().parent()` sidecars /
//!    `BaseDirectory::Resource` resource-tree binaries, §3.3.3), handed in by the caller so this tier-2
//!    module never touches the Tauri path APIs.
//!
//! ## The §2.12.3 two-tier model P4.13+ implements (design-of-record, `[DECIDED — two tiers]`)
//! [Build-Session-Entscheidung: P3.2] Recorded here as the design P4's wrapper is built to, NOT as a Rust
//! type: (1) the **cheap tier** — the §2.12.1 process boundary + §1.7 timeout + cleared / minimal env (with
//! `LD_PRELOAD` / `LD_LIBRARY_PATH` / `DYLD_*` stripped, P4.14) + a scratch-cwd working dir + only the exact
//! input + `tmp` output paths handed in — is the NON-NEGOTIABLE v1 floor, shipped unconditionally on
//! Windows / macOS / Linux. (2) the **privilege-drop tier** — seccomp-bpf / Landlock + net-namespace
//! (Linux), an intermediate-integrity write confinement + an own Job Object with `JOB_OBJECT_LIMIT` caps
//! (Windows, P4.17), nothing beyond the cheap floor (macOS, P4.16) — is best-effort
//! defence-in-depth that degrades SILENTLY to the cheap tier where it cannot be enabled without install-time
//! elevation or breaking the portable build, and is NOT load-bearing (the §0.11 T9b network guarantee rests
//! on the §3.5 / §6.1.3 argv / build controls). The per-OS profile CONTENTS are a §2.12.3 tuning residual.
//! **Windows realization `[DECIDED — P4.17, Co-Pilot ruling 2026-08-25]`:** the restricted-token /
//! AppContainer leg and the AppContainer / WFP network-deny leg are NOT realizable in v1-portable — stable
//! `CommandExt` carries no spawn-token / process-creation-attribute path and `tokio::process::Child` cannot be
//! built from a raw handle, an AppContainer additionally needs `ALL APPLICATION PACKAGES` DACL grants on the
//! portable bundle dir (impossible on a FAT/exFAT stick) and on every input (source-metadata mutation = §2.0
//! harm), and a WFP/firewall rule needs elevation plus a persistent machine-global mutation. What IS realized,
//! both PARENT-SIDE on the `CREATE_SUSPENDED` child before its threads resume: a reduced-integrity token at a
//! ConvertIA-private level strictly between Low and Medium with the write sinks labelled at the same level
//! FIRST (the Windows analogue of the Landlock `{scratch rw}` grant; the label is stripped again before the
//! §2.1.2 publish), and an own Job Object carrying kill-on-job-close plus generous runaway caps. No FFI for
//! the unrealizable legs enters the core (pinned by the `crate::platform`
//! `no_appcontainer_or_spawn_token_ffi_in_the_core` source-scan); the revisit anchor is an installer-build
//! epoch plus a brokered/staged input model. **Network deny therefore has no Windows privilege-drop leg** —
//! the load-bearing offline gate is the §2.11.4 packet-monitor regardless of tier, and the §6.7.3 CI egress
//! gate (an ELEVATED runner firewall, a CI fact) is unaffected. spec §2.12.3 carries the ruling.
//! **macOS realization `[DECIDED — P4.16, Co-Pilot ruling 2026-07-25]`:** the macOS Seatbelt leg is realized as
//! the cheap-tier floor ONLY in v1-portable — its sole apply path is a private-libsandbox call in the
//! post-fork/pre-exec child, which is neither auditable fork-safe nor silent-skippable at its worst case (a
//! hang, not an errno), so §2.12.3's never-break floor forbids it (the Linux in-closure admission test the
//! macOS apply fails). No Seatbelt profile is applied and no private-sandbox FFI enters the core (pinned by the
//! `crate::platform` `no_seatbelt_apply_callsite_in_the_core` source-scan). spec §2.12.3 carries the ruling.
//! Whether the achieved depth surfaces as a Rust tier value (e.g. a `SandboxTier` enum) or as an
//! unconditional cheap floor plus best-effort privilege-drop with no runtime discriminant was left open
//! here for the P4 box that would own its real consumer — no possibly-unused type was planted (CLAUDE §5
//! no-premature-commitment; the P3.1 doc-only precedent). **P4.18 CLOSED it: a Rust value, but a per-LEG
//! one.** `crate::platform::SpawnTier` (a leg id + its `LegOutcome`) is assembled by [`run_confined`] right
//! after the spawn and handed out on [`crate::engines::ConfinedRun::tier`]; the legs are never collapsed
//! into one discriminant, because they degrade independently (a FAT/exFAT or SMB destination degrades the
//! Windows write confinement while its Job Object still attaches, and one value could not say so). Its
//! durable projection is the tracked `privilege-drop-coverage.toml` the G64 ratchet reads.
//!
//! ## §2.12.4 absolute — the P3 walking-skeleton conversion BYPASSES this module entirely
//! [Build-Session-Entscheidung: P3.2] The §2.12.4 absolute forbids any third-party C/C++ decoder in-core;
//! the sole in-core exception is the native CSV/TSV `EngineProgram::InProcessNative` engine (§3.5.6) — pure
//! memory-safe Rust, no third-party bytes — which runs its transform IN-CORE and does NOT route through this
//! module. So the P3 walking skeleton's only live conversion (CSV → TSV) never reaches the confined-spawn
//! seam: the §1.7 dispatch (P3.4) reaches ONLY its `InProcessNative` arm; the `Sidecar` and `ResourceBin`
//! (subprocess-class) arms are unreachable-by-construction in P3 — the walking skeleton wires only the
//! in-core engine, so no subprocess `Invocation` is ever produced (the subprocess engines land P5–P7; the
//! registry landed at P4.4) — and return the honest §2.13 `ConversionErrorKind::InternalError` outcome;
//! they route through this module's [`run_confined`] entry once the P4.32 program-path resolution supplies
//! the resolved binary path the entry takes (no resolvable subprocess program exists before then).

use std::path::Path;
use std::process::Stdio;

use std::ffi::OsStr;

#[cfg(windows)]
use process_wrap::tokio::JobObject;
#[cfg(unix)]
use process_wrap::tokio::ProcessGroup;
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};
use tokio::process::{ChildStderr, ChildStdout, Command};

use crate::engines::{ConfinedRun, EngineInvocation, InvocationResult, ProgressModel, StdinPlan};
use crate::outcome::ConversionErrorKind;
use crate::pool::GROUP_CONFIRM_WAIT;

// The §3.5.0 / §7.2.6 macOS TCC source-staging slice. Declared UNCONDITIONALLY on purpose: a
// `#[cfg(target_os = "macos")]` here would itself be a mac-cfg in the isolation tree OUTSIDE
// `isolation/macos.rs`, which is exactly what check-sast's `misplaced_macos_cfg` leg forbids (it exists so
// the `paths:`-scoped G29 rule (d) can see every mac-conditional isolation slice). The per-item `#[cfg]`
// lives inside that file instead — the established `crate::platform` pattern.
// [Build-Session-Entscheidung: P4.24]
pub(crate) mod macos;

/// The §3.5/§2.12.3 dynamic-loader INJECTION variables the §2.12.3 minimal env strips (P4.14) so a hostile
/// input cannot coerce a side-load (§0.11 T3a): the OS run-time loader honours these to PRELOAD a shared
/// object or PREPEND a library search path, so an engine handed one could be steered to load an
/// attacker-controlled `.so`/`.dylib` ahead of the bundled ones (§3.3.3 absolute-path resolution). `LD_*` are
/// the Linux (glibc) loader's, `DYLD_*` the macOS dyld's; the strip is UNCONDITIONAL on every OS
/// (belt-and-suspenders — filtering a not-present var is a harmless no-op), so no platform path is missed.
/// [Build-Session-Entscheidung: P4.14]
const LOADER_INJECTION_VARS: [&str; 4] = [
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
];

/// True if `name` is a [`LOADER_INJECTION_VARS`] dynamic-loader injection variable (P4.14) — an EXACT,
/// case-sensitive match (the POSIX loaders these target read case-sensitive env-var names). Used by
/// [`run_confined`] to strip the vars from the constructed engine env (the §2.12.3 minimal-env floor).
fn is_loader_injection_var(name: &OsStr) -> bool {
    LOADER_INJECTION_VARS
        .iter()
        .any(|var| name == OsStr::new(var))
}

/// The §2.12 confined-spawn entry (P4.13) — runs ONE subprocess engine invocation inside the §2.12.3
/// **cheap-tier floor**, the NON-NEGOTIABLE v1 confinement shipped unconditionally on all three OSes:
///
/// - the **§2.12.1 process boundary** — a real OS subprocess (`tokio::process`); a decoder that
///   segfaults/aborts takes down only its own process, never the core (§2.12.1);
/// - a **minimal / cleared environment** — `env_clear()` then the plan's `env` pairs MINUS the §3.5/§2.12.3
///   dynamic-loader injection vars (`LD_PRELOAD`/`LD_LIBRARY_PATH`/`DYLD_*`), which are filtered OUT HERE by
///   [`is_loader_injection_var`] (P4.14, §0.11 T3a — a consumer-side strip, defense-in-depth over `env_clear`,
///   which drops only the INHERITED copy). No inherited secrets, no poisoned parent env, no coerced side-load
///   (T9b/T3a, G29 rule (b1));
/// - a **scratch-cwd** — the working directory is the plan's per-run scratch dir (§2.6); a `None` cwd on
///   a confined spawn is a mis-built plan → honest `InternalError`, never an inherited cwd;
/// - **input/tmp-only handing** — the child receives exactly the plan's argv (§3.5 embeds only the
///   resolved input path + the `tmp` output path — never a scannable directory) and null stdin;
/// - **§1.7 per-`ProgressModel` stdout/stderr handling (P4.8)** — stdout + stderr are PIPED and drained
///   CONCURRENTLY with the exit wait (a `tokio::join!` on one task, so a full pipe never back-pressures the
///   child into a deadlock). `stdout` is read per [`EngineInvocation::plan`]'s [`ProgressModel`]: a streaming
///   model ([`ProgressModel::FfmpegKeyValue`] / [`ProgressModel::VipsStdout`]) is read **line by line** and
///   each [`ProgressModel::progress_fraction`] tick is fed to `on_progress`; [`ProgressModel::CoarseSpawnDone`]
///   is **buffered whole** (no line reader — the JSON-blob-safe probe path, §1.7) and returned in
///   [`ConfinedRun::stdout`]; [`ProgressModel::InProcessFraction`] is not a subprocess model (the P3.43 in-core
///   mpsc lane) → the honest mis-wired [`ConversionErrorKind::InternalError`] seam. `stderr` is captured **in
///   full** into [`ConfinedRun::stderr`] for the P4.12 exit-classification / §7.5 echo / §2.13 classify.
///
/// Exit mapping (the pre-classification floor), returned as [`ConfinedRun`]: clean exit → `Succeeded` (the §1.7 non-empty output
/// verification runs conductor-side on that path, the P3.48 re-cut); a non-success exit →
/// `Failed(EngineCrash)` (§2.12.1's reap mapping — P4.12 routes exit≠0 through the §3.5 per-engine
/// `classify_failure` for the precise §2.8 kind); a spawn error (binary missing/denied) →
/// `Failed(InternalError)` — the §2.13.1 ITEM-level answer (a runtime per-item spawn failure fails that one
/// item, §2.13.2; the app-level `EngineMissing`/`BundleDamaged` escalation is the §7.2.3 startup probe's, a
/// distinct path — P4.7-resolved: no per-item AppFault here); a cancel trip →
/// **whole-GROUP kill + a [`GROUP_CONFIRM_WAIT`]-bounded confirm-wait** → `Cancelled` (P4.10 the kill: the
/// engine and every descendant it spawned die together — the `process-wrap` Job Object on Windows, the POSIX
/// process group elsewhere; P4.11 the §1.7 step-2 confirm-wait: after the kill, wait up to the bound for the
/// OS to reap, then return regardless so a wedged descendant cannot hang the cancel/quit path — the return
/// never proves the group empty, so the honest reclaim verdict is the conductor's own §2.6.4 removal, and the
/// deferred-reclaim `CleanupResidue` tail is the tier-1 conductor's, which owns the temp). `StdinPlan::PipeBytes` is
/// unreachable-by-construction until the §3.5.4 pandoc adapter (P7) wires its byte feed — the honest
/// `InternalError` seam (the P2.25 precedent), matched exhaustively so the arm cannot be silently
/// dropped. [Build-Session-Entscheidung: P4.13]
// [Test-Change: P4.7 — old-obsolete+new-correct, §1.7 §2.12.3] the P4.13 dead-code lint level assumed this
// entry had no caller; the §1.7 `engines::run_subprocess` seam (below) now references `run_confined`, so
// relaxing the level is correct — the entry stays unreachable until P4.32 yet is no longer reported unused.
// Mechanism: `run_subprocess` counts as a dead-code-analysis root (via the `engines` module-level dead-code
// lint attribute), so its body marks `run_confined` used even though `run_subprocess` is ITSELF dead until
// P4.32, leaving `run_confined` unreachable but no longer reported unused. dispatch's
// `Sidecar`/`ResourceBin` arms call `run_subprocess` when P4.32's program-path resolution supplies the resolved
// `&Path` (no resolvable subprocess program before then); the cfg(test) real-subprocess suite below exercises
// every arm.
#[cfg_attr(not(test), allow(dead_code))]
pub async fn run_confined(
    invocation: &EngineInvocation,
    program: &Path,
    on_progress: impl Fn(f32),
) -> ConfinedRun {
    // §2.12.3(a): the scratch working directory is MANDATORY on a confined spawn.
    let Some(cwd) = invocation.plan.cwd.as_deref() else {
        return ConfinedRun::failed(ConversionErrorKind::InternalError);
    };
    match invocation.plan.stdin {
        StdinPlan::None => {}
        // No PipeBytes engine is registered before the §3.5.4 pandoc adapter (P7), which owns the byte
        // feed — the honest unreachable-by-construction seam (P2.25). [Build-Session-Entscheidung: P4.13]
        StdinPlan::PipeBytes => {
            return ConfinedRun::failed(ConversionErrorKind::InternalError);
        }
    }

    // §1.7 per-`ProgressModel` stdout handling (P4.8): the two streaming models are read line-by-line into
    // `on_progress` fractions; `CoarseSpawnDone` buffers stdout whole (the JSON-blob-safe probe path, no line
    // reader — a line reader would fragment the single-blob output); `InProcessFraction` is NOT a subprocess
    // model — the native CSV/TSV engine self-reports over the §1.7 in-core mpsc lane (P3.43) and never routes
    // through a confined spawn, so reaching it here is a mis-wired plan → the honest InternalError seam (the
    // PipeBytes-seam precedent). stderr is ALWAYS piped + captured in full below. [Build-Session-Entscheidung: P4.8]
    let line_read_stdout = match &invocation.plan.progress {
        ProgressModel::FfmpegKeyValue { .. } | ProgressModel::VipsStdout => true,
        ProgressModel::CoarseSpawnDone => false,
        ProgressModel::InProcessFraction => {
            return ConfinedRun::failed(ConversionErrorKind::InternalError);
        }
    };

    // §2.12.3 best-effort Windows privilege-drop tier, Leg A step (i) — LABEL-THEN-LOWER (P4.17): BEFORE the
    // spawn, the parent labels every write sink the §2.14.1/§2.14.3 placement chose (the per-run scratch cwd
    // `(OI)(CI)`, and the `.part` publish temp when this invocation produces one) at the ConvertIA-private
    // intermediate integrity level, and reports whether the child's token may therefore be lowered to it. The
    // Windows analogue of the Landlock `{scratch rw}` grant: the grant is issued FIRST, so the lowered child
    // can still write exactly its own sinks and nothing else. `false` = cheap tier for this spawn (a FAT/exFAT
    // or SMB destination, a `Modify`-only folder, a read/execute-blocking label on the engine binary) — never
    // an error, never a broken conversion. The label is removed again before the §2.1.2 publish (the tier-1
    // conductor's single strip site), so `final` never carries it. [Build-Session-Entscheidung: P4.17]
    #[cfg(windows)]
    let lower_to =
        crate::platform::label_confinement_sinks(cwd, invocation.plan.out_tmp.as_deref(), program)
            .then_some(crate::platform::CONFINED_INTEGRITY_RID);

    // The §2.12.3 cheap-tier spawn, built as an OWNED `tokio::process::Command` — the shape `process-wrap`
    // forces (its `CommandWrap` takes the builder BY VALUE, so the P4.13 single fluent `…spawn()` chain cannot
    // survive the P4.10 group-kill wrapping). `env_clear()` is therefore the IMMEDIATELY-following statement:
    // that gap-free construction+scrub pair is exactly the G29 rule-(b1) split-builder suppression the P4.85
    // L(-1) refinement authored FOR this crate ("the owned-Command shape `process-wrap` forces") — a gapped
    // split would redden the SAST. G29 rule (d) (macOS stage_for_tcc-before-spawn) does NOT reach this
    // cross-platform floor: its P4.85-refined form is `paths:`-scoped to the macOS isolation module
    // (`isolation/macos.rs` / `isolation/macos/**`), and this floor embeds no macOS-TCC path (the §3.5.0
    // staging fn + its macOS-scoped spawn land at P4.24) — so no (d) suppression is needed or present.
    // [Build-Session-Entscheidung: P4.13] [Build-Session-Entscheidung: P4.10]
    let mut command = Command::new(program);
    command.env_clear();
    command
        // §3.5/§2.12.3 minimal-env dynamic-loader-injection STRIP (P4.14, §0.11 T3a): filter the dynamic-loader
        // injection vars (LD_PRELOAD/LD_LIBRARY_PATH, DYLD_INSERT_LIBRARIES/DYLD_LIBRARY_PATH) OUT of the
        // constructed env so a hostile input can never coerce a side-load. `env_clear()` above already dropped
        // any INHERITED copy; this filter is the defense-in-depth over the CONSTRUCTED env — no plan env, now or
        // the P5 per-engine whitelist seam, can pass one through. The engine resolves only the bundled shared
        // libs beside it (absolute paths, §3.3.3; `PATH` not relied on). [Build-Session-Entscheidung: P4.14]
        .envs(
            invocation
                .plan
                .env
                .iter()
                .filter(|(name, _)| !is_loader_injection_var(name))
                .map(|(k, v)| (k.clone(), v.clone())),
        )
        .args(&invocation.plan.args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // §2.12.3 best-effort Linux privilege-drop tier (P4.15): attach ALL THREE legs — the network-namespace
    // egress-deny (P4.15.2), Landlock fs-restrict (P4.15.1) and seccomp-bpf exec-deny (P4.15.3) — as ONE
    // best-effort `pre_exec` closure homed in crate::platform (the one core `unsafe` module) so THIS module stays
    // unsafe-free per its P3.2 contract. Net-ns is applied INSIDE the post-fork/pre-exec child (single-threaded,
    // where `unshare(CLONE_NEWUSER|CLONE_NEWNET)` is valid), so a runtime namespace-setup failure just SKIPS
    // net-ns and the engine still runs — the tier is non-load-bearing and NEVER fails the conversion. Because
    // the engine is spawned DIRECTLY (no argv-wrap), a missing binary still surfaces as the §2.13.1 spawn-error
    // `InternalError`, never a masked crash. A `pre_exec` set on the underlying std command (via `as_std_mut`)
    // survives the process-wrap `CommandWrap` (which preserves it) and composes with its setpgid, so the P4.10
    // group-kill still reaps the engine (the process-group leader). [Build-Session-Entscheidung: P4.15]
    #[cfg(target_os = "linux")]
    crate::platform::install_confinement(command.as_std_mut(), program, cwd);

    // §2.12.3 macOS privilege-drop tier: DECIDED cheap-tier ONLY — no best-effort leg is attached here (unlike
    // the Linux `install_confinement` call above). [Decision: P4.16 — Co-Pilot ruling 2026-07-25, anchor
    // never-break > non-load-bearing defence-in-depth] The macOS Seatbelt route would have to apply its profile
    // via the private libsandbox apply API INSIDE the post-fork/pre-exec child of this multithreaded tokio
    // parent (an unsigned portable build has no parent-side or spawn-time apply path). Unlike the Linux legs —
    // where Landlock `restrict_self()` is auditable fork-safe and every failure is an errno the closure silently
    // skips — that private call is (a) closed-source / not provably fork-safe and (b) its WORST case is a HANG
    // (a fork-malloc / dispatch deadlock), which is NOT silent-skippable: a hung child never execs, the §1.7
    // watchdog reaps it, and the item Fails — a never-break violation. §2.12.3's never-break floor (the same
    // admission test that ADMITTED the Linux in-closure legs: auditable + errno-skippable) therefore forbids the
    // macOS apply leg in v1-portable, so macOS runs the P4.13 cheap-tier floor built above unconditionally. No
    // profile artifact is built (no dead code for a decided-not-applied mechanism) and no private-sandbox FFI
    // enters the core — `crate::platform`'s `no_seatbelt_apply_callsite_in_the_core` source-scan pins that on
    // all three CI legs. Revisit anchors (spec §2.12.3): (a) a signed / notarized build epoch with a safe apply
    // path; (b) a future Apple-sanctioned spawn-time sandbox API. [Build-Session-Entscheidung: P4.16]

    // §1.7 `[DECIDED — sole owner]` (P4.10): every engine is spawned as a process-group / job-object LEADER so
    // ONE kill tears down the engine AND ALL ITS DESCENDANTS. Several bundled engines re-exec or launch
    // children of their own — most importantly LibreOffice (`soffice` → `soffice.bin`) — and killing only the
    // IMMEDIATE child ORPHANS them, leaking processes, file handles and scratch files and breaking "cleanly
    // discards the one in progress" (§2.1 no-partial). The composable wrappers, per §1.7:
    //   * `JobObject` (Windows) — engine + children join one Win32 Job Object; `TerminateJobObject` on it
    //     terminates the entire tree. It sets `CREATE_SUSPENDED` itself so the job is assigned before any
    //     thread runs (and resumes them right after).
    //   * `ProcessGroup::leader()` (POSIX) — `setpgid` makes the engine a process-group leader, so ONE kill
    //     signals the WHOLE group (`killpg`), descendants included. (The reaping is the KILL's doing, not the
    //     wait's: `waitpid(-pgid)` only ever collects OUR OWN children, so it can neither reap nor observe a
    //     grandchild — see [`GroupKillGuard`]'s `Drop`.)
    //   * `KillOnDrop` — §1.7 names this shim; it sets tokio's own kill-on-drop flag on the builder, which is
    //     what makes tokio kill + background-reap the IMMEDIATE child if its handle is dropped unwaited (so a
    //     dropped run leaves no zombie). See the FORCED DEVIATION below for what it does NOT buy.
    //
    // FORCED DEVIATION (DoD item 2 — §1.7's kill-on-job-close clause; spec §1.7 reconciled in this commit):
    // §1.7 also expects the Job Object to carry `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` ("closing its last handle
    // with kill-on-close") via the `KillOnDrop` shim, so that even an UNGRACEFUL end of ConvertIA has the OS
    // reap the job. `process-wrap` 9.1.0 CANNOT deliver that, verified in its source: `CommandWrap::spawn_with`
    // does `let mut wrappers = mem::take(&mut self.wrappers);` and then passes `&self` as the `core` argument
    // to every hook, so `core.has_wrap::<KillOnDrop>()` — the read `JobObject::wrap_child` uses to choose the
    // limit — sees an EMPTY wrapper map and is unconditionally `false`, whatever the registration order. The
    // job is therefore created with no kill-on-close limit and there is no public API to add one (the job
    // handle lives in a `pub(crate)` `JobPort`). Consequences + compensations:
    //   * IN-PROCESS teardown is carried by [`GroupKillGuard`] below — every exit path that ends the
    //     invocation WITHOUT a completed engine wait, INCLUDING the whole future being dropped by a caller
    //     (the P4.12 watchdog, the §7.3.3 quit path), issues an explicit whole-group kill. For the paths that
    //     actually need a teardown that is a stronger guarantee than a drop-flag: it is not limited to a
    //     process exit. (After a COMPLETED wait the guard deliberately stands down — its `Drop` says why.)
    //   * The residual was a HARD end of ConvertIA itself (crash / power-loss / SIGKILL), where no Rust `Drop`
    //     runs: engine descendants could survive us — the posture §1.7 already accepts for POSIX
    //     ("POSIX orphans are reaped by re-parenting + the startup cleanup"), and the §2.6 startup sweep
    //     discards the previous run's owned temp either way.
    //   * **CLOSED ON WINDOWS BY P4.17 (§2.12.3 Leg B) — for a STILL-RUNNING engine, WHERE LEG B ATTACHED**
    //     (`attach_confined_job` can degrade to `None`, and the residual is then unchanged). The first-party Win32
    //     job the P4.10 forward note assigns to that box now exists: [`crate::platform::attach_confined_job`]
    //     creates ConvertIA's OWN job WITH `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (plus the memory /
    //     active-process / die-on-unhandled-exception caps) and assigns the still-suspended child to it in the
    //     `post_spawn` hook below — a Win8+ NESTED pair with `process-wrap`'s own limit-less job, so the
    //     P4.10/P4.11 group-kill + wait contract is untouched while the OS reaps the tree when our handle
    //     closes. The upstream `core`-is-empty defect is routed around, not depended on. **Arm split (§1.7's
    //     `[CORRECTED — P4.17]` note):** the limit is cleared ONLY on a clean completed wait — the same
    //     stand-down this guard makes, so a launcher-outlives-worker tree is never truncated — so the residual
    //     survives in exactly one shape: a host crash AFTER a clean engine exit. POSIX keeps its re-parenting
    //     posture.
    //
    // v1 uses the §1.7 `[REC]` FORCEFUL group-kill (no cooperative drain): the output lives on a §2.14 temp
    // path promoted only by the §2.1 atomic rename, so a hard kill leaves only a discardable temp artifact.
    // The `tauri_plugin_shell` sidecar kill path is deliberately NOT used (its `CommandChild::kill` is
    // tree-incomplete, and §0.10/§3.3.3 grant no `shell:allow-execute` at all) — the spawn+kill is pure Rust
    // here. [Build-Session-Entscheidung: P4.10] the per-OS `wrap` calls are `cfg`-gated rather than always
    // registered: each wrapper type only EXISTS on its own platform (`job-object` is a Windows-only feature,
    // `process-group` a POSIX-only one), so the gate is the crate's own shape, not a ConvertIA choice.
    let mut wrapped = group_wrapped(command);

    // §2.12.3 best-effort Windows privilege-drop tier (P4.17): register ConvertIA's own `CommandWrapper`
    // ALONGSIDE the P4.10 group-kill composition (never inside `group_wrapped`, which owns only the §1.7
    // whole-group shape). Its `post_spawn` hook is where BOTH Windows legs apply, and `process-wrap` runs every
    // `post_spawn` BEFORE any `wrap_child` — while `JobObject::wrap_child` is what resumes the `CREATE_SUSPENDED`
    // threads — so the child is still suspended there: parent-side, at creation time, before it runs one
    // instruction (the P4.16 forward note's constraint, satisfied literally). The shared cell hands the created
    // job back out here. This is the Windows peer of the Linux `install_confinement` call above; macOS attaches
    // nothing (P4.16). [Build-Session-Entscheidung: P4.17]
    #[cfg(windows)]
    let win_confinement =
        std::sync::Arc::new(std::sync::Mutex::new(WindowsConfinementOutcome::default()));
    #[cfg(windows)]
    wrapped.wrap(WindowsConfinement {
        lower_to,
        outcome: std::sync::Arc::clone(&win_confinement),
    });

    let spawned = match wrapped.spawn() {
        Ok(child) => child,
        // Spawn error (binary missing / denied) is the §2.13.1 ITEM-level fault: a runtime per-item spawn
        // failure fails that one item as InternalError (§2.13.2) — the final answer at this per-item level
        // (P4.7-resolved). The app-level EngineMissing/BundleDamaged split is the §7.2.3 startup probe's, not
        // this path (a mid-run vanished binary fails the item; the next startup probe catches a broken bundle).
        Err(_) => return ConfinedRun::failed(ConversionErrorKind::InternalError),
    };
    // From here on the child is owned by the guard, so no way out of this fn that ends the invocation WITHOUT
    // a completed engine wait — an early return, a panic, or the caller dropping the whole future — can leave
    // the engine's process tree running (§1.7 P4.10). After a COMPLETED wait the guard deliberately stands
    // down; its `Drop` carries that decision and the reason.
    let mut child = GroupKillGuard::new(spawned);

    // §2.12.3 Leg B (P4.17): move ConvertIA's own kill-on-job-close job OUT of the hand-back cell and INTO the
    // guard, so exactly one owner decides its fate on exactly the arms the guard already discriminates — the
    // clean-exit stand-down and the crash-arm backstop live together, and the handle closes when the guard
    // drops. A `None` here is the silent degrade (nested jobs unavailable, the assign refused): the cheap-tier
    // floor plus `process-wrap`'s own job, exactly as before this box. [Build-Session-Entscheidung: P4.17]
    #[cfg(windows)]
    {
        child.job = win_confinement
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .job
            .take();
    }

    // §2.12.3 achieved-tier record (P4.18): read the per-leg verdicts ONCE, here — the first point where a
    // child exists and every leg has had its apply window — and thread the same record onto whichever arm
    // the invocation ends on below. This is the production READ of the P4.17 hand-back cell's `integrity`
    // verdict and of the P4.15 host probes; the durable projection is the platform's row in the tracked
    // `privilege-drop-coverage.toml` the G64 ratchet guards, and the per-spawn assertion is P4.18.1's.
    //
    // The verdict SOURCE differs PER LEG because the apply point does (crate::platform::VERDICT_SOURCES is
    // the record of which is which): Windows applied both legs parent-side on the still-suspended child, so
    // each has a real per-spawn read-back — Leg A the `GetTokenInformation` re-read `lower_child_to` already
    // performed, Leg B simply whether ConvertIA's own job attached (a `None` there IS the silent degrade).
    // The two are recorded SEPARATELY, never collapsed: a FAT/exFAT or SMB destination legitimately leaves
    // Leg B applied while Leg A degraded, and one tier value could not say so. On Linux the net-namespace
    // leg is likewise read off the RUNNING CHILD (`/proc/<pid>/ns/net` vs ours), while Landlock and seccomp
    // apply inside the pre-exec child with no channel back and are therefore host-capability readings;
    // macOS records nothing (P4.16 — no leg exists). [Build-Session-Entscheidung: P4.18]
    #[cfg(windows)]
    let spawn_tier = {
        let mut tier = crate::platform::SpawnTier::default();
        let integrity = win_confinement
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .integrity;
        tier.record(
            crate::platform::LEG_INTEGRITY,
            integrity.unwrap_or(crate::platform::LegOutcome::Degraded(
                // No verdict at all means the label-then-lower GRANT was never issued for this spawn (a
                // sink that could not be labelled), so the leg never reached its apply point — the
                // `Unavailable` class, not a refused apply.
                crate::platform::DegradeReason::Unavailable,
            )),
        );
        tier.record(
            crate::platform::LEG_JOB,
            if child.job.is_some() {
                crate::platform::LegOutcome::Applied
            } else {
                crate::platform::LegOutcome::Degraded(crate::platform::DegradeReason::Unavailable)
            },
        );
        tier
    };
    // The pid is read while the child is still owned by the guard and has not been waited on, which is the
    // only window in which `/proc/<pid>/ns/net` is guaranteed to exist; a child that raced us to exit yields
    // the honest `Degraded(Unavailable)` rather than a guess.
    #[cfg(target_os = "linux")]
    let spawn_tier = crate::platform::spawn_leg_verdicts(child.inner.id());
    #[cfg(not(any(target_os = "linux", windows)))]
    let spawn_tier = crate::platform::SpawnTier::default();

    // Take the piped handles OUT so the two drains borrow THEM (owned) while `wait()` borrows the child —
    // all three run CONCURRENTLY under one `tokio::join!` on this task, so a full stdout/stderr pipe can never
    // back-pressure the child into a deadlock (the classic "wait without draining" hang). The whole join runs
    // under the §0.4.4 cancel token: a cancel trip drops the future (freeing the borrows) and the arm below
    // group-kills.
    let child_stdout = child.inner.stdout().take();
    let child_stderr = child.inner.stderr().take();

    let captured = invocation
        .cancel
        .run_until_cancelled(async {
            let stdout_fut = drain_stdout(
                child_stdout,
                line_read_stdout,
                &invocation.plan.progress,
                &on_progress,
            );
            let stderr_fut = read_all(child_stderr);
            tokio::join!(child.inner.wait(), stdout_fut, stderr_fut)
        })
        .await;

    match captured {
        Some((Ok(status), stdout_buf, stderr_buf)) => {
            // The engine ran to completion and `wait()` returned. Whether the guard stands down depends on the
            // exit status (P4.12 — the crash-vs-clean decision the P4.10 forward note delegated here):
            let result = if status.success() {
                // A CLEAN completed exit: the invocation ended through its own normal arm, so the guard stands
                // down (see its `Drop` for why a post-exit group-kill of a SUCCESSFUL run would be a correctness
                // regression — a launcher that legitimately exits before its worker finishes writing valid
                // output must not be truncated, §1.7 936-945).
                child.group_settled = true;
                // §2.12.3 Leg B (P4.17): the CLEAN arm is the ONLY one that stands ConvertIA's own Job Object
                // down (clear `KILL_ON_JOB_CLOSE`, keep the caps). The crash / reap-fault / cancel arms
                // deliberately leave it ARMED so a ConvertIA host crash still reaps the tree — the P4.10
                // crash-time-reap residual closed on exactly the arms where the tree should die. A separate flag
                // from `group_settled` on purpose: that one is ALSO set on the cancel arm, where the job must
                // stay armed. [Build-Session-Entscheidung: P4.17]
                #[cfg(windows)]
                {
                    child.clean_exit = true;
                }
                InvocationResult::Succeeded
            } else {
                // A CRASH completed exit (non-zero): the item is `Failed` → its `out_tmp` is discarded, so there
                // is NO valid output to preserve. Leave `group_settled` FALSE so [`GroupKillGuard`]'s Drop
                // group-kills any descendant that outlived the crashed launcher (e.g. a `soffice.bin` left
                // running by a `soffice` error exit) — otherwise a pure process leak (+ on Windows a
                // `*.part`-handle holder that would spuriously fail the conductor's cleanup into a
                // `CleanupResidue`). The §1.7 936-945 stand-down rationale is success-specific ("publishing a
                // corrupt output as a clean one"), so it does not apply to a crash. (The Drop's POSIX `killpg`
                // on this arm accepts the microsecond pgid-recycle window the success arm avoids — negligible
                // next to the leaked-worker cost; on Windows the held Job-Object handle makes the kill
                // non-speculative regardless.) The value stays the §2.12.1
                // reap PRE-CLASSIFICATION FLOOR (`EngineCrash`) — `run_subprocess`'s `classify_exit` (P4.12)
                // refines it via the §3.5 per-engine `classify_failure` over `stderr_buf`, keyed on the raw
                // `status` surfaced in `ConfinedRun::exit` below. [Build-Session-Entscheidung: P4.12]
                InvocationResult::Failed(ConversionErrorKind::EngineCrash)
            };
            ConfinedRun {
                result,
                stdout: stdout_buf,
                stderr: stderr_buf,
                // The raw completed-wait exit status — `run_subprocess`/`run_probe_then_encode` `classify_exit`
                // consume it for the §3.5 `classify_failure(exit, stderr)` seam (P4.12). `Some` on both the
                // clean and the crash arm; the non-completed arms (`ConfinedRun::failed`/`cancelled`) carry `None`.
                exit: Some(status),
                // The §2.12.3 achieved-tier record of THIS spawn (P4.18) — read above, right after the child
                // existed. Present on every arm that really spawned, including the crash arm.
                tier: Some(spawn_tier),
            }
        }
        // The reap itself failed — an internal fault, never a panic (the crate no-panic policy). `group_settled`
        // stays FALSE, so the guard group-kills on the way out: a failed reap must not leave the tree running.
        Some((Err(_), _, _)) => {
            ConfinedRun::failed(ConversionErrorKind::InternalError).with_tier(spawn_tier)
        }
        None => {
            // User cancel → the §1.7 step-2 GROUP-kill (P4.10): `start_kill` signals the whole process group
            // (`killpg(pgid, SIGKILL)`) / terminates the whole Job Object, so the engine AND every descendant
            // it spawned die — never an orphan holding the temp file open. SIGKILL / `TerminateJobObject` are
            // not refusable, so the kill itself needs no await; the flag records an OBSERVED delivery, never an
            // assumed one (if `start_kill` errored, the [`GroupKillGuard`] still gets its turn on the way out —
            // both kills are idempotent). [Build-Session-Entscheidung: P4.10]
            child.group_settled = child.inner.start_kill().is_ok();

            // §1.7 step 2's TIMEOUT-BOUNDED confirm-wait (P4.11): after the kill, wait UP TO
            // [`GROUP_CONFIRM_WAIT`] for the OS to reap the group, THEN return regardless. This is the
            // load-bearing half of step 2 — on Windows an open descendant handle blocks the `*.part` deletion,
            // so this window is what lets the tier-1 conductor's subsequent single removal (§1.7 step 3)
            // succeed on the NORMAL cancel; the bound is what stops a wedged descendant from hanging the §5.8
            // cancel round-trip / §7.3.3 quit (SSOT *app stays responsive*). Per the P4.10 forward note the
            // `wait()` does NOT prove the group empty on either platform — so the return value is DELIBERATELY
            // ignored: settled-within-`T` and timed-out are treated identically here (the runner returns a bare
            // `Cancelled` either way). The honest settle/wedge verdict is the conductor's own removal
            // success/failure (§2.6.4 "single bounded attempt"): a still-held `*.part` fails the removal → a
            // §2.6.4-case-3 `CleanupResidue` reclaimed by the §2.6.3 sweep; a released one removes clean. On
            // timeout the dropped `wait()` future may leave `process-wrap`'s blocking group-reaper running
            // detached — bounded and accepted exactly like the §1.7 InProcessNative wedged-read case (the
            // `spawn_blocking` pool has headroom above the §0.9 degree). NOTE the confirm-wait lives ONLY on
            // this cancel arm. A CLEAN completed exit (the `Some((Ok(status)…))` arm above, `status.success()`)
            // performs no kill and must NOT get a confirm-wait, or it would truncate a launcher-exits-early
            // worker (the exact regression [`GroupKillGuard`]'s Drop stand-down exists to prevent). A CRASH
            // completed exit DOES group-kill (P4.12 leaves `group_settled` false → the Drop backstop) but gets
            // no confirm-wait here either: its output is discarded, so the tier-1 conductor's own §2.6.4
            // single-attempt cleanup surfaces any still-held-handle honestly as a `CleanupResidue` — no
            // blocking wait needed on the doomed tree. [Build-Session-Entscheidung: P4.11]
            tokio::time::timeout(GROUP_CONFIRM_WAIT, child.inner.wait())
                .await
                .ok();
            ConfinedRun::cancelled().with_tier(spawn_tier)
        }
    }
}

/// Compose the §1.7 whole-group wrappers (P4.10) over an already-configured `tokio::process::Command` — the
/// ONE place the Job-Object / process-group / kill-on-drop composition is spelled out, so the production
/// spawn and the [`GroupKillGuard`] tests exercise the SAME wrapping rather than two drifting copies.
/// [Build-Session-Entscheidung: P4.10]
fn group_wrapped(command: Command) -> CommandWrap {
    let mut wrapped = CommandWrap::from(command);
    wrapped.wrap(KillOnDrop);
    #[cfg(windows)]
    wrapped.wrap(JobObject);
    #[cfg(unix)]
    wrapped.wrap(ProcessGroup::leader());
    wrapped
}

/// What the §2.12.3 Windows tier (P4.17) achieved for ONE spawn, handed back out of the `post_spawn` hook.
/// `job` is moved into [`GroupKillGuard`] straight after the spawn (it owns the teardown decision);
/// `integrity` is the Leg-A read-back the P4.18 achieved-tier record consumes — [`run_confined`] folds both
/// into the spawn's [`crate::platform::SpawnTier`] right after the move, as the two SEPARATE per-leg
/// verdicts the G64 record keeps un-collapsed.
/// [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
#[derive(Debug, Default)]
struct WindowsConfinementOutcome {
    /// ConvertIA's own kill-on-job-close job (§2.12.3 Leg B), or `None` when the leg degraded.
    job: Option<crate::platform::ConfinedJob>,
    /// The Leg-A per-spawn verdict — `None` when the label-then-lower grant was not issued at all (a
    /// non-persistent-ACL sink, a blocked engine label), otherwise the `GetTokenInformation` read-back's
    /// applied-vs-degraded outcome.
    ///
    /// Read in production by [`run_confined`] (P4.18) into the spawn's `SpawnTier` — the achieved-tier
    /// record the G64 `privilege-drop-coverage.toml` ratchet is the durable projection of. `None` there is
    /// NOT "applied": it means the grant was never issued, which the record maps to
    /// `Degraded(Unavailable)`.
    integrity: Option<crate::platform::LegOutcome>,
}

/// The §2.12.3 Windows privilege-drop `CommandWrapper` (P4.17) — the parent-side, pre-resume apply point for
/// BOTH Windows legs. `process-wrap` runs every `post_spawn` BEFORE any `wrap_child`, and its own
/// `JobObject::wrap_child` is what resumes the `CREATE_SUSPENDED` threads, so this hook is the exact window in
/// which the child exists but has not run: a restricted-token-equivalent adjustment is legal there and a job
/// assignment covers the process before it can spawn anything.
///
/// **It MUST NOT return `Err`.** A `post_spawn` error propagates with `?` out of `spawn_inner` BEFORE any
/// `wrap_child` runs, so the `JobObject` wrapper never resumes the threads: the child would be dropped
/// suspended and unreaped — a stranded orphan AND a broken conversion. Every failure inside is therefore a
/// silent degrade to the P4.13 cheap-tier floor, pinned by the two red-green tests
/// `a_failing_confinement_step_still_yields_a_resumed_completing_child` (the grant leg never issued) and
/// `a_refused_token_lowering_still_yields_a_resumed_completing_child` (the risky call itself refused).
/// [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
#[derive(Debug)]
struct WindowsConfinement {
    /// The mandatory integrity level to lower the child's token to, or `None` when Leg A's label-then-lower
    /// grant did not succeed for every sink — then Leg A is skipped for this spawn and Leg B still applies
    /// (the two legs are independent). Production always passes
    /// `Some(crate::platform::CONFINED_INTEGRITY_RID)`; carrying the level as DATA rather than hardcoding it
    /// here is what lets the tier tests stand a co-tenant up at a different level and prove the enforcement
    /// claim from the other side.
    lower_to: Option<u32>,
    /// The hand-back cell `run_confined` reads after the spawn.
    outcome: std::sync::Arc<std::sync::Mutex<WindowsConfinementOutcome>>,
}

#[cfg(windows)]
impl process_wrap::tokio::CommandWrapper for WindowsConfinement {
    fn post_spawn(
        &mut self,
        _command: &mut Command,
        child: &mut tokio::process::Child,
        _core: &CommandWrap,
    ) -> std::io::Result<()> {
        // Every arm returns `Ok(())` — see the type doc: an `Err` here strands a suspended child.
        if let Some(pid) = child.id() {
            let job = crate::platform::attach_confined_job(pid);
            let integrity = self
                .lower_to
                .map(|rid| crate::platform::lower_child_to(pid, rid));
            let mut slot = self
                .outcome
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            slot.job = job;
            slot.integrity = integrity;
        }
        Ok(())
    }
}

/// The §1.7 whole-group kill BACKSTOP (P4.10) — owns the spawned child so that **no way out of
/// [`run_confined`] that ends the invocation WITHOUT a completed engine wait** can leave the engine's process
/// tree running. That covers the failed-reap arm, any early return, and above all the exit no explicit arm can
/// reach: the caller **dropping the whole future** (the P4.12 no-progress watchdog, the §7.3.3
/// quit-while-converting path). `process-wrap`'s `KillOnDrop` shim cannot serve as this backstop — it sets
/// tokio's kill-on-drop, which kills only the IMMEDIATE child, and the Job Object's kill-on-job-close limit it
/// is supposed to switch on is unreachable in 9.1.0 (the `core`-is-empty defect documented in `run_confined`).
/// [Build-Session-Entscheidung: P4.10]
struct GroupKillGuard {
    inner: Box<dyn ChildWrapper>,
    /// `true` once the invocation reached a terminal state that must NOT be backstopped by a group-kill on drop.
    /// `run_confined` sets it in exactly two places: a **CLEAN** completed engine wait (`wait()` returned `Ok`
    /// AND `status.success()` — P4.12; killing there would truncate a launcher-outlives-worker's valid output),
    /// and the cancel arm (a group kill was already delivered). Left **FALSE** on a **crash** completed exit
    /// (non-zero — P4.12 wants the doomed tree killed), a failed reap, an early return, and the caller dropping
    /// the whole future — all paths the [`Drop`] backstop group-kills.
    group_settled: bool,
    /// `true` ONLY on a clean completed exit (§2.12.3 Leg B, P4.17) — the one arm on which ConvertIA's own
    /// Job Object stands down. Distinct from `group_settled`, which is ALSO set on the cancel arm, where the
    /// job must stay armed.
    #[cfg(windows)]
    clean_exit: bool,
    /// ConvertIA's own `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` job around this spawn (§2.12.3 Leg B, P4.17), or
    /// `None` when the leg degraded. Owned HERE so its teardown decision rides the same clean-vs-crash split
    /// the group-kill backstop already makes: on the clean arm it is stood down (flag cleared, caps kept), on
    /// every other arm it stays armed and the handle's close reaps the tree — the P4.10 crash-time residual.
    #[cfg(windows)]
    job: Option<crate::platform::ConfinedJob>,
}

impl GroupKillGuard {
    fn new(inner: Box<dyn ChildWrapper>) -> Self {
        Self {
            inner,
            group_settled: false,
            #[cfg(windows)]
            clean_exit: false,
            #[cfg(windows)]
            job: None,
        }
    }
}

impl Drop for GroupKillGuard {
    fn drop(&mut self) {
        // Best-effort and never panicking (the crate no-panic policy); `start_kill` is `killpg(pgid, SIGKILL)`
        // on POSIX and `TerminateJobObject` on Windows — both tear down the WHOLE group, which is the point.
        //
        // The guard fires on drop iff `group_settled` is false. `run_confined` sets it (P4.10 / P4.12):
        //   - a CLEAN completed wait (`wait()` returned `Ok` AND `status.success()`) → `group_settled = true`,
        //     stand down. Neither platform's `wait()` proves the group empty (POSIX `waitpid(-pgid)` → `ECHILD`
        //     only means WE hold no children — a grandchild was never our child; `JobObjectChild::wait` returns
        //     on the FIRST completion-port message, not on `JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO`), so a post-exit
        //     kill here would be speculative AND, worse, a CORRECTNESS regression: for an engine whose launcher
        //     legitimately exits before its worker has finished writing (the `soffice` → `soffice.bin` shape
        //     this box family exists for), it would truncate valid in-flight output and publish a corrupt file
        //     as a clean success. So a SUCCESSFUL run stands down; a descendant outliving it is left to the §1.7
        //     app-exit group-kill and the §2.6 sweep. On POSIX standing down also keeps a stale `killpg` off a
        //     pgid the OS may already have recycled.
        //   - a CRASH completed wait (non-zero exit) → `group_settled` stays FALSE, so the guard DOES fire
        //     (P4.12, the crash-vs-clean decision the P4.10 forward note delegated): the item is `Failed` and
        //     its output discarded, so there is no valid work to truncate — the correctness objection above is
        //     success-specific — and a descendant outliving the crashed launcher is a pure leak (+ a Windows
        //     temp-handle holder that would spuriously fail cleanup). Killing the doomed tree is the clean win.
        //   - a cancel / failed reap / early return / the caller DROPPING the whole future (the P4.12 no-progress
        //     watchdog, the §7.3.3 quit path) → `group_settled` false → the guard is the backstop that tears the
        //     tree down. [Build-Session-Entscheidung: P4.10 / P4.12]
        if !self.group_settled {
            self.inner.start_kill().ok();
        }
        // §2.12.3 Leg B (P4.17) — ConvertIA's OWN job, torn down on the SAME clean-vs-crash split. On the CLEAN
        // arm it is stood down (kill-on-job-close cleared, the caps kept) so the close below cannot truncate the
        // launcher-outlives-worker tree the stand-down above exists to protect. On every other arm the flag is
        // left ARMED, so dropping the handle here — and, crucially, the OS closing it if ConvertIA itself is
        // killed — reaps the whole engine tree: the P4.10 crash-time-reap residual `process-wrap` 9.1.0 cannot
        // deliver. The order matters: the explicit group-kill above runs first, this is the OS-level backstop
        // behind it. [Build-Session-Entscheidung: P4.17]
        #[cfg(windows)]
        if let Some(job) = self.job.take() {
            if self.clean_exit {
                job.stand_down();
            }
        }
    }
}

/// Drain a confined child's stdout per the §1.7 [`ProgressModel`] (P4.8). For a **streaming** model
/// (`line_read == true`, i.e. `FfmpegKeyValue` / `VipsStdout`) it reads stdout **line by line** and feeds
/// each parsed `0.0..=1.0` fraction to `on_progress`, returning an EMPTY buffer (the lines were consumed as
/// progress, never retained). For `CoarseSpawnDone` (`line_read == false`) it **buffers stdout in full** with
/// NO line reader (a line reader would fragment the single-JSON-blob probe output, §1.7) and returns the
/// buffer for the P4.9 probe parse. A `None` handle or a read error ends the drain best-effort — progress is
/// advisory and the exit code is authoritative, so a broken pipe never panics. [Build-Session-Entscheidung: P4.8]
async fn drain_stdout(
    stdout: Option<ChildStdout>,
    line_read: bool,
    progress: &ProgressModel,
    on_progress: &impl Fn(f32),
) -> Vec<u8> {
    let Some(stdout) = stdout else {
        return Vec::new();
    };
    if line_read {
        // Read stdout as BYTES per line (`read_until(b'\n')`, lossy-decoded) rather than a `Lines` reader: a
        // `Lines` reader ERRORS on a non-UTF-8 byte, which would abandon the drain early and re-open the very
        // pipe-back-pressure deadlock this concurrent drain exists to close. `read_until` never decode-errs
        // (it reads raw bytes; `Ok(0)` = EOF), so the drain always runs to EOF regardless of content. The v1
        // streaming wires are engine-generated ASCII, but keeping the drain total is free robustness.
        let mut reader = BufReader::new(stdout);
        let mut raw = Vec::new();
        loop {
            raw.clear();
            match reader.read_until(b'\n', &mut raw).await {
                // EOF, or a terminal read error (the child's pipe is gone anyway) — stop draining.
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if let Some(fraction) =
                        progress.progress_fraction(String::from_utf8_lossy(&raw).trim_end())
                    {
                        on_progress(fraction);
                    }
                }
            }
        }
        Vec::new()
    } else {
        read_bytes(stdout).await
    }
}

/// Read a confined child's stderr (P4.8) — captured **in full** for the P4.12 exit-classification / §7.5 echo
/// / §2.13 classify (§1.7). A `None` handle (never piped / already taken) or a read error yields an empty
/// buffer, never a panic. [Build-Session-Entscheidung: P4.8]
async fn read_all(stream: Option<ChildStderr>) -> Vec<u8> {
    match stream {
        Some(stream) => read_bytes(stream).await,
        None => Vec::new(),
    }
}

/// Read an async byte stream to end, best-effort (a read error stops at the bytes captured so far — never a
/// panic; the confined child's exit status is the authoritative signal).
///
/// **Capture bound (T10) — a decided residual, owned by no specific box.** The read is `read_to_end`
/// (unbounded): this is the box-mandated "capture in full" (the `CoarseSpawnDone` probe JSON must arrive
/// whole; stderr must be complete for the P4.12 classify). For v1 the bundled engines' diagnostic volume is
/// bounded in practice by their own §3.5 argv log controls (e.g. FFmpeg `-loglevel error`), so a crafted input
/// cannot realistically flood these buffers. The §0.11 **T10** memory class ("never OOM-crash →
/// `Failed(TooBig)`") is real, but a review of §1.10 confirms its ceilings do not cover THIS vector: §1.10
/// governs the OUTPUT/SCRATCH **disk** budget (`est_output_bytes`/`est_scratch_bytes`, P4.72/P4.73/P9.41) and
/// the **engine process's own** memory (the §2.12.3 Job-Object cap) — not `convertia-core`'s OWN heap growth
/// from draining a child's pipe. So the core-side captured-byte ceiling (a bounded read cap on this drain) is
/// an OPEN concern owned by no scheduled box — escalated as a Co-Pilot item for the §0.11 threat-map assembly
/// (P4) / the P4.12 touch of this fn, recorded here so the unbounded read is an explicit residual, never a
/// silent gap. (Sibling: `drain_stdout`'s reused per-line buffer retains peak capacity across a drain — same
/// class, same cap.) [Build-Session-Entscheidung: P4.8]
async fn read_bytes<R: AsyncRead + Unpin>(mut reader: R) -> Vec<u8> {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).await.ok();
    buf
}

// §6.4.1/§6.4.2 (G15): the §2.12.3 cheap-tier floor exercised against a REAL subprocess + a REAL temp
// filesystem — the isolation LAYER is never mocked (test-strategy §0.1). The child is the platform shell
// at its ABSOLUTE System32//bin path (PATH is never relied on — the confined env has none).
//
// [Test-Change: P4.8 — old-obsolete+new-correct, §1.7 §2.12.3] the P4.13 asserts read `run_confined(..).await`
// directly against `InvocationResult`; P4.8 changed the return type to `ConfinedRun` (the stdout/stderr
// capture) + added the `on_progress` param, so those asserts now read `run_confined(.., |_| {}).await.result`
// — the old expectation (a bare `InvocationResult` return) is obsolete, the new one is correct
// (`ConfinedRun::result` IS the prior `InvocationResult`, verified field-for-field); the outcomes asserted are
// unchanged. The P4.8 progress-tick + stdout-buffer + stderr-capture behaviour is proven by the NEW tests
// below (real subprocess emitting synthetic progress lines → the captured `ConfinedRun`).
#[cfg(test)]
mod confined_spawn_tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use tokio_util::sync::CancellationToken;

    use crate::domain::JobId;
    use crate::engines::{EngineId, EngineProgram, Invocation, ProgressModel};

    // The absolute platform shell + its arg prefix. Windows: %SystemRoot%\System32\cmd.exe with /d
    // (skip registry AutoRun — a host's AutoRun must not leak into the confined child) /c; Unix: /bin/sh -c.
    #[cfg(windows)]
    fn shell() -> (PathBuf, Vec<OsString>) {
        let system_root = std::env::var_os("SystemRoot").expect("SystemRoot is set on Windows");
        let mut cmd = PathBuf::from(system_root);
        cmd.push("System32");
        cmd.push("cmd.exe");
        (cmd, vec![OsString::from("/d"), OsString::from("/c")])
    }
    #[cfg(unix)]
    fn shell() -> (PathBuf, Vec<OsString>) {
        (PathBuf::from("/bin/sh"), vec![OsString::from("-c")])
    }

    // The minimal env the test child NEEDS (§2.12.3(b): "cleared env except what the engine needs"):
    // cmd.exe needs SystemRoot to run reliably on Windows; /bin/sh needs nothing. NOT an inherited leak —
    // the env-cleared assertion below proves the parent's own vars never reach the child.
    fn minimal_env() -> Vec<(OsString, OsString)> {
        #[cfg(windows)]
        {
            vec![(
                OsString::from("SystemRoot"),
                std::env::var_os("SystemRoot").expect("SystemRoot is set on Windows"),
            )]
        }
        #[cfg(unix)]
        {
            Vec::new()
        }
    }

    // A confined envelope running `script` through the platform shell in `cwd` under `progress`, returning the
    // envelope + the resolved absolute program path run_confined takes (the P4.32 seam, caller-supplied).
    fn confined_shell_invocation_with_progress(
        script: &str,
        cwd: Option<PathBuf>,
        progress: ProgressModel,
    ) -> (EngineInvocation, PathBuf) {
        let (program, mut args) = shell();
        args.push(OsString::from(script));
        let envelope = EngineInvocation {
            job: JobId::from_index(0),
            engine: EngineId::Pandoc,
            plan: Invocation {
                program: EngineProgram::Sidecar(EngineId::Pandoc),
                args,
                cwd,
                env: minimal_env(),
                stdin: StdinPlan::None,
                progress,
                out_tmp: None,
            },
            cancel: CancellationToken::new(),
        };
        (envelope, program)
    }

    // The exit/env/cancel tests do not exercise progress — they run under the coarse spawn→done model.
    fn confined_shell_invocation(
        script: &str,
        cwd: Option<PathBuf>,
    ) -> (EngineInvocation, PathBuf) {
        confined_shell_invocation_with_progress(script, cwd, ProgressModel::CoarseSpawnDone)
    }

    #[cfg(windows)]
    const EXIT_ZERO: &str = "exit 0";
    #[cfg(unix)]
    const EXIT_ZERO: &str = "exit 0";
    #[cfg(windows)]
    const EXIT_THREE: &str = "exit 3";
    #[cfg(unix)]
    const EXIT_THREE: &str = "exit 3";

    // §2.12.1 (G15): a clean exit maps to Succeeded; a nonzero exit to the §2.8 EngineCrash
    // pre-classification floor (P4.12 routes it through classify_failure for the precise kind).
    #[tokio::test]
    async fn a_clean_exit_maps_to_succeeded_and_a_nonzero_exit_to_engine_crash() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        let (ok, program) =
            confined_shell_invocation(EXIT_ZERO, Some(scratch.path().to_path_buf()));
        assert_eq!(
            run_confined(&ok, &program, |_| {}).await.result,
            InvocationResult::Succeeded
        );
        let (bad, program) =
            confined_shell_invocation(EXIT_THREE, Some(scratch.path().to_path_buf()));
        assert_eq!(
            run_confined(&bad, &program, |_| {}).await.result,
            InvocationResult::Failed(ConversionErrorKind::EngineCrash),
            "§2.12.1: a nonzero engine exit is the reap-mapped EngineCrash floor"
        );
    }

    // §2.12.3(a)+(b) (G15): the child runs IN the scratch cwd with a CLEARED env — the parent's own vars
    // (the CARGO_MANIFEST_DIR canary cargo-test always sets, and PATH) never reach it; the plan's
    // minimal pairs DO. Proven by the child itself writing its cwd + env into files inside the scratch.
    #[tokio::test]
    async fn the_child_runs_env_cleared_in_the_scratch_cwd() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        assert!(
            std::env::var_os("CARGO_MANIFEST_DIR").is_some(),
            "the canary parent var is set under cargo test"
        );
        #[cfg(windows)]
        let script = "cd > cwd.txt & set > env.txt";
        #[cfg(unix)]
        let script = "pwd > cwd.txt; env > env.txt";
        let (envelope, program) =
            confined_shell_invocation(script, Some(scratch.path().to_path_buf()));
        assert_eq!(
            run_confined(&envelope, &program, |_| {}).await.result,
            InvocationResult::Succeeded
        );
        let cwd_line = std::fs::read_to_string(scratch.path().join("cwd.txt"))
            .expect("the child wrote its cwd into the scratch dir — the scratch IS the cwd");
        let reported = std::fs::canonicalize(PathBuf::from(cwd_line.trim()))
            .expect("the child-reported cwd resolves");
        let expected = std::fs::canonicalize(scratch.path()).expect("the scratch dir resolves");
        assert_eq!(
            reported, expected,
            "§2.12.3(a): the working dir is the scratch dir"
        );
        let env_dump = std::fs::read_to_string(scratch.path().join("env.txt"))
            .expect("the child wrote its env into the scratch dir");
        assert!(
            !env_dump
                .lines()
                .any(|line| line.to_ascii_lowercase().starts_with("cargo_manifest_dir=")),
            "§2.12.3(b): the parent's canary var never reaches the confined child"
        );
        // §2.12.3(b): the parent's PATH never leaks. Windows cmd.exe leaves the cleared env with NO
        // PATH at all; POSIX `/bin/sh` unconditionally re-seeds a default PATH of its OWN (e.g.
        // `/usr/bin:/bin`) that is never the parent's — so on unix we prove the child's PATH is not the
        // inherited value rather than asserting PATH is absent (which sh's self-seed would falsely fail).
        // (absolute bundled paths only, §3.3.3)
        #[cfg(windows)]
        assert!(
            !env_dump
                .lines()
                .any(|line| line.to_ascii_lowercase().starts_with("path=")),
            "§2.12.3(b): the inherited PATH never reaches the confined child (absolute paths only, §3.3.3)"
        );
        #[cfg(unix)]
        {
            let parent_path = std::env::var("PATH").unwrap_or_default();
            let child_path = env_dump
                .lines()
                .find_map(|line| line.strip_prefix("PATH="))
                .unwrap_or_default();
            assert_ne!(
                child_path, parent_path,
                "§2.12.3(b): the parent's PATH never reaches the confined child; /bin/sh's self-seeded default is not the inherited value (absolute bundled paths only, §3.3.3)"
            );
        }
        #[cfg(windows)]
        assert!(
            env_dump
                .lines()
                .any(|line| line.to_ascii_lowercase().starts_with("systemroot=")),
            "§2.12.3(b): the plan's own minimal pairs DO reach the child"
        );
    }

    // §3.5/§2.12.3 (G15, P4.14): is_loader_injection_var matches EXACTLY the four dynamic-loader injection
    // vars, case-sensitively — a legit var (PATH/HOME/SystemRoot) or a P5 per-engine whitelist var
    // (LIBHEIF_PLUGIN_PATH/VIPS_BLOCK_UNTRUSTED) is NOT stripped, and a lowercase near-miss (`ld_preload`) does
    // not match (the POSIX loaders these target read case-sensitive env-var names).
    #[test]
    fn is_loader_injection_var_matches_exactly_the_four_loader_vars() {
        for var in [
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "DYLD_INSERT_LIBRARIES",
            "DYLD_LIBRARY_PATH",
        ] {
            assert!(
                is_loader_injection_var(std::ffi::OsStr::new(var)),
                "§3.5: {var} is a stripped dynamic-loader injection var (T3a)"
            );
        }
        for var in [
            "PATH",
            "HOME",
            "SystemRoot",
            "LIBHEIF_PLUGIN_PATH",
            "VIPS_BLOCK_UNTRUSTED",
            "ld_preload",
        ] {
            assert!(
                !is_loader_injection_var(std::ffi::OsStr::new(var)),
                "{var} is NOT a loader injection var — a legit/whitelist var (or a case near-miss) must survive"
            );
        }
    }

    // §3.5/§2.12.3/§0.11 (G15, P4.14): the four dynamic-loader injection vars are STRIPPED from the constructed
    // engine env even when the plan env carries them — a real confined child dumps its environment and NONE of
    // the sentinel loader values appear, while a legitimate non-loader var survives (the strip is precise, not
    // a blanket wipe). A hostile input therefore cannot coerce a side-load (T3a). `env_clear` (tested above)
    // drops the INHERITED copy; this proves the CONSTRUCTED-env defense-in-depth over the plan env. A broken
    // strip would leave `LD_PRELOAD=evil-preload` in the child env and the sentinel would appear.
    #[tokio::test]
    async fn the_loader_injection_vars_are_stripped_from_the_engine_env() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        #[cfg(windows)]
        let script = "set > env.txt";
        #[cfg(unix)]
        let script = "env > env.txt";
        let (mut envelope, program) = confined_shell_invocation_with_progress(
            script,
            Some(scratch.path().to_path_buf()),
            ProgressModel::CoarseSpawnDone,
        );
        // Inject all four loader-injection vars (distinct sentinel values) + a legit var that MUST survive.
        for (name, value) in [
            ("LD_PRELOAD", "evil-preload"),
            ("LD_LIBRARY_PATH", "evil-libpath"),
            ("DYLD_INSERT_LIBRARIES", "evil-dyldins"),
            ("DYLD_LIBRARY_PATH", "evil-dyldlib"),
            ("CONVERTIA_KEPT", "kept-value"),
        ] {
            envelope
                .plan
                .env
                .push((OsString::from(name), OsString::from(value)));
        }
        assert_eq!(
            run_confined(&envelope, &program, |_| {}).await.result,
            InvocationResult::Succeeded
        );
        let env_dump = std::fs::read_to_string(scratch.path().join("env.txt"))
            .expect("the child wrote its environment into the scratch dir");
        for sentinel in [
            "evil-preload",
            "evil-libpath",
            "evil-dyldins",
            "evil-dyldlib",
        ] {
            assert!(
                !env_dump.contains(sentinel),
                "§3.5/§2.12.3 (T3a): the loader-injection var carrying {sentinel:?} was STRIPPED from the \
                 engine env — a hostile input cannot coerce a side-load. Full env dump:\n{env_dump}"
            );
        }
        assert!(
            env_dump.contains("kept-value"),
            "the strip is PRECISE — a legitimate (non-loader) env var survives. Full env dump:\n{env_dump}"
        );
    }

    // §1.7/§0.4.4 (G15): a pre-tripped cancel token yields Cancelled — the child is killed best-effort
    // and never runs to completion (the busy-loop/sleeper would otherwise outlive the test bound).
    #[tokio::test]
    async fn a_pre_tripped_cancel_token_yields_cancelled() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        #[cfg(windows)]
        let script = "%SystemRoot%\\System32\\ping.exe -n 4 127.0.0.1 >nul";
        #[cfg(unix)]
        let script = "while :; do :; done";
        let (envelope, program) =
            confined_shell_invocation(script, Some(scratch.path().to_path_buf()));
        envelope.cancel.cancel();
        assert_eq!(
            run_confined(&envelope, &program, |_| {}).await.result,
            InvocationResult::Cancelled,
            "§1.7: a tripped cancel token reports Cancelled, never a fabricated success"
        );
    }

    // §2.13 (G15): a missing/unspawnable binary is a clean internal fault — no panic, no wedge.
    #[tokio::test]
    async fn a_missing_binary_is_a_clean_internal_error() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        let (envelope, _) =
            confined_shell_invocation(EXIT_ZERO, Some(scratch.path().to_path_buf()));
        let missing = scratch.path().join("no-such-engine-binary.exe");
        assert_eq!(
            run_confined(&envelope, &missing, |_| {}).await.result,
            InvocationResult::Failed(ConversionErrorKind::InternalError)
        );
    }

    // §3.5.4 (G15): the PipeBytes stdin plan is the honest unreachable-by-construction seam (P2.25) —
    // refused BEFORE any spawn (no pandoc adapter owns the byte feed before P7).
    #[tokio::test]
    async fn a_pipe_bytes_plan_is_the_honest_internal_error_seam() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        let (mut envelope, program) =
            confined_shell_invocation(EXIT_ZERO, Some(scratch.path().to_path_buf()));
        envelope.plan.stdin = StdinPlan::PipeBytes;
        assert_eq!(
            run_confined(&envelope, &program, |_| {}).await.result,
            InvocationResult::Failed(ConversionErrorKind::InternalError)
        );
    }

    // §2.12.3(a) (G15): a missing cwd on a confined spawn is a mis-built plan — refused BEFORE any
    // spawn (the scratch working dir is the floor's own mandate, never inherited).
    #[tokio::test]
    async fn a_missing_cwd_is_a_mis_built_plan() {
        let (envelope, program) = confined_shell_invocation(EXIT_ZERO, None);
        assert_eq!(
            run_confined(&envelope, &program, |_| {}).await.result,
            InvocationResult::Failed(ConversionErrorKind::InternalError)
        );
    }

    // ─── P4.8: the §1.7 per-`ProgressModel` stdout/stderr handling — over a REAL subprocess ───────────────
    //
    // Each test drives a real shell child that emits synthetic progress lines / a JSON blob / a stderr
    // diagnostic and asserts the captured `ConfinedRun` (the on_progress fractions, the buffered stdout, the
    // stderr-in-full) — the isolation layer is never mocked (test-strategy §0.1).

    // A progress sink capturing every fraction it receives (interior-mutable — `run_confined` takes `Fn`).
    fn capturing_sink() -> (Arc<Mutex<Vec<f32>>>, impl Fn(f32)) {
        let hits = Arc::new(Mutex::new(Vec::<f32>::new()));
        let sink = {
            let hits = hits.clone();
            move |fraction: f32| {
                hits.lock()
                    .expect("the progress mutex is not poisoned")
                    .push(fraction)
            }
        };
        (hits, sink)
    }

    // §1.7/§1.11 (G15): a FfmpegKeyValue streaming child's `key=value` stdout lines are read line-by-line and
    // parsed into `on_progress` fractions (out_time_us / duration_us; progress=end → 1.0); a line-read model
    // NEVER buffers stdout (the bytes were consumed as progress, so `ConfinedRun.stdout` is empty).
    #[tokio::test]
    async fn ffmpeg_key_value_stdout_lines_feed_progress_fractions() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        #[cfg(windows)]
        let script = "echo out_time_us=500000&echo progress=end";
        #[cfg(unix)]
        let script = "printf 'out_time_us=500000\\nprogress=end\\n'";
        let (envelope, program) = confined_shell_invocation_with_progress(
            script,
            Some(scratch.path().to_path_buf()),
            ProgressModel::FfmpegKeyValue {
                duration_us: 1_000_000,
            },
        );
        let (hits, sink) = capturing_sink();
        let run = run_confined(&envelope, &program, sink).await;
        assert_eq!(run.result, InvocationResult::Succeeded);
        let fractions = hits.lock().expect("the progress mutex is readable").clone();
        assert_eq!(
            fractions,
            vec![0.5_f32, 1.0_f32],
            "§1.11: out_time_us=500000 over duration_us=1_000_000 → 0.5, then progress=end → 1.0"
        );
        assert!(
            run.stdout.is_empty(),
            "§1.7: a line-read streaming model consumes stdout as progress and buffers nothing"
        );
    }

    // §1.7/§3.5.5 (G15): a VipsStdout streaming child's `progress=<0..100>` lines feed the SAME §1.7 line
    // reader as FFmpeg — progress=50 → 0.5, progress=end → 1.0.
    #[tokio::test]
    async fn vips_stdout_progress_lines_feed_the_same_line_reader() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        #[cfg(windows)]
        let script = "echo progress=50&echo progress=end";
        #[cfg(unix)]
        let script = "printf 'progress=50\\nprogress=end\\n'";
        let (envelope, program) = confined_shell_invocation_with_progress(
            script,
            Some(scratch.path().to_path_buf()),
            ProgressModel::VipsStdout,
        );
        let (hits, sink) = capturing_sink();
        let run = run_confined(&envelope, &program, sink).await;
        assert_eq!(run.result, InvocationResult::Succeeded);
        let fractions = hits.lock().expect("the progress mutex is readable").clone();
        assert_eq!(
            fractions,
            vec![0.5_f32, 1.0_f32],
            "§3.5.5: the image-worker's progress=<0..100> wire feeds the same §1.7 reader"
        );
    }

    // §1.7 (G15): a CoarseSpawnDone child's stdout is BUFFERED WHOLE (the ffprobe single-JSON-blob path) — NO
    // line reader is attached (so the blob is not fragmented) and NO progress fraction is emitted; the buffer
    // is surfaced in `ConfinedRun.stdout` for the P4.9 probe parse.
    #[tokio::test]
    async fn coarse_spawn_done_buffers_stdout_whole_and_emits_no_fraction() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        #[cfg(windows)]
        let script = "echo {\"streams\":[]}";
        #[cfg(unix)]
        let script = "printf '{\"streams\":[]}'";
        let (envelope, program) = confined_shell_invocation_with_progress(
            script,
            Some(scratch.path().to_path_buf()),
            ProgressModel::CoarseSpawnDone,
        );
        let (hits, sink) = capturing_sink();
        let run = run_confined(&envelope, &program, sink).await;
        assert_eq!(run.result, InvocationResult::Succeeded);
        assert!(
            String::from_utf8_lossy(&run.stdout).contains("streams"),
            "§1.7: a CoarseSpawnDone stdout is buffered in full for the P4.9 probe parse"
        );
        assert!(
            hits.lock()
                .expect("the progress mutex is readable")
                .is_empty(),
            "§1.7: no line reader is attached to a CoarseSpawnDone stdout — no fraction is emitted"
        );
    }

    // §1.7/§2.13 (G15): stderr is captured IN FULL for every subprocess model (the P4.12 classify / §7.5 echo
    // input), independent of the exit code — proven on a nonzero exit that also writes a diagnostic line.
    #[tokio::test]
    async fn stderr_is_captured_in_full_on_a_failing_exit() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        #[cfg(windows)]
        let script = "echo boom-diagnostic 1>&2&exit 3";
        #[cfg(unix)]
        let script = "printf 'boom-diagnostic\\n' 1>&2; exit 3";
        let (envelope, program) = confined_shell_invocation_with_progress(
            script,
            Some(scratch.path().to_path_buf()),
            ProgressModel::CoarseSpawnDone,
        );
        let run = run_confined(&envelope, &program, |_| {}).await;
        assert_eq!(
            run.result,
            InvocationResult::Failed(ConversionErrorKind::EngineCrash),
            "§2.12.1: the nonzero exit is the reap-mapped EngineCrash floor"
        );
        assert!(
            String::from_utf8_lossy(&run.stderr).contains("boom-diagnostic"),
            "§1.7: stderr is captured in full regardless of exit code (the P4.12 classify input)"
        );
    }

    // §1.7/§3.2.2 (G15): InProcessFraction is NOT a subprocess model — the native CSV/TSV engine self-reports
    // over the in-core mpsc lane (P3.43) and never routes through a confined spawn, so reaching run_confined
    // with it is a mis-wired plan → the honest InternalError seam (refused BEFORE any spawn, no fraction).
    #[tokio::test]
    async fn in_process_fraction_on_a_confined_spawn_is_the_mis_wired_seam() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        let (envelope, program) = confined_shell_invocation_with_progress(
            EXIT_ZERO,
            Some(scratch.path().to_path_buf()),
            ProgressModel::InProcessFraction,
        );
        let (hits, sink) = capturing_sink();
        let run = run_confined(&envelope, &program, sink).await;
        assert_eq!(
            run.result,
            InvocationResult::Failed(ConversionErrorKind::InternalError),
            "§1.7: an in-process progress model on a subprocess spawn is a mis-wired plan"
        );
        assert!(
            hits.lock()
                .expect("the progress mutex is readable")
                .is_empty(),
            "§1.7: the seam is refused before any spawn — no progress is emitted"
        );
    }

    // §1.7/§0.4.4 (G15, P4.8): a cancel arriving WHILE the concurrent drain is active (mid-stream) PROMPTLY
    // tears the child down to Cancelled — the P4.8 `tokio::join!` (stdout drain + stderr drain + `child.wait`)
    // runs under `run_until_cancelled`, so a cancel drops the whole join and kills the child without waiting
    // for it to exit (`run_confined` returns as soon as the token trips — measured ~105 ms, NOT the child's
    // lifetime). This is the NEW P4.8 path the pre-tripped-token tests don't reach: they never enter the join,
    // so they never exercise dropping the ACTIVE drains + wait on cancel. THIS test asserts only that
    // responsiveness half; the descendant-teardown half is the P4.10 group-kill test below.
    //
    // The shell blocks via a grandchild (`ping`/`sleep`). Since P4.10 the cancel arm group-kills, so the
    // grandchild dies WITH the shell — on Windows that also closes the stdout/stderr pipe handles the
    // grandchild INHERITED (std-stream redirection does not defeat Win32 handle inheritance), so the drain's
    // blocking read sees EOF at once and test teardown is immediate on every platform.
    #[tokio::test]
    async fn a_cancel_mid_drain_still_tears_the_child_down() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        // Emit one progress line, then block ~1 s (>> the 100 ms cancel) — so the cancel reliably lands while
        // the join is active (draining stdout + waiting on the child), never after the child has exited.
        #[cfg(windows)]
        let script = "echo progress=10& %SystemRoot%\\System32\\ping.exe -n 2 127.0.0.1 >nul 2>&1";
        #[cfg(unix)]
        let script = "printf 'progress=10\\n'; sleep 1 >/dev/null 2>&1";
        let (envelope, program) = confined_shell_invocation_with_progress(
            script,
            Some(scratch.path().to_path_buf()),
            ProgressModel::VipsStdout,
        );
        let token = envelope.cancel.clone();
        let canceller = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            token.cancel();
        });
        let run = run_confined(&envelope, &program, |_| {}).await;
        canceller.await.expect("the canceller task joins");
        assert_eq!(
            run.result,
            InvocationResult::Cancelled,
            "§1.7: a cancel during the active drain reports Cancelled, never a fabricated success"
        );
    }

    // §1.7 (G15, P4.8): a streaming child whose FINAL progress line has NO trailing newline is still parsed —
    // `read_until(b'\n')` returns the partial trailing bytes at EOF (before the next `Ok(0)`), so the last
    // observed fraction is not dropped (an engine that exits right after a partial write still yields its
    // last tick). Windows `<nul set /p=` prints the value with no CRLF; unix `printf` without `\n` likewise.
    #[tokio::test]
    async fn a_final_progress_line_without_a_trailing_newline_is_still_parsed() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        // `<nul set /p=` prints the value with NO trailing CRLF; it returns errorlevel 1 on the nul-EOF read,
        // so `& exit 0` forces the clean exit this test needs (the point is the no-newline output, not the code).
        #[cfg(windows)]
        let script = "<nul set /p=progress=75&exit 0";
        #[cfg(unix)]
        let script = "printf 'progress=75'";
        let (envelope, program) = confined_shell_invocation_with_progress(
            script,
            Some(scratch.path().to_path_buf()),
            ProgressModel::VipsStdout,
        );
        let (hits, sink) = capturing_sink();
        let run = run_confined(&envelope, &program, sink).await;
        assert_eq!(run.result, InvocationResult::Succeeded);
        let fractions = hits.lock().expect("the progress mutex is readable").clone();
        assert_eq!(
            fractions,
            vec![0.75_f32],
            "§1.7: a final line lacking a trailing newline still yields its fraction (read_until returns the partial at EOF)"
        );
    }

    // ─── P4.10: the §1.7 whole-group / job-object teardown — over a REAL process TREE ─────────────────────
    //
    // The load-bearing §1.7 guarantee: ONE kill tears down the engine AND ALL ITS DESCENDANTS. The engine that
    // motivates it is LibreOffice (`soffice` re-execs `soffice.bin`), so the fixture reproduces exactly that
    // shape — a confined child that itself runs a longer-lived DESCENDANT — and both tests below prove the
    // descendant is reaped, not orphaned, on the two paths that reach the kill: an explicit CANCEL, and the
    // caller DROPPING the whole `run_confined` future (the `GroupKillGuard` backstop, which carries the
    // teardown that `process-wrap`'s inert kill-on-job-close limit cannot). Pre-P4.10 (direct-child kill only)
    // the cancel test FAILS: the orphan survives and writes its late marker.
    // [Build-Session-Entscheidung: P4.10] the marker-file design is what makes the assertion OS-portable
    // without a process-enumeration dependency (`remoteprocess`/`sysinfo` would be a new dep for a test-only
    // capability): a live orphan announces itself by writing a file, a reaped one cannot.

    // The descendant's own delay before it writes its late marker, and the margin the test waits past it. The
    // margin is the slack for kill latency + the filesystem flush on a loaded CI runner. It must stay generous
    // in BOTH directions, because the consumers assert the marker in both: an ABSENCE assertion (the two
    // teardown tests, and the guard test's `settled == false` branch) is weakened toward a false PASS if a
    // runner stalls a surviving descendant past the whole window, while a PRESENCE assertion (the guard test's
    // `settled == true` branch) turns into a false FAILURE if a stalled runner has not let the descendant
    // reach its write by then. So do not trim the margin on the strength of one direction.
    const DESCENDANT_LATE_MARKER_DELAY: Duration = Duration::from_secs(2);
    const DESCENDANT_LATE_MARKER_MARGIN: Duration = Duration::from_secs(2);

    // Build the process-TREE fixture in `scratch`: a confined child that starts a longer-lived DESCENDANT.
    // The descendant writes `started.txt` at once and `alive.txt` only after DESCENDANT_LATE_MARKER_DELAY, so
    // the early marker proves it ran (non-vacuity) and the late marker appears IFF it outlived the teardown.
    //
    // Windows: the confined child is `cmd.exe`, which runs a NESTED `cmd.exe` (the descendant) reading a script
    // FILE written into the scratch dir — a file rather than an inline nested command so the script text
    // carries no quote character (cmd's `/c` quote-stripping rules and Rust's MSVCRT arg quoting disagree about
    // nested quotes; every script in this module stays quote-free for that reason). `%SystemRoot%` expands from
    // the plan's own minimal env; `ping -n 3` is the PATH-free ~2 s sleep this module already uses.
    // Unix: `/bin/sh` backgrounds a SUBSHELL (the descendant) and then blocks. A non-interactive shell has no
    // job control, so that subshell and its `sleep` stay in THE SHELL'S process group — exactly the group
    // members `killpg` must reap. [Build-Session-Entscheidung: P4.10]
    fn descendant_tree_script(scratch: &Path) -> &'static str {
        #[cfg(windows)]
        {
            std::fs::write(
                scratch.join("descendant.cmd"),
                "@echo off\r\n\
                 echo x> started.txt\r\n\
                 %SystemRoot%\\System32\\ping.exe -n 3 127.0.0.1 > nul\r\n\
                 echo x> alive.txt\r\n",
            )
            .expect("the descendant script is written into the scratch dir");
            "%SystemRoot%\\System32\\cmd.exe /d /c descendant.cmd"
        }
        #[cfg(unix)]
        {
            let _ = scratch;
            "( : > started.txt; sleep 2; : > alive.txt ) & sleep 10"
        }
    }

    fn descendant_tree_invocation(scratch: &Path) -> (EngineInvocation, PathBuf) {
        confined_shell_invocation_with_progress(
            descendant_tree_script(scratch),
            Some(scratch.to_path_buf()),
            ProgressModel::CoarseSpawnDone,
        )
    }

    // Poll for the descendant's early marker, bounded — returns whether it appeared. Every teardown assertion
    // is armed off THIS, never off a wall-clock guess that could fire before the descendant even existed.
    async fn descendant_started(marker: &Path) -> bool {
        for _ in 0..250 {
            if marker.exists() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        marker.exists()
    }

    // §1.7 (G15, P4.10): a cancel GROUP-kills — the engine's descendant dies with it and never writes the
    // late marker.
    #[tokio::test]
    async fn a_cancel_group_kills_the_engines_descendants() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        let (envelope, program) = descendant_tree_invocation(scratch.path());
        let token = envelope.cancel.clone();
        let started = scratch.path().join("started.txt");
        let canceller = tokio::spawn(async move {
            let observed = descendant_started(&started).await;
            token.cancel();
            observed
        });
        let run = run_confined(&envelope, &program, |_| {}).await;
        let observed_start = canceller.await.expect("the canceller task joins");

        assert_eq!(
            run.result,
            InvocationResult::Cancelled,
            "§1.7: the cancelled invocation reports Cancelled"
        );
        assert!(
            observed_start,
            "non-vacuity: the descendant really ran before the cancel (its early marker exists), so an absent late marker below can only mean it was reaped"
        );
        // Past the descendant's own delay: an ORPHANED descendant writes its late marker in this window.
        tokio::time::sleep(DESCENDANT_LATE_MARKER_DELAY + DESCENDANT_LATE_MARKER_MARGIN).await;
        assert!(
            !scratch.path().join("alive.txt").exists(),
            "§1.7: the group-kill reaped the engine's DESCENDANT too — a direct-child-only kill would have left it running to write this marker (the soffice -> soffice.bin orphan class)"
        );
    }

    // §1.7 (G15, P4.10): the guard's DECISION, both directions, over a REAL process tree — an UNSETTLED guard
    // group-kills on drop, a SETTLED one deliberately STANDS DOWN. The stand-down half is the load-bearing
    // one: neither platform's `wait()` proves the group is empty, so a post-exit group-kill would be
    // speculative, and for an engine whose launcher exits before its worker has finished writing it would
    // truncate `out_tmp` mid-write while the exit still reads as success — publishing a corrupt output as a
    // clean one. Without this test a refactor could silently re-introduce the always-kill and only a real
    // engine would notice.
    //
    // [Build-Session-Entscheidung: P4.10] this exercises the guard DIRECTLY rather than through
    // `run_confined`, and that is what makes it deterministic on all three OSes: the child's std handles are
    // NULL here, so no descendant can hold a pipe open. Driving the stand-down end-to-end would instead need a
    // descendant detached from the invocation's stdout/stderr PIPES — trivially expressible on POSIX
    // (`>/dev/null 2>&1 &`), but on Windows a `start /b`-launched grandchild inherits those handles whatever
    // cmd-level redirection is applied (measured: every variant kept the pipe open until the worker exited),
    // so the invocation could not return while the worker still ran and the assertion would be VACUOUS. The
    // spawn path is not mocked away: the tree is real and it is wrapped by the SAME `group_wrapped`
    // composition production uses. This test itself does not exercise the per-arm `group_settled` assignment
    // INSIDE `run_confined`; those are pinned end-to-end elsewhere — the two teardown tests below pin the cancel
    // arm + the drop backstop, and the P4.12 crash-vs-clean completed-exit test in this module (unix) pins the
    // COMPLETED-WAIT arm (clean → settled/stand-down, crash → unsettled/group-kill).
    #[tokio::test]
    async fn a_settled_guard_stands_down_while_an_unsettled_one_group_kills() {
        for settled in [true, false] {
            let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
            let (program, mut args) = shell();
            args.push(OsString::from(descendant_tree_script(scratch.path())));
            let mut command = Command::new(&program);
            command.env_clear();
            command
                .envs(minimal_env())
                .args(&args)
                .current_dir(scratch.path())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let child = group_wrapped(command)
                .spawn()
                .expect("the real process tree spawns");
            let mut guard = GroupKillGuard::new(child);
            guard.group_settled = settled;

            // Drop only once the DESCENDANT is really running — otherwise "its marker never appeared" would
            // prove nothing about the guard.
            assert!(
                descendant_started(&scratch.path().join("started.txt")).await,
                "non-vacuity: the descendant really started before the guard was dropped"
            );
            drop(guard);

            tokio::time::sleep(DESCENDANT_LATE_MARKER_DELAY + DESCENDANT_LATE_MARKER_MARGIN).await;
            assert_eq!(
                scratch.path().join("alive.txt").exists(),
                settled,
                "§1.7: a SETTLED guard must leave a still-working descendant alone (killing it would truncate a worker mid-write and publish a corrupt output as a success); an UNSETTLED one must group-kill it"
            );
        }
    }

    // §1.7 (G15, P4.12): the run_confined COMPLETED-WAIT arm's crash-vs-clean group-kill decision, END-TO-END
    // over a real process tree — the arm the P4.10 guard-decision test above could exercise only by SETTING
    // group_settled directly. The launcher starts a detached descendant (its std handles redirected to
    // /dev/null so it does NOT hold the invocation's stdout/stderr pipes — otherwise the drains never reach EOF
    // and run_confined could not return while the descendant runs, the exact Windows vacuity the guard-decision
    // test records), waits until the descendant has really started, then EXITS with the chosen code. On a CLEAN
    // exit the guard stands down and the descendant SURVIVES to write its late marker; on a CRASH (non-zero)
    // exit the guard stays armed → the Drop backstop group-kills the tree (no late marker). Unix-only for the
    // detach-from-pipes reason above; the decision logic itself is platform-independent Rust.
    // [Build-Session-Entscheidung: P4.12]
    #[cfg(unix)]
    #[tokio::test]
    async fn a_crash_completed_exit_group_kills_the_tree_while_a_clean_one_stands_down() {
        for (exit_code, expect_alive) in [(0, true), (3, false)] {
            let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
            // The descendant is detached from the launcher's stdout/stderr (`</dev/null >/dev/null 2>&1`) so the
            // drains reach EOF when the LAUNCHER exits — reaching the completed-wait arm while the descendant is
            // still sleeping. It stays in the launcher's process group (no job control) so `killpg` reaps it.
            // The launcher waits until `started.txt` exists before exiting, so the descendant has provably run
            // (non-vacuity) before run_confined returns and the crash arm's kill fires.
            let script = format!(
                "( : > started.txt; sleep {}; : > alive.txt ) </dev/null >/dev/null 2>&1 &\n\
                 until [ -f started.txt ]; do sleep 0.01; done\n\
                 exit {}",
                DESCENDANT_LATE_MARKER_DELAY.as_secs(),
                exit_code,
            );
            let (envelope, program) = confined_shell_invocation_with_progress(
                &script,
                Some(scratch.path().to_path_buf()),
                ProgressModel::CoarseSpawnDone,
            );
            let run = run_confined(&envelope, &program, |_| {}).await;
            assert_eq!(
                run.result,
                if exit_code == 0 {
                    InvocationResult::Succeeded
                } else {
                    InvocationResult::Failed(ConversionErrorKind::EngineCrash)
                },
                "§1.7: a clean exit is Succeeded; a nonzero exit is the pre-classification EngineCrash floor"
            );
            assert_eq!(
                run.exit.map(|status| status.success()),
                Some(exit_code == 0),
                "§1.7 (P4.12): run_confined surfaces the raw completed-wait ExitStatus in ConfinedRun::exit"
            );
            assert!(
                descendant_started(&scratch.path().join("started.txt")).await,
                "non-vacuity: the descendant really started (the launcher waited for it), so the late-marker \
                 check below is meaningful in both directions"
            );
            // Past the descendant's own delay: it writes alive.txt IFF it outlived run_confined's teardown.
            tokio::time::sleep(DESCENDANT_LATE_MARKER_DELAY + DESCENDANT_LATE_MARKER_MARGIN).await;
            assert_eq!(
                scratch.path().join("alive.txt").exists(),
                expect_alive,
                "§1.7 (P4.12): a CLEAN completed exit stands the guard down (descendant survives → alive.txt \
                 written); a CRASH completed exit leaves the guard armed → the Drop backstop group-kills the \
                 doomed tree (no alive.txt)"
            );
        }
    }

    // §1.7 (G15, P4.10): DROPPING the `run_confined` future group-kills too — the `GroupKillGuard` backstop.
    // This is the path no explicit arm can reach (a caller's `tokio::time::timeout` at P4.12, the §7.3.3
    // quit-while-converting path) and the one `process-wrap`'s `KillOnDrop` shim does NOT cover: it sets only
    // tokio's DIRECT-child kill-on-drop, and the Job Object's kill-on-job-close limit that the shim is meant to
    // switch on is unreachable in 9.1.0 (the `core`-is-empty defect recorded in `run_confined`). Without the
    // guard the descendant survives the drop and writes its late marker.
    #[tokio::test]
    async fn dropping_the_confined_run_group_kills_the_engines_descendants() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        let (envelope, program) = descendant_tree_invocation(scratch.path());
        let started = scratch.path().join("started.txt");

        // Drive the future until the descendant has really started, then DROP it mid-run — no cancel token is
        // tripped, so the only thing that can tear the tree down is the guard's `Drop`.
        let mut run = Box::pin(run_confined(&envelope, &program, |_| {}));
        let mut observed_start = false;
        for _ in 0..250 {
            tokio::select! {
                _ = &mut run => break,
                _ = tokio::time::sleep(Duration::from_millis(20)) => {}
            }
            if started.exists() {
                observed_start = true;
                break;
            }
        }
        drop(run);

        assert!(
            observed_start,
            "non-vacuity: the descendant really ran before the drop, so an absent late marker below can only mean it was reaped"
        );
        tokio::time::sleep(DESCENDANT_LATE_MARKER_DELAY + DESCENDANT_LATE_MARKER_MARGIN).await;
        assert!(
            !scratch.path().join("alive.txt").exists(),
            "§1.7: dropping the run group-kills the engine AND its descendants — the GroupKillGuard backstop, since kill-on-job-close is inert upstream"
        );
    }

    // §1.7 step 2 (G15, P4.11): the cancel arm's bounded confirm-wait RUNS (the group is reaped before
    // `run_confined` returns, which is what lets the tier-1 conductor's subsequent §2.6.4 removal succeed on
    // the normal cancel) AND is BOUNDED — on a group that settles normally it returns in well under
    // `GROUP_CONFIRM_WAIT` (5s), never near the cap. This is the responsiveness half of §1.7 step 2 (SSOT *app
    // stays responsive*; the §5.8 cancel round-trip / §7.3.3 quit must not hang): the runner returns as soon
    // as the OS reaps, and the cap only bites on a genuinely wedged descendant (not constructed here — a real
    // wedge would hold the test ~5s; the bound itself is exercised by the `tokio::time::timeout` wrapper, and
    // the wedged path is the conductor-side §2.6.4 residue test's concern). [Build-Session-Entscheidung: P4.11]
    #[tokio::test]
    async fn a_cancel_confirm_wait_reaps_the_group_and_returns_well_within_the_bound() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        let (envelope, program) = descendant_tree_invocation(scratch.path());
        let token = envelope.cancel.clone();
        let started = scratch.path().join("started.txt");
        let canceller = tokio::spawn(async move {
            let observed = descendant_started(&started).await;
            token.cancel();
            observed
        });
        let began = tokio::time::Instant::now();
        let run = run_confined(&envelope, &program, |_| {}).await;
        let elapsed = began.elapsed();
        let observed_start = canceller.await.expect("the canceller task joins");

        assert!(
            observed_start,
            "non-vacuity: the descendant really ran before the cancel"
        );
        assert_eq!(
            run.result,
            InvocationResult::Cancelled,
            "§1.7: the cancelled invocation reports Cancelled"
        );
        // The confirm-wait ran (the group is reaped by the time we return) yet the runner returned far under
        // the 5s cap — a normal-teardown cancel is responsive, never anywhere near the wedged-descendant bound.
        assert!(
            elapsed < GROUP_CONFIRM_WAIT,
            "§1.7: run_confined returned in {elapsed:?}, well within the {GROUP_CONFIRM_WAIT:?} confirm-wait bound (the group settled; the cap only bites on a wedged descendant)"
        );
    }

    // §2.12.3 P4.16 (Co-Pilot ruling 2026-07-25): a concurrency regression over the CHEAP-TIER spawn path
    // (macOS attaches no privilege-drop apply leg — DECIDED cheap-tier). Origin: the P4.16 design's macOS
    // fork-safety concern; RETAINED without an apply leg (per the ruling) as a standing regression that the
    // cheap-tier spawn + the §1.7 concurrent stdout/stderr drain handle many in-flight confined spawns cleanly
    // — no deadlock, no leak, every child reaps — each bounded by a timeout so a wedged spawn surfaces as a
    // deterministic RED, never a hung suite. `multi_thread` so the spawns genuinely run in parallel across
    // worker threads (the shape that would have exposed a fork-child hang, had an apply leg been introduced).
    // [Build-Session-Entscheidung: P4.16]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn many_concurrent_cheap_tier_spawns_all_complete_under_timeout() {
        const CONCURRENT_SPAWNS: usize = 16;
        let mut tasks = Vec::with_capacity(CONCURRENT_SPAWNS);
        for _ in 0..CONCURRENT_SPAWNS {
            tasks.push(tokio::spawn(async {
                // Each task owns its scratch dir (a `TempDir` with `Drop`, so it outlives the confined child
                // that runs in it) and its own confined envelope — no shared state between the concurrent spawns.
                let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
                let (envelope, program) =
                    confined_shell_invocation(EXIT_ZERO, Some(scratch.path().to_path_buf()));
                let run = tokio::time::timeout(
                    Duration::from_secs(60),
                    run_confined(&envelope, &program, |_| {}),
                )
                .await
                .expect("a cheap-tier confined spawn must complete well within the timeout (never wedge)");
                run.result
            }));
        }
        for task in tasks {
            assert_eq!(
                task.await.expect("the spawned task must not panic"),
                InvocationResult::Succeeded,
                "every concurrent cheap-tier `exit 0` spawn must reap to Succeeded (no deadlock / no leak)"
            );
        }
    }

    // ─── P4.18.1: the §2.12.3 per-spawn tier-APPLIED regression ──────────────────────────────────────
    //
    // This is the activation of the P0.5.9 isolation/privilege-drop home ("the privilege-drop-tier-applied
    // per-run regression assertion", §6.4.2/test-strategy §6) and the named anchor of the **P0.7.12 leg-(a)
    // enforcement SUBSTRATE**, the substrate P0.7.12 records as "activating with the first engine spawn in
    // P4". Its three parts are asserted here, each exactly once, none of them re-stated:
    //   * the `.env_clear()` spawn invariant — `the_child_runs_env_cleared_in_the_scratch_cwd` above (P4.13),
    //     reinforced structurally by the G29 SAST rule on the spawn builder;
    //   * the Landlock `{scratch rw, everything else denied}` grant + the net-deny namespace — the Linux
    //     effect test below, driven through the PRODUCTION `run_confined` rather than through
    //     `install_confinement` directly (which is what `crate::platform`'s per-leg tests exercise);
    //   * the per-spawn achieved-tier record itself — the two cross-platform tests immediately below.
    //
    // The DIVISION OF LABOUR against the sibling gates: `crate::platform`'s `privilege_drop_record_tests`
    // hold `privilege-drop-coverage.toml` to the CODE (host-independent); these hold it to a running SPAWN
    // (host-dependent). Neither can substitute for the other — a leg can be present in code and silently
    // stop attaching at spawn time, which is precisely the invisible regression G64 exists to surface.

    // §2.12.3/§2.11.4 (G31/G42/G42b/G64, P4.18.1): every leg the platform's `privilege-drop-coverage.toml`
    // row names reports a verdict on a REAL confined spawn — the mechanical half of "the achieved tier is
    // applied on each engine spawn". A leg that stopped attaching would report nothing here.
    #[tokio::test]
    async fn every_recorded_leg_reports_a_verdict_on_a_real_confined_spawn() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        let (envelope, program) =
            confined_shell_invocation(EXIT_ZERO, Some(scratch.path().to_path_buf()));
        let run = run_confined(&envelope, &program, |_| {}).await;
        assert_eq!(
            run.result,
            InvocationResult::Succeeded,
            "non-vacuity: the confined child must really have run for its tier record to mean anything"
        );
        let tier = run.tier.expect(
            "§2.12.3/P4.18: a spawn that produced a child always carries its achieved-tier record",
        );
        let reported: Vec<&str> = tier.verdicts().iter().map(|verdict| verdict.leg).collect();
        assert_eq!(
            reported,
            crate::platform::ATTACHED_LEGS,
            "§2.12.3/G64: every leg `privilege-drop-coverage.toml` records for this platform must report a \
             per-spawn verdict — a leg that silently stopped attaching is the NET tier regression the G64 \
             ratchet exists to surface"
        );
        // The tier a SPAWN reports must be BACKED by a verdict, never inferred from the leg set the build
        // attaches: a `tier()` that read `ATTACHED_LEGS` instead of the recorded outcomes would claim the
        // privilege-drop tier for a spawn where every leg degraded — an over-report, the one direction the
        // G64 ratchet must never take. Asserted as an EQUIVALENCE so both directions bite (and so it cannot
        // be trivially true the way a "reaches at most the recorded tier" phrasing would be, with only two
        // possible tier values).
        let any_applied = tier
            .verdicts()
            .iter()
            .any(|verdict| verdict.outcome == crate::platform::LegOutcome::Applied);
        assert_eq!(
            tier.tier() == crate::platform::TIER_PRIVILEGE_DROP,
            any_applied,
            "§2.12.3/G64: a spawn reports the privilege-drop tier EXACTLY when one of its legs actually \
             APPLIED — verdicts={:?}",
            tier.verdicts()
        );
    }

    // §2.12.3 (G31, P4.18.1): the NON-VACUITY partner of the assertion above — the record is `Some` because
    // a child was confined, not because the field is unconditionally filled. A spawn that never produced a
    // child (a missing engine binary, the §2.13.1 item-level spawn fault) has nothing to confine and must
    // say so, so an empty record can never be mistaken for "the tier degraded".
    #[tokio::test]
    async fn a_spawn_that_never_produced_a_child_carries_no_tier_record() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        let (envelope, _program) =
            confined_shell_invocation(EXIT_ZERO, Some(scratch.path().to_path_buf()));
        let missing = scratch.path().join("no-such-engine-binary");
        let run = run_confined(&envelope, &missing, |_| {}).await;
        assert_eq!(
            run.result,
            InvocationResult::Failed(ConversionErrorKind::InternalError),
            "non-vacuity: the spawn really failed before any child existed"
        );
        assert!(
            run.tier.is_none(),
            "§2.12.3/P4.18: no child, no confinement, no achieved-tier record — {:?}",
            run.tier
        );
    }

    // §2.12.3/§2.11.4 (G31/G42/G42b/G64, P4.18.1) — the Linux EFFECT half, through the production
    // `run_confined` (the `crate::platform` per-leg tests drive `install_confinement` directly, so this is
    // the first assertion that the wired-up spawn path really confines). It is also the P0.7.12 leg-(a)
    // substrate in one observation: the confined child WRITES its own scratch (never-break — the Landlock
    // `{scratch rw}` grant) while the out-of-input read the T9b fs-audit half targets is DENIED.
    //
    // Each leg is keyed on its own recorded verdict, the `match landlock_probe()` shape the P4.15 tests use
    // — the tier degrades silently by design, so a kernel without Landlock must skip its arm rather than
    // fail the build. The two arms differ in STRENGTH because their verdict sources do
    // (`crate::platform::VERDICT_SOURCES`): Landlock's is a `host-probe`, so the assertion is one-directional
    // (an APPLIED Landlock leg MUST deny; a degraded one says nothing about this spawn). The net-ns verdict
    // is `per-spawn` — the parent compares the child's own `/proc/<pid>/ns/net` — so it is asserted as an
    // EQUIVALENCE against what the child itself observed: the record must not claim a namespace the child
    // never got (the over-report G64 must never take), and must not deny one it demonstrably did get.
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn the_recorded_linux_legs_take_effect_on_a_real_confined_spawn() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        let outside = tempfile::tempdir().expect("a real dir OUTSIDE the Landlock grant set");
        let sentinel = outside.path().join("out-of-sandbox.txt");
        std::fs::write(&sentinel, b"sentinel").expect("plant the out-of-input sentinel");
        // Every redirection targets the scratch cwd, so the parent reads each verdict off the filesystem.
        // `2>/dev/null` keeps a denial quiet: a denied read must show as an EMPTY leak file, not as a
        // failed script (the child must still complete — never-break).
        let script = format!(
            "readlink /proc/self/ns/net > netns.txt 2>/dev/null; \
             cat {sentinel} > leaked.txt 2>/dev/null; \
             : > done.txt",
            sentinel = sentinel.display()
        );
        let (envelope, program) =
            confined_shell_invocation(&script, Some(scratch.path().to_path_buf()));
        let run = run_confined(&envelope, &program, |_| {}).await;
        let tier = run
            .tier
            .expect("§2.12.3/P4.18: a spawned child always carries its achieved-tier record");
        assert!(
            scratch.path().join("done.txt").exists(),
            "non-vacuity + never-break: the confined child ran AND could write its own scratch (the \
             Landlock `{{scratch rw}}` grant, the P0.7.12 leg-(a) substrate)"
        );

        if tier.outcome_of(crate::platform::LEG_LANDLOCK)
            == Some(crate::platform::LegOutcome::Applied)
        {
            let leaked = std::fs::read(scratch.path().join("leaked.txt")).unwrap_or_default();
            assert!(
                leaked.is_empty(),
                "§2.12.3 Landlock: an APPLIED fs-restrict leg must DENY the out-of-sandbox read the \
                 record claims it prevents — leaked {} bytes",
                leaked.len()
            );
        }

        let child_ns =
            std::fs::read_to_string(scratch.path().join("netns.txt")).unwrap_or_default();
        let parent_ns = std::fs::read_link("/proc/self/ns/net")
            .map(|target| target.display().to_string())
            .unwrap_or_default();
        let netns = tier.outcome_of(crate::platform::LEG_NETNS);
        // `Unavailable` = the parent could not read the membership at all (the child raced it to exit), so
        // there is nothing to compare against; every OTHER verdict is a claim about this spawn and is held
        // to what the child itself saw.
        if netns
            != Some(crate::platform::LegOutcome::Degraded(
                crate::platform::DegradeReason::Unavailable,
            ))
        {
            assert!(
                !child_ns.trim().is_empty() && !parent_ns.is_empty(),
                "non-vacuity: both namespace links must be readable for the equivalence below to mean \
                 anything (child={child_ns:?} parent={parent_ns:?})"
            );
            assert_eq!(
                netns == Some(crate::platform::LegOutcome::Applied),
                child_ns.trim() != parent_ns,
                "§2.12.3 net-ns: the record reports the egress-deny leg APPLIED for this spawn EXACTLY \
                 when the child really ran in its OWN network namespace — over-reporting a namespace the \
                 child never got is the one direction the G64 record must never take (child={child_ns:?} \
                 parent={parent_ns:?} verdict={netns:?})"
            );
        }
    }

    // §2.12.3 (G31/G64, P4.18.1) — the Windows EFFECT half, through the production `run_confined`. Both
    // P4.17 legs are applied parent-side on the still-suspended child, so both have a real per-spawn
    // read-back and this is the assertion that the RECORDED tier is the tier a spawn actually reaches.
    // Every Leg-A precondition holds here (a local NTFS scratch we created ourselves, an unlabelled
    // System32 program), which is what keeps the assertion from passing forever on a silent-degrade arm —
    // the same non-vacuity posture as `the_tier_actually_applies_on_a_local_scratch_with_an_owned_publish_temp`,
    // one layer up: that one reads the internal hand-back cell, this one reads the PUBLIC per-spawn record.
    #[cfg(windows)]
    #[tokio::test]
    async fn the_recorded_windows_legs_apply_on_a_real_confined_spawn() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the confined cwd");
        let (envelope, program) =
            confined_shell_invocation(EXIT_ZERO, Some(scratch.path().to_path_buf()));
        let run = run_confined(&envelope, &program, |_| {}).await;
        assert_eq!(
            run.result,
            InvocationResult::Succeeded,
            "non-vacuity + never-break: the confined child ran to a clean exit under BOTH applied legs"
        );
        let tier = run
            .tier
            .expect("§2.12.3/P4.18: a spawned child always carries its achieved-tier record");
        assert_eq!(
            tier.outcome_of(crate::platform::LEG_JOB),
            Some(crate::platform::LegOutcome::Applied),
            "§2.12.3 Leg B: ConvertIA's own kill-on-job-close Job Object attaches on every spawn — it \
             depends on no volume or label precondition"
        );
        assert_eq!(
            tier.outcome_of(crate::platform::LEG_INTEGRITY),
            Some(crate::platform::LegOutcome::Applied),
            "§2.12.3 Leg A: every label-then-lower precondition holds on a local NTFS scratch we own, so \
             the write confinement must APPLY here — a degrade in THIS environment is a real defect, not \
             the production silent-degrade (a FAT/exFAT or SMB destination)"
        );
        assert_eq!(
            tier.tier(),
            crate::platform::attached_tier(),
            "§2.12.3/G64: with both legs applied the spawn reaches exactly the tier the record claims"
        );
    }
}

// §2.12.3 best-effort WINDOWS privilege-drop tier (P4.17) — the cross-module ENFORCEMENT half. The per-leg
// primitives are unit-tested in `crate::platform`; what only this module can prove is the composed claim the
// ruling arms here: a child spawned through the SAME `WindowsConfinement` composition the production path
// registers can still write its own labelled sinks (never-break) and is DENIED a Medium sink it does not own
// (the confinement's actual goal). Child-observed against the real kernel — the grant-IS-enforcement model,
// exactly as the Landlock / seccomp / net-ns legs are proven, never a mock (test-strategy §0.1). TWO STACKED
// cfg attrs (`#[cfg(test)]` then `#[cfg(windows)]`) — NOT a compound `all(test, windows)` (the P1.17 clippy
// `is_cfg_test` trap).
#[cfg(test)]
#[cfg(windows)]
mod windows_confinement_tests {
    use super::{group_wrapped, WindowsConfinement, WindowsConfinementOutcome};
    use crate::platform::{DegradeReason, LegOutcome};
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::process::Stdio;
    use std::sync::{Arc, Mutex, PoisonError};
    use tokio::process::Command;

    // An absolute System32 executable — never a PATH lookup (the confined child runs env-cleared).
    fn system32(exe: &str) -> PathBuf {
        let root = std::env::var_os("SystemRoot").expect("SystemRoot is set on Windows");
        Path::new(&root).join("System32").join(exe)
    }

    // Run `script` through `cmd.exe` under the SAME composition `run_confined` builds — the P4.10 group
    // wrappers plus the P4.17 `WindowsConfinement`, with the Leg-A grant issued first exactly as production
    // issues it. Returns the tier outcome so each assertion can key on the arm that matches this runner
    // (the P4.15 `match landlock_probe()` shape).
    async fn confined_cmd(
        scratch: &Path,
        out_tmp: Option<&Path>,
        script: &str,
    ) -> WindowsConfinementOutcome {
        confined_cmd_at(scratch, out_tmp, script, None).await
    }

    // The general form: `force_level` overrides the level the child's token is lowered to (production always
    // uses the grant's own `CONFINED_INTEGRITY_RID`), so a test can stand a CO-TENANT up at another level —
    // the only way to prove the enforcement claim from the outside — or force the lowering itself to be
    // refused by the kernel.
    async fn confined_cmd_at(
        scratch: &Path,
        out_tmp: Option<&Path>,
        script: &str,
        force_level: Option<u32>,
    ) -> WindowsConfinementOutcome {
        let program = system32("cmd.exe");
        let granted = crate::platform::label_confinement_sinks(scratch, out_tmp, &program);
        let lower_to = match force_level {
            Some(rid) => Some(rid),
            None => granted.then_some(crate::platform::CONFINED_INTEGRITY_RID),
        };
        // The production shape verbatim: an owned `Command` whose `env_clear()` is the gap-free next
        // statement (the G29 rule-(b1) split-builder suppression this crate carries), then the wrappers.
        // nosemgrep: convertia-command-outside-isolation
        let mut command = Command::new(&program);
        command.env_clear();
        command
            .envs([(
                OsString::from("SystemRoot"),
                std::env::var_os("SystemRoot").expect("SystemRoot is set on Windows"),
            )])
            .args([OsString::from("/d"), OsString::from("/c")])
            .current_dir(scratch)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // `cmd.exe` parses `/c`'s tail with its OWN quoting rules, which do not understand the MSVCRT
        // backslash-escaping `Command::arg` applies — a script containing `"` would arrive mangled. `raw_arg`
        // passes it through verbatim, the documented way to hand cmd.exe a command line.
        {
            use std::os::windows::process::CommandExt;
            command.as_std_mut().raw_arg(script);
        }
        let outcome = Arc::new(Mutex::new(WindowsConfinementOutcome::default()));
        let mut wrapped = group_wrapped(command);
        wrapped.wrap(WindowsConfinement {
            lower_to,
            outcome: Arc::clone(&outcome),
        });
        let mut child = wrapped.spawn().expect("spawn the confined cmd.exe child");
        child.wait().await.expect("reap the confined child");
        let mut slot = outcome.lock().unwrap_or_else(PoisonError::into_inner);
        WindowsConfinementOutcome {
            job: slot.job.take(),
            integrity: slot.integrity,
        }
    }

    // §6.4.3 integration (G31) / §2.12.3 Leg A — THE enforcement claim. The confined child writes markers into
    // its own labelled scratch, so the parent reads the verdict off the filesystem rather than off a localized
    // console string. Three observations: the run happened at all (non-vacuity), the labelled `.part` stayed
    // writable (never-break), and the Medium sibling directory was refused (the confinement).
    #[tokio::test]
    async fn a_confined_child_writes_its_labelled_sink_and_is_denied_a_medium_one() {
        let scratch = tempfile::tempdir().expect("a real per-run scratch dir");
        let dest = tempfile::tempdir().expect("a real destination dir for the publish temp");
        let medium = tempfile::tempdir().expect("a real UNLABELLED (Medium) sibling dir");
        let part = dest.path().join("item.part");
        std::fs::write(&part, b"seed").expect("create the publish temp the parent owns");
        let obs = scratch.path();
        let script = format!(
            "(echo x)>>\"{part}\" && (echo ok)>\"{obs}\\\\part_ok\" & \
             (echo y)>\"{medium}\\\\leak.txt\" && (echo leak)>\"{obs}\\\\medium_leak\" & \
             (echo done)>\"{obs}\\\\done\"",
            part = part.display(),
            obs = obs.display(),
            medium = medium.path().display(),
        );
        let outcome = confined_cmd(scratch.path(), Some(&part), &script).await;
        assert!(
            obs.join("done").exists(),
            "non-vacuity: the confined child must have run its script to the end"
        );
        assert!(
            obs.join("part_ok").exists(),
            "never-break: the child MUST still write the labelled publish temp it was granted"
        );
        match outcome.integrity {
            Some(LegOutcome::Applied) => assert!(
                !obs.join("medium_leak").exists(),
                "§2.12.3 Leg A applied — a write into an unlabelled Medium directory MUST be denied"
            ),
            Some(LegOutcome::Degraded(reason)) => assert!(
                obs.join("medium_leak").exists(),
                "Leg A degraded ({reason:?}) — the child keeps its Medium write access (silent-degrade)"
            ),
            None => assert!(
                obs.join("medium_leak").exists(),
                "the Leg-A grant was not issued — the child keeps its Medium write access (cheap tier)"
            ),
        }
        assert_ne!(
            outcome.integrity,
            Some(LegOutcome::Degraded(DegradeReason::NotApplied)),
            "a lowering that returned yet did not take is a real defect, not a legitimate degrade"
        );
    }

    // §6.4.3 integration (G31) / §2.12.3 Leg A — **THE claim the 0x1800 choice exists for**, armed here on a
    // real Windows runner as the P4.17 ruling requires. The round-1 review of that ruling found that labelling
    // the `.part` at the WELL-KNOWN LOW level would leave it writable by every same-user Low process — an
    // Acrobat renderer, Office Protected View, a browser content process, i.e. exactly the sandboxes a hostile
    // document compromises — which could overwrite the `.part` mid-conversion, before the strip publishes it
    // under the expected name. The fix was the INTERMEDIATE level; this test proves the fix, from the co-tenant's
    // side: a child standing in for such a sandbox, lowered to Low (4096), is DENIED write-UP to the 6144
    // `.part`, while the Medium parent still writes it. The co-tenant reports into its OWN Low-labelled dir,
    // because a Low subject cannot write a Medium one either — so its markers are the honest non-vacuity signal.
    #[tokio::test]
    async fn a_low_co_tenant_is_denied_write_up_to_the_labelled_publish_temp() {
        const LOW_INTEGRITY_RID: u32 = 0x1000;
        let scratch = tempfile::tempdir().expect("a real per-run scratch dir");
        let dest = tempfile::tempdir().expect("a real destination dir");
        let obs = tempfile::tempdir().expect("a real Low-labelled observation dir");
        let part = dest.path().join("item.part");
        std::fs::write(&part, b"seed").expect("create the publish temp the parent owns");
        assert!(
            crate::platform::label_at_level(obs.path(), LOW_INTEGRITY_RID, true),
            "the co-tenant needs a sink at its OWN level to report into"
        );
        let script = format!(
            "(echo tampered)>>\"{part}\" && (echo t)>\"{obs}\\\\tampered\" & \
             (echo done)>\"{obs}\\\\done\"",
            part = part.display(),
            obs = obs.path().display(),
        );
        let outcome = confined_cmd_at(
            scratch.path(),
            Some(&part),
            &script,
            Some(LOW_INTEGRITY_RID),
        )
        .await;
        assert_eq!(
            outcome.integrity,
            Some(LegOutcome::Applied),
            "non-vacuity: the co-tenant must really be running at Low, or the denial below proves nothing"
        );
        // The OTHER half of the non-vacuity: an UNLABELLED `.part` would deny the Low writer just as well
        // (implicit Medium), so without this the denial could pass for the wrong reason — the test would keep
        // passing if the grant stopped labelling entirely. Pin the sink's actual level in-test.
        assert!(
            crate::platform::read_label_sddl_for_test(&part)
                .unwrap_or_default()
                .contains("S-1-16-6144"),
            "non-vacuity: the `.part` must really carry the confined level, not merely deny by implicit Medium"
        );
        assert!(
            obs.path().join("done").exists(),
            "non-vacuity: the Low co-tenant must have run and been able to write its OWN level"
        );
        assert!(
            !obs.path().join("tampered").exists(),
            "§2.12.3 P4.17: a Low co-tenant MUST be denied write-UP to the 6144-labelled `.part` — this is \
             exactly the tamper the intermediate level (rather than the well-known Low) was chosen to exclude"
        );
        assert_eq!(
            std::fs::read(&part).expect("read the publish temp back"),
            b"seed",
            "no-harm: the denied write must not have changed a byte of the publish temp"
        );
        std::fs::write(&part, b"parent still writes").expect(
            "the Medium parent must still write the labelled temp — the label restricts write-UP only",
        );
    }

    // §6.4.2 fault-injection (G16/G31) / §2.12.3 — the second half of the never-break red-green: here the
    // hook TRAVERSES the risky `SetTokenInformation` call and the kernel REFUSES it (raising an integrity
    // level is `ERROR_INVALID_LABEL`, unlike lowering). The child must still resume and complete, and the
    // refusal must read as `Unavailable` — the grant could not be obtained at all — never `NotApplied`, which
    // is reserved for a call that succeeded without the kernel showing the level.
    //
    // The forced level is derived from THIS process's own, never a fixed constant: the refusal depends on the
    // target being ABOVE the caller's level, so a hardcoded High would be a no-op success on an ELEVATED
    // runner (a GitHub-hosted `windows-2022` job runs at High) and the test would invert. One step above
    // whatever we are is a refusal on every host — the CI-runtime-validation lesson applied up front.
    #[tokio::test]
    async fn a_refused_token_lowering_still_yields_a_resumed_completing_child() {
        let own_level = crate::platform::child_integrity_rid(std::process::id())
            .expect("read this process's own integrity level");
        let above_us = own_level + 0x100;
        let scratch = tempfile::tempdir().expect("a real per-run scratch dir");
        let obs = scratch.path();
        let script = format!("(echo done)>\"{obs}\\\\done\"", obs = obs.display());
        let outcome = confined_cmd_at(scratch.path(), None, &script, Some(above_us)).await;
        assert!(
            obs.join("done").exists(),
            "never-break: a REFUSED token adjustment must still resume the child and let it complete"
        );
        assert_eq!(
            outcome.integrity,
            Some(LegOutcome::Degraded(DegradeReason::Unavailable)),
            "a refused write is an unobtainable grant (`Unavailable`), not a grant that failed to enforce \
             (own level {own_level:#x}, refused target {above_us:#x})"
        );
        assert!(
            outcome.job.is_some(),
            "Leg B is independent of Leg A — the own Job Object still attaches on the same spawn"
        );
    }

    // §6.4.3 integration (G31) / §2.12.3 Leg A — the NON-VACUITY guard for the assertion above (the g24
    // never-silently-watch-nothing lesson, and the ruling's "the runner test ARMS the full claim"). The
    // enforcement test keeps a legitimate silent-degrade arm, which on its own could pass forever on a runner
    // where the tier never applies. On a local NTFS scratch with a publish temp we created ourselves, every
    // precondition holds, so the tier MUST reach `Applied` here — a degrade in THIS environment is a real
    // defect, not the production silent-degrade.
    #[tokio::test]
    async fn the_tier_actually_applies_on_a_local_scratch_with_an_owned_publish_temp() {
        let scratch = tempfile::tempdir().expect("a real per-run scratch dir");
        let dest = tempfile::tempdir().expect("a real destination dir");
        let part = dest.path().join("item.part");
        std::fs::write(&part, b"seed").expect("create the publish temp the parent owns");
        let obs = scratch.path();
        let script = format!("(echo done)>\"{obs}\\done\"", obs = obs.display());
        let outcome = confined_cmd(scratch.path(), Some(&part), &script).await;
        assert!(
            obs.join("done").exists(),
            "non-vacuity: the confined child must have run"
        );
        assert_eq!(
            outcome.integrity,
            Some(LegOutcome::Applied),
            "every §2.12.3 Leg-A precondition holds here (local NTFS, sinks we own) — the tier must APPLY, \
             so the enforcement test above cannot pass forever on its silent-degrade arm"
        );
    }

    // §6.4.2 fault-injection (G16/G31) / §2.12.3 — the NEVER-BREAK red-green the ruling names: a Leg-A grant
    // that CANNOT be issued (a sink that is not ours to label) must leave a resumed, completing child. This is
    // the observable proof that the `post_spawn` hook silently degrades instead of returning `Err`, which would
    // propagate before any `wrap_child` and strand the `CREATE_SUSPENDED` child un-resumed and un-reaped. It
    // also pins LEG INDEPENDENCE: Leg B still attaches its job on the very same spawn.
    #[tokio::test]
    async fn a_failing_confinement_step_still_yields_a_resumed_completing_child() {
        let scratch = tempfile::tempdir().expect("a real per-run scratch dir");
        let dest = tempfile::tempdir().expect("a real destination dir");
        let unlabellable = dest.path().join("never-created.part");
        let obs = scratch.path();
        let script = format!("(echo done)>\"{obs}\\\\done\"", obs = obs.display());
        let outcome = confined_cmd(scratch.path(), Some(&unlabellable), &script).await;
        assert!(
            obs.join("done").exists(),
            "never-break: a degraded confinement step must still resume the child and let it complete"
        );
        assert!(
            outcome.integrity.is_none(),
            "the Leg-A grant failed, so the token must NOT have been lowered: {:?}",
            outcome.integrity
        );
        assert!(
            outcome.job.is_some(),
            "Leg B is independent of Leg A — the own Job Object still attaches on the same spawn"
        );
    }
}
