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
  branches, no push-lock coordination, no separate feedback/sniping sessions.
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
are the deterministic gates G2–G50** (a gate either passes on a clean checkout or it
does not). The dual review raises quality and catches design defects the gates can't
encode; its evidence trail (each reviewer's findings + convergence/divergence
recorded verbatim in the commit body) makes a "both GO, 0 findings" on a non-trivial
diff an **auditable smell** for periodic Co-Pilot spot-audit. Conflict order for the
Build-Loop: **SSOT > spec > these security/process docs > code > conversation.**

## 3. Defense in depth — the enforcement planes

A change passes staged, independent defensive planes on its way from idea to a
published release. No plane trusts an earlier one to have caught everything.

| Plane | When | Mechanism | Blocks | Bypassable? |
|---|---|---|---|---|
| **L0 — Build-Loop per box** | While building, before each commit | The build-loop discipline + the **Opus + Sonnet dual review** on the staged diff (**no fix-push cycle** — no push between a fix and its re-review); P0/P1 findings fixed in the working tree before push | the commit (self-gate) | only by rule violation (no technical bypass) |
| **L1 — pre-commit hook** | `git commit` | Git-hook manager, `parallel`, budget < ~10 s | the commit | only `--no-verify` (**forbidden**) |
| **L2 — pre-push hook** | `git push` | Git-hook manager, budget < ~3 min; cheap-commit fastpath | the push | `--no-verify` (**forbidden**); legitimate fastpath skips for docs-only / check-off commits |
| **L3 — commit-msg hook** | `git commit` | Conventional-commit format check | the commit | git auto-subjects (merge/revert/fixup) allowed |
| **L4 — CI (GitHub Actions)** | After push | The same gates re-run on a clean checkout + the heavy gates (cross-platform build, corpus, coverage, SAST, SBOM) | a red `main` (fix immediately) | none for required checks |
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
    protect (it holds `MINISIGN_SECRET_KEY`): every workflow declares
    least-privilege `permissions`, every third-party action is pinned by full
    commit SHA, the build resolves only the committed lockfiles (`--locked` /
    `--frozen-lockfile`, no silent graph drift), and a workflow security lint
    (G49/G50) runs in both planes. The secret-bearing release job never runs on a
    fork pull-request.

## 5. Threat model → control → gate

The spec's threat map (spec §0.11) is the **authoritative enumeration** — exactly
**14 classes**: `T1, T2, T2a, T2b, T2c, T3, T4, T5, T6, T7, T8, T9a, T9b, T10`.
This table carries **one row per class** (including the `a`/`b`/`c` sub-rows),
mapping each to its primary runtime/build control **and** the concrete `Gnn` gate
that proves the control is in place. **§0.11 ↔ §5 parity is bidirectional and
machine-checked** (plan-lint check 8, §6 of [build-gates.md](build-gates.md)): every
§0.11 class has a row here, and every row cites a `Gnn`. A class with a runtime
control but no verifying gate is itself a gap.

