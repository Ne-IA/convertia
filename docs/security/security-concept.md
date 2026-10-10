# ConvertIA — Security Concept (living)

> **The build-time security & guardrail concept** for ConvertIA. Defines *what we
> protect against* and *which guardrail proves it*. The companion
> [build-gates.md](build-gates.md) is the operational gate catalogue (the *how*).
>
> **Status: living.** This document is refined *during* implementation; whenever a
> control or gate changes while building, it is recorded here first. It does **not**
> override the product truth:
> **conflict order = [SSOT](../SINGLE-SOURCE-OF-TRUTH.md) > [spec](../spec/README.md) > this document**
> (the full order is SSOT > spec > security/process docs > plan > code > conversation;
> this document sits in the security/process-docs layer).
> Where this doc and the spec describe the same control, the spec's `§` is the
> source of the technical detail; this doc is the consolidated security view + the
> mapping to enforcement.

## 1. Scope & axis

The [spec](../spec/README.md) describes **what the app does**. This document
describes **how we build it safely** — the threat model, the security controls,
and the defense-in-depth gate system that enforces them. The two are different
axes and are kept in separate files on purpose.

Out of scope (same as SSOT *Explicitly Out of Scope*): distribution/store
logistics, legal advice, developer-account processes — **except** where they
impose an in-code/in-CI requirement (SBOM, checksums, signing the checksum
manifest, license compliance).

## 2. Working model — two sessions, one branch

