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
  change; **kvazaar (BSD)** recorded as the licence-clean alternative. Owner: §3.4.
- **AVIF** — ship-bundled all 3 (royalty-free). Owner: §3.4.
- **Rust↔TS type-sharing = tauri-specta** (+ specta), generated `bindings.ts`, §06
  drift check; specta-only is the documented fallback. Owner: §0.4.5.
- **Supported-OS floor** — Win10 1809+/11; macOS 11+; Ubuntu-22.04-LTS-class
  `libwebkit2gtk-4.1`; x86-64. (Exact build numbers `[DEFER: §6.4 drift matrix]`.)
  Owner: §0.3.1.
- **§0.10 capability allowlist** — **no `shell:allow-execute`** (engines spawn
  Rust-side §3.3.3); **no `dialog:allow-open`** (C2 picker opens Rust-side via
  `DialogExt`); **no `opener:*`** (C9/C10 call `OpenerExt` internally); `log:default` +
  `store:default` only. Own `#[tauri::command]`s C1..C13 need **no per-command
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
- **Windows atomic-publish primitive** — first-time (no-clobber) publish =
  `MoveFileExW` **without** `MOVEFILE_REPLACE_EXISTING` (create-only, no 0-byte
  placeholder); `ReplaceFileW` reserved only for the §2.5 replacing path. Keeps the
  §2.1.3 "never a third state" invariant true by construction. The §2.2.2 numbering loop
  uses this **same** primitive (bump-suffix-and-retry on `ERROR_ALREADY_EXISTS`), not a
  `create_new`-reserve. Owner: §2.1.2.
- **SVG rasteriser = librsvg** — libvips' native `svgload` backend is **librsvg**;
  **resvg is NOT a libvips backend at any released version** and is **dropped** (not
  shipped, not in the SBOM). Owner: §3.1 row 1c / images.md.
- **AVIF decode = dav1d only** — `dav1d` is the AVIF *decode* load module; **libaom is
  encode-only** (via `heifsave compression=av1`). Owner: §3.1 row 1b / images.md.
- **libimagequant in the inventory + SBOM** — added to §3.1 (PNG/GIF palette
  quantisation, inside the image-worker) with SPDX **`BSD-2-Clause`** (the permissive
  leg of the libvips-vendored fork's dual licence — **NOT** BSD-3; verify the shipped
  leg). x265 plugin SPDX corrected to **`GPL-2.0-or-later`** (compatible with the
  LGPL-3.0 libheif host). Owner: §3.1 / §3.7.2 / §6.3.3 gate.
- **Re-run/EquivKey is destination-INDEPENDENT in v1** — the EquivKey has no
  destination component, so a **C5 `set_destination` never produces a new `rerun`**;
  `DestinationResolved.rerun` is **carried through unchanged** from C4 and C5
  re-evaluates only the destination-volume free-space preflight. A destination-aware
  signal is `[DEFER: post-v1]` with the cross-session ledger. Owner: §2.5 / §0.6 / §1.8.
- **C2 picker opens Rust-side via `DialogExt`** — **no `dialog:allow-open` WebView
  grant**; picked paths enter Rust intake directly (the single C1 freeze point) and
  never transit the WebView, closing the asymmetric "WebView hands raw FS paths" door
  (mirrors the opener model). Owner: §0.10 / §0.4.1 C2.
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
  `kind: EngineKind` is **`Subprocess | InCoreNative`** (every third-party engine incl.
  the image-worker = `Subprocess`; only native CSV/TSV = `InCoreNative`). The §3.2
  `EngineProgram::InProcess` is renamed **`InProcessNative`** (native CSV/TSV only — no
  in-process path for any untrusted-byte decoder). Owner: §0.6 / §3.2.
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
  (raised by §1.2).
- **macOS E2E driver under an unsigned build** — `tauri-driver`/`safaridriver`
  cannot cleanly drive an unsigned WKWebView; the macOS *automated* E2E may degrade to
  launch+screenshot, with the §6.6 human walkthrough (which now also tests the Sequoia
  Gatekeeper/sidecar-quarantine recovery) carrying macOS core-flow validation. Owner:
  §6.4.6.
