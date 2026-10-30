#!/usr/bin/env python3
"""g24-install-gate-tools - G24 self-test for the pinned-tool fetch+verify control (P0.2.1).

Proves the security control BOTH ways (build-gates s0 / G24): a WRONG checksum
MUST fail the install; a CORRECT checksum MUST pass; an off-origin url MUST be
rejected; --offline with no prior install MUST fail; and the pip --require-hashes
leg MUST reject a hashless requirement. The hermetic legs (no network) always run;
the three network legs (wrong checksum, correct checksum, idempotent re-install)
fetch a tiny (~KB) real asset under the lefthook origin and, when offline,
SKIP-as-PASS by default OR FAIL under --require-network (which CI passes, so an
offline CI run cannot vacuously pass the checksum legs). This script's own fetches
and the installer's downloads run under the gate plane's one network retry policy
(scripts/net-retry, build-gates §0): a transient failure is retried for the retry
window, a final one (a 4xx) is not. Online, a leg SKIPs only when its own download
failed through its whole retry window (a release-asset outage); a retried attempt
the download recovered from, a final failure (a 4xx), a sha mismatch and any
failure after the download are real outcomes (on the correct pin, a FAIL), and a
reference asset refused with a 4xx FAILs all three legs. The three legs' first
installs start together, each its own process with its own retry window, so an
outage costs one window of wall time, not one per leg, while every leg is still
judged on its own run. The hermetic retry legs pin the installer's schedule with a
fake clock: the pauses, the window, the jitter range, the final failures, the
spent run, a 302 to the CDN that rides the whole window (a request per attempt), a
local write error that is final at once, a request ended at its bound (its late
response closed unread), and in real time, on scaled-down bounds, a stalled transfer
ended at the socket timeout, a slow body above the floor rate completing at two
sizes, and a body still arriving at its deadline ended there with no byte read
after; the hermetic decision legs pin the reference fetch's ok / outage / refusal
split (behind a CDN redirect too), the four-way network decision and each install
leg's verdict on its own run. The pip leg resolves its hashless requirement against the package index
before pip reports the missing hash, so it needs the network as well: offline it
FAILs, and it SKIPs only when pip is absent or outruns its 120 s timeout. The
hermetic legs mock the network only inside context managers and drive every clock
through a Retrier of their own (the installer copy's, or one passed to the
probes), and a leak-guard leg proves the network legs start from the process state
the script started with (a urlopen double that outlived its leg made real_sha()
hash the double's payload, so the correct-checksum and idempotency legs SKIPped on
every host).

Run:  python3 scripts/gate-selftests/g24-install-gate-tools.py
Exit: 0 = every assertion held; 1 = a self-test assertion FAILED (the gate is broken).
"""
import argparse
import contextlib
import hashlib
import http.client
import importlib.machinery
import importlib.util
import io
import os
import random
import subprocess
import sys
import tempfile
import threading
import time
import types
import urllib.error
import urllib.request
import urllib.response
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Callable
from unittest import mock
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

REPO = Path(__file__).resolve().parents[2]
INSTALLER = REPO / "scripts" / "install-gate-tools"
ORIGIN = "https://github.com/evilmartians/lefthook/releases/download/v2.1.9/"
SMALL_ASSET = ORIGIN + "lefthook_checksums.txt"  # tiny, stable real asset under the origin
# A RELIABLE endpoint to confirm the CI HAS NETWORK, SEPARATE from SMALL_ASSET's availability: GitHub's
# release-download CDN can 5xx an asset independently of github.com being up (the shellcheck-502 class).
# online() probes THIS for the --require-network offline check; an install leg skips-as-pass only when its
# asset DOWNLOAD failed through its retry window (_download_failed) - so a GitHub asset outage does not redden
# main for the wrong reason, while a genuinely-offline CI still fails. The reds of the 8c6fa67 and 8b5a8e0
# pushes, once read as a GitHub transient and a GitHub asset incident, were the retry_leg mock leak: both
# self-tests replay the same reds with GitHub healthy.
ONLINE_PROBE = "https://github.com/"
PLATFORM_KEYS = ("linux-x86_64", "linux-aarch64", "macos-x86_64", "macos-aarch64", "windows-x86_64")

# The gate plane's one network retry policy, compiled from its source into a fresh module as the installer does
# (no scripts/ module is importable under -P, and no bytecode cache is read or written). This process's own
# fetches - online() and real_sha() - share one Retrier, so an outage spends one retry window here.
_NET_PATH = REPO / "scripts" / "net-retry"
NET = types.ModuleType("net_retry")
exec(compile(_NET_PATH.read_bytes(), str(_NET_PATH), "exec"), NET.__dict__)
_RETRY = NET.Retrier("g24-install-gate-tools")

results: list[tuple[str, bool, str]] = []

_ap = argparse.ArgumentParser(description="G24 self-test for install-gate-tools")
_ap.add_argument("--require-network", action="store_true",
                 help="turn an offline SKIP of the network legs into a FAIL (CI passes this)")
args = _ap.parse_args()


def record(name: str, ok: bool, detail: str = "") -> None:
    results.append((name, ok, detail))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}{(' - ' + detail) if detail else ''}")


def manifest(url: str, sha256: str) -> str:
    rows = "".join(
        f'{k} = {{ url = "{url}", sha256 = "{sha256}" }}\n' for k in PLATFORM_KEYS
    )
    return (
        "schema_version = 1\n"
        "[tools.selftest]\n"
        'version = "0"\n'
        f'origin = "{ORIGIN}"\n'
        'bin_name = "selftest-asset"\n'
        'asset_kind = "raw"\n'
        'corroboration = "self-test synthetic"\n'
        "[tools.selftest.platforms]\n" + rows
    )


def run_installer(manifest_text: str, *extra: str, dest: Path | None = None) -> tuple[int, str]:
    """One install-gate-tools run on `manifest_text`: (exit code, everything it printed). It installs into `dest`,
    or into a fresh temporary directory when none is given."""
    with tempfile.TemporaryDirectory() as td:
        man = Path(td) / "manifest.toml"
        man.write_text(manifest_text, encoding="utf-8")
        cmd = [sys.executable, str(INSTALLER), "--manifest", str(man),
               "--dest", str(dest if dest is not None else Path(td) / "dest"), *extra]
        p = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
        return p.returncode, p.stdout + p.stderr


def _load_installer(name: str) -> types.ModuleType:
    """A fresh in-process copy of install-gate-tools (no .py extension, so an explicit source loader). Its
    `urllib` globals are THIS process's modules - a mock set through them is process-wide. Its Retrier
    (`_RETRY`) is the copy's own object, so a leg replaces it with a fake-clock one and nothing leaks."""
    loader = importlib.machinery.SourceFileLoader(name, str(INSTALLER))
    igt = importlib.util.module_from_spec(importlib.util.spec_from_loader(name, loader))
    loader.exec_module(igt)
    return igt


class _Resp(io.BytesIO):
    """A urlopen() response double (a context manager over fixed bytes)."""

    def __enter__(self):
        return self

    def __exit__(self, *_a):
        self.close()
        return False


# --- the mock-leak guard ---------------------------------------------------------------------------------
# The hermetic legs mock the network through the installer module's `urllib` name, which is this process's own
# module. A mock that outlives its leg is what online() and real_sha() call: online() then reports a network
# even offline, and the installer subprocess's real bytes never match the mock payload's sha - the retry_leg
# incident, where the correct-checksum and idempotency legs routed that mismatch to SKIP on every host (a
# mismatch FAILs them now, _download_failed). _process_state() snapshots every CODE binding of every loaded
# module (its module-level callables and the callables / descriptors in the dicts of the classes it defines)
# plus the environment the installer subprocesses inherit; the leak-guard leg compares it with the state the
# network legs start from.
# [Build-Session-Entscheidung: P0.2.1] code bindings and the environment, never every module global: the stdlib
# rebinds data globals as lazy caches during a leak-free run (measured with every global compared, Linux /
# Python 3.12 and Windows / 3.13: tempfile.tempdir, tempfile._name_sequence, urllib.parse._typeprog and
# urllib.parse._hostprog, plus ntpath._varsub on Windows), while the same runs rebound no function, class or
# class member. A leak through a data binding (urllib.request.install_opener sets _opener) is therefore the
# network legs' own catch: the reference sha it corrupts no longer matches the real asset, and that FAILs.
_ABSENT = object()


