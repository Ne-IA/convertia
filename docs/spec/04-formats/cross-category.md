# 04 — Formats: Cross-category outputs (closed set)

> The only cross-category conversions in v1 (SSOT *Direction & shape rule*):
> **extract-audio** (video → audio) and **to-animated-GIF** (video → GIF).
> A **closed set** — anything else is parked. This file **intentionally departs**
> from the per-format template in [README](README.md): its entries are
> **operations** (transformations of a video source), not standalone formats.
> The formats produced (MP3, WAV, GIF, …) are documented as formats in
> [audio.md](audio.md) and [images.md](images.md); here we document only the
> two video→X operations, their engine, options/defaults, lossiness and edge
> cases.

## What this file owns vs references

| Concern | Owner |
|---------|-------|
| The two operations, their options & defaults, lossy flags, edge cases | **this file** |
| The video **source** formats (MP4, MOV, MKV, WEBM, AVI, WMV, FLV, MPG/MPEG, M4V, 3GP) — detection, codecs-inside-container | [video.md](video.md) |
| The audio **target** formats (MP3, WAV, FLAC, M4A, …) — detection, codecs, tag handling | [audio.md](audio.md) |
| The **GIF** target format — palette/animation/transparency | [images.md](images.md) |
| AAC patent disposition (the M4A extract target is AAC-encoded) | §3.4 patent matrix |
| Generic option-declaration model / no-decision defaulting | §1.6 |
| Lossy disclosure strings | §2.9 (this file only links, never restates) |
| Target resolution (these appear as extra targets of one video source) | §1.5 |
| Engine-invocation lifecycle (spawn/progress/cancel/timeout) | §1.7 |
| Per-engine FFmpeg argument construction | §3.5 |
| Resource pre-flight / "too big" / output-size estimation | §1.10 (the GIF guardrail feeds it) |

These operations are **additional targets of a video source**, not a second
source format. The SSOT batch rule keys **only on the video source type**: drop
48 `.mov` files → one batch → the offered target set is the video targets (mp4,
mkv, …) **plus** the five "extract audio → …" targets **plus** "to animated GIF". One chosen
target applies to the whole same-source batch (per-file target is out of v1).

---

## Operation matrix (which video source supports which cross-category output)

All ten v1 video sources support **both** operations: every common video carries
(or can carry) an audio track, and any video can be turned into a short GIF. The
matrix is therefore uniform across sources; the variation lives in **how**
extract-audio runs (stream-copy vs re-encode, decided per item by the *codec
inside the container*, not by the container name). Cells follow the
[README](README.md) *Matrix cell grammar*; one engine serves every cell, so they carry no
tag: FFmpeg (`ff`).

**`ExtractAudio(FormatId)` `[DECIDED]`.** Each column is one §0.6 `TargetId`: the five
extract columns are `CrossCatOp::ExtractAudio(<format>)`, the format being MP3, WAV,
FLAC, M4A or OGG, and the GIF column is `CrossCatOp::ToGif`. Each is its own target
tile beside the video targets (§5.3 FormatPicker), carries its own options (*Options /
settings + defaults* below), and maps 1:1 onto a corpus `covers` 2-tuple —
`["<SOURCE>", "<FORMAT>"]` for an extract column, `["<SOURCE>", "GIF"]` for to-GIF
(§6.4.5) — and onto the target component of the §2.5.1 equivalence key.

