# 03 — Engines & Bundling

> Which engines do the work, how they are selected, and how they ship — **all
> bundled, fully offline**. Origin: SSOT *Engine-license policy*, *Local/private/
> offline*, *v1 Definition of Done* (offline floor), *Cross-platform, one product*.
>
> **Ownership recap (what this file decides vs references).** This file OWNS: the
> engine inventory + licences (§3.1), the engine **registry/selection** abstraction
> and the trait (§3.2), the **bundling model** (§3.3), **per-platform packaging +
> the patent-disposition matrix** (§3.4), **per-engine argument construction**
> (§3.5), **copyleft isolation** (§3.6), **NOTICE/SBOM data generation** (§3.7),
> **engine maintenance/versioning** (§3.8), and the **binary-size budget** (§3.9).
> It REFERENCES, never restates: the per-format engine→pair mapping, options and
> lossy flags (`04-formats/*`); the IPC contract (§0.4); the project layout /
> module seam and concurrency degree (§0.7, §0.9); the generic
> spawn/progress/cancel/timeout invocation lifecycle (§1.7); the resource/size
> pre-flight (§1.10); the per-item progress model (§1.11); the no-harm / atomic /
> temp / cross-volume guarantees (§2.1–§2.7, §2.14); the error taxonomy + lossy
> string catalog (§2.8, §2.9); the **decoder-isolation boundary** (§2.12); the
> app-fault model (§2.13); the Tauri capabilities/CSP allowlist (§0.10); the in-app
> About listing presentation (§5.9); and the build/SBOM/reliability-gate pipeline
> (§6.1, §6.3, §6.5).

---

## 3.1 Engine inventory & licences `[DECIDED per format]`

The full engine set is fixed by `04-formats/*` (those files already chose every
per-pair engine; this section must **not** contradict them). v1 ships **five
top-level third-party engines** — libvips (image core, with its linked codec/delegate
components 1a–1d), FFmpeg, LibreOffice, poppler, pandoc (+ optional Ghostscript) —
plus ConvertIA's own in-core Rust text engine. Counting each separately-licensed
bundled component (the SBOM granularity, §3.7), the inventory rows below enumerate
every one; they cluster into four families:

