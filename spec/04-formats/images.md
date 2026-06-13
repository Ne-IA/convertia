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

Engine short-names: **vips** = libvips raster core, **heif** = libheif (HEVC/x265),
**avif** = libavif (AV1), **svg** = SVG rasteriser (resvg/librsvg) **invoked via
libvips' SVG loader** (so the *raster save* stays in vips — still one engine for
the pair). See *Engines* for the binding.

| src ＼ tgt | JPG | PNG | WEBP | GIF | BMP | TIFF | HEIC | AVIF | ICO |
|-----------|-----|-----|------|-----|-----|------|------|------|-----|
| **JPG**   | —          | ✓ vips      | ✓★~ vips     | ✓~ vips      | ✓ vips      | ✓ vips      | ✓~ heif      | ✓~ avif      | ✓ vips |
| **PNG**   | ✓~ vips    | —           | ✓★~ vips     | ✓~ vips      | ✓ vips      | ✓ vips      | ✓~ heif      | ✓~ avif      | ✓ vips |
| **WEBP**  | ✓★~ vips   | ✓ vips      | —            | ✓~ vips      | ✓ vips      | ✓ vips      | ✓~ heif      | ✓~ avif      | ✓ vips |
| **GIF**   | ✓~ vips    | ✓★ vips     | ✓~ vips      | —            | ✓ vips      | ✓ vips      | ✓~ heif      | ✓~ avif      | ✓ vips |
| **BMP**   | ✓~ vips    | ✓★ vips     | ✓~ vips      | ✓~ vips      | —           | ✓ vips      | ✓~ heif      | ✓~ avif      | ✓ vips |
| **TIFF**  | ✓~ vips    | ✓★ vips     | ✓~ vips      | ✓~ vips      | ✓ vips      | —           | ✓~ heif      | ✓~ avif      | ✓ vips |
| **HEIC**  | ✓★~ vips/heif | ✓ vips/heif | ✓~ vips/heif | ✓~ vips/heif | ✓ vips/heif | ✓ vips/heif | —          | ✓~ avif      | ✓ vips/heif |
| **AVIF**  | ✓★~ vips/avif | ✓ vips/avif | ✓~ vips/avif | ✓~ vips/avif | ✓ vips/avif | ✓ vips/avif | ✓~ heif    | —            | ✓ vips/avif |
| **ICO**   | ✓~ vips    | ✓★ vips     | ✓~ vips      | ✓~ vips      | ✓ vips      | ✓ vips      | ✓~ heif      | ✓~ avif      | —      |
| **SVG**†  | ✓~ svg     | ✓★ svg      | ✓ svg        | ✓~ svg       | ✓ svg       | ✓ svg       | ✓~ heif*     | ✓ avif*      | ✓ svg  |

† **SVG is source-only.** It is rasterised once (vector → pixels) and that bitmap
is saved to the target; the rasterise step is inherently lossy *to a fixed pixel
grid* (you lose infinite scalability), independent of the target codec — see the
SVG entry. The `✓~` cells additionally carry the target codec's own lossiness.

\* SVG→HEIC / SVG→AVIF: the rasterised pixels are handed to heif/avif. Listed for
matrix completeness, but see *Category-wide → SVG default* — these are **low
demand** and are **out** unless a user explicitly picks them; the offered set for
SVG is PNG/JPG/WEBP/(BMP/TIFF/ICO). Marked `out` in the SVG entry.

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

Single raster core + two codec sidecars + one vector rasteriser. **Per (source,
target) pair exactly one engine runs** (§3.2). HEIC/AVIF decode *into* the raster
core for non-HEIC/non-AVIF targets, and the dedicated codec encodes *out* for
HEIC/AVIF targets — each such pair is still served end-to-end by **one** binary
(see the binding rules below), never a vips→heif chain.