| Source ↓ \ Target → | Extract audio → MP3 ★ | → WAV | → FLAC | → M4A | → OGG | To animated GIF |
|---|---|---|---|---|---|---|
| **MP4** | ✓~ | ✓ | ✓ | ✓~ | ✓~ | ✓~ |
| **MOV** | ✓~ | ✓ | ✓ | ✓~ | ✓~ | ✓~ |
| **MKV** | ✓~ | ✓ | ✓ | ✓~ | ✓~ | ✓~ |
| **WEBM** | ✓~ | ✓ | ✓ | ✓~ | ✓~ | ✓~ |
| **AVI** | ✓~ | ✓ | ✓ | ✓~ | ✓~ | ✓~ |
| **WMV** | ✓~ | ✓ | ✓ | ✓~ | ✓~ | ✓~ |
| **FLV** | ✓~ | ✓ | ✓ | ✓~ | ✓~ | ✓~ |
| **MPG/MPEG** | ✓~ | ✓ | ✓ | ✓~ | ✓~ | ✓~ |
| **M4V** | ✓~ | ✓ | ✓ | ✓~ | ✓~ | ✓~ |
| **3GP** | ✓~ | ✓ | ✓ | ✓~ | ✓~ | ✓~ |

The `★` on the MP3 column marks MP3 as the everyday extract target (`[XCAT-A]`); a
video source's default target stays its video.md `★` (MP4). The extract `~` cells are the
re-encode worst case: a track already in the target's codec is stream-copied bit-exact
(*Stream-copy vs re-encode*).

> **Single-engine rule (§3.2).** Both operations are satisfied end-to-end by the
> **one** FFmpeg invocation — no chaining. extract-audio = demux (+ optional
> re-encode) in one process; to-GIF = decode → filtergraph (palettegen +
> paletteuse) → GIF mux in one process.

> **No degenerate pairs.** Extract audio is offered on every video source, including
> one that turns out to carry no audio track: that item fails clearly with
> `NoAudioTrack` at run time, never silently (`[XCAT-C]`, edge cases). "To GIF from a
> video" always passes the SSOT inclusion test (sharing a short clip as a GIF is a
> normal-person want). No cross-category output is marked `out` — the set is exactly
> these two operations by SSOT fiat.

---

## Lossy kinds

| Kind | Pairs | Layer | Condition |
|---|---|---|---|
| `audio_lossy_target` | `video → MP3`, `→ M4A`, `→ OGG` (extract audio) | per-item-runtime | the track is re-encoded; a stream copy (an MP3, AAC or Vorbis track into its own container) keeps it bit-exact |
| `audio_transcode` | `video → MP3`, `→ M4A`, `→ OGG` (extract audio) | per-item-runtime | a lossy track is re-encoded |
| `audio_downmix` | `video → MP3`, `→ OGG` (extract audio) | per-item-runtime | a surround track whose encoder forces a downmix (rare, *Edge cases*) |
| `video_to_gif` | `video → GIF` | pair-static | — |

---

## Operation 1 — Extract audio (video → MP3 / WAV / FLAC / M4A / OGG)

Pull the audio track out of a video and save it as a standalone audio file.

- **Role:** operation. **Source side:** any v1 video format. **Target side:** five
  audio formats, one target each — MP3, WAV, FLAC, M4A, OGG (**[XCAT-A] `[DECIDED]`**,
  below).
- **Engine:** **FFmpeg** (the shared **GPL-2.0+** binary — enables libx264, §3.6.1;
  copyleft-isolated separate binary per §3.6, invoked via §3.5/§1.7, through the §2.12
  isolation wrapper). Single process per item. Same engine on Windows / macOS / Linux.
- **Detection signature:** none of its own — this operation is *offered* once a
  source is detected as a v1 video (signatures owned by [video.md](video.md));
  the output file's own signature is the target audio format's (owned by
  [audio.md](audio.md)).

### Target subset offered — `[XCAT-A]` `[DECIDED]`

The audio category has ten formats (MP3, WAV, FLAC, AAC, M4A, OGG, OPUS, WMA,
AIFF, ALAC). Offering **all ten** as extract-audio targets fails the SSOT
inclusion test (a normal person does not extract a video's soundtrack to **WMA**
or **AIFF**). The v1 subset, by everyday demand:

