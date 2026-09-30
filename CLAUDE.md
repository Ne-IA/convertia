# CLAUDE.md — ConvertIA

> Persistent instructions for Claude Code in **this** repo. Short, specific,
> project-unique. Generic best-practices live in the system prompt, **not** here —
> this file only adds what Claude must know about *this* project. The org-wide
> rules live one level up in `../CLAUDE.md` (Ne-IA platform); this file does not
> repeat them.

**Conflict rule:** **SSOT > spec > security/process docs > plan > code > conversation.**
When two layers disagree, the higher one wins — **never silently**. SSOT
([`docs/SINGLE-SOURCE-OF-TRUTH.md`](docs/SINGLE-SOURCE-OF-TRUTH.md))
is the tech-free *what & why*; the [spec](docs/spec/README.md) is the *how* derived
from it; the [security/process](docs/security/security-concept.md) docs are the
*how we build it safely*; the [plan](docs/plan/README.md) is the executable TODO.
A spec contradiction (two `§§` disagree) first runs the pre-check in
[roles-and-escalation.md](docs/process/roles-and-escalation.md) §4(a): a rank-ordered pair is
reconciled in the open, in the same commit (a `Spec-Reconcile:` body line); a same-rank fork is a
**scoped stop + escalate** — the build is downstream of the spec and never picks a side it cannot rank.

---

## 1. Project identity

- **ConvertIA** — a portable, install-free **desktop file converter**: drop a file
  on one drop area, get it in another sensible everyday format. Audience is the
  everyday person, not specialists (the canonical inclusion test lives in SSOT
  *What It Converts*).
- **MIT licensed, fully open** — free as in freeware *and* free as in source.
  **Ne-IA's first fully-open product.** Public repository under the **Ne-IA**
  GitHub org (`github.com/Ne-IA/convertia`). `Copyright (c) 2026 Ne-IA and
  ConvertIA contributors`; inbound = outbound, no CLA (DCO sign-off may be
  requested).
- **Fully offline, cross-platform.** One codebase, **one artifact per platform**
  (Windows / macOS / Linux desktop) — three builds, one product. No mobile, web,
  or CLI build in v1.
- **Tauri v2** — Rust core + a React 19 / TypeScript / Tailwind / Vite WebView UI
  (00-architecture §0.4.0). Conversion engines are **bundled third-party binaries**
  (FFmpeg, libvips, LibreOffice, poppler, pandoc; native Rust CSV/TSV) shipped as
  sidecars/resources, run as **isolated subprocesses** — everything is in the build,
  nothing is fetched at runtime.
