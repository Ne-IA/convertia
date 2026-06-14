<div align="center">

# ConvertIA

**A portable, install-free desktop app to convert common everyday files into
other sensible formats — drag them onto one drop area, pick a target, done.**

Cross-platform (Windows · macOS · Linux) · fully local & private · MIT-licensed.

</div>

> **Status: Planning.** This repository currently holds the project's design
> documents. Implementation follows once the technical specification is complete.

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

## Documentation

The docs form a single layered system. The **conflict order** (higher wins) is
**SSOT > spec > security/process docs > plan > code > conversation** — when two
layers disagree, the higher one wins and the lower is corrected, never silently
reconciled.

| Doc | Purpose |
|-----|---------|
| [SINGLE-SOURCE-OF-TRUTH.md](docs/SINGLE-SOURCE-OF-TRUTH.md) | The idea, rules and scope — **what & why** (authoritative). |
| [spec/](docs/spec/README.md) | The technical specification — **how the app works** (living). |
| [security/](docs/security/security-concept.md) | The build-safety concept — threat model, defense-in-depth, and the gate catalogue (`G1..Gnn`): **how we build it safely** (living). |
| [process/](docs/process/build-loop.md) | The build process — the autonomous build-loop runbook, roles & escalation, and the test strategy (living). |
| [plan/](docs/plan/README.md) | The implementation roadmap — phased executable TODO (P0 bootstrap + P1–P11). |
| [CLAUDE.md](CLAUDE.md) | The repo's own project rules for Claude Code (conflict rule, DoD summary, anti-patterns). |

## License

[MIT](LICENSE) © Ne-IA and ConvertIA contributors. Bundled third-party
conversion engines keep their own licenses (see the NOTICE / third-party-licenses
shipped with each release).
