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
  - leg (5) Python isolation: each interpreter spelling without `-P` (bare, `-m`, stdin, `-c`,
    `python3.N`, path-prefixed, `.exe`, line end, `<<`) is caught and its `-P` form is clean; comment
    text and a quoted `#` are handled; the real planes are clean, and stripping their `-P` flags
    reports one finding per invocation; a missing lefthook.yml, zero workflows, an unreadable plane
    file and an action.yml are covered; main() runs the leg; setup-dev spawns install-gate-tools
    with `-P`. The incident is replayed: a planted
    `scripts/argparse.py` and `scripts/json/__init__.py` run when a gate starts without `-P`, and
    never with `-P` or under the runner's PYTHONSAFEPATH=1.
  - leg (6) no hook auto-sync: a top-level `no_auto_install: true` is clean (a trailing comment, a
    blank before the colon, a CRLF end); an absent key, any other value, a key only indented, only in
    a comment or with no blank after the colon, and a second top-level key are caught; the real
    lefthook.yml sets it; a missing or non-UTF-8 lefthook.yml fails closed; main() runs the leg.
stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import contextlib
import importlib.machinery
import importlib.util
import io
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

REPO = Path(__file__).resolve().parents[2]
CHECK = REPO / "scripts" / "check-gate-planes"
REAL = REPO / "scripts" / "gate-planes.toml"
_loader = importlib.machinery.SourceFileLoader("cgp", str(CHECK))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cgp", _loader))
_loader.exec_module(m)
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

# --- posture_flag structural integrity (P1.66) ------------------------------------------------
# The scheduled fail-soft -> fail-closed registry: each row must name a real script + real plane
# files + a --flag + a P<n> phase. A malformed row (the structural half of the G71/P1 prevention)
# must FAIL here; plan-lint check 27 does the phase-aware WIRING half. Paths resolve against the
# REAL repo root, so positive legs name real files (check-l-neg1-ack / lefthook.yml / ci.yml).
PF = ('[[posture_flag]]\ngate = "G71"\nscript = "scripts/check-l-neg1-ack"\n'
      'enforce_flag = "--enforce"\nfail_closed_after_phase = "P1"\n'
      'planes = ["lefthook.yml", ".github/workflows/ci.yml"]\nreason = "r"\n')
VALID_PF = VALID + PF
leg("a well-formed posture_flag row passes", VALID_PF, 0)
leg("posture_flag missing a field (no enforce_flag) fails", VALID_PF.replace('enforce_flag = "--enforce"\n', ""), 1)
leg("posture_flag enforce_flag without -- fails", VALID_PF.replace('enforce_flag = "--enforce"', 'enforce_flag = "enforce"'), 1)
leg("posture_flag fail_closed_after_phase not a P<n> token fails",
    VALID_PF.replace('fail_closed_after_phase = "P1"', 'fail_closed_after_phase = "phase-one"'), 1)
leg("posture_flag planes = [] (empty) fails", VALID_PF.replace('planes = ["lefthook.yml", ".github/workflows/ci.yml"]', 'planes = []'), 1)
leg("posture_flag script that does not exist fails (dead guard path)",
    VALID_PF.replace('script = "scripts/check-l-neg1-ack"', 'script = "scripts/__nope__"'), 1)
leg("posture_flag plane file that does not exist fails (dead guard path)",
    VALID_PF.replace('".github/workflows/ci.yml"', '"nope/__missing__.yml"'), 1)
leg("posture_flag unknown key fails", VALID_PF.replace('reason = "r"\n', 'reason = "r"\nbogus = "x"\n'), 1)
# a plain [posture_flag] table (not array-of-tables) must FAIL cleanly (rc 1, no traceback)
_pf_plain = VALID + ('[posture_flag]\ngate = "G71"\nscript = "scripts/check-l-neg1-ack"\n'
                     'enforce_flag = "--enforce"\nfail_closed_after_phase = "P1"\nplanes = ["lefthook.yml"]\n')
