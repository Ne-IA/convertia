# 06 — Build, Test & Release

> The technical build/test/release pipeline (software-side only — no store/account
> logistics). Origin: SSOT *v1 Definition of Done*, *Distribution & download
> trust*, *Cross-platform, one product*, *Local/private/offline*, *Engine-license
> policy*.
>
> **This section spans all four tracks (A+B+C+D).** It does not re-own pipeline,
> guarantee, engine or UI content — it owns the **process** that turns the codebase
> into verified, attributed, downloadable artifacts and the **gates** that decide
> a release is allowed. Where another section owns a fact, it is **referenced by
> §number**, never restated:
> - The engine inventory, per-platform packaging, the patent matrix and the
>   NOTICE/SBOM **data generation** are owned by §3 (§3.1, §3.3, §3.4, §3.6, §3.7,
>   §3.8, §3.9). This file owns *when* those run in CI and *that* they gate a release.
> - The hard guarantees under test (no-harm, atomicity, fail-clearly, isolation)
>   are owned by §2. This file owns the **test harness** that proves them.
> - The IPC surface under integration test is owned by §0.4; the Rust↔TS type
>   drift-check tooling is owned by §0.4.5; this file runs the check in CI.
> - The per-pair format facts (engines, defaults, lossy flags) are owned by §04;
>   this file consumes them as the **conversion matrix under test**.
> - The UI flow under the usability-floor walkthrough is owned by §5; the
>   instance/run/log model the tests rely on is owned by §7.

Decision tags: `[DECIDED]` (fixed here or by the SSOT), `[OPEN]` (needs an
owner-level call — fed to the README open-questions log), `[DEFER]` (resolved
during implementation). Recommended defaults for easy `[OPEN]`s are marked
**(recommendation)**.

---

## 6.1 Build matrix `[DECIDED — native-per-platform; artifact formats recommended]`

### 6.1.1 The hard constraint: no meaningful cross-compilation

Tauri links the **native system WebView** (WebView2 / WKWebView / WebKitGTK,
§0.3.1) and ConvertIA bundles **native engine binaries per platform** (FFmpeg,
LibreOffice, libvips, poppler, pandoc, … — §3.1/§3.3). Both make cross-compilation
impractical: Tauri's own guidance is that cross-compiling desktop bundles is not
supported in practice, and our copyleft engines ship as **separate per-OS
binaries** that must themselves be obtained/built for each target. **Decision:
build each platform on its own native CI runner.** No cross-compile; the matrix is
three independent native legs that fan in to one release. This is the documented
Tauri-recommended path (compile per platform on CI).

### 6.1.2 The artifact-per-platform table

One product, **one primary artifact per platform** (SSOT *Cross-platform, one
product*). SSOT *Portable, no installation* / *no system pollution* makes the
**portable / no-installer** variant the canonical download where one exists; an
installer variant is offered only where it is the platform norm and does not
require admin rights for the user to run.

| Platform | Tauri bundle target(s) | Canonical download (portable-first) | Notes |
|----------|------------------------|-------------------------------------|-------|
| **Windows x64** | `nsis` (+ optionally the raw `.exe`) | **Portable single `.exe`** is the canonical "download, run, done" artifact. NSIS is offered as a convenience installer that can run **per-user / no-admin** (`installMode: currentUser`). | MSI (`wix`) is **not** used — it implies a system install / admin. `[OPEN-6.1a]` whether to ship NSIS at all vs portable-exe-only. **(recommendation: ship the portable `.exe` as primary, NSIS per-user as a secondary convenience.)** |
| **macOS (universal)** | `app` (inside) → `dmg` | **`.dmg`** containing a **universal** `ConvertIA.app` (arm64 + x86_64 via `--target universal-apple-darwin`). | One universal artifact covers Apple-Silicon and Intel → honours "one product per platform". Unsigned/unnotarized (SSOT *Out of Scope*) → first-launch Gatekeeper friction is documented on the download page (§6.2.4) and About (§5.9). |
| **Linux x64** | `appimage` (+ optionally `deb`) | **AppImage** — the portable, distro-agnostic, no-install, runs-anywhere artifact (matches SSOT portability best). | `.deb`/`.rpm` are distro-specific *installs* (system pollution); they are **secondary at most**. `[OPEN-6.1b]` whether to also publish a `.deb`. **(recommendation: AppImage-only for v1; revisit `.deb` by demand.)** |

ARM Windows and ARM Linux are **out of v1** (SSOT platform scope = Win/macOS/Linux
desktop; no commitment to every CPU arch). `[OPEN-6.1c]` Linux arm64 / Windows
arm64 — deferred, low demand. The supported-OS floor (minimum Windows/macOS/distro
versions, WebView availability) is **owned by §0.3.1** and referenced by the
release notes; it is not re-decided here.

### 6.1.3 How engines bundle per platform (process, not policy)

§3.3 owns the bundling model and §3.4 the patent matrix; **this file owns the
build-time mechanics that realise them**:

- Copyleft engines (FFmpeg, LibreOffice, poppler, pandoc, Ghostscript-if-shipped)
  are **separate invoked binaries** (§3.6). They are placed under
  `src-tauri/binaries/` (sidecars) and/or `src-tauri/resources/` (engine support
  trees like the LibreOffice program dir + the bundled font set, §3 / documents.md
  fonts `[OPEN]`), and declared in `tauri.conf.json`:
  - **Sidecars** → `bundle.externalBin`. Tauri requires each sidecar to exist as
    `name-<target-triple>[.exe]` (e.g. `ffmpeg-x86_64-pc-windows-msvc.exe`,
    `ffmpeg-aarch64-apple-darwin`); a small build script (`scripts/stage-engines.*`,
    run before `tauri build`) stages and target-triple-suffixes each binary for the
    runner's host triple. For the macOS **universal** build, Tauri's externalBin
    naming requires **BOTH** `<name>-aarch64-apple-darwin` **AND**
    `<name>-x86_64-apple-darwin` files to be present (each *may* itself be a fat
    Mach-O, but there is **no** `<name>-universal-apple-darwin` slot — Tauri expects
    the two per-arch suffixed files, not one merged-universal one). `scripts/stage-
    engines.*` must therefore stage **two files per sidecar** on the macOS leg.
  - **Engine support files** (non-executable: LibreOffice's `share/`, `program/`
    libs, fonts, pandoc data) → `bundle.resources`, resolved at runtime via the
    Tauri resource path (§3.5 owns the working-dir/env wiring; §7.2 owns startup
    presence-verification of these files).
- The whole engine set is **vendored into the build inputs** — never fetched at
  runtime (SSOT offline floor) and, per the supply-chain stance (§6.3.4),
  **pinned by version + checksum**, ideally not fetched at build time from a live
  network either (a local/cached engine artifact store). **`[DECIDED]` (adopting the
  [REC]): a pinned, checksum-verified engine-asset cache keyed by engine version**,
  **not** committed raw into Git (avoids bloating the repo) and **not** built from
  source per-release (too slow). The **size budget** this implies is owned by §3.9.
  (Build-from-source remains a documented fallback if a pinned artifact becomes
  unavailable.)
- A platform's artifact ships **only the engines available on that platform per
  §3.4**. A patent-gapped engine (e.g. an HEVC encoder absent on a platform) is
  simply not staged there; the affected target is surfaced as unavailable in the UI
  (§5.2, sourced from §3.4) — **never a silent omission** (SSOT *v1 DoD* exception 1).
- **LGPL dynamic-link assertion (§3.6.1 build rule):** the stage step verifies the
  LGPL libraries (libvips, libheif, libde265, librsvg, any linked FFmpeg libs) are
  present as **bundled shared objects** (`.so`/`.dylib`/`.dll`) beside the binary —
  not statically absorbed into a single MIT executable — so the LGPL §6 relinkability
  path holds. A static LGPL link is a **build failure** (it would taint the MIT core).

### 6.1.4 CI runners

