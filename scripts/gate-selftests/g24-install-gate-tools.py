#!/usr/bin/env python3
"""g24-install-gate-tools - G24 self-test for the pinned-tool fetch+verify control (P0.2.1).

Proves the security control BOTH ways (build-gates s0 / G24): a WRONG checksum
MUST fail the install; a CORRECT checksum MUST pass; an off-origin url MUST be
rejected; --offline with no prior install MUST fail; and the pip --require-hashes
leg MUST reject a hashless requirement. The hermetic legs (no network) always run;
the three network legs (wrong checksum, correct checksum, idempotent re-install)
fetch a tiny (~KB) real asset under the lefthook origin and, when offline,
SKIP-as-PASS by default OR FAIL under --require-network (which CI passes, so an
offline CI run cannot vacuously pass the checksum legs). Online, a leg SKIPs only
when its asset download never completed (a retried attempt the download recovered
from is no SKIP); a sha mismatch or any failure after the download is always a
real outcome (on the correct pin, a FAIL). The pip leg resolves its hashless
requirement against the package index before pip reports the missing hash, so it
needs the network as well: offline it FAILs, and it SKIPs only when pip is absent
or outruns its 120 s timeout. The hermetic legs mock the network and the clock
only inside context managers, and a leak-guard leg proves the network legs start
from the process state the script started with (a urlopen double that outlived
its leg made real_sha() hash the double's payload, so the correct-checksum and
idempotency legs SKIPped on every host).

Run:  python3 scripts/gate-selftests/g24-install-gate-tools.py
Exit: 0 = every assertion held; 1 = a self-test assertion FAILED (the gate is broken).
"""
import argparse
import hashlib
import importlib.machinery
import importlib.util
import io
import os
import subprocess
import sys
import tempfile
import time
import types
import urllib.error
import urllib.request
from pathlib import Path
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
# asset DOWNLOAD failed (_download_failed) - so a GitHub asset outage does not redden main for the wrong
# reason, while a genuinely-offline CI still fails. The reds of the 8c6fa67 and 8b5a8e0 pushes, once read as
# a GitHub transient and a GitHub asset incident, were the retry_leg mock leak: both self-tests replay the
# same reds with GitHub healthy.
ONLINE_PROBE = "https://github.com/"
PLATFORM_KEYS = ("linux-x86_64", "linux-aarch64", "macos-x86_64", "macos-aarch64", "windows-x86_64")

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


def run_installer(manifest_text: str, *extra: str) -> tuple[int, str]:
    with tempfile.TemporaryDirectory() as td:
        man = Path(td) / "manifest.toml"
        man.write_text(manifest_text, encoding="utf-8")
        cmd = [sys.executable, str(INSTALLER), "--manifest", str(man),
               "--dest", str(Path(td) / "dest"), *extra]
        p = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
        return p.returncode, p.stdout + p.stderr


def _load_installer(name: str) -> types.ModuleType:
    """A fresh in-process copy of install-gate-tools (no .py extension, so an explicit source loader). Its
    `urllib` / `time` globals are THIS process's modules - a mock set through them is process-wide."""
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
# The hermetic legs mock the network and the clock through the installer module's `urllib` / `time` names,
# which are this process's own modules. A mock that outlives its leg is what online() and real_sha() call:
# online() then reports a network even offline, and the installer subprocess's real bytes never match the
# mock payload's sha - the retry_leg incident, where the correct-checksum and idempotency legs routed that
# mismatch to SKIP on every host (a mismatch FAILs them now, _download_failed). _process_state() snapshots
# every CODE binding of every loaded module (its module-level callables and the callables / descriptors in
# the dicts of the classes it defines) plus the environment the installer subprocesses inherit; the
# leak-guard leg compares it with the state the network legs start from.
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


def online() -> bool:
    """Confirm the CI HAS NETWORK by probing the RELIABLE github.com root (ONLINE_PROBE), retrying a
    transient blip up to 3 times (2s backoff). This is deliberately NOT the flaky release asset: the asset
    can 5xx independently (handled by _download_failed / real_sha()->None). Under --require-network a False
    here means a genuinely-offline CI (no vacuous skip of the checksum legs)."""
    for attempt in range(3):
        try:
            req = urllib.request.Request(ONLINE_PROBE, method="HEAD",
                                         headers={"User-Agent": "convertia-selftest"})
            urllib.request.urlopen(req, timeout=20).close()
            return True
        except (urllib.error.URLError, TimeoutError, OSError):
            if attempt < 2:
                time.sleep(2)
    return False


def real_sha() -> str | None:
    """sha256 of SMALL_ASSET, retrying a transient blip up to 3 times (2s backoff). Returns None if the
    release asset is transiently UNFETCHABLE after the retries (a GitHub 5xx/timeout/connection error on
    the asset — infra, not a test failure) so the caller skips-as-pass."""
    for attempt in range(3):
        try:
            req = urllib.request.Request(SMALL_ASSET, headers={"User-Agent": "convertia-selftest"})
            with urllib.request.urlopen(req, timeout=60) as r:
                return hashlib.sha256(r.read()).hexdigest()
        except (urllib.error.URLError, TimeoutError, OSError):
            if attempt < 2:
                time.sleep(2)
    return None


