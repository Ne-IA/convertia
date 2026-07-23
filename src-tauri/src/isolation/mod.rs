//! `crate::isolation` — the §2.12 decoder-isolation wrapper every engine SUBPROCESS spawn routes through,
//! and the SOLE sanctioned `process::Command::new` site in the codebase — the concrete spawn primitive is
//! `tokio::process::Command` (the async spawn the §2.12 confined runner awaits under the §0.4.4 cancel
//! token): the G9 repo-invariant (b) scopes its qualified `process::Command::new` grep to this module, and the G29 spawn rule excludes
//! `**/isolation/**` from the spawn-outside-isolation ban — keeping every spawn inside this module is what
//! makes those two gates honest. A §0.7 tier-2 module: the §1.7 invocation lifecycle CALLS it and §3.5
//! builds the engine args INSIDE it; it depends DOWN only, never up on IPC / orchestrator / the engine
//! registry. Unsafe-free — the crate-root `#![deny(unsafe_code)]` (main.rs) covers it; the §2.12.3
//! privilege-drop tier reaches its per-OS confinement through SAFE wrapper crates (`process-wrap`
//! group-kill / Job-Object teardown + the best-effort seccomp / Landlock / Seatbelt / AppContainer
//! mechanisms), so this module adds NO FFI and NO `unsafe`; the confined-spawn entry [`run_confined`]
//! below is the P4.13-authored cheap-tier body.
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
//!    full; returned in the [`crate::engines::ConfinedRun`] outcome). The remaining layers land on THIS entry
//!    at their boxes: the §1.7 whole-group kill (`process-wrap` Job-Object / process-group teardown of the
//!    engine AND its descendants, e.g. `soffice` → `soffice.bin`) at **P4.10**, the §1.7 / §2.12.2
//!    timeout / no-progress watchdog at **P4.12**, the loader-var strip at **P4.14**, the per-OS
//!    privilege-drop legs at **P4.15 / P4.16 / P4.17**, and the achieved-tier record into
//!    `privilege-drop-coverage.toml` at **P4.18**. It never runs the §2.1 publish — that is
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
//! (Linux), Seatbelt (macOS), restricted-token / AppContainer + Job-Object caps (Windows) — is best-effort
//! defence-in-depth that degrades SILENTLY to the cheap tier where it cannot be enabled without install-time
//! elevation or breaking the portable build, and is NOT load-bearing (the §0.11 T9b network guarantee rests
//! on the §3.5 / §6.1.3 argv / build controls). The per-OS profile CONTENTS are a §2.12.3 tuning residual.
//! Whether the achieved depth surfaces as a Rust tier value (e.g. a `SandboxTier` enum) or as an
//! unconditional cheap floor plus best-effort privilege-drop with no runtime discriminant is a P4 shaping
//! choice made WITH its real consumer (the P4.18 achieved-tier record) — no possibly-unused type is planted
//! here (CLAUDE §5 no-premature-commitment; the P3.1 doc-only precedent).
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

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};
use tokio::process::{ChildStderr, ChildStdout, Command};

use crate::engines::{ConfinedRun, EngineInvocation, InvocationResult, ProgressModel, StdinPlan};
use crate::outcome::ConversionErrorKind;

