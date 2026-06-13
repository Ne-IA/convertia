# 04 — Formats: Audio

> Category spec for audio. Formats (SSOT *What It Converts*): MP3, WAV, FLAC,
> AAC\*, M4A, OGG (Vorbis), OPUS, WMA, AIFF, ALAC. \*patent-encumbered →
> disposition in §3.4. Follows the per-format template in [README](README.md).
>
> **One engine for the whole category: FFmpeg.** (The single bundled FFmpeg binary
> is **GPL-2.0+** because it enables `libx264` for the video category, §3.1/§3.6.1 —
> it is the same binary across audio/video/cross-category, shipped as a separate
> invoked binary. The *audio* encoders it uses here are all LGPL/BSD; the GPL class
> comes from x264 in the shared binary.) Every audio
> source→target pair is satisfied by a single FFmpeg invocation (decode source →
> re-encode target), so the §3.2 single-engine-per-pair rule holds trivially and
> no pair is ever chained. The "extract audio from video" outputs (video →
> MP3/WAV/…) are **not** owned here — they live in
> [cross-category.md](cross-category.md) as an *operation* on a video source.
> This file covers **audio-file → audio-file** only.

---

## Intro: the two distinctions that drive this file

Two everyday-invisible distinctions decide most of the matrix and the defaults:

1. **Lossless vs lossy.** WAV, FLAC, AIFF and ALAC are **lossless** (bit-exact or
   perfectly reconstructable PCM). MP3, AAC, M4A(AAC), OGG/Vorbis, OPUS and WMA
   are **lossy** (audio is permanently discarded to save space). A
   lossy→lossless conversion (e.g. MP3→FLAC) **cannot recover** what the lossy
   step already threw away — the output is larger but **not** higher quality than
   its source. A lossy→lossy conversion (e.g. MP3→AAC) **decodes then re-encodes**
   and adds a *second* round of loss (transcoding/generation loss). Both of these
   are flagged in the matrix and disclosed per §2.9.

2. **Codec vs container.** A user picks a *format*, but a file is a **container**
   (the wrapper, e.g. M4A / OGG / WAV) holding an **audio codec** (the actual
   compression, e.g. AAC / Vorbis / PCM). This matters for three SSOT formats:
   - **AAC** — both a *codec* and, as a file, a raw **ADTS** stream (`.aac`). When
     a user picks "AAC" as a target ConvertIA writes a raw ADTS `.aac` file.
   - **M4A** — a *container* (MP4/iTunes flavour) that can hold **AAC** *or*
     **ALAC**. As a ConvertIA target, "M4A" means **M4A holding AAC** (the
     everyday meaning). M4A-holding-ALAC is offered under the **ALAC** target.
   - **ALAC** — a lossless *codec* that, as a file, lives **inside an M4A/MP4
     container** (`.m4a`). Picking "ALAC" produces an `.m4a` whose codec is ALAC.

ConvertIA never asks the user about containers/codecs; the target name fixes both
(table below). The distinction only shows up in detection (an `.m4a` can be AAC
or ALAC inside) and in the per-format notes.

---

## Source → target matrix

Rows = detected **source**, columns = chosen **target**. Cell legend:

- `✓` supported (FFmpeg) · `✓★` supported **and the pre-highlighted default
  target** for that source · `✓~` supported **but predictably lossy** (note via
  §2.9) · `✓★~` default **and** lossy · `—` not offered (see footnote) · self
  cell (diagonal) = re-encode same format, offered only where it has everyday
  meaning (re-compress), else `—`.

All cells are FFmpeg (the shared GPL-2.0+ binary; §3.6.1) — the engine column is omitted from the grid to
keep it readable and stated once per entry instead.

| src ＼ tgt | MP3 | WAV | FLAC | AAC | M4A | OGG | OPUS | AIFF | ALAC | WMA |
|-----------|-----|-----|------|-----|-----|-----|------|------|------|-----|
| **MP3**   | —   | ✓★  | ✓~   | ✓~  | ✓~  | ✓~  | ✓~   | ✓    | ✓~   | —   |
| **WAV**   | ✓★~ | —   | ✓    | ✓~  | ✓~  | ✓~  | ✓~   | ✓    | ✓    | —   |
| **FLAC**  | ✓★~ | ✓   | —    | ✓~  | ✓~  | ✓~  | ✓~   | ✓    | ✓    | —   |
| **AAC**   | ✓★~ | ✓   | ✓~   | —   | ✓   | ✓~  | ✓~   | ✓    | ✓~   | —   |
| **M4A**   | ✓★~ | ✓   | ✓~   | ✓   | —   | ✓~  | ✓~   | ✓    | ✓~   | —   |
| **OGG**   | ✓★~ | ✓   | ✓~   | ✓~  | ✓~  | —   | ✓~   | ✓    | ✓~   | —   |
| **OPUS**  | ✓★~ | ✓   | ✓~   | ✓~  | ✓~  | ✓~  | —    | ✓    | ✓~   | —   |
| **AIFF**  | ✓★~ | ✓   | ✓    | ✓~  | ✓~  | ✓~  | ✓~   | —    | ✓    | —   |
| **ALAC**  | ✓★~ | ✓   | ✓    | ✓~  | ✓~  | ✓~  | ✓~   | ✓    | —    | —   |
| **WMA**   | ✓★~ | ✓   | ✓~*  | ✓~  | ✓~  | ✓~  | ✓~   | ✓    | ✓~*  | —   |

