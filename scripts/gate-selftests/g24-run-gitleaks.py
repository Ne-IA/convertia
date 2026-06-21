#!/usr/bin/env python3
"""g24-run-gitleaks.py - G24 self-test for run-gitleaks' L2 range-base fallback chain (P0.3.1).

The L2 range scan must NEVER silently scan nothing: its base is the first of @{u} -> origin/<branch>
-> origin/main -> origin/HEAD that resolves, and on a FIRST PUSH (none resolve) it must fall back to
None so the driver FULL-scans every commit. Proves resolve_base() in real temp git repos (no network,
no gitleaks binary needed). stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import os
import subprocess
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "run-gitleaks"
_loader = importlib.machinery.SourceFileLoader("rgl", str(SCRIPT))
_spec = importlib.util.spec_from_loader("rgl", _loader)
m = importlib.util.module_from_spec(_spec)
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def git(repo: Path, *args: str) -> None:
    subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, encoding="utf-8", errors="replace", check=True)


def resolve_base_in(repo: Path):
    cwd = os.getcwd()
    os.chdir(repo)
    try:
        return m.resolve_base()
    finally:
        os.chdir(cwd)


# --- FIRST PUSH: no upstream, no origin/* -> resolve_base() is None (driver full-scans) -----------
with tempfile.TemporaryDirectory() as td:
    repo = Path(td)
    git(repo, "init", "-q", "-b", "main")
    git(repo, "config", "user.email", "t@t.t")
    git(repo, "config", "user.name", "t")
    (repo / "a.txt").write_text("x\n", encoding="utf-8")
    git(repo, "add", "-A")
    git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "init")
    record("first push (no upstream / no origin) -> resolve_base() is None (=> full-scan)",
           resolve_base_in(repo) is None)

    # --- with an origin/main ref present -> resolve_base() picks it -------------------------------
    git(repo, "update-ref", "refs/remotes/origin/main", "HEAD")
    record("origin/main present -> resolve_base() == 'origin/main'",
           resolve_base_in(repo) == "origin/main")

failed = [n for n, ok in results if not ok]
print(f"\n[g24-run-gitleaks] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