def _asset_flake(output: str) -> bool:
    """True if an install FAILED because the release ASSET download never completed (an HTTP, timeout or
    connection error download() did not recover from - a GitHub 5xx outage is the case this SKIP exists
    for) rather than a real assertion outcome. install-gate-tools raises
    GateToolError('download failed: ...') for every download that did not complete; a real
    sha-verification failure says 'sha256 mismatch', which never matches here. A retried attempt
    ('download attempt N/3 failed (...)') is no download failure: download() logs it before the retry
    that can recover, and what the install reports after a recovery is a real outcome.
    Lets a GitHub asset-outage skip-as-pass (infra) while a genuine checksum-control defect still fails."""
    return "download failed" in output.lower()


def _download_failed(rc: int, output: str) -> bool:
    """(pure) The one SKIP-as-PASS condition of an install leg: the install exited non-zero, its output
    names a download that never completed (_asset_flake), and it reports no sha mismatch. Every other
    failure is a real outcome: a mismatch - on the CORRECT pin it FAILs - and a failure after the download
    completed, also after a retried attempt. (The correct-checksum leg used to SKIP on a mismatch as "the
    asset served corrupt bytes"; the reds that SKIP was added for compared the real asset with the
    retry_leg mock payload's sha - the 8b5a8e0 self-test replays them with GitHub healthy.)"""
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
    """time.sleep double: no real backoff in a hermetic leg."""


def retry_leg() -> None:
    """Hermetic: download() retries a TRANSIENT failure (502) then succeeds, does NOT retry a PERMANENT
    404, and re-raises after the final attempt - the shellcheck-502 transient hardening. urlopen +
    time.sleep are mocked (no real network, no real backoff) ONLY inside mock.patch.object context
    managers: igt.urllib.request / igt.time are this process's modules, so a bare assignment would stay
    in force for every leg after this one (the incident the leak-guard legs replay)."""
    igt = _load_installer("install_gate_tools_r")

    def _make(seq):
        """urlopen that raises each exception in seq in turn, then returns a fresh payload response."""
        calls = {"n": 0}

        def _open(_req, timeout=None):
            i = calls["n"]
            calls["n"] += 1
            if i < len(seq):
                raise seq[i]
            return _Resp(_MOCK_PAYLOAD)

        return _open, calls

    h502 = igt.urllib.error.HTTPError("u", 502, "Bad Gateway", {}, None)
    h404 = igt.urllib.error.HTTPError("u", 404, "Not Found", {}, None)
    with mock.patch.object(igt.time, "sleep", _no_sleep), tempfile.TemporaryDirectory() as td:
        out = Path(td) / "o"
        opener, calls = _make([h502, h502])          # 2 transient then success
        with mock.patch.object(igt.urllib.request, "urlopen", opener):
            igt.download("https://x/y", out)
        record("download retries a transient 502 then succeeds",
               out.read_bytes() == _MOCK_PAYLOAD and calls["n"] == 3)
        opener, calls = _make([h404, h404])          # permanent -> first attempt raises
        raised = False
        with mock.patch.object(igt.urllib.request, "urlopen", opener):
            try:
                igt.download("https://x/y", out)
            except igt.urllib.error.HTTPError as e:
                raised = e.code == 404
        record("download does NOT retry a permanent 404", raised and calls["n"] == 1)
        opener, calls = _make([h502, h502, h502, h502])   # all attempts fail
        raised = False
        with mock.patch.object(igt.urllib.request, "urlopen", opener):
            try:
                igt.download("https://x/y", out)
            except igt.urllib.error.HTTPError:
                raised = True
        record("download re-raises after all retries exhausted",
               raised and calls["n"] == igt.RETRY_ATTEMPTS)


retry_leg()


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

# --- _asset_flake detector (hermetic): a download that never completed skips-as-pass; a sha-mismatch (a
# real checksum-control outcome) and a retried attempt the download recovered from are NEVER skipped, so a
# genuine control defect still fails. ---
record("_asset_flake: 'download failed: HTTP Error 502' -> True (skip-as-pass)",
       _asset_flake("GateToolError: download failed: HTTP Error 502: Bad Gateway") is True)
# [Test-Change: P0.2.1 — old-obsolete+new-correct, build-gates §0 / G24] the old leg read a retried
# attempt's log line as a download failure; as a SKIP route it let an install that failed after its sha
# verified SKIP behind one recovered 502 (measured with the installer mutated both ways). download() logs
# the line before the retry that can recover, and every download that never completes ends in
# GateToolError('download failed: ...') (install_tool), so the line alone is no download failure.
record("_asset_flake: a 'download attempt 1/3 failed (...)' retry log alone -> False (a recoverable attempt)",
       _asset_flake("[install-gate-tools] download attempt 1/3 failed (HTTP 502); retrying in 2s") is False)
