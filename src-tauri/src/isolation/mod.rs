//! `crate::isolation` — the §2.12 decoder-isolation wrapper every engine SUBPROCESS spawn routes through,
//! and the SOLE sanctioned `std::process::Command::new` site in the codebase: the G9 repo-invariant (b)
//! scopes its qualified `process::Command::new` grep to this module, and the G29 spawn rule excludes
//! `**/isolation/**` from the spawn-outside-isolation ban — keeping every spawn inside this module is what
//! makes those two gates honest. A §0.7 tier-2 module: the §1.7 invocation lifecycle CALLS it and §3.5
//! builds the engine args INSIDE it; it depends DOWN only, never up on IPC / orchestrator / the engine
//! registry. Unsafe-free — the crate-root `#![deny(unsafe_code)]` (main.rs) covers it; the §2.12.3
//! privilege-drop tier reaches its per-OS confinement through SAFE wrapper crates (`process-wrap`
//! group-kill / Job-Object teardown + the best-effort seccomp / Landlock / Seatbelt / AppContainer
//! mechanisms), so this module adds NO FFI and NO `unsafe`; the real confined-spawn wrapper is authored at
//! P4.13.
//!
//! ## P3.2 public-surface contract map — the confined-spawn entry's signature AND body land at P4.13
//! [Build-Session-Entscheidung: P3.2] As in `crate::fs_guard` / `crate::run` (P3.1.1 / P3.1.2), this root
//! is a documented CONTRACT MAP, not a callable body — the box title's "interface shell" is the public
//! SURFACE this module will expose, declared so P4.13 EXPANDS it (never a spawn stub that P4 would rebuild).
//! Two facts make P3.2 a map, not code:
//!  1. The one entry point has NO honest non-spawn body: a `run_confined` that confines and runs nothing is
//!     the rejected quiet-stub / lie (CLAUDE §5) — the P2.25 "the shell returns the SAME value the real body
//!     returns" and P2.62 "a genuinely-correct zero result" honesty checks both FAIL for it (a confined
//!     runner that runs nothing is not a correct outcome, it is broken; and the real P4 body spawns and
//!     returns Succeeded / Failed / Cancelled, never a fabricated value). So — the P2.74 "author the type,
//!     never a half-body" rule generalised to the spawn seam — its signature AND body co-land in the P4.13
//!     fill-box, exactly as P3.1.1 generalised it across `fs_guard`.
//!  2. Its natural signature references the §1.7 `EngineInvocation` input and `InvocationResult` output that
//!     `crate::engines` authors at P3.4, AFTER this box — so the entry cannot even be TYPED at P3.2.
//!
//! The confined-spawn entry P4.13 EXPANDS this map into:
//!  - `run_confined(inv: &EngineInvocation, ...) -> InvocationResult` — the §1.7 confined-spawn entry every
//!    SUBPROCESS engine invocation routes through: the §2.12.1 OS process boundary + the §1.7 / §2.12.2
//!    timeout + the §1.7 whole-group kill (`process-wrap` Job-Object / process-group teardown of the engine
//!    AND its descendants, e.g. `soffice` → `soffice.bin`) + the §2.12.3 cheap-tier floor + the best-effort
//!    §2.12.3 privilege-drop tier. It never runs the §2.1 publish — that is `crate::fs_guard`, invoked by
//!    the §1.7 lifecycle after a `Succeeded` return; the §0.9 pool permit is acquired one layer up (§1.7).
//!    Authored by **P4.13** (the cheap floor, spawn routed via `process-wrap` P4.10) + **P4.14** (the
//!    loader-var strip) + **P4.15 / P4.16 / P4.17** (the per-OS privilege-drop legs) + **P4.18** (the
//!    achieved-tier record into `privilege-drop-coverage.toml`).
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
//! in-core engine, so no subprocess `Invocation` is ever produced (the engine registry + the subprocess
//! engines land in P4, the registry itself at P4.4) — and return the honest §2.13
//! `ConversionErrorKind::InternalError` outcome, routing to this module's `run_confined` entry after P4.13
//! authors it.
