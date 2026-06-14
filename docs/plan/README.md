# ConvertIA — Implementation Plan (Phase 3)

> The implementation roadmap, derived from the [Specification](../spec/README.md)
> (which is itself derived from the [Single Source of Truth](../SINGLE-SOURCE-OF-TRUTH.md)).
> Conflict rule unchanged: **SSOT wins**, then the spec, then this plan.

## How this plan is used

- Work proceeds **one logical phase at a time, top to bottom**, and **box by box**
  within a phase. A phase is "done" when every `[ ]` step under it is checked.
- This file currently holds the **logical phase skeleton only**. The atomic
  `[ ]` steps are added in a later fill pass — **after** this structure is
  reviewed and signed off (and after P0 content is supplied by the owner).
- Scope is the **pure software** plus the *technical* release mechanics that make
  verified **downloads** work. ConvertIA is offline and **does not auto-update or
  phone home** (SSOT Principle 4; spec §7.6.1 — `tauri-plugin-updater` is *absent*
  by decision). Explicitly **not** in this plan: marketing, legal advice,
  store/developer-account logistics, code-signing/notarization (see SSOT
  *Explicitly Out of Scope*) — except where they impose an in-code requirement
  (SBOM, checksums, signing the **checksum manifest** — minisign over `SHA256SUMS`,
  §6.2.3; not an update-manifest signing key).

## Sequencing philosophy — walking skeleton first

After the foundation (P1–P2), P3 drives **one trivial conversion end-to-end
through the real architecture** (drop → detect → pick → convert → atomic publish
→ result UI) before any heavy engine is integrated. The walking-skeleton
conversion is the **in-process CSV→TSV** path (`EngineProgram::InProcessNative`,
§3.5.6) — chosen because it is buildable from P1–P2 alone with **no engine /
sidecar dependency**, yet still exercises the full vertical slice and the real
`crate::fs_guard` atomic, no-clobber publish + FAT/exFAT divert primitives
(§2.1.2/§2.7.2; `fs_guard` has no engine dependency). This de-risks the whole
Tauri + atomic-publish + IPC stack on all three OS early; the **first real
sidecar** is exercised later in P4/P5 where the image-worker is built. The later
format phases (P5–P7) only *broaden* coverage on a proven harness instead of
discovering architectural problems late. Phase order therefore favours a
**vertical slice early, then horizontal breadth**. CSV→TSV is the *one* path that
may legitimately bypass the §2.12 decoder-isolation boundary because it is **pure
memory-safe Rust operating in-core, not a decoder of untrusted third-party C/C++
bytes** (§2.12.4 absolute) — so P3 has **no** forward dependency on P4's isolation
framework even though it detects and transforms untrusted input.

---

## P0 — Build & Security System

**The foundation before the foundation** — the guardrail system every later phase
runs under: security concept, the full gate system, the build-loop, the Opus/Sonnet
dual review, and the test methodology. Built **before** P1 writes app code.

**Detailed structure:** [P0-build-and-security.md](P0-build-and-security.md)
(clusters P0.1–P0.7). **Concept:** [security-concept.md](../security/security-concept.md)
+ [build-gates.md](../security/build-gates.md).

**Boundaries:** P0 builds the gate *system* + content-independent gates + the
language-gate framework; **P1** wires the language gates as it scaffolds + authors
the general governance docs; **P10** builds the release pipeline whose policy P0
defines.

---

## P1 — Foundation & Scaffolding

**Goal:** an empty ConvertIA window boots on Windows, macOS and Linux from a
clean checkout, with the full toolchain and CI in place.

**Scope:** monorepo layout; Tauri v2 shell + React 19 / TypeScript (strict) /
Tailwind / Vite WebView; pnpm workspace; lint / format / Vitest / Rust test
wiring; baseline capabilities/permissions allowlist + CSP; runs `dev` and
produces a bundle on each OS. The **`src/strings/ui.ts`** English-string module
and the `a11y/` module shells (announcer.ts, keymap.ts, §5.6/§5.7/§5.10) are
established here as structural scaffolding, not deferred. **Governance docs +
README skeleton + `.github/` templates** (§6.8): author `CONTRIBUTING.md`,
`CODE_OF_CONDUCT.md`, `SECURITY.md`, `PRIVACY.md`, `TRADEMARK.md`, the README
download/trust page skeleton, and the issue/PR/private-advisory templates — they
gate contribution from the first commit and have no build dependency (the
release-blocking governance-completeness **gate** is asserted later in P10).
**CI (Lane A scaffold):** stand up the per-PR Lane-A pipeline (lint / format /
type-check / compile-sanity, build on all 3 OS, Principle-11 English-only lint,
Rust↔TS type-drift §0.4.5, cargo-deny/audit). Data-dependent Lane-A guards
(corpus↔pair bijection §6.4.3a, defaults-registry §1.6, axe-core a11y §6.4.6a)
are **ADDED by the phase that produces their input** — CI is incrementally
hardened, not finished here; Lane B (release pipeline) is assembled in P10.

