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

## 1a. Repo layout (the operational per-dir map)

> **This is the operational map of where everything lives — the flat dir-set `G69`
> mechanically checks bidirectionally against the on-disk tree** (every repo directory
> appears here ∧ every mapped dir exists; [`build-gates.md`](docs/security/build-gates.md)
> §6 check 26). **It is NOT the single source of truth — the higher source is spec
> [§0.7](docs/spec/00-architecture.md) "Physical tree"** (the logical-module
> decomposition + its rationale), which outranks this docs-layer map per the repo
> conflict rule **SSOT > spec > docs**. This §1a map is a **faithful projection of the
> §0.7 physical tree** onto the flat dir-set G69 asserts; a `G68/G69` sub-check binds
> the two so they cannot drift (the §0.7-derived dir set ⊇ this map's dir set;
> [`build-gates.md`](docs/security/build-gates.md) §6 checks 25/26). When the two
> disagree, **§0.7 wins** — fix §1a to match, never the reverse.
> **Standing rule (anti-pattern below):** never create a structural element (a folder)
> that is not in **both** §0.7 and this map — if a new one is genuinely needed for
> clean logical separation, **update §0.7 AND this map in the SAME commit**
> (gate-enforced).
>
> **Authored at P1.64** (the P1-end structure box, `needs:` the P1 scaffold boxes): every
> directory the product needs across P1–P11 now exists with exactly one row below, so no
> later phase invents an unmapped folder. This flipped `G69` from skip-with-warning to
> **fail-closed** — the bidirectional map ↔ on-disk-tree bijection (every git-tracked dir
> is mapped ∧ every mapped dir is tracked) + the §1a ⊆ §0.7 projection bind now enforce
> ([`build-gates.md`](docs/security/build-gates.md) §6 check 26). A new directory therefore
> requires a row here **and** in §0.7 in the same commit (the standing rule above).
> **Deliberately absent** (each created by its owning phase, which adds its §0.7 + §1a rows
> then): `bundle/` — engines stage into `src-tauri/binaries/` + `src-tauri/resources/` per
> §6.1.3, not a separate staging tree; `fuzz/` — the P3-owned `cargo-fuzz` tree, created
> **with** its G48 harness (pre-creating its empty `corpus/`/`crashes/` would prematurely
> activate the `check-fuzz-contract` live tier, which then requires the not-yet-built harness).