def _process_state() -> tuple[dict[tuple[str, str, str], object], dict[str, str]]:
    code: dict[tuple[str, str, str], object] = {}
    for mname, mod in list(sys.modules.items()):
        if not isinstance(mod, types.ModuleType):
            continue
        for name, obj in list(vars(mod).items()):
            if callable(obj):
                code[(mname, name, "")] = obj
            if isinstance(obj, type) and getattr(obj, "__module__", None) == mname:
                for attr, member in list(vars(obj).items()):
                    if callable(member) or isinstance(member, (classmethod, staticmethod, property)):
                        code[(mname, name, attr)] = member
    return code, dict(os.environ)


def _leaked_since(before: tuple[dict[tuple[str, str, str], object], dict[str, str]]) -> list[str]:
    """Every code binding of `before` that no longer holds the same object, plus every changed environment
    variable. A module no longer in sys.modules is skipped: it re-imports fresh, so no mock survives in it."""
    code, env = before
    leaked: list[str] = []
    for (mname, name, attr), obj in code.items():
        mod = sys.modules.get(mname)
        if not isinstance(mod, types.ModuleType):
            continue
        cur = vars(mod).get(name, _ABSENT)
        if attr:
            cur = vars(cur).get(attr, _ABSENT) if isinstance(cur, type) else _ABSENT
        if cur is not obj:
            leaked.append(".".join(p for p in (mname, name, attr) if p))
    now = dict(os.environ)
    leaked += [f"os.environ[{k!r}]" for k in sorted(set(env) | set(now)) if env.get(k) != now.get(k)]
    return leaked


# The probes' bounds: a blocking step of an attempt (the socket timeout) and the whole attempt (its wall clock).
PROBE_TIMEOUT_S, PROBE_ATTEMPT_S = 20, 60
ASSET_TIMEOUT_S, ASSET_ATTEMPT_S = 60, 120


def online(retrier=_RETRY) -> bool:
    """Confirm the CI HAS NETWORK by probing the RELIABLE github.com root (ONLINE_PROBE) under the retry policy
    (`retrier`, this process's own unless a hermetic leg passes a fake-clock one): a transient failure is retried
    for the retry window. This is deliberately NOT the flaky release asset: the asset can 5xx independently
    (handled by _download_failed / real_sha()'s 'unfetchable'). Under --require-network a False here - the probe
    failed through its window, or was refused outright - means no usable network (no vacuous skip of the checksum
    legs). Each attempt builds its own request (urllib counts the redirects it followed on the request object) and
    runs under its wall-clock bound."""

    def attempt() -> None:
        req = urllib.request.Request(ONLINE_PROBE, method="HEAD", headers={"User-Agent": "convertia-selftest"})
        urllib.request.urlopen(req, timeout=PROBE_TIMEOUT_S).close()

    try:
        retrier.run(NET.bounded(attempt, PROBE_ATTEMPT_S), what=ONLINE_PROBE,
                    transient_error=NET.transient_urllib_error)
    except (urllib.error.URLError, http.client.HTTPException, OSError, NET.WindowSpent):
        return False
    return True


def real_sha(retrier=_RETRY) -> tuple[str, str]:
    """('ok', the sha256 of SMALL_ASSET) | ('unfetchable', reason) | ('refused', reason), fetched under the retry
    policy (`retrier`, as online()'s). 'unfetchable' is a failure that stayed transient through the whole window -
    a GitHub 5xx / timeout / connection outage on the asset, infra, so the caller skips-as-pass - or a fetch the
    spent window no longer attempts; 'refused' is a final failure (a 4xx: the asset is gone or forbidden), which the
    caller FAILs - a permanent failure never earns an outage SKIP. Each attempt builds its own request (a release
    asset answers with a redirect to its CDN) and runs under its wall-clock bound."""

    def attempt() -> str:
        req = urllib.request.Request(SMALL_ASSET, headers={"User-Agent": "convertia-selftest"})
        with urllib.request.urlopen(req, timeout=ASSET_TIMEOUT_S) as r:
            return hashlib.sha256(r.read()).hexdigest()

    try:
        return "ok", retrier.run(NET.bounded(attempt, ASSET_ATTEMPT_S), what=SMALL_ASSET,
                                 transient_error=NET.transient_urllib_error)
    except NET.WindowSpent as e:
        return "unfetchable", str(e)
    except (urllib.error.URLError, http.client.HTTPException, OSError) as e:
        return ("unfetchable" if NET.transient_urllib_error(e) is not None else "refused"), str(e)


def _asset_flake(output: str) -> bool:
    """True if an install FAILED because its asset download failed through its whole retry window (a GitHub 5xx
    or connection outage on the asset - the case this SKIP exists for) rather than a real assertion outcome.
    install-gate-tools words that failure 'download failed after its retry window: ...'; a final failure (a 4xx)
    says 'download failed: HTTP Error 404 ...', and a real sha-verification failure says 'sha256 mismatch' -
    neither matches here. A retried attempt's log line ('<url>: attempt N failed (...); retrying in ...') is no
    download failure: download() logs it before the retry that can recover, and what the install reports after a
    recovery is a real outcome. Lets a GitHub asset outage skip-as-pass (infra) while a genuine checksum-control
    defect, or an asset that is gone, still fails."""
    return "download failed after its retry window" in output.lower()


def _download_failed(rc: int, output: str) -> bool:
    """(pure) The one SKIP-as-PASS condition of an install leg: the install exited non-zero, its output names a
    download that failed through its retry window (_asset_flake), and it reports no sha mismatch. Every other
    failure is a real outcome: a mismatch - on the CORRECT pin it FAILs - a final download failure (a 4xx), and a
    failure after the download completed, also after a retried attempt. (The correct-checksum leg used to SKIP on
    a mismatch as "the asset served corrupt bytes"; the reds that SKIP was added for compared the real asset with
    the retry_leg mock payload's sha - the 8b5a8e0 self-test replays them with GitHub healthy.)"""
    return rc != 0 and _asset_flake(output) and "mismatch" not in output.lower()


# The process state every leg starts from: taken before the first leg, compared by the leak-guard leg.
_PRISTINE = _process_state()

# --- hermetic legs (no network) ----------------------------------------------
rc, out = run_installer(manifest("https://evil.example.com/x", "0" * 64))
record("off-origin url is rejected", rc == 1 and "origin" in out.lower(), f"exit={rc}")

rc, out = run_installer(manifest(SMALL_ASSET, "0" * 64), "--offline")
record("offline with no install fails", rc == 1 and "offline" in out.lower(), f"exit={rc}")

# "asserted not-floating": a floating toolchain channel must be rejected.
rc, out = run_installer('schema_version = 1\n[toolchain]\nrust_stable = "stable"\nfuzz_nightly = "nightly"\n')
record("floating toolchain channel is rejected", rc == 1 and "floating" in out.lower(), f"exit={rc}")


def manifest_missing_platform(declare_skip: bool, plant_other_platform: bool = False) -> str:
    """A manifest missing the CURRENT platform key - drives the declared-gap arm
    (`missing_platforms = "skip"`, the cargo-mutants x86_64-only-upstream case,
    P3.72) vs the fail-closed default. `plant_other_platform` pins ONE real row for
    a platform that is guaranteed NOT the current one (the legitimate declared-gap
    shape); without it the platforms table is EMPTY (the dead-pin shape)."""
    skip = 'missing_platforms = "skip"\n' if declare_skip else ""
    rows = ""
    if plant_other_platform:
        igt = _load_installer("igt_probe")
        other = next(k for k in PLATFORM_KEYS if k != igt.detect_platform())
        rows = f'{other} = {{ url = "{SMALL_ASSET}", sha256 = "{"0" * 64}" }}\n'
    return (
        "schema_version = 1\n"
        "[tools.selftest]\n"
        'version = "0"\n'
        f'origin = "{ORIGIN}"\n'
        'bin_name = "selftest-asset"\n'
        'asset_kind = "raw"\n'
        + skip
        + 'corroboration = "self-test synthetic"\n'
        "[tools.selftest.platforms]\n" + rows
    )


