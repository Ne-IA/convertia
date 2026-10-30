#!/usr/bin/env python3
"""g24-net-retry - G24 self-test for the gate plane's one network retry policy (scripts/net-retry, build-gates §0).

Pins the policy both where it lives and where it is copied:

1. scripts/net-retry, compiled from its source as its users load it and driven with a fake clock (no leg waits): the
   nominal pauses (2 s doubling to a 60 s cap), the jitter range (the upper half of each nominal pause), the window
   (retrying ends 300 s after the FIRST failure, the last pause cut to end there), a final outcome returned or raised
   at once, the last transient outcome returned or raised once the window has ended, the spent Retrier that attempts
   no further fetch, the urllib transient classifier (a URLError judged by its reason; a proxy's 429 or 5xx answer to
   a CONNECT transient; a local file error, a rejected certificate, a proxy's refusal, a string reason and an invalid
   URL final) and the log lines; read_body() (one read1 per chunk, its deadline, its limit, a body cut short); and, in
   real time, the wall-clock bound of one attempt: bounded() runs the attempt on a daemon thread, ends a hung one as
   a timeout, hands a value that arrives late to its on_late and returns one that is in when the bound passes, and
   run_bounded() closes stdin and ends a hung subprocess and every process of its group with SIGTERM - a group that
   cleans up on it ends at once - and with SIGKILL once KILL_GRACE_S has passed, the bound measured from the moment
   its child reported its grandchildren, and returns with its output read from files although a grandchild that left
   the group with setsid() still holds them.
2. The workflow retry loops (.github/workflows/*.yml and *.yaml), which run the same schedule in shell, read by regex
   over the lines of each run: value (check-ci-supply-chain's own reader; quoted text, escapes and ${...} blanked):
   every loop around a `sleep "$pause"` line opens with `until ...; do` or `while :; do`, has its schedule reset on
   the line right before it and runs the policy schedule built from net-retry's constants line for line after its
   opener (a `while :` loop: after its `if [ "$rc" -eq 0 ]; then break; fi` line) - the window, the time left, a
   spent branch that fails the step (`exit 1`), the pause draw and cut, one `::warning::` line, the sleep, the
   doubling, the echoed texts free of backticks, escapes and `$(` or `$[` - and no other line of the loop reads the
   window or exits; each loop runs a fetch; no run: line sleeps a fixed literal; every fetch command - an apt
   download (apt-get, apt, aptitude or apt-fast without --no-download), a git clone, fetch, pull or ls-remote, a curl
   or wget download, a cargo fetch, with global options before the subcommand - sits inside such a loop; every try
   but a cargo fetch runs under a coreutils `timeout` after any privilege switch; a loop that clones removes its
   partial clone first (`until rm -rf <dir> && ... git clone ... <dir>; do`); and each workflow holds the number of
   loops _LOOPS_PINNED names. Planted negatives prove each rule bites (the old fixed tries, a drifted schedule line,
   a spent branch that does not fail the step, a decoy exit, an unwrapped fetch in each spelling, a try without its
   bound, a clone without its removal). The loops' shell grammar is not modelled: a loop whose condition, a line
   ahead of it or a value it reads defeats its own schedule (`until <fetch> || true; do`, a trap, a function, an
   environment variable, an expansion that runs a command) is outside this canary, a residual-ledger line; the
   workflows are L(-1) inputs under the owner's ack and the G1 review (build-gates §0 *Gates over L(-1) inputs*).
3. The attempt bounds and the retry jobs' timeouts: every per-attempt bound of the users (each module's constant,
   read by its source's syntax tree, never run) equals the value _BOUNDS pins, and each job that runs a retried fetch
   carries `timeout-minutes` equal to its no-outage budget (_JOB_BUDGET_MIN) plus its slowest retried fetch's worst
   case - the first try at its bound, the 300 s window, the last try at its bound, a run_bounded step counted with
   KILL_GRACE_S - rounded up to the minute; a loop's try bound is the sum of its tries' `timeout` durations and
   kill-after delays (a `cargo fetch` loop has none and is left out). Planted negatives prove a timeout below or
   above the computed one, an unbudgeted retry job and a drifted bound each red.

The sites that USE the policy pin their wiring in their own canaries (g24-install-gate-tools, g24-supply-chain,
g24-branch-protection, g24-release-tag-trust, and the --selftest of fetch-engine-assets and record-action-pins).

Run:  python3 scripts/gate-selftests/g24-net-retry.py
Exit: 0 = every assertion held; 1 = a self-test assertion FAILED, or a leg hung past _HANG_S (the watchdog's exit).
"""
import ast
import contextlib
import errno
import faulthandler
import http.client
import importlib.machinery
import importlib.util
import math
import os
import random
import re
import signal
import socket
import ssl
import subprocess
import sys
import tempfile
import threading
import time
import types
import urllib.error
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")
# No leg may hang the canary: the real-time legs of the attempt bounds wait on a thread or a child process, and a bound
# that no longer holds would hold the canary - and its job - until the job's own timeout with no finding printed. Past
# _HANG_S, faulthandler's watchdog thread prints the stack that hung and exits 1. The run's end cancels it.
_HANG_S = 60
faulthandler.dump_traceback_later(_HANG_S, exit=True)

REPO = Path(__file__).resolve().parents[2]
WORKFLOWS = REPO / ".github" / "workflows"


def _load_gate(name: str, rel: str):
    loader = importlib.machinery.SourceFileLoader(name, str(REPO / rel))
    module = importlib.util.module_from_spec(importlib.util.spec_from_loader(name, loader))
    loader.exec_module(module)
    return module


def _compile_policy() -> types.ModuleType:
    """scripts/net-retry compiled from its source into a fresh module, the way every user of it loads it."""
    path = REPO / "scripts" / "net-retry"
    module = types.ModuleType("net_retry")
    exec(compile(path.read_bytes(), str(path), "exec"), module.__dict__)
    return module


NET = _compile_policy()
GATE = _load_gate("check_ci_supply_chain", "scripts/check-ci-supply-chain")

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}{(' - ' + detail) if detail else ''}")


class _FakeClock:
    """Moves only when the Retrier sleeps or an attempt spends time on purpose: a leg reads the exact pauses."""

    def __init__(self) -> None:
        self.now = 0.0
        self.pauses: list[float] = []

    def clock(self) -> float:
        return self.now

    def sleep(self, seconds: float) -> None:
        self.pauses.append(seconds)
        self.now += seconds


def _retrier(rng=lambda: 1.0) -> tuple[object, _FakeClock, list[str]]:
    fc, logs = _FakeClock(), []
    return NET.Retrier("probe", clock=fc.clock, sleep=fc.sleep, rng=rng, log=logs.append), fc, logs


def _raiser(errors: list[BaseException], fc: _FakeClock | None = None, stall: float = 0.0):
    """An attempt that raises errors[i] on call i, then returns "ok"; with `fc`, the first call spends `stall`."""
    calls = {"n": 0}

    def attempt() -> str:
        i = calls["n"]
        calls["n"] += 1
        if fc is not None and i == 0:
            fc.now += stall
        if i < len(errors):
            raise errors[i]
        return "ok"

    return attempt, calls


def _run(retrier, attempt, **kw):
    """retrier.run(attempt) -> ('value', v) | ('raised', exception)."""
    try:
        return "value", retrier.run(attempt, what="probe-fetch", **kw)
    except Exception as e:  # noqa: BLE001 - the leg judges the exception
        return "raised", e


_H502 = urllib.error.HTTPError("u", 502, "Bad Gateway", {}, None)

# --- 1. the module ---------------------------------------------------------------------------------------------
record("the constants: the first nominal pause 2 s, the cap 60 s, the window 300 s",
       (NET.BASE_S, NET.CAP_S, NET.WINDOW_S) == (2, 60, 300), f"{(NET.BASE_S, NET.CAP_S, NET.WINDOW_S)}")
_gen = NET.nominal_pauses()
record("the nominal pauses double from 2 s and stay at the 60 s cap",
       [next(_gen) for _ in range(8)] == [2, 4, 8, 16, 32, 60, 60, 60])

r, fc, logs = _retrier()
attempt, calls = _raiser([_H502] * 99)
kind, got = _run(r, attempt, transient_error=NET.transient_urllib_error)
record("schedule: a fetch that stays transient pauses 2, 4, 8, 16, 32, 60, 60, 60 s, cuts the last pause to end "
       "300 s after its first failure, then re-raises its last error and spends the Retrier",
       kind == "raised" and got is _H502 and calls["n"] == 10 and r.spent
       and fc.pauses == [2.0, 4.0, 8.0, 16.0, 32.0, 60.0, 60.0, 60.0, 58.0] and fc.now == 300.0,
       f"calls={calls['n']} pauses={fc.pauses} clock={fc.now}")
record("the log: one line per retried attempt naming the fetch, the reason and the pause, then the closing line",
       logs[0] == "[probe] probe-fetch: attempt 1 failed (HTTP 502); retrying in 2.0s"
       and logs[-1] == "[probe] probe-fetch: attempt 10 failed (HTTP 502); the 300 s retry window is spent - "
                       "failing closed" and len(logs) == 10, logs[-1] if logs else "no log line")
attempt, calls = _raiser([])
kind, got = _run(r, attempt)
record("a spent Retrier raises WindowSpent and never calls the attempt",
       kind == "raised" and isinstance(got, NET.WindowSpent) and calls["n"] == 0
       and "not attempted" in str(got), str(got))

r, fc, _logs = _retrier(rng=lambda: 0.0)
attempt, calls = _raiser([_H502] * 99)
_run(r, attempt, transient_error=NET.transient_urllib_error)
record("jitter: the lowest draw halves every nominal pause and the window still ends 300 s after the first failure",
       fc.pauses == [1.0, 2.0, 4.0, 8.0, 16.0] + [30.0] * 8 + [29.0] and fc.now == 300.0, f"pauses={fc.pauses}")

r, fc, _logs = _retrier(rng=random.Random(4242).random)
attempt, calls = _raiser([_H502] * 99)
_run(r, attempt, transient_error=NET.transient_urllib_error)
_nominal = [2, 4, 8, 16, 32] + [60] * 99
record("jitter: every random pause lies in the upper half of its nominal pause, the cut last one excepted",
       len(fc.pauses) > 6 and all(n / 2 <= p <= n for p, n in zip(fc.pauses[:-1], _nominal)) and fc.now == 300.0,
       f"pauses={[round(p, 1) for p in fc.pauses]}")

r, fc, _logs = _retrier()
attempt, calls = _raiser([_H502] * 99, fc=fc, stall=100.0)
_run(r, attempt, transient_error=NET.transient_urllib_error)
record("the window opens at the FIRST FAILURE: a first attempt that hangs 100 s still leaves 300 s of retries",
       fc.now == 400.0 and fc.pauses[-1:] == [58.0], f"clock={fc.now}")