| Target | Why offered | Stream-copy possible? |
|--------|-------------|-----------------------|
| **MP3** ★ | The universal "rip the audio" target; opens everywhere | only if source track is already MP3 (rare: FLV/AVI) |
| **M4A**  | Native, lossy, smaller-than-MP3 at equal quality; the AAC-in-MP4 case is a **free, lossless copy** | yes, when source track is AAC (MP4/MOV/M4V/3GP — the common case) |
| **WAV**  | Uncompressed PCM — the "edit it in an audio editor" target | no (always decode → PCM) |
| **FLAC** | Lossless + compressed — "keep full quality, smaller than WAV" | no (re-encode), but **lossless** |
| **OGG** (Vorbis) | Open lossy target; the natural copy when source is WebM/OGG-Vorbis | only if source track is Vorbis (WebM) |

Excluded from the subset (still full formats in [audio.md](audio.md), just not
offered *as extract-audio targets*): **AAC** (raw `.aac` — M4A covers the AAC use
case in a friendlier container; **this does NOT avoid the §3.4 AAC patent
disposition** — M4A re-encode invokes the **same AAC encoder** regardless, so the §3.4
disposition applies either way; the exclusion is purely a UX/redundancy call, not a
patent one),
**OPUS** (re-encoding an already-lossy track to Opus is niche), **WMA**
(Windows-legacy, declining), **AIFF** (Apple-uncompressed — WAV covers the
uncompressed want), **ALAC** (Apple-lossless — FLAC covers the lossless want).

> **`[XCAT-A]` `[DECIDED]` — extract-audio targets.** All five — MP3, WAV, FLAC, M4A and
> OGG — are offered on every video source, one `CrossCatOp::ExtractAudio(FormatId)` target
> each (§0.6); MP3 is the everyday one (the SSOT mov→mp3 case, the `★` above). Two notes:
> 1. **AAC/M4A patent flag.** M4A output is AAC-encoded → an encoder choice with
>    patent implications. The bundled FFmpeg uses the **native FFmpeg AAC encoder**
>    (built-in, no external libfdk-aac), so the disposition routes through the §3.4
>    matrix exactly like the audio category's AAC/M4A row — this file does **not**
>    re-decide it, it **references §3.4**. If §3.4 gates AAC encoding on some platform,
>    the M4A extract target is honestly **unavailable there** (per SSOT first
>    exception); the other four are unaffected.
> 2. **OGG stays** for the free stream copy of a WebM/Vorbis track.

### Stream-copy vs re-encode (decided per item, automatically)

The engine decides this from the **codec inside the container** (detected, not
guessed from the container name), with **zero user choice** — it just works:

- **Stream-copy (`-c:a copy`, lossless, fast)** is used when the source audio
  codec is byte-compatible with the chosen target container/extension **and** no
  parameter change is requested:
  - source **AAC** → **M4A** (the dominant case: MP4/MOV/M4V/3GP almost always
    carry AAC) — instant, no quality loss, no re-encode.
  - source **MP3** → **MP3** (e.g. an FLV/AVI carrying an MP3 track).
  - source **Vorbis** → **OGG** (WebM carrying Vorbis).
- **Re-encode** is used in every other case (the target codec differs from the
  source codec, or the target is inherently a re-encode):
  - any source → **MP3** (LAME-style; unless source is already MP3),
  - any source → **WAV** (decode to PCM — never a copy),
  - any source → **FLAC** (re-encode, but **lossless**),
  - source AAC → MP3, source Opus → MP3, etc.

This selection is an **engine-internal capability decision (§3.2 capability
declaration)**, not a user-facing option. The user picks a *target format*; the
copy-vs-re-encode choice is made for them to be as lossless and fast as the
target allows. The lossy inline note (below) reflects the **outcome**, not the
mechanism.

**AAC copy path vs the §3.4 AAC encoder gate `[DECIDED]`.** The §3.4.4a availability flag
gates only the **AAC *encoder*/capabilities** (the patent argument rests on
encoder-distribution). The **AAC→M4A `-c:a copy`** path **decodes/remuxes only — it never
invokes the AAC encoder** — so it carries a **lighter patent profile** (no encode) and is in
principle unaffected by the encoder gate. **But to keep the format×platform offered set
honest and simple, the rule is: if AAC is ever marked unavailable on a platform (§3.4), the
M4A extract-audio target is DISABLED regardless of copy-vs-encode** (we do not offer an M4A
target that silently emits an AAC bitstream where AAC is gated off). So the copy path's
lighter profile is **noted**, but the **gate is applied at the M4A-target level**, not at the
copy-vs-encode branch — one consistent availability story per platform.

