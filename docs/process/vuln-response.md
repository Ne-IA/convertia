# ConvertIA — Vulnerability Response (CVE → user, no auto-update)

> **How a security fix reaches a user when there is no auto-update.** ConvertIA is
> fully offline and ships **no updater** (`tauri-plugin-updater` is absent by
> decision, spec §7.6.1 / CLAUDE.md §3), so the **only** channel a security fix can
> travel to a user is a **new full release** they choose to download. Every security
> fix is therefore a **release event**, and this runbook is the path from "an
> advisory exists" to "a fixed build is published". It is the **operational companion**
> to [build-loop.md](build-loop.md) (the mechanics), [roles-and-escalation.md](roles-and-escalation.md)
> (the *who-acts* org chart — the CVE→user path is one of its escalations), and
> [security-concept.md](../security/security-concept.md) (the threat model).
>
> **Status: living.** Refined *during* implementation; a change to the response path
> is recorded here first, in the same commit as the change.
> **Conflict order (every layer):**
> **SSOT > spec > security/process docs > plan > code > conversation.**
> When two layers disagree, the higher one wins — **never silently reconcile,
> always escalate**.

---

## 1. Why this runbook exists — the offline, no-auto-update premise

ConvertIA ingests **arbitrary, possibly-malicious files** and decodes them with
**bundled third-party C/C++ engines** (FFmpeg, libvips with its statically-linked
ImageMagick delegate, LibreOffice, poppler, pandoc) — a classic, high-CVE attack surface. Two facts make the response
path unusual:

- **No auto-update, no phone-home.** There is no silent patch channel and no
  telemetry that could tell us a user is exposed. A fixed engine reaches a user
  **only** when they download a new release.
- **The decoders are isolated, not trusted.** The §2.12 isolation boundary contains
  a decoder crash/hang to one failed item, so a vuln is rarely a whole-app compromise —
  but a memory-safety bug in a decoder run on a crafted file is still real, and the
  fix is still upstream-then-rebuild-then-release.

So the response is not "ship a hotfix to running installs" — it is **triage →
escalate → bump or mitigate → re-validate → cut a release**, and make the state
**auditable** so a security-conscious user can verify it themselves (the embedded
SBOM, G55, and the dated open-CVE report, G17b).

---

## 2. The engine-CVE path (the dominant case)

An advisory lands against a bundled engine. The path:

1. **Detect.** The release-tier **G17b** awareness leg (`osv-scanner`/`grype` over the
   PURL-keyed `engines.lock` + the G35 SBOM) surfaces a dated open-CVE report; an
   upstream advisory or a downstream report (§4) is the other entry point. The
   per-push G17b leg is **informational** (it honours SSOT §3.8 "engine currency is
   best-effort, not a gate") — the **release tier** is where escalation bites.
2. **Triage against the severity threshold.** A CVE with **CVSS ≥ 7 on an engine code
   path ConvertIA actively exercises for a §04 format** → the Build-Loop **escalates
   to Co-Pilot and blocks the next release** until the engine is bumped or the path is
   triaged not-exercised. This threshold is **stated in SECURITY.md** (a release-tier
   artifact) so a user knows the effective turnaround they can expect. A CVE only in a
   code path ConvertIA never exercises (a decoder excluded from the G38 allow-list, a
   muxer never invoked) is recorded as not-exercised, not a release blocker.
3. **Escalate.** The loop never decides a release-blocking security call itself — it
   raises a **Co-Pilot item** (roles-and-escalation §4/§6); a genuine ship-vs-hold
   fork goes to the **owner**.
4. **Bump.** Co-Pilot bumps the `engines.lock` pin to the fixed upstream version
   (the version/SHA edit is itself an L(-1)/`engines.lock` change with the usual
   provenance checks, G36/G37).
5. **Re-validate.** The bumped engine is re-run against the **§6.5 reliability corpus**
   before it lands — a new version can pass its hash yet regress a conversion or break
   a pair. From **P4 onward** this is enforced by **G72**, which requires a
   `Reliability-Gate: <ledger-ref>` proof that the §6.5.2 pair-status ledger was
   regenerated **green** for the bumped engine's pairs (spec §6.5.4 — re-validation on
   engine bump). During the P1–P3 window the §6.5 machinery (the §6.4.3 runner + the
   P4.61 ledger) does not exist (it is built in P4), so an `engines.lock` bump is
   **held as a Co-Pilot review item, surfaced not auto-merged** (roles-and-escalation §5a).
6. **Release.** A new full release is cut (the P10 pipeline), signed and published; the
   dated open-CVE report and the embedded SBOM let a user audit "no known CVEs" against
   a known DB age.

---

## 3. The hard sub-case — a high-severity engine vuln with no upstream fix available

