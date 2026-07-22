#!/usr/bin/env python3
"""g24-lockfile-integrity.py - G24 self-test for check-lockfile-integrity (P0.4.9, G18a).

Proves the structural freeze (A) CANNOT be relaxed (imports.lock dropped from the drift-set / a
no-flag cargo command excluded / a lock-mutating or no-`--locked` subcommand leaking into the
pin-set / fuzz/Cargo.lock dropped from the sub-workspace lock-sync set or the drift paths), the
CI-workflow flag-scan (B) CATCHES an unpinned `cargo build`/`pnpm install` and PASSES a pinned one
(while NOT mistaking a `cargo audit`/`cargo deny`/`cargo vet`/step-`name:` mention for a pinned
build), the live drift-guard (C) is a no-op when no lockfile exists, clean on rc=0, and fail-CLOSED
on a drift / a git error / git-unavailable, and the sub-workspace lock-sync (D) CATCHES a committed
sub-lock resolving a shared crate to a version the root lock does not pin (the 2026-07-22 P3.73
class) AND a same-version source/checksum substitution (the [patch]/fork shape), PASSES a
synced/subset resolution + sub-only crates + identical source/checksum, is target-absent without
the sub-lock, and fail-CLOSES on an unparseable/malformed lock (incl. a duplicate (name, version)
pair) / a sub-lock without a root lock. stdlib-only. Exit 0 = held.
"""
import importlib.machinery
import importlib.util
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-lockfile-integrity"
_loader = importlib.machinery.SourceFileLoader("cli", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cli", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def scan(text: str) -> tuple[list[str], int]:
    return m.scan_workflow_flags(text, "wf.yml")


def wf_run(*cmds: str) -> str:
    """A minimal workflow with one block-scalar `run:` step holding the given command lines."""
    body = "\n".join("          " + c for c in cmds)
    return ("name: ci\non:\n  push:\n    branches: [main]\njobs:\n  build:\n"
            "    runs-on: ubuntu-22.04\n    steps:\n      - run: |\n" + body + "\n")


def _freeze_with(**overrides) -> list[str]:
    """frozen_contract() with module constants temporarily overridden."""
    saved = {k: getattr(m, k) for k in overrides}
    for k, v in overrides.items():
        setattr(m, k, v)
    try:
        return m.frozen_contract()
    finally:
        for k, v in saved.items():
            setattr(m, k, v)


# --- (A) the structural freeze -----------------------------------------------------------------
record("freeze: the real frozen constants are internally consistent", m.frozen_contract() == [])
record("freeze: imports.lock dropped from the drift-set is caught (the G18b-overlap guard)",
       len(_freeze_with(DRIFT_SET=("Cargo.lock", "pnpm-lock.yaml"))) >= 1)
record("freeze: Cargo.lock dropped from the drift-set is caught",
       len(_freeze_with(DRIFT_SET=("pnpm-lock.yaml", "imports.lock"))) >= 1)
record("freeze: an empty cargo pin-set is caught",
       len(_freeze_with(RUST_PINNED_SUBCOMMANDS=frozenset())) >= 1)
record("freeze: `audit` leaking into the pin-set is caught (cargo audit has no --locked, G17 r7)",
       len(_freeze_with(RUST_PINNED_SUBCOMMANDS=frozenset({"build", "audit"}))) >= 1)
record("freeze: `update` leaking into the pin-set is caught (it MUTATES the lock by design)",
       len(_freeze_with(RUST_PINNED_SUBCOMMANDS=frozenset({"build", "update"}))) >= 1)
record("freeze: `vet` leaking into the pin-set is caught (G18b owns cargo vet --locked)",
       len(_freeze_with(RUST_PINNED_SUBCOMMANDS=frozenset({"build", "vet"}))) >= 1)
record("freeze: dropping --locked from the cargo pin flags is caught",
       len(_freeze_with(PIN_FLAGS=("--frozen",))) >= 1)
record("freeze: the pnpm pin flag retyped away from --frozen-lockfile is caught",
       len(_freeze_with(PNPM_PIN_FLAG="--prod")) >= 1)
record("freeze: fuzz/Cargo.lock dropped from the sub-workspace lock-sync set is caught",
       len(_freeze_with(SUBWORKSPACE_LOCKS=())) >= 1)
record("freeze: fuzz/Cargo.lock dropped from the drift-guard paths is caught",
       len(_freeze_with(DRIFT_PATHS=("Cargo.lock", "pnpm-lock.yaml", "supply-chain/imports.lock"))) >= 1)

# --- (B) the CI-workflow flag-scan -------------------------------------------------------------
record("scan: `cargo build` without --locked is caught", scan(wf_run("cargo build")) == (["wf.yml: `cargo build` runs without --locked/--frozen - it would re-resolve the dependency graph instead of the committed Cargo.lock (the audited/SBOM'd one); add --locked (§3.8 pin-everything)"], 1))
record("scan: `cargo build --locked` passes (found=1, no problem)",
       scan(wf_run("cargo build --locked")) == ([], 1))
record("scan: `cargo build --frozen` passes (--frozen is --locked+--offline, a superset)",
       scan(wf_run("cargo build --frozen")) == ([], 1))
record("scan: `cargo test --locked --all-features` passes", scan(wf_run("cargo test --locked --all-features")) == ([], 1))
record("scan: `cargo --color always build` (subcommand after a value-flag) is still caught",
       len(scan(wf_run("cargo --color always build"))[0]) == 1)
record("scan: `cargo audit` is NOT treated as a pinned build (no --locked flag exists, G17 r7)",
       scan(wf_run("cargo audit")) == ([], 0))
record("scan: `cargo deny check` is NOT treated as a pinned build (plugin, reads the lock)",
       scan(wf_run("cargo deny check")) == ([], 0))
record("scan: `cargo vet --locked check` is NOT counted here (owned by G18b)",
       scan(wf_run("cargo vet --locked check")) == ([], 0))
record("scan: `cargo +nightly-2025-01-01 fuzz` is NOT a built-in compile (fuzz-pin is G56)",
       scan(wf_run("cargo +nightly-2025-01-01 fuzz run")) == ([], 0))
record("scan: `cargo metadata` is read-only, not flagged", scan(wf_run("cargo metadata")) == ([], 0))
record("scan: `pnpm install` without --frozen-lockfile is caught",
       len(scan(wf_run("pnpm install"))[0]) == 1 and scan(wf_run("pnpm install"))[1] == 1)
record("scan: `pnpm install --frozen-lockfile` passes", scan(wf_run("pnpm install --frozen-lockfile")) == ([], 1))
record("scan: `pnpm i --frozen-lockfile` passes (the i alias)", scan(wf_run("pnpm i --frozen-lockfile")) == ([], 1))
record("scan: `pnpm --filter app install --frozen-lockfile` passes (value-flag skipped)",
       scan(wf_run("pnpm --filter app install --frozen-lockfile")) == ([], 1))
record("scan: `pnpm run build` is NOT an install, not flagged", scan(wf_run("pnpm run build")) == ([], 0))
record("scan: a step `name:` mentioning a build command is NOT scanned as a command",
       scan("jobs:\n  b:\n    steps:\n      - name: cargo build the project\n        run: echo hi\n") == ([], 0))
record("scan: a sibling `with:` after a `run: |` body is NOT swallowed into the run text",
       scan("jobs:\n  b:\n    steps:\n      - run: |\n          echo hi\n        with:\n          x: cargo build\n") == ([], 0))
record("scan: a shell line-continuation `cargo build \\<NL> --locked` is one pinned command",
       scan(wf_run("cargo build \\", "  --locked")) == ([], 1))
record("scan: `sudo cargo build` (transparent prefix) is still caught",
       len(scan(wf_run("sudo cargo build"))[0]) == 1)
record("scan: `CARGO_NET_OFFLINE=true cargo build --locked` passes (env-assignment prefix)",
       scan(wf_run("CARGO_NET_OFFLINE=true cargo build --locked")) == ([], 1))
record("scan: `cargo build --locked | tee log` passes (pipe-split segment keeps the flag)",
       scan(wf_run("cargo build --locked | tee log")) == ([], 1))
record("scan: one pinned + one UNpinned build -> exactly the unpinned one is caught (found=2)",
       (lambda r: len(r[0]) == 1 and r[1] == 2)(scan(wf_run("cargo build --locked", "cargo test"))))
record("scan: an inline `- run: cargo build` (not a block scalar) is caught",
       len(scan("jobs:\n  b:\n    steps:\n      - run: cargo build\n")[0]) == 1)
record("scan: a workflow with no cargo/pnpm build command -> target-absent (found=0)",
       scan("name: ci\non:\n  push:\njobs:\n  b:\n    steps:\n      - run: echo hi\n") == ([], 0))
# G1-review P1: a YAML-quoted inline `run:` scalar must NOT evade the scan (the quotes were leaking
# into the program token `'cargo`/`"pnpm`); it must behave identically to the block-scalar form.
record("scan: a single-quoted inline `run: 'cargo build'` is caught (YAML-quote-unwrapped)",
       len(scan("jobs:\n  b:\n    steps:\n      - run: 'cargo build'\n")[0]) == 1)
record("scan: a double-quoted inline `run: \"pnpm install\"` is caught",
       len(scan('jobs:\n  b:\n    steps:\n      - run: "pnpm install"\n')[0]) == 1)
record("scan: a single-quoted PINNED `run: 'cargo build --locked'` passes (found=1)",
       scan("jobs:\n  b:\n    steps:\n      - run: 'cargo build --locked'\n") == ([], 1))
record("scan: a double-quoted PINNED `run: \"pnpm install --frozen-lockfile\"` passes",
       scan('jobs:\n  b:\n    steps:\n      - run: "pnpm install --frozen-lockfile"\n') == ([], 1))
record("scan: a quoted unpinned build INCREMENTS found (the target-absent notice cannot mask it)",
       scan("jobs:\n  b:\n    steps:\n      - run: 'cargo build'\n")[1] == 1)
record("scan: a command that merely CONTAINS quotes (echo \"hi\" && cargo build) is still split + caught",
       len(scan(wf_run('echo "hi" && cargo build'))[0]) == 1)
# G1-review P2: a pin flag AFTER a bare `--` separator goes to the built binary, not to cargo/pnpm.
record("scan: a pin flag AFTER `--` does NOT count (`cargo run -- --locked` is unpinned -> caught)",
       len(scan(wf_run("cargo run -- --locked"))[0]) == 1)
record("scan: a pin flag BEFORE `--` counts (`cargo run --locked -- --frozen` passes)",
       scan(wf_run("cargo run --locked -- --frozen")) == ([], 1))
# G1-review P3 (the cheap half, now caught): subshell-grouped + glued-redirect build forms.
record("scan: a subshell-grouped `( cargo build )` is caught (segment-split on parens)",
       len(scan(wf_run("( cargo build )"))[0]) == 1)
record("scan: a glued-redirect `cargo build>log` is caught (segment-split on >)",
       len(scan(wf_run("cargo build>log"))[0]) == 1)
# G1 re-review P3: a shell-quoted SUBCOMMAND (`cargo 'build'`) must not fail-open (it did - the
# subcommand token kept its quotes); _subcommand now strips them. A quoted PINNED form still passes.
record("scan: a quoted subcommand `cargo 'build'` is caught (subcommand token de-quoted)",
       len(scan(wf_run("cargo 'build'"))[0]) == 1)
record("scan: a quoted subcommand pinned `cargo 'build' --locked` passes",
       scan(wf_run("cargo 'build' --locked")) == ([], 1))

# --- (C) the live drift-guard ------------------------------------------------------------------
with tempfile.TemporaryDirectory() as td:
    root = Path(td)
    # no lockfile exists -> no-op (clean)
    record("drift: no lockfile present -> no-op (0 problems)",
           m.drift_guard(root, runner=lambda r, p: (_ for _ in ()).throw(AssertionError("must not call git"))) == [])
    # create a lockfile so the guard actually consults git (injected runner)
    (root / "Cargo.lock").write_text("# lock\n", encoding="utf-8")
    record("drift: lockfile present + git rc=0 -> clean", m.drift_guard(root, runner=lambda r, p: (0, "")) == [])
    record("drift: lockfile present + git rc=1 -> drift caught",
           len(m.drift_guard(root, runner=lambda r, p: (1, "diff --git a/Cargo.lock"))) == 1)
    record("drift: git unavailable (rc=None) -> fail-closed",
           len(m.drift_guard(root, runner=lambda r, p: (None, ""))) == 1)
    record("drift: a git error (rc=128) -> fail-closed (not read as clean/drift)",
           len(m.drift_guard(root, runner=lambda r, p: (128, "fatal: not a git repo"))) == 1)
    record("drift: the present-path list passed to git contains the existing Cargo.lock",
           m.drift_guard(root, runner=lambda r, p: (0, "") if "Cargo.lock" in p else (1, "wrong paths")) == [])

# --- (D) the sub-workspace lock-sync -----------------------------------------------------------
_ROOT_LOCK = ('[[package]]\nname = "shared"\nversion = "1.0.0"\n\n'
              '[[package]]\nname = "dual"\nversion = "1.0.0"\n\n'
              '[[package]]\nname = "dual"\nversion = "2.0.0"\n')


with tempfile.TemporaryDirectory() as _sync_td:
    _sync_n = 0

    def _sync_tree(root_lock: str | None, fuzz_lock: str | None) -> Path:
        global _sync_n
        _sync_n += 1
        td = Path(_sync_td) / f"case{_sync_n}"
        td.mkdir()
        if root_lock is not None:
            (td / "Cargo.lock").write_text(root_lock, encoding="utf-8")
        if fuzz_lock is not None:
            (td / "fuzz").mkdir()
            (td / "fuzz" / "Cargo.lock").write_text(fuzz_lock, encoding="utf-8")
        return td

    record("sync: a sub-lock resolving a shared crate OFF the root pin is caught (the P3.73 class)",
           len(m.subworkspace_lock_sync(_sync_tree(
               _ROOT_LOCK, '[[package]]\nname = "shared"\nversion = "1.0.1"\n'))) == 1)
    record("sync: a sub-lock matching the root pin passes",
           m.subworkspace_lock_sync(_sync_tree(
               _ROOT_LOCK, '[[package]]\nname = "shared"\nversion = "1.0.0"\n')) == [])
    record("sync: a SUBSET of a root dual-version pin passes (bitflags-1+2 shape)",
           m.subworkspace_lock_sync(_sync_tree(
               _ROOT_LOCK, '[[package]]\nname = "dual"\nversion = "2.0.0"\n')) == [])
    record("sync: a sub-ONLY crate (libfuzzer-sys shape) is not compared, passes",
           m.subworkspace_lock_sync(_sync_tree(
               _ROOT_LOCK, '[[package]]\nname = "libfuzzer-sys"\nversion = "0.4.10"\n')) == [])
    record("sync: no sub-lock -> target-absent (0 problems)",
           m.subworkspace_lock_sync(_sync_tree(_ROOT_LOCK, None)) == [])
    record("sync: a sub-lock WITHOUT a root lock is fail-closed",
           len(m.subworkspace_lock_sync(_sync_tree(
               None, '[[package]]\nname = "shared"\nversion = "1.0.0"\n'))) == 1)
    record("sync: an unparseable sub-lock is fail-closed (never treated as synced)",
           len(m.subworkspace_lock_sync(_sync_tree(_ROOT_LOCK, "not = [ toml"))) == 1)
    record("sync: a malformed [[package]] row (no version) is fail-closed",
           len(m.subworkspace_lock_sync(_sync_tree(
               _ROOT_LOCK, '[[package]]\nname = "shared"\n'))) == 1)
    record("sync: a duplicate (name, version) pair (a shape cargo never writes) is fail-closed",
           len(m.subworkspace_lock_sync(_sync_tree(
               _ROOT_LOCK,
               '[[package]]\nname = "shared"\nversion = "1.0.0"\n\n'
               '[[package]]\nname = "shared"\nversion = "1.0.0"\nsource = "x"\n'))) == 1)
    record("sync: a same-version DIFFERENT-SOURCE substitution ([patch]/fork shape) is caught",
           len(m.subworkspace_lock_sync(_sync_tree(
               '[[package]]\nname = "reg"\nversion = "1.0.0"\n'
               'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
               'checksum = "aaaa"\n',
               '[[package]]\nname = "reg"\nversion = "1.0.0"\n'
               'source = "git+https://example.invalid/fork"\n'
               'checksum = "aaaa"\n'))) == 1)
    record("sync: a same-version DIFFERENT-CHECKSUM entry is caught",
           len(m.subworkspace_lock_sync(_sync_tree(
               '[[package]]\nname = "reg"\nversion = "1.0.0"\n'
               'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
               'checksum = "aaaa"\n',
               '[[package]]\nname = "reg"\nversion = "1.0.0"\n'
               'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
               'checksum = "bbbb"\n'))) == 1)
    record("sync: an identical source+checksum entry passes",
           m.subworkspace_lock_sync(_sync_tree(
               '[[package]]\nname = "reg"\nversion = "1.0.0"\n'
               'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
               'checksum = "aaaa"\n',
               '[[package]]\nname = "reg"\nversion = "1.0.0"\n'
               'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
               'checksum = "aaaa"\n')) == [])
    record("sync: the drift message names the crate, both version sets and the re-seed command",
           (lambda p: "shared" in p and "1.0.1" in p and "1.0.0" in p and "cp Cargo.lock" in p)(
               m.subworkspace_lock_sync(_sync_tree(
                   _ROOT_LOCK, '[[package]]\nname = "shared"\nversion = "1.0.1"\n'))[0]))

# --- end-to-end over the real repo (live since P1) ---------------------------------------------
record("main: the real repo passes (flag-scan live over the build commands + drift-guard clean)", m.main([]) == 0)

passed = sum(1 for _, ok in results if ok)
print(f"\n[g24-lockfile-integrity] {passed}/{len(results)} assertions passed.")
sys.exit(0 if passed == len(results) else 1)
