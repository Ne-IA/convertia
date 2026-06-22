# Contributing to ConvertIA

ConvertIA is a portable, offline, install-free desktop file converter — drop a file in one
area, get it back in another sensible everyday format — and Ne-IA's first fully-open product.
Contributions are welcome.

## License: inbound = outbound

ConvertIA is **MIT licensed** (see [`LICENSE`](LICENSE)). Contributions are accepted
**inbound = outbound**: by submitting a contribution you agree it is licensed under the same
MIT license as the project. There is **no Contributor License Agreement (CLA)** and **no
copyright assignment** — you keep your copyright. The collective notice is
`Copyright (c) 2026 Ne-IA and ConvertIA contributors`.

### Developer Certificate of Origin (optional)

A `Signed-off-by` trailer — the [Developer Certificate of Origin](https://developercertificate.org/),
added with `git commit -s` — is **requested but not required**. It is a lightweight statement
that you have the right to submit the work under the project's license.

### Inbound-warranty

By contributing you **warrant** that your submission is **your own work** or is otherwise
**compatibly licensed** for inbound MIT. **Incompatibly-licensed code is not accepted.** In
particular, ConvertIA's own code is MIT and stays free of copyleft (GPL/LGPL/AGPL)
contamination; the bundled third-party conversion engines are separate, independently-invoked
binaries under their own licenses and are not mixed into the MIT core.

## Quality bar

Every change must be **production-ready**. The bar is stated here directly:

- **No `any`** in TypeScript (`: any` / `as any`) — the IPC boundary is fully typed end to end.
- **No `// TODO` / `FIXME`** (or other deferral markers) in committed code — build it fully, or
  open an issue to track the work.
- **No `console.log`** (or `println!` / `dbg!`) in production code.
- **No inline CSS** in hand-authored components — styling goes through the design tokens / Tailwind.
- Every change is complete, **tested at the highest sensible level**, and passes all checks.

## Running the checks

The same checks run **locally** (git hooks, via lefthook) on every commit and push, and again in
**CI** on every pull request. To run them yourself:

- **Frontend (TypeScript / React):** `pnpm install`, then `pnpm typecheck`, `pnpm lint`,
  `pnpm lint:css`, `pnpm format:check`, and `pnpm test`.
- **Rust core:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`.
- **Repo gates:** the pinned, standard-library gate scripts under `scripts/` run automatically on
  commit and push (and are mirrored in CI). A red gate is **fixed, never bypassed** — `--no-verify`
  and force-pushes to the default branch are not used.

Per-OS development prerequisites (toolchains, the platform WebView runtime, system build
dependencies) and how to obtain the bundled engine binaries for a local run are documented in the
project's development setup notes.

## How to contribute

External contributions come as **GitHub pull requests against `main`**. Keep the change focused,
make CI green, and a maintainer will review and merge. Requests for **new file formats** default
to **Future Ideas (Parked)** per the project's inclusion test — please open an issue to discuss
before sending a PR that adds a format.

## Conduct and security

This project follows a Contributor-Covenant-style Code of Conduct (`CODE_OF_CONDUCT.md`). Please
report security vulnerabilities **privately** through GitHub's private security advisories rather
than a public issue; the disclosure process is described in `SECURITY.md`.
