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
body macro call after an explanatory comment. Leg (c) (deleted-definition dangling references):
net-deletion from a diff (moved / commented / non-.rs forms excluded, a whole-file deletion keyed by
its old path; `complete_kind_list!` heads resolve) and the backticked-comment
resolver (caught, qualified forms, resolves when still defined, comment-defs do not resolve, code /
string / plain-word mentions are not references, block-comment state, empty-set short-circuit), the
ACMRDT pin on both live diffs, and three END-TO-END legs driving `--diff` over a throwaway repo's real
staged `git rm` (cited -> red, uncited -> green) plus the pin that the throwaway repos are removed.
Also pins the four live-mode postures (staged fail-open w/o HEAD; empty and zero --base skip with
notice; live range). stdlib-only.
Exit 0 = held.
"""
import importlib.machinery
import importlib.util
import os
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

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
record("hijack: an ADDED body line beginning `+++ ` earlier in the hunk does not re-key the file - the hijack after it is still caught",
       len(m.scan_unified_diff(diff(
           "diff --git a/src-tauri/src/one.rs b/src-tauri/src/one.rs", "--- a/src-tauri/src/one.rs", "+++ b/src-tauri/src/one.rs",
           "@@ -1,3 +1,7 @@", "+++ a diff sample line inside a string continuation", " fn keep() {}",
           " // the P4 decision pin for the item below", " ", "+fn spliced() {}", "+", " #[cfg(test)]", " mod t {"))) == 1
       and len(m.scan_unified_diff(diff(
           "diff --git a/src-tauri/src/one.rs b/src-tauri/src/one.rs", "--- a/src-tauri/src/one.rs", "+++ b/src-tauri/src/one.rs",
           "@@ -1,3 +1,6 @@", " fn keep() {}",
           " // the P4 decision pin for the item below", " ", "+fn spliced() {}", "+", " #[cfg(test)]", " mod t {"))) == 1)
record("hijack: the `\\ No newline at end of file` marker is not a new-side line - a hijack after it is reported at the right line",
       (lambda ps: len(ps) == 1 and "src-tauri/src/x.rs:2:" in ps[0])(m.scan_unified_diff(diff(
           "diff --git a/src-tauri/src/x.rs b/src-tauri/src/x.rs", "--- a/src-tauri/src/x.rs", "+++ b/src-tauri/src/x.rs",
           "@@ -1,2 +1,4 @@", " /// docs of the fn below", "\\ No newline at end of file", "+fn stealer() {}", "+",
           " fn documented() {}"))))
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

# --- leg (c): deleted-definition dangling references (pure fns) --------------------------------------
_c_diff = diff("diff --git a/src-tauri/src/engines/mod.rs b/src-tauri/src/engines/mod.rs",
               "--- a/src-tauri/src/engines/mod.rs", "+++ b/src-tauri/src/engines/mod.rs", "@@ -10,4 +10,2 @@",
               "-    fn engine_id_exhaustive(id: EngineId) {",
               "-        match id { EngineId::Ffmpeg => {} }",
               "-    }",
               "+    complete_kind_list!(ENGINE_ID_WIRE_NAMES, t: EngineId = [Ffmpeg => \"ffmpeg\"]);")
record("deleted-def: a `-` fn definition no `+` line re-declares is a net deletion, keyed by file",
       m.net_deleted_definitions(_c_diff) == {"engine_id_exhaustive": "src-tauri/src/engines/mod.rs"})
record("deleted-def: a definition MOVED within the diff (removed here, added there) is not a deletion",
       m.net_deleted_definitions(_c_diff + diff("diff --git a/src-tauri/src/pool/mod.rs b/src-tauri/src/pool/mod.rs",
                                                "--- a/src-tauri/src/pool/mod.rs", "+++ b/src-tauri/src/pool/mod.rs", "@@ -1,0 +1,1 @@",
                                                "+fn engine_id_exhaustive(id: EngineId) {}")) == {})
record("deleted-def: a `//`-led line or a string on the `-` side is not a definition (a removed `/* */` body line would be - the recorded over-strict residual)",
       m.net_deleted_definitions(diff("+++ b/a.rs", "@@ -1,2 +1,1 @@",
                                      "-// fn ghost() was here", "-let s = \"fn ghost2()\";")) == {})
record("deleted-def: a WHOLE-FILE deletion (`+++ /dev/null`) still yields its definitions, keyed by the old path",
       m.net_deleted_definitions(diff("--- a/src-tauri/src/gone.rs", "+++ /dev/null", "@@ -1,2 +0,0 @@",
                                      "-fn zz_gone_helper() {}", "-fn zz_second() {}"))
       == {"zz_gone_helper": "src-tauri/src/gone.rs", "zz_second": "src-tauri/src/gone.rs"})
record("deleted-def: an ADDED body line starting with `+++ ` is not a file header - the chunk's later deletion is still seen",
       m.net_deleted_definitions(diff("+++ b/src-tauri/src/a.rs", "@@ -1,2 +1,2 @@", "+++ decorative banner",
                                      "-fn victim_helper() {}", "+// gone")) == {"victim_helper": "src-tauri/src/a.rs"})
record("deleted-def: a non-.rs file's `-` lines are out of scope",
       m.net_deleted_definitions(diff("+++ b/scripts/x.py", "@@ -1,1 +1,0 @@", "-fn not_rust() {}")) == {})
_c_gone = {"engine_id_exhaustive": "src-tauri/src/engines/mod.rs"}
record("deleted-def: a backticked comment reference to a deleted name no file defines -> caught with file:line",
       [p for p in m.dangling_deleted_refs(_c_gone, {
           "src-tauri/src/pool/mod.rs": "// cf. `crate::engines`' `engine_id_exhaustive`: the lock\nfn other() {}\n"})
        if p.startswith("src-tauri/src/pool/mod.rs:1:") and "engine_id_exhaustive" in p] != [])
record("deleted-def: a `path::name()` / `name!` qualified backtick reference is caught too",
       m.dangling_deleted_refs(_c_gone, {"a.rs": "/// see `engines::engine_id_exhaustive()`\n"}) != []
       and m.dangling_deleted_refs({"define_x": "b.rs"}, {"a.rs": "// via `define_x!`\n"}) != [])
record("deleted-def: a name still DEFINED in some tracked file resolves (no finding)",
       m.dangling_deleted_refs(_c_gone, {
           "src-tauri/src/pool/mod.rs": "// cf. `engine_id_exhaustive`\nfn engine_id_exhaustive() {}\n"}) == [])
record("deleted-def: a definition-shaped line inside a BLOCK-COMMENT body does not resolve a reference "
       "(the stripped projection; a `//` line never line-anchors as a definition anyway)",
       m.dangling_deleted_refs(_c_gone, {"a.rs": "// `engine_id_exhaustive`\n/*\nfn engine_id_exhaustive() {}\n*/\n"}) != [])
record("deleted-def: the name in CODE or in a STRING (not a comment) is not a reference",
       m.dangling_deleted_refs(_c_gone, {"a.rs": "let s = \"`engine_id_exhaustive`\";\nengine_id_exhaustive();\n"}) == [])
record("deleted-def: a plain-word mention without backticks is prose, not a reference",
       m.dangling_deleted_refs({"flush": "x.rs"}, {"a.rs": "// the writer must flush before close\n"}) == [])
record("deleted-def: a `/* ... */` body line carries comment state across lines",
       m.dangling_deleted_refs(_c_gone, {"a.rs": "/* the lock\n   `engine_id_exhaustive` here\n*/\n"}) != [])
record("deleted-def: an empty deleted set short-circuits (no tree read, no finding)",
       m.dangling_deleted_refs({}, {"a.rs": "// `anything`\n"}) == [])

record("deleted-def: an item the crate's `complete_kind_list!` declares (its const, test and `elsewhere` names) resolves",
       m.dangling_deleted_refs({"X_KINDS": "a.rs", "x_is_complete": "a.rs", "X_ELSEWHERE": "a.rs"}, {
           "b.rs": "// `X_KINDS` / `x_is_complete` / `X_ELSEWHERE`\ncomplete_kind_list!(\n"
                   "    X_KINDS, x_is_complete: Kind = [A, B], elsewhere X_ELSEWHERE = [C],\n);\n"}) == [])
record("deleted-def: both live git-diff calls include DELETIONS and typechanges (--diff-filter=ACMRDT) so a removed file's definitions reach leg (c)",
       SCRIPT.read_text(encoding="utf-8").count('"--diff-filter=ACMRDT"') == 2)


def _rmtree_git(d: str) -> None:
    """Remove a throwaway repo: git's pack/object files are read-only, which shutil.rmtree cannot delete on
    Windows - make everything writable first, then remove (nothing may be left behind in the temp dir)."""
    for root, dirs, files in os.walk(d):
        for name in dirs + files:
            try:
                os.chmod(os.path.join(root, name), stat.S_IWRITE | stat.S_IREAD)
            except OSError:
                pass
    shutil.rmtree(d, ignore_errors=True)


# The throwaway-repo git calls (and the script run inside the repo) get an environment WITHOUT the GIT_*
# location variables a hook plane could carry (GIT_DIR / GIT_INDEX_FILE / GIT_WORK_TREE / ...): with one of
# them absolute, `git -C <tmp> add -A` + `commit` would operate on the OUTER repository. GIT_EXEC_PATH stays.
_GIT_ENV = {k: v for k, v in os.environ.items() if not k.startswith("GIT_") or k == "GIT_EXEC_PATH"}
_E2E_DIRS: list[str] = []


def _e2e_deleted_file(cite: bool, config=(), attributes=None, extra=None) -> tuple[int, str]:
    """The WIRED path, not the pure fn: a throwaway repo, a real staged `git rm` of a defining file, then the
    script's own `--diff` mode (Loop-memory `test-both-halves-against-one-artifact`)."""
    d = tempfile.mkdtemp(prefix="g24-doc-splice-")
    _E2E_DIRS.append(d)
    try:
        def g(*a):
            subprocess.run(["git", "-C", d, "-c", "commit.gpgsign=false", *a], check=True, capture_output=True,
                           env=_GIT_ENV)
        g("init", "-q")
        (Path(d) / "nohooks").mkdir()
        g("config", "core.hooksPath", str(Path(d) / "nohooks"))
        g("config", "user.email", "g24@example.invalid")
        g("config", "user.name", "g24")
        for k, v in config:
            g("config", k, v)
        if attributes is not None:
            (Path(d) / ".git" / "info").mkdir(exist_ok=True)
            (Path(d) / ".git" / "info" / "attributes").write_text(attributes, encoding="utf-8")
        src = Path(d) / "src-tauri" / "src"
        src.mkdir(parents=True)
        (src / "gone.rs").write_text("fn zz_gone_helper() {}\n", encoding="utf-8")
        (src / "keep.rs").write_text(("// cites `zz_gone_helper` here\n" if cite else "// cites nothing\n") + "fn keep() {}\n",
                                     encoding="utf-8")
        for rel, text in (extra or {}).items():
            (Path(d) / rel).parent.mkdir(parents=True, exist_ok=True)
            (Path(d) / rel).write_bytes(text.encode("utf-8"))
        g("add", "-A")
        g("commit", "-q", "-m", "base")
        g("rm", "-q", "src-tauri/src/gone.rs")
        r = subprocess.run([sys.executable, str(SCRIPT), "--diff"], capture_output=True, text=True, cwd=d,
                           encoding="utf-8", errors="replace", env=_GIT_ENV)
        return r.returncode, (r.stdout or "") + (r.stderr or "")
    finally:
        _rmtree_git(d)


