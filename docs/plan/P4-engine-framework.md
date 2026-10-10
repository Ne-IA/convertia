# P4 — Engine & Bundling Framework

> **The reusable harness every format engine plugs into.** P4 builds the generic
> engine-invocation layer, per-OS sidecar packaging/bundling, the §2.12 decoder-
> isolation boundary, the §0.9 subprocess pool, the cross-cutting reliability test
> machinery (per-pair runner + pair-status ledger + corpus↔pair bijection guard), the
> SBOM/NOTICE scaffold, the §7.2.3 startup engine-presence/integrity verifier, the §3.4
> patent-disposition matrix + availability wiring, and the **generic UX-correctness
> primitives** (options-panel shell, lossy notes, progress/
> cancel, result-actions, error copy, structural a11y) — so each later engine phase
> (P5–P7) registers per-format declarations against an already-built UI and reaches its
> §6.5 `reliable` gate without waiting on P8.
>
> Spec home: 01-conversion-pipeline (§1.7 the generic engine-invocation lifecycle —
> P4.6–P4.12; §1.10 the resource pre-flight & budgets engine — P4.72/P4.73),
> 03-engines-and-bundling (engine-invocation layer, per-OS bundling, image-
> worker `convertia-imgworker`, §3.4 patent matrix + §3.4.4a availability wiring, §3.5.0
> macOS TCC staging, §3.8 engine-source anchors), 02-guarantees (§2.1 publish contract,
> §2.12 isolation, §0.9 pool, §2.12.3 privilege-drop, §0.11 threat-map, §2.13 app-fault,
> §2.8/§2.9 UX primitives), 06-build-test-release (§6.4.3/§6.4.3a/§6.5 reliability
> machinery, SBOM scaffold, §6.1.3 build assertions, §6.3.4 consumer verify), 07-app-shell
> (§7.2.3 integrity manifest + startup verifier, §7.2.6 macOS TCC), 05-ui-ux (generic UX
> primitives). Index: [README.md](README.md). Box format: [`_format.md`](_format.md).
>
> **Exit criterion (proof-of-life):** `convertia-imgworker` boots, a round-trip
> invocation succeeds through the §2.12 isolation boundary, the startup verifier reports
> a populated `EngineHealth`, the §6.4.3 runner + pair-status ledger + §6.4.3a bijection
> guard produce their first report, **and** a representative **in-P4 image round-trip**
> (a P4-staged minimal fixture + throwaway pair, NOT a P5 corpus pair — so P4 is
> self-contained, no P4→P5 inversion) is driven through the §1.7 lifecycle and isolation
> to a first ledger cell over the P4 fixture (engine half), and the P4-built options-panel
> shell + progress/cancel + result-actions UI renders that run under jsdom (UI half) — P4
> is not "done" on the engine side alone.
>
> **Scope:** the §3.4 patent-disposition matrix is decided here once, never re-decided
> downstream; P5/P6 only read its per-codec cell. The §7.2.3 startup verifier (DoD gate 19,
> the runtime half of the T3 supply-chain threat) turns a missing or corrupt engine into a
> §2.13 app fault, never a crash. The §3.9 size levers are built where each first fires
> (P7.1, P7.2); the §6.7.2 release-time size gate is P10's. The §3.5.0/§7.2.6 macOS TCC
> source staging lives here because the first real engine spawn needs it.
>
> Each phase's boxes are audited against the as-built codebase at the preceding phase's
> sweep (test-strategy §11). P4 does **not** re-implement `crate::fs_guard` (built in
> P3); it fills the `crate::isolation` + pool shells P3 established. Per-engine SSRF/LFR
> hardening (FFmpeg/pandoc/LibreOffice/librsvg) lives in P5/P6/P7, not here; per-engine
> SBOM rows / §6.1.3 assertion lists / §7.2.3 availability rows are populated by P5–P7
> against the generic frameworks built here.
>
> Condensed at 7f091bc: `git show 7f091bc:docs/plan/P4-engine-framework.md` holds every pre-diet note.

---

## Engine-registry seam & the `Engine` trait

- [x] **P4.1** [RUST] Define the `Engine` trait — id/descriptor/capabilities/plan/plan_encode/classify_failure · §3.2.2 · G29
  needs: P3.4, P3.75, P4.2, P4.3
  > Delivered: 1fb63e2
- [x] **P4.2** [RUST] Define `ProbeOutput` + reconcile the §3.2.2 plan-seam types against their P3.4 authoring · §3.2.2 · G29
  needs: P3.4
  > Delivered: c94b5f4
- [x] **P4.3** [RUST] Define the engine-layer leaf types (`Direction`/`EngineCapability` + aliases) + reconcile `ProgressModel` against its P3.4 authoring · §3.2.2 · G29
  needs: P3.4
  > Delivered: ffc07c5
- [x] **P4.4** [RUST] Build the engine registry + `select()` static-lookup algorithm · §3.2.3 §0.6 · G29
  needs: P4.1, P4.3
  > Delivered: 2e25de1
- [x] **P4.5** [RUST] Wire the `EngineId → serialised_only` data path for the pool · §3.2.2 §0.9 · G29
  needs: P4.4
  > Delivered: ce666a4

## Generic invocation lifecycle (§1.7)

- [x] **P4.6** [RUST] Reconcile the §1.7 `EngineInvocation` envelope + `InvocationResult` against their P3.4 authoring + wire the P4 consumers · §1.7 · G29
  needs: P4.2, P3.4
  > Delivered: 750e720
- [x] **P4.7** [RUST] Build the generic spawn lifecycle state machine (spawn→Running→exit/timeout/cancel/spawn-error) · §1.7 §2.12 · G29 G31
  needs: P4.6, P4.13
  > Delivered: 94a3088
- [x] **P4.8** [RUST] Build the per-`ProgressModel` stdout/stderr handling dispatch · §1.7 §3.2.2 §1.11 · G29
  needs: P4.7, P4.3
  > Delivered: 17d5a05
- [x] **P4.9** [RUST] Build the two-step probe-then-encode sequencing (call plan→spawn probe→parse ProbeOutput→plan_encode→spawn encode) · §1.7 §3.2.1 · G29
  needs: P4.8
  > Delivered: 0f078ea
- [x] **P4.10** [RUST] Build the cross-platform process-group / job-object spawn + whole-group kill (process-wrap) · §1.7 · G29 G9
  needs: P4.7
  > Delivered: 19065cc
- [x] **P4.11** [RUST] Build the kill↔cleanup↔no-partial ordering + the bounded confirm-wait + deferred-reclaim residue path · §1.7 §2.6 · G29 G31
  needs: P4.10
  > Delivered: d03ae4c
- [x] **P4.12** [RUST] Build the timeout / no-progress watchdog + exit & output verification (non-empty temp) · §1.7 §0.9 · G29 G31
  needs: P4.7, P4.1
  > Delivered: f3bee24

## The §2.12 decoder-isolation wrapper (`crate::isolation`)

- [x] **P4.13** [RUST] Build the `crate::isolation` cheap-tier floor (process boundary + minimal/cleared env + scratch-cwd + input/tmp-only handing) · §2.12.1 §2.12.3 · G29 G9
  needs: P3.2, P4.85
  > Delivered: ca80c99
- [x] **P4.14** [RUST] Strip the dynamic-loader injection vars in the minimal env (LD_PRELOAD/LD_LIBRARY_PATH/DYLD_*) · §3.5 §2.12.3 §0.11 · G29
  needs: P4.13
  > Delivered: ba06393
- [x] **P4.15** [RUST] Build the Linux privilege-drop tier (the three independent kernel-subsystem legs, each silent-degrade) · §2.12.3 · G42 G42b
  needs: P4.13
  > Delivered: 4f9b051
  - [x] **P4.15.1** [RUST] Build the Landlock fs-restrict leg (ABI≥1 probe + `{input ro, tmp rw}`) · §2.12.3 · G42b
    > Delivered: 4f9b051
  - [x] **P4.15.2** [RUST] Build the network-namespace egress-deny leg (`unshare --net`, loopback-only, preflight probe) · §2.12.3 · G42
    > Delivered: 4f9b051
    > Deviation: the namespace is entered in the pre-exec child and a setup failure skips the leg; no parent-side preflight exists (`unshare(CLONE_NEWUSER)` is `EINVAL` in a multithreaded process). Spec §2.12.3 Linux row.
  - [x] **P4.15.3** [RUST] Build the seccomp-bpf exec-deny leg (deny exec/unexpected syscalls, defence-in-depth) · §2.12.3 · G42b
    > Delivered: 4f9b051
    > Deviation: exec is inheritance-limited, not denied (the filter installs pre-exec). Spec §2.12.3 Linux row.
- [x] **P4.16** [RUST] Decide + pin the macOS privilege-drop tier realization — v1-portable = cheap-tier floor (the Seatbelt apply leg is not never-break-safe) · §2.12.3 · G42 G42b
  needs: P4.13
  > Delivered: 54392e0
- [x] **P4.17** [RUST] Build the Windows privilege-drop tier (restricted-token/AppContainer + low-integrity + Job-Object resource caps + AppContainer/WFP net-deny) · §2.12.3 · G42 G42b
  needs: P4.13, P4.10
  > Delivered: 6facd91
  > Deviation (the P4.17 Co-Pilot ruling): the realized legs are the intermediate-integrity token and ConvertIA's own Job Object; restricted token, AppContainer and the network deny are unrealizable in the portable build. Spec §2.12.3 Windows row and its residual note.
- [x] **P4.18** [RUST] Record the §2.12.3 achieved privilege-drop tier per platform into `privilege-drop-coverage.toml` · §2.12.3 · G64
  needs: P4.15, P4.16, P4.17
  > Delivered: 8212ebc
  - [x] **P4.18.1** [TEST] Instantiate the P0.5.9 tier-APPLIED-per-spawn regression + anchor the P0.7.12 leg-(a) enforcement substrate (privilege-drop tier applied on each engine spawn) · §2.12.3 §2.11.4 · G31 G42 G42b G64
    needs: P4.18, P4.10, P4.13, P4.15, P4.16, P4.17, P0.5.9, P0.7.12
    > Delivered: 8212ebc
  - [x] **P4.18.2** [TEST] Instantiate the §2.12.3 memory-cap kill → item-`Failed`, batch-continues regression (P0.5.9 home) · §2.12.3 §2.8 §1.9 · G31 G64
    needs: P4.18, P4.10, P4.17, P0.5.9
    > Delivered: 8212ebc
  - [x] **P4.18.3** [TEST] Instantiate the process-group / Job-Object reap regression (no orphaned descendant survives a kill, P0.5.9 home) · §2.12.3 §1.7 · G31 G64
    needs: P4.18, P4.10, P0.5.9
    > Delivered: 8212ebc
- [x] **P4.19** [RUST] Assert detection's in-core untrusted-byte boundary holds (no third-party C/C++ decoder in-core) · §2.12.4 · G29
  needs: P4.13
  > Delivered: 602c2b4

## Subprocess pool & concurrency degree (§0.9)

- [x] **P4.20** [RUST] Expand the P3 pool shell into the bounded engine-subprocess pool + global concurrency degree · §0.9 · G29
  needs: P4.5, P3.3
  > Delivered: dfff198
- [x] **P4.21** [RUST] Wire the per-engine parallelism caps (LibreOffice serialised-1, video re-encode 1–2, image/poppler/pandoc/CSV up to degree) · §0.9 · G29
  needs: P4.20
  > Delivered: 3b6f55c
  > Deviation: LibreOffice's "serialised-1" is the §0.9 `serialised_only` lane (P4.22), not a cap term; §0.9 records why.
- [x] **P4.22** [RUST] Build the `serialised_only` single-permit-semaphore enforcement + the `MAX_LO_CONCURRENCY` const · §0.9 · G29
  needs: P4.21, P4.5
  > Delivered: dab8593
- [x] **P4.23** [RUST] Attach the P3-built `InProcessNative` lane (P3.43–P3.45) to the now-real bounded pool · §1.7 §3.5.6 · G29
  needs: P4.20, P4.8, P3.43, P3.44, P3.45
  > Delivered: 532bf73

## macOS TCC source staging (§3.5.0 / §7.2.6)

- [x] **P4.24** [RUST] Build the macOS TCC source-staging copy (core copies source into per-job kind-2 scratch before spawn) · §3.5.0 §7.2.6 §0.11 · G29 G31
  needs: P4.13, P3.21
  > Delivered: 3b876f9
- [x] **P4.25** [RUST] Hand engines the staged scratch path, never the raw protected path (per-engine input-arg/handle plumbing) · §3.5.0 §7.2.6 · G29
  needs: P4.24
  > Delivered: a41c2f2

## Per-OS sidecar packaging & bundling (§3.3)

- [x] **P4.27** [BUILD] Build the `scripts/stage-engines` skeleton — placement, externalBin triple-suffixing, resources tree, per-OS layout · §3.3.1 §3.3.2 §6.1.3 · G37
  needs: P0.4.10
  > Delivered: 5d9b403
- [x] **P4.28** [BUILD,CI] Wire the pinned checksum-verified engine-asset cache (`actions/cache` keyed `<cache_engine>-<cache_version>-<triple>` + pinned-URL fallback) · §6.1.3 §3.8 · G37 G56
  needs: P4.27, P4.56.1
  > Delivered: 4a5f359
  - [x] **P4.28.1** [BUILD,CI] Build the generic from-source engine-compilation harness — digest-pinned per-OS/triple build container + the populate-cache-from-CI-compile path + the per-engine configure-flag manifest seam · §6.1.3 §3.8 §3.5.1 · G37 G37b
    needs: P4.27, P0.7.3
    > Delivered: 4a5f359
- [x] **P4.29** [BUILD] Build the macOS universal-sidecar `lipo -create` step + the per-sidecar `lipo -info` fat-Mach-O assertion · §6.1.3 §3.4.5 · G30 G37
  needs: P4.27, P4.28
  > Delivered: b257a42
  > Deviation: the fat check parses the Mach-O fat header in stdlib (the G30 method), not `lipo -info`, at the step that produces the file.
  - [x] **P4.29.1** [BUILD] Build the missing-`x86_64`-slice from-source cross-toolchain / Rosetta-2 fallback (compile the absent slice via the P4.28.1 harness before lipo) · §6.1.3 §3.4.5 · G30 G37
    needs: P4.28.1
    > Delivered: b257a42