/// The §2.12 confined-spawn entry (P4.13) — runs ONE subprocess engine invocation inside the §2.12.3
/// **cheap-tier floor**, the NON-NEGOTIABLE v1 confinement shipped unconditionally on all three OSes:
///
/// - the **§2.12.1 process boundary** — a real OS subprocess (`tokio::process`); a decoder that
///   segfaults/aborts takes down only its own process, never the core (§2.12.1);
/// - a **minimal / cleared environment** — `env_clear()` then EXACTLY the plan's `env` pairs (no
///   inherited secrets; a poisoned parent env never reaches the decoder — T9b, G29 rule (b1)); the
///   `LD_PRELOAD`/`DYLD_*` loader-var strip on the PLAN side is P4.14's (§3.5 builds the pairs);
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
/// best-effort kill → `Cancelled` (single-process kill here; the whole-GROUP teardown + the
/// kill↔cleanup↔no-partial ordering are P4.10/P4.11, layered on this entry). `StdinPlan::PipeBytes` is
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

    // The §2.12.3 cheap-tier spawn — ONE fluent chain so the G29 rule-(b1) chain-anchored env-scrub
    // (`.env_clear()` FIRST on the builder's own chain) is structurally visible to the SAST. stdout/stderr are
    // now PIPED (the P4.8 re-cut) for the per-`ProgressModel` handling above; kill_on_drop: a dropped child
    // (the cancel arm below, or a caller drop) is killed, never orphaned (the whole-group descendant teardown
    // is P4.10's process-wrap layer). G29 rule (d) (macOS stage_for_tcc-before-spawn) does NOT reach this
    // cross-platform floor: its P4.85-refined form is `paths:`-scoped to the macOS isolation module
    // (`isolation/macos.rs` / `isolation/macos/**`), and this floor embeds no macOS-TCC path (the §3.5.0
    // staging fn + its macOS-scoped spawn land at P4.24) — so no (d) suppression is needed or present.
    // [Build-Session-Entscheidung: P4.13]
    let spawned = Command::new(program)
        .env_clear()
        .envs(
            invocation
                .plan
                .env
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        )
        .args(&invocation.plan.args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        // Spawn error (binary missing / denied) is the §2.13.1 ITEM-level fault: a runtime per-item spawn
        // failure fails that one item as InternalError (§2.13.2) — the final answer at this per-item level
        // (P4.7-resolved). The app-level EngineMissing/BundleDamaged split is the §7.2.3 startup probe's, not
        // this path (a mid-run vanished binary fails the item; the next startup probe catches a broken bundle).
        Err(_) => return ConfinedRun::failed(ConversionErrorKind::InternalError),
    };

    // Take the piped handles OUT so the two drains borrow THEM (owned) while `child.wait()` borrows `child` —
    // all three run CONCURRENTLY under one `tokio::join!` on this task, so a full stdout/stderr pipe can never
    // back-pressure the child into a deadlock (the classic "wait without draining" hang). The whole join runs
    // under the §0.4.4 cancel token: a cancel trip drops the future (freeing the borrows) and best-effort-kills
    // the child (kill_on_drop backstops; the bounded confirm-wait + residue path are P4.11).
    let child_stdout = child.stdout.take();
    let child_stderr = child.stderr.take();

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
            tokio::join!(child.wait(), stdout_fut, stderr_fut)
        })
        .await;

    match captured {
        Some((Ok(status), stdout_buf, stderr_buf)) => {
            let result = if status.success() {
                InvocationResult::Succeeded
            } else {
                // The §2.12.1 reap mapping (pre-classification floor): P4.12 routes exit≠0 through the
                // §3.5 per-engine classify_failure (over `stderr_buf`) for the precise §2.8 kind.
                InvocationResult::Failed(ConversionErrorKind::EngineCrash)
            };
            ConfinedRun {
                result,
                stdout: stdout_buf,
                stderr: stderr_buf,
            }
        }
        // The reap itself failed — an internal fault, never a panic (the crate no-panic policy).
        Some((Err(_), _, _)) => ConfinedRun::failed(ConversionErrorKind::InternalError),
        None => {
            // User cancel: best-effort kill (kill_on_drop backstops); the item is Cancelled and the
            // §1.7 caller discards the partial temp (§3.2.2).
            child.kill().await.ok();
            ConfinedRun::cancelled()
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
    // runs under `run_until_cancelled`, so a cancel drops the whole join and best-effort-kills the child
    // without waiting for it to exit (`run_confined` returns as soon as the token trips — measured ~105 ms,
    // NOT the child's lifetime). This is the NEW P4.8 path the pre-tripped-token tests don't reach: they never
    // enter the join, so they never exercise dropping the ACTIVE drains + wait on cancel.
    //
    // The shell blocks via a grandchild (`ping`/`sleep`). On this cheap-tier floor only the IMMEDIATE child is
    // killed (the whole-GROUP `process-wrap` kill that reaps descendants is P4.10) — and on Windows the
    // grandchild INHERITS the shell's stdout/stderr pipe handles (std-stream redirection does not defeat
    // Win32 handle inheritance), so the drain's blocking read only sees EOF when the orphaned grandchild
    // finally exits. That does not delay `run_confined` (it already returned on cancel) — it only holds test
    // TEARDOWN until the grandchild dies, so the grandchild is kept SHORT (~1 s). On Linux the pipe fds are
    // CLOEXEC (the grandchild never inherits them) so teardown is immediate; P4.10's group-kill makes it
    // immediate everywhere. The lingering orphan is a P4.10 concern, not a run_confined-responsiveness one.
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
}
