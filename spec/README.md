# ConvertIA — Technical Specification

> The complete technical specification for ConvertIA, derived from the
> [Single Source of Truth](../SINGLE-SOURCE-OF-TRUTH.md) (SSOT). The SSOT remains
> authoritative on **what & why**; this spec defines **how**.

## Status & rules of engagement

- **Living document.** Unlike the SSOT, this spec is expected to be refined and
  referenced *during* development — sections get adjusted as implementation
  reveals detail. The SSOT does **not** change for that; it stays the single
  source of truth.
- **Conflict rule:** if the spec ever contradicts the SSOT, the **SSOT wins** and
  the spec is corrected.
- **Derivation:** Phase 3 (the implementation TODO/plan) is derived from this
  spec, so it must be **complete** — every behaviour the SSOT promises has a
  technical home here.
- **Scope:** technical specification of the *software*. **Out of scope:**
  distribution/store logistics, developer accounts, code-signing/notarization
  processes (see SSOT *Explicitly Out of Scope*) — **except** where they impose an
  in-code requirement (e.g. generating an SBOM, producing release checksums).

## Structure / reading order

| # | File | Covers (SSOT origin) | Maps to A/B/C/D |
|---|------|----------------------|-----------------|
| 00 | [architecture](00-architecture.md) | System architecture, Tauri model, IPC, project layout, domain model, tech stack | **A** |
| 01 | [conversion-pipeline](01-conversion-pipeline.md) | Detection, queue, batch rules, job lifecycle, engine-invocation model, progress, cancellation | **B** |
| 02 | [guarantees](02-guarantees.md) | Implementation of the SSOT hard guarantees (no-harm, atomicity, fail-clearly, output destination, security/isolation) | **B** |
| 03 | [engines-and-bundling](03-engines-and-bundling.md) | Engine registry/selection, bundling (all offline), per-platform packaging, licence surfacing (NOTICE/SBOM) | **B** |
| 04 | [formats/](04-formats/README.md) | Per-category format matrix — detection, targets (both directions), engine, options, lossy notes | **C** |
| 05 | [ui-ux](05-ui-ux.md) | Frontend architecture, screen states, components, design system, accessibility, IPC integration | **D** |
| 06 | [build-test-release](06-build-test-release.md) | Build matrix, checksums/releases, SBOM, repo-policy artifacts, release gates, test strategy & real-world corpus | A+B+C+D (spans all) |
| 07 | [app-shell](07-app-shell.md) | ConvertIA as a running app: instance/run identity, lifecycle, persistence, logging, update posture | **A** |

_Legend — **A** Architecture & app shell · **B** Core engine & guarantees · **C** Format coverage · **D** UI (these are the Phase-1 A/B/C/D buckets; 06 spans all). **Read 00 and 07 together** — 07 is A-track foundational despite its file number._

## Conventions

- **Decision tags:** `[DECIDED]` (fixed here / by the SSOT), `[OPEN]` (a genuine
  unresolved owner-level call — collected in the log below), `[DEFER: …]` (design is
  decided; only an empirical number or a real-world validation remains).
- **SSOT references** by section *name* (e.g. *Never harm the original*).
- Code/identifiers in English; this doc in English (public OSS repo).

## Parked decisions inherited from Phase 1 (the "how" seeds)

- **Framework:** Tauri (Rust core + React/TS/Tailwind/Vite UI). `[DECIDED]`
- **Engine delivery:** bundle **everything**, fully offline, no runtime fetch. `[DECIDED]`
- **Licensing mechanism:** copyleft engines shipped as **separate, independently
  invoked binaries** (aggregation, not linking) so the MIT core stays clean;
  NOTICE/third-party-licenses + SBOM. `[DECIDED]`

## Open-questions log

> Kept honest after the convergence pass. `[DECIDED]` = resolved (one-line
> rationale); `[DEFER: corpus]` / `[DEFER: …]` = the *design* is fixed and only an
> empirical number/validation remains; `[OPEN]` = a genuine unresolved owner-level
> call. After this pass the vast majority are decided or deferred.

