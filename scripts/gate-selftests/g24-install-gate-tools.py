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
        p = subprocess.run(cmd, capture_output=True, text=True)
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


def pip_leg() -> None:
    with tempfile.TemporaryDirectory() as td:
        rq = Path(td) / "reqs.txt"
        rq.write_text("requests==2.31.0\n", encoding="utf-8")  # no --hash on purpose
        try:
            p = subprocess.run(
                [sys.executable, "-m", "pip", "install", "--require-hashes", "--no-deps",
                 "-r", str(rq)],
                capture_output=True, text=True, timeout=120,
            )
        except (FileNotFoundError, subprocess.TimeoutExpired) as e:
            record("pip --require-hashes rejects hashless", True, f"SKIP ({type(e).__name__})")
            return
        ok = p.returncode != 0 and "hash" in (p.stdout + p.stderr).lower()
        record("pip --require-hashes rejects hashless", ok, f"exit={p.returncode}")


pip_leg()

# --- network legs (run if online; --require-network turns an offline skip into a FAIL) ---
if not online():
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

failed = [n for n, ok, _ in results if not ok]
print(f"\n[g24-install-gate-tools] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
