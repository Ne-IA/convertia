# P6 — Audio · Video · Cross-category (FFmpeg family)

> **Full audio + video coverage and the cross-category conversions on the proven
> P4 harness.** "Full coverage" = every enumerated `(source → target)` pair is
> backed by §6.4.5 corpus files + §6.4.3 per-pair integration tests and marked
> **`reliable`** in the §6.5 pair-status ledger on all three platforms. One engine
> for the whole phase: **FFmpeg / FFprobe** (the shared GPL-2.0+ binary, §3.6.1),
> staged + hardened here, invoked through the §2.12 isolation boundary built in P4.
> P6 resolves the deferred cross-category items (the extract-audio target subset
> floor; the to-GIF trim + size-guardrail caps) and registers per-format
> advanced-option DECLARATIONS (§1.6) against the P4-built options-panel shell.
>
> **Spec home:** [04-formats/audio](../spec/04-formats/audio.md),
> [04-formats/video](../spec/04-formats/video.md),
> [04-formats/cross-category](../spec/04-formats/cross-category.md),
> [03-engines-and-bundling §3.5.1](../spec/03-engines-and-bundling.md) (FFmpeg
> hardening: protocol-whitelist / curated-demuxers / concat-safe) +
> §6.1.3 build assertions, [06-build-test-release §6.5](../spec/06-build-test-release.md)
> (reliability gate). Index: [plan/README.md](README.md). Box format:
> [`_format.md`](_format.md).
>
> **Reads, never re-decides:** AAC / H.264 / AV1 target availability **reads** the
> §3.4 patent-disposition matrix homed in P4 (per-codec cell). The generic
> isolation wrapper (§2.12), the per-pair integration runner (§6.4.3), the
> pair-status ledger generator (§6.5.2), the corpus↔pair bijection guard (§6.4.3a),
> the options-panel shell (§1.6), progress/cancel, result-actions and the per-engine
> §6.1.3 / §7.2.3 assertion FRAMEWORKS are all built in P4 — P6 only registers /
> populates / runs them for this engine. The native CSV/TSV engine and `fs_guard`
> are P3's; not re-built here.
>
> **Internal §6.5 sub-gate (skeleton-review r3 guidance):** **all audio pairs are
> marked `reliable` before any video pair is attempted** — video on three platforms
> is the heaviest corpus run, so the audio-reliable milestone gives measurable
> intra-phase progress. The sub-gate box (P6.35 below) sits between the audio
> cluster and the video cluster and `needs:` every audio pair box.
>
> **This is the v0 BASE** — the smallest-atomic `[ ]` boxes are below, grouped under
> `### ` sub-headings; a later adversarial review will deepen, split and complete
> them.

---

## P6.x — FFmpeg engine staging, hardening & runtime wiring

> The FFmpeg/FFprobe sidecar must exist, be hardened (curated build + argv
> controls), checksum-verified, SBOM-rowed and runtime-wired through the §2.12
> boundary before any pair can be built. These boxes execute the per-engine variants
> of the P0.7-policy / P4-framework gates for FFmpeg specifically.

- [ ] **P6.1** [BUILD] Stage the FFmpeg + FFprobe sidecar per-OS (cache-keyed, target-triple-suffixed) · §6.1.3 §3.3 · G37
  needs: P4.27
  > `scripts/stage-engines` restores the `actions/cache`-hosted `ffmpeg-<ver>-<triple>` engine-asset cache (checksum-verified pinned-URL fetch on a miss), places the FFmpeg + FFprobe sidecars under `src-tauri/binaries/` target-triple-suffixed (`ffmpeg-x86_64-pc-windows-msvc.exe`, …), and declares them in `tauri.conf.json` `bundle.externalBin`. The single GPL-2.0+ binary serves audio/video/cross-category. → executes the P0.7.3/P0.7.4 acquisition+staging policy for FFmpeg.
- [ ] **P6.2** [BUILD] Anchor the FFmpeg engine acquisition + add its `engines.lock` row + SBOM rows · §3.7.2 §3.8 · G37 G35 G36
  needs: P6.1
  > add the FFmpeg `engines.lock` row (`purl` `pkg:generic/ffmpeg@<ver>` + SHA-256 + the FFmpeg CPE `cpe:2.3:a:ffmpeg:ffmpeg:<ver>`), corroborate the unsigned gyan/BtbN prebuilt per the P0.7.3 named-anchor (≥2 mirrors / distro-signed) or the from-source signed-tarball path; populate the CycloneDX SBOM rows for FFmpeg + its nested component libs (`libmp3lame` LGPL, `libvorbis`/`libogg`/`libopus`/`libvpx` BSD, `libx264` GPL) with SPDX + source URLs. → executes the P0.7.1/P0.7.3 policy for FFmpeg.
- [ ] **P6.3** [BUILD] Assert the FFmpeg GPL license posture — `--enable-gpl`, hard-fail `--enable-nonfree`/`libfdk_aac` · §3.6.1 §6.1.3 · G38 G36
  needs: P6.1
  > the §6.1.3 configure-flag / whole-binary-license assertion: assert the staged FFmpeg was built `--enable-gpl` (x264 ⇒ whole binary GPL-2.0+) and HARD-FAIL on `--enable-nonfree`/`--enable-libfdk-aac` (nonfree taint = non-redistributable, distinct from the x264 GPL relicense); record the configuration line. The native built-in `aac` encoder is the license-clean choice.
- [ ] **P6.4** [BUILD] Build + assert the curated decoder/muxer coverage (generated `ffmpeg-required-decoders.lock`) · §6.1.3 §3.4.3 · G38
  needs: P6.1
  > generate `ffmpeg-required-decoders.lock` by parsing the §04 audio/video/cross-category source matrices, run `ffmpeg -decoders`/`-muxers` on the staged binary, and FAIL the build if any required decoder/muxer is absent. The generated floor must cover `hevc`/`h264`/`av1` (FFmpeg's OWN native video decoders — never the image-worker's libde265/dav1d), `mpeg4`/`msmpeg4v2`/`msmpeg4v3`/`mjpeg`/`vc1`/`h263`/`mpeg1video`/`mpeg2video`/`flv1`/`vp6a`/`vp6f`/`vp8`/`vp9`/`mp2`/`dca`/`ac3`/`amrnb`/`wmav1`/`wmav2`/`wmapro`/`wmalossless` plus `aac`/`vorbis`/`opus`/`alac`/`flac`/`pcm`.
- [ ] **P6.5** [BUILD] Build + assert the curated ENCODER coverage (generated `ffmpeg-required-encoders.lock`) · §6.1.3 · G38
  needs: P6.1
  > generate `ffmpeg-required-encoders.lock` by parsing the §04 TARGET matrices, run `ffmpeg -encoders` on the staged binary, FAIL the build if any required encoder is absent. Floor must include native `aac`, `alac`, `flac`, `pcm_s16le`/`pcm_s16be`/`pcm_s24le`/`pcm_s24be`/`pcm_f32le`, `libmp3lame`, `libvorbis`, `libopus`, `libx264`, `libvpx-vp9`. A `--disable-everything` trim cannot silently drop a needed encoder.
- [ ] **P6.6** [BUILD] Assert the network-protocol-family-ABSENT curated build (the primary SSRF floor) · §3.5.1 §6.1.3 §0.11 · G38 G42
  needs: P6.1
  > the §3.5.1 LOAD-BEARING structural SSRF floor (T9b): assert the network protocol family (`http`/`https`/`tcp`/`tls`/`rtmp`/`rtsp`/`hls`-fetch/`srtp`) is ABSENT at configure time (`--disable-network` preferred wholesale; no `--enable-protocol=` for the network family), proven by `ffmpeg -protocols` on the staged binary — the build FAILS if any network protocol is present. With the network family unbuilt, even a pre-whitelist demuxer dereference (CVE-2023-6605 class) has no transport.
