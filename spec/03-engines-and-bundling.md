# 03 — Engines & Bundling

> Which engines do the work, how they are selected, and how they ship — all
> bundled, fully offline. Origin: SSOT *Engine-license policy*, *Local/private/
> offline*, *v1 Definition of Done* (offline floor).

## 3.1 Engine inventory `[OPEN→DECIDED per format]`
- Candidate engines per category (e.g. FFmpeg, libvips/ImageMagick, LibreOffice,
  Ghostscript/poppler, pandoc, …) with licence + platform notes. Cross-ref
  04-formats. _(expand)_

## 3.2 Engine registry & selection
- Abstract `Engine` interface (trait crate? — §0.7); registry; selection per
  (source, target); fallbacks; capability declaration. _(expand)_
- **Single-engine decision:** every v1 (source→target) pair is satisfied by
  **one** engine — **no multi-step/chained conversions in v1**. Verify no listed
  pair is left unreachable by this rule. If chaining ever enters scope it needs
  its own home (intermediate scratch, per-step progress, step-attributed errors);
  not v1. _(expand)_

## 3.3 Bundling model (all offline) `[DECIDED: bundle everything]`
- Tauri sidecar/resource bundling; per-platform engine binaries; how the build
  assembles them; no runtime fetch. _(expand)_

## 3.4 Per-platform packaging & the patent-disposition matrix `[OPEN]`
- Win/macOS/Linux specifics per engine. **Single owner of the HEIC/AAC patent
  decision** the SSOT mandates: an explicit **format × platform × disposition**
  table (ship-bundled / gate / rely-on-OS / unavailable). "Rely on the OS" is a
  distinct strategy from bundling, with its own offline/isolation implications —
  note them. The `images.md`/`audio.md` patent flags and §6.5 **reference** this
  table; honest per-platform availability flows from it. Tracked in the
  open-questions log until decided. _(decide & expand)_

## 3.5 Per-engine argument construction (concrete)
- **Only** the per-engine concretes: argument construction (FFmpeg vs
  LibreOffice vs poppler/Ghostscript vs pandoc …), working dir, env, the engine's
  progress-signal format, and its exit-code/`stderr` quirks. The generic
  invocation lifecycle (spawn / progress / cancel / timeout / error-mapping) is
  owned by §1.7; every invocation routes through the §2.12 isolation wrapper.
  _(expand)_

## 3.6 Copyleft isolation `[DECIDED]`
- GPL/LGPL engines as separate invoked binaries (aggregation, not linking); how
  the MIT core stays clean; written-offer-of-source obligation handling. _(expand)_

## 3.7 Licence surfacing
- **Owns generation** of the NOTICE / third-party-licenses data + SBOM (source,
  format, build step). In-app **presentation** of that listing is owned by §5.9
  (About) — this section produces the data, §5.9 displays it. _(expand)_

## 3.8 Engine maintenance & versioning
- Pinning, update/patch posture (best-effort, not a gate), how a bumped engine is
  re-validated against the corpus. _(expand)_

## 3.9 Binary size budget
- Expected per-platform size with everything bundled (incl. the heavy office
  engine); what dominates; any compression/trimming. _(expand)_
