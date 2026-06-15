# ConvertIA — Plan Box Format (the machine-checkable spec `plan-lint` enforces)

> **The contract for every `[ ]` box in `docs/plan/`.** This file defines the
> *shape* of a plan box — its markers, its anatomy, its tags, and its dependency
> annotations — so that two readers agree on it: the **Build-Loop** session, which
> reads the plan to pick and build the next box (`build-loop.md` §3 step 1), and
> **`plan-lint`** (G7/G20), which mechanically rejects a malformed box on both
> enforcement planes (L1 pre-commit on a staged plan edit, L4 full-tree). The plan
> is an executable TODO, not prose: a box that does not parse cannot be selected or
> checked, so its format is a gate, not a style preference.
>
> **Conflict order (unchanged, every layer):**
> **SSOT > spec > security/process docs > plan > code > conversation.**
> When two layers disagree, the higher one wins — **never silently reconcile,
> always escalate**. This file is a *security/process doc*: it is **above** the plan
> it describes, so if a box in `P*.md` contradicts the format here, the box is wrong
> and `plan-lint` fails it.
>
> **Status: living.** A change to the box format is a change to a `plan-lint`
> contract — it is recorded **here first**, in the **same commit** as the `plan-lint`
> code that enforces it, so the linter never drifts from its own spec.

---

## 1. Why a fixed format

The Build-Loop is autonomous: it scans `docs/plan/P*.md`, finds the next buildable
box, reads the spec `§§` and gate IDs the box points at, builds, and checks the box
off — with **no human in the selection loop** (`build-loop.md` §0). Three properties
have to hold mechanically, or the loop either stalls or silently skips work:

1. **Selectable.** The loop must find *the* next box deterministically — lowest
   phase first, top to bottom, dependencies resolved (§6). An ambiguous or
   unparseable marker breaks selection.
2. **Self-describing.** A box must carry everything the loop needs to *route* the
   work — what kind of work it is (the **tag**, §4), where the acceptance criteria
   live (the **spec `§` / gate-id refs**, §3), and what it depends on (the
   **`needs:` annotation**, §5) — without the loop guessing. *(The acceptance
   criteria themselves live in the referenced spec `§§`, not in the box — the box
   is a pointer, the spec is the contract; `build-loop.md` §1.)*
3. **Auditable.** Every reference must resolve, every dependency target must exist,
   and the numbering must be gap-free — so a typo'd `§` or a dangling `needs:`
   surfaces as a `plan-lint` failure, not as a box the loop builds against thin air.

`plan-lint` enforces all three (§7). This file is the human-readable definition of
what it enforces.

---

## 2. Markers

Exactly **four** box markers exist. `plan-lint` (check: marker validity) rejects any
other bracketed token at a box position — a stray `[X]`, `[-]`, `[~]`, `[wip]`,
`[blocked]` or an empty `[]` **fails the lint**; there is no fifth state.