### Options / settings + defaults

extract-audio is deliberately **near-zero-option** (SSOT "it just works by
default"; v1 exposes only settings that materially change a normal user's
result). The **no-decision default** path is: drop video → choose "Extract audio
→ MP3" → convert. Per-target options:

| Target | Option (where) | Values | **Default (no-decision)** |
|--------|----------------|--------|---------------------------|
| MP3 | Quality (Advanced) | the MP3 preset set is **owned canonically by [`audio.md`](audio.md)** — *High (V0) / Standard (V2) / Small (V5)* + explicit CBR — reused **verbatim** here (no separate label set; resolves [XCAT-B]) | **Standard ≈ `-q:a 2` (VBR ~190 kbps)** |
| M4A | (none by default) — copy when AAC source; Quality (Advanced) only applies on re-encode | re-encode bitrate *Standard / High* | copy if AAC source, else **`-b:a 192k`** |
| WAV | (none) | fixed 16-bit PCM `pcm_s16le` | **16-bit PCM**, source sample rate & channels preserved |
| FLAC | Compression level (Advanced only, rarely useful) | 0–8 | **5** (FFmpeg default) — lossless regardless |
| OGG | Quality (Advanced) | Vorbis `-q:a` 0–10 | copy if Vorbis source, else **`-q:a 3` (~112 kbps)** — the canonical OGG default owned by [audio.md](audio.md) (aligned; the earlier `-q:a 5` drift is resolved to audio.md's value) |

- **Sample rate / channels:** **always preserved from the source** by default (no
  resample, no downmix) — a setting to change them is a scope addition, not a v1
  default.
- **Track selection:** **first audio track** by default (`-map 0:a:0`). Multiple
  audio tracks (multilingual MKV) → first only in v1; per-track choice is **out
  of v1** (parked — see edge cases). No silent picking of "the loudest" track;
  deterministic first-track.
- **Metadata/tags:** carry over title/artist/album where the source has them and
  the target container supports tags (per [audio.md](audio.md) tag policy); never
  invent tags. Cover-art extraction is **not** part of extract-audio.

> **`[XCAT-B]` — MP3 quality preset → FFmpeg flag mapping. `[DECIDED]`** The
> *Standard / High / Small* labels and their exact `-q:a` / `-b:a` values are shared
> with the audio category (MP3 as a standalone target) and are **defined once** — the
> canonical MP3 preset table is **owned by [audio.md](audio.md)** and referenced here
> verbatim (no separate label set). Resolved.

### Lossy?

Lossiness depends on the target, **not** the source container:

| Path | Lossy? | Note |
|------|:------:|------|
| AAC source → M4A (copy) | **No** | bit-exact copy of the original track |
| MP3 source → MP3 (copy) | **No** | copy |
| Vorbis source → OGG (copy) | **No** | copy |
| any source → WAV | **No** (transcode-lossless) | PCM is the decoded signal; no *further* loss, but the file is large and the **source's own lossy compression is already baked in** — WAV does not recover lost quality |
| any source → FLAC | **No** (transcode-lossless) | same caveat: lossless *relative to the decoded signal* |
| any lossy/AAC/Vorbis source → MP3 / re-encoded M4A / OGG | **Yes** | re-encoding one lossy codec to another (generation loss) |

The lossy inline note (passive, at target choice; SSOT *Fail clearly*) fires
**only** for the genuinely-lossy re-encode rows. **Exact strings live in §2.9**
(this file links, never restates). A subtle but important note §2.9 should carry:
even WAV/FLAC "lossless" extraction **cannot un-bake** the source's existing
lossy compression — the disclosure should not imply WAV/FLAC *improves* quality.

### Edge cases

- **No audio track** (silent screen-capture, GoPro clip with audio disabled) —
  **[XCAT-C] `[DECIDED]`: offer, then fail.** The extract-audio targets are offered on
  every video source; on run, a video without an audio stream **fails that one item
  clearly** — the §2.8 `NoAudioTrack` kind ("This file has no audio to extract.") —
  and the rest of the batch continues (§1.9 mid-run skip, §2.8 error taxonomy; this is
  a *named* failure kind, not a generic engine error). Never writes a 0-byte audio
  file. Rejected: an up-front per-item probe that disables the targets with a reason —
  it probes every item of a large recursive batch before the targets can show (§1.10
  runs no up-front `ffprobe`).
- **Multiple audio tracks** (multilingual MKV, commentary track): **first track
  only** in v1 (deterministic). Per-track / all-tracks extraction is **parked**
  (would be a one-to-many fan-out → out of v1 by SSOT). The lossy/summary text
  does not pretend other tracks were handled.
- **Image/cover-art "video" streams:** an MP3-with-embedded-cover detected as a
  video container, or an audio file mis-detected — handled by detection
  (video.md / audio.md), not here; extract-audio is only offered on a real video.
- **Variable/odd sample formats** (32-bit float, multichannel 5.1): WAV default
  is 16-bit PCM (downconverts bit depth — minor, expected); 5.1 → preserved as
  5.1 in WAV/FLAC, **not** auto-downmixed to stereo (no silent channel loss).
  MP3/OGG re-encode of >2ch follows the encoder's standard channel handling
  (§2.9 `audio_downmix` if a downmix is forced by the codec).
- **Very long videos** (2-hour movie): extract-audio output is bounded and
  modest (audio is small); the §1.10 "too big" guardrail rarely triggers here —
  unlike to-GIF. Progress is real per-item (§1.11), driven by FFmpeg time
  progress (§3.5).
- **Corrupt / truncated video:** decode fails partway → that item fails clearly,
  **no partial audio file left** (§2.1 atomic write, §2.6 cleanup); batch
  continues.

---

## Operation 2 — To animated GIF (video → GIF)

Turn a (short) video clip into a shareable animated GIF.

- **Role:** operation. **Source side:** any v1 video format. **Target side:**
  exactly **GIF** (the format is documented in [images.md](images.md)).
- **Engine:** **FFmpeg** (same bundled GPL-2.0+ binary as above, §3.6.1). Single
  process per item. The high-quality path uses FFmpeg's `palettegen` + `paletteuse` filters.
- **Detection signature:** output GIF = `GIF87a` / `GIF89a` magic (`47 49 46 38
  37/39 61`), animated GIF89a; owned by [images.md](images.md).

### Method — single-process palette pipeline (no temp PNG)

GIF is limited to **256 colours per frame**; a naïve `-f gif` produces banded,
ugly output. ConvertIA uses the **palettegen/paletteuse** approach in **one
FFmpeg invocation** via a `split` filtergraph (so it is a single §3.2 engine
call, no intermediate palette file on disk, consistent with the no-chaining rule
and §2.6 temp ownership):

```
[0:v] fps=<fps>,scale=<w>:-1:flags=lanczos,split [s0][s1];
[s0] palettegen=stats_mode=diff [p];
[s1][p] paletteuse=dither=bayer:bayer_scale=5
```

(`sjpeg` is **not** a valid `paletteuse` dither value — FFmpeg rejects it; the
**v1-exposed** dither modes are `bayer` / `sierra2_4a` / `floyd_steinberg` / `none`
(FFmpeg also accepts `sierra2` and `heckbert` — and `sierra3`/`burkes`/`atkinson` on
6.0+ — which we deliberately do **not** expose in v1), and the v1 default is
`bayer:bayer_scale=5` per [XCAT-D] `[DECIDED]`, matching §3.5.1.) Exact filter string
is constructed in §3.5; shown here to fix the **method**, not to own argument
syntax. `lanczos` scaling and a per-clip optimised palette are
what make the result look good; `fps` downsampling + width cap are what keep the
file sane. `stats_mode=diff` weights the palette toward moving regions
(better motion fidelity); `dither` choice trades dot-pattern visibility vs
banding (**[XCAT-D] `[DECIDED]`** — default `bayer:bayer_scale=5`; see the table).

> Single-pass `palettegen` in the same graph (no separate analysis pass writing a
> PNG) is the chosen trade-off: marginally less optimal than a true two-pass
> global palette, but **one process, no temp artifact, no chaining** — the right
> call for an everyday converter. A second analysis pass is **not** worth the
> temp-file + double-decode cost for v1.

### Options / settings + defaults — `[XCAT-E]` `[DECIDED]`

SSOT says expose only settings that materially change a normal user's result; for
to-GIF those are **fps**, **width** and **trim**, the Basic options:

| Option | Where | Values | **Default (no-decision)** |
|--------|-------|--------|---------------------------|
| **FPS** | Basic (it visibly changes smoothness vs size) | presets *Smooth 15 / Standard 12 / Small 10* (or a 5–20 range, Advanced) | **12 fps** |
| **Width** | Basic | presets *Large 640 / Medium 480 / Small 320* px (height auto, aspect kept, `-1`) | **480 px** |
| **Trim start** | Basic | an `IntRange` of 0–86 400 s, step 1 (`Unit::Seconds`), applied as `-ss` | **0** (the clip's start) |
| **Trim duration** | Basic | an `IntRange` of 1 s up to `GIF_DURATION_CAP` (§1.10), step 1 (`Unit::Seconds`), applied as `-t` | **`GIF_DURATION_CAP`** |
| Dither | Advanced (rarely touched) | `bayer` / `sierra2_4a` / `floyd_steinberg` / `none` (the **v1-exposed subset**; FFmpeg `paletteuse` additionally supports `sierra2` and `heckbert`, not exposed in v1 — note this is FFmpeg, NOT the cgif `gifsave` path, so error-diffusion IS available here) | **`bayer:bayer_scale=5`** ([XCAT-D] `[DECIDED]`) |
| Loop | (none) | — | **infinite loop** (`-loop 0`, the GIF norm) |
| Max colours | (not exposed) | — | **256** (full palette) |

> **`[XCAT-E]` `[DECIDED]` — trim.** Start + duration in Basic: a trim window is most of
> why people make GIFs, and left at its defaults it takes the clip's first
> `GIF_DURATION_CAP` seconds. One window applies to the whole batch (§1.6: one
> `OptionValues` per batch). The window never runs past a clip's end: a start at or after
> the end moves the window to the clip's last *duration* seconds (the whole clip when it
> is shorter), so every clip still yields a GIF, like the single-frame case below.
> Rejected: a hard duration cap with no trim — it cannot grab a moment from the middle.

> **`[XCAT-D]` `[DECIDED]` — default dither.** `bayer` (ordered, crosshatch but tiny files)
> vs `sierra2_4a` (error-diffusion, smoother but larger, can "shimmer" between
> frames). DECIDED: default `bayer:bayer_scale=5` (favours small files — the everyday GIF
> priority); the error-diffusion modes remain available in Advanced.

- **Sample rate / audio:** GIF has **no audio** — the audio track is dropped
  (intrinsic to the format, not a "loss" to disclose beyond the obvious). No
  setting.
- **Colour:** full 256-colour optimised palette by default; transparency from
  the source is **not** preserved (most video has none; GIF's 1-bit transparency
  is rarely meaningful here) — opaque output.

### Guardrail — absurdly large GIFs (mandatory, feeds §1.10)

GIF is an **uncompressed-ish, per-frame-paletted** format: file size grows
roughly linearly with `frames × width × height`. A long or large-resolution
source trivially produces a **multi-hundred-MB GIF** — a foot-gun ConvertIA must
not walk the user into. Required behaviour (this operation **supplies the inputs**
to the §1.10 resource pre-flight; §1.10 owns the threshold mechanics):

1. **Up-front estimate** before encoding: `estimated_frames (= fps × min(clip_len,
   trim_or_cap)) × out_w × out_h × ~1 byte/px` (a deliberately conservative
   per-pixel-per-frame heuristic for GIF). This is cheap (no decode needed — clip
   length + chosen fps/width are known).
2. **Duration cap:** encode at most `GIF_DURATION_CAP` (§1.10) — the trim duration's
   default and maximum ([XCAT-E]); the cap is *also* the guardrail's main lever. It is
   applied as `-t` in the same single invocation.
3. **Fail-fast threshold:** if the estimate still exceeds `GIF_ESTIMATE_CEILING` (e.g. very
   high width + long allowed window), the item **fails clearly before its encode** at
   §1.10 point 2 with the §2.8.2 `TooBig` (to-GIF) row rather than grinding out a giant
   file (SSOT *fail fast and clearly, preferably up front*). The rest of the batch
   continues.
4. The estimate + cap are **honest, not silent truncation**: if a cap shortened
   the clip, that's a predictable, disclosed outcome (passive note via §2.9
   `video_to_gif`), not a quiet surprise.