**Spec home:** 00-architecture, 06-build-test-release (tooling, §6.7 Lane A,
§6.8 governance), 07-app-shell (window), 05-ui-ux (strings/a11y modules).

*Atomic `[ ]` steps: added in the fill pass.*

---

## P2 — App Shell & Pipeline Contracts

**Goal:** the application skeleton and the conversion-pipeline contracts exist
and are type-shared end-to-end — with no real conversion engine yet.

**Scope:** window lifecycle, single-instance, file-open / drag-drop intake
events; Rust↔TS type-sharing (tauri-specta + specta) and the IPC `Channel<T>`;
store/persistence; structured logging; the detect → plan → convert → publish
state machine + domain types + error model as contracts. The **§7.8 OS-intake
funnel** (the macOS-only `RunEvent::Opened` Open-with hook + `forward_launch_intake`
+ the `PendingIntake` buffer-then-replay for the first-launch Open-with race) and
the **§7.8.2 explicit negatives** (no file-association, no URL scheme) — DoD gate
20 — are homed here, feeding the §1.1 intake state machine + single-instance
contracts. The **§7.2.1 startup-sequence ORDERING** is established here as the
app-shell spine (single-instance guard, the engine presence+integrity probe slot,
§7.2.4 exec-permission setup, §7.2.5 scratch/log orphan reclaim, §7.2.2
zero-startup-network assertion, launch-intake feed, WebView-absent app-fault) —
the verifier body lands in P4, but the ordered sequence is owned here. The **C12
`get_engine_health` IPC command + the `EngineHealth` type** (§7.2 — `present` /
`integrity_ok` / `runnable` fields, consumed by §5.2 to disable unavailable
targets, escalated to §2.13 app-fault) is a pipeline contract with **no engine
dependency** — it is type-shared here alongside the other pipeline contracts; the
runtime probe that populates it is built in P4.

**Spec home:** 00-architecture, 01-conversion-pipeline (§1.1 intake), 07-app-shell
(§7.2 startup sequence + C12 `EngineHealth` contract, §7.8 intake posture).

*Atomic `[ ]` steps: added in the fill pass.*

---

## P3 — Walking Skeleton (first conversion, end-to-end)

**Goal:** one dependency-light conversion works fully through the real stack on
all 3 OS, proving the architecture before any heavy engine lands.

**Scope:** the **in-core CSV→TSV** conversion (`EngineProgram::InProcessNative`,
§3.5.6 — no sidecar binary) wired drop → detect → pick target → convert →
**atomic, no-overwrite publish** → result UI; exercises the **in-process
conversion path** and the real `crate::fs_guard` **atomic-publish OS primitives**
(§2.1.2/§2.7.2, built here from P1–P2 — incl. the FAT/exFAT no-atomic-publish
divert). Also bootstraps the **§1.2 layered-detection framework** (magic-sniff +
container/text/encoding classification) needed to detect the walking-skeleton
type; P5–P7 later add only per-format signatures. No image-worker / libvips here
— that is P4/P5 (the first real sidecar is validated there, not in P3).
**§1.7 dispatch stubs (compile-time only):** the §1.7 dispatch enum reaches the
InProcessNative branch only, but if its types reference `crate::isolation` (§2.12)
and the §0.9 subprocess-pool envelope, P3 creates **minimal interface-only shells
for those modules** (InProcessNative path compiles without spawning anything) which
**P4 expands into the real isolation wrapper + pool** — at runtime CSV→TSV
genuinely bypasses isolation/pool (§2.12.4: pure in-core Rust, no subprocess), so
this is a compile-time stub, not a forward dependency. P4 therefore does **not**
build `crate::isolation`/the pool from scratch — it fills the shells P3 establishes.

