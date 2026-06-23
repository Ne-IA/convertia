<div align="center">

# ConvertIA

**A portable, install-free desktop app to convert common everyday files into
other sensible formats — drag them onto one drop area, pick a target, done.**

Cross-platform (Windows · macOS · Linux) · fully local & private · MIT-licensed.

</div>

> **Status: pre-release.** ConvertIA is still being built, so there is **no public
> release yet**. This page already describes how you will download, verify, and run
> it, so the trust model is in place from day one. Until the first release is
> published, the repository holds the project's design documents and code.

## What it is

ConvertIA is a small, friendly file converter for everyday people — no sketchy
online uploaders, no accounts, no installation. Drop a file (or a folder of the
same type), choose what to turn it into, and convert. It speaks images, audio,
video, documents, spreadsheets and presentations.

### Principles

- **Portable, no installation** — download, run, done.
- **Local, private & offline** — your files never leave your machine; no
  accounts, no telemetry; everything is bundled and runs without a network.
- **Never harms the original** — sources are never overwritten or deleted.
- **It just works** — sensible defaults, clear errors, for anyone (not just
  specialists).

## Download

ConvertIA is distributed **only** through the project's **canonical GitHub Releases
page** — that is the single official source. Do not trust a ConvertIA build obtained
from anywhere else.

Each release provides one artifact per platform:

- **Windows** — a portable `.zip` (x64). Unzip and run; there is no installer.
- **macOS** — a `.dmg` (universal: Intel + Apple Silicon).
- **Linux** — a 64-bit `.AppImage`.

## Verify your download

Every release publishes a `SHA256SUMS` file and a minisign signature next to the
artifacts. Verifying takes a few seconds and confirms the file is the official,
untampered build.

**1. Check the hash** — compare the result to the value published with the release:

- **Windows (PowerShell):** `Get-FileHash .\ConvertIA-<version>-x64.zip -Algorithm SHA256`
- **macOS:** `shasum -a 256 ConvertIA-<version>.dmg`
- **Linux:** `sha256sum ConvertIA-<version>.AppImage`

…or check every artifact at once against the published list: `sha256sum -c SHA256SUMS`.

**2. Verify the signature** with [minisign](https://jedisct1.github.io/minisign/),
using the project's public key:

```sh
minisign -Vm SHA256SUMS -p docs/minisign.pub
```

The exact published hashes and the signing key are provided with the first release.

## Supported systems

- **Windows 10 (version 1809 / build 17763) or Windows 11**, 64-bit, with the
  **Microsoft Edge WebView2** runtime present (see Prerequisites).
- **macOS 11 (Big Sur) or later.**
- **Linux:** a 64-bit glibc desktop with **WebKitGTK 4.1** (Ubuntu 22.04 LTS or
  newer, current Fedora, and equivalents).
- **Memory:** 2 GB minimum, 4 GB recommended.

## Running it the first time

Because the v1 builds are **not code-signed**, your operating system may warn you on
the first launch. This is expected for an open-source portable app; here is how to
proceed.

- **Windows:** if SmartScreen shows "Windows protected your PC", click **More info →
  Run anyway**.
- **macOS:** if you see "ConvertIA can't be opened", open **System Settings → Privacy
  & Security**, scroll to the blocked-app notice, click **Open Anyway**, then confirm
  **Open** on the dialog that follows, and re-launch. On **macOS Sequoia (15.x)** the
  older Control-click → "Open" shortcut no longer works, so this Privacy & Security path
  is the way in. The first time you run a conversion, macOS may show the same prompt once
  per bundled tool — use the same **Open Anyway** step.

### Prerequisites

- **Windows — WebView2:** ConvertIA uses Microsoft Edge WebView2, which is built into
  Windows 11 and current Windows 10. If ConvertIA's window flashes and closes
  immediately, install the **WebView2 Runtime** (or update Windows / Edge) and try
  again.
- **Linux — libfuse2:** the AppImage mounts itself with FUSE 2 at launch. If it will
  not start, install `libfuse2` (Ubuntu: `sudo apt install libfuse2`, or `libfuse2t64`
  on 24.04 and newer), or run it with `--appimage-extract-and-run`.

## As-is, no warranty

ConvertIA is free and open source, provided **as is, without warranty of any kind**
(see the [LICENSE](LICENSE)). Security is handled on a **best-effort** basis (see
[SECURITY.md](SECURITY.md)); your privacy is covered in [PRIVACY.md](PRIVACY.md).

## Documentation

The docs form a single layered system. The **conflict order** (higher wins) is
**SSOT > spec > security/process docs > plan > code > conversation** — when two
layers disagree, the higher one wins and the lower is corrected, never silently
reconciled.

| Doc | Purpose |
| --- | --- |
| [SINGLE-SOURCE-OF-TRUTH.md](docs/SINGLE-SOURCE-OF-TRUTH.md) | The idea, rules and scope — **what & why** (authoritative). |
| [spec/](docs/spec/README.md) | The technical specification — **how the app works** (living). |
| [security/](docs/security/security-concept.md) | The build-safety concept — threat model, defense-in-depth, and the gate catalogue (`G1..Gnn`): **how we build it safely** (living). |
| [process/](docs/process/build-loop.md) | The build process — the autonomous build-loop runbook, roles & escalation, and the test strategy (living). |
| [plan/](docs/plan/README.md) | The implementation roadmap — phased executable TODO (P0 bootstrap + P1–P11). |
| [CLAUDE.md](CLAUDE.md) | The repo's own project rules for Claude Code (conflict rule, DoD summary, anti-patterns). |

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) and the
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Use of the "ConvertIA" name and the Ne-IA
logo is covered by [TRADEMARK.md](TRADEMARK.md).

## License

[MIT](LICENSE) © Ne-IA and ConvertIA contributors. Bundled third-party conversion
engines keep their own licenses (see the NOTICE / third-party-licenses shipped with
each release).
