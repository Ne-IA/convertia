#!/usr/bin/env python3
"""g24-editorconfig.py - G24 planted-positive self-test for the G52 EOL/charset hygiene gate (P0.3.11).

Proves the pinned editorconfig-checker BINARY + the committed .editorconfig actually CATCH the three
hygiene violations - CRLF line endings, trailing whitespace, a missing final newline - and pass a
clean file. The violating fixtures are created at RUNTIME in a temp dir (a CRLF/no-final-newline file
cannot be committed cleanly under `.gitattributes eol=lf`); the repo's real .editorconfig is copied in
(its `root = true` stops the upward search). Then check-editorconfig over the real repo is asserted
clean - the gate does not false-positive on the committed tree.

Skips with a warning (exit 0) if the pinned binary is absent (a dev box that did not run
install-gate-tools); the L4 gate-tooling job installs it and runs this for real. stdlib-only.
Exit 0 = all held / skipped; 1 = a self-test failed.
"""
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EDITORCONFIG = ROOT / ".editorconfig"
results: list[tuple[str, bool]] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}{(' - ' + detail) if detail else ''}")


def ec_bin() -> str | None:
    for cand in (ROOT / ".gate-tools" / "bin" / "editorconfig-checker.exe",
                 ROOT / ".gate-tools" / "bin" / "editorconfig-checker"):
        if cand.is_file():
            return str(cand)
    return shutil.which("editorconfig-checker")


def run_ec(workdir: Path) -> tuple[int, str]:
    r = subprocess.run([EC], cwd=str(workdir), capture_output=True, text=True, encoding="utf-8", errors="replace")
    return r.returncode, r.stdout + r.stderr


EC = ec_bin()
if EC is None:
    print("[g24-editorconfig] SKIP - pinned editorconfig-checker binary not found (run "
          "scripts/install-gate-tools); the L4 gate-tooling canary installs it and runs this for real.")
    sys.exit(0)
if not EDITORCONFIG.is_file():
    print(f"[g24-editorconfig] FAIL - missing {EDITORCONFIG}", file=sys.stderr)
    sys.exit(1)

# --- planted positives: each violation type is caught in a temp dir -------------------------------
with tempfile.TemporaryDirectory() as td:
    d = Path(td)
    shutil.copy(EDITORCONFIG, d / ".editorconfig")
    (d / "crlf.toml").write_bytes(b"a = 1\r\nb = 2\r\n")                 # CRLF line endings
    (d / "trailing.toml").write_bytes(b"a = 1   \nb = 2\n")             # trailing whitespace
    (d / "nofinal.toml").write_bytes(b"a = 1\nb = 2")                   # no final newline
    (d / "clean.toml").write_bytes(b"a = 1\nb = 2\n")                   # clean
    rc, out = run_ec(d)
    record("editorconfig-checker FAILS on the violating temp dir (rc != 0)", rc != 0, f"rc={rc}")
    record("CRLF file flagged", "crlf.toml" in out)
    record("trailing-whitespace file flagged", "trailing.toml" in out)
    record("missing-final-newline file flagged", "nofinal.toml" in out)
    record("the CLEAN file is NOT flagged", "clean.toml" not in out)

# --- a wholly-clean temp dir passes ---------------------------------------------------------------
with tempfile.TemporaryDirectory() as td:
    d = Path(td)
    shutil.copy(EDITORCONFIG, d / ".editorconfig")
    (d / "ok.toml").write_bytes(b"a = 1\nb = 2\n")
    rc, _ = run_ec(d)
    record("a wholly-clean temp dir passes (rc == 0)", rc == 0, f"rc={rc}")

# --- the real repo is clean today (check-editorconfig exits 0) ------------------------------------
rc = subprocess.run([sys.executable, str(ROOT / "scripts" / "check-editorconfig")]).returncode
record("check-editorconfig over the real committed tree is clean (rc=0)", rc == 0)

# --- .editorconfig carries the load-bearing rules -------------------------------------------------
ec_text = EDITORCONFIG.read_text(encoding="utf-8")
record(".editorconfig sets end_of_line=lf + insert_final_newline + charset=utf-8",
       "end_of_line = lf" in ec_text and "insert_final_newline = true" in ec_text and "charset = utf-8" in ec_text)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-editorconfig] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
