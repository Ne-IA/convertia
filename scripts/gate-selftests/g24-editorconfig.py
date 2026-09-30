#!/usr/bin/env python3
"""g24-editorconfig.py - G24 planted-positive self-test for the G52 EOL/charset hygiene gate (P0.3.11).

Proves the pinned editorconfig-checker BINARY + the committed .editorconfig actually CATCH the four hygiene
violations - CRLF line endings, trailing whitespace, a missing final newline, non-UTF-8 text - and pass a
clean file. The violating fixtures are created at RUNTIME in a temp dir (a CRLF/no-final-newline file
cannot be committed cleanly under `.gitattributes eol=lf`); the repo's real .editorconfig is copied in
(its `root = true` stops the upward search). Then check-editorconfig over the real repo is asserted
clean - the gate does not false-positive on the committed tree.

The §6.4.5 fixture exemption is proven the same way: fixtures violating all four axes, planted at
every `tests/corpus/` depth (and in `tests/corpus-large/`), pass, the same bytes outside the corpus
trees fail (so the passes are the exemption, not a binary auto-skip), and the first-party
`tests/corpus/manifest.toml` still fails on a violation of any one axis. The paired `.gitattributes`
rows are read through `git check-attr` on the real repo: fixtures `-text` at every depth, the
manifest `text eol=lf`.

Skips with a warning (exit 0) if the pinned binary is absent (a dev box that did not run
install-gate-tools); the L4 gate-tooling job installs it and runs this for real. stdlib-only.
Exit 0 = all held / skipped; 1 = a self-test failed.
"""
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parents[2]
EDITORCONFIG = ROOT / ".editorconfig"
results: list[tuple[str, bool]] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}{(' - ' + detail) if detail else ''}")


def ec_bin() -> str | None:
    for cand in (ROOT / ".gate-tools" / "bin" / "editorconfig-checker.exe",
                 ROOT / ".gate-tools" / "bin" / "editorconfig-checker"):
        if cand.is_file():
            return str(cand)
    return shutil.which("editorconfig-checker")


def run_ec(workdir: Path) -> tuple[int, str]:
    r = subprocess.run([EC], cwd=str(workdir), capture_output=True, text=True, encoding="utf-8", errors="replace")
    return r.returncode, r.stdout + r.stderr


def section_pairs(text: str, header: str) -> dict[str, str]:
    """The `key = value` pairs under every `[header]` line of an .editorconfig text, lower-cased."""
    pairs: dict[str, str] = {}
    current = None
    for line in (raw.strip() for raw in text.splitlines()):
        if line.startswith("[") and line.endswith("]"):
            current = line[1:-1]
        elif current == header and "=" in line and not line.startswith(("#", ";")):
            key, _, value = line.partition("=")
            pairs[key.strip().lower()] = value.strip().lower()
    return pairs


EC = ec_bin()
if EC is None:
    print("[g24-editorconfig] SKIP - pinned editorconfig-checker binary not found (run "
          "scripts/install-gate-tools); the L4 gate-tooling canary installs it and runs this for real.")
    sys.exit(0)
if not EDITORCONFIG.is_file():
    print(f"[g24-editorconfig] FAIL - missing {EDITORCONFIG}", file=sys.stderr)
    sys.exit(1)

# Windows-1252 text with LF endings, a final newline and no trailing whitespace: charset is the only
# axis it violates. The checker's encoding detector is statistical, so the text is long enough to be
# reported (a lone b"caf\xe9" is not).
CP1252_BYTES = ("city,contact,note\nKöln,Björn Groß,Grüße\nGenève,René Müller,café\n"
                "Málaga,José Peña,mañana\n").encode("cp1252")

# --- planted positives: each violation type is caught in a temp dir -------------------------------
with tempfile.TemporaryDirectory() as td:
    d = Path(td)
    shutil.copy(EDITORCONFIG, d / ".editorconfig")
    (d / "crlf.toml").write_bytes(b"a = 1\r\nb = 2\r\n")                 # CRLF line endings
    (d / "trailing.toml").write_bytes(b"a = 1   \nb = 2\n")             # trailing whitespace
    (d / "nofinal.toml").write_bytes(b"a = 1\nb = 2")                   # no final newline
    (d / "cp1252.csv").write_bytes(CP1252_BYTES)                        # not UTF-8
    (d / "clean.toml").write_bytes(b"a = 1\nb = 2\n")                   # clean
    rc, out = run_ec(d)
    record("editorconfig-checker FAILS on the violating temp dir (rc != 0)", rc != 0, f"rc={rc}")
    record("CRLF file flagged", "crlf.toml" in out)
    record("trailing-whitespace file flagged", "trailing.toml" in out)
    record("missing-final-newline file flagged", "nofinal.toml" in out)
    record("non-UTF-8 (Windows-1252) file flagged", "cp1252.csv" in out)
    record("the CLEAN file is NOT flagged", "clean.toml" not in out)

# --- a wholly-clean temp dir passes ---------------------------------------------------------------
with tempfile.TemporaryDirectory() as td:
    d = Path(td)
    shutil.copy(EDITORCONFIG, d / ".editorconfig")
    (d / "ok.toml").write_bytes(b"a = 1\nb = 2\n")
    rc, _ = run_ec(d)
    record("a wholly-clean temp dir passes (rc == 0)", rc == 0, f"rc={rc}")

