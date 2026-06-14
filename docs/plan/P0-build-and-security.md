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
- Build-loop master prompt authored (`docs/process/build-loop.md`) — single-branch, 2-session, **no fix-push cycle** (the canonical term: no push between a fix and its re-review).
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
- **CI supply-chain hardening** (content-independent — lives in `.github/`): every workflow declares top-level `permissions: contents: read`, elevated per-job only where needed (the release job touching `MINISIGN_SECRET_KEY`/`MINISIGN_PASSWORD` gets `contents: write` ONLY); every third-party action pinned by **full 40-char commit SHA** (not a tag), kept current via a `dependabot.yml` github-actions entry (its presence asserted by G56); `actionlint` (G49, L1) + `zizmor` (G50, L4) workflow lint; the secret-bearing release job **never runs on a fork pull-request**; per-PUSH-workflow `concurrency: {group, cancel-in-progress: true}` (never on the release/tag workflow) + explicit `timeout-minutes` per job.
- **CI runner-host integrity** (G56 — content-independent, lives in `.github/`): the secret-bearing signing/release step is bound to an **ephemeral GitHub-hosted runner**, **never** the shared self-hosted VPS that runs the Lane-B untrusted-corpus/fuzz jobs (spec §6.1.4/§6.7.2); a workflow lint fails any secret-using job bound to a self-hosted label, asserts the corpus/fuzz job and the signing job declare disjoint hosts, and the Linux jobs use `step-security/harden-runner`. Implements security-concept principle 11. (Spec §6.7.2 is synced in the same change to bind the signing step to a hosted runner.)
- **Gate-toolchain pinning** (content-independent): every gate/SAST/SBOM/lint tool pinned by exact version AND verified by checksum/image digest at install (vendored binaries / digest-pinned containers over `cargo install`/`npm -g` from a live registry); a committed `rust-toolchain.toml` pins the exact stable channel + a separate nightly for `cargo-fuzz` (G48), asserted not-floating.
- Defense-in-depth plane definitions + fail-open/closed policy wired as config.
- Fastpath / skip detectors (docs-only, check-off) + their self-tests (G10/G24) — the docs-only guard operates on the **range diff over all unpushed commits**, not the HEAD subject.

**Home:** build-gates.md §0–§4 · 06-build-test-release (record the `actionlint`/`zizmor`/token-scope CI hardening in the spec per the living-doc rule).

---

## P0.3 — Content-independent gates (buildable now)

**Goal:** every gate that needs no app code is live on both planes.

**Box areas:**
- Secrets / credential scan — **`gitleaks`** (G2), pinned version, committed `.gitleaks.toml` with a **custom rule for the minisign secret-key shape** (the default PEM rule cannot catch a minisign key — no `-----BEGIN-----` envelope) + the fixture-key allowlist + a baseline; `--staged` at L1, full-tree at L4, and the full-**history** `gitleaks detect` leg at the release tier (no PR-review backstop in the solo model).
- WebView **CSP + capability structural lint** (G47) — `jq`/`serde_json` parse of `tauri.conf.json` csp + `src-tauri/capabilities/*.json`, fails on any §0.10 violation incl. the `tauri-plugin-updater`/HTTP-client/`updater`-block absence (content-independent — the conf/capabilities files exist from P1, but the linter is authored here).
- Conventional-commit + dual-review-trailer (G11/G12) — exact `Dual-Review:` regex at pre-push.
- Deferral / dead-marker gate (G8/G21) — incl. the no-`any` / no-inline-CSS CLAUDE.md markers; L4 mirror is **fail-closed full-tree**.
- `plan-lint` / `spec-lint` doc-consistency linter (G7/G20) — incl. its own unit tests + the §0.11↔§5 parity check (check 8, now **15** classes incl. T3a) + inventory parity (check 9) + the **one-directional gate-catalogue check 5** (catalogue is the superset — the prior bidirectional wording was unsatisfiable on a clean checkout and would block the P0 exit criterion) + **check 10** (generated-security-manifest currency) + **check 11** (span-bound currency, "every gate except G1" never narrower than `max(Gnn)`).
- License + supply-chain policy config (`cargo-deny` `deny.toml` **skeleton**; G18) — `[bans]` updater/HTTP-client deny-list, `[licenses]` GPL/AGPL deny **fail-closed on low-confidence**, `[advisories]` `yanked = "deny"`, `[sources]` allow-list — plus the **lockfile-integrity** wiring (G18a) and the **`cargo-vet`** scaffold (G18b) **with a bootstrap protocol** (`cargo vet init`; import ≥1 trusted DB — Mozilla/Google/ISRG; `safe-to-run` build-only vs `safe-to-deploy` runtime criteria; a new unvetted dep ESCALATES, never silently exempts; P0 exit = clean `cargo vet check` on the initial `Cargo.lock`). *(The G36 SBOM forbidden-family release gate is homed in P0.7 with the other L5 policy — not here.)*
- **JS/WebView supply-chain leg** (config here; contract in P0.4) — committed `.npmrc` registry pin + a resolution-URL guard asserting every `pnpm-lock.yaml` URL ∈ the allowed registry (G18c, the `[sources]` analogue / dependency-confusion defence); a committed minimal `onlyBuiltDependencies` allowlist + a growth lint (+ fail if `enable-pre-post-scripts`/`unsafe-perm` is set) for install-lifecycle-script lockdown (G18d); the frontend GPL/AGPL license hard-fail over the pnpm graph (G36b — `cargo-deny [licenses]` is Rust-only, so the WebView, the entire T2 surface, needs its own).
- Generated-artifact drift-check framework (G19).
- Repo-invariant grep gate scaffold (G9); prose-typo gate (G51, `typos`) + EOL/charset hygiene (G52, `.editorconfig` + `editorconfig-checker`).