**Spec home:** 01-conversion-pipeline (incl. §1.2 detection, §1.7 dispatch stubs),
02-guarantees (§2.1–§2.7 fs_guard incl. §2.2 output naming, §2.3 link-safety [T7],
§2.4 frozen set [T8], §2.5 re-run detection, §2.6 cleanup/temp-ownership),
05-ui-ux (minimal).

*Atomic `[ ]` steps: added in the fill pass.*

---

## P4 — Engine & Bundling Framework

**Goal:** the reusable harness every format engine plugs into, plus per-OS
bundling, the **generic** security/isolation layer, the cross-cutting reliability
test machinery format phases plug pairs into, and the **generic UX-correctness
primitives** so each engine phase is testable end-to-end against a built UI.
**Exit criterion (proof-of-life):** P4 is "done" when `convertia-imgworker` boots,
a round-trip invocation succeeds through the §2.12 isolation boundary, the startup
verifier reports a populated `EngineHealth`, and the §6.4.3 runner + pair-status
ledger + §6.4.3a bijection guard produce their first report — **and** a
representative P5 image pair can be driven end-to-end through the P4-built
options-panel shell + progress/cancel + result-actions UI (the UX-harness leg, so
P4 is not "done" on the engine side alone).

**Scope:** generalized engine-invocation layer; per-OS sidecar packaging &
bundling (everything offline); the isolated **image-worker** process
(`convertia-imgworker`, §3.5.5 — the first real sidecar) + the §2.12 isolation
boundary; resource budgets/limits; the **generic isolation framework only** —
the §2.12 decoder-isolation wrapper, the §0.9 subprocess pool, the §2.12.3
best-effort privilege-drop tier, and threat-map assembly/ownership (§0.11).
**macOS TCC source-staging (§3.5.0 / §7.2.6 — load-bearing for every macOS engine
read):** the Rust core copies a TCC-protected source into per-job kind-2 scratch
(§2.14.2) **before** spawning and hands the sidecar the scratch path, so a spawned
engine is never the first process to touch Desktop/Documents/Downloads/removable;
it is the macOS staged-input term in the §1.10 `est_scratch_bytes` preflight and
composes with the §2.14 cross-volume strategy — homed here because the first real
engine spawn (the imgworker proof-of-life) would otherwise hit a TCC chain-break
denial.
**Per-engine SSRF/LFR hardening** (FFmpeg protocol-whitelist §3.5.1, pandoc
`--sandbox` §3.5.4, LibreOffice profile-hardening §3.5.2, librsvg-no-base-URL
§3.5.5) lives in its **engine phase** (P5/P6/P7), not here. **Reliability
harness:** build the cross-cutting machinery format phases plug pairs into — the
per-pair integration runner (§6.4.3), the §6.5.2 **pair-status ledger**
generator (`reliability-report.json`), and the §6.4.3a corpus↔pair **bijection
guard** (`scripts/check-corpus-coverage.rs`). **SBOM + NOTICE /
third-party-licenses:** tooling/schema **scaffold only** — per-engine rows are
populated in P5–P7 as each engine is staged; finalization is P10. **Startup
engine-presence + integrity verification (§7.2.3 — DoD gate 19):** the build-time
**in-bundle hash manifest** GENERATION, the `engine-integrity.json` warm-launch
marker, and the **startup verifier** (hash-on-first-launch / cheap warm check)
that populates the C12 `EngineHealth` contract (declared in P2) and escalates a
missing/corrupt engine to a §2.13 app-fault, not a crash — homed here because it
travels with per-OS sidecar packaging and consumes the build-time hash manifest;
the **runtime half of the T3 supply-chain threat**. Per-engine availability rows
are incrementally filled by P5–P7 as each sidecar is staged, mirroring the
SBOM-row pattern. **§3.4 patent-disposition matrix (single owner — decided here,
never re-decided downstream):** author the HEIC/AAC/H.264/AV1 ship-bundled /
rely-on-OS / gate / unavailable matrix, the §3.4.4a `engines.lock` per-platform
`available` boolean → `PatentDisposition` → `EngineHealth.unavailable_targets`
wiring (feeding the C12 contract declared in P2 and the §1.5 per-source default
availability) and the §3.4.5 per-platform packaging specifics, as **one**
cross-cutting deliverable — P5/P6 then only **read** the per-codec cell.
**Bundle-time build assertions (§6.1.3 — run by
`scripts/stage-engines`):** the **generic cross-cutting** assertions — the LGPL
shared-object-or-fail link assertion for the MIT core, the libvips-no-copyleft-PDF-loader
assertion, the libimagequant BSD-2-Clause leg-text + lockfile-pin provenance
check, the libheif-resolves-dav1d-for-AV1 wiring assertion, and the
exposed-parameter capability-assertion framework — belong here; the **per-engine**
assertion lists land in P5/P6/P7. **§3.9 binary-size-budget levers:** the size
engineering that keeps the build under the §3.9.2 ≤400 MB compressed ceiling
(LibreOffice strip help/l10n/dictionaries, CJK font subset-vs-full, pandoc
GHC-runtime weight, shared-lib dedup) is owned here with an **early baseline
measurement**, so P5–P7 each track their incremental size cost against the budget
rather than discovering overflow at release (the §6.7.2 release-time size *gate*
itself is in P10). **Generic UX-correctness primitives (so P5–P7 are UI-testable
end-to-end, not only at the Rust level):** the **advanced-options panel shell**
(into which P5–P7 register only per-format option DECLARATIONS, §1.6 — the panel
chrome is built once here), the **lossy/fidelity-note surfacing** mechanism in
FormatPicker (§2.9), **progress + cancel**, **result-actions / open-folder** flow,
the §2.8 **error / edge-state copy** framework, and the **structural-a11y wiring**
(ARIA roles + keyboard operability on DropZone/FormatPicker/DestinationBar/
ProgressList, focus management, wired via the P1 `a11y/` module). This keeps the
walking-skeleton philosophy literally true: P5–P7 register declarations against an
already-built UI, so a pair can reach its §6.5 `reliable` gate without waiting on
P8, and P8 stays a genuinely sequential pure-polish phase. P4 must **not**
re-implement `crate::fs_guard` (built in P3, §02 reference here covers only the
§2.12 isolation wrapper + §2.13 app-fault model).

