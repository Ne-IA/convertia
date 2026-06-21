#!/usr/bin/env python3
"""g24-gate-planes.py - G24 self-test for check-gate-planes (P0.2.12, G54b).

Proves the plane-config validator PASSES the committed gate-planes.toml (and the real-config
shapes: a multi-plane fail_open_at with a non-plane PHASE covered_by) and FAILS each weakening /
malformed shape:
  - wrong / missing default posture; a missing, duplicated, or under-fielded plane; an unknown key
  - an unjustified fail-open (missing covered_by/reason); a self-covering fail-open - including the
    multi-plane ("L1/L2/L4" + covered_by "L1") and whitespace (" L4 ") evasions; a dangling
    covered_by/fail_open_at plane ("L99"); an inline-array `fail_open` mis-scoped into the last
    [[plane]]; a plain [fail_open] table (must FAIL CLEANLY, no Python traceback); unparseable TOML.
stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import subprocess
import sys
import tempfile
from pathlib import Path

CHECK = Path(__file__).resolve().parents[2] / "scripts" / "check-gate-planes"
REAL = Path(__file__).resolve().parents[2] / "scripts" / "gate-planes.toml"
results: list[tuple[str, bool]] = []

# a minimal VALID config (all 7 planes, fail-closed default, one justified fail-open)
PLANES = "".join(
    f'[[plane]]\nid = "{i}"\nname = "n"\ntrigger = "t"\nenforcement = "e"\nmirror = "m"\n\n'
    for i in ("L(-1)", "L0", "L1", "L2", "L3", "L4", "L5"))
FO = '[[fail_open]]\ngate = "Gx"\nfail_open_at = "L1"\ncovered_by = "L4"\nreason = "r"\n'
VALID = 'default_posture = "fail-closed"\n\n' + PLANES + FO


def record(name: str, ok: bool, detail: str = "") -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}{(' - ' + detail) if detail else ''}")


def run(text: str | None) -> tuple[int, str]:
    """(rc, stderr) of check-gate-planes against a temp toml (text); None -> the real committed file."""
    if text is None:
        p = subprocess.run([sys.executable, str(CHECK), str(REAL)], capture_output=True, text=True, encoding="utf-8", errors="replace")
        return p.returncode, p.stderr
    with tempfile.TemporaryDirectory() as td:
        f = Path(td) / "gp.toml"
        f.write_text(text, encoding="utf-8")
        p = subprocess.run([sys.executable, str(CHECK), str(f)], capture_output=True, text=True, encoding="utf-8", errors="replace")
        return p.returncode, p.stderr


def leg(name: str, text: str | None, want_rc: int) -> None:
    rc, _ = run(text)
    record(name, rc == want_rc, f"want rc={want_rc}, got {rc}")


# --- pass cases (the committed file + the real multi-plane / phase-covered_by shape) ----------
leg("committed gate-planes.toml passes", None, 0)
leg("minimal valid config passes", VALID, 0)
leg("multi-plane fail_open_at with non-plane PHASE covered_by passes (the real G56 shape)",
    VALID.replace('fail_open_at = "L1"\ncovered_by = "L4"',
                  'fail_open_at = "L1/L2/L4"\ncovered_by = "P10 / P3+ activation"'), 0)

# --- plane-structure defects ------------------------------------------------------------------
leg("missing a plane (no L3) fails", VALID.replace(
    '[[plane]]\nid = "L3"\nname = "n"\ntrigger = "t"\nenforcement = "e"\nmirror = "m"\n\n', ""), 1)
leg("duplicate plane (L3 twice) fails", VALID.replace(
    '[[plane]]\nid = "L3"\nname = "n"\ntrigger = "t"\nenforcement = "e"\nmirror = "m"\n\n',
    '[[plane]]\nid = "L3"\nname = "n"\ntrigger = "t"\nenforcement = "e"\nmirror = "m"\n\n' * 2), 1)
leg("plane missing a field fails", VALID.replace(
    '[[plane]]\nid = "L4"\nname = "n"\ntrigger = "t"\nenforcement = "e"\nmirror = "m"\n\n',
    '[[plane]]\nid = "L4"\nname = "n"\ntrigger = "t"\nenforcement = "e"\n\n'), 1)
leg("unknown key in a plane fails", VALID.replace(
    '[[plane]]\nid = "L4"\nname = "n"\ntrigger = "t"\nenforcement = "e"\nmirror = "m"\n\n',
    '[[plane]]\nid = "L4"\nname = "n"\ntrigger = "t"\nenforcement = "e"\nmirror = "m"\nbogus = "x"\n\n'), 1)

# --- default-posture defects ------------------------------------------------------------------
leg("non-fail-closed default fails", VALID.replace(
    'default_posture = "fail-closed"', 'default_posture = "fail-open"'), 1)
leg("missing default_posture fails", VALID.replace('default_posture = "fail-closed"\n', ""), 1)

# --- fail-open justification + genuine-cover defects ------------------------------------------
leg("unjustified fail-open (no covered_by) fails", VALID.replace('covered_by = "L4"\n', ""), 1)
leg("self-covering fail-open (covered_by == fail_open_at) fails",
    VALID.replace('covered_by = "L4"', 'covered_by = "L1"'), 1)
leg("MULTI-PLANE self-cover (fail_open_at 'L1/L2/L4' + covered_by 'L1') fails",
    VALID.replace('fail_open_at = "L1"\ncovered_by = "L4"',
                  'fail_open_at = "L1/L2/L4"\ncovered_by = "L1"'), 1)
leg("WHITESPACE self-cover (fail_open_at 'L4' + covered_by ' L4 ') fails",
    VALID.replace('fail_open_at = "L1"\ncovered_by = "L4"',
                  'fail_open_at = "L4"\ncovered_by = " L4 "'), 1)
leg("dangling covered_by plane ('L99' undefined) fails",
    VALID.replace('covered_by = "L4"', 'covered_by = "L99"'), 1)
leg("dangling fail_open_at plane ('L42' undefined) fails",
    VALID.replace('fail_open_at = "L1"', 'fail_open_at = "L42"'), 1)

# --- TOML form-confusion (the two ways a fail-open can hide / crash) ---------------------------
leg("inline-array `fail_open=[...]` after [[plane]] (TOML-scoped into last plane) fails",
    'default_posture = "fail-closed"\n\n' + PLANES +
    'fail_open = [{gate = "Gx", fail_open_at = "L1", covered_by = "L1", reason = "r"}]\n', 1)

# plain [fail_open] table must FAIL (rc 1) AND not via a Python traceback (clean diagnostic)
_plain_table = ('default_posture = "fail-closed"\n\n' + PLANES +
                '[fail_open]\ngate = "Gx"\nfail_open_at = "L1"\ncovered_by = "L4"\nreason = "r"\n')
_rc, _err = run(_plain_table)
record("plain [fail_open] table fails cleanly (rc 1, no traceback)",
       _rc == 1 and "Traceback" not in _err, f"rc={_rc}, traceback={'Traceback' in _err}")

leg("unparseable TOML -> exit 2", 'default_posture = "fail-closed"\n[[plane\n', 2)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-gate-planes] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