Notes on the matrix:

- **WMA is source-only** (whole column `—`). FFmpeg's only usable WMA encoder is
  `wmav2` (low quality, max 2 channels, effectively legacy); no normal person
  asks to convert *into* WMA — WMA is a format people convert *away from*. Per the
  SSOT inclusion test and the forward/derivative-direction rule, `→ WMA` is **out
  of v1** (parked). WMA-as-source is fully supported (decode is solid).
- **WMA → FLAC / → ALAC** (`✓~*`): the source WMA is lossy, so the lossless target
  is honest storage but carries **no quality benefit** over the source — flagged
  lossy-origin (same §2.9 note as any lossy→lossless).
- **Diagonal (same→same) is `—`** for v1. "Re-compress an MP3 to a smaller MP3"
  has everyday demand in theory, but in v1 it is **parked**: it requires exposing
  a target bitrate as a *required* choice to be meaningful (otherwise it silently
  re-encodes at the default and just adds generation loss), which conflicts with
  the "no required choices" default model. Same-format re-encode is therefore not
  offered. *(If promoted later, it gets its own [OPEN] resolution — see
  Category-wide.)*
- **Lossy→lossless cells** (e.g. MP3→FLAC, AAC→ALAC) are supported because users
  genuinely want them (archival, importing into a lossless library, feeding a
  tool that only accepts FLAC), but they are flagged `✓~` and disclosed: the
  result is bigger, not better.
- Every non-`—` cell passes the SSOT inclusion test: each is a plausible everyday
  audio interchange (shrink for a phone, get a universally-playable MP3/WAV,
  archive losslessly, produce an Apple-friendly M4A/ALAC, get a small
  modern OPUS).

---

## Per-format entries

### `MP3`

- **Detection:** MPEG-1/2 Audio Layer III. Magic = MPEG audio frame sync
  `FF Fx` (commonly `FF FB`, `FF F3`, `FF F2`) **or** a leading `ID3` tag header
  (`49 44 33`, "ID3", ID3v2) before the first audio frame. Extension `.mp3`.
  Ambiguity: a bare frame-sync also matches MP1/MP2 — FFmpeg's probe distinguishes
  the layer; a file that is really MP2 is detected as such and treated as
  out-of-scope (not silently called MP3).
- **Role:** both.
- **As source → targets:** **WAV ★**, FLAC, AAC, M4A, OGG, OPUS, AIFF, ALAC.
  *(MP3→MP3 not offered — see diagonal note; the per-source default is WAV per the
  Category-wide table, [OPEN/DEFER]: WAV-vs-FLAC.)* All conversions decode MP3 to PCM
  first, so every target inherits the source's already-lost detail.
- **As target ← sources:** WAV, FLAC, AAC, M4A, OGG, OPUS, AIFF, ALAC, WMA
  (**MP3 is the default target of every other audio source** — the universally
  compatible everyday choice).
- **Engine:** FFmpeg, encoder **`libmp3lame`** (LAME, LGPL — bundled in the shared FFmpeg binary). All
  platforms. No patent flag for ConvertIA's purposes (MP3 patents expired 2017).
- **Options/settings:**
  - *Default (no choice):* **VBR quality `-q:a 2`** (LAME `-V2`, ≈170–210 kb/s
    stereo) — perceptually transparent for everyday use, smaller than CBR 320. The
    default is VBR, not CBR.
  - *Advanced — "MP3 quality":* a small preset set mapping to LAME VBR/CBR:
    `High (V0, ≈245k)` · `Standard (V2, ≈190k)` **[default]** · `Small (V5,
    ≈130k)` · plus explicit CBR `128k / 192k / 320k` for users who need a fixed
    bitrate. Implemented as `-q:a N` (VBR presets) or `-b:a Nk` (CBR presets).
  - Sample rate / channels: **preserved from source by default** (no resample, no
    down/up-mix); an Advanced "force 44.1 kHz / stereo" is *not* exposed in v1.
- **Lossy?:** **Always lossy as a target** (LAME re-encode). As a source it is
  already lossy, so any further conversion is lossy-origin. Disclosure → §2.9.
- **Edge cases:** ID3v1/ID3v2 tags (title/artist/album/year/genre/track/cover
  art) are carried to targets that support tags (§ Category-wide); VBR-header
  (Xing/LAME) is regenerated by the encoder. Decoder-padding/encoder-delay gaps
  are not gap-trimmed (acceptable for everyday use). CBR vs VBR of the *source* is
  irrelevant — we always decode to PCM.