class _ShortSleepClock(_FakeClock):
    """A clock whose sleep ends after half the time asked for (a sleep cut short): the end of the window then comes
    from the cut pause alone, never from the clock read after it."""

    def sleep(self, seconds: float) -> None:
        self.pauses.append(seconds)
        self.now += seconds / 2


_sc = _ShortSleepClock()
_sc_retrier = NET.Retrier("probe", clock=_sc.clock, sleep=_sc.sleep, rng=lambda: 1.0, log=lambda _msg: None)
_sc_calls = {"n": 0}


def _sc_attempt() -> str:
    _sc_calls["n"] += 1
    if _sc_calls["n"] == 2:
        _sc.now = 296.0          # the second attempt fails 4 s before the window ends: its 4 s pause reaches the end
    raise _H502


kind, got = _run(_sc_retrier, _sc_attempt, transient_error=NET.transient_urllib_error)
record("schedule: a pause that reaches the window's end exactly is the last one, so the attempt after it is the last "
       "whatever the clock reads after the sleep (a sleep that ended early)",
       kind == "raised" and _sc_calls["n"] == 3 and _sc.pauses == [2.0, 4.0] and _sc_retrier.spent,
       f"calls={_sc_calls['n']} pauses={_sc.pauses}")
r, fc, _logs = _retrier()
_at_end = {"n": 0}


def _end_attempt() -> str:
    _at_end["n"] += 1
    if _at_end["n"] == 2:
        fc.now = 300.0           # the second attempt fails exactly when the window ends
    raise _H502


kind, got = _run(r, _end_attempt, transient_error=NET.transient_urllib_error)
record("schedule: an attempt that fails exactly when the window ends is the last one (no pause of 0 s and no further "
       "attempt)", kind == "raised" and _at_end["n"] == 2 and fc.pauses == [2.0] and r.spent,
       f"calls={_at_end['n']} pauses={fc.pauses}")

r, fc, _logs = _retrier()
_h404 = urllib.error.HTTPError("u", 404, "Not Found", {}, None)
attempt, calls = _raiser([_h404])
kind, got = _run(r, attempt, transient_error=NET.transient_urllib_error)
record("a final error is raised at once: one attempt, no pause, the Retrier not spent",
       kind == "raised" and got is _h404 and calls["n"] == 1 and fc.pauses == [] and not r.spent)

r, fc, _logs = _retrier()
attempt, calls = _raiser([_H502, _H502])
kind, got = _run(r, attempt, transient_error=NET.transient_urllib_error)
record("a fetch that recovers returns its value and leaves the Retrier unspent",
       (kind, got) == ("value", "ok") and calls["n"] == 3 and not r.spent and fc.pauses == [2.0, 4.0])

r, fc, _logs = _retrier()
_seq = iter([503, 503, 200])
kind, got = _run(r, lambda: next(_seq), transient_result=lambda code: f"HTTP {code}" if code >= 500 else None)
record("a transient RESULT is retried like a transient error; a final result is returned",
       (kind, got) == ("value", 200) and fc.pauses == [2.0, 4.0])
r, fc, _logs = _retrier()
kind, got = _run(r, lambda: 503, transient_result=lambda code: f"HTTP {code}" if code >= 500 else None)
record("once the window has ended, the last transient RESULT is returned to its caller and the Retrier is spent",
       (kind, got) == ("value", 503) and r.spent and fc.now == 300.0)

r, fc, _logs = _retrier()
attempt, calls = _raiser([ValueError("not a network error")])
kind, got = _run(r, attempt, transient_error=NET.transient_urllib_error)
record("an exception the classifier does not name (a programming error) propagates at once",
       kind == "raised" and isinstance(got, ValueError) and calls["n"] == 1)

_d = NET.Retrier("defaults")
record("the production defaults are the real monotonic clock, time.sleep and random.random",
       _d._clock is time.monotonic and _d._sleep is time.sleep and _d._rng is random.random and not _d.spent)


def _status(code: int) -> urllib.error.HTTPError:
    return urllib.error.HTTPError("u", code, "x", {}, None)


# (exception, transient?) - urlopen wraps each socket-level failure as URLError(<that OSError>), so a URLError is
# judged by its reason, and one whose reason is a string (an unknown URL type, no host) never came from the network.
_CLASSIFIED = [
    (_status(429), True), (_status(500), True), (_status(502), True), (_status(503), True), (_status(504), True),
    (_status(599), True), (_status(400), False), (_status(403), False), (_status(404), False), (_status(410), False),
    (urllib.error.URLError(ConnectionRefusedError("refused")), True),
    (urllib.error.URLError(socket.gaierror(socket.EAI_AGAIN, "Temporary failure in name resolution")), True),
    (urllib.error.URLError(OSError(errno.ENETUNREACH, "Network is unreachable")), True),
    (urllib.error.URLError(TimeoutError("timed out")), True),
    (urllib.error.URLError(ssl.SSLError(1, "[SSL: UNEXPECTED_EOF_WHILE_READING]")), True),
    (urllib.error.URLError(ssl.SSLCertVerificationError(1, "[SSL: CERTIFICATE_VERIFY_FAILED]")), False),
    (urllib.error.URLError("unknown url type: ftp"), False), (urllib.error.URLError("no host given"), False),
    (TimeoutError("timed out"), True), (NET.AttemptTimeout("no result within the bound"), True),
    (ConnectionResetError("reset"), True), (http.client.RemoteDisconnected("closed"), True),
    (http.client.IncompleteRead(b"x", 10), True),
    (OSError(errno.ENOSPC, "No space left on device"), False), (PermissionError(errno.EACCES, "denied"), False),
    (FileNotFoundError(errno.ENOENT, "No such file or directory"), False),
    (http.client.InvalidURL("nonnumeric port"), False), (http.client.BadStatusLine("garbage"), False),
    (ValueError("x"), False), (KeyError("x"), False),
    # an HTTP proxy's own answer to the CONNECT, as http.client raises it (an OSError without an errno) and urlopen
    # wraps it: its 429 or 5xx (the egress proxy's 502 or 503 for an upstream it cannot reach) is an outage, its 403
    # (a host off its allowlist) or 407 a refusal
    (OSError("Tunnel connection failed: 503 Service Unavailable"), True),
    (urllib.error.URLError(OSError("Tunnel connection failed: 502 Bad Gateway")), True),
    (OSError("Tunnel connection failed: 429 Too Many Requests"), True),
    (urllib.error.URLError(OSError("Tunnel connection failed: 403 Forbidden")), False),
    (OSError("Tunnel connection failed: 407 Proxy Authentication Required"), False),
    (NET.BodyTooLarge("past the limit"), False),
]
_wrong = [f"{type(e).__name__}({getattr(e, 'code', '') or getattr(e, 'reason', '') or e})" for e, want in _CLASSIFIED
          if (NET.transient_urllib_error(e) is not None) != want]
record("the urllib classifier: 429 and every 5xx transient, every other status final; a URLError by its reason - a "
       "refused connection, DNS, no route, a timeout, TLS transient, a rejected certificate or a string reason final; "
       "a timeout (a bound that fired among them), a reset or dropped connection and a short body transient; a local "
       "file error, an invalid URL, any other http.client failure and any other exception final",
       not _wrong, ", ".join(_wrong) or f"{len(_CLASSIFIED)} cases")
record("the urllib classifier: a proxy's 429, 502 or 503 answer to a CONNECT is transient, its 403 or 407 final, and "
       "a body past its reader's limit final",
       NET.transient_urllib_error(OSError("Tunnel connection failed: 503 Service Unavailable")) == "proxy CONNECT 503"
       and NET.transient_urllib_error(urllib.error.URLError(OSError("Tunnel connection failed: 403 Forbidden"))) is None
       and NET.transient_urllib_error(NET.BodyTooLarge("x")) is None)

# The wall-clock bound of one attempt, in real time (each leg waits a fraction of a second).
record("bounded: an attempt's value and its exception pass through",
       NET.bounded(lambda: 7, 5.0)() == 7 and _run(NET.Retrier("x"), NET.bounded(lambda: int("x"), 5.0))[0] == "raised")
_release = threading.Event()
_t0 = time.monotonic()
try:
    NET.bounded(lambda: _release.wait(5), 0.2)()          # an attempt that hangs for 5 s, past its 0.2 s bound
    _hung = "returned"
except NET.AttemptTimeout as _e:
    _hung = f"AttemptTimeout: {_e}"
except Exception as _e:  # noqa: BLE001 - the leg judges the exception
    _hung = f"{type(_e).__name__}: {_e}"
_took = time.monotonic() - _t0
_release.set()
record("bounded: an attempt with no result inside its bound raises AttemptTimeout (a TimeoutError) at the bound",
       _hung.startswith("AttemptTimeout") and issubclass(NET.AttemptTimeout, TimeoutError) and _took < 5.0,
       f"{_hung} after {_took:.2f} s")
record("bounded: the attempt runs on a daemon thread (a worker past its bound never holds the process open)",
       NET.bounded(lambda: threading.current_thread().daemon and threading.current_thread().name, 5.0)()
       == "net-retry-attempt")
_late_gate, _late_seen = threading.Event(), []
_late_done = threading.Event()


def _late_value() -> str:
    _late_gate.wait(5)
    return "late response"


def _on_late(value: str) -> None:
    _late_seen.append(value)
    _late_done.set()


try:
    NET.bounded(_late_value, 0.1, on_late=_on_late)()
    _late_kind = "returned"
except NET.AttemptTimeout:
    _late_kind = "AttemptTimeout"
_late_gate.set()
_late_done.wait(5)
record("bounded: a value that arrives after the bound goes to on_late (a late response closed there), never to the "
       "caller, who got AttemptTimeout", _late_kind == "AttemptTimeout" and _late_seen == ["late response"],
       f"{_late_kind}; on_late got {_late_seen}")


class _ExpiredEvent(threading.Event):
    """An Event whose wait() lets the worker finish and then reports its timeout as passed: the attempt's value or
    error is in by the time bounded() looks, as when an attempt ends just as its bound does."""

    def wait(self, timeout: float | None = None) -> bool:
        super().wait(5)
        return False


_saved_event = threading.Event
threading.Event = _ExpiredEvent
try:
    _at_bound_value = _run(NET.Retrier("x"), NET.bounded(lambda: "in at the bound", 0.01))
    _at_bound_error = _run(NET.Retrier("x"), NET.bounded(lambda: int("x"), 0.01))
finally:
    threading.Event = _saved_event
record("bounded: a value or an error that is in once the bound has passed is returned or raised as it is - only an "
       "attempt with no result is abandoned as AttemptTimeout",
       _at_bound_value == ("value", "in at the bound") and _at_bound_error[0] == "raised"
       and type(_at_bound_error[1]) is ValueError, f"{_at_bound_value}; {_at_bound_error}")


