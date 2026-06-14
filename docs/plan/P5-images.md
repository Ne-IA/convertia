# P5 — Images (libvips family)

> **Goal:** full image-category coverage on the proven P4 harness — every image
> `(source → target)` pair (both directions) backed by §6.4.5 corpus files + a
> §6.4.3 per-pair integration test and marked **`reliable`** in the §6.5 pair-status
> ledger, with per-format advanced-option **declarations** registered against the
> P4-built options-panel shell, the per-engine §6.1.3 build assertions + §7.2.3
> availability rows + SBOM/NOTICE rows for the image stack, and the per-engine
> SSRF/LFR hardening (librsvg no-base-URL, ImageMagick coder lockdown). The §3.4
> patent matrix is **owned by P4**; P5 only **reads** the per-codec cell (HEVC for
> HEIC, AV1 ship-posture for AVIF).
>
> **Spec home:** [`04-formats/images.md`](../spec/04-formats/images.md) (every pair
> both directions, per-format advanced options, patent-gated paths, the ICO build
> spike), [`03-engines-and-bundling.md`](../spec/03-engines-and-bundling.md)
> (§3.5.5 the image-worker / librsvg no-base-URL, libheif/x265/libaom/dav1d/
> ImageMagick/cgif staging + the §6.1.3 per-engine assertions, §3.6/§3.7 isolation +
> SBOM), [`06-build-test-release.md`](../spec/06-build-test-release.md) (§6.4.3
> per-pair tests, §6.4.5 corpus, §6.5 reliability ledger).
> Box format: [`_format.md`](_format.md). Index: [README.md](README.md).
>
> **This is the v0 base** — the smallest atomic `[ ]` boxes below, grouped under
> `### ` sub-headings; a later adversarial-review pass deepens, splits and reconciles
> them (incl. P0's `→ activated in P5` / `needs: P5.x` cross-refs). Pairs are grouped
> by **engine code-path** (one saver / one load module / one SSRF path = one group),
> because a code-path is the smallest unit that is genuinely built once and then
> exercised across the pairs that share it; each pair still gets its own corpus
> backing, integration test and ledger row so no pair "hides" inside a group.

## Boundaries (read against P4)

- **P4 ↔ P5:** P4 built the **generic** harness — the image-worker boots + a
  round-trip succeeds through the §2.12 isolation boundary, the §6.4.3 per-pair
  runner + §6.5.2 ledger generator + §6.4.3a bijection guard exist, the
  options-panel **shell** + lossy-note + progress/cancel + result-actions + a11y
  chrome exist, the §3.4 patent matrix + its `engines.lock.available →
  PatentDisposition → EngineHealth.unavailable_targets` wiring exists, and the
  **generic** §6.1.3 assertion framework + SBOM/NOTICE tooling-scaffold + §7.2.3
  startup verifier exist. **P5 fills the image-specific variants:** the per-saver /
  per-codec pairs, the corpus, the per-pair tests, the option **declarations**
  (chrome already built), the per-engine §6.1.3 assertion **lists**, the per-engine
  §7.2.3 availability rows, the per-engine SBOM/NOTICE rows, and the librsvg /
  ImageMagick hardening. P5 must **not** re-implement the panel chrome, the runner,
  the ledger generator, the isolation wrapper, or the patent matrix.
- **Reads, never re-decides:** the §3.4 per-codec cell (HEVC HEIC encode ship-bundled
  behind the availability flag; AVIF ship-bundled everywhere), and the §3.5.5
  `[DECIDED]` engine bindings (all HEIC/AVIF encode via `heifsave`; dav1d for AV1
  decode; required ImageMagick for BMP).

---

### Engine staging & §6.1.3 build assertions (the image stack)

- [ ] **P5.1** [BUILD] Stage the libvips raster core into the image-worker (no copyleft PDF loader) · §3.1 §3.5.5 §6.1.3 · G37 G37b
  needs: P4.27
  > the bundled libvips configured **without** the poppler/PDF + MuPDF + any GPL/AGPL loader (so the worker stays LGPL-only, §3.1/§3.6.1); pinned by version+SHA-256 in `engines.lock` per the P0.7.3/P0.7.4 acquisition policy, staged by `scripts/stage-engines`, dynamic-dependency-closure asserted (G37b).
- [ ] **P5.2** [BUILD] Add the libvips no-copyleft-PDF-loader §6.1.3 positive build assertion · §6.1.3 §3.1 · G38
  needs: P5.1
  > the stage-step assertion that the staged libvips exposes **no** `pdfload`/`poppler`/`mupdf` foreign loader/symbols and **fails the build** if one is present (a distro libvips often enables poppler-glib PDF — libvips#2222) — the image-specific variant of the P4 generic §6.1.3 framework.
- [ ] **P5.3** [BUILD] Stage cgif + the lovell/libimagequant v2.4.x BSD fork for native gifsave + palette PNG · §3.1 §3.5.5 §6.1.3 · G37
  needs: P5.1
  > cgif (MIT) for native `gifsave`, plus the **BSD-2-Clause `lovell/libimagequant` v2.4.x fork ONLY** vendored/statically linked inside the cgif/palette path (never upstream 4.x GPLv3-or-commercial which would taint the LGPL worker, §3.1 row 1e); pinned by exact version+ref in `engines.lock`.
- [ ] **P5.4** [BUILD] Add the libimagequant BSD-2-Clause leg-text + lockfile-pin provenance §6.1.3 assertion · §3.1 §6.1.3 §6.3.3 · G38
  needs: P5.3
  > the stage-step assertion that the staged `libimagequant` `COPYRIGHT` contains the **BSD-2-Clause** text (the SPDX-presence gate sees a declared id, not shipped text, so the text check is the real guard) **and** the `engines.lock`/`Cargo.lock` ref is exactly the `lovell/libimagequant` v2.4.x-fork commit (provenance, not an ABI/soname check — it is statically vendored, §3.1 row 1e).
- [ ] **P5.5** [BUILD] Stage the REQUIRED ImageMagick delegate (permissive, GPL optional-delegates excluded) for BMP load/save · §3.1 §3.5.5 §6.1.3 · G37 G37b
  needs: P5.1
  > ImageMagick (the ImageMagick License, Apache-2.0-style, **not** GPL) staged as a libvips `magickload`/`magicksave` delegate **inside** the image-worker — **required** because libvips has no native BMP support; the trimmed build **excludes GPL optional delegates** (§3.6.1); its own `engines.lock`+SBOM row + dynamic-closure assert (G37b).
- [ ] **P5.6** [BUILD] Author the ImageMagick coder/delegate hardening (policy.xml OR coder-excluded build) + MAGICK_CONFIGURE_PATH wiring · §3.5.5 §0.11 · G38 G29
  needs: P5.5
  > the T9b/T1 load-bearing lockdown — **either (a)** a bundled hardened `policy.xml` denying `{URL,HTTPS,HTTP,FTP,EPHEMERAL,MVG,MSL,TEXT,LABEL,SHOW,WIN,PLT}` + `@`-indirect-read path-rights deny, with the worker setting **`MAGICK_CONFIGURE_PATH`** to the bundle policy dir in its minimal env (mandatory — without it MagickCore reads no/system policy); **or (b)** a trimmed IM built `--without-modules`/coder-excluded; the §6.1.3 assertion (P5.7) verifies whichever path was taken. The `MAGICK_CONFIGURE_PATH`-at-bundle-policy env construction is the §0.4.2 SAST-ruled (G29) path.
- [ ] **P5.7** [BUILD] Add the ImageMagick coder/policy §6.1.3 build assertion (parse policy.xml OR introspect -list coder/policy) · §3.5.5 §6.1.3 · G38
  needs: P5.6
  > the stage-step assertion that the chosen P5.6 path is in force — parse the staged `policy.xml` for the denied coder set, OR introspect `convert -list coder`/`-list policy` — and **fail the build** if a dangerous coder/delegate is enabled.
- [ ] **P5.8** [BUILD] Stage libheif + libde265 (HEVC decode) for HEIC read · §3.1 §3.5.5 §6.1.3 §3.4.3 · G37 G37b
  needs: P5.1
  > libheif (LGPL-3.0) + libde265 (LGPL-3.0) as the libvips HEIC **load** module (decode-only — §3.4.3 image HEVC-decode row is ship-bundled all platforms); pinned in `engines.lock`, dynamic-closure asserted; SPDX-expression validated (P5.64).
- [ ] **P5.9** [BUILD] Stage the x265 HEVC encoder as a dynamically-loaded libheif encoder plugin (GPL, never static-linked) · §3.1 §3.5.5 §3.6.1 §3.4.3 · G37 G38b
  needs: P5.8
  > x265 (GPL-2.0-or-later — verified vs the pinned source `COPYING`; `-or-later` is compatible with the LGPL-3.0 libheif host) shipped as a **dynamically-loaded libheif encoder plugin** `.so`/`.dll`/`.dylib` under `resources`, **never** statically linked into libvips or the MIT core (§3.6 aggregation); behind the §3.4.4a per-platform `available` flag (read in P5.32). G38b: the x265 GPL §3 corresponding-source bundle (P5.65).
- [ ] **P5.10** [BUILD] Wire x265-libheif-plugin runtime discovery in the portable bundle (LIBHEIF_PLUGIN_PATH whitelist OR add-plugin API) · §3.5.5 §6.1.3 · G38 G29
  needs: P5.9
  > the statically-linked libheif must find the plugin at an **arbitrary extracted path** while §3.5 strips loader/injection env vars — so the worker resolves `<exe_dir>/resources/heif-plugins/` relative to `current_exe()` and points libheif at it **either** by whitelisting the single `LIBHEIF_PLUGIN_PATH` var in the otherwise-minimal env **or** (preferred, env-free) via libheif's explicit add-plugin-directory/load-plugin API; the minimal-env construction is G29-ruled.
- [ ] **P5.11** [BUILD] Stage libaom (AV1 encode) + dav1d (AV1 decode) via libheif for AVIF · §3.1 §3.5.5 §6.1.3 §3.4.3 · G37 G37b
  needs: P5.8
  > libaom (`BSD-2-Clause AND LicenseRef-AOMPL-1.0` — both legs, §3.7) as the single bundled AV1 **encoder** via `heifsave compression=av1`; **dav1d** (`BSD-2-Clause`) configured as libheif's AV1 **decoder** (smaller/faster — "libaom is encode-only" is a build choice, not a libaom limitation); both pinned in `engines.lock`.
- [ ] **P5.12** [BUILD] Add the libheif-resolves-dav1d-for-AV1-decode §6.1.3 wiring assertion · §3.1 §3.5.5 §6.1.3 · G38
  needs: P5.11
  > the stage-step runtime-plugin-enumeration assertion that the staged libheif resolves **dav1d** (not libaom) as its AV1 decoder (`heif-info`/`libheif_decoder` enumeration lists dav1d) and **fails the build** if libaom is wired as the decoder or no dav1d decoder is present.
- [ ] **P5.13** [BUILD] Stage librsvg (>= 2.56.3 floor) for direct rsvg::Loader SVG load · §3.1 §3.5.5 §6.1.3 · G37 G37b
  needs: P5.1
  > librsvg (LGPL-2.1+) staged inside the image-worker for the **direct `rsvg::Loader`** path (NOT libvips `svgload`); pinned **`>= 2.56.3`** in `engines.lock` (the CVE-2023-38633 belt-and-suspenders floor, not load-bearing for v1 since P5.28 sets no base URL), dynamic-closure asserted.
- [ ] **P5.14** [BUILD] Add the librsvg version-floor + rsvg::Loader API-presence §6.1.3 assertions · §3.5.5 §6.1.3 · G38
  needs: P5.13
  > the stage-step assertions that the staged librsvg is **>= 2.56.3** (fail if older) **and** that the pinned `librsvg` crate/version exposes the relied-upon `rsvg::Loader::read_stream`/`from_data`-without-`base_file` path P5.28 depends on.
- [ ] **P5.15** [BUILD] Author the image-stack exposed-parameter capability §6.1.3 assertions (webpsave/heifsave effort + Q, jpegsave/pngsave/tiffsave/gifsave args) · §3.5.5 §6.1.3 · G38
  needs: P5.1, P5.8, P5.11
  > the image-specific list plugged into the P4 capability-assertion framework — assert the staged libvips actually exposes every per-format knob P5.33–P5.41 declare (`jpegsave` Q/chroma/progressive/optimize_coding, `pngsave` compression/interlace/palette, `webpsave` Q/lossless/effort/alpha_q/near_lossless/smart_subsample, `tiffsave` compression/predictor/tile/pyramid, `gifsave` dither-amount/bitdepth/effort, **`heifsave` `effort`+`Q`** for HEIC+AVIF) — a version bump silently dropping a knob **fails the build**. (The `heifsave effort` arg-presence check is necessary-but-not-sufficient for HEIC exposure — the steer-confirmation corpus spike P5.42 decides HEIC exposure.)

### Image-worker operation wiring (load → transform → save)

- [ ] **P5.16** [RUST] Wire the image-worker load step — by detected type (not extension), inside the §2.12 boundary · §3.5.5 §1.2 §2.12 · G29 G31
  needs: P5.1
  > the worker's load dispatch keyed on the §1.2-detected `FormatId` (built P3, per-format image signatures added P5.43), routing to the right libvips loader / load module / direct rsvg path; runs inside the P4 §2.12 isolation boundary; `VIPS_BLOCK_UNTRUSTED=1` whitelisted in the worker env as defence-in-depth for the non-SVG loaders (NOT load-bearing — the process boundary is, §3.5.5 control 3).
- [ ] **P5.17** [RUST] Wire the EXIF-orientation auto-rotate (bake upright, reset tag to 1) transform step · §3.5.5 · G29 G31
  needs: P5.16
  > the always-on orientation normalisation (image rotated to upright pixels, EXIF `Orientation` reset to `1`) — the one metadata field normalised not passed through (images.md Metadata policy); applies across every source that carries orientation (JPG/TIFF/HEIC/…).
- [ ] **P5.18** [RUST] Wire the alpha-flatten-to-background transform step (white default) for alpha-incapable targets · §3.5.5 · G29 G31
  needs: P5.16
  > the conditional flatten onto a background (default **white**, advanced override) applied only for alpha-incapable targets (JPG/BMP), feeding the `image_alpha_flatten` LossyKind for any alpha-carrying source; pure transform, no I/O.
- [ ] **P5.19** [RUST] Wire the per-target saver dispatch + Invocation/VipsStdout progress marshalling · §3.5.5 §3.2.2 §1.11 · G29 G31
  needs: P5.16
  > the worker's save dispatch mapping a resolved `TargetFmt` to the per-target saver + its params (P5.33–P5.41), producing the `Invocation`-equivalent plan; the worker installs the libvips `eval` signal handler and **marshals each tick to stdout as `progress=<0..100>` key=value lines** (`ProgressModel::VipsStdout`) parsed by the §1.7 line-reader (the worker is a separate process — an in-process callback cannot cross the boundary).

### Raster→raster pairs (the shared in-core vips savers)

> The 9 raster formats (JPG/PNG/WEBP/GIF/BMP/TIFF/HEIC/AVIF/ICO) form an all-to-all
> minus diagonal = 72 raster↔raster pairs, every one served by **vips** in one
> process. Grouped here by **encode saver code-path** (the smallest built-once unit);
> the HEIC/AVIF encode + decode load modules + BMP/ICO delegate paths are split into
> their own groups below because they are distinct code-paths with patent/spike/
> delegate concerns. Each pair's corpus backing, integration test and ledger row are
> in the corpus / per-pair-test / ledger groups (one per pair), so no pair hides in a
> group.

- [ ] **P5.20** [RUST] Wire the `jpegsave` encode path (→ JPG, all 9 sources) · §3.5.5 · G29 G31
  needs: P5.19, P5.18
  > `jpegsave` for `{PNG,WEBP,GIF,BMP,TIFF,HEIC,AVIF,ICO,SVG} → JPG`; always flattens transparency (P5.18), bakes orientation (P5.17), preserves ICC + EXIF/XMP/IPTC; lossy by codec at any Q (`image_lossy_codec`) + `image_alpha_flatten` for alpha sources.
- [ ] **P5.21** [RUST] Wire the `pngsave` encode path (→ PNG, all 9 sources) + APNG-collapse-to-first-frame · §3.5.5 · G29 G31
  needs: P5.19
  > `pngsave` for `{JPG,WEBP,GIF,BMP,TIFF,HEIC,AVIF,ICO,SVG} → PNG`; lossless by default (the per-source default target for GIF/BMP/TIFF/ICO/SVG); APNG **output** not supported — animated sources collapse to the first frame (`[DECIDED]` item 3); preserves RGBA + ICC + text chunks.
- [ ] **P5.22** [RUST] Wire the `webpsave` encode path (→ WEBP, all 9 sources) + animation passthrough · §3.5.5 · G29 G31
  needs: P5.19
  > `webpsave` for `{JPG,PNG,GIF,BMP,TIFF,HEIC,AVIF,ICO,SVG} → WEBP` (the per-source default for JPG+PNG); lossy by default (`image_lossy_codec`), `lossless` toggle; animation preserved from animated sources (GIF/animated-WEBP/APNG/avis), first-frame for stills; alpha + ICC/EXIF preserved.
- [ ] **P5.23** [RUST] Wire the native `gifsave` (cgif backend) encode path (→ GIF, all 9 sources) + animation passthrough · §3.5.5 · G29 G31
  needs: P5.19, P5.3
  > native `gifsave` (cgif, vips >= 8.12 — NOT the ImageMagick delegate) for `{JPG,PNG,WEBP,BMP,TIFF,HEIC,AVIF,ICO,SVG} → GIF`; 256-colour palette via the lovell/libimagequant fork (P5.3); lossy as target (`image_palette`); 1-bit transparency; animation preserved on GIF→GIF/animated-WEBP→GIF, first-frame for still sources; ImageMagick `magicksave` retained only as a fallback if native gifsave is unavailable.
- [ ] **P5.24** [RUST] Wire the `tiffsave` encode path (→ TIFF, all 9 sources) · §3.5.5 · G29 G31
  needs: P5.19
  > `tiffsave` for `{JPG,PNG,WEBP,GIF,BMP,HEIC,AVIF,ICO,SVG} → TIFF`; lossless by default (`compression=deflate`) → not lossy-flagged unless the user picks `compression=jpeg`; 16-bit + CMYK + alpha + ICC preserved to TIFF; multi-page source → first page for still (note when >1 page).

### BMP via the required ImageMagick delegate

- [ ] **P5.25** [RUST] Wire the `magicksave` BMP-save + `magickload` BMP-load delegate path (24-bit, alpha-flatten) · §3.5.5 §3.1 · G29 G31
  needs: P5.19, P5.5
  > BMP **load** (`magickload`) and BMP **save** (`magicksave`) through the required ImageMagick delegate (libvips has no native BMP, §3.1 row 1d) — still one vips process; for `→ BMP` from all 9 sources writes **24-bit BMP flattening alpha onto white** (`image_alpha_flatten` for alpha sources; JPG→BMP stays lossless — JPG has no alpha); BMP-as-source decodes for all targets.

### ICO save — the build spike (magicksave default / in-core Rust assembler fallback)

- [ ] **P5.26** [BUILD] Run the §6.1.3 ICO multi-size/256px build spike (magicksave write valid `[16,32,48,256]` .ico) + record the (a)-or-(b) outcome · §3.5.5 §6.1.3 · G38 G7
  needs: P5.5
  > the `[DEFER: build spike]` resolution — confirm the bundled libvips+ImageMagick `magicksave` can write a valid multi-size `.ico` including a **256px embedded-PNG** entry; **record the binary outcome in this plan's notes:** (a) spike passes → the ICO-save path uses `magicksave` and the §6.1.3 assertion fails the build if magicksave ICO save regresses; (b) spike fails → the ICO-save path uses the in-core Rust ICO container assembler and the assertion targets that output, dropping ImageMagick from the ICO path. This box is the **single decision record**; the ICO-save path itself is built once in P5.28 against the recorded outcome (so exactly one code path is authored, never two mutually-exclusive open boxes).
- [ ] **P5.27** [RUST] Wire the ICO-save path per the P5.26-recorded outcome (magicksave default OR the in-core Rust ICO assembler fallback; pad-to-square + per-size Lanczos, no-upscale) · §3.5.5 §6.1.3 · G29 G31
  needs: P5.26, P5.19
  > ICO save for `{JPG,PNG,WEBP,GIF,BMP,TIFF,HEIC,AVIF,SVG} → ICO`, built against **whichever path P5.26 recorded** (no mutual-exclusion: a single box, one code path chosen by the spike): **outcome (a)** → `magicksave` writes the `.ico` directly; **outcome (b)** → a safe-Rust ICONDIR + per-entry image-data assembler wraps vips-produced per-size frames (ICO is a trivial container), removing ImageMagick from the ICO path while keeping vips as the per-frame encoder (the per-frame encode stays one vips process). Both paths produce the default multi-resolution set `[16,32,48,256]`, high-quality Lanczos downscale, **upscale-beyond-source skipped** (note if smaller), non-square **padded to square with transparency** (`[DECIDED]` item 5), 256px stored as embedded PNG; lossy by downscale (`image_downscale`, NOT `image_palette`). The §6.1.3 assertion (P5.54) targets whichever output ships.

### SVG source path (librsvg, the no-base-URL T9b/SSRF control)

- [ ] **P5.28** [RUST] Wire the SVG load via direct `rsvg::Loader` with NO base_file (the load-bearing T9b/CVE-2023-38633 control) → vips save · §3.5.5 §0.11 §3.3.4 · G29 G31
  needs: P5.16, P5.13
  > the image-worker reads SVG bytes into memory and loads via `rsvg::Loader` (`read_stream`/`from_data`) **without** a `base_file`/base URL (NOT via libvips `svgload`, which exposes no external-resource toggle); with no base URL librsvg refuses **all** local `<image href>`/XInclude reads by construction (closes the absolute-file LFR half) and remote schemes regardless (closes the SSRF half) — **no base-URL/scratch confinement is used** (supplying a base URL re-enables the CVE-class surface; the defence is the *absence*). Rendered raster handed to vips for save — one process, no chaining. Also handles `.svgz` (transparent gunzip first).
- [ ] **P5.29** [RUST] Wire SVG output-size resolution (intrinsic / viewBox@96DPI) + bundled-font rendering + pathological-size clamp · §3.5.5 §1.10 · G29 G31
  needs: P5.28, P4.71
  > default render at intrinsic `width`/`height`, else viewBox @ **96 DPI**; target-width-in-px + 2×/3× scale shortcut; SVG text rendered from the **bundled** font set (Liberation/Carlito/Caladea + Noto subset) via the worker's fontconfig (no host-font access, deterministic substitution); transparent background (white for JPG/BMP targets); a pathological tiny-viewBox-huge-render clamped against the **P4-built §1.10 budget engine (P4.71)** (fail-clearly, not OOM) — this box FEEDS the §1.10 engine its raster-dims input, never re-implements the ceiling.
- [ ] **P5.30** [RUST] Wire the SVG → {PNG★,JPG,WEBP,BMP,TIFF,ICO} target routing (HEIC/AVIF out) · §3.5.5 · G29 G31
  needs: P5.28, P5.29
  > the 6 offered SVG targets routed to the P5.20–P5.26 savers; **SVG→HEIC / SVG→AVIF are `out`** (no everyday demand — matrix and offered set agree, so the bijection guard does not enumerate them); every SVG→raster cell fires `image_svg_raster` (incl. the PNG★ default — never omit it), plus the target-codec LossyKind where additionally lossy.

### Patent-gated encode paths (HEIC / AVIF via heifsave — reads §3.4)

- [ ] **P5.31** [RUST] Wire `heifsave compression=hevc` (→ HEIC encode, x265 plugin) — single code path incl. AVIF→HEIC · §3.5.5 §3.4.3 · G29 G31
  needs: P5.19, P5.9, P5.10
  > `heifsave compression=hevc` for `{JPG,PNG,WEBP,GIF,BMP,TIFF,AVIF,ICO,SVG} → HEIC` (incl. the cross-codec `AVIF→HEIC`, one vips process); lossy by default (`image_lossy_codec`), `lossless` toggle; never a default target; gated on the §3.4 HEIC-encode availability cell (P5.32).
- [ ] **P5.32** [RUST] Wire HEIC-encode availability gating — read the §3.4.4a per-platform `available` flag → unavailable-with-reason · §3.4.3 §3.4.4a §2.8 · G29
  needs: P5.31
  > **reads** (never re-decides) the §3.4 patent cell via the P4-built `engines.lock.available → PatentDisposition → EngineHealth.unavailable_targets` wiring; when HEIC-encode is `available=false` on a platform, the target tile is surfaced **disabled-with-reason** (`PlatformUnavailable`, §2.8), never silently dropped — the only legitimate `select()→None` for an in-scope pair. (P4 owns the wiring; this box consumes it for the HEIC target.)
- [ ] **P5.33** [RUST] Wire `heifsave compression=av1` (→ AVIF encode, libaom) — single code path incl. HEIC→AVIF · §3.5.5 §3.4.3 · G29 G31
  needs: P5.19, P5.11
  > `heifsave compression=av1` for `{JPG,PNG,WEBP,GIF,BMP,TIFF,HEIC,ICO,SVG} → AVIF` (incl. the cross-codec `HEIC→AVIF`, one vips process; libaom the single bundled AV1 encoder); lossy by default (`image_lossy_codec`), `lossless` toggle; AVIF ship-bundled everywhere per §3.4 (no gate), but never defaulted *to* in v1.

### Decode load modules (HEIC / AVIF source → raster, via libheif/libde265 + dav1d)

- [ ] **P5.34** [RUST] Wire HEIC decode (libheif+libde265 load module) → all raster targets, primary image only · §3.5.5 §3.4.3 · G29 G31
  needs: P5.16, P5.8
  > `HEIC → {JPG★,PNG,WEBP,GIF,BMP,TIFF,AVIF,ICO}` via the libheif load module (libde265 HEVC decode, LGPL, decode-only — no x265); **primary image only** (Live Photos/sequences/bursts/depth/aux dropped, note); HDR/10-bit tone-mapped to 8-bit SDR for 8-bit targets (note if HDR dropped); EXIF orientation baked, ICC preserved.
- [ ] **P5.35** [RUST] Wire AVIF decode (dav1d load module) → all raster targets, primary image only · §3.5.5 §3.4.3 · G29 G31
  needs: P5.16, P5.11
  > `AVIF → {JPG★,PNG,WEBP,GIF,BMP,TIFF,HEIC,ICO}` via libheif configured to resolve **dav1d** for AV1 decode (P5.12); animated AVIF (`avis`) → GIF/animated-WEBP preserves animation, first-frame for stills (note); HDR/10-12-bit/wide-gamut tone-mapped for 8-bit targets; alpha + ICC/EXIF preserved.
- [ ] **P5.36** [RUST] Wire ICO decode (built-in) → all raster targets, largest-frame selection · §3.5.5 · G29 G31
  needs: P5.16
  > `ICO → {JPG,PNG★,WEBP,GIF,BMP,TIFF,HEIC,AVIF}` via the libvips built-in ICO loader; when the ICO holds several sizes the **largest image** is selected as source pixels (note if >1 size), the rest discarded; alpha preserved; ICO→PNG (largest frame) not lossy. (BMP/GIF/TIFF/JPG/PNG/WEBP as *source* decode through their built-in/delegate loaders already wired at P5.16/P5.25.)

### Advanced-option declarations (registered against the P4 options-panel shell)

> The panel **chrome** was built in P4; P5 registers only per-format option
> **DECLARATIONS** (§1.6 generic declaration model). Each declaration's exposed-arg
> existence is build-asserted at P5.15; the values are the images.md `[DECIDED]`
> defaults (the `[DEFER: corpus]` ones are confirmed against the corpus, P5.42).

- [ ] **P5.37** [UI] Register the JPG advanced-option declarations (Q=82, chroma auto, progressive on, optimize_coding on, flatten bg) · §1.6 · G47
  needs: P5.20
  > basic Q slider (1–100, default **82**), advanced `chroma subsampling` auto / `progressive` on / `optimize_coding` on / flatten background white — declared against the P4 shell; no new panel chrome.
- [ ] **P5.38** [UI] Register the PNG advanced-option declarations (compression=6, interlace off, palette/bitdepth off; palette-only Q100/effort7) · §1.6 · G47
  needs: P5.21
  > basic none (lossless just-works); advanced `compression` 0–9 (default **6**), `interlace` off, `palette`/`bitdepth` quantisation off; `Q`/`effort` apply only when `palette` is enabled (libimagequant — Q100/effort7).
- [ ] **P5.39** [UI] Register the WEBP advanced-option declarations (Q=80, lossless off, effort=4, alpha_q=100, near_lossless/smart_subsample/min_size/mixed off) · §1.6 · G47
  needs: P5.22
  > basic Q slider (0–100, default **80**) + `lossless` toggle (reinterprets Q as lossless effort); advanced `effort` 0–6 (default **4**), `alpha_q` 1–100 (default **100**), `near_lossless`/`smart_subsample`/`min_size`/`mixed` off.
- [ ] **P5.40** [UI] Register the GIF advanced-option declarations (dither = single 0–1 AMOUNT, NOT a mode selector; bitdepth=8, effort=7) · §1.6 · G47
  needs: P5.23
  > advanced `dither` exposed as a **single float amount (0–1) / on-off toggle, default on — NOT a Floyd–Steinberg/mode dropdown** (the cgif/libimagequant save path has no error-diffusion MODE, only strength — the seam note: the bayer/sierra2_4a mode choice exists ONLY on the FFmpeg video→GIF path, P6, and must not be conflated); `bitdepth`/colour count <=256 (default **8**), `effort` palette-search (default **7**); interframe maxerror/reuse at vips defaults.
- [ ] **P5.41** [UI] Register the BMP + TIFF advanced-option declarations (BMP none; TIFF compression=deflate, Q82-if-jpeg, predictor horizontal, tile/pyramid off) · §1.6 · G47
  needs: P5.24, P5.25
  > BMP: none meaningful (uncompressed, no RLE toggle); TIFF advanced `compression` `none|jpeg|deflate|lzw|packbits|zstd` (default **deflate**), `Q` 82 only if `compression=jpeg`, `predictor` horizontal for deflate/lzw, `tile`/`pyramid` off.
- [ ] **P5.42** [UI] Register the HEIC + AVIF advanced-option declarations (HEIC Q=60, lossless off, effort 0–9 corpus-gated; AVIF Q=60, effort=4, lossless off; NO cq-level/preset/speed) · §1.6 · G47
  needs: P5.31, P5.33
  > HEIC basic Q (0–100, default **60**), advanced `lossless` off + integer `effort` 0–9 (default 5) **exposed only if the P5.46 corpus spike confirms `effort` measurably steers the bundled x265/HEVC path — else HIDDEN for HEIC** (no dead control; libheif `speed=9-effort`); AVIF basic Q (0–100, default **60**), advanced `effort` 0–9 (default **4**, libvips-documented as honoured → stays exposed) + `lossless` off; **no `cq-level`/`preset`/`speed` controls** (not heifsave params); 8-bit default, 10/12-bit + chroma 4:2:0 advanced.
- [ ] **P5.43** [UI] Register the SVG advanced-option declarations (width-px / scale 1.0 / explicit WxH / background transparent / dpi 96) · §1.6 · G47
  needs: P5.29
  > basic target width in px (height auto) default = intrinsic + 2×/3× scale shortcut; advanced `scale`/`zoom` (default **1.0**), explicit `width`×`height`, `background` transparent (white for JPG/BMP), `dpi` (default **96**).
- [ ] **P5.44** [UI] Register the category-wide advanced toggles — strip location/metadata (off) + ICC preserve (no sRGB-convert in v1) + flatten background colour · §1.6 · G47
  needs: P5.37
  > the cross-format advanced toggles: **"remove location/metadata"** off-by-default (preserve-all incl. GPS is the v1 default, `[DECIDED]` item 4); ICC **preserve/embed** (the "convert to sRGB" toggle is explicitly NOT in v1, `[DEFER: post-v1]`); the alpha-flatten background-colour picker (default white) — declared once for JPG/BMP targets.

### Detection signatures (per-format magic, added to the P3 framework)

- [ ] **P5.45** [RUST] Add the per-format image detection signatures to the §1.2 framework (content-sniff, not extension) · §1.2 · G15 G29
  needs: P5.16
  > the magic/structure signatures for every image source (JPG `FF D8 FF`+APPn+`FF D9`; PNG `89504E47…`+APNG `acTL`; WEBP RIFF/WEBP+`VP8 `/`VP8L`/`VP8X`; GIF `GIF87a`/`GIF89a`; BMP `42 4D`+DIB-header-sanity; TIFF `II*\0`/`MM\0*`+BigTIFF; HEIC ISO-BMFF `ftyp` `heic`/`heix`/`heif`/`mif1`/`heis`/`hevc`; AVIF `ftyp avif`/`avis`; ICO `00 00 01 00` — CUR `00 00 02 00` declined; SVG root `<svg`+`.svgz` gunzip) — added to the P3-bootstrapped §1.2 layered detector; the P0.5.7 detect-KAT convention covers the ambiguous cells.

### Corpus (the §6.4.5 image set + the bijection-guard backing)

- [ ] **P5.46** [TEST] Stage the image corpus into `tests/corpus/images/` + manifest entries with `covers` + SHA-256 manifest · §6.4.5 §6.4.3a · G24a G31
  needs: P5.45
  > the concrete §6.4.5 image contents — real iPhone HEIC (HDR/10-bit/orientation 1/3/6/8/GPS/ICC-P3); JPEG (EXIF-orientation/progressive/CMYK/12-bit/truncated-tail); PNG (RGBA/16-bit/palette/APNG); WEBP (lossy/lossless/animated/alpha); AVIF (still + `avis` + HDR); GIF (static + animated); TIFF (multi-page/16-bit/CMYK/big-endian); BMP (24/32-bit, top-down/bottom-up); ICO (multi-res 16/32/48/256 + non-square); SVG (intrinsic/viewBox-only/missing-font/`.svgz`/**remote `<image href>`**/pathological tiny-viewBox-huge-render) — each redistributable (CC0/synthetic), each `[[file]]` with `source`/`licence`/`exercises`/`covers`/`[file.expect]`; SHA-256 added to the §6.4.5 manifest (G24a integrity), small/synthetic in-repo + large real media in `corpus-large` LFS.
  - [ ] **P5.46.1** [TEST] Populate every image `covers` 2-tuple so the §6.4.3a bijection guard passes for all 78 image pairs · §6.4.3a §6.4.5 · G31
    > the `covers` arrays collectively name **every** offered image `(source→target)` pair (72 raster↔raster + 6 SVG→raster), excluding diagonals/`out`(SVG→HEIC/AVIF)/`unavailable`-on-all-platforms cells, so `scripts/check-corpus-coverage.rs` (P4) finds a backing file for each — and no `covers` 2-tuple names a non-existent matrix cell (both directions of the bijection).
  - [ ] **P5.46.2** [TEST] Add the §6.4.5 minimum-content image tags (non-ascii-encoding n/a; orientation/HDR/ICC/animation/alpha exercisers) · §6.4.5 · G31
    > assert the manifest carries at least the image-relevant content exercisers (orientation-bake, ICC-P3, HDR-10bit, animation-collapse, APNG, animated-WEBP/AVIF/GIF, alpha-flatten, multi-page, palette) so the corpus is content-complete for images, not merely pair-complete.

### Per-pair integration tests (the §6.4.3 runner + structural readers)

> One §6.4.3 integration test per offered image pair, run on the P4-built per-pair
> runner against the §6.4.5 corpus, using the **real structural reader** (`vipsheader`
> decode + nonzero dims — NOT magic re-detect) plus the G31/G32 sub-assertions. Split
> by saver/decoder group so each is an atomic, separately-faileable box; a later pass
> may split further to one-box-per-pair if the reviewer wants finer grain.

- [ ] **P5.47** [TEST] Per-pair integration tests: → JPG (all sources) — vipsheader decode + dims + orientation-bake + alpha-flatten + lossy-disclosure-iff-flagged · §6.4.3 §6.5 · G31 G32
  needs: P5.20, P5.46.1
  > each `* → JPG` pair completes with exit success, output decodes via `vipsheader` with nonzero dims, source byte-unchanged (G32 no-harm), output≠input, orientation baked upright, alpha-source flatten asserted, `image_lossy_codec`(+`image_alpha_flatten`) fires iff the §04 cell is flagged.
- [ ] **P5.48** [TEST] Per-pair integration tests: → PNG (all sources) — decode + dims + lossless + APNG-collapse + no-spurious-lossy · §6.4.3 §6.5 · G31 G32
  needs: P5.21, P5.46.1
  > each `* → PNG` pair decodes, source-unchanged, lossless (no lossy note unless palette explicitly enabled), animated source collapses to first frame with the note, RGBA/16-bit/ICC/text-chunk fidelity spot-checks.
- [ ] **P5.49** [TEST] Per-pair integration tests: → WEBP (all sources) — decode + dims + animation-passthrough + lossy-disclosure · §6.4.3 §6.5 · G31 G32
  needs: P5.22, P5.46.1
  > each `* → WEBP` pair decodes, source-unchanged, `image_lossy_codec` iff flagged (default lossy), animated source preserves animation (validated via ffprobe per the §6.4.5 animated-WEBP convention, not dwebp), alpha preserved.
- [ ] **P5.50** [TEST] Per-pair integration tests: → GIF (all sources) — decode + dims + palette + animation-passthrough + image_palette · §6.4.3 §6.5 · G31 G32
  needs: P5.23, P5.46.1
  > each `* → GIF` pair decodes, source-unchanged, `image_palette` fires, animation preserved on animated sources / first-frame for stills with note, 1-bit transparency handled.
- [ ] **P5.51** [TEST] Per-pair integration tests: → TIFF (all sources) — decode + dims + lossless-default + 16-bit/CMYK fidelity · §6.4.3 §6.5 · G31 G32
  needs: P5.24, P5.46.1
  > each `* → TIFF` pair decodes, source-unchanged, lossless by default (no lossy note unless `compression=jpeg`), 16-bit/CMYK/alpha/ICC fidelity, multi-page source → first page with note.
- [ ] **P5.52** [TEST] Per-pair integration tests: → BMP (all sources) — decode + dims + 24-bit alpha-flatten (JPG→BMP lossless) · §6.4.3 §6.5 · G31 G32
  needs: P5.25, P5.46.1
  > each `* → BMP` pair decodes via magickload re-read, source-unchanged, 24-bit output, `image_alpha_flatten` fires for alpha sources and **does not** fire for JPG→BMP (lossless — JPG has no alpha), no spurious lossy note for no-alpha sources.
- [ ] **P5.53** [TEST] Per-pair integration tests: → ICO (all sources) — re-open .ico + assert 16/32/48/256 entries + 256px PNG marker + non-square padding + image_downscale · §6.4.3 §6.1.3 §6.5 · G31 G32
  needs: P5.27, P5.46.1
  > each `* → ICO` pair completes, the produced `.ico` is **re-opened and all four [16,32,48,256] entries + the 256px embedded-PNG marker verified** (the runtime proof of whichever ICO path shipped per the P5.26-recorded outcome, built in P5.27), non-square padding to square asserted, `image_downscale` fires; this is the corpus case the §6.1.3 ICO spike (P5.26) points at — the single ICO-save box P5.27 covers both spike outcomes, so no `[!]`/mutual-exclusion gating is needed.
- [ ] **P5.54** [TEST] Per-pair integration tests: → HEIC encode (all sources, incl. AVIF→HEIC) — decode + codec + lossy + patent-gap-skip · §6.4.3 §3.4.3 §6.5 · G31 G32
  needs: P5.31, P5.32, P5.46.1
  > each `* → HEIC` pair on platforms where §3.4 marks HEIC-encode **available**: completes, output decodes (cross-library re-validate via ffprobe per P0.5.6), `image_lossy_codec` iff flagged, source-unchanged; on a platform where §3.4 marks it **unavailable** the test asserts the target is **absent/disabled (not attempted)** — honest unavailability, not a failure.
- [ ] **P5.55** [TEST] Per-pair integration tests: → AVIF encode (all sources, incl. HEIC→AVIF) — decode + codec + lossy + dav1d-decode-revalidate · §6.4.3 §3.4.3 §6.5 · G31 G32
  needs: P5.33, P5.46.1
  > each `* → AVIF` pair completes, output decodes (cross-library re-validate via ffprobe per P0.5.6 — a different decoder family), `image_lossy_codec` iff flagged, source-unchanged; AVIF available everywhere (no patent gap).
- [ ] **P5.56** [TEST] Per-pair integration tests: HEIC/AVIF/ICO source → raster targets — primary-image-only + HDR-tonemap + largest-frame · §6.4.3 §6.5 · G31 G32
  needs: P5.34, P5.35, P5.36, P5.46.1
  > each `{HEIC,AVIF,ICO} → raster` pair decodes, source-unchanged, primary-image-only (HEIC bursts / animated-AVIF-to-still), HDR→8-bit tone-map note, ICO largest-frame selection note, orientation baked, ICC preserved; HEIC→raster/AVIF→raster lossless w.r.t. decoded pixels (no spurious lossy on →PNG/TIFF).
- [ ] **P5.57** [TEST] Per-pair integration tests: SVG → {PNG,JPG,WEBP,BMP,TIFF,ICO} — decode + dims + image_svg_raster always + bundled-font render · §6.4.3 §6.5 · G31 G32
  needs: P5.30, P5.46.1
  > each SVG→raster pair decodes at the resolved size, `image_svg_raster` fires for **every** pair incl. the PNG★ default (plus the target-codec LossyKind where additionally lossy), bundled-font glyphs render (no tofu, deterministic substitution), source-unchanged.
- [ ] **P5.58** [TEST] SVG no-base-URL out-of-input / no-egress §6.1.3 corpus case (remote + relative-`../` + absolute `<image href>` not resolved) · §3.5.5 §6.4.2 §0.11 · G31 G32 G42b
  needs: P5.28, P5.46
  > the SVG analogue of the FFmpeg adversarial-egress case — an SVG with an external `<image href>` (relative `../` escape AND absolute AND remote) must **NOT** embed any out-of-input bytes in the output and must trigger **no egress**; with the SVG loaded no-base-URL (P5.28) the reference simply does not resolve. The per-push adversarial-egress pull-forward leg (G42b) is wired in P9; this box stages the SVG sentinel + the out-of-input assertion the corpus run consumes.
- [ ] **P5.59** [TEST] ImageMagick crafted-BMP + SVG-via-MSL/URL-coder sentinel corpus case (no egress, no out-of-input read) · §3.5.5 §6.4.2 §0.11 · G31 G42b
  needs: P5.6, P5.46
  > the §3.5.5 T9b/T1 ImageMagick sentinel (the densest-CVE decoder family — ImageTragick / MSL / MVG / URL coder class): a crafted BMP + an SVG-via-MSL/URL-coder fixture must produce **no egress + no out-of-input read** (the coder lockdown P5.6 holds); staged here for the P9 adversarial-egress window + the §6.4.2 oracle.
- [ ] **P5.60** [TEST] Determinism + source-unchanged sub-assertions: >=1 byte-stable pair per saver category + known-non-deterministic AVIF/HEIC manifest exceptions · §2.5 §6.4.3 · G31 G32
  needs: P5.47, P5.48, P5.51
  > the P0.5.5 determinism floor for images — same source+settings twice → `sha256(out1)==sha256(out2)` for >=1 pair per output-format category, byte-stable pairs enumerated with a rationale in `tests/corpus/manifest.toml`, and the **known-non-deterministic AVIF/HEIC variable-encode** documented as manifest exceptions; source-unchanged (G32) over every image corpus source.

### Reliability ledger, SBOM/NOTICE rows, availability rows

- [ ] **P5.61** [TEST] Mark every available image pair `reliable` in the §6.5.2 pair-status ledger on all 3 platforms · §6.5 §6.5.1 §6.5.2 · G31
  needs: P5.47, P5.48, P5.49, P5.50, P5.51, P5.52, P5.53, P5.54, P5.55, P5.56, P5.57
  > drive the P4-built ledger generator so every enumerated image pair is `reliable` (valid output + no-harm + fail-clearly + lossy-disclosure-matches + content-fidelity, on each platform where §3.4 says it is available) — `reliability-report.json` + human table; any `failing` cell blocks; this is the §6.5 coverage gate for the image category travelling with the format work (category-by-category sequencing).
- [ ] **P5.62** [TEST] Record the HEIC-encode patent-gap exception (per platform) as a §6.5.3 demoted-pairs / release-note row · §6.5.3 §3.4.3 · G31
  needs: P5.54, P5.61
  > for any platform where §3.4 marks HEIC-encode `unavailable`, add the structured `docs/demoted-pairs.md` row (`kind=patent-gap-per-platform`, affected platform(s), one-sentence reason, ledger ref + the `engines.lock available=false` row it derives from) so the §6.8 governance gate finds a matching row and the gap is documented, never silent. (No-op if §3.4 ships HEIC-encode available on all three.)
- [ ] **P5.63** [BUILD] Populate the image-stack §3.7.2 `engines.lock` + CycloneDX SBOM rows (libvips/libheif/libde265/x265/libaom/dav1d/librsvg/cgif/libimagequant/ImageMagick) · §3.7.2 §3.6.2 · G35 G35a G36 G37
  needs: P5.1, P5.5, P5.8, P5.9, P5.11, P5.13, P5.3
  > each image engine/component a §3.7.2 row (mandatory `purl` + SHA-256), the SBOM `purl`-keyed rows + the **DERIVED static-link closure** (G35a) for the statically-linked image-worker stack; license hard-fail (G36) confirms only x265 is GPL (the isolated plugin), the rest LGPL/BSD/permissive; per-engine acquisition anchored per P0.7.3.
- [ ] **P5.64** [BUILD] Validate the image-stack SPDX expressions + generated-vs-committed NOTICE/THIRD-PARTY-LICENSES parity · §3.7 §6.3.3 · G36 G35
  needs: P5.63
  > the SPDX-expression validation leg for the image rows — x265 `GPL-2.0-or-later`, libheif/libde265/librsvg/libvips `LGPL-*`, libaom **both** `BSD-2-Clause AND LicenseRef-AOMPL-1.0` (the AOM Patent License text carried in `THIRD-PARTY-LICENSES.txt`, the §6.3.3 LicenseRef carve-out), dav1d/cgif/libimagequant BSD/MIT, ImageMagick `ImageMagick`; every GPL/LGPL row has its license text + a corresponding-source pointer line in `THIRD-PARTY-LICENSES`; AAC/HEVC patent posture surfaced in NOTICE.
- [ ] **P5.65** [BUILD] Author the x265 GPL §3 + image-worker LGPL §6 corresponding-source bundle-present §6.1.3 assertion · §6.1.3 §3.6.2 · G38b
  needs: P5.9, P5.63
  > the §6.1.3 carve-out ii/iii bundle-presence assertion for the image stack — ship the static image-worker's complete corresponding source + LGPL object files / relink recipe **and** (because it loads the GPL x265 plugin → GPL combined work) the **x265 GPL §3 complete corresponding source + written offer**; the stage step **fails the build** if either source bundle is missing (the §5 T6 row).
- [ ] **P5.66** [BUILD] Populate the per-engine §7.2.3 availability rows for the image stack (presence + integrity manifest entries) · §7.2.3 §3.4.4a · G37
  needs: P5.1, P5.63
  > add the image-engine entries to the build-time in-bundle hash manifest + the §7.2.3 startup-verifier availability rows (the image-specific variant of the P4 generic verifier), so `EngineHealth.present`/`integrity_ok`/`runnable` covers libvips + the codec stack and HEIC-encode availability reflects the §3.4.4a flag; ImageMagick is presence-checked for attribution only (it is a delegate, not a registry engine — no `Engine` impl / registry row, §3.5.5).

### Build-spike / corpus-gated decision records

- [ ] **P5.67** [DOC] Record the HEIC `effort`-steers-x265 corpus spike outcome → decide HEIC `effort` exposure (else hidden) · §6.1.3 · G7
  needs: P5.42, P5.46
  > run the `[DEFER: corpus]` spike confirming whether the integer `heifsave effort` measurably steers the bundled x265/HEVC path; **record the outcome** in this plan's notes and flip the P5.42 declaration accordingly — exposed for HEIC iff it steers, **HIDDEN for HEIC** if inert (no dead control; AVIF `effort` stays exposed regardless). The §6.1.3 arg-presence check (P5.15) is necessary but not sufficient — this corpus spike is the exposure decider.
- [ ] **P5.68** [DOC] Confirm the corpus-gated default-Q values (JPG 82 / WEBP 80 / HEIC&AVIF 60) against the real-photo corpus · §6.5 · G7
  needs: P5.46, P5.61
  > the `[DEFER: corpus]` calibration of the reasoned everyday defaults against the §6.4.5 real-photo corpus before locking §1.6 — a measured confirmation, not an open design call; record the confirmed values (or any adjustment + rationale) in this plan's notes.
