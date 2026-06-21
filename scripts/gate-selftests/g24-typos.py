#!/usr/bin/env python3
"""g24-typos.py - G24 planted-positive self-test for the G51 prose typo gate (P0.3.11).

Proves the pinned typos BINARY + the committed .typos.toml: real misspellings in the fixture ARE
flagged, the `.typos.toml` allowlist (`mis = "mis"`) DOES suppress the valid `mis-` prefix (and the
suppression is what does it - the same word IS flagged without the config), and run-typos over the
real G51 public-facing-prose scope is clean.

Skips with a warning (exit 0) if the pinned typos binary is absent (a dev box that did not run
install-gate-tools); the L4 gate-tooling job installs it and runs this for real. stdlib-only.
Exit 0 = all held / skipped; 1 = a self-test failed.
"""
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONFIG = ROOT / ".typos.toml"
FIXTURE = ROOT / "scripts" / "gate-selftests" / "typos-fixtures" / "has-typo.md"
results: list[tuple[str, bool]] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}{(' - ' + detail) if detail else ''}")


def typos_bin() -> str | None:
    for cand in (ROOT / ".gate-tools" / "bin" / "typos.exe", ROOT / ".gate-tools" / "bin" / "typos"):
        if cand.is_file():
            return str(cand)
    return shutil.which("typos")


def flagged_words(target: Path, use_config: bool) -> set[str]:
    """The set of `typo` words typos reports for `target` (JSON output), with/without .typos.toml."""
    cmd = [TYPOS, "--format", "json"]
    if use_config:
        cmd += ["--config", str(CONFIG)]
    cmd.append(str(target))
    r = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
    words: set[str] = set()
    for line in r.stdout.splitlines():
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue
        if obj.get("type") == "typo":
            words.add(obj.get("typo", ""))
    return words


TYPOS = typos_bin()
if TYPOS is None:
    print("[g24-typos] SKIP - pinned typos binary not found (run scripts/install-gate-tools); the L4 "
          "gate-tooling canary installs it and runs this for real.")
    sys.exit(0)
if not CONFIG.is_file() or not FIXTURE.is_file():
    print(f"[g24-typos] FAIL - missing {CONFIG if not CONFIG.is_file() else FIXTURE}", file=sys.stderr)
    sys.exit(1)

# --- planted positive: the fixture's real misspellings ARE caught (with the live config) ----------
fix_flagged = flagged_words(FIXTURE, use_config=True)
record("real misspelling `teh` caught", "teh" in fix_flagged, f"flagged={sorted(fix_flagged)}")
record("real misspelling `recieve` caught", "recieve" in fix_flagged)
record("the `mis` token is NOT flagged (.typos.toml allowlist applied)", "mis" not in fix_flagged)

# --- the allowlist (not typos) is what suppresses `mis` (it IS flagged WITHOUT the config) ---------
with tempfile.TemporaryDirectory() as td:
    t = Path(td) / "x.md"
    t.write_text("a mis-stripped value\n", encoding="utf-8")
    record("`mis` IS flagged without .typos.toml (so the config, not typos, suppresses it)",
           "mis" in flagged_words(t, use_config=False))
    record("`mis` is NOT flagged WITH .typos.toml (whole-word allowlist)",
           "mis" not in flagged_words(t, use_config=True))
    # honest ledger: the whole-word allowlist suppresses a STANDALONE `mis` too (the documented
    # masking trade-off accepted for the prose scope, which discusses the bare token "mis" itself)
    s = Path(td) / "standalone.md"
    s.write_text("I will mis you when it ends\n", encoding="utf-8")
    record("a standalone `mis` is ALSO suppressed (the documented whole-word masking trade-off)",
           "mis" not in flagged_words(s, use_config=True))

# --- the live G51 scope is clean today (run-typos exits 0) ----------------------------------------
rc = subprocess.run([sys.executable, str(ROOT / "scripts" / "run-typos")]).returncode
record("run-typos over the real G51 public-facing-prose scope is clean (rc=0)", rc == 0)

# --- .typos.toml carries the expected allowlist entries -------------------------------------------
cfg_text = CONFIG.read_text(encoding="utf-8")
record(".typos.toml allowlists `mis` (whole-word, honestly ledgered) + `unparseable`",
       'mis = "mis"' in cfg_text and "unparseable" in cfg_text)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-typos] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