class _Body:
    """A urllib response double for read_body(): `chunks` returned by read1 one per call, each call moving the fake
    clock one second (`clock`); `declared` the length its headers declare (http.client's `length` attribute, counted
    down per chunk). read() raises: read_body reads with read1, one system call per chunk."""

    def __init__(self, chunks: list[bytes], declared: int | None = None) -> None:
        self.chunks = list(chunks)
        self.length = declared
        self.reads = 0
        self.now = 0.0

    def clock(self) -> float:
        return self.now

    def read1(self, _n: int = -1) -> bytes:
        self.reads += 1
        self.now += 1.0
        part = self.chunks.pop(0) if self.chunks else b""
        if self.length is not None:
            self.length -= len(part)
        return part

    def read(self, *_a) -> bytes:
        raise AssertionError("read_body must read with read1 (one system call per chunk), never read()")


def _read(body: _Body, deadline: float, limit: int | None = None) -> tuple[str, object]:
    try:
        return "value", NET.read_body(body, deadline=deadline, limit=limit, clock=body.clock)
    except Exception as exc:  # noqa: BLE001 - the leg judges the exception
        return "raised", exc


_b = _Body([b"ab", b"cd", b"ef"])
record("read_body: every chunk of a body read with read1, one call per chunk, until the end of the body",
       _read(_b, deadline=100.0) == ("value", b"abcdef") and _b.reads == 4, f"reads={_b.reads}")
_b = _Body([b"x"] * 10)
_kind, _got = _read(_b, deadline=2.5)
_reads_at_end = _b.reads
record("read_body: a transfer still running at its deadline raises AttemptTimeout after the chunk in flight, and "
       "reads no further byte", _kind == "raised" and isinstance(_got, NET.AttemptTimeout) and _reads_at_end == 3
       and _b.reads == 3 and NET.transient_urllib_error(_got) is not None, f"{_kind} {_got!r} reads={_b.reads}")
_b = _Body([b"ab", b"cd"], declared=4)
record("read_body: a body whose declared length is in returns at its deadline without another read",
       _read(_b, deadline=2.0) == ("value", b"abcd") and _b.reads == 2, f"reads={_b.reads}")
_kind, _got = _read(_Body([b"abc", b"def"]), deadline=100.0, limit=4)
record("read_body: a body past its limit raises BodyTooLarge (final)",
       _kind == "raised" and isinstance(_got, NET.BodyTooLarge) and NET.transient_urllib_error(_got) is None)
_kind, _got = _read(_Body([b"ab", b"cd"]), deadline=100.0, limit=4)
record("read_body: a body of exactly its limit is read whole (only a byte past the limit is refused)",
       (_kind, _got) == ("value", b"abcd"), f"{_kind} {_got!r}")
_kind, _got = _read(_Body([b"abc"], declared=8), deadline=100.0)
record("read_body: a body that ends short of its declared length raises IncompleteRead (transient)",
       _kind == "raised" and isinstance(_got, http.client.IncompleteRead) and NET.transient_urllib_error(_got) is not None,
       f"{_kind} {_got!r}")

_ok = NET.run_bounded([sys.executable, "-c", "import sys; print('out'); sys.stderr.write('err'); sys.exit(3)"], 30)
record("run_bounded: exit code, stdout and stderr of a run inside its bound pass through",
       (_ok.returncode, _ok.stdout.strip(), _ok.stderr.strip()) == (3, "out", "err"),
       f"{(_ok.returncode, _ok.stdout, _ok.stderr)}")
# This process's own stdin is a pipe holding bytes for the leg, so a child that inherited it would read them: the
# child of run_bounded must read end-of-file from the null device instead. The stdin this process had - none at all
# under a runner that closed it - is put back after the leg.
try:
    _stdin_saved: int | None = os.dup(0)
except OSError:
    _stdin_saved = None
_stdin_r, _stdin_w = os.pipe()
os.write(_stdin_w, b"inherited stdin\n")
os.close(_stdin_w)
if _stdin_r != 0:
    os.dup2(_stdin_r, 0)
    os.close(_stdin_r)
try:
    _in = NET.run_bounded([sys.executable, "-c", "import sys; print(repr(sys.stdin.read()))"], 30)
finally:
    if _stdin_saved is not None:
        os.dup2(_stdin_saved, 0)
        os.close(_stdin_saved)
    else:
        os.close(0)
record("run_bounded: stdin is the null device, so a command that reads it gets end-of-file - never this process's "
       "stdin (a pipe holding bytes here), never a wait", _in.returncode == 0 and _in.stdout.strip() == "''",
       _in.stdout.strip())
_env = NET.run_bounded([sys.executable, "-c", "import os; print(os.environ.get('NR_PROBE', '-'))"], 30,
                       env={**os.environ, "NR_PROBE": "set"})
record("run_bounded: the environment passed is the child's", _env.stdout.strip() == "set", _env.stdout.strip())
# A child that ignores SIGTERM and starts two grandchildren that ignore it too, both holding the same output: one in
# its process group, and (POSIX) one that leaves the group with setsid(); it reports their pids, then hangs. The bound
# starts once the child has reported them (the Popen of run_bounded waits for the report file, so a slow runner's
# start-up never races the bound): the kill must reach the child and the grandchild of its group although they
# ignore SIGTERM, and the call must return at the bound with the report read from its files although the grandchild
# that left the group still holds them.
_HANG_GROUP = ("import os, signal, subprocess, sys, time\n"
               "signal.signal(signal.SIGTERM, signal.SIG_IGN)\n"
               "ignore = 'import signal, time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(120)'\n"
               "inside = subprocess.Popen([sys.executable, '-c', ignore])\n"
               "left = subprocess.Popen([sys.executable, '-c', ignore], start_new_session=True) "
               "if os.name == 'posix' else None\n"
               "line = f'{inside.pid} {left.pid if left else 0}'\n"
               "print(line, flush=True)\n"
               "with open(sys.argv[1], 'w', encoding='utf-8') as fh:\n"
               "    fh.write(line + '\\n')\n"
               "time.sleep(120)\n")
_REAL_POPEN = subprocess.Popen


class _ReportedPopen(_REAL_POPEN):
    """subprocess.Popen for the leg's run_bounded call: it returns once the child wrote its report file (30 s at
    most), so the bound run_bounded starts after it is measured from the report."""

    report: Path = Path()
    reported_at = 0.0

    def __init__(self, *args, **kwargs) -> None:
        super().__init__(*args, **kwargs)
        end = time.monotonic() + 30
        while time.monotonic() < end:
            with contextlib.suppress(OSError):
                if type(self).report.read_text(encoding="utf-8").strip():
                    break
            time.sleep(0.01)
        type(self).reported_at = time.monotonic()


with tempfile.TemporaryDirectory() as _td:
    _ReportedPopen.report = Path(_td) / "report"
    subprocess.Popen = _ReportedPopen
    try:
        _hang = NET.run_bounded([sys.executable, "-c", _HANG_GROUP, str(_ReportedPopen.report)], 1.0)
    finally:
        subprocess.Popen = _REAL_POPEN
    _took = time.monotonic() - _ReportedPopen.reported_at
_pids = [int(w) for w in _hang.stdout.split()[:2]] if len(_hang.stdout.split()) >= 2 else [0, 0]
_inside, _left = _pids


def _gone(pid: int) -> bool:
    """True once `pid` is no live process (POSIX: signal 0; a zombie awaiting its reaper counts as gone)."""
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return True
    except PermissionError:
        return False
    try:
        return Path(f"/proc/{pid}/stat").read_text().split(")")[-1].split()[0] == "Z"
    except OSError:
        return False


if os.name == "posix":
    _left_alive = bool(_left) and not _gone(_left)          # read before this leg ends it below
    _deadline = time.monotonic() + 10
    while _inside and not _gone(_inside) and time.monotonic() < _deadline:
        time.sleep(0.1)
    _ended = bool(_inside) and _gone(_inside)
    if _left:
        with contextlib.suppress(ProcessLookupError, PermissionError):
            os.kill(_left, signal.SIGKILL)
        _deadline = time.monotonic() + 10
        while not _gone(_left) and time.monotonic() < _deadline:
            time.sleep(0.1)
else:
    # Windows: no setsid(), and a pid probe would end the process (os.kill there is TerminateProcess); taskkill /T
    # ends the tree, and the leg pins the exit, the bound and the report read from the files
    _left_alive, _ended = True, bool(_inside)
_GRACE = NET.KILL_GRACE_S if os.name == "posix" else 0.0     # Windows: taskkill /F /T at once, no grace
record("run_bounded: a run past its bound whose group ignores SIGTERM returns exit 124 with a `timed out` line once "
       "KILL_GRACE_S has passed after its bound (then SIGKILL; Windows: taskkill /F /T at once), the grandchild of its "
       "process group ended, and the report read from the output files although the grandchild that left the group "
       "(setsid) still held them",
       _hang.returncode == 124 and "timed out" in _hang.stderr and 1.0 + _GRACE <= _took < 1.0 + _GRACE + 8.0
       and _ended and _left_alive and _inside > 0,
       f"exit {_hang.returncode} {_took:.1f} s after the report; in-group grandchild {_inside} ended: {_ended}; "
       f"grandchild {_left} out of the group still running at the return: {_left_alive}")
# A child and a grandchild of its process group that both clean up on SIGTERM - each writes its marker 0.3 s after the
# signal and exits, the child once it has reaped the grandchild - as git removes the partial clone it was making: the
# group must get SIGTERM before any SIGKILL, so both markers exist, and the call returns once the group has ended,
# well inside the grace. Each process installs its handler before the child reports (the grandchild says so in a
# ready file), so the signal never meets a process without its handler.
_TERM_CLEAN = ("import signal, sys, time\n"
               "def done(*_):\n"
               "    time.sleep(0.3)\n"
               "    open(sys.argv[1], 'w').write('cleaned')\n"
               "    sys.exit(0)\n"
               "signal.signal(signal.SIGTERM, done)\n"
               "open(sys.argv[1] + '.ready', 'w').write('ready')\n"
               "time.sleep(120)\n")
_TERM_GROUP = ("import os, signal, subprocess, sys, time\n"
               "mark, report, clean = sys.argv[1], sys.argv[2], sys.argv[3]\n"
               "kid = subprocess.Popen([sys.executable, '-c', clean, mark + '.grandchild'])\n"
               "end = time.monotonic() + 30\n"
               "while not os.path.exists(mark + '.grandchild.ready') and time.monotonic() < end:\n"
               "    time.sleep(0.01)\n"
               "def done(*_):\n"
               "    time.sleep(0.3)\n"
               "    open(mark + '.child', 'w').write('cleaned')\n"
               "    kid.wait(30)\n"
               "    sys.exit(0)\n"
               "signal.signal(signal.SIGTERM, done)\n"
               "open(report, 'w').write(f'{kid.pid}\\n')\n"
               "time.sleep(120)\n")