- [ ] **P6.7** [BUILD] Assert the dereferencing-demuxer-ABSENT curated build (the LFR half) · §3.5.1 §6.1.3 §0.11 · G38 G42b
  needs: P6.1
  > run `ffmpeg -demuxers` on the staged binary and FAIL the build if a playlist/manifest dereferencing demuxer ConvertIA does not need is present (local-HLS `hls`, DASH `dash`, `image2` glob/pattern, external-reference EXTF). No §04 pair needs a playlist demuxer (single self-contained files only). This is the absolute-file LFR half that `-protocol_whitelist file,pipe` does NOT cover.
- [ ] **P6.8** [RUST] Wire the FFmpeg invocation through the §2.12 isolation boundary with the minimal-env + loader-strip + cwd=scratch contract · §3.5.1 §2.12 §2.14 · G29
  needs: P6.1, P4.13, P4.14
  > register FFmpeg in the §3.2 `Engine` registry; route every invocation through the **P4-built §2.12 isolation wrapper (P4.13 cheap-tier floor + P4.14 loader-var strip)** with cwd = per-run scratch (§2.14), minimal isolated env (`LC_ALL=C.UTF-8`, no proxy vars, `PATH` not relied on — absolute bundled path), and the explicit dynamic-loader-injection strip (`LD_PRELOAD`/`LD_LIBRARY_PATH`/`DYLD_INSERT_LIBRARIES`/`DYLD_LIBRARY_PATH` cleared, G29 `.env_clear()` invariant). Untrusted A/V parsed in FFmpeg demuxers/decoders = classic attack surface. (`needs: P4.13/P4.14` — the P4 isolation wrapper this engine routes through, per the P6.78 reconciliation obligation.)
- [ ] **P6.9** [RUST] Apply the always-on global FFmpeg flags + the argv SSRF/LFR defence-in-depth · §3.5.1 · G38 G42 G42b
  needs: P6.8
  > prepend the global flags to every FFmpeg job: `-nostdin -hide_banner -loglevel error -y` (`-y` safe — target is the §2.1 temp path, `-nostdin` prevents the parent-stdin-consume hang); the per-input `-protocol_whitelist file,pipe` (defence-in-depth on the network-absent build, set before each input); and NEVER `-safe 0` on the concat demuxer (`-safe 1` default rejects absolute/`..` paths). These are the runtime half of the §3.5.1 control.
- [ ] **P6.10** [RUST] Wire FFprobe stream inspection + the `ProbeOutput` → `plan_encode` denominator path · §3.5.1 §1.7 §1.11 · G31
  needs: P6.8, P4.9
  > `ffprobe -v error -print_format json -show_streams -show_format <input>` (probe-first for video) parsed into `ProbeOutput` (inner codecs / duration / rotation / interlace); the duration becomes the §1.11 progress denominator carried into `Engine::plan_encode(.., &probe)` (§3.2.1/§3.2.2) which builds the encode `Invocation` with `ProgressModel::FfmpegKeyValue { duration_us }` already populated — NO placeholder-then-mutate. (`needs: P4.9` — the P4-built two-step probe-then-encode sequencing this populates, per the P6.78 reconciliation obligation.)
- [ ] **P6.11** [RUST] Wire the FFmpeg `-progress pipe:1` parser into the §1.11 real per-item progress bar · §3.5.1 §1.11 · G31
  needs: P6.10, P4.8
  > `-progress pipe:1 -nostats` → key=value lines (`out_time_us=`/`total_size=`/`progress=continue|end`) parsed into `ProgressModel::FfmpegKeyValue`; until the first `out_time_us` tick the bar reads `Spawning`/indeterminate-but-working, then a true % against the probe duration — never a spinner (the SSOT *How It Feels* 6 promise for long video re-encodes). (`needs: P4.8` — the P4-built per-`ProgressModel` stdout line-reader dispatch this feeds, per the P6.78 reconciliation obligation.)
- [ ] **P6.12** [RUST] Wire the FFmpeg exit/stderr → §2.8 error-kind mapping (`classify_failure`) · §3.5.1 §2.8 · G31
  needs: P6.8, P4.48
  > map known stderr patterns: "could not find codec parameters"/"Invalid data" → corrupt; the cross-category "No audio" path → the named `NoAudioTrack` kind; DRM/"Operation not permitted" on FairPlay/WMV → the §video.md "copy-protected" message; everything else → generic engine-failure (still plain-language §2.13). A crashing/hanging decode fails THAT one item and the batch continues. (`needs: P4.48` — the P4-built capture-and-classify-into-§2.8 generic seam this fills the FFmpeg classifier for, per the P6.78 reconciliation obligation.)
- [ ] **P6.13** [RUST] Wire cancellation via process-group kill + the no-partial-output guarantee for FFmpeg · §3.5.1 §1.7 §2.1
  needs: P6.8, P4.10, P4.11
  > cancellation routes through the **P4-built §1.7 process-group kill (P4.10) + the kill↔cleanup↔no-partial ordering (P4.11)** (FFmpeg may spawn children); a cancelled re-encode leaves NO partial output (FFmpeg writes to the §2.1 temp `out_tmp`, atomic-renamed only on success); already-finished batch items are kept. (`needs: P4.10/P4.11` — the P4 cancel/kill mechanism, per the P6.78 reconciliation obligation.)
- [ ] **P6.14** [TEST] Add the per-engine FFmpeg §7.2.3 availability/integrity row + the in-bundle hash-manifest entry · §7.2.3 · G46 G37
  needs: P6.2, P4.42
  > populate the FFmpeg + FFprobe rows in the build-time in-bundle hash manifest and the `EngineHealth` availability table (the per-engine variant of the **P4-built §7.2.3 startup-verifier framework, P4.42**) so a missing/corrupt FFmpeg escalates to a §2.13 app-fault, not a crash, and feeds C12 `get_engine_health` (§5.2 disables unavailable targets). (`needs: P4.42` — the P4 integrity-verifier framework this populates a row in, per the P6.78 reconciliation obligation.)

---

## P6.x — Audio pairs (FFmpeg, audio-file → audio-file)

> Every audio source→target pair is one FFmpeg invocation (decode → re-encode);
> §3.2 single-engine-per-pair holds trivially, no chaining. Defaults + advanced
> presets + lossy flags + tag policy are owned by [audio.md](../spec/04-formats/audio.md);
> these boxes wire them. WMA is **source-only** (`→ WMA` parked out of v1). The
> diagonal (same→same) is `—` for v1. Each per-target encode box `needs:` the
> shared FFmpeg runtime wiring (P6.9/P6.10/P6.12).

- [ ] **P6.15** [RUST] Wire the audio-source detection signatures + codec-in-container disambiguation · §1.2 · G15 G31
  needs: P6.10
  > add the §1.2 per-format audio signatures: MP3 frame-sync `FF Fx`/ID3 (layer-disambiguated from MP1/MP2); WAV RIFF/WAVE + `fmt ` chunk; FLAC `fLaC`; raw-ADTS AAC `FF F1`/`FF F9` (ADTS-vs-MP3 by header parse); M4A `ftyp` reading codec from `stsd`/`esds` (AAC ⇒ "M4A", ALAC ⇒ "ALAC"); OGG `OggS`+`\x01vorbis` vs OPUS `OggS`+`OpusHead`; AIFF `FORM`/`AIFF`/`AIFC`; ALAC = M4A `ftyp` + `alac` in `stsd`; WMA ASF GUID + stream-properties codec id. Codec id is authoritative over extension (a `.mp3` that is really FLAC, a `.m4a` that is really ALAC).
