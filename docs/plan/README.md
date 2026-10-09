# ConvertIA — Implementation Plan (index)

> The implementation roadmap, derived from the [Specification](../spec/README.md)
> (which is itself derived from the [Single Source of Truth](../SINGLE-SOURCE-OF-TRUTH.md)).
> **Conflict order (unchanged, every layer):**
> **SSOT > spec > security/process docs > plan > code > conversation.**
> When two layers disagree, the higher one wins — never silently reconcile.

## How this plan is used

- The autonomous Build-Loop works the plan **one phase at a time, lowest phase first,
  box by box** in document order, under [`build-loop.md`](../process/build-loop.md) (the
  runbook, the 8-point Definition of Done, the hard-stops) and
  [`roles-and-escalation.md`](../process/roles-and-escalation.md). Every box runs the
  gate system in [`build-gates.md`](../security/build-gates.md) (`G1..Gnn`) and the dual
  review. `scripts/plan-lint --next` prints the next box; the box format and its selection
  rule (§6) are [`_format.md`](_format.md). The repo's own rules are in
  [`CLAUDE.md`](../../CLAUDE.md).
- **P0 is bootstrapped manually** (DECISION B); the loop starts at P1.
- A phase is done when every box in its file is `[x]`. Every phase `P2`..`P11` ends with
  the standing `[!extern]` phase-end Co-Pilot hardening-sweep box
  ([test-strategy §11](../process/test-strategy.md#11-the-phase-end-co-pilot-hardening-sweep)):
  the Co-Pilot session, never the Build-Loop, re-tests the phase's whole delivery and
  checks the next phase's readiness, and the next phase's first box (for `P11`, the RC
  sign-off `P11.33`) `needs:` the sweep, so the loop stops at each phase boundary until
  the sweep is `[x]`.
- Each phase file's header carries the phase's scope and exit criterion; the table below
  names the phase and points at its file.
- What boxes and Co-Pilot acts leave outside their work at hand goes to the
  [residual ledger](residual-ledger.md), triaged at each phase-end sweep.
- **In scope:** the pure software plus the technical release mechanics that make
  verified downloads work. ConvertIA is offline and does not auto-update or phone home
  (SSOT Principle 4; §7.6.1). **Out of scope** (SSOT *Explicitly Out of Scope*):
  marketing, legal advice, store and developer-account logistics, code-signing and
  notarization — except where they impose an in-code requirement (the SBOM, the
  checksums, and the §6.2.3 minisign signature over the `SHA256SUMS` checksum manifest,
  which is no update-manifest signing key).

## Sequencing philosophy — walking skeleton first

After the foundation (P1–P2), P3 drives one trivial conversion end to end through the
real architecture before any heavy engine: the in-core CSV→TSV path (§3.5.6), buildable
from P1–P2 alone with no engine or sidecar, through the real `fs_guard` atomic,
no-clobber publish on all three OS. The first real sidecar follows in P4/P5, and the
format phases P5–P7 only broaden coverage on the proven harness: a vertical slice early,
then horizontal breadth.

## Phases

Spec files by number: 00-architecture, 01-conversion-pipeline, 02-guarantees,
03-engines-and-bundling, 04-formats (one file per category), 05-ui-ux,
06-build-test-release, 07-app-shell.

| Phase | Goal | File | Spec homes |
|---|---|---|---|
| P0 | The build and security system every later phase runs under, before any app code | [P0-build-and-security.md](P0-build-and-security.md) | [security-concept.md](../security/security-concept.md), [build-gates.md](../security/build-gates.md) |
| P1 | An empty window boots on Windows, macOS and Linux from a clean checkout, with toolchain and CI | [P1-foundation.md](P1-foundation.md) | 00; 05 (strings, a11y); 06 (§6.7.1, §6.8); 07 (window) |
| P2 | The app shell and the pipeline contracts, type-shared end to end, with no engine yet | [P2-app-shell-contracts.md](P2-app-shell-contracts.md) | 00; 01 (§1.1); 07 (§7.2, §7.8) |
| P3 | One conversion (in-core CSV→TSV) through the real stack on all three OS | [P3-walking-skeleton.md](P3-walking-skeleton.md) | 01 (§1.2, §1.7); 02 (§2.1–§2.7); 03 (§3.5.6); 05 |
| P4 | The engine harness: invocation, bundling, isolation, reliability machinery, generic UX | [P4-engine-framework.md](P4-engine-framework.md) | 01 (§1.7, §1.10); 02 (§2.8, §2.9, §2.12, §2.13); 03 (§3.4, §3.5.0, §3.9); 05; 06 (§6.1.3, §6.4.3, §6.5); 07 (§7.2.3, §7.2.6) |
| P5 | Every image pair `reliable` in the §6.5 ledger (libvips family) | [P5-images.md](P5-images.md) | 04 images; 03 (§3.5.5); 06 (§6.4, §6.5) |
| P6 | Every audio, video and cross-category pair `reliable` (FFmpeg family) | [P6-av-crosscat.md](P6-av-crosscat.md) | 04 audio, video, cross-category; 03 (§3.5.1); 06 (§6.5) |
| P7 | Every document, spreadsheet and presentation pair `reliable` (office family) | [P7-office.md](P7-office.md) | 04 documents, spreadsheets, presentations; 03 (§3.5.2, §3.5.3, §3.5.4, §3.5.6); 06 (§6.5) |
| P8 | The full designed experience: ship-gating UI, then non-blocking polish | [P8-ui-ux.md](P8-ui-ux.md) | 05; 02 (§2.8, §2.9); 07 (§7.6.2) |
| P9 | The non-functional contracts met and the deferred empirical items validated | [P9-hardening.md](P9-hardening.md) | 01 (§1.10); 02 (§2.10, §2.11.4); 03 (§3.9); 05 (§5.6); 06 (§6.4, §6.4.6, §6.7.3) |
| P10 | Verified downloads: the release machinery, with no auto-update | [P10-release.md](P10-release.md) | 06 (§6.2, §6.3, §6.7.2, §6.8, §6.9, §6.10); 07 (§7.6) |
| P11 | The release candidate verified end to end and signed off | [P11-acceptance.md](P11-acceptance.md) | 06 (§6.5, §6.6, §6.10); 07 (§7.2.3); SSOT (Definition of Done) |

The pre-condense index, with each phase's former scope prose: `git show 1147033:docs/plan/README.md`.
