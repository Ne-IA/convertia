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
- Git-hook manager (`lefthook`) set up (pre-commit / pre-push / commit-msg), `parallel`, perf budgets; the **hooks-installed assertion** (G54 — `lefthook install` mandatory after clone, no local protection without it).
- GitHub Actions CI skeleton (L4) — clean-checkout job matrix placeholders (Win/macOS/Linux).
- Release workflow skeleton (L5) — tag-triggered, empty gate slots.
- **CI supply-chain hardening** (content-independent — lives in `.github/`): every workflow declares top-level `permissions: contents: read`, elevated per-job only where needed (the release job touching `MINISIGN_SECRET_KEY` gets `contents: write` ONLY); every third-party action pinned by **full 40-char commit SHA** (not a tag), kept current via a `dependabot.yml` github-actions entry; `actionlint` (G49, L1) + `zizmor` (G50, L4) workflow lint; the secret-bearing release job **never runs on a fork pull-request**.
- Defense-in-depth plane definitions + fail-open/closed policy wired as config.
- Fastpath / skip detectors (docs-only, check-off) + their self-tests (G10/G24) — the docs-only guard operates on the **range diff over all unpushed commits**, not the HEAD subject.

**Home:** build-gates.md §0–§4 · 06-build-test-release (record the `actionlint`/`zizmor`/token-scope CI hardening in the spec per the living-doc rule).

---

## P0.3 — Content-independent gates (buildable now)

**Goal:** every gate that needs no app code is live on both planes.

**Box areas:**
- Secrets / credential scan — **`gitleaks`** (G2), pinned version, committed `.gitleaks.toml` allowlist, `--staged` at L1 + full at L4.
- WebView **CSP + capability structural lint** (G47) — `jq`/`serde_json` parse of `tauri.conf.json` csp + `src-tauri/capabilities/*.json`, fails on any §0.10 violation incl. the `tauri-plugin-updater`/HTTP-client/`updater`-block absence (content-independent — the conf/capabilities files exist from P1, but the linter is authored here).
- Conventional-commit + dual-review-trailer (G11/G12) — exact `Dual-Review:` regex at pre-push.
- Deferral / dead-marker gate (G8/G21) — incl. the no-`any` / no-inline-CSS CLAUDE.md markers; L4 mirror is **fail-closed full-tree**.
- `plan-lint` / `spec-lint` doc-consistency linter (G7/G20) — incl. its own unit tests + the §0.11↔§5 parity check (check 8) + inventory parity (check 9).
- License + supply-chain policy config (`cargo-deny` `deny.toml` **skeleton**; G18) — `[bans]` updater/HTTP-client deny-list, `[licenses]` GPL/AGPL deny, `[sources]` allow-list — plus the **lockfile-integrity** wiring (G18a) and the **`cargo-vet`** scaffold (G18b). *(The G36 SBOM forbidden-family release gate is homed in P0.7 with the other L5 policy — not here.)*
- Generated-artifact drift-check framework (G19).
- Repo-invariant grep gate scaffold (G9); prose-typo gate (G51, `typos`) + EOL/charset hygiene (G52, `.editorconfig` + `editorconfig-checker`).

**Home:** build-gates.md §2/§4/§6.

---

## P0.4 — Language & build gate contracts

**Goal:** the contract + CI wiring-points for the language gates are defined; full
activation happens when P1 scaffolds the toolchains.

**Box areas:**
- Rust gate contracts: rustfmt, `clippy -D warnings`, `cargo test`, `cargo audit --locked`, `cargo deny`, `cargo-vet`; the **unsafe-policy** primary SAST gate — `#![forbid(unsafe_code)]` at the core-crate root + an allow-listed FFI module + "no new `unsafe` outside it" (G29); **Semgrep** packs (`p/rust` + `p/typescript` + `p/security-audit` + project-local rules) pinned/vendored; `cargo-geiger` **informational only**; `cargo fuzz` harness layout (the **in-core `crate::detect` target** G48 — Linux+macOS nightly — NOT the isolated engines, which are the §6.4.2 corpus fault-injection G26). `→ activated in P1`.
- TS gate contracts: `tsc` strict, eslint, `prettier`, vitest. `→ activated in P1`.
- Coverage gates: **per-domain floors** — `cargo-llvm-cov` (Rust branch) AND vitest v8 (TS), fail if EITHER below its floor (never averaged); ratchet 50→70 stored in a tracked file (increase-only, no auto-increment); diff gate ≥80% (G27/G28).
- Lockfile-integrity contract (G18a) — `--locked` / `--frozen-lockfile` + `git diff --exit-code` on the lockfiles.
- Cross-platform build-matrix contract (G30) — native per-OS, macOS universal.

