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

- **Decision tags:** `[DECIDED]` (fixed here / by the SSOT), `[OPEN]` (a genuine
  unresolved owner-level call — collected in the log below), `[DEFER: …]` (design is
  decided; only an empirical number or a real-world validation remains).
- **SSOT references** by section *name* (e.g. *Never harm the original*).
- Code/identifiers in English; this doc in English (public OSS repo).

## Parked decisions inherited from Phase 1 (the "how" seeds)

- **Framework:** Tauri (Rust core + React/TS/Tailwind/Vite UI). `[DECIDED]`
- **Engine delivery:** bundle **everything**, fully offline, no runtime fetch. `[DECIDED]`
- **Licensing mechanism:** copyleft engines shipped as **separate, independently
  invoked binaries** (aggregation, not linking) so the MIT core stays clean;
  NOTICE/third-party-licenses + SBOM. `[DECIDED]`

## Open-questions log

> Kept honest after the convergence pass. `[DECIDED]` = resolved (one-line
> rationale); `[DEFER: corpus]` / `[DEFER: …]` = the *design* is fixed and only an
> empirical number/validation remains; `[OPEN]` = a genuine unresolved owner-level
> call. After this pass the vast majority are decided or deferred.

### Resolved this convergence pass `[DECIDED]`
- **Name/trademark clearance verdict = `clear`** — both "ConvertIA" and the public
  "Ne-IA" brand cleared for v1; `docs/name-clearance.md` records it; the §6.9 gate
  (record present + current) is retained and the rename machinery stays dormant.
  Owner: §6.9.
- **HEIC/AAC/H.264 patent disposition** — **ship-bundled on all 3 platforms** (native
  LGPL AAC, x264, libde265 HEVC-decode), isolated per §3.6; the MP4-default-video
  dependency is honored. Owner: §3.4.
- **HEVC *encode* (write HEIC)** — **ship-bundled-isolated (x265), behind the §3.4
  availability flag** so it can flip to `unavailable` (SSOT exception-1) as a config
  change; **kvazaar (BSD)** recorded as the licence-clean alternative. Owner: §3.4.
- **AVIF** — ship-bundled all 3 (royalty-free). Owner: §3.4.
- **Rust↔TS type-sharing = tauri-specta** (+ specta), generated `bindings.ts`, §06
  drift check; specta-only is the documented fallback. Owner: §0.4.5.
- **Supported-OS floor** — Win10 1809+/11; macOS 11+; Ubuntu-22.04-LTS-class
  `libwebkit2gtk-4.1`; x86-64. (Exact build numbers `[DEFER: §6.4 drift matrix]`.)
  Owner: §0.3.1.
- **§0.10 capability allowlist** — **no `shell:allow-execute`** (engines spawn
  Rust-side §3.3.3); opener output-scoped + compiled-in project URL; `log:default` +
  `store:default` added. Owner: §0.10.
- **cancel-collect** — command-backed **C13 `cancel_ingest`** (ingest-scoped token);
  the §5.2 Collecting cancel control + §5.10 Esc back it. Owner: §0.4/§1.1/§5.
- **HEIC/AVIF encode code-path** — standardise on libvips `heifsave` (one AV1 encoder,
  libaom; standalone heif/avif dropped). Owner: images.md [OPEN-1] / §3.5.5.
- **GIF/BMP/ICO save path** — native `gifsave` (cgif, MIT) / `bmpsave` (libvips
  ≥ 8.12); ImageMagick `magicksave` fallback only. **ImageMagick is permissive (not
  GPL).** Owner: images.md / §3.5.5 / §3.6.1.
- **Ghostscript** — **dropped in v1** (poppler-only PDF→TXT, no AGPL). `[DEFER: re-add
  if corpus shows GS-salvageable PDFs]`. Owner: §3.1/§3.6.
- **Cross-session re-run ledger** — **not in v1** (session-only; signal 1 demoted to
  in-session corroborator only, §2.5.2). `[DEFER: post-v1 hashes-only ledger]`.
  Owner: §7.4/§2.5.
- **Persistence** — ship the **2-key prefs blob** (theme + lastDestinationMode), OS
  config dir. Owner: §7.4.
- **Logging** — ship the **local on-disk log + verbose opt-in** (privacy-by-default,
  no network). Owner: §7.5.
