#!/usr/bin/env python3
"""g24-doc-splice.py - G24 self-test for check-doc-splice (G74, the P4.18 follow-up).

Proves the attachment-state scan CATCHES the REAL incident geometries - the P4.18 form AS IT
IS IN THE FILE (a plain `//` decision-pin block, then a BLANK line, then the spliced attr-led
`#[cfg(test)] mod` run: rustc-provenly attachment crosses blank lines), the attr-SEPARATED
form (comment -> attr -> item, insertion between attr and item - 975 in-tree sites carry that
shape), the P3.65/doc-led form (a spliced item leading with its own `///` docs - rustc-proven
doc merge+steal), the plain-led form, the attr-led item-MACRO form, and an indented
block-comment `*/` tail - while PASSING the calibrated-legitimate shapes: a pure `//`-comment
insertion between a `///` block and its item (the repo-mandated `[Build-Session-Entscheidung]`
/`[Test-Change]` tag convention, rustc-proven harmless), extending the doc block, adding an
attribute, a declaration REPLACEMENT under its block (the removal clears the attachment), an
insertion after CODE + a blank line, after a closing brace, after a letter-free `// ----`
divider (punctuation owns no attachment), a non-.rs file, a context-less hunk start, and a
body macro call after an explanatory comment. Also pins the four live-mode postures (staged
fail-open w/o HEAD; empty and zero --base skip with notice; live range). stdlib-only.
Exit 0 = held.
"""
import importlib.machinery
import importlib.util
import subprocess
import sys
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "check-doc-splice"
_loader = importlib.machinery.SourceFileLoader("cds", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cds", _loader))
sys.modules["cds"] = m
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


NL = "\n"


def diff(*body: str) -> str:
    return NL.join(body) + NL


# --- the hijack shapes (RED) -------------------------------------------------------------------
record("hijack: the REAL P4.18 geometry - `//` decision-pin block, then a BLANK line, then the "
       "attr-led module splice - is caught (attachment crosses the blank)",
       len(m.scan_unified_diff(diff(
           "+++ b/src-tauri/src/platform/mod.rs", "@@ -2920,5 +2920,10 @@",
           " // block is INSPECTION-pinned prose. [Build-Session-Entscheidung: P4.16]",
           " ",
           "+#[cfg(test)]",
           "+mod spliced_record_tests {",
           "+    fn t() {}",
           "+}",
           "+",
           " #[cfg(test)]", " mod macos_seatbelt_decision_tests {"))) == 1)
record("hijack: the attr-SEPARATED geometry - comment, attr(s), THEN the insertion between attr "
       "and item - is caught (attachment crosses attribute lines)",
       len(m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,4 +5,6 @@",
           " /// Docs for `documented`.",
           " #[allow(dead_code)]",
           "+pub struct Stealer;",
           "+",
           " pub fn documented() {}"))) == 1)
record("hijack: the P3.65/doc-led shape - a spliced item LEADING WITH ITS OWN /// docs "
       "(rustc-proven doc merge+steal) - is caught",
       len(m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,3 +5,6 @@",
           " /// Docs for `documented`.",
           "+/// New docs.",
           "+pub struct Stealer;",
           "+",
           " pub fn documented() {}"))) == 1)
record("hijack: a plain foreign item line after a /// context is caught",
       len(m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,3 +5,5 @@",
           " /// docs of the fn below", "+fn stealer() {}", "+", " fn documented() {}"))) == 1)
record("hijack: an attr-led run declaring its item via an item-MACRO (define_x!(..)) is caught",
       len(m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,3 +5,5 @@",
           " /// docs", "+#[cfg(test)]", "+define_test_module!(spliced);", " fn documented() {}"))) == 1)
record("hijack: an INDENTED block-comment tail (`*/`) is attachment context too",
       len(m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,3 +5,4 @@",
           "    docs tail */", "+fn stealer() {}", " fn documented() {}"))) == 1)
record("hijack: a spliced ASYNC fn under a doc block is caught (the modifier-chain forms)",
       all(len(m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,3 +5,4 @@",
           " /// docs", "+" + form + " stealer() {}", " fn documented() {}"))) == 1
           for form in ("async fn", "pub async fn", "pub(crate) async fn",
                        "unsafe extern \"C\" fn", "const fn")))
record("hijack: a spliced `pub use` re-export under a doc block is caught (rustdoc attaches "
       "docs to re-exports); a bare body `use` does not fire",
       len(m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,3 +5,4 @@",
           " /// docs", "+pub use crate::x::Stealer;", " fn documented() {}"))) == 1
       and m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -20,3 +20,4 @@",
           "    // explain the import", "+    use std::io::Read;", "    run();")) == [])
