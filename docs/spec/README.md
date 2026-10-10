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
- **Derivation:** the implementation plan (`docs/plan/`) is derived from this
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

_Legend — **A** Architecture & app shell · **B** Core engine & guarantees · **C** Format coverage · **D** UI (06 spans all). **Read 00 and 07 together** — 07 is A-track foundational despite its file number._

## Conventions

### Tag glossary

One glossary for every spec file; per-file headers point here.

- `[DECIDED]` — binding, fixed here or by the SSOT. A short qualifier after a colon may say *what*
  is decided (`[DECIDED: dropped v1]`). Apart from the Co-Pilot ruling form below, a tag carries no
  date, no plan-box id and no other provenance.
- `[REC]` — an `[OPEN]` resolved with a recommended default; it ranks below `[DECIDED]`. It binds
  like `[DECIDED]`; the rank only orders a contradiction between the two (the pre-check of
  [roles-and-escalation §4(a)](../process/roles-and-escalation.md)).
- `[DEFER: post-v1]` — decided: out of v1 scope. It returns only through an SSOT scope change.
- `[DEFER: <what>]`, also written bare as `[DEFER]` — the design is decided; only the empirical
  value or real-world validation the sentence names remains, calibrated by the §6.5 corpus, the
  named spike or the implementing plan box. It is neither an owner question nor a licence to
  redesign.
- `[OPEN: P<n>.<m>]` — a live owner-level fork. The id is an open plan box that carries the fork:
  the box that rules it, or the first box whose build needs the answer. No other `[OPEN` spelling
  marks a live fork, and no `[PROPOSED` tag is written: a proposal is decided (`[DECIDED]`) or it
  is an `[OPEN: …]` fork. Every live fork has a row in the *Open-decision register* below.
- A Co-Pilot ruling reads `[DECIDED — Co-Pilot ruling <YYYY-MM-DD>]`, the date being the day its
  commit landed (it names the ruling: "the 2026-09-15 ruling"), and
  `[DECIDED — Co-Pilot ruling <YYYY-MM-DD>, owner-ratified]` once the owner ratified it. A ruling
  binds from landing and carries no reopen clause; the owner overturns it by an ordinary edit
  ([roles-and-escalation §4](../process/roles-and-escalation.md)).
- A decision label such as `[XCAT-A]` (cross-category), `[IMG-1]` (images) or `[PRES-1]`
  (presentations) names a decision so other files can cite it; the tag beside it states its
  status.

### Writing the spec

- The spec states current behaviour. Apart from the Co-Pilot ruling tag above, dates, plan-box
  ids, "formerly / earlier / was" narratives, superseded or correction blocks and review
  provenance live in git history and the commit body, never here.
- A ruling lands as one binding sentence in its owning §; every other § points to it instead of
  restating it.
- A rejected design gets at most one line: `Rejected: <design> — <reason>.`
- Code/identifiers in English; this doc in English (public OSS repo).

### Citing the spec

- Cite a § by its number, adding the heading or row name when the § is long
  (`§5.10 Ctrl/⌘+N row`). Stable ordinals (`§5.2 row 7a`, `§7.2.1 step 6`, `§0.6 invariant 6`)
  are part of the text: an edit keeps them, and a new one is appended, never inserted.
- Never cite a line number (`§N.N:NNN`, `<file>:NNN`, `line NNN`, `row NNN`): it rots on the
  next edit of the cited file.
- SSOT references by section *name* (e.g. *Never harm the original*).

## Foundational decisions (the "how" seeds)

- **Framework:** Tauri (Rust core + React/TS/Tailwind/Vite UI). `[DECIDED]`
- **Engine delivery:** bundle **everything**, fully offline, no runtime fetch. `[DECIDED]`
- **Licensing mechanism:** copyleft engines shipped as **separate, independently
  invoked binaries** (aggregation, not linking) so the MIT core stays clean;
  NOTICE/third-party-licenses + SBOM. `[DECIDED]`

## Open-decision register

> One row per live open fork in the spec (the glossary's open-fork tag): an owner-level call
> still to be made. A row names the fork, the § that owns it and the plan box its tag names,
> and it leaves the register in the commit that writes the decision into that § as
> `[DECIDED]`. A `[DEFER: …]` item (the design is decided; a measured number or a validation
> remains) lives only in its owning §; `git grep -n "\[DEFER" docs/spec` lists them.
> Resolved decisions live in their owning § as `[DECIDED]`; the retired open-questions log is
> read with `git show 15d8e29a5ac3:docs/spec/README.md`.

| Open item | Owning § | Deciding box |
|---|---|---|