| Short | Engine | Role | Licence | Patent | Platforms |
|-------|--------|------|---------|--------|-----------|
| **vips** | **libvips** (raster core) | Decode+encode JPG/PNG/WEBP/GIF/BMP/TIFF/ICO; SVG load (via its SVG load module); orchestrates resize/colour/alpha | LGPL-2.1+ | none for these codecs | Win / macOS / Linux |
| **heif** | **libheif** + **x265** (HEVC encode) / built-in HEVC decode | Encode HEIC; decode HEIC | libheif LGPL-3.0; **x265 GPL-2.0** | **HEVC patents → §3.4** | per §3.4 disposition |
| **avif** | **libavif** + **aom** (or dav1d decode) | Encode AVIF (AV1); decode AVIF | BSD-2-Clause / aom BSD | AV1 royalty-free, **but ship-disposition tracked in §3.4** | Win / macOS / Linux |
| **svg** | **resvg** (preferred) *or* **librsvg** | Rasterise SVG → bitmap (no scripting, no network) | resvg MPL-2.0 / librsvg LGPL-2.1+ | none | Win / macOS / Linux |

**Single-engine binding (resolves §3.2 for this category):**

1. **Raster ↔ raster** (JPG/PNG/WEBP/GIF/BMP/TIFF/ICO any-to-any) → **vips**.
2. **→ HEIC** (any raster source) → **heif** (heif decodes nothing here; it
   receives RGBA from the loader and encodes). **HEIC →** any raster target →
   handled by **vips** (vips links libheif as its HEIC *load* module), so the pair
   is one binary (vips) end-to-end. **HEIC → AVIF** is the one cross-codec pair →
   served by **avif** (avif tool decodes HEIC? no — see note) → **DECISION below**.
3. **→ AVIF** (any raster source) → **avif**. **AVIF →** any raster target →
   **vips** (libavif as vips' AVIF load module). Single binary.
4. **SVG → raster** → **svg** rasteriser invoked **through libvips' SVG loader**;
   libvips performs the bitmap save. Because libvips bundles the SVG load module
   that wraps resvg/librsvg, the whole pair is one process (vips) — this satisfies
   the no-chaining rule (the rasteriser is a *load module of* vips, not a separate
   pipeline stage we orchestrate).

> **Single-engine note for the two cross-codec pairs `HEIC→AVIF` and `AVIF→HEIC`.**
> These cannot be done by a raster-core decode + same-tool encode in one binary
> without that binary linking *both* libheif and libavif. Resolution: route both
> through **libvips**, which can be built with **both** libheif (HEIC load) and a
> heif/avif *save* module (libheif also encodes AVIF via its AV1 plugin, and
> libvips' `heifsave` takes a `compression` selector = `hevc | av1`). So
> `HEIC→AVIF` and `AVIF→HEIC` are **one vips process** using `heifsave
> compression=av1|hevc`. This keeps every pair single-engine. The standalone
> `heif`/`avif` CLI sidecars remain available for `raster→HEIC` / `raster→AVIF`
> where they give better encoder control; **[OPEN]** whether to standardise on
> vips' `heifsave`/`heifsave compression=av1` for *all* HEIC/AVIF encodes (one
> code path, simpler) versus the standalone encoders (more knobs). See
> *Category-wide → [OPEN]*.

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
  **vips** except HEIC (**heif**) / AVIF (**avif**). Lossy where the target is a
  lossy codec or a palette reduction (WEBP/GIF/HEIC/AVIF → §2.9).
- **As target ← sources:** PNG, WEBP, GIF, BMP, TIFF, HEIC, AVIF, ICO, SVG.
  Always **vips** (`jpegsave`). Producing JPG **always flattens transparency**
  onto a background (JPEG has no alpha) → see *Edge cases*.
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
  *target*, vips writing APNG is limited — **[OPEN]** whether APNG *output* is
  supported or animated sources →PNG collapse to first frame (see *Category-wide*).
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
  expired).