_rc2, _err2 = run(_pf_plain)
record("plain [posture_flag] table fails cleanly (rc 1, no traceback)",
       _rc2 == 1 and "Traceback" not in _err2, f"rc={_rc2}, traceback={'Traceback' in _err2}")

# --- leg (5) Python isolation: every plane Python invocation passes -P first -------------------
# The incident first: a file placed beside a gate shadows the stdlib module the gate imports, and it
# runs at import, before the gate's own code. `scripts/gate` imports argparse and json; the planted
# `scripts/argparse.py` and `scripts/json/__init__.py` each write a marker. The ambient
# PYTHONSAFEPATH (the runner sets it for this canary) is removed so the replay sees a bare start.
_GATE = "import argparse\nimport json\n"
_PLANT = "import os\nopen(os.path.join(os.environ['G24_SHADOW_MARKERS'], {name!r}), 'w').close()\n"


def _shadow_markers(flags: list[str], safepath_env: bool) -> list[str] | None:
    """The markers the planted shadows wrote when `scripts/gate` ran with `flags`; None on a crash."""
    try:
        with tempfile.TemporaryDirectory() as td:
            scripts = Path(td) / "scripts"
            (scripts / "json").mkdir(parents=True)
            markers = Path(td) / "markers"
            markers.mkdir()
            (scripts / "gate").write_text(_GATE, encoding="utf-8")
            (scripts / "argparse.py").write_text(_PLANT.format(name="argparse"), encoding="utf-8")
            (scripts / "json" / "__init__.py").write_text(_PLANT.format(name="json"), encoding="utf-8")
            env = {k: v for k, v in os.environ.items() if k != "PYTHONSAFEPATH"}
            env["G24_SHADOW_MARKERS"] = str(markers)
            if safepath_env:
                env["PYTHONSAFEPATH"] = "1"
            p = subprocess.run([sys.executable, *flags, str(scripts / "gate")], cwd=td, env=env,
                               capture_output=True, text=True, encoding="utf-8", errors="replace")
            if p.returncode != 0:
                print(f"[g24-gate-planes] shadow replay rc={p.returncode}: {p.stderr.strip()[:300]}")
                return None
            return sorted(f.name for f in markers.iterdir())
    except OSError as e:
        print(f"[g24-gate-planes] shadow replay raised: {type(e).__name__}: {e}")
        return None


_bare = _shadow_markers([], False)
record("5 incident: without -P the planted scripts/argparse.py and scripts/json/ both run when the gate "
       "imports argparse and json", _bare == ["argparse", "json"], f"markers={_bare}")
_dash_p = _shadow_markers(["-P"], False)
record("5 incident: with -P neither planted shadow runs", _dash_p == [], f"markers={_dash_p}")
_safe_env = _shadow_markers([], True)
record("5 incident: under PYTHONSAFEPATH=1 (the runner's child env) neither planted shadow runs",
       _safe_env == [], f"markers={_safe_env}")


def iso(text: str) -> int:
    """The number of leg-(5) findings over one plane text."""
    return len(m.python_isolation_findings("plane.yml", text))


record("5 caught: bare `python3 scripts/x`", iso("      run: python3 scripts/x\n") == 1)
record("5 clean: `python3 -P scripts/x`", iso("      run: python3 -P scripts/x --flag\n") == 0)
record("5 caught: `python3 -m pip`", iso("          python3 -m pip install -r r.txt\n") == 1)
record("5 clean: `python3 -P -m pip`", iso("          python3 -P -m pip install -r r.txt\n") == 0)
record("5 caught: stdin `python3 - <<'PY'`", iso("          python3 - >> \"$GITHUB_OUTPUT\" <<'PY'\n") == 1)
record("5 caught: `python -c`", iso("        run: python -c 'print(1)'\n") == 1)
record("5 caught: `python3.12 scripts/x`", iso("        run: python3.12 scripts/x\n") == 1)
record("5 caught: a path-prefixed interpreter (`/usr/bin/python3`, `C:\\Python312\\python.exe`)",
       iso("        run: /usr/bin/python3 scripts/x\n") == 1
       and iso("        run: C:\\Python312\\python.exe scripts/x\n") == 1)
