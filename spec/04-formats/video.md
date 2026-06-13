# 04 — Formats: Video

> Formats (SSOT *What It Converts* → Video): MP4, MOV, MKV, WEBM, AVI, WMV, FLV,
> MPG/MPEG, M4V, 3GP. The two cross-category outputs of every video source —
> **extract-audio** (→ MP3/WAV/…) and **to-animated-GIF** — are owned by
> [cross-category.md](cross-category.md) and are **referenced, never duplicated**
> here (they appear in the matrix as targets so the picture is complete, but their
> options/engine detail live there). Follows the per-format template in
> [README](README.md). Single engine for the whole category: **FFmpeg** (§3.1, §3.5).

## Intro

Video is the heaviest category by runtime and the one where the SSOT *Fail
clearly* progress promise matters most: a single re-encode of a long clip can run
for minutes, so every pair reports **real per-item progress** (§1.11), never a
spinner. Two ideas drive the whole category:

1. **Container vs codec.** A `.mkv`/`.mp4`/`.mov` file is a *container* (a wrapper)
   holding one or more **codec**-encoded streams (video, audio, subtitles). The
   user-facing format is the **container** (MP4 ≠ MKV is the §1.3 batch key), but
   what ConvertIA can do cheaply depends on the **codecs inside**. ConvertIA detects
   the container by content (magic bytes) and inspects the inner codecs with
   `ffprobe` before planning the job.
2. **Re-encode vs remux (stream-copy).** When the source's inner codecs are already
   legal in the target container, ConvertIA **remuxes** (`-c copy`): it repackages
   the existing streams into the new container with **no quality loss** and in
   seconds (no decode/encode). When they are not, it **re-encodes** to the target's
   canonical codecs — slower and **lossy**. This is the single most important
   behaviour in the category and is decided **automatically** per item (see
   *Category-wide → Re-encode vs remux*); the user never has to know the words.

**The everyday job is "modernize / make it play everywhere".** Real users drop a
`.mkv` from a download, a `.mov` from an iPhone, an `.avi`/`.wmv`/`.flv` from an
old drive, and want one thing: **a file that plays anywhere** — which in 2026 is
overwhelmingly **MP4 (H.264 + AAC)**. So **MP4 is the pre-highlighted default for
every video source.** Modern-container targets (MKV, WEBM, MOV) are offered where
there is genuine everyday demand; the dead legacy containers (AVI, WMV, FLV,
MPG/MPEG, 3GP) are **valid sources but not offered as targets** — nobody needs to
*produce* a `.wmv` in 2026 (they fail the SSOT inclusion test as targets).

## Source → target matrix

Rows = source format, cols = target format. Legend: **✓** supported (FFmpeg) ·
**✓★** the one pre-highlighted default target · **✓~** supported but predictably
**lossy** (re-encode; loss flag → §2.9) · **R** = usually a lossless **remux**
(stream-copy) when inner codecs already fit, else re-encode · **—** not offered
(degenerate / no everyday demand — fails the SSOT inclusion test) · **self** =
same-format (see *Category-wide → Same-container*). All conversions are one
source → one target, satisfied by the **single** engine FFmpeg (§3.2).

The two cross-category targets (rightmost) are owned by
[cross-category.md](cross-category.md); shown here only so each source's full
offered set is visible.

| Source ↓ \ Target → | MP4 | MOV | MKV | WEBM | M4V | →audio | →GIF |
|---|---|---|---|---|---|---|---|
| **MP4**  | ✓★ self R | ✓ R | ✓ R | ✓~ | ✓ R | ✓ (x-cat) | ✓~ (x-cat) |
| **MOV**  | ✓★ R | self R | ✓ R | ✓~ | ✓ R | ✓ (x-cat) | ✓~ (x-cat) |
| **MKV**  | ✓★ R | ✓ R | self R | ✓~ | ✓ R | ✓ (x-cat) | ✓~ (x-cat) |
| **WEBM** | ✓★~ | ✓~ | ✓ R | self R | ✓~ | ✓ (x-cat) | ✓~ (x-cat) |
| **AVI**  | ✓★~ | ✓~ | ✓ R | ✓~ | ✓~ | ✓ (x-cat) | ✓~ (x-cat) |
| **WMV**  | ✓★~ | ✓~ | ✓ R | ✓~ | ✓~ | ✓ (x-cat) | ✓~ (x-cat) |
| **FLV**  | ✓★ R | ✓ R | ✓ R | ✓~ | ✓ R | ✓ (x-cat) | ✓~ (x-cat) |
| **MPG/MPEG** | ✓★~ | ✓~ | ✓ R | ✓~ | ✓~ | ✓ (x-cat) | ✓~ (x-cat) |
| **M4V**  | ✓★ R | ✓ R | ✓ R | ✓~ | self R | ✓ (x-cat) | ✓~ (x-cat) |
| **3GP**  | ✓★~ | ✓~ | ✓ R | ✓~ | ✓~ | ✓ (x-cat) | ✓~ (x-cat) |

