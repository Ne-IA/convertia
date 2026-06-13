# 04 — Formats: Images

> Category spec for images. **Template demo** for the other category files.
> Formats (SSOT *What It Converts*): JPG/JPEG, PNG, WEBP, GIF, BMP, TIFF,
> HEIC/HEIF *(patent → §3.4)*, AVIF *(patent → §3.4)*, ICO; plus **SVG as a
> *source* only** (rasterised → PNG/JPG/…). SVG-as-target (raster→vector) is a
> reverse/reconstructive conversion and is **out of v1 / parked** (SSOT *Direction
> & shape rule*, *Future Ideas*).
>
> Scope reminder (SSOT): conversions are strictly **one-source → one-target**,
> every pair is satisfied by **one** engine (§3.2 single-engine rule, **no
> chaining**), and a pair is included only if it passes the canonical inclusion
> test ("would a normal person plausibly want it?"). Degenerate / no-demand pairs
> are marked **out** below with a reason. The full coverage ships in v1 (no MVP
> cut).

---

## Source → target matrix

Rows = **source** format, columns = **target** format. Cell legend:

- `✓ <eng>` — supported (engine short-name; see *Engines* below)
- `✓★ <eng>` — supported **and the pre-highlighted DEFAULT target** for that source
- `✓~ <eng>` — supported but **predictably lossy** (→ §2.9 disclosure)
- `✓★~ <eng>` — default **and** lossy
- `—` — same format as source on the diagonal (re-encode handled, see note)
- `out: <reason>` — fails the inclusion test / direction rule; not offered

Engine short-names: **vips** = libvips raster core (incl. `heifsave` for ALL
HEIC/AVIF *encode* — `compression=hevc` via the x265 libheif plugin, `compression=av1`
via libaom — and `magicksave` via the **required** ImageMagick delegate for BMP, and the
default ICO-save path (ICO save **`[DEFER: build spike]`** §3.5.5; in-core Rust ICO
assembler fallback)),
**svg** = SVG rasteriser (**librsvg**, libvips' native `svgload` backend) **invoked
via libvips' SVG loader** (so the *raster save* stays in vips — still one engine for
the pair; resvg is NOT a libvips backend and is **not shipped** [DECIDED] §3.1 row 1c).
(There are **no separate `heif`/`avif` short-names** — the standalone encoders were
dropped; all HEIC/AVIF encode is `vips heifsave`, [OPEN-1] [DECIDED].) See *Engines*
for the binding.

| src ＼ tgt | JPG | PNG | WEBP | GIF | BMP | TIFF | HEIC | AVIF | ICO |
|-----------|-----|-----|------|-----|-----|------|------|------|-----|
| **JPG**   | —          | ✓ vips      | ✓★~ vips     | ✓~ vips      | ✓ vips      | ✓ vips      | ✓~ vips      | ✓~ vips      | ✓~ vips |
| **PNG**   | ✓~ vips    | —           | ✓★~ vips     | ✓~ vips      | ✓ vips      | ✓ vips      | ✓~ vips      | ✓~ vips      | ✓~ vips |
| **WEBP**  | ✓★~ vips   | ✓ vips      | —            | ✓~ vips      | ✓ vips      | ✓ vips      | ✓~ vips      | ✓~ vips      | ✓~ vips |
| **GIF**   | ✓~ vips    | ✓★ vips     | ✓~ vips      | —            | ✓ vips      | ✓ vips      | ✓~ vips      | ✓~ vips      | ✓~ vips |
| **BMP**   | ✓~ vips    | ✓★ vips     | ✓~ vips      | ✓~ vips      | —           | ✓ vips      | ✓~ vips      | ✓~ vips      | ✓~ vips |
| **TIFF**  | ✓~ vips    | ✓★ vips     | ✓~ vips      | ✓~ vips      | ✓ vips      | —           | ✓~ vips      | ✓~ vips      | ✓~ vips |
| **HEIC**  | ✓★~ vips   | ✓ vips      | ✓~ vips      | ✓~ vips      | ✓ vips      | ✓ vips      | —          | ✓~ vips      | ✓~ vips |
| **AVIF**  | ✓★~ vips   | ✓ vips      | ✓~ vips      | ✓~ vips      | ✓ vips      | ✓ vips      | ✓~ vips    | —            | ✓~ vips |
| **ICO**   | ✓~ vips    | ✓★ vips     | ✓~ vips      | ✓~ vips      | ✓ vips      | ✓ vips      | ✓~ vips      | ✓~ vips      | —      |
| **SVG**†  | ✓~ svg     | ✓★~ svg     | ✓~ svg       | ✓~ svg       | ✓~ svg      | ✓~ svg      | out*         | out*         | ✓~ svg |

† **SVG is source-only, and EVERY SVG→raster cell is `~` (lossy).** It is rasterised
once (vector → pixels) and that bitmap is saved to the target; the rasterise step is
inherently lossy *to a fixed pixel grid* (you lose infinite scalability) **regardless
of the target codec**, so **all** SVG→raster pairs — including the SVG→PNG ★ default —
fire the §2.9 **`image_svg_raster`** LossyKind (this is why every cell in the SVG row,
not just SVG→JPG/GIF, is marked `✓~`). Cells whose target codec is *additionally* lossy
**also** fire that target-codec-specific LossyKind on top of `image_svg_raster` — e.g.
SVG→GIF adds **`image_palette`** (≤256-colour palette), SVG→JPG adds JPEG compression
loss, SVG→WEBP/HEIC/AVIF add their lossy-codec note. The disclosure derivation MUST emit
`image_svg_raster` for every SVG→raster pair (never omit it for the `★` PNG default).

\* SVG→HEIC / SVG→AVIF are **`out`** (matrix and offered set agree — see *Pairs
deliberately out* and the SVG entry): no everyday demand to rasterise a vector to
HEIC/AVIF. They are **not** in the offered set (SVG offers PNG/JPG/WEBP/BMP/TIFF/ICO),
so the §6.4.3a corpus↔pair bijection guard does not enumerate them. (Technically the
SVG loader could rasterise to pixels for `heifsave` in one vips process, but the pair
is deliberately excluded.)

