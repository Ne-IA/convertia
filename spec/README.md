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
- **`crosses_volume` is reactive, not pre-planned** — `OutputPlan` drops the
  `crosses_volume` field; `fs_guard::atomic_publish` detects cross-volume **reactively
  on EXDEV / cross-device failure** (§2.14.3) and runs the copy-into-dest-volume
  fallback. Owner: §0.6 / §1.8 / §2.14.
- **`willReencode` emission** — the core **always emits a definite value**
  (`false` for non-video / non-applicable batches), never omitted; consumers treat
  absent as `false`. Owner: §0.4.2 / §5.8.
- **`ItemId` assignment** — assigned at the §1.1 freeze as the stable index of each item
  in the de-duplicated frozen items `Vec`, identical through Batch/Run/events. Owner:
  §0.6 / §1.1.
- **`EngineDescriptor` (was `struct Engine`)** — the §0.6 capability descriptor is
  renamed **`EngineDescriptor`** to avoid colliding with the §3.2 `trait Engine`; its
  `kind: EngineKind` is **`Subprocess | InProcessNative`** (every third-party engine incl.
  the image-worker = `Subprocess`; only native CSV/TSV = `InProcessNative`) — the **one
  canonical name**, identical to the §3.2 `EngineProgram::InProcessNative` variant (the
  earlier `EngineKind::InCoreNative` spelling and the `EngineProgram::InProcess` spelling
  are both retired in favour of `InProcessNative`). Owner: §0.6 / §3.2.
- **macOS universal sidecar naming** — `--target universal-apple-darwin` resolves a
  **single fat Mach-O `<name>-universal-apple-darwin`** (Tauri `lipo`-merges), not two
  per-arch files; `scripts/stage-engines` `lipo -create`s each sidecar. Owner: §6.1.3.
- **E2E driver = `tauri-driver` (WebDriver), NOT Playwright** — Playwright cannot drive
  a Tauri WebView in CDP mode; use a WebDriver client (WebdriverIO / `webdriver` crate)
  over `tauri-driver`. macOS remains `[OPEN]` (unsigned WKWebView). Owner: §6.4.6.
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
- **`renameat2(RENAME_NOREPLACE)` fallback** — chosen **at runtime per destination** on
  `EINVAL` (not a static kernel switch), falling back to `link`+`unlink`; NFS ambiguous
  rename → treat as name-may-be-taken and re-pick. Owner: §2.1.2.
- **Detection canonical type** — §1.2's `DetectionOutcome` is the one canonical type;
  §0.6's `DroppedItem.detected` carries it; the `DetectedFormat`/`DetectionConfidence`
  pair is retired (one confidence enum, one cardinality). Owner: §1.2 (referenced by §0.6).
- **Empty/Unreadable classification** — intake-time empty/unreadable = **Skipped**
  (pre-flight `SkipReason`, never queued); turn-time-after-freeze unreadable/gone =
  **Failed** (mid-run). Owner: §1.1 / §1.9 / §0.6.
- **Target type name** — §1.5 adopts §0.6's `TargetOffer`/`Target` (the C3 return type);
  `OfferedTargets`/`OfferedTarget` retired. Owner: §0.6 (struct) / §1.5 (logic).
- **`SkippedItem`** — defined in §0.6 `{ item, source, reason: ErrorKind }`;
  `CollectedSet::Single` carries `skipped: Vec<SkippedItem>`. Owner: §0.6.
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
  removed (§7.4 is `[DECIDED]`). Owner: §7.4 / §5.9 / §7.5.
- **Logging** — ship the **local on-disk log + verbose opt-in** (privacy-by-default,
  no network). Owner: §7.5.
- **Instance hand-off while RUNNING** — **refuse-busy**. Owner: §7.1.
- **Engine integrity verification** — **hash-on-first-launch + cheap warm check**.
  Owner: §7.2.
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
  Skipped(reason), output: None, reason }`, counted in `Totals.skipped`). Owner: §1.12 / §0.6.
- **PreflightVerdict.up_front_fail is whole-batch only** — per-item too-big/out-of-disk is
  enforced at write-time (mid-run), not an up-front per-item list. Owner: §0.6 / §1.10.
- **§2.1.2 no-placeholder publish is the single mechanism** — the `create_new`-reserve
  bullets removed; "exclusive create" everywhere = the no-placeholder exclusive-rename.
  Owner: §2.1.2.
- **No replacing publish path / `ReplaceFileW` has no caller** — FreshCopy uses ordinary
  §2.2 create-only numbering; Windows publish is always `MoveFileExW`-without-`REPLACE`.
  Owner: §2.1.2 / §2.5.2.
- **§2.3.3 parent-swap race closed by dir-handle-relative publish** — Windows
  `NtSetInformationFile(FileRenameInformationEx)` with the verified parent HANDLE as
  `RootDirectory`, `ReplaceIfExists = FALSE` → `STATUS_OBJECT_NAME_COLLISION`; Unix
  `linkat`/`renameat2(…, newdirfd, …, RENAME_NOREPLACE)` (NOT `openat O_CREAT|O_EXCL`).
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
  `minimumWebview2Version` is NSIS-installer-only. Owner: §0.3.1 / §6.2.4.
- **Windows portable artifact = a `.zip`** (app exe + `binaries/` + `resources/` engine
  trees, post-build packaging), NOT a single `.exe`; NSIS is the secondary installer.
  Owner: §6.1.2 / §6.10 row 13.
- **Linux log dir = `~/.config/dev.ne-ia.convertia/logs/`** (Tauri v2 `app_log_dir()`
  resolves via `configDir`, not the data dir). Owner: §7.5.2.
- **macOS launch-intake = `RunEvent::Opened { urls: Vec<Url> }`** (real in Tauri v2;
  `tauri-plugin-deep-link` `on_open_url` the ergonomic equivalent) — `file://` URLs →
  paths before §1.1; one canonical hook across §1.1/§7.8.1. Owner: §1.1 / §7.8.1.
