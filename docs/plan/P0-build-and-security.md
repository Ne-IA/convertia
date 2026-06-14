# P0 — Build & Security System

> **The foundation before the foundation.** P0 establishes the guardrail system
> every later phase (P1–P11) runs under: the security concept, the full gate
> system, the build-loop, the dual review, and the test methodology. It is built
> **before** P1 writes any app code, so P1's very first commit already runs through
> the loop + the gates.
>
> Derives from [security-concept.md](../security/security-concept.md) +
> [build-gates.md](../security/build-gates.md). Index: [plan/README.md](README.md).
> Box format: [`_format.md`](_format.md) (the contract `plan-lint` enforces).
>
> **P0 is bootstrapped MANUALLY** (Co-Pilot session + owner — DECISION B,
> [CLAUDE.md](../../CLAUDE.md) §2), **not** by the Build-Loop: P0 *creates* the loop,
> the gate system and the dual review every later phase runs under, so the loop's
> `P1`..`P11` scan deliberately excludes `P0.x` ([`_format.md`](_format.md) §6 step 1).
> The dual review (G1) still applies to each P0 box, driven by hand. The boxes below
> are therefore ordered so the manual build can proceed strictly top to bottom: the
> docs (P0.1) come first because every gate cites them, then the two enforcement
> planes (P0.2), then the gates that need no app code (P0.3), then the language-gate
> *contracts* (P0.4 — buildable now, `→ activated in P1` against real code), the test
> methodology (P0.5), the review/commit protocol (P0.6), and the release-plane gate
> *policy* (P0.7 — built in P10).
>
> **Activation convention** (the content-independence rule above, made concrete for
> this file): a P0 box that authors a gate/contract whose *enforcement target*
> (Rust/TS source, `tauri.conf.json`, coverage data, a built artifact, a staged
> engine) does not exist until a later phase is **fully buildable now** and carries a
> `> → activated in P<n>` note — it is **not** a `needs:` on an unbuilt later box. A
> `needs:` annotation is used **only** for a genuine intra-P0 prerequisite (one P0.x
> box that must be `[x]` before another). This keeps the P0/P1 boundary exact (P0
> authors the contract + wiring-point; P1 wires it against the code it scaffolds) and
> keeps every `needs:` target resolvable on a clean checkout (`plan-lint`
> needs-targets-exist, [`_format.md`](_format.md) §7).

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
consistent. These come **first** because every gate box below cites one of them and
the doc-consistency gate (G7/G20) cannot pass until they parse.

- [ ] **P0.1.1** [DOC] Finalize the security concept — threat→control→gate map complete · §0.11 · G7
  > the security-concept.md §5 table maps all **16** §0.11 classes (T1–T11 incl. T2a/T2b/T2c/T3a/T9a/T9b and the r3 T11 macOS-engine-as-first-TCC-accessor) to a control + a concrete `Gnn`; enforced by plan-lint check 8. Verifies internal consistency, not new authorship.
- [ ] **P0.1.2** [DOC] Finalize the gate catalogue `Gnn` rows + the pin-and-verify policy · G7
  > every `Gnn` cited in a later P0 box's **header `· <Gnn>` refs** exists as a `| **Gnn** |` catalogue row (the `plan-lint` reference-resolution invariant, [`_format.md`](_format.md) §3.1/§7); a **reserved** id (G65/G66/G67) — which by the catalogue's own rule is "a `| **Gnn** |` row only when its control is adopted" and is NOT one today — is therefore named **only in a box's prose `>`-note**, never in a header ref (P0.2.11/P0.7.8/P0.7.15 adopt G67/G66/G65 that way); the gate-ID space is gap-free-or-annotated (plan-lint check 22) and the §0 pin-and-verify / fail-closed / structural-parsing policy is stated. Verifies the catalogue [build-gates.md](../security/build-gates.md) is the authoritative superset (plan-lint check 5).
- [ ] **P0.1.3** [DOC] Author the build-loop master prompt — single-branch, 2-session, no fix-push cycle · G1 G7
  > `docs/process/build-loop.md` — the §0 P1..P11 range (DECISION B), §3 selection algorithm, the no-push-between-fix-and-re-review rule. It is the canonical home of the 8-point DoD, the hard-stop numbers, the reviewer rubric and the recorded reviewer-family decision (all authored in P0.6); this box stands the document up so P0.6 fills it.
- [ ] **P0.1.4** [DOC] Author the test-strategy doctrine · §6.4 §6.5 · G7
  > `docs/process/test-strategy.md` — the test-levels doctrine, corpus/fixture conventions and the cross-cutting test invariants P0.5 fills in detail. Stood up here so P0.5's boxes have a home.
- [ ] **P0.1.5** [DOC] Author roles & escalation — Build-Loop ↔ Co-Pilot ↔ owner · G7
  > `docs/process/roles-and-escalation.md` — who follows a `needs:` (DECISION C, not escalation) vs who escalates a block; the escalation triggers P0.6 references.
- [ ] **P0.1.6** [DOC] Author the box-format spec `_format.md` · G7
  > `docs/plan/_format.md` — the four markers `[ ]/[x]/[!]/[!extern]`, the tag taxonomy, sub-boxes, the `needs:`/`unlocked-by:` dependency vocabulary (DECISION C), and the per-phase-file + index convention. This is the contract `plan-lint` (P0.3) enforces, so it precedes the linter; a format change is authored here in the same commit as the `plan-lint` code.

**Home:** docs/security/, docs/process/, docs/plan/.

---

## P0.2 — Gate orchestration framework

**Goal:** the two enforcement planes (local git-hooks L1–L3, CI L4/L5) exist as
empty-but-wired harnesses that the P0.3/P0.4 gates plug into. The gate-tool *pinning
mechanism* (P0.2.1) comes first because every other tool here is installed through it.

- [ ] **P0.2.1** [GATE,CI] Build the pinned-gate-tool fetch-and-verify mechanism + its wrong-checksum self-test · §3.8 · G24
  needs: P0.1.2
  > the §0 pin-and-verify control: a `scripts/install-gate-tools` that downloads each tool at its committed exact version, verifies checksum/image-digest, and ships a **G24 negative self-test — a deliberately-wrong checksum MUST fail the install**. Pin store covers `lefthook`/`gitleaks`/`actionlint`/`zizmor`/`typos`/`editorconfig-checker`/Semgrep/Syft/`cargo-deny`/`cargo-vet`/`cargo-fuzz`/`minisign`/… A committed **gate-tool source allow-list** (per-tool upstream origin) + each checksum corroborated at pin-establishment against the tool's OWN published signature OR ≥2 independent origins (the G37 acquisition-mode discipline applied to gate tools, which both read CI secrets and catch real findings); a checksum edit is a hard Co-Pilot escalation. `pip`-installed tools use `pip install --require-hashes -r requirements-ci.txt` with a planted-positive that a hashless install fails. Commit `rust-toolchain.toml` (exact stable + date-pinned `nightly-YYYY-MM-DD` for G48), asserted not-floating.
- [ ] **P0.2.2** [GATE] Set up the `lefthook` git-hook manager — the L1/L2/L3 plane executor · G54 G24
  needs: P0.2.1
  > install `lefthook` (itself a pinned+checksum-verified tool via P0.2.1 — a poisoned `lefthook` can silently no-op every local hook, the `core.hooksPath`-bypass equivalent, so it carries its own G24 wrong-checksum negative self-test). Author `lefthook.yml` with `parallel` + the perf budgets (L1 soft <10 s, L2 <3 min) and the **chronological plane↔hook mapping: pre-commit = L1, commit-msg = L3 (fires at `git commit`, BEFORE pre-push), pre-push = L2 (fires at `git push`)** — stated so commit-msg is not wired into pre-push or omitted. Empty gate slots that P0.3/P0.4 fill.
- [ ] **P0.2.3** [GATE] Build the gate-plane integrity assertion — hooks-installed + not-redirected + two-plane parity + push-from-stale-base · G54 G24
  needs: P0.2.2
  > `lefthook install` is mandatory after clone (no local protection without it); G54 resolves the **EFFECTIVE** hooks dir via `git rev-parse --git-path hooks` and asserts `core.hooksPath` is unset/lefthook-managed (a redirect silently disables ALL local hooks without `--no-verify`); the two-plane parity (CI re-invokes the same hooks); and the push-from-stale-base guard `git merge-base HEAD origin/main == origin/main`. G24 positive+negative self-test.
- [ ] **P0.2.4** [CI] Stand up the GitHub Actions CI skeleton (L4) — clean-checkout matrix · §6.7.1 · G25
  > the L4 clean-checkout job matrix (Win/macOS/Linux) that mirrors L1–L2 (G25) + holds the empty heavy-gate slots P0.3/P0.4 fill. Top-level `permissions: contents: read`; per-job `timeout-minutes`; per-PUSH-workflow `concurrency: {group, cancel-in-progress: true}` (never on the release/tag workflow).
- [ ] **P0.2.5** [CI] Stand up the release-workflow skeleton (L5) — tag-triggered, empty gate slots · §6.7.2 · G58
  needs: P0.2.1
  > the `v*`-tag-triggered Lane-B skeleton with empty L5 acceptance-gate slots (P0.7 policy → P10 fill). No secret read in the skeleton; the secret-bearing job is host-bound + token-scoped in P0.2.7/P0.2.10. `needs:` P0.2.1 — like every CI workflow it installs its gate tooling through the pinned fetch-and-verify mechanism (the cluster goal: "the gate-tool pinning mechanism (P0.2.1) comes first because every other tool here is installed through it"), so the dependency is explicit and `plan-lint`-detectable rather than left to document order.