Notes on the `R`/`✓~` choice per cell:
- **MP4 / MOV / MKV / M4V / FLV → MP4 / MOV / MKV / M4V** are marked **R** because
  these containers very commonly already hold **H.264 + AAC** (FLV typically
  H.264/AAC since ~2008), which is legal in all four MP4-family/Matroska
  containers → a clean **lossless remux**. The runtime check (§3.5) downgrades a
  given item to re-encode (`✓~`) only when a specific inner codec is *not* legal in
  the target (e.g. MKV holding HEVC/VP9 → MP4 may re-encode video; PCM/Vorbis audio
  → MP4 re-encodes audio to AAC). The matrix marks the **common** case.
  - **FLV footnote:** the FLV `R` cells hold **only for H.264/AAC FLV**. **Older
    VP6 / Sorenson Spark (H.263-class) FLV cannot losslessly remux to MP4/MOV/M4V**
    (those codecs are not legal in the MP4 family — only MKV can wrap them verbatim),
    so the §3.5 runtime check re-encodes such items (`✓~`) on the MP4-family targets.
    Phase-3 must **not** implement an always-remux FLV→MP4 path — the remux-vs-reencode
    decision is per-item from the ffprobe inner-codec inventory (§3.5).
- **→ WEBM** is always **✓~ re-encode**: WEBM legally carries only VP8/VP9/AV1
  video + Vorbis/Opus audio, which the H.264/AAC mainstream never matches, so a
  WEBM target always decodes-and-re-encodes (lossy).
- **AVI / WMV / MPG / 3GP sources → MP4/MOV/M4V** are **✓~** because their inner
  codecs (MPEG-4 Part 2/DivX, MS-MPEG4/WMV, MPEG-1/2, H.263) are old and a clean
  modernization re-encodes to H.264/AAC (lossy but that is the point). **→ MKV** is
  more often **R**: MKV is permissive enough to wrap most legacy codecs verbatim
  (lossless container change) — but the everyday user wants MP4, hence MP4 default.

## Per-format entries

For every source the **default target is MP4** (`✓★`), so the per-entry
*As-source → targets* lists repeat the same offered set; the **detection signature,
typical inner codecs, remux-vs-reencode disposition, and edge cases** are what
differ and are spelled out per format.

---

### `MP4` (.mp4)
- **Detection:** ISO Base Media File Format. Bytes 4–7 = `66 74 79 70` (`ftyp`);
  the **major brand** at offset 8 distinguishes the MP4 family: `isom`, `mp41`,
  `mp42`, `avc1`, `dash`, etc. Shares the `ftyp` box with MOV / M4V / 3GP — the
  brand, not the extension, disambiguates (a `.mov`-branded `qt  ` file dropped as
  `.mp4` is treated as MOV per SSOT *Recognize files by content*). Extension `.mp4`
  (also `.m4v`, see its entry).
- **Role:** both.
- **As source → targets:** **MP4★** (self, re-mux/repackage), MOV (R), MKV (R),
  WEBM (✓~ re-encode), M4V (R) · + extract-audio, to-GIF (→ cross-category).
- **As target ← sources:** MP4, MOV, MKV, WEBM, AVI, WMV, FLV, MPG/MPEG, M4V, 3GP
  (i.e. **every** video source — MP4 is the universal default target).
- **Engine(s):** FFmpeg (single engine, all platforms). Video re-encode →
  **libx264** (H.264); audio re-encode → native **aac** encoder. **Patent flag:**
  H.264 and AAC are patent-encumbered; ConvertIA bundles a **GPL-2.0+ FFmpeg**
  (`--enable-gpl` to link libx264 relicenses the whole binary GPL — it is NOT an LGPL
  build, §3.1/§3.6.1) **with the native built-in AAC encoder and libx264 enabled**,
  shipped as a separate invoked binary (aggregation) with the written-offer-of-source
  obligation — disposition deferred to the **§3.4 patent matrix** (the single owner). This entry does not re-decide it; if
  §3.4 marks H.264/AAC encode unavailable on a platform, MP4-as-target is honestly
  surfaced as unavailable there per SSOT *v1 DoD* exception 1. (Practical note: x264
  + the FFmpeg-native AAC encoder are the long-standing default bundle choice and
  are expected to be `ship-bundled` on all three platforms; final word = §3.4.)
- **Options/settings:** see *Category-wide → Options*. Defaults: video CRF **23**,
  preset **medium**, audio AAC **128 kbps** — applied **only on the re-encode
  path**; a remux copies streams untouched and ignores quality options.
- **Lossy?:** Remux path (MP4→MP4/MOV/MKV/M4V) = **lossless**. Re-encode path
  (→WEBM, or any item whose codecs force a transcode) = **lossy** → §2.9.
