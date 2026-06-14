# CLAUDE.md — ConvertIA

> Persistent instructions for Claude Code in **this** repo. Short, specific,
> project-unique. Generic best-practices live in the system prompt, **not** here —
> this file only adds what Claude must know about *this* project. The org-wide
> rules live one level up in `../CLAUDE.md` (Ne-IA platform); this file does not
> repeat them.

**Conflict rule:** **SSOT > spec > security/process docs > plan > code > conversation.**
When two layers disagree, the higher one wins — **never silently reconcile, always
escalate**. SSOT ([`docs/SINGLE-SOURCE-OF-TRUTH.md`](docs/SINGLE-SOURCE-OF-TRUTH.md))
is the tech-free *what & why*; the [spec](docs/spec/README.md) is the *how* derived
from it; the [security/process](docs/security/security-concept.md) docs are the
*how we build it safely*; the [plan](docs/plan/README.md) is the executable TODO.
A spec-internal contradiction (two `§§` disagree) is an **unconditional
hard-stop + escalate** — the build is downstream of the spec and cannot pick a side.

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

## 2. Working model — two sessions, one branch

| Session | Role |
|---|---|
| **Build-Loop** | Autonomous. Builds the plan box by box (**P1 onward**), writes tests, runs every gate + the dual review, commits directly to `main`. The gates are the protection — no second branch, no merge step. |
| **Co-Pilot** | Escalation & clarification target for the Build-Loop; strategic decisions; high-level review. Works with the owner. |

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
  established pattern first); it escalates on genuine blocks and on any
  spec-internal contradiction.
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
5. **The Opus + Sonnet pre-commit dual review (G1) is through** — both reviewers'
   findings + convergence/divergence recorded verbatim in the commit body, trailer
   `Dual-Review: opus=… sonnet=…` present. **P0/P1 findings are fixed in the
   working tree, re-staged and re-reviewed before push** (no fix-push cycle);
   P2/P3 noted in the body.
6. **Inline decision tags set** at every non-spec choice site —
   `[Build-Session-Entscheidung: <box-id>]` for self-made pattern/naming/default
   choices, directly at the code site, not only in the commit body.
7. **`engines.lock` + SBOM row** added if a new engine was staged.
8. **`§0.11` threat-map + security-concept `§5` row** added if a new threat class
   was introduced. (Items 7 and 8 fire **independently** — either alone requires its
   action.)

(This is RMACLAUDE §11's nine points with the RLS/tenant, immutable-audit-row and
migrations rows **dropped** — no multi-tenant DB in an offline desktop app — and the
two ConvertIA-specific facts (`engines.lock`/SBOM row, threat-class row) **added**:
9 − 3 + 2 = 8. Output-validity is not a ninth item — it lives inside item 3's
highest-sensible-test bar, per P0.6's canonical list.)

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
  the runbook is [`docs/process/vuln-response.md`](docs/process/vuln-response.md).)
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
- **Auto-generated `CLAUDE.md` / spec / security sections without review.**
- **Backwards-compat hacks for not-yet-existing code.**
- `--no-verify`, force-push, `core.hooksPath` redirection, or disabling a required
  CI check — the complete forbidden-bypass set (security-concept §3).

## 6. The owner's core rule

**The cleanest / most-complete / most-professional solution ALWAYS wins over
token-cost, session speed, and "pragmatism."** System completeness beats a local
shortcut; spec-mandated work beats a pragma-sized box; tech debt is paid now, not
accumulated. The entire gate layer — dual review, hooks, tests, spec-sync,
plan-lint, the reliability/output-validity gate — exists *precisely* so this
priority holds; ranking pragmatism above it would undercut the whole protection
layer. When two genuinely professional options exist, decide strictly at this anchor,
not reflexively by the cheaper one.

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
- Vulnerability response (CVE → user, no-auto-update): [`docs/process/vuln-response.md`](docs/process/vuln-response.md)
- Plan (executable TODO): [`docs/plan/README.md`](docs/plan/README.md) ·
  [`docs/plan/P0-build-and-security.md`](docs/plan/P0-build-and-security.md)

---

## 10. Owner's own rules

<!-- The owner adds personal rules here. Claude does not touch this block without an explicit instruction. -->

- _…_

<!-- End owner rules -->