- **Options/settings:**
  - *Basic:* none required. Palette is generated automatically.
  - *Advanced:* `dither` — default **on** (Floyd–Steinberg, better gradients);
    `bitdepth`/colour count ≤ 256 — default **8** (256 colours); `effort` (palette
    search) — default **7** (vips default). `interframe maxerror`/`reuse` for
    animation — defaults left at vips/IM defaults.
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
- **Engine(s):** **vips**. *(libvips BMP load is built-in; BMP **save** uses the
  native saver where the bundled vips build provides it, else the ImageMagick
  `magicksave` fallback — still one vips process.)* No patent.
- **Options/settings:** none required (BMP is uncompressed). *Advanced:* none
  meaningful for v1 (no RLE toggle exposed).
- **Lossy?:** BMP is uncompressed/lossless → **`→ BMP` is not lossy**; the source's
  bit depth is written out. (A source that had alpha loses it only if writing a
  legacy 24-bit BMP — see edge cases.)
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
  **heif** encode (x265). Included because some users want Apple-native HEIC, but
  **never a default** (compatibility-poor on non-Apple).
- **Engine(s):** **heif** (libheif + x265 encode; built-in HEVC decode) for
  encode; **vips** (libheif load module) for HEIC→raster decode. **Patent flag →
  §3.4** (HEVC; x265 is GPL-2.0 → ships as a separate invoked binary per §3.6).
- **Options/settings:**
  - *Basic:* **Quality — default `60`** (range 0–100; libheif/x265 mid-quality;
    visually near-transparent for photos at far smaller size than JPEG).
  - *Advanced:* `lossless` — default **off**; `preset`/x265 `preset` — default
    **slow** (libheif default; good quality, acceptable speed) — **[OPEN]** may
    drop to `medium` for speed on large batches; `chroma` 4:2:0 default; bit depth
    8 default (10-bit advanced).
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
  TIFF, HEIC, ICO. AVIF→raster runs in **vips** (libavif load module); AVIF→HEIC
  via vips `heifsave compression=hevc`.
- **As target ← sources:** JPG, PNG, WEBP, GIF, BMP, TIFF, HEIC, ICO, SVG —
  **avif** encode (aom). May be the **default** *only* where the SSOT tie-breaker
  clearly favours a modern target — but for safe everyday compatibility we keep
  the per-source defaults at PNG/JPG/WEBP and offer AVIF as an explicit choice (not
  defaulted *from* anything in v1, to avoid handing users files that "don't open").
- **Engine(s):** **avif** (libavif + aom encode; dav1d/aom decode) for encode;
  **vips** (libavif load module) for AVIF→raster. **Flag → §3.4.**
- **Options/settings:**
  - *Basic:* **Quality — default `60`** (libavif default; range 0–100).
  - *Advanced:* `speed`/effort 0–10 — default **6** (libavif default balance;
    lower = slower/smaller); `lossless` — default **off**; `cq-level` (aom, 0–63)
    — derived from Quality unless overridden; bit depth 8 default (10/12 advanced);
    chroma 4:2:0 default.
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
  **vips** (ICO save via the ImageMagick delegate). The classic everyday use is
  **PNG/JPG/SVG → ICO** to make a favicon/app icon.
- **Engine(s):** **vips** (load built-in; **save via the native ICO saver where the
  bundled vips build provides one, else the ImageMagick `magicksave` delegate** —
  ICO multi-size assembly is the case most likely to still use the delegate). One
  vips process either way. No patent.
- **Options/settings:**
  - *Basic:* **Icon sizes — default a standard multi-resolution set
    `[16, 32, 48, 256]`** (covers favicons + Windows app icons in one file). The
    source is downscaled to each (high-quality Lanczos); upscaling beyond the
    source is **skipped** (never invents detail) with a note if the source is
    smaller than a requested size.
  - *Advanced:* custom size list; `single size` mode; 256-px stored as **embedded
    PNG** (default on — required for the 256 entry to be valid/small).
