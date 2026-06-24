# P2 — App Shell & Pipeline Contracts

> **The spine + the contracts (on top of the P1 scaffold).** P2 stands up the
> running-app **behaviour** the P1 shell can't carry — the window/quit lifecycle,
> single-instance + run identity, the §7.8 OS-intake funnel, persistence, logging —
> and the **detect → plan → convert → publish** contracts: the C1–C13 IPC surface,
> the §0.6 domain types, the error model, the §1.1 intake state machine, the §7.2.1
> ordered startup-sequence spine, and the C12 `EngineHealth` contract — **type-shared
> end-to-end Rust↔TS with NO real engine yet**. P3 (walking skeleton) builds the
> first conversion *through* these contracts; P4 builds the runtime engine-health
> probe that *populates* C12.
>
> Spec homes: [00-architecture](../spec/00-architecture.md) (§0.3/§0.4/§0.6/§0.7/
> §0.9/§0.10), [01-conversion-pipeline](../spec/01-conversion-pipeline.md)
> (§1.1 intake state machine, §1.11 IPC-responsiveness), [07-app-shell](../spec/07-app-shell.md)
> (§7.1 instance/run identity + single-instance, §7.2 startup-sequence ordering +
> C12 `EngineHealth`, §7.3 window lifecycle, §7.4 persistence, §7.5 logging,
> §7.6 no-updater, §7.7 shell-out, §7.8 OS-intake funnel + §7.8.2 negatives).
> Index: [plan/README.md](README.md). Box format: [`_format.md`](_format.md).
>
> **This is the v0 base.** The atomic `[ ]` boxes below derive exhaustively from
> the spec homes; a later adversarial review deepens, splits and completes them.
>
> **Boundaries (read against P1).** P1 already **scaffolded everything structural** —
> the workspace `Cargo.toml` + `src-tauri` crate (P1.6), the §0.7 module tree as
> downward-only shells incl. the G9 assertion (P1.11), the React/TS/Vite/Tailwind
> frontend (P1.29–P1.31), `index.html` + the `x-dns-prefetch-control:off` meta
> (P1.23), `strings/ui.ts` (P1.37), `tauri.conf.json` incl. `productName`/`bundle.icon`/
> the §0.10 CSP + the three hardening keys + the no-URL-scheme negative (P1.19–P1.24),
> `capabilities/main.json` (P1.21), the §0.8 plugin registration in the Builder
> (single-instance/dialog/store/log/opener, P1.14), the no-updater posture (P1.18),
> and the §0.4.5 tauri-specta codegen pipeline — the `collect_commands!`/`collect_types!`
> registry seam (P1.25), the generated `bindings.ts` (P1.26), the typed-façade
> re-export shells + the single-IPC-consumer lint (P1.27/P1.36), the `cargo xtask
> codegen` invocation + the G19 drift check (P1.28/P1.53). **P2 does NOT re-scaffold
> any of these.** P2 **adds**: the C1–C13 commands + the three `app://` events +
> every wire type into the **existing** `collect_commands!`/`collect_types!` registry
> (P1.25); the behaviour bodies (intake, registries, lifecycle, the startup-ordering
> spine); and the domain/error/detection contract types. Every P2 box that consumes a
> P1 artifact carries an explicit `needs: P1.<n>` so the dependency is `plan-lint`-
> detectable rather than left to document order.
>
> **Behaviour boundaries.** P2 owns *contracts + skeleton*, not engine behaviour: the
> C12 type is declared here, the **probe body is P4**; the §7.2.1 *ordering* is
> established here, the **engine presence/integrity verifier body is P4**; `fs_guard` /
> isolation / pool real bodies are **P3/P4** (P2 declares only the types the contract
> surface references). No engine spawn, no conversion, no corpus.

---

## Domain model contracts (§0.6 shared vocabulary)