> **`[XCAT-F]` — the cap & ceiling numbers.** The duration cap and the size ceiling are
> `[DECIDED]`: the §1.10 constants `GIF_DURATION_CAP` and `GIF_ESTIMATE_CEILING` (their one
> home). The per-pixel heuristic constant (item 1) stays `[DEFER: corpus]`: a finite
> starting value ships and the §6 corpus calibrates it; leaving it unset is not an option
> (it reintroduces the foot-gun).

### Lossy?

**Always lossy — intrinsically.** to-GIF combines several unavoidable reductions:
fps downsampling (motion), scale-down (resolution), 256-colour quantisation
(palette/dither), and audio drop. This is **not** an error and **not** a
re-encode-quality slider — it is the nature of the operation, so the passive
inline note is shown for to-GIF **unconditionally** (SSOT: "to animated GIF"
genuinely drops a lot). **Exact string lives in §2.9 `video_to_gif`** (link
only) — it conveys that colours, smoothness and sound are reduced and GIFs are
for short clips, calmly, once, not per-conversion.

### Edge cases

- **Very long video** (movie, lecture recording): handled by the **guardrail**
  above (cap + fail-fast). Without it, the single biggest no-harm/UX risk in the
  category. Progress is real per-item (§1.11).
- **Very high resolution source** (4K): width default (480 px) already scales it
  down; the guardrail catches a user who cranks width to Large on a long clip.