| Leg | Runner | Toolchain installed | Platform-specific deps |
|-----|--------|---------------------|------------------------|
| Windows | `windows-latest` (x64) | Rust (MSVC host triple), Node + pnpm | WebView2 is preinstalled on supported Windows; **not** bundled (no-network forbids downloading it at runtime — §0.3.1 owns the floor). NSIS provided by tauri-cli. |
| macOS | `macos-latest` (Apple Silicon) building `universal-apple-darwin` | Rust with **`rustup target add aarch64-apple-darwin x86_64-apple-darwin`** (both targets — prerequisite for the universal build and for staging both per-arch sidecar files, §6.1.3), Node + pnpm | Xcode CLT for `lipo`/codesign-less packaging. No notarization step (out of scope). |
| Linux | `ubuntu-latest` (pin a specific LTS for glibc floor stability) | Rust, Node + pnpm | `libwebkit2gtk-4.1-dev`, `libappindicator`, `librsvg2-dev`, `patchelf`, `libfuse2` (AppImage). **glibc of the build image sets the minimum Linux version** — pin an older Ubuntu LTS to maximise compatibility; documented in §0.3.1's floor. |

The platform CI standard (`reference_self_hosted_ci_runner.md`) runs a **self-hosted
VPS runner** for the Ne-IA org's existing four projects. ConvertIA's build matrix
**cannot** reuse a single Linux VPS runner for all three legs (no native macOS/Windows
there). **`[DECIDED]` (adopting the [REC]): GitHub-hosted runners for the
macOS/Windows legs; the self-hosted Linux runner for the Linux leg + the Lane-A
lint/test gate.** Rationale: matches upstream Tauri guidance, and release builds are
**infrequent** (one-large-all-or-nothing v1, SSOT) so Actions-minute spend is
bounded. **Budget note (kept visible):** GitHub **macOS**-hosted minutes bill ~10×
Linux/min — relevant to the hobby/no-paid-upgrades budget
(`user_hobby_budget_no_paid_upgrades.md`); the infrequent-release cadence keeps it
within free-tier/affordable bounds, and the Linux leg (the frequent Lane-A path)
stays on the free self-hosted runner. Revisit only if release cadence rises.

---

## 6.2 Reproducibility & integrity `[DECIDED — published hashes; reproducibility = best-effort intent]`

### 6.2.1 Why this matters (SSOT)

Signing/notarization is **deliberately out of scope** (SSOT *Out of Scope*), so the
**stated trust substitute** is *published integrity hashes from one canonical
location*. This is the only protection a user has that a download is authentic, so
it is treated as a **release gate**, not an afterthought.

### 6.2.2 Canonical location `[DECIDED]`

Releases are published **only** from the **Ne-IA org's GitHub Releases**
(`github.com/Ne-IA/convertia/releases`) — the single source of authentic builds
(SSOT *Distribution & download trust*). No mirror, no third-party host is endorsed.
The README/download page states this explicitly.

### 6.2.3 Hashes & manifest `[DECIDED]`

For **every** published artifact:
- A **SHA-256** is computed in CI immediately after the artifact is built (before
  upload) and published as:
  - a per-file `<artifact>.sha256` sidecar, **and**
  - a single `SHA256SUMS` manifest covering **every release asset** (the familiar
    `sha256sum -c SHA256SUMS` workflow). "Every asset" means the platform binaries
    **and** the SBOM (CycloneDX/SPDX), `NOTICE`/`THIRD-PARTY-LICENSES.txt`, and
    `reliability-report.json` — so a user can verify the attribution and
    reliability artifacts too, not just the executable. (`SHA256SUMS` itself is the
    only asset it cannot list; the optional minisign signature §6.2.3a covers it.)
- **`[DECIDED]` publish a project minisign detached signature over `SHA256SUMS`.**
  This is *not* code-signing the binary (out of scope) but signing the *checksum
  manifest* — it closes the "attacker replaces both the artifact **and** its hash"
  gap that bare checksums leave open, at near-zero cost and entirely within
  "no store/cert" scope. The minisign **public key lives in the repo**; the verify
  recipe (§6.2.4) includes `minisign -Vm SHA256SUMS -P <pubkey>`. (Adopting the
  [REC] — a clear strengthening of the SSOT trust substitute with no downside.)

### 6.2.4 How a user verifies (must be surfaced) `[DECIDED]`

The download page and README give a copy-paste verification recipe **at the
highest-risk moment** (SSOT):
- Windows (PowerShell): `Get-FileHash .\ConvertIA.exe -Algorithm SHA256` → compare
  to the published value.
- macOS/Linux: `shasum -a 256 ConvertIA.dmg` / `sha256sum ConvertIA.AppImage`, or
  `sha256sum -c SHA256SUMS`.
- If §6.2.3's signature ships: `minisign -Vm SHA256SUMS -P <pubkey>`.
The page also restates the **as-is / no-warranty / best-effort-security** posture
(SSOT *License & Openness*), and the unsigned-build first-launch friction per OS
(Gatekeeper on macOS, SmartScreen on Windows) so a normal user isn't surprised.

### 6.2.5 Reproducible-build intent `[OPEN — best-effort, not a gate]`