# The declared-gap arm ALL ways (hermetic - the skip/raise happens before any download):
# an UNDECLARED missing platform stays FAIL-CLOSED; a DECLARED one with a real foreign
# pin SKIPs visibly (exit 0); a declared skip with an EMPTY table is a DEAD PIN and FAILS.
rc, out = run_installer(manifest_missing_platform(declare_skip=False))
record("undeclared missing platform FAILS (fail-closed default)",
       rc == 1 and "no pin for platform" in out, f"exit={rc}")
rc, out = run_installer(manifest_missing_platform(declare_skip=True, plant_other_platform=True))
record("declared missing_platforms=skip + a foreign pin -> visible SKIP, exit 0",
       rc == 0 and "[SKIP]" in out and "missing_platforms=skip" in out, f"exit={rc}")
rc, out = run_installer(manifest_missing_platform(declare_skip=True))
record("declared skip with an EMPTY platforms table FAILS (dead-pin guard)",
       rc == 1 and "dead pin" in out.lower(), f"exit={rc}")


def archive_leg() -> None:
    """Hermetic: extract_binary round-trips the inner binary from a tar.gz (nested
    member) + a zip (root member), and fails on a missing member - covers the P0.2.4
    archive-extraction code path without network."""
    import tarfile
    import zipfile
    igt = _load_installer("install_gate_tools")
    payload = b"#!/bin/sh\necho fake-tool\n"
    with tempfile.TemporaryDirectory() as td:
        d = Path(td)
        tgz = d / "a.tar.gz"
        with tarfile.open(tgz, "w:gz") as tf:
            info = tarfile.TarInfo("actionlint-1.0/actionlint")
            info.size = len(payload)
            tf.addfile(info, io.BytesIO(payload))
        o1 = d / "o1"
        igt.extract_binary(tgz, "targz", o1, "actionlint")
        record("archive extract tar.gz (nested member)", o1.read_bytes() == payload)
        z = d / "a.zip"
        with zipfile.ZipFile(z, "w") as zf:
            zf.writestr("actionlint.exe", payload)
        o2 = d / "o2.exe"
        igt.extract_binary(z, "zip", o2, "actionlint.exe")
        record("archive extract zip (root member)", o2.read_bytes() == payload)
        missing = False
        try:
            igt.extract_binary(z, "zip", d / "x", "nonexistent")
        except igt.GateToolError:
            missing = True
        record("archive missing member fails", missing)
        # hermetic: sha256_file (the integrity primitive the correct-checksum network leg verifies
        # end-to-end) computes the SAME digest as hashlib over known bytes. Since that network leg
        # skips-as-pass when the asset download fails (and cannot run offline), THIS deterministic check
        # is what keeps a sha256_file defect caught without GitHub.
        blob = d / "blob.bin"
        blob.write_bytes(payload)
        record("sha256_file matches hashlib (the integrity primitive, hermetic)",
               igt.sha256_file(blob) == hashlib.sha256(payload).hexdigest())


archive_leg()


_MOCK_PAYLOAD = b"ok-bytes\n"      # the bytes every urlopen double serves


def _no_sleep(*_a, **_k) -> None:
    """time.sleep double for the incident replay below: no real backoff."""


class _FakeClock:
    """The clock of a fake-clock Retrier: it moves only when the code under test sleeps, or when an urlopen double
    spends time on purpose, so a leg reads the exact pauses and never waits."""

    def __init__(self) -> None:
        self.now = 0.0
        self.pauses: list[float] = []

    def clock(self) -> float:
        return self.now

    def sleep(self, seconds: float) -> None:
        self.pauses.append(seconds)
        self.now += seconds


def _fake_retrier(igt: types.ModuleType, rng: float = 1.0) -> tuple[_FakeClock, list[str]]:
    """Give the installer copy a fake-clock Retrier from its own policy module; `rng` is the jitter draw (1.0 = the
    full nominal pause, 0.0 = its lower half-bound). Returns the clock and the captured log lines."""
    fc, logs = _FakeClock(), []
    igt._RETRY = igt._NET.Retrier("install-gate-tools", clock=fc.clock, sleep=fc.sleep, rng=lambda: rng,
                                  log=logs.append)
    return fc, logs


def _opener(seq, fc: _FakeClock | None = None, stall: float = 0.0):
    """urlopen double that raises each exception in seq in turn, then returns a fresh payload response. With `fc`,
    the first call spends `stall` seconds of the fake clock before it fails."""
    calls = {"n": 0}

    def _open(_req, timeout=None):
        i = calls["n"]
        calls["n"] += 1
        if fc is not None and i == 0:
            fc.now += stall
        if i < len(seq):
            raise seq[i]
        return _Resp(_MOCK_PAYLOAD)

    return _open, calls


class _Paced(io.BytesIO):
    """A urlopen() response double in real time: read1 hands out `step` bytes of `body` per call, `gap` seconds
    apart, and records each call's time; its headers declare `declared` bytes. With `stall_after`, the read after that
    many bytes waits the socket timeout urlopen() was given, then raises TimeoutError - a stalled socket's error. read()
    raises: the installer reads its body with read1, one system call per chunk."""

    def __init__(self, body: bytes, *, step: int, gap: float, declared: int | None = None,
                 stall_after: int | None = None, timeout: float | None = None) -> None:
        super().__init__(body)
        self.headers = {"Content-Length": str(declared)} if declared is not None else {}
        self.step, self.gap, self.stall_after, self.timeout = step, gap, stall_after, timeout
        self.reads: list[float] = []
        self.stalled = False

    def __enter__(self):
        return self

    def __exit__(self, *_a):
        self.close()
        return False

    def read1(self, _size: int = -1) -> bytes:
        self.reads.append(time.monotonic())
        if self.stall_after is not None and self.tell() >= self.stall_after:
            self.stalled = True
            time.sleep(self.timeout or 0)
            raise TimeoutError("timed out")
        if self.gap:
            time.sleep(self.gap)
        return super().read1(self.step)

    def read(self, *_a) -> bytes:
        raise AssertionError("the installer must read its body with read1 (one system call per chunk), never read()")


def _paced_opener(specs: list[dict]):
    """A urlopen double that answers call i with a _Paced response built from specs[i] (the last repeating) and the
    timeout it was given; `calls` counts the calls and keeps each one's timeout, response and start time."""
    calls: dict = {"n": 0, "timeouts": [], "responses": [], "opened": []}

    def _open(_req, timeout=None):
        calls["opened"].append(time.monotonic())
        spec = specs[min(calls["n"], len(specs) - 1)]
        calls["n"] += 1
        calls["timeouts"].append(timeout)
        calls["responses"].append(_Paced(**spec, timeout=timeout))
        return calls["responses"][-1]

    return _open, calls


def _download(igt: types.ModuleType, opener, out: Path) -> BaseException | None:
    """Run igt.download under the urlopen double; the exception it raised, or None."""
    with mock.patch.object(igt.urllib.request, "urlopen", opener):
        try:
            igt.download("https://x/y", out)
        except Exception as e:  # noqa: BLE001 - the leg judges the exception
            return e
    return None


def _http(code: int) -> urllib.error.HTTPError:
    return urllib.error.HTTPError("u", code, "x", {}, None)


def _scripted_reply(url: str, code: int, location: str | None = None) -> urllib.response.addinfourl:
    """An HTTP reply as urllib's handlers read it, without a socket: the status, an empty body and, for a redirect,
    its Location header."""
    headers = http.client.HTTPMessage()
    if location is not None:
        headers["Location"] = location
    reply = urllib.response.addinfourl(io.BytesIO(b""), headers, url, code)
    reply.msg = "scripted"
    return reply