This is the **dominant real-world case**: the advisory is real and serious, but no
patched upstream version exists to bump to. Waiting is not a safe default (users keep
running the vulnerable decoder on untrusted files). The **ranked options**, narrowest
first, each tied to a **SECURITY.md known-issues line** so the state is disclosed:

1. **Disable the specific decoder** via the **G38** `ffmpeg-allowed-decoders.lock`
   allow-list (or the equivalent per-engine deny for a non-FFmpeg decoder). This is the
   narrowest cut — it removes only the vulnerable code path, and G38 hard-fails any
   build that re-enables it, so the disable cannot silently regress.
2. **Drop the affected format path** — remove the conversion that reaches the
   vulnerable decoder from the supported set (a §04 format / registry edit), if the
   decoder cannot be disabled without breaking an in-scope path.
3. **Publish a documented mitigation** — a SECURITY.md known-issues line telling users
   which input class to avoid until a fix ships, when neither disable nor drop is
   feasible.
4. **Escalate the disable-vs-ship call to Co-Pilot** — when the trade-off (drop a
   user-facing capability vs. ship a known-vulnerable decoder) is a genuine fork, the
   owner calls it. The default bias is **protect the user**: a disabled format is a
   recoverable inconvenience; a shipped RCE decoder is not.

Whichever option is taken, the decision and its restore condition (the upstream
version that re-enables the path) are recorded so the cut is **reversed** once a fixed
engine exists — never left silently disabled.

---

## 4. Own-code (non-engine) vulns + coordinated disclosure

A vulnerability in ConvertIA's **own** MIT code (the Rust core, the IPC boundary, the
WebView UI) follows the same release-is-the-channel rule, through a
coordinated-disclosure loop:

- **Intake.** A reporter reaches the project through the **security contact published
  in SECURITY.md** (a private channel, not a public issue) so a vuln is not disclosed
  before a fix exists.
- **Embargo.** The report is held private while it is confirmed and fixed; the
  reporter is kept in the loop on timing.
- **Fix.** The fix is built through the normal gated loop (it is code, so it carries
  the full gate + dual-review discipline) — a security fix is **never** an excuse to
  bypass a gate.
- **Release + disclose.** The fix ships in a new release; the advisory (and credit, if
  the reporter wants it) is published with or shortly after the release, so users learn
  of the issue and the fixed version together.

---

## 5. Signing-key compromise or loss — revocation without a push channel

An offline app has **no key-revocation push channel**: there is no updater to tell an
installed copy "the old signing key is retired". So the **revocation channel is the
repository itself** — a **committed, human-readable retired-key notice** (a signed
commit + a release note + a SECURITY.md entry) that announces the old key is retired,
publishes the new public key, and tells users to trust only releases signed by the new
key going forward. A user who re-checks the repo before trusting a download sees the
retirement; the public, append-only commit history **is** the revocation record.

To keep key **loss** from becoming an unrecoverable release-signing dead end, the
signing key is held with an **offline, encrypted backup of the key + its passphrase**
(stored separately from the key material, owner-held) so a lost working copy can be
restored without minting a brand-new identity. Key custody, rotation, and the release
signing material are **owner-held** (roles-and-escalation §1) — a compromise or loss is
an unconditional escalation to the owner.

---

## 6. References

- The loop + the escalation mechanics this path rides on (push-wait, hard-stops,
  gate-quarantine, the per-box status line / Co-Pilot hand-off):
  [build-loop.md](build-loop.md) §3 / §6 / §8.
- Who acts at each rung (Build-Loop → Co-Pilot → owner; the incoming-PR / Dependabot
  `engines.lock`-bump ownership): [roles-and-escalation.md](roles-and-escalation.md)
  §1 / §4 / §5a / §6.
- The threat model + the isolation boundary + the no-egress posture:
  [security-concept.md](../security/security-concept.md).
- The gate catalogue — **G17b** (bundled-engine CVE awareness + the release-tier CVSS ≥ 7
  escalation), **G72** (`engines.lock`-bump corpus re-validation), **G38** (the FFmpeg
  enabled-decoder allow-list), **G35/G36/G37/G55** (SBOM + engine provenance + auditable
  binary): [build-gates.md](../security/build-gates.md).
- The spec — engine maintenance & versioning (§3.8), the reliability gate and
  re-validation-on-bump (§6.5 / §6.5.4), the no-updater decision (§7.6.1):
  [docs/spec/README.md](../spec/README.md).
- Plan home (the box that authored this runbook): [`docs/plan/P0-build-and-security.md`](../plan/P0-build-and-security.md) §P0.6.
- SSOT (what & why; the no-telemetry / no-egress principle): [SINGLE-SOURCE-OF-TRUTH.md](../SINGLE-SOURCE-OF-TRUTH.md).