**Home:** build-gates.md §2/§4/§6.

---

## P0.4 — Language & build gate contracts

**Goal:** the contract + CI wiring-points for the language gates are defined; full
activation happens when P1 scaffolds the toolchains.

**Box areas:**
- Rust gate contracts: rustfmt, `clippy -D warnings` + the **no-panic-sloppiness deny set** (`unwrap_used`/`expect_used`/`panic`/`indexing_slicing` for `crate::detect`, allow-listed in tests with a `// PANIC:` escape) + the **exhaustive-match deny** on the dispatch enums (G4/G14), `cargo test`, `cargo audit --locked`, `cargo deny`, `cargo-vet`; the **unsafe-policy** primary SAST gate — `#![forbid(unsafe_code)]` at the root of **every first-party crate (core AND `convertia-imgworker`)** + a narrowly allow-listed FFI module + "no new `unsafe` outside it" (G29); **Semgrep** — pinned by version in `requirements-ci.txt` with rulesets **committed under `scripts/semgrep-rules/` at the pinned registry version** (the managed `p/security-audit` pack is fetched live from `semgrep.dev` and breaks offline), so the offline gate uses `p/rust`+`p/typescript`+`p/bash`+`p/python` + the committed project-local rules (the `#[tauri::command]` path-validation rule, the structural `.env_clear()`/argv-safety-flag rule); `shellcheck` over all committed `.sh` gate-scripts/hooks; `cargo-geiger` **informational only**; `cargo fuzz` harness layout (the in-core G48 targets — `crate::detect` + `crate::fs_guard` + the in-core CSV/TSV engine — Linux+macOS nightly, committed crash-corpus replayed on all platforms; NOT the isolated engines, which are the §6.4.2 corpus fault-injection G26). `→ activated in P1`.
- TS gate contracts: `tsc` strict, eslint, `prettier`, vitest. `→ activated in P1`.
- Coverage gates: **per-domain floors (per-crate Rust / per-package TS)** — `cargo-llvm-cov` (Rust branch) AND vitest v8 (TS), fail if ANY below its floor (never averaged); ratchet 50→70 stored in a tracked file (increase-only, no auto-increment), **created at 0% in P0 and enforcing from P1** (annotate `→ activated in P1`); gate scripts excluded from the floors (G24-self-tested); shard-merge determinism (named partials, fixed merge order, floor on the merged report); diff gate ≥80% (G27/G28).
- Lockfile-integrity contract (G18a) — `--locked` / `--frozen-lockfile` + `git diff --exit-code` on the lockfiles.
- Cross-platform build-matrix contract (G30) — native per-OS, macOS universal.

**Home:** build-gates.md §4/§5 · 06-build-test-release (§6.4.2 corpus fault-injection; home the `cargo-fuzz` in-core harness + the SAST/unsafe-policy layer in the spec per the living-doc rule) · 00-architecture (§0.4.5 type-drift).

> **Note:** the JS supply-chain contract (G18c/G18d/G36b — `.npmrc` registry pin + resolution-URL guard, `onlyBuiltDependencies` lockdown, frontend GPL/AGPL deny) is wired here alongside the TS gate contracts; its config skeleton is in P0.3.