- [ ] **P6.16** [RUST] Wire the audio `-map_metadata 0` tag-carry + per-container cover-art mechanism · §3.5.1 §1.2 · G31
  needs: P6.15
  > `-map_metadata 0` for tag carry (title/artist/album/year/genre/track/comment + CJK/RTL UTF-8 preserved, §2.10); cover-art mechanism BY CONTAINER: MP3/M4A/FLAC = attached-picture video stream (`-map 0:v? -c:v copy`, `?` keeps audio-without-art working); OGG/OPUS = FLAC PICTURE metadata block via metadata copy (NOT `-c:v copy`); raw ADTS `.aac` + WAV/AIFF omit it (fire `audio_tags_dropped`).
- [ ] **P6.17** [RUST] Wire the channel-preservation + forced-downmix policy · §3.5.1 · G31
  needs: P6.15
  > preserve source channel layout by default for every target; for >2-channel sources → MP3/OGG add `-ac 2` and fire the §2.9 `audio_downmix` note; AAC/M4A/OPUS/FLAC/WAV preserve the source layout (no forced downmix). No silent resample/bit-depth change in the default path.
- [ ] **P6.18** [RUST] Wire the MP3 target — `libmp3lame -q:a 2` VBR default · §3.5.1 · G31
  needs: P6.9, P6.16
  > MP3 encode `-c:a libmp3lame -q:a 2` (VBR ≈190k default); always-lossy-as-target. MP3 is the default target of every audio source except MP3 itself. (audio.md MP3 entry.)
  - [ ] **P6.18.1** [RUST] Wire the MP3 quality advanced-option mapping (V0/V2/V5 + CBR 128/192/320) · §1.6 · G31
    > the "MP3 quality" preset set → `-q:a N` (VBR High V0 / Standard V2 / Small V5) or `-b:a Nk` (CBR 128/192/320); this same canonical MP3 preset table is reused verbatim by cross-category extract-audio→MP3 (OPEN-B). The "MP3 quality" UI option DECLARATION is the top-level box P6.70 (registered against the P4 panel).
- [ ] **P6.19** [RUST] Wire the WAV target — `pcm_s16le` 16-bit default + bit-depth advanced option · §3.5.1 · G31
  needs: P6.9, P6.16
  > WAV encode `-c:a pcm_s16le` via `wav` muxer, 16-bit default, sample-rate/channels preserved; lossless-as-target except the >16-bit-source → default-16-bit bit-depth-reduction case (fire `audio_bitdepth`). Weak WAV tag model → fire `audio_tags_dropped` only when the source carried tags. The "WAV bit depth" UI option DECLARATION is the top-level box P6.71.
- [ ] **P6.20** [RUST] Wire the FLAC target — `flac -compression_level 5` default + level advanced option · §3.5.1 · G31
  needs: P6.9, P6.16
  > FLAC encode `-c:a flac -compression_level 5` (level changes size/speed only, never the audio); lossless-as-target; lossy-ORIGIN flagged `audio_lossy_origin` (no quality gain). Vorbis comments + PICTURE block round-trip. The "FLAC compression" UI option DECLARATION is the top-level box P6.72.
- [ ] **P6.21** [RUST] Wire the AAC target — native `aac -b:a 192k` CBR + adts muxer, reading §3.4 availability · §3.5.1 §3.4 · G31
  needs: P6.9, P6.16
  > AAC encode native `-c:a aac -b:a 192k` CBR (native-encoder VBR is unstable) + muxer `adts` (raw `.aac`); always-lossy; raw ADTS has NO tag container → cover art + tags DROPPED (`audio_tags_dropped`). READS the §3.4 AAC per-platform cell (P4 matrix) — if AAC is unavailable on a platform, the AAC target is honestly disabled there (never re-decided here). The "AAC quality" UI option DECLARATION is the top-level box P6.73.
- [ ] **P6.22** [RUST] Wire the M4A target — native `aac` + `ipod` muxer + faststart, reading §3.4 (inherits AAC) · §3.5.1 §3.4 · G31
  needs: P6.21
  > M4A encode native `-c:a aac -b:a 192k` + muxer `ipod` (`.m4a`) + `-movflags +faststart`; identical quality knobs to AAC; KEEPS metadata (iTunes `ilst` atoms + cover art) — M4A's advantage over raw `.aac`. INHERITS AAC's §3.4 disposition (the codec is AAC); M4A-holding-ALAC is the separate ALAC target. The "M4A quality" UI option DECLARATION is the top-level box P6.74.
- [ ] **P6.23** [RUST] Wire the OGG (Vorbis) target — `libvorbis -q:a 3` VBR + ogg muxer · §3.5.1 · G31
  needs: P6.9, P6.16
  > OGG encode `-c:a libvorbis -q:a 3` (≈112k) muxer `ogg` — DISTINCT from OPUS (the OGG target is always Vorbis, never Opus); always-lossy; Vorbis comments + cover-art-as-PICTURE-block. No patent flag (royalty-free). The "OGG quality" UI option DECLARATION is the top-level box P6.75.
- [ ] **P6.24** [RUST] Wire the OPUS target — `libopus -b:a 128k` VBR + opus muxer (48 kHz internal) · §3.5.1 · G31
  needs: P6.9, P6.16
  > OPUS encode `-c:a libopus -b:a 128k` (`-vbr on`) muxer `opus`; FFmpeg resamples to Opus's 48 kHz internal rate transparently (not a user-visible loss); always-lossy; never the per-source DEFAULT (older players may not open `.opus`). No patent flag. The "OPUS bitrate" UI option DECLARATION is the top-level box P6.76.
- [ ] **P6.25** [RUST] Wire the AIFF target — `pcm_s16be` 16-bit big-endian + aiff muxer · §3.5.1 · G31
  needs: P6.9, P6.16
  > AIFF encode `-c:a pcm_s16be` muxer `aiff`, 16-bit big-endian default; lossless-as-target with the same >16-bit-source → 16-bit `audio_bitdepth` caveat as WAV; limited AIFF tag model → `audio_tags_dropped` when the source carried tags. The "AIFF bit depth" UI option DECLARATION is the top-level box P6.77.
- [ ] **P6.26** [RUST] Wire the ALAC target — native `alac` + `ipod` muxer + faststart (lossless, no knob) · §3.5.1 · G31
  needs: P6.9, P6.16
  > ALAC encode `-c:a alac` muxer `ipod` (`.m4a` whose codec is ALAC) + `+faststart`; lossless, NO quality/compression knob exposed (FFmpeg's ALAC encoder has none) — the advanced view stays clean; lossy-ORIGIN flagged `audio_lossy_origin`. NO patent flag (ALAC is open/royalty-free — never confuse with AAC's §3.4 status). Same `ilst` metadata + cover art as M4A.
- [ ] **P6.27** [RUST] Wire WMA as a DECODE-only source (no `→ WMA` target) + the source-options-from-target rule · §3.5.1 · G31
  needs: P6.15
  > WMA decoders `wmav1`/`wmav2`/`wmapro`/`wmalossless` all decode-capable; `→ WMA` is PARKED out of v1 (no target wiring — only `wmav2` exists and it is low-quality legacy); as a source its options are the chosen target's; WMA→lossless-target = lossy-origin, WMA→lossy-target = second lossy round; ASF metadata mapped to tag-supporting targets.
