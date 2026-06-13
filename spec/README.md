# ConvertIA — Technical Specification

> The complete technical specification for ConvertIA, derived from the
> [Single Source of Truth](../SINGLE-SOURCE-OF-TRUTH.md) (SSOT). The SSOT remains
> authoritative on **what & why**; this spec defines **how**.

## Status & rules of engagement

- **Living document.** Unlike the SSOT, this spec is expected to be refined and
  referenced *during* development — sections get adjusted as implementation
  reveals detail. The SSOT does **not** change for that; it stays the single
  source of truth.
- **Conflict rule:** if the spec ever contradicts the SSOT, the **SSOT wins** and
  the spec is corrected.
- **Derivation:** Phase 3 (the implementation TODO/plan) is derived from this
  spec, so it must be **complete** — every behaviour the SSOT promises has a
  technical home here.
- **Scope:** technical specification of the *software*. **Out of scope:**
  distribution/store logistics, developer accounts, code-signing/notarization
  processes (see SSOT *Explicitly Out of Scope*) — **except** where they impose an
  in-code requirement (e.g. generating an SBOM, producing release checksums).

## Structure / reading order

| # | File | Covers (SSOT origin) | Maps to A/B/C/D |
|---|------|----------------------|-----------------|
| 00 | [architecture](00-architecture.md) | System architecture, Tauri model, IPC, project layout, domain model, tech stack | **A** |
| 01 | [conversion-pipeline](01-conversion-pipeline.md) | Detection, queue, batch rules, job lifecycle, engine-invocation model, progress, cancellation | **B** |
| 02 | [guarantees](02-guarantees.md) | Implementation of the SSOT hard guarantees (no-harm, atomicity, fail-clearly, output destination, security/isolation) | **B** |
| 03 | [engines-and-bundling](03-engines-and-bundling.md) | Engine registry/selection, bundling (all offline), per-platform packaging, licence surfacing (NOTICE/SBOM) | **B** |
| 04 | [formats/](04-formats/README.md) | Per-category format matrix — detection, targets (both directions), engine, options, lossy notes | **C** |
| 05 | [ui-ux](05-ui-ux.md) | Frontend architecture, screen states, components, design system, accessibility, IPC integration | **D** |
| 06 | [build-test-release](06-build-test-release.md) | Build matrix, checksums/releases, SBOM, repo-policy artifacts, release gates, test strategy & real-world corpus | A+B+C+D (spans all) |
| 07 | [app-shell](07-app-shell.md) | ConvertIA as a running app: instance/run identity, lifecycle, persistence, logging, update posture | **A** |

_Legend — **A** Architecture & app shell · **B** Core engine & guarantees · **C** Format coverage · **D** UI (these are the Phase-1 A/B/C/D buckets; 06 spans all). **Read 00 and 07 together** — 07 is A-track foundational despite its file number._

## Conventions

- **Decision tags:** `[DECIDED]` (fixed here / by the SSOT), `[OPEN]` (needs a
  call — collected in the log below), `[DEFER]` (resolved during implementation).
- **SSOT references** by section *name* (e.g. *Never harm the original*).
- Code/identifiers in English; this doc in English (public OSS repo).

## Parked decisions inherited from Phase 1 (the "how" seeds)

- **Framework:** Tauri (Rust core + React/TS/Tailwind/Vite UI). `[DECIDED]`
- **Engine delivery:** bundle **everything**, fully offline, no runtime fetch. `[DECIDED]`
- **Licensing mechanism:** copyleft engines shipped as **separate, independently
  invoked binaries** (aggregation, not linking) so the MIT core stays clean;
  NOTICE/third-party-licenses + SBOM. `[DECIDED]`

## Open-questions log

> Running list of `[OPEN]` items surfaced while writing the spec; resolved items
> move to `[DECIDED]` with a one-line rationale.

### Patents & codecs
- **HEIC / AAC / H.264 patent disposition (umbrella)** — per format × platform: ship-bundled / gate / rely-on-OS / unavailable; MP4-as-default-video depends on H.264/AAC shipping on all 3 platforms. Owner: §3.4. `[OPEN]`
- **HEVC *encode* (writing HEIC) per-platform disposition** — x265 GPL + heaviest patents, never a default target; ship-bundled-isolated (REC) vs unavailable (SSOT-exception-1). Decide before the §6.5 corpus run. Owner: §3.4. `[OPEN]`

### Architecture & toolchain
- **Rust↔TS type-sharing mechanism** — tauri-specta (REC) vs ts-rs/specta + hand-written drift-checked command map; toolchain-maturity bet against pinned Tauri. Owner: §0.4 (type-sharing subsection §0.4.5). `[OPEN]`
- **Supported-OS floor** — exact min Windows build / macOS version / WebKitGTK version; product-support commitment, QA cost cross-cutting §6.1/§6.4. Owner: §0.3.1. `[OPEN]`

