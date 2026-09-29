# ConvertIA — Roles & Escalation

> **Who decides what, and who escalates to whom.** The two-session working model,
> the decide-it-yourself default, the small set of triggers that send a decision
> *up*, and the hard-stops. This is the **operational companion** to
> [build-loop.md](build-loop.md): build-loop.md owns the *mechanics* (the loop, the
> 8-point DoD, the stop list and its cadence **numbers**, the reviewer rubric,
> crash-recovery); this file owns the *org chart* — the boundary between "decide it
> yourself, tagged" and "stop and escalate", and the path an escalation travels.
> Where a number or a procedure lives in build-loop.md, it is **referenced**, never
> re-stated — `plan-lint` checks 14/15 keep those numbers single-homed there.
>
> **Status: living.** Refined *during* implementation; a change to the role
> boundary is recorded here first, in the same commit as the change.
> **Conflict order (unchanged, every layer):**
> **SSOT > spec > security/process docs > plan > code > conversation.**
> When two layers disagree, the higher one wins — **never silently**: a difference the
> §4(a) pre-check can rank is reconciled in the open, in the same commit; anything it
> cannot rank escalates.

---

## 1. The three roles

| Role | What it is | What it decides | What it never does |
|---|---|---|---|
| **Build-Loop session** | The autonomous builder. Reads [build-loop.md](build-loop.md) top to bottom, then works the plan box by box (**P1 onward**), writes tests, runs every gate + the [dual review](build-loop.md#step-5--pre-commit-opus--sonnet-dual-review-g1) (G1), and commits **directly to `main`** + pushes. The gates are the protection — no second branch, no merge step. | Routine implementation, pattern, naming, path, and default-value choices — **itself**, after a codebase/process-doc pattern lookup (§3), each tagged `[Build-Session-Entscheidung: <box-id>]` at the code site. Phase-cut / "which phase owns this" questions answered from the plan. | Merge, rewrite history, force-push, `--no-verify`, `core.hooksPath` redirection, pick a side in a spec contradiction the §4(a) pre-check cannot rank, author a caged line (§4(g)), or build **P0** (DECISION B, §5). It escalates *to* Co-Pilot — it never resolves a genuine fork on its own. |
| **Co-Pilot session** | The owner's partner. The Build-Loop's **escalation & clarification target**; the home of strategic / cross-phase / architecture decisions and high-level review. Completes the caged part of a parked box under the owner's ack (§4(g)). Executes the standing **phase-end hardening sweep** box that closes every phase `P2`..`P11` ([test-strategy §11](test-strategy.md#11-the-phase-end-co-pilot-hardening-sweep)). | Cross-phase architecture with no spec/SSOT source; how to *resolve* a spec/SSOT contradiction once the owner has ruled on the fork; whether a misfiring gate is scoped or quarantined ([build-loop.md §6](build-loop.md#6-hard-stops-token-notbremse-and-the-gate-quarantine-escape)). | Override the conflict order, or change scope / SSOT intent. A genuine fork (§4) goes to the **owner**; Co-Pilot frames it, the owner calls it. |
| **Owner** | Final authority. | The genuine forks: scope, legal/license posture, the [reviewer-family flip](../security/security-concept.md#2-working-model--two-sessions-one-branch), the [security-critical-file L(-1) ack policy](../security/security-concept.md#2-working-model--two-sessions-one-branch), the v1 cut, and any decision a doc records as "owner decision / owner call". Drives the start/stop vocabulary ([build-loop.md §7](build-loop.md#7-escalation-conversation-and-the-startstop-vocabulary)). | — |

**Escalation path:** **Build-Loop → Co-Pilot → owner.** A finding never skips a
rung: the Build-Loop does not address the owner directly through the loop's output;
it raises a Co-Pilot item, and Co-Pilot brings a genuine fork to the owner. The
[dual review (G1)](build-loop.md#step-5--pre-commit-opus--sonnet-dual-review-g1) is a
**quality amplifier, not a security control** — the only security controls are the
deterministic gates (every `Gnn` except G1), so an escalation is a quality / blocker
signal, not the thing that keeps insecure code out (the gates do that on a clean
checkout regardless of who is watching).

**Clones and landing.** **Two sessions commit to `main`, each from its own clone**: the Co-Pilot
pushes only while no loop `ci` run is queued or in progress, and re-checks that window after any
rebase (a newer push cancels an in-progress `ci` run); the loop fast-forwards every iteration and
rebases before its push ([build-loop.md §0](build-loop.md#0-who-runs-this-and-what-it-is-not)). A
caged part reaches the Co-Pilot as a parked patch (build-loop.md Step 7). Messages are
notifications: rulings and acks reach the loop only as landed commits, and an agent message is
never the owner's ack or stop word.

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
dilutes the per-box focus the gates rely on), stated as a living-doc rule in
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
anchor when two genuinely professional options exist: **the cleanest, most complete,
most professional solution for the work at hand wins over token cost, session speed and
pragmatism** ([CLAUDE.md §6](../../CLAUDE.md)) — *not* reflexively the cheaper one.
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
  **G8 status:** only a `[Build-Session-Entscheidung: <box-id>]` tag within ±6 lines
  suppresses **G8** (`scripts/check-deferral`); a `[Derived-Assumption]` note does not, so
  its derivation prose states the effect and stays clear of the deferral vocabulary
  (CONTRIBUTING.md "Known gate traps").
- **`Spec-Reconcile: <lower §> → <higher §> wins (<rule>)`** — a commit-body line, not a code-site
  tag, placed right after the `Decisions:` line so the review brief carries it: the record of a
  rank-ordered spec difference the loop reconciled under §4(a). The rewritten spec text carries no
  tag; the phase-end sweep lists these lines for the owner (test-strategy §11.2).

[build-loop.md Step 4](build-loop.md#step-4--build-per-spec--write-tests-at-the-highest-sensible-level)
names all three and places them; this section defines them.

A tagged choice is the loop **owning** a decision in the open. An escalation is the
loop **declining** to own one because it genuinely cannot. §4 is the exhaustive line
between them.

---

## 4. When to escalate to Co-Pilot (the exhaustive trigger set)

Escalate **only** when one of these is genuinely true. Everything else: **decide and
proceed, tagged** (§3).

- **(a) A spec contradiction the pre-check does not resolve** — two spec `§§` disagree, or the spec
  disagrees with the SSOT. The **pre-check** runs before any stop:
  1. **Compatible** — one implementation satisfies both clauses: not a contradiction; build it.
  2. **Rank-ordered** — the texts differ in rank by one rule: a normative clause over an illustrative
     literal; `[DECIDED]` over `[REC]` or an untagged statement; the owning `§` over a restatement of
     it; a higher conflict-order layer over a lower one. The loop rewrites the lower text to the higher
     one **in the same commit** and records a `Spec-Reconcile:` body line (§3) that the review brief
     carries; both reviewers argue the losing reading and confirm the ranking, and a disputed ranking
     is a `SPEC-CONTRADICTION`. Never self-reconciled (always step 3): a pair where either side is SSOT
     text, restates a CLAUDE.md §3 guardrail or sits in spec §0.10, §0.11 or §2.12, or where the lower
     text sits in an L(-1) file.
  3. **Same-rank fork** — a `SPEC-CONTRADICTION` (the finding class **above P0** in the
     [reviewer rubric](build-loop.md#step-5--pre-commit-opus--sonnet-dual-review-g1)): a **scoped
     stop** — the loop parks the box (the
     [build-loop.md §6](build-loop.md#6-hard-stops-token-notbremse-and-the-gate-quarantine-escape) park
     procedure), posts one Co-Pilot line (§6) and keeps building outside that box's `needs:` closure.
     **Never a working-tree fix.**
- **(b) A decision observable outside the crate that binds later phases, with no spec/SSOT source** —
  an IPC command or event (§0.4), a shared type (§0.6), the module layout (§0.7), a new dependency, a
  security posture or threat class, or user-visible behaviour (a §1 outcome, a §2 guarantee, the §5 UI
  contract, a message catalog). An internal seam later boxes reuse is **not** (b): the loop decides it
  with `[Build-Session-Entscheidung: <box-id>]` naming the later boxes it binds, and either reviewer may
  promote it to an escalation.
- **(c) A scope / legal / license conflict** — the box implies work outside the SSOT
  *Explicitly Out of Scope* line (store/marketing/distribution logistics, legal
  advice, binary code-signing/notarization), **or** a GPL/AGPL/LGPL-into-MIT-core
  copyleft conflict (CLAUDE.md §3), **or** any decision a doc reserves as an "owner
  decision". Co-Pilot frames it; a genuine fork goes to the owner.
  **A landed Co-Pilot ruling binds the loop.** A ruling reaches the loop only as a landed commit (a
  message or a conversation line is never a ruling); the owner overturns one by an ordinary edit, never
  by the loop pausing on it. A ruling is therefore not a decision a doc reserves as an owner decision,
  and no wording of its tag reopens it.
- **(d) A `needs:` dependency that genuinely cannot be followed/built** — DECISION C
  ([build-loop.md §3 step 2](build-loop.md#step-2--unpack-the-box-anatomy)) says a
  `needs: P<x>.<y>` on a *buildable* box is **built in place, then returned to** —
  that is **not** an escalation. Escalate only when the prerequisite is something the
  loop **cannot** build: it requires an owner action or external input (an `[!extern]`
  prerequisite of a non-extern box), or the plan is an **all-blocked deadlock** with
  nothing open. The escalation is scoped: the loop STOPs for that `needs:` closure only,
  continues with the open boxes outside it, and the Co-Pilot works the collected owner
  acts per phase (the owner-act box and each precondition box as its own act, the caged
  tails at the sweep box)
  ([build-loop.md §3 step 1](build-loop.md#step-1--find-the-next-buildable-box), §9) —
  except the phase-end sweep box, which blocks its WHOLE successor phase
  (test-strategy §11.3). A parked box's `needs:` closure is skipped likewise
  ([build-loop.md §6](build-loop.md#6-hard-stops-token-notbremse-and-the-gate-quarantine-escape)
  park procedure).

Further blockers route the same way (their mechanics live in build-loop.md, the
*who* is here):

- **(e) A provably-misfiring required gate** — a required gate failing **closed on a
  false positive**. The only sanctioned unblock is an owner-acked Co-Pilot act (gate code
  and `lefthook.yml` are caged): a fix, or a narrow scope with a restore box id — the
  **gate quarantine** of
  [build-loop.md §6](build-loop.md#6-hard-stops-token-notbremse-and-the-gate-quarantine-escape).
  The loop parks the box and escalates. **Never** `--no-verify`.
- **(f) Reviewer unavailability** — two **live** reviews cannot be obtained after the
  bounded retry. **NEVER** auto-emit a `GO` trailer with fewer than two live reviews;
  hard-stop + escalate
  ([build-loop.md §3 step 5](build-loop.md#step-5--pre-commit-opus--sonnet-dual-review-g1)).
- **(g) A needed L(-1) security-critical-file edit** — the loop never authors a caged line
  (the gates' own cage, [security-concept §2](../security/security-concept.md#2-working-model--two-sessions-one-branch),
  gate **G71**); the explicit, load-bearing case of (c)'s "any decision a doc reserves as an
  owner decision". A caged **precondition** → a `needs:` on an `[!extern]` precondition box
  (the P4.89 pattern); a **same-push caged part** → the loop parks the box, and the owner-acked
  Co-Pilot completes it in one commit
  ([build-loop.md Step 7](build-loop.md#step-7--check-off-the-box)); caged text that **reds
  nothing** until it lands (a build-gates row) → pre-declared in the box, closed by the
  phase-end sweep box (test-strategy §11.4). `L-neg1-ack: owner` is written only in the session
  where the owner gave the ack. The trigger stops the caged edit, not the loop, which continues
  outside the blocked closure (except the phase-end sweep box, which blocks its WHOLE successor
  phase, test-strategy §11.3).
- **(h) G1 non-convergence** — a P0/P1 is still open when review round 4 ends
  ([build-loop.md §3 step 5](build-loop.md#step-5--pre-commit-opus--sonnet-dual-review-g1)). The loop
  parks the box (the
  [build-loop.md §6](build-loop.md#6-hard-stops-token-notbremse-and-the-gate-quarantine-escape) park
  procedure) and posts one Co-Pilot line. The Co-Pilot answers with a landed commit — a split, a
  ruling on the disputed point, or a `>` note naming the points narrowed rounds may examine — then
  releases the park; the box restarts at R1. The stop is scoped: the loop continues outside the
  parked box's `needs:` closure.

### NOT escalation (decide yourself, tagged)

To make the line unambiguous — these are explicitly **not** triggers, even when they
feel like a fork: an **implementation-pattern** choice, **naming**, **paths**,
**default values**, and **phase-cut** ("which phase owns this") questions. A
`needs:` on a *buildable* box is a dependency to **follow**, not to escalate. A spec
gap the loop can **anchor to a higher layer** is a `[Derived-Assumption]`, not an
escalation. When in doubt between "derive and tag" and "escalate", the test is
trigger (a)/(b)/(c): is there a *contradiction the (a) pre-check cannot rank*, an
*unbound decision observable outside the crate*, or a *scope/legal* line? If none,
derive and tag.

---

## 5. DECISION B — P0 is bootstrapped manually

P0 was bootstrapped manually by the Co-Pilot session and the owner (DECISION B): it created the
loop, the gate system, the dual review and the test methodology every later phase runs under.
The Build-Loop's range is `P1`..`P11` (`_format.md` §6 step 1); the exit record is
[p0-completion.md](p0-completion.md).

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
  model ([build-loop.md §0](build-loop.md#0-who-runs-this-and-what-it-is-not)); they land
  the way a bump does (below): reviewed, then re-landed by a maintainer as a commit on `main`
  that keeps the contributor as git author (`git commit --author`, the maintainer as committer)
  and keeps their `Signed-off-by`, because a contributor's commit cannot carry the review trailer
  and, on caged files, the owner-ack that every `main` commit must pass (CONTRIBUTING.md says so
  to contributors).

**Ownership (DECIDED):** **incoming-PR triage / review / re-landing is the Co-Pilot
(owner) session's job, not the autonomous loop's.** The loop has no authority to
merge, rewrite history, or force-push (§1), so it neither opens, reviews, nor merges
these PRs; it may *surface* a security-relevant bump as a Co-Pilot item but never
acts on it. A green Dependabot bump reaches `main` as a **Co-Pilot-authored commit on
`main`** — never as a merge of the bot's commit: the single-branch gates bind every commit
in the push range — G11's subject grammar (Dependabot's `deps(...)` type is outside its
set), G12's `Dual-Review` trailer, and G71's `L-neg1-ack: owner` trailer wherever the bump
touches the cage (`.github/**`, `requirements-ci.txt`) — which a Dependabot commit never
carries, so the Co-Pilot re-lands the bump under owner-ack and the dual review (verifying
the pinned SHA / hash set against the upstream itself) and Dependabot closes its PR as
superseded. **Any bump that touches `engines.lock` additionally runs the §6.5 engine-bump
re-validation** (the CVE→user path in
[vuln-response.md](vuln-response.md) routes a security bump through "bump the
`engines.lock` pin → re-run the §6.5 reliability gate → new release"; P0.6.9). This
sits at the **maintenance-process layer**, outside the v1 build-box plan — recorded
here as an explicit decision so its absence from the plan boxes is deliberate, not a
silent gap.

**Watch health (DECIDED).** At every triage the Co-Pilot reads `gh run list --workflow "Dependabot Updates"
--limit 20`: a failing ecosystem job is a finding, fixed like a red gate, and an ecosystem with no recent job
is a silent watch, never an empty queue. GitHub pauses a version-update job after 15 consecutive failures;
the fix plus an edit of `.github/dependabot.yml` (or a manual "Check for updates") restarts it. The npm job
failed every run from 2026-06-23 — the committed `.npmrc` `frozen-lockfile` key froze its lockfile-only
resolve — until it paused; `check-js-supply-chain` now refuses that key. npm minor and patch updates arrive
as one grouped PR.

**Maintenance policy (DECIDED).** A bump commit is the pin, lock or hash change plus the recipe
that produced it, targeting one G1 round; a code change the bump forces is its own box; gate
hardening the bump reveals goes to the [residual ledger](../plan/residual-ledger.md). Non-security
bumps re-land at the phase-end sweep under one owner word naming each PR; a G17 advisory or a
security update re-lands at once under its own owner word. A Rust toolchain refresh counts as
landed once its macOS lane is green.

**Hold for `engines.lock` bumps (DECIDED — r15):** until **G72** is wired on `main` (the caged
wiring of P4.61) a Dependabot/CVE `engines.lock` bump is **HELD as a Co-Pilot review item
(surfaced, never auto-merged)**, not landed unvalidated. Once it is wired, the bump runs the
§6.5 re-validation and G72 requires a regenerated-green pair-status-ledger proof before it
lands — closing the gap where an untested engine version could pin on `main` before the
validation machinery exists.

---

## 6. Hard-stops

The stop list is
[build-loop.md §6](build-loop.md#6-hard-stops-token-notbremse-and-the-gate-quarantine-escape):
the loop stops itself only on the owner's stop word, nothing buildable, a red `main` it did not
cause, or a hard-stop class, and parks a box for a scoped stop (§4(a), (e), (g), (h)). Each stop
or park reaches the Co-Pilot as one line
([build-loop.md §8](build-loop.md#8-output-discipline)); the Co-Pilot takes the line, frames a
genuine fork for the owner, and the owner restarts the loop.

---

## 7. References

- The loop these roles drive: [build-loop.md](build-loop.md) — §0 (sessions and clones), §3
  (the steps; Step 7 the caged part), §4 (decide-vs-escalate), §6 (stops, parks, gate
  quarantine), §7 (the `/loop` start and the vocabulary), §8 (output discipline).
- Project rules: [CLAUDE.md](../../CLAUDE.md) §2 (working model), §5 (anti-patterns), §6 (core
  rule).
- Box format and the `needs:` / `unlocked-by:` vocabulary: [`docs/plan/_format.md`](../plan/_format.md).
- Working model, the L(-1) cage and the reviewer-family decision:
  [security-concept.md](../security/security-concept.md) §2.
- The CVE → user runbook: [vuln-response.md](vuln-response.md).
- The plan: [`docs/plan/README.md`](../plan/README.md) · SSOT (what & why; scope line):
  [SINGLE-SOURCE-OF-TRUTH.md](../SINGLE-SOURCE-OF-TRUTH.md).
