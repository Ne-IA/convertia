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
> intra-phase progress. The sub-gate box (P6.45 below) sits between the audio
> cluster and the video cluster and `needs:` every audio pair box.
>
> **This is the v0 BASE** — the smallest-atomic `[ ]` boxes are below, grouped under
> `### ` sub-headings; a later adversarial review will deepen, split and complete
> them.

---

### FFmpeg engine staging, hardening & runtime wiring

> The FFmpeg/FFprobe sidecar must exist, be hardened (curated build + argv
> controls), checksum-verified, SBOM-rowed and runtime-wired through the §2.12
> boundary before any pair can be built. These boxes execute the per-engine variants
> of the P0.7-policy / P4-framework gates for FFmpeg specifically.

- [ ] **P6.1** [BUILD] Stage the FFmpeg + FFprobe sidecar per-OS (cache-keyed, target-triple-suffixed) · §6.1.3 §3.3 · G37
  needs: P4.27, P0.7.3
  > `scripts/stage-engines` restores the `actions/cache`-hosted `ffmpeg-<ver>-<triple>` engine-asset cache (checksum-verified pinned-URL fetch on a miss; the from-source curated-build populate path is P6.1.1), places the FFmpeg + FFprobe sidecars under `src-tauri/binaries/` target-triple-suffixed (`ffmpeg-x86_64-pc-windows-msvc.exe`, …), and declares them in `tauri.conf.json` `bundle.externalBin`. The single GPL-2.0+ binary serves audio/video/cross-category. → executes the P0.7.3/P0.7.4 acquisition+staging policy for FFmpeg (`needs: P0.7.3` for the from-source acquisition + engine-source-allow-list policy this anchors against; the cross-phase edge carried via the P6.92 reconciliation box).
  - [ ] **P6.1.1** [BUILD] Compile FFmpeg from source as the curated `--enable-gpl --disable-network --disable-everything --enable-…` network-absent build via the P4.28.1 harness (fills the FFmpeg configure-flag manifest seam) · §6.1.3 §3.5.1 §3.6.1 · G37 G38
    needs: P4.28.1
    > the from-source curated FFmpeg build the **P6.3/P6.4/P6.5/P6.6/P6.7 §6.1.3 assertions can only pass against** — the gyan.dev/BtbN Windows prebuilts §3 (03-engines:1588) names ship the network protocol family (`http`/`https`/`tcp`/`tls`/`rtmp`/`rtsp`/`hls`) **ENABLED**, so the prebuilt branch provably CANNOT satisfy P6.6's `--disable-network` LOAD-BEARING T9b SSRF floor; the network-absent + curated-decoder/encoder + dereferencing-demuxer-absent properties are ones **only a build ConvertIA configures** has (§3.5.1 "the LOAD-BEARING SSRF floor is the curated build, NOT the argv whitelist"). Compile FFmpeg through the **P4.28.1 from-source compilation harness** with `--enable-gpl` (x264), HARD-rejecting `--enable-nonfree`/`--enable-libfdk-aac`, `--disable-network` (no `--enable-protocol=` network family), `--disable-everything --enable-…` trimmed to the generated §04 decoder/encoder set (P6.4/P6.5), the dereferencing demuxers (DASH/HLS-fetch/external-reference playlist) NOT enabled (P6.7) — filling the P4.28.1 per-engine `ffmpeg.configure.flags` manifest seam so the configure line is the data P6.3/P6.6/P6.7 cross-check; populate the `ffmpeg-<ver>-<triple>` cache key P6.1 staging reads. (§3.5.1:1597 offers FFmpeg as "from-source CI build OR prebuilt cross-checked", but only the from-source branch satisfies `--disable-network`; the prebuilt branch is corroboration-only for an already-network-absent build, never the curated build itself.) (`needs: P4.28.1` for the from-source harness this fills the seam of; the cross-phase edge carried via the P6.92 reconciliation box.)
  - [ ] **P6.1.2** [BUILD] Apply the FFmpeg beside-the-exe dynamic-library RELOCATION (rpath / install_name rewrite over `libmp3lame`/`libvorbis`/`libogg`/`libopus`/`libvpx`) + assert the post-relocation G37b closure · §3.9.1 §3.6.1 §6.1.3 · G37 G37b
    needs: P6.1, P4.30
    > the **primary relocation case** the P4.30 generic mechanism exists for: §3.1 row 2a / §3.9.1 ship FFmpeg's five component libs (`libmp3lame` LGPL, `libvorbis`/`libogg`/`libopus`/`libvpx` BSD-3) as **separate shared objects staged BESIDE the FFmpeg exe** (the v1 dynamic-beside-the-exe preference, §3.7.2). As downloaded/compiled they carry ABSOLUTE `install_name`/`RUNPATH` entries, so inside the portable bundle — where §3.5 strips `DYLD_*`/`LD_LIBRARY_PATH` and §3.3.3 does not rely on `PATH` — they would NOT resolve. Drive the **P4.30 per-OS rewrite** over the FFmpeg set: **macOS** `install_name_tool -id @loader_path/<lib>` on each component lib + `install_name_tool -change <abs> @loader_path/<lib>` on the `ffmpeg`/`ffprobe` Mach-O (after the P4.29 `lipo`); **Linux** `patchelf --set-rpath '$ORIGIN'` on `ffmpeg`/`ffprobe` so `DT_RUNPATH` finds the beside-the-exe `.so`s; **Windows** no-op (the DLLs resolve from the exe dir). Then **assert the G37b dynamic-dependency-closure on the RELOCATED binaries** (`otool -L`/`otool -l` macOS · `ldd`/`readelf -d` Linux · `dumpbin /dependents` Windows): every non-system FFmpeg dependency resolves to a `@loader_path`/`$ORIGIN`/exe-dir path **inside the bundle**, the build FAILS on any absolute/out-of-bundle reference — so the relocation is proven, not assumed. **If FFmpeg is instead built statically (§6.1.3 carve-out iii)** the component libs are subsumed and this leg is a recorded no-op (the static GPL binary needs no beside-the-exe relocation); v1's stated preference is dynamic-beside-the-exe (§3.9.1), so this leg is the v1 path. (`needs: P6.1` for the staged FFmpeg set this rewrites + `P4.30` for the generic relocation mechanism it drives; the cross-phase edge carried via the P6.92 reconciliation box.)