**Spec home:** 03-engines-and-bundling (incl. §3.4 patent-disposition matrix,
§3.5.0 macOS TCC staging, §3.9 size-budget levers),
02-guarantees (§2.8/§2.9 UX-correctness primitives, §2.12/§2.13),
06-build-test-release (§6.4.3/§6.4.3a/§6.5 reliability machinery, SBOM scaffold,
§6.1.3 build assertions), 07-app-shell (§7.2.3 integrity-manifest generation +
startup verifier, §7.2.6 macOS TCC-staging requirement), 05-ui-ux (generic UX-correctness primitives: options-panel
shell, lossy notes, progress/cancel, error copy, structural a11y).

*Atomic `[ ]` steps: added in the fill pass.*

---

## P5 — Images (libvips family)

**Goal:** full image-category coverage on the proven harness — where "full
coverage" means each pair is backed by corpus files + per-pair integration tests
and marked **reliable** in the §6.5 ledger.

**Scope:** libvips core + libheif/x265, libaom (AVIF), librsvg (svgload),
ImageMagick magicksave delegate, cgif; all image conversions (both directions)
with per-format advanced options and patent-gated paths (reading the §3.4
disposition matrix homed in P4 — never re-deciding); resolves the ICO
build-spike (magicksave 256px/multi-size vs in-core Rust ICO fallback). Per-format
**advanced-option DECLARATIONS** (§1.6) are registered here against the P4-built
options-panel shell (no new panel chrome); the per-engine §6.1.3 build assertions
and the per-engine §7.2.3 availability rows are this phase's variants of the
generic frameworks built in P4. **Coverage
gate (§6.5):** for every pair, on all three available platforms, against every
§6.4.5 corpus file of its source format, the §6.4.3 per-pair integration test
passes and the pair is marked `reliable` in the pair-status ledger
(`reliability-report.json`) — the gate travels with the format work (§6.5.2
category-by-category). **Per-engine hardening:** librsvg loaded with **NO base
URL** (§3.5.5) so it resolves no local/remote references.

**Spec home:** 04-formats/images, 03-engines-and-bundling (§3.5.5 librsvg),
06-build-test-release (§6.4.3/§6.4.5 corpus, §6.5 ledger).

*Atomic `[ ]` steps: added in the fill pass.*

---

## P6 — Audio · Video · Cross-category (FFmpeg family)

**Goal:** full audio + video coverage and the cross-category conversions — "full
coverage" = each pair backed by corpus + per-pair integration tests and marked
**reliable** in the §6.5 ledger.

