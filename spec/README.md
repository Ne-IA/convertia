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
  **single fat Mach-O `<name>-universal-apple-darwin`** (Tauri `lipo`-merges), not two
  per-arch files; `scripts/stage-engines` `lipo -create`s each sidecar. Owner: §6.1.3.
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
  Rust-internal `ConversionErrorKind`, mapped to wire `ErrorKind` only at §0.4.3. Owner:
  §3.2.2 / §1.7.
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
  `GPL-2.0-only OR GPL-3.0-only`, libaom `BSD-2-Clause AND AOMedia-Patent-License-1.0`
  (§3.1). Owner: §6.2 / §6.3 / §3.1.
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
- **First-launch macOS drain mechanism = C1 re-use on root-shell mount** (no dedicated
  command, no 4th `app://` event); `PendingIntake` carries the real `origin` (`LaunchArg`),
  never a hard-coded `SecondInstance`. Owner: §7.8.1.
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
  target/option change, §5.8) and computes `rerun` + the §1.10 `preflight` verdict; it
  **freezes after** a C5 on the same collected-set (a C4-after-C5 is a no-op/error); C5
  never recomputes `rerun` and re-evaluates only the destination-volume `preflight`. The
  ONLY ordering rule is the post-C5 freeze — there is no "fires exactly once". Owner:
  §0.4.1.
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
- **§06 test-realism corrections** — a11y gate uses **`vitest-axe` only** (not jest-axe);
  axe under jsdom **can't measure contrast** → WCAG-AA contrast runs on the
  `@axe-core/webdriverio` session, jsdom leg = ARIA/role/focus only; **`tauri-driver` has
  NO macOS WKWebView driver** (safaridriver ref removed — it automates Safari, not a
  WKWebView); the Linux egress snippet gets a `/status` readiness probe + `kill` +
  propagated exit; the Windows egress is a **per-run `New-NetFirewallRule -Program <abs
  path>`** or network-denied Job Object, with the §2.11.4 packet-monitor as the real gate;
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
- **C4 call-frequency = multi-call in state 4, freeze after C5** — C4 is callable at any
  point in state 4 (eager initial call + debounced re-calls on target/option change, §5.8)
  and computes `rerun` + the §1.10 `preflight`; the one-shot "fires exactly once" rule is
  removed; the ONLY ordering rule is the post-C5 freeze. Owner: §0.4.1 / §5.8.
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
- **SVG/librsvg local-file LFR (T9b) closed** — the image-worker configures librsvg to
  refuse ALL external resource loads (no remote href AND no local `<image href>`/XInclude
  out-of-input file read) and stages the SVG into per-job scratch on ALL platforms; §6.1.3
  corpus assertion + §0.11 T9b / §2.11.1 cite the SVG control alongside FFmpeg/pandoc/LO.
  Owner: §3.5.5 / §3.3.4 / §0.11 T9b / §2.11.1 / §6.1.3.
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
- **None at the owner level after the consolidation pass.** All prior `[OPEN]` items are
  now `[DECIDED]` or `[DEFER: corpus]` (above). The remaining unknowns are **empirical
  calibration only** (`[DEFER: corpus/build]` — resource-budget digits, the ≤400 MB
  compressed ceiling vs full-CJK+pandoc upper bound, CJK font breadth, the per-OS
  privilege-drop profile contents) — design-decided, awaiting a measured number or a
  real-world validation, not an owner-level design call.
