# ConvertIA — Security Concept (living)

> **The build-time security & guardrail concept** for ConvertIA. Defines *what we
> protect against* and *which guardrail proves it*. The companion
> [build-gates.md](build-gates.md) is the operational gate catalogue (the *how*).
>
> **Status: living.** It maps threats to controls and gates and changes only on the
> triggers in §6. It does **not** override the product truth:
> **conflict order = [SSOT](../SINGLE-SOURCE-OF-TRUTH.md) > [spec](../spec/README.md) > this document**
> (the full order is SSOT > spec > security/process docs > plan > code > conversation;
> this document sits in the security/process-docs layer).
> Where this doc and the spec describe the same control, the spec's `§` is the
> source of the technical detail; this doc is the consolidated security view + the
> mapping to enforcement.
>
> The long-form text before this index (mechanics now owned by the spec sections and the
> build-gates rows): `git show 608a771d07a9:docs/security/security-concept.md`.

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

The session model — one branch, `main`, with no merge step and no auto-merge; two sessions,
each committing from its own clone; enforcement by the deterministic gates plus the required
status checks on every push — is defined in [CLAUDE.md](../../CLAUDE.md) §2,
[build-loop.md](../process/build-loop.md) §0,
[roles-and-escalation.md](../process/roles-and-escalation.md) and spec §6.7; this section
keeps the two security rules.

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
  L(-1) file needs no trailer) and the cage's own liveness (a glob matching no tracked path,
  a stale/orphan targetless declaration, a refused, dead or orphan Loop-tool escape, or a
  refused or dead monotone entry fails the gate — the build-gates G71 row carries the
  mechanics). G71 fails closed at pre-push and on every push in CI, and its posture-flag row
  in `scripts/gate-planes.toml` keeps it so (plan-lint check 27). This is the one
  **ownership control above the deterministic gates** — the
  trailer records an owner decision; G71 checks the evidence, not the intent (so a leaked
  key or a unilateral cage edit cannot pass unseen). It is independent of the G1
  `Dual-Review:` trailer; both may co-occur on one commit.
- **Ratchets.** A ratchet the Loop moves as ordinary box work lives in uncaged data with a
  `[[monotone]]` entry in `scripts/l-neg1-files.toml`: the strengthening direction is free,
  the weakening direction needs the owner ack (G71); the relied-upon dependency floors
  (spec §0.8) are such ratchets. A ratchet moved only at owner or sweep acts
  (`coverage-floors.toml`, `max_survived_mutants.toml`) stays caged whole.

**The dual review is a quality amplifier, not a security control.** The Opus+Sonnet
review (G1) is self-attested via an unverifiable commit trailer (G12 checks its presence
and a review marker in the body, a heuristic that cannot prove two live models ran); a
gamed `GO/GO` trailer cannot, by itself, ship insecure code, because the **only security controls
are the deterministic gates (every `Gnn` except G1, the dual review)** — a gate
either passes on a clean checkout or it does not. The dual review raises quality and
catches design defects the gates can't encode; its review record in the commit body
(build-loop.md §3 Step 6) makes a "both GO, 0 findings" on a non-trivial diff an
**auditable smell** for the phase-end Co-Pilot spot-audit. The review protocol (inputs,
severity bar, round cap, post-GO rule, and two live reviews or no `GO`) is canonical in
build-loop.md §3 Step 5 and is not restated here. The correlated-reviewer residual (Opus
and Sonnet share lineage) is accepted for v1; the decision, its spot-audit cadence and the
open reviewer-family flip live in [build-loop.md](../process/build-loop.md) §3 Step 5
(plan-lint check 20).

## 3. Defense in depth — the enforcement planes

A change passes staged, independent defensive planes on its way from idea to a
published release. No plane trusts an earlier one to have caught everything.