record("_asset_flake: a real 'SHA-256 MISMATCH' install failure -> False (a real outcome, not skipped)",
       _asset_flake("[FAIL]    selftest: SHA-256 MISMATCH - refusing to install (expected abc, got def)") is False)
record("_asset_flake: a clean install output -> False",
       _asset_flake("[ok]      selftest 0 (windows-x86_64) - already verified") is False)

# --- _download_failed (hermetic): the one SKIP condition of every install leg; a leg pins each conjunct
# (the exit code, the download that never completed, the absent mismatch). ---
_RETRIED = "[install-gate-tools] download attempt 1/3 failed (HTTP 502); retrying in 2s\n"
_DL_FAILED = "[FAIL]    selftest: download failed: HTTP Error 502: Bad Gateway\n"
_MISMATCH = "[FAIL]    selftest: SHA-256 MISMATCH - refusing to install\n"
_AFTER_DL = "[FAIL]    selftest: 'selftest' not found in the zip archive\n"   # raised after the sha verified
record("_download_failed: a download that never completed -> True (SKIP-as-PASS)",
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
    seen: str | None = None
    leaked: list[str] = []
    try:
        igt.time.sleep = _no_sleep
        igt.urllib.request.urlopen = lambda _req, timeout=None: _Resp(_MOCK_PAYLOAD)
        seen = real_sha()
        leaked = _leaked_since(before)
    finally:
        urllib.request.urlopen, time.sleep = saved
    record("incident replay: a double assigned through the installer module reaches this process's real_sha()",
           seen == hashlib.sha256(_MOCK_PAYLOAD).hexdigest())
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
# THREE-way decision, computed ONCE and applied to all three network legs:
#   * online()==False (github.com itself unreachable) -> genuinely OFFLINE; --require-network turns the
#     skip into a FAIL (no vacuous pass of the checksum legs);
#   * online()==True but _GOOD is None / an install's download failed (_download_failed) -> the CI HAS
#     network but the release ASSET is transiently 5xx -> SKIP-AS-PASS (infra, not a checksum-control
#     defect; the mechanism is exercised at L1/L2 + whenever the asset is up). This is what keeps a GitHub
#     release-asset 502 from reddening main for the wrong reason;
#   * otherwise -> run the real bad/good install assertions: every sha mismatch is a real outcome, so a
#     genuine control defect - or a wrong reference sha, the retry_leg incident - FAILS.
# real_sha() is fetched ONCE here (_GOOD) and reused, to minimise GitHub hits (fewer = less rate-limiting).
NETWORK_LEGS = ("wrong checksum fails the install", "correct checksum passes the install",
                "install idempotent (2nd run = already verified)")
WRONG_LEG, CORRECT_LEG, IDEMPOTENT_LEG = NETWORK_LEGS
_UNFETCHABLE = "SKIP (asset download failed)"
_ONLINE = online()
_GOOD = real_sha() if _ONLINE else None
if not _ONLINE:
    ok = not args.require_network
    detail = "FAIL (offline, --require-network set)" if args.require_network else "SKIP (offline)"
    for leg in NETWORK_LEGS:
        record(leg, ok, detail)
elif _GOOD is None:
    for leg in NETWORK_LEGS:
        record(leg, True, _UNFETCHABLE)
else:
    bad = ("1" if _GOOD[0] != "1" else "0") + _GOOD[1:]  # one-char flip guarantees a mismatch
    rc, out = run_installer(manifest(SMALL_ASSET, bad))
    if _download_failed(rc, out):
        record(WRONG_LEG, True, _UNFETCHABLE)
    else:
        record(WRONG_LEG, rc == 1 and "mismatch" in out.lower(), f"exit={rc}")
    rc, out = run_installer(manifest(SMALL_ASSET, _GOOD))
    if _download_failed(rc, out):
        record(CORRECT_LEG, True, _UNFETCHABLE)
    else:
        record(CORRECT_LEG, rc == 0, f"exit={rc}")
    # Idempotency: a 2nd install of the same pinned source = a verified no-op via the
    # source-sha256 stamp (the path ARCHIVE tools rely on for re-runs + --offline).
    with tempfile.TemporaryDirectory() as td:
        man = Path(td) / "m.toml"
        man.write_text(manifest(SMALL_ASSET, _GOOD), encoding="utf-8")
        dest = Path(td) / "dest"
        cmd = [sys.executable, str(INSTALLER), "--manifest", str(man), "--dest", str(dest)]
        p1 = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
        p2 = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
        if (_download_failed(p1.returncode, p1.stdout + p1.stderr)
                or _download_failed(p2.returncode, p2.stdout + p2.stderr)):
            record(IDEMPOTENT_LEG, True, _UNFETCHABLE)
        else:
            ok = (p1.returncode == 0 and p2.returncode == 0
                  and "already verified" in (p2.stdout + p2.stderr))
            record(IDEMPOTENT_LEG, ok, f"{p1.returncode}/{p2.returncode}")

failed = [n for n, ok, _ in results if not ok]
print(f"\n[g24-install-gate-tools] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
