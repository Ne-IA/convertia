# P4 — Engine & Bundling Framework

> **The reusable harness every format engine plugs into.** P4 builds the generic
> engine-invocation layer, per-OS sidecar packaging/bundling, the §2.12 decoder-
> isolation boundary, the §0.9 subprocess pool, the cross-cutting reliability test
> machinery (per-pair runner + pair-status ledger + corpus↔pair bijection guard), the
> SBOM/NOTICE scaffold, the §7.2.3 startup engine-presence/integrity verifier, the §3.4
> patent-disposition matrix + availability wiring, the §3.9 size-budget levers, and the
> **generic UX-correctness primitives** (options-panel shell, lossy notes, progress/
> cancel, result-actions, error copy, structural a11y) — so each later engine phase
> (P5–P7) registers per-format declarations against an already-built UI and reaches its
> §6.5 `reliable` gate without waiting on P8.
>
> Spec home: 01-conversion-pipeline (§1.7 the generic engine-invocation lifecycle —
> §P4.1, P4.6–P4.12; §1.10 the resource pre-flight & budgets engine — §P4.15, P4.71/P4.72),
> 03-engines-and-bundling (engine-invocation layer, per-OS bundling, image-
> worker `convertia-imgworker`, §3.4 patent matrix + §3.4.4a availability wiring, §3.5.0
> macOS TCC staging, §3.9 size levers), 02-guarantees (§2.12 isolation, §0.9 pool,
> §2.12.3 privilege-drop, §0.11 threat-map, §2.13 app-fault, §2.8/§2.9 UX primitives),
> 06-build-test-release (§6.4.3/§6.4.3a/§6.5 reliability machinery, SBOM scaffold, §6.1.3
> build assertions), 07-app-shell (§7.2.3 integrity manifest + startup verifier, §7.2.6
> macOS TCC), 05-ui-ux (generic UX primitives). Index: [README.md](README.md). Box
> format: [`_format.md`](_format.md).
>
> **Exit criterion (proof-of-life):** `convertia-imgworker` boots, a round-trip
> invocation succeeds through the §2.12 isolation boundary, the startup verifier reports
> a populated `EngineHealth`, the §6.4.3 runner + pair-status ledger + §6.4.3a bijection
> guard produce their first report, **and** a representative **in-P4 image round-trip**
> (a P4-staged minimal fixture + throwaway pair, NOT a P5 corpus pair — so P4 is
> self-contained, no P4→P5 inversion) is driven end-to-end through the P4-built
> options-panel shell + progress/cancel + result-actions UI to a first ledger cell over
> the P4 fixture (the UX-harness leg — P4 is not "done" on the engine side alone).
>
> **This is the v0 base** — atomic `[ ]` boxes below; a later adversarial review will
> deepen, split and complete it. P4 does **not** re-implement `crate::fs_guard` (built in
> P3); it fills the `crate::isolation` + pool shells P3 established. Per-engine SSRF/LFR
> hardening (FFmpeg/pandoc/LibreOffice/librsvg) lives in P5/P6/P7, not here; per-engine
> SBOM rows / §6.1.3 assertion lists / §7.2.3 availability rows are populated by P5–P7
> against the generic frameworks built here.

---

### Engine-registry seam & the `Engine` trait

- [ ] **P4.1** [RUST] Define the `Engine` trait — id/descriptor/capabilities/plan/plan_encode/classify_failure · §3.2.2 · G29
  needs: P3.4
  > the §3.2.2 trait shape + semantics in `engines/registry.rs`: `fn id() -> EngineId`, `fn descriptor() -> EngineDescriptor`, `fn capabilities(Platform, &PatentDisposition) -> Vec<EngineCapability>`, `fn plan(&job, &out_tmp) -> Result<Invocation, PlanError>`, the two-phase `fn plan_encode(&job, &out_tmp, &ProbeOutput)` default-impl returning the `InternalError` PlanError, `fn classify_failure(ExitStatus, &str) -> ConversionErrorKind`. NO `progress_model()` method (progress is per-Invocation). `Send + Sync`. (Build-order: expands the P3 §1.7 dispatch-stub interface shell P3.4 established — `needs: P3.4`, the cross-phase edge wired here per the P4.76 reconciliation obligation.)
- [ ] **P4.2** [RUST] Define the engine-layer supporting types — `Invocation`/`EngineProgram`/`StdinPlan`/`TempPath`/`PlanError`/`ProbeOutput` · §3.2.2 · G29
  needs: P4.1
  > the §3.2.2 named structs/enums: `Invocation { program, args, cwd, env, stdin, progress, out_tmp: Option<TempPath> }`; `EngineProgram::{Sidecar(EngineId), ResourceBin{engine,rel}, InProcessNative(EngineId)}`; `StdinPlan::{None, PipeBytes}`; `TempPath = tempfile::TempPath`; `PlanError { kind, detail }`; `ProbeOutput { duration_us, inner_codecs, rotation_deg, interlaced }`. `out_tmp: None` semantics for the read-only probe documented.
- [ ] **P4.3** [RUST] Define `ProgressModel` + the engine-layer leaf types (`Platform`/`Direction`/`EngineCapability`) · §3.2.2 · G29
  needs: P4.1
  > `ProgressModel::{FfmpegKeyValue{duration_us}, VipsStdout, CoarseSpawnDone, InProcessFraction}` with per-variant dispatch semantics; `Platform::{Win,MacOS,Linux}`; `Direction::{Decode,Encode,Both}`; `EngineCapability { source, target, direction }`; the `SourceFmt = UserFacingFormat` / `TargetFmt = TargetId` aliases (§0.6-owned vocabulary).
- [ ] **P4.4** [RUST] Build the engine registry + `select()` static-lookup algorithm · §3.2.3 §0.6 · G29
  needs: P4.1, P4.3
  > the §3.2.3 `HashMap<(SourceFmt,TargetFmt), EngineId>` built at startup from each engine's `capabilities()` filtered by the resolved `PatentDisposition`; `select(src,tgt,plat) -> Option<EngineId>` = lookup + `available_on(plat,patents)` filter; the single legitimate `None` = §3.4 codec-unavailable → `PlatformUnavailable` (§2.8). NO fallback engine chain (single owner per pair). (`needs: P4.3` for the `EngineCapability`/`Platform`/`SourceFmt`/`TargetFmt` leaf types — NOT the P4.7 spawn-lifecycle state machine, which is a CONSUMER of the registry, not a prerequisite.)
- [ ] **P4.5** [RUST] Wire the `EngineId → serialised_only` data path for the pool · §3.2.2 §0.9 · G29
  needs: P4.4
  > the pool reads `registry.engine(id).descriptor().serialised_only` before dispatch (or a pre-computed `HashMap<EngineId,bool>` at registry-build time) — the named path §0.9 depends on, no descriptor-less lookup gap; consumed by the §0.9 single-permit semaphore wiring (P4.20).

### P4.1 — Generic invocation lifecycle (§1.7)

- [ ] **P4.6** [RUST] Build the `EngineInvocation` dispatch envelope + `InvocationResult` · §1.7 · G29
  needs: P4.2
  > the §1.7 dispatch envelope `EngineInvocation { job: JobId, engine: EngineId, plan: Invocation, cancel: CancellationToken }` (wraps the §3.2.2 `Invocation`, re-declares no argv/cwd/env); `InvocationResult::{Succeeded, Failed(ConversionErrorKind), Cancelled}`. NOT a second plan type.
- [ ] **P4.7** [RUST] Build the generic spawn lifecycle state machine (spawn→Running→exit/timeout/cancel/spawn-error) · §1.7 §2.12 · G29 G31
  needs: P4.6, P4.13
  > the §1.7 per-item state machine routed **through the §2.12 isolation wrapper**: spawn on the Tokio runtime (`tokio::process`), Running, exit-0→verify-output→Succeeded, exit≠0/stderr-classified→Failed(kind), timeout/no-progress→kill→Failed(EngineHang), user-cancel→kill→Cancelled, spawn-error (binary missing/denied)→Failed/AppFault (§2.13). Sole owner of the lifecycle skeleton.
- [ ] **P4.8** [RUST] Build the per-`ProgressModel` stdout/stderr handling dispatch · §1.7 §3.2.2 §1.11 · G29
  needs: P4.7, P4.3
  > streaming models (`FfmpegKeyValue`/`VipsStdout`/`InProcessFraction`) → line-by-line stdout reader → normalised `ConversionEvent::ItemProgress` over the §0.4.2 Channel; `CoarseSpawnDone` → buffer stdout in full, no line reader attached (so the single-JSON-blob probe parse is not corrupted); stderr captured in full for exit-classification + §7.5 echo + §2.13 classify-into-§2.8.
- [ ] **P4.9** [RUST] Build the two-step probe-then-encode sequencing (call plan→spawn probe→parse ProbeOutput→plan_encode→spawn encode) · §1.7 §3.2.1 · G29
  needs: P4.8
  > the §1.7/§3.2.1 two-phase contract for a probe-requiring engine: call `plan()` (returns the probe `Invocation`, `out_tmp:None`, `CoarseSpawnDone`), spawn it, buffer-and-JSON-parse stdout into `ProbeOutput`, call `plan_encode(job,out_tmp,&probe)`, spawn the encode (`FfmpegKeyValue{duration_us:probe.duration_us}` built in `plan_encode`, never mutated onto a pre-probe struct); both legs share the cancel/timeout/group-kill machinery; NO atomic-publish/cleanup for the probe leg (`out_tmp.is_none()`).
- [ ] **P4.10** [RUST] Build the cross-platform process-group / job-object spawn + whole-group kill (process-wrap) · §1.7 · G29 G9
  needs: P4.7
  > the §1.7 sole-owner cancel/kill mechanism: wrap each spawn with `process-wrap` over `tokio::process` — Windows Job Object (kill-on-close, `CreationFlags`/`KillOnDrop` shims), POSIX `ProcessGroup::leader()` (`setpgid`, negative-pgid SIGKILL); forceful group-kill (no cooperative drain) tears down the engine + all descendants; never routes through `tauri_plugin_shell` (no `shell:allow-execute`). Lives in `crate::isolation` so G9 invariant (b) `std::process::Command::new` outside `crate::isolation` holds.
