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

## 3. Defense in depth — the enforcement planes

A change passes staged, independent defensive planes on its way from idea to a
published release. No plane trusts an earlier one to have caught everything.

| Plane | When | Mechanism | Blocks | Bypassable? |
|---|---|---|---|---|
| **L0 — Build-Loop per box** | While building, before each commit | The build-loop discipline + the **Opus + Sonnet dual review** on the staged diff; P0/P1 findings fixed in the working tree before push | the commit (self-gate) | only by rule violation (no technical bypass) |
| **L1 — pre-commit hook** | `git commit` | Git-hook manager, `parallel`, budget < ~10 s | the commit | only `--no-verify` (**forbidden**) |
| **L2 — pre-push hook** | `git push` | Git-hook manager, budget < ~3 min; cheap-commit fastpath | the push | `--no-verify` (**forbidden**); legitimate fastpath skips for docs-only / check-off commits |
| **L3 — commit-msg hook** | `git commit` | Conventional-commit format check | the commit | git auto-subjects (merge/revert/fixup) allowed |
| **L4 — CI (GitHub Actions)** | After push | The same gates re-run on a clean checkout + the heavy gates (cross-platform build, corpus, coverage, SAST, SBOM) | a red `main` (fix immediately) | none for required checks |
| **L5 — Release** | On a `v*` tag | Release workflow: SBOM, license hard-fail, checksums + minisign, signing/provenance, size budget, observability gates | the release | none (release-blocking) |

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
   Rust-side only. (spec §0.10)
7. **Authentic, verifiable downloads.** Release artifacts carry per-file SHA-256 +
   a minisign-signed `SHA256SUMS`; the download page carries the verification
   recipe. (spec §6.2)
8. **No stubs, no drift.** Every change is production-ready (CLAUDE.md): no
   `TODO`/`FIXME`/`unimplemented!`/`console.log` in production code without a
   tracked box-id; generated artifacts never drift from source.

## 5. Threat model → control → gate

The spec's threat map (spec §0.11 + §2.x) is the authoritative enumeration. This
table maps each threat to its primary runtime/build control **and** to the gate
that proves the control is in place. *(Exact `T#` numbering and completeness are
reconciled against spec §0.11 during review — see the open item in §7.)*

| Threat (spec) | Primary control | Verifying gate(s) (see [build-gates.md](build-gates.md)) |
|---|---|---|
| **T1** malicious input → crash/hang | decoder isolation + resource limits (§2.12) | fuzz gate (`cargo-fuzz` on decode path); per-pair corpus incl. malformed fixtures; isolation runtime test |
| **T3** supply-chain (tampered engine) | pinned + checksum-verified engines; build-time hash manifest; startup integrity (§3.8, §6.1.3, §7.2.3) | engine-checksum build gate; SBOM completeness; startup integrity verification |
| **T7** symlink/hardlink/junction onto source | resolved-identity link-safety (§2.3) | atomic-publish/fs-safety unit + property tests; adversarial-path corpus |
| **T8** source-set self-feeding | frozen snapshot + identity de-dup (§2.4) | frozen-set unit tests |
| **T9a / T9b** SSRF / local-file-read via engine | FFmpeg `-protocol_whitelist file,pipe`; librsvg no-base-URL; pandoc `--sandbox`; LibreOffice hardened profile (§3.5.x) | per-engine build assertions; adversarial-egress corpus (§6.4.2); offline-egress observability gate |
| **(supply/secret) credential leak** | no secrets in repo | secrets-scan gate (L1) |
| **(license) copyleft contamination** | copyleft isolation (§3.6) | `cargo-deny` license bans; SBOM forbidden-license-family hard-fail |
| **(integrity) tampered download** | signed checksum manifest (§6.2) | release signing gate; published verification recipe |

> Every row above also lists, in [build-gates.md](build-gates.md), the concrete
> tool, the plane it runs at, and its fail-open/closed posture. Any threat in spec
> §0.11 **without** a row here is a gap to be closed (review item §7).

## 6. Living-doc rules

- A control or gate that changes during the build is updated **here first**, in
  the same commit as the change, with a one-line rationale.
- If a security control is *removed* or *weakened*, that is an escalation to the
  Co-Pilot session, never a silent edit.
- This doc and [build-gates.md](build-gates.md) are themselves under the
  doc-consistency gate (`plan-lint`/`spec-lint`, P0.3): every `§` reference must
  resolve and every gate named here must exist in the catalogue.

## 7. Open / to-reconcile (closed during P0 review)

- [ ] Reconcile the full `T1–T10` enumeration + `T9a/T9b` split against spec
  §0.11 and confirm **every** threat has a control + gate row in §5.
- [ ] Confirm no threat-class is left without a *verifying gate* (vs. only a
  runtime control) — runtime controls still need an adversarial test that proves
  them.