**Scope:** FFmpeg-backed audio and video conversions; cross-category
(extract-audio, to-GIF) with the specified guardrails; per-format advanced
options; A/V probing (FFprobe). AAC/H.264/AV1 target availability **reads** the
§3.4 patent-disposition matrix homed in P4 (per-codec cell, never re-decided).
Resolves the deferred cross-category items
(extract-audio target subset, to-GIF trim/caps). Per-format **advanced-option
DECLARATIONS** (§1.6) are registered here against the P4-built options-panel shell;
the per-engine §6.1.3 assertions and §7.2.3 availability rows are this phase's
variants of the generic P4 frameworks. **Coverage gate (§6.5):** every
pair backed by §6.4.5 corpus files + §6.4.3 per-pair integration tests + marked
`reliable` in the pair-status ledger on all three platforms. **Per-engine
hardening (§3.5.1):** FFmpeg built with the network-protocol family absent +
curated demuxers, `-protocol_whitelist file,pipe`, `concat -safe 1` (asserted at
§6.1.3) — closes the T9b SSRF/LFR egress surface for this engine.

**Spec home:** 04-formats/audio, 04-formats/video, 04-formats/cross-category,
03-engines-and-bundling (§3.5.1 FFmpeg hardening), 06-build-test-release (§6.5).

*Atomic `[ ]` steps: added in the fill pass.*

---

## P7 — Documents · Spreadsheets · Presentations (office family)

**Goal:** full document, spreadsheet and presentation coverage — "full coverage"
= each pair backed by corpus + per-pair integration tests and marked **reliable**
in the §6.5 ledger.

**Scope:** LibreOffice headless + poppler pdftotext + pandoc + native Rust
CSV/TSV; office / PDF / markup / spreadsheet / presentation conversions with
per-format advanced options; resolves the LibreOffice Markdown-import gate (else
MD→PDF parks). The **§3.5.6 native CSV/TSV engine is already built in P3** (the
walking skeleton) and is **not** re-implemented here — P7 only broadens its pairs
and adds the CSV-delimiter advanced option (paralleling the `fs_guard`-built-in-P3
disclaimer). Per-format **advanced-option DECLARATIONS** (§1.6) are registered
here against the P4-built options-panel shell. **Coverage gate (§6.5):** every pair backed by §6.4.5 corpus files
+ §6.4.3 per-pair integration tests + marked `reliable` in the pair-status ledger
on all three platforms. **Per-engine hardening:** pandoc `--sandbox` (§3.5.4);
LibreOffice disposable-profile + no remote-link auto-update (§3.5.2).

**Spec home:** 04-formats/documents, 04-formats/spreadsheets,
04-formats/presentations, 03-engines-and-bundling (§3.5.2/§3.5.4 hardening),
06-build-test-release (§6.5).

*Atomic `[ ]` steps: added in the fill pass.*

---

## P8 — UI/UX (full experience + visual polish)

**Goal:** the full designed experience, beyond the minimal walking-skeleton UI —
a genuinely **sequential** phase that may run after P5–P7 without blocking their
§6.5 DoD gates (the **generic** UX-correctness primitives — options-panel shell,
lossy-note surfacing, progress/cancel, result-actions, error copy, structural a11y
— are built once in **P4**, and the **per-format option declarations** are
registered by P5–P7 against that already-built UI as part of each engine phase, so
no engine phase is blocked on P8).

**Scope (i) ship-gating UI not owned elsewhere:** the **About + NOTICE
attribution** screen (release-blocking per SSOT) + Impressum, the
About→Releases user-initiated link (§7.6.2), settings chrome, and any cross-cutting
UI refinement of the §2.8 error / edge-state copy and §2.9 lossy/fidelity
presentation that is not a per-format declaration (those are registered in P5–P7).
**Scope (ii) visual-polish / branding pass — non-blocking, may trail P11:** modern
visual styling, Ne-IA branding, empty-state eye-candy (SSOT marks only
"modern/eye-candy" polish as non-blocking).

**Spec home:** 05-ui-ux, 02-guarantees (§2.8 fail-clearly, §2.9 lossy notes),
07-app-shell (§7.6.2 About→Releases link).

*Atomic `[ ]` steps: added in the fill pass.*

---

## P9 — Hardening (performance · validation · security · corpus)

**Goal:** the app meets its non-functional contracts and the deferred empirical
items are validated. (The strings module is built in P1 and the structural-a11y
wiring in P4, so this phase *validates* them, it does not introduce them.)