- **v1 is one large, all-or-nothing public release** (SSOT *v1 Definition of Done*):
  no minimal-viable tiering, no fixed deadline — completeness is the gate. Partial
  *public* release is not a thing; internal *sequencing* (the plan's phases) is.

## 1a. Repo layout

The directory map is spec [§0.7 "Physical tree"](docs/spec/00-architecture.md) — directories only,
plus its *Load-bearing files* list; §0.7 outranks this file (SSOT > spec > docs). **G69** binds it
both ways to the git-tracked tree ([`build-gates.md`](docs/security/build-gates.md) §6 check 26). A
new directory gets its §0.7 row in the commit that adds its first tracked file; a new file owes no
row.

## 2. Working model — two sessions, one branch

| Session | Role |
|---|---|
| **Build-Loop** | Autonomous. Builds the plan box by box (**P1 onward**), writes tests, runs every gate + the dual review, commits directly to `main`. The gates are the protection — no second branch, no merge step. |
| **Co-Pilot** | Escalation & clarification target for the Build-Loop; strategic decisions; high-level review. Works with the owner. Executes the standing phase-end hardening sweep box that closes every phase `P2`..`P11` ([test-strategy §11](docs/process/test-strategy.md#11-the-phase-end-co-pilot-hardening-sweep)). |

- **Single branch (`main`), GitHub, GitHub Actions.** No worktrees, no parallel
  branches, no merge step. Enforcement = **CI green on `main` + required status
  checks on every push**; a red `main` is fixed immediately, never `--no-verify`,
  never force-push. The only surviving `PR` concept is the external **fork** PR
  (public OSS repo); "per-PR" elsewhere means "per-push".
- **P0 is bootstrapped manually** (Co-Pilot session + owner), **not** by the
  Build-Loop — because P0 *creates* the loop, the gate system and the dual review
  that every later phase runs under. The Build-Loop starts at **P1**. The dual
  review still applies to P0, driven manually.
- **Dependency-following, not box-skipping.** If the next buildable box needs an
  unbuilt box, **build the prerequisite first, then return** — never leave a hole.
  A forward dependency is declared with a `needs: P<x>.<y>` annotation on the box
  (the box format that carries it is authored in `docs/plan/_format.md`, a P0.1
  deliverable); it is the inverse of the plan's existing reverse-unlock marker
  `unlocked-by: <box-id>`, which flips a `[!]`-blocked box to `[ ]` once its
  prerequisite is `[x]`. `needs:` = "this box requires that one"; `unlocked-by:` =
  "this box, when done, releases that one" — one coherent dependency vocabulary,
  two directions.
- **Escalation path:** Build-Loop → Co-Pilot → owner. The Build-Loop decides
  routine implementation/pattern/naming/default choices itself (grep for an
  established pattern first); it escalates on genuine blocks and on a spec
  contradiction the roles-and-escalation §4(a) pre-check cannot rank (a scoped stop).
- **The dual review is a quality amplifier, not a security control.** The only
  security controls are the **deterministic gates** (every `Gnn` except G1). G1
  raises quality and catches design defects the gates can't encode.
- **Process is canonical in:** [`docs/process/build-loop.md`](docs/process/build-loop.md)
  (master prompt, hard-stop thresholds, the 8-point DoD, crash-recovery),
  [`docs/process/roles-and-escalation.md`](docs/process/roles-and-escalation.md) and
  [`docs/process/vuln-response.md`](docs/process/vuln-response.md) (the CVE→user path).
  **Bootstrap note:** `docs/process/` is authored in **P0.1/P0.6** and does not exist
  until then — while P0 is still being bootstrapped, the canonical process rules and
  the DoD live in **P0.6 of** [`docs/plan/P0-build-and-security.md`](docs/plan/P0-build-and-security.md).
  The Build-Loop starts at P1, by which point these files are live.

## 3. Architecture guardrails (always / never)

- **Fully offline, zero egress.** Every in-scope conversion ships in the build and
  runs with **zero network access**. No update check, no phone-home, no telemetry.
  The *only* network activity is **one user-initiated** action — opening the
  About → Releases link (`tauri-plugin-updater` is **absent by decision**, §7.6.1).
  No silent network call, ever.
- **Never harm the original.** Source files are never overwritten or deleted, even
  when source and target format match. Output keeps the source base name + the
  target extension; no-clobber numbering only appends `(1)`, `(2)`…. The final
  write is **atomic and exclusive (create-new-or-fail)**, evaluated on the
  *resolved real file* (symlink/alias/junction/hardlink safe), so a conversion
  **either fully succeeds or leaves no file behind** — even across a crash. The
  source set is **frozen at drop**.
- **Untrusted bytes are decoded only in isolated subprocesses — never in the core.**
  ConvertIA ingests arbitrary, possibly-malicious files; third-party decoders are a
  classic attack surface. A decoder crash/hang fails that one item clearly without
  wedging the app or breaking no-harm. The §2.12 isolation boundary is **absolute**;
  the *single* sanctioned in-core path is the pure memory-safe Rust CSV/TSV engine
  (`EngineProgram::InProcessNative`, §3.5.6), which decodes no third-party C/C++
  bytes.
- **MIT core clean; copyleft isolated.** ConvertIA's own code is MIT. GPL/LGPL/AGPL
  engines ship as **separate, independently-invoked binaries** (aggregation, not
  static linking into the MIT core); their obligations are honored (license text +
  written offer of source where required) and surfaced via NOTICE /
  third-party-licenses + the SBOM.
- **Least-privilege Tauri.** A locked capabilities/permissions allowlist + the
  locked §0.10 CSP object (no remote origin, no `unsafe-eval`, no `asset:`, no
  updater/deep-link/URL-scheme). G47 asserts the CSP and capabilities structurally.

## 4. Definition of Done

The canonical **8-point DoD** lives in [`docs/process/build-loop.md`](docs/process/build-loop.md);
`plan-lint` check 14 holds the **G1, P0.6 and build-loop.md** copies item-count- and
item-identifier-identical. **`build-loop.md` is canonical; this section is a
human-readable summary and is *not* itself one of the plan-lint-policed copies** — if
it ever disagrees, build-loop.md wins. **Bootstrap note:** until P0.1/P0.6 author
build-loop.md, the authoritative list is **P0.6 of**
[`docs/plan/P0-build-and-security.md`](docs/plan/P0-build-and-security.md) (line 146).
A change is **done** only when:

1. **Spec-`§` or gate-id referenced** in the commit — or deliberately marked
   tooling-only.
2. **Spec/docs are synchronous in the same commit** — a deliberate or forced
   deviation is reflected in the spec/security docs in the *same* commit; code
   never outlives the spec that covers it.
3. **Tests at the highest technically sensible level are green** (unit / property /
   per-pair integration / corpus / E2E, per the layer — see
   [`docs/process/test-strategy.md`](docs/process/test-strategy.md)). For a
   conversion this *includes* the **output-validity** bar: the produced file is read
   back by a **real structural reader** (G31/G32 — not merely "the engine returned
   no error"), with a representative real-world corpus behind the §6.5 reliability
   ledger.
4. **Hard gates green** (`cargo clippy -D warnings`, `tsc --noEmit`, eslint/stylelint,
   `cargo fmt`/prettier, the test suite, `plan-lint`/`spec-lint`) — **without**
   `--no-verify` and without `core.hooksPath` redirection.
5. **The Opus + Sonnet pre-commit dual review (G1) is through** — the review
   recorded in the commit body (one `Review:` line per round, one line per P0/P1
   finding with its resolution), trailer `Dual-Review: opus=… sonnet=…` present.
   **P0/P1 findings are fixed in the working tree, re-staged and re-reviewed before
   push** (no fix-push cycle); P2/P3 on the body's `Open P2/P3:` lines, the intake of
   the [residual ledger](docs/plan/residual-ledger.md).
6. **Inline decision tags set** at every non-spec choice site —
   `[Build-Session-Entscheidung: <box-id>]` for self-made pattern/naming/default
   choices, directly at the code site, not only in the commit body.
7. **`engines.lock` + SBOM row** added if a new engine was staged.
8. **`§0.11` threat-map + security-concept `§5` row** added if a new threat class
   was introduced. (Items 7 and 8 fire **independently** — either alone requires its
   action.)

(This 8-point set derives from a prior-project nine-point DoD with the RLS/tenant,
immutable-audit-row and migrations rows **dropped** — no multi-tenant DB in an
offline desktop app — and the two ConvertIA-specific facts (`engines.lock`/SBOM
row, threat-class row) **added**: 9 − 3 + 2 = 8. Output-validity is not a ninth
item — it lives inside item 3's highest-sensible-test bar, per P0.6's canonical
list. The derivation is recorded in P0.6 of
[`docs/plan/P0-build-and-security.md`](docs/plan/P0-build-and-security.md).)

## 5. Anti-patterns (NEVER)

- `any` (`: any` / `as any`) in TypeScript; an untyped IPC boundary (the generated
  `bindings.ts` is the *only* IPC door, §0.4.5).
- `TODO` / `FIXME` / `unimplemented!` / `todo!` / `dbg!` / `console.log` /
  `println!` in production code — and the semantic-deferral vocabulary ("for now",
  "later", "not yet", "comes in P<n>", "currently absent") that escapes a
  `TODO`-only scan (G8).
- `unreachable!` in **production** code — the exhaustive-match `clippy` deny on the
  dispatch enums (G4/G14, P0.4) makes the compiler enforce exhaustiveness, so it is
  never needed for dispatch; allowed only in an unreachable-by-construction
  `#[cfg(test)]` branch with a comment.
- **Any network call** outside the one user-initiated About → Releases link. No
  runtime fetch of engines, no update check, no telemetry. (Because there is **no
  auto-update**, the only path a security fix reaches a user is a new full release —
  the runbook is [`docs/process/vuln-response.md`](docs/process/vuln-response.md),
  authored in P0.6; see the §2 bootstrap note for where the rule lives until then.)
- **A dependency that opens a network surface** — `tauri-plugin-http` (it wraps
  `reqwest` and registers an IPC-accessible HTTP client on init), or any
  `reqwest`/`ureq`/`hyper`/`isahc`/`curl`-class crate, in `Cargo.toml`. Caught at
  compile time by `cargo-deny [bans]` (G18, P0.3) — the no-network *runtime* rule
  has an explicit *dependency-level* enforcement surface, so a careless
  `cargo add tauri-plugin-http` fails before it can escape G18 and G29 rule (g).
- **GPL/AGPL/LGPL contamination of the MIT core** — copyleft is isolated as a
  separately-invoked binary, never linked into the core.
- **Skeleton/stub as a default.** Build it fully (SSOT Principle 1: completeness
  within scope). A stub is only ever a named, compile-time interface shell that a
  *named, scheduled* box fills — never a quiet placeholder.
- **Rewriting / relaxing / skipping / deleting a failing test to make it pass
  WITHOUT proving the old assertion is genuinely obsolete AND the new one correct.**
  A red test may be catching a real regression in the new code; the default is **the
  code is wrong**, not the test. (Changing a test IS allowed and usually right — but
  **verified + justified**, never assumed: prove (1) the old expectation is obsolete
  (cite the spec-`§`/decision) **and** (2) the new expectation is correct (verified vs
  the spec / by reading the real result back, not "it's green now"), record the
  `[Test-Change: <box-id> — old-obsolete+new-correct, §ref]` rationale, and pass
  `G70` + the `G1` test-integrity check. This is "no green-by-rewrite" —
  [`docs/process/test-strategy.md`](docs/process/test-strategy.md) §8; it flags +
  requires justification, it does **not** forbid.)
- **Auto-generated `CLAUDE.md` / spec / security sections without review.**
- **Backwards-compat hacks for not-yet-existing code.**
- **A directory spec §0.7 does not home.** A new directory's §0.7 row lands in the commit
  that adds its first tracked file — gate-enforced by `G69` (tracked directories == §0.7
  directories, both ways; [`build-gates.md`](docs/security/build-gates.md) §6 check 26).
  Nothing structural lives outside §0.7.
- **A change to an authoritative source that leaves a referencing doc stale.** Any
  change to a source of truth — a **gate** (`Gnn`), a **control**, a **decision**, a
  **path**/directory, a **convention**, an **enum** variant, a **version pin** — is
  reflected in **every** doc that references it, in the **SAME commit**: no stale, no
  contradictory, no orphaned `.md`. This is **DoD item 2's general form** ("spec/docs
  synchronous in the same commit") extended to the whole doc graph, gate-enforced by
  `G68` (doc-graph integrity & freshness — orphan / cross-doc-resolution /
  described-the-old-way; the gates→`.md` case is one instance,
  [`build-gates.md`](docs/security/build-gates.md) §6 check 25).
- **An L(-1) security-critical-file edit by the autonomous Build-Loop, or any L(-1)
  edit lacking the `L-neg1-ack: owner` trailer.** The files that can silently weaken an
  enforcement plane (every file under `scripts/` except the declared Loop build tools, the
  hook and CI planes, the supply-chain, secret-scan, toolchain, hygiene and lint configs,
  the caged ratchets, the pinned engine manifest, the Tauri capabilities, the G53 fixture
  and the security/process docs — the authoritative list is `scripts/l-neg1-files.toml`)
  are the **L(-1) set** — the loop NEVER edits
  one (it parks the box; the owner-acked Co-Pilot makes the caged edit, build-loop Step 7);
  enforced by the pre-push gate
  **G71** (owner decision D1,
  [security-concept §2](docs/security/security-concept.md#2-working-model--two-sessions-one-branch),
  [`build-gates.md`](docs/security/build-gates.md) G71).
- `--no-verify`, force-push, `core.hooksPath` redirection, or disabling a required
  CI check — the complete forbidden-bypass set (security-concept §3).

## 6. The owner's core rule

**The cleanest, most complete, most professional solution for the work at hand always wins over
token cost, session speed and pragmatism:** production-grade, fully built, no stub, no shortcut, no
known defect left in the diff. **What bounds it is SCOPE, not quality.** The work at hand is the box's
referenced `§§` and the DoD (for a Co-Pilot act, the act's stated problem). An improvement outside that
scope — polishing neighbouring text, a new gate for a class with no security link and no recurrence,
rewording untouched prose — goes to the [residual ledger](docs/plan/residual-ledger.md) with its reason
and is triaged at the phase-end sweep; it is never silently dropped and never executed mid-box. The
entire gate layer — dual review, hooks, tests, spec-sync, plan-lint, the reliability/output-validity
gate — exists *precisely* so this priority holds. When two genuinely professional options exist for
the work at hand, decide strictly at this anchor, not reflexively by the cheaper one.

## 7. Tech-stack conventions

Authoritative detail is in [00-architecture](docs/spec/00-architecture.md) (§0.4
mechanics, **§0.8 pinned versions**, §0.10 CSP/capabilities) and
[06-build-test-release](docs/spec/06-build-test-release.md) — not duplicated here.
In brief:

- **Tauri v2**; Rust core, React 19 / TypeScript (**strict**) / Tailwind / Vite UI.
- **Rust ↔ TS type-sharing** via **tauri-specta + specta** — `bindings.ts` is
  generated, the single IPC door (no hand-written invoke glue, no `any`); §0.4.5
  drift check guards it.
- **pnpm** workspace (platform-pinned `pnpm`); **all tool versions are pinned and
  checksum-verified** (security-concept §0 pin-and-verify discipline) — a fill-pass
  must not reach for an un-pinned Marketplace action.
- **Bundled engines** (FFmpeg GPL, libvips, LibreOffice, poppler, pandoc, native
  Rust CSV) staged offline; per-engine hardening lives in each engine's spec §3.5.x.
- **Supply-chain / quality tooling:** `cargo clippy -D warnings` (+ no-panic policy
  on the in-core detect path), `cargo-deny`, `cargo audit`, `cargo vet`, `gitleaks`,
  Semgrep, `typos`, `actionlint`. The deterministic gate catalogue is
  [`docs/security/build-gates.md`](docs/security/build-gates.md) (G1..Gnn).

## 8. Language

- **Code, identifiers, comments and all docs: English** (public OSS repo).
- **Communication with the owner: German.**

## 9. References

- SSOT (what & why): [`docs/SINGLE-SOURCE-OF-TRUTH.md`](docs/SINGLE-SOURCE-OF-TRUTH.md)
- Spec (how): [`docs/spec/README.md`](docs/spec/README.md)
- Security concept (threat model + defense-in-depth): [`docs/security/security-concept.md`](docs/security/security-concept.md)
- Build-gate catalogue (G1..Gnn): [`docs/security/build-gates.md`](docs/security/build-gates.md)
- Build-loop (master prompt, DoD, hard-stops): [`docs/process/build-loop.md`](docs/process/build-loop.md)
- Test strategy: [`docs/process/test-strategy.md`](docs/process/test-strategy.md)
- Roles & escalation: [`docs/process/roles-and-escalation.md`](docs/process/roles-and-escalation.md)
- Gate-status ledger (owner-decidable gate posture decision-log, `plan-lint` check 23): [`docs/process/gate-status.md`](docs/process/gate-status.md)
- Vulnerability response (CVE → user, no-auto-update): [`docs/process/vuln-response.md`](docs/process/vuln-response.md)
  *(authored in P0.6.9 — see the §2 bootstrap note)*
- minisign key genesis & custody (the signing-key birth / backup / loss-recovery policy, L(-1)): [`docs/process/minisign-key-custody.md`](docs/process/minisign-key-custody.md)
  *(authored in P0.7.16)*
- Release-pipeline trust (tag-protection ruleset + commit/tag signing + the approval-gated `release` Environment + token scope, L(-1)): [`docs/process/release-pipeline-trust.md`](docs/process/release-pipeline-trust.md)
  *(authored in P0.7.17 / P0.7.18 — both provisioned + verified, boxes `[x]`)*
- P0-completion record (the durable L4-green P0-exit proof, `plan-lint` check 24): [`docs/process/p0-completion.md`](docs/process/p0-completion.md)
  *(stubbed in P0.6.10; the `run_url` is filled at P0 exit)*
- Plan (executable TODO): [`docs/plan/README.md`](docs/plan/README.md) ·
  [`docs/plan/P0-build-and-security.md`](docs/plan/P0-build-and-security.md)

---

## 10. Owner's own rules

<!-- The owner adds personal rules here. Claude does not touch this block without an explicit instruction. -->

- **Root-cause rule (owner, 2026-08-26; scoped by the owner).** It applies to a defect that already
  exists on `main` — a red CI run, a bug in committed code, an escalation, a sweep finding, or a review
  finding that shows the pattern in committed code. A finding confined to the diff under review is
  fixed at every instance in the diff and needs nothing more. For such a defect ask: *"can this — or a
  sibling of it — recur?"* If yes, the fix is not done until the **class** is closed **in the same
  commit**: sweep the siblings (grep the pattern across plan/spec/code/gates) and land the closure. A
  **new permanent catcher** (a gate leg, a lint, a self-test) is built only when the class guards a
  security control, has **recurred** (a second instance in git history), or turned `main` red while the
  local gates were green; otherwise the closure is a one-line repo-homed note
  ([Known gate traps](CONTRIBUTING.md#known-gate-traps) or the owning doc). Catcher ladder, lowest rung
  first: prevent at the source → a note → a regex or plan-lint leg → a parser leg (only for a bypass of
  a security control, in its own commit, with a written input model). A catcher beyond this trigger is
  a residual-ledger line (§6). A genuine one-off records **why** in the commit body. Enforced per
  commit by the G1 rubric's CLASS-CLOSURE item ([build-loop.md](docs/process/build-loop.md),
  drift-guarded by plan-lint check 19).
- **Decided = homed (owner, 2026-06-20: "there is no 'for later'", scoped by the owner).** Every point
  found is decided and homed now — a box with a `needs:` edge, a spec `[DECIDED]`, or a residual-ledger
  line with its reason — never an unowned "later". These stay **immediate**: a security red (a G17
  advisory, a red `main`), a gate that fires for the wrong reason, and any P0/P1 functional defect.
  Everything else outside the work at hand is a residual-ledger line, triaged at the phase-end sweep (§6).

<!-- End owner rules -->
