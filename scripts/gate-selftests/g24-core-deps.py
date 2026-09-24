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
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

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

# --- the §0.8 flate2 backend selection (the feature plane) -------------------------------------
# The motivating incident, replayed verbatim: flate2 1.1.10's feature table + the ACTIVATED features and
# the resolve deps `cargo metadata --locked` reported on 2026-09-24 - `zlib-rs` is on the deps list (the
# weak `zlib-rs?/std` in `default` puts it into the lock) yet NOT activated. Each clause of the leg then
# gets a mutant-discriminating leg: read the deps list instead of the features -> the replay reds; drop the
# subset check -> the zlib-rs / libz-sys activations pass; count a weak `x?/feat` as an activation -> the
# replay reds; drop the missing-`rust_backend` guard -> that leg passes silently; drop the positive
# miniz_oxide requirement -> the re-pointed-`rust_backend` leg passes (the G1 review P1); treat a missing
# `features` record as "nothing activated" -> that leg passes.
FLATE2_1_1_10_TABLE = {
    "any_c_zlib": ["any_zlib"], "any_impl": [], "any_zlib": ["any_impl"], "cloudflare_zlib": ["zlib"],
    "default": ["rust_backend", "runtime_detection"], "document-features": ["dep:document-features"],
    "libz-ng-sys": ["dep:libz-ng-sys"], "libz-sys": ["dep:libz-sys"], "miniz-sys": ["rust_backend"],
    "miniz_oxide": ["any_impl", "dep:miniz_oxide", "dep:crc32fast"],
    "runtime_detection": ["zlib-rs?/std", "crc32fast?/std"], "rust_backend": ["miniz_oxide", "any_impl"],
    "zlib": ["any_c_zlib", "libz-sys", "dep:crc32fast"], "zlib-default": ["any_c_zlib", "libz-sys/default", "dep:crc32fast"],
    "zlib-ng": ["any_c_zlib", "libz-ng-sys", "dep:crc32fast"], "zlib-ng-compat": ["zlib", "libz-sys/zlib-ng", "dep:crc32fast"],
    "zlib-rs": ["any_zlib", "dep:zlib-rs"],
}
FLATE2_1_1_10_ACTIVATED = ["any_impl", "default", "miniz_oxide", "runtime_detection", "rust_backend"]
FLATE2_1_1_10_DEPS = ["crc32fast", "miniz_oxide", "zlib-rs"]
FLATE2_ID = "flate2 1.1.10"


def meta_flate2(activated: list[str], table: dict = FLATE2_1_1_10_TABLE, deps: list[str] = FLATE2_1_1_10_DEPS,
                in_closure: bool = True) -> dict:
    """A synthetic `cargo metadata` with the core, flate2 (its feature table + activated features + the
    resolve deps) and the crates it names; `in_closure=False` leaves flate2 out of the core's edges."""
    names = ["convertia-core", "png"] + deps
    packages = [{"id": f"{n} 0.0.0", "name": n} for n in names] + [{"id": FLATE2_ID, "name": "flate2", "features": table}]
    nodes = {p["id"]: {"id": p["id"], "deps": [], "features": []} for p in packages}
    nodes["convertia-core 0.0.0"]["deps"].append({"pkg": "png 0.0.0"})
    if in_closure:
        nodes["png 0.0.0"]["deps"].append({"pkg": FLATE2_ID})
    nodes[FLATE2_ID]["deps"] = [{"pkg": f"{d} 0.0.0"} for d in deps]
    nodes[FLATE2_ID]["features"] = list(activated)
    return {"packages": packages, "resolve": {"nodes": list(nodes.values())}}


def _f2(md: dict) -> tuple[list[str], list[str]]:
    return m.flate2_backend_problems(md, m.core_closure_ids(md) or set())


_replay = _f2(meta_flate2(FLATE2_1_1_10_ACTIVATED))
record("REPLAY 2026-09-24: flate2 1.1.10 with `zlib-rs` on its resolve deps but NOT activated -> clean, and "
       "the leg EVALUATED it (non-vacuous)", _replay == ([], [FLATE2_ID]))
_zrs = _f2(meta_flate2(FLATE2_1_1_10_ACTIVATED + ["zlib-rs"]))
record("flate2 with the `zlib-rs` feature ACTIVATED -> caught, naming zlib-rs",
       len(_zrs[0]) == 1 and "'zlib-rs'" in _zrs[0][0])
_czl = _f2(meta_flate2(["default", "zlib"], deps=FLATE2_1_1_10_DEPS + ["libz-sys"]))
record("flate2 with the C `zlib` feature ACTIVATED (libz-sys) -> caught, naming libz-sys",
       len(_czl[0]) == 1 and "'libz-sys'" in _czl[0][0])
_zdef = _f2(meta_flate2(["zlib-default"], deps=FLATE2_1_1_10_DEPS + ["libz-sys"]))
record("a `x/feat` entry (zlib-default -> libz-sys/default) counts as activating x -> caught (and, with "
       "miniz_oxide not turned on at all, that positive requirement fires beside it)",
       len(_zdef[0]) == 2 and any("'libz-sys'" in p for p in _zdef[0]) and any("do not turn on `miniz_oxide`" in p for p in _zdef[0]))
