# Developing ConvertIA

This guide covers setting up a local development environment for ConvertIA — a Tauri v2
desktop app with a Rust core and a React 19 / TypeScript / Vite WebView UI. For the
contribution workflow, the quality bar, and how to run the checks, see
[CONTRIBUTING.md](CONTRIBUTING.md).

## Toolchains

- **Rust** — install it with [rustup](https://rustup.rs/). The exact toolchain is pinned in
  [`rust-toolchain.toml`](rust-toolchain.toml) (currently stable **1.96.0** with the
  `rustfmt`, `clippy`, and `llvm-tools-preview` components); rustup reads that file and
  installs the right toolchain automatically the first time you build, so you never pick a
  channel by hand.
- **Node.js + pnpm** — ConvertIA uses **pnpm**, pinned to `pnpm@10.13.1` through the
  `packageManager` field. The simplest way to get the pinned pnpm is Corepack, which ships
  with Node: run `corepack enable`, then `pnpm install` in the repo root. Use a current
  Node.js LTS.

## Per-OS system prerequisites

Tauri renders the UI in the OS-provided WebView and links against the platform's native GUI
libraries, so each OS needs a few system packages.

### Windows

- **Microsoft Edge WebView2 runtime** — built into Windows 11 and current Windows 10; if it
  is missing, install the Evergreen WebView2 Runtime from Microsoft.
- The **MSVC build tools** (the "Desktop development with C++" workload, including the
  Windows SDK), which rustup's `x86_64-pc-windows-msvc` toolchain links against.

### macOS

- **Xcode Command Line Tools** — `xcode-select --install`. macOS provides the WKWebView
  runtime, so no separate WebView install is needed (macOS 11 Big Sur or later).

### Linux

The Tauri / WebKitGTK build dependencies (Debian / Ubuntu package names):

```sh
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev \
  libdbus-1-dev \
  build-essential curl pkg-config
```

(`build-essential`, `curl`, and `pkg-config` are general build basics — CI's runner image
already provides them; they are listed here for a clean machine.)

Producing a release **AppImage** with `tauri build` additionally needs `librsvg2-dev` and
`patchelf` (to build) plus `libfuse2` (to run the resulting AppImage); see the packaging
spec (§6.1.4) for the authoritative runtime / bundle dependency list.

## Running the app

From the repo root, after `pnpm install`:

- **`pnpm tauri dev`** — builds the Rust core and the Vite UI and launches the app with
  WebView hot-reload. This is the normal development loop.
- **`pnpm tauri build`** — produces the optimized, per-platform artifact (the portable
  `.zip` on Windows, the `.dmg` on macOS, the `.AppImage` on Linux).

The individual check commands (type-check, lint, tests) are listed in
[CONTRIBUTING.md](CONTRIBUTING.md).

## Bundled conversion engines

ConvertIA's conversions are powered by **bundled third-party engine binaries** (FFmpeg,
libvips, LibreOffice, poppler, pandoc, plus a native Rust CSV/TSV engine) that ship inside
the app and run as isolated subprocesses. They are integrated in a later phase of the build;
the foundation builds are the app shell and run without them, so an early `pnpm tauri dev`
needs no engine download.

When the engines are in play they are **not** committed to the repository — they are large
and carry their own licenses. Instead each engine is fetched from a **pinned URL recorded in
`engines.lock`** and verified against its pinned checksum by the staging script
(`scripts/stage-engines`, added alongside the engine integration) — the same pinned set CI
stages, so a local build matches what ships. This keeps the repository small and the engine provenance auditable, and it does not
weaken the shipped app's offline guarantee: the fetch happens only at **build** time, never
when a user runs ConvertIA.
