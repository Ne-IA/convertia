# Security Policy

ConvertIA is a fully offline, install-free desktop file converter — it converts files entirely on
your machine, with **no telemetry and no auto-update** (see `PRIVACY.md` for the full offline /
no-network statement). Because of that, this policy works differently from a server or
auto-updating app — please read the **No auto-update** section below.

## Reporting a vulnerability

**Please report security vulnerabilities privately — not in a public issue.**

- Use **GitHub's private vulnerability reporting** for this repository: the **Security** tab →
  **"Report a vulnerability"** (a private GitHub security advisory). This reaches the maintainers
  confidentially.
- If you cannot use that channel, contact a maintainer privately through GitHub.

We follow **coordinated disclosure**: your report is kept private while we confirm and fix the
issue, you are kept informed on timing, and the advisory (with credit, if you want it) is published
with or shortly after the fixed release — so users learn of the issue and the fix together. Please
give us a reasonable chance to ship a fix before any public disclosure.

### What to include

- A clear description and the **steps to reproduce**.
- The affected version (the release tag) and your operating system.
- A **redacted** sample or log if relevant. By default, ConvertIA's logs record only structural
  facts plus an output basename — never file contents or full paths (spec §7.5.3) — so a
  default-level log can usually be shared safely. Note that **verbose / diagnostic mode** (which
  you may turn on to capture a reproduction) _additionally_ records full file paths and the exact
  engine command line (§7.5.4), so double-check a verbose log before attaching it, and never
  include a file you would not want seen.

## Scope and threat model

ConvertIA's whole job is to open **arbitrary, possibly-malicious files** and convert them, using
**bundled third-party decoders** (FFmpeg, libvips with its statically-linked ImageMagick delegate,
LibreOffice, poppler, pandoc) alongside a native Rust CSV/TSV engine. Third-party media and document decoders are a classic, high-CVE attack
surface, so this is treated as a primary security concern:

- **Untrusted bytes are decoded only in isolated subprocesses, never in the core.** A decoder
  crash, hang, or memory-safety bug on a crafted file is contained to that one failed conversion by
  the process-isolation boundary (spec §2.12); it does not compromise the rest of the app or your
  other files. The full threat model is the architecture specification's threat map (§0.11).
- ConvertIA is **MIT-licensed open source**; the bundled copyleft engines ship as separate,
  independently-invoked binaries and are not linked into the core.

In scope: memory-safety or sandbox-escape issues in how ConvertIA invokes a decoder, in the Rust
core, in the typed IPC boundary, or in the WebView UI. A crash in a bundled upstream decoder that
stays contained to a single failed item is still worth reporting — we may disable the affected
decoder path or bump the engine — but is generally lower severity.

## No auto-update: how a fix reaches you

ConvertIA ships **no updater** and never phones home. The **only** way a security fix reaches you is
a **new full release that you choose to download.** So:

- **The supported version is the latest release.** Older releases do not receive back-ported fixes
  — a security fix is always a new release.
- After a security release, please **download the new build** to get the fix.
- Each release is **signed**, and the build embeds a **software bill of materials (SBOM)**, so you
  (or a tool) can verify a download and audit its bundled engine versions against known CVEs. If a
  signing key is ever retired, the repository itself carries a signed retired-key notice that names
  the replacement key.

## Response posture (best-effort, no SLA)

ConvertIA is free and open source, maintained on a **best-effort basis**: there is **no guaranteed
response or fix time (no SLA).** We do prioritise by severity — a vulnerability with **CVSS ≥ 7 on
an engine code path ConvertIA actively exercises** for a supported conversion blocks the next
release until the engine is bumped or the path is mitigated (the decoder disabled, the format
dropped, or a documented work-around published). Known security issues and any interim mitigations
are published in the **Known issues** section below as they arise.

## Known issues

There are currently no published security advisories or known unpatched vulnerabilities for
ConvertIA.
