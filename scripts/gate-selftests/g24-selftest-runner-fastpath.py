#!/usr/bin/env python3
"""g24-selftest-runner-fastpath.py - G24 self-test for run-gate-selftests' --changed fastpath.

The L2 slot the G24 row declared from P0 landed 2026-08-30 as a --changed gate on the full-canary
runner. These legs pin the DECISION (pure fn - hermetic, no live git state) and the CROSS-PLANE
WIRING as PER-LINE rules over every non-comment `run-gate-selftests` invocation - lefthook.yml:
every one carries --changed; ci.yml: every one carries --require-network and NONE carries
--changed - flag-order- and second-call-site-robust, mutation-tested against the appended
`--require-network --changed` form (the R1 opus P1: a literal-adjacency substring pin passed that
mutation green). The fail-safe direction is pinned both ways: a canary may over-run, never
under-run. Since P4.29.1's owner tail these legs also pin the runner's BYTECODE-CACHE posture:
the purge of every `__pycache__` under scripts/ on entry (the ORDERING observed at spawn time,
never narrated), the PYTHONDONTWRITEBYTECODE=1 child environment proven from the wiring (the
ambient variable is popped for the leg, so inheritance cannot fake it), and the fail-closed
refusal when a cache SURVIVES the purge (the length-preserving-mutation-poisons-pycache
class - see `_purge_bytecode_caches` in the runner). Named with the g24- prefix DELIBERATELY: it is a G24
wiring/planted-positive self-test for an internal fastpath of the G24 runner, not a G10
`test-*-fastpath-pattern` detector.

Run:  python3 scripts/gate-selftests/g24-selftest-runner-fastpath.py
Exit: 0 = every assertion held; 1 = a self-test assertion FAILED.
"""
import contextlib
import importlib.machinery
import importlib.util
import io
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from types import SimpleNamespace
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "run-gate-selftests"
_loader = importlib.machinery.SourceFileLoader("rgs", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("rgs", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def _runner_lines(text: str) -> list[str]:
    """Every NON-comment line invoking run-gate-selftests (a flag in a comment does not count -
    the check-27 'promised, not applied' discipline)."""
    out = []
    for line in text.splitlines():
        s = line.split(" #", 1)[0].strip()
        if s and not s.startswith("#") and "run-gate-selftests" in s:
            out.append(s)
    return out


def lefthook_ok(text: str) -> bool:
    """L2 rule: EVERY invocation file-wide carries --changed (flag order irrelevant), and at least
    one sits UNDER the `pre-push:` key — the hook-TIER bind (the R2 opus P3: a slot relocated into
    pre-commit would blow the L1 budget while a whole-file rule stayed green). pre-push is the last
    top-level hook section in this file; the slice-to-end is exact for that layout."""
    lines = _runner_lines(text)
    idx = 0 if text.startswith("pre-push:") else text.find("\npre-push:")
    prepush = _runner_lines(text[idx:]) if idx != -1 else []
    return bool(lines) and all("--changed" in ln for ln in lines) and bool(prepush)


def ci_ok(text: str) -> bool:
    """L4 rule: >=1 invocation, EVERY one carries --require-network, NONE carries --changed."""
    lines = _runner_lines(text)
    return (bool(lines) and all("--require-network" in ln for ln in lines)
            and not any("--changed" in ln for ln in lines))


# --- the pure decision (fail-safe toward RUN) --------------------------------------------------
record("no resolvable base (first push / no git) -> RUN (fail-safe)",
       m.fastpath_decision(False, None)[0] == "run")
record("base resolved but the range diff unreadable -> RUN (fail-safe)",
       m.fastpath_decision(True, None)[0] == "run")
record("an empty push range -> SKIP", m.fastpath_decision(True, [])[0] == "skip")
record("a docs/src-only range -> SKIP (the ~0 s normal-push path)",
       m.fastpath_decision(True, ["docs/plan/P4-engine-framework.md", "src-tauri/src/pool/mod.rs"])[0] == "skip")
record("a gate SCRIPT in the range -> RUN",
       m.fastpath_decision(True, ["docs/x.md", "scripts/check-doc-links"])[0] == "run")
record("a gate SELF-TEST in the range -> RUN (scripts/** is wholesale)",
       m.fastpath_decision(True, ["scripts/gate-selftests/g24-plan-lint.py"])[0] == "run")
record("a gate-config TOML in the range -> RUN (scripts/** is wholesale)",
       m.fastpath_decision(True, ["scripts/gate-planes.toml"])[0] == "run")
record("lefthook.yml in the range -> RUN (a wiring-only push arms the pins at L2)",
       m.fastpath_decision(True, ["lefthook.yml"])[0] == "run")
record("a .github/workflows/ change -> RUN (the L4 plane is gate tooling too)",
       m.fastpath_decision(True, [".github/workflows/ci.yml"])[0] == "run")
record("requirements-ci.txt in the range -> RUN (the pip gate-toolchain pin)",
       m.fastpath_decision(True, ["requirements-ci.txt"])[0] == "run")
record("a backslash-separated path still matches the scope (Windows diff hygiene)",
       m.fastpath_decision(True, ["scripts\\check-x"])[0] == "run")
record("a QUOTED path shape still matches (the quotepath under-run closed both ways)",
       m.fastpath_decision(True, ['"scripts/check-ä.py"'])[0] == "run")
record("a NEAR-MISS prefix (scripts-extra/) does NOT arm the canary",
       m.fastpath_decision(True, ["scripts-extra/x"])[0] == "skip")
record("the runner's git wrapper pins core.quotepath=false (source pin)",
       '"core.quotepath=false"' in SCRIPT.read_text(encoding="utf-8"))

# --- the cross-plane wiring rules: mutation-tested pure legs -----------------------------------
record("ci rule: the APPENDED `--require-network --changed` mutation is RED (the R1 opus P1 probe)",
       not ci_ok("      - name: x\n        run: python3 scripts/run-gate-selftests --require-network --changed\n"))
record("ci rule: a clean full-prelude line is GREEN",
       ci_ok("        run: python3 scripts/run-gate-selftests --require-network\n"))
record("ci rule: a SECOND call site lacking --require-network is RED (per-line, not first-match)",
       not ci_ok("        run: python3 scripts/run-gate-selftests --require-network\n"
                 "        run: python3 scripts/run-gate-selftests\n"))
record("ci rule: zero invocations is RED (the prelude cannot silently vanish)", not ci_ok("jobs: {}\n"))
record("lefthook rule: flag-order-robust - `--require-network --changed` under pre-push is GREEN "
       "(no false red)",
       lefthook_ok("pre-push:\n  commands:\n    g:\n"
                   "      run: python3 scripts/run-gate-selftests --require-network --changed\n"))
record("lefthook rule: a slot line MISSING --changed is RED",
       not lefthook_ok("      run: python3 scripts/run-gate-selftests\n"))
record("lefthook rule: a flag only in a COMMENT does not count as wired",
       not lefthook_ok("    # run: python3 scripts/run-gate-selftests --changed\n"))
record("lefthook rule: a --changed slot ONLY under pre-commit is RED (the hook-TIER bind)",
       not lefthook_ok("pre-commit:\n  commands:\n    x:\n"
                       "      run: python3 scripts/run-gate-selftests --changed\n"
                       "\npre-push:\n  commands:\n    y:\n      run: echo ok\n"))

# --- the bytecode-cache purge (the length-preserving-mutation-poisons-pycache class, found
# by the P4.29.1/8affa89 review's own harness): the g24 canaries SourceFileLoader-load their
# tools, which HONOURS __pycache__, and a same-size mutation restored in the same mtime second
# leaves a VALID-looking stale .pyc compiled from the MUTANT - so the runner purges the gate
# plane's caches on entry and spawns every canary with PYTHONDONTWRITEBYTECODE=1. Both halves
# are pinned here: the purge behaviourally on this canary's own temp tree, the wiring
# hermetically (the dir globals patched, subprocess stubbed - never m.main([]) against the real
# repo, the recorded g24-target-absent hermeticity rule). ---------------------------------------


def _purge_leg() -> bool:
    try:
        with tempfile.TemporaryDirectory() as td:
            scripts = Path(td) / "scripts"
            (scripts / "__pycache__").mkdir(parents=True)
            (scripts / "__pycache__" / "toolcpython-312.pyc").write_bytes(b"stale")
            (scripts / "gate-selftests" / "__pycache__").mkdir(parents=True)
            (scripts / "gate-selftests" / "__pycache__" / "g24.pyc").write_bytes(b"stale")
            (scripts / "__pycache__extra").mkdir()
            (scripts / "__pycache__extra" / "keep.txt").write_text("near-miss", encoding="utf-8")
            (scripts / "check-x").write_text("survives", encoding="utf-8")
            removed, survivors = m._purge_bytecode_caches(scripts)
            return (
                len(removed) == 2
                and survivors == []
                and not (scripts / "__pycache__").exists()
                and not (scripts / "gate-selftests" / "__pycache__").exists()
                and (scripts / "__pycache__extra" / "keep.txt").exists()
                and (scripts / "check-x").exists()
            )
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        print(f"[g24-selftest-runner-fastpath] purge leg raised: {type(e).__name__}: {e}")
        return False


record("purge: every __pycache__ under scripts/ is removed, nested included, with NO survivor "
       "reported; a near-miss directory name and regular files survive (exact-name rglob)",
       _purge_leg())


def _survivor_arm_leg() -> bool:
    """The r2 opus P1: the SURVIVOR arm of the REAL function, driven by injection - stub
    shutil.rmtree to a no-op so the cache cannot be removed, and the fn must report it as a
    survivor, never as purged (the exact r1 sonnet P0 defect, reverted, must red HERE: an
    unconditional removed.append survives every other leg). Hermetic, no OS privilege, no
    skip - portable to all three CI platforms."""
    saved_shutil = m.shutil
    try:
        with tempfile.TemporaryDirectory() as td:
            scripts = Path(td) / "scripts"
            (scripts / "__pycache__").mkdir(parents=True)
            (scripts / "__pycache__" / "stale.pyc").write_bytes(b"stale")
            m.shutil = SimpleNamespace(rmtree=lambda *a, **k: None)
            removed, survivors = m._purge_bytecode_caches(scripts)
            return removed == [] and survivors == [scripts / "__pycache__"]
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        print(f"[g24-selftest-runner-fastpath] survivor-arm leg raised: {type(e).__name__}: {e}")
        return False
    finally:
        m.shutil = saved_shutil


record("purge survivor ARM (real fn, rmtree injected to no-op): an unremovable cache is "
       "reported as a SURVIVOR and never as purged (an unconditional removed.append reds here)",
       _survivor_arm_leg())


class _FakeProc:
    """The stubbed canary child: its transcript as an iterable stdout, then an exit code."""
    def __init__(self, text: str, rc: int = 0) -> None:
        self.stdout = io.StringIO(text)
        self._rc = rc

    def wait(self) -> int:
        return self._rc


def _fake_subprocess(popen):
    return SimpleNamespace(Popen=popen, PIPE=subprocess.PIPE, STDOUT=subprocess.STDOUT)


def _wiring_leg() -> bool:
    saved = m.SELFTEST_DIR, m.ROOT, m.subprocess
    # The runner builds child_env FROM os.environ, so an ambient PYTHONDONTWRITEBYTECODE=1
    # (exactly what the recorded probing discipline tells a human to export) would satisfy the
    # env assertion by INHERITANCE - popped here so the leg proves the WIRING, restored in the
    # same finally as the globals (the r1 opus P2).
    ambient = os.environ.pop("PYTHONDONTWRITEBYTECODE", None)
    calls: list[dict] = []

    def _popen(cmd, **kwargs):
        # cache_gone is observed AT SPAWN TIME: the leg's ordering claim ("purges BEFORE the
        # first canary") is asserted, not narrated - a purge relocated below the canary loop
        # leaves the stale cache visible to this first call and reds the leg (the r1 opus P1).
        calls.append({"cmd": cmd, "env": kwargs.get("env"), "kwargs": kwargs,
                      "cache_gone": not (m.ROOT / "scripts" / "__pycache__").exists()})
        return _FakeProc("[g24-fake] 1/1 assertions passed.\n")

    try:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            (root / "scripts" / "__pycache__").mkdir(parents=True)
            (root / "scripts" / "__pycache__" / "stale.pyc").write_bytes(b"stale")
            # The purge SCOPE, pinned in the OVER-fire direction too (the r3 opus P3): a cache
            # OUTSIDE the gate plane must survive - the runner's claim is "under scripts/",
            # never the whole tree.
            (root / "other" / "__pycache__").mkdir(parents=True)
            (root / "other" / "__pycache__" / "keep.pyc").write_bytes(b"outside the plane")
            selftests = root / "scripts" / "gate-selftests"
            selftests.mkdir()
            (selftests / "g24-fake.py").write_text("", encoding="utf-8")
            m.SELFTEST_DIR, m.ROOT = selftests, root
            m.subprocess = _fake_subprocess(_popen)
            with contextlib.redirect_stdout(io.StringIO()):
                rc = m.main([])
            return (
                rc == 0
                and len(calls) == 1
                and calls[0]["env"] is not None
                and calls[0]["env"].get("PYTHONDONTWRITEBYTECODE") == "1"
                and calls[0]["env"].get("PYTHONIOENCODING") == "utf-8"
                and calls[0]["env"].get("PYTHONUNBUFFERED") == "1"
                and calls[0]["kwargs"].get("stdout") is subprocess.PIPE
                and calls[0]["kwargs"].get("stderr") is subprocess.STDOUT
                and calls[0]["cache_gone"]
                and not (root / "scripts" / "__pycache__").exists()
                and (root / "other" / "__pycache__" / "keep.pyc").exists()
            )
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        print(f"[g24-selftest-runner-fastpath] wiring leg raised: {type(e).__name__}: {e}")
        return False
    finally:
        m.SELFTEST_DIR, m.ROOT, m.subprocess = saved
        if ambient is not None:
            os.environ["PYTHONDONTWRITEBYTECODE"] = ambient


record("wiring: main() purges the gate plane's caches BEFORE the first canary (observed at "
       "spawn time), scoped to scripts/ in BOTH directions (an outside cache survives), and "
       "spawns every canary with PYTHONDONTWRITEBYTECODE=1 + PYTHONIOENCODING=utf-8 + PYTHONUNBUFFERED=1 proven from "
       "the wiring, not ambient inheritance, its transcript piped with stderr merged into stdout "
       "(hermetic: dir globals patched, subprocess stubbed)",
       _wiring_leg())


def _survivor_leg() -> bool:
    saved = m.SELFTEST_DIR, m.ROOT, m.subprocess, m._purge_bytecode_caches
    calls: list = []

    def _popen(cmd, **kwargs):
        calls.append(cmd)
        return _FakeProc("")

    try:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            selftests = root / "scripts" / "gate-selftests"
            selftests.mkdir(parents=True)
            (selftests / "g24-fake.py").write_text("", encoding="utf-8")
            m.SELFTEST_DIR, m.ROOT = selftests, root
            m.subprocess = _fake_subprocess(_popen)
            m._purge_bytecode_caches = lambda d: ([], [d / "__pycache__"])
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(buf):
                rc = m.main([])
            return rc == 1 and calls == [] and "refusing to run" in buf.getvalue()
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        print(f"[g24-selftest-runner-fastpath] survivor leg raised: {type(e).__name__}: {e}")
        return False
    finally:
        m.SELFTEST_DIR, m.ROOT, m.subprocess, m._purge_bytecode_caches = saved


record("fail-closed: a __pycache__ that SURVIVES the purge refuses the whole run before any "
       "canary spawns (PYTHONDONTWRITEBYTECODE cannot prevent reads of a stale cache - the "
       "r1 sonnet P0)", _survivor_leg())

# --- the build-gates.md leg-count cross-check (2026-09-08, the round-12 G1 class: three row counts went stale in ONE
# commit, one of them `= 51 legs` against a live 220 - the runner is the only plane that KNOWS every canary's live
# total, so it reads the rows and reds a claim that disagrees) -------------------------------------------------------
record("live count: the LAST `N/N assertions passed` / `N/N legs passed` line of a canary's output; none -> None",
       m.live_count_of("[PASS] a\n[g24-x] 3/3 assertions passed.\n") == 3
       and m.live_count_of("50/50 legs passed\n") == 50
       and m.live_count_of("[g24-x] 2/3 assertions passed.\n[g24-x] 3/3 assertions passed.\n") == 3
       and m.live_count_of("[g24-x] SKIP - tool absent\n") is None)
_lc = lambda stem, prefix: {("g24-x", "30 r"): 25}.get((stem, prefix))
record("row counts: every `` `g24-x.py` = N legs `` / `` `g24-x.py` (N legs `` claim must equal the canary's live total; a dated"
       " history after the first number is not a claim",
       m.row_count_mismatches("| `g24-x.py` = 3 legs | `g24-y.py` (7 legs; 5 at P1.1) |", {"g24-x": 3, "g24-y": 7}, {"g24-x", "g24-y"}, _lc)
       == ([], [])
       and m.row_count_mismatches("`g24-x.py` = 4 legs", {"g24-x": 3}, {"g24-x"}, _lc)[0] == ["4 legs: the canary `g24-x.py` counts 3"]
       and m.row_count_mismatches("`g24-y.py` (5 legs", {"g24-y": 7}, {"g24-y"}, _lc)[0] == ["5 legs: the canary `g24-y.py` counts 7"])
record("row counts: a stem with no self-test file is a phantom (fail); a canary that printed no count is a NOTICE, neither pass nor fail",
       m.row_count_mismatches("`g24-zz.py` = 3 legs", {}, {"g24-x"}, _lc)[0] == ["3 legs: no such self-test file `g24-zz.py`"]
       and m.row_count_mismatches("`g24-x.py` = 3 legs", {}, {"g24-x"}, _lc)
       == ([], ["3 legs (`g24-x.py`): its canary printed no count this run - unverified"]))
record("row counts: the per-check shape `` N `prefix` legs in `g24-x.py` `` is checked against the file's `record(\"prefix` lines",
       m.row_count_mismatches("25 `30 r` legs in `g24-x.py`", {}, {"g24-x"}, _lc) == ([], [])
       and m.row_count_mismatches("24 `30 r` legs in `g24-x.py`", {}, {"g24-x"}, _lc)[0]
       == ["24 `30 r` legs in `g24-x.py`: the file carries 25 `record(\"30 r` legs"]
       and m.row_count_mismatches("2 `30 r` legs in `g24-nope.py`", {}, {"g24-x"}, lambda s, p: None)[0]
       == ["2 `30 r` legs in `g24-nope.py`: no such self-test file"])
record("row counts: prose numbers outside the two checkable shapes are not claims to this check (`twenty legs`, `canary 21/21`), nor is a"
       " word-glued number in the per-check shape (`X30 `30 r` legs in …`)",
       m.row_count_mismatches("twenty legs in the canary; canary 21/21; `g24-x.py` = 3 legs (was 2)", {"g24-x": 3}, {"g24-x"}, _lc) == ([], [])
       and m.row_count_mismatches("X30 `30 r` legs in `g24-x.py`", {}, {"g24-x"}, _lc) == ([], []))
record("row counts (shape rule): `= N <adjectives> legs` / `(N <adjectives> legs` is a total claim too (the round-14 finding:"
       " `= 56 planted-positive legs` escaped the adjacent form); a sub-count in prose (`the 9 temp-dir-backstop legs`) is not",
       m.row_count_mismatches("`g24-a.py` = 4 planted-positive legs", {"g24-a": 3}, {"g24-a"}, _lc)[0] == ["4 legs: the canary `g24-a.py` counts 3"]
       and m.row_count_mismatches("`g24-a.py` (4 planted positive legs)", {"g24-a": 3}, {"g24-a"}, _lc)[0] == ["4 legs: the canary `g24-a.py` counts 3"]
       and m.row_count_mismatches("`g24-a.py` = 3 legs (44 after the 9 temp-dir-backstop legs; 70 after the P2.135 boot-glue-exemption legs)",
                                  {"g24-a": 3}, {"g24-a"}, _lc) == ([], []))


def _cp1252_stream_leg() -> tuple[int, bytes]:
    """main() under a cp1252-encoded stdout (a Windows console pipe) streaming a transcript line that carries `→`: the
    parent reconfigures its own stdout to UTF-8 at entry, so the line streams instead of raising (the round-14 P1)."""
    saved = m.SELFTEST_DIR, m.ROOT, m.subprocess

    def _popen(cmd, **kwargs):
        return _FakeProc("[PASS] a `N→M` history\n[g24-fake] 1/1 assertions passed.\n")

    try:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            selftests = root / "scripts" / "gate-selftests"
            selftests.mkdir(parents=True)
            (selftests / "g24-fake.py").write_text("", encoding="utf-8")
            m.SELFTEST_DIR, m.ROOT = selftests, root
            m.subprocess = _fake_subprocess(_popen)
            raw = io.BytesIO()
            stream = io.TextIOWrapper(raw, encoding="cp1252", errors="strict", write_through=True)
            with contextlib.redirect_stdout(stream), contextlib.redirect_stderr(io.StringIO()):
                rc = m.main([])
            stream.flush()
            return rc, raw.getvalue()
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        return -1, repr(e).encode()
    finally:
        m.SELFTEST_DIR, m.ROOT, m.subprocess = saved


_rc_cp, _bytes_cp = _cp1252_stream_leg()
record("wiring: under a cp1252 stdout a transcript line carrying `→` streams (the parent's own stdout is reconfigured to UTF-8 at"
       " entry) instead of raising UnicodeEncodeError - rc 0 and the arrow reaches the stream as UTF-8",
       _rc_cp == 0 and "→".encode("utf-8") in _bytes_cp)
record("scope: build-gates.md in the push range -> RUN (its leg counts are cross-checked by this very run)",
       m.fastpath_decision(True, ["docs/security/build-gates.md"])[0] == "run")


def _row_check_wiring_leg(claim: str) -> tuple[int, str]:
    """Hermetic: dir globals patched, subprocess stubbed to a canary that reports 3/3; the temp root carries a
    build-gates.md with the given claim and a self-test file with two `record("30 r` legs."""
    saved = m.SELFTEST_DIR, m.ROOT, m.subprocess

    def _popen(cmd, **kwargs):
        return _FakeProc("[g24-fake] 3/3 assertions passed.\n")

    try:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            selftests = root / "scripts" / "gate-selftests"
            selftests.mkdir(parents=True)
            (selftests / "g24-fake.py").write_text('record("30 r: a")\nrecord("30 r: b")\n', encoding="utf-8")
            (root / "docs" / "security").mkdir(parents=True)
            (root / "docs" / "security" / "build-gates.md").write_text(claim, encoding="utf-8")
            m.SELFTEST_DIR, m.ROOT = selftests, root
            m.subprocess = _fake_subprocess(_popen)
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(buf):
                rc = m.main([])
            return rc, buf.getvalue()
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        return -1, f"raised: {type(e).__name__}: {e}"
    finally:
        m.SELFTEST_DIR, m.ROOT, m.subprocess = saved


_rc1, _out1 = _row_check_wiring_leg("`g24-fake.py` = 4 legs and 1 `30 r` legs in `g24-fake.py`")
_rc0, _out0 = _row_check_wiring_leg("`g24-fake.py` = 3 legs and 2 `30 r` legs in `g24-fake.py`")
record("wiring: main() reads each canary's live total from its captured output and reds a stale build-gates.md claim in EITHER"
       " shape, green when both claims hold (hermetic: dir globals patched, subprocess stubbed)",
       _rc1 == 1 and "counts 3" in _out1 and "the file carries 2" in _out1 and _rc0 == 0)
record("row counts (shape rule): EVERY `N legs` number on a line is a live claim of the NEAREST self-test mention on that line - backticked"
       " or not, `scripts/gate-selftests/`-prefixed or not, `= N` or `(N` or bare - and a line with a number but no mention FAILS as"
       " unresolvable (the round-13 P1: `g24 self-test = 30 legs` and `` `g24-core-deps.py` 12 legs `` escaped the two regexes)",
       m.row_count_mismatches("| **G1** | x | `scripts/gate-selftests/g24-a.py` (3 legs incl. the E2E) | c | d |", {"g24-a": 3}, {"g24-a"}, _lc) == ([], [])
       and m.row_count_mismatches("g24-a.py 4 legs.", {"g24-a": 3}, {"g24-a"}, _lc)[0] == ["4 legs: the canary `g24-a.py` counts 3"]
       and m.row_count_mismatches("g24 self-test = 4 legs (shared)", {"g24-a": 3}, {"g24-a"}, _lc)[0]
       == ["4 legs: no self-test file is named on this line - name the file (a bare count is unresolvable)"]
       and m.row_count_mismatches("`g24-a.py` self-test (3 legs incl. an E2E); then `g24-b.py` = 7 legs", {"g24-a": 3, "g24-b": 7}, {"g24-a", "g24-b"}, _lc)
       == ([], []))
record("row counts (shape rule): `N legs at <when>` is a dated snapshot, `+N legs` a delta and `N→M` a history - none is a live claim; the"
       " per-check shape is still checked by its own rule",
       m.row_count_mismatches("`g24-a.py` = 3 legs (2 legs at P0.4; +1 legs since; 2→3)", {"g24-a": 3}, {"g24-a"}, _lc) == ([], [])
       and m.row_count_mismatches("`g24-a.py` = 3 legs, 9 legs at delivery", {"g24-a": 3}, {"g24-a"}, _lc) == ([], [])
       and m.row_count_mismatches("25 `30 r` legs in `g24-x.py`", {}, {"g24-x"}, _lc) == ([], []))
record("wiring: a canary's transcript is streamed AND kept, so it still reaches the console",
       "[g24-fake] 3/3 assertions passed." in _out0)
record("real rows: every per-check `` N `prefix` legs in `g24-x.py` `` claim in build-gates.md holds against the real self-test"
       " files, and every total-shape claim names a real file (the live totals are checked by the runner's own run)",
       m.row_count_mismatches((REPO / "docs" / "security" / "build-gates.md").read_text(encoding="utf-8"), {},
                              {p.stem for p in (REPO / "scripts" / "gate-selftests").glob("*.py")}, m._leg_counter)[0] == [])

# --- the real plane files satisfy the rules ----------------------------------------------------
record("lefthook.yml wires `run-gate-selftests --changed` per the L2 rule",
       lefthook_ok((REPO / "lefthook.yml").read_text(encoding="utf-8")))
record("ci.yml keeps the FULL --require-network prelude and never passes --changed (L4 rule)",
       ci_ok((REPO / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")))

# The sibling-canary convention (the r2 opus P3): the last leg pins the others, so a
# silently-deleted canary leg reds the canary itself - this file newly carries a P0 closure.
record("the canary's own leg count is pinned (41 + this pin)", len(results) == 41)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-selftest-runner-fastpath] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
