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
- **Custom gate scripts are themselves tested** (positive **and** negative cases —
  every custom gate and fastpath detector ships a self-test that proves it FAILS on
  a planted violation, not only that it passes clean) under a narrowly-scoped
  self-test gate (G24).
- **Structural parsing over regex.** A gate that consumes a generated/structured
  file (JSON/TOML/YAML/lockfile/SBOM) **parses** it (`jq`/`serde_json`/a real YAML
  reader), never a regex — and a gate that **cannot resolve or parse its input fails
  CLOSED in CI** (a missing/unparseable target is a gate failure, not a skip).
- **Offline tolerance.** Any gate with a network step (advisory-DB refresh, rule
  fetch) **decouples** the refresh (warn-only) from the check (hard-fail against
  the local/vendored DB), and honours an offline env flag.
- **The dual review (G1) is a quality amplifier, NOT a security control.** Only the
  deterministic gates — **every `Gnn` except G1** — are security controls. The
  `Dual-Review:` trailer is self-attested and unverifiable, so a gamed trailer cannot
  ship insecure code — the gates either pass on a clean checkout or they do not. (The
  range is phrased "every gate except G1" rather than a frozen *G2–G50* bound, which
  drifts each time a gate is added; the catalogue is the authoritative span. plan-lint
  check 5 asserts the prose never claims a span narrower than `max(Gnn)`.)
- **Gate-toolchain is pinned + verified as a class.** Every gate/SAST/SBOM/lint tool
  (Semgrep, Syft, `gitleaks`, `zizmor`, `actionlint`, `typos`, `osv-scanner`/`grype`,
  `cargo-fuzz`, the CycloneDX tools, `cdxgen`, `editorconfig-checker`, …) is pinned by
  **exact version AND verified by checksum / image digest at install** — vendored
  binaries / digest-pinned containers preferred over `cargo install`/`npm -g` from a
  live registry (a poisoned/typosquatted gate tool both misses a real finding and can
  read the CI secret). Spot-checked by the workflow lint. The Rust toolchain itself is
  pinned via a committed `rust-toolchain.toml` (an exact stable version + a separate
  nightly channel for `cargo-fuzz` G48), asserted not-floating.

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
- Recorded as a machine-readable commit trailer of the **exact** form
  `Dual-Review: opus=GO sonnet=GO` (or `NOGO`) — the *evidence of review* is
  mandatory; each reviewer's findings + convergence/divergence are recorded verbatim
  in the commit body (so a "both GO, 0 findings" on a non-trivial diff is an
  auditable smell for Co-Pilot spot-audit).
- **Staged-diff sanity (the trailer attests *this exact staged diff*).** Immediately
  before `git commit`, `git diff --cached --stat` MUST match the file set the two
  reviewers saw at GO; any file added/removed after GO **requires re-review** (no
  silent post-review staging).
- **Definition-of-Done.** A box is "done" only when it satisfies the ConvertIA DoD
  (authored in [build-loop.md](../process/build-loop.md), P0.6): (a) spec-`§`
  referenced or marked tooling-only; (b) spec synced in the same commit; (c) tests
  at the highest sensible level green; (d) hard gates green with **no** `--no-verify`;
  (e) dual-review done + trailer present; (f) inline `[Build-Session-Entscheidung]`
  tags at non-spec choice sites; (g) `engines.lock` + SBOM row if a new engine was
  staged; (h) §0.11 + security-concept §5 row if a new threat class was introduced.
- **Skipped only** for: (a) check-off commits with no code/config diff; (b)
  `[!extern]` boxes (nothing built).
- *Blocks:* the build commit (self-gate; the trailer is also checked at pre-push by
  G12).

## 2. L1 — pre-commit (cheap, < 10 s, parallel)

> *(Reading note: gate IDs are stable, so a gate appears at its **enforcement plane**,
> not in numeric order — the higher-numbered G47/G49/G51/G52 below sit in L1 because
> that is where they run, even though their IDs were assigned later. The `< 10 s`
> budget is now a **soft** target: `gitleaks` + the G47 jq parse + `actionlint`
> (shells out to `shellcheck`) + `typos` + `editorconfig-checker` + the
> diff-scoped fmt/clippy/tsc/eslint were all added since the budget was written, and
> `actionlint`+`shellcheck` or clippy-on-a-cold-changed-crate can each approach 10 s;
> the heavier additions are diff/glob-scoped to hold the budget.)*