- [ ] **P4.11** [RUST] Build the kill↔cleanup↔no-partial ordering + the bounded confirm-wait + deferred-reclaim residue path · §1.7 §2.6 · G29 G31
  needs: P4.10
  > the §1.7 ordering: signal cancel → group-kill + **timeout-bounded** confirm-wait (so a wedged descendant cannot hang the UI/quit) → on-timeout defer temp reclaim to the §2.6 sweep AND carry `CleanupResidue` (the §2.8.2 "With residue" tail, never a silent leftover) → §2.6 cleanup of the per-job temp → mark Cancelled/Failed and continue the queue (§1.9). Already-`Succeeded` items untouched.
- [ ] **P4.12** [RUST] Build the timeout / no-progress watchdog + exit & output verification (non-empty temp) · §1.7 §0.9 · G29 G31
  needs: P4.7
  > the §1.7 watchdog (per-engine no-progress interval, parameters from §0.9) → kill → `Failed(EngineHang)`; exit-0 reports success **only if** the expected temp output exists and is non-empty (the "exit 0 but empty/zero output" guard); exit≠0 / stderr-classified → §2.8 taxonomy via the §3.5 per-engine `classify_failure`.

### P4.2 — The §2.12 decoder-isolation wrapper (`crate::isolation`)

- [ ] **P4.13** [RUST] Build the `crate::isolation` cheap-tier floor (process boundary + minimal/cleared env + scratch-cwd + input/tmp-only handing) · §2.12.1 §2.12.3 · G29 G9
  needs: P3.2
  > the §2.12.3 NON-NEGOTIABLE v1 floor every engine spawn routes through: the §2.12.1 process boundary, a minimal/cleared environment (no inherited secrets), working-dir = the per-run scratch dir (§2.6), the engine handed **only** the exact input path + the `tmp` output path (not a scannable dir). Fills the P3 interface-only `crate::isolation` shell P3.2 established — `needs: P3.2`, the cross-phase edge wired here per the P4.76 reconciliation obligation. Spawn routed via `process-wrap` (P4.10).
- [ ] **P4.14** [RUST] Strip the dynamic-loader injection vars in the minimal env (LD_PRELOAD/LD_LIBRARY_PATH/DYLD_*) · §3.5 §2.12.3 §0.11 · G29
  needs: P4.13
  > the §3.5/§2.12.3 minimal-env STRIP of `LD_PRELOAD`/`LD_LIBRARY_PATH` (Linux), `DYLD_INSERT_LIBRARIES`/`DYLD_LIBRARY_PATH` (macOS) so a hostile input cannot coerce a side-load (T3a); `PATH` not relied on (absolute bundled paths, §3.3.3); the env-whitelist seam for the per-engine vars (`LIBHEIF_PLUGIN_PATH`/`MAGICK_CONFIGURE_PATH`/`VIPS_BLOCK_UNTRUSTED`) added by P5.
- [ ] **P4.15** [RUST] Build the Linux privilege-drop tier (the three independent kernel-subsystem legs, each silent-degrade) · §2.12.3 · G42 G42b
  needs: P4.13
  > the §2.12.3 best-effort Linux tier as the parent of three **independent kernel subsystems** — each with its own build-time availability probe, runtime silent-degrade, and test coverage (a Landlock failure does not imply a net-namespace/seccomp failure, so each is separately built + checked off). Best-effort, not load-bearing (the §3.5/§6.1.3 argv/build controls are); together they activate the G42/G42b enforcement SUBSTRATE for the read-half fs-audit. **Degrades silently to the cheap tier** (P4.13) where the kernel/portable-build can't enable a leg.
  - [ ] **P4.15.1** [RUST] Build the Landlock fs-restrict leg (ABI≥1 probe + `{input ro, tmp rw}`) · §2.12.3 · G42b
    > Landlock (kernel ≥ 5.13, the `landlock` crate) restricting the decoder FS to `{input ro, tmp rw}`; **probe ABI ≥ 1 first** + assert the ruleset applied (the grant-is-enforcement model — never assume the grant took); silent-degrade to the cheap tier where Landlock is absent.
  - [ ] **P4.15.2** [RUST] Build the network-namespace egress-deny leg (`unshare --net`, loopback-only, preflight probe) · §2.12.3 · G42
    > network deny via a **network namespace** (`unshare --net`, loopback-only) — NOT seccomp socket-filtering; a preflight probe for unprivileged userns (`unshare --net true`) with fail-loud-not-silent-skip; silent-degrade to the cheap tier where unavailable.
  - [ ] **P4.15.3** [RUST] Build the seccomp-bpf exec-deny leg (deny exec/unexpected syscalls, defence-in-depth) · §2.12.3 · G42b
    > seccomp-bpf denying `exec`/unexpected syscalls as defence-in-depth; independent of the Landlock + net-ns legs; silent-degrade where the seccomp filter cannot be installed.
- [ ] **P4.16** [RUST] Build the macOS privilege-drop tier (Seatbelt/sandbox profile, silent-degrade to cheap on unsigned portable) · §2.12.3 · G42 G42b
  needs: P4.13
  > the §2.12.3 best-effort macOS tier: a `sandbox-exec`/Seatbelt SBPL profile restricting the engine to read-input + write-scratch, deny network + process-exec; **explicitly accepted** that on an unsigned portable build it most often degrades to the cheap tier (`sandbox_init` is private/unsupported) — not load-bearing, T9b/offline do not depend on it.
- [ ] **P4.17** [RUST] Build the Windows privilege-drop tier (restricted-token/AppContainer + low-integrity + Job-Object resource caps + AppContainer/WFP net-deny) · §2.12.3 · G42 G42b
  needs: P4.13, P4.10
  > the §2.12.3 best-effort Windows tier: restricted token / AppContainer + low-integrity token inside a Job Object with `JOB_OBJECT_LIMIT` (kill-on-job-close, memory cap); network denied by an AppContainer network-isolation profile OR a per-program WFP/Firewall outbound-block rule (NOT the Job Object, which cannot restrict sockets); silent-degrade to cheap tier.
- [ ] **P4.18** [RUST] Record the §2.12.3 achieved privilege-drop tier per platform into `privilege-drop-coverage.toml` · §2.12.3 · G64
  needs: P4.15, P4.16, P4.17
  > emit the per-platform achieved tier into the tracked `privilege-drop-coverage.toml` the §2.12.3/G64 decrease-guarded ratchet (policy authored in P0.7.14) reads; the per-run tier-APPLIED regression assertion is the G31 leg homed in P0.5.9 — this box produces the data it asserts against. (The §2.12.3 per-OS profile *contents* are `[DEFER: tuning]`; the tier model is built here.)
  - [ ] **P4.18.1** [TEST] Instantiate the P0.5.9 isolation/privilege-drop activation homes — tier-APPLIED-per-spawn regression + §2.12.3 memory-cap kill + process-group/Job-Object reap · §2.12.3 §2.8 · G31 G64
    needs: P4.18, P4.10, P0.5.9
    > the activation target for the P0.5.9 `→ activated in … P4` isolation/privilege-drop arm (the P4 leg of the cross-cutting security-test home, resolved here where the §2.12 wrapper + tiers land): a per-spawn regression assertion that the achieved tier from `privilege-drop-coverage.toml` (P4.18) is actually APPLIED on each engine spawn (§2.12.3 — the cheap-tier floor always, the best-effort tier where it works); the **§2.12.3 memory-cap kill** → the offending item reported `Failed` (§2.8) while the **batch continues** (§1.9); and the **process-group / Job-Object reap** assertion (no orphaned descendant survives a kill, P4.10). Distinct from the §7.5 log-redaction arm (P2.127) and the macOS-T11/ratchet validation (P9.40/P9.42 — which CALIBRATE; this INSTANTIATES the per-run regression). This is the P4 box the P0.5.9 isolation/privilege-drop edge points at (`needs: P0.5.9`, the P0 home is `[x]` before the loop). → activates the P0.5.9 isolation/privilege-drop homes.
- [ ] **P4.19** [RUST] Assert detection's in-core untrusted-byte boundary holds (no third-party C/C++ decoder in-core) · §2.12.4 · G29 G48
  needs: P4.13
  > the §2.12.4 absolute as a build/lint assertion: every full decode runs in a subprocess; the in-core untrusted-byte operations (detection sniffs P3, the native CSV/TSV transform §3.5.6) are pure memory-safe Rust with no third-party C/C++ decoder linked into the core — the G53 forbidden-dep gate (P0.3.7) + G29 unsafe-policy are the enforcers; this box wires the §2.12.4 confirmation that the image core runs in the separate worker (P4.30), not in-core.

### P4.3 — Subprocess pool & concurrency degree (§0.9)

- [ ] **P4.20** [RUST] Expand the P3 pool shell into the bounded engine-subprocess pool + global concurrency degree · §0.9 · G29
  needs: P4.5, P3.3
  > fill the P3 interface-only pool shell P3.3 established (`needs: P3.3`, the cross-phase edge wired here per the P4.76 reconciliation obligation): `global_degree = clamp(physical_cores−1, 1, 4)`; the bounded pool governing how many engine processes run at once; `effective = min(global_degree, per_engine_cap)`; per-`(InstanceId,RunId,ItemId)` binding to per-run scratch so parallel jobs never collide on temp. P4 fills the shell P3 established (does not build from scratch).
- [ ] **P4.21** [RUST] Wire the per-engine parallelism caps (LibreOffice serialised-1, video re-encode 1–2, image/poppler/pandoc/CSV up to degree) · §0.9 · G29
  needs: P4.20
  > the §0.9 per-engine caps overriding the global degree downward: LibreOffice serialised exactly 1, FFmpeg video re-encode 1–2, FFmpeg audio/remux + image-worker + poppler + pandoc + native CSV/TSV up to global degree. (FFmpeg/libvips internal-threading oversubscription levers `[DEFER: profile]`.)
- [ ] **P4.22** [RUST] Build the `serialised_only` single-permit-semaphore enforcement + the `MAX_LO_CONCURRENCY` const · §0.9 · G29
  needs: P4.21, P4.5
  > the §0.9 mechanism: a dedicated single-permit `Semaphore` per serialised engine allocated at registry-build time; a serialised-engine job acquires BOTH the global-degree permit AND the engine's single-permit before spawn, releasing both on exit; `MAX_LO_CONCURRENCY = 1` as the §0.9-owned `pub const` (single source, imported by the §6.7.2 test harness, never hard-coded).