**Scope:** performance budgets; the **§6.4.6 headed-E2E infrastructure** itself —
wire up **`tauri-driver`** (Windows + Linux only), **WebdriverIO v9** +
**`@axe-core/webdriverio`**, author `wdio.conf.js` with the `tauri:options`
capabilities, and the **Linux `Xvfb` virtual-display** wiring (the scaffolding
that produces the validation outputs below); **a11y validation** (Lane-B
live-WebView axe-core scan + keyboard-path equivalence — the *foundations* were
built in P1/P4);
**§2.10 real-world filename/content fidelity** validation (adversarial-name unit
tests §6.4.1 + CJK/RTL/encoding corpus §6.4.5; the byte-verbatim-stem / path-limit
*mechanism* lives in the guarantees-fs layer of P2/P3 — **not** parked UI
localization, which the SSOT defers); decoder-isolation / fuzz validation (exercising the P4-built §2.12
boundary + the P5–P7 per-engine §3.5.x SSRF/LFR controls — no new isolation
mechanism introduced); threat-map verification; the **offline-egress observability gate** (§2.11.4 /
§6.7.3 — per-platform packet-monitor + egress-deny window proving zero outbound
packets + crafted-input engines cannot reach out or read out-of-input files
(T9a/T9b), plus the §6.4.2 adversarial-egress case, with zero-startup-network in
the same window); corpus validation of all `[DEFER: corpus]` items (resource
budgets, CJK font breadth, privilege-drop profile, etc.).

**Spec home:** 02-guarantees (§2.10, §2.11.4), 05-ui-ux (a11y validation),
06-build-test-release (§6.4 corpus, §6.4.6 headed-E2E infra, §6.7.3 egress gate).

*Atomic `[ ]` steps: added in the fill pass.*

---

## P10 — Release Mechanics (technical — downloads only)

**Goal:** users can **download a verified build** — the technical machinery only.
**No auto-update / phone-home:** `tauri-plugin-updater` is **absent**, no update
manifest, no updater endpoint/pubkey, capabilities grant **no remote origin**
(§7.6.1 — "its absence is the implementation"; assert it). Users learn of a new
release only via the user-initiated About→Releases link (§7.6.2, homed in P8).

**Scope:** per-OS bundles/builds (one artifact per platform); **release
checksums** (SHA-256 per asset + `SHA256SUMS`) and the **§6.2.3 minisign detached
signature over `SHA256SUMS`** (public key at `docs/minisign.pub`, private key as
the `MINISIGN_SECRET_KEY` CI secret, with the rotation policy) — the *only*
signing in scope; this is **not** binary code-signing/notarization (SSOT *Out of
Scope*). **SBOM finalization** (per-engine rows populated in P5–P7 now finalized)
+ integrity hashing; reproducible-ish builds (§6.2.5 — best-effort, **not** a
release gate); the **Lane-B GitHub Releases
pipeline**. **§6.2.4 download / trust-page content authoring** (pure technical
release mechanics enabling verified downloads — in scope): write the copy-paste
**verify-hash recipe** including the literal `minisign -Vm SHA256SUMS -p docs/minisign.pub`
step (lowercase **`-p` = public-key FILE PATH**; uppercase `-P` expects an inline base64
key string and would FAIL on a path — build-gates G39 RUNS this literal recipe so a broken
form fails the release) + the Windows WebView2 / Linux libfuse2 prerequisite notes, and the
**macOS Sequoia step-by-step Gatekeeper / per-sidecar-quarantine recovery**
instructions (the SSOT "reaching the user at the highest-risk moment"
trust-substitute) — so the P11 §6.6 walkthrough has authored content to validate.
**Lane-B in-scope gates assembled here:** the **§6.7.2 ≤400 MB compressed
artifact-size gate** (DoD row 22 — measure each platform's compressed artifact,
fail the release if any exceeds the §3.9.2 ceiling, publish measured sizes as a
release asset; the size *levers* are owned in P4) and the **§6.10 row 21
no-system-pollution post-launch assertion** (G43 — the load-bearing leg is a
**CLI-automatable before/after STATE snapshot-diff** on every OS: `reg export` of
HKCU+HKLM\SOFTWARE + file-system diff (Win) / `LaunchAgents`+`LaunchDaemons`+file-association
DB enumeration + `lsof +D` (macOS — NOT the SIP-blocked `fs_usage` DTrace live trace) /
`~/.local`+`~/.config`+desktop-dir diff (Linux); the live `strace`+inotify monitor is the
authoritative leg on Linux and informational-where-available on macOS/Windows — asserting no
registry/LaunchAgent/daemon/file-association writes and no writes outside config+log+chosen
output). **Release-blocking gates homed here:**
the §6.8 **governance-completeness** assertion (every required governance doc
present + non-stub) and the §6.9 **name/trademark clearance-record** assertion
(`docs/name-clearance.md` present, dated for the release line, verdict = clear) +
the dormant scripted **rename-propagation** machinery + old-name grep gate.
*Registering* a mark stays **out of scope**; only the in-repo clearance check +
any rename is in scope. *(Pure technical — no store/marketing.)*