with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as _td:
    _mark = Path(_td) / "mark"
    _ReportedPopen.report = Path(_td) / "report"
    subprocess.Popen = _ReportedPopen
    try:
        _term = NET.run_bounded([sys.executable, "-c", _TERM_GROUP, str(_mark), str(_ReportedPopen.report),
                                 _TERM_CLEAN], 1.0)
    finally:
        subprocess.Popen = _REAL_POPEN
    _term_took = time.monotonic() - _ReportedPopen.reported_at
    _marks = sorted(p.name for p in Path(_td).iterdir() if p.name in ("mark.child", "mark.grandchild"))
if os.name == "posix":
    _term_ok = _marks == ["mark.child", "mark.grandchild"] and 1.2 < _term_took < 1.0 + NET.KILL_GRACE_S - 1.5
else:
    _term_ok = _marks == [] and _term_took < 1.0 + 8.0     # taskkill /F /T: the tree ends before either cleans up
record("run_bounded: a run past its bound gets SIGTERM before any SIGKILL, so its child and the grandchild of its "
       "process group each clean up (a marker each), and the call returns once the group has ended, well inside "
       "KILL_GRACE_S (Windows: taskkill /F /T ends the tree at once, before either cleans up)",
       _term.returncode == 124 and _term_ok, f"exit {_term.returncode} {_term_took:.1f} s after the report; "
                                            f"markers {_marks}")
# A child that exits at once on SIGTERM while the grandchild of its group still cleans up (as cargo-deny ends at once
# and git still removes its partial clone): run_bounded waits for the group, not for its child alone, so the
# grandchild's marker is there when the call returns, at the latest once the grace has passed.
_TERM_LEADER = ("import os, signal, subprocess, sys, time\n"
                "mark, report, clean = sys.argv[1], sys.argv[2], sys.argv[3]\n"
                "kid = subprocess.Popen([sys.executable, '-c', clean, mark + '.grandchild'])\n"
                "end = time.monotonic() + 30\n"
                "while not os.path.exists(mark + '.grandchild.ready') and time.monotonic() < end:\n"
                "    time.sleep(0.01)\n"
                "signal.signal(signal.SIGTERM, lambda *_: os._exit(0))\n"
                "open(report, 'w').write(f'{kid.pid}\\n')\n"
                "time.sleep(120)\n")
with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as _td:
    _mark = Path(_td) / "mark"
    _ReportedPopen.report = Path(_td) / "report"
    subprocess.Popen = _ReportedPopen
    try:
        _lead = NET.run_bounded([sys.executable, "-c", _TERM_LEADER, str(_mark), str(_ReportedPopen.report),
                                 _TERM_CLEAN], 1.0)
    finally:
        subprocess.Popen = _REAL_POPEN
    _lead_took = time.monotonic() - _ReportedPopen.reported_at
    _lead_marked = (Path(_td) / "mark.grandchild").is_file()
if os.name == "posix":
    _lead_ok = _lead_marked and _lead_took < 1.0 + NET.KILL_GRACE_S + 3.0
else:
    _lead_ok = not _lead_marked and _lead_took < 1.0 + 8.0
record("run_bounded: after SIGTERM it waits for every process of the group, not for its child alone: a child that "
       "ends at once leaves the grandchild of its group cleaning up, and the grandchild's marker is there when the "
       "call returns (Windows: taskkill /F /T ends the tree at once)",
       _lead.returncode == 124 and _lead_ok, f"exit {_lead.returncode} {_lead_took:.1f} s after the report; "
                                            f"grandchild marker {_lead_marked}")

# --- 2. the workflow retry loops -------------------------------------------------------------------------------
# The canonical shell lines of a workflow retry loop, built from the module's constants: a loop that drifts from the
# module (or the module from the loops) reds here. CANON[0] resets the schedule before its loop; CANON[1:] is the
# loop body's schedule, in order, with the spent branch (_SPENT_RE) between the time left and the pause draw.
CANON = (
    f"delay={NET.BASE_S} deadline=",
    f"deadline=${{deadline:-$((SECONDS + {NET.WINDOW_S}))}}",
    "left=$((deadline - SECONDS))",
    "pause=$((delay / 2 + RANDOM % (delay / 2 + 1)))",
    'if [ "$pause" -gt "$left" ]; then pause=$left; fi',
    'sleep "$pause"',
    f"delay=$((delay * 2 < {NET.CAP_S} ? delay * 2 : {NET.CAP_S}))",
)
# The text a loop echoes: no backtick, no backslash, no `$(` or `$[` - an echoed text that runs a command or an
# arithmetic assignment (`$(...)`, `$((SECONDS = 0))`, `$[...]`) is no message. `$rc` and `${pause}` read as text.
_ECHO_TEXT = r'(?:[^"`\\$]|\$(?![(\[]))+'
# The spent branch: once the window has ended, the loop fails its step - an `::error::` line naming the fetch, then
# `exit 1`. A branch that exits 0, breaks out of the loop or only echoes lets the job run on without the fetch (the
# G17 advisory-db warm step is fail-closed only through this exit).
_SPENT_RE = re.compile(rf'if \[ "\$left" -le 0 \]; then echo "::error::{_ECHO_TEXT}"; exit 1; fi')
_SPENT_SHAPE = 'if [ "$left" -le 0 ]; then echo "::error::<the fetch>"; exit 1; fi'
_WARN_RE = re.compile(rf'echo "::warning::{_ECHO_TEXT}"')
_LEFT_READ_RE = re.compile(r"\$\{?left\b")
_EXIT_RE = re.compile(r"\bexit\b")
_SLEEP_RE = re.compile(r"\bsleep\b")
# The schedule a loop runs after its opener (a `while :` loop: after its success break), line for line: the spent
# branch and the `::warning::` line are patterns (each names its fetch), every other line is CANON's.
_SCHEDULE = (CANON[1], CANON[2], _SPENT_RE, CANON[3], CANON[4], _WARN_RE, CANON[5], CANON[6])
_SCHEDULE_SHAPE = (CANON[1], CANON[2], _SPENT_SHAPE, CANON[3], CANON[4], 'echo "::warning::<the fetch>"', *CANON[5:])
_OPEN_UNTIL_RE = re.compile(r"until .+; do")
_OPEN_WHILE = "while :; do"
_SUCCESS_BREAK = 'if [ "$rc" -eq 0 ]; then break; fi'
_LOOP_OPEN_RE = re.compile(r"^(?:until|while)\b.*;\s*do$")
# A fetch command: the first apt, git, curl, wget or cargo word of a simple command, past any prefix (`sudo`,
# `timeout` and its options): an apt call (the apt spellings check-ci-supply-chain's leg (13) reads) without
# `--no-download`, git's clone, fetch, pull or ls-remote, any curl or wget call, cargo's fetch - each subcommand found
# past the global options before it (`git -C <dir>`, `git -c <k=v>`, `cargo +<toolchain>`, `cargo -Z <flag>`); the
# options named here take their value as the next word.
_GIT_WORD_RE = re.compile(r"(?:\S*/)?git")
_DOWNLOADER_WORD_RE = re.compile(r"(?:\S*/)?(?:curl|wget)")
_CARGO_WORD_RE = re.compile(r"(?:\S*/)?cargo")
_GIT_FETCHES = ("clone", "fetch", "pull", "ls-remote")
_CARGO_FETCH = "a cargo fetch"
_GIT_VALUED = frozenset({"-C", "-c", "--git-dir", "--work-tree", "--namespace", "--super-prefix", "--config-env",
                         "--attr-source"})
_CARGO_VALUED = frozenset({"-C", "-Z", "--config", "--color", "--explain"})
# The retry loops each workflow holds: a loop added or removed, or a workflow whose loops this reader stops seeing,
# reds here until the pin names the new count.
_LOOPS_PINNED = {"ci.yml": 5, "fuzz.yml": 1, "release.yml": 1}


def _workflow_files(directory: Path) -> list[Path]:
    """Every workflow file G56 reads, in its order: *.yml, then *.yaml."""
    return (sorted(directory.glob("*.yml")) + sorted(directory.glob("*.yaml"))) if directory.is_dir() else []


def _blocks(text: str) -> list[list[tuple[int, str]]]:
    """Every run: value of a workflow as its (line, text) list - the G56 reader's run_values, grouped by adjacency
    (two run: values are never on adjacent lines)."""
    blocks: list[list[tuple[int, str]]] = []
    prev = None
    for no, line in GATE.run_values(text.split("\n")):
        if prev is None or no != prev + 1:
            blocks.append([])
        blocks[-1].append((no, line))
        prev = no
    return blocks


def _loop_spans(lines: list[str]) -> list[tuple[int, int]]:
    """(first, last) line indexes of every `until ...; do` / `while ...; do` loop through its `done`."""
    spans: list[tuple[int, int]] = []
    opened: list[int] = []
    for i, line in enumerate(lines):
        if _LOOP_OPEN_RE.match(line):
            opened.append(i)
        elif line == "done" and opened:
            spans.append((opened.pop(), i))
    return spans


def _subcommand(words: list[str], valued: frozenset[str]) -> str | None:
    """The first word past a command's global options: each `-` word (with the next word when it is an option of
    `valued` written apart from its value) and each `+<toolchain>` word; None when none follows."""
    i = 0
    while i < len(words):
        word = words[i]
        if word.startswith("+"):
            i += 1
        elif word.startswith("-"):
            i += 2 if word in valued else 1
        else:
            return word
    return None


def _fetch_kind(words: list[str]) -> tuple[str, int] | None:
    """(what one simple command fetches, the index of its command word) for its words (quoted text blanked), or None
    (the comment above _GIT_WORD_RE)."""
    for at, word in enumerate(words):
        rest = words[at + 1:]
        if GATE._APT_WORD_RE.fullmatch(word):
            return None if "--no-download" in rest else ("an apt download", at)
        if _DOWNLOADER_WORD_RE.fullmatch(word):
            return f"a {word.rsplit('/', 1)[-1]} download", at
        if _GIT_WORD_RE.fullmatch(word):
            sub = _subcommand(rest, _GIT_VALUED)
            return (f"a git {sub}", at) if sub in _GIT_FETCHES else None
        if _CARGO_WORD_RE.fullmatch(word):
            return (_CARGO_FETCH, at) if _subcommand(rest, _CARGO_VALUED) == "fetch" else None
    return None


