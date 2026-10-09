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
> When two layers disagree, the higher one wins — **never silently**: a difference the
> pre-check in [roles-and-escalation.md](../process/roles-and-escalation.md) §4(a) can rank
> is reconciled in the open, in the same commit; anything it cannot rank escalates. This
> file is a *security/process doc*: it is **above** the plan
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
| `[!extern]` | **needs something external** | Waits on an **owner / external** action the loop cannot take (an off-repo asset, a human decision, an external dependency — plus the **standing per-phase Co-Pilot hardening-sweep box**, [test-strategy §11](../process/test-strategy.md#11-the-phase-end-co-pilot-hardening-sweep)). Each owner act is its own `[!extern]` box; a phase also carries at most one caged-preconditions act (test-strategy §11.4). | **Skip + collect** into the phase's owner-act batch (`plan-lint --report owner-acts --phase <n>`). A box whose `needs:` closure reaches it is not selectable until it is `[x]` (§6, computed by `plan-lint --next`). |

> **`[!]` is the exception, not the tool of first resort.** When the next box needs an
> unbuilt but buildable box, the loop follows the `needs:` edge and builds it first
> (DECISION C, `build-loop.md` Step 2) — no hole. `[!]`/`[!extern]` are for a block the
> loop cannot resolve by building: a separable owner or external act is its own
> `[!extern]` box that the dependent box names in `needs:`; a box with nothing to build
> yet is itself `[!]` (or `[!extern]`), with a one-line `>`-note saying why and, when a
> scheduled box releases it, an `unlocked-by:` (§5.2).

**Sub-box rule for `[x]`.** A box with sub-boxes (§3) is marked `[x]` **only after
every sub-box is `[x]`** — the top marker is the AND of its children, an `[!extern]`
sub-box included. The loop
checks the top box off in the box commit that completes the last sub-box
(`build-loop.md` Step 7). `plan-lint` (sub-box consistency, §7) fails a `[x]` top box
over a `[ ]`, `[!]` or `[!extern]` sub-box.

---

## 3. Box anatomy

A box is a single Markdown list item with a **fixed header line**, optional prose,
optional sub-boxes, and optional annotation lines:

```
- [ ] **P<phase>.<n>** [Tag] Short imperative title · <spec-§ refs> · <Gnn refs>
  needs: P<x>.<y>[, P<a>.<b> ...]          # optional; forward dependency (§5.1)
  unlocked-by: P<x>.<y>                     # only under a [!] box; reverse unlock (§5.2)
  l-neg1: <route>                           # optional; the caged-file route (§5.3)
  > optional one-line note (the block, under a [!] / [!extern] box; §3.3)
  - [ ] **P<phase>.<n>.<m>** [Tag] Sub-box title · <spec-§ refs> · <Gnn refs>
  - [ ] **P<phase>.<n>.<m>** [Tag] Sub-box title · <spec-§ refs> · <Gnn refs>
```

The annotation lines (`needs:`, `unlocked-by:`, `l-neg1:`, the `>`-note) and the sub-box
bullets all sit at the **same two-space indent** under the box header, but `plan-lint`
reads them by their **leading token** — `needs:` / `unlocked-by:` / `l-neg1:` / `>` /
`- [` — not by indentation, so the order is unambiguous to the linter. For a human author
the order is fixed: **the annotation lines come first, in the order `needs:` →
`unlocked-by:` → `l-neg1:` → `>`-note, before the first `- [` sub-box** (§5 states the
ordering; the comments above show the placement).

### 3.1 The header line — every field

`- [ ] **P<phase>.<n>** [Tag] Short title · <refs>`

| Field | Form | Rule |
|---|---|---|
| **List bullet** | `- ` | Markdown unordered-list dash + one space. The marker (`[ ]`/`[x]`/`[!]`/`[!extern]`) follows immediately. |
| **Box-id** | `**P<phase>.<n>**` | **Bold.** `<phase>` is the integer phase number (`0`..`11`); `<n>` is the box number within the phase, **1-based, gap-free** (§7). The id is the loop's stable handle and the inline-decision-tag suffix (`[Build-Session-Entscheidung: <box-id>]`, roles-and-escalation §3). |
| **Tag** | `[Tag]` | Exactly one primary tag from the taxonomy (§4), in square brackets, right after the box-id. A second tag is allowed only as a comma-joined pair `[Tag,Tag2]` for a genuinely cross-cutting box (§4). |
| **Title** | short imperative phrase | One line of at most 160 characters, English (CLAUDE.md §8), imperative ("Wire …", "Author …", "Stage …"), no trailing period. Describes the *deliverable*, not the activity. |
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
- Sub-boxes are worked top to bottom; an `[!extern]` or `[!]` sub-box is skipped and
  the sub-boxes after it stay buildable when their own closure is clear. The top box
  is checked off only when **all** sub-boxes are `[x]` (§2). One review per commit: a
  box, or a sub-box that meets the DoD on its own (`build-loop.md` Step 2).
- A sub-box carries its own tag + refs and may itself carry a `needs:` (§5). Nesting
  is **at most one level deep** (`P<phase>.<n>.<m>`) — a box that wants three levels
  is two boxes, not a grandchild; `plan-lint` rejects a fourth dotted segment.
- A sub-box is **never a selection target of its own** (§6). It inherits its parent's
  own `needs:` — never its siblings' — and adds its own edges. A box that `needs:` the
  parent waits for every sub-box, an `[!extern]` one included (§2); a box that needs
  only the buildable part names that sub-box. A parent whose own `needs:` closure is
  blocked blocks every sub-box under it (the P4.44 / P4.46 shape).
- An owner act is a top-level `[!extern]` box (P4.56.3 is the one legacy `[!extern]`
  sub-box).

### 3.3 The `>`-note

A Markdown blockquote (`  > …`) directly under a box records a fact the header cannot
carry — for a blocked box, **what** the block is. Each `>`-note is a single line; a box
may carry several consecutive ones, and `plan-lint` (§7) parses each `>`-leading line
independently. A `[!]` / `[!extern]` box is never a **silent** block: it carries a
`>`-note **or** an `unlocked-by:` (or both), the either-or `plan-lint` enforces (§5.2,
§7 "annotation pairing"):

- **`[!extern]`** has no loop-releasable `unlocked-by:` (it waits on an owner /
  external action, not on a buildable box), so its `>`-note is **mandatory** — it is
  the only thing that documents the block.
- **`[!]`** must carry the `>`-note **unless** an `unlocked-by:` already names the
  releaser; in practice a clear `[!]` box carries **both** — the `unlocked-by:` for
  the auto-unlock scan and a one-line `>`-note saying why (§5.2).

**A note on a box that is not `[x]` carries only:** the scope, in and out, in one line; a
decided literal the spec does not carry (a pre-decided choice, a measured fact the build
needs), in one line, pointing to its home; for a `[!]`/`[!extern]` box, what the block
waits on.

**Everything else lives elsewhere.** A normative choice (a decided fork, a ruling, a
mechanism) goes to its spec `§` or gate row in the same commit, and the box keeps a
one-line pointer — a diet never drops a ruling. An implementation choice the Loop may
make itself is not written: the Loop tags it `[Build-Session-Entscheidung: <box-id>]` at
the code site. Evidence, provenance, review records and the tests a commit added go to
the commit body. A dependency is the `needs:` line, never a forward-ref note:
`plan-lint --show` prints the box's unmet needs and the roots that block its closure. A
caged-file route is the `l-neg1:` line (§5.3).

**Size.** The notes of a box that is not `[x]` have a budget of 1,500 characters in total,
wrapped one idea per `>` line, no line over 400 characters. The budget is a target, never
a reason to drop binding content (what a note carries, above, and its ruling pointers): a
box still over it once its normative choices sit in their spec homes keeps that content,
and the commit that leaves it over the budget acknowledges that with the reason. No
calendar date appears in a phase file: git is the clock, and a ruling is cited by its spec
home, box id or short SHA.

**Reference, never restate.** A note names the deciding `§`
and quotes at most the decided literal it depends on; it does not paraphrase the
mechanism. A paraphrase is a second copy that drifts — the spec-restatement class
`plan-lint` check 30 polices inside the spec is authorial here — and every drift is a
plan-vs-spec difference the loop must reconcile mid-box (the spec wins,
roles-and-escalation §4(a)). A reference names a `§` (plus the heading or row name when the
`§` is long), never a line number: the spec README *Citing the spec* rule. The pre-fill
readiness check (test-strategy §11.4) deletes restated prose in the boxes it audits. A note
carries neither a reopen clause nor the Co-Pilot ruling tag of the spec's tag glossary
([spec README](../spec/README.md) *Tag glossary*): that tag is dated, so it lives in the
ruling's spec home, and the note cites the ruling as the **Size** paragraph says.

**A delivered box** carries no delivery note: its commit names the box id in the subject,
and `plan-lint --show <id>` lists that commit. A deviation from the box as written is
recorded in its spec home in the same commit, and the box keeps at most one `>` line
pointing there. A box whose delivery notes predate these rules may be condensed to the
single note `> Delivered: <short-sha>` once every live obligation they carry has its home
(a spec `§`, a gate row, an open box or a residual-ledger line). A note that a `plan-lint`
check reads is never condensed; each such check names its box in its own code.

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
| `[RUST]` | Rust core / `convertia-imgworker` / xtask code — the pipeline, `crate::fs_guard`/`crate::detection`/`crate::isolation`, an IPC command, an in-core engine. | `clippy -D warnings` + no-panic policy; unit/property/fuzz; `deny(unsafe_code)` outside the FFI module (G29). |
| `[UI]` | WebView code — React 19 / TypeScript / Tailwind, a component, the strings module, a11y wiring, the generated `bindings.ts` consumer side. | `tsc` strict / eslint / no `any`; vitest + jsdom `vitest-axe` (G33a); English-only (G57). |
| `[BUILD]` | Engine staging, bundling, per-OS packaging, `engines.lock`, SBOM rows, size budget, the build toolchain. | `engines.lock` + SBOM row (DoD (g)); per-engine build assertions (G37/G38); link/license assertions. |
| `[TEST]` | Test infrastructure / methodology — a corpus, a fixture set, a harness, the reliability ledger, a per-pair integration runner. | Output-validity readers (G31/G32); corpus integrity (G24a); determinism (pinned seed/locale). |
| `[RELEASE]` | Release-plane mechanics — checksums, the minisign step, attestation, the GitHub Releases pipeline, the download/trust page, release-blocking acceptance gates. | L5 acceptance (G39/G44/G58); release-tier ratchets; no auto-update posture (§7.6.1). |

**Cross-cutting boxes** (e.g. a gate that is *also* a CI job) use a **comma-joined
pair** `[GATE,CI]` — the **first** tag is primary (it drives the loop's routing).
`plan-lint` allows at most two tags and requires both ∈ the taxonomy. Prefer a single
tag; reach for the pair only when the box genuinely lives in two homes.

---

## 5. Box annotations — `needs:`, `unlocked-by:` and `l-neg1:`

ConvertIA has **one coherent dependency vocabulary, two directions** (§5.1, §5.2; the
loop follows it in `build-loop.md` §3 steps 1–2), plus **one routing annotation**,
`l-neg1:` (§5.3). Each lives on its own line directly under the box header, before any
`>`-note or sub-box.

### 5.1 `needs:` — the forward dependency (DECISION C)

```
- [ ] **P98.7** [BUILD] Stage the example decoder · §3.5.5 · G37 G38
  needs: P98.3, P99.2
```

`needs: P<x>.<y>[, ...]` declares that this box **requires** the listed box(es) to
be `[x]` first. It is what makes a forward dependency **detectable** — and detection
is the whole point of **DECISION C, dependency-following**:

> `needs:` is followed, never skipped: the loop builds an unbuilt prerequisite first and
> returns (DECISION C, `build-loop.md` Step 2).

- `needs:` targets are **other box-ids** (`P<x>.<y>` or a sub-box `P<x>.<y>.<z>`),
  comma-separated. **Every target must exist** in the plan (§7) — a dangling
  `needs:` fails `plan-lint` (check: needs-targets exist).
- A `needs:` on a **buildable** box is **followed, never escalated**
  (`roles-and-escalation.md` §4 "NOT escalation"). The loop only escalates when the
  prerequisite is something it **cannot build** — an `[!extern]` prerequisite of a
  non-extern box, or an all-blocked deadlock (`roles-and-escalation.md` §4(d)).
- A `needs:` must not point **forward in a way that creates a cycle** — `plan-lint`
  (check: needs-acyclic) fails a dependency cycle, since the loop could not resolve it.
  Pointing at a *later-phase* box is allowed (DECISION C builds it early), but a cycle
  is not. A top box counts as needing its sub-boxes (its `[x]` is their AND, §2) and a
  sub-box as needing its parent's body (§3.2), so a sub-box that needs a box whose
  closure needs the parent is a cycle, and so is a box that needs a sub-box while the
  parent needs that box.
- Distinguish from a `[!]`: a `needs:` says *"build that first, then me"*; a `[!]`
  says *"I cannot be built at all right now"*. The same fact is **never** expressed
  as both — if a box is genuinely blocked on a separable owner/external act, that act is
  its own `[!extern]` box (with its `>`-note) and the blocked box names it in `needs:`
  (§2, §6 step 4 — the loop STOPs for that closure); if there is nothing to build AT the
  box yet, the box itself is `[!]` / `[!extern]` with its releaser in `unlocked-by:` (the
  §9 `P98.4` shape, §5.2). A `[ ]` box never carries a `needs:` that points at nothing
  buildable AND nothing `[!extern]`.

### 5.2 `unlocked-by:` — the reverse direction (auto-unlock)

```
- [!] **P98.4** [TEST] Cross-decoder re-validate · §6.4.5 · G32
  unlocked-by: P99.1
  > blocked: needs the P99.1 sidecar staged first.
```

`unlocked-by: <box-id>` sits under a **`[!]`** box and names the box whose
completion **releases** it. At every iteration start (`build-loop.md` Step 1) the loop
runs the **auto-unlock scan**: for each `[!]` box carrying an `unlocked-by:` whose
dep is now `[x]`, it flips `[!]` → `[ ]`; its flips ride in the next box commit. This
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
- **The deciding test (worked example, §9):** `P98.4` is `[!]` + `unlocked-by: P99.1`,
  **not** `[ ]` + `needs: P99.1`, because the cross-decoder re-validation is genuinely
  **un-buildable** until the `P99.1` sidecar exists — there is nothing for the loop to
  build at `P98.4` yet, so it is a skip-and-report block, not a dependency to follow.
  Had `P98.4` merely needed `P99.1` *staged as an input* to a step it can run, it would
  be `[ ]` + `needs: P99.1`, and DECISION C would build `P99.1` early and return. The
  test is always §2's: *can the loop build the thing it is blocked on?* Yes ⇒ `needs:`;
  no ⇒ the *blocker* gets the marker — its own `[!extern]` box named in `needs:` when
  the owner act is separable (§5.1), else `[!]` / `[!extern]` on the blocked box itself
  when there is nothing to build AT the box yet — the `P98.4` shape, whose releaser is
  named in `unlocked-by:` rather than `needs:`.
- `unlocked-by:` appears **only** under a `[!]` box; `plan-lint` (check: marker /
  annotation pairing) fails an `unlocked-by:` under a `[ ]`/`[x]`/`[!extern]` box. It
  fails a **silent block**: a `[!]` box that carries **neither** a `>`-note **nor**
  an `unlocked-by:`, and a `[!extern]` box that carries **no** `>`-note (an
  `[!extern]` has no `unlocked-by:`, so its note is the only documentation of the
  block — §3.3). A `[!]` box with an `unlocked-by:` and no separate `>`-note is
  **valid** (the `unlocked-by:` names the releaser), though a `>`-note is encouraged.

### 5.3 `l-neg1:` — the caged-file route

```
- [ ] **P98.3** [BUILD] Stage the example engine · §3.5.5 · G37
  needs: P98.2
  l-neg1: same-push
```

A box whose work edits a file in the L(-1) cage (`scripts/l-neg1-files.toml`,
CLAUDE.md §5) states how those caged bytes land before the Loop selects it; the Loop
never authors a caged line (G71). The line carries exactly one route:

| Route | Meaning |
|---|---|
| `same-push` | The Loop builds and stages the uncaged part, parks the box and escalates; the Co-Pilot adds the caged part under owner ack and makes the box's final commit (`build-loop.md` Step 7). |
| `act <box-id>` | A named `[!extern]` act lands the caged bytes — an owner-act box or the phase's caged-preconditions act (test-strategy §11.4); this box also names it in `needs:`. |
| `sweep-tail` | The caged tail reds nothing without it and lands at the phase-end sweep act. |
| `none` | The box names a caged file only as a reference. |

`act` and `sweep-tail` are the plan-data form of the DoD's pre-declared caged half
(`build-loop.md` §5). An `[!extern]` box carries no `l-neg1:`. A box without an
`l-neg1:` line that meets a caged path takes the `build-loop.md` Step 7 park.
`plan-lint --report owner-acts --phase <n>` lists the phase's `same-push`, `act` and
`sweep-tail` boxes that are not `[x]`. The tails the phase-end sweep lands belong to `[x]`
boxes, so the sweep finds them by their `l-neg1:` lines (test-strategy §11.2).

---

## 5a. Editing the plan

An edit to a phase file follows these rules:

- A new box takes the next free number of its phase (max+1) and sits at its build
  position, before the phase-end sweep box (`plan-lint` check 31, `build-gates.md` §6,
  fails a later box that does not `needs:` the sweep). Existing boxes may move within
  their phase file; ids never change.
- Never renumber, never delete: a duplicate or obsolete box becomes `[x]` with the single
  note `> RECONCILE: <where the work lives, or why it is void>` and drops its annotation
  lines (`needs:`, `unlocked-by:`, `l-neg1:`; a box after its phase's sweep box keeps its
  `needs:` on the sweep, check 31); the header stays as written, and a box whose `needs:`
  or `unlocked-by:` names it re-points that edge to where the work lives.
- Edges live on the consumer: a box names its own load-bearing prerequisites; no box
  carries edges on behalf of others. An edge into an earlier phase that ends in its sweep
  box (every phase from `P2` on) is implied by the phase chain — `P<n+1>.1` needs the
  `P<n>` sweep (check 31) — and is not written, except an edge to a `[!]` box (it can
  stay open past its phase's sweep, test-strategy §11.2) and the edges of a box that an
  earlier-phase box reaches through `needs:` (a DECISION C early build can run before
  that sweep, test-strategy §11.3).
- Owner acts are top-level `[!extern]` boxes (§3.2).
- Split an over-grained box by extracting the leaf the consumer needs into its own box
  at max+1 and giving the consumer a `needs:` on it; never defer the consumer.
- Read a box from its header line: `scripts/plan-lint --show <id>`.
- A format change follows the §7 protocol.

---

## 6. How the loop selects the next box

The single home of the Build-Loop's selection algorithm (`build-loop.md` Step 1 adds
the loop's actions), stated against this format so a box author knows exactly how their
box will be picked. `scripts/plan-lint --next` computes this algorithm from plan data
plus the parked set (`<git common dir>/parked/`, the `build-loop.md` §6 park procedure)
and prints the answer: a parked box to resume or the target, with its unmet
prerequisites in build order, the skipped closures with their roots and the pending
`unlocked-by:` flips.

1. **Scan all `docs/plan/P*.md`, lowest phase first, top to bottom.** Phase order is
   numeric (`P1` before `P2` … before `P11`); within a file, document order. The
   loop's range is **`P1`..`P11`** — **`P0` is bootstrapped manually** (DECISION B,
   `build-loop.md` §0); the loop never builds a `P0.x` box, and an open one in a `needs:`
   closure stops that closure like an `[!extern]` box (step 4).
2. **The target is the first `[ ]` box** in that scan that is **not**
   `[!]`/`[!extern]` — the document-order-next open box, *before* checking its deps.
   (The scan picks the target by position; Step 3 then resolves its dependencies — the
   two are separate phases, so a target with an unmet `needs:` is not skipped over.)
3. **If the target's `needs:` deps are all `[x]`** → build it now. **If it has a
   `needs:` dep not yet `[x]`** (but **buildable**) → **DECISION C:** build that
   prerequisite first (recurse on *its* `needs:`), then **return** to the target.
   Never skip, never hole.
4. **`[!extern]`** → skip + collect into the per-phase owner-act batch; a non-extern box
   that names it anywhere in its `needs:` closure → **STOP for that closure** (report
   once, collect) and continue with the next open box outside it (`build-loop.md` §3
   step 1) — except the phase-end sweep box, which blocks its whole successor phase
   (test-strategy §11.3). **`[!]`** → read the `>`-note, skip, mention at the phase end.
   A **parked** box (the `build-loop.md` §6 park procedure) is skipped with its closure
   like an `[!extern]` box. A resumable park — a `<box-id>.patch.released` file or a
   `stop` park — is answered first, in plan order, when its box is selectable: open and
   in range (steps 1–2), its own `needs:` closure clear and, for a parent, a buildable
   sub-box (step 5). It carries its build order (step 3); the sweep box's block of its
   successor phase does not apply to it, since a park there is a DECISION C early build
   (test-strategy §11.3). Otherwise it is skipped with its roots, the box itself when it
   is not open or out of range.
5. **Sub-boxes** are worked top to bottom under their parent; each inherits the
   parent's own `needs:`, never a sibling's, and an `[!extern]`/`[!]` sub-box is
   skipped (§3.2).
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
this file defines (distinct from the doc-wide consistency checks catalogued in
`build-gates.md` §6, which `plan-lint` also runs) are:

- **Box parse completeness** — a line that looks like a box header but does not parse
  fails, so a malformed box cannot evade the other checks.
- **Marker validity** — every box marker ∈ `{[ ], [x], [!], [!extern]}`; no fifth
  state, no empty `[]`, no stray token at a box position (§2).
- **Sub-box consistency** — two-space-per-level indentation; a `[x]` top box has no
  `[ ]`, `[!]` or `[!extern]` sub-box; nesting at most one level deep (§2, §3.2).
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
  exists under that exact name **and** contains a `### ` heading outside a code fence whose slug
  by plan-lint's `_slug` (the check-2 rule for intra-doc anchors; close to, not identical with,
  GitHub's) equals `<slug>` — so
  the per-format/per-pair acceptance contract a box implements is a *resolvable* anchor,
  not a filename-only reference. A bare `§4` / `§4.x` token or a zero-padded `§0N` token (it
  numbers a spec file title, never a section) **fails**; a `§04/<file>#<slug>` whose file or slug
  does not exist **fails**; and every whitespace-separated header token carrying a `§` or a gate
  id must be whole — `§<n>[.<n>…][a-z]`, `§04/<file>#<slug>` or `G<n>[a-z]` — so an incomplete
  anchor or a glued tail (`§04/images.md#png#jpg`, `§1.7#foo`, `G7#foo`) **fails**. This gives the coverage track (track C) the same resolvable-anchor
  guarantee the numbered `§0`–`§3`/`§5`–`§7` tracks already have. (The leg + its G24
  self-test landed at P4.60.3, recorded here first per the protocol below. `_slug`:
  lowercase, every character except word characters, spaces and hyphens dropped, whitespace collapsed to one hyphen, edge hyphens stripped — `### JPG / JPEG` → `jpg-jpeg`, `### PNG` → `png`.)
- **`needs:`-targets exist** — every `needs:` box-id is a real box in the plan; the
  graph is **acyclic** (§5.1; a top box counts as needing its sub-boxes). A dangling or
  cyclic `needs:` fails. **`plan-lint` loads ALL phase files — `P0`..`P11` — when
  resolving `needs:` targets** (even though the Build-Loop's *execution* scan is
  `P1`..`P11`, §6): a later phase that activates a `P0`-authored gate may carry
  `needs: P0.x` (trivially satisfied, since `P0` is `[x]` before the loop reaches
  `P1`), so a `needs: P0.x` edge must resolve, not dangle. The acyclicity check
  likewise spans `P0`..`P11`.
- **Annotation pairing** — `unlocked-by:` appears **only** under a `[!]` box and
  names a real box; no blocked box is **silent** — a `[!]` box carries a `>`-note
  **or** an `unlocked-by:`, and a `[!extern]` box carries a mandatory `>`-note (it
  has no `unlocked-by:`); an open `[ ]`/`[x]`/`[!extern]` box carries no
  `unlocked-by:` (§3.3, §5.2).
- **`l-neg1:` annotation** — at most one line, a §5.3 route; an `act` target exists, is
  `[!extern]` (or `[x]`) and is in the box's `needs:`; none under an `[!extern]` box.
- **Numbering gap-free** — within each phase the box numbers `P<phase>.1, .2, …` are
  **1-based and contiguous** (no gap, no duplicate), and sub-box numbers
  `P<phase>.<n>.1, .2, …` likewise under their parent (§3.1, §3.2). A gap would make
  "the next box" ambiguous and could hide a dropped box.

> **Format change protocol.** Adding or changing a marker, a tag, an annotation, a
> numbering rule or a check above is authored **here first**, in the **same commit** as
> the `plan-lint` code that enforces it and its G24 legs. That commit is a Loop `[GATE]`
> box or a Co-Pilot L(-1) act; `build-gates.md` §6 names every check.

---

## 8. Per-phase file + index convention

The plan is **split per phase**, indexed by a README:

| File | Holds |
|---|---|
| [`docs/plan/README.md`](README.md) | **The index:** the conflict rule, how the plan is used, the sequencing philosophy and one table row per phase (goal, file, spec homes). No `[ ]` boxes and no scope prose. |
| [`docs/plan/P0-build-and-security.md`](P0-build-and-security.md) | **P0** — the bootstrap phase (clusters P0.1–P0.7). Built **manually** (DECISION B), not by the loop. |
| [`docs/plan/residual-ledger.md`](residual-ledger.md) | **The residual ledger** — what boxes and Co-Pilot acts left outside their work at hand (CLAUDE.md §6), one plain bullet each, triaged per phase at the sweep (test-strategy §11.2). No `[ ]` boxes; the loop never builds from it. |
| `docs/plan/P<n>-<slug>.md` | **One file per later phase** (`P1-foundation.md`, …, `P11-acceptance.md`): the atomic `[ ]` boxes for that phase; its header carries the phase's scope and exit criterion. |

- **File naming:** `P<n>-<kebab-slug>.md` (e.g. `P5-images.md`); the slug is a short
  name, not the full phase title. One phase, one file; the loop's lowest-phase-first
  scan (§6) is a numeric sort over these file names then document order within each.
- **Box-ids are phase-scoped, not file-scoped** — `P5.4` is box 4 of phase 5
  regardless of which file it physically lives in (they coincide by convention: each
  phase = one file). The phase number in the id and in the file name agree;
  `plan-lint` (numbering check) treats each phase's boxes as one contiguous sequence.
- **Granularity:** P2 lands one box per domain type or contract; P5–P7 one box per
  code-path, with each pair's corpus, test and ledger row a box of its own.
- A box belongs to **exactly one** phase file (the doc-consistency discipline in
  `build-gates.md` §6). **The index never carries `[ ]` boxes** and never restates a
  phase's scope: each fact has **one home**, the phase header.

---

## 9. A worked example

A small, well-formed slice of one phase. Every example box in this file carries a
fictional id from phases `P97`–`P99`, outside the real `P0`..`P11` range, so no example
is mistaken for a real box; the refs are real:

```markdown
## P98 — Example phase (fictional ids)

- [x] **P98.1** [RUST] Add the example engine's argument builder · §3.5 · G29
  needs: P97.9
- [ ] **P98.2** [RUST] Wire the example conversion through the isolation boundary · §2.12 §1.7 · G29 G31
  needs: P99.2
  - [x] **P98.2.1** [RUST] Worker decode/encode command · §3.5.5 · G31
  - [ ] **P98.2.2** [TEST] Per-pair integration: output validity of one pair · §6.4.3 §6.5 · G32
- [ ] **P98.3** [BUILD] Stage the example engine · §3.5.5 · G37
  needs: P98.2
  l-neg1: same-push
- [!] **P98.4** [TEST] Cross-decoder re-validate · §6.4.5 · G32
  unlocked-by: P99.1
  > blocked: needs the P99.1 sidecar staged first.
- [ ] **P98.5** [DOC] Refresh the contributor setup steps · tooling-only
```

Reading it the way the loop does: `P98.1` is done, and its `needs: P97.9` is the phase
chain's edge to the `P97` sweep box (check 31, §5a). The next open box is `P98.2`, which
`needs: P99.2`, a later-phase box — if `P99.2` is not `[x]`, the loop builds it first
(DECISION C), then returns to `P98.2` and works its sub-boxes `P98.2.1` → `P98.2.2`
before checking `P98.2` off. `P98.3` follows; its `l-neg1: same-push` line sends its
caged part to the Co-Pilot (§5.3). `P98.4` is `[!]`-blocked with a note and auto-flips
to `[ ]` when `P99.1` is `[x]`. `P98.5` is a pure-tooling doc box with **neither** a
spec `§` **nor** a gate id, so it carries the explicit `· tooling-only` token (§3.1) to
declare that absence — a box with a real `§` would *not* carry `tooling-only`, since
the two are mutually exclusive. Numbering is gap-free (`.1`–`.5`; sub-boxes `.1`–`.2`);
every `§` and `Gnn` resolves; every tag is in the taxonomy; the one `tooling-only`
box carries no ref. In a plan where `P97.9`, `P99.1` and `P99.2` exist, the §7 checks
pass it.

---

## 10. References

- The loop that reads this format (selection, DoD, hard-stops, the reviewer rubric):
  [`build-loop.md`](../process/build-loop.md) — §3 step 1 (selection), step 2
  (DECISION C / sub-boxes), §0 (P1..P11 range / DECISION B).
- Who follows a `needs:` vs who escalates a block:
  [`roles-and-escalation.md`](../process/roles-and-escalation.md) §4.
- The gate that enforces this format (`plan-lint`, G7/G20) + the doc-wide
  consistency checks: [`build-gates.md`](../security/build-gates.md) §6.
- The selection tool: `scripts/plan-lint --next`, `--show <id>`, `--report owner-acts
  --phase <n>` (build-gates G7).
- Project rules (the working model, the anti-patterns): [`CLAUDE.md`](../../CLAUDE.md)
  §2, §5.
- The plan index (one row per phase file): [`README.md`](README.md).
- The P0 box areas that author this file (P0.1) + the gate framework (P0.2):
  [`P0-build-and-security.md`](P0-build-and-security.md).
- SSOT (what & why): [`docs/SINGLE-SOURCE-OF-TRUTH.md`](../SINGLE-SOURCE-OF-TRUTH.md).