**Diagonal (same→same).** Not a category-internal "conversion" in the menu, but
re-encoding *is* a real user action (re-compress a JPG, flatten a PNG). The SSOT
*Never harm the original* clause explicitly covers source==target (kept original +
adapted name). v1 policy: **same-format is not offered as a target tile** in the
images target list (it would clutter and confuse "convert to what?"); a
dedicated "re-compress / optimise" action is **parked** (not in v1). Marked `—`.

### Pairs deliberately **out** (fail the inclusion test / direction rule)

| Pair | Why out |
|------|---------|
| `* → SVG` | Raster→vector = reverse/reconstructive (SSOT *Direction & shape rule*); parked. SVG is source-only. |
| `SVG → HEIC`, `SVG → AVIF` | No everyday demand (nobody rasterises a logo to HEIC); kept off the SVG target list to stay uncluttered. |
| `* → GIF` for **non-animated** still images as a *quality* choice | GIF is offered (256-colour still / animation passthrough) but is **never the default** for a still — it is a strictly worse still codec than PNG/WEBP. Included only because a normal person does sometimes specifically want a `.gif`. |
| animated GIF / WEBP / APNG **→ video** (mp4/webm) | Cross-category; the **only** sanctioned cross-category image output is none — image→video is **not** in the closed cross-category set (that set is extract-audio + to-GIF, both *from video*). Out. |
| any image → **multi-frame fan-out** (e.g. animated GIF → one PNG per frame) | One-to-many fan-out, parked (SSOT). |

---

## Engines

Single raster core (libvips, with linked codec/delegate components) + one vector
rasteriser. **Per (source, target) pair exactly one engine runs** (§3.2): every image
pair is served end-to-end by **vips** (the SVG loader and the heif/cgif/ImageMagick
savers are libvips load/save modules, not separate pipeline stages), never a chain.