- **No video stream** (an audio file mis-routed here): not offered — to-GIF only
  appears on a real video source (detection in [video.md](video.md)).
- **Variable frame rate (VFR) source** (screen recordings, phone video): `fps`
  filter normalises to constant output fps — correct, expected.
- **Aspect ratio / odd dimensions:** width set, height `-1` (auto, even-rounded
  as GIF/encoder requires) — aspect preserved, never stretched.
- **Single-frame / sub-second clip:** still produces a valid (tiny, possibly
  1-frame) GIF; not an error.
- **HDR / wide-gamut source:** tone-mapped down to GIF's 8-bit palette by the
  decode→scale→palettegen chain — visibly flattened but valid; covered by the
  unconditional lossy note.
- **Corrupt / truncated video:** decode fails → item fails clearly, **no partial
  GIF** (§2.1/§2.6); batch continues.

---

## Category-wide

### Per-source "default cross-category target" summary

Cross-category outputs are **not** the pre-highlighted default of any video
source — a video's pre-highlighted default is a **video** target (owned by
[video.md](video.md), e.g. MOV→MP4). The cross-category targets sit alongside that
default in the offered target list, each with its no-decision options:

| Target | Offered on | The no-choice path |
|--------|-----------|--------------------|
| Extract audio → MP3 (WAV, FLAC, M4A and OGG likewise, each with its own defaults) | every v1 video source | **Standard quality (~190 kbps VBR)**, first audio track, source rate/channels preserved |
| To animated GIF | every v1 video source | **12 fps, 480 px wide, the clip's first `GIF_DURATION_CAP` seconds, bayer dither, infinite loop** |