---

## P0.5 — Test methodology & harness conventions

**Goal:** *how we write tests* is defined and the cross-cutting test invariants
have a home.

**Box areas:**
- Test-levels doctrine: unit · integration (real files, **never mock the thing under test**) · property · fuzz · E2E · a11y (the §6.4.6a jsdom/`vitest-axe` ARIA leg = G33a, vs the Lane-B `@axe-core/webdriverio` contrast leg = G33b).
- Corpus / fixture conventions — single-source helper, auto-discovery, no inline duplication; **explicit decompression-bomb corpus FIXTURES** (svgz bomb, ZIP-bomb-in-OPC DOCX, deeply-nested PDF flate stream) so the §6.4.2 bomb case is backed by files, not just a property concept.
- **Source-unchanged + output-validity invariant defined (G32 — replaces the vacuous "A→B→A byte-stable / within tolerance"):** (a) SOURCE-UNCHANGED — `sha256` of every corpus source unchanged before/after (the no-harm proof); (b) OUTPUT-VALIDITY — produced output passes a REAL per-format structural check (ffprobe decodable+codec, `vipsheader` decode+dims, poppler opens, `unzip`+`[Content_Types].xml`, CSV field-count parity), not magic-sniff; the literal byte-stable check is scoped to the small truly-invertible set (e.g. PNG→BMP→PNG) with the covered pairs documented.
- Property-test conventions — **language-split, not either/or:** Rust = **`proptest`** (macro-based shrinking, no manual `Shrink` impls — satisfies "shrinking mandatory"); TS = **`fast-check`** (custom `Arbitrary` must delegate to built-in shrinkers — ban `fc.gen()` without a shrink wrapper); **determinism:** pinned CI seed, a case-count floor above the thin default 256, a property failure NEVER retried to pass. The coverage-guided **`cargo-fuzz`** harness is the **separate** in-core target set — `crate::detect` + `crate::fs_guard::resolve_identity`/`is_safe_output` (untrusted paths: null bytes, overlong UTF-8, symlink chains, `..`) + the in-core CSV/TSV engine (gigabyte quoted fields, recursive quoting, NUL) (G48), with the XML peek's DTD/external-entity resolution disabled by construction; **every libFuzzer crash is minimized + committed under `fuzz/corpus/`+`fuzz/crashes/` and the deterministic replay runs the committed crash set on EVERY platform incl. Windows.**
- Flaky-test policy — retry infra/timeout only, E2E-only auto-retry, determinism engineered (pinned locale/timezone, animations off).
- "Build fully, no skeleton/stub" rule wired to the deferral gate (G8).
- Coverage thresholds (per-domain) + shard-merge determinism.
- **Cross-cutting security-test homes (defined here, activated by their phase):** the §7.5 **log-redaction** property gate (feed a secret-looking path stem through the logger, assert absent), the §2.14.1 **temp ownership + mode-bits** assertion (`0o700` scratch root / `0o600` `.part` publish-temp), the **T10 adversarial resource-budget** gate (oversized-render SVG, over-duration to-GIF, over-cardinality batch → fail-clearly, batch continues, no handle/RAM exhaustion), the **T9b corpus sentinels** (`.docm`/`.xlsm`/`.pptm` AutoOpen-macro canary NOT created; `WEBSERVICE()` `.xlsx` no-egress/no-out-of-input), the **T7 INPUT-side symlink/junction** case (resolved-real-path handed to the engine), the **Windows AV-retry** fault-injection (§2.1.2), the **privilege-drop-tier-applied** regression assertion (§2.12.3 silent-degrade can't disable on every run unnoticed), and the **process-group/Job-Object reap** assertion (no orphan/zombie).
- **Atomicity-under-interruption** (§6.4.2): the kill is injected **specifically in the post-`sync_all()`-pre-`rename` window** (a `#[cfg(test)]` fence in `crate::fs_guard::atomic_publish`, all 3 OS) to exercise the §2.1.3 two-state invariant at the critical boundary.
- **Scoped mutation-testing gate** (`cargo-mutants` over `crate::fs_guard` + `crate::detect` + `crate::outcome` — the no-harm/atomicity/no-misroute kernel) as a release-tier informational-then-ratcheted gate (line coverage proves a line ran, not that a test would CATCH a regression there); **owner-decidable** required-vs-informational like G17b.

**Home:** docs/process/test-strategy.md · 06-build-test-release (§6.4 corpus/reliability; record the bomb fixtures §6.4.5 + the §2.14.1 mode-bit pin in the spec per the living-doc rule).

---

## P0.6 — Dual review (holy grail) & commit protocol

**Goal:** the Opus/Sonnet review and the commit discipline are specified and
enforceable.

**Box areas:**
- Dual-review protocol (G1) — staged-diff input, P0–P3 severity, converge/diverge, fix-before-push (no fix-push cycle), skip conditions; **the dual review is a quality amplifier, NOT a security control** (only the deterministic gates — every `Gnn` except G1 — are security controls).
- Review-trace commit trailer + its format gate (G12 — exact `^Dual-Review: opus=(GO|NOGO) sonnet=(GO|NOGO)$` at pre-push).
- **Staged-diff sanity:** before `git commit`, `git diff --cached --stat` must match what the reviewers saw at GO; any post-GO file add/remove forces re-review.
- Commit conventions — Conventional-commit + body (spec-§ + box-id + findings + co-author trailer); solo-on-`main` rollback convention (`chore(scope): roll back — reason`, no `revert` type).
- **Definition-of-Done (the ConvertIA-adapted RMACLAUDE §11, dropping RLS/audit/migrations):** (a) spec-§ referenced or marked tooling-only; (b) spec synced in the same commit; (c) tests at the highest sensible level green; (d) hard gates green with **no** `--no-verify`; (e) dual-review done + trailer; (f) inline `[Build-Session-Entscheidung]` at non-spec choice sites; (g) `engines.lock`+SBOM row if a new engine staged; (h) §0.11+§5 row if a new threat class introduced.
- **Build-loop soundness rules** (the autonomous-direct-to-main model depends on these): **(0) session-start sanity** — correct repo + remote, clean working tree, no half-committed/half-staged state before any box starts. **(1) session-start CI health** — query the last Actions run on `main` (`gh run list --branch main --limit 1 --json status,conclusion`); STOP+escalate on failure/cancelled, proceed on success/pending/queued, fail-open (warn+continue) if the API is unreachable. **(2) push-exit-code wait** — the push is a **foreground `git push` with the exit code captured directly** (NOT piped through `tee`, which masks the hook's non-zero exit; NOT the RMA background marker-file pattern, which solves a parallel-sessions problem we don't have; NOT `pgrep`-polling); on non-zero do NOT proceed to the check-off commit or next box (fix, re-stage, re-review, retry; escalate after **3 consecutive push failures**); never start a new box while a push is unresolved. **(2a) watch own CI run** — because the loop pushes **direct to `main`**, its own push's CI failure IS a red `main`, so after a successful push it **`gh run watch`** the run it just triggered before starting the next box; a concluded failure ⇒ same STOP+escalate as session-start. **(3) next-box selection** — lowest-phase/lowest-box first; `[!extern]`/`[!]`-blocked boxes skipped+reported; after each check-off, scan for `[!]` boxes the just-completed box unblocked and unlock them; on zero open boxes report convergence and stop (never loop forever); escalate on a genuine all-blocked deadlock.
- **Hard-stop / token-Notbremse numbers (decided here, the ConvertIA baselines):** **soft-stop ~8 boxes** (pause + summarize for the owner), **hard-stop ~12 boxes** in one session, **phase-change hard-stop** after ≥1 committed box of a new phase (re-orient at a phase boundary), **3 consecutive gate-red pushes = hard-stop+escalate**. (Adapted from the RMA model; digits are the v1 baseline, tunable.)
- **Spec-contradiction hard-stop:** a **spec-internal contradiction** (two §§ disagree) is an **unconditional hard-stop+escalate regardless of P-severity** — the Build-Loop is downstream of the spec and cannot pick a side.
- **Pattern-lookup before escalating a P0:** grep the codebase/process docs for an established pattern first; escalate only if none exists (so a routine choice with a precedent isn't escalated).
- **Per-box status-line format** (RMA §11): one line per box — `P0.X done — <summary>, gates green, SHA <short>, Review: P0=N P1=N P2=N P3=N` + a separate `Co-Pilot: N item(s)` block for P1+ findings — so session progress + escalation signals are scannable.
- **Conflict rule** (written as a build-loop rule): **SSOT > spec > security/process docs > code > conversation.**
- Escalation rules — when the Build-Loop escalates to the Co-Pilot session; the hard-stop conditions (concrete box-counter / token-notbremse numbers); the "decide-it-yourself" default + tagging (`[Build-Session-Entscheidung]`).
- **Vuln-response runbook** (`docs/process/vuln-response.md`) — because the app ships known-vulnerable-class C/C++ decoders against untrusted files with **no auto-update**, the only path a security fix reaches a user is a new full release: advisory (G17b/upstream) → Build-Loop escalates to Co-Pilot → bump the `engines.lock` pin → re-run the §6.5 reliability gate → new release; matches the `SECURITY.md` "no SLA, best-effort" posture with an actual triage process. **Severity threshold (stated in `SECURITY.md`):** a CVE with **CVSS ≥ 7 on an engine code path ConvertIA actively exercises for a §04 format** → MUST escalate + block the next release until bumped or triaged not-exercised (G17b release-tier rule), so a CVSS≥9 PoC in an actively-exercised FFmpeg/poppler path can't sit shipped for weeks. **Beyond engine CVEs, the runbook also covers:** (a) **own-code (non-engine) vulns** + the coordinated-disclosure intake→embargo→fix→release loop (via the private-advisory template, §6.8); (b) a **signing-key compromise/loss path** — the human-readable "this key is retired/compromised" commit IS the revocation channel for an offline no-phone-home app (an authored artifact), plus an offline encrypted key+passphrase backup/custody note (the §6.2.3 rotation policy is the silent-swap defence; this is the incident path it doesn't cover).

**Home:** docs/process/build-loop.md · docs/process/roles-and-escalation.md · docs/process/vuln-response.md.

---

## P0.7 — Release & supply-chain gate policy (defined here; pipeline built in P10)

**Goal:** the release-plane gate policy + acceptance criteria are defined so P10
only wires them.

> **Plane annotation:** G42 (egress observability) is scoped to **P9** (README line
> 372); G43 (no-system-pollution) to **P10** (README line 413) — they are policy-defined
> together here but built in different phases.

**Box areas:**
- SBOM generation + **completeness** policy (G35) — `cargo cyclonedx` (Rust) + **`@cyclonedx/cdxgen`** (`--spec-version 1.5`) for the frontend — **NOT `@cyclonedx/cyclonedx-npm`**, which is npm-only and cannot read `pnpm-lock.yaml` (it would SBOM an npm-resolved tree diverging from the G18a-frozen pnpm graph and feed G17b the wrong component set); `cdxgen` has native `pnpm-lock.yaml` support. **`Syft` mandatory** for the staged-bundle completeness cross-check, backed by a deterministic stage-tree file-manifest diff (an unexpected — or side-loaded-mismatched, T3a — `.so`/`.dll`/`.dylib` hard-fails); **each `engines.lock` row carries a mandatory `purl` (`pkg:generic/<name>@<version>` min, a CPE where one exists) + a SHA-256** (§3.7.2 schema reconciled in the same change) so G17b matches by PURL (not an empty match) and G37 verifies a named hash; the license hard-fail (G36) forbidden-family policy (moved here from P0.3) + **the frontend GPL/AGPL hard-fail (G36b)** over the pnpm graph; the **SBOM-diff between releases (G35b, non-blocking Co-Pilot-signed)**.
- **Copyleft corresponding-source bundle-present** policy (G38b) — the §6.1.3 carve-out ii/iii assertion (LGPL relink source + x265 GPL §3 corresponding source/offer present or the build fails); maps to the §5 T6 row.
- Engine checksum/integrity build gate (G37 — verify each engine's **AND each staged codec shared object's** SHA-256 against the change-reviewed in-repo `engines.lock` **before staging AND on cache-restore**; **pin-establishment provenance** — a new pin's SHA-256 corroborated against upstream's own published checksum/signature, source URL recorded, any `engines.lock` SHA edit a hard Co-Pilot escalation; manifest cross-check generated after final staging; Linux exec-bit assert) + the **dynamic-dependency-closure assertion (G37b — `ldd`/`otool -L`/`dumpbin`, every non-system dep resolves inside the bundle; Windows bundle-only `PATH`)** + per-engine build assertions (G38, incl. the T2c `tauri-plugin-store`-cannot-escape-`config_dir` assertion, the **LibreOffice `registrymodifications.xcu` macro/profile assertion** — `MacroSecurityLevel=3`+`DisableMacrosExecution`+`LinkUpdateMode=0`+the Calc external-data/`WEBSERVICE()` keys, the previously-ungated T1 macro-RCE control — and the **pandoc `--version ≥ 2.17`** floor so `--sandbox` isn't silently ignored) + the **pre-publish archive-validity leg (G41b — `unzip -t`/`hdiutil verify`/AppImage extract)**.
- Checksums + **minisign over `SHA256SUMS`** policy — the **only** signing in scope (G39); **key provisioned AND the `minisign -Sm SHA256SUMS` step wired**, on an **ephemeral GitHub-hosted runner host-isolated from the untrusted-corpus jobs (G56 — security-concept principle 11; spec §6.7.2 synced)**; **an out-of-band pubkey fingerprint anchor** (a pinned README via the verified GitHub web UI / org page the pipeline can't rewrite) + a G39/G44 sub-assertion that `docs/minisign.pub` matches it (the in-repo TOFU is otherwise circular). *(No `cosign`/SLSA binary-signing — SSOT *Out of Scope*; the former G40 is deleted. GitHub's free `actions/attest-build-provenance` is a concrete non-blocking **`[DEFER: post-v1]`** decision — the one genuinely-free build-ORIGIN signal, additive to minisign, NOT binary code-signing so it doesn't breach the out-of-scope line; needs only `id-token: write`, verified with `gh attestation verify`; reconciled into the spec FIRST.)*
- Bundled-engine CVE-awareness (G17b) — **informational per-push** OSV/grype scan over the **PURL-keyed** `engines.lock` (a bare `(name, version)` matches nothing — a **planted-positive self-test** guards the empty-report-masquerades-as-clean failure), dated open-CVE report as an owner-signed-off release asset; the **CVSS ≥ 7 on an actively-exercised path → release-blocking escalation** rule (vuln-response.md / `SECURITY.md`).
- Auditable Rust binary (G55) — `cargo auditable build --release` so the shipped artifact embeds its dependency list (G55 + G17b are the two halves of the offline "audit-it-yourself" story).
- Artifact size-budget policy (G41).
- WCAG-AA contrast a11y (G33b) — `@axe-core/webdriverio` Lane-B, Linux+Windows, macOS human-walkthrough gap noted.
- Offline-egress (G42 — **active OS egress-DENY + observe-the-attempt**, per-OS tooling incl. the **named Windows ETW consumer + the loopback-socket-state snapshot** since Windows Firewall doesn't cover loopback, `.env_clear()` engine-spawn invariant now Semgrep-enforced G29) + no-system-pollution (G43 — live monitor **plus** a before/after registry/LaunchAgent/file-assoc **state snapshot-diff**) observability gates.
- Governance-completeness + name-clearance gate policy (G44/G45).
- Startup-integrity acceptance (G46).
- **Release-job token scope:** the secret-bearing release job declares `contents: write` ONLY (+ `id-token: write` only if attest-build-provenance is later adopted) and never runs on a fork pull-request (the §5 cross-cutting CI-hardening row).

**Home:** build-gates.md §5 · 03-engines-and-bundling (§3.5.2 LibreOffice profile, §3.7.2 `engines.lock` `purl`+SHA-256 schema) · 06-build-test-release (§6.7.2 signing-runner binding) · 07-app-shell.

---

## Exit criterion for P0

P0 is "done" when: both enforcement planes are live; every content-independent
gate (P0.3 — incl. G2 `gitleaks`, G47 CSP/capability lint, G49/G50/G56 CI hardening)
runs green on both planes; the language-gate contracts (P0.4) are defined with CI
wiring-points; the **seven P0 docs** (six in P0.1 +
the P0.6 `vuln-response.md` runbook):
[security-concept.md](../security/security-concept.md),
[build-gates.md](../security/build-gates.md), `build-loop.md`, `test-strategy.md`,
`roles-and-escalation.md`, `_format.md`, **and `vuln-response.md`**, exist and pass
the doc-consistency gate; the **security-concept §5 threat table is fully populated
vs spec §0.11** (all **15** classes — incl. the new T3a — have a control + a concrete
`Gnn`, enforced by plan-lint check 8); the `cargo-vet` bootstrap is clean
(`cargo vet check` on the initial `Cargo.lock`); the build-loop + dual-review protocol
(incl. the build-loop soundness rules + the hard-stop numbers + the ConvertIA DoD)
are written such that P1's first box can be built strictly through the loop with no
guardrail missing; **AND at least one push to `main` has triggered an L4 CI run that
completed green** (run-log URL recorded in the P0 completion commit) — so the exit
criterion is not satisfiable by a local-only run that never exercises L4 and could
hide a workflow-syntax error G49's local lint misses.