### `WAV`

- **Detection:** RIFF/WAVE PCM container. Magic = `52 49 46 46` ("RIFF") at byte
  0, then `57 41 56 45` ("WAVE") at byte 8. Extension `.wav` (`.wave`).
  Sub-format is read from the `fmt ` chunk (PCM `0x0001`, IEEE float `0x0003`,
  WAVE_FORMAT_EXTENSIBLE `0xFFFE`). Note: a `.wav` can technically wrap a non-PCM
  codec (rare); FFmpeg decodes whatever the `fmt ` chunk declares.
- **Role:** both.
- **As source → targets:** MP3 ★, FLAC, AAC, M4A, OGG, OPUS, AIFF, ALAC.
- **As target ← sources:** every other audio format (the universal **lossless
  uncompressed** interchange — picked when a user needs raw audio for editing or a
  tool that only eats WAV).
- **Engine:** FFmpeg, encoder **`pcm_s16le`** (default) via the **`wav`** muxer.
  All platforms. No patent flag.
- **Options/settings:**
  - *Default:* **16-bit PCM little-endian (`pcm_s16le`)**, sample rate and channels
    preserved from source. 16-bit is the everyday CD-quality default and keeps
    files from ballooning vs 24/32-bit.
  - *Advanced — "WAV bit depth":* `16-bit [default]` · `24-bit (pcm_s24le)` ·
    `32-bit float (pcm_f32le)` — for users feeding pro audio tools. No bitrate
    concept (uncompressed).
  - Sample rate / channels preserved; no resample/down-mix by default.
- **Lossy?:** **Lossless as a target** (PCM). *Caveat:* a 24-bit or float source
  → default **16-bit** WAV is a **bit-depth reduction = lossy** in the strict
  sense; flagged only in that specific case (§2.9), not for the common
  16-bit→16-bit path.
- **Edge cases:** WAV has weak native metadata (LIST/INFO chunk) — FFmpeg maps the
  common tags it can; rich tags from a tagged source may not survive into WAV (see
  Category-wide metadata policy). Files >4 GB exceed classic WAV's 32-bit size
  fields — FFmpeg can write RF64, but a normal everyday file never reaches this.

### `FLAC`

- **Detection:** Free Lossless Audio Codec, native FLAC stream. Magic = `66 4C 61
  43` ("fLaC") at byte 0. Extension `.flac`. Unambiguous.
- **Role:** both.
- **As source → targets:** MP3 ★, WAV, AAC, M4A, OGG, OPUS, AIFF, ALAC.
- **As target ← sources:** every other audio format (the everyday **lossless
  compressed** archive choice; from a lossy source it is honest storage with no
  quality gain — flagged).
- **Engine:** FFmpeg, encoder **`flac`** (native, all platforms). No patent flag.
- **Options/settings:**
  - *Default:* **compression level 5** (`-compression_level 5`) — FFmpeg's own
    default, the standard speed/size balance. Compression level changes **size and
    speed only, never the audio** (FLAC is lossless at every level).
  - *Advanced — "FLAC compression":* `Fast (0)` · `Standard (5) [default]` ·
    `Best/smallest (8)` (0–12 valid; we cap the exposed choice at 8 — 9–12 are
    marginal and slow).
  - Sample rate / bit depth / channels preserved exactly from source.
- **Lossy?:** **Lossless as a target.** Lossy *origin* (e.g. MP3→FLAC) is flagged
  as no-quality-gain (§2.9), not as further loss.
- **Edge cases:** FLAC carries **Vorbis comments** (rich tags) + embedded
  **picture** block (cover art) — tags and cover art round-trip well to/from OGG,
  OPUS, M4A. FLAC supports up to 8 channels and high bit depths; all preserved.

### `AAC`

- **Detection:** Advanced Audio Coding, **raw ADTS** stream. Magic = ADTS sync
  word `FF Fx` where the low nibble encodes MPEG-version + protection-absent bits:
  **`FF F1` = MPEG-4, no CRC**; **`FF F9` = MPEG-2, no CRC**; `FF F0` / `FF F8` =
  the with-CRC variants (protection present). All are valid ADTS. Extension `.aac`.
  **Disambiguation:** an
  AAC *codec* inside an MP4 container is an **M4A**, not this — `.aac` here means
  the raw ADTS elementary stream. Bare `FF Fx` overlaps MP3 frame sync; FFmpeg's
  probe resolves ADTS-vs-MP3 by parsing the header fields.