- [ ] **P6.28** [TEST] Wire the per-source-default-target table (MP3 default for all except MP3→WAV) · §1.5 §1.6 · G31
  needs: P6.18, P6.19
  > the pre-highlighted default = MP3 for every audio source EXCEPT MP3 itself (→ WAV, since MP3→MP3 is excluded); a Lane-A defaults-registry assertion (§1.6) verifies the no-required-choices gate: dropping any audio and hitting convert with zero clicks produces the table's default. (DECIDED: MP3-source default is WAV over FLAC.)
- [ ] **P6.29** [TEST] Wire the audio lossy-disclosure trigger map (the `✓~` matrix cells ↔ §2.9 kinds) · §2.9 · G31 G32
  needs: P6.18, P6.19, P6.20, P6.21, P6.22, P6.23, P6.24, P6.25, P6.26
  > assert each §2.9 audio kind fires IFF the §04 matrix flags the pair: `audio_lossy_target` (any → MP3/AAC/M4A/OGG/OPUS), `audio_transcode` (lossy → lossy), `audio_lossy_origin` (lossy → FLAC/ALAC ONLY — deliberately NOT WAV/AIFF), `audio_bitdepth` (>16-bit → default 16-bit WAV/AIFF), `audio_tags_dropped` (→ raw AAC / WAV / AIFF when source had tags), `audio_downmix` (forced codec downmix). The G32 lossy-disclosure property holds over the `FormatId×FormatId` product.

---

## P6.x — Audio corpus + per-pair audio tests

> The §6.4.5 audio corpus + the per-pair §6.4.3 integration tests that let each
> audio pair reach `reliable`. The corpus↔pair bijection guard (§6.4.3a, built in
> P4) fails Lane A if any audio pair has no backing file, so the corpus boxes
> precede the test boxes.

- [ ] **P6.30** [TEST] Stage the audio corpus (one file per source format) + its manifest + SHA-256 entries · §6.4.5 · G24a G22
  needs: P6.1
  > add `tests/corpus/audio/` files: one per source format (MP3 VBR/CBR + ID3v2 + cover; WAV 16/24/float; FLAC + Vorbis comments + cover; raw-ADTS `.aac`; M4A-holding-AAC AND a separate M4A-holding-ALAC; OGG-Vorbis; `.opus`; AIFF; WMA v2/Pro/Lossless), each with a root-`manifest.toml` `[[file]]` (source / redistributable licence / `exercises` / `covers` 2-tuples / `[file.expect]`); regenerate the §6.4.5/P0.5.4 SHA-256 corpus manifest in the same commit (G24a). Files must be CC0/public-domain/self-produced/synthetic.
- [ ] **P6.31** [TEST] Stage the audio edge-case + content-floor corpus fixtures · §6.4.5 · G24a G31
  needs: P6.30
  > add the audio edge fixtures + content-floor tags: a multichannel (5.1) source (`audio_downmix` / channel-preservation); a >16-bit source (`audio_bitdepth`); files with non-Latin/CJK/RTL tag text (`non-latin-tags` content-floor tag, §2.10); corrupt/truncated + 0-byte + a `.mp3` that is really FLAC (mislabel) cases; cover-art round-trip fixtures for the MP3↔FLAC↔OGG↔OPUS↔M4A/ALAC set.
- [ ] **P6.32** [TEST] Add the audio per-pair integration tests (every audio pair, structural reader = ffprobe) · §6.4.3 §6.5 · G31 G32
  needs: P6.30, P6.31, P6.29, P4.58
  > for every enumerated audio `(source → target)` pair, against every corpus file of its source format, on all three platforms: completes with exit success; output validated by the MANDATORY structural reader (`ffprobe` decodes + reports the expected codec, stream count > 0 — NOT magic re-detect); no-harm (source `sha256` unchanged, atomic write, no-clobber); fail-clearly on the known-bad fixtures; lossy disclosure fires iff flagged; tag/cover-art/channel content-fidelity spot-checks. (§6.4.3 runner built in P4 — `needs: P4.58`, the per-pair runner, per the P6.78 reconciliation obligation.)
- [ ] **P6.33** [TEST] Add the audio determinism + cross-decoder re-validation sub-assertions · §6.4.3 §2.5 · G32 G38
  needs: P6.32
  > the §2.5/G32 determinism floor — same source+settings twice → `sha256(out1)==sha256(out2)` for ≥1 pair per output-format category (enumerated in the manifest); cross-decoder validation for headline formats via ffprobe; document known-non-deterministic encoders as manifest exceptions.
- [ ] **P6.34** [TEST] Add the per-push adversarial-egress + T9b-sentinel PULL-FORWARD leg for FFmpeg audio · §6.4.2 §2.11.4 §0.11 · G42 G42b
  needs: P6.32
  > the §6.4.2 per-push adversarial-egress + T9b-sentinel corpus run inside G42's egress-deny window (the P0.7.12 "per-push pull-forward" leg activating from P6 as the first egressing engine is staged): a crafted network-trigger A/V input must show ZERO egress (incl. zero DNS) AND no out-of-input file read, so a T9b regression is caught on the push that introduces it. (Full per-OS deny window + release-confirmation leg are P9.)

---

## Internal §6.5 sub-gate — audio reliable before video

- [ ] **P6.35** [TEST] Sub-gate — assert every audio pair is `reliable` in the ledger before any video pair is attempted · §6.5 §6.5.2 · G31
  needs: P6.32, P6.33, P6.34
  > the skeleton-review-r3 intra-phase milestone: assert the §6.5.2 pair-status ledger (`reliability-report.json`) marks EVERY enumerated audio pair `reliable` on all three available platforms (or `unavailable-per-§3.4` for the AAC/M4A cells) before the video cluster begins — video on three platforms is the heaviest corpus run, so the audio-reliable milestone gives measurable progress. Every subsequent video box transitively follows this via the runner; the gate is the named checkpoint.

---

## P6.x — Video pairs (FFmpeg, container conversions + remux/re-encode)

> The user-facing format is the CONTAINER (§1.3 batch key); what is cheap depends
> on the inner CODECS (probed by FFprobe). Remux-vs-re-encode is decided
> AUTOMATICALLY per item from the inner-codec inventory — never asked. MP4 is the
> pre-highlighted default for every video source. AVI/WMV/FLV/MPG/3GP are valid
> SOURCES but NOT offered as targets. Each video box `needs:` the audio sub-gate
> (P6.35).

- [ ] **P6.36** [RUST] Wire the video-source detection signatures + brand/DocType disambiguation · §1.2 · G15 G31
  needs: P6.35
  > add the §1.2 video signatures: MP4-family `ftyp` with brand disambiguation (`isom`/`mp4x`/`avc1` = MP4, `qt  ` = MOV, `M4V `/`M4VH` = M4V, `3gp4`/`3g2a` = 3GP — brand not extension); MOV `moov`/`mdat`/`wide` for ftyp-less QuickTime; MKV/WEBM EBML `1A 45 DF A3` disambiguated by DocType (`matroska` vs `webm`); AVI RIFF+`AVI `; WMV ASF GUID + video-stream-present (vs WMA audio-only); FLV `FLV`+version; MPG/MPEG start codes `00 00 01 BA`/`B3` + `.ts` sync `0x47`.
- [ ] **P6.37** [RUST] Wire the automatic remux-vs-re-encode decision from the FFprobe inner-codec inventory · §3.5.1 §3.2 · G31
  needs: P6.36, P6.10
  > the §3.5/video.md per-item decision (a §3.2 capability decision, zero user choice): remux (`-c copy`, lossless) IFF every kept stream's codec is legal in the target container AND no normalization is needed; else re-encode (decode → H.264/AAC or VP9/Opus, lossy); MIXED allowed (video copies while audio transcodes) — still ONE FFmpeg invocation. Per-item from the inventory, never an always-remux path (FLV VP6/Sorenson, WMV7/8, MKV-only audio all force re-encode).
