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
components 1a–1d, including the **mandatory bundled ImageMagick delegate** for BMP/ICO
save — §3.5.5), FFmpeg, LibreOffice, poppler, pandoc — plus ConvertIA's own in-core Rust
text engine. **Ghostscript is `[DECIDED: NOT shipped in v1]`** (poppler-only PDF→TXT, no
AGPL — §3.6); it is **not** an "optional" component. Counting each separately-licensed
bundled component (the SBOM granularity, §3.7), the inventory rows below enumerate
every one; they cluster into four families:

| # | Engine (bundled artifact) | Family | Drives (cross-ref) | Licence | Ships as | Patent flag |
|---|---|---|---|---|---|---|
| 1 | **libvips** (raster core; built with libheif/libde265, libaom/dav1d, the native **`svgload` SVG load module (librsvg)**, **cgif** for native `gifsave`, and a **REQUIRED ImageMagick** delegate for BMP save, the default ICO-save path (**`[DEFER: build spike]`** §3.5.5; in-core Rust ICO assembler fallback), and GIF fallback) | Images | `04-formats/images.md` (raster↔raster, SVG→raster, HEIC/AVIF **decode**, HEIC↔AVIF via `heifsave`) | **LGPL-2.1+** (libvips); **cgif MIT**; see per-component rows | linked lib **inside the separate image-worker process** (LGPL — static-link-as-aggregation OK with the §6.1.3 carve-out ii relinkable-source bundle; never into the MIT core, §3.6) | none for its own codecs |
| 1a | **libheif + libde265** (HEVC decode) + **x265** (HEVC encode, built as a **dynamically-loaded libheif encoder plugin** `.so`/`.dll`/`.dylib`, not statically linked) — used by libvips' HEIC load module via `heifsave compression=hevc` | Images | HEIC decode (vips) / HEIC encode | libheif **LGPL-3.0**, libde265 **LGPL-3.0**, **x265 GPL-2.0-or-later** (verify vs the pinned source's `COPYING`; -or-later is compatible with the LGPL-3.0 libheif host, GPL-2.0-only would not be) | **x265 → dynamically-loaded libheif *plugin*, isolated** (§3.6); libheif/libde265 LGPL link | **HEVC → §3.4** |
| 1b | **AV1: libaom (enc, via libheif `heifsave compression=av1`) / dav1d (dec, via vips AVIF load module)** — the ONE bundled AV1 encoder is **libaom** (the standalone `libavif`+aom encoder is **not** bundled; encode standardised on `heifsave`, images.md [OPEN-1] [DECIDED]) | Images | AVIF decode/encode | libaom **`BSD-2-Clause AND LicenseRef-AOMPL-1.0`** (the row MUST carry **both** the BSD-2-Clause code licence **and** the "Alliance for Open Media Patent License 1.0" from the `PATENTS` file — complete attribution, §3.7. **SPDX id note `[DECIDED]`:** the AOM Patent License has **no registered SPDX short id** — `AOMPL-1.0` is only a pending SPDX request — so it is expressed as the CycloneDX/SPDX **`LicenseRef-AOMPL-1.0`** custom-licence reference with the full AOM Patent License text carried in `THIRD-PARTY-LICENSES.txt`; the §6.3.3 gate's LicenseRef carve-out treats this as a *resolved* id. Switch to the bare `AOMPL-1.0` once SPDX registers it); dav1d **`BSD-2-Clause`** | LGPL/BSD link in the image worker | AV1 royalty-free; **ship-posture → §3.4** |
| 1c | **librsvg** (SVG rasteriser — libvips' native `svgload` module is librsvg-backed; resvg is NOT a libvips backend at any released version, so it is **not shipped** [DECIDED]) | Images | SVG→raster | **LGPL-2.1+** (librsvg) | linked load module inside the separate image-worker (LGPL — static-link-as-aggregation OK with the §6.1.3 carve-out ii relinkable-source bundle; never into the MIT core, §3.6) | none |
| 1d | **ImageMagick** (libvips BMP save delegate — **REQUIRED for BMP**; ICO save is the **default** path but **`[DEFER: build spike]`** §3.5.5; plus GIF fallback) | Images | **BMP load+save (`magickload`/`magicksave` — REQUIRED)**; **ICO save (`magicksave`) — default, multi-size/256px unverified, in-core Rust ICO assembler fallback §3.5.5**; GIF fallback | **ImageMagick License** (Apache-2.0-style, SPDX `ImageMagick`) — **permissive, NOT GPL** | linked delegate (permissive — no isolation); GPL *optional delegates* excluded at build | none |
| 1e | **libimagequant** — **the BSD-2-Clause `lovell/libimagequant` v2.4.x fork ONLY** (PNG/GIF palette quantisation, used by libvips' `cgif`/`gifsave` and palette PNG output) | Images | PNG/GIF palette quantisation | **BSD-2-Clause** — and **only** via the frozen `lovell/libimagequant` v2.4.x fork (e.g. v2.4.1). **Upstream libimagequant 4.x is GPLv3-or-commercial — NOT permissive — and MUST NOT be bundled** (it would taint the LGPL image-worker). Pin the BSD fork by exact version+ref in `engines.lock`; a §6.1.3/§6.3.3 build assertion checks the staged `COPYRIGHT` actually contains the BSD-2 text. **ABI/soname coupling `[DECIDED]`:** libimagequant 4.x changed its **soname**, and the bundled libvips' `cgif`/`gifsave` (the §3.8 floor) links a **specific libimagequant version** — so the **bundled libvips MUST be built/linked against the v2.4.x-fork API/soname**, and a **§6.1.3 build/link assertion verifies the staged libvips resolves the bundled BSD libimagequant v2.4.x (NOT a system 4.x)**, not just that the COPYRIGHT text is BSD. | linked/vendored **inside the image-worker process** (BSD fork only) | none |
| 2 | **FFmpeg** (**GPL-2.0+ build** — `./configure --enable-gpl` to link `libx264`; built **without `--enable-nonfree`**: `libmp3lame`, `libvorbis`, `libopus`, native `aac`/`flac`/`alac`/`pcm`, `libx264`, `libvpx-vp9`, **WMA *decoders* (decode-only — there is NO WMA encoder; WMA is a source-only format per audio.md)**; no `libfdk_aac`) | Audio, Video, Cross-category | `04-formats/audio.md`, `video.md`, `cross-category.md` | **GPL-2.0+** (the whole binary, because it enables GPL `libx264`; the LGPL component libs are still dynamically linked beside it, §3.6.1); written-offer-of-source obligation | **separate invoked binary** (`ffmpeg`/`ffprobe`) per §3.6 | **AAC, H.264 → §3.4**; MP3/Vorbis/Opus/FLAC/ALAC/PCM/VP9 patent-clean |
| 3 | **LibreOffice** (headless `soffice`, Writer+Calc+Impress + PDF export filters; bundled with a baseline open font set, §3.9) | Documents, Spreadsheets, Presentations | `04-formats/documents.md`, `spreadsheets.md`, `presentations.md` (all office↔office + every `*→PDF`) | **MPL-2.0** (+ many bundled components — full set enumerated by the SBOM, §3.7) | **separate invoked binary** (sidecar process) per §3.6 | none |
| 4 | **poppler** (`pdftotext`) | Documents | `PDF→TXT` | **`GPL-2.0-only OR GPL-3.0-only`** (a valid SPDX expression — *not* the bare `GPL-2.0/GPL-3.0`, which §6.3.3 would reject as unresolved) | **separate invoked binary** (§3.6) | none |
| 5 | **Ghostscript** **[DECIDED: NOT shipped v1]** (was a PDF read/repair backstop behind poppler; no user-facing pair) | Documents | (malformed-PDF tolerance — dropped) | **AGPL-3.0** | not shipped (`[DEFER: re-add if §6.5 corpus shows GS-salvageable PDFs]`) | none |
| 6 | **pandoc** | Documents | markup conversions (`MD/HTML/TXT ↔`, office→markup for XML/text sources) | **GPL-2.0+** | **separate invoked binary** (§3.6) | none |
| — | **ConvertIA native CSV/TSV engine** (Rust, in-core) | Spreadsheets | `CSV↔TSV`, encoding/delimiter sniff | **MIT** (own code) | compiled into the core | none |

**Per-family notes.**
- **libvips** runs **inside the image-worker process** (§0.7/§3.5.5 `[DECIDED]`),
  linked there rather than spawned as a standalone exe, because its job is many small,
  latency-sensitive image ops and it is LGPL (link-compatible, §3.6); the worker
  process gives the §2.12 isolation boundary. **ImageMagick is a REQUIRED bundled
  component, NOT a fallback `[DECIDED]`:** libvips has **no native BMP support at all**
  (BMP load *and* save go through the ImageMagick `magickload`/`magicksave` delegate),
  so BMP (both directions) — an in-scope v1 format — **depends on ImageMagick**.
  **ICO save is `[DEFER: corpus/build spike]`** (§3.5.5): the default path is also
  `magicksave` (libvips has no native ICO saver), but ImageMagick's 256px/multi-size ICO
  support is unverified, so v1 either confirms it via the §6.1.3 spike OR falls back to an
  **in-core Rust ICO container assembler** wrapping vips-produced frames (which would remove
  ImageMagick from the ICO path). (The native **cgif** `gifsave` claim is correct;
  ImageMagick is only a *GIF* fallback.) ImageMagick is **permissive — the ImageMagick License (an OSI-approved
  Apache-2.0-style licence, SPDX `ImageMagick`), NOT GPL** — so it is link-OK like the
  BSD/MPL components and is **not** an aggregation/isolation case. The only **GPL**
  component reachable from the image stack is **x265** (HEVC encode), the genuine
  aggregation case (a dynamically-loaded libheif encoder *plugin*, never statically
  linked — see §3.6). **Build caveat:** ImageMagick *optional delegates* can themselves
  be GPL, so the trimmed build **must exclude GPL delegates** — but IM core itself is
  permissive. **libvips' OWN copyleft loaders are excluded too `[DECIDED]`:** a stock/
  distro libvips often enables the **poppler-glib PDF loader (GPL — it makes the whole
  libvips build effectively GPL, libvips#2222)** or a **MuPDF PDF loader (AGPL)**. The
  load-bearing "libvips is LGPL → dynamic link OK" claim only holds **without** those
  loaders. ConvertIA needs **no** libvips PDF loading (PDF→TXT is the separate poppler
  `pdftotext` **sidecar**, §3.5.3), so the bundled libvips is configured **without the
  poppler/PDF loader, without the MuPDF loader, and without any other GPL/AGPL loader**;
  a §6.1.3 positive build assertion fails the build if a poppler/mupdf loader is present.
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

> **Probe-then-encode is NOT chaining (and `plan()` stays Pure) `[DECIDED]`.** The
> single-engine FFmpeg video job runs **two sequential sub-invocations of the SAME engine**
> — `ffprobe` (read inner codecs + duration) **then** `ffmpeg` (remux-or-reencode) — which
> is **not** an `A→(X)→B` *format* chain (no intermediate artifact, one engine, one source,
> one target). To keep `Engine::plan()` **Pure** (no I/O) while the encode argv depends on
> the probe result, the **two-phase contract is `[DECIDED]` as a second trait method**
> (option b — a `plan_encode(ProbeOutput) -> Invocation`, NOT struct mutation, NOT a
> stored closure):
> - `plan()` returns the **probe `Invocation`** (the `ffprobe` sub-invocation) for a
>   probe-requiring engine (video FFmpeg); for non-probe engines `plan()` returns the
>   single encode `Invocation` directly and `plan_encode` is never called. **The probe
>   `Invocation` `[DECIDED]`** carries **`program: EngineProgram::Sidecar(EngineId::FFprobe)`**
>   (resolves `binaries/ffprobe`, distinct from `binaries/ffmpeg`; §0.6 / §3.3.1),
>   **`out_tmp: None`** (the probe writes only stdout JSON — no publish artifact; §1.7
>   runs no publish/cleanup for it), and **`progress: ProgressModel::CoarseSpawnDone`**
>   (a short read, not a streaming `-progress` source — §1.7 dispatches it through the
>   coarse spawn→done path, never the FfmpegKeyValue line-reader).
> - the §3.2.2 `Engine` trait gains **`fn plan_encode(&self, job: &ConversionJob,
>   out_tmp: &TempPath, probe: &ProbeOutput) -> Result<Invocation, PlanError>`** — §1.7 runs
>   the probe sub-invocation, parses its stdout into a typed **`ProbeOutput`** (inner codecs
>   + `duration_us` + rotation + interlace), then calls `plan_encode(.., &probe)` to get the
>   finalised encode `Invocation`, which §1.7 then spawns.
> - **`duration_us` is provided BY the `ProbeOutput`** carried into `plan_encode` (the
>   encode `Invocation`'s `progress` is built with the real denominator at that point) —
>   it is **NOT** mutated in-place onto a prior `progress` struct returned before the probe.
>
> So for video, `plan()` is the probe and `plan_encode(probe)` is the encode — never a
> fixed encode argv computed before the probe. §3.5.1 / §1.7 own the two-step sequencing;
> §3.2.1's "one engine, one (format) conversion, no intermediate artifact"
> invariant is intact (the probe is a read, not a conversion step).

### 3.2.2 The `Engine` trait (registry seam — physical home owned by §0.7)

The engine layer is a **registry of capability-declaring engines** behind one
trait. The trait lives in the engine-registry crate/module (§0.7 owns where);
this section owns its **shape and semantics**. Pseudo-signature (Rust):

```rust
/// A bundled conversion engine. One impl per engine binary/lib.
pub trait Engine: Send + Sync {
    /// Stable id for logging/SBOM/registry (e.g. "ffmpeg", "libreoffice", "vips").
    fn id(&self) -> EngineId;

    /// The §0.6 capability descriptor for this engine, incl. `serialised_only` and
    /// `kind: EngineKind`. The §0.9 pool reads `descriptor().serialised_only` from a
    /// job's resolved `EngineId` BEFORE spawn to decide whether to also acquire the
    /// engine's single-permit semaphore (LibreOffice). This is the concrete
    /// `EngineId → serialised_only` data path §0.9 depends on (without it the pool has
    /// no way to get `serialised_only` from the §3.2.3 `(SourceFmt,TargetFmt)→EngineId`
    /// registry). Pure, const-ish (a static fact per engine).
    fn descriptor(&self) -> EngineDescriptor;

    /// What this engine can do, *on this platform*, given the §3.4 patent
    /// disposition resolved at build time. Used to populate the registry and to
    /// decide per-platform availability (honest "unavailable here").
    fn capabilities(&self, platform: Platform, patents: &PatentDisposition)
        -> Vec<EngineCapability>;      // named struct (defined below): {source, target, direction}

    /// Build the concrete invocation plan for one job. Pure (no I/O, no spawn):
    /// returns argv / env / cwd / progress-parser kind / temp-output path shape.
    /// The actual spawn/cancel/timeout is owned by §1.7; this only *describes* it.
    /// For a PROBE-requiring engine (video FFmpeg, §3.2.1) this returns the **probe
    /// sub-invocation** (`ffprobe`) whose `Invocation.out_tmp` is `None` (the probe has
    /// no publish artifact); the `out_tmp` PASSED IN is the ENCODE output temp, which
    /// `plan()` of a probe engine ignores and `plan_encode` consumes for the encode.
    /// For a single-step engine it returns the encode `Invocation` directly (with
    /// `out_tmp: Some(..)` built from the passed temp).
    fn plan(&self, job: &ConversionJob, out_tmp: &TempPath)
        -> Result<Invocation, PlanError>;

    /// Two-phase encode plan `[DECIDED §3.2.1]`. Called by §1.7 ONLY for an engine whose
    /// `plan()` returned a probe sub-invocation: §1.7 runs the probe, parses its stdout
    /// into `ProbeOutput`, then calls this to finalise the encode `Invocation` (with
    /// `out_tmp: Some(..)`). The progress denominator (`duration_us`) is taken FROM
    /// `probe` here — never mutated onto a previously-returned struct. Default impl
    /// returns a `ConversionErrorKind::InternalError` PlanError with the detail string
    /// below (single-step engines never reach it — §1.7 only calls `plan_encode` after a
    /// probe Invocation). Pure (no I/O, no spawn).
    fn plan_encode(&self, _job: &ConversionJob, _out_tmp: &TempPath, _probe: &ProbeOutput)
        -> Result<Invocation, PlanError> {
        Err(PlanError { kind: ConversionErrorKind::InternalError,
                        detail: "engine has no probe/encode two-phase plan".into() })
    }

    // NO `progress_model()` trait method `[DECIDED]`. Progress is a PER-INVOCATION
    // property, not a per-engine constant: the SAME video FFmpeg engine emits a probe
    // `Invocation` with `progress: ProgressModel::CoarseSpawnDone` and an encode
    // `Invocation` with `progress: ProgressModel::FfmpegKeyValue { duration_us }` — two
    // different values for one engine, which a single static method cannot express. The
    // §1.7 dispatch therefore reads the progress model from `Invocation.progress` (the
    // field §1.7 already carries in its `EngineInvocation.plan`); §1.11 normalises THAT.
    // (FFmpeg `-progress` k=v, LibreOffice coarse, image-worker libvips eval-progress
    // marshalled to stdout k=v across the worker boundary — all set per-invocation on
    // `Invocation.progress` by `plan()`/`plan_encode()`.)

    /// Map this engine's exit code + stderr into the §2.8 error taxonomy.
    /// Returns the §2.8-owned `ConversionErrorKind` (NOT a separate "FailureKind" —
    /// that name is dropped; §2.8 is the single owner of the failure-kind set).
    /// `ErrorKind` (§0.4.3) is the wire projection of `ConversionErrorKind`; the
    /// §06 drift check keeps the two byte-identical for ALL variants (the item-level
    /// kinds AND the run/app-level MixedDrop/EngineMissing/WebviewFault/BundleDamaged).
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
    pub out_tmp: Option<TempPath>, // engine writes here; §2.1 atomic-publishes on success.
                                   //   `Some` for every ENCODE invocation (the publish artifact).
                                   //   `None` for a READ-ONLY sub-invocation that produces no
                                   //   publish artifact — the video PROBE (`ffprobe`, §3.2.1):
                                   //   ffprobe writes only stdout JSON, so there is NO output
                                   //   temp to allocate and NOTHING for §2.1 to publish. §1.7
                                   //   atomic-publishes ONLY when `out_tmp.is_some()`; for a
                                   //   `None` invocation §1.7 parses stdout and runs no publish/
                                   //   cleanup step (§1.7 cleanup table). [DECIDED]
}

/// How the Rust core locates the program to spawn. Engines are spawned Rust-side
/// (§3.3.3), never via the WebView shell — the path is resolved through Tauri's
/// PathResolver (externalBin sidecar or a binary inside the resources tree §3.3.1).
pub enum EngineProgram {
    Sidecar(EngineId),            // externalBin. STAGED at build as binaries/<name>-<triple>[.exe];
                                  //   Tauri STRIPS the triple on bundle, so at RUNTIME it is the
                                  //   bare <name>[.exe] beside the app exe — resolve via
                                  //   current_exe().parent() (§3.3.3), NOT BaseDirectory::Resource.
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

/// The publish-temp the engine writes its output to. `[DECIDED]` This is
/// `tempfile::TempPath` (tempfile is already in §0.8): a path whose file is deleted
/// on drop, matching the §2.1 "path deleted on drop / never a placeholder" semantics.
/// Lifecycle: the path is picked by `crate::run` inside the destination volume
/// (§2.14.4) and OWNED by the §1.7 invocation; on item SUCCESS the §2.1 atomic publish
/// consumes it (rename/link) so drop is a no-op; on failure/cancel drop (or the §2.6
/// sweep) removes it. The engine only writes to it — it never owns deletion.
pub type TempPath = tempfile::TempPath;

/// A pure planning error (no I/O): the engine cannot build an Invocation for this
/// job (e.g. an option value out of range). Mapped by §1.7 to a §2.8 kind
/// (typically InternalError/UnsupportedPair). Distinct from a runtime failure.
pub struct PlanError { pub kind: ConversionErrorKind, pub detail: String }

/// The parsed result of a probe sub-invocation (§3.2.1 two-phase contract), produced
/// by §1.7 from `ffprobe`'s stdout and handed to `plan_encode`. Engine-layer-internal.
/// `duration_us` becomes the ProgressModel::FfmpegKeyValue denominator for the encode
/// (provided here, NOT mutated onto a pre-probe struct). Video FFmpeg is the only v1
/// probe-requiring engine; the shape is FFmpeg-shaped but the contract is generic.
pub struct ProbeOutput {
    pub duration_us: u64,                 // total media duration → §1.11 progress denominator
    pub inner_codecs: Vec<String>,        // stream codecs → video.md remux-vs-reencode decision
    pub rotation_deg: Option<i32>,        // display rotation (auto-orient)
    pub interlaced: Option<bool>,         // flagged-interlaced → §video.md deinterlace default
}

pub enum ProgressModel {
    FfmpegKeyValue { duration_us: u64 },   // denominator = ffprobe duration (video.md)
    VipsStdout,                            // image-worker marshals the libvips eval-progress
                                           //   callback to stdout `progress=<0..100>` key=value
                                           //   lines (the worker is a SEPARATE process, §3.5.5);
                                           //   parsed by the §1.7 same line-reader path as
                                           //   FfmpegKeyValue. (Renamed from VipsCallback — an
                                           //   in-process callback cannot cross the worker's
                                           //   process boundary.)
    CoarseSpawnDone,                       // LibreOffice/pandoc/poppler: 0%→spin→100%.
                                           //   ALSO the video PROBE sub-invocation
                                           //   (`ffprobe`, §3.2.1): the probe is a short
                                           //   read whose ONLY output is a single stdout
                                           //   JSON blob (NOT FFmpeg `-progress` key=value
                                           //   lines), so it streams no fraction — §1.7
                                           //   dispatches it through the coarse spawn→done
                                           //   path, NOT the FfmpegKeyValue line-reader.
                                           //   [DECIDED] The probe Invocation always carries
                                           //   `progress: ProgressModel::CoarseSpawnDone`;
                                           //   the FfmpegKeyValue model belongs to the ENCODE
                                           //   Invocation returned by `plan_encode`, whose
                                           //   `duration_us` comes FROM the parsed ProbeOutput.
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
- The few `04` engine-ownership items for a pair (`MD→PDF` LO-vs-pandoc `[DEFER: corpus]`;
  `RTF→markup` pandoc-vs-LO `[DEFER: corpus]`; HEIC/AVIF encode standardised on vips
  `heifsave` `[DECIDED]`) are **owned by their `04` files**, referenced here: whichever
  way the deferred ones resolve, it remains a *single* registry owner — the trait and
  lookup are unaffected. They feed the README open-questions log via `04`.

---

## 3.3 Bundling model (all offline) `[DECIDED: bundle everything]`

**Everything ships inside the build; zero runtime fetch** (SSOT *Local/private/
offline*, *v1 DoD* offline floor). No engine, codec, font, or update is ever
downloaded after the app itself.

### 3.3.1 Two physical bundling mechanisms (Tauri v2)

| Mechanism | Used for | Tauri config | Resolved at runtime by |
|---|---|---|---|
| **`bundle.externalBin`** (sidecars, target-triple-suffixed) | FFmpeg, ffprobe, soffice launcher, pdftotext, pandoc, **`convertia-imgworker`** (the libvips image-worker process, §3.5.5) — the **standalone invoked binaries** (Ghostscript **[DECIDED: dropped]**; **x265 is NOT a sidecar** — it ships as a dynamically-loaded libheif encoder *plugin* under `resources`, §3.1 row 1a) | `"bundle": { "externalBin": ["binaries/ffmpeg", "binaries/ffprobe", "binaries/soffice", "binaries/pdftotext", "binaries/pandoc", "binaries/convertia-imgworker"] }` | spawned by the Rust core (see 3.3.3) |
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
    launcher where single-file): resolved at runtime by the **bare name** (NO triple
    suffix) **beside the app executable** —
    **`std::env::current_exe()?.parent()` joined with `ffmpeg` / `ffmpeg.exe`** (the
    same `[.exe]` extension rule as the app binary). **The `-<target-triple>` suffix is a
    build/stage-time naming convention only** (§3.3.2 stages the source as
    `binaries/<name>-<target-triple>[.exe]`); **Tauri strips that suffix when bundling**,
    so the shipped binary next to the app exe is just `ffmpeg`/`ffmpeg.exe` — resolving
    `current_exe()` + the triple suffix would look for a file that does not exist in the
    shipped bundle. **Do NOT use `BaseDirectory::Resource` for externalBin** — sidecars sit
    next to the main exe, not in the resources tree. Because §0.10 deliberately omits the
    shell plugin, this manual `current_exe()`-relative resolution (the same location the
    shell plugin's `sidecar()` would compute) is the supported pattern. Resolves to an
    **absolute path**; `PATH` is never relied on (§3.5 env note).
  - **resources-tree binaries** (the LibreOffice `program/soffice.bin` and the other
    `bundle.resources` engine files): **`app.path().resolve("engines/libreoffice/program/soffice", BaseDirectory::Resource)`**
    — an absolute path inside the bundled resource tree. `BaseDirectory::Resource` is
    correct **here only** (genuine resources-tree binaries), not for externalBin.
  This is the `EngineProgram::{Sidecar, ResourceBin}` distinction in §3.2's
  `Invocation`. The externalBin/resources placement (§3.3.1/§3.3.2) guarantees the
  file exists beside the app (portable, no install — SSOT *Portable, no installation*).
  - **`Sidecar(EngineId)` → binary-filename mapping `[DECIDED]`.** Because
    `EngineProgram::Sidecar` carries only an `EngineId`, the resolver needs the bare
    binary name per `EngineId`. The convention is a **fixed `EngineId → binary-name`
    table** owned here (Phase-3 does not invent one): `FFmpeg → "ffmpeg"`,
    `FFprobe → "ffprobe"`, `LibreOffice → "soffice"` (launcher, where single-file —
    else resolved via the resource tree, §3.3.1), `Poppler → "pdftotext"`,
    `Pandoc → "pandoc"`, `ImageCore → "convertia-imgworker"` (the libvips image-worker,
    §3.5.5). The non-trait/non-sidecar `EngineId::ImageMagick` is **not** in this table
    (it is a delegate linked inside the image-worker, never spawned as its own sidecar,
    §3.5.5). **`EngineId::NativeCsvTsv` is also absent from this table** (it is
    `InProcessNative`, §3.5.6 — an in-core pure-Rust engine with **no sidecar binary** to
    resolve/spawn; its §7.2.3 `EngineStatus` is synthesized, not loop-derived). The `.exe`
    extension is appended on Windows (same rule as the app binary). This is the single
    source of the externalBin names listed in §3.3.1.
- `[DECIDED]` ConvertIA does **not** depend on the Tauri **shell plugin** for
  engine execution at all — engines run only from Rust (`tokio::process`). §7.7's
  open-folder/open-file/open-url uses the separate **`opener` plugin**, which is
  unrelated to `shell:allow-execute`. Net: **no WebView command may execute an
  engine**; there is **no `shell:allow-execute` on the §0.10 allowlist** (the prior
  draft's grant was removed — see §0.10). This closes the §0.10/§3.3.3 [OPEN] by
  cross-reference: the answer is **no WebView shell grant**.

### 3.3.4 Offline invariant (cross-ref §2.11)

Every engine above is self-contained once bundled. The only network code paths in
the entire app are the **user-initiated** §7.7 open-project-page shell-out. The
"no engine fetches anything" property is backed by **structural, always-on controls**,
**not** by the degradable §2.12 OS network-deny:
- **FFmpeg/ffprobe** — a **network-protocol-family-absent build** is the **primary** SSRF
  floor (the argv `-protocol_whitelist file,pipe` is defence-in-depth only — bypassable per
  CVE-2023-6605's pre-whitelist DASH dereference, §3.5.1) **plus** concat `-safe 1` (never
  `-safe 0`) + a curated demuxer set without the playlist/manifest dereferencing demuxers
  (absolute-file LFR half), asserted at §6.1.3 (`ffmpeg -protocols`/`-demuxers`, §3.5.1).
  Closes the HLS/DASH/concat SSRF & LFR class **structurally at build time** (the network
  family is unbuilt), not by the OS sandbox.
- **pandoc** — invoked with **`--sandbox`** (its built-in restriction that blocks
  reading/writing files and fetching network resources from the document) so a crafted MD/
  HTML cannot pull a remote image/include (§3.5.4).
- **LibreOffice** — the disposable `-env:UserInstallation` profile is hardened so
  document load does **not** auto-update remote/OLE links or external references (§3.5.2).
- **libvips / librsvg (SVG) — BOTH halves `[DECIDED]`:** the **primary, load-bearing
  control is loading the SVG via `rsvg::Loader` with NO base URL/`base_file`** — with no
  base URL librsvg has nothing to resolve a local/relative `href` against, so it refuses
  **all** local `<image href>`/XInclude reads by construction, and remote schemes are refused
  regardless. This closes **both** the SSRF half (no remote `href`/`<image>` fetch) **and**
  the absolute-file LFR half (no local out-of-input read) by construction (v1 SVG→raster
  needs no external resources; fonts are bundled). The image-worker calls librsvg directly
  for this (libvips `svgload` has **no** external-resource toggle). **No base-URL/scratch
  confinement is used** — supplying any base URL is exactly what re-enables the
  CVE-2023-38633-class resolution surface this control closes; the defence is the *absence*
  of a base URL. The **librsvg ≥ 2.56.3** pin (§6.1.3) is a belt-and-suspenders floor, not
  load-bearing for v1 (§3.5.5 SVG control). §6.1.3 corpus case asserts no out-of-input bytes
  are embedded.
§2.11 owns the *observable* "no network" property (packet monitor); §6.4 adds the
*adversarial* egress case; this section guarantees the *supply/structural* side. These
controls hold on the common v1 machine even when the §2.12 privilege-drop tier degrades
to the cheap tier (which by itself does not block a socket open).

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
- **H.264 / AVC** — patent pool (MPEG LA / now Via LA); the **bulk of the pool's US
  patents expire ~2027-11** (patent-term-adjusted), **but later-filed AVC-essential
  patents may run to ~2030** — so "months from free" applies to the bulk, not the entire
  pool. H.264 is **not yet free** at the v1 horizon either way. Same shape as AAC: the
  *engine* (x264/FFmpeg) is license-clean; the residual risk is a patent claim on
  distributing an H.264 encoder.
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
| **HEVC / H.265 — DECODE (image)** (read HEIC) | `images.md` HEIC source | **ship-bundled** (libheif+libde265, LGPL, decode-only; image-worker) | **ship-bundled** (libheif+libde265) | **ship-bundled** (libheif+libde265) |
| **HEVC / H.265 — DECODE (video)** (read iPhone HEVC `.mov`, HEVC-in-MKV) | `video.md` MOV/MKV HEVC source | **ship-bundled** (FFmpeg native `hevc` decoder, inside the GPL FFmpeg binary — **never** libde265) | **ship-bundled** (FFmpeg native `hevc`) | **ship-bundled** (FFmpeg native `hevc`) |
| **HEVC / H.265 — ENCODE** (write HEIC) | `images.md` HEIC **target** (never a default) | **ship-bundled (x265, isolated) `[DECIDED]`, behind §3.4 availability flag** | **ship-bundled (x265, isolated) `[DECIDED]`, behind flag** | **ship-bundled (x265, isolated) `[DECIDED]`, behind flag** |
| **AV1 — image (AVIF)** encode+decode | `images.md` AVIF | **ship-bundled** (libaom enc / dav1d dec via libheif, image-worker) | **ship-bundled** | **ship-bundled** |
| **AV1 — video** DECODE | `video.md` MKV/WEBM AV1 source (decode-only; AV1 is **not** a v1 WEBM-output codec) | **ship-bundled** (FFmpeg internal `av1`/`libdav1d` decoder, inside the GPL FFmpeg binary — **never** the image-worker's libheif/dav1d module) | **ship-bundled** (FFmpeg `av1`) | **ship-bundled** (FFmpeg `av1`) |
| **Legacy encumbered codecs — DECODE ONLY** (VC-1, MPEG-2, H.263, MPEG-4 Part 2 / DivX-class) | `video.md` WMV source (VC-1), MPG/MPEG source (MPEG-2), 3GP source (H.263), AVI source (MPEG-4 Part 2) — **read-side only** (these are never v1 encode targets) | **ship-bundled-decode-only** | **ship-bundled-decode-only** | **ship-bundled-decode-only** |

**Image-decoder vs video-decoder split — never conflate the two engines `[DECIDED]`.**
HEVC and AV1 each have **two distinct decoders** in the build, in **different
processes**, and the matrix rows above are split accordingly:
- **Image path** (`images.md` HEIC / AVIF source) decodes in the **image-worker** via
  **libheif → libde265** (HEVC) and **libheif → dav1d** (AV1). These LGPL/BSD modules
  decode *still images only*.
- **Video path** (`video.md` HEVC-in-MOV/MKV, AV1-in-MKV/WEBM source) decodes inside
  the **GPL FFmpeg binary** via FFmpeg's **own native `hevc`/`av1` decoders** (FFmpeg
  does **not** link libde265, and its AV1 decode is its internal `av1`/`libdav1d`, not
  the image-worker's libheif/dav1d module).
A Phase-3 engine/trim/licence decision must read the **right row**: image HEVC/AV1
decode = libheif+libde265/dav1d (image-worker, §3.5.5); video HEVC/AV1 decode =
FFmpeg's internal decoders (FFmpeg sidecar, §3.5.1). The §6.1.3 curated-FFmpeg
decoder-coverage assertion must therefore list **`hevc` and `av1`** as required FFmpeg
decoders (they are the video-side decoders, not redundant with the image modules).

**Legacy decode-only codecs — ship-bundled-decode-only everywhere, no gate `[DECIDED]`.**
The §04 video matrices accept **WMV/MPG/MPEG/3GP/AVI sources**, whose inner bitstreams are
VC-1 / MPEG-2 / H.263 / MPEG-4 Part 2 — all encumbered, but ConvertIA only **decodes**
them (it re-encodes to the royalty-free/permitted default target, never *writes* these
codecs). Disposition: **ship-bundled-decode-only on all three platforms, no §3.4
availability flag**, because (a) **decode** has a materially lighter patent profile than
encode — the active pools target *encode/distribution*, not bitstream *decode*; (b)
**MPEG-2's US essential patents fully expired in 2018** (the last US essential patent,
US 7,334,248, expired Feb 2018 — verified against the MPEG-LA pool wind-down) and
VC-1/H.263 are near/past expiry over
v1's deadline-free lifetime; (c) the whole OSS ecosystem (FFmpeg in every Linux distro)
ships these decoders. These are inside the **GPL FFmpeg binary** (the §6.1.3 curated-decoder
assertion already lists `vc1`/`mpeg2video`/`h263`/`mpeg4` as required), so no extra
licence surface beyond FFmpeg's. This keeps §3.4's "single owner, never re-decided
elsewhere" claim honest — the legacy decoders now have an explicit disposition row.

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

**HEVC decode — ship-bundled everywhere `[DECIDED]`, via TWO engines.** Decoding HEIC
("open my iPhone photo") and reading HEVC-in-MOV are core everyday needs, but they run
in **different decoders**: the **image** HEIC source decodes in the image-worker via
**libheif+libde265** (LGPL, decode-only — no x265, no GPL-encode patent-heavy path),
while the **video** HEVC-in-MOV/MKV source decodes in the **GPL FFmpeg binary** via
FFmpeg's **native `hevc` decoder** (FFmpeg does not link libde265). Decode-only HEVC
has the lighter patent profile and is widely shipped either way. The `images.md`
`HEIC→JPG★` default-source path (libde265) and the `video.md` HEVC source path
(FFmpeg `hevc`) must both work everywhere; the §6.1.3 curated-FFmpeg assertion lists
`hevc` so the video decoder cannot be trimmed out.

**HEVC *encode* (writing HEIC) — `[DECIDED]`: ship-bundled-isolated (x265), behind
the §3.4 availability flag.** This is the highest-risk codec: x265 is **both GPL and
the most patent-encumbered** codec in the set, and HEIC-as-a-target is **never a
default** (`images.md`: "never a default… compatibility-poor on non-Apple"). The
decision (adopting the standing [REC]):
- **Ship-bundled x265 on all three platforms `[DECIDED]`** — so HEIC *output* exists
  everywhere, **isolated as a separately-invoked binary** per §3.6 (GPL never linked
  into the MIT core; only redistributable code ships), patent posture surfaced in
  NOTICE. The codec is **redistributable** (GPL, aggregation) so it meets the "ship only
  what is redistributable" constraint, consistent with the one-product promise.
  **Honest risk note:** HEVC-*encode* exposure is **materially HIGHER than AAC/H.264** —
  not the "same posture". HEVC has **27 000+ patents across multiple active pools that
  run well beyond 2027** (Access Advance / Via LA / MPEG LA-legacy), and at least one
  pool (Access Advance, libheif#591) asserts that *encoder* use at end-user request
  needs a licence. This is why HEIC-encode is the **most likely §3.4.4a flag-flip to
  `unavailable`** of any codec in the set; it is a never-default, low-demand target
  precisely so the flip is zero-blast-radius.
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

### 3.4.4a The §3.4 availability flag — concrete representation `[DECIDED]`

The "behind the §3.4 availability flag" escape hatch (HEVC-encode, SSOT exception-1)
is concretely:

- **Where it lives:** a **per-platform boolean `available` field on the codec's
  `engines.lock` row** — e.g. the x265-libheif-plugin row carries
  `available = { win = true, macos = true, linux = true }`. Flipping any platform to
  `false` is the **config change** (edit `engines.lock` + rebuild), **not** a code
  change. (Equivalently a Cargo feature could gate it; `engines.lock` is chosen so the
  flip is data, lives beside the SBOM, and the build-staging step can skip staging the
  plugin when `false`.)
- **How it propagates to the registry (the parse→map→capabilities flow) `[DECIDED]`:**
  the startup sequence (§7.2) **parses `engines.lock` once**, reads each codec row's
  per-platform `available` boolean for the **running** `Platform`, and **maps** it into a
  `PatentDisposition` value (`available == true → CodecPosture::Available`, `false →
  CodecPosture::Unavailable`) for each of `heic_hevc` / `aac` / `h264`. This resolved
  `PatentDisposition` is built **before** any `Engine::capabilities(platform, patents)`
  call and is passed into it; an `Unavailable` posture makes `capabilities()` omit (or
  mark unavailable) the gated capability, so the §3.2.3 `select()` returns `None` for the
  gated pair (HEIC-encode) → surfaced as `PlatformUnavailable` (§2.8). (So
  `engines.lock.available` is the **source** of `PatentDisposition`, not a separate truth:
  the boolean is parsed → mapped → handed to `capabilities()`; there is no second place
  the posture is decided.)
- **How it propagates to the UI (the load-bearing wiring):** C12 `get_engine_health`
  (§0.4.1 / §7.2.3) computes `EngineHealth.unavailable_targets: Vec<TargetId>` — it
  reads the **resolved `available` flag** (not only the build-time hash manifest): a
  target whose only encoder is an `available = false` codec is added to
  `unavailable_targets`. §5.2 reads `EngineHealth` and renders that target's tile
  **disabled-with-reason** ("HEIC isn't available on this system"). So flipping x265 to
  unavailable on a platform removes HEIC as an offered target there with no code change.
- HEIC-encode is **never a default** target (§3.4.4 / images.md), so a flip is a clean,
  zero-blast-radius drop of one never-default tile.

### 3.4.5 Per-platform packaging specifics (beyond the matrix)

| Aspect | Windows | macOS | Linux |
|---|---|---|---|
| Artifact | portable `.zip` (exe + bundled engine trees; no installer) — **NSIS NOT shipped v1 (§6.1.2 `[DECIDED-6.1a]`)** | `.app` (and/or `.dmg`) | AppImage / portable dir |
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

**Read-side only (write side is the core's, per §7.2.6) `[DECIDED]`:** this staging
covers the **INPUT** (engines never first-read a protected source). The **OUTPUT**
publish is *not* staged on the default beside-source path: the core writes the
§2.14.1 `out_tmp/.part` sibling dotfile **inside the destination dir** and performs
the §2.1 exclusive publish there. On the beside-source default that destination dir
is itself TCC-protected, so the **core** (never the engine) is the process that first
creates the `.part` — but a TCC denial on that beside-source write still **fails that
item** per §2.8 and the batch continues (per §7.2.6 fact 2, the absolute "engines
never first-touch a protected path" holds; "a TCC chain-break can never block a
conversion" is a **read-side** claim, not a write-side one).

### 3.5.1 FFmpeg / ffprobe (audio, video, cross-category)

- **Probe first (video only):** `ffprobe -v error -print_format json -show_streams
  -show_format <input>` → inner codecs / duration / rotation / interlace. The
  **duration** becomes the §1.11 progress denominator; the **inner codecs** drive
  `video.md`'s remux-vs-reencode decision (a §3.2 capability decision, executed
  here).
- **Progress:** `-progress pipe:1 -nostats` → key=value lines (`out_time_us=…`,
  `total_size=…`, `progress=continue|end`) parsed into `ProgressModel::
  FfmpegKeyValue { duration_us }` (§1.11). Real per-item %, never a spinner.
  **`duration_us` population — via the two-phase plan, not struct mutation `[DECIDED]`:**
  the `duration_us` denominator comes from the **`ProbeOutput`** §1.7 parses from
  `ffprobe`, carried into **`Engine::plan_encode(.., &probe)`** (§3.2.1/§3.2.2) which builds
  the encode `Invocation` with `ProgressModel::FfmpegKeyValue { duration_us: probe.duration_us }`
  already populated. There is **no placeholder-then-mutate**: `plan()` returns the probe
  invocation (no encode `progress` to mutate); the encode invocation is built **after** the
  probe with the real denominator in hand. (The earlier "`progress_model()` returns
  `duration_us: 0` and §1.7 sets `progress.duration_us` in place" mechanism is **removed**
  in favour of `plan_encode`.) Until the first encode `out_time_us` tick the §1.11 bar
  reads as `Spawning`/indeterminate-but-working; from the first tick onward it is a true %.
- **Global flags (all FFmpeg jobs):** `-nostdin -hide_banner -loglevel error -y`
  — `-y` is safe because the target is the **temp** path (§2.1), never the user
  file; `-nostdin` prevents the classic FFmpeg "consumes the parent's stdin" hang.
- **Engine-level network/protocol restriction `[DECIDED — always-on, cheap-tier]`.**
  The bundled GPL FFmpeg ships with the full default protocol set, so a crafted dropped
  file (HLS/`.m3u8`, `-f concat` script, DASH manifest, external-reference box) can make
  FFmpeg open an outbound socket or read an arbitrary local file at convert time — the
  SSRF/LFR class (e.g. CVE-2023-6605 DASH-playlist SSRF). This is the **T9b** vector
  (§0.11) and would defeat the SSOT *Local/private/offline* promise on adversarial input.
  Mitigation is **argv + build controls** covering **both halves** (network/SSRF AND
  absolute-file LFR), all independent of the §2.12 OS privilege-drop tier (which is
  **best-effort `[DECIDED]`** and may degrade to the cheap tier with no network/FS deny —
  so it is **not** relied on here):
  - **Build-time is the PRIMARY structural SSRF floor (NOT the argv whitelist) `[DECIDED]`:**
    the argv `-protocol_whitelist` is **bypassable** — CVE-2023-6605 (cited above) shows the
    **DASH demuxer dereferences manifest URLs BEFORE the protocol whitelist is applied**, so
    a whitelist-only defence is not airtight. Therefore the **load-bearing SSRF floor is the
    curated build**: the network protocol family
    (`http`/`https`/`tcp`/`tls`/`rtmp`/`rtsp`/`hls`-fetch/`srtp`) MUST be **absent at
    configure time** (`--disable-network` / no `--enable-protocol=` for the network family),
    asserted by the §6.1.3 **`ffmpeg -protocols`** build check (the network family MUST NOT
    be built in), **plus** the dereferencing demuxers (DASH/HLS-fetch/external-reference
    playlist) absent per the `ffmpeg -demuxers` assertion below. With the network family
    unbuilt, even a pre-whitelist demuxer dereference has no network transport to use.
  - **Argv-level — network/SSRF half (defence-in-depth on top of the build floor):** every
    FFmpeg/ffprobe invocation additionally prepends **`-protocol_whitelist file,pipe`** (and,
    where a concat/segment demuxer is legitimately used, the explicit `-f` is pinned and the
    whitelist is **not** widened to network schemes). This is set **before each input** (the
    option is per-demuxer). It is **defence-in-depth** (catches anything the build trim
    missed), **not** the structural floor — the build-time absence of the network protocol
    family is.
  - **Argv-level — absolute-file LFR half (the part `file,pipe` does NOT cover) `[DECIDED]`:**
    `-protocol_whitelist file,pipe` MUST keep `file:` enabled (the input *is* a file), so a
    crafted playlist/manifest/concat-script could otherwise dereference an arbitrary
    **absolute** local file (`file:///etc/passwd`) or a `..`-traversal. This half is closed
    structurally by **two argv/build controls, not the OS sandbox**:
    - **`-safe 1` on the concat demuxer (NEVER `-safe 0`):** `-safe 1` is FFmpeg's default
      and **rejects absolute paths and `..`-traversal** in a concat script (only portable
      relative names are accepted) — ConvertIA never passes `-safe 0`, so a crafted
      `-f concat` script cannot read out-of-input absolute files. (Verified against the
      FFmpeg concat-demuxer docs: a path is "safe" only if it has no protocol spec, is
      relative, and uses the portable charset; `-safe 0` is the only way to lift this, and
      we never set it.)
    - **dereferencing demuxers constrained/absent in the curated build:** the playlist/
      manifest demuxers that can open *other* files (local-HLS `.m3u8`, DASH `.mpd`,
      `image2` glob/pattern, external-reference EXTF/`dash`) are either **not enabled** in
      the §6.1.3 `--disable-everything --enable-…` curated build (none is needed for any §04
      pair — ConvertIA converts single self-contained media files, never playlists) **or**
      invoked only with their non-dereferencing options. A §6.1.3 `ffmpeg -demuxers` build
      assertion verifies the playlist/segment demuxers ConvertIA does not need are absent.
  - **Build-time (network protocol family absent — the primary control):** the curated
    FFmpeg build **MUST omit the entire network protocol family** at configure time
    (`--disable-network` preferred wholesale; at minimum no `--enable-protocol=` for
    `http`/`https`/`tcp`/`tls`/`rtmp`/`rtsp`/`hls`-fetch/`srtp`), asserted by a **§6.1.3
    `ffmpeg -protocols` build assertion** (the network family MUST be absent / not built in).
    **No §04 pair needs a network protocol** (ConvertIA converts single self-contained local
    files), so `--disable-network` should apply wholesale; **if a future build ever cannot
    set `--disable-network` wholesale** (a needed demuxer pulls it in), the exact demuxers
    requiring it MUST be listed here and the `ffmpeg -protocols` assertion MUST still prove
    the **network protocol family is unbuilt** (the demuxer may exist; the network transport
    it would call must not).
  Together with the §6.4.2 adversarial-egress case (a network-trigger input must
  show **zero egress AND no out-of-input file read**), this backs the §3.3.4 "nothing
  fetches" claim **structurally on BOTH halves** (SSRF via the network-family-absent build,
  with the whitelist as defence-in-depth; absolute-file LFR via `-safe 1` + the curated
  demuxer set) — it does **not** rely on the degradable OS network-deny / §2.12.3
  FS-restriction tier. The §2.12.3 privilege-drop tier remains defence-in-depth, no longer
  load-bearing for T9b-LFR.
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
  (`video.md` `[DEFER: corpus]` — default-on for flagged-interlaced); rotation honored.
  **Mixed remux/re-encode in one
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
  - DOC→markup (the LO-owned down-conversions — `[DECIDED]` LibreOffice, documents.md
    item 2): `Text` / `HTML (StarWriter)` / `Markdown`† (LO 26.2).
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
- **Profile hardening `[DECIDED — concrete mechanism behind "macros never executed"]`.**
  `--headless --convert-to` does **not** by itself disable macros, and document-event
  /`AutoOpen` macros plus remote/OLE link auto-update can fire on *load*. The enforcement
  is the disposable `-env:UserInstallation` profile, **pre-seeded with a
  `registrymodifications.xcu`** that pins (T1 DOCX-macro vector, §0.11):
  - **Macro security at the highest level** — `org.openoffice.Office.Common/Security/
    Scripting/MacroSecurityLevel = 3` (disable all without notification) + `DisableMacrosExecution
    = true`; no Basic IDE. Macros are never run, no prompt blocks the headless process.
  - **No link auto-update on load** — `…/Filter/…`/document `LinkUpdateMode = 0` (never
    update links when loading) so external-reference / DDE / remote-OLE links don't trigger
    a load-time fetch or file read (composes with the §3.3.4 offline claim).
  - **No remote/OLE auto-fetch** — external-reference auto-update disabled; combined with
    the offline floor, a crafted office file cannot pull a remote target on load.
  - **Calc external-data vectors (T9b, `[DECIDED]`)** — a crafted spreadsheet can carry
    `WEBSERVICE()`/`WEBSERVICE`-class functions, **external data ranges** (web/import
    ranges that refresh on load), **external cell references** to another workbook, and
    **linked OLE objects** — each a potential load-time SSRF/LFR. The profile additionally
    pins, best-effort: **no external-data-range refresh on load**, **no external-reference
    recalculation on load** (`org.openoffice.Office.Calc/.../Load` external-reference update
    off), and **linked-object / DDE auto-update off** (composing with `LinkUpdateMode = 0`
    above). **Proof-parity with FFmpeg/pandoc `[DECIDED]`:** the registry pins are
    **defence-in-depth**, not the load-bearing proof — the office-engine T9b half **leans on
    the §2.11.4 packet-monitor gate + the §6.4.2 adversarial-egress Calc case** (a crafted
    `.xlsx` with a `WEBSERVICE`/external-data-range trigger must produce **zero egress AND
    no out-of-input file read**) as its release-blocking proof, exactly as the FFmpeg
    `-protocols`/`-demuxers` and pandoc `--sandbox` controls are corpus-proven. So Calc gets
    the same proof level as the other engines even where a registry key is only best-effort.
  The profile is disposable per-run (§2.14) and torn down with the run (§2.6).
- **Licence/isolation:** MPL-2.0 sidecar (§3.6); untrusted office files (zip-bomb,
  malformed OOXML, macro-bearing) parsed inside §2.12; **macros never executed**
  (the profile-hardening above + the `04` "macros dropped" policy).

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
  `MD` read dialect `-f gfm`; `*→MD` `-t gfm`; `[DEFER: corpus]` image policy
  (leans drop-with-note vs data-URI) owned by documents.md.
- **Reader limits honored (not re-decided):** pandoc **cannot** read legacy binary
  `.doc` and has gaps reading RTF → those down-conversions are **not** assigned to
  pandoc (documents.md reassigns them to LibreOffice); the registry (§3.2) never
  hands pandoc a `.doc`.
- **Network / file-read restriction `[DECIDED — always-on `--sandbox`]`:** every pandoc
  invocation runs with **`--sandbox`** (pandoc ≥2.15), which confines readers/writers to
  the file(s) named on the command line and **blocks all network access and file-system
  reads** from the document. This is the cheap-tier, OS-sandbox-independent control behind
  the §3.3.4 "pandoc fetches nothing" claim: a crafted MD/HTML/RST/Org/LaTeX include or
  remote `<img>`/CSS cannot pull a remote or local out-of-input file (mitigates the LFR/
  SSRF class for the markup engine; RST/Org/LaTeX file-include directives are the named
  risk pandoc's own security note calls out). **Trade-off (consistent with documents.md):**
  because `--sandbox` also blocks *legitimate* local image reads, `*→HTML`
  `--embed-resources` can embed only resources pandoc can reach under the sandbox — so the
  documents.md `*→markup` image policy resolves to **drop-with-note** for out-of-input
  images, not silent local inlining. (`--sandbox` does not constrain PDF production or
  filters, but ConvertIA uses **neither** with pandoc — `*→PDF` is LibreOffice-owned and no
  pandoc Lua/JSON filters are configured — so the documented `--sandbox` gaps do not apply.)
  **`[DEFER: corpus]` data-file check:** confirm the actual pairs ConvertIA assigns pandoc
  (markup↔markup, `*→HTML --standalone --embed-resources`) all **run cleanly under
  `--sandbox`** on the §6.4 corpus — i.e. none needs an on-disk pandoc **data file**
  (templates, reference docs, syntax-highlight definitions) that `--sandbox` would block.
  If a chosen pair turns out to require a blocked data file, the fix is to **bundle that
  data file and pass it explicitly on the argv** (so it is a named input the sandbox
  permits), never to drop `--sandbox`.
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
  `images.md` feeds §1.10.) **Packaged artifact `[DECIDED]`:** the worker ships as a
  concrete **`externalBin` sidecar `convertia-imgworker-<triple>[.exe]`** (named in the
  §0.7 `binaries/` tree and the §0.3 subprocess box), **resolved Rust-side via
  `current_exe().parent()`** (the §3.3.3 [DECIDED] sidecar-resolution path — Tauri strips
  the triple suffix on bundle), **never linked into the MIT core**. So `EngineProgram::
  Sidecar(EngineId::ImageCore)` resolves to this artifact; Phase-3 builds it as its own
  binary that statically links the libvips/libheif/libde265/librsvg/ImageMagick stack.
- **Operation map (from `images.md`):** load (by detected type, **not** extension)
  → optional auto-rotate (EXIF orientation baked, tag reset to 1) → optional
  alpha-flatten (white bg for JPG/BMP) → save with the per-target saver and its
  params: `jpegsave Q=82 …`, `pngsave compression=6`, `webpsave Q=80`,
  `tiffsave compression=deflate`, **`gifsave` (native cgif backend, vips ≥ 8.12)**,
  **`magicksave` for BMP save (REQUIRED — libvips has no native `bmpsave`)**, and
  `magickload` for BMP load; **ICO save `[DEFER: corpus/build spike]`** — the **default
  path is `magicksave`** (libvips has no native ICO saver), BUT ImageMagick's ICO encoder
  has documented limitations with **256px / multi-size** entries and libvips' magicksave is
  not documented to support `.ico` save, so the multi-size-ICO-incl-256px-embedded-PNG
  capability is **unverified** until the §6.1.3 build spike confirms the bundled
  libvips+ImageMagick can write a valid `[16,32,48,256]` `.ico`. **Named fallback if the
  spike fails:** an **in-core Rust ICO container assembler** that wraps vips-produced
  per-size PNG/BMP frames into the ICO container (ICO is a trivial header + ICONDIR +
  per-entry image-data layout — assembling it in safe Rust removes ImageMagick from the ICO
  path entirely; the per-size frames are still vips-encoded). ImageMagick is *also* a GIF
  fallback only (§3.6.1: ImageMagick is permissive, cgif is MIT),
  `heifsave compression=hevc Q=…` (HEIC, via the **x265 libheif plugin**) /
  `heifsave compression=av1 Q=…` (AVIF, via **libaom** — the single-engine
  `HEIC↔AVIF` path; **all** HEIC/AVIF *encode* is `heifsave`, no standalone
  `heif`/`avif` encoder), ICO multi-size list. ICC/metadata carried per `images.md`
  policy.
- **x265 libheif-plugin runtime discovery in the portable bundle `[DECIDED]`:** the x265
  HEVC encoder ships as a **dynamically-loaded libheif plugin** under `resources` (§3.6.1),
  but the statically-linked libheif inside `convertia-imgworker` must find it at an
  **arbitrary extracted path**, and the §3.5 minimal-env policy strips loader/injection vars
  (so we cannot rely on an inherited `LIBHEIF_PLUGIN_PATH`). libheif loads plugins from the
  colon-separated (semicolon-separated on Windows) **`LIBHEIF_PLUGIN_PATH`**, else a
  compile-time `PLUGIN_DIRECTORY`, else a **programmatic add-plugin-directory API** (verified
  vs libheif's plugin-loading docs). **v1 mechanism:** the worker resolves its plugin dir
  **relative to `current_exe()`** (e.g. `<exe_dir>/resources/heif-plugins/`) and **points
  libheif at it explicitly** — either by **whitelisting that ONE var** (`LIBHEIF_PLUGIN_PATH`
  = the resolved absolute dir) in the otherwise-minimal env **before the first `heifsave
  compression=hevc`**, OR (preferred, env-free) via libheif's **explicit
  add-plugin-directory / load-plugin API** so no env var is needed at all. Without this,
  `heifsave compression=hevc` would fail at runtime even though the §3.4.4a availability flag
  reports HEIC "available". The §6.1.3 HEIC capability assertion exercises an actual HEVC
  encode so a mis-resolved plugin dir fails the build, not first use.
- **SVG external-resource control (T9b absolute-file LFR + SSRF, §0.11) `[DECIDED]`:**
  librsvg loads resources referenced from an SVG (`<image xlink:href>`, XInclude). It
  resolves a referenced `file:`/relative `href` **only** when it has a **base URL/base file**
  to resolve it against; remote schemes (`http`/`ftp`/…) are **always** refused regardless.
  Crucially, librsvg's documented model is that **when an SVG is loaded from in-memory bytes
  or a stream with NO `base_file` set, referenced local files are refused by construction** —
  "buffer and stream-based SVG input is unaffected" by the CVE-2023-38633 LFR class precisely
  *because* there is no base URL to resolve a relative/absolute `href` against. Conversely,
  **supplying a base URL/base file is exactly what RE-ENABLES** local/relative resource
  resolution. On Win/Linux the image-worker is normally handed the **real source path**, so
  loading the SVG *with* its source directory as base URL would let a crafted SVG with
  `<image href="../secret.txt">` read an **out-of-input local file** and rasterise it into
  the output — the same absolute-file LFR class the spec closes for FFmpeg. ConvertIA closes
  it **structurally and symmetrically**, with the **PRIMARY load-bearing control being "load
  the SVG with NO base URL", NOT directory-confinement** (which a parser-disagreement bug has
  historically bypassed):
  1. **PRIMARY (load-bearing) — load the SVG via `rsvg::Loader` with NO `base_file` set
     `[DECIDED]`.** The image-worker reads the SVG bytes into memory and loads them via
     `rsvg::Loader` (`read_stream`/`from_data`) **without** setting a `base_file`/base URL.
     With no base URL, librsvg has nothing to resolve a `file:`/relative `href` against, so
     **every** local `<image href>`/XInclude reference is **refused by construction**, and
     remote schemes are refused regardless — closing **both** the SSRF half (no
     `http`/`ftp` fetch) **and** the absolute-file LFR half (no local out-of-input read).
     The image-worker calls **`rsvg::Loader` directly for SVG load** rather than libvips
     `svgload`, because libvips `svgload` exposes **no** external-resource toggle (its only
     coarse lever is the `VIPS_BLOCK_UNTRUSTED` env var); calling librsvg directly lets us
     guarantee the no-base-URL path. This is the load-bearing control because **v1
     SVG→raster needs no external `<image>`/XInclude** (fonts resolve from the **bundled**
     set, §images.md), so refusing them costs nothing and removes the entire LFR/SSRF
     surface by construction — there is no base URL, hence no resolution step to subvert.
     **No base-URL/scratch-confinement step is used in v1**, because supplying *any* base URL
     would re-open the exact CVE-2023-38633-class resolution surface this control exists to
     close (per librsvg's own model, a base URL is what re-enables local resolution); the
     defence is **the absence of a base URL**, not the confinement of one.
  2. **Version pin (belt-and-suspenders) `[DECIDED]`:** **librsvg is pinned `>= 2.56.3` in
     `engines.lock`** (the CVE-2023-38633 fix floor), with a **§6.1.3 version assertion**
     that fails the build if the staged librsvg is older. This is **not** load-bearing for
     v1 (control 1 sets no base URL, so the base-URL parser-disagreement bug is never
     reached) — it is a belt-and-suspenders floor so that *if* a future version ever needs a
     base URL, it is not a known-bypassed librsvg. **If** a base URL is ever genuinely
     required in a later version, then **base-URL confinement becomes the load-bearing
     control and must be honestly labelled as such** (carrying the residual CVE-2023-38633
     parser-disagreement risk, mitigated only by this version floor) — it must **never** be
     demoted under a "refuse all" that no longer holds.

  Asserted by a **§6.1.3 corpus case**: an SVG with an external `<image href>` (relative
  `../` escape AND absolute) must **NOT** embed any out-of-input bytes in the output (the
  SVG analogue of the §6.4.2 FFmpeg adversarial-egress case) — and with the SVG loaded with
  no base URL (control 1) the reference must simply not resolve at all. **§6.1.3 also
  asserts** the pinned `librsvg` crate/version exposes the relied-upon
  `rsvg::Loader::read_stream`/`from_data`-without-`base_file` path. Cross-ref §3.3.4 offline
  invariant, §0.11 T9b, §2.11.1.
- **Progress `[DECIDED]`:** `ProgressModel::VipsStdout`. The image-worker is a **separate
  process** (§3.5.5/§0.7), so its in-process libvips **eval-progress callback** cannot
  reach the core directly. The worker installs the libvips `eval` signal handler and
  **marshals each tick to its own stdout as a `progress=<0..100>` key=value line**
  (optionally `progress=end` on completion) — exactly the cross-process wire mechanism
  FFmpeg uses (`-progress pipe:1` key=value). The §1.7 invocation layer's **same**
  line-by-line stdout reader parses these into normalised `ItemProgress` ticks. (For ops
  that are reliably sub-second the worker may simply emit start→`progress=end`, equivalent
  to `CoarseSpawnDone`; HEIC/AVIF HEVC/AV1 encode is the case where a real % matters.)
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
  (linked **inside this separate worker binary** — static-link-as-aggregation OK, with
  the §6.1.3 carve-out ii relinkable-source bundle; never linked into the MIT core);
  aom/dav1d = BSD; **ImageMagick = permissive
  (ImageMagick License, Apache-2.0-style — link-OK, NOT GPL) and REQUIRED for BMP (BMP
  save goes only through it; the ICO-save path is the `magicksave` default but
  `[DEFER: build spike]` §3.5.5, with the in-core Rust ICO assembler as the fallback that
  would drop ImageMagick from the ICO path; §3.1 row 1d).** The **only** GPL piece in the image stack
  is **x265** (HEVC encode), the aggregation case (§3.6) — shipped as a
  **dynamically-loaded libheif encoder plugin** (`ENABLE_PLUGIN_LOADING`), never
  statically linked into the image-worker's libvips or the MIT core (see §3.6 for the
  exact line). (Build caveat: exclude any GPL ImageMagick *optional delegates*; IM
  core is permissive.)
- **ImageMagick is a delegate, NOT a registry engine `[DECIDED]`:** ImageMagick is a
  **bundled delegate called inside the image-worker** via libvips `magicksave`/
  `magickload` — no `(source,target)` pair maps to `EngineId::ImageMagick` (BMP/ICO
  route through `EngineId::ImageCore` = the image-worker). It has **no `EngineProgram`**,
  **no §3.2.3 registry entry**, and **no `trait Engine` impl**; its `EngineId` exists
  **only** for SBOM/NOTICE attribution (§3.7) and the §7.2 EngineHealth presence-check.
  (Stated so Phase-3 does not author a spurious `Engine` impl / registry row for it.)

### 3.5.6 Native CSV/TSV engine (in-core Rust)

- No subprocess; a single streamed pass: detect encoding/delimiter (spreadsheets.md
  policy) → re-encode to UTF-8 (no BOM default) → swap delimiter → RFC-4180
  re-quote where a field contains the new delimiter/quote/newline → write to
  `out_tmp`. CSV-injection-safe (leading `= + - @` stay literal text). **Progress:**
  because it streams the file, it **self-reports `bytes_processed / source_size`** per
  N-KB chunk (the §1.11 *Native CSV/TSV* row owns this), falling back to a start→done
  (`CoarseSpawnDone`-equivalent) tick for sub-100 KB inputs — never a bare spinner. MIT
  (own code) — no §3.6 concern.
- **`out_tmp` is the §2.14.1 destination-dir publish temp, NOT a system-temp file
  `[DECIDED]`.** Like every other engine the native CSV/TSV `out_tmp` is the
  `.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part` **sibling in the destination
  directory** (`tempfile::NamedTempFile::new_in(final_dir)` / a `TempPath` rooted in
  `final_dir`), picked by `crate::run` (§2.14.4) — **never** `tempfile::NamedTempFile::new()`
  in the system temp dir. So its publish is the **same intra-volume exclusive rename**
  (§2.1.2) as every other engine, honouring the §2.14.1 same-volume invariant. The
  `TempPath` "deleted on drop" semantics apply only on the **cancel/fail** path; **on
  success the temp is consumed by the §2.1 atomic publish** (rename/link), so drop is a
  no-op — the output is **published, not dropped** (§2.6.2).

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
| **libvips** + **librsvg** | **`LGPL-2.1-or-later`** (both) | **NO — inside the separate image-worker process** (§3.5.5), where they may be **statically** linked (aggregation, not a link into the MIT core); never linked into the MIT core | LGPL §6 satisfied by aggregation (separate process) **+** the relinkable-source bundle the static image-worker ships (§6.1.3 carve-out ii / §3.6.2); we ship the LGPL libs + their source/offer (§3.7). **No LGPL is linked into the MIT core** (where one ever is, it must be a shared object — §6.1.3 carve-out i) |
| **libheif** + **libde265** | **`LGPL-3.0-or-later`** (both) | NO — inside the image-worker process (§3.5.5), static-link-as-aggregation OK | as above (aggregation + relinkable-source bundle, §6.1.3 carve-out ii). **Per-component SPDX ids are split out** (libvips/librsvg = `LGPL-2.1-or-later`; libheif/libde265 = `LGPL-3.0-or-later`) so §3.7.2 emits the **correct distinct SBOM rows** rather than a lumped "LGPL-2.1/3.0" — the LGPL-3.0 host (libheif) is also why the x265 plugin must be `GPL-2.0-or-later` (upgradeable to GPLv3), §3.7.2 |
| **libaom / dav1d** | libaom `BSD-2-Clause AND LicenseRef-AOMPL-1.0`; dav1d `BSD-2-Clause` | link OK | BSD permissive; libaom's `PATENTS` (AOM Patent License 1.0, no registered SPDX id → `LicenseRef-AOMPL-1.0`, §6.3.3 carve-out) is carried in the SBOM/NOTICE alongside the BSD-2 text (§3.7) |
| **libimagequant** (PNG/GIF palette quantisation) — **BSD-2-Clause `lovell/libimagequant` v2.4.x fork ONLY** | **BSD-2-Clause** (the frozen `lovell/libimagequant` v2.4.x fork). **Upstream 4.x is GPLv3-or-commercial and MUST NOT ship** — if a GPL-leg 4.x build slipped in it would taint the LGPL image-worker (the §6.1.3/§6.3.3 COPYRIGHT-text assertion fails the build on that). | link OK (inside the image-worker) | BSD permissive; **the v2.4.x BSD fork** vendored/linked inside the image-worker process, not the MIT core |
| **ImageMagick** (GIF/BMP/ICO save delegate) | **ImageMagick License** (Apache-2.0-style, SPDX `ImageMagick`) — **permissive, NOT GPL** | link OK | Permissive like BSD/MPL — no isolation needed. **Build caveat:** exclude GPL *optional delegates*; IM core is permissive. (Listed in the SBOM/NOTICE §3.7.) |
| **x265** (HEVC encode) | **GPL-2.0-or-later** | **NO — dynamically-loaded libheif *plugin*** | x265 ships as a **separately-built, dynamically-loaded libheif encoder plugin** (`.so`/`.dll`/`.dylib`, libheif `ENABLE_PLUGIN_LOADING`) that `heifsave compression=hevc` loads at runtime. The GPL code is **never statically linked** into the image-worker's libvips or the MIT core; it lives behind libheif's plugin ABI and runs **inside the §0.7 image-worker process** (already a separate process from the core). **Accurate framing `[DECIDED]`: when x265 is loaded, the running image-worker is a GPL *combined work*** (per the FSF, dynamically loading a GPL plugin into a process makes that process's combination a GPL combined work — it is **not** an "LGPL worker with an isolated GPL plugin"). **The aggregation argument that keeps the MIT CORE clean is the *separate process* boundary** (the core invokes the worker as a child process), and that is sound + load-bearing — but **inside** the worker, both the **LGPL relink obligation** (libvips/libheif stack) **AND** the **x265 GPL corresponding-source obligation** apply to the worker-with-x265-loaded. *(A static x265-in-libvips link would taint — hence the plugin form. This replaces the dropped "standalone heif/x265 sidecar" — no such sidecar exists under the [OPEN-1] heifsave-only decision.)* |
| **x264** (H.264 encode) | **GPL-2.0-or-later** (SPDX `GPL-2.0-or-later` — matches x265's form; x264 is GPL-2.0-**or-later**, not `GPL-2.0-only`) | **NO — inside the GPL FFmpeg binary** | reached only via the **FFmpeg binary** (separate invoked process); never linked into the MIT core |
| **FFmpeg** build | **GPL-2.0+** (enables GPL x264 via `--enable-gpl` → the *whole* binary is GPL-2.0+, not LGPL) | **NO — separate exe** | invoked as `ffmpeg`/`ffprobe` child processes (§3.3.3); aggregation keeps the MIT core clean. The LGPL component libs (libmp3lame etc.) are **dynamically linked beside the exe** (§3.9.1) per LGPL §6 — a static FFmpeg build would fail the §6.1.3 dynamic-link assertion. Written-offer-of-source obligation honored (§3.6.2). |
| **LibreOffice** | MPL-2.0 | **NO — separate sidecar** | invoked `soffice` process; MPL is weak/file-level anyway, but isolation is belt-and-suspenders + the SSOT policy |
| **poppler**, **pandoc** | GPL | **NO — separate exe** | invoked child processes |
| **Ghostscript** | **AGPL-3.0** | **NOT shipped v1 [DECIDED]** | dropped (§3.1) so no AGPL surface ships; `[DEFER: re-add only if §6.5 corpus shows GS-salvageable PDFs]` |

**LGPL link build rule (a buildability gate, not just an assertion) — scoped by
linkage site `[DECIDED]`.** LGPL §6 compliance depends on **where** each LGPL lib is
linked, and the build rule (asserted by §6.1.3, carve-outs i/ii/iii) reflects that:
- **Into the MIT core (the Tauri app binary):** any LGPL lib linked here **MUST be a
  bundled *shared* library** (`.so`/`.dylib`/`.dll`), dynamically linked — Rust links
  **statically by default**, so a vendored *static* LGPL absorbed into the MIT binary
  would silently break LGPL §6 and is a **build failure** (§6.1.3 carve-out i). In v1
  ConvertIA links **no** LGPL into the MIT core (the whole image stack lives in the
  separate worker), so this carve-out is a guard against regression.
- **Inside the separate image-worker process (libvips + libheif/libde265/librsvg, and
  any linked FFmpeg libs the worker pulls):** the worker is its **own binary** (§3.5.5),
  so even a **statically** linked LGPL inside it is **aggregation, not a link into the MIT
  core** — this is the **canonical v1 mechanism** (§3.5.5 builds the worker statically
  linking the stack), not a fallback. LGPL §6 relinkability is satisfied here by the
  **relinkable-source bundle** the static worker ships (complete corresponding source +
  LGPL object files / a documented relink recipe), which §6.1.3 carve-out ii **asserts is
  present and fails the build if missing** (§3.6.2 written-offer + §3.7 SBOM record the
  pinned source). So the worker does **not** need its LGPL libs as separate shared objects.
  **The relinkable-source/written-offer bundle MUST cover x265 too `[DECIDED]`:** because
  the worker-with-x265-loaded is a **GPL combined work** (x265 row above), the bundle's
  corresponding-source obligation extends to **x265 as the GPL component of the worker**
  (the GPL §3 complete-corresponding-source for x265, not only the LGPL stack's source) —
  §6.1.3 carve-out ii asserts the pinned **x265** source + offer is present alongside the
  LGPL source, and §3.6.2/§3.7 record it. The *separate-process* boundary keeps the MIT
  core clean; the *in-worker* obligations (LGPL relink + x265 GPL corresponding-source)
  are both satisfied by this one bundle.

**The same build rule forbids libvips' own copyleft PDF
loaders** (`[DECIDED]`): the bundled libvips is configured **without the poppler PDF
loader (GPL — taints the whole libvips, libvips#2222) and without the MuPDF loader
(AGPL)**, so "libvips is LGPL" stays true; §6.1.3 asserts no poppler/mupdf loader is
present (ConvertIA does no libvips PDF loading — PDF→TXT is the poppler `pdftotext`
sidecar, §3.5.3).

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
- **LGPL §6 — STATIC-LINK additionally requires a relink path `[DECIDED]`.** A source
  pointer alone does **not** discharge LGPL §6 for a **statically-linked** LGPL library.
  The image-worker statically links the LGPL stack (libvips + libheif + libde265 +
  librsvg — §3.6.1 aggregation-inside-the-worker), so for it ConvertIA **additionally
  ships the relinkable object files (the worker's own `.o`/archive + the LGPL libs as
  separately-relinkable units) AND a documented relink recipe** so a recipient can
  substitute a modified LGPL library and rebuild the worker (the §6.1.3 carve-out ii
  "relinkable-source bundle"). This relink bundle is a **release artifact** asserted by
  the §6.1.3 build (carve-out ii). (Where an LGPL lib is instead **dynamically** linked
  — §6.1.3 carve-out i — the user can already swap the shared object, so §6 is satisfied
  without the relink bundle.) The FFmpeg binary's internal static LGPL libs are the
  aggregation case (carve-out iii) covered by the source pointer above.
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
   class are declared in the **build manifest `engines.lock`** (in `src-tauri/`; the
   single canonical name used by §6.3.1/§6.3.2/§6.3.3/§6.8 — there is no `engines.toml`).
   This manifest is the authoritative input — **not** hand-curated prose, so it can't
   drift from what actually ships.
2. A build script (`cargo xtask sbom` / a Node build step, §6.1) reads the
   manifest + Rust crate licences (via `cargo about` / `cargo-cyclonedx`) +
   the bundled-engine entries → emits the **CycloneDX SBOM** and concatenates
   `THIRD-PARTY-LICENSES.txt` from each component's vendored `LICENSE`/`COPYING`.
3. The bundled fonts (§3.9) are **also** listed (their OFL/Apache licences).
4. **Every linked sub-component gets its own SBOM/`engines.lock` row**, not just the
   top-level engines — including the **FFmpeg binary** (SPDX `GPL-2.0-or-later`, with
   the written-offer-of-source line — it enables x264, §3.6.1), **ImageMagick**
   (SPDX `ImageMagick`, permissive, **REQUIRED** for BMP save; default ICO-save path,
   `[DEFER: build spike]` §3.5.5), **cgif** (MIT, the
   native `gifsave` backend §3.5.5), the **x265 libheif plugin** (SPDX
   **`GPL-2.0-or-later`** — verify against the pinned source's `COPYING`; GPL-2.0-only
   would be incompatible with the LGPL-3.0 libheif host, whereas -or-later is
   upgradeable to GPLv3 (what Debian ships) — with offer-of-source), the **libheif
   AV1-encoder dependency `libaom`** (`BSD-2-Clause AND LicenseRef-AOMPL-1.0` — the AOM
   Patent License has no registered SPDX id, so it ships as a `LicenseRef` custom
   licence with full text in `THIRD-PARTY-LICENSES.txt`, §6.3.3 carve-out),
   **dav1d** (`BSD-2-Clause`),
   **librsvg** (LGPL-2.1+ — the libvips `svgload` SVG backend), and
   **libimagequant** (the gifsave/cgif palette-quantisation dependency — SPDX
   **`BSD-2-Clause`**, shipped **only** as the frozen `lovell/libimagequant` **v2.4.x**
   fork, pinned by exact version+ref. **Upstream libimagequant 4.x is
   `GPL-3.0-or-later`-or-commercial — NOT permissive — and must NOT be bundled.** A
   §6.1.3/§6.3.3 build assertion verifies the staged `COPYRIGHT` actually contains the
   **BSD-2-Clause** text and **fails the build** if a GPL leg slipped in — so the SPDX
   id in `engines.lock` is corroborated by the shipped text, not trusted blindly).
   The §6.3.3 attribution-completeness gate fails if any shipped component lacks a row,
   so these must be enumerated or the release blocks.
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
| **pandoc** | **~80–220 MB** (version-dependent; pandoc 3.x) | Haskell static binary (notoriously large; the **GHC runtime dominates**, so stripping saves little) | a release/stripped build trims marginally. **pandoc CANNOT be dropped wholesale for v1** — it **owns the `DOCX/ODT/RTF → MD/HTML` markup pairs** that LibreOffice 26.2 Markdown export is **not validated for** (documents.md item 1/2 `[DEFER: corpus]`); dropping it would orphan those pairs. So this is at most a **post-v1 contingency** (re-evaluate once LO Markdown export is corpus-proven for those pairs), **not** a v1 trim knob. It is the **second-biggest single exe** after LibreOffice |
| **Ghostscript** **[DECIDED: NOT shipped v1]** | **0 MB** (~30–60 MB if ever re-added) | PDF repair backstop — dropped (§3.1) | already dropped; this whole row is saved |
| **ConvertIA Tauri app** (Rust core + WebView assets) | **~10–25 MB** | the app itself (Tauri's own footprint is small — the WebView is system-provided) | Tauri's whole point: tiny vs Electron |
| **WebView runtime** | **0 MB bundled** | system WebView2/WKWebView/WebKitGTK (never bundled, never downloaded — §3.4.5/§0.3.1) | n/a |

### 3.9.2 Total estimate & what dominates

- **Per-platform total: roughly ~430 MB–820 MB installed** (revised up for the
  pandoc 3.x ~80–220 MB figure), depending almost entirely on (a) the LibreOffice
  trim level, (b) the **bundled-font breadth** (CJK is the swing factor — Latin-only
  would be tens of MB; full CJK+RTL pushes toward the top of the range), (c) pandoc
  (**kept for v1** — it owns the `DOCX/ODT/RTF → MD/HTML` pairs LO Markdown export is
  unvalidated for, documents.md item 1/2 `[DEFER: corpus]`; dropping it is a **post-v1
  contingency only**, not a v1 trim lever), and
  (d) **Ghostscript** is **[DECIDED: dropped]** (saving ~30–60 MB; not in the total).
- **LibreOffice + fonts + pandoc together are ~80–90% of the bundle.** Image and
  PDF-text tooling are minor. Trimming effort, if any is spent, belongs there.
- **Compression:** the release artifact is compressed (platform-native: Windows
  `.zip`, dmg, AppImage squashfs) — download size is materially smaller than installed;
  exact ratios `[DEFER §6.1/§6.2]`. (NSIS is not a v1 artifact, §6.1.2 `[DECIDED-6.1a]`.)
- This is an **estimate for planning**, not a contract; the real numbers are
  measured in §6.1 once the trimmed engine builds exist, and fed back here.

#### Per-platform compressed-artifact size budget + CI enforcement `[DECIDED — "stay light", SSOT Principle 1]`
"Stay light" needs an owning number and a gate, not just an estimate:
- **Per-platform COMPRESSED artifact ceiling (the downloaded file):** ship a **finite
  starting budget of ≤ 400 MB compressed per platform** (the Windows `.zip`, `.dmg`,
  AppImage) as the v1 target — comfortably above the dominant LibreOffice+fonts+pandoc
  floor yet a real cap. **`[DEFER: corpus/build]`** the exact number is re-pinned once the
  trimmed engine builds + the chosen CJK-font breadth (§3.9.3) are measured in §6.1; the
  **design (a hard ceiling exists and is enforced) is DECIDED**, only the digit is empirical.
- **CI enforcement (the actionable gate):** the **§6.1.2 packaging step measures each
  platform artifact's compressed size and FAILS the build if it exceeds the budget** (a
  Lane-B gate, with the current measured sizes published as a release asset for
  transparency). Without this gate "stay light" has no teeth; with it, a regression that
  bloats the bundle (e.g. an un-trimmed engine, a full Noto CJK) blocks the release.
- **Corpus-size co-ownership:** the **LFS `corpus-large` total size is co-owned with
  §6.4.5** (the corpus asset), tracked separately from the *shipped artifact* budget —
  the corpus is a test asset, never shipped, so it does **not** count against this ceiling;
  this matches §6.4.5's `[DEFER: corpus]` total-size note.
- **Feasibility risk at the upper bound (must verify before the digit is fixed) `[DEFER:
  corpus/build]`.** The ≤ 400 MB **compressed** ceiling is comfortable against the *low*
  installed end (~430 MB → ~40-50% compression is routine), but at the **high installed end
  (~700-820 MB: full-CJK fonts + pandoc 3.x upper bound)** it needs **~50%+ compression**,
  achievable only with aggressive trim. The `[DEFER: corpus/build]` calibration **MUST
  verify this is actually reachable before treating 400 MB as a fixed gate** (measure the
  real trimmed-build compressed size in §6.1, both font-breadth extremes). **If the gate
  trips, the lever order is fixed `[DECIDED]`:** (1) **trim the CJK font weights first**
  (§3.9.3 — the single biggest swing knob, SC-only vs all-CJK); (2) only then revisit other
  font/help trims; (3) **dropping pandoc stays BLOCKED** until LibreOffice Markdown export
  is corpus-proven for the `DOCX/ODT/RTF → MD/HTML` pairs (documents.md item 1/2
  `[DEFER: corpus]`) — it is a post-v1 contingency, not a size lever to reach for. This
  ties the deferred digit to a
  **decided remedy** rather than a silent build-gate failure.

### 3.9.3 Open size decisions (genuine)

- **`[DECIDED]` Bundled-font baseline** (adopting the [REC]) — **Liberation +
  Carlito + Caladea** (metric-compat Arial/Calibri/Cambria/Times/Courier) **+ a
  curated Noto subset: Noto Sans/Serif CJK-SC/TC/JP/KR "Regular" weights + Noto Sans
  Arabic/Hebrew**. This is the single biggest fidelity lever for documents/
  spreadsheets/presentations (their font items — `documents.md` §5 `[DECIDED]`,
  `presentations.md` [OPEN-2] `[DECIDED]` — all resolve to this baseline). The **only residual is
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
| **HEVC *decode* ship-bundled all 3 platforms (two engines)** | `[DECIDED — recommended]` | §3.4 | **image** HEIC source → libheif+libde265 (LGPL, decode-only, image-worker); **video** HEVC-in-MOV/MKV → FFmpeg native `hevc` decoder (GPL FFmpeg binary, **never** libde265). §6.1.3 lists `hevc`+`av1` as required FFmpeg decoders |
| **HEVC *encode* (write HEIC) disposition** | **`[DECIDED]`** | §3.4 | ship-bundled-isolated (x265, GPL → separate invoked binary), **behind the §3.4 availability flag** so it can flip to `unavailable` (SSOT exception-1) as a config change. kvazaar (BSD) recorded as the license-clean alternative. |
| AVIF ship-bundled all 3 platforms | `[DECIDED]` | §3.4 | royalty-free |
| Drop Ghostscript in v1 | `[DECIDED]` (DEFER re-add to corpus) | §3.1 / §3.6 | poppler-only `PDF→TXT`, no AGPL surface; [DEFER: re-add only if the §6.5 corpus shows poppler failing PDFs GS would salvage] |
| **FFmpeg licence class = GPL-2.0+** (enables x264) | **`[DECIDED]`** | §3.1 / §3.6.1 | the whole FFmpeg binary is GPL-2.0+, not LGPL; separate invoked binary (aggregation); written-offer-of-source; LGPL component libs dynamically linked beside it |
| SBOM format = CycloneDX JSON; manifest-driven generation | `[recommended]` | §3.7 | feeds §6.3 release-blocking gate |
| HEIC/AVIF encode code-path | **`[DECIDED]`** | §3.5.5 / images.md [OPEN-1] | libvips `heifsave` (`compression=hevc|av1`) for all HEIC/AVIF encode; **one AV1 encoder (libaom)** ships; standalone heif/avif encoders dropped; x265 ships as a **dynamically-loaded libheif plugin** |
| GIF native; **BMP requires ImageMagick; ICO-save deferred** | BMP **`[DECIDED]`** / ICO **`[DEFER: build spike]`** | §3.5.5 / images.md / §6.1.3 | native `gifsave` (cgif, MIT); **BMP load+save go ONLY through the REQUIRED ImageMagick `magicksave`/`magickload` delegate** (libvips has no native BMP save at any version; ImageMagick is permissive, not GPL, and cannot be dropped). **ICO save** = default `magicksave` but the multi-size/256px capability is **unverified** — gated on the §6.1.3 build spike, with an **in-core Rust ICO container assembler** (wrapping vips frames) as the fallback that drops ImageMagick from the ICO path |
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
> via Context7). These ground the §3.4 recommendations; the former owner-level calls
> (HEVC-encode ship-posture, font set) are now `[DECIDED]` — HEVC-encode
> ship-bundled-isolated behind the §3.4 availability flag, font set the §3.9.3 baseline
> (only CJK breadth `[DEFER: size]`) — design-closed, with the patent exposure recorded
> as an honest grey area rather than an open design question.
