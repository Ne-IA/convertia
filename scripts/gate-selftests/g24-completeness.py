#!/usr/bin/env python3
"""g24-completeness.py - G24 self-test for check-completeness (P0.4.11, G22 + G23).

Proves the G22 membership bijection logic CATCHES an uncovered format (no fixture / no round-trip
test) and PASSES full coverage; the G23 `convert_*`-handler scan finds command handlers (ignoring
non-command / non-`convert_*` fns) and the untested-handler walk CATCHES a handler with no partner
test (positive + negative, the P0.4.11 'a `convert_*` with no test MUST fail' requirement) and
PASSES one that is tested; and the live tier is target-absent today (no handler tracked). stdlib-
only, git-free (the walk is exercised via a monkeypatched `_git_tracked`). Exit 0 = held.
"""
import importlib.machinery
import importlib.util
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-completeness"
_loader = importlib.machinery.SourceFileLoader("cc", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cc", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def main_with(root: Path, tracked: dict[str, str]) -> int:
    """Write `tracked` {relpath: content} under root + run m.main(['--root', root]) with a
    `_git_tracked` monkeypatched to return those rel paths (git-free)."""
    for rel, content in tracked.items():
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")
    saved = m._git_tracked
    m._git_tracked = lambda r, *pats: list(tracked.keys())
    try:
        return m.main(["--root", str(root)])
    finally:
        m._git_tracked = saved


CMD = "#[tauri::command]\npub async fn convert_csv_tsv(input: String) -> Result<(), E> { Ok(()) }\n"

# --- G22 membership bijection (pure) -----------------------------------------------------------
record("g22: full coverage -> no gaps",
       m.g22_membership_gaps({"png", "csv"}, {"png", "csv"}, {"png", "csv"})
       == {"no_fixture": [], "no_round_trip_test": []})
record("g22: a format with NO fixture is caught",
       m.g22_membership_gaps({"png", "csv"}, {"png"}, {"png", "csv"})["no_fixture"] == ["csv"])
record("g22: a format with NO round-trip test is caught",
       m.g22_membership_gaps({"png", "csv"}, {"png", "csv"}, {"png"})["no_round_trip_test"] == ["csv"])
record("g22: an empty supported set -> no gaps (target-absent shape)",
       m.g22_membership_gaps(set(), set(), set()) == {"no_fixture": [], "no_round_trip_test": []})
record("g22: an extra fixture/test for an unsupported format is NOT a gap (subset, not equality)",
       m.g22_membership_gaps({"png"}, {"png", "extra"}, {"png", "extra"})
       == {"no_fixture": [], "no_round_trip_test": []})

# --- G23 handler scan (pure) -------------------------------------------------------------------
record("g23 scan: a `#[tauri::command] convert_*` handler is found",
       m.scan_convert_handlers(CMD) == {"convert_csv_tsv"})
record("g23 scan: a plain `fn convert_x` WITHOUT the command attribute is NOT a handler",
       m.scan_convert_handlers("fn convert_x() {}\n") == set())
record("g23 scan: a `#[tauri::command]` fn NOT named convert_* is NOT a G23 handler",
       m.scan_convert_handlers("#[tauri::command]\nfn start_conversion() {}\n") == set())
record("g23 scan: an intervening attribute/doc-comment between attr and fn is tolerated",
       m.scan_convert_handlers("#[tauri::command]\n/// doc\n#[allow(unused)]\nfn convert_a() {}\n")
       == {"convert_a"})
record("g23 scan: multiple handlers in one file are all found",
       m.scan_convert_handlers(CMD + "#[tauri::command]\nfn convert_b() {}\n")
       == {"convert_csv_tsv", "convert_b"})
# G1-review P1: forms the line-comment-only regex MISSED (a fail-OPEN in G23's sole enforcement) -
# now covered via the comment+string-blanking pre-pass + the widened fn-qualifier group.
record("g23 scan: a BLOCK comment /* */ between attr and fn does not hide the handler",
       m.scan_convert_handlers("#[tauri::command]\n/* note */\nfn convert_a() {}\n") == {"convert_a"})
record("g23 scan: a /** */ outer-doc between attr and fn does not hide the handler",
       m.scan_convert_handlers("#[tauri::command]\n/** doc */\npub fn convert_b() {}\n") == {"convert_b"})
record("g23 scan: an `unsafe fn` handler is found",
       m.scan_convert_handlers("#[tauri::command]\nunsafe fn convert_c() {}\n") == {"convert_c"})
record("g23 scan: a `const fn` handler is found",
       m.scan_convert_handlers("#[tauri::command]\nconst fn convert_d() {}\n") == {"convert_d"})
record("g23 scan: combined `pub async unsafe fn` qualifiers are tolerated",
       m.scan_convert_handlers("#[tauri::command]\npub async unsafe fn convert_e() {}\n") == {"convert_e"})
record("g23 scan: a `]` inside an attribute-arg string does not truncate the match",
       m.scan_convert_handlers('#[tauri::command]\n#[doc = "input[0]"]\nfn convert_f() {}\n') == {"convert_f"})
record("g23 scan: a handler name appearing only INSIDE a string/comment is NOT a false handler",
       m.scan_convert_handlers('let s = "#[tauri::command] fn convert_ghost() {}";\n// fn convert_z\n') == set())
# G1 re-review P1 (a REGRESSION the comment/string pre-pass introduced): a char literal containing a
# double-quote (`'"'`, idiomatic in the CSV/TSV delimiter context) must NOT send the stripper into
# string-mode + run away, blanking a real handler below it.
record("g23 scan: a `'\"'` char literal does NOT swallow a handler below it (the delimiter case)",
       m.scan_convert_handlers("const Q: char = '\"';\n#[tauri::command]\nfn convert_x() {}\n") == {"convert_x"})
record("g23 scan: a byte-char `b'\"'` likewise does not swallow a handler",
       m.scan_convert_handlers("const Q: u8 = b'\"';\n#[tauri::command]\nfn convert_y() {}\n") == {"convert_y"})
record("g23 scan: an escaped-quote char literal `'\\''` before a handler is consumed atomically",
       m.scan_convert_handlers("let c = '\\'';\n#[tauri::command]\nfn convert_z() {}\n") == {"convert_z"})
record("g23 scan: a lifetime `'a` (no closing quote) is NOT mistaken for a char literal",
       m.scan_convert_handlers("#[tauri::command]\nfn convert_w<'a>(x: &'a str) {}\n") == {"convert_w"})

# --- G23 untested walk (pure) ------------------------------------------------------------------
record("g23 untested: a handler with NO reference in test text is caught",
       m.g23_untested({"convert_a", "convert_b"}, "fn t() { convert_a(); }") == {"convert_b"})
record("g23 untested: a handler referenced in test text is NOT a gap",
       m.g23_untested({"convert_a"}, "fn t() { convert_a(); }") == set())
record("g23 untested: word-boundary - `convert_x` is NOT covered by a `convert_xy` reference",
       m.g23_untested({"convert_x"}, "convert_xy();") == {"convert_x"})
record("g23 untested: no handlers -> no gaps", m.g23_untested(set(), "") == set())

# --- end-to-end (git-free, monkeypatched walk) -------------------------------------------------
with tempfile.TemporaryDirectory() as td:
    record("e2e: no convert_* handler tracked -> target-absent (exit 0)",
           main_with(Path(td), {"src/lib.rs": "fn helper() {}\n"}) == 0)
with tempfile.TemporaryDirectory() as td:
    record("e2e: a handler WITH a partner test in tests/ -> exit 0",
           main_with(Path(td), {"src/cmd.rs": CMD,
                                "tests/convert.rs": "#[test] fn t() { convert_csv_tsv(); }\n"}) == 0)
with tempfile.TemporaryDirectory() as td:
    record("e2e: a handler with NO partner test -> exit 1",
           main_with(Path(td), {"src/cmd.rs": CMD}) == 1)
with tempfile.TemporaryDirectory() as td:
    # _git_tracked returning None (git unavailable) -> fail-closed exit 2
    saved = m._git_tracked
    m._git_tracked = lambda r, *pats: None
    try:
        record("e2e: git unavailable -> fail-closed exit 2", m.main(["--root", td]) == 2)
    finally:
        m._git_tracked = saved

record("e2e: the real repo passes (no convert_* handler yet -> target-absent)", m.main([]) == 0)

passed = sum(1 for _, ok in results if ok)
print(f"\n[g24-completeness] {passed}/{len(results)} assertions passed.")
sys.exit(0 if passed == len(results) else 1)