| Short | Engine | Role | Licence | Patent | Platforms |
|-------|--------|------|---------|--------|-----------|
| **vips** | **libvips** (raster core, built with libheif/libde265, libaom/dav1d, cgif, the **required** ImageMagick delegate, and the librsvg `svgload` module) | Decode+encode JPG/PNG/WEBP/GIF/BMP/TIFF/ICO; HEIC/AVIF **decode** (libheif / dav1d load modules) **and encode** (`heifsave compression=hevc\|av1`); BMP save via the required ImageMagick `magicksave` delegate; **ICO save = default `magicksave`, `[DEFER: build spike]` (in-core Rust ICO assembler fallback, §3.5.5)**; SVG load (librsvg); orchestrates resize/colour/alpha | LGPL-2.1+ (libvips); cgif MIT; ImageMagick permissive; **x265 GPL-2.0-or-later (dynamically-loaded libheif plugin)** | **HEVC patents → §3.4** (HEIC); AV1 royalty-free (AVIF, ship-posture §3.4) | Win / macOS / Linux (HEIC per §3.4 disposition) |
| **svg** | **librsvg** (libvips' native `svgload` backend; resvg is NOT a libvips backend and is **not shipped**, §3.1 row 1c) | Rasterise SVG → bitmap (no scripting, no network), **invoked as libvips' SVG load module** so vips saves the raster | librsvg LGPL-2.1+ | none | Win / macOS / Linux |

**Single-engine binding (resolves §3.2 for this category):**

1. **Raster ↔ raster** (JPG/PNG/WEBP/GIF/BMP/TIFF/ICO any-to-any) → **vips** (GIF
   save via native cgif; **BMP load+save via the required ImageMagick
   `magicksave`/`magickload` delegate** — libvips has no native BMP save, §3.1; **ICO save
   = default `magicksave`, `[DEFER: build spike]` with an in-core Rust ICO assembler
   fallback wrapping vips frames, §3.5.5**).
2. **→ HEIC** (any raster source) → **vips `heifsave compression=hevc`** (the x265
   libheif plugin). **HEIC →** any raster target → **vips** (libheif as the HEIC *load*
   module). **HEIC → AVIF** → **vips `heifsave compression=av1`** — one vips process.
3. **→ AVIF** (any raster source) → **vips `heifsave compression=av1`** (libaom via
   libheif — encode). **AVIF →** any raster target → **vips** (**dav1d** as the AVIF
   *load*/decode module). **Note `[DECIDED — configuration]`:** "libaom is encode-only"
   is a **build/configuration choice**, not a libaom limitation — libaom *can* decode AV1,
   but ConvertIA **configures libheif to resolve dav1d for AV1 decode** (smaller/faster
   decoder) and uses libaom **only** as the encoder. A §6.1.3 build assertion confirms the
   staged libheif resolves dav1d for decode (parallel to the libimagequant-soname
   assertion). Single binary.
4. **SVG → raster** → the image-worker loads the SVG via **`rsvg::Loader` directly**
   (NOT via libvips' `svgload` — `svgload` exposes no external-resource toggle, and the
   T9b LFR control requires loading with **no base URL**, §3.5.5), renders it, and
   **libvips performs the bitmap save**. Both librsvg and libvips live **inside the one
   image-worker process**, so the whole pair is still **one process** — this satisfies
   the no-chaining rule (the rasteriser is in-process with vips, not a separate pipeline
   stage we orchestrate). (libvips' native `svgload` module is itself librsvg-backed; we
   call librsvg directly only to guarantee the no-base-URL security path of §3.5.5.)

> **Single-engine note for HEIC/AVIF encode `[DECIDED — heifsave only]`.** ALL
> HEIC/AVIF *encoding* is done by **libvips `heifsave`** with its `compression`
> selector (`hevc` via the x265 libheif plugin, `av1` via libheif's libaom AV1
> encoder). So `raster→HEIC`, `raster→AVIF`, and the cross-codec `HEIC→AVIF` /
> `AVIF→HEIC` are each **one vips process** — every pair single-engine, one code path,
> and **only ONE AV1 encoder ships** (libaom). The standalone `heif`/`avif` CLI
> encoders are **not** bundled (dropped in [OPEN-1] [DECIDED]; see *Category-wide →
> [DECIDED]* and §3.4 / §3.5.5 / §3.6.1).

Patent dispositions (HEVC for HEIC, AV1 build/ship posture for AVIF) are **owned
by the §3.4 format × platform × disposition matrix** and are **not re-decided
here**. The per-format **patent flag** below points at §3.4; honest per-platform
availability (e.g. HEIC encode possibly *unavailable* on a platform with no
redistributable HEVC encoder) flows from that matrix, not from this file.

---

## Per-format entries

### JPG / JPEG
- **Detection:** magic `FF D8 FF` (SOI), then `E0`/`E1`/`EE`/`DB`… APPn markers;
  trailer `FF D9`. Extensions `.jpg .jpeg .jpe .jfif`. Unambiguous; a `.png`
  that is really JPEG is detected as JPEG (SSOT *Recognize by content*).
- **Role:** **both** (very common source and target).
- **As source → targets:** PNG, **WEBP★**, GIF, BMP, TIFF, HEIC, AVIF, ICO — all
  **vips** (HEIC/AVIF via `vips heifsave compression=hevc|av1`; there are no separate
  `heif`/`avif` engines). Lossy where the target is a lossy codec or a palette
  reduction (WEBP/GIF/HEIC/AVIF → §2.9; ICO → image_downscale, §2.9).
- **As target ← sources:** PNG, WEBP, GIF, BMP, TIFF, HEIC, AVIF, ICO, SVG.
  Always **vips** (`jpegsave`). Producing JPG **always flattens transparency**
  onto a background (JPEG has no alpha) → see *Edge cases*.
  **Lossy?:** a source **with alpha → JPG** is lossy by alpha-flatten →
  **`image_alpha_flatten`** (§2.9) — the predictable-loss note fires for any
  alpha-carrying source (PNG/WEBP/GIF/TIFF/HEIC/AVIF/ICO/SVG with transparency);
  baseline JPEG is also lossy by codec (`image_lossy_codec`, §2.9) at any Q.
- **Engine(s):** **vips** `jpegsave`. No patent flag (baseline JPEG is free).
- **Options/settings:**
  - *Basic:* **Quality `Q` — default `82`** (libvips lib default is 75; we raise
    to 82 as the "looks-clearly-good, still small" everyday default per the
    no-decision rule §1.6). Range 1–100. Exposed as a simple slider.
  - *Advanced:* `chroma subsampling` — default **auto** (vips disables subsampling
    at `Q ≥ 90` automatically; we keep auto); `progressive` — default **on**
    (smaller + nicer progressive load); `optimize_coding` (Huffman) — default
    **on**; background colour for flatten — default **white**.
- **Lossy?:** **JPEG is always lossy on save** (DCT). `→ JPG` lossy → §2.9. From
  a lossless source (PNG/BMP/TIFF) it is lossy; JPG→JPG re-encode (not offered,
  diagonal) would be generational loss.
- **Edge cases:** **EXIF orientation** is honoured — the image is **auto-rotated
  to upright pixels** and the orientation tag reset to 1 (so every downstream
  viewer shows it correctly; see *Category-wide → Orientation*). **ICC profile**
  is **preserved** (embedded into the output). EXIF/XMP/IPTC metadata preserved by
  default (§ metadata policy). 12-bit/CMYK JPEGs: decoded to 8-bit RGB. Truncated
  JPEG → fail clearly (§2.8).

### PNG
- **Detection:** magic `89 50 4E 47 0D 0A 1A 0A`. Extension `.png`. Unambiguous.
  APNG (animated PNG) = a PNG with `acTL` chunk before `IDAT`.
- **Role:** **both**. The everyday **lossless / transparency** workhorse.
- **As source → targets:** JPG, **WEBP★**, GIF, BMP, TIFF, HEIC, AVIF, ICO.
- **As target ← sources:** JPG, WEBP, GIF, BMP, TIFF, HEIC, AVIF, ICO, SVG —
  always **vips** (`pngsave`). PNG is the **default target** for sources whose
  natural home is lossless (GIF, BMP, TIFF, ICO, SVG).
- **Engine(s):** **vips** `pngsave`. No patent.
- **Options/settings:**
  - *Basic:* none required — PNG is lossless; default just works.
  - *Advanced:* `compression` 0–9 — default **6** (vips default; good size/speed);
    `interlace` (Adam7) — default **off**; `palette`/`bitdepth` quantisation —
    default **off** (true-colour). Optional `Q`/`effort` only apply when `palette`
    is on (palette PNG uses libimagequant) — default `Q 100`, `effort 7` *if*
    palette enabled.
- **Lossy?:** **Lossless** for PNG→PNG (n/a, diagonal) and as a *target* from any
  source — saving PNG never loses data. Becomes lossy only if `palette` is
  explicitly enabled (colour quantisation). `→ PNG` is therefore **not** flagged
  lossy in the matrix.
- **Edge cases:** **Transparency preserved** (RGBA). **APNG (animated PNG):** as a
  *source*, libvips can load APNG frames (`n=-1`); animation is **preserved only
  when the target also supports animation** (→ WEBP/GIF). For a still target the
  **first frame** is used (note surfaced like other animation-flatten cases). As a
  *target*, vips writing APNG is limited — **`[DECIDED]`** animated sources → PNG
  **collapse to the first frame** (APNG *output* not supported v1; see *Format-default
  decisions* item 3).
  16-bit PNG preserved through 16-bit-capable targets (TIFF/PNG), down-converted
  to 8-bit for 8-bit targets. ICC + text chunks preserved.

### WEBP
- **Detection:** RIFF container — `52 49 46 46` (`RIFF`) … `57 45 42 50` (`WEBP`)
  at offset 8; sub-chunk `VP8 ` (lossy) / `VP8L` (lossless) / `VP8X` (extended:
  alpha/animation). Extension `.webp`.
- **Role:** **both**. A **modern default target** (good compression + alpha +
  animation).
- **As source → targets:** **JPG★**, PNG, GIF, BMP, TIFF, HEIC, AVIF, ICO. JPG is
  the *default out of* WEBP because WEBP→JPG is the common "make it open
  everywhere" need (a normal person who has a `.webp` usually wants a JPG).
- **As target ← sources:** JPG, PNG, GIF, BMP, TIFF, HEIC, AVIF, ICO, SVG —
  **vips** (`webpsave`). WEBP is the **pre-highlighted default** *for JPG and PNG
  sources* (modern, smaller, keeps alpha) per the SSOT tie-breaker that allows a
  modern format when it is clearly the better everyday choice.
- **Engine(s):** **vips** `webpsave`. No patent (WEBP/VP8 is royalty-free).
- **Options/settings:**
  - *Basic:* **Quality `Q` — default `80`** (range 0–100; vips default 75, raised
    to 80 for a clean everyday result). `lossless` toggle — default **off**
    (lossy). When `lossless` on, `Q` is reinterpreted as effort/quality of the
    lossless coder.
  - *Advanced:* `effort` 0–6 — default **4** (vips default); `alpha_q` 1–100 —
    default **100** (full-quality alpha); `near_lossless` — default **off**;
    `smart_subsample` — default **off**; `min_size`/`mixed` (anim) — default off.
- **Lossy?:** **Lossy by default** (`→ WEBP` flagged lossy → §2.9); flip
  `lossless` to make WEBP→/→WEBP lossless. WEBP→JPG is lossy (JPEG). WEBP(lossy)→
  PNG is *not newly* lossy (PNG is lossless) but cannot recover detail already lost.
- **Edge cases:** **Transparency** (alpha) preserved to alpha-capable targets;
  flattened to background for JPG/BMP. **Animated WEBP** as source: frames loaded
  with `n=-1`; animation **preserved → GIF / animated-WEBP**, collapsed to **first
  frame** for still targets (note). ICC/EXIF preserved. Extended-format (`VP8X`)
  features handled by the loader.

### GIF
- **Detection:** magic `47 49 46 38 37 61` (`GIF87a`) or `47 49 46 38 39 61`
  (`GIF89a`). Extension `.gif`. Animation = multiple image descriptors + Graphics
  Control Extensions.
- **Role:** **both** (still or animated source; offered as a target because users
  sometimes specifically want a `.gif`, esp. to keep an animation).
- **As source → targets:** JPG, **PNG★**, WEBP, BMP, TIFF, HEIC, AVIF, ICO. PNG is
  the default (lossless, keeps transparency, universally openable). For an
  **animated** GIF, **animated WEBP** is the sensible "smaller, still animated"
  pick — but the *fixed* default stays PNG (first frame) for predictability;
  WEBP/GIF-passthrough keep the animation if the user chooses them.
- **As target ← sources:** JPG, PNG, WEBP, BMP, TIFF, HEIC, AVIF, ICO, SVG. GIF
  **save uses libvips' native `gifsave` (cgif backend, libvips ≥ 8.12)** `[DECIDED]`
  — **not** the ImageMagick delegate. This is one vips process, gives better GIF
  quality/size, and **removes ImageMagick from the GIF path** (cgif is MIT). The
  ImageMagick delegate (`magicksave`) is retained **only** as a compatibility
  fallback if a needed native saver is unavailable in the bundled vips build.
- **Engine(s):** **vips** (load built-in; **save via native `gifsave`/cgif**, vips
  ≥ 8.12; ImageMagick `magicksave` fallback only). No patent (LZW patent long
  expired). **Licence landmine cross-ref `[DECIDED]`:** the cgif `gifsave` palette path
  (and the palette-PNG path, line 188) depends on **libimagequant**, which **MUST be the
  BSD-2-Clause `lovell/libimagequant` v2.4.x fork — NEVER upstream libimagequant 4.x
  (GPLv3-or-commercial, which would taint the LGPL image-worker)**; the bundled libvips
  must build/link against that fork's API/soname (§3.1 row 1e owns this — version pin +
  §6.1.3/§6.3.3 COPYRIGHT-and-soname assertions).
