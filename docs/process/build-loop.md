# ConvertIA — Build-Loop (the master prompt / runbook)

> **The runbook for the autonomous Build-Loop session**, written to be read *as a prompt*: at
> session start and after every compaction (§2), then the loop runs (§7). It is the single
> canonical home of the **8-point Definition-of-Done** (§5), the **stop list and its cadence
> numbers** (§6, §7), the **reviewer rubric**, the **reviewer-family decision** and the
> **dual-review divergence rule** (§3 Step 5), and the **crash recovery** (§9) — `plan-lint`
> checks 14, 15, 18, 19 and 20 assert them here. If a copy of any of them drifts elsewhere,
> **this file wins**.
>
> **Conflict order (unchanged, every layer):**
> **SSOT > spec > security/process docs > plan > code > conversation.**
> When two layers disagree, the higher one wins — **never silently**: a spec
> contradiction first runs the pre-check ([roles-and-escalation.md](roles-and-escalation.md)
> §4(a)); a rank-ordered pair is reconciled in the open in the same commit, a
> same-rank fork is a scoped stop + escalate — the Build-Loop is downstream of the
> spec and never picks a side it cannot rank.

---

## 0. Who runs this, and what it is not

| Session | Role |
|---|---|
| **Build-Loop** (this file) | Autonomous. Builds `P1`..`P11`, one box per commit — code, tests, every gate, the dual review — and pushes **directly to `main`**. It never authors a caged line (§3 Step 7). |
| **Co-Pilot** | Works with the owner: escalations and rulings, the caged part of a parked box, owner acts, the phase-end sweep boxes ([test-strategy §11](test-strategy.md#11-the-phase-end-co-pilot-hardening-sweep)), and incoming PRs and Dependabot bumps ([roles-and-escalation.md §5a](roles-and-escalation.md#5a-incoming-pull-requests--dependabot-bumps--owned-by-co-pilot-never-the-loop)). |

Single branch `main`, no merge step, no worktrees. **Two sessions commit to `main`, each from its
own clone**: the Co-Pilot pushes only while no loop `ci` run is queued or in progress; the loop
fast-forwards every iteration (Step 0) and rebases before its push (Step 6). P0 was bootstrapped
manually (roles-and-escalation §5). The one surviving PR concept is the external fork pull
request; "per-PR" elsewhere means "per-push".

**The dual review is a quality amplifier, NOT a security control.** G1 is
self-attested through an unverifiable commit trailer; a gamed `GO/GO` cannot, by
itself, ship insecure code, because the **only security controls are the
deterministic gates — every `Gnn` except G1**. A gate either passes on a clean
checkout or it does not. G1 raises quality and catches design defects the gates
cannot encode.

---

## 1. Mission

Work `docs/plan/` **strictly**: **one box per iteration** (§7), lowest phase first, top to bottom
(`_format.md` §6). A phase is done when every box under it is `[x]`.

**Never build on a hunch.** The acceptance criteria for a box do not live in the
box — they live in the **spec `§§`** and the **gate IDs** the box references, and
in the [SSOT](../SINGLE-SOURCE-OF-TRUTH.md) above them. **If you have to guess, you
missed something** — re-read the referenced spec section in full, or escalate. The
box is a pointer; the spec is the contract.

**Build it fully — no stub as a default** (SSOT Principle 1: completeness within
scope; CLAUDE.md §6: the cleanest, most complete, most professional solution for
the work at hand always wins over token cost, session speed and pragmatism — scope,
not quality, bounds it). A stub is only ever
a **named, compile-time interface shell** that a **named, scheduled** box fills
(the P3 `crate::isolation` interface shells P4 expands are the sanctioned example,
plan/README.md P3) — never a quiet placeholder, never a "Phase 2 / for now / comes
in P\<n\>" deferral (those phrasings fail G8). The entire gate layer exists
*precisely* so this priority holds; ranking pragmatism above it undercuts the
whole protection layer.

---

## 2. Read these at session start (mandatory)

At session start and after every compaction — the summary is not a rule source — read:

1. [`CLAUDE.md`](../../CLAUDE.md) — the project rules (the harness loads it; re-read it here).
2. **This file**, in full.
3. [`_format.md`](../plan/_format.md) §2–§6 — markers, box anatomy, tags, `needs:` /
   `unlocked-by:`, the selection algorithm.
4. [`test-strategy.md`](test-strategy.md) §0, the §1 table, §4, §7, §8 and §10.
5. [`roles-and-escalation.md`](roles-and-escalation.md) §3–§4 — the tags and the escalation
   triggers.
6. [CONTRIBUTING.md](../../CONTRIBUTING.md) "Known gate traps" and
   [DEVELOPMENT.md](../../DEVELOPMENT.md) "Windows host notes".

Per box (Step 3): the cited spec §§ in full and the cited `Gnn` rows of
[`build-gates.md`](../security/build-gates.md), never the whole catalogue. Then print one line and
go on — at session start with Step 0, after a compaction with the step in progress (§7):

```
Bereit. Letzte abgehakte Box: <id>, naechste baubare Box: <id>.
```

---

## 3. The loop (step 0 → step 7), one box per iteration

### Step 0 — Start sanity (every iteration, the first action)

It runs when an iteration starts, never when a compaction resumes a step (§7). A mismatch in any
check is §6 stop (4):

- **Repo and branch:** `git rev-parse --show-toplevel` is this repo's clone,
  `git symbolic-ref --short HEAD` is `main`, `origin` is `github.com/Ne-IA/convertia`.
- **Hooks:** `git config --get core.hooksPath` is unset or the lefthook-managed path — a redirect
  silently disables L1–L3 without `--no-verify` (CLAUDE.md §5); G54 checks the effective hooks
  dir at pre-push.
- **No out-of-band gate edit:** `git diff --name-only HEAD -- scripts/ lefthook.yml .github/` lists
  no path but an uncaged one of the loop's own unfinished box (§9); a caged path is never the
  loop's (Step 7).
- **Clean tree:** `git status` shows nothing but the loop's own unfinished box (§9), its Step 1
  flips included (Step 7).
- **Sync:** `git fetch origin main`; on a clean tree `git merge --ff-only origin/main`; re-read
  any §2 file or selected box it changed. If it changed `lefthook.yml`, run
  `python3 -P scripts/setup-dev` before the next commit (DEVELOPMENT.md "Windows host notes").
- **CI health:** the last `gh run list --workflow ci --branch main --event push` run: success →
  go; in progress → Steps 1–3 only until it concludes (Step 6); red → attribute it (Step 6);
  unreachable → warn and go.
- **Parks:** list `$(git rev-parse --git-common-dir)/parked/` (§6 park procedure): a `.released`
  park is re-selected (Step 1); the patch of a box that is `[x]` on `main` is deleted.

### Step 1 — Find the next buildable box

The selection algorithm is `_format.md` §6; the range is `P1`..`P11`. The loop adds:

- **An `[!extern]` box in the target's `needs:` closure** → one Co-Pilot line (§8), collect the
  `[!extern]` box into its phase's owner-act batch, and continue with the next open box outside
  that closure (roles-and-escalation §4(d)). An undeclared dependency on it found at Step 2 or 3
  is a §4(d) escalation, never a build against an absent prerequisite. The phase-end sweep box
  blocks the whole successor phase (test-strategy §11.3).
- **`[!]`** → read its note, skip, report it at the phase end.
- **Parked** (a `parked/<box-id>.patch` file under the git dir, §6 park procedure) →
  skip it and its `needs:` closure like an `[!extern]` block; it is re-selected once
  the Co-Pilot releases the park.
- **Auto-unlock scan**, every iteration: flip each `[!]` box whose `unlocked-by:` box is `[x]` to
  `[ ]` (`_format.md` §5.2); the flips ride in the next box commit (Step 7).
- **Nothing buildable** → §6 stop (2).

### Step 2 — Unpack the box anatomy

Read the header line, prose, every sub-box and every note in full before deciding anything.
Sub-boxes are built top to bottom; one that meets the DoD on its own may be its own iteration
and commit, and the top box flips `[x]` in the commit that completes its last sub-box.

> **DECISION C — dependency-following, NOT box-skipping.** A `needs:` box that is not yet `[x]`
> and is buildable is built first (recursively, following its own `needs:`), then the loop
> **returns** to the target. Never skip, never leave a hole; a prerequisite the loop cannot build
> is Step 1's `[!extern]` / `[!]` case.

### Step 3 — Read ALL referenced spec `§§` and gate IDs, fully

Read every referenced spec `§` **in full** (not the box's paraphrase) and every
referenced gate ID in `build-gates.md`. The acceptance bar, the column/enum lists,
the error kinds, the IPC schemas, and the fail-mode of each gate are there. The cited `§`
body, parentheticals included, is the binding field list; a gap → derive it from a higher
layer and tag `[Derived-Assumption]` (Step 4), and escalate (roles-and-escalation §4(b)) only
when no higher layer anchors it — never improvise. If two spec `§§` contradict each other → run
the pre-check (roles-and-escalation §4(a)) first: build a compatible pair, reconcile a
rank-ordered pair in the same commit with a `Spec-Reconcile:` body line, and park the box (the
§6 park procedure) + escalate a same-rank fork (a scoped stop).

### Step 4 — Build per spec + write tests at the highest sensible level

- **Build** exactly per the cited `§§` and the CLAUDE.md §3 guardrails (zero egress, never harm
  the original, untrusted bytes decoded only in isolated subprocesses, MIT core clean,
  least-privilege Tauri and the locked §0.10 CSP).
- **Tests at the highest technically sensible level** for the layer
  ([test-strategy.md](test-strategy.md) §10). For a conversion this includes the
  **output-validity** bar: the produced file read back by a **real structural reader**
  (G31/G32), behind the §6.5 reliability ledger.
- **No green-by-rewrite.** A previously green test the change turns red is, by default,
  catching a regression in the new code: fix the code. A test edit is allowed only after proving
  **both** (1) the old expectation is obsolete (a spec-`§` or decision cite) **and** (2) the new
  one is correct (read back against the spec or the real result, never "it's green now"). The
  (1) cite sits in the G70 `[Test-Change: <box-id> — old-obsolete+new-correct, §ref]` tag at the
  changed test (a marker in a brand-new test: `[Test-Change: <box-id> — new-test:<reason>,
  §ref]`), the (2) read-back evidence on the brief's `Tests:` line; doctrine: test-strategy §8.
- **Same-commit sync (DoD (b)).** A deviation from the spec lands in the spec or security docs
  in the same commit; a change to any authoritative source — a gate, control, decision, path,
  convention, enum variant or version pin — reaches every doc that references it (G68); a new
  directory gets its spec §0.7 row in the commit that adds its first tracked file (G69).
- **Tags and records** (defined in roles-and-escalation §3): `[Build-Session-Entscheidung:
  <box-id>]` and `[Derived-Assumption: <box-id> — …]` at the code site, the `Spec-Reconcile:`
  body line; `[Test-Change]` → test-strategy §8.
- **`engines.lock` + SBOM row** for a newly staged engine; **§0.11 threat-map +
  security-concept §5 row** for a new threat class (§5).
- **Scope guard.** If a named gate already enforces the invariant, deliver the wiring plus one
  test that proves it fires; mutation evidence only for `scripts/` code; tooling for an absent
  input is hardened by the box that lands the first real input, named as a `needs:` edge or in
  its note. A caged path → Step 7.

### Step 4a — Pre-review sweep on the final staged tree

After the box's last `git add`, before R1 and after every fix round:

1. `lefthook run pre-commit --force` and `lefthook run pre-push --force`
   (`.gate-tools/bin/lefthook`; without `--force` a clean tree skips every command —
   CONTRIBUTING.md "Known gate traps").
2. A staged gate-plane path (the `run-gate-selftests` `CHANGED_PREFIXES`) →
   `python3 -P scripts/run-gate-selftests` in full.
3. The CI-only legs the diff touches: `src/**` → `pnpm test:a11y` and `pnpm test:coverage`;
   Rust, TypeScript or shell source → the containerized `check-sast --full`; `cfg(unix)` / Linux
   code or `scripts/**` → a run in the `convertia-linux` image (DEVELOPMENT.md "Windows host
   notes" carries both recipes); `.github/**` → `python3 -P scripts/check-ci-supply-chain`;
   macOS-only code: CI only.
4. The brief (Step 5) gets one `<gate>: exit <n>` line per gate run.

### Step 5 — Pre-commit Opus + Sonnet dual review (G1)

**Stage selectively first** — `git add <specific files>` (**never `git add -A`**), then
`git diff --cached --stat`, and record the index tree (`git write-tree`) as the round's tree
id. Spawn **two** reviewers (opus + sonnet) in parallel. Each receives exactly: the **reviewer
rubric** below verbatim; the **staged diff** inline (`git diff --cached` — **not** a SHA,
because it is not yet committed); the box (header through its last `>` note); the full text of
every spec `§` the box cites; the build-gates.md rows of every `Gnn` the box cites and of every
gate whose script or config the diff touches; CLAUDE.md §3 and §5; and the **review brief**.
The brief is the commit body's fields `Box/§/Gates:` through `Class:` (Step 6), written before
R1 and committed as reviewed, plus the gate transcript: one `<gate>: exit <n>` line per gate run
on the final staged tree. Record the model IDs the harness ran on the body's `Models:` line; a
model deprecation or rename surfaces as an escalation, never a silent skip.

**Reviewer contract.** Reviewers never change the shared tree, index or `target/` (no
checkout, stash, reset, commit or edit). A reviewer that must run code, a mutant or a repro does
it in its own scratch clone — a real `git clone`, never a junction or symlink of the repo — with
the staged diff applied and `CARGO_TARGET_DIR` inside the clone, reused across that reviewer's
rounds of the box. Reviewers run targeted commands (one test module, one gate script, one
repro), never the full suite; the brief carries the gate transcript. A third reviewer or a lens
run, when used, runs once, before R1, under the same severity bar. After every round the loop
compares `git write-tree` with the round's tree id before it edits anything.

```text
=== ConvertIA dual-review rubric (canonical — emitted to BOTH reviewers verbatim) ===
You are one of TWO independent pre-commit reviewers (opus + sonnet) of a ConvertIA
build commit. Input: the STAGED diff (git diff --cached, inline). Critique it for:

  1. COMPLETENESS  — does it fully build the box per the referenced spec §§, with no
     stub/placeholder/"phase 2"/"for now"/"comes in P<n>" deferral (those fail G8)?
  2. CORRECTNESS   — logic, error handling, edge/adversarial inputs, the no-panic
     policy on the in-core detect/fs_guard path, exhaustive dispatch matches.
  3. SPEC-CONFORMANCE — does it match the referenced spec §§ and the architecture
     guardrails (zero egress; never-harm-original atomic/exclusive publish;
     untrusted bytes decoded only in isolated subprocesses; MIT-core-clean;
     locked §0.10 CSP / least-privilege Tauri)?
  4. SECURITY      — does it open a network surface, weaken a gate, widen the CSP /
     capabilities, or touch a security-critical file? Name the threat class (§5).
  5. TEST-INTEGRITY (HIGH-SCRUTINY) — does the diff MODIFY / RELAX / SKIP / DELETE a
     test, or flip one red→green (a rewritten assertion, an added #[ignore]/it.skip/
     should_panic, a removed/commented-out assertion)? If so, ask explicitly: "is this
     SUPPRESSING A REAL REGRESSION?" The default is the CODE is wrong, not the test. A
     test change is acceptable ONLY if the commit proves BOTH (1) the old expectation
     is genuinely obsolete (a spec-§/decision cite) AND (2) the new expectation is
     correct (verified vs the spec / by reading back the real result, never "it's green
     now"). A red→green test edit lacking that (1)+(2) justification is a P0/P1
     finding. (Mechanical signal: G70 flags an unjustified suppression marker; YOUR job
     is the SEMANTIC call — test-strategy.md §8.)
  6. CLASS-CLOSURE (owner rule, 2026-08-26 — CLAUDE.md §10) — scope: a defect that
     already exists on main (a red CI run, a bug in committed code, an escalation, a
     sweep finding, or a review finding that shows the pattern in committed code). A
     finding confined to the diff under review is fixed at every instance in the diff
     and needs nothing more. For an in-scope fix ask: "can this or a SIBLING of it
     recur?" If yes, the commit carries the sibling sweep (the pattern grepped across
     plan/spec/code/gates) AND a closure. The closure is a NEW permanent catcher (a gate
     leg / lint / self-test) only when the class guards a security control, has
     recurred (a second instance in git history), or turned main red while the local
     gates were green; otherwise it is a one-line repo-homed note (CONTRIBUTING.md
     "Known gate traps" or the owning doc). Lowest rung first: prevent at the source,
     a note, a regex or plan-lint leg, a parser leg (only for a bypass of a security
     control). The Class: line names the closure or the one-off reason. A class left
     silently open is a P1. What bounds this item is SCOPE, not quality: a catcher
     beyond this trigger is an improvement outside the work at hand, not asked for;
     it goes to the residual ledger with its reason (CLAUDE.md §6), never silently
     dropped and never executed mid-box.
  7. PROSE ECONOMY (owner rule, 2026-09-09) — a count, an unmeasured "because"
     mechanism claim, or a coverage/done-ness claim in a comment, a plan note or the
     brief that no mechanical checker cross-checks is a defect in the PROSE,
     never a request for more prose: the fix is to reword it to the observable
     EFFECT or to drop it. Rank an unmeasured claim P2. Ask for a checker, a hedge
     or a deletion — not for narrative.

Rank every finding:
  P0    — a CLAUDE.md §3 guardrail broken, a gate weakened or bypassed, or something
          built that the spec forbids.
  P1    — a concrete failure, written as "scenario: <input/state> -> <wrong output |
          crash | behaviour the cited § requires and the diff lacks>"; an unswept
          sibling with the same scenario as a fixed P0/P1; a test or leg that stays
          green when the behaviour it claims to pin is removed; a test change without
          the item-5 (1)+(2) proof; an item-6 omission; an unmet DoD item — for DoD (b)
          only where normative text (a spec §, a gate-row contract, a test assertion)
          states behaviour the diff changed.
  P2/P3 — everything else, never blocking: wording, counts and mechanism claims in
          comments, plan notes or the brief; descriptive prose left stale; sweep
          breadth over sites that carry no defect.
A P0/P1 without its scenario line is recorded as P2. One line per finding: severity,
file:line, summary, and for a P0/P1 its scenario. State convergence/divergence
explicitly: "both agree on X" / "opus additionally: Y" / "divergence: opus sees A,
sonnet sees B". Even at zero findings, give ONE line saying why the diff is clean. Do
NOT collapse the two reviews — each reviewer reports separately. In a delta round (R2
onward) verify each prior P0/P1 fix and review the fix delta; outside the delta report
only a P0, or a P1 whose scenario you reproduced.

SPEC-CONTRADICTION is a finding CLASS ABOVE P0: two spec §§ (or the spec and the
SSOT) that no single implementation satisfies. A diff may instead reconcile a
RANK-ORDERED pair in the open (a Spec-Reconcile: line in the review brief): a
normative clause beats an illustrative literal, [DECIDED] beats [REC] or an untagged
statement, the owning § beats a restatement of it, a higher layer beats a lower one.
Argue the losing reading as strongly as you can; confirm the ranking only if it
still loses. Flag SPEC-CONTRADICTION when you dispute the ranking, when the pair is
same-rank, or when either side is SSOT text, restates a CLAUDE.md §3 guardrail or
sits in spec §0.10, §0.11 or §2.12. It parks the box (a scoped stop + escalate) and
is NEVER a working-tree fix.
=== end rubric ===
```

**Consolidating findings and rounds:**

- **R1** is a full review. **P0 / P1** → fix in the working tree, re-stage the affected files,
  and re-review. **No push between a fix and its re-review — there is no fix-push cycle.**
  Repeat until both reviewers are GO with no open P0/P1.
- **R2 onward are delta rounds.** Resume the SAME two reviewer agents (SendMessage) with one
  message: `G1 R<n> delta — <box-id>`, the prior round's P0/P1 lines with their scenarios,
  `git diff <tree R(n-1)> <tree R(n)>` inline, the new gate transcript, and "apply the rubric's
  delta-round rule; reply with a verdict line and one line per finding". An agent that cannot
  be resumed (crash, context limit) is replaced by a fresh one given the same inputs plus the
  prior findings lines and the delta.
- **Withdrawal.** Only the reviewer who raised a P0/P1 withdraws it. The loop may rebut it with
  evidence (a spec cite, a reproduction, a gate transcript) in the next delta round; it never
  down-ranks a finding itself.
- **R3 triage.** Entering R3, the loop settles each still-open P0/P1 one way: fix it; delete the
  disputed claim — only in a comment, a plan note or the brief, never normative spec text;
  refuse the exotic input form — only in first-party tooling or gate parsers with a declared
  input model, fail-closed, never a spec-required product input; or split the box, where the
  GO-able part meets the DoD on its own and the rest becomes a sub-box.
- **Round cap.** `open P0/P1 after round == 4` → park the box (§6 park procedure), a scoped
  stop. A split, re-cut or released box restarts at R1.
- **P2 / P3** → never blocking and not applied (except the post-GO rule below): one
  `Open P2/P3:` body line each; one that a later box must act on also gets a `>` note on
  that box or a new box with a `needs:` edge. Every `Open P2/P3:` line — and every
  improvement outside the work at hand, as its own `Open P2/P3: improvement …` line — is
  the intake of the [residual ledger](../plan/residual-ledger.md), which the Co-Pilot
  collects and triages at the phase-end sweep (test-strategy §11.2); the loop never edits
  the ledger file.
- **Divergence-resolution rule (canonical):** a **P0/P1 GO-vs-NOGO divergence is treated as
  NOGO — the stricter reviewer wins.** A P2/P3 divergence needs no resolution (both lines go
  to `Open P2/P3:`) — **unless** it is a SPEC-CONTRADICTION, which is the scoped stop +
  escalate above.
- **SPEC-CONTRADICTION** (either reviewer, including a disputed `Spec-Reconcile:` ranking) →
  run the §6 park procedure and escalate (the scoped stop of roles-and-escalation §4(a)),
  never a working-tree fix.
- **Post-GO (the single home of this rule; other documents point here).** Both reviewers' GO
  freezes the staged diff; record its tree id. The only edit permitted after GO is deleting a
  sentence a reviewer flagged, or correcting a factual error in it, when that sentence sits in
  a code comment, the box's own plan note or the brief. Never after GO: a spec, SSOT, security
  or process doc, an L(-1) path, or any non-comment line. Each such edit goes on a `Post-GO:`
  body line and is re-confirmed by the same two reviewers (one message each: the edit as a
  diff, verdict line only). A P0, or a P1 with a scenario, opens a delta round. Any other
  change after GO opens a delta round.
- **Reviewer availability:** on a reviewer error / timeout / rate-limit / 5xx,
  retry with backoff a bounded number of times, then **HARD-STOP + escalate** to
  Co-Pilot. **NEVER** auto-emit a `GO` trailer with fewer than **two live**
  reviews; never silently degrade to one or zero reviewers. (G12 checks the trailer
  is well-formed and that a `GO/GO` commit's body carries a review marker — a
  presence heuristic, not a per-reviewer parse — and cannot prove two live models
  ran; this rule is the load-bearing defence against a well-formed-but-unbacked `GO`.)
- **Staged-diff sanity (the trailer attests *this exact staged diff*):** immediately before
  `git commit`, `git diff <GO tree> $(git write-tree)` MUST be empty or consist only of the
  `Post-GO:` edits both reviewers re-confirmed. Any other difference (a file added or removed,
  a code or test line changed) needs a delta round. There is no silent post-review staging.

> **Recorded reviewer-family decision (do not run without it — plan-lint check 20
> asserts this is present).** Opus and Sonnet share model lineage, so "both `GO`,
> 0 findings" is a **correlated** signal, not two independent ones. **The
> correlated-lineage residual is explicitly ACCEPTED for v1** — the deterministic
> gates (every `Gnn` except G1) carry the real security weight and bound blast
> radius regardless of reviewer correlation; G1 is a quality amplifier. The
> accepted residual ships **with a concrete spot-audit cadence: a Co-Pilot
> auditable-smell spot-audit at every phase boundary AND a random ≥1-in-10-box
> sample** of the committed `GO/GO` Review records (both prongs run at the phase-end
> sweep, test-strategy §11.2) (a "both GO, 0 findings" on a non-trivial diff is the
> audit target). **The flip option remains open** — making one reviewer a different
> model family (e.g. a non-Anthropic model) to make "independent" literally true is a
> future owner decision that can be taken at any time.

There is no review-free commit: every commit carries `Dual-Review:` (G12).

### Step 6 — Commit + push (gates run; never bypass)

Commit message — **Conventional-commit** form (G11), in this template:

```
<type>(<scope>): <box-id> <summary>

Box/§/Gates: <box-id> · §<x.y> … · G<nn> …
What: <file group> — <what changed>
Decisions: <file:line> per [Build-Session-Entscheidung] / [Derived-Assumption] site, or none
Spec-Reconcile: <lower §> → <higher §> wins (<rule>)   (only when used)
Tests: <tests added or changed, with their level>; Test-Change <file:line> — <(2) read-back evidence>
Class: <closed: sibling sweep + catcher | closed: sibling sweep + note <where> | one-off: reason | homed: box-id | n/a>
Models: opus=<model id> sonnet=<model id>
Review: r1 opus=NOGO sonnet=GO
  P1 (opus) <file:line> <summary> — scenario: <…> → fixed
Review: r2 opus=GO sonnet=GO
Open P2/P3: <severity> <file:line> <summary>, or none
Post-GO: <file:line> <deleted or corrected sentence>, re-confirmed   (only when used)

Dual-Review: opus=GO sonnet=GO
<the attribution line the harness prescribes>
```

- `<type>` ∈ `feat|fix|chore|docs|refactor|test|perf|ci|build`; `<scope>` is
  `[a-z0-9._-]+`; at most **100 characters**. A box commit starts the summary with its
  box id. A commit that builds no box (a Co-Pilot act, a re-land) uses
  `<type>(<scope>): <summary>` and writes `none` for the box on the `Box/§/Gates:` line.
  **Rollback convention** (solo on `main`): `chore(scope): roll back — <reason>`, with
  **no `revert` type** for build-session commits. G11 rejects a subject over 100
  characters or one not followed by a blank line (L3 + L4; merge/revert/fixup/squash/amend
  exempt).
- `Box/§/Gates:` through `Class:` are the review brief (Step 5), committed as reviewed.
  After GO the loop adds only `Models:`, the `Review:` lines, `Open P2/P3:` and
  `Post-GO:`. The review record is one `Review:` line per round plus one indented line
  per P0/P1 finding (severity, raising reviewer, file:line, summary, scenario,
  resolution) — never the reviewers' prose. The **`Dual-Review:` trailer** is
  mandatory, sits in the final trailer block, and is machine-checked at pre-push (G12,
  exact form `Dual-Review: opus=(GO|NOGO) sonnet=(GO|NOGO)`).
- **Body economy (owner rule, 2026-09-09).** The body is evidence, not narrative.
  Write it in the template's fields. A count appears only where a checker
  cross-checks it (the runner's `N legs` rows, `--shortstat`); a mechanism is written
  as its observable effect, never as an unmeasured "because". Aim for ≤ 40 lines.
  G11 rejects a body over 120 counted lines (the non-blank lines git stores after
  the subject; a final trailer block of at most 5 lines is not counted).
- **Commit** in the foreground; a hook red → fix → Step 4a → Step 5 → commit again. **A red gate
  is fixed, never bypassed:** no `--no-verify`, no force-push, no `core.hooksPath` redirection,
  no disabled required CI check.
- **Rebase before the push:** `git fetch origin main`; if `origin/main` moved,
  `git rebase origin/main`, re-run Step 4a (1)–(2), then push. A rebase never counts as a push
  failure; a conflict → `git rebase --abort` → §6 stop (4).
- **Push status** is `git push`'s own exit status, never read through a pipe (DEVELOPMENT.md
  "Windows host notes"). A non-zero exit reports `Push gescheitert (exit N), Lefthook-Hook X rot`;
  the loop fixes the cause and pushes again (§6 stop (4) counts the failures).
- **Egress-window rule (G42/G42b):** never push while the previous push's `ci` run is
  unresolved, and never start Step 4 of the next box before that run concluded green. The run is
  found by `headSha` (`gh run list --workflow ci --branch main --json
  databaseId,headSha,status,conclusion`) and observed without blocking; a run a newer push
  cancelled (the `ci` concurrency cancel G56 asserts) is superseded by the successor run, and a
  cancel with no successor is §6 stop (4); a GitHub API error is retried with backoff.
- **A red `ci` run is attributed by SHA.** The loop's own last push, failing inside its diff →
  one uncaged fix-forward commit through Steps 4a–6 (a new commit, never an amend); a Co-Pilot
  commit is never fixed forward by the loop; anything else → §6 stop (3).

### Step 7 — Check off the box

The check-off rides in the box commit: `[x]` for the box and its completed sub-boxes plus the
Step 1 auto-unlock flips, staged before Step 4a — markers only.

**A caged part** — a `scripts/l-neg1-files.toml` path (G71) that must change in the same push: build
the uncaged part, run Step 4a (not Step 5), and run the §6 park procedure with reason `caged` (the
Co-Pilot line names `caged part: <paths> (roles-and-escalation §4(g))`). The owner-acked Co-Pilot
applies the patch in its clone, adds the caged part, runs Steps 4a and 5 on the whole diff and
makes the box commit with `Dual-Review:` and `L-neg1-ack: owner`; its landed commit is the park's
answer. The loop never writes that trailer, and deletes the patch once the box is `[x]` (Step 0).
The preference for a caged part: a precondition box (`needs:` on an `[!extern]` box), then a
`caged` park, then a pre-declared sweep tail (§5 (b)).

---

## 4. Decide it yourself vs escalate

Decide by default, tagged; grep the codebase and the process docs for an established pattern
first. Escalate only on roles-and-escalation §4 (a)–(h). The §8 line goes to the running Co-Pilot
session when reachable and is always printed; a message is a notification — rulings and acks
reach the loop only as landed commits (Step 0).

Everything else: **decide and proceed, tagged.** When two genuinely professional
options exist, decide strictly at the owner's core-rule anchor (CLAUDE.md §6 — the
cleanest, most complete, most professional solution for the work at hand wins; scope,
not quality, bounds it), **not** reflexively by the cheaper one.

---

## 5. Definition of Done (the canonical 8-point list — this file is canonical)

> `plan-lint` check 14 keeps the G1 bullet and P0.6.5 letter-identical; this file wins.

A change is **done** only when:

- **(a)** **Spec-`§` or gate-id referenced** in the commit — or deliberately marked
  tooling-only.
- **(b)** **Spec/docs synced in the same commit** — a deliberate or forced deviation
  is reflected in the spec/security docs in the *same* commit; code never outlives
  the spec that covers it; where the doc is L(-1) and the box pre-declares it, gate-row or
  security-concept text that reds nothing may land as the phase-end sweep tail.
- **(c)** **Tests at the highest technically sensible level are green** (unit /
  property / per-pair integration / corpus / E2E per the layer; for a conversion,
  the output-validity bar — the produced file read back by a real structural reader,
  G31/G32, behind the §6.5 reliability ledger).
- **(d)** **Hard gates green** (`cargo clippy -D warnings`, `tsc --noEmit`,
  eslint/stylelint, `cargo fmt`/prettier, the test suite, `plan-lint`/`spec-lint`)
  — **without** `--no-verify` and without `core.hooksPath` redirection.
- **(e)** **The Opus + Sonnet pre-commit dual review (G1) is through** — the review
  recorded in the commit body (one `Review:` line per round, one line per P0/P1
  finding with its resolution — §3 step 6), trailer `Dual-Review: opus=… sonnet=…`
  present. P0/P1 findings fixed in the working tree, re-staged, re-reviewed before
  push (no fix-push cycle); P2/P3 on the body's `Open P2/P3:` lines.
- **(f)** **Inline decision tags set** at every non-spec choice site —
  `[Build-Session-Entscheidung: <box-id>]`, directly at the code site, not only in
  the commit body.
- **(g)** **`engines.lock` + SBOM row** added if a new engine was staged; the row lands
  before the staging box (a precondition box, roles-and-escalation §4(g)) or in the box's
  own commit (§3 Step 7), never after it.
- **(h)** **§0.11 threat-map + security-concept §5 row** added if a new threat class
  was introduced; the §0.11 row in the same commit, the security-concept §5 row in it or
  as a pre-declared sweep tail. (Items (g) and (h) fire **independently** — either alone
  requires its action.)

---

## 6. Hard-stops, token-Notbremse, and the gate-quarantine escape

**The loop stops itself** only on:

1. **the owner's stop word** — it parks an unfinished box (reason `stop`); the owner's next start
   re-selects it;
2. **nothing buildable** — zero open boxes (§9), or every open box inside a parked, escalated or
   `[!extern]` closure;
3. **a red `main` not attributable to its own last push** (Step 6);
4. **a hard-stop class** — `>= 3 consecutive push failures`, a second red after the
   fix-forward, a rebase conflict, reviewer unavailability (Step 5), an anomalous cancel, the
   GitHub API unreachable while a run is unobserved, a Step 0 mismatch.

On a stop it leaves a clean tree, posts the §8 line and schedules no next iteration.

**Scoped stops** (park the box, continue outside its `needs:` closure):

- **G1 non-convergence** — `open P0/P1 after round == 4` (§3 step 5), and only after the
  pattern lookup (§4): a P0/P1 with an established pattern is fixed, not parked. A
  scoped stop: run the park procedure below and continue outside the box's `needs:`
  closure.
- A **spec contradiction the pre-check cannot rank** (roles-and-escalation §4(a)) —
  regardless of severity, never silently reconciled; the loop runs the park procedure
  below and keeps building outside the box's `needs:` closure.
- **A caged part** (§3 Step 7).

**Park procedure (a scoped stop for one box).** (1)
`mkdir -p "$(git rev-parse --git-common-dir)/parked"`, `git add -N` the box's new files, then
write `git diff --binary HEAD -- <the box's paths>` to
`$(git rev-parse --git-common-dir)/parked/<box-id>.patch`, first line
`# base <HEAD sha> reason <caged|ruling|stop>`; the git dir survives the session and never shows
in `git status`. (2) Restore only the box's paths:
`git apply --check -R` the patch, then `git restore --staged --worktree -- <the box's paths>`; it
also drops step (1)'s intent-to-add entries and deletes the new files, so `git status` lists no
box path — never a blanket `reset --hard`, `clean` or `stash`. (3) Post one
Co-Pilot line (§8): `Co-Pilot: park <box-id> — <reason> — patch <absolute path> @ <base sha7>`.
(4) Continue outside the box's `needs:` closure (§3 step 1 skips a parked box); when nothing
buildable remains, stop. The Co-Pilot answers with a landed commit — a split of the box, a ruling
on the disputed point, or a `>` note naming the points narrowed rounds may examine — then renames
the file to `<box-id>.patch.released`. The loop re-selects the box, may `git apply` the released
patch as its starting point, restarts at R1, and deletes the released file when the box commits.
The reason field: `ruling` (G1 non-convergence, a spec contradiction, a gate quarantine) is
answered as above; `caged` is answered by the Co-Pilot's box commit (§3 Step 7); `stop` runs
steps (1)–(3), then the loop ends, and the next start re-selects the box with no release.

**Token-Notbremse, per box:** no session box limit; the Step 5 round cap and stop (4) bound a
box; a phase boundary is no stop of its own (the sweep box blocks its successor phase, Step 1).

**Gate quarantine.** A required gate that fails closed on a false positive is unblocked only by
an owner-acked Co-Pilot act (gate code and `lefthook.yml` are caged): a fix, or a narrow scope
with a restore box id — never `--no-verify`. The loop parks the box and escalates
(roles-and-escalation §4(e)). A single Semgrep false positive in loop code gets
`// nosemgrep: <rule-id>` at the site (CONTRIBUTING.md "Known gate traps").

---

## 7. Escalation, conversation, and the start/stop vocabulary

The owner starts the loop once with the Claude Code `/loop` skill, self-paced, and this prompt:

```text
Build-Loop: follow docs/process/build-loop.md; one iteration = §3 Step 0 to Step 7 for one box; stop only on a §6 stop.
```

**Cadence: `== 1 box per iteration`.** Each iteration runs Step 0 to Step 7 for one box, pushes,
and schedules its next wake-up. A wake-up whose Step 0 finds the previous push's `ci` run still
in progress runs Steps 1–3 only, prints the §8 waiting line and schedules the next wake-up at the
run's expected end. Compaction carries the loop across boxes; after one it re-reads §2 and resumes
the step in progress, since Step 0 runs only when an iteration starts. An iteration that schedules
no wake-up ends the loop, so the loop schedules none only on a §6 stop.
There is no batching and no session box limit.

Escalation goes **Build-Loop → Co-Pilot → owner** ([roles-and-escalation.md](roles-and-escalation.md)),
as a single own line right after the status line (§8), never inlined into a box summary, naming
the count, the severity and the source `§`/file.

**Vocabulary** — the owner's own messages in the loop session only, never an agent message:

| Word | Effect |
|---|---|
| `los` / `start` / `weiter` | Start the loop (the `/loop` start above) from the next buildable box (a `stop` park is re-selected). |
| `eine` / `one` | Build exactly one box, then stop. |
| `bis ende phase PN` | Build until phase `PN` is complete, then stop. |
| `stop` / `halt` / `pause` | §6 stop (1). |
| `status` | Emit the current state (last `[x]`, next `[ ]`, open `[!]`/`[!extern]`, parks). |
| `skip <reason>` | Skip the current box with a recorded reason (owner-directed only). |
| `revert` | Roll back the last box (`chore(scope): roll back — <reason>`, no `revert` type). |

---

## 8. Output discipline

**One line per box.** No more:

```
P3.4 done — CSV→TSV atomic publish wired drop→detect→convert→publish, tests green, SHA <short>, Review: P0=0 P1=0 P2=1 P3=0
```

A P1-or-worse finding, a clarification, a park or a spec inner contradiction goes on its
**own** line immediately after, never inlined:

```
Co-Pilot: 1 item — SPEC-CONTRADICTION §2.7.2 vs §2.14.2 on cross-volume publish (scoped stop, escalated)
```

While a `ci` run is unresolved the loop prints `waiting on ci <run-id>` with the elapsed time at
each wake-up. At each **phase boundary**: a mini-report — boxes built, commits, parks, and the
consolidated `[!extern]` list for that phase.

---

## 9. Convergence & crash-recovery

**Convergence report** (§6 stop (2)): the boxes completed with their commit SHAs, the parked
boxes with their patch paths, and the **consolidated `[!extern]` list = the owner/Co-Pilot action
list**. The Co-Pilot works it per phase — the owner-act box and each precondition box as its own
owner-acked act, the caged tails at the sweep box (test-strategy §11.4); the phase-end sweep box
blocks its whole successor phase (test-strategy §11.3). Never loop forever.

**Crash-recovery procedure (a session crash mid-box is recoverable without manual
surgery — `plan-lint` check 18 asserts a canonical phrase for this exists here):**

- **(a)** A **partial staged state** → `git reset HEAD` + re-read the box (no
  half-staged commit); a box re-entered after a crash restarts its G1 review at R1.
- **(b)** **Committed-but-CI-red** → a **NEW** commit fixing it (the Step 6 fix-forward);
  **never amend a pushed commit**.
- **(c)** **Committed-but-not-pushed** → push first (Step 6, with its rebase).
- **(d)** **Push is idempotent on retry** — a re-push of an already-pushed commit is
  a no-op, safe to repeat.

---

## 10. The non-negotiables (never break)

1. **On `main`, clean tree, hooks not redirected** — the Step 0 sanity runs every iteration.
2. **Never** `--no-verify`, **never** force-push, **never** `core.hooksPath`
   redirection, **never** disable a required CI check, **never** edit a gate to pass
   it (the §6 gate quarantine is an owner-acked Co-Pilot act).
3. **Two live reviews or no `GO`** — never auto-emit a trailer with fewer than two
   live reviewers.
4. **A spec contradiction is never silently reconciled** — the loop reconciles only a
   pair the roles-and-escalation §4(a) pre-check ranks, in the open; a same-rank fork is
   a scoped stop + escalate (the loop is downstream of the spec).
5. **Conflict order, always:** SSOT > spec > security/process docs > plan > code >
   conversation.
6. **Build fully, no stub as a default** — the cleanest, most complete, most
   professional solution for the work at hand wins over token cost, session speed and
   pragmatism; scope, not quality, bounds it, and an improvement outside the scope is a
   residual-ledger line, never mid-box work (CLAUDE.md §6).
7. **The loop never authors a caged line** (§3 Step 7).

---

## 11. References

- Project rules: [`CLAUDE.md`](../../CLAUDE.md) · box format and selection:
  [`docs/plan/_format.md`](../plan/_format.md) · the plan: [`docs/plan/README.md`](../plan/README.md)
- Gates and threat model: [`build-gates.md`](../security/build-gates.md) ·
  [`security-concept.md`](../security/security-concept.md)
- Process: [`roles-and-escalation.md`](roles-and-escalation.md) · [`test-strategy.md`](test-strategy.md) ·
  [`vuln-response.md`](vuln-response.md)
- SSOT (what & why): [`docs/SINGLE-SOURCE-OF-TRUTH.md`](../SINGLE-SOURCE-OF-TRUTH.md)
