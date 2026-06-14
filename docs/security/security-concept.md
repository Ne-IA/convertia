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
zero reviewer (G12 only checks the trailer is well-formed, never that two live models
actually ran, so this rule is the only thing preventing a well-formed-but-unbacked
`GO`). **Correlated-blind-spot residual (stated honestly):** Opus and Sonnet share
model lineage, so "both `GO`, 0 findings" is a *correlated* signal, not two independent
ones — the only retrospective backstop is the auditable-smell spot-audit above; whether
one reviewer should be a different model family (to make "independent" literally true)
is an owner call recorded here, not silently assumed.

## 3. Defense in depth — the enforcement planes

A change passes staged, independent defensive planes on its way from idea to a
published release. No plane trusts an earlier one to have caught everything.

| Plane | When | Mechanism | Blocks | Bypassable? |
|---|---|---|---|---|
| **L0 — Build-Loop per box** | While building, before each commit | The build-loop discipline + the **Opus + Sonnet dual review** on the staged diff (**no fix-push cycle** — no push between a fix and its re-review); P0/P1 findings fixed in the working tree before push | the commit (self-gate) | only by rule violation (no technical bypass) |
| **L1 — pre-commit hook** | `git commit` | Git-hook manager, `parallel`, budget < ~10 s | the commit | only `--no-verify` (**forbidden**) |
| **L2 — pre-push hook** (fires at `git push` time) | `git push` | Git-hook manager, budget < ~3 min; cheap-commit fastpath | the push | `--no-verify` (**forbidden**); legitimate fastpath skips for docs-only / check-off commits |
| **L3 — commit-msg hook** (L3 fires at `git commit`, chronologically **before** L2 which fires at `git push`; numbering is stable-by-assignment, not chronological) | `git commit` | Conventional-commit format check | the commit | git auto-subjects (merge/revert/fixup) allowed |
| **L4 — CI (GitHub Actions)** | After push | The same gates re-run on a clean checkout + the heavy gates (cross-platform build, corpus, coverage, SAST, SBOM) | a red `main` (fix immediately) | none for required checks — and **G56a** asserts in CI that the GitHub required-status-checks config exists (so "a red L4 blocks" is real repo state, not an invisible assumption) |
| **L5 — Release** | On a `v*` tag | Release workflow: SBOM + completeness, license hard-fail, copyleft-source-bundle present, **checksums + minisign over `SHA256SUMS`** (the *only* signing in scope — **not** binary code-signing/notarization, SSOT *Out of Scope*), size budget, egress/no-pollution observability gates | the release | none (release-blocking) |

**Two enforcement planes principle.** Every meaningful gate runs **locally (L1–L3)
and again in CI (L4)**. Local hooks give realtime feedback and keep `main` clean;
CI is the immutable backstop that proves green on a fresh clone. A red CI run is
fixed immediately — never re-run hoping it passes, never `--no-verify`.

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
    `cargo`, AND `npm`** ecosystems are all watched — the pin-and-watch discipline is
    symmetric across all three graphs, not actions-only; bump PRs are reviewed by
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
    fails on missing required checks / enabled force-push or deletion / admin-bypass.
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
    jobs, never on a self-hosted job where the free tier would be inert).

## 5. Threat model → control → gate

The spec's threat map (spec §0.11) is the **authoritative enumeration** — exactly
**16 classes**: `T1, T2, T2a, T2b, T2c, T3, T3a, T4, T5, T6, T7, T8, T9a, T9b, T10, T11`.
This table carries **one row per class** (including the `a`/`b`/`c` sub-rows),
mapping each to its primary runtime/build control **and** the concrete `Gnn` gate
that proves the control is in place. **§0.11 ↔ §5 parity is bidirectional and
machine-checked** (plan-lint check 8, §6 of [build-gates.md](build-gates.md)): every
§0.11 class has a row here, and every row cites a `Gnn`. A class with a runtime
control but no verifying gate is itself a gap.