- **Instance hand-off while RUNNING** — **refuse-busy**. Owner: §7.1.
- **Engine integrity verification** — **hash-on-first-launch + cheap warm check**.
  Owner: §7.2.
- **Sign `SHA256SUMS`** — **yes, project minisign key** (manifest signature, not
  code-signing). Owner: §6.2.
- **CI runners** — **GitHub-hosted mac/win, self-hosted Linux for Lane A** (budget
  note retained). Owner: §6.1.
- **CI engine-acquisition** — **pinned, checksum-verified asset cache**. Owner: §6.1.
- **Corpus storage** — **small CC0/synthetic in-repo + LFS `corpus-large` for the
  full gate**; total size `[DEFER: corpus]`. Owner: §6.4.
- **Bundled-font baseline** — **Liberation + Carlito + Caladea + curated Noto CJK/RTL
  subset**; only CJK breadth `[DEFER: size]`. Owner: §3.9.3.

### Deferred to corpus / usability validation `[DEFER: corpus]`
> Design decided; only an empirical number or a real-world validation remains. These
> are **not** open design questions.
- **Resource budget numbers** — "too big" ceiling, memory/handle ceilings,
  per-category heuristics, **headroom margin 1.3×**, **GIF duration cap ~10 s** ship
  as finite starting values, tuned against the §6 corpus. Owner: §1.10 (co-owned
  §0.9 + cross-category [OPEN-F]).
- **Documents `MD→PDF`/`MD→ODT/DOCX` ownership** (LO 26.2 MD import unproven; default
  LO, pandoc fallback) and **`RTF→markup` ownership** (pandoc, LO fallback if too
  lossy). `DOC→markup` is already DECIDED LibreOffice. Owner: documents.md.
- **`*→MD` image policy** — drop-with-note (lean) vs data-URI inline. Owner:
  documents.md.
- **extract-audio target subset** (MP3★/M4A/WAV/FLAC/OGG; keep OGG?) and **"no audio
  track" up-front probe** (disable-with-reason vs offer-then-fail). Owner:
  cross-category [OPEN-A]/[OPEN-C].
- **to-GIF option scope** (trim: hard-cap / Basic start+duration / Advanced) and
  **default dither** (bayer-vs-sierra2_4a; bayer is the v1 default). Owner:
  cross-category [OPEN-D]/[OPEN-E].
- **Video HEVC-source default** (remux-verbatim vs re-encode-to-H.264; leaning
  re-encode default + remux as an Advanced "keep original quality"), **auto-
  deinterlace default** (yadif on for flagged-interlaced), and **MOV-as-target
  demand** — validate in §6.6. Owner: video.md.
- **Spreadsheets multi-sheet → CSV sheet selection** (active/first/picker; lean
  picker→active) and **XLSX default CSV-vs-PDF** — validate in §6.6. Owner:
  spreadsheets.md.
- **Images defaults to confirm vs corpus**: GPS/location-EXIF strip-vs-preserve;
  APNG-output vs first-frame-collapse (lean collapse); ICO non-square pad-vs-crop
  (lean pad); default Q values (JPG 82 / WEBP 80 / HEIC&AVIF 60); x265 `preset`
  slow-vs-medium for HEIC. Owner: images.md.

### Genuinely still open `[OPEN]` (owner-level, not yet resolvable)
- **Decoder-isolation v1 sandbox depth per OS** — the cheap tier (process + timeout +
  minimal-env + scratch-cwd) is non-negotiable v1; how far the privilege-drop tier
  (seccomp/Landlock / Seatbelt / Job-Object + low-integrity) goes is a real
  engineering/portability call. Owner: §2.12.
- **In-core text-encoding heuristic / Rust ZIP central-directory peek** — may it stay
  outside the §2.12 isolation boundary (lean: yes, memory-safe/bounded). Owner: §2.12
  (raised by §1.2).
- **libvips in-process vs separate image-worker process** — security/robustness
  isolation placement (lean: separate worker); licence analysis unaffected. Owner:
  §2.12/§0.9 (raised by §3.5.5).
- **macOS E2E driver under an unsigned build** — `tauri-driver`/`safaridriver`
  cannot cleanly drive an unsigned WKWebView; the macOS E2E may degrade to
  launch+screenshot, with the §6.6 human walkthrough carrying macOS core-flow
  validation. Owner: §6.4.6.