- **Options/settings:**
  - *Basic:* none required. Palette is generated automatically.
  - *Advanced:* `dither` amount — default **on** (the native cgif/`gifsave` backend
    supports an **ordered/Bayer-style** dither, NOT Floyd–Steinberg — error-diffusion
    is not available in cgif; **bayer is the v1 default** per the README [DEFER]);
    `bitdepth`/colour count ≤ 256 — default **8** (256 colours); `effort` (palette
    search) — default **7** (vips default). `interframe maxerror`/`reuse` for
    animation — defaults left at vips defaults.
  - **Seam note — this is the *image*→GIF (cgif) path `[DECIDED]`.** The Bayer-only
    constraint applies to the **cgif `gifsave` save path** used here (raster image → GIF).
    The **video→GIF** path is a **different engine** (FFmpeg `palettegen`+`paletteuse`,
    cross-category.md), where **error-diffusion dither IS available** (`paletteuse=dither=
    sierra2_4a` etc.) — so the dither options differ by source category and must not be
    conflated. cross-category.md owns the video→GIF dither set; this section owns only the
    image→GIF cgif set.
- **Lossy?:** **Lossy as a target** (`→ GIF`) — 256-colour palette quantisation +
  optional dithering loses colour (→ §2.9). As a *source*, GIF→PNG/etc. is
  lossless w.r.t. the GIF's own pixels (GIF is already ≤256 colours), so GIF→PNG
  is **not** lossy.
