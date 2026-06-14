# ConvertIA — Build-Gate Catalogue (living)

> The operational catalogue of every guardrail: **when** it runs, **what** it
> blocks, **which tool** enforces it, and its **fail posture**. Companion to the
> [security-concept.md](security-concept.md) (the *why*). Living document — same
> conflict order and update rules as the concept doc.
>
> Gate IDs are stable (`Gnn`). The plane column maps to the
> [defense-in-depth planes](security-concept.md#3-defense-in-depth--the-enforcement-planes)
> L0–L5.

## 0. Policy

- **Two planes.** Each gate marked *(mirror)* runs both as a local git-hook
  (L1/L2/L3) **and** in CI (L4). Local = realtime; CI = clean-checkout backstop.
- **Performance budgets.** pre-commit (L1) target **< 10 s**; pre-push (L2) target
  **< 3 min**; anything heavier is **CI-only (L4)** or **release-only (L5)**.
- **Hooks run in parallel** within a plane; gates sharing a resource (e.g. one
  test DB-equivalent / one build dir) are merged into a single sequential wrapper.
- **Fail-closed by default.** A gate blocks on doubt. Only the explicitly listed
  gates are **fail-open** (skip when a prerequisite is genuinely absent), and only
  because another plane guarantees enforcement.
- **No bypass.** `--no-verify`, force-push, and disabling a required CI check are
  forbidden (CLAUDE.md). A red gate is fixed, not bypassed.
- **Severity ≠ exit code** for custom gate scripts: *any* finding fails the gate;
  severity only drives output formatting.
- **Custom gate scripts are themselves tested** (positive + negative cases) under
  a narrowly-scoped self-test gate (G24).
- **Offline tolerance.** Any gate with a network step (advisory-DB refresh, rule
  fetch) **decouples** the refresh (warn-only) from the check (hard-fail against
  the local/vendored DB), and honours an offline env flag.

## 1. L0 — Build-Loop per box (the dual review, "holy grail")

**G1 — Opus + Sonnet pre-commit dual review.** *Plane L0.* Before each build
commit, two model reviewers (`opus` + `sonnet`) critique the **staged diff**
(`git diff --cached`, inline — not a SHA) for completeness, correctness, and
spec-conformance, plus the relevant spec `§§` + this catalogue.
- Output per reviewer: findings ranked **P0→P3**, each with a one-line reason +
  spec-`§`/file ref; convergence/divergence stated explicitly.
- **P0/P1 → fix in the working tree, re-stage, re-review** (loop). **No push
  between fix and re-review** (no fix-push cycle). **P2/P3 →** documented in the
  commit body + a follow-up box if structural.
- Recorded as a machine-readable commit trailer (`Dual-Review: opus=GO/NOGO
  sonnet=GO/NOGO`) — the *evidence of review* is mandatory.
- **Skipped only** for: (a) check-off commits with no code/config diff; (b)
  `[!extern]` boxes (nothing built).
- *Blocks:* the build commit (self-gate; the trailer is also checked by G2-class
  format rules).

## 2. L1 — pre-commit (cheap, < 10 s, parallel)

| ID | Gate | Tool / mechanism | Blocks | Scope / fail-mode |
|---|---|---|---|---|
| **G2** | Secrets / credential scan *(mirror)* | regex for PEM private keys, cloud secret keys, generic long `api_key`/token literals (e.g. `gitleaks`/`trufflehog` or a vetted regex) | commit | always-on (no glob); fail-closed |
| **G3** | Format check *(mirror)* | `cargo fmt --check` + `prettier`/`biome --check` (check-only, no auto-write) | commit | by file glob; fail-closed |
| **G4** | Lint — Rust *(mirror)* | `cargo clippy -D warnings` | commit | changed-crate scope; fail-closed |
| **G5** | Lint — TS/React *(mirror)* | `eslint` (flat config) + `stylelint` (CSS) | commit | by glob; fail-closed |
| **G6** | Type-check (fast) *(mirror full at L2)* | `tsc --noEmit` on changed scope; (`cargo check` is implicit in clippy) | commit | by glob; fail-closed |
| **G7** | Doc-consistency `plan-lint`/`spec-lint` *(mirror)* | custom stdlib-only script (see §6) `--quiet` | commit | on docs glob; any finding = exit 1 |
| **G8** | Deferral / dead-marker gate *(mirror)* | diff-based scan for `TODO`/`FIXME`/`unimplemented!`/`todo!`/`unreachable!`/`dbg!`/`println!`/`console.log`/"stub"/"placeholder"/"phase 2" in **new** production lines lacking a box-id or `[!extern]` within ±6 lines | commit | new-marker only; **fail-open** if no diff base |
| **G9** | Repo-invariant grep gate *(mirror)* | cheap repo-wide regex for a project invariant (e.g. no hardcoded colours outside token primitives), path-whitelisted | commit | by glob; fail-closed |
| **G10** | Fastpath self-tests | `test-*-fastpath-pattern` smoke tests | commit | only when a fastpath detector is edited |

## 3. L3 — commit-msg

| ID | Gate | Tool / mechanism | Blocks | Scope / fail-mode |
|---|---|---|---|---|
| **G11** | Conventional-commit format | regex `^(feat\|fix\|chore\|docs\|refactor\|test\|perf\|ci\|build)(\(scope\))?: .+` (first line); merge/revert/fixup exempt | commit | always; fail-closed |
| **G12** | Dual-review trailer present | the build commit body carries the `Dual-Review:` trailer (skipped for check-off/`[!extern]`) | commit | conditional; fail-closed |

## 4. L2 — pre-push (heavier, < 3 min, parallel; cheap-commit fastpath)

| ID | Gate | Tool / mechanism | Blocks | Scope / fail-mode |
|---|---|---|---|---|
| **G13** | Full type-check *(mirror)* | `tsc --noEmit` whole project | push | always; fail-closed |
| **G14** | Full lint *(mirror)* | `clippy --all-targets --all-features -D warnings` + `eslint` whole tree | push | always; fail-closed |
| **G15** | Unit + integration tests *(mirror)* | `cargo test` (incl. real-file round-trips) + `vitest run` | push | always; fail-closed |
| **G16** | Property + fuzz smoke *(mirror)* | `proptest`/`fast-check` suites; `cargo fuzz` short smoke on the decode path | push | always; full fuzz at L4 |
| **G17** | Dependency-vuln audit *(mirror)* | `cargo audit` + `pnpm audit --audit-level=high` (DB refresh warn-only/offline-tolerant) | push | always; check fail-closed, refresh fail-open |
| **G18** | License + supply-chain policy *(mirror)* | `cargo deny check` (advisories + **licenses** + bans + sources) | push | always; fail-closed |
| **G19** | Generated-artifact drift *(mirror)* | regenerate Tauri→TS bindings / CLI `--help` / asset manifest, then `git diff --exit-code`; + structural (parsed, not regex) non-empty sanity | push | by glob; fail-closed |
| **G20** | `plan-lint`/`spec-lint` full *(mirror)* | the G7 script, verbose, all checks | push | always; fail-closed |
| **G21** | Deferral gate full *(mirror)* | the G8 scan vs `origin/main` | push | new-marker; fail-open w/o base |
| **G22** | Schema/membership parity *(mirror)* | "every supported format ∈ README matrix ∧ has a fixture ∧ has a round-trip test"; locale-file key parity (if i18n) | push | by glob; fail-closed |
| **G23** | "every X has a Y" completeness *(mirror)* | e.g. every `convert_*` command has a test (via `git ls-files`) — caveat: tracking-aware, stage partner file together | push | by glob; fail-closed |
| **G24** | Gate-script self-tests *(mirror)* | run the custom-gate unit tests | push | when a gate script changed; fail-closed |

**Fastpath / skip (L2 only).** Expensive hooks (G15/G16/G17/G18 + heavy lint) are
skipped **only** when provably irrelevant, via detectors that each default to
*run* on ambiguity:
- **Docs-only push** — net range-diff is markdown-only ⇒ no Rust/TS/lockfile to
  scan ⇒ safe to skip the byte-scanning gates. (Hard safety guard = the md-only
  diff, not the subject.)
- **Check-off fastpath** — `chore(todo): … abgehakt`-style subject **AND** a
  markdown-only diff.
- Detector fallback chain: `@{u}` → `origin/<branch>` → `origin/main` →
  `origin/HEAD`; **no base / 0 unpushed commits ⇒ run the full gate.** Skipping is
  opt-in; anything ambiguous runs everything.
- Cheap structural gates (G13/G20/G21/...) and glob-gated gates always run when
  their glob matches — they have nothing expensive to skip.

## 5. L4 — CI (GitHub Actions, post-push) & L5 — Release (`v*` tag)

### L4 — CI (clean checkout; mirrors L1–L2 + the heavy gates)
| ID | Gate | Tool / mechanism | Blocks |
|---|---|---|---|
| **G25** | All L1–L2 gates re-run on clean checkout | the same hooks/scripts in CI | red `main` |
| **G26** | Full fuzz pass | `cargo fuzz` longer budget on decode path | red `main` |
| **G27** | Coverage — global floor | `cargo-llvm-cov` + vitest v8; branch coverage; ratchet **50 % → 70 %** | red `main` |
| **G28** | Coverage — diff gate | **≥ 80 %** on changed lines (change-only) so new code can't dilute the floor | red `main` |
| **G29** | SAST / static security | Semgrep (Rust/TS packs) + `cargo geiger` (unsafe census) | red `main` |
| **G30** | Cross-platform build matrix | native build on Windows / macOS / Linux runners (no cross-compile); macOS universal `lipo` | red `main` |
| **G31** | Per-pair corpus + reliability | real-file round-trips per `(source→target)` pair per platform; output validated (codec/container/header), not "file exists"; reliability threshold | red `main` |
| **G32** | Round-trip invariant | A→B→A byte-stable (lossless) / within tolerance (lossy) as a CI gate | red `main` |
| **G33** | a11y | axe WCAG 2.1 AA — critical+serious = 0 blocks; moderate/minor log-only | red `main` (release-tier) |
| **G34** | Visual regression | screenshot diff, fixed tolerance; baseline update only on intentional change | red `main` (release-tier) |

### L5 — Release (tag-triggered; release-blocking)
| ID | Gate | Tool / mechanism | Blocks |
|---|---|---|---|
| **G35** | SBOM generation + completeness | CycloneDX (`cargo cyclonedx` + `cyclonedx-npm`); every staged binary mapped; no `UNKNOWN`/`NOASSERTION` | release |
| **G36** | License hard-fail | SBOM scanned for forbidden families (GPL/AGPL in MIT core / static binary) → exit 1 | release |
| **G37** | Engine checksum / integrity build gate | pinned-version + SHA-256 verify of every bundled engine; build-time hash manifest generated | release |
| **G38** | Per-engine build assertions | FFmpeg `-protocols`/`-demuxers` curated + required-codec lock; librsvg ≥2.56.3 + no-base-URL API; libvips `effort` params; LGPL shared-object-or-fail; libimagequant BSD pin | release |
| **G39** | Checksums + minisign | per-file SHA-256 + signed `SHA256SUMS` (pubkey at `docs/minisign.pub`, rotation logged) | release |
| **G40** | Signing + provenance | release-binary signing (cosign/minisign) + GitHub SLSA build-provenance attestation. **Provision the key AND wire the step.** | release |
| **G41** | Artifact size budget | per-platform compressed artifact ≤ budget (≤400 MB target) — measured after bundle | release |
| **G42** | Offline-egress observability | packet monitor (tcpdump/Procmon/strace) during a representative corpus run + at startup → zero egress (except user-initiated releases-page) | release |
| **G43** | No-system-pollution audit | syscall/fs monitor during a conversion → no registry/LaunchAgent/daemon/file-assoc writes; writes only to config+log+chosen-output+scratch | release |
| **G44** | Governance completeness | every required governance doc present + non-stub; download/trust page complete (verify recipe, WebView2/FUSE/Sequoia notes) | release |
| **G45** | Name/trademark clearance record | `docs/name-clearance.md` present, dated for the release line, verdict = clear; dormant rename-propagation + old-name grep gate | release |
| **G46** | Startup integrity (runtime, also a release acceptance) | engine presence + integrity verification; missing/corrupt engine → app-fault, not crash | release acceptance |

## 6. The `plan-lint` / `spec-lint` doc-consistency gate (G7/G20)

A single **stdlib-only** script (no third-party deps → runs anywhere instantly),
treating our canonical docs as machine-checkable truth. Exit codes: `0` none,
`1` ≥1 finding (**any** severity), `2` target missing. CLI: `--check <ids>`,
`--json`, `--quiet`, `--max-per-check N`. Three call sites: L1 (`--quiet` on glob),
L2 (full), L4.

Invariant checks (initial set; expanded during P0 review):
1. **Membership / matrix parity** — every format named in prose ∈ the README
   support matrix, and every matrix row has a code registry entry + a fixture + a
   round-trip test (ties G22).
2. **Cross-reference validity** — every `§X.Y` / internal anchor resolves.
3. **Heading hierarchy** — no skipped levels.
4. **Numbering gap-freeness** — sub-sections run min→max, no gaps.
5. **Gate-catalogue integrity** — every gate named in security-concept.md exists
   here as a `Gnn` row, and vice-versa.
6. **No forbidden tokens** — banned stamps/strikethrough/stale-dates in doc bodies.
7. **Generated-file structural sanity** — when validating a generated file, parse
   it (not regex) and assert non-empty/well-formed.

## 7. Open / to-reconcile (closed during P0 review)

- [ ] Pick concrete tools where the spec leaves them `[DEFER]` (secrets-scanner,
  CSP-lint, SAST pack set, fuzz harness layout, observability tooling per OS).
- [ ] Confirm the gate↔threat mapping in [security-concept.md](security-concept.md#5-threat-model--control--gate)
  has a verifying gate for **every** spec §0.11 threat class.
- [ ] Decide which L4 gates are *required checks* vs informational on day one
  (ratchet plan), so a half-built P1 isn't wedged.
- [ ] Confirm no gate is missing a tool, and hunt for gates we haven't thought of
  (adversarial review mandate).
