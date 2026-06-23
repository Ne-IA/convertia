# Gate-status ledger — owner-decidable / informational-then-ratcheted gates

> **The committed decision-log for every gate whose required-vs-informational posture
> is an OWNER decision** (not a deterministic always-on gate). A status change is a
> dated committed line here — never an invisible flip. Asserted present + status-agreeing
> by [`plan-lint`](../../scripts/plan-lint) **check 23** (the catalogue defines the check in
> [build-gates.md](../security/build-gates.md) §6 check 23 — the G68 sibling that keeps a
> posture flip from being silent). The owner-decidable backstops below were parked through
> the P0 security review ([security-concept.md](../security/security-concept.md) §8
> reconciliation notes) and are recorded here as contracts by
> [`P0.4.5`](../plan/P0-build-and-security.md).

## Scope

The deterministic gates — **every `Gnn` except G1** — are always-on; their **posture** is
not owner-decidable, so they are **not** in the ratchet ledger below. Their one-time
**bootstrap-skip → fail-closed ACTIVATION** (a P0 `→ activated in P<n>` gate flipping the
moment its target lands) is a distinct event class, logged in the **"P1 gate-activation
flips"** section below (authored by P1.62) — that log carries **no `Status`/`Since` posture
columns**, so `plan-lint` check 23 (which governs only the owner-decidable ratchet table)
does not parse it. This ledger tracks only the gates whose posture is an owner/ratchet
decision (`informational` ↔ `required`), the purely-informational census tools, plus
one-time **`decided`** owner-adopt rows (a gate the owner adopted/declined once — e.g.
**G59** build-provenance — recorded so the adopt is a dated committed line, not a buried
catalogue footnote; a `decided` row carries no ratcheting posture, so it is **not** in the
`plan-lint` check-23 `_OWNER_DECIDABLE_GATES` map).
Each box that introduces such a gate appends its row in the **same commit**:

- **P0.4.5** (this file's creator) — the four over-assurance behavioural backstops below.
- **P0.5.10** — `cargo-mutants` (scoped mutation testing), the fifth ledger row (appended by this box).
- **P0.7.6** — **G59** (build-provenance attestation), a one-time **`decided`** adopt row — recorded here so the adopt is dated; NOT a ratcheting posture, so it is deliberately absent from the check-23 `_OWNER_DECIDABLE_GATES` map.
- **P0.7.14** — **G64** (privilege-drop-tier ratchet) + the formal flip protocol.
- **P0.7.15** — **G65** (engine-subprocess coverage-guided fuzz), a reserved-not-row id registered `informational`; owner flips →`required` on the before-P10 also-per-push decision.
- **P0.7.7** — **G17b** (bundled-engine CVE awareness), `informational` per-push; owner flips →`required` via the CVSS ≥ 7-on-an-actively-exercised-§04-path release escalation.

**Status values.** `informational` (runs, never blocks the build) · `required`
(blocks the build) · `decided` (a one-time adopt/decline owner decision). **A flip edits
BOTH this ledger's row (its status + its `Since` date) AND the `plan-lint` check-23
effective-posture map (`_OWNER_DECIDABLE_GATES`) in the SAME owner-acked L(-1) commit** —
the dual record that makes an `informational`↔`required` change a dated, auditable line
rather than an invisible drift.

## Ledger

| Gate / tool | Status | Since | Activation | Contract (one line) |
|---|---|---|---|---|
| `cargo-acl` / cackle | informational | 2026-06-18 | P1 | `cackle.toml` denies `std::net` graph-wide + `std::process::Command` to `crate::isolation` only — catches a renamed/transitive network crate that G18's name-ban and G29 rule (g) both miss |
| `cargo-careful` | informational | 2026-06-18 | P1 | nightly wrapper adding extra std debug assertions + runtime-UB checks on the untrusted-byte detect/`fs_guard` path (Principle 9) |
| Kani | informational | 2026-06-18 | P1 | bounded model checking that PROVES the small numeric caps (≤100× decompression ratio, `MAX_SVGZ_SNIFF` ≤64 KiB, the `fs_guard` predicates) rather than fuzzer-hoping them |
| `cargo-geiger` | informational | 2026-06-18 | P1 | `unsafe`-usage census over the dependency graph — informational-forever (a visibility tool; it never ratchets to `required`) |
| `cargo-mutants` | informational | 2026-06-19 | P3 (P3.72) | scoped mutation testing over `crate::fs_guard`+`crate::detection`+`crate::outcome` (the no-harm/atomicity/no-misroute kernel), a **G15** sub-leg — line coverage proves a line RAN, not that a test would CATCH a regression there; owner flips `informational`→`required` once survived-mutants reach **0** for `crate::fs_guard`+`crate::detection` (the P3.72 first run + the decrease-only per-crate `max_survived_mutants.toml` ratchet) |
| **G59** — build-provenance attestation (`actions/attest-build-provenance`) | decided | 2026-06-19 | P10 | a v1 OWNER DECISION (promoted from a post-v1 deferral): the one genuinely-free build-**ORIGIN** signal — binds the artifact to runner+workflow+commit, so a silently re-signed release from a poisoned shared VPS is detectable **even if the minisign key leaked**; additive to minisign, **NOT** binary code-signing; needs only `id-token: write` scoped to the release/attestation job. **VERIFIED, not just generated** — a release step runs `gh attestation verify` (fail-on-non-zero); the **Sigstore bundle + a paired `trusted_root.jsonl`** ship as named release assets for OFFLINE verify; both join the **G58** completeness enumeration. `decided` = a one-time adopt → NOT in the check-23 `_OWNER_DECIDABLE_GATES` posture map; §8/catalogue/box statuses agree (check 17: the §8 entry is PROMOTED, not a live deferral) |
| **G17b** — bundled-engine CVE awareness (`osv-scanner`/`grype`) | informational | 2026-06-19 | P10 | informational per-push OSV/grype over the **PURL-keyed** `engines.lock` (a planted-positive — a known historical internal-FFmpeg-decoder CVE — guards the empty-report-masquerading-as-clean failure; the FFmpeg CPE `cpe:2.3:a:ffmpeg:ffmpeg:<ver>` is MANDATORY); emits a dated open-CVE report (recording the advisory-DB age) as an owner-signed-off release asset; offline-tolerant (vendored DB, refresh warn-only). Owner flips `informational`→`required` via the **CVSS ≥ 7 on an actively-exercised §04 path → release-blocking escalation** (recorded in `vuln-response.md` / `SECURITY.md`); the release-tier advisory-DB-staleness floor (`MAX_ADVISORY_DB_STALENESS`) is shared with **G17**. A flip edits BOTH this row AND the check-23 `_OWNER_DECIDABLE_GATES` map in the same owner-acked L(-1) commit |
| **G64** — privilege-drop-tier ratchet | informational | 2026-06-19 | P9 | records the achieved §2.12.3 privilege-drop tier **per platform** into a tracked `privilege-drop-coverage.toml`, **decrease-guarded** like the coverage floor / `max_survived_mutants.toml` (a commit lowering an achieved tier fails/escalates; raises are deliberate) — the §2.12.3 runtime containment of the untrusted C/C++ decoders is best-effort and silently degrades (the T1 honest residual), and G31 proves the tier FIRED on the runner but nothing tracked the TREND, so G64 makes a NET regression visible. Owner flips `informational`→`required` once the §2.12.3 tier matrix **stabilises** (informational while it is filled in P4–P9). A flip edits BOTH this row AND the check-23 `_OWNER_DECIDABLE_GATES` map in the same owner-acked L(-1) commit |
| **G65** — engine-subprocess coverage-guided fuzz *(reserved id)* | informational | 2026-06-19 | P9/P10 | a **reserved-not-row** id (named in prose, never a `· G65` header ref): the engine-side T1 surface (bundled C/C++ decoders on untrusted bytes) is covered today only by a fixed fault-injected corpus (G26/G31) — G65 adds a **black-box mutational fuzz** of the **real G37-staged SHA-256-verified sidecar** (AFL++ binary-only/QEMU **OR** a `radamsa` harness through the §2.12 isolation wrapper; `zzuf` LD_PRELOAD for LibreOffice headless), reusing the §6.4.2 oracles (no-crash-escapes-boundary + no-egress + no-out-of-input-read via **G42b**), CI-host resource-bounded (cgroup/`ulimit`/`docker --memory` + the G56 `timeout-minutes`). Pre-committed to a **REQUIRED SCHEDULED non-PR-blocking** job (≥ weekly `radamsa`-through-the-isolation-wrapper that FILES AN ISSUE on a boundary-escaping crash, an issue-opener like G66). Owner flips `informational`→`required` on the BEFORE-P10 decision whether to ALSO make it per-push. A flip edits BOTH this row AND the check-23 `_OWNER_DECIDABLE_GATES` map in the same owner-acked L(-1) commit |

None of the four over-assurance backstops replaces **G48**'s fuzz; each is an
**additive** proof/observation layer on top of the deterministic gates, which the
owner may adopt. `cargo-mutants` (the fifth row, P0.5.10) is likewise additive to
G48 — fuzzing finds crashes on hostile input; mutation testing finds assertions the
tests forgot to make. It is detailed in its own section below.

## P1 gate-activation flips (deterministic gates: bootstrap-skip → fail-closed)

The P0 build authored a set of deterministic gates carrying a `→ activated in P<n>` annotation:
each **fail-opens / skips-with-warning while its target is absent** (so the empty P0 tree neither
wedges the green-L4 exit criterion nor leaves the gate silently fail-open once its target lands) and
**fail-closes the moment its target exists**. **P1.62** is the single owner that proves the flip
actually happened — for each gate, a planted violation in the now-real target MUST fail it (the gate
is *enforcing*, not stuck in its bootstrap skip). This log records each flip as a dated committed
line. Unlike the owner-decidable ratchet ledger above, these are **deterministic, one-time**
activations (no `informational`↔`required` posture), so the table below deliberately carries **no
`Status`/`Since` columns** (check 23 governs only the ratchet ledger). The reverse `→ activated in P<n>`
edges live on the P0 gate rows in [build-gates.md](../security/build-gates.md); this is their closing
side. **G69** (the §1a structural-map ↔ on-disk bijection) activated at **P1.64** — the P1-END
structure-establishment box — and its row is below. The remaining two `→ activated in P1`-annotated
gates are NOT rows here yet, by design: **G22/G23** (the format membership/completeness mirror gates)
activate when their §04-matrix / corpus / `convert_*` targets stand up in **P3–P7**, so they add their
rows then. (The continuously-active scanners G8 / G29 / G71 are a different class — see the P1.62 plan note.)

| Gate | Activated-in | Now-real target (P-box) | Negative self-test — a planted violation MUST fail | Flipped |
|---|---|---|---|---|
| **G47** — WebView CSP + Tauri capability lint | P1.62.1 | `tauri.conf.json` + `capabilities/main.json` (P1.18–P1.21) | `g24-csp-capabilities` — a mis-encoded CSP directive / an `fs:`/`http:`/`shell`/`opener:`/`dialog:` grant / a present updater block fails | 2026-06-23 |
| **G19** — generated-artifact drift | P1.62.2 | `src/lib/ipc/bindings.ts` (P1.26 / regen wired P1.53) | `g24-generated-drift` — a stale / hand-edited / un-regenerated artifact fails the regen + `git diff --exit-code` | 2026-06-23 |
| **G27** — per-domain coverage floors | P1.62.3 | `cargo llvm-cov` + Vitest v8 reports (P1.54) | `g24-coverage` — a measured domain below its `[line]` floor fails (never averaged) | 2026-06-23 |
| **G28** — diff-coverage gate | P1.62.4 | the per-line lcov reports (P1.54) | `g24-coverage` — changed executable product lines < 80 % covered fails (`_diff_verdict`) | 2026-06-23 |
| **G33a** — automated a11y (vitest-axe jsdom) | P1.62.5 | the rendered React tree (P1.35 / P1.56) | `src/a11y/g33a-canary.a11y.test.tsx` — axe MUST report ≥ 1 violation on a planted invalid ARIA role (the leg is armed) | 2026-06-23 |
| **G57** — Principle-11 English-only lint | P1.62.6 | `src/strings/ui.ts` (P1.37) | `g24-english-only` — a non-English user-facing literal / an i18n-runtime import fails | 2026-06-23 |
| **G53** — core-crate forbidden-dependency walk | P1.62.7 | the Cargo workspace `convertia-core` (P1.6) | `g24-core-deps` — a forbidden lib (updater / HTTP-client / imgworker C libs) in the core closure fails | 2026-06-23 |
| **G30** — cross-platform build-matrix | P1.62.8 | the 3-OS `compile-sanity` matrix (P1.58); the universal-sidecar fat-Mach-O slice assertion stays target-absent until the P10 release staging | **gate-logic self-test:** `g24-build-matrix` plants a single-arch fat-named Mach-O that MUST fail the slice-assertion. **Separate live CI enforcer (not the G30 gate script):** the P1.58 `compile-sanity` job reddens on a platform-specific compile break | 2026-06-23 |
| **G18 / G18a–d** — supply-chain + lockfile integrity | P1.62.9 | `Cargo.lock` / `pnpm-lock.yaml` / `.npmrc` (P1.6 / P1.59 / P1.60) | `g24-supply-chain` + `g24-lockfile-integrity` + `g24-js-supply-chain` — a non-frozen lockfile / bad resolution URL / lifecycle-script-enabled dep / forbidden crate fails | 2026-06-23 |
| **G69** — §1a structural-map ↔ on-disk-tree bijection | P1.64 | the CLAUDE.md §1a Repo-layout map + the spec §0.7 physical tree (§1a authored, §0.7 expanded to all 60 tracked dirs, the `PLACEHOLDER` stub removed → the skip lifted; the `spec-0.7-physical-tree` G68 fingerprint re-blessed same-commit) | `g24-plan-lint` check-26 — a planted on-disk dir absent from §1a (disk-not-in-map) / a §1a dir not on disk (map-not-on-disk) / a §1a dir §0.7 does not home (map-not-in-spec07) MUST fail | 2026-06-23 |

The four **owner-decidable over-assurance contracts** (`cargo-acl`/cackle, Kani, `cargo-careful`,
`cargo-geiger`) ALSO carry `→ activated in P1`, but — being `informational`-only, not fail-closed —
their activation is the **presence** of their dated `informational` rows in the ratchet ledger above
(P1.62.10, a check over those entries), NOT a planted-violation self-test.

## Over-assurance behavioural backstops (P0.4.5 · §1.2 · G29 G48)

Each contract is `→ activated in P1` (the dependency graph, the crate roots, and the
numeric-cap code land in P1+); all four are **informational-only in P0** and stay so
until an explicit owner decision flips one to `required` (recorded here per the flip
protocol above). Three (`cargo-acl`, `cargo-careful`, Kani) can ratchet to `required`;
`cargo-geiger` is informational-forever.

### `cargo-acl` / cackle — dependency-graph capability cap

A committed `cackle.toml` denying the **`std::net`** capability to the WHOLE dependency
graph and **`std::process::Command`** to `crate::isolation` only (the one module that
legitimately spawns the bundled engines). This is a **build-time graph check**
(Linux-only). It is **additive to G18 and G29 rule (g)**: G18 bans network crates **by
name** and G29 rule (g) greps first-party source for `std::net`, but a **renamed or
transitive** network-capable crate pulled in deep in the graph escapes both — cackle's
capability-graph analysis catches exactly that class. **Owner decision:**
informational-then-required (the owner flips it once the dependency graph is stable and
the build-time cost is acceptable in CI).

### `cargo-careful` — runtime-UB / extra-assertion wrapper

Runs the in-core test suite under `cargo +nightly careful test` on the Linux and macOS
nightly legs, enabling extra standard-library debug assertions and runtime
undefined-behaviour checks (uninitialized-memory reads, invalid enum discriminants, and
similar) specifically on the **untrusted-byte detect / `fs_guard` path** (SSOT
Principle 9 — the bytes ConvertIA ingests are arbitrary and possibly hostile, so the
code that first touches them gets the strictest runtime checking available). It is
**additive to the deny-`unsafe` policy (G29)**: G29 forbids new `unsafe` outside the one
FFI module statically, while `cargo-careful` exercises the std-internal soundness
assumptions at run time. **Owner decision:** informational-then-required once the
nightly leg is stable.

### Kani — bounded model checking of the numeric caps

Bounded model checking (`kani`) that **proves** — rather than fuzzer-hopes — the small,
finite numeric caps the safety story depends on: the ≤100× decompression-ratio bound,
`MAX_SVGZ_SNIFF` ≤64 KiB, and the `fs_guard` path-classification predicates. These caps
are small enough to be tractable for a SAT/SMT-backed proof over all inputs in the
bounded domain, which is strictly stronger than G48's fuzzing (fuzzing samples the input
space; Kani exhausts the bounded one). It is **additive to G48**, not a replacement.
**Owner decision:** informational-then-required once the proof harnesses are written and
the proof time is acceptable in CI.