- [ ] **P0.2.6** [CI] Wire the CI supply-chain hardening — token scope, SHA-pinned actions, `dependabot.yml`, concurrency · §6.7.2 · G49 G50 G56
  needs: P0.2.4
  > every workflow declares top-level `permissions: contents: read`, elevated per-job only where needed; every third-party action pinned by **full 40-char commit SHA** (not a tag); a `dependabot.yml` covering **github-actions + cargo + npm + pip** (presence asserted by G56); per-PUSH-workflow `concurrency: {group, cancel-in-progress: true}` (never on the release/tag workflow) + per-job `timeout-minutes`. **`id-token` scope sub-rule:** workflow-level `permissions:` sets `id-token` absent/`none`, `id-token: write` ONLY on the release/attestation job (zizmor's excessive-permissions check targets `GITHUB_TOKEN`, not id-token). **`pull_request_target` safe-handler sub-rule:** no such workflow without an Environment-protection gate OR a handler that never checks out / runs fork `head_ref` code. **Cache-hygiene sub-rule:** the secret-bearing job restores no writable shared cache, fork-PR workflows share no cache-key namespace with release/`main`, the signed artifact builds `--locked`/`--frozen-lockfile` cache-cold-or-verified. **Secret-in-log sub-rule:** jq/yq-parsed assertion that the secret-bearing job has no `set -x`, echoes no secret-named env var, `::add-mask::`-wraps secret-derived values + a release-tier post-run log scan for the G2 secret SHAPES.
- [ ] **P0.2.7** [CI] Wire CI runner-host integrity — ephemeral hosted signing runner, host-disjoint from the corpus/fuzz VPS · §6.7.2 · G56
  needs: P0.2.6
  > the secret-bearing signing/release step is bound to an **ephemeral GitHub-hosted runner**, **never** the shared self-hosted VPS that runs the Lane-B untrusted-corpus/fuzz jobs (§6.1.4/§6.7.2). The workflow lint fails any secret-using job on a self-hosted label, asserts the corpus/fuzz job and the signing job declare disjoint hosts, and asserts the GitHub-hosted signing job uses `step-security/harden-runner` (BLOCK mode) — asserted ONLY on GitHub-hosted jobs (its free tier is hosted-only; the self-hosted VPS's egress enforcement is G42/G42b + the VPS allowlist + an ephemeral low-priv runner). Implements security-concept principle 11; spec §6.7.2 synced in the same change.
- [ ] **P0.2.8** [CI] Wire the branch-protection / required-status-checks config assertion · §6.7.1 · G56a
  needs: P0.2.6
  > in the single-branch direct-to-`main` model the only thing that turns a red CI run into a real block is GitHub repo config — invisible to the codebase, silently relaxable in the UI. A scheduled + per-push CI step queries the ruleset/branch-protection API for `main`, failing if the required status checks are not all present-and-required, or `allow_force_pushes`/`allow_deletions`/admin-bypass is on; **plus** native secret-scanning + push-protection enabled, default workflow permissions read-only, the T2-taint OR-gate (`code-scanning/default-setup` for `javascript-typescript` OR the G29 rule-(i) Semgrep taint ruleset — machine-enforced as an XOR by plan-lint check 21), and `required_signatures` on `main` (the loop signs its own commits with the P0.7-provisioned SSH key). **Fail-soft during the P0 bootstrap box, then hard.** Spec §6.7.1 synced. (Exempt from the plan-lint check-16 fixture requirement — API-introspection gate, owner-verified instead.)
- [ ] **P0.2.9** [CI] Wire the release-tag (`v*`) trust assertion — the workflow half · §6.7.2 · G56b
  needs: P0.2.5
  > the release-workflow skeleton's **first step asserts the tagged commit is an ancestor of `origin/main` AND main's required checks were green for that exact SHA** (`git merge-base --is-ancestor` + `gh api …/commits/<sha>/check-runs`) and aborts **before any secret is read** otherwise; the `v*` tag must be a signed annotated tag and the step runs `git verify-tag` against the committed SSH allowed-signers file (leg 3 — provisioned in P0.7). G56a guards only the `main` branch ref, not `v*`; legs 2/3 here are fail-closed always. (The tag-protection ruleset POLICY half + allowed-signers provisioning are P0.7.)
- [ ] **P0.2.10** [BUILD,CI] Pin the build toolchain that touches the minisigned bytes · §3.8 · G37 G56
  needs: P0.2.1
  > the trust boundary is "everything that touches the bytes that get minisigned": digest-pin the CI base image/container; version/digest-pin the C/C++ toolchain + Tauri CLI/bundler; if engines are from-source, the source tarballs are hash-pinned + the build container digest-pinned (§3.8). **`cargo-fuzz` invokes the date-pinned nightly EXPLICITLY** (`cargo +nightly-YYYY-MM-DD fuzz`), never a bare `+nightly` (cargo-fuzz's internal `+nightly` does not necessarily honour `rust-toolchain.toml`'s channel) — asserted by an actionlint/workflow-lint check that the fuzz job names the date-pinned channel.
- [ ] **P0.2.11** [CI] Add the OSSF Scorecard informational corroboration workflow · tooling-only
  needs: P0.2.4
  > the reserved id **G65/G66/G67 are NOT catalogue rows** ([build-gates.md](../security/build-gates.md) "Vacated / reserved gate IDs": "A reserved id becomes a `| **Gnn** |` row only when its control is adopted"), so they are named here in prose, never as a header `· <Gnn>` ref — a non-row `Gnn` in the refs position fails `plan-lint` reference resolution ([`_format.md`](_format.md) §3.1/§7), the very check P0 stands up. This box adopts **G67** (OSSF Scorecard corroboration): an informational scheduled `ossf/scorecard-action` (default-branch push + schedule) that independently corroborates the bespoke G50/G56/G56a assertions (branch-protection, pinned deps, dangerous workflows, token scope); a divergence is a Co-Pilot review item, never a required green check (a third-party scanner, not a deterministic in-repo gate). It is `tooling-only` — a scheduled informational corroborator, not a deterministic in-repo gate with a spec-§ home or a catalogue row of its own.
- [ ] **P0.2.12** [GATE] Define the defense-in-depth planes + fail-open/closed policy as config · G25
  needs: P0.2.2, P0.2.4
  > the L0–L5 plane definitions (security-concept §3) wired as config: which plane each gate runs on, the mirror policy (local + CI), and the fail-open/closed default (fail-closed; the explicitly-listed fail-open gates only because another plane guarantees enforcement). The empty-but-wired contract P0.3/P0.4 gates slot into.
- [ ] **P0.2.13** [GATE] Build the fastpath / docs-only skip detectors + their self-tests · G10 G24
  needs: P0.2.2
  > the docs-only / cheap-commit skip detectors (script naming convention `test-*-fastpath-pattern`) — the docs-only guard operates on the **range diff over all unpushed commits**, not the HEAD subject; the G2 L2 range leg is excluded from the docs-only skip. Each ships a G10 positive+negative self-test. **The L4 clean-checkout mirror runs EVERY custom gate-script's planted-positive self-test as a prelude on every run** (the continuous-armed-canary pattern, complementing plan-lint check 16), so a pinned-tool bump that breaks a stable script's parsing is caught.

**Home:** build-gates.md §0–§4 · 06-build-test-release (record the `actionlint`/`zizmor`/token-scope CI hardening in the spec per the living-doc rule).

---

## P0.3 — Content-independent gates (buildable now)

**Goal:** every gate that needs no app code is live on both planes (L1/L2 local +
L4 mirror). These are the boxes that protect P1's very first commit. Each is
fully buildable now; a box whose *target* file lands in P1+ (the CSP conf, the
codegen output) is authored here and carries a `> → activated in P<n>` note.

- [ ] **P0.3.1** [GATE] Build the secrets / credential scan — `gitleaks` + the minisign-key custom rule, four legs · G2 G24
  needs: P0.2.2, P0.2.4
  > pinned `gitleaks`; committed `.gitleaks.toml` with a **custom rule for the minisign secret-key shape** (the default PEM rule cannot catch a minisign key — no `-----BEGIN-----` envelope) + the fixture-key allowlist + a one-time baseline. **All THREE real CI secrets named, provably caught, planted-positived:** `MINISIGN_SECRET_KEY` (custom rule), `MINISIGN_PASSWORD` (entropy + env-name scan + a banned `MINISIGN_PASSWORD=<value>` literal), `ANTHROPIC_API_KEY` (`sk-ant-…`, the reviewer-API secret — gitleaks' bundled Anthropic rule + a planted-positive). **Current subcommands** (v8.19+ deprecated `protect`/`detect`): `gitleaks git --staged` at L1; `gitleaks git @{u}..HEAD` range leg at L2 (the last local catch before a secret goes public — excluded from the docs-only fastpath; the `@{u}`-unset fallback chain `→ origin/<branch> → origin/main → origin/HEAD → HEAD` with a G24 first-push self-test); `gitleaks dir` full-tree at L4; full-**history** `gitleaks git` at the release tier on a nightly `on: schedule:` trigger, its CI job `actions/checkout` with `fetch-depth: 0` (a default shallow checkout silently scans only the tip; a G56/G49 sub-assertion asserts it). Allowlist/baseline growth-lint (box-id + Dual-Review note + content-fingerprint that rotates); `.gitleaks.toml` + baseline join the L(-1) security-critical-file set. CI uses the pinned, vendored, checksum-verified `gitleaks` BINARY (not `gitleaks/gitleaks-action`, which requires a commercial license for org-owned repos).
- [ ] **P0.3.2** [GATE] Build the WebView CSP + capability structural lint · §0.10 §7.8.2 §7.6 · G47 G24
  needs: P0.2.2
  > `jq`/`serde_json` parse (not regex) of `tauri.conf.json` `app.security.csp` + `src-tauri/capabilities/*.json`, asserting **structural equality per directive against the literal locked §0.10 CSP object** (the permitted non-`'self'` tokens recorded per directive: `connect-src` = `{ipc:, http://ipc.localhost}`, `script-src` = `'self'`, `img-src`/`media-src` no `asset:`, `object-src`/`frame-src`/`frame-ancestors` = `'none'`, …). Fails on: any capability `fs:`/`http:`/`shell:allow-execute`/`opener:`-prefix/`dialog:` grant; any `updater`/bundle-updater block or updater pubkey; `dangerousRemoteDomainIpcAccess`/`assetProtocol.enable`/`bundle.createUpdaterArtifacts` present; the three release-hardening keys `withGlobalTauri`/`dangerousDisableAssetCspModification`/release-profile `devtools`; the `index.html` `x-dns-prefetch-control:off` meta; no `plugins.deep-link` block + no custom URL-scheme in any `Info.plist`/`.desktop`/`.reg` under `src-tauri/`; `app.windows[].url` resolves to a local/bundled URI (fails any `http(s)`). The conf/capabilities + `index.html` shell land in P1; the asserted-present policy + the linter are authored now.
  > → activated in P1 (the conf/capabilities/`index.html` targets exist from P1; the linter fails-open/skip-with-warning until then, fail-closed once present).
- [ ] **P0.3.3** [GATE] Build the conventional-commit + dual-review-trailer gates · G11 G12
  needs: P0.2.2
  > G11 conventional-commit subject regex `^(feat|fix|chore|docs|refactor|test|perf|ci|build)(\(…\))?: .+` at commit-msg (L3), with the solo-on-`main` rollback convention `chore(scope): roll back — <reason>`; G12 the exact `^Dual-Review: opus=(GO|NOGO) sonnet=(GO|NOGO)$` trailer + the findings-block-presence sub-check (a GO/GO trailer on a non-trivial diff carries each reviewer's non-empty findings block) at pre-push (L2). The check-off-commit double predicate (subject `chore(todo): .* (abgehakt|done)` AND md-only diff) is recognised by a shared skip-regex with a G54 planted-positive (the canonical shape is pinned in `build-loop.md`, P0.6).
- [ ] **P0.3.4** [GATE] Build the deferral / dead-marker gate · G8 G21 G24
  needs: P0.2.2
  > diff-based scan for `TODO`/`FIXME`/`unimplemented!`/`todo!`/`unreachable!`/`dbg!`/`println!`/`console.log`/`": any"`/`as any`/inline `style=`/`compile_error!` **+ the broadened semantic-deferral vocabulary** ("later"/"for now"/"temporary"/"not yet"/"will add"/"comes in P<n>"/"deferred to"/"once <phase>"/"currently absent") — the phrasings a phase-sequenced loop writes instead of a literal `TODO` (a prior-project audit which found 308 untracked gaps). Suppressed only by a `[Build-Session-Entscheidung: box-id]` tag within ±6 lines; the `[!extern]` suppressor is restricted to `.md` under `docs/plan/` (a G24 negative self-test plants `[!extern]` in a `.rs` comment beside a deferral and asserts it STILL FAILS). L1 new-marker-only, **fail-open** if no diff base; the L4 mirror (G21) is **fail-closed full-tree**.
- [ ] **P0.3.5** [GATE] Build the `plan-lint` / `spec-lint` doc-consistency linter (checks 1–24) + its base-case meta-check · §0.4 §0.11 · G7 G20 G24
  needs: P0.1.1, P0.1.2, P0.1.6, P0.2.2
  > a single **stdlib-only** script (exit `0` none / `1` ≥1 finding / `2` target missing; `--check`/`--json`/`--quiet`/`--max-per-check`), call-sited at L1 (`--quiet` on docs glob), L2 (full), L4. Authors the format-specific checks (`_format.md` §7: markers/sub-boxes/header/tags/refs/needs-targets/annotation-pairing/numbering) + the doc-wide checks 1–24 — incl. check 5 (one-directional gate-catalogue superset), check 8 (§0.11↔§5 16-class parity, read ONLY from the §5 table + the canonical list, never the §7 frozen snapshot), check 9 (inventory parity, IPC as a SET incl. C2a/C2b/C13 + the `QuarantinedByOs` kind), checks 10–13 (manifest currency / span-bound / IPC-surface drift / plugin-surface drift), checks 14–15 (DoD-list + hard-stop-number parity — `→ activated` once `build-loop.md` is filled in P0.1.3/P0.6), check 16 (planted-positive coverage of every fail-closed §5 gate + the ≥N-fixtures-with-evasion-variants rule for the load-bearing custom gates, API-introspection gates exempted), checks 17–23 (forward-idea status agreement / named-procedure presence / reviewer-rubric presence / reviewer-family decision / T2-taint XOR / gate-ID gap-freeness / ratchet decision-log), **check 24 (the P0-completion record format gate — once `docs/process/p0-completion.md` exists (P0.6.10), assert its recorded L4 run-log URL matches the GitHub Actions run-URL pattern `https://github.com/Ne-IA/convertia/actions/runs/<id>`; a `2`-target-missing exit while the file is absent during the P0 bootstrap, fail-closed once it lands — so the P0 exit criterion's durable-record claim is machine-verifiable, build-gates.md §6 r7)**. Ships its own unit fixtures + the **base-case golden-fixture-that-MUST-exit-1** run in the L4 self-test prelude.
- [ ] **P0.3.6** [GATE] Build the `cargo-deny` `deny.toml` supply-chain skeleton + `cargo-vet` bootstrap · §3.8 · G18 G18a G18b G24
  needs: P0.2.1, P0.2.2
  > `deny.toml`: `[bans]` updater/HTTP-client deny-list (`tauri-plugin-updater` + `reqwest`/`ureq`/`hyper`/`isahc`/`curl` **+ `tauri-plugin-http`**) **+ a deny-all-except-allowlist for every `tauri-plugin-*`** (only the §0.10-granted set {single-instance, dialog, opener, store, log} — closes the `cargo add`→G47 window before a capability JSON exists, plan-lint check 13); `[licenses]` GPL/AGPL deny fail-closed on low-confidence; `[advisories]` `yanked = "deny"` + a kind-policy for unmaintained/unsound/notice; `[sources]` allow-list. **G18a** lockfile-integrity wiring (`--locked`/`--frozen-lockfile` + `git diff --exit-code` on the lockfiles incl. `imports.lock`). **G18b** `cargo-vet` scaffold (`cargo vet init`; ≥2 distinct import sources REQUIRED — Mozilla + Google baseline; `imports.lock` COMMITTED + `cargo vet check --locked` offline; no `cargo vet update`/`sync`/`import` in any workflow/`scripts/` file — G9 invariant (e); a new unvetted dep ESCALATES). P0 exit = clean `cargo vet check` on the initial `Cargo.lock`.
  > → activated in P1 (the workspace `Cargo.toml`/`Cargo.lock` are scaffolded in P1; the skeleton + bootstrap protocol are authored now, the clean-`cargo vet check` exit gate runs once the lockfile exists).
- [ ] **P0.3.7** [GATE] Build the core-crate forbidden-dependency gate (T6) · §3.6 · G53 G24
  needs: P0.3.6
  > a `cargo-deny [bans]` workspace-member-scoped rule asserting the **core crate's** dependency closure does NOT contain the image-worker-only C libs (`libvips`/`libheif`/`librsvg`/`libimagequant`) — the build-time analogue of "LGPL must not link into the MIT core" (§3.6); fall back to a `cargo metadata` dep-tree walk only if workspace-member scoping is unavailable in the pinned `cargo-deny`. Ships a **G24 negative self-test fixture** (`tests/g53-fixture/` where the core crate is given an image-worker dep and `cargo deny … check bans` MUST exit 1), registered in `scripts/gate-selftests/` so plan-lint check 16 is satisfiable for G53.
  > → activated in P1 (needs the workspace member graph; rule + fixture authored now).
- [ ] **P0.3.8** [GATE] Build the JS/WebView supply-chain config — registry pin, lifecycle-script lockdown, frontend license deny · G18c G18d G36b
  needs: P0.2.2
  > committed `.npmrc` registry pin + a resolution-URL guard asserting every `pnpm-lock.yaml` URL ∈ the allowed registry (G18c, the `[sources]` analogue / dependency-confusion defence); a minimal `onlyBuiltDependencies` allowlist + a growth lint (+ fail if `enable-pre-post-scripts`/`unsafe-perm` is set) for install-lifecycle-script lockdown (G18d); the frontend GPL/AGPL hard-fail over the pnpm graph (G36b — `cargo-deny [licenses]` is Rust-only, so the WebView, the entire T2 surface, needs its own). Config here; the TS gate contract that consumes it is P0.4.
  > → activated in P1 (the `pnpm-lock.yaml`/pnpm graph land in P1; config authored now).
- [ ] **P0.3.9** [GATE] Build the generated-artifact drift-check framework · §0.4.5 · G19
  needs: P0.2.2
  > regenerate Tauri→TS bindings (`tauri-specta` + `specta`) / CLI `--help` / asset manifest, then `git diff --exit-code` + a structural (parsed, not regex) non-empty sanity check. The concrete codegen command + generated paths are filled in P1 (named so the gate cannot silently pass on a stale file via a wrong invocation).
  > → activated in P1 (the codegen targets do not exist until P1+; framework authored now, activated when the producing code lands).
- [ ] **P0.3.10** [GATE] Build the repo-invariant grep gate — non-empty initial list + self-tests · §3.5.2 · G9 G24
  needs: P0.2.2
  > a committed **non-empty** invariant list (`scripts/repo-invariants.sh`/`g9.toml`, NOT a placeholder) + a G24 positive+negative self-test: (a) no hardcoded colour outside `design/tokens.css`; (b) no `std::process::Command::new` outside `crate::isolation`; (c) no `unsafe impl Send/Sync` outside the FFI module; (d) no raw `127.0.0.1`/`localhost` outside `#[cfg(test)]`; (e) no `cargo vet update`/`sync`/`import` in any workflow/`scripts/` file; (f) no `fc.gen(` outside the approved shrink-wrapper in `*.test.ts`. The **LibreOffice carve-out is precise**: bundled third-party config inside the LibreOffice program tree is excluded via the pinned globs `bundle/LibreOffice/**`, `bundle/LibreOffice.app/**`, `bundle/libreoffice/**` (first-party code stays localhost-banned with no exception); a G24 positive+negative self-test machine-tests the carve-out.
  > → activated in P1 (invariants (a)/(b)/(c)/(f) target Rust/TS source that lands in P1; the list + self-tests are authored now and the doc/script-scoped invariants (e) are live immediately).
- [ ] **P0.3.11** [GATE] Build the prose-typo + EOL/charset hygiene gates · G51 G52 G24
  needs: P0.2.1, P0.2.2
  > G51 `typos` (`typos-cli`, curated-list, near-zero false positives on identifiers) over public-facing prose (`SECURITY.md`/`PRIVACY.md`/`TRADEMARK.md`, the security docs, the minisign verify recipe, the user-string catalog) — a typo in the security policy or verify recipe is a trust-damaging defect G8/G21 miss; G52 a committed `.editorconfig` + `editorconfig-checker` for EOL/charset/final-newline over `.toml`/`.yaml`/`.md`/shell (a CRLF drift in a `.sh` hook is a Windows footgun G3 misses). Three small fixtures.

**Home:** build-gates.md §2/§4/§6.

---

## P0.4 — Language & build gate contracts

**Goal:** the contract + CI wiring-points for the language gates are defined; full
activation happens when P1 scaffolds the toolchains.

These boxes author the **contract + CI wiring-point** for each language gate now;
the gate **activates in P1** when the Rust crate / pnpm workspace it acts on is
scaffolded (the P0/P1 boundary — P0 defines, P1 wires against real code). Each
therefore carries a `> → activated in P<n>` note and is fully buildable now.

- [ ] **P0.4.1** [GATE] Define the Rust lint/format/test gate contract — clippy no-panic + exhaustive-match + audit · §1.2 · G3 G4 G14 G17
  needs: P0.3.6
  > rustfmt `--check`; `clippy -D warnings` + the **no-panic-sloppiness deny set** (`unwrap_used`/`expect_used`/`panic`/`indexing_slicing` for `crate::detect`, allow-listed in `#[cfg(test)]` with a `// PANIC:` escape); the **exhaustive-match deny** (no `_ =>` catch-all on `FormatId`/`EngineProgram`/`PatentDisposition` + the error taxonomy); `#![deny(clippy::arithmetic_side_effects)]` on the IPC-handler module (T10); `cargo test`; **`cargo audit`** plain (NO `--locked` flag — it reads `Cargo.lock` by default and has none; `--no-fetch` for the offline leg) decoupled from the advisory-DB refresh (warn-only). The lint config (`clippy.toml`/crate attributes) is authored as the contract; the CI wiring-point is the L1 diff-scoped + L2 `--all-targets --all-features` leg.
  > → activated in P1 (the Rust crates land in P1; contract + config authored now).
- [ ] **P0.4.2** [GATE] Define the unsafe-policy + Semgrep SAST contract — `deny(unsafe_code)` + the vendored rulesets + project-local rules · §0.10 §2.12 §3.5.2 §3.5.5 · G29 G24
  needs: P0.2.1, P0.3.10
  > the **unsafe-policy primary SAST gate**: `#![deny(unsafe_code)]` at every first-party crate root (core AND `convertia-imgworker`) + a single allow-listed FFI module carrying `#[allow(unsafe_code)]`, with an `#[allow(unsafe_code)]`-appears-on-exactly-one-module check (`#![forbid(unsafe_code)]` is NOT usable on an FFI-bearing crate — un-overridable, would not compile; reserved for a pure-logic zero-unsafe sub-crate). **Semgrep** pinned in `requirements-ci.txt` with rulesets committed under `scripts/semgrep-rules/` at the pinned registry version (offline: `p/rust`+`p/typescript`+`p/bash`+`p/python`+`p/owasp-top-ten`, all vendored) + the committed **project-local rules**: the `#[tauri::command]` path-validation rule + the `name`/`rename_all`-forbidden rule (so plan-lint check 12 is sound); the `.env_clear()`/argv-safety rule (pandoc scratch-only `--resource-path`, LibreOffice no-`--accept` + `-env:UserInstallation` disposable profile, imgworker `MAGICK_CONFIGURE_PATH`-at-bundle-policy); the macOS-T11 `stage_for_tcc`-before-spawn rule; the single-store-name (T2c) rule; rule (g) `std::net`/`tokio::net` allow-list (initially-empty `net-allow-list.txt` + planted-positive); rule (j) raw-socket FFI net-ban (`libc::socket`/`connect`/`nix::sys::socket` + planted-positive); rule (i) the TS/WebView `mode: taint` ruleset (sources = `invoke` inputs + DOM events; sinks = `innerHTML`/`eval`/`Function` + IPC marshalling — the inter-procedural T2 taint, plan-lint check 21's Semgrep leg). `shellcheck` over all committed `.sh`. Each rule ships a planted-positive (plan-lint check 16 ≥N-fixtures-with-evasion-variants).
  > → activated in P1 (the rules target Rust/TS source that lands in P1; rules + planted-positives authored now).
- [ ] **P0.4.3** [GATE] Lay out the in-core `cargo-fuzz` harness contract (G48 targets) + the per-numeric-IPC-arg overflow leg · §1.2 §6.4.2 · G48 G16
  needs: P0.4.1
  > the in-core G48 target layout (NOT the isolated engines — those are G26/§6.4.2): `crate::detect`; `crate::fs_guard::resolve_identity`/`is_safe_output` (incl. the Windows dangerous-path classes — device `\\.\`/`\\?\`, reserved `CON`/`NUL`/`COM1-9`/`LPT1-9` with any extension, drive-relative `C:foo`, UNC, trailing dots/spaces — each a deterministic fixture; + NUL-path and `PATH_MAX`+1 bound-firing fixtures); the in-core CSV/TSV engine; the zip-slip archive-entry-name target (`../../etc/passwd`-entry fixture); each `#[tauri::command]` serde boundary (malformed `serde_json` → structured `Err`, never panic); **per-numeric-IPC-arg arithmetic-overflow `proptest`** (boundary values `u32::MAX`/`i32::MIN`/0/1/2^16-1 → structured `Err`); the imgworker Rust→FFI surface linked against staged libvips/libheif/librsvg, ASAN on (honest note: ASAN covers only the Rust/boundary side of pre-compiled `.so`, not decoder internals — that is G65). Date-pinned nightly; pinned libFuzzer resource bounds (`-rss_limit_mb`/`-max_len`/`-timeout`/`-max_total_time` + G56 `timeout-minutes` — an OOM/timeout is a committed FINDING, never a runner kill); committed crash-corpus replayed on all platforms.
  > → activated in P3 (the real fuzz-target function BODIES — `crate::detect`, `crate::fs_guard::resolve_identity`/`is_safe_output`, the in-core CSV/TSV engine — land in P3; P1 ships only interface-only shells, so the harness has nothing instrumentable until P3. The per-`#[tauri::command]` serde-boundary + per-numeric-IPC-arg overflow legs activate in **P2 (P2.126)** as C1–C13 land, the imgworker-FFI leg in P4; harness layout + the date-pinned nightly contract authored now).
- [ ] **P0.4.4** [CI] Define the `build.rs`/proc-macro execution-isolation contract for the secret-bearing job · §3.8 · G56
  needs: P0.2.7, P0.4.1
  > `build.rs` + proc-macros run arbitrary native code during `cargo build`/`test` in the SAME release job that holds `MINISIGN_SECRET_KEY`, with full network by default, and `cargo-vet` is a TRUST signal not an execution sandbox — so the secret-bearing job runs all `cargo build`/`test` with **`CARGO_NET_OFFLINE=true` AFTER an explicit `cargo fetch --locked`** (a build script then cannot phone home to exfiltrate the key), reinforced by harden-runner BLOCK; asserted by a G56 jq-over-parsed-YAML no-network-after-fetch sub-rule. Honest residual: cargo cannot fully sandbox build scripts the way pnpm blocks lifecycle scripts — the full per-crate cap is the owner-decidable `cargo-acl` contract (P0.4.5).
- [ ] **P0.4.5** [GATE] Record the owner-decidable over-assurance contracts — `cargo-acl`/Kani/`cargo-careful` + `cargo-geiger` · §1.2 · G29 G48
  needs: P0.4.2, P0.4.3
  > the §8 owner-decidable behavioural backstops, recorded as contracts (status tracked in `docs/process/gate-status.md`, plan-lint check 23): **`cargo-acl`/cackle** (a committed `cackle.toml` denying `std::net` to the whole dep graph + `std::process::Command` to `crate::isolation` only — catches a renamed/transitive network crate G18's name-ban and G29 rule (g) both miss; Linux-only build-time graph check; informational-then-required); **`cargo-careful`** in-core wrapper on the Linux+macOS nightly legs (extra std debug assertions + runtime UB checks on the untrusted-byte detect/`fs_guard` path, principle 9); **Kani** bounded model checking to PROVE the small numeric caps (≤100× decompression ratio, `MAX_SVGZ_SNIFF ≤64 KiB`, the `fs_guard` predicates) rather than fuzzer-hoping them; `cargo-geiger` informational only. None replaces G48's fuzz.
  > → activated in P1 (graph/code targets land P1+; the contracts + their initial `gate-status.md` entries authored now).
- [ ] **P0.4.6** [GATE] Define the Principle-11 English-only / string-ownership lint contract · §6.10 · G57
  needs: P0.4.7
  > fail on any locale-switch/i18n-runtime import; every `strings/ui.ts` key resolves to a non-empty English value; user-facing literals live in `strings/ui.ts`. v1 is English-ONLY with NO i18n runtime — the INVERSE of locale-key parity (§6.10 row 23). The config skeleton lives with the TS contract (P0.4.7), so this box `needs:` P0.4.7 — a backward dependency the loop builds first under DECISION C (the shared TS/strings config skeleton must exist before the English-only lint plugs into it).
  > → activated in P1 (the `strings/ui.ts` module is established in P1; contract authored now).
- [ ] **P0.4.7** [GATE] Define the TS gate contract — `tsc` strict / eslint / `prettier` / vitest · §0.4.5 · G5 G6 G13
  needs: P0.3.8
  > `tsc --noEmit` strict (L1 diff-scoped, L2 whole-project); `eslint` (flat config) + `stylelint`; `prettier --check`; vitest. The flat eslint config carries the project-local rules (no `any`, the `fc.gen()`-shrink-wrapper rule paired with G9 invariant (f)); consumes the JS supply-chain config from P0.3.8.
  > → activated in P1 (the pnpm workspace + TS sources land in P1; config authored now).
- [ ] **P0.4.8** [GATE] Define the coverage-gate contract — per-domain floors + the security-crate branch floor + diff gate · G27 G28 G24
  needs: P0.4.1, P0.4.7
  > **per-domain floors** (per-crate Rust via `cargo-llvm-cov`, per-package TS via vitest v8) — fail if ANY below its floor, never averaged; ratchet 50→70 in a tracked increase-only file; a **BRANCH-coverage floor** (`cargo-llvm-cov --branch`, increase-only) for `crate::detect`/`crate::fs_guard`/`crate::isolation` (a guard rejecting `../` but not `..\` is 100% line-covered with only Unix tests); created at 0% in P0; gate scripts excluded from the floors; shard-merge determinism (named partials, fixed merge order, floor on the merged report); diff gate ≥80% on changed lines (G28).
  > → activated in P1 (no coverage data until code exists; floors created at 0% now, enforcing from P1).
- [ ] **P0.4.9** [GATE] Define the lockfile-integrity contract · G18a
  needs: P0.3.6
  > `--locked` / `--frozen-lockfile` on every build/test invocation + `git diff --exit-code` on `Cargo.lock`/`pnpm-lock.yaml`/`imports.lock` (a silent `cargo vet sync`/`update` refresh of `imports.lock` fails on the same push). The CI wiring-point + the offline-tolerant posture are defined here; the contract overlaps the G18b bootstrap in P0.3.6.
  > → activated in P1 (the lockfiles land in P1).
- [ ] **P0.4.10** [BUILD] Define the cross-platform build-matrix contract — native per-OS + macOS-universal `lipo` assertion · §6.1.3 · G30
  needs: P0.2.4
  > native per-OS build; macOS universal **with a per-sidecar `lipo -info` fat-Mach-O assertion** (both `arm64` AND `x86_64` slices present before `tauri build`, since Tauri does not itself `lipo` and a single-arch `*-universal-apple-darwin` sidecar bundles silently then crashes on the other arch, §6.1.3). The matrix shape + the assertion are authored now.
  > → activated in P1 (no buildable artifact until P1 scaffolds the Tauri shell).
- [ ] **P0.4.11** [GATE] Define the schema/membership-parity + "every-X-has-a-Y" completeness gate contracts · §0.4 §6.4 · G22 G23 G24
  needs: P0.2.2
  > the two mirror gates over absent app code that share the **identical** `→ activated in P1` bootstrap annotation their peers G47 (P0.3.2) / G27 (P0.4.8) / G33a / G57 (P0.4.6) carry (named verbatim in their catalogue rows) and so need a P0 home of their own. **G22** schema/membership parity — "every supported format ∈ the README support matrix ∧ has a fixture ∧ has a round-trip test" (a structural set-comparison over the format registry / matrix / corpus, NOT the locale-key parity v1's English-only model excludes — that inverse is G57); **G23** "every X has a Y" completeness — "every `convert_*` command has a test" via a tracking-aware `git ls-files` walk (stage the partner test file in the same commit). Authored as the contract + the L2/L4 wiring-point now; the registry/matrix/fixtures/`convert_*` handlers they scan do not exist until P1, so each **fail-opens / skips-with-warning while the target is absent and fail-closes the moment a registry entry / matrix row / `convert_*` command lands** (the bootstrap annotation, so the P0 green-L4 exit criterion is not blocked and the gate is never silently fail-open once its target exists). Each ships a G24 positive+negative self-test (a format with no fixture / a `convert_*` with no test MUST fail). Distinct from `plan-lint` check 12 (IPC-surface drift, format-matrix-scoped) — these are the SET/completeness gates check 12 explicitly does not subsume.
  > → activated incrementally P3–P7 (the first format-registry entry + `convert_*` handler — CSV→TSV — land in **P3** the walking skeleton, NOT P1; the README support matrix + corpus fixtures fill from P3 then P5–P7 per format; the G22/G23 schema/membership-parity gates fail-closed per format as each registry entry / matrix row / `convert_*` command / fixture lands in P3–P7; the contracts + self-tests authored now, fail-open/skip-with-warning while the target is absent).

**Home:** build-gates.md §4/§5 · 06-build-test-release (§6.4.2 corpus fault-injection; home the `cargo-fuzz` in-core harness + the SAST/unsafe-policy layer in the spec per the living-doc rule) · 00-architecture (§0.4.5 type-drift).

> **Note:** the JS supply-chain contract (G18c/G18d/G36b — `.npmrc` registry pin + resolution-URL guard, `onlyBuiltDependencies` lockdown, frontend GPL/AGPL deny) is wired here alongside the TS gate contracts; its config skeleton is in P0.3.

---

## P0.5 — Test methodology & harness conventions

**Goal:** *how we write tests* is defined and the cross-cutting test invariants have
a home. Most boxes are doctrine/conventions homed in `test-strategy.md` (P0.1.4); the
cross-cutting security-test *homes* are defined now and `→ activated` by their phase,
so a later phase has a named home rather than re-deciding the methodology.

- [ ] **P0.5.1** [DOC] Author the test-levels doctrine — unit/integration/property/fuzz/E2E/a11y · §6.4 · G15 G33a G33b
  needs: P0.1.4
  > the level matrix and the **never mock the thing under test** rule — for a converter "the thing" is the **no-harm/isolation LAYER** (`fs_guard`/`isolation`/`outcome`), tested with a real temp FS + a real isolated subprocess, NOT every engine in every L2 test (engine-INTERNAL behaviour is tested at L4 with real engines, exercised via fast corpus fixtures at L2). The a11y split: the §6.4.6a jsdom/`vitest-axe` ARIA leg (G33a) vs the Lane-B `@axe-core/webdriverio` contrast leg (G33b).
- [ ] **P0.5.2** [DOC] Author the property-test + flaky-test + no-stub conventions · §6.4.2 · G8 G16 G48
  needs: P0.1.4
  > **language-split:** Rust = `proptest` (macro-based shrinking, no manual `Shrink` impls); TS = `fast-check` (custom `Arbitrary` delegates to built-in shrinkers — ban `fc.gen()` without a shrink wrapper, machine-enforced by G9 invariant (f), P0.3.10). Determinism: pinned CI seed, a case-count floor above the thin default 256, a property failure NEVER retried to pass. Flaky-test policy: retry infra/timeout only, E2E-only auto-retry, determinism engineered (pinned locale/timezone, animations off). The "build fully, no skeleton/stub" rule wired to the deferral gate (G8).
- [ ] **P0.5.3** [TEST] Author the corpus / fixture conventions + the bomb + bound-firing fixtures · §6.4.2 §6.4.5 · G16 G48
  needs: P0.5.1
  > single-source helper, auto-discovery, no inline duplication; **explicit decompression-bomb corpus FIXTURES** (svgz bomb, ZIP-bomb-in-OPC DOCX, deeply-nested PDF flate stream); **two deterministic bound-firing fixtures** (gzip exactly 101× → bounded `Err`; svgz exactly `MAX_SVGZ_SNIFF + 1` → sniff stops at the limit) so a removed cap is caught structurally, not fuzzer-hoped. The fixture-set conventions are authored now; per-format fixtures are added by P5–P7.
- [ ] **P0.5.4** [GATE] Build the corpus / crash-fixture integrity gate + the LFS-resolution governance · §6.4.2 · G24a G24
  needs: P0.5.3
  > a committed **SHA-256 manifest of every tracked corpus/crash fixture** (+ the LFS-resolved `corpus-large` objects) verified in CI **before** the corpus runs, plus `git lfs fsck` on the Lane-B leg — these untrusted files are fed to the highest-privilege C/C++ decoders, so a poisoned/swapped fixture or redirected LFS pointer must surface as a manifest diff (a dual-review item), mirroring the `engines.lock` SHA discipline. G24a asserts every manifest path is `filter=lfs`-tracked per the EFFECTIVE `git check-attr filter` and `.lfsconfig` (if present) names only the canonical GitHub LFS endpoint; `.gitattributes` + `.lfsconfig` join the L(-1) security-critical-file set. **Manifest update protocol:** regenerated by the same `stage-corpus` step that adds a fixture, in the SAME commit; G24a fails on a stale manifest (`git diff --exit-code`); a plan-lint sub-check asserts every `fuzz/corpus/`+`corpus-large/` path has a manifest entry.
  > → activated in P1 (the manifest fills as corpus lands in P3–P7; the gate + protocol authored now).
- [ ] **P0.5.5** [TEST] Define the source-unchanged + output-validity invariant (G32) + the determinism sub-assertion · §2.5 §6.4.3 · G31 G32
  needs: P0.5.3
  > (a) SOURCE-UNCHANGED — `sha256` of every corpus source unchanged before/after (the no-harm proof); (b) OUTPUT-VALIDITY — produced output passes a REAL per-format structural check (NOT magic-sniff, NOT bare field-count); the literal byte-stable check scoped to the small truly-invertible set machine-enumerated in `tests/corpus/manifest.toml` (every byte-stable pair + a one-line rationale; G32 fails a listed pair that doesn't round-trip or a pair added without a rationale) + a pure-logic lossy-disclosure property test over the complete `FormatId×FormatId` product (`lossy_disclosure(src,tgt) == is_lossy(src,tgt)`). **Determinism sub-assertion:** same source+settings twice → `sha256(out1)==sha256(out2)`, floor ≥1 pair per engine PER OUTPUT-FORMAT CATEGORY (enumerated in the corpus manifest so plan-lint checks the floor) + a `diffoscope` empty-diff assertion (the reserved G60 tool, localises the non-determinism); known-non-deterministic encoders (VP9/AVIF variable-encode) are documented manifest exceptions.
  > → activated in P3+ (the invariant is defined now; it binds each pair as P3/P5–P7 land them).
- [ ] **P0.5.6** [TEST] Name the output-validity per-format reader conventions (G31/G32) · §6.4.3 §3.5.5 · G31 G32 G38
  needs: P0.5.5
  > the REAL structural readers (ffprobe decodable+codec; `vipsheader` decode+dims; poppler opens; `unzip`+`[Content_Types].xml`; CSV/TSV via a real RFC-4180 reader (the `csv` crate) + CSV-injection literal-preservation, NOT bare field-count) + the non-empty / output≠input / size-plausibility sub-assertions + the **document→image OCR content check** (every PDF→PNG / DOCX→PNG fixture carries an `expected_text` field, OCR-verified with `tesseract … --psm 6` — pixel-variance + size floor pass on a blank/wrong-content raster, so the check is content, not a size floor; OCR scoped to L4 if per-push cost bites) + the **cross-library decode validation** for the headline formats (AVIF/HEIC re-validated with `ffprobe` — a different decoder family; animated WEBP via `ffprobe`, not `dwebp`; the "lacks the decoder" skip evaluated against the committed `ffmpeg-allowed-decoders.lock` golden, NOT a live `ffmpeg -decoders` — a golden-listed decoder absent from the live binary is a G38 hard-fail).
  > → activated in P5+ (the readers bind as each engine's pairs land; conventions named now).
- [ ] **P0.5.7** [TEST] Define the detection-layer KAT convention · §1.2 §6.4.1 · G15 G48
  needs: P0.5.1
  > a committed `tests/detect-kat.toml` pinning canonical files to their exact `FormatId` (one entry per ambiguous case — DOC vs XLS from the same OLE2 magic; detected-but-unsupported; uncertain), read by the G15 unit test so §6.4.1's claim is machine-enforceable at **L2** (a `quick-xml` bump or a detect refactor changing an ambiguous result is caught before L4's corpus). Sits alongside the §1.2 detection-fuzz (G48, P0.4.3).
  > → activated in P3 (the detect framework bootstraps in P3; the KAT convention + an initial entry authored now).
- [ ] **P0.5.8** [TEST] Define the fuzz-crash replay convention — cross-platform stable-toolchain integration test · §6.4.2 · G48 G24
  needs: P0.4.3, P0.5.3
  > every libFuzzer crash is minimized + committed under `fuzz/corpus/`+`fuzz/crashes/`; the deterministic replay is a plain `cargo test` integration test (`tests/fuzz_replay.rs`) feeding every corpus/crash file directly to the target function with NO libFuzzer harness — so it compiles + runs on EVERY platform incl. Windows under the STABLE toolchain (Linux/macOS additionally run the instrumented `cargo-fuzz` nightly leg); a G24 planted-positive asserts a committed crash fixture MUST fail the replay if its fix is reverted.
  > → activated in P3 (the real `crate::detect` + `crate::fs_guard` fuzz-target function BODIES land in P3 — P1 ships only interface-only shells, so the replay harness has nothing meaningful to exercise until P3; convention authored now, `tests/fuzz_replay.rs` stood up by the P3 box after the detect/fs_guard targets exist).
- [ ] **P0.5.9** [TEST] Define the cross-cutting security-test homes — log-redaction / temp-ownership / resource-budget / sentinels / atomicity-under-interruption · §2.1.3 §2.14.1 §6.4.2 §7.5 · G31 G42b
  needs: P0.5.1
  > the homes (activated by their phase): §7.5 **log-redaction** property gate (a secret-looking path stem through the logger → absent); §2.14.1 **temp ownership + mode-bits** (`0o700` scratch root / `0o600` `.part`) + a **Windows ACL leg** (the scratch root grants access only to the current-user SID — explicit restrictive DACL at create / `icacls`) + a **cleanup-on-fault/kill** sub-case + a **Windows AV-lock** sub-case (`ERROR_SHARING_VIOLATION` → retry-after-release or `MoveFileEx(MOVEFILE_DELAY_UNTIL_REBOOT)`); the **T10 adversarial resource-budget** gate + an **output/scratch-BYTE-budget** sub-case (a 1 KB→50 GB intermediate within memory/time budget → `Failed(TooBig)`, batch continues, scratch back to baseline); the **T9b corpus sentinels** (`.docm`/`.xlsm`/`.pptm` AutoOpen canary NOT created; `WEBSERVICE()` `.xlsx` no-egress; a crafted BMP / SVG-via-MSL ImageMagick sentinel — the densest-CVE family, §3.5.5); the **T8 self-feeding / batch-expansion** integration case (§2.4.2/§2.4.3); the **T7 INPUT-side symlink/junction** case; the **Windows AV-retry** fault-injection (§2.1.2); the **privilege-drop-tier-applied** per-run regression assertion (§2.12.3 — the RATCHET itself is G64, P0.7); the **§2.12.3 memory-cap kill** + the **process-group/Job-Object reap** assertions; and **atomicity-under-interruption** — the kill injected specifically in the post-`sync_all()`-pre-`rename` window (a `#[cfg(test)]` fence in `crate::fs_guard::atomic_publish`, all 3 OS) for the §2.1.3 two-state invariant.
  > → activated in P2/P3/P4/P9 (each home binds when its mechanism lands — the §7.5 **log-redaction** property gate in **P2 (P2.127)** where the logging infra lands, fs_guard atomicity/temp-ownership P3, isolation/privilege-drop P4, the egress window + sentinels P9; the homes are defined now so no phase re-decides the methodology).
- [ ] **P0.5.10** [GATE] Define the scoped mutation-testing gate (`cargo-mutants`) + its ratchet · §6.4 · G15
  needs: P0.4.8, P0.5.1
  > `cargo-mutants` over `crate::fs_guard` + `crate::detect` + `crate::outcome` (the no-harm/atomicity/no-misroute kernel) as a release-tier informational-then-ratcheted gate (line coverage proves a line ran, not that a test would CATCH a regression there); owner-decidable required-vs-informational like G17b, recorded in `docs/process/gate-status.md` (plan-lint check 23). **Activation criteria:** the initial informational run outputs a survived-mutant report per kernel crate; the ratchet is a tracked `max_survived_mutants.toml` per crate, initialised at first-run count, decrease-only; the owner flips informational→required when the count reaches **0** for `crate::fs_guard` + `crate::detect`.
  > → activated in P3+ (needs the kernel crates; the gate + ratchet + its `gate-status.md` entry authored now).

**Home:** docs/process/test-strategy.md · 06-build-test-release (§6.4 corpus/reliability; record the bomb fixtures §6.4.5 + the §2.14.1 mode-bit pin in the spec per the living-doc rule).

---

## P0.6 — Dual review (holy grail) & commit protocol

**Goal:** the Opus/Sonnet review and the commit discipline are specified and
enforceable. Most boxes author `build-loop.md` (stood up in P0.1.3); the trailer +
check-off-shape gates were built in P0.3.3 — these boxes specify the protocol those
gates enforce, and plan-lint checks 14–20 (P0.3.5) hold the cross-doc copies honest.

- [ ] **P0.6.1** [DOC] Author the dual-review protocol (G1) — staged-diff input, P0–P3 severity, no fix-push cycle · G1
  needs: P0.1.3
  > staged-diff input (`git diff --cached`, inline — not a SHA), P0–P3 severity, converge/diverge stated explicitly, **P0/P1 → fix in the working tree, re-stage, re-review (no push between fix and re-review)**, P2/P3 → commit body + a follow-up box if structural; skip conditions (check-off commits with no code/config diff; `[!extern]` boxes). **The dual review is a quality amplifier, NOT a security control** — only the deterministic gates (every `Gnn` except G1) are security controls.
- [ ] **P0.6.2** [DOC] Author the reviewer rubric as a committed, drift-guarded fenced block in `build-loop.md` · G1 G7
  needs: P0.6.1, P0.3.5
  > the review PROMPT/RUBRIC the two reviewers receive (what they critique for — spec-conformance, the SPEC-CONTRADICTION-above-P0 class, severity ranking, divergence handling) authored as a **fenced canonical block INSIDE `build-loop.md`** (NOT a separate file, so the "seven P0 docs" exit criterion stays exact), added to the L(-1) security-critical-file set; plan-lint **check 19** asserts the block exists/non-empty, the emitted prompt is sourced from it verbatim, and it carries the canonical SPEC-CONTRADICTION-class + divergence-resolution phrases — so the loop cannot silently edit or under-specify its own review prompt.
- [ ] **P0.6.3** [DOC] Record the reviewer-family decision + spot-audit cadence · G1 G7
  needs: P0.6.2
  > security-concept §4 states Opus+Sonnet share lineage so "both GO, 0 findings" is a CORRELATED signal; record verbatim in `build-loop.md` the owner call (one reviewer a different family + the IDs, OR explicit acceptance of the correlated residual) **plus a concrete spot-audit cadence** (every Nth box / every phase boundary); plan-lint **check 20** asserts the decision is present.
- [ ] **P0.6.4** [DOC] Specify the review-trace trailer, the check-off-commit shape + the staged-diff sanity rule · G12 G54
  needs: P0.3.3, P0.6.1
  > the trailer `^Dual-Review: opus=(GO|NOGO) sonnet=(GO|NOGO)$` + the findings-block-presence sub-check (a GO/GO trailer on a non-trivial diff carries each reviewer's non-empty findings block — the §4 auditable-smell as a machine invariant) — the **gate** is G12 (P0.3.3); this box pins the EXACT strings the loop emits so the shared G12/G54/fastpath skip-regex matches. The **check-off-commit double predicate** (subject `chore(todo): .* (abgehakt|done)` AND a markdown-only diff) with a G54 planted-positive. The **staged-diff sanity** rule (before `git commit`, `git diff --cached --stat` must match what the reviewers saw at GO; any post-GO add/remove forces re-review).
- [ ] **P0.6.5** [DOC] Author the commit conventions + the canonical 8-point Definition-of-Done · G7 G11
  needs: P0.1.3, P0.3.5
  > Conventional-commit + body (spec-§ + box-id + findings + co-author trailer); the solo-on-`main` rollback convention `chore(scope): roll back — <reason>` (no `revert` type). The **canonical 8-point DoD** lives here: (a) spec-§ referenced or tooling-only; (b) spec synced in the same commit; (c) tests at the highest sensible level green; (d) hard gates green with no `--no-verify`; (e) dual-review done + trailer; (f) inline `[Build-Session-Entscheidung]` at non-spec choice sites; (g) `engines.lock`+SBOM row if a new engine staged; (h) §0.11+§5 row if a new threat class introduced. The **8-vs-9 derivation** (prior nine-point DoD minus RLS/audit/migrations plus the two ConvertIA facts) is recorded so the count cannot drift; plan-lint **check 14** holds the G1, P0.6 and `build-loop.md` copies item-count + item-identifier (in-order) identical.
- [ ] **P0.6.6** [DOC] Author the build-loop soundness rules (session-start sanity → push-wait → watch-own-CI → next-box) · G54 G56
  needs: P0.6.5
  > (0) session-start sanity (`git rev-parse --show-toplevel` == ConvertIA root, `HEAD` == `main`, clean tree, the `core.hooksPath`-not-redirected check, the out-of-band-tamper `git diff --exit-code HEAD -- scripts/ lefthook.yml .github/`, the startup `[!]` dep-unlock scan); (1) per-box CI health (`gh run list --workflow <lane-A> --event push --branch main --limit 1`, STOP+escalate on failure, fail-open if unreachable); (1a) gate-currency fetch (its pre-push enforcement is the G54 stale-base guard); (2) the push-exit-code wait via the **background-push + marker-file + synchronous foreground until-loop** (`run_in_background`/`| tee`/`pgrep` FORBIDDEN; the check-off push is NOT exempt; escalate after **3 consecutive push failures**); (2a) watch own CI run SHA-anchored (`gh run watch --exit-status <run-id>` — `--exit-status` MANDATORY; the G56-cancel successor-exists reconciliation; the loop MUST NOT push the check-off commit until the box-commit run completes green); (3) next-box selection (lowest-phase/lowest-box, skip+report `[!]`/`[!extern]`, the `unlocked-by:` auto-unlock scan, convergence-stop on zero open boxes).
- [ ] **P0.6.7** [DOC] Specify the hard-stop / token-Notbremse numbers + box-batching + dual-review availability soundness · G7
  needs: P0.6.6
  > the ConvertIA baselines stated as exact operator-anchored strings (plan-lint **check 15** canonical form): **soft-stop at >= 8 committed boxes**, **hard-stop at == 12**, the **phase-change hard-stop** (top-level phases only, not the P0.x clusters; does NOT fire if the counter is 0), the **P0 cluster-boundary soft-stop at >= 5 committed boxes** (running counter resetting at each soft-stop, firing at the next cluster boundary after crossing 5; `[!extern]`/`[!]`-skipped boxes do NOT increment), **>= 3 consecutive push failures = hard-stop**. Opt-in box-batching (≤3 sister-boxes, no sub-boxes/cross-deps, trivial repetition → 1 build-commit + 1 dual-review over the combined diff + 1 check-off; dual-review fires once per TOP-box). Dual-review availability soundness (the two reviewer model IDs pinned; on error/timeout/5xx retry bounded N then HARD-STOP+escalate — NEVER auto-emit a `GO` with fewer than two live reviews).
- [ ] **P0.6.8** [DOC] Author the named build-loop procedures — crash-recovery + divergence-resolution + gate-quarantine + suppression ledger · G7
  needs: P0.6.6
  > the **crash-recovery procedure** ((a) partial staged → `git reset HEAD` + re-read; (b) committed-but-CI-red → a NEW fixing commit, never amend a pushed commit; (c) pushed-but-not-checked-off → the open-box scan catches it; (d) push idempotent on retry) and the **dual-review divergence-resolution rule** (P0/P1 GO-vs-NOGO → NOGO, the stricter reviewer wins; P2/P3 → a recorded `[Build-Session-Entscheidung]` rationale; a spec-contradiction → unconditional hard-stop+escalate) — both as canonical verbatim phrases plan-lint **check 18** asserts present. Plus the **gate-quarantine procedure** (a provably-misfiring required gate → a COMMITTED, dual-reviewed narrow scope/suppress with a restore box-id, never `--no-verify`; the self-referential `lefthook.yml`-comment-out-then-restore bootstrap escape incl. the plan-lint circularity case) and the **per-finding suppression ledger** (content-derived fingerprint + box-id + Dual-Review note, fingerprint rotates on code change). The spec-contradiction hard-stop, the pattern-lookup-before-escalating-a-P0 rule, the per-box status-line format, the convergence-report content, the conflict rule and the escalation rules are all stated here.
- [ ] **P0.6.9** [DOC] Author the vuln-response runbook (CVE → user, no-auto-update) · §3.8 §6.5 · G17b
  needs: P0.1.5
  > `docs/process/vuln-response.md` — the seventh P0 doc. The app ships known-vulnerable-class C/C++ decoders against untrusted files with **no auto-update**, so the only path a security fix reaches a user is a new full release: advisory (G17b/upstream) → escalate to Co-Pilot → bump the `engines.lock` pin → re-run the §6.5 reliability gate → new release. **Severity threshold (stated in `SECURITY.md`):** CVSS ≥ 7 on an engine path actively exercised for a §04 format → MUST escalate + block the next release. Also covers: own-code (non-engine) vulns + the coordinated-disclosure intake→embargo→fix→release loop; the signing-key compromise/loss path (the human-readable retired-key commit IS the revocation channel for an offline app) + the offline encrypted key+passphrase backup note; and **a confirmed high-severity engine vuln with NO upstream fix yet** (the dominant real-world case) — ranked options: disable the specific decoder via the G38 `ffmpeg-allowed-decoders.lock` allow-list / drop the affected format path / publish a documented mitigation / escalate the disable-vs-ship call to Co-Pilot, tied to a `SECURITY.md` known-issues line.
- [ ] **P0.6.10** [DOC] Stub the P0-completion record + its format schema (the durable L4-green proof) · G7 G20
  needs: P0.3.5, P0.1.3
  > stand up `docs/process/p0-completion.md` with its committed SCHEMA: a single required `run_url:` line recording the **first push to `main` that triggered an L4 CI run completed green** (the P0 exit criterion's durable record — r7: a commit body is overwritable, a tracked file is not), plus the date + the box-state-at-exit summary the convergence report names. The URL field is left a placeholder marker until the genuine green L4 run exists at P0 exit (then filled in the exit-recording commit); `plan-lint` **check 24** (P0.3.5) asserts the recorded URL matches `https://github.com/Ne-IA/convertia/actions/runs/<id>`, so the exit criterion is machine-verifiable rather than discovered missing at bootstrap end. This box stubs the file + its schema now so check 24 has a target shape to validate against; the file joins the L(-1) security-critical-file set (its `run_url` is the attestable P0-done proof).

**Home:** docs/process/build-loop.md · docs/process/roles-and-escalation.md · docs/process/vuln-response.md · docs/process/p0-completion.md.

---

## P0.7 — Release, supply-chain & security-ratchet gate policy (defined here; pipeline built in P10)

**Goal:** the release-plane gate policy + acceptance criteria are defined so P10
only wires them.

> **Plane annotation (G42/G42b are phased in THREE legs — reconciled in r5 so the earlier
> "built in P9" / "activates in P4" / "per-push from P6-P7" statements no longer contradict):**
> G42/G42b have an ENFORCEMENT SUBSTRATE, a per-push PULL-FORWARD leg, and a full
> RELEASE-CONFIRMATION leg, and each activates at a different phase:
> **(a)** the G42/G42b enforcement SUBSTRATE (the `.env_clear()` spawn invariant + the
> `ptrace`/Landlock fs-audit + the egress-monitor harness) **activates with the first engine
> spawn in P4** (the imgworker proof-of-life) — this is the leg that "activates with the first
> engine spawn", NOT the full gate;
> **(b)** the per-push PULL-FORWARD leg (the §6.4.2 adversarial-egress + T9b-sentinel corpus run
> inside G42's egress-deny window in the per-push L4 integration leg, §5 "per-push
> adversarial-egress pull-forward") runs **from P6/P7** as egressing engines (FFmpeg/pandoc/
> LibreOffice) are staged, so a T9b egress regression is caught on the push that introduces it;
> **(c)** the full per-OS egress-DENY window + the armed-window canary + the **release-confirmation
> G42/G42b** are **BUILT in P9** (the offline-egress observability gate, §2.11.4/§6.7.3; README P9 /
> spec §6.7.3 home the release-confirmation gate, so "built in P9" is authoritative for the
> release-confirmation leg).
> G43 (no-system-pollution) is built in **P10** (the Lane-B release pipeline, §6.10 row 21).
> They are policy-defined together here but built/activated across P4/P6-P7/P9/P10. (Phase-name
> references, not line numbers, so an insertion above them cannot silently invalidate the
> cross-ref.)
>
> **Policy-defined-here / executed-elsewhere annotation (for the fill pass):** the
> **per-engine** G38 build assertions are **policy-defined in P0.7** but **EXECUTED in
> P4/P5/P6/P7** as each engine is staged (mirroring the coverage-gate/SBOM-row phasing) —
> so the fill pass must NOT place unfillable "run `pandoc --version` on an absent engine"
> steps in P0. G46 has **no L4-plane-of-its-own in P0** — its enforcement plane is
> "L4 smoke when a build runs the startup verifier in the G31/G42 window (wired in P4) +
> L5 release acceptance"; G42/G42b/G47/G27/G33a are **always-on/fail-closed** but their targets
> (`tauri.conf.json`/capabilities/coverage data/the rendered React tree/an engine to fs-audit)
> do not exist until P1+, so each carries a
> **bootstrap annotation** (fail-open/skip-with-warning when the target is absent in P0,
> fail-closed as soon as it exists in P1+) so the P0 green-L4 exit criterion is not blocked
> by an absent target. (G33a `vitest-axe` is `→ activated in P1` with the rendered React
> tree; **the G42/G42b enforcement SUBSTRATE — `.env_clear()` spawn invariant + the
> `ptrace`/Landlock read-half fs-audit — activates with the first engine spawn in P4**; the
> per-push pull-forward leg runs from P6/P7; the full per-OS egress-DENY window + the
> release-confirmation G42/G42b are BUILT in P9 — see the three-leg breakdown above, so
> "activates in P4" names the SUBSTRATE leg, not the whole gate.)

These boxes **author the policy + acceptance criteria** for the release plane so
**P10 only wires them** (the P0/P10 boundary). They are DOC-policy boxes carrying the
release-plane tags; their gate executes in P10 (or per-engine in P4–P7 per the
plane annotation above). A box that authors a config/schema/doc is fully buildable
now; a box whose gate *runs* against a built artifact carries `> → executed in P<n>`.

- [ ] **P0.7.1** [RELEASE] Author the SBOM generation + completeness policy + the `engines.lock` schema · §3.7.2 §3.6.2 · G35 G35a G35b G36 G36b
  needs: P0.1.2
  > `cargo cyclonedx` (Rust) + `@cyclonedx/cdxgen --spec-version 1.5` for the frontend (NOT `cyclonedx-npm`, npm-only, cannot read `pnpm-lock.yaml`); `Syft` mandatory for the staged-bundle completeness cross-check backed by a deterministic stage-tree file-manifest diff (an unexpected/side-loaded-mismatched `.so`/`.dll`/`.dylib` hard-fails, T3a). The **`engines.lock` schema (§3.7.2)**: each row a mandatory `purl` (`pkg:generic/<name>@<version>` min, a CPE where one exists) + a SHA-256; **bundled security-CONFIG files** (`policy.xml`/`registrymodifications.xcu`/coder/fontconfig) and **bundled FONTS** are first-class rows (SHA-256 + SPDX + source URL; the Liberation OFL-1.1 trap, Carlito/Caladea Apache-2.0, Noto CJK OFL-1.1). The license hard-fail policy (G36 Rust+bundled + G36b frontend pnpm graph) + a **SPDX-expression VALIDATION leg** (the `spdx` crate / `cargo-about`: poppler `GPL-2.0-only OR GPL-3.0-only`, x265/x264 `-or-later` for the LGPL-3.0 libheif host, libaom `LicenseRef-AOMPL-1.0`) + the static-link SBOM leg + the **DERIVED static-link closure (G35a)** for the imgworker static stack + the generated-vs-committed NOTICE parity (every GPL/LGPL/AGPL row has its license text AND a corresponding-source-POINTER line in `THIRD-PARTY-LICENSES`) + the SBOM-diff between releases (G35b, non-blocking).
  > → executed in P10 (rows populated per-engine in P4–P7; the policy + schema authored now).
- [ ] **P0.7.2** [RELEASE] Author the copyleft corresponding-source bundle-present policy · §6.1.3 §3.6 · G38b
  needs: P0.7.1
  > the §6.1.3 carve-out ii/iii assertion — for the static image-worker (LGPL) ship its complete corresponding source + LGPL object files / relink recipe, and because it links GPL x265 ship the x265 GPL §3 complete corresponding source + written offer; the stage step fails the build if the source bundle is missing. Maps to the §5 T6 row.
  > → executed in P10 (the source bundle is assembled at release; the policy authored now).
- [ ] **P0.7.3** [RELEASE] Author the engine-acquisition policy + the engine-source allow-list · §3.8 · G37
  needs: P0.7.1
  > prebuilt-vs-from-source **per engine per platform**: **from-source** ⇒ the signed source tarball verified with `gpg --verify`/`sq verify` against an in-repo PINNED upstream key (committed keyring/fingerprint on the allow-list), recorded in `engines.lock`, a key/fingerprint change a hard escalation (not a bare hash — the xz class) + the **VCS-tag anchor** (PREFER a `git archive` of the signed tag with `configure`/`m4` regenerated locally, or diff the tarball's non-generated sources against the tag; record BOTH the tarball SHA AND the VCS tag/commit) + the digest-pinned toolchain/base-image; **prebuilt** ⇒ corroborate via the upstream signature, else ≥2 independent mirrors OR a distro GPG-signed package + signed repo metadata (a bare hash of one unsigned download is rejected); FFmpeg (unsigned gyan/BtbN prebuilts) gets a named satisfiable anchor. Plus the committed **engine-source allow-list** (per-engine permitted origins; every `engines.lock` source URL AND corroboration-checksum URL ∈ the allow-list, on independent origins).
  > → executed in P4–P7 (anchored per-engine as each is staged; the policy + allow-list authored now).
- [ ] **P0.7.4** [RELEASE] Author the engine checksum/integrity + dependency-closure build-gate policy · §3.5.5 §3.5.2 §3.5.4 §3.4.3 · G37 G37b G37c G38 G41b
  needs: P0.7.3
  > **G37** — verify each engine's AND each staged codec `.so`'s SHA-256 against the change-reviewed `engines.lock` before staging AND on cache-restore (a G56 jq-parsed-YAML sub-rule asserts a verify step precedes `scripts/stage-engines`, with a G24 planted-mismatch self-test; any `engines.lock` SHA edit a hard escalation); static-link reconciliation; Linux exec-bit assert. **G37b** dynamic-dependency-closure (`ldd`/`readelf -d` Linux · `otool -L` AND `otool -l` macOS · `dumpbin /dependents` Windows; every non-system dep inside the bundle). **G37c** Linux glibc symbol-version floor. **G38** per-engine build assertions — the T2c store-cannot-escape-`config_dir`; the LibreOffice `registrymodifications.xcu` macro/profile keys (the T1 macro-RCE control), no-`--accept`, `-env:UserInstallation` disposable profile; poppler `pdftotext`-built-without-network + remote-URI sentinel; pandoc scratch-only `--resource-path` + the **`--version ≥ 2.15`** floor; the imgworker `MAGICK_CONFIGURE_PATH`-at-bundle-policy + the **ImageMagick hardened `policy.xml` / coder-exclusion** (deny `{URL,HTTPS,HTTP,FTP,EPHEMERAL,MVG,MSL,TEXT,LABEL,SHOW,WIN,PLT}` + `@`-indirect-read); the **FFmpeg enabled-decoder allow-list** (`ffmpeg -decoders` == `ffmpeg-allowed-decoders.lock` golden, regenerate-and-diff, an extra decoder a T1-surface hard-fail) + the **configure-flag / whole-binary-license assertion** (assert `--enable-gpl`, HARD-FAIL on `--enable-nonfree`/`--enable-libfdk-aac`, record the configuration line). **G41b** pre-publish archive-validity (`unzip -t`/`hdiutil verify`/AppImage extract).
  > → executed in P4–P7 (per-engine assertions run as each engine is staged — NO unfillable "run pandoc --version on an absent engine" step in P0; the policy + the `ffmpeg-allowed-decoders.lock` golden authored now).
- [ ] **P0.7.5** [RELEASE] Author the checksums + minisign-over-`SHA256SUMS` signing policy + the verify-recipe assertion · §6.2.3 §6.2.4 §6.7.2 · G39 G44 G56
  needs: P0.2.7, P0.2.9, P0.7.1
  > the **only** signing in scope: the `minisign -Sm SHA256SUMS` sign step on an ephemeral GitHub-hosted runner host-isolated from the untrusted-corpus jobs (G56, principle 11). The **release-tier verify-recipe assertion** RUNS the literal `minisign -Vm SHA256SUMS -p docs/minisign.pub` (lowercase `-p` = pubkey FILE PATH; `-P` is the inline-base64 flag and would FAIL on a path) against the just-produced `SHA256SUMS` + `.minisig` + committed pubkey, failing the release on non-zero; an **out-of-band pubkey fingerprint anchor** (a pinned README via the verified GitHub web UI the pipeline can't rewrite) + a sub-assertion that `docs/minisign.pub` matches it. No `cosign`/SLSA binary-signing (SSOT Out of Scope; the former G40 is deleted).
  > → executed in P10 (the sign step runs in the release pipeline; the policy + the verify-recipe + the pubkey-anchor authored now).
- [ ] **P0.7.6** [RELEASE] Author the build-provenance attestation policy (verify-on-runner + offline bundle) · §6.7.2 · G59 G58
  needs: P0.7.5
  > `actions/attest-build-provenance` is a **v1 OWNER DECISION (G59 — DECIDED, not deferred)** — the one genuinely-free build-ORIGIN signal (binds the artifact to runner+workflow+commit, so a silently re-signed release from a poisoned shared VPS is detectable even if the key leaked), additive to minisign, NOT binary code-signing. Needs only `id-token: write` on the release job (scoped to ONLY that job). **VERIFIED, not just generated:** a release-tier step runs `gh attestation verify` against the just-produced artifact on a clean runner, failing on non-zero; the attestation is added to the G58 enumeration; the **Sigstore bundle + a paired `trusted_root.jsonl`** (via `gh attestation trusted-root`) are named release assets so users verify OFFLINE via `gh attestation verify <artifact> --bundle <file> --custom-trusted-root trusted_root.jsonl --repo Ne-IA/convertia`. Recorded in `gate-status.md` (plan-lint check 23) as DECIDED so the §8/catalogue/box statuses agree (plan-lint check 17).
  > → executed in P10 (the attest+verify steps run in the release pipeline; the decision + policy authored now).
- [ ] **P0.7.7** [RELEASE] Author the bundled-engine CVE-awareness policy + the advisory-DB staleness floor · §3.4.3 §6.5 · G17b G17
  needs: P0.7.1, P0.6.9
  > **informational per-push** OSV/grype over the **PURL-keyed** `engines.lock` (a bare `(name, version)` matches nothing — a planted-positive guards the empty-report-masquerades-as-clean failure; the **FFmpeg CPE `cpe:2.3:a:ffmpeg:ffmpeg:<ver>` MANDATORY** + poppler/libheif/libde265/libvips/LibreOffice CPEs because the highest-CVE surface is FFmpeg's internal decoders/demuxers; the planted-positive uses a historical internal-decoder CVE); a dated open-CVE report (recording the advisory-DB age) as an owner-signed-off release asset; the **CVSS ≥ 7 on an actively-exercised path → release-blocking escalation** rule (vuln-response.md / `SECURITY.md`); the release-tier **advisory-DB staleness floor** (`cargo audit --json .database.last-updated` + the OSV/grype DB timestamp against a committed `MAX_ADVISORY_DB_STALENESS` ≈ 7 days — a committed-timestamp assertion, offline-tolerant; shared with G17). Recorded in `gate-status.md` (informational→required, plan-lint check 23).
  > → executed in P10 (runs over the populated `engines.lock`; the policy authored now).
- [ ] **P0.7.8** [CI] Author the scheduled engine-version-currency poller · §3.8
  needs: P0.6.9
  > the reserved id **G66 is NOT a catalogue row** (the "Vacated / reserved gate IDs" rule, [build-gates.md](../security/build-gates.md)), so it is named here in prose, never as a header `· G66` ref that would fail `plan-lint` reference resolution ([`_format.md`](_format.md) §3.1/§7). This box adopts **G66** (scheduled engine-version-currency poller): a scheduled `.github/` workflow polling the upstream engine release endpoints (FFmpeg/LibreOffice/poppler/pandoc/libheif/dav1d) that **opens an issue when a newer version than the `engines.lock` pin exists** — closing the "upstream shipped a security fix but no CVE/OSV advisory exists yet" gap G17b is structurally blind to for a no-auto-update offline app. An issue-OPENER not a gate (cannot wedge the loop), feeds `vuln-response.md`, honours SSOT §3.8 best-effort currency. The `§3.8` ref satisfies the at-least-one-ref rule; `tooling-only` is **not** added (the two are mutually exclusive, [`_format.md`](_format.md) §3.1).
  > → executed in P10 (polls the populated `engines.lock`; the workflow shape authored now).
- [ ] **P0.7.9** [RELEASE] Author the auditable-Rust-binary policy + the SBOM↔embedded-list agreement · G55 G35 G58
  needs: P0.7.1
  > `cargo auditable build --release` so the shipped artifact embeds its dependency list (G55 + G17b are the two halves of the offline "audit-it-yourself" story); a release-tier sub-assertion extracts the embedded list (`cargo audit bin`) and asserts the Rust-component set == the CycloneDX SBOM's Rust components, and runs `cargo audit bin`/grype against the shipped binary (the two halves are independent tools and must be proven to agree).
  > → executed in P10 (runs against the built binary; the policy authored now).
- [ ] **P0.7.10** [RELEASE] Author the third-party-reproducibility delta + the `docs/reproduce.md` rebuild recipe · §6.2.5 §3.8 · G60
  needs: P0.2.10
  > the `diffoscope` delta of the self-compiled Rust-core + WebView layer (NOT the vendored engines) as an informational release asset, PLUS the human half — a committed `docs/reproduce.md` rebuild recipe + a build-environment lock (pinned base-image digest, `rust-toolchain.toml`, Tauri CLI/bundler digest, exact build command, expected per-file SHA-256 of the Rust core) an independent party can follow (G59 proves build ORIGIN, G60 proves build DETERMINISM of the bytes we own). Non-blocking (vendored-engine non-determinism cannot fail it); a Co-Pilot review item.
  > → executed in P10 (the delta runs against built artifacts; the recipe + env-lock authored now, the recipe filled as the toolchain firms up).
- [ ] **P0.7.11** [RELEASE] Author the artifact size-budget + WCAG-AA contrast a11y release policy · §3.9.2 §6.4.6 · G41 G33b
  needs: P0.7.1
  > the per-platform compressed artifact ≤ budget (≤400 MB target, measured after bundle; G41 — the levers are owned in P4, the release *gate* is P10); the WCAG-AA contrast a11y gate (G33b — `@axe-core/webdriverio` Lane-B, Linux+Windows, the macOS human-walkthrough gap noted, recorded in `docs/usability-floor.md`).
  > → executed in P10/P9 (G41 measures the built artifact in P10; G33b runs the Lane-B contrast scan in P9; the budget + the contrast policy authored now).
- [ ] **P0.7.12** [RELEASE] Author the offline-egress + read-half + no-system-pollution observability policy · §2.11.4 §6.7.3 §6.4.2 §2.12.3 · G42 G42b G43
  needs: P0.7.1
  > **G42** active OS egress-DENY + observe-the-attempt (named Windows ETW consumer + process-scoped loopback-socket snapshot + named-pipe enumeration since Windows Firewall doesn't cover loopback; the `.env_clear()` spawn invariant is G29-enforced; **+ the REQUIRED DNS-only sub-assertion** — `tcpdump -i any port 53` Linux/macOS / Windows ETW `Microsoft-Windows-DNS-Client` over the engine PID scope, zero DNS in the deny window, armed-window canary + resolver-cache flush). **G42b** the symmetric READ-half fs-audit (the T9b no-out-of-input-read half, with its OWN armed substrate: `ptrace` via `docker --cap-add SYS_PTRACE` / native-on-VPS / the §2.12.3 Landlock `{input ro, scratch rw}` fallback / the kernel≥5.13 ABI probe / FAIL-CLOSED with the `::error::fs-audit cannot enforce…` annotation / an out-of-input sentinel + a planted-positive; record the §6.1.4 VPS runner kernel version as a prerequisite). **G43** no-system-pollution (live monitor + a before/after registry/LaunchAgent/file-assoc state snapshot-diff).
  > → executed in P4/P6-P7/P9 (the substrate activates with the first engine spawn in P4, the per-push adversarial-egress pull-forward runs from P6/P7, the full per-OS deny window + release-confirmation leg are built in P9 — the three-leg breakdown above; the policy authored now).
- [ ] **P0.7.13** [RELEASE] Author the governance-completeness + name-clearance + release-artifact-completeness meta-gate policy · §6.8 §6.9 §7.2.3 §7.2.4 · G44 G45 G46 G58
  needs: P0.7.5, P0.7.6
  > **G44** governance-completeness (incl. the literal-form minisign-recipe assertion + the parse-checked libfuse2/WebView2/Sequoia prerequisite notes) + **G45** name/trademark clearance-record (`docs/name-clearance.md` present, dated, verdict = clear + the dormant rename-propagation + old-name grep gate). **G58** the release-artifact completeness meta-gate enumerating EVERY required asset (per-OS bundle, `SHA256SUMS`, `.minisig`, SBOM, dated open-CVE report, `NOTICE`/`THIRD-PARTY-LICENSES`, copyleft corresponding-source bundle, measured-sizes asset, `usability-floor.md`, `name-clearance.md`, the §6.5.3 CHANGELOG/release-notes, the G59 Sigstore bundle) — failing if any is missing AND asserting every enumerated asset has a corresponding LINE in the signed `SHA256SUMS`. **G46** startup-integrity acceptance (incl. the `QuarantinedByOs` sub-test — a mocked quarantine spawn-failure asserts the distinct kind, not `EngineMissing`/`BundleDamaged`, §7.2.3/§7.2.4).
  > → executed in P10/P11 (G44/G45/G58 are P10 release-blocking; G46's runtime verifier is wired in P4 and accepted at release; the enumeration + policy authored now).
- [ ] **P0.7.14** [RELEASE] Author the privilege-drop-tier ratchet policy + the `gate-status.md` decision-log · §2.12.3 · G64
  needs: P0.5.9
  > record the achieved §2.12.3 privilege-drop tier per platform in a tracked `privilege-drop-coverage.toml`, **decrease-guarded** like the coverage floor (a commit lowering an achieved tier fails/escalates; raises are deliberate); the schema + the ratchet criteria homed here; owner-decidable required-vs-informational (informational while the tier matrix is filled in P4–P9, required once stable). Author the committed **`docs/process/gate-status.md`** decision-log that records a dated line whenever any informational-then-ratcheted / owner-decidable gate (cargo-mutants, G17b, G64, G65, cargo-acl/Kani/cargo-careful) flips informational↔required — asserted present + status-agreeing by plan-lint **check 23**. The per-platform tier-APPLIED regression assertion stays a G31 leg in P0.5.9 (this box owns the TREND/ratchet).
  > → executed in P9 (the tier matrix fills in P4–P9; the ratchet + `gate-status.md` authored now).
- [ ] **P0.7.15** [RELEASE] Author the engine-subprocess coverage-guided fuzz policy + its scheduled required job · §6.4.2 §6.1.4 · G42b
  needs: P0.7.4, P0.7.12
  > the reserved id **G65 is NOT a catalogue row** (the "Vacated / reserved gate IDs" rule, [build-gates.md](../security/build-gates.md): "A reserved id becomes a `| **Gnn** |` row only when its control is adopted"), so it is named here in prose, never as a header `· G65` ref that `plan-lint` reference resolution would dangle on ([`_format.md`](_format.md) §3.1/§7); the header keeps `· G42b`, a real row. The single biggest coverage asymmetry: engine-side T1 is covered today only by a fixed fault-injected corpus (G26/G31), so **G65** adds a black-box mutational fuzz of the real sidecar — AFL++ binary-only/QEMU mode OR a `radamsa` harness through the §2.12 isolation wrapper (and `zzuf` LD_PRELOAD for LibreOffice headless), reusing the §6.4.2 oracles (no-crash-escapes-boundary + no-egress + no-out-of-input-read via G42b). **Constraint:** the harness MUST use the G37-staged, SHA-256-verified bundled engine binary, NOT a debug build. **CI-host resource bound:** per-job memory/disk/wall-clock bounds via cgroup/`ulimit`/`systemd-run`/`docker --memory` + the G56 `timeout-minutes` so a corpus-induced host OOM/disk-fill is a contained finding, not a shared-VPS outage. **Pre-committed to a REQUIRED SCHEDULED (non-PR-blocking) job:** at minimum a weekly `radamsa`-through-the-isolation-wrapper run that FILES AN ISSUE on a boundary-escaping crash (an issue-opener like G66); the owner decides whether to ALSO make it per-push. Recorded in `gate-status.md`.
  > → executed in P9/P10 (the real sidecars exist from P4–P7; the policy + the scheduled-job commitment authored now, the id reserved so adoption does not renumber).
- [ ] **P0.7.16** [RELEASE] Author the minisign key genesis-and-custody policy · §6.2.3 · G39
  needs: P0.7.5
  > distinct from the rotation/compromise runbook in `vuln-response.md` (which covers key USE): **(a) GENESIS** — the keypair generated air-gapped / off the shared multi-tenant VPS, the secret key + passphrase entered into the GitHub secret (or the `release` Environment) from that off-host generation; **(b) BACKUP** — an offline ENCRYPTED backup of BOTH the secret key AND its passphrase kept off-platform (a single GitHub-secret copy means a deleted secret = permanent inability to sign continuations); **(c) LOSS-RECOVERY** — the decision path for a loss survivable by restoring the backup (same key continues) vs one forcing a rotation event (the retired-key commit in `vuln-response.md` is the revocation channel). The policy doc joins the L(-1) security-critical-file set.
- [ ] **P0.7.17** [CI] Author the release-tag trust policy + provision the SSH allowed-signers / signing key (legs 2/3) · §6.7.1 · G56b G56a
  needs: P0.2.9
  > a GitHub **tag-protection ruleset on `v*`** (asserted via `gh api …/rulesets` on schedule + on tag, fail-soft in the P0 bootstrap box then hard) so only the owner / a protected actor may create a release tag with a minimal bypass-actor list — the tag-ref sibling of G56a's branch-ref protection. **Leg 3 provisioning:** commit a public SSH allowed-signers file + wire `git config gpg.ssh.allowedSignersFile` so the loop signs every `v*` tag (`git tag -s`) and the P0.2.9 first step can `git verify-tag` it; the SAME key signs the loop's own `main` commits (`git config commit.gpgsign true`) so the `main`-ruleset `required_signatures` knob (G56a sub-check (g)) is satisfiable. The allowed-signers file joins the L(-1) security-critical-file set (an edit is a Co-Pilot escalation). Distinct from §6.7.1's requested-not-required DCO sign-off; pairs with the P0.2.9 enforcement half.
- [ ] **P0.7.18** [CI] Provision the `release` GitHub Environment + author the release-job token-scope policy · §6.7.2 · G56
  needs: P0.2.7, P0.7.5
  > the irreversible key-bearing signing action is gated only by the `v*` ruleset + G56b leg-2's in-job ancestry/green-history abort, but leg-2 runs INSIDE a job where the secret was already injected ("abort before read" ≠ "never injected"). Move `MINISIGN_SECRET_KEY`/`MINISIGN_PASSWORD`/the signing-relevant secrets into a **`release` GitHub Environment** with **required-reviewers + a `v*` deployment-branch/tag policy**, and bind the signing job with `environment: release` so the secret is never injected until a human approves (the human-in-the-loop on the one irreversible action). The **G56 `gh api …/environments/release` assertion** (required_reviewers + the `v*` deployment_branch_policy + the `environment: release` binding, fail-soft in P0 bootstrap then hard) is wired in P0.2. **Release-job token scope:** `contents: write` ONLY (+ `id-token: write` only on the release/attestation job); never runs on a fork PR; a G56 jq-parsed-YAML sub-assertion asserts the workflow-level `permissions:` sets `id-token` absent/`none` and `id-token: write` appears ONLY on the release/attestation job.

**Home:** build-gates.md §5 · 03-engines-and-bundling (§3.5.2 LibreOffice profile, §3.7.2 `engines.lock` `purl`+SHA-256 schema) · 06-build-test-release (§6.7.2 signing-runner binding) · 07-app-shell.

---

## Exit criterion for P0

P0 is "done" when: both enforcement planes are live; every content-independent
gate (P0.3 — incl. G2 `gitleaks` on the current `git`/`dir` subcommands + the L2 range leg,
G47 CSP/capability lint, G49/G50/G56 CI hardening, **G56a branch-protection config
assertion** + **G56b release-tag (`v*`) trust gate** — both fail-soft during the P0 bootstrap box,
then hard, and G56b's ancestry/green-history release-workflow leg fail-closed always)
runs green on both planes; the language-gate contracts (P0.4 — incl. the `deny(unsafe_code)`
unsafe policy and the G57 English-only lint) are **defined with CI
wiring-points** (these are `→ activated in P1` when the Rust/TS code they act on
exists — P0 authors the contract + wiring, not the enforcement on absent code); the **seven P0 docs** (six in P0.1 +
the P0.6 `vuln-response.md` runbook):
[security-concept.md](../security/security-concept.md),
[build-gates.md](../security/build-gates.md), `build-loop.md`, `test-strategy.md`,
`roles-and-escalation.md`, `_format.md`, **and `vuln-response.md`**, exist and pass
the doc-consistency gate; the **security-concept §5 threat table is fully populated
vs spec §0.11** (all **16** classes — incl. T3a and the r3 **T11** — have a control + a
concrete `Gnn`, enforced by plan-lint check 8); the `cargo-vet` bootstrap is clean
(`cargo vet check` on the initial `Cargo.lock`); the build-loop + dual-review protocol
(incl. the build-loop soundness rules + the hard-stop numbers + the ConvertIA DoD)
are written such that P1's first box can be built strictly through the loop with no
guardrail missing; **AND at least one push to `main` has triggered an L4 CI run that
completed green** (run-log URL recorded in a **committed `docs/process/p0-completion.md`** — r7: a commit body is overwritable, so the durable record is a tracked file, stubbed with its schema by **P0.6.10**, with the **plan-lint check 24** (authored in P0.3.5) asserting the recorded URL matches the GitHub Actions run-URL pattern `https://github.com/Ne-IA/convertia/actions/runs/<id>`) — so the exit
criterion is not satisfiable by a local-only run that never exercises L4 and could
hide a workflow-syntax error G49's local lint misses.