- [ ] **P4.23** [RUST] Attach the P3-built `InProcessNative` lane (P3.43–P3.45) to the now-real bounded pool · §1.7 §3.5.6 · G29
  needs: P4.20, P4.8, P3.43, P3.44, P3.45
  > **Does NOT rebuild the P3 InProcessNative lifecycle — only re-homes it on the real pool.** P3 already built the full §1.7 `InProcessNative` lifecycle for the one non-subprocess engine (native CSV/TSV): the bounded `spawn_blocking` execution + `mpsc::Sender<f32>` `progress_tx`→`ItemProgress` (P3.43), the cooperative chunk-boundary cancel that drops `out_tmp` (P3.44), and the wall-clock timeout → `Failed(EngineHang)` + the wedged-uninterruptible-read bounded-pool/short-per-read-deadline caveat (P3.45) — against the P3 interface-only pool shell. This box RE-HOMES that already-built lane onto the now-real bounded engine-subprocess pool (P4.20): register the InProcessNative engine to acquire a global-degree permit (no serialised lane) from the real pool, so the P3-built `spawn_blocking` execution shares the pool's bounded budget + headroom rather than the P3 shell's stub. (`needs: P3.43/P3.44/P3.45` — the P3-built lifecycle this re-homes, NOT a second build; the cross-phase edge wired here per the P4.76 reconciliation obligation.)

### P4.4 — macOS TCC source staging (§3.5.0 / §7.2.6)

- [ ] **P4.24** [RUST] Build the macOS TCC source-staging copy (core copies source into per-job kind-2 scratch before spawn) · §3.5.0 §7.2.6 §0.11 · G29 G31
  needs: P4.13, P3.21
  > the §3.5.0/§7.2.6 read-side staging (macOS-only, `cfg(target_os="macos")`): the core (which holds the TCC grant from the §1.1 freeze) copies the source into a per-job §2.14.2 kind-2 scratch path **before** spawning, so the engine is never the first process to touch a protected Desktop/Documents/Downloads/removable path (T11); composes with the §2.14 cross-volume strategy. (Build-order: the §2.14.2 kind-2 scratch-ROOT primitive is the P3 `crate::run` run-lifecycle layer P3.21 — `needs: P3.21`, the cross-phase edge wired here per the P4.76 reconciliation obligation.)
- [ ] **P4.25** [RUST] Hand engines the staged scratch path, never the raw protected path (per-engine input-arg/handle plumbing) · §3.5.0 §7.2.6 · G29
  needs: P4.24
  > the §3.5.0 engine-arg plumbing: FFmpeg/poppler/LibreOffice get the scratch source as `<input>` (LO `--outdir` already at scratch); pandoc pipes bytes on stdin (`StdinPlan::PipeBytes`) or the scratch path; libvips/image-worker loads from the scratch path; output `out_tmp` published per §2.1, staged source reclaimed with the run (§2.6); read-side only (the write-side beside-source `.part` is core-created, a TCC denial there fails that item per §2.8).