### `cargo-geiger` — `unsafe`-usage census

A reporting tool that counts `unsafe` blocks/functions across the dependency graph,
giving a visible census of where `unsafe` lives in third-party crates. It is **purely
informational — it never ratchets to `required`** (a visibility aid, not a pass/fail
gate; the enforced `unsafe` policy is G29). It is recorded here so the decision to keep
it informational-forever is itself a dated, auditable line.

## Scoped mutation testing — `cargo-mutants` (P0.5.10 · §6.4 · G15)

A **release-tier** mutation-testing sub-leg of **G15** over the safety kernel
`crate::fs_guard` + `crate::detection` + `crate::outcome` (the no-harm / atomicity /
no-misroute kernel). Line coverage proves a line **executed**; it does **not** prove a
test would CATCH a regression there — `cargo-mutants` mutates the kernel's code and
**fails if a mutation survives the test suite** (a gap a coverage percentage hides). It
is **additive to G48**, not a replacement: G48's fuzz finds crashes on hostile input;
mutation testing finds the assertion a test forgot to make on benign input.

**Owner decision (required-vs-informational, like G17b).** Recorded `informational`
here; the owner flips it to `required` once the survived-mutant count reaches **0** for
`crate::fs_guard` + `crate::detection` (the two kernels whose silent regression breaks the
no-harm / no-misroute guarantees). A flip edits BOTH this ledger row (status + `Since`)
AND the `plan-lint` check-23 `_OWNER_DECIDABLE_GATES` posture map in the same owner-acked
L(-1) commit (the flip protocol above).