| Session | Role |
|---|---|
| **Build-Loop session** | Autonomous. Builds the plan box by box, writes tests, runs every gate + the dual review, commits directly to `main`. The gates are the protection — there is no second branch and no merge step. |
| **Co-Pilot session** (the owner's partner) | Escalation & clarification target for the Build-Loop session; strategic decisions; high-level review. Works with the owner. Executes the standing phase-end hardening sweep box that closes every phase `P2`..`P11` ([test-strategy §11](../process/test-strategy.md#11-the-phase-end-co-pilot-hardening-sweep)). |

- **Single branch (`main`), GitHub, GitHub Actions.** No worktrees, no parallel
  branches, no push-lock coordination, no separate feedback/sniping sessions, **no
  merge step and no auto-merge** — the enforcement is **CI green on `main` + required
  status checks on every push** (asserted by G56a), a red `main` fixed immediately.
  The **one** surviving `PR` concept is the **external fork pull-request** (this is a
  *public* OSS repo, so an outside contributor can still open one) — the G56 fork-PR
  secret guard is retained for that reason, **not** for our own direct-to-`main` flow;
  "per-PR" vocabulary elsewhere means "per-push" unless it is explicitly that guard.
- **Two sessions commit to `main`, each from its own clone**: the Co-Pilot pushes only
  while no loop `ci` run is queued or in progress; the loop fast-forwards every iteration
  and rebases before its push ([build-loop.md §0](../process/build-loop.md#0-who-runs-this-and-what-it-is-not));
  G54 refuses a stale-base push. The safety comes from the gates, not from branch
  isolation.
- **Escalation path:** Build-Loop → Co-Pilot session → owner. The Build-Loop
  session escalates on genuine blocks (see [roles-and-escalation.md](../process/roles-and-escalation.md),
  authored in P0); it decides routine implementation/pattern/naming/default
  choices itself.
- **L(-1) security-critical-file change-control (owner decision D1).** The files that
  carry the power to silently weaken an enforcement plane are the **L(-1)
  security-critical-file set** (the cage the gates live in): every file under `scripts/`
  except the declared Loop build tools, the hook and CI planes (`lefthook.yml`,
  `.github/**`), the supply-chain, secret-scan and toolchain trust configs, the
  text-hygiene and lint policies, the caged ratchets, the Tauri capabilities, the pinned
  engine manifest, the G53 negative fixture and the security/process docs. The
  authoritative list is `scripts/l-neg1-files.toml`; this paragraph names its categories
  and never enumerates it. **The autonomous Build-Loop NEVER edits an L(-1) file:** a box
  that needs one is **parked and escalated**
  ([roles-and-escalation §4(g)](../process/roles-and-escalation.md#4-when-to-escalate-to-co-pilot-the-exhaustive-trigger-set),
  [build-loop.md Step 7](../process/build-loop.md#step-7--check-off-the-box)); the
  owner-acked Co-Pilot makes the caged edit and the commit, and only that session writes the
  `L-neg1-ack: owner` commit-body trailer — an agent message is never the owner's ack. The
  pre-push gate **G71** mechanically audits the trailer's
  presence on any L(-1)-touching commit (the trailer is the **only** sanctioned escape —
  there is **no** check-off / `[!extern]` exemption for an L(-1) edit; a commit touching no
  L(-1) file needs no trailer) and, since P4.56.1, the cage's own liveness (a glob matching
  no tracked path, a stale/orphan targetless declaration, a refused, dead or orphan
  Loop-tool escape, or a refused or dead monotone entry fails the gate — the build-gates
  G71 row carries the mechanics)
  (fail-soft during the P0 bootstrap, fail-closed from P1). This is the one **ownership control above the deterministic gates** — the
  trailer records an owner decision; G71 checks the evidence, not the intent (so a leaked
  key or a unilateral cage edit cannot pass unseen). It is independent of the G1
  `Dual-Review:` trailer; both may co-occur on one commit.
- **Ratchets.** A ratchet the Loop moves as ordinary box work lives in uncaged data with a
  `[[monotone]]` entry in `scripts/l-neg1-files.toml`: the strengthening direction is free,
  the weakening direction needs the owner ack (G71); the relied-upon dependency floors
  (spec §0.8) are such ratchets. A ratchet moved only at owner or sweep acts
  (`coverage-floors.toml`, `max_survived_mutants.toml`) stays caged whole.

**The dual review is a quality amplifier, not a security control.** The Opus+Sonnet
review (G1) is self-attested via an unverifiable commit trailer; a gamed `GO/GO`
trailer cannot, by itself, ship insecure code, because the **only security controls
are the deterministic gates (every `Gnn` except G1, the dual review)** — a gate
either passes on a clean checkout or it does not. (The numeric span is stated as
"every gate except G1" deliberately: a frozen upper bound like *G2–G50* drifts every
time a gate is added; a `plan-lint` assertion that the prose matches `max(Gnn)` would
be the alternative, but the open phrasing cannot rot.) The dual review raises quality and catches design defects the gates can't
encode; its evidence trail (the commit body's review record: one `Review:` line per
round and one line per P0/P1 finding, build-loop.md §3 Step 6) makes a "both GO, 0
findings" on a non-trivial diff an **auditable smell** for the phase-end Co-Pilot
spot-audit. The review protocol itself (inputs, severity bar, round cap, post-GO rule)
is canonical in build-loop.md §3 Step 5 and is not restated here. Conflict order for the
Build-Loop: **SSOT > spec > these security/process docs > plan > code > conversation.**

**Reviewer availability + integrity (build-loop soundness, authored in build-loop.md).**
Because the autonomous loop leans on G1 executing, its failure modes are explicit:
**(a)** the two reviewer **model IDs are recorded** on each commit's `Models:`
line, and a deprecation/rename surfaces as an escalation, not a silent skip; **(b)** on a
reviewer error/timeout/rate-limit/5xx the loop retries with backoff a bounded number
of times, then **HARD-STOPS + escalates** to Co-Pilot — it **NEVER** auto-emits a `GO`
trailer with fewer than **two live** reviews and never silently degrades to a single or
zero reviewer (G12 checks the trailer is well-formed and that a `GO/GO` commit's
body carries a review marker — a presence heuristic, not a per-reviewer parse — but cannot prove
two LIVE models ran, so this rule remains the load-bearing defence against a
well-formed-but-unbacked `GO`; the marker check raises the cheat cost from "emit one trailer
line" to "write a plausible review record" and keeps a fabricated record an auditable spot-audit
target). **Correlated-blind-spot residual (stated honestly):** Opus and Sonnet share
model lineage, so "both `GO`, 0 findings" is a *correlated* signal, not two independent
ones — the only retrospective backstop is the auditable-smell spot-audit above. **Recorded
owner decision (r6 — no longer left open):** the correlated-lineage residual is **explicitly
ACCEPTED for v1** (the deterministic gates — every `Gnn` except G1 — carry the real security
weight and bound blast radius regardless of reviewer correlation; G1 is a quality amplifier),
**with a concrete spot-audit cadence: a Co-Pilot auditable-smell spot-audit at every phase
boundary AND a random ≥1-in-10-box sample** of the committed `GO/GO` Review records, both run
at the phase-end sweep (test-strategy §11.2). The
**flip option remains open** — making one reviewer a different model family (e.g. a non-Anthropic
model) to make "independent" literally true is a future owner decision that can be taken at any
time. This decision is recorded verbatim in `build-loop.md` and asserted present by plan-lint
check 20, so the autonomous loop cannot silently run without it.

## 3. Defense in depth — the enforcement planes

A change passes staged, independent defensive planes on its way from idea to a
published release. No plane trusts an earlier one to have caught everything.

| Plane | When | Mechanism | Blocks | Bypassable? |
|---|---|---|---|---|
| **L0 — Build-Loop per box** | While building, before each commit | The build-loop discipline + the **Opus + Sonnet dual review** on the staged diff (**no fix-push cycle** — no push between a fix and its re-review); P0/P1 findings fixed in the working tree before push | the commit (self-gate) | only by rule violation (no technical bypass) |
| **L1 — pre-commit hook** | `git commit` | Git-hook manager, `parallel`, budget < ~10 s (a **SOFT** target — see [build-gates §0](build-gates.md#0-policy)) | the commit | `--no-verify` **and** `core.hooksPath` redirection (both **forbidden** — see the forbidden-bypass list below) |
| **L2 — pre-push hook** (fires at `git push` time) | `git push` | Git-hook manager, budget < ~3 min; docs-only fastpath (six heavy toolchain legs, build-gates §4) | the push | `--no-verify` / `core.hooksPath` (**forbidden**); the fastpath skip of those six legs for a documentation-only push range |
| **L3 — commit-msg hook** (L3 fires at `git commit`, chronologically **before** L2 which fires at `git push`; numbering is stable-by-assignment, not chronological) | `git commit` | Conventional-commit format check + the subject (≤ 100 chars) / body (≤ 120 lines) caps (G11) | the commit | `--no-verify` / `core.hooksPath` (**forbidden**); git auto-subjects (merge/revert/fixup) allowed |
| **L4 — CI (GitHub Actions)** | After push | The same gates re-run on a clean checkout + the heavy gates (cross-platform build, corpus, coverage, SAST, SBOM) | a red `main` (fix immediately) | none for required checks — and **G56a** asserts in CI that the GitHub required-status-checks config exists (so "a red L4 blocks" is real repo state, not an invisible assumption) |
| **L5 — Release** | On a `v*` tag | Release workflow: SBOM + completeness, license hard-fail, copyleft-source-bundle present, **checksums + minisign over `SHA256SUMS`** (the *only* signing in scope — **not** binary code-signing/notarization, SSOT *Out of Scope*), size budget, egress/no-pollution observability gates. The **`v*` tag trigger itself is trust-gated by G56b** (tag-protection ruleset + the release job's first step asserting the tagged commit is an ancestor of `origin/main` with main's required checks green for that SHA, before any secret is read) — so a tag on a never-green commit cannot mint a signed artifact | the release | none (release-blocking); the tag trigger guarded by G56b |

**Two enforcement planes principle.** Every gate that CAN run locally runs **both
planes — locally (L1–L3) and again in CI (L4)**; inherently CI-only gates
(repo-config introspection G56a/G56b, the CI-only L4 corpus/SAST/coverage heavies
— G25–G33a, G48, G50, G57; **G34 is vacated**, not a heavy — and the release-tier L5
gates beginning at G33b and running through the release section) run in CI only.
(Accurate prose rather than a closed `Gnn–Gnn` span — r7: the old "G26–G34 / G35–G67"
labels were factually wrong: G34 is vacated, the L5 set STARTS at G33b not G35, and the
closed G35–G67 range swept in the non-existent G40/G61–G63 and the prose-only reserved
G65/G66/G67; plan-lint check 11 only forbids claiming a span NARROWER than `max(Gnn)`, so
it did not catch this.) Local hooks give
realtime feedback and keep `main` clean; CI is the immutable backstop that proves
green on a fresh clone. A red CI run is
fixed immediately — never re-run hoping it passes, never `--no-verify`.

**Forbidden local-plane bypasses (the complete named set).** The local L1–L3 plane
is **entirely git-hook-based**, so the no-bypass policy must name *every* way to make
a hook not fire — not only the obvious one: **(a)** `--no-verify`/`-n` on commit/push;
**(b)** force-push; **(c)** disabling a required CI check; **(d)** `core.hooksPath`
redirection — `git -c core.hooksPath=<elsewhere> commit/push` (or a persisted
`git config core.hooksPath`) silently disables ALL local hooks **without** `--no-verify`,
a functionally identical un-named bypass. This is **machine-checked**, not only
documented: the P0.6 step-0 session-start sanity asserts `git config --get core.hooksPath`
is unset (or equals the lefthook-managed path), and **G54 resolves the EFFECTIVE hooks
dir** (`git rev-parse --git-path hooks` / `git config core.hooksPath`) rather than
hardcoding `.git/hooks/`, so a hooksPath pointing away from `.git/hooks` cannot make G54
inspect an inert directory and pass while no hook fires. (L4/G25 remains the immutable
net regardless; this closes the local-plane-completeness claim the design otherwise could
not honestly make.)

**Gate isolation from the tree it checks.** A gate that resolves an import from the checkout
can be subverted by a file placed beside it: `scripts/re.py` shadows `re` for every
`python3 scripts/<gate>` run before the gate's code executes, which no in-gate check can
catch. Every plane therefore runs its Python gates as `python3 -P`, and the canary runner sets
`PYTHONSAFEPATH=1`; G54b (leg (5)) fails a plane Python invocation without `-P`. A self-test's
throwaway repository is kept apart from the repository the hook runs in the same way: a hook in
a linked worktree exports an absolute `GIT_DIR`, under which a canary's own
`git config core.hooksPath <tmp>` would persist bypass (d) there, so the runner drops the
inherited `GIT_*` variables (except `GIT_EXEC_PATH`) from every self-test it starts, and every
self-test that starts git itself drops them first (G24).

## 4. Security principles (the invariants the gates defend)

1. **Fully offline, zero egress.** No update check, no telemetry, no font/asset
   fetch, no engine network access. The only outbound action is the explicit,
   user-initiated *open releases page* link. (spec §2.11, §7.2.2, §7.6)
2. **Never harm the original.** Sources are frozen at intake; outputs are
   published atomically with no-overwrite; never write through a symlink/hardlink
   onto a frozen source. (spec §2.1–§2.7)
3. **Untrusted bytes are decoded in isolation.** Every third-party
   decoder/encoder runs in a separate, confined process — never linked into the
   core. The core (memory-safe Rust) is the only thing that touches protected
   paths. (spec §2.12, §3.5)
4. **MIT core stays clean; copyleft is isolated.** Copyleft engines ship as
   separately invoked binaries (aggregation, not linking). No GPL/AGPL component
   may contaminate the MIT-licensed core or the statically-linked Rust binary.
   (spec §3.6)
5. **Supply-chain integrity.** Every bundled engine is version-pinned +
   checksum-verified at build time, recorded in an SBOM, and integrity-verified at
   startup. (spec §3.7, §3.8, §6.1.3, §7.2.3)
6. **Least privilege at the WebView boundary.** Tauri capabilities/CSP grant no
   remote origin, no shell-execute, no broad dialog/opener — engines are spawned
   Rust-side only. This is enforced by construction: a structural CSP/capability
   lint (G47) parses `tauri.conf.json` + `src-tauri/capabilities/*.json` and fails
   on any §0.10 violation, and the deliberate absence of `tauri-plugin-updater` /
   any HTTP-client crate is asserted (a deny-list, not just a convention). (spec
   §0.10, §0.11 T2/T2a/T2b/T2c, §7.6.1)
7. **Authentic, verifiable downloads.** Release artifacts carry per-file SHA-256 +
   a minisign-signed `SHA256SUMS`; the download page carries the verification
   recipe. (spec §6.2)
8. **No stubs, no drift.** Every change is production-ready (CLAUDE.md): no
   `TODO`/`FIXME`/`unimplemented!`/`console.log` in production code without a
   tracked box-id; generated artifacts never drift from source.
9. **In-core untrusted bytes are still adversarially tested.** The §1.2 detection
   layer (the bounded gzip/svgz inflate, the Rust ZIP central-directory peek, the
   OLE2/CFB directory read, the bounded XML structural peeks) runs in the trust
   kernel *outside* the §2.12 isolation boundary — so it is the one untrusted-byte
   path where a panic/OOM/UB lands in the core. It carries its own adversarial fuzz
   gate (G48), not only the engine-side corpus. (spec §1.2, §2.12.4)
10. **Hermetic, hardened CI supply chain.** The CI plane itself is an asset to
    protect (it holds the signing trust anchor `MINISIGN_SECRET_KEY` **and** its
    passphrase `MINISIGN_PASSWORD`, §6.2.3): every workflow declares
    least-privilege `permissions`, every third-party action is pinned by full
    commit SHA (kept current by a `dependabot.yml` whose **`github-actions`,
    `cargo`, `npm`, AND `pip`** ecosystems are all watched — `pip` for the gate-toolchain
    `requirements-ci.txt` layer (Semgrep et al.), which reads CI secrets AND produces findings,
    so it is in the CI trust boundary; the pin-and-watch discipline is
    symmetric across all four graphs, not actions-only; bump PRs are reviewed by
    Co-Pilot/owner, never auto-merged), the build resolves only the committed
    lockfiles (`--locked` / `--frozen-lockfile`, no silent graph drift), and a
    workflow security lint (G49/G50) runs in both planes. The secret-bearing release
    job never runs on a fork pull-request. Every gate/SAST/SBOM tool is itself pinned
    by exact version **and** verified by checksum / image digest at install — the
    fetch-and-verify mechanism is itself a **self-tested control** (a deliberately-wrong
    checksum MUST fail the install; `pip`-installed gate tools use
    `--require-hashes`), because a poisoned or typosquatted gate tool would both miss a
    real finding **and** read the CI secrets (same discipline as a bundled engine). The
    **build toolchain that produces the shipped artifact** is in the same trust
    boundary — "everything that touches the bytes that get minisigned" — so the CI base
    image/container is digest-pinned, the C/C++ toolchain and the Tauri
    CLI/bundler/linker are version/digest-pinned, and the `cargo-fuzz` nightly channel
    is **date-pinned** (`nightly-YYYY-MM-DD`, asserted not a floating `nightly`).
    **Branch-protection / required-status-checks config** is also CI-protected, not
    just documented: in the single-branch direct-to-`main` model the repo config is the
    only thing that makes a red run actually block, so G56a queries the GitHub **rulesets**
    API (NOT classic branch protection — its required-checks block *all* direct pushes incl.
    admins, GH006, so it cannot coexist with direct-to-`main`) and fails on missing required
    checks, on the force-push/`deletion` rules being absent or carrying any bypass actor, or
    on the required-checks rule being bypassable by a **non-admin / wider** actor. **The repo-
    ADMIN role bypassing the *checks* rule is the intended trusted-direct-pusher mechanism**
    (a fork-PR stays gated; the owner/Build-Loop can direct-push), while the history-protection
    rules carry NO bypass — this owner-decided ruleset split is what reconciles "required checks
    on `main`" with the autonomous direct-to-`main` push, replacing the unsatisfiable
    enforce-admins assertion. As sub-checks G56a also fails on disabled secret-scanning/push-
    protection or a non-read default workflow-permission. **The secret-bearing release trigger is the `v*` TAG ref, which
    G56a does not cover — so G56b applies the same "a red run actually blocks" guard to the
    tag ref:** a `v*` tag-protection ruleset + a release-workflow first step asserting the
    tagged commit is an ancestor of `origin/main` with main's required checks green for that
    SHA, before any secret is read, so a tag on a never-green commit cannot mint a signed
    artifact.
11. **CI runner-host integrity (the secret never shares a host with untrusted
    input).** The `MINISIGN_SECRET_KEY`/`MINISIGN_PASSWORD` are the single most
    damaging secret in the system — the entire user-facing trust substitute
    (minisign over `SHA256SUMS`, principle 7) collapses to this one key. The
    Ne-IA self-hosted IONOS VPS runner is **shared** (four other projects' Lane-A
    CI) **and** runs the Lane-B Linux corpus leg, which processes `corpus-large`
    untrusted/adversarial files + the fuzz/adversarial-egress inputs (spec §6.1.4 /
    §6.7.2). A persistent multi-tenant runner that handles untrusted input is the
    textbook host-compromise vector — once poisoned, every future release can be
    silently re-signed with the real key. So: **(a)** the secret-bearing
    signing/release step runs **only on an ephemeral GitHub-hosted runner** (or a
    single-tenant JIT runner destroyed per job), **never on the shared VPS**;
    **(b)** the untrusted-corpus/fuzz jobs and the secret-bearing job are
    host-isolated (no shared workspace, no shared runner host); **(c)** runner-egress/
    process hardening is split **by runner type** because the named tools have a
    runner-type constraint: **`step-security/harden-runner`'s free/Community tier only
    works on GitHub-HOSTED runners — self-hosted support requires a StepSecurity
    Enterprise license** (verified against the StepSecurity docs). So harden-runner runs
    in **BLOCK** mode (strict egress allowlist) on the **ephemeral GitHub-hosted
    signing job** (where the free tier applies and adds value — it runs no engine and has
    nothing to observe). On the **shared self-hosted IONOS VPS** the free tier does NOT
    apply, so the enforcement there is **G42b's `ptrace`/Landlock fs-audit + G42's
    nftables/strace egress monitor + the VPS's own OS egress allowlist + generic
    self-hosted hardening** (an **ephemeral / JIT runner with no persistent workspace,
    a dedicated low-privilege user**) — NOT harden-runner. (Adopting Enterprise to run
    harden-runner on the self-hosted leg is an owner decision; without it the catalogue
    must not claim a free-tier harden-runner control on a self-hosted job.) This is
    enforced structurally (G56 — a workflow lint flags any secret-using job bound to a
    self-hosted label, and asserts harden-runner presence/mode ONLY on GitHub-hosted
    jobs, never on a self-hosted job where the free tier would be inert). **Community-tier
    silent-degrade residual (stated honestly):** harden-runner's free/Community tier carries
    a ~10k-runs/week ceiling above which it silently degrades to no-enforcement — a
    silent-degrade-to-no-enforcement on a control the design relies on for the secret's host
    is exactly the failure class hardened against elsewhere. The signing job is rare/tag-only
    so the ceiling won't bind in practice, but to be safe the release job **asserts
    harden-runner reported ENFORCED (BLOCK active), not merely present** — a degraded-to-audit
    or no-enforce status on the signing job is a release-blocking fail. **harden-runner's lack
    of a programmatic enforcement-status step output is handled by an ACTIVE PROBE, not a
    log-string parse (r7 — verified: step-security/harden-runner exposes NO step output for
    block/enforcement status; status surfaces only via the job log, the markdown summary, and
    the StepSecurity dashboard, so "parse a log line for ENFORCED" is fragile):** a post-step in
    the signing job attempts a known-blocked outbound connection and asserts it **FAILS**
    (proving BLOCK is live), with a G24-style self-test; G56 names the probe (verify the action's
    actual output API at the exact pinned version first and use a real enforcement output if one
    is added upstream).
    **`ANTHROPIC_API_KEY` host-isolation (r7 — principle 11 was asymmetric, naming only the
    minisign pair; the §5 row names `ANTHROPIC_API_KEY` as the THIRD real secret):** the
    reviewer-API key is **build-session-local — it lives in the autonomous Build-Loop session's
    environment and is NEVER present in any GitHub Actions / CI job** (the dual review G1 runs in
    the build session, not in CI), so it does not co-reside with the untrusted-corpus CI legs and
    needs no CI host-isolation. Its blast radius is bounded regardless (**G1 is NOT a security
    control** — a leaked reviewer key cannot ship insecure code or mint a signed artifact; it
    only suppresses the human review-trace the spot-audit relies on). **IF any future workflow
    ever reads `ANTHROPIC_API_KEY`, it inherits the same self-hosted-label ban + host-isolation
    as the minisign secret** — the G56 self-hosted-label / disjoint-host lint extends to all
    THREE named secrets, not only the minisign pair.

## 5. Threat model → control → gate

The spec's threat map (spec §0.11, owned by
[00-architecture.md](../spec/00-architecture.md) — `02-guarantees.md` only
cross-references it) is the **authoritative enumeration** — exactly
**19 classes**: `T1, T2, T2a, T2b, T2c, T3, T3a, T4, T5, T6, T7, T7a, T8, T9a, T9b, T10, T11, T12, T13`
(`T7a` added by the 2026-07-21 P3.73 P0 ruling — the Windows reserved-name output-aliasing class).
This table carries **one row per class** (including the `a`/`b`/`c` sub-rows),
mapping each to its primary runtime/build control **and** the concrete `Gnn` gate
that proves the control is in place. **§0.11 ↔ §5 parity is bidirectional and
machine-checked** (plan-lint check 8, §6 of [build-gates.md](build-gates.md)): every
§0.11 class has a row here, and every row cites a `Gnn`. A class with a runtime
control but no verifying gate is itself a gap.

| Threat (spec §0.11) | Primary control | Verifying gate (see [build-gates.md](build-gates.md)) |
|---|---|---|
| **T1** untrusted decoder input → crash/hang/exploit | engine isolation (§2.12) + invocation timeout/kill (§1.7) + pool bounds (§0.9); the in-core §1.2 detection layer is memory-safe Rust. **Honest residual `[DECIDED]` (stated, not implied-covered):** the §2.12.3 runtime privilege-drop — the realized tiers (Linux Landlock + netns + seccomp; macOS cheap tier; Windows intermediate IL + Job Object) — is **best-effort and SILENTLY degrades** to a cheap tier — so T1 has **no LOAD-BEARING runtime containment** of an RCE inside a decoder; process-isolation bounds a *crash* (and a successful exploit's blast radius to one sandboxed sidecar + its scratch), but does not by itself stop code-exec inside that process. The load-bearing controls are isolation + the timeout/kill + the input never reaching the core; the privilege-drop tier is defence-in-depth that is *tracked* (G64 ratchet) but accepted as degradable | **G48** in-core detector fuzz (`cargo-fuzz` over `crate::detection`); **G31** per-pair corpus + reliability through the §2.12 boundary (engine-side T1 — incl. the crafted-BMP ImageMagick sentinel, the densest-CVE decoder) + the §2.12.3 **privilege-drop-tier-applied** + **memory-cap kill** + **Job-Object reap** positive assertions; **G64** privilege-drop-tier ratchet (a commit lowering an achieved tier fails/escalates — the silent-degrade can't quietly regress net coverage); **G26** engine-side adversarial corpus through the §2.12 boundary, with an **`engines.lock`↔adversarial-fixture bijection guard** (every staged engine — incl. ImageMagick — has ≥1 fault-injection fixture targeting its type, hard-fail otherwise); isolation runtime test |
| **T2** malicious / compromised WebView content | §0.10 capability allowlist (no WebView `fs`/network) + CSP (no remote origins, `object-src 'none'`); the §0.10 by-construction hardening keys asserted by G47 — **all six (r7 — the prose previously named only three, lagging the G47 gate body):** `withGlobalTauri` off, `dangerousDisableAssetCspModification` off, release `devtools` off, **`app.security.dangerousRemoteDomainIpcAccess` absent/empty** (a Tauri v2 `app.security` knob — NOT `app.windows[]` — that, if set, lets ANY remote origin invoke registered IPC commands directly, collapsing the T2 boundary), **`assetProtocol.enable` absent/false**, and **`bundle.createUpdaterArtifacts` absent/false**. **Un-pinnable runtime residual `[DECIDED]` (named, not implied-covered):** the **OS-provided WebView itself** (WebView2 Evergreen on Windows, WebKitGTK on Linux, WKWebView on macOS — a large untrusted-HTML-parsing C++ surface with its own CVE stream) is the named OS mechanism for T2 yet is **un-pinned, un-versioned, absent from the SBOM** — its integrity/CVE-currency is delegated to the OS update channel; ConvertIA's only side-bound is the G47 CSP lock (the WebView analogue of the G37c glibc-floor honesty). **JS name-trust gap (r7, un-verified residual ACCEPTED for v1):** G18c proves every `pnpm-lock` resolution URL ∈ the pinned registry but NOT package-name legitimacy — a typosquat / convincing-name malicious package hosted ON the pinned registry passes G18c entirely, where the Rust side's `cargo-vet` (G18b) closes exactly this class; residual accepted for v1 — its lightweight closure (a committed npm package-name+scope allowlist diffed against `pnpm-lock.yaml`, or Socket CLI as a non-blocking corroborator) is declined for v1 (a gate-status `decided` row) | **G47** CSP/capability structural lint (incl. ALL six §0.10 by-construction keys above); **G18** deny-list (no `tauri-plugin-updater`/HTTP-client crate); **G18c** registry-origin pin (name-trust gap residual noted above); **G42** offline-egress monitor (Lane-B confirmation, macOS WebView gap noted); **TAINT depth for the T2 surface (r7): G56a sub-check (f) is an OR-GATE** — it passes if EITHER CodeQL `javascript-typescript` code-scanning is ENABLED (`code-scanning/default-setup` → `state == "configured"`) OR the **G29 rule (i)** Semgrep `mode: taint` ruleset (WebView/IPC sources → DOM/eval + IPC-arg sinks) is present with its planted-positive — the inter-procedural taint the structural checks cannot reach; **owner-decidable which ships, exactly one is required (machine-enforced as an XOR by plan-lint check 21)** — so selecting the sanctioned Semgrep path can never leave a permanently-red required check (the prior hard CodeQL assert did) |
| **T2a** WebView steers a write to an attacker-chosen path | destination roots are core-picked (`[DECIDED 2026-07-06]` core-owned paths): C2b runs the native picker Rust-side and returns a `DestinationId` + display string, and `DestinationChoice::ChosenRoot(DestinationId)` resolves against the core-side session registry — the WebView never supplies a root path STRING, only an ID the core itself minted; the no-harm machinery — non-destructive create + write-target link-safety + divert (§2.1/§2.3.3/§2.7) — remains the backstop bound on the resolved root | **G19/G31** fs-safety unit + property tests (no-clobber + link-safe on a core-resolved `ChosenRoot` (`DestinationId`)); adversarial-path corpus (T7-shared) |
| **T2b** WebView re-submits an attacker-chosen SOURCE path | eliminated by construction (`[DECIDED 2026-07-06]` core-owned paths — the wire carries no path in either direction: intake paths live only in the core-side `PendingIntake` buffer and C1 `drain_intake` drains them by `collectingId`, so the WebView holds no path to echo back); the freeze-time §1.1 re-validation (canonicalise / resolve-identity / existence / detection at the §2.4 freeze) remains as defence-in-depth over the core-side intake sources — provenance-independent | **G31** freeze re-validation property test (every intake path re-validated regardless of provenance) |
| **T2c** WebView plugin-write surface (`log:default`) | the log bridge appends WebView-supplied diagnostic text to ConvertIA's own log file (spec §0.11): no path, no file choice, no user-file contents; no `store:` permission — prefs are core-owned (spec §7.4.2), so no `plugin:store` command is invocable (the `store:default` grant, an arbitrary-file JSON write through the pinned plugin's path-taking `load`, was removed 2026-09-29). Worst case: noisy local log lines, never read/exfil/overwrite of user data | **G47** — every capability permission is one of `{core:default, log:default}` (per-entry membership; a scoped entry fails), so any other grant or token fails the build |
| **T3** bundled-binary supply chain | pinned + checksum-verified engines; build-time hash manifest; startup integrity. **Engine acquisition decided per engine per platform (spec §3.8, P0 review r3):** prebuilt-vs-from-source is an explicit policy because the two have different ground truths — **from-source** ⇒ the binary SHA is a build-output stability check, and provenance moves to the **§3.8 `from_source.anchor` (one of four variants, each independent of the download host) + digest-pinned build toolchain/base image** — **but a validly-signed tarball is NOT sufficient (r7): the xz/liblzma backdoor rode a SIGNED tarball whose autotools-GENERATED files differed from git, so the anchor prefers a VCS tag/commit (`git archive` of the signed tag, `configure`/`m4` regenerated locally) or, where a tarball must be used, a diff of its non-generated sources against the upstream VCS tag; both the tarball SHA AND the VCS tag/commit are recorded in `engines.lock`** (G37); **prebuilt** ⇒ corroborate via **≥ 2 independent mirrors** OR a **distro GPG-signed package + signed repo metadata** (a bare hash of one unsigned download is unacceptable — it launders provenance, the xz/liblzma class). **FFmpeg** (which publishes no signature for the common gyan/BtbN prebuilts) is built from source only, anchored on the GPG-signed `ffmpeg.org` release (`detached-signature`): no prebuilt satisfies the §3.5.1 `--disable-network` floor. An **engine-source allow-list** constrains which hosts a pin (and its corroboration checksum) may come from, on independent origins. *Residual risk stated honestly:* startup integrity gives **no runtime tamper-resistance** (a whole-bundle swap swaps the in-bundle manifest too); the floor is the §6.2 SHA256SUMS + minisign anchor, not a runtime check (§3.8, §6.1.3, §6.3.4, §7.2.3). **Declined for v1 (a gate-status `decided` row):** binding the in-bundle manifest's digest into the binary at build time, and a startup read of the signed `SHA256SUMS` — the residual above stands, and the authenticity floor stays the user-verified minisign signature (§6.2) | **G37** engine-checksum build gate (verify vs in-repo `engines.lock` before staging + on cache-restore) + **pin-establishment provenance assertion** (the spec §3.8 acquisition-mode corroboration — named satisfiable source per engine incl. FFmpeg; any `engines.lock` SHA edit is a hard Co-Pilot escalation) + **engine-source allow-list assertion** (every `engines.lock` source URL ∈ the committed per-engine origin allow-list, pin URL and corroboration URL on independent origins); **G35** SBOM completeness; **G46** startup integrity verification; **G17b** *(informational, planted-positive self-tested)* OSV/grype CVE scan over `engines.lock` (PURL-keyed; the **FFmpeg CPE is MANDATORY** — the highest-CVE surface is FFmpeg's enabled internal decoders, which `pkg:generic` PURLs miss; the planted-positive uses a historical INTERNAL-decoder CVE) |
| **T3a** DLL/dylib/`.so` side-loading of a bundled shared object inside an engine resource tree (LibreOffice's; FFmpeg links its codecs statically, spec §3.9.1) | every staged shared object pinned — individually `engines.lock`-rowed with its SHA-256, or covered by its prebuilt tree engine's per-(archive, triple) row with its per-file digest in the §7.2.3 manifest (spec §3.7.2 item 1) — + verified before staging; on Windows, engines spawned with a minimal explicit `PATH` (the program's own bundle directory only, the spec §3.5 env table, so the OS DLL search starts inside the bundle) + the §3.5 loader-injection-var strip (`LD_PRELOAD`/`LD_LIBRARY_PATH`/`DYLD_*` cleared); a staging-time dynamic-dependency-closure check that every non-system dependency resolves **inside** the bundle | **G37** per-shared-object SHA-256 verify (each `.dll`/`.dylib`/`.so` by its row or its tree's per-file digest, not just the primary engine binary); **G35** manifest diff hard-fails on a staged shared object not matching its `engines.lock` row (an ATTRIBUTION check per §6.3.3 item 1 — never a post-staging byte re-verify: staging re-emits bytes, the `lipo` merge and the P4.30 load-path rewrite both rewriting load commands, so the post-staging anchor is the §7.2.3 in-bundle manifest); **G37b** dynamic-dependency-closure assertion (`ldd`/`readelf -d` Linux · `otool -L` **AND `otool -l`** macOS — the `-l` leg enumerates `LC_RPATH`/`LC_LOAD_DYLIB` so an `@rpath` resolving OUTSIDE the bundle (Homebrew/`/usr/local`) is caught, which `otool -L` alone misses · `dumpbin /dependents` Windows — every non-system dep resolves inside the bundle, catching both a side-loading vector and an offline-floor break where an engine links a Homebrew/distro lib present only on the build runner) |
| **T4** open-file launch of a fresh artifact | §7.7 open-file safety (reveal-in-folder, no auto-open) + the §7.7.3 Rust-side C9 gate: `open_path { target: OpenTarget }` resolves a run-scoped ID against `State<RunResultStore>` — the WebView cannot name a path at all (`[DECIDED 2026-07-06]` core-owned paths), only a target the core maps to its own recorded result path | **G15/G31** membership-check unit + integration tests re-keyed to the `OpenTarget`→`RunResultStore` ID-resolution (only a current-run result may be opened, and only by ID) |
| **T5** core panic / app fault | §2.13 app-level fault model (`catch_unwind` worker boundary) + §7.2 startup faults + §0.3.1 WebView-absent handling | **G15** panic-boundary unit test (panic → app-fault, not crash); **G46** missing/corrupt-engine → app-fault acceptance |
| **T6** copyleft aggregation boundary | §3.6 copyleft isolation (separately-invoked binaries, aggregation not linking); §0.3/§0.7 subprocess model | **G18** `cargo-deny` GPL/AGPL ban on the Rust crate graph; **G36** SBOM forbidden-family hard-fail; **G38b** LGPL-relink + GPL-corresponding-source bundle-present assertion (§6.1.3 ii/iii incl. x265 GPL §3); **G53** core-crate forbidden-dependency check (`cargo-deny [bans]` workspace-member-scoped — no image-worker C libs in the core closure) |
| **T7** path / link redirection (symlink/junction/TOCTOU) | §2.3 resolved-identity & link safety + §2.1 exclusive create-new-or-fail on the resolved real file | **G19/G31** atomic-publish/fs-safety unit + property tests; adversarial-path corpus |
| **T7a** Windows reserved-name / trailing-dot-space output aliasing (`CON.csv` → candidate `CON.tsv`: a Win32 open aliases the console/device; the §2.1.2 NT dir-handle publish would create a real-but-Win32-unopenable file; a trailing dot/space is silently stripped — an alias onto a DIFFERENT name) | **§2.2.4** Windows-unopenable-name guard (`[DECIDED — the 2026-07-21 P3.73 P0 ruling]`): the pre-publish validation seam (beside §2.2.3 `check_path_limit`) rejects — Windows-only, on every ConvertIA-**constructed** component (the §2.2.1 leaf + §2.7.1-recreated subtree dirs, never user-chosen existing ancestors) — a right-trimmed first dot-segment equal to a reserved DOS device name or a trailing dot/space, as a clear per-item §2.8 failure naming the token; never alias/sanitise/truncate; the divert path re-checks identically. Neither `output_name` (verbatim-stem, §2.2.1) nor `is_safe_output` (no-clobber-only, §2.3.3) owns the class — §2.2.4 does; built by **P3.88** | **G15** P3.88 unit tests (reserved first-dot-segment incl. right-trim + trailing-dot/space reject on Windows; the legal-on-Unix pass-through; the divert re-check) — the corpus/E2E leg joins G31's hosted fs-safety assertion set when the §2.1.1 write sequence exercises it per-pair |
| **T8** self-feeding / batch expansion | §2.4 frozen source set + §7.1 instance/run identity | **G15** frozen-set + per-run-ownership unit tests (the data-structure leg); **G31** T8 INTEGRATION sub-test (the live-path leg) — a batch whose conversion writes outputs INTO the same watched/dropped source folder mid-run asserts the fresh outputs are **NOT** in the run's ingest/result set (snapshot-not-live-iteration, §2.4.2), and a two-instance fixture where instance B drops a `file`/`*.part` into a shared folder mid-run asserts instance A's frozen set never grows (§2.4.3 concurrent-instance hand-off) |
| **T9a** ConvertIA's own code exfiltrates user files | structural: opens no socket — no HTTP/updater on the §0.10 allowlist, no remote `connect-src`, no phone-home (§7.6) | **G29 project-local rules (g)+(j)** the per-push Rust-source net-ban — **(g)** `std::net`/`tokio::net`-outside-allow-list (the import-site path) **and (j)** `libc::socket`/`libc::connect`/`nix::sys::socket`-outside-allow-list (the raw-syscall FFI path the imgworker's `#[allow(unsafe_code)]` surface allows, which (g) structurally misses) — together the per-push structural proof that first-party Rust opens no socket by EITHER path, catching the renamed/transitive crate G18's name-based ban misses + **G47** CSP/capability lint + **G18** HTTP-client deny-list (no socket-opening dep ships); **G42** packet-monitor / egress-deny release gate (the release-tier proof) |
| **T9b** bundled engine reaches out / reads out-of-input on hostile input (incl. the LibreOffice macro-execution / `WEBSERVICE()`-external-data vectors) | load-bearing argv/build controls: FFmpeg `-protocol_whitelist file,pipe` + curated demuxers (no `concat` demuxer; `-safe 0` never passed); pandoc `--sandbox` (pandoc ≥ 2.15 — `--sandbox` shipped in 2.15 and is honoured from 2.15 on, enforced by pandoc's type system; spec §3.5.4); LibreOffice hardened profile (`-env:UserInstallation=file://<per-job scratch>` disposable profile, asserted by the G29 argv rule + G31 — without it LibreOffice re-uses the user/home profile, a T9b read-half + T8 cross-job leak) (`MacroSecurityLevel = 3` + `DisableMacrosExecution = true`, `LinkUpdateMode = 0`, no external-data-range / `WEBSERVICE()` refresh on load, §3.5.2); librsvg **no base URL** (§3.5.x); **poppler `pdftotext` built WITHOUT network/HTTP** (no `curl`/`libsoup` in its dynamic closure — a crafted PDF can carry `GoToR`/remote-action / annotation URIs; asserted by G38 + a G31 remote-URI-PDF corpus sentinel; r7 — poppler was omitted from this control enumeration); **ImageMagick hardened policy** — a bundled `policy.xml` (consulted via `MAGICK_CONFIGURE_PATH` set by the imgworker, since MagickCore reads it from there) setting `<policy domain="coder" rights="none" pattern="{URL,HTTPS,HTTP,FTP,EPHEMERAL,MVG,MSL,TEXT,LABEL,SHOW,WIN,PLT}">` + a path-rights deny on `@`-indirect reads, OR equivalently a trimmed IM build compiled with those coders/delegates excluded (the historically most CVE-dense decoder family: ImageTragick CVE-2016-3714 + the URL/MSL/MVG coder SSRF/LFR/RCE class; ImageMagick is statically linked inside `convertia-imgworker`, §3.5.5, so the §2.12 worker isolation bounds blast radius but is the degradable §2.12.3 tier, NOT the structural T9b control — the load-bearing control is this policy/coder lockdown). **Two load-bearing halves, each with its own armed enforcement substrate:** (a) **zero outbound packets** (the egress half) and (b) **no out-of-input FILE READ** (the read half — symmetric, not a mere oracle). **The structural T9b/T1 controls are bundled CONFIG, not binaries (r7):** the hardened `policy.xml` and the `registrymodifications.xcu` each carry their own `engines.lock` SHA-256 row (verified by G37, counted by Syft G35) and are hashed by the §7.2.3 startup integrity check — a 0-byte `policy.xml` / damaged `.xcu` MUST fault, not silently disarm the lockdown | **G38** per-engine build assertions (`ffmpeg -protocols`/`-demuxers`, librsvg no-base-URL, `pandoc --version ≥ 2.15`, the **poppler `pdftotext`-built-without-network** introspection + the remote-URI-PDF G31 sentinel (r7), the **LibreOffice profile assertion** — parse the shipped `registrymodifications.xcu` and assert `MacroSecurityLevel`/`DisableMacrosExecution`/`LinkUpdateMode` + the external-data keys, and the **ImageMagick hardened-policy assertion** — the bundled `policy.xml` denies the dangerous coders `{URL,HTTPS,HTTP,FTP,EPHEMERAL,MVG,MSL,TEXT,LABEL,SHOW,WIN,PLT}` + the `@`-indirect-read path-rights deny, OR the trimmed IM build was compiled coder-/delegate-excluded — see the ImageMagick T9b/T1 row); **G29** the per-spawn argv/env safety rule (incl. the imgworker `MAGICK_CONFIGURE_PATH`-points-at-the-bundle-policy assertion + the LibreOffice `-env:UserInstallation` disposable-profile presence + value check + the pandoc `--resource-path <scratch-only>` value check); **G31** corpus sentinels (a `.docm`/`.xlsm`/`.pptm` AutoOpen/`Workbook_Open` macro writing a canary inside the egress-deny window → canary **NOT** created; a `WEBSERVICE()` `.xlsx` → no egress/no out-of-input read; **a crafted BMP / SVG-via-MSL/URL-coder → no egress + no out-of-input read inside the G42/G42b window**; **a poppler remote-URI-annotation `.pdf` → no packet inside the egress-deny window (r7 — the poppler sentinel was documented in G38 but missing from this T9b verifying-gates cell and the G31 sentinel host-list)**), pulled forward into the per-push L4 leg; **G42** release-confirmation adversarial-egress monitor (the EGRESS half — zero outbound packets, armed-window canary, fail-closed); **G42b** the read-half fs-audit enforcement substrate (spec §6.4.2 — `ptrace`/Landlock, fail-CLOSED when neither is available, out-of-input sentinel + planted-positive, symmetric with G42 so the read half can never silently no-enforce **on the Linux leg**; macOS/Windows rest on the degradable §2.12.3 tier + the G31 sentinel oracle; **macOS = G42b option (b) BY DECISION since the P4.16 cheap-tier ruling (spec §2.12.3 macOS row: no Seatbelt profile is ever applied in v1-portable — the owner-accepted residual is recorded in the G42b row); Windows = option (b) BY DECISION since the P4.17 Co-Pilot ruling 2026-08-25 (the realizable Windows legs — a reduced-integrity intermediate-IL token + Job Object — confine writes/resources only, files carry no `NO_READ_UP`, and the AppContainer profile is unrealizable in v1-portable; the owner-accepted residual is recorded in the G42b row)**) |
| **T10** resource exhaustion / DoS-by-input | §1.10 resource pre-flight & budgets (incl. an **output/scratch BYTE budget** — r7: the decompression-bomb defence covers the in-core detect path + the §1.10 RAM / §2.12.3 memory-cap-kill, but a decoder that slowly explodes a 1 KB input into a 50 GB intermediate WITHIN its memory/time budget exhausts the scratch DISK, an exhaustion axis the RAM/handle/time budgets miss; the output/scratch byte budget kills to a clean `Failed(TooBig)` when decoded output exceeds N× input or an absolute scratch ceiling) + §0.9 pool/handle bounds + the to-GIF guardrail | **G16/G31** adversarial resource-budget corpus + property tests (oversized-render SVG, over-duration to-GIF, over-cardinality batch → fail-clearly, batch continues, no handle/RAM exhaustion); decompression-bomb fixtures (svgz/ZIP-in-OPC/nested-flate); **an output/scratch-byte-budget adversarial sub-case** (a bomb whose DECODED output exceeds N× input or the absolute scratch ceiling → killed to a clean `Failed(TooBig)`, batch continues, scratch returns to baseline) |
| **T11** macOS engine-as-first-TCC-accessor (silent-deny) | §3.5.0/§7.2.6 macOS TCC source staging — the Rust core (holding the TCC grant from §1.1 freeze) copies a TCC-protected source into a per-job kind-2 scratch path (§2.14.2) **before** spawning, so a sidecar is never the first process to touch Desktop/Documents/Downloads/removable media (a chain-break otherwise triggers an invisible TCC denial / wrong-process prompt that defeats the conversion, and is silent on CI which runs from `TMPDIR`) | **G31** macOS sub-test (the Rust core PID, not the engine PID, is the first accessor of the protected path; the engine receives a kind-2 scratch path); **G29** leg (two halves per the 2026-08-30 P4.26 adjudication, build-gates G29 (d)): the Semgrep rule (every `Command::new` in the macOS isolation module — the delivered `paths:`-scoped P4.85 form — is preceded by the stage-for-TCC / scratch-staging call), a deliberately-vacuous armed door guard with zero in-scope spawn sites (every spawn lives on the cross-platform §2.12.3 floor and takes the already-staged §3.2.2 `engine_input` value) that fires mechanically on any future mac-homed spawn carrying the `misplaced_macos_cfg` spellings (a literal-free OS predicate or an audited `nosemgrep` is the G31 leg's to catch); plus the non-vacuous check-sast `t11_seam_pin` (the seam's macOS arm must call the literal standalone `stage_for_tcc` — the rename class closed); the behavioural burden sits on the G31 leg |
| **T12** unsigned distribution / download-MITM (tamper between our release and the user) | the build ships **unsigned/unnotarized** artifacts (binary code-signing is SSOT *Out of Scope*) — the trust anchor is the **minisign-signed `SHA256SUMS`** (§6.2) + an **out-of-band pubkey-fingerprint** anchor (the in-repo `docs/minisign.pub` TOFU is otherwise circular); the user verifies before first run. **Residual (accepted + documented, not implied-covered):** no runtime tamper-resistance + SmartScreen/Gatekeeper friction on an unsigned build (§6.2.4) — named in SSOT *Out of Scope* + the release notes | **G39** checksums + minisign over `SHA256SUMS` + the executable verify-recipe assertion; **G44** verify-recipe-present + the pubkey out-of-band-fingerprint match (the deleted **G40** cosign/SLSA binary-signing path is the explicitly-out-of-scope alternative) |
| **T13** cross-user single-instance socket (macOS machine-global) | `tauri-plugin-single-instance` hard-codes the macOS single-instance socket at world-writable `/tmp/{id}_si.sock` carrying the second launch's `cwd`+`argv` (§7.1.1) — so on a **multi-user Mac** a different OS user can pre-bind (path leak), inject (paths→intake), or squat (DoS). **Accepted v1 limitation (§7.1.1 `[DECIDED]`):** the injection half is bounded by the **same provenance-independent freeze re-validation that backstops T2b as defence-in-depth** (the injected leg is core-side — a socket-delivered path enters the core-side §7.8.1 funnel/`PendingIntake` buffer exactly like a launch argv, never the IPC wire; every intake path is canonicalised/resolve-identity/existence/detection re-validated at the §2.4 freeze; a substituted path only converts an A-readable file to an output beside it, no-clobber + link-safe). The **confidentiality (launch-path leak) + DoS halves are accepted residual** (named, not implied-covered): a local logged-in second macOS user is out of the offline-converter threat model, the leaked data is **user-visible launch PATHS** (never file contents), single-user Macs dominate, and the **macOS PRIMARY single-instance path is the AppleEvent (§7.8)** — unaffected (the /tmp socket covers only direct-binary re-exec, the least-mature leg). Win/Linux are per-OS-user by construction (session namespace / session D-Bus). | **G31** freeze re-validation property test (the injected-path half — every intake path re-validated regardless of provenance, the same property that backstops T2b); the §6.6 macOS walkthrough confirms the normal single-`.app` path. The leak/DoS halves are accepted residual, no gate (§7.1.1 `[DECIDED]`) |

**Cross-cutting build/release controls** (not §0.11 threat classes, but
load-bearing security guarantees with their own gates):

| Guarantee | Primary control | Verifying gate |
|---|---|---|
| credential / secret in repo | no secrets committed; the real CI secrets are **THREE**: `MINISIGN_SECRET_KEY` **and** `MINISIGN_PASSWORD` (§6.2.3) **and** the build-session reviewer-API `ANTHROPIC_API_KEY` (`sk-ant-…`, the §5 reviewer-API dependency row) | **G2** `gitleaks` secrets scan using the **current** subcommands (`gitleaks` v8.19+ deprecated `protect`/`detect`): L1 `gitleaks git --staged` + **L2** `gitleaks git` over the unpushed range `@{u}..HEAD` (the last local catch before a secret goes public; **excluded from the docs-only fastpath** — secrets in `.md` count) + L4 full-tree `gitleaks dir` + a release-tier **full-history `gitleaks git`** leg over the commit log (a once-committed-then-removed secret stays live forever in a **public** OSS repo with no PR-review backstop). All THREE secrets are named + provably caught + planted-positived: a committed `.gitleaks.toml` carries a **custom rule matching the minisign secret-key shape** (the `untrusted comment:` header co-occurring with a long base64 blob / a banned `*.key` staging) — `gitleaks`' default PEM rule keys on `-----BEGIN…-----` delimiters and a minisign key has none, so the PEM rule alone would **miss** it; **`ANTHROPIC_API_KEY` is caught by `gitleaks`' bundled Anthropic-key rule (`sk-ant-` prefix) — confirmed present + a planted-positive**; **`MINISIGN_PASSWORD` is a free-form passphrase that relies only on entropy + the `MINISIGN_PASSWORD` env-name scan**, so a committed `MINISIGN_PASSWORD=<value>` literal line is additionally banned by a custom rule. The committed `.gitleaks.toml` path allowlist + the zero-entry baseline are the **only** suppression this repository controls, frozen by `check-gitleaks-allowlist` (a key-shape check of the whole config + a refusal of a tracked `.gitleaksignore`); `scripts/run-gitleaks` honours no inline `gitleaks:allow` comment, no `.gitleaksignore` (it refuses to scan beside one) and no binary verdict of git's (its `git` modes read no in-tree attributes file and diff every path as text, so neither a `binary`/`-diff` mark nor a NUL byte hides a file), in every mode — each of these let the planted secret through with no caged edit before the driver closed it. **Named residual:** the global allowlist of gitleaks' bundled rule set, which `useDefault` appends (the pinned 8.30.1 cannot keep the bundled rules without it), skips a path such as an image, a font, a document, a lock file or a dependency / vendored tree for every rule, the custom ones too, and drops a bundled rule's finding whose secret holds `false` in any case or a stopword, starts with `true`, ends in `null` or is a placeholder shape (each custom rule reports a fixed capture — the key's `RW` prefix, the variable name, `sk-ant-` — so no value it matches reaches that list: a `MINISIGN_PASSWORD` holding `False` passed every mode while the rule reported the value) — accepted, since G2 guards against a careless leak of a text secret, those paths hold generated, binary or vendored artefacts, a bundled rule's secret is a random token that holds such a word only by chance, and a secret placed there on purpose inside the owner-acked tree is the G1 / owner surface; G24 legs pin both reaches, so a gitleaks bump that moves either reds. The G56a secret-scanning + push-protection sub-check is the free GitHub-native backstop for all three |
| authentic, verifiable download | per-file SHA-256 + minisign-signed `SHA256SUMS` + published verify recipe (§6.2) — the recipe is `minisign -Vm SHA256SUMS -p docs/minisign.pub` (**lowercase `-p` = public-key file path**; uppercase `-P` expects an inline base64 string and would fail on a path — standardised across README + spec §6.2.3/§6.2.4); an **out-of-band pubkey fingerprint** anchor (the in-repo `docs/minisign.pub` TOFU is otherwise circular — an attacker serving a tampered clone swaps artifact + `SHA256SUMS` + `.minisig` + pubkey together) | **G39** checksums + minisign over `SHA256SUMS` **+ a release-tier executable assertion that RUNS the exact documented recipe** against the just-produced `SHA256SUMS` + `.minisig` + committed pubkey and fails the release on non-zero (turns "recipe present" into "recipe correct and working"); **G44** verify recipe present + literal-form match; **G39/G44 sub-assertion** that `docs/minisign.pub` matches the fingerprint published out-of-band (a pinned README via the verified GitHub web UI / org page the pipeline cannot rewrite); the key-compromise/loss + coordinated-disclosure path lives in `vuln-response.md` (the human-readable retired/compromised-key commit IS the revocation channel for an offline app) |
| §7.5 log never carries file contents / full paths | redaction in the logging layer (§7.5) | **G15/G31** (redaction property-test sub-case, homed in G31's hosted security-assertion set — parallel to the temp-ownership row below): a known secret-looking path stem fed through the logger is absent from the log |
| §2.14.1 per-run temp ownership + mode | per-run-owned scratch, `0o700` scratch root / `0o600` `.part` publish-temp | **G15/G31** temp-ownership + mode-bits assertion |
| hardened CI supply chain | least-privilege `permissions`, SHA-pinned actions, transitively recorded (a caged inventory holds every action's nested `uses:` and container image; current via `dependabot.yml` covering **github-actions, cargo, npm, and pip** ecosystems — pip for the gate-toolchain layer `requirements-ci.txt`, r7; a `docker://` step image is digest-pinned and re-pinned at the phase-end sweep), lockfile-locked builds, no fork-PR release, pinned+digest-verified gate tools, per-workflow concurrency + `timeout-minutes` | **G49** `actionlint` (L1); **G50** `zizmor` (L4); **G18a** lockfile-integrity (`--locked`/`--frozen-lockfile` + `git diff --exit-code` lockfiles); **G56** `dependabot.yml` github-actions **+ cargo + npm + pip** entries + push-workflow concurrency/timeout-minutes assertions + the action-pin inventory bijection (leg 12) |
| CI runner-host integrity (principle 11) | the secret-bearing signing job runs only on an ephemeral GitHub-hosted runner, host-isolated from the untrusted-corpus/fuzz jobs; **`step-security/harden-runner` (BLOCK mode) on the GitHub-hosted signing job ONLY** (its free/Community tier works only on GitHub-hosted runners; self-hosted needs Enterprise); on the shared self-hosted VPS the enforcement is **G42b ptrace/Landlock + G42 nftables/strace + the VPS egress allowlist + an ephemeral/JIT low-priv runner**, NOT harden-runner | **G56** workflow lint — a secret-using job bound to a self-hosted runner label is a hard fail; the corpus/fuzz job and the signing job assert disjoint runner hosts; **harden-runner presence/mode is asserted ONLY on GitHub-hosted jobs** (a self-hosted harden-runner claim would be inert on the free tier, so the lint must not require it there) |
| GitHub required-status-checks / repo config (via rulesets) | the single-branch direct-to-`main` model has no PR and no second reviewer, so the **only** thing that turns a red CI run into an actual block is repo config — invisible to the codebase and silently relaxable in the GitHub UI. Classic branch protection cannot express it: its required-checks block **all** direct pushes incl. admins (GH006), incompatible with the direct-to-`main` "red `main`, fix immediately" model — so the config is **two rulesets**: a required-checks ruleset whose bypass is the repo-**ADMIN role ONLY** (the trusted direct-pusher — a fork-PR stays gated, the owner/Build-Loop can direct-push), and a `non_fast_forward`+`deletion` ruleset with **NO** bypass (history protection for everyone). **Plus two further invisible-config settings free and load-bearing for a public repo holding the most-damaging secret:** native secret-scanning + push-protection ENABLED, and the repo's default workflow permissions = read-only (a workflow omitting a `permissions:` key inherits this) | **G56a** repo-config assertion — queries the GitHub **rulesets** API for `main` (`gh api repos/:owner/:repo/rulesets`, GET per active ruleset) and fails if the agreed required checks are not all present-and-required, if the `non_fast_forward`/`deletion` rules are absent or carry **any** bypass actor, or if a **non-admin/wider** actor can bypass the required-checks rule (the repo-admin bypassing the *checks* rule is the intended trusted-direct-pusher mechanism — this REPLACES the old "admin-bypass off" assertion the GH006 reality made unsatisfiable; owner-decided, §3 principle 10); **+ sub-checks: `secret_scanning`/`secret_scanning_push_protection` enabled and `default_workflow_permissions == "read"` + `can_approve_pull_request_reviews == false`** (fail-soft only during the P0 bootstrap box, then hard) |
| release-tag (`v*`) trust — the secret-bearing trigger G56a does NOT cover | the minisign release job (the ONLY holder of `MINISIGN_SECRET_KEY`/`MINISIGN_PASSWORD`) fires on a `v*` tag, but G56a guards only the `main` BRANCH ref — so the loop / a compromised `GITHUB_TOKEN` / a stale-or-forced tag could create a `v*` tag on a commit that never passed L4 green and mint a signed artifact; the release trigger needs the same "a red run actually blocks" guard applied to the tag ref | **G56b** release-tag trust gate — (1) a GitHub **tag-protection ruleset on `v*`** asserted via the rulesets API (only the owner/a protected actor may create release tags); (2) the release workflow's FIRST step asserts the tagged commit is an **ancestor of `origin/main`** AND **main's required checks were green for that exact SHA** (`gh api …/commits/<sha>/check-runs`), aborting before any secret is read otherwise; (3) the `v*` tag is a **signed annotated tag**, verified with `git verify-tag` against a **committed SSH allowed-signers file** (the loop signs its own release tags; the file is provisioned in P0.7 — distinct from the spec §6.7.1 requested-not-required DCO commit sign-off). Leg 1 fail-soft in the P0 bootstrap box then hard; legs 2/3 fail-closed always |
| JS/WebView supply-chain symmetry with Rust | committed `.npmrc` registry pin + resolution-URL guard (dependency-confusion defence); a frontend GPL/AGPL license deny over the pnpm graph; a committed minimal `onlyBuiltDependencies` allowlist (install-lifecycle-script lockdown). **Asymmetry stated HONESTLY (r7 — the JS-side execution lockdown is the STRONGER half, the reverse of what "symmetry with Rust" implies):** G18d BLOCKS arbitrary code via `postinstall` the moment `pnpm install` runs in CI, but the Rust side has NO equivalent execution lockdown — `build.rs` scripts and proc-macros execute arbitrary native code during `cargo build`/`test` in the SAME secret-bearing release job, with full network by default; `cargo-vet` (G18b) is a TRUST signal, not an execution sandbox (a vetted-then-compromised, or as-yet-unvetted-but-allowed, crate's `build.rs` can open a socket / exfiltrate `MINISIGN_SECRET_KEY` at build time). The mitigation: in the secret-bearing release/signing job, after `cargo fetch --locked`, run all `cargo build`/`test` with **`CARGO_NET_OFFLINE=true`** (plus the harden-runner BLOCK already mandated on that hosted job) so a `build.rs`/proc-macro cannot phone home. **Residual stated honestly: cargo cannot fully sandbox build scripts the way pnpm blocks lifecycle scripts** — offline-after-fetch + harden-runner BLOCK bound the egress, not in-process behaviour; a per-crate capability cap (`cargo-acl`/cackle) is declined for v1 (gate-status.md), so a `build.rs` or transitive crate that opens a socket outside the fetch step is the accepted residual | **G17** `osv-scanner` over `pnpm-lock.yaml` (`scripts/check-js-advisories`: offline against the Lane-A-refreshed OSV database, fail-closed at L4; **NOT** `pnpm audit`, which is online-only — see G17); **G18c** `.npmrc` registry-pin + every `pnpm-lock.yaml` resolution URL ∈ the allowed registry; **G36b** frontend GPL/AGPL license hard-fail (cdxgen SBOM → `jq`/license filter); **G18d** `onlyBuiltDependencies` allowlist-growth lint (+ fail unless `enable-pre-post-scripts`/`unsafe-perm` are each pinned `false` — pnpm reads an absent key as ON); **G56** jq-over-YAML sub-rule asserting the secret-bearing signing job sets `CARGO_NET_OFFLINE=true` after `cargo fetch --locked` (no-network-after-fetch) |
| Principle-11 English-only / string-ownership (spec §6.7.1 / §6.10 row 23 — v1 is English-ONLY, no i18n runtime) | no locale-switch / i18n-runtime library import; every `strings/ui.ts` key resolves to a non-empty English value; user-facing literals live in `strings/ui.ts` | **G57** English-only / string-ownership lint (Lane-A, activated in P1) — fails on any locale-switch/i18n import, on an empty/missing `ui.ts` key, or a user-facing literal outside `strings/ui.ts` |
| build-time reviewer-API dependency (the ONE breach of the hermetic-CI principle, named honestly) | the G1 dual review calls the **Anthropic API** on every box — the single sanctioned build-time network dependency, the one place principle 10's "hermetic CI / everything that touches the minisigned bytes" is breached. It is **OUTSIDE the minisigned-bytes boundary** because it *observes but does not produce* the artifact. **Residuals stated:** (a) it is a live network call from the build session, the only one; (b) a spoofed/compromised endpoint could emit a **forged `GO`** — but G1 is **not** a security control (every `Gnn` except G1 is the real control), so a forged GO suppresses only the human review-trace the spot-audit relies on, it cannot ship insecure code; (c) the full staged diff (incl. any committed test-fixture material) leaves the machine to a third party — acceptable for an OSS repo, but a stated fact; (d) **prompt-injection via a committed fixture** — adversarial corpus bytes added to the diff reach the reviewer as DATA, never as instructions, and even a reviewer subverted into a forged `GO` cannot ship insecure code because **G1 is not a security control** (every `Gnn` except G1 is the deterministic control), so the bound on this vector is the same "a forged GO suppresses only the review-trace, never a gate" framing as (b). Model IDs are recorded per commit (§2) | **G1** (quality amplifier, not a security control) + the auditable-smell spot-audit (§2); mitigated structurally by "every Gnn except G1 is the deterministic control" |
| release-artifact completeness (the single backstop catching "the SBOM/CVE-report/source-bundle silently didn't get attached") | every required release asset enumerated + present before publish | **G58** release-manifest completeness meta-gate — fails the release if any of the per-OS bundle, `SHA256SUMS`, `SHA256SUMS.minisig`, SBOM file(s), dated open-CVE report (G17b), `NOTICE`/`THIRD-PARTY-LICENSES`, copyleft corresponding-source bundle (G38b), measured-sizes asset, `usability-floor.md`, `name-clearance.md`, the §6.5.3 CHANGELOG/release-notes (with demoted/lossy pairs), the G59 attestation Sigstore bundle, or its paired `trusted_root.jsonl` offline-verify asset is missing |

> Every row above also lists, in [build-gates.md](build-gates.md), the concrete
> tool, the plane it runs at, and its fail-open/closed posture. The §0.11 ↔ §5
> parity check (plan-lint check 8) fails the build if **any** §0.11 class loses its
> row or a row loses its `Gnn`, so this mapping can never silently drift.

## 6. Living-doc rules

- A control or gate that changes during the build is updated **here first**, in
  the same commit as the change, with a one-line rationale.
- If a security control is *removed* or *weakened*, that is an escalation to the
  Co-Pilot session, never a silent edit.
- This doc and [build-gates.md](build-gates.md) are themselves under the
  doc-consistency gate (`plan-lint`/`spec-lint`, P0.3): every `§` reference must
  resolve and every gate named here must exist in the catalogue.
- **Doc-graph freshness is machine-enforced, not a promise (the general form of the
  spec-sync rule).** A change to **any** authoritative source — a gate (`Gnn`), a
  control, a decision, a path/directory, a convention, an enum variant, a version
  pin — must be reflected in **every** referencing doc in the **same commit** (no
  stale, no contradictory, no orphaned `.md`); the **gates→`.md` consistency case is
  ONE instance, not the scope**. **G68** (doc-graph integrity & freshness) enforces
  this graph-wide — no-orphan + cross-doc-reference resolution + the
  described-the-old-way freshness fingerprint across the **repo-scoped** graph
  `CLAUDE.md` ↔ SSOT ↔ spec ↔ security ↔ process ↔ plan (the out-of-repo `~/.claude`
  memory channel is audited once, manually, in P0.1.7 — it is outside `git ls-files`
  and not a continuous G68 node). The structural analogue is **G69**: every tracked
  directory has a row in the spec §0.7 physical tree (directories only), every §0.7
  directory is tracked, and every file on §0.7's load-bearing list is tracked — nothing
  structural lives outside §0.7; `CLAUDE.md` §1a points to it.
- **Context-routing — the autonomous loop runs LEAN; the Co-Pilot holds the full
  picture.** The [build-loop.md](../process/build-loop.md) prompt **references** this
  doc + [build-gates.md](build-gates.md) for a per-box / red-CI gate lookup but does
  **NOT inline** the whole security/gate corpus into the lean loop session (no context
  ballast). The complete gate catalogue, the threat model, and the cross-phase
  security view are the **Co-Pilot's** to carry
  ([roles-and-escalation.md](../process/roles-and-escalation.md)). This is a
  defense-in-depth property, not a convenience: a bloated loop prompt dilutes the
  per-box focus the gates depend on. The split is asserted by the P0.1.7
  documentation-wiring & context-routing audit.
- **Current state only — git is the log.** This document carries no review-round logs or history
  sections; a decision lands in the section it governs. The retired P0 review reconciliation log
  (r1–r15) is read with `git show cf97638f7280:docs/security/security-concept.md`. A provenance
  citation — a review tag such as `r7`, a `[Test-Change: …]` tag, "the P<n>.<m> note" or
  "ruling" — is a historical pointer resolved through git: never a freshness target (G68), and
  not re-pointed when its source text is retired or condensed.
