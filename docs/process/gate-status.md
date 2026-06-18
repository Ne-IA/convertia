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
decision (`informational` ↔ `required`), plus the purely-informational census tools.
Each box that introduces such a gate appends its row in the **same commit**:

- **P0.4.5** (this file's creator) — the four over-assurance behavioural backstops below.
- **P0.5.10** — `cargo-mutants` (scoped mutation testing), appended when authored.
- **P0.7.14** — **G64** (privilege-drop-tier ratchet) + the formal flip protocol.
- **P0.7.15** — **G65** (engine-subprocess coverage-guided fuzz), appended when authored.
- **G17b** — appended by its box.

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

None of the four replaces **G48**'s fuzz; each is an **additive** proof/observation
layer on top of the deterministic gates, which the owner may adopt.

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
