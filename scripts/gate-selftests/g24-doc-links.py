#!/usr/bin/env python3
"""g24-doc-links.py - G24 self-test for check-doc-links (the G74 leg-(b) rustdoc driver).

Source-pin + arming legs, NO live `cargo doc` run (the live leg IS the gate at L2/L4; a ~47 s
doc build inside the selftest canary would treble every push for no coverage gain):
  - the two rustdoc link lints and the exact cargo argv are PINNED (a dropped flag - e.g.
    --document-private-items, which carries most of the coverage - is a self-test red, not a
    silent narrowing);
  - the child env carries the flags IN-PROCESS (the pwsh inline-prefix class stays closed);
  - subprocess decode pins encoding="utf-8" (the gate-subprocess-cp1252-decode standing rule);
  - the cargo-absent skip is pinned via a shutil.which mock - NEVER via live absence, because
    GitHub runners SHIP cargo (the g24 hermeticity rule: mock the probe, don't trust the plane);
  - red/green arming: a failing child propagates exit 1 (the gate bites), a clean child passes.

Run:  python3 scripts/gate-selftests/g24-doc-links.py
Exit: 0 = every assertion held; 1 = a self-test assertion FAILED (the gate is broken).
"""
import importlib.machinery
import importlib.util
import sys
import types
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-doc-links"
_loader = importlib.machinery.SourceFileLoader("cdl", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cdl", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


src = SCRIPT.read_text(encoding="utf-8")

# --- the pinned contract ----------------------------------------------------------------------
record("both rustdoc link lints are DENIED in RUSTDOCFLAGS (broken + private)",
       "-D rustdoc::broken_intra_doc_links" in m.RUSTDOCFLAGS
       and "-D rustdoc::private_intra_doc_links" in m.RUSTDOCFLAGS)
record("cargo argv pins doc + --workspace + --no-deps + --document-private-items + --locked",
       m.CARGO_DOC[:2] == ["cargo", "doc"]
       and all(f in m.CARGO_DOC for f in
               ("--workspace", "--no-deps", "--document-private-items", "--locked")))
record("subprocess decode pins encoding=utf-8 (the cp1252 Windows-decode trap)",
       'encoding="utf-8"' in src)

# --- the cargo-absent skip (mocked: runners SHIP cargo, so live absence proves nothing) -------
_which = m.shutil.which
try:
    m.shutil.which = lambda _cmd: None
    record("cargo-absent plane skips green with a notice (toolchain-absent posture)",
           m.main() == 0)
finally:
    m.shutil.which = _which

# --- red/green arming + the in-process env pin ------------------------------------------------
calls: dict[str, object] = {}


def _fake_run(argv, **kw):
    calls["argv"] = list(argv)
    calls["env"] = dict(kw.get("env") or {})
    return types.SimpleNamespace(returncode=_fake_run.rc, stdout="", stderr="error: unresolved link\n")


_run, _sw = m.subprocess.run, m.shutil.which
try:
    m.shutil.which = lambda _cmd: "cargo"
    m.subprocess.run = _fake_run
    _fake_run.rc = 101
    record("a failing cargo doc propagates exit 1 (the gate bites - arming leg)", m.main() == 1)
    _fake_run.rc = 0
    record("a clean cargo doc passes (exit 0)", m.main() == 0)
    record("the child env carries the pinned RUSTDOCFLAGS in-process + the pinned argv "
           "(shell-form-immune on every OS - the pwsh inline-prefix class)",
           calls["env"].get("RUSTDOCFLAGS") == m.RUSTDOCFLAGS and calls["argv"] == m.CARGO_DOC)
finally:
    m.subprocess.run, m.shutil.which = _run, _sw

failed = [n for n, ok in results if not ok]
print(f"\n[g24-doc-links] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
