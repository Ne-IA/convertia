# ConvertIA — Build-Loop (the master prompt / runbook)

> **The runbook for the autonomous Build-Loop session.** It is written to be read
> *as a prompt*: a fresh session reads it top to bottom, then starts. It is the
> single canonical home of the **8-point Definition-of-Done**, the **hard-stop /
> token-Notbremse numbers**, the **reviewer rubric**, the **recorded
> reviewer-family decision**, the **crash-recovery procedure**, and the
> **dual-review divergence-resolution rule** — several `plan-lint` checks assert
> these live *here* verbatim (checks 14, 15, 18, 19, 20). If a copy of any of them
> drifts elsewhere, **this file wins**.
>
> **Conflict order (unchanged, every layer):**
> **SSOT > spec > security/process docs > plan > code > conversation.**
> When two layers disagree, the higher one wins — **never silently reconcile,
> always escalate**. A spec-internal contradiction (two `§§` disagree) is an
> **unconditional hard-stop + escalate** regardless of severity — the Build-Loop is
> downstream of the spec and cannot pick a side.

---

## 0. Who runs this, and what it is not

| Session | Role |
|---|---|
| **Build-Loop** (this file) | Autonomous. Builds the plan box by box (**P1 onward**), writes tests, runs every gate + the dual review, commits **directly to `main`** + pushes. The gates are the protection — **no second branch, no merge step**. |
| **Co-Pilot** | Escalation & clarification target; strategic / cross-phase decisions; high-level review. Works with the owner. Executes the standing **phase-end hardening sweep** box that closes every phase `P2`..`P11` ([test-strategy §11](test-strategy.md#11-the-phase-end-co-pilot-hardening-sweep)). The Build-Loop escalates *to* Co-Pilot; it never merges or rewrites history on its own. |

This is a **single-branch (`main`), GitHub + GitHub-Actions** model. There are **no
worktrees, no parallel branches, no push-lock coordination, no separate
feedback/sniping sessions, no `safe-push` wrapper, no merge step, no auto-merge.**
Because only one session builds and commits, there is **no push contention** —
ordinary `git push` is used; the safety comes from the gates (L1–L5), not from
branch isolation. The only surviving `PR` concept is the **external fork
pull-request** (this is a *public* OSS repo); "per-PR" anywhere else means
"per-push". **Incoming PRs (external fork PRs + Dependabot bumps) are reviewed and
merged by the Co-Pilot/owner session, NEVER by this loop** — the loop has no merge
step; ownership + the `engines.lock`-bump re-validation rule are in
[roles-and-escalation.md §5a](roles-and-escalation.md#5a-incoming-pull-requests--dependabot-bumps--owned-by-co-pilot-never-the-loop).

> **Bootstrap note (DECISION B — read this before assuming any range).** **P0 is
> NOT built by this loop.** P0 (`docs/plan/P0-build-and-security.md`) is the
> *bootstrap* phase, built **manually by the Co-Pilot session + the owner**,
> because P0 *creates* the loop, the gate system, and the dual review that every
> later phase runs under — the loop cannot build the thing it runs inside.
> **This loop's range is `P1`..`P11`.** The dual review (G1) still applies to P0,
> but it is driven manually there. By the time the loop starts at P1, the seven P0
> docs — `security-concept.md`, `build-gates.md`, **this file**, `test-strategy.md`,
> `roles-and-escalation.md`, `_format.md`, `vuln-response.md` — are live and the
> two enforcement planes are green. If you find yourself reaching into P0, **stop**:
> you are out of range.

**The dual review is a quality amplifier, NOT a security control.** G1 is
self-attested through an unverifiable commit trailer; a gamed `GO/GO` cannot, by
itself, ship insecure code, because the **only security controls are the
deterministic gates — every `Gnn` except G1**. A gate either passes on a clean
checkout or it does not. G1 raises quality and catches design defects the gates
cannot encode.

---

## 1. Mission

Work `docs/plan/` **strictly**. **One `[ ]` box per iteration**, **lowest phase
first**, **top to bottom** within a phase. A phase is "done" when every `[ ]` step
under it is `[x]`.

**Never build on a hunch.** The acceptance criteria for a box do not live in the
box — they live in the **spec `§§`** and the **gate IDs** the box references, and
in the [SSOT](../SINGLE-SOURCE-OF-TRUTH.md) above them. **If you have to guess, you
missed something** — re-read the referenced spec section in full, or escalate. The
box is a pointer; the spec is the contract.

**Build it fully — no stub as a default** (SSOT Principle 1: completeness within
scope; CLAUDE.md §6: the cleanest / most-complete / most-professional solution
ALWAYS wins over token-cost, session speed, and "pragmatism"). A stub is only ever
a **named, compile-time interface shell** that a **named, scheduled** box fills
(the P3 `crate::isolation` interface shells P4 expands are the sanctioned example,
plan/README.md P3) — never a quiet placeholder, never a "Phase 2 / for now / comes
in P\<n\>" deferral (those phrasings fail G8). The entire gate layer exists
*precisely* so this priority holds; ranking pragmatism above it undercuts the
whole protection layer.

---

## 2. Read these at session start (mandatory)

Before the first box of a session, read — in this order:

1. [`CLAUDE.md`](../../CLAUDE.md) (repo root) — the project rules, the conflict
   rule, the 8-point DoD summary, the anti-patterns, the owner's core rule. (The
   org-wide Ne-IA platform rules — one directory *above the repo root*, i.e.
   `../../../CLAUDE.md` from this file, or `../CLAUDE.md` from the repo root — are
   inherited; the repo-root `CLAUDE.md` carries only ConvertIA-specific rules.)
2. [`docs/plan/_format.md`](../plan/_format.md) — the **box format**: `[ ]/[x]/[!]/
   [!extern]`, the tag set, sub-boxes, the `needs:` and `unlocked-by:` dependency
   annotations, the per-phase-file + index convention.
3. **This file** in full (the loop, the DoD, the hard-stops, the reviewer rubric).
4. [`docs/process/roles-and-escalation.md`](roles-and-escalation.md) — when to
   escalate, to whom, and what decide-it-yourself looks like.
5. [`docs/process/test-strategy.md`](test-strategy.md) — the testing doctrine (test
   levels: unit / property / per-pair integration / corpus / E2E, and the
   output-validity bar) applied at step 4 of **every** box; read it before the
   first box so the very first test-level decision is correct.

Then, **per box** (step 3 of the loop), read the box's referenced **spec `§§` in
full** and the referenced **gate IDs** in
[`build-gates.md`](../security/build-gates.md). Never skim a referenced section —
acceptance criteria, column lists, enums, error kinds, and IPC schemas live there,
not in the box.

> **Context-routing — the loop runs LEAN (references, not inlines).** This prompt
> **points** at [security-concept.md](../security/security-concept.md) /
> [build-gates.md](../security/build-gates.md) for a per-box gate lookup and a red-CI
> fix — it deliberately does **NOT inline the whole gate/security corpus** into the
> loop session (no context ballast). The loop reads only the `Gnn` rows a box
> references, on demand. The **full picture** — the complete gate catalogue, the
> threat model, the cross-phase security view — is the **Co-Pilot's** to hold
> ([roles-and-escalation.md](roles-and-escalation.md): Co-Pilot owns/holds the full
> security+gate picture; the loop looks up on red-CI). A fill-pass must never paste
> the corpus into this prompt — the lean-loop / full-Co-Pilot split is asserted by the
> P0.1.7 documentation-wiring & context-routing audit.

At the end of session-start, emit exactly one line and **wait for a start word**
(see §10) — do not proactively build:

```
Bereit. Letzte abgehakte Box: <id>, naechste baubare Box: <id>.
```

---

## 3. The loop (step 0 → step 7), one box per iteration

### Step 0 — Start sanity (once per session, the very first action)

These are unconditional STOP conditions on mismatch — the autonomous-direct-to-`main`
model depends on every one of them:

- **Repo root:** `git rev-parse --show-toplevel` MUST be the ConvertIA repo root.
  Mismatch → **STOP**, report, do not build.
- **Branch:** `git symbolic-ref --short HEAD` MUST be `main`.
- **Clean tree:** no half-staged / half-committed state; `git status` clean before
  any box starts.
- **Remote:** `origin` points at `github.com/Ne-IA/convertia`.
- **No hooks bypass:** `git config --get core.hooksPath` MUST be **unset OR equal
  the lefthook-managed path**. A `core.hooksPath` redirect silently disables ALL
  local L1–L3 hooks **without** `--no-verify` — it is in the forbidden-bypass set
  (CLAUDE.md §5, security-concept §3). Its verifying pre-push enforcement is **G54**
  (which resolves the *effective* hooks dir, not a hardcoded `.git/hooks/`).
- **No out-of-band gate tamper:** `git diff --exit-code HEAD -- scripts/
  lefthook.yml .github/` clean (the two-plane principle does not cover a local
  out-of-band edit to a gate script).
- **Gate-currency:** `git fetch origin main && git diff --name-only HEAD origin/main`
  — if any gate file (`lefthook.yml`, `scripts/**`, `.github/**`, `deny.toml`,
  `.gitleaks.toml`, `.npmrc`, lockfiles) drifted on `origin` (a Co-Pilot/owner gate
  bugfix mid-session), **STOP + escalate** rather than continue on stale gate
  scripts. Its verifying pre-push enforcement is the **G54 push-from-stale-base
  guard** (`git merge-base HEAD origin/main == origin/main`).
- **Startup `[!]` dep-unlock scan (before first-box selection):** run the full
  `[!]` dep-unlock scan (§3 step-1) at startup, not only after each check-off, so a
  box another (P0-bootstrap) session already unblocked is selectable on this
  session's first box.
- **CI health:** query the last Lane-A run on `main`, filtered to the Lane-A
  workflow + `push` event + `main` branch (without the filter a scheduled
  Scorecard / G56a run or a tag/release run false-passes or false-stops the loop):
  `gh run list --workflow <lane-A> --event push --branch main --limit 1 --json
  status,conclusion,headSha`. **STOP + escalate** on `failure`/`cancelled`;
  proceed on `success`/`pending`/`queued`; **fail-open** (warn + continue) if the
  API is unreachable. **First-push-to-empty-remote fail-open:** on session 1
  against a fresh repo there is no prior run — treat the absent run as fail-open
  (warn + continue), do NOT misread it as a red push and do NOT count it toward the
  3-push-failures escalation.

> Fire the **CI-health check at EVERY box start**, not only session start (one
> cheap `gh` call), to close the mid-session red-`main` gap between a push (step 6)
> and the next box.

### Step 1 — Find the next buildable box

Scan **all** `docs/plan/P*.md`, **lowest phase first, top to bottom**; the next
buildable box is the first `[ ]` that is not `[!extern]`/`[!]`-blocked. Concretely:
the lowest-phase / lowest-position open `[ ]` box whose `needs:` dependencies (if
any) are all `[x]`.

- **`[!extern]`** (nothing for the loop to build — an owner/external action) →
  **skip + collect** into the consolidated `[!extern]` list; the loop continues.
  **Exception — a real block:** a non-extern box that *names an `[!extern]` box as
  a prerequisite* → **STOP** instead of skipping.
- **`[!]`** (blocked) → read the note under it, skip, mention at the phase end.
- **Auto-unlock scan:** after each check-off (and at startup), scan for `[!]` boxes
  carrying an `unlocked-by: <box-id>` marker whose dep is now `[x]`, and flip them
  `[!]`→`[ ]` (`_format.md`'s reverse-unlock direction).
- **Zero open boxes** → emit the convergence report (§9) and **stop**; never loop
  forever. A genuine all-blocked deadlock → escalate.

### Step 2 — Unpack the box anatomy

Read the box header (`[ ] **<box-id>** [Tag] Title · §spec-refs · gate-ids`), its
prose, **and every indented `  - [ ]` sub-box** and `>`-note, in full, **before**
deciding anything. Sub-boxes are built strictly top to bottom; the **top box is
checked off only after every sub-box is done**. The dual review (step 5) fires
**once per top box** over the combined sub-box diff, **not** per sub-box.

> **DECISION C — dependency-following, NOT box-skipping (read this carefully).**
> If the chosen box has a **`needs: P<x>.<y>`** annotation pointing at a box that
> is not yet `[x]`, **do not skip and do not leave a hole**: **build that
> prerequisite box first** (recursively — follow its `needs:` too), then **RETURN**
> to the original box and build it. The plan is dependency-following, not
> dependency-stepping-over.
>
> This **replaces a `[!]`-block-and-skip model**, which we do **not** use: that model
> would mark a box `[!]` and move on, leaving a hole to be filled later out of
> order. ConvertIA instead resolves the dependency *in place* — the `needs:`
> annotation makes the prerequisite *detectable*, and the loop satisfies it before
> returning. (`needs:` = "this box requires that one"; the inverse `unlocked-by:` =
> "this box, when done, releases that one" — one coherent dependency vocabulary,
> two directions, both defined in `_format.md`.)
>
> The `[!]` marker still exists for a box that is blocked on something the loop
> genuinely **cannot** build (an owner action, an external dependency) — that is a
> skip-and-report, not a dependency to follow. The distinction: a `needs:` on a
> *buildable* box ⇒ **build the prerequisite, then return**; a `[!]` / `[!extern]`
> ⇒ **skip + report** (and STOP if a non-extern box hard-requires it).

### Step 3 — Read ALL referenced spec `§§` and gate IDs, fully

Read every referenced spec `§` **in full** (not the box's paraphrase) and every
referenced gate ID in `build-gates.md`. The acceptance bar, the column/enum lists,
the error kinds, the IPC schemas, and the fail-mode of each gate are there. **If
the spec is incomplete or ambiguous for what the box needs → escalate (§7), do not
improvise.** If two spec `§§` contradict each other → **unconditional hard-stop +
escalate** (the loop is downstream of the spec).

### Step 4 — Build per spec + write tests at the highest sensible level

- Build the code / config / engine-staging / doc exactly per the spec `§`. Honor
  the architecture guardrails (CLAUDE.md §3): zero egress; never harm the original
  (atomic, exclusive, no-clobber publish on the resolved real file); untrusted
  bytes decoded only in isolated subprocesses (the §2.12 boundary is absolute; the
  one in-core exception is the pure-Rust CSV/TSV engine); MIT core clean / copyleft
  isolated; least-privilege Tauri + the locked §0.10 CSP.
- **Tests at the highest technically sensible level** for the layer (unit /
  property / per-pair integration / corpus / E2E — see
  [`test-strategy.md`](test-strategy.md)). For a **conversion**, this includes the
  **output-validity** bar: the produced file is read back by a **real structural
  reader** (G31/G32), not "the engine returned no error", with a representative
  real-world corpus behind the §6.5 reliability ledger.
- **No green-by-rewrite (the mindset for a test that the change turns red).** If the
  box's change makes a **previously-passing** test go red, the **default assumption is
  the CODE is wrong, not the test**. Rewriting/relaxing/skipping/deleting the test to
  get green is allowed (and is usually right), but **only** after proving **both** (1)
  the old expectation is genuinely obsolete (cite the spec-`§`/decision that changed
  the behaviour) **and** (2) the new expectation is correct (verified against the spec
  or by reading back the real result — never "it's green now"). If (1)+(2) can't be
  proven, fix the code. This does **not** forbid changing a test — it requires a
  one-line `[Test-Change: <box-id> — old-obsolete+new-correct, §ref]` justification;
  the mechanical signal is **G70**, the doctrine is [test-strategy.md](test-strategy.md)
  §8.
- **Spec-sync in the same commit:** a deliberate or forced deviation from the spec
  is reflected in the spec/security docs **in the same commit** — code never
  outlives the spec that covers it (living-doc rule). Run `plan-lint`/`spec-lint`
  `--quiet` locally before staging if a doc was edited.
- **Doc-graph freshness in the same commit (the general form of spec-sync — DoD item
  (b)).** A change to **any** authoritative source — a gate (`Gnn`), a control, a
  decision, a path/directory, a convention, an enum variant, a version pin — is
  reflected in **every** referencing doc in the **same commit**: no stale, no
  contradictory, no orphaned `.md`. **G68** (doc-graph integrity & freshness) enforces
  it graph-wide (the gates→`.md` case is one instance); a drift reddens the push.
- **Structural-map update in the same commit.** Never create a structural element (a
  folder) that is not in the **CLAUDE.md §1a "Repo layout" map** — if a new directory
  is genuinely needed for clean logical separation, **update the map in the same
  commit**. **G69** asserts the map ↔ on-disk tree bidirectionally; an unmapped folder
  (or a stale map entry) reddens the push.
- **Inline decision tags** at every non-spec choice site:
  `[Build-Session-Entscheidung: <box-id>]` for a self-made pattern/naming/default
  choice, **directly at the code site**, not only in the commit body. (This tag
  also suppresses G8 at a documented choice site; a bare `[!extern]` does **not**
  suppress in production code.)
- **`engines.lock` + SBOM row** if a new engine was staged; **§0.11 threat-map +
  security-concept §5 row** if a new threat class was introduced.

### Step 5 — Pre-commit Opus + Sonnet dual review (G1)

**Stage selectively first** — `git add <specific files>` (**never `git add -A`**),
then `git diff --cached --stat`. Spawn **two** reviewers, in parallel, on the
**staged diff** (`git diff --cached`, inline — **not** a SHA, because it is not yet
committed). Their model IDs are **pinned exactly** (like every other tool); a
deprecation/rename surfaces as an escalation, never a silent skip. Both receive the
**reviewer rubric** below verbatim, plus the relevant spec `§§`, this catalogue,
CLAUDE.md, and the box.

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

Rank every finding P0 (must-fix, blocks) → P1 (must-fix) → P2 → P3. Give each one a
one-line reason WITH a spec-§ or file ref. State convergence/divergence explicitly:
"both agree on X" / "opus additionally: Y" / "divergence: opus sees A, sonnet sees B".
Even at zero findings, give ONE line saying why the diff is clean. Do NOT collapse
the two reviews — each reviewer reports separately.

SPEC-CONTRADICTION is a finding CLASS ABOVE P0: if two spec §§ disagree (a §
cross-reference inconsistency), flag it as SPEC-CONTRADICTION — it is an
unconditional hard-stop + escalate, NEVER a working-tree fix (the loop is
downstream of the spec and cannot pick a side).
=== end rubric ===
```

**Consolidating findings:**

- **P0 / P1** → **fix in the working tree, re-stage the affected files, and
  re-review** (loop). **No push between a fix and its re-review — there is no
  fix-push cycle.** Repeat until both reviewers are GO with no open P0/P1.
- **P2 / P3** → documented in the commit body + the status line; raise a follow-up
  box if structural; not a blocker.
- **Divergence-resolution rule (canonical):** a **P0/P1 GO-vs-NOGO divergence is
  treated as NOGO — the stricter reviewer wins.** A **P2/P3 divergence is resolved
  by the loop** with a recorded `[Build-Session-Entscheidung]` rationale — **unless**
  it is a SPEC-CONTRADICTION, which is the unconditional hard-stop + escalate above.
- **SPEC-CONTRADICTION** (either reviewer) → **hard-stop + escalate**, never a
  working-tree fix.
- **Reviewer availability:** on a reviewer error / timeout / rate-limit / 5xx,
  retry with backoff a bounded number of times, then **HARD-STOP + escalate** to
  Co-Pilot. **NEVER** auto-emit a `GO` trailer with fewer than **two live**
  reviews; never silently degrade to one or zero reviewers. (G12 checks the trailer
  is well-formed and that a `GO/GO` on a non-trivial diff carries each reviewer's
  non-empty findings block, but it cannot prove two live models ran — this rule is
  the load-bearing defence against a well-formed-but-unbacked `GO`.)
- **Staged-diff sanity (the trailer attests *this exact staged diff*):**
  immediately before `git commit`, `git diff --cached --stat` MUST match the file
  set the two reviewers saw at GO; any file added/removed after GO **requires
  re-review** — no silent post-review staging.

> **Recorded reviewer-family decision (do not run without it — plan-lint check 20
> asserts this is present).** Opus and Sonnet share model lineage, so "both `GO`,
> 0 findings" is a **correlated** signal, not two independent ones. **The
> correlated-lineage residual is explicitly ACCEPTED for v1** — the deterministic
> gates (every `Gnn` except G1) carry the real security weight and bound blast
> radius regardless of reviewer correlation; G1 is a quality amplifier. The
> accepted residual ships **with a concrete spot-audit cadence: a Co-Pilot
> auditable-smell spot-audit at every phase boundary AND a random ≥1-in-10-box
> sample** of the committed `GO/GO` findings blocks (a "both GO, 0 findings" on a
> non-trivial diff is the audit target). **The flip option remains open** — making
> one reviewer a different model family (e.g. a non-Anthropic model) to make
> "independent" literally true is a future owner decision that can be taken at any
> time.

**Skip the dual review only** for: **(a)** a check-off commit with no code/config
diff (a markdown-only `chore(todo): … abgehakt`/`done` commit); **(b)** an
`[!extern]` box (nothing was built).

### Step 6 — Commit + push (gates run; never bypass)

Commit message — **Conventional-commit** form (G11), subject on the first line, no
special characters in the subject (`—`/`#` only in the body):

```
<type>(<scope>): <short summary>

<spec-§ ref> · <box-id> · <P2/P3 findings, if any>

Dual-Review: opus=GO sonnet=GO
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

- `<type>` ∈ `feat|fix|chore|docs|refactor|test|perf|ci|build`; `<scope>` is
  `[a-z0-9._-]+`. The **`Dual-Review:` trailer** is mandatory and machine-checked
  at pre-push (G12, exact form `Dual-Review: opus=(GO|NOGO) sonnet=(GO|NOGO)`); each
  reviewer's findings + convergence/divergence are recorded **verbatim** in the
  body. **Rollback convention** (solo on `main`): `chore(scope): roll back —
  <reason>` — **no `revert` type** for build-session commits.
- **Push exit code MUST be observed reliably.** The agent tool environment does not
  propagate a subprocess exit code the way a plain shell does: `| tee` masks the
  hook's non-zero exit, and a naive `$?` can capture the tool-call's own success
  rather than `git push`'s. **The mechanism:** a **background push + marker-file +
  a synchronous foreground until-loop polling the marker file** —

  ```bash
  LOG=/tmp/convertia-push.log; DONE=/tmp/convertia-push.done
  rm -f "$LOG" "$DONE"
  git commit -m "..."            # commit fires L1 (pre-commit) + L3 (commit-msg)
  { git push origin main; echo "EXIT=$?" >> "$DONE"; } > "$LOG" 2>&1   # push fires L2 (pre-push)
  # then a FOREGROUND until-loop polling $DONE for the EXIT= line; read the code FROM it (not a bare $?)
  ```

  `run_in_background` is **FORBIDDEN for the wait-loop** (it must block
  synchronously on the marker), as are `| tee` and `pgrep`-polling the push
  process. **The same marker-file capture applies to the check-off (hak) push — it
  is NOT exempt.** On a non-zero `EXIT`, the push did **not** go through (a hook
  blocked it): report `Push gescheitert (exit N), Lefthook-Hook X rot` — **never**
  report a generic "push done (exit 0)". Do not start a new box, and do not push
  the check-off commit, while a push is unresolved.
- **A red gate is fixed, not bypassed.** Lefthook pre-push red → fix the cause →
  re-stage → re-review (step 5) → new commit. **Never `--no-verify`, never
  force-push, never `core.hooksPath` redirection, never disable a required CI
  check.** **3 consecutive gate-red pushes → hard-stop + escalate.**
- **When the red is a TEST the box's change made fail — apply the code-first
  default, do NOT green-by-rewrite.** A test that the change turned red is, by
  default, **catching a regression in the new code** — so the first move is **fix the
  code**, not the test. A test edit to get green is permitted **only** after proving
  **both** (1) the old expectation is genuinely obsolete (cite the spec-`§`/decision)
  **and** (2) the new expectation is correct (verified vs the spec / by reading the
  real result back, never "it's green now"); that **(1)+(2) justification goes in the
  commit body** and the edit is a **high-scrutiny item for the step-5 dual review**
  (rubric point 5). The mechanical signal that an unjustified test-suppression marker
  (`#[ignore]`/`it.skip`/a `should_panic` on a real assertion/a removed assertion)
  slipped in is **G70** — it FLAGS + REQUIRES the `[Test-Change: <box-id> —
  old-obsolete+new-correct, §ref]` justification (or, for a marker in a brand-new
  test with no prior expectation, the net-new variant `[Test-Change: <box-id> —
  new-test:<reason>, §ref]`), it does **not** forbid the change. Doctrine:
  [test-strategy.md](test-strategy.md) §8.
- **Watch your own CI run (your push to `main` IS `main`).** Capture the run
  **SHA-anchored** — `git rev-parse HEAD`, then `gh run list --branch main --json
  databaseId,headSha,status` filtering `headSha == <commit-sha>` (NOT `--limit 1`
  "most recent run", which the fast second push — the check-off — can steal), then
  **`gh run watch --exit-status <run-id>`** (`--exit-status` is **MANDATORY** —
  without it `gh run watch` exits 0 even on a failed run, a no-op guard). A
  non-zero exit ⇒ the same STOP + escalate as session-start. A `cancelled`
  conclusion from the G56 concurrency cancel is reconciled by the
  **successor-exists check** (a higher-`databaseId` run for a *later* SHA): if a
  successor exists the cancel was expected and the loop waits on it; if **no
  successor exists, the cancel is anomalous → STOP + escalate**. **Transient GitHub-API
  failure during the wait (r15):** `gh run list` / `gh run watch` (and the push-wait above)
  are wrapped in a **bounded retry-with-backoff** for a transient API error / rate-limit /
  5xx / timeout — the same posture as the step-5 reviewer-availability retry; if the API
  stays unreachable **beyond the bounded retry**, the loop **hard-stops + escalates** (it
  never silently proceeds past an unobserved CI run, and never treats an unreachable API as
  a green run). This is a **mid-session** rule — distinct from the step-0 startup CI-health
  check, which fail-OPENS if unreachable; once a box is in flight, an unobservable CI run is
  a STOP, not a fail-open. The loop emits a periodic **liveness/heartbeat** status line while
  blocked on CI (§8) so the operator can tell "building" from "silently wedged".
- **Egress-window protection — the one mandatory rule:** **the loop MUST NOT push
  the check-off commit until the box-commit run completes green**
  (`gh run watch --exit-status <box-run-id>` returns 0). (`cancel-in-progress:
  false` on the G42/G42b job group is a belt-and-suspenders G56 assertion, **not**
  an alternative.)

### Step 7 — Check off the box

Edit the box `[ ] **<box-id>**` → `[x] **<box-id>**` (and each sub-box marker).
Commit with the **canonical check-off shape** — a double predicate: subject matches
`chore(todo): .* (abgehakt|done)` **AND** the diff is **markdown-only**. Push with
the same marker-file wait pattern as step 6 (the check-off push is **not** exempt);
the docs-only fastpath skips the heavy hooks because the diff is provably
markdown-only (G54 recognises exactly this shape). Then run the **auto-unlock scan**
(step 1) and emit the status line (§8).

---

## 4. Decide it yourself vs escalate

**Default: decide it yourself.** Routine implementation / pattern / naming / default
choices are the loop's to make — **grep the codebase + the process docs for an
established pattern first** (escalate a P0 only if none exists, so a routine choice
with a precedent is never escalated), then decide, and tag the choice site with
`[Build-Session-Entscheidung: <box-id>]`.

**Escalate to the Co-Pilot session** only when one of these is genuinely true:

- **Spec / SSOT contradiction** — two `§§` disagree, or the spec disagrees with the
  SSOT. **Unconditional hard-stop + escalate** (never a working-tree fix).
- **Cross-phase architecture decision with no source** — a design choice that binds
  later phases and is not derivable from the spec/SSOT.
- **Scope / legal conflict** — the box implies work outside the
  *Explicitly Out of Scope* line (store/marketing/legal advice/binary code-signing),
  or a license/copyleft conflict.
- **A dependency that genuinely cannot be followed** — a `needs:` box that the loop
  cannot build (it requires an owner action / external input), or an all-blocked
  deadlock with nothing open.
- **A provably-misfiring required gate** — see the gate-quarantine procedure (§6).
- **Reviewer unavailability** — two live reviews cannot be obtained after bounded
  retry (§3 step 5).

Everything else: **decide and proceed, tagged.** When two genuinely professional
options exist, decide strictly at the owner's core-rule anchor (CLAUDE.md §6 —
cleanest / most-complete / most-professional wins), **not** reflexively by the
cheaper one.

---

## 5. Definition of Done (the canonical 8-point list — this file is canonical)

> `plan-lint` check 14 holds the **G1, P0.6, and this** copy of the list
> item-count- and item-identifier-identical; if they ever disagree, **this file
> wins**. This 8-point list derives from a prior-project nine-point DoD with the
> **RLS/tenant, immutable-audit-log-row, and migrations rows dropped** (no
> multi-tenant DB in an offline desktop app) and the **`engines.lock`/SBOM row and
> the §0.11+§5 threat-class row added** (9 − 3 + 2 = 8). Output-validity is **not**
> a ninth item — it lives inside item (c)'s highest-sensible-test bar.

A change is **done** only when:

- **(a)** **Spec-`§` or gate-id referenced** in the commit — or deliberately marked
  tooling-only.
- **(b)** **Spec/docs synced in the same commit** — a deliberate or forced deviation
  is reflected in the spec/security docs in the *same* commit; code never outlives
  the spec that covers it.
- **(c)** **Tests at the highest technically sensible level are green** (unit /
  property / per-pair integration / corpus / E2E per the layer; for a conversion,
  the output-validity bar — the produced file read back by a real structural reader,
  G31/G32, behind the §6.5 reliability ledger).
- **(d)** **Hard gates green** (`cargo clippy -D warnings`, `tsc --noEmit`,
  eslint/stylelint, `cargo fmt`/prettier, the test suite, `plan-lint`/`spec-lint`)
  — **without** `--no-verify` and without `core.hooksPath` redirection.
- **(e)** **The Opus + Sonnet pre-commit dual review (G1) is through** — both
  reviewers' findings + convergence/divergence recorded verbatim in the commit
  body, trailer `Dual-Review: opus=… sonnet=…` present. P0/P1 findings fixed in the
  working tree, re-staged, re-reviewed before push (no fix-push cycle); P2/P3 noted
  in the body.
- **(f)** **Inline decision tags set** at every non-spec choice site —
  `[Build-Session-Entscheidung: <box-id>]`, directly at the code site, not only in
  the commit body.
- **(g)** **`engines.lock` + SBOM row** added if a new engine was staged.
- **(h)** **§0.11 threat-map + security-concept §5 row** added if a new threat class
  was introduced. (Items (g) and (h) fire **independently** — either alone requires
  its action.)

---

## 6. Hard-stops, token-Notbremse, and the gate-quarantine escape

**Stop the loop (hard-stop + escalate) on any of:**

- The owner writes a stop word (`stop` / `halt` / `pause`).
- **3 consecutive gate-red pushes** despite fix attempts.
- A **dual-review P0 that is genuinely not fixable** — but only after the
  pattern-lookup (§4): a P0 with an established pattern is **not** a hard-stop, apply
  the pattern.
- A **spec-internal contradiction** (two `§§` disagree) — unconditional, regardless
  of severity, never silently reconciled.
- **Reviewer unavailability** — two live reviews unobtainable after bounded retry.
- An **anomalous CI cancel** (a `cancelled` run with no successor — §3 step 6).
- A needed **L(-1) security-critical-file edit** — the loop NEVER edits a
  security-critical file (the gates' own cage) autonomously; hard-stop + escalate so the
  owner makes/acks it (`L-neg1-ack: owner`, G71; security-concept §2,
  roles-and-escalation §4(g)).
- **GitHub API unreachable mid-session beyond the bounded retry** — during the push-wait
  or `gh run watch` (§3 step 6), a transient API error / rate-limit / 5xx / timeout is
  retried with backoff; if it cannot be resolved the loop hard-stops + escalates rather than
  proceed past an unobserved CI run (distinct from the step-0 startup health check, which
  fail-opens if the API is unreachable).

**Token-Notbremse / cadence numbers (the ConvertIA v1 baselines — `plan-lint` check
15 asserts these appear verbatim here):**

- **Soft-stop** — `soft-stop fires when committed-box-count >= 8` in one session;
  pause and summarize for the owner.
- **Hard-stop** — `hard-stop at == 12` committed boxes in one session; a new session
  is required.
- **Phase-change hard-stop** after ≥1 committed box of a new **top-level** phase
  (re-orient at a phase boundary) — this applies to any **top-level phase boundary
  (Pn→Pn+1)**, **NOT** the P0.1–P0.7 clusters; for this autonomous loop that always
  means a P1→P2…→P11 transition (the loop never reaches a P0.x boundary, §0
  bootstrap note). It does **NOT** fire if the box counter is **0** (all of the new
  phase's boxes so far were `[!extern]`/`[!]`-skipped).
- **P0 cluster soft-stop** (bootstrap phase only, driven manually):
  `cluster soft-stop at >= 5 committed boxes` — fires at a P0.x cluster boundary once
  that many boxes have been committed since the last soft-stop (a running counter that
  **resets at each soft-stop**; it pauses at the *next* cluster boundary after the
  counter crosses 5). This number governs the **manual P0
  bootstrap** run; this autonomous loop never reaches a P0.x boundary (§0 bootstrap
  note) — it lives here only because `build-loop.md` is the single canonical home of
  every cadence number (`plan-lint` check 15).
- **`>= 3 consecutive push failures` = hard-stop + escalate.**

The box counter **increments only on a COMMITTED box** — `[!extern]`/`[!]`-skipped
boxes do **not** count (so a soft-stop cannot fire spuriously after 5 skips,
consistent with the phase-change hard-stop's count-0 exemption). The digits are the
v1 baseline, tunable by the owner.

**Opt-in box-batching:** at most **3 sister-boxes** in the same cluster, **no
sub-boxes / no cross-deps**, trivial repetition only (e.g. structurally-identical
per-engine rows) ⇒ **1 build-commit + 1 dual-review over the combined diff + 1
check-off**, counter `+= N`. In doubt, build singly. The dual review fires **once
per top box**, never per sub-box.

**Gate-quarantine procedure (a provably-misfiring *required* gate, no-bypass model).**
A required gate that fails **CLOSED on a false positive** (a tool regression on a
version bump, a Semgrep/zizmor/actionlint misfire) would wedge the loop with no
sanctioned escape that is not a forbidden `--no-verify`. The **only** sanctioned
unblock is a **committed, dual-reviewed change** that fixes — or **narrowly scopes /
temporarily suppresses** — the specific gate check, with a tracked **restore
box-id**, so the fix goes through the same gates and the gate is never silently
skipped. A misfiring gate that cannot be scoped this way is a hard-stop + escalate.
**Bootstrap exception for a self-referential deadlock** (a gate that fails closed on
its *own* fix commit — e.g. a plan-lint parser bug that blocks the plan-lint fix):
the sanctioned escape is a **committed, narrowly-scoped edit to `lefthook.yml`** that
comments out **only that one gate command** (a `lefthook.yml` edit matches a
*different* glob than the misfiring gate, so it is not blocked by it) — **never
`--no-verify`** — paired with a `[Build-Session-Entscheidung]` tag and a restore
box-id; the gate is re-enabled in the immediately following box. For a single
legitimate Semgrep/advisory false-positive, prefer the **per-finding suppression
ledger** (a committed entry with a content-derived fingerprint + box-id +
Dual-Review note, the fingerprint **rotating** when the surrounding code changes) —
the gate-quarantine sledgehammer is for a tool-level misfire with no per-finding
fingerprint.

---

## 7. Escalation, conversation, and the start/stop vocabulary

Escalation goes **Build-Loop → Co-Pilot → owner** (see
[`roles-and-escalation.md`](roles-and-escalation.md)). Escalate as a **single, own
line** right after the status line (§8) — never inlined into a box summary — naming
the count, the severity, and the source `§`/file.

**Start / stop vocabulary (the owner drives the loop with these):**

| Word | Effect |
|---|---|
| `los` / `start` | Begin the loop from the next buildable box. |
| `weiter` / `continue` | Resume after a soft-stop / a clarification. |
| `eine` / `one` | Build exactly one box, then pause. |
| `bis ende phase PN` | Build until phase `PN` is complete, then stop. |
| `stop` / `halt` / `pause` | Save state and wait. |
| `status` | Emit the current state (last `[x]`, next `[ ]`, open `[!]`/`[!extern]`). |
| `skip <reason>` | Skip the current box with a recorded reason (owner-directed only). |
| `revert` | Roll back the last box (`chore(scope): roll back — <reason>`, no `revert` type). |

---

## 8. Output discipline

**One line per box.** No more:

```
P3.4 done — CSV→TSV atomic publish wired drop→detect→convert→publish, tests green, SHA <short>, Review: P0=0 P1=0 P2=1 P3=0
```

A P1-or-worse finding, a clarification, or a spec inner contradiction goes on its
**own** line immediately after, never inlined:

```
Co-Pilot: 1 item — SPEC-CONTRADICTION §2.7.2 vs §2.14.2 on cross-volume publish (hard-stop, escalated)
```

Batched boxes: **one status line per box** (each with the shared SHA + shared review
result). At each **phase boundary**: a mini-report — boxes built, commits, and the
consolidated `[!extern]` list for that phase.

**Liveness while blocked.** While the loop is blocked waiting on a CI run
(`gh run watch`, §3 step 6) or a long-running box, it emits a **periodic
liveness/heartbeat line** (the run-id + elapsed wait) so the operator can distinguish
"building / waiting on CI" from "silently wedged". A wait that exceeds the bounded retry
on an unreachable GitHub API is the §6 **mid-session hard-stop**, not a silent hang.

---

## 9. Convergence & crash-recovery

**Convergence report (zero open boxes / end of session):** boxes completed + their
commit SHAs + the **consolidated `[!extern]` list = the owner/Co-Pilot action list** (the owner rules it; the standing test-strategy §11 phase-end sweep boxes on it are Co-Pilot-executed), so the
owner has one scannable hand-off. Never loop forever; on zero open boxes, report and
stop.

**Crash-recovery procedure (a session crash mid-box is recoverable without manual
surgery — `plan-lint` check 18 asserts a canonical phrase for this exists here):**

- **(a)** A **partial staged state** → `git reset HEAD` + re-read the box (no
  half-staged commit).
- **(b)** **Committed-but-CI-red** → a **NEW** commit fixing it; **never amend a
  pushed commit**.
- **(c)** **Pushed-but-not-checked-off** → the normal open-box scan (§3 step 1)
  catches it; the check-off is idempotent on retry.
- **(d)** **Push is idempotent on retry** — a re-push of an already-pushed commit is
  a no-op, safe to repeat.

---

## 10. The non-negotiables (never break)

1. **On `main`, clean tree, hooks not redirected** before any box; the
   session-start sanity (§3 step 0) is unconditional.
2. **Never** `--no-verify`, **never** force-push, **never** `core.hooksPath`
   redirection, **never** disable a required CI check, **never** edit a gate to pass
   it (the gate-quarantine escape in §6 is the only sanctioned exception, and it is
   itself a committed, dual-reviewed, gated change).
3. **Two live reviews or no `GO`** — never auto-emit a trailer with fewer than two
   live reviewers.
4. **Spec contradiction = hard-stop**, never silently reconciled — the loop is
   downstream of the spec.
5. **Conflict order, always:** SSOT > spec > security/process docs > plan > code >
   conversation.
6. **Build fully, no stub as a default** — the cleanest / most-complete /
   most-professional solution wins over token-cost and speed (CLAUDE.md §6).

---

## 11. References

- Project rules / DoD summary / anti-patterns: [`CLAUDE.md`](../../CLAUDE.md)
- Box format + dependency vocabulary: [`docs/plan/_format.md`](../plan/_format.md)
- The plan (executable TODO): [`docs/plan/README.md`](../plan/README.md) ·
  [`docs/plan/P0-build-and-security.md`](../plan/P0-build-and-security.md)
- Gate catalogue (G1..Gnn): [`docs/security/build-gates.md`](../security/build-gates.md)
- Security concept (threat model + defense-in-depth): [`docs/security/security-concept.md`](../security/security-concept.md)
- Roles & escalation: [`roles-and-escalation.md`](roles-and-escalation.md)
- Test strategy: [`test-strategy.md`](test-strategy.md)
- Vulnerability response (CVE → user, no auto-update): [`vuln-response.md`](vuln-response.md)
  *(authored in P0.6.9)*
- SSOT (what & why): [`docs/SINGLE-SOURCE-OF-TRUTH.md`](../SINGLE-SOURCE-OF-TRUTH.md)