record("5 caught: an interpreter at the line end (`cat x | python3`, also before a CRLF) and before a heredoc "
       "(`python3<<'PY'`)",
       iso("          cat x | python3\n") == 1 and iso("          cat x | python3\r\n") == 1
       and iso("          python3<<'PY'\n") == 1 and iso("          python3 -P<<'PY'\n") == 0)
record("5 caught: `-P` must be the token right after the interpreter (`python3 scripts/x -P` passes it to the "
       "script)", iso("      run: python3 scripts/x -P\n") == 1)
record("5 caught: every invocation on a line counts (one bare of two = 1, two bare = 2)",
       iso("      run: python3 -P scripts/a || python3 scripts/b\n") == 1
       and iso("      run: python3 scripts/a || python3 scripts/b\n") == 2)
record("5 clean: comment text (`# system python3 is 3.10`; a trailing `# python3 x` after a -P run)",
       iso("      # system python3 is 3.10, so setup-python pins 3.12\n") == 0
       and iso("      run: python3 -P scripts/x  # python3 scripts/y\n") == 0)
record("5 caught: a `#` inside quotes is not a comment (`echo \"a #b\" && python3 scripts/x`)",
       iso("      run: echo \"a #b\" && python3 scripts/x\n") == 1)
record("5 caught: an unterminated quote keeps the rest of the line (`echo it's # python3 x`)",
       iso("      run: echo it's fine # python3 scripts/x\n") == 1)
record("5 clean: non-invocation spellings (`actions/setup-python@`, `python-version: '3.12'`, "
       "`` `python3` ``, `python3-config`)",
       iso("        uses: actions/setup-python@5fda3b95a4ea91299a34e894583c3862153e4b97  # v7.0.0\n"
           "          python-version: '3.12'\n"
           "      - name: 'needs `python3` on PATH'\n"
           "        run: python3-config --includes\n") == 0)
record("5 finding: names the file and git's line (LF split, a CRLF line end tolerated)",
       m.python_isolation_findings("a.yml", "x: 1\r\n  run: python3 s\n")
       == [f"a.yml:2: {m._PY_ISOLATION_MSG}"])
record("5 strip_line_comment: `#` at column 0 or after a blank starts a comment; `a#b` and quoted `#` are "
       "text; a backslash escapes inside double quotes",
       m.strip_line_comment("# x") == "" and m.strip_line_comment("a # b") == "a "
       and m.strip_line_comment("a#b") == "a#b" and m.strip_line_comment("'a # b' # c") == "'a # b' "
       and m.strip_line_comment('"a \\" # b" # c') == '"a \\" # b" ')

record("5 E2E: the real plane files are clean", m.plane_isolation_findings(REPO) == [],
       "; ".join(m.plane_isolation_findings(REPO)[:3]))


def _stripped_real_counts() -> list[tuple[str, int, int]]:
    """(plane file, -P flags removed, findings after the removal) for every real plane file."""
    out = []
    files = [REPO / "lefthook.yml", *sorted((REPO / ".github" / "workflows").glob("*.y*ml"))]
    for path in files:
        text = path.read_bytes().decode("utf-8")
        stripped, n = re.subn(r"\bpython3 -P(?= )", "python3", text)
        out.append((path.name, n, len(m.python_isolation_findings(path.name, stripped))))
    return out


_real = _stripped_real_counts()
_by_name = {name: (n, found) for name, n, found in _real}
record("5 E2E: removing every `-P` from the real planes reports one finding per removed flag, and "
       "lefthook.yml and ci.yml each carry invocations",
       all(n == found for _, n, found in _real)
       and _by_name.get("lefthook.yml", (0, 0))[0] > 0 and _by_name.get("ci.yml", (0, 0))[0] > 0,
       ", ".join(f"{name} {n}/{found}" for name, n, found in _real))