class _RedirectTransport(urllib.request.BaseHandler):
    """An https transport double inside a private urllib opener (its redirect and error handlers): `start` answers
    302 to a Location - the same one on every request, or a new one each time, the way a release asset redirects to
    its CDN - and every Location answers 503, so a fetch rides the 302 into a CDN outage with no socket opened."""

    handler_order = 50

    def __init__(self, start: str, same_location: bool) -> None:
        self.start = start
        self.same_location = same_location
        self.redirects = 0

    def https_open(self, request):
        url = request.full_url
        if url == self.start:
            self.redirects += 1
            hop = "fixed" if self.same_location else str(self.redirects)
            return _scripted_reply(url, 302, f"https://cdn.invalid/asset/{hop}")
        return _scripted_reply(url, 503)


def _urlopen_via(transport: _RedirectTransport):
    """A urlopen double that opens every request through a PRIVATE opener (the transport double, urllib's redirect
    and error handlers) - never `install_opener`, which would rebind this process's global opener (the leak guard)."""
    opener = urllib.request.OpenerDirector()
    for handler in (transport, urllib.request.HTTPRedirectHandler(), urllib.request.HTTPDefaultErrorHandler(),
                    urllib.request.HTTPErrorProcessor()):
        opener.add_handler(handler)
    return lambda req, timeout=None: opener.open(req, timeout=timeout)


def retry_leg() -> None:
    """Hermetic: download() runs under the gate plane's one network retry policy (scripts/net-retry, build-gates
    §0), pinned here through the installer with a fake clock: a transient failure (429/5xx, a timeout, a refused or
    dropped connection) is retried, the nominal pause doubling from 2 s to a 60 s cap with jitter in its upper
    half, until 300 s after the FIRST failure; then the last error is re-raised and the run's Retrier is spent, so
    no later download is attempted. A final status (404, 403) is raised at once. urlopen is mocked ONLY inside
    mock.patch.object context managers: igt.urllib.request is this process's module, so a bare assignment would
    stay in force for every leg after this one (the incident the leak-guard legs replay); the clock belongs to the
    installer copy's own Retrier object, never to a process module."""
    igt = _load_installer("install_gate_tools_r")
    with tempfile.TemporaryDirectory() as td:
        out = Path(td) / "o"

        fc, logs = _fake_retrier(igt)
        opener, calls = _opener([_http(502), _http(502)])          # 2 transient then success
        err = _download(igt, opener, out)
        record("download retries a transient 502 then succeeds",
               err is None and out.read_bytes() == _MOCK_PAYLOAD and calls["n"] == 3 and fc.pauses == [2.0, 4.0],
               f"calls={calls['n']} pauses={fc.pauses}")
        record("a retry log line names the url, the attempt and the pause",
               logs[:1] == ["[install-gate-tools] https://x/y: attempt 1 failed (HTTP 502); retrying in 2.0s"],
               logs[0] if logs else "no log line")

        fc, _logs = _fake_retrier(igt)
        opener, calls = _opener([_http(404), _http(404)])          # final -> the first attempt raises
        err = _download(igt, opener, out)
        record("download does NOT retry a permanent 404",
               isinstance(err, urllib.error.HTTPError) and err.code == 404 and calls["n"] == 1 and fc.pauses == [])

        fc, _logs = _fake_retrier(igt)
        opener, calls = _opener([_http(403)])
        err = _download(igt, opener, out)
        record("download does NOT retry a 403 (every 4xx but 429 is final)",
               isinstance(err, urllib.error.HTTPError) and err.code == 403 and calls["n"] == 1)

        fc, _logs = _fake_retrier(igt)
        opener, calls = _opener([_http(429), TimeoutError("timed out"),
                                 urllib.error.URLError(ConnectionRefusedError("refused")),
                                 http.client.RemoteDisconnected("closed"), _http(503)])
        err = _download(igt, opener, out)
        record("a 429, a timeout, a refused connection, a dropped connection and a 503 are each retried",
               err is None and calls["n"] == 6 and out.read_bytes() == _MOCK_PAYLOAD, f"calls={calls['n']}")

        # [Test-Change: G24 network retry policy — old-obsolete+new-correct, build-gates §0] the old leg pinned
        # RETRY_ATTEMPTS = 3 tries with a linear 2 s / 4 s backoff, the schedule a 75 s release-CDN 504 burst
        # outlasted (ci run 38245893904); the policy now retries until 300 s after the first failure, the pause
        # doubling from 2 s to a 60 s cap, and this leg pins that schedule exactly (an installer back on 3 linear
        # tries makes 3 calls with pauses [2, 4] and reds it).
        fc, _logs = _fake_retrier(igt)
        opener, calls = _opener([_http(502)] * 99)                  # every attempt fails
        err = _download(igt, opener, out)
        record("schedule: an always-502 download pauses 2, 4, 8, 16, 32, 60, 60, 60 s, cuts the last pause to end "
               "300 s after its first failure, then re-raises",
               isinstance(err, urllib.error.HTTPError) and err.code == 502 and calls["n"] == 10
               and fc.pauses == [2.0, 4.0, 8.0, 16.0, 32.0, 60.0, 60.0, 60.0, 58.0] and fc.now == 300.0,
               f"calls={calls['n']} pauses={fc.pauses} clock={fc.now}")
        opener, calls = _opener([])
        err = _download(igt, opener, out)
        record("a spent run attempts no further download (WindowSpent, no request)",
               isinstance(err, igt._NET.WindowSpent) and calls["n"] == 0)

        fc, _logs = _fake_retrier(igt, rng=0.0)
        opener, calls = _opener([_http(504)] * 99)
        _download(igt, opener, out)
        record("jitter: the lowest draw halves every nominal pause (1, 2, 4, 8, 16, 30 s ...), the window still "
               "ends 300 s after the first failure",
               fc.pauses == [1.0, 2.0, 4.0, 8.0, 16.0] + [30.0] * 8 + [29.0] and fc.now == 300.0,
               f"pauses={fc.pauses} clock={fc.now}")

        fc, _logs = _fake_retrier(igt)
        igt._RETRY = igt._NET.Retrier("install-gate-tools", clock=fc.clock, sleep=fc.sleep,
                                      rng=random.Random(20261012).random, log=lambda _m: None)
        opener, calls = _opener([_http(500)] * 99)
        _download(igt, opener, out)
        nominal = [2, 4, 8, 16, 32] + [60] * 99
        record("jitter: every random pause lies in the upper half of its nominal pause, the last one excepted "
               "(it is cut to the window's end)",
               len(fc.pauses) > 6 and all(n / 2 <= p <= n for p, n in zip(fc.pauses[:-1], nominal))
               and fc.now == 300.0, f"pauses={[round(p, 1) for p in fc.pauses]}")

        fc, _logs = _fake_retrier(igt)
        opener, calls = _opener([_http(504)] * 99, fc=fc, stall=100.0)
        _download(igt, opener, out)
        record("the window opens at the FIRST FAILURE: a first attempt that hangs 100 s still gets 300 s of retries",
               fc.now == 400.0 and fc.pauses[-1:] == [58.0], f"clock={fc.now}")

        # A release asset answers 302 to its CDN; urllib counts the redirects it followed on the request object, so
        # a request reused across attempts is refused as a redirect loop (a final HTTP 302) after four attempts on
        # the same Location and ten on changing ones, and the window ends early. The lowest jitter draw gives the
        # most attempts inside the window (15); each attempt must build its own request to ride all of them.
        for same, how in ((True, "the same Location"), (False, "a new Location each time")):
            fc, _logs = _fake_retrier(igt, rng=0.0)
            transport = _RedirectTransport("https://x/y", same_location=same)
            err = _download(igt, _urlopen_via(transport), out)
            record(f"a 302 to {how} into a 503 CDN outage rides the whole 300 s window (a request per attempt)",
                   isinstance(err, urllib.error.HTTPError) and err.code == 503 and transport.redirects == 15
                   and fc.now == 300.0,
                   f"{type(err).__name__} {getattr(err, 'code', '')} redirects={transport.redirects} clock={fc.now}")

        fc, _logs = _fake_retrier(igt)
        opener, calls = _opener([])
        err = _download(igt, opener, Path(td) / "no-such-dir" / "o")
        record("a local write error (the destination's directory is missing) is a GateToolError at once, never a "
               "retried outage", isinstance(err, igt.GateToolError) and "cannot write the download" in str(err)
               and calls["n"] == 1 and fc.pauses == [] and not igt._RETRY.spent,
               f"{type(err).__name__}: {err} calls={calls['n']}")

        fc, _logs = _fake_retrier(igt)
        release, hung, answers = threading.Event(), {"n": 0}, []

        def hang_first(_req, timeout=None):
            hung["n"] += 1
            if hung["n"] == 1:
                release.wait(5)          # the first attempt does not answer inside its bound (5 s at most: no hang)
            answers.append(_Resp(_MOCK_PAYLOAD))
            return answers[-1]

        saved_bound, igt.DOWNLOAD_OPEN_S = igt.DOWNLOAD_OPEN_S, 0.05
        try:
            err = _download(igt, hang_first, out)
        finally:
            release.set()
            igt.DOWNLOAD_OPEN_S = saved_bound
        record("an attempt with no response inside its request bound is ended and retried as a timeout",
               err is None and out.read_bytes() == _MOCK_PAYLOAD and hung["n"] == 2 and fc.pauses == [2.0],
               f"calls={hung['n']} pauses={fc.pauses}")
        _until = time.monotonic() + 5
        while not (len(answers) == 2 and all(a.closed for a in answers)) and time.monotonic() < _until:
            time.sleep(0.01)
        record("the response that arrives after its request bound is closed unread, never handed on",
               len(answers) == 2 and all(a.closed for a in answers), f"responses={len(answers)}")

        # The body's bounds in real time, on scaled-down constants of the installer copy (a floor of 50 B/s, a stall
        # bound of 0.2 s): a stall ends the attempt at the socket timeout, a slow body at four times the floor rate
        # completes in one attempt at either size - the larger one taking longer than the smaller one's time, so a
        # bound that did not scale with the size would cut it - and one still arriving at its deadline is ended there.
        scaled = {"DOWNLOAD_FLOOR_BPS": 50, "DOWNLOAD_STALL_S": 0.2, "DOWNLOAD_OPEN_S": 2.0}
        saved = {k: getattr(igt, k) for k in scaled}
        for k, v in scaled.items():
            setattr(igt, k, v)
        try:
            for size in (40, 200):
                fc, _logs = _fake_retrier(igt)
                body = bytes(range(size))
                paced, calls = _paced_opener([dict(body=body, step=2, gap=0.01, declared=size)])
                t0 = time.monotonic()
                err = _download(igt, paced, out)
                took = time.monotonic() - t0
                record(f"a {size}-byte body arriving at four times the floor rate completes in one attempt although "
                       f"it takes {size / 200:.1f} s (its time scales with its declared size)",
                       err is None and out.read_bytes() == body and calls["n"] == 1 and fc.pauses == []
                       and took >= size / 200 * 0.8, f"{type(err).__name__ if err else 'ok'} calls={calls['n']} "
                                                     f"{took:.2f} s")
            fc, _logs = _fake_retrier(igt)
            paced, calls = _paced_opener([dict(body=b"0123456789" * 4, step=10, gap=0.0, declared=40, stall_after=10),
                                          dict(body=_MOCK_PAYLOAD, step=len(_MOCK_PAYLOAD), gap=0.0)])
            err = _download(igt, paced, out)
            first = calls["responses"][0]
            record("a transfer that stalls (no byte for the stall bound) ends at the socket timeout urlopen() was given "
                   "and is retried", err is None and out.read_bytes() == _MOCK_PAYLOAD and calls["n"] == 2
                   and fc.pauses == [2.0] and calls["timeouts"][0] == igt.DOWNLOAD_STALL_S and first.stalled,
                   f"{type(err).__name__ if err else 'ok'} calls={calls['n']} timeouts={calls['timeouts']}")
            fc, _logs = _fake_retrier(igt)
            paced, calls = _paced_opener([dict(body=b"x" * 1000, step=1, gap=0.01, declared=40),
                                          dict(body=_MOCK_PAYLOAD, step=len(_MOCK_PAYLOAD), gap=0.0)])
            err = _download(igt, paced, out)
            trickle = calls["responses"][0]
            record("a body still arriving at its deadline (40 bytes declared, 1 byte per 10 ms) is ended there and "
                   "retried, and no byte of it is read after the attempt ended",
                   err is None and out.read_bytes() == _MOCK_PAYLOAD and calls["n"] == 2 and fc.pauses == [2.0]
                   and bool(trickle.reads) and max(trickle.reads) < calls["opened"][1],
                   f"{type(err).__name__ if err else 'ok'} calls={calls['n']} reads={len(trickle.reads)}")
        finally:
            for k, v in saved.items():
                setattr(igt, k, v)
        record("the body's time: its declared size at the floor rate, at least the stall bound, an undeclared or "
               "oversized body read as the size cap",
               igt.body_seconds(None) == igt.body_seconds(igt.DOWNLOAD_CAP_BYTES * 2)
               == igt.DOWNLOAD_CAP_BYTES / igt.DOWNLOAD_FLOOR_BPS
               and igt.body_seconds(0) == igt.DOWNLOAD_STALL_S
               and igt.body_seconds(58_692_096) == 58_692_096 / igt.DOWNLOAD_FLOOR_BPS
               and igt.DOWNLOAD_ATTEMPT_S == igt.DOWNLOAD_OPEN_S + igt.body_seconds(None) + igt.DOWNLOAD_STALL_S,
               f"attempt bound {igt.DOWNLOAD_ATTEMPT_S} s")


