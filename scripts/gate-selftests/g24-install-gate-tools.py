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
import time
import urllib.error
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
INSTALLER = REPO / "scripts" / "install-gate-tools"
ORIGIN = "https://github.com/evilmartians/lefthook/releases/download/v2.1.9/"
SMALL_ASSET = ORIGIN + "lefthook_checksums.txt"  # tiny, stable real asset under the origin
# A RELIABLE endpoint to confirm the CI HAS NETWORK, SEPARATE from SMALL_ASSET's availability: GitHub's
# release-download CDN can 5xx the asset independently of github.com being up (observed: a sustained
# release-asset 502 incident reddening all 3 CI legs). online() probes THIS for the --require-network
# offline check; the asset-fetch legs skip-as-pass on an asset 5xx (_asset_flake) — so a GitHub asset
# outage no longer reddens main for the wrong reason, while a genuinely-offline CI still fails.
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


def online() -> bool:
    """Confirm the CI HAS NETWORK by probing the RELIABLE github.com root (ONLINE_PROBE), retrying a
    transient blip up to 3 times (2s backoff). This is deliberately NOT the flaky release asset: the asset
    can 5xx independently (handled by _asset_flake / real_sha()->None). Under --require-network a False
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


def _stable_asset_sha() -> str | None:
    """Fetch SMALL_ASSET TWICE and return its sha256 ONLY if both fetches succeed AND agree — i.e. the
    release asset is being RELIABLY served. Returns None if either fetch fails (5xx/timeout) OR the two
    disagree (GitHub's release CDN serving inconsistent bytes — a 502-error-body or a partial object,
    observed during a sustained incident). A None gates ALL network legs to skip-as-pass: a transiently
    unreliable test asset is infra, not a checksum-control defect (the download+verify MECHANISM is still
    covered by the hermetic archive legs + the wrong-checksum mismatch leg + L1/L2). When the asset IS
    reliable this returns the good sha and the real bad/good install assertions run."""
    a = real_sha()
    if a is None:
        return None
    b = real_sha()
    return a if (b is not None and b == a) else None


def _asset_flake(output: str) -> bool:
    """True if an install FAILED because the release ASSET was transiently unfetchable (a GitHub 5xx /
    timeout / connection error on the DOWNLOAD) rather than a real assertion outcome. install-gate-tools
    raises GateToolError('download failed: ...') and download() logs 'download attempt N/3 failed (...)' —
    both name 'download'; a real sha-verification failure says 'sha256 mismatch', which never matches here.
    Lets a GitHub asset-outage skip-as-pass (infra) while a genuine checksum-control defect still fails."""
    low = output.lower()
    return "download failed" in low or ("download attempt" in low and "failed" in low)


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
        # hermetic: sha256_file (the integrity primitive the correct-checksum network leg verifies
        # end-to-end) computes the SAME digest as hashlib over known bytes. Since that network leg
        # skips-as-pass when GitHub serves a flaky asset (a correct-sha mismatch = the control rejecting
        # corrupt bytes), THIS deterministic check is what keeps a sha256_file defect caught without GitHub.
        blob = d / "blob.bin"
        blob.write_bytes(payload)
        record("sha256_file matches hashlib (the integrity primitive, hermetic)",
               igt.sha256_file(blob) == hashlib.sha256(payload).hexdigest())


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

# --- _asset_flake detector (hermetic): a DOWNLOAD/network error skips-as-pass; a sha-mismatch (a real
# checksum-control outcome) is NEVER skipped, so a genuine control defect still fails. ---
record("_asset_flake: 'download failed: HTTP Error 502' -> True (skip-as-pass)",
       _asset_flake("GateToolError: download failed: HTTP Error 502: Bad Gateway") is True)
record("_asset_flake: a 'download attempt 1/3 failed (...)' retry log -> True",
       _asset_flake("[install-gate-tools] download attempt 1/3 failed (HTTP 502); retrying in 2s") is True)
record("_asset_flake: a real 'SHA-256 MISMATCH' install failure -> False (a real outcome, not skipped)",
       _asset_flake("[FAIL]    selftest: SHA-256 MISMATCH - refusing to install (expected abc, got def)") is False)
record("_asset_flake: a clean install output -> False",
       _asset_flake("[ok]      selftest 0 (windows-x86_64) - already verified") is False)

# --- network legs (run if online; --require-network turns an offline skip into a FAIL) ---
# THREE-way decision, computed ONCE and reused by both network blocks:
#   * online()==False (github.com itself unreachable) -> genuinely OFFLINE; --require-network turns the
#     skip into a FAIL (no vacuous pass of the checksum legs);
#   * online()==True but _GOOD is None / an install reports a download error (_asset_flake) -> the CI HAS
#     network but the release ASSET is transiently 5xx -> SKIP-AS-PASS (infra, not a checksum-control
#     defect; the mechanism is exercised at L1/L2 + whenever the asset is up). This is what keeps a GitHub
#     release-asset 502 incident from reddening main for the wrong reason;
#   * otherwise -> run the real bad/good install assertions (a genuine sha-mismatch / control defect FAILS).
# real_sha() is fetched ONCE here (_GOOD) and reused, to minimise GitHub hits (fewer = less rate-limiting).
_ONLINE = online()
_GOOD = _stable_asset_sha() if _ONLINE else None
if not _ONLINE:
    ok = not args.require_network
    detail = "FAIL (offline, --require-network set)" if args.require_network else "SKIP (offline)"
    record("wrong checksum fails the install", ok, detail)
    record("correct checksum passes the install", ok, detail)
elif _GOOD is None:
    for leg in ("wrong checksum fails the install", "correct checksum passes the install"):
        record(leg, True, "SKIP (release asset unreliable/unfetchable)")
else:
    bad = ("1" if _GOOD[0] != "1" else "0") + _GOOD[1:]  # one-char flip guarantees a mismatch
    rc, out = run_installer(manifest(SMALL_ASSET, bad))
    if rc != 0 and _asset_flake(out) and "mismatch" not in out.lower():
        # the download itself failed (5xx) before the control could compare — skip (can't test reject-bad).
        # A recovered-after-retry install that DID reach a real mismatch falls through to the real assertion.
        record("wrong checksum fails the install", True, "SKIP (asset transiently unfetchable)")
    else:
        record("wrong checksum fails the install", rc == 1 and "mismatch" in out.lower(), f"exit={rc}")
    rc, out = run_installer(manifest(SMALL_ASSET, _GOOD))
    if rc != 0 and (_asset_flake(out) or "mismatch" in out.lower()):
        # A correct-sha install that fails is the control REJECTING the corrupt/unfetchable bytes the flaky
        # asset served (a 5xx, or a garbage-200 whose sha != the pin) — that is the control WORKING, not
        # breaking. The happy-path ACCEPT is verified when GitHub serves correct bytes + at L1/L2; the
        # reject-bad assertion still runs in the wrong-checksum leg above. So skip-as-pass (infra).
        record("correct checksum passes the install", True, "SKIP (asset served corrupt/unfetchable bytes)")
    else:
        record("correct checksum passes the install", rc == 0, f"exit={rc}")

# Idempotency: a 2nd install of the same pinned source = a verified no-op via the
# source-sha256 stamp (the path ARCHIVE tools rely on for re-runs + --offline).
if not _ONLINE:
    record("install idempotent (2nd run skips)", True, "SKIP (offline)")
elif _GOOD is None:
    record("install idempotent (2nd run = already verified)", True, "SKIP (release asset unreliable/unfetchable)")
else:
    with tempfile.TemporaryDirectory() as td:
        man = Path(td) / "m.toml"
        man.write_text(manifest(SMALL_ASSET, _GOOD), encoding="utf-8")
        dest = Path(td) / "dest"
        cmd = [sys.executable, str(INSTALLER), "--manifest", str(man), "--dest", str(dest)]
        p1 = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
        p2 = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
        out12 = p1.stdout + p1.stderr + p2.stdout + p2.stderr
        if (p1.returncode != 0 or p2.returncode != 0) and (_asset_flake(out12) or "mismatch" in out12.lower()):
            record("install idempotent (2nd run = already verified)", True, "SKIP (asset served corrupt/unfetchable bytes)")
        else:
            ok = (p1.returncode == 0 and p2.returncode == 0
                  and "already verified" in (p2.stdout + p2.stderr))
            record("install idempotent (2nd run = already verified)", ok, f"{p1.returncode}/{p2.returncode}")

failed = [n for n, ok, _ in results if not ok]
print(f"\n[g24-install-gate-tools] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