**Home:** build-gates.md §4/§5 · 06-build-test-release (§6.4.2 corpus fault-injection; home the `cargo-fuzz` in-core harness + the SAST/unsafe-policy layer in the spec per the living-doc rule) · 00-architecture (§0.4.5 type-drift).

---

## P0.5 — Test methodology & harness conventions

**Goal:** *how we write tests* is defined and the cross-cutting test invariants
have a home.

**Box areas:**
- Test-levels doctrine: unit · integration (real files, **never mock the thing under test**) · property · fuzz · E2E · a11y (the §6.4.6a jsdom/`vitest-axe` ARIA leg = G33a, vs the Lane-B `@axe-core/webdriverio` contrast leg = G33b).
- Corpus / fixture conventions — single-source helper, auto-discovery, no inline duplication; **explicit decompression-bomb corpus FIXTURES** (svgz bomb, ZIP-bomb-in-OPC DOCX, deeply-nested PDF flate stream) so the §6.4.2 bomb case is backed by files, not just a property concept.
- Round-trip invariant defined (property + CI gate G32).
- Property-test conventions — **language-split, not either/or:** Rust = **`proptest`** (macro-based shrinking, no manual `Shrink` impls — satisfies "shrinking mandatory"); TS = **`fast-check`**; the coverage-guided **`cargo-fuzz`** harness is the **separate** in-core `crate::detect` target (G48), with the XML peek's DTD/external-entity resolution disabled by construction.
- Flaky-test policy — retry infra/timeout only, E2E-only auto-retry, determinism engineered (pinned locale/timezone, animations off).
- "Build fully, no skeleton/stub" rule wired to the deferral gate (G8).
- Coverage thresholds (per-domain) + shard-merge determinism.
- **Cross-cutting security-test homes (defined here, activated by their phase):** the §7.5 **log-redaction** property gate (feed a secret-looking path stem through the logger, assert absent), the §2.14.1 **temp ownership + mode-bits** assertion (`0o700` scratch root / `0o600` `.part` publish-temp), and the **T10 adversarial resource-budget** gate (oversized-render SVG, over-duration to-GIF, over-cardinality batch → fail-clearly, batch continues, no handle/RAM exhaustion).

**Home:** docs/process/test-strategy.md · 06-build-test-release (§6.4 corpus/reliability; record the bomb fixtures §6.4.5 + the §2.14.1 mode-bit pin in the spec per the living-doc rule).

---

## P0.6 — Dual review (holy grail) & commit protocol

**Goal:** the Opus/Sonnet review and the commit discipline are specified and
enforceable.

**Box areas:**
- Dual-review protocol (G1) — staged-diff input, P0–P3 severity, converge/diverge, fix-before-push (no fix-push cycle), skip conditions; **the dual review is a quality amplifier, NOT a security control** (only G2–G50 are security controls).
- Review-trace commit trailer + its format gate (G12 — exact `^Dual-Review: opus=(GO|NOGO) sonnet=(GO|NOGO)$` at pre-push).
- **Staged-diff sanity:** before `git commit`, `git diff --cached --stat` must match what the reviewers saw at GO; any post-GO file add/remove forces re-review.
- Commit conventions — Conventional-commit + body (spec-§ + box-id + findings + co-author trailer); solo-on-`main` rollback convention (`chore(scope): roll back — reason`, no `revert` type).
- **Definition-of-Done (the ConvertIA-adapted RMACLAUDE §11, dropping RLS/audit/migrations):** (a) spec-§ referenced or marked tooling-only; (b) spec synced in the same commit; (c) tests at the highest sensible level green; (d) hard gates green with **no** `--no-verify`; (e) dual-review done + trailer; (f) inline `[Build-Session-Entscheidung]` at non-spec choice sites; (g) `engines.lock`+SBOM row if a new engine staged; (h) §0.11+§5 row if a new threat class introduced.
- **Build-loop soundness rules** (the autonomous-direct-to-main model depends on these): **(1) session-start CI health** — query the last Actions run on `main` (`gh run list --branch main --limit 1 --json status,conclusion`); STOP+escalate on failure/cancelled, proceed on success/pending/queued, fail-open (warn+continue) if the API is unreachable; **(2) push-exit-code wait** — after `git push` (triggers pre-push hooks), wait synchronously for the exit code; on non-zero do NOT proceed to the check-off commit or next box (fix, re-stage, re-review, retry; escalate after 3 failures); never start a new box while a push is unresolved; **(3) next-box selection** — lowest-phase/lowest-box first; `[!extern]`/`[!]`-blocked boxes skipped+reported; after each check-off, scan for `[!]` boxes the just-completed box unblocked and unlock them; on zero open boxes report convergence and stop (never loop forever); escalate on a genuine all-blocked deadlock.
- **Conflict rule** (written as a build-loop rule): **SSOT > spec > security/process docs > code > conversation.**
- Escalation rules — when the Build-Loop escalates to the Co-Pilot session; the hard-stop conditions (concrete box-counter / token-notbremse numbers); the "decide-it-yourself" default + tagging (`[Build-Session-Entscheidung]`).
- **Vuln-response runbook** (`docs/process/vuln-response.md`) — because the app ships known-vulnerable-class C/C++ decoders against untrusted files with **no auto-update**, the only path a security fix reaches a user is a new full release: advisory (G17b/upstream) → Build-Loop escalates to Co-Pilot → bump the `engines.lock` pin → re-run the §6.5 reliability gate → new release; matches the `SECURITY.md` "no SLA, best-effort" posture with an actual triage process.