```
convertia/                          → repo root (Git, GitHub: Ne-IA/convertia)
├── CLAUDE.md  README.md  LICENSE   → repo rules + this map · download/trust page · MIT + collective copyright
├── Cargo.toml  Cargo.lock          → the Rust workspace root (virtual manifest + committed lockfile, P1.6)
├── index.html  package.json  tsconfig.json   → the frontend entry + npm manifest + TS config (+ vite/vitest/eslint config, .gitignore, .npmrc, …)
├── .github/                        → CI + supply-chain automation (SHA-pinned actions; L(-1))
│   ├── ISSUE_TEMPLATE/             → issue templates
│   └── workflows/                  → Lane-A `ci.yml` + `release.yml` / `scorecard.yml` / `secrets-history.yml`
├── assets/                         → static brand assets
│   └── branding/                   → logo / brand source art
├── crates/                         → non-core first-party Rust workspace members
│   └── imgworker/                  → pkg `convertia-imgworker` — the isolated libvips/libheif/librsvg image-worker (§3.5.5, G53)
│       └── src/                    → `ffi.rs` (the one allow-listed unsafe FFI surface, G29) + `main.rs`
├── design/                         → UI / design assets (a sibling of `src/`, §0.7)
├── docs/                           → all documentation (the doc graph G68 guards)
│   ├── plan/                       → P0..P11 + README index + `_format.md`
│   ├── process/                    → build-loop / test-strategy / roles-and-escalation / vuln-response / gate-status / p0-completion / minisign-key-custody / release-pipeline-trust
│   ├── security/                   → `security-concept.md` + `build-gates.md` (G1..Gnn)
│   └── spec/                       → the spec (how) — `00-architecture` … `07-app-shell`
│       └── 04-formats/             → the per-category §04 format matrices
├── scripts/                        → the stdlib gate scripts + pinned-tool installer + bundle/SBOM/checksum (§06, P0.2; L(-1))
│   ├── gate-selftests/             → the G24 `g24-*`/`g54-*` planted-positive self-tests (the G10 canary)
│   │   ├── gitleaks-fixtures/      → G2 gitleaks self-test fixtures
│   │   └── typos-fixtures/         → G51 typos self-test fixtures
│   └── semgrep-rules/              → the G29 SAST rule corpus (L(-1))
│       ├── fixtures/               → planted-positive SAST fixtures
│       │   └── engines/            → engine-arg SAST fixtures
│       ├── project/                → first-party Semgrep rules
│       └── vendor/                 → vendored Semgrep rule packs
├── src/                            → the React 19 / TS / Tailwind / Vite UI (§05)
│   ├── a11y/                       → the §5.6 a11y helpers (announcer / keymap) + the G33a leg
│   ├── components/                 → presentational components (§5.x)
│   ├── hooks/                      → React hooks (§5.x)
│   ├── lib/                        → frontend library code
│   │   └── ipc/                    → the generated `bindings.ts` (the only IPC door, §0.4.5) + the commands/events wrappers
│   ├── state/                      → the zustand store (§5.x)
│   ├── strings/                    → the §5.7 English-only `ui.ts` string catalog (G57)
│   └── styles/                     → Tailwind / app CSS
├── src-tauri/                      → the MIT core crate `convertia-core` (the Tauri host binary)
│   ├── binaries/                   → bundled engine sidecars per platform (externalBin, §3.3)
│   ├── capabilities/               → the §0.10 capability allowlist (`main.json`, G47; L(-1))
│   ├── icons/                      → the bundled app-icon set (§3.3, P1.19.2)
│   ├── resources/                  → bundled non-exe engine assets (LibreOffice profile, fonts, codec libs)
│   └── src/                        → the core crate source (`main.rs` + the §0.7 tier modules):
│       ├── detection/              → tier 2 — §1.2 content sniffing (first to touch untrusted bytes)
│       ├── domain/                 → tier 3 — the §0.6 types (the identity spine)
│       ├── engines/                → tier 2 — the §3.2 engine registry/seam + §1.7 invocation + §3.5 args
│       ├── fs_guard/               → tier 2 — the §2.0 no-harm kernel (atomic write / no-clobber / resolved-id)
│       ├── ipc/                    → tier 0 — the §0.4 command/event handlers (the WebView's only door)
│       ├── isolation/              → tier 2 — the §2.12 decoder-isolation wrapper (the sole `Command::new`)
│       ├── orchestrator/           → tier 1 — queue + job lifecycle (§1.9) + run registry + cancellation
│       ├── outcome/                → tier 2 — the §2.8 taxonomy + message catalog → §0.4.3 IpcError
│       ├── platform/               → tier 3 — path / volume / OS shims (§2.14, §7.7)
│       ├── pool/                   → tier 3 — the §0.9 subprocess pool + concurrency degree
│       └── run/                    → tier 2 — per-run/instance scratch ownership + cleanup (§2.4/§2.6)
├── supply-chain/                   → the `cargo-vet` audit trust store (`config.toml` + `audits.toml`; `imports.lock` joins when the live cargo-vet run lands, G18b; L(-1))
├── tests/                          → Rust integration + corpus harness (§6.4)
│   ├── corpus/                     → the §6.5 reliability corpus (manifest + fixtures, fills P3–P7)
│   └── g53-fixture/                → the G53 NEGATIVE fixture (a planted copyleft-into-core violation; own nested workspace, excluded from the real graph)
│       ├── convertia-core/         → the fixture's fake core crate
│       │   └── src/                → its source
│       └── libvips-sys/            → the fixture's planted forbidden lib
│           └── src/                → its source
└── xtask/                          → the `cargo xtask` dev-bin (§0.4.5 codegen + §6.7.1 coverage, G19)
    └── src/                        → the xtask source
```

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
- **A structural element (a folder) not in the §1a "Repo layout" map (a projection of
  the higher spec §0.7 physical tree).** Never create a directory that is absent from
  the map; if a new one is genuinely needed for clean logical separation, **update spec
  §0.7 AND the §1a map in the SAME commit** (§0.7 is the higher source per SSOT > spec >
  docs; §1a is its operational projection) — gate-enforced by `G69` (the bidirectional
  CLAUDE.md-map ↔ on-disk-tree assertion + the §1a ⊆ §0.7 projection bind,
  [`build-gates.md`](docs/security/build-gates.md) §6 check 26). Nothing structural
  lives outside the map, and the map never invents a dir §0.7 does not home.
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
  enforcement plane (gate scripts, `lefthook.yml`, `.github/**`, `deny.toml`,
  `.gitleaks.toml`, `.npmrc`, `.editorconfig`, `.typos.toml`, the cargo-vet exemption set, `engines.lock`, the Tauri capabilities,
  the reviewer rubric, the security/process docs, and `scripts/l-neg1-files.toml` itself)
  are the **L(-1) set** (non-exhaustive; the authoritative list is
  `scripts/l-neg1-files.toml`, enumerated in security-concept §2) — the loop NEVER edits
  one (hard-stop + escalate so the **owner** makes/acks it); enforced by the pre-push gate
  **G71** (owner decision D1,
  [security-concept §2](docs/security/security-concept.md#2-working-model--two-sessions-one-branch),
  [`build-gates.md`](docs/security/build-gates.md) G71).
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

- _…_

<!-- End owner rules -->
