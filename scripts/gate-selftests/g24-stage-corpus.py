#!/usr/bin/env python3
"""g24-stage-corpus.py - G24 self-test for stage-corpus (P0.5.11, the G24a manifest generator).

Proves the tool (positive) regenerates correct sha256s, is IDEMPOTENT (a re-run is a clean diff) and
PRESERVES the §6.4.5 schema (the `[file.expect]` sub-table, `covers`, comments, row order); and
(negative) FAILS a fixture whose hash was not re-staged (stale, via --check), a row with a
missing/non-redistributable licence, and a row with no provenance - the P0.5.11 "a fixture added
without running stage-corpus → stale; a non-redistributable licence → fail" requirement. stdlib-only.
Exit 0 = held.
"""
import hashlib
import importlib.machinery
import importlib.util
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "stage-corpus"
_loader = importlib.machinery.SourceFileLoader("sc", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("sc", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def sha(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


FULL = '''# corpus manifest (the §6.4.5 schema)
[[file]]
path = "a.png"
source = "PNG"
licence = "CC0"          # must be redistributable
provenance = "self-produced"
exercises = ["x", "y"]
covers = [["PNG", "JPG"], ["PNG", "WEBP"]]
[file.expect]
"PNG→JPG" = { result = "success", lossy = "none" }
'''


def _staged(root: Path, manifest: str, files: dict[str, bytes]):
    cr = root / "tests" / "corpus"
    cr.mkdir(parents=True, exist_ok=True)
    for rel, body in files.items():
        (cr / rel).write_bytes(body)
    (cr / "manifest.toml").write_text(manifest, encoding="utf-8")


# --- content_sha256 + the block primitives ----------------------------------------------------
record("sha: real bytes -> content hash", m.content_sha256(b"png") == sha(b"png"))
record("sha: an LFS pointer -> its oid",
       m.content_sha256(b"version https://git-lfs.github.com/spec/v1\noid sha256:" + b"a" * 64 + b"\nsize 9\n") == "a" * 64)
record("sha: a malformed LFS pointer (no oid) -> None",
       m.content_sha256(b"version https://git-lfs.github.com/spec/v1\nsize 9\n") is None)

blk = ['[[file]]', 'path = "a.png"', 'licence = "CC0"', '[file.expect]', '"A" = { r = 1 }']
record("set_block_sha256: inserts sha256 after `path`, BEFORE the [file.expect] sub-table",
       m.set_block_sha256(blk, "deadbeef")[:3] == ['[[file]]', 'path = "a.png"', 'sha256 = "deadbeef"']
       and '[file.expect]' in m.set_block_sha256(blk, "deadbeef"))
blk2 = ['[[file]]', 'path = "a.png"', 'sha256 = "OLD"', 'licence = "CC0"']
record("set_block_sha256: REPLACES an existing sha256 (idempotent shape)",
       m.set_block_sha256(blk2, "NEW").count('sha256 = "NEW"') == 1 and 'sha256 = "OLD"' not in m.set_block_sha256(blk2, "NEW"))
# G1 re-review P3: a multi-line `covers` array's continuation line must NOT be mistaken for the end of
# the direct-key region - so a `sha256` placed AFTER a multi-line `covers` is REPLACED, not duplicated.
blk_after = ['[[file]]', 'path = "a"', 'covers = [', '  ["A", "B"],', ']', 'sha256 = "OLD"', '[file.expect]', '"A→B" = { r = 1 }']
record("set_block_sha256: a sha256 AFTER a multi-line `covers` array is replaced (no duplicate key)",
       (lambda r: r.count('sha256 = "NEW"') == 1 and 'sha256 = "OLD"' not in r
        and sum(1 for ln in r if ln.lstrip().startswith("sha256")) == 1)(m.set_block_sha256(blk_after, "NEW")))

pre, blocks = m.split_file_blocks(FULL)
record("split_file_blocks: the [file.expect] sub-table stays inside its [[file]] block",
       len(blocks) == 1 and any('[file.expect]' in ln for ln in blocks[0]))
# G1 P1: a `[[file]]` header with a TRAILING COMMENT (valid TOML) must be recognized (else the row
# collapses into the preamble + silently escapes staging/validation - a stale fixture slips). An EXOTIC
# header the splitter still misses (a quoted key) must FAIL-CLOSED via the block-count-vs-tomllib
# cross-check, never silently un-stage.
record("split_file_blocks: a `[[file]] # comment` trailing-comment header is recognized",
       len(m.split_file_blocks('[[file]]  # an image\npath = "a"\n')[1]) == 1)
with tempfile.TemporaryDirectory() as td:
    cr = Path(td) / "tests" / "corpus"
    cr.mkdir(parents=True)
    (cr / "a.png").write_bytes(b"z")
    nt, probs = m.regenerate('[[file]]  # an image\npath = "a.png"\nlicence = "CC0"\nprovenance = "p"\n', cr)
    record("regenerate: a trailing-comment header IS staged (sha written, no false-clean escape)",
           probs == [] and f'sha256 = "{sha(b"z")}"' in nt)
with tempfile.TemporaryDirectory() as td:
    cr = Path(td) / "tests" / "corpus"
    cr.mkdir(parents=True)
    (cr / "a.png").write_bytes(b"z")
    quoted = '[["file"]]\npath = "a.png"\nlicence = "CC0"\nprovenance = "p"\n'
    nt, probs = m.regenerate(quoted, cr)
    record("regenerate: a quoted-key `[[\"file\"]]` header the splitter misses FAILS-CLOSED (cross-check)",
           len(probs) == 1 and nt == quoted)

# --- regenerate: correct sha256 + idempotency + schema preservation ---------------------------
with tempfile.TemporaryDirectory() as td:
    root = Path(td)
    cr = root / "tests" / "corpus"
    cr.mkdir(parents=True)
    body = b"\x89PNG bytes"
    (cr / "a.png").write_bytes(body)
    new_text, problems = m.regenerate(FULL, cr)
    record("regenerate: a clean manifest validates (no problems)", problems == [])
    record("regenerate: the correct sha256 is written into the row",
           f'sha256 = "{sha(body)}"' in new_text)
    record("regenerate: the [file.expect] sub-table is PRESERVED",
           '[file.expect]' in new_text and "PNG→JPG" in new_text)
    record("regenerate: `covers` + `exercises` + the comment are PRESERVED",
           'covers = [["PNG", "JPG"], ["PNG", "WEBP"]]' in new_text and "must be redistributable" in new_text)
    record("regenerate: IDEMPOTENT - re-running over the staged text is a no-op (clean diff)",
           m.regenerate(new_text, cr)[0] == new_text)

# --- negative: stale hash, bad licence, missing provenance ------------------------------------
with tempfile.TemporaryDirectory() as td:
    root = Path(td)
    _staged(root, FULL, {"a.png": b"original"})
    # first stage writes the correct hash
    record("e2e: staging an un-hashed manifest writes the sha256 (exit 0)", m.main(["--root", str(root)]) == 0)
    record("e2e(--check): an up-to-date manifest passes", m.main(["--root", str(root), "--check"]) == 0)
    # now change the fixture WITHOUT re-staging -> stale
    (root / "tests" / "corpus" / "a.png").write_bytes(b"changed bytes")
    record("e2e(--check): a changed fixture (stale hash) is CAUGHT (exit 1)",
           m.main(["--root", str(root), "--check"]) == 1)
    record("e2e: re-staging fixes the stale hash (exit 0)", m.main(["--root", str(root)]) == 0)
    record("e2e(--check): up-to-date again after re-stage", m.main(["--root", str(root), "--check"]) == 0)

with tempfile.TemporaryDirectory() as td:
    root = Path(td)
    bad = FULL.replace('licence = "CC0"          # must be redistributable', 'licence = "GPL"')
    _staged(root, bad, {"a.png": b"x"})
    record("e2e: a NON-redistributable licence (GPL) FAILS (exit 1)", m.main(["--root", str(root)]) == 1)

with tempfile.TemporaryDirectory() as td:
    root = Path(td)
    noprov = FULL.replace('provenance = "self-produced"', 'provenance = ""')
    _staged(root, noprov, {"a.png": b"x"})
    record("e2e: an empty provenance FAILS (exit 1)", m.main(["--root", str(root)]) == 1)

with tempfile.TemporaryDirectory() as td:
    root = Path(td)
    _staged(root, FULL, {})            # manifest row, but NO fixture file on disk
    record("e2e: a manifest row with no on-disk fixture FAILS (exit 1)", m.main(["--root", str(root)]) == 1)

# --- target-absent + real repo ----------------------------------------------------------------
with tempfile.TemporaryDirectory() as td:
    record("e2e: no manifest -> target-absent no-op (exit 0)", m.main(["--root", td]) == 0)
record("e2e: the real repo passes (manifest present + current - stage-corpus is LIVE since P3.30)", m.main([]) == 0)

passed = sum(1 for _, ok in results if ok)
print(f"\n[g24-stage-corpus] {passed}/{len(results)} assertions passed.")
sys.exit(0 if passed == len(results) else 1)