- **willReencode note timing** — surfaced at target choice (state 4, C3
  `Target.lossy=video_reencode`); `RunStarted.willReencode` only confirms/clears it.
  Owner: §5.7 / §5.8 / §2.9.2.
- **fs module canonical = `core::fs_guard`** (layer "guarantees-fs", dir `fs_guard/`);
  `fs_guarantees` module name retired. Owner: §2.0 / §0.7.
- **engine manifest filename = `engines.lock`** (the §3.7.2 `engines.toml` mention fixed).
  Owner: §3.7.2.
- **macOS automated E2E = defined degraded smoke test** (launch + synthetic-argv
  conversion + window/output/exit-0 assertions); WebView UX via §6.6 human walkthrough.
  Was `[OPEN]`. Owner: §6.4.6.
- **Usability-floor tester sourcing** — ≥1 genuine non-dev walkthrough on ≥1 platform;
  owner (developer) may run the other two where no non-dev tester is available (solo/hobby
  project). Was `[OPEN-6.6a]`. Owner: §6.6.

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
- **extract-audio target subset** (MP3★/M4A/WAV/FLAC/OGG; keep OGG?) and **"no audio
  track" up-front probe** (disable-with-reason vs offer-then-fail). Owner:
  cross-category [OPEN-A]/[OPEN-C].
- **to-GIF option scope** (trim: hard-cap / Basic start+duration / Advanced) and
  **default dither** (bayer-vs-sierra2_4a; bayer is the v1 default). Owner:
  cross-category [OPEN-D]/[OPEN-E].
- **Video HEVC-source default** (remux-verbatim vs re-encode-to-H.264; leaning
  re-encode default + remux as an Advanced "keep original quality"), **auto-
  deinterlace default** (yadif on for flagged-interlaced), and **MOV-as-target
  demand** — validate in §6.6. Owner: video.md.
- **Spreadsheets multi-sheet → CSV sheet selection** (active/first/picker; lean
  picker→active) and **XLSX default CSV-vs-PDF** — validate in §6.6. Owner:
  spreadsheets.md.
- **Images defaults to confirm vs corpus**: GPS/location-EXIF strip-vs-preserve;
  APNG-output vs first-frame-collapse (lean collapse); ICO non-square pad-vs-crop
  (lean pad); default Q values (JPG 82 / WEBP 80 / HEIC&AVIF 60); x265 `preset`
  slow-vs-medium for HEIC. Owner: images.md.
- **OGG/OPUS cover-art round-trip** — cover art for OGG/OPUS is a **FLAC PICTURE
  metadata block** (`-map_metadata 0`), not a video stream (`-map 0:v? -c:v copy` is
  MP3/M4A/FLAC only). Verify the round-trip on the §6.4 corpus; if unreliable, move
  OGG/OPUS to the tag-poor list (`audio_tags_dropped`). Owner: §3.5.1 / audio.md.
- **AAC manufacturer-distribution patent leg** — the Via LA AAC programme nominally
  levies a per-unit royalty on distributing AAC encoder/decoder implementations
  (free/low-volume tier exists). v1 ships FFmpeg's native LGPL AAC, surfaced in NOTICE;
  the decision (ship-bundled, no revenue) stands. Tracked as honest grey area, not an
  open design call (legal-advice items are out of scope). Owner: §3.4.2.
- **Curated-FFmpeg decoder coverage** — the `--disable-everything --enable-…` build
  must assert it covers every decoder the 04 matrices reference (`ffmpeg -decoders`
  build assertion + §6.4.3 per-pair tests). Owner: §6.1.3 / §3.1.

### Genuinely still open `[OPEN]` (owner-level, not yet resolvable)
- **Decoder-isolation v1 sandbox depth per OS** — the cheap tier (process + timeout +
  minimal-env + scratch-cwd, incl. stripping `LD_PRELOAD`/`LD_LIBRARY_PATH`/
  `DYLD_*`) is non-negotiable v1; how far the privilege-drop tier (seccomp/Landlock /
  Seatbelt / Job-Object + low-integrity) goes is a real engineering/portability call.
  Owner: §2.12. *(Note: the libvips in-process-vs-worker question is now DECIDED —
  separate image-worker process — and is no longer open.)*
- **In-core text-encoding heuristic / Rust ZIP central-directory peek** — may it stay
  outside the §2.12 isolation boundary (lean: yes, memory-safe/bounded). Owner: §2.12
  (raised by §1.2). *(This is the one genuinely-open isolation-boundary owner call;
  everything else from the prior convergence pass is now DECIDED or DEFER:corpus.)*
