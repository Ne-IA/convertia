#!/usr/bin/env python3
"""g24-gitleaks.py - G24 planted-positive self-test for the G2 secrets scan (P0.3.1).

Proves the committed .gitleaks.toml + the pinned gitleaks BINARY actually CATCH each of the three
real CI secrets (security-concept §5) and do NOT catch the look-alike controls: it copies the
fixture (scripts/gate-selftests/gitleaks-fixtures/planted-secrets.txt) into a NON-allowlisted temp
dir, runs `gitleaks dir`, and asserts the finding set; then it scans the committed fixture path and
asserts the path-allowlist excludes it (no production false-positive). stdlib-only. A pinned-tool
bump that breaks rule parsing, or an allowlist that swallows a real secret, fails here.

Skips with a warning (exit 0) if the pinned gitleaks binary is absent (e.g. a dev box that did not
run install-gate-tools) - the L4 gate-tooling job installs it, so the canary runs it for real there.
Exit 0 = all held / skipped; 1 = a self-test failed.
"""
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONFIG = ROOT / ".gitleaks.toml"
FIXTURE = ROOT / "scripts" / "gate-selftests" / "gitleaks-fixtures" / "planted-secrets.txt"
results: list[tuple[str, bool]] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}{(' - ' + detail) if detail else ''}")


def find_gitleaks() -> str | None:
    for cand in (ROOT / ".gate-tools" / "bin" / "gitleaks.exe", ROOT / ".gate-tools" / "bin" / "gitleaks"):
        if cand.is_file():
            return str(cand)
    return shutil.which("gitleaks")


def scan(target: Path) -> list[dict]:
    """`gitleaks dir <target>` with the committed config; returns the parsed findings."""
    with tempfile.TemporaryDirectory() as td:
        report = Path(td) / "report.json"
        subprocess.run([GITLEAKS, "dir", str(target), "--config", str(CONFIG),
                        "--report-format", "json", "--report-path", str(report),
                        "--no-banner", "--exit-code", "0"], capture_output=True, text=True)
        if not report.is_file():
            return []
        return json.loads(report.read_text(encoding="utf-8") or "[]")


GITLEAKS = find_gitleaks()
if GITLEAKS is None:
    print("[g24-gitleaks] SKIP - pinned gitleaks binary not found (run scripts/install-gate-tools); "
          "the L4 gate-tooling canary installs it and runs this for real.")
    sys.exit(0)
if not CONFIG.is_file() or not FIXTURE.is_file():
    print(f"[g24-gitleaks] FAIL - missing {CONFIG if not CONFIG.is_file() else FIXTURE}", file=sys.stderr)
    sys.exit(1)

# --- planted-positive: the fixture in a NON-allowlisted temp dir -> every secret caught -----------
with tempfile.TemporaryDirectory() as td:
    shutil.copy(FIXTURE, Path(td) / "secrets.txt")
    findings = scan(Path(td))
ids = sorted(f.get("RuleID", "?") for f in findings)
by_id = {i: ids.count(i) for i in set(ids)}

record("minisign secret key caught — BOTH KDF variants (RWQA passwordless + RWRT password)",
       by_id.get("minisign-secret-key", 0) == 2, f"got {by_id.get('minisign-secret-key', 0)}")
record("MINISIGN_PASSWORD literal caught", by_id.get("minisign-password-literal", 0) >= 1)
record("ANTHROPIC_API_KEY (sk-ant-…) caught (custom rule and/or gitleaks builtin)",
       by_id.get("anthropic-api-key-custom", 0) + by_id.get("anthropic-api-key", 0) >= 1)

# the look-alikes must NOT appear: a public key (too short) and the ${{ secrets }} reference. No
# finding may match the public-key line or the reference comment.
matched = " ".join(f.get("Match", "") for f in findings)
record("look-alike minisign PUBLIC key NOT caught (length floor)", "RWQgCdI3cFGL" not in matched)
record("legitimate ${{ secrets.MINISIGN_PASSWORD }} reference NOT caught",
       not any("secrets.MINISIGN_PASSWORD" in f.get("Match", "") for f in findings))

# --- the production allowlist excludes the committed fixture (no false-positive) -----------------
record("committed fixture path is allowlisted (production scan finds 0)",
       len(scan(FIXTURE.parent)) == 0)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-gitleaks] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
