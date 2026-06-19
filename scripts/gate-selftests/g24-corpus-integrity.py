#!/usr/bin/env python3
"""g24-corpus-integrity.py - G24 self-test for check-corpus-integrity (P0.5.4, G24a).

Proves the structural freeze cannot be weakened, and the corpus-integrity checks CATCH each violation
(a poisoned/swapped fixture sha256 mismatch, a missing/non-redistributable licence or provenance, an
un-manifested corpus byte, an un-tracked LFS-tier fixture, a repointed `.lfsconfig`) while PASSING a
clean manifest+corpus - incl. the unresolved-LFS-pointer `oid` integrity path. The live verification is
target-absent today (no `tests/corpus/manifest.toml`). stdlib-only, git-free (the git legs are exercised
via injected runners / monkeypatched defaults). Exit 0 = held.
"""
import hashlib
import importlib.machinery
import importlib.util
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-corpus-integrity"
_loader = importlib.machinery.SourceFileLoader("cci", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cci", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def sha(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def lfs_pointer(oid: str, size: int = 10) -> bytes:
    return (f"version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {size}\n").encode()


def _freeze_with(**ov):
    saved = {k: getattr(m, k) for k in ov}
    for k, v in ov.items():
        setattr(m, k, v)
    try:
        return m.frozen_contract()
    finally:
        for k, v in saved.items():
            setattr(m, k, v)


def good_row(path="a.png", body=b"hello"):
    return {"path": path, "sha256": sha(body), "licence": "CC0", "provenance": "self-produced"}


# --- the structural freeze ---------------------------------------------------------------------
record("freeze: real constants are consistent", m.frozen_contract() == [])
record("freeze: a non-redistributable licence sneaking into the allow-list is caught",
       len(_freeze_with(REDISTRIBUTABLE_LICENCES=frozenset({"CC0", "GPL"}))) >= 1)
record("freeze: a repointed canonical LFS endpoint is caught",
       len(_freeze_with(CANONICAL_LFS_ENDPOINT="https://evil.example/lfs")) >= 1)
record("freeze: dropping `sha256` from the required fields is caught",
       len(_freeze_with(REQUIRED_FIELDS=("path", "licence", "provenance"))) >= 1)

# --- (2) row fields + licence ------------------------------------------------------------------
record("row: a clean row passes", m.check_row_fields(good_row()) == [])
record("row: a missing provenance is caught",
       len(m.check_row_fields({"path": "a", "sha256": "x", "licence": "CC0"})) >= 1)
record("row: an empty provenance is caught",
       len(m.check_row_fields({"path": "a", "sha256": "x", "licence": "CC0", "provenance": "  "})) >= 1)
record("row: a non-redistributable licence (GPL) is caught",
       any("redistributable" in p for p in m.check_row_fields(
           {"path": "a", "sha256": "x", "licence": "GPL", "provenance": "url"})))
record("row: each redistributable licence is accepted",
       all(m.check_row_fields({"path": "a", "sha256": "x", "licence": L, "provenance": "p"}) == []
           for L in ("public-domain", "CC0", "self-produced", "synthetic")))

# --- content_sha256: bytes vs LFS pointer ------------------------------------------------------
record("sha: real bytes -> content hash", m.content_sha256(b"hello") == sha(b"hello"))
record("sha: an unresolved LFS pointer -> its oid (not the pointer-text hash)",
       m.content_sha256(lfs_pointer("a" * 64)) == "a" * 64)
record("sha: is_lfs_pointer distinguishes a pointer from real bytes",
       m.is_lfs_pointer(lfs_pointer("b" * 64)) and not m.is_lfs_pointer(b"\x89PNG real bytes"))

# --- (1) verify_file: sha256 integrity + LFS-tier detection ------------------------------------
with tempfile.TemporaryDirectory() as td:
    cr = Path(td)
    (cr / "a.png").write_bytes(b"hello")
    record("verify: a matching in-repo fixture passes (is_lfs=False)",
           m.verify_file(cr, good_row("a.png", b"hello")) == ([], False))
    (cr / "bad.png").write_bytes(b"tampered")
    r = m.verify_file(cr, {"path": "bad.png", "sha256": sha(b"original"),
                           "licence": "CC0", "provenance": "p"})
    record("verify: a swapped/poisoned fixture (sha mismatch) is caught", len(r[0]) == 1)
    record("verify: a manifest row with no on-disk file is caught (dangling)",
           len(m.verify_file(cr, good_row("missing.png", b"x"))[0]) == 1)
    oid = "c" * 64
    (cr / "big.bin").write_bytes(lfs_pointer(oid))
    record("verify: an unresolved-LFS fixture matches via oid + is flagged LFS-tier (is_lfs=True)",
           m.verify_file(cr, {"path": "big.bin", "sha256": oid, "licence": "CC0", "provenance": "p"})
           == ([], True))

# --- (4) LFS tracking + .lfsconfig endpoint ----------------------------------------------------
record("lfs-track: an lfs-tracked path passes (runner True)",
       m.check_lfs_tracking(Path("."), "tests/corpus", "big.bin", lambda r, p: True) == [])
record("lfs-track: an UN-tracked LFS-tier path is caught (runner False)",
       len(m.check_lfs_tracking(Path("."), "tests/corpus", "big.bin", lambda r, p: False)) == 1)
record("lfs-track: git unavailable -> fail-closed (runner None)",
       len(m.check_lfs_tracking(Path("."), "tests/corpus", "big.bin", lambda r, p: None)) == 1)
with tempfile.TemporaryDirectory() as td:
    root = Path(td)
    record("lfsconfig: absent -> clean", m.check_lfsconfig(root) == [])
    (root / ".lfsconfig").write_text(f"[lfs]\n\turl = {m.CANONICAL_LFS_ENDPOINT}\n", encoding="utf-8")
    record("lfsconfig: the canonical endpoint passes", m.check_lfsconfig(root) == [])
    (root / ".lfsconfig").write_text("[lfs]\n\turl = https://evil.example/lfs\n", encoding="utf-8")
    record("lfsconfig: a repointed endpoint is caught", len(m.check_lfsconfig(root)) == 1)
# G1 P1: the endpoint guard must catch EVERY LFS-endpoint key (a `pushurl` / `remote.*.lfsurl` repoint
# was a fail-OPEN), be section-aware (a git-remote `url` is NOT an LFS endpoint), and unquote.
CAN = m.CANONICAL_LFS_ENDPOINT
record("lfsconfig: a `lfs.pushurl` repoint is caught",
       m.parse_lfsconfig_endpoints("[lfs]\npushurl = https://evil.example/lfs\n") == ["https://evil.example/lfs"])
record("lfsconfig: a `remote.origin.lfsurl` repoint is caught (the FETCH-tampering key)",
       m.parse_lfsconfig_endpoints('[remote "origin"]\nlfsurl = https://evil.example/lfs\n') == ["https://evil.example/lfs"])
record("lfsconfig: a `remote.origin.lfspushurl` repoint is caught",
       m.parse_lfsconfig_endpoints('[remote "origin"]\nlfspushurl = https://evil.example/lfs\n') == ["https://evil.example/lfs"])
record("lfsconfig: a git-remote `url` under [remote] is NOT an LFS endpoint (no false-flag)",
       m.parse_lfsconfig_endpoints('[remote "origin"]\nurl = https://github.com/Ne-IA/convertia.git\n') == [])
record("lfsconfig: a named `[lfs \"x\"]` section url is collected",
       m.parse_lfsconfig_endpoints('[lfs "x"]\nurl = https://evil.example/lfs\n') == ["https://evil.example/lfs"])
record("lfsconfig: a QUOTED canonical endpoint passes (no false-flag; unquoted)",
       m.parse_lfsconfig_endpoints(f'[lfs]\nurl = "{CAN}"\n') == [CAN])
record("lfsconfig: an inline `# comment` after the url is stripped",
       m.parse_lfsconfig_endpoints(f"[lfs]\nurl = {CAN} # primary\n") == [CAN])
record("lfsconfig: a same-line `[lfs] url = ...` is parsed",
       m.parse_lfsconfig_endpoints(f"[lfs] url = {CAN}\n") == [CAN])
# G1 re-review P1: an [include]/[includeIf] directive (git-lfs honours it, possibly into an un-committed
# file outside the reviewed tree) cannot be statically followed -> check_lfsconfig must HARD-FAIL.
with tempfile.TemporaryDirectory() as td:
    root = Path(td)
    (root / ".lfsconfig").write_text("[include]\n\tpath = sneaky.cfg\n", encoding="utf-8")
    record("lfsconfig: an [include] directive is disallowed (hard-fail, cannot be statically verified)",
           len(m.check_lfsconfig(root)) >= 1)
    (root / ".lfsconfig").write_text('[includeIf "gitdir:**"]\n\tpath = .git/x.cfg\n', encoding="utf-8")
    record("lfsconfig: an [includeIf] directive (incl. into .git/) is disallowed",
           len(m.check_lfsconfig(root)) >= 1)
    (root / ".lfsconfig").write_text(f"[lfs]\n\turl = {CAN}\n", encoding="utf-8")
    record("lfsconfig: a direct canonical pin (no include) still passes (no false-fail)",
           m.check_lfsconfig(root) == [])

# --- (3) bijection -----------------------------------------------------------------------------
record("bijection: every tracked fixture manifested -> clean",
       m.check_bijection(Path("."), "tests/corpus", {"a.png"},
                         lambda r, s: ["tests/corpus/a.png", "tests/corpus/manifest.toml"]) == [])
record("bijection: an un-manifested committed fixture is caught",
       len(m.check_bijection(Path("."), "tests/corpus", {"a.png"},
                             lambda r, s: ["tests/corpus/a.png", "tests/corpus/ghost.png"])) == 1)
record("bijection: git unavailable -> fail-closed",
       len(m.check_bijection(Path("."), "tests/corpus", {"a.png"}, lambda r, s: None)) == 1)
# G1 P1: a real fixture named `manifest.toml` in a SUBDIR must NOT be excluded (basename bug); the ROOT
# manifest.toml IS excluded; and the bijection logic handles a RAW unicode path (the -z runner fix).
record("bijection: a subdir `sub/manifest.toml` fixture is NOT excluded (caught when un-manifested)",
       len(m.check_bijection(Path("."), "tests/corpus", set(),
                             lambda r, s: ["tests/corpus/sub/manifest.toml"])) == 1)
record("bijection: the ROOT `manifest.toml` IS excluded (not a fixture)",
       m.check_bijection(Path("."), "tests/corpus", set(),
                         lambda r, s: ["tests/corpus/manifest.toml"]) == [])
record("bijection: a RAW unicode un-manifested fixture is caught (no quoting in the runner output)",
       len(m.check_bijection(Path("."), "tests/corpus", set(),
                             lambda r, s: ["tests/corpus/café_evil.png"])) == 1)

# --- end-to-end (target-absent + a clean manifest, git legs monkeypatched) ---------------------
with tempfile.TemporaryDirectory() as td:
    record("e2e: no manifest -> target-absent (exit 0)", m.main(["--root", td]) == 0)
with tempfile.TemporaryDirectory() as td:
    root = Path(td)
    (root / "tests" / "corpus").mkdir(parents=True)
    body = b"\x89PNG fixture bytes"
    (root / "tests" / "corpus" / "a.png").write_bytes(body)
    (root / "tests" / "corpus" / "manifest.toml").write_text(
        f'[[file]]\npath = "a.png"\nsha256 = "{sha(body)}"\nlicence = "CC0"\n'
        'provenance = "self-produced"\ncovers = [["PNG","JPG"]]\n', encoding="utf-8")
    saved_ls, saved_attr = m._default_ls_files, m._default_check_attr
    m._default_ls_files = lambda r, s: ["tests/corpus/a.png", "tests/corpus/manifest.toml"]
    m._default_check_attr = lambda r, p: True
    try:
        record("e2e: a clean manifest + matching in-repo fixture -> exit 0", m.main(["--root", str(root)]) == 0)
        # poison the fixture
        (root / "tests" / "corpus" / "a.png").write_bytes(b"tampered bytes")
        record("e2e: a poisoned fixture (sha mismatch) -> exit 1", m.main(["--root", str(root)]) == 1)
    finally:
        m._default_ls_files, m._default_check_attr = saved_ls, saved_attr

record("e2e: the real repo passes (no corpus manifest yet -> target-absent)", m.main([]) == 0)

# G1 P1 end-to-end (real git): a committed UNICODE-named fixture NOT in the manifest must be CAUGHT by
# the bijection - proves the `-z` / core.quotePath=false runner fix against real `git ls-files` quoting.
def _git_unicode_uncovered():
    import os
    import shutil
    import subprocess as sp
    if not shutil.which("git"):
        return None                                  # git absent -> skip (the gate needs git anyway)
    td = tempfile.mkdtemp()
    try:
        root = Path(td)
        cdir = root / "tests" / "corpus"
        cdir.mkdir(parents=True)
        (cdir / "ok.png").write_bytes(b"ok")
        (cdir / "café_evil.png").write_bytes(b"\x89PNG cafe")   # an UN-manifested unicode fixture
        (cdir / "manifest.toml").write_text(
            f'[[file]]\npath = "ok.png"\nsha256 = "{sha(b"ok")}"\nlicence = "CC0"\nprovenance = "p"\n',
            encoding="utf-8")
        env = {**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t",
               "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t", "GIT_CONFIG_GLOBAL": os.devnull}
        for a in (["init", "-q"], ["add", "-A"], ["commit", "-qm", "x"]):
            r = sp.run(["git", *a], cwd=td, env=env, capture_output=True, text=True)
            if r.returncode != 0:
                return None                          # git setup failed -> skip rather than false-fail
        return m.main(["--root", td])
    finally:
        shutil.rmtree(td, ignore_errors=True)

_rc = _git_unicode_uncovered()
record("e2e(git): an un-manifested UNICODE-named fixture is CAUGHT (the -z/quotePath runner fix)",
       _rc in (1, None))                             # 1 = caught; None = git-absent skip

passed = sum(1 for _, ok in results if ok)
print(f"\n[g24-corpus-integrity] {passed}/{len(results)} assertions passed.")
sys.exit(0 if passed == len(results) else 1)