- **Lossy?:** **`→ ICO` is lossy by downscaling** (multiple reduced copies) →
  §2.9 — though each stored copy is itself losslessly stored (PNG/BMP). ICO→PNG
  (largest frame) is **not** lossy.
- **Edge cases:** **Transparency preserved** (ICO supports alpha via PNG/32-bit
  BMP entries). Non-square sources are letter-/pillar-boxed transparently or
  centred — **[OPEN]** default: **pad to square with transparency** (don't crop,
  don't distort) — flagged in *Category-wide*. CUR files declined (out of scope).

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
- **Engine(s):** **svg** rasteriser (**resvg** preferred; **librsvg** fallback)
  invoked **as libvips' SVG load module**, then the bitmap is saved by **vips** —
  **one process** for the pair. No scripting, no external/`href` network fetch
  (offline + security: a remote `<image href>` is **not** fetched). No patent.
- **Options/settings:**
  - *Basic:* **Output size.** Default render at the SVG's **intrinsic size** if it
    has explicit `width`/`height`; if it only has a `viewBox`, default to a sane
    **96 DPI** rasterisation of the viewBox (resvg/librsvg default DPI = 96).
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
  background for JPG/BMP. **Fonts:** resvg/librsvg use **system fonts**; a missing
  font substitutes (a font note may be surfaced — predictable loss). **Huge/zero
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
- **[OPEN] — GPS/privacy:** should ConvertIA **strip GPS** (and other
  location/serial EXIF) by default for privacy, given the SSOT privacy ethos? It is
  a *local* tool (nothing uploaded), so stripping is not required for the offline
  guarantee — but a user converting a photo to share may not expect GPS to ride
  along. Candidate default: **preserve all** (faithful, least surprising for
  archival) with an **Advanced "remove location/metadata" toggle**. Flagged for
  decision; not silently resolved.

### Colour-profile (ICC) policy
- **Default: preserve/embed** the source ICC profile into the output whenever the
  target supports embedded ICC (JPG/PNG/WEBP/TIFF/HEIC/AVIF). This keeps colours
  faithful across viewers.
- Wide-gamut (Display-P3, Adobe RGB) sources are **not force-converted to sRGB** in
  v1 (faithful passthrough); if a target/codec path cannot carry the profile, the
  pixels are converted into the embedded/working space so colours don't shift
  visibly. **[OPEN]** whether to offer an Advanced "convert to sRGB for max
  compatibility" — parked unless demand.
- CMYK sources (JPEG/TIFF) are converted to RGB for RGB-only targets (with the
  source profile if present, else a default CMYK profile).

### Transparency policy
- Alpha is **preserved** across alpha-capable targets (PNG, WEBP, GIF[1-bit],
  TIFF, HEIC, AVIF, ICO).
- For alpha-incapable targets (**JPG, BMP**) alpha is **flattened onto a
  background** — default **white** (Advanced: choose background colour). This is a
  predictable, calm inline note, not a blocker.

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

### [OPEN] items (genuine, not fake-resolved)
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
3. **APNG output:** is animated-PNG *output* supported, or do animated sources →
   PNG always collapse to first frame? (Animated PNG *input* is supported.)
   Current lean: collapse to first frame for PNG; route "keep the animation" to
   WEBP/GIF.
4. **EXIF GPS / location stripping default** (privacy) — see Metadata policy.
   Lean: preserve-all + Advanced strip toggle.
5. **ICO non-square padding default** — pad-to-square-with-transparency (no crop /
   no distort) vs centre-crop to square. Lean: pad with transparency.
6. **x265 `preset` for HEIC encode** — `slow` (quality) vs `medium` (batch speed)
   default; revisit against the reliability corpus / batch timing (§3.8).
7. **JPG default Q = 82 / WEBP default Q = 80 / HEIC&AVIF default Q = 60** — these
   are reasoned everyday defaults above the bare-library defaults; confirm against
   the real-photo corpus (SSOT *v1 DoD* reliability gate) before locking §1.6.