- **Edge cases:** **Transparency:** GIF supports 1-bit (on/off) transparency only;
  preserved to PNG/WEBP (promoted to full alpha edge), flattened for JPG/BMP.
  **Animation:** preserved on GIF→WEBP and GIF→GIF (passthrough); for **still
  targets only the first frame** is taken (calm inline note "animated — only the
  first frame is converted"). Per-frame disposal/timing honoured by the loader.
  **→ video is out** (cross-category, not sanctioned).

### BMP
- **Detection:** magic `42 4D` (`BM`) + DIB header. Extension `.bmp .dib`.
  Note: very short/ambiguous magic — combine with header sanity (file size field).
- **Role:** **both** (common Windows source; an occasional target).
- **As source → targets:** JPG, **PNG★**, WEBP, GIF, TIFF, HEIC, AVIF, ICO.
- **As target ← sources:** JPG, PNG, WEBP, GIF, TIFF, HEIC, AVIF, ICO, SVG —
  **vips**.
- **Engine(s):** **vips**. *(libvips has **no native BMP support** — both BMP **load**
  (`magickload`) and BMP **save** (`magicksave`) go through the **required** ImageMagick
  delegate; still one vips process. ImageMagick is permissive, not GPL, §3.1 row 1d.)*
  No patent.
- **Options/settings:** none required (BMP is uncompressed). *Advanced:* none
  meaningful for v1 (no RLE toggle exposed).
- **Lossy?:** BMP's codec is uncompressed/lossless, so a no-alpha source `→ BMP` is
  **not lossy** (the source's bit depth is written out). **But a source WITH alpha →
  BMP is lossy by alpha-flatten** (v1 writes 24-bit BMP, §edge-cases) →
  **`image_alpha_flatten`** (§2.9) — the predictable-loss note fires for any
  alpha-carrying source (PNG/WEBP/GIF/TIFF/HEIC/AVIF/ICO/SVG with transparency).
- **Edge cases:** **Transparency:** classic BMP has no alpha; 32-bit BMP can carry
  alpha but support is patchy — v1 writes **24-bit BMP and flattens alpha** onto
  white (predictable, universally readable). Huge BMPs are large but fine (handled
  by §1.10 size pre-flight). Top-down vs bottom-up rows handled by loader.

### TIFF
- **Detection:** magic `49 49 2A 00` (`II*\0`, little-endian) or `4D 4D 00 2A`
  (`MM\0*`, big-endian). Extension `.tif .tiff`. (BigTIFF: `…2B 00`.) Multi-page
  via multiple IFDs.
- **Role:** **both** (scans/photography source; archival target).
- **As source → targets:** JPG, **PNG★**, WEBP, GIF, BMP, HEIC, AVIF, ICO.
- **As target ← sources:** JPG, PNG, WEBP, GIF, BMP, HEIC, AVIF, ICO, SVG —
  **vips** (`tiffsave`).
- **Engine(s):** **vips** `tiffsave`. No patent.
- **Options/settings:**
  - *Basic:* none required.
  - *Advanced:* `compression` — default **`deflate`** (lossless zip; good
    everyday balance). Choices: `none | jpeg | deflate | lzw | packbits | zstd`.
    `Q` (only if `compression=jpeg`) — default **82**; `predictor` — default
    **horizontal** for deflate/lzw; `tile` — default **off** (strip); `pyramid` —
    default **off**.
- **Lossy?:** TIFF *as a target* is **lossless by default** (deflate) → **not**
  flagged lossy; becomes lossy only if the user explicitly picks
  `compression=jpeg`. TIFF *as a source* loses nothing on decode.
- **Edge cases:** **Multi-page TIFF** as source: v1 converts the **first page**
  for still targets (per-page fan-out is parked); a calm note when >1 page. **16/32-bit
  and CMYK** TIFFs: preserved to 16-bit-capable targets (TIFF/PNG), else
  down-converted to 8-bit RGB. Alpha preserved to alpha-capable targets. ICC
  preserved. Float/scientific TIFFs (specialist) → still rasterised but flagged if
  out of normal range.

### HEIC / HEIF
- **Detection:** ISO-BMFF box `ftyp` at offset 4 with major/compatible brand
  `heic`/`heix`/`heif`/`mif1`/`heis`/`hevc` (bytes `66 74 79 70` then brand).
  Extensions `.heic .heif .hif`. **Patent-encumbered (HEVC) → §3.4.**
- **Role:** **both**, **subject to §3.4 per-platform availability** — on a
  platform where §3.4 says HEIC encode/decode is *unavailable* (no redistributable
  HEVC), the relevant direction is honestly surfaced as unavailable there (SSOT
  *v1 DoD* exception 1), **never silently dropped**.
- **As source → targets:** **JPG★** (the overwhelming "open my iPhone photo
  everywhere" need), PNG, WEBP, GIF, BMP, TIFF, AVIF, ICO. HEIC→* (to raster)
  runs in **vips** (libheif as load module); HEIC→AVIF via vips `heifsave
  compression=av1` (see single-engine note).
- **As target ← sources:** JPG, PNG, WEBP, GIF, BMP, TIFF, AVIF, ICO, SVG —
  **vips `heifsave compression=hevc`**. Included because some users want
  Apple-native HEIC, but **never a default** (compatibility-poor on non-Apple).
- **Engine(s):** **vips** end-to-end — `heifsave compression=hevc` for encode (via
  the **x265 libheif plugin**), and the libheif load module for HEIC→raster decode.
  **Patent flag → §3.4** (HEVC; x265 is GPL-2.0 → ships as a **dynamically-loaded
  libheif plugin**, never statically linked, per §3.6).
- **Options/settings:**
  - *Basic:* **Quality — default `60`** (range 0–100; libheif/x265 mid-quality;
    visually near-transparent for photos at far smaller size than JPEG).
  - *Advanced:* `lossless` — default **off**; **`effort` (integer 0–9, libvips
    `heifsave` param; NOT a `preset` string) — default `5`, but exposure is
    `[DEFER: corpus]`-GATED** (higher = slower/smaller). libvips `heifsave` has **no
    `preset` string** at the API level: it exposes the speed/size trade-off as the integer
    `effort`, which libvips maps to the libheif encoder `speed` setting (`speed = 9 -
    effort`). **x265-path caveat `[DECIDED — gate exposure, do not ship a dead control]`:**
    libvips currently documents `effort` as primarily honoured by the AV1 encoder; for the
    HEVC/x265 plugin path it may **not measurably steer** x265 on the bundled build.
    **Resolution (no-surprise UI):** the HEIC `effort` control is **exposed ONLY IF the
    `[DEFER: corpus]` spike confirms it measurably steers the bundled x265/HEVC path**; **if
    the corpus shows `effort` is inert for HEIC, the control is HIDDEN for HEIC targets** (the
    libheif x265 default applies silently — ConvertIA does **not** show a control that does
    nothing). This differs from **AVIF** `effort` (libvips-documented as honoured → stays
    exposed). The §6.1.3 `heifsave effort` capability assertion (arg exists) is necessary but
    not sufficient — the steer-confirmation is the corpus gate that decides exposure;
    `chroma` 4:2:0 default; bit depth 8 default (10-bit advanced).
- **Lossy?:** HEIC encode is **lossy by default** (`→ HEIC` flagged → §2.9; flip
  `lossless`). HEIC→JPG is lossy (JPEG). HEIC→PNG/TIFF is lossless w.r.t. the
  decoded pixels (but cannot recover the HEIC's own prior loss).
- **Edge cases:** **Live Photos / image sequences / depth / aux images:** v1
  converts the **primary image** only (the still). **HDR / 10-bit:** decoded and
  tone-mapped/down-converted to 8-bit SDR for 8-bit targets (note if HDR dropped).
  **Multi-image HEIF (bursts):** primary image only (fan-out parked).
  **EXIF/orientation** honoured (auto-upright). ICC preserved. Transparency rare in
  HEIC; preserved if present.

### AVIF
- **Detection:** ISO-BMFF `ftyp` with brand `avif` (single image) or `avis`
  (sequence). Bytes `66 74 79 70 61 76 69 66`/`…61 76 69 73`. Extension `.avif`.
  **Patent/AV1 ship-posture tracked in §3.4** (AV1 is royalty-free; the flag is for
  the §3.4 build/disposition decision, not a usage royalty).
- **Role:** **both**. A genuinely modern target (excellent compression, alpha,
  HDR, animation).
- **As source → targets:** **JPG★** (open-everywhere need), PNG, WEBP, GIF, BMP,
  TIFF, HEIC, ICO. AVIF→raster runs in **vips** (**dav1d** AVIF *decode* load module —
  libheif is configured to resolve dav1d for AV1 decode, using libaom only as the encoder,
  see the configuration note above); AVIF→HEIC via vips `heifsave compression=hevc`.
- **As target ← sources:** JPG, PNG, WEBP, GIF, BMP, TIFF, HEIC, ICO, SVG —
  **vips `heifsave compression=av1`** (libaom). May be the **default** *only* where the
  SSOT tie-breaker clearly favours a modern target — but for safe everyday
  compatibility we keep the per-source defaults at PNG/JPG/WEBP and offer AVIF as an
  explicit choice (not defaulted *from* anything in v1, to avoid handing users files
  that "don't open").
- **Engine(s):** **vips** end-to-end — `heifsave compression=av1` for encode (via
  libheif's **libaom** AV1 encoder — the single bundled AV1 encoder; standalone
  libavif dropped) and the **dav1d** load module for AVIF→raster decode (libaom is
  the encoder only; AVIF *decode* is dav1d, §3.1 row 1b). **Flag → §3.4.**
- **Options/settings:** (the engine is **libvips `heifsave compression=av1`**, whose
  exposed knobs are **`Q` / `effort` / `lossless`** — there is **no `cq-level`** on
  `heifsave` (that is a libaom/libavif-CLI concept ConvertIA does **not** expose); the
  internal libaom `cq-level` is derived from `Q` by libheif and is not a user control.)
  - *Basic:* **Quality (`Q`) — default `60`** (libvips `heifsave` default is 50;
    ConvertIA pins 60; range 0–100).
  - *Advanced:* **`effort` 0–9** — default **4** (libvips `heifsave` default for the AV1
    encoder; 0 = fastest, 9 = slowest/smallest); **`lossless`** — default **off**; bit
    depth 8 default (10/12 advanced); chroma 4:2:0 default. (No `speed`/`cq-level`
    controls — those are not `heifsave` parameters.)
- **Lossy?:** **Lossy by default** (`→ AVIF` flagged → §2.9; `lossless` available).
  AVIF→JPG lossy; AVIF→PNG/TIFF lossless w.r.t. decoded pixels.
- **Edge cases:** **Animated AVIF** (`avis`) source: animation preserved → GIF /
  animated WEBP; first frame for stills (note). **HDR / 10-12-bit / wide gamut:**
  tone-mapped/down-converted for 8-bit targets (note if HDR dropped).
  **Transparency** (alpha) preserved to alpha targets. ICC/EXIF preserved.

### ICO
- **Detection:** magic `00 00 01 00` (icon resource; `00 00 02 00` = CUR cursor,
  out of scope). Extension `.ico`. Contains **1..N images** at different sizes
  (16/32/48/256 …), each either BMP-style or an embedded PNG.
- **Role:** **both** (favicon/app-icon target; occasionally a source).
- **As source → targets:** JPG, **PNG★**, WEBP, GIF, BMP, TIFF, HEIC, AVIF. When
  an ICO holds several sizes, the **largest image** is selected as the source
  pixels (most useful), with the rest discarded (note if >1 size). **vips**.
- **As target ← sources:** JPG, PNG, WEBP, GIF, BMP, TIFF, HEIC, AVIF, SVG —
  **vips** (ICO save via the ImageMagick `magicksave` delegate **by default**, or the
  in-core Rust ICO assembler fallback — `[DEFER: build spike]`, §3.5.5/§6.1.3). The classic
  everyday use is **PNG/JPG/SVG → ICO** to make a favicon/app icon.
- **Engine(s):** **vips** (ICO load built-in; **ICO save `[DEFER: corpus/build spike]`** —
  the default path is the ImageMagick `magicksave` delegate (libvips has no native ICO
  saver), but ImageMagick's ICO encoder has documented trouble with **256px / multi-size**
  entries, so the multi-size-incl-256px capability is **unverified** until the §6.1.3 build
  spike confirms it (§3.5.5). **Fallback if the spike fails: an in-core Rust ICO container
  assembler** wrapping vips-produced per-size PNG/BMP frames — ICO is a trivial container,
  so this removes ImageMagick from the ICO path entirely while keeping vips as the per-frame
  encoder. Either way: one vips process for the frames. No patent.
- **Options/settings:**
  - *Basic:* **Icon sizes — default a standard multi-resolution set
    `[16, 32, 48, 256]`** (covers favicons + Windows app icons in one file). The
    source is downscaled to each (high-quality Lanczos); upscaling beyond the
    source is **skipped** (never invents detail) with a note if the source is
    smaller than a requested size.
  - *Advanced:* custom size list; `single size` mode; 256-px stored as **embedded
    PNG** (default on — required for the 256 entry to be valid/small).
- **Lossy?:** **`→ ICO` is lossy by downscaling** (multiple reduced copies) →
  **`image_downscale`** (§2.9 — NOT `image_palette`; ICO stores full-colour PNG/32-bit
  BMP entries, so there is no colour-depth reduction) — though each stored copy is
  itself losslessly stored (PNG/BMP). ICO→PNG (largest frame) is **not** lossy.
- **Edge cases:** **Transparency preserved** (ICO supports alpha via PNG/32-bit
  BMP entries). Non-square sources are letter-/pillar-boxed transparently or
  centred — **`[DECIDED]`** default: **pad to square with transparency** (don't crop,
  don't distort) — see *Format-default decisions* item 5. CUR files declined (out of scope).

### SVG (source only)
- **Detection:** text/XML — root `<svg` element (optionally after `<?xml …?>` /
  BOM / DOCTYPE / comments); MIME `image/svg+xml`. Extension `.svg` (`.svgz` =
  gzip-compressed SVG; supported by transparently gunzipping first). Content
  sniff, not extension, per SSOT.
- **Role:** **source only.** SVG-as-target (vector output) is reverse/
  reconstructive → **parked** (SSOT). The whole `* → SVG` column is **out**.
- **As source → targets:** **PNG★** (lossless, keeps the crisp edges + alpha),
  JPG, WEBP, BMP, TIFF, ICO. *(HEIC/AVIF technically possible but **out** — no
  everyday demand to rasterise a vector to HEIC/AVIF; kept off the offered set to
  stay uncluttered.)*
- **As target ← sources:** **none** (source-only).
- **Engine(s):** **svg** rasteriser (**librsvg**) — the image-worker loads the SVG via
  **`rsvg::Loader` directly** (not via libvips `svgload`, which exposes no external-resource
  toggle), renders it, then the bitmap is saved by **vips** — **one process** for the pair.
  No scripting, no external/`href` network fetch (offline + security: a remote `<image href>`
  is **not** fetched), **and no out-of-input local-file read** — the **load-bearing control
  is loading the SVG via `rsvg::Loader` with NO `base_file`/base URL** (`read_stream`/
  `from_data` without a base; v1 SVG→raster needs no external `<image>`/XInclude, fonts are
  bundled). With no base URL, librsvg has nothing to resolve a local/relative `href` against,
  so it refuses **all** local `<image href>`/XInclude reads by construction (and remote
  schemes regardless). **No base-URL/scratch confinement is used** — supplying any base URL
  is exactly what RE-ENABLES the CVE-2023-38633-class resolution surface (the defence is the
  *absence* of a base URL). **librsvg is pinned ≥ 2.56.3** as a belt-and-suspenders floor,
  not load-bearing for v1 (§3.5.5 / §6.1.3 version + API + corpus assertions). No patent.
- **Options/settings:**
  - *Basic:* **Output size.** Default render at the SVG's **intrinsic size** if it
    has explicit `width`/`height`; if it only has a `viewBox`, default to a sane
    **96 DPI** rasterisation of the viewBox (librsvg default DPI = 96).
    Common everyday control exposed: **target width in pixels** (height auto from
    aspect) — default = intrinsic; an "export at 2× / 3×" scale shortcut is offered.
  - *Advanced:* `scale`/`zoom` factor — default **1.0**; explicit `width`×`height`;
    `background` — default **transparent** (white when the target is JPG/BMP which
    have no alpha); `dpi` — default **96**.
- **Lossy?:** Rasterising is **inherently a one-way loss of vector scalability**
  (you bake to a pixel grid). We surface a calm note for SVG→raster ("vector →
  fixed-size image — picked size: WxH") → §2.9. On top of that, SVG→JPG/WEBP/GIF
  carries the target codec's own loss.
- **Edge cases:** **Transparency** preserved (PNG/WEBP/TIFF/ICO); flattened to
  background for JPG/BMP. **Fonts `[DECIDED]`:** SVG text is rendered with the
  **bundled font set (§3.9.3)** — **not** host OS fonts. The librsvg rasteriser runs
  **inside the image-worker process**, which has **no host-font access** (consistent with
  the offline/portable floor and the §2.12 isolation), so its fontconfig is pointed at the
  bundled Liberation/Carlito/Caladea + Noto subset. A glyph not in the bundled set
  substitutes (a predictable-loss font note may be surfaced) — the substitution is
  deterministic across machines, unlike host-font resolution. **Huge/zero
  intrinsic size:** if no size resolvable, fall back to viewBox @96 DPI; clamp a
  pathological render size against the §1.10 budget (a 1×1 viewBox asked to render
  at 50000 px fails clearly, not OOM). **Untrusted SVG** is decoded inside the
  §2.12 isolation boundary like every other decoder; no JS, no network.

---

## Category-wide

### Per-source default target (one-glance summary)

Every detected source has **exactly one** fixed, pre-highlighted default (SSOT
*How It Feels* 4; §1.5). Rationale follows the SSOT tie-breaker: widely-compatible
target unless a modern format is clearly the better everyday pick.

| Source | **Default target** | Why |
|--------|--------------------|-----|
| JPG | **WEBP** ✓★ | Modern, ~25–35 % smaller, keeps quality; clearly better everyday choice. |
| PNG | **WEBP** ✓★ | Smaller, keeps alpha; modern-better tie-break. |
| WEBP | **JPG** ✓★ | The user has a modern file and usually needs "open everywhere". |
| GIF | **PNG** ✓★ | Lossless, alpha, universal; (animated GIF→WEBP offered, not defaulted). |
| BMP | **PNG** ✓★ | Lossless + much smaller than BMP; universal. |
| TIFF | **PNG** ✓★ | Lossless, universal, smaller. |
| HEIC | **JPG** ✓★ | The canonical "open my iPhone photo everywhere" need. |
| AVIF | **JPG** ✓★ | "Open everywhere" — same as WEBP/HEIC rationale. |
| ICO | **PNG** ✓★ | Extract the usable bitmap losslessly. |
| SVG | **PNG** ✓★ | Lossless raster, keeps crisp edges + transparency. |

Note the deliberate asymmetry: **into** modern formats we default JPG/PNG → WEBP
(modern-better), but **out of** modern formats (WEBP/HEIC/AVIF) we default → JPG
(compatibility), because someone holding a modern file usually wants portability.
AVIF/HEIC are **never** a default *target* (handing a user a file that may not
open contradicts "it just works").

### Metadata policy (EXIF / XMP / IPTC)
- **Default: preserve** descriptive metadata (EXIF, XMP, IPTC) when the target
  supports it (JPG/WEBP/TIFF/HEIC/AVIF/PNG-text). GIF/BMP/ICO carry none.
- **Orientation:** always **baked** — the image is rotated to upright pixels and
  the EXIF `Orientation` tag reset to `1`, so no viewer can re-rotate it wrongly.
  This is the one metadata field we normalise rather than passthrough.
- **`[DECIDED]` — GPS/privacy:** ConvertIA **preserves all** descriptive metadata
  (incl. GPS/location EXIF) by default, with an **Advanced "remove location/metadata"
  toggle**. It is a *local* tool (nothing uploaded), so stripping is not required for the
  offline guarantee, and silent metadata loss is the bigger surprise for archival; a user
  sharing a photo who wants GPS gone uses the explicit toggle. (See *Format-default
  decisions* item 4.)

### Colour-profile (ICC) policy
- **Default: preserve/embed** the source ICC profile into the output whenever the
  target supports embedded ICC (JPG/PNG/WEBP/TIFF/HEIC/AVIF). This keeps colours
  faithful across viewers.
- Wide-gamut (Display-P3, Adobe RGB) sources are **not force-converted to sRGB** in
  v1 (faithful passthrough); if a target/codec path cannot carry the profile, the
  pixels are converted into the embedded/working space so colours don't shift
  visibly. **`[DECIDED]`** an Advanced "convert to sRGB for max compatibility" toggle is
  **NOT in v1** (`[DEFER: post-v1]`, by demand) — faithful wide-gamut passthrough is the
  v1 default.
- CMYK sources (JPEG/TIFF) are converted to RGB for RGB-only targets (with the
  source profile if present, else a default CMYK profile).

### Transparency policy
- Alpha is **preserved** across alpha-capable targets (PNG, WEBP, GIF[1-bit],
  TIFF, HEIC, AVIF, ICO).
- For alpha-incapable targets (**JPG, BMP**) alpha is **flattened onto a
  background** — default **white** (Advanced: choose background colour). This is a
  predictable, calm inline note, not a blocker → **§2.9 `image_alpha_flatten`** (the
  canonical LossyKind that owns this disclosure; the JPG and BMP *As target* entries
  carry the matching `lossy: image_alpha_flatten` hook so the §6.7.1 Lane-A guard and
  the matrix below agree).

### Animation policy
- **Animated sources:** GIF, animated WEBP, APNG, animated AVIF (`avis`),
  multi-image HEIF.
- **Preserved** only when the chosen target is animation-capable: GIF↔WEBP (and
  GIF/WEBP passthrough). All animation→animation in v1 is **GIF or animated WEBP**
  as the destination.
- **Collapsed to the first frame** for every still target (JPG/PNG/BMP/TIFF/HEIC/
  AVIF-still/ICO) with a calm inline note: *"animated — only the first frame is
  converted"* (→ §2.9 catalog).
- **No image→video** (cross-category, not in the sanctioned set) and **no
  frame-by-frame fan-out** (parked).

### Large-image / resource limits
- Decode/encode runs inside the §2.12 isolation boundary; pixel-count and output-
  size estimates feed the §1.10 pre-flight (a doomed-for-RAM or doomed-for-disk
  item fails fast and clearly, batch continues). libvips' streaming model keeps
  even very large rasters within bounded memory; a pathological synthetic size
  (e.g. tiny SVG asked to render at 50 000 px) is rejected up front, never OOMs.

### Format-default decisions (resolved; only the corpus-gated items remain `[DEFER: corpus]`)
1. **HEIC/AVIF encode code-path — `[DECIDED]`: standardise on libvips `heifsave`.**
   *All* HEIC/AVIF *encoding* uses libvips `heifsave` (`compression=hevc` for HEIC
   via x265, `compression=av1` for AVIF via libheif's AV1 plugin → **libaom**). One
   code path; `HEIC↔AVIF` is trivially single-engine; and crucially **only ONE AV1
   encoder ships** (libaom, via libheif) — the standalone `libavif`+aom encoder is
   **not** bundled (it would duplicate an AV1 encoder for no v1 benefit). The
   standalone `heif`/`avif` CLI sidecars are dropped from v1. Decode-side binding via
   vips load modules was already settled. (Cross-ref §3.4 / §3.5.5 / §3.6.1.)
2. **§3.4 patent dispositions** (HEVC for HEIC; AV1 ship-posture for AVIF) per
   platform — **owned by §3.4**, referenced here. Per-platform HEIC availability
   (and any "unavailable on platform X" honest surfacing) flows from that table.
3. **APNG output — `[DECIDED]`: collapse to first frame for PNG.** Animated sources →
   PNG collapse to the first frame; "keep the animation" routes to WEBP/GIF. (Animated
   PNG *input* is supported.) Rationale: libvips APNG *write* is limited, and PNG is the
   "single still image" everyday target — animation belongs on WEBP/GIF.
4. **EXIF GPS / location stripping default — `[DECIDED]`: preserve-all + Advanced strip
   toggle.** v1 preserves metadata (incl. GPS) by default and offers an Advanced
   "strip location/metadata" toggle (see Metadata policy). Rationale: silent metadata
   loss is the bigger surprise; stripping is an explicit, opt-in privacy action.
5. **ICO non-square padding default — `[DECIDED]`: pad to square with transparency**
   (no crop, no distort). Rationale: padding never discards image content, whereas
   centre-crop silently drops pixels.
6. **`heifsave effort` for HEIC encode** — integer 0–9 (libvips param; NOT a `preset`
   string). v1 default `effort 5` `[DECIDED]`; `[DEFER: corpus]` whether to lower to
   `effort 3` for batch speed (and whether the bundled libheif/x265 path measurably
   honours `effort` — libvips documents it as primarily an AV1 lever; the HEVC steer
   flows through libheif `speed = 9 - effort`). Revisit against batch timing (§3.8).
7. **JPG default Q = 82 / WEBP default Q = 80 / HEIC&AVIF default Q = 60 —
   `[DEFER: corpus]`.** These reasoned everyday defaults (above the bare-library
   defaults) are the v1 starting values; the only residual is confirming them against
   the real-photo corpus (SSOT *v1 DoD* reliability gate) before locking §1.6 — a
   measured calibration, not an open design call.