# --- the corpus fixture exemption holds at every depth, on all four axes ---------------------------
# CRLF + trailing spaces + no final newline + Windows-1252 text: every axis the corpus sections unset.
FIXTURE_BYTES = CP1252_BYTES.rstrip(b"\n").replace(b"\n", b"  \r\n")
CORPUS_FIXTURES = ("tests/corpus/flat.csv",                         # the P3 root layout
                   "tests/corpus/spreadsheets/deep/nested.tsv",     # two directories down
                   "tests/corpus/documents/prose.md",               # the [*.md] section matches too
                   "tests/corpus/documents/page.xml",               # a format no glob names
                   "tests/corpus-large/audio/large.csv")            # the LFS-backed tier
CONTROL = "tests/outside/outside.csv"                               # the same bytes, no exemption
with tempfile.TemporaryDirectory() as td:
    d = Path(td)
    shutil.copy(EDITORCONFIG, d / ".editorconfig")
    for rel in (*CORPUS_FIXTURES, CONTROL):
        (d / rel).parent.mkdir(parents=True, exist_ok=True)
        (d / rel).write_bytes(FIXTURE_BYTES)
    rc, out = run_ec(d)
    # The other three axes are plain byte tests; the charset flag is asserted by its message, so a
    # detector that stopped reporting these bytes cannot leave the fixtures' charset=unset untested.
    record("corpus: the same violating bytes OUTSIDE the corpus trees are flagged, the wrong charset "
           "included (the passes below are the exemption, not a binary skip)",
           Path(CONTROL).name in out and "Wrong character encoding" in out, f"rc={rc}")
    for rel in CORPUS_FIXTURES:
        record(f"corpus: a violating fixture at {rel} is NOT flagged", Path(rel).name not in out)

# --- the first-party manifest is back under every [*] axis: one manifest per axis, violating only it ---
# The CRLF payload's last line ends in LF: the checker's final-newline test expects LF even with
# end_of_line gone from the manifest section, so a CRLF last line would keep this leg green without
# the end_of_line rule.
MANIFEST_VIOLATIONS = (("a CRLF line ending", b"a = 1\r\nb = 2\n"),
                       ("trailing whitespace", b"a = 1  \nb = 2\n"),
                       ("no final newline", b"a = 1\nb = 2"),
                       ("Windows-1252 text", CP1252_BYTES))
for label, payload in MANIFEST_VIOLATIONS:
    with tempfile.TemporaryDirectory() as td:
        d = Path(td)
        shutil.copy(EDITORCONFIG, d / ".editorconfig")
        (d / "tests" / "corpus").mkdir(parents=True)
        (d / "tests" / "corpus" / "manifest.toml").write_bytes(payload)
        rc, out = run_ec(d)
        record(f"corpus: a tests/corpus/manifest.toml with {label} IS flagged (first-party, under the "
               "global rules)", rc != 0 and "manifest.toml" in out, f"rc={rc}")

# --- the paired .gitattributes rows, as git resolves them in the real repo -------------------------
EXPECTED_TEXT_ATTR = {**{rel: "unset" for rel in CORPUS_FIXTURES},
                      "tests/corpus/manifest.toml": "set", CONTROL: "auto"}
r = subprocess.run(["git", "-C", str(ROOT), "-c", "core.quotePath=false", "check-attr", "-z", "text", "eol",
                    "--", *EXPECTED_TEXT_ATTR], capture_output=True)
fields = r.stdout.decode("utf-8", errors="replace").split("\0")
attrs = {(fields[i], fields[i + 1]): fields[i + 2] for i in range(0, len(fields) - 2, 3)}
record("gitattributes: git check-attr runs on the real repo", r.returncode == 0, f"rc={r.returncode}")
for rel, want in EXPECTED_TEXT_ATTR.items():
    got = attrs.get((rel, "text"))
    record(f"gitattributes: {rel} resolves text={want}", got == want, f"got {got!r}")
record("gitattributes: tests/corpus/manifest.toml resolves eol=lf",
       attrs.get(("tests/corpus/manifest.toml", "eol")) == "lf")

# --- the real repo is clean today (check-editorconfig exits 0) ------------------------------------
rc = subprocess.run([sys.executable, str(ROOT / "scripts" / "check-editorconfig")]).returncode
record("check-editorconfig over the real committed tree is clean (rc=0)", rc == 0)

# --- .editorconfig carries the load-bearing rules -------------------------------------------------
# [Test-Change: G52 corpus exemption — old-obsolete+new-correct, build-gates G52 row] the old leg searched
# the whole file, which the [tests/corpus/manifest.toml] section satisfies with the same values, so it
# passed with them gone from [*]; the leg reads the [*] section alone.
star = section_pairs(EDITORCONFIG.read_text(encoding="utf-8"), "*")
record(".editorconfig [*] sets end_of_line=lf + insert_final_newline + charset=utf-8",
       star.get("end_of_line") == "lf" and star.get("insert_final_newline") == "true"
       and star.get("charset") == "utf-8", f"[*] = {star}")

failed = [n for n, ok in results if not ok]
print(f"\n[g24-editorconfig] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