- [ ] **P6.38** [RUST] Wire the H.264 re-encode params + faststart + yuv420p + rotation, reading §3.4 H.264 · §3.5.1 §3.4 · G31
  needs: P6.37
  > re-encode path for MP4/MOV/MKV/M4V: `-c:v libx264 -crf 23 -preset medium -pix_fmt yuv420p` + `-c:a aac -b:a 128k`; `-movflags +faststart` (front-loaded moov) for MP4/MOV/M4V; `-fflags +genpts` for FLV remux; rotation honoured (portrait stays portrait); resolution/fps unchanged (never upscale). READS the §3.4 H.264/AAC cell (P4 matrix) — ship-bundled on all three platforms is the category's hardest dependency (MP4 is every source's default).
- [ ] **P6.39** [RUST] Wire the VP9/Opus WEBM-target re-encode params (constant-quality, single-pass) · §3.5.1 · G31
  needs: P6.37
  > WEBM target `-c:v libvpx-vp9 -b:v 0 -crf 32 -row-mt 1` (constant-quality, single-pass — two-pass + AV1-as-WEBM-target are DECIDED-not-in-v1) + `-c:a libopus -b:a 96k`; → WEBM is ALWAYS lossy re-encode (codecs never match the H.264/AAC mainstream). VP9 CRF validation bound is `0..=63` (15–35 recommended band, default 32) — must not clamp the codec range.
- [ ] **P6.40** [RUST] Wire the HEVC/H.265-default disposition (re-encode to H.264) + the keep-original Advanced toggle · §3.5.1 §3.4 · G31
  needs: P6.38
  > the DECIDED HEVC default: an H.265 source (common iPhone `.mov`) re-encodes HEVC→H.264 by DEFAULT (lossy, larger, plays everywhere — honours the usability-floor mov→mp4 promise) using FFmpeg's native `hevc` DECODER (inside the GPL binary, never libde265); a "keep original quality (H.265)" Advanced toggle offers verbatim remux. Same disposition for AV1-in-MP4. Decode reads the §3.4 HEVC-video-decode cell.
  - [ ] **P6.40.1** [UI] Register the "keep original quality (H.265)" Advanced-option DECLARATION · §1.6 · G47
    > the verbatim-remux toggle, default OFF (re-encode is the default); same toggle covers AV1-in-MP4.
- [ ] **P6.41** [RUST] Wire the audio-tracks + subtitles + chapters/attachments keep/convert/drop policy · §3.5.1 · G31
  needs: P6.37
  > keep ALL audio tracks (remux copies; re-encode transcodes each to AAC/Opus; WEBM keeps first track); MKV→MP4 subtitles: TEXT (SRT/MOV_TEXT/WebVTT) → converted to `mov_text` in the same invocation; IMAGE (PGS/VobSub) + styled ASS/SSA → DROPPED with `video_subs_dropped` (no subtitle burn-in in v1); chapters + font attachments copied to MKV, dropped-with-note for MP4 where unsupported.
- [ ] **P6.42** [RUST] Wire the auto-deinterlace (yadif) + metadata/color/HDR preservation + alpha-loss note · §3.5.1 · G31
  needs: P6.37
  > `yadif` (mode 0) deinterlace default-ON for flagged-interlaced sources (DEFER:corpus calibrates only the call, not the design); `-map_metadata 0` metadata preserve (no strip toggle in v1); color primaries/transfer/matrix + HDR (BT.2020/PQ/HLG) preserved on remux, kept-as-signalling on H.264 re-encode (no tone-map in v1); WEBM-alpha → H.264 fires `video_alpha_lost`.
- [ ] **P6.43** [RUST] Wire the per-format video target registrations (MP4/MOV/MKV/WEBM/M4V) + the self-conversion normalize path · §3.5.1 · G31
  needs: P6.38, P6.39
  > register the five offered video targets (MP4, MOV, MKV, WEBM, M4V) reading each source's offered set + the `R`/`✓~` disposition from the video.md matrix; the same-container "self" path (MP4→MP4 etc.) NORMALIZES (remux + `+faststart` + re-index, no re-encode) and writes `name (1).mp4` beside the source (no overwrite). AVI/WMV/FLV/MPG/3GP have NO self target (not offered as targets). Software-only encoding (no NVENC/QSV/VideoToolbox in v1).
- [ ] **P6.44** [TEST] Wire the every-source-default-is-MP4 zero-click assertion · §1.6 · G31
  needs: P6.38
  > a §1.6 defaults-registry assertion: dropping ANY video and hitting convert with zero clicks produces a valid MP4 (MP4 is the pre-highlighted default for all ten sources); flag the §3.4 H.264/AAC-ship-bundled-on-all-three-platforms hard precondition (a platform with no H.264 encode would have no default target — a product problem, not a footnote).
- [ ] **P6.45** [TEST] Wire the worst-case `willReencode` note + the §1.12 actual-disposition summary · §2.9 §2.9.2 §0.4.2 · G31 G32
  needs: P6.37
  > the §2.9.2 timing rule: the target-choice note is a header/container-pair worst-case (`RunStarted.willReencode`, §0.4.2) — a definitely-re-encode pair (→WEBM, legacy source) fires `video_reencode` certainly; a commonly-remux pair fires the "may be re-encoded" worst-case rather than falsely promising losslessness; the §1.12 end-of-batch summary reflects what ACTUALLY happened once §3.5 resolved the real per-item disposition. G32 lossy-disclosure-iff-flagged uses the PLANNED disposition.
- [ ] **P6.46** [RUST] Wire the DRM-protected + zero-audio + very-large video edge handling · §3.5.1 §1.10 · G31
  needs: P6.12, P6.37
  > DRM (FairPlay `.m4v`, PlaysForSure WMV/ASF) → the §video.md "copy-protected, can't be converted" message, batch continues, nothing written; a source with no audio track converts fine (silent video, never an error); §1.10 owns the up-front size/space pre-flight + "too big" fast-fail (video is the category most likely to trip the budgets); concurrency degree owned by §0.9 (low parallelism for CPU-heavy re-encode).

---

## P6.x — Video corpus + per-pair video tests

- [ ] **P6.47** [TEST] Stage the video corpus (short clips, one per source + the inner-codec cases) + manifest + SHA-256 · §6.4.5 · G24a G22
  needs: P6.1
  > add `tests/corpus/video/` short clips: MP4 (H.264+AAC, lossless-remux baseline); MOV-from-iPhone (HEVC, the re-encode-default case); MKV with multiple audio tracks + SRT + ASS + PGS subtitles + chapters + font attachments; WEBM (VP9+Opus, and a VP8 alpha clip); AVI (DivX+MP3); WMV (VC-1+WMA); FLV (H.264/AAC and old Sorenson); MPG (interlaced MPEG-2 + AC-3); M4V (DRM-free); 3GP (H.263+AMR-NB) — each with its `manifest.toml` `[[file]]` + redistributable licence; regenerate the SHA-256 corpus manifest in the same commit (G24a).
- [ ] **P6.48** [TEST] Stage the video edge-case fixtures (DRM, rotation, VFR, silent, interlace) + content-floor `representative-av` · §6.4.5 · G24a G31
  needs: P6.47
  > add a DRM-protected FairPlay `.m4v` + a DRM WMV (fail-clearly); a portrait/rotated clip (rotation honoured); a VFR screen recording (to-GIF fps-normalise); a silent clip (extract-audio "no audio track"); a long-ish clip for the to-GIF guardrail/cap; tag the `representative-av` content floor (≥1 real video, already implied by the per-format rows).
