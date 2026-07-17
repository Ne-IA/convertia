#!/usr/bin/env python3
"""g24-completeness.py - G24 self-test for check-completeness (P0.4.11, G22 + G23).

Proves the G22 membership bijection logic CATCHES an uncovered format (no fixture / no round-trip
test) and PASSES full coverage; the G23 conversion-command scan finds the SS0.4.1 conversion
command(s) (`_CONVERSION_COMMANDS`, exactly `start_conversion` - re-keyed 2026-07-17 by the P3.63
ruling; the retired `convert_*` shape and every non-conversion command are ignored) and the
untested-command walk CATCHES a command with no partner test (positive + negative) and PASSES one
that is tested; and the live tier is GREEN on the real repo (G23 LIVE since P3.63). stdlib-only,
git-free (the walk is exercised via a monkeypatched `_git_tracked`). Exit 0 = held.
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


# The REAL SS0.4.1 C6 form (ipc/conversion.rs): attr-args + a stacked attribute + `pub async`.
CMD = ('#[tauri::command(rename_all = "camelCase")]\n#[specta::specta]\n'
       "pub async fn start_conversion(input: String) -> Result<(), E> { Ok(()) }\n")

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

# --- G23 conversion-command scan (pure) --------------------------------------------------------
# NB the pre-re-key "multiple handlers in one file are all found" leg has NO successor while
# `_CONVERSION_COMMANDS` is single-element (two distinct member names cannot be constructed);
# the standing obligation in check-completeness requires RE-ADDING a multi-member/multi-match
# leg in the same commit that first grows the set (the G1 re-key review's convergent finding).
record("g23 scan: the REAL C6 form (attr-args + stacked attr + pub async) is found",
       m.scan_conversion_commands(CMD) == {"start_conversion"})
record("g23 scan: the bare-attr form is found (the pre-re-key self-test pinned this as IGNORED)",
       m.scan_conversion_commands("#[tauri::command]\nfn start_conversion() {}\n")
       == {"start_conversion"})
record("g23 scan: a plain `fn start_conversion` WITHOUT the command attribute is NOT a handler",
       m.scan_conversion_commands("fn start_conversion() {}\n") == set())
record("g23 scan: a `#[tauri::command]` fn NOT in the conversion set (get_targets) is ignored",
       m.scan_conversion_commands("#[tauri::command]\nfn get_targets() {}\n") == set())
record("g23 scan: the RETIRED `convert_*` shape is no longer special-cased (spec SS0.4 forbids it)",
       m.scan_conversion_commands("#[tauri::command]\npub async fn convert_csv_tsv() {}\n") == set())
record("g23 scan: `start_conversion_extended` is NOT captured (the keyed set is word-bounded)",
       m.scan_conversion_commands("#[tauri::command]\nfn start_conversion_extended() {}\n") == set())
record("g23 scan: an intervening attribute/doc-comment between attr and fn is tolerated",
       m.scan_conversion_commands("#[tauri::command]\n/// doc\n#[allow(unused)]\nfn start_conversion() {}\n")
       == {"start_conversion"})
# G1-review P1 (P0.4.11): forms the line-comment-only regex MISSED (a fail-OPEN in G23's sole
# enforcement) - covered via the comment+string-blanking pre-pass + the widened fn-qualifier group.
record("g23 scan: a BLOCK comment /* */ between attr and fn does not hide the handler",
       m.scan_conversion_commands("#[tauri::command]\n/* note */\nfn start_conversion() {}\n")
       == {"start_conversion"})
record("g23 scan: a /** */ outer-doc between attr and fn does not hide the handler",
       m.scan_conversion_commands("#[tauri::command]\n/** doc */\npub fn start_conversion() {}\n")
       == {"start_conversion"})
record("g23 scan: an `unsafe fn` handler is found",
       m.scan_conversion_commands("#[tauri::command]\nunsafe fn start_conversion() {}\n")
       == {"start_conversion"})
record("g23 scan: a `const fn` handler is found",
       m.scan_conversion_commands("#[tauri::command]\nconst fn start_conversion() {}\n")
       == {"start_conversion"})
record("g23 scan: combined `pub async unsafe fn` qualifiers are tolerated",
       m.scan_conversion_commands("#[tauri::command]\npub async unsafe fn start_conversion() {}\n")
       == {"start_conversion"})
record("g23 scan: a `]` inside an attribute-arg string does not truncate the match",
       m.scan_conversion_commands('#[tauri::command]\n#[doc = "input[0]"]\nfn start_conversion() {}\n')
       == {"start_conversion"})
record("g23 scan: a handler name appearing only INSIDE a string/comment is NOT a false handler",
       m.scan_conversion_commands('let s = "#[tauri::command] fn start_conversion() {}";\n// fn start_conversion\n')
       == set())
# G1 re-review P1 (P0.4.11, a REGRESSION the comment/string pre-pass introduced): a char literal
# containing a double-quote (`'"'`, idiomatic in the CSV/TSV delimiter context) must NOT send the
# stripper into string-mode + run away, blanking a real handler below it.
record("g23 scan: a `'\"'` char literal does NOT swallow a handler below it (the delimiter case)",
       m.scan_conversion_commands("const Q: char = '\"';\n#[tauri::command]\nfn start_conversion() {}\n")
       == {"start_conversion"})
record("g23 scan: a byte-char `b'\"'` likewise does not swallow a handler",
       m.scan_conversion_commands("const Q: u8 = b'\"';\n#[tauri::command]\nfn start_conversion() {}\n")
       == {"start_conversion"})
record("g23 scan: an escaped-quote char literal `'\\''` before a handler is consumed atomically",
       m.scan_conversion_commands("let c = '\\'';\n#[tauri::command]\nfn start_conversion() {}\n")
       == {"start_conversion"})
record("g23 scan: a lifetime `'a` (no closing quote) is NOT mistaken for a char literal",
       m.scan_conversion_commands("#[tauri::command]\nfn start_conversion<'a>(x: &'a str) {}\n")
       == {"start_conversion"})

# --- G23 untested walk (pure) ------------------------------------------------------------------
record("g23 untested: a command with NO reference in test text is caught (the future-set case)",
       m.g23_untested({"start_conversion", "resume_conversion"},
                      "fn t() { start_conversion(); }") == {"resume_conversion"})
record("g23 untested: a command referenced in test text is NOT a gap",
       m.g23_untested({"start_conversion"}, "fn t() { start_conversion(); }") == set())
record("g23 untested: word-boundary - `start_conversion` is NOT covered by `start_conversion_extended`",
       m.g23_untested({"start_conversion"}, "start_conversion_extended();") == {"start_conversion"})
record("g23 untested: no commands -> no gaps", m.g23_untested(set(), "") == set())

# --- end-to-end (git-free, monkeypatched walk) -------------------------------------------------
with tempfile.TemporaryDirectory() as td:
    record("e2e: no conversion-command handler tracked -> target-absent (exit 0)",
           main_with(Path(td), {"src/lib.rs": "fn helper() {}\n"}) == 0)
with tempfile.TemporaryDirectory() as td:
    record("e2e: a conversion command WITH a partner test in tests/ -> exit 0",
           main_with(Path(td), {"src/cmd.rs": CMD,
                                "tests/convert.rs": "#[test] fn t() { start_conversion(); }\n"}) == 0)
with tempfile.TemporaryDirectory() as td:
    record("e2e: a conversion command with NO partner test -> exit 1 (the planted positive)",
           main_with(Path(td), {"src/cmd.rs": CMD}) == 1)
with tempfile.TemporaryDirectory() as td:
    # _git_tracked returning None (git unavailable) -> fail-closed exit 2
    saved = m._git_tracked
    m._git_tracked = lambda r, *pats: None
    try:
        record("e2e: git unavailable -> fail-closed exit 2", m.main(["--root", td]) == 2)
    finally:
        m._git_tracked = saved

record("e2e: the real repo passes (start_conversion + its partner suite tracked - G23 LIVE since P3.63)",
       m.main([]) == 0)

passed = sum(1 for _, ok in results if ok)
print(f"\n[g24-completeness] {passed}/{len(results)} assertions passed.")
sys.exit(0 if passed == len(results) else 1)