- [x] **P4.30** [BUILD] Build the generic `scripts/stage-engines` dynamic-library RELOCATION mechanism (per-OS rpath / install_name rewrite so beside-the-exe libs resolve inside the bundle) · §3.5 §3.9.1 §3.6.1 · G37 G37b
  needs: P4.27
  > Delivered: 4be187d
  - [x] **P4.30.1** [BUILD] Build the macOS `install_name_tool` relocation leg (`-id @loader_path/<name>` per lib + `-change <abs> @loader_path/<lib>` on the dependent binary) · §3.5 §3.6.1 · G37 G37b
    > Delivered: 4be187d
  - [x] **P4.30.2** [BUILD] Build the Linux `patchelf --set-rpath '$ORIGIN'` relocation leg (`DT_RUNPATH` resolves beside-the-exe) · §3.5 §3.6.1 · G37 G37b
    > Delivered: 4be187d
  - [x] **P4.30.3** [DOC] Record the Windows recorded-no-op relocation decision (PE DLLs resolve from the exe's own directory) · §3.5 §3.6.1 · G37
    > Delivered: 4be187d
- [x] **P4.31** [BUILD] Wire `tauri.conf.json` `bundle.externalBin` + `bundle.resources` for the engine set · §3.3.1 §0.10 · G47
  needs: P4.27
  > Delivered: f62d0c7
- [x] **P4.32** [RUST] Build the runtime program-path resolution (`current_exe().parent()` sidecars · `BaseDirectory::Resource` for resource-tree binaries) + the `EngineId→binary-name` table · §3.3.3 · G29
  needs: P4.2
  > Delivered: c6568b7
- [ ] **P4.33** [RUST] Wire the §7.2.4 executable-permission setup — fill the P2.106.4 `ensure_engine_permissions` step-4 slot over the P1.17 `ensure_executable` helper · §7.2.4 §7.2.1 · G29
  needs: P4.32, P1.17, P2.106.4
  > Scope: the §7.2.1 step-4 body over the delivered P1.17 `crate::platform::ensure_executable` (never re-authored), in the `ensure_engine_permissions` slot P2.106.4 put on the live `readiness_checks` spine; its verdict and the absent-sidecar rule are §7.2.1 step 4. The macOS quarantine path is P4.46.1's.
  > (1) The one engine-program table `[(EngineId, EngineProgram)]` in `crate::engines::program`, with the five rows that carry no `[DEFER]` (`Sidecar` for FFmpeg, FFprobe, Poppler, Pandoc and ImageCore), and a handle-free walk over `&ProgramRoots` (fed from `program_roots()`) resolving each row through `resolve_program`.
  > P4.42 reuses the table, and P7.4 adds the LibreOffice row once P7.1 fixes its path.
  > (2) The helper on each resolved path, with the call-site `NotFound` skip of §7.2.1 step 4.
  > (3) The helper has no non-test caller today and goes live through `readiness_checks` ← `run()`, so its two `expect(dead_code)` attributes are removed, not flipped, with the adjacent G70 `[Test-Change: P4.33 …]` tag; the P1/P2-era futurity comments in `platform/mod.rs` and `lib.rs` get `[Corrected by P4.33]` (plan-lint check 29).
  > Tests: a Unix temp exe-dir walk (a `0o644` fake sidecar becomes `+x`, an already-`+x` one is untouched, an absent one is skipped), the Windows no-op leg, and the §1.1a source-scan pin that step 4 calls the P1.17 helper.
  > The non-`NotFound` test arm uses an `exe_dir` that is a regular file (`NotADirectory`): a present-but-unwritable file cannot fail for its owner in a tempdir.

## The engine-source chain — lock validator, anchor enum, consumer verify, SBOM tooling (§3.7 / §3.8 / G37)

> P4.100, the §1.7 `process-wrap` 10 adoption, is not part of the chain: it sits right after P4.94 so the
> spawn-code major lands before the boxes that build on it.

- [ ] **P4.94** [BUILD,RUST] Build the `lock.rs` Loop half of the engine-source allow-list gate — the (b2) distinct-host validator floor + the committed-manifest binding test · §3.8 §3.7.2 · G37
  needs: P4.56.1
  l-neg1: none
  > The uncaged `lock.rs` half of the P4.56.3 allow-list gate. (1) `validate()` rejects, as a `LockViolation`, a `mirrors` row whose `corroboration_urls` are not pairwise on distinct hostnames (§3.8 *Origin model*: the same URL twice, or two URLs on one host), with a planted fixture leg for each; it compares the parsed hostname, never the netloc.
  > (2) The committed-manifest binding test lives in `lock.rs`'s own `#[cfg(test)]` module (`mod lock;` is private): it parses the real `src-tauri/engines.lock` through the one §3.4.4a compile-time embed const (P4.40 or this box, whichever lands first, creates it) and asserts `validate()` is `Ok`, so every owner-acked manifest edit is gated.
  > If the hostname parse promotes `url` (a transitive dependency today) to a direct one, its §0.8 row and floor row land with the dep-add; never hand-roll a netloc split.
- [ ] **P4.100** [RUST] Upgrade `process-wrap` 9.1.0 → 10.x and re-derive the isolation workarounds against its source · §1.7 §2.12.3 · G29 G31
  needs: P4.10, P4.17
  > Bump `process-wrap` in `src-tauri/Cargo.toml` and `Cargo.lock`, refresh `fuzz/Cargo.lock` and raise its floor in the root `Cargo.toml` pinned-floors table in the same commit (a raise is ordinary box work, §0.8 *Relied-upon floors*).
  > Re-read 10.x's source for the two residuals `crate::isolation` records (the limit-less own job, the crash-time reap): retire each workaround 10.x makes redundant, keep each it does not, and re-word the comments to the measured behaviour.
  > Same-commit spec sync: §1.7's Windows kill/reap correction blocks state the measured 10.x behaviour; the FORCED DEVIATION and UPSTREAM notes and the 9.1.x literals retire.
  > P4.18.2 and P4.18.3 stay green on all three OS; the macOS lane green is part of done.
- [ ] **P4.98** [BUILD] Verify every staged engine byte at the consumer — `stage-engines` hash-on-copy + exact-key select, `fetch-engine-assets` hit re-verify · §6.3.4 §6.1.3 · G37
  needs: P4.27, P4.28
  l-neg1: same-push
  > `stage-engines` selects each cache entry by its exact `<cache_engine>-<cache_version>-<triple>` key and hashes each rowed member on copy, before relocation or `lipo` (a universal slice against its per-arch row); a tree row stages by unpacking its verified archive (`fetch-engine-assets`' unpacker); a compiled member without an output anchor is refused.
  > The run ends asserting every covered row was hashed once; a mismatch or missing member exits 1, writes nothing and names the remedy (prebuilt: delete the cache key, refetch; compiled: also recompile); no bypass flag.
  > `fetch-engine-assets` counts a cache hit only if its rowed members verify: a mismatching prebuilt entry is deleted and refetched, its notice naming the key and `gh cache delete`; compiled groups are untouched. Its `--selftest` recorder prints each leg.
  > Caged part: outside planted positives in `g24-stage-engines.py` and `g24-fetch-engine-assets.py` (the printed legs retire its recorded bound); `check-ci-supply-chain` rule (10)'s refusal retired, its raw-view legs moved onto checks (6)/(8); the `ci.yml` comments; the G37/G56 rows.
  > Spec sync: §6.1.3's intrinsic-verify paragraph (a tree engine's entry keeps its as-acquired archive) and §6.3.4's consumer rule (each consumer of cached engine bytes verifies them, image-worker link inputs included; no step order is policed) with its compiled arm (no upstream URL: recompile).
  > An archive `stage-engines` cannot unpack stages from a fresh in-job acquisition, never a restored tree.
- [ ] **P4.99** [BUILD,RUST] Re-cut the from-source anchor into the §3.8 anchor enum and build each variant's verify leg · §3.8 §3.7.2 · G37
  needs: P4.94
  l-neg1: sweep-tail
  > `crate::engines::lock`: `from_source` carries the §3.8 `from_source.anchor` enum (its four variants, preference order and independence rule are §3.8's); `VerificationTool` gains `Minisign`; `validate()` checks each variant's mandatory fields and rejects, as a `LockViolation`, a corroborating record §3.8's independence rule refuses (the P4.94 hostname parse); `deny_unknown_fields` stays.
  > `scripts/fetch-engine-assets --source`: one verify leg per variant at acquisition, with an explicit keyring or `GNUPGHOME`, never the ambient one; `--selftest` planted-mismatch legs per variant over fixtures (a committed attestation bundle; no network in tests). The real pins land with P4.89.
  > Realizability probe at this box: a verifier absent on an OS (gpg, sq, minisign, `gh attestation`) runs its leg on Linux only, pinned as a sweep tail, without escalation. Fixtures live under `tests/` (a new directory takes its §0.7 row in the same commit), never under the caged `scripts/`.
  > Sweep tail: the verify legs' planted positives from outside the tool in `g24-fetch-engine-assets.py` (the P4.81 list).
- [ ] **P4.56** [BUILD] Build the `engines.lock` schema + the `cargo xtask sbom` two-layer merge scaffold + the engine-source allow-list gate (CycloneDX 1.5, purl+SHA-256 rows) · §3.7.2 §6.3.1 §3.7.1 §3.8 · G35 G35a G35b G37
  needs: P0.7.1
  l-neg1: none
  > Scope: the §3.7.2/§6.3.1 scaffold, tooling and schema only (per-engine rows land in P5–P7, finalized in P10), as three independently failing sub-boxes: the schema (P4.56.1), the `xtask sbom` tool (P4.56.2) and the allow-list gate (P4.56.3, an owner act whose uncaged `lock.rs` half is P4.94).
  - [x] **P4.56.1** [BUILD] Build the `engines.lock` schema (mandatory `purl`+CPE-where-one-exists + per-artifact SHA-256, one row per staged `.so`/`.dll`/`.dylib`, T3a) · §3.7.2 §6.3.1 · G35
    needs: P0.7.1
    > Delivered: 5c24664
  - [ ] **P4.56.2** [BUILD] Build the `cargo xtask sbom` two-layer merge scaffold (app dep-graph + bundled-engine layers, `--spec-version 1.5`, abort-on-mismatch) · §6.3.1 §3.7.1 · G35 G35a G35b
    needs: P4.56.1
    l-neg1: sweep-tail
    > The `cargo xtask sbom` tool, the single merger §6.3.1 *The merge step* and *Invocation* specify (the layers, their tools, the `--spec-version 1.5` pin, the mismatch abort and the flag-drop fallback live there). It fails on a malformed merge or a version mismatch, a failure mode distinct from the P4.56.1 schema.
    > It builds G35a and G35b whole per their build-gates rows, never interface shells: the G35a derivation (its method a `[Build-Session-Entscheidung: P4.56.2]`) proven on committed fixtures that include the row's planted positive; the G35b CycloneDX diff proven on two fixture SBOMs. No other box builds either: P5.67 is G35a's first real run, P10.16 its release run, P10.22 G35b's release emission.
    > G35a's real input is the native worker P4.34 builds over the P4.89 closure (in the P4.89 (d) native job; the worker is staged only at P10.9): the leg skips with a warning while no native build exists and fails closed once one does (an input interface, not a `needs:` edge).
    > Reader: `xtask sbom` reads `engines.lock` through the read-only `gate_api` façade (§0.7 `lib.rs` row), never a second hand-written §3.7.2 reader; first in build order, this box builds the façade, the `gate-api` feature and the xtask path-dependency (the P4.60 core-access door).
    > Realizability probe first: an xtask binary linking the core starts on Windows; on `STATUS_ENTRYPOINT_NOT_FOUND` the P3.87 fix applies without escalation (the `src-tauri/build.rs` link args do not reach xtask, so xtask's own `build.rs` embeds the committed Common-Controls-v6 manifest: `/MANIFEST:EMBED`, `/MANIFESTUAC:NO`, `/MANIFESTINPUT`).
    > Field map: `EngineRow` gains the mandatory `supplier` of §3.7.2 item 1 (validator and fixtures; the committed manifest has no rows, so no migration); the rest of the component map (`purl`, `cpe`, `sha256`, `licence`, `source_ref` + `upstream_url`, `triples`) is the fill's. Invocation: `cargo run -p xtask -- sbom`.
    > The "bundled as a resource" leg is the §6.1.3 build-generated-resource placeholder rule's (P4.41; its ordering against P10.9's compile is P10.18's): this scaffold writes an untracked build-output path and never edits `tauri.conf.json`.
    > Pin tail (the P4.81 list): the `cargo-cyclonedx` and `cdxgen` rows in `scripts/gate-tools.toml`, their only pin home (§6.3.1). The tests run over committed fixture layer SBOMs, so the pins red nothing until P10.14 and close with the P4.81 sweep beside P4.58's Syft pin; a P4 lane that must run either tool live makes that pin its own `[!extern]` precondition box.
    > Pin facts (measured): cargo-cyclonedx 0.5.9 ships linux/macOS as `.tar.xz`, which `scripts/install-gate-tools` does not extract, so the pin act adds a `tarxz` `asset_kind` with its `g24-install-gate-tools` extract leg (corroboration: the GitHub attestation) or uses `cargo install --locked` with a re-blessed G35 row naming `cargo-cyclonedx@<ver>`.
    > cdxgen is a `raw` row, never a devDependency (that would add its tree to `pnpm-lock.yaml`, which G17 audits and G36b licence-scans, so no `package.json` floor row); its `.sha256` is same-origin and carries no attestation, so the row needs a second independent origin or an honest RESIDUAL line.
    > Invocation `cdxgen -t js --no-recurse --spec-version 1.5` (its own default is 1.7); `--required-only` is decided at the fill against §6.3.1 and G36b, escalated if they disagree. The pin act discharges §6.3.1's `[DEFER: verify]` with the pinned version's default specVersion (its `--spec-version` doc comment).
  - [!extern] **P4.56.3** [GATE] Build the committed engine-source allow-list gate — every `engines.lock` URL on an allow-listed origin of its engine, independence per the §3.8 origin model · §3.8 §3.7.2 · G37 G24
    needs: P4.56.1, P0.7.3, P4.94
    > Owner act (L(-1)); P4.89 needs it, so it lands as its own act, not with the P4.81 caged tails. It executes the §3.8 engine-source allow-list with its *Origin model* and *Independence by acquisition mode*; the uncaged `lock.rs` half is P4.94.
    > Caged deliverables: `scripts/check-engine-sources`; the committed allow-list under `scripts/` (e.g. `scripts/engine-sources.toml`, caged by `scripts/**`), seeded with the §3.8 origins, which P4.89 (b) widens with its rows; its G24 self-test `g24-check-engine-sources.py`; the Lane-A `ci.yml` wiring; the `engines.lock` header re-bless (the clause that assigns origin independence to this gate).
    > Self-test legs: an off-allow-list origin fails; a same-URL-twice and a same-host `mirrors` pair each fail; a `mirrors` row whose `corroboration_urls` all sit on `upstream_url`'s host fails; a `signed-repo` row and a from-source row with a same-host signature pass; the clean tree passes. The gate re-runs as each P5–P7 row lands (vacuous on an empty manifest).
    > Docs sync in the act: the G37 row's allow-list sentence and security-concept T3's gate column gain the transport-set and pairwise-independence wording, pluralize "corroboration URL" to the `corroboration_urls` field, and scope T3's "pin URL and corroboration URL on independent origins" to the per-mode reading.
    > Same act, uncaged: `scripts/fetch-engine-assets` re-points onto the committed artifact (`group_plans` and the `--source` plan builder read each engine's provenance ∪ transport sets) and moves its three `netloc` comparisons, `assert_fetchable_url`'s included, to `hostname`; decide the port rule explicitly.
    > Same act, caged: `g24-fetch-engine-assets.py` and its `.legs` re-pin the derivation-equality, off-row-URL and off-allow-list-message legs to the artifact, add a leg that tells a hostname from a netloc comparison, and re-bless every `--selftest` leg name the re-point renames or removes (the monotone pin does not cover a rename).
    > Out of scope: the G37 signing-key half (key material, home, verifier binding, cross-check leg) is P4.89 (a)'s; this act adds no placeholder for it, does not duplicate `verified_with`, and leaves the G37 keyring clause untouched. Today each row carries `from_source.signing_key_fingerprint`, and `scripts/compile-engine-asset` verifies against the runner's ambient keyring.
    > Until it lands, the fetch hop check admits only the row-named hosts (fail-closed).

## The image-worker `convertia-imgworker` (the first real sidecar)

- [ ] **P4.83** [RUST] Split `convertia-imgworker` into lib + thin bin (`crates/imgworker/src/lib.rs`; the P3.87 minimal-pub pattern) — the P4.35.1 fuzz lib-target precondition · §3.5.5 §0.7
  > The P3.87 minimal-pub pattern on the worker crate: the crate-root module declarations (the single G29-allow-listed `ffi` module included) move to a new `crates/imgworker/src/lib.rs`; `main.rs` becomes the thin entry calling the lib's run/dispatch fn; modules stay `pub(crate)` except the narrow pub surface the entry needs; the fuzz-entry module is P4.35.1's.
  > No new directory (the G69 set is unchanged) and no §0.7 row. Same commit: G29 (`ffi.rs` stays the crate's one `allow(unsafe_code)` site; check-unsafe-policy rule 1 walks `src/lib.rs` and `src/main.rs`, so both roots carry `#![deny(unsafe_code)]`), G53 and G27 green.
  > The worker has no check-rust-lint-contract `REQUIRED_ATTRS` row (those globs are `src-tauri/src/`-only); its G4 leg is the workspace `cargo clippy -D warnings`.
- [ ] **P4.34** [BUILD,RUST] Build `convertia-imgworker` as its own binary over the verified minimal PNG closure (`build.rs` link switch + link-input verify) · §3.5.5 §3.6.1 · G53 G37
  needs: P4.27, P4.32, P4.28, P4.28.1, P4.89
  > The §3.5.5 worker binary, statically linked over the minimal PNG closure P4.89 acquires (libvips, libspng/zlib, GLib/GObject/libffi/pcre2, expat), never linked into the MIT core (G53). This box owns `crates/imgworker/build.rs`; the CI legs and toolchain pins are P4.89's.
  > Link switch per the §3.5.5 build shape: `build.rs` emits `cargo:rustc-cfg=imgworker_native` (with `rustc-check-cfg`) and resolves the closure with `pkg-config --static` restricted to the verified prefix (`PKG_CONFIG_LIBDIR=<prefix>/lib/pkgconfig`).
  > Link-input verify, before any link directive: under a P4.89 (a′)(i) output anchor, re-hash every consumed archive and header against it; under (a′)(ii), require `compile-engine-asset`'s compile record for this workflow run (`GITHUB_RUN_ID`/`GITHUB_RUN_ATTEMPT`), so a restored prefix never links; outside Actions the host's own compile record is accepted.
  > A mismatch fails the build (planted-mismatch test).
  > Packaging: no `externalBin` entry here. Lane A, L1 and L2 resolve the cargo-built worker from the profile dir through P4.32; the entry lands with the release build-and-place (P10.9), whose placed worker must be native.
- [ ] **P4.35** [RUST] Build the imgworker Rust↔FFI surface + the in-worker decode/encode `Invocation`-equivalent plan · §3.5.5 §3.2.2 · G29 G48
  needs: P4.34, P4.2, P4.83
  > First-party `extern "C"` libvips declarations in the single G29-allow-listed `crates/imgworker/src/ffi.rs` (no crates.io binding), compiled only under `imgworker_native`; the Rust-only build answers `--version` and fails every conversion with a distinct exit.
  > Wire contract (this box authors §3.5.5's `Shape:` and `Exit/stderr` bullets): one invocation = one load→transform→save conversion; argv `key=value` (source `FormatId`, `TargetId`, input, `out_tmp`, options; resource roots as named keys, never env). §3.5.5's "not via argv" is the libvips call inside the worker, not this wire.
  > Emits the §3.5.5 `VipsStdout` progress wire (eval-progress → stdout `progress=<0..100>`); the consumer is the delivered P4.8 reader.
  > Operation allow-list via `vips_operation_block_set` through `ffi.rs`. Realizability probe at this box: if the pinned libvips lacks it, `VIPS_BLOCK_UNTRUSTED=1` in P4.37's env allow-list applies without escalation. librsvg is P5's.
  - [ ] **P4.35.1** [TEST] Instantiate the P0.4.3 imgworker-FFI G48 fuzz leg against the real `convertia-imgworker` Rust→FFI surface · §6.4.2 §3.5.5 · G48
    needs: P4.35, P0.4.3, P4.83
    l-neg1: same-push
    > Activates the P0.4.3 imgworker-FFI G48 leg: a cargo-fuzz target over the Rust→FFI surface linked against the minimal closure, pinned libFuzzer bounds, a committed corpus replayed on all three OS under stable by an imgworker-crate `#[cfg(test)]` mirror of the P3.67 replay (never the core `fuzz_replay`).
    > The mirror is a `#[cfg(test)]` file beside the P4.83 `crates/imgworker/src/lib.rs` (no new directory) that walks `fuzz/corpus/imgworker_ffi/` and `fuzz/crashes/imgworker_ffi/` through the pub fuzz-entry fn this box adds to the lib root, under `cargo test -p convertia-imgworker`; `fuzz/crashes/` is not created here (the P0.5.8 convention: the first crash-committing box owns its rows).
    > Same commit: the `fuzz_replay.rs` texts naming P7.50.1 as the only remaining key-adder, a pin that the core walk routes `imgworker_ffi/` harmlessly, and the §0.7 rows (the `fuzz/corpus/imgworker_ffi/` row; the `fuzz_targets/` row's "importing only `convertia_core::fuzz_api`" text).
    > Caged part: `.github/workflows/fuzz.yml` (the target loop, the header comment, the G56 timeout cap for 5×300 s, the closure restore with its consumer verify). `check-fuzz-contract` arms on this box's `[x]`.
- [x] **P4.36** [RUST] Build the imgworker `VipsStdout` progress marshalling (eval-progress callback → stdout `progress=<0..100>` key=value) · §3.5.5 §1.11 · G29
  > RECONCILE: folded into P4.35 — the worker emits the `VipsStdout` wire with its FFI surface.
- [ ] **P4.37** [RUST] Wire the image-worker through the §2.12 isolation boundary + the §0.9 image-core pool (one short-lived worker per item) · §3.5.5 §2.12.4 §0.9 · G29 G31
  needs: P4.35, P4.13, P4.20
  > The `ImageCoreEngine` `impl Engine` in the per-engine module `engines/image.rs` (a new file, no §0.7 row), registered in `registry.rs` `registered_engines()` as the first subprocess engine, which gives the P4.32 `Sidecar` dispatch arm its producer and `subprocess_target` its registry handle.
  > `parallelism()` answers the §0.9 image-core row; `plan()` builds the `Sidecar(ImageCore)` `Invocation` from the P4.35 wire contract (cwd the per-run scratch, the §3.5 minimal env; resource roots arrive as named argv keys, §3.5.5, never env); `classify_failure()` reads P4.35's `Exit/stderr` bullet.
  > One capability row, `PNG→PNG`, `Direction::Encode`, registry-only: `Direction::Both` on a diagonal inserts the `(Png, Format(Png))` key twice (`RegistryBuildError::DuplicatePair`, so the whole startup registry fails). §1.5 rule 5 keeps image diagonals out of the P4.92 offer (C3/C6); P4.38 and P4.26 enter below C6.
  > The row confirms the forward-key reading (only `Both` registers the reverse cell) against §04 images.md.
  > Options seam: widen `Engine::plan` and `plan_encode` with the batch `&OptionValues` (the P4.35 argv carries them; §3.2.2 names an out-of-range option a planning failure) and sync the §3.2.2 signature in the same commit. Same commit (plan-lint check 29): every "no subprocess engine registers until P5–P7" comment in `registry.rs` and `engines/mod.rs` flips; engine-staging claims stay.
  > Additive input grants (§2.12.3 `[DEFER: tuning]` grant-set completeness; `run_confined` has no structured input path, §3.5 flattens it into `plan.args`): (a) the Linux Landlock `{input ro}` grant.
  > (b) The Windows Leg-A input check: skip the token lowering when the input carries an explicit mandatory label at or above the confined level with `NO_READ_UP`/`NO_EXECUTE_UP` (`crate::platform::label_blocks_lowered_access` takes the level; its engine-binary half is live).
  > Landlock `.part` sink: the realized Linux leg grants the working directory but not the `out_tmp` beside `final` (§2.14.1) that §2.12.3's scratch grant names, so add its file rule or the first output write is denied wherever Landlock enforces. Test: a `run_confined` Linux leg writes a real `out_tmp` outside the working directory under an applied ruleset.
- [ ] **P4.38** [RUST,TEST] Build the imgworker round-trip proof-of-life invocation through the isolation boundary · §3.5.5 §2.12 §6.4.3 · G31 G26
  needs: P4.37, P4.7, P4.59
  > The P4 proof-of-life: a representative image round-trip through the §1.7 lifecycle and the §2.12 isolation wrapper (spawn worker → decode/encode → `VipsStdout` progress → exit 0 → non-empty `out_tmp` → §2.1 atomic publish), driving the P4.37 engine through the tier-1 conductor below C6 with a raw `Target`; the G26/G31 corpus fault-injection oracle binds here.
  > Per-format pairs and per-engine hardening are P5's.
  > Structural reader: G26's positive output-validity leg and DoD item 3 bind here; the image reader (`vipsheader` decode + dims, `vips stats` variance; G26/G31, §6.4.3) is provisioned by P4.89 (e), and this box reaches it through the P4.59 per-category dispatch. The round-trip's own self-report re-decodes nothing and is never the reader.
  > Test root: tests resolve the worker from the cargo profile dir (the parent of `target/<profile>/deps`), published to `ProgramRoots` by one shared `#[cfg(test)]` helper in `crate::engines::program`; `program.rs` tests built on another root are re-cut with `[Test-Change: P4.38]`. A missing worker binary fails, never skips.
  > The round-trip tests compile under `imgworker_native` (this box makes the core's `build.rs` set it from the same env var), so they run in the native job and the Linux Docker validation.
- [ ] **P4.26** [TEST] Build the G31 macOS T11 first-accessor sub-test over a real spawn (the core reads first; the engine argv carries only the staged kind-2 path) · §3.5.0 §7.2.6 §0.11 · G31
  needs: P4.25, P4.85, P4.32, P4.38
  > The G31 macOS T11 first-accessor sub-test over a real spawn (§3.5.0 T11 verification posture): on macos-14, drive a real engine through the conductor and assert (1) argv/stdin carries only the staged kind-2 path, (2) the core read the source first.
  > The real-spawn leg stays the behavioural proof; additively, a `#[cfg(test)]` argv capture at the single `run_confined` door and a portable staging twin assert the same on every OS. Its tests join P4.38's test module, in its own commit after P4.38.

## Patent-disposition matrix & availability wiring (§3.4)

- [x] **P4.39** [DOC] Verify the §3.4 patent-disposition matrix's DECIDED cells against the §04 source-codec set + record the codecs §3.4 leaves unclassified as the owner question (do NOT re-author, do NOT author a cell) · §3.4 §3.4.2 §3.4.3 §3.4.4 · G7
  needs: P0.1.1
  > Delivered: d0d54c6
- [ ] **P4.40** [BUILD,RUST] Build the §3.4.4a `engines.lock` `available` boolean → `PatentDisposition` parse→map flow · §3.4.4a §3.2.2 · G35 G37
  needs: P4.39, P4.3, P4.56.1
  l-neg1: none
  > Keep the edge on the schema sub-box P4.56.1, never the parent: the SBOM tool and the allow-list gate are no inputs of this flow, and `needs: P4.56` would park it behind P4.56.3's owner act.
  > Builds §3.4.4a as written there (the `codec` key and its two validator laws, the absent-row defaults, the one compile-time embed, the parse→map flow before any `capabilities()` call): `EngineRow.codec` (`CodecKey`; wire tokens the §3.2.2 `PatentDisposition` field names), each law its own `LockViolation`, the map, and the embed in place of `registry.rs::resolved_patent_disposition()`.
  > The P4.94 real-manifest leg reads the same embed const; whichever of the two boxes lands first creates it. The C3 offer marking is P4.92's; `EngineHealth.unavailable_targets` mirrors it (P4.45).
  > As the first production reader of `engines.lock`, this box promotes `toml` from `[dev-dependencies]` to `[dependencies]`; its floor row exists (a floor is keyed by crate, not dep-kind), so none is added. The same commit re-cuts the §0.8 `toml` row's "dev-only" parenthetical and the `src-tauri/Cargo.toml` `[P4.19]` DEV-ONLY comment.
  > It reads the committed manifest only through the embed and tests over fixture files; it never edits `engines.lock`.

## Startup engine-presence + integrity verification (§7.2.3)

- [ ] **P4.41** [BUILD] Build the build-time in-bundle hash manifest GENERATION (one row per bundled FILE: `{path, engine, check, expected_hash, expected_size}`) · §7.2.3 §6.2 · G37 G35
  needs: P4.27
  l-neg1: sweep-tail
  > Scope: the §7.2.3 per-file in-bundle hash manifest (one row per bundled file, `{path, engine, check, expected_hash, expected_size}`), generated by `stage-engines`, the input the warm-launch verifier consumes; a corruption check, not a tamper anchor (§0.11 T3). The second of P4.41/P4.51 to land retires the `stage-engines` SCOPE sentence and its freshness legs (P4.51 (d)).
  > Rows: `path` is the bundle-relative key, walked over the full §3.3.1 closure after the P4.29/P4.30 tails (`binaries/` with the cargo-placed `convertia-imgworker`, the declared `bundle.resources` trees, the licence-text map entry), never a projection of `STAGED_ENGINES`.
  > The manifest's own path is never a row (it is written after the walk); one leg generates a real manifest and re-verifies every row against the tree it came from.
  > `engine: Option<EngineId>` is the owning engine per the §3.3.1 map (each sidecar its own id; the FFprobe→FFmpeg roll-up is P4.45's surface; the licence text is `None`).
  > `check ∈ {exec-magic, full-hash, size-only}` is fixed at generation, so P4.43 never re-classifies: the six §3.3.1 sidecars `exec-magic`; a declared per-file `full-hash` set (the G46-r7 security configs, carried as data like `RELOCATIONS`, empty here, filled by P5.6, P5.29.1 and P7.5); every other file `size-only`.
  > Carrier (the §6.1.3 build-generated-resource rule): (1) a committed `src-tauri/resources/hash-manifest.json` with an empty entry set and `"placeholder": true`, plus its `.gitignore` negation line.
  > (2) Declared in `bundle.resources`; §3.3.1's literal and §3.3.2 step 4 gain the entry in the same commit. (3) `stage_all`'s tail overwrites it on both return paths after `relocate_all`, never under `write=False`.
  > (4) The P4.31 binder gains a named generated-resource set for `resources/`-prefixed keys, the `BUILT_SIDECARS` analogue with its both-exempt-and-staged self-audit leg (re-blessed at the phase-end sweep under the monotone pin). (5) Not registered with G19 (per-build output).
  > (6) P4.43 fails closed on a declared engine file with no row (`BundleDamaged`). (7) The committed copy keeps `"placeholder": true`: a `check-repo-invariants` leg, a caged tail that reds nothing and closes with the P4.81 sweep.
  > Order: generate after the P4.30 relocation tail and the P4.29 `lipo -create` merge, on both `stage_all` branches (both re-emit load commands, so an earlier manifest records hashes no shipped file has); wire and pin both tail call sites (a one-branch tail is a suite-invisible defect, P4.30's mutation battery).
  > `stage-engines` stays an uncaged Loop tool (a `[[loop_tool]]` escape in `scripts/l-neg1-files.toml`): its manifest is a corruption check, never a trust anchor, and G37 reads it only against the same run's bytes.
  > Condition: each assertion arm it hosts (P4.51, P4.53, P4.76, the P5.1/P6.1/P7.1/P7.17/P7.21 staging fills) gets a planted positive from outside the tool in the caged `g24-stage-engines.py`, because the monotone leg-name pin catches a removed or renamed leg, not a neutered body.
  > That positive is a caged tail per box that reds nothing until it lands: the P4 arms close at the P4.81 sweep, each P5–P7 arm at its phase's sweep.
- [ ] **P4.42** [RUST] Build the §7.2.3 presence loop over the expected BINARY list (bare runtime names, not the trait registry) · §7.2.3 §0.4.1 · G46 G29
  needs: P4.32, P4.33
  > Scope: the §7.2.3 out-of-band presence loop over the §3.3.1 expected bundled-binary list, filling the P2.106.3 `verify_engine_presence` slot (§7.2.1 step 3).
  > It iterates P4.33's typed engine-program table, never a second enumeration, resolving every row through `crate::engines::program::resolve_program` over the published `program_roots` (the one resolver the §1.7 spawn uses), never a bare-name `sidecar_path` pass.
  > §3.3.3 owns the name table and the Windows `.exe` rule (P4.32's `sidecar_binary_name`); §7.2.3 owns the FFprobe→FFmpeg roll-in (surfaced by P4.45); the in-bundle manifest is P4.43's input. Table rows the build does not declare are skipped (§7.2.3 *Build-window posture*).
  > The LibreOffice row is §3.3.1's `[DEFER]` (launcher as `externalBin` vs in resources, decided at P7.1); P7.4 adds it once P7.1 fixes the path. The DEFER's discharge in the P7.1 commit sweeps every sibling naming the bare `soffice` sidecar: §3.3.1, §3.3.3, §7.2.3, `program.rs` (with a `[Test-Change: P7.1 …]`), `stage-engines`, spec README. This box does not re-cut `program.rs`.
  > Class-closer: a structural test binds the table to the §3.2.3 registry: for every registered engine whose representative `plan()` yields a `Sidecar` or `ResourceBin` program (in-process engines have no row), that program equals the table row keyed on its own `EngineId` and is declared in the compiled `bundle` config.
- [ ] **P4.43** [RUST] Build the integrity verifier — hash-on-first-launch + `engine-integrity.json` warm marker + cheap warm size/header check · §7.2.3 · G46 G29
  needs: P4.42, P4.41
  > implements the §7.2.3 `[DECIDED]` strategy as written there — the `engine-integrity.json` marker, the `app_version` re-hash rule, the warm size/header check and the size-only non-binary rule; §7.2.3 is the only copy. It reads the P4.41 per-file manifest rows and applies each row's `check` class, never re-classifying. Plus the G46-r7 bundled-security-config leg (build-gates G46): the `full-hash` rows (`policy.xml` / `registrymodifications.xcu` / coder and fontconfig configs, staged by P5.6 (asserted by P5.7), P5.29.1 and P7.5, so this leg is built ahead of its rows) are hashed IN FULL on every launch, and a blanked or damaged config → `BundleDamaged`. The §7.2.3 spec carrier for that leg lands with this fill (DoD 2 — G46 already promises it). The placeholder manifest follows the §7.2.3 Build-window posture, tested on both arms through a pure verdict fn that takes `cfg!(debug_assertions)` as a parameter.
- [ ] **P4.44** [RUST] Build the §7.2.3 smoke probe (cheap `--version`-style run through the §2.12 wrapper) · §7.2.3 · G46 G29
  needs: P4.43, P4.37, P4.82, P4.33
  > the §7.2.3 smoke probe; the BMP-delegate exercise is P5.25's.
  - [ ] **P4.44.1** [RUST] Build the general `--version`-style smoke per critical engine through the §2.12 wrapper (glibc/arch-mismatch → `runnable = Some(false)`) · §7.2.3 · G46 G29
    > A fast `--version`-style invocation per critical engine through the §3.5/§2.12 wrapper (it catches a glibc/arch mismatch a hash cannot), cheap and gated behind verbose mode on warm launches.
    > A smoke failure sets that engine's `runnable = Some(false)`; `None` means the probe did not run (skipped: the warm-launch fast path or the macOS deferred spawn, §7.2.3) or was quarantine-blocked (P4.46.1 (i)). The BMP-delegate exercise is P5.25's.
    > Placement per §7.2.3 *Placement*: after `prepare_scratch_and_log` (step 5), before the step-6 reveal; the cwd is a `.lock`-held `run-<probe RunId>/` from `RunScratch::acquire`, released by `cleanup_run` (the §7.1.2 carve-out).
    > The spawn runs through `bounded_confined_run` (private today: home the probe in `crate::engines` or widen it `pub(crate)` with a tag), and a step-order pin beside `readiness_gate_chains_steps_3_4_5_in_order` asserts the smoke follows step 5. `convertia-imgworker` answers `--version` per P4.35's wire contract.
  - [x] **P4.44.2** [RUST] Build the imgworker BMP-delegate exercise (the delegate-registry check for `magickload`/`magicksave` and the `BMP` coder → `ImageCore.runnable = Some(false)` on fail) · §7.2.3 §3.5.5 · G46 G29
    > RECONCILE: re-homed to P5.25 — the ImageMagick delegate is acquired with the P5 image stack.
- [ ] **P4.45** [RUST] Populate the C12 `EngineHealth`/`EngineStatus` contract (incl. synthesized NativeCsvTsv row + `unavailable_targets` from the resolved §3.4.4a flag) · §7.2.3 §3.4.4a §0.4.1 · G46 G29
  needs: P4.44, P4.40, P2.111, P4.92
  > Populate C12 `EngineHealth` per §7.2.3 (the declared roster, the FFprobe/ImageMagick roll-ins, the synthesized `NativeCsvTsv` row, `unavailable_targets`, the derived `all_critical_ok`); `unavailable_targets` reads the resolved §3.4.4a `available` flag, the §3.4 per-platform gaps and the §3.1 degraded targets. The §3.4 build-time gap marking is P4.92's; this box owns the health-cache degrade only.
  > Required vs degradable is §3.1 *Startup-fault classification*: `all_critical_ok` reads the roster from one named const beside the §3.3.1 binary list, filtered to the declared set, and neither `runnable == None` (skipped or quarantine-blocked) nor a delegate-absent record (P5.25) clears it.
  > C3 marks the §3.1-degraded targets `Unavailable { reason }` (the §2.8.2 `degraded_component` line, never the patent sentence) over the P4.92 offer, from the health cache and the P5.25 attribution record; `unavailable_targets` stays reason-less.
  > The `degraded_component` line is a hoisted `crate::outcome` template, bound per row in `every_catalog_row_is_bound_to_its_spec_table_row` as P4.72's batch lines are.
  > The cache C12 reads stays updatable after the readiness gate (the macOS smoke runs after step 6, P4.46.1): a Mutex-wrapped `crate::engines` static, never a write-once `OnceLock`, which keeps C12 directly testable (the C11 `AppInfo::gather` precedent); a `State<'_, _>` injection supersedes `c12_contract` per the C8/C9 pattern.
  > Same commit: the C12 `///` rewrite and the `EngineHealth`/`EngineStatus` docs in `engines/mod.rs` (the declared roster, no longer "registry-eligible"; `unavailable_targets`, `all_critical_ok`) mirror into `bindings.ts` (G19); the `PatentDisposition` doc there and the `registry.rs` availability doc, which still name the §5.2 disable/omit set, are re-cut. The tile render is P4.70.2's.
- [ ] **P4.46** [RUST] Wire the missing/corrupt/non-runnable-engine outcome — app-fault vs degrade-to-unavailable · §7.2.3 §2.13 §2.8 · G46 G29
  needs: P4.45, P4.50
  > Scope: the §7.2.3 *Outcome of a failure* routing for every engine and platform over §3.1's *Startup-fault classification* of the declared engines: a required engine missing, corrupt or non-runnable → the app-level startup fault (`EngineMissing`, or `BundleDamaged` for an integrity mismatch, with the §7.7 link); a sub-component failure inside a present engine → those targets unavailable (§5.2).
  > It builds the §2.13.3 readiness-presentation body the P2.106.3/P2.109 `present_startup_fault` shell defers (log-only until here): the §0.4.2 `app://fault` emit to the §5.8 screen and the `PendingFault` buffer replayed once the §5.8 listener registers (the first-frame race, §7.2.1). P4.50.1 keeps the `WebviewFault` native-surface body; the frontend listener is live since P3.60.
  > Its emit is pinned against the final `AppFault.kind` set, P4.82's `ScratchUnavailable` included. The macOS `QuarantinedByOs` lazy-probe ordering is the sub-box P4.46.1 (a macOS quarantine fixture and a distinct startup ordering; it fails independently).
  - [ ] **P4.46.1** [RUST] Wire the macOS `QuarantinedByOs` lazy-probe ordering + per-sidecar retry flow (deferred post-reveal smoke) · §7.2.3 §7.2.4 §2.8 · G46 G29
    needs: P4.33
    > The macOS ordering caveat (`cfg(target_os = "macos")`, §7.2.3): the smoke runs after the step-6 reveal and `QuarantinedByOs` is classified lazily at conversion time, so it surfaces in a window, distinct from `EngineMissing`/`BundleDamaged`; the per-sidecar retry flow is §7.2.4's (no auto-retry, name the blocked sidecar). Needs a quarantined-`xattr` sidecar fixture; it fails independently of P4.46.
    > (i) The deferred smoke writes the updatable health cache (P4.45) that C3 reads on each Confirm→Targets advance, so no §0.4.2 event and no `AppFault` kind is added. A smoke spawn the (ii) classifier marks quarantined records `runnable = None`, never `Some(false)` and never a degraded-target record (§7.2.3), so only the item-level §7.2.4 failure and its retry flow surface.
    > Same commit: the `EngineStatus.runnable` `///` doc in `engines/mod.rs` and every other code site that glosses `runnable == None` as probe-skipped read "None if skipped or quarantine-blocked" (grep), mirrored into `bindings.ts` by `xtask codegen`.
    > (ii) The discriminator is the fill's measurement, not a fork: on macos-14, spawn a quarantine-marked unsigned Mach-O from a tokio parent and record whether the spawn fails (which errno) or the child is signal-killed after a successful spawn, and whether "Open Anyway" removes the xattr or only rewrites its flag field (§7.2.4's "cleared by macOS" is unmeasured).
    > The classifier lives in `crate::platform` under `cfg(target_os = "macos")`, reads the xattr and never strips it (§7.2.4), and gates the arm the probe shows (the `run_confined` spawn `Err` arm and/or the signal-death path through `classify_exit`) on the quarantine state the probe shows is authoritative, never on mere xattr presence.
    > Each arm's non-quarantine outcome stays: a spawn `Err` stays the P4.7 item-level `InternalError` (`EngineMissing` has no §2.8.2 per-item row).
    > (iii) Arg duty (§2.8.2, per raise site): `convert_item`'s `InvocationResult::Failed(kind)` arm calls `fail_cleanup_named` with the friendly name of the failed sidecar from `envelope.engine` (FFprobe counts as its own sidecar); author the missing `EngineId` friendly-name fn.
    > The name reaches both renders through the existing `name_arg` chain; `project_outcome` has no production caller and needs no change.
    > G46 sub-test: a classifier unit test with an injected quarantine-state fn (every OS), and a macOS integration test over a real xattr-marked fixture asserting the rendered line names the sidecar.

## App-level fault model & panic boundary (§2.13)

- [ ] **P4.47** [RUST] Build the worker-thread `catch_unwind` panic boundary (per-item isolate-and-report, `panic="unwind"`) · §2.13.2 · G29
  needs: P4.7
  > The §2.13.2 worker-thread boundary as written there (its convert-loop per-item clauses; the C1 intake clause is P4.48's); the run-level escaped-panic half of §2.13.3 is P4.50.2's. The InProcessNative lane's boundary is delivered (P3.3 `pool::run_in_core`'s `spawn_blocking` `JoinError` → `LaneError::Panicked`, mapped by P3.46 `bounded_lane` to `InternalError`): expand it, never re-wrap it.
  > Log that lane's payload at the `run_in_core` map site, never through an inner `catch_unwind` in the closure. Pin `panic = "unwind"` in the root virtual `Cargo.toml`: a member-manifest `[profile.release]` is ignored with only a warning and `panic` is rejected in a `[profile.release.package.*]` override, so the pin also binds `convertia-imgworker` and `xtask`.
  > Mechanism (§2.13.2's poll-level catch; `pool::run_subprocess` defers its panics to it), std only: `std::pin::pin!` the future and poll it in a `std::future::poll_fn` inside `catch_unwind(AssertUnwindSafe(..))`, `Err(payload)` resolving Ready; wrap each `convert_item` await in `orchestrator::run_conversion`.
  > A caught payload becomes that item's `ItemRunOutcome::Failed { kind: InternalError, .. }`; the helper's §0.7 home is a `[Build-Session-Entscheidung]`.
  > Rejected: a hand-rolled `CatchUnwind<F>` (its pin projection needs `unsafe`, denied at the crate root, G29); `futures::FutureExt::catch_unwind` or `pin-project-lite` (transitive-only today, so a new pinned dependency with its §0.8 and floor rows); a per-item `tokio::spawn` (the conductor's borrows are not `'static`).
  > Payload capture: a `catch_unwind` or `JoinError::into_panic` payload carries only the message, and the §2.13.2 location exists only inside a `std::panic::set_hook` callback, on the panicking thread.
  > So one process-wide hook, installed early in `convertia_core::run()` before the Builder (not G28 boot-glue-exempt: `run()` has no `AppHandle`), records the `Location` keyed by `tokio::task::try_id()` (the thread id outside a task) in a bounded process-wide slot; each read removes its entry. Test the installer in its own process: a global hook re-routes every parallel test's panic output.
  > Each boundary reads that record by task id (at `run_in_core` through `JoinError::id()`), never through a thread-local: the hook runs on the `spawn_blocking` thread and the map site on the awaiting worker (probed on tokio 1.52.3).
  > Redaction, a `[Build-Session-Entscheidung: P4.47]` inside §7.5.3 (`crate::log_redact::RedactedPath` handles only `&Path`): the default-level line carries the §2.8 kind, `RunId`/`ItemId` and the compile-time `file:line`, never the message.
  > Decided at the fill: whether a verbose `debug!` line may carry the message or only a structural stand-in (a panic in the in-core CSV lane may embed decoded data, which §7.5.3 bars at every level), and replacing vs chaining the default hook via `take_hook`.
- [ ] **P4.48** [RUST,UI] Build the intake/detection panic boundary (C1 — per-path → Uncertain, whole-walk → calm IpcError) · §2.13.2 §5.2 · G29
  needs: P4.47
  > The §2.13.2 intake/detection boundary (C1 `drain_intake`), both granularities as written there; C2a is outside it since P3.78 (it opens the picker and buffers). The whole-walk leg is delivered (P3.49 runs the walk in `tauri::async_runtime::spawn_blocking` and maps its `JoinError` to the calm `IpcError`): expand that map site, never re-wrap it with an inner `catch_unwind`.
  > (1) Per-path boundary in `crate::orchestrator::ingest`: wrap only the untrusted-byte decode of one path (the header read + `crate::detection::detect`) in `catch_unwind(AssertUnwindSafe(..))`, with the stat outside so `size_bytes` survives; a caught panic becomes that path's `DetectionOutcome::Uncertain { best_guess: None }` and the walk continues.
  > (2) The §7.5 payload log at both sites per P4.47's capture and redaction; the P3.49 map site discards the payload today, so match `tauri::Error::JoinError` and take it.
  > (3) Presentation: a C1 `IpcError` rejects `commands.drainIntake` unhandled in `consumeIntakeNudge` / `consumeMountDrain` (`src/lib/ipc/events.ts`), so the machine stays in `Collecting` and the mount drain drops the launch set.
  > Add the §5.2 row-2 exit (C1 `IpcError` → state 10 `Unreadable`, §2.13.2's "couldn't read these files"), the machine arm and the façade catch in one commit, §5.2 synced, a `[Build-Session-Entscheidung: P4.48]` at the arm.
  > (4) Tests: `drain_intake` needs an `AppHandle` and the crate ships no `tauri::test` mock, so move the `spawn_blocking` + `JoinError` → `IpcError` map into an AppHandle-free helper driven by a planted panicking closure; a planted per-path panic leaves that path `Uncertain` while the walk collects the rest; a rejected C1 lands in state 10, never stuck in `Collecting`.
- [ ] **P4.49** [RUST] Build the engine-output capture-and-classify rule (bounded stderr head and tail + coarse stdout cap; never raw to the user) · §2.13.4 §1.7 §2.8 · G29
  needs: P4.8, P4.1, P4.12
  > The §2.13.4 rule, dispatched through the §3.2.2 `Engine::classify_failure` seam; the per-engine §3.5.x classifiers are P6.12 / P7.13 / P7.20 / P7.26.
  > Capture (P4.8, `crate::isolation::run_confined`'s `ConfinedRun.stderr`) and dispatch (P4.12, `classify_exit` → `Engine::classify_failure`, encode and probe legs) are delivered and never re-implemented; "never raw to the user" holds by type (`InvocationResult::Failed` carries only a `ConversionErrorKind`).
  > (1) The §7.5 raw-stderr log at the `classify_exit` site: the raw text at `debug!` only (the §7.5.4 verbose set, the P2.94.1 producer contract), because engine stderr embeds full input paths §7.5.3 bars at the default level; a default-level line carries only engine id, exit code and classified kind.
  > (2) The unclassifiable floor (§2.8, §2.12.1): abnormal termination (a Unix signal death or a Windows NTSTATUS exception-range exit) → `EngineCrash`; a clean nonzero exit with no matched pattern → `EngineError`. Today `classify_exit` hands every completed nonzero exit to the engine and drops `run_confined`'s `EngineCrash` floor: build one shared generic fallback every §3.5 classifier falls back to.
  > Add no `classify_failure` default body (§3.2.2's trait block has none; one would edit §3.2.2 in the same commit); the native engine's P4.1 `InternalError` answer stays. (3) The never-raw pin: a test that `InvocationResult::Failed` stays kind-only, and an `include_str!` source-scan that `ConfinedRun.stderr` has no production reader besides `classify_exit` and the (1) log site.
  > Also the §1.7 capture bounds on the P4.8 capture: the stderr head and tail and the `CoarseSpawnDone` stdout cap (over it, `Corrupt`). Same commit: every capture doc, test message and assertion still saying "in full" (`crate::isolation`, `crate::engines`, `crate::engines::registry`).
- [ ] **P4.101** [RUST] Emit the §1.7 coarse-progress contract (spawn/running/done ticks, probe rescaled into 0.05..=1.0) · §1.7 §1.11 §0.4.2 · G29 G31
  needs: P4.8, P4.9, P4.12
  > Scope: the §1.7 coarse progress contract (landed) — the three `CoarseSpawnDone` stages, and the probe in 0.0..0.05 with the encode rescaled into 0.05..=1.0 (monotonic, §0.4.2); the streaming-only no-progress leg is P4.12's, delivered. Tests: the event sequence per `ProgressModel`; monotonicity across the probe→encode boundary.
  > Same commit (DoD 2): the `bounded_confined_run` and `NO_PROGRESS_TIMEOUT` docs drop output-file growth as a planned no-progress signal (§1.7: the wall clock bounds `CoarseSpawnDone`), and the `PROGRESS_CHUNK_BYTES` doc and its sub-chunk test comment drop "wire-indistinguishable from `CoarseSpawnDone`".
- [ ] **P4.50** [UI,RUST] Build the §2.13.3 app-level fault presentation (single calm no-trace screen; startup/mid-run-panic/WebView-disconnect classes) · §2.13.1 §2.13.3 §0.4.1 §0.6 · G29 G33a
  needs: P4.47, P3.53, P3.60, P4.82
  > The §2.13.1 app-level class, presented per the three §2.13.3 bullets (startup, mid-run escaped panic, WebView-backend disconnect; lines: the §2.13.5 catalog and the §5.8 run-path line) through the P3.60-delivered `AppFaultNotice` (state 12): this box wires entries and copy only; the `AppFaultNotice` supersede is P4.69's; no fabricated per-item outcomes (§5.2 row 12).
  > Built as its sub-boxes; each lands as its own commit where _format.md allows it.
  - [ ] **P4.50.1** [UI,RUST] Present the startup `WebviewFault` on a native surface and `ScratchUnavailable` through `AppFaultNotice` · §2.13.3 §7.2.1 · G29 G33a
    l-neg1: none
    > Two §2.13.3 startup classes: (a) the `WebviewFault` native leg of `present_startup_fault`, the §7.2.1 `frontend_ready` watchdog and native non-blocking dialog showing the §2.13.5 line (probe and fallback in §7.2.1); the `app://fault` emit + `PendingFault` body is P4.46's.
    > (b) `ScratchUnavailable`, raised by P4.82's §7.2.1 step-5 probe, presented through the same `AppFaultNotice` path as the other readiness kinds.
    > It also hoists each §2.13.5 line the code carries into a named const its fault constructor reads, bound to its §2.13.5 row like `every_catalog_row_is_bound_to_its_spec_table_row`.
    > Re-cut the P2.109 `None` arm and the `reveal_main_window` doc comment that calls it the detection seam (`[Test-Change]` on the P2.109 source-scan pin the re-cut moves). No L(-1) act: Rust-side `DialogExt` is not capability-gated (`main.json` grants no `dialog:`; the C2a/C2b pickers already call it).
    > Use `tauri-plugin-dialog`'s `MessageDialogBuilder::show(callback)`: `setup` runs on the main thread inside the event loop, where `blocking_show` freezes the app.
  - [ ] **P4.50.2** [RUST] Supervise the detached run task (escaped panic → token release, local log, no emit) + C16 `get_run_liveness` · §2.13.3 §0.4.1 §0.6 · G29
    > The §2.13.3 run-level escaped-panic core half (P4.47 is per-item only): the detached `start_conversion` run task (`ipc::conversion` `run_conversion_spawned`) supervises `run_conversion`; joining its handle or reusing P4.47's poll-level catch is a `[Build-Session-Entscheidung]`.
    > An escaped panic logs its payload locally (§7.5, P4.47's capture and redaction), releases the run's `RunRegistry` token (without it `converter_is_busy` stays true and wedges the §7.1.1 refuse-busy gate and the §7.3.2 close guard) and emits nothing: no wire `AppFault`, no `RunFinished`, no retained `RunResult` (§5.2 state 12).
    > The scratch the unwind leaves is the §2.6.3 sweep's; the boundary survives the P4.86 concurrent re-cut. Test: a planted run-body panic leaves `has_active_run()` false, reaches a capturing `log::Log` redacted, and sends no `RunFinished`.
    > C16 `get_run_liveness` per its §0.4.1 row and the §0.6 `RunLiveness` enum (no core heartbeat, §0.4.2): the golden entry (`src-tauri/ipc-commands.golden`, check 12), `xtask codegen` → `bindings.ts` with the `src/lib/ipc/commands.ts` wrapper, the `crate::ipc` `HANDLERS` row and `get_run_liveness` in the `lib.rs` command-surface name pins; plan-lint check 9 admits C16.
  - [ ] **P4.50.3** [UI] Wire the state-12 run-path entry — `onRunFault`, the C16-backed silence watchdog, the run-fault carrier, the copy · §5.2 §5.8 §2.13.3 · G33a
    needs: P4.50.2
    > The state-12 run-path entry P3.60 leaves unwired: supply `startConversionRun.onRunFault` (the P2.124 detection source fires into the optional-absent handler, a documented no-op until here) and the §5.8 channel-silence watchdog.
    > Re-cut the P3.53 `runFault { fault: AppFault }` typing for the DTO-less run fault (a nullable or class-split carrier is this fill's decision), never by synthesizing a wire `AppFault` in the frontend.
    > Render the §5.8 run-path line (§5.2 row 12); P4.69's supersede carries the verbatim-`message` model forward.
    > Watchdog per §5.2 row 12 (a) and the §5.8 silence bullet: on silence past a named interval it calls C16 (P4.50.2); `Running` keeps waiting, `Finished` re-fetches C8, and `Unknown`, a rejected call or a call that does not settle within its own named bound fires `onRunFault`. Every threshold is a named constant tagged `[Build-Session-Entscheidung: P4.50.3]`.
    > Tests: a quiet-but-alive run never reaches state 12; a panicking run task, a rejected C16 call and a C16 call that never settles (after its bound) each do; a `Finished` answer re-fetches C8.

## Generic bundle-time build assertions (§6.1.3)

- [ ] **P4.51** [BUILD] Build the generic §6.1.3 build-assertion framework hooked into `scripts/stage-engines` · §6.1.3 · G37 G38
  needs: P4.27
  l-neg1: sweep-tail
  > The §6.1.3 generic assertion harness `stage-engines` runs as it stages: the per-engine assertion-list slots (filled by P5–P7 per the P0.7.4 policy), the exposed-parameter capability-assertion framework (the engine option names ConvertIA exposes exist in the staged build) and the structural plumbing (parse staged binaries, fail the build on a miss).
  > The exposed-parameter legs themselves are per engine: the libvips `effort`/`Q` leg is P5.15, the FFmpeg `paletteuse` dither leg P6.94.
  > `stage-engines` is an uncaged Loop tool (P4.41), so the hook-in is Loop-buildable; its new `--selftest` legs are re-blessed at the phase-end sweep, and the framework's fail-closed arm gets a planted positive from outside the tool in the caged `g24-stage-engines.py` (a P4.81 caged tail).
  > (a) Ordering: the hook runs after `relocate_all` on both `stage_all` return paths, and `--selftest` pins both call sites (the P4.30 two-call-site rule); an earlier assertion checks bytes that never ship, or runs a binary that cannot load its beside-the-exe libraries.
  > (b) `--check` reads no staged byte and executes nothing, yet validates every byte-free contract (each declared assertion binds to a staged row, its argv can be built: `relocate_all`'s rule that a check pass never validates less than the run); the declaration binder also runs on `main`'s no-rows branch (the P4.31 binder precedent), live before the first P5 row.
  > (c) An executing assertion runs only on its triple's native runner (the universal x86_64 slice under Rosetta 2 on macos-14, the P4.29.1 probe); a foreign `--target` run performs every byte-free binding, lists each executing assertion as not-run and never reports it passed.
  > (d) SCOPE sentence, same commit: remove `(P4.51)` from the `stage-engines` docstring's "Each excluded piece is its own box" sentence; a P4.51 decision tag with the id still listed reds the SCOPE-freshness leg.
  > If P4.41 already removed `(P4.41)`, the list empties and reds both the freshness and the non-vacuity legs, so the second of P4.41/P4.51 to land retires the sentence and moves the three SCOPE-freshness legs to their terminal state under the same leg names (blessed in the caged `g24-stage-engines.legs`), with a `[Test-Change: <that box>]` rationale that passes G70.
- [ ] **P4.52** [BUILD] Build the MIT-core LGPL shared-object-or-fail link assertion (static LGPL into the MIT core = build FAILURE) · §6.1.3 §3.6.1 · G38
  l-neg1: sweep-tail
  > The §6.1.3 carve-out (i) assertion: no static LGPL link into the MIT core (§3.6.1), scoped to the Rust core code object. Carve-out (ii) is P4.76 (a different code object); leg (iii), FFmpeg-internal static LGPL, is aggregation that never fails and carries no assertion body.
  > Mechanism: assert the link kind of every native library the MIT core's binary links (G18's `deny.toml` LGPL deny is blind here: every native library enters through an MIT `-sys` crate).
  > Data source: the `cargo:rustc-link-lib=[KIND=]NAME` build-script directives of the packages in `convertia-core`'s normal target closure (`cargo metadata --locked`); host build-dependency outputs are excluded (vswhom-sys `static=vswhom` feeds a build script).
  > The `links` keys are no source: glib/gobject/gio/pango/cairo/gdk-pixbuf/soup3/javascriptcore-sys carry none. Rule: any `static=` (or `static:+whole-archive=`) native library not on a committed fail-closed permissive allow-list FAILS the build; a dynamic link passes whether bundled or OS-provided (§6.1.3 (i)), so no by-name GTK exclusion exists.
  > The allow-list is `scripts/core-static-links.toml` (caged by `scripts/**`), seeded by the Co-Pilot from a three-OS measurement in the same P4.81 act as the compile-sanity invocation.
  > Hook site: a post-core-build leg (compile-sanity after `pnpm tauri build --debug --no-bundle`; the Lane-B release job after `tauri build`), not a `stage-engines` leg and no `needs: P4.51` (the core does not exist at staging time; `stage-engines --check` runs in a toolchain-less job).
  > Split: the Loop builds the checker as a `cargo xtask` subcommand (every `scripts/` path is caged), unit-tested against synthetic build-script `output` and metadata fixtures. Caged tails for the P4.81 sweep: the compile-sanity CI invocation (`.github/**`) and the checker's caged self-test file with a planted positive from outside it.
  > The Lane-B release-job invocation is P10.9's (`needs: P4.52`): `release.yml` is the P0.2.5 fail-closed skeleton, so a step added now never runs. G37b scans staged engines only, so a binary-import cross-check of the core would be a new named leg, never a G37b ride-along.
- [ ] **P4.53** [BUILD] Build the libvips-no-copyleft-PDF-loader assertion (no poppler/mupdf/GPL/AGPL loader present, no svgload) · §6.1.3 §3.1 §3.6.1 · G38
  needs: P4.34, P4.51
  l-neg1: sweep-tail
  > The §6.1.3 no-copyleft-PDF-loader `[DECIDED]` positive assertion (rationale §3.1/§3.6.1): fail the build if a `pdfload`/`poppler`/`mupdf` foreign loader or `svgload` is registered in the staged libvips (SVG is the rsvg crate, §3.5.5).
  > It is the P4 proof-of-life check on the P4.34 imgworker's static libvips; P5.2 is the distinct stage-time check on the newly staged P5.1 libvips (same property, different artifact and stage).
  > Mechanism (a symbol assertion, which §6.1.3's "loader/symbols" allows; verified against libvips v8.18.6, re-verified at the P4.89 pin): the pinned rustup `llvm-tools-preview` `llvm-nm` over the static libvips archive in its cache entry (the P4.89 closure), on all three OS.
  > Any symbol matching `vips_foreign_load_pdf` (the poppler and pdfium loader classes, compiled only under `HAVE_POPPLER`/`HAVE_PDFIUM`), `poppler_` or `FPDF_` FAILS.
  > `pdfload` is never a needle: libvips always defines `vips_pdfload`/`vips_pdfload_buffer`/`vips_pdfload_source`, even with no PDF loader, and those three wrappers prove `llvm-nm` read libvips' foreign code; absent, the leg FAILS, never a silent pass.
  > The same scan runs over the staged `convertia-imgworker-<triple>` where it has a symbol table (ELF/Mach-O), with the same proof; on a symbol-less binary (a Windows PE keeps its symbols in the PDB) that leg is skip-with-reason.
  > Configure cross-check: the `libvips.configure.flags` line the P4.89 act writes into `engine-configure.toml` must carry `-Dpoppler=disabled`, `-Dpdfium=disabled` and `-Dmodules=disabled` (a module build moves the poppler loader out of the scanned archive). libvips has no MuPDF loader; the `mupdf` word stays because §6.1.3 uses it.
  > The `svgload` leg takes the same scan and cross-check; its class needle, proof symbols and configure switch are measured at the P4.89 pin.
  > Sweep tail (the P4.81 list): the planted positive from outside the tool in `g24-stage-engines.py` for this box's assertion arm (the P4.41 `stage-engines` condition).
- [x] **P4.54** [BUILD] Build the libimagequant BSD-2-leg COPYRIGHT-text assertion + the `engines.lock`/Cargo.lock fork-pin provenance check · §6.1.3 §3.1 · G38
  > RECONCILE: re-homed to P5.4 — the libimagequant fork is acquired with the P5 image stack.
- [x] **P4.55** [BUILD] Build the libheif-resolves-dav1d-for-AV1-decode assertion (dav1d, not libaom, as the AV1 decoder plugin) · §6.1.3 §3.1 · G38
  > RECONCILE: re-homed to P5.12 — libheif/dav1d are acquired with the P5 image stack.

## SBOM + NOTICE / third-party-licenses scaffold (§3.7 / §6.3)

- [ ] **P4.57** [BUILD] Build the THIRD-PARTY-LICENSES.txt / NOTICE generation scaffold (full licence text + corresponding-source pointer per component) · §3.7.1 §3.7.2 §6.3.2 · G36 G35
  needs: P4.56.1, P4.56.2
  l-neg1: none
  > The §3.7.1/§3.7.2/§6.3.2 generation scaffold: `THIRD-PARTY-LICENSES.txt` per the §3.7.1 row (its "corresponding source: <url>@<ref>" literal) and §3.7.2 steps 2–3, the repo `NOTICE` per the §3.7.1 NOTICE row with §6.3.2, and the G35 generated-vs-committed NOTICE and corresponding-source-pointer parity hook. Scaffold; rows land in P5–P7.
  > The edge names P4.56.1 and P4.56.2 (the schema and the SBOM it reads), never the P4.56.3 origin gate. The committed placeholder pair (651d4bf) exists: this box builds its producer, the §3.7.2 step-2 `cargo xtask sbom` build step (P4.56.2), not a second tool, and regenerates both in its own commit so the zero-row output and the committed pair agree.
  > `THIRD-PARTY-LICENSES.txt` is compile-time-embedded (P2.98) and a bundle resource (P4.31); the generator reads only `engines.lock` and the SBOM inputs, so it can run before the release compile (the ordering fix is P10.18's). It overwrites the repo-root pair in place.
  > Licence texts: the §3.7.2 item 2 vendored tree `third-party-licenses/` (its §0.7 row exists, so no owner act). A new dependency for the SPDX cross-check adds its floor row to `Cargo.toml` `[workspace.metadata.convertia.pinned-floors]` in the dep-add commit (§0.8).
  > The tree stays uncaged: the `engines.lock` rows are caged, the §6.1.3 licence-text assertion reads the pinned source's own text (P5.4's COPYRIGHT leg, at the location its acquisition records), and the vendored tree only feeds the generated `THIRD-PARTY-LICENSES.txt`.
- [ ] **P4.58** [BUILD] Wire the §3.7.3 manifest-driven completeness gate scaffold (every externalBin + resources engine file has a manifest row) · §3.7.3 §6.3.3 · G36 G35
  needs: P4.56.1, P4.56.2, P4.41
  l-neg1: sweep-tail
  > The §3.7.3/§6.3.3 items 1 + 3 release-blocking completeness gate scaffold: the G35 row's Syft + stage-tree file-manifest-diff leg (T3a; an attribution check, never a post-staging byte re-verify) and its SPDX-expression validation leg (the §6.3.3 `LicenseRef` carve-out).
  > The NOTICE and source-pointer parity legs are P4.57's; the whole-bundle runs are P10.15 / P10.19 / P10.20; it activates per engine as P5–P7 rows land.
  > The edge names the two sub-boxes it reads, never the P4.56.3 origin gate.
  > Wire target: a `cargo xtask` leg (never a `scripts/check-*` name: that glob is L(-1)), exercised in P4 only by its own `xtask` tests under Lane A's `cargo llvm-cov --workspace` step, over scratch stage trees and fixture `engines.lock`/SBOM inputs; no `.github/**` wiring in P4 (`release.yml` is the P0.2.5 fail-closed skeleton; its §6.7.2 stage-3 slot is P10.4's).
  > Tests: a clean mapped tree passes; a planted unmapped `.so`/`.dll`/`.dylib` fails (item 1); a planted `UNKNOWN`/`NOASSERTION` licence fails (item 3); a planted `LicenseRef-AOMPL-1.0` passes with its text in `THIRD-PARTY-LICENSES.txt` and fails without it (the item-3 carve-out).
  > Scan scope (the §6.1.3 bundle-binder boundary, never a second classifier): every `bundle.externalBin` entry except the first-party `convertia-imgworker` (no `engines.lock` row; its static libraries are G35a sub-component rows), plus every file under a `resources/<subdir>/` tree; map entries outside `resources/` are §6.1.3's non-engine resources.
  > Apply the path-shape rule `stage-engines` implements (P4.31), with a bind leg that both classify the committed `tauri.conf.json` identically.
  > Inside an engine tree each file in G35's per-file classes needs its own row (§3.7.2 items 3–4); every other file maps to its tree's component (§6.3.3 item 1).
  > Row binding: through the `stage-engines` placement record (`STAGED_ENGINES` dest → `cache_engine`, the P4.41 manifest `engine` column), never by `sha256` equality: relocation rewrites bytes, and the row hashes the bytes entering staging, which P4.98 verifies.
  > SPDX parser: the `spdx` crate, a Loop dep-add in `xtask/Cargo.toml` with default features, its §0.8 row and floor row with the dep-add, reused by P5.68 / P10.20 (`cargo-about` is not chosen, so no pin box exists); the leg reads `engines.lock` through the `gate_api` façade like P4.56.2.
  > §6.3.3 item 4 (no MIT taint) is the live G18/G53 gates for the Rust graph and G36's SBOM forbidden-family hard-fail for the engines, run at release by P10.19.
  > Pin tail (the P4.81 list): the Syft row in `scripts/gate-tools.toml` (its header assigns the Syft pin to this box), batched with the P4.56.2 pins; the tests run over fixture Syft output, so the pin reds nothing. Syft v1.51.1 ships a checksums file with a cosign signature and per-OS archives for darwin, linux and windows, so the row is installable and corroborable.

## Reliability harness (§6.4.3 / §6.4.3a / §6.5)

- [ ] **P4.59** [TEST] Build the §6.4.3 per-pair integration runner (real engines, per-format structural reader, fidelity + lossy + patent-gap assertions) · §6.4.3 §6.5 §6.5.2 · G31 G32
  needs: P4.7, P0.5.6, P3.62, P3.63
  > The §6.4.3 per-pair runner as written there (the structural-reader, fidelity, lossy and patent-gap legs, magic re-detect only a pre-screen, the §0.9 LibreOffice serialization); per-pair fixtures land in P5–P7.
  > It expands, never rebuilds, the P3.63 walking-skeleton runner (`orchestrator::run_conversion_e2e_tests` and its `run_conversion_tests` `pub(super)` conductor harness, the G23 `start_conversion` partner suite).
  > The drive generalises to manifest-enumerated `(source→target)` pairs with per-category reader dispatch; CSV↔TSV is its first live pair (the P3.62 `covers` row); the P3.62/P3.63 assertions stand (no green-by-rewrite, test-strategy §8), and the G23-keyed `*_tests.rs` partner file keeps referencing `start_conversion`.
  > It authors the serde manifest types in a crate-root module compiled under `#[cfg(any(test, feature = "gate-api"))]`, which the P4.60 `gate_api` façade re-exports. It emits the §6.5.2 *Runner record* P4.61 folds; the cross-job hand-off is P10.61's Lane-B stage 2, so no caged tail.
  > Readers are test-side tools, never shipped: `ffprobe` and `pdftotext` come from the staged P5–P7 sidecars (§3.3), CSV/TSV reads through the in-workspace `csv` crate, and P4.89 (e) acquires the image reader and names the box for each other non-bundled reader; the fill never picks a route.
  > A category's reader runs only for a listed pair of it; a missing reader tool for a listed pair is a hard FAIL, never a skip (§6.4.3, test-strategy §0.2).
  > Reader conventions (P0.5.6): the §6.4.3 dispatch plus test-strategy §0.2 sub-assertions 1–3. The cross-library decode ("Break the circular decode") is a per-pair hook not run in P4 (no FFmpeg staged); its assertions land in P5.51, P5.56, P5.57 and P6.43.
  > No OCR arm and no `tesseract` pin: no §04 matrix offers a document- or presentation-to-image pair (§04/documents.md#out-of-scope-parked-honestly-surfaced, §04/presentations.md#decisions-parked-resolved), so test-strategy §0.2 item 4 has no v1 subject; a future such pair re-opens it.
- [ ] **P4.60** [TEST] Build the §6.4.3a corpus↔pair bijection guard's cage-free half — shared parsers, required-pair enumeration, fixtures (the caged bin is P4.95) · §6.4.3a · G22 G24
  needs: P4.59, P0.4.11, P4.56.1, P4.92
  l-neg1: none
  > Builds §6.4.3a as authored (steps 1–3, both directions, the one root manifest of §6.4.5 *Manifest layout + discovery*) plus the two §6.4.5 legs it carries: the `[[file]].path`-exists assertion and the *Two pair encodings* `[file.expect]`↔`covers` step-check. Engine-free at run time: its `needs: P4.59` is for the serde manifest types; P4.61 and P4.79 carry the runner edge.
  > The `harness_fixture` exemption is §6.4.3a's; the SSOT ⊇ §04 anchor is P4.60.1.
  > G24 self-test legs (P4.95 lands them): a required §04 pair with zero backing corpus files fails; a `covers` naming a non-existent §04 pair fails; an absent `[[file]].path` fails; a `[file.expect]` key with no matching `covers` 2-tuple fails; a `harness_fixture = true` diagonal entry passes the exemption leg, and one naming a non-diagonal non-existent pair fails; the clean tree passes.
  > Posture per §6.4.3a *Phase-in*: direction 1 fail-closes per category at images P5.48/P5.65, audio/video/cross-category at their P6 corpus and ledger boxes, office at their P7 ones, all categories on the RC at P11.15. Direction 2 and the step-check are live from P4: the registered CSV↔TSV couplings already satisfy both.
  > The posture and each flip are the G22 row in gate-status.md: the P4 posture's gate-status and gate-planes touches land with the P4.95 wiring, each later flip with its phase's sweep box.
  > Realization: the spec's `cargo run` literal is an xtask `[[bin]]` (`name = "check-corpus-coverage"`, `path = "../scripts/check-corpus-coverage.rs"`, run via `cargo run -p xtask --bin check-corpus-coverage`; cargo permits the out-of-package path, and xtask is `publish = false`), which keeps the source at the spec-pinned `scripts/` path the caged G22 row and `scripts/check-completeness` name.
  > Re-homing the source into `xtask/src/` would stale those literals, so it is no fill choice: escalate first if the `[[bin]]`-path form proves unworkable.
  > The .1/.2/.4/.5 guards are xtask task subcommands in flat module files directly under `xtask/src/`, dispatched from the `xtask/src/main.rs` task switch beside `codegen`; the parsers they share live in a flat `xtask/src/lib.rs` library target (a module declared from `main.rs` is private to that bin).
  > Never `xtask/src/bin/` or any nested directory (a new tracked directory fails G69); only this bin keeps a `[[bin]]` target.
  > Core access: the read-only `gate_api` façade behind the non-default `gate-api` feature (§0.7 `lib.rs` row, so no owner act); xtask depends on `convertia-core = { path = "../src-tauri", features = ["gate-api"] }` plus its own `toml`.
  > The façade exports read-only types and pure lookups (the P4.92 registry-level offer before health marking, the `UserFacingFormat` roster), never a `toml` parse; the serde manifest types (P4.59, re-exported) are the one manifest reader.
  > The door serves P4.60.1/.2/.4/.5, P4.56.2 and P4.58; P4.56.2, first in build order, builds the façade, the feature and the xtask dependency, and later readers extend its exports.
  > Matrix reading per the §04 README *Matrix cell grammar*: a category matrix is the first pipe table under `## Source → target matrix` of the six category files; a label drops `**` and any footnote mark and resolves through one label table shared with the `covers` reader (e.g. `MPG/MPEG`→`Mpeg`, `3GP`→`ThreeGp`); an unresolvable label or a cell outside the grammar FAILS.
  > A diagonal cell is excluded whatever it holds (the video matrix is not square); cross-category pairs encode per §6.4.5 *Two pair encodings*; the all-platform-`unavailable` exclusion reads §3.4.4a `EngineRow::available` (no rows in P4, so the set is empty).
  > Caged half (G71): the P4.95 owner act, which P4.80 needs, lands `scripts/check-corpus-coverage.rs`, every sub-box's G24 self-test under `scripts/gate-selftests/`, each gate's Lane-A `ci.yml` wiring, and the xtask `[[bin]]` entry (cargo rejects a `[[bin]]` whose `path` is absent).
  > The Loop builds everything cage-free (the parsers, the guard modules, `xtask/src/lib.rs`, `tests/` fixtures); P4.60 and its sub-boxes flip `[x]` on their Loop halves.
  > The caged bin reaches the parsers as `pub` items of the xtask library and keeps the bijection logic (the union of `covers`, both directions, the `harness_fixture` exception).
  > The required-pair enumeration (cell classification plus the diagonal, `out` and unavailable exclusions) is a `pub` item of `xtask/src/lib.rs`, called by that bin and the P4.61 generator; the caged G24 self-test drives it over planted fixtures from outside, landing with P4.95.
  - [ ] **P4.60.1** [GATE] Build the SSOT §5 ⊇ §04/`UserFacingFormat` coverage anchor — every SSOT-named format ∈ the §04 matrices ∧ the `UserFacingFormat` enum, else FAIL · §0.6 · G22 G24
    l-neg1: none
    > The SSOT §5 ⊇ §04 anchor that makes the completeness chain non-circular at its top: P4.60 enumerates from §04, so a format dropped from §04 (e.g. cut to dodge a hard engine) is invisible to it and the RC ships missing a promised conversion.
    > A Lane-A guard parses the SSOT §5 *What It Converts* per-category format lists (the six categories plus the two cross-category outputs) and FAILS if an SSOT-named format is absent from the §04 category matrices or the `UserFacingFormat` enum (§0.6).
    > SSOT *v1 Definition of Done* makes it load-bearing: the per-platform patent gap and last-resort demotion are the only sanctioned omissions and surface as ledger cells, never as an absent §04 row. It is the upstream half of the SSOT→§04→corpus→ledger chain (downstream: P4.60, then P4.61/P11.15).
    > G24 self-test legs (P4.95 lands them): deleting a format from §04 and the enum fails; from §04 alone fails; an unnormalized `MPG/MPEG` fixture reports no false absence; a dead alias row fails; the clean tree passes.
    > Matching rule: SSOT tokens come from each `- **<Category>** —` bullet with the emphasis, the `(¹)` marker and every parenthetical dropped, split on commas; the `; plus` tail gives `SVG` (source-only) and the two operations. A format under two categories (`PDF`) counts once for the enum and must be present in each category matrix that lists it.
    > A slash token (`JPG/JPEG`, `HEIC/HEIF`, `MPG/MPEG`) is one format matching either half case-insensitively; `3GP ↔ ThreeGp` is a declared alias row, and an alias row matching nothing FAILS. After normalization the clean tree gives 46 = 46.
    > On the §04 side a format is present as a matrix row label or a column header (the §04 README grammar; headers alone miss the source-only formats). The two operations are checked only against the cross-category.md operation columns, never the enum. Core access is the P4.60 `gate_api` door; the task name is a `[Build-Session-Entscheidung: P4.60.1]`.
  - [ ] **P4.60.2** [GATE] Build the §1.6 defaults-registry "no required choices" guard — the merged `OptionDecl.default` index over all §04-offered pairs, FAIL on a gap · §1.6 §6.7.1 · G61 G24
    l-neg1: none
    > Builds the §1.6 *Defaults registry & the DoD gate* mechanism (§6.7.1 step 4a, §6.10 row 7), the machine-checkable home of the SSOT *v1 DoD* "no required choices" gate: a Lane-A xtask guard reads every §04 matrix and the P4.92 option registry and generates the merged `OptionDecl.default` index over every offered pair (the values stay §04-owned), re-run as P5–P7 register declarations.
    > `OptionDecl.default` is mandatory, so a missing Rust default is a compile error; the guard FAILS if (i) an engine-offered §04 pair has no explicit registry entry, or (ii) a registered default is invalid for its `OptionKind` (outside the `IntRange`/`Size` bounds, an `Enum` value outside its choices, a `Color` not `#RRGGBB(AA)`). Live from P4 over the CSV↔TSV entries.
    > G24 self-test legs (P4.95 lands them): an offered pair with no entry fails; an out-of-range `IntRange` default fails; an `Enum` default outside its choices fails; the complete registry passes; a data-file registry at P4.92 adds a deleted-`default` fail leg.
    > The audio/video/spreadsheet default tables (P6.28/P6.54/P7.60) register against this guard; the image option defaults (P5.37–P5.46, P5.75) are covered by it reading the images matrix and the registry, with no ad-hoc P5 assertion. The per-source default target (one `★` per source, §1.5) is the separate P5.77 gate, keyed on C3 `get_targets`. Verified green on the RC by P11.26.
    > Engine-free, on P4.60's parsers and `gate_api` door (its P4.59 edge is the parent's: the serde manifest types).
  - [x] **P4.60.3** [GATE] Build the `§04/<file>#<slug>` coverage-anchor reference-resolution leg of plan-lint (the format-coverage track gets the same resolvable-anchor guarantee tracks A/B/D have) · G7 G20 G24
    > Delivered: 40af538
  - [ ] **P4.60.4** [GATE] Build the §04 per-format-ENTRY template + category-file-shape completeness gate (a present-but-malformed entry FAILS, one level below P4.60.1's absent-row case) · §0.6 · G22 G24
    l-neg1: none
    > The gate one level below P4.60.1's absent-row case: a present-but-malformed §04 entry (PNG missing its lossy field, WEBP its options default) passes every other gate while the per-pair lossy disclosure or option default is silently absent.
    > A Lane-A xtask guard (the §0.6/§04 parser) checks each `04-formats/*.md` against the §04 README *Per-format entry template* and *Category file shape* and FAILS on a missing required field.
    > G24 self-test legs (P4.95 lands them): deleting the lossy field or the Options/settings field from one entry fails; a stub's sibling-file link to a file with no same-token entry fails; a malformed h3 under `## Shared option sets` passes (out of scope); the complete tree passes.
    > Posture: fail-closed on landing; the six category files carry a full `## Per-format entries` section (measured: 46 template-complete entries plus 2 stubs, so no §04 sweep is needed); it re-runs whenever a §04 file changes.
    > Entry rule: an entry is a `### ` heading inside `## Per-format entries`, up to the next `## `; an h3 under another h2 is no entry, and no matrix-header roster is used (it lists target columns only and skips source-only entries). The token is the heading's first word with backticks stripped, split on `/`; the bare, backticked, `— <name>` and `(<ext>)` forms are accepted.
    > Cross-reference stub: an entry whose heading links to a sibling `04-formats/*.md` file is exempt from the field check and FAILS unless the linked file has a non-stub entry with the same token.
    > Field rule: every non-stub entry has a top-level `- **<label>**` bullet per template field, matched by label prefix; the Options field must be present, its default values stay with P4.60.2 (G61), since entries may declare `none surfaced` or a shared option set.
    > Shape rule: each category file has the README *Category file shape* (non-empty intro prose, then its h2 sections in that order); extra h2s are allowed; `cross-category.md` departs from the template (the README says so), so the gate applies its operations-entry shape. Engine-free; it shares P4.60's parser (its P4.59 edge is the parent's); core access is the P4.60 `gate_api` door.
  - [ ] **P4.60.5** [GATE] Build the detection-KAT completeness gate — every §04 format has ≥1 `tests/detect-kat.toml` entry pinning a real fixture to its `FormatId`, else FAIL · §1.2 §0.6 · G15 G24
    l-neg1: none
    > The per-format completeness peer of the G15 detection-KAT convention (P0.5.7), which pins one entry per ambiguous case, not per format: a newly wired detector (the P6.15 M4A-AAC↔ALAC split, a §04 video format) could ship with no KAT pin, and a mis-wired container split (a DOCX classified as bare ZIP) is caught neither by P4.59 (a correctly detected file) nor by P4.60 (enumeration only).
    > A Lane-A xtask guard (the §0.6/§04 parser plus a `tests/detect-kat.toml` reader) FAILS if a §04 format has no `detect-kat.toml` entry pinning a real, manifest-tracked fixture to its exact `FormatId` (the per-format ≥ 1 floor the P5.47/P6.15/P6.46/P7.30/P7.31 KAT-entry notes register against).
    > G24 self-test legs (P4.95 lands them): deleting the only entry for one format fails; a complete KAT passes; one fail leg per arm (b), (c) and (d) below.
    > Posture: no wired-signature accessor exists, so the gate carries a declared map `KAT_PENDING` (variant → the §1.2 signature box that wires it): image variants → P5.47, audio → P6.15, video → P6.46, `Docx`/`Xlsx`/`Pptx`/`Odt`/`Ods`/`Odp` → P7.30, `Doc`/`Xls`/`Ppt` → P7.31.1, `Rtf`/`Html`/`Pdf`/`Md`/`Txt` → P7.31.2.
    > It starts with the 44 variants that have no KAT pin, so `Csv`/`Tsv` are fail-closed from P4.
    > It FAILS if (a) a variant outside `KAT_PENDING` has no `[[case]]` whose `expect` names it on a manifest-tracked `file`; (b) a `KAT_PENDING` variant already has one (the pinning box removes its variants in the same commit); (c) a `KAT_PENDING` variant's owning box is `[x]`; (d) a key is not a variant or its box id does not resolve.
    > Pending variants are reported on every run; once every signature box is `[x]`, arm (c) forces the map empty (the §6.4.1 steady state).
    > The posture and the map are the gate's gate-status.md row, landed by P4.95 with the wiring, beside the P4.60 G22 row. Engine-free; it shares P4.60's parser (its P4.59 edge is the parent's); core access is the P4.60 `gate_api` door.
- [ ] **P4.61** [TEST] Build the §6.5.2 pair-status ledger generator (`reliability-report.json` + human table, the release-gate cell set) · §6.5 §6.5.2 · G31 G32 G72
  needs: P4.59, P4.60
  l-neg1: sweep-tail
  > The §6.5.2 pair-status ledger as authored there (the `(source, target, platform)` key, the closed cell set, the release predicate, the informational `harness_fixture` row) over the §6.5.1 definition of `reliable`, published as a release asset; the generator is built here, and pairs are marked reliable category by category in P5–P7.
  > Realization (the P4.60 sibling pattern): the xtask task `reliability-ledger` (`cargo run -p xtask -- reliability-ledger`) in a flat module under `xtask/src/`, folding the §6.5.2 *Runner record* P4.59 emits into `reliability-report.json` and the human table; the release-gate cell set calls P4.60's `xtask/src/lib.rs` required-pair enumeration, never a re-implementation.
  > The three-leg Lane-B run and merge are P10.61's; publication is P10.58's.
  > `demoted` cells follow §6.5.3 *`demoted` source*; this makes the `demoted` halves of P10.42 and P11.16 true by construction, needs no new file and no §0.7 row.
  > It also wires G72 (the build-gates row reads "Wired in P4.61"; no G72 enforcement exists): a `scripts/check-*` trailer gate over the push range (the G71 `check-l-neg1-ack` pattern) asserting every commit that bumps an engine per §6.5.4 *What counts as a bump* carries a `Reliability-Gate: <ledger-ref>` trailer.
  > The trailer is a shape-only attestation (the G71 pattern): no doc defines `<ledger-ref>`, and single-branch `main` has no three-platform proof run before a bump lands, so the green three-platform check stays in the release pipeline. The fill records the trailer regex here.
  > G72 mechanics: per commit it parses `src-tauri/engines.lock` in the parent and in the commit (an unparseable side fails closed) and compares `(id, triple)` keys, never `version`/`sha256` lines; range resolution reuses `check-l-neg1-ack`'s `resolve_base` / `commits_in_range`.
  > G24 self-test legs: a post-P4 version-row edit without the trailer fails, with it passes; a non-`engines.lock` diff passes; a pure new-row addition without the trailer passes; without the trailer, a `triples` split re-pinning an existing `(id, triple)` into a new row at another version fails, and so does removing an existing row or renaming its `id`.
  > Owner tail: one owner-acked act on the P4.81 caged-tail list; it reds nothing until it lands, so it is neither a hard stop nor an `[!extern]` precondition.
  > The act carries the gate and its self-test, the lefthook L2 / ci.yml L4 wiring, the G72 row's Delivered update with its trigger sentence and self-test list reconciled to §6.5.4, vuln-response §2 step 5, and roles-and-escalation §5a (its hold window and its "regenerated-green" wording).
  > It lands fail-closed directly: the P1–P3 window was the gate's absence under the roles §5a hold, so no gate-planes `[[fail_open]]` row is removed, and a `[[fail_open]]`/`[[posture_flag]]` row joins `gate-planes.toml` only if the script ships a bootstrap leg. The Loop builds the ledger generator green and flips this box `[x]` on that half.

## Binary-size-budget levers (§3.9)

- [x] **P4.62** [BUILD] Build the early per-component size-baseline measurement (compressed, per platform) · §3.9 §3.9.1 §3.9.2
  > RECONCILE: retired — no § mandates an early size baseline; P10.36 (G41) measures the shipped artifact.
- [x] **P4.63** [BUILD] Build the size-budget trim levers (LibreOffice strip help/l10n/dictionaries, CJK font subset) + the fixed lever-order · §3.9.1 §3.9.2 §3.9.3
  > RECONCILE: the lever mechanism moves to P7.1 (strip-list) and P7.2 (CJK subset), where each lever first fires.

## Generic UX-correctness primitives (§05 / §2.8 / §2.9)

> **P3↔P4 UI-seam model (DECIDED — same statement as P3's UI header):** P3 built
> intentionally-minimal, slice-only renderers (DropZone P3.54, FormatPicker P3.56 — its
> DestinationBar sibling is the carve-out below — ProgressList+Cancel P3.58,
> ResultSummary+OpenActions P3.59, fault screens P3.60);
> these P4 boxes **SUPERSEDE** (rebuild) them into the generic, `OptionDecl`-declaration-
> driven, fully-a11y components P5–P7 register against — P4 does **not** extend the P3
> renderers in place (the P3 versions are throwaway slice scaffolding). Each P4 UI box
> names the P3 box it supersedes + carries the `needs: P3.5x` edge, so the loop builds the
> P3 slice-renderer first (the live UI until P4 lands) and the supersede is explicit, never
> a silent double-build.
> **Carve-out (the P3.56 item-1/item-2 ruling; the owner statement sits in
> the P3 UI header):** the **DestinationBar** has NO rebuild box and none is warranted
> (nothing about it is option-declaration-generic) — the P3.56 build is the durable
> component; **P4.69 EXTENDS** it with the up-front-fail `Note` leg, **P4.70.3**
> a11y-wires it.
> **The P3.60 fault screens:** P4.69 carries them forward unchanged.

- [ ] **P4.64** [UI] Build the OptionsPanel widget-dispatch that renders declared `OptionDecl` widgets generically (Basic tier) · §1.6 §5.3 · G33a
  needs: P2.8, P1.27, P3.56, P4.92
  > Scope: the §1.6/§5.3 generic widget dispatch, built once (P5–P7 register only per-format `OptionDecl` declarations, no new chrome): each backend-supplied `OptionDecl` renders by its `OptionKind` into the declared widget, the Basic tier shown directly (§1.6). The Advanced drawer is P4.74's.
  > It renders the P2.8-authored §0.6 option types from `bindings.ts`, never re-homes them, and mounts into the P3.56 `TargetsScreen`.
  > Collect half (§1.6, §5.2 row 4, §5.8 call-timing): every widget edit dispatches a state-4 self-loop Msg that writes the override into the held `Planned.options` (`src/state/machine.ts`) and re-fires C4 through the P3.56 re-plan seam (`replanOutput`, `src/lib/ipc/events.ts`).
  > The ~150 ms debounce and the drop of a superseded in-flight C4 result go on that shared seam (the delivered seam has neither), so a target change debounces like an option change. C5 and C6 already carry `Planned.options`; the same dispatch serves the Advanced widgets P4.74 reveals.
  - [ ] **P4.64.1** [GATE] Assert every registered `OptionDecl.label` and `EnumChoice` label resolves to a non-empty `strings/ui.ts` entry — the option-label single-source gate · §1.6 · G57 G15
    l-neg1: none
    > The option-label peer of the keymap (P8.18.1), ErrorKind (P3.68) and LossyKind (P3.69) completeness gates: G57(b) asserts every `strings/ui.ts` key non-empty and G57(c) bans inline literals, but neither asserts that a registered label points at a real key, so an option could ship a blank or key-name control.
    > A core-crate Rust `#[test]` (the P3.68/P3.69 catalog-gate form, run by `cargo test` under G15) enumerates the registered set: the union, over every `UserFacingFormat`, of the P4.92 per-source offer's `targets[].options`, plus each `OptionKind::Enum` `EnumChoice.label`. That offer is the one production source, never a registry internal or a hand-kept list.
    > It `include_str!`s `src/strings/ui.ts` and fails if any label does not resolve to a non-empty entry. In-suite cases over a planted fixture set: a label at a missing key and an `EnumChoice.label` at an empty value each report; a fully resolved set passes.
    > No `scripts/gate-selftests/` leg and no L(-1) act: plan-lint check 16 covers only the `scripts/check-*`/`run-*` gates, never a `#[test]`.
    > It re-runs as each P5–P7 declaration box registers options: fail-open while that union is empty, fail-closed from the first declaration (the P4.60.2 pattern).
- [ ] **P4.65** [UI] Build the lossy/fidelity-note surfacing in FormatPicker (passive inline `Note` keyed by `LossyKind`, incl. the video worst-case note) · §2.9 §5.7 §5.3 §5.8 · G57 G33a
  needs: P2.20, P1.27, P3.56, P3.69, P3.58, P4.92
  > Scope: the §2.9/§5.7 lossy-note surfacing (§5.7's *Predictable lossy* and *Worst-case lossy* rows, the §2.9.2 layers and render order) on the §5.3 `Note` primitive this box builds, with the P3.69 §2.9.1 catalog as the verbatim string source. It supersedes the P3.56 slice FormatPicker.
  > Carrier: add the §0.6 `OutputPlanPreview.lossy_notes` with the `bindings.ts` regen, computed core-side from the C4 `options`. `Target.lossy` stays the untouched §1.5 single offer marker, and the frontend never mirrors the `crate::outcome` catalog (§2.9).
  > Same commit (DoD 2): the `LossyKind`/`Target` `///` docs that call the set "a SEPARATE render-time computation" are re-pointed to `lossy_notes` (`xtask codegen`).
  > The set rides C4, never the C3 `Target`: C3 is option-blind (the WEBP `lossless` toggle removes `image_lossy_codec`), and `Target` is a tier-3 pure `crate::domain` type that may name no `crate::outcome` type (§0.7) and derives `Deserialize`, which the outbound-only `OutcomeMsg` cannot satisfy.
  > Inputs (§2.9.2 layers): (i) the pair-static kind set from the P4.92 per-pair registry data (one pair can carry two kinds); (ii) `AnimatedSource` with a still target for `image_animation_flatten`, alpha being pair-static (images.md *Lossy kinds*), so no alpha peek and no new `CollectedNoteKind`; (iii) the per-item-runtime kinds at their §2.9.1 before-convert rows.
  > This box hoists the before-convert lines into `crate::outcome` as templates bound per row in `every_catalog_row_is_bound_to_its_spec_table_row`; every other kind wraps the delivered `lossy_note` once.
  > ConvertingNote (state 7, its §5.3 row) wraps `Note`, reads the store's `pendingVideoReencodeNote` and mounts in the P3.58 `ConvertingScreen`; the last of this box, P4.66 and P4.87 to land reconciles that mount site.
  > §5.8 lifecycle: at state 4 the field takes the `video_reencode` text from this box's note set, so the state-4 note and the state-7 banner share one value. The `RunStarted.willReencode` keep/clear arm is delivered (P2.120, `reduceConvertEvent`); the §5.8 reset fires on the 4→3 Back edge and on any C3 re-run for a different target.
  > Tests: set, carry and reset in the store reducer; the banner shown and hidden under jsdom with the G33a `vitest-axe` assertion. Same commit (DoD 2): the `ConvertingScreen.tsx` slice-scope comment and the `store.ts` `runStarted` comment that hand the banner to P4.65.
- [ ] **P4.66** [UI] Build the ProgressList + aggregate-bar progress surface (real determinate per-item `ItemProgress`, staged-coarse fallback, terminal rows) · §1.11 §5.3 §5.1 · G33a
  needs: P4.8, P2.37, P1.27, P3.58, P1.31.2, P4.101
  > Scope: the §1.11/§5.3 ProgressList over the §0.4.2 `ItemProgress` payloads keyed by `itemId`, with the aggregate bar, the terminal rows and the staged bar of a `null`-fraction row (its §5.3 row). It supersedes the P3.58 slice Converting/ProgressList.
  > It reads the §5.1 store's live-progress map (P1.31.2) with per-row selector granularity (§5.8) and windows the rows over the P3.55 `useVirtualWindow`; windowing composes with the selector granularity, and a fixed-row-height insufficiency escalates per the hook's rule.
  > Same commit: rewrite the slice comments in `src/components/ProgressList.tsx` and `src/components/ConvertingScreen.tsx` that home virtualisation, per-row selectors and the staged bar on P4.70.3; only the per-element ARIA is P4.70.3's.
  > Store extension (over P1.31.2/P3.58, no new seam; today `ItemRow` has no `stage` and the `itemProgress` arm keeps only `fraction`): `ItemRow` gains `stage: JobStage | null`, `null` from `itemStarted` (none is synthesised), set from `ItemProgress.stage`, kept across the `itemFinished` merge.
  > A `null`-fraction row with a stage renders the staged bar; one with a `null` stage is the pre-first-tick running row.
  > Sync the `ItemRow`/`fraction` docs in `src/state/store.ts`; each changed `toEqual` row pin in `src/state/store.test.ts` carries `[Test-Change: P4.66 — old-obsolete+new-correct, §1.11 §5.3 §0.4.2]` (G70). The staged-bar legs drive synthetic `ItemProgress { fraction: null, stage }` fixtures; the core's coarse stages are §1.7's (P4.101).
- [ ] **P4.67** [UI] Build the cancel surface + the optimistic→confirmed round-trip + the 7a Cancelling sub-state · §1.11 §5.8 §5.3 §5.2 · G33a
  needs: P4.66, P4.10, P4.78
  > Scope: the §1.11/§5.8 cancel surface over the delivered chain, never rebuilt: the C7 wiring (`cancelConversionRun` in `src/lib/ipc/events.ts`: optimistic `cancelRun`, then C7 → `RunRegistry::cancel` → `GroupKillGuard`), the confirmed `RunFinished`→Summary transition and the 7a arm in `src/state/machine.ts` `fromConverting`, which P4.78 extends in place.
  > `ConvertingScreen`'s disabled "Cancelling…" label, `cancellingRef` double-C7 guard and document-level Esc carry forward too; only the Cancel button renderer follows the P4.66 supersede, and this box adds no reducer cell.
  > Builds: (1) §5.6 focus-on-entry to the Cancel button on entering Converting (7); (2) the §5.6(d) 7a rule: the winding-down item's `role="progressbar"` keeps its last `aria-valuenow` until its terminal row, held across the P4.66 staged-bar rebuild.
  > (3) A machine test pinning the delivered 7a arm on the P4.78-completed reducer (no new arm; the 7a↔11 cells are P4.78's (i)). G33a jsdom legs for (1) and (2).
  > Same commit (DoD 2): the ownership comments in `src/components/ConvertingScreen.tsx` (focus-on-entry is P4.67's, not P4.70.3's) and `src/components/ProgressList.tsx` (the §5.6(d) valuenow retention is P4.67's).
  - [ ] **P4.67.1** [UI,RUST] Build the QuitConfirm component (state 11, `app://close-requested`) — a focus-trapped `role="alertdialog"` over Converting, dispatching onto the P4.78 cells · §5.2 §5.3 §7.3.3 §7.3.2 §5.10 §5.6 §5.6.1 §0.4.1 · G29 G33a G57
    needs: P2.39, P2.120
    > Scope: the §5.3 `QuitConfirm.tsx` (state 11) per its §5.3, §5.6 and §5.10 rows, over the inert-but-mounted Converting (7); it dispatches onto the P4.78 state-11 cells by passing the `onCloseRequested` handler to `useAppEvents` and adds no reducer cell.
    > Literals via `strings/ui.ts` (G57); the §5.6.1(2) assertive announce on entry and the §5.6.1(3) Stay landing are validated in P9.15.
    > Quit invokes C15 `confirm_quit` per its §0.4.1 row and §7.3.3 step 2, a `pub async fn` returning `Result<(), IpcError>`. `RunRegistry` has no await primitive, so the bounded wait for `has_active_run()` to turn false polls or is notified on `finish`, a `[Build-Session-Entscheidung: P4.67.1]`. Stay and Esc invoke nothing; G23 is untouched (C15 is no conversion command).
    > Never a `core:window:allow-close` grant with a WebView `close()`: that is an L(-1) §0.10 capability change, and `close()` re-raises `CloseRequested` into the busy guard, handing the wind-down wait to JS (§7.3.2).
    > Deliverables: the golden entry (`src-tauri/ipc-commands.golden`, check 12), `xtask codegen` → `bindings.ts` with the `src/lib/ipc/commands.ts` wrapper, the `crate::ipc` `HANDLERS` row and `confirm_quit` in the `lib.rs` command-surface name pins. Plan-lint check 9 admits C15, so there is no L(-1) tail.
- [ ] **P4.68** [UI] Build the result-actions / open-folder flow (OpenActions → C9 OpenTarget, split-divert two-button, Summary-only) · §7.7 §5.3 · G33a
  needs: P2.7, P1.27, P3.59
  > Scope: the §7.7/§5.3 OpenActions per its §5.3 row (C9 `open_path { target: OpenTarget }` resolved against `State<RunResultStore>`, the split-divert two buttons and connector line), Summary-only (state 8): the §7.7.3 `RunResult` membership is final only at terminal. It supersedes the P3.59 slice Summary OpenActions.
  > The delivered screen-prefixed `strings/ui.ts` keys stand (`summary_open_folder`, `summary_open_source_folder`, `summary_open_saved_to_folder`, `summary_saved_to_connector`); this box adds `summary_open_file` for the single-output "Open file" button.
  > The Ctrl/⌘+Shift+F and Ctrl/⌘+Shift+Enter Summary chords are P4.70.3's state-keyed gate, which fires this box's C9 handlers.
- [ ] **P4.69** [UI,RUST] Build the error / edge-state copy framework (ResultSummary + CommandError + AppFaultNotice rendering §2.8/§2.13.3 strings verbatim, residue path) · §2.8 §2.13.3 §5.7 §5.3 §5.2 §5.8 §1.12 §0.6 · G57 G33a
  needs: P4.50, P2.12, P2.19, P3.56, P3.59, P3.60, P3.68, P4.72, P4.65
  > Delta over the P3.59 ResultSummary and the P3.60 fault screens: the §5.3 CommandError slot in every state its rule names (About's is P8.9's), the DestinationBar up-front-fail `Note`, `ItemResult.lossy` and its render, and the results-list windowing. Everything else carries forward, never re-authored.
  > The CommandError rule's opaque half (§5.3; §5.2 row 12 class (c)): a rejection that is not a structured `IpcError` enters AppFault (12) from the issuing state, with its §5.8 chrome line in `strings/ui.ts`; this box builds the façade split, the reducer cell and the copy. An opaque C6/C7 rejection stays P4.50.3's run path.
  > ResultSummary renders the resolved `OutcomeMsg.text` and `summaryLineDisplay` verbatim (§2.8.2). The P3.60 MixedDropRefusal, UnsupportedNotice and AppFaultNotice carry forward unchanged (AppFaultNotice renders `AppFault.message`, the §2.13.5 line, never a §2.8.2 one); their focus work is P4.70.1's, their announce work P4.75's.
  > DestinationBar leg (its §5.3 row, §5.7 *Fails fast up front*): render P4.72's `PreflightVerdict.up_front_fail_text` verbatim as the passive `Note` beside the disabled Convert, extending the durable P3.56 DestinationBar. This `Note` and the CommandError slot wrap P4.65's `Note` primitive.
  > Windowing (§1.10, §5.6): the rebuilt ResultSummary windows its rows over the P3.55 `useVirtualWindow`, which is fixed-row-height while the slice `ResultRow` is variable-height with a focusable reveal-residue button. Give the rows a fixed-height layout or escalate per the hook's rule; never add a dependency.
  > Same commit: rewrite the `src/components/ResultSummary.tsx` slice comment that homes windowing on its own P4 box.
  > The §1.12 `reason` slot: residue precedence stays core-side in `crate::orchestrator::project_run_result` (`residue_item_reason(..).or(base_reason)`), and the lossy note rides `ItemResult.lossy` (§0.6, §1.12, §2.8.2). Add the field, re-word the `ItemResult.reason` and `OutcomeMsg` `///` docs (`xtask codegen`) and the PRECEDENCE comment, and render both notes verbatim in `ResultSummary.tsx`.
  > The lossy-plus-residue case is not constructible in P4 (`item_base_reason` has only Skipped and Failed arms, no per-item producer writes a Lossy note, and P4.65's `lossy_note` caller is the offer-time C4 set), so its producer and pinning `#[test]` are P6.95 (P6.55 `needs:` it).
- [ ] **P4.70** [UI] Build the structural-a11y wiring on the harness components (ARIA roles + keyboard operability + focus management) · §5.6 §5.10 · G33a G57
  needs: P4.64, P4.65, P4.66, P4.68, P4.69, P3.54, P3.57
  > Scope: the §5.6 structural a11y on the P4-built surfaces, one sub-box per component contract (ARIA, keyboard, focus), all wired via the P1 `a11y/` module (keymap, §5.10). Each sub-box lands its components' §6.4.6a `vitest-axe` jsdom assertions under `test:a11y` (G33a).
  > The DropZone that .1 wires is the rebuild superseding the P3.54 slice DropZone; FormatPicker, OptionsPanel, ProgressList and OpenActions are the P4.64–P4.68 rebuilds; the DestinationBar is the durable P3.56 build, extended by P4.69.
  > .4 extends the P3.57 slice RerunPrompt with focus-restore-to-trigger and the §5.6(c) accelerator suppression. The §5.6.1(2) announce-on-entry wiring over the P1.39 `announcer.ts` is P4.75's, a disjoint surface.
  - [ ] **P4.70.1** [UI] Build the DropZone structural a11y (role=button + drag-drop + keyboard activation + focus-on-entry) · §5.6 §5.10 · G33a
    > The §5.6/§5.10 DropZone a11y on the P4 rebuild of the P3.54 slice DropZone, carrying forward every delivered leg (`DropZone.test.tsx`, `DropZone.a11y.test.tsx`, `MixedDropRefusal.test.tsx`).
    > Carried forward: the native `<button>`, Enter/Space → C2a `pick_for_intake { kind: 'files' }`, the choose-folder button → `{ kind: 'folder' }` (C2b is the DestinationBar's), the keyboard picker fallback (§5.6) and the state-9 re-drop focus-on-entry.
    > The Ctrl/⌘+O and Ctrl/⌘+Shift+O chords bind through `a11y/keymap.ts` in Idle only; state 9 re-drops via Enter/Space (§5.10 *Split semantics*).
    > New: focus-on-entry to the drop surface on every entry into Idle (1), one focus leg per path: the initial mount, the post-C13 return, Esc/Dismiss from MixedDropRefusal (9) onto the new Idle instance's DropZone, Dismiss/Esc from UnsupportedNotice (10), Esc from Confirm (3), and the Ctrl/⌘+N / Start-over returns from 4/5/8/12 (§5.6, §5.10).
    > Collecting (2) focuses its cancel-collect control (§5.6), never the drop affordance.
  - [ ] **P4.70.2** [UI] Build the FormatPicker + OptionsPanel structural a11y (radiogroup/roving-tabindex/aria-checked + aria-disabled patent-gap tiles + labelled option widgets) · §5.6 §5.2 §1.6 · G33a
    > FormatPicker: a `role="radiogroup"` of `role="radio"` tiles with `aria-checked` and roving tabindex (one tab stop, arrow keys), focus on the default tile on entering Targets; the OptionsPanel widgets (P4.64) each labelled and keyboard-operable. Both are the P4.64/P4.65 rebuilds.
    > Render source (§5.2 *Patent-gapped / unavailable target rendering*): a tile's disabled state and reason come from the C3 `Target.availability`; `Unavailable { reason }` → `aria-disabled="true"`, kept in the arrow-key order with `tabindex="-1"`, the reason shown verbatim and linked by `aria-describedby` (§5.6.1(1) *Target tile*).
    > `EngineHealth.unavailableTargets` carries no reason, so it is never a render or string source; any set-only consistency check and its mismatch behaviour are a `[Build-Session-Entscheidung: P4.70.2]`.
    > Same commit (G68): the P2.114 seam comments on `engineHealth` and `selectUnavailableTargets` in `src/state/store.ts` and `src/state/store.test.ts` name `Target.availability` as the reason carrier.
  - [ ] **P4.70.3** [UI] Build the ProgressList, DestinationBar and OpenActions structural a11y, the Summary focus order and the one state-keyed §5.10 workflow-chord gate · §5.6 §5.10 §1.11 · G33a
    > ProgressList per its §5.6.1(1) row and the §5.6 terminal-transition rule (each item and the aggregate bar; the indeterminate row's `aria-busy` cleared on terminal). P4.66's staged bar is visual only and exposes no implicit value (never a native `<progress value>`); the 7a last-value freeze is P4.67's.
    > DestinationBar Convert/Change labelled and keyboard-operable, focus moved to Convert only the first time it appears for the batch, never on a debounced C4 re-call (§5.6); OpenActions' split-divert two buttons keyboard-operable.
    > The §5.6 Summary focus order on entering state 8 (§5.6.1(3) row 8) over P4.69's windowed list: a row target is scrolled into the window and rendered before focus is set.
    > §5.10 workflow chords (the leg `TargetsScreen`, `SummaryScreen`, `OpenActions` and `strings/ui.ts` assign here): bind the P1.40 `a11y/keymap.ts` entries via `matchesAccelerator`, each firing its button's handler, never a second path.
    > Bindings: `startOver` → `cancel` in 4/5 and `convertMore` in 8; `backToConfirm` → `back` in 4/5; `changeDestination` → the C2b→C5 path in 4/5; `convert` → the Convert handler in 4/5 once the destination shows, never while disabled; `openOutputFolder` → the common-root C9 in 8; `openOutputFile` → P4.68's "Open file" C9 in 8 when exactly one output exists.
    > Every binding keys on the active §5.2 state per the §5.10 *Available in* column, never on mount (Targets/Destination stay mounted-but-inert under RerunPrompt (6), §5.3). P4.70.4's §5.6(c) suppression and P4.74's Toggle-Advanced chord use this one gate, never a per-component guard.
    > Tests: each chord fires in its active state and asserts its handler (the C9 call or the Msg); a state-6 leg asserts the 4/5 chords stay inert. Out of scope: AppFault's Ctrl/⌘+N (P3.60), Idle's Ctrl/⌘+O and Ctrl/⌘+Shift+O (P3.54/P3.60), Ctrl/⌘+. (P4.74), F1/? (P8.18).
  - [ ] **P4.70.4** [UI] Build the decision-modal (RerunPrompt) structural a11y (focus-restore-to-trigger on close + §5.6(c) global-accelerator suppression while a modal is open) · §5.6 §5.10 · G33a
    > the §5.6 modal focus-management the P3.57 slice defers: on RerunPrompt (6) close (Cancel/Esc), restore focus to the **Convert button that opened it** (§5.6's "Focus-restore-on-close is scoped to the modals that HAVE a UI trigger" [DECIDED] rule / §5.10's "Close a focus-trapped dialog (restore focus to trigger)" row) — the P3.57 slice returns to Targets landing on the FormatPicker's default-tile focus, not the trigger; and the **§5.6(c)** reducer/keymap suppression of the global accelerators (Ctrl/⌘+N/O/Backspace) while state 6 is open, through the P4.70.3 state-keyed chord gate. EXTENDS the P3.57 slice RerunPrompt (which ships the focus-trap + default-focus-on-Skip + Esc-cancel + the commit-final Cancel guard); QuitConfirm (11, P4.67.1) / AboutDialog join the same modal-a11y contract as they land (the `needs: P3.57` edge sits on the parent P4.70).
- [x] **P4.71** [TEST] Wire the §6.4.6a `vitest-axe` jsdom a11y assertions over the P4 harness component tree (ARIA/role validity + focus-order) · §6.4.6a §5.6 · G33a
  > RECONCILE: folded into P4.70 — each P4.70 sub-box lands its components' `vitest-axe` jsdom assertions.

## Resource pre-flight & budgets engine (§1.10)

> The §1.10 `[DECIDED design]` estimation+decision mechanism — the cross-cutting home
> is P4 (every engine phase depends on it; P4 owns the §0.9 pool + §2.14 staging it
> composes with). P2 DECLARED the `PreflightVerdict` DTO + C4 RETURNS it; P3 FEEDS it
> the walking-skeleton verdict; P5/P6 FEED inputs (SVG clamp, to-GIF estimate). These
> two boxes BUILD the engine those declarations/feeds resolve against; P9.41 then
> VALIDATES + calibrates the numbers against the corpus.

- [ ] **P4.72** [RUST] Build the §1.10 pre-flight engine — `SizeEstimate`, per-physical-volume footprint grouping, headroom, the whole-batch `up_front_fail` and its text · §1.10 §2.14.4 §0.6 · G31
  needs: P4.20, P2.11, P3.37, P3.17, P4.88
  > Scope: §1.10 *Up-front estimation* (`SizeEstimate`, first authored here), decision point 1 and the constants table, composed with the §0.9 degree and the §2.14 scratch layout; the § is the binding field list. The macOS staged-input term is §2.14.2's peak-concurrent bound, never the whole-batch Σ; P4.88's per-item reclaim makes it true.
  > Text leg: add the §0.6 `PreflightVerdict.up_front_fail_text` with the `bindings.ts` regen, rendered from the §2.8.2 batch-scoped lines (`up_front_too_big`, `up_front_out_of_disk`); P4.69 renders it verbatim. Each line is a hoisted `pub const` template in `crate::outcome` (the `residue_annotation` / `BATCH_*_TEMPLATE` pattern).
  > Each such template is bound per row in `every_catalog_row_is_bound_to_its_spec_table_row`, and that test's enumerating comment is updated in the same commit.
  > `PER_ITEM_OUTPUT_CEILING` is a `pub const` beside `HEADROOM_MARGIN` and `AGGREGATE_OUTPUT_CEILING`, with its predicate over `SizeEstimate.est_output_bytes`; P4.73.3 (point 2) and P6.73 (to-GIF) consume it. Same commit (DoD 2): the code doc of `PreflightVerdict.up_front_fail` and its `bindings.ts` mirror name §1.10 points 2–4 for the per-item kinds.
  > Volume key: the §2.3.1 identity `fs_guard::resolve_identity(dir)?.dev_or_volserial`, the key `test_volumes::volume_of` already uses, never a second shim in `crate::platform`; `platform::available_bytes` stays the one free-space read.
  > Scratch base: the §2.14.2 `RunScratch` base `app_local_data_dir()`, threaded from the C4/C5 handlers' `AppHandle` into `orchestrator::plan_output_preview` as a plain `&Path` (no `AppHandle` below tier 0). Absent before the first run, it resolves key and free space on its nearest existing ancestor; the preview never creates it, and an `Err` never reads as "fits".
  > A `[Build-Session-Entscheidung: P4.72]` records that `dev_or_volserial` is per mount, not per free-space pool (APFS container volumes, btrfs subvolumes, non-unique Windows serials), and how the check stays conservative or which limitation remains.
  > Tests (G31): two dirs on one volume form one group; a `test_volumes::second_volume_dir` pair forms two (skip or require per `CONVERTIA_REQUIRE_SECOND_VOLUME`); a missing base resolves through its ancestor.
  > Wire: the estimator replaces the trivial verdict inside `orchestrator::plan_output_preview`, the single production site C4 (`resolve_output_plan`) and C5 (`resolve_destination_change`) delegate to, so C5 re-checks the new destination by construction. New inputs (the §0.9 degree, the free-space read, the scratch base) enter through its signature; a C4-handler patch would leave C5 on the stub.
  > Tests: the C4 and C5 resolve tests each assert `up_front_fail = Some(OutOfDisk)` below the grouped footprint × `HEADROOM_MARGIN`, the free-space figure through a seam the builder picks and tags; the CSV→TSV `None` asserts stay. Same commit: re-word the stub wording in the `plan_output_preview`/`plan_output` docs and the `planning.rs` assert messages.
  > Feeder seam: one pure up-front estimator over what C4/C5 hold (detection `dims` and `total_bytes`, the `TargetId`, the C4 `OptionValues`) returning a `SizeEstimate` with `basis: PerCategoryHeuristic`.
  > This box fills the §1.10 category defaults, P5.29.2 the SVG→raster arm (the render size in `OptionValues`; the viewBox size resolves only in the worker), P6.73 the to-GIF arm (`trim_or_cap`, no `ffprobe`).
  > The seam's home (a required `Engine` method, the P4.21 `parallelism` precedent, or one `estimate`-module table keyed by (source, target)) is a `[Build-Session-Entscheidung: P4.72]`. A pair without its own arm falls back to its §1.10 category heuristic, never zero, and every heuristic has a real cap computed with checked or saturating arithmetic.
- [ ] **P4.73** [RUST] Build the §1.10 per-item `Failed(TooBig|OutOfDisk)` enforcement — points 2–4 of the decision table · §1.10 §2.1 §2.8 · G31
  needs: P4.72, P3.38
  > builds the per-item points of the §1.10 decision table `[DECIDED]` — point 2 before spawn and point 3 in the §2.1.1 step-2 dispatch (P4.73.3), point 4 in the core's own write/publish error mappings (P4.73.1) — with the §2.6.2 **Out-of-disk mid-write** trigger row as its cleanup; point 1 is P4.72's. (`needs: P3.38` for the §2.1.1 per-item write sequence.)
  > Built as its sub-boxes; each lands as its own commit where _format.md allows it.
  - [ ] **P4.73.1** [RUST] Map disk-full on the core's own writes to `OutOfDisk` through one shared `StorageFull`/`QuotaExceeded` predicate · §1.10 §2.8.1 · G31
    > Enforcement leg (b), §1.10 point 4: a disk-full from the core's own writes is `OutOfDisk`, never `WriteFailed` (§2.8.1). `From<TransformError>` (`TransformError::Write(_)`) and `map_publish_error` (`PublishError::Io(_)`) split on the `StorageFull`/`QuotaExceeded` predicate `staging_failure_kind` (P4.25) already uses, factored into one shared helper.
  - [ ] **P4.73.2** [RUST] Run each item's engine in a per-item kind-2 working sub-directory, removed at its terminal transition · §1.10 §2.14.2 §2.6.2 · G31
    > Ruling (ii), the per-item working directory: its spec is §2.14.2 (§1.10 point 3 counts it, §2.6.2 removes it). §1.7 sets `cwd` and the temp env to it after planning, as it fills `out_tmp`; P7.8 adds the plan-time input for argv-embedded per-item paths (the LibreOffice `--outdir`). The removal sits beside the P4.88 staged-copy reclaim.
    > Same commit (DoD 2): the `engines/mod.rs` mirror of the §3.2.2 `Invocation.cwd` comment.
  - [ ] **P4.73.3** [RUST] Build the §1.10 point-2 dispatch check, the watchdog byte-budget kill, the one-victim free-space arbiter and the per-item RSS ceiling · §1.10 §1.7 · G31
    needs: P4.100, P4.73.2
    > Scope: §1.10 points 2 and 3 with their constants and per-OS table; point 4 is P4.73.1's.
    > (a) `TooBig` is a mid-run byte-budget kill, never a check after exit (the test-strategy §6 T10 row; P9.41 calibrates that case against this box). The `WATCHDOG_POLL_INTERVAL` loop of `engines::bounded_confined_run` compares `out_tmp` with the P4.72 per-item ceiling and the ruling-(i) byte budget, and `out_tmp` plus the item's working directory (P4.73.2) with `PER_ITEM_SCRATCH_CEILING`.
    > A breach returns a `Failed(TooBig)` `ConfinedRun`; dropping the run triggers the P4.10 group-kill, and the conductor's `Failed` arm cleans `tmp` (§2.6.2). The in-core §3.5.6 lane enforces the same ceiling in its write loop.
    > (c) The same poll reads `crate::platform::available_bytes` for `tmp`'s volume and the scratch volume; below `FREE_SPACE_MARGIN` the ruling-(iii) arbiter kills its one victim to `Failed(OutOfDisk)` before an engine hits ENOSPC, and the batch continues through the per-item `Failed` arm. This replaces the per-engine `classify_failure` out-of-disk spellings, so no P5–P7 engine box needs P4.73.
    > Each item's poll updates a run-scoped table of attributable bytes that the one arbiter reads to choose the victim per interval; their home is a `[Build-Session-Entscheidung: P4.73.3]`. The one-victim leg is vacuous under the sequential conductor until P4.86, so it is driven against a hand-built concurrent set, never the live conductor.
    > RSS ceiling in the same poll (the §1.10 per-OS table and its probe fallback, without escalation); on Windows the Leg-B job's `JOB_MEMORY_CAP_BYTES` (16 GiB) gives way to `PER_ITEM_RSS_CEILING`.
    > Point 2: before an item's engine spawns, the P4.72 predicate fails a projection over `PER_ITEM_OUTPUT_CEILING` to `Failed(TooBig)`; P6.73 adds the to-GIF arm (`GIF_ESTIMATE_CEILING`).
- [ ] **P4.102** [RUST] Carry the §2.1.1 step-7 contract as `PublishError::PublishedNotDurable`, never a pre-create failure · §2.1.1 §2.14.3 §2.8 · G31
  needs: P3.16, P3.17, P3.36
  > A directory-fsync error after a successful no-replace rename returns `PublishError::PublishedNotDurable` (`final` exists), never `Io`; the orchestrator never routes it to the late divert and reports `WriteFailed` with `final` kept (§2.1.1 step 7).
  > Test: the §2.14.3 cross-volume path with an injected fsync failure after the rename leaves exactly one output, none at the divert target.
- [ ] **P4.103** [RUST] Divert an unsupported no-replace rename on Windows FAT32/exFAT to the §2.7.2 `NoAtomicPublish` path · §2.1.2 §2.7.2 · G31
  needs: P3.14, P3.36
  > `crate::platform::rename_noreplace_at`: the NTSTATUS set a FAT32 or exFAT volume returns for `FileRenameInformationEx` maps to the §2.7.2 `NoAtomicPublish` divert — never `Failed`, never a replace. The §2.1.2 per-OS table carries the FAT row.
  > Realizability probe at this box: the set is measured on FAT32 and exFAT volumes (at least `STATUS_INVALID_INFO_CLASS`, `STATUS_INVALID_PARAMETER`, `STATUS_NOT_SUPPORTED`). Without a mountable VHD on the build host, an injected-status unit test proves the mapping and the Lane-B loop-mount leg (P11.25) is the real-volume proof, without escalation.

## Deferred-split completions & cross-phase reconciliation

> The split-off siblings of P4.52/P4.64/P4.70 (each a genuinely disjoint surface that
> must carry its own dual review, _format.md §3.2 / build-loop §3 step 2). They sit after
> their parents (document order) and before the proof-of-life exit gate (P4.79/P4.80);
> each is its own P4 deliverable (the phase is "done" only when every `[ ]` box is `[x]`),
> independent of the proof-of-life predicate the exit gate asserts.

- [ ] **P4.74** [UI] Build the AdvancedDrawer collapsed-by-default shell over the OptionsPanel (Advanced-tier reveal) · §1.6 §5.3 §5.10 · G33a
  needs: P4.64, P4.70
  > The §1.6/§5.3 AdvancedDrawer chrome over P4.64's widget dispatch: a collapsed-by-default toggle revealing the Advanced-tier `OptionDecl` widgets P4.64 renders; it never gates conversion (its §5.3 row; §1.6 no-decision defaulting). P5–P7 Advanced-tier declarations register against it.
  > Keyboard and disclosure (§5.10, §5.6): the toggle is a native `<button>` (Tab + Enter/Space, the §5.6 "open Advanced" path and the §5.6.1(3) state-4/5 Tab stop) with `aria-expanded` and `aria-controls` to the open region, the P3.55 FileList disclosure precedent (a repo pattern, not a §5.6.1(1) row).
  > The §5.10 Toggle Advanced chord `keymap.toggleAdvancedOptions` opens and closes it via `matchesAccelerator` in Targets (4) only, through the P4.70.3 state-keyed gate, so it is inert under RerunPrompt (6). Tests: the drawer collapsed and expanded (G33a), and the chord toggling `aria-expanded`.
- [ ] **P4.75** [UI] Wire the §5.6.1(2) live-region announce-on-state-entry calls + §5.6 throttling onto the P1.39 `announcer.ts` mechanism · §5.6.1 §5.6 · G33a G57
  needs: P4.70
  > the announce-on-state-entry wiring split from P4.70's structural a11y (P1.39 DELIVERED the live-region mechanism — `announce(message, priority)` over the polite/assertive regions; its header assigns the per-component wiring + throttling to P4/P8; this box EXPANDS, never rebuilds): wire the §5.6.1(2) announce-on-entry calls onto the existing announcer — its assertive and polite sets exactly as that list enumerates them (the Confirm-3 assertive announce already fires via BatchSummary, P3.55 — preserve/extend through the P4 screen supersede, never double-fire) — plus the §5.6 `Converting` milestone throttling — the distinct-from-focus-management live-region mechanism P9.15 validates (the progressbar's terminal-transition `aria-busy`/`aria-valuenow` state is per-element, P4.70.3). Disjoint surface from P4.70 (the shared announcer vs per-element roles/keyboard/focus), independently buildable + separately dual-reviewed.
- [ ] **P4.76** [BUILD] Build the image-worker §6.1.3 carve-out-(ii) relink-bundle assertion over its static LGPL closure · §6.1.3 §3.6.2 · G38b
  needs: P4.51, P4.34
  l-neg1: sweep-tail
  > The carve-out (ii) assertion over the worker's static LGPL closure (libvips, GLib/GObject): a function over a bundle ROOT and the closure rows, proven by a planted-complete leg and one planted-missing leg per member class (corresponding source, relinkable unit, relink recipe).
  > Stage-time real-tree leg: every from-source row has its verified `.src` entry, fail-closed from the first run. The unit/recipe half runs over the root P10.21 assembles, never fail-open. The P4 closure has no GPL member; the P5 acquisition's rows join the same function.
  > Sweep tail (the P4.81 list): the planted positive from outside the tool in `g24-stage-engines.py` for this box's stage-time arm (the P4.41 `stage-engines` condition).
- [x] **P4.77** [GATE] Wire the cross-phase reconciliation obligation — every deferred P4→P1/P2/P3 `needs:` edge declared, no half-wired plan declares done · G7 G20
  > RECONCILE: void — cross-phase edges live on the consumer boxes and the check-31 phase chain (`_format.md` §5a); the planned plan-lint legs are not built.

## P4.16a — Full §5.2 frontend state machine (completes the P3 slice subset)

> P3.53 built only the §5.2 *slice subset* (states 1→2→3→4/5→[6]→7→8 + 9/10 +
> `app://fault`→12). §5.2 is the named owner of the full 12-state FSM + transition
> diagram; no later box completed it (the P1.31 "P2/P8" note was wrong). This box
> extends the slice reducer to **all 12 states** so the cancel/quit boxes (P4.67/
> P4.67.1) and the P4.79 UX-harness exit leg drive a complete machine, not a partial
> one. It is the machine the 7a/11 surface boxes dispatch onto.

- [ ] **P4.78** [UI,RUST] Complete the full §5.2 12-state reducer FSM — the state-11 cells (incl. the §5.6(a) 7a sub-case), the §5.4 intake and launch arms — over the §5.1 store · §5.2 §5.4 §5.6 §5.8 §5.1 §0.6 · G33a G57
  needs: P3.53, P1.31.2
  > Extend the P3.53 slice reducer to the complete §5.2 transition table: the machine half of a build-vs-wire split, pure in `src/state/machine.ts`, every cell a test. It supersedes the P3.53 subset in place by extension (the reducer is the one FSM), driven by inbound IPC (§5.8), its state in the §5.1 store (P1.31.2), literals via `strings/ui.ts` (G57).
  > (i) State 11 `AppCloseRequested`: an overlay variant carrying the underlying Converting state with its cancelling bit. Cells: 7/7a + close-requested → 11; Stay → the underlying 7 or 7a (§5.6(a)); Quit → stays 11 (the backend exit ends the process, §7.3.3).
  > The cells §5.2 leaves undefined (11 × run-finished, run-fault, a repeated close-requested) derive from §7.3.3 ("batch continues") under a decision tag, or escalate. The same box adds the `App.tsx` `screenFor` arm for 11 rendering the underlying Converting screen (P4.67.1 overlays QuitConfirm); the Msg names are a `[Build-Session-Entscheidung: P4.78]`.
  > (ii) The §5.4/§5.8 fresh-intake arms: Confirm (3), Targets/Destination (4/5), Summary (8), Unsupported (10) and AppFault (12) → Collecting on an intake nudge, discarding the left state, via `intakeEntryMsg` in `src/lib/ipc/events.ts` extended to the full §5.4 set; a non-intake state drains with C1's `discard` flag (§5.4; the BusyNotice is P8.1.1's).
  > (iii) Same commit: the slice comments in `machine.ts`, `store.ts`, `App.tsx`, `App.test.tsx` and `events.ts` that home these cells here, with the `events.test.ts` state-10-not-drainable leg replaced under `[Test-Change: P4.78 — old-obsolete+new-correct, §5.4]`.
  > (iv) The §5.4 *Launch-time intake* entry and the §0.6 `NothingPending` arm: the Rust variant and C1's `discard` flag with the empty-buffer return (their docs, tests and `xtask codegen` move with them), the machine arm, and the mount drain's first-`onScan`-tick entry into Collecting under the nudge path's stale-walk guard; `launchCollectingState` retires.
  > While the mount drain is in flight, `Idle` takes a nudge the non-intake way (a `discard` drain, §5.4 *Launch-time intake*).
  > P4.67 and P4.67.1 add no reducer cell; the state-12 entry cells are P4.50.3's (the run path) and P4.69's (an opaque command rejection); P4.93 audits that every slice arm is still dispatched by its superseding screen.

## Proof-of-life exit gate

- [ ] **P4.79** [TEST] Drive a representative in-P4 image round-trip to a first ledger cell (engine half) and render that run through the P4-built UI under jsdom (UI half) · §6.5 §6.4.3 §5.7 · G31 G33a
  needs: P4.38, P4.59, P4.61, P4.64, P4.65, P4.66, P4.67, P4.68, P4.92
  > The P4 UX-harness exit leg, self-contained in P4 (it stages its own input and never reaches into P5): a minimal synthetic image fixture (e.g. a few-pixel PNG) under `tests/corpus/` with its SHA-256 in the manifest (G24a), its `covers [PNG,PNG]` entry registered against the P4.37 PNG→PNG harness row (`EngineId::ImageCore`, `Direction::Encode`, registry-only).
  > PNG→PNG is the one round-trip the P4 worker does with no P5 saver wiring: libvips' native `pngload`/`pngsave` over libspng, which the P4.89 closure enables in `libvips.configure.flags`. A non-diagonal pair would pull a P5 saver into P4.
  > The fixture's `[[file]]` entry carries `harness_fixture = true`: the §6.4.3a exception exempts its diagonal from P4.60's stale-coupling leg, and P4.61 emits it as the §6.5.2 informational row outside the release-gate set.
  > Engine half: below C6 through the P3.63 `run_conversion_tests` harness to a §6.4.3 structural-reader pass (`vipsheader` decode with nonzero dims and the G31 (4) `vips stats` has-content check; the reader is P4.89 (e)'s) and a first §6.5.2 ledger cell over the fixture. No harness-scoped C6 door exists.
  > UI half: the P4-built options panel, lossy note, progress/cancel and result actions render that run's events under jsdom, the IPC façade `vi.mock`'ed to offer the harness target; the real-WebView E2E is P9's.
- [ ] **P4.80** [TEST] Verify the P4 proof-of-life exit criterion (imgworker boots + isolated round-trip + populated EngineHealth + first reliability report) · §3.5.5 §2.12 §7.2.3 §6.4.3 · G46 G31
  needs: P4.38, P4.45, P4.60, P4.61, P4.79, P4.95
  > The consolidated P4 exit gate (the header's proof-of-life criterion): `convertia-imgworker` boots, a round-trip succeeds through the §2.12 boundary (P4.38), the §7.2.3 startup verifier reports a populated `EngineHealth` (P4.45), and P4.79 passes in both halves.
  > The §6.4.3 runner, the §6.5.2 ledger and the §6.4.3a bijection guard (the P4.95 bin) execute and produce their first report over the P4-era corpus (the P3 CSV↔TSV pairs and the flagged P4.79 fixture), every in-force leg green (P4.59–P4.61).
  > The §6.4.3a required-pair leg runs per §6.4.3a *Phase-in*; its full-§04 green is the P11 release check (§6.10 row 3), not this gate.

## The §7.2.5 orphan-reclaim slot body — sweep_stale at startup (deferred from P2.106.5)

> P2.106.5 built §7.2 step 5 as the scratch+log dir creation with an `Ok(())` orphan-reclaim SLOT
> ("mechanism §2.6, body P3/P4") — and no P4 box named the body, leaving `crate::run::sweep_stale`'s
> intended §7.2.5 startup caller owner-less (the P3.74 [Decision] FLAG, resolved by
> authoring this box). Appended at end-of-phase (`.82`, the max+1 id) per the established convention.

- [ ] **P4.82** [RUST] Fill the §7.2.5 orphan-reclaim slot — `crate::run::sweep_stale` before the window shows, logged per §7.5 — and the startup `ScratchUnavailable` probe · §7.2.5 §7.2.1 §2.6 §2.13.3 §2.8 §0.4.2
  needs: P2.106.5, P3.23
  > Scope: the §7.2.1 step-5 body in the crate-root `prepare_scratch_and_log` slot (lib.rs since P3.87; the run/mod.rs caller note's old `main::` phrasing is corrected when touched): `crate::run::sweep_stale` over the launch's `app_local_data_dir()` scratch base before the window shows (§7.2.5 the when, §2.6 the mechanism), held-lock-gated (§2.6.3).
  > The outcome is logged per §7.5: the structural `InstanceId`/`RunId` at the default level, the full path only in verbose (§7.5.3); a cleanup that cannot complete is logged, never a clean success (§7.2.5). That line is §7.2.5's "surfaced per §2.6": every §2.6.4 surface is per item or run, and none exists at step 5.
  > Tag the call site `[Derived-Assumption: P4.82 — §2.6.4 surfaces are per-item; the startup could-not-reclaim surface is the §7.5 log line]`. A sweep error is best-effort, never an `AppFault`; the destination-resident `*.part` reclaim stays §2.6.3(b)/P3.24's.
  > `sweep_stale` returns only the removed dirs (a Remove-verdict dir whose `remove_dir_all` fails is silently absent, P3.23), so extend `sweep_stale`/`sweep_stale_within` to also surface the failed reclaims (e.g. `SweepOutcome { removed, failed }`) and log both at the startup caller.
  > Never log inside the sweep body: the P3.74 `RunEvent::Exit` caller runs after the log flush (§7.3.2) and keeps its best-effort `drop(…)` posture, which still satisfies the lib.rs `sweep_stale` source-scan needle. The two callers share one sweep and lock gate; the `run::sweep_tests` assertions update under `[Test-Change]` (test-strategy §8).
  > Writability probe (§7.2.1 step 5, §2.13.3): create the per-instance root, then write and delete a probe file in a `.lock`-held `run-<probe RunId>/` taken through `RunScratch::acquire` and released by `cleanup_run` (the P4.44.1 pattern), so a concurrent sweep cannot remove it and a crash residue is reclaimable. A failure returns `ScratchUnavailable` (§2.13.5); only the probe raises an `AppFault`.
  > It adds the variant, the `[Test-Change]` on the wire pins and the `complete_kind_list!` rosters, the `xtask codegen` regen and the code sites that list the app-level kinds, found by grep (e.g. `src/components/AppFaultNotice.tsx`, `src/lib/ipc/events.ts`, the `lib.rs` readiness-gate docs naming `EngineMissing`/`BundleDamaged`). P4.50 presents the kind.

## The P4 §0.8 drift-floor rows — the dep↔floor split (the P3.82–P3.85 pattern)

> The paired §0.8 floor boxes for the P4 direct-dep promotions (the P3.85-named class: a
> direct-dep promotion whose paired floor box was never authored caused the P3.6/P3.12
> hard-stops). The `PINNED_FLOORS` preamble comment in `check-supply-chain` named the process-wrap + landlock floors as
> pending rows at authoring time; P4.84 landed them. Appended at end-of-phase
> (`.84`, the max+1 id) per the established convention.

- [x] **P4.84** [CI] Add the P4 dep-promotion floor rows to `check-supply-chain`'s §0.8 `PINNED_FLOORS` — `process-wrap` (P4.10), `landlock` (P4.15.1), `seccompiler` (P4.15.3) — plus any further P4 direct-dep promotion's row · §0.8 §1.7 §1.10 §2.12.3 §2.12.4 · G18 G18a
  needs: P4.10, P4.15.1, P4.15.3
  > Delivered: 40af538

## The G29 process-isolation rule refinement — the pre-P4.13 L(-1) act

> The process-isolation.yaml comments defer the (b1)/(d) rule refinements to "P1" — never
> homed in any box — and the first production `Command::new` (P4.13) reds the un-refined
> CI-only G29 real scan with no Loop-fixable path, so the refinement box is authored here
> and P4.13 carries `needs: P4.85` (a DECISION-C ordering inversion, documented at that
> box's `needs:` line). Appended at end-of-phase (`.85`, the max+1 id) per the established
> convention.

- [x] **P4.85** [GATE] Refine the G29 process-isolation rules to their comment-promised precise forms before the first in-core spawn — (b1) split-builder value-flow for the owned-Command `env_clear` shape `process-wrap` forces; (d) the per-path `stage_for_tcc`-precedes-spawn dataflow/taint check + `cfg(target_os="macos")`/`paths:` scoping — + extend the planted-positive fixtures (`b1-env-clear.rs`/`d-stage-tcc.rs`) and `g24-sast.py` in the same owner-acked act · §2.12.3 §7.2.6 §0.11 · G29 G24
  > Delivered: 2e3a5dc

## The §0.9 concurrency realization + the §1.10 signal surface — the P4.20-raised plan gaps

> The two Co-Pilot items raised at P4.20 (its pre-condense note holds the evidence): the
> §0.9 degree got a correct bound but no concurrent dispatch site,
> and the §1.10 watermark pause went live without its same-sentence-mandated `LowMemoryNote`
> signal. Appended at end-of-phase (`.86`/`.87`, the max+1 convention); authored by the
> Co-Pilot. The §11.4 pre-fill audit gained surface (e) (mandate-pair & promise
> coverage) in the same commit, so the escape class is caught at the next phase boundary
> mechanically rather than re-learned (the CLAUDE §10 root-cause rule).

- [ ] **P4.86** [RUST] Re-cut the sequential tier-1 conductor to concurrent dispatch over the §0.9 bounded pool · §0.9 §1.9 §1.7 · G29
  needs: P4.20, P4.21, P4.22, P4.23, P3.48
  > Scope: `orchestrator::run_conversion`'s sequential loop becomes N-way dispatch bounded by the live P4.20 pool (`effective = min(global_degree, per_engine_cap, memory_based_cap)`; the P4.21 caps and the P4.22 `serialised_only` permit bind by construction). Its "the §0.9 concurrency degree is P4" doc is reconciled in this commit.
  > Constraints: items start in the frozen `Batch.jobs` order (never re-sorted; completions may interleave, §1.9) with the §1.11/§1.12 event and summary semantics intact. The P4.20 cancel-aware gate composes (a cancel stops new dispatch; in-flight items finish or are killed per §1.7), and the §1.10 watermark gate stays at the `engines::dispatch` entry (`Pool::await_dispatch_headroom`).
  > Acquire outside the timeout on both lanes: move the weighted permit acquisition out of the §1.7 wall-clock window on the in-core lane (`bounded_lane` wraps the whole `run_in_core` future, P3.45) and pin the subprocess lane's acquire-first order with a regression. The clock starts at permit grant, so a queued item never reports `Failed(EngineHang)` unspawned.
  > The macOS staging call (`crate::isolation::macos::engine_input` in `convert_item`) runs before any §0.9 permit, so coexisting §2.14.2 staged copies are bounded by in-flight items, not the degree. Keep the in-flight set at the degree or move the staging inside the permit (§2.14.2, and §1.10 counts from that bound); the call site records it too.
  > The serialised-engine lane acquire (`SerialisedLanes::acquire`, held across `dispatch`) is not cancel-aware: build it as the same `biased;` `select!` on the job token as the dispatch gate. Its test is P7.10's, because no `serialised_only` engine registers before P7.10, which also carries the end-to-end serialisation assertion.
  > The per-engine cap is enforced by weight (`Pool::slot_weight`: a capped job holds `ceil(degree / cap)` global permits other engines cannot use, under-admitting where `degree` is not a multiple of the cap). If this box's mixed batch shows it biting, the alternative is a per-engine semaphore (the `serialised_only` lane shape).
  > Tests: items > degree (max concurrent in-flight == the effective degree, start order == `Batch.jobs`, the batch completes, totals correct); the acquire-outside-the-timeout red-green (a slot-starved item never ticks its engine timeout); the cancel-under-contention drain; the P4.16 `many_concurrent_cheap_tier_spawns_all_complete_under_timeout` precedent extends to the conductor path.
  > Run the full suite repeatedly after adding any `select!`/cancel arm (the P4.20 `biased;` class).
- [ ] **P4.87** [UI,RUST] Carry the §1.10 low-memory watermark to the §5.2 `LowMemoryNote` — a run-scoped `ConversionEvent::LowMemory` on the progress Channel, reducer, banner · §1.10 §0.9 §5.2 §0.4.2 · G29 G57 G33a
  needs: P4.20, P2.37, P3.58, P1.31.2
  > Builds the signal half of §1.10's low-memory sentence (P6.56.1 asserts the banner end to end). Transport: the §0.4.2 `LowMemory { active }` Channel variant, never a fourth `app://` event (the §0.4.2 closed set, frozen by the caged plan-lint check 28); this box adds the variant and runs `xtask codegen`.
  > The tier-1 conductor, which already holds `on_progress` (no `AppHandle`), emits it on watermark-pause enter and exit; only new dispatch pauses and `MEMORY_PAUSE_MAX` bounds the stall (§1.10). Being run-scoped, the signal dies with the run, matching §5.2's auto-dismiss on leaving the Converting family.
  > Observer seam: the gate (`pool.await_dispatch_headroom`) sits one tier below in `engines::dispatch`, whose conductor sink is a plain `Fn(f32)` (tier 2 names no orchestrator type), so the edges reach the conductor through a new plain-bool seam beside it: a `Fn(bool)` callback or a pool-side `tokio::sync::watch::Receiver<bool>`, picked under a `[Build-Session-Entscheidung]`.
  > Frontend: the §5.8 reducer consumes the variant; the store gains `lowMemoryActive` (default `false`); `ConvertingScreen` renders the §5.3 `LowMemoryNote` as a passive status `Banner` (§5.5), never an `alertdialog`, built inline (the P4.65 `Note` precedent, no shared-primitive extraction, no later-phase edge); its §5.7 string joins the G57 `ui.ts` catalog.
  > Same commit (DoD 2): the `ConvertingScreen.tsx` slice-scope comment. It composes with P4.86: whichever lands second reconciles the emit site.
  > Tests: the conductor emits on enter and exit; the reducer flips the store both ways; the banner renders and auto-dismisses under jsdom with the G33a `vitest-axe` assertion (`vi.mock` the façade, and a new façade import joins both App mock factories in the same commit); the §0.4.2 golden and round-trip wire tests cover the variant.
  > The flip drives the existing `fn() -> Option<u64>` pool seam, never a new closure seam, on `Pool::with_degree_and_memory_for_test`, not the `Pool::new()` of the shared conductor `deps()`. A per-test static flips the probe from below to above the watermark by read count, distinct from the `MEMORY_PAUSE_MAX` exit.
  > That static is the private `pool::tests` `mem_low_then_high` pattern, copied locally or exposed through a `#[cfg(test)] pub(crate)` door beside `below_watermark_for_test`; the test never writes a value mid-pause.

## The §2.14.2 per-item staged-copy reclaim — the P4.24-raised plan gap

> One item raised at P4.24 while its §6.4.2 kill-fence test was being verified (its pre-condense note
> holds the evidence): §2.14.2 is `[DECIDED]` that the macOS staged source copy is reclaimed **per item** as
> each engine finishes, and no box owned it — neither reclaimer that exists is per-item (the §2.6.2
> run-scope cleanup fires at run end, the §2.6.3 sweep at the NEXT launch), so the §1.10
> peak-concurrent bound that rests on the per-item clause is not yet true. Appended at end-of-phase (`.88`, the
> max+1 convention); authored by the **Build-Loop**, which is why it sits in its own section
> rather than under the P4.20 one above (that section's heading, its count and its authorship line all
> scope it to the two Co-Pilot-authored `.86`/`.87` boxes).

- [ ] **P4.88** [RUST] Reclaim the macOS staged source copy PER ITEM as its engine finishes, so the §2.14.2 peak-concurrent kind-2 bound is true · §2.14.2 §2.6 §1.7 · G31
  needs: P4.25
  > Scope: §2.14.2's per-item reclaim, which no reclaimer does yet (the §2.6.2 cleanup fires at run end, the §2.6.3 sweep at the next launch): delete the macOS staged copy at the item's terminal transition (success, failure, cancel) on the §1.7 path that owns "the engine finished", never at run end and never before the engine's last read. The §2.6 reclaimers stay the crash/cancel backstop.
  > Tests (§6.4.2 level): (1) no staged copy survives its item's terminal transition on any of the three outcomes, the load-bearing leg; (2) at most `degree` staged copies exist at any instant across a batch larger than `degree`.
  > Leg (2) is vacuous until P4.86 (the sequential conductor has one item in flight), so build it after P4.86 or against a hand-driven concurrent set, never the live sequential conductor.
  > P4.25's macOS leg `on_macos_the_conductor_stages_the_source_into_the_run_scratch_before_planning` asserts the staged copy is still present after `convert_item`; it inverts here (gone once the item is terminal) under `[Test-Change: P4.88 — old-obsolete+new-correct, §2.14.2]`.
  > Same-commit spec sync, never earlier (a spec saying "per item" ahead of the code inverts DoD item 2): (a) §3.5.0 step 3's "reclaimed with the run (§2.6)" becomes "reclaimed per item as the engine finishes (§2.14.2), with the §2.6 run-scope cleanup and the §2.6.3 startup sweep as backstops".
  > (b) §2.6.2's trigger table gains the staged-copy row, whose triggers this box decides. §2.14.2 stays as it is.
  > §3.5.0 step 3 is a stale citation, not a rival clause: §2.14 is the single temp-lifecycle owner, its per-item carve-out specialises the general kind-2 rule, and §2.14.4/§1.10 compute from it. The plan follows the per-item model.

## The P4.34 link-input acquisition — the `[!extern]` prerequisite the P4.34 hard-stop surfaced

> Authored by the **Co-Pilot** on the Build-Loop's roles §4(d)+(g) hard-stop at P4.34 (the
> P4.26 → P4.38 → P4.37 → P4.35 → P4.34 DECISION-C chain); max+1 convention. Its own section because the
> P4.34 box above is what it unblocks, and the sweep section below stays the file's last.

- [!extern] **P4.89** [BUILD,CI] Establish the imgworker's minimal PNG link closure — §3.8 anchors, rows, per-triple compile legs, consumer verify, reader · §3.8 §3.7.2 §6.1.3 · G37 G71 G31
  needs: P4.56.3, P4.56.2, P4.99
  > Owner act (L(-1): `engines.lock`, `.github/**`, the allow-list) acquiring the minimal PNG closure P4.34 links, libvips curated without a PDF loader or svgload; the rest is P5's. Probes first (§3.8, §6.1.1): the per-input anchor survey, the (a′) byte-stability probe, the mingw closure build.
  > (a) Each input's §3.8 `from_source.anchor` variant (built by P4.99), by §3.8's preference order; its keys, signers and corroboration records join the allow-list in the same act.
  > (a′) The §3.8 link-input output anchor: (i) per-triple output hashes, or (ii) the consuming job's own compile from the verified `.src` entries, whose per-run record `compile-engine-asset` writes and P4.34 checks.
  > (b) Per-triple rows with `supplier` and allow-listed origins, the vendored licence texts with a regenerated `THIRD-PARTY-LICENSES.txt`/`NOTICE`, and the first `libvips.configure.flags` line (`compile-engine-asset --check` binds both); one triple per commit after local Docker validation (Linux, mingw), macOS last.
  > (c) The §6.1.1 builds and fallbacks in order (an MSYS2 row brings its LGPL corresponding source in the same act, §3.6.2); the toolchain pins with the container's signature verifiers, and the `from_source.toolchain_digest` binding.
  > (d) Linking jobs set `CONVERTIA_IMGWORKER_PREFIX` (§3.5.5) only after the consumer verify (§6.3.4); a native job per triple runs clippy and the tests with `imgworker_native`, P4.38 included. Windows uses the cross-built worker only through an (a′)(i) anchor, else the one the (c) fallback builds natively.
  > (e) Reader: the pinned libvips build's `vipsheader`/`vips` restored into `gate-tooling`, never bundled; the act names the box for each other non-bundled reader (`pdfimages` for G31 (3), `unzip` on the Windows leg).
  > The `engine-asset-populate`/`engine-asset-compile` jobs have never run live (an empty manifest gives `[]` matrices), so this act's first row is their first live run; the matrix derivation was probed locally against a non-empty inline manifest.

## The P4 owner rulings (the P4 pre-fill audit)

> Authored by the **Co-Pilot** from the test-strategy §11.4 pre-fill audit of the open P4 boxes: the
> owner rulings it found, consolidated into one act (§11.4), and the box that records them; max+1
> convention. P4.89 stays the separate precondition act of the P4.34 chain, so a ruling never waits on an
> acquisition act.

- [x] **P4.90** [DOC] Rule the owner forks the P4 pre-fill audit found — the §3.4 disposition of the §04 source codecs §3.4 leaves unclassified · §3.4.2 §3.4.3 · G7
  > Delivered: d0d54c6

- [x] **P4.91** [DOC] Record the P4.90 (A) per-codec ruling in the spec + re-point the §04 / §3.1 cross-refs at §3.4 · §3.4.2 §3.4.3 §3.4.4 · G7 G68
  needs: P4.39, P4.90
  > Delivered: d0d54c6

## The registry-driven §1.5 offer and option-declaration registry — the seam the option and lossy-note boxes read (the P4 pre-fill audit)

> Authored by the **Co-Pilot** from the test-strategy §11.4 pre-fill audit of P4.64.1: no box moves C3 and C6
> off the P3.48 walking-skeleton hard-wire, so the option gates, the lossy-note set and the P4 UX-harness leg have
> nothing to read. The option-declaration registry the same audit found missing is this seam, so both land here; max+1 convention.

- [ ] **P4.92** [RUST] Make the §1.5 offer registry-driven — per-pair `Target` data and `OptionDecl` sets in the §3.2 registry, C3 and C6 re-cut onto it, the slice hard-wire retired · §1.5 §1.6 §3.2.2 §3.2.3 §0.6 §3.1 · G29
  needs: P4.4, P2.8, P3.48, P3.49, P4.56.2
  > Gap: §1.5 and §1.6 source the offer, its default and `OptionDecl` from the §3.2 registry, but §3.2.2 `EngineCapability` holds only `{source, target, direction}`, and C3 (`ipc/planning.rs` `resolve_targets`) and C6 (`ipc/conversion.rs` `resolve_slice_target`) still read the P3.48 hard-wire `engines::slice_target` (CSV↔TSV, no options). P4.4 re-cut only the conductor.
  > (1) Per-pair §0.6 `Target` data declared through the §3.2 registry: the display `label`, the §1.5 rule-3 default flag, the ≤1 `lossy` marker, `options: Vec<OptionDecl>` and the rule-5 offered-diagonal flag. Beside it, not on `Target`, sits the per-pair §2.9.1 `LossyKind` set P4.65 renders (the §2.9.2 pair-static layer; each category's §04 *Lossy kinds* table).
  > A registry unit test asserts that a `Some` `Target.lossy` kind is a member of its pair's set. `availability` is the §3.4 posture each cell carries: a gated cell stays registered, marked unavailable, and `select()` refuses it (§3.2.3, §3.4.4a); this re-cuts the P4.4 build-time filter and its `registry.rs` docs.
  > The data sits on the capability row or in a registry-side per-pair table (a `[Build-Session-Entscheidung: P4.92]`), one production source either way, with the additive §3.2.2 sync in this commit (DoD 2). `OptionDecl` owns `String`s, so the lookup returns an owned `Vec` or a `LazyLock` slice; CSV↔TSV registers an explicit entry, so "no entry" and "explicitly empty" stay distinct.
  > (2) A pure, spawn-free per-source offer function over `engine_registry()` (`UserFacingFormat → Option<TargetOffer>`) applying §1.5 rules 1, 3 and 5 before any health marking; C3 `resolve_targets` delegates to it, then marks health. (3) C6 validates the wire `TargetId` against the same offer.
  > (4) `slice_target` and `resolve_slice_target` are deleted; `ipc/slice_round_trip.rs` and the orchestrator test mirrors move to the offer function in the same commit. (5) The registry-level offer and the `UserFacingFormat` roster are exported through the P4.60 `gate_api` façade (§0.7) for the Lane-A guards, which never read the health cache.
  > C3/C6 are the single home of the §3.4 build-time gap marking (`Unavailable` with the `PlatformUnavailable` reason). The degraded-target marking is P4.45's (it `needs:` this box): every C3 call marks a §3.1 degraded target `Unavailable { reason }` from the updatable health cache over the (2) offer, so the deferred macOS check (P4.46.1) reaches the next Confirm→Targets advance.
  > Acceptance: the CSV↔TSV offer comes out byte-identical with the P3.49 C3 unit tests unchanged (G70); P5–P7 register their per-pair data and §04 option shapes (§1.6) through this seam. In-phase consumers: P4.45, P4.60.2, P4.64, P4.64.1, P4.65, P4.79.

## The §5.2 slice-arm dispatch audit — split from P4.78

> Authored by the **Co-Pilot** from the test-strategy §11.4 pre-fill audit: split out of P4.78, whose
> DECISION-C build point (pulled in by P4.67) comes before P4.68–P4.70 are built. The audit therefore runs once
> every superseding screen has landed; max+1 convention.

- [ ] **P4.93** [TEST] Audit that every §5.2 reducer arm is still dispatched by its superseding P4 screen (no dead action after the P4.64–P4.70 supersedes) · §5.2 §5.3 · G15
  needs: P4.50, P4.64, P4.65, P4.66, P4.67, P4.68, P4.69, P4.70, P4.78
  > for every (state, Msg) arm in `src/state/machine.ts`, assert under jsdom with the IPC façade mocked that its dispatcher still fires the Msg (the P3 dead-button class): a user-action arm against the rendered P4 screen for that state, an event-sourced arm against its `src/lib/ipc/events.ts` dispatcher. The audit includes the §5.2 row-4 option-change cell P4.64's OptionsPanel introduces and the run-fault carrier after P4.50's re-cut.

## The P4.60 caged corpus-coverage gate — the owner act the P4.80 exit gate needs

> Authored by the **Co-Pilot** from the test-strategy §11.4 pre-fill audit: the caged half of P4.60,
> top-level and a precondition because P4.80 executes the bijection guard it lands; max+1 convention.

- [!extern] **P4.95** [BUILD,CI] Land the P4.60 caged corpus-coverage gate — `scripts/check-corpus-coverage.rs`, its `[[bin]]` entry, the P4.60-family G24 self-tests and the Lane-A wiring · §6.4.3a · G22 G61 G15 G24
  needs: P4.60
  > Every deliverable is caged (`scripts/check-*`, `scripts/gate-selftests/**`, `.github/**`; G71), so the Co-Pilot lands it under owner-ack as one act, the P4.56.3 shape: the bin and its `[[bin]]` entry, the G24 self-tests of P4.60 and P4.60.1/.2/.4/.5 (with the planted positive for the shared `xtask/src/lib.rs` enumeration) and the Lane-A `ci.yml` wiring.
  > It is a precondition, not a P4.81 tail, because P4.80 `needs:` it to execute the §6.4.3a guard. The same act lands the P4.60 G22 posture's gate-status/gate-planes touches and the P4.60.5 gate-status.md row with its `KAT_PENDING` map, because those postures start with its wiring.
  > It also reconciles the build-gates G61 row's bootstrap annotation and delete-a-default self-test leg to the P4.60.2 predicate, because it lands that self-test. The P4.60.3 self-test is delivered and not part of it.
  > The Loop halves (the xtask tasks, the `xtask/src/lib.rs` parsers and enumeration, the fixtures) are P4.60's; the loop skips and collects this box.

## The G17 JS advisory leg — the owner act the JS graph never had

> Authored by the **Co-Pilot** from the dependency-refresh pre-flight: build-gates G17 had promised the
> `osv-scanner`-over-`pnpm-lock.yaml` leg as "later boxes" since P0.4.1 and no box owned it — the 27 dev-tooling
> advisories that refresh closed had been invisible to every gate (test-strategy §11.4 (g), plan-lint check 33
> now refuse the shape). Caged end to end, so it landed as one owner-acked Co-Pilot act; max+1 convention.

- [x] **P4.96** [GATE,CI] Wire the G17 JS advisory leg — `osv-scanner` offline over `pnpm-lock.yaml` against the Lane-A-refreshed OSV database · §6.3.4 §6.7.1 · G17 G24
  > Delivered: ed1e95f

## The G56 transitive-action-pin half — the owner act the r6 sentence promised

> Authored by the **Co-Pilot** from the sweep behind plan-lint check 33: the G56 row has promised this
> half as "its own acquisition box" since b9b01df (the P4.28 tail) with no owner, and named pinact as
> its checker since the r6 review (99a7b52). Caged end to end, so it landed as one owner-acked Co-Pilot act;
> max+1 convention.

- [x] **P4.97** [GATE,CI] Land G56 leg (12) — the caged action-pin inventory bound to every workflow `uses:`, and the scorecard image digest pin · §6.3.4 §6.7.1 · G56 G50 G24
  > Delivered: 98373c2

## The phase-end Co-Pilot hardening sweep — the standing phase-close box

> The standing test-strategy §11 phase-close box (owner directive):
> Co-Pilot-executed — never the Build-Loop; mandate, level and evidence rules in
> [test-strategy §11](../process/test-strategy.md#11-the-phase-end-co-pilot-hardening-sweep).

- [!extern] **P4.81** [TEST] Run the phase-end Co-Pilot hardening sweep over the whole P4 delivery — adversarial re-test at the hardest technically-possible level · §6.4
  > Co-Pilot act (never the Build-Loop); procedure and entry condition: test-strategy §11; check 31 binds the phase boundary.
  > The P4 caged tails (the P4.81 list) close here: before this box flips `[x]`, the Co-Pilot lands under owner-ack the caged tail every P4 `l-neg1: sweep-tail` box names in its note, found by a grep of this file for those lines (test-strategy §11.2; `plan-lint --report owner-acts` skips `[x]` boxes); each reds nothing until it lands.
  > The P4.52 checker's outside planted positive runs it over a planted `static=` LGPL fixture that must fail.