- [ ] **P6.49** [TEST] Add the video per-pair integration tests (every container pair, structural reader = ffprobe + remux-correctness) · §6.4.3 §6.5 · G31 G32
  needs: P6.47, P6.48, P6.45, P4.58
  > for every enumerated video `(source → target)` container pair, against every corpus file of its source, on all three platforms: completes + output decodes via `ffprobe` (expected codec, stream count > 0); no-harm + fail-clearly on DRM/corrupt fixtures; remux-vs-re-encode chose the LOSSLESS path when codecs already fit (the key video content-fidelity check); lossy disclosure fires per the PLANNED disposition; rotation/subtitle/chapter content-fidelity spot-checks; patent-gapped targets asserted absent (not attempted) where §3.4 marks unavailable. (`needs: P4.58` — the P4-built §6.4.3 per-pair runner, per the P6.78 reconciliation obligation.)
- [ ] **P6.50** [TEST] Add the video determinism note + the per-push adversarial-egress leg for FFmpeg video · §6.4.2 §6.4.3 §2.11.4 · G32 G42 G42b
  needs: P6.49
  > extend the §6.4.2 per-push adversarial-egress + T9b-sentinel run to the video corpus (crafted external-reference/manifest-bearing video → zero egress incl. DNS + no out-of-input read); document VP9/AV1 variable-encode as known-non-deterministic G32 manifest exceptions (no `sha256` determinism floor for those, a `diffoscope`-localised note instead).

---

## P6.x — Cross-category: extract-audio (video → audio subset)

> Operations on a video source, NOT a second source format. The batch key is the
> video source type only (§1.3); the offered target set = the video targets PLUS
> "extract audio (→ …)" PLUS "to animated GIF". One FFmpeg invocation (demux +
> optional re-encode). Resolves the deferred [OPEN-A] subset (floor MP3★+WAV+FLAC
> guaranteed; M4A/OGG corpus-validated).

- [ ] **P6.51** [RUST] Wire extract-audio as a target of every video source (`-vn -map 0:a:0`) + the first-track rule · §3.5.1 §1.5 · G31
  needs: P6.35, P6.36
  > offer extract-audio on all ten v1 video sources (§1.5 target resolution adds it alongside the video default); `-vn -map 0:a:0` (deterministic FIRST audio track in v1 — per-track / all-tracks is parked, no one-to-many fan-out); preserve source sample rate + channels (no resample/downmix by default); carry source tags where the target container supports them; cover-art extraction is NOT part of extract-audio.
