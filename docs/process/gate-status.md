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

The deterministic gates — **every `Gnn` except G1** — are always-on and are **not**
tracked here. This ledger tracks only the gates whose posture is an owner/ratchet
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
- **P0.7.15** — **G65** (engine-subprocess coverage-guided fuzz), appended when authored.
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
| `cargo-mutants` | informational | 2026-06-19 | P3 (P3.72) | scoped mutation testing over `crate::fs_guard`+`crate::detect`+`crate::outcome` (the no-harm/atomicity/no-misroute kernel), a **G15** sub-leg — line coverage proves a line RAN, not that a test would CATCH a regression there; owner flips `informational`→`required` once survived-mutants reach **0** for `crate::fs_guard`+`crate::detect` (the P3.72 first run + the decrease-only per-crate `max_survived_mutants.toml` ratchet) |
| **G59** — build-provenance attestation (`actions/attest-build-provenance`) | decided | 2026-06-19 | P10 | a v1 OWNER DECISION (promoted from a post-v1 deferral): the one genuinely-free build-**ORIGIN** signal — binds the artifact to runner+workflow+commit, so a silently re-signed release from a poisoned shared VPS is detectable **even if the minisign key leaked**; additive to minisign, **NOT** binary code-signing; needs only `id-token: write` scoped to the release/attestation job. **VERIFIED, not just generated** — a release step runs `gh attestation verify` (fail-on-non-zero); the **Sigstore bundle + a paired `trusted_root.jsonl`** ship as named release assets for OFFLINE verify; both join the **G58** completeness enumeration. `decided` = a one-time adopt → NOT in the check-23 `_OWNER_DECIDABLE_GATES` posture map; §8/catalogue/box statuses agree (check 17: the §8 entry is PROMOTED, not a live deferral) |
| **G17b** — bundled-engine CVE awareness (`osv-scanner`/`grype`) | informational | 2026-06-19 | P10 | informational per-push OSV/grype over the **PURL-keyed** `engines.lock` (a planted-positive — a known historical internal-FFmpeg-decoder CVE — guards the empty-report-masquerading-as-clean failure; the FFmpeg CPE `cpe:2.3:a:ffmpeg:ffmpeg:<ver>` is MANDATORY); emits a dated open-CVE report (recording the advisory-DB age) as an owner-signed-off release asset; offline-tolerant (vendored DB, refresh warn-only). Owner flips `informational`→`required` via the **CVSS ≥ 7 on an actively-exercised §04 path → release-blocking escalation** (recorded in `vuln-response.md` / `SECURITY.md`); the release-tier advisory-DB-staleness floor (`MAX_ADVISORY_DB_STALENESS`) is shared with **G17**. A flip edits BOTH this row AND the check-23 `_OWNER_DECIDABLE_GATES` map in the same owner-acked L(-1) commit |

None of the four over-assurance backstops replaces **G48**'s fuzz; each is an
**additive** proof/observation layer on top of the deterministic gates, which the
owner may adopt. `cargo-mutants` (the fifth row, P0.5.10) is likewise additive to
G48 — fuzzing finds crashes on hostile input; mutation testing finds assertions the
tests forgot to make. It is detailed in its own section below.

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
`crate::fs_guard` + `crate::detect` + `crate::outcome` (the no-harm / atomicity /
no-misroute kernel). Line coverage proves a line **executed**; it does **not** prove a
test would CATCH a regression there — `cargo-mutants` mutates the kernel's code and
**fails if a mutation survives the test suite** (a gap a coverage percentage hides). It
is **additive to G48**, not a replacement: G48's fuzz finds crashes on hostile input;
mutation testing finds the assertion a test forgot to make on benign input.

**Owner decision (required-vs-informational, like G17b).** Recorded `informational`
here; the owner flips it to `required` once the survived-mutant count reaches **0** for
`crate::fs_guard` + `crate::detect` (the two kernels whose silent regression breaks the
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
