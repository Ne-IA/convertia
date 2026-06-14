# P2 — App Shell & Pipeline Contracts

> **The skeleton + the spine.** P2 stands up the running-app shell (window
> lifecycle, single-instance, OS-intake, store, logging) and the
> **detect → plan → convert → publish** contracts — the IPC surface, the §0.6
> domain types, the error model, the §1.1 intake state machine, and the C12
> `EngineHealth` contract — **type-shared end-to-end Rust↔TS with NO real engine
> yet**. P3 (walking skeleton) builds the first conversion *through* these
> contracts; P4 builds the runtime engine-health probe that *populates* C12.
>
> Spec homes: [00-architecture](../spec/00-architecture.md) (§0.3/§0.4/§0.6/§0.7/
> §0.9/§0.10), [01-conversion-pipeline](../spec/01-conversion-pipeline.md)
> (§1.1 intake state machine), [07-app-shell](../spec/07-app-shell.md)
> (§7.1 instance/run identity + single-instance, §7.2 startup-sequence ordering +
> C12 `EngineHealth`, §7.3 window lifecycle, §7.4 persistence, §7.5 logging,
> §7.6 no-updater, §7.7 shell-out, §7.8 OS-intake funnel + §7.8.2 negatives).
> Index: [plan/README.md](README.md). Box format: [`_format.md`](_format.md).
>
> **This is the v0 base.** The atomic `[ ]` boxes below derive exhaustively from
> the spec homes; a later adversarial review deepens, splits and completes them.
> Where a P2 box wires a gate whose *enforcement target* lands here (the conf,
> the capability file, the generated `bindings.ts`, the React shell), it is the
> **activation target** the matching P0 box's `> → activated in P1` note refers
> to (a later pass reconciles P0's `needs:` against these real box-ids).
>
> **Boundaries.** P2 owns *contracts + skeleton*, not behaviour: the C12 type is
> declared here, the **probe body is P4**; the §7.2.1 *ordering* is established
> here, the **engine presence/integrity verifier body is P4**; `fs_guard` /
> isolation / pool real bodies are **P3/P4** (P2 declares only the types the
> contract surface references). No engine spawn, no conversion, no corpus.

---

### P2.0 — Scaffolding the shell crate & frontend skeleton

- [ ] **P2.1** [RUST] Scaffold the `src-tauri` Tauri-v2 host crate + workspace `Cargo.toml` · §0.7 §0.8 · G18a
- [ ] **P2.2** [RUST] Author the logical-module tree (`ipc`/`orchestrator`/`detection`/`engines`/`fs_guard`/`run`/`outcome`/`isolation`/`pool`/`domain`/`platform`) as empty downward-only modules · §0.7
  needs: P2.1
  - [ ] **P2.2.1** [RUST] Create the tier-3 `domain` + `platform` module skeletons (depend on nothing) · §0.7
  - [ ] **P2.2.2** [RUST] Create the tier-2 `detection`/`engines`/`fs_guard`/`run`/`outcome`/`isolation` module skeletons · §0.7
  - [ ] **P2.2.3** [RUST] Create the tier-1 `orchestrator` + tier-0 `ipc` + tier-3 `pool` module skeletons · §0.7
  - [ ] **P2.2.4** [GATE] Add the downward-only module-dependency assertion (nothing below depends on anything above) · §0.7 · G9
- [ ] **P2.3** [UI] Scaffold the React 19 / TypeScript-strict / Vite / Tailwind frontend tree (`src/`, `index.html`, `vite.config.ts`) · §0.7 §0.8 · G18c
  needs: P2.1
- [ ] **P2.4** [UI] Author the `index.html` shell carrying the `x-dns-prefetch-control:off` meta + the local bundled `app.windows[].url` · §0.10 §7.6.1 · G47
  needs: P2.3
- [ ] **P2.5** [UI] Stand up the frontend store + the `strings/ui.ts` English string module skeleton · §0.8 · G57
  needs: P2.3

### P2.1 — Tauri config, capabilities & CSP (the WebView security boundary)

- [ ] **P2.6** [BUILD] Author `tauri.conf.json` — bundle id `dev.ne-ia.convertia`, externalBin slots, minimum-OS floor · §0.3.1 §0.7 §3.3
  needs: P2.1
- [ ] **P2.7** [BUILD] Encode the locked §0.10 CSP object into `tauri.conf.json` `app.security.csp` (per-directive golden) · §0.10 · G47
  needs: P2.6
- [ ] **P2.8** [BUILD] Author `capabilities/main.json` — `core:default` + `log`/`store` plugin perms, NO `dialog`/`opener`/`fs`/`shell:allow-execute` grant · §0.10 §3.3.3 · G47
  needs: P2.6
- [ ] **P2.9** [BUILD] Assert the §7.6.1 updater-absence + asset-protocol/hardening keys in the conf (`withGlobalTauri`/`dangerousDisableAssetCspModification`/`createUpdaterArtifacts`/`assetProtocol.enable`/release-`devtools` all absent/false) · §0.10 §7.6.1 · G47
  needs: P2.7, P2.8
- [ ] **P2.10** [BUILD] Assert no `plugins.deep-link` block + no custom URL-scheme registration under `src-tauri/` (the §7.8.2 no-URL-scheme negative) · §7.8.2 §0.10 · G47
  needs: P2.6

### P2.2 — Pinned plugins & the Tauri Builder chain (registration order)

- [ ] **P2.11** [RUST] Pin + register `tauri-plugin-single-instance` FIRST in the Builder (wins before any window) · §7.1.1 §0.8
  needs: P2.2, P2.6
- [ ] **P2.12** [RUST] Register the remaining §0.8 plugins after single-instance in the one Builder chain (dialog → opener → store → log) · §7.1.1 §0.8
  needs: P2.11
  - [ ] **P2.12.1** [RUST] Register `tauri-plugin-dialog` (`DialogExt` for the C2a/C2b Rust-side pickers) · §7.1.1 §0.8
  - [ ] **P2.12.2** [RUST] Register `tauri-plugin-opener` (`OpenerExt` for the C9/C10 Rust-side shell-out) · §7.7.1 §0.8
  - [ ] **P2.12.3** [RUST] Register `tauri-plugin-store` (the `settings.json` blob) · §7.4.2 §0.8
  - [ ] **P2.12.4** [RUST] Register `tauri-plugin-log` (rotating local log) · §7.5.1 §0.8
- [ ] **P2.13** [GATE] Assert `tauri-plugin-updater` is absent from `Cargo.toml`/the Builder (the no-phone-home implementation) · §7.6.1 · G47

### P2.3 — Rust↔TS type-sharing (tauri-specta + specta codegen)

- [ ] **P2.14** [RUST] Wire `tauri-specta` + `specta` into the Builder (`collect_commands!` / `collect_events!`) + enable the `specta` feature on the `tauri` crate · §0.4.5 §0.8
  needs: P2.2
- [ ] **P2.15** [RUST] Author the `collect_types![]` registry (every wire type derives `specta::Type`) · §0.4.5 §0.6
  needs: P2.14
- [ ] **P2.16** [BUILD] Wire the `bindings.ts` generation step (build hook / `cargo` step) emitting to `src/lib/ipc/bindings.ts` · §0.4.5
  needs: P2.14, P2.3
- [ ] **P2.17** [GATE] Fill the concrete codegen command + generated-path into the §0.4.5 bindings-drift check (regenerate + `git diff --exit-code` + parsed non-empty sanity) · §0.4.5 · G19
  needs: P2.16
- [ ] **P2.18** [UI] Establish `src/lib/ipc/bindings.ts` as the SOLE IPC door — no raw `invoke` elsewhere · §0.4.5 §0.7
  needs: P2.16

### P2.4 — Domain model contracts (§0.6 shared vocabulary)

- [ ] **P2.19** [RUST] Author the identity types — `InstanceId`/`RunId`/`CollectedSetId`/`ItemId`/`JobId`/`CollectingId` · §0.6 §7.1.2
  needs: P2.2.1, P2.15
- [ ] **P2.20** [RUST] Author `IntakeOrigin` { Drop, Picker, LaunchArg, SecondInstance } · §0.6 §7.8
  needs: P2.19
- [ ] **P2.21** [RUST] Author `UserFacingFormat` (the single grouping key — the full SSOT *What It Converts* set) · §0.6 §1.3
  needs: P2.19
- [ ] **P2.22** [RUST] Author `DroppedItem` (raw/resolved path, size, `DetectionOutcome` ref) + the display-only `raw_path` scope note · §0.6 §1.2
  needs: P2.21
- [ ] **P2.23** [RUST] Author `SkippedItem` + `SkipReason` { UnsupportedType, Uncertain, Empty, Unreadable } (id-disjoint over the single id space) · §0.6 §1.3
  needs: P2.22
- [ ] **P2.24** [RUST] Author the `CollectedSet` enum — `Single`/`Mixed`/`Unsupported`/`Uncertain`/`Empty` (the C1/C2a return + unified §1.4 confirm-summary fields) · §0.6 §1.1 §1.4
  needs: P2.23
- [ ] **P2.25** [RUST] Author the wire-DTO types — `PickKind`/`OpenKind`/`IntakePayload`/`ScanProgress` · §0.6 §0.4.1 §0.4.2
  needs: P2.20
- [ ] **P2.26** [RUST] Author the target/option types — `TargetId`/`FormatId`/`CrossCatOp`/`Availability`/`Target`/`TargetOffer`/`OptionValues` · §0.6 §1.5 §1.6
  needs: P2.21
- [ ] **P2.27** [RUST] Author the destination/plan types — `DestinationChoice`/`OutputPlan`/`DivertReason` (directory-based, no pre-baked `final_path`) · §0.6 §2.7 §2.14.1
  needs: P2.24
- [ ] **P2.28** [RUST] Author `Batch`/`ConversionJob`/`JobState`/`JobStage` · §0.6 §1.9
  needs: P2.26, P2.27
- [ ] **P2.29** [RUST] Author the command-return DTOs — `OutputPlanPreview`/`RerunPrompt`/`RerunDecision`/`PreflightVerdict`/`DestinationResolved` · §0.6 §1.8 §1.10 §2.5
  needs: P2.28
- [ ] **P2.30** [RUST] Author the result types — `RunResult`/`ItemResult`/`Totals`/`CleanupResidue`/`ItemOutcome` · §0.6 §1.12 §2.6
  needs: P2.28
- [ ] **P2.31** [RUST] Author the engine-descriptor seam types — `EngineId`/`EngineDescriptor`/`EngineKind` (non-trait `FFprobe`/`ImageMagick` note) · §0.6 §3.2
  needs: P2.21
- [ ] **P2.32** [TEST] Property-test the §0.6 normative invariants (one-Target-per-Batch, `count == items.len()`, frozen `items`, stable `ItemId`, same-volume publish-temp) · §0.6 · G22 G23
  needs: P2.30, P2.31

### P2.5 — Detection-outcome contract (the §1.2 result type)

- [ ] **P2.33** [RUST] Author `DetectionOutcome` (`Recognized`/`UnsupportedType`/`Uncertain`/`Empty`/`Unreadable`) + `Confidence` { High, Low } as the single canonical detection result · §1.2 §0.6
  needs: P2.21
- [ ] **P2.34** [RUST] Author the `DetectionOutcome → SkipReason` projection (ineligible-outcome → skip) · §1.2 §1.3 §0.6
  needs: P2.33, P2.23
- [ ] **P2.35** [RUST] Author `ReadFailure`/`EmptyReport` contract types feeding the `Empty { skipped }` reason tally · §1.2 §0.6
  needs: P2.33

### P2.6 — Error & outcome model contract (the §2.8 wire mirror)

- [ ] **P2.36** [RUST] Author `ErrorKind` as a `type` alias of (or drift-locked mirror of) the §2.8 `ConversionErrorKind` in `crate::outcome` · §0.4.3 §2.8.1
  needs: P2.2.2, P2.15
  - [ ] **P2.36.1** [RUST] Enumerate the item-level `ErrorKind` variants byte-identical to the §2.8 catalog · §0.4.3 §2.8.1
  - [ ] **P2.36.2** [RUST] Add the run/app-level kinds (`EngineMissing`/`WebviewFault`/`BundleDamaged`) + the mirror-only `MixedDrop` entry · §0.4.3 §2.13.1
  - [ ] **P2.36.3** [TEST] Lock anti-drift — `static_assertions` variant-count + variant-name round-trip `#[test]` · §0.4.3 §2.8.2 · G23
- [ ] **P2.37** [RUST] Author the `IpcError` shape (`kind`/`message`/`path`/`residue`, derives `specta::Type`, in `collect_types![]`) · §0.4.3 §2.8
  needs: P2.36
- [ ] **P2.38** [RUST] Author `OutcomeMsg` + the `SkipReason → ErrorKind` forward (one-way, non-inverted) projection helper · §0.6 §2.8.2 §1.12
  needs: P2.36, P2.34

### P2.7 — IPC command surface (C1–C13 contracts)

- [ ] **P2.39** [RUST] Wire the `invoke_handler` + register C1–C13 on the Builder (handlers thin, delegate to orchestrator) · §0.4.0 §0.7
  needs: P2.2.3, P2.15
- [ ] **P2.40** [RUST] Author the C1 `ingest_paths` contract — frozen-set builder, `origin`, `collectingId`, `drainPending`, optional `onScan` Channel · §0.4.1 §1.1 §2.4
  needs: P2.39, P2.24
- [ ] **P2.41** [RUST] Author the C2a `pick_for_intake` contract — Rust-side `DialogExt` picker funnelling into the C1 freeze, no raw path to WebView · §0.4.1 §1.1 §5.4
  needs: P2.40, P2.12.1
- [ ] **P2.42** [RUST] Author the C2b `pick_destination` contract — Rust-side folder picker returning the chosen `PathBuf` (the one write-path that transits the WebView) · §0.4.1 §0.10
  needs: P2.39, P2.12.1
- [ ] **P2.43** [RUST] Author the C3 `get_targets` contract — pure function of detection → `TargetOffer` (one pre-highlighted default, no spawn) · §0.4.1 §1.5
  needs: P2.39, P2.26
- [ ] **P2.44** [RUST] Author the C4 `plan_output` contract — `OutputPlanPreview` (resolved dest, divert preview, §2.5 rerun, §1.10 preflight) · §0.4.1 §1.8 §2.5 §1.10
  needs: P2.39, P2.29
- [ ] **P2.45** [RUST] Author the C5 `set_destination` contract — `DestinationResolved` (re-eval preflight, carry rerun through unchanged) · §0.4.1 §1.8 §2.14.4
  needs: P2.44
- [ ] **P2.46** [RUST] Encode the C4/C5 asymmetry as an enforced orchestrator lifecycle rule (C4 re-callable; C5 owns destination; C4 never overrides C5) · §0.4.1
  needs: P2.45
- [ ] **P2.47** [RUST] Author the C6 `start_conversion` contract — mint `RunId`, enqueue, return immediately, stream over `onProgress` Channel; `destination` authoritative · §0.4.1 §1.9 §7.1.2
  needs: P2.39, P2.28
- [ ] **P2.48** [RUST] Author the C7 `cancel_run` contract — trip the `RunId` token (keep finished, discard in-progress) · §0.4.1 §0.4.4 §1.7
  needs: P2.47
- [ ] **P2.49** [RUST] Author the C8 `get_run_summary` contract — idempotent re-fetch of the retained `RunResult` · §0.4.1 §0.4.4 §1.12
  needs: P2.47, P2.30
- [ ] **P2.50** [RUST] Author the C9 `open_path` contract — Rust-side `OpenerExt` reveal/open with the §7.7.3 `RunResult` membership gate · §0.4.1 §7.7.1 §7.7.3
  needs: P2.39, P2.12.2
- [ ] **P2.51** [RUST] Author the C10 `open_project_page` contract — Rust handler opens a compiled-in canonical URL constant (no WebView URL arg) · §0.4.1 §7.6.2 §7.7.2
  needs: P2.39, P2.12.2
- [ ] **P2.52** [RUST] Author the C11 `get_app_info` contract — `AppInfo` (version, build id, platform, third-party-notice) · §0.4.1 §7.2.3
  needs: P2.39
- [ ] **P2.53** [RUST] Author the C13 `cancel_ingest` contract — trip the `CollectingId` ingest-scoped token · §0.4.1 §1.1
  needs: P2.40
- [ ] **P2.54** [GATE] Assert the C1–C13 IPC-surface set is complete + drift-free (no extra/missing command; plan-lint check 9/12 target) · §0.4.1 · G23
  needs: P2.53, P2.51, P2.52, P2.49, P2.50

### P2.8 — IPC event / Channel surface (the three `app://` events + telemetry Channels)

- [ ] **P2.55** [RUST] Author the `ConversionEvent` Channel enum + its payload structs (`RunStarted`/`ItemStarted`/`ItemProgress`/`ItemFinished`/`BatchProgress`/`RunFinished`) · §0.4.2 §1.11
  needs: P2.30, P2.15
  - [ ] **P2.55.1** [RUST] Encode the `RunStarted.totalItems` = queued-eligible-only denominator rule · §0.4.2 §1.3
  - [ ] **P2.55.2** [RUST] Encode the conservative `willReencode` worst-case `bool` (always definite, never omitted) · §0.4.2 §2.9.2
  - [ ] **P2.55.3** [RUST] Encode the `BatchProgress.total` == `RunStarted.totalItems` (queued-only) invariant · §0.4.2 §1.11
  - [ ] **P2.55.4** [RUST] Encode the pre-flight-skip emission policy (no live `ItemFinished{Skipped}`; terminal projection only) · §0.4.2 §1.9 §1.12
- [ ] **P2.56** [RUST] Author the `ScanProgress { scanned }` intake-telemetry Channel payload (throttled, dies with C1) · §0.4.2 §1.1
  needs: P2.40
- [ ] **P2.57** [RUST] Author the three `app://` events — `app://fault` (`AppFault`), `app://intake` (`IntakePayload`), `app://close-requested` (`()`) · §0.4.2 §2.13 §7.8.1 §7.3.2
  needs: P2.25, P2.15
- [ ] **P2.58** [RUST] Encode the `app://intake` IDLE-path-only rule (busy refuses + drops core-side, never emits ingestable paths) · §0.4.2 §7.8.1
  needs: P2.57
- [ ] **P2.59** [GATE] Assert the closed three-event invariant — exactly `{fault, intake, close-requested}`, no fourth `app://` event · §0.4.2 · G23
  needs: P2.57

### P2.9 — Registries & cancellation lifecycle (the orchestrator state)

- [ ] **P2.60** [RUST] Build the `RunId` → `CancellationToken` run registry (created in C6, tripped by C7, dropped on `RunFinished`) · §0.4.4 §1.7
  needs: P2.47, P2.48
- [ ] **P2.61** [RUST] Build the `RunResult` retention (process-local, until next run / app exit) for C8 re-serve · §0.4.4 §1.12 §7.4
  needs: P2.49, P2.60
- [ ] **P2.62** [RUST] Build the `CollectedSetId` → `FrozenCollectedSet` registry (created on C1/C2a freeze; resolved by C3/C4/C5/C6; evicted on run-start/supersede/exit) · §0.4.4 §2.4
  needs: P2.40, P2.24
- [ ] **P2.63** [RUST] Build the `CollectingId` → ingest-scoped token registry (frontend-generated id, registered at handler entry, dropped on EVERY exit branch) · §0.4.4 §1.1
  needs: P2.53, P2.41
- [ ] **P2.64** [DOC] Record the macOS reload-during-run non-recovery scope (`[DECIDED]` post-terminal re-serve only) · §0.4.4

### P2.10 — Instance & run identity + single-instance policy (§7.1)

- [ ] **P2.65** [RUST] Establish the `InstanceId` app-managed singleton (random v4, never persisted/networked) · §7.1.2 §2.11
  needs: P2.19, P2.11
- [ ] **P2.66** [RUST] Fix the `RunId` mint point — at C6 accept (NOT at the §2.4 freeze; the freeze yields `CollectedSetId`) · §7.1.2 §0.4.4
  needs: P2.47, P2.65
- [ ] **P2.67** [RUST] Encode the `<InstanceId>.<pid>` scratch-root naming + `run-<RunId>/` subdir identity (PID = label, not liveness) · §7.1.2 §2.14
  needs: P2.65
- [ ] **P2.68** [DOC] Record the advisory-lock-is-authoritative liveness predicate (PID never used as the test; §2.6.3 owns the lock) · §7.1.2 §2.6.3
  needs: P2.67
- [ ] **P2.69** [RUST] Wire the single-instance callback — re-focus the "main" window + forward argv via `forward_launch_argv`, origin `SecondInstance` · §7.1.1 §7.8.1
  needs: P2.11, P2.70
- [ ] **P2.70** [RUST] Encode the per-OS-user (not machine-global) single-instance lock scope · §7.1.1
  needs: P2.11
- [ ] **P2.71** [DOC] Record the macOS edge cases — least-mature single-instance leg (§6.6 verification item) + the unsigned two-copies accepted-limitation · §7.1.1

### P2.11 — OS-intake funnel (§7.8.1) — the launch/Open-with state machine

- [ ] **P2.72** [RUST] Build the single `forward_launch_intake(app, paths, origin)` funnel (every launch-time path source routes here) · §7.8.1 §1.1
  needs: P2.65, P2.57
- [ ] **P2.73** [RUST] Enforce the §7.1.1 PRIMARY refuse-busy gate inside the funnel (mid-run: DROP paths, no emit, no buffer) · §7.8.1 §7.1.1 §2.4
  needs: P2.72, P2.58
- [ ] **P2.74** [RUST] Wire the macOS `RunEvent::Opened { urls }` handler — `Url::to_file_path()` → funnel, origin LaunchArg/SecondInstance by readiness · §7.8.1 §1.1
  needs: P2.72
  - [ ] **P2.74.1** [DOC] Record the macOS-only Tauri-v2 fact (`RunEvent::Opened` never fires on Win/Linux; registered unconditionally for code simplicity) · §7.8.1
  - [ ] **P2.74.2** [DOC] Record the NOT-`tauri-plugin-deep-link`/`on_open_url` decision (custom-scheme intent, never the open-documents AppleEvent) · §7.8.1 §7.8.2
- [ ] **P2.75** [RUST] Wire the Windows-argv (`std::env::args_os` at first launch) + Linux `%F`/`%U` argv intake into `forward_launch_argv` · §7.8.1 §1.1
  needs: P2.72
- [ ] **P2.76** [RUST] Build the `State<PendingIntake>` first-launch buffer (stash paths+origin when frontend not ready) · §7.8.1
  needs: P2.72
- [ ] **P2.77** [RUST] Wire the ready-flag branch — emit `app://intake` if ready, else `buffer_pending_intake` · §7.8.1 §0.4.2
  needs: P2.76, P2.58
- [ ] **P2.78** [RUST] Build the `drainPending` drain path — C1 `paths: []` + `drainPending: true` consumes `PendingIntake` once (stored origin), returns its `CollectedSet` · §7.8.1 §0.4.1
  needs: P2.77, P2.40
- [ ] **P2.79** [UI] Wire the root-shell-mount drain trigger (always re-call C1 with `drainPending: true` after listener registration, closing the listener race) · §7.8.1 §5.2
  needs: P2.78, P2.18

### P2.12 — Intake freeze state machine (§1.1) — idle-vs-in-flight gating

- [ ] **P2.80** [RUST] Implement the §1.1 single `ingest(paths, origin) -> CollectedSet` funnel (the exhaustive freeze point for all five entry points) · §1.1 §2.4
  needs: P2.40, P2.24
- [ ] **P2.81** [RUST] Set the per-entry-point `origin` stamping (C1 from request; C2a handler stamps `Picker`; launch hooks stamp `LaunchArg`/`SecondInstance`) · §1.1 §0.6
  needs: P2.80, P2.41
- [ ] **P2.82** [RUST] Implement Rust-side folder recursion (`walkdir`, depth-first, symlinked dirs not traversed) · §1.1 §0.8
  needs: P2.80
- [ ] **P2.83** [RUST] Encode the fixed hidden/system-file ignore constant (dotfiles, `.DS_Store`/`Thumbs.db`/`desktop.ini`, Win hidden/system attrs) · §1.1
  needs: P2.82
- [ ] **P2.84** [RUST] Retain the dropped root(s) on the frozen set (for §2.7 subtree re-creation + open-folder common root) · §1.1 §2.7
  needs: P2.82
- [ ] **P2.85** [RUST] Implement the mid-walk per-item-failure-does-not-abort rule (per-item `Unreadable`/`Empty` → `SkippedItem`, walk continues) · §1.1 §1.2 §1.9
  needs: P2.82, P2.34
- [ ] **P2.86** [RUST] Encode the fatal-walk-root-error stop (dropped root itself unreadable/gone) distinct from per-item skip · §1.1
  needs: P2.85
- [ ] **P2.87** [RUST] Implement cooperative ingest cancellation — poll the `CollectingId` token in the walk/detect loop, discard partial unfrozen set (no cleanup obligation) · §1.1 §0.4.1
  needs: P2.82, P2.63
- [ ] **P2.88** [RUST] Implement the C2a native-dialog-phase rules — async/`spawn_blocking` picker (never `blocking_pick_file` on a Tokio worker), token registered before dialog opens · §1.1 §0.4.1
  needs: P2.87, P2.41
- [ ] **P2.89** [RUST] Implement the C2a token-drop-on-EVERY-exit-branch rule (cancelled-dialog → `Empty`, C13-tripped → `Empty`, normal walk-completes) · §1.1 §0.4.4
  needs: P2.88
- [ ] **P2.90** [RUST] Implement the freeze idle-vs-in-flight gating — IDLE starts a new frozen set; in-flight refuses-busy (never mutate/merge a frozen set) · §1.1 §7.1.1 §2.4
  needs: P2.80, P2.73
- [ ] **P2.91** [RUST] Encode the zero-byte/unreadable-at-intake classification — intake-time `Empty`/`Unreadable` = Skipped (pre-flight, never queued); turn-time = Failed (mid-run) · §1.1 §1.2 §0.6
  needs: P2.85, P2.23
- [ ] **P2.92** [RUST] Apply resolved-identity de-dup as the frozen set is built (a file reached via two paths is one member) · §1.1 §2.3
  needs: P2.93
- [ ] **P2.93** [RUST] Assign `ItemId` at the freeze over the single id space (eligible + skipped, never re-indexed from 0) · §1.1 §0.6
  needs: P2.80, P2.94
- [ ] **P2.94** [RUST] Author the `crate::fs_guard::resolve_identity` interface stub the freeze de-dup calls (real body P3) · §1.1 §2.3
  needs: P2.2.2

### P2.13 — Window & app lifecycle (§7.3)

- [ ] **P2.95** [RUST] Create the single "main" window at startup (no tray, no secondary windows, default size each launch) · §7.3.1 §7.4.1
  needs: P2.6, P2.96
- [ ] **P2.96** [DOC] Record the no-tray / no-background-agent / closing-quits posture (portable, no system pollution) · §7.3.1
- [ ] **P2.97** [RUST] Wire `Builder::on_window_event` — v2 two-arg `(&Window, &WindowEvent)` `CloseRequested` handler · §7.3.2
  needs: P2.95
- [ ] **P2.98** [RUST] Implement the close-requested decision in Rust — `converter_is_busy` → `api.prevent_close()` + emit `app://close-requested` (`serde_json::Value::Null` payload) · §7.3.2 §7.3.3
  needs: P2.97, P2.57
- [ ] **P2.99** [RUST] Wire the `App::run` `RunEvent::ExitRequested` (last `prevent_exit` chance) + `RunEvent::Exit` (flush logs, best-effort scratch cleanup) handlers · §7.3.2 §2.6
  needs: P2.95
- [ ] **P2.100** [RUST] Route `RunEvent::Opened` through the funnel inside the `App::run` closure (the macOS Open-with hook, §7.8.1 refuse-busy enforced) · §7.3.2 §7.8.1
  needs: P2.99, P2.74
- [ ] **P2.101** [RUST] Establish the quit-while-converting contract — confirm → cancel-in-flight (§1.7) + §2.6 cleanup + exit = same path as in-UI Cancel; idle quits immediately · §7.3.3 §1.7 §2.6
  needs: P2.98, P2.60
- [ ] **P2.102** [DOC] Record the no-persistent-queue / no-resume-across-launches `[DECIDED]` (in-memory queue only; re-drop on next launch) · §7.3.4 §7.4

### P2.14 — Persistence (§7.4) — the 3-key prefs blob

- [ ] **P2.103** [RUST] Implement the 3-key `settings.json` prefs blob via `tauri-plugin-store` (`theme`/`lastDestinationMode`/`verboseLog`, defaults) · §7.4.1 §7.4.2
  needs: P2.12.3
  - [ ] **P2.103.1** [RUST] Resolve the per-OS config-dir location via `app.path().app_config_dir()` (`dev.ne-ia.convertia/settings.json`) · §7.4.2
  - [ ] **P2.103.2** [RUST] Implement best-effort-never-load-bearing tolerance (unreadable/corrupt → log + run with defaults, never block a conversion) · §7.4.2
- [ ] **P2.104** [RUST] Encode the single-store-name (T2c) convention — only `Store.load('settings.json')`, one call site · §7.4.2 §0.10 · G29
  needs: P2.103
- [ ] **P2.105** [DOC] Record the explicit persistence negatives (no history / recent-files / presets / window-geometry / resumable queue) · §7.4.1 §7.3.4
- [ ] **P2.106** [RUST] Encode the `lastDestinationMode` re-validate-as-writable-at-use-time rule (a hint, never a guarantee; §2.7 fallback applies) · §7.4.1 §2.7
  needs: P2.103

### P2.15 — Logging & diagnostics (§7.5) — local-only, no telemetry

- [ ] **P2.107** [RUST] Configure `tauri-plugin-log` — rotating file + dev stderr, default level `warn`/`info`, no network sink · §7.5.1 §7.5.2
  needs: P2.12.4
- [ ] **P2.108** [RUST] Resolve the per-OS log-dir via `app.path().app_log_dir()` + the Linux config-dir deviation note · §7.5.2
  needs: P2.107
- [ ] **P2.109** [RUST] Configure rotation — `max_file_size(5_000_000)` + `RotationStrategy::KeepOne` (≈1× footprint, source-verified vs the pinned version) · §7.5.2
  needs: P2.107
- [ ] **P2.110** [DOC] Record the `KeepOne == fs::remove_file` ≈1× footprint audit + the `[DEFER: verify-on-bump]` re-check trigger against the pinned commit · §7.5.2
  needs: P2.109
- [ ] **P2.111** [RUST] Implement the redaction stance — NEVER log file contents/bytes/full-paths at default level; structural facts + basename only · §7.5.3 §2.11 · G29
  needs: P2.107
- [ ] **P2.112** [RUST] Implement the verbose-mode opt-in (full paths + exact engine argv) read-once-at-startup (`verboseLog` + `--verbose`), effective next launch · §7.5.3 §3.5
  needs: P2.111, P2.103
- [ ] **P2.113** [RUST] Add the JS-bridge so frontend errors land in the same log file · §7.5.1
  needs: P2.107, P2.18
- [ ] **P2.114** [DOC] Record the no-automatic-upload-ever stance (the §6.8 bug-report flow attaches the log manually) · §7.5.3 §2.11

### P2.16 — Update posture (§7.6) — no auto-updater (defense in depth)

- [ ] **P2.115** [DOC] Record the no-startup/background version-check assertion (zero network calls at startup) · §7.6.1 §7.2.2
- [ ] **P2.116** [RUST] Encode the version-display source for About (`app.package_info().version` / `CARGO_PKG_VERSION`) feeding C11 · §7.6.2 §7.2.3
  needs: P2.52
- [ ] **P2.117** [DOC] Record the future opt-in update-check parked decision (`updateCheckOptIn` not present in v1) · §7.6.3 §7.4

### P2.17 — OS shell-out (§7.7) — open-folder / open-file / open-url

- [ ] **P2.118** [RUST] Map all three `OpenKind` variants to concrete `OpenerExt` calls (`RevealInFolder`→`reveal_item_in_dir`, `Folder`→`open_path`(dir), `File`→`open_path`) · §7.7.1 §0.6
  needs: P2.50
- [ ] **P2.119** [RUST] Implement the Rust-side `RunResult`-membership gate (no static opener scope) — reveal/open-path validated against recorded outputs + roots before `OpenerExt` · §7.7.2 §7.7.3
  needs: P2.118, P2.61
- [ ] **P2.120** [RUST] Implement the two-membership-rule split — file-launch admits only output FILES; folder-browse admits run ROOTS (`common_root` + `divert_root`) · §7.7.3 §0.6
  needs: P2.119
- [ ] **P2.121** [RUST] Implement the split-output two-open-folder-targets contract (`common_root` + `Some(divert_root)` both in the membership set) · §7.7.1 §7.7.3
  needs: P2.120, P2.30
- [ ] **P2.122** [RUST] Implement C10 as a compiled-in canonical URL constant via `OpenerExt::open_url` (no URL-injection surface) · §7.7.2 §7.6.2
  needs: P2.51
- [ ] **P2.123** [DOC] Record the open-file safety posture (no auto-open, reveal-in-folder is the preferred default, OS default app on explicit click only) · §7.7.3

### P2.18 — Startup sequence ordering (§7.2.1) — the app-shell spine

- [ ] **P2.124** [RUST] Establish the §7.2.1 ordered startup sequence as the shell spine (steps 1–8, window shown only after steps 3–5 succeed) · §7.2.1 §2.13
  needs: P2.11, P2.95, P2.99
  - [ ] **P2.124.1** [RUST] Step 1 — single-instance guard registered first (second launch hands off + exits) · §7.2.1 §7.1.1
  - [ ] **P2.124.2** [RUST] Step 2 — establish `InstanceId` + resolve base paths (config/scratch/log) via `app.path()`, no dir created yet · §7.2.1 §7.1.2
  - [ ] **P2.124.3** [RUST] Step 3 — engine presence+integrity verification SLOT (app-level fault on failure; verifier body P4) · §7.2.1 §7.2.3
  - [ ] **P2.124.4** [RUST] Step 4 — executable-permission setup SLOT on the engine binaries (portable build; body P4) · §7.2.1 §7.2.4
  - [ ] **P2.124.5** [RUST] Step 5 — scratch + log dir creation with the per-instance root + orphan-reclaim SLOT (mechanism §2.6, body P3/P4) · §7.2.1 §7.2.5 §2.6
  - [ ] **P2.124.6** [RUST] Step 6 — WebView window create + frontend load (WebView-init fault where the core can observe it) · §7.2.1 §0.3.1
  - [ ] **P2.124.7** [RUST] Step 7 — process launch-time intake feed (argv / PendingIntake drain → §1.1) · §7.2.1 §7.8.1
  - [ ] **P2.124.8** [UI] Step 8 — hand to the UI empty/idle state · §7.2.1 §5.2
- [ ] **P2.125** [RUST] Implement the §7.2.2 offline assertion at startup (the shell adds ZERO startup network activity) · §7.2.2 §2.11
  needs: P2.124
- [ ] **P2.126** [DOC] Record the Windows-WebView2-absent honest-exception (loader fails before the core; download-page note, no in-app dialog) · §7.2.1 §0.3.1
- [ ] **P2.127** [RUST] Surface a missing/old/broken macOS-WKWebView / Linux-WebKitGTK init as a §2.13/§7.2 startup fault (where the core observes it) · §7.2.1 §0.3.1 §2.13
  needs: P2.124.6, P2.57

### P2.19 — The C12 `EngineHealth` contract (probe body is P4)

- [ ] **P2.128** [RUST] Author the `EngineStatus` type (`id`/`present`/`integrity_ok`/`runnable: Option<bool>`) · §7.2.3 §0.6
  needs: P2.31, P2.15
- [ ] **P2.129** [RUST] Author the `EngineHealth` type (`engines`/`unavailable_targets`/`all_critical_ok`) — one row per registry-eligible engine · §7.2.3 §0.6
  needs: P2.128
  - [ ] **P2.129.1** [DOC] Record the non-trait-binary roll-up rule (`FFprobe`→FFmpeg, `ImageMagick`→`ImageCore`; no standalone `EngineStatus` row) · §7.2.3 §0.6
  - [ ] **P2.129.2** [DOC] Record the `NativeCsvTsv` synthesized always-available `EngineStatus` (appended after the loop, never from it) · §7.2.3 §3.5.6
- [ ] **P2.130** [RUST] Author the `AppInfo` type (C11 return) — version/build_id/platform/third_party_notice · §7.2.3 §0.6
  needs: P2.128
- [ ] **P2.131** [RUST] Wire C12 `get_engine_health` to return the cached `EngineHealth` (the cache is populated by the P4 probe; contract type-shared now) · §0.4.1 §7.2.3
  needs: P2.129, P2.39
- [ ] **P2.132** [UI] Consume `EngineHealth` in the UI to disable/omit unavailable targets (the §5.2 surface) · §7.2.3 §5.2
  needs: P2.131, P2.18

### P2.20 — §7.8.2 explicit negatives (DoD gate 20)

- [ ] **P2.133** [DOC] Record the no-file-association / no-default-handler-claim negative (no `.heic`/`.docx` handler registration) · §7.8.2
- [ ] **P2.134** [DOC] Record the no-URL-scheme / no-deep-link negative (no `convertia://`, no deep-link plugin) · §7.8.2
- [ ] **P2.135** [DOC] Record the no-drag-out / no-clipboard-export negative (parked under Future Ideas; WebView cannot originate a real path drag) · §7.8.2
- [ ] **P2.136** [DOC] Record the no-service / no-login-item / no-shell-extension negative (no Explorer/Quick-Action integration) · §7.8.2
- [ ] **P2.137** [GATE] Assert the §7.8.2 negatives structurally (no deep-link block, no URL-scheme registration under `src-tauri/`) — the DoD-gate-20 enforcement · §7.8.2 §0.10 · G47
  needs: P2.10, P2.134

### P2.21 — Shell-level a11y, English-only & UI-async contracts

- [ ] **P2.138** [UI] Wire the frontend async model to the generated `commands.*` / `ConversionEvent` Channel + the three `app://` listeners (§5.8) · §5.8 §0.4.2
  needs: P2.18, P2.55, P2.57
- [ ] **P2.139** [UI] Wire the native drag-drop affordance (hover/visual only; paths arrive over the native event → C1, never the DOM drop) · §5.4 §0.4.0
  needs: P2.138, P2.40
- [ ] **P2.140** [UI] Establish the app-chrome a11y baseline (ARIA roles/focus order on the shell — the per-push `vitest-axe` target) · §5.5 · G33a
  needs: P2.138
- [ ] **P2.141** [UI] Enforce English-only / string-ownership on the shell (every user-facing literal in `strings/ui.ts`, no i18n-runtime import) · §5.5 · G57
  needs: P2.5, P2.138
- [ ] **P2.142** [UI] Wire the backend-disconnect / mid-run IPC-drop handling to `AppFault` (the §5.8 app-fault surface) · §5.8 §2.13
  needs: P2.138, P2.57