- [ ] **P6.52** [RUST] Wire the GUARANTEED extract-audio target floor (MP3★/WAV/FLAC) · §3.4 · G31
  needs: P6.51, P6.18, P6.19, P6.20
  > register the [OPEN-A] **guaranteed floor** {MP3★ default, WAV, FLAC} (C3-derivable now once the MP3/WAV/FLAC encode paths P6.18–P6.20 are done — the SSOT mov→mp3 case in scope; this leg can be checked off immediately, **independent of any corpus evidence**). Excluded as extract targets: raw AAC, OPUS, WMA, AIFF, ALAC. Reuse the audio.md encode params + the canonical MP3 preset table verbatim ([OPEN-B] resolved). The corpus-validated **M4A/OGG additions** are the separate box P6.69 (they require P9.44 corpus evidence and must NOT block the floor's check-off — split per the atomicity bar).
- [ ] **P6.53** [RUST] Wire the extract-audio stream-copy-vs-re-encode decision (codec-inside-container) · §3.5.1 · G31
  needs: P6.52
  > automatic per-item: `-c:a copy` (lossless, fast) when source codec is byte-compatible with the chosen target container — source AAC → M4A (the dominant MP4/MOV/M4V/3GP case), source MP3 → MP3 (FLV/AVI), source Vorbis → OGG (WebM); else re-encode (any → MP3/WAV always-decode-to-PCM/FLAC-lossless, AAC→MP3, etc.). Engine-internal §3.2 capability decision, zero user choice; the lossy note reflects the OUTCOME not the mechanism.
- [ ] **P6.54** [RUST] Wire the M4A-extract-target §3.4 gate at the target level (copy path noted, gate at M4A) · §3.4 · G31
  needs: P6.53
  > the DECIDED rule: the AAC→M4A `-c:a copy` path decodes/remuxes only (lighter patent profile, no encode) — NOTED — but to keep the format×platform offered set honest, if §3.4 marks AAC unavailable on a platform the M4A extract target is DISABLED there REGARDLESS of copy-vs-encode, falling back to MP3 (already the default, no UX disruption). One consistent availability story per platform.
- [ ] **P6.55** [RUST] Wire the extract-audio NoAudioTrack named-failure + edge cases · §2.8 · G31
  needs: P6.51, P6.12
  > the §2.8 `NoAudioTrack` kind ("This file has no audio to extract.") on a silent source — a NAMED failure, batch continues, never a 0-byte audio file ([OPEN-C] cheap up-front probe to disable-with-reason is DEFER:corpus); multichannel (5.1) preserved into WAV/FLAC (not auto-downmixed); corrupt/truncated source → item fails clearly, no partial audio (§2.1/§2.6); WAV/FLAC extraction cannot un-bake the source's existing lossy compression (the §2.9 note must not imply quality improvement).
  - [ ] **P6.55.1** [UI] Register the extract-audio quality advanced-option DECLARATIONS (per-target, reusing audio.md presets) · §1.6 · G47
    > MP3 quality (Standard ≈ V2 default), M4A re-encode bitrate, FLAC compression level, OGG quality — all reusing the audio.md canonical preset tables; WAV has none (fixed 16-bit PCM).

---

## P6.x — Cross-category: to-animated-GIF (video → GIF, with guardrails)

> Turn a short video clip into a shareable animated GIF — one FFmpeg invocation via
> the split/palettegen/paletteuse filtergraph (no temp PNG, no chaining). ALWAYS
> intrinsically lossy. Resolves the deferred to-GIF trim + size-cap items
> ([OPEN-E]/[OPEN-F] finite ship-now values, corpus-calibrated).

- [ ] **P6.56** [RUST] Wire to-GIF as a target of every video source via the single-process palette filtergraph · §3.5.1 §3.2 · G31
  needs: P6.35, P6.36
  > offer to-GIF on all ten v1 video sources; build the single-invocation filtergraph `fps=<fps>,scale=<w>:-1:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5` + `-loop 0` — NO intermediate palette PNG (one §3.2 engine call, no chaining); `lanczos` scale + per-clip palette = quality, `fps`-down + width-cap = sane size; audio dropped (intrinsic), transparency not preserved (opaque output).
- [ ] **P6.57** [RUST] Wire the to-GIF basic options — FPS + width defaults + the dither/loop/colours fixed defaults · §1.6 · G31
  needs: P6.56
  > FPS preset (Smooth 15 / Standard 12 / Small 10, default 12); Width preset (Large 640 / Medium 480 / Small 320 px, height `-1` aspect-kept, default 480); fixed defaults — dither `bayer:bayer_scale=5` ([OPEN-D] DECIDED), `-loop 0` infinite, 256 colours; VFR sources fps-normalised; odd dimensions even-rounded; sub-second clip → valid tiny GIF.
  - [ ] **P6.57.1** [UI] Register the to-GIF FPS + Width Basic-option DECLARATIONS · §1.6 · G47
    > FPS + Width in the Basic view (they visibly change smoothness/size vs file size).
  - [ ] **P6.57.2** [UI] Register the to-GIF dither Advanced-option DECLARATION (bayer/sierra2_4a/floyd_steinberg/none) · §1.6 · G47
    > the v1-exposed dither subset only (NOT `sierra2`/`heckbert` — FFmpeg accepts but v1 hides; `sjpeg` is not a valid value); default `bayer:bayer_scale=5`.
- [ ] **P6.58** [RUST] Wire the to-GIF trim window (start + duration) · §1.6 · G31
  needs: P6.56
  > the [OPEN-E] resolution (design leans Basic start+duration, validate §6.6): `-ss <start>` + `-t <duration>` in the same single invocation; default = whole clip up to the duration cap (P6.59). A trim window is most of why people make GIFs.
  - [ ] **P6.58.1** [UI] Register the to-GIF trim Basic-option DECLARATION (start + duration) · §1.6 · G47
    > two number fields ("from 00:15, for 6 s"); leaving defaults = whole clip up to cap.
- [ ] **P6.59** [RUST] Wire the to-GIF size guardrail — up-front estimate + duration cap + fail-fast (feeds §1.10) · §1.10 §2.8 · G31
  needs: P6.56, P6.58, P4.71
  > the MANDATORY guardrail (this op supplies the inputs; the **P4-built §1.10 engine (P4.71)** owns the threshold mechanics): up-front cheap estimate `fps × min(clip_len, trim_or_cap) × out_w × out_h × ~1 byte/px` (no decode needed); a finite default duration cap (proposal N=10 s, [OPEN-F] DEFER:corpus — MUST be some finite value, leaving it unset reintroduces the foot-gun) applied as `-t`; fail-fast up front if the estimate exceeds the §1.10 "too big" ceiling (a §2.8 named failure kind), batch continues; a cap that shortened the clip is a DISCLOSED outcome (`video_to_gif`), never silent truncation. (cross-category.md owns the GIF-specific defaults; this box feeds them into the §1.10 engine.)
- [ ] **P6.60** [RUST] Wire the to-GIF unconditional lossy note + HDR/4K edge handling · §2.9 · G31 G32
  needs: P6.56
  > to-GIF is ALWAYS intrinsically lossy (fps-down + scale-down + 256-colour quantize + audio-drop), so the §2.9 `video_to_gif` passive note shows UNCONDITIONALLY (once, calmly, not per-conversion) — G32 must assert it fires for every to-GIF pair; HDR/wide-gamut tone-mapped down by the decode→scale→palettegen chain (flattened but valid); 4K caught by the 480px width default + the guardrail; corrupt source → fails clearly, no partial GIF.

---

## P6.x — Cross-category corpus, tests, re-run detection & batch

- [ ] **P6.61** [TEST] Stage the to-GIF bijection corpus coverage (every `["<SOURCE>","GIF"]` pair) + extract-audio covers · §6.4.5 §6.4.3a · G24a G22
  needs: P6.47
  > extend each video corpus item's `covers` list to include its `["<SOURCE>","GIF"]` 2-tuple (MP4 item → `["MP4","GIF"]`, WEBM item → `["WEBM","GIF"]`, … — not one generic clip) AND its extract-audio `(video → MP3/WAV/FLAC/…)` 2-tuples, so the §6.4.3a bijection guard does not fail at Lane A for most cross-category pairs; regenerate the SHA-256 manifest in the same commit (G24a).
- [ ] **P6.62** [TEST] Add the cross-category per-pair integration tests (extract-audio FLOOR + to-GIF, structural readers) · §6.4.3 §6.5 · G31 G32
  needs: P6.61, P6.55, P6.60, P6.54, P4.58
  > for every `(video → audio-FLOOR-subset {MP3/WAV/FLAC})` extract-audio pair and every `(video → GIF)` pair, against the corpus, on all three platforms: completes + output decodes (`ffprobe` for extracted audio with expected codec; GIF89a valid + nonzero frames for to-GIF); the stream-copy path verified lossless where codecs match; the NoAudioTrack fixture fails-clearly; the to-GIF note fires unconditionally; the guardrail fail-fast triggers on the over-cap fixture; M4A patent-gapped target asserted absent where §3.4 unavailable. **The corpus-validated M4A/OGG extract-audio additions are tested by P6.69** (the `[!]` box unlocked by P9.44) — NOT a `needs:` of this floor box, so this box does not deadlock waiting on the P9.44 corpus evidence (P9.44 itself `needs: P6.66` which `needs:` this box, so P6.69 must not be a prerequisite of the gate that ultimately unlocks it).
- [ ] **P6.63** [TEST] Wire the cross-category re-run/equivalent-output detection (source + target + effective settings) · §2.5 · G31
  needs: P6.51, P6.56
  > §2.5 keys on source + target + EFFECTIVE settings: "extract audio → MP3 (Standard)" re-run on the same video triggers the skip/fresh-copy prompt; changing fps or MP3 quality is a NEW conversion (ordinary numbering); output naming/no-clobber/atomic-write/beside-source-divert apply identically (an extracted `clip.mp3` / `clip.gif` keeps the source base name + new extension, no-clobber numbered).
- [ ] **P6.64** [TEST] Wire the cross-category batch interaction (one chosen target over the same-source batch) · §1.3 · G31
  needs: P6.51, P6.56
  > the SSOT batch rule for this file: cross-category outputs are TARGETS of one video source, never a second source format; the batch key is the video source type only (§1.3); choosing "Extract audio" or "To GIF" applies that one target to the whole same-source batch (48 `.mov` → 48 MP3s, or 48 GIFs); per-file target is out of v1.

---

## P6.x — macOS TCC staging, phase reliability gate & advanced-options completeness

- [ ] **P6.65** [RUST] Verify FFmpeg receives the macOS kind-2 scratch-staged source path (never the raw protected path) · §3.5.0 §7.2.6 §2.14.2 · G31
  needs: P6.8, P4.25
  > assert (macOS only) that the **P4-built TCC source-staging (P4.24/P4.25)** stages the dropped source into per-job kind-2 scratch BEFORE spawning FFmpeg and hands FFmpeg the SCRATCH path as `<input>` — so a spawned engine is never the first process to touch a TCC-protected Desktop/Documents/Downloads/removable path (composes with the §2.14 cross-volume strategy; the macOS staged-input term in the §1.10 preflight). Read-side only (the write-side `.part` is the core's per §7.2.6). (`needs: P4.25` — the P4 staged-path engine-arg plumbing, per the P6.78 reconciliation obligation.)
- [ ] **P6.66** [TEST] Assert the §6.5 phase reliability gate — every P6 pair `reliable` on all three platforms · §6.5 §6.5.2 · G31 G32
  needs: P6.35, P6.49, P6.62, P6.65, P4.59, P4.60
  > the phase-level §6.5 coverage gate: the **P4-built §6.5.2 pair-status ledger generator (P4.60) + the §6.4.3a corpus↔pair bijection guard (P4.59)** mark EVERY enumerated P6 pair (audio + video container + extract-audio + to-GIF) `reliable` on every platform where it is not `unavailable-per-§3.4` or `demoted`; any `failing` cell blocks; record the two permissible exceptions (patent per-platform gap; last-resort demotion) in `docs/demoted-pairs.md` + the ledger with the required fields. The report is published as a release asset. (`needs: P4.59/P4.60` — the P4 ledger generator + bijection guard, per the P6.78 reconciliation obligation.)
- [ ] **P6.67** [TEST] Assert the FFmpeg-family advanced-option completeness (every declared option resolves + every pair has a test) · §1.6 §6.4 · G22 G23
  needs: P6.18, P6.19, P6.20, P6.21, P6.22, P6.23, P6.24, P6.25, P6.40, P6.55, P6.57, P6.58
  > the §6.4 completeness wiring for this engine: G22 every FFmpeg-family format ∈ the README support matrix ∧ has a corpus fixture ∧ has a round-trip test; G23 every `convert_*`/engine command for an FFmpeg pair has a partner test; every registered §1.6 advanced-option declaration resolves to a non-empty handler + a UI control on the P4 panel (no orphan declaration, no declared-but-unwired option).
- [ ] **P6.68** [TEST] Add the FFmpeg engine-bump re-validation hook (full reliability gate re-runs on a pin change) · §6.5.4 §3.8 · G37 G17b
  needs: P6.2, P6.66
  > wire the §6.5.4 rule for the FFmpeg `engines.lock` pin: a version/SHA change re-runs the FULL P6 reliability gate before that FFmpeg version can ship (a patch must not silently regress a pair); the ledger status-diff is part of the bump review; the informational per-push OSV/grype over the PURL-keyed FFmpeg row (CPE `cpe:2.3:a:ffmpeg:ffmpeg:<ver>`) feeds vuln-response (CVSS ≥ 7 on an exercised path → release-blocking escalation).

### Corpus-validated extract-audio additions (split from the guaranteed floor)

- [!] **P6.69** [RUST] Wire the corpus-validated extract-audio M4A/OGG additions (on the P6.52 floor) · §3.4 · G31
  unlocked-by: P9.44
  > blocked: the [OPEN-A] **corpus-validated additions** {M4A, OGG} registered on top of the P6.52 guaranteed floor are genuinely **un-buildable until P9.44 lands its corpus evidence** (M4A pending §3.4 AAC confirmation; OGG pending the §6.6 OGG-keep round-trip) — P9.44 is an activation/unlock (it "demotes only these two targets" on a pessimistic outcome), NOT a staged input to a step this box can run now, so this is a `[!]` + `unlocked-by:` block, not a `[ ]` + `needs:` (the _format.md §5.2 worked-example test: there is nothing to build at P6.69 until P9.44's corpus run decides keep-vs-demote — exactly the P5.4-style case). On the auto-unlock scan P9.44→`[x]` flips this to `[ ]` with the effective `needs: P6.52, P6.54` (the floor + the §3.4 M4A target-level gate it then registers on); a P9.44 demote outcome drops the affected target + adds a `docs/demoted-pairs.md` row (the P9.44 wiring-consequence this box owns). Reuse the audio.md encode params verbatim. (P9.44 in turn `needs: P6.66`, the P6 FFmpeg-reliability gate, so the corpus run validates against real staged infrastructure — see P9.44.)

---

## P6.x — Audio advanced-option declarations (registered against the P4 options-panel shell)

> The panel **chrome** was built in P4 (P4.63 widget dispatch + P4.73 AdvancedDrawer);
> these boxes register only per-target audio option **DECLARATIONS** (§1.6), elevated to
> top-level boxes (matching P5's RUST-savers-vs-UI-declarations separation, P5.20–P5.25
> vs P5.37–P5.44) so a [RUST] encoder box is never blocked on its [UI] declaration — each
> declaration `needs:` its encoder box (the source-of-the-knob) and renders against the
> P4 panel (the `needs: P4.63/P4.73` edge is carried centrally by the P6.78 reconciliation
> box, mirroring P5.70). No new panel chrome.

- [ ] **P6.70** [UI] Register the "MP3 quality" advanced-option DECLARATION against the P4 panel · §1.6 §2.9 · G47
  needs: P6.18.1
  > register the §1.6 option declaration (no new panel chrome — P4 owns it); Standard [default]; the V0/V2/V5 + CBR 128/192/320 preset set mapped by P6.18.1.
- [ ] **P6.71** [UI] Register the "WAV bit depth" advanced-option DECLARATION (16 / 24 / 32-float) · §1.6 · G47
  needs: P6.19
  > `16-bit [default]` · `24-bit (pcm_s24le)` · `32-bit float (pcm_f32le)`.
- [ ] **P6.72** [UI] Register the "FLAC compression" advanced-option DECLARATION (Fast 0 / Standard 5 / Best 8) · §1.6 · G47
  needs: P6.20
  > exposed cap is 8 (libFLAC native max; FFmpeg 9–12 are non-standard — do NOT surface); validation range `0..=8`.
- [ ] **P6.73** [UI] Register the "AAC quality" advanced-option DECLARATION (128/192/256 CBR) · §1.6 · G47
  needs: P6.21
  > no VBR exposed (encoder limitation); 192k [default].
- [ ] **P6.74** [UI] Register the M4A quality advanced-option DECLARATION (shares the AAC preset set) · §1.6 · G47
  needs: P6.22
  > 128/192/256 CBR; container difference (`.m4a` iTunes atoms) chosen automatically, not a user setting.
- [ ] **P6.75** [UI] Register the "OGG quality" advanced-option DECLARATION (q3/q5/q7) · §1.6 · G47
  needs: P6.23
  > Vorbis quality scale −1..10; expose the useful middle; q3 [default].
- [ ] **P6.76** [UI] Register the "OPUS bitrate" advanced-option DECLARATION (96/128/192) · §1.6 · G47
  needs: P6.24
  > 128k [default]; `-vbr on` retained throughout.
- [ ] **P6.77** [UI] Register the "AIFF bit depth" advanced-option DECLARATION (16 / 24) · §1.6 · G47
  needs: P6.25
  > `16-bit [default]` · `24-bit (pcm_s24be)`.

---

## Cross-phase reconciliation (the deferred P6→P4 harness `needs:`)

- [ ] **P6.78** [GATE] Wire the deferred P6→P4 harness `needs:` edges — isolation boundary, §1.7 lifecycle/probe/progress/kill, per-pair runner + ledger + bijection, TCC staging, options-panel shell · §3.5.1 · G7 G20
  needs: P4.13, P4.14, P4.8, P4.9, P4.10, P4.11, P4.25, P4.42, P4.48, P4.58, P4.59, P4.60, P4.63, P4.73
  > the P6 instance of the cross-phase reconciliation obligation (the master plan-lint forbidden-string check is P4.76; reciprocal of P3.70/P5.70/P9.46): declare the load-bearing P6→P4 edges the FFmpeg-family boxes consume — FFmpeg routes through the **P4.13/P4.14 §2.12 isolation wrapper** (P6.8); probe-then-encode + progress + classify + cancel/kill ride the **P4.8/P4.9/P4.10/P4.11/P4.48 §1.7 lifecycle** (P6.10–P6.13); macOS TCC staging is **P4.25** (P6.65); the per-engine availability row populates the **P4.42 verifier framework** (P6.14); every per-pair test runs on the **P4.58 §6.4.3 runner** (P6.32/P6.49/P6.62) and the phase gate drives the **P4.59 bijection guard + P4.60 ledger generator** (P6.66); every advanced-option DECLARATION box (P6.18.1's UI P6.70–P6.77, the video P6.40.1, the extract-audio P6.55.1, the to-GIF P6.57.1/P6.57.2/P6.58.1) renders against the **P4.63 OptionsPanel widget dispatch + the P4.73 AdvancedDrawer**. `needs:` these P4 harness boxes here so the §6 selection builds the P4 mechanism first (P4 is `[x]` before the loop reaches P6 — the edges must RESOLVE, not dangle; the inline engine edges on P6.8–P6.66 carry the per-box dependency, this box is the auditable single owner). No P6 box `>`-note defers a `needs:` with the P4.76-forbidden phrasing.
