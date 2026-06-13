# 06 — Build, Test & Release

> The technical build/test/release pipeline (software-side only — no store/account
> logistics). Origin: SSOT *v1 Definition of Done*, *Distribution & download
> trust*, *Cross-platform, one product*.

## 6.1 Build matrix
- One artifact per platform (Win `.exe`/portable, macOS `.dmg`/app, Linux
  AppImage/binary) from one codebase; how engines get bundled per platform; CI
  runners (cross-compile vs native). _(expand)_

## 6.2 Reproducibility & integrity
- Published integrity hashes per release; canonical GitHub Releases; optional
  reproducible-build intent; how a user verifies. _(expand)_

## 6.3 SBOM & licence artifacts
- SBOM generation step; NOTICE / third-party-licenses assembly; attribution
  completeness gate (release-blocking). _(expand)_

## 6.4 Test strategy
- Unit (Rust core, guarantees), integration (per-pair conversions), the
  **real-world input corpus** (real photos, Office docs incl. non-Latin/RTL,
  audio/video) and how a pair is declared "reliable"; no-harm/fail-clearly
  property tests; cross-platform test runs. _(expand)_

## 6.5 The reliability gate (DoD operationalised)
- How "every pair works reliably on all 3 platforms" is measured; the corpus as
  precondition; recording the two permissible exceptions (patent per-platform;
  reliability demotion) as release-note items. _(expand)_

## 6.6 Usability-floor check
- The informal non-developer walkthrough per platform (SSOT DoD); what it must
  cover to count. _(expand)_

## 6.7 CI/CD
- Pipeline stages, where the matrix builds + tests run, artifact publishing.
  _(expand)_

## 6.8 Repo governance & policy artifacts
- The concrete in-repo deliverables the SSOT mandates: `LICENSE` (MIT),
  `NOTICE`/third-party-licenses (data from §3.7), `CONTRIBUTING`, `CODE_OF_CONDUCT`,
  `SECURITY` (private vuln reporting — ties to §2.12/§7.5), `PRIVACY`
  (plain-language §2.11 restatement), `TRADEMARK.md`, and the **DCO posture**
  (no CLA; optional sign-off + the inbound-warranty clause). Each = a Phase-3
  authoring task. _(expand)_

## 6.9 Name/trademark clearance gate + rename propagation
- The SSOT hard, release-blocking gate: clearance for **both** "ConvertIA" and
  the public "Ne-IA" brand is a precondition before first public release; if a
  conflict surfaces, the mechanical **rename across repo / LICENSE / NOTICE /
  TRADEMARK / branding** is applied before release, never after. This is an
  **in-scope release gate** (the *process* of registering a mark is not our
  concern; *doing the check + propagating a rename* is). _(expand)_

## 6.10 DoD-traceability checklist
- One table mapping every SSOT *v1 Definition of Done* gate (offline, basic
  a11y, core-UX-flow, unwritable/ephemeral fallback, engine attribution,
  name/trademark clearance, usability-floor) → its owning spec section, so the
  README's "every behaviour has a home" claim is verifiable. Marks each gate
  in-scope-gate vs out-of-scope-process. _(fill table)_