**Activation (the end-of-P3 [GATE] box `P3.72`, `needs: P0.5.10`).** The runnable first
informational pass lands after the kernel crate bodies exist (the P3.6/P3.8/P3.18/P3.29
kernel boxes): it emits a per-crate survived-mutant report, and the ratchet is a tracked
per-crate `max_survived_mutants.toml` initialised at the first-run count, **decrease-only**
(authored by P3.72 — this box registers only the gate + its posture, mirroring the P3.67
fuzz-replay activation pattern). The gate then ratchets decrease-only per crate as
subsequent phases deepen the kernel test suites.

## Bundled-engine CVE awareness — G17b (P0.7.7 · §3.4.3 §6.5 · G17b G17)

An **informational per-push** OSV/grype scan of the **PURL-keyed** `engines.lock` (the full
gate mechanics — the planted-positive, the MANDATORY FFmpeg CPE, the dated open-CVE report,
the advisory-DB-staleness floor shared with **G17** — live in the **G17b build-gate row**;
this ledger records only its **owner-decidable posture**). It honours SSOT §3.8 "engine
currency is best-effort, not a gate": the per-push leg never blocks.

**Owner decision (`informational`↔`required`).** Recorded `informational` here. The owner
flips it to `required` via the **CVSS ≥ 7 on an engine code path ConvertIA actively exercises
for a §04 format → release-blocking escalation** (the Build-Loop escalates to Co-Pilot and
blocks the next release until bumped or triaged not-exercised; the threshold is stated in
`SECURITY.md` so users know the effective turnaround, and a bump triggers the §6.5.4
re-validation-on-engine-bump). A flip edits BOTH this ledger row (status + `Since`) AND the
`plan-lint` check-23 `_OWNER_DECIDABLE_GATES` posture map in the same owner-acked L(-1) commit
(the flip protocol above). G17b + **G55** are the two halves of the offline
"audit-it-yourself" story (the dated open-CVE report + the embedded-SBOM auditable binary).