### Guarantees & resources
- **Decoder-isolation v1 sandbox depth per OS** — cheap tier (process + timeout + minimal-env + scratch-cwd) is non-negotiable; how far the privilege-drop tier (seccomp/Landlock / Seatbelt / Job-Object + low-integrity) goes, constrained by portable/no-install. Owner: §2.12. `[OPEN]`
- **Resource budget numbers** — absolute "too big" output ceiling, memory/handle ceilings, per-category size-heuristic constants, headroom margin (REC 1.3×), GIF duration cap (~10 s) + per-pixel heuristic; must be finite v1 values, tuned against the §6 corpus. Owner: §1.10, co-owned §0.9 + 04/cross-category [OPEN-F]. `[OPEN]`
- **In-core text-encoding heuristic / Rust ZIP central-directory peek** — may it stay outside the §2.12 isolation boundary (lean: yes, memory-safe/bounded). Owner: §2.12 (raised by §1.2). `[OPEN]`
- **Cross-session re-run ledger** — add a hashes-only on-disk EquivKey record (survives restart) vs strict persist-nothing (REC not v1). Owner: §7.4 / §2.5. `[OPEN]`
- **libvips in-process vs separate image-worker process** — security/robustness isolation placement (REC separate worker); licence analysis unaffected. Owner: §2.12 / §0.9 (raised by §3.5.5). `[OPEN]`
- **Bundled-font-set contents** — Liberation/Carlito/Caladea + CJK/RTL Noto breadth vs binary size (the SC-vs-all-CJK weight knob); shared by documents/spreadsheets/presentations. Owner: §3.9.3. `[OPEN]`

### App shell
- **Instance & run identity** — single-instance + hand-off and the InstanceId/RunId model are DECIDED; remaining: second-launch hand-off **while a batch is RUNNING** — queue-after-current vs refuse-with-busy (REC refuse-busy). Owner: §7.1. `[OPEN]`
- **Engine integrity verification** — hash-every-engine-every-launch vs hash-once-then-cache-marker (startup latency for the heavy office engine vs assurance; REC hash-on-first-launch + cheap warm check). Owner: §7.2 with §3.3. `[OPEN]`
- **Persistence** — ship the minimal 2-key prefs blob (theme + lastDestinationMode; REC) vs strict zero-persistence in v1; and prefs file location OS-config-dir (REC) vs beside-binary. Owner: §7.4. `[OPEN]`
- **Logging** — ship a local on-disk log at all + the verbose-mode opt-in for full-path/command-line capture (REC yes to both, privacy-by-default). Owner: §7.5. `[OPEN]`

### Formats (04)
- **extract-audio target subset** — proposed MP3★/M4A/WAV/FLAC/OGG; whether to keep OGG, and the AAC/M4A patent flag routes to §3.4. Owner: 04-formats/cross-category [OPEN-A]. `[OPEN]`
- **extract-audio "no audio track" up-front probe** — disable-target-with-reason vs offer-then-fail (cost vs UX on large recursive batches). Owner: 04-formats/cross-category [OPEN-C]. `[OPEN]`
- **to-GIF option scope** — trim window: hard-cap-only / Basic start+duration / Advanced (REC Basic start+duration); plus default dither bayer-vs-sierra2_4a. Owner: 04-formats/cross-category [OPEN-D]/[OPEN-E]. `[OPEN]`
- **Video HEVC/H.265 default disposition when source is already H.265** — remux verbatim (lossless, less compatible) vs re-encode to H.264 (lossy, plays-everywhere; leaning re-encode default + remux as Advanced "keep original quality"). Owner: 04-formats/video. `[OPEN]`
- **Video auto-deinterlace default** (yadif on for flagged-interlaced MPEG-2) and MOV-as-an-offered-target-at-all — validate in the §6.6 usability walkthrough. Owner: 04-formats/video. `[OPEN]`
- **Spreadsheets multi-sheet → CSV sheet selection** — active-sheet vs first-sheet vs user-picker (lean picker defaulting to active); and XLSX default target CSV-vs-PDF (validate in §6.6). Owner: 04-formats/spreadsheets. `[OPEN]`
- **Documents MD→PDF / MD→ODT/DOCX engine ownership** (LibreOffice 26.2 MD import unproven) and DOC/RTF→markup ownership (pandoc can't read .doc; RTF reader gaps) — single-engine, no chaining; needs corpus validation. Owner: 04-formats/documents. `[OPEN]`
- **Documents Ghostscript bundling** — drop in v1 (REC, poppler-only PDF→TXT) vs keep AGPL backstop; and *→MD image policy (drop-with-note vs data-URI inline). Owner: 04-formats/documents / §3.1. `[OPEN]`
- **Images metadata/privacy & encode paths** — strip GPS/location EXIF by default vs preserve-all + Advanced toggle; APNG output vs first-frame collapse; ICO non-square pad-vs-crop; HEIC/AVIF encode code-path (vips heifsave vs standalone heif/avif); confirm default Q values (JPG 82 / WEBP 80 / HEIC&AVIF 60) against the corpus. Owner: 04-formats/images. `[OPEN]`

### Build / test / release (06)
- **Name/trademark clearance VERDICT** for "ConvertIA" and the public "Ne-IA" brand (clear vs rename vs abort) — SSOT-mandated, release-blocking legal/branding judgement, no recommendation. Owner: §6.9 [OPEN-6.9a]. `[OPEN]`
- **Sign SHA256SUMS with a project minisign/GPG key** — strengthens the no-code-signing trust substitute (REC yes). Owner: §6.2 [OPEN-6.2a]. `[OPEN]`
- **macOS/Windows CI runners** — GitHub-hosted (Actions-minute spend vs hobby/no-paid-upgrades budget) vs self-hosted (REC GitHub-hosted for mac/win, self-hosted Linux for Lane A). Owner: §6.1 [OPEN-6.1e]. `[OPEN]`
- **CI engine-acquisition mechanism** — vendored-LFS vs pinned checksum cache (REC) vs build-from-source; co-owned with the §3.9 size budget. Owner: §6.1 [OPEN-6.1d]. `[OPEN]`
- **Real-world corpus storage/size** — in-repo CC0/synthetic (REC) vs LFS corpus-large for the full gate; total size co-owned with §3.9. Owner: §6.4 [OPEN-6.4a]. `[OPEN]`