retry_leg()


def _tool_rows(name: str) -> str:
    """One synthetic raw tool under ORIGIN, pinned for every platform key."""
    rows = "".join(f'{k} = {{ url = "{ORIGIN}{name}.bin", sha256 = "{"0" * 64}" }}\n' for k in PLATFORM_KEYS)
    return (f"[tools.{name}]\n"
            'version = "0"\n'
            f'origin = "{ORIGIN}"\n'
            f'bin_name = "{name}"\n'
            'asset_kind = "raw"\n'
            'corroboration = "self-test synthetic"\n'
            f"[tools.{name}.platforms]\n" + rows)


def _main_run(igt: types.ModuleType, opener, *argv: str) -> tuple[int, str]:
    """igt.main(argv) under the urlopen double: (exit code, everything it printed)."""
    buf = io.StringIO()
    with (mock.patch.object(igt.urllib.request, "urlopen", opener), contextlib.redirect_stdout(buf),
          contextlib.redirect_stderr(buf)):
        rc = igt.main(list(argv))
    return rc, buf.getvalue()


def run_main_leg() -> None:
    """Hermetic, through main(): a run of two tools whose first download fails through its retry window reports
    that tool as an outage ('download failed after its retry window') and the second as not attempted, exit 1, with
    no request for the second tool; a final 404, and a body past the size cap, report 'download failed' with no
    retry and no window wording - the split the network legs' SKIP condition (_asset_flake) reads."""
    igt = _load_installer("install_gate_tools_m")
    with tempfile.TemporaryDirectory() as td:
        man = Path(td) / "m.toml"
        man.write_text("schema_version = 1\n" + _tool_rows("alpha") + _tool_rows("beta"), encoding="utf-8")
        _fake_retrier(igt)
        opener, calls = _opener([_http(502)] * 99)
        rc, text = _main_run(igt, opener, "--manifest", str(man), "--dest", str(Path(td) / "d1"))
        record("a run whose first download fails through its window: that tool reads 'download failed after its "
               "retry window', the next 'download not attempted', exit 1, no request for the next",
               rc == 1 and "alpha: download failed after its retry window: HTTP Error 502" in text
               and "beta: download not attempted" in text and calls["n"] == 10, f"exit={rc} calls={calls['n']}")
        _fake_retrier(igt)
        opener, calls = _opener([_http(404)] * 99)
        rc, text = _main_run(igt, opener, "--manifest", str(man), "--dest", str(Path(td) / "d2"), "--tool", "alpha")
        record("a final 404 reads 'download failed: HTTP Error 404' - no retry, no window wording",
               rc == 1 and "alpha: download failed: HTTP Error 404" in text
               and "after its retry window" not in text and calls["n"] == 1, f"exit={rc} calls={calls['n']}")
        _fake_retrier(igt)
        cap = len(_MOCK_PAYLOAD) - 1
        saved_cap, igt.DOWNLOAD_CAP_BYTES = igt.DOWNLOAD_CAP_BYTES, cap
        try:
            opener, calls = _opener([])
            rc, text = _main_run(igt, opener, "--manifest", str(man), "--dest", str(Path(td) / "d3"),
                                 "--tool", "alpha")
        finally:
            igt.DOWNLOAD_CAP_BYTES = saved_cap
        record("a body that grows past DOWNLOAD_CAP_BYTES (scaled down here) reads 'download failed: the body exceeds "
               "its <cap> byte limit' - one request, no retry, no window wording",
               rc == 1 and f"alpha: download failed: the body exceeds its {cap} byte limit" in text
               and "after its retry window" not in text and calls["n"] == 1, f"exit={rc} calls={calls['n']}")


