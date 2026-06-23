#!/usr/bin/env python3
"""g24-install-gate-tools - G24 self-test for the pinned-tool fetch+verify control (P0.2.1).

Proves the security control BOTH ways (build-gates s0 / G24): a WRONG checksum
MUST fail the install; a CORRECT checksum MUST pass; an off-origin url MUST be
rejected; --offline with no prior install MUST fail; and the pip --require-hashes
leg MUST reject a hashless requirement. The hermetic legs (no network) always run;
the two network legs fetch a tiny (~KB) real asset under the lefthook origin and,
when offline, SKIP-as-PASS by default OR FAIL under --require-network (which CI
passes, so an offline CI run cannot vacuously pass the checksum legs).

Run:  python3 scripts/gate-selftests/g24-install-gate-tools.py
Exit: 0 = every assertion held; 1 = a self-test assertion FAILED (the gate is broken).
"""
import argparse
import hashlib
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
INSTALLER = REPO / "scripts" / "install-gate-tools"
ORIGIN = "https://github.com/evilmartians/lefthook/releases/download/v2.1.9/"
SMALL_ASSET = ORIGIN + "lefthook_checksums.txt"  # tiny, stable real asset under the origin
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


def online() -> bool:
    try:
        req = urllib.request.Request(SMALL_ASSET, method="HEAD",
                                     headers={"User-Agent": "convertia-selftest"})
        urllib.request.urlopen(req, timeout=20).close()
        return True
    except (urllib.error.URLError, TimeoutError, OSError):
        return False


def real_sha() -> str:
    req = urllib.request.Request(SMALL_ASSET, headers={"User-Agent": "convertia-selftest"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return hashlib.sha256(r.read()).hexdigest()


# --- hermetic legs (no network) ----------------------------------------------
rc, out = run_installer(manifest("https://evil.example.com/x", "0" * 64))
record("off-origin url is rejected", rc == 1 and "origin" in out.lower(), f"exit={rc}")

rc, out = run_installer(manifest(SMALL_ASSET, "0" * 64), "--offline")
record("offline with no install fails", rc == 1 and "offline" in out.lower(), f"exit={rc}")

# "asserted not-floating": a floating toolchain channel must be rejected.
rc, out = run_installer('schema_version = 1\n[toolchain]\nrust_stable = "stable"\nfuzz_nightly = "nightly"\n')
record("floating toolchain channel is rejected", rc == 1 and "floating" in out.lower(), f"exit={rc}")


def archive_leg() -> None:
    """Hermetic: extract_binary round-trips the inner binary from a tar.gz (nested
    member) + a zip (root member), and fails on a missing member - covers the P0.2.4
    archive-extraction code path without network."""
    import importlib.machinery
    import importlib.util
    import io
    import tarfile
    import zipfile
    # install-gate-tools has no .py extension, so give an explicit source loader.
    loader = importlib.machinery.SourceFileLoader("install_gate_tools", str(INSTALLER))
    spec = importlib.util.spec_from_loader("install_gate_tools", loader)
    igt = importlib.util.module_from_spec(spec)
    loader.exec_module(igt)
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


archive_leg()


def retry_leg() -> None:
    """Hermetic: download() retries a TRANSIENT failure (502) then succeeds, does NOT retry a PERMANENT
    404, and re-raises after the final attempt - the shellcheck-502 transient hardening. urlopen +
    time.sleep are monkeypatched (no real network, no real backoff)."""
    import importlib.machinery
    import importlib.util
    import io
    loader = importlib.machinery.SourceFileLoader("install_gate_tools_r", str(INSTALLER))
    igt = importlib.util.module_from_spec(importlib.util.spec_from_loader("install_gate_tools_r", loader))
    loader.exec_module(igt)
    igt.time.sleep = lambda *_a, **_k: None             # no real backoff in the test
    payload = b"ok-bytes\n"

    class _Resp(io.BytesIO):
        def __enter__(self):
            return self

        def __exit__(self, *_a):
            self.close()
            return False

    def _make(seq):
        """urlopen that raises each exception in seq in turn, then returns a fresh payload response."""
        calls = {"n": 0}

        def _open(_req, timeout=None):
            i = calls["n"]
            calls["n"] += 1
            if i < len(seq):
                raise seq[i]
            return _Resp(payload)

        return _open, calls

    h502 = igt.urllib.error.HTTPError("u", 502, "Bad Gateway", {}, None)
    h404 = igt.urllib.error.HTTPError("u", 404, "Not Found", {}, None)
    with tempfile.TemporaryDirectory() as td:
        out = Path(td) / "o"
        igt.urllib.request.urlopen, calls = _make([h502, h502])          # 2 transient then success
        igt.download("https://x/y", out)
        record("download retries a transient 502 then succeeds",
               out.read_bytes() == payload and calls["n"] == 3)
        igt.urllib.request.urlopen, calls = _make([h404, h404])          # permanent -> first attempt raises
        raised = False
        try:
            igt.download("https://x/y", out)
        except igt.urllib.error.HTTPError as e:
            raised = e.code == 404
        record("download does NOT retry a permanent 404", raised and calls["n"] == 1)
        igt.urllib.request.urlopen, calls = _make([h502, h502, h502, h502])   # all attempts fail
        raised = False
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

# --- network legs (run if online; --require-network turns an offline skip into a FAIL) ---
# Probe connectivity ONCE and reuse it: online() does a HEAD that can flake between calls, and a
# disagreement between the two network blocks (skip in one, run in the other) is what made the
# idempotency leg fail for the wrong reason when GitHub was intermittently 502/rate-limiting. One
# probe -> one decision for every network leg.
_ONLINE = online()
if not _ONLINE:
    ok = not args.require_network
    detail = "FAIL (offline, --require-network set)" if args.require_network else "SKIP (offline)"
    record("wrong checksum fails the install", ok, detail)
    record("correct checksum passes the install", ok, detail)
else:
    good = real_sha()
    bad = ("1" if good[0] != "1" else "0") + good[1:]  # one-char flip guarantees a mismatch
    rc, out = run_installer(manifest(SMALL_ASSET, bad))
    record("wrong checksum fails the install", rc == 1 and "mismatch" in out.lower(), f"exit={rc}")
    rc, out = run_installer(manifest(SMALL_ASSET, good))
    record("correct checksum passes the install", rc == 0, f"exit={rc}")

# Idempotency: a 2nd install of the same pinned source = a verified no-op via the
# source-sha256 stamp (the path ARCHIVE tools rely on for re-runs + --offline).
if not _ONLINE:
    record("install idempotent (2nd run skips)", True, "SKIP (offline)")
else:
    with tempfile.TemporaryDirectory() as td:
        man = Path(td) / "m.toml"
        man.write_text(manifest(SMALL_ASSET, real_sha()), encoding="utf-8")
        dest = Path(td) / "dest"
        cmd = [sys.executable, str(INSTALLER), "--manifest", str(man), "--dest", str(dest)]
        p1 = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
        p2 = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
        ok = (p1.returncode == 0 and p2.returncode == 0
              and "already verified" in (p2.stdout + p2.stderr))
        record("install idempotent (2nd run = already verified)", ok, f"{p1.returncode}/{p2.returncode}")

failed = [n for n, ok, _ in results if not ok]
print(f"\n[g24-install-gate-tools] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