- [ ] **P6.2** [BUILD] Anchor the FFmpeg engine acquisition + add its `engines.lock` row + SBOM rows · §3.7.2 §3.8 · G37 G35 G36
  needs: P6.1
  > the FFmpeg acquisition-anchor + `engines.lock` row + SBOM-row population, decomposed into two independently-failing sub-boxes (the acquisition/lock-row is a download+hash+anchor concern that fails differently from a malformed CycloneDX SBOM row — mirroring the P5 pattern where staging P5.1 and SBOM population P5.67 fail independently; _format.md §3.2, dual review once over the combined diff). → executes the P0.7.1/P0.7.3 policy for FFmpeg.
  - [ ] **P6.2.1** [BUILD] Anchor the FFmpeg acquisition + add its `engines.lock` row (purl + SHA-256 + CPE, P0.7.3 named-anchor corroboration) · §3.8 §3.7.2 · G37
    needs: P6.1
    > add the FFmpeg `engines.lock` row (`purl` `pkg:generic/ffmpeg@<ver>` + SHA-256 + the FFmpeg CPE `cpe:2.3:a:ffmpeg:ffmpeg:<ver>`), corroborate the unsigned gyan/BtbN prebuilt per the P0.7.3 named-anchor (≥2 mirrors / distro-signed) or the from-source signed-tarball path (P6.1.1). The download/hash/anchor concern — fails on a hash mismatch or an un-corroboratable source, distinct from a malformed SBOM row.
  - [ ] **P6.2.2** [BUILD] Populate the FFmpeg CycloneDX SBOM rows (FFmpeg + nested component libs, SPDX + source URLs) · §3.7.2 §3.6.2 · G35 G36
    needs: P6.2.1
    > populate the CycloneDX SBOM rows for FFmpeg + its nested component libs (`libmp3lame` LGPL, `libvorbis`/`libogg`/`libopus`/`libvpx` BSD, `libx264` GPL) with SPDX + source URLs — the SBOM-population concern that fails on a malformed/incomplete row, distinct from the acquisition anchor (mirrors P5.67's separation from P5.1 staging). → executes the P0.7.1 SBOM policy for FFmpeg.
- [ ] **P6.3** [BUILD] Assert the FFmpeg GPL license posture — `--enable-gpl`, hard-fail `--enable-nonfree`/`libfdk_aac` · §3.6.1 §6.1.3 · G38 G36
  needs: P6.1, P0.7.4
  > the §6.1.3 configure-flag / whole-binary-license assertion: assert the staged FFmpeg was built `--enable-gpl` (x264 ⇒ whole binary GPL-2.0+) and HARD-FAIL on `--enable-nonfree`/`--enable-libfdk-aac` (nonfree taint = non-redistributable, distinct from the x264 GPL relicense); record the configuration line. The native built-in `aac` encoder is the license-clean choice. → executes the P0.7.4 per-engine build-assertion policy for FFmpeg (`needs: P0.7.4`, the assertion-policy home, `[x]` before the loop; the cross-phase edge carried via the P6.92 reconciliation box).
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
  > register FFmpeg in the §3.2 `Engine` registry; route every invocation through the **P4-built §2.12 isolation wrapper (P4.13 cheap-tier floor + P4.14 loader-var strip)** with cwd = per-run scratch (§2.14), minimal isolated env (`LC_ALL=C.UTF-8`, no proxy vars, `PATH` not relied on — absolute bundled path), and the explicit dynamic-loader-injection strip (`LD_PRELOAD`/`LD_LIBRARY_PATH`/`DYLD_INSERT_LIBRARIES`/`DYLD_LIBRARY_PATH` cleared, G29 `.env_clear()` invariant). Untrusted A/V parsed in FFmpeg demuxers/decoders = classic attack surface. (`needs: P4.13/P4.14` — the P4 isolation wrapper this engine routes through, per the P6.92 reconciliation obligation.)
- [ ] **P6.9** [RUST] Apply the always-on global FFmpeg flags + the argv SSRF/LFR defence-in-depth · §3.5.1 · G38 G42 G42b
  needs: P6.8
  > prepend the global flags to every FFmpeg job: `-nostdin -hide_banner -loglevel error -y` (`-y` safe — target is the §2.1 temp path, `-nostdin` prevents the parent-stdin-consume hang); the per-input `-protocol_whitelist file,pipe` (defence-in-depth on the network-absent build, set before each input); and NEVER `-safe 0` on the concat demuxer (`-safe 1` default rejects absolute/`..` paths). These are the runtime half of the §3.5.1 control.
- [ ] **P6.10** [RUST] Wire FFprobe stream inspection + the `ProbeOutput` → `plan_encode` denominator path · §3.5.1 §1.7 §1.11 · G31
  needs: P6.8, P4.9
  > `ffprobe -v error -print_format json -show_streams -show_format <input>` (probe-first for video) parsed into `ProbeOutput` (inner codecs / duration / rotation / interlace); the duration becomes the §1.11 progress denominator carried into `Engine::plan_encode(.., &probe)` (§3.2.1/§3.2.2) which builds the encode `Invocation` with `ProgressModel::FfmpegKeyValue { duration_us }` already populated — NO placeholder-then-mutate. (`needs: P4.9` — the P4-built two-step probe-then-encode sequencing this populates, per the P6.92 reconciliation obligation.)
- [ ] **P6.11** [RUST] Wire the FFmpeg `-progress pipe:1` parser into the §1.11 real per-item progress bar · §3.5.1 §1.11 · G31
  needs: P6.10, P4.8
  > `-progress pipe:1 -nostats` → key=value lines (`out_time_us=`/`total_size=`/`progress=continue|end`) parsed into `ProgressModel::FfmpegKeyValue`; until the first `out_time_us` tick the bar reads `Spawning`/indeterminate-but-working, then a true % against the probe duration — never a spinner (the SSOT *How It Feels* 6 promise for long video re-encodes). (`needs: P4.8` — the P4-built per-`ProgressModel` stdout line-reader dispatch this feeds, per the P6.92 reconciliation obligation.)
- [ ] **P6.12** [RUST] Wire the FFmpeg exit/stderr → §2.8 error-kind mapping (`classify_failure`) · §3.5.1 §2.8 · G31
  needs: P6.8, P4.49
  > map known stderr patterns: "could not find codec parameters"/"Invalid data" → corrupt; the cross-category "No audio" path → the named `NoAudioTrack` kind; DRM/"Operation not permitted" on FairPlay/WMV → the §video.md "copy-protected" message; everything else → generic engine-failure (still plain-language §2.13). A crashing/hanging decode fails THAT one item and the batch continues. (`needs: P4.49` — the P4-built capture-and-classify-into-§2.8 generic seam this fills the FFmpeg classifier for, per the P6.92 reconciliation obligation.)
- [ ] **P6.13** [RUST] Wire cancellation via process-group kill + the no-partial-output guarantee for FFmpeg · §3.5.1 §1.7 §2.1
  needs: P6.8, P4.10, P4.11
  > cancellation routes through the **P4-built §1.7 process-group kill (P4.10) + the kill↔cleanup↔no-partial ordering (P4.11)** (FFmpeg may spawn children); a cancelled re-encode leaves NO partial output (FFmpeg writes to the §2.1 temp `out_tmp`, atomic-renamed only on success); already-finished batch items are kept. (`needs: P4.10/P4.11` — the P4 cancel/kill mechanism, per the P6.92 reconciliation obligation.)
- [ ] **P6.14** [TEST] Add the per-engine FFmpeg §7.2.3 availability/integrity row + the in-bundle hash-manifest entry · §7.2.3 · G46 G37
  needs: P6.2, P4.43
  > populate the FFmpeg + FFprobe rows in the build-time in-bundle hash manifest and the `EngineHealth` availability table (the per-engine variant of the **P4-built §7.2.3 startup-verifier framework, P4.43**) so a missing/corrupt FFmpeg escalates to a §2.13 app-fault, not a crash, and feeds C12 `get_engine_health` (§5.2 disables unavailable targets). (`needs: P4.43` — the P4 integrity-verifier framework this populates a row in, per the P6.92 reconciliation obligation.)

---

### Audio pairs (FFmpeg, audio-file → audio-file)

> Every audio source→target pair is one FFmpeg invocation (decode → re-encode);
> §3.2 single-engine-per-pair holds trivially, no chaining. Defaults + advanced
> presets + lossy flags + tag policy are owned by [audio.md](../spec/04-formats/audio.md);
> these boxes wire them. WMA is **source-only** (`→ WMA` parked out of v1). The
> diagonal (same→same) is `—` for v1. Each per-target encode box `needs:` the
> shared FFmpeg runtime wiring (P6.9/P6.10/P6.12).

- [ ] **P6.15** [RUST] Wire the audio-source detection signatures + codec-in-container disambiguation · §1.2 · G15 G31
  needs: P6.10
  > add the §1.2 per-format audio signatures: MP3 frame-sync `FF Fx`/ID3 (layer-disambiguated from MP1/MP2); WAV RIFF/WAVE + `fmt ` chunk; FLAC `fLaC`; raw-ADTS AAC `FF F1`/`FF F9` (ADTS-vs-MP3 by header parse); M4A `ftyp` reading codec from `stsd`/`esds` (AAC ⇒ "M4A", ALAC ⇒ "ALAC"); OGG `OggS`+`\x01vorbis` vs OPUS `OggS`+`OpusHead`; AIFF `FORM`/`AIFF`/`AIFC`; ALAC = M4A `ftyp` + `alac` in `stsd`; WMA ASF GUID + stream-properties codec id. Codec id is authoritative over extension (a `.mp3` that is really FLAC, a `.m4a` that is really ALAC).
- [ ] **P6.16** [RUST] Wire the audio `-map_metadata 0` universal tag-carry flag + the channel/sample-rate preservation cross-ref · §3.5.1 §1.2 §2.10 · G31
  needs: P6.15
  > `-map_metadata 0` — the single universal FFmpeg flag for tag carry (title/artist/album/year/genre/track/comment + CJK/RTL UTF-8 preserved, §2.10), applicable to EVERY audio target; no silent resample/bit-depth change in the default path (channel/sample-rate preservation owned by P6.17). The per-container cover-art mechanism (a distinct concern with per-container conditional codec-copy logic that fails independently of the metadata flag) is the sub-box P6.16.1 below.
  > **Pessimistic-outcome wiring-consequence (reciprocal of P9.43):** if the P9.43 OGG/OPUS embedded-picture round-trip corpus spike FAILS, the OGG/OPUS PICTURE-block cover-art path (P6.16.1) is dropped and those targets move to the tag-poor list (the §2.9 `audio_tags_dropped` trigger then fires, P6.29) — so the P9.43 backward amendment to this tag/cover-art wiring is auditable from this end too.
  - [ ] **P6.16.1** [RUST] Wire the per-container cover-art mechanism (MP3/M4A/FLAC copy / OGG-OPUS PICTURE-block / raw-ADTS-WAV-AIFF drop) · §3.5.1 · G31
    > the cover-art mechanism BY CONTAINER (split from the universal `-map_metadata 0` flag — three distinct codec-copy paths, not one flag): MP3/M4A/FLAC = attached-picture video stream (`-map 0:v? -c:v copy`, `?` keeps audio-without-art working); OGG/OPUS = FLAC PICTURE metadata block via metadata copy (NOT `-c:v copy`); raw ADTS `.aac` + WAV/AIFF omit it (fire `audio_tags_dropped`). A broken cover-art codec-copy path does not affect the P6.16 metadata flag and vice versa.
- [ ] **P6.17** [RUST] Wire the channel-preservation policy (no proactive downmix; forced-only `audio_downmix`) · §3.5.1 · G31
  needs: P6.15
  > **preserve the source channel layout for EVERY target — MP3 and OGG included** (audio.md "Sample rate, channels, bit depth — preservation policy": "There is **no down-mix to stereo** in v1 even for surround sources into stereo lossy targets unless the encoder strictly requires it"; Format-default item 4: `audio_downmix` fires "only if the codec forces a downmix"; the WMA edge note: MP3 "down-mixes per the encoder only if needed"). libmp3lame and libvorbis both carry >2 channels, so do **NOT** add an unconditional `-ac 2` for MP3/OGG — that proactive stereo downmix is exactly the silent channel-loss the spec forbids (and it would contradict P6.29's `audio_downmix` = "forced codec downmix" definition). The §2.9 `audio_downmix` note fires **ONLY** where the chosen encoder/muxer strictly forces a downmix (a codec-forced-only kind, not a per-target proactive `-ac 2`). No silent resample/bit-depth change in the default path. (Conflict order: the audio.md format spec is authoritative over the §3 engine-prose `-ac 2` line — escalate the §3.5.1 engine-prose `>2-channel → MP3/OGG -ac 2 downmix` sentence to be reconciled to the audio.md preserve-all rule, since the two spec files disagree.)
- [ ] **P6.18** [RUST] Wire the MP3 target — `libmp3lame -q:a 2` VBR default · §3.5.1 · G31
  needs: P6.9, P6.16
  > MP3 encode `-c:a libmp3lame -q:a 2` (VBR ≈190k default); always-lossy-as-target. MP3 is the default target of every audio source except MP3 itself. (audio.md MP3 entry.)
  - [ ] **P6.18.1** [RUST] Wire the MP3 quality advanced-option mapping (V0/V2/V5 + CBR 128/192/320) · §1.6 · G31
    > the "MP3 quality" preset set → `-q:a N` (VBR High V0 / Standard V2 / Small V5) or `-b:a Nk` (CBR 128/192/320); this same canonical MP3 preset table is reused verbatim by cross-category extract-audio→MP3 (OPEN-B). The "MP3 quality" UI option DECLARATION is the top-level box P6.84 (registered against the P4 panel).
- [ ] **P6.19** [RUST] Wire the WAV target — `pcm_s16le` 16-bit default + bit-depth advanced option · §3.5.1 · G31
  needs: P6.9, P6.16
  > WAV encode `-c:a pcm_s16le` via `wav` muxer, 16-bit default, sample-rate/channels preserved; lossless-as-target except the >16-bit-source → default-16-bit bit-depth-reduction case (fire `audio_bitdepth`). Weak WAV tag model → fire `audio_tags_dropped` only when the source carried tags. The "WAV bit depth" UI option DECLARATION is the top-level box P6.85.
- [ ] **P6.20** [RUST] Wire the FLAC target — `flac -compression_level 5` default + level advanced option · §3.5.1 · G31
  needs: P6.9, P6.16
  > FLAC encode `-c:a flac -compression_level 5` (level changes size/speed only, never the audio); lossless-as-target; lossy-ORIGIN flagged `audio_lossy_origin` (no quality gain). Vorbis comments + PICTURE block round-trip. The "FLAC compression" UI option DECLARATION is the top-level box P6.86.
- [ ] **P6.21** [RUST] Wire the AAC target — native `aac -b:a 192k` CBR + adts muxer, reading §3.4 availability · §3.5.1 §3.4 · G31
  needs: P6.9, P6.16
  > AAC encode native `-c:a aac -b:a 192k` CBR (native-encoder VBR is unstable) + muxer `adts` (raw `.aac`); always-lossy; raw ADTS has NO tag container → cover art + tags DROPPED (`audio_tags_dropped`). READS the §3.4 AAC per-platform cell (P4 matrix) — if AAC is unavailable on a platform, the AAC target is honestly disabled there (never re-decided here). The "AAC quality" UI option DECLARATION is the top-level box P6.87.
- [ ] **P6.22** [RUST] Wire the M4A target — native `aac` + `ipod` muxer + faststart, reading §3.4 (inherits AAC) · §3.5.1 §3.4 · G31
  needs: P6.21
  > M4A encode native `-c:a aac -b:a 192k` + muxer `ipod` (`.m4a`) + `-movflags +faststart`; identical quality knobs to AAC; KEEPS metadata (iTunes `ilst` atoms + cover art) — M4A's advantage over raw `.aac`. INHERITS AAC's §3.4 disposition (the codec is AAC); M4A-holding-ALAC is the separate ALAC target. The "M4A quality" UI option DECLARATION is the top-level box P6.88.
- [ ] **P6.23** [RUST] Wire the OGG (Vorbis) target — `libvorbis -q:a 3` VBR + ogg muxer · §3.5.1 · G31
  needs: P6.9, P6.16
  > OGG encode `-c:a libvorbis -q:a 3` (≈112k) muxer `ogg` — DISTINCT from OPUS (the OGG target is always Vorbis, never Opus); always-lossy; Vorbis comments + cover-art-as-PICTURE-block. No patent flag (royalty-free). The "OGG quality" UI option DECLARATION is the top-level box P6.89.
- [ ] **P6.24** [RUST] Wire the OPUS target — `libopus -b:a 128k` VBR + opus muxer (48 kHz internal) · §3.5.1 · G31
  needs: P6.9, P6.16
  > OPUS encode `-c:a libopus -b:a 128k` (`-vbr on`) muxer `opus`; FFmpeg resamples to Opus's 48 kHz internal rate transparently (not a user-visible loss); always-lossy; never the per-source DEFAULT (older players may not open `.opus`). No patent flag. The "OPUS bitrate" UI option DECLARATION is the top-level box P6.90.
- [ ] **P6.25** [RUST] Wire the AIFF target — `pcm_s16be` 16-bit big-endian + aiff muxer · §3.5.1 · G31
  needs: P6.9, P6.16
  > AIFF encode `-c:a pcm_s16be` muxer `aiff`, 16-bit big-endian default; lossless-as-target with the same >16-bit-source → 16-bit `audio_bitdepth` caveat as WAV; limited AIFF tag model → `audio_tags_dropped` when the source carried tags. The "AIFF bit depth" UI option DECLARATION is the top-level box P6.91.
- [ ] **P6.26** [RUST] Wire the ALAC target — native `alac` + `ipod` muxer + faststart (lossless, no knob) · §3.5.1 · G31
  needs: P6.9, P6.16
  > ALAC encode `-c:a alac` muxer `ipod` (`.m4a` whose codec is ALAC) + `+faststart`; lossless, NO quality/compression knob exposed (FFmpeg's ALAC encoder has none) — the advanced view stays clean; lossy-ORIGIN flagged `audio_lossy_origin`. NO patent flag (ALAC is open/royalty-free — never confuse with AAC's §3.4 status). Same `ilst` metadata + cover art as M4A.
- [ ] **P6.27** [RUST] Wire WMA as a DECODE-only source (no `→ WMA` target) + the source-options-from-target rule · §3.5.1 · G31
  needs: P6.15
  > WMA decoders `wmav1`/`wmav2`/`wmapro`/`wmalossless` all decode-capable; `→ WMA` is PARKED out of v1 (no target wiring — only `wmav2` exists and it is low-quality legacy); as a source its options are the chosen target's; WMA→lossless-target = lossy-origin, WMA→lossy-target = second lossy round; ASF metadata mapped to tag-supporting targets.
- [ ] **P6.28** [TEST] Wire the per-source-default-target table (MP3 default for all except MP3→WAV) · §1.5 §1.6 · G31 G61
  needs: P6.18, P6.19, P4.60.2
  > the pre-highlighted default = MP3 for every audio source EXCEPT MP3 itself (→ WAV, since MP3→MP3 is excluded); this box wires the per-CATEGORY default-target table for audio, and its §04-offered audio pairs + their `OptionDecl` defaults FEED the §1.6 consolidated defaults registry the **P4.60.2 G61 guard** (the single machine-checkable home of the SSOT "no required choices" gate) merges + checks across ALL options of ALL offered pairs — `needs: P4.60.2` so the per-category default table is registered against the guard, not asserted ad-hoc here. (DECIDED: MP3-source default is WAV over FLAC.)
- [ ] **P6.29** [TEST] Wire the audio lossy-disclosure trigger map (the `✓~` matrix cells ↔ §2.9 kinds) · §2.9 · G31 G32
  needs: P6.18, P6.19, P6.20, P6.21, P6.22, P6.23, P6.24, P6.25, P6.26
  > assert each §2.9 audio kind fires IFF the §04 matrix flags the pair: `audio_lossy_target` (any → MP3/AAC/M4A/OGG/OPUS), `audio_transcode` (lossy → lossy), `audio_lossy_origin` (lossy → FLAC/ALAC ONLY — deliberately NOT WAV/AIFF), `audio_bitdepth` (>16-bit → default 16-bit WAV/AIFF), `audio_tags_dropped` (→ raw AAC / WAV / AIFF when source had tags), `audio_downmix` (forced codec downmix). The G32 lossy-disclosure property holds over the `FormatId×FormatId` product.
  > **Pessimistic-outcome wiring-consequence (reciprocal of P9.43):** if the P9.43 OGG/OPUS embedded-picture round-trip corpus spike FAILS, OGG/OPUS move to the tag-poor list and `audio_tags_dropped` now fires for them — this trigger map is EDITED accordingly, so the P9.43 backward amendment is auditable from this end too.

---

### Audio corpus + per-pair audio tests

> The §6.4.5 audio corpus + the per-pair §6.4.3 integration tests that let each
> audio pair reach `reliable`. The corpus↔pair bijection guard (§6.4.3a, built in
> P4) fails Lane A if any audio pair has no backing file, so the corpus boxes
> precede the test boxes.

- [ ] **P6.30** [TEST] Stage the audio corpus (one file per source format) + its manifest + SHA-256 entries · §6.4.5 · G24a G22
  needs: P6.1, P0.5.11
  > add `tests/corpus/audio/` files: one per source format (MP3 VBR/CBR + ID3v2 + cover; WAV 16/24/float; FLAC + Vorbis comments + cover; raw-ADTS `.aac`; M4A-holding-AAC AND a separate M4A-holding-ALAC; OGG-Vorbis; `.opus`; AIFF; WMA v2/Pro/Lossless), each with a root-`manifest.toml` `[[file]]` (source / redistributable licence / `exercises` / `covers` 2-tuples / `[file.expect]`); regenerate the §6.4.5/P0.5.4 SHA-256 corpus manifest **via the `stage-corpus` generator (P0.5.11)** in the same commit (G24a). Files must be CC0/public-domain/self-produced/synthetic. (`needs: P0.5.11` for the manifest generator.)
- [ ] **P6.31** [TEST] Stage the audio edge-case + content-floor corpus fixtures · §6.4.5 · G24a G31
  needs: P6.30, P0.5.11
  > add the audio edge fixtures + content-floor tags: a multichannel (5.1) source (`audio_downmix` / channel-preservation); a >16-bit source (`audio_bitdepth`); files with non-Latin/CJK/RTL tag text (`non-latin-tags` content-floor tag, §2.10); corrupt/truncated + 0-byte + a `.mp3` that is really FLAC (mislabel) cases; cover-art round-trip fixtures for the MP3↔FLAC↔OGG↔OPUS↔M4A/ALAC set. These are NEW SHA-256-manifest-tracked fixtures, so regenerate the manifest **via the `stage-corpus` generator (P0.5.11)** in the same commit (G24a). (`needs: P0.5.11` for the manifest generator.)
> **Per-OUTPUT-FORMAT per-pair audio test split (one box per encode code-path).**
> The former monolithic "every audio pair" box is split into one box per audio
> output-format code-path (matching the per-saver RUST split P6.18–P6.27 and the
> per-target P5 test split P5.49–P5.61), so each code-path is independently
> dual-reviewed. Every box runs on the P4.59 §6.4.3 per-pair runner against every
> corpus file of each source format on all three platforms, with the MANDATORY
> structural reader (`ffprobe` decodes + reports the expected codec, stream count > 0
> — NOT magic re-detect), no-harm (source `sha256` unchanged, atomic write,
> no-clobber), fail-clearly on the known-bad fixtures, lossy-disclosure-iff-flagged
> (P6.29), and tag/cover-art/channel content-fidelity spot-checks. (`needs: P4.59`
> carried per the P6.92 reconciliation obligation.) WMA is source-only (no `→ WMA`).
- [ ] **P6.32** [TEST] Per-pair audio tests: → MP3 (`libmp3lame` VBR, all sources) · §6.4.3 §6.5 · G31 G32
  needs: P6.30, P6.31, P6.29, P4.59
  > each `* → MP3` pair (the default target of every audio source except MP3): completes, `ffprobe` reports `mp3`/`libmp3lame` codec + stream>0, source-unchanged, `audio_lossy_target`(+`audio_transcode` for lossy-source) fires iff §04-flagged, ID3 tag + cover-art content-fidelity spot-check, channel layout preserved (no proactive downmix per P6.17).
- [ ] **P6.33** [TEST] Per-pair audio tests: → WAV (`pcm_s16le` 16-bit, all sources) · §6.4.3 §6.5 · G31 G32
  needs: P6.30, P6.31, P6.29, P4.59
  > each `* → WAV` pair: completes, `ffprobe` reports `pcm_s16le` + stream>0, source-unchanged, lossless-as-target except the >16-bit-source → `audio_bitdepth` case, `audio_tags_dropped` only when the source carried tags, channels preserved.
- [ ] **P6.34** [TEST] Per-pair audio tests: → FLAC (level-only, all sources) · §6.4.3 §6.5 · G31 G32
  needs: P6.30, P6.31, P6.29, P4.59
  > each `* → FLAC` pair: completes, `ffprobe` reports `flac` + stream>0, source-unchanged, lossless (no quality change at any level), `audio_lossy_origin` fires for a lossy source (no quality gain), Vorbis-comment + PICTURE round-trip, channels (up to 8) preserved.
- [ ] **P6.35** [TEST] Per-pair audio tests: → AAC (native `aac` + adts muxer, reads §3.4) · §6.4.3 §6.5 · G31 G32
  needs: P6.30, P6.31, P6.29, P4.59
  > each `* → AAC` pair on platforms where §3.4 marks AAC **available**: completes, `ffprobe` reports `aac` (ADTS) + stream>0, source-unchanged, `audio_lossy_target` fires, raw-ADTS → `audio_tags_dropped` (no tag container); on a platform where §3.4 marks AAC **unavailable** the target is asserted absent/disabled (not attempted) — honest unavailability per §3.4.
- [ ] **P6.36** [TEST] Per-pair audio tests: → M4A (native `aac` + ipod muxer + faststart, reads §3.4) · §6.4.3 §6.5 · G31 G32
  needs: P6.30, P6.31, P6.29, P4.59
  > each `* → M4A` pair on AAC-available platforms: completes, `ffprobe` reports `aac` in an MP4/ipod container + faststart (moov front-loaded) + stream>0, source-unchanged, `audio_lossy_target` fires, iTunes `ilst` tag + cover-art content-fidelity (M4A's advantage over raw AAC); §3.4-unavailable → target absent/disabled.
- [ ] **P6.37** [TEST] Per-pair audio tests: → OGG (Vorbis, all sources) · §6.4.3 §6.5 · G31 G32
  needs: P6.30, P6.31, P6.29, P4.59
  > each `* → OGG` pair: completes, `ffprobe` reports `vorbis` (NEVER opus) in an ogg container + stream>0, source-unchanged, `audio_lossy_target` fires, Vorbis-comment + cover-art-as-PICTURE round-trip, channels preserved.
- [ ] **P6.38** [TEST] Per-pair audio tests: → OPUS (`libopus` 48 kHz, all sources) · §6.4.3 §6.5 · G31 G32
  needs: P6.30, P6.31, P6.29, P4.59
  > each `* → OPUS` pair: completes, `ffprobe` reports `opus` (48 kHz internal — not a user-visible loss) + stream>0, source-unchanged, `audio_lossy_target` fires, never the per-source default; channels preserved.
- [ ] **P6.39** [TEST] Per-pair audio tests: → AIFF (`pcm_s16be` big-endian, all sources) · §6.4.3 §6.5 · G31 G32
  needs: P6.30, P6.31, P6.29, P4.59
  > each `* → AIFF` pair: completes, `ffprobe` reports `pcm_s16be` in an aiff container + stream>0, source-unchanged, lossless-as-target with the same >16-bit → `audio_bitdepth` caveat as WAV, `audio_tags_dropped` when the source carried tags, channels preserved.
- [ ] **P6.40** [TEST] Per-pair audio tests: → ALAC (native `alac` + ipod muxer, all sources) · §6.4.3 §6.5 · G31 G32
  needs: P6.30, P6.31, P6.29, P4.59
  > each `* → ALAC` pair: completes, `ffprobe` reports `alac` in an MP4/ipod container + faststart + stream>0, source-unchanged, lossless (no knob), `audio_lossy_origin` for a lossy source, NO patent flag (ALAC is open — never AAC's §3.4 status), `ilst` tag + cover-art content-fidelity.
- [ ] **P6.41** [TEST] Per-pair audio tests: WMA source → audio targets (decode-only source path) · §6.4.3 §6.5 · G31 G32
  needs: P6.30, P6.31, P6.29, P4.59
  > each `WMA → {target}` pair (WMA is source-only, no `→ WMA`): the `wmav1`/`wmav2`/`wmapro`/`wmalossless` decoders feed every offered audio target; completes, `ffprobe` reports the target codec + stream>0, source-unchanged; WMA→lossless-target = `audio_lossy_origin`, WMA→lossy-target = second lossy round (`audio_transcode`); ASF metadata mapped to tag-supporting targets; the DRM/"copy-protected" WMA fixture fails clearly (P6.12 classifier), batch continues.
- [ ] **P6.42** [TEST] Audio determinism floor — same source+settings twice → byte-identical, per output-format category · §6.4.3 §2.5 · G32
  needs: P6.32, P6.33, P6.34, P6.35, P6.36, P6.37, P6.38, P6.39, P6.40, P6.41
  > the §2.5/G32 determinism floor — same source+settings twice → `sha256(out1)==sha256(out2)` for ≥1 pair per audio output-format category (enumerated in the manifest); document known-non-deterministic encoders as manifest exceptions. Split from the cross-decoder leg (P6.43) — these are orthogonal test properties (encoder non-determinism vs codec-compatibility), each with different fixtures + assertions.
- [ ] **P6.43** [TEST] Audio cross-decoder ffprobe re-validation for the headline formats · §6.4.3 §6.5 · G32 G38
  needs: P6.32, P6.34, P6.35, P6.37, P6.38
  > the cross-decoder re-validation property for the headline audio formats — re-decode each output with `ffprobe` (and the P0.5.6 cross-library convention) to confirm codec compatibility, distinct from the per-pair structural reader; the `ffmpeg-allowed-decoders.lock`-golden skip evaluation (a golden-listed decoder absent from the live binary is a G38 hard-fail). Orthogonal to the determinism floor (P6.42).
- [ ] **P6.44** [TEST] Add the per-push adversarial-egress + T9b-sentinel PULL-FORWARD leg for FFmpeg audio · §6.4.2 §2.11.4 §0.11 · G42 G42b
  needs: P6.32, P6.33, P6.34, P6.35, P6.36, P6.37, P6.38, P6.39, P6.40, P6.41, P0.7.12
  > the §6.4.2 per-push adversarial-egress + T9b-sentinel corpus run inside G42's egress-deny window (the P0.7.12 "per-push pull-forward" leg activating from P6 as the first egressing engine is staged): a crafted network-trigger A/V input must show ZERO egress (incl. zero DNS) AND no out-of-input file read, so a T9b regression is caught on the push that introduces it. (Full per-OS deny window + release-confirmation leg are P9.)

---

## Internal §6.5 sub-gate — audio reliable before video

- [ ] **P6.45** [TEST] Sub-gate — assert every audio pair is `reliable` in the ledger before any video pair is attempted · §6.5 §6.5.2 · G31
  needs: P6.32, P6.33, P6.34, P6.35, P6.36, P6.37, P6.38, P6.39, P6.40, P6.41, P6.42, P6.43, P6.44
  > the skeleton-review-r3 intra-phase milestone: assert the §6.5.2 pair-status ledger (`reliability-report.json`) marks EVERY enumerated audio pair `reliable` on all three available platforms (or `unavailable-per-§3.4` for the AAC/M4A cells) before the video cluster begins — video on three platforms is the heaviest corpus run, so the audio-reliable milestone gives measurable progress. Every subsequent video box transitively follows this via the runner; the gate is the named checkpoint.

---

### Video pairs (FFmpeg, container conversions + remux/re-encode)

> The user-facing format is the CONTAINER (§1.3 batch key); what is cheap depends
> on the inner CODECS (probed by FFprobe). Remux-vs-re-encode is decided
> AUTOMATICALLY per item from the inner-codec inventory — never asked. MP4 is the
> pre-highlighted default for every video source. AVI/WMV/FLV/MPG/3GP are valid
> SOURCES but NOT offered as targets. Each video box `needs:` the audio sub-gate
> (P6.45).

- [ ] **P6.46** [RUST] Wire the video-source detection signatures + brand/DocType disambiguation · §1.2 · G15 G31
  needs: P6.45
  > add the §1.2 video signatures: MP4-family `ftyp` with brand disambiguation (`isom`/`mp4x`/`avc1` = MP4, `qt  ` = MOV, `M4V `/`M4VH` = M4V, `3gp4`/`3g2a` = 3GP — brand not extension); MOV `moov`/`mdat`/`wide` for ftyp-less QuickTime; MKV/WEBM EBML `1A 45 DF A3` disambiguated by DocType (`matroska` vs `webm`); AVI RIFF+`AVI `; WMV ASF GUID + video-stream-present (vs WMA audio-only); FLV `FLV`+version; MPG/MPEG start codes `00 00 01 BA`/`B3` covering the full `.mpg`/`.mpeg`/`.m1v`/`.m2v`/`.vob` (DVD) extension set (video.md:297) + `.ts` sync `0x47` (transport stream, treated here) — the legacy `.vob`/`.m1v`/`.m2v` extensions pinned so none is silently dropped.
- [ ] **P6.47** [RUST] Wire the automatic remux-vs-re-encode decision from the FFprobe inner-codec inventory · §3.5.1 §3.2 · G31
  needs: P6.46, P6.10
  > the §3.5/video.md per-item decision (a §3.2 capability decision, zero user choice): remux (`-c copy`, lossless) IFF every kept stream's codec is legal in the target container AND no normalization is needed; else re-encode (decode → H.264/AAC or VP9/Opus, lossy); MIXED allowed (video copies while audio transcodes) — still ONE FFmpeg invocation. Per-item from the inventory, never an always-remux path (FLV VP6/Sorenson, WMV7/8, MKV-only audio all force re-encode).
- [ ] **P6.48** [RUST] Wire the H.264 re-encode params + faststart + yuv420p + rotation, reading §3.4 H.264 · §3.5.1 §3.4 · G31
  needs: P6.47
  > re-encode path for MP4/MOV/MKV/M4V: `-c:v libx264 -crf 23 -preset medium -pix_fmt yuv420p` + `-c:a aac -b:a 128k`; `-movflags +faststart` (front-loaded moov) for MP4/MOV/M4V; `-fflags +genpts` for FLV remux; rotation honoured (portrait stays portrait); resolution/fps unchanged (never upscale). READS the §3.4 H.264/AAC cell (P4 matrix) — ship-bundled on all three platforms is the category's hardest dependency (MP4 is every source's default).
- [ ] **P6.49** [RUST] Wire the VP9/Opus WEBM-target re-encode params (constant-quality, single-pass) · §3.5.1 · G31
  needs: P6.47
  > WEBM target `-c:v libvpx-vp9 -b:v 0 -crf 32 -row-mt 1` (constant-quality, single-pass — two-pass + AV1-as-WEBM-target are DECIDED-not-in-v1) + `-c:a libopus -b:a 96k`; → WEBM is ALWAYS lossy re-encode (codecs never match the H.264/AAC mainstream). VP9 CRF validation bound is `0..=63` (15–35 recommended band, default 32) — must not clamp the codec range.
- [ ] **P6.50** [RUST] Wire the HEVC/H.265-default disposition (re-encode to H.264) + the keep-original Advanced toggle · §3.5.1 §3.4 · G31
  needs: P6.48
  > the DECIDED HEVC default: an H.265 source (common iPhone `.mov`) re-encodes HEVC→H.264 by DEFAULT (lossy, larger, plays everywhere — honours the usability-floor mov→mp4 promise) using FFmpeg's native `hevc` DECODER (inside the GPL binary, never libde265); a "keep original quality (H.265)" Advanced toggle offers verbatim remux. Same disposition for AV1-in-MP4. Decode reads the §3.4 HEVC-video-decode cell.
  - [ ] **P6.50.1** [UI] Register the "keep original quality (H.265)" Advanced-option DECLARATION · §1.6 · G47
    > the verbatim-remux toggle, default OFF (re-encode is the default); same toggle covers AV1-in-MP4.
- [ ] **P6.51** [RUST] Wire the audio-tracks + subtitles + chapters/attachments keep/convert/drop policy · §3.5.1 · G31
  needs: P6.47
  > keep ALL audio tracks (remux copies; re-encode transcodes each to AAC/Opus; WEBM keeps first track); MKV→MP4 subtitles: TEXT (SRT/MOV_TEXT/WebVTT) → converted to `mov_text` in the same invocation; IMAGE (PGS/VobSub) + styled ASS/SSA → DROPPED with `video_subs_dropped` (no subtitle burn-in in v1); chapters + font attachments copied to MKV, dropped-with-note for MP4 where unsupported.
- [ ] **P6.52** [RUST] Wire the auto-deinterlace (yadif) + metadata/color/HDR preservation + alpha-loss note · §3.5.1 · G31
  needs: P6.47
  > `yadif` (mode 0) deinterlace default-ON for flagged-interlaced sources (DEFER:corpus calibrates only the call, not the design); `-map_metadata 0` metadata preserve (no strip toggle in v1); color primaries/transfer/matrix + HDR (BT.2020/PQ/HLG) preserved on remux, kept-as-signalling on H.264 re-encode (no tone-map in v1); WEBM-alpha → H.264 fires `video_alpha_lost`.
- [ ] **P6.53** [RUST] Wire the per-format video target registrations (MP4/MOV/MKV/WEBM/M4V) + the self-conversion normalize path · §3.5.1 · G31
  needs: P6.48, P6.49
  > register the five offered video targets (MP4, MOV, MKV, WEBM, M4V) reading each source's offered set + the `R`/`✓~` disposition from the video.md matrix; the same-container "self" path (MP4→MP4 etc.) NORMALIZES (remux + `+faststart` + re-index, no re-encode) and writes `name (1).mp4` beside the source (no overwrite). AVI/WMV/FLV/MPG/3GP have NO self target (not offered as targets). Software-only encoding (no NVENC/QSV/VideoToolbox in v1). **M4V = all 10 video sources (video.md:330-336 correction, pinned so the target is not under-offered):** the M4V target source-list is **identical to the MP4 target's** — MP4/MOV/MKV/WEBM/AVI/WMV/FLV/MPG/3GP/M4V all valid (MP4-family sources remux, the rest re-encode `✓~`), the matrix M4V column marks every source row; this expressly corrects the earlier 5-source list (MP4/MOV/MKV/FLV/M4V) that wrongly excluded AVI/WMV/WEBM/MPG/3GP.
  > **Pessimistic-outcome wiring-consequence (reciprocal of P9.43):** a pessimistic P9.43 MOV-as-target ship-vs-demote corpus outcome may EDIT this registration — drop the MOV cell from the offered target set + add a `docs/demoted-pairs.md` row (`kind=corpus-no-demand`) + the matrix-column update — so the backward amendment named in P9.43's note is auditable from this end too.
- [ ] **P6.54** [TEST] Wire the every-source-default-is-MP4 zero-click assertion · §1.6 · G31 G61
  needs: P6.48, P4.60.2
  > the per-CATEGORY video default-target table (MP4 the pre-highlighted default for all ten sources); its §04-offered video pairs + their `OptionDecl` defaults FEED the §1.6 consolidated defaults registry the **P4.60.2 G61 guard** merges + checks across all options of all offered pairs (the single machine-checkable home of the no-required-choices gate — `needs: P4.60.2` so the video default table is registered against the guard, not asserted ad-hoc here); flag the §3.4 H.264/AAC-ship-bundled-on-all-three-platforms hard precondition (a platform with no H.264 encode would have no default target — a product problem, not a footnote).
- [ ] **P6.55** [TEST] Wire the worst-case `willReencode` note + the §1.12 actual-disposition summary · §2.9 §2.9.2 §0.4.2 · G31 G32
  needs: P6.47
  > the §2.9.2 timing rule: the target-choice note is a header/container-pair worst-case (`RunStarted.willReencode`, §0.4.2) — a definitely-re-encode pair (→WEBM, legacy source) fires `video_reencode` certainly; a commonly-remux pair fires the "may be re-encoded" worst-case rather than falsely promising losslessness; the §1.12 end-of-batch summary reflects what ACTUALLY happened once §3.5 resolved the real per-item disposition. G32 lossy-disclosure-iff-flagged uses the PLANNED disposition.
- [ ] **P6.56** [RUST] Wire the DRM-protected + zero-audio + very-large video edge handling · §3.5.1 §1.10 · G31
  needs: P6.12, P6.47
  > DRM (FairPlay `.m4v`, PlaysForSure WMV/ASF) → the §video.md "copy-protected, can't be converted" message, batch continues, nothing written; a source with no audio track converts fine (silent video, never an error); §1.10 owns the up-front size/space pre-flight + "too big" fast-fail (video is the category most likely to trip the budgets); concurrency degree owned by §0.9 (low parallelism for CPU-heavy re-encode).

---

### Video corpus + per-pair video tests

- [ ] **P6.57** [TEST] Stage the video corpus (short clips, one per source + the inner-codec cases) + manifest + SHA-256 · §6.4.5 · G24a G22
  needs: P6.1, P0.5.11
  > add `tests/corpus/video/` short clips: MP4 (H.264+AAC, lossless-remux baseline); MOV-from-iPhone (HEVC, the re-encode-default case); MKV with multiple audio tracks + SRT + ASS + PGS subtitles + chapters + font attachments; WEBM (VP9+Opus, and a VP8 alpha clip); AVI (DivX+MP3); WMV (VC-1+WMA); FLV (H.264/AAC and old Sorenson); MPG (interlaced MPEG-2 + AC-3); M4V (DRM-free); 3GP (H.263+AMR-NB) — each with its `manifest.toml` `[[file]]` + redistributable licence; regenerate the SHA-256 corpus manifest **via the `stage-corpus` generator (P0.5.11)** in the same commit (G24a). (`needs: P0.5.11` for the manifest generator.)
- [ ] **P6.58** [TEST] Stage the video edge-case fixtures (DRM, rotation, VFR, silent, interlace) + content-floor `representative-av` · §6.4.5 · G24a G31
  needs: P6.57, P0.5.11
  > add a DRM-protected FairPlay `.m4v` + a DRM WMV (fail-clearly); a portrait/rotated clip (rotation honoured); a VFR screen recording (to-GIF fps-normalise); a silent clip (extract-audio "no audio track"); a long-ish clip for the to-GIF guardrail/cap; tag the `representative-av` content floor (≥1 real video, already implied by the per-format rows). These are NEW SHA-256-manifest-tracked fixtures, so regenerate the manifest **via the `stage-corpus` generator (P0.5.11)** in the same commit (G24a). (`needs: P0.5.11` for the manifest generator.)
> **Per-CONTAINER-target per-pair video test split (one box per remux/re-encode
> code-path).** The former monolithic "every container pair" box is split into one
> box per target-container code-path (the remux-vs-re-encode decision differs per
> target family), so each code-path is independently dual-reviewed. Every box runs
> on the P4.59 §6.4.3 per-pair runner against every corpus file of each source on all
> three platforms: completes + output decodes via `ffprobe` (expected codec, stream
> count > 0); no-harm + fail-clearly on DRM/corrupt fixtures; **remux-vs-re-encode
> chose the LOSSLESS path when codecs already fit** (the key video content-fidelity
> check); lossy disclosure fires per the PLANNED disposition (P6.55); rotation /
> subtitle / chapter content-fidelity spot-checks; §3.4-patent-gapped targets asserted
> absent (not attempted). (`needs: P4.59` carried per the P6.92 reconciliation
> obligation.)
- [ ] **P6.59** [TEST] Per-pair video tests: → MP4 / MOV / M4V (H.264+AAC re-encode + faststart + HEVC-default, all sources) · §6.4.3 §6.5 · G31 G32
  needs: P6.57, P6.58, P6.55, P4.59
  > each `* → {MP4,MOV,M4V}` pair: H.264+AAC re-encode where codecs don't fit, `-c copy` remux where they do (the lossless-when-it-fits check), `+faststart` (moov front-loaded) asserted, the HEVC-source-default re-encode-to-H.264 disposition (P6.50) exercised, rotation honoured; reads the §3.4 H.264/AAC cell (ship-bundled all 3) — MP4 is every source's default so this is the category's hardest dependency.
- [ ] **P6.60** [TEST] Per-pair video tests: → WEBM (VP9+Opus re-encode, all sources) · §6.4.3 §6.5 · G31 G32
  needs: P6.57, P6.58, P6.55, P4.59
  > each `* → WEBM` pair: ALWAYS lossy VP9+Opus re-encode (codecs never match the H.264/AAC mainstream — `video_reencode` certain), `-c:v libvpx-vp9 -crf 32` + `-c:a libopus`, output decodes via `ffprobe` (vp9/opus), VP9 CRF range honoured (0..=63 not clamped); a VP8-alpha source fires `video_alpha_lost`.
- [ ] **P6.61** [TEST] Per-pair video tests: → MKV (multi-stream + subtitle convert/drop, all sources) · §6.4.3 §6.5 · G31 G32
  needs: P6.57, P6.58, P6.55, P4.59
  > each `* → MKV` pair: all audio tracks kept (remux copies / re-encode transcodes each), TEXT subtitles (SRT/MOV_TEXT/WebVTT) converted, IMAGE (PGS/VobSub) + styled ASS/SSA DROPPED with `video_subs_dropped`, chapters + font attachments copied; output decodes via `ffprobe` with the expected stream set; the MKV-only-audio force-re-encode case exercised.
- [ ] **P6.62** [TEST] Per-pair video tests: legacy sources (AVI/WMV/FLV/MPG/3GP) → offered targets — force-re-encode + DRM/corrupt fail-clearly · §6.4.3 §6.5 · G31 G32
  needs: P6.57, P6.58, P6.55, P4.59
  > each legacy `{AVI,WMV,FLV,MPG,3GP} → {offered target}` pair: the force-re-encode path (FLV VP6/Sorenson, WMV7/8, DivX, MPEG-2/H.263 all force re-encode — never an always-remux assumption), `+genpts` for FLV; output decodes via `ffprobe`; the DRM WMV + corrupt fixtures fail clearly (P6.12 classifier), batch continues; legacy sources are NOT offered as targets (no self path).
- [ ] **P6.63** [TEST] Per-pair video tests: → MP4-family self-conversion normalize path (remux + faststart + re-index, no re-encode) · §6.4.3 §6.5 · G31 G32
  needs: P6.57, P6.58, P6.55, P4.59
  > each same-container "self" pair (MP4→MP4, MOV→MOV, MKV→MKV, WEBM→WEBM, M4V→M4V): the NORMALIZE path (remux + `+faststart` + re-index, NO re-encode), output `name (1).<ext>` written beside the source (no overwrite), `ffprobe` confirms codecs unchanged (lossless normalize); the distinct code-path from the cross-container re-encode boxes above.
- [ ] **P6.64** [TEST] Add the video determinism note + the per-push adversarial-egress leg for FFmpeg video · §6.4.2 §6.4.3 §2.11.4 · G32 G42 G42b
  needs: P6.59, P6.60, P6.61, P6.62, P6.63, P0.7.12
  > extend the §6.4.2 per-push adversarial-egress + T9b-sentinel run to the video corpus (crafted external-reference/manifest-bearing video → zero egress incl. DNS + no out-of-input read); document VP9/AV1 variable-encode as known-non-deterministic G32 manifest exceptions (no `sha256` determinism floor for those, a `diffoscope`-localised note instead).

---

### Cross-category: extract-audio (video → audio subset)

> Operations on a video source, NOT a second source format. The batch key is the
> video source type only (§1.3); the offered target set = the video targets PLUS
> "extract audio (→ …)" PLUS "to animated GIF". One FFmpeg invocation (demux +
> optional re-encode). Resolves the deferred [OPEN-A] subset (floor MP3★+WAV+FLAC
> guaranteed; M4A/OGG corpus-validated).

- [ ] **P6.65** [RUST] Wire extract-audio as a target of every video source (`-vn -map 0:a:0`) + the first-track rule · §3.5.1 §1.5 · G31
  needs: P6.45, P6.46
  > offer extract-audio on all ten v1 video sources (§1.5 target resolution adds it alongside the video default); `-vn -map 0:a:0` (deterministic FIRST audio track in v1 — per-track / all-tracks is parked, no one-to-many fan-out); preserve source sample rate + channels (no resample/downmix by default); carry source tags where the target container supports them; cover-art extraction is NOT part of extract-audio.
- [ ] **P6.66** [RUST] Wire the GUARANTEED extract-audio target floor (MP3★/WAV/FLAC) · §3.4 · G31
  needs: P6.65, P6.18, P6.19, P6.20
  > register the [OPEN-A] **guaranteed floor** {MP3★ default, WAV, FLAC} (C3-derivable now once the MP3/WAV/FLAC encode paths P6.18–P6.20 are done — the SSOT mov→mp3 case in scope; this leg can be checked off immediately, **independent of any corpus evidence**). Excluded as extract targets: raw AAC, OPUS, WMA, AIFF, ALAC. Reuse the audio.md encode params + the canonical MP3 preset table verbatim ([OPEN-B] resolved). The corpus-validated **M4A/OGG additions** are the separate box P6.83 (they require P9.44 corpus evidence and must NOT block the floor's check-off — split per the atomicity bar).
- [ ] **P6.67** [RUST] Wire the extract-audio stream-copy-vs-re-encode decision (codec-inside-container) · §3.5.1 · G31
  needs: P6.66
  > automatic per-item: `-c:a copy` (lossless, fast) when source codec is byte-compatible with the chosen target container — source AAC → M4A (the dominant MP4/MOV/M4V/3GP case), source MP3 → MP3 (FLV/AVI), source Vorbis → OGG (WebM); else re-encode (any → MP3/WAV always-decode-to-PCM/FLAC-lossless, AAC→MP3, etc.). Engine-internal §3.2 capability decision, zero user choice; the lossy note reflects the OUTCOME not the mechanism.
- [ ] **P6.68** [RUST] Wire the M4A-extract-target §3.4 gate at the target level (copy path noted, gate at M4A) · §3.4 · G31
  needs: P6.67
  > the DECIDED rule: the AAC→M4A `-c:a copy` path decodes/remuxes only (lighter patent profile, no encode) — NOTED — but to keep the format×platform offered set honest, if §3.4 marks AAC unavailable on a platform the M4A extract target is DISABLED there REGARDLESS of copy-vs-encode, falling back to MP3 (already the default, no UX disruption). One consistent availability story per platform.
- [ ] **P6.69** [RUST] Wire the extract-audio NoAudioTrack named-failure + edge cases · §2.8 · G31
  needs: P6.65, P6.12
  > the §2.8 `NoAudioTrack` kind ("This file has no audio to extract.") on a silent source — a NAMED failure, batch continues, never a 0-byte audio file ([OPEN-C] cheap up-front probe to disable-with-reason is DEFER:corpus); multichannel (5.1) preserved into WAV/FLAC (not auto-downmixed); corrupt/truncated source → item fails clearly, no partial audio (§2.1/§2.6); WAV/FLAC extraction cannot un-bake the source's existing lossy compression (the §2.9 note must not imply quality improvement).
  - [ ] **P6.69.1** [UI] Register the extract-audio quality advanced-option DECLARATIONS (per-target, reusing audio.md presets) · §1.6 · G47
    > MP3 quality (Standard ≈ V2 default), M4A re-encode bitrate, FLAC compression level, OGG quality — all reusing the audio.md canonical preset tables; WAV has none (fixed 16-bit PCM).

---

### Cross-category: to-animated-GIF (video → GIF, with guardrails)

> Turn a short video clip into a shareable animated GIF — one FFmpeg invocation via
> the split/palettegen/paletteuse filtergraph (no temp PNG, no chaining). ALWAYS
> intrinsically lossy. Resolves the deferred to-GIF trim + size-cap items
> ([OPEN-E]/[OPEN-F] finite ship-now values, corpus-calibrated).

- [ ] **P6.70** [RUST] Wire to-GIF as a target of every video source via the single-process palette filtergraph · §3.5.1 §3.2 · G31
  needs: P6.45, P6.46
  > offer to-GIF on all ten v1 video sources; build the single-invocation filtergraph `fps=<fps>,scale=<w>:-1:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5` + `-loop 0` — NO intermediate palette PNG (one §3.2 engine call, no chaining); `lanczos` scale + per-clip palette = quality, `fps`-down + width-cap = sane size; audio dropped (intrinsic), transparency not preserved (opaque output).
- [ ] **P6.71** [RUST] Wire the to-GIF basic options — FPS + width defaults + the dither/loop/colours fixed defaults · §1.6 · G31
  needs: P6.70
  > FPS preset (Smooth 15 / Standard 12 / Small 10, default 12); Width preset (Large 640 / Medium 480 / Small 320 px, height `-1` aspect-kept, default 480); fixed defaults — dither `bayer:bayer_scale=5` ([OPEN-D] DECIDED), `-loop 0` infinite, 256 colours; VFR sources fps-normalised; odd dimensions even-rounded; sub-second clip → valid tiny GIF.
  - [ ] **P6.71.1** [UI] Register the to-GIF FPS + Width Basic-option DECLARATIONS · §1.6 · G47
    > FPS + Width in the Basic view (they visibly change smoothness/size vs file size).
  - [ ] **P6.71.2** [UI] Register the to-GIF dither Advanced-option DECLARATION (bayer/sierra2_4a/floyd_steinberg/none) · §1.6 · G47
    > the v1-exposed dither subset only (NOT `sierra2`/`heckbert` — FFmpeg accepts but v1 hides; `sjpeg` is not a valid value); default `bayer:bayer_scale=5`.
- [ ] **P6.72** [RUST] Wire the to-GIF trim window (start + duration) · §1.6 · G31
  needs: P6.70
  > the [OPEN-E] resolution (design leans Basic start+duration, validate §6.6): `-ss <start>` + `-t <duration>` in the same single invocation; default = whole clip up to the duration cap (P6.73). A trim window is most of why people make GIFs.
  - [ ] **P6.72.1** [UI] Register the to-GIF trim Basic-option DECLARATION (start + duration) · §1.6 · G47
    > two number fields ("from 00:15, for 6 s"); leaving defaults = whole clip up to cap.
- [ ] **P6.73** [RUST] Wire the to-GIF size guardrail — up-front estimate + duration cap + fail-fast (feeds §1.10) · §1.10 §2.8 · G31
  needs: P6.70, P6.72, P4.72
  > the MANDATORY guardrail (this op supplies the inputs; the **P4-built §1.10 engine (P4.72)** owns the threshold mechanics): up-front cheap estimate `fps × min(clip_len, trim_or_cap) × out_w × out_h × ~1 byte/px` (no decode needed); a finite default duration cap (proposal N=10 s, [OPEN-F] DEFER:corpus — MUST be some finite value, leaving it unset reintroduces the foot-gun) applied as `-t`; fail-fast up front if the estimate exceeds the §1.10 "too big" ceiling (a §2.8 named failure kind), batch continues; a cap that shortened the clip is a DISCLOSED outcome (`video_to_gif`), never silent truncation. (cross-category.md owns the GIF-specific defaults; this box feeds them into the §1.10 engine.)
- [ ] **P6.74** [RUST] Wire the to-GIF unconditional lossy note + HDR/4K edge handling · §2.9 · G31 G32
  needs: P6.70
  > to-GIF is ALWAYS intrinsically lossy (fps-down + scale-down + 256-colour quantize + audio-drop), so the §2.9 `video_to_gif` passive note shows UNCONDITIONALLY (once, calmly, not per-conversion) — G32 must assert it fires for every to-GIF pair; HDR/wide-gamut tone-mapped down by the decode→scale→palettegen chain (flattened but valid); 4K caught by the 480px width default + the guardrail; corrupt source → fails clearly, no partial GIF.

---

### Cross-category corpus, tests, re-run detection & batch

- [ ] **P6.75** [TEST] Stage the to-GIF bijection corpus coverage (every `["<SOURCE>","GIF"]` pair) + extract-audio covers · §6.4.5 §6.4.3a · G24a G22
  needs: P6.57, P0.5.11
  > extend each video corpus item's `covers` list to include its `["<SOURCE>","GIF"]` 2-tuple (MP4 item → `["MP4","GIF"]`, WEBM item → `["WEBM","GIF"]`, … — not one generic clip) AND its extract-audio **guaranteed-floor** `(video → MP3)`/`(video → WAV)`/`(video → FLAC)` 2-tuples (the floor enumerated explicitly, not the ambiguous `…`); the **conditional `(video → M4A)`/`(video → OGG)` extract-audio covers are added by P6.83.1** when P9.44 unlocks those targets (so the bijection guard's required-pair input is unambiguous and a shipped M4A/OGG extract pair has a backing file). Regenerate the SHA-256 manifest **via the `stage-corpus` generator (P0.5.11)** in the same commit (G24a). (`needs: P0.5.11` for the manifest generator.)
- [ ] **P6.76** [TEST] Add the cross-category per-pair integration tests (extract-audio FLOOR + to-GIF, structural readers) · §6.4.3 §6.5 · G31 G32
  needs: P6.75, P6.69, P6.74, P6.68, P4.59
  > for every `(video → audio-FLOOR-subset {MP3/WAV/FLAC})` extract-audio pair and every `(video → GIF)` pair, against the corpus, on all three platforms: completes + output decodes (`ffprobe` for extracted audio with expected codec; GIF89a valid + nonzero frames for to-GIF); the stream-copy path verified lossless where codecs match; the NoAudioTrack fixture fails-clearly; the to-GIF note fires unconditionally; the guardrail fail-fast triggers on the over-cap fixture; M4A patent-gapped target asserted absent where §3.4 unavailable. **The corpus-validated M4A/OGG extract-audio additions are tested by P6.83** (the `[!]` box unlocked by P9.44) — NOT a `needs:` of this floor box, so this box does not deadlock waiting on the P9.44 corpus evidence (P9.44 itself `needs: P6.80` which `needs:` this box, so P6.83 must not be a prerequisite of the gate that ultimately unlocks it).
- [ ] **P6.77** [TEST] Wire the cross-category re-run/equivalent-output detection (source + target + effective settings) · §2.5 · G31
  needs: P6.65, P6.70
  > §2.5 keys on source + target + EFFECTIVE settings: "extract audio → MP3 (Standard)" re-run on the same video triggers the skip/fresh-copy prompt; changing fps or MP3 quality is a NEW conversion (ordinary numbering); output naming/no-clobber/atomic-write/beside-source-divert apply identically (an extracted `clip.mp3` / `clip.gif` keeps the source base name + new extension, no-clobber numbered).
- [ ] **P6.78** [TEST] Wire the cross-category batch interaction (one chosen target over the same-source batch) · §1.3 · G31
  needs: P6.65, P6.70
  > the SSOT batch rule for this file: cross-category outputs are TARGETS of one video source, never a second source format; the batch key is the video source type only (§1.3); choosing "Extract audio" or "To GIF" applies that one target to the whole same-source batch (48 `.mov` → 48 MP3s, or 48 GIFs); per-file target is out of v1.

---

### macOS TCC staging, phase reliability gate & advanced-options completeness

- [ ] **P6.79** [RUST] Verify FFmpeg receives the macOS kind-2 scratch-staged source path (never the raw protected path) · §3.5.0 §7.2.6 §2.14.2 · G31
  needs: P6.8, P4.25
  > assert (macOS only) that the **P4-built TCC source-staging (P4.24/P4.25)** stages the dropped source into per-job kind-2 scratch BEFORE spawning FFmpeg and hands FFmpeg the SCRATCH path as `<input>` — so a spawned engine is never the first process to touch a TCC-protected Desktop/Documents/Downloads/removable path (composes with the §2.14 cross-volume strategy; the macOS staged-input term in the §1.10 preflight). Read-side only (the write-side `.part` is the core's per §7.2.6). (`needs: P4.25` — the P4 staged-path engine-arg plumbing, per the P6.92 reconciliation obligation.)
- [ ] **P6.80** [TEST] Assert the §6.5 phase reliability gate — every P6 pair `reliable` on all three platforms · §6.5 §6.5.2 · G31 G32
  needs: P6.45, P6.59, P6.60, P6.61, P6.62, P6.63, P6.76, P6.79, P4.60, P4.61
  > the phase-level §6.5 coverage gate: the **P4-built §6.5.2 pair-status ledger generator (P4.61) + the §6.4.3a corpus↔pair bijection guard (P4.60)** mark EVERY enumerated P6 pair (audio + video container + extract-audio + to-GIF) `reliable` on every platform where it is not `unavailable-per-§3.4` or `demoted`; any `failing` cell blocks; record the two permissible exceptions (patent per-platform gap; last-resort demotion) in `docs/demoted-pairs.md` + the ledger with the required fields. The report is published as a release asset. **The conditional M4A/OGG extract-audio pairs are covered: whenever P9.44 KEEPS a target, P6.83.1 has staged its covers + per-pair §6.4.3 tests, so the ledger cell exists and this gate sees it `reliable`/`failing` (never silently absent); a P9.44-DEMOTED target carries its `docs/demoted-pairs.md` row instead — so a shipped-but-untested extract M4A/OGG pair cannot reach release un-ledgered.** (`needs: P4.60/P4.61` — the P4 ledger generator + bijection guard, per the P6.92 reconciliation obligation. P6.83/P6.83.1 are not a `needs:` of this gate — they are `[!]`-unlocked by P9.44 which itself `needs: P6.80`, so making them prerequisites would cycle; the ledger COVERAGE check fires on whatever pairs exist at RC, and P11.15 + the §6.5.3 bijection P11.16 are the release-time backstop that every existing cell is `reliable` or documented.)
- [ ] **P6.81** [TEST] Assert the FFmpeg-family advanced-option completeness (every declared option resolves + every pair has a test) · §1.6 §6.4 · G22 G23
  needs: P6.18, P6.19, P6.20, P6.21, P6.22, P6.23, P6.24, P6.25, P6.50, P6.69, P6.71, P6.72
  > the §6.4 completeness wiring for this engine: G22 every FFmpeg-family format ∈ the §04 category format matrices (the `docs/spec/04-formats/` audio/video/cross-category matrices the bijection guard reads — not a README table) ∧ has a corpus fixture ∧ has a round-trip test; G23 every `convert_*`/engine command for an FFmpeg pair has a partner test; every registered §1.6 advanced-option declaration resolves to a non-empty handler + a UI control on the P4 panel (no orphan declaration, no declared-but-unwired option).
- [ ] **P6.82** [TEST] Add the FFmpeg engine-bump re-validation hook (full reliability gate re-runs on a pin change) · §6.5.4 §3.8 · G37 G17b
  needs: P6.2, P6.80
  > wire the §6.5.4 rule for the FFmpeg `engines.lock` pin: a version/SHA change re-runs the FULL P6 reliability gate before that FFmpeg version can ship (a patch must not silently regress a pair); the ledger status-diff is part of the bump review; the informational per-push OSV/grype over the PURL-keyed FFmpeg row (CPE `cpe:2.3:a:ffmpeg:ffmpeg:<ver>`) feeds vuln-response (CVSS ≥ 7 on an exercised path → release-blocking escalation).

### Corpus-validated extract-audio additions (split from the guaranteed floor)

- [!] **P6.83** [RUST] Wire the corpus-validated extract-audio M4A/OGG additions (on the P6.66 floor) · §3.4 · G31
  unlocked-by: P9.44
  > blocked: the [OPEN-A] **corpus-validated additions** {M4A, OGG} registered on top of the P6.66 guaranteed floor are genuinely **un-buildable until P9.44 lands its corpus evidence** (M4A pending §3.4 AAC confirmation; OGG pending the §6.6 OGG-keep round-trip) — P9.44 is an activation/unlock (it "demotes only these two targets" on a pessimistic outcome), NOT a staged input to a step this box can run now, so this is a `[!]` + `unlocked-by:` block, not a `[ ]` + `needs:` (the _format.md §5.2 worked-example test: there is nothing to build at P6.83 until P9.44's corpus run decides keep-vs-demote — exactly the P5.4-style case). On the auto-unlock scan P9.44→`[x]` flips this to `[ ]` with the effective `needs: P6.66, P6.68` (the floor + the §3.4 M4A target-level gate it then registers on); a P9.44 demote outcome drops the affected target + adds a `docs/demoted-pairs.md` row (the P9.44 wiring-consequence this box owns). Reuse the audio.md encode params verbatim. (P9.44 in turn `needs: P6.80`, the P6 FFmpeg-reliability gate, so the corpus run validates against real staged infrastructure — see P9.44.) The corpus COVERS + the per-pair §6.4.3 integration TESTS for these two cross-category targets are the sibling sub-box P6.83.1 (the RUST encode-path registration here vs the TEST corpus+test home there are disjoint surfaces, separately dual-reviewed).
  - [!] **P6.83.1** [TEST] Stage the (video → M4A)/(video → OGG) extract-audio covers + the per-pair §6.4.3 integration tests · §6.4.5 §6.4.3 §6.5 · G24a G31 G32
    unlocked-by: P9.44
    > blocked: the M4A/OGG extract-audio targets are un-buildable until P9.44 lands its corpus keep/demote evidence (the same block as the parent P6.83) — there is nothing to stage/test until the targets exist, so this is `[!]` + `unlocked-by:` not `[ ]` + `needs:`. On the auto-unlock scan P9.44→`[x]` flips this to `[ ]` with the effective `needs: P6.83` (the encode paths), `P6.75` (the covers list to extend), `P0.5.11` (the manifest generator) and `P4.59` (the §6.4.3 per-pair runner): extend each video corpus item's `covers` with its conditional `(video → M4A)`/`(video → OGG)` 2-tuples (regenerate the SHA-256 manifest via `stage-corpus` in the same commit, G24a — the bijection guard P4.60 then has a backing file for each kept target) AND add the per-pair `(video → M4A)`/`(video → OGG)` §6.4.3 integration tests on the P4.59 runner (completes + `ffprobe` reports the expected codec/container, stream-copy-where-codecs-match verified lossless, `audio_lossy_target` fires iff §04-flagged, §3.4-AAC-unavailable platforms assert the M4A extract target absent). A P9.44 demote outcome drops the demoted target's covers + tests instead (no orphan covers). Named in P6.80 so a shipped-but-untested extract M4A/OGG pair cannot reach release un-ledgered.

---

### Audio advanced-option declarations (registered against the P4 options-panel shell)

> The panel **chrome** was built in P4 (P4.64 widget dispatch + P4.74 AdvancedDrawer);
> these boxes register only per-target audio option **DECLARATIONS** (§1.6), elevated to
> top-level boxes (matching P5's RUST-savers-vs-UI-declarations separation, P5.20–P5.25
> vs P5.37–P5.46) so a [RUST] encoder box is never blocked on its [UI] declaration — each
> declaration `needs:` its encoder box (the source-of-the-knob) and renders against the
> P4 panel (the `needs: P4.64/P4.74` edge is carried centrally by the P6.92 reconciliation
> box, mirroring P5.74). No new panel chrome.

- [ ] **P6.84** [UI] Register the "MP3 quality" advanced-option DECLARATION against the P4 panel · §1.6 §2.9 · G47
  needs: P6.18.1
  > register the §1.6 option declaration (no new panel chrome — P4 owns it); Standard [default]; the V0/V2/V5 + CBR 128/192/320 preset set mapped by P6.18.1.
- [ ] **P6.85** [UI] Register the "WAV bit depth" advanced-option DECLARATION (16 / 24 / 32-float) · §1.6 · G47
  needs: P6.19
  > `16-bit [default]` · `24-bit (pcm_s24le)` · `32-bit float (pcm_f32le)`.
- [ ] **P6.86** [UI] Register the "FLAC compression" advanced-option DECLARATION (Fast 0 / Standard 5 / Best 8) · §1.6 · G47
  needs: P6.20
  > exposed cap is 8 (libFLAC native max; FFmpeg 9–12 are non-standard — do NOT surface); validation range `0..=8`.
- [ ] **P6.87** [UI] Register the "AAC quality" advanced-option DECLARATION (128/192/256 CBR) · §1.6 · G47
  needs: P6.21
  > no VBR exposed (encoder limitation); 192k [default].
- [ ] **P6.88** [UI] Register the M4A quality advanced-option DECLARATION (shares the AAC preset set) · §1.6 · G47
  needs: P6.22
  > 128/192/256 CBR; container difference (`.m4a` iTunes atoms) chosen automatically, not a user setting.
- [ ] **P6.89** [UI] Register the "OGG quality" advanced-option DECLARATION (q3/q5/q7) · §1.6 · G47
  needs: P6.23
  > Vorbis quality scale −1..10; expose the useful middle; q3 [default].
- [ ] **P6.90** [UI] Register the "OPUS bitrate" advanced-option DECLARATION (96/128/192) · §1.6 · G47
  needs: P6.24
  > 128k [default]; `-vbr on` retained throughout.
- [ ] **P6.91** [UI] Register the "AIFF bit depth" advanced-option DECLARATION (16 / 24) · §1.6 · G47
  needs: P6.25
  > `16-bit [default]` · `24-bit (pcm_s24be)`.

---

## Cross-phase reconciliation (the deferred P6→P4 harness `needs:`)

- [ ] **P6.92** [GATE] Wire the deferred P6→P4 harness reconciliation `needs:` edges — isolation boundary, §1.7 lifecycle/probe/progress/kill, per-pair runner + ledger + bijection, TCC staging, options-panel shell · §3.5.1 · G7 G20
  needs: P4.13, P4.14, P4.8, P4.9, P4.10, P4.11, P4.25, P4.30, P4.43, P4.49, P4.59, P4.60, P4.61, P4.64, P4.74, P4.28.1, P0.7.3, P0.7.4
  > the P6 instance of the cross-phase reconciliation obligation (the master plan-lint forbidden-string check is P4.77; reciprocal of P3.70/P5.74/P7.77/P9.46): declare the load-bearing P6→P4 + P6→P0 edges the FFmpeg-family boxes consume — the FFmpeg staging executes the **P0.7.3 engine-acquisition + allow-list policy** (P6.1) and the per-engine §6.1.3 assertions execute the **P0.7.4 build-assertion policy** (P6.3–P6.7); the from-source curated FFmpeg compile (P6.1.1) fills the **P4.28.1 from-source compilation harness** configure-flag manifest seam (the network-absent `--disable-network` build the P6.3/P6.6/P6.7 §6.1.3 assertions can only pass against — the prebuilt branch cannot); the FFmpeg beside-the-exe dynamic-library relocation (P6.1.2) drives the **P4.30 generic rpath/install_name rewrite mechanism** so the five component libs resolve inside the bundle (the post-relocation G37b closure asserted in P6.1.2); FFmpeg routes through the **P4.13/P4.14 §2.12 isolation wrapper** (P6.8); probe-then-encode + progress + classify + cancel/kill ride the **P4.8/P4.9/P4.10/P4.11/P4.49 §1.7 lifecycle** (P6.10–P6.13); macOS TCC staging is **P4.25** (P6.79); the per-engine availability row populates the **P4.43 verifier framework** (P6.14); every per-pair test runs on the **P4.59 §6.4.3 runner** (the audio per-output-format tests P6.32–P6.41, the video per-container tests P6.59–P6.63, the cross-category tests P6.76) and the phase gate drives the **P4.60 bijection guard + P4.61 ledger generator** (P6.80); every advanced-option DECLARATION box (the audio declarations P6.84–P6.91, the video P6.50.1, the extract-audio P6.69.1, the to-GIF P6.71.1/P6.71.2/P6.72.1) renders against the **P4.64 OptionsPanel widget dispatch + the P4.74 AdvancedDrawer**. `needs:` these P4 harness boxes here so the §6 selection builds the P4 mechanism first (P4 is `[x]` before the loop reaches P6 — the edges must RESOLVE, not dangle; the inline engine edges on P6.8–P6.80 carry the per-box dependency, this box is the auditable single owner). No P6 box `>`-note defers a `needs:` with the P4.77-forbidden phrasing.