_rc, _out = _e2e_deleted_file(cite=True)
record("deleted-def END-TO-END: a real staged `git rm` of a defining file, cited elsewhere, reds `--diff` (the wired path sees D)",
       _rc == 1 and "zz_gone_helper" in _out)
_rc, _out = _e2e_deleted_file(cite=False)
record("deleted-def END-TO-END: the same deletion, uncited, passes `--diff`",
       _rc == 0 and "no comment citing" in _out)
record("deleted-def E2E: a `diff.external` driver a user set (one that cannot run) neither replaces the patch nor turns the staged read"
       " into a failure - the diff-SYNTAX pins hold -> rc 1",
       _e2e_deleted_file(True, config=[("diff.external", "g24-no-such-diff-driver")])[0] == 1)

def _e2e_hijack(env_extra=None) -> tuple[int, str]:
    """The WIRED leg (a): a throwaway repo whose committed file is `/// docs` + `fn documented`, an insertion staged
    between them, then the script's own `--diff` under the given extra environment."""
    d = tempfile.mkdtemp(prefix="g24-doc-splice-")
    _E2E_DIRS.append(d)
    try:
        def g(*a):
            subprocess.run(["git", "-C", d, "-c", "commit.gpgsign=false", *a], check=True, capture_output=True,
                           env=_GIT_ENV)
        g("init", "-q")
        (Path(d) / "nohooks").mkdir()
        g("config", "core.hooksPath", str(Path(d) / "nohooks"))
        g("config", "user.email", "g24@example.invalid")
        g("config", "user.name", "g24")
        src = Path(d) / "src-tauri" / "src"
        src.mkdir(parents=True)
        (src / "x.rs").write_text("/// docs of the fn below\nfn documented() {}\n", encoding="utf-8")
        g("add", "-A")
        g("commit", "-q", "-m", "base")
        (src / "x.rs").write_text("/// docs of the fn below\nfn stealer() {}\n\nfn documented() {}\n", encoding="utf-8")
        g("add", "-A")
        r = subprocess.run([sys.executable, str(SCRIPT), "--diff"], capture_output=True, text=True, cwd=d,
                           encoding="utf-8", errors="replace", env={**_GIT_ENV, **(env_extra or {})})
        return r.returncode, (r.stdout or "") + (r.stderr or "")
    finally:
        _rmtree_git(d)