| # | Engine (bundled artifact) | Family | Drives (cross-ref) | Licence | Ships as | Patent flag |
|---|---|---|---|---|---|---|
| 1 | **libvips** (raster core; built with libheif/libde265, libaom/dav1d, the native **`svgload` SVG load module (librsvg)**, **cgif** for native `gifsave`, and a **REQUIRED ImageMagick** delegate for BMP+ICO save and GIF fallback) | Images | `04-formats/images.md` (raster↔raster, SVG→raster, HEIC/AVIF **decode**, HEIC↔AVIF via `heifsave`) | **LGPL-2.1+** (libvips); **cgif MIT**; see per-component rows | linked lib **inside the image-worker process** (LGPL — dynamic link OK, §3.6) | none for its own codecs |
| 1a | **libheif + libde265** (HEVC decode) + **x265** (HEVC encode, built as a **dynamically-loaded libheif encoder plugin** `.so`/`.dll`/`.dylib`, not statically linked) — used by libvips' HEIC load module via `heifsave compression=hevc` | Images | HEIC decode (vips) / HEIC encode | libheif **LGPL-3.0**, libde265 **LGPL-3.0**, **x265 GPL-2.0-or-later** (verify vs the pinned source's `COPYING`; -or-later is compatible with the LGPL-3.0 libheif host, GPL-2.0-only would not be) | **x265 → dynamically-loaded libheif *plugin*, isolated** (§3.6); libheif/libde265 LGPL link | **HEVC → §3.4** |
| 1b | **AV1: libaom (enc, via libheif `heifsave compression=av1`) / dav1d (dec, via vips AVIF load module)** — the ONE bundled AV1 encoder is **libaom** (the standalone `libavif`+aom encoder is **not** bundled; encode standardised on `heifsave`, images.md [OPEN-1] [DECIDED]) | Images | AVIF decode/encode | aom **BSD-2 + patent grant**, dav1d **BSD-2** | LGPL/BSD link in the image worker | AV1 royalty-free; **ship-posture → §3.4** |
| 1c | **librsvg** (SVG rasteriser — libvips' native `svgload` module is librsvg-backed; resvg is NOT a libvips backend at any released version, so it is **not shipped** [DECIDED]) | Images | SVG→raster | **LGPL-2.1+** (librsvg) | linked load module inside the image-worker (LGPL — dynamic link OK, §3.6) | none |
| 1d | **ImageMagick** (libvips BMP/ICO save delegate — **REQUIRED**, plus GIF fallback) | Images | **BMP load+save, ICO save (`magickload`/`magicksave` — REQUIRED)**; GIF fallback | **ImageMagick License** (Apache-2.0-style, SPDX `ImageMagick`) — **permissive, NOT GPL** | linked delegate (permissive — no isolation); GPL *optional delegates* excluded at build | none |
| 1e | **libimagequant** (PNG/GIF palette quantisation — used by libvips' `cgif`/`gifsave` and palette PNG output) | Images | PNG/GIF palette quantisation | **BSD-2-Clause** (the permissive leg of the libvips-vendored fork's GPL-vs-BSD dual licence — verify the shipped leg; **NOT** BSD-3) | linked/vendored **inside the image-worker process** | none |
| 2 | **FFmpeg** (**GPL-2.0+ build** — `./configure --enable-gpl` to link `libx264`; built **without `--enable-nonfree`**: `libmp3lame`, `libvorbis`, `libopus`, native `aac`/`flac`/`alac`/`pcm`, `libx264`, `libvpx-vp9`, WMA *decoders*; no `libfdk_aac`) | Audio, Video, Cross-category | `04-formats/audio.md`, `video.md`, `cross-category.md` | **GPL-2.0+** (the whole binary, because it enables GPL `libx264`; the LGPL component libs are still dynamically linked beside it, §3.6.1); written-offer-of-source obligation | **separate invoked binary** (`ffmpeg`/`ffprobe`) per §3.6 | **AAC, H.264 → §3.4**; MP3/Vorbis/Opus/FLAC/ALAC/PCM/VP9 patent-clean |
| 3 | **LibreOffice** (headless `soffice`, Writer+Calc+Impress + PDF export filters; bundled with a baseline open font set, §3.9) | Documents, Spreadsheets, Presentations | `04-formats/documents.md`, `spreadsheets.md`, `presentations.md` (all office↔office + every `*→PDF`) | **MPL-2.0** (+ many bundled components — full set enumerated by the SBOM, §3.7) | **separate invoked binary** (sidecar process) per §3.6 | none |
| 4 | **poppler** (`pdftotext`) | Documents | `PDF→TXT` | **GPL-2.0/GPL-3.0** | **separate invoked binary** (§3.6) | none |
| 5 | **Ghostscript** **[DECIDED: NOT shipped v1]** (was a PDF read/repair backstop behind poppler; no user-facing pair) | Documents | (malformed-PDF tolerance — dropped) | **AGPL-3.0** | not shipped (`[DEFER: re-add if §6.5 corpus shows GS-salvageable PDFs]`) | none |
| 6 | **pandoc** | Documents | markup conversions (`MD/HTML/TXT ↔`, office→markup for XML/text sources) | **GPL-2.0+** | **separate invoked binary** (§3.6) | none |
| — | **ConvertIA native CSV/TSV engine** (Rust, in-core) | Spreadsheets | `CSV↔TSV`, encoding/delimiter sniff | **MIT** (own code) | compiled into the core | none |

**Per-family notes.**
- **libvips** runs **inside the image-worker process** (§0.7/§3.5.5 `[DECIDED]`),
  linked there rather than spawned as a standalone exe, because its job is many small,
  latency-sensitive image ops and it is LGPL (link-compatible, §3.6); the worker
  process gives the §2.12 isolation boundary. **ImageMagick is a REQUIRED bundled
  component, NOT a fallback `[DECIDED]`:** libvips has **no native BMP support at all**
  (BMP load *and* save go through the ImageMagick `magickload`/`magicksave` delegate)
  and **no native ICO saver** (ICO save is `magicksave`-only), so BMP (both
  directions) and ICO-save — all in-scope v1 image formats — **depend on ImageMagick**.
  (The native **cgif** `gifsave` claim is correct; ImageMagick is only a *GIF*
  fallback.) ImageMagick is **permissive — the ImageMagick License (an OSI-approved
  Apache-2.0-style licence, SPDX `ImageMagick`), NOT GPL** — so it is link-OK like the
  BSD/MPL components and is **not** an aggregation/isolation case. The only **GPL**
  component reachable from the image stack is **x265** (HEVC encode), the genuine
  aggregation case (a dynamically-loaded libheif encoder *plugin*, never statically
  linked — see §3.6). **Build caveat:** ImageMagick *optional delegates* can themselves
  be GPL, so the trimmed build **must exclude GPL delegates** — but IM core itself is
  permissive.
- **FFmpeg** is one binary covering three `04` categories (audio, video,
  cross-category). Because it links GPL `libx264` (`--enable-gpl`), the **whole FFmpeg
  binary is GPL-2.0+** (not LGPL) — shipped as a separate invoked binary so aggregation
  keeps the MIT core clean (§3.6.1); its written-offer-of-source obligation is honored
  (§3.6.2). `ffprobe` ships alongside it (same upstream, same licence) for the
  §video.md remux-vs-reencode probe.
- **LibreOffice** is one binary covering three `04` categories. It is the size
  driver of the whole product (§3.9).
- **Ghostscript is `[DECIDED: dropped in v1]`** — poppler's own fault tolerance plus a
  clean fail-clearly (§2.8) on the rare unrecoverable PDF is the lighter, AGPL-free
  choice; `[DEFER: re-add only if the §6.5 corpus shows poppler failing PDFs GS would
  have salvaged]`. This removes the AGPL surface entirely from v1 (§3.6 + §3.9).

Licence-class summary (drives §3.6/§3.7): **MIT** core; **LGPL** (libvips,
libheif/libde265, librsvg, and the FFmpeg *component* libs) dynamic-linked beside the
exe; **GPL** (the **FFmpeg binary itself** — GPL-2.0+ because it enables x264 —
plus x264, the **x265 libheif plugin**, poppler, pandoc) **always invoked or
dynamically-plugin-loaded, never statically linked into the MIT core**, each carrying
the written-offer-of-source obligation (§3.6.2); **AGPL** (Ghostscript) **not shipped
v1**; **MPL** (LibreOffice) invoked;
**permissive** — **BSD** (libaom/dav1d) and the **ImageMagick License** (ImageMagick,
SPDX `ImageMagick`, Apache-2.0-style) — both unrestricted and link-OK (ImageMagick is
**not** GPL and is a **required** component, not a fallback).

---

## 3.2 Engine registry & selection

### 3.2.1 Single-engine-per-pair rule `[DECIDED]`

Every v1 `(source → target)` pair is satisfied by **exactly one** engine in
**one** invocation — **no multi-step/chained conversions in v1** (a `A→(X)→B`
pipeline is out). Every `04-formats/*` file has already proven its cells obey this
(images' `HEIC↔AVIF` resolved via vips `heifsave compression=hevc|av1`; documents'
`DOC/RTF→markup` reassigned to LibreOffice because pandoc can't read them;
spreadsheets' `CSV↔TSV` carved out to the native engine). **Reachability audit
(this section's job): every non-`out`, non-diagonal cell across all six category
files maps to a single engine — verified, no pair is left reachable only by
chaining.** If chaining ever enters scope it needs its own home (intermediate
scratch on the §2.14 volume, per-step progress §1.11, step-attributed errors
§2.8); explicitly **not v1**.

### 3.2.2 The `Engine` trait (registry seam — physical home owned by §0.7)

The engine layer is a **registry of capability-declaring engines** behind one
trait. The trait lives in the engine-registry crate/module (§0.7 owns where);
this section owns its **shape and semantics**. Pseudo-signature (Rust):

```rust
/// A bundled conversion engine. One impl per engine binary/lib.
pub trait Engine: Send + Sync {
    /// Stable id for logging/SBOM/registry (e.g. "ffmpeg", "libreoffice", "vips").
    fn id(&self) -> EngineId;

    /// What this engine can do, *on this platform*, given the §3.4 patent
    /// disposition resolved at build time. Used to populate the registry and to
    /// decide per-platform availability (honest "unavailable here").
    fn capabilities(&self, platform: Platform, patents: &PatentDisposition)
        -> Vec<EngineCapability>;      // named struct (defined below): {source, target, direction}

    /// Build the concrete invocation plan for one job. Pure (no I/O, no spawn):
    /// returns argv / env / cwd / progress-parser kind / temp-output path shape.
    /// The actual spawn/cancel/timeout is owned by §1.7; this only *describes* it.
    fn plan(&self, job: &ConversionJob, out_tmp: &TempPath)
        -> Result<Invocation, PlanError>;

    /// How this engine reports progress so §1.11 can normalise it
    /// (FFmpeg `-progress` k=v, LibreOffice = coarse/none, vips = callback, etc.).
    fn progress_model(&self) -> ProgressModel;

    /// Map this engine's exit code + stderr into the §2.8 error taxonomy.
    /// Returns the §2.8-owned `ConversionErrorKind` (NOT a separate "FailureKind" —
    /// that name is dropped; §2.8 is the single owner of the failure-kind set).
    /// `ErrorKind` (§0.4.3) is the wire projection of `ConversionErrorKind`; the
    /// §06 drift check keeps the two byte-identical for the item-level variants.
    fn classify_failure(&self, exit: ExitStatus, stderr: &str) -> ConversionErrorKind;
}
```

Supporting types (named here for §3.5/§1.7; the §0.6 domain types are referenced,
the engine-layer-internal ones are defined here):

```rust
pub struct Invocation {
    pub program: EngineProgram,   // resolved bundled program to spawn (below)
    pub args: Vec<OsString>,      // fully constructed (§3.5)
    pub cwd: Option<PathBuf>,     // per-run scratch (§2.14)
    pub env: Vec<(OsString, OsString)>, // isolated/minimal env (§3.5, §2.12)
    pub stdin: StdinPlan,         // how stdin is fed (below) — see §3.5
    pub progress: ProgressModel,
    pub out_tmp: TempPath,        // engine writes here; §2.1 atomic-publishes on success
}

/// How the Rust core locates the program to spawn. Engines are spawned Rust-side
/// (§3.3.3), never via the WebView shell — the path is resolved through Tauri's
/// PathResolver (externalBin sidecar or a binary inside the resources tree §3.3.1).
pub enum EngineProgram {
    Sidecar(EngineId),            // externalBin, resolved to binaries/<name>-<triple>[.exe].
                                  //   The image-worker (libvips, §3.5.5) is ALSO Sidecar-class:
                                  //   a separate short-lived subprocess [DECIDED] (§0.6 note,
                                  //   §3.5.5, §3.6.1) — NOT linked into the core.
    ResourceBin { engine: EngineId, rel: PathBuf }, // a binary inside a resources tree (e.g. soffice)
    InProcessNative(EngineId),    // ConvertIA's own MIT in-core Rust engine — native CSV/TSV
                                  //   ONLY (§3.5.6). No spawn, no third-party native code; the
                                  //   ONLY non-subprocess engine. (There is NO in-process path
                                  //   for any decoder of untrusted bytes — §2.12.4 absolute.)
}

/// How the engine's stdin is supplied (§3.5; pandoc sometimes reads bytes on stdin).
pub enum StdinPlan { None, PipeBytes }

/// A pure planning error (no I/O): the engine cannot build an Invocation for this
/// job (e.g. an option value out of range). Mapped by §1.7 to a §2.8 kind
/// (typically InternalError/UnsupportedPair). Distinct from a runtime failure.
pub struct PlanError { pub kind: ConversionErrorKind, pub detail: String }

pub enum ProgressModel {
    FfmpegKeyValue { duration_us: u64 },   // denominator = ffprobe duration (video.md)
    VipsCallback,                          // libvips eval callback %
    CoarseSpawnDone,                       // LibreOffice/pandoc/poppler: 0%→spin→100%
}

// ─── Engine-layer types referenced by the trait (defined here, §3.2 is owner) ──
/// The running/target platform. Resolved at build/startup; drives both
/// `capabilities()` and the §3.4 patent disposition.
pub enum Platform { Win, MacOS, Linux }

/// Conversion direction of a capability cell (matches the 04 matrices' arrows).
pub enum Direction { Decode, Encode, Both }

/// The build-time-resolved patent/ship posture per encumbered codec on THIS
/// platform (§3.4). `Available` = shipped & usable; `Unavailable` = honestly gapped
/// (the only legitimate `select()` → None, surfaced as §2.8 PlatformUnavailable).
pub struct PatentDisposition {
    pub heic_hevc: CodecPosture,   // HEVC encode/decode for HEIC (§3.4)
    pub aac: CodecPosture,         // AAC (§3.4)
    pub h264: CodecPosture,        // H.264 (§3.4)
    // additional encumbered codecs added here as §3.4 evolves; default royalty-free → Available
}

pub enum CodecPosture { Available, Unavailable }

/// One capability a registered engine declares for a (source, target) pair on a
/// platform. Replaces the earlier bare `(SourceFmt, TargetFmt, Direction)` tuple
/// with a named struct so the registry/codegen surface is unambiguous.
pub struct EngineCapability {
    pub source: SourceFmt,         // user-facing source format (§1.5 / 04)
    pub target: TargetFmt,         // user-facing target format
    pub direction: Direction,
}

// Type aliases (the user-facing format vocabulary is §0.6-owned):
//   SourceFmt = UserFacingFormat (§0.6);  TargetFmt = TargetId (§0.6);
//   EngineId  = the stable engine discriminant (§0.6 EngineDescriptor.id).
pub type SourceFmt = UserFacingFormat;     // §0.6
pub type TargetFmt = TargetId;             // §0.6
```

### 3.2.3 Selection algorithm `[DECIDED]`

Selection is a **static lookup, not a search** (because the `04` files have
pre-assigned exactly one owner per pair — there is nothing to "choose" at
runtime). The registry is built at startup into a `HashMap<(SourceFmt,
TargetFmt), EngineId>` keyed by the user-facing format pair, populated from each
engine's `capabilities(...)` filtered by the resolved §3.4 `PatentDisposition`
for the running platform.

```
fn select(src: SourceFmt, tgt: TargetFmt, plat: Platform) -> Option<EngineId>
    = registry.lookup((src, tgt))            // single owner, decided in 04
        .filter(|e| e.available_on(plat, patents))   // §3.4 may mark unavailable
```

- A pair returning `None` because §3.4 marked its codec **unavailable** on this
  platform is surfaced as **honestly unavailable** (SSOT *v1 DoD* exception 1) —
  the target tile is shown disabled-with-reason (UI §5), never silently dropped.
  This is the *only* legitimate source of `None`; an in-scope license-clean pair
  must always resolve.
- **No fallback engine chain.** There is intentionally no "if engine A fails try
  engine B" — a single owner per pair keeps results deterministic and identical
  across platforms (SSOT *Cross-platform, one product*). Engine *internal*
  fallbacks (poppler's GS backstop *within* `PDF→TXT`) are an
  implementation detail of one engine, not a registry-level alternate, and do not
  violate single-engine-per-pair.
- The few `04` `[OPEN]`s about *which* engine owns a pair (`MD→PDF` LO-vs-pandoc;
  `RTF→markup` pandoc-vs-LO; "standardise all HEIC/AVIF encode on vips `heifsave`
  vs standalone `heif`/`avif`") are **owned by their `04` files**, referenced here:
  whichever they resolve to, it remains a *single* registry owner — the trait and
  lookup are unaffected. They feed the README open-questions log via `04`.

---

## 3.3 Bundling model (all offline) `[DECIDED: bundle everything]`

**Everything ships inside the build; zero runtime fetch** (SSOT *Local/private/
offline*, *v1 DoD* offline floor). No engine, codec, font, or update is ever
downloaded after the app itself.

### 3.3.1 Two physical bundling mechanisms (Tauri v2)

| Mechanism | Used for | Tauri config | Resolved at runtime by |
|---|---|---|---|
| **`bundle.externalBin`** (sidecars, target-triple-suffixed) | FFmpeg, ffprobe, soffice launcher, pdftotext, pandoc — the **standalone invoked binaries** (Ghostscript **[DECIDED: dropped]**; **x265 is NOT a sidecar** — it ships as a dynamically-loaded libheif encoder *plugin* under `resources`, §3.1 row 1a) | `"bundle": { "externalBin": ["binaries/ffmpeg", "binaries/ffprobe", "binaries/soffice", "binaries/pdftotext", "binaries/pandoc"] }` | spawned by the Rust core (see 3.3.3) |
| **`bundle.resources`** (verbatim files/dirs) | the LibreOffice **program tree + profile template + bundled fonts**, the **image-worker stack** (libvips + libheif/libde265 + the **x265 libheif plugin** + libaom/dav1d + librsvg + cgif + the **required ImageMagick** delegate), FFmpeg/pandoc data files if any, the NOTICE/third-party-licenses text (§3.7) | `"bundle": { "resources": { "engines/libreoffice/": "engines/libreoffice/", "engines/image/": "engines/image/", "fonts/": "fonts/", "THIRD-PARTY-LICENSES.txt": "" } }` | `app.path().resolve(rel, BaseDirectory::Resource)` |

> **Why LibreOffice is `resources`, not `externalBin`.** `externalBin` is for a
> single self-contained executable that gets the target-triple suffix; LibreOffice
> is a **directory tree** (the `soffice`/`soffice.bin` launcher plus `program/`,
> `share/`, type libraries, the bundled font dir). So the **tree ships as a
> `resources` dir** and the launcher inside it is invoked by absolute path
> resolved via `BaseDirectory::Resource`. The `externalBin` line for `soffice`
> above is only the thin launcher where a single-file form exists; on platforms
> where it isn't single-file, the launcher is reached purely through the resource
> tree. `[DEFER]` exact split (launcher-as-externalBin vs launcher-in-resources)
> to the §6.1 packaging step — both are offline and resolve via the PathResolver;
> it does not change any contract here.

### 3.3.2 Build-time assembly (cross-ref §6.1)

The build (§6.1 owns the CI matrix) performs, per platform:

1. Fetch/build the **pinned** engine versions (§3.8) for that platform's
   target-triple from a vendored/cached source — **build inputs are not fetched
   at app runtime; they are fetched at *build* time** (the offline guarantee is
   about the *shipped app*, not the CI machine).
2. Place each standalone engine at `src-tauri/binaries/<name>-<target-triple>[.exe]`
   (the `externalBin` naming convention; e.g. `ffmpeg-x86_64-pc-windows-msvc.exe`,
   `ffmpeg-aarch64-apple-darwin`, `ffmpeg-x86_64-unknown-linux-gnu`).
3. Place the LibreOffice tree + fonts under `src-tauri/engines/` /
   `src-tauri/fonts/` (the `resources` map).
4. Emit the per-build **SBOM + NOTICE** (§3.7) and the patent-disposition record
   (§3.4) as bundled resources.
5. `tauri build` packages everything into the one artifact per platform (§6.1).

### 3.3.3 Runtime invocation path `[DECIDED — Rust-core spawn, not WebView shell]`

**Decision: engines are spawned by the Rust core via `std::process` /
`tokio::process`, resolving the bundled binary path through Tauri's
`PathResolver`, and the WebView is granted NO `shell:allow-execute` permission.**

Rationale (this materially shapes §0.10 and §1.7):

- ConvertIA needs **fine-grained subprocess control** the WebView-facing shell
  plugin does not cleanly give: **process-group creation + group-kill** for
  cancellation (§1.7), **stdin piping** (pandoc reads from stdin in some paths),
  custom **cwd/env** for the §2.14 scratch volume and the §2.12 isolation
  sandbox, FFmpeg's `-progress pipe:` line parsing (§1.11), and the
  catch-and-classify of `stderr` (§2.8/§2.13). All of this lives naturally in the
  Rust orchestrator (§0.7), which already owns the queue and the guarantees.
- **Security:** keeping `shell:allow-execute` *off the WebView capability set*
  (§0.10) means a compromised/buggy front-end **cannot ask the shell plugin to
  run an arbitrary sidecar with arbitrary args** — the only way to start an engine
  is through a typed IPC command (§0.4) that the Rust core validates against the
  registry and the frozen job. This is the tighter half of the §0.11 threat map
  (the §3.5 sidecar invocation is *not* WebView-reachable).
- **Concrete path resolution `[DECIDED]`.** When the Rust core spawns via
  `tokio::process`, it resolves the bundled program path as follows (so Phase 3 does
  not rediscover it):
  - **externalBin sidecars** (ffmpeg, ffprobe, pdftotext, pandoc, the soffice
    launcher where single-file): the triple-suffixed binary sits beside the app
    executable, resolved via **`std::env::current_exe()`** + the target-triple suffix
    Tauri stages (`<name>-<target-triple>[.exe]`), or equivalently
    `app.path().resolve("<name>", BaseDirectory::Resource)` where the bundler places
    them in the resource dir. Either resolves to an **absolute path**; `PATH` is never
    relied on (§3.5 env note).
  - **resources-tree binaries** (the LibreOffice `program/soffice.bin`):
    **`app.path().resolve("engines/libreoffice/program/soffice", BaseDirectory::Resource)`**
    — an absolute path inside the bundled resource tree.
  This is the `EngineProgram::{Sidecar, ResourceBin}` distinction in §3.2's
  `Invocation`. The externalBin/resources placement (§3.3.1/§3.3.2) guarantees the
  file exists beside the app (portable, no install — SSOT *Portable, no installation*).
- `[DECIDED]` ConvertIA does **not** depend on the Tauri **shell plugin** for
  engine execution at all — engines run only from Rust (`tokio::process`). §7.7's
  open-folder/open-file/open-url uses the separate **`opener` plugin**, which is
  unrelated to `shell:allow-execute`. Net: **no WebView command may execute an
  engine**; there is **no `shell:allow-execute` on the §0.10 allowlist** (the prior
  draft's grant was removed — see §0.10). This closes the §0.10/§3.3.3 [OPEN] by
  cross-reference: the answer is **no WebView shell grant**.

### 3.3.4 Offline invariant (cross-ref §2.11)

Every engine above is self-contained once bundled. The only network code paths in
the entire app are the **user-initiated** §7.7 open-project-page shell-out;
**there is no engine that fetches anything** — explicitly: libvips' SVG loader
does **not** fetch remote `href`/`<image>` (offline + §2.12), pandoc does **not**
fetch remote images (`documents.md` MD/HTML notes), LibreOffice HTML import does
**not** fetch remote CSS/img. §2.11 owns the observable "no network" property;
this section guarantees the *supply* side (nothing to fetch).

---

## 3.4 Per-platform packaging & the patent-disposition matrix `[DECIDED per cell below]`

This section is the **single owner** of the HEIC/AAC/H.264 patent decision the
SSOT mandates. `images.md`, `audio.md`, `video.md`, `cross-category.md` and §6.5
all **reference this matrix and never re-decide**. Honest per-platform
availability (SSOT *v1 DoD* exception 1) flows from here.

### 3.4.1 The four dispositions (definitions)

| Disposition | Meaning | Offline? | Isolation? |
|---|---|---|---|
| **ship-bundled** | ConvertIA bundles the encoder/decoder inside the build and runs it itself | yes (bundled) | §2.12 (our subprocess) |
| **rely-on-OS** | ConvertIA does **not** bundle the codec; it asks the OS/system framework to do that codec step (e.g. macOS VideoToolbox/ImageIO, Windows Media Foundation/HEVC extension) | yes **only if the OS component is already present** — ConvertIA never downloads it (offline floor); if absent → behaves as *unavailable* | partly OS-controlled — **a distinct, weaker isolation story than §2.12** (the decode runs in/through an OS service, not our sandboxed subprocess); the §2.12 boundary still wraps *our* call but cannot contain an OS framework crash the same way — **noted as a real trade-off** |
| **gate** | The codec direction is **off by default** and only enabled by an explicit, documented user action/flag (not v1's "just works" model) | n/a | n/a |
| **unavailable** | The codec direction is honestly not offered on this platform; the target tile is disabled-with-reason (SSOT exception 1) | n/a | n/a |

> **"rely-on-OS" is a real, distinct strategy with costs.** It breaks the SSOT
> *Cross-platform, one product* "identical result" ideal (output quality differs
> by OS encoder), it makes availability depend on an OS component ConvertIA may
> not download (so it can silently degrade to *unavailable* on a stripped OS), and
> its isolation is OS-mediated rather than our §2.12 sandbox. It is therefore a
> **last-resort** disposition, chosen only where bundling is legally untenable.

### 3.4.2 The patent landscape (grounding the decisions; sources at foot)

- **MP3** — patents **expired 2017**; royalty-free. *(audio.md already treats it
  so.)* Not in this matrix.
- **AAC** — still administered by a patent pool (Via LA, reorg'd late 2025).
  Crucially: there are **no licence fees for *distributing AAC bitstreams*** — the
  royalties target *encoder/decoder distribution*. The honest grey area: the Via LA
  AAC programme **does nominally levy a per-unit royalty on distributing AAC
  encoder/decoder implementations by "manufacturers"** (a free / low-volume tier
  exists — the SSOT flags this as grey, not clearly-zero). ConvertIA bundles FFmpeg's
  **native, license-clean LGPL AAC encoder** (not libfdk_aac), so there is **no
  licence-compatibility problem**; the residual exposure is the **manufacturer
  encoder/decoder-distribution patent leg**, not bitstream distribution. **`[DECIDED]`
  ship bundled** (no revenue, hobby/non-commercial, free/low-volume tier; many
  open-source distros ship FFmpeg's native AAC freely) and **surface AAC in the
  NOTICE** (§3.7). The decision is technical/redistributability — legal-advice items
  are out of scope for this spec.
- **H.264 / AVC** — patent pool (MPEG LA / now Via LA); **last US patent expires
  ~2027-11** (patent-term-adjusted). So H.264 is *months* from expiry at the v1
  horizon but **not yet free**. Same shape as AAC: the *engine* (x264/FFmpeg) is
  license-clean; the residual risk is a patent claim on distributing an H.264
  encoder.
- **HEVC / H.265** (used by HEIC) — the **most encumbered**: multiple active pools
  (Access Advance, Via LA), a Jan-2026 rate increase, **27,000+ patents**, full
  protection well beyond 2027. The HEVC **encoder** (x265) is **GPL** *and*
  patent-heavy; this is the one codec where distribution risk is materially higher.
- **AV1 (AVIF)** — **royalty-free** (AOMedia patent grant); libavif/aom/dav1d are
  BSD. No patent royalty; the §3.4 entry exists only to record the **build/ship
  posture** (it ships everywhere).
- **VP9 / Opus / Vorbis / FLAC / ALAC / PCM** — royalty-free; not in this matrix.

### 3.4.3 The matrix — recommended disposition per (codec × platform)

Rows = the patent-encumbered codec *as used by a ConvertIA format*; columns =
platform. Each cell is the **recommended** disposition (read with §3.4.4).

| Codec / use | Affects (04 formats) | **Windows** | **macOS** | **Linux** |
|---|---|---|---|---|
| **AAC** (encode+decode) | `audio.md` AAC, M4A targets; `cross-category` M4A extract; `video.md` MP4/MOV/M4V audio | **ship-bundled** | **ship-bundled** | **ship-bundled** |
| **H.264 / AVC** (encode; decode) | `video.md` MP4/MOV/MKV/M4V re-encode (the **default video target**) | **ship-bundled** | **ship-bundled** | **ship-bundled** |
| **HEVC / H.265 — DECODE** (read HEIC; read iPhone HEVC `.mov`) | `images.md` HEIC source; `video.md` MOV/MKV HEVC source | **ship-bundled** (libde265, LGPL, decode-only) | **ship-bundled** (libde265) | **ship-bundled** (libde265) |
| **HEVC / H.265 — ENCODE** (write HEIC) | `images.md` HEIC **target** (never a default) | **ship-bundled (x265, isolated) `[DECIDED]`, behind §3.4 availability flag** | **ship-bundled (x265, isolated) `[DECIDED]`, behind flag** | **ship-bundled (x265, isolated) `[DECIDED]`, behind flag** |
| **AV1 (AVIF)** encode+decode | `images.md` AVIF | **ship-bundled** | **ship-bundled** | **ship-bundled** |

### 3.4.4 Rationale + what is genuinely still OPEN

**AAC — ship-bundled everywhere `[DECIDED]`.** FFmpeg's native AAC is
LGPL and license-clean; the AAC patent exposure is a distribution-royalty question
that the broad open-source ecosystem (Linux distros, countless OSS apps) treats as
acceptable for a free, non-commercial, no-revenue project. ConvertIA is exactly
that (SSOT *MIT freeware*, *hobby/no revenue*). **rely-on-OS is rejected** because
AAC must work *identically* on all three platforms — M4A and the MP4 audio track
depend on it, and the SSOT one-product promise forbids a platform-specific AAC
gap. **Consequence honored:** the NOTICE/About surfaces the patent posture (§3.7).

**H.264 — ship-bundled everywhere `[DECIDED]`, with the same
posture as AAC.** This is **load-bearing**: `video.md` makes **MP4 (H.264+AAC) the
default target of *every* video source**, and §video.md flags that "a platform
without H.264/AAC encode would have no default target — a product problem." So the
matrix **must** put H.264 encode at ship-bundled on all three platforms, and it
does. x264 is GPL → isolated as an invoked binary inside the FFmpeg sidecar (§3.6).
The ~2027 expiry further de-risks this over v1's (deadline-free) lifetime.

**HEVC decode — ship-bundled everywhere `[DECIDED]`.** Decoding HEIC
("open my iPhone photo") and reading HEVC-in-MOV are core everyday needs.
**libde265** is LGPL and decode-only (no x265, no GPL-encode patent-heavy path);
decode-only HEVC has the lighter patent profile and is widely shipped. This is the
`images.md` `HEIC→JPG★` default-source path and must work everywhere.

**HEVC *encode* (writing HEIC) — `[DECIDED]`: ship-bundled-isolated (x265), behind
the §3.4 availability flag.** This is the highest-risk codec: x265 is **both GPL and
the most patent-encumbered** codec in the set, and HEIC-as-a-target is **never a
default** (`images.md`: "never a default… compatibility-poor on non-Apple"). The
decision (adopting the standing [REC]):
- **Ship-bundled x265 on all three platforms `[DECIDED]`** — so HEIC *output* exists
  everywhere, **isolated as a separately-invoked binary** per §3.6 (GPL never linked
  into the MIT core; only redistributable code ships), patent posture surfaced in
  NOTICE. Rationale: the **same OSS-acceptable posture as AAC/H.264** applies, and
  consistency with the one-product promise. The codec is **redistributable** (GPL,
  aggregation) so it meets the "ship only what is redistributable" constraint.
- **Build it behind the registry's §3.4 availability flag** so flipping HEIC-encode
  to **`unavailable`** on any/all platforms is a **config change, not a code change**
  — preserving the SSOT exception-1 escape hatch (a never-default, low-demand,
  highest-risk target can be dropped cleanly if the owner later reconsiders the
  patent posture, at zero cost to the default path since no source defaults *to* HEIC).
- **License-clean alternative recorded:** **kvazaar (BSD)** removes the GPL half
  entirely (patent exposure unchanged); if the GPL surface is ever the deciding
  factor, swap x265→kvazaar without changing the disposition.
  - A further option, **rely-on-OS for HEIC encode on macOS only** (ImageIO writes
    HEIC natively, no x265), would give Apple users native HEIC output while
    keeping x265 off macOS — but it reintroduces per-platform divergence and the
    weaker isolation; **not recommended** as the primary, recorded as a known
    alternative.
  - A fourth option, **kvazaar (BSD-licensed HEVC encoder)** instead of x265,
    **removes the GPL/licence half of the concern entirely** (BSD links cleanly, no
    aggregation needed) — the **patent** exposure is unchanged (HEVC patents apply
    regardless of which encoder produces the bitstream). Recorded so the owner
    decision is complete: if HEIC-encode ships, kvazaar is the **licence-clean**
    encoder choice (quality is lower than x265 but adequate for a never-default
    target); the remaining call is purely the patent posture, identical to the AAC/
    H.264 reasoning above.
- **Decision `[DECIDED]`: ship-bundled-isolated, behind the flag** (above). HEVC
  *decode* is settled (ship-bundled, libde265); HEIC *encode* is **built behind the
  registry's §3.4 availability flag** so flipping it to `unavailable` remains a
  config change, not a code change — the escape hatch is preserved without leaving
  the disposition itself open.

**AVIF — ship-bundled everywhere `[DECIDED]`.** Royalty-free; no real decision —
the row exists only to record that AV1 ships on all platforms with no gate.

> **Isolation constraint on any future `rely-on-OS` disposition `[DECIDED]`.** A
> `rely-on-OS` codec step runs untrusted bytes through an **OS framework**, not
> ConvertIA's §2.12-sandboxed subprocess — a **weaker isolation tier** for the T1
> threat (§0.11). So **any** future switch of an untrusted-decode path to `rely-on-OS`
> must be **re-evaluated against §0.11 T1 / §2.12** before adoption (it trades the
> uniform process-boundary isolation for an OS-mediated one). v1 ships **no**
> `rely-on-OS` decode path; this records the gate so a later change cannot slip the
> isolation regression in silently.

### 3.4.5 Per-platform packaging specifics (beyond the matrix)

| Aspect | Windows | macOS | Linux |
|---|---|---|---|
| Artifact | portable `.exe` (no installer) (§6.1) | `.app` (and/or `.dmg`) | AppImage / portable dir |
| Target triples | `x86_64-pc-windows-msvc` (+ `aarch64` `[DEFER]`) | `aarch64-apple-darwin` + `x86_64-apple-darwin` (universal `[DEFER §6.1]`) | `x86_64-unknown-linux-gnu` (musl `[DEFER]`) |
| WebView runtime | WebView2 (system; **bundle-vs-rely** is §0.3.1, **never download** per offline floor) | WKWebView (system) | WebKitGTK (system; distro drift → §0.3.1) |
| Engine exe extension | `.exe` suffix on every sidecar | none | none; **executable bit must be set on extraction** (§7.2) |
| LibreOffice tree | `program\soffice.bin` + `share\` | `LibreOffice.app` contents inside our resources | `program/soffice.bin` + `share/` |
| Notable | x264/x265/ffmpeg `.exe` are `externalBin` triple-suffixed | code-signing/notarization **out of scope** (SSOT) — unsigned `.app`; the integrity-hash trust substitute (§6.2) applies | AppImage must carry glibc-compatible engine builds (or musl `[DEFER]`) |

The **supported-OS floor** per platform is **`[DECIDED]` in §0.3.1** (Win10 1809+/11;
macOS 11+; Ubuntu-22.04-LTS-class WebKitGTK; exact build numbers `[DEFER: §6.4]`).
§3.4 only notes that *engine* binaries pin their own min-OS (e.g. a macOS FFmpeg
built for the chosen min `MACOSX_DEPLOYMENT_TARGET`) and that floor must not exceed
§0.3.1's.

---

## 3.5 Per-engine argument construction (concrete)

**Scope:** only the per-engine concretes — argv construction, cwd, env, the
progress-signal format, stdin, and exit-code/`stderr` quirks. The **generic
invocation lifecycle** (spawn → progress → cancel → timeout → error-map) is owned
by **§1.7**; **every** invocation here routes through the **§2.12 isolation
wrapper** and writes only to the **§2.14 per-run scratch** then is atomic-renamed
by **§2.1**. Output paths below are always the temp path `out_tmp`, never the
final user path.

**Shared invocation conventions (all engines).**
- **cwd** = the per-run scratch dir (§2.14); engines that emit beside their input
  (LibreOffice `--outdir`) are pointed at scratch.
- **env** = a **minimal, isolated environment** (§2.12): no inherited user env
  beyond what the engine needs; `LC_ALL=C.UTF-8`/`LANG` set for deterministic
  text handling; `HOME`/profile redirected (LibreOffice) into per-run scratch;
  no proxy vars (offline). **`PATH` is *not* relied on** — every program is an
  absolute resolved bundled path (§3.3.3). The minimal env **explicitly STRIPS the
  dynamic-loader injection variables** so a hostile input cannot coerce a side-load:
  `LD_PRELOAD`, `LD_LIBRARY_PATH` (Linux), `DYLD_INSERT_LIBRARIES`, `DYLD_LIBRARY_PATH`
  (macOS) are cleared; the engine resolves only the bundled shared libs shipped beside
  it (§3.6.1 / §3.9.1).
- **timeout/hang** parameters: mechanism §1.7; per-engine *values* tuned in §3.8
  against the corpus (LibreOffice cold-start is slow → a longer first-spawn grace).
- **cancellation**: process-group kill (§1.7) — relevant because LibreOffice and
  FFmpeg may spawn children.

### 3.5.0 macOS TCC source staging — the core touches protected paths, not engines `[DECIDED]`

> Requirement owner: **§7.2.6** (macOS TCC). This subsection owns the *engine-arg /
> handle plumbing* §7.2.6 mandates.

On **macOS**, the source file the user dropped frequently sits in a TCC-protected
location (Desktop / Documents / Downloads / a removable volume). To guarantee a
spawned engine never has to be the process that *first* touches such a path (which a
TCC responsible-process chain-break could deny — §7.2.6), the Rust core **stages the
source into per-job scratch before spawning**:

1. The **core** (which holds the TCC grant, having been the process that read the
   protected path during §1.1 freeze / detection) **copies the source** into a
   per-job **kind-2 scratch path** (§2.14.2) under the app-owned scratch root.
2. The engine is handed the **scratch path** as its input argument — never the raw
   protected user path:
   - **FFmpeg / poppler / LibreOffice**: the scratch source path is the `<input>`
     argument (LibreOffice `--outdir` already points at scratch).
   - **pandoc**: where applicable, bytes are piped on stdin (`StdinPlan::PipeBytes`,
     §3.2) so no path is opened by the child at all; otherwise the scratch path.
   - **libvips (in-process / image-worker)**: loads from the scratch path.
3. The output is written to `out_tmp` and published per §2.1; the staged scratch
   source is reclaimed with the run (§2.6).

**Scope:** this staging is **macOS-only** (Windows/Linux have no TCC; ordinary ACL
denials there map to the §2.8 `Unreadable` kind and need no staging). It composes
with the §2.14 cross-volume strategy (the staged copy already lives on the scratch
volume) and the §2.12 isolation wrapper. Cost: one extra source-sized copy per
macOS item — acceptable, and the same copy the cross-volume path would make anyway.

### 3.5.1 FFmpeg / ffprobe (audio, video, cross-category)

- **Probe first (video only):** `ffprobe -v error -print_format json -show_streams
  -show_format <input>` → inner codecs / duration / rotation / interlace. The
  **duration** becomes the §1.11 progress denominator; the **inner codecs** drive
  `video.md`'s remux-vs-reencode decision (a §3.2 capability decision, executed
  here).
- **Progress:** `-progress pipe:1 -nostats` → key=value lines (`out_time_us=…`,
  `total_size=…`, `progress=continue|end`) parsed into `ProgressModel::
  FfmpegKeyValue` (§1.11). Real per-item %, never a spinner.
- **Global flags (all FFmpeg jobs):** `-nostdin -hide_banner -loglevel error -y`
  — `-y` is safe because the target is the **temp** path (§2.1), never the user
  file; `-nostdin` prevents the classic FFmpeg "consumes the parent's stdin" hang.
- **Audio (`audio.md`):** decode → encoder per that file's table, e.g.
  - MP3 `-c:a libmp3lame -q:a 2` (VBR default) / `-b:a Nk` (CBR presets);
  - AAC `-c:a aac -b:a 192k` + muxer `adts` (raw `.aac`) or `ipod` (`.m4a`,
    `-movflags +faststart`);
  - FLAC `-c:a flac -compression_level 5`; WAV `-c:a pcm_s16le`; AIFF
    `-c:a pcm_s16be`; OGG `-c:a libvorbis -q:a 3`; OPUS `-c:a libopus -b:a 128k`;
    ALAC `-c:a alac` + `ipod`. `-map_metadata 0` for tag carry (audio.md policy).
  - **Cover-art passthrough:** mechanism differs by container.
    - **MP3 / M4A / FLAC** store cover art as an **attached-picture *video* stream** →
      add `-map 0:v? -c:v copy` (the `?` makes the attached-picture stream optional so
      audio-without-art still works).
    - **OGG / OPUS** do **NOT** carry a video stream; cover art is a **FLAC PICTURE
      metadata block** (`METADATA_BLOCK_PICTURE` Vorbis comment), so `-map 0:v? -c:v
      copy` would drop or error. Cover-art carry for OGG/OPUS is a **metadata copy**
      (`-map_metadata 0` carries the comment block where ffmpeg supports it), **not** a
      video-stream copy. **`[DEFER: corpus]`** verify OGG/OPUS picture round-trips on
      the §6.4 corpus; if it proves unreliable, annotate OGG/OPUS with
      `audio_tags_dropped` (§2.9) and remove them from the "supports embedded picture"
      list (update audio.md accordingly).
    - **Raw ADTS `.aac` and WAV/AIFF omit it** (no picture support → the
      `audio_tags_dropped` §2.9 note fires, audio.md tag policy).
  - **Channel handling:** preserve source channels by default (audio.md). For
    **>2-channel sources → MP3/OGG** (whose everyday encoders are best at stereo) add
    `-ac 2` downmix and fire the §2.9 `audio_downmix` note; **AAC/M4A/OPUS/FLAC/WAV**
    preserve the source channel layout (no forced downmix).
- **Video (`video.md`):** **remux** = `-c copy` (+ `-movflags +faststart` for
  MP4/MOV/M4V, `-fflags +genpts` for FLV, `mov_text` subtitle convert for MKV→MP4
  text subs); **re-encode** = `-c:v libx264 -crf 23 -preset medium -pix_fmt
  yuv420p` (H.264 family) or `-c:v libvpx-vp9 -b:v 0 -crf 32 -row-mt 1` (WEBM) +
  `-c:a aac -b:a 128k` / `-c:a libopus -b:a 96k`; `yadif` deinterlace when flagged
  (`video.md` `[OPEN]`); rotation honored. **Mixed remux/re-encode in one
  invocation** (video may copy while audio transcodes) — still one process (§3.2).
- **Cross-category (`cross-category.md`):** extract-audio = `-vn -map 0:a:0
  -c:a copy|<encoder>` (copy when codec matches container, else least-lossy
  re-encode); to-GIF = the single-process `split`/`palettegen`/`paletteuse`
  filtergraph from `cross-category.md` (`-vf "fps=12,scale=480:-1:flags=lanczos,
  split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:
  bayer_scale=5" -loop 0`) with the §1.10 duration cap applied as `-t`. **No temp
  PNG** (no chaining — §3.2).
- **Exit/stderr quirks:** non-zero exit → `classify_failure` maps known stderr
  patterns to §2.8 kinds: "could not find codec parameters"/"Invalid data" →
  corrupt; "No audio" path → the `cross-category` *named* "no audio track" kind;
  DRM/"Operation not permitted" on FairPlay/WMV → the §video.md "copy-protected"
  message; everything else → generic engine-failure (still plain-language, §2.13).
- **Licence/isolation:** the FFmpeg binary is **GPL-2.0+** (it enables `libx264`,
  `--enable-gpl`; no `--enable-nonfree`) — the whole binary is the aggregation case,
  shipped as a separate invoked binary (§3.6.1), written-offer-of-source honored.
  Untrusted A/V parsed in FFmpeg demuxers/decoders = classic attack surface → always
  inside §2.12.

### 3.5.2 LibreOffice headless (documents, spreadsheets, presentations)

- **Shape:** `soffice --headless --norestore --nolockcheck --nodefault
  --nofirststartwizard -env:UserInstallation=file://<per-run-profile>
  --convert-to <ext>:<FilterName>[:<FilterData-JSON>] --outdir <scratch> <input>`.
- **One document per invocation, serialized.** LibreOffice headless is **NOT
  safely parallel under one profile** (§0.9 owns the concurrency degree and the
  *serialize-LibreOffice* rule — parallel instances on one profile lock/corrupt).
  This section honors it by giving **each invocation its own disposable
  `-env:UserInstallation` profile** in per-run scratch (§2.14), and the queue
  serializes LO jobs per §0.9. The profile is torn down with the run (§2.6).
- **Filter names (from the `04` files, fixed here):**
  - `*→PDF`: `writer_pdf_Export` (Writer sources), `calc_pdf_Export` (Calc),
    `impress_pdf_Export` (Impress);
  - office↔office: `MS Word 2007 XML` / `MS Word 97` / `writer8` (ODT) /
    `Rich Text Format`; `Calc MS Excel 2007 XML` / `MS Excel 97` / `calc8` (ODS);
    `Impress MS PowerPoint 2007 XML` / `MS PowerPoint 97` / `impress8` (ODP);
  - CSV/TSV: `Text - txt - csv (StarCalc)` with the **`FilterOptions` token
    string** carrying the sniffed delimiter + encoding + values-not-formulas
    (spreadsheets.md owns the token semantics; this file owns assembling the
    comma-separated token string, e.g. field-sep=44/9, text-delim=34, charset
    token, "save cell contents as shown"=true / "export formulae"=false);
  - DOC→markup (the LO-owned down-conversions, documents.md `[OPEN-2]`): `Text` /
    `HTML (StarWriter)` / `Markdown`† (LO 26.2).
- **FilterData JSON** (PDF export options, e.g. `ExportNotesPages`,
  `SelectPdfVersion`, `UseTaggedPDF`, `Quality`) is passed inline:
  `pdf:impress_pdf_Export:{"ExportNotesPages":{"type":"boolean","value":"true"}}`
  — values + defaults owned by the `04` files; this file owns the JSON wire form.
- **Progress:** `ProgressModel::CoarseSpawnDone` — headless LO gives **no
  fine-grained progress**; §1.11 shows a determinate-but-coarse bar (spawn→running
  →done), acceptable because office conversions are usually short relative to a
  long video re-encode.
- **Output discovery `[DECIDED]`:** LibreOffice writes `<basename>.<ext>` into
  `--outdir`, but it can **normalise or truncate** the basename (illegal chars,
  length, charset folding), so the core must **not** string-match the source
  basename to find the result. Instead, each LO job is given a **unique, empty,
  per-job `--outdir`** under the per-run scratch (§2.14 kind-2), and discovery is by
  **snapshot-diff**: list the (empty) outdir before spawn, list it after a verified
  success, and pick **the single new `*.<ext>` file** that appeared. (A unique outdir
  per job guarantees exactly one new file, so the diff is unambiguous even under LO
  basename mangling and even with LO serialized per §0.9.) That discovered file is
  then atomic-published to the planned final name (§2.1) — LO's own output naming is
  **never** the user-facing name. If **zero** new files appear despite exit 0, that
  is the LO "exit 0 but wrote nothing" failure → mapped per the exit/stderr rule
  below (e.g. `password-protected` / `EngineError`).
- **Exit/stderr quirks:** LO headless famously returns **exit 0 even on some
  failures** and writes nothing → **success is verified by the expected output
  file existing and being non-empty in `--outdir`**, not by exit code alone (a
  critical correctness rule). Encrypted/password files → no output → mapped to the
  §2.8 "password-protected" kind (documents/spreadsheets/presentations all rely on
  this). A stale soffice lock from a crashed prior run is avoided by the per-run
  profile + `--nolockcheck`.
- **Licence/isolation:** MPL-2.0 sidecar (§3.6); untrusted office files (zip-bomb,
  malformed OOXML, macro-bearing) parsed inside §2.12; **macros never executed**
  (headless + the `04` "macros dropped" policy).

### 3.5.3 poppler `pdftotext` (PDF→TXT)

- **Shape:** `pdftotext -enc UTF-8 -eol unix <input> <out_tmp.txt>` (layout via
  default reading order; `-layout` **not** used by default — plain reading order
  is the everyday "get the words out"; documents.md owns the lossy note).
- **Encryption:** `pdftotext` exits non-zero / emits "Command Line Error:
  Incorrect password" on encrypted PDFs with no user password → mapped to the
  §2.8 "password-protected" kind (documents.md edge case). **No password is ever
  prompted or cracked** (SSOT/scope).
- **Empty extraction** (scanned/image PDF) → a valid-but-near-empty `.txt`; this
  is reported honestly (documents.md: not surfaced as misleading success), not an
  error.
- **No Ghostscript backstop `[DECIDED]`:** GS is **dropped in v1** (§3.1) — the
  default `PDF→TXT` path is **poppler-only** with a clean fail-clearly (§2.8) on the
  unrecoverable minority. (A GS repair step would have been a two-step chain §3.2
  forbids for a user pair anyway; it could only ever have been an internal aid.
  `[DEFER: re-add only if the §6.5 corpus shows poppler failing PDFs GS would salvage]`.)
- **Progress:** `CoarseSpawnDone`. **Licence:** GPL → invoked binary (§3.6).

### 3.5.4 pandoc (markup conversions)

- **Shape:** `pandoc -f <in-fmt> -t <out-fmt> [opts] -o <out_tmp> <input>` (or
  `… < input` via **stdin** where the path has awkward characters — pandoc reads
  stdin cleanly; `StdinPlan::PipeBytes`).
- **Concrete opts (from `documents.md`):** `--wrap=preserve`; `*→HTML`
  `--standalone --embed-resources` (self-contained single file, images inlined);
  `MD` read dialect `-f gfm`; `*→MD` `-t gfm`; `[OPEN]` image policy
  (drop-with-note vs data-URI) owned by documents.md.
- **Reader limits honored (not re-decided):** pandoc **cannot** read legacy binary
  `.doc` and has gaps reading RTF → those down-conversions are **not** assigned to
  pandoc (documents.md reassigns them to LibreOffice); the registry (§3.2) never
  hands pandoc a `.doc`.
- **Network:** pandoc must **not** fetch remote images/CSS — runs with no network
  reachable (offline); remote refs become broken refs with a note (documents.md).
- **Progress:** `CoarseSpawnDone`. **Exit/stderr:** non-zero + message → §2.8
  generic; a "pandoc: …: openBinaryFile … does not exist" never occurs because the
  core verifies the input before spawn. **Licence:** GPL → invoked binary (§3.6).

### 3.5.5 libvips (images) — linked inside the image-worker process, not a CLI exe

- **Invocation `[DECIDED]`:** libvips is **linked inside a separate short-lived
  image-worker process** (§0.7/§2.12, the resolved placement), called via its Rust
  binding on a decode/encode worker thread *within that process*, not via argv — but
  it still produces an `Invocation`-equivalent plan (operation + params + `out_tmp`) so
  §1.7's lifecycle and §2.12's isolation wrap it uniformly. (libvips' streaming model
  keeps even huge rasters in bounded memory; the pathological-size guard from
  `images.md` feeds §1.10.)
- **Operation map (from `images.md`):** load (by detected type, **not** extension)
  → optional auto-rotate (EXIF orientation baked, tag reset to 1) → optional
  alpha-flatten (white bg for JPG/BMP) → save with the per-target saver and its
  params: `jpegsave Q=82 …`, `pngsave compression=6`, `webpsave Q=80`,
  `tiffsave compression=deflate`, **`gifsave` (native cgif backend, vips ≥ 8.12)**,
  **`magicksave` for BMP and ICO save (REQUIRED — libvips has no native `bmpsave` and
  no native ICO saver; both go through the ImageMagick delegate)**, and `magickload`
  for BMP load; ImageMagick is *also* a GIF fallback only (§3.6.1: ImageMagick is
  permissive, cgif is MIT),
  `heifsave compression=hevc Q=…` (HEIC, via the **x265 libheif plugin**) /
  `heifsave compression=av1 Q=…` (AVIF, via **libaom** — the single-engine
  `HEIC↔AVIF` path; **all** HEIC/AVIF *encode* is `heifsave`, no standalone
  `heif`/`avif` encoder), ICO multi-size list. ICC/metadata carried per `images.md`
  policy.
- **Progress:** `ProgressModel::VipsCallback` (libvips eval-progress signal) → a
  real % for large images.
- **Isolation `[DECIDED]`:** image decode/encode runs in a **separate short-lived
  image-worker process** (not an in-app thread), so a libvips/libheif/libde265/librsvg/
  codec crash, hang, **or memory-corruption exploit** is contained by the OS process
  boundary and fails **that one item** (§2.8) without wedging the app — exactly the
  §2.12.1 boundary every other engine gets, matching the §0.3 "separate engine
  subprocesses" model. (The earlier in-process-vs-worker `[OPEN]` is resolved to the
  worker; the §3.6 *licence* analysis is unaffected — libvips is LGPL either way; the
  worker links it internally, which is aggregation, not a link into the MIT core. Do
  **not** rely on a §2.13 `catch_unwind` boundary for this — that catches Rust panics,
  not hostile native code; §2.12.4.)
- **Licence/isolation of components:** libvips/libheif/libde265/librsvg = LGPL
  (link OK, dynamic); aom/dav1d = BSD; **ImageMagick = permissive
  (ImageMagick License, Apache-2.0-style — link-OK, NOT GPL) and REQUIRED (BMP+ICO
  save go only through it; §3.1 row 1d).** The **only** GPL piece in the image stack
  is **x265** (HEVC encode), the aggregation case (§3.6) — shipped as a
  **dynamically-loaded libheif encoder plugin** (`ENABLE_PLUGIN_LOADING`), never
  statically linked into the image-worker's libvips or the MIT core (see §3.6 for the
  exact line). (Build caveat: exclude any GPL ImageMagick *optional delegates*; IM
  core is permissive.)

### 3.5.6 Native CSV/TSV engine (in-core Rust)

- No subprocess; a single streamed pass: detect encoding/delimiter (spreadsheets.md
  policy) → re-encode to UTF-8 (no BOM default) → swap delimiter → RFC-4180
  re-quote where a field contains the new delimiter/quote/newline → write to
  `out_tmp`. CSV-injection-safe (leading `= + - @` stay literal text). No progress
  model needed beyond byte-count (`CoarseSpawnDone`-class). MIT (own code) — no
  §3.6 concern.

---

## 3.6 Copyleft isolation `[DECIDED]`

**Policy (SSOT *Engine-license policy*):** ConvertIA's own code is **MIT**;
"compatible terms" **explicitly includes copyleft**; GPL/AGPL engines ship as
**separate, independently-invoked binaries — aggregation, not static linking into
the MIT core** — their obligations honored (licence text + written offer of
source where required), so the MIT core stays clean.

### 3.6.1 The aggregation boundary — what links vs what is invoked

| Engine/component | Licence | Linked into MIT core? | Mechanism that keeps MIT clean |
|---|---|---|---|
| ConvertIA orchestrator + native CSV/TSV | MIT | — | it *is* the core |
| **libvips** (+ libheif, libde265, librsvg) | **LGPL-2.1/3.0** | **dynamic link only** (LGPL permits dynamic linking from non-GPL code, provided relinkability) — or run as the separate image-worker process (§3.5.5) | LGPL §6 dynamic-link allowance; we ship the LGPL libs + their source/offer (§3.7); **no static link** of LGPL into the MIT binary |
| **aom/dav1d** (BSD) | BSD-2 | link OK | BSD permissive |
| **libimagequant** (PNG/GIF palette quantisation) | **BSD-2-Clause** (the permissive leg of the libvips-vendored fork's GPL-vs-BSD dual licence; verify the shipped leg — if a GPL leg ever shipped it would move to the isolated-GPL rows below) | link OK (inside the image-worker) | BSD permissive; vendored/linked inside the image-worker process, not the MIT core |
| **ImageMagick** (GIF/BMP/ICO save delegate) | **ImageMagick License** (Apache-2.0-style, SPDX `ImageMagick`) — **permissive, NOT GPL** | link OK | Permissive like BSD/MPL — no isolation needed. **Build caveat:** exclude GPL *optional delegates*; IM core is permissive. (Listed in the SBOM/NOTICE §3.7.) |
| **x265** (HEVC encode) | **GPL-2.0-or-later** | **NO — dynamically-loaded libheif *plugin*** | x265 ships as a **separately-built, dynamically-loaded libheif encoder plugin** (`.so`/`.dll`/`.dylib`, libheif `ENABLE_PLUGIN_LOADING`) that `heifsave compression=hevc` loads at runtime. The GPL code is **never statically linked** into the image-worker's libvips or the MIT core; it lives behind libheif's plugin ABI and runs **inside the §0.7 image-worker process** (already a separate process from the core). *(A static x265-in-libvips link would taint — hence the plugin form. This replaces the dropped "standalone heif/x265 sidecar" — no such sidecar exists under the [OPEN-1] heifsave-only decision.)* |
| **x264** (H.264 encode) | **GPL-2.0** | **NO — inside the GPL FFmpeg binary** | reached only via the **FFmpeg binary** (separate invoked process); never linked into the MIT core |
| **FFmpeg** build | **GPL-2.0+** (enables GPL x264 via `--enable-gpl` → the *whole* binary is GPL-2.0+, not LGPL) | **NO — separate exe** | invoked as `ffmpeg`/`ffprobe` child processes (§3.3.3); aggregation keeps the MIT core clean. The LGPL component libs (libmp3lame etc.) are **dynamically linked beside the exe** (§3.9.1) per LGPL §6 — a static FFmpeg build would fail the §6.1.3 dynamic-link assertion. Written-offer-of-source obligation honored (§3.6.2). |
| **LibreOffice** | MPL-2.0 | **NO — separate sidecar** | invoked `soffice` process; MPL is weak/file-level anyway, but isolation is belt-and-suspenders + the SSOT policy |
| **poppler**, **pandoc** | GPL | **NO — separate exe** | invoked child processes |
| **Ghostscript** | **AGPL-3.0** | **NOT shipped v1 [DECIDED]** | dropped (§3.1) so no AGPL surface ships; `[DEFER: re-add only if §6.5 corpus shows GS-salvageable PDFs]` |

**LGPL dynamic-link build rule (a buildability gate, not just an assertion)
`[DECIDED]`.** The "libvips link stays LGPL-clean" claim rests on **dynamic
linking**, and Rust links **statically by default** — a vendored *static* libvips
would silently break LGPL §6. So it is a **build constraint**, enforced at build
time, that **libvips and every LGPL library it pulls in** (libheif, libde265,
librsvg, and any linked FFmpeg libs) **ship as bundled *shared* libraries and are
*dynamically* linked** (or supplied as relinkable object files), satisfying LGPL §6's
shared-library path. The CI bundle check (§6.1.3) asserts the LGPL libs are present
as shared objects (`.so`/`.dylib`/`.dll`) alongside the binary, not absorbed into a
static MIT executable. (The separate image-worker-process `[OPEN]` (§3.5.5), if
chosen, *also* resolves the static-link risk — the worker is a separate process, so
even a statically-linked LGPL inside it is aggregation, not a link into the MIT core
— but the shared-library rule is the primary, in-process-safe guarantee.)

**The one nuance to state plainly:** the GPL components are the **whole FFmpeg
binary** (GPL-2.0+ because it enables x264) and the *encoders* x264/x265, plus poppler
and pandoc. (Ghostscript/AGPL is **not shipped v1**.) **None are statically linked into
the MIT core.** x264 lives inside the GPL FFmpeg child process; x265 is a
dynamically-loaded libheif plugin inside the separate image-worker process, never a
static link inside libvips. Everything ConvertIA *links* into the MIT core or the
worker is MIT/LGPL/MPL/BSD/permissive — all of which permit linking from MIT (LGPL via
dynamic link; the image-worker is a separate process anyway). Each shipped GPL binary
carries its written-offer-of-source obligation (§3.6.2). This is the precise sense in
which "the MIT core stays clean."

### 3.6.2 Written-offer-of-source obligation `[DECIDED]`

For every GPL/LGPL/AGPL/MPL component shipped, ConvertIA satisfies the
"corresponding source" obligation by the **public-repo + canonical-release**
model (SSOT *Distribution & download trust*):
- The build is from **public source** (Ne-IA org). The **exact pinned source
  revision** of every bundled engine (§3.8) is recorded in the **SBOM** (§3.7)
  with its upstream URL/commit, which **is** a valid written offer / source
  pointer for OSS distribution.
- The bundled `THIRD-PARTY-LICENSES.txt` (§3.7) carries each engine's **full
  licence text** and a line stating where its corresponding source is obtained
  (upstream + the pinned ref). This is shipped *inside* the app (resource, §3.3.1)
  and shown in About (§5.9) — so the offer travels with every copy.
- AGPL: not applicable to v1 (Ghostscript dropped, §3.1). If GS is ever re-added
  (`[DEFER]`), the same model applies plus an explicit note; no network service exists
  so the AGPL §13 remote-interaction clause would not trigger.
- This obligation completeness is a **release-blocking gate** (§6.3/SSOT *v1 DoD*:
  "a missing attribution is release-blocking, same status as no-harm").

### 3.6.3 Trademark / name boundary (cross-ref, not owned here)

The MIT grant covers code, **not** the "ConvertIA" name or Ne-IA logo (SSOT
*Trademark*); §6.8 owns `TRADEMARK.md`. Noted here only so the §3.7 NOTICE does
not imply a trademark grant.

---

## 3.7 Licence surfacing — NOTICE / third-party-licenses + SBOM (data generation)

**This section OWNS the *generation* of the licence/attribution data + SBOM**
(source, format, build step). In-app **presentation** is owned by **§5.9 (About)**;
the in-repo policy files (`NOTICE`, `LICENSE`) are authored under **§6.8**; the CI
gate is **§6.3**. This section produces the *data* those consume.

### 3.7.1 What is generated

| Artifact | Content | Format | Where it lives |
|---|---|---|---|
| **SBOM** | Every bundled component: name, version, **pinned source ref/commit**, upstream URL, licence (SPDX id), supplier | **CycloneDX JSON** (recommended) `[recommended]` — broad tooling, easy to diff in CI | shipped as a release artifact (§6.2) **and** bundled as a resource |
| **THIRD-PARTY-LICENSES.txt** | Concatenated **full licence text** of every component + per-component "corresponding source: <url>@<ref>" line (§3.6.2) | plain UTF-8 text | bundled resource (§3.3.1); displayed verbatim in About (§5.9) |
| **NOTICE** (repo) | Top-level attribution + the collective copyright notice (SSOT: `Copyright (c) 2026 Ne-IA and ConvertIA contributors`) + pointer to THIRD-PARTY-LICENSES | text | repo root (authored §6.8 from this data) |

### 3.7.2 How it is generated (build step — cross-ref §6.3)

1. The engine inventory (§3.1) is the **single source list**: each engine's id,
   pinned version (§3.8), upstream URL, SPDX licence id, and `linked|invoked`
   class are declared in a **build manifest** (e.g. `engines.toml` in
   `src-tauri/`). This manifest is the authoritative input — **not** hand-curated
   prose, so it can't drift from what actually ships.
2. A build script (`cargo xtask sbom` / a Node build step, §6.1) reads the
   manifest + Rust crate licences (via `cargo about` / `cargo-cyclonedx`) +
   the bundled-engine entries → emits the **CycloneDX SBOM** and concatenates
   `THIRD-PARTY-LICENSES.txt` from each component's vendored `LICENSE`/`COPYING`.
3. The bundled fonts (§3.9) are **also** listed (their OFL/Apache licences).
4. **Every linked sub-component gets its own SBOM/`engines.lock` row**, not just the
   top-level engines — including the **FFmpeg binary** (SPDX `GPL-2.0-or-later`, with
   the written-offer-of-source line — it enables x264, §3.6.1), **ImageMagick**
   (SPDX `ImageMagick`, permissive, **REQUIRED** for BMP+ICO save), **cgif** (MIT, the
   native `gifsave` backend §3.5.5), the **x265 libheif plugin** (SPDX
   **`GPL-2.0-or-later`** — verify against the pinned source's `COPYING`; GPL-2.0-only
   would be incompatible with the LGPL-3.0 libheif host, whereas -or-later is
   upgradeable to GPLv3 (what Debian ships) — with offer-of-source), the **libheif
   AV1-encoder dependency `libaom`** (BSD-2 + patent grant), **dav1d** (BSD-2),
   **librsvg** (LGPL-2.1+ — the libvips `svgload` SVG backend), and
   **libimagequant** (the gifsave/cgif palette-quantisation dependency — SPDX
   **`BSD-2-Clause`**, the permissive leg of the libvips-vendored fork's GPL-vs-BSD
   dual licence; **NOT** BSD-3 — verify the shipped leg against the vendored `COPYRIGHT`,
   and if any GPL leg were shipped it would need the §3.6 copyleft-isolation note;
   recorded explicitly so the §6.3.3 no-UNKNOWN gate does not block on it). The §6.3.3
   attribution-completeness gate fails if any shipped component lacks a row, so these
   must be enumerated or the release blocks.
5. `tauri build` includes both as resources (§3.3.1).

### 3.7.3 Completeness gate (cross-ref §6.3, release-blocking)

CI fails the release if any bundled binary/lib/font in the manifest lacks a
licence-text entry or a source pointer — directly implementing the SSOT
"missing attribution is release-blocking" rule. The check is **manifest-driven**:
every `externalBin` + every `resources` engine file must have a manifest row.

---

## 3.8 Engine maintenance & versioning

**Posture (SSOT):** keeping bundled engines reasonably current/patched is a
**best-effort maintenance posture — NOT a v1 ship gate, no SLA, no committed patch
turnaround** (they are third-party decoders = a classic attack surface, so
"bundle once, never update" is unacceptable, but currency is never a release
blocker).

- **Pinning:** every engine is pinned to an **exact version + source ref** in the
  build manifest (§3.7.2). Reproducible inputs → the §3.7 SBOM is exact and the
  §6.2 integrity hashes are meaningful.
- **Minimum-version floors that gate a *capability*** (not just currency) are
  recorded here so a bump can't silently drop them: **libvips ≥ 8.12** (native
  `gifsave`/cgif backend — §3.5.5/§3.6.1, so the GIF path uses cgif natively). A bump
  below such a floor is rejected. **Note:** there is **no native-`bmpsave` floor** —
  libvips has no native BMP or ICO save at any version; BMP (both directions) and ICO
  save go through the **required** ImageMagick delegate (§3.1 row 1d), so the
  ImageMagick component is a hard build dependency, not a tunable floor.
- **Update trigger (best-effort):** a security advisory in a bundled decoder
  (FFmpeg, poppler, libheif/libde265, libvips loaders, LibreOffice filters — the
  untrusted-input parsers) is the practical reason to bump; cosmetic upstream
  releases are not chased.
- **Re-validation on bump (cross-ref §6.5):** a bumped engine is **re-run against
  the §6.4/§6.5 real-world corpus** before it ships — a bump that regresses a
  previously-reliable pair (§6.5 gate) is not released until fixed or reverted.
  This is the concrete tie between "best-effort currency" and "no regression of a
  done pair."
- **Per-engine timeout/hang values** (§1.7 owns the mechanism) are part of an
  engine's profile and re-checked on a bump (a new LibreOffice may change
  cold-start time; a new FFmpeg may change `-progress` output).
- **No runtime update path** (SSOT: no phone-home; §7.6): a patched engine reaches
  users only via a **new full release** on the canonical GitHub Releases page —
  there is no engine-only delta download (that would breach the offline/no-fetch
  floor).

---

## 3.9 Binary-size budget

**Accepted trade-off (SSOT *Completeness > lightweight*, temp.md):** bundling
everything — *including* LibreOffice — makes the download large; this is
deliberate. v1 has **no hard size cap** (completeness is the gate, not size), but
the budget below sets expectations and identifies what dominates so trimming
effort is spent where it matters.

### 3.9.1 Estimated per-component compressed contribution

| Component | Rough installed size | What it is | Trim levers |
|---|---|---|---|
| **LibreOffice (headless, trimmed)** | **~250–400 MB** (dominant) | Writer+Calc+Impress program tree + needed type libs; **minimal** build (no help, no UI translations, no dictionaries, no DB/Draw/Math beyond deps) | strip help/l10n/dictionaries (under ~200 MB minimal is reported feasible); drop unused modules; the **bundled font set is a sub-line below** |
| **Bundled fonts** (LibreOffice + documents/presentations fidelity) | **~30–120 MB** `[OPEN — §3.9.2]` | Liberation/Carlito/Caladea (metric-compat Arial/Calibri/Cambria/Times/Courier) + broad **CJK + RTL** coverage (Noto-class) | CJK is the size driver; a full Noto CJK is ~100 MB+ — subset vs full is the open call |
| **FFmpeg + ffprobe** (GPL-2.0+ build, the listed codecs incl. x264/vpx) | **~30–80 MB** (two exes + their shared libs) | multimedia binary **with the LGPL component libs as dynamically-linked shared objects beside the exe** (NOT a static build — a static link would fail the §6.1.3 LGPL dynamic-link assertion, §3.6.1) | drop unused (de)muxers/filters via `--disable-everything --enable-…` to a curated list (the `04` codec set only) |
| **libvips + image codec stack** (libheif/libde265/x265-plugin/aom/dav1d/librsvg/cgif + **required ImageMagick** delegate) | **~20–40 MB** | image lib + codecs (image-worker process) | exclude unneeded loaders; ImageMagick is **required** (BMP+ICO save) but trimmed to BMP/ICO/GIF delegates with **GPL optional delegates excluded** (§3.6.1) — it cannot be removed |
| **poppler `pdftotext`** | **~5–15 MB** | PDF text extractor | small |
| **pandoc** | **~80–220 MB** (version-dependent; pandoc 3.x) | Haskell static binary (notoriously large; the **GHC runtime dominates**, so stripping saves little) | a release/stripped build trims marginally; the real lever is **dropping pandoc in favour of LibreOffice 26.2 Markdown** (co-owned [OPEN] with documents.md) — this is the **second-biggest single exe** after LibreOffice |
| **Ghostscript** **[DECIDED: NOT shipped v1]** | **0 MB** (~30–60 MB if ever re-added) | PDF repair backstop — dropped (§3.1) | already dropped; this whole row is saved |
| **ConvertIA Tauri app** (Rust core + WebView assets) | **~10–25 MB** | the app itself (Tauri's own footprint is small — the WebView is system-provided) | Tauri's whole point: tiny vs Electron |
| **WebView runtime** | **0 MB bundled** | system WebView2/WKWebView/WebKitGTK (never bundled, never downloaded — §3.4.5/§0.3.1) | n/a |

### 3.9.2 Total estimate & what dominates

- **Per-platform total: roughly ~430 MB–820 MB installed** (revised up for the
  pandoc 3.x ~80–220 MB figure), depending almost entirely on (a) the LibreOffice
  trim level, (b) the **bundled-font breadth** (CJK is the swing factor — Latin-only
  would be tens of MB; full CJK+RTL pushes toward the top of the range), (c) whether
  **pandoc is kept at all** (dropping it in favour of LibreOffice 26.2 Markdown is
  the single biggest non-font trim lever, co-owned [OPEN] with documents.md), and
  (d) **Ghostscript** is **[DECIDED: dropped]** (saving ~30–60 MB; not in the total).
- **LibreOffice + fonts + pandoc together are ~80–90% of the bundle.** Image and
  PDF-text tooling are minor. Trimming effort, if any is spent, belongs there.
- **Compression:** the release artifact is compressed (platform-native: NSIS/zip,
  dmg, AppImage squashfs) — download size is materially smaller than installed;
  exact ratios `[DEFER §6.1/§6.2]`.
- This is an **estimate for planning**, not a contract; the real numbers are
  measured in §6.1 once the trimmed engine builds exist, and fed back here.

### 3.9.3 Open size decisions (genuine)

- **`[DECIDED]` Bundled-font baseline** (adopting the [REC]) — **Liberation +
  Carlito + Caladea** (metric-compat Arial/Calibri/Cambria/Times/Courier) **+ a
  curated Noto subset: Noto Sans/Serif CJK-SC/TC/JP/KR "Regular" weights + Noto Sans
  Arabic/Hebrew**. This is the single biggest fidelity lever for documents/
  spreadsheets/presentations (their font `[OPEN]`s — `documents.md` §5,
  `presentations.md` [OPEN-2] — all resolve to this baseline). The **only residual is
  the CJK weight count / SC-vs-all-CJK breadth**, a pure size knob: **[DEFER: tune
  CJK breadth against the §3.9 size measurement once the trimmed builds exist].** The
  *families* are fixed; only how many CJK weights ship is the deferred calibration.

---

## 3.x Decision tags summary (for the README open-questions log)

| Item | Tag | Owner | Note |
|---|---|---|---|
| Bundle everything, fully offline | `[DECIDED]` | §3.3 | inherited Phase-1 |
| Copyleft engines = separately-invoked binaries; MIT core clean | `[DECIDED]` | §3.6 | x264/x265/poppler/pandoc invoked, never linked |
| Engine inventory per category | `[DECIDED]` | §3.1 | fixed by `04-formats/*` |
| Engines spawned by Rust core (not WebView shell); no `shell:allow-execute` to WebView | `[DECIDED — recommended]` | §3.3.3 / →§0.10 | tighter threat surface + full subprocess control |
| **AAC ship-bundled all 3 platforms** | `[DECIDED — recommended]` | §3.4 | native FFmpeg AAC, LGPL-clean; one-product requires it |
| **H.264 ship-bundled all 3 platforms** | `[DECIDED — recommended]` | §3.4 | MP4 default-target depends on it; ~2027 expiry |
| **HEVC *decode* ship-bundled all 3 platforms** | `[DECIDED — recommended]` | §3.4 | libde265 LGPL, decode-only; HEIC-source default path |
| **HEVC *encode* (write HEIC) disposition** | **`[DECIDED]`** | §3.4 | ship-bundled-isolated (x265, GPL → separate invoked binary), **behind the §3.4 availability flag** so it can flip to `unavailable` (SSOT exception-1) as a config change. kvazaar (BSD) recorded as the license-clean alternative. |
| AVIF ship-bundled all 3 platforms | `[DECIDED]` | §3.4 | royalty-free |
| Drop Ghostscript in v1 | `[DECIDED]` (DEFER re-add to corpus) | §3.1 / §3.6 | poppler-only `PDF→TXT`, no AGPL surface; [DEFER: re-add only if the §6.5 corpus shows poppler failing PDFs GS would salvage] |
| **FFmpeg licence class = GPL-2.0+** (enables x264) | **`[DECIDED]`** | §3.1 / §3.6.1 | the whole FFmpeg binary is GPL-2.0+, not LGPL; separate invoked binary (aggregation); written-offer-of-source; LGPL component libs dynamically linked beside it |
| SBOM format = CycloneDX JSON; manifest-driven generation | `[recommended]` | §3.7 | feeds §6.3 release-blocking gate |
| HEIC/AVIF encode code-path | **`[DECIDED]`** | §3.5.5 / images.md [OPEN-1] | libvips `heifsave` (`compression=hevc|av1`) for all HEIC/AVIF encode; **one AV1 encoder (libaom)** ships; standalone heif/avif encoders dropped; x265 ships as a **dynamically-loaded libheif plugin** |
| GIF native; **BMP+ICO require ImageMagick** | **`[DECIDED]`** | §3.5.5 / images.md | native `gifsave` (cgif, MIT); **BMP load+save and ICO save go ONLY through the REQUIRED ImageMagick `magicksave`/`magickload` delegate** (libvips has no native BMP/ICO save at any version); ImageMagick is permissive (not GPL) and **cannot be dropped** |
| libvips placement = **separate image-worker process** | **`[DECIDED]`** | §3.5.5 → §2.12/§0.9 | resolves the §2.12 T1 isolation + the §2.12.4 "all subprocesses" absolute in one stroke; licence analysis unaffected |
| Bundled-font baseline | **`[DECIDED]`** (CJK breadth `[DEFER: size]`) | §3.9.3 | Liberation+Carlito+Caladea+curated Noto CJK/RTL subset; shared by docs/sheets/slides; only the CJK weight count is size-tuned |

---

> **Sources consulted (patent landscape & sizes, June 2026):** HEVC/H.265 pools &
> Jan-2026 rate change — Access Advance / Via-LA; H.264/AVC last-patent ~2027-11 —
> MPEG-LA pool records / end-software-patents wiki; AAC distribution-vs-encoder
> royalty split & Via-LA reorg — Via-LA AAC FAQ / SCC Online; libheif LGPL + x265
> GPL & decode-only libde265 — strukturag/libheif, x265.org; LibreOffice headless
> ~190 MB minimal–~400 MB — LibreOffice portable/headless distributions; Tauri v2
> `externalBin`/`resources`/sidecar + capabilities — v2.tauri.app docs (verified
> via Context7). These ground the §3.4 recommendations; the owner-level `[OPEN]`s
> (HEVC-encode, font set) remain explicit calls, not closed by research.
