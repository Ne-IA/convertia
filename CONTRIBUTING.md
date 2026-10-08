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
  commit and push (and are mirrored in CI) once `python3 -P scripts/setup-dev` has installed the git
  hooks after cloning; setup-dev is their one installer. A red gate is **fixed, never bypassed** —
  `--no-verify` and force-pushes to the default branch are not used.

Per-OS development prerequisites (toolchains, the platform WebView runtime, system build
dependencies), the `tauri dev` / `tauri build` commands, and how to obtain the bundled engine
binaries for a local run are documented in [DEVELOPMENT.md](DEVELOPMENT.md).

## Known gate traps

A gate that fires for a reason other than a real defect costs a contributor, human or agent, a round
trip. One line per trap: the trigger, the gate, the fix. An entry here is the closure the root-cause rule
([CLAUDE.md](CLAUDE.md) §10) accepts for a gate-trap class that guards no security control, has not
recurred and never turned `main` red; a class that does gets a mechanical catcher instead.

- **An edit to the G1 rubric in `docs/process/build-loop.md` alone** — `run-gate-selftests --changed`
  (L2 pre-push) does not scope `docs/process/`, so the check-19 pin-uniqueness leg of
  `g24-plan-lint.py` first runs at L4 — run `python3 -P scripts/run-gate-selftests` locally before
  pushing.
- **A new doc linked only inside backticks** (`` `[x](y)` ``) — `plan-lint` check 25 strips inline code
  before reading links, so the doc is an orphan — link it with a plain markdown link.
- **`lefthook run <hook>` on a clean tree** — lefthook skips every command and exits 0 — add `--force`
  (`.gate-tools/bin/lefthook run pre-push --force`); nothing is committed or pushed, so every leg that
  reads the unpushed range sees no commit.
- **`lefthook install` run by hand** — lefthook writes its own path unquoted into each hook, so with
  the pinned `.gate-tools/bin/lefthook` on a repo path with spaces or parentheses every hook fails to
  parse and `git commit` aborts — install with `python3 -P scripts/setup-dev`, which quotes the path
  and parse-checks each hook; no lefthook run re-installs a hook (`lefthook.yml` sets
  `no_auto_install: true`, G54b).
- **A marker in production code, comments included** — `check-deferral` (G8 staged, G21 tree) reads
  `src/` and `src-tauri/src/` minus test paths, in-file `#[cfg(test)]` modules included: `TODO` and the
  marker macros (`todo!`, `unreachable!`, `println!`, …) match anywhere, strings included, the deferral
  words (`later`, `for now`, `not yet`, …) in comments — describe the effect instead. A
  `[Build-Session-Entscheidung: <box-id>]` within 6 lines suppresses; it marks a real choice only.
- **An assertion token in an added comment line of a `.rs` or TS test file** — `check-test-suppression
  --diff` (G70, L1) reads `expect(`, `assert(`, an `assert!`-family macro or a `.toBe`-family matcher
  there as a commented-out assertion, backticks or not — describe the check's effect instead.
- **A removed assertion or `#[expect(…)]` lint attribute** — G70 `--diff` needs a
  `[Test-Change: <box-id> — old-obsolete+new-correct, <§>]` tag within 6 diff lines of the deletion
  run, and the pre-push `--full` leg checks markers only — add the tag and run
  `python3 -P scripts/check-test-suppression --diff` on the staged tree.
- **A `// SAFETY:` comment of three or more lines above `// nosemgrep:`** — `check-unsafe-policy` (G29)
  wants `SAFETY:` on the `unsafe` line or the 3 lines above, and `// nosemgrep:` sits directly above
  the `unsafe` — keep the SAFETY text to 2 lines.
- **A Semgrep false positive** — `check-sast` (G29) honours `// nosemgrep: <rule-id>` on the finding's
  first line or the line above it — write the rule id alone there, the reason and the decision tag on
  the lines above; the rule corpus `scripts/semgrep-rules/` is caged, never the fix.
- **A double-quoted `"app://…"` under `src-tauri/src`, comments included** — `plan-lint` check 28
  reads it as an event literal, allowed only as one of the three event names in
  `src-tauri/src/ipc/mod.rs` — use backticks.
- **A new item inserted between a comment block and the item it documents** (an edit anchored on
  `#[cfg(test)]` or `mod x {`) — `check-doc-splice` (G74) fails: a `///` block re-attaches to the
  insertion across blank and attribute lines — insert above the comment block or below the item.
- **A renamed `…_contract` test still cited in a comment under `src-tauri/src/ipc/`** —
  `check-rs-test-refs` (G73, pre-push) fails on a cited name that is no `mod` or `fn` under `ipc/` —
  update the comment with the rename; prefer citing the test module to a `#[test] fn` name.
- **A `Dual-Review:` line with anything after the verdicts** — `check-dual-review` (G12, pre-push)
  needs a line reading exactly `Dual-Review: opus=GO sonnet=GO` (each `GO` or `NOGO`), and a GO/GO
  line needs the review record in the body — put round notes in the body; run
  `python3 -P scripts/check-dual-review` before pushing.
- **A `///` comment on a specta-exported item** — it lands in `src/lib/ipc/bindings.ts`, which
  `check-generated-drift` (G19, pre-push) regenerates and diffs — run `cargo run -p xtask -- codegen`
  and commit the regenerated file with the change.
- **`#[cfg(all(test, unix))]` on a test module** — clippy's `allow-expect-in-tests` misses the
  compound form, so an `expect` inside it reds G4/G14 — stack `#[cfg(test)]`, then `#[cfg(unix)]`.
- **`#[expect(dead_code)]` on an item that gains a caller** — the expectation turns unfulfilled, an
  error under `-D warnings` (G4/G14) — remove it, or turn it into `allow` in place where another
  target or build still leaves the item dead (always `allow` for an item only another target calls);
  the removed `expect(` line needs the G70 tag.
- **A source-scan test** (a `.rs` file read through `include_str!`) — a needle that also occurs in a
  comment, a string or another file passes on the wrong site — strip comments and strings before
  cutting at `#[cfg(test)]`, and use a needle that occurs once in the scanned file.

## How to contribute

External contributions come as **GitHub pull requests against `main`**. Keep the change focused,
make CI green, and a maintainer reviews it and lands it on `main` as a commit that keeps you as the git
author (`git commit --author`) and keeps your `Signed-off-by`, with the maintainer as committer — every
commit on `main` must pass the repo's review gates (a dual-review trailer; owner-ack on protected files),
which a fork commit cannot carry as pushed. Requests for **new file formats** default
to **Future Ideas (Parked)** per the project's inclusion test — please open an issue to discuss
before sending a PR that adds a format.

## Conduct and security

This project follows a Contributor-Covenant-style Code of Conduct (`CODE_OF_CONDUCT.md`). Please
report security vulnerabilities **privately** through GitHub's private security advisories rather
than a public issue; the disclosure process is described in `SECURITY.md`.