record("hijack: a spliced `const` ITEM under a doc block is caught (const stays an item, "
       "not only a modifier)",
       len(m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,3 +5,4 @@",
           " /// docs", "+const STOLEN: u8 = 1;", " fn documented() {}"))) == 1)
record("window: both live git-diff calls carry -U10 (the attachment-visibility window pin)",
       (lambda src: src.count('"-U10"') == 2 and '"-U3"' not in src)(
           SCRIPT.read_text(encoding="utf-8")))
record("hijack: the failure text carries the re-anchoring instruction + the memory name",
       (lambda ps: len(ps) == 1 and "BELOW the documented item's closing brace" in ps[0]
        and "inserting-a-module-hijacks-the-preceding-doc-comment" in ps[0])(
           m.scan_unified_diff(diff(
               "+++ b/a.rs", "@@ -5,3 +5,4 @@",
               " /// docs", "+fn stealer() {}", " fn documented() {}"))))

# --- the calibrated-legitimate shapes (GREEN) --------------------------------------------------
record("clean: a pure `//` comment insertion between a /// block and its item passes (the "
       "mandated [Build-Session-Entscheidung]/[Test-Change] tag convention; rustc-proven harmless)",
       m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,3 +5,5 @@",
           " /// docs",
           "+// [Build-Session-Entscheidung: P4.12] rationale directly at the code site.",
           "+// second rationale line.",
           " #[allow(clippy::too_many_arguments)]", " fn documented() {}")) == [])
record("clean: extending the doc block itself passes",
       m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,3 +5,4 @@",
           " /// docs line one", "+/// docs line two", " fn documented() {}")) == [])
record("clean: adding an attribute to the documented item passes",
       m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,3 +5,4 @@",
           " /// docs", "+#[allow(dead_code)]", " fn documented() {}")) == [])
record("clean: a declaration REPLACEMENT under its doc block (rename/signature/rustfmt) passes "
       "- the removal clears the attachment",
       m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,3 +5,3 @@",
           " /// docs", "-fn documented(a: u8) {}", "+fn documented(a: u8, b: u8) {}")) == [])
record("clean: a new item after CODE + a blank line passes (a blank carries an attachment but "
       "never creates one - code cleared it)",
       m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,4 +5,7 @@",
           "     run(state);",
           " ",
           "+/// new docs",
           "+fn new_item() {}",
           "+",
           " fn existing() {}")) == [])
record("clean: a new documented item inserted BELOW a closing brace passes",
       m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,3 +5,6 @@",
           " }", "+", "+/// new docs", "+fn new_item() {}", " #[cfg(test)]")) == [])
record("clean: a letter-free `// ----` divider owns no attachment - an item after it passes",
       m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,3 +5,5 @@",
           " // ----------------------------------------------------------------",
           "+impl From<A> for B { }",
           "+",
           " struct C;")) == [])
record("clean: a non-.rs file is out of scope",
       m.scan_unified_diff(diff(
           "+++ b/README.md", "@@ -1,2 +1,3 @@",
           " /// looks like docs", "+whatever fn", " text")) == [])
record("clean: a hunk starting directly with added lines (no preceding context) passes",
       m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -5,2 +5,3 @@",
           "+fn appended() {}", " fn existing() {}")) == [])
record("clean: a bare macro CALL after an explanatory `//` comment inside a body does not fire "
       "(item-macros count only in the attr-led item position)",
       m.scan_unified_diff(diff(
           "+++ b/a.rs", "@@ -20,3 +20,4 @@",
           "    // explain the next call",
           "+    log_run!(state);",
           "    run(state);")) == [])

# --- live-mode postures ------------------------------------------------------------------------
run = lambda *a: subprocess.run([sys.executable, str(SCRIPT), *a], capture_output=True, text=True)
r = run("--diff")
record("--diff live: exits 0 on the real repo",
       r.returncode == 0)
r = run("--range", "--base", "")
record("--range EMPTY base (pull_request/schedule): SKIPS with a notice, exit 0 - never argparse exit 2",
       r.returncode == 0 and "no resolvable range base" in r.stdout)
r = run("--range", "--base", "0" * 40)
record("--range zero base (first push): SKIPS with a notice, exit 0",
       r.returncode == 0 and "no resolvable range base" in r.stdout)
r = run("--range")
record("--range live: exits 0 over the real unpushed range (or skips with notice)",
       r.returncode == 0)

passed = sum(1 for _, ok in results if ok)
print(f"\n[g24-doc-splice] {passed}/{len(results)} assertions passed.")
sys.exit(0 if passed == len(results) else 1)