| Plane | When | Mechanism | Blocks | Bypassable? |
|---|---|---|---|---|
| **L(-1) — change control** | a commit touching an L(-1) file | the owner-ack trailer, audited by G71 (§2) | the push | none |
| **L0 — Build-Loop per box** | while building, before each commit | the build-loop discipline and the G1 dual review of the staged diff; P0/P1 findings are fixed in the working tree and re-reviewed before the commit, never in a fix-push cycle | the commit (self-gate) | only by a rule violation (no technical bypass) |
| **L1 — pre-commit** | `git commit` | lefthook, parallel and diff-scoped, under a soft ~10 s budget ([build-gates §0](build-gates.md#0-policy)) | the commit | only by a forbidden bypass (below) |
| **L2 — pre-push** | `git push`; on the landing path (build-loop Step 6), the forced pre-push in a throwaway container on exactly the signed commits, before the signer repo pushes them | lefthook range and full-tree gates under ~3 min; the docs-only fastpath skips six heavy toolchain legs (build-gates §4) | the push | a forbidden bypass; the fastpath skip of those six legs on a documentation-only push range |
| **L3 — commit-msg** | `git commit`, before L2 (the plane numbers are assigned, not chronological) | the conventional-commit subject and the subject (≤ 100 characters) and body (≤ 120 lines) caps (G11) | the commit | a forbidden bypass; git's merge, revert and fixup auto-subjects are exempt |
| **L4 — CI (GitHub Actions)** | after push | the same gates re-run on a clean checkout (G25) plus the heavy gates: cross-platform build, corpus, coverage, SAST, SBOM, repo-config introspection | a red `main`, fixed immediately | none for the required checks, whose repo configuration G56a asserts |
| **L5 — Release** | a `v*` tag | the release gates: SBOM and completeness, the licence hard-fail, the copyleft source bundle, checksums and minisign over `SHA256SUMS` (the only signing in scope — never binary code-signing or notarization, SSOT *Out of Scope*), the build-provenance attestation, the size budget, the egress and no-pollution gates; the G56b tag-trust check runs before any secret is read | the release | none (release-blocking) |

**Two planes.** The two-planes principle: a gate that can run locally runs at L1–L3 and
again on a clean checkout at L4 (G25), so a skipped or missing local hook is still caught;
repo-config introspection, the heavy build, corpus, coverage and SAST jobs and the release
tier run in CI only. The principle steers the wiring; where each gate actually runs is its
build-gates row (the *Enforced by* cell), and the plane structure and the fail-open registry
are [`scripts/gate-planes.toml`](../../scripts/gate-planes.toml) (G54b). No check asserts
that every local gate command has its L4 step — the G25 residual. A red CI run is fixed
immediately: never re-run hoping it passes, never `--no-verify`.

**Forbidden local-plane bypasses (the complete named set).** The local L1–L3 plane is
entirely git-hook-based, so the no-bypass policy names every way to make a hook not fire or
not run its commands: **(a)** `--no-verify`/`-n` on commit or push; **(b)** force-push;
**(c)** disabling a required CI check; **(d)** `core.hooksPath` redirection —
`git -c core.hooksPath=<elsewhere>` (or a persisted `git config core.hooksPath`) silently
disables every local hook without `--no-verify`; **(e)** a push from a repository without the
installed hooks (a fresh clone, a bare repo, a hook removed or edited), or a change made to
`main` on GitHub itself (the web editor, the API); **(f)** a pushed commit that no hooked
`git commit` made — a plumbing commit (`git commit-tree`, or `git hash-object -w` with
`git mktree` and `git update-ref`), one made in a repository without the installed hooks, or one
that `git cherry-pick`, `git rebase`, `git am` or `git merge` wrote (none of them runs the
pre-commit hook) — save a rewrite of a commit L1 and L3 passed with a forced L1 run
(`lefthook run pre-commit --force`) on it before the push, as Step 4a (1) runs after the
build-loop Step 6 rebase; **(g)** a lefthook switch that skips or swaps what a hook runs — the
`LEFTHOOK=0`, `LEFTHOOK_EXCLUDE`, `LEFTHOOK_CONFIG` and `LEFTHOOK_BIN` environment variables and
a per-clone local config - `lefthook-local.<ext>` or `.lefthook-local.<ext>` at the repository
root or `.config/lefthook-local.<ext>`, `<ext>` one of yml, yaml, toml, json and jsonc (measured
with the pinned lefthook 2.1.9) - which lefthook merges over `lefthook.yml`. The landing path
([build-loop.md](../process/build-loop.md) Step 6) holds the one sanctioned hook-free commit and
the one sanctioned hook-free push, both in the signer repo of the signing hop, which a session
runs on the owner's PC only after the owner's own Freigabe message
([release-pipeline-trust.md](../process/release-pipeline-trust.md) §3): its `git commit-tree -S`
re-creates each commit that L1 and L3 passed in a throwaway container, on that commit's own tree
and message, and changes only the signature and the parent line; on this path L2 is the forced
pre-push (`lefthook run pre-push --force`) in a throwaway container on exactly the signed
commits, and the signer repo pushes only after that run is green and only those commits. Any
other such push is bypass (e), and any other such commit is bypass (f). (d) is machine-checked:
the build-loop Step 0 start sanity asserts `core.hooksPath` is unset or the lefthook-managed
path, and G54 resolves the effective hooks directory instead of assuming `.git/hooks/`, so a
redirect cannot leave it inspecting an inert directory. No gate sees which repository a push
comes from, how a commit was made or which variables a hook ran under, so (e)–(g) are not. L4
(G25) remains the net regardless.

**Gate isolation from the tree it checks.** No in-gate check can catch a file placed beside a
gate that shadows a module the gate imports (`scripts/re.py` shadows `re` for a plain
`python3 scripts/<gate>` run), so every plane runs its Python gates as `python3 -P`, which G54b
leg (5) enforces, and the canary runner sets `PYTHONSAFEPATH=1`. A hook in a linked worktree
exports an absolute `GIT_DIR`, under which a self-test's own `git config core.hooksPath <tmp>`
would persist bypass (d) in that repository, so the runner and every self-test that starts git
drop the inherited `GIT_*` variables except `GIT_EXEC_PATH` (G24).

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
9. **In-core untrusted bytes are still adversarially tested.** The in-core
   untrusted-byte paths — the §1.2 detection layer (the bounded gzip/svgz inflate, the
   Rust ZIP central-directory peek, the OLE2/CFB directory read, the bounded XML
   structural peeks) and the native CSV/TSV engine — run in the trust kernel *outside*
   the §2.12 isolation boundary, where a panic/OOM/UB lands in the core. They carry
   their own adversarial fuzz gate (G48), not only the engine-side corpus. (spec §1.2,
   §2.12.4)
10. **Hermetic, hardened CI supply chain — pin and verify.** The CI plane is itself an
    asset — it holds the signing key `MINISIGN_SECRET_KEY` and its passphrase
    `MINISIGN_PASSWORD` (spec §6.2.3) — so every workflow declares least-privilege
    `permissions`, every third-party action is pinned by full commit SHA, the build
    resolves only the committed lockfiles (`--locked` / `--frozen-lockfile`), the workflow
    lints run on every push, and the secret-bearing release job never runs on a fork pull
    request (spec §6.3.4). `dependabot.yml` watches all four graphs — github-actions,
    cargo, npm and pip (the gate-tool `requirements-ci.txt`, which reads CI secrets and
    produces findings) — and every bump is reviewed and re-landed, never auto-merged. Every
    gate, SAST and SBOM tool is pinned by exact version and verified by checksum or image
    digest at install, through a verify step that is itself self-tested (a wrong checksum
    must fail the install; pip tools install `--require-hashes`), because a poisoned gate
    tool both misses a real finding and reads the CI secrets. The build toolchain that
    produces the shipped artifact is in the same trust boundary (everything that touches the
    minisigned bytes) and carries its own pins: the CI base image and build container are
    digest-pinned, the C/C++ toolchain and the Tauri CLI version- or digest-pinned, and the
    Rust toolchain pinned to an exact stable version and the `cargo-fuzz` nightly to a date
    (`nightly-YYYY-MM-DD`), both asserted not floating
    ([build-gates §0](build-gates.md#0-policy)). The repo config that makes a red run block
    is asserted, not assumed: G56a reads the `main` rulesets — the required checks,
    bypassable by the repo-admin role only (the trusted direct pusher, while a fork pull
    request stays gated; classic branch protection would block every direct push), and
    history protection with no bypass actor — plus secret scanning with push protection and
    read-only default workflow permissions; the secret-bearing `v*` tag trigger, which those
    rulesets do not cover, has G56b instead: the release aborts before any secret is read
    unless the tagged commit is an ancestor of `main` with its checks green (its tag-ruleset
    read is unbuilt, §5).
11. **CI runner-host integrity — the secret never shares a host with untrusted
    input.** The `MINISIGN_SECRET_KEY`/`MINISIGN_PASSWORD` pair is the single most
    damaging secret (the user-facing trust substitute, principle 7, collapses to it), and
    a runner that handles untrusted input and outlives its job is the textbook
    host-compromise vector: once poisoned, every later release could be silently
    re-signed with the real key. So every lane runs on GitHub-hosted runners, a fresh VM
    per job (spec §6.1.4), and signing is its own ephemeral job with no corpus, fuzz or
    engine step, under `step-security/harden-runner` in block mode whose enforcement an
    active probe proves rather than trusts (spec §6.7.2); on the Linux corpus and fuzz
    legs the egress monitor and the read-half fs-audit bound what an untrusted input can
    reach (G42, G42b). G56 fails a job that reads any of the three secret names on a
    self-hosted label or without harden-runner block mode. `ANTHROPIC_API_KEY` lives only
    in the build session, where G1 runs, and is present in no CI job — a leaked reviewer key
    can at most forge a review trace, never ship code or mint a signed artifact — and a
    workflow that ever reads it falls under the same rule.

## 5. Threat model → control → gate

The spec §0.11 table is the class list; this table has one row per class, and plan-lint
check 8 holds it to the canonical §0.11 class set in both directions and requires a
catalogued gate in every row. A class with a control but no verifying gate is a gap. A row
names the owning spec sections and the control in one sentence (the mechanics live in the
spec), the verifying gates (their mechanics and their Status live in the
[build-gates.md](build-gates.md) rows; check 16 requires a self-test for every live,
fail-closed, script-driven gate a threat row cites) and the residual the class accepts; a
residual accepted by declining a closure tool is that tool's `decided` row in
[gate-status.md](../process/gate-status.md).

| Threat (spec §0.11) | Owning § and control | Verifying gates ([build-gates.md](build-gates.md)) | Accepted residual |
|---|---|---|---|
| **T1** untrusted decoder input → crash, hang or exploit | §2.12, §1.7, §0.9, §1.2 — every third-party C/C++ decoder runs in an isolated subprocess under the invocation timeout/kill and the pool bounds; untrusted bytes reach the core only through memory-safe Rust (the detection sniffs and the native CSV/TSV engine, §2.12.4) | G48 in-core detector fuzz · G31 the corpus through the §2.12 boundary (the crafted-BMP ImageMagick sentinel included), with the privilege-drop-tier-applied, memory-cap-kill and Job-Object-reap assertions · G26 the engine-side adversarial corpus, at least one fault-injection fixture per staged engine · G64 the privilege-drop-tier ratchet | the §2.12.3 privilege drop (Linux Landlock + netns + seccomp; the macOS cheap tier; Windows intermediate IL + Job Object) is best-effort and degrades silently, so a decoder exploit has no load-bearing runtime containment: isolation bounds it to one sidecar and its scratch, and the tier is tracked, not guaranteed |
| **T2** malicious / compromised WebView content | §0.10 — the capability allow-list gives the WebView no `fs` and no network, the CSP admits no remote origin (`object-src 'none'`), and the §0.10 by-construction hardening keys (`withGlobalTauri`, `dangerousRemoteDomainIpcAccess`, release `devtools`, the asset protocol, the updater artifacts and the rest of that list) stay absent or false; an eslint ban by name on the DOM and eval sinks — the APIs that parse a string as unsanitized markup or as code — in the listed code shapes keeps a WebView-controlled value from reaching one through those shapes, whatever the flow depth | G47 the CSP, capability and hardening-key lint · G5 the sink ban, the primary sink control: no inline directive lifts it, each sink rule is frozen as declared once at error and proven to fire on a planted fixture before every run, and every `src/` file is linted under the sink rules, language options and settings the fixture proves · G18 no updater or HTTP-client crate · G18c the npm registry pin · G18b cargo-vet, the crate-side name-trust check the npm side lacks · G42 the offline-egress monitor (the macOS WebView leg rests on G47) · G29 rule (i), defense in depth: the WebView taint rule into a sink the ban forbids, within one function (plan-lint check 21 asserts its file) · G37c the glibc floor, the OS-library analogue, with no WebView leg | the OS WebView (WebView2, WebKitGTK, WKWebView) is un-pinned, un-versioned and absent from the SBOM: its CVE currency is the OS update channel's, and the CSP lock is ConvertIA's only side-bound; a typosquatted npm package hosted on the pinned registry passes the registry pin (its closure is declined, gate-status.md); the sink ban reads each listed sink in its listed forms, and an attribute- or element-method call or a global timer at its callee, directly or through exactly one wrapper; every other indirection lints clean — one measured class, such as an alias or a destructured or imported binding, a computed or concatenated name, `call`/`apply`/`bind`/`Reflect`, a callee wrapped more than once or an API outside the list, which the CodeQL decline lists in full (gate-status.md) — with the runtime CSP's `script-src 'self'` (no `unsafe-eval` or `unsafe-inline` source) and React's escaping as the compensating controls |
| **T2a** WebView steers a write to an attacker-chosen path | §0.4.4, §2.1, §2.3.3, §2.7 — destination roots are core-picked: C2b runs the native picker Rust-side and `ChosenRoot(DestinationId)` resolves against the core-side picked-roots registry, so the WebView never names a path; the no-harm machinery (non-destructive create, write-target link safety, divert) stays the backstop on the resolved root | G15 / G16 the fs-safety unit and property tests (no-clobber and link-safe on a core-resolved root) · G31 the hosted fs-safety assertions and the adversarial-path corpus shared with T7 | — |
| **T2b** WebView re-submits an attacker-chosen SOURCE path | §7.8.1, §1.1, §2.4 — eliminated by construction: no path crosses the IPC wire in either direction (intake paths stay in the core-side `PendingIntake` buffer that C1 `drain_intake` drains by `collectingId`), and the freeze-time §1.1 re-validation stays as defence in depth over every core-side source | G31 the freeze re-validation property test (every intake path re-validated, whatever its provenance) | — |
| **T2c** WebView plugin-write surface (`log:default`) | §7.5.1, §7.4.2, §0.10 — the log bridge appends WebView-supplied diagnostic text to ConvertIA's own log file and names no path, file or target; the WebView holds no `store:` permission, because prefs are core-owned (§7.4.2), so no `plugin:store` command is invocable | G47 every capability permission is one of `{core:default, log:default}`, entry by entry (a scoped entry fails) | noisy local log lines (rotation bounds the size); never a read, an exfiltration or an overwrite of user data |
| **T3** bundled-binary supply chain | §3.8, §6.1.3, §6.2, §7.2.3 — every engine is pinned and checksum-verified before staging under the per-engine acquisition policy (from source: an anchor independent of the download host, with the generated build files regenerated locally; prebuilt: corroborated by ≥ 2 independent mirrors or a distro's signed package; FFmpeg from source only, from its GPG-signed release), from allow-listed origins, enumerated in the SBOM and integrity-checked at startup | G37 engine checksum, pin-establishment provenance and engine-source allow-list (any `engines.lock` SHA edit is a hard Co-Pilot escalation) · G35 SBOM completeness · G46 startup integrity · G17b the engine CVE scan over `engines.lock` (informational per push; the FFmpeg CPE mandatory) | startup integrity detects damage, not tampering: a whole-bundle swap replaces the in-bundle manifest too, so the authenticity floor is the minisign-signed `SHA256SUMS` the user verifies before first run (§6.2); binding the manifest digest into the binary and a startup read of the signed sums are declined (gate-status.md) |
| **T3a** DLL/dylib/`.so` side-loading of a bundled shared object inside an engine resource tree | §3.7.2, §3.5, §6.1.3 — every staged shared object is pinned (its own `engines.lock` row, or its tree engine's per-file digest in the §7.2.3 manifest) and verified before staging; on Windows, engines spawn with a minimal `PATH` (their own bundle directory), the loader-injection variables (`LD_PRELOAD`, `LD_LIBRARY_PATH`, `DYLD_*`) are stripped, and a staging-time closure check resolves every non-system dependency inside the bundle | G37 the per-shared-object SHA-256 before staging · G35 the stage-tree manifest diff, an attribution check (never a post-staging byte re-verify: staging re-emits bytes) · G37b the dynamic-dependency closure (`ldd`/`readelf -d`, `otool -L` and `otool -l`, `dumpbin /dependents`) | — |
| **T4** open-file launch of a fresh artifact | §7.7, §7.7.3 — reveal-in-folder and no auto-open; C9 takes an `OpenTarget` resolved Rust-side against the current run's `RunResultStore`, so the WebView can select only a recorded result, never name a path | G15 / G31 the membership tests (only a current-run result opens, and only by id) | — |
| **T5** core panic / app fault | §2.13, §7.2, §0.3.1 — the app-level fault model (`catch_unwind` worker boundary), the startup faults and the WebView-absent handling | G15 the panic-boundary unit test (a panic becomes an app fault, never a crash) · G46 a missing or corrupt engine is an app fault | — |
| **T6** copyleft aggregation boundary | §3.6, §0.3, §0.7 — copyleft engines ship as separately invoked binaries (aggregation, not linking) | G18 the `cargo-deny` licence allow-list over the Rust graph · G36 the SBOM forbidden-family hard-fail · G38b the LGPL relink and GPL corresponding-source bundle present (§6.1.3) · G53 the core-crate forbidden-dependency walk (no image-worker C library in the core closure) | — |
| **T7** path / link redirection (symlink/junction/TOCTOU) | §2.3, §2.1 — resolved-identity and link safety, and the exclusive create-new-or-fail publish evaluated on the resolved real file | G15 / G16 the atomic-publish and fs-safety unit and property tests · G31 the hosted fs-safety assertions and the adversarial-path corpus | — |
| **T7a** Windows reserved-name / trailing-dot-space output aliasing | §2.2.4 — on Windows the pre-publish seam rejects every ConvertIA-constructed component whose right-trimmed first dot-segment is a reserved DOS device name or that ends in a dot or space, as a clear per-item failure naming the token (never alias, sanitise or truncate); the divert path re-checks identically | G15 the guard's unit tests (the reserved-segment and trailing-dot/space rejects on Windows, the Unix pass-through, the divert re-check) · G31 the per-pair fs-safety leg, once the §2.1.1 write sequence runs per pair | — |
| **T8** self-feeding / batch expansion | §2.4, §7.1 — the source set is frozen at drop and each run owns its instance identity, so fresh outputs and a second instance's files never join a run | G15 the frozen-set and per-run-ownership unit tests · G31 the integration leg: outputs written into the dropped folder mid-run stay out of the run, and a second instance's drop never grows the frozen set (§2.4.2, §2.4.3) | — |
| **T9a** ConvertIA's own code exfiltrates user files | §7.6, §0.10 — ConvertIA's own code opens no socket: no HTTP or updater plugin, no remote `connect-src`, no phone-home | G29 rules (g) and (j), the per-push net ban over the Rust import path and the raw-socket FFI path, which also catches a renamed or transitive crate a name-based ban misses · G47 the CSP and capability lint · G18 the HTTP-client deny-list · G42 the egress monitor, the release-tier proof | — |
| **T9b** bundled engine reaches out / reads out-of-input on hostile input (incl. the LibreOffice macro and `WEBSERVICE()` vectors) | §3.5.1, §3.5.2, §3.5.4, §3.5.5, §6.1.3 — load-bearing per-engine argv and build controls, not the degradable OS sandbox: FFmpeg `-protocol_whitelist file,pipe` and curated demuxers (no `concat`, never `-safe 0`); pandoc `--sandbox` (≥ 2.15); a disposable per-job LibreOffice profile (never the user's own) with macros and link updates disabled and no `WEBSERVICE()` or external-data refresh on load; librsvg loaded with no base URL; poppler `pdftotext` built without network support; the ImageMagick coder allow-list compiled into the image worker — each half (zero outbound packets; no out-of-input file read) with its own armed substrate, and the LibreOffice profile and the worker's compiled lockdown hashed at startup, so a damaged one faults instead of disarming | G38 the per-engine build assertions (protocols and demuxers, the no-base-URL librsvg, pandoc ≥ 2.15, poppler without network, the parsed LibreOffice profile keys, the ImageMagick policy read-back) · G29 (b), the per-spawn environment rule and the engine argv contract (Semgrep absence rules, the pinned `argv_contract` presence tests) · G31 the corpus sentinels on the per-push leg (macro-bearing office files, a `WEBSERVICE()` sheet, a crafted BMP or an MSL/URL-coder SVG, a remote-URI PDF) · G42 the egress half · G42b the read half (`ptrace`/Landlock, fail-closed on Linux) · G37 / G35 the LibreOffice profile's `engines.lock` row and its SBOM count | on macOS and Windows the read half rests on the corpus sentinels and the argv and build controls: the realized tiers there confine no reads (owner-accepted, option (b)) |
| **T10** resource exhaustion / DoS-by-input | §1.10, §0.9 — the resource pre-flight and budgets, including the per-item output and scratch byte budget that kills a slowly exploding decode to `Failed(TooBig)`, the pool and handle bounds and the to-GIF guardrail | G16 / G31 the adversarial resource-budget corpus and property tests — oversized SVG renders, over-duration to-GIF, over-cardinality batches, decompression bombs (svgz, ZIP-in-OPC, nested flate) and an output/scratch byte-budget case — each failing clearly while the batch continues | — |
| **T11** macOS engine-as-first-TCC-accessor (silent-deny) | §3.5.0, §7.2.6 — the Rust core, holding the TCC grant from the §1.1 freeze, copies a protected source into a per-job scratch path before spawning, so an engine is never the first process to touch Desktop, Documents, Downloads or removable media | G31 the macOS sub-test (the core PID is the first accessor; the engine gets the scratch path) · G29 rule (d), an armed door guard with no in-scope spawn by design, plus the `t11_seam_pin` source pin on the seam's `stage_for_tcc` call | — |
| **T12** unsigned distribution / download-MITM | §6.2 — the build ships unsigned and un-notarized; the trust anchor is the minisign-signed `SHA256SUMS` plus an out-of-band public-key fingerprint (the in-repo key alone is trust-on-first-use), verified by the user before first run | G39 checksums, minisign and the executed verify recipe · G44 the verify recipe and the out-of-band fingerprint match | no runtime tamper resistance, and SmartScreen/Gatekeeper friction on an unsigned build (§6.2.4), named in SSOT *Out of Scope* and the release notes |
| **T13** cross-user single-instance socket (macOS machine-global) | §7.1.1, §2.4 — an accepted v1 limitation: the plugin's macOS socket is the world-writable `/tmp/{id}_si.sock`; its injection half is bounded by the provenance-independent freeze re-validation that backs T2b (a socket-delivered path enters the core-side intake funnel, never the wire) | G31 the freeze re-validation property test (the injected-path half); the §6.6 macOS walkthrough confirms the normal single-`.app` path | the launch-path leak and the squat DoS: a second logged-in macOS user is outside the offline-converter threat model, the leak is user-visible paths (never contents), and the primary macOS path is the AppleEvent (§7.8), which the socket does not touch; Windows and Linux are per-user by construction |

**Cross-cutting build/release controls** (not §0.11 threat classes, but load-bearing
security guarantees with their own gates):

| Guarantee | Owning § and control | Verifying gates | Accepted residual |
|---|---|---|---|
| credential / secret in repo | §6.2.3 — no secret is committed; the real secrets are three: the CI pair `MINISIGN_SECRET_KEY` and `MINISIGN_PASSWORD` and the build-session reviewer key `ANTHROPIC_API_KEY` (`sk-ant-…`); the committed path allow-list and the capped baseline are the only suppression the repository controls, so in every mode the scan honours no inline `gitleaks:allow` comment, no `.gitleaksignore` and no binary verdict of git's | G2 `gitleaks` over the staged diff, the unpushed range, the working tree and the full history, with custom rules for the minisign key shape, a literal `MINISIGN_PASSWORD` and the Anthropic key, each planted-positive; `run-gitleaks` closes those three in-repo suppression channels and `check-gitleaks-allowlist` freezes the config shape and refuses a tracked `.gitleaksignore` · G24 the self-test pins both reaches of the bundled allow-list, so a gitleaks bump that moves either reds · G56a secret scanning and push protection on, the GitHub-native backstop · G1 the dual review, never a security control: with the owner, the surface for a secret placed on purpose in an allow-listed path | gitleaks' bundled global allow-list, which `useDefault` appends (the pinned 8.30.1 cannot keep the bundled rules without it), skips a path such as an image, a font, a document, a lock file or a vendored tree for every rule, the custom ones included, and drops a bundled rule's finding by its secret (`false` or a stopword in it, `true` at its start, `null` at its end, or a placeholder shape; a custom rule reports a fixed capture, so no value it matches reaches that list); accepted, because the gate guards a careless leak of a text secret, those paths hold generated, binary or vendored artefacts, a bundled rule's random token holds such a word only by chance, and a secret placed there on purpose inside the owner-acked tree is the review's and the owner's surface; `scripts/run-gitleaks` lists the reach |
| authentic, verifiable download | §6.2 — per-file SHA-256, the minisign-signed `SHA256SUMS` and the published recipe `minisign -Vm SHA256SUMS -p docs/minisign.pub` (lowercase `-p`, the key file), plus an out-of-band public-key fingerprint (the in-repo key alone is trust-on-first-use: a tampered clone swaps artifact, sums, signature and key together); key loss and coordinated disclosure follow [vuln-response.md](../process/vuln-response.md) | G39 runs the documented recipe against the produced sums and fails the release on non-zero · G44 the recipe's literal form and the published fingerprint | — |
| §7.5 log never carries file contents / full paths | §7.5 — redaction in the logging layer | G15 / G31 the redaction property test: a secret-looking path stem fed through the logger is absent from the log | — |
| §2.14.1 per-run temp ownership + mode | §2.14.1 — per-run-owned scratch: a `0o700` scratch root and `0o600` `.part` publish temps | G15 / G31 the temp-ownership and mode-bits assertion | — |
| hardened CI supply chain | §6.3.4, principle 10 — least-privilege `permissions`; SHA-pinned actions recorded transitively in the caged inventory; `dependabot.yml` over github-actions, cargo, npm and pip; a `docker://` step image digest-pinned and re-pinned at the phase-end sweep; lockfile-locked builds; no fork-PR release; pinned, digest-verified gate tools; per-workflow concurrency and `timeout-minutes` | G49 `actionlint` · G50 `zizmor` · G18a lockfile integrity · G56 the Dependabot ecosystems, concurrency and timeouts, and the action-pin inventory (check 12) | — |
| CI runner-host integrity (principle 11) | §6.1.4, §6.3.4, §6.7.2 — every lane on GitHub-hosted runners, a fresh VM per job; signing is its own ephemeral hosted job under `step-security/harden-runner` block mode, with no untrusted-input step | G56 a secret-reading job on a self-hosted label, or without harden-runner block mode, fails · G42 / G42b the egress monitor and the read-half fs-audit on the Linux corpus and fuzz legs | — |
| GitHub required-status-checks / repo config (via rulesets) | §6.7.1, principle 10 — with no PR and no second reviewer, repo config is the only thing that turns a red run into a block, and it is invisible to the codebase: two rulesets on `main` — the required checks, bypassable by the repo-admin role only (the trusted direct pusher; classic branch protection blocks every direct push, GH006), and `non_fast_forward` + `deletion` with no bypass actor — plus secret scanning with push protection and read-only default workflow permissions | G56a reads the rulesets and settings through the API on every push and on schedule | — |
| release-tag (`v*`) trust — the secret-bearing trigger G56a does not cover | §6.7.2, [release-pipeline-trust.md](../process/release-pipeline-trust.md) — the signing job fires on a `v*` tag, outside the branch rulesets, so the tag gets the same guard: a tag-protection ruleset, an ancestry-and-green check of the tagged commit before any secret is read, and a signed annotated tag | G56b (1) the `v*` tag ruleset, whose rulesets-API read is unbuilt (the G56b Status cell names its box): until it runs, the owner-provisioned ruleset is the control and no CI leg verifies it; (2) the tagged commit an ancestor of `origin/main` with `main`'s checks green for that exact SHA, before any secret is read; (3) `git verify-tag` against the committed allowed-signers file. Legs (2) and (3) fail closed; (3) fails open only while that file is absent (its `scripts/gate-planes.toml` `[[fail_open]]` row) | — |
| JS/WebView supply-chain symmetry with Rust | §6.3.4 — the `.npmrc` registry pin and resolution guard (dependency confusion), a frontend GPL/AGPL licence deny over the pnpm graph and a frozen `onlyBuiltDependencies` allow-list (no install-lifecycle scripts). The JS execution lockdown is the stronger half: Rust `build.rs` scripts and proc-macros run arbitrary native code in the secret-bearing release job, and cargo-vet is a trust signal, not a sandbox, so that job builds with `CARGO_NET_OFFLINE=true` after `cargo fetch --locked`, under harden-runner block mode | G17 `osv-scanner` offline over `pnpm-lock.yaml` (never the online-only `pnpm audit`) · G18c the registry pin and resolution-URL guard · G36b the frontend GPL/AGPL hard-fail · G18d the lifecycle-script lockdown: the `onlyBuiltDependencies` allow-list frozen, and `enable-pre-post-scripts` and `unsafe-perm` each pinned `false` (pnpm reads an absent key as on) · G18b cargo-vet, a trust signal and no sandbox · G56 check 8, `CARGO_NET_OFFLINE` after the locked fetch | cargo cannot sandbox a build script the way pnpm blocks lifecycle scripts: the offline build and harden-runner bound egress, not in-process behaviour, so a `build.rs` or transitive crate that opens a socket outside the fetch step stays (the per-crate capability cap is declined, gate-status.md) |
| SSOT Principle 11 English-only / string ownership (§6.7.1, §6.10 row 23) | §5.7, §6.10 — v1 is English-only with no i18n runtime: no locale-switch import, every `strings/ui.ts` key a non-empty English value, user-facing literals only in `strings/ui.ts` | G57 the English-only and string-ownership lint | — |
| build-time reviewer-API dependency (the one breach of the hermetic-CI principle) | build-loop §3 Step 5 — the dual review calls the Anthropic API on every box: the single sanctioned build-time network dependency, outside the minisigned-bytes boundary because it observes the artifact and produces none; the model IDs are recorded per commit | G1, a quality amplifier and never a security control · the phase-end auditable-smell spot-audit (§2) | the build session's one live network call; a spoofed endpoint, or a prompt injected through a committed fixture (diff content reaches the reviewer as data, never as instructions), can at most forge a `GO`, which suppresses the review trace the spot-audit relies on but cannot ship insecure code, because the deterministic gates are the control; the staged diff, fixtures included, leaves the machine to a third party, acceptable for an open-source repo |
| release-artifact completeness (the single backstop for a silently missing SBOM, CVE report or source bundle) | §6.7.2 — every required release asset is enumerated and present before publish | G58 the release-manifest meta-gate over the per-OS bundles, `SHA256SUMS` and its signature, the SBOM, the G17b open-CVE report, the licence files, the G38b source bundle, the measured sizes, the usability and name-clearance records, the release notes and the G59 attestation bundle with its `trusted_root.jsonl` | — |

## 6. Living-doc rules

- **What changes this document.** Control mechanics live in the owning spec §; gate
  mechanics live in the build-gates row. This document changes only when (i) a threat class
  is added or removed, (ii) the owning § or the verifying gate set of a class or a
  cross-cutting guarantee changes, (iii) an accepted residual is decided or changed, or (iv)
  the L(-1) policy, the plane structure or a principle changes. Such an edit is an owner act
  (L(-1)); as the tail of a Loop box it is a caged part, landed by a route of
  [build-loop.md](../process/build-loop.md) §3 Step 7.
- If a security control is *removed* or *weakened*, that is an escalation to the
  Co-Pilot session, never a silent edit.
- This doc and [build-gates.md](build-gates.md) are under plan-lint (build-gates §6): every
  `§` reference and link resolves (checks 2 and 25), every gate named here is catalogued
  (check 5), the threat rows match the canonical class set (check 8), and a cited live gate
  has its self-test (check 16).
- **Doc-graph freshness.** A change to a source of truth moves every referencing doc in the
  same commit (CLAUDE.md §5): G68 enforces the links, the orphans, the vanished gate and
  threat ids and the fingerprinted cage, and G69 the §0.7 structural map — their build-gates
  rows carry the mechanics; other drift is a G1 review item.
- **Context routing.** The Build-Loop runs lean and looks a gate up per box or red CI; the
  Co-Pilot carries the full gate catalogue and threat model
  ([roles-and-escalation.md](../process/roles-and-escalation.md) §1).
- **Current state only — git is the log.** This document carries no review-round logs or history
  sections; a decision lands in the section it governs. The retired P0 review reconciliation log
  (r1–r15) is read with `git show cf97638f7280:docs/security/security-concept.md`. A provenance
  citation — a review tag such as `r7`, a `[Test-Change: …]` tag, "the P<n>.<m> note" or
  "ruling" — is a historical pointer resolved through git: never a freshness target (G68), and
  not re-pointed when its source text is retired or condensed.