| Threat (spec §0.11) | Primary control | Verifying gate (see [build-gates.md](build-gates.md)) |
|---|---|---|
| **T1** untrusted decoder input → crash/hang/exploit | engine isolation (§2.12) + invocation timeout/kill (§1.7) + pool bounds (§0.9); the in-core §1.2 detection layer is memory-safe Rust | **G48** in-core detector fuzz (`cargo-fuzz` over `crate::detect`); **G31** per-pair corpus + reliability through the §2.12 boundary (engine-side T1); **G26** full fuzz pass; isolation runtime test |
| **T2** malicious / compromised WebView content | §0.10 capability allowlist (no WebView `fs`/network) + CSP (no remote origins, `object-src 'none'`) | **G47** CSP/capability structural lint; **G18** deny-list (no `tauri-plugin-updater`/HTTP-client crate); **G42** offline-egress monitor (Lane-B confirmation, macOS WebView gap noted) |
| **T2a** WebView steers a write to an attacker-chosen path | non-destructive create + write-target link-safety + divert (§2.1/§2.3.3/§2.7) | **G19/G31** fs-safety unit + property tests (no-clobber + link-safe on a WebView-supplied `ChosenRoot`); adversarial-path corpus (T7-shared) |
| **T2b** WebView re-submits an attacker-chosen SOURCE path | freeze-time §1.1 re-validation (canonicalise / resolve-identity / existence / detection at the §2.4 freeze) — provenance-independent | **G31** freeze re-validation property test (every C1 path re-validated regardless of provenance) |
| **T2c** WebView plugin-write surface (`store:default` + `log:default`) | bounded to `app_config_dir()`, compiled-in store name, no user-file contents (§7.4.2/§7.5) | **G47** capability lint (only `store:default`/`log:default` granted, no broader write surface); **G38** §6.1.3 plugin-cannot-escape-`config_dir` assertion |
| **T3** bundled-binary supply chain | pinned + checksum-verified engines; build-time hash manifest; startup integrity; trust anchor = §6.2 SHA256SUMS + minisign verified before first run (§3.8, §6.1.3, §7.2.3) | **G37** engine-checksum build gate (verify vs in-repo `engines.lock` before staging + on cache-restore); **G35** SBOM completeness; **G46** startup integrity verification; **G17b** *(informational)* OSV/grype CVE scan over `engines.lock` |
| **T4** open-file launch of a fresh artifact | §7.7 open-file safety (reveal-in-folder, no auto-open) + §7.7.3 Rust-side `RunResult`-membership check | **G15/G31** membership-check unit + integration test (only a current-run result path may be opened) |
| **T5** core panic / app fault | §2.13 app-level fault model (`catch_unwind` worker boundary) + §7.2 startup faults + §0.3.1 WebView-absent handling | **G15** panic-boundary unit test (panic → app-fault, not crash); **G46** missing/corrupt-engine → app-fault acceptance |
| **T6** copyleft aggregation boundary | §3.6 copyleft isolation (separately-invoked binaries, aggregation not linking); §0.3/§0.7 subprocess model | **G18** `cargo-deny` GPL/AGPL ban on the Rust crate graph; **G36** SBOM forbidden-family hard-fail; **G38b** LGPL-relink + GPL-corresponding-source bundle-present assertion (§6.1.3 ii/iii incl. x265 GPL §3); core-crate forbidden-dependency check (no image-worker C libs in the core closure) |
| **T7** path / link redirection (symlink/junction/TOCTOU) | §2.3 resolved-identity & link safety + §2.1 exclusive create-new-or-fail on the resolved real file | **G19/G31** atomic-publish/fs-safety unit + property tests; adversarial-path corpus |
| **T8** self-feeding / batch expansion | §2.4 frozen source set + §7.1 instance/run identity | **G15** frozen-set + per-run-ownership unit tests |
| **T9a** ConvertIA's own code exfiltrates user files | structural: opens no socket — no HTTP/updater on the §0.10 allowlist, no remote `connect-src`, no phone-home (§7.6) | **G47** CSP/capability lint + **G18** HTTP-client deny-list (no socket-opening dep ships); **G42** packet-monitor / egress-deny release gate (the proof) |
| **T9b** bundled engine reaches out / reads out-of-input on hostile input | load-bearing argv/build controls: FFmpeg `-protocol_whitelist file,pipe` + curated demuxers + `concat -safe 1`; pandoc `--sandbox`; LibreOffice hardened profile; librsvg **no base URL** (§3.5.x) | **G38** per-engine build assertions (`ffmpeg -protocols`/`-demuxers`, librsvg no-base-URL); **G31/G42** adversarial-egress corpus (§6.4.2) inside the egress-deny window (zero egress **and** no out-of-input read) |
| **T10** resource exhaustion / DoS-by-input | §1.10 resource pre-flight & budgets + §0.9 pool/handle bounds + the to-GIF guardrail | **G16/G31** adversarial resource-budget corpus + property tests (oversized-render SVG, over-duration to-GIF, over-cardinality batch → fail-clearly, batch continues, no handle/RAM exhaustion); decompression-bomb fixtures (svgz/ZIP-in-OPC/nested-flate) |

**Cross-cutting build/release controls** (not §0.11 threat classes, but
load-bearing security guarantees with their own gates):

| Guarantee | Primary control | Verifying gate |
|---|---|---|
| credential / secret in repo | no secrets committed; real CI secret `MINISIGN_SECRET_KEY` | **G2** `gitleaks` secrets scan (L1 `--staged` + L4 full) |
| authentic, verifiable download | per-file SHA-256 + minisign-signed `SHA256SUMS` + published verify recipe (§6.2) | **G39** checksums + minisign over `SHA256SUMS`; **G44** verify recipe present |
| §7.5 log never carries file contents / full paths | redaction in the logging layer (§7.5) | **G15** redaction property test (known secret-looking path stem → absent from log) |
| §2.14.1 per-run temp ownership + mode | per-run-owned scratch, `0o700` scratch root / `0o600` `.part` publish-temp | **G15/G31** temp-ownership + mode-bits assertion |
| hardened CI supply chain | least-privilege `permissions`, SHA-pinned actions, lockfile-locked builds, no fork-PR release | **G49** `actionlint` (L1); **G50** `zizmor` (L4); **G18a** lockfile-integrity (`--locked`/`--frozen-lockfile` + `git diff --exit-code` lockfiles) |

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

- **§0.11 ↔ §5 parity (closed).** §5 now carries one row per §0.11 class — all 14
  (`T1, T2, T2a, T2b, T2c, T3, T4, T5, T6, T7, T8, T9a, T9b, T10`) — each naming a
  primary control **and** a concrete `Gnn`. Bidirectional parity is enforced
  mechanically by plan-lint check 8.
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