- **Edge cases:** `faststart` (`-movflags +faststart`) applied to all MP4/MOV/M4V
  outputs so the `moov` atom is at the front (instant playback). Multiple audio
  tracks and embedded **subtitle** tracks: see *Category-wide*. Fragmented MP4
  (fMP4/DASH init segments) detected and handled by remux. Very large files stream
  through FFmpeg (constant memory) — size limit is §1.10's concern, not the codec.

---

### `MOV` (.mov, .qt)
- **Detection:** QuickTime / ISO-BMFF variant — `ftyp` box with brand `qt  `
  (`71 74 20 20`) at offset 8, **or** a top-level `moov`/`mdat`/`wide` atom for
  older QuickTime files that omit `ftyp`. Same family as MP4; brand disambiguates.
  Extension `.mov` (`.qt` legacy).
- **Role:** both.
- **As source → targets:** **MP4★** (R), MOV (self R), MKV (R), WEBM (✓~), M4V (R)
  · + extract-audio, to-GIF.
- **As target ← sources:** MP4, MOV, MKV, AVI, WMV, FLV, MPG/MPEG, M4V, 3GP.
  *(MOV is offered as a target — Apple users sometimes want it — but it is never a
  source's default; MP4 is.)*
- **Engine(s):** FFmpeg. Same encoders/patent disposition as MP4 (H.264/AAC →
  §3.4). **ProRes** (a common MOV video codec from Apple devices/editors) is
  **decoded** fine; on a **→MP4 re-encode** it becomes H.264 (intended
  modernization). ConvertIA does **not** *encode* ProRes as a target (specialist —
  out per inclusion test).
- **Options/settings:** category defaults (CRF 23 / medium / AAC 128k), re-encode
  path only.
- **Lossy?:** Remux (MOV→MP4/MKV/M4V with H.264/AAC inside) lossless; ProRes/other
  → MP4 re-encode lossy → §2.9; →WEBM always lossy.
- **Edge cases:** iPhone `.mov` often carries **HEVC (H.265)** video + AAC + a
  timed-metadata track and sometimes rotation flags — ConvertIA honours the
  display-matrix **rotation** so portrait clips stay portrait. HEVC→MP4: H.265 is
  legal in MP4, so the **video can remux**; but the everyday-compatibility default
  re-encodes HEVC→H.264 for "plays everywhere" (this is an `[OPEN]` policy choice,
  see *Category-wide*).

---

### `MKV` (.mkv)
- **Detection:** Matroska = EBML. Magic bytes `1A 45 DF A3` at offset 0 (shared
  with WEBM, which is a Matroska subset); the **DocType** EBML element reads
  `matroska` (vs `webm`) and disambiguates. Extension `.mkv` (`.mka` audio-only is
  out of the video category).
- **Role:** both.
- **As source → targets:** **MP4★** (R when H.264/AAC inside; else ✓~), MOV (R),
  MKV (self R), WEBM (✓~), M4V (R) · + extract-audio, to-GIF.
- **As target ← sources:** MP4, MOV, MKV, WEBM, AVI, WMV, FLV, MPG/MPEG, M4V, 3GP
  (**every** source — MKV is the permissive "wrap anything losslessly" target and
  is offered to all, just never the default).
- **Engine(s):** FFmpeg. MKV is the most permissive container: most legacy and
  modern codecs can be **remuxed in verbatim**, so source→MKV is frequently a
  lossless container change. Re-encode only when a target-codec is explicitly
  requested (not in v1 — no codec picker).
- **Options/settings:** category defaults on re-encode; remux ignores them.
- **Lossy?:** source→MKV usually **lossless remux** → no §2.9 note in the common
  case; MKV→MP4 lossy only if an MKV-only codec (e.g. some PCM/DTS audio, certain
  subtitle formats) forces a transcode; MKV→WEBM always lossy → §2.9.
- **Edge cases:** MKV routinely holds **multiple audio tracks, multiple subtitle
  tracks (SRT/ASS/PGS), chapters, and attachments (fonts)**. On **→MP4**: MP4 can
  carry **multiple audio** and **tx3g/mov_text subtitles** but **not** ASS/PGS →
  see *Category-wide → Subtitles & tracks* for the keep/convert/drop policy. Default
  video codec inside MKV is often **H.264** (remux) but increasingly **HEVC/VP9/AV1**
  (may re-encode for the MP4 default).

---

### `WEBM` (.webm)
- **Detection:** EBML magic `1A 45 DF A3` (= Matroska) **with DocType `webm`**.
  Distinguished from MKV by the DocType string only. Extension `.webm`.
- **Role:** both.
- **As source → targets:** **MP4★** (✓~ re-encode — see below), MOV (✓~), MKV (R),
  WEBM (self R), M4V (✓~) · + extract-audio, to-GIF.
- **As target ← sources:** MP4, MOV, MKV, WEBM, AVI, WMV, FLV, MPG/MPEG, M4V, 3GP
  (offered as a modern web target for every source).
- **Engine(s):** FFmpeg. **As target:** video → **libvpx-vp9** (VP9), audio →
  **libopus** (Opus). VP8/Vorbis are legacy; VP9/Opus is the v1 WEBM output. (AV1 is
  intentionally **not** the WEBM-target codec in v1 — `libaom-av1` is far too slow
  for an everyday desktop converter; `[OPEN]` revisit if SVT-AV1 bundling proves
  fast enough.) **As source:** VP8/VP9/AV1 + Vorbis/Opus decode fine.
- **Options/settings:** WEBM-target re-encode uses **constant-quality** mode:
  `-c:v libvpx-vp9 -b:v 0 -crf 32 -row-mt 1` (CRF 32 = the "good for everyday web"
  default). **The libvpx-vp9 CRF range is `0–63`** (0 = best/largest, 63 =
  worst/smallest); **15–35 is the *recommended* everyday band**, not the codec's full
  range. If a VP9 quality slider is ever exposed, its **validation bound is `0..=63`** and
  it maps the Smaller↔Better presets into the 15–35 recommended band (default 32) — the
  slider must not clamp the codec range to 15–35. Audio `-c:a libopus -b:a 96k`.
  **Single-pass** by default — two-pass is smaller but doubles runtime and is out of v1's
  no-knobs default (an Advanced toggle is `[OPEN]`).
- **Lossy?:** WEBM→MP4/MOV/M4V = re-encode (VP9→H.264) = **lossy** → §2.9.
  WEBM→MKV = remux VP9 verbatim (lossless). source→WEBM = **always lossy** (VP9/Opus
  re-encode) → §2.9.
- **Edge cases:** WEBM can have **transparency (alpha, VP8/VP9 yuva420p)** — alpha
  is **lost** when re-encoding to H.264/MP4 (H.264 has no alpha) → an extra §2.9
  note (`video_alpha_lost`) for that specific case. WEBM has no native subtitle muxing in common use;
  audio is single-track in practice.

---

### `AVI` (.avi)
- **Detection:** RIFF container — bytes 0–3 `52 49 46 46` (`RIFF`), bytes 8–11
  `41 56 49 20` (`AVI `). Extension `.avi`.
- **Role:** both (legacy source; **not** offered as a target).
- **As source → targets:** **MP4★** (✓~), MOV (✓~), MKV (R — wrap codec verbatim),
  WEBM (✓~), M4V (✓~) · + extract-audio, to-GIF.
- **As target ← sources:** **none** — AVI is a dead delivery container; producing a
  `.avi` fails the SSOT everyday-demand test. Marked **out** as a target.
- **Engine(s):** FFmpeg. AVI commonly holds **MPEG-4 Part 2 (DivX/Xvid)** or older
  **MJPEG/Cinepak** video + **MP3 or PCM** audio. Modernizing → H.264/AAC MP4 is the
  whole point (re-encode). →MKV can remux the old codec verbatim (lossless container
  swap) for users who just want a modern wrapper.
- **Options/settings:** category defaults (CRF 23 / medium / AAC 128k).
- **Lossy?:** AVI→MP4/MOV/M4V/WEBM = re-encode = **lossy** → §2.9; AVI→MKV usually
  lossless remux.
- **Edge cases:** AVI has **no native B-frame timestamps / VFR** support — some AVIs
  use a fixed frame rate that FFmpeg reads cleanly; broken/odd-fps AVIs are handled
  by FFmpeg's `-vsync` defaults (CFR output). Interleaved-but-degraded audio sync is
  re-aligned on re-encode. PCM/uncompressed-DV AVIs can be huge → §1.10 size
  pre-flight.

---

### `WMV` (.wmv)
- **Detection:** Advanced Systems Format (ASF) — 16-byte GUID at offset 0
  `30 26 B2 75 8E 66 CF 11 A6 D9 00 AA 00 62 CE 6C`. The **same ASF GUID** is used
  by `.wma` (audio-only) — the presence of a video stream (probed) tells WMV from
  WMA. Extension `.wmv` (`.asf`).
- **Role:** both (legacy source; **not** a target).
- **As source → targets:** **MP4★** (✓~), MOV (✓~), **MKV (✓ R — usually lossless,
  see below)**, WEBM (✓~), M4V (✓~) · + extract-audio, to-GIF.
- **As target ← sources:** **none** — Windows-only legacy delivery format, no
  everyday demand to *produce*. Out as target.
- **Engine(s):** FFmpeg. WMV holds **WMV1/2/3 / VC-1** video + **WMA** audio.
  **Matroska (MKV) can carry VC-1 and WMA verbatim**, so **WMV→MKV is commonly a
  lossless remux** (`-c copy`, container swap) — corrected from the prior "always
  re-encode" claim. Old **WMV7/8 (WMV1/2)** variants that some Matroska muxers won't
  accept fall back to re-encode at runtime (the §3.5 probe decides per item). The
  **MP4-family** targets (MP4/MOV/M4V) are **not** first-class homes for WMV/WMA, so
  those stay re-encode (✓~); modernization → H.264/AAC.
- **Options/settings:** category defaults.
- **Lossy?:** **WMV→MKV is usually a lossless remux** (VC-1/WMA copied verbatim;
  older WMV7/8 may re-encode). **WMV→MP4/MOV/M4V/WEBM** = re-encode = lossy → §2.9.
- **Edge cases:** DRM-protected WMV/ASF (legacy PlaysForSure) **cannot** be decoded
  → fails clearly per SSOT *Fail clearly* (one plain message: "this file is
  copy-protected and can't be converted"), batch continues. VC-1 advanced-profile
  decode is supported by bundled FFmpeg.

---

### `FLV` (.flv)
- **Detection:** bytes 0–2 `46 4C 56` (`FLV`), byte 3 = version (`01`). Extension
  `.flv` (`.f4v` is actually MP4-branded — detected as MP4 by `ftyp`).
- **Role:** both (legacy source; **not** a target).
- **As source → targets:** **MP4★** (R — usually lossless!), MOV (R), MKV (R),
  WEBM (✓~), M4V (R) · + extract-audio, to-GIF.
- **As target ← sources:** **none** — Flash is dead; producing `.flv` has no
  everyday demand. Out as target.
- **Engine(s):** FFmpeg. Modern FLV (post-2008) holds **H.264 video + AAC audio**
  → **direct lossless remux into MP4** (the FLV container is the only thing dead,
  the streams are fine). Old Sorenson Spark (H.263)/VP6 + MP3/Nellymoser FLVs →
  re-encode to H.264/AAC.
- **Options/settings:** category defaults (re-encode path only).
- **Lossy?:** H.264/AAC FLV→MP4/MOV/MKV/M4V = **lossless remux**; Sorenson/VP6 FLV
  or →WEBM = lossy → §2.9.
- **Edge cases:** FLV cue/metadata (`onMetaData`) is informational and dropped on
  remux without affecting playback. Some FLVs have non-monotonic timestamps →
  FFmpeg's `-fflags +genpts` regenerates them on remux so the MP4 seeks correctly.

---

### `MPG / MPEG` (.mpg, .mpeg, .m1v, .m2v, .vob)
- **Detection:** MPEG Program Stream / Elementary Stream start codes — `00 00 01 BA`
  (Program Stream pack header) or `00 00 01 B3` (sequence header). Extensions
  `.mpg`, `.mpeg`, `.m2v`, `.vob` (DVD). `.ts` (transport stream) also detected by
  sync byte `0x47` and treated here.
- **Role:** both (legacy source; **not** a target).
- **As source → targets:** **MP4★** (✓~), MOV (✓~), MKV (R), WEBM (✓~), M4V (✓~)
  · + extract-audio, to-GIF.
- **As target ← sources:** **none** — MPEG-1/2 PS is a legacy/DVD delivery format;
  no everyday demand to produce. Out as target.
- **Engine(s):** FFmpeg. Holds **MPEG-1 / MPEG-2 video + MP2/AC-3/MP3 audio**.
  MPEG-2 video *is* legal in MP4/MKV (could remux) but the everyday default
  **re-encodes to H.264** for compatibility and large size reduction; →MKV can
  remux verbatim.
- **Options/settings:** category defaults.
- **Lossy?:** re-encode to H.264/AAC = **lossy** → §2.9; →MKV remux lossless.
- **Edge cases:** **Interlaced** MPEG-2 (DVD/broadcast) is common — ConvertIA applies
  **deinterlace (`yadif`) automatically** when the source is flagged interlaced so
  the MP4 looks right on progressive screens (an `[OPEN]` default — see
  *Category-wide*). Multi-program transport streams: ConvertIA takes the **first/best
  program** (specialist multi-program demux is out of scope). AC-3 audio → AAC on
  re-encode.

---

### `M4V` (.m4v)
- **Detection:** ISO-BMFF `ftyp`, brand `M4V `/`mp42`/`M4VH` (Apple). Byte-identical
  family to MP4; the **only** practical difference is Apple's optional **FairPlay
  DRM** flag and the `.m4v` extension. Detected as the MP4 family by `ftyp`.
- **Role:** both.
- **As source → targets:** **MP4★** (R — essentially a rename+repackage), MOV (R),
  MKV (R), WEBM (✓~), M4V (self R) · + extract-audio, to-GIF.
- **As target ← sources:** **all 10 video sources** — MP4, MOV, MKV, WEBM, AVI, WMV,
  FLV, MPG/MPEG, M4V, 3GP — **identical to the MP4 target source-list** (M4V is an
  MP4-family remux/repackage: the MP4-family sources remux, the rest re-encode `✓~` to
  H.264/AAC, exactly as for the MP4 target). The matrix M4V column marks every source row,
  so all ten are valid targets here. A user picks `.m4v` for the Apple TV/iTunes extension.
  Offered, never default. (The earlier 5-source list — MP4/MOV/MKV/FLV/M4V only — wrongly
  excluded AVI/WMV/WEBM/MPG/3GP, which the matrix shows as valid `✓~` M4V targets.)
- **Engine(s):** FFmpeg. Streams are H.264/AAC (same as MP4) → remux throughout.
- **Options/settings:** category defaults (re-encode path only — rare for M4V).
- **Lossy?:** M4V↔MP4/MOV/MKV = **lossless remux**; →WEBM lossy → §2.9.
- **Edge cases:** **DRM-protected (FairPlay) M4V** — purchased iTunes content —
  **cannot** be decoded → fails clearly ("this file is copy-protected and can't be
  converted"), batch continues, nothing written. Non-DRM M4V (Handbrake output,
  home videos) converts identically to MP4.

---

### `3GP` (.3gp, .3g2)
- **Detection:** ISO-BMFF `ftyp` with brand `3gp4`/`3gp5`/`3gp6`/`3g2a`. MP4 family;
  brand disambiguates. Extensions `.3gp`, `.3g2`.
- **Role:** both (legacy mobile source; **not** a target).
- **As source → targets:** **MP4★** (✓~ — see below), MOV (✓~), MKV (R), WEBM (✓~),
  M4V (✓~) · + extract-audio, to-GIF.
- **As target ← sources:** **none** — 3GP exists for ~2005-era feature phones
  (H.263 + AMR-NB, QCIF 176×144). Producing one in 2026 has **no everyday demand**
  and the 3GP container imposes exotic constraints (fixed QCIF frame sizes, AMR
  audio that needs a separate non-default encoder). Out as target — fails the
  inclusion test decisively.
- **Engine(s):** FFmpeg. Modern 3GP holds **H.264 + AAC-LC** → could remux to MP4
  (lossless); old 3GP holds **H.263 / MPEG-4 Part 2 + AMR-NB** → re-encode. The
  matrix marks **✓~** because the everyday 3GP-in-the-wild is the old AMR-NB kind;
  the runtime check upgrades an H.264/AAC 3GP to a remux automatically.
- **Options/settings:** category defaults.
- **Lossy?:** old (H.263/AMR) 3GP→MP4 = re-encode = **lossy** → §2.9; H.264/AAC
  3GP→MP4 = lossless remux.
- **Edge cases:** **AMR-NB audio** decode is supported by bundled FFmpeg; on
  re-encode to MP4 it becomes AAC (mono 8 kHz upsampled). Tiny QCIF resolution is
  **not upscaled** (no synthetic detail) — *Category-wide → Resolution* keeps the
  source resolution.

---

## Category-wide

### Per-source default-target summary (one glance)

Every video source's single pre-highlighted default is **MP4** (H.264 + AAC) — the
universal "plays everywhere" choice (SSOT *How It Feels* 4, tie-breaker → widely
compatible).

| Source | Default target | Why |
|---|---|---|
| MP4 | **MP4** (self, normalize/faststart) | already ideal; re-saves with `+faststart`, never overwrites original |
| MOV | **MP4** | Apple → universal; remux when H.264/AAC inside |
| MKV | **MP4** | download container → universal; remux when H.264/AAC |
| WEBM | **MP4** | web format → universal (re-encode VP9→H.264) |
| AVI | **MP4** | legacy → modernize (re-encode) |
| WMV | **MP4** | Windows-legacy → modernize (re-encode) |
| FLV | **MP4** | Flash-dead → universal; usually lossless remux |
| MPG/MPEG | **MP4** | DVD/legacy → modernize (re-encode, deinterlace) |
| M4V | **MP4** | drop the Apple quirk → universal (remux) |
| 3GP | **MP4** | old phone clip → modernize (re-encode/remux) |

The §1.6 "no required choices" gate verifies: **dropping any video and hitting
convert with zero clicks produces a valid MP4.**

### Re-encode vs remux (stream-copy) — the automatic decision

The engine layer (§3.5) decides per item, never asking the user:

1. **Probe** the source's streams (`ffprobe`): inner video codec, audio codec(s),
   subtitle codec(s), pixel format, rotation, interlace flag.
2. **Remux** (`-c copy`, **lossless**, seconds) **iff** *every* stream the target
   keeps is a codec **legal in the target container** *and* no normalization is
   required. This is the common, preferred path for the MP4/MOV/MKV/M4V family and
   for FLV/MOV/MKV holding H.264/AAC.
3. **Re-encode** (decode → H.264/AAC or VP9/Opus, **lossy**, minutes) when any kept
   stream's codec is illegal in the target (e.g. VP9→MP4, PCM→MP4), when the target
   is WEBM (codecs never match), or when a normalization (deinterlace, rotation
   bake-in) is needed.
4. **Mixed** is allowed: video may remux while audio re-encodes (e.g. MKV with
   H.264 video + Vorbis audio → MP4 copies video, transcodes audio to AAC). This is
   still **one engine, one FFmpeg invocation** — no chaining, satisfies §3.2.

Because remux vs re-encode changes whether the result is lossy, the §2.9
`video_reencode` note's firing is governed by §2.9.2. **Timing matters:** the *exact*
per-item disposition needs the full `ffprobe` stream inventory, which is deferred to
**convert-time** (§1.2/§3.5) — it is **not** run on every item before convert (too
costly on large recursive batches). So the note shown at **target choice** is a
**header/container-pair-derived best-effort worst-case** (§2.9.2): a target pair that
is **always re-encode** (→WEBM, or a legacy source with known-incompatible inner
codecs) fires the note definitely; a pair that **commonly remuxes** but *might*
re-encode a given item fires the worst-case *"may be re-encoded"* phrasing rather
than falsely promising losslessness. `RunStarted.willReencode` (§0.4.2) carries this
same worst-case flag. The **§1.12 summary reflects what actually happened** per item
once §3.5 resolved the real disposition. (For a mixed batch, the pre-convert note
shows if **any** item *may* re-encode — honest worst-case.)

### Default codec / quality / resolution

| Aspect | Default (no-decision) | Notes |
|---|---|---|
| Video codec (re-encode) | **H.264 (libx264)** for MP4/MOV/MKV/M4V; **VP9 (libvpx-vp9)** for WEBM | universal vs web |
| Quality (H.264) | **CRF 23**, preset **medium** | x264 native defaults; sane 18–28 range |
| Quality (VP9) | **CRF 32**, `-b:v 0` (constant quality), single-pass, `-row-mt 1` | libvpx-vp9 CRF range is **0–63** (15–35 is the recommended band; 32 = "good for web"); a slider validates `0..=63` |
| Audio codec (re-encode) | **AAC-LC** (MP4 family) / **Opus** (WEBM) | native FFmpeg encoders |
| Audio bitrate | **128 kbps** AAC / **96 kbps** Opus | transparent-enough everyday default |
| Resolution / frame rate | **unchanged** (copy source W×H and fps) | never upscale; never down-res by default — no synthetic loss |
| Pixel format | normalize to **yuv420p** on H.264 re-encode | maximizes player compatibility (some players reject yuv444/yuv422) |
| Container flags | MP4/MOV/M4V: **`-movflags +faststart`** | front-loaded `moov` → instant playback/stream |
| Rotation | **honoured** (rotation flag baked or preserved so orientation is correct) | iPhone portrait clips stay portrait |

These appear in the UI only if surfaced; the **basic** view shows at most a single
**Quality** control (Smaller file ↔ Better quality, mapping to CRF 28/23/18) for
the **re-encode** path; everything else lives behind **Advanced options**
(§1.6 / SSOT *How It Feels* 5). A remux exposes **no** quality control (nothing to
tune — streams are copied). v1 deliberately ships **no codec picker, no
resolution/fps picker, no two-pass toggle** — those are scope additions, not
defaults.

### Audio tracks

- **Default:** keep **all** audio tracks (re-mux copies them; re-encode transcodes
  each to AAC/Opus). MP4 and MKV both support multiple audio tracks → preserved.
- WEBM in practice carries a single audio track; if a multi-track source targets
  WEBM, ConvertIA keeps the **first/default** track (documented limitation, rare).
- A source with **no audio track** converts fine (silent video) — never an error.

### Subtitles & embedded tracks

- **MKV → MP4** is the important case. MP4 supports **`mov_text` (tx3g)** subtitles
  only. Policy:
  - **Text** subtitles (SRT, MOV_TEXT, WebVTT) → **converted to `mov_text`** and
    kept inside the MP4 (one engine, in the same invocation).
  - **Image** subtitles (PGS, VobSub/DVD) and **styled ASS/SSA** → **cannot** be
    represented as `mov_text`; they are **dropped** with a §2.9 note
    (`video_subs_dropped`) rather than failing the conversion. Burning subtitles
    into the picture is **out of v1** (irreversible, a real choice — parked).
- **MKV → MKV / → MP4-as-remux**: all subtitle/chapter/attachment streams that the
  target supports are **copied**; unsupported ones are dropped with the note.
- Chapters and font attachments: copied to MKV; dropped (with note) for MP4 where
  unsupported.

### Metadata / color

- **Metadata** (title, creation time, GPS/maker tags) is **copied** where the target
  supports it (`-map_metadata 0`). Privacy note: ConvertIA is offline and does not
  strip metadata by default — a "strip location/metadata" toggle is `[OPEN]` for a
  future release, not v1.
- **Color** primaries / transfer / matrix and HDR (BT.2020/PQ/HLG) tags are
  **preserved on remux**. On an **HDR→H.264 re-encode** the tags are kept but x264
  does not tone-map → an `[OPEN]` edge: HDR→SDR tone-mapping is *not* done in v1
  (out — specialist); HDR sources re-encoded to MP4 keep HDR signalling, which most
  everyday players handle. Flagged for the corpus.

### Long-running progress, cancellation, very large files

- FFmpeg's `-progress pipe:` (key=value `out_time_us` / `total_size` lines) feeds
  the §1.11 **real per-item progress bar**; the **denominator** is the source
  **duration** (from `ffprobe`), so progress is a true percentage even for a 2-hour
  film — never an indeterminate spinner (SSOT *How It Feels* 6).
- **Cancellation** routes through §1.7 (process-group kill); a cancelled re-encode
  leaves **no partial output** (§2.1 atomic write — FFmpeg writes to a temp path,
  atomic-renamed only on success). Cancelling keeps already-finished items.
- **Remux** of a multi-GB file is I/O-bound and near-instant; **re-encode** is
  CPU-bound and the slowest operation in the whole app. §1.10 owns the up-front
  **size/space pre-flight** (re-encode temp + output estimate) and the "too big /
  doomed for disk" fast-fail; this entry just notes that **video is the category
  most likely to trip those budgets**. Concurrency degree (how many FFmpeg jobs run
  at once) is owned by §0.9 — video re-encode is CPU-heavy, so a **low** parallelism
  (likely 1–2) is expected; final number lives there.

### Same-container ("self") conversions

Per SSOT *Never harm* (re-save same format keeps the original + adapted name),
MP4→MP4 / MOV→MOV / MKV→MKV / WEBM→WEBM / M4V→M4V are valid: ConvertIA **normalizes**
(remux + `+faststart`, re-index) without re-encoding — a genuinely useful
"fix a broken/un-streamable MP4" everyday action — and writes `name (1).mp4`
beside the source, never overwriting. The legacy-only containers (AVI/WMV/FLV/MPG/
3GP) have no self target because they aren't offered as targets at all.

### Hardware vs software encoding

v1 uses **software encoding only** (libx264 / libvpx-vp9). Hardware encoders
(NVENC / QSV / VideoToolbox / AMF) are **out of v1**: they are per-GPU, produce
visibly different quality, complicate the bundled-offline guarantee, and are a
classic source of "works on my machine" failures — exactly what an everyday
converter must avoid. Software-only keeps **one identical result on all three
platforms** (SSOT *Cross-platform, one product*). Revisit as an opt-in Advanced
acceleration later. `[OPEN]` (parked, low priority).

### [OPEN] items (genuinely undecided)

- **[OPEN] HEVC/H.265 default disposition.** When a source already holds **H.265**
  (common from iPhones), H.265 is *legal in MP4* so a lossless **remux** is
  possible — but H.265 does **not** "play everywhere" (older Windows/browsers lack a
  decoder), which contradicts the MP4-default rationale. Decision needed:
  **(a)** remux HEVC→MP4 verbatim (lossless, smaller, but less compatible) vs
  **(b)** re-encode HEVC→H.264 (lossy, larger, maximally compatible). Leaning **(b)
  for the everyday default** with **(a)** as a possible "keep original quality"
  Advanced toggle. Must be settled before the corpus run. (Same question applies to
  AV1-in-MP4.)
- **[OPEN] Auto-deinterlace default.** Auto-applying `yadif` to interlaced MPEG-2
  sources is proposed as the everyday-correct default, but deinterlacing is a
  judgement call (wrong field order looks worse). Confirm `yadif` (mode 0) as
  default-on for flagged-interlaced sources, default-off otherwise.
- **[OPEN] WEBM two-pass & AV1-as-WEBM-target.** v1 = single-pass VP9. Whether to
  offer two-pass (smaller, ~2× time) as Advanced, and whether SVT-AV1 is fast enough
  to ever be the WEBM target codec, are deferred.
- **[OPEN] Metadata/location stripping toggle.** Default = preserve metadata
  (`-map_metadata 0`). A privacy "strip location & metadata" Advanced toggle is a
  candidate but not v1.
- **[OPEN] MOV as an offered target at all.** MOV-as-target is included on the
  assumption Mac users occasionally want it; if the corpus/usability walkthrough
  shows no real demand (everyone wants MP4), MOV-as-target may be demoted to
  source-only to shrink the matrix. Validate during the §9 usability floor.
- **[OPEN] §3.4 dependency.** H.264/AAC encode availability per platform is owned by
  §3.4. If §3.4 ever marks them unavailable on a platform, **MP4-as-target there
  must fall back** — but MP4 is *the default for every source*, so a platform
  without H.264/AAC encode would have **no default target**, which is a product
  problem, not just a per-format note. This category **depends on §3.4 deciding
  ship-bundled on all three platforms**; flagged as the category's hardest external
  dependency.
```
