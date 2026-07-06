# 06 — Build, Test & Release

> The technical build/test/release pipeline (software-side only — no store/account
> logistics). Origin: SSOT *v1 Definition of Done*, *Distribution & download
> trust*, *Cross-platform, one product*, *Local/private/offline*, *Engine-license
> policy*.
>
> **This section spans all four tracks (A+B+C+D).** It does not re-own pipeline,
> guarantee, engine or UI content — it owns the **process** that turns the codebase
> into verified, attributed, downloadable artifacts and the **gates** that decide
> a release is allowed. Where another section owns a fact, it is **referenced by
> §number**, never restated:
> - The engine inventory, per-platform packaging, the patent matrix and the
>   NOTICE/SBOM **data generation** are owned by §3 (§3.1, §3.3, §3.4, §3.6, §3.7,
>   §3.8, §3.9). This file owns *when* those run in CI and *that* they gate a release.
> - The hard guarantees under test (no-harm, atomicity, fail-clearly, isolation)
>   are owned by §2. This file owns the **test harness** that proves them.
> - The IPC surface under integration test is owned by §0.4; the Rust↔TS type
>   drift-check tooling is owned by §0.4.5; this file runs the check in CI.
> - The per-pair format facts (engines, defaults, lossy flags) are owned by §04;
>   this file consumes them as the **conversion matrix under test**.
> - The UI flow under the usability-floor walkthrough is owned by §5; the
>   instance/run/log model the tests rely on is owned by §7.

Decision tags: `[DECIDED]` (fixed here or by the SSOT), `[OPEN]` (needs an
owner-level call — fed to the README open-questions log), `[DEFER]` (resolved
during implementation). Recommended defaults for easy `[OPEN]`s are marked
**(recommendation)**.

---

## 6.1 Build matrix `[DECIDED — native-per-platform; artifact formats recommended]`

### 6.1.1 The hard constraint: no meaningful cross-compilation

Tauri links the **native system WebView** (WebView2 / WKWebView / WebKitGTK,
§0.3.1) and ConvertIA bundles **native engine binaries per platform** (FFmpeg,
LibreOffice, libvips, poppler, pandoc, … — §3.1/§3.3). Both make cross-compilation
impractical: Tauri's own guidance is that cross-compiling desktop bundles is not
supported in practice, and our copyleft engines ship as **separate per-OS
binaries** that must themselves be obtained/built for each target. **Decision:
build each platform on its own native CI runner.** No cross-compile; the matrix is
three independent native legs that fan in to one release. This is the documented
Tauri-recommended path (compile per platform on CI).

### 6.1.2 The artifact-per-platform table

One product, **one primary artifact per platform** (SSOT *Cross-platform, one
product*). SSOT *Portable, no installation* / *no system pollution* makes the
**portable / no-installer** variant the canonical download where one exists; an
installer variant is offered only where it is the platform norm and does not
require admin rights for the user to run.

| Platform | Tauri bundle target(s) | Canonical download (portable-first) | Notes |
|----------|------------------------|-------------------------------------|-------|
| **Windows x64** | post-build **zip** (portable) — **`nsis` NOT shipped v1 [DECIDED-6.1a]** | **Portable zip archive (`.zip`)** containing the app `.exe` + the `binaries/` and `resources/` engine trees — the **canonical AND ONLY v1 Windows artifact** ("download, unzip, run"). **NOT** a single `.exe`: the bare `app`/raw-`.exe` target does **not** embed the sidecar engine trees (FFmpeg, LibreOffice, pandoc — §3.3), which must sit **beside** the exe, so the portable artifact is necessarily a folder/zip. It is produced by an **explicit post-build packaging step** (`scripts/stage-engines` + zip), **not** natively by the `nsis` target. | MSI (`wix`) is **not** used — it implies a system install / admin. **`[DECIDED-6.1a]` NSIS is NOT shipped in v1** (resolves the former `[OPEN-6.1a]`): the portable `.zip` is the single canonical artifact, consistent with the SSOT *Portable, no installation* / *no system pollution* posture. NSIS would add a WebView2-bootstrapper wiring burden (§0.3.1) and an installer mode that contradicts portable-first for **no** v1 benefit (the portable zip already runs per-user, no-admin). **NSIS is deferred to a post-v1 convenience** `[DEFER: post-v1]`; if added later it runs **per-user / no-admin** (`installMode: currentUser`) and is the variant where the WebView2 floor/bootstrapper applies (§0.3.1). |
| **macOS (universal)** | `app` (inside) → `dmg` | **`.dmg`** containing a **universal** `ConvertIA.app` (arm64 + x86_64 via `--target universal-apple-darwin`). | One universal artifact covers Apple-Silicon and Intel → honours "one product per platform". Unsigned/unnotarized (SSOT *Out of Scope*) → on **Sequoia (15.x)** the first launch is blocked and the Control-click bypass is gone; the user must use **Privacy & Security → "Open Anyway"**, and **each bundled sidecar is independently quarantined** (the first conversion can hit `QuarantinedByOs`, §2.8/§7.2.4). Step-by-step on the download page (§6.2.4) and About (§5.9); the §6.6 macOS walkthrough tests it on Sequoia. |
| **Linux x64** | `appimage` (**AppImage-only v1**) | **AppImage** — the portable, distro-agnostic, no-install, runs-anywhere artifact (matches SSOT portability best). | `.deb`/`.rpm` are distro-specific *installs* (system pollution). **`[DECIDED-6.1b]` AppImage-only for v1** (resolves the former `[OPEN-6.1b]`): the single canonical Linux artifact is the AppImage, consistent with the portable-first / no-system-pollution posture (same rationale as the Windows portable-zip `[DECIDED-6.1a]`); a `.deb` is **deferred to post-v1, by demand** `[DEFER: post-v1]`. |

ARM Windows and ARM Linux are **out of v1** (SSOT platform scope = Win/macOS/Linux
desktop; no commitment to every CPU arch). **`[DECIDED-6.1c]` Linux arm64 / Windows
arm64 are out of v1** (`[DEFER: post-v1]`, by demand — low demand; resolves the former
`[OPEN-6.1c]`). The supported-OS floor (minimum Windows/macOS/distro
versions, WebView availability) is **owned by §0.3.1** and referenced by the
release notes; it is not re-decided here.

**Compressed-artifact size gate (SSOT Principle 1 "stay light") `[DECIDED]`:** the
packaging step **measures each platform artifact's compressed size and FAILS the build if
it exceeds the §3.9.2 per-platform budget** (≤ 400 MB compressed v1 target,
`[DEFER: corpus/build]` exact digit). The measured sizes are published as a release asset.
This is the actionable owner of "stay light"; the budget itself is owned by **§3.9.2**.

### 6.1.3 How engines bundle per platform (process, not policy)

§3.3 owns the bundling model and §3.4 the patent matrix; **this file owns the
build-time mechanics that realise them**:

- Copyleft engines (FFmpeg, LibreOffice, poppler, pandoc — **Ghostscript: `[DECIDED]` NOT
  shipped v1**, §3.1) are **separate invoked binaries** (§3.6). They are placed under
  `src-tauri/binaries/` (sidecars) and/or `src-tauri/resources/` (engine support
  trees like the LibreOffice program dir + the bundled font set — `[DECIDED]` baseline
  per §3.9.3 / documents.md: **Liberation + Carlito + Caladea + a curated Noto CJK/RTL
  subset** (only the CJK breadth is `[DEFER: size]`), the §6.4.5 corpus font floor),
  and declared in `tauri.conf.json`:
  - **Sidecars** → `bundle.externalBin`. Tauri requires each sidecar to exist as
    `name-<target-triple>[.exe]` (e.g. `ffmpeg-x86_64-pc-windows-msvc.exe`,
    `ffmpeg-aarch64-apple-darwin`); a small build script (`scripts/stage-engines.*`,
    run before `tauri build`) stages and target-triple-suffixes each binary for the
    runner's host triple. For the macOS **universal** build (`--target
    universal-apple-darwin`), Tauri v2 resolves a **single fat Mach-O sidecar named
    `<name>-universal-apple-darwin`** and **expects that file to ALREADY be a pre-merged
    fat binary** — **Tauri does NOT `lipo` sidecars `[DECIDED — verified vs Tauri v2 docs/
    tauri#3355]`** (it auto-lipos only its **own** main app binary; for `externalBin` it
    reads `binaries/<name>-universal-apple-darwin` as-is, or per-arch suffixed files with
    runtime selection). So **the `lipo` is entirely `scripts/stage-engines`' job**, not
    Tauri's: on the macOS leg the stage script must build each per-arch engine
    (`aarch64`/`x86_64`) and **`lipo -create` them into one `<name>-universal-apple-darwin`
    fat binary** for the externalBin slot, **before** `tauri build` runs. (This intro and
    the §6.1.4 dual-arch-engine-sourcing note now agree: stage-engines lipos; Tauri
    consumes.) **Dual-arch sourcing fallback `[DECIDED]`:** the universal build runs on an
    **arm64** runner (§6.1.4), so the engine-asset cache MUST supply **both** the
    `aarch64-apple-darwin` AND `x86_64-apple-darwin` slices for every sidecar/lib (the cache
    is the source of the x86_64 slices). If a needed x86_64 slice is absent, the **documented
    fallback** is to build it x86_64-on-arm64 via the cross toolchain / Rosetta 2 from source
    (the hardest practical step, used only when the cache lacks a slice); `lipo` cannot merge
    a slice it does not have, so a missing slice fails the macOS leg clearly rather than
    shipping a single-arch sidecar.
  - **Engine support files** (non-executable: LibreOffice's `share/`, `program/`
    libs, fonts, pandoc data) → `bundle.resources`, resolved at runtime via the
    Tauri resource path (§3.5 owns the working-dir/env wiring; §7.2 owns startup
    presence-verification of these files).
- The whole engine set is **vendored into the build inputs** — never fetched at
  runtime (SSOT offline floor) and, per the supply-chain stance (§6.3.4),
  **pinned by version + checksum**, ideally not fetched at build time from a live
  network either (a local/cached engine artifact store). **`[DECIDED]` (adopting the
  [REC]): a pinned, checksum-verified engine-asset cache keyed by engine version**,
  **not** committed raw into Git (avoids bloating the repo) and **not** built from
  source per-release (too slow). The **size budget** this implies is owned by §3.9.
  (Build-from-source remains a documented fallback if a pinned artifact becomes
  unavailable.)
- **Cache hosting mechanism `[DECIDED]`:** the engine-asset cache is **GitHub Actions
  cache** (`actions/cache`) keyed **`<engine>-<version>-<triple>`** (e.g.
  `ffmpeg-7.1-aarch64-apple-darwin`), with a **checksum-verified pinned-upstream-URL fetch**
  as the populate path / fallback on a cache miss (download the pinned upstream release
  asset, verify its SHA-256 against the in-repo pin, then store under the same key). Each
  cache entry stores the verified per-triple binary tree; `scripts/stage-engines` reads from
  the restored cache (never the live network at package time). **macOS dual-arch key scheme:**
  the universal build needs **two slices per engine** — the key carries the triple, so a
  macOS engine has **two distinct keys** (`<engine>-<version>-aarch64-apple-darwin` AND
  `<engine>-<version>-x86_64-apple-darwin`); `scripts/stage-engines` restores both and
  `lipo -create`s them into the `<name>-universal-apple-darwin` fat binary (§6.1.4). A
  cache miss on either slice falls back to the pinned-URL fetch for that slice.
- A platform's artifact ships **only the engines available on that platform per
  §3.4**. A patent-gapped engine (e.g. an HEVC encoder absent on a platform) is
  simply not staged there; the affected target is surfaced as unavailable in the UI
  (§5.2, sourced from §3.4) — **never a silent omission** (SSOT *v1 DoD* exception 1).
- **LGPL link assertion (§3.6.1 build rule) — scoped by WHERE the lib is linked
  `[DECIDED]`:** the LGPL §6 obligation differs for the MIT core vs the separate
  image-worker, and the assertion must match (a single blanket "all LGPL must be shared,
  static = fail" rule would WRONGLY fail the statically-linked image-worker that §3.5.5/
  §3.6.1 mandate as aggregation). Three carve-outs:
  - **(i) LGPL linked into the MIT core (the Tauri app binary) → MUST be shared/relinkable.**
    Any LGPL lib linked into the MIT core binary itself must be a **bundled shared object**
    (`.so`/`.dylib`/`.dll`) — a static LGPL link into the MIT core is a **build failure**
    (it would taint the MIT core; Rust links statically by default, so this is enforced).
    The **external FFmpeg component libs** dynamically linked *beside* the FFmpeg binary
    are verified present as shared objects too. **Only `libmp3lame` is LGPL** (so the §6
    relinkability-beside-the-GPL-exe obligation applies to it); `libvorbis`/`libogg`/
    `libopus`/`libvpx` are **BSD-3-Clause** (no relink obligation — present-as-shared-object
    is for SBOM-completeness, not LGPL §6). Each has its own §3.7.2 row (release-blocking if
    absent); `libvpx` is the VP9/WEBM-target encoder and carries its `PATENTS` text.
  - **(ii) LGPL inside the separate image-worker (libvips/libheif/libde265/librsvg) →
    static LGPL is acceptable AGGREGATION, but carries the LGPL §6 relink obligation.**
    The image-worker is its **own binary** (a separate process, §3.5.5), so a static LGPL
    link inside it is **aggregation**, not a link into the MIT core — it must **NOT** fail
    the build for being static. **BUT** LGPL §6 relinkability still applies to a statically
    linked LGPL executable, so this carve-out **carries an obligation, not just an
    exemption**: the build **MUST ship the image-worker's complete corresponding source +
    the LGPL object files** (or a documented relink recipe / `Makefile` target) alongside
    the release so a user can relink the worker against a modified LGPL lib (§3.6.2
    written-offer + §3.7 SBOM record the exact pinned source). The stage step **asserts the
    relinkable-source bundle (object files / recipe) is present** for the static image-worker
    and **fails the build if it is missing** — mirroring the FFmpeg carve-out below.
    **The bundle MUST also cover x265 `[DECIDED]`:** when the x265 GPL plugin is loaded, the
    image-worker is a **GPL combined work** (§3.6.1 x265 row), so the corresponding-source
    obligation extends to **x265 itself** (its GPL §3 complete corresponding source + offer),
    not only the LGPL stack — the assertion checks the **pinned x265 source/offer is present**
    alongside the LGPL source, and fails the build if x265's source is missing.
  - **(iii) FFmpeg-internal static LGPL → aggregation, never fails the assertion.** A
    static GPL FFmpeg with `libmp3lame` (LGPL) plus the BSD `libvorbis`/`libogg`/`libopus`/
    `libvpx` baked in is GPL-clean (GPL permits static LGPL/BSD) and the whole binary is
    aggregation (§3.6.1), so it must not fail the assertion for a non-licence reason. v1's
    stated preference is dynamic-beside-the-exe (carve-out i path), so this carve-out covers
    the static-FFmpeg alternative without leaving both implied (§3.7.2). Each component still
    appears as a nested SBOM sub-component of the FFmpeg build.

  Summary: **shared-object / relinkable LGPL is required only where the lib is linked into
  the MIT core (carve-out i); the separate image-worker (ii) and FFmpeg (iii) are
  aggregation — static LGPL is permitted there, with (ii) owing the §6 relinkable-source
  bundle.**