- **Role:** both.
- **As source → targets:** MP3 ★, WAV, FLAC, M4A, OGG, OPUS, AIFF, ALAC.
- **As target ← sources:** every other audio format (the small, modern, widely
  playable lossy choice when MP3 isn't specifically wanted).
- **Engine:** FFmpeg, **native `aac` encoder** (FFmpeg's built-in encoder —
  **license-clean, no `--enable-nonfree`/libfdk_aac**). Muxer = **`adts`**
  (writes raw `.aac`). All platforms.
- **Patent flag:** ⚠ **AAC is patent-encumbered → disposition decided in §3.4**
  (format × platform × ship/gate/rely-on-OS/unavailable). ConvertIA references
  that matrix; it does **not** re-decide here. *If* §3.4 marks AAC unavailable on
  a platform, both the **AAC and M4A(AAC) targets** and AAC-related decode on that
  platform follow that disposition and are honestly surfaced as unavailable there
  (per SSOT v1-DoD first exception). (Note: FFmpeg's *native* AAC encoder is
  itself license-clean LGPL; §3.4 owns whether the *patent* situation gates it.)
- **Options/settings:**
  - *Default:* **CBR `-b:a 192k`** (LC-AAC, default profile). 192 kb/s AAC is
    perceptually strong and small; the native encoder's VBR mode (`-q:a` /
    `-vbr`) is **experimental/unstable** per FFmpeg docs, so ConvertIA uses CBR
    for AAC by default (deliberate divergence from the MP3/Vorbis VBR default).
  - *Advanced — "AAC quality":* CBR presets `128k` · `192k [default]` · `256k`.
    No VBR exposed (encoder limitation).
  - Sample rate / channels preserved.
- **Lossy?:** **Always lossy as a target.** Disclosure → §2.9.
- **Edge cases:** raw ADTS `.aac` carries **no metadata container** — tags from
  the source are **dropped** (ADTS has no tag frames); this is itself a predictable
  loss noted at the AAC target (§2.9 `audio_tags_dropped`). Users who want AAC *with*
  tags should pick **M4A**. HE-AAC / AAC+ profiles are not produced (native
  encoder = LC only); a normal user never needs them.

### `M4A`

- **Detection:** MPEG-4 audio container (MP4 flavour, iTunes/Apple). Magic = `ftyp`
  box: bytes 4–7 = `66 74 79 70` ("ftyp"), major brand `M4A ` / `mp42` / `isom` /
  `dash`. Extension `.m4a` (also `.mp4` used for audio-only). **Codec inside is
  read from the `stsd`/`esds` atom:** an `.m4a` holds **AAC** (the common case) or
  **ALAC**. ConvertIA reports the user-facing format as **M4A** when the codec is
  AAC and as **ALAC** when the codec is ALAC (so the offered targets and lossy
  flag are correct).
- **Role:** both.
- **As source → targets:** MP3 ★, WAV, FLAC, AAC, OGG, OPUS, AIFF, ALAC.
  (An M4A-holding-AAC source → "AAC" target = same codec, re-wrapped to raw ADTS;
  → "ALAC" target = transcode AAC→ALAC, lossy-origin.)
- **As target ← sources:** every other audio format. **"M4A" target = M4A
  container holding AAC** (the Apple-ecosystem-friendly lossy choice that *keeps
  tags*, unlike raw `.aac`).
- **Engine:** FFmpeg, encoder **native `aac`**, muxer **`ipod`** (writes `.m4a`).
  All platforms.
- **Patent flag:** ⚠ **inherits AAC's §3.4 disposition** (the codec is AAC). M4A
  target availability per platform = AAC's availability per §3.4. *(An M4A holding
  ALAC is offered as the ALAC target, which is patent-free.)*
- **Options/settings:** identical to AAC (CBR `-b:a 192k` default; Advanced
  `128k/192k/256k`). The only difference from the AAC target is the **container**
  (`.m4a` with iTunes metadata atoms) — chosen automatically, not a user setting.
- **Lossy?:** **Always lossy as a target** (AAC re-encode). Disclosure → §2.9.
- **Edge cases:** M4A **keeps metadata** (iTunes `ilst` atoms: title/artist/album/
  cover art) — this is M4A's advantage over raw AAC. Cover art round-trips from
  FLAC/MP3/OGG. Faststart (`-movflags +faststart`) is applied so the moov atom is
  at the front (instant playback). M4A-as-source that is actually ALAC is handled
  by the ALAC entry.

### `OGG` (Vorbis)

- **Detection:** Ogg container; for this format the codec is **Vorbis**. Magic =
  `4F 67 67 53` ("OggS") at byte 0; the first page's codec ID = `\x01vorbis`
  (`01 76 6F 72 62 69 73`). Extension `.ogg` (`.oga`). **Disambiguation:** an Ogg
  page can also carry **Opus** (`OpusHead`) or FLAC — ConvertIA reads the codec
  ID and reports **OGG** only for Vorbis, **OPUS** for Opus (so they are distinct
  user-facing formats and separate batches per SSOT batch rule).
- **Role:** both.
- **As source → targets:** MP3 ★, WAV, FLAC, AAC, M4A, OPUS, AIFF, ALAC.
- **As target ← sources:** every other audio format (the open, lossy choice;
  modest everyday demand but a real "I need an .ogg" case).
- **Engine:** FFmpeg, encoder **`libvorbis`**, muxer **`ogg`**. All platforms.
  No patent flag (Vorbis is royalty-free).
