# P0 — Build & Security System

> **The foundation before the foundation.** P0 establishes the guardrail system
> every later phase (P1–P11) runs under: the security concept, the full gate
> system, the build-loop, the dual review, and the test methodology. It is built
> **before** P1 writes any app code, so P1's very first commit already runs through
> the loop + the gates.
>
> Derives from [security-concept.md](../security/security-concept.md) +
> [build-gates.md](../security/build-gates.md). Index: [plan/README.md](README.md).
>
> **This file currently holds the cluster structure + box headings only** — the
> atomic `[ ]` steps are added in the fill pass, after this structure is reviewed
> and signed off. No `[ ]` boxes are filled yet.

## Boundaries (decided, see plan/README.md)

- **P0 ↔ P1:** P0 builds the gate *system* + content-independent gates + the
  framework/wiring-points for language gates; **P1** wires the language-specific
  gates (clippy/tsc/cargo-deny/…) into the P0 framework as it scaffolds, and
  authors the general governance docs.
- **P0 ↔ P10:** P0 *defines* the release-/supply-chain-gate policy; **P10** builds
  the release pipeline. No double-build.
- **Content-independence rule:** a P0 box may only build what can exist without app
  code. Gates that need Rust/TS to act on are *defined + wired-pointed* here and
  *activated* by the producing phase (annotate `→ activated in P1`).

---

## P0.1 — Security concept & process docs

**Goal:** the living docs that govern how we build exist and are internally
consistent.

**Box areas (headings; steps in fill pass):**
- Security concept doc finalized ([security-concept.md](../security/security-concept.md)) — threat→control→gate mapping complete vs spec §0.11.
- Gate catalogue finalized ([build-gates.md](../security/build-gates.md)).
- Build-loop master prompt authored (`docs/process/build-loop.md`) — single-branch, 2-session, no quick-fixes.
- Test strategy authored (`docs/process/test-strategy.md`).
- Roles & escalation authored (`docs/process/roles-and-escalation.md`) — Build-Loop ↔ Co-Pilot ↔ owner.
- Box-format spec authored (`docs/plan/_format.md`) — `[ ]/[x]/[!]/[!extern]`, tags, sub-boxes, per-phase-file + index convention.

**Home:** docs/security/, docs/process/, docs/plan/.

---

## P0.2 — Gate orchestration framework

**Goal:** the two enforcement planes exist as empty-but-wired harnesses that later
gates plug into.

**Box areas:**
- Git-hook manager set up (pre-commit / pre-push / commit-msg), `parallel`, perf budgets.
- GitHub Actions CI skeleton (L4) — clean-checkout job matrix placeholders (Win/macOS/Linux).
- Release workflow skeleton (L5) — tag-triggered, empty gate slots.
- Defense-in-depth plane definitions + fail-open/closed policy wired as config.
- Fastpath / skip detectors (docs-only, check-off) + their self-tests (G10/G24).

**Home:** build-gates.md §0–§4 · 06-build-test-release.

---

## P0.3 — Content-independent gates (buildable now)

**Goal:** every gate that needs no app code is live on both planes.

**Box areas:**
- Secrets / credential scan (G2).
- Conventional-commit + dual-review-trailer (G11/G12).
- Deferral / dead-marker gate (G8/G21).
- `plan-lint` / `spec-lint` doc-consistency linter (G7/G20) — incl. its own unit tests.
- License + supply-chain policy config (`cargo-deny` skeleton; G18) + SBOM forbidden-family gate definition (G36).
- Generated-artifact drift-check framework (G19).
- Repo-invariant grep gate scaffold (G9).

**Home:** build-gates.md §2/§4/§6.

---

## P0.4 — Language & build gate contracts

**Goal:** the contract + CI wiring-points for the language gates are defined; full
activation happens when P1 scaffolds the toolchains.

**Box areas:**
- Rust gate contracts: rustfmt, `clippy -D warnings`, `cargo test`, `cargo audit`, `cargo deny`, `cargo geiger`, `cargo fuzz` (decode-path harness layout). `→ activated in P1`.
- TS gate contracts: `tsc` strict, eslint, prettier, vitest. `→ activated in P1`.
- Coverage gates: global floor (ratchet 50→70, branch) + diff gate ≥80% (G27/G28).
- Cross-platform build-matrix contract (G30) — native per-OS, macOS universal.

**Home:** build-gates.md §4/§5 · 06-build-test-release · 00-architecture (§0.4.5 type-drift).

---

## P0.5 — Test methodology & harness conventions

**Goal:** *how we write tests* is defined and the cross-cutting test invariants
have a home.

**Box areas:**
- Test-levels doctrine: unit · integration (real files, **never mock the thing under test**) · property · fuzz · E2E · a11y · visual.
- Corpus / fixture conventions — single-source helper, auto-discovery, no inline duplication.
- Round-trip invariant defined (property + CI gate G32).
- Property-test conventions — fixed case counts, **shrinking mandatory**; fuzz conventions (`cargo-fuzz` on decode path).
- Flaky-test policy — retry infra/timeout only, E2E-only auto-retry, determinism engineered (pinned locale/timezone, animations off).
- "Build fully, no skeleton/stub" rule wired to the deferral gate (G8).
- Coverage thresholds + shard-merge determinism.

**Home:** docs/process/test-strategy.md · 06-build-test-release (§6.4 corpus/reliability).

---

## P0.6 — Dual review (holy grail) & commit protocol

**Goal:** the Opus/Sonnet review and the commit discipline are specified and
enforceable.

**Box areas:**
- Dual-review protocol (G1) — staged-diff input, P0–P3 severity, converge/diverge, fix-before-push (no fix-push cycle), skip conditions.
- Review-trace commit trailer + its format gate (G12).
- Commit conventions — Conventional-commit + body (spec-§ + box-id + findings + co-author trailer).
- Escalation rules — when the Build-Loop escalates to the Co-Pilot session; the hard-stop conditions; the "decide-it-yourself" default + tagging (`[Build-Session-Entscheidung]`).
- Box-counter / session-stop discipline (token notbremse).

**Home:** docs/process/build-loop.md · docs/process/roles-and-escalation.md.

---

## P0.7 — Release & supply-chain gate policy (built in P10)

**Goal:** the release-plane gate policy + acceptance criteria are defined so P10
only wires them.

**Box areas:**
- SBOM generation + completeness + license hard-fail policy (G35/G36).
- Engine checksum/integrity build gate + per-engine build assertions (G37/G38).
- Checksums + minisign + signing/provenance policy — **key provisioned AND step wired** (G39/G40).
- Artifact size-budget policy (G41).
- Offline-egress + no-system-pollution observability gates (G42/G43).
- Governance-completeness + name-clearance gate policy (G44/G45).
- Startup-integrity acceptance (G46).

**Home:** build-gates.md §5 · 03-engines-and-bundling · 06-build-test-release · 07-app-shell.

---

## Exit criterion for P0

P0 is "done" when: both enforcement planes are live; every content-independent
gate (P0.3) runs green on both planes; the language-gate contracts (P0.4) are
defined with CI wiring-points; the five process/security docs exist and pass the
doc-consistency gate; and the build-loop + dual-review protocol are written such
that P1's first box can be built strictly through the loop with no guardrail
missing.
