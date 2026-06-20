#!/usr/bin/env python3
"""g24-core-deps.py - G24 self-test for check-core-deps (P0.3.7, T6/G53).

Proves the core-crate forbidden-dependency walk: a forbidden image-worker C lib (libvips/libheif/
librsvg/libimagequant) anywhere in the MIT core's transitive closure is caught; a clean closure passes;
a core crate absent from the metadata returns None (cannot-evaluate). Drives the pure walk against
synthetic `cargo metadata` JSON (no cargo needed), confirms the tests/g53-fixture/ negative fixture is
structurally present + planted, and - when cargo is installed (P1) - runs the REAL fixture through
`cargo metadata` and asserts the planted libvips-sys dep is flagged. stdlib-only.
Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import json
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check-core-deps"
FIXTURE = ROOT / "tests" / "g53-fixture"
_loader = importlib.machinery.SourceFileLoader("ccd", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("ccd", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def meta(*edges: tuple[str, str], names: tuple[str, ...]) -> dict:
    """Build a synthetic `cargo metadata` dict: `names` are the packages, `edges` are (from, to) dep
    links (by name). Ids are 'NAME 0.0.0'."""
    pid = lambda n: f"{n} 0.0.0"
    packages = [{"id": pid(n), "name": n} for n in names]
    nodes = {n: {"id": pid(n), "deps": []} for n in names}
    for frm, to in edges:
        nodes[frm]["deps"].append({"pkg": pid(to)})
    return {"packages": packages, "resolve": {"nodes": list(nodes.values())}}


# --- pure closure walk ------------------------------------------------------------------------
violation = meta(("convertia-core", "libvips-sys"), names=("convertia-core", "libvips-sys"))
record("core -> libvips-sys in closure -> forbidden hit",
       m.forbidden_in_closure(m.core_closure(violation)) == ["libvips-sys"])

transitive = meta(("convertia-core", "img-helper"), ("img-helper", "libheif-sys"),
                  names=("convertia-core", "img-helper", "libheif-sys"))
record("TRANSITIVE core -> img-helper -> libheif-sys -> caught (full closure, not just direct deps)",
       m.forbidden_in_closure(m.core_closure(transitive)) == ["libheif-sys"])

clean = meta(("convertia-core", "serde"), ("serde", "serde_derive"),
             names=("convertia-core", "serde", "serde_derive"))
record("clean closure (serde only) -> no forbidden hit", m.forbidden_in_closure(m.core_closure(clean)) == [])

record("each forbidden binding (the §3.6.1 set) is caught",
       all(m.forbidden_in_closure({"convertia-core", dep}) == [dep] for dep in
           ("libvips-sys", "libheif-sys", "librsvg-sys", "libimagequant", "rsvg")))
record("libde265-sys (§3.6.1 LGPL HEVC decoder, paired with libheif) -> caught (G1 review P1 fix)",
       m.forbidden_in_closure({"convertia-core", "libde265-sys"}) == ["libde265-sys"])
record("an ImageMagick binding (magick-rust) -> caught (G1 review P2; image-worker delegate, §2.12)",
       m.forbidden_in_closure({"convertia-core", "magick-rust"}) == ["magick-rust"])

record("core crate ABSENT from metadata -> core_closure returns None (cannot-evaluate)",
       m.core_closure(meta(("a", "b"), names=("a", "b"))) is None)

record("the core crate itself is in its closure but is NOT a forbidden hit",
       "convertia-core" not in m.forbidden_in_closure(m.core_closure(clean)))

# --- the fixture is structurally present + planted --------------------------------------------
core_toml = FIXTURE / "convertia-core" / "Cargo.toml"
record("tests/g53-fixture workspace + core + libvips-sys crates exist",
       (FIXTURE / "Cargo.toml").is_file() and core_toml.is_file()
       and (FIXTURE / "libvips-sys" / "Cargo.toml").is_file())
record("the fixture core crate declares the planted libvips-sys dependency",
       "libvips-sys" in core_toml.read_text(encoding="utf-8"))

# --- main() is target-absent today (no repo-root/src-tauri workspace) -------------------------
record("check-core-deps main() exits 0 today (no workspace yet -> target-absent skip)", m.main() == 0)

# --- live: run the REAL fixture through cargo metadata when cargo is installed (P1) ------------
if shutil.which("cargo"):
    proc = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--manifest-path", str(FIXTURE / "Cargo.toml")],
        capture_output=True, text=True)
    if proc.returncode == 0:
        live = json.loads(proc.stdout)
        record("LIVE fixture: cargo metadata core closure flags libvips-sys",
               "libvips-sys" in m.forbidden_in_closure(m.core_closure(live) or set()))
    else:
        print(f"[g24-core-deps] cargo present but `cargo metadata` on the fixture failed "
              f"(env/offline) - skipping the live leg:\n{proc.stderr.strip()[:200]}")
else:
    print("[g24-core-deps] cargo not installed - skipping the live fixture leg (P1 activates it)")

# --- the P1-runway fix: a Cargo manifest present but cargo absent in this plane -> SKIP (0), not the
# old hard FAIL (the live walk enforces at L1/L2 + the equipped Rust CI job; skip-here / enforce-there) -
def _skip_when_cargo_absent() -> int:
    import tempfile
    saved = (m.ROOT, m.CARGO_TOML_CANDIDATES, m.shutil.which)
    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        (root / "Cargo.toml").write_text('[package]\nname = "x"\n', encoding="utf-8")
        m.ROOT = root
        m.CARGO_TOML_CANDIDATES = (root / "Cargo.toml", root / "src-tauri" / "Cargo.toml")
        m.shutil.which = lambda tool: None
        try:
            return m.main()
        finally:
            m.ROOT, m.CARGO_TOML_CANDIDATES, m.shutil.which = saved


record("main(): Cargo manifest present but cargo absent in this plane -> SKIP (0), not the old fail "
       "(P1-runway fix; the live walk enforces where cargo is present)",
       _skip_when_cargo_absent() == 0)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-core-deps] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