- [x] **P2.1** [RUST] Author the identity types — `InstanceId`/`RunId`/`CollectedSetId`/`ItemId`/`JobId`/`CollectingId` · §0.6 §7.1.2
  needs: P1.9, P1.25
  > the §0.6 identity newtypes (extending the P1.9 identity spine with `JobId`), each deriving `specta::Type` and registered in the P1.25 `collect_types!` registry so they don't generate as `any`.
  > Delivered: the six §0.6 identity types + their `specta::Type` derives + the five-newtype tauri-specta registration already landed in P1.9/P1.15/P1.25 (`JobId` is the §0.6 `type JobId = ItemId` alias — it inherits `ItemId`'s derive + registration, never separately registered); this box adds the one previously-unguarded contract — the compile-time `JobId = ItemId` alias lock (`jobid_compiles_as_itemid_alias`) + scoping the module `dead_code` expectation to `not(test)`.
- [x] **P2.2** [RUST] Author `IntakeOrigin` { Drop, Picker, LaunchArg, SecondInstance } · §0.6 §7.8
  needs: P2.1
- [x] **P2.3** [RUST] Author `UserFacingFormat` (the single grouping key — the full SSOT *What It Converts* set) · §0.6 §1.3
  needs: P2.1
- [ ] **P2.4** [RUST] Author `DroppedItem` (`item: ItemId`, raw/resolved path, size, `DetectionOutcome` ref) + the display-only `raw_path` scope note · §0.6 §1.2
  needs: P2.3, P2.15
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P2.15` points at the `DetectionOutcome` type box later in document order — `DroppedItem.detected: DetectionOutcome` (§0.6 / §1.2-owned) has no type to embed until `DetectionOutcome` (P2.15) exists, so DECISION C builds P2.15 first; the edge is acyclic and valid (P2.15 only `needs: P2.3`), the inversion documented at the `needs:` line.
  > `item: ItemId` is the §0.6-invariant-6 freeze-assigned id every eligible `DroppedItem` carries (`ItemId` from P2.1, already `[x]` — no new `needs:` edge); symmetric with `SkippedItem.item` (P2.5). Added by the §0.6 contradiction fix (the 4-field literal had omitted it).
- [ ] **P2.5** [RUST] Author `SkippedItem` + `SkipReason` { UnsupportedType, Uncertain, Empty, Unreadable } (id-disjoint over the single id space) · §0.6 §1.3
  needs: P2.4
- [ ] **P2.6** [RUST] Author the `CollectedSet` enum — `Single`/`Mixed`/`Unsupported`/`Uncertain`/`Empty` (the C1/C2a return + unified §1.4 confirm-summary fields) + the `CollectedNote` type · §0.6 §1.1 §1.4
  needs: P2.5
  > the §0.6 `CollectedSet` enum + the §1.4-owned **`CollectedNote`** type the §0.6 `CollectedSet` confirm-summary embeds (`notes: Vec<CollectedNote>`, PRODUCED by §1.2's bounded peek — encoding/delimiter/multi-sheet/animation hints): author `CollectedNote` explicitly, deriving `specta::Type` and **registered in the P1.25 `collect_types!` registry** so it mirrors to `bindings.ts` rather than generating as `any` (the §1.4 confirm-summary FIELDS are P3.27/P3.28's; the wire TYPE is homed here).
- [ ] **P2.7** [RUST] Author the wire-DTO types — `PickKind`/`OpenKind`/`IntakePayload`/`ScanProgress` · §0.6 §0.4.1 §0.4.2
  needs: P2.2
- [ ] **P2.8** [RUST] Author the target/option types — `TargetId`/`FormatId`/`CrossCatOp`/`Availability`/`Target`/`TargetOffer`/`OptionValues` · §0.6 §1.5 §1.6
  needs: P2.3
  > the §0.6 target/option vocabulary, decomposed into the scalar/alias layer (P2.8.3) and the composite layer (P2.8.4 — `Target`/`TargetOffer`/`OptionValues` that REFERENCE the scalars + the P2.8.1 `OptionDecl` family + the P2.8.2 `LossyKind`) so the foundational scalars and the composites that depend on them fail independently (_format.md §3.2, dual review once over the combined diff; matching the existing P2.8.1/P2.8.2 sub-box pattern). The §1.5 `Target.lossy: Option<LossyKind>` field (the predictable-loss marker) lives on the P2.8.4 composite `Target` and its `LossyKind` type is authored in the P2.8.2 sub-box so the field type-checks and mirrors to `bindings.ts` rather than generating as `any`.
  - [ ] **P2.8.1** [RUST] Author the §1.6 `OptionDecl` wire-type family — `OptionDecl`/`OptionKind`/`OptionKey`/`OptionValue`/`EnumChoice`/`Unit` (+ `LabelKey`) · §0.6 §1.6
    > the §1.6-owned generic option-declaration model the §0.6 `Target.options: Vec<OptionDecl>` embeds and `OptionValues == BTreeMap<OptionKey, OptionValue>` keys on: author `OptionDecl` (the declared knob: key/label/kind/default/tier), `OptionKind` (`IntRange`/`Enum`/`Toggle`/`Size`/`Color`), `OptionKey`, `OptionValue`, `EnumChoice`, `Unit` (and `LabelKey`), each deriving `specta::Type` and **registered in the P1.25 `collect_types!` registry** so the §0.4.5 type-drift check has them to mirror to `bindings.ts` (else they generate as `any`). This is the **single home** the P4 options-panel RENDERS (P4.64) and P5–P7 register declarations against — without it the entire per-format `OptionDecl` registration design rests on an unhomed type.
  - [ ] **P2.8.2** [RUST] Author the §2.9 `LossyKind` enum (all variants) + register it in `collect_types![]` · §2.9 §1.5 §0.4.3 · G23
    needs: P1.25
    > the §2.9 `LossyKind` wire enum the §1.5 `Target.lossy: Option<LossyKind>` field (P2.8.4) and the §0.6 `OutcomeMsg::Lossy { kind }` (P2.20) reference: author every §2.9.1 variant (`image_lossy_codec`/`image_palette`/`image_downscale`/`image_alpha_flatten`/`image_animation_flatten`/`image_svg_raster`/`doc_pdf_reflow`/`doc_pdf_to_text`/`doc_html_render`/`doc_to_text`/`doc_simplified`/`sheet_to_delimited`/`xls_legacy_limits`/`text_encoding_narrowed`/`slides_to_pdf_flatten`/`office_roundtrip_approx`/`pptx_to_ppt_legacy`/`audio_lossy_target`/`audio_transcode`/`audio_lossy_origin`/`audio_bitdepth`/`audio_tags_dropped`/`video_reencode`/`video_alpha_lost`/`video_subs_dropped`/`video_to_gif`/`audio_downmix`), deriving `specta::Type` and **registered in the P1.25 `collect_types![]` registry** (§2.8.2 line 1261 explicitly REQUIRES `LossyKind` derive `specta::Type` + be in `collect_types![]`) so `Target.lossy` does NOT generate as `any` (the no-`any` rule). The enum is the wire TYPE; the §2.9.1 kind→note STRING TABLE is the separate `crate::outcome` box P3.69. **Cardinality note (escalated, not silently reconciled):** §1.5 declares `Target.lossy: Option<LossyKind>` (≤1 on the wire) but §2.9.2 + P4.65 render a CO-APPLYING set (de-dup to the most-specific 2–3) — author the wire field as §1.5 says (`Option<LossyKind>` for the single primary marker) and record the §1.5-vs-§2.9.2 conflict for owner escalation per the conflict order (SSOT > spec); do NOT change `Option` to `Vec` here without a spec decision.
  - [ ] **P2.8.3** [RUST] Author the scalar/alias layer — `TargetId`/`FormatId`/`CrossCatOp`/`Availability` (the leaf types the composites key on) · §0.6 §1.5
    needs: P1.25
    > the foundational §0.6 scalar/alias types `TargetId`/`FormatId`/`CrossCatOp`/`Availability` (the leaf vocabulary the P2.8.4 composites reference) — each deriving `specta::Type` and registered in the P1.25 `collect_types!` registry so they mirror to `bindings.ts` rather than generating as `any`. Built before the composites (P2.8.4 `needs:` this) so the foundational scalars fail independently of the composite structs that key on them.
  - [ ] **P2.8.4** [RUST] Author the composite layer — `Target`/`TargetOffer`/`OptionValues` (referencing the P2.8.3 scalars + P2.8.1 `OptionDecl` + P2.8.2 `LossyKind`) · §0.6 §1.5 §1.6
    needs: P2.8.3, P2.8.1, P2.8.2, P1.25
    > the §0.6 composite types that compose the scalars + the option/lossy families: `Target` (incl. the §1.5 `Target.lossy: Option<LossyKind>` field from P2.8.2 + `options: Vec<OptionDecl>` from P2.8.1), `TargetOffer` (the C3 return — the offered targets + the one pre-highlighted default), `OptionValues == BTreeMap<OptionKey, OptionValue>`; each deriving `specta::Type` and registered in the P1.25 `collect_types!` registry so they mirror to `bindings.ts` rather than generating as `any`. Fails independently of the scalar layer (a malformed composite struct vs a missing leaf alias). (`needs: P2.8.3` for the scalars + `P2.8.1`/`P2.8.2` for the `OptionDecl`/`LossyKind` families the composites embed.)
- [ ] **P2.9** [RUST] Author the destination/plan types — `DestinationChoice`/`OutputPlan`/`DivertReason` (directory-based, no pre-baked `final_path`) · §0.6 §2.7 §2.14.1
  needs: P2.6
- [ ] **P2.10** [RUST] Author `Batch`/`ConversionJob`/`JobState`/`JobStage` · §0.6 §1.9
  needs: P2.8, P2.9, P2.18
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P2.18` points at the `ErrorKind` type box later in document order — `JobState::Failed(ErrorKind)` (§0.6) has no type to land until `ErrorKind` (P2.18) exists in `crate::outcome`, so DECISION C builds P2.18 first; the edge is acyclic and valid (P2.18 needs only P1.10/P1.25), the inversion documented at the `needs:` line.
- [ ] **P2.11** [RUST] Author the command-return DTOs — `OutputPlanPreview`/`RerunPrompt`/`RerunDecision`/`PreflightVerdict`/`DestinationResolved` · §0.6 §1.8 §1.10 §2.5
  needs: P2.10, P2.18
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P2.18` points at the `ErrorKind` type box later in document order — `PreflightVerdict.up_front_fail: Option<ErrorKind>` (§0.6) has no type to land until `ErrorKind` (P2.18) exists, so DECISION C builds P2.18 first; the edge is acyclic and valid, the inversion documented at the `needs:` line.
- [ ] **P2.12** [RUST] Author the result types — `RunResult`/`ItemResult`/`Totals`/`CleanupResidue`/`ItemOutcome` · §0.6 §1.12 §2.6
  needs: P2.10, P2.19, P2.20
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P2.19` points at the `IpcError` shape box later in document order — the `ItemOutcome::Failed { error: IpcError }` variant (§0.6 / §0.4.3) has no payload type to land until `IpcError` (P2.19) exists, so DECISION C builds P2.19 first; the edge is acyclic and valid, the inversion documented at the `needs:` line.
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P2.20` points at the `OutcomeMsg` box later in document order — `ItemResult.reason: Option<OutcomeMsg>` (§0.6; the documented domain↔outcome type pairing) has nowhere to land until `OutcomeMsg` (P2.20) exists, so DECISION C builds P2.20 first; the edge is acyclic and valid, the inversion documented at the `needs:` line.
- [ ] **P2.13** [RUST] Author the engine-descriptor seam types — `EngineId`/`EngineDescriptor`/`EngineKind` (non-trait `FFprobe`/`ImageMagick` note) · §0.6 §3.2
  needs: P2.3
- [ ] **P2.14** [TEST] Property-test the §0.6 normative invariants (one-Target-per-Batch, `count == items.len()`, `ConversionJob.item == source.item`, frozen `items`, stable `ItemId`, same-volume publish-temp) · §0.6 · G22 G23
  needs: P2.12, P2.13

## Detection-outcome contract (the §1.2 result type)

- [x] **P2.15** [RUST] Author `DetectionOutcome` (`Recognized`/`UnsupportedType`/`Uncertain`/`Empty`/`Unreadable`) + `Confidence` { High, Low } + `ReadFailure` { NotFound, PermissionDenied, Locked, IoError } as the single canonical §1.2 detection-result family · §1.2 §0.6
  needs: P2.3
  > `ReadFailure` is folded in here (not its own box) because §1.2 defines `DetectionResult`/`DetectionOutcome`/`Confidence`/`ReadFailure` as one [DECIDED] type-family and `DetectionOutcome::Unreadable { reason: ReadFailure }` embeds it — authoring the family as one box avoids the otherwise-fatal P2.15↔P2.17 needs-cycle.
- [ ] **P2.16** [RUST] Author the `DetectionOutcome → SkipReason` projection (ineligible-outcome → skip) · §1.2 §1.3 §0.6
  needs: P2.15, P2.5
- [ ] **P2.17** [RUST] Author the `EmptyReport` contract type feeding the `Empty { skipped }` reason tally · §1.2 §0.6
  needs: P2.15
  > the §1.2-cohesive `ReadFailure` is authored with `DetectionOutcome` in P2.15; this box authors only `EmptyReport` (the `Empty { skipped }` tally), which embeds `DetectionResult` — hence `needs: P2.15` is correct and acyclic.

## Error & outcome model contract (the §2.8 wire mirror)

- [ ] **P2.18** [RUST] Author `ErrorKind` as a `type` alias of (or drift-locked mirror of) the §2.8 `ConversionErrorKind` in `crate::outcome` · §0.4.3 §2.8.1
  needs: P1.10, P1.25
  - [ ] **P2.18.1** [RUST] Enumerate the item-level `ErrorKind` variants byte-identical to the §2.8 catalog · §0.4.3 §2.8.1
  - [ ] **P2.18.2** [RUST] Add the run/app-level kinds (`EngineMissing`/`WebviewFault`/`BundleDamaged`) + the mirror-only `MixedDrop` entry · §0.4.3 §2.13.1
  - [ ] **P2.18.3** [TEST] Lock anti-drift — `static_assertions` variant-count + variant-name round-trip `#[test]` · §0.4.3 §2.8.2 · G23
- [ ] **P2.19** [RUST] Author the `IpcError` shape (`kind`/`message`/`path`/`residue`, derives `specta::Type`, in `collect_types![]`) · §0.4.3 §2.8
  needs: P2.18
- [ ] **P2.20** [RUST] Author `OutcomeMsg` + the `SkipReason → ErrorKind` forward (one-way, non-inverted) projection helper · §0.6 §2.8.2 §1.12
  needs: P2.18, P2.16, P2.8.2

## IPC command surface (C1–C13 contracts)

- [ ] **P2.21** [RUST] Wire the `invoke_handler` + register C1–C13 on the Builder (handlers thin, delegate to orchestrator) · §0.4.0 §0.7
  needs: P1.11, P1.13, P1.25
- [ ] **P2.22** [RUST] Author the C1 `ingest_paths` contract — frozen-set builder, `origin`, `collectingId`, `drainPending`, optional `onScan` Channel · §0.4.1 §1.1 §2.4
  needs: P2.21, P2.6, P2.2, P2.7
- [ ] **P2.23** [RUST] Author the C2a `pick_for_intake` contract — Rust-side `DialogExt` picker funnelling into the C1 freeze, no raw path to WebView · §0.4.1 §1.1 §5.4
  needs: P2.22, P1.14, P2.7
- [ ] **P2.24** [RUST] Author the C2b `pick_destination` contract — Rust-side folder picker returning the chosen `PathBuf` (the one write-path that transits the WebView) · §0.4.1 §0.10
  needs: P2.21, P1.14
- [ ] **P2.25** [RUST] Author the C3 `get_targets` contract — pure function of detection → `TargetOffer` (one pre-highlighted default, no spawn) · §0.4.1 §1.5
  needs: P2.21, P2.8
- [ ] **P2.26** [RUST] Author the C4 `plan_output` contract — `OutputPlanPreview` (resolved dest, divert preview, §2.5 rerun, §1.10 preflight) · §0.4.1 §1.8 §2.5 §1.10
  needs: P2.21, P2.11
- [ ] **P2.27** [RUST] Author the C5 `set_destination` contract — `DestinationResolved` (re-eval preflight, carry rerun through unchanged) · §0.4.1 §1.8 §2.14.4
  needs: P2.26
- [ ] **P2.28** [RUST] Encode the C4/C5 asymmetry as an enforced orchestrator lifecycle rule (C4 re-callable; C5 owns destination; C4 never overrides C5) · §0.4.1
  needs: P2.27
- [ ] **P2.29** [RUST] Author the C6 `start_conversion` contract — mint `RunId`, enqueue, return immediately, stream over `onProgress` Channel; `destination` authoritative · §0.4.1 §1.9 §7.1.2
  needs: P2.21, P2.11, P2.37
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P2.37` points at the `ConversionEvent` Channel-enum box later in document order — the C6 `start_conversion` signature's `onProgress: Channel<ConversionEvent>` parameter (§0.4.1) has nowhere to land until `ConversionEvent` (P2.37) exists, so DECISION C builds P2.37 first; the edge is acyclic and valid (P2.37 → P2.12 → P2.10), the inversion documented at the `needs:` line.
- [ ] **P2.30** [RUST] Author the C7 `cancel_run` contract — trip the `RunId` token (keep finished, discard in-progress) · §0.4.1 §0.4.4 §1.7
  needs: P2.29
- [ ] **P2.31** [RUST] Author the C8 `get_run_summary` contract — idempotent re-fetch of the retained `RunResult` · §0.4.1 §0.4.4 §1.12
  needs: P2.29, P2.12
- [ ] **P2.32** [RUST] Author the C9 `open_path` contract — Rust-side `OpenerExt` reveal/open with the §7.7.3 `RunResult` membership gate · §0.4.1 §7.7.1 §7.7.3
  needs: P2.21, P1.14, P2.7
- [ ] **P2.33** [RUST] Author the C10 `open_project_page` contract — Rust handler opens a compiled-in canonical URL constant (no WebView URL arg) · §0.4.1 §7.6.2 §7.7.2
  needs: P2.21, P1.14
- [ ] **P2.34** [RUST] Author the C11 `get_app_info` contract — `AppInfo` (version, build id, platform, third-party-notice) · §0.4.1 §7.2.3
  needs: P2.21, P2.112
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P2.112` points at the `AppInfo` type box later in document order — the C11 `get_app_info` contract returns `AppInfo` (§0.4.1 / §7.2.3), which has no definition to compile / type-share against until `AppInfo` (P2.112) exists, so DECISION C builds P2.112 first; the edge is acyclic and valid, the inversion documented at the `needs:` line.
- [ ] **P2.35** [RUST] Author the C13 `cancel_ingest` contract — trip the `CollectingId` ingest-scoped token · §0.4.1 §1.1
  needs: P2.22
- [ ] **P2.36** [GATE] Assert the C1–C13 IPC-surface set is complete + drift-free (no extra/missing command; plan-lint check 9/12 target) · §0.4.1 · G23
  needs: P2.35, P2.33, P2.34, P2.31, P2.32

## IPC event / Channel surface (the three `app://` events + telemetry Channels)

- [ ] **P2.37** [RUST] Author the `ConversionEvent` Channel enum + its payload structs (`RunStarted`/`ItemStarted`/`ItemProgress`/`ItemFinished`/`BatchProgress`/`RunFinished`) · §0.4.2 §1.11
  needs: P2.12, P2.10, P1.25
  - [ ] **P2.37.1** [RUST] Encode the `RunStarted.totalItems` = queued-eligible-only denominator rule · §0.4.2 §1.3
  - [ ] **P2.37.2** [RUST] Encode the conservative `willReencode` worst-case `bool` (always definite, never omitted) · §0.4.2 §2.9.2
  - [ ] **P2.37.3** [RUST] Encode the `BatchProgress.total` == `RunStarted.totalItems` (queued-only) invariant · §0.4.2 §1.11
  - [ ] **P2.37.4** [RUST] Encode the pre-flight-skip emission policy (no live `ItemFinished{Skipped}`; terminal projection only) · §0.4.2 §1.9 §1.12
- [ ] **P2.38** [RUST] Author the `ScanProgress { scanned }` intake-telemetry Channel payload (throttled, dies with C1) · §0.4.2 §1.1
  needs: P2.22
- [ ] **P2.39** [RUST] Author the three `app://` events — `app://fault` (`AppFault`), `app://intake` (`IntakePayload`), `app://close-requested` (`()`) · §0.4.2 §2.13 §7.8.1 §7.3.2
  needs: P2.7, P1.25, P2.18.2
  > `needs: P2.18.2` (the earlier P2.3-group app-level `ErrorKind` variants `EngineMissing`/`WebviewFault`/`BundleDamaged`) so the P2.39.1 `AppFault.kind` subset has its source authored first — a normal backward edge (P2.18.2 precedes P2.39 in document order), named so the `AppFault.kind` ↔ app-level-`ErrorKind` source is plan-lint-detectable.
  - [ ] **P2.39.1** [RUST] Author the `AppFault` wire struct (`kind` = the app-level `ErrorKind` subset {EngineMissing,WebviewFault,BundleDamaged} + `message: String`) + register it in `collect_types![]` · §0.4.2 §2.13.1 §2.13.3 §0.4.3 · G23
    needs: P2.18.2, P1.25
    > the §0.4.2 wire-table row `| app://fault | AppFault | …` payload the P2.39 `app.emit('app://fault', AppFault{..})` carries (§2.13.1/§2.13.3 — the app-level fault the §2.13.3 single-screen presentation renders): author `AppFault { kind: <app-level ErrorKind subset {EngineMissing, WebviewFault, BundleDamaged}>, message: String }`, deriving `specta::Type` and **registered in the P1.25 `collect_types![]` registry** so the TS `listen('app://fault')` side type-checks against the mirrored type rather than generating as `any` (the no-`any` rule the P2.5 group enforces for every sibling wire type — IntakePayload/ScanProgress/IpcError/EngineHealth/AppInfo/CollectedNote each authored + registered). The `kind` field draws its three variants from the app-level `ErrorKind` set P2.18.2 authors (the §2.13.1 app/run-level kinds, NOT the item-level §2.8 catalog) — this box authors the STRUCT that carries `kind`+`message`, P2.18.2 authors the variant set. (`needs: P2.18.2` for the app-level `ErrorKind` variants the `kind` field subsets + `P1.25` for the `collect_types!` registry.)
- [ ] **P2.40** [RUST] Encode the `app://intake` IDLE-path-only rule (busy refuses + drops core-side, never emits ingestable paths) · §0.4.2 §7.8.1
  needs: P2.39
- [ ] **P2.41** [GATE] Assert the closed three-event invariant — exactly `{fault, intake, close-requested}`, no fourth `app://` event, each with its authored+registered payload type · §0.4.2 · G23
  needs: P2.39
  > exactly `{app://fault, app://intake, app://close-requested}` exist, no fourth `app://` event — AND each event's §0.4.2 payload type is authored + in `collect_types![]` (`AppFault` P2.39.1, `IntakePayload` P2.7, `()` for close-requested) so no `app://` payload mirrors as `any` (the no-`any` rule); transitively covers P2.39.1's `AppFault` via the `needs: P2.39` parent (P2.39 is `[x]` only when P2.39.1 is).

## Registries & cancellation lifecycle (the orchestrator state)

- [ ] **P2.42** [RUST] Build the `RunId` → `CancellationToken` run registry (created in C6, tripped by C7, dropped on `RunFinished`) · §0.4.4 §1.7
  needs: P2.29, P2.30
- [ ] **P2.43** [RUST] Build the `RunResult` retention (process-local, until next run / app exit) for C8 re-serve · §0.4.4 §1.12 §7.4
  needs: P2.31, P2.42
- [ ] **P2.44** [RUST] Build the `CollectedSetId` → `FrozenCollectedSet` registry (created on C1/C2a freeze; resolved by C3/C4/C5/C6; evicted on run-start/supersede/exit) · §0.4.4 §2.4
  needs: P2.22, P2.6
- [ ] **P2.45** [RUST] Build the `CollectingId` → ingest-scoped token registry (frontend-generated id, registered at handler entry, dropped on EVERY exit branch) · §0.4.4 §1.1
  needs: P2.35, P2.23
- [ ] **P2.46** [DOC] Record the macOS reload-during-run non-recovery scope (`[DECIDED]` post-terminal re-serve only) · §0.4.4

## Instance & run identity + single-instance policy (§7.1)

- [ ] **P2.47** [RUST] Establish the `InstanceId` app-managed singleton (random v4, never persisted/networked) · §7.1.2 §2.11
  needs: P2.1, P1.14
- [ ] **P2.48** [RUST] Fix the `RunId` mint point — at C6 accept (NOT at the §2.4 freeze; the freeze yields `CollectedSetId`) · §7.1.2 §0.4.4
  needs: P2.29, P2.47
- [ ] **P2.49** [RUST] Encode the `<InstanceId>.<pid>` scratch-root naming + `run-<RunId>/` subdir identity (PID = label, not liveness) · §7.1.2 §2.14
  needs: P2.47
- [ ] **P2.50** [DOC] Record the advisory-lock-is-authoritative liveness predicate (PID never used as the test; §2.6.3 owns the lock) · §7.1.2 §2.6.3
  needs: P2.49
- [ ] **P2.51** [RUST] Encode the per-OS-user (not machine-global) single-instance lock scope · §7.1.1
  needs: P1.14
- [ ] **P2.52** [RUST] Wire the single-instance callback — re-focus the "main" window + forward argv via `forward_launch_argv`, origin `SecondInstance` · §7.1.1 §7.8.1
  needs: P1.14, P2.51, P2.54.1
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P2.54.1` points at the `parse_path_args` helper sub-box defined later in document order — `forward_launch_argv` forwards argv through that helper, so DECISION C builds P2.54.1 first; the edge is acyclic and valid, the inversion documented at the `needs:` line.
- [ ] **P2.53** [DOC] Record the macOS edge cases — least-mature single-instance leg (§6.6 verification item) + the unsigned two-copies accepted-limitation · §7.1.1

## OS-intake funnel (§7.8.1) — the launch/Open-with state machine

- [ ] **P2.54** [RUST] Build the single `forward_launch_intake(app, paths, origin)` funnel (every launch-time path source routes here) · §7.8.1 §1.1
  needs: P2.47, P2.39
  - [ ] **P2.54.1** [RUST] Build `parse_path_args(argv, cwd) -> Vec<PathBuf>` — the §7.8.1 `forward_launch_argv` flag/path classifier · §7.8.1 §7.5.3 §1.1
    > the named §7.8.1 helper `forward_launch_argv(app, argv, cwd, origin)` calls (`forward_launch_intake(app, parse_path_args(argv, cwd), origin)`): separate **flag tokens from file-path tokens** — strip the `--verbose`/env-flag launch switches (`--verbose` is a `[DECIDED]` launch flag, §7.5.3, so it MUST NOT become an ingestable path), skip `argv[0]` (the program path), resolve **relative** path args against the launching `cwd`, and handle Win-vs-Linux argv conventions; return `Vec<PathBuf>`. The §1.1 freeze re-validates every returned path (so this is classification, not a trust boundary) — but the flag-vs-path split + cwd-relative resolution are genuinely homed here. Consumed by the argv intake (P2.57) and the single-instance callback (P2.52, which forwards `argv` via `forward_launch_argv`).
- [ ] **P2.55** [RUST] Enforce the §7.1.1 PRIMARY refuse-busy gate inside the funnel (mid-run: DROP paths, no emit, no buffer) · §7.8.1 §7.1.1 §2.4
  needs: P2.54, P2.40
- [ ] **P2.56** [RUST] Wire the macOS `RunEvent::Opened { urls }` handler — `Url::to_file_path()` → funnel, origin LaunchArg/SecondInstance by readiness · §7.8.1 §1.1
  needs: P2.54
  - [ ] **P2.56.1** [DOC] Record the macOS-only Tauri-v2 fact (`RunEvent::Opened` never fires on Win/Linux; registered unconditionally for code simplicity) · §7.8.1
  - [ ] **P2.56.2** [DOC] Record the NOT-`tauri-plugin-deep-link`/`on_open_url` decision (custom-scheme intent, never the open-documents AppleEvent) · §7.8.1 §7.8.2
- [ ] **P2.57** [RUST] Wire the Windows-argv (`std::env::args_os` at first launch) + Linux `%F`/`%U` argv intake into `forward_launch_argv` · §7.8.1 §1.1
  needs: P2.54, P2.54.1
- [ ] **P2.58** [RUST] Build the `State<PendingIntake>` first-launch buffer (stash paths+origin when frontend not ready) · §7.8.1
  needs: P2.54
- [ ] **P2.59** [RUST] Wire the ready-flag branch — emit `app://intake` if ready, else `buffer_pending_intake` · §7.8.1 §0.4.2
  needs: P2.58, P2.40
- [ ] **P2.60** [RUST] Build the `drainPending` drain path — C1 `paths: []` + `drainPending: true` consumes `PendingIntake` once (stored origin), returns its `CollectedSet` · §7.8.1 §0.4.1
  needs: P2.59, P2.22
- [ ] **P2.61** [UI] Wire the root-shell-mount drain trigger (always re-call C1 with `drainPending: true` after listener registration, closing the listener race) · §7.8.1 §5.2
  needs: P2.60, P1.27

## Intake freeze state machine (§1.1) — idle-vs-in-flight gating

- [ ] **P2.62** [RUST] Implement the §1.1 single `ingest(paths, origin) -> CollectedSet` funnel (the exhaustive freeze point for all five entry points) · §1.1 §2.4
  needs: P2.22, P2.6
- [ ] **P2.63** [RUST] Set the per-entry-point `origin` stamping (C1 from request; C2a handler stamps `Picker`; launch hooks stamp `LaunchArg`/`SecondInstance`) · §1.1 §0.6
  needs: P2.62, P2.23
- [ ] **P2.64** [RUST] Implement Rust-side folder recursion (`walkdir`, depth-first, symlinked dirs not traversed) · §1.1 §0.8
  needs: P2.62
- [ ] **P2.65** [RUST] Encode the fixed hidden/system-file ignore constant (dotfiles, `.DS_Store`/`Thumbs.db`/`desktop.ini`, Win hidden/system attrs) · §1.1
  needs: P2.64
- [ ] **P2.66** [RUST] Retain the dropped root(s) on the frozen set (for §2.7 subtree re-creation + open-folder common root) · §1.1 §2.7
  needs: P2.64
- [ ] **P2.67** [RUST] Implement the mid-walk per-item-failure-does-not-abort rule (per-item `Unreadable`/`Empty` → `SkippedItem`, walk continues) · §1.1 §1.2 §1.9
  needs: P2.64, P2.16
- [ ] **P2.68** [RUST] Encode the fatal-walk-root-error stop (dropped root itself unreadable/gone) distinct from per-item skip · §1.1
  needs: P2.67
- [ ] **P2.69** [RUST] Implement cooperative ingest cancellation — poll the `CollectingId` token in the walk/detect loop, discard partial unfrozen set (no cleanup obligation) · §1.1 §0.4.1
  needs: P2.64, P2.45
- [ ] **P2.70** [RUST] Implement the C2a native-dialog-phase rules — async/`spawn_blocking` picker (never `blocking_pick_file` on a Tokio worker), token registered before dialog opens · §1.1 §0.4.1
  needs: P2.69, P2.23
- [ ] **P2.71** [RUST] Implement the C2a token-drop-on-EVERY-exit-branch rule (cancelled-dialog → `Empty`, C13-tripped → `Empty`, normal walk-completes) · §1.1 §0.4.4
  needs: P2.70
- [ ] **P2.72** [RUST] Implement the freeze idle-vs-in-flight gating — IDLE starts a new frozen set; in-flight refuses-busy (never mutate/merge a frozen set) · §1.1 §7.1.1 §2.4
  needs: P2.62, P2.55
- [ ] **P2.73** [RUST] Encode the zero-byte/unreadable-at-intake classification — intake-time `Empty`/`Unreadable` = Skipped (pre-flight, never queued); turn-time = Failed (mid-run) · §1.1 §1.2 §0.6
  needs: P2.67, P2.5
- [ ] **P2.74** [RUST] Author the `crate::fs_guard::resolve_identity` interface stub the freeze de-dup calls (real body P3) · §1.1 §2.3
  needs: P1.11
- [ ] **P2.75** [RUST] Assign `ItemId` at the freeze over the single id space (eligible + skipped, never re-indexed from 0) · §1.1 §0.6
  needs: P2.62, P2.74
- [ ] **P2.76** [RUST] Apply resolved-identity de-dup as the frozen set is built (a file reached via two paths is one member) · §1.1 §2.3
  needs: P2.75

## Window & app lifecycle (§7.3)

- [ ] **P2.77** [DOC] Record the no-tray / no-background-agent / closing-quits posture (portable, no system pollution) · §7.3.1
- [ ] **P2.78** [RUST] Create the single "main" window at startup (no tray, no secondary windows, default size each launch) · §7.3.1 §7.4.1
  needs: P1.16, P2.77
- [ ] **P2.79** [RUST] Wire `Builder::on_window_event` — v2 two-arg `(&Window, &WindowEvent)` `CloseRequested` handler · §7.3.2
  needs: P2.78
- [ ] **P2.80** [RUST] Implement the close-requested decision in Rust — `converter_is_busy` → `api.prevent_close()` + emit `app://close-requested` (`serde_json::Value::Null` payload) · §7.3.2 §7.3.3
  needs: P2.79, P2.39
- [ ] **P2.81** [RUST] Wire the `App::run` `RunEvent::ExitRequested` (last `prevent_exit` chance) + `RunEvent::Exit` (flush logs, best-effort scratch cleanup) handlers · §7.3.2 §2.6
  needs: P2.78
- [ ] **P2.82** [RUST] Route `RunEvent::Opened` through the funnel inside the `App::run` closure (the macOS Open-with hook, §7.8.1 refuse-busy enforced) · §7.3.2 §7.8.1
  needs: P2.81, P2.56
- [ ] **P2.83** [RUST] Establish the quit-while-converting contract — confirm → cancel-in-flight (§1.7) + §2.6 cleanup + exit = same path as in-UI Cancel; idle quits immediately · §7.3.3 §1.7 §2.6
  needs: P2.80, P2.42
- [ ] **P2.84** [DOC] Record the no-persistent-queue / no-resume-across-launches `[DECIDED]` (in-memory queue only; re-drop on next launch) · §7.3.4 §7.4

## Persistence (§7.4) — the 3-key prefs blob

- [ ] **P2.85** [RUST] Implement the 3-key `settings.json` prefs blob via `tauri-plugin-store` (`theme`/`lastDestinationMode`/`verboseLog`, defaults) · §7.4.1 §7.4.2
  needs: P1.14
  - [ ] **P2.85.1** [RUST] Resolve the per-OS config-dir location via `app.path().app_config_dir()` (`dev.ne-ia.convertia/settings.json`) · §7.4.2
  - [ ] **P2.85.2** [RUST] Implement best-effort-never-load-bearing tolerance (unreadable/corrupt → log + run with defaults, never block a conversion) · §7.4.2
- [ ] **P2.86** [RUST] Encode the single-store-name (T2c) convention — only `Store.load('settings.json')`, one call site · §7.4.2 §0.10 · G29
  needs: P2.85
- [ ] **P2.87** [DOC] Record the explicit persistence negatives (no history / recent-files / presets / window-geometry / resumable queue) · §7.4.1 §7.3.4
- [ ] **P2.88** [RUST] Encode the `lastDestinationMode` re-validate-as-writable-at-use-time rule (a hint, never a guarantee; §2.7 fallback applies) · §7.4.1 §2.7
  needs: P2.85

## Logging & diagnostics (§7.5) — local-only, no telemetry

- [ ] **P2.89** [RUST] Configure `tauri-plugin-log` — rotating file + dev stderr, default level `warn`/`info`, no network sink · §7.5.1 §7.5.2
  needs: P1.14
- [ ] **P2.90** [RUST] Resolve the per-OS log-dir via `app.path().app_log_dir()` + the Linux config-dir deviation note · §7.5.2
  needs: P2.89
- [ ] **P2.91** [RUST] Configure rotation — `max_file_size(5_000_000)` + `RotationStrategy::KeepOne` (≈1× footprint, source-verified vs the pinned version) · §7.5.2
  needs: P2.89
- [ ] **P2.92** [DOC] Record the `KeepOne == fs::remove_file` ≈1× footprint audit + the `[DEFER: verify-on-bump]` re-check trigger against the pinned commit · §7.5.2
  needs: P2.91
- [ ] **P2.93** [RUST] Implement the redaction stance — NEVER log file contents/bytes/full-paths at default level; structural facts + basename only · §7.5.3 §2.11 · G29
  needs: P2.89
- [ ] **P2.94** [RUST] Implement the verbose-mode opt-in (full paths + exact engine argv) read-once-at-startup (`verboseLog` + `--verbose`), effective next launch · §7.5.3 §3.5
  needs: P2.93, P2.85
  - [ ] **P2.94.1** [RUST] Wire the §7.5.4 dev-facing diagnostics set into verbose mode — per-engine spawned argv + persisted stderr, resolved scratch/temp paths, per-item timing, output-plan/divert decisions · §7.5.4 §2.14 §1.8 §3.5
    needs: P2.94
    > the §7.5.4 "makes §6.5 operable" capture set verbose mode ADDITIONALLY records beyond P2.94's full-paths/engine-argv (the diagnostic surface the §6.5 reliability gate operationally depends on): the **exact spawned argv per engine** (§3.5), **engine `stderr` persisted** (§2.13 captures-and-classifies; here also written to the log), the **resolved scratch/temp paths** (§2.14), **per-item timing**, and the **chosen output-plan decisions incl. per-location divert** (§1.8). The logging plumbing + the redaction-stance interaction are homed here; the actual capture points are wired by their producers as they land (the per-engine argv/stderr in P4 where the §2.12 spawn wrapper lands, the scratch/temp-path + output-plan/divert captures in P3 where `crate::run`/the §1.8 output-plan land) — each producer feeds this verbose-diagnostics sink. The P2.127 log-redaction property gate must prove the §7.5.4 full paths/scratch-paths added here still redact at default level (only verbose surfaces them).
- [ ] **P2.95** [RUST] Add the JS-bridge so frontend errors land in the same log file · §7.5.1
  needs: P2.89, P1.27
- [ ] **P2.96** [DOC] Record the no-automatic-upload-ever stance (the §6.8 bug-report flow attaches the log manually) · §7.5.3 §2.11

## Update posture (§7.6) — no auto-updater (defense in depth)

- [ ] **P2.97** [DOC] Record the no-startup/background version-check assertion (zero network calls at startup) · §7.6.1 §7.2.2
- [ ] **P2.98** [RUST] Encode BOTH C11/About data sources — the version-display source (`app.package_info().version` / `CARGO_PKG_VERSION`) AND the `AppInfo.build_id` PRODUCER (§6 CI build id at build time + deterministic dev fallback) · §7.6.2 §7.2.3 · G19
  needs: P2.34, P2.112
  > **Forward-ref note (DECISION-C ordering inversion):** `needs: P2.112` points at the `AppInfo` type box later in document order — the `build_id`/`version` fields this box populates have nowhere to land until `AppInfo` (P2.112) exists, so DECISION C builds P2.112 first; the edge is acyclic and valid, the inversion documented at the `needs:` line.
  > the two data sources that POPULATE the C11 `AppInfo` (P2.112) the §5.9 About screen renders (RELEASE-BLOCKING per SSOT — neither field may silently ship empty): **(a) version** — `app.package_info().version` / `CARGO_PKG_VERSION`, the §7.6.2 displayed current version. **(b) the `build_id` PRODUCER** — wire WHERE the §7.2.3 `build_id: String // CI build identifier (§6)` comes from: the §6 (Lane-B/`build-loop`) build-time CI build identifier (the git SHA + the GitHub Actions run-id, injected at build time via a build-script `env!`/`option_env!` over a CI-set var) with a **deterministic dev fallback** (e.g. the short git SHA or a literal `"dev"` marker when the CI var is absent, never an empty string), so a local `tauri dev` build still yields a non-empty `build_id` and a CI build carries the real §6 identifier. The drift-check (G19, §0.4.5) covers the generated-binding side once C11 is type-shared. (`needs: P2.34` for the C11 contract + `P2.112` for the `AppInfo` type whose `build_id`/`version` fields this box populates.)
- [ ] **P2.99** [DOC] Record the future opt-in update-check parked decision (`updateCheckOptIn` not present in v1) · §7.6.3 §7.4

## OS shell-out (§7.7) — open-folder / open-file / open-url

- [ ] **P2.100** [RUST] Map all three `OpenKind` variants to concrete `OpenerExt` calls (`RevealInFolder`→`reveal_item_in_dir`, `Folder`→`open_path`(dir), `File`→`open_path`) · §7.7.1 §0.6
  needs: P2.32
- [ ] **P2.101** [RUST] Implement the Rust-side `RunResult`-membership gate (no static opener scope) — reveal/open-path validated against recorded outputs + roots before `OpenerExt` · §7.7.2 §7.7.3
  needs: P2.100, P2.43
- [ ] **P2.102** [RUST] Implement the two-membership-rule split — file-launch admits only output FILES; folder-browse admits run ROOTS (`common_root` + `divert_root`) · §7.7.3 §0.6
  needs: P2.101
- [ ] **P2.103** [RUST] Implement the split-output two-open-folder-targets contract (`common_root` + `Some(divert_root)` both in the membership set) · §7.7.1 §7.7.3
  needs: P2.102, P2.12
- [ ] **P2.104** [RUST] Implement C10 as a compiled-in canonical URL constant via `OpenerExt::open_url` (no URL-injection surface) · §7.7.2 §7.6.2
  needs: P2.33
- [ ] **P2.105** [DOC] Record the open-file safety posture (no auto-open, reveal-in-folder is the preferred default, OS default app on explicit click only) · §7.7.3

## Startup sequence ordering (§7.2.1) — the app-shell spine

- [ ] **P2.106** [RUST] Establish the §7.2.1 ordered startup sequence as the shell spine (steps 1–8, window shown only after steps 3–5 succeed) · §7.2.1 §2.13
  needs: P1.15, P2.51, P2.78, P2.81
  - [ ] **P2.106.1** [RUST] Step 1 — single-instance guard registered first (second launch hands off + exits) · §7.2.1 §7.1.1
  - [ ] **P2.106.2** [RUST] Step 2 — establish `InstanceId` + resolve base paths (config/scratch/log) via `app.path()`, no dir created yet · §7.2.1 §7.1.2
  - [ ] **P2.106.3** [RUST] Step 3 — engine presence+integrity verification SLOT (app-level fault on failure; verifier body P4) · §7.2.1 §7.2.3
  - [ ] **P2.106.4** [RUST] Step 4 — executable-permission setup SLOT on the engine binaries (portable build; body P4) · §7.2.1 §7.2.4
  - [ ] **P2.106.5** [RUST] Step 5 — scratch + log dir creation with the per-instance root + orphan-reclaim SLOT (mechanism §2.6, body P3/P4) · §7.2.1 §7.2.5 §2.6
  - [ ] **P2.106.6** [RUST] Step 6 — WebView window create + frontend load (WebView-init fault where the core can observe it) · §7.2.1 §0.3.1
  - [ ] **P2.106.7** [RUST] Step 7 — process launch-time intake feed (argv / PendingIntake drain → §1.1) · §7.2.1 §7.8.1
  - [ ] **P2.106.8** [UI] Step 8 — hand to the UI empty/idle state · §7.2.1 §5.2
- [ ] **P2.107** [RUST] Implement the §7.2.2 offline assertion at startup (the shell adds ZERO startup network activity) · §7.2.2 §2.11
  needs: P2.106
- [ ] **P2.108** [DOC] Record the Windows-WebView2-absent honest-exception (loader fails before the core; download-page note, no in-app dialog) · §7.2.1 §0.3.1
- [ ] **P2.109** [RUST] Surface a missing/old/broken macOS-WKWebView / Linux-WebKitGTK init as a §2.13/§7.2 startup fault (where the core observes it) · §7.2.1 §0.3.1 §2.13
  needs: P2.106.6, P2.39

## The C12 `EngineHealth` contract (probe body is P4)

- [ ] **P2.110** [RUST] Author the `EngineStatus` type (`id`/`present`/`integrity_ok`/`runnable: Option<bool>`) · §7.2.3 §0.6
  needs: P2.13, P1.25
- [ ] **P2.111** [RUST] Author the `EngineHealth` type (`engines`/`unavailable_targets`/`all_critical_ok`) — one row per registry-eligible engine · §7.2.3 §0.6
  needs: P2.110, P2.8.3
  - [ ] **P2.111.1** [DOC] Record the non-trait-binary roll-up rule (`FFprobe`→FFmpeg, `ImageMagick`→`ImageCore`; no standalone `EngineStatus` row) · §7.2.3 §0.6
  - [ ] **P2.111.2** [DOC] Record the `NativeCsvTsv` synthesized always-available `EngineStatus` (appended after the loop, never from it) · §7.2.3 §3.5.6
- [ ] **P2.112** [RUST] Author the `AppInfo` type (C11 return) — version/build_id/platform/third_party_notice · §7.2.3 §0.6
  needs: P2.110, P4.3
  > **Forward-ref note (DECISION-C cross-phase inversion):** `needs: P4.3` points at the §3.2 `Platform` enum owner in a later phase (the spec homes `Platform` in §3.2; P4.3 authors `Platform::{Win,MacOS,Linux}`) — `AppInfo.platform: Platform` (§7.2.3) has no type to land until P4.3 exists, so DECISION C builds P4.3 first; the edge is acyclic and valid, the inversion documented at the `needs:` line.
- [ ] **P2.113** [RUST] Wire C12 `get_engine_health` to return the cached `EngineHealth` (the cache is populated by the P4 probe; contract type-shared now) · §0.4.1 §7.2.3
  needs: P2.111, P2.21
- [ ] **P2.114** [UI] Author the typed `EngineHealth` → `unavailable_targets` store-selector seam (contract plumbing only; the visual disable-with-reason tiles are P4.70.2) · §7.2.3 §5.1
  needs: P2.113, P1.27
  > **contract seam only, no engine behaviour** (P2 boundary: "no engine spawn, no conversion, no corpus"; the cache C12 reads is empty until the P4 probe populates it, P2.113 note). Author the typed §5.1-store-shape selector/façade surfacing `EngineHealth.unavailable_targets: Vec<TargetId>` to the FormatPicker layer over the generated `commands.ts`/`bindings.ts` C12 path (P1.27 façade) — the read seam later consumers bind against. It does **NOT** render anything: the visual **disable-with-reason** FormatPicker tiles (the §5.2 surface — `aria-disabled` on the §3.4 patent-gapped/unavailable tiles) are built for real in **P4.70.2**, fed by the P4.45 `EngineHealth` population, exactly as **P5.32 says "P4 owns the wiring; this box consumes it"**. So this box is the type-shared store-shape seam (buildable now with no backing data), not the disable UI (which has nothing to disable until P4). (`needs: P2.113` for the C12 return + `P1.27` for the IPC façade the selector reads through.)

## §7.8.2 explicit negatives (DoD gate 20)

- [ ] **P2.115** [DOC] Record the no-file-association / no-default-handler-claim negative (no `.heic`/`.docx` handler registration) · §7.8.2
- [ ] **P2.116** [DOC] Record the no-URL-scheme / no-deep-link negative (no `convertia://`, no deep-link plugin) · §7.8.2
- [ ] **P2.117** [DOC] Record the no-drag-out / no-clipboard-export negative (parked under Future Ideas; WebView cannot originate a real path drag) · §7.8.2
- [ ] **P2.118** [DOC] Record the no-service / no-login-item / no-shell-extension negative (no Explorer/Quick-Action integration) · §7.8.2
- [ ] **P2.119** [GATE] Assert the §7.8.2 negatives structurally (no deep-link block, no URL-scheme registration under `src-tauri/`) — the DoD-gate-20 enforcement · §7.8.2 §0.10 · G47
  needs: P1.24, P2.116

## Shell-level a11y, English-only, UI-async & IPC-responsiveness contracts

- [ ] **P2.120** [UI] Wire the frontend async model to the generated `commands.*` / `ConversionEvent` Channel + the three `app://` listeners (§5.8) — feeding the §5.1 store live-progress map + the `pendingVideoReencodeNote` field · §5.8 §0.4.2 §5.1
  needs: P1.27, P2.37, P2.39, P1.31.2
  > the §5.8 async wiring populates the §5.1 store (typed shape homed in P1.31.2): the live-progress map from the `ConversionEvent::ItemProgress` Channel, and the **`pendingVideoReencodeNote`** field from the `RunStarted.willReencode` signal (§0.4.2/§5.8) — the worst-case `video_reencode` ConvertingNote banner P8.20 reads + P4.65 surfaces. P1.31.2 owns the typed field; this box owns the population.
- [ ] **P2.121** [UI] Wire the native drag-drop affordance (hover/visual only; paths arrive over the native event → C1, never the DOM drop) · §5.4 §0.4.0
  needs: P2.120, P2.22
- [ ] **P2.122** [UI] Establish the app-chrome a11y baseline (ARIA roles/focus order on the shell — the per-push `vitest-axe` target) · §5.5 · G33a
  needs: P2.120
- [ ] **P2.123** [UI] Enforce English-only / string-ownership on the shell (every user-facing literal in `strings/ui.ts`, no i18n-runtime import) · §5.5 · G57
  needs: P1.37, P2.120
- [ ] **P2.124** [UI] Wire the backend-disconnect / mid-run IPC-drop handling to `AppFault` (the §5.8 app-fault surface) · §5.8 §2.13
  needs: P2.120, P2.39
- [ ] **P2.125** [TEST,RUST] Assert the IPC-responsiveness invariant — no synchronous C-command blocks the WebView past a bound (grouping shell) · §0.4 §1.1 §1.11
  needs: P2.36, P2.38
  > the WebView-side analogue of the per-engine watchdog (the §0.4 C6 "return immediately, stream" model + the platform 100s-timeout discipline): assert no synchronous C-command can wedge the UI. The two independent assertions target different commands and fail independently, so they are split into separately-faileable sub-boxes; the parent is `[x]` only when both are (_format.md §2). (The per-ENGINE wall-clock/watchdog timeouts are P3.44/P4.12; this is the C-command-surface responsiveness contract.)
  - [ ] **P2.125.1** [TEST,RUST] Assert the C1 scan-path streams `ScanProgress` on a large folder (never blocks until the whole walk finishes) · §1.1 §1.11 · G31
    needs: P2.38
    > a large-folder C1 `ingest_paths` streams `ScanProgress { scanned }` over its `onScan` Channel (P2.38) rather than blocking until the whole walk completes; a test drives a synthetic large-folder C1 and asserts progress events arrive (the UI is never frozen during a deep recursive walk).
  - [ ] **P2.125.2** [TEST,RUST] Assert C3 `get_targets` / C4 `plan_output` (incl. §1.10 preflight) return within a bounded budget · §0.4 §1.11 · G31
    needs: P2.36
    > C3 `get_targets` / C4 `plan_output` (incl. the §1.10 preflight) and a huge-folder C1 ingest return within a bounded budget or yield cooperatively, never a frozen WebView; a test drives a slow-preflight C4 and asserts the call returns bounded.

## P0 activation targets (the cross-cutting security-test homes P0 points into P2)

> Two P0 boxes carry `→ activated in P2` / `→ activated in P3/P4/P9` edges that point
> into the C1–C13 surface + the logging infra P2 builds: P0.4.3's per-`#[tauri::command]`
> serde-boundary + per-numeric-IPC-arg overflow legs, and P0.5.9's §7.5 log-redaction
> property gate. These boxes are the concrete activation targets those P0 homes resolve
> against — each names the P0 box-id so the cross-ref is plan-lint-detectable (the
> P3.67→P0.5.8 pattern).

- [ ] **P2.126** [TEST] Instantiate the P0.4.3 serde-boundary fuzz + per-numeric-IPC-arg overflow legs over the now-real C1–C13 commands · §0.4.3 §1.1 · G48 G16
  needs: P2.36, P0.4.3
  > the activation target for the P0.4.3 `→ activated in P2 as C1–C13 land` edge: now that C1–C13 exist (P2.21–P2.35, surface-complete at P2.36), instantiate both legs using the P0.4.3 harness layout — **(a)** the cargo-fuzz serde-boundary target over **each** `#[tauri::command]` (malformed `serde_json` at the IPC boundary → a structured `Err`, **never** a panic across the Tauri boundary) and **(b)** the per-numeric-IPC-arg arithmetic-overflow `proptest` (boundary values `u32::MAX`/`i32::MIN`/0/1/2^16-1 → a structured `Err`, the T10 `arithmetic_side_effects`-deny companion). This is the P2 box the P0.4.3 `→ activated in P2` edge points at (`needs: P2.36`; the P0.4.3 harness/contract is `[x]` before the loop). → activates the P0.4.3 serde-boundary + per-numeric-IPC-arg legs.
- [ ] **P2.127** [TEST] Stand up the §7.5 log-redaction property gate — a secret-shaped path stem through the configured logger is absent from output · §7.5 §2.11 · G31
  needs: P2.94, P2.94.1, P0.5.9
  > the activation target for the P0.5.9 §7.5 log-redaction home (this is the **P2 leg** — the §7.5 log-redaction property gate's home, resolved HERE in P2 where the logging infra lands; the P0.5.9 isolation/privilege-drop arm activates in P4, the egress-window/sentinel arms in P9 — those are SEPARATE P0.5.9 homes, NOT this redaction gate): feed a **secret-looking path stem** (a value matching the gitleaks minisign-secret-key / generic-secret shape, plus a full file path) through the **configured `tauri-plugin-log` logger at verbose level** (P2.89/P2.94) and assert the secret + the full path are **absent** from the rotating-file + stderr output (the §7.5.3 redaction stance P2.93 asserts as a STANCE — this is the property test that proves it fires). Distinct from the egress-window sentinels (P9.x), which exercise out-of-input reads, not logger redaction. → this is the P0.5.9 log-redaction activation target (`needs: P2.94`, the verbose-mode/logger box; the P0.5.9 home is `[x]` before the loop).