| Marker | Name | Meaning | Loop behaviour |
|---|---|---|---|
| `[ ]` | **open / buildable** | Not yet built. The unit of work. | The selection target (§6) — built when it is the next one and its `needs:` are all `[x]`. |
| `[x]` | **done** | Built, tested, dual-reviewed, committed, gates green. | Skipped (already done); may **unlock** a `[!]` box via `unlocked-by:` (§5). |
| `[!]` | **blocked-with-note** | Cannot be built **and is not a dependency to follow** — it waits on something the loop genuinely cannot produce. **Rare.** | **Skip + report** at the phase end; read the `>`-note under it. May be auto-flipped to `[ ]` by an `unlocked-by:` dep going `[x]` (§5). |
| `[!extern]` | **needs something external** | Waits on an **owner / external** action the loop cannot take (an off-repo asset, a human decision, an external dependency). **Very rare** for a fully-offline OSS app. | **Skip + collect** into the consolidated `[!extern]` list (the owner's action list, `build-loop.md` §9). **STOP** instead of skipping if a *non-extern* box names this one as a `needs:` prerequisite. |

> **`[!]` is the exception, not the tool of first resort — prefer dependency-
> following (DECISION C, §5).** When the next box needs an *unbuilt but buildable*
> box, the loop does **not** mark it `[!]` and move on (that is the
> block-and-skip model, which ConvertIA does **not** use, `build-loop.md` §3 step
> 2). It follows the `needs:` annotation, builds the prerequisite **in place**, and
> returns — leaving **no hole**. `[!]` / `[!extern]` are reserved for a block the
> loop **cannot resolve by building** (an owner action, an external input). The
> test: *can the loop build the thing it is blocked on?* If yes ⇒ it is a `needs:`
> dependency, not a `[!]`. If no ⇒ `[!]` (or `[!extern]` if the blocker is
> off-repo), with a one-line `>`-note saying **why** and (where applicable) an
> `unlocked-by:` marker (§5).

**Sub-box rule for `[x]`.** A box with sub-boxes (§3) is marked `[x]` **only after
every sub-box is `[x]`** — the top marker is the AND of its children. The loop
checks the top box off in the same check-off commit that flips the last sub-box
(`build-loop.md` §3 step 7). `plan-lint` (check: sub-box consistency) fails a `[x]`
top box that still has an open `[ ]`/`[!]` sub-box under it.

---

## 3. Box anatomy

A box is a single Markdown list item with a **fixed header line**, optional prose,
optional sub-boxes, and optional annotation lines:

```
- [ ] **P<phase>.<n>** [Tag] Short imperative title · <spec-§ refs> · <Gnn refs>
  needs: P<x>.<y>[, P<a>.<b> ...]          # optional; forward dependency (§5.1)
  unlocked-by: P<x>.<y>                     # only under a [!] box; reverse unlock (§5.2)
  > optional one-line note (the block, under a [!] / [!extern] box; §3.3)
  - [ ] **P<phase>.<n>.<m>** [Tag] Sub-box title · <spec-§ refs> · <Gnn refs>
  - [ ] **P<phase>.<n>.<m>** [Tag] Sub-box title · <spec-§ refs> · <Gnn refs>
```

The annotation lines (`needs:`, `unlocked-by:`, the `>`-note) and the sub-box bullets
all sit at the **same two-space indent** under the box header, but `plan-lint` reads
them by their **leading token** — `needs:` / `unlocked-by:` / `>` / `- [` — not by
indentation, so the order is unambiguous to the linter. For a human author the order
is fixed: **the annotation lines come first, in the order `needs:` → `unlocked-by:` →
`>`-note, before the first `- [` sub-box** (§5 states the ordering; the comments above
show the placement).

### 3.1 The header line — every field

`- [ ] **P<phase>.<n>** [Tag] Short title · <refs>`

| Field | Form | Rule |
|---|---|---|
| **List bullet** | `- ` | Markdown unordered-list dash + one space. The marker (`[ ]`/`[x]`/`[!]`/`[!extern]`) follows immediately. |
| **Box-id** | `**P<phase>.<n>**` | **Bold.** `<phase>` is the integer phase number (`0`..`11`); `<n>` is the box number within the phase, **1-based, gap-free** (§7). The id is the loop's stable handle and the inline-decision-tag suffix (`[Build-Session-Entscheidung: P5.4]`, CLAUDE.md §5). |
| **Tag** | `[Tag]` | Exactly one primary tag from the taxonomy (§4), in square brackets, right after the box-id. A second tag is allowed only as a comma-joined pair `[Tag,Tag2]` for a genuinely cross-cutting box (§4). |
| **Title** | short imperative phrase | One line, English (CLAUDE.md §8), imperative ("Wire …", "Author …", "Stage …"), no trailing period. Describes the *deliverable*, not the activity. |
| **Refs separator** | ` · ` | A space-bullet-space (`·`, U+00B7) separates the title from the references and the reference groups from each other. |
| **Spec-§ refs** | `§<n>.<...>` | Zero or more spec section references (`§2.1.2`, `§3.5.6`, `§0.10`). **Every one must resolve** to a real heading/anchor in `docs/spec/` (§7). The acceptance criteria live there. The **format-coverage track (`docs/spec/04-formats/`) is prose-anchored, not numbered** — its category files (`images.md`/`audio.md`/… ) carry `### <FORMAT>` slug headings, no numbered `§4.x` sections — so a box citing a per-format/per-pair coverage CONTRACT uses the **`§04/<file>#<slug>` anchor form** (e.g. `§04/images.md#png`, `§04/audio.md#mp3`), which `plan-lint` resolves to a real `### ` heading anchor in that category file (§7). A bare `§4` / `§4.x` token does **not** resolve (there is no numbered §4 tree) and **fails** — the resolvable coverage ref is always the `§04/<file>#<slug>` form. A conversion-behaviour box that implements a per-pair acceptance fact (which sources→targets, the per-pair lossy classification, the per-source default target) **should carry the `§04/<file>#<slug>` ref alongside** its `§3.x` engine ref / `§6.x` test ref so the builder routes to the coverage contract, not only the engine/test spec. |
| **Gate-id refs** | `G<nn>` | Zero or more gate IDs (`G31`, `G47`, `G54`). **Every one must resolve** to a row in [`build-gates.md`](../security/build-gates.md) (§7). A box that *builds* or *activates* a gate names it; a box merely *governed by* a gate need not. |

A box **must reference at least one** spec `§` **or** gate id (DoD item (a):
"spec-`§` or gate-id referenced … or deliberately marked tooling-only";
`build-loop.md` §5). A pure-tooling box that legitimately has neither carries the
literal `· tooling-only` token in the refs position so the absence is **declared,
not accidental** — `plan-lint` treats a box with no ref and no `tooling-only` token
as malformed. `tooling-only` and a real ref are **mutually exclusive**: it *declares
the absence* of a ref, so a box carrying a `§` or a `Gnn` must **not** also carry
`tooling-only`, and `plan-lint` (§7, reference resolution) fails the combination.

### 3.2 Sub-boxes

A box that decomposes into ordered steps lists them as **indented** child boxes:

- Indentation is **two spaces** per level under the parent bullet (`  - [ ]`).
  `plan-lint` (check: sub-box consistency) rejects ragged/odd indentation.
- The sub-box-id **extends the parent** with a third dotted segment:
  `P<phase>.<n>.<m>`, `<m>` 1-based and gap-free under that parent (§7).
- Sub-boxes are worked **strictly top to bottom**; the top box is checked off only
  when **all** sub-boxes are `[x]` (§2). The **dual review fires once per top box**
  over the combined sub-box diff, **never per sub-box** (`build-loop.md` §3 step 2 /
  §6 box-batching).
- A sub-box carries its own tag + refs and may itself carry a `needs:` (§5). Nesting
  is **at most one level deep** (`P<phase>.<n>.<m>`) — a box that wants three levels
  is two boxes, not a grandchild; `plan-lint` rejects a fourth dotted segment.

### 3.3 The `>`-note

A Markdown blockquote (`  > …`) directly under a box records a fact the
header cannot carry — for a blocked box, **what** the block is. Each `>`-note is a
**single line**, but a box may carry **more than one** consecutive single-line
`>`-note (the established forward-ref convention pairs a structured
`> **Forward-ref note (DECISION-C ordering inversion):** …` line with the box's
descriptive `>`-note — both single-line, stacked). `plan-lint` (§7) enforces **no
"exactly one `>`-note" cap**; it parses each `>`-leading line independently. The requirement is
the **either-or** the linter enforces (§5.2, §7 "annotation pairing"): a `[!]` /
`[!extern]` box must not be a **silent** block, so it carries a `>`-note **or** an
`unlocked-by:` (or both). Concretely:

- **`[!extern]`** has no loop-releasable `unlocked-by:` (it waits on an owner /
  external action, not on a buildable box), so its `>`-note is **mandatory** — it is
  the only thing that documents the block.
- **`[!]`** must carry the `>`-note **unless** an `unlocked-by:` already names the
  releaser; in practice a clear `[!]` box carries **both** — the `unlocked-by:` for
  the auto-unlock scan and a one-line `>`-note saying why (§5.2).

Under an open `[ ]` box the note is optional (a clarifying constraint, a phasing note
like "`→ activated in P1`"). Notes are **prose, not parsed for acceptance criteria** —
those are in the spec `§§`.

---

## 4. Tag taxonomy

A box carries **exactly one** primary tag telling the loop what *kind* of work it
is — which DoD items bite, which test levels apply, which spec home it lives in. The
set is intentionally **small and closed**; `plan-lint` (check: tag validity) rejects
any tag outside it.

| Tag | Covers | Typical DoD / test emphasis |
|---|---|---|
| `[DOC]` | A doc/spec/process/security artifact — the SSOT-derived spec, a security/process doc, a governance file (`SECURITY.md`, `NOTICE`, `THIRD-PARTY-LICENSES`), a runbook. | Living-doc sync (DoD (b)); `plan-lint`/`spec-lint` clean; **no** code tests. |
| `[GATE]` | A guardrail itself — a `plan-lint`/`spec-lint` check, a custom gate script, a `cargo-deny`/`gitleaks`/Semgrep rule, a fastpath detector. | Ships its **G24 positive+negative self-test** (a planted violation MUST fail it); registered for `plan-lint` check 16. |
| `[CI]` | A `.github/` workflow or CI-plumbing change — a job, the matrix, runner binding, token scope, `dependabot.yml`, branch/tag-protection config assertions. | `actionlint`/`zizmor` clean (G49/G50); pinned-by-SHA actions; least-privilege `permissions:`. |
| `[RUST]` | Rust core / `convertia-imgworker` / xtask code — the pipeline, `crate::fs_guard`/`crate::detect`/`crate::isolation`, an IPC command, an in-core engine. | `clippy -D warnings` + no-panic policy; unit/property/fuzz; `deny(unsafe_code)` outside the FFI module (G29). |
| `[UI]` | WebView code — React 19 / TypeScript / Tailwind, a component, the strings module, a11y wiring, the generated `bindings.ts` consumer side. | `tsc` strict / eslint / no `any`; vitest + jsdom `vitest-axe` (G33a); English-only (G57). |
| `[BUILD]` | Engine staging, bundling, per-OS packaging, `engines.lock`, SBOM rows, size budget, the build toolchain. | `engines.lock` + SBOM row (DoD (g)); per-engine build assertions (G37/G38); link/license assertions. |
| `[TEST]` | Test infrastructure / methodology — a corpus, a fixture set, a harness, the reliability ledger, a per-pair integration runner. | Output-validity readers (G31/G32); corpus integrity (G24a); determinism (pinned seed/locale). |
| `[RELEASE]` | Release-plane mechanics — checksums, the minisign step, attestation, the GitHub Releases pipeline, the download/trust page, release-blocking acceptance gates. | L5 acceptance (G39/G44/G58); release-tier ratchets; no auto-update posture (§7.6.1). |

**Cross-cutting boxes** (e.g. a gate that is *also* a CI job) use a **comma-joined
pair** `[GATE,CI]` — the **first** tag is primary (it drives the loop's routing).
`plan-lint` allows at most two tags and requires both ∈ the taxonomy. Prefer a single
tag; reach for the pair only when the box genuinely lives in two homes.

---

## 5. Dependency annotations — `needs:` and `unlocked-by:`

ConvertIA has **one coherent dependency vocabulary, two directions** (CLAUDE.md §2;
`build-loop.md` §3 step 2). Both live on their own line directly under the box
header, before any `>`-note or sub-box.

### 5.1 `needs:` — the forward dependency (DECISION C)

```
- [ ] **P5.7** [BUILD] Stage libheif/x265 for HEIC read · §3.5.5 · G37 G38
  needs: P4.3, P4.9
```

`needs: P<x>.<y>[, ...]` declares that this box **requires** the listed box(es) to
be `[x]` first. It is what makes a forward dependency **detectable** — and detection
is the whole point of **DECISION C, dependency-following**:

> **If the next buildable box has a `needs:` pointing at a box that is not yet
> `[x]`, the loop does NOT skip and does NOT leave a hole. It builds that
> prerequisite box first (recursively — following *its* `needs:` too), then RETURNS
> and builds the original box.** The plan is dependency-*following*, not
> dependency-stepping-over.

This **replaces a `[!]`-block-and-skip model**, which ConvertIA does not use:
that model marks a box `[!]` and moves on, leaving a hole to be filled later out of order.
ConvertIA resolves the dependency **in place**.

- `needs:` targets are **other box-ids** (`P<x>.<y>` or a sub-box `P<x>.<y>.<z>`),
  comma-separated. **Every target must exist** in the plan (§7) — a dangling
  `needs:` fails `plan-lint` (check: needs-targets exist).
- A `needs:` on a **buildable** box is **followed, never escalated**
  (`roles-and-escalation.md` §4 "NOT escalation"). The loop only escalates when the
  prerequisite is something it **cannot build** — an `[!extern]` prerequisite of a
  non-extern box, or an all-blocked deadlock (`roles-and-escalation.md` §4(d)).
- A `needs:` must not point **forward in a way that creates a cycle** — `plan-lint`
  (check: needs-acyclic) fails a dependency cycle, since the loop could not resolve
  it. Pointing at a *later-phase* box is allowed (DECISION C builds it early), but a
  cycle is not.
- Distinguish from a `[!]`: a `needs:` says *"build that first, then me"*; a `[!]`
  says *"I cannot be built at all right now"*. The same fact is **never** expressed
  as both — if a box is genuinely blocked on an unbuildable thing it is `[!]` /
  `[!extern]` with a `>`-note, **not** `[ ]` with a `needs:` the loop cannot satisfy.

### 5.2 `unlocked-by:` — the reverse direction (auto-unlock)

```
- [!] **P9.4** [TEST] Headed-E2E axe-core contrast scan · §6.4.6 · G33b
  unlocked-by: P9.1
  > blocked: needs the tauri-driver + WebdriverIO harness (P9.1) standing first.
```

`unlocked-by: <box-id>` sits under a **`[!]`** box and names the box whose
completion **releases** it. After every check-off (and at session start) the loop
runs the **auto-unlock scan**: for each `[!]` box carrying an `unlocked-by:` whose
dep is now `[x]`, it flips `[!]` → `[ ]` (`build-loop.md` §3 step 1 / step 7). This
makes a box that an earlier (e.g. P0-bootstrap) session left `[!]`-blocked
selectable again automatically, without a manual edit.

- `needs:` and `unlocked-by:` are **inverses**: `needs:` = "this box **requires**
  that one" (the box names *its* prerequisites); `unlocked-by:` = "this box, when
  done, **releases** that one" (the blocked box names *its* releaser). One
  vocabulary, two directions.
- **Which to use.** Prefer **`needs:`** on a *buildable* `[ ]` box — DECISION C
  follows it in place, the normal case. Use **`unlocked-by:`** only on a genuinely
  `[!]`-blocked box that becomes buildable the moment a *named, scheduled* box lands
  — it is the auto-unblock marker, not a substitute for dependency-following.
- **The deciding test (worked example, §9):** `P5.4` is `[!]` + `unlocked-by: P6.1`,
  **not** `[ ]` + `needs: P6.1`, because the cross-decoder re-validation is genuinely
  **un-buildable** until the FFmpeg sidecar (`P6.1`) exists — there is nothing for the
  loop to build at `P5.4` yet, so it is a skip-and-report block, not a dependency to
  follow. Had `P5.4` merely needed `P6.1` *staged as an input* to a step it can run,
  it would be `[ ]` + `needs: P6.1`, and DECISION C would build `P6.1` early and
  return. The test is always §2's: *can the loop build the thing it is blocked on?*
  Yes ⇒ `needs:`; no ⇒ `[!]` / `[!extern]`.
- `unlocked-by:` appears **only** under a `[!]` box; `plan-lint` (check: marker /
  annotation pairing) fails an `unlocked-by:` under a `[ ]`/`[x]`/`[!extern]` box. It
  fails a **silent block**: a `[!]` box that carries **neither** a `>`-note **nor**
  an `unlocked-by:`, and a `[!extern]` box that carries **no** `>`-note (an
  `[!extern]` has no `unlocked-by:`, so its note is the only documentation of the
  block — §3.3). A `[!]` box with an `unlocked-by:` and no separate `>`-note is
  **valid** (the `unlocked-by:` names the releaser), though a `>`-note is encouraged.

---

## 6. How the loop selects the next box

The Build-Loop's selection algorithm (`build-loop.md` §3 step 1), stated against
this format so a box author knows exactly how their box will be picked:

1. **Scan all `docs/plan/P*.md`, lowest phase first, top to bottom.** Phase order is
   numeric (`P1` before `P2` … before `P11`); within a file, document order. The
   loop's range is **`P1`..`P11`** — **`P0` is bootstrapped manually** (DECISION B,
   `build-loop.md` §0); a loop reaching a `P0.x` box is out of range and stops.
2. **The target is the first `[ ]` box** in that scan that is **not**
   `[!]`/`[!extern]` — the document-order-next open box, *before* checking its deps.
   (The scan picks the target by position; Step 3 then resolves its dependencies — the
   two are separate phases, so a target with an unmet `needs:` is not skipped over.)
3. **If the target's `needs:` deps are all `[x]`** → build it now. **If it has a
   `needs:` dep not yet `[x]`** (but **buildable**) → **DECISION C:** build that
   prerequisite first (recurse on *its* `needs:`), then **return** to the target.
   Never skip, never hole.
4. **`[!extern]`** → skip + collect into the owner's action list; **STOP** if a
   non-extern box hard-requires it. **`[!]`** → read the `>`-note, skip, mention at
   the phase end.
5. **Sub-boxes** are worked top to bottom under their parent before the parent is
   checked off (§2, §3.2).
6. **Zero open boxes** → emit the convergence report and **stop** (never loop
   forever); a genuine all-blocked deadlock → escalate (`build-loop.md` §3 step 1).

Because selection is deterministic, the **numbering and reference integrity that
`plan-lint` enforces (§7) are load-bearing** — a gap in numbering or a dangling
`needs:` would make "the first open box, deps resolved" ambiguous or unsatisfiable.

---

## 7. What `plan-lint` checks about this format

`plan-lint` (G7/G20) runs on both planes — **L1 pre-commit** on a staged plan edit,
**L4 full-tree fail-closed** — and is itself unit-tested (its checks ship fixtures;
`plan-lint` check 16 / the G24 self-test discipline). The **format-specific** checks
this file defines (distinct from the doc-wide consistency checks 5–24 catalogued in
`build-gates.md` §6, which `plan-lint` also runs) are:

- **Marker validity** — every box marker ∈ `{[ ], [x], [!], [!extern]}`; no fifth
  state, no empty `[]`, no stray token at a box position (§2).
- **Sub-box consistency** — two-space-per-level indentation; a `[x]` top box has no
  open `[ ]`/`[!]` sub-box; nesting at most one level deep (§2, §3.2).
- **Header well-formedness** — `- <marker> **P<phase>.<n>** [Tag] Title · <refs>`:
  bold gap-free box-id, exactly one (or a two-tag) taxonomy tag, the ` · ` refs
  separator, a non-empty title (§3.1, §4).
- **Tag validity** — every tag ∈ the §4 taxonomy; at most two, comma-joined (§4).
- **Reference resolution** — **every `§<...>` resolves** to a real spec
  heading/anchor in `docs/spec/`, and **every `G<nn>` resolves** to a row in
  `build-gates.md`; a box with no ref carries the explicit `· tooling-only` token,
  and a box that **does** carry a ref must **not** also carry `tooling-only` (the two
  are mutually exclusive, §3.1). A typo'd `§`, a dangling `Gnn`, or a `tooling-only`
  token alongside a real ref **fails** — the loop never builds against a phantom
  reference, and `tooling-only` always means a genuine, declared absence.
  **Format-coverage anchor leg (`§04/<file>#<slug>`):** a coverage-track ref of the
  form `§04/<file>#<slug>` (§3.1) resolves by checking that `docs/spec/04-formats/<file>`
  exists **and** contains a `### ` heading whose GitHub-style slug equals `<slug>` — so
  the per-format/per-pair acceptance contract a box implements is a *resolvable* anchor,
  not a filename-only reference. A bare `§4` / `§4.x` token **fails** (there is no
  numbered §4 tree in `04-formats/`); a `§04/<file>#<slug>` whose file or slug does not
  exist **fails**. This gives the coverage track (track C) the same resolvable-anchor
  guarantee the numbered `§0`–`§3`/`§5`–`§7` tracks already have. (The gate that BUILDS
  this leg + its G24 self-test is the P4 coverage-anchor `[GATE]` box; this format change
  is recorded here in the same commit as that box per the format-change protocol below.)
- **`needs:`-targets exist** — every `needs:` box-id is a real box in the plan; the
  graph is **acyclic** (§5.1). A dangling or cyclic `needs:` fails. **`plan-lint`
  loads ALL phase files — `P0`..`P11` — when resolving `needs:` targets** (even though
  the Build-Loop's *execution* scan is `P1`..`P11`, §6): a later phase that activates a
  `P0`-authored gate may carry `needs: P0.x` (trivially satisfied, since `P0` is `[x]`
  before the loop reaches `P1`), so a `needs: P0.x` edge must resolve, not dangle. The
  acyclicity check likewise spans `P0`..`P11`.
- **Annotation pairing** — `unlocked-by:` appears **only** under a `[!]` box and
  names a real box; no blocked box is **silent** — a `[!]` box carries a `>`-note
  **or** an `unlocked-by:`, and a `[!extern]` box carries a mandatory `>`-note (it
  has no `unlocked-by:`); an open `[ ]`/`[x]`/`[!extern]` box carries no
  `unlocked-by:` (§3.3, §5.2).
- **Numbering gap-free** — within each phase the box numbers `P<phase>.1, .2, …` are
  **1-based and contiguous** (no gap, no duplicate), and sub-box numbers
  `P<phase>.<n>.1, .2, …` likewise under their parent (§3.1, §3.2). A gap would make
  "the next box" ambiguous and could hide a dropped box.

> **Format change protocol.** Adding or changing a marker, a tag, an annotation, or
> a numbering rule is a change to the above checks. It is authored **here first**
> and in the **same commit** as the `plan-lint` code that enforces it, with a `[GATE]`
> box and the gate's G24 self-test updated — so this spec and its linter never
> disagree (the living-doc rule, the doc header).

---

## 8. Per-phase file + index convention

The plan is **split per phase**, indexed by a README:

| File | Holds |
|---|---|
| [`docs/plan/README.md`](README.md) | **The index.** The phase skeleton (`P0`..`P11`) — each phase's goal, scope, spec home, and a link to its phase file — plus the sequencing philosophy (walking-skeleton-first) and the conflict rule. It carries **no atomic `[ ]` boxes**; it is the map, not the territory. |
| [`docs/plan/P0-build-and-security.md`](P0-build-and-security.md) | **P0** — the bootstrap phase (clusters P0.1–P0.7). Built **manually** (DECISION B), not by the loop. |
| `docs/plan/P<n>-<slug>.md` | **One file per later phase** (`P1-foundation-and-scaffolding.md`, …, `P11-final-e2e-and-acceptance.md`). The atomic `[ ]` boxes for that phase live here, added in the fill pass. |

- **File naming:** `P<n>-<kebab-slug>.md`, the slug from the phase title in the
  index (`P5 — Images` → `P5-images.md`). One phase, one file; the loop's
  lowest-phase-first scan (§6) is a numeric sort over these file names then document
  order within each.
- **Box-ids are phase-scoped, not file-scoped** — `P5.4` is box 4 of phase 5
  regardless of which file it physically lives in (they coincide by convention: each
  phase = one file). The phase number in the id and in the file name agree;
  `plan-lint` (numbering check) treats each phase's boxes as one contiguous sequence.
- **The index never carries `[ ]` boxes**, and a phase file never restates the
  index's scope prose — each fact has **one home** (the doc-consistency discipline
  in `build-gates.md` §6). A box belongs to **exactly one** phase file.

---

## 9. A worked example

A small, well-formed slice illustrating every construct (illustrative box-ids/refs):

```markdown
## P5 — Images (libvips family)

- [x] **P5.1** [BUILD] Stage libvips core + cgif offline · §3.5.5 · G37 G38
- [ ] **P5.2** [RUST] Wire libvips raster→raster through the isolation boundary · §2.12 §1.7 · G29 G31
  needs: P4.3
  - [x] **P5.2.1** [RUST] imgworker raster decode/encode command · §3.5.5 · G31
  - [ ] **P5.2.2** [TEST] Per-pair integration: png→webp output-validity · §6.4.3 §6.5 · G32
- [ ] **P5.3** [UI] Register webp-quality advanced-option declaration · §1.6 §2.9 · G47
- [!] **P5.4** [TEST] Cross-library AVIF re-validate via ffprobe · §6.4.5 · G32
  unlocked-by: P6.1
  > blocked: needs the FFmpeg sidecar (P6.1) staged for the cross-decoder check.
- [ ] **P5.5** [DOC] Record the ICO build-spike outcome in this plan's notes · tooling-only
```

Reading it the way the loop does: `P5.1` is done; the next open box is `P5.2`, which
`needs: P4.3` — if `P4.3` is not `[x]`, the loop builds `P4.3` first (DECISION C),
then returns to `P5.2` and works its sub-boxes `P5.2.1` → `P5.2.2` before checking
`P5.2` off; `P5.3` follows; `P5.4` is `[!]`-blocked with a note and will auto-flip
to `[ ]` when `P6.1` is `[x]`; `P5.5` is a pure-tooling doc box with **neither** a spec
`§` **nor** a gate id, so it carries the explicit `· tooling-only` token (§3.1) to
declare that absence — a box with a real `§` would *not* carry `tooling-only`, since
the two are mutually exclusive. Numbering is gap-free (`.1`–`.5`; sub-boxes `.1`–`.2`);
every `§` and `Gnn` resolves; every tag is in the taxonomy; the one `tooling-only`
box carries no ref. `plan-lint` passes it.

---

## 10. References

- The loop that reads this format (selection, DoD, hard-stops, the reviewer rubric):
  [`build-loop.md`](../process/build-loop.md) — §3 step 1 (selection), step 2
  (DECISION C / sub-boxes), §0 (P1..P11 range / DECISION B).
- Who follows a `needs:` vs who escalates a block:
  [`roles-and-escalation.md`](../process/roles-and-escalation.md) §4.
- The gate that enforces this format (`plan-lint`, G7/G20) + the doc-wide
  consistency checks 5–24: [`build-gates.md`](../security/build-gates.md) §6.
- Project rules / the `needs:` ↔ `unlocked-by:` vocabulary (DECISION C) / DoD
  summary: [`CLAUDE.md`](../../CLAUDE.md) §2.
- The plan index + the phase skeleton this format fills: [`README.md`](README.md).
- The P0 box areas that author this file (P0.1) + the gate framework (P0.2):
  [`P0-build-and-security.md`](P0-build-and-security.md).
- SSOT (what & why): [`docs/SINGLE-SOURCE-OF-TRUTH.md`](../SINGLE-SOURCE-OF-TRUTH.md).