run_main_leg()


def pip_leg() -> None:
    with tempfile.TemporaryDirectory() as td:
        rq = Path(td) / "reqs.txt"
        rq.write_text("requests==2.31.0\n", encoding="utf-8")  # no --hash on purpose
        try:
            p = subprocess.run(
                [sys.executable, "-m", "pip", "install", "--require-hashes", "--no-deps",
                 "-r", str(rq)],
                capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=120,
            )
        except (FileNotFoundError, subprocess.TimeoutExpired) as e:
            record("pip --require-hashes rejects hashless", True, f"SKIP ({type(e).__name__})")
            return
        ok = p.returncode != 0 and "hash" in (p.stdout + p.stderr).lower()
        record("pip --require-hashes rejects hashless", ok, f"exit={p.returncode}")


pip_leg()

# --- _asset_flake detector (hermetic): a download that failed through its retry window skips-as-pass; a final
# failure (a 4xx), a sha-mismatch (a real checksum-control outcome) and a retried attempt the download recovered
# from are NEVER skipped, so a genuine control defect still fails. ---
# [Test-Change: G24 network retry policy — old-obsolete+new-correct, build-gates §0] the installer now words an
# outage 'download failed after its retry window' and a final failure 'download failed', and a retry log line is
# '<url>: attempt N failed (...); retrying in ...'; the legs below feed those wordings (a bare 'download failed'
# matched a 404 too, so an asset that is gone SKIPped as an outage).
record("_asset_flake: 'download failed after its retry window: HTTP Error 502' -> True (skip-as-pass)",
       _asset_flake("GateToolError: download failed after its retry window: HTTP Error 502: Bad Gateway") is True)
record("_asset_flake: a final 'download failed: HTTP Error 404' -> False (a 4xx never earns an outage SKIP)",
       _asset_flake("[FAIL]    selftest: download failed: HTTP Error 404: Not Found") is False)
# [Test-Change: P0.2.1 — old-obsolete+new-correct, build-gates §0 / G24] the old leg read a retried
# attempt's log line as a download failure; as a SKIP route it let an install that failed after its sha
# verified SKIP behind one recovered 502 (measured with the installer mutated both ways). download() logs
# the line before the retry that can recover, and every download that never completes ends in
# GateToolError('download failed ...') (install_tool), so the line alone is no download failure.
record("_asset_flake: a retry log line alone ('attempt 1 failed (...); retrying in ...') -> False (a recoverable "
       "attempt)",
       _asset_flake(f"[install-gate-tools] {SMALL_ASSET}: attempt 1 failed (HTTP 502); retrying in 2.0s") is False)
record("_asset_flake: a real 'SHA-256 MISMATCH' install failure -> False (a real outcome, not skipped)",
       _asset_flake("[FAIL]    selftest: SHA-256 MISMATCH - refusing to install (expected abc, got def)") is False)
record("_asset_flake: a clean install output -> False",
       _asset_flake("[ok]      selftest 0 (windows-x86_64) - already verified") is False)

# --- _download_failed (hermetic): the one SKIP condition of every install leg; a leg pins each conjunct
# (the exit code, the download that failed through its window, the absent mismatch). ---
_RETRIED = f"[install-gate-tools] {SMALL_ASSET}: attempt 1 failed (HTTP 502); retrying in 2.0s\n"
_DL_FAILED = "[FAIL]    selftest: download failed after its retry window: HTTP Error 502: Bad Gateway\n"
_MISMATCH = "[FAIL]    selftest: SHA-256 MISMATCH - refusing to install\n"
_AFTER_DL = "[FAIL]    selftest: 'selftest' not found in the zip archive\n"   # raised after the sha verified
record("_download_failed: a download that failed through its retry window -> True (SKIP-as-PASS)",
       _download_failed(1, _RETRIED + _DL_FAILED) is True)
record("_download_failed: a retried download that reached a SHA-256 MISMATCH -> False (the correct pin FAILs)",
       _download_failed(1, _RETRIED + _MISMATCH) is False)
record("_download_failed: an install that failed after its download completed -> False (a real outcome)",
       _download_failed(1, _AFTER_DL) is False)
record("_download_failed: the same failure behind a recovered retry -> False (a retried attempt is no SKIP)",
       _download_failed(1, _RETRIED + _AFTER_DL) is False)
record("_download_failed: a mismatch beside a failed download (a two-tool install) -> False",
       _download_failed(1, _DL_FAILED + _MISMATCH.replace("selftest", "other")) is False)
record("_download_failed: an install that exited 0 -> False, whatever its output names",
       _download_failed(0, _RETRIED + _DL_FAILED) is False)


# --- the network decision (run last, after the leak guard) and its hermetic legs ---
NETWORK_LEGS = ("wrong checksum fails the install", "correct checksum passes the install",
                "install idempotent (2nd run = already verified)")
WRONG_LEG, CORRECT_LEG, IDEMPOTENT_LEG = NETWORK_LEGS
_UNFETCHABLE = "SKIP (asset download failed through its retry window)"
Recorder = Callable[[str, bool, str], None]


def network_legs(good: str, install: Callable[..., tuple[int, str]] = run_installer, rec: Recorder = record) -> None:
    """The three install legs against the real asset, each judged on its own installer run: a leg whose download
    failed through its retry window SKIPs-as-PASS, and every other outcome is that leg's verdict - a SKIP never
    carries over to another leg. The three first runs start together, each its own process with its own retry
    window, so an outage that began after the reference fetch costs one window of wall time, not one per leg. The
    idempotency leg's second run reuses its first run's destination and follows the first only once that first
    download completed (a first run that failed through its window SKIPs the leg whatever a second run did)."""
    bad = ("1" if good[0] != "1" else "0") + good[1:]  # one-char flip guarantees a mismatch
    with tempfile.TemporaryDirectory() as td, ThreadPoolExecutor(max_workers=3) as pool:
        dest = Path(td) / "dest"
        wrong = pool.submit(install, manifest(SMALL_ASSET, bad))
        correct = pool.submit(install, manifest(SMALL_ASSET, good))
        first = pool.submit(install, manifest(SMALL_ASSET, good), dest=dest)
        rc, out = wrong.result()
        if _download_failed(rc, out):
            rec(WRONG_LEG, True, _UNFETCHABLE)
        else:
            rec(WRONG_LEG, rc == 1 and "mismatch" in out.lower(), f"exit={rc}")
        rc, out = correct.result()
        if _download_failed(rc, out):
            rec(CORRECT_LEG, True, _UNFETCHABLE)
        else:
            rec(CORRECT_LEG, rc == 0, f"exit={rc}")
        # Idempotency: a 2nd install of the same pinned source = a verified no-op via the
        # source-sha256 stamp (the path ARCHIVE tools rely on for re-runs + --offline).
        rc1, out1 = first.result()
        if _download_failed(rc1, out1):
            rec(IDEMPOTENT_LEG, True, _UNFETCHABLE)
            return
        rc2, out2 = install(manifest(SMALL_ASSET, good), dest=dest)
        if _download_failed(rc2, out2):
            rec(IDEMPOTENT_LEG, True, _UNFETCHABLE)
            return
        rec(IDEMPOTENT_LEG, rc1 == 0 and rc2 == 0 and "already verified" in out2, f"{rc1}/{rc2}")