def _plane_root(td: str, lefthook: str | None, workflows: dict[str, bytes],
                actions: dict[str, bytes] | None = None) -> Path:
    root = Path(td)
    if lefthook is not None:
        (root / "lefthook.yml").write_text(lefthook, encoding="utf-8")
    wf = root / ".github" / "workflows"
    wf.mkdir(parents=True)
    for name, data in workflows.items():
        (wf / name).write_bytes(data)
    for rel, data in (actions or {}).items():
        target = root / ".github" / "actions" / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    return root


_CLEAN = "      run: python3 -P scripts/x\n"
with tempfile.TemporaryDirectory() as _td:
    _found = m.plane_isolation_findings(_plane_root(_td, None, {"ci.yml": _CLEAN.encode()}))
record("5 fail-closed: a missing lefthook.yml is a finding",
       len(_found) == 1 and _found[0].startswith("lefthook.yml is missing"), f"{_found}")
with tempfile.TemporaryDirectory() as _td:
    _found = m.plane_isolation_findings(_plane_root(_td, _CLEAN, {}))
record("5 fail-closed: zero workflow files is a finding",
       len(_found) == 1 and _found[0].startswith(".github/workflows/ holds no"), f"{_found}")
with tempfile.TemporaryDirectory() as _td:
    _found = m.plane_isolation_findings(_plane_root(_td, _CLEAN, {"ci.yml": b"      run: python3 \xff scripts/x\n"}))
record("5 fail-closed: a plane file that is not UTF-8 is a finding",
       len(_found) == 1 and _found[0].startswith(".github/workflows/ci.yml: cannot be read as UTF-8"), f"{_found}")
with tempfile.TemporaryDirectory() as _td:
    _found = m.plane_isolation_findings(_plane_root(
        _td, _CLEAN, {"a.yaml": b"      run: python3 scripts/x\n"},
        {"setup/action.yml": b"    - run: python3 scripts/x\n", "b/c/action.yaml": b"    - run: python3 -m pip\n"}))
record("5 scope: a .yaml workflow and every .github/actions/**/action.y*ml are scanned",
       sorted(f.split(":", 1)[0] for f in _found)
       == [".github/actions/b/c/action.yaml", ".github/actions/setup/action.yml", ".github/workflows/a.yaml"],
       f"{_found}")


def _main_rc(lefthook: str) -> tuple[int, str]:
    """(rc, stderr) of the real main() over the minimal VALID config with ROOT patched to a temp plane root."""
    saved = m.ROOT
    try:
        with tempfile.TemporaryDirectory() as td:
            root = _plane_root(td, lefthook, {"ci.yml": _CLEAN.encode()})
            cfg = root / "gp.toml"
            cfg.write_text(VALID, encoding="utf-8")
            m.ROOT = root
            err = io.StringIO()
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(err):
                rc = m.main([str(cfg)])
            return rc, err.getvalue()
    finally:
        m.ROOT = saved


def _setup_dev_spawn() -> list[str] | None:
    """The argv scripts/setup-dev spawns install-gate-tools with (its `run` stubbed, nothing executed)."""
    try:
        loader = importlib.machinery.SourceFileLoader("sdv", str(REPO / "scripts" / "setup-dev"))
        sd = importlib.util.module_from_spec(importlib.util.spec_from_loader("sdv", loader))
        loader.exec_module(sd)
        calls: list[list[str]] = []
        sd.run = lambda cmd, **kw: calls.append([str(c) for c in cmd]) or subprocess.CompletedProcess(cmd, 0)
        with contextlib.redirect_stdout(io.StringIO()):
            sd.install_gate_tools()
        return calls[0] if len(calls) == 1 else None
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        print(f"[g24-gate-planes] setup-dev spawn leg raised: {type(e).__name__}: {e}")
        return None


_sd = _setup_dev_spawn()
record("5 spawner: scripts/setup-dev runs install-gate-tools as `<python> -P scripts/install-gate-tools`",
       _sd is not None and len(_sd) == 3 and _sd[1] == "-P" and _sd[2].endswith("install-gate-tools"), f"{_sd}")

