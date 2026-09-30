# CLAUDE.md — ConvertIA

> Persistent instructions for Claude Code in **this** repo. Short, specific,
> project-unique. Generic best-practices live in the system prompt, **not** here —
> this file only adds what Claude must know about *this* project. The org-wide
> rules live one level up in `../CLAUDE.md` (Ne-IA platform); this file does not
> repeat them.

**Platform rules.** The org-wide `../CLAUDE.md` applies where this file is silent; where they
differ, this file wins. Its web-project sections (the admin and auth patterns, domains and ports,
Cloudflare, Docker, nginx, env files, deployment, the platform script) do not apply here; its
shared stack applies only as far as spec §0.8 adopts it, and §8 below replaces its German-docs
rule. Its other global rules stay in force: the quality rules (no inline CSS among them), never
commit secrets, and never continue blindly after a context compaction — the Build-Loop re-reads
[`build-loop.md`](docs/process/build-loop.md) §2 before its next step and escalates a real
uncertainty (build-loop §4).

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

- **Build-Loop:** autonomous; builds the `P1`..`P11` boxes into `main`, one box per commit
  ([`build-loop.md`](docs/process/build-loop.md)); P0 was bootstrapped manually
  (roles-and-escalation §5). It decides routine choices itself, tagged, and escalates
  Build-Loop → Co-Pilot → owner only on the roles-and-escalation §4 triggers; a spec contradiction
  the §4(a) pre-check cannot rank is a scoped stop.
- **Co-Pilot:** works with the owner — escalations and rulings, caged changes under the owner's ack
  (build-loop Step 7), the phase-end sweeps
  ([test-strategy §11](docs/process/test-strategy.md#11-the-phase-end-co-pilot-hardening-sweep))
  and incoming PRs and Dependabot bumps (roles-and-escalation §5a).
- Each session commits from its own clone; `main` is the only branch, with no merge step.
  Enforcement is the deterministic gates plus a green CI on `main` (required status checks on every
  push). The G1 dual review amplifies quality; the deterministic gates (every `Gnn` except G1) are
  the only security controls ([`roles-and-escalation.md`](docs/process/roles-and-escalation.md) §1).

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

The 8-point DoD (a)–(h) is canonical in [`build-loop.md`](docs/process/build-loop.md) §5;
`plan-lint` check 14 keeps the G1 and P0.6.5 copies letter-identical.

## 5. Anti-patterns (NEVER)

One line each, with the gate that enforces it:

- `any` (`: any` / `as any`) in TypeScript, or an untyped IPC boundary (the generated
  `bindings.ts` is the only IPC door, §0.4.5) → G5, G6, G8, G19.
- `TODO` / `FIXME` / `unimplemented!` / `todo!` / `dbg!` / `console.log` / `println!` in production
  code, and the semantic-deferral vocabulary ("for now", "later", "not yet", "comes in P<n>") that
  escapes a marker-only scan → G8/G21.
- `unreachable!` in production code — the exhaustive-match `clippy` deny on the dispatch enums makes
  it unneeded; allowed only in an unreachable-by-construction `#[cfg(test)]` branch with a comment
  → G4/G14, G8.
- Any network call outside the one user-initiated About → Releases link — no runtime engine fetch,
  no update check, no telemetry; a security fix reaches users only as a new full release
  ([`vuln-response.md`](docs/process/vuln-response.md)) → G29 (g), G42/G42b.
- A dependency that opens a network surface — `tauri-plugin-http` or any
  `reqwest`/`ureq`/`hyper`/`isahc`/`curl`-class crate in `Cargo.toml` → G18 (`cargo-deny [bans]`).
- GPL/AGPL/LGPL linked into the MIT core — copyleft ships as a separately invoked binary → G18
  (`[licenses]`), G53.
- A skeleton or stub as a default — a stub is only a named, compile-time interface shell that a
  named, scheduled box fills, never a quiet placeholder (SSOT Principle 1) → G8, G1.
- Rewriting, relaxing, skipping or deleting a failing test to make it pass without proving the old
  assertion obsolete and the new one correct (a verified change carries
  `[Test-Change: <box-id> — old-obsolete+new-correct, §ref]`) → G70, G1 item 5,
  [`test-strategy.md`](docs/process/test-strategy.md) §8.
- Auto-generated `CLAUDE.md` / spec / security sections without review → G1.
- Backwards-compat hacks for not-yet-existing code → G1.
- A directory spec §0.7 does not home — its §0.7 row lands in the commit that adds its first
  tracked file → G69 ([`build-gates.md`](docs/security/build-gates.md) §6 check 26).
- A change to a source of truth — a gate, a control, a decision, a path, a convention, an enum
  variant, a version pin — that leaves a referencing doc stale, contradictory or orphaned; every doc
  that references it moves in the same commit → G68, DoD (b).
- An L(-1) edit by the Build-Loop, or any L(-1) edit without the `L-neg1-ack: owner` trailer. The
  files that can silently weaken an enforcement plane (every file under `scripts/` except the
  declared Loop build tools, the hook and CI planes, the supply-chain, secret-scan, toolchain,
  hygiene and lint configs, the caged ratchets, the pinned engine manifest, the Tauri capabilities,
  the G53 fixture and the security/process docs — the authoritative list is
  `scripts/l-neg1-files.toml`) are the L(-1) set, and lowering or removing a `[[monotone]]`
  ratchet row (the §0.8 floors in `Cargo.toml` / `package.json`) needs the same trailer; the Loop
  parks a box with a caged part (build-loop Step 7) → G71 (owner decision D1,
  [security-concept §2](docs/security/security-concept.md#2-working-model--two-sessions-one-branch)).
- `--no-verify`, force-push, `core.hooksPath` redirection, or disabling a required CI check — the
  complete forbidden-bypass set (security-concept §3) → G54, G56a.

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

- **Tauri v2** — a Rust core and a React 19 / TypeScript (**strict**) / Tailwind / Vite UI; the
  tauri-specta-generated `bindings.ts` is the only IPC door (§0.4.5, G19); a pnpm workspace.
- Every tool and engine version is pinned and checksum-verified (security-concept §4 principles 5
  and 10); never an un-pinned Marketplace action.
- The pins and the detail: [00-architecture](docs/spec/00-architecture.md) §0.8 (versions) and
  §0.10 (CSP, capabilities), [06-build-test-release](docs/spec/06-build-test-release.md), and the
  gate catalogue [`build-gates.md`](docs/security/build-gates.md).

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
- minisign key genesis & custody (the signing-key birth / backup / loss-recovery policy, L(-1)): [`docs/process/minisign-key-custody.md`](docs/process/minisign-key-custody.md)
- Release-pipeline trust (tag-protection ruleset + commit/tag signing + the approval-gated `release` Environment + token scope, L(-1)): [`docs/process/release-pipeline-trust.md`](docs/process/release-pipeline-trust.md)
- P0-completion record (the durable L4-green P0-exit proof, `plan-lint` check 24): [`docs/process/p0-completion.md`](docs/process/p0-completion.md)
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