record("deleted-def E2E: a user attribute marking `.rs` binary cannot hide a deleted definition behind `Binary files differ` - the diff is"
       " read `--text` (the round-13 P0) -> rc 1",
       _e2e_deleted_file(True, attributes="*.rs binary\n")[0] == 1)
record("hijack E2E: GIT_DIFF_OPTS=-u0 in the environment (git honours it over the explicit -U10) is scrubbed, so the doc comment above an"
       " inserted item is still in the hunk and the hijack is caught -> rc 1",
       _e2e_hijack(env_extra={"GIT_DIFF_OPTS": "-u0"})[0] == 1 and _e2e_hijack()[0] == 1)
record("headers: a git-QUOTED `+++`/`---` path is C-unquoted in leg (a)'s file key and leg (c)'s deletion key (the round-12 G70 fix swept to"
       " its siblings, the round-13 P3)",
       m._header_path(' "b/src-tauri/src/a\\"b.rs"') == 'b/src-tauri/src/a"b.rs' and m._header_path(" b/plain.rs ") == "b/plain.rs"
       and any('src-tauri/src/a"b.rs:2' in p for p in m.scan_unified_diff(
           'diff --git "a/src-tauri/src/a\\"b.rs" "b/src-tauri/src/a\\"b.rs"\n--- "a/src-tauri/src/a\\"b.rs"\n+++ "b/src-tauri/src/a\\"b.rs"\n'
           "@@ -1,2 +1,4 @@\n /// docs of the fn below\n+fn stealer() {}\n+\n fn documented() {}\n"))
       and m.net_deleted_definitions('diff --git "a/src-tauri/src/a\\"b.rs" b/dev/null\ndeleted file mode 100644\n--- "a/src-tauri/src/a\\"b.rs"\n'
                                     "+++ /dev/null\n@@ -1,1 +0,0 @@\n-fn zz_gone() {}\n") == {"zz_gone": 'src-tauri/src/a"b.rs'})
