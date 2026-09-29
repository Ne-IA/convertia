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
- **CPython from python.org as `python3`** — the git hooks (installed by
  `python3 -P scripts/setup-dev`) run every repo gate as `python3 -P scripts/…`, and CI uses
  Python 3.12. Give the python.org install a `python3.exe` (a copy of its `python.exe`) and keep
  its directory ahead of `%LOCALAPPDATA%\Microsoft\WindowsApps` on `PATH`, or turn off the
  `python.exe` / `python3.exe` App Execution Aliases. A `python3` that resolves to the Microsoft
  Store Python can make a vitest suite of the TypeScript gate fail intermittently to resolve an
  import: every process it starts carries Windows' RedirectionGuard mitigation, under which Vite
  can miss a package import through pnpm's `node_modules` junctions. `scripts/setup-dev` refuses
  it.

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

## Windows host notes

- **Python.** `python3` is the python.org CPython 3.12 of the [Windows prerequisites](#windows), never
  the Microsoft Store Python. Under a Store `python3`, a random vitest suite of `check-ts-gate` can
  fail with `Failed to resolve import`; the prerequisite gives the cause and the `PATH` fix.
  `setup-dev` refuses a Store `python3` only when it runs, so after a `PATH` change check that
  `python3 -c "import sys; print(sys.executable)"` prints a path outside `WindowsApps`.
- **Setup.** Run `python3 -P scripts/setup-dev` after cloning: it installs the pinned gate tools into
  `.gate-tools/bin` and the lefthook hooks, and quotes the path lefthook writes unquoted into each hook,
  which breaks the hooks on a path with spaces or parentheses. Re-run it after every `lefthook.yml`
  change, your own or a pulled one: lefthook re-syncs its hooks on its next run and writes the unquoted
  path back, so the next commit fails with a hook `sh` syntax error.
- **Line endings.** Every text file in the repo is LF (`.gitattributes`, G52). A Python script that
  writes a repo file opens it with `newline="\n"` or in binary mode; text mode writes CRLF on Windows.
- **Signed commits.** Commit in the foreground: a commit that hangs with no output is waiting on the
  signing step. `setup-dev` points `gpg.ssh.program` at the Windows OpenSSH `ssh-keygen`, which reaches
  the SSH agent.
- **Push status.** Read `git push`'s own exit status (`git push origin main; echo "exit $?"`), never
  through a pipe or `tee`, which reports its last command's status. A pre-push red ends in
  `failed to push some refs`, which reads like a remote rejection.
- **Linux-only code.** Code under `cfg(unix)` or `cfg(target_os = "linux")` runs in the local
  `convertia-linux` image (Git Bash; `<repo>` is the repo path):

  ```sh
  docker build -t convertia-linux - <<'EOF'
  FROM rust:1.96-bookworm
  RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends libwebkit2gtk-4.1-dev \
      libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev libssl-dev libglib2.0-dev \
      libsoup-3.0-dev libjavascriptcoregtk-4.1-dev pkg-config build-essential file \
      && rm -rf /var/lib/apt/lists/*
  EOF
  MSYS_NO_PATHCONV=1 docker run --rm --security-opt seccomp=unconfined -v "<repo>:/work" -w /work \
    -v convertia_rustup:/usr/local/rustup -v convertia_cargo_registry:/usr/local/cargo/registry \
    -v convertia_target:/build-target -e CARGO_TARGET_DIR=/build-target convertia-linux \
    sh -c 'cargo test -p convertia-core --lib --locked &&
      cargo clippy -p convertia-core --all-targets --locked -- -D warnings'
  ```

  `seccomp=unconfined` lets the Landlock and user-namespace legs run instead of degrading. The image's
  Rust is the newest 1.96.x, not the `rust-toolchain.toml` pin, so rustup installs the pinned toolchain
  on the first run; the `convertia_rustup` volume keeps it for the next runs.
- **G29 SAST.** Semgrep runs natively on Windows, but a `semgrep` on `PATH` is not the CI pin and
  `check-sast` runs whichever it finds without a version check, so a native green is advisory.
  `install-gate-tools` does not install Semgrep, and the pinned set in `requirements-ci.txt` does not
  install on Windows (a Windows-only transitive dependency has no hash). The pinned run is the
  `python:3.12` container over a git clone with the staged diff applied (`paths.include` resolves
  against the git root):

  ```sh
  MSYS_NO_PATHCONV=1 docker run --rm -v "<repo>:/src:ro" python:3.12 bash -c 'set -e
    git config --global --add safe.directory "*"; git clone -q /src /work
    git -C /src diff --cached --binary | git -C /work apply --index --allow-empty; cd /work
    python3 -P -m pip install -q --require-hashes -r requirements-ci.txt
    python3 -P scripts/install-gate-tools --tool shellcheck; python3 -P scripts/check-sast --full'
  ```
- **Tauri lib tests.** A crate that links Tauri and has a lib test target needs `src-tauri/build.rs`'s
  Common-Controls v6 manifest pattern; without it the test binary exits with `0xc0000139`
  (`STATUS_ENTRYPOINT_NOT_FOUND`) before any test runs.
- **git and gh.** A push to `main` also starts `scorecard`, so select CI runs with
  `gh run list --workflow ci`. `git log -S` stops at a rename; use `git log --follow -S <text> -- <path>`.

## Bundled conversion engines

ConvertIA's conversions are powered by **bundled third-party engine binaries** (FFmpeg,
libvips, LibreOffice, poppler, pandoc, plus a native Rust CSV/TSV engine) that ship inside
the app and run as isolated subprocesses. They are integrated in a later phase of the build;
the foundation builds are the app shell and run without them, so an early `pnpm tauri dev`
needs no engine download.

When the engines are in play they are **not** committed to the repository — they are large
and carry their own licenses. Instead each engine is pinned by URL and SHA-256 in
**`engines.lock`**, fetched and checksum-verified at build time into a per-engine asset cache,
and placed into the bundle by the staging script `scripts/stage-engines`, which reads only that
verified cache and never the network — the same pinned set CI stages, so a local build matches
what ships. This keeps the repository small and the engine provenance auditable, and it does not
weaken the shipped app's offline guarantee: the fetch happens only at **build** time, never
when a user runs ConvertIA.
