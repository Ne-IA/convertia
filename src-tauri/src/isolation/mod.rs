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
//!  - [`run_confined`]`(inv: &EngineInvocation, program: &Path) -> InvocationResult` — the §1.7
//!    confined-spawn entry every SUBPROCESS engine invocation routes through: the §2.12.1 OS process
//!    boundary + the §2.12.3 cheap-tier floor (P4.13, below). The remaining layers land on THIS entry at
//!    their boxes: the §1.7 whole-group kill (`process-wrap` Job-Object / process-group teardown of the
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

use tokio::process::Command;

use crate::engines::{EngineInvocation, InvocationResult, StdinPlan};
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
///   resolved input path + the `tmp` output path — never a scannable directory) and null stdio (the
///   per-`ProgressModel` stdout/stderr handling is P4.8's re-cut).
///
/// Exit mapping (the pre-classification floor): clean exit → `Succeeded` (the §1.7 non-empty output
/// verification runs conductor-side on that path, the P3.48 re-cut); a non-success exit →
/// `Failed(EngineCrash)` (§2.12.1's reap mapping — P4.12 routes exit≠0 through the §3.5 per-engine
/// `classify_failure` for the precise §2.8 kind); a spawn error (binary missing/denied) →
/// `Failed(InternalError)` (P4.7's machine refines the §2.13 spawn-error split); a cancel trip →
/// best-effort kill → `Cancelled` (single-process kill here; the whole-GROUP teardown + the
/// kill↔cleanup↔no-partial ordering are P4.10/P4.11, layered on this entry). `StdinPlan::PipeBytes` is
/// unreachable-by-construction until the §3.5.4 pandoc adapter (P7) wires its byte feed — the honest
/// `InternalError` seam (the P2.25 precedent), matched exhaustively so the arm cannot be silently
/// dropped. [Build-Session-Entscheidung: P4.13]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the §1.7 engines::dispatch subprocess arms route through run_confined when the P4.32 program-path resolution supplies the resolved binary path this entry takes (no resolvable subprocess program exists before then — no subprocess engine is registered until P5-P7); the cfg(test) real-subprocess suite below exercises every arm, keeping the test build dead-code-clean. expect (not allow) auto-flags the moment the wiring lands."
    )
)]
pub async fn run_confined(invocation: &EngineInvocation, program: &Path) -> InvocationResult {
    // §2.12.3(a): the scratch working directory is MANDATORY on a confined spawn.
    let Some(cwd) = invocation.plan.cwd.as_deref() else {
        return InvocationResult::Failed(ConversionErrorKind::InternalError);
    };
    match invocation.plan.stdin {
        StdinPlan::None => {}
        // No PipeBytes engine is registered before the §3.5.4 pandoc adapter (P7), which owns the byte
        // feed — the honest unreachable-by-construction seam (P2.25). [Build-Session-Entscheidung: P4.13]
        StdinPlan::PipeBytes => {
            return InvocationResult::Failed(ConversionErrorKind::InternalError);
        }
    }

    // The §2.12.3 cheap-tier spawn — ONE fluent chain so the G29 rule-(b1) chain-anchored env-scrub
    // (`.env_clear()` FIRST on the builder's own chain) is structurally visible to the SAST. kill_on_drop:
    // a dropped child (the cancel arm below, or a caller drop) is killed, never orphaned (the whole-group
    // descendant teardown is P4.10's process-wrap layer). G29 rule (d) (macOS stage_for_tcc-before-spawn)
    // does NOT reach this cross-platform floor: its P4.85-refined form is `paths:`-scoped to the macOS
    // isolation module (`isolation/macos.rs` / `isolation/macos/**`), and this floor embeds no macOS-TCC
    // path (the §3.5.0 staging fn + its macOS-scoped spawn land at P4.24) — so no (d) suppression is needed
    // or present. [Build-Session-Entscheidung: P4.13]
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
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        // Spawn error (binary missing / denied): the P4.7 lifecycle machine refines the §2.13
        // Failed-vs-AppFault split; the floor answers the honest internal fault.
        Err(_) => return InvocationResult::Failed(ConversionErrorKind::InternalError),
    };

    // Await exit under the §0.4.4 cancel token (tokio-util's run_until_cancelled — no select! macro
    // feature needed): a cancel trip kills the child best-effort and reports Cancelled (the partial
    // out_tmp is dropped by the §1.7 caller, §3.2.2; the bounded confirm-wait + residue path are P4.11).
    match invocation.cancel.run_until_cancelled(child.wait()).await {
        Some(Ok(status)) => {
            if status.success() {
                InvocationResult::Succeeded
            } else {
                // The §2.12.1 reap mapping (pre-classification floor): P4.12 routes exit≠0 through the
                // §3.5 per-engine classify_failure for the precise §2.8 kind.
                InvocationResult::Failed(ConversionErrorKind::EngineCrash)
            }
        }
        // The reap itself failed — an internal fault, never a panic (the crate no-panic policy).
        Some(Err(_)) => InvocationResult::Failed(ConversionErrorKind::InternalError),
        None => {
            // User cancel: best-effort kill (kill_on_drop backstops); the item is Cancelled and the
            // §1.7 caller discards the partial temp (§3.2.2).
            child.kill().await.ok();
            InvocationResult::Cancelled
        }
    }
}

// §6.4.1/§6.4.2 (G15): the §2.12.3 cheap-tier floor exercised against a REAL subprocess + a REAL temp
// filesystem — the isolation LAYER is never mocked (test-strategy §0.1). The child is the platform shell
// at its ABSOLUTE System32//bin path (PATH is never relied on — the confined env has none).
#[cfg(test)]
mod confined_spawn_tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

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

    // A confined envelope running `script` through the platform shell in `cwd`, returning the envelope +
    // the resolved absolute program path run_confined takes (the P4.32 seam, caller-supplied).
    fn confined_shell_invocation(
        script: &str,
        cwd: Option<PathBuf>,
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
                progress: ProgressModel::CoarseSpawnDone,
                out_tmp: None,
            },
            cancel: CancellationToken::new(),
        };
        (envelope, program)
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
            run_confined(&ok, &program).await,
            InvocationResult::Succeeded
        );
        let (bad, program) =
            confined_shell_invocation(EXIT_THREE, Some(scratch.path().to_path_buf()));
        assert_eq!(
            run_confined(&bad, &program).await,
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
            run_confined(&envelope, &program).await,
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
        let script = "%SystemRoot%\\System32\\ping.exe -n 30 127.0.0.1 >nul";
        #[cfg(unix)]
        let script = "while :; do :; done";
        let (envelope, program) =
            confined_shell_invocation(script, Some(scratch.path().to_path_buf()));
        envelope.cancel.cancel();
        assert_eq!(
            run_confined(&envelope, &program).await,
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
            run_confined(&envelope, &missing).await,
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
            run_confined(&envelope, &program).await,
            InvocationResult::Failed(ConversionErrorKind::InternalError)
        );
    }

    // §2.12.3(a) (G15): a missing cwd on a confined spawn is a mis-built plan — refused BEFORE any
    // spawn (the scratch working dir is the floor's own mandate, never inherited).
    #[tokio::test]
    async fn a_missing_cwd_is_a_mis_built_plan() {
        let (envelope, program) = confined_shell_invocation(EXIT_ZERO, None);
        assert_eq!(
            run_confined(&envelope, &program).await,
            InvocationResult::Failed(ConversionErrorKind::InternalError)
        );
    }
}