def network_decision(require_network: bool, probe: Callable[[], bool] = online,
                     reference: Callable[[], tuple[str, str]] = real_sha,
                     legs: Callable[[str], None] = network_legs, rec: Recorder = record) -> None:
    """The network legs' FOUR-way decision, from the online probe and the reference fetch, each under the retry
    policy:
      * the probe fails (github.com unreachable through the retry window, or refused) -> no usable network;
        --require-network turns the skip into a FAIL (no vacuous pass of the checksum legs);
      * the reference asset 'refused' (a final failure: a 4xx) -> FAIL, with or without --require-network: a
        permanent failure never earns an outage SKIP, and an asset that is gone would otherwise skip these legs
        forever;
      * the reference asset 'unfetchable' (transient through its whole window) -> the CI HAS network but the
        release ASSET is down -> SKIP-AS-PASS (infra, not a checksum-control defect; the mechanism is exercised at
        L1/L2 + whenever the asset is up) - this keeps a GitHub release-asset outage from reddening main for the
        wrong reason;
      * otherwise -> the install legs against the fetched sha (network_legs): every sha mismatch is a real
        outcome, so a genuine control defect - or a wrong reference sha, the retry_leg incident - FAILS.
    The reference is fetched ONCE and reused, to minimise GitHub hits (fewer = less rate-limiting)."""
    if not probe():
        detail = "FAIL (offline, --require-network set)" if require_network else "SKIP (offline)"
        for leg in NETWORK_LEGS:
            rec(leg, not require_network, detail)
        return
    state, value = reference()
    # [Test-Change: G24 network retry policy — old-obsolete+new-correct, build-gates §0] the old three-way decision
    # SKIPped every network leg as an outage whenever the reference fetch did not complete, a final 4xx included, so
    # an asset that is gone would skip the checksum legs on every run; under the policy a final status fails at once
    # and is no outage, so a refused reference FAILs the legs, with or without --require-network (decision_leg).
    if state == "refused":
        for leg in NETWORK_LEGS:
            rec(leg, False, f"FAIL (the reference asset was refused: {value} - a final failure is no outage SKIP)")
    elif state == "unfetchable":
        for leg in NETWORK_LEGS:
            rec(leg, True, _UNFETCHABLE)
    else:
        legs(value)


def probe_leg() -> None:
    """Hermetic: the two probes under the retry policy, each through a fake-clock Retrier passed in (never this
    process's own) with urlopen mocked inside a context manager. The reference fetch is 'ok' after a recovered
    transient failure, 'refused' at once on a final 4xx (the asset is gone: a FAIL, never an outage SKIP) and
    'unfetchable' once its 5xx outlasts the window, also behind a 302 to its CDN; the online probe is True on an
    answer, False at once on a 4xx and False after the window on a 5xx, and a probe that spent the window leaves the
    reference fetch 'unfetchable' without a request (the two share one window)."""
    want = hashlib.sha256(_MOCK_PAYLOAD).hexdigest()

    def run(seq, *probes):
        fc = _FakeClock()
        retrier = NET.Retrier("g24-install-gate-tools", clock=fc.clock, sleep=fc.sleep, rng=lambda: 1.0,
                              log=lambda _m: None)
        opener, calls = _opener(seq)
        with mock.patch.object(urllib.request, "urlopen", opener):
            got = [p(retrier) for p in probes]
        return got, calls["n"], fc, retrier

    (ref,), n, fc, r = run([_http(503)], real_sha)
    record("reference fetch: a 503, then the asset -> ('ok', its sha256) after one 2 s pause",
           ref == ("ok", want) and n == 2 and fc.pauses == [2.0], f"{ref[0]} calls={n} pauses={fc.pauses}")
    (ref,), n, fc, r = run([_http(404)], real_sha)
    record("reference fetch: a final 404 -> 'refused' at once (one request, no pause, the window unspent)",
           ref[0] == "refused" and "404" in ref[1] and n == 1 and fc.pauses == [] and not r.spent,
           f"{ref} calls={n}")
    (ref,), n, fc, r = run([_http(502)] * 99, real_sha)
    record("reference fetch: a 502 through the whole window -> 'unfetchable' 300 s after the first failure",
           ref[0] == "unfetchable" and "502" in ref[1] and n == 10 and fc.now == 300.0 and r.spent,
           f"{ref} calls={n} clock={fc.now}")
    (up,), n, fc, r = run([], online)
    record("online probe: an answer -> True", up is True and n == 1, f"{up} calls={n}")
    (up,), n, fc, r = run([_http(404)], online)
    record("online probe: a final 404 -> False at once (one request, no pause)",
           up is False and n == 1 and fc.pauses == [], f"{up} calls={n}")
    (up, ref), n, fc, r = run([_http(502)] * 99, online, real_sha)
    record("online probe: a 502 through the whole window -> False, and the reference fetch on that spent window "
           "-> 'unfetchable' without a request",
           up is False and n == 10 and fc.now == 300.0 and ref[0] == "unfetchable" and "not attempted" in ref[1],
           f"{up} {ref[0]} calls={n} clock={fc.now}")
    # The reference asset answers 302 to its CDN: with a request per attempt the fetch rides the whole window into
    # 'unfetchable'; a reused request is refused as a redirect loop (a final 302, 'refused') after four attempts on
    # the same Location and ten on changing ones. The lowest jitter draw gives the most attempts (15).
    for same, how in ((True, "the same Location"), (False, "a new Location each time")):
        fc = _FakeClock()
        retrier = NET.Retrier("g24-install-gate-tools", clock=fc.clock, sleep=fc.sleep, rng=lambda: 0.0,
                              log=lambda _m: None)
        transport = _RedirectTransport(SMALL_ASSET, same_location=same)
        with mock.patch.object(urllib.request, "urlopen", _urlopen_via(transport)):
            ref = real_sha(retrier)
        record(f"reference fetch: a 302 to {how} into a 503 CDN outage -> 'unfetchable' after the whole window",
               ref[0] == "unfetchable" and "503" in ref[1] and transport.redirects == 15 and fc.now == 300.0,
               f"{ref} redirects={transport.redirects} clock={fc.now}")


def decision_leg() -> None:
    """Hermetic: network_decision() through each of its four branches, with doubles for the probe, the reference
    fetch, the install legs and the recorder."""
    cases = (
        ("offline under --require-network -> every network leg FAILs, no reference fetch",
         True, False, None, False, "FAIL (offline, --require-network set)", 0),
        ("offline without --require-network -> every network leg SKIPs, no reference fetch",
         False, False, None, True, "SKIP (offline)", 0),
        ("a refused reference asset -> every network leg FAILs, also without --require-network, no install run",
         False, True, ("refused", "HTTP Error 404: Not Found"), False, "FAIL (the reference asset was refused", 1),
        ("an unfetchable reference asset -> every network leg SKIPs, also under --require-network, no install run",
         True, True, ("unfetchable", "HTTP Error 502: Bad Gateway"), True, _UNFETCHABLE, 1),
    )
    for label, require, up, ref, ok, needle, fetches in cases:
        recs: list[tuple[str, bool, str]] = []
        refs: list[object] = []
        runs: list[str] = []

        def reference() -> tuple[str, str]:
            refs.append(ref)
            return ref

        network_decision(require, probe=lambda: up, reference=reference, legs=runs.append,
                         rec=lambda n, o, d: recs.append((n, o, d)))
        record(f"network decision: {label}",
               [(n, o) for n, o, _d in recs] == [(leg, ok) for leg in NETWORK_LEGS]
               and all(needle in d for _n, _o, d in recs) and len(refs) == fetches and runs == [],
               f"{recs} fetches={len(refs)} install runs={runs}")
    sha = "ab" * 32
    recs, runs = [], []
    network_decision(True, probe=lambda: True, reference=lambda: ("ok", sha), legs=runs.append,
                     rec=lambda n, o, d: recs.append((n, o, d)))
    record("network decision: a fetched reference -> the install legs run on its sha, nothing recorded here",
           runs == [sha] and recs == [], f"install runs={runs} recorded={recs}")


