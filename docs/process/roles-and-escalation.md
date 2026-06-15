# ConvertIA — Roles & Escalation

> **Who decides what, and who escalates to whom.** The two-session working model,
> the decide-it-yourself default, the small set of triggers that send a decision
> *up*, and the hard-stops. This is the **operational companion** to
> [build-loop.md](build-loop.md): build-loop.md owns the *mechanics* (the loop, the
> 8-point DoD, the hard-stop / token-Notbremse **numbers**, the reviewer rubric,
> crash-recovery); this file owns the *org chart* — the boundary between "decide it
> yourself, tagged" and "stop and escalate", and the path an escalation travels.
> Where a number or a procedure lives in build-loop.md, it is **referenced**, never
> re-stated — `plan-lint` checks 14/15 keep those numbers single-homed there.
>
> **Status: living.** Refined *during* implementation; a change to the role
> boundary is recorded here first, in the same commit as the change.
> **Conflict order (unchanged, every layer):**
> **SSOT > spec > security/process docs > plan > code > conversation.**
> When two layers disagree, the higher one wins — **never silently reconcile,
> always escalate**.

---

## 1. The three roles

| Role | What it is | What it decides | What it never does |
|---|---|---|---|
| **Build-Loop session** | The autonomous builder. Reads [build-loop.md](build-loop.md) top to bottom, then works the plan box by box (**P1 onward**), writes tests, runs every gate + the [dual review](build-loop.md#step-5--pre-commit-opus--sonnet-dual-review-g1) (G1), and commits **directly to `main`** + pushes. The gates are the protection — no second branch, no merge step. | Routine implementation, pattern, naming, path, and default-value choices — **itself**, after a codebase/process-doc pattern lookup (§3), each tagged `[Build-Session-Entscheidung: <box-id>]` at the code site. Phase-cut / "which phase owns this" questions answered from the plan. | Merge, rewrite history, force-push, `--no-verify`, `core.hooksPath` redirection, pick a side in a spec contradiction, or build **P0** (DECISION B, §5). It escalates *to* Co-Pilot — it never resolves a genuine fork on its own. |
| **Co-Pilot session** | The owner's partner. The Build-Loop's **escalation & clarification target**; the home of strategic / cross-phase / architecture decisions and high-level review. Drives the **manual P0 bootstrap** with the owner (§5). | Cross-phase architecture with no spec/SSOT source; how to *resolve* a spec/SSOT contradiction once the owner has ruled on the fork; whether a misfiring gate is scoped or quarantined ([build-loop.md §6](build-loop.md#6-hard-stops-token-notbremse-and-the-gate-quarantine-escape)); P0 content alongside the owner. | Override the conflict order, or change scope / SSOT intent. A genuine fork (§4) goes to the **owner**; Co-Pilot frames it, the owner calls it. |
| **Owner** | Final authority. | The genuine forks: scope, legal/license posture, the [reviewer-family flip](../security/security-concept.md#2-working-model--two-sessions-one-branch), the [security-critical-file L(-1) ack policy](../security/security-concept.md#2-working-model--two-sessions-one-branch), the v1 cut, and any decision a doc records as "owner decision / owner call". Drives the start/stop vocabulary ([build-loop.md §7](build-loop.md#7-escalation-conversation-and-the-startstop-vocabulary)). | — |

**Escalation path:** **Build-Loop → Co-Pilot → owner.** A finding never skips a
rung: the Build-Loop does not address the owner directly through the loop's output;
it raises a Co-Pilot item, and Co-Pilot brings a genuine fork to the owner. The
[dual review (G1)](build-loop.md#step-5--pre-commit-opus--sonnet-dual-review-g1) is a
**quality amplifier, not a security control** — the only security controls are the
deterministic gates (every `Gnn` except G1), so an escalation is a quality / blocker
signal, not the thing that keeps insecure code out (the gates do that on a clean
checkout regardless of who is watching).

**Context-routing — who holds what (lean loop / full Co-Pilot).** The two sessions
are deliberately given **different amounts of the security/gate corpus**:

- **The Build-Loop runs LEAN.** Its prompt
  ([build-loop.md §2](build-loop.md#2-read-these-at-session-start-mandatory))
  **references** [build-gates.md](../security/build-gates.md) +
  [security-concept.md](../security/security-concept.md) for a **per-box / red-CI
  gate lookup** — it reads only the `Gnn` rows a box cites, on demand — and does
  **NOT inline** the whole gate/security corpus into the session (no context
  ballast). On a red CI it looks the gate up; it does not carry the catalogue.
- **The Co-Pilot holds the FULL picture.** The complete gate catalogue (`G1..Gnn`),
  the threat model (security-concept §0.11/§5), and the cross-phase security view are
  the Co-Pilot's to carry — it is the home of the strategic / high-level security
  review, so it reasons over the corpus the loop only samples.

This split is a defense-in-depth property, not a convenience (a bloated loop prompt
dilutes the per-box focus the gates rely on) and is **asserted by the P0.1.7
documentation-wiring & context-routing audit** + stated as a living-doc rule in
[security-concept.md §6](../security/security-concept.md#6-living-doc-rules). A
fill-pass must never paste the corpus into the loop prompt.

---

## 2. The default: decide it yourself

**Most choices are the Build-Loop's to make.** The autonomous model only works if
the loop does **not** escalate routine work. The following are **decided by the
loop, never escalated**:

- **Implementation pattern** — how a thing is structured internally, given the spec
  `§` fixes the contract (the types, the error kinds, the IPC schema).
- **Naming** — identifiers, module names, test names, fixture names (English; CLAUDE.md §8).
- **Paths** — file/module layout within the established monorepo structure.
- **Default values** — where the spec leaves a default open and the SSOT inclusion
  test / everyday-person audience (CLAUDE.md §1) makes one sensible.
- **Phase-cut questions** — "which phase owns this box" is answered from the plan
  ([README.md](../plan/README.md) phase boundaries), not escalated.

These are **derived assumptions**, not forks. The discipline that keeps them honest
is the **inline tag**, not an escalation (§3).

---

## 3. Decide-it-yourself, in practice — the inline tags

Before deciding a non-spec choice, **grep the codebase + the process docs for an
established pattern first** — a routine choice that already has a precedent is never
a fresh decision (and never an escalation). Then decide at the owner's core-rule
anchor when two genuinely professional options exist: **the cleanest /
most-complete / most-professional solution wins over token-cost, speed, and
"pragmatism"** ([CLAUDE.md §6](../../CLAUDE.md)) — *not* reflexively the cheaper one.
Then **tag the choice at the code site** so it is auditable without reading the
commit body:

- **`[Build-Session-Entscheidung: <box-id>]`** — at every non-spec
  pattern / naming / path / default choice the loop made itself. It lives **directly
  at the code site**, not only in the commit body (DoD item (f),
  [build-loop.md §5](build-loop.md#5-definition-of-done-the-canonical-8-point-list--this-file-is-canonical)),
  and it **also suppresses G8** at a documented choice site (a bare `[!extern]` does
  **not** suppress G8 in production code).
- **`[Derived-Assumption: <box-id> — <what was assumed, from where>]`** — at a place
  where the loop **filled a gap the spec left open** by deriving from a higher layer
  (SSOT intent, an adjacent spec `§`, an established sibling pattern) rather than
  picking arbitrarily. It records *what* was assumed and *why that source*. This is
  the honesty marker for "the spec did not say, so I derived X from Y" — distinct
  from a free design choice (`[Build-Session-Entscheidung]`) because it is anchored
  to a named source. If the assumption cannot be anchored to a higher layer at all,
  it is not a derived assumption — it is one of the escalation triggers in §4.
  **G8 status:** because this form carries a `<box-id>`, it is already a documented
  choice site for **G8** — suppressed via the **existing box-id path**
  ([build-gates.md G8](../security/build-gates.md), which suppresses on a `box-id`
  **OR** a `[Build-Session-Entscheidung]` within ±6 lines), exactly like a tagged
  `[Build-Session-Entscheidung]`. So a `[Derived-Assumption]` note may sit beside its
  derivation prose — even prose that uses G8 deferral vocabulary while explaining
  "the spec did not say, so I derived X from Y" — **without tripping G8**.

> **Where this tag is emitted from the loop's runbook.** The canonical inline-tag
> emission home is [build-loop.md step 4](build-loop.md#step-4--build-per-spec--write-tests-at-the-highest-sensible-level)
> / [DoD item (f)](build-loop.md#5-definition-of-done-the-canonical-8-point-list--this-file-is-canonical),
> which today name only `[Build-Session-Entscheidung: <box-id>]`. A Build-Loop
> session discovers `[Derived-Assumption]` because [build-loop.md §2](build-loop.md#2-read-these-at-session-start-mandatory)
> mandates reading **this file** at session start; if build-loop.md step 4 ever gains
> a sibling mention of `[Derived-Assumption]`, that is the place for it. (Co-Pilot
> note flagged at the bottom of §7.)

A tagged choice is the loop **owning** a decision in the open. An escalation is the
loop **declining** to own one because it genuinely cannot. §4 is the exhaustive line
between them.

---

## 4. When to escalate to Co-Pilot (the exhaustive trigger set)

Escalate **only** when one of these is genuinely true. Everything else: **decide and
proceed, tagged** (§3).

- **(a) Spec / SSOT internal contradiction** — two spec `§§` disagree, or the spec
  disagrees with the SSOT. This is an **unconditional hard-stop + escalate
  regardless of severity** — the loop is **downstream** of the spec and cannot pick
  a side. **Never a working-tree fix.** Either reviewer flagging a
  `SPEC-CONTRADICTION` (the finding class **above P0** in the
  [reviewer rubric](build-loop.md#step-5--pre-commit-opus--sonnet-dual-review-g1))
  triggers this same path.
- **(b) A cross-phase architecture decision with no spec/SSOT source** — a design
  choice that **binds later phases** and is **not derivable** from the spec or the
  SSOT (so it is not a `[Derived-Assumption]` either). Picking it inside one box
  would silently constrain phases that have not been planned yet.
- **(c) A scope / legal / license conflict** — the box implies work outside the SSOT
  *Explicitly Out of Scope* line (store/marketing/distribution logistics, legal
  advice, binary code-signing/notarization), **or** a GPL/AGPL/LGPL-into-MIT-core
  copyleft conflict (CLAUDE.md §3), **or** any decision a doc reserves as an "owner
  decision". Co-Pilot frames it; a genuine fork goes to the owner.
- **(d) A `needs:` dependency that genuinely cannot be followed/built** — DECISION C
  ([build-loop.md §3 step 2](build-loop.md#step-2--unpack-the-box-anatomy)) says a
  `needs: P<x>.<y>` on a *buildable* box is **built in place, then returned to** —
  that is **not** an escalation. Escalate only when the prerequisite is something the
  loop **cannot** build: it requires an owner action or external input (an
  `[!extern]` prerequisite of a non-extern box), or the plan is an **all-blocked
  deadlock** with nothing open.

Two further blockers route the same way (their mechanics live in build-loop.md, the
*who* is here):

- **(e) A provably-misfiring required gate** — a required gate failing **closed on a
  false positive**. The only sanctioned unblock is the committed, dual-reviewed,
  narrowly-scoped **gate-quarantine** procedure
  ([build-loop.md §6](build-loop.md#6-hard-stops-token-notbremse-and-the-gate-quarantine-escape));
  a misfire that cannot be scoped that way is a hard-stop + escalate. **Never**
  `--no-verify`.
- **(f) Reviewer unavailability** — two **live** reviews cannot be obtained after the
  bounded retry. **NEVER** auto-emit a `GO` trailer with fewer than two live reviews;
  hard-stop + escalate
  ([build-loop.md §3 step 5](build-loop.md#step-5--pre-commit-opus--sonnet-dual-review-g1)).
- **(g) A needed L(-1) security-critical-file edit** — the loop **NEVER** edits a
  security-critical file (the gates' own cage) autonomously; a box that requires editing
  one is a **hard-stop + escalate** so the **owner** makes/approves it and adds the
  `L-neg1-ack: owner` trailer ([security-concept §2](../security/security-concept.md#2-working-model--two-sessions-one-branch),
  gate **G71**). The explicit, load-bearing case of (c)'s "any decision a doc reserves as
  an owner decision".

### NOT escalation (decide yourself, tagged)

To make the line unambiguous — these are explicitly **not** triggers, even when they
feel like a fork: an **implementation-pattern** choice, **naming**, **paths**,
**default values**, and **phase-cut** ("which phase owns this") questions. A
`needs:` on a *buildable* box is a dependency to **follow**, not to escalate. A spec
gap the loop can **anchor to a higher layer** is a `[Derived-Assumption]`, not an
escalation. When in doubt between "derive and tag" and "escalate", the test is
trigger (a)/(b)/(c): is there a *contradiction*, an *unbound cross-phase commitment*,
or a *scope/legal* line? If none, derive and tag.

---

## 5. DECISION B — P0 is bootstrapped manually

**P0 is built by the Co-Pilot session + the owner, manually — not by the
Build-Loop.** P0 (`docs/plan/P0-build-and-security.md`) is the **bootstrap** phase:
it *creates* the loop, the gate system, the dual review, and the test methodology
that **every later phase runs under**. The loop cannot build the thing it runs
inside. Concretely:

- **The Build-Loop's range is `P1`..`P11`** (the plan is exactly `P0`..`P11`;
  the concrete upper bound is single-homed in the
  [build-loop.md §0 bootstrap note](build-loop.md#0-who-runs-this-and-what-it-is-not)).
  If the loop finds itself reaching into a `P0.x` box, that is **out of range** —
  **stop**, do not build (the session-start sanity in
  [build-loop.md §3 step 0](build-loop.md#step-0--start-sanity-once-per-session-the-very-first-action)
  is the mechanical guard).
- **The dual review (G1) still applies to P0** — it is simply **driven manually** by
  the Co-Pilot session during the bootstrap, not auto-run by the loop.
- **The hand-off is the seven P0 docs.** By the time the loop starts at P1, the seven
  P0 process/security artifacts are live and the two enforcement planes are green:
  [security-concept.md](../security/security-concept.md),
  [build-gates.md](../security/build-gates.md), [build-loop.md](build-loop.md),
  [test-strategy.md](test-strategy.md), **this file**,
  [`docs/plan/_format.md`](../plan/_format.md), and
  [vuln-response.md](vuln-response.md) *(authored in P0; absent until P0 completes)*.
  Until they exist, the canonical process rules
  + the DoD live in **P0.6 of**
  [`docs/plan/P0-build-and-security.md`](../plan/P0-build-and-security.md) (CLAUDE.md §2/§4).
- **The P0 cadence is a manual one.** P0's cluster-boundary soft-stop is a
  **manual-bootstrap** number; the autonomous loop never reaches a `P0.x` boundary.
  Both numbers are single-homed in
  [build-loop.md §6](build-loop.md#6-hard-stops-token-notbremse-and-the-gate-quarantine-escape).

---

## 5a. Incoming pull requests & Dependabot bumps — owned by Co-Pilot, never the loop

**The autonomous Build-Loop never reviews or merges an incoming PR.** The loop
commits **directly to `main`** and has **no merge step** ([build-loop.md §0](build-loop.md#0-who-runs-this-and-what-it-is-not)) —
it builds plan boxes, it does not process the inbound-PR queue. Yet this is a
*public* OSS repo with two real incoming-PR sources that need an explicit owner so
the queue cannot grow silently while the loop builds forever:

- **Dependabot dependency-bump PRs** — the `dependabot.yml` stood up in P0.2.6 covers
  **github-actions + cargo + npm + pip**, so green bumps arrive as PRs against `main`.
- **External fork pull-requests** — the only surviving "PR" concept in the single-branch
  model ([build-loop.md §0](build-loop.md#0-who-runs-this-and-what-it-is-not)); their
  review→merge is otherwise unspecified.

**Ownership (DECIDED):** **incoming-PR triage / review / merge is the Co-Pilot
(owner) session's job, not the autonomous loop's.** The loop has no authority to
merge, rewrite history, or force-push (§1), so it neither opens, reviews, nor merges
these PRs; it may *surface* a security-relevant bump as a Co-Pilot item but never
acts on it. A green Dependabot bump reaches `main` via a **manual Co-Pilot merge**,
and **any bump that touches `engines.lock` additionally runs the §6.5 engine-bump
re-validation** (the CVE→user path in
[vuln-response.md](vuln-response.md) routes a security bump through "bump the
`engines.lock` pin → re-run the §6.5 reliability gate → new release"; P0.6.9). This
sits at the **maintenance-process layer**, outside the v1 build-box plan — recorded
here as an explicit decision so its absence from the plan boxes is deliberate, not a
silent gap.

---

## 6. Hard-stops

The loop **stops and escalates** (it does not push past these). The
**numbers** for the count-based stops are canonical in
[build-loop.md §6](build-loop.md#6-hard-stops-token-notbremse-and-the-gate-quarantine-escape)
(`plan-lint` check 15) and are **not** duplicated here — this list is the *who-acts*
view:

- The owner writes a **stop word** (`stop` / `halt` / `pause`) → save state, wait.
- A **spec-internal contradiction** (trigger (a)) → unconditional, regardless of
  severity, never silently reconciled.
- A **dual-review P0 that is genuinely not fixable** — but **only after** the
  pattern lookup (§3): a P0 with an established pattern is **not** a hard-stop, apply
  the pattern.
- **Reviewer unavailability** (trigger (f)) — two live reviews unobtainable after
  bounded retry.
- **Consecutive gate-red pushes** beyond the build-loop.md threshold, despite fix
  attempts.
- An **anomalous CI cancel** (a `cancelled` run with no successor —
  [build-loop.md §3 step 6](build-loop.md#step-6--commit--push-gates-run-never-bypass)).
- A needed **L(-1) security-critical-file edit** (trigger (g)) — the loop never edits the
  gates' own cage; the **owner** makes/acks it (`L-neg1-ack: owner`, G71).
- A **soft-stop / hard-stop / phase-change** cadence boundary
  ([build-loop.md §6](build-loop.md#6-hard-stops-token-notbremse-and-the-gate-quarantine-escape)) —
  pause and summarize for the owner.

On every hard-stop the loop reports the blocker as a **single Co-Pilot line** right
after the status line (never inlined into a box summary), naming the **count**, the
**severity**, and the source **`§`/file**
([build-loop.md §8](build-loop.md#8-output-discipline)) — so the owner has a
scannable hand-off and Co-Pilot has an actionable item.

---

## 7. References

- The loop these roles drive (mechanics, DoD, hard-stop numbers, reviewer rubric,
  crash-recovery): [build-loop.md](build-loop.md) — §0 (roles), §3 step 0
  (session-start sanity / out-of-range guard), §4 (decide-vs-escalate), §6
  (hard-stops / gate-quarantine), §7 (start/stop vocabulary), §8 (output discipline).
- Project rules / DoD summary / anti-patterns / owner's core rule:
  [CLAUDE.md](../../CLAUDE.md) §2 (working model), §5 (anti-patterns), §6 (core rule).
- Box format + the `needs:` / `unlocked-by:` dependency vocabulary (DECISION C):
  [`docs/plan/_format.md`](../plan/_format.md).
- Working-model + dual-review-as-quality-amplifier + the recorded reviewer-family
  decision: [security-concept.md](../security/security-concept.md) §2.
- The CVE → user escalation runbook (engine vuln → Build-Loop escalates → Co-Pilot →
  release): [vuln-response.md](vuln-response.md) *(a P0 deliverable; does not exist
  until P0 completes)*.
- Plan home (P0 bootstrap clusters; P1..P11 range — the plan is exactly P0..P11):
  [`docs/plan/README.md`](../plan/README.md) ·
  [`docs/plan/P0-build-and-security.md`](../plan/P0-build-and-security.md) §P0.6.
- SSOT (what & why; scope line): [SINGLE-SOURCE-OF-TRUTH.md](../SINGLE-SOURCE-OF-TRUTH.md).

> **Co-Pilot note (non-blocking, doc-consistency).** The `[Derived-Assumption: <box-id>
> — …]` tag is currently introduced **only here** (§3); build-loop.md step 4 / DoD
> item (f) name only `[Build-Session-Entscheidung]`. The tag is discoverable because
> build-loop.md §2 mandates reading this file at session start, but build-loop.md step
> 4 gaining a sibling mention of `[Derived-Assumption]` would make the inline-tag
> emission home self-contained. Tracked for a future build-loop.md edit; not a blocker.