| Threat (spec §0.11) | Primary control | Verifying gate (see [build-gates.md](build-gates.md)) |
|---|---|---|
| **T1** untrusted decoder input → crash/hang/exploit | engine isolation (§2.12) + invocation timeout/kill (§1.7) + pool bounds (§0.9); the in-core §1.2 detection layer is memory-safe Rust. **Honest residual `[DECIDED]` (stated, not implied-covered):** the §2.12.3 runtime privilege-drop (seccomp/Landlock · Seatbelt · AppContainer) is **best-effort and SILENTLY degrades** to a cheap tier — so T1 has **no LOAD-BEARING runtime containment** of an RCE inside a decoder; process-isolation bounds a *crash* (and a successful exploit's blast radius to one sandboxed sidecar + its scratch), but does not by itself stop code-exec inside that process. The load-bearing controls are isolation + the timeout/kill + the input never reaching the core; the privilege-drop tier is defence-in-depth that is *tracked* (G64 ratchet) but accepted as degradable | **G48** in-core detector fuzz (`cargo-fuzz` over `crate::detect`); **G31** per-pair corpus + reliability through the §2.12 boundary (engine-side T1) + the §2.12.3 **privilege-drop-tier-applied** + **memory-cap kill** + **Job-Object reap** positive assertions; **G64** privilege-drop-tier ratchet (a commit lowering an achieved tier fails/escalates — the silent-degrade can't quietly regress net coverage); **G26** full fuzz pass; isolation runtime test |
| **T2** malicious / compromised WebView content | §0.10 capability allowlist (no WebView `fs`/network) + CSP (no remote origins, `object-src 'none'`); the §0.10 by-construction hardening keys (`withGlobalTauri` off, `dangerousDisableAssetCspModification` off, release `devtools` off). **Un-pinnable runtime residual `[DECIDED]` (named, not implied-covered):** the **OS-provided WebView itself** (WebView2 Evergreen on Windows, WebKitGTK on Linux, WKWebView on macOS — a large untrusted-HTML-parsing C++ surface with its own CVE stream) is the named OS mechanism for T2 yet is **un-pinned, un-versioned, absent from the SBOM** — its integrity/CVE-currency is delegated to the OS update channel; ConvertIA's only side-bound is the G47 CSP lock (the WebView analogue of the G37c glibc-floor honesty) | **G47** CSP/capability structural lint (incl. the three §0.10 hardening keys); **G18** deny-list (no `tauri-plugin-updater`/HTTP-client crate); **G42** offline-egress monitor (Lane-B confirmation, macOS WebView gap noted) |
| **T2a** WebView steers a write to an attacker-chosen path | non-destructive create + write-target link-safety + divert (§2.1/§2.3.3/§2.7) | **G19/G31** fs-safety unit + property tests (no-clobber + link-safe on a WebView-supplied `ChosenRoot`); adversarial-path corpus (T7-shared) |
| **T2b** WebView re-submits an attacker-chosen SOURCE path | freeze-time §1.1 re-validation (canonicalise / resolve-identity / existence / detection at the §2.4 freeze) — provenance-independent | **G31** freeze re-validation property test (every C1 path re-validated regardless of provenance) |
| **T2c** WebView plugin-write surface (`store:default` + `log:default`) | bounded to `app_config_dir()`, no user-file contents (§7.4.2/§7.5). **Honest framing (spec §0.10):** `store:default` grants ALL store operations with **no per-file scope** — the single-`settings.json` confinement is a **code convention** (one compiled-in store name, one call site), **NOT** a runtime permission boundary; the primary enforced boundary is the §6.1.3 path-resolution assertion (the plugin cannot traverse out of `config_dir`), not a G47 scope. Worst-case harm is corrupt local prefs/log (clean reset recovers), never reading/exfil of user data | **G47** capability lint (only `store:default`/`log:default` granted, no broader `fs:`/`shell:` write surface — but G47 does **not** prove the single-store convention); **G38** §6.1.3 plugin-cannot-escape-`config_dir` path-resolution assertion (the primary boundary) **+ the G29 project-local single-store-name / single-`Store.load`-call-site Semgrep rule** (so an XSS/supply-chained second `Store.load(...)` call is caught) |
| **T3** bundled-binary supply chain | pinned + checksum-verified engines; build-time hash manifest; startup integrity. **Engine acquisition decided per engine per platform (spec §3.8, P0 review r3):** prebuilt-vs-from-source is an explicit policy because the two have different ground truths — **from-source** ⇒ the binary SHA is a build-output stability check, and provenance moves to the **signed source tarball + digest-pinned build toolchain/base image**; **prebuilt** ⇒ corroborate via **≥ 2 independent mirrors** OR a **distro GPG-signed package + signed repo metadata** (a bare hash of one unsigned download is unacceptable — it launders provenance, the xz/liblzma class). **FFmpeg** (which publishes no signature for the common gyan/BtbN prebuilts) has a named satisfiable anchor: from-source from the GPG-signed `ffmpeg.org` release, or a ≥ 2-provider cross-check. An **engine-source allow-list** constrains which hosts a pin (and its corroboration checksum) may come from, on independent origins. *Residual risk stated honestly:* startup integrity gives **no runtime tamper-resistance** (a whole-bundle swap swaps the in-bundle manifest too); the floor is the §6.2 SHA256SUMS + minisign anchor, not a runtime check (§3.8, §6.1.3, §6.3.4, §7.2.3) | **G37** engine-checksum build gate (verify vs in-repo `engines.lock` before staging + on cache-restore) + **pin-establishment provenance assertion** (the spec §3.8 acquisition-mode corroboration — named satisfiable source per engine incl. FFmpeg; any `engines.lock` SHA edit is a hard Co-Pilot escalation) + **engine-source allow-list assertion** (every `engines.lock` source URL ∈ the committed per-engine origin allow-list, pin URL and corroboration URL on independent origins); **G35** SBOM completeness; **G46** startup integrity verification; **G17b** *(informational, planted-positive self-tested)* OSV/grype CVE scan over `engines.lock` (PURL-keyed; the **FFmpeg CPE is MANDATORY** — the highest-CVE surface is FFmpeg's enabled internal decoders, which `pkg:generic` PURLs miss; the planted-positive uses a historical INTERNAL-decoder CVE) |
| **T3a** DLL/dylib/`.so` side-loading of a bundled codec shared object beside the engine `.exe` | every staged shared object individually `engines.lock`-rowed with its SHA-256 + verified before staging; engines spawned with a minimal explicit `PATH` (the bundle dir only, so the OS DLL/dylib search starts inside the bundle) + the §3.5 loader-injection-var strip (`LD_PRELOAD`/`LD_LIBRARY_PATH`/`DYLD_*` cleared); a staging-time dynamic-dependency-closure check that every non-system dependency resolves **inside** the bundle | **G37** per-shared-object SHA-256 verify (each `.dll`/`.dylib`/`.so`, not just the primary engine binary); **G35** manifest diff hard-fails on a staged shared object not matching its `engines.lock` row; **G37b** dynamic-dependency-closure assertion (`ldd`/`readelf -d` Linux · `otool -L` **AND `otool -l`** macOS — the `-l` leg enumerates `LC_RPATH`/`LC_LOAD_DYLIB` so an `@rpath` resolving OUTSIDE the bundle (Homebrew/`/usr/local`) is caught, which `otool -L` alone misses · `dumpbin /dependents` Windows — every non-system dep resolves inside the bundle, catching both a side-loading vector and an offline-floor break where an engine links a Homebrew/distro lib present only on the build runner) |
| **T4** open-file launch of a fresh artifact | §7.7 open-file safety (reveal-in-folder, no auto-open) + §7.7.3 Rust-side `RunResult`-membership check | **G15/G31** membership-check unit + integration test (only a current-run result path may be opened) |
| **T5** core panic / app fault | §2.13 app-level fault model (`catch_unwind` worker boundary) + §7.2 startup faults + §0.3.1 WebView-absent handling | **G15** panic-boundary unit test (panic → app-fault, not crash); **G46** missing/corrupt-engine → app-fault acceptance |
| **T6** copyleft aggregation boundary | §3.6 copyleft isolation (separately-invoked binaries, aggregation not linking); §0.3/§0.7 subprocess model | **G18** `cargo-deny` GPL/AGPL ban on the Rust crate graph; **G36** SBOM forbidden-family hard-fail; **G38b** LGPL-relink + GPL-corresponding-source bundle-present assertion (§6.1.3 ii/iii incl. x265 GPL §3); **G53** core-crate forbidden-dependency check (`cargo-deny [bans]` workspace-member-scoped — no image-worker C libs in the core closure) |
| **T7** path / link redirection (symlink/junction/TOCTOU) | §2.3 resolved-identity & link safety + §2.1 exclusive create-new-or-fail on the resolved real file | **G19/G31** atomic-publish/fs-safety unit + property tests; adversarial-path corpus |
| **T8** self-feeding / batch expansion | §2.4 frozen source set + §7.1 instance/run identity | **G15** frozen-set + per-run-ownership unit tests |
| **T9a** ConvertIA's own code exfiltrates user files | structural: opens no socket — no HTTP/updater on the §0.10 allowlist, no remote `connect-src`, no phone-home (§7.6) | **G47** CSP/capability lint + **G18** HTTP-client deny-list (no socket-opening dep ships); **G42** packet-monitor / egress-deny release gate (the proof) |
| **T9b** bundled engine reaches out / reads out-of-input on hostile input (incl. the LibreOffice macro-execution / `WEBSERVICE()`-external-data vectors) | load-bearing argv/build controls: FFmpeg `-protocol_whitelist file,pipe` + curated demuxers + `concat -safe 1`; pandoc `--sandbox` (pandoc ≥ 2.17, else the flag is silently ignored); LibreOffice hardened profile (`MacroSecurityLevel = 3` + `DisableMacrosExecution = true`, `LinkUpdateMode = 0`, no external-data-range / `WEBSERVICE()` refresh on load, §3.5.2); librsvg **no base URL** (§3.5.x). **Two load-bearing halves, each with its own armed enforcement substrate:** (a) **zero outbound packets** (the egress half) and (b) **no out-of-input FILE READ** (the read half — symmetric, not a mere oracle) | **G38** per-engine build assertions (`ffmpeg -protocols`/`-demuxers`, librsvg no-base-URL, `pandoc --version ≥ 2.17`, the **LibreOffice profile assertion** — parse the shipped `registrymodifications.xcu` and assert `MacroSecurityLevel`/`DisableMacrosExecution`/`LinkUpdateMode` + the external-data keys); **G31** corpus sentinels (a `.docm`/`.xlsm`/`.pptm` AutoOpen/`Workbook_Open` macro writing a canary inside the egress-deny window → canary **NOT** created; a `WEBSERVICE()` `.xlsx` → no egress/no out-of-input read), pulled forward into the per-push L4 leg; **G42** release-confirmation adversarial-egress monitor (the EGRESS half — zero outbound packets, armed-window canary, fail-closed); **G42b** the read-half fs-audit enforcement substrate (spec §6.4.2 — `ptrace`/Landlock, fail-CLOSED when neither is available, out-of-input sentinel + planted-positive, symmetric with G42 so the read half can never silently no-enforce) |
| **T10** resource exhaustion / DoS-by-input | §1.10 resource pre-flight & budgets + §0.9 pool/handle bounds + the to-GIF guardrail | **G16/G31** adversarial resource-budget corpus + property tests (oversized-render SVG, over-duration to-GIF, over-cardinality batch → fail-clearly, batch continues, no handle/RAM exhaustion); decompression-bomb fixtures (svgz/ZIP-in-OPC/nested-flate) |
| **T11** macOS engine-as-first-TCC-accessor (silent-deny) | §3.5.0/§7.2.6 macOS TCC source staging — the Rust core (holding the TCC grant from §1.1 freeze) copies a TCC-protected source into a per-job kind-2 scratch path (§2.14.2) **before** spawning, so a sidecar is never the first process to touch Desktop/Documents/Downloads/removable media (a chain-break otherwise triggers an invisible TCC denial / wrong-process prompt that defeats the conversion, and is silent on CI which runs from `TMPDIR`) | **G31** macOS sub-test (the Rust core PID, not the engine PID, is the first accessor of the protected path; the engine receives a kind-2 scratch path); **G29** Semgrep rule (every `Command::new` in `crate::isolation` under `cfg(target_os="macos")` is preceded by the stage-for-TCC / scratch-staging call) |

**Cross-cutting build/release controls** (not §0.11 threat classes, but
load-bearing security guarantees with their own gates):

| Guarantee | Primary control | Verifying gate |
|---|---|---|
| credential / secret in repo | no secrets committed; the real CI secrets `MINISIGN_SECRET_KEY` **and** `MINISIGN_PASSWORD` (§6.2.3) | **G2** `gitleaks` secrets scan using the **current** subcommands (`gitleaks` v8.19+ deprecated `protect`/`detect`): L1 `gitleaks git --staged` + **L2** `gitleaks git` over the unpushed range `@{u}..HEAD` (the last local catch before a secret goes public; **excluded from the docs-only fastpath** — secrets in `.md` count) + L4 full-tree `gitleaks dir` + a release-tier **full-history `gitleaks git`** leg over the commit log (a once-committed-then-removed secret stays live forever in a **public** OSS repo with no PR-review backstop); a committed `.gitleaks.toml` carries a **custom rule matching the minisign secret-key shape** (the `untrusted comment:` header co-occurring with a long base64 blob / a banned `*.key` staging) — `gitleaks`' default PEM rule keys on `-----BEGIN…-----` delimiters and a minisign key has none, so the PEM rule alone would **miss** it |
| authentic, verifiable download | per-file SHA-256 + minisign-signed `SHA256SUMS` + published verify recipe (§6.2) — the recipe is `minisign -Vm SHA256SUMS -p docs/minisign.pub` (**lowercase `-p` = public-key file path**; uppercase `-P` expects an inline base64 string and would fail on a path — standardised across README + spec §6.2.3/§6.2.4); an **out-of-band pubkey fingerprint** anchor (the in-repo `docs/minisign.pub` TOFU is otherwise circular — an attacker serving a tampered clone swaps artifact + `SHA256SUMS` + `.minisig` + pubkey together) | **G39** checksums + minisign over `SHA256SUMS` **+ a release-tier executable assertion that RUNS the exact documented recipe** against the just-produced `SHA256SUMS` + `.minisig` + committed pubkey and fails the release on non-zero (turns "recipe present" into "recipe correct and working"); **G44** verify recipe present + literal-form match; **G39/G44 sub-assertion** that `docs/minisign.pub` matches the fingerprint published out-of-band (a pinned README via the verified GitHub web UI / org page the pipeline cannot rewrite); the key-compromise/loss + coordinated-disclosure path lives in `vuln-response.md` (the human-readable retired/compromised-key commit IS the revocation channel for an offline app) |
| §7.5 log never carries file contents / full paths | redaction in the logging layer (§7.5) | **G15** (redaction property-test sub-case): a known secret-looking path stem fed through the logger is absent from the log |
| §2.14.1 per-run temp ownership + mode | per-run-owned scratch, `0o700` scratch root / `0o600` `.part` publish-temp | **G15/G31** temp-ownership + mode-bits assertion |
| hardened CI supply chain | least-privilege `permissions`, SHA-pinned actions (current via `dependabot.yml` covering **github-actions, cargo, and npm** ecosystems), lockfile-locked builds, no fork-PR release, pinned+digest-verified gate tools, per-workflow concurrency + `timeout-minutes` | **G49** `actionlint` (L1); **G50** `zizmor` (L4); **G18a** lockfile-integrity (`--locked`/`--frozen-lockfile` + `git diff --exit-code` lockfiles); **G56** `dependabot.yml` github-actions **+ cargo + npm** entries + push-workflow concurrency/timeout-minutes assertions |
| CI runner-host integrity (principle 11) | the secret-bearing signing job runs only on an ephemeral GitHub-hosted runner, host-isolated from the untrusted-corpus/fuzz jobs; **`step-security/harden-runner` (BLOCK mode) on the GitHub-hosted signing job ONLY** (its free/Community tier works only on GitHub-hosted runners; self-hosted needs Enterprise); on the shared self-hosted VPS the enforcement is **G42b ptrace/Landlock + G42 nftables/strace + the VPS egress allowlist + an ephemeral/JIT low-priv runner**, NOT harden-runner | **G56** workflow lint — a secret-using job bound to a self-hosted runner label is a hard fail; the corpus/fuzz job and the signing job assert disjoint runner hosts; **harden-runner presence/mode is asserted ONLY on GitHub-hosted jobs** (a self-hosted harden-runner claim would be inert on the free tier, so the lint must not require it there) |
| GitHub branch-protection / required-status-checks config | the single-branch direct-to-`main` model has no PR and no second reviewer, so the **only** thing that turns a red CI run into an actual block is repo config (required status checks present + required, force-push/deletion disabled, admin-bypass off) — invisible to the codebase and silently relaxable in the GitHub UI | **G56a** branch-protection config assertion — queries the GitHub ruleset/branch-protection API for `main` (`gh api repos/:owner/:repo/branches/main/protection` or the rulesets API) and fails if the agreed required checks are not all present-and-required, or `allow_force_pushes`/`allow_deletions`/admin-bypass is enabled (fail-soft only during the P0 bootstrap box, then hard) |
| JS/WebView supply-chain symmetry with Rust | committed `.npmrc` registry pin + resolution-URL guard (dependency-confusion defence); a frontend GPL/AGPL license deny over the pnpm graph; a committed minimal `onlyBuiltDependencies` allowlist (install-lifecycle-script lockdown) | **G17** `pnpm audit`; **G18c** `.npmrc` registry-pin + every `pnpm-lock.yaml` resolution URL ∈ the allowed registry; **G36b** frontend GPL/AGPL license hard-fail (cdxgen SBOM → `jq`/license filter); **G18d** `onlyBuiltDependencies` allowlist-growth lint (+ fail if `enable-pre-post-scripts`/`unsafe-perm` is set) |
| Principle-11 English-only / string-ownership (spec §6.7.1 / §6.10 row 23 — v1 is English-ONLY, no i18n runtime) | no locale-switch / i18n-runtime library import; every `strings/ui.ts` key resolves to a non-empty English value; user-facing literals live in `strings/ui.ts` | **G57** English-only / string-ownership lint (Lane-A, activated in P1) — fails on any locale-switch/i18n import, on an empty/missing `ui.ts` key, or a user-facing literal outside `strings/ui.ts` |
| build-time reviewer-API dependency (the ONE breach of the hermetic-CI principle, named honestly) | the G1 dual review calls the **Anthropic API** on every box — the single sanctioned build-time network dependency, the one place principle 10's "hermetic CI / everything that touches the minisigned bytes" is breached. It is **OUTSIDE the minisigned-bytes boundary** because it *observes but does not produce* the artifact. **Residuals stated:** (a) it is a live network call from the build session, the only one; (b) a spoofed/compromised endpoint could emit a **forged `GO`** — but G1 is **not** a security control (every `Gnn` except G1 is the real control), so a forged GO suppresses only the human review-trace the spot-audit relies on, it cannot ship insecure code; (c) the full staged diff (incl. any committed test-fixture material) leaves the machine to a third party — acceptable for an OSS repo, but a stated fact. Model IDs are pinned (§4) | **G1** (quality amplifier, not a security control) + the auditable-smell spot-audit (§4); mitigated structurally by "every Gnn except G1 is the deterministic control" |
| release-artifact completeness (the single backstop catching "the SBOM/CVE-report/source-bundle silently didn't get attached") | every required release asset enumerated + present before publish | **G58** release-manifest completeness meta-gate — fails the release if any of the per-OS bundle, `SHA256SUMS`, `SHA256SUMS.minisig`, SBOM file(s), dated open-CVE report (G17b), `NOTICE`/`THIRD-PARTY-LICENSES`, copyleft corresponding-source bundle (G38b), measured-sizes asset, `usability-floor.md`, `name-clearance.md`, or the §6.5.3 CHANGELOG/release-notes (with demoted/lossy pairs) is missing |

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
- **Boundary statement de-frozen.** "G2–G50" → "every gate except G1" (the catalogue
  now reaches G56, and G53/G55 ARE security controls).
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
  offline bundle asset.
- **Living-doc/spec-sync (r4).** Spec §0.10 (the three by-construction hardening keys),
  spec §0.7 (main.json directory comment), spec §6.7.2 + §6 CI-hardening (harden-runner
  runner-type reality), spec §2.14 (scratch-residue confidentiality accepted-residual) are
  owned by the spec and edited in this same change (SSOT > spec > docs).