class _InstallDouble:
    """The installer double for network_legs(): a call is routed by its manifest's pin and its destination to a leg
    ('wrong': the flipped pin; 'correct': the good pin in a fresh destination; 'idempotent': the good pin in the
    shared destination), answered with that leg's next scripted (exit code, output) and logged. The first three
    calls wait at a barrier until all three have started, so a network_legs() that ran its first runs one after
    another breaks the barrier (BrokenBarrierError) instead of passing."""

    def __init__(self, good: str, script: dict[str, list[tuple[int, str]]]) -> None:
        self._good_pin = f'sha256 = "{good}"'
        self._script = {leg: list(runs) for leg, runs in script.items()}
        self._lock = threading.Lock()
        self._barrier = threading.Barrier(3, timeout=10)
        self.calls: list[tuple[str, Path | None]] = []

    def __call__(self, manifest_text: str, *_extra: str, dest: Path | None = None) -> tuple[int, str]:
        leg = "wrong" if self._good_pin not in manifest_text else ("correct" if dest is None else "idempotent")
        with self._lock:
            self.calls.append((leg, dest))
            first_three = len(self.calls) <= 3
            outcome = self._script[leg].pop(0)
        if first_three:
            self._barrier.wait()
        return outcome


def install_legs_leg() -> None:
    """Hermetic: network_legs() with the installer double - every install leg is judged on its own run (a SKIP
    never carries over to another leg), the three first runs start together, and the idempotency leg's second run
    reuses its first run's destination, only after a first download that completed."""
    good = "cd" * 32
    mismatch, outage = (1, _MISMATCH), (1, _RETRIED + _DL_FAILED)
    installed = (0, "[install] selftest 0 (linux-x86_64) - sha256 verified\n")
    verified = (0, "[ok]      selftest 0 (linux-x86_64) - already verified\n")
    skip = _UNFETCHABLE
    cases = (
        ("every run completes: the wrong pin fails its install, the correct pin installs, the re-run is a verified "
         "no-op, and the three first runs started together",
         {"wrong": [mismatch], "correct": [installed], "idempotent": [installed, verified]},
         [(WRONG_LEG, True, "exit=1"), (CORRECT_LEG, True, "exit=0"), (IDEMPOTENT_LEG, True, "0/0")], 4),
        ("a SKIP stays its own leg's: beside the wrong pin's outage SKIP, the correct pin's mismatch and a re-run "
         "that installs again FAIL",
         {"wrong": [outage], "correct": [mismatch], "idempotent": [installed, installed]},
         [(WRONG_LEG, True, skip), (CORRECT_LEG, False, "exit=1"), (IDEMPOTENT_LEG, False, "0/0")], 4),
        ("a wrong pin that installs FAILs beside the correct pin's outage SKIP",
         {"wrong": [installed], "correct": [outage], "idempotent": [installed, verified]},
         [(WRONG_LEG, False, "exit=0"), (CORRECT_LEG, True, skip), (IDEMPOTENT_LEG, True, "0/0")], 4),
        ("a first idempotency run whose download failed through its window SKIPs that leg with no second run",
         {"wrong": [mismatch], "correct": [installed], "idempotent": [outage]},
         [(WRONG_LEG, True, "exit=1"), (CORRECT_LEG, True, "exit=0"), (IDEMPOTENT_LEG, True, skip)], 3),
        ("a second idempotency run whose download failed through its window SKIPs that leg",
         {"wrong": [mismatch], "correct": [installed], "idempotent": [installed, outage]},
         [(WRONG_LEG, True, "exit=1"), (CORRECT_LEG, True, "exit=0"), (IDEMPOTENT_LEG, True, skip)], 4),
    )
    for label, script, want, runs in cases:
        double = _InstallDouble(good, script)
        recs: list[tuple[str, bool, str]] = []
        try:
            network_legs(good, install=double, rec=lambda n, o, d: recs.append((n, o, d)))
            err = ""
        except Exception as e:  # noqa: BLE001 - the leg judges the exception (a broken barrier, an unscripted run)
            err = f"{type(e).__name__}: {e}"
        idem = [d for leg, d in double.calls if leg == "idempotent"]
        record(f"install legs: {label}",
               not err and recs == want and len(double.calls) == runs and len(set(idem)) == 1
               and idem[0] is not None and all(d is None for leg, d in double.calls if leg != "idempotent"),
               err or f"{recs} runs={len(double.calls)}")


probe_leg()
decision_leg()
install_legs_leg()


# --- the mock-leak guard legs (hermetic; after every other hermetic leg, before the network legs) ---
def leak_replay_leg() -> None:
    """The incident replay: the pre-fix retry_leg shape - a bare assignment of a urlopen / sleep double
    through the installer module - rebinds this process's urllib.request.urlopen and time.sleep, so the
    network legs' real_sha() hashes the double's payload (the reference sha every host then compared the
    real asset against) and _leaked_since() names both bindings. The replay undoes its own rebinding in a
    finally; the leak-guard leg after it proves the undo."""
    igt = _load_installer("install_gate_tools_leak")
    before = _process_state()
    saved = (urllib.request.urlopen, time.sleep)
    seen: tuple[str, str] | None = None
    leaked: list[str] = []
    try:
        # [Test-Change: G24 network retry policy — old-obsolete+new-correct, build-gates §0] real_sha() now returns
        # its verdict beside the sha - ('ok', sha) for a fetch that completed, the split network_decision() reads -
        # so the replay compares that pair; the doubles are assigned through the installer module as before.
        igt.time.sleep = _no_sleep
        igt.urllib.request.urlopen = lambda _req, timeout=None: _Resp(_MOCK_PAYLOAD)
        seen = real_sha()
        leaked = _leaked_since(before)
    finally:
        urllib.request.urlopen, time.sleep = saved
    record("incident replay: a double assigned through the installer module reaches this process's real_sha()",
           seen == ("ok", hashlib.sha256(_MOCK_PAYLOAD).hexdigest()))
    record("incident replay: the leak guard names exactly the two rebound bindings",
           sorted(leaked) == ["time.sleep", "urllib.request.urlopen"], ", ".join(leaked) or "none named")


def guard_reach_leg() -> None:
    """The guard's reach beyond the incident's two module functions: a class member and an environment
    variable, changed inside this leg (and restored on its way out), are named as well."""
    before = _process_state()
    probe = "CONVERTIA_G24_LEAK_PROBE"
    leaked: list[str] = []
    try:
        with mock.patch.object(subprocess.CompletedProcess, "check_returncode", lambda _self: None):
            os.environ[probe] = "1"
            leaked = _leaked_since(before)
    finally:
        os.environ.pop(probe, None)
    record("leak guard reach: a rebound class member and a changed environment variable are named",
           sorted(leaked) == sorted([f"os.environ[{probe!r}]", "subprocess.CompletedProcess.check_returncode"]),
           ", ".join(leaked) or "none named")


leak_replay_leg()
guard_reach_leg()
_LEAKED = _leaked_since(_PRISTINE)
record("leak guard: the network legs start from the process state the script started with", not _LEAKED,
       ", ".join(_LEAKED) or f"{len(_PRISTINE[0])} code bindings and the environment unchanged")

# --- network legs (run if online; --require-network turns an offline skip into a FAIL) ---
# The four-way decision of network_decision() over the live probe, the live reference fetch and the real installer.
network_decision(args.require_network)

failed = [n for n, ok, _ in results if not ok]
print(f"\n[g24-install-gate-tools] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