So a user who never opens Advanced gets: *drop video → "Extract audio → MP3"* or
*drop video → "To GIF"* → done, two clicks.

### Batch interaction (restating the SSOT rule for this file)

- These are **targets of one video source**, never a second source format. The
  batch grouping key is the **video source type only** (§1.3); choosing an
  extract-audio target or "To GIF" applies that one target to the **whole same-source
  batch** (e.g. 48 `.mov` → 48 MP3s, or 48 GIFs). Per-file target is out of v1.
- Output naming, no-clobber, atomic write, beside-source destination + per-
  location divert, free-space/path-limit guarantees apply **identically** to
  cross-category outputs (§2.1–§2.7) — an extracted `clip.mp3` / `clip.gif` keeps
  the source base name with the new extension, no-clobber numbered.
- The **re-run/equivalent-output detection** (§2.5) keys on *source + target +
  effective settings* — so "extract audio → MP3 (Standard)" re-run on the same
  video triggers the skip/fresh-copy prompt; changing fps or MP3 quality is a new
  conversion (ordinary numbering).

### Metadata / encoding / audio-channel policy

- **Extract audio:** preserve source tags where target supports them; preserve
  sample rate & channels (no resample/downmix by default); first audio track;
  stream-copy (lossless) whenever the codec matches the container, else
  least-lossy re-encode the target allows.