# Both wiring fixtures carry leg (6)'s key, so main() reports leg (5) alone.
_SYNC_OFF = "no_auto_install: true\n"
_rc_bad, _err_bad = _main_rc("      run: python3 scripts/x\n" + _SYNC_OFF)
_rc_ok, _err_ok = _main_rc(_CLEAN + _SYNC_OFF)
record("5 wiring: main() runs leg (5) over the repository's plane files (a bare invocation -> rc 1 naming "
       "lefthook.yml:1; the -P form -> rc 0)",
       _rc_bad == 1 and "lefthook.yml:1: a Python invocation without -P" in _err_bad and _rc_ok == 0,
       f"bad rc={_rc_bad}, ok rc={_rc_ok}")


# --- leg (6) no hook auto-sync: lefthook.yml's top-level `no_auto_install: true` ------------------------------
# The incident, measured with the pinned lefthook 2.1.9 in a clone on a path with spaces and parentheses: after a
# lefthook.yml change, a forced `lefthook run pre-commit` re-installed all three hooks with lefthook's path
# unquoted (each then failed `sh -n`) and a `git commit` aborted in the re-installed commit-msg hook; with the
# key, the hooks stayed byte-identical through a forced run, a plain run, `git hook run` and a real commit.
def nai(text: str) -> list[str]:
    """The leg-(6) findings over one lefthook.yml text."""
    return m.no_auto_install_findings(text)


_ABSENT = "lefthook.yml: no top-level `no_auto_install`"
record("6 clean: a top-level `no_auto_install: true`, also with a trailing comment, a blank before the colon and a "
       "CRLF line end",
       nai('min_version: "2.1.9"\n' + _SYNC_OFF) == [] and nai("no_auto_install: true   # leg (6)\r\n") == []
       and nai("no_auto_install : true\n") == [])
record("6 caught: the key absent",
       [f[:len(_ABSENT)] for f in nai('min_version: "2.1.9"\nassert_lefthook_installed: true\n')] == [_ABSENT])
record("6 caught: a value other than `true` (`false`, `yes`, `on`, `True`, `\"true\"`, empty)",
       all(len(f := nai(f"no_auto_install: {v}\n")) == 1 and "is not `true`" in f[0]
           for v in ("false", "yes", "on", "True", '"true"', "")))
record("6 caught: the key only indented under another key, only in a comment, or with no blank after the colon",
       all([f[:len(_ABSENT)] for f in nai(t)] == [_ABSENT]
           for t in ("pre-commit:\n  no_auto_install: true\n", "# no_auto_install: true\n",
                     "min_version: x  # no_auto_install: true\n", "no_auto_install:true\n")))
record("6 caught: the key set twice at the top level (twice true; true then false)",
       all(len(f := nai(t)) == 1 and "set 2 times" in f[0]
           for t in (_SYNC_OFF * 2, _SYNC_OFF + "no_auto_install: false\n")))
_real_sync = m.lefthook_sync_findings(REPO)
record("6 E2E: the real lefthook.yml sets it", _real_sync == [], "; ".join(_real_sync))
with tempfile.TemporaryDirectory() as _td:
    _missing = m.lefthook_sync_findings(Path(_td))
    (Path(_td) / "lefthook.yml").write_bytes(_SYNC_OFF.encode() + b"# \xff\n")
    _not_utf8 = m.lefthook_sync_findings(Path(_td))
record("6 fail-closed: a missing lefthook.yml and one that is not UTF-8 are findings",
       len(_missing) == 1 and _missing[0].startswith("lefthook.yml is missing")
       and len(_not_utf8) == 1 and "cannot be read as UTF-8" in _not_utf8[0], f"{_missing} {_not_utf8}")
_rc_nokey, _err_nokey = _main_rc(_CLEAN)
record("6 wiring: main() runs leg (6) over the repository's lefthook.yml (no key -> rc 1 naming it; the key -> rc 0)",
       _rc_nokey == 1 and _ABSENT in _err_nokey and _rc_ok == 0, f"no key rc={_rc_nokey}, key rc={_rc_ok}")

failed = [n for n, ok in results if not ok]
print(f"\n[g24-gate-planes] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