## Privilege-drop-tier ratchet — G64 (P0.7.14 · §2.12.3 · G64)

The §2.12.3 runtime containment of the untrusted C/C++ decoders is **best-effort and silently
degrades** (the T1 honest residual). **G31** proves the privilege-drop tier *fired* on the CI
runner, but nothing tracked the tier as a **TREND** — a later P-phase change dropping a
platform from a full tier to a cheap one would be an invisible NET regression. **G64** records
the **exact tier achieved per platform** into a tracked **`privilege-drop-coverage.toml`**,
**decrease-guarded** exactly like the coverage floor / `max_survived_mutants.toml`: a commit
that lowers an achieved tier **fails / escalates** (raises are deliberate committed changes).
The schema + the per-tier ratchet criteria are homed here; the `.toml` is populated as the
§2.12.3 tier matrix fills in P4–P9.

**Owner decision (`informational`↔`required`).** Recorded `informational` here — it stays so
while the §2.12.3 tier matrix is still being filled (P4–P9); the owner flips it to `required`
once the matrix **stabilises**. A flip edits BOTH this ledger row (status + `Since`) AND the
`plan-lint` check-23 `_OWNER_DECIDABLE_GATES` posture map in the same owner-acked L(-1) commit
(the flip protocol above). G64 is the **TREND/ratchet** owner; the per-platform
tier-APPLIED-per-spawn regression assertion stays a **G31** leg (P0.5.9).