def _timeout_bound(prefix: list[str]) -> float | None:
    """(pure) The seconds a coreutils `timeout` before a command bounds it - its duration plus its kill-after delay -
    for the words before the command (after their last privilege switch, check-ci-supply-chain's _timed rule), or
    None when no `timeout` with a positive literal duration stands there."""
    if not GATE._timed(prefix):
        return None
    switch = max((n for n, word in enumerate(prefix) if GATE._PRIVILEGE_WORD_RE.fullmatch(word)), default=-1)
    unit = {"": 1, "s": 1, "m": 60, "h": 3600, "d": 86400}
    for n in range(len(prefix) - 1, switch, -1):
        if not GATE._TIMEOUT_WORD_RE.fullmatch(prefix[n]):
            continue
        rest, k, kill = prefix[n + 1:], 0, 0.0
        while k < len(rest) and rest[k].startswith("-"):
            if rest[k] in ("-k", "--kill-after") and k + 1 < len(rest):
                m = re.fullmatch(r"(\d+(?:\.\d*)?)([smhd]?)", rest[k + 1])
                kill = float(m.group(1)) * unit[m.group(2)] if m else 0.0
            k += 2 if rest[k] in GATE._TIMEOUT_VALUED_OPTS else 1
        m = re.fullmatch(r"(\d+(?:\.\d*)?)([smhd]?)", rest[k]) if k < len(rest) else None
        if m and float(m.group(1)) > 0:
            return float(m.group(1)) * unit[m.group(2)] + kill
    return None


def _masked(text: str) -> str:
    """(pure) `text` with every quoted span (its quotes included), every backslash escape and every `${...}` outside
    single quotes filled with `q`, length-preserving: check-ci-supply-chain's _blank_shell_words with a filler that
    keeps each span one word, so the separators left are the unquoted ones and `git -C "$db" fetch` still reads the
    value of its `-C`."""
    out = list(text)
    i, quote = 0, None
    while i < len(text):
        ch = text[i]
        if quote != "'" and ch == "\\":
            out[i:i + 2] = "q" * len(text[i:i + 2])
            i += 2
            continue
        if quote != "'" and text.startswith("${", i):
            end = text.find("}", i)
            end = len(text) if end < 0 else end + 1
            out[i:end] = "q" * (end - i)
            i = end
            continue
        if quote is None and ch in "'\"":
            quote = ch
        elif ch == quote:
            quote = None
        elif quote is None:
            i += 1
            continue
        out[i] = "q"
        i += 1
    return "".join(out)


def _fetches(lines: list[str]) -> list[tuple[int, str, float | None]]:
    """(line index, what it fetches, its coreutils `timeout` bound in seconds or None) for every fetch command of a
    run: value's lines: its logical lines (a trailing backslash joins the next; the index is the first line's), quoted
    text, escapes and `${...}` masked (_masked), cut at the unquoted command separators (check-ci-supply-chain's)."""
    logical: list[tuple[int, str]] = []
    pending: tuple[int, str] | None = None
    for i, text in enumerate(lines):
        if pending is not None:
            i, text, pending = pending[0], pending[1] + " " + text, None
        if text.endswith("\\"):
            pending = (i, text[:-1])
            continue
        logical.append((i, text))
    if pending is not None:
        logical.append(pending)
    found: list[tuple[int, str, float | None]] = []
    for i, text in logical:
        for command in re.split("[" + re.escape("".join(sorted(GATE._SHELL_SEPARATORS))) + "]", _masked(text)):
            words = command.split()
            kind = _fetch_kind(words)
            if kind:
                found.append((i, kind[0], _timeout_bound(words[:kind[1]])))
    return found


def _schedule_findings(where: str, setup: list[str], opener: str, body: list[str], blanks: list[str]) -> list[str]:
    """The findings for one retry loop's policy lines: `setup` is its run: value's lines between the previous retry
    loop and this one's `opener`, `body` its lines between the opener and `done` (`blanks`: the unquoted words of the
    opener and the body). The loop is preceded by its own reset, carries the schedule lines and the spent branch in
    order, and no other line - its opener included - reads the window or exits."""
    missing = [] if CANON[0] in setup else [f"{CANON[0]} (before the loop)"]
    missing += [c for c in CANON[1:] if c not in body]
    spent = [i for i, line in enumerate(body) if _SPENT_RE.fullmatch(line)]
    if not spent:
        missing.append(_SPENT_SHAPE)
    findings = [f"{where}: the retry loop lacks the policy line(s) " + " | ".join(missing)] if missing else []
    if CANON[0] in setup and setup[-1] != CANON[0]:
        findings.append(f"{where}: `{setup[-1]}` stands between the loop's schedule reset and its opener")
    for line, blank in zip([opener, *body], blanks):
        if _SPENT_RE.fullmatch(line):
            continue
        if _LEFT_READ_RE.search(line) and line != CANON[4]:
            findings.append(f"{where}: `{line}` reads the retry window outside the spent branch and the pause cut")
        if _EXIT_RE.search(blank):
            findings.append(f"{where}: `{line}` exits the loop outside its spent branch (`exit 1`)")
    if not missing:
        order = [body.index(CANON[1]), body.index(CANON[2]), spent[0], *(body.index(c) for c in CANON[3:])]
        if any(a >= b for a, b in zip(order, order[1:])):
            findings.append(f"{where}: the retry loop runs its schedule out of order (open the window, read the time "
                            "left, fail the step once it is spent, draw and cut the pause, sleep, double)")
    return findings


def _shape_findings(where: str, opener: str, body: list[str]) -> list[str]:
    """The findings for one retry loop's opener and its schedule line for line: `until ...; do` with the schedule
    as its body, or `while :; do` with the schedule after its success break."""
    findings: list[str] = []
    schedule = body
    if _OPEN_UNTIL_RE.fullmatch(opener):
        pass
    elif opener != _OPEN_WHILE:
        findings.append(f"{where}: `{opener}` opens the retry loop - it opens with `until <fetch>; do` or "
                        f"`{_OPEN_WHILE}`")
    elif _SUCCESS_BREAK in body:
        schedule = body[body.index(_SUCCESS_BREAK) + 1:]
    else:
        findings.append(f"{where}: the `while :` loop has no success break (`{_SUCCESS_BREAK}`)")
    for n, want in enumerate(_SCHEDULE):
        got = schedule[n] if n < len(schedule) else None
        if got is None or not (want.fullmatch(got) if isinstance(want, re.Pattern) else got == want):
            findings.append(f"{where}: the loop's schedule is not the policy's line for line - its line {n + 1} is "
                            f"{'missing' if got is None else f'`{got}`'}, the policy's `{_SCHEDULE_SHAPE[n]}`")
            return findings
    if len(schedule) > len(_SCHEDULE):
        findings.append(f"{where}: the loop's schedule is not the policy's line for line - "
                        f"`{schedule[len(_SCHEDULE)]}` follows its last line")
    return findings


# A loop that clones removes the partial clone of a failed try first: its `until` condition is `rm -rf <dir> && ...
# git clone ... <dir>`, the same directory word at both ends.
_CLONE_RESET_RE = re.compile(r"until rm -rf (\S+) && .* \1; do")


def loop_findings(name: str, text: str) -> tuple[list[str], int]:
    """(findings, the number of retry loops read) for one workflow's text (_read_loops)."""
    findings, loops, _tries = _read_loops(name, text)
    return findings, loops


def loop_tries(text: str) -> list[tuple[int, float | None]]:
    """(the 1-based line of each retry loop's opener, its try bound in seconds) for one workflow's text: the sum of
    the coreutils `timeout` bounds of the fetches the loop runs (_read_loops), None when one of them carries none (a
    `cargo fetch`)."""
    return _read_loops("", text)[2]


def _read_loops(name: str, text: str) -> tuple[list[str], int, list[tuple[int, float | None]]]:
    """(findings, the number of retry loops read, each loop's opener line and try bound) for one workflow's text. A
    retry loop is the innermost `until`/`while` ... `done` loop around a `sleep "$pause"` line; each one is checked on
    its own lines - its policy lines (_schedule_findings), its opener and schedule (_shape_findings), its fetches,
    each but a `cargo fetch` under a coreutils `timeout`, and the removal of a partial clone - so one loop never lends
    its lines to another, and every fetch of a run: value must sit inside a retry loop (its opener or its body)."""
    findings: list[str] = []
    loops = 0
    tries: list[tuple[int, float | None]] = []
    for block in _blocks(text):
        lines = [line.strip() for _no, line in block]
        words = [GATE._blank_shell_words(line) for line in lines]
        for (no, _raw), line, blank in zip(block, lines, words):
            if _SLEEP_RE.search(blank) and line != CANON[5]:
                findings.append(f"{name}:{no}: `{line}` sleeps outside the policy schedule (`sleep \"$pause\"`)")
        spans = _loop_spans(lines)
        sleeps = [i for i, line in enumerate(lines) if line == CANON[5]]
        innermost = [max(((a, b) for a, b in spans if a < i < b), default=None) for i in sleeps]
        retry_spans = sorted({span for span in innermost if span is not None})
        loops += len(retry_spans)
        for i, span in zip(sleeps, innermost):
            if span is None:
                findings.append(f"{name}:{block[i][0]}: `sleep \"$pause\"` outside an `until`/`while` ... `done` loop")
        fetches = _fetches(lines)
        prev_end = -1
        for a, b in retry_spans:
            where = f"{name}:{block[a][0]}"
            findings += _schedule_findings(where, lines[prev_end + 1:a], lines[a], lines[a + 1:b], words[a:b])
            findings += _shape_findings(where, lines[a], lines[a + 1:b])
            inside = [(i, what, bound) for i, what, bound in fetches if a <= i <= b]
            if not inside:
                findings.append(f"{where}: the retry loop runs no fetch (an apt download, a git clone, fetch, pull or "
                                "ls-remote, a curl or wget download, a cargo fetch)")
            for i, what, bound in inside:
                if bound is None and what != _CARGO_FETCH:
                    findings.append(f"{name}:{block[i][0]}: {what} in a retry loop runs under no coreutils `timeout` "
                                    "(a try with no wall-clock bound; after any `sudo`)")
                if what == "a git clone" and not (i == a and _CLONE_RESET_RE.fullmatch(lines[a])):
                    findings.append(f"{name}:{block[i][0]}: a git clone in a retry loop does not remove its partial "
                                    "clone first (`until rm -rf <dir> && ... git clone ... <dir>; do`)")
            bounds = [bound for _i, _what, bound in inside]
            tries.append((block[a][0], sum(bounds) if bounds and None not in bounds else None))
            prev_end = b
        for i, what, _bound in fetches:
            if not any(a <= i <= b for a, b in retry_spans):
                findings.append(f"{name}:{block[i][0]}: {what} outside a policy retry loop")
    return findings, loops, tries


def count_findings(counts: dict[str, int]) -> list[str]:
    """(pure) The per-workflow loop counts against _LOOPS_PINNED (a workflow it does not name pins 0)."""
    return [f"{name}: {counts.get(name, 0)} retry loop(s), the canary pins {_LOOPS_PINNED.get(name, 0)}"
            for name in sorted(set(counts) | set(_LOOPS_PINNED))
            if counts.get(name, 0) != _LOOPS_PINNED.get(name, 0)]


