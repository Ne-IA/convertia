# ConvertIA — Implementation Plan (Phase 3)

> The implementation roadmap, derived from the [Specification](spec/README.md)
> (which is itself derived from the [Single Source of Truth](SINGLE-SOURCE-OF-TRUTH.md)).
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
**vertical slice early, then horizontal breadth**.

---

## P0 — RESERVED (owner)

> Intentionally left for the owner to define before the box-fill pass. Not
> touched by the skeleton review.

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
state machine + domain types + error model as contracts.

**Spec home:** 00-architecture, 01-conversion-pipeline, 07-app-shell.

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

**Spec home:** 01-conversion-pipeline (incl. §1.2 detection), 02-guarantees
(§2.1/§2.7 fs_guard), 05-ui-ux (minimal).

*Atomic `[ ]` steps: added in the fill pass.*

---

## P4 — Engine & Bundling Framework

**Goal:** the reusable harness every format engine plugs into, plus per-OS
bundling, the **generic** security/isolation layer, and the cross-cutting
reliability test machinery format phases plug pairs into.

**Scope:** generalized engine-invocation layer; per-OS sidecar packaging &
bundling (everything offline); the isolated **image-worker** process
(`convertia-imgworker`, §3.5.5 — the first real sidecar) + the §2.12 isolation
boundary; resource budgets/limits; the **generic isolation framework only** —
the §2.12 decoder-isolation wrapper, the §0.9 subprocess pool, the §2.12.3
best-effort privilege-drop tier, and threat-map assembly/ownership (§0.11).
**Per-engine SSRF/LFR hardening** (FFmpeg protocol-whitelist §3.5.1, pandoc
`--sandbox` §3.5.4, LibreOffice profile-hardening §3.5.2, librsvg-no-base-URL
§3.5.5) lives in its **engine phase** (P5/P6/P7), not here. **Reliability
harness:** build the cross-cutting machinery format phases plug pairs into — the
per-pair integration runner (§6.4.3), the §6.5.2 **pair-status ledger**
generator (`reliability-report.json`), and the §6.4.3a corpus↔pair **bijection
guard** (`scripts/check-corpus-coverage.rs`). **SBOM + NOTICE /
third-party-licenses:** tooling/schema **scaffold only** — per-engine rows are
populated in P5–P7 as each engine is staged; finalization is P10. P4 must **not**
re-implement `crate::fs_guard` (built in P3, §02 reference here covers only the
§2.12 isolation wrapper + §2.13 app-fault model).

**Spec home:** 03-engines-and-bundling, 02-guarantees (§2.12/§2.13),
06-build-test-release (§6.4.3/§6.4.3a/§6.5 reliability machinery, SBOM scaffold).

*Atomic `[ ]` steps: added in the fill pass.*

---

## P5 — Images (libvips family)

**Goal:** full image-category coverage on the proven harness — where "full
coverage" means each pair is backed by corpus files + per-pair integration tests
and marked **reliable** in the §6.5 ledger.

**Scope:** libvips core + libheif/x265, libaom (AVIF), librsvg (svgload),
ImageMagick magicksave delegate, cgif; all image conversions (both directions)
with per-format advanced options and patent-gated paths; resolves the ICO
build-spike (magicksave 256px/multi-size vs in-core Rust ICO fallback). **Coverage
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
options; A/V probing (FFprobe). Resolves the deferred cross-category items
(extract-audio target subset, to-GIF trim/caps). **Coverage gate (§6.5):** every
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
MD→PDF parks). **Coverage gate (§6.5):** every pair backed by §6.4.5 corpus files
+ §6.4.3 per-pair integration tests + marked `reliable` in the pair-status ledger
on all three platforms. **Per-engine hardening:** pandoc `--sandbox` (§3.5.4);
LibreOffice disposable-profile + no remote-link auto-update (§3.5.2).

**Spec home:** 04-formats/documents, 04-formats/spreadsheets,
04-formats/presentations, 03-engines-and-bundling (§3.5.2/§3.5.4 hardening),
06-build-test-release (§6.5).

*Atomic `[ ]` steps: added in the fill pass.*

---

## P8 — UI/UX (correctness layer + polish)

**Goal:** the full designed experience, beyond the minimal walking-skeleton UI —
split into a DoD-gating **UX-correctness layer** and a late **visual-polish** pass.