#### Resolved in the second synthesis-fix pass (6 blockers + sound SHOULDs) `[DECIDED]`
- **FFmpeg static-vs-dynamic linkage `[DECIDED]`** — the §3.6.1/§3.9.1 "a static FFmpeg
  would FAIL the §6.1.3 dynamic-link assertion" claim contradicted §6.1.3 carve-out **iii**
  (a static GPL FFmpeg with LGPL libs baked in is GPL-clean aggregation that never fails the
  assertion; the GPL's corresponding-source subsumes LGPL §6). Resolved in favour of
  carve-out iii: FFmpeg may be static OR dynamic; v1 ships dynamic-beside-the-exe as an
  **engineering preference**, not a licence-mandated rule; the LGPL dynamic-link assertion
  applies ONLY to LGPL linked into the MIT core (carve-out i). Owner: §6.1.3 / §3.6.1 / §3.9.1.
- **CSV/TSV in-process progress = `ProgressModel::InProcessFraction` + `mpsc::Sender<f32>`
  `[DECIDED]`** — the enum had only 3 variants (none covering the in-process self-reported
  fraction) and §1.7 named no IPC. Added a 4th variant; §1.7's `InProcessNative` sub-case now
  passes the executor a bounded `tokio::sync::mpsc::Sender<f32>` (`blocking_send` per N-KB
  chunk, verified current Tokio API) which §1.7 forwards as `ItemProgress`. Owner: §3.2.2 /
  §1.7 / §1.11 / §3.5.6.
- **Screen-reader path now has an implementable contract (Principle 10) `[DECIDED]`** — new
  §5.6.1 enumerates the mandatory per-component ARIA role, the assertive-announcement states,
  and the per-state SR traversal order through the 12-state machine; §6.6 is the verification
  gate that walks it. Owner: §5.6.1 / §6.6.
- **English-only UI now has an owning section + CI gate (Principle 11) `[DECIDED]`** — §5.7
  states v1 ships English-only strings with no i18n runtime; §6.10 row 23 + a Lane-A
  Principle-11 lint (no locale-switch library import; every `strings/ui.ts` key resolves to a
  non-empty English value) make it machine-checkable. Owner: §5.7 / §6.10.
- **BatchSummary vs FileList skipped-list ownership `[DECIDED]`** — BatchSummary owns the
  passive one-line tally only; **FileList** ("Show N files" disclosure) is the SINGLE owner of
  the expandable per-item skipped rows (no duplicate inline list). Owner: §5.3.
- **Images BMP-column alpha-flatten `[DECIDED]`** — every alpha-capable source → BMP
  (PNG/WEBP/GIF/TIFF/HEIC/AVIF/ICO) is now `✓~` (matrix matched to its own alpha-flatten
  prose and the parallel JPG column); JPG→BMP stays `✓` (no alpha). Owner: images.md.
- **image→GIF dither default `[DECIDED]`** — promoted the stale images.md `[DEFER]` to
  `[DECIDED] bayer` (cgif's only mode; parallels the video→GIF `bayer:bayer_scale=5` default).
  Owner: images.md / cross-category.md [OPEN-D].
- **libimagequant guard = lockfile pin, not soname `[DECIDED]`** — since the BSD v2.4.x fork
  is statically vendored in libvips' cgif path there is no runtime soname; the §6.1.3 guard is
  a COPYRIGHT-BSD-text check + a Cargo.lock/engines.lock provenance pin. Owner: §3.1 row 1e /
  §6.1.3.
- **macOS file-open = `RunEvent::Opened` only (NOT `on_open_url`) `[DECIDED]`** — corrected the
  mis-equation with `tauri-plugin-deep-link` (custom-scheme deep links, which never fire for
  the Open-With AppleEvent). Owner: §7.3.2 / §1.1 / §7.8.1.
- **`vitest-axe@0.1.0` verified-real + pinned `[DECIDED]`** — confirmed on npm (Vitest-native
  jest-axe fork, deps `axe-core ^4.4`); pinned in §0.8; the "jest-axe is wrong under Vitest"
  framing corrected (it works; vitest-axe is preferred for ergonomics). Owner: §0.8 / §6.4.6a.
- **Stray code-fence at video.md EOF removed**; **WMA-encoder factual fix** (wmav2 exists but
  is out-of-v1, not "no encoder exists"); **images.md SVG engines row** corrected to the direct
  `rsvg::Loader` security boundary (not libvips `svgload`). Owners: video.md / §3.1 / images.md.
- **Several §5 UI derivability gaps closed `[DECIDED]`** — RerunPrompt Ctrl/⌘+N suppression is
  reducer-level (not focus-trap alone); ProgressList `aria-busy` cleared on terminal; OpenActions
  divert labels are concrete `strings/ui.ts` entries; `pendingVideoReencodeNote` resets on 4→3
  back-nav; Collecting orphaned-focus fallback = the `role=status` scanning region;
  UnsupportedNotice §5.3 controls enumerated + focus-on-Dismiss; native drop in RerunPrompt
  silently ignored; BusyNotice auto-dismiss precisely scoped. Owner: §5.3 / §5.6 / §5.8 / §5.10.
- **Corpus minimum-content gate `[DECIDED]`** — §6.4.5 adds a machine-checkable content floor
  (≥1 CJK-body + ≥1 RTL-body Office doc, ≥1 non-ASCII-encoding CSV/TSV, ≥1 non-Latin-tag audio,
  representative A/V) so an all-ASCII corpus can't pass; backs §6.10 rows 3/15. Owner: §6.4.5.

#### Resolved in the synthesis-fix pass (7 blockers + alignment) `[DECIDED]`
- **C4 never overrides a C5 destination** (was: "C4 freezes after C5") — the contradiction
  with the §5.2 rows-4/5 self-loop + §5.8 debounced re-call is removed: a post-C5
  target/option change still re-runs C4 (nothing goes stale), but the recomputed plan
  carries the C5-resolved destination; destination authority stays with C5. Owner: §0.4.1;
  propagated §5.8 / §5.2 rows 4/5 (no freeze ref) / two resolved-decision entries below.
- **`CollectedNoteKind` — four typed variants have producers; `Other` is a reserved
  extension point** — the false "four variants / no unreachable" claim (enum has five) is
  corrected: the typed four each have a §1.2 producer, `Other` is a forward-compatible
  catch-all emitted by no v1 engine. Owner: §1.2 / §1.4.
- **`tauri-plugin-dialog` added to the §0.8 plugin table** — both C2a/C2b pickers depend on
  `DialogExt`; `tauri_plugin_dialog::init()` registration noted in §7.1 Builder wiring
  (verified vs the v2 plugins-workspace docs). Owner: §0.8 / §7.1.
- **`image_alpha_flatten` wired in images.md** — the §2.9 LossyKind now has its hook: the
  transparency-policy section + the JPG and BMP *As target* entries carry it, and every
  raster→ICO matrix cell is `✓~` (matching the ICO entry's `image_downscale` prose). Owner:
  images.md / §2.9.
- **`CollectedSet::Empty { skipped: Vec<SkippedItem> }`** — was payload-less, dropping the
  per-item skip reasons §1.3 promised to show; now carries them (projected from
  `EmptyReport.outcomes`) so a 2+ all-ineligible drop renders "N files, none convertible (M
  unreadable, K unsupported, …)". Owner: §0.6; propagated §1.3 / §5.2 state-10 / UnsupportedNotice.
- **Minimum body text-size DoD value pinned** — `--text-base = 1rem/16px` is the body floor
  (`--text-xs` is supplementary labels only); §5.6 adds the rule; verified by the §6.6 human
  walkthrough against the §5.5 token minimum (§6.10 row 6 qualified — axe-core does not check
  text size). Owner: §5.5 / §5.6 / §6.10.
- **ICO-save via `magicksave` demoted to `[DEFER: corpus/build spike]`** — ImageMagick's ICO
  encoder 256px/multi-size support is unverified (and libvips' magicksave docs do not list
  `.ico` for save); the §6.1.3 build assertion is gated on the spike, with an in-core Rust
  ICO container assembler (wrapping vips-produced PNG/BMP frames) as the named fallback.
  Owner: images.md / §3.5.5 / §6.1.3.
- **SHOULD-level alignment (same pass) `[DECIDED]`** — **engine-asset cache hosting** =
  `actions/cache` keyed `<engine>-<version>-<triple>` + checksum-verified pinned-URL
  populate/fallback, macOS keeps two per-triple keys per engine for the `lipo` universal build
  (§6.1.3); **Windows network confinement** = AppContainer network-isolation profile / per-program
  firewall (WFP) rule — **NOT** a Job Object (which cannot restrict sockets), §2.12.3/§6.7.3;
  **macOS CI smoke** runs on the build-output dir (no quarantine) + writes to a temp dir (no TCC
  prompt), TCC moved to the §6.6 walkthrough (§6.4.4/§6.4.6); **FFmpeg SSRF floor** reframed as
  **build-time-primary** (network protocol family absent at configure time; `-protocol_whitelist`
  is defence-in-depth, bypassable per CVE-2023-6605), §3.5.1/§6.1.3; **x265-in-libheif** = the
  worker-with-x265-loaded is a **GPL combined work** (separate-process keeps the MIT core clean;
  the relinkable-source bundle covers x265's GPL corresponding-source), §3.6.1/§6.1.3; **x265
  libheif-plugin runtime discovery** via `LIBHEIF_PLUGIN_PATH` (one whitelisted var) or libheif's
  explicit plugin-load API, §3.5.5; **HEIC `effort` exposure** corpus-gated (hidden for HEIC if
  inert), §images.md; **`store:default`** has no per-file scope (convention-scoped), §0.10/§7.4.2;
  **WebView source-path echo (app://intake → C1)** = §0.11 **T2b** (accepted bounded harm,
  freeze-time §1.1 re-validation is the bound); **EXDEV cross-volume intermediate** free-space
  re-checked before the copy (§2.14.3); **§2.6 space-restoration** carve-out broadened to the
  wedged-descendant graceful-cancel case (§2.6.3); **run-end cleanup** enumerates the RECORDED
  `final_dir` set incl. divert/cross-volume dirs (§2.6.2); **§2.3.3↔§2.7.1** subtree ancestor-
  creation ordering cross-referenced; **ffprobe stdout** buffered-and-JSON-parsed, not line-read
  (§1.7); **ErrorKind::MixedDrop** has no IpcError producer (it is the `CollectedSet::Mixed`
  success return) (§0.4.3); **FLAC compression** range corrected to **0–8** (libFLAC max 8;
  FFmpeg 9–12 non-standard), audio.md; **MD→PDF** = no chain-free fallback → parks if the LO 26.2
  gate fails, documents.md; **extract-audio AAC copy** gated at the M4A-target level (not the
  copy/encode branch), cross-category.md; **FFmpeg ENCODER set** now generated-and-asserted like
  decoders (`ffmpeg-required-encoders.lock`), §6.1.3; **UI/UX** five derivability gaps
  (Targets→Confirm back-arrow + 7a node + app://fault wildcard pinned as notes; ConvertingNote
  reads the store, props = `note: string|null`; native-drop-in-non-Idle rule; CommandError
  inline-error slot) + the keyboard/focus/ARIA edge-case cluster (7a/back-nav/RerunPrompt/AppFault),
  §5.2/§5.3/§5.4/§5.6/§5.8; **build/test alignment** (§3.9.1→§3.9.2 ceiling cross-ref; WebdriverIO
  v9 + `@axe-core/webdriverio` pinned in §0.8; macOS single-run-180min operative trigger, 3-run
  average post-v1; `[OPEN-6.1b]`→AppImage-only v1; EngineHealth NativeCsvTsv synthesized not
  loop-derived; warm magic-byte check executables-only; RunEvent::Exit cleanup = idempotent §2.6
  path), §6/§7.

#### Resolved in this round (SVG-API / FAT-exFAT / engine-derivability / NSIS / facts) `[DECIDED]`
- **SVG/librsvg LFR primary control corrected to the REAL librsvg API** — was a
  non-existent `set_load_external_resources(false)` + a self-cancelling base-URL step. Now:
  **load via `rsvg::Loader` with NO `base_file`/base URL** (verified against librsvg's Rust
  API — no base URL ⇒ all local `href`/XInclude refused by construction; remote always
  refused), image-worker calls librsvg directly (libvips `svgload` has no toggle), **no
  base-URL confinement** (a base URL is what re-enables the CVE-2023-38633 surface). librsvg
  ≥ 2.56.3 pin demoted to belt-and-suspenders; §6.1.3 adds an API-presence assertion.
  Owner: §3.5.5; propagated §3.3.4 / §0.11 T9b / §2.11.1 / §6.1.3 / images.md.
- **FAT/exFAT publish gap closed** — on Unix, FAT/exFAT support **neither**
  `RENAME_NOREPLACE` **nor** hardlinks, so the §2.1.2 `link`+`unlink` fallback itself
  fails (no atomic no-clobber primitive). Added a **third fallback = §2.7.2 DIVERT trigger**
  (`DivertReason::NoAtomicPublish`, Unix-only — Windows `MoveFileExW` is fine on FAT/exFAT):
  divert to a hardlink-capable system-disk target. Owner: §2.1.2 / §2.7.2 / §2.14.2-3 / §0.6.
- **Engine-layer derivability (4 blockers)** — **(a)** `EngineId::FFprobe` added (non-trait,
  mirrors ImageMagick — sidecar-path + SBOM + health only); **(b)** `Invocation.out_tmp:
  Option<TempPath>` (`None` for the probe; §1.7 publishes only when `Some`); **(c)** the
  probe Invocation uses `ProgressModel::CoarseSpawnDone` (not FfmpegKeyValue); **(d)** §7.2.3
  out-of-band binary presence loop (iterates the §3.3.1 binary list, NOT the trait registry)
  defines the path for non-trait engines (FFprobe→FFmpeg, ImageMagick→ImageCore health).
  Plus a §3.3.3 `Sidecar(EngineId)`→binary-name table. Owner: §0.6 / §3.2 / §3.3 / §7.2.
- **§1.6 defaults registry `[REC]`→`[DECIDED]`** — CI-generated `OptionDecl.default` index;
  §6.7.1 Lane-A guard fails the build if any §04 pair lacks a default; §6.10 row 7 now
  "owned by §1.6" (de-hedged). Owner: §1.6 / §6.7.1 / §6.10.
- **NSIS NOT shipped v1 `[DECIDED-6.1a]`** — portable `.zip` is the only v1 Windows
  artifact (SSOT portable-first); NSIS deferred post-v1. Resolves the former `[OPEN-6.1a]`.
  Owner: §6.1.2; propagated §6.7.2 / §6.10 row 13 / §0.3.1 / §3.4.5 / §3.9.
- **§6.4.6 macOS WKWebView contradiction fixed** — opener no longer claims a macOS
  WKWebView driver; macOS degrades to the synthetic smoke test (matches the sub-bullet).
  Owner: §6.4.6.
- **Two factual errors fixed** — MPEG-2 US essential patents expired **2018** (US 7,334,248),
  not "~2026" (§3.4.3); HEIC encode uses libvips `heifsave` **integer `effort` 0–9**
  (default 5), **not** an x265 `preset` string (verified vs libvips 8.17 / heifsave.c —
  `speed = 9 - effort`; HEVC steer flows through libheif). Owner: §3.4.3 / images.md.
- **HEVC/H.265 MOV→MP4 default `[DECIDED]`** — re-encode to H.264 (usability-floor); verbatim
  remux = Advanced "keep original quality" toggle. Owner: video.md.
- **§5.2 state diagram** gained the two missing transitions (MixedDropRefusal→Collecting
  re-drop, Rerun→Targets/Destination cancel-Esc); **§5.6 focus-on-entry** rewritten as two
  named `[DECIDED]` rules (FormatPicker default tile on 3→4; Convert button when the
  DestinationBar first appears) — broken sentence fixed. Owner: §5.2 / §5.6.
- **SHOULD-level corrections** — intake/detection panic boundary (C1/C2a `catch_unwind`,
  §2.13.2 / §0.4.0); seccomp NOT the egress block (net namespace preferred, §2.12.3); T3
  runtime tamper-resistance downgraded to corruption-only (§0.11); `EmptyReport` defined +
  projection rule (§1.3); batch-progress denominator = QUEUED-only (§0.4.2); `Recognized.dims`
  step-4 producer named (§1.2); engines.lock→PatentDisposition flow (§3.4.4a); governance-doc
  CI completeness gate (§6.8); bundled-font baseline de-`[OPEN]`'d (documents.md/§6.1.3/§6.4.5);
  `RotationStrategy` fallback fixed (§7.5.2); macOS CI timeout per-OS (§6.7.2); VPS kernel
  recorded + enforcement path (§6.1.4/§6.4.2); §7.2.3 warm-launch magic-bytes check; LGPL §6
  relink obligation (§3.6.2); libimagequant landmine cross-ref (images.md); WMA decode-only
  (§3.1); fs-audit `::error::` annotation (§6.4.2); §5.x cluster (RerunPrompt state-vs-modal,
  MixedDropRefusal Esc focus, file-picker split, `raw_path` display-only, BusyNotice re-focus).

### Resolved this convergence pass `[DECIDED]`
- **Name/trademark clearance verdict = `clear`** — both "ConvertIA" and the public
  "Ne-IA" brand cleared for v1; `docs/name-clearance.md` records it; the §6.9 gate
  (record present + current) is retained and the rename machinery stays dormant.
  Owner: §6.9.
- **HEIC/AAC/H.264 patent disposition** — **ship-bundled on all 3 platforms** (native
  LGPL AAC, x264, libde265 HEVC-decode), isolated per §3.6; the MP4-default-video
  dependency is honored. Owner: §3.4.
- **HEVC *encode* (write HEIC)** — **ship-bundled-isolated (x265), behind the §3.4
  availability flag** so it can flip to `unavailable` (SSOT exception-1) as a config
  change. **The flag is concrete (§3.4.4a):** a **per-platform `available` boolean on
  the codec's `engines.lock` row**; flipping it `false` makes §3.2.3 resolve the pair to
  `PlatformUnavailable` and C12 `get_engine_health` add HEIC to
  `EngineHealth.unavailable_targets`, so §5.2 renders it disabled-with-reason — data,
  not code. HEVC-encode is the **highest patent-exposure** codec in the set (27 000+
  patents, multiple active pools beyond 2027; libheif#591) — **materially riskier than
  AAC/H.264** and the most likely flag-flip; **kvazaar (BSD)** recorded as the
  licence-clean alternative (removes the GPL leg, not the patent exposure). Owner: §3.4.
- **AVIF** — ship-bundled all 3 (royalty-free). Owner: §3.4.
- **Rust↔TS type-sharing = tauri-specta** (+ specta), generated `bindings.ts`, §06
  drift check; specta-only is the documented fallback. Owner: §0.4.5.
- **Supported-OS floor** — Win10 1809+/11; macOS 11+; Ubuntu-22.04-LTS-class
  `libwebkit2gtk-4.1`; x86-64. (Exact build numbers `[DEFER: §6.4 drift matrix]`.)
  Owner: §0.3.1.
- **§0.10 capability allowlist** — **no `shell:allow-execute`** (engines spawn
  Rust-side §3.3.3); **no `dialog:allow-open`** (both C2 pickers open Rust-side via
  `DialogExt`); **no `opener:*`** (C9/C10 call `OpenerExt` internally); `log:default` +
  `store:default` only. Own `#[tauri::command]`s C1..C13 (incl. C2a/C2b) need **no per-command
  permission entry** in Tauri v2 (only plugin commands do). Owner: §0.10.
- **cancel-collect** — command-backed **C13 `cancel_ingest`** (ingest-scoped token);
  the §5.2 Collecting cancel control + §5.10 Esc back it. Owner: §0.4/§1.1/§5.
- **HEIC/AVIF encode code-path** — standardise on libvips `heifsave` (one AV1 encoder,
  libaom; standalone heif/avif dropped). **x265 ships as a dynamically-loaded libheif
  encoder plugin** (never statically linked). Owner: images.md [OPEN-1] / §3.5.5 / §3.6.1.
- **GIF native; BMP/ICO require ImageMagick** — native `gifsave` (cgif, MIT). **libvips
  has NO native BMP or ICO save at any version**, so **BMP (load+save) and ICO (save)
  go through the REQUIRED ImageMagick `magicksave`/`magickload` delegate — ImageMagick
  is a mandatory bundled component, NOT a fallback.** ImageMagick is permissive (not
  GPL). Owner: images.md / §3.1 row 1d / §3.5.5 / §3.6.1.
- **FFmpeg licence class = GPL-2.0+** — the single bundled FFmpeg binary enables
  `libx264` (`--enable-gpl`), so the **whole binary is GPL-2.0+, not LGPL**; shipped as
  a separate invoked binary (aggregation), written-offer-of-source honored, LGPL
  component libs dynamically linked beside it. Owner: §3.1 / §3.6.1.
- **libvips placement = separate image-worker process** — image decode/encode runs
  out-of-process so a hostile-image exploit is contained by the OS process boundary
  like every other engine (resolves the §2.12.4 "all decoders are subprocesses"
  absolute and the T1 isolation). Licence analysis unaffected. Owner: §2.12 / §0.9 /
  §3.5.5 (was [OPEN]).
- **Windows atomic-publish primitive** — the publish is **always** `MoveFileExW`
  **without** `MOVEFILE_REPLACE_EXISTING` (create-only, no 0-byte placeholder). **There
  is NO replacing path:** the §2.5 re-run FreshCopy uses ordinary §2.2 create-only
  numbering (next non-existing name), never replacement, so
  `ReplaceFileW`/`MOVEFILE_REPLACE_EXISTING` have **no caller** (absolute no-clobber
  forbids overwriting an unrelated same-named file). Keeps the §2.1.3 "never a third
  state" invariant true by construction. The §2.2.2 numbering loop uses this **same**
  primitive (bump-suffix-and-retry on `ERROR_ALREADY_EXISTS`), not a `create_new`-reserve.
  Owner: §2.1.2 / §2.5.2.
- **SVG rasteriser = librsvg** — libvips' native `svgload` backend is **librsvg**;
  **resvg is NOT a libvips backend at any released version** and is **dropped** (not
  shipped, not in the SBOM). Owner: §3.1 row 1c / images.md.
- **AVIF decode = dav1d only** — `dav1d` is the AVIF *decode* load module; **libaom is
  encode-only** (via `heifsave compression=av1`). Owner: §3.1 row 1b / images.md.
- **libimagequant in the inventory + SBOM** — added to §3.1 (PNG/GIF palette
  quantisation, inside the image-worker) with SPDX **`BSD-2-Clause`**, shipped **ONLY**
  as the frozen **`lovell/libimagequant` v2.4.x fork** (e.g. v2.4.1), pinned by exact
  version+ref in `engines.lock`. **Upstream libimagequant 4.x is `GPL-3.0-or-later`-or-
  commercial — NOT permissive — and must NOT be bundled** (it would taint the LGPL
  image-worker). A §6.1.3/§6.3.3 build assertion verifies the staged `COPYRIGHT`
  contains the BSD-2 text (fails the build if a GPL leg slipped in). x265 plugin SPDX
  corrected to **`GPL-2.0-or-later`** (compatible with the LGPL-3.0 libheif host).
  Owner: §3.1 / §3.7.2 / §6.3.3 gate.
- **Re-run/EquivKey is destination-INDEPENDENT in v1** — the EquivKey has no
  destination component, so a **C5 `set_destination` never produces a new `rerun`**;
  `DestinationResolved.rerun` is **carried through unchanged** from C4 and C5
  re-evaluates only the destination-volume free-space preflight. A destination-aware
  signal is `[DEFER: post-v1]` with the cross-session ledger. Owner: §2.5 / §0.6 / §1.8.
- **C2 split into two Rust-side pickers `[DECIDED]`** — **no `dialog:allow-open` WebView
  grant** (both opened via `DialogExt`). **C2a `pick_for_intake`** funnels picked paths
  straight into the C1 freeze and returns a `CollectedSet`, so **intake** paths never
  transit the WebView (a cancelled dialog is a clean no-op → `CollectedSet::Empty`).
  **C2b `pick_destination`** returns the chosen **write-destination `PathBuf`** to the
  WebView for C5 — that one path *does* transit the WebView (acceptable per §0.11 T2a,
  bounded by §2.1). The "no raw FS path reaches the WebView" claim is **scoped to the
  intake picker**, not absolute (drop & launch-arg structurally hand paths to the
  WebView; the real bound is core-side re-validation at the §1.1 freeze / §2.3.3 write
  check). Owner: §0.10 / §0.4.1 C2a/C2b / §5.4.
- **C6 destination authority** — **C6's `destination` argument is authoritative**; C4/C5
  are plan/preview + revalidation only, with **no separate server-side destination
  store** (the UI carries the last C5-resolved destination into C6). Owner: §0.4.1.
- **Collecting live count** — fed by an **optional `onScan` `Channel<ScanProgress>`** on
  C1 (≈2/s throttled), a run-telemetry-style Channel, **not** a 4th `app://` event (the
  three-event invariant covers `app.emit`, not command Channels). Owner: §0.4.1/§0.4.2.
- **`crosses_volume` is reactive at the PUBLISH, not pre-planned as a field** — `OutputPlan`
  drops the `crosses_volume` field; `fs_guard::atomic_publish` detects cross-volume
  **reactively on EXDEV / cross-device failure** (§2.14.3) and runs the copy-into-dest-volume
  fallback. **Clarified `[DECIDED]`:** "not pre-planned" means **no plan field**, NOT "no
  pre-engine decision" — *where the engine writes* when a same-volume sibling temp can't be
  created is a pre-engine temp-PLACEMENT decision owned by §2.14.3 at run time. Owner:
  §0.6 / §1.8 / §2.14.
- **`willReencode` emission + wire type** — the core **always emits a definite value**
  (`false` for non-video / non-applicable batches), never omitted. The Rust struct field
  is non-optional `bool`, so the **generated `bindings.ts` type is non-optional
  `willReencode: boolean`** (no `undefined` third state); the §0.4.2 table / §5.8 comments
  no longer show a stale `?`. Consumers still treat any absent as `false` for robustness.
  Owner: §0.4.2 / §5.8.
- **`ItemId` assignment** — assigned at the §1.1 freeze as the stable index of each item
  in the de-duplicated frozen `Vec` of **ALL dropped items (eligible AND skipped alike)**,
  over a **single id space**; `CollectedSet::Single.items` / `.skipped` are id-DISJOINT
  filtered views (never re-indexed from 0), so a `SkippedItem.item` never collides with an
  eligible id and §1.12 projects skipped items into `RunResult.items` clash-free. Identical
  through Batch/Run/events. Owner: §0.6 / §1.1.
- **`EngineDescriptor` (was `struct Engine`)** — the §0.6 capability descriptor is
  renamed **`EngineDescriptor`** to avoid colliding with the §3.2 `trait Engine`; its
  `kind: EngineKind` is **`Subprocess | InProcessNative`** (every third-party engine incl.
  the image-worker = `Subprocess`; only native CSV/TSV = `InProcessNative`) — the **one
  canonical name**, identical to the §3.2 `EngineProgram::InProcessNative` variant (the
  earlier `EngineKind::InCoreNative` spelling and the `EngineProgram::InProcess` spelling
  are both retired in favour of `InProcessNative`). Owner: §0.6 / §3.2.
- **macOS universal sidecar naming** — `--target universal-apple-darwin` resolves a
  **single fat Mach-O `<name>-universal-apple-darwin`**, not two per-arch files.
  **Correction `[DECIDED — verified vs Tauri v2]`:** Tauri **does NOT `lipo` sidecars** (it
  auto-lipos only its own main app binary); it expects the externalBin fat binary to be
  **pre-merged**, so `scripts/stage-engines` `lipo -create`s each sidecar **before**
  `tauri build`. The §6.1.3 intro and the script section now agree (stage-engines lipos,
  Tauri consumes). Dual-arch sourcing fallback (build x86_64-on-arm64 via cross/Rosetta when
  the cache lacks a slice) documented. Owner: §6.1.3.
- **E2E driver = `tauri-driver` (WebDriver), NOT Playwright** — Playwright cannot drive
  a Tauri WebView in CDP mode; use a WebDriver client (WebdriverIO / `webdriver` crate)
  over `tauri-driver`. macOS automated E2E is **`[DECIDED]` a defined degraded smoke test**
  (launch + synthetic-argv conversion + window/output/exit-0 assertions; `tauri-driver` has
  no macOS WKWebView driver), with WebView UX covered by the §6.6 human walkthrough — no
  longer `[OPEN]` (see the resolved-log entry below). Owner: §6.4.6.
- **Offline-observability = hard gate** — the §6.4.6 E2E runs with **egress blocked**
  (Linux `unshare --net` / `iptables DROP`; macOS `pf`; Windows Firewall) **plus** the
  §2.11.4 packet-monitor assertion; any outbound attempt fails the release. Owner:
  §6.7.3 / §6.10 DoD #5.
- **Lane-B Linux corpus runner** — stays on the **self-hosted VPS runner** with a
  dedicated concurrency group / `max-parallel: 1` + nice/cgroup caps so it does not
  starve the four other projects' Lane-A CI; `corpus-large` uses a persistent VPS-local
  LFS cache (Ne-IA org quota for the macOS/Windows legs only). GitHub-hosted Linux is the
  documented fallback. Owner: §6.7.2.
- **Concurrent identical same-session batches** — **accept the documented best-effort
  degradation** (a silent extra numbered copy, never an overwrite); reserving in-flight
  EquivKeys is `[DEFER: post-v1]`. Owner: §2.5.2.
- **OpenActions availability** — **Summary-only (state 8), not mid-run** — the run's
  RunResult-membership set is not final during `Converting`. Owner: §5.2 / §7.7.
- **Exclusive create-only rename primitive named per platform** — Linux
  `renameat2(RENAME_NOREPLACE)` / macOS `renameatx_np(RENAME_EXCL)` (macOS has NO
  `renameat2`/`RENAME_NOREPLACE`) / Windows `MoveFileExW`-without-`REPLACE_EXISTING`. The
  single-call no-replace primitive is chosen **at runtime per destination** (Linux
  `EINVAL` / macOS filesystem lacking `VOL_CAP_INT_RENAME_EXCL` → fall back to
  `link`+`unlink` for that destination; not a static kernel switch); the residual
  `.part` success-window sub-state (§2.1.3) is the `link`+`unlink`-fallback case on
  EITHER Unix OS, not a macOS-always penalty; NFS ambiguous rename → treat as
  name-may-be-taken and re-pick. Owner: §2.1.2.
- **Detection canonical type** — §1.2's `DetectionOutcome` is the one canonical type;
  §0.6's `DroppedItem.detected` carries it; the `DetectedFormat`/`DetectionConfidence`
  pair is retired (one confidence enum, one cardinality). Owner: §1.2 (referenced by §0.6).
- **Empty/Unreadable classification** — intake-time empty/unreadable = **Skipped**
  (pre-flight `SkipReason`, never queued); turn-time-after-freeze unreadable/gone =
  **Failed** (mid-run). Owner: §1.1 / §1.9 / §0.6.
- **Target type name** — §1.5 adopts §0.6's `TargetOffer`/`Target` (the C3 return type);
  `OfferedTargets`/`OfferedTarget` retired. Owner: §0.6 (struct) / §1.5 (logic).
- **`SkippedItem`** — defined in §0.6 `{ item, source, reason: SkipReason }` (NOT
  `ErrorKind` — every SkippedItem is detection-ineligible so it always has a SkipReason,
  making the §1.12 `OutcomeMsg::Skipped` projection a trivial copy; the forward
  `SkipReason → ErrorKind` is the only, one-way, conversion, on the §1.12 projection
  helper); `CollectedSet::Single` carries `skipped: Vec<SkippedItem>`. Owner: §0.6.
- **CollectingId delivery** — the **frontend generates `CollectingId` and passes it as a
  C1 argument** (single-funnel); **no `collecting-started` event** — the §0.4.2 "no
  other events" invariant holds. Owner: §0.4.1 / §1.1.
- **Opener model** — the WebView calls only ConvertIA's own C9/C10 commands, whose Rust
  handlers call `OpenerExt` internally (not capability-gated); **no `opener:*` WebView
  grant**. The real gate is the Rust-side §7.7.3 `RunResult`-membership check (works for
  arbitrary beside-source outputs a static scope could never cover). Owner: §0.10 /
  §0.4.1 / §7.7.
- **Theme persistence** — the §7.4 **3-key** prefs blob persists `theme`; a minimal in-app
  Light/Dark/System toggle is provided (default `system`). Owner: §7.4 / §5.5.
- **macOS unsigned posture** — accepted for v1, **with** the §6.2.4 Sequoia step-by-step
  (blocked first launch → Privacy & Security → "Open Anyway" → per-sidecar quarantine),
  the §2.8 `QuarantinedByOs` error kind, and a mandatory §6.6 Sequoia walkthrough that
  must pass (the unsigned floor depends on the guided recovery working). Owner: §6.2.4 /
  §7.2.4 / §6.6.
- **Ghostscript** — **dropped in v1** (poppler-only PDF→TXT, no AGPL). `[DEFER: re-add
  if corpus shows GS-salvageable PDFs]`. Owner: §3.1/§3.6.
- **Cross-session re-run ledger** — **not in v1** (session-only; signal 1 demoted to
  in-session corroborator only, §2.5.2). `[DEFER: post-v1 hashes-only ledger]`.
  Owner: §7.4/§2.5.
- **Persistence** — ship the **3-key prefs blob** (theme + lastDestinationMode +
  verboseLog), OS config dir. Owner: §7.4.
- **Verbose-log toggle persistence** — `verboseLog` is the **3rd §7.4 prefs key**
  (persisted across launches), not session-only; the earlier "if §7.4 ships" hedge is
  removed (§7.4 is `[DECIDED]`). **Effect timing = read-at-startup → effective next
  launch** (tauri-plugin-log sets verbosity at plugin-init); the About toggle shows
  "applies after restart". Owner: §7.4 / §5.9 / §7.5.
- **Logging** — ship the **local on-disk log + verbose opt-in** (privacy-by-default,
  no network). Owner: §7.5.
- **Instance hand-off while RUNNING** — **refuse-busy** (UI surface = the `BusyNotice`
  Banner, §5.3). Owner: §7.1.
- **Engine integrity verification** — **`[DECIDED]` hash-on-first-launch + cheap warm
  check**; cache = a `engine-integrity.json` marker in the OS config dir (next to, not
  inside, the prefs blob) keyed on `app_version` (re-hash on absent/version-mismatch,
  presence+size/header check otherwise). Owner: §7.2.3 (SSOT DoD gate 19).
- **Sign `SHA256SUMS`** — **yes, project minisign key** (manifest signature, not
  code-signing). Owner: §6.2.
- **CI runners** — **GitHub-hosted mac/win, self-hosted Linux for Lane A** (budget
  note retained). Owner: §6.1.
- **CI engine-acquisition** — **pinned, checksum-verified asset cache**. Owner: §6.1.
- **Corpus storage** — **small CC0/synthetic in-repo + LFS `corpus-large` for the
  full gate**; total size `[DEFER: corpus]`. Owner: §6.4.
- **Bundled-font baseline** — **Liberation + Carlito + Caladea + curated Noto CJK/RTL
  subset**; only CJK breadth `[DEFER: size]`. Owner: §3.9.3.

#### Resolved in this fix pass `[DECIDED]`
- **C2 split into two Rust-side pickers** — **C2a `pick_for_intake`** (→ `CollectedSet`,
  no path to WebView, cancel = clean no-op) + **C2b `pick_destination`** (→ `PathBuf` to
  WebView for C5; that one write-destination path transits the WebView, §0.11 T2a). The
  "no raw path reaches the WebView" claim is **scoped to the intake picker**; drop &
  launch-arg paths still reach the WebView and are re-validated at the §1.1 freeze.
  Owner: §0.4.1 / §0.10 / §5.4.
- **Collected-set registry** — a `State` map `CollectedSetId → frozen CollectedSet +
  roots`, created on C1/C2a, retained through C3/C4/C5/C6, evicted on run start; resolves
  the IPC `collectedSetId` for C3/C4/C5/C6. Owner: §0.4.4 / §0.6.
- **CollectedSummary wiring** — unified into `CollectedSet::Single` (now carries
  `total_bytes`/`roots`/`encoding_hint`/`delimiter_hint`/`notes`); it IS the wire shape
  C1/C2a return; no separate `get_collected_summary` command. Owner: §0.6 / §1.4.
- **Image dims carrier** — `DetectionOutcome::Recognized { …, dims: Option<(u32,u32)> }`
  (header-derived raster w/h, §1.2 step 4) is the §1.10 cheap-estimate input. Owner:
  §1.2 / §0.6 / §1.10.
- **RunId timing** — minted at **start_conversion (C6)**, NOT at the §2.4 freeze (the
  freeze produces the `CollectedSetId`). §7.1.2 corrected. Owner: §7.1.2 / §0.4.1 C6.
- **`OutcomeMsg` / `ConversionErrorKind` / `LossyKind` derive `specta::Type`** and are in
  `collect_types![]` (§06 drift check covers them) — no `any` for `ItemResult.reason`.
  Owner: §2.8 / §0.4.3/§0.4.5.
- **`EngineKind` canonical name = `InProcessNative`** (matches §3.2
  `EngineProgram::InProcessNative`); `InCoreNative`/`InProcess` retired. Owner: §0.6/§3.2.
- **`serialised_only` access path** — `trait Engine` gains `fn descriptor() ->
  EngineDescriptor`; the §0.9 pool reads `registry.engine(id).descriptor().serialised_only`
  before dispatch. Owner: §3.2 / §0.9.
- **Pre-flight SkippedItems ARE in `RunResult.items`** (projected as `ItemResult { state:
  Skipped(reason), output: None, reason: Some(OutcomeMsg::Skipped{..}) }`, counted in
  `Totals.skipped`). The reason rides the skip-shaped `OutcomeMsg::Skipped` variant (§2.8),
  **not** `OutcomeMsg::Failure`, so skip ≠ fail at the type level. Owner: §1.12 / §0.6 / §2.8.
- **PreflightVerdict.up_front_fail is whole-batch only** — per-item too-big/out-of-disk is
  enforced at write-time (mid-run), not an up-front per-item list. Owner: §0.6 / §1.10.
- **§2.1.2 no-placeholder publish is the single mechanism** — the `create_new`-reserve
  bullets removed; "exclusive create" everywhere = the no-placeholder exclusive-rename.
  Owner: §2.1.2.
- **No replacing publish path / `ReplaceFileW` has no caller** — FreshCopy uses ordinary
  §2.2 create-only numbering; Windows publish is always `MoveFileExW`-without-`REPLACE`.
  Owner: §2.1.2 / §2.5.2.
- **§2.3.3 parent-swap race closed by dir-handle-relative publish** — Windows
  `NtSetInformationFile(…, FileRenameInformationEx)` with a `FILE_RENAME_INFORMATION_EX`
  whose `RootDirectory` is the verified parent HANDLE and whose `Flags` bitfield OMITS
  `FILE_RENAME_REPLACE_IF_EXISTS` (the Ex class's no-replace — NOT the boolean
  `ReplaceIfExists` of the non-Ex struct) → `STATUS_OBJECT_NAME_COLLISION`; bounded
  AV-retry on transient NTSTATUS `STATUS_ACCESS_DENIED`/`STATUS_SHARING_VIOLATION`. Unix
  `linkat` / Linux `renameat2(…, newdirfd, …, RENAME_NOREPLACE)` / macOS
  `renameatx_np(…, newdirfd, …, RENAME_EXCL)` (NOT `openat O_CREAT|O_EXCL`).
  Owner: §2.3.3 / §2.1.2.
- **libimagequant = BSD-2-Clause `lovell/libimagequant` v2.4.x fork ONLY** — upstream 4.x
  is GPLv3-or-commercial and must NOT ship; §6.1.3/§6.3.3 COPYRIGHT-text build assertion.
  Owner: §3.1 / §3.6.1 / §3.7.2 / §6.1.3.
- **libvips bundled WITHOUT poppler(GPL)/MuPDF(AGPL)/any GPL-AGPL PDF loader** — keeps
  the image-worker LGPL-only; §6.1.3 positive build assertion. Owner: §3.1 / §3.6.1 / §6.1.3.
- **§3.4 availability flag is concrete** — per-platform `available` boolean on the codec's
  `engines.lock` row; C12 `get_engine_health` reads it into `unavailable_targets`; §5.2
  renders disabled-with-reason. Owner: §3.4.4a / §7.2.3.
- **WebView2-absent portable launch fails before the core runs** — cannot show an in-app
  fault; the "fail clearly" substitute is the §6.2.4 download-page prerequisite note;
  `minimumWebview2Version` is NSIS-installer-only and **NSIS is NOT shipped v1** (§6.1.2
  `[DECIDED-6.1a]`), so this floor-enforcement mechanism is absent in v1 — the download-page
  note is the sole Windows floor mechanism. Owner: §0.3.1 / §6.2.4.
- **Windows portable artifact = a `.zip`** (app exe + `binaries/` + `resources/` engine
  trees, post-build packaging), NOT a single `.exe`; **it is the ONLY v1 Windows artifact —
  NSIS NOT shipped v1** (§6.1.2 `[DECIDED-6.1a]`, deferred post-v1). Owner: §6.1.2 / §6.10
  row 13.
- **Linux log dir = `~/.config/dev.ne-ia.convertia/logs/`** (Tauri v2 `app_log_dir()`
  resolves via `configDir`, not the data dir). Owner: §7.5.2.
- **macOS launch-intake = `RunEvent::Opened { urls: Vec<Url> }`** (real in Tauri v2, the
  `App::run` closure; the **sole** macOS file-open mechanism — **NOT** `tauri-plugin-deep-link`
  `on_open_url`, which is for custom-scheme deep links and does not fire for the Open-With
  AppleEvent) — `file://` URLs → paths before §1.1; one canonical hook across §1.1/§7.8.1.
  Owner: §1.1 / §7.8.1.
- **willReencode note timing** — surfaced at target choice (state 4, C3
  `Target.lossy=video_reencode`); `RunStarted.willReencode` only confirms/clears it.
  Owner: §5.7 / §5.8 / §2.9.2.
- **fs module canonical = `crate::fs_guard`** (layer "guarantees-fs", dir `fs_guard/`);
  `fs_guarantees` module name retired. Path is `crate::fs_guard` **not** `core::fs_guard`
  (in a Rust binary crate `core` is the no_std stdlib crate, so an app module can't be
  named `core`). Owner: §2.0 / §0.7.
- **engine manifest filename = `engines.lock`** (the §3.7.2 `engines.toml` mention fixed).
  Owner: §3.7.2.
- **macOS automated E2E = defined degraded smoke test** (launch + synthetic-argv
  conversion + window/output/exit-0 assertions); WebView UX via §6.6 human walkthrough.
  Was `[OPEN]`. Owner: §6.4.6.
- **Usability-floor tester sourcing** — ≥1 genuine non-dev walkthrough on ≥1 platform;
  owner (developer) may run the other two where no non-dev tester is available (solo/hobby
  project). Was `[OPEN-6.6a]`. **The SSOT §9 gate text is now AMENDED at the source**
  (recorded owner amendment with footnote) to match this wording — so it is no longer a
  spec relaxation of a literal SSOT gate but a spec implementing the amended SSOT. §6.6 +
  §6.10 DoD row 11 match the amended SSOT. Owner: §6.6 (SSOT amendment by the SSOT owner).

#### Resolved in this convergence fix pass `[DECIDED]`
- **Engine network+file control for T9b = always-on argv/build, NOT the OS sandbox** —
  FFmpeg `-protocol_whitelist file,pipe` + network-disabled build (§6.1.3 `ffmpeg
  -protocols` assertion) closes the **SSRF half**; FFmpeg concat **`-safe 1`** (never
  `-safe 0`, rejects absolute/`..` paths) + a curated demuxer set without the playlist/
  manifest dereferencing demuxers (§6.1.3 `ffmpeg -demuxers` assertion) closes the
  **absolute-file LFR half**; pandoc `--sandbox`, LibreOffice profile-hardening (no remote/
  OLE link auto-update). The §0.11 threat split into **T9a** (app's own code opens no
  socket — structural) and **T9b** (a bundled engine coerced out on hostile input —
  argv/build, **both halves**, + §6.4.2 adversarial-egress case which checks zero egress
  AND no out-of-input file read). The OS network/FS-restriction (§2.12.3) is defence-in-
  depth only — **no longer load-bearing for the LFR half** (the earlier over-claim that
  argv/build alone needed the OS tier for absolute-file LFR is corrected here).
  Owner: §3.5.1/§3.5.2/§3.5.4 / §0.11 / §2.11 / §6.1.3.
- **`.svgz` in-core inflate = pure-Rust `flate2 rust_backend`/miniz_oxide**, ≤64 KiB +
  ≤100× ratio cap; §2.12.4 absolute reworded to "no third-party **C/C++** decoder in-core"
  (the three bounded pure-Rust sniffs don't violate it). Owner: §1.2 / §2.12 / §0.8.
- **Resource pre-flight free-space = PER-PHYSICAL-VOLUME, split by category** — `est_output`
  + the publish temp checked against each item's `final_dir` volume; `est_scratch` (kind-2
  LO profile / FFmpeg two-pass temp) checked against the system/scratch volume
  (`app_local_data_dir`), which is **not** necessarily the destination volume. Requires
  headroom on every physical volume the batch touches (refines the earlier
  per-destination-volume DECIDED, which mis-attributed kind-2 scratch to the destination).
  Owner: §1.10 / §2.14.4 / §0.6.
- **externalBin sidecar runtime path** = bare name beside the app exe via
  `current_exe().parent()` (Tauri strips the target-triple suffix on bundle; the suffix is
  build/stage-time only); `BaseDirectory::Resource` is for resources-tree binaries only.
  Owner: §3.3.3 / §3.2.2.
- **`RotationStrategy` API fact corrected** — three variants `KeepAll | KeepOne |
  KeepSome(usize)` (no `KeepN`); we keep `KeepOne` (on-disk max ≈ `max_file_size`, single
  file, old deleted on rotation). Promoted `[REC]`→`[DECIDED]`. Owner: §7.5.2.
- **`Invocation` (plan, §3.2.2) vs `EngineInvocation` (dispatch envelope, §1.7)** — the
  latter wraps `(JobId, EngineId, Invocation, CancellationToken)`; duplicated argv/cwd/env
  removed from §1.7. `TempPath = tempfile::TempPath`. `InvocationResult` carries the
  Rust-internal `ConversionErrorKind`; the orchestrator (`crate::run`) maps it to wire
  `ErrorKind` via `ErrorKind::from(kind)` at the **§1.9 Running→Failed transition** (the
  `From<ConversionErrorKind> for ErrorKind` impl is owned by `crate::outcome`; identity
  under the §2.8 type-alias mechanism) and at the §0.4.3 IPC boundary — one conversion,
  call-site `crate::run`, definition-site `crate::outcome`. Owner: §1.9 / §3.2.2 / §1.7.
- **`OutcomeMsg::Skipped { reason: SkipReason }`** added — a pre-flight skip rides a
  skip-shaped variant (not `Failure`), so skip ≠ fail at the type level. Owner: §2.8 / §1.12.
- **process-wrap = the maintainer-described successor to command-group** (was wrongly
  "not a successor"). Owner: §1.7.
- **UI component homes added** — `AppHeader` (BrandLogo + ThemeToggle + About), `BusyNotice`
  (refuse-busy Banner, §7.1.1), `ThemeToggle`, and the DropZone **choose-folder** affordance;
  **no `Toast` primitive** ("Toast?" resolved → Banner). C3/C4/C5 **IPC call-timing** pinned
  (§5.8). AppFault→Idle diagram arrow relabelled "Start over". Owner: §5.3 / §5.5 / §5.8.
- **Automated a11y gate** = axe-core via vitest-axe (Lane A, §6.4.6a/§6.7.1): WCAG 2.1 AA
  contrast (both themes), ARIA-role validity, focus-order. Owner: §6.4.6a / §6.10 row 6.
- **minisign signature is unconditional/DECIDED** (the "optional"/"if it ships" hedges
  removed, §6.2.3/§6.2.4/§6.10 row 12). **CycloneDX `specVersion 1.5` pinned for ALL SBOM
  inputs** (§6.3.1). **engines.lock SPDX fixed**: x264 `GPL-2.0-or-later`, poppler
  `GPL-2.0-only OR GPL-3.0-only`, libaom `BSD-2-Clause AND LicenseRef-AOMPL-1.0`
  (the AOM Patent License has **no registered SPDX id** — `AOMPL-1.0` is only a pending
  SPDX request — so it is a `LicenseRef` custom licence with full text in
  `THIRD-PARTY-LICENSES.txt`; the §6.3.3 gate gains a `LicenseRef`-with-text carve-out
  so it is not a `NOASSERTION` hard fail) (§3.1). Owner: §6.2 / §6.3 / §3.1.
- **verboseLog effect timing = read-at-startup → next launch** (About toggle: "applies
  after restart"). **Engine-integrity cache** = `engine-integrity.json` in the config dir,
  keyed on `app_version`. **First-launch macOS `Opened` buffer-then-replay** (avoids the
  listener race). Owner: §7.5.3 / §7.2.3 / §7.8.1.

#### Resolved in this round `[DECIDED]`
- **`IpcError` / `ErrorKind` derive `specta::Type`** (were commented out) + `ScanProgress`
  derives `specta::Type` — all in `collect_types![]`; no `any` for errors or the C1 scan
  Channel. The §0.4.3/§2.8 wire-mirror notes corrected to "ALL variants" (item- AND
  run/app-level). Owner: §0.4.3 / §2.8 / §0.4.2.
- **macOS `RunEvent::Opened` Open-with goes through the refuse-busy gate** — the busy check
  is promoted into the shared `forward_launch_intake` funnel both launch hooks call
  (argv callback AND `RunEvent::Opened`), so a mid-conversion Open-with is refused on macOS
  too (no longer bypasses the PRIMARY §7.1.1 gate). Owner: §7.8.1 / §7.1.1.
- **First-launch macOS drain mechanism = C1 re-use on root-shell mount with a concrete
  `drainPending: true` + `paths: []` call** (no dedicated command, no 4th `app://` event);
  the handler consumes `State<PendingIntake>` (its stored `origin`, `LaunchArg`) and freezes
  it, or returns `CollectedSet::Empty` if none; the frontend never holds the buffered paths.
  `PendingIntake` carries the real `origin` (`LaunchArg`), never a hard-coded
  `SecondInstance`. Owner: §7.8.1 / §0.4.1 C1. **`RunEvent::Opened` is macOS-only in Tauri
  v2** (NOT cross-platform — Win/Linux intake is argv/single-instance); the handler is
  registered unconditionally only for code simplicity, never invoked off macOS. Owner: §7.8.1.
- **FFmpeg T9b covers BOTH halves structurally** — SSRF via `-protocol_whitelist file,pipe`
  + network-disabled build; **absolute-file LFR via concat `-safe 1` (never `-safe 0`) +
  curated demuxer set without playlist/manifest dereferencing demuxers** (§6.1.3 `-protocols`
  + `-demuxers` assertions). The §2.12.3 OS tier is no longer load-bearing for LFR.
  Owner: §3.5.1 / §6.1.3 / §0.11 / §2.11.
- **`ItemId` indexes the FULL frozen Vec of ALL dropped items** (eligible + skipped),
  one id space; `items`/`skipped` are id-disjoint filtered views (never re-indexed) — no
  collision when §1.12 projects skipped into `RunResult.items`. Owner: §0.6 / §1.1.
- **`InProcessNative` (native CSV/TSV) lifecycle** — cooperative cancel polled at each
  N-KB chunk boundary (no kill step), a wall-clock timeout guard, runs up to the global
  degree on `spawn_blocking` threads (never blocks the Tokio runtime). Owner: §1.7 / §0.9.
- **Image-worker progress = `VipsStdout`** (renamed from `VipsCallback`) — the separate
  worker process marshals the libvips eval-progress to its stdout `progress=<0..100>`
  key=value, parsed by the §1.7 same line-reader as FFmpeg `-progress`. Owner: §3.2.2 / §3.5.5.
- **Image-worker is a named externalBin** `convertia-imgworker-<triple>` in the §0.7
  `binaries/` tree + §0.3 subprocess box, resolved via `current_exe().parent()` (§3.3.3).
  Owner: §0.7 / §0.3 / §3.5.5.
- **Usability gate = AMENDED SSOT §9** (recorded owner amendment with footnote): ≥1
  genuine non-dev walkthrough overall, owner may run the others — the SSOT text itself is
  changed, no longer a spec relaxation. Owner: §6.6 / SSOT §9.
- **`RunResult.divert_root: Option<PathBuf>`** added — `common_root` (beside-source) + a
  separate `divert_root` cover split outputs (one PathBuf can't carry both); §7.7.3
  membership covers both. Owner: §0.6 / §1.12 / §7.7.3.
- **C4 vs C5 asymmetry enforced** — C4 is callable at any point in state 4 (eager initial
  call with the pre-highlighted default, then re-callable/debounced ~150 ms on any
  target/option change, §5.8) and computes `rerun` + the §1.10 `preflight` verdict; C5
  never recomputes `rerun` and re-evaluates only the destination-volume `preflight`. The
  ONLY ordering rule: **C4 never overrides a C5 destination** — a post-C5 target/option
  change still re-runs C4 (so nothing goes stale), but the recomputed plan **carries the
  C5-resolved destination** in C4's `destination: DestinationChoice` argument; destination
  authority stays with C5. There is no "fires exactly once" and no post-C5 C4 freeze.
  Owner: §0.4.1.
- **`OutputPlan.scratch_dir` → `publish_temp_dir`** (= `final_dir` in v1; the `*.part` is a
  sibling dotfile, not a subdir, §2.14.1); kept distinct from the kind-2 engine scratch
  root. Owner: §1.8 / §0.6.
- **Free-space preflight = PER-PHYSICAL-VOLUME, split by category** — `est_output`+publish
  temp → final_dir volume; `est_scratch` (kind-2) → system/scratch volume; headroom on
  every physical volume (refines the earlier per-destination-volume DECIDED). Owner: §1.10
  / §2.14.4 / §0.6.
- **Publish temp embeds `InstanceId`+`RunId`** (`.convertia-<InstanceId>-<RunId>-<jobId>-
  <rand>.part`) so the opportunistic same-dir sweep resolves the exact owning lock
  cross-instance — never deletes a live foreign instance's `.part`; absent lock ⇒ dead ⇒
  reclaimable **— safe ONLY because of the lock-before-part ordering invariant**:
  `run-<RunId>/.lock` is created + OS-locked BEFORE the run writes its first `.part`, so a
  live in-progress `.part` can never coexist with an absent lock. Owner: §2.14.1 / §2.6.3.
- **Windows publish primitive = §2.3.3 dir-handle-relative `NtSetInformationFile`** for
  every publish incl. the §2.2.2 numbering loop (the bare path-string `MoveFileExW` is only
  the conceptual shape). Owner: §2.2.2 / §2.3.3.
- **Late divert re-checks free-space + path-limit** on the divert volume (not just
  link-safety) before its §2.1 publish — fails the item clearly, never assumes it fits.
  Owner: §2.7.2 / §2.14.4 / §2.10.
- **Native CSV/TSV is a 4th in-core untrusted-byte path** — §2.12.4 absolute reworded:
  it is about third-party C/C++ decoders, not "only sniffs in-core"; the pure-Rust bounded
  CSV transform is acceptable. Owner: §2.12.4.
- **Video probe-then-encode is two sub-invocations of one engine, not a chain** — `plan()`
  stays Pure and returns the `ffprobe` invocation; §1.7 spawns it, parses a typed
  `ProbeOutput`, then calls `Engine::plan_encode(job, out_tmp, &probe)` (a second trait
  method) to build the encode `Invocation` with `duration_us` taken FROM the probe — NO
  in-place `progress.duration_us` struct mutation. Owner: §3.2.1 / §1.7 / §3.5.1.
- **`PPTX → PPT` is `✓~` lossy (`pptx_to_ppt_legacy`)** — legacy BIFF8 can't hold SmartArt
  / modern charts / Morph; **`PPT → PPTX` (modernizing) stays plain `✓`**. New §2.9
  LossyKind added. Owner: presentations.md / §2.9.
- **Format facts corrected** — VP9 CRF range is **0–63** (15–35 is the recommended band, a
  slider validates 0..=63); "libaom encode-only" is a **configuration** (libheif resolves
  dav1d for AV1 decode, §6.1.3 assertion); GIF dither **seam** (cgif Bayer-only on
  image→GIF vs FFmpeg `paletteuse` error-diffusion on video→GIF); **SVG fonts** resolve
  from the **bundled** set (image-worker has no host fonts), not host OS; raw-AAC exclusion
  does **not** dodge the §3.4 AAC patent (M4A re-encode invokes the same encoder); H.264
  hedged to "~2027-11 bulk of the pool; later-filed AVC-essential patents may run to
  ~2030"; §3.1 "five engines" headline drops "+ optional Ghostscript" (GS not shipped) and
  names ImageMagick mandatory; §3.6.1 LGPL row split per-component (libvips/librsvg
  `LGPL-2.1-or-later`; libheif/libde265 `LGPL-3.0-or-later`); §6.1.3 capability assertions
  for `paletteuse` dither set + `webpsave`/`heifsave` `effort`. Owner: video.md / images.md
  / cross-category.md / §3.1 / §3.4 / §3.6.1 / §6.1.3.
- **§06 test-realism corrections** — a11y gate uses **`vitest-axe@0.1.0`** (a real npm
  package, Vitest-native `jest-axe` fork — verified on npm; preferred for Vitest ergonomics,
  **not** because jest-axe is "wrong", which works under Vitest too — §6.4.6a);
  axe under jsdom **can't measure contrast** → WCAG-AA contrast runs on the
  `@axe-core/webdriverio` session, jsdom leg = ARIA/role/focus only; **`tauri-driver` has
  NO macOS WKWebView driver** (safaridriver ref removed — it automates Safari, not a
  WKWebView); the Linux egress snippet gets a `/status` readiness probe + `kill` +
  propagated exit; the Windows egress is a **per-run `New-NetFirewallRule -Program <abs
  path>`** or an **AppContainer network-isolation profile** (NOT a Job Object — that cannot
  restrict sockets), with the §2.11.4 packet-monitor as the real gate;
  Linux runner pinned to **`ubuntu-22.04`** (FUSE2/FUSE3 + glibc drift); `cargo-cyclonedx
  --spec-version 1.5` **verified exposed**. Owner: §6.4.6/§6.4.6a / §6.7.3 / §6.1.4 / §6.3.1.
- **macOS reload-during-run is NOT a supported recovery path in v1** — known open Tauri
  crash (#9933/#12338); C8 idempotent re-serve covers a FRESH post-terminal listener, not
  a mid-stream reload; a mid-run IPC drop surfaces as `AppFault`. Owner: §0.4.4 / §5.8 /
  §6.4.6/§6.6.
- **`MixedDropRefusal` is a full-screen STATE, not a modal** — own active re-drop DropZone
  (disabled-while-converting guard inert), `aria-live=assertive` heading, NOT
  `role=alertdialog` (the `role=alertdialog` decision dialogs are RerunPrompt + QuitConfirm
  only — UnsupportedNotice is ALSO full-screen and AboutDialog is `role=dialog`; superseded
  by the consolidation-pass bullet below); its own
  §5.10 Esc→Idle row. **Summary focus-on-entry** + **AppFault scoped to the run path**
  (pre-run C3/C4/C5 rejections render inline) decided. `BatchSummary.sampleNames` is
  client-derived (not a wire field). Owner: §5.6 / §5.2 / §5.5 / §5.10 / §5.3.
- **`willReencode` generated binding is non-optional `boolean`** — the `?` dropped from the
  §0.4.2 table + §5.8 comments to match the Rust `bool`. Owner: §0.4.2 / §5.8.

#### Resolved in the consolidation pass `[DECIDED]`
- **C4 call-frequency = multi-call in state 4, C4 never overrides a C5 destination** — C4
  is callable at any point in state 4 (eager initial call + debounced re-calls on
  target/option change, §5.8) and computes `rerun` + the §1.10 `preflight`; the one-shot
  "fires exactly once" rule is removed; the ONLY ordering rule is that a post-C5 C4 re-run
  carries the C5-resolved destination (C4 never changes the destination away from the C5
  value — destination authority lives with C5). Owner: §0.4.1 / §5.8.
- **Exclusive-rename primitive named per platform** — Linux `renameat2(RENAME_NOREPLACE)`
  / macOS `renameatx_np(RENAME_EXCL)` (macOS has NO renameat2) / Windows
  `MoveFileExW`-without-`REPLACE`; common `link`+`unlink` fallback; residual `.part`
  sub-state (§2.1.3) is the fallback case on EITHER Unix OS. Owner: §2.1.2 / §2.3.3.
- **Mid-run launch-intake = refuse-busy on ALL platforms** — a macOS `RunEvent::Opened` /
  argv / second-instance arriving while a run is in flight is refused-busy (paths dropped)
  via the shared `forward_launch_intake` funnel; while idle it starts a new frozen set.
  The §1.1 "starts a new batch mid-run" line is corrected. BusyNotice surfaces only on the
  UI defence-in-depth guard; the core-primary path's feedback is the window re-focus.
  Owner: §1.1 / §7.1.1 / §7.8.1 / §5.8.
- **LGPL link assertion scoped by linkage site** — shared-object/relinkable LGPL required
  only where linked into the MIT core (§6.1.3 carve-out i); the separate image-worker may
  statically link the LGPL stack as aggregation BUT must ship the relinkable-source bundle
  (LGPL §6, carve-out ii, asserted by the build); FFmpeg-internal static LGPL is
  aggregation (carve-out iii). Resolves the §6.1.3-vs-§3.5.5 static-link contradiction.
  Owner: §6.1.3 / §3.5.5 / §3.6.1.
- **SVG/librsvg local-file LFR (T9b) closed** — **PRIMARY load-bearing control = load the
  SVG via `rsvg::Loader` with NO `base_file`/base URL** (`read_stream`/`from_data` without
  a base; v1 SVG→raster needs no external resources, fonts bundled). With no base URL,
  librsvg has nothing to resolve a local/relative `href` against, so it refuses ALL local
  `<image href>`/XInclude reads by construction and remote schemes regardless — closing both
  SSRF and local-LFR. The image-worker calls **librsvg directly** (libvips `svgload` has no
  external-resource toggle; only `VIPS_BLOCK_UNTRUSTED`). **No base-URL/scratch confinement
  is used** — supplying any base URL is exactly what RE-ENABLES the CVE-2023-38633-class
  resolution surface (the defence is the *absence* of a base URL, not the confinement of
  one). The **librsvg ≥ 2.56.3** pin (engines.lock + §6.1.3 version assertion) is
  belt-and-suspenders, NOT load-bearing for v1; if a base URL is ever required later, base-URL
  confinement becomes the load-bearing control and must be labelled as such (carrying the
  CVE-2023-38633 residual). §6.1.3 corpus assertion + §6.1.3 API assertion (the pinned crate
  exposes `read_stream`/`from_data` without `base_file`) + §0.11 T9b / §2.11.1 cite the SVG
  control alongside FFmpeg/pandoc/LO. Owner: §3.5.5 / §3.3.4 / §0.11 T9b / §2.11.1 / §6.1.3.
- **Lock-before-part ordering invariant** — `run-<RunId>/.lock` is created and OS-locked
  BEFORE the run writes its first `.part`; the opportunistic same-dir sweep's
  "absent-lock ⇒ reclaimable" rule is safe ONLY because of this guaranteed ordering.
  Owner: §2.6.3 / §2.14.1.
- **§0.7 guarantees-layer module paths + outcome/error naming** — §0.7 maps
  `src-tauri/src/run/` (crate::run), `outcome/` (crate::outcome), `isolation/`
  (crate::isolation); the §2.8 taxonomy module is renamed `outcome.rs` (crate::outcome) to
  match §2.0 — one canonical spelling, no error.rs/outcome clash. Owner: §0.7 / §2.0.
- **`SkippedItem.reason: SkipReason`** (was `ErrorKind`) — all SkippedItems come from
  detection-ineligible outcomes which all have a SkipReason, so the `OutcomeMsg::Skipped`
  projection is a trivial copy; the forward `SkipReason → ErrorKind` is the only
  (one-way) conversion, moved onto the projection helper. Owner: §0.6 / §1.12 / §2.8.
- **Pre-flight-skip Channel emission policy = RunResult-only (no live ItemFinished)** —
  pre-flight-skipped items are NOT emitted as live `ItemFinished{Skipped}` Channel events;
  they appear only in the terminal `RunFinished → RunResult.items`. The `ItemOutcome::
  Skipped` variant is reserved for that projection path. Owner: §0.4.2 / §1.9 / §1.12.
- **Video two-phase plan contract = `plan_encode(ProbeOutput)` trait method** — `plan()`
  returns the probe `Invocation`; the §3.2 Engine trait gains
  `plan_encode(&self, probe: ProbeOutput) -> Invocation`; `duration_us` is provided BY the
  probe output (carried into `plan_encode`), NOT mutated on a prior struct. The §3.5.1
  "sets progress.duration_us" in-place-mutation sentence is removed. Owner: §3.2.1 / §1.7
  / §3.5.1.
- **`lastDestinationMode` read/inject flow** — on startup the frontend reads
  `lastDestinationMode` from `tauri-plugin-store`; it is the default destination arg on
  C4's first Targets-entry call; a stored absolute path is a re-validated "preferred" hint
  (§7.4.1), falling back to beside-source on failure. Owner: §5.8 / §5.5 / §7.4.
- **OpenKind = three variants with full §7.7.1 mapping; Summary split-divert two buttons**
  — `OpenKind { Folder, File, RevealInFolder }` each maps to a concrete OpenerExt call
  (§7.7.1); when `RunResult.divert_root` is `Some`, §5.3 OpenActions renders TWO
  open-folder buttons (beside-source `common_root` + the divert root), both via
  `RevealInFolder`. Owner: §5.3 / §5.2 / §0.6 / §7.7.1.
- **UnsupportedNotice = full-screen state, not a modal** — removed from the
  `role=alertdialog` lists; `aria-live=assertive` heading on entry, focus → DropZone on
  dismiss, its own §5.10 Esc row. AboutDialog moved to `role=dialog` + `aria-modal=true`
  (informational, not an alert); the `role=alertdialog` decision dialogs are now ONLY
  RerunPrompt + QuitConfirm. Owner: §5.6 / §5.7 / §5.10.
- **§6.7.1 Lane A a11y leg = ARIA/role + focus-order only; contrast on Lane B** — the
  WCAG 2.1 AA contrast check runs on the `@axe-core/webdriverio` Lane-B session (jsdom
  cannot measure computed contrast, §6.4.6a); removed from the Lane A bullet. Owner:
  §6.7.1 / §6.4.6a.

#### Resolved in the review-fix pass `[DECIDED]`
- **macOS TCC absolute scoped to READS only** — §7.2.6 fact 2: engines never first-*read*
  a protected source (staged via §3.5.0 scratch), but the §2.14.1 beside-source publish
  `.part` write is the **core's** (never the engine's) and a TCC denial there **fails that
  item** per §2.8; the "a TCC chain-break can never block a conversion" claim is a
  **read-side** claim, not write-side. §3.5.0 carries the write-side scope note. Owner:
  §7.2.6 / §3.5.0 / §2.14.1.
- **Video vs image HEVC/AV1 decode are TWO engines** — image HEIC/AVIF decode =
  libheif+libde265/dav1d (image-worker); video HEVC-in-MOV/MKV + AV1-in-MKV/WEBM decode =
  FFmpeg's **own native `hevc`/`av1` decoders** (GPL FFmpeg binary, never libde265/the
  image module). §3.4.3 matrix split into per-engine rows; §6.1.3 lists `hevc`+`av1` as
  required FFmpeg decoders. Owner: §3.4.3 / §3.4.4 / §3.5.1 / §6.1.3.
- **Curated-FFmpeg decoder set = generated-from-04 manifest** (`ffmpeg-required-decoders.lock`,
  never hand-kept); the documented floor now includes the modern decoders
  `hevc`/`h264`/`av1`/`mpeg4`/`msmpeg4v2`/`msmpeg4v3`/`mjpeg`/`aac`/`vorbis`/`opus` (+ legacy)
  so a literal build can open iPhone-HEVC/AAC/WEBM/AVI sources. Owner: §6.1.3 / §3.1.
- **libaom SPDX = `BSD-2-Clause AND LicenseRef-AOMPL-1.0`** (AOM Patent License has no
  registered SPDX id — `AOMPL-1.0` is only a pending request) + a **§6.3.3 `LicenseRef`-with-
  text carve-out** so it satisfies the "resolved id" gate (not a `NOASSERTION` hard fail);
  full AOM Patent License text in `THIRD-PARTY-LICENSES.txt`. Owner: §3.1 / §3.6.1 / §3.7.2 /
  §6.3.3.
- **Orchestrator ConversionErrorKind→ErrorKind mapping home named** — `crate::run` (the §1.9
  transition owner) calls `ErrorKind::from(kind)` at the Running→Failed transition; the
  `From<ConversionErrorKind> for ErrorKind` impl is owned by `crate::outcome` (identity under
  the §2.8 type-alias mechanism). Owner: §1.9 / §0.4.3 / §1.7 / §2.8.
- **§1.2 in-core sniffs DECIDED (stale `[OPEN — owner §2.12]` tag removed)** — text-encoding
  heuristic + ZIP central-dir peek + `.svgz` bounded inflate stay in-core (memory-safe,
  bounded, no third-party C/C++ decoder, §2.12.4). Owner: §1.2 / §2.12.4.
- **QuitConfirm focus-restore + alertdialog accessible names** — §5.6 focus-restore-on-close
  scoped to the modals WITH a UI trigger (RerunPrompt → Convert button; AboutDialog → About
  control); QuitConfirm (OS-raised, no trigger) returns focus to the underlying `Converting`
  active element. Both `role=alertdialog` elements get accessible names via `aria-labelledby`
  (RerunPrompt "Already converted with these settings"; QuitConfirm "Conversion in progress").
  Owner: §5.6 / §5.3.
- **SVG-source matrix: every SVG→raster cell is `~` (lossy)** — `image_svg_raster` fires for
  every SVG→raster pair incl. the SVG→PNG ★ default; the matrix row marks all raster targets
  `~`; the footnote rewritten (rasterise inherently lossy + target-codec LossyKind on top).
  Owner: images.md (matrix) / §2.9 (kind).
- **§6.4.4 cross-platform test corrected** — the `tauri-driver` WebDriver flow runs on
  Windows + Linux only; macOS WebView-drift is covered by the §6.4.6 degraded smoke test +
  §6.6 walkthrough (no macOS WKWebView driver). Owner: §6.4.4 / §6.4.6.
- **`RunEvent::Opened` is macOS-only in Tauri v2** (NOT cross-platform) — Win/Linux intake
  is argv/single-instance; handler registered unconditionally only for code simplicity,
  never invoked off macOS. **First-launch drain = C1 with `paths:[]` + `drainPending:true`**
  (consumes `State<PendingIntake>`; frontend never holds the buffered paths). Owner: §7.8.1 /
  §0.4.1 C1 / §7.3.2.
- **`RotationStrategy::KeepOne` footprint re-verified at source = ~1× `max_file_size`** —
  the `KeepOne` arm is `fs::remove_file` (deletes, no `.bak`); the lens's ~2x rename-to-backup
  claim was wrong. Owner: §7.5.2.
- **lipo: Tauri does NOT merge sidecars** (verified vs Tauri v2) — it auto-lipos only its own
  main binary; `externalBin` must be a **pre-merged** fat binary, so `scripts/stage-engines`
  does the `lipo`. §6.1.3 intro aligned with the script section; dual-arch fallback documented.
  Owner: §6.1.3.
- **E2E client binding = WebdriverIO (JS)**, not the Rust webdriver/fantoccini crate — because
  `@axe-core/webdriverio` (the contrast a11y gate) is JS-only. Owner: §6.4.6 / §6.4.6a.
- **ThemeToggle keyboard = Tab-reachable only** (no dedicated accelerator) — recorded in §5.10
  with the FileList-disclosure / Convert-more / Reveal-residue rows; Confirm-gate assertive SR
  string + canonical no-warranty About string added. Owner: §5.10 / §5.7 / §5.9.
- **CollectedNoteKind — four typed variants have producers; `Other` is a reserved
  extension point** — §1.2 step 4 adds the ICO ICONDIR count peek (MultiSizeIcon) + the
  audio cover-art tag peek (EmbeddedCoverArt); the bare-variant + `detail` carrier
  convention clarified. The enum has **five** variants: the four typed ones each have a
  declared §1.2 producer, and `Other` is a forward-compatible catch-all **emitted by no v1
  engine** (not an unreachable bug) — the §1.2 "no unreachable variant" claim is scoped to
  the typed four. Owner: §1.2 / §0.6.
- **CycloneDX→SPDX export tool named** = CycloneDX CLI `convert` (`--output-format spdxjson`;
  Syft `convert` fallback), pinned in §3.8. **minisign key-rotation policy** added (announced
  signed commit + retained `minisign-retired.pub` + release-note). Owner: §6.3.1 / §6.2.3.
- **fs-audit fails CLOSED if neither ptrace NOR Landlock available**; Landlock availability
  asserted before relying on it; Lane-B VPS runner kernel version recorded as a prerequisite.
  **`ubuntu-22.04` floor honoured per lane** (VPS-host may differ → `ubuntu:22.04` Docker or
  GitHub-hosted fallback). Owner: §6.4.2 / §6.1.4.
- **librsvg LFR primary control = load via `rsvg::Loader` with NO `base_file`/base URL**
  (`read_stream`/`from_data` without a base; no base URL ⇒ librsvg refuses all local
  `href`/XInclude by construction); image-worker calls librsvg directly (libvips `svgload`
  has no external-resource toggle). **No base-URL confinement** — supplying any base URL is
  what RE-ENABLES the CVE-2023-38633-class surface, so the defence is the *absence* of a base
  URL. **librsvg pinned ≥ 2.56.3** (belt-and-suspenders, not load-bearing for v1) with a
  §6.1.3 version assertion + a §6.1.3 API assertion that the pinned crate exposes the
  no-`base_file` path. Owner: §3.5.5 / §0.11 T9b / §6.1.3 / images.md.
- **New DoD rows + property tests** — §6.10 row 21 (portable/no-system-pollution, Lane-B
  Procmon/strace post-launch assertion) + row 22 (≤400 MB compressed artifact gate, §6.7.2
  step); §6.4.2 macOS staged-source-copy crash-residue case; Xvfb `-nolisten tcp` in the
  egress snippet; macOS Lane-B timeout ladder fires on a single >180-min run for the first two
  releases; §6.6 screen-reader smoke pass (VoiceOver/NVDA/Orca). Owner: §6.10 / §6.4.2 / §6.7.2
  / §6.7.3 / §6.6.
- **Guarantee tightenings** — §2.7 subtree dir-creation mechanism (create-only ancestors,
  full-final-dir link-safety, non-dir-collision fail); §2.6.3 startup-sweep liveness probe is a
  NON-BLOCKING try-lock (`flock LOCK_NB`/`F_SETLK`/`LockFileEx LOCKFILE_FAIL_IMMEDIATELY`);
  §2.14.3 cross-volume fallback temp lives under the per-run scratch root / carries
  InstanceId+RunId; macOS staged-input preflight bounded to PEAK CONCURRENT, not whole-batch Σ;
  §1.1 per-item walk failure skips-and-continues; C2a dialog non-blocking + CollectingId token
  dropped on every exit branch; §0.4.2 app://intake IDLE-path-only; RunStarted.willReencode =
  conservative container-pair worst-case (pre-ffprobe); ConvertingNote carry-over store field
  `pendingVideoReencodeNote`; WebRTC/macOS-privilege-drop accepted-residual framing; T4
  open_path per-file-launch vs per-root-folder-browse split. Owners as noted in each section.

### Deferred to corpus / usability validation `[DEFER: corpus]`
> Design decided; only an empirical number or a real-world validation remains. These
> are **not** open design questions.
- **Resource budget numbers** — "too big" ceiling, memory/handle ceilings,
  per-category heuristics, **headroom margin 1.3×**, **GIF duration cap ~10 s** ship
  as finite starting values, tuned against the §6 corpus. Owner: §1.10 (co-owned
  §0.9 + cross-category [OPEN-F]).
- **Documents `MD→PDF`/`MD→ODT/DOCX` ownership** (LO 26.2 MD import unproven; default
  LO, pandoc fallback) and **`RTF→markup` ownership** (pandoc, LO fallback if too
  lossy). `DOC→markup` is already DECIDED LibreOffice. Owner: documents.md.
- **`*→MD` image policy** — drop-with-note (lean) vs data-URI inline. Owner:
  documents.md.
- **pandoc `--sandbox` data-file check** — confirm the assigned pandoc pairs
  (markup↔markup, `*→HTML --embed-resources`) run under `--sandbox` without needing a
  blocked on-disk data file; if one does, bundle it and pass it explicitly on argv (never
  drop `--sandbox`). Owner: §3.5.4.
- **extract-audio target subset** (MP3★/M4A/WAV/FLAC/OGG; keep OGG?) and **"no audio
  track" up-front probe** (disable-with-reason vs offer-then-fail). Owner:
  cross-category [OPEN-A]/[OPEN-C].
- **to-GIF option scope** (trim: hard-cap / Basic start+duration / Advanced) and
  **default dither** (bayer-vs-sierra2_4a; bayer is the v1 default). Owner:
  cross-category [OPEN-D]/[OPEN-E].
- **Video HEVC-source default `[DECIDED]`** — re-encode HEVC→H.264 by default (honours
  the SSOT mov→mp4 "plays everywhere" usability-floor; the §6.10 row-7 no-required-choices
  gate can verify it), with verbatim remux offered as an Advanced "keep original quality
  (H.265)" toggle; same disposition for AV1-in-MP4. Newly DECIDED on video.md:
  **metadata-strip toggle NOT v1** (preserve; `[DEFER: post-v1]`), **WEBM two-pass &
  AV1-as-WEBM-target NOT v1** (single-pass VP9; `[DEFER: post-v1]`), **HW-encode NOT v1**.
  Still `[DEFER: corpus]` (empirical only): **auto-deinterlace default** (design = yadif
  on for flagged-interlaced) and **MOV-as-target demand** — validate in §6.6. Owner: video.md.
- **Spreadsheets multi-sheet → CSV sheet selection `[DECIDED]`** — **picker defaulting to
  active sheet** (§6.6 confirms the affordance, `[DEFER: corpus]`); **PSV target NOT v1**
  `[DECIDED]`. **XLSX default CSV-vs-PDF `[DEFER: corpus]`** (CSV is the v1 default; validate
  in §6.6). Owner: spreadsheets.md.
- **Audio MP3-source default `[DECIDED]` = WAV** (over FLAC — FLAC-of-MP3 is the misleading
  no-gain case); **MP3→MP3 same-format & surround force-stereo NOT v1** `[DECIDED]`
  (`[DEFER: post-v1]`). Owner: audio.md.
- **Presentations `[DECIDED]`** — notes switch → `ExportNotesPages=true`; bundled font set =
  §3.9.3 baseline (only CJK breadth `[DEFER: size]`). Owner: presentations.md.
- **Documents `[DECIDED]`** — "compress/smaller PDF" toggle & TXT "output encoding" toggle
  both NOT v1 (`[DEFER: post-v1]`); `*→MD` image policy `[DEFER: corpus]` (leans drop-with-note).
  Owner: documents.md.
- **UI `[DECIDED]`** — state store = **Zustand**; patent-gapped target = **disabled-tile-with-note**
  (§9 usability confirms the affordance, `[DEFER: corpus]`). Owner: §5.11.
- **Images defaults — newly DECIDED**: GPS/location-EXIF **preserve** (+ Advanced strip
  toggle); APNG-output → **first-frame-collapse**; ICO non-square → **pad** with
  transparency; wide-gamut→sRGB toggle NOT v1. Still `[DEFER: corpus]`: default Q values
  (JPG 82 / WEBP 80 / HEIC&AVIF 60); **`heifsave effort`
  (integer 0–9, libvips param — NOT an x265 `preset` string) default `5` — but HEIC
  `effort` EXPOSURE is `[DEFER: corpus]`-gated: exposed only if the corpus confirms it
  measurably steers the bundled x265/HEVC path, else HIDDEN for HEIC (no dead control);
  AVIF `effort` stays exposed (libvips-documented as honoured)**. Owner: images.md.
- **OGG/OPUS cover-art round-trip** — cover art for OGG/OPUS is a **FLAC PICTURE
  metadata block** (`-map_metadata 0`), not a video stream (`-map 0:v? -c:v copy` is
  MP3/M4A/FLAC only). Verify the round-trip on the §6.4 corpus; if unreliable, move
  OGG/OPUS to the tag-poor list (`audio_tags_dropped`). Owner: §3.5.1 / audio.md.
- **AAC manufacturer-distribution patent leg** — the Via LA AAC programme nominally
  levies a per-unit royalty on distributing AAC encoder/decoder implementations
  (free/low-volume tier exists). v1 ships FFmpeg's native LGPL AAC, surfaced in NOTICE;
  the decision (ship-bundled, no revenue) stands. Tracked as honest grey area, not an
  open design call (legal-advice items are out of scope). Owner: §3.4.2.
- **Curated-FFmpeg decoder coverage** — `[DECIDED]` **generated-from-04 manifest**
  (`ffmpeg-required-decoders.lock`, never hand-kept): the build parses every codec the
  04 matrices name on the source side and asserts the curated `--disable-everything
  --enable-…` build covers it (`ffmpeg -decoders` build assertion + §6.4.3 per-pair
  tests). The documented floor explicitly includes the modern decoders `hevc`/`h264`/
  `av1`/`mpeg4`/`msmpeg4v2`/`msmpeg4v3`/`mjpeg`/`aac`/`vorbis`/`opus` (+ legacy set) so
  a literal build can open the headline iPhone-HEVC/AAC/WEBM/AVI sources. The only
  remaining `[DEFER: corpus]` part is confirming the generated set is complete against
  the real corpus, not the design. Owner: §6.1.3 / §3.1.

#### Resolved in the consolidation pass (moved off `[OPEN]`) `[DECIDED]`
- **Decoder-isolation v1 sandbox depth per OS — `[DECIDED]` (two-tier model, §2.12.3).**
  The **cheap tier** (process boundary + timeout + minimal/cleared env incl. stripping
  `LD_PRELOAD`/`LD_LIBRARY_PATH`/`DYLD_*` + scratch-cwd + input/tmp-only paths) is the
  **non-negotiable v1 floor on all three OSes**. The **privilege-drop tier**
  (seccomp/Landlock, Seatbelt/`sandbox_init`, restricted-token/AppContainer +
  Job-Object + low-integrity) is **`[DECIDED]` best-effort, silent-degrade** — enabled
  where it works without install-time elevation / portable-build breakage, degrading to
  the cheap tier otherwise — and is **NOT load-bearing** (the T9b network/LFR guarantee
  rests on the always-on argv/build controls §3.5/§6.1.3, not this tier). The only
  residual is the **precise per-OS profile contents** (`[DEFER: tuning]`, not a
  commitment). Owner: §2.12.3.
- **In-core memory-safe sniffs vs the §2.12 isolation boundary — `[DECIDED]` (§2.12.4).**
  The **text-encoding heuristic**, the **Rust ZIP central-directory peek**, and the
  **`.svgz` bounded inflate** (`flate2 rust_backend`/miniz_oxide — pure safe Rust, **no
  C/C++ decoder**; capped ≤64 KiB inflated + ≤100× ratio, §1.2 step 2) **stay outside the
  §2.12 isolation boundary** — all are memory-safe/bounded, none is a full decode, none
  links a third-party C/C++ decoder, so none violates the §2.12.4 "no third-party C/C++
  decoder in-core" absolute (which is worded exactly that way for this reason). Owner:
  §2.12.4 (raised by §1.2). *(§2.12.4 already DECIDED this; moved here off `[OPEN]`.)*

### Genuinely still open `[OPEN]` (owner-level, not yet resolvable)
- **None at the owner level after this round.** The items the prior pass's "None open"
  claim had actually still left open — **NSIS-vs-portable (`[OPEN-6.1a]`)**, **HEVC/H.265
  MOV→MP4 default**, **§1.6 defaults registry (`[REC]`)** — are `[DECIDED]`; the
  synthesis-fix round additionally resolved **`[OPEN-6.1b]` Linux `.deb`** (→ AppImage-only
  v1, `.deb` post-v1), the **engine-asset cache hosting** (→ `actions/cache` keyed
  `<engine>-<version>-<triple>` + pinned-URL fallback), the **Windows network-confinement
  mechanism** (→ AppContainer profile / per-program firewall rule, NOT a Job Object), the
  **`store:default` scope** (no per-file scope — convention-scoped), the **min body
  text-size** (→ `--text-base` = 16px floor), the **WebdriverIO pin** (→ v9), and the
  **`CollectedSet::Empty` payload / CollectedNoteKind `Other` / C4-vs-C5 destination
  authority / `tauri-plugin-dialog` / `image_alpha_flatten` wiring** blockers. This round
  also **demoted `[OPEN-6.1c]` (Linux/Windows arm64 → `[DECIDED-6.1c]`, out of v1 / post-v1
  by demand)** and **`[OPEN-6.8a]` (`GOVERNANCE.md` → `[DECIDED-6.8a]`, not adopted v1)**,
  and **synced the cross-category.md inline body tags `[OPEN-A]/[OPEN-C]/[OPEN-D]/[OPEN-E]/
  [OPEN-F]` to their own §"Open items (honest)" table dispositions** (A/C/E/F `[DEFER: corpus]`;
  D `[DECIDED]`) so no live bare `[OPEN]` remains in the body that the table already settled.
  So the claim is true rather than aspirational. The remaining unknowns are **empirical calibration
  only** (`[DEFER: corpus/build]` — resource-budget digits, the ≤400 MB compressed ceiling
  vs full-CJK+pandoc upper bound, CJK font breadth, the per-OS privilege-drop profile
  contents, the **bundled libvips/libheif HEVC-path `effort` honour (which also gates whether
  the HEIC `effort` control is EXPOSED or hidden, §images.md)**, the **ICO multi-size/256px
  `magicksave` build spike (else the in-core Rust ICO assembler fallback ships, §images.md/
  §6.1.3)**, the **LO 26.2 Markdown-import gate (else `MD→PDF` parks — no chain-free
  fallback, §documents.md)**, and the §2.1 publish-primitive availability spike incl. the
  FAT/exFAT detection) — design-decided, awaiting a measured number or a real-world
  validation, not an owner-level design call.