_norb = _f2(meta_flate2(FLATE2_1_1_10_ACTIVATED, table={k: v for k, v in FLATE2_1_1_10_TABLE.items() if k != "rust_backend"}))
record("a flate2 whose feature table has NO `rust_backend` -> fail-closed (cannot evaluate the selection)",
       len(_norb[0]) == 1 and "cannot be evaluated" in _norb[0][0])
record("flate2 OUTSIDE the core closure is not evaluated (no problem, nothing evaluated)",
       _f2(meta_flate2(FLATE2_1_1_10_ACTIVATED + ["zlib-rs"], in_closure=False)) == ([], []))
record("_feature_closure: the weak `x?/feat` form turns on nothing; `dep:x` and `x/feat` turn on x",
       m._feature_closure({"a": ["b?/std", "dep:c", "d/e"]}, {"a"}) == ({"a"}, {"c", "d"}))
_REPOINTED = {**FLATE2_1_1_10_TABLE, "rust_backend": ["zlib-rs", "any_impl"],
              "zlib-rs": ["any_zlib", "dep:zlib-rs", "miniz_oxide?/std"]}
_rp = _f2(meta_flate2(["any_impl", "any_zlib", "default", "runtime_detection", "rust_backend", "zlib-rs"],
                      table=_REPOINTED))
record("REVIEW P1 replay: a flate2 whose `rust_backend` is re-pointed at zlib-rs (miniz_oxide only weakly) "
       "-> caught, naming miniz_oxide (the row names the crate, not just the feature)",
       len(_rp[0]) == 1 and "no longer turns on `miniz_oxide`" in _rp[0][0])
_nofeat = meta_flate2(FLATE2_1_1_10_ACTIVATED)
for _n in _nofeat["resolve"]["nodes"]:
    if _n["id"] == FLATE2_ID:
        del _n["features"]
record("a flate2 resolve node WITHOUT a `features` record -> fail-closed (cannot evaluate), not \"nothing activated\"",
       any("no `features` record" in p for p in _f2(_nofeat)[0]))

# --- the fixture is structurally present + planted --------------------------------------------
core_toml = FIXTURE / "convertia-core" / "Cargo.toml"
record("tests/g53-fixture workspace + core + libvips-sys crates exist",
       (FIXTURE / "Cargo.toml").is_file() and core_toml.is_file()
       and (FIXTURE / "libvips-sys" / "Cargo.toml").is_file())
record("the fixture core crate declares the planted libvips-sys dependency",
       "libvips-sys" in core_toml.read_text(encoding="utf-8"))

# --- regression guard (P1.12): the live `cargo metadata` read MUST pin encoding="utf-8" -------
# Once the tauri dep tree (~250 crates / ~900 KB metadata) landed, a non-ASCII byte in a crate
# description cp1252-crashed check-core-deps on Windows (text=True without encoding). main()'s skip
# path is covered by _skip_when_cargo_absent below + the live walk by the fixture leg below, so
# main() is NOT re-run live here (heavy + plane-dependent on a real cargo).
import inspect as _inspect
record("regression: check-core-deps' live `cargo metadata` read pins encoding=\"utf-8\" "
       "(a non-ASCII byte in the ~900 KB metadata must not cp1252-crash on Windows; P1.12)",
       'encoding="utf-8"' in _inspect.getsource(m.main))

# --- live: run the REAL fixture through cargo metadata when cargo is installed (P1) ------------
if shutil.which("cargo"):
    proc = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--manifest-path", str(FIXTURE / "Cargo.toml")],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
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

# --- main() WIRING (r4 review, P3): the pure legs above prove each clause; these prove main() ACTS on both
# branches (a disconnected `if hits:` / `if f2_problems:` would print OK and exit 0). `cargo metadata` is
# monkeypatched to return the synthetic metadata; the real Cargo.toml + a fake `cargo` on PATH drive main().
def _main_over(md: dict) -> int:
    saved = (m.shutil.which, m.subprocess.run)
    m.shutil.which = lambda tool: "cargo"
    m.subprocess.run = lambda *a, **k: subprocess.CompletedProcess(a, 0, stdout=json.dumps(md), stderr="")
    try:
        return m.main()
    finally:
        m.shutil.which, m.subprocess.run = saved


record("main(): the replayed flate2 1.1.10 metadata (zlib-rs on the deps, not activated) -> exit 0",
       _main_over(meta_flate2(FLATE2_1_1_10_ACTIVATED)) == 0)
record("main(): flate2 with `zlib-rs` ACTIVATED -> exit 1 (the flate2 branch is wired, not only pure)",
       _main_over(meta_flate2(["zlib-rs"])) == 1)
record("main(): libvips-sys in the core closure -> exit 1 (the forbidden-stem branch is wired)",
       _main_over(meta(("convertia-core", "libvips-sys"), names=("convertia-core", "libvips-sys"))) == 1)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-core-deps] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
