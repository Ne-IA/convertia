# ConvertIA — Security Concept (living)

> **The build-time security & guardrail concept** for ConvertIA. Defines *what we
> protect against* and *which guardrail proves it*. The companion
> [build-gates.md](build-gates.md) is the operational gate catalogue (the *how*).
>
> **Status: living.** This document is refined *during* implementation; whenever a
> control or gate changes while building, it is recorded here first. It does **not**
> override the product truth:
> **conflict order = [SSOT](../SINGLE-SOURCE-OF-TRUTH.md) > [spec](../spec/README.md) > this document.**
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
| **Co-Pilot session** (the owner's partner) | Escalation & clarification target for the Build-Loop session; strategic decisions; high-level review. Works with the owner. |

- **Single branch (`main`), GitHub, GitHub Actions.** No worktrees, no parallel
  branches, no push-lock coordination, no separate feedback/sniping sessions, **no
  merge step and no auto-merge** — the enforcement is **CI green on `main` + required
  status checks on every push** (asserted by G56a), a red `main` fixed immediately.
  The **one** surviving `PR` concept is the **external fork pull-request** (this is a
  *public* OSS repo, so an outside contributor can still open one) — the G56 fork-PR
  secret guard is retained for that reason, **not** for our own direct-to-`main` flow;
  "per-PR" vocabulary elsewhere means "per-push" unless it is explicitly that guard.
- Because only one session builds and commits, there is **no push contention** —
  ordinary `git push` is used; the safety comes from the gates, not from branch
  isolation.
- **Escalation path:** Build-Loop → Co-Pilot session → owner. The Build-Loop
  session escalates on genuine blocks (see [roles-and-escalation.md](../process/roles-and-escalation.md),
  authored in P0); it decides routine implementation/pattern/naming/default
  choices itself.

**The dual review is a quality amplifier, not a security control.** The Opus+Sonnet
review (G1) is self-attested via an unverifiable commit trailer; a gamed `GO/GO`
trailer cannot, by itself, ship insecure code, because the **only security controls
are the deterministic gates (every `Gnn` except G1, the dual review)** — a gate
either passes on a clean checkout or it does not. (The numeric span is stated as
"every gate except G1" deliberately: a frozen upper bound like *G2–G50* drifts every
time a gate is added; a `plan-lint` assertion that the prose matches `max(Gnn)` would
be the alternative, but the open phrasing cannot rot.) The dual review raises quality and catches design defects the gates can't
encode; its evidence trail (each reviewer's findings + convergence/divergence
recorded verbatim in the commit body) makes a "both GO, 0 findings" on a non-trivial
diff an **auditable smell** for periodic Co-Pilot spot-audit. Conflict order for the
Build-Loop: **SSOT > spec > these security/process docs > code > conversation.**

**Reviewer availability + integrity (build-loop soundness, authored in build-loop.md).**
Because the autonomous loop leans on G1 executing, its failure modes are explicit:
**(a)** the two reviewer **model IDs are pinned** (exact IDs, like every other tool)
so a deprecation/rename surfaces as an escalation, not a silent skip; **(b)** on a
reviewer error/timeout/rate-limit/5xx the loop retries with backoff a bounded number
of times, then **HARD-STOPS + escalates** to Co-Pilot — it **NEVER** auto-emits a `GO`
trailer with fewer than **two live** reviews and never silently degrades to a single or
zero reviewer (G12 checks the trailer is well-formed and — since r5 — that a `GO/GO` trailer
on a non-trivial diff carries each reviewer's NON-EMPTY findings block, but still cannot prove
two LIVE models ran, so this rule remains the load-bearing defence against a
well-formed-but-unbacked `GO`; the findings-block sub-check raises the cheat cost from "emit one
trailer line" to "fabricate a plausible per-reviewer finding set" and keeps a fabricated block an
auditable spot-audit target). **Correlated-blind-spot residual (stated honestly):** Opus and Sonnet share
model lineage, so "both `GO`, 0 findings" is a *correlated* signal, not two independent
ones — the only retrospective backstop is the auditable-smell spot-audit above. **Recorded
owner decision (r6 — no longer left open):** the correlated-lineage residual is **explicitly
ACCEPTED for v1** (the deterministic gates — every `Gnn` except G1 — carry the real security
weight and bound blast radius regardless of reviewer correlation; G1 is a quality amplifier),
**with a concrete spot-audit cadence: a Co-Pilot auditable-smell spot-audit at every phase
boundary AND a random ≥1-in-10-box sample** of the committed `GO/GO` findings blocks. The
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
| **L2 — pre-push hook** (fires at `git push` time) | `git push` | Git-hook manager, budget < ~3 min; cheap-commit fastpath | the push | `--no-verify` / `core.hooksPath` (**forbidden**); legitimate fastpath skips for docs-only / check-off commits |
| **L3 — commit-msg hook** (L3 fires at `git commit`, chronologically **before** L2 which fires at `git push`; numbering is stable-by-assignment, not chronological) | `git commit` | Conventional-commit format check | the commit | `--no-verify` / `core.hooksPath` (**forbidden**); git auto-subjects (merge/revert/fixup) allowed |
| **L4 — CI (GitHub Actions)** | After push | The same gates re-run on a clean checkout + the heavy gates (cross-platform build, corpus, coverage, SAST, SBOM) | a red `main` (fix immediately) | none for required checks — and **G56a** asserts in CI that the GitHub required-status-checks config exists (so "a red L4 blocks" is real repo state, not an invisible assumption) |
| **L5 — Release** | On a `v*` tag | Release workflow: SBOM + completeness, license hard-fail, copyleft-source-bundle present, **checksums + minisign over `SHA256SUMS`** (the *only* signing in scope — **not** binary code-signing/notarization, SSOT *Out of Scope*), size budget, egress/no-pollution observability gates. The **`v*` tag trigger itself is trust-gated by G56b** (tag-protection ruleset + the release job's first step asserting the tagged commit is an ancestor of `origin/main` with main's required checks green for that SHA, before any secret is read) — so a tag on a never-green commit cannot mint a signed artifact | the release | none (release-blocking); the tag trigger guarded by G56b |

**Two enforcement planes principle.** Every gate that CAN run locally runs **both
planes — locally (L1–L3) and again in CI (L4)**; inherently CI-only gates
(repo-config introspection G56/G56a/G56b, the CI-only L4 corpus/SAST/coverage heavies
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
    only thing that makes a red run actually block, so G56a queries the GitHub API and
    fails on missing required checks / enabled force-push or deletion / admin-bypass (and,
    as sub-checks, on disabled secret-scanning/push-protection or a non-read default
    workflow-permission). **The secret-bearing release trigger is the `v*` TAG ref, which
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
**16 classes**: `T1, T2, T2a, T2b, T2c, T3, T3a, T4, T5, T6, T7, T8, T9a, T9b, T10, T11`.
This table carries **one row per class** (including the `a`/`b`/`c` sub-rows),
mapping each to its primary runtime/build control **and** the concrete `Gnn` gate
that proves the control is in place. **§0.11 ↔ §5 parity is bidirectional and
machine-checked** (plan-lint check 8, §6 of [build-gates.md](build-gates.md)): every
§0.11 class has a row here, and every row cites a `Gnn`. A class with a runtime
control but no verifying gate is itself a gap.

| Threat (spec §0.11) | Primary control | Verifying gate (see [build-gates.md](build-gates.md)) |
|---|---|---|
| **T1** untrusted decoder input → crash/hang/exploit | engine isolation (§2.12) + invocation timeout/kill (§1.7) + pool bounds (§0.9); the in-core §1.2 detection layer is memory-safe Rust. **Honest residual `[DECIDED]` (stated, not implied-covered):** the §2.12.3 runtime privilege-drop (seccomp/Landlock · Seatbelt · AppContainer) is **best-effort and SILENTLY degrades** to a cheap tier — so T1 has **no LOAD-BEARING runtime containment** of an RCE inside a decoder; process-isolation bounds a *crash* (and a successful exploit's blast radius to one sandboxed sidecar + its scratch), but does not by itself stop code-exec inside that process. The load-bearing controls are isolation + the timeout/kill + the input never reaching the core; the privilege-drop tier is defence-in-depth that is *tracked* (G64 ratchet) but accepted as degradable | **G48** in-core detector fuzz (`cargo-fuzz` over `crate::detect`); **G31** per-pair corpus + reliability through the §2.12 boundary (engine-side T1 — incl. the crafted-BMP ImageMagick sentinel, the densest-CVE decoder) + the §2.12.3 **privilege-drop-tier-applied** + **memory-cap kill** + **Job-Object reap** positive assertions; **G64** privilege-drop-tier ratchet (a commit lowering an achieved tier fails/escalates — the silent-degrade can't quietly regress net coverage); **G26** engine-side adversarial corpus through the §2.12 boundary, with an **`engines.lock`↔adversarial-fixture bijection guard** (every staged engine — incl. ImageMagick — has ≥1 fault-injection fixture targeting its type, hard-fail otherwise); isolation runtime test |
| **T2** malicious / compromised WebView content | §0.10 capability allowlist (no WebView `fs`/network) + CSP (no remote origins, `object-src 'none'`); the §0.10 by-construction hardening keys asserted by G47 — **all six (r7 — the prose previously named only three, lagging the G47 gate body):** `withGlobalTauri` off, `dangerousDisableAssetCspModification` off, release `devtools` off, **`app.windows[].dangerousRemoteDomainIpcAccess` absent/empty** (a Tauri v2 knob that, if set, lets ANY remote origin invoke registered IPC commands directly, collapsing the T2 boundary), **`assetProtocol.enable` absent/false**, and **`bundle.createUpdaterArtifacts` absent/false**. **Un-pinnable runtime residual `[DECIDED]` (named, not implied-covered):** the **OS-provided WebView itself** (WebView2 Evergreen on Windows, WebKitGTK on Linux, WKWebView on macOS — a large untrusted-HTML-parsing C++ surface with its own CVE stream) is the named OS mechanism for T2 yet is **un-pinned, un-versioned, absent from the SBOM** — its integrity/CVE-currency is delegated to the OS update channel; ConvertIA's only side-bound is the G47 CSP lock (the WebView analogue of the G37c glibc-floor honesty). **JS name-trust gap (r7, un-verified residual ACCEPTED for v1):** G18c proves every `pnpm-lock` resolution URL ∈ the pinned registry but NOT package-name legitimacy — a typosquat / convincing-name malicious package hosted ON the pinned registry passes G18c entirely, where the Rust side's `cargo-vet` (G18b) closes exactly this class; residual accepted for v1, the lightweight closure (a committed npm package-name+scope allowlist diffed against `pnpm-lock.yaml`, OR Socket CLI as a non-blocking corroborator) is the §8 forward idea | **G47** CSP/capability structural lint (incl. ALL six §0.10 by-construction keys above); **G18** deny-list (no `tauri-plugin-updater`/HTTP-client crate); **G18c** registry-origin pin (name-trust gap residual noted above); **G42** offline-egress monitor (Lane-B confirmation, macOS WebView gap noted); **TAINT depth for the T2 surface (r7): G56a sub-check (f) is an OR-GATE** — it passes if EITHER CodeQL `javascript-typescript` code-scanning is ENABLED (`code-scanning/default-setup` → `state == "configured"`) OR the **G29 rule (i)** Semgrep `mode: taint` ruleset (WebView/IPC sources → DOM/eval + IPC-arg sinks) is present with its planted-positive — the inter-procedural taint the structural checks cannot reach; **owner-decidable which ships, exactly one is required (machine-enforced as an XOR by plan-lint check 21)** — so selecting the sanctioned Semgrep path can never leave a permanently-red required check (the prior hard CodeQL assert did) |
| **T2a** WebView steers a write to an attacker-chosen path | non-destructive create + write-target link-safety + divert (§2.1/§2.3.3/§2.7) | **G19/G31** fs-safety unit + property tests (no-clobber + link-safe on a WebView-supplied `ChosenRoot`); adversarial-path corpus (T7-shared) |
| **T2b** WebView re-submits an attacker-chosen SOURCE path | freeze-time §1.1 re-validation (canonicalise / resolve-identity / existence / detection at the §2.4 freeze) — provenance-independent | **G31** freeze re-validation property test (every C1 path re-validated regardless of provenance) |
| **T2c** WebView plugin-write surface (`store:default` + `log:default`) | bounded to `app_config_dir()`, no user-file contents (§7.4.2/§7.5). **Honest framing (spec §0.10):** `store:default` grants ALL store operations with **no per-file scope** — the single-`settings.json` confinement is a **code convention** (one compiled-in store name, one call site), **NOT** a runtime permission boundary; the primary enforced boundary is the §6.1.3 path-resolution assertion (the plugin cannot traverse out of `config_dir`), not a G47 scope. Worst-case harm is corrupt local prefs/log (clean reset recovers), never reading/exfil of user data | **G47** capability lint (only `store:default`/`log:default` granted, no broader `fs:`/`shell:` write surface — but G47 does **not** prove the single-store convention); **G38** §6.1.3 plugin-cannot-escape-`config_dir` path-resolution assertion (the primary boundary) **+ the G29 project-local single-store-name / single-`Store.load`-call-site Semgrep rule** (so an XSS/supply-chained second `Store.load(...)` call is caught) |
| **T3** bundled-binary supply chain | pinned + checksum-verified engines; build-time hash manifest; startup integrity. **Engine acquisition decided per engine per platform (spec §3.8, P0 review r3):** prebuilt-vs-from-source is an explicit policy because the two have different ground truths — **from-source** ⇒ the binary SHA is a build-output stability check, and provenance moves to the **signed source tarball + digest-pinned build toolchain/base image** — **but a validly-signed tarball is NOT sufficient (r7): the xz/liblzma backdoor rode a SIGNED tarball whose autotools-GENERATED files differed from git, so the anchor prefers a VCS tag/commit (`git archive` of the signed tag, `configure`/`m4` regenerated locally) or, where a tarball must be used, a diff of its non-generated sources against the upstream VCS tag; both the tarball SHA AND the VCS tag/commit are recorded in `engines.lock`** (G37); **prebuilt** ⇒ corroborate via **≥ 2 independent mirrors** OR a **distro GPG-signed package + signed repo metadata** (a bare hash of one unsigned download is unacceptable — it launders provenance, the xz/liblzma class). **FFmpeg** (which publishes no signature for the common gyan/BtbN prebuilts) has a named satisfiable anchor: from-source from the GPG-signed `ffmpeg.org` release, or a ≥ 2-provider cross-check. An **engine-source allow-list** constrains which hosts a pin (and its corroboration checksum) may come from, on independent origins. *Residual risk stated honestly:* startup integrity gives **no runtime tamper-resistance** (a whole-bundle swap swaps the in-bundle manifest too); the floor is the §6.2 SHA256SUMS + minisign anchor, not a runtime check (§3.8, §6.1.3, §6.3.4, §7.2.3). **Near-free hardening (r6 forward idea):** `build.rs` embeds `SHA-256(engine-integrity.json)` as a `const` and the startup verifier checks the on-disk manifest against it BEFORE any engine-hash lookup, so forging startup integrity requires replacing BOTH the manifest AND the binary — the lowest-cost step before the full "startup reads from the signed `SHA256SUMS`" upgrade | **G37** engine-checksum build gate (verify vs in-repo `engines.lock` before staging + on cache-restore) + **pin-establishment provenance assertion** (the spec §3.8 acquisition-mode corroboration — named satisfiable source per engine incl. FFmpeg; any `engines.lock` SHA edit is a hard Co-Pilot escalation) + **engine-source allow-list assertion** (every `engines.lock` source URL ∈ the committed per-engine origin allow-list, pin URL and corroboration URL on independent origins); **G35** SBOM completeness; **G46** startup integrity verification; **G17b** *(informational, planted-positive self-tested)* OSV/grype CVE scan over `engines.lock` (PURL-keyed; the **FFmpeg CPE is MANDATORY** — the highest-CVE surface is FFmpeg's enabled internal decoders, which `pkg:generic` PURLs miss; the planted-positive uses a historical INTERNAL-decoder CVE) |
| **T3a** DLL/dylib/`.so` side-loading of a bundled codec shared object beside the engine `.exe` | every staged shared object individually `engines.lock`-rowed with its SHA-256 + verified before staging; engines spawned with a minimal explicit `PATH` (the bundle dir only, so the OS DLL/dylib search starts inside the bundle) + the §3.5 loader-injection-var strip (`LD_PRELOAD`/`LD_LIBRARY_PATH`/`DYLD_*` cleared); a staging-time dynamic-dependency-closure check that every non-system dependency resolves **inside** the bundle | **G37** per-shared-object SHA-256 verify (each `.dll`/`.dylib`/`.so`, not just the primary engine binary); **G35** manifest diff hard-fails on a staged shared object not matching its `engines.lock` row; **G37b** dynamic-dependency-closure assertion (`ldd`/`readelf -d` Linux · `otool -L` **AND `otool -l`** macOS — the `-l` leg enumerates `LC_RPATH`/`LC_LOAD_DYLIB` so an `@rpath` resolving OUTSIDE the bundle (Homebrew/`/usr/local`) is caught, which `otool -L` alone misses · `dumpbin /dependents` Windows — every non-system dep resolves inside the bundle, catching both a side-loading vector and an offline-floor break where an engine links a Homebrew/distro lib present only on the build runner) |
| **T4** open-file launch of a fresh artifact | §7.7 open-file safety (reveal-in-folder, no auto-open) + §7.7.3 Rust-side `RunResult`-membership check | **G15/G31** membership-check unit + integration test (only a current-run result path may be opened) |
| **T5** core panic / app fault | §2.13 app-level fault model (`catch_unwind` worker boundary) + §7.2 startup faults + §0.3.1 WebView-absent handling | **G15** panic-boundary unit test (panic → app-fault, not crash); **G46** missing/corrupt-engine → app-fault acceptance |
| **T6** copyleft aggregation boundary | §3.6 copyleft isolation (separately-invoked binaries, aggregation not linking); §0.3/§0.7 subprocess model | **G18** `cargo-deny` GPL/AGPL ban on the Rust crate graph; **G36** SBOM forbidden-family hard-fail; **G38b** LGPL-relink + GPL-corresponding-source bundle-present assertion (§6.1.3 ii/iii incl. x265 GPL §3); **G53** core-crate forbidden-dependency check (`cargo-deny [bans]` workspace-member-scoped — no image-worker C libs in the core closure) |
| **T7** path / link redirection (symlink/junction/TOCTOU) | §2.3 resolved-identity & link safety + §2.1 exclusive create-new-or-fail on the resolved real file | **G19/G31** atomic-publish/fs-safety unit + property tests; adversarial-path corpus |
| **T8** self-feeding / batch expansion | §2.4 frozen source set + §7.1 instance/run identity | **G15** frozen-set + per-run-ownership unit tests (the data-structure leg); **G31** T8 INTEGRATION sub-test (the live-path leg) — a batch whose conversion writes outputs INTO the same watched/dropped source folder mid-run asserts the fresh outputs are **NOT** in the run's ingest/result set (snapshot-not-live-iteration, §2.4.2), and a two-instance fixture where instance B drops a `file`/`*.part` into a shared folder mid-run asserts instance A's frozen set never grows (§2.4.3 concurrent-instance hand-off) |
| **T9a** ConvertIA's own code exfiltrates user files | structural: opens no socket — no HTTP/updater on the §0.10 allowlist, no remote `connect-src`, no phone-home (§7.6) | **G29 project-local rules (g)+(j)** the per-push Rust-source net-ban — **(g)** `std::net`/`tokio::net`-outside-allow-list (the import-site path) **and (j)** `libc::socket`/`libc::connect`/`nix::sys::socket`-outside-allow-list (the raw-syscall FFI path the imgworker's `#[allow(unsafe_code)]` surface allows, which (g) structurally misses) — together the per-push structural proof that first-party Rust opens no socket by EITHER path, catching the renamed/transitive crate G18's name-based ban misses + **G47** CSP/capability lint + **G18** HTTP-client deny-list (no socket-opening dep ships); **G42** packet-monitor / egress-deny release gate (the release-tier proof) |
| **T9b** bundled engine reaches out / reads out-of-input on hostile input (incl. the LibreOffice macro-execution / `WEBSERVICE()`-external-data vectors) | load-bearing argv/build controls: FFmpeg `-protocol_whitelist file,pipe` + curated demuxers + `concat -safe 1`; pandoc `--sandbox` (pandoc ≥ 2.15 — `--sandbox` shipped in 2.15 and is honoured from 2.15 on, enforced by pandoc's type system; spec §3.5.4); LibreOffice hardened profile (`-env:UserInstallation=file://<per-job scratch>` disposable profile, asserted by the G29 argv rule + G31 — without it LibreOffice re-uses the user/home profile, a T9b read-half + T8 cross-job leak) (`MacroSecurityLevel = 3` + `DisableMacrosExecution = true`, `LinkUpdateMode = 0`, no external-data-range / `WEBSERVICE()` refresh on load, §3.5.2); librsvg **no base URL** (§3.5.x); **poppler `pdftotext` built WITHOUT network/HTTP** (no `curl`/`libsoup` in its dynamic closure — a crafted PDF can carry `GoToR`/remote-action / annotation URIs; asserted by G38 + a G31 remote-URI-PDF corpus sentinel; r7 — poppler was omitted from this control enumeration); **ImageMagick hardened policy** — a bundled `policy.xml` (consulted via `MAGICK_CONFIGURE_PATH` set by the imgworker, since MagickCore reads it from there) setting `<policy domain="coder" rights="none" pattern="{URL,HTTPS,HTTP,FTP,EPHEMERAL,MVG,MSL,TEXT,LABEL,SHOW,WIN,PLT}">` + a path-rights deny on `@`-indirect reads, OR equivalently a trimmed IM build compiled with those coders/delegates excluded (the historically most CVE-dense decoder family: ImageTragick CVE-2016-3714 + the URL/MSL/MVG coder SSRF/LFR/RCE class; ImageMagick is statically linked inside `convertia-imgworker`, §3.5.5, so the §2.12 worker isolation bounds blast radius but is the degradable §2.12.3 tier, NOT the structural T9b control — the load-bearing control is this policy/coder lockdown). **Two load-bearing halves, each with its own armed enforcement substrate:** (a) **zero outbound packets** (the egress half) and (b) **no out-of-input FILE READ** (the read half — symmetric, not a mere oracle). **The structural T9b/T1 controls are bundled CONFIG, not binaries (r7):** the hardened `policy.xml` and the `registrymodifications.xcu` each carry their own `engines.lock` SHA-256 row (verified by G37, counted by Syft G35) and are hashed by the §7.2.3 startup integrity check — a 0-byte `policy.xml` / damaged `.xcu` MUST fault, not silently disarm the lockdown | **G38** per-engine build assertions (`ffmpeg -protocols`/`-demuxers`, librsvg no-base-URL, `pandoc --version ≥ 2.15`, the **poppler `pdftotext`-built-without-network** introspection + the remote-URI-PDF G31 sentinel (r7), the **LibreOffice profile assertion** — parse the shipped `registrymodifications.xcu` and assert `MacroSecurityLevel`/`DisableMacrosExecution`/`LinkUpdateMode` + the external-data keys, and the **ImageMagick hardened-policy assertion** — the bundled `policy.xml` denies the dangerous coders `{URL,HTTPS,HTTP,FTP,EPHEMERAL,MVG,MSL,TEXT,LABEL,SHOW,WIN,PLT}` + the `@`-indirect-read path-rights deny, OR the trimmed IM build was compiled coder-/delegate-excluded — see the ImageMagick T9b/T1 row); **G29** the per-spawn argv/env safety rule (incl. the imgworker `MAGICK_CONFIGURE_PATH`-points-at-the-bundle-policy assertion + the LibreOffice `-env:UserInstallation` disposable-profile presence + value check + the pandoc `--resource-path <scratch-only>` value check); **G31** corpus sentinels (a `.docm`/`.xlsm`/`.pptm` AutoOpen/`Workbook_Open` macro writing a canary inside the egress-deny window → canary **NOT** created; a `WEBSERVICE()` `.xlsx` → no egress/no out-of-input read; **a crafted BMP / SVG-via-MSL/URL-coder → no egress + no out-of-input read inside the G42/G42b window**; **a poppler remote-URI-annotation `.pdf` → no packet inside the egress-deny window (r7 — the poppler sentinel was documented in G38 but missing from this T9b verifying-gates cell and the G31 sentinel host-list)**), pulled forward into the per-push L4 leg; **G42** release-confirmation adversarial-egress monitor (the EGRESS half — zero outbound packets, armed-window canary, fail-closed); **G42b** the read-half fs-audit enforcement substrate (spec §6.4.2 — `ptrace`/Landlock, fail-CLOSED when neither is available, out-of-input sentinel + planted-positive, symmetric with G42 so the read half can never silently no-enforce **on the Linux leg**; macOS/Windows rest on the degradable §2.12.3 Seatbelt/AppContainer tier + the G31 sentinel oracle — see §10/§11 residual) |
| **T10** resource exhaustion / DoS-by-input | §1.10 resource pre-flight & budgets (incl. an **output/scratch BYTE budget** — r7: the decompression-bomb defence covers the in-core detect path + the §1.10 RAM / §2.12.3 memory-cap-kill, but a decoder that slowly explodes a 1 KB input into a 50 GB intermediate WITHIN its memory/time budget exhausts the scratch DISK, an exhaustion axis the RAM/handle/time budgets miss; the output/scratch byte budget kills to a clean `Failed(TooBig)` when decoded output exceeds N× input or an absolute scratch ceiling) + §0.9 pool/handle bounds + the to-GIF guardrail | **G16/G31** adversarial resource-budget corpus + property tests (oversized-render SVG, over-duration to-GIF, over-cardinality batch → fail-clearly, batch continues, no handle/RAM exhaustion); decompression-bomb fixtures (svgz/ZIP-in-OPC/nested-flate); **an output/scratch-byte-budget adversarial sub-case** (a bomb whose DECODED output exceeds N× input or the absolute scratch ceiling → killed to a clean `Failed(TooBig)`, batch continues, scratch returns to baseline) |
| **T11** macOS engine-as-first-TCC-accessor (silent-deny) | §3.5.0/§7.2.6 macOS TCC source staging — the Rust core (holding the TCC grant from §1.1 freeze) copies a TCC-protected source into a per-job kind-2 scratch path (§2.14.2) **before** spawning, so a sidecar is never the first process to touch Desktop/Documents/Downloads/removable media (a chain-break otherwise triggers an invisible TCC denial / wrong-process prompt that defeats the conversion, and is silent on CI which runs from `TMPDIR`) | **G31** macOS sub-test (the Rust core PID, not the engine PID, is the first accessor of the protected path; the engine receives a kind-2 scratch path); **G29** Semgrep rule (every `Command::new` in `crate::isolation` under `cfg(target_os="macos")` is preceded by the stage-for-TCC / scratch-staging call) |

**Cross-cutting build/release controls** (not §0.11 threat classes, but
load-bearing security guarantees with their own gates):

| Guarantee | Primary control | Verifying gate |
|---|---|---|
| credential / secret in repo | no secrets committed; the real CI secrets are **THREE**: `MINISIGN_SECRET_KEY` **and** `MINISIGN_PASSWORD` (§6.2.3) **and** the build-session reviewer-API `ANTHROPIC_API_KEY` (`sk-ant-…`, the §5 reviewer-API dependency row) | **G2** `gitleaks` secrets scan using the **current** subcommands (`gitleaks` v8.19+ deprecated `protect`/`detect`): L1 `gitleaks git --staged` + **L2** `gitleaks git` over the unpushed range `@{u}..HEAD` (the last local catch before a secret goes public; **excluded from the docs-only fastpath** — secrets in `.md` count) + L4 full-tree `gitleaks dir` + a release-tier **full-history `gitleaks git`** leg over the commit log (a once-committed-then-removed secret stays live forever in a **public** OSS repo with no PR-review backstop). All THREE secrets are named + provably caught + planted-positived: a committed `.gitleaks.toml` carries a **custom rule matching the minisign secret-key shape** (the `untrusted comment:` header co-occurring with a long base64 blob / a banned `*.key` staging) — `gitleaks`' default PEM rule keys on `-----BEGIN…-----` delimiters and a minisign key has none, so the PEM rule alone would **miss** it; **`ANTHROPIC_API_KEY` is caught by `gitleaks`' bundled Anthropic-key rule (`sk-ant-` prefix) — confirmed present + a planted-positive**; **`MINISIGN_PASSWORD` is a free-form passphrase that relies only on entropy + the `MINISIGN_PASSWORD` env-name scan**, so a committed `MINISIGN_PASSWORD=<value>` literal line is additionally banned by a custom rule. The G56a secret-scanning + push-protection sub-check is the free GitHub-native backstop for all three |
| authentic, verifiable download | per-file SHA-256 + minisign-signed `SHA256SUMS` + published verify recipe (§6.2) — the recipe is `minisign -Vm SHA256SUMS -p docs/minisign.pub` (**lowercase `-p` = public-key file path**; uppercase `-P` expects an inline base64 string and would fail on a path — standardised across README + spec §6.2.3/§6.2.4); an **out-of-band pubkey fingerprint** anchor (the in-repo `docs/minisign.pub` TOFU is otherwise circular — an attacker serving a tampered clone swaps artifact + `SHA256SUMS` + `.minisig` + pubkey together) | **G39** checksums + minisign over `SHA256SUMS` **+ a release-tier executable assertion that RUNS the exact documented recipe** against the just-produced `SHA256SUMS` + `.minisig` + committed pubkey and fails the release on non-zero (turns "recipe present" into "recipe correct and working"); **G44** verify recipe present + literal-form match; **G39/G44 sub-assertion** that `docs/minisign.pub` matches the fingerprint published out-of-band (a pinned README via the verified GitHub web UI / org page the pipeline cannot rewrite); the key-compromise/loss + coordinated-disclosure path lives in `vuln-response.md` (the human-readable retired/compromised-key commit IS the revocation channel for an offline app) |
| §7.5 log never carries file contents / full paths | redaction in the logging layer (§7.5) | **G15** (redaction property-test sub-case): a known secret-looking path stem fed through the logger is absent from the log |
| §2.14.1 per-run temp ownership + mode | per-run-owned scratch, `0o700` scratch root / `0o600` `.part` publish-temp | **G15/G31** temp-ownership + mode-bits assertion |
| hardened CI supply chain | least-privilege `permissions`, SHA-pinned actions (current via `dependabot.yml` covering **github-actions, cargo, npm, and pip** ecosystems — pip for the gate-toolchain layer `requirements-ci.txt`, r7), lockfile-locked builds, no fork-PR release, pinned+digest-verified gate tools, per-workflow concurrency + `timeout-minutes` | **G49** `actionlint` (L1); **G50** `zizmor` (L4); **G18a** lockfile-integrity (`--locked`/`--frozen-lockfile` + `git diff --exit-code` lockfiles); **G56** `dependabot.yml` github-actions **+ cargo + npm + pip** entries + push-workflow concurrency/timeout-minutes assertions |
| CI runner-host integrity (principle 11) | the secret-bearing signing job runs only on an ephemeral GitHub-hosted runner, host-isolated from the untrusted-corpus/fuzz jobs; **`step-security/harden-runner` (BLOCK mode) on the GitHub-hosted signing job ONLY** (its free/Community tier works only on GitHub-hosted runners; self-hosted needs Enterprise); on the shared self-hosted VPS the enforcement is **G42b ptrace/Landlock + G42 nftables/strace + the VPS egress allowlist + an ephemeral/JIT low-priv runner**, NOT harden-runner | **G56** workflow lint — a secret-using job bound to a self-hosted runner label is a hard fail; the corpus/fuzz job and the signing job assert disjoint runner hosts; **harden-runner presence/mode is asserted ONLY on GitHub-hosted jobs** (a self-hosted harden-runner claim would be inert on the free tier, so the lint must not require it there) |
| GitHub branch-protection / required-status-checks config | the single-branch direct-to-`main` model has no PR and no second reviewer, so the **only** thing that turns a red CI run into an actual block is repo config (required status checks present + required, force-push/deletion disabled, admin-bypass off) — invisible to the codebase and silently relaxable in the GitHub UI; **plus two further invisible-config settings free and load-bearing for a public repo holding the most-damaging secret:** native secret-scanning + push-protection ENABLED, and the repo's default workflow permissions = read-only (a workflow omitting a `permissions:` key inherits this) | **G56a** branch-protection config assertion — queries the GitHub ruleset/branch-protection API for `main` (`gh api repos/:owner/:repo/branches/main/protection` or the rulesets API) and fails if the agreed required checks are not all present-and-required, or `allow_force_pushes`/`allow_deletions`/admin-bypass is enabled; **+ sub-checks: `secret_scanning`/`secret_scanning_push_protection` enabled and `default_workflow_permissions == "read"` + `can_approve_pull_request_reviews == false`** (fail-soft only during the P0 bootstrap box, then hard) |
| release-tag (`v*`) trust — the secret-bearing trigger G56a does NOT cover | the minisign release job (the ONLY holder of `MINISIGN_SECRET_KEY`/`MINISIGN_PASSWORD`) fires on a `v*` tag, but G56a guards only the `main` BRANCH ref — so the loop / a compromised `GITHUB_TOKEN` / a stale-or-forced tag could create a `v*` tag on a commit that never passed L4 green and mint a signed artifact; the release trigger needs the same "a red run actually blocks" guard applied to the tag ref | **G56b** release-tag trust gate — (1) a GitHub **tag-protection ruleset on `v*`** asserted via the rulesets API (only the owner/a protected actor may create release tags); (2) the release workflow's FIRST step asserts the tagged commit is an **ancestor of `origin/main`** AND **main's required checks were green for that exact SHA** (`gh api …/commits/<sha>/check-runs`), aborting before any secret is read otherwise; (3) the `v*` tag is a **signed annotated tag**, verified with `git verify-tag` against a **committed SSH allowed-signers file** (the loop signs its own release tags; the file is provisioned in P0.7 — distinct from the spec §6.7.1 requested-not-required DCO commit sign-off). Leg 1 fail-soft in the P0 bootstrap box then hard; legs 2/3 fail-closed always |
| JS/WebView supply-chain symmetry with Rust | committed `.npmrc` registry pin + resolution-URL guard (dependency-confusion defence); a frontend GPL/AGPL license deny over the pnpm graph; a committed minimal `onlyBuiltDependencies` allowlist (install-lifecycle-script lockdown). **Asymmetry stated HONESTLY (r7 — the JS-side execution lockdown is the STRONGER half, the reverse of what "symmetry with Rust" implies):** G18d BLOCKS arbitrary code via `postinstall` the moment `pnpm install` runs in CI, but the Rust side has NO equivalent execution lockdown — `build.rs` scripts and proc-macros execute arbitrary native code during `cargo build`/`test` in the SAME secret-bearing release job, with full network by default; `cargo-vet` (G18b) is a TRUST signal, not an execution sandbox (a vetted-then-compromised, or as-yet-unvetted-but-allowed, crate's `build.rs` can open a socket / exfiltrate `MINISIGN_SECRET_KEY` at build time). The mitigation: in the secret-bearing release/signing job, after `cargo fetch --locked`, run all `cargo build`/`test` with **`CARGO_NET_OFFLINE=true`** (plus the harden-runner BLOCK already mandated on that hosted job) so a `build.rs`/proc-macro cannot phone home. **Residual stated honestly: cargo cannot fully sandbox build scripts the way pnpm blocks lifecycle scripts** — offline-after-fetch + harden-runner BLOCK bound the egress, not in-process behaviour; the full cap is the owner-decidable `cargo-acl`/cackle (§8) | **G17** `osv-scanner` over `pnpm-lock.yaml` (offline-tolerant; **NOT** `pnpm audit`, which is online-only and broken on pnpm 10.x — see G17); **G18c** `.npmrc` registry-pin + every `pnpm-lock.yaml` resolution URL ∈ the allowed registry; **G36b** frontend GPL/AGPL license hard-fail (cdxgen SBOM → `jq`/license filter); **G18d** `onlyBuiltDependencies` allowlist-growth lint (+ fail if `enable-pre-post-scripts`/`unsafe-perm` is set); **G56** jq-over-YAML sub-rule asserting the secret-bearing signing job sets `CARGO_NET_OFFLINE=true` after `cargo fetch --locked` (no-network-after-fetch) |
| Principle-11 English-only / string-ownership (spec §6.7.1 / §6.10 row 23 — v1 is English-ONLY, no i18n runtime) | no locale-switch / i18n-runtime library import; every `strings/ui.ts` key resolves to a non-empty English value; user-facing literals live in `strings/ui.ts` | **G57** English-only / string-ownership lint (Lane-A, activated in P1) — fails on any locale-switch/i18n import, on an empty/missing `ui.ts` key, or a user-facing literal outside `strings/ui.ts` |
| build-time reviewer-API dependency (the ONE breach of the hermetic-CI principle, named honestly) | the G1 dual review calls the **Anthropic API** on every box — the single sanctioned build-time network dependency, the one place principle 10's "hermetic CI / everything that touches the minisigned bytes" is breached. It is **OUTSIDE the minisigned-bytes boundary** because it *observes but does not produce* the artifact. **Residuals stated:** (a) it is a live network call from the build session, the only one; (b) a spoofed/compromised endpoint could emit a **forged `GO`** — but G1 is **not** a security control (every `Gnn` except G1 is the real control), so a forged GO suppresses only the human review-trace the spot-audit relies on, it cannot ship insecure code; (c) the full staged diff (incl. any committed test-fixture material) leaves the machine to a third party — acceptable for an OSS repo, but a stated fact; (d) **prompt-injection via a committed fixture** — adversarial corpus bytes added to the diff reach the reviewer as DATA, never as instructions, and even a reviewer subverted into a forged `GO` cannot ship insecure code because **G1 is not a security control** (every `Gnn` except G1 is the deterministic control), so the bound on this vector is the same "a forged GO suppresses only the review-trace, never a gate" framing as (b). Model IDs are pinned (§4) | **G1** (quality amplifier, not a security control) + the auditable-smell spot-audit (§4); mitigated structurally by "every Gnn except G1 is the deterministic control" |
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

## 7. Reconciled during P0 review r1

- **§0.11 ↔ §5 parity (closed).** §5 carries one row per §0.11 class. **(FROZEN r1
  snapshot — superseded by the authoritative 16-class set in §5 / §8 / §9 / plan-lint
  check 8; this enumeration is historical and intentionally not updated.)** At r1 the 14
  classes `T1, T2, T2a, T2b, T2c, T3, T4, T5, T6, T7, T8, T9a, T9b, T10` (r2 added
  **T3a** → 15; r3 added **T11** → **16**; see §8/§9) — each naming a primary control
  **and** a concrete `Gnn`. Bidirectional parity is enforced mechanically by plan-lint
  check 8.
- **Every class has a *verifying* gate (closed).** Runtime-only controls were
  given adversarial gates by construction: T2/T2a/T2c → the new structural CSP/
  capability lint **G47**; T1 in-core path → the new in-core detector fuzz **G48**;
  T10 → the adversarial resource-budget corpus (G16/G31); T6 → the new corresponding-
  source bundle-present assertion **G38b**. CI supply-chain hardening (token scope,
  SHA-pinning, workflow lint) added as **G49/G50/G18a**.
- **Living-doc/spec-sync note.** Where a gate added here is not yet named in the
  spec (CSP-lint, the in-core detector fuzz harness, any SAST layer), the spec is
  updated in the **same change** per the SSOT > spec > docs conflict order — these
  are not silent doc-only inventions.

## 8. Reconciled during P0 review r2

- **CI runner-host integrity (principle 11, new).** The single most damaging secret
  (`MINISIGN_SECRET_KEY`/`MINISIGN_PASSWORD`) must never share a host with the
  untrusted Lane-B corpus/fuzz inputs that run on the shared self-hosted VPS
  (spec §6.1.4/§6.7.2) — the signing step runs on an ephemeral GitHub-hosted runner,
  host-isolated, hardened. New gate **G56** asserts it structurally; spec §6.7.2 is
  synced in the same change to bind the signing step to a hosted runner.
- **T3a DLL/dylib side-loading (new §0.11 class).** Bundled codec shared objects
  beside the engine `.exe` are now individually `engines.lock`-rowed + SHA-verified
  (G37), manifest-diff-guarded (G35), and dynamic-dependency-closure-checked (G37b);
  engines spawn with a bundle-only `PATH`. §0.11 grows from 14 to 15 classes — the
  spec threat map + plan-lint check 8 enumeration are synced in the same change.
- **Honest threat-map framing.** T3 now states its residual risk (no runtime
  tamper-resistance) + the pin-establishment provenance requirement (the xz/liblzma
  class) rather than implying full coverage; the download-trust row names the
  out-of-band fingerprint anchor + the key-compromise runbook.
- **Secrets gate corrected.** G2's PEM-rule claim was factually wrong for a minisign
  key (no `-----BEGIN-----` envelope); a committed custom rule + a full-history scan
  leg + `MINISIGN_PASSWORD` are added.
- **JS supply-chain parity (G17/G18c/G18d/G36b).** The WebView tree — the entire T2
  attack surface — gains a registry pin + resolution-URL guard, a GPL/AGPL license
  hard-fail, and an install-lifecycle-script lockdown, matching the Rust-side
  `cargo-deny`/`cargo-vet` discipline.
- **Boundary statement de-frozen.** "G2–G50" → "every gate except G1" (at r2 the catalogue
  reached G56 — it has since grown past G67; the open phrasing deliberately does not freeze a
  numeric upper bound, plan-lint check 11 enforces that — and G53/G55 ARE security controls).
- **Living-doc/spec-sync (r2).** The LibreOffice macro/profile build assertion + the
  `WEBSERVICE()` sentinel (G38/G31), the pandoc `--sandbox` version-floor (G38), the
  PURL/SHA-256 `engines.lock` schema fields, the `engines.lock` pin-provenance rule,
  and the §2.12.4 in-core-surface reconciliation are owned by the spec and synced in
  the same change (SSOT > spec > docs).

## 9. Reconciled during P0 review r3

- **T11 macOS engine-as-first-TCC-accessor (new §0.11 class).** The load-bearing
  §3.5.0/§7.2.6 invariant (the core stages a TCC-protected source into kind-2 scratch
  before any spawn, so the engine never first-touches a protected path) had no threat
  row and no verifying gate — it is now a §0.11 class (spec synced) with G31 (first-
  accessor sub-test) + a G29 Semgrep rule. §0.11 grows 15 → **16**; check 8 enumeration
  updated.
- **Spec §6.7 single-branch reconciliation.** Spec §6.7/§6.7.1 carried
  branch-protection/auto-merge/before-merge wording that contradicted the single-branch
  model the whole gate system rests on (and the spec outranks these docs). The spec was
  edited first (living-doc rule): Lane A is "per-push validation on `main`"; "cannot
  merge"/"before merge to main"/"auto-merge" removed; enforcement is CI-green-on-`main`
  + required-status-checks-on-push. The only surviving `PR` is the external fork-PR
  secret guard (G56), justified by external contributors.
- **`forbid(unsafe_code)` → `deny(unsafe_code)` (G29 + spec §6.4.2).**
  `#![forbid(unsafe_code)]` is un-overridable, so it cannot coexist with an
  allow-listed FFI module — invalid Rust for the FFI-heavy imgworker/core. Corrected to
  `#![deny(unsafe_code)]` at root + `#[allow(unsafe_code)]` on exactly the one FFI
  module; `forbid` reserved for a pure-logic zero-unsafe sub-crate.
- **Minisign verify recipe corrected.** `-P <docs/minisign.pub>` (inline-string flag
  fed a file path → fails) → `-p docs/minisign.pub` (file-path flag) across README,
  spec §6.2.3/§6.2.4; G39/G44 now RUN the literal recipe.
- **New gates this round:** **G56a** (branch-protection config assertion — the invisible
  repo state that makes a red L4 block), **G57** (Principle-11 English-only / string-
  ownership lint — spec-DECIDED + plan-named but previously catalogue-less), **G58**
  (release-artifact completeness meta-gate). **G42** gains a planted-positive canary
  (proves the deny window + monitor are armed), **G37** gains the engine-acquisition
  provenance + engine-source allow-list (the FFmpeg worst-case anchor), **G2** moves to
  current `gitleaks git`/`dir` subcommands + an L2 leg, **G29** gains ASAN/UBSan/Miri +
  the single-store-name + Command-surface project-local rules.
- **Build-loop dual-review soundness.** Pinned reviewer model IDs, reviewer-
  unavailability HARD-STOP (never a `GO` with < 2 live reviews), and the same-vendor
  correlated-blind-spot residual are stated (§4, authored in build-loop.md).
- **Living-doc/spec-sync (r3).** Spec §6.7 (single-branch), spec §6.4.2 (`deny` unsafe
  policy), spec §6.2.3/§6.2.4 (minisign `-p`), spec §3.8 (engine acquisition mode +
  engine-source allow-list), spec §0.11 (T11) are all owned by the spec and edited in
  this same change (SSOT > spec > docs).

## 10. Reconciled during P0 review r4

- **T9b read half got its own first-class gate (G42b).** The egress half (G42) had the full
  treatment (per-OS monitor + armed-window canary + fail-closed `::error::`); the "no
  out-of-input file read" half existed only as a corpus oracle and could silently no-enforce.
  **G42b** now mirrors spec §6.4.2 exactly: `ptrace` (`docker --cap-add SYS_PTRACE` / native
  on the VPS) primary, the §2.12.3 Landlock `{input ro, scratch rw}` grant-is-enforcement
  fallback, the mandatory Landlock-ABI/kernel≥5.13 probe, FAIL-CLOSED + the diagnosable
  `::error::` when neither is available, an out-of-input sentinel fixture + a planted-positive,
  and the §6.1.4 runner-kernel-version prerequisite (recorded in P0.7). §5 T9b cites G42+G42b.
- **plan-lint check 13 made satisfiable.** The "`Cargo.lock` set == capability-file plugin
  blocks" cross-check was unsatisfiable (dialog/opener/single-instance carry zero capability
  grants by §0.10), so it would hard-fail every clean checkout. Split into two independent
  assertions (allowlist membership + capability entries reference only allowlisted plugins).
- **G47 hardened with three §0.10 by-construction keys** — `app.withGlobalTauri` absent/false,
  `app.security.dangerousDisableAssetCspModification` absent/empty, release-profile `devtools`
  off — real Tauri v2 knobs that widen T2; spec §0.10 synced FIRST (living-doc rule), and the
  stale spec §0.7 main.json directory comment (dialog/opener) corrected at the same conflict tier.
- **harden-runner self-hosted reality.** The free/Community tier works only on GitHub-hosted
  runners (self-hosted needs Enterprise) — principle 11(c), the §5 row, G56, P0.2 and spec
  §6.7.2/§6.4-era wording corrected so the self-hosted VPS leg relies on G42/G42b + the VPS
  egress allowlist + an ephemeral/JIT low-priv runner, and G56 asserts harden-runner only on
  GitHub-hosted jobs.
- **New gates this round:** **G42b** (T9b read-half fs-audit), **G64** (privilege-drop-tier
  ratchet — the most consequential runtime security property, decrease-guarded), **G65**
  (reserved — engine-subprocess coverage-guided fuzz, owner-decidable). plan-lint **checks 14**
  (DoD-list parity), **15** (hard-stop-number parity), **16** (every fail-closed §5 gate has a
  registered planted-positive). T1 honest residual (no load-bearing runtime containment) +
  T2 OS-WebView un-pinnable residual stated; the **Anthropic reviewer API** named as the one
  sanctioned build-time network dependency outside the minisigned-bytes boundary.
- **Gate hardening this round.** General build-cache-poisoning policy (G37/G56); advisory-DB
  staleness floor (G17/G17b); bundled fonts as first-class `engines.lock` rows + OFL/Liberation
  provenance (G35/G36/G36b); mandatory FFmpeg CPE + internal-decoder planted-positive (G17b);
  from-source signing-key verification with `gpg`/`sq` against pinned keys + committed
  cargo-vet `imports.lock` (G37/G18b); G48 gained the fs_guard Windows dangerous-path classes,
  NUL/`PATH_MAX`+1 + zip-slip bound-firing fixtures, the IPC-handler serde-boundary target, and
  the imgworker FFI target; the OWASP Semgrep pack (G29); the broadened deferral vocabulary
  (G8); the G54 push-from-stale-base guard; the per-finding suppression ledger + the L4
  self-test-prelude (G24); macOS `otool -l` LC_RPATH (G37b); the process-scoped Windows
  socket snapshot + named-pipe tool + ETW privilege/fail-closed (G42); the minisign-fingerprint
  consistency (G44) + corpus-provenance (G24a) sub-checks; attest-build-provenance verify +
  offline bundle asset (the gate id **G59** was assigned in r5 — see §11).
- **Living-doc/spec-sync (r4).** Spec §0.10 (the three by-construction hardening keys),
  spec §0.7 (main.json directory comment), spec §6.7.2 + §6 CI-hardening (harden-runner
  runner-type reality), spec §2.14 (scratch-residue confidentiality accepted-residual) are
  owned by the spec and edited in this same change (SSOT > spec > docs).

## 11. Reconciled during P0 review r5

- **Release-tag (`v*`) trust gate (G56b, new).** The secret-bearing minisign release job fires on a
  `v*` tag, but G56a guarded only the `main` BRANCH ref — so the loop / a compromised `GITHUB_TOKEN`
  / a stale-or-forced tag could create a `v*` tag on a never-green commit and mint a signed artifact.
  G56b applies the same "a red run actually blocks" guard to the tag ref: a `v*` tag-protection
  ruleset + a release-workflow first step asserting the tagged commit is an ancestor of `origin/main`
  with main's required checks green for that SHA (before any secret is read) + signed/annotated-tag
  verify. The single highest-value gap closed this round.
- **T8 got its first integration/corpus gate (G31).** T8 was the only adversarial input-class with a
  unit-only verifying gate; G31 now hosts the live-walk leg (output-written-into-source-folder
  mid-batch → not in the ingest/result set, §2.4.2; a two-instance fixture where B drops a `*.part`
  mid-run → A's frozen set never grows, §2.4.3), listed in the §5 T8 row + the G31 host-list + the
  P0.5 homes. G15 stays the data-structure unit leg.
- **attest-build-provenance status drift resolved (G59).** It carried three contradictory statuses
  (§8 `[DEFER]` / P0.7 "v1 OWNER DECISION" / §10 "done"). Resolved to ONE: adopted-for-v1, catalogue
  row **G59** (verify-on-runner + offline Sigstore bundle asset, in the G58 enumeration), §8
  `[DEFER]` removed, the P0.7 "only if later adopted" hedge removed, `id-token: write` scoped to the
  release job only. New plan-lint **check 17** asserts the four homes agree.
- **G42 macOS fallback corrected (tool fix).** `nettop -m tcp -P` observes socket STATE
  (`ESTABLISHED`), not the connect() ATTEMPT, so a blocked canary records nothing and the gate's own
  armed-window self-test would spuriously fail closed on every macOS fallback run; `-m tcp` also
  blinds it to UDP/DNS. Replaced with interface-level `tcpdump -i lo0 -n` + `tcpdump -i en0 -n`
  (ships on the macOS runner, captures SYN/RST + UDP DNS even for a blocked connection), same
  fail-closed posture if absent.
- **T9a per-push Rust-source proof (G29 rule (g)).** The `std::net`/`tokio::net`-outside-allow-list
  Semgrep rule was listed in P0.4, parked in §8, AND missing from the G29 rule list. Promoted to G29
  project-local rule **(g)** (initially-empty `net-allow-list.txt`, planted-positive), removed from §8
  forward-ideas, cited in the §5 T9a row. The behavioural `cargo-acl`/cackle upgrade is now an
  owner-decidable P0.4 contract.
- **P0.7 G42/G42b plane-annotation reconciled.** The three-leg phasing is named explicitly: the
  ENFORCEMENT SUBSTRATE activates with the first engine spawn (P4); the per-push PULL-FORWARD leg runs
  from P6/P7; the full per-OS egress-DENY window + release-confirmation G42/G42b are BUILT in P9.
- **New gates/ids this round:** **G56b** (release-tag trust), **G59** (attest-build-provenance,
  promoted from `[DEFER]`), **G60** (reserved — third-party-reproducibility delta of the self-compiled
  layer), **G66** (reserved — scheduled engine-version-currency poller), **G67** (reserved — OSSF
  Scorecard informational corroboration); G29 rule **(g)** (`std::net` ban); G9 invariants **(e)**
  (no `cargo vet update/sync/import` in CI/scripts) + **(f)** (no `fc.gen()` outside the shrink
  wrapper); plan-lint **checks 17** (§8↔adopted-gate status agreement) + **18** (named-build-loop-
  procedure presence). G56a gained secret-scanning/push-protection + default-workflow-permission
  sub-checks; G56 gained the `id-token`-scope + `pull_request_target`-safe-handler + harden-runner-
  ENFORCED sub-rules; G64/G65 homed in P0.7 beside the other release ratchets; cargo-careful + Kani
  added as owner-decidable in-core over-assurance contracts.
- **Gate hardening this round.** G35 NOTICE-parity gained the GPL/LGPL corresponding-source-POINTER
  assertion (the §3.6.2 GPL-FFmpeg discharge mechanism) + the from-source pinned-source/toolchain
  reference; G18a added `imports.lock` to the lockfile diff (+ G18b decided ≥2 import sources required);
  G31 gained the non-empty/output≠input/size-plausibility + the T8 integration sub-tests; G32 gained
  machine-enumerated lossless pairs + the lossy-disclosure property test + the conversion-output
  determinism sub-assertion; G48 clarified the ASAN-coverage honesty + pinned libFuzzer resource
  bounds (`-rss_limit_mb`/`-max_len`/`-timeout`/`-max_total_time`); G38 gained the FFmpeg
  enabled-decoder allow-list; G12 gained the findings-block-presence sub-check; G29 gained per-rule
  planted-positives; check 10 promoted to regenerate-and-diff for the security manifests; plan-lint
  gained the golden-fixture-must-exit-1 base-case meta-check; the gate-quarantine procedure gained the
  self-referential L1/plan-lint bootstrap exception; the build-loop push-exit-code capture adopted the
  RMA marker-file pattern (Quirk #22); G47 gained `bundle.createUpdaterArtifacts`-absent + the
  DNS-prefetch meta-tag; `ANTHROPIC_API_KEY` named as the third real secret; the gitleaks-action
  org-license footgun, the `cargo-cyclonedx` crate-vs-subcommand naming, the `scripts/glibc-floor.toml`
  + the LibreOffice carve-out globs named; the L1-budget-soft + the two-enforcement-planes-honest
  framing corrected; the security-critical-file change-control named as an owner-decidable L(-1).
- **Living-doc/spec-sync (r5).** Where r5 gates reference spec material owned by the spec — §6.7.2/§6.7.3
  (the G56b tag-trust enforcement on the release trigger), §3.6.2 (the GPL FFmpeg corresponding-source
  discharge mechanism G35 asserts), §0.10 (the `bundle.createUpdaterArtifacts` + DNS-prefetch
  side-channel G47 asserts), §6.4.1 (the detect-KAT G15 reads) — the spec is edited in the same change
  per the SSOT > spec > docs conflict order; those spec edits are noted here so the fill pass syncs them.

## 12. Reconciled during P0 review r6

- **ImageMagick hardening — the last untrusted-input decoder with no per-engine control (closed).**
  ImageMagick (a REQUIRED bundled BMP load+save delegate, spec §3.5.5, statically linked inside
  `convertia-imgworker`) — the historically most CVE-dense decoder family (ImageTragick + the
  URL/MSL/MVG coder class) — appeared NOWHERE in the security docs while every other untrusted-input
  engine had a load-bearing argv/build control + a G38 build assertion + a G31 sentinel. Added: the §5
  T9b/T1 ImageMagick row content (hardened `policy.xml` denying the dangerous coders via
  `MAGICK_CONFIGURE_PATH`, OR a coder-/delegate-excluded build), the **G38** policy/coder build
  assertion, the **G29** imgworker `MAGICK_CONFIGURE_PATH` env rule, the **G31** crafted-BMP / SVG-via-
  MSL/URL-coder sentinel, the **G26** `engines.lock`↔adversarial-fixture bijection guard, and the P0.5/P0.7
  homes. Spec §3.5.5 synced FIRST (the ImageMagick coder/delegate hardening `[DECIDED]` block).
- **pandoc `--sandbox` version floor corrected to ≥ 2.15 (a wrong fact removed from a security gate).**
  The docs asserted `pandoc --version ≥ 2.17` with the false rationale "older silently ignore `--sandbox`";
  `--sandbox` shipped in 2.15 and is honoured from 2.15 (enforced by pandoc's type system; verified against
  the 2.15 release notes), and spec §3.5.4 correctly says ≥ 2.15. Corrected in §5 T9b, G38, G29, and the
  P0.7 box (SSOT > spec > docs — the docs were the ones in error).
- **`core.hooksPath` named as a forbidden local-plane bypass (closed).** The entire L1–L3 plane is
  git-hook-based; `git -c core.hooksPath=<elsewhere>` silently disables ALL local hooks WITHOUT
  `--no-verify` — an un-named bypass. Added to the forbidden-bypass list (§3 + build-gates §0 + the L1/L2/L3
  "Bypassable?" column), machine-checked by P0.6 step-0 (`git config --get core.hooksPath` unset/lefthook)
  and by **G54** (which now resolves the EFFECTIVE hooks dir via `git rev-parse --git-path hooks`, not a
  hardcoded `.git/hooks/`).
- **G53 homed in P0.3 (plan-lint check 16 made satisfiable for it).** The core-crate forbidden-dependency
  gate (a fail-closed §5 T6 control) had a catalogue row + §5 citation but no P0 cluster home; added to the
  P0.3 `deny.toml` box with its `cargo-deny [bans]` workspace-member rule + a G24 negative self-test fixture.
- **G56b leg 3 made real (reconciled with spec §6.7.1).** The "verify-tag against the committed signer key"
  was unsatisfiable (no key was ever provisioned) and disagreed with itself ("(recommended)" vs "fail-closed
  always"); now the loop signs its own `v*` tags and a committed SSH allowed-signers file backs `git
  verify-tag` — distinct from the spec §6.7.1 requested-not-required DCO COMMIT sign-off (unchanged).
- **G42b read-half "can never silently no-enforce" scoped to the LINUX leg (honest residual stated).**
  The armed fail-closed `ptrace`/Landlock substrate is Linux-only; macOS/Windows rest on the degradable
  §2.12.3 Seatbelt/AppContainer tier + the G31 sentinel oracle, with an optional armed planted-positive
  (`lsof +D` / ETW `Microsoft-Windows-Kernel-File`) or an owner-accepted residual — stated in §5 T9b + G42b.
- **Correlated-reviewer-family residual forced to a recorded decision (§4).** Explicitly ACCEPTED for v1
  with a concrete spot-audit cadence (every phase boundary + a random ≥1-in-10-box sample); the flip to a
  different model family stays open. plan-lint check 20 asserts the decision is present in `build-loop.md`.
- **Tool corrections (verified against reality).** JS-graph audit `pnpm audit` → **`osv-scanner` over
  `pnpm-lock.yaml`** (`pnpm audit` is online-only AND broken on the pinned pnpm 10.x — registry retired the
  legacy endpoint, HTTP 410; G17); Windows PE mitigation census `checksec`/`dumpbin` → **`winchecksec`**
  (checksec has no native PE support; G38); G43 macOS/Windows live monitors are SIP-blocked (`fs_usage`/
  DTrace) / GUI-first (Procmon) → the CLI-automatable snapshot-diff is the load-bearing leg; G59 offline
  attestation needs **`--custom-trusted-root trusted_root.jsonl`** (a bare `--bundle/--repo` is not
  air-gapped); the gate-tool checksum VALUE is now corroborated against the tool's own signature / ≥2 origins
  (the verify-mechanism self-test alone proved nothing about authenticity); the macOS universal sidecars get
  a **`lipo -info` both-slices** assertion (Tauri does not itself `lipo`); the pandoc `--resource-path`
  VALUE must resolve to scratch (presence-only passed `/`); the LibreOffice `-env:UserInstallation`
  disposable-profile is now argv-asserted.
- **Suppression-ratchet discipline applied uniformly.** The `.gitleaks.toml` allowlist/baseline + the
  cargo-vet exemption set now carry the same box-id + Dual-Review + rotating-fingerprint growth-guard the
  G29 Semgrep ledger has, and both join the L(-1) security-critical-file set; a G56 secret-in-Actions-log
  scan + a GitHub-Environment-with-required-reviewers idea close the secret-leak axis further.
- **New plan-lint checks 19 (reviewer-rubric committed/sourced/canonical-phrase) + 20 (recorded
  reviewer-family decision)**; new forward ideas (CodeQL/Semgrep-taint for the T2 surface, JS name-trust
  via Socket/allowlist, the `build.rs` manifest binding, a DNS-observation G42 leg, gate-infra ReDoS bounds,
  TruffleHog verified-secret, the GitHub Environment release gate, the `docs/reproduce.md` third-party
  rebuild recipe); G65 pre-committed to a REQUIRED SCHEDULED issue-opener (`zzuf` named for LibreOffice).
- **Living-doc/spec-sync (r6).** The ImageMagick coder/delegate hardening is owned by the spec
  (§3.5.5, edited FIRST this change); the pandoc ≥ 2.15 floor already matched spec §3.5.4 (the docs were
  corrected to the spec, no spec edit needed); per the SSOT > spec > docs conflict order.

## 13. Reconciled during P0 review r7

- **Three blockers fixed (the build-gates.md catalogue owns the detail; here the §5/§3/principle homes are
  synced).** G17 `cargo audit --locked` → plain `cargo audit` (no such flag — a tool-fact footgun). G56a
  sub-check (f) made an **OR-gate** (CodeQL configured OR the G29 rule-(i) Semgrep taint ruleset, machine-enforced
  XOR by plan-lint check 21) — the §5 T2 verifying-gate cell reworded so the sanctioned Semgrep path can never
  leave a permanently-red required check. The invisible gate-ID gaps (G40 deleted, G61–G63 reserved) are closed
  by a vacated/reserved blockquote + plan-lint check 22 in the catalogue.
- **§3 gate-range labels corrected.** "the cross-platform/corpus/SAST heavies G26–G34, the whole release-tier L5
  set G35–G67" was factually wrong (G34 vacated; L5 starts at G33b not G35; the closed G35–G67 range swept in the
  non-existent G40/G61–G63 + the prose-only G65/G66/G67) → accurate prose enumerating the CI-only and release-tier
  gates.
- **§5 hardening.** T9a now cites **rules (g)+(j)** (the raw-socket FFI net-ban `libc::socket`/`nix::sys::socket`
  promoted from "optionally" to REQUIRED). T10 gains the **output/scratch BYTE budget** (the disk-exhaustion
  sibling). T9b control + verifying-gates cells gain **poppler** (`pdftotext`-without-network + the remote-URI-PDF
  G31 sentinel, previously omitted from the §5 enumeration). The cross-cutting hardened-CI row + principle 10 gain
  the **pip** dependabot ecosystem (the gate-toolchain `requirements-ci.txt` is in the CI trust boundary).
- **Principle 11 made symmetric.** The `ANTHROPIC_API_KEY` host-isolation residual is now STATED: it is
  build-session-local and present in NO CI job, so it needs no CI host-isolation, but IF a future workflow reads
  it, it inherits the same self-hosted-label ban + host-isolation as the minisign pair (the G56 lint keys on all
  THREE named secrets, not only the minisign pair). harden-runner's missing enforcement-status output is handled by
  an ACTIVE PROBE, not a fragile log-string parse.
- **Living-doc/spec-sync (r7).** §3.6.1 (FFmpeg configure-flag / `--enable-nonfree` whole-binary-license, G38),
  §7.8.2 (deep-link/URL-scheme absence + local-`url`, G47), §1.10 (output/scratch byte budget, T10), §7.2.3
  (bundled-config startup hashing, G46), §6.4.2 (CI-host resource bound + DNS-egress leg, G26/G42) are owned by the
  spec and edited in the same change per SSOT > spec > docs; noted here so the fill pass syncs them.