- **Options/settings:**
  - *Default:* **VBR quality `-q:a 3`** (libvorbis quality 3.0 ≈ 112 kb/s) —
    FFmpeg/Vorbis's own default; Vorbis is quality-based and VBR by nature.
  - *Advanced — "OGG quality":* `q3 (≈112k) [default]` · `q5 (≈160k)` ·
    `q7 (≈224k)` (Vorbis quality scale −1…10; we expose the useful middle).
  - Sample rate / channels preserved.
- **Lossy?:** **Always lossy as a target.** Disclosure → §2.9.
- **Edge cases:** carries **Vorbis comments** (rich tags) + cover art
  (METADATA_BLOCK_PICTURE) — round-trips with FLAC/OPUS/M4A. Chained/multiplexed
  Ogg streams are uncommon for plain audio and not specially handled.

### `OPUS`

- **Detection:** Opus codec in an Ogg container. Magic = `4F 67 67 53` ("OggS")
  then `OpusHead` (`4F 70 75 73 48 65 61 64`) in the first page. Extension
  `.opus` (sometimes `.ogg` — codec ID is authoritative, not the extension).
- **Role:** both.
- **As source → targets:** MP3 ★, WAV, FLAC, AAC, M4A, OGG, AIFF, ALAC.
- **As target ← sources:** every other audio format (the **modern, best-quality-
  per-byte** lossy choice — excellent for voice and music at low bitrate).
- **Engine:** FFmpeg, encoder **`libopus`**, muxer **`opus`** (`.opus`). All
  platforms. No patent flag (Opus is royalty-free / IETF RFC 6716).
- **Options/settings:**
  - *Default:* **VBR `-b:a 128k`** (libopus is VBR by default; bitrate is the
    natural Opus control — at 128k Opus is transparent for music). *(libopus's own
    no-bitrate default is 96k; ConvertIA pins 128k as a slightly safer everyday
    music default.)*
  - *Advanced — "OPUS bitrate":* `96k (voice/small)` · `128k [default]` ·
    `192k (high)`; `-vbr on` retained throughout.
  - Sample rate: Opus operates at 48 kHz internally — FFmpeg resamples the source
    to 48 kHz transparently; channels preserved.
- **Lossy?:** **Always lossy as a target.** Disclosure → §2.9.
- **Edge cases:** carries Vorbis-comment-style tags + cover art; round-trips with
  FLAC/OGG/M4A. The forced 48 kHz internal rate is standard Opus behaviour, not a
  user-visible loss for everyday content. Note SSOT *Fail clearly* downstream
  caveat: `.opus` may not open in older players — the default target is therefore
  never OPUS.

### `AIFF`

- **Detection:** Audio Interchange File Format (Apple, big-endian PCM). Magic =
  `46 4F 52 4D` ("FORM") at byte 0, then `41 49 46 46` ("AIFF") at byte 8 (or
  `41 49 46 43` "AIFC" for the compressed variant). Extension `.aiff` (`.aif`,
  `.aifc`). Structurally the big-endian RIFF analogue of WAV.