record("line model: a lone CR inside a comment cannot forge a definition line - lines are split on git's `\\n` only, from bytes"
       " (under `str.splitlines()` + universal newlines `// x \\rfn helper_thing() {}` declared `helper_thing`, the round-14 finding)",
       m._lines("a\rb\n") == ["a\rb"] and "helper_thing" not in m.collect_definitions("// x \rfn helper_thing() {}\n")
       and "helper_thing" in m.collect_definitions("// x\nfn helper_thing() {}\n"))
record("deleted-def E2E: an unrelated tracked file carrying `// x \\rfn helper_thing() {}` does not resolve a cited, deleted"
       " `helper_thing` -> rc 1 (the resolver reads the worktree from bytes, git's line model)",
       _e2e_deleted_file(True, extra={"src-tauri/src/other.rs": "// x \rfn zz_gone_helper() {}\nfn other() {}\n"})[0] == 1)
record("source pin: the git reads and the worktree sources are read as BYTES decoded as UTF-8 (no universal-newline translation)",
       'subprocess.run(["git", *args], env=_git_env(), capture_output=True)' in SCRIPT.read_text(encoding="utf-8")
       and 'open(f"{base}/{rel}", "rb") as fh:' in SCRIPT.read_text(encoding="utf-8")
       and SCRIPT.read_text(encoding="utf-8").replace("str.splitlines()", "").count("splitlines()") == 0)
record("deleted-def: both live diff reads carry the diff-SYNTAX pins (non-ASCII literal, `a/`/`b/` prefixes forced, blank context never"
       " elided, hunks never merged, paths never relative, no external driver, no textconv, every file read as text) as one constant pair,"
       " and the env knob that outranks the command line (GIT_DIFF_OPTS) is scrubbed",
       m.GIT_DIFF_SYNTAX == ("-c", "core.quotepath=false", "-c", "diff.noprefix=false", "-c", "diff.mnemonicPrefix=false",
                             "-c", "diff.srcPrefix=a/", "-c", "diff.dstPrefix=b/", "-c", "diff.suppressBlankEmpty=false",
                             "-c", "diff.relative=false", "-c", "diff.interHunkContext=0")
       and m.GIT_DIFF_FLAGS == ("--no-ext-diff", "--no-textconv", "--text", "--no-color")
       and m.SCRUBBED_GIT_ENV == ("GIT_DIFF_OPTS", "GIT_EXTERNAL_DIFF")
       and 'subprocess.run(["git", *args], env=_git_env(),' in SCRIPT.read_text(encoding="utf-8")
       and SCRIPT.read_text(encoding="utf-8").count('_git(*GIT_DIFF_SYNTAX, "diff"') == 2
       and SCRIPT.read_text(encoding="utf-8").count('*GIT_DIFF_FLAGS, "--diff-filter=ACMRDT"') == 2)
record("deleted-def END-TO-END: this run's throwaway repos are removed (each of its own temp dirs is gone)",
       len(_E2E_DIRS) == 7 and not any(os.path.exists(p) for p in _E2E_DIRS))

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