**Spec home:** 06-build-test-release (§6.2 integrity, §6.2.4 download/trust-page
authoring, §6.3 SBOM, §6.7.2 Lane B incl. size gate, §6.10 row 21 no-pollution
gate, §6.8 governance gate, §6.9 clearance gate), 07-app-shell (§7.6 no-update
posture).

*Atomic `[ ]` steps: added in the fill pass.*

---

## P11 — Final E2E & Acceptance

**Goal:** the release candidate is verified end-to-end and signed off.

**Scope:** cross-platform E2E test matrix; the **§6.6 SSOT usability-floor
walkthrough** as its **six distinct, separately-faileable sub-gates** (each a
distinct atomic box in the fill pass): **(a)** a genuine non-developer conversion
walkthrough on ≥1 platform; **(b)** a keyboard-only pass (§5.10); **(c)** a
screen-reader smoke pass (§5.6.1 SR contract) on ≥1 platform; **(d)** the
mandatory macOS Sequoia first-launch + per-sidecar quarantine recovery sub-test;
**(e)** the `docs/usability-floor.md` artifact + its machine-checkable staleness
criterion (release-line/date match); **(f)** the single-instance double-extract
macOS sub-test. **Definition-of-Done verification against the SSOT** — confirms
the §6.5 reliability gate is green (every v1 pair `reliable` in the pair-status
ledger on all available platforms), the offline-egress observability gate (P9),
**§7.2.3 startup integrity & engine-presence (DoD gate 19 — missing/corrupt engine
yields an app-fault, not a crash)**, the **§6.7.2 ≤400 MB artifact-size gate (row
22)**, the **§6.10 row 21 no-system-pollution gate**, and the remaining
release-blocking gates (governance-completeness, name-clearance — P10) all pass;
RC sign-off. (Verification phase — the gate machinery itself is built in P2/P4–P10;
P11 only proves it green.)

**Spec home:** 06-build-test-release (§6.5/§6.6 usability floor/§6.10 DoD),
07-app-shell (§7.2.3 startup-integrity gate), SINGLE-SOURCE-OF-TRUTH.md
(Definition of Done).

*Atomic `[ ]` steps: added in the fill pass.*

---

## Notes for the box-fill pass (captured from skeleton review r3)

> Guidance for when atomic `[ ]` steps are written — not structural, not boxes.
> Lives here so it survives until the fill pass; strip once consumed.

- **P6:** add an internal §6.5 ledger sub-gate marking **audio pairs reliable
  before video pairs** are attempted (video on 3 platforms is the heaviest corpus
  run — gives measurable intra-phase progress).
- **P8:** make scope (i) ship-gating UI atomic boxes with a clear
  **"P8 ship-gating done"** sub-gate; label scope (ii) visual polish as a named
  **non-blocking** stretch (may continue after the P11 RC) so "P8 done for
  release" is unambiguous.
- **P4 / P1:** note that P4's threat-map assembly **back-fills** the
  `SECURITY.md` → §0.11 reference (P1 authors `SECURITY.md`; the §0.11 map is
  assembled in P4) so the "no orphan threat classes" contract stays auditable.
- **Owner call — early headed-E2E smoke:** a single thin `tauri-driver` smoke
  driving the P3 CSV→TSV slice through the real WebView (Win+Linux, no
  axe/contrast/corpus) could be pulled forward into P4 as an early integration
  probe. **Default (taken):** headed-E2E stays in P9 — defensible, since the §6.5
  gate keys on engine-level tests and P4's vitest-axe jsdom leg keeps the UI
  unit-testable. Flip if you'd rather probe the real WebView earlier.