## Engine-subprocess coverage-guided fuzz — G65 (P0.7.15 · §6.4.2 §6.1.4 · G42b)

The single biggest coverage asymmetry: the engine-side **T1** surface (the bundled C/C++
decoders on untrusted bytes — the literal product premise) is covered today only by a fixed
fault-injected corpus (**G26/G31**), while the lower-blast-radius in-core Rust detector gets
real coverage-guided libFuzzer (**G48**). **G65** (a **reserved-not-row** id — named in prose,
never a `· G65` header ref) adds a **black-box mutational fuzz of the real sidecar**: AFL++
binary-only/QEMU **OR** a `radamsa` harness through the §2.12 isolation wrapper (`zzuf`
LD_PRELOAD for LibreOffice headless), reusing the §6.4.2 oracles (no-crash-escapes-boundary +
no-egress + no-out-of-input-read via **G42b**). **Constraint:** the harness MUST use the
G37-staged, SHA-256-verified bundled engine binary, NOT a debug build. CI-host
resource-bounded (cgroup / `ulimit` / `docker --memory` + the G56 `timeout-minutes`) so a
corpus-induced host OOM/disk-fill is a contained finding, not a shared-VPS outage; the §6.1.4
VPS runner is the host.

**Owner decision (`informational`↔`required`).** Recorded `informational` here. It is
**pre-committed to a REQUIRED SCHEDULED, non-PR-blocking** job (at minimum a weekly
`radamsa`-through-the-isolation-wrapper run that FILES AN ISSUE on a boundary-escaping crash —
an issue-opener like G66, so it can't wedge a half-built phase); the owner decides **before
P10** whether to ALSO make it per-push, which is the `informational`→`required` flip. A flip
edits BOTH this ledger row (status + `Since`) AND the `plan-lint` check-23
`_OWNER_DECIDABLE_GATES` posture map in the same owner-acked L(-1) commit (the flip protocol
above). The reserved id `G65` is held now so an adoption does not renumber.