- **To GIF:** drops audio (intrinsic), drops to 256 colours, no metadata of
  consequence to carry; infinite loop.

### Engine & platform

One engine — **FFmpeg** — covers both operations on all three platforms; the
only platform-conditional element is the potential **AAC/M4A** patent gate, which
**references §3.4** and never re-decides it here. If the M4A extract target is gated
on a platform, extract-audio still ships everywhere (MP3/WAV/FLAC/OGG unaffected) — so
no platform loses the *operation*, at most the M4A target (honest per-platform
availability per SSOT first exception).

### Open items (honest)

| ID | Decision | Status |
|----|----------|--------|
| **[XCAT-A]** | Extract-audio targets | **`[DECIDED]`** MP3★, WAV, FLAC, M4A and OGG, one `ExtractAudio(FormatId)` target each (§0.6); M4A follows the §3.4 AAC disposition |
| **[XCAT-B]** | MP3 *Standard/High/Max* preset → `-q:a`/`-b:a` mapping | **`[DECIDED]`** — owned canonically in [audio.md](audio.md) (High V0 / Standard V2 / Small V5 + explicit CBR), reused verbatim here; resolved in Operation 1's *Options / settings + defaults* MP3 row |
| **[XCAT-C]** | "No audio track": probe up front or offer, then fail | **`[DECIDED]`** offer-then-fail with `NoAudioTrack` (*Edge cases*) |
| **[XCAT-D]** | Default GIF dither | **`[DECIDED]`** — `bayer:bayer_scale=5` (favours small files, the everyday GIF priority); error-diffusion modes remain available as Advanced |
| **[XCAT-E]** | to-GIF **trim** | **`[DECIDED]`** Basic start + duration, each an `IntRange` in seconds; the duration's default and maximum is `GIF_DURATION_CAP` |
| **[XCAT-F]** | to-GIF guardrail numbers: duration cap, size ceiling, per-pixel size heuristic (co-owned §1.10) | cap and ceiling **`[DECIDED]`** — the §1.10 constants `GIF_DURATION_CAP` and `GIF_ESTIMATE_CEILING`; the per-pixel heuristic `[DEFER: corpus]` — a finite starting value ships; calibrate against the §6 corpus |

> Every item is decided; only [XCAT-F]'s per-pixel heuristic is calibrated against the
> §6 corpus (a finite starting value ships).