def _wf(body: str) -> str:
    """A minimal workflow whose one step runs `body` (a block scalar, one shell line per body line)."""
    lines = "".join(f"          {ln}\n" for ln in body.strip("\n").split("\n"))
    return ("name: t\non: push\njobs:\n  j:\n    runs-on: ubuntu-22.04\n    steps:\n      - name: s\n"
            "        run: |\n" + lines)


_LOOP = (
    "delay=2 deadline=\n"
    "until timeout -k 10 60 git fetch origin main; do\n"
    "  deadline=${deadline:-$((SECONDS + 300))}\n"
    "  left=$((deadline - SECONDS))\n"
    '  if [ "$left" -le 0 ]; then echo "::error::fetch failed through its retry window"; exit 1; fi\n'
    "  pause=$((delay / 2 + RANDOM % (delay / 2 + 1)))\n"
    '  if [ "$pause" -gt "$left" ]; then pause=$left; fi\n'
    '  echo "::warning::fetch failed; retrying in ${pause}s"\n'
    '  sleep "$pause"\n'
    "  delay=$((delay * 2 < 60 ? delay * 2 : 60))\n"
    "done\n"
)
_SPENT_LINE = '  if [ "$left" -le 0 ]; then echo "::error::fetch failed through its retry window"; exit 1; fi\n'
_WARN_LINE = '  echo "::warning::fetch failed; retrying in ${pause}s"\n'
_SLEEP_LINE = '  sleep "$pause"\n'
_RESET_LINE = "delay=2 deadline=\n"
_DOUBLING_LINE = "  delay=$((delay * 2 < 60 ? delay * 2 : 60))\n"
_FETCH = "timeout -k 10 60 git fetch origin main"
_OPENER = f"until {_FETCH}; do\n"
_PREAMBLE = (
    "  rc=0\n"
    '  sudo timeout -k 10 120 apt-get "${apt_opts[@]}" update || rc=$?\n'
    '  if [ "$rc" -eq 0 ]; then\n'
    '    sudo timeout -k 10 300 apt-get "${apt_opts[@]}" install -y --download-only "${pkgs[@]}" || rc=$?\n'
    "  fi\n"
    f"  {_SUCCESS_BREAK}\n"
)
_WLOOP = _LOOP.replace(_OPENER, f"{_OPEN_WHILE}\n{_PREAMBLE}")
_SETUP = (
    "apt_opts=(-o Acquire::Retries=1 -o Acquire::http::Timeout=15)\n"
    "pkgs=(\n"
    "  libgtk-3-dev\n"
    "  g++ pkg-config\n"
    ")\n"
    'db="${CARGO_HOME:-$HOME/.cargo}/advisory-db"\n'
)
_OFFLINE = 'sudo apt-get install -y --no-download "${pkgs[@]}"\n'


def _until(cond: str) -> str:
    """The canonical loop with `cond` as its `until` condition."""
    return _LOOP.replace(_OPENER, f"until {cond}; do\n")


# (label, run: body, the finding the reader must produce - None: the body reads clean)
_PLANTED = (
    ("the canonical loop", _LOOP, None),
    ("a fixed `sleep 5` beside the loop", _LOOP + "sleep 5\n", "sleeps outside the policy schedule"),
    ("a 120 s window", _LOOP.replace("SECONDS + 300", "SECONDS + 120"), "lacks the policy line(s) deadline="),
    ("a 64 s cap", _LOOP.replace("? delay * 2 : 60", "? delay * 2 : 64").replace("< 60", "< 64"),
     "lacks the policy line(s) delay=$((delay * 2 < 60"),
    ("a full-range jitter", _LOOP.replace("delay / 2 + RANDOM % (delay / 2 + 1)", "RANDOM % (delay + 1)"),
     "lacks the policy line(s) pause="),
    ("a loop without its spent check", _LOOP.replace('if [ "$left" -le 0 ]', 'if [ "$left" -lt 0 ]'), _SPENT_SHAPE),
    ("a spent branch that exits 0", _LOOP.replace('window"; exit 1; fi', 'window"; exit 0; fi'), _SPENT_SHAPE),
    ("a spent branch without its exit", _LOOP.replace('window"; exit 1; fi', 'window"; fi'), _SPENT_SHAPE),
    ("a spent branch that breaks out of the loop", _LOOP.replace('window"; exit 1; fi', 'window"; break; fi'),
     _SPENT_SHAPE),
    ("a spent branch whose message runs an arithmetic assignment (`$((SECONDS = 0))`)",
     _LOOP.replace("its retry window", "its retry window $((SECONDS = 0))"), _SPENT_SHAPE),
    ("an exit-0 decoy ahead of the spent branch",
     _LOOP.replace(_SPENT_LINE, _SPENT_LINE.replace("exit 1", "exit 0") + _SPENT_LINE),
     "exits the loop outside its spent branch"),
    ("a `break` on the window ahead of the spent branch",
     _LOOP.replace(_SPENT_LINE, '  [ "$left" -le 0 ] && break\n' + _SPENT_LINE),
     "reads the retry window outside the spent branch and the pause cut"),
    ("the spent branch after the sleep", _LOOP.replace(_SPENT_LINE, "").replace(_SLEEP_LINE, _SLEEP_LINE + _SPENT_LINE),
     "runs its schedule out of order"),
    ("the schedule lines without their loop",
     _LOOP.replace("until timeout -k 10 60 git fetch origin main; do\n", "").replace("done\n", ""),
     "outside an `until`/`while` ... `done` loop"),
    ("two loops in one run: value, each with its own reset and lines", _LOOP + _LOOP, None),
    ("a second loop that borrows the first one's reset", _LOOP + _LOOP.replace(_RESET_LINE, ""),
     f"{_RESET_LINE.strip()} (before the loop)"),
    ("a second loop that borrows the first one's spent branch", _LOOP + _LOOP.replace(_SPENT_LINE, ""), _SPENT_SHAPE),
    ("the old three fixed tries", 'for i in 1 2 3; do\n  git clone https://x/y && exit 0\n  sleep 5\ndone\nexit 1\n',
     "sleeps outside the policy schedule"),
    ("an unwrapped `cargo fetch --locked`", "cargo fetch --locked\n", "a cargo fetch outside a policy retry loop"),
    ("a fetch before the loop in the same run: value", "git fetch origin main\n" + _LOOP,
     "a git fetch outside a policy retry loop"),
    ("a fetch after the loop in the same run: value", _LOOP + "git clone https://x/y\n",
     "a git clone outside a policy retry loop"),
    ("an unwrapped apt-get refresh", "sudo timeout -k 10 120 apt-get update\n",
     "an apt download outside a policy retry loop"),
    ("an unwrapped `apt` download", "sudo timeout -k 10 120 apt install -y --download-only x\n",
     "an apt download outside a policy retry loop"),
    ("an unwrapped `aptitude` install", "sudo timeout -k 10 300 aptitude install -y x\n",
     "an apt download outside a policy retry loop"),
    ("an unwrapped `apt-fast` download by path", "sudo timeout -k 10 300 /usr/local/bin/apt-fast install -y x\n",
     "an apt download outside a policy retry loop"),
    ("an unwrapped `git -C <dir> fetch`", 'git -C "$db" fetch origin\n', "a git fetch outside a policy retry loop"),
    ("an unwrapped `git -c <k=v> clone`", "git -c http.lowSpeedLimit=1000 clone https://x/y\n",
     "a git clone outside a policy retry loop"),
    ("an unwrapped `git --git-dir=<d> pull`", "/usr/bin/git --git-dir=x/.git pull\n",
     "a git pull outside a policy retry loop"),
    ("an unwrapped `cargo +<toolchain> fetch`", "cargo +nightly-2026-05-20 fetch\n",
     "a cargo fetch outside a policy retry loop"),
    ("an unwrapped `cargo -Z <flag> --locked fetch`", "cargo -Z unstable-options --locked fetch\n",
     "a cargo fetch outside a policy retry loop"),
    ("a fetch continued over two lines outside the loop", "git -C x \\\n  fetch origin\n",
     "a git fetch outside a policy retry loop"),
    ("the offline `--no-download` install alone", "sudo apt-get install -y --no-download x\n", None),
    ("a quoted 'sleep' and 'git fetch' in an echo", 'echo "sleep 5; git fetch later"\n', None),
    ("`git` and `cargo` subcommands that fetch nothing", "git -C x rev-parse HEAD\ncargo metadata --locked\n", None),
    ("an `until` condition that exits 0 on a failed fetch (`|| exit 0`)", _until(f"{_FETCH} || exit 0"),
     "exits the loop outside its spent branch"),
    ("an `until` condition that reads the window", _until(f'{_FETCH} "$left"'),
     "reads the retry window outside the spent branch and the pause cut"),
    ("an `until` loop that runs no fetch (only the offline install)", _until("sudo apt-get install -y --no-download x"),
     "the retry loop runs no fetch"),
    ("the canonical `while :` loop", _WLOOP, None),
    ("a `while` loop that never runs (`while false`)", _WLOOP.replace(_OPEN_WHILE, "while false; do"),
     "opens the retry loop"),
    ("a `while :` loop that breaks without its rc guard", _WLOOP.replace(f"  {_SUCCESS_BREAK}\n", "  break\n"),
     "has no success break"),
    ("a `break` first in an `until` loop's body", _LOOP.replace(_OPENER, f"{_OPENER}  break\n"),
     "its line 1 is `break`"),
    ("a `break` in place of the `::warning::` line", _LOOP.replace(_WARN_LINE, "  break\n"), "its line 6 is `break`"),
    ("a `::warning::` line that runs a command (`$(...)`)",
     _LOOP.replace(_WARN_LINE, '  echo "::warning::fetch failed $(sudo ln -sf /bin/true /usr/bin/git)"\n'),
     "its line 6 is `echo"),
    ("a `::warning::` line with a `$[...]` arithmetic expansion",
     _LOOP.replace(_WARN_LINE, '  echo "::warning::fetch failed $[SECONDS=-1000]"\n'), "its line 6 is `echo"),
    ("the doubling line twice", _LOOP.replace(_DOUBLING_LINE, _DOUBLING_LINE * 2), "follows its last line"),
    ("a line between the schedule reset and the opener", _LOOP.replace(_RESET_LINE, f"{_RESET_LINE}delay=60\n"),
     "stands between the loop's schedule reset and its opener"),
    ("a retry step's setup, `while :` loop and offline install", _SETUP + _WLOOP + _OFFLINE, None),
    ("an unwrapped `curl` download", "curl -fsSL --retry 2 https://x/y.tgz -o y.tgz\n",
     "a curl download outside a policy retry loop"),
    ("an unwrapped `wget` download by path", "/usr/bin/wget -q https://x/y.tgz\n",
     "a wget download outside a policy retry loop"),
    ("an unwrapped `git ls-remote`", "git ls-remote https://x/y HEAD\n", "a git ls-remote outside a policy retry loop"),
    ("a `curl` download in a retry loop under its `timeout`", _until("timeout -k 10 60 curl -fsSL https://x/y -o y"),
     None),
    ("a git fetch in a retry loop without its `timeout`", _until("git fetch origin main"),
     "a git fetch in a retry loop runs under no coreutils `timeout`"),
    ("an apt download whose `timeout` stands before `sudo`",
     _WLOOP.replace('sudo timeout -k 10 120 apt-get "${apt_opts[@]}" update',
                    'timeout -k 10 120 sudo apt-get "${apt_opts[@]}" update'),
     "an apt download in a retry loop runs under no coreutils `timeout`"),
    ("a `timeout 0` (no bound) on a git fetch in a retry loop", _until("timeout 0 git fetch origin main"),
     "a git fetch in a retry loop runs under no coreutils `timeout`"),
    ("a clone retry loop that keeps the partial clone of a failed try",
     _until('timeout -k 10 120 git clone --depth 1 https://x/y "$db"'),
     "a git clone in a retry loop does not remove its partial clone first"),
    ("a clone retry loop that removes another directory",
     _until('rm -rf "$other" && timeout -k 10 120 git clone --depth 1 https://x/y "$db"'),
     "a git clone in a retry loop does not remove its partial clone first"),
    ("the clone retry loop that removes its partial clone first",
     _until('rm -rf "$db" && timeout -k 10 120 git clone --depth 1 https://x/y "$db"'), None),
    ("a `cargo fetch` retry loop (cargo's tries carry no coreutils bound)", _until("cargo fetch --locked"), None),
)
for label, body, needle in _PLANTED:
    f, _n = loop_findings("t.yml", _wf(body))
    record(f"planted: {label} -> {'flagged' if needle else 'clean'}",
           needle in "; ".join(f) if needle else not f, "; ".join(f) or "no finding")