Full bit-for-bit reproducibility across the Rust+WebView+vendored-engine artifact
is **hard** (timestamps, build-paths, per-runner toolchain drift, the prebuilt
engine binaries we don't compile ourselves). v1 stance: **reproducibility is a
best-effort intent, explicitly NOT a release gate** (mirrors the SSOT
engine-currency "best-effort, not a gate" posture). Cheap measures we *do* take:
pinned toolchains (§0.8), pinned engine versions+checksums (§3.8/§6.1.3),
`SOURCE_DATE_EPOCH` where the toolchain honours it, and recording the exact
toolchain/engine versions in the SBOM so a build is at least **auditable** even if
not bit-reproducible. `[OPEN-6.2b]` how far to pursue determinism — deferred.

---

## 6.3 SBOM & licence artifacts `[DECIDED — attribution is a release gate]`

> §3.7 **owns the generation** of the NOTICE/third-party-licenses **data** and the
> SBOM source. §5.9 **displays** the NOTICE in-app. **This file owns** the CI
> assembly step and the **completeness gate**: a missing or incorrect attribution
> is **release-blocking — same status as the no-harm guarantee** (SSOT *v1 DoD*).

### 6.3.1 What "the SBOM" actually covers (two layers)

ConvertIA's bill of materials is **not** just its Rust crate graph — the
load-bearing licence risk is the **bundled engine binaries** (FFmpeg LGPL,
LibreOffice MPL, poppler/pandoc/Ghostscript GPL/AGPL, libvips LGPL, x265 GPL, …).
So the SBOM is assembled in **two layers**:

| Layer | Contents | Tool |
|-------|----------|------|
| **App dependency graph** | Rust crates (`Cargo.lock`) + JS deps (`pnpm-lock.yaml`) that compose ConvertIA's own MIT code | **`cargo cyclonedx`** for Rust; an npm/pnpm CycloneDX generator for the frontend; merged into one CycloneDX document. |
| **Bundled engines (the important layer)** | Every separately-invoked engine binary + its support libs/fonts, each as an SBOM component with **name, version, licence (SPDX id), source URL, and the per-platform availability** | A **manually-maintained `engines.lock` manifest** (owned/sourced by §3.1/§3.8) is the authoritative input; CI converts it into CycloneDX components and merges with the dependency-graph layer. Optionally **Syft** scans the staged bundle to *cross-check* that nothing in the shipped tree is missing from the manifest (drift detection). |

Output format: **CycloneDX JSON** as the canonical SBOM (developer-friendly,
good licence+component fidelity); a **CycloneDX→SPDX** export is generated too if a
consumer needs the ISO-standard form. Both are release assets.

### 6.3.2 NOTICE / third-party-licenses assembly

- The repo `NOTICE` (and a longer `THIRD-PARTY-LICENSES.txt`) is **generated from
  the same `engines.lock` + dependency SBOM** so it can never silently drift from
  what actually ships. It contains, per bundled engine: the engine name+version,
  its full licence text, and — for **GPL/LGPL/AGPL** engines — the required
  **written offer of source** (where the corresponding source can be obtained:
  the pinned upstream tag + our build recipe), honouring §3.6's copyleft
  obligations.
- The in-app About screen (§5.9) presents this listing; the file in the repo and
  the in-bundle copy are the **same generated artifact** (one source of truth).

### 6.3.3 The completeness gate (release-blocking) `[DECIDED]`

CI runs an **attribution-completeness check** and **fails the release** if it does
not pass. Concretely:
1. **Every** binary/resource staged into the bundle (§6.1.3) maps to a component in
   `engines.lock` (Syft cross-check from 6.3.1 — *no shipped file without an SBOM
   entry*).
2. **Every** SBOM component with a copyleft licence (GPL/LGPL/MPL/AGPL) has its
   licence text present in `THIRD-PARTY-LICENSES.txt` **and** (for GPL-family) a
   written-offer-of-source entry.
3. **No** component is missing a resolved SPDX licence id (an `UNKNOWN`/`NOASSERTION`
   licence is a hard fail — forces a human to classify it before ship).
4. **No** engine whose licence is *incompatible with inbound-MIT-clean distribution
   as a separate binary* slipped in (policy: copyleft is fine **as an aggregated
   separate binary**; anything that would taint the MIT core via linking is rejected
   — this is a guardrail on §3.6, surfaced as a CI assertion).
This check is part of the **release pipeline** (§6.7), not the per-PR fast lane, and
its failure blocks artifact publication exactly like a failed no-harm property test.

### 6.3.4 Supply-chain hygiene (the bundled-binary surface)

Per §0.11's threat map (*bundled-binary supply chain → §3.8/§6.3*): the pinned
engine versions+checksums (§6.1.3) are verified at stage time; `cargo audit` /
`cargo deny` run in CI over the Rust graph (advisory + licence-policy enforcement,
non-release-blocking advisory-wise but licence-policy-blocking). Engine
**currency** (keeping decoders patched) is a **best-effort posture, not a gate**
(SSOT) — owned by §3.8; this file only ensures a bumped engine is re-validated
against the corpus (§6.4/§6.5) before it can ship.

---

## 6.4 Test strategy `[DECIDED]`

The DoD bar — *"works reliably"* = passes **fail-clearly (§2.8)** and **no-harm
(§2.5)** on a **representative real-world corpus** — is operationalised as a layered
test suite. Layers, from cheapest/most-frequent to most-expensive:

### 6.4.1 Unit tests — Rust core (the guarantees layer)

Pure-logic tests on the §0.7 modules, **no engines, no real FS where avoidable**:
- **Output naming contract (§2.2):** base-name-kept + target-extension; `(1)`/`(2)`
  numbering before the extension; never hashed / `_converted`; path-limit →
  fail-clearly (no truncation). Property-tested with adversarial names (Unicode,
  emoji, RTL, spaces, dots, max-length).
- **No-clobber & resolved identity (§2.1/§2.3):** exclusive-create semantics;
  symlink/junction/hardlink resolution; de-dup of the frozen set by resolved
  identity; refusal to write through a link onto a frozen source.
- **Frozen source set (§2.4):** files appearing after the freeze are never ingested;
  outputs in a source folder don't expand the batch.
- **Re-run/equivalence (§2.5):** equality on (resolved source + target + effective
  settings); safe fallback to silent numbering when undeterminable.
- **Detection (§1.2):** magic-byte classification table — every signature in §04
  (JPEG SOI, PNG, RIFF/WEBP, EBML DocType matroska-vs-webm, ISO-BMFF `ftyp`
  brand, OLE2-stream disambiguation DOC/XLS/PPT, ZIP-OPC content-type
  DOCX/XLSX/PPTX/ODF-mimetype, ADTS-vs-MP3, Ogg codec-id Vorbis-vs-Opus, ASF GUID
  WMA-vs-WMV, text/CSV-TSV delimiter sniff) gets a fixture asserting the correct
  user-facing type, **including misnamed-extension fixtures** (`.jpg` that is PNG;
  `.m4a` that is ALAC) and the "detected-but-unsupported" / "uncertain" outcomes.
- **Batch grouping (§1.3):** one-source-format-per-batch; mixed-drop pre-flight
  refusal lists the found formats; cross-category targets attach to the *source*.
- **Target resolution (§1.5) + defaults registry (§1.6):** for **every** source
  format in §04, exactly **one** pre-highlighted default; the "no required choices"
  invariant (drop → default → convert needs zero clicks) is asserted against the
  consolidated defaults registry §1.6 may own.
- **Error taxonomy mapping (§2.8/§2.13):** each failure kind maps to its catalog
  string; worker-thread panic boundary (`catch_unwind`) surfaces a clean per-item
  failure, not a poisoned pool.

### 6.4.2 Property / fault-injection tests (no-harm + fail-clearly, hardened)

These directly defend the SSOT hard promises and run with a real (temp) filesystem
and stub/real engines:
- **Atomicity under interruption (§2.1):** a conversion killed mid-write (simulated
  crash/force-quit/cancel) **never** leaves a truncated visible file — only a
  discardable temp artifact, cleaned on next run (§2.6). Cross-volume path
  (source on USB → output in Downloads, §2.14) exercised: copy→fsync→atomic-rename
  *within* the destination volume.
- **No-harm fuzz:** randomized batches over the corpus assert **source bytes are
  byte-identical before/after** every run (originals never touched), including the
  same-source==same-target re-encode case (§2.1) and the divert/fallback path
  (§2.7) (guarantees hold identically there).
- **Out-of-disk / too-big (§1.10/§2.8):** a constrained-FS harness proves the item
  fails fast+clearly, the batch continues, and free space returns to ~baseline
  (§2.6); a cleanup that itself fails is **never** reported as a clean success.
- **Malformed/adversarial inputs (§2.12/§2.13):** truncated, 0-byte, fuzzed-header,
  encrypted/DRM (password PDF/XLSX/PPTX, FairPlay M4V, PlaysForSure WMV), and
  decompression-bomb-shaped inputs each produce **one plain message**, no crash, no
  app wedge, batch continues. The decoder runs inside the §2.12 isolation boundary;
  these tests verify a hanging/crashing engine fails **one** item.
- **Cancellation (§1.7/§1.11):** mid-batch cancel keeps finished items, discards the
  in-flight one with no partial leftover, never touches originals.

### 6.4.3 Integration tests — per-pair conversions (the real engines)

The heart of the reliability gate (§6.5). For **every** (source→target) pair
enumerated across §04 (the matrices in images/audio/video/documents/spreadsheets/
presentations + cross-category extract-audio/to-GIF), against the §6.4.5 corpus:
- the conversion **completes** with exit success and produces a **valid file of the
  target format** (validated by re-detecting the output's magic bytes via §1.2, and
  a format-appropriate structural check — e.g. `ffprobe` the audio/video output is
  decodable; the image decodes and has expected dimensions; `pdftotext`/poppler can
  open the produced PDF; the CSV round-trips field counts);
- the **content-fidelity** spot-checks pass: CJK/RTL text survives doc/sheet/slide
  conversions (§2.10); image orientation is baked upright (§04 images); audio
  tags/cover-art round-trip where the target supports them (§04 audio); video
  remux-vs-reencode chose the lossless path when codecs already fit (§04 video);
- the **lossy disclosure** is asserted to fire **iff** the pair is flagged lossy in
  §04 (and, for video, based on the *planned* remux-vs-reencode disposition, not the
  static pair) — the §2.9 catalog string is shown for exactly the flagged cases.
- **Patent-gapped pairs (§3.4):** on a platform where §3.4 marks a target
  unavailable, the integration test asserts the target is **absent/disabled** (not
  attempted) rather than failing — honest unavailability, not a test failure.

#### 6.4.3a Corpus↔pair bijection guard (the non-circular gate, made concrete) `[DECIDED]`

SSOT §9 makes "the corpus exists and backs every pair" a precondition; this is the
**machine-checkable** form (the check §6.10 row 3 names but no section previously
specified). A CI script (`scripts/check-corpus-coverage.*`, run in **Lane A**
§6.7.1 — cheap, no engines) asserts a **bijection** between the §04 pair matrices
and the corpus `manifest.toml`:

1. **Enumerate every v1-required `(source → target)` pair** from the §04 matrices
   (images/audio/video/documents/spreadsheets/presentations + the two cross-category
   ops), excluding diagonals/`out`/`—` cells and pairs §3.4 marks `unavailable` on
   *all* platforms.
2. **Union the `covers` lists** from every corpus `manifest.toml` entry.
3. **Fail CI if any required pair has zero backing corpus files** (a pair with no
   `covers` entry) — *and* fail if any `covers` entry names a pair that does **not**
   exist in the §04 matrices (a stale/typo'd coupling). Both directions of the
   bijection are checked, so the gate cannot rot.

This is what makes the §6.5 reliability gate **non-circular**: a pair literally
cannot be declared `reliable` without a corpus file whose `covers` list names it.

### 6.4.4 Cross-platform test runs

The integration + property suites run on **all three native CI legs** (§6.1.4) —
the reliability bar is *per-platform* (SSOT: "on all three platforms"). Additional
platform-specific concerns:
- **WebView rendering drift (§0.3.1):** a light UI smoke test (the §6.4.6
  Playwright/WebDriver flow) runs on each platform to catch WebView2/WKWebView/
  WebKitGTK layout/behaviour differences in the core flow.
- **macOS TCC** file-access prompts that the beside-source default can trigger
  (§7.2) are exercised in the macOS leg's headed smoke run.
- **LibreOffice headless is NOT safely parallel** (§0.9) — the office-pair
  integration tests must run LibreOffice **serialized**; the harness honours the
  §0.9 concurrency-degree config so the test environment matches production.

### 6.4.5 The real-world input corpus (concrete contents) `[DECIDED — required v1 asset]`

The corpus is a **required v1 asset and a precondition for declaring any pair done**
(SSOT) — without it the reliability gate is circular. It lives in the repo (or an
LFS/release-asset store if size demands) under `tests/corpus/`, **organised by
source format**, with a `manifest.toml` recording for each file: source format,
provenance/licence (corpus files must themselves be redistributable — public-domain
/ CC0 / self-produced / synthetic), the **properties it is chosen to exercise**, the
**expected outcome** per target (success / specific fail-clearly kind / specific
lossy note), and a **`covers` list** — the explicit `(source, target)` pairs this
file backs (the coupling field that makes the §6.4.3a bijection guard machine-
checkable). Manifest shape (per file):

```toml
[[file]]
path     = "images/iphone_p3_orientation6.heic"
source   = "HEIC"
licence  = "CC0"          # must be redistributable
exercises = ["orientation-bake", "ICC-P3", "HDR-10bit"]
covers   = [              # the (source→target) pairs this file backs (§6.4.3a)
  ["HEIC", "JPG"], ["HEIC", "PNG"], ["HEIC", "WEBP"], ["HEIC", "AVIF"],
]
[file.expect]             # expected outcome per target
"HEIC→JPG"  = { result = "success", lossy = "image_lossy_codec" }
"HEIC→AVIF" = { result = "success", lossy = "image_lossy_codec" }
```

Concrete required contents:

**Images** (`tests/corpus/images/`)
- Real **iPhone HEIC** photos (HDR, 10-bit, with EXIF orientation tags 1/3/6/8, GPS,
  ICC Display-P3) — the canonical HEIC→JPG everyday case + orientation-bake + ICC.
- **JPEG** with EXIF orientation, progressive, CMYK, 12-bit, truncated-tail.
- **PNG** RGBA, 16-bit, palette, **APNG** (animation-collapse case).
- **WEBP** lossy, lossless, **animated**, with alpha.
- **AVIF** still + **animated (`avis`)**, HDR/wide-gamut.
- **GIF** static + multi-frame **animated** (first-frame-collapse + passthrough).
- **TIFF** multi-page, 16-bit, CMYK, big-endian; **BMP** 24/32-bit, top-down/bottom-up.
- **ICO** multi-resolution (16/32/48/256) + non-square (padding case).
- **SVG** with intrinsic size, viewBox-only, missing-font, `.svgz`, a **remote
  `<image href>`** (must NOT be fetched — offline/security assertion), and a
  pathological tiny-viewBox-huge-render (must fail-clearly, not OOM).

**Audio** (`tests/corpus/audio/`)
- One file per source format (MP3 incl. VBR/CBR + ID3v2 + cover art; WAV 16/24/float;
  FLAC with Vorbis comments + cover; raw-ADTS `.aac`; **M4A holding AAC** *and* a
  separate **M4A holding ALAC** — the detection-by-codec case; OGG-Vorbis;
  `.opus`; AIFF; WMA v2/Pro/Lossless).
- Multichannel (5.1) source (channel-preservation, no silent downmix).
- A **>16-bit source** (bit-depth-reduction-to-default-16-bit disclosure case).
- Files with **non-Latin/CJK/RTL tag text** (tag fidelity, §2.10).
- Corrupt/truncated + 0-byte + a `.mp3` that is really FLAC (mislabel) cases.

**Video** (`tests/corpus/video/`) — short clips (a few seconds) to keep runtime sane:
- **MP4 (H.264+AAC)** → the lossless-remux baseline; **MOV from iPhone (HEVC)** →
  the §04 HEVC-default `[OPEN]` case; **MKV** with **multiple audio tracks + SRT +
  ASS + PGS subtitles + chapters + font attachments** (the keep/convert/drop policy);
  **WEBM (VP9+Opus, and a VP8 alpha clip)**; legacy **AVI (DivX+MP3)**, **WMV
  (VC-1+WMA)**, **FLV (H.264/AAC and old Sorenson)**, **MPG (interlaced MPEG-2 +
  AC-3 — deinterlace case)**, **M4V (DRM-free)**, **3GP (H.263+AMR-NB)**.
- A **DRM-protected FairPlay `.m4v`** and a DRM WMV (must fail-clearly).
- A **portrait/rotated** clip (rotation honoured); a **VFR screen recording**
  (to-GIF fps-normalise); a **silent** clip (extract-audio "no audio track" case);
  a long-ish clip to exercise the to-GIF guardrail/cap (§cross-category).

**Documents** (`tests/corpus/documents/`)
- **DOCX/DOC/ODT/RTF** real-world samples incl. **non-Latin (CJK) + RTL (Arabic/
  Hebrew)** body text, embedded images, a doc referencing a **non-bundled font**
  (substitution/reflow case), tracked-changes, a macro-enabled `.docm` (macro must
  drop, never execute).
- **PDF**: a text PDF (→TXT extraction), a **scanned/image-only** PDF (near-empty
  extraction, no OCR — honest), a **password-protected** PDF (fail-clearly), a
  malformed/truncated PDF (poppler/Ghostscript tolerance), a tagged/AcroForm PDF.
- **TXT** in UTF-8/UTF-16/Windows-1252 + an invalid-byte file (fail-clearly, no
  mojibake); **MD** (GFM: tables/task-lists/code-fences, a local image + a remote
  image-URL that must not be fetched, YAML front-matter); **HTML** (article-like →
  PDF faithful; a JS/complex-CSS page → must render static-only, no JS exec, no
  remote-asset fetch).

**Spreadsheets** (`tests/corpus/spreadsheets/`)
- **XLSX/XLS/ODS** with formulas, multiple sheets (multi-sheet→CSV one-sheet
  behaviour + note), charts/conditional-formatting (drop-on-CSV), hidden rows/cols,
  a **macro-enabled `.xlsm`** (drop macros), a **password-protected** workbook
  (fail-clearly), a **>65k-row** sheet (XLS legacy-limit lossy case), a sheet with
  CJK/RTL cell text.
- **CSV/TSV**: comma, **semicolon (European decimal-comma)**, tab, pipe samples;
  UTF-8-BOM, UTF-16, Windows-1252; embedded-newline-in-quoted-field (RFC-4180);
  ragged rows; a **leading `=`/`+`/`@` cell** (CSV-injection: must import as text,
  never evaluate); a leading-zero value (`0123`, mis-typing-defence case).

**Presentations** (`tests/corpus/presentations/`)
- **PPTX/PPT/ODP** with animations/transitions (flatten-to-PDF), **embedded video/
  audio** (poster-only on PDF), embedded vs non-embedded fonts (substitution),
  SmartArt/WordArt (cross-family approximation), CJK/RTL slide text, a
  macro-enabled `.pptm`, a password-protected deck (fail-clearly), a several-hundred-
  slide deck (size pre-flight).

Corpus files **must be redistributable**; where a real-world artifact can't be
licensed for the public repo, a **synthetic equivalent** that reproduces the same
structural property is used and noted in the manifest. **`[DECIDED]` corpus storage
(adopting the [REC]): small synthetic + CC0 files committed in-repo** (so the per-PR
fast lane and the bijection guard §6.4.3a always have them), **larger real-world
media in an LFS-backed `corpus-large`** fetched **only** for the full Lane-B gate run
(§6.7.2), never required for the per-PR fast lane. Target total size co-owned with
§3.9. **[DEFER:** the exact total LFS size is calibrated as the corpus is filled.**]**

### 6.4.6 UI / end-to-end (the core-UX-flow gate)

A headed browser-driver run (**Tauri's WebDriver support / `tauri-driver`**, or
Playwright against the built app) exercises the full §5.2 flow per platform:
empty → intake → collected/confirm → target+default → destination shown → progress →
summary → open-folder. This is the automated half of the DoD **core-UX-flow** gate;
the human half is §6.6. Frontend component/unit tests use **Vitest** (§0.8).

- **Native file-drop is NOT automatable** by `tauri-driver`/Playwright (the OS-level
  native drag-drop event §5.4 cannot be synthesised by a WebDriver). So the
  **automated E2E uses the file-picker path** (C2 `pick_paths` via the §5.10
  accelerator, which funnels into the *same* C1 `ingest_paths` as a drop, §1.1) to
  reach Collecting; the **native drop itself is validated in the human walkthrough**
  (§6.6), where a real person drags a real file. This split keeps the automated gate
  honest about what it can and cannot synthesise.
- **Per-platform E2E driver `[OPEN]`:** the Windows and Linux legs use `tauri-driver`
  (WebDriver). The **macOS leg is an `[OPEN]`**: `tauri-driver` relies on
  `WKWebView`/`safaridriver`, which an **unsigned** build cannot drive cleanly — so
  the macOS E2E may degrade to a **scripted launch + screenshot/presence assertion**
  rather than full WebDriver, with the §6.6 human walkthrough carrying the macOS
  core-flow validation. Flagged for the owner; the Win/Linux automated E2E is firm.

---

## 6.5 The reliability gate (DoD operationalised) `[DECIDED]`

> SSOT: *every sensible source→target pair across all categories works reliably on
> all three platforms*; "reliably" = passes fail-clearly + no-harm on the corpus;
> the corpus existing is a precondition.

### 6.5.1 The unit of "done" and how a pair is declared reliable

The unit is the **individual (source→target) pair** (a category is just its set of
pairs). A pair is **`reliable`** when, **on each of the three platforms** (or on
each platform where §3.4 says it is available), against **every corpus file of that
source format**:
1. it produces a **valid, correctly-detected output** of the target format
   (§6.4.3 structural check), **and**
2. it upholds **no-harm** (originals byte-identical; atomic write; no-clobber) and
   **fail-clearly** (the corpus's known-bad inputs for that source produce the
   *expected* failure kind, batch continues), **and**
3. its **lossy disclosure** matches §04 (fires iff flagged), **and**
4. content fidelity holds for the properties the corpus exercises (CJK/RTL,
   orientation, tags, channels, remux-when-possible).

A pair that meets all four on all three (available) platforms is marked `reliable`
in the **pair-status ledger** (§6.5.2). **No pair is "done" until the corpus run
backs it** — this is what makes the gate non-circular.

### 6.5.2 The pair-status ledger (machine-checkable)

CI maintains a generated **pair-status report** (`reliability-report.json` +
human table) keyed by `(source, target, platform)`, each cell ∈
`{reliable, failing, unavailable-per-§3.4, demoted}`. The **release gate** is:

> **Every enumerated pair is `reliable` on every platform where it is not
> `unavailable-per-§3.4` or explicitly `demoted`.** Any `failing` cell blocks the
> release. The report is published as a release asset (transparency).

This directly realises the SSOT *v1 DoD* conversions clause. Because v1 is **one
large all-or-nothing release with no deadline** (SSOT), the gate has no
time-pressure escape hatch — internal *sequencing* (fill/validate category by
category) is allowed (SSOT) and the ledger naturally tracks that progress.

### 6.5.3 Recording the two permissible exceptions (as release-note items)

Both SSOT exceptions are recorded **in the ledger and the release notes**, never as
silent omissions:
- **Exception 1 — patent per-platform gap (§3.4).** A pair `unavailable-per-§3.4`
  on a platform (e.g. an HEVC-encode-dependent HEIC target, or — the category's
  hardest dependency — H.264/AAC for the **MP4 default target**) is an **explicit,
  documented, honestly-surfaced** release-note line ("HEIC encoding is unavailable
  on Linux because no openly-redistributable HEVC encoder ships there"). The UI
  shows it unavailable (§5.2). **§3.4 owns the decision; this gate owns recording
  it.** *Critical dependency note:* MP4 is the default target of **every** video
  source (§04 video), so the gate **flags it as a hard precondition** that §3.4
  decide H.264/AAC `ship-bundled` on all three platforms — a platform without an MP4
  default target is a product-level problem the release notes must call out, not a
  per-format footnote.
- **Exception 2 — last-resort reliability demotion.** An in-scope, licence-clean
  pair that **genuinely cannot meet the bar despite reasonable effort** may be
  `demoted` to *Future Ideas (Parked)* as an **explicit, documented release-note
  item**, so one stubborn pair can't block the whole release forever. Demotion is a
  **last resort**, requires a recorded rationale in the ledger, and is **never** a
  convenience cut. (SSOT.)

### 6.5.4 Re-validation on engine bump

When §3.8 bumps a bundled engine (best-effort currency), the **full reliability gate
re-runs** before that engine version can ship (§6.3.4) — a patch must not silently
regress a pair. The ledger diff (pairs that changed status) is part of the bump's
review.

---

## 6.6 Usability-floor check `[DECIDED — gate; per-platform human walkthrough]`

> SSOT *v1 DoD*: *an ordinary non-technical person can complete each named
> conversion unaided on first try (drop → pick → convert → find output), validated
> by at least one informal non-developer walkthrough per platform.*

This is a **release gate** the automated §6.4.6 E2E flow **cannot** replace — it
specifically tests whether a *human who didn't build it* succeeds. Protocol:

- **Who:** at least **one non-developer** per platform (Windows, macOS, Linux) —
  three walkthroughs minimum. The SSOT usability walkthrough is also the natural
  place to validate the genuinely-debatable per-source defaults flagged in §04
  (XLSX→CSV vs →PDF; MP3-source→WAV vs FLAC; MOV-as-target demand).
- **What they must complete unaided (the named conversions = the DoD bar
  examples, one per category):** `mov→mp4`, `png→webp`, `heic→jpg`, `mp3` source →
  its default, `docx→pdf`, `xlsx→csv`, `pptx→pdf`, plus the two cross-category ops
  (extract-audio → MP3; a clip → GIF). Each via the **two-click common path**
  (drop → already-highlighted-or-pick target → convert) with **no instruction**.
- **What "counts" (pass criteria):** for each task the tester, with no help,
  (1) understands the empty screen and drops/browses a file; (2) sees the collected
  summary and confirms; (3) reaches a sensible result with the **pre-highlighted
  default** (no required choices); (4) sees **where it will save before converting**;
  (5) on completion uses **open-folder/open-file** and **finds the output**; (6)
  hits no stack trace, no cryptic message, no dead end. A task where the tester
  gets stuck or needs help **fails** the floor for that platform → fix → re-walk.
- **Accessibility floor (part of the same gate, SSOT *For anyone*):** at least one
  walkthrough completes the core path **keyboard-only** (per the §5.10 shortcut map)
  and verifies readable contrast/text-size; this checks the DoD **basic-a11y** gate
  with a human, complementing automated a11y assertions (§5.6).
- **Recording:** results captured in `docs/usability-floor.md` (per platform:
  tester profile, tasks, pass/fail, observed friction, the default-validation notes).
  This file is a **required v1 artifact**; the gate is "three platform walkthroughs
  recorded, all named conversions pass" before release.
- **Staleness criterion (machine-checkable) `[DECIDED]`:** each walkthrough record
  carries a **`release_line` (the version/tag it validated)** and a **`date`**. The
  Lane-B §6.7.2 stage 5 gate **fails if the recorded `release_line` does not match
  the release being built** (or, absent a version match, if the `date` predates the
  current release branch's cut) — so an old walkthrough cannot silently satisfy a new
  release. (CI checks the *evidence's* freshness; the human does the walkthrough.)

`[OPEN-6.6a]` exact tester sourcing/count beyond the SSOT minimum-of-one-per-
platform — owner-level. **(recommendation: SSOT minimum of one non-dev per platform
for v1; more if cheaply available.)**

---

## 6.7 CI/CD `[DECIDED — two-lane pipeline]`

Two lanes, reflecting the platform CI standard (`reference_cicd_setup.md`:
reusable workflows + branch protection + auto-merge + deploy-gate) adapted from a
*server-deploy* model to a *desktop-release* model (there is **no server deploy** —
ConvertIA is a downloadable artifact; the "deploy" is a GitHub Release).

### 6.7.1 Lane A — PR / push validation (fast, every change)

Runs on the **self-hosted Linux runner** (cheap; `reference_self_hosted_ci_runner.md`)
for the OS-agnostic checks, fanning to the matrix only for compile-sanity:
1. **Lint/format:** `cargo fmt --check`, `cargo clippy -D warnings` (enforces the
   platform **no-`any`/no-unwrap-sloppiness** quality bar), ESLint + `tsc --noEmit`
   (no `any` — CLAUDE.md global rule), Prettier, `yamllint` (via `python3 -m`, per
   the platform runner PATH workaround in the recent commits).
2. **Rust↔TS type drift check (§0.4.5):** the codegen tool (ts-rs/specta/
   tauri-specta — decision owned by §0.4.5) regenerates the shared types and CI
   **fails if the committed types differ** (enforces the IPC contract + "no `any`").
3. **Unit + property + fault-injection tests (§6.4.1/§6.4.2)** — Rust + Vitest;
   fast, engine-light, run on every PR.
3a. **Corpus↔pair bijection guard (§6.4.3a):** `scripts/check-corpus-coverage.*`
   asserts every §04 v1-required pair has ≥1 backing corpus `covers` entry (and no
   stale couplings). Engine-free, fast — runs every PR so coverage gaps surface
   before the expensive Lane B corpus run.
4. **Compile-sanity on the matrix:** `cargo check` / a debug `tauri build` on all
   three legs to catch platform-specific breakage early (no full corpus run here).
5. **`cargo audit` / `cargo deny`** (advisory + licence policy, §6.3.4).
Branch protection requires Lane A green before merge to `main` (matches the
platform's branch-protection + auto-merge model).

### 6.7.2 Lane B — Release pipeline (tag-triggered, the full gate)

Triggered by a release tag (e.g. `v1.0.0`) on `main`. Stages, **in order**, each
blocking the next:
1. **Matrix build (native, §6.1.4):** stage engines per platform (§6.1.3), run
   `tauri build` → per-platform artifact (portable `.exe`/NSIS, universal `.dmg`,
   AppImage).
2. **Full reliability gate (§6.5):** integration + property + corpus + E2E on **all
   three** legs; emits `reliability-report.json`. **Any `failing` pair aborts the
   release.** **Runtime / cost:** the dominant cost is the corpus run (video
   re-encode + LibreOffice, the slow engines). Estimate **~30–90 min per leg**
   depending on corpus size; set CI **`timeout-minutes` ≈ 120 per leg** with headroom.
   **Intra-leg parallelism** is bounded by the **§0.9 concurrency degree** and must
   honour the **LibreOffice-serialised** constraint (the office-pair tests run LO
   single-slot — the harness reuses the §0.9 config so the test env matches prod).
   The **`corpus-large` LFS set is fetched only for this Lane-B run** (never the
   per-PR fast lane, §6.4.5).
3. **SBOM + NOTICE assembly + attribution-completeness gate (§6.3):** generate
   CycloneDX (app + engines), assemble `NOTICE`/`THIRD-PARTY-LICENSES.txt`, run the
   §6.3.3 completeness check. **A missing/UNKNOWN attribution aborts the release**
   (release-blocking, same status as no-harm).
4. **Name/trademark clearance gate (§6.9):** assert the clearance record is present
   and current; if a rename was required, assert it propagated (the §6.9 check).
   **Blocks if not cleared.**
5. **Usability-floor evidence gate (§6.6):** assert `docs/usability-floor.md` records
   passing walkthroughs for all three platforms for this release line. **Blocks if
   absent.** (Human step — its *evidence* is what CI checks.)
6. **Integrity hashing (§6.2.3):** compute SHA-256 per artifact, build `SHA256SUMS`
   covering **every** release asset, **and the minisign signature over `SHA256SUMS`**
   (§6.2.3 `[DECIDED]`).
7. **Publish to canonical GitHub Releases (§6.2.2):** upload artifacts + `SHA256SUMS`
   + `.sha256` files + SBOM (CycloneDX/SPDX) + `reliability-report.json` +
   `NOTICE`/`THIRD-PARTY-LICENSES.txt` as a **single coordinated release** (one
   large all-or-nothing v1 — SSOT). The release body restates as-is/no-warranty +
   the verify-your-hash recipe (§6.2.4) and lists the two-exception release-note
   items (§6.5.3).

There is **no auto-update/phone-home publishing step** — the updater is explicitly
disabled/absent (§7.6); users learn of releases by visiting the page
(user-initiated, SSOT).

### 6.7.3 What CI does *not* do

No code-signing, no notarization, no store submission (SSOT *Out of Scope*) — only
the in-code-required artifacts (SBOM, checksums) are produced. No telemetry, no
network-touching test that would contradict the offline invariant (§2.11) — the
offline-observability test (6.10 / §2.11) asserts the *running app* makes **zero
network calls**, ideally enforced by running the E2E flow with network egress
blocked at the runner.

---

## 6.8 Repo governance & policy artifacts `[DECIDED — concrete deliverables]`

The SSOT *License & Openness* mandates a specific set of in-repo documents; each is
a **Phase-3 authoring task** and several are **referenced by the release gates**.
All are English (public OSS repo). Mapping each to its SSOT origin and content owner:

| File | Required content | SSOT origin / owner |
|------|------------------|---------------------|
| **`LICENSE`** | MIT, header `Copyright (c) 2026 Ne-IA and ConvertIA contributors` (collective notice — inbound=outbound, **no assignment**). | SSOT *License & Openness*. Gate: present + name matches §6.9 clearance. |
| **`NOTICE`** + **`THIRD-PARTY-LICENSES.txt`** | Per-engine name+version, full licence text, written-offer-of-source for GPL-family. **Generated** from `engines.lock` + SBOM (§6.3.2), never hand-drifted. | SSOT *Engine-license policy*; **data owned by §3.7**, assembly here (§6.3), display §5.9. **Release-blocking** (§6.3.3). |
| **`CONTRIBUTING.md`** | Inbound=outbound under MIT; **no CLA**; **optional DCO sign-off** (`Signed-off-by`, *requested not required*); the **inbound-warranty clause** (contributors warrant submissions are their own work or compatibly-licensed for inbound MIT; incompatibly-licensed code is not accepted); how to run the test/lint lanes (§6.7.1); the quality bar **stated directly** in CONTRIBUTING (no `any`; no `// TODO`; no `console.log` in prod; no inline CSS; every change production-ready) — **not** by reference to a private `CLAUDE.md` (an internal file, not present in the public OSS repo). | SSOT *License & Openness* (contributions). |
| **`CODE_OF_CONDUCT.md`** | A standard CoC (Contributor Covenant-class) with the SECURITY/maintainer contact for enforcement. | SSOT *License & Openness* (a code of conduct accompanies the repo). |
| **`SECURITY.md`** | **Private vulnerability reporting** channel (GitHub private advisories + a contact); scope statement = ConvertIA opens **untrusted files through third-party decoders** → references the §0.11 threat-surface map and the §2.12 isolation posture; best-effort patch posture **with no SLA** (SSOT); how a reporter can include a (redacted, §7.5) repro from the local log. | SSOT *Security posture* / *License & Openness*; ties to §2.12, §7.5, §0.11. |
| **`PRIVACY.md`** | Plain-language restatement of **§2.11**: fully offline, **no network/telemetry/accounts/update-phone-home**; the only network is user-initiated (open project page, §7.7); the **cloud-sync caveat** (ConvertIA neither causes/prevents/detects your OneDrive/iCloud/Dropbox sync uploading files in a synced folder). | SSOT *Local/private/offline*; restates §2.11 (owner of the invariant). |
| **`TRADEMARK.md`** | The MIT grant covers **code, not the "ConvertIA" name or the Ne-IA logo**; forks/redistributions must use a **different name** and may **not** use the Ne-IA logo; guidelines for nominative use. | SSOT *Trademark*. |
| **`README.md`** (download + trust) | What it is, the **canonical-GitHub-Releases-only** download location (§6.2.2), the **verify-your-hash** recipe (§6.2.4), as-is/no-warranty + best-effort-security posture, supported-OS floor (§0.3.1), per-platform unsigned-build first-launch note. | SSOT *Distribution & download trust*. |
| **`.github/` policy** | Issue templates (default new format/feature requests to **Future Ideas (Parked)** per the SSOT inclusion test — SSOT *Out of Scope*); PR template referencing the DCO/quality bar; private-advisory config wired to `SECURITY.md`. | SSOT *Out of Scope* (inbound-request default) + governance. |

**DCO posture (explicit) `[DECIDED]`:** **no CLA**; a DCO **`Signed-off-by`** line
is **requested, not required** (SSOT: "a DCO sign-off may be requested, not
required"). CI **does not hard-block** an unsigned commit (that would make it
required) but **may surface a friendly reminder**; authorship is recorded in Git
history (the collective-notice / no-assignment model). The inbound-warranty clause
lives in `CONTRIBUTING.md`.

`[OPEN-6.8a]` whether to additionally adopt a `GOVERNANCE.md`/maintainer model doc
for v1 — **(recommendation: defer; the seven files above satisfy the SSOT mandate;
add governance docs only if the contributor base grows.)**

---

## 6.9 Name/trademark clearance gate + rename propagation `[DECIDED — release-blocking; process out-of-scope, doing-the-check in-scope]`

> SSOT *Naming* + *v1 DoD*: trademark/name-collision risk for **both** "ConvertIA"
> **and** the public use of the "Ne-IA" brand has **not** been cleared; a clearance
> check (in the jurisdictions relevant to a globally-downloadable app) is a
> **precondition before first public release**, and the name **may change** if a
> conflict is found.

**Scope split (important):** *registering* a trademark is **out of scope** (SSOT
*Out of Scope* — no store/cert/vendor process). **Performing the clearance check and
propagating any required rename** is **in scope** as a release gate — this is the
distinction the README open-questions log records.

### 6.9.1 The clearance check (the gate input)

A documented clearance review for **both** marks ("ConvertIA" and the public "Ne-IA"
brand) across the jurisdictions relevant to a globally-downloadable app (at minimum
EU/EUIPO, US/USPTO, and a sanity check on common app-distribution regions), plus
common-law / existing-product / domain / package-name collision search (crates.io,
npm, GitHub org, app listings). The result is recorded in
`docs/name-clearance.md` with: marks checked, jurisdictions/registries searched,
date, findings, and a **verdict ∈ {clear, conflict→rename, conflict→abort}**. This
is an **owner/human task**, not automatable — but its **evidence is what the gate
checks**.

**`[DECIDED]` clearance verdict = `clear`.** The owner has cleared **both**
"ConvertIA" and the public "Ne-IA" brand for v1 use. `docs/name-clearance.md` records
this verdict (`clear`), dated for the release line; the §6.9.2 gate asserts the record
is present and current. No rename is required, so the §6.9.3 mechanical rename
propagation stays dormant (a documented capability, not a v1 action). (The legal
*advice/registration* process remains out of scope per the SSOT; the **technical
gate** — assert a current `clear` record exists — is in scope and retained.)

### 6.9.2 The release gate (CI-checkable)

Lane B stage 4 (§6.7.2) **asserts** `docs/name-clearance.md` exists, is dated for
the current release line, and its verdict is `clear` **or** `conflict→rename` with a
completed rename (next clause). A `conflict→abort` or a missing/stale record
**blocks the release**. (CI checks the *record*; the human does the *check*.) For v1
the verdict is **`clear`** (§6.9.1 [DECIDED]); the gate's job is to confirm that
record stays present and current per release line.

### 6.9.3 Mechanical rename propagation (if a conflict surfaces)

If clearance returns `conflict→rename`, the rename is applied **before** release,
**never after** (SSOT). A single scripted, reviewable rename pass
(`scripts/rename-brand.*`) propagates the new name across **every** surface so no
stale "ConvertIA" leaks into a published build:
- repo/package identity: `Cargo.toml` (crate + `productName`), `package.json`,
  `tauri.conf.json` (`productName`, `identifier`, window title, bundle name),
  the GitHub repo + org references;
- **`LICENSE` / `NOTICE` / `TRADEMARK.md`** copyright/name lines;
- README, all §6.8 governance docs, the download page, the verify recipe;
- **branding**: the Ne-IA logo/app-icon assets and About-screen strings (§5.5/§5.9 —
  placeholders the owner controls), bundle icons, the `.desktop`/Info.plist names;
- the in-app product name strings (§5 UI-chrome) and the SBOM/`engines.lock`
  product field.
A post-rename CI **grep gate** asserts the **old name appears nowhere** in
shippable artifacts (a `rg` over the repo + the staged bundle for the old token,
excluding historical changelog entries). The rename is one atomic PR, reviewed,
then the release proceeds.

---

## 6.10 DoD-traceability checklist `[DECIDED — the "every behaviour has a home" table]`

Maps **every** SSOT *v1 Definition of Done* gate to its **owning spec section** and
the **§6 mechanism that verifies it**, so the README claim "every behaviour the SSOT
promises has a technical home" is **verifiable**. Each gate is marked
**in-scope-gate** (we implement+verify it) vs **out-of-scope-process** (the
*process* — e.g. registering a mark — is out; *doing/checking* it is in).

| # | SSOT DoD gate | Owning section(s) | §6 verification mechanism | Scope |
|---|---------------|-------------------|----------------------------|-------|
| 1 | **Every sensible source→target pair works reliably on all 3 platforms** | §04 (pairs) · §1 (pipeline) · §3 (engines) | Reliability gate / pair-status ledger (§6.5); integration+corpus tests (§6.4.3–6.4.5) | **in-scope-gate** |
| 2 | **"Reliably" = fail-clearly + no-harm on a real-world corpus** | §2.5 (no-harm) · §2.8 (fail-clearly) | Property/fault-injection (§6.4.2) + the corpus (§6.4.5) as precondition | **in-scope-gate** |
| 3 | **The corpus exists (required v1 asset, non-circular gate)** | this file | `tests/corpus/` + `manifest.toml` (§6.4.5); the **corpus↔pair bijection guard (§6.4.3a)** fails CI if any §04 pair has no backing corpus file (or a `covers` entry names a non-existent pair) | **in-scope-gate** |
| 4 | **Everything runs fully offline (whole engine set bundled, no fetch)** | §3.3 (bundle-all) · §2.11 (offline invariant) | Bundling at build (§6.1.3); offline-observability E2E with egress blocked (§6.7.3); SBOM proves no runtime-fetch component | **in-scope-gate** |
| 5 | **Offline guarantee observably true (no network at all)** | §2.11 | Network-egress-blocked E2E run asserts zero calls (§6.7.3 / §6.4.6) | **in-scope-gate** |
| 6 | **Basic accessibility (keyboard path + readable contrast/sizes; WCAG 2.1 AA per §5.6)** | §5.6 · §5.10 (shortcut map) | Automated a11y assertions incl. **WCAG 2.1 AA contrast (≥4.5:1 text, ≥3:1 large/UI)** (§5.6) + the keyboard-only human walkthrough (§6.6) | **in-scope-gate** |
| 7 | **Core UX flow (drag/drop+picker+keyboard → same result; reacts to type; pre-highlighted default; destination shown before convert; visible cancellable progress; end-of-batch summary; one-click open-folder/file)** | §5.2 (states) · §1.1/§1.5/§1.11/§1.12 · §7.7 (open) | E2E flow per platform (§6.4.6) + usability-floor human walkthrough (§6.6) | **in-scope-gate** |
| 8 | **Unwritable/ephemeral-location fallback works** | §2.7 (per-location divert) · §2.14 (cross-volume) | Property tests on read-only/USB/network/temp locations (§6.4.2); divert path in corpus runs | **in-scope-gate** |
| 9 | **Every bundled engine's required licence text + attribution present and correct (NOTICE/third-party-licenses, backed by SBOM) — missing attribution release-blocking** | §3.7 (data) · §5.9 (display) | SBOM + NOTICE assembly + **attribution-completeness gate** (§6.3.3); blocks release | **in-scope-gate** |
| 10 | **Name/trademark clearance completed; any rename applied across repo/LICENSE/NOTICE/branding before release** | this file (§6.9) | Clearance-record gate (§6.9.2) + scripted rename propagation + old-name grep gate (§6.9.3) | **in-scope-gate** (the *clearance check + rename*); **out-of-scope-process** (*registering* a mark) |
| 11 | **Usability floor: ordinary non-tech person completes each named conversion unaided on first try; ≥1 non-dev walkthrough per platform** | §5 (UX) · this file (§6.6) | Per-platform human walkthrough recorded in `docs/usability-floor.md`; evidence gate in Lane B (§6.6/§6.7.2) | **in-scope-gate** |
| 12 | **Published integrity hashes from one canonical source (trust substitute for no-signing)** | this file (§6.2) | SHA-256 + `SHA256SUMS` (+ optional sig) published to canonical GitHub Releases (§6.2.2/§6.2.3); verify recipe surfaced (§6.2.4) | **in-scope-gate** |
| 13 | **One artifact per platform (cross-platform, one product)** | §0.2 · this file (§6.1) | Build matrix artifact table (§6.1.2): portable-exe / universal-dmg / AppImage | **in-scope-gate** |
| 14 | **No-harm / atomicity / fail-clearly hold even across crash/cancel/out-of-disk** | §2.1/§2.6/§2.8/§2.13/§2.14 | Atomicity-under-interruption + out-of-disk + panic-boundary property tests (§6.4.2) | **in-scope-gate** |
| 15 | **Real-world filename + content fidelity (Unicode/emoji/long-path; CJK/RTL/encodings; CSV delimiters)** | §2.10 · §04 (per-format) | Adversarial-name unit tests (§6.4.1) + CJK/RTL/encoding corpus files (§6.4.5) | **in-scope-gate** |
| 16 | **Patent per-platform gaps honestly surfaced (exception 1), never silent** | §3.4 (decision) · §5.2 (UI surfacing) | Ledger marks `unavailable-per-§3.4`; release-note item (§6.5.3); UI-unavailable assertion (§6.4.3) | **in-scope-gate** (recording/surfacing); patent **decision** owned by §3.4 |
| 17 | **Reliability-demotion (exception 2) explicit + documented, last resort** | §3.2/§04 (which pair) · this file | Ledger `demoted` state + recorded rationale + release-note item (§6.5.3) | **in-scope-gate** |
| 18 | **Single-instance + run identity (no cross-instance temp clobber; freeze unaffected by a second launch)** | §7.1 · §2.4/§2.6 | Single-instance plugin behaviour test; per-run/instance temp-ownership + advisory-lock liveness property tests (§6.4.2) | **in-scope-gate** |
| 19 | **Startup integrity & engine-presence (missing/corrupt engine → app-fault, not a crash)** | §7.2.3 · §2.13 | Startup-fault test: a removed/truncated bundled engine yields the plain app-fault screen, never a stack trace (§6.4.2 / §6.4.6 headed smoke) | **in-scope-gate** |
| 20 | **OS intake (Open-with / launch-args route through the single freeze funnel; no file-association pollution)** | §7.8 · §1.1/§2.4 | Launch-with-files E2E (UI enters Collecting at startup); assert no associations registered (§7.8.2) | **in-scope-gate** |
| — | **NOT a gate: subjective visual polish; engine-currency** | §5.5 (polish) · §3.8 (currency) | Polish is iterative (never blocks); currency is best-effort, re-validated against the gate when bumped (§6.3.4/§6.5.4) | **out-of-scope-gate** (explicit non-gates) |

If a future SSOT clause is added, it must appear here with an owning section and a
§6 mechanism, or it has no technical home — that is the check this table enforces.

---

## Open-questions log contributions (this section)

**Now `[DECIDED]` (this round) — adopted from their recommendations:**
- **[6.9a] Name/trademark clearance verdict = `clear`** for "ConvertIA" / "Ne-IA"
  (owner-cleared; §6.9.1). The release gate (record present + current) is retained;
  the legal *process* stays out of scope.
- **[6.2a]** Sign `SHA256SUMS` with a **project minisign key** — DECIDED yes (§6.2.3).
- **[6.1e]** CI runners — **GitHub-hosted for mac/win, self-hosted Linux for Lane A**
  (§6.1.4; budget note retained).
- **[6.1d]** CI engine-acquisition — **pinned, checksum-verified asset cache**
  (§6.1.3).
- **[6.4a]** Corpus storage — **small CC0/synthetic in-repo + LFS `corpus-large` for
  the full gate** (§6.4.5); exact total size **[DEFER: calibrate as corpus fills]**.

Easy `[OPEN]`s resolved with a recommended default (not owner-level): artifact
formats (§6.1.2: portable-exe / NSIS-per-user / universal-dmg / AppImage),
NSIS-vs-portable (§6.1.2a), Linux `.deb` (§6.1.2b), reproducible-build depth
(§6.2.5b), `GOVERNANCE.md` (§6.8a), usability tester count (§6.6a) — each carries a
**(recommendation)** inline.

**Genuinely still open / deferred (feed the README log):** the macOS E2E driver under
an unsigned build (§6.4.6 `[OPEN]`), and the exact `corpus-large` total size
(`[DEFER: corpus]`).