- **libvips no-copyleft-PDF-loader assertion (§3.1/§3.6.1 build rule) `[DECIDED]`:** the
  bundled libvips MUST be configured **WITHOUT the poppler/PDF loader (GPL — it makes
  the whole libvips build effectively GPL, libvips#2222), WITHOUT the MuPDF loader
  (AGPL), and without any other GPL/AGPL loader**, so the image-worker stays LGPL-only.
  ConvertIA needs **no** libvips PDF loading (PDF→TXT is the separate poppler `pdftotext`
  sidecar, §3.5.3), so this costs nothing. The stage step runs a **positive assertion**
  that the staged libvips exposes **no poppler/mupdf loader/symbols** (e.g. no
  `pdfload`/`poppler`/`mupdf` foreign loaders registered) and **fails the build** if one
  is present (a distro/default libvips often enables poppler-glib PDF).
- **libimagequant BSD-leg assertion (§3.1 row 1e / §3.6.1) `[DECIDED]`:** the stage step
  asserts the staged `libimagequant` `COPYRIGHT` actually contains the **BSD-2-Clause**
  text (the frozen `lovell/libimagequant` v2.4.x fork), and **fails the build** if a
  GPLv3 leg (upstream 4.x) slipped in — the §6.3.3 SPDX-presence gate sees a declared id,
  not the shipped text, so this text check is the real guard. **Plus a lockfile-pin
  provenance check `[DECIDED]`:** since libimagequant is **statically vendored** inside
  libvips' `cgif` path (no runtime soname to resolve), the stage step also asserts the
  pinned `imagequant`/`libimagequant` ref in `engines.lock` (and any `Cargo.lock` entry) is
  **exactly the `lovell/libimagequant` v2.4.x-fork commit** — provenance, not an ABI/soname
  check (which would be meaningless for a vendored static lib).
- **libheif-resolves-dav1d-for-AV1-decode assertion (§3.1 row 1b / images.md) `[DECIDED]`:**
  "libaom is encode-only" is a configuration choice, not a libaom limitation — so the stage
  step asserts the staged **libheif resolves `dav1d` as its AV1 *decoder* plugin** (e.g.
  `heif-info`/`libheif_decoder` enumeration lists dav1d, not libaom, for AV1) and **fails
  the build** if libaom is wired as the decoder (or no dav1d decoder is present). Parallel
  to the libimagequant pin/provenance check (§3.1 row 1e): the shipped wiring is verified,
  not trusted. (Here the wiring IS a runtime plugin enumeration, so this one legitimately
  inspects the staged libheif's resolved decoder — distinct from libimagequant, which is
  statically vendored and so verified by its lockfile pin, not a soname.)
- **Exposed-parameter capability assertions (against the §3.8-pinned versions) `[DECIDED]`:**
  the per-format option names ConvertIA exposes must actually exist in the staged engine
  builds, so the stage step asserts (and **fails the build** on a miss): (1) the **FFmpeg
  `paletteuse` dither modes** the video→GIF path exposes — the **canonical v1-exposed set is
  exactly `bayer`, `sierra2_4a`, `floyd_steinberg`, `none`** (cross-category.md [OPEN-D]
  `[DECIDED]`; `none`/`floyd_steinberg` are valid `paletteuse` values and `floyd_steinberg`
  IS the "error-diffusion" mode — there is no separate generic value) — are **all** present
  in the staged `ffmpeg -h filter=paletteuse`. The assertion checks **this exact enumerated
  list** (the same four cross-category.md exposes in its Dither option), so the asserted set
  and the UI-exposed set cannot drift;
  (2) the **libvips `webpsave`/`heifsave` `effort` parameter** (and `Q`) exists in the staged
  libvips (images.md exposes the integer `effort` for WEBP/**AVIF** — `heifsave` has
  no `preset` string, only `effort`, §images.md) — `vips webpsave`/`heifsave` arg
  introspection. **The `heifsave effort` arg-presence check is necessary but NOT sufficient
  for HEIC exposure `[DECIDED]`:** images.md gates the HEIC `effort` *control* on a
  `[DEFER: corpus]` spike confirming `effort` measurably steers the bundled x265/HEVC path —
  if the corpus shows it is inert for HEIC, the control is **hidden for HEIC** (no dead
  control); AVIF `effort` stays exposed (libvips-documented as honoured). These prevent a
  version bump from silently dropping an exposed knob.
- **ICO multi-size/256px save spike `[DEFER: corpus/build spike]` (gates the v1 `* → ICO`
  pairs):** ImageMagick's ICO encoder has documented trouble with **256px / multi-size**
  entries and libvips' `magicksave` is not documented to support `.ico` save (§3.5.5), so the
  ICO-save capability is **unverified**. A build spike MUST confirm the bundled
  libvips+ImageMagick can **write a valid multi-size `.ico` including a 256px embedded-PNG
  entry** (`[16,32,48,256]`, the 256 stored as embedded PNG); a corpus case re-opens the
  produced `.ico` and verifies all four entries + the 256px PNG marker. **Two outcomes:**
  (a) **spike passes** → the `magicksave` ICO path is confirmed and **this assertion fails
  the build if magicksave ICO save regresses**; (b) **spike fails** → ConvertIA ships the
  **in-core Rust ICO container assembler** (§3.5.5) instead, the assertion targets that
  assembler's output, and ImageMagick is dropped from the ICO path. **Until the spike
  resolves, the `* → ICO` v1 pairs are gated on it** — the §6.4.3 ICO corpus case (multi-res
  16/32/48/256 + non-square) is the runtime proof of whichever path ships, and this build
  assertion is **not** stated as settled-and-working until the spike outcome is recorded.
- **Curated-FFmpeg decoder-coverage assertion `[DECIDED]`:** the FFmpeg build uses
  `--disable-everything --enable-…` trimmed to the `04` codec set (size lever, §3.9),
  which risks **silently dropping a decoder a 04 pair needs**. So the stage step runs
  a **build assertion** that the curated `--enable` list covers **every decoder the
  source matrices reference**. **The required-decoder set is a GENERATED manifest, not
  a hand-kept list `[DECIDED]`:** a build step parses the `04` matrices (every codec
  named in `audio.md` / `video.md` / `cross-category.md` / `images.md`, on the source
  side) into `ffmpeg-required-decoders.lock`, and the assertion runs against **that**
  generated set — so the named minimum can never drift below the real requirement. The
  generated set **must include**, among others, the load-bearing **modern** decoders
  the headline use cases need: **`hevc`, `h264`, `av1`** (iPhone HEVC `.mov`, modern
  MKV, AV1-in-MKV/WEBM — see §3.4.3's image/video decoder split: these are FFmpeg's
  *own* `hevc`/`h264`/`av1` decoders, **not** the image-worker's libde265/dav1d),
  **`mpeg4`, `msmpeg4v2`, `msmpeg4v3`** (DivX/Xvid AVI, WMV1/2), **`mjpeg`**, **`aac`**
  (every AAC track), **`vorbis`, `opus`** (WEBM/OGG audio decode), plus **`flv1`,
  `vp6a`/`vp6f`** (FLV), **`mp2`**, **`wmav1`/`wmav2`/`wmapro`/`wmalossless`**, **`vc1`**,
  **`h263`**, **`amrnb`**, **`mpeg1video`/`mpeg2video`**, **`vp8`/`vp9`**, **`dca`/`ac3`**,
  **`alac`/`flac`/`pcm`**. (This enumeration is the *expected floor* the generator must
  meet or exceed, documented here as a sanity check; the **authoritative** set is the
  generated `ffmpeg-required-decoders.lock`, regenerated from the 04 matrices, never
  hand-edited.) It runs `ffmpeg -decoders` / `-muxers` on the staged binary and **fails
  the build** if any decoder/muxer in the generated set is absent; the §6.4.3 per-pair
  integration tests are the runtime backstop that catches anything the static list missed.
- **Curated-FFmpeg ENCODER-coverage assertion `[DECIDED]` (same generated-from-04 treatment
  as decoders):** the `--disable-everything --enable-…` trim can **also** silently drop a
  needed native **encoder** (a default-on native encoder excluded by the trim), so the same
  build step parses the `04` matrices on the **TARGET** side into
  **`ffmpeg-required-encoders.lock`** and runs **`ffmpeg -encoders`** on the staged binary,
  **failing the build** if any required encoder is absent. The generated encoder set **must
  include**, among others, the load-bearing target encoders: **native `aac`** (M4A/AAC
  targets), **`alac`**, **`flac`**, **`pcm_s16le`/`pcm_s16be`/pcm_*** (WAV/AIFF/ALAC),
  **`libmp3lame`** (MP3), **`libvorbis`** (OGG), **`libopus`** (OPUS), **`libx264`** (H.264
  video), **`libvpx-vp9`** (WEBM/VP9). (As with decoders, this is the *expected floor*; the
  authoritative set is the generated `ffmpeg-required-encoders.lock`, regenerated from the 04
  target matrices, never hand-edited.) So a trimmed build can't drop an encoder a 04 target
  needs without failing CI.
- **Curated-FFmpeg network-protocol + dereferencing-demuxer absence assertion `[DECIDED]`
  (T9b SSRF **and** LFR, §0.11 / §3.5.1):** the FFmpeg build disables networking protocols
  at configure time (no `--enable-protocol=http`/`https`/`tcp`/`tls`/`rtmp`/`hls` family;
  `--disable-network` where it does not break a needed demuxer). The stage step runs
  **`ffmpeg -protocols`** on the staged binary and **fails the build** if any network
  protocol is present (the SSRF half). It **also** runs **`ffmpeg -demuxers`** and **fails
  the build** if a playlist/manifest dereferencing demuxer ConvertIA does not need is
  present (the absolute-file LFR half — local-HLS `hls`, DASH `dash`, and any external-
  reference playlist demuxer; the `concat` demuxer, if present for a legitimate §04 pair,
  is only ever invoked with the default `-safe 1`, never `-safe 0`, so it rejects absolute/
  `..` paths). These are the build-time half of the §3.5.1 control; the argv
  `-protocol_whitelist file,pipe` + `-safe 1` are the always-on runtime half, and the
  §6.4.2 adversarial-egress case (zero egress **AND** no out-of-input file read) is the
  runtime proof.
- **SVG/librsvg external-resource (LFR) corpus assertion `[DECIDED]` (T9b absolute-file
  LFR, §0.11 / §3.5.5):** a corpus case feeds the image-worker a **crafted SVG with an
  external `<image href>`** — both a relative `../`-escape (`href="../secret.txt"`) and an
  absolute `file:///etc/passwd`-style reference — pointing at a known out-of-input
  sentinel file, and **fails the build/test if the rasterised output embeds any sentinel
  bytes** (it must not — the **PRIMARY control is loading the SVG via `rsvg::Loader` with
  NO `base_file`/base URL**, so librsvg has nothing to resolve a local/relative `href`
  against and refuses all such loads by construction; **no base-URL confinement is used** —
  supplying any base URL is what re-enables the CVE-2023-38633-class surface, §3.5.5).
  **librsvg API assertion `[DECIDED]`:** the stage step **also asserts the pinned `librsvg`
  crate/version exposes the relied-upon `rsvg::Loader::read_stream`/`from_data`-without-
  `base_file` path** and **fails the build** if it is absent (so the load-bearing control is
  actually buildable against the staged crate). **librsvg version-floor assertion
  `[DECIDED]`:** the stage step **also asserts the staged librsvg is `>= 2.56.3`** (the
  CVE-2023-38633 fix floor — the CVE that bypassed base-URL directory-confinement) and
  **fails the build** if older; this is a belt-and-suspenders floor (not load-bearing for
  v1, which sets no base URL) so that if a base URL is ever required later it is not a
  known-bypassed version. This is the SVG analogue of the §6.4.2 FFmpeg adversarial-egress
  case, giving the SVG vector the same proof-parity as FFmpeg/pandoc/LibreOffice.

### 6.1.4 CI runners

| Leg | Runner | Toolchain installed | Platform-specific deps |
|-----|--------|---------------------|------------------------|
| Windows | `windows-latest` (x64) | Rust (MSVC host triple), Node + pnpm | WebView2 is preinstalled on supported Windows; **not** bundled (no-network forbids downloading it at runtime — §0.3.1 owns the floor). **CI-realism note `[DECIDED]`:** WebView2's presence is a **runner-IMAGE property** (true on the `windows-latest` image) — **not guaranteed if the image is later pinned to a specific version**, so the E2E step verifies WebView2 is present on the pinned image. The Windows runner provides a **virtual desktop**, so — unlike the Linux/Xvfb leg — the §6.4.6 E2E needs **no extra display setup**. NSIS provided by tauri-cli. |
| macOS | `macos-latest` (Apple Silicon) building `universal-apple-darwin` | Rust with **`rustup target add aarch64-apple-darwin x86_64-apple-darwin`** (both targets — prerequisite for the universal build and for `lipo`-merging each sidecar into its single `<name>-universal-apple-darwin` fat binary, §6.1.3), Node + pnpm | Xcode CLT for `lipo`/codesign-less packaging. No notarization step (out of scope). **Dual-arch engine sourcing (Lane-B operational prerequisite) `[DECIDED]`:** the universal build runs on an **arm64** runner, so the **engine-asset cache (§6.1.3) MUST supply pre-built binaries for BOTH `aarch64-apple-darwin` AND `x86_64-apple-darwin`** for every sidecar/lib — `scripts/stage-engines` `lipo -create`s them per §6.1.3, and it cannot lipo a slice it doesn't have. The cache provides the x86_64 slices (this is the hardest practical part); building x86_64 engines on an arm64 runner from source needs the cross toolchain / Rosetta 2 and is the documented fallback only. |
| Linux | **`ubuntu-22.04` (pinned, NOT `ubuntu-latest`) `[DECIDED]`** | Rust, Node + pnpm | **compile closure** `libwebkit2gtk-4.1-dev` + `libgtk-3-dev` + `libsoup-3.0-dev` + `libjavascriptcoregtk-4.1-dev` + `libdbus-1-dev` (each maps to a Tauri-v2 `-sys` build-script consumer — `libgtk-3-dev` is the gdk/pango/cairo/atk/glib/x11/wayland umbrella; the per-push gate-tooling lint job §6.7.1 installs **exactly this Linux-only subset**, since it compiles+tests but does not bundle), **plus** the release-bundle/runtime deps `librsvg2-dev` + `libayatana-appindicator3-dev` (runtime SVG / dlopen tray) + `patchelf`; **plus FUSE 2** for the AppImage. **Pin rationale + two FUSE notes `[DECIDED]`:** the runner is pinned to **`ubuntu-22.04`** (not the drifting `ubuntu-latest`) for **(i)** glibc-floor stability (older glibc = wider compatibility, §0.3.1 floor) and **(ii)** to avoid **FUSE2-vs-FUSE3 drift** — `ubuntu-latest` rolling to 24.04+ broke `libfuse2` packaging (the time_t `libfuse2t64` rename). (1) **FUSE 2 is a RUNTIME dependency, not build-time** — an AppImage *mounts* itself via FUSE 2 at launch, so the **end user's machine needs `libfuse2`**; the download page must disclose this (a bare "download, run, done" is false on a distro shipping only FUSE 3 — alternatively `tauri build` / the AppImage runs with `--appimage-extract-and-run`, which needs **no** FUSE at all and is the recommended CI invocation to sidestep the issue entirely). (2) If a newer runner is ever used, the install step must handle both package names (`libfuse2 \|\| libfuse2t64`) or use `--appimage-extract-and-run`. |

**macOS/Windows runner-pin asymmetry — explained + drift-guarded `[DECIDED]`.** The Linux
leg is pinned (`ubuntu-22.04`) for the glibc/FUSE reasons above; macOS/Windows are left at
**`macos-latest`/`windows-latest`** *deliberately*, with a guard rather than a hard pin:
- **Why not hard-pin them:** the §0.3.1 floor for Win/macOS is "rely on the OS WebView2 /
  WKWebView present on the supported OS" — building on the *current* image is the realistic
  end-user baseline, and unlike Linux there is no glibc-floor / FUSE-packaging hazard that a
  newer image reopens. `macos-latest` rolling (Sonoma → Sequoia) *can* change Xcode CLT /
  the default deployment target, which affects the universal build + the sidecar `lipo`.
- **Drift guard (the price of `latest`) `[DECIDED]`:** each macOS/Windows leg **records the
  resolved image label + Xcode/CLT (macOS) / WebView2 (Windows) version as a release-asset
  line**, and the build **fails if the macOS deployment target drifts below the §0.3.1
  floor** (`MACOSX_DEPLOYMENT_TARGET` assertion = `11.0`) or WebView2 is absent on the
  image — so a `latest` roll surfaces loudly, not silently. If a future roll breaks the
  build, the fallback is to **pin to the last-known-good label** (e.g. `macos-15` /
  `windows-2025`) — recorded as the remedy, not pre-applied.

The platform CI standard (`reference_self_hosted_ci_runner.md`) runs a **self-hosted
VPS runner** for the Ne-IA org's existing four projects. ConvertIA's build matrix
**cannot** reuse a single Linux VPS runner for all three legs (no native macOS/Windows
there). **`[DECIDED]` (adopting the [REC]): GitHub-hosted runners for the
macOS/Windows legs; the self-hosted Linux runner for the Linux leg + the Lane-A
lint/test gate.** Rationale: matches upstream Tauri guidance, and release builds are
**infrequent** (one-large-all-or-nothing v1, SSOT) so Actions-minute spend is
bounded. **Budget note (kept visible):** GitHub **macOS**-hosted minutes bill ~10×
Linux/min — relevant to the hobby/no-paid-upgrades budget
(`user_hobby_budget_no_paid_upgrades.md`); the infrequent-release cadence keeps it
within free-tier/affordable bounds, and the Linux leg (the frequent Lane-A path)
stays on the free self-hosted runner. Revisit only if release cadence rises.
**Runner-host integrity carve-out `[DECIDED — P0 review r2]`:** the self-hosted VPS
runs the Lane-B Linux **corpus** leg (untrusted `corpus-large` + fuzz/adversarial
inputs), so the **secret-bearing signing step (§6.7.2 stage 6) must NOT run on it** —
it runs on an ephemeral GitHub-hosted runner, host-isolated from the corpus/fuzz jobs,
enforced by build-gates **G56** (a secret-using job bound to a self-hosted label is a
hard fail). A persistent multi-tenant runner that handles untrusted input is the
standard host-compromise vector for the one key the whole trust substitute depends on.

**`ubuntu-22.04` pin vs the self-hosted Lane-A runner's actual OS — reconciled per lane
`[DECIDED]`.** The `ubuntu-22.04` pin above is a **GitHub-hosted-image** label; the
**self-hosted IONOS VPS runner** (used for **Lane-A** Linux and the **Lane-B** Linux corpus
leg) has a **fixed host OS that may not be ubuntu-22.04**. So the pin is honoured **per
lane**, not by assuming the VPS image:
- **Record the VPS distro AND kernel as concrete facts** in
  `reference_self_hosted_ci_runner.md` / §6.1.4 (the runner's actual `lsb_release` **and
  `uname -r`**), so the gap is known, not guessed. **Recorded fact (placeholder, confirm
  at setup) `[DECIDED]`:** the Ne-IA self-hosted runner is **Ubuntu-class** (the
  org-standard IONOS VPS per `reference_self_hosted_ci_runner.md`), whose stock kernel is
  **≥ 5.15 (Jammy-class)** — i.e. **above the Landlock ≥ 5.13 floor** — so the **expected
  §6.4.2 fs-audit enforcement path on the self-hosted Linux leg is Landlock** (with
  `SYS_PTRACE` as the alternative inside `--cap-add SYS_PTRACE` Docker, §6.4.2). The
  **exact `uname -r` is recorded at runner provisioning** (`[DEFER: record at setup]`) and
  the build asserts Landlock availability before relying on it (§6.4.2); if the recorded
  kernel turns out < 5.13, the leg must run under `--cap-add SYS_PTRACE` or the
  GitHub-hosted fallback (so the fs-audit half never silently no-enforces).
- **If the VPS OS matches the `ubuntu-22.04` glibc/FUSE floor**, the pin is satisfied
  natively and Lane-A runs directly on the host.
- **If it does NOT match**, the **Lane-A compile-sanity + the Lane-B Linux corpus build run
  inside a `ubuntu:22.04` Docker container** on the VPS runner (so the glibc floor / FUSE
  packaging match the pinned baseline regardless of the host OS), **or** the Lane-B Linux
  leg uses **GitHub-hosted `ubuntu-22.04` as the documented fallback** (already the standing
  fallback for VPS contention, above). Either way the *artifact-relevant* build always meets
  the `ubuntu-22.04` floor; only the *runner host* is allowed to differ.

---

## 6.2 Reproducibility & integrity `[DECIDED — published hashes; reproducibility = best-effort intent]`

### 6.2.1 Why this matters (SSOT)

Signing/notarization is **deliberately out of scope** (SSOT *Out of Scope*), so the
**stated trust substitute** is *published integrity hashes from one canonical
location*. This is the only protection a user has that a download is authentic, so
it is treated as a **release gate**, not an afterthought.

### 6.2.2 Canonical location `[DECIDED]`

Releases are published **only** from the **Ne-IA org's GitHub Releases**
(`github.com/Ne-IA/convertia/releases`) — the single source of authentic builds
(SSOT *Distribution & download trust*). No mirror, no third-party host is endorsed.
The README/download page states this explicitly.

### 6.2.3 Hashes & manifest `[DECIDED]`

For **every** published artifact:
- A **SHA-256** is computed in CI immediately after the artifact is built (before
  upload) and published as:
  - a per-file `<artifact>.sha256` sidecar, **and**
  - a single `SHA256SUMS` manifest covering **every release asset** (the familiar
    `sha256sum -c SHA256SUMS` workflow). "Every asset" means the platform binaries
    **and** the SBOM (CycloneDX/SPDX), `NOTICE`/`THIRD-PARTY-LICENSES.txt`, and
    `reliability-report.json` — so a user can verify the attribution and
    reliability artifacts too, not just the executable. (`SHA256SUMS` itself is the
    only asset it cannot list; the §6.2.3 minisign signature covers it.)
- **`[DECIDED]` publish a project minisign detached signature over `SHA256SUMS`.**
  This is *not* code-signing the binary (out of scope) but signing the *checksum
  manifest* — it closes the "attacker replaces both the artifact **and** its hash"
  gap that bare checksums leave open, at near-zero cost and entirely within
  "no store/cert" scope. **Key handling `[DECIDED]`:** the minisign **public key is
  committed at `docs/minisign.pub` in the repo** (and restated on the download page),
  so anyone can verify; the **private key is a CI secret named `MINISIGN_SECRET_KEY`**
  (with its passphrase in `MINISIGN_PASSWORD`), injected only into the Lane-B release
  job that signs `SHA256SUMS` — never committed, never in the bundle. The verify
  recipe (§6.2.4) includes `minisign -Vm SHA256SUMS -p docs/minisign.pub` (lowercase
  `-p` = a public-key **file path**; uppercase `-P` would expect the key as an inline
  base64 **string**, so `-P docs/minisign.pub` would fail — the recipe is standardised on
  `-p` across this doc, §6.2.4, and the README, and a release-tier gate **runs the literal
  recipe**, build-gates G39/G44, so "recipe present" becomes "recipe correct and working").
  (Adopting the [REC] — a clear strengthening of the SSOT trust substitute with no
  downside.)
  - **Key-rotation policy `[DECIDED]`:** a re-key MUST be **distinguishable from a
    supply-chain compromise**, so rotation is a **deliberate, announced, signed** event,
    never a silent swap: (1) commit the **new** public key to `docs/minisign.pub` in a
    **dedicated commit whose message announces the rotation** (and, where possible, GPG/SSH-
    signed or made via a protected-branch PR), (2) **retain the old key** as
    `docs/minisign-retired.pub` (so old releases stay verifiable and the change is auditable),
    (3) **note the rotation in the release notes** of the first release signed with the new
    key, and (4) rotate the CI secrets (`MINISIGN_SECRET_KEY`/`MINISIGN_PASSWORD`). A user
    who sees `docs/minisign.pub` change without the announced retired-key + release-note
    trail should treat it as suspicious — which is exactly the property a silent swap would
    destroy.

### 6.2.4 How a user verifies (must be surfaced) `[DECIDED]`

The download page and README give a copy-paste verification recipe **at the
highest-risk moment** (SSOT):
- Windows (PowerShell): `Get-FileHash .\ConvertIA-<version>-x64.zip -Algorithm SHA256`
  → compare to the published value. (The Windows download is the **portable `.zip`**,
  §6.1.2 — hash the downloaded archive, not a loose `.exe`.)
- macOS/Linux: `shasum -a 256 ConvertIA.dmg` / `sha256sum ConvertIA.AppImage`, or
  `sha256sum -c SHA256SUMS`.
- Verify the minisign signature: `minisign -Vm SHA256SUMS -p docs/minisign.pub`
  (lowercase `-p` = public-key **file path**; uppercase `-P` expects an inline base64
  key string and would fail on a file path).
The page also restates the **as-is / no-warranty / best-effort-security** posture
(SSOT *License & Openness*), and the unsigned-build first-launch friction per OS so a
normal user isn't surprised:

- **macOS Sequoia (15.x) `[DECIDED]` — step-by-step (the bypass changed):** because
  the build is unsigned/unnotarized, the first launch is **blocked** by Gatekeeper, and
  on Sequoia the old Control-click "Open" shortcut **no longer works**. The page must
  spell out: (1) double-click → "ConvertIA can't be opened" → **Open System Settings →
  Privacy & Security**, scroll to the blocked-app notice, click **"Open Anyway"**, then
  **on the final confirmation dialog macOS shows, click "Open" to confirm** (the Sequoia flow
  adds this extra confirm step after "Open Anyway"), then re-launch and confirm; (2) **each bundled tool (FFmpeg, LibreOffice, pandoc, etc.) is
  independently quarantined** — so the **first conversion** may also be blocked and the
  same Privacy & Security → "Open Anyway" step may be needed **per sidecar** the first
  time it runs. ConvertIA surfaces this in-app as the `QuarantinedByOs` message (§2.8 /
  §7.2.4) rather than failing silently. This is a **material non-technical-user
  blocker**, so the §6.6 macOS walkthrough must test+pass it on Sequoia (the unsigned
  posture is re-affirmed acceptable only because the §6.6 floor verifies the guided
  recovery actually works).
- **Windows:** SmartScreen "Windows protected your PC" → **More info → Run anyway**
  (the analogous unsigned-build friction; surfaced on the download page).
- **Windows WebView2 prerequisite `[DECIDED]`:** because the **portable `.zip`** cannot
  show an in-app fault when WebView2 is **absent** (the loader fails before the core
  runs — §0.3.1), the download page **must** carry a prerequisite note: *"ConvertIA needs
  Microsoft Edge WebView2 (built into Windows 11 and current Windows 10; if a window
  flashes and closes, install the WebView2 Runtime or update Windows/Edge)."* — this is
  the "fail clearly" substitute for the portable path (not a runtime dialog). **Since
  the portable `.zip` is the only v1 Windows artifact (NSIS NOT shipped v1, §6.1.2
  `[DECIDED-6.1a]`), this WebView2 prerequisite note is the sole Windows floor mechanism
  in v1** — there is no NSIS bootstrapper enforcing it. (A future post-v1 NSIS variant
  would enforce the floor via its bootstrapper; not applicable to v1.)
- **Linux AppImage FUSE 2 prerequisite `[DECIDED]`:** an AppImage *mounts* itself via
  **FUSE 2 at launch** (a runtime dependency — §6.1.4), so the page must note:
  *"Linux: the AppImage needs `libfuse2` (Ubuntu: `sudo apt install libfuse2`, or
  `libfuse2t64` on 24.04+); alternatively run with `--appimage-extract-and-run`."* —
  satisfying the §6.1.4 disclosure requirement with an enforcement home (and a §6.8
  README content item, below).

### 6.2.5 Reproducible-build intent `[DECIDED — best-effort, not a gate]`

Full bit-for-bit reproducibility across the Rust+WebView+vendored-engine artifact
is **hard** (timestamps, build-paths, per-runner toolchain drift, the prebuilt
engine binaries we don't compile ourselves). v1 stance: **reproducibility is a
best-effort intent, explicitly NOT a release gate** (mirrors the SSOT
engine-currency "best-effort, not a gate" posture). Cheap measures we *do* take:
pinned toolchains (§0.8), pinned engine versions+checksums (§3.8/§6.1.3),
`SOURCE_DATE_EPOCH` where the toolchain honours it, and recording the exact
toolchain/engine versions in the SBOM so a build is at least **auditable** even if
not bit-reproducible. **`[DECIDED-6.2b]`** how far to pursue determinism — **best-effort,
NOT a release gate** (the cheap measures above ship; deeper bit-reproducibility is
`[DEFER: post-v1]`, not an owner-level design call).

---

## 6.3 SBOM & licence artifacts `[DECIDED — attribution is a release gate]`

> §3.7 **owns the generation** of the NOTICE/third-party-licenses **data** and the
> SBOM source. §5.9 **displays** the NOTICE in-app. **This file owns** the CI
> assembly step and the **completeness gate**: a missing or incorrect attribution
> is **release-blocking — same status as the no-harm guarantee** (SSOT *v1 DoD*).

### 6.3.1 What "the SBOM" actually covers (two layers)

ConvertIA's bill of materials is **not** just its Rust crate graph — the
load-bearing licence risk is the **bundled engine binaries** (**FFmpeg GPL-2.0+** —
it enables x264, §3.6.1; LibreOffice MPL; poppler/pandoc GPL; libvips LGPL; the x265
libheif plugin GPL; the **required** ImageMagick permissive; …). (Ghostscript is
**dropped v1**, §3.1 — no AGPL row.) So the SBOM is assembled in **two layers**:

| Layer | Contents | Tool |
|-------|----------|------|
| **App dependency graph** | Rust crates (`Cargo.lock`) + JS deps (`pnpm-lock.yaml`) that compose ConvertIA's own MIT code | **`cargo cyclonedx`** for Rust; **`@cyclonedx/cdxgen`** for the frontend (native `pnpm-lock.yaml` support — **NOT `@cyclonedx/cyclonedx-npm`**, which is npm-only and would SBOM an npm-resolved tree diverging from the frozen pnpm graph, `[DECIDED — P0 review r2]`); merged into one CycloneDX document. |
| **Bundled engines (the important layer)** | Every separately-invoked engine binary + its support libs/fonts, each as an SBOM component with **name, version, licence (SPDX id), source URL, and the per-platform availability** | A **manually-maintained `engines.lock` manifest** (owned/sourced by §3.1/§3.8) is the authoritative input; CI converts it into CycloneDX components and merges with the dependency-graph layer. Optionally **Syft** scans the staged bundle to *cross-check* that nothing in the shipped tree is missing from the manifest (drift detection). |

**The merge step `[DECIDED]`:** the two layers are merged by **§3.7.2's `cargo xtask
sbom`** build step (the single named tool — it reads `engines.lock` + the
`cargo cyclonedx`/pnpm outputs and emits one document); CI does not invent a second
merger. **Pin the CycloneDX schema version `specVersion 1.5` for ALL inputs `[DECIDED]`:**
different generators (and different `cargo-cyclonedx` versions) **default to different
specVersions**, so merging mixed-version CycloneDX docs can **fail the schema gate** — we
therefore pin **1.5 explicitly on every input** rather than relying on any tool default.
**Verified `[DECIDED]`:** `cargo-cyclonedx` **does expose `--spec-version` (values incl.
`1.3 | 1.4 | 1.5`)** (1.5 supported since the CycloneDX-1.5 release; the §3.8-pinned
`cargo-cyclonedx` version MUST be one that exposes it). `[DEFER: verify]` the **exact
default specVersion of the §3.8-pinned `cargo-cyclonedx`** at pin time and record it
factually here — we do not rely on it (we pass `--spec-version 1.5` regardless), but the
note should state the pinned tool's real default rather than a speculative "now defaults to
1.6". **Invocation `[DECIDED]`:** `cargo xtask sbom` invokes **`cargo cyclonedx` as a
subprocess** (the CLI, not the library API) — it shells out to the pinned `cargo-cyclonedx`
binary and to the pnpm/npm CycloneDX generator, then merges their JSON. It therefore
MUST pass **`--spec-version 1.5`** on the `cargo cyclonedx` **command line** **and** to the
pnpm/npm CycloneDX generator (so every input is 1.5 before merge), and **abort the merge on
a schema-version mismatch** rather than emit an invalid mixed-version document. If a future pinned version
ever drops the flag, the fallback is to emit the tool default and **post-process +
re-validate** the JSON to 1.5. This keeps the gate and the version-to-version diff stable
across tooling bumps.

Output format: **CycloneDX JSON** as the canonical SBOM (developer-friendly,
good licence+component fidelity); a **CycloneDX→SPDX** export is generated too if a
consumer needs the ISO-standard form. Both are release assets. **Conversion tool
`[DECIDED]`:** the CycloneDX→SPDX export is produced by the **CycloneDX CLI's `convert`
command** (`cyclonedx convert --input-format json --output-format spdxjson` — the official
`@cyclonedx/cyclonedx-cli`/`cyclonedx-cli` tool, which supports SPDX-JSON output), pinned
in §3.8 alongside the other SBOM tools. (If a future pin drops SPDX-JSON support, the
fallback is `syft convert` from the already-present Syft, which also emits `spdx-json`.) The
SPDX export is a convenience artifact, not the gate input — the §6.3.3 completeness gate
reads the canonical CycloneDX JSON.

### 6.3.2 NOTICE / third-party-licenses assembly

- The repo `NOTICE` (and a longer `THIRD-PARTY-LICENSES.txt`) is **generated from
  the same `engines.lock` + dependency SBOM** so it can never silently drift from
  what actually ships. It contains, per bundled engine: the engine name+version,
  its full licence text, and — for **GPL/LGPL/AGPL** engines — the required
  **written offer of source** (where the corresponding source can be obtained:
  the pinned upstream tag + our build recipe), honouring §3.6's copyleft
  obligations.
- The in-app About screen (§5.9) presents this listing; the file in the repo and
  the in-bundle copy are the **same generated artifact** (one source of truth).

### 6.3.3 The completeness gate (release-blocking) `[DECIDED]`

CI runs an **attribution-completeness check** and **fails the release** if it does
not pass. Concretely:
1. **Every** binary/resource staged into the bundle (§6.1.3) maps to a component in
   `engines.lock` (Syft cross-check from 6.3.1 — *no shipped file without an SBOM
   entry*).
2. **Every** SBOM component with a copyleft licence (GPL/LGPL/MPL/AGPL) has its
   licence text present in `THIRD-PARTY-LICENSES.txt` **and** (for GPL-family) a
   written-offer-of-source entry.
3. **No** component is missing a resolved SPDX licence id (an `UNKNOWN`/`NOASSERTION`
   licence is a hard fail — forces a human to classify it before ship).
   **`LicenseRef` carve-out `[DECIDED]`:** a CycloneDX/SPDX **`LicenseRef-…`**
   custom-licence entry (for a legitimately-attributed licence that has **no registered
   SPDX short id yet** — e.g. **`LicenseRef-AOMPL-1.0`** for libaom's AOM Patent
   License, §3.1 row 1b) **satisfies** this "resolved id" gate **iff** its full licence
   text is present in `THIRD-PARTY-LICENSES.txt`; it is **not** treated as
   `UNKNOWN`/`NOASSERTION`. Without this carve-out a not-yet-registered but correctly
   attributed licence would be an unfixable hard fail. (`UNKNOWN`/`NOASSERTION` remain
   hard fails — the carve-out covers only a named `LicenseRef` *with* its text.)
4. **No** engine whose licence is *incompatible with inbound-MIT-clean distribution
   as a separate binary* slipped in (policy: copyleft is fine **as an aggregated
   separate binary**; anything that would taint the MIT core via linking is rejected
   — this is a guardrail on §3.6, surfaced as a CI assertion).
This check is part of the **release pipeline** (§6.7), not the per-PR fast lane, and
its failure blocks artifact publication exactly like a failed no-harm property test.

### 6.3.4 Supply-chain hygiene (the bundled-binary surface)

Per §0.11's threat map (*bundled-binary supply chain → §3.8/§6.3*): the pinned
engine versions+checksums (§6.1.3) are verified at stage time **against the
change-reviewed in-repo `engines.lock`** before staging AND **re-verified on
cache-restore** (an Actions cache is not integrity-protected — on mismatch, delete +
refetch from the pinned upstream URL); `cargo audit` / `cargo deny` run in CI over
the Rust graph (advisory + licence-policy enforcement, non-release-blocking
advisory-wise but licence-policy-blocking). Engine **currency** (keeping decoders
patched) is a **best-effort posture, not a gate** (SSOT) — owned by §3.8; this file
only ensures a bumped engine is re-validated against the corpus (§6.4/§6.5) before it
can ship.

**Additional supply-chain & CI-hardening controls `[DECIDED — added P0 review r1]`.**
These are owned by the build-gate catalogue (`docs/security/build-gates.md`) and are
recorded here per the SSOT > spec > security/docs conflict order (a living
security/process doc may not add a gate the spec does not mention without the spec
being updated in the same change):

- **WebView CSP + capability structural lint** (build-gates **G47**): a per-push gate
  parses `tauri.conf.json`'s `app.security.csp` + `src-tauri/capabilities/*.json`
  with `jq`/`serde_json` (not regex) and fails on any §0.10 violation — any remote
  scheme in any CSP directive (the only non-`'self'` `connect-src` tokens permitted
  are the Tauri IPC `ipc:` + `http://ipc.localhost`), `object-src ≠ 'none'`,
  `form-action ≠ 'self'`, any `fs:`/`http:`/`shell:allow-execute`/`opener:*`/`dialog:`
  grant, or the presence of `tauri-plugin-updater` / any HTTP-client plugin / an
  `updater` bundle block / an updater pubkey. This is the **per-push** verifying gate
  for §0.11 T2/T2a/T2c/T9a and the structural assertion of the §7.6.1 updater-absence
  claim (the §6.7.1 Lane-A blind-spot note's "verified by the type/config checks" is
  this gate). Paired with a `cargo-deny [bans]` deny-list for `tauri-plugin-updater`
  and the common HTTP-client crates (`reqwest`/`ureq`/`hyper`/`isahc`/`curl`).
- **Static-security / unsafe policy** (build-gates **G29**): `#![deny(unsafe_code)]`
  at the crate root of **every** first-party Rust crate (the core AND
  `convertia-imgworker`), with a **single narrowly allow-listed FFI module** carrying
  `#[allow(unsafe_code)]` (the core's §2.1/§2.3 OS primitives + the §0.9 Job-Object kill;
  the imgworker's libvips/libheif/libde265/librsvg/libimagequant FFI). The gate is "no new
  `unsafe` outside the allow-listed FFI module", enforced by deny-at-root **plus** a check
  that `#[allow(unsafe_code)]` appears on exactly the one allow-listed module path.
  **`#![forbid(unsafe_code)]` is NOT usable on an FFI-bearing crate** — `forbid` is
  deliberately un-overridable (a module cannot re-permit `unsafe` via
  `#[allow(unsafe_code)]` under `forbid`), so an FFI crate under `forbid` would not
  compile; `forbid` is reserved only for a pure-logic sub-crate with **zero** unsafe.
  A **Semgrep** layer (`p/rust` + `p/typescript` + the committed project-local rules,
  pinned/vendored for offline use; the managed `p/security-audit` pack is fetched live and
  breaks offline) augments it; `cargo-geiger` is **informational only** (a census, not an
  enforcer).
- **CI workflow hardening** (build-gates **G49/G50/G18a**): every workflow declares
  least-privilege `permissions` (the secret-bearing release job gets `contents: write`
  ONLY and never runs on a fork PR); every third-party action is pinned by full
  commit SHA (kept current via `dependabot.yml`); `actionlint` (per-push) + `zizmor`
  (CI) lint the workflows; the build resolves only the committed lockfiles (`--locked`
  / `pnpm install --frozen-lockfile`, with `git diff --exit-code` on the lockfiles);
  per-PUSH-workflow `concurrency` + `cancel-in-progress` + explicit `timeout-minutes`.
- **CI runner-host integrity** (build-gates **G56**, *added P0 review r2*): the
  secret-bearing signing step (§6.7.2 stage 6) runs on an ephemeral GitHub-hosted
  runner, host-isolated from the self-hosted-VPS Lane-B corpus/fuzz jobs (§6.1.4/§6.7.2);
  a workflow lint fails any secret-using job bound to a self-hosted label, and the
  **GitHub-hosted signing job** uses `step-security/harden-runner` (its free/Community
  tier works only on GitHub-hosted runners; self-hosted requires a StepSecurity
  Enterprise license, so on the self-hosted VPS the egress enforcement is the §6.4.2
  ptrace/Landlock fs-audit + the §6.7.3 nftables/strace monitor + the VPS egress
  allowlist + an ephemeral/JIT low-priv runner, not harden-runner). The single most
  damaging secret never shares a host with untrusted corpus input.
- **JS/WebView supply-chain parity** (build-gates **G17/G18c/G18d/G36b**, *added P0
  review r2*): a committed `.npmrc` registry pin + a resolution-URL guard over
  `pnpm-lock.yaml` (dependency-confusion defence); a committed minimal
  `onlyBuiltDependencies` allowlist + a growth lint (install-lifecycle-script lockdown);
  a frontend GPL/AGPL license hard-fail over the pnpm graph (`cargo-deny [licenses]` is
  Rust-only). The frontend SBOM is generated by **`@cyclonedx/cdxgen`** from
  `pnpm-lock.yaml` (NOT the npm-only `@cyclonedx/cyclonedx-npm`).
- **Bundled-engine CVE awareness** (build-gates **G17b**, *informational*): an
  `osv-scanner` (or `grype`-over-SBOM) scan of the **PURL-keyed** `engines.lock`
  components (a bare `(name, version)` matches nothing — a planted-positive self-test
  guards the empty-report failure) emits a dated open-CVE report as an owner-signed-off
  release asset — **non-blocking per-push**, with a **CVSS ≥ 7 on an actively-exercised
  path → release-blocking escalation** (vuln-response), honouring the §3.8 "engine
  currency is best-effort, not a gate" posture while turning it into a real detector.

---

## 6.4 Test strategy `[DECIDED]`

The DoD bar — *"works reliably"* = passes **fail-clearly (§2.8)** and **no-harm
(§2.5)** on a **representative real-world corpus** — is operationalised as a layered
test suite. Layers, from cheapest/most-frequent to most-expensive:

### 6.4.1 Unit tests — Rust core (the guarantees layer)

Pure-logic tests on the §0.7 modules, **no engines, no real FS where avoidable**:
- **Output naming contract (§2.2):** base-name-kept + target-extension; `(1)`/`(2)`
  numbering before the extension; never hashed / `_converted`; path-limit →
  fail-clearly (no truncation). Property-tested with adversarial names (Unicode,
  emoji, RTL, spaces, dots, max-length).
- **No-clobber & resolved identity (§2.1/§2.3):** exclusive-create semantics;
  symlink/junction/hardlink resolution; de-dup of the frozen set by resolved
  identity; refusal to write through a link onto a frozen source.
- **Frozen source set (§2.4):** files appearing after the freeze are never ingested;
  outputs in a source folder don't expand the batch.
- **Re-run/equivalence (§2.5):** equality on (resolved source + target + effective
  settings); safe fallback to silent numbering when undeterminable.
- **Detection (§1.2):** magic-byte classification table — every signature in §04
  (JPEG SOI, PNG, RIFF/WEBP, EBML DocType matroska-vs-webm, ISO-BMFF `ftyp`
  brand, OLE2-stream disambiguation DOC/XLS/PPT, ZIP-OPC content-type
  DOCX/XLSX/PPTX/ODF-mimetype, ADTS-vs-MP3, Ogg codec-id Vorbis-vs-Opus, ASF GUID
  WMA-vs-WMV, text/CSV-TSV delimiter sniff) gets a fixture asserting the correct
  user-facing type, **including misnamed-extension fixtures** (`.jpg` that is PNG;
  `.m4a` that is ALAC) and the "detected-but-unsupported" / "uncertain" outcomes.
- **Batch grouping (§1.3):** one-source-format-per-batch; mixed-drop pre-flight
  refusal lists the found formats; cross-category targets attach to the *source*.
- **Target resolution (§1.5) + defaults registry (§1.6):** for **every** source
  format in §04, exactly **one** pre-highlighted default; the "no required choices"
  invariant (drop → default → convert needs zero clicks) is asserted against the
  consolidated defaults registry **owned by §1.6** (`[DECIDED]`, CI-generated; the
  §6.7.1 Lane-A guard below fails the build if any §04 pair lacks a default).
- **Error taxonomy mapping (§2.8/§2.13):** each failure kind maps to its catalog
  string; worker-thread panic boundary (`catch_unwind`) surfaces a clean per-item
  failure, not a poisoned pool.

### 6.4.2 Property / fault-injection tests (no-harm + fail-clearly, hardened)

These directly defend the SSOT hard promises and run with a real (temp) filesystem
and stub/real engines:
- **Atomicity under interruption (§2.1):** a conversion killed mid-write (simulated
  crash/force-quit/cancel) **never** leaves a truncated visible file — only a
  discardable temp artifact, cleaned on next run (§2.6). Cross-volume path
  (source on USB → output in Downloads, §2.14) exercised: copy→fsync→atomic-rename
  *within* the destination volume. **macOS staged-source-copy case (added) `[DECIDED]`:**
  on macOS, **kill the app between the §3.5.0 staged source copy and the engine spawn** and
  assert on next launch that the staged copy **and** its `run-<RunId>/` dir are reclaimed by
  the §2.6.3 startup sweep (the staged source copy is created *after* the run-lock, so
  absent-lock⇒dead⇒reclaimable covers it, §2.14.2) — so the staged SOURCE copy is a tested
  residue path, not only the kind-1 `.part`.
- **No-harm fuzz:** randomized batches over the corpus assert **source bytes are
  byte-identical before/after** every run (originals never touched), including the
  same-source==same-target re-encode case (§2.1) and the divert/fallback path
  (§2.7) (guarantees hold identically there).
- **Divert / fallback scenarios (the §2.7 per-location divert, named) `[DECIDED]`:** the
  property/fault-injection harness exercises each concrete unwritable/ephemeral case
  ConvertIA must divert from — **(a) a read-only mount** (`chmod`-stripped dir / read-only
  loopback or USB), **(b) a network share** (writable and read-only — flips mid-run to
  exercise the §2.7.2 late-divert), **(c) an OS-ephemeral temp dir** (`/tmp`, `%TEMP%`,
  `$TMPDIR`/`/var/folders` — output must divert, not silently land in a purgeable place),
  and **(d) a cross-volume destination** (source on USB → output diverted to Downloads on
  the system disk, exercising the §2.14.3 copy→fsync→atomic-rename-within-destination
  fallback). Each asserts the output lands at the §2.7.3 divert target (or fails clearly
  per §2.7.3 when the divert target is itself ephemeral/unwritable), the original is
  untouched, and the **late-divert re-checks** (§2.7.2: §2.3.3 link-safety + §2.2.3
  path-limit + §2.14.4 per-volume free-space on the divert volume) all run.
- **Out-of-disk / too-big (§1.10/§2.8):** a constrained-FS harness proves the item
  fails fast+clearly, the batch continues, and free space returns to ~baseline
  (§2.6); a cleanup that itself fails is **never** reported as a clean success.
- **Malformed/adversarial inputs (§2.12/§2.13):** truncated, 0-byte, fuzzed-header,
  encrypted/DRM (password PDF/XLSX/PPTX, FairPlay M4V, PlaysForSure WMV), and
  decompression-bomb-shaped inputs each produce **one plain message**, no crash, no
  app wedge, batch continues. The decoder runs inside the §2.12 isolation boundary;
  these tests verify a hanging/crashing engine fails **one** item. **Backed by explicit
  decompression-bomb FIXTURES `[DECIDED]`** in the §6.4.5 corpus (an svgz bomb, a
  ZIP-bomb-in-OPC DOCX, a deeply-nested PDF flate stream) so the bomb case is files,
  not only a property concept.
  - **In-core detector fuzz harness `[DECIDED]` (the one untrusted-byte path OUTSIDE
    the §2.12 isolation boundary):** the §1.2 detection layer (the pure-Rust bounded
    gzip/svgz inflate, the Rust ZIP central-directory peek, the OLE2/CFB directory
    read, the bounded `xl/workbook.xml`/ODS `content.xml` structural peeks) runs in the
    trust kernel, so a panic/OOM/UB there is in the core, not a contained subprocess.
    It carries a **coverage-guided `cargo-fuzz` (libFuzzer)** target over
    `crate::detection`/sniff on a hostile ZIP/OLE2/gzip/svgz/XML corpus, asserting: **no
    panic/abort**, the decompression-ratio cap (≤100×) and the `MAX_SVGZ_SNIFF`
    (≤64 KiB) bound **actually fire**, and the XML reader has **DTD/external-entity
    resolution disabled by construction** (a `quick-xml`/`roxmltree` reader with entity
    resolution off — defeats XXE / billion-laughs in the workbook/content peek). The
    coverage-guided run is constrained to where libFuzzer is reliable (**Linux + macOS,
    nightly toolchain**); the per-push/pre-push leg is a fast deterministic `proptest`
    smoke / saved-crash-corpus replay, **not** an instrumented Windows build. This is
    distinct from the engine-side T1 control above (the corpus fault-injection THROUGH
    the §2.12 boundary): `cargo-fuzz`/libFuzzer is in-process Rust and cannot reach the
    isolated C/C++ engines, so the two T1 surfaces have two distinct gates.
- **Adversarial-egress / network-trigger inputs (§0.11 T9b, §3.5.1/§3.5.4/§3.5.2)
  `[DECIDED]`:** a small **adversarial-network corpus** — an HLS `.m3u8` / DASH `.mpd` /
  `-f concat` script / external-reference-box MP4 (FFmpeg), a remote-`<img>`/RST-include
  document (pandoc), a remote/OLE-link office file **AND a `WEBSERVICE()`/external-data-range
  `.xlsx`** (LibreOffice Calc, §3.5.2), a remote-`href` **and an external local-`<image
  href>` `../`-escape** SVG (librsvg, §3.5.5) — is converted **inside the §6.7.3
  packet-monitor / egress-deny window** and
  must produce **(a) zero outbound packets AND (b) no out-of-input file read** (the
  out-of-input-read half is asserted by a known out-of-input sentinel file the engine must
  NOT read/embed).
  - **fs-audit-half enforcement dependency `[DECIDED]`:** the "no out-of-input file read"
    half typically uses **`ptrace`** (strace / an `ptrace`-based fs-audit), which is
    **commonly blocked inside CI containers** (no `SYS_PTRACE` capability) → the check
    would silently not-enforce. So: run this leg with **`docker --cap-add SYS_PTRACE`**
    (or outside Docker on the §6.1.4 self-hosted VPS runner); **if `ptrace` is unavailable,
    the fallback is the §2.12.3 Linux Landlock tier** — restrict the decoder to `{input
    ro, scratch rw}` and treat **the grant itself as the enforcement** (an out-of-input
    open is denied by the kernel, observable as the engine's `EACCES`), so the property
    holds without `ptrace`. **Landlock availability MUST be asserted before relying on it
    `[DECIDED]`:** Landlock is a best-effort *silent-degrade* tier in production (§2.12.3),
    so the fs-audit MUST first **probe that the kernel actually has Landlock** (ABI ≥ 1,
    kernel ≥ 5.13) and that the ruleset applied — it must NOT assume the grant took. **Fail
    CLOSED if NEITHER `ptrace` NOR Landlock is available `[DECIDED]`:** when the runner has
    no `SYS_PTRACE` **and** no working Landlock (e.g. kernel < 5.13), the fs-audit half has
    **no enforcement mechanism** — it MUST **FAIL the CI gate** (a hard fail, the runner is
    misconfigured for this mandatory check), **never silently pass**. **The fail-closed MUST
    be diagnosable `[DECIDED]`:** before the non-zero exit, the step **emits a GitHub Actions
    `::error::` annotation** (e.g. `::error::fs-audit cannot enforce: neither ptrace
    (SYS_PTRACE) nor Landlock (kernel ≥ 5.13) available on this runner — see §6.4.2`) so the
    reason surfaces in the checks UI, not just an opaque red exit code. **§6.1.4 must record
    the Lane-B VPS runner's kernel version** as a prerequisite (so Landlock availability is
    a known fact, not a runtime surprise) and **document which enforcement path the runner
    uses**. A mandatory adversarial-egress gate that silently no-enforces is worse than a
    visible red — so the absence of both tiers blocks, it does not pass.
  This is a **distinct case from the benign §2.11.4
  gate** and proves the argv/build controls — not "all engines bundled" — close T9b.
- **Cancellation (§1.7/§1.11):** mid-batch cancel keeps finished items, discards the
  in-flight one with no partial leftover, never touches originals.

### 6.4.3 Integration tests — per-pair conversions (the real engines)

The heart of the reliability gate (§6.5). For **every** (source→target) pair
enumerated across §04 (the matrices in images/audio/video/documents/spreadsheets/
presentations + cross-category extract-audio/to-GIF), against the §6.4.5 corpus:
- the conversion **completes** with exit success and produces a **valid file of the
  target format**. **The MANDATORY validity gate is a per-format STRUCTURAL READER
  that decodes the output, NOT a magic re-detect** — re-detecting the output's magic
  bytes via §1.2 only proves the file *starts* with the right header (a truncated MOOV
  atom, a 0-duration track, a blank raster, a broken OOXML relationship graph all pass
  magic re-detect), so magic re-detect via §1.2 is an **optional cheap pre-screen
  only**, never sufficient on its own. The structural reader is per category and
  REQUIRED: `ffprobe` decodes the audio/video output and reports the expected codec
  (stream count > 0); the image decodes via `vipsheader` with nonzero dimensions;
  `pdftotext`/poppler opens the produced PDF *and returns nonzero text for a
  text-bearing source*; OOXML is `unzip`-able with a well-formed `[Content_Types].xml`;
  **CSV/TSV is parsed with a real RFC-4180 reader (the in-workspace `csv` crate, §3.5.6)
  and the leading `=`/`+`/`@` injection cells are asserted preserved literally as text
  (CSV-injection non-execution on the output side) — NOT a bare field-count parity,
  which passes on mis-quoted / embedded-newline output that is unparseable.** These are
  reinforced by the §6.4.5/`build-gates.md` G31 non-empty / output≠input /
  content-bearing sub-assertions;
- the **content-fidelity** spot-checks pass: CJK/RTL text survives doc/sheet/slide
  conversions (§2.10); image orientation is baked upright (§04 images); audio
  tags/cover-art round-trip where the target supports them (§04 audio); video
  remux-vs-reencode chose the lossless path when codecs already fit (§04 video);
- the **lossy disclosure** is asserted to fire **iff** the pair is flagged lossy in
  §04 (and, for video, based on the *planned* remux-vs-reencode disposition, not the
  static pair) — the §2.9 catalog string is shown for exactly the flagged cases.
- **Patent-gapped pairs (§3.4):** on a platform where §3.4 marks a target
  unavailable, the integration test asserts the target is **absent/disabled** (not
  attempted) rather than failing — honest unavailability, not a test failure.

#### 6.4.3a Corpus↔pair bijection guard (the non-circular gate, made concrete) `[DECIDED]`

SSOT §9 makes "the corpus exists and backs every pair" a precondition; this is the
**machine-checkable** form (the check §6.10 row 3 names but no section previously
specified). A CI script — **`scripts/check-corpus-coverage.rs` run via `cargo run`
(a `cargo xtask`-style Rust bin) `[DECIDED]`** (the earlier `.*` wildcard is fixed to
Rust so the guard reuses the §0.6/§04 types and the `engines.lock`/manifest parsers
already in the workspace, rather than a second language drifting from them), run in
**Lane A** §6.7.1 (cheap, no engines), asserts a **bijection** between the §04 pair
matrices and the corpus `manifest.toml`:

1. **Enumerate every v1-required `(source → target)` pair** from the §04 matrices
   (images/audio/video/documents/spreadsheets/presentations + the two cross-category
   ops), excluding diagonals/`out`/`—` cells and pairs §3.4 marks `unavailable` on
   *all* platforms.
2. **Union the `covers` lists** from every corpus `manifest.toml` entry.
3. **Fail CI if any required pair has zero backing corpus files** (a pair with no
   `covers` entry) — *and* fail if any `covers` entry names a pair that does **not**
   exist in the §04 matrices (a stale/typo'd coupling). Both directions of the
   bijection are checked, so the gate cannot rot.

This is what makes the §6.5 reliability gate **non-circular**: a pair literally
cannot be declared `reliable` without a corpus file whose `covers` list names it.

### 6.4.4 Cross-platform test runs

The integration + property suites run on **all three native CI legs** (§6.1.4) —
the reliability bar is *per-platform* (SSOT: "on all three platforms"). Additional
platform-specific concerns:
- **WebView rendering drift (§0.3.1):** a light UI smoke test catches
  WebView2/WKWebView/WebKitGTK layout/behaviour differences in the core flow. **The
  driver differs by platform `[DECIDED §6.4.6]`:** the **§6.4.6 `tauri-driver` WebDriver
  flow runs on Windows and Linux only**. On **macOS** `tauri-driver` has **no WKWebView
  driver**, so the WebView-rendering-drift check there is covered by the **§6.4.6 degraded
  smoke test** (launch + synthetic-argv conversion + window/output/exit-0 assertions)
  **plus** the §6.6 human walkthrough — there is no macOS WebDriver flow to implement.
- **macOS TCC** file-access prompts that the beside-source default can trigger
  (§7.2) **cannot be answered headlessly**, so they are **NOT** exercised in the automated
  smoke run (which writes only to a temp dir, where no TCC prompt fires, §6.4.6) — the
  **TCC-prompt exercise is a §6.6 human-walkthrough item**.
- **LibreOffice headless is NOT safely parallel** (§0.9) — the office-pair
  integration tests must run LibreOffice **serialized**; the harness honours the
  §0.9 concurrency-degree config so the test environment matches production.
- **Output-determinism floor is per-platform (§2 / G32):** the enumerated determinism
  pairs (≥ 1 per engine per output-format category) run on all three native CI legs, so a
  platform-specific non-determinism (an OS encoder build embedding a timestamp the others
  do not, or uninitialised padding) is caught; a category whose only pair is §3.4-unavailable
  on a platform is covered by another available pair in that category on that platform.

### 6.4.5 The real-world input corpus (concrete contents) `[DECIDED — required v1 asset]`

The corpus is a **required v1 asset and a precondition for declaring any pair done**
(SSOT) — without it the reliability gate is circular. It lives in the repo (or an
LFS/release-asset store if size demands) under `tests/corpus/` (a **repo/CI-only test
asset — never bundled into the installed application**; a `tests/corpus/` path in the
staged app bundle is caught by the G35 test-asset-exclusion sub-check), **organised by
source format**, with a `manifest.toml` recording for each file: source format,
provenance/licence (corpus files must themselves be redistributable — public-domain
/ CC0 / self-produced / synthetic) **plus a non-empty `provenance` source** (a URL, a
generator-script reference, or `self-produced`) making each licence declaration
human-auditable — **G24a** asserts the `provenance` record's presence + that every
`tests/corpus/` file is manifested (byte-level licence inference is not machine-decidable,
so the gate enforces the auditable RECORD, not a content-licence claim), the **properties
it is chosen to exercise**, the
**expected outcome** per target (success / specific fail-clearly kind / specific
lossy note), and a **`covers` list** — the explicit `(source, target)` pairs this
file backs (the coupling field that makes the §6.4.3a bijection guard machine-
checkable). Manifest shape (per file):

```toml
[[file]]
path     = "images/iphone_p3_orientation6.heic"
source   = "HEIC"
licence  = "CC0"          # must be redistributable
provenance = "https://example.org/source"  # auditable source / generator-ref / self-produced (G24a)
exercises = ["orientation-bake", "ICC-P3", "HDR-10bit"]
covers   = [              # the (source→target) pairs this file backs (§6.4.3a)
  ["HEIC", "JPG"], ["HEIC", "PNG"], ["HEIC", "WEBP"], ["HEIC", "AVIF"],
]
[file.expect]             # expected outcome per target
"HEIC→JPG"  = { result = "success", lossy = "image_lossy_codec" }
"HEIC→AVIF" = { result = "success", lossy = "image_lossy_codec" }
```

> **Two pair encodings, deliberately distinct `[DECIDED]`.** `covers` uses the
> **structured array form** `["HEIC", "JPG"]` (an array of `[source, target]` 2-tuples)
> because the §6.4.3a bijection guard **parses** it programmatically and matches each
> 2-tuple against the §04 matrix cells — no string-splitting on `→`. The
> `[file.expect]` keys use the **`"SOURCE→TARGET"` label string** purely as a
> human-readable TOML table key for the per-target expected outcome. The guard reads
> `covers` (the array form) **only**; it never parses the `→` keys. A CI lint asserts
> every `[file.expect]` key has a matching `covers` 2-tuple (so the two stay in step)
> — but the machine-checkable coupling is always the array.

> **Manifest layout + discovery `[DECIDED]`.** There is **ONE root manifest
> `tests/corpus/manifest.toml`** listing **all** `[[file]]` entries by their
> `tests/corpus/`-relative `path` (e.g. `images/iphone_…heic`) — **not** per-category
> manifests. `scripts/check-corpus-coverage.rs` reads exactly that single file (a fixed
> path, no globbing of manifests), unions every entry's `covers`, and checks the bijection
> against the §04 matrices. (A single manifest keeps discovery trivial and the bijection
> guard's input deterministic; the per-source-format *directory* organisation is just file
> layout, independent of the one manifest.) The guard also asserts every `[[file]].path`
> exists on disk, so a manifest entry can't reference a missing corpus file.

Concrete required contents:

**Images** (`tests/corpus/images/`)
- Real **iPhone HEIC** photos (HDR, 10-bit, with EXIF orientation tags 1/3/6/8, GPS,
  ICC Display-P3) — the canonical HEIC→JPG everyday case + orientation-bake + ICC.
- **JPEG** with EXIF orientation, progressive, CMYK, 12-bit, truncated-tail.
- **PNG** RGBA, 16-bit, palette, **APNG** (animation-collapse case).
- **WEBP** lossy, lossless, **animated**, with alpha.
- **AVIF** still + **animated (`avis`)**, HDR/wide-gamut.
- **GIF** static + multi-frame **animated** (first-frame-collapse + passthrough).
- **TIFF** multi-page, 16-bit, CMYK, big-endian; **BMP** 24/32-bit, top-down/bottom-up.
- **ICO** multi-resolution (16/32/48/256) + non-square (padding case).
- **SVG** with intrinsic size, viewBox-only, missing-font, `.svgz`, a **remote
  `<image href>`** (must NOT be fetched — offline/security assertion), and a
  pathological tiny-viewBox-huge-render (must fail-clearly, not OOM).

**Audio** (`tests/corpus/audio/`)
- One file per source format (MP3 incl. VBR/CBR + ID3v2 + cover art; WAV 16/24/float;
  FLAC with Vorbis comments + cover; raw-ADTS `.aac`; **M4A holding AAC** *and* a
  separate **M4A holding ALAC** — the detection-by-codec case; OGG-Vorbis;
  `.opus`; AIFF; WMA v2/Pro/Lossless).
- Multichannel (5.1) source (channel-preservation, no silent downmix).
- A **>16-bit source** (bit-depth-reduction-to-default-16-bit disclosure case).
- Files with **non-Latin/CJK/RTL tag text** (tag fidelity, §2.10).
- Corrupt/truncated + 0-byte + a `.mp3` that is really FLAC (mislabel) cases.

**Video** (`tests/corpus/video/`) — short clips (a few seconds) to keep runtime sane:
- **MP4 (H.264+AAC)** → the lossless-remux baseline; **MOV from iPhone (HEVC)** →
  the §04 HEVC-default `[DECIDED]` case (re-encode→H.264 by default); **MKV** with **multiple audio tracks + SRT +
  ASS + PGS subtitles + chapters + font attachments** (the keep/convert/drop policy);
  **WEBM (VP9+Opus, and a VP8 alpha clip)**; legacy **AVI (DivX+MP3)**, **WMV
  (VC-1+WMA)**, **FLV (H.264/AAC and old Sorenson)**, **MPG (interlaced MPEG-2 +
  AC-3 — deinterlace case)**, **M4V (DRM-free)**, **3GP (H.263+AMR-NB)**.
- A **DRM-protected FairPlay `.m4v`** and a DRM WMV (must fail-clearly).
- A **portrait/rotated** clip (rotation honoured); a **VFR screen recording**
  (to-GIF fps-normalise); a **silent** clip (extract-audio "no audio track" case);
  a long-ish clip to exercise the to-GIF guardrail/cap (§cross-category).
- **to-GIF bijection coverage `[DECIDED]`:** the §6.4.3a corpus↔pair bijection guard
  requires every `(source → target)` pair to have a backing corpus file, so **each
  video source corpus item's `covers` list MUST include its `["<SOURCE>", "GIF"]` pair**
  (e.g. the MP4 item's `covers` includes `["MP4","GIF"]`, the WEBM item's includes
  `["WEBM","GIF"]`, …) — not just one generic "long-ish clip" for all of to-GIF.
  Otherwise the guard fails for most `video→GIF` pairs at Lane A. (One clip may *also*
  be the dedicated guardrail/cap exerciser, but the `covers` lists must collectively
  name every offered `video→GIF` pair.)

**Documents** (`tests/corpus/documents/`)
- **DOCX/DOC/ODT/RTF** real-world samples incl. **non-Latin (CJK) + RTL (Arabic/
  Hebrew)** body text, embedded images, a doc referencing a **non-bundled font**
  (substitution/reflow case), tracked-changes, a macro-enabled `.docm` (macro must
  drop, never execute). **Font floor `[DECIDED]`:** the CJK/RTL fidelity assertion tests
  against the **committed bundled font set (§3.9.3: Liberation + Carlito + Caladea +
  curated Noto CJK/RTL)** — the corpus CJK/RTL glyphs must render (no tofu) **from the
  bundled set alone**, so a missing-font regression fails the gate rather than silently
  degrading to host-font substitution.
- **PDF**: a text PDF (→TXT extraction), a **scanned/image-only** PDF (near-empty
  extraction, no OCR — honest), a **password-protected** PDF (fail-clearly), a
  malformed/truncated PDF (**poppler tolerance — Ghostscript is NOT shipped v1, §3.1; an
  unrecoverable PDF must poppler-fail-clearly per §2.8**), a tagged/AcroForm PDF.
- **TXT** in UTF-8/UTF-16/Windows-1252 + an invalid-byte file (fail-clearly, no
  mojibake); **MD** (GFM: tables/task-lists/code-fences, a local image + a remote
  image-URL that must not be fetched, YAML front-matter); **HTML** (article-like →
  PDF faithful; a JS/complex-CSS page → must render static-only, no JS exec, no
  remote-asset fetch).

**Spreadsheets** (`tests/corpus/spreadsheets/`)
- **XLSX/XLS/ODS** with formulas, multiple sheets (multi-sheet→CSV one-sheet
  behaviour + note), charts/conditional-formatting (drop-on-CSV), hidden rows/cols,
  a **macro-enabled `.xlsm`** (drop macros), a **password-protected** workbook
  (fail-clearly), a **>65k-row** sheet (XLS legacy-limit lossy case), a sheet with
  CJK/RTL cell text.
- **CSV/TSV**: comma, **semicolon (European decimal-comma)**, tab, pipe samples;
  UTF-8-BOM, UTF-16, Windows-1252; embedded-newline-in-quoted-field (RFC-4180);
  ragged rows; a **leading `=`/`+`/`@` cell** (CSV-injection: must import as text,
  never evaluate); a leading-zero value (`0123`, mis-typing-defence case).

**Presentations** (`tests/corpus/presentations/`)
- **PPTX/PPT/ODP** with animations/transitions (flatten-to-PDF), **embedded video/
  audio** (poster-only on PDF), embedded vs non-embedded fonts (substitution),
  SmartArt/WordArt (cross-family approximation), CJK/RTL slide text, a
  macro-enabled `.pptm`, a password-protected deck (fail-clearly), a several-hundred-
  slide deck (size pre-flight).

Corpus files **must be redistributable**; where a real-world artifact can't be
licensed for the public repo, a **synthetic equivalent** that reproduces the same
structural property is used and noted in the manifest. **`[DECIDED]` corpus storage
(adopting the [REC]): small synthetic + CC0 files committed in-repo** (so the per-PR
fast lane and the bijection guard §6.4.3a always have them), **larger real-world
media in an LFS-backed `corpus-large`** fetched **only** for the full Lane-B gate run
(§6.7.2), never required for the per-PR fast lane. Target total size co-owned with
§3.9. **[DEFER:** the exact total LFS size is calibrated as the corpus is filled.**]**

**Minimum-content gate (the corpus is *content*-complete, not just pair-complete)
`[DECIDED]`.** The §6.4.3a bijection guard proves only that every pair has *a* backing
file — an all-ASCII / all-Latin corpus would pass it while leaving SSOT *v1 DoD*'s
"non-Latin/RTL text, representative audio/video" content requirement unverified. So
`scripts/check-corpus-coverage.rs` **additionally** asserts (failing Lane A / §6.7.1 if
any is absent) that the root `manifest.toml` contains **at least one `[[file]]` whose
`exercises` (or a dedicated `content` tag) names each of**:
- **`cjk-body`** — ≥1 Office document (DOCX/ODT/XLSX/PPTX) with **CJK body text**;
- **`rtl-body`** — ≥1 Office document with **RTL (Arabic/Hebrew) body text**;
- **`non-ascii-encoding`** — ≥1 **CSV/TSV in a non-ASCII encoding** (e.g. UTF-16 or
  Windows-1252) and ≥1 TXT in a non-UTF-8 encoding;
- **`non-latin-tags`** — ≥1 **audio file with non-Latin ID3/Vorbis tag text**;
- **`representative-av`** — ≥1 real **audio** and ≥1 real **video** clip (already implied
  by the per-format rows; the lint makes the floor machine-checkable);
- **`real-image`** (image floor `[DECIDED]`) — the bijection guard alone could pass §6.10
  row 3 ("every pair works") on an all-synthetic / all-plain raster set, undercutting the SSOT
  "real photos" clause, so the content floor **additionally** requires each of: **≥1 HEIC**
  (real iPhone-class photo, the headline decode path), **≥1 AVIF**, **≥1 SVG** (the
  librsvg/no-base-URL path), **≥1 multi-size ICO** (the ICONDIR-count / `magicksave` or
  in-core-assembler path), and **≥1 PNG with an alpha channel** (the §2.9 `image_alpha_flatten`
  source). A missing image-floor tag is a build failure exactly like the others.

The required tag set is a fixed list in the lint; a missing tag is a **build failure**, so
the content floor cannot silently regress. This is the machine-checkable backing for §6.10
DoD rows **3** (corpus exists) and **15** (real-world filename + content fidelity).

### 6.4.6 UI / end-to-end (the core-UX-flow gate)

A headed browser-driver run drives the built app through **`tauri-driver`** — which
exposes a **WebDriver** endpoint over the platform WebView (WebKitGTK on Linux,
WebView2 on Windows). **`tauri-driver` officially supports ONLY Windows and Linux —
there is NO macOS WKWebView driver**, so the macOS leg degrades to the synthetic
smoke test defined in the per-platform sub-bullet below (it does **not** open a macOS
`tauri-driver`/WebDriver session). **E2E client binding = WebdriverIO (JS),
NOT the Rust `webdriver`/`fantoccini` crate `[DECIDED]`.** The client is **WebdriverIO**
(the JavaScript/Node WebDriver client), not a Rust webdriver crate, **because the a11y
contrast gate uses `@axe-core/webdriverio`** (§6.4.6a) — a **JS-only** package that
cannot be driven from a Rust client; choosing the Rust crate would force hand-rolling the
axe-core injection (inject + run axe via `execute_script`, capture JSON). With WebdriverIO
the `@axe-core/webdriverio` integration is first-class and the contrast session reuses the
same driver session as the flow E2E. (The earlier "WebdriverIO, or the Rust crate" hedge
is resolved to **WebdriverIO**.) **Version pin `[DEFER: implementation]` → WebdriverIO v9**
(the W3C-WebDriver-only major, aligned with `tauri-driver`'s W3C session model; pinned in
§0.8 alongside `@axe-core/webdriverio`). **`wdio.conf.js` capabilities for `tauri-driver`:**
the session is configured with **`tauri:options`** (the `application` = the built app
binary path, plus any `args`) and a **`tauri-driver` host/port** WebdriverIO connects to
(`tauri-driver` proxies to the platform driver — `msedgedriver` on Windows,
`WebKitWebDriver` on Linux); no Chrome/Firefox capability block. **Two concrete
per-platform driver facts `[DECIDED]`:** (a) **Linux:** `tauri:options.application` must
point at the **extracted ELF binary, NOT the `.AppImage`** — the AppImage is a
self-mounting wrapper WebDriver cannot launch as a process target, so CI first runs
`./ConvertIA.AppImage --appimage-extract` (or `--appimage-extract-and-run`) and points
`application` at the extracted binary under `squashfs-root/usr/bin/`. **The binary name is
resolved DYNAMICALLY, NOT hardcoded `[DECIDED]`:** the extracted binary name matches the
**case-sensitive Tauri `productName`** (e.g. `ConvertIA`, not a hardcoded lowercase
`convertia`), so CI **globs `squashfs-root/usr/bin/*`** (or reads `productName` from
`tauri.conf.json`) to find the actual binary rather than assuming a lowercase name — a
hardcoded `convertia` would not exist if `productName` is `ConvertIA`. **Cleanup:** after the
E2E, CI runs **`rm -rf squashfs-root/`** so the extracted tree does not accumulate across runs
or contaminate the artifact/disk-budget. (b) **Windows:** the
**`msedgedriver` version MUST match the runner's installed WebView2/Edge runtime** (a
mismatched msedgedriver fails to attach) — the CI step resolves the runner's WebView2 build
and fetches the matching `msedgedriver`. Concrete pin + capabilities
block are an `[DEFER: implementation]` detail to finalise against the pinned `tauri-driver`
minor at build time (also record the `tauri-driver` minor against which the default port
`4444` holds). **Note:** plain **Playwright cannot drive a Tauri WebView**
in its normal CDP mode (Tauri is not a Chrome DevTools-Protocol target); it is *not* the
E2E driver here. The run exercises
the full §5.2 flow per platform: empty → intake → collected/confirm → target+default →
destination shown → progress → summary → open-folder. **The empty/Idle step also asserts
the "all conversion happens locally, on your machine" reassurance line is present
`[DECIDED]`** (SSOT *Offline / privacy* surfaced on Idle, §5.2 row 1 / §5.7) — a cheap
string-presence check so the offline reassurance can't silently drop. This is the automated
half of the DoD **core-UX-flow** gate; the human half is §6.6. Frontend component/unit tests use
**Vitest** (§0.8).

- **Native file-drop is NOT automatable** by `tauri-driver` (the OS-level
  native drag-drop event §5.4 cannot be synthesised by a WebDriver). So the
  **automated E2E uses the file-picker path** (C2a `pick_for_intake` via the §5.10
  accelerator, which fills the *same* §7.8.1 funnel → `PendingIntake` → C1
  `drain_intake` completion door as a drop, §1.1) to
  reach Collecting; the **native drop itself is validated in the human walkthrough**
  (§6.6), where a real person drags a real file. This split keeps the automated gate
  honest about what it can and cannot synthesise.
- **Linux E2E needs a virtual display `[DECIDED]`:** the Linux `tauri-driver` leg runs
  on a headless CI runner, but **WebKitGTK will not initialise without an X/Wayland
  display** — so the leg must run **under `Xvfb`** (e.g. `xvfb-run -a ...`) or a Wayland
  headless compositor. The CI Linux E2E step wraps the driver in `xvfb-run`; without it
  the WebView never comes up and the E2E silently can't start.
- **Per-platform E2E driver `[DECIDED — macOS degrades to a defined smoke test]`:** the
  Windows and Linux legs use `tauri-driver` (WebDriver; Linux under `Xvfb` per above).
  **`tauri-driver` officially supports ONLY Windows (Edge WebDriver) and Linux
  (WebKitWebDriver) — there is NO macOS WKWebView driver in the official stack** (Apple's
  `safaridriver` automates *Safari itself*, not a WKWebView embedded in a desktop app, so
  it does **not** apply here — the earlier `safaridriver` reference was incorrect). So the
  **macOS automated leg is explicitly the degraded test, not full WebDriver:** CI
  **launches the built app, drives a synthetic `argv` conversion of one corpus file through
  the launch-intake path (§7.8/§1.1), and asserts (a) the window/process is present, (b)
  the expected output file appears, and (c) exit 0** — a scripted launch + presence/output
  assertion. The **WebView UX flow on macOS is covered by the §6.6 human walkthrough**
  (which also tests the Sequoia Gatekeeper / per-sidecar quarantine recovery). This is the
  knowable macOS DoD-#7 satisfaction; the Win/Linux full-WebDriver E2E is firm. (If a
  third-party WKWebView WebDriver bridge — e.g. an in-app W3C WebDriver server plugin —
  later proves it can drive an unsigned WKWebView on CI, the macOS leg is upgraded to full
  WebDriver — a bonus, not a gate.)
- **macOS smoke test: Gatekeeper quarantine + TCC `[DECIDED]`.** The smoke test runs against
  a **locally-built artifact on the same runner**, which does **NOT** receive
  `com.apple.quarantine` (that xattr is set only on *downloaded* files), so the launch needs
  **no `spctl`/`xattr` bypass** — provided the smoke test runs on the **build-output
  `ConvertIA.app` directly** (no archive/re-extract step between build and smoke). **If the
  pipeline zips and re-unzips the `.app` before the smoke test**, the re-extracted copy IS
  quarantined and the smoke step MUST first run **`xattr -rd com.apple.quarantine
  ConvertIA.app`** before launch. **Phase-3 path: run the smoke test on the build-output dir
  (no zip round-trip) → no quarantine handling needed.** **TCC `[DECIDED]`:** TCC
  file-access prompts **cannot be answered headlessly**, so the automated smoke leg **writes
  to and reads from a `TMPDIR`/temp dir only** (no Desktop/Documents/Downloads access), where
  **no TCC prompt fires**; the **TCC-prompt exercise (beside-source default touching a
  protected folder) is moved to the §6.6 human walkthrough** (§6.4.4's "macOS TCC … in the
  headed smoke run" is corrected to: TCC is a §6.6 human-walkthrough item, the automated leg
  stays in a temp dir to avoid prompting).

#### 6.4.6a Automated accessibility assertions (DoD basic-a11y owner) `[DECIDED]`

The DoD **basic-a11y** gate (keyboard path + readable contrast/sizes, WCAG 2.1 AA per
§5.6) has both a human half (§6.6 keyboard-only walkthrough) and an **automated half
owned here** (it had no named tool/lane before):

- **Tool & lane:** **`axe-core` (`^4.4`)** run via **`vitest-axe`** — a real, published npm
  package (a Vitest-native fork of `jest-axe`; npm `latest 0.1.0`, with a `1.0.0-pre`
  prerelease track; deps `axe-core ^4.4.2`, peer `vitest >=0.16.0`) `[DECIDED — verified on
  npm]`. **Pin `[DECIDED]`:** pin **`vitest-axe@0.1.0`** (the stable `latest`) in §0.8; if
  the `1.0.0-pre` line stabilises before Phase 3, bump to it. **Honest fallback note:**
  `jest-axe` is **not** "wrong because the runner is Vitest" — it works fine under Vitest's
  Jest-compatible matcher API; `vitest-axe` is preferred purely for first-class Vitest
  ergonomics. If `vitest-axe` were ever unavailable, the fallback is **`axe-core` +
  `@testing-library/react` with a manual `axe(container)` call under Vitest** (or `jest-axe`)
  — the gate is "axe-core runs against the tree under Vitest", not a specific wrapper.
  Used against the rendered React component tree under **Vitest**, as a **Lane-A** step
  (§6.7.1) — no WebDriver session needed for the static ARIA/role/focus checks, so it runs
  per-PR. **jsdom limitation `[DECIDED]`:** axe-core under jsdom **cannot measure computed
  contrast** (jsdom applies no CSS/layout), so the **WCAG-AA contrast check does NOT run on
  the Vitest/jsdom leg** — it runs on the live WebView via the **`@axe-core/webdriverio`
  session against the §6.4.6 `tauri-driver`** run (Linux/Windows, where real computed
  styles exist). The jsdom leg covers only **ARIA/role validity + focus-order**.
- **Concrete assertions:** (a) **WCAG 2.1 AA contrast** — ≥4.5:1 for normal text, ≥3:1
  for large text and UI components/graphical objects (axe `color-contrast` rule, run in
  both Light and Dark themes, §5.5) — **on the `@axe-core/webdriverio` Linux/Windows
  session only** (jsdom cannot compute contrast). **macOS automated-coverage gap
  `[DECIDED]`:** because `tauri-driver` has **no macOS WKWebView driver** (§6.4.6), there
  is **no automated contrast check on macOS** — the macOS WCAG-AA contrast gate is
  satisfied **only by the §6.6 human walkthrough's readable-contrast check** (an explicitly
  acknowledged gap, not a silent skip; recorded in `docs/usability-floor.md`); (b) **ARIA role/state validity** (no
  invalid or orphaned roles; the §5.6 `radiogroup`/`radio` tiles carry valid
  `aria-checked`) — jsdom leg; (c) **focus-order / tabindex sanity** (roving-tabindex
  radiogroup, §5.6) — jsdom leg — and labelled
  controls. Any axe violation at the configured impact level **fails the build**.
- **Text-size half of the gate `[DECIDED]`:** axe-core does **not** check font size, so the
  **minimum-body-text-size** half of the §5.6 gate (body copy ≥ `--text-base` = 16px, §5.5)
  is **verified by the §6.6 human walkthrough**, not by the automated leg — the walkthrough
  confirms body copy renders at the §5.5 `--text-base` floor (`--text-xs`/`--text-sm` reserved
  for supplementary labels). (An optional belt-and-suspenders computed-`font-size` assertion
  on the `@axe-core/webdriverio` session — no main-content text element below 16px — MAY be
  added in the Lane-B run, but the human walkthrough is the operative v1 text-size gate.)
- **Cross-ref:** the rendered colours come from the §5.5 design tokens; this gate is what
  makes the §5.6 "WCAG 2.1 AA" claim verifiable rather than aspirational.

---

## 6.5 The reliability gate (DoD operationalised) `[DECIDED]`

> SSOT: *every sensible source→target pair across all categories works reliably on
> all three platforms*; "reliably" = passes fail-clearly + no-harm on the corpus;
> the corpus existing is a precondition.

### 6.5.1 The unit of "done" and how a pair is declared reliable

The unit is the **individual (source→target) pair** (a category is just its set of
pairs). A pair is **`reliable`** when, **on each of the three platforms** (or on
each platform where §3.4 says it is available), against **every corpus file of that
source format**:
1. it produces a **valid, correctly-detected output** of the target format
   (§6.4.3 structural check), **and**
2. it upholds **no-harm** (originals byte-identical; atomic write; no-clobber) and
   **fail-clearly** (the corpus's known-bad inputs for that source produce the
   *expected* failure kind, batch continues), **and**
3. its **lossy disclosure** matches §04 (fires iff flagged), **and**
4. content fidelity holds for the properties the corpus exercises (CJK/RTL,
   orientation, tags, channels, remux-when-possible).

A pair that meets all four on all three (available) platforms is marked `reliable`
in the **pair-status ledger** (§6.5.2). **No pair is "done" until the corpus run
backs it** — this is what makes the gate non-circular.

### 6.5.2 The pair-status ledger (machine-checkable)

CI maintains a generated **pair-status report** (`reliability-report.json` +
human table) keyed by `(source, target, platform)`, each cell ∈
`{reliable, failing, unavailable-per-§3.4, demoted}`. The **release gate** is:

> **Every enumerated pair is `reliable` on every platform where it is not
> `unavailable-per-§3.4` or explicitly `demoted`.** Any `failing` cell blocks the
> release. The report is published as a release asset (transparency).

This directly realises the SSOT *v1 DoD* conversions clause. Because v1 is **one
large all-or-nothing release with no deadline** (SSOT), the gate has no
time-pressure escape hatch — internal *sequencing* (fill/validate category by
category) is allowed (SSOT) and the ledger naturally tracks that progress.

### 6.5.3 Recording the two permissible exceptions (as release-note items)

Both SSOT exceptions are recorded **in the ledger and the release notes**, never as
silent omissions:
- **Exception 1 — patent per-platform gap (§3.4).** A pair `unavailable-per-§3.4`
  on a platform (e.g. an HEVC-encode-dependent HEIC target, or — the category's
  hardest dependency — H.264/AAC for the **MP4 default target**) is an **explicit,
  documented, honestly-surfaced** release-note line ("HEIC encoding is unavailable
  on Linux because no openly-redistributable HEVC encoder ships there"). The UI
  shows it unavailable (§5.2). **§3.4 owns the decision; this gate owns recording
  it.** *Critical dependency note:* MP4 is the default target of **every** video
  source (§04 video), so the gate **flags it as a hard precondition** that §3.4
  decide H.264/AAC `ship-bundled` on all three platforms — a platform without an MP4
  default target is a product-level problem the release notes must call out, not a
  per-format footnote.
- **Exception 2 — last-resort reliability demotion.** An in-scope, licence-clean
  pair that **genuinely cannot meet the bar despite reasonable effort** may be
  `demoted` to *Future Ideas (Parked)* as an **explicit, documented release-note
  item**, so one stubborn pair can't block the whole release forever. Demotion is a
  **last resort**, requires a recorded rationale in the ledger, and is **never** a
  convenience cut. (SSOT.)

**Concrete shape & home of a "release-note item" `[DECIDED]` (so rows 16/17 are non-stub).**
A release-note item for a demoted or patent-gapped pair is **not** free-form prose; it is a
structured entry with a fixed home and required fields, so Phase 3 has an exact anchor:
- **Home:** a tracked `docs/demoted-pairs.md` table in the repo (the single canonical file),
  **plus** a one-line summary mirrored into the release `CHANGELOG.md`/GitHub Release body for
  that version. The §6.5.2 pair-status ledger is the machine-readable source; `docs/demoted-pairs.md`
  is its human-readable, release-attached projection.
- **Required fields (each row):** **(a)** the pair (`source → target`, e.g. `HEIC encode`/
  `RTF → MD`); **(b)** the **kind** (`patent-gap-per-platform` (exception 1) | `reliability-
  demotion` (exception 2)); **(c)** the **affected platform(s)** (all / Linux / macOS /
  Windows); **(d)** the **reason** (one sentence — e.g. "no openly-redistributable HEVC
  encoder ships on Linux", or the recorded reliability-failure rationale); **(e)** the
  **ledger ref** (the §6.5.2 entry id) and, for exception 1, the §3.4 `engines.lock`
  `available=false` row it derives from.
- **CI tie-in `[DECIDED]`:** the §6.8 governance-completeness gate verifies that **every
  ledger entry in state `unavailable-per-§3.4` or `demoted` has a matching `docs/demoted-pairs.md`
  row** (and vice-versa — no orphan rows), so a silent omission **fails the release**. This is
  what makes §6.10 rows 16/17 ("release-note item (§6.5.3)") a concrete, machine-checkable
  gate rather than a stub.

### 6.5.4 Re-validation on engine bump

When §3.8 bumps a bundled engine (best-effort currency), the **full reliability gate
re-runs** before that engine version can ship (§6.3.4) — a patch must not silently
regress a pair. The ledger diff (pairs that changed status) is part of the bump's
review.

---

## 6.6 Usability-floor check `[DECIDED — gate; per-platform human walkthrough]`

> SSOT *v1 DoD*: *an ordinary non-technical person can complete each named
> conversion unaided on first try (drop → pick → convert → find output), validated
> by at least one informal non-developer walkthrough per platform.*

This is a **release gate** the automated §6.4.6 E2E flow **cannot** replace — it
specifically tests whether a *human who didn't build it* succeeds. Protocol:

- **Who:** ideally one non-developer per platform (Windows, macOS, Linux), but the
  binding requirement is the **"Tester sourcing" [DECIDED] block below** — **≥1 genuine
  non-dev walkthrough on ≥1 platform**, with the owner permitted to run the remaining two
  (solo-project reading, recorded by the SSOT owner). The SSOT usability walkthrough is
  also the natural place to validate the genuinely-debatable per-source defaults flagged
  in §04 (XLSX→CSV vs →PDF; MP3-source→WAV vs FLAC; MOV-as-target demand).
- **What they must complete unaided (the named conversions) `[DECIDED — representative
  samples, not an exhaustive set]`:** `mov→mp4`, `png→webp`, `heic→jpg`, `mp3` source →
  its default, `docx→pdf`, `xlsx→csv`, `pptx→pdf`, plus the two cross-category ops
  (extract-audio → MP3; a clip → GIF). **These are one-per-category illustrations of the
  usability bar, NOT a reduced gate subset** (matching the SSOT framing of the reliability
  set as "illustrations of the bar, not a reduced subset"): the *reliability* gate (§6.5)
  still covers **every** enumerated pair; this human floor samples **one representative
  conversion per category** because a human cannot run thousands of pairs. Each via the
  **two-click common path** (drop → already-highlighted-or-pick target → convert) with
  **no instruction**.
- **What "counts" (pass criteria):** for each task the tester, with no help,
  (1) understands the empty screen and drops/browses a file; (2) sees the collected
  summary and confirms; (3) reaches a sensible result with the **pre-highlighted
  default** (no required choices); (4) sees **where it will save before converting**;
  (5) on completion uses **open-folder/open-file** and **finds the output**; (6)
  hits no stack trace, no cryptic message, no dead end. A task where the tester
  gets stuck or needs help **fails** the floor for that platform → fix → re-walk.
- **macOS Sequoia first-launch + sidecar quarantine (mandatory macOS sub-test)
  `[DECIDED]`:** the macOS walkthrough **must run on Sequoia (15.x)** from a freshly
  **downloaded** (quarantined) artifact, and the tester must succeed at **both** the
  blocked-app first-launch recovery (Privacy & Security → "Open Anyway", since the
  Control-click bypass is gone) **and** the **first-conversion** step where an
  independently-quarantined sidecar may be blocked — confirming the in-app
  `QuarantinedByOs` guidance (§2.8/§7.2.4) and the §6.2.4 download-page steps actually
  get a non-technical user through. If the unsigned build leaves the tester stuck at
  Gatekeeper or a silent first-conversion failure, the macOS floor **fails** → revisit
  the unsigned posture / the guidance copy → re-walk.
  - **Single-instance double-extract sub-test (macOS, posture assumption) `[DECIDED]`:**
    `tauri-plugin-single-instance` identifies "the same app" by bundle identity, which on
    an **unsigned `.app` launched from two separately-extracted copies** may not be
    recognised as one instance. The macOS walkthrough therefore **extracts the `.app`
    twice and launches from both** to confirm the §7.1.1 single-instance / refuse-busy
    hand-off behaves (or to **document the limitation** if two unsigned copies run as
    independent instances — an accepted v1 edge, not a silent gap).
- **Accessibility floor (part of the same gate, SSOT *For anyone*):** at least one
  walkthrough completes the core path **keyboard-only** (per the §5.10 shortcut map)
  and verifies readable contrast/text-size; this checks the DoD **basic-a11y** gate
  with a human, complementing automated a11y assertions (§5.6).
- **Screen-reader smoke pass (SSOT Principle 10 "a screen-reader path exists")
  `[DECIDED]`:** in addition to the keyboard-only pass, **one screen-reader walkthrough**
  steps through the core flow **Idle → Collecting → Confirm → Converting → Summary** with
  the platform's native SR — **VoiceOver** (macOS), **NVDA** (Windows), **Orca** (Linux) —
  on **at least one platform** (axe-core/§6.4.6a cannot *prove* usable announcement, only
  ARIA validity). **This is the verification gate for the §5.6.1 implementable SR
  contract** — the walkthrough follows the §5.6.1(3) per-state SR traversal table and
  confirms, against §5.6.1(1)/(2): every state has a reachable non-orphaned landing
  element; the collected summary and confirm-gate string are announced (assertive,
  §5.6.1(2)); progress milestones are announced (not every tick); the decision dialogs
  announce as `alertdialog` with their accessible name (§5.6.1(1)); and lossy/divert notes
  announce politely. Recorded in `docs/usability-floor.md` (which SR, which platform);
  referenced from the §6.10 DoD row 6 (basic accessibility). This closes the gap that the
  keyboard-only walkthrough alone left (SR announcement quality is otherwise unverified).
- **Recording:** results captured in `docs/usability-floor.md` (per platform:
  tester profile, tasks, pass/fail, observed friction, the default-validation notes).
  This file is a **required v1 artifact**; the gate is "three platform walkthroughs
  recorded, all named conversions pass" before release.
- **Staleness criterion (machine-checkable) `[DECIDED]`:** each walkthrough record
  carries a **`release_line` (the version/tag it validated)** and a **`date`**. The
  Lane-B §6.7.2 stage 5 gate **fails if the recorded `release_line` does not match
  the release being built** (or, absent a version match, if the recorded **`date`
  predates the date of the git tag triggering the Lane-B run** — concretely
  `date >= git log -1 --format=%ai <tag>`, the tag's commit date) — so an old
  walkthrough cannot silently satisfy a new release. (CI checks the *evidence's*
  freshness against the **tag date**, unambiguously; the human does the walkthrough.)

**Tester sourcing `[DECIDED]` (was `[OPEN-6.6a]`; ConvertIA is a solo/hobby project):**
the gate is satisfiable by **at least one genuine non-developer walkthrough on at least
one platform**, with the **owner (developer) permitted to perform the remaining two
platform walkthroughs where a non-developer tester is not available** — recording in
`docs/usability-floor.md` which walkthroughs were non-dev vs owner-run. "Non-developer"
means *did not build or contribute code to ConvertIA and is not given instructions* (a
first-time user). Rationale: the SSOT floor's intent (a human who didn't build it
succeeds) is preserved by requiring ≥1 true non-dev pass; the solo-project reality is
accommodated without dropping the gate. More non-dev testers are used if cheaply
available; the macOS Sequoia quarantine sub-test (above) should preferentially get a
non-dev tester since it is the highest non-technical-user blocker.

> **SSOT note `[DECIDED — SSOT amended]`.** The SSOT *v1 DoD* usability-floor gate has
> been **amended at the source** (SSOT §9, recorded owner amendment with footnote): its
> wording is now "≥1 genuine non-developer walkthrough overall, owner may run the
> remaining platform walkthroughs", exactly matching this §6.6 text — so this is **no
> longer a spec relaxation of a literal SSOT gate** (the prior "SSOT-acknowledged
> decision" framing) but a spec faithfully implementing the amended SSOT. The SSOT intent
> — a human who didn't build it succeeds unaided — is preserved by the ≥1 genuine non-dev
> pass. §6.10 DoD row 11 matches this wording exactly.

---

## 6.7 CI/CD `[DECIDED — two-lane pipeline]`

Two lanes, reflecting the platform CI standard (`reference_cicd_setup.md`:
reusable workflows + a green-`main` deploy-gate) adapted from a
*server-deploy* model to a *desktop-release* model (there is **no server deploy** —
ConvertIA is a downloadable artifact; the "deploy" is a GitHub Release).

> **Single-branch model `[DECIDED — P0 review r3]`.** ConvertIA builds **direct to
> `main`** in a single-Build-Loop model: **no second branch, no merge step, no
> auto-merge** (security-concept §2). The enforcement is therefore **CI green on
> `main` + required status checks on every push to `main`** — a red `main` is **fixed
> immediately**, never bypassed. The reference's "branch-protection + auto-merge +
> before-merge" framing does **not** apply to our flow; it is replaced throughout this
> section by **per-push validation on `main`**. The **one** residual `PR` concept that
> legitimately survives is the **external fork pull-request**: ConvertIA is a *public*
> OSS repo, so an external contributor can still open a fork PR, and the **G56 fork-PR
> secret guard** (the secret-bearing release job never runs on a fork PR) is retained —
> justified by *external contributors*, **not** by our own direct-to-`main` flow. Where
> a gate below reads "per-PR", read **"per-push (on `main`)"** unless it is explicitly
> the fork-contributor guard.

### 6.7.1 Lane A — per-push validation on `main` (fast, every change)

Runs on the **self-hosted Linux runner** (cheap; `reference_self_hosted_ci_runner.md`)
for the OS-agnostic checks, fanning to the matrix only for compile-sanity:
1. **Lint/format:** `cargo fmt --check`, `cargo clippy -D warnings` (enforces the
   platform **no-`any`/no-unwrap-sloppiness** quality bar), ESLint + `tsc --noEmit`
   (no `any` — CLAUDE.md global rule), Prettier, `yamllint` (via `python3 -m`, per
   the platform runner PATH workaround in the recent commits).
2. **Rust↔TS type drift check (§0.4.5):** the codegen tool **`tauri-specta` (DECIDED,
   §0.4.5; + specta)** regenerates the shared types and CI **fails if the committed types
   differ** (enforces the IPC contract + "no `any`").
3. **Unit + property + fault-injection tests (§6.4.1/§6.4.2)** — Rust + Vitest;
   fast, engine-light, run on every PR.
4. **Corpus↔pair bijection guard (§6.4.3a):** `scripts/check-corpus-coverage.rs`
   (a `cargo run`/xtask Rust bin, §6.4.3a) asserts every §04 v1-required pair has ≥1
   backing corpus `covers` entry (and no stale couplings). Engine-free, fast — runs every
   push so coverage gaps surface before the expensive Lane B corpus run.
4a. **Defaults-registry "no required choices" guard (§1.6) `[DECIDED]`:** an
   engine-free xtask **generates the §1.6 consolidated `OptionDecl.default` index from the
   §04 tables** and **fails the build if any §04-offered `(source,target)` pair lacks a
   complete default option set** (every declared option must have a `default`). This is the
   single machine-checkable home of the SSOT *v1 DoD* "no required choices" gate (§6.10 row
   7); it runs every push so a pair/option shipping without a default reddens `main`
   (fixed immediately). The generated index is the §1.6-owned artifact (values still
   owned by §04).
4b. **Automated a11y assertions (§6.4.6a) — jsdom leg = ARIA/role + focus-order ONLY:**
   `axe-core` via `vitest-axe` over the rendered React tree asserts **ARIA-role/state
   validity** and **focus-order / roving-tabindex sanity**. **The WCAG 2.1 AA contrast
   check does NOT run here** — axe-core under jsdom **cannot measure computed contrast**
   (jsdom applies no CSS/layout, §6.4.6a); contrast (both themes) runs on the live WebView
   via the **`@axe-core/webdriverio`** session in **Lane B** (§6.4.6a / §6.7.2). Engine-free,
   fast — runs every push; any jsdom-leg violation fails the lane.
5. **Compile-sanity on the matrix:** `cargo check` / a debug `tauri build` on all
   three legs to catch platform-specific breakage early (no full corpus run here).
6. **`cargo audit` / `cargo deny`** (advisory + licence policy, §6.3.4).
The enforcement is **CI green on `main` via required status checks on every push to
`main`** (single-branch model, security-concept §2) — a red Lane A **fails the push /
reddens `main`** and is **fixed immediately**, never bypassed; there is **no merge step
and no auto-merge** to gate. (The set of Lane-A checks that are *required status checks*
on `main` is asserted in CI by build-gates **G56a** — the branch-protection/ruleset
config that makes a red run actually block is otherwise invisible repo state.)

> **Lane-A blind spot (acknowledged) `[DECIDED]`:** the **offline-egress hard gate is
> Lane-B-only** (tag-triggered, §6.7.3), so an accidental network call introduced on a push
> is **not** caught per-push. The compensating **per-push** guards are: the **§2.11.4
> packet-monitor *property tests*** (core — assert no socket opens in the conversion path,
> run in Lane A step 3) and the **§0.10 WebView CSP** (frontend — no remote `connect-src`,
> verified by the type/config checks); the §6.7.3 adversarial-egress corpus is additionally
> **pulled forward into the per-push L4 leg** where the enforcement path is available
> (build-gates G42 / the per-push adversarial-egress pull-forward). The full OS-egress-deny
> E2E run remains the Lane-B backstop. This keeps the blind spot bounded rather than silent.

### 6.7.2 Lane B — Release pipeline (tag-triggered, the full gate)

Triggered by a release tag (e.g. `v1.0.0`) on `main`. Stages, **in order**, each
blocking the next:
1. **Matrix build (native, §6.1.4):** stage engines per platform (§6.1.3), run
   `tauri build` (+ the Windows post-build zip-packaging step §6.1.2) → per-platform
   artifact (Windows portable `.zip` **only** — NSIS NOT shipped v1, §6.1.2
   `[DECIDED-6.1a]`; universal `.dmg`; AppImage). **Artifact-size gate `[DECIDED]`:**
   immediately measure each platform's
   **compressed** artifact and **fail the release if any exceeds the §3.9.2 ≤ 400 MB
   compressed ceiling** (record the measured sizes as a release-asset line; §6.10 row 22).
   **No-system-pollution post-launch check (§6.10 row 21)** also runs here on the built
   artifact (Procmon/`fsusage`/`strace` — assert no registry/LaunchAgent/daemon/association
   writes).
2. **Full reliability gate (§6.5):** integration + property + corpus + E2E on **all
   three** legs; emits `reliability-report.json`. **Any `failing` pair aborts the
   release.** **Includes the WCAG-AA contrast a11y session `[DECIDED]`:** the
   **`@axe-core/webdriverio` contrast check** (§6.4.6a — WCAG 2.1 AA, both themes) runs as
   a **named step on the Linux + Windows legs** here (it needs the live WebView's computed
   styles; jsdom cannot compute contrast, so it is **NOT** in the Lane-A per-push a11y leg).
   **macOS contrast is the acknowledged automated-coverage gap:** `tauri-driver` has no
   macOS WKWebView driver (§6.4.6), so the **macOS WCAG-AA contrast gate is verified ONLY
   via the §6.6 human walkthrough** (readable-contrast check) — Phase 3 must not silently
   skip macOS contrast; it is human-covered, recorded in `docs/usability-floor.md`. **Runtime / cost:** the dominant cost is the corpus run (video
   re-encode + LibreOffice, the slow engines). Estimate **~30–90 min per leg**
   depending on corpus size; set CI **`timeout-minutes` ≈ 120 per leg** with headroom.
   **Per-OS summary `[DECIDED]`:** the **120-min** figure applies to the **Linux +
   Windows** legs only; the **macOS** leg starts at **180** (the escalation ladder below) —
   a CI YAML authored from a flat 120 would under-time macOS. **macOS-leg caveat +
   mitigation ladder `[DECIDED]`:** the **~30–90 min** estimate is
   **optimistic for the macOS leg** — it pulls `corpus-large` over **GitHub LFS** (no
   VPS-local cache, unlike the self-hosted Linux leg) **and** `macos-latest` minutes bill
   **~10×** Linux/min (§6.1.4 budget note), so the 120-min figure has thin headroom. **The
   escalation ladder is concrete (not ad-hoc) `[DECIDED]`:**
   1. **Initial macOS `timeout-minutes = 180`** (not 120) — give the LFS pull + 10×-cost leg
      real headroom from the start.
   2. **Trigger to split `[DECIDED]`:** the **operative v1 trigger is TWO CONSECUTIVE macOS
      Lane-B runs each exceeding 180 min, OR a single run exceeding 240 min** — actionable from
      run 2 / run 1 respectively, while a **one-off** slow or LFS-congested macOS run does
      **not** permanently switch the leg to a subset corpus (which could hide a macOS-specific
      failure). A bare single-run-over-180 trigger is too sensitive to runner variability, so it
      is **not** the v1 gate. The **3-consecutive-run average > 150 min** rule is **post-v1
      only** (a smoothing refinement once a release history exists). On the trigger firing,
      switch the macOS leg to a **representative macOS subset**:
      one video re-encode pair (the slowest engine), one office→PDF pair (the LibreOffice
      path), one image-worker pair, and the E2E smoke — **the §6.6 video/office smoke set** —
      while the **full `corpus-large` continues to run on the cheaper Linux leg** (which has
      the VPS-local LFS cache and 1× cost).
   3. **Subset selection criterion (so it is not ad-hoc):** pick the **slowest pair per
      engine family** (one each: video, office, image, audio) plus any pair whose §3.4
      disposition is *macOS-specific*, so the macOS subset still exercises every macOS-unique
      code path; pairs with no macOS-specific behaviour are covered by Linux only.
   The full corpus always runs on at least one leg (Linux), so coverage is never lost — only
   the redundant macOS re-run of platform-identical pairs is trimmed if the timeout bites.
   **Intra-leg parallelism** is bounded by the **§0.9 concurrency degree** and must
   honour the **LibreOffice-serialised** constraint (the office-pair tests run LO
   single-slot — the harness **imports the §0.9-owned `MAX_LO_CONCURRENCY` const** (and
   likewise the §0.9-owned **timeout / watchdog consts** — per-engine wall-clock timeout,
   watchdog poll interval, no-progress threshold), NOT a
   hard-coded `1` (or hard-coded timeout values), so the test env can never drift from prod).
   The **`corpus-large` LFS set is fetched only for this Lane-B run** (never the
   per-PR fast lane, §6.4.5). **Runner `[DECIDED]`:** the **Linux** Lane-B leg runs on
   the **self-hosted VPS runner** (same as Lane A, §6.1.4) — it has local/cheap LFS
   bandwidth and disk for `corpus-large`; the **macOS/Windows** Lane-B legs use the
   GitHub-hosted runners (no self-hosted equivalent), so for those `corpus-large` is
   pulled over GitHub LFS bandwidth — a budgeted, tag-only cost (release frequency is
   low, §6.1.4 budget note).
   - **Self-hosted-runner capacity & contention analysis `[DECIDED]`.** The VPS runner
     (12 vCore / 24 GB RAM / 720 GB NVMe) is **shared** with four other Ne-IA projects'
     Lane-A CI. A Lane-B Linux corpus run is **disk- and CPU-heavy** (corpus-large LFS
     +
     staged engines + `tauri build` artifacts + transient scratch can reach tens of GB;
     the slow LibreOffice/video legs saturate cores for 30–90 min) and must **not starve
     the other projects' fast lanes**:
     - **Concurrency isolation:** the Lane-B job runs under a **dedicated runner label /
       `concurrency` group** with **`max-parallel: 1`** for corpus jobs, and is **niced /
       cgroup-capped** (CPU + IO weight) so co-scheduled Lane-A jobs still get a slice.
       Tag-triggered Lane-B is **rare** (release-only), so a single long run is tolerable;
       it **should not materially delay Lane-A on typical PR loads**. **Fallback trigger
       `[DECIDED]`:** if a Lane-B Linux run **measurably delays Lane-A on more than one
       occasion**, **migrate the Lane-B Linux leg to a dedicated GitHub-hosted runner**
       and keep the VPS runner **Lane-A-only** (the GitHub-hosted Linux fallback already
       documented above becomes the standing arrangement).
     - **Disk budget:** the runner reserves headroom for the worst-case sum
       (corpus-large + 3-leg artifacts + scratch); the post-run cleanup (`docker`/build
       cache + corpus checkout) is mandatory so 720 GB is never exhausted.
     - **LFS hosting:** `corpus-large` lives in the **Ne-IA org LFS quota**, but the
       Linux leg uses a **persistent VPS-local LFS cache** (clone once, reuse) so repeat
       runs do not re-egress the org quota; only the macOS/Windows legs pull over GitHub
       LFS bandwidth (the budgeted tag-only cost above). If org LFS egress becomes the
       bottleneck, the Linux leg is the cache of record. (Revisiting GitHub-hosted Linux
       for Lane-B remains the documented fallback if VPS contention proves unmanageable.)
3. **SBOM + NOTICE assembly + attribution-completeness gate (§6.3):** generate
   CycloneDX (app + engines), assemble `NOTICE`/`THIRD-PARTY-LICENSES.txt`, run the
   §6.3.3 completeness check. **A missing/UNKNOWN attribution aborts the release**
   (release-blocking, same status as no-harm).
4. **Name/trademark clearance gate (§6.9):** assert the clearance record is present
   and current; if a rename was required, assert it propagated (the §6.9 check).
   **Blocks if not cleared.**
5. **Usability-floor evidence gate (§6.6):** assert `docs/usability-floor.md` records
   passing walkthroughs for all three platforms for this release line. **Blocks if
   absent.** (Human step — its *evidence* is what CI checks.)
6. **Integrity hashing (§6.2.3):** compute SHA-256 per artifact, build `SHA256SUMS`
   covering **every** release asset, **and the minisign signature over `SHA256SUMS`**
   (§6.2.3 `[DECIDED]`). **Runner-host isolation `[DECIDED — P0 review r2]`:** this is
   the **only** step that holds `MINISIGN_SECRET_KEY`/`MINISIGN_PASSWORD`, and the
   user-facing trust substitute collapses to that one key, so the signing job runs on an
   **ephemeral GitHub-hosted runner** — **NEVER on the shared self-hosted VPS** that ran
   the stage-2 Lane-B Linux corpus leg (which processes `corpus-large` untrusted/
   adversarial files + fuzz inputs, §6.1.4). A persistent multi-tenant runner that
   handles untrusted input is the textbook host-compromise vector — once poisoned, every
   future release could be silently re-signed with the real key. The signing job and the
   untrusted-corpus/fuzz jobs declare **disjoint runner hosts** (no shared workspace, no
   shared host); the **GitHub-hosted signing job** runs under `step-security/harden-runner`
   (BLOCK mode) — its free/Community tier works only on GitHub-hosted runners (self-hosted
   needs a StepSecurity Enterprise license), so the shared self-hosted VPS leg relies on
   the §6.4.2 ptrace/Landlock fs-audit + the §6.7.3 nftables/strace egress monitor + the
   VPS egress allowlist + an ephemeral/JIT low-priv runner instead. A workflow lint
   (build-gates **G56**) fails any secret-using job bound to a self-hosted label. This
   implements security-concept principle 11.
7. **Publish to canonical GitHub Releases (§6.2.2):** upload artifacts + `SHA256SUMS`
   + `.sha256` files + SBOM (CycloneDX/SPDX) + `reliability-report.json` +
   `NOTICE`/`THIRD-PARTY-LICENSES.txt` as a **single coordinated release** (one
   large all-or-nothing v1 — SSOT). The release body restates as-is/no-warranty +
   the verify-your-hash recipe (§6.2.4) and lists the two-exception release-note
   items (§6.5.3).

There is **no auto-update/phone-home publishing step** — the updater is explicitly
disabled/absent (§7.6); users learn of releases by visiting the page
(user-initiated, SSOT).

### 6.7.3 What CI does *not* do

No code-signing, no notarization, no store submission (SSOT *Out of Scope*) — only
the in-code-required artifacts (SBOM, checksums) are produced. No telemetry, no
network-touching test that would contradict the offline invariant (§2.11).

**Offline-observability hard gate `[DECIDED]` (6.10 DoD #5 / §2.11.4).** This is a
**concrete release-blocking gate**, not "ideally enforced": the §6.4.6 E2E flow is run
with **egress blocked** and **any outbound attempt fails the test**. Per-platform tool:
- **Linux (Lane-B leg):** run the full E2E **inside a network namespace with egress
  blocked** — `unshare --net` (loopback only) or `iptables -A OUTPUT -j DROP` (allowing
  only the local WebDriver/IPC loopback). Any outbound packet aborts the run.
  **Precondition `[DECIDED]`:** `unshare --net` requires **unprivileged user namespaces**
  (`kernel.unprivileged_userns_clone=1` / `user.max_user_namespaces>0`), so the job runs a
  **preflight `unshare --net true` assertion with a clear diagnostic** ("net-namespace
  unavailable — enable unprivileged userns or run with `--cap-add NET_ADMIN`") and **fails
  loud rather than silently skipping the isolation**. **If the VPS runner runs inside Docker**
  (the §6.1.4 containerised path), unprivileged `unshare --net` may be denied by the default
  seccomp/AppArmor profile — in that case the job MUST use **`--network=none`** on the
  container **or** add **`--cap-add NET_ADMIN`** rather than relying on in-container `unshare`;
  the §6.1.4 kernel-recording requirement is the cross-ref for which path the runner host
  supports.
  **Composing the net-ns with the §6.4.6 `Xvfb` display (order-sensitive, pinned)
  `[DECIDED]`:** WebKitGTK needs a display, so the E2E must run under **both** `xvfb-run`
  **and** `unshare --net`. The **net-namespace wraps the entire `Xvfb`+E2E process**, and
  loopback must be brought up inside it:
  ```sh
  unshare --net -- sh -c '
    set -e
    ip link set lo up
    DRIVER_PORT="${DRIVER_PORT:-4444}"
    # Xvfb must NOT open a TCP X socket inside the loopback-only net-ns: distro Xvfb
    # packages vary on the -nolisten tcp default, and a TCP bind attempt fails here →
    # pass it explicitly so Xvfb uses only Unix-domain sockets (otherwise intermittent
    # CI failures when the default is "listen").
    xvfb-run -a --server-args="-nolisten tcp" tauri-driver --port "$DRIVER_PORT" &
    drv=$!
    # readiness probe: wait for the WebDriver endpoint before starting the client
    until curl -sf "http://127.0.0.1:${DRIVER_PORT}/status" >/dev/null; do sleep 0.5; done
    run_e2e_client                        # the WebdriverIO run (§6.4.6 client binding)
    rc=$?
    kill "$drv" 2>/dev/null || true       # tear the backgrounded driver down
    exit $rc
  '
  ```
  (The earlier one-liner backgrounded `tauri-driver` and immediately "ran the E2E" with
  **no readiness probe and no kill** — a race that either fails to connect or leaks the
  driver process; the probe + explicit `kill` + propagated exit code fix both. **Port
  `[DECIDED]`:** `tauri-driver` listens on **`4444` by default** and accepts **`--port`** to
  override (`tauri-driver --help`); the script passes `--port "$DRIVER_PORT"` explicitly and
  the readiness probe uses the same `${DRIVER_PORT:-4444}`, so the two can never disagree
  whether or not the default changes.)
  This ordering is safe because **X11 talks over a Unix-domain socket** under
  `/tmp/.X11-unix` — a filesystem object that **survives the network-namespace isolation**
  (only TCP/IP is namespaced) — so `Xvfb` + the WebView still get a display while **all
  network egress is blocked**. Bringing `lo` up inside the new netns is required for the
  local WebDriver/IPC loopback. Getting the nesting backwards (xvfb-run outside the netns,
  or forgetting `ip link set lo up`) yields either no display or a half-isolated gate that
  silently passes — hence the pinned form above.
- **macOS `[DECIDED — what actually runs here]`:** macOS has **no `tauri-driver` WKWebView
  driver** (§6.4.6), so the offline gate runs the **§6.4.6 synthetic-argv smoke test** (launch
  + argv-driven conversion + window/output/exit-0 assertions) — **NOT** the WebDriver E2E flow
  the Linux/Windows legs run — under a **`pf` outbound-deny** profile (`pf` anchor/rule blocking
  outbound to non-loopback) **plus** the §2.11.4 packet-monitor assertion. **`pfctl` needs
  `sudo`** to load the anchor; GitHub-hosted macOS runners have **passwordless sudo**, recorded
  as a **§6.1.4 runner-image assumption** (if a future runner image drops passwordless sudo the
  gate degrades to the packet-monitor alone). **Acknowledged gap (§6.10 row 5):** because the
  macOS leg cannot drive the WebView, the **WebView CSP offline property on macOS is verified by
  human walkthrough (§6.6) + static config inspection only**, NOT packet-monitored through a
  driven WebView — the argv smoke test packet-asserts the *core/engine* egress, not the
  WKWebView's; this is the one platform where the offline observability is not driver-monitored.
- **Windows `[DECIDED — packet-monitor is the load-bearing gate here]`:** a blanket
  "Firewall outbound-deny rule for the app" is **fragile for a portable, unsigned exe at a
  random `TEMP` path** (no stable program identity / install path to scope a rule to). So
  the enforcement is **either** a **per-run `New-NetFirewallRule -Program <resolved
  absolute path> -Direction Outbound -Action Block`** created **and removed** around the test
  (scoped to that run's actual exe path), **or** the process is launched inside an
  **AppContainer network-isolation profile** (an AppContainer with no network capability
  cannot open sockets). **A Job Object is NOT an option for network deny** — `JOB_OBJECT_LIMIT`
  flags govern memory/CPU/process-count/UI, not sockets (§2.12.3). **Note: unlike the Linux
  `unshare --net` net-namespace, the Windows per-program firewall rule is NOT a
  structural-equivalent hard isolation** (the AppContainer profile is the closest structural
  equivalent) — so on Windows the **§2.11.4 packet-monitor assertion is the real load-bearing
  gate** (the firewall/AppContainer is best-effort enforcement, the monitor is the proof).
  Run both.

The packet-monitor assertion (§2.11.4) — zero outbound packets observed for the whole
E2E — is the load-bearing proof on every platform; the OS-level egress block is the
enforcement that turns an accidental call into a hard failure rather than a silent
success.

---

## 6.8 Repo governance & policy artifacts `[DECIDED — concrete deliverables]`

The SSOT *License & Openness* mandates a specific set of in-repo documents; each is
a **Phase-3 authoring task** and several are **referenced by the release gates**.
All are English (public OSS repo). Mapping each to its SSOT origin and content owner:

| File | Required content | SSOT origin / owner |
|------|------------------|---------------------|
| **`LICENSE`** | MIT, header `Copyright (c) 2026 Ne-IA and ConvertIA contributors` (collective notice — inbound=outbound, **no assignment**). | SSOT *License & Openness*. Gate: present + name matches §6.9 clearance. |
| **`NOTICE`** + **`THIRD-PARTY-LICENSES.txt`** | Per-engine name+version, full licence text, written-offer-of-source for GPL-family. **Generated** from `engines.lock` + SBOM (§6.3.2), never hand-drifted. | SSOT *Engine-license policy*; **data owned by §3.7**, assembly here (§6.3), display §5.9. **Release-blocking** (§6.3.3). |
| **`CONTRIBUTING.md`** | Inbound=outbound under MIT; **no CLA**; **optional DCO sign-off** (`Signed-off-by`, *requested not required*); the **inbound-warranty clause** (contributors warrant submissions are their own work or compatibly-licensed for inbound MIT; incompatibly-licensed code is not accepted); how to run the test/lint lanes (§6.7.1); the quality bar **stated directly** in CONTRIBUTING (no `any`; no `// TODO`; no `console.log` in prod; no inline CSS; every change production-ready) — **not** by reference to a private `CLAUDE.md` (an internal file, not present in the public OSS repo). | SSOT *License & Openness* (contributions). |
| **`CODE_OF_CONDUCT.md`** | A standard CoC (Contributor Covenant-class) with the SECURITY/maintainer contact for enforcement. | SSOT *License & Openness* (a code of conduct accompanies the repo). |
| **`SECURITY.md`** | **Private vulnerability reporting** channel (GitHub private advisories + a contact); scope statement = ConvertIA opens **untrusted files through third-party decoders** → references the §0.11 threat-surface map and the §2.12 isolation posture; best-effort patch posture **with no SLA** (SSOT); how a reporter can include a (redacted, §7.5) repro from the local log. | SSOT *Security posture* / *License & Openness*; ties to §2.12, §7.5, §0.11. |
| **`PRIVACY.md`** | Plain-language restatement of **§2.11**: fully offline, **no network/telemetry/accounts/update-phone-home**; the only network is user-initiated (open project page, §7.7); the **cloud-sync caveat** (ConvertIA neither causes/prevents/detects your OneDrive/iCloud/Dropbox sync uploading files in a synced folder). | SSOT *Local/private/offline*; restates §2.11 (owner of the invariant). |
| **`TRADEMARK.md`** | The MIT grant covers **code, not the "ConvertIA" name or the Ne-IA logo**; forks/redistributions must use a **different name** and may **not** use the Ne-IA logo; guidelines for nominative use. | SSOT *Trademark*. |
| **`README.md`** (download + trust) | What it is, the **canonical-GitHub-Releases-only** download location (§6.2.2), the **verify-your-hash** recipe (§6.2.4), as-is/no-warranty + best-effort-security posture, supported-OS floor (§0.3.1), per-platform unsigned-build first-launch note, **plus the per-platform prerequisites (§6.2.4): Windows portable-zip WebView2 note (§0.3.1) and the Linux AppImage `libfuse2` note (§6.1.4)**. | SSOT *Distribution & download trust*. |
| **`.github/` policy** | Issue templates (default new format/feature requests to **Future Ideas (Parked)** per the SSOT inclusion test — SSOT *Out of Scope*); PR template referencing the DCO/quality bar; private-advisory config wired to `SECURITY.md`. | SSOT *Out of Scope* (inbound-request default) + governance. |

**DCO posture (explicit) `[DECIDED]`:** **no CLA**; a DCO **`Signed-off-by`** line
is **requested, not required** (SSOT: "a DCO sign-off may be requested, not
required"). CI **does not hard-block** an unsigned commit (that would make it
required) but **may surface a friendly reminder**; authorship is recorded in Git
history (the collective-notice / no-assignment model). The inbound-warranty clause
lives in `CONTRIBUTING.md`.

**Governance-doc completeness CI gate `[DECIDED]`.** Today only `LICENSE`
(name-match, §6.9) and `NOTICE`/`THIRD-PARTY-LICENSES.txt` (existence, §6.3.3) are
gated; a missing/stub `PRIVACY.md` / `SECURITY.md` / `TRADEMARK.md` /
`CODE_OF_CONDUCT.md` / `CONTRIBUTING.md` would **not** block release. So a **Lane-B
release-gate assertion** verifies all **five** SSOT-mandated governance docs are
**present AND non-empty** — a byte-count floor (e.g. ≥ 200 bytes, defeating an empty
placeholder) **plus** a `grep` for one required key section per file (e.g.
`SECURITY.md` → a "report" / private-advisory heading; `PRIVACY.md` → an "offline" /
"no telemetry" statement; `TRADEMARK.md` → the name/logo carve-out; `CONTRIBUTING.md`
→ "inbound=outbound" / the quality-bar list; `CODE_OF_CONDUCT.md` → an enforcement
contact). A missing/stub file **fails the Lane-B gate** (§6.7.2). This closes the gap
that a governance doc could silently ship empty. **The same gate also asserts the
`docs/demoted-pairs.md` ↔ ledger consistency `[DECIDED]`** (§6.5.3): every §6.5.2 pair-status
ledger entry in state `unavailable-per-§3.4` or `demoted` MUST have a matching
`docs/demoted-pairs.md` row (required fields present, §6.5.3) **and vice-versa** (no orphan
rows) — so a patent-gapped or demoted pair can never ship without its release-note item, making
§6.10 rows 16/17 a concrete machine-checkable gate. **Authoring owner `[DECIDED]`:** the
five governance docs are a **blocking Phase-3 authoring task owned by the project owner
(the §6.6 "owner" — the developer)**; the Lane-B gate checks **existence + non-emptiness +
the key-section grep**, NOT prose quality, so authoring the substantive content is an
explicit owner deliverable, not something the gate can substitute for.

**`[DECIDED-6.8a]`** (resolves the former `[OPEN-6.8a]`): a `GOVERNANCE.md`/maintainer
model doc is **NOT adopted for v1** — the seven files above satisfy the SSOT mandate; a
governance doc is added only if the contributor base grows (ConvertIA is a solo/hobby
project, so no maintainer-model doc is warranted yet). `[DEFER: post-v1]` by demand.

---

## 6.9 Name/trademark clearance gate + rename propagation `[DECIDED — release-blocking; process out-of-scope, doing-the-check in-scope]`

> SSOT *Naming* + *v1 DoD*: trademark/name-collision risk for **both** "ConvertIA"
> **and** the public use of the "Ne-IA" brand was flagged as a **precondition before
> first public release** (the name *could* change if a conflict were found). **Resolved
> `[DECIDED]`: the clearance verdict is `clear` for both marks** (§6.9.1 below; recorded
> in `docs/name-clearance.md`). The gate remains as "the clearance record is present +
> current" (a CI-checkable artifact gate, §6.9.2), and the rename machinery stays dormant.

**Scope split (important):** *registering* a trademark is **out of scope** (SSOT
*Out of Scope* — no store/cert/vendor process). **Performing the clearance check and
propagating any required rename** is **in scope** as a release gate — this is the
distinction the README open-questions log records.

### 6.9.1 The clearance check (the gate input)

A documented clearance review for **both** marks ("ConvertIA" and the public "Ne-IA"
brand) across the jurisdictions relevant to a globally-downloadable app (at minimum
EU/EUIPO, US/USPTO, and a sanity check on common app-distribution regions), plus
common-law / existing-product / domain / package-name collision search (crates.io,
npm, GitHub org, app listings). The result is recorded in
`docs/name-clearance.md` with: marks checked, jurisdictions/registries searched,
date, findings, and a **verdict ∈ {clear, conflict→rename, conflict→abort}**. This
is an **owner/human task**, not automatable — but its **evidence is what the gate
checks**.

**`[DECIDED]` clearance verdict = `clear`.** The owner has cleared **both**
"ConvertIA" and the public "Ne-IA" brand for v1 use. `docs/name-clearance.md` records
this verdict (`clear`), dated for the release line; the §6.9.2 gate asserts the record
is present and current. No rename is required, so the §6.9.3 mechanical rename
propagation stays dormant (a documented capability, not a v1 action). (The legal
*advice/registration* process remains out of scope per the SSOT; the **technical
gate** — assert a current `clear` record exists — is in scope and retained.)

### 6.9.2 The release gate (CI-checkable)

Lane B stage 4 (§6.7.2) **asserts** `docs/name-clearance.md` exists, is dated for
the current release line, and its verdict is `clear` **or** `conflict→rename` with a
completed rename (next clause). A `conflict→abort` or a missing/stale record
**blocks the release**. (CI checks the *record*; the human does the *check*.) For v1
the verdict is **`clear`** (§6.9.1 [DECIDED]); the gate's job is to confirm that
record stays present and current per release line.

### 6.9.3 Mechanical rename propagation (if a conflict surfaces)

If clearance returns `conflict→rename`, the rename is applied **before** release,
**never after** (SSOT). A single scripted, reviewable rename pass
(`scripts/rename-brand.*`) propagates the new name across **every** surface so no
stale "ConvertIA" leaks into a published build:
- repo/package identity: `Cargo.toml` (crate + `productName`), `package.json`,
  `tauri.conf.json` (`productName`, `identifier`, window title, bundle name),
  the GitHub repo + org references;
- **`LICENSE` / `NOTICE` / `TRADEMARK.md`** copyright/name lines;
- README, all §6.8 governance docs, the download page, the verify recipe;
- **branding**: the Ne-IA logo/app-icon assets and About-screen strings (§5.5/§5.9 —
  placeholders the owner controls), bundle icons, the `.desktop`/Info.plist names;
- the in-app product name strings (§5 UI-chrome) and the SBOM/`engines.lock`
  product field.
A post-rename CI **grep gate** asserts the **old name appears nowhere** in
shippable artifacts (a `rg` over the repo + the staged bundle for the old token,
excluding historical changelog entries). The rename is one atomic PR, reviewed,
then the release proceeds.

---

## 6.10 DoD-traceability checklist `[DECIDED — the "every behaviour has a home" table]`

Maps **every** SSOT *v1 Definition of Done* gate to its **owning spec section** and
the **§6 mechanism that verifies it**, so the README claim "every behaviour the SSOT
promises has a technical home" is **verifiable**. Each gate is marked
**in-scope-gate** (we implement+verify it) vs **out-of-scope-process** (the
*process* — e.g. registering a mark — is out; *doing/checking* it is in).

| # | SSOT DoD gate | Owning section(s) | §6 verification mechanism | Scope |
|---|---------------|-------------------|----------------------------|-------|
| 1 | **Every sensible source→target pair works reliably on all 3 platforms** | §04 (pairs) · §1 (pipeline) · §3 (engines) | Reliability gate / pair-status ledger (§6.5); integration+corpus tests (§6.4.3–6.4.5) | **in-scope-gate** |
| 2 | **"Reliably" = fail-clearly + no-harm on a real-world corpus** | §2.5 (no-harm) · §2.8 (fail-clearly) | Property/fault-injection (§6.4.2) + the corpus (§6.4.5) as precondition | **in-scope-gate** |
| 3 | **The corpus exists (required v1 asset, non-circular gate)** | this file | `tests/corpus/` + `manifest.toml` (§6.4.5); the **corpus↔pair bijection guard (§6.4.3a)** fails CI if any §04 pair has no backing corpus file (or a `covers` entry names a non-existent pair); **plus the §6.4.5 minimum-content gate** — fails CI unless the manifest tags ≥1 CJK-body + ≥1 RTL-body Office doc, ≥1 non-ASCII-encoding CSV/TSV, ≥1 non-Latin-tag audio file, representative A/V, **and the image floor (≥1 HEIC, ≥1 AVIF, ≥1 SVG, ≥1 multi-size ICO, ≥1 PNG-with-alpha)** (so the corpus is content-complete, not just pair-complete, and not all-synthetic/all-plain images) | **in-scope-gate** |
| 4 | **Everything runs fully offline (whole engine set bundled, no fetch)** | §3.3 (bundle-all) · §2.11 (offline invariant) | Bundling at build (§6.1.3); offline-observability E2E with egress blocked (§6.7.3); SBOM proves no runtime-fetch component | **in-scope-gate** |
| 5 | **Offline guarantee observably true (no network at all)** | §2.11 | Network-egress-blocked E2E run asserts zero calls (§6.7.3 / §6.4.6) | **in-scope-gate** |
| 6 | **Basic accessibility (keyboard path + readable contrast/sizes; **screen-reader path, SSOT Principle 10**; WCAG 2.1 AA per §5.6)** | §5.6 · §5.6.1 (SR contract) · §5.10 (shortcut map) | **Automated axe-core a11y assertions (§6.4.6a)** — **ARIA-role validity + focus-order run in Lane A (jsdom, §6.7.1)**; **WCAG 2.1 AA contrast (≥4.5:1 text, ≥3:1 large/UI, both themes) runs in Lane B on the `@axe-core/webdriverio` live-WebView session (§6.7.2)** — jsdom cannot compute contrast. **Text-size half (body copy ≥ `--text-base` = 16px, §5.5) is verified by the §6.6 human walkthrough** — axe-core does not measure font size (§6.4.6a). Plus the keyboard-only human walkthrough (§6.6) **and the §6.6 screen-reader smoke pass that walks the §5.6.1 SR contract** | **in-scope-gate** |
| 7 | **Core UX flow (drag/drop+picker+keyboard → same result; reacts to type; pre-highlighted default; destination shown before convert; visible cancellable progress; end-of-batch summary; one-click open-folder/file)** | §5.2 (states) · §1.1/§1.5/§1.11/§1.12 · §7.7 (open) | E2E flow per platform (§6.4.6) + usability-floor human walkthrough (§6.6) | **in-scope-gate** |
| 8 | **Unwritable/ephemeral-location fallback works** | §2.7 (per-location divert) · §2.14 (cross-volume) | Property tests on read-only/USB/network/temp locations (§6.4.2); divert path in corpus runs | **in-scope-gate** |
| 9 | **Every bundled engine's required licence text + attribution present and correct (NOTICE/third-party-licenses, backed by SBOM) — missing attribution release-blocking** | §3.7 (data) · §5.9 (display) | SBOM + NOTICE assembly + **attribution-completeness gate** (§6.3.3); blocks release | **in-scope-gate** |
| 10 | **Name/trademark clearance completed; any rename applied across repo/LICENSE/NOTICE/branding before release** | this file (§6.9) | Clearance-record gate (§6.9.2) + scripted rename propagation + old-name grep gate (§6.9.3) | **in-scope-gate** (the *clearance check + rename*); **out-of-scope-process** (*registering* a mark) |
| 11 | **Usability floor: ordinary non-tech person completes each named conversion unaided on first try; ≥1 genuine non-dev walkthrough on ≥1 platform (owner may run the remaining two — matches the AMENDED SSOT §9 gate, owner amendment recorded at the SSOT source, implemented in §6.6)** | §5 (UX) · this file (§6.6) | Human walkthrough recorded in `docs/usability-floor.md` (which were non-dev vs owner-run); evidence gate in Lane B (§6.6/§6.7.2) | **in-scope-gate** |
| 12 | **Published integrity hashes from one canonical source (trust substitute for no-signing)** | this file (§6.2) | SHA-256 + `SHA256SUMS` + **minisign signature (DECIDED, unconditional — Lane-B stage 6)** published to canonical GitHub Releases (§6.2.2/§6.2.3); verify recipe surfaced (§6.2.4) | **in-scope-gate** |
| 13 | **One artifact per platform (cross-platform, one product)** | §0.2 · this file (§6.1) | Build matrix artifact table (§6.1.2): Windows portable-zip (exe + bundled engines; **NSIS NOT shipped v1, `[DECIDED-6.1a]`**) · universal-dmg · AppImage | **in-scope-gate** |
| 14 | **No-harm / atomicity / fail-clearly hold even across crash/cancel/out-of-disk** | §2.1/§2.6/§2.8/§2.13/§2.14 | Atomicity-under-interruption + out-of-disk + panic-boundary property tests (§6.4.2) | **in-scope-gate** |
| 15 | **Real-world filename + content fidelity (Unicode/emoji/long-path; CJK/RTL/encodings; CSV delimiters)** | §2.10 · §04 (per-format) | Adversarial-name unit tests (§6.4.1) + CJK/RTL/encoding corpus files (§6.4.5) | **in-scope-gate** |
| 16 | **Patent per-platform gaps honestly surfaced (exception 1), never silent** | §3.4 (decision) · §5.2 (UI surfacing) | Ledger marks `unavailable-per-§3.4`; release-note item (§6.5.3); UI-unavailable assertion (§6.4.3) | **in-scope-gate** (recording/surfacing); patent **decision** owned by §3.4 |
| 17 | **Reliability-demotion (exception 2) explicit + documented, last resort** | §3.2/§04 (which pair) · this file | Ledger `demoted` state + recorded rationale + release-note item (§6.5.3) | **in-scope-gate** |
| 18 | **Single-instance + run identity (no cross-instance temp clobber; freeze unaffected by a second launch)** | §7.1 · §2.4/§2.6 | Single-instance plugin behaviour test; per-run/instance temp-ownership + advisory-lock liveness property tests (§6.4.2) | **in-scope-gate** |
| 19 | **Startup integrity & engine-presence (missing/corrupt engine → app-fault, not a crash)** | §7.2.3 · §2.13 | Startup-fault test: a removed/truncated bundled engine yields the plain app-fault screen, never a stack trace (§6.4.2 / §6.4.6 headed smoke) | **in-scope-gate** |
| 20 | **OS intake (Open-with / launch-args route through the single freeze funnel; no file-association pollution)** | §7.8 · §1.1/§2.4 | Launch-with-files E2E (UI enters Collecting at startup); assert no associations registered (§7.8.2) | **in-scope-gate** |
| 21 | **Portable, no installation, no system pollution (SSOT Principle 2 — no installer/admin/elevation/registry writes/no LaunchAgent or daemon)** | §7.4/§7.8.2 (explicit negatives) · §0.10 (capabilities) · §3.4.5/§3.3 (no runtime fetch) · §7.3 (no tray/agent) | **Lane-B post-launch assertion `[DECIDED]`:** run the built app under **Procmon (Windows)** / **`fsusage`+config-dir watch (macOS)** / **`strace`/inotify (Linux)** during a conversion and assert: **no writes outside the OS config/log dir + the user's chosen output** — specifically **no registry writes** (Windows, beyond none expected), **no `LaunchAgent`/`LaunchDaemon` install** (macOS), **no system-service/unit install** (Linux), **no file-association registration** (§7.8.2). A pollution write fails the gate | **in-scope-gate** |
| 22 | **Compressed artifact ≤ 400 MB per platform (§3.9.2 size ceiling)** | §3.9.2 (ceiling) · §3.9 (size levers) | **Artifact-size gate `[DECIDED]`:** an explicit **§6.7.2 Lane-B step** measures each platform's compressed artifact and **fails the release if any exceeds 400 MB compressed** (the §3.9.2 ceiling); recorded as a release-asset line | **in-scope-gate** |
| 23 | **English-only UI (SSOT Principle 11) — covered by construction (no i18n runtime)** | §5.7 (English-only owning statement) | **Principle-11 CI lint `[DECIDED]`:** a **Lane-A (§6.7.1)** static lint asserts **(a)** no i18n / locale-switching library is imported anywhere in the frontend (e.g. no `i18next`/`react-intl`/`Intl`-locale-negotiation in `package.json` deps or source) and **(b)** every `strings/ui.ts` key resolves to a **non-empty English string value** (no empty/placeholder/locale-keyed entries). Fails CI on any locale-switch path or a non-English/empty string key | **in-scope-gate** |
| — | **NOT a gate: subjective visual polish; engine-currency** | §5.5 (polish) · §3.8 (currency) | Polish is iterative (never blocks); currency is best-effort, re-validated against the gate when bumped (§6.3.4/§6.5.4) | **out-of-scope-gate** (explicit non-gates) |

If a future SSOT clause is added, it must appear here with an owning section and a
§6 mechanism, or it has no technical home — that is the check this table enforces.

---

## Open-questions log contributions (this section)

**Now `[DECIDED]` (this round) — adopted from their recommendations:**
- **[6.9a] Name/trademark clearance verdict = `clear`** for "ConvertIA" / "Ne-IA"
  (owner-cleared; §6.9.1). The release gate (record present + current) is retained;
  the legal *process* stays out of scope.
- **[6.2a]** Sign `SHA256SUMS` with a **project minisign key** — DECIDED yes (§6.2.3).
- **[6.1e]** CI runners — **GitHub-hosted for mac/win, self-hosted Linux for Lane A**
  (§6.1.4; budget note retained).
- **[6.1d]** CI engine-acquisition — **pinned, checksum-verified asset cache** hosted on
  **`actions/cache` keyed `<engine>-<version>-<triple>`** with a checksum-verified
  pinned-upstream-URL populate/fallback; macOS keeps **two per-triple keys per engine**
  (arm64 + x86_64) for the `lipo` universal build (§6.1.3).
- **[6.4a]** Corpus storage — **small CC0/synthetic in-repo + LFS `corpus-large` for
  the full gate** (§6.4.5); exact total size **[DEFER: calibrate as corpus fills]**.

Easy `[OPEN]`s resolved (not owner-level): artifact formats (§6.1.2: Windows
portable-zip [NSIS NOT shipped v1, **`[DECIDED-6.1a]`**] / universal-dmg / AppImage),
NSIS-vs-portable (**`[DECIDED-6.1a]`** — portable-zip only, §6.1.2), Linux `.deb`
(**`[DECIDED-6.1b]`** — AppImage-only v1, `.deb` deferred post-v1, §6.1.2), reproducible-build
depth (§6.2.5b), `GOVERNANCE.md` (§6.8a), usability
tester count (§6.6a) — each carries a **(recommendation)** or `[DECIDED]` inline.

**Genuinely still open / deferred (feed the README log):** the macOS automated E2E
under an unsigned build is **`[DECIDED]` — a defined degraded smoke test** (§6.4.6:
launch + synthetic-argv conversion + window/output/exit-0 assertions; WebView UX via the
§6.6 human walkthrough), **not** an open question; the one genuinely deferred number here
is the exact `corpus-large` total size (`[DEFER: corpus]`).