with tempfile.TemporaryDirectory() as _td:
    for _name in ("a.yml", "b.yaml", "c.txt", "d.yml.bak"):
        (Path(_td) / _name).write_text("name: x\n", encoding="utf-8")
    _picked = [p.name for p in _workflow_files(Path(_td))]
record("the workflow files read are G56's: every *.yml, then every *.yaml", _picked == ["a.yml", "b.yaml"],
       ", ".join(_picked))
record("the loop-count pin: the pinned counts read clean, and a loop lost, a loop added and a loop in an unpinned "
       "workflow each red",
       not count_findings({"ci.yml": 5, "fuzz.yml": 1, "release.yml": 1, "scorecard.yml": 0})
       and count_findings({"ci.yml": 4, "fuzz.yml": 1, "release.yml": 1}) == ["ci.yml: 4 retry loop(s), the canary pins 5"]
       and count_findings({"ci.yml": 5, "fuzz.yml": 2, "release.yml": 1}) == ["fuzz.yml: 2 retry loop(s), the canary pins 1"]
       and count_findings({"ci.yml": 5, "fuzz.yml": 1, "release.yml": 1, "new.yaml": 1})
       == ["new.yaml: 1 retry loop(s), the canary pins 0"])

_LIVE_FINDINGS: list[str] = []
_LIVE_LOOPS: dict[str, int] = {}
for _wf_path in _workflow_files(WORKFLOWS):
    _f, _n = loop_findings(_wf_path.name, _wf_path.read_text(encoding="utf-8"))
    _LIVE_FINDINGS += _f
    _LIVE_LOOPS[_wf_path.name] = _n
_LIVE_FINDINGS += count_findings(_LIVE_LOOPS)
record("the live workflows: every retry loop runs the policy schedule line for line after its opener and fails its "
       "step (`exit 1`) once its window is spent, no run: line sleeps a fixed literal, every apt download, git "
       "clone, fetch, pull or ls-remote, curl or wget download and cargo fetch sits inside a loop, every try but a "
       "cargo fetch under a coreutils `timeout`, the clone loop removes its partial clone first, and each workflow "
       "holds its pinned loop count",
       not _LIVE_FINDINGS,
       "; ".join(_LIVE_FINDINGS) or ", ".join(f"{k}: {v} loop(s)" for k, v in _LIVE_LOOPS.items() if v))

# --- 3. the attempt bounds and the retry jobs' timeouts --------------------------------------------------------
# Every per-attempt bound of the policy's users and the policy's own kill grace, by (file, constant), as its
# module-level assignment reads (_constants: numbers, arithmetic over the constants before it, max() and min(); the
# file is parsed, never run). A bound that moves reds here until its new value is pinned, and the timeouts below
# follow it. The bounds were sized from Linux measurements (each file's comment states its own); the Windows and
# macOS runners were not measured.
_IGT, _CSC, _FEA = "scripts/install-gate-tools", "scripts/check-supply-chain", "scripts/fetch-engine-assets"
_CANARY_IGT = "scripts/gate-selftests/g24-install-gate-tools.py"
_BOUNDS = {
    (_IGT, "DOWNLOAD_STALL_S"): 60, (_IGT, "DOWNLOAD_OPEN_S"): 60, (_IGT, "DOWNLOAD_FLOOR_BPS"): 131072,
    (_IGT, "DOWNLOAD_CAP_BYTES"): 67108864, (_IGT, "DOWNLOAD_ATTEMPT_S"): 632,
    (_CSC, "_DENY_FETCH_ATTEMPT_S"): 120, (_CSC, "_LS_REMOTE_ATTEMPT_S"): 60, (_CSC, "_MERGE_BASE_S"): 60,
    (_CSC, "_CARGO_FETCH_ATTEMPT_S"): 300, (_CSC, "_DENY_CHECK_S"): 120,
    ("scripts/check-branch-protection", "_GH_ATTEMPT_TIMEOUT_S"): 60,
    ("scripts/check-release-tag-trust", "_GH_ATTEMPT_TIMEOUT_S"): 60,
    (_FEA, "FETCH_TIMEOUT_SECONDS"): 120, (_FEA, "FETCH_ATTEMPT_SECONDS"): 600,
    ("scripts/record-action-pins", "TIMEOUT_S"): 30, ("scripts/record-action-pins", "ATTEMPT_S"): 60,
    ("scripts/check-js-advisories", "REFRESH_TIMEOUT"): 900,
    (_CANARY_IGT, "PROBE_TIMEOUT_S"): 20, (_CANARY_IGT, "PROBE_ATTEMPT_S"): 60,
    (_CANARY_IGT, "ASSET_TIMEOUT_S"): 60, (_CANARY_IGT, "ASSET_ATTEMPT_S"): 120,
    ("scripts/net-retry", "KILL_GRACE_S"): 5,
}
# Each retried fetch a workflow's run: line starts: the bounds one attempt of it runs in sequence (its try bound is
# their sum), whether it retries inside the window (its worst case: the first try at its bound, the 300 s window,
# the last try at its bound) or runs once (the G17 OSV refresh, fail-open with one attempt: its bound alone), and
# whether its steps are subprocesses under net-retry's run_bounded, each of which may outlast its bound by
# KILL_GRACE_S (SIGTERM, then SIGKILL once the grace has passed).
_FETCH_SITES = {
    "the gate-tool install": (((_IGT, "DOWNLOAD_ATTEMPT_S"),), True, False),
    "G18's advisory fetch": (((_CSC, "_DENY_FETCH_ATTEMPT_S"), (_CSC, "_LS_REMOTE_ATTEMPT_S"),
                              (_CSC, "_MERGE_BASE_S")), True, True),
    "G18's crate fetch": (((_CSC, "_CARGO_FETCH_ATTEMPT_S"),), True, True),
    "a G56a gh read": ((("scripts/check-branch-protection", "_GH_ATTEMPT_TIMEOUT_S"),), True, True),
    "a G56b gh read": ((("scripts/check-release-tag-trust", "_GH_ATTEMPT_TIMEOUT_S"),), True, True),
    "an engine-asset fetch": (((_FEA, "FETCH_ATTEMPT_SECONDS"), (_FEA, "FETCH_TIMEOUT_SECONDS")), True, False),
    "the OSV database refresh": ((("scripts/check-js-advisories", "REFRESH_TIMEOUT"),), False, False),
    "the canary's network probe": (((_CANARY_IGT, "PROBE_ATTEMPT_S"),), True, False),
    "the canary's reference fetch": (((_CANARY_IGT, "ASSET_ATTEMPT_S"),), True, False),
}
# The scripts whose run starts those fetches (matched over a run: line with its quoted text blanked); the canary
# runner runs the install-gate-tools and supply-chain canaries, whose network legs make the fetches named.
_SITE_SCRIPTS = (
    (re.compile(r"\bscripts/install-gate-tools\b"), ("the gate-tool install",)),
    (re.compile(r"\bscripts/check-supply-chain\b"), ("G18's advisory fetch", "G18's crate fetch")),
    (re.compile(r"\bscripts/run-gate-selftests\b"), ("the gate-tool install", "G18's advisory fetch",
                                                    "G18's crate fetch", "the canary's network probe",
                                                    "the canary's reference fetch")),
    (re.compile(r"\bscripts/check-branch-protection\b"), ("a G56a gh read",)),
    (re.compile(r"\bscripts/check-release-tag-trust\b"), ("a G56b gh read",)),
    (re.compile(r"\bscripts/fetch-engine-assets\b(?!.*--check\b)"), ("an engine-asset fetch",)),
    (re.compile(r"\bscripts/check-js-advisories\b.*--refresh\b"), ("the OSV database refresh",)),
)
# Each job that runs a retried fetch, with the timeout-minutes its run with no outage had before the policy (its
# budget): its timeout-minutes is that budget plus its slowest retried fetch's worst case, rounded up to the minute.
_JOB_BUDGET_MIN = {
    ("ci.yml", "gate-tooling"): 30, ("ci.yml", "workflow-security"): 10, ("ci.yml", "sast"): 15,
    ("ci.yml", "compile-sanity"): 25, ("ci.yml", "engine-asset-populate"): 30, ("ci.yml", "engine-asset-compile"): 45,
    ("fuzz.yml", "fuzz"): 80, ("release.yml", "guard"): 10, ("secrets-history.yml", "gitleaks-history"): 15,
}
_JOB_HEAD_RE = re.compile(r"^  ([A-Za-z0-9_-]+):\s*(?:#.*)?$")
_JOB_TIMEOUT_RE = re.compile(r"^    timeout-minutes:\s*(\d+)\b")