**Home:** docs/process/build-loop.md · docs/process/roles-and-escalation.md · docs/process/vuln-response.md.

---

## P0.7 — Release & supply-chain gate policy (built in P10)

**Goal:** the release-plane gate policy + acceptance criteria are defined so P10
only wires them.

**Box areas:**
- SBOM generation + **completeness** policy (G35) — `cargo cyclonedx` + **`@cyclonedx/cyclonedx-npm`** (`--spec-version 1.5`) for generation; **`Syft` mandatory** (not optional) for the staged-bundle completeness cross-check, backed by a deterministic stage-tree file-manifest diff (an unexpected `.so`/`.dll`/`.dylib` hard-fails) — plus the license hard-fail (G36) forbidden-family policy (moved here from P0.3).
- **Copyleft corresponding-source bundle-present** policy (G38b) — the §6.1.3 carve-out ii/iii assertion (LGPL relink source + x265 GPL §3 corresponding source/offer present or the build fails); maps to the §5 T6 row.
- Engine checksum/integrity build gate (G37 — verify each engine's SHA-256 against the change-reviewed in-repo `engines.lock` **before staging AND on cache-restore**) + per-engine build assertions (G38, incl. the T2c `tauri-plugin-store`-cannot-escape-`config_dir` assertion).
- Checksums + **minisign over `SHA256SUMS`** policy — the **only** signing in scope (G39); **key provisioned AND the `minisign -Sm SHA256SUMS` step wired**. *(No `cosign`/SLSA binary-signing or build-provenance — SSOT *Out of Scope*; the former G40 is deleted. GitHub's free `actions/attest-build-provenance`, if ever wanted, is a non-blocking owner-approved bonus that must be reconciled into the spec FIRST.)*
- Bundled-engine CVE-awareness (G17b) — **informational** OSV/grype scan over `engines.lock`, dated open-CVE report as an owner-signed-off release asset (honours the SSOT "engine currency best-effort, not a gate").
- Auditable Rust binary (G55) — `cargo auditable build --release` so the shipped artifact embeds its dependency list.
- Artifact size-budget policy (G41).
- WCAG-AA contrast a11y (G33b) — `@axe-core/webdriverio` Lane-B, Linux+Windows, macOS human-walkthrough gap noted.
- Offline-egress (G42 — **active OS egress-DENY + observe-the-attempt**, per-OS tooling, `.env_clear()` engine-spawn invariant) + no-system-pollution (G43) observability gates.
- Governance-completeness + name-clearance gate policy (G44/G45).
- Startup-integrity acceptance (G46).
- **Release-job token scope:** the secret-bearing release job declares `contents: write` ONLY and never runs on a fork pull-request (the §5 cross-cutting CI-hardening row).

**Home:** build-gates.md §5 · 03-engines-and-bundling · 06-build-test-release · 07-app-shell.

---

## Exit criterion for P0

P0 is "done" when: both enforcement planes are live; every content-independent
gate (P0.3 — incl. G2 `gitleaks`, G47 CSP/capability lint, G49/G50 CI hardening)
runs green on both planes; the language-gate contracts (P0.4) are defined with CI
wiring-points; the **six P0.1** process/security docs
([security-concept.md](../security/security-concept.md),
[build-gates.md](../security/build-gates.md), `build-loop.md`, `test-strategy.md`,
`roles-and-escalation.md`, `_format.md`) exist and pass the doc-consistency gate
(the P0.6 `vuln-response.md` runbook is authored alongside the build-loop docs);
the **security-concept §5 threat table is fully populated vs spec §0.11** (all 14
classes have a control + a concrete `Gnn`, enforced by plan-lint check 8); and the
build-loop + dual-review protocol (incl. the build-loop soundness rules + the
ConvertIA DoD) are written such that P1's first box can be built strictly through
the loop with no guardrail missing.