- [ ] **P4.26** [RUST] Wire the T11 `stage_for_tcc`-before-spawn invariant for the G29 Semgrep rule · §7.2.6 §0.11 · G29
  needs: P4.25
  > make the §0.11-T11 / G29 rule satisfiable: every `Command::new` in `crate::isolation` under `cfg(target_os="macos")` is preceded by the stage-for-TCC call (the project-local Semgrep rule's enforcement target, authored in P0.4.2); + the staged-input term feeds the §1.10 `est_scratch_bytes` macOS preflight.

### P4.5 — Per-OS sidecar packaging & bundling (§3.3)

- [ ] **P4.27** [BUILD] Build the `scripts/stage-engines` skeleton — placement, externalBin triple-suffixing, resources tree, per-OS layout · §3.3.1 §3.3.2 §6.1.3 · G37
  needs: P0.4.10
  > the §3.3.2/§6.1.3 build-time assembly skeleton (run before `tauri build`): place each standalone engine at `src-tauri/binaries/<name>-<target-triple>[.exe]` (externalBin), the LibreOffice tree + fonts + image stack under `src-tauri/resources/`/`src-tauri/fonts/` (the `resources` map); per-platform layout (§3.4.5 — Windows `.exe` suffix, Linux exec-bit, macOS `.app`); the engine-asset-cache read (never the live network at package time, P4.28). Generic skeleton; per-engine staging lands in P5–P7.
- [ ] **P4.28** [BUILD,CI] Wire the pinned checksum-verified engine-asset cache (`actions/cache` keyed `<engine>-<version>-<triple>` + pinned-URL fallback) · §6.1.3 §3.8 · G37 G56
  needs: P4.27
  > the §6.1.3 cache mechanism: `actions/cache` keyed `<engine>-<version>-<triple>`, with a checksum-verified pinned-upstream-URL fetch as the populate/cache-miss path (download pinned asset → verify SHA-256 vs the in-repo pin → store under the key); `stage-engines` reads only the restored cache; the per-engine acquisition-mode + source-allow-list policy (P0.7.3/P0.7.4) is what each P5–P7 staging anchors against.
- [ ] **P4.29** [BUILD] Build the macOS universal-sidecar `lipo -create` step + the per-sidecar `lipo -info` fat-Mach-O assertion · §6.1.3 §3.4.5 · G30 G37
  needs: P4.27, P4.28
  > the §6.1.3 macOS-leg requirement: `stage-engines` (NOT Tauri — Tauri does not lipo sidecars) builds each per-arch engine and `lipo -create`s them into one `<name>-universal-apple-darwin` fat binary for the externalBin slot before `tauri build`; the dual-arch sourcing (both `aarch64`+`x86_64` slices from the cache, the cross-toolchain/Rosetta fallback for a missing slice); the per-sidecar `lipo -info` assertion (both slices present, the G30 fat-Mach-O check) so a single-arch sidecar fails the leg, never ships.
- [ ] **P4.30** [BUILD] Wire `tauri.conf.json` `bundle.externalBin` + `bundle.resources` for the engine set · §3.3.1 §0.10 · G47
  needs: P4.27
  > the §3.3.1 Tauri config: `bundle.externalBin` listing the sidecars (`binaries/ffmpeg`, `ffprobe`, `soffice`, `pdftotext`, `pandoc`, `convertia-imgworker`) and `bundle.resources` mapping the LibreOffice tree / image stack / fonts / `THIRD-PARTY-LICENSES.txt`; no `updater`/`createUpdaterArtifacts` block (the G47 structural lint, P0.3.2, enforces the absence). Generic wiring; per-engine entries filled as P5–P7 stage each.
- [ ] **P4.31** [RUST] Build the runtime program-path resolution (`current_exe().parent()` sidecars · `BaseDirectory::Resource` for resource-tree binaries) + the `EngineId→binary-name` table · §3.3.3 · G29
  needs: P4.2
  > the §3.3.3 [DECIDED] resolution: externalBin sidecars resolved by **bare name** beside the app exe via `current_exe()?.parent()` (Tauri strips the triple suffix; NEVER `BaseDirectory::Resource` for externalBin, `.exe` on Windows); resource-tree binaries (`program/soffice.bin`) via `app.path().resolve(rel, BaseDirectory::Resource)`; the fixed `EngineId → binary-name` table (`FFmpeg→"ffmpeg"`, `FFprobe→"ffprobe"`, `LibreOffice→"soffice"`, `Poppler→"pdftotext"`, `Pandoc→"pandoc"`, `ImageCore→"convertia-imgworker"`; `ImageMagick`/`NativeCsvTsv` absent — delegate / in-core). All absolute paths; `PATH` never relied on.
- [ ] **P4.32** [RUST] Build the §7.2.4 executable-permission setup (idempotent +x on extracted sidecars, Unix) · §7.2.4 · G29
  needs: P4.31
  > the §7.2.4 portable-build `ensure_executable` (Unix): on each launch set the exec bit on each engine binary if missing (`perm.mode() | 0o755`), so a portable-archive extract without +x can still spawn; Windows runs `.exe` as-is. (The macOS quarantine path → `QuarantinedByOs` is the §7.2.4/§2.8 surface, wired with the verifier in P4.45.)

### P4.6 — The image-worker `convertia-imgworker` (the first real sidecar)

- [ ] **P4.33** [BUILD,RUST] Build `convertia-imgworker` as its own externalBin binary (links the libvips/libheif/librsvg/ImageMagick stack, not the MIT core) · §3.5.5 §0.7 §3.6.1 · G53 G37
  needs: P4.27, P4.31
  > the §3.5.5/§0.7 [DECIDED] packaged artifact: a concrete `externalBin` sidecar `convertia-imgworker-<triple>[.exe]` that statically links the libvips/libheif/libde265/librsvg/ImageMagick/cgif stack, resolved Rust-side via `current_exe().parent()` (P4.31), **never linked into the MIT core** (the G53 forbidden-dep gate, P0.3.7, enforces the core does not pull the image-worker C libs). The proof-of-life sidecar.
- [ ] **P4.34** [RUST] Build the imgworker Rust↔FFI surface + the in-worker decode/encode `Invocation`-equivalent plan · §3.5.5 §3.2.2 · G29 G48
  needs: P4.33, P4.2
  > the §3.5.5 worker: calls libvips via its Rust binding on a decode/encode thread, producing an `Invocation`-equivalent plan (operation + params + `out_tmp`) so §1.7's lifecycle + §2.12's isolation wrap it uniformly; the Rust→FFI surface is the G48 imgworker fuzz target (harness layout authored in P0.4.3, instantiated in P4.34.1); `deny(unsafe_code)` outside the single allow-listed FFI module (G29).
  - [ ] **P4.34.1** [TEST] Instantiate the P0.4.3 imgworker-FFI G48 fuzz leg against the real `convertia-imgworker` Rust→FFI surface · §6.4.2 §3.5.5 · G48
    needs: P4.34, P0.4.3
    > the activation target for the P0.4.3 `→ activated in P4` imgworker-FFI fuzz leg (the third leg alongside the serde-boundary/IPC P2.126 and the detect/fs_guard P3.67 legs): stand up the cargo-fuzz target over the imgworker's **Rust→FFI surface** linked against the staged libvips/libheif/librsvg, ASAN on (honest note per P0.4.3: ASAN covers only the Rust/boundary side of the pre-compiled `.so`, not decoder internals — that depth is the G65 black-box engine fuzz, P9.36); date-pinned nightly (Linux/macOS), pinned libFuzzer resource bounds, committed crash-corpus replayed via the P3.67 stable-toolchain `tests/fuzz_replay.rs`. This is the P4 box the P0.4.3 imgworker-FFI edge points at (`needs: P0.4.3`, the harness contract is `[x]` before the loop). → activates the P0.4.3 imgworker-FFI fuzz leg.
- [ ] **P4.35** [RUST] Build the imgworker `VipsStdout` progress marshalling (eval-progress callback → stdout `progress=<0..100>` key=value) · §3.5.5 §1.11 · G29
  needs: P4.34, P4.8
  > the §3.5.5 [DECIDED] cross-process progress: the worker installs the libvips `eval` signal handler and marshals each tick to its own stdout as a `progress=<0..100>` key=value line (optional `progress=end`), parsed by the §1.7 same line-reader path as FfmpegKeyValue (the worker is a separate process — an in-process callback can't cross the boundary); sub-second ops may emit start→`progress=end` (≈CoarseSpawnDone).
- [ ] **P4.36** [RUST] Wire the image-worker through the §2.12 isolation boundary + the §0.9 image-core pool (one short-lived worker per item) · §3.5.5 §2.12.4 §0.9 · G29 G31
  needs: P4.34, P4.13, P4.20
  > the §2.12.4/§3.5.5 [DECIDED] isolation: image decode/encode runs in a separate short-lived worker process (not an in-app thread) so a libvips/libheif/librsvg crash/hang/memory-corruption is contained by the OS process boundary and fails that one item (§2.8); one worker per item up to the §0.9 image-core degree; do NOT rely on `catch_unwind` for hostile native code (§2.12.4).
- [ ] **P4.37** [RUST,TEST] Build the imgworker round-trip proof-of-life invocation through the isolation boundary · §3.5.5 §2.12 §6.4.3 · G31 G26
  needs: P4.36, P4.7
  > the P4 exit-criterion proof-of-life: a representative image round-trip invocation succeeds end-to-end through the §1.7 lifecycle + the §2.12 isolation wrapper (spawn worker → decode/encode → `VipsStdout` progress → exit-0 → non-empty `out_tmp` → §2.1 atomic publish), the first real sidecar validated; the G26/G31 corpus fault-injection oracle binds here. (Per-format image pairs + per-engine hardening are P5.)

### P4.7 — Patent-disposition matrix & availability wiring (§3.4)

- [ ] **P4.38** [DOC] Author the §3.4 patent-disposition matrix (HEIC/HEVC/AAC/H.264/AV1/legacy-decode × platform) as the single owner · §3.4 §3.4.3 §3.4.4 · G7
  needs: P0.1.1
  > the §3.4 single-owner matrix (decided here, never re-decided downstream): the ship-bundled/rely-on-OS/gate/unavailable disposition per (codec × platform) — AAC + H.264 ship-bundled all 3; HEVC decode ship-bundled (two engines — image libde265 / video FFmpeg native `hevc`); HEVC encode ship-bundled-isolated x265 **behind the §3.4 availability flag**; AVIF ship-bundled; legacy decode-only (VC-1/MPEG-2/H.263/MPEG-4-Part-2) ship-bundled-decode-only; the §3.4.4 rationale + the rely-on-OS re-evaluation gate. P5/P6 only READ the per-codec cell.
- [ ] **P4.39** [BUILD,RUST] Build the §3.4.4a `engines.lock` per-platform `available` boolean → `PatentDisposition` parse→map flow · §3.4.4a §3.2.2 · G35 G37
  needs: P4.38, P4.3, P4.55
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P4.55` points at the §3.7.2 `engines.lock` schema scaffold 16 boxes later in document order — the per-platform `available` boolean row this box reads has nowhere to land until the `engines.lock` container (P4.55) exists, so DECISION C builds P4.55 first; the edge is acyclic and valid, the inversion is documented here so it is visible at the `needs:` line.
  > the §3.4.4a [DECIDED] wiring: the per-platform `available = {win,macos,linux}` boolean on the codec's `engines.lock` row (the config-change escape hatch — flip = data edit + rebuild, not code); the startup sequence (§7.2) parses `engines.lock` once, reads each codec row's `available` for the running `Platform`, maps it into a `PatentDisposition` (`true→Available`, `false→Unavailable`) for `heic_hevc`/`aac`/`h264`, built BEFORE any `capabilities(platform, patents)` call (P4.4) and passed in. The single source of the posture, not a second truth. The patent→UI wiring (`EngineHealth.unavailable_targets`) is built with the startup verifier (P4.44), since C12 `get_engine_health` reads the resolved `available` flag.

### P4.8 — Startup engine-presence + integrity verification (§7.2.3)

- [ ] **P4.40** [BUILD] Build the build-time in-bundle hash manifest GENERATION (per-engine `{id,expected_hash,expected_size}`) · §7.2.3 §6.2 · G37 G35
  needs: P4.27
  > the §7.2.3 build-time manifest of expected per-engine hashes shipped in-bundle (the same SBOM/checksum data §3.7/§6.2 produce) — the input the warm-launch verifier consumes; generated by `stage-engines` over the staged binaries; the runtime half of the T3 supply-chain threat (corruption/integrity only, not a tamper anchor — §0.11 T3).
- [ ] **P4.41** [RUST] Build the §7.2.3 presence loop over the expected BINARY list (bare runtime names, not the trait registry) · §7.2.3 §0.4.1 · G46 G29
  needs: P4.31, P4.40
  > the §7.2.3 [DECIDED] out-of-band presence loop iterating the §3.3.1 expected bundled-binary list (`ffmpeg`/`ffprobe`/`soffice`/`pdftotext`/`pandoc`/`convertia-imgworker` — **bare names**, `.exe` on Windows, NOT triple-suffixed, NOT the `trait Engine` registry, NOT `descriptor()`), confirming each resolves + exists; `FFprobe` presence-checked but its health rolled into FFmpeg's `EngineStatus`. The §7.2.1 step-3 slot.
- [ ] **P4.42** [RUST] Build the integrity verifier — hash-on-first-launch + `engine-integrity.json` warm marker + cheap warm size/header check · §7.2.3 · G46 G29
  needs: P4.41
  > the §7.2.3 [DECIDED] strategy: first-launch (or `app_version`-mismatch) full re-hash of all engines + rewrite the `engine-integrity.json` marker (`{id,expected_hash,expected_size,app_version}`) in `app_config_dir()` (a separate file, never merged into the 3-key prefs blob); warm-launch = presence + the cheap size/header check (size == `expected_size` AND first-N-bytes match the platform exec magic — ELF/PE/Mach-O, the platform-conditional `soffice` shebang on Linux; size-only for non-binary resources); a size/header mismatch forces a re-hash of that engine.
- [ ] **P4.43** [RUST] Build the smoke probe (cheap `--version`-style run through the §2.12 wrapper) + the imgworker BMP-delegate exercise · §7.2.3 · G46 G29
  needs: P4.42, P4.36
  > the §7.2.3 smoke probe: a fast `--version`-style invocation per critical engine through the §3.5/§2.12 wrapper (catches a glibc/arch mismatch a hash can't), cheap/gated behind verbose mode on warm launches; the imgworker smoke probe MUST include a BMP-delegate exercise (a tiny `magicksave`/`magickload` BMP round-trip or `--list-formats` BMP-registered check) so a missing/corrupt ImageMagick delegate makes `ImageCore.runnable = Some(false)` at startup, never a silent per-BMP-job failure.
- [ ] **P4.44** [RUST] Populate the C12 `EngineHealth`/`EngineStatus` contract (incl. synthesized NativeCsvTsv row + `unavailable_targets` from the resolved §3.4.4a flag) · §7.2.3 §3.4.4a §0.4.1 · G46 G29
  needs: P4.43, P4.39, P2.111
  > populate the C12 `EngineHealth` (declared in P2): one `EngineStatus { id, present, integrity_ok, runnable }` per registry-eligible engine from the loop, FFprobe→FFmpeg and ImageMagick→ImageCore rolled in, the `NativeCsvTsv` row SYNTHESIZED (`{present:true, integrity_ok:true, runnable:Some(true)}`, appended after the loop); `unavailable_targets: Vec<TargetId>` reads the resolved §3.4.4a `available` flag (a target whose only encoder is `available=false` is added, e.g. HEIC-encode) + the §3.4 per-platform gaps; `all_critical_ok` derived. Feeds §5.2 disable-with-reason.
- [ ] **P4.45** [RUST] Wire the missing/corrupt/non-runnable-engine outcome — app-fault vs degrade-to-unavailable + the macOS QuarantinedByOs ordering · §7.2.3 §2.13 §2.8 · G46 G29
  needs: P4.44, P4.49
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P4.49` points at the §2.13.3 fault-presentation box 4 boxes later — the missing/corrupt-engine routing surfaces through the §2.13.3 app-fault screen, so section P4.9 (P4.49) is a prereq of this section P4.8 box; DECISION C builds P4.49 first, the edge is acyclic, the inversion is documented at the `needs:` line.
  > the §7.2.3 outcome routing: a missing/corrupt/non-runnable **required** engine → app-level startup fault (§2.13, "A required conversion component is missing or damaged…", with the §7.7 link), a single-engine-affecting-some-formats failure → mark those targets unavailable (§5.2); the macOS ordering caveat (defer the smoke probe until after the window shows / lazy on first conversion) so a `QuarantinedByOs` fault surfaces **in a window**, distinct from `EngineMissing`/`BundleDamaged`; the per-sidecar `QuarantinedByOs` retry-flow (no auto-retry, name the blocked sidecar). Back-fills the `SECURITY.md`→§0.11 threat-map ref (P4 assembles the map P1's SECURITY.md points at).

### P4.9 — App-level fault model & panic boundary (§2.13)

- [ ] **P4.46** [RUST] Build the worker-thread `catch_unwind` panic boundary (per-item isolate-and-report, `panic="unwind"`) · §2.13.2 · G29
  needs: P4.7
  > the §2.13.2 boundary: each item's core-side work in `std::panic::catch_unwind` (`AssertUnwindSafe` as needed) → a caught panic becomes `ConversionError::InternalError` for that item, batch continues; payload logged locally-only (§7.5 redacted), user sees only the calm string, no stack trace; no `resume_unwind` on the worker; `panic = "unwind"` required in `Cargo.toml` release (never `abort` for the app binary).
- [ ] **P4.47** [RUST] Build the intake/detection panic boundary (C1/C2a — per-path → Uncertain, whole-walk → calm IpcError) · §2.13.2 · G29
  needs: P4.46
  > the §2.13.2 [DECIDED] intake boundary so "no command panics across the boundary" holds for the intake path too: per-path detection in `catch_unwind` → a panic decoding one header becomes that path's `DetectionOutcome::Uncertain` (walk continues); the C1/C2a outer body wrapped so an escaped panic becomes a calm `IpcError` (a `CollectedSet`-level "couldn't read these files", never a blank window), not an unwind across the Tauri boundary.
- [ ] **P4.48** [RUST] Build the engine-stderr capture-and-classify-into-§2.8 rule (never raw to the user) · §2.13.4 §2.8 · G29
  needs: P4.8
  > the §2.13.4 rule: each engine subprocess's stderr captured (never shown raw), classified into a §2.8 kind via the §3.5 per-engine classifier (the generic seam; per-engine stderr quirks land in P5–P7); unclassifiable → generic `EngineError` calm string, raw text to the §7.5 log only.
- [ ] **P4.49** [RUST,UI] Build the §2.13.3 app-level fault presentation (single calm no-trace screen; startup/mid-run-panic/WebView-disconnect classes) · §2.13.1 §2.13.3 · G29 G33a
  needs: P4.46
  > the §2.13.1 three fault classes (item/run/app) + the §2.13.3 calm single-screen presentation (no crash dialog/trace): startup faults (engine missing/corrupt, damaged bundle, missing/old WebView, no scratch) → the §7.2-owned plain message; mid-run escaped panic → "Something went wrong… Your original files are safe and untouched."; WebView/backend disconnect → calm reconnect affordance (§5.8); the `AppFaultNotice` component (state 12) with no fabricated per-item outcomes.

### P4.10 — Generic bundle-time build assertions (§6.1.3)

- [ ] **P4.50** [BUILD] Build the generic §6.1.3 build-assertion framework hooked into `scripts/stage-engines` · §6.1.3 · G37 G38
  needs: P4.27
  > the §6.1.3 generic assertion harness `stage-engines` runs as it stages: the per-engine assertion-list slots (filled by P5–P7 per the P0.7.4 policy), the exposed-parameter capability-assertion framework (engine option names ConvertIA exposes actually exist in the staged build), and the structural plumbing (parse staged binaries, fail the build on a miss). Generic framework; per-engine assertions land in P5/P6/P7.
- [ ] **P4.51** [BUILD] Build the MIT-core LGPL shared-object-or-fail link assertion (static LGPL into the MIT core = build FAILURE) · §6.1.3 §3.6.1 · G36 G38b
  needs: P4.50
  > the §6.1.3 [DECIDED] linkage-site (i) leg: any LGPL into the **MIT core** MUST be a bundled shared object — static LGPL into the MIT core is a build FAILURE (Rust links static by default), so assert the core links no LGPL component statically. Scoped to the Rust core code object only. (Leg (ii) — the separate image-worker's static LGPL aggregation + §6 relink bundle + x265 GPL §3 corresponding source — is its own box P4.75, since it acts on a different code object with a different compliance obligation and fails independently. Leg (iii) — FFmpeg-internal static LGPL — is aggregation that NEVER fails and carries no assertion body; it is a prose note in §3.6.1/`engines.lock`, not a build check.)
- [ ] **P4.52** [BUILD] Build the libvips-no-copyleft-PDF-loader assertion (no poppler/mupdf/GPL/AGPL loader present) · §6.1.3 §3.1 §3.6.1 · G38
  needs: P4.33, P4.50
  > the §6.1.3 [DECIDED] positive assertion that the staged libvips exposes NO poppler/PDF loader (GPL, taints the whole libvips — libvips#2222), NO MuPDF loader (AGPL), and no other GPL/AGPL loader (so "libvips is LGPL" stays true; ConvertIA needs no libvips PDF loading — that's the poppler `pdftotext` sidecar); fail the build if a `pdfload`/`poppler`/`mupdf` foreign loader is registered. **Artifact + stage:** this is the **P4 proof-of-life** check on the **P4.33 imgworker's** statically-linked libvips (the first time libvips is in a built sidecar). P5.2 is the **distinct** stage-time check on the **newly-staged P5.1 libvips** before any P5 image engine builds against it — same property, different artifact at a different build stage (so each fact keeps one home, _format.md §8).
- [ ] **P4.53** [BUILD] Build the libimagequant BSD-2-leg COPYRIGHT-text assertion + the `engines.lock`/Cargo.lock fork-pin provenance check · §6.1.3 §3.1 · G36 G38
  needs: P4.33, P4.50
  > the §6.1.3 [DECIDED] guards: assert the staged `libimagequant` `COPYRIGHT` actually contains the BSD-2-Clause text (the frozen `lovell/libimagequant` v2.4.x fork) — fail if a GPLv3 leg (upstream 4.x, which would taint the LGPL worker) slipped in; PLUS a lockfile-pin provenance check that the pinned `imagequant`/`libimagequant` ref in `engines.lock`/`Cargo.lock` is exactly the lovell v2.4.x-fork commit (it is statically vendored — no soname, so a provenance check, not an ABI/soname check).
- [ ] **P4.54** [BUILD] Build the libheif-resolves-dav1d-for-AV1-decode assertion (dav1d, not libaom, as the AV1 decoder plugin) · §6.1.3 §3.1 · G38
  needs: P4.33, P4.50
  > the §6.1.3 [DECIDED] runtime-plugin-enumeration assertion that the staged libheif resolves `dav1d` as its AV1 *decoder* plugin (e.g. `heif-info`/decoder enumeration lists dav1d, not libaom, for AV1) and fails the build if libaom is wired as the decoder or no dav1d decoder is present (the shipped wiring is verified, not trusted).

### P4.11 — SBOM + NOTICE / third-party-licenses scaffold (§3.7 / §6.3)

- [ ] **P4.55** [BUILD] Build the `engines.lock` schema + the `cargo xtask sbom` two-layer merge scaffold (CycloneDX 1.5, purl+SHA-256 rows) · §3.7.2 §6.3.1 §3.7.1 · G35 G35a
  needs: P0.7.1
  > the §3.7.2/§6.3.1 scaffold (tooling/schema only — per-engine rows populated P5–P7, finalized P10): the `engines.lock` schema (each row a mandatory `purl` (`pkg:generic/<name>@<version>` min + a CPE where one exists) + a per-artifact SHA-256; every staged `.so`/`.dll`/`.dylib` its own row, T3a); the `cargo xtask sbom` merge of the app dep-graph layer (`cargo cyclonedx` + `@cyclonedx/cdxgen`) with the bundled-engine layer, pinned `--spec-version 1.5` on every input, abort-on-mismatch; the DERIVED static-link closure (G35a) + the SBOM-diff (G35b) hooks.
- [ ] **P4.56** [BUILD] Build the THIRD-PARTY-LICENSES.txt / NOTICE generation scaffold (full licence text + corresponding-source pointer per component) · §3.7.1 §3.7.2 §6.3.2 · G36 G36b
  needs: P4.55
  > the §3.7.1/§6.3.2 scaffold: concatenate `THIRD-PARTY-LICENSES.txt` from each component's vendored LICENSE/COPYING + a per-component "corresponding source: <url>@<ref>" line (the §3.6.2 written-offer model); the repo `NOTICE` generated from the same `engines.lock`+SBOM so it can't drift; bundled fonts also listed (OFL/Apache); the generated-vs-committed NOTICE parity hook (every GPL/LGPL/AGPL row has its text + a corresponding-source pointer line). Scaffold; rows populated P5–P7.
- [ ] **P4.57** [BUILD] Wire the §3.7.3 manifest-driven completeness gate scaffold (every externalBin + resources engine file has a manifest row) · §3.7.3 §6.3.3 · G36 G35
  needs: P4.55
  > the §3.7.3/§6.3.3 release-blocking completeness gate scaffold: every `externalBin` + every `resources` engine file (incl. each staged shared object) must have an `engines.lock`/SBOM row with licence text + source pointer or the build fails; the Syft staged-bundle cross-check (an unexpected/side-loaded `.so`/`.dll`/`.dylib` hard-fails, T3a); the SPDX-expression VALIDATION leg + LicenseRef-AOMPL-1.0 carve-out hooks. Activates per-engine as rows land in P5–P7 (the gate is the framework here).

### P4.12 — Reliability harness (§6.4.3 / §6.4.3a / §6.5)

- [ ] **P4.58** [TEST] Build the §6.4.3 per-pair integration runner (real engines, per-format structural reader, fidelity + lossy + patent-gap assertions) · §6.4.3 §6.5 · G31 G32
  needs: P4.7, P0.5.6
  > the §6.4.3 cross-cutting per-pair runner format phases plug pairs into: for each `(source→target)` against the §6.4.5 corpus — completes with exit-success + a **per-format STRUCTURAL READER** decodes the output (NOT magic re-detect — ffprobe codec, `vipsheader` dims, poppler text, `unzip`+`[Content_Types].xml`, RFC-4180 CSV + injection-literal-preservation); content-fidelity spot-checks; lossy disclosure fires iff §04-flagged; patent-gapped pairs asserted absent/disabled not failing. The harness honors §0.9 LibreOffice-serialised. Generic runner; per-pair fixtures land in P5–P7.
- [ ] **P4.59** [TEST] Build the §6.4.3a corpus↔pair bijection guard (`scripts/check-corpus-coverage.rs`, both directions) · §6.4.3a · G22 G23
  needs: P4.58
  > the §6.4.3a [DECIDED] Lane-A bijection guard (a `cargo run` Rust bin reusing the §0.6/§04 + `engines.lock`/manifest parsers): enumerate every v1-required `(source→target)` from the §04 matrices (excl. diagonals/`out`/all-platform-`unavailable`), union the `covers` lists from the single root `tests/corpus/manifest.toml`, fail if any required pair has zero backing corpus files AND fail if any `covers` names a non-existent pair (both directions — the gate can't rot); the `[file.expect]`↔`covers` step-check lint. Makes the §6.5 gate non-circular **at the corpus↔pair layer** — but it enumerates FROM §04, so a format silently dropped from §04 is invisible to it; the SSOT§5⊇§04 anchor that closes that hole is the .1 sub-box below.
  - [ ] **P4.59.1** [GATE] Build the SSOT §5 ⊇ §04/`UserFacingFormat` coverage anchor — every SSOT-named format ∈ the §04 matrices ∧ the `UserFacingFormat` enum, else FAIL · §0.6 · G22 G24
    > the missing PEER of P4.59 that makes the whole completeness chain non-circular **at its top** (without it the bijection guard P4.59 enumerates FROM §04 — a row silently dropped from §04 in the fill pass, e.g. a format cut to dodge a hard engine, is invisible to it, the ledger reads fully green, and the RC ships missing a promised conversion with no gate noticing — the exact "favicon entirely absent" failure mode). A Lane-A `[GATE]` (a `cargo run` Rust bin / a plan-lint-style structural check) that **parses the SSOT §5 *What It Converts* enumerated per-category format list** (Images/Audio/Video/Documents/Spreadsheets/Presentations + the two closed cross-category outputs) and **FAILS if any SSOT-§5-named format is absent from BOTH the §04 category matrices AND the `UserFacingFormat` enum** (the enum lives in §00-architecture, whose comment already states "the enumeration is the SSOT *What It Converts* set"; §04 owns each format's detection/targets/engine/options). SSOT line 337 ("No minimal viable tiering: v1 ships the full coverage in *What It Converts*") makes this load-bearing — the per-platform-patent gap + last-resort-demotion are the ONLY sanctioned omissions and surface as `unavailable`/`demoted` ledger cells, never as an absent §04 row. Ships its **G24 positive+negative self-test** registered in `scripts/gate-selftests/` (deleting an SSOT-§5 format from §04 AND the enum MUST fail; the clean tree MUST pass), so plan-lint check 16 is satisfiable. The upstream half of the SSOT→§04→corpus→ledger chain whose downstream halves are P4.59 (corpus↔pair) + P4.60/P11.15 (pair reliable). (Co-resident with P4.59 as its `.1` so the chain's two halves sit together with no top-level renumber.)
- [ ] **P4.60** [TEST] Build the §6.5.2 pair-status ledger generator (`reliability-report.json` + human table, the release-gate cell set) · §6.5 §6.5.2 · G31 G32
  needs: P4.58
  > the §6.5.2 [DECIDED] generated pair-status report keyed `(source,target,platform)`, each cell ∈ `{reliable, failing, unavailable-per-§3.4, demoted}`; a pair is `reliable` per §6.5.1 (valid structural output + no-harm + fail-clearly + lossy-matches-§04 + content-fidelity on each available platform); the release gate (every enumerated pair `reliable` where not `unavailable-per-§3.4`/`demoted`, any `failing` blocks release); published as a release asset. Generator built here; pairs marked reliable category-by-category in P5–P7.

### P4.13 — Binary-size-budget levers (§3.9)

- [ ] **P4.61** [BUILD] Build the early per-component size-baseline measurement (compressed, per platform) · §3.9 §3.9.1 §3.9.2 · G41
  needs: P4.27
  > the §3.9 [DECIDED] early baseline measurement so P5–P7 track incremental size cost against the ≤400 MB compressed ceiling rather than discovering overflow at release: measure the staged per-component compressed contribution per platform (LibreOffice, fonts, FFmpeg+libs, image stack, poppler, pandoc, the app); the macOS universal near-doubling note. The §6.7.2 release-time size GATE (G41) itself is P10; the levers + baseline are owned here.
- [ ] **P4.62** [BUILD] Build the size-budget trim levers (LibreOffice strip help/l10n/dictionaries, CJK font subset, shared-lib dedup) + the fixed lever-order · §3.9.1 §3.9.3 · G41
  needs: P4.61
  > the §3.9.1/§3.9.3 trim levers + the [DECIDED] fixed lever-order if the ceiling trips: (1) trim CJK font weights first (the §3.9.3 baseline = Liberation+Carlito+Caladea+a curated Noto CJK/RTL subset; only the CJK breadth is `[DEFER: size]`); (2) other font/help trims; (3) dropping pandoc stays BLOCKED (it owns the DOCX/ODT/RTF→MD/HTML pairs LO Markdown export is unvalidated for) — a post-v1 contingency, not a lever; GPL-optional-delegate exclusion / shared-lib dedup. Ties the deferred digit to a decided remedy.

### P4.14 — Generic UX-correctness primitives (§05 / §2.8 / §2.9)

> **P3↔P4 UI-seam model (DECIDED — same statement as P3's UI header):** P3 built
> intentionally-minimal, slice-only renderers (DropZone P3.54, FormatPicker+DestinationBar
> P3.56, ProgressList+Cancel P3.58, ResultSummary+OpenActions P3.59, fault screens P3.60);
> these P4 boxes **SUPERSEDE** (rebuild) them into the generic, `OptionDecl`-declaration-
> driven, fully-a11y components P5–P7 register against — P4 does **not** extend the P3
> renderers in place (the P3 versions are throwaway slice scaffolding). Each P4 UI box
> names the P3 box it supersedes + carries the `needs: P3.5x` edge, so the loop builds the
> P3 slice-renderer first (the live UI until P4 lands) and the supersede is explicit, never
> a silent double-build.

- [ ] **P4.63** [UI] Build the OptionsPanel widget-dispatch that renders declared `OptionDecl` widgets generically (Basic tier) · §1.6 §5.3 · G47 G33a
  needs: P2.8, P1.27
  > the §1.6/§5.3 generic options-panel widget-rendering dispatch (built once here — P5–P7 register only per-format option DECLARATIONS, no new chrome): render each backend-supplied `OptionDecl` by its `OptionKind` (`IntRange`/`Enum`/`Toggle`/`Size`/`Color`) into the declared widget; Basic-tier options shown directly; never gates Convert (the no-decision defaulting rule, §1.6); descriptors come from the backend, UI just renders the declared type. RENDERS the P2-authored `OptionDecl`/`OptionKind`/`OptionKey`/`OptionValue`/`EnumChoice`/`Unit` §0.6 wire types — it is NOT their home (they are authored + registered in `collect_types!` by P2.8). The collapsed-by-default AdvancedDrawer shell is the separate box P4.73. (`needs: P2.8` for the authored `bindings.ts` `OptionDecl` family + `P1.27` for the component/IPC scaffold — the cross-phase edges wired here per the P4.76 reconciliation obligation.)
- [ ] **P4.64** [UI] Build the lossy/fidelity-note surfacing in FormatPicker (passive inline `Note` keyed by `LossyKind`, incl. the video worst-case note) · §2.9 §5.7 · G57 G33a
  needs: P2.20, P1.27, P3.56, P3.69
  > the §2.9/§5.7 lossy-note surfacing mechanism: a passive inline `Note` beside the chosen target the moment a lossy target is selected (the §2.9.1 string by `LossyKind`, verbatim — UI never paraphrases the §02-owned string), once, calm, never a blocking "I understand"/per-file nag; multiple kinds co-apply (de-dup to the most-specific 2–3); the `video_reencode` worst-case "may be re-encoded" note first surfaced at target choice (state 4), with `RunStarted.willReencode` only confirming/clearing it (§5.8 ConvertingNote). The `Note` primitive + the **§2.9.1 catalog (P3.69) as the string source** (rendered verbatim). **SUPERSEDES the P3.56 slice FormatPicker** (the slice had no lossy-note slot) — `needs: P3.56` per the P3↔P4 UI-seam model. (`needs: P2.20` for the `LossyKind`/`OutcomeMsg` bindings + `P1.27` for the FormatPicker/IPC scaffold + `P3.69` for the §2.9.1 note catalog the UI renders — the cross-phase edges wired here per the P4.76 reconciliation obligation.)
- [ ] **P4.65** [UI] Build the ProgressList + aggregate-bar progress surface (real determinate per-item `ItemProgress`, staged-coarse fallback, terminal rows) · §1.11 §5.3 §5.1 · G33a
  needs: P4.8, P2.37, P1.27, P3.58, P1.31.2
  > the §1.11/§5.3 ProgressList: per-item rows keyed by `itemId` over the §0.4.2 `ItemProgress` Channel payloads + the aggregate batch bar; real determinate progress (a mandatory determinate ProgressBar; the indeterminate Spinner only for the brief Collecting step); an indeterminate-`fraction` (LibreOffice) row shows a staged determinate-looking bar from `stage`; rows transition to terminal Succeeded/Failed/Cancelled/Skipped; **reads the §5.1 store live-progress map (P1.31.2) with per-row selector granularity** so a 1000-row batch re-renders only the changed rows, not the whole tree per tick (§1.10 virtualisation). **SUPERSEDES the P3.58 slice Converting/ProgressList** — `needs: P3.58` per the P3↔P4 UI-seam model. (`needs: P2.37` for the `ItemProgress` Channel binding + `P1.27` for the component/IPC scaffold + `P1.31.2` for the §5.1 store — the cross-phase edges wired here per the P4.76 reconciliation obligation.)
- [ ] **P4.66** [UI] Build the cancel surface + the optimistic→confirmed round-trip + the 7a Cancelling sub-state · §1.11 §5.8 §5.3 §5.2 · G33a
  needs: P4.65, P4.10, P4.77
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P4.77` (and P4.66.1's same edge) points at the full §5.2 12-state reducer 11 boxes later in document order — the 7a/11 sub-state edges attach ONTO that complete reducer, so DECISION C builds P4.77 first; the P4.16a header documents the design, this note makes the inversion visible at the `needs:` line. Acyclic + valid.
  > the §1.11/§5.8 cancel surface: a batch-level Cancel control wired to the §1.7 group-kill (mechanism P4.10); the optimistic-vs-confirmed round-trip; the `Converting (Cancelling…)` 7a sub-state registered as an edge ONTO the full §5.2 reducer (P4.77); the in-progress item's `role="progressbar"` retains its last `aria-valuenow` during 7a (no regress to busy); focus-on-entry to the Cancel button in Converting (§5.6). **SUPERSEDES the P3.58 slice Cancel button** (the P3↔P4 UI-seam model) — the `needs: P3.58` edge is carried transitively via P4.65. (`needs: P4.77` for the full §5.2 reducer the 7a edge attaches to.)
  - [ ] **P4.66.1** [UI] Build the QuitConfirm component (state 11 / `app://close-requested`) — focus-trapped `role="alertdialog"` over Converting + the Converting→11→Stay/Quit reducer edges · §5.2 §5.3 §7.3.2 §5.6.1 · G33a G57
    needs: P2.39, P2.120, P4.77
    > the §5.3 `QuitConfirm.tsx` (state 11) the §5.2 machine reaches on the `app://close-requested` event (P2.80 emits it; P2.83 owns the quit-while-converting backend contract; P2.120 wires the listener): a focus-trapped `role="alertdialog"` interstitial **over** the inert-but-mounted `Converting` (7) state with `aria-labelledby`→its heading **"Conversion in progress"** (§5.6 WCAG 4.1.2), the §7.1.1-string body, **Quit** / **Stay** (Stay the safe pre-focused default), **Enter = Stay**, **Esc = cancel-the-close** (→ back to Converting), and focus-restore to the **active element in the underlying Converting state** (Cancel button / progress row — QuitConfirm has NO UI trigger, §5.6); the state-11 edges registered ONTO the full §5.2 reducer (P4.77): Converting(7)→11→{Stay→7/7a, Quit→backend cancel+cleanup+exit} incl. the 7a sub-case (`app://close-requested` during Cancelling raises 11, Stay returns to 7a). All literals via `strings/ui.ts` (English-only, G57); the §5.6.1(2) assertive-announce-on-entry + the §5.6.1(3) Stay-landing are VALIDATED in P9.15. (`needs: P2.39` for the `app://close-requested` event binding + `P2.120` for the listener wiring + `P4.77` for the full §5.2 reducer the state-11 edges attach to — the cross-phase edges wired here per the P4.76 reconciliation obligation.)
- [ ] **P4.67** [UI] Build the result-actions / open-folder flow (OpenActions → C9 OpenKind, split-divert two-button, Summary-only) · §7.7 §5.3 · G33a
  needs: P2.7, P1.27, P3.59
  > the §7.7/§5.3 result-actions: OpenActions backed by C9 `OpenKind` (the only OS shell-out, the `opener` plugin) — "Open folder"→`RevealInFolder{commonRoot}`, "Open file"→`File{filePath}`; split-divert → TWO labelled buttons ("Open source folder"/"Open saved-to folder") + the connector line, single button when no divert; the real `strings/ui.ts` labels (`open_folder`/`open_source_folder`/`open_saved_to_folder`/`open_file`/`saved_to_connector`); Summary-only (state 8), NOT mid-run (the §7.7.3 RunResult-membership set isn't final until terminal). **SUPERSEDES the P3.59 slice Summary OpenActions** — `needs: P3.59` per the P3↔P4 UI-seam model. (`needs: P2.7` for the C9 `OpenKind` binding + `P1.27` for the component/IPC scaffold — the cross-phase edges wired here per the P4.76 reconciliation obligation.)
- [ ] **P4.68** [UI] Build the error / edge-state copy framework (ResultSummary + CommandError + AppFaultNotice rendering §2.8 strings verbatim, residue path) · §2.8 §5.7 §5.3 · G57 G33a
  needs: P4.49, P2.12, P2.19, P3.59, P3.60, P3.68
  > the §2.8/§5.7 error/edge-state copy: ResultSummary renders `RunResult` (success/fail counts, per-item §2.8 reason verbatim, output→source map, fully-failed banner, residue-item-as-Failed with the residue path + a reveal-residue C9 link); the pre-run inline CommandError slot (a passive `Note` above FormatPicker in state 4/5 for a C3/C4/C5 reject, `aria-live="assertive"`, retry action, focus NOT moved); the batch-level summary strings (§2.8.2 — all/partial/all-failed/cancelled/with-residue); **strings owned by §02 (the §2.8.2 catalog authored in P3.68), never paraphrased — the UI RENDERS the resolved `OutcomeMsg.text`, it does not re-author the catalog**. **SUPERSEDES the P3.59 slice ResultSummary + the P3.60 slice fault screens** — `needs: P3.59, P3.60` per the P3↔P4 UI-seam model. (`needs: P2.12` for the `RunResult` binding + `P2.19` for the `IpcError` binding + `P3.68` for the §2.8.2 string catalog the framework renders — the cross-phase edges wired here per the P4.76 reconciliation obligation.)
- [ ] **P4.69** [UI] Build the structural-a11y wiring on the harness components (ARIA roles + keyboard operability + focus management) · §5.6 §5.10 · G33a G57
  needs: P4.63, P4.64, P4.65, P4.67, P4.68, P3.54
  > the §5.6 structural a11y on the P4-built surfaces, decomposed into per-component sub-boxes (each an independent ARIA/keyboard/focus contract — _format.md §3.2: a multi-deliverable box lists its ordered steps as sub-boxes; the dual review fires once over the combined sub-box diff). All wired via the P1 `a11y/` module (keymap, §5.10); the §6.4.6a jsdom/`vitest-axe` ARIA+focus leg (G33a) binds on the combined tree in P4.70. **The DropZone the .1 sub-box wires a11y onto SUPERSEDES the P3.54 slice DropZone** (`needs: P3.54`, the P3↔P4 UI-seam model; FormatPicker/DestinationBar/OptionsPanel/ProgressList/OpenActions are the P4.63–P4.68 rebuilds this box already needs). The distinct ARIA-live announcer mechanism (the `announcer.ts` body + §5.6.1(2) announce-on-state-entry) is the separate box P4.74 — disjoint surface (the shared live-region mechanism vs per-element roles/keyboard/focus), independently buildable + separately dual-reviewed.
  - [ ] **P4.69.1** [UI] Build the DropZone structural a11y (role=button + drag-drop + keyboard activation + focus-on-entry) · §5.6 §5.10 · G33a
    > DropZone `role="button"` (or equivalent) with an accessible name, keyboard activation (Enter/Space opens the native picker via the C2a/C2b path), drag-drop operable from the keyboard fallback, and focus-on-entry to the drop affordance in Idle/Collecting; the keymap binding via the P1 `a11y/` module. SUPERSEDES the P3.54 slice DropZone (the `needs: P3.54` edge sits on the parent P4.69).
  - [ ] **P4.69.2** [UI] Build the FormatPicker + OptionsPanel structural a11y (radiogroup/roving-tabindex/aria-checked + aria-disabled patent-gap tiles + labelled option widgets) · §5.6 §1.6 · G33a
    > FormatPicker `role="radiogroup"` of `role="radio"` tiles with `aria-checked`, **roving-tabindex** (one tab stop, arrow-key navigation), `aria-disabled` on the §3.4 patent-gapped/unavailable tiles (the disable-with-reason surface), focus-moved to the default tile on entering Targets; the OptionsPanel widgets (rendered by P4.63) each labelled + keyboard-operable. The FormatPicker/OptionsPanel are the P4.63/P4.64 rebuilds.
  - [ ] **P4.69.3** [UI] Build the ProgressList + DestinationBar + OpenActions structural a11y (role=progressbar + aria-valuemin/max/now + Convert/Change keyboard + split-divert two-button + Summary focus order) · §5.6 §1.11 · G33a
    > ProgressList `role="progressbar"` with `aria-valuemin`/`aria-valuemax`/`aria-valuenow` per item + the aggregate bar; DestinationBar Convert/Change controls labelled + keyboard-operable with focus moved to Convert when the DestinationBar appears; OpenActions split-divert TWO-button keyboard operability; the §5.6 Summary priority focus order on entering state 8. The ProgressList/DestinationBar/OpenActions are the P4.65/P4.67/P4.68 rebuilds.
- [ ] **P4.70** [TEST] Wire the §6.4.6a `vitest-axe` jsdom a11y assertions over the P4 harness component tree (ARIA/role validity + focus-order) · §6.4.6a §5.6 · G33a
  needs: P4.69
  > the §6.4.6a [DECIDED] automated-a11y Lane-A leg: `axe-core` via `vitest-axe@0.1.0` over the rendered React tree under Vitest/jsdom — ARIA role/state validity (no invalid/orphaned roles; the radiogroup tiles carry valid `aria-checked`) + focus-order/tabindex sanity + labelled controls; any violation at the configured impact fails the build. (Contrast is the Lane-B `@axe-core/webdriverio` leg, G33b, P9 — jsdom can't compute contrast; text-size is the §6.6 human walkthrough, P11.)

### P4.15 — Resource pre-flight & budgets engine (§1.10)

> The §1.10 `[DECIDED design]` estimation+decision mechanism — the cross-cutting home
> is P4 (every engine phase depends on it; P4 owns the §0.9 pool + §2.14 staging it
> composes with). P2 DECLARED the `PreflightVerdict` DTO + C4 RETURNS it; P3 FEEDS it
> the walking-skeleton verdict; P5/P6 FEED inputs (SVG clamp, to-GIF estimate). These
> two boxes BUILD the engine those declarations/feeds resolve against; P9.41 then
> VALIDATES + calibrates the numbers against the corpus.

- [ ] **P4.71** [RUST] Build the §1.10 resource pre-flight estimation + per-physical-volume decision engine (`SizeEstimate`, footprint grouping, 1.3× headroom, whole-batch `up_front_fail`, `TooBig` ceilings) · §1.10 §2.14.4 §0.6 · G31
  needs: P4.20, P2.11, P3.37
  > the §1.10 estimation+decision core: the `SizeEstimate { est_output_bytes, est_scratch_bytes, basis }` struct; the per-category cheap heuristics (raster from `DetectionOutcome.dims`, video/GIF from the source-byte worst-case bound, never an up-front `ffprobe`); per-**physical-volume** footprint grouping (`est_output`+publish-temp → each item's `final_dir` volume; `est_scratch` kind-2 → the system/scratch volume); the **macOS staged-input PEAK-CONCURRENT term** (≈ `degree × largest in-flight`, NOT the whole-batch Σ; Windows/Linux term = 0); the **1.3× headroom-per-volume** rule; and the whole-batch `PreflightVerdict.up_front_fail = Some(OutOfDisk)` when any one physical volume's grouped footprint can't fit, plus the absolute per-item/aggregate `TooBig` ceilings. Composes with the §0.9 pool (the concurrency `degree` it bounds the peak-concurrent term by) + the §2.14 scratch layout. (`needs: P2.11` for the declared `PreflightVerdict`/`SizeEstimate` §0.6 DTOs + `P3.37` for the §2.14 scratch/`final_dir`-volume resolution — the cross-phase edges wired here per the P4.76 reconciliation obligation.)
- [ ] **P4.72** [RUST] Build the §1.10 mid-run write-time per-item `Failed(TooBig|OutOfDisk)` enforcement in the §2.1 write sequence · §1.10 §2.1 §2.8 · G31
  needs: P4.71, P3.38
  > the §1.10 mid-run half (the up-front-vs-mid-run split's per-item leg): at WRITE time, when an item's own size/space breaches the budget (or real disk usage outruns the estimate), the §2.1 write fails, §2.6 restores free space, and the item is reported `Failed(TooBig|OutOfDisk)` (§2.8) **while the batch continues** (§1.9/§1.11 fast-fail surfacing) — there is NO per-item up-front-fail list on `PreflightVerdict`; a per-item doom shows as that item's mid-run terminal row. Plugs the budget check into the P3-built §2.1.1 per-item write sequence (the resolve-late step) + the §2.6 free-space restore. (`needs: P3.38` for the §2.1.1 per-item write sequence — the cross-phase edge wired here per the P4.76 reconciliation obligation.)

### P4.16 — Deferred-split completions & cross-phase reconciliation

> The split-off siblings of P4.51/P4.63/P4.69 (each a genuinely disjoint surface that
> must carry its own dual review, _format.md §3.2 / build-loop §3 step 2) plus the
> single reconciliation-obligation box that wires the cross-phase P1/P2/P3 `needs:`
> every P4 box deferred. They sit after their parents (document order) and before the
> §P4.17 exit gate; each is its own P4 deliverable (the phase is "done" only when every
> `[ ]` box is `[x]`, README "How this plan is used"), independent of the proof-of-life
> predicate the exit gate asserts.

- [ ] **P4.73** [UI] Build the AdvancedDrawer collapsed-by-default shell over the OptionsPanel (Advanced-tier reveal) · §1.6 §5.3 · G47 G33a
  needs: P4.63
  > the §1.6/§5.3 AdvancedDrawer chrome split from P4.63's widget dispatch: a collapsed-by-default toggle that reveals the Advanced-tier `OptionDecl` widgets the P4.63 dispatch renders (Basic shown directly; Advanced behind this drawer); never gates Convert (the no-decision defaulting rule, §1.6). P5–P7 Advanced-tier option DECLARATIONS register against this drawer. Independently buildable + separately dual-reviewed from the Basic-tier widget dispatch (P4.63).
- [ ] **P4.74** [UI] Build the `announcer.ts` body + the §5.6.1(2) live-region announce-on-state-entry wiring · §5.6.1 §5.6 · G33a G57
  needs: P4.69
  > the ARIA-live announcer mechanism split from P4.69's structural a11y (P1.39 stood up `announcer.ts` interface-only): fill the live-region announcer and fire it per state transition — **ASSERTIVE** on entry for Confirm 3 / RerunPrompt 6 / Summary 8+first-Failed / MixedDropRefusal 9 / UnsupportedNotice 10 / QuitConfirm 11 / AppFault 12; **POLITE** for Collecting progress / lossy-divert notes / throttled Converting milestones (no per-tick flood); `aria-busy` cleared + `aria-valuenow` set to 100/last-known on each item's terminal transition (WCAG 4.1.2) — the distinct-from-focus-management live-region mechanism P9.15 validates. Disjoint surface from P4.69 (the shared announcer vs per-element roles/keyboard/focus), independently buildable + separately dual-reviewed.
- [ ] **P4.75** [BUILD] Build the image-worker §6 relink-bundle + x265 GPL §3 corresponding-source assertion (static-LGPL-aggregation leg) · §6.1.3 §3.6.2 · G36 G38b
  needs: P4.51
  > the §6.1.3 [DECIDED] linkage-site (ii) leg split from P4.51 (a different code object — the separate `convertia-imgworker`, not the MIT core — with a different obligation that fails independently): static LGPL inside the image-worker is acceptable aggregation BUT carries the §6 relink obligation — assert the relinkable-source bundle (object files / recipe) is present **incl. x265's GPL §3 corresponding source** (worker-with-x265-loaded is a GPL combined work), fail the build if either source bundle is missing. (Per-engine corresponding-source bundles land in P5–P7; the generic carve-out logic for the image-worker code object is built here.)
- [ ] **P4.76** [GATE] Wire the cross-phase reconciliation obligation — every deferred P4→P1/P2/P3 `needs:` edge declared, no half-wired plan declares done · §0.7 · G7 G20
  > No `needs:` — this box installs the G20 plan-lint rule that AUDITS the listed boxes' edges; it runs structurally (the rule fires on the plan text), it does not build code against those boxes, so it requires none of them `[x]` first.
  > the single reconciliation-obligation box closing the DECISION-C cross-phase gap: declare the deferred forward `needs:` edges P4 boxes carry in their build-order notes — P4.1→P3 §1.7-dispatch-shell; P4.13→P3.2 isolation-shell; P4.18.1→P0.5.9 (isolation/privilege-drop activation); P4.20→P3.3 pool-shell; P4.23→P3.43/P3.44/P3.45 (re-homes the P3-built InProcessNative lane onto the real pool — NOT a rebuild); P4.24→P3 §2.14 kind-2 scratch; P4.34.1→P0.4.3 (imgworker-FFI fuzz activation); P4.39 reads the P2 C12 contract; P4.63→P2.8+P1.27; P4.64→P2.20+P1.27; P4.65→P2.37+P1.27+P1.31.2; P4.66.1→P2.39+P2.120+P4.77; P4.67→P2.7+P1.27; P4.68→P2.12+P2.19; P4.71→P2.11+P3.37; P4.72→P3.38; P4.77→P3.53+P1.31.2 (full §5.2 reducer) — and **register a plan-lint check (G20, check-set) that FAILS any box whose `>`-note literally contains `the fill pass adds those needs` / `the reconciliation pass wires those`**, so a half-wired plan cannot be declared done. **Plus a structural leg (the silent-omission guard): plan-lint asserts every format/exercise phase file that ships per-pair tests against the P4 §6.4.3 runner (any box carrying `needs: P4.58` — P5/P6/P7) contains a `[GATE]` reconciliation box whose title contains `reconciliation` and whose `needs:` includes P4.58 — turning a phase that simply OMITS its reconciliation box (the case the forbidden-string leg cannot catch, since it only fires on a box that USES the banned phrase) into a detectable lint failure rather than a review-time catch.** (A phase like P8 that ships NO box with `needs: P4.58` is exempt and carries its cross-phase edges inline per a boundary note, P8 Boundaries.) The reciprocal P3/P5/P6/P7/P9 reconciliation boxes (P3.70/P5.72/P6.92/P7.77/P9.46) carry the same obligation for their phases. (Authored as the obligation + the plan-lint leg; the actual `needs:` edges are added on the P4 boxes in the same pass — this box is the tracked owner so the obligation is auditable, not a silent deferral.)

### P4.16a — Full §5.2 frontend state machine (completes the P3 slice subset)

> P3.53 built only the §5.2 *slice subset* (states 1→2→3→4/5→[6]→7→8 + 9/10 +
> `app://fault`→12). §5.2 is the named owner of the full 12-state FSM + transition
> diagram; no later box completed it (the P1.31 "P2/P8" note was wrong). This box
> extends the slice reducer to **all 12 states** so the cancel/quit boxes (P4.66/
> P4.66.1) and the P4.77 UX-harness exit leg drive a complete machine, not a partial
> one. It is the foundation the 7a/11 edge boxes register their specific edges onto.

- [ ] **P4.77** [UI] Complete the full §5.2 12-state reducer FSM (all states + the back-edges P3.53 omitted) over the §5.1 store · §5.2 §5.8 §5.1 · G33a G57
  needs: P3.53, P1.31.2
  > extend the P3.53 slice reducer to the **complete §5.2 transition table** — all 12 states + the edges the slice omitted: the **Targets(4)→Confirm(3) Back** edge (preserve the frozen set, never re-collect), the **Rerun(6)→Destination(5) cancel-Esc** edge, the **MixedDropRefusal(9)→Collecting** re-drop edge, and the **launch-into-Collecting initial state** (a launch-arg/Open-with intake enters Collecting directly, not Idle); states 7a (Cancelling) and 11 (QuitConfirm) are the edge boxes P4.66/P4.66.1 add ONTO this reducer (they `needs:` this box). Driven by inbound IPC results/events (§5.8); the backend is the source of truth; the reducer state lives in the §5.1 store (P1.31.2). All user-facing literals via `strings/ui.ts` (English-only, G57). SUPERSEDES the P3.53 slice subset in place by extension (the reducer is the one FSM, not a throwaway like the P3 *renderers* P4.63–P4.69 replace). (`needs: P3.53` for the slice reducer it extends + `P1.31.2` for the §5.1 store it lives in — the cross-phase edges wired here per the P4.76 reconciliation obligation.)

### P4.17 — Proof-of-life exit gate

- [ ] **P4.78** [TEST] Drive a representative in-P4 image round-trip end-to-end through the P4 UX harness (in-P4 fixture + throwaway pair → options-panel → progress/cancel → result-actions → first ledger cell) · §6.5 §6.4.3 §5.7 · G31 G33a
  needs: P4.37, P4.58, P4.60, P4.63, P4.65, P4.67
  > the P4 UX-harness exit leg (so P4 is not "done" on the engine side alone), made **self-contained in P4** — it stages its own input, never reaches forward into P5: stage a **minimal in-P4 image fixture** (a tiny synthetic raster, e.g. a few-pixel PNG, committed under `tests/corpus/` with its SHA-256 in the manifest, G24a) and register a **throwaway/representative pair** (the imgworker decode→encode round-trip P4.37 already drives — e.g. PNG→PNG over the synthetic fixture) so the §6.4.3 per-pair runner (P4.58) has a real backing file + a real `(source,target,platform)` key with NO P5 corpus/saver dependency; then drive that round-trip end-to-end through the P4-built options-panel shell + lossy-note + progress/cancel + result-actions UI to a §6.4.3 structural-reader pass (`vipsheader` decode + nonzero dims over the P4 fixture) and a **first §6.5.2 ledger cell over the P4 fixture** — proving P5–P7 register declarations against an already-built UI + runner + ledger and a pair can reach its §6.5 `reliable` gate without waiting on P8. (The real P5 image corpus + savers + per-pair pairs land in P5; this exit leg proves the harness on a P4-owned fixture, so P4 has no P4→P5 phase-order inversion.)
- [ ] **P4.79** [TEST] Verify the P4 proof-of-life exit criterion (imgworker boots + isolated round-trip + populated EngineHealth + first reliability report) · §3.5.5 §2.12 §7.2.3 §6.4.3 · G46 G31
  needs: P4.37, P4.44, P4.59, P4.60, P4.78
  > the consolidated P4 exit gate (README P4 proof-of-life): `convertia-imgworker` boots, a round-trip invocation succeeds through the §2.12 isolation boundary (P4.37), the §7.2.3 startup verifier reports a populated `EngineHealth` (P4.44), the §6.4.3 runner + §6.5.2 pair-status ledger + §6.4.3a bijection guard produce their first report (P4.58–P4.60), AND the UX-harness leg (P4.78) passes — the full P4 "done" predicate.