def _constants(source: str) -> dict[str, float]:
    """(pure) The module-level numeric constants of a Python source, by its syntax tree (never run): an assignment
    (a tuple of them too) of a number, of arithmetic (+ - * / //) over numbers and the constants before it, or of max()
    or min() over those; any other value is left out."""
    values: dict[str, float] = {}
    ops = {ast.Add: lambda a, b: a + b, ast.Sub: lambda a, b: a - b, ast.Mult: lambda a, b: a * b,
           ast.Div: lambda a, b: a / b if b else None, ast.FloorDiv: lambda a, b: a // b if b else None}

    def ev(node: ast.AST) -> float | None:
        if isinstance(node, ast.Constant) and isinstance(node.value, (int, float)) and not isinstance(node.value, bool):
            return node.value
        if isinstance(node, ast.Name):
            return values.get(node.id)
        if isinstance(node, ast.BinOp) and type(node.op) in ops:
            a, b = ev(node.left), ev(node.right)
            return None if a is None or b is None else ops[type(node.op)](a, b)
        if (isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id in ("max", "min")
                and node.args and not node.keywords):
            args = [ev(arg) for arg in node.args]
            return None if None in args else (max if node.func.id == "max" else min)(args)
        return None

    for stmt in ast.parse(source).body:
        if not isinstance(stmt, ast.Assign):
            continue
        for target in stmt.targets:
            pairs = ([(target, stmt.value)] if isinstance(target, ast.Name) else
                     list(zip(target.elts, stmt.value.elts))
                     if isinstance(target, ast.Tuple) and isinstance(stmt.value, ast.Tuple)
                     and len(target.elts) == len(stmt.value.elts) else [])
            for name, node in pairs:
                value = ev(node) if isinstance(name, ast.Name) else None
                if value is not None:
                    values[name.id] = value
    return values


def bound_values(repo: Path) -> dict[tuple[str, str], float | None]:
    """Each pinned bound's value as its file assigns it (None: the file or the constant is gone)."""
    by_file: dict[str, dict[str, float]] = {}
    for rel, _name in _BOUNDS:
        if rel not in by_file:
            path = repo / rel
            by_file[rel] = _constants(path.read_text(encoding="utf-8")) if path.is_file() else {}
    return {(rel, name): by_file[rel].get(name) for rel, name in _BOUNDS}


def _site_worst(site: str, values: dict[tuple[str, str], float | None]) -> float:
    parts, windowed, subprocesses = _FETCH_SITES[site]
    grace = (values[("scripts/net-retry", "KILL_GRACE_S")] or 0) if subprocesses else 0
    bound = sum((values[part] or 0) + grace for part in parts)
    return 2 * bound + NET.WINDOW_S if windowed else bound


def _job_spans(lines: list[str]) -> list[tuple[str, int, int, int | None]]:
    """(job id, its first and last 1-based line, its job-level timeout-minutes or None) for each job of a workflow in
    G56's dialect: block-style jobs two spaces under the top-level `jobs:`, their keys four."""
    start = next((i for i, ln in enumerate(lines) if ln.rstrip() == "jobs:"), None)
    if start is None:
        return []
    end = next((i for i in range(start + 1, len(lines)) if lines[i][:1] not in ("", " ", "#")), len(lines))
    heads = [(m.group(1), i) for i in range(start + 1, end) if (m := _JOB_HEAD_RE.match(lines[i]))]
    spans: list[tuple[str, int, int, int | None]] = []
    for n, (jid, i) in enumerate(heads):
        last = heads[n + 1][1] - 1 if n + 1 < len(heads) else end - 1
        timeout = next((int(m.group(1)) for k in range(i + 1, last + 1) if (m := _JOB_TIMEOUT_RE.match(lines[k]))),
                       None)
        spans.append((jid, i + 1, last + 1, timeout))
    return spans


def job_timeout_findings(name: str, text: str, values: dict[tuple[str, str], float | None],
                         budgets: dict[tuple[str, str], int] = _JOB_BUDGET_MIN) -> tuple[list[str], dict[str, int]]:
    """(findings, each budgeted job's computed timeout-minutes) for one workflow's text: every job that runs a
    retried fetch - a retry loop with a try bound (loop_tries) or a run: line of a _SITE_SCRIPTS script - is budgeted
    and carries its budget plus its slowest retried fetch's worst case in whole minutes, rounded up; every budgeted
    job exists and runs one."""
    lines = text.split("\n")
    tries = loop_tries(text)
    runs = [(no, GATE._blank_shell_words(line)) for no, line in GATE.run_values(lines)]
    findings: list[str] = []
    computed: dict[str, int] = {}
    spans = _job_spans(lines)
    for jid, first, last, timeout in spans:
        worst = [(f"the retry loop at line {no} ({bound:g} s a try)", 2 * bound + NET.WINDOW_S)
                 for no, bound in tries if first <= no <= last and bound is not None]
        worst += [(site, _site_worst(site, values)) for no, blank in runs if first <= no <= last
                  for pattern, sites in _SITE_SCRIPTS if pattern.search(blank) for site in sites]
        budget = budgets.get((name, jid))
        if not worst:
            if budget is not None:
                findings.append(f"{name}: job '{jid}' is budgeted but runs no retried fetch")
            continue
        if budget is None:
            findings.append(f"{name}: job '{jid}' runs a retried fetch ({worst[0][0]}) but has no budget in "
                            "_JOB_BUDGET_MIN")
            continue
        what, seconds = max(worst, key=lambda w: w[1])
        computed[jid] = budget + math.ceil(seconds / 60)
        if timeout != computed[jid]:
            findings.append(f"{name}: job '{jid}' carries timeout-minutes {timeout}, computed {computed[jid]}: its "
                            f"{budget} min budget plus {seconds:g} s for its slowest retried fetch ({what})")
    findings += [f"{name}: the budgeted job '{jid}' is gone" for wf, jid in budgets
                 if wf == name and jid not in {span[0] for span in spans}]
    return findings, computed


_LIVE_VALUES = bound_values(REPO)
_drift = [f"{rel} {const}: {_LIVE_VALUES[(rel, const)]} (pinned {want})" for (rel, const), want in _BOUNDS.items()
          if _LIVE_VALUES[(rel, const)] != want]
record("the attempt bounds: every user's per-attempt bound is the value the canary pins (the installer's stall, "
       "request, floor-rate and size cap and its attempt bound; G18's five steps; the gh reads; the engine-asset, "
       "action-pin and OSV fetches; the canary's own probes; run_bounded's 5 s kill grace)", not _drift,
       "; ".join(_drift) or f"{len(_BOUNDS)} bounds")
record("_constants: numbers, arithmetic over the constants before them, max() and min() and a tuple assignment are "
       "read; a call it does not know and a constant used before its assignment are left out",
       _constants("A = 2\nB, C = 3, 4.5\nD = A * 128 + max(B, C) - 1 // 2\nE = int('7')\nF = G + 1\nG = 1\n")
       == {"A": 2, "B": 3, "C": 4.5, "D": 260.5, "G": 1})
_TIMED_WF = ("name: t\non: push\njobs:\n  j:\n    runs-on: ubuntu-22.04\n    timeout-minutes: @T@\n    steps:\n"
             "      - name: s\n        run: |\n" + "".join(f"          {ln}\n" for ln in _LOOP.strip("\n").split("\n"))
             + "  k:\n    runs-on: ubuntu-22.04\n    timeout-minutes: @U@\n    steps:\n"
               "      - run: python3 -P scripts/check-branch-protection --enforce\n"
               "      - run: echo 'python3 -P scripts/install-gate-tools'\n")


def _timed_wf(t: int, u: int) -> str:
    """The planted two-job workflow: job j a canonical 70 s-try git fetch loop, job k a G56a read (and a quoted script
    name that starts nothing), with timeout-minutes `t` and `u`."""
    return _TIMED_WF.replace("@T@", str(t)).replace("@U@", str(u))


_budgets = {("t.yml", "j"): 10, ("t.yml", "k"): 10}
_f, _c = job_timeout_findings("t.yml", _timed_wf(18, 18), _LIVE_VALUES, _budgets)
record("job timeouts: a job's computed timeout is its budget plus its slowest retried fetch's worst case rounded up "
       "to the minute (a 70 s-try loop: 10 + 440 s -> 18; a G56a read, a run_bounded step: 10 + 2 x (60 + 5) + 300 s "
       "-> 18), a quoted script name no fetch", not _f and _c == {"j": 18, "k": 18}, "; ".join(_f) or str(_c))
_f, _c = job_timeout_findings("t.yml", _timed_wf(17, 18), _LIVE_VALUES, _budgets)
record("job timeouts: a job whose timeout-minutes is below its computed one reds",
       any("'j' carries timeout-minutes 17, computed 18" in f for f in _f), "; ".join(_f))
_f, _c = job_timeout_findings("t.yml", _timed_wf(19, 18), _LIVE_VALUES, _budgets)
record("job timeouts: a job whose timeout-minutes is above its computed one reds too (each carries exactly its own)",
       any("'j' carries timeout-minutes 19, computed 18" in f for f in _f), "; ".join(_f))
_f, _c = job_timeout_findings("t.yml", _timed_wf(18, 18), _LIVE_VALUES, {("t.yml", "j"): 10, ("t.yml", "gone"): 5})
record("job timeouts: a retry job without a budget and a budgeted job that is gone each red",
       any("'k' runs a retried fetch" in f for f in _f) and any("budgeted job 'gone' is gone" in f for f in _f),
       "; ".join(_f))
_grown = {**_LIVE_VALUES, ("scripts/check-branch-protection", "_GH_ATTEMPT_TIMEOUT_S"): 120}
_f, _c = job_timeout_findings("t.yml", _timed_wf(18, 18), _grown, _budgets)
record("job timeouts: a bound that grows raises the computed timeout, and the job that kept its old one reds",
       any("'k' carries timeout-minutes 18, computed 20" in f for f in _f), "; ".join(_f))
record("loop_tries: a loop's try bound is the sum of its tries' `timeout` durations and kill-after delays (the git "
       "fetch loop 60 + 10 s, the apt loop 120 + 10 + 300 + 10 s); a `cargo fetch` loop has none",
       [b for _n, b in loop_tries(_wf(_LOOP + _WLOOP + _until("cargo fetch --locked")))] == [70.0, 440.0, None])
_JOB_FINDINGS: list[str] = []
_JOB_COMPUTED: dict[str, int] = {}
for _wf_path in _workflow_files(WORKFLOWS):
    _f, _c = job_timeout_findings(_wf_path.name, _wf_path.read_text(encoding="utf-8"), _LIVE_VALUES)
    _JOB_FINDINGS += _f
    _JOB_COMPUTED.update({f"{_wf_path.name}:{k}": v for k, v in _c.items()})
record("the live workflows: every job that runs a retried fetch is budgeted, and its timeout-minutes is its budget "
       "plus its slowest retried fetch's worst case (first try at its bound + 300 s + last try at its bound, a "
       "run_bounded step counted with its 5 s kill grace)",
       not _JOB_FINDINGS and len(_JOB_COMPUTED) == len(_JOB_BUDGET_MIN),
       "; ".join(_JOB_FINDINGS) or ", ".join(f"{k} {v} min" for k, v in sorted(_JOB_COMPUTED.items())))

faulthandler.cancel_dump_traceback_later()
failed = [n for n, ok in results if not ok]
print(f"\n[g24-net-retry] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