**Scope (i) UX-correctness layer — DoD ship gates, delivered with / immediately
after the format phases P5–P7** so each engine phase is testable end-to-end (not
only at the Rust level): per-format **advanced-options panels** that affect output
(quality/effort/encoding/CSV delimiter), the **lossy/fidelity notes** surfaced in
FormatPicker (§2.9), **progress + cancel**, **result actions / open-folder** flow,
the §2.8 **error / edge-state copy**, the **About + NOTICE attribution** screen
(release-blocking per SSOT) + Impressum, and the **structural accessibility
foundations** (ARIA roles on interactive primitives, keyboard-operable
DropZone/FormatPicker/DestinationBar/ProgressList, focus management — §5.6/§5.10,
wired via the P1 `a11y/` module) so a11y is not retrofitted onto a finished UI.
**Scope (ii) visual-polish / branding pass — may stay late:** modern visual
styling, Ne-IA branding, empty-state eye-candy, settings chrome (SSOT marks only
"modern/eye-candy" polish as non-blocking).

**Spec home:** 05-ui-ux, 02-guarantees (§2.8 fail-clearly, §2.9 lossy notes).

*Atomic `[ ]` steps: added in the fill pass.*

---

## P9 — Hardening (performance · validation · security · corpus)

**Goal:** the app meets its non-functional contracts and the deferred empirical
items are validated. (Structural a11y + the strings module are built earlier —
P1/P8 — so this phase *validates* them, it does not introduce them.)

**Scope:** performance budgets; **a11y validation** (Lane-B live-WebView axe-core
scan + keyboard-path equivalence — the *foundations* were built in P1/P8);
**§2.10 real-world filename/content fidelity** validation (adversarial-name unit
tests §6.4.1 + CJK/RTL/encoding corpus §6.4.5; the byte-verbatim-stem / path-limit
*mechanism* lives in the guarantees-fs layer of P2/P3 — **not** parked UI
localization, which the SSOT defers); decoder-isolation / fuzz validation;
threat-map verification; the **offline-egress observability gate** (§2.11.4 /
§6.7.3 — per-platform packet-monitor + egress-deny window proving zero outbound
packets + crafted-input engines cannot reach out or read out-of-input files
(T9a/T9b), plus the §6.4.2 adversarial-egress case, with zero-startup-network in
the same window); corpus validation of all `[DEFER: corpus]` items (resource
budgets, CJK font breadth, privilege-drop profile, etc.).

**Spec home:** 02-guarantees (§2.10, §2.11.4), 05-ui-ux (a11y validation),
06-build-test-release (§6.4 corpus, §6.7.3 egress gate).

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
+ integrity hashing; reproducible-ish builds; the **Lane-B GitHub Releases
pipeline**. **Release-blocking gates homed here:** the §6.8
**governance-completeness** assertion (every required governance doc present +
non-stub) and the §6.9 **name/trademark clearance-record** assertion
(`docs/name-clearance.md` present, dated for the release line, verdict = clear) +
the dormant scripted **rename-propagation** machinery + old-name grep gate.
*Registering* a mark stays **out of scope**; only the in-repo clearance check +
any rename is in scope. *(Pure technical — no store/marketing.)*

**Spec home:** 06-build-test-release (§6.2 integrity, §6.3 SBOM, §6.7.2 Lane B,
§6.8 governance gate, §6.9 clearance gate), 07-app-shell (§7.6 no-update posture).

*Atomic `[ ]` steps: added in the fill pass.*

---

## P11 — Final E2E & Acceptance

**Goal:** the release candidate is verified end-to-end and signed off.

**Scope:** cross-platform E2E test matrix; the SSOT usability-floor
walkthrough(s); **Definition-of-Done verification against the SSOT** — confirms
the §6.5 reliability gate is green (every v1 pair `reliable` in the pair-status
ledger on all available platforms), the offline-egress observability gate (P9)
and all release-blocking gates (governance-completeness, name-clearance — P10)
pass; RC sign-off. (Verification phase — the gate machinery itself is built in
P4–P10; P11 only proves it green.)

**Spec home:** 06-build-test-release (§6.5/§6.10 DoD), SINGLE-SOURCE-OF-TRUTH.md
(Definition of Done).

*Atomic `[ ]` steps: added in the fill pass.*