- **Role:** both.
- **As source → targets:** MP3 ★, WAV, FLAC, AAC, M4A, OGG, OPUS, ALAC.
- **As target ← sources:** every other audio format (the **lossless uncompressed**
  choice on the Apple/pro side, when WAV isn't specifically wanted).
- **Engine:** FFmpeg, encoder **`pcm_s16be`** (big-endian PCM, AIFF's native),
  muxer **`aiff`**. All platforms. No patent flag.
- **Options/settings:**
  - *Default:* **16-bit big-endian PCM**, sample rate / channels preserved.
  - *Advanced — "AIFF bit depth":* `16-bit [default]` · `24-bit (pcm_s24be)`.
  - No bitrate (uncompressed).
- **Lossy?:** **Lossless as a target.** Same 24/float-source → 16-bit caveat as
  WAV (bit-depth reduction = lossy in that one case; §2.9).
- **Edge cases:** AIFF metadata support is limited (ID3 chunk in some files,
  NAME/AUTH chunks) — FFmpeg maps what it can; rich tags may not fully survive
  (see Category-wide). AIFC (compressed AIFF) sources are decoded by codec like any
  container.

### `ALAC`

- **Detection:** Apple Lossless codec — **not a standalone file type**; it lives
  **inside an M4A/MP4 container** (`.m4a`) or rarely a CAF. Detection = the M4A
  `ftyp` signature **plus** codec ID `alac` in the `stsd` atom. Extension `.m4a`
  (occasionally `.caf`). ConvertIA reports this as the **ALAC** user-facing format
  (distinct from AAC-in-M4A which is reported as **M4A**), so the lossless flag and
  target set are correct.
- **Role:** both.
- **As source → targets:** MP3 ★, WAV, FLAC, AAC, M4A, OGG, OPUS, AIFF.
- **As target ← sources:** every other audio format. **"ALAC" target = `.m4a`
  whose codec is ALAC** — the **lossless** Apple-ecosystem archive choice (iTunes/
  Apple Music friendly, unlike FLAC which Apple historically didn't ingest).
- **Engine:** FFmpeg, encoder **`alac`** (native, lossless), muxer **`ipod`**
  (writes `.m4a`). All platforms. **No patent flag** (ALAC is open-sourced by
  Apple, royalty-free — *do not* confuse with AAC's §3.4 status; ALAC is clean).
- **Options/settings:**
  - *Default:* none required — ALAC is lossless, no quality/bitrate dial. Sample
    rate / bit depth / channels preserved exactly.
  - *Advanced:* none meaningful (no compression-level knob exposed by FFmpeg's ALAC
    encoder). The view stays clean.
- **Lossy?:** **Lossless as a target.** Lossy *origin* (e.g. MP3→ALAC) flagged as
  no-quality-gain (§2.9).
- **Edge cases:** uses the same MP4 `ilst` metadata + cover art as M4A — tags and
  art round-trip from FLAC/OGG/M4A. `+faststart` applied. Because both ALAC and
  AAC live in `.m4a`, the **as-source detection must read the codec atom** (done in
  detection §1.2), never trust the `.m4a` extension.

### `WMA`

- **Detection:** Windows Media Audio, inside an ASF container. Magic = ASF GUID
  header `30 26 B2 75 8E 66 CF 11 A6 D9 00 AA 00 62 CE 6C` at byte 0. Codec is read
  from the stream properties (WMA v1 `0x160`, WMA v2 `0x161`, WMA Pro `0x162`, WMA
  Lossless `0x163`). Extension `.wma`.
- **Role:** **source only** (see matrix note — `→ WMA` is parked/out-of-v1).
- **As source → targets:** MP3 ★, WAV, FLAC, AAC, M4A, OGG, OPUS, AIFF, ALAC.
- **As target ← sources:** **none in v1** (no everyday demand to produce WMA; the
  only FFmpeg WMA encoder `wmav2` is low-quality, 2-channel-max legacy). This is an
  explicit, documented exclusion under the SSOT direction rule, not an oversight.
- **Engine (decode):** FFmpeg decoders `wmav1` / `wmav2` / `wmapro` / `wmalossless`
  — all decode-capable, all platforms. No patent flag for our use.
- **Options/settings:** as a source, options are those of the chosen *target*
  (e.g. converting WMA→MP3 uses the MP3 defaults). Nothing WMA-specific.
- **Lossy?:** WMA sources are usually lossy (v1/v2/Pro); WMA Lossless exists but is
  rare. Either way, treat WMA→lossless target as **lossy-origin** (no quality gain)
  per §2.9, and WMA→lossy target as a second lossy round.
- **Edge cases:** ASF metadata (title/artist/album) is mapped to tag-supporting
  targets. Surround WMA Pro decodes fine (multi-channel preserved into FLAC/WAV/
  ALAC); but the default MP3 target down-mixes per the encoder only if needed —
  channels are otherwise preserved.

---

## Category-wide

### Per-source default target (one-glance summary)

The pre-highlighted default is **MP3** for every audio source **except MP3
itself, which defaults to WAV** (MP3→MP3 is not offered). Rationale (SSOT
*How It Feels* 4 + the *Fail clearly* downstream-compatibility caveat): MP3 is the
single most universally playable audio format — it opens on every phone, car
stereo, browser, and legacy device. When someone converts audio, "give me a normal
MP3" is overwhelmingly the everyday intent; modern formats (OPUS/AAC/M4A) and
lossless formats (FLAC/WAV/ALAC) are deliberate, opt-in choices, never the safe
default.

| Source | Default target | Why |
|--------|---------------|-----|
| MP3  | **WAV** | MP3→MP3 is excluded, so the everyday default is "decode to plain WAV" (raw/editable audio); lossy→lossless adds no quality, but WAV is the expected target. ([OPEN]: WAV vs FLAC) |
| WAV  | **MP3** | universal, small |
| FLAC | **MP3** | universal, small (shrink a lossless library file to share) |
| AAC  | **MP3** | universal (escape Apple/ADTS into something that plays everywhere) |
| M4A  | **MP3** | universal (the classic "my .m4a won't play here" case) |
| OGG  | **MP3** | universal |
| OPUS | **MP3** | universal |
| AIFF | **MP3** | universal, small |
| ALAC | **MP3** | universal, small |
| WMA  | **MP3** | universal (the classic "convert this old Windows .wma") |

> **MP3-source note:** because same-format MP3→MP3 is not offered (diagonal `—`),
> an MP3 source's offered targets are WAV/FLAC/AAC/M4A/OGG/OPUS/AIFF/ALAC and the
> **default is WAV** (the most-compatible *non-MP3* everyday choice — raw
> playable/editable audio). This is the single source whose default is not MP3,
> purely because MP3 is excluded as its own target. *(Marked in the matrix MP3 row:
> WAV cell is the highlighted default for the MP3 source.)* — **[OPEN]** below
> questions whether an MP3 source should instead default to FLAC; provisionally
> **WAV**.

### Bitrate / quality defaults (the no-decision defaults), at a glance

| Target | Default | Mode | Advanced presets |
|--------|---------|------|------------------|
| MP3  | `-q:a 2` (V2, ≈190k) | **VBR** | V0/V2/V5 + CBR 128/192/320 |
| AAC  | `-b:a 192k` | **CBR** (native enc. VBR unstable) | 128/192/256 |
| M4A  | `-b:a 192k` (AAC) | **CBR** | 128/192/256 |
| OGG  | `-q:a 3` (≈112k) | **VBR** | q3/q5/q7 |
| OPUS | `-b:a 128k` | **VBR** | 96/128/192 |
| WAV  | `pcm_s16le` 16-bit | lossless | 16/24/float |
| AIFF | `pcm_s16be` 16-bit | lossless | 16/24 |
| FLAC | level 5 | lossless | 0/5/8 |
| ALAC | — (no knob) | lossless | — |
| WMA  | n/a (target out of v1) | — | — |

Defaulting principle: **lossy targets use VBR where the encoder supports it well**
(MP3, Vorbis, Opus) for best size/quality, and **CBR only where VBR is unsafe**
(native AAC). **Lossless targets preserve everything** and only expose
size/speed-neutral knobs (FLAC level) or bit-depth.

### Sample rate, channels, bit depth — preservation policy

- **Always preserved from source by default** for every conversion: sample rate
  (44.1/48/96 kHz …), channel layout (mono/stereo/5.1), and — for lossless
  targets — bit depth. No silent resampling, down-mixing, or bit-depth change in
  the default path. *(Exception, disclosed:* a >16-bit source → the **default**
  16-bit WAV/AIFF reduces bit depth; choose 24-bit in Advanced to avoid it.
  *Exception, transparent:* OPUS always runs at 48 kHz internally — Opus's design,
  not a user-visible quality loss.)
- There is **no down-mix to stereo** in v1 even for surround sources into stereo
  lossy targets unless the encoder strictly requires it; multichannel is carried
  through wherever the target codec supports it (FLAC, WAV, AIFF, ALAC, AAC, OPUS).

### Metadata / tag preservation policy

- **Intent:** carry the source's common tags (title, artist, album, album-artist,
  year, genre, track/disc number, comment) and **embedded cover art** into every
  target whose container supports them. FFmpeg maps tags across container tag
  models automatically (`-map_metadata 0`, default behaviour).
- **Tag-rich targets (full round-trip):** **M4A/ALAC** (iTunes `ilst` atoms +
  cover art), **FLAC / OGG / OPUS** (Vorbis comments + embedded picture), **MP3**
  (ID3v2 + APIC cover art).
- **Tag-poor / tag-less targets (predictable metadata loss):**
  - **AAC (raw ADTS `.aac`)** — **no tag container at all**; tags and cover art
    are **dropped**. This is a disclosed loss at the AAC target (§2.9). Users who
    want tagged AAC pick **M4A**.
  - **WAV / AIFF** — only a weak INFO/chunk tag model; rich tags and cover art may
    **not survive**. Disclosed where the source carried tags.
- **Cover art** specifically: survives MP3↔FLAC↔OGG↔OPUS↔M4A/ALAC; dropped for raw
  AAC and (largely) WAV/AIFF. **Mechanism differs by container (§3.5.1):** MP3/M4A/FLAC
  carry cover art as an **attached-picture video stream** (`-map 0:v? -c:v copy`);
  **OGG/OPUS** carry it as a **FLAC PICTURE metadata block** (`METADATA_BLOCK_PICTURE`
  Vorbis comment) via metadata copy, **not** a video-stream copy. OGG/OPUS picture
  round-trip is **`[DEFER: corpus]`** — if it proves unreliable on the §6.4 corpus,
  OGG/OPUS move to the tag-poor list and fire `audio_tags_dropped` (§2.9).
- This is the audio side of SSOT *Content fidelity* (preserve the content, not
  just the wrapper) — tags are content. Non-Latin/CJK/RTL tag text is preserved
  (UTF-8 through the tag models that support it; §2.10).

### Lossless ↔ lossy disclosure (links §2.9, never restates the string)

Three predictably-lossy situations are flagged in the matrix and disclosed as a
calm inline note at target choice (per SSOT *Fail clearly* / §2.9 catalog):

1. **lossy → lossy** (e.g. MP3→AAC, OGG→OPUS): a *second* round of compression
   loss (transcoding/generation loss). → §2.9 `audio_transcode`.
2. **lossless/lossy → lossy** as a target (any `→ MP3/AAC/M4A/OGG/OPUS`): the
   target is lossy. → §2.9 `audio_lossy_target`.
3. **lossy → lossless** (e.g. MP3→FLAC, WMA→ALAC): **no quality gain** — bigger
   file, the discarded detail is gone forever. → §2.9 `audio_lossy_origin` (so
   users aren't misled into thinking they "upgraded" their audio).
   - **Deliberate scope `[DECIDED]`:** `audio_lossy_origin` is flagged on lossy →
     **FLAC/ALAC** but **intentionally NOT on lossy → WAV/AIFF**, even though WAV/AIFF
     are equally lossless targets. Rationale: WAV/AIFF are the **common/default
     "give me a plain uncompressed file" targets**, where users do not expect an
     archive-quality claim — firing the note on every MP3→WAV would be alarming noise;
     FLAC/ALAC are the **archive-quality** targets users *do* reach for to "preserve
     quality", so the no-gain disclosure is meaningful there. The asymmetry is a
     deliberate UX call, not an oversight (the matrix `✓~` cells reflect it).
4. **bit-depth reduction** (>16-bit source → default 16-bit WAV/AIFF): a narrow
   lossy case, flagged only for that path. → §2.9 `audio_bitdepth`.

Exact strings live in the **§2.9 message catalog** (home); this file only records
*which* pairs trigger them (the `✓~` cells).

### Engine, licensing, offline

- **One engine, FFmpeg**, shipped as a **separate invoked binary** (SSOT
  engine-license policy / §3.6 copyleft isolation) — never linked into the MIT core.
  The single bundled FFmpeg binary is **GPL-2.0+** (it enables `libx264` for the video
  category, §3.6.1; the whole binary is therefore GPL, not LGPL), carrying the
  written-offer-of-source obligation (§3.6.2). The *audio* encoders/decoders it uses
  are all license-clean: `libmp3lame` (LAME, LGPL), `libvorbis`, `libopus`, native
  `aac`, native `flac`/`alac`/`pcm`, and the WMA *decoders*. It is built **without
  `--enable-nonfree`** — so **no `libfdk_aac`** (that nonfree taint is separate from
  the x264 GPL relicensing and would make the binary non-redistributable). The native
  AAC encoder is used precisely because it is license-clean. FFmpeg's GPL NOTICE +
  LAME's licence + the source offer are surfaced via §3.7.
- **Fully offline:** every codec is inside the bundled binary; no runtime fetch
  (SSOT offline floor).
- **AAC patent disposition is *not* an engine-licence issue** (the native encoder
  is LGPL) — it is the **patent** question owned solely by **§3.4**; AAC and
  M4A(AAC) availability per platform flow from that matrix. ALAC, MP3 (expired),
  Vorbis, Opus, FLAC, PCM, WMA-decode are all patent-clean for our purposes.

### Isolation & fail-clearly

- Every FFmpeg invocation routes through the §2.12 decoder-isolation wrapper
  (untrusted audio is parsed by FFmpeg's demuxers/decoders — classic attack
  surface). A crashing/hanging decode fails **that one item** with a plain message
  (§2.8) and the batch continues.
- A truncated/corrupt audio file, a `.wav`/`.m4a` that is actually a different
  type, or a 0-byte file → detected and reported per §2.8, never silently
  mis-converted. Detection (§1.2) reads the real container/codec, so a mislabeled
  extension (`.mp3` that is really FLAC; `.m4a` that is really ALAC) is handled
  correctly.

### [OPEN] items (genuine, not fake-resolved)

1. **MP3-source default target — WAV vs FLAC.** Since MP3→MP3 is excluded, the MP3
   source needs *some* default. Provisionally **WAV** (maximally compatible, raw
   audio). Argument for **FLAC** instead: smaller, still lossless, and "I have an
   MP3 and want a lossless-ish archive" is plausible — but FLAC of an MP3 is the
   misleading no-quality-gain case (§2.9 point 3), which is a poor *default*. Lean
   **WAV**. → confirm.
2. **Same-format re-encode (MP3→MP3, "shrink this MP3") — parked or included?**
   Currently parked (diagonal `—`) because it needs a *required* bitrate choice to
   be non-degenerate, clashing with the no-required-choices model. Real everyday
   demand exists ("make this MP3 smaller for email"). If promoted, it needs: a
   mandatory target-bitrate control (the one allowed exception to "no required
   choices") and a clear generation-loss disclosure. → decide in a later pass; not
   v1 as written.
3. **AAC patent disposition** is **deferred to §3.4** (not open *here*, but its
   resolution directly sets AAC + M4A(AAC) per-platform availability — flagged so
   this file's coverage is read together with that matrix). If §3.4 rules AAC
   "unavailable" on, say, Linux, then on Linux the AAC and M4A targets disappear
   *and* AAC/M4A sources can't be decoded — surfaced honestly per SSOT v1-DoD.
4. **Down-mix policy for surround→stereo-only contexts.** v1 preserves channels;
   no explicit "convert 5.1 to stereo" control. Edge everyday case (a surround M4A
   → MP3 for a phone) — currently channels are preserved and the encoder handles
   it; whether to expose a "force stereo" Advanced toggle is unresolved. Lean: not
   in v1. → confirm.