| ID | Gate | Tool / mechanism | Blocks | Scope / fail-mode |
|---|---|---|---|---|
| **G2** | Secrets / credential scan *(mirror)* | **`gitleaks`** (pinned version) — entropy + curated ruleset covering cloud keys, GitHub PATs (`ghp_`/`github_pat_`), PEM/private-key blocks, generic high-entropy literals; **plus a committed `.gitleaks.toml` CUSTOM RULE for the minisign secret-key shape** (the `untrusted comment:` header co-occurring with a long base64 blob, and/or banning a `*.key` from being staged) — a minisign secret key is **NOT** a PEM block (no `-----BEGIN … PRIVATE KEY-----` envelope), so the default PEM rule **cannot** catch the project's single most damaging secret (`MINISIGN_SECRET_KEY`; `MINISIGN_PASSWORD` is the companion) and the custom rule is the real guard. **Three legs:** L1 = `gitleaks protect --staged`; L4 = full-tree `gitleaks detect --no-git` (working tree); **release-tier (L4 nightly / L5)** = full-**history** `gitleaks detect` over the commit log (a once-committed-then-removed secret stays live forever in a **public** repo with no PR-review backstop). Committed `.gitleaks.toml` allowlist + one-time baseline for test-fixture keys so the first history run doesn't flood | commit (history leg: release-tier) | always-on (no glob); fail-closed |
| **G3** | Format check *(mirror)* | `cargo fmt --check` + `prettier --check` (TS/CSS/JSON; check-only, no auto-write) — Biome's linter, if wanted, belongs in G5, not here | commit | by file glob; fail-closed |
| **G4** | Lint — Rust *(mirror)* | `cargo clippy -D warnings` — **plus the no-panic-sloppiness policy** (`clippy::unwrap_used` / `expect_used` / `panic` at **deny** in production crates, `+ indexing_slicing` for `crate::detect`; allow-listed in `#[cfg(test)]`, with a `// PANIC:`-justified escape — same allow-listed-module pattern as G29's unsafe policy). The §1.2 detect path runs untrusted bytes **outside** the §2.12 boundary (principle 9), so a stray `.unwrap()` there is a **guaranteed in-core DoS (T5)** — the lint prevents the class, G48 fuzz finds the residue. Also set **`#[deny]` on non-exhaustive matches** (no `_ =>` catch-all) for the dispatch enums `FormatId`/`EngineProgram`/`PatentDisposition` + the error taxonomy, so adding a registry format/variant without updating every dispatch arm is a **compile error**, not a misroute caught late by a P5–P7 corpus test | commit | changed-crate scope; fail-closed |
| **G5** | Lint — TS/React *(mirror)* | `eslint` (flat config) + `stylelint` (CSS) | commit | by glob; fail-closed |
| **G6** | Type-check (fast) *(mirror full at L2)* | `tsc --noEmit` on changed scope; (`cargo check` is implicit in clippy) | commit | by glob; fail-closed |
| **G7** | Doc-consistency `plan-lint`/`spec-lint` *(mirror)* | custom stdlib-only script (see §6) `--quiet` | commit | on docs glob; any finding = exit 1 |
| **G8** | Deferral / dead-marker gate *(mirror)* | diff-based scan for `TODO`/`FIXME`/`unimplemented!`/`todo!`/`unreachable!`/`dbg!`/`println!`/`console.log`/`": any"`/`as any`/inline `style=`/`style:`/"stub"/"placeholder"/"phase 2" in **new** production lines lacking a **box-id, `[!extern]`, or `[Build-Session-Entscheidung]`** suppressing marker within ±6 lines (the `any`/inline-CSS markers encode the CLAUDE.md hard rules; a correctly-placed `[Build-Session-Entscheidung]` tag is a documented choice site, not a deferral, so it suppresses) | commit | new-marker only; **fail-open** if no diff base (mirror at L4 is **fail-closed**, full-tree, no diff-base excuse) |
| **G9** | Repo-invariant grep gate *(mirror)* | cheap repo-wide regex for a project invariant (e.g. no hardcoded colours outside token primitives), path-whitelisted; the structural CSP/capability invariant is its own gate **G47** (it parses, not greps) | commit | by glob; fail-closed |
| **G10** | Fastpath self-tests | `test-*-fastpath-pattern` smoke tests (script naming convention documented in P0.2; → scripts authored in P0.2) | commit | only when a fastpath detector is edited; positive **and** negative self-tests required |
| **G47** | WebView CSP + capability structural lint *(mirror — L1 cheap, L4 structural)* | parse `tauri.conf.json` `app.security.csp` + `src-tauri/capabilities/*.json` with `jq`/`serde_json` (**not** regex) and **diff the FULL §0.10 CSP shape directive-by-directive against the locked golden value** (not a deny-list of a few directives): `connect-src` is an **ALLOW-list** — the only non-`'self'` tokens permitted are exactly `{ipc:, http://ipc.localhost}`, so any unknown future token (`blob:`/`data:`/`https://…`) **fails**; `script-src` has no remote origin and **no `unsafe-eval`**; `img-src`/`media-src` carry **NO `asset:`** (a deliberate v1 prohibition, §0.10); `object-src = 'none'`, `form-action = 'self'`, `base-uri` present, `frame-src = 'none'`. Also FAIL on: any `fs:`/`http:`/`shell:allow-execute`/`opener:*`/`dialog:` grant in a capability file; any `updater`/bundle-updater block or updater pubkey in the conf; **and `app.security.assetProtocol.enable` being present/true** (if a dev enables the asset protocol without adding `asset:` to the CSP, the WebView can still read bundled files via `asset://localhost` despite the CSP host token being blocked — so this must be asserted absent/false). Verifies the §7.6.1 updater-absence claim structurally; the concrete instantiation of the G9 invariant placeholder for T2/T2a/T2c/T9a. *(Low-sev follow-up: add `frame-ancestors 'none'` to the §0.10 block + the asserted set; `webrtc 'block'` is already in §0.10.)* | commit | always when the conf/capabilities glob matches; fail-closed |
| **G49** | Workflow lint (fast) *(mirror)* | **`actionlint`** — YAML/expression/shell lint of `.github/workflows/*` (catches syntax, bad `${{ }}` expressions, shellcheck issues in `run:` steps) | commit | on `.github/` glob; fail-closed |
| **G51** | Prose typo gate *(mirror)* | **`typos`** (`typos-cli`) — curated-list typo finder (not a dictionary spell-checker; near-zero false positives on Rust/TS identifiers) over public-facing prose: `SECURITY.md`/`PRIVACY.md`/`TRADEMARK.md`, **`docs/security/security-concept.md` + `docs/security/build-gates.md`** (both are referenced from `SECURITY.md` and carry the `minisign` verify recipe + gate descriptions), the verify-your-hash recipe, the user-facing error/string catalog — a typo in the security policy or the `minisign` verify recipe is a trust-damaging defect G8/G21 (markers, not prose) miss | commit | on docs/strings glob; fail-closed |
| **G52** | Cross-platform EOL/charset hygiene *(mirror)* | committed **`.editorconfig`** + **`editorconfig-checker`** — EOL/charset/final-newline guard for `.toml`/`.yaml`/`.md`/shell scripts (G3 covers only `cargo fmt` + `prettier`-managed TS/CSS/JSON, leaving these unguarded; a CRLF drift in a `.sh` gate-script/hook is a real Windows footgun) | commit | by glob; fail-closed |

## 3. L3 — commit-msg

> *(Ordering note: L3 is documented here, right after L1, because both fire at
> `git commit` time — the two commit-time planes are grouped; L2 (`git push` time)
> follows in §4. The defense-in-depth plane list in [security-concept.md §3](security-concept.md#3-defense-in-depth--the-enforcement-planes)
> remains in strict L0→L5 order.)*

| ID | Gate | Tool / mechanism | Blocks | Scope / fail-mode |
|---|---|---|---|---|
| **G11** | Conventional-commit format | regex `^(feat\|fix\|chore\|docs\|refactor\|test\|perf\|ci\|build)(\([a-z0-9._-]+\))?: .+` (first line); merge/revert/fixup exempt. Solo-on-`main` rollback convention: `chore(scope): roll back — <reason>` (no `revert` type for build-session commits) | commit | always; fail-closed |
| **G12** | Dual-review trailer present + well-formed | **plane: L2 (grouped with G11 only by documentation locality — G12 functionally fires at *pre-push*).** The build commit body carries a trailer matching `^Dual-Review: opus=(GO\|NOGO) sonnet=(GO\|NOGO)$`; skipped for check-off/`[!extern]`. **Implementation note:** it runs at pre-push (not commit-msg) because it needs **conditional skip logic over the whole push range** — the bodies are read via `git log --format=%B <old>..<new>` and the check-off/`[!extern]` commits in the range are excluded. (The body *is* available to a commit-msg hook; the real reason for pre-push is the range-scoped conditional, not body availability.) | push | conditional; fail-closed |

## 4. L2 — pre-push (heavier, < 3 min, parallel; cheap-commit fastpath)

| ID | Gate | Tool / mechanism | Blocks | Scope / fail-mode |
|---|---|---|---|---|
| **G13** | Full type-check *(mirror)* | `tsc --noEmit` whole project | push | always; fail-closed |
| **G14** | Full lint *(mirror)* | `clippy --all-targets --all-features -D warnings` (incl. the G4 no-panic-sloppiness `unwrap_used`/`expect_used`/`panic`/`indexing_slicing` deny set + the exhaustive-match deny) + `eslint` whole tree | push | always; fail-closed |
| **G15** | Unit + integration tests *(mirror)* | `cargo test` (incl. real-file round-trips) + `vitest run`. **Atomicity-under-interruption (§6.4.2):** the kill is injected **specifically in the post-`sync_all()`-pre-`rename` window** (a `#[cfg(test)]` fence in `crate::fs_guard::atomic_publish`, all 3 OS) so it exercises the §2.1.3 two-state invariant at the critical boundary, not an uninteresting pre-sync kill. **Scoped mutation-testing sub-leg (release-tier, owner-decidable required-vs-informational):** `cargo-mutants` over `crate::fs_guard` + `crate::detect` + `crate::outcome` (the no-harm/atomicity/no-misroute kernel) — line coverage proves a line executed, not that a test would CATCH a regression there | push (mutants: release-tier) | always; fail-closed |
| **G16** | Property + fuzz smoke *(mirror)* | property tests — **`proptest`** (Rust, macro-based shrinking, no manual `Shrink` impls — satisfies the §P0.5 "shrinking mandatory" rule) + **`fast-check`** (TS, custom `Arbitrary` must delegate to built-in shrinkers — **ban `fc.gen()` without a shrink wrapper**); plus a fast **deterministic** fuzz leg here (a saved-crash-corpus replay / `proptest` smoke over `crate::detect` — **not** an instrumented libFuzzer build, which is L4-only and Unix-nightly-only). **Determinism (property tests are non-deterministic by design):** a **pinned CI seed**, a **case-count floor** above the thin default 256 (the adversarial path/budget space is large), and a property failure is **NEVER retried to pass** (only infra/timeout flakiness is retryable, E2E-only — §P0.5 flaky policy). The coverage-guided `cargo-fuzz` harness is **G48** (in-core detector + `fs_guard` + CSV/TSV) + **G26** (full pass) | push | always; coverage-guided fuzz at L4 |
| **G17** | Dependency-vuln audit *(mirror)* | `cargo audit --locked` + `pnpm audit --audit-level=high` (DB refresh warn-only/offline-tolerant). **NB:** these cover only the Rust crate + npm graph — the bundled-engine CVE surface is **G17b** | push | always; check fail-closed, refresh fail-open |
| **G17b** | Bundled-engine CVE awareness *(informational per-push; release-tier escalation)* | feed the **PURL-keyed** `engines.lock` components (see G35 — each row carries a `pkg:generic/<name>@<version>` PURL, a CPE where one exists) to **`osv-scanner`** (consumes OSV — indexes FFmpeg/poppler/LibreOffice/x265 advisories) **or** `grype` (consumes the G35 CycloneDX SBOM); both match advisories **by PURL/ecosystem, not by a bare `(name, version)` string**, so a PURL-less manifest would silently match **nothing** and a green report would be a green-but-**empty** report masquerading as "no known CVEs". A **planted-positive self-test** (a deliberately-old pin that MUST surface a known historical CVE) guards against that empty-report failure. **Coverage caveat stated honestly:** `pkg:generic` PURLs miss CVEs that only a CPE indexes (FFmpeg/poppler/libheif carry such) — add a CPE per row where one exists. **Severity escalation rule (release-tier, NOT per-push):** a CVE with **CVSS ≥ 7 on an engine code path ConvertIA actively exercises for a §04 format** → the Build-Loop **escalates to Co-Pilot and blocks the next release** until bumped or triaged not-exercised (threshold stated in `SECURITY.md` so users know the effective turnaround). The per-push leg stays **non-blocking** (honours SSOT §3.8 "engine currency is best-effort, not a gate"); offline-tolerant (vendored DB, refresh warn-only). Two halves of the offline "audit-it-yourself" story: G17b (dated open-CVE report) + **G55** (embedded-SBOM auditable binary) | informational (release CVSS≥7 escalation blocks) | report only per-push; never `--no-verify` |
| **G18** | License + supply-chain policy *(mirror)* | `cargo deny check` with an **explicit** `deny.toml`: `[bans]` deny-list for **`tauri-plugin-updater`** + the common HTTP-client crates (`reqwest`/`ureq`/`hyper`/`isahc`/`curl`) — no socket-opening dep enters the core (T2/T9a); `[licenses]` GPL/AGPL **denied** for the Rust crate graph **and set fail-closed on unresolved/low-confidence detection** (symmetry with G36); `[advisories]` `yanked = "deny"`; `[sources]` populated allow-registry/allow-git list (the `sources` check is a no-op without it). *(name-based `[bans]` is the baseline T9a "opens no socket" control; the behavioural upgrade — `cargo-acl`/`cackle`, a per-crate `std::net`/`std::process` capability policy that catches a renamed/transitive network crate — is flagged as a §8 forward idea, Linux-only, policy-authoring-intensive.)* | push | always; fail-closed |
| **G18a** | Lockfile integrity *(mirror)* | CI builds/tests/audits with `--locked` (Rust) and `pnpm install --frozen-lockfile`; a post-install `git diff --exit-code Cargo.lock pnpm-lock.yaml` so a drifted lockfile **FAILS** rather than auto-resolving a different graph than the audited/SBOM'd one (§3.8 pin-everything) | push | always; fail-closed |
| **G19** | Generated-artifact drift *(mirror)* | regenerate Tauri→TS bindings / CLI `--help` / asset manifest, then `git diff --exit-code`; + structural (parsed, not regex) non-empty sanity | push | by glob; fail-closed |
| **G20** | `plan-lint`/`spec-lint` full *(mirror)* | the G7 script, verbose, all checks | push | always; fail-closed |
| **G21** | Deferral gate full *(mirror)* | the G8 scan vs `origin/main` | push | new-marker; fail-open w/o base |
| **G22** | Schema/membership parity *(mirror)* | "every supported format ∈ README matrix ∧ has a fixture ∧ has a round-trip test"; locale-file key parity (if i18n) | push | by glob; fail-closed |
| **G23** | "every X has a Y" completeness *(mirror)* | e.g. every `convert_*` command has a test (via `git ls-files`) — caveat: tracking-aware, stage partner file together | push | by glob; fail-closed |
| **G24** | Gate-script self-tests *(mirror)* | run the custom-gate unit tests | push | when a gate script changed; fail-closed |
| **G18b** | First-party crate-trust audit *(mirror)* | **`cargo-vet`** — records per-crate trust audits and **fails when an unvetted/changed crate enters the tree** (closes the gap G17 leaves: a *new* malicious/typosquatted crate has no advisory yet, so `cargo audit` can't see it); offline-friendly (audits are committed). **Bootstrap protocol (P0.3):** `cargo vet init`; `cargo vet import` ≥1 trusted external DB (Mozilla / Google supply-chain / ISRG) as baseline; `cargo vet suggest` then certify/exempt the initial tree with documented reasons; criteria policy = `safe-to-run` for build-only deps (incl. proc-macros) vs `safe-to-deploy` for runtime deps; a **new unvetted dep requires certify-or-documented-exemption and the Build-Loop ESCALATES a block** (never silently exempts). P0 exit includes a clean `cargo vet check` on the initial `Cargo.lock`; the audit-diff (which crates newly entered the tree) is surfaced to the dual review | push | always; fail-closed |
| **G18c** | JS registry pin + resolution-URL guard *(mirror)* | a committed **`.npmrc`** pins the registry, and a check asserts **every `pnpm-lock.yaml` resolution URL is from the allowed registry** (the `[sources]` analogue for the JS tree — dependency-confusion / source-substitution defence; the WebView IS the entire T2 attack surface, so its supply chain must match the Rust-side discipline) | push | always; fail-closed |
| **G18d** | JS install-lifecycle-script lockdown *(mirror)* | a committed **minimal `onlyBuiltDependencies` allowlist** (pnpm 10 blocks build/lifecycle scripts by default; this asserts + pins that posture) + a lint that **fails if the allowlist grows** or if `enable-pre-post-scripts`/`unsafe-perm` is set — a malicious dep otherwise runs arbitrary code via `postinstall` the moment `pnpm install` runs in CI (which holds the signing secrets at release time) | push | always; fail-closed |
| **G53** | Core-crate forbidden-dependency *(mirror)* | **`cargo-deny [bans]` with a workspace-member-scoped deny list** (OR a `cargo metadata`/xtask dep-tree walk) asserting the core crate's Cargo dependency closure does **NOT** contain the image-worker-only C libs (`libvips`/`libheif`/`librsvg`/`libimagequant`) — the build-time analogue of the §3.6 "LGPL must not link into the MIT core" assertion, the **explicit G53 verifying gate for the §5 T6 row**; a careless refactor that pulls a copyleft C lib into the core fails here | push | always; fail-closed |
| **G54** | Hooks-installed assertion *(mirror)* | a **Lane-A step runs `lefthook install` then `git diff --exit-code .git/hooks/`** (the chosen mechanism — a `post-checkout` hook can't be distributed because `.git/hooks` is untracked, so that option is circular; a single fresh-install hash is OS-specific because lefthook generates per-OS scripts). A clone that skipped `lefthook install` has no local L1–L3 protection and (single-Build-Loop model) **no PR gate to catch it**; "`lefthook install` is mandatory after clone" is documented in `build-loop.md`/`CONTRIBUTING.md` (cannot be enforced technically). **Two-planes parity (machine-true):** a parity check asserts every L1/L2 lefthook command id has a corresponding CI invocation — drive CI from `lefthook run pre-commit`/`pre-push`, or diff the lefthook command set against the CI job set — so a hook wired in `lefthook.yml` but never wired into CI can't silently have only one plane | push | always; fail-open only if Lefthook absent by design (CI never absent) |
| **G56** | CI runner-host integrity *(L4-authored; runs in CI)* | a workflow lint (zizmor custom check / a small jq-over-parsed-YAML script) that **fails any secret-using job (`MINISIGN_SECRET_KEY`/`MINISIGN_PASSWORD`) bound to a self-hosted runner label** — the secret-bearing signing/release step must run on an **ephemeral GitHub-hosted runner**, host-isolated from the Lane-B untrusted-corpus/fuzz jobs that run on the shared VPS (spec §6.1.4/§6.7.2). Also asserts the corpus/fuzz job and the signing job declare **disjoint runner hosts**, and the Linux jobs use `step-security/harden-runner` (Linux-only enforcement). **Plus the CI-config hygiene assertions:** `dependabot.yml` contains a `package-ecosystem: github-actions` entry with `directory: /` and a schedule (if deleted/missing, the SHA pins silently stop updating and miss security updates); every PUSH workflow declares a **`concurrency: {group: ci-${{github.ref}}, cancel-in-progress: true}`** (rapid autonomous-loop pushes otherwise spin overlapping full matrices — macOS bills ~10× Linux — and a "red main fixed immediately" model wants the superseded run cancelled) — **never** on the release/tag workflow (never cancel a release mid-sign); and **explicit `timeout-minutes` per job** (esp. the WebView/LibreOffice legs, which can otherwise run to the 6 h default). Implements security-concept principle 11 / the §5 CI-runner-host-integrity row | push (workflow glob) / red `main` | by `.github/` glob; fail-closed |

**Fastpath / skip (L2 only).** Expensive hooks (G15/G16/G17/G18 + heavy lint) are
skipped **only** when provably irrelevant, via detectors that each default to
*run* on ambiguity:
- **Docs-only push** — the hard safety guard is the **RANGE diff over ALL unpushed
  commits** (`git diff --name-only @{u} HEAD`, fallback chain below), **not** the
  HEAD-commit subject: if every changed path across the unpushed range is markdown
  ⇒ no Rust/TS/lockfile to scan ⇒ safe to skip the byte-scanning gates. A code
  change in *any* unpushed commit forces the full gate even if HEAD is docs-only.
- **Check-off fastpath** — `chore(todo): … abgehakt`-style subject **AND** a
  markdown-only diff.
- Detector fallback chain: `@{u}` → `origin/<branch>` → `origin/main` →
  `origin/HEAD`; **no base / 0 unpushed commits ⇒ run the full gate.** Skipping is
  opt-in; anything ambiguous runs everything. **First-push-to-an-empty-remote note:**
  on the very first push (no `@{u}`/`origin/main`/`origin/HEAD` yet) the detector
  correctly fails **open** to "run everything" — this is the **normal** no-base case,
  **not** a hook failure, so the Build-Loop must not misread it as a red push and trip
  the 3-failures escalation on session 1.
- **The docs-only guard is `.md`-only by path, and the security-load-bearing NON-`.md`
  files are deliberately NOT skippable** — `tauri.conf.json`, `src-tauri/capabilities/*.json`,
  `engines.lock`, `deny.toml`, `.gitleaks.toml`, `.npmrc`, `.github/**`, the SBOM inputs
  are non-`.md`, so any change to them already forces the full gate via the `.md`-only
  test. This is stated explicitly (RMA `is-docs-only-push.sh` style) so a future "docs-only
  also covers `.json` docs" relaxation cannot silently disable G47/G37/G18c.
- Cheap structural gates (G13/G20/G21/...) and glob-gated gates always run when
  their glob matches — they have nothing expensive to skip.

## 5. L4 — CI (GitHub Actions, post-push) & L5 — Release (`v*` tag)

### L4 — CI (clean checkout; mirrors L1–L2 + the heavy gates)
| ID | Gate | Tool / mechanism | Blocks |
|---|---|---|---|
| **G25** | All L1–L2 gates re-run on clean checkout | the same hooks/scripts in CI | red `main` |
| **G26** | Full fuzz pass (engine-side T1 = corpus fault-injection) | the §6.4.2 corpus/no-harm fault-injection **through the §2.12 isolation boundary** (truncated/0-byte/fuzzed-header/decompression-bomb inputs → one plain message, no crash, batch continues). *(`cargo-fuzz`/libFuzzer is in-process Rust and CANNOT reach the isolated C/C++ engines — the in-core fuzzable surface is **G48**; this row is the engine-side T1 control.)* | red `main` |
| **G27** | Coverage — per-domain floors | **`cargo-llvm-cov`** (Rust, LLVM branch) **and** **vitest v8** (TS) — **separate** floors, **per-domain = per-crate** (Rust) / per-package (TS), fail if **ANY** is below its floor (never averaged); ratchet **50 % → 70 %** stored in a tracked file (can only increase; a commit that lowers it fails; raises are deliberate committed config changes — no auto-increment). **Initial value + activation:** the tracked file is created at **0 %** in P0 (there is no app code yet) and **enforces from P1** (annotated `→ activated in P1`) so it does not trip on the empty P0 tree; **gate scripts are excluded from the coverage floors** (they are G24-self-tested instead). **Shard-merge determinism (3-OS matrix):** each leg emits a **named partial** report, merged in a **fixed order**, and the floor is applied to the **merged** report only (avoids the last-writer-wins race). The G48 saved-crash-corpus replay does **not** count toward the Rust floor | red `main` |
| **G28** | Coverage — diff gate | **≥ 80 %** on changed lines (change-only) so new code can't dilute the floor | red `main` |
| **G29** | SAST / static security | **unsafe-policy gate (primary):** `#![forbid(unsafe_code)]` at the root of **every first-party Rust crate (the core AND `convertia-imgworker`)** — the worker FFI-links libvips/libheif/libde265/librsvg/libimagequant and is the **densest unsafe surface** in the product (the first Rust code touching untrusted image bytes across a C/C++ FFI boundary), so it carries its own narrowly **allow-listed FFI module** (each `unsafe extern` with a `// SAFETY:` comment) exactly like the core (whose allow-listed FFI is the §2.1/§2.3 OS primitives `renameat2`/`MoveFileExW`/`GetFileInformationByHandle` + the §0.9 Job-Object kill). The gate is "**no new `unsafe` block outside the allow-listed FFI module**". **Semgrep** (pinned by version in `requirements-ci.txt`; **rulesets committed under `scripts/semgrep-rules/` at the pinned registry version**, regenerated on bump — `p/security-audit` is a managed registry pack fetched live from `semgrep.dev`, which **breaks under network isolation**, so the offline gate uses the community packs **`p/rust` + `p/typescript` + `p/bash` + `p/python`** plus the committed **project-local ruleset**: (a) a `#[tauri::command]` taking `PathBuf`/string-as-path from the WebView **without** an adjacent `fs_guard` validation call fails (T2a/T2b "forgot to validate"); (b) any `std`/`tokio` `process::Command::new` in `crate::isolation`/`crate::engines` **without** a `.env_clear()`/`.envs([])` in the same chain fails, and a `pandoc`/`soffice`/`ffmpeg` argv missing its mandatory safety flag (`--sandbox` / the macro-profile env / `-protocol_whitelist file,pipe`) fails — making the §5 T9b/G42 "every spawn is sanitized" a **structural** invariant, not just the one unit-tested spawn; (c) `process::Command` constructed outside `crate::isolation`). **`shellcheck` runs over ALL committed `.sh` gate-scripts/hooks** (`actionlint` only shellchecks workflow `run:` steps, not the standalone scripts, which are themselves code per G24) and a `p/bash`/`p/python` leg over `scripts/`+hooks. **`cargo-geiger` is INFORMATIONAL only** (a census, not an enforcer — version-fragile; never a required green check). *(Windows caveat: `.env_clear()` does NOT address PATH/DLL-search-order / `APPINIT_DLLS`/IFEO — those are the T3a side-loading controls, G37/G37b; G42's `.env_clear()` invariant is scoped cross-platform but the Windows DLL-search defence is the bundle-only `PATH`.)* | red `main` (Semgrep + unsafe-policy + shellcheck block; geiger informational) |
| **G30** | Cross-platform build matrix | native build on Windows / macOS / Linux runners (no cross-compile); macOS universal `lipo` | red `main` |
| **G31** | Per-pair corpus + reliability | real-file round-trips per `(source→target)` pair per platform; **output validated by a REAL per-format structural check** (re-detect output magic via §1.2 **plus** `ffprobe` decodable+correct codec for A/V · decode+nonzero dimensions via `vipsheader` for images · `pdftotext`/poppler opens for PDF · `unzip` + well-formed `[Content_Types].xml` for OOXML · field-count parity for CSV) — **not** a bare magic-sniff or "file exists"; reliability threshold. **Hosts these adversarial/security integration assertions when their input phase exists:** the fs-safety/membership/freeze/redaction/temp-mode/resource-budget assertions (T2a/T2b/T4/T9b/T10); **T9b sentinels** — a `.docm`/`.xlsm`/`.pptm` with an `AutoOpen`/`Workbook_Open` macro writing a canary file inside the egress-deny window → canary **NOT** created (proves LibreOffice macro-suppression), and a `WEBSERVICE()` `.xlsx` → no egress + no out-of-input read (pulled forward from §6.4.2 into the per-push leg); **T7 INPUT-side symlink/junction** — a dropped folder containing `innocent.jpg -> /etc/passwd` (or a Windows junction → System32) is `resolve_identity`-resolved before the engine sees it: assert the engine receives the **resolved real path**, the frozen-set records the resolved identity (symlink+target dropped together convert once), a resolved non-convertible target fails clearly per §2.8; **Windows AV-retry** (§2.1.2) — a fault-injection test thread holding a handle on the publish target asserts the bounded `MoveFileExW`/`ERROR_ACCESS_DENIED` retry fires and recovers or fails to a clean `WriteFailed`, never a panic/silent discard; **privilege-drop-tier-applied regression** — a positive assertion per platform that the §2.12.3 tier actually FIRED (a denied syscall/socket/exec is refused inside the engine's own sandbox profile) so the silent-by-design degrade can't disable seccomp/Landlock/AppContainer on every run unnoticed (the §6.4.2 probe asserts availability; this asserts application), recording "tier applied vs degraded" per platform in the release evidence; **T10 process-group/Job-Object reap** — a deliberately-hanging/child-spawning sidecar is reaped by the §0.9 Job-Object/process-group kill with no orphan/zombie left and handle count returning to baseline | red `main` |
| **G32** | Source-unchanged + output-validity invariant | replaces the methodologically-vacuous "A→B→A byte-stable / within tolerance" (most of the catalogue is **one-way** — HEIC→JPG has no JPG→HEIC; XLSX→CSV→XLSX routes the reverse through LibreOffice, lossy — so a naive round-trip is either trivially-true under any tolerance or always-false). **Two sharp invariants instead:** (a) **SOURCE-UNCHANGED** — `sha256(source_before) == sha256(source_after)` on **every** corpus file (the no-harm proof, T2/T7); (b) **OUTPUT-VALIDITY** — the produced output passes the **real per-format structural check** of G31 (not magic-sniff). The literal A→B→A byte-stable check is **scoped explicitly to the small truly-lossless invertible set** (e.g. PNG→BMP→PNG, FLAC↔WAV if the pair exists) and the corpus pairs it covers are documented; everything asymmetric relies on (a)+(b). "Tolerance" is no longer an undefined fudge — there is no tolerance band, only structural validity | red `main` |
| **G33a** | a11y — ARIA/role/focus (per-PR) | **`vitest-axe`** (axe-core under jsdom) over the rendered React tree: ARIA-role/state validity + focus-order / roving-tabindex sanity. Lane-A per-PR (jsdom cannot compute contrast) | red `main` |
| **G48** | In-core untrusted-byte fuzz | **`cargo-fuzz`** (libFuzzer) targets over the in-core surfaces that process untrusted bytes **outside** the §2.12 boundary (§2.12.4), so a panic/OOM/UB lands in the core: **(1) `crate::detect`/sniff** on a hostile corpus (malformed ZIP central-directory, OLE2/CFB, gzip/svgz, XML) — no panic/abort, the §1.2 decompression-ratio cap (≤100×) + `MAX_SVGZ_SNIFF` (≤64 KiB) bound actually fire, the XML reader has **DTD/external-entity resolution disabled by construction** (`quick-xml`/`roxmltree` with entity resolution off — defeats XXE / billion-laughs in the `xl/workbook.xml`/ODS `content.xml` peek); **(2) `crate::fs_guard::resolve_identity`/`is_safe_output`** on untrusted PATHS from WebView/OS drag-drop (null bytes, overlong UTF-8, max-length, symlink chains, `..`) — no panic, structured `Err` on bad input, **never `Ok` on a null-byte path** (T7+T2a); **(3) the in-core CSV/TSV native engine** (§3.5.6 `EngineProgram::InProcessNative` — memory-safe ≠ panic/OOM-safe; gigabyte quoted fields, recursive quoting, NUL bytes) — no panic, bounded output relative to input, clear failure beyond a column floor. **Crash-corpus persistence:** every libFuzzer-found crash is minimized and committed under tracked `fuzz/corpus/` + `fuzz/crashes/`, and the **G16 deterministic replay runs the committed crash set on EVERY platform incl. Windows** (where instrumented fuzzing is unavailable) so a fixed crash can't silently regress. **Determinism:** a pinned CI seed; a case-count floor above libFuzzer's thin default; a property failure is **NEVER** retried to pass. Constrained to where libFuzzer is reliable (**Linux + macOS, nightly toolchain**); the L2 leg is the deterministic G16 replay, never an instrumented Windows build | red `main` |
| **G49** | *(see L1 — `actionlint`, mirrored)* | mirrored in CI on a clean checkout | red `main` |
| **G50** | Workflow security lint | **`zizmor`** (Rust GH-Actions static analyzer) — flags unpinned actions (mutable tags vs full commit SHA), dangerous `pull_request_target`, template-injection via untrusted `${{ github.event.* }}` in `run:` steps, and excessive `GITHUB_TOKEN` scope | red `main` |

**Per-push adversarial-egress pull-forward.** On runners that support the §6.7.3
enforcement path, the §6.4.2 adversarial-egress corpus (HLS `m3u8`, DASH `mpd`,
`concat` script, external-`href` SVG, remote-`<img>` pandoc, `WEBSERVICE()` xlsx)
runs in the per-push **L4** integration leg (under G42's egress-deny window) so a
**T9b** egress regression introduced in P6/P7 is caught on the push that introduced
it; **G42** is the final release confirmation. (The macOS WebView leg degrades to
the §6.6 walkthrough — see G42/G33b.)

> **G34 is intentionally vacated.** A screenshot/visual-regression gate has **no
> §-home in the spec** (§6.4.6 is the WebDriver flow; §6.4.6a is axe) — a
> release-blocking gate with no spec home is not added. If visual-regression is
> wanted, **add a spec §6.4.6-family entry first, then assign an id from the vacant
> range above G56**; the id `G34` stays reserved/unused so existing references do not
> renumber.

### L5 — Release (tag-triggered; release-blocking)
| ID | Gate | Tool / mechanism | Blocks |
|---|---|---|---|
| **G33b** | a11y — WCAG-AA contrast (release-tier) | **`@axe-core/webdriverio`** against the live WebView (`tauri-driver`) — WCAG 2.1 AA `color-contrast`, **both** themes, on the **Linux + Windows** legs (jsdom cannot compute contrast). **macOS is the acknowledged automated gap** (`tauri-driver` has no WKWebView driver, §6.4.6) → satisfied by the §6.6 human walkthrough's readable-contrast check, recorded in `docs/usability-floor.md` | release |
| **G35** | SBOM generation + completeness | **generation:** `cargo cyclonedx` (Rust) + **`@cyclonedx/cdxgen`** for the frontend (`--spec-version 1.5`) — **NOT `@cyclonedx/cyclonedx-npm`**, which runs `npm ls` against an npm `package-lock.json`/`node_modules` tree and does **not** natively understand pnpm's `.pnpm` symlink structure; ConvertIA is a pnpm project (G18a freezes `pnpm-lock.yaml`), so the npm tool would either error or SBOM an npm-resolved tree that **diverges** from the pnpm-locked graph G18a froze and G17 audited — and feed `grype` (G17b) the wrong component set. `cdxgen` has native `pnpm-lock.yaml` support (the frontend SBOM is **generated from `pnpm-lock.yaml`** so it matches the G18a-frozen graph). Merged via §3.7.2 `cargo xtask sbom` with `engines.lock`. **Each `engines.lock` row carries a mandatory `purl` (`pkg:generic/<name>@<version>` minimum, a CPE where one exists) + a SHA-256** (§3.7.2 schema reconciled in the same change), emitted into the CycloneDX component so G17b/G37 have a named key, not an implied one. **completeness (MANDATORY):** **`Syft`** scans the staged bundle so every shipped executable/lib/font ∈ `engines.lock`+declared sub-components, no `UNKNOWN`/`NOASSERTION` (except the §6.3.3 `LicenseRef` carve-out); **backed by** a deterministic stage-tree file-manifest diffed against `engines.lock` (Syft can miss libs statically compiled INTO FFmpeg/LibreOffice) so an unexpected `.so`/`.dll`/`.dylib` **hard-fails**, AND a staged shared object **not matching its `engines.lock` row** (the T3a side-loading guard) hard-fails. **Transitive-closure leg (the dangerous case for prebuilt C binaries):** Syft + the file-manifest catch only files PRESENT in the tree — the genuinely dangerous case is a transitive shared lib an engine **LOADS** that was NOT staged (a system/Homebrew/distro lib present on the build runner but absent on the user's machine — simultaneously an offline-floor break AND an SBOM hole), so G37b's `ldd`/`otool -L`/`dumpbin` closure check fails the build if any non-system dep resolves outside the bundle | release |
| **G35b** | SBOM-diff between releases *(informational, Co-Pilot-signed)* | diff each release's CycloneDX against the previous, surfacing added/removed/changed components — a careless P5–P7 stage commit adding a transitive `.so` would otherwise enter the bundle unreviewed; the diff is a Co-Pilot review item, **non-blocking** | informational |
| **G36** | License hard-fail (Rust + bundled) | SBOM scanned for forbidden families (GPL/AGPL in MIT core / static Rust binary) → exit 1 | release |
| **G36b** | License hard-fail (frontend) | the `cdxgen` frontend SBOM (G35) is scanned for **GPL/AGPL over the pnpm dependency graph** → exit 1 (`cargo-deny [licenses]` is Rust-only — nothing otherwise stops an AGPL/GPL npm dep tainting the MIT WebView; transferred directly from the RMA SBOM AGPL-frontend hard-fail). Runs `jq`/a license filter over the CycloneDX components | release |
| **G37** | Engine checksum / integrity build gate | pinned-version + SHA-256 verify of every bundled engine **AND every staged codec shared object beside it** (`.dll`/`.dylib`/`.so` — each individually `engines.lock`-rowed, the T3a side-loading guard, not just the primary `.exe`) **against the change-reviewed in-repo `engines.lock`** *before* staging **AND re-verified on cache-restore** (an Actions cache is not integrity-protected — on mismatch, delete + refetch from the pinned upstream URL); build-time in-bundle hash manifest generated. **Pin-establishment provenance (the xz/liblzma class):** when a NEW engine version is pinned, the recorded SHA-256 MUST be corroborated against the upstream project's own published checksum/signature (FFmpeg/LibreOffice/poppler all publish these), with the corroboration source URL recorded beside the pin; any `engines.lock` SHA edit is a **hard Co-Pilot escalation** (highest-value place to plant a backdoor). **Manifest cross-check:** the in-bundle hash manifest is generated **after** final staging and each staged binary's SHA-256 is re-read vs the just-generated manifest (so it can't be stale). On **Linux**, assert every staged engine binary has the exec bit (`test -x` per `engines.lock` entry) before AppImage assembly (a dropped `+x` otherwise surfaces as a misleading startup "integrity failed", not a packaging error) | release |
| **G37b** | Dynamic-dependency-closure assertion | at staging, run `ldd`/`readelf -d` (Linux) · `otool -L` (macOS) · `dumpbin /dependents` (Windows) on every staged engine binary/`.so`/`.dylib`/`.dll`: **every non-system dependency must resolve INSIDE the bundle** — fails the build otherwise. Generalises the §3.5.5 "x265 plugin must resolve from the bundle" concern to every staged binary; catches both a **T3a side-loading** vector and an **offline-floor break** (an engine linking a Homebrew/distro lib present only on the build runner). On **Windows**, engines are additionally spawned with an explicit **minimal `PATH` (the engine's own bundle dir only)** so the OS DLL search starts inside the bundle | release |
| **G41b** | Pre-publish archive validity | before publishing, a <30 s leg asserts each artifact is a **valid OPENABLE archive** (not just size-checked, G41): `unzip -t` (Windows `.zip`), `hdiutil verify` (macOS `.dmg`), `--appimage-extract-and-run`/`file`+`sha256sum` (Linux AppImage) — a corrupt artifact passing the size check is otherwise discovered only by users | release |
| **G38** | Per-engine build assertions | FFmpeg `-protocols`/`-demuxers` curated + required-codec lock + `concat -safe 1`; librsvg no-base-URL API + version-floor ≥ 2.56.3; libvips no-copyleft-PDF-loader (no `pdfload`/`poppler`/`mupdf` loader); LGPL shared-object-or-fail (carve-out i); libimagequant BSD-2-Clause leg-text + lockfile pin; the §6.1.3/§0.10 `tauri-plugin-store`-cannot-escape-`config_dir` assertion (T2c). **pandoc (T9b):** assert the staged `pandoc --version` **≥ 2.17** (older versions silently ignore `--sandbox`); name `--resource-path` restriction as a mandatory flag alongside `--sandbox`; the corpus sentinel (`$include(sentinel)` / `.. include:: /etc/passwd` under `--sandbox` inside the egress-deny window → output must NOT contain the sentinel) is G31. **LibreOffice (T1 macro / T9b, the previously-ungated control):** parse the shipped `registrymodifications.xcu` (xmllint / `quick-xml`) and assert `org.openoffice.Office.Common/Security/Scripting/MacroSecurityLevel = 3` + `DisableMacrosExecution = true`, `LinkUpdateMode = 0`, and the Calc external-data / external-reference / `WEBSERVICE()` refresh-on-load keys are disabled (§3.5.2) — a profile-existence check is insufficient; LibreOffice headless executes embedded Basic/VBA `AutoOpen`/`Workbook_Open` macros by default unless the profile blocks them (an RCE path on untrusted office files) | release |
| **G38b** | Copyleft corresponding-source bundle present | the §6.1.3 carve-out ii/iii **bundle-presence** assertion: for the static image-worker (LGPL §6) ship its complete corresponding source + LGPL object files / relink recipe, **and** because it links GPL x265 ship the **x265 GPL §3 complete corresponding source + written offer** — the stage step **fails the build if the source bundle is missing**. Maps to the §5 **T6** row | release |
| **G39** | Checksums + minisign | per-file SHA-256 + minisign detached signature **over `SHA256SUMS`** (pubkey at `docs/minisign.pub`, private key = `MINISIGN_SECRET_KEY` CI secret, rotation policy §6.2.3). **`minisign -Sm SHA256SUMS`** is the actual step — **provision the key AND wire the step**. This is the **only** signing in scope (§6.2.3) — **not** binary code-signing/notarization (SSOT *Out of Scope*) | release |
| **G55** | Auditable Rust binary | build the shipped Rust core with **`cargo auditable build --release`** so the dependency list is embedded in the binary (~4 KB, zero CI cost) — a portable, no-auto-update artifact can be audited **from the binary alone** long after a CVE drops (`cargo audit bin` / grype consume it); a strong fit for an offline "audit it yourself" MIT product. **G55 + G17b are the two halves of the offline "audit-it-yourself" story** (embedded SBOM in the binary + the dated open-CVE report) | release |
| **G41** | Artifact size budget | per-platform compressed artifact ≤ budget (≤400 MB target) — measured after bundle | release |
| **G42** | Offline-egress: active-deny + observe-the-attempt | mirror spec §6.7.3 — an **OS-level egress-DENY window** (the enforcement) **plus** a packet monitor (the proof) on each platform: **Linux** netns/nftables drop with `iptables -j LOG`/`NFLOG` + an `strace -e trace=network` `connect()`/`getaddrinfo` leg (a *blocked-but-attempted* connection is caught, so a silent DROP can't make "zero packets" prove nothing); **macOS** `pf` `block log` → `pflog0` read by `tcpdump`; **Windows** outbound firewall block **with** dropped-packet logging via a **named ETW consumer** (`netsh trace start provider=Microsoft-Windows-Windows-Firewall-With-Advanced-Security … convert` then `Get-WinEvent`, not a vague "ETW"). **Windows loopback gap (explicit):** Windows Firewall outbound rules do **NOT** apply to loopback (`127.0.0.1`/`::1`), so if any engine (e.g. LibreOffice headless's local UNO socket server) opens a loopback connection the firewall block silently misses it and the DROP log won't record it — the Windows leg therefore **also** takes a `netstat -an`/ETW socket-STATE snapshot during the window asserting no TCP/UDP socket in `ESTABLISHED`/`CLOSE_WAIT` (or runs inside Windows Sandbox, where loopback is also blocked); a LibreOffice UNO loopback socket is decided as either a suppressed false-positive or a real gap fixed via the hardened profile. The §7.2.3 startup engine smoke probes run **inside the same window**; engine spawns assert `.env_clear()` (no inherited `http_proxy`/`HTTPS_PROXY`/`*_PROXY`/`LD_PRELOAD`/`DYLD_*`) — a structural Semgrep-enforced invariant (G29), not just one unit-tested spawn. Zero egress except the user-initiated releases-page. **macOS WKWebView leg is driver-gapped (§6.7.3)** — core/engine egress is asserted there, the WebView's via §6.6 + static inspection | release |
| **G43** | No-system-pollution audit | a live syscall/fs monitor during a conversion (Procmon/Win · `fs_usage`/macOS · `strace`+inotify/Linux) → no registry/LaunchAgent/daemon/file-assoc writes; writes only to config+log+chosen-output+scratch. **Plus a deterministic before/after STATE snapshot-diff** (positive proof of the negative, complementing the monitor exactly as G42 pairs deny+observe — a live monitor proving a negative is fragile): diff registry hives (Win) / `LaunchAgents`+file-association DBs (macOS) / `~/.local`+desktop-dirs (Linux) before vs after — any new entry fails | release |
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
5. **Gate-catalogue integrity (ONE-directional)** — every gate **named in
   security-concept.md** exists here as a `Gnn` row (the catalogue is the
   **superset**; membership, not phrasing). It is **not** bidirectional: this
   catalogue legitimately defines gates (G3/G5/G6/G10/G19/G22/G23/G27/G28/… and
   many more) that security-concept.md never names because they are
   quality/structural gates, not security controls — a "vice-versa over the whole
   catalogue" check would always fail on a clean checkout and make the P0 exit
   criterion unsatisfiable. The reverse direction, **where it matters** (every §5
   threat row cites a real `Gnn`), is owned by check 8, which is correctly
   bidirectional but scoped to the §5 threat rows.
6. **No forbidden tokens** — banned stamps/strikethrough/stale-dates in doc bodies.
7. **Generated-file structural sanity** — when validating a generated file, parse
   it (not regex) and assert non-empty/well-formed.
8. **§0.11 ↔ §5 threat-map parity (bidirectional)** — every spec §0.11 class
   (`T1, T2, T2a, T2b, T2c, T3, T3a, T4, T5, T6, T7, T8, T9a, T9b, T10` — **15**
   classes since the r2 addition of T3a) has exactly one row in security-concept.md
   §5, and every §5 threat row cites a `Gnn` that exists in this catalogue. Fails the
   build if a class loses its row or a row loses its gate (so the mapping can never
   silently drift).
9. **Inventory parity (membership checks)** — every IPC command `C1..C13` named in
   prose ∈ §0.4.1; every engine id in prose ∈ §3.1; the fixed-set enums
   (`FormatId`, `EngineProgram`, `PatentDisposition`, the error taxonomy, the
   lossy-catalog) are internally consistent across the files that reference them.
   (Brings the linter toward the RMA 10-check depth.)
10. **Generated-security-manifest currency** — if any `04-formats/*.md` is **newer**
    than the generated `ffmpeg-required-decoders.lock` / `ffmpeg-required-encoders.lock`
    (the G38 security-assertion manifests), fail "regeneration needed": a format change
    that updated the spec but left the generated security manifest stale is a
    security-relevant drift. Uses only `git log --format=%ct` (stdlib-only).
11. **Span-bound currency** — the security-vs-quality boundary prose ("every gate
    except G1") never claims a numeric span narrower than `max(Gnn)` in this catalogue,
    so a frozen `G2–Gxx` bound can never silently drift below a newly-added gate.

## 7. Reconciled during P0 review r1

- **Concrete tools picked (closed).** secrets-scanner = **`gitleaks`** (G2);
  CSP/capability-lint = **G47** (`jq`/`serde_json` structural); SAST = `#![forbid(unsafe_code)]`
  + Semgrep packs (`cargo-geiger` informational only, G29); in-core fuzz harness =
  **G48** (`cargo-fuzz` over `crate::detect`, Linux+macOS nightly); per-OS observability =
  G42 (netns/nftables+strace · `pf` block-log+tcpdump · Windows firewall-block+ETW);
  CI hardening = `actionlint` (G49) + `zizmor` (G50) + lockfile integrity (G18a).
- **Every §0.11 class has a verifying gate (closed).** Enforced mechanically by
  plan-lint check 8 (§6); the new gates G47/G48/G38b/G17b close the previously
  runtime-only classes T2/T2a/T2c, the in-core T1 path, T6's source-bundle, and the
  T3 CVE-awareness signal.
- **Living-doc/spec-sync.** Gates added here that the spec only named as `[DEFER]`
  (CSP-lint) or did not name (in-core detector fuzz, the SAST layer, `actionlint`/
  `zizmor`/lockfile-integrity) are reconciled into the spec **in the same change**
  per the SSOT > spec > docs conflict order.

**Still owner-decidable (ratchet plan, not blocking the P0 *design*):**

- [ ] Which L4 gates are *required checks* vs informational on day one, so a
  half-built P1 isn't wedged (G17b/`cargo-geiger` are informational by design).
- [ ] **Required-vs-informational** for the new owner-decidable gates: `cargo-mutants`
  (G15 mutation-testing sub-leg, below), the AFL++/radamsa engine-subprocess fuzz idea
  (§8), and the `cargo-acl`/cackle capability upgrade (§8) — same class as G17b/geiger.

## 8. Reconciled during P0 review r2

- **CI runner-host integrity (G56, new).** The signing secret must never share a host
  with the untrusted Lane-B corpus/fuzz inputs on the shared VPS (spec §6.1.4/§6.7.2);
  a workflow lint fails any secret-using job bound to a self-hosted label, and the
  signing step is bound to an ephemeral GitHub-hosted runner (spec §6.7.2 synced).
- **T3a DLL/dylib side-loading (new §0.11 class).** Per-shared-object SHA-256 verify
  (G37), manifest-diff guard (G35), dynamic-dependency closure (G37b), bundle-only
  `PATH` on Windows. §0.11 → 15 classes (check 8 enumeration updated).
- **G2 secrets gate corrected.** The PEM-rule "catches a minisign key" claim was
  factually wrong (minisign keys have no `-----BEGIN-----` envelope) — a committed
  custom rule + a full-**history** scan leg + `MINISIGN_PASSWORD` added.
- **G35 SBOM tool corrected.** `@cyclonedx/cyclonedx-npm` (npm-only) → `@cyclonedx/cdxgen`
  (native `pnpm-lock.yaml`); mandatory `purl` + SHA-256 `engines.lock` schema fields so
  G17b/G37 match by PURL (not an empty match) and verify a named hash.
- **G37 pin provenance.** The recorded SHA-256 is corroborated against upstream's own
  published checksum/signature at pin time (the xz/liblzma class), the source URL
  recorded; any `engines.lock` SHA edit is a hard Co-Pilot escalation.
- **G38 LibreOffice + pandoc.** Added the LibreOffice `registrymodifications.xcu`
  macro/profile build assertion (the previously-ungated T1 macro RCE control) + the
  pandoc `--sandbox` version-floor ≥ 2.17; corpus sentinels in G31.
- **G32 round-trip de-vacuumed.** Replaced with SOURCE-UNCHANGED + OUTPUT-VALIDITY;
  the literal byte-stable check is scoped to the small truly-invertible set.
- **plan-lint check 5 made satisfiable.** One-directional (catalogue is the superset);
  the reverse where it matters is check 8. New checks 10 (generated-manifest currency)
  + 11 (span-bound currency).
- **New gates this round:** G18c (`.npmrc` registry pin), G18d (`onlyBuiltDependencies`
  lockdown), G35b (SBOM-diff), G36b (frontend GPL/AGPL deny), G37b (dynamic-dep
  closure), G41b (pre-publish archive validity), G56 (CI runner-host integrity).
  Boundary statement de-frozen from "G2–G50" to "every gate except G1".
- **G29 broadened** to `convertia-imgworker` (densest unsafe surface), Semgrep
  offline-vendored, `shellcheck` over all `.sh`, the structural `.env_clear()`/argv
  Semgrep rule, the `#[tauri::command]` path-validation rule. **G4/G14** gained the
  no-panic-sloppiness deny set + exhaustive-match deny. **G48** gained `fs_guard` +
  CSV/TSV fuzz sub-targets + committed crash-corpus replay on all platforms.
- **Owner-decidable mutation gate.** A scoped `cargo-mutants` leg over
  `crate::fs_guard` + `crate::detect` + `crate::outcome` (the no-harm/atomicity kernel)
  is added as a release-tier informational-then-ratcheted gate — line coverage proves a
  line executed, not that a test would CATCH a regression; flagged required-vs-informational
  for the owner like G17b. The §6.4.2 atomicity-under-interruption test injects the kill
  **specifically in the post-`sync_all()`-pre-`rename` window** (a `#[cfg(test)]` fence in
  `crate::fs_guard::atomic_publish` on all 3 OS) so it exercises the §2.1.3 two-state
  invariant at the critical boundary, not an uninteresting pre-sync kill.
- **Living-doc/spec-sync (r2).** The §3.7.2 `engines.lock` schema (`purl` + SHA-256),
  the §3.5.2 LibreOffice profile assertion in §6.1.3, the pandoc version-floor, the
  §2.12.4 in-core-surface reconciliation (it must list the OLE2/CFB read + the bounded
  XML peeks that §1.2/G48 actually cover), the §5 T6→G53 explicit citation, and the
  §6.7.2 signing-runner binding are owned by the spec and synced in the same change.

### Forward ideas (non-blocking; reconcile into the spec FIRST per the conflict rule)

- **`actions/attest-build-provenance` `[DEFER: post-v1]`.** GitHub's free build-provenance
  attestation is the ONE genuinely-free, in-scope-adjacent trust signal for an unsigned,
  no-notarization portable build — it is **not** binary code-signing (so it doesn't breach
  the SSOT out-of-scope line) and is strictly additive to minisign (which proves
  checksum-manifest integrity but says nothing about build ORIGIN). Needs only `id-token:
  write` on the release job (already `contents: write`), produces a Sigstore-verifiable
  attestation (`gh attestation verify`), no key to manage. A concrete non-blocking owner
  decision, reconciled into the spec first.
- **Engine-subprocess coverage-guided fuzz (the highest-risk surface).** Engine-side T1
  (the C/C++ decoders) is covered today only by a fixed fault-injected corpus through the
  §2.12 boundary (G26/G31) — regression samples, not coverage-guided exploration —
  while `cargo-fuzz` (G48) correctly only reaches the in-core Rust detector. So the
  highest-risk surface gets the weakest input generation. Add a **non-blocking CI-tier /
  periodic black-box mutational fuzz of the real sidecar** (AFL++ binary-only/QEMU mode,
  or a radamsa harness feeding the real engine through the isolation wrapper), reusing the
  §2.12 boundary + §6.4.2 oracles (no-crash-escapes-boundary + no-egress +
  no-out-of-input-read). Mark non-blocking (like G17b) so it can't wedge a half-built phase.
- **`cargo-acl` (cackle) capability policy.** Structural upgrade to G18's name-based
  HTTP-client `[bans]`: a per-crate `std::net`/`std::process` capability policy turns the
  T9a "opens no socket" claim from name-based into **behavioural** (a renamed/transitive
  network crate can't slip the list). Linux-only, policy-authoring-intensive — the
  name-based `[bans]` stays the baseline; pairs with `cargo-vet` (who-do-I-trust) +
  cackle (what-can-this-crate-do).
- **Startup integrity tied to the signed `SHA256SUMS`.** The spec is honest that T3
  startup integrity gives no runtime tamper-resistance (an attacker who swaps a binary
  swaps the in-bundle manifest too). A near-free hardening: make the §7.2.3 startup
  verifier read engine hashes from the **minisign-signed `SHA256SUMS`** (or a signed
  subset bundled as a resource) and verify the minisign signature with the committed
  pubkey at startup — it still can't stop a whole-bundle-including-pubkey replacement, but
  it removes the trivial "swap the unsigned manifest too" path and reuses in-scope
  machinery. Owner strengthening, not a blocker.
