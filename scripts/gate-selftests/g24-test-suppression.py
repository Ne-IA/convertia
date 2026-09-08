#!/usr/bin/env python3
"""g24-test-suppression.py - G24 self-test for check-test-suppression (P0.3.14, G70).

Proves the "no green-by-rewrite" canary: a suppression MARKER (#[ignore]/it.skip/.only/#[should_panic])
or a removed/commented-out assertion in a TEST file FLAGS unless a [Test-Change] tag sits within ±6
lines; the #[cfg(test)] scoping (a marker in a src .rs counts ONLY inside its test block); both tag
shapes (old-obsolete+new-correct, new-test:<reason>) suppress; a clean test diff passes; and the
contract's enumerated cases (plant a marker/removed-assertion WITHOUT a tag ⇒ fail; WITH ⇒ pass).
Drives the pure fns + a real temp-git-repo E2E for --diff (GIT_* scrubbed), incl. a DELETED file: the staged
diff (read once, `-M`, deletions included, split per file) sees D; its HEAD blob's TEST-scope assertions flag
(a production module's `.expect(` does not); an in-file historical tag does not clear them; only an added-line
[Test-Change] tag NAMING the file (its path, or an unshared basename) justifies the deletion, one tag per
file; a rename that also retires an assertion keeps its `-` line; a pure `git mv` passes. stdlib-only.
Exit 0 = all held; 1 = failed.
"""
import contextlib
import importlib.machinery
import importlib.util
import io
import os
import subprocess
import sys
import tempfile
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-test-suppression"
_loader = importlib.machinery.SourceFileLoader("cts", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cts", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def mk(text: str, rel: str) -> list:
    return m.scan_markers(text, rel)


# --- markers flag in a test file (full/marker mode), per language ----------------------------
record("rust #[ignore] in a tests/ file -> flagged", len(mk("#[test]\n#[ignore]\nfn t() {}\n", "tests/a.rs")) == 1)
record("rust #[should_panic] in a tests/ file -> flagged", len(mk("#[should_panic]\nfn t() {}\n", "tests/a.rs")) == 1)
record("rust #[cfg(ignore)] in a tests/ file -> flagged", len(mk("#[cfg(ignore)]\nfn t() {}\n", "tests/a.rs")) == 1)
record("ts it.skip in a .test.ts -> flagged", len(mk("it.skip('x', () => {})\n", "src/a.test.ts")) == 1)
record("ts describe.only in a .spec.ts -> flagged", len(mk("describe.only('x', () => {})\n", "src/a.spec.ts")) == 1)
record("ts xit( in a .test.tsx -> flagged", len(mk("xit('x', () => {})\n", "src/a.test.tsx")) == 1)
record("ts it.todo in a .test.ts -> flagged", len(mk("it.todo('later')\n", "src/a.test.ts")) == 1)

# --- a marker in a NON-test file is out of scope ---------------------------------------------
record("a marker in a production .ts (not .test/.spec) -> NOT scanned", mk("it.skip('x')\n", "src/a.ts") == [])
record("a .md file is out of scope (test_scope_lines None)", m.test_scope_lines("# it.skip\n", "docs/x.md") is None)

# --- #[cfg(test)] scoping: a marker counts ONLY inside the test block of a src .rs ------------
SRC_RS = "fn prod() {\n    #[ignore]\n}\n#[cfg(test)]\nmod tests {\n    #[ignore]\n    fn t() {}\n}\n"
flagged_lines = {ln for ln, _, _ in mk(SRC_RS, "src-tauri/src/a.rs")}
record("src .rs: a marker INSIDE #[cfg(test)] is flagged", 6 in flagged_lines)
record("src .rs: a marker OUTSIDE #[cfg(test)] (production code) is NOT flagged", 2 not in flagged_lines)
record("src .rs with NO #[cfg(test)] block -> empty scope (nothing scanned)",
       m.test_scope_lines("fn prod() {\n    let x = 1;\n}\n", "src-tauri/src/a.rs") == set())
record("_cfg_test_ranges spans the attr line through the matched closer",
       m._cfg_test_ranges("a\n#[cfg(test)]\nmod t {\n x\n}\nb\n") == {2, 3, 4, 5})

# --- the justification tag (within ±6 lines) suppresses; both shapes ---------------------------
record("a marker WITH an old-obsolete+new-correct [Test-Change] tag within ±6 -> suppressed",
       mk("#[ignore]\n// [Test-Change: P0.3.14 — old-obsolete+new-correct, §6.4]\nfn t() {}\n", "tests/a.rs") == [])
record("a net-new #[should_panic] WITH a new-test:<reason> tag -> suppressed",
       mk("// [Test-Change: P0.3.14 — new-test:panic-path, §6.4]\n#[should_panic]\nfn t() {}\n", "tests/a.rs") == [])
FAR = "#[ignore]\n" + "\n" * 8 + "// [Test-Change: P0.3.14 — new-test:x, §6.4]\n"
record("a [Test-Change] tag >6 lines away -> NOT suppressed", len(mk(FAR, "tests/a.rs")) >= 1)
record("a malformed [Test-Change] tag (no sanctioned shape) -> NOT suppressed",
       len(mk("#[ignore]\n// [Test-Change: P0.3.14 just because]\nfn t() {}\n", "tests/a.rs")) >= 1)

# --- comment-stripping: a marker mentioned in a // comment is NOT flagged ---------------------
record("a marker in a // comment is NOT flagged (line-comment stripped)",
       mk("// example uses it.skip here\nconst x = 1;\n", "src/a.test.ts") == [])
record("a real it.skip with a trailing // comment IS flagged",
       len(mk("it.skip('x', () => {}) // disabled\n", "src/a.test.ts")) == 1)

# --- _scan_hunks: the DIFF signals (added marker / removed assertion / commented-out) ----------
def hunk(diff, rel="tests/a.rs"):
    return m._scan_hunks(diff, rel)


D_ADD_NOTAG = "@@ -1,2 +1,3 @@\n fn t() {\n+    #[ignore]\n     assert!(x);\n"
record("diff: an ADDED #[ignore] with no tag -> violation", len(hunk(D_ADD_NOTAG)) == 1)
D_ADD_TAG = "@@ -1,2 +1,4 @@\n fn t() {\n+    // [Test-Change: P0.3.14 — new-test:x, §6.4]\n+    #[ignore]\n     assert!(x);\n"
record("diff: an ADDED #[ignore] WITH a tag in the hunk -> no violation", hunk(D_ADD_TAG) == [])
D_RM_NOTAG = "@@ -1,3 +1,2 @@\n fn t() {\n-    assert_eq!(a, b);\n }\n"
record("diff: a REMOVED assertion with no tag -> violation", len(hunk(D_RM_NOTAG)) == 1)
D_RM_TAG = "@@ -1,3 +1,3 @@\n fn t() {\n+    // [Test-Change: P0.3.14 — old-obsolete+new-correct, §6.4]\n-    assert_eq!(a, b);\n }\n"
record("diff: a REMOVED assertion WITH a tag -> no violation", hunk(D_RM_TAG) == [])
D_COMMENTED = "@@ -1,2 +1,2 @@\n fn t() {\n+    // assert_eq!(a, b);\n"
record("diff: a newly COMMENTED-OUT assertion with no tag -> violation", len(hunk(D_COMMENTED)) == 1)
D_CLEAN = "@@ -1,1 +1,2 @@\n fn t() {\n+    let helper = 1;\n"
record("diff: a clean test change (no marker/removed-assertion) -> no violation", hunk(D_CLEAN) == [])

# --- string/comment-blanked `-` lines (2026-08-30, the check-29-calibration commit): prose is not
#     an assertion — a reworded reason/message naming expect(/assert( must not demand a tag no test
#     change could honestly carry; a real removed assertion survives the blanking -----------------
D_RM_STRING = ('@@ -1,3 +1,3 @@\n fn t() {\n'
               '-    reason = "the expect(dead_code) lint could not catch the rot"\n'
               '+    reason = "the module dead_code expectation could not catch the rot"\n }\n')
record("diff: a REMOVED line whose only expect( lives in a STRING literal -> prose, no violation",
       hunk(D_RM_STRING) == [])
D_RM_COMMENT = "@@ -1,3 +1,2 @@\n fn t() {\n-    // the old assert!(x) shape was retired at P3.86\n }\n"
record("diff: a REMOVED `//` comment line naming assert!( -> prose, no violation",
       hunk(D_RM_COMMENT) == [])
record("diff: a REAL removed assertion still flags after the blanking (tokens live outside literals)",
       len(hunk('@@ -1,3 +1,2 @@\n fn t() {\n-    assert_eq!(mangle("expect("), b);\n }\n')) == 1)
record("diff: a Rust LIFETIME tick does not hide a removed assertion (tick=False for .rs)",
       len(hunk("@@ -1,3 +1,2 @@\n fn t() {\n-    check::<'a>(v); assert!(v.ok());\n }\n")) == 1)
record("_string_blanked: literal bodies blanked, code preserved, unterminated quote blanks to EOL",
       m._string_blanked('assert_eq!(f("expect("), \'assert(\')')
       == "assert_eq!(f(\"_______\"), '_______')"
       and "expect(" not in m._string_blanked('x = "expect( unterminated'))

# --- the whole-deletion-run window (the 2026-07-12 P3.86 refinement): a REMOVED assertion is
#     justified by ONE tag within ±WINDOW of its contiguous `-`-run's BOUNDARIES — git emits every
#     `-` before any `+`, so a buried assert in an atomically-deleted test unit can never carry a
#     tag within ±WINDOW of itself; the tombstone belongs to the deletion EVENT ------------------
_RUN_BODY = ("-#[test]\n-fn gone() {\n-    assert_eq!(retired(), 7);\n-    let x1 = 1;\n-    let x2 = 2;\n"
             "-    let x3 = 3;\n-    let x4 = 4;\n-    let x5 = 5;\n-    let x6 = 6;\n-    let x7 = 7;\n-}\n")
D_RUN_NOTAG = "@@ -1,13 +1,2 @@\n fn keep() {}\n" + _RUN_BODY + " fn also_keep() {}\n"
record("diff: a buried assert in a whole-deletion run WITHOUT any tag -> violation",
       len(hunk(D_RUN_NOTAG)) == 1)
D_RUN_TOMBSTONE = ("@@ -1,13 +1,3 @@\n fn keep() {}\n" + _RUN_BODY
                   + "+// [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1]\n fn also_keep() {}\n")
record("diff: a buried assert (>±6 from the tag) in a whole-deletion run WITH one adjacent tombstone -> no violation",
       hunk(D_RUN_TOMBSTONE) == [])
D_RUN_FARTAG = ("@@ -1,20 +1,10 @@\n fn keep() {}\n" + _RUN_BODY
                + " c1();\n c2();\n c3();\n c4();\n c5();\n c6();\n c7();\n"
                + "+// [Test-Change: P3.77 — old-obsolete+new-correct, §7.8.1]\n")
record("diff: a tag BEYOND ±6 of the deletion run's boundary does NOT justify it -> violation",
       len(hunk(D_RUN_FARTAG)) == 1)
D_TS = "@@ -1,1 +1,2 @@\n describe('x', () => {\n+  it.only('only this', () => {})\n"
record("diff: an ADDED it.only in a .ts test with no tag -> violation", len(hunk(D_TS, "src/a.test.ts")) == 1)

# --- G1 r1 fixes: multi-line literal/lifetime brace state (P1) — a } in a multi-line literal must NOT
#     close the #[cfg(test)] scope early and eject a later #[ignore] from --full -----------------
RAW = ('#[cfg(test)]\nmod tests {\n    #[test] fn snap() { let e = r#"{ "k": 1 }"#; assert_eq!(r(), e); }\n'
       '    #[ignore]\n    #[test] fn t() { assert_eq!(1, 2); }\n}\n')
record("26-style: a } inside a multi-line r#\"…\"# does NOT close the cfg(test) scope (later #[ignore] flagged)",
       4 in {ln for ln, _, _ in mk(RAW, "src-tauri/src/a.rs")})
LIFE = ('#[cfg(test)]\nmod tests {\n    fn make<\'a>(s: &\'a str) -> &\'a str { s }\n'
        '    #[ignore]\n    #[test] fn t() { assert_eq!(1, 2); }\n}\n')
record("lifetime: a &'a apostrophe does NOT eat the opening brace (later #[ignore] still flagged)",
       4 in {ln for ln, _, _ in mk(LIFE, "src-tauri/src/a.rs")})
MLSTR = ('#[cfg(test)]\nmod tests {\n    fn s() { let e = "line\n    } still in the string"; }\n'
         '    #[ignore]\n    #[test] fn t() {}\n}\n')
record("multi-line regular string: a } in its body does NOT close the cfg(test) scope early",
       any(ln >= 4 for ln in {l for l, _, _ in mk(MLSTR, "src-tauri/src/a.rs")}))
# G1 round 2: Rust block comments NEST — a } between an inner */ and the outer */ must NOT leak
NEST1 = "#[cfg(test)]\nmod tests {\n    /* disable: /* note */ } */\n    #[ignore]\n    fn t() { assert_eq!(real(), 1); }\n}\n"
record("nested block comment (one-line, stray } inside) does NOT eject the later #[ignore]",
       4 in {ln for ln, _, _ in mk(NEST1, "src-tauri/src/a.rs")})
NEST2 = "#[cfg(test)]\nmod tests {\n    /* dead:\n    fn old() { /* inner */ }\n    more }\n    */\n    #[ignore]\n    #[test] fn t() {}\n}\n"
record("nested block comment (multi-line region) does NOT eject the later #[ignore]",
       7 in {ln for ln, _, _ in mk(NEST2, "src-tauri/src/a.rs")})
record("a BALANCED single block comment still blanks correctly (#[ignore] in scope)",
       4 in {ln for ln, _, _ in mk("#[cfg(test)]\nmod tests {\n    /* dead fn old() {} */\n    #[ignore]\n    fn t() {}\n}\n", "src-tauri/src/a.rs")})

# G1 round 3: a MULTI-LINE attribute (rustfmt wraps a long #[cfg_attr(…, ignore)]) must still be caught
record("a multi-line #[\\n ignore\\n] is caught (line-wrapped attribute, --full plane)",
       len(mk("#[test]\n#[\n    ignore\n]\nfn flaky() { assert_eq!(2 + 2, 5); }\n", "tests/a.rs")) == 1)
record("a rustfmt-wrapped #[cfg_attr(\\n …, ignore\\n)] is caught",
       len(mk("#[cfg_attr(\n    target_os = \"windows\",\n    ignore\n)]\n#[test]\nfn t() {}\n", "tests/a.rs")) == 1)
record("a multi-line #[ignore] inside a src .rs #[cfg(test)] block is caught",
       4 in {ln for ln, _, _ in mk("fn prod() {}\n#[cfg(test)]\nmod tests {\n    #[\n        ignore\n    ]\n    fn t() {}\n}\n", "src-tauri/src/a.rs")})
record("a multi-line #[ignore] WITH a [Test-Change] tag within ±6 is suppressed",
       mk("// [Test-Change: P0.3.14 — new-test:scaffold, §6.4]\n#[\n    ignore\n]\nfn t() {}\n", "tests/a.rs") == [])
record("a multi-line #[derive(...)] (not a marker) is NOT flagged (no false positive)",
       mk("#[derive(\n    Debug,\n    Clone\n)]\nstruct S;\n", "tests/a.rs") == [])
record("a line-wrapped #[cfg(\\n test\\n)] is still SCOPE-detected (a marker inside is flagged)",
       6 in {ln for ln, _, _ in mk("fn prod() {}\n#[cfg(\n    test\n)]\nmod tests {\n    #[ignore]\n    fn t() {}\n}\n", "src-tauri/src/a.rs")})

# --- G1 r1: TAG_RE must carry a real box-id (P1) -----------------------------------------------
record("a [Test-Change] tag with NO box-id does NOT suppress",
       len(mk("#[ignore]\n// [Test-Change:  — old-obsolete+new-correct, §6.4]\nfn t() {}\n", "tests/a.rs")) >= 1)
record("a [Test-Change] tag WITH a P-box-id suppresses",
       mk("#[ignore]\n// [Test-Change: P5.3 — old-obsolete+new-correct, §6.4]\nfn t() {}\n", "tests/a.rs") == [])

# --- G1 r1: ASSERT_RE covers the project's property-test + chained-matcher families (P2/P3) ----
def _rm(line, rel="tests/a.rs"):
    return m._scan_hunks("@@ -1,2 +1,1 @@\n fn t() {\n" + line + "\n", rel)


record("removed prop_assert! is caught (proptest is first-class here)", len(_rm("-    prop_assert!(x);")) == 1)
record("removed assert_matches! is caught", len(_rm("-    assert_matches!(x, Y);")) == 1)
record("removed ensure! is caught", len(_rm("-    ensure!(cond);")) == 1)
record("a removed chained .toBe(...) on its OWN line is caught (split-line, no word char before the dot)",
       len(_rm("-    .toBe(expected)", "src/a.test.ts")) == 1)

# --- G1 r1: the extended marker set (P3 — real test-disable idioms) ----------------------------
record("#[cfg(any())] (always-false cfg = disabled test) is flagged", len(mk("#[cfg(any())]\nfn t() {}\n", "tests/a.rs")) == 1)
record("#[cfg_attr(unix, ignore)] is flagged", len(mk("#[cfg_attr(unix, ignore)]\nfn t() {}\n", "tests/a.rs")) == 1)
record("jasmine fdescribe( is flagged", len(mk("fdescribe('x', () => {})\n", "src/a.spec.ts")) == 1)
record("vitest it.skipIf( is flagged", len(mk("it.skipIf(cond)('x', () => {})\n", "src/a.test.ts")) == 1)

# --- the live --full passes on the real repo (the test files carry no unjustified markers) ----
record("--full passes on the real repo (test files clean — no unjustified markers)", m.main(["--full"]) == 0)


# --- run_diff E2E in a real temp git repo (the staged-blob path + fail-open-without-base) ------
# The throwaway-repo git calls and the in-process `m.main(["--diff"])` runs get an environment WITHOUT the
# GIT_* location variables a hook plane could carry (GIT_DIR / GIT_INDEX_FILE / GIT_WORK_TREE / ...): with one
# of them absolute, `git -C <tmp> add` + `commit` would operate on the OUTER repository. GIT_EXEC_PATH stays.
for _k in [k for k in os.environ if k.startswith("GIT_") and k != "GIT_EXEC_PATH"]:
    os.environ.pop(_k)


def _git(repo, *a):
    subprocess.run(["git", "-C", str(repo), "-c", "commit.gpgsign=false", *a], capture_output=True, text=True,
                   encoding="utf-8", errors="replace", check=True)


with tempfile.TemporaryDirectory() as td:
    repo = Path(td)
    _git(repo, "init", "-q", "-b", "main"); _git(repo, "config", "user.email", "t@t.t"); _git(repo, "config", "user.name", "t")
    (repo / "README").write_text("x\n", encoding="utf-8")
    _git(repo, "add", "README"); _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "init")
    (repo / "tests").mkdir()
    tf = repo / "tests" / "a.rs"
    tf.write_text("#[test]\nfn t() {\n    assert_eq!(1, 1);\n}\n", encoding="utf-8")
    _git(repo, "add", "tests/a.rs"); _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "base test")
    cwd = os.getcwd()
    # (1) stage an #[ignore] with NO tag -> run_diff fails
    tf.write_text("#[test]\n#[ignore]\nfn t() {\n    assert_eq!(1, 1);\n}\n", encoding="utf-8")
    _git(repo, "add", "tests/a.rs")
    os.chdir(repo)
    try:
        rc_notag = m.main(["--diff"])
        # (2) add a [Test-Change] tag next to it -> run_diff passes
        tf.write_text("#[test]\n// [Test-Change: P0.3.14 — new-test:scaffold, §6.4]\n#[ignore]\nfn t() {\n    assert_eq!(1, 1);\n}\n", encoding="utf-8")
        _git(repo, "add", "tests/a.rs")
        rc_tag = m.main(["--diff"])
    finally:
        os.chdir(cwd)
    record("run_diff: a staged #[ignore] with NO [Test-Change] tag -> rc 1", rc_notag == 1)
    record("run_diff: the same marker WITH a [Test-Change] tag -> rc 0", rc_tag == 0)

# --- a DELETED file (2026-09-08): the staged diff is read ONCE (`-M`, deletions included) and split per file so
# git's rename pairing survives; a deleted file's HEAD blob yields its TEST-scope assertions (a production
# `.expect(` does not); in-file historical tags do not count; only an added-line [Test-Change] tag NAMING the
# file (its path, or its basename when no other tracked file shares it) justifies the deletion (pure fns, the
# source pins, then E2E in a GIT_*-scrubbed repo) ---
_blob_test = "#[test]\nfn t() {\n    assert_eq!(1, 1);\n}\n// [Test-Change: P1.1 — old-obsolete+new-correct, §x: an OLD in-file tag]\n"
record("deleted file: a tests/ blob's assertion is read from the HEAD blob (the in-file historical tag does not clear it)",
       len(m.deleted_file_assertions(_blob_test, "tests/a.rs")) == 1)
record("deleted file: a src .rs blob yields only its #[cfg(test)]-scoped assertions - a production `.expect(` is no test change",
       m.deleted_file_assertions("fn load() { std::fs::read_to_string(p).expect(\"readable\"); }\n", "src-tauri/src/prefs.rs") == []
       and len(m.deleted_file_assertions("fn load() { x.expect(\"prod\"); }\n#[cfg(test)]\nmod t {\n    fn a() { assert!(true); }\n}\n",
                                         "src-tauri/src/prefs.rs")) == 1)
record("deleted file: a `// expect(` prose line and a string-embedded `assert!(` are not assertions",
       m.deleted_file_assertions("// the caller must expect( a value\nlet s = \"assert!(x)\";\n", "tests/a.rs") == [])
record("deleted file: added-line tags are collected PER FILE (a tag wrapped over two comment lines is ONE tag; an unclosed "
       "opener in one file cannot borrow a `]` from another; removed lines and `+++` headers are not)",
       len(m.added_line_tags("+++ b/tests/b.rs\n@@ -1,1 +1,3 @@\n+// [Test-Change: P4.33 — old-obsolete+new-correct, §6.4: tests/a.rs retired,\n+// its case moved here]\n")) == 1
       and m.added_line_tags("+++ b/README.md\n@@ -1,1 +1,2 @@\n+see [Test-Change: P4.33 — old-obsolete+new-correct, unrelated prose\n"
                             "diff --git a/other.md b/other.md\n+++ b/other.md\n@@ -1,1 +1,2 @@\n+mentioning tests/a.rs] in a bracket\n") == []
       and m.added_line_tags("+++ b/tests/b.rs\n@@ -1,2 +1,2 @@\n+// no tag\n-// [Test-Change: P4.33 — old-obsolete+new-correct, §6.4: a.rs] on a REMOVED line\n") == []
       and m.added_line_tags("+++ b/[Test-Change: P4.33 — old-obsolete+new-correct, §6.4].rs\n@@ -1,1 +1,1 @@\n+x\n") == []
       and len(m.added_line_tags("diff --git a/tests/b.rs b/tests/b.rs\n+++ b/tests/b.rs\n@@ -1,1 +1,3 @@\n"
                                 "+// [Test-Change: P4.33 — old-obsolete+new-correct, §6.4: tests/a.rs retired,\n"
                                 "+++ a body line whose content begins ++\n+// its case moved here]\n")) == 1)
record("deleted file: basename uniqueness counts HEAD's tree UNION the staged new paths - `b.rs` is shared once `src/b.rs` is staged",
       m.shared_basenames_of(["tests/b.rs", "src/x/mod.rs", "src/y/mod.rs"]) == {"mod.rs"}
       and m.shared_basenames_of(["tests/b.rs", "src/b.rs"]) == {"b.rs"})
_tag = lambda s: [f"[Test-Change: P4.33 — old-obsolete+new-correct, §6.4: {s} retired]"]
record("deleted file: the justifying tag must NAME the deleted file - its path always; its basename only when no other tracked file shares it",
       m.deletion_justified(_tag("tests/a.rs"), "tests/a.rs", set())
       and m.deletion_justified(_tag("a.rs"), "tests/a.rs", set())
       and not m.deletion_justified(_tag("b.rs"), "tests/a.rs", set())
       and not m.deletion_justified([], "tests/a.rs", set()))
record("deleted file: a SHARED basename (mod.rs) is named only by its path - `x/mod.rs` does not justify deleting `y/mod.rs`",
       not m.deletion_justified(_tag("src-tauri/src/x/mod.rs"), "src-tauri/src/y/mod.rs", {"mod.rs"})
       and not m.deletion_justified(_tag("mod.rs"), "src-tauri/src/y/mod.rs", {"mod.rs"})
       and m.deletion_justified(_tag("src-tauri/src/y/mod.rs"), "src-tauri/src/y/mod.rs", {"mod.rs"}))
record("deleted file: the name match is segment-bounded - a tag naming `tests/ab.rs` does not cover `tests/b.rs`",
       not m.deletion_justified(_tag("tests/ab.rs"), "tests/b.rs", set())
       and not m.deletion_justified(_tag("ab.rs"), "tests/b.rs", set()))
_split = m.split_staged_diff(
    "diff --git a/tests/a.rs b/tests/b.rs\nsimilarity index 96%\nrename from tests/a.rs\nrename to tests/b.rs\n"
    "--- a/tests/a.rs\n+++ b/tests/b.rs\n@@ -1,3 +1,3 @@\n #[test]\n-    assert_eq!(1, 1);\n+    // removed\n }\n"
    "diff --git a/tests/c.rs b/tests/c.rs\ndeleted file mode 100644\n--- a/tests/c.rs\n+++ /dev/null\n@@ -1,1 +0,0 @@\n-fn c() {}\n"
    "diff --git a/src/d.rs b/src/d.rs\n--- a/src/d.rs\n+++ b/src/d.rs\n@@ -1,1 +1,1 @@\n-x\n+y\n")
record("deleted file: the combined diff splits per file - a RENAME keeps its `-` lines under the new path, a deletion is keyed by the old path",
       [(o, n, d) for o, n, d, _ in _split] == [("tests/a.rs", "tests/b.rs", False), ("tests/c.rs", "tests/c.rs", True), ("src/d.rs", "src/d.rs", False)]
       and "-    assert_eq!(1, 1);" in _split[0][3] and len(m._scan_hunks(_split[0][3], "tests/b.rs")) == 1)
_sp = lambda text: [(o, n, d) for o, n, d, _ in m.split_staged_diff(text)]
record("deleted file: every chunk's identity is SEEDED from its `diff --git` line - a content-identical rename, an EMPTY add, a"
       " BINARY add, a mode-only change and an EMPTY deletion (none carries a `---`/`+++` pair) all keep their paths",
       _sp("diff --git a/src/old.rs b/src/b.rs\nsimilarity index 100%\nrename from src/old.rs\nrename to src/b.rs\n")
       == [("src/old.rs", "src/b.rs", False)]
       and _sp("diff --git a/src/b.rs b/src/b.rs\nnew file mode 100644\nindex 0000000..e69de29\n") == [("src/b.rs", "src/b.rs", False)]
       and _sp("diff --git a/x.bin b/x.bin\nnew file mode 100644\nindex 0000000..1234567\nBinary files /dev/null and b/x.bin differ\n")
       == [("x.bin", "x.bin", False)]
       and _sp("diff --git a/src/m.rs b/src/m.rs\nold mode 100644\nnew mode 100755\n") == [("src/m.rs", "src/m.rs", False)]
       and _sp("diff --git a/tests/e.rs b/tests/e.rs\ndeleted file mode 100644\nindex e69de29..0000000\n") == [("tests/e.rs", "tests/e.rs", True)])
record("deleted file: an UNQUOTED `diff --git` path with a space - even a directory ending in ` b` - is split where the halves"
       " agree (git quotes only a quote, a backslash or a control char); a rename takes the first ` b/` and its rename lines correct it",
       m._diff_git_paths("diff --git a/tests/a b.rs b/tests/a b.rs") == ("tests/a b.rs", "tests/a b.rs")
       and m._diff_git_paths("diff --git a/tests/notes b/x.rs b/tests/notes b/x.rs") == ("tests/notes b/x.rs", "tests/notes b/x.rs")
       and _sp("diff --git a/tests/notes b/x.rs b/tests/notes b/x.rs\ndeleted file mode 100644\nindex 1234567..0000000\n"
               "Binary files a/tests/notes b/x.rs and /dev/null differ\n") == [("tests/notes b/x.rs", "tests/notes b/x.rs", True)]
       and _sp("diff --git a/tests/a.rs b/tests/b.rs\nsimilarity index 90%\nrename from tests/a.rs\nrename to tests/b.rs\n"
               "--- a/tests/a.rs\n+++ b/tests/b.rs\n@@ -1,1 +1,1 @@\n-x\n+y\n") == [("tests/a.rs", "tests/b.rs", False)])
record("deleted file: a QUOTED `diff --git` line (a path with a quote, a backslash or an octal-escaped byte) is C-unquoted",
       m._diff_git_paths('diff --git "a/tests/a\\"b.rs" "b/tests/a\\"b.rs"') == ('tests/a"b.rs', 'tests/a"b.rs')
       and m._diff_git_paths('diff --git "a/tests/\\303\\274.rs" "b/tests/\\303\\274.rs"') == ("tests/ü.rs", "tests/ü.rs")
       and m._diff_git_paths('diff --git "a/x\\\\y.rs" "b/x\\\\y.rs"') == ("x\\y.rs", "x\\y.rs"))
record("deleted file: the `---`/`+++`/`rename from`/`rename to` refinements C-unquote a quoted path like the seed does (the round-12"
       " finding: `.strip('\"')` kept the escapes and overwrote the seed, so a quoted deletion's HEAD blob was unreadable and skipped)",
       _sp('diff --git "a/tests/a\\"b.rs" "b/tests/a\\"b.rs"\ndeleted file mode 100644\n--- "a/tests/a\\"b.rs"\n+++ /dev/null\n@@ -1,1 +0,0 @@\n-x\n')
       == [('tests/a"b.rs', 'tests/a"b.rs', True)]
       and _sp('diff --git "a/tests/a\\"b.rs" "b/tests/c\\"b.rs"\nsimilarity index 90%\nrename from "tests/a\\"b.rs"\nrename to "tests/c\\"b.rs"\n'
               '--- "a/tests/a\\"b.rs"\n+++ "b/tests/c\\"b.rs"\n@@ -1,1 +1,1 @@\n-x\n+y\n') == [('tests/a"b.rs', 'tests/c"b.rs', False)]
       and m._header_path(' "a/x\\\\y.rs"') == "a/x\\y.rs" and m._header_path(" a/plain.rs ") == "a/plain.rs")
record("deleted file: a chunk whose `diff --git` line cannot be seeded (no `a/` prefix) is KEPT with empty paths for run_diff's"
       " notice, never dropped silently",
       _sp("diff --git x.rs x.rs\ndeleted file mode 100644\nindex 1234567..0000000\nBinary files x.rs and /dev/null differ\n") == [("", "", False)]
       and "could not be seeded" in SCRIPT.read_text(encoding="utf-8"))
record("deleted file: the chunk's header scan stops at its first `@@` - an in-hunk `--- banner` / `+++ decorative` line never re-keys it",
       _sp("diff --git a/t.rs b/t.rs\n--- a/t.rs\n+++ b/t.rs\n@@ -1,1 +1,3 @@\n+++ decorative\n--- banner\n+++ /dev/null\n")
       == [("t.rs", "t.rs", False)])
record("deleted file: the staged diff is read ONCE with rename detection forced and deletions + typechanges included (-M --diff-filter=ACMRDT)",
       SCRIPT.read_text(encoding="utf-8").count('"-M",\n                    "--diff-filter=ACMRDT"') == 1
       and 'git(*GIT_DIFF_SYNTAX, "diff", "--cached", f"--unified={WINDOW}", *GIT_DIFF_FLAGS, "-M",' in SCRIPT.read_text(encoding="utf-8")
       and SCRIPT.read_text(encoding="utf-8").count('"--cached", "--name-only"') == 0)

record("deleted file: the name match's RIGHT boundary - `tests/b.rs.bak` and `tests/b.rs-old` do not cover `tests/b.rs`; a sentence-ending `.` does",
       not m.deletion_justified(_tag("tests/b.rs.bak"), "tests/b.rs", set())
       and not m.deletion_justified(_tag("tests/b.rs-old"), "tests/b.rs", set())
       and m.deletion_justified(["[Test-Change: P4.33 — old-obsolete+new-correct, §6.4: retired tests/b.rs.]"], "tests/b.rs", set())
       and m.deletion_justified(["[Test-Change: P4.33 — old-obsolete+new-correct, §6.4: retired `tests/b.rs`]"], "tests/b.rs", set()))
record("deleted file: when HEAD's tree cannot be listed (shared basenames unknown) the rule is PATH-only - fail-closed, never permissive",
       not m.deletion_justified(_tag("a.rs"), "tests/a.rs", None)
       and m.deletion_justified(_tag("tests/a.rs"), "tests/a.rs", None)
       and "HEAD tree unlistable" in SCRIPT.read_text(encoding="utf-8"))
record("deleted file: the per-file split reports the OLD path of a rename too (a move OUT of the parsed set arrives as a deletion under"
       " the pathspec and its HEAD blob is read; the scope test reads the new path)",
       [(o, n) for o, n, _, _ in m.split_staged_diff(
           "diff --git a/tests/a.rs b/docs/a.md\nsimilarity index 90%\nrename from tests/a.rs\nrename to docs/a.md\n"
           "--- a/tests/a.rs\n+++ b/docs/a.md\n@@ -1,2 +1,2 @@\n-    assert_eq!(1, 1);\n+    // gone\n x\n")] == [("tests/a.rs", "docs/a.md")])
record("hunk scan: a blank context line rendered EMPTY (`diff.suppressBlankEmpty=true` emits '' for ' ') is still a body line, so the"
       " +/-WINDOW tag window is not compressed (the round-13 finding: three elided blanks pulled an unrelated tag inside the window)",
       (lambda blank: m._scan_hunks("@@ -1,9 +1,8 @@\n-    assert_eq!(1, 1);\n" + " fn a() {}\n" + blank + " fn b() {}\n" + blank
                                     + " fn c() {}\n" + blank + " fn d() {}\n // [Test-Change: P4.33 — old-obsolete+new-correct, §6.4: tests/zz.rs]\n",
                                     "tests/a.rs"))(" \n") == (lambda blank: m._scan_hunks(
           "@@ -1,9 +1,8 @@\n-    assert_eq!(1, 1);\n" + " fn a() {}\n" + blank + " fn b() {}\n" + blank + " fn c() {}\n" + blank
           + " fn d() {}\n // [Test-Change: P4.33 — old-obsolete+new-correct, §6.4: tests/zz.rs]\n", "tests/a.rs"))("\n")
       and m._scan_hunks("@@ -1,9 +1,8 @@\n-    assert_eq!(1, 1);\n fn a() {}\n\n fn b() {}\n\n fn c() {}\n\n fn d() {}\n"
                         " // [Test-Change: P4.33 — old-obsolete+new-correct, §6.4: tests/zz.rs]\n", "tests/a.rs") != [])
record("line model: lines are split on git's `\\n` only - a lone CR or form feed stays INSIDE its line (a trailing newline opens no"
       " empty line), so a `+` line carrying a CR-joined tag is ONE added line that carries the tag (TAG_RE is a substring search by"
       " design - a tag may trail code; under `str.splitlines()` the tag's pseudo-line lost its `+` and the tag vanished, the round-14"
       " review; the CR byte itself is G52's to reject at L1)",
       m._lines("a\rb\n") == ["a\rb"] and m._lines("a\x0cb\nc") == ["a\x0cb", "c"] and m._lines("") == []
       and len(m.added_line_tags("diff --git a/tests/n.rs b/tests/n.rs\n--- a/tests/n.rs\n+++ b/tests/n.rs\n@@ -0,0 +1 @@\n"
                                 "+// harmless \r// [Test-Change: P4.33 — old-obsolete+new-correct, §6.4: tests/x.rs]\n")) == 1)
record("source pin: the git reads return BYTES decoded as UTF-8 (no universal-newline translation), and the staged diff is scoped to"
       " the paths this gate parses (`.rs` + the TS test files, one list with TEST_TS_RE) so `--text` never renders an icon",
       'subprocess.run(["git", *args], env=_git_env(), capture_output=True)\n    return p.returncode, _decode(p.stdout)' in SCRIPT.read_text(encoding="utf-8")
       and '"--diff-filter=ACMRDT", "--", *SCOPE_PATHSPECS)' in SCRIPT.read_text(encoding="utf-8")
       and m.SCOPE_PATHSPECS[0] == ":/*.rs" and ":/*.test.mjs" in m.SCOPE_PATHSPECS and ":/*.spec.tsx" in m.SCOPE_PATHSPECS
       and len(m.SCOPE_PATHSPECS) == 13 and all(p.startswith(":/*.") and m.TEST_TS_RE.search(p[3:]) for p in m.SCOPE_PATHSPECS[1:])
       and SCRIPT.read_text(encoding="utf-8").replace("str.splitlines()", "").count("splitlines()") == 0)
record("line model: the removed-line forgery IS closed - a `-` line whose CR-remainder begins `+` (`-let x = 1;\\r+[Test-Change: …]`) is"
       " ONE removed line under git's model and yields no tag (under `str.splitlines()` the remainder was buffered as an added line and"
       " forged one - the round-14 review's shape, reproduced in round 15)",
       m.added_line_tags("diff --git a/tests/n.rs b/tests/n.rs\n--- a/tests/n.rs\n+++ b/tests/n.rs\n@@ -1,1 +0,0 @@\n"
                         "-let x = 1;\r+[Test-Change: P4.33 — old-obsolete+new-correct, §6.4: tests/x.rs]\n") == [])
record("source pin: the `--full` mirror's tracked-file list is read `ls-files -z` (NUL-separated, never quoted - the round-15 finding:"
       " a quoted non-ASCII test file failed the extension test and the fail-closed plane scanned nobody)",
       'git("ls-files", "-z", "--full-name", "--", ":/")' in SCRIPT.read_text(encoding="utf-8") and 'git("ls-files")' not in SCRIPT.read_text(encoding="utf-8"))
record("deleted file: the diff-SYNTAX pins - non-ASCII literal, `a/`/`b/` prefixes forced, blank context never elided, hunks never merged,"
       " paths never relative, no external driver, no textconv, every file read as text - are one constant pair on the diff read; HEAD's"
       " tree is listed `-z` (never quoted); the env knob that outranks the command line (GIT_DIFF_OPTS) is scrubbed",
       m.GIT_DIFF_SYNTAX == ("-c", "core.quotepath=false", "-c", "diff.noprefix=false", "-c", "diff.mnemonicPrefix=false",
                             "-c", "diff.srcPrefix=a/", "-c", "diff.dstPrefix=b/", "-c", "diff.suppressBlankEmpty=false",
                             "-c", "diff.relative=false", "-c", "diff.interHunkContext=0")
       and m.GIT_DIFF_FLAGS == ("--no-ext-diff", "--no-textconv", "--text", "--no-color")
       and m.SCRUBBED_GIT_ENV == ("GIT_DIFF_OPTS", "GIT_EXTERNAL_DIFF")
       and 'subprocess.run(["git", *args], env=_git_env(),' in SCRIPT.read_text(encoding="utf-8")
       and SCRIPT.read_text(encoding="utf-8").count('"ls-tree", "-r", "-z", "--full-tree", "--name-only", "HEAD"') == 1)


def _e2e(setup, staged, config=(), cwd_sub=None) -> tuple[int, str]:
    """A throwaway repo (`config`: extra `git config` pairs, for the user-config class): `setup(repo)` writes + commits
    the base, `staged(repo)` stages the change; then --diff."""
    with tempfile.TemporaryDirectory() as td:
        repo = Path(td)
        _git(repo, "init", "-q", "-b", "main"); _git(repo, "config", "user.email", "t@t.t"); _git(repo, "config", "user.name", "t")
        for k, v in config:
            _git(repo, "config", k, v)
        setup(repo)
        _git(repo, "add", "-A"); _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "base")
        staged(repo)
        cwd = os.getcwd(); os.chdir(repo / cwd_sub if cwd_sub else repo)
        buf = io.StringIO()
        try:
            with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(buf):
                rc = m.main(["--diff"])
        finally:
            os.chdir(cwd)
        return rc, buf.getvalue()


def _w(repo, rel, text):
    p = repo / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")


_tests_blob = "#[test]\nfn t() {\n    assert_eq!(1, 1);\n}\n"
_tagged_blob = _tests_blob + "// [Test-Change: P1.1 — old-obsolete+new-correct, §x: an OLD in-file tag]\n"
_note = lambda s: f"// [Test-Change: P4.33 — old-obsolete+new-correct, §6.4: {s} retired, its case moved here]\n"
record("run_diff E2E: a staged `git rm` of a test file with NO tag anywhere -> rc 1 (the wired path sees D)",
       _e2e(lambda r: _w(r, "tests/a.rs", _tests_blob), lambda r: _git(r, "rm", "-q", "tests/a.rs"))[0] == 1)
record("run_diff E2E: the same deletion WITH an added-line [Test-Change] tag NAMING the file -> rc 0",
       _e2e(lambda r: _w(r, "tests/a.rs", _tests_blob),
            lambda r: (_git(r, "rm", "-q", "tests/a.rs"), _w(r, "tests/note.rs", _note("tests/a.rs")), _git(r, "add", "-A")))[0] == 0)
record("run_diff E2E: an added-line tag naming ANOTHER file does not justify this deletion -> rc 1",
       _e2e(lambda r: _w(r, "tests/a.rs", _tests_blob),
            lambda r: (_git(r, "rm", "-q", "tests/a.rs"), _w(r, "tests/note.rs", _note("tests/zz.rs")), _git(r, "add", "-A")))[0] == 1)
record("run_diff E2E: the deleted file's OWN historical tag does not clear its assertions -> rc 1",
       _e2e(lambda r: _w(r, "tests/a.rs", _tagged_blob), lambda r: _git(r, "rm", "-q", "tests/a.rs"))[0] == 1)
record("run_diff E2E: a deleted PRODUCTION module whose `.expect(` sits outside any test scope -> rc 0 (no test change)",
       _e2e(lambda r: _w(r, "src-tauri/src/prefs.rs", "pub fn load(p: &str) -> String { std::fs::read_to_string(p).expect(\"readable\") }\n"),
            lambda r: _git(r, "rm", "-q", "src-tauri/src/prefs.rs"))[0] == 0)
_cfg_blob = "pub fn f() {}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() { assert_eq!(1, 1); }\n}\n"
_rc, _out = _e2e(lambda r: (_w(r, "src-tauri/src/x/mod.rs", _cfg_blob), _w(r, "src-tauri/src/y/mod.rs", _cfg_blob)),
                 lambda r: (_git(r, "rm", "-q", "src-tauri/src/x/mod.rs", "src-tauri/src/y/mod.rs"),
                            _w(r, "tests/note.rs", _note("src-tauri/src/x/mod.rs")), _git(r, "add", "-A")))
record("run_diff E2E: two deleted `mod.rs` files and ONE tag naming only x/mod.rs -> rc 1 naming y/mod.rs (one tag per retired file)",
       _rc == 1 and "src-tauri/src/y/mod.rs" in _out and "justified" in _out and "x/mod.rs" in _out.split("justified")[0])
_filler = "".join(f"// filler line {i}\n" for i in range(40))
record("run_diff E2E: a `git mv` that ALSO retires the assertion (git pairs it as a rename) -> rc 1 (the combined diff keeps the `-` line)",
       _e2e(lambda r: _w(r, "tests/a.rs", _tests_blob + _filler),
            lambda r: (_git(r, "mv", "tests/a.rs", "tests/b.rs"),
                       _w(r, "tests/b.rs", "#[test]\nfn t() {\n    // assertion removed\n}\n" + _filler), _git(r, "add", "-A")))[0] == 1)
record("run_diff E2E: a pure `git mv` of a test file passes even with `diff.renames=false` (rename detection is forced with -M) -> rc 0",
       _e2e(lambda r: (_w(r, "tests/a.rs", _tests_blob + _filler), _git(r, "config", "diff.renames", "false")),
            lambda r: _git(r, "mv", "tests/a.rs", "tests/b.rs"))[0] == 0)
record("run_diff E2E: a `git mv tests/a.rs docs/a.md` that retires the assertion (a rename OUT of the test namespace) -> rc 1",
       _e2e(lambda r: _w(r, "tests/a.rs", _tests_blob + _filler),
            lambda r: ((r / "docs").mkdir(), _git(r, "mv", "tests/a.rs", "docs/a.md"),
                       _w(r, "docs/a.md", "#[test]\nfn t() {\n    // assertion removed\n}\n" + _filler), _git(r, "add", "-A")))[0] == 1)
record("run_diff E2E: a deleted test file with a NON-ASCII name is read from HEAD (no quoting mangling) -> rc 1",
       _e2e(lambda r: _w(r, "tests/über.rs", _tests_blob), lambda r: _git(r, "rm", "-q", "tests/über.rs"))[0] == 1)
record("run_diff E2E: deleting tests/foo/b.rs while MOVING src/old.rs to src/b.rs unchanged (a 100% rename, no hunk) still makes"
       " `b.rs` ambiguous -> rc 1",
       _e2e(lambda r: (_w(r, "tests/foo/b.rs", _tests_blob), _w(r, "src/old.rs", "pub fn old() {}\n")),
            lambda r: (_git(r, "rm", "-q", "tests/foo/b.rs"), _git(r, "mv", "src/old.rs", "src/b.rs"),
                       _w(r, "tests/note.rs", _note("b.rs")), _git(r, "add", "-A")))[0] == 1)
record("run_diff E2E: a NUL-carrying `tests/notes b/x.rs` - a path whose directory ends in ` b`, read as text under `--text` (the"
       " pair-less binary shape is the pure leg's) - deleted without a tag -> rc 1",
       _e2e(lambda r: _w(r, "tests/notes b/x.rs", "#[test]\nfn t() {\n    assert_eq!(1, 1);\n}\n\x00"),
            lambda r: _git(r, "rm", "-q", "tests/notes b/x.rs"))[0] == 1)
record("run_diff E2E: `diff.noprefix=true` in the repo config cannot hide a NUL-carrying test file's deletion - the diff SYNTAX is pinned"
       " on the command line (the round-12 finding: the unprefixed `diff --git` line seeded nothing and the chunk vanished silently)"
       " -> rc 1",
       _e2e(lambda r: _w(r, "tests/x.rs", _tests_blob + "\x00"), lambda r: _git(r, "rm", "-q", "tests/x.rs"),
            config=[("diff.noprefix", "true")])[0] == 1)
record("run_diff E2E: `diff.mnemonicPrefix=true` cannot re-key a deleted test file to `c/...` (its HEAD blob would be unreadable) -> rc 1",
       _e2e(lambda r: _w(r, "tests/x.rs", _tests_blob), lambda r: _git(r, "rm", "-q", "tests/x.rs"),
            config=[("diff.mnemonicPrefix", "true")])[0] == 1)
record("run_diff E2E: a `diff.external` driver a user set (one that cannot run) neither replaces the patch nor fails the read into a"
       " skip-with-notice -> rc 1",
       _e2e(lambda r: _w(r, "tests/x.rs", _tests_blob), lambda r: _git(r, "rm", "-q", "tests/x.rs"),
            config=[("diff.external", "g24-no-such-diff-driver")])[0] == 1)
record("run_diff E2E: HEAD's tree is listed `-z` (never quoted, whatever core.quotepath says), so two `ü.rs` files make a bare `ü.rs`"
       " tag ambiguous -> rc 1",
       _e2e(lambda r: (_w(r, "tests/ü.rs", _tests_blob), _w(r, "src/ü.rs", "pub fn f() {}\n")),
            lambda r: (_git(r, "rm", "-q", "tests/ü.rs"), _w(r, "tests/note.rs", _note("ü.rs")), _git(r, "add", "-A")),
            config=[("core.quotepath", "true")])[0] == 1)
record("run_diff E2E: a NUL byte anywhere in a MODIFIED test file (git's own binary detection, no config needed) cannot hide its removed"
       " assertion behind `Binary files differ` - every file is read `--text` (the round-13 P0) -> rc 1",
       _e2e(lambda r: _w(r, "tests/x.rs", _tests_blob + "// \x00 note\n"),
            lambda r: (_w(r, "tests/x.rs", "#[test]\nfn t() {\n}\n// \x00 note\n"), _git(r, "add", "-A")))[0] == 1)
record("run_diff E2E: a user attribute marking `.rs` binary (`.git/info/attributes`, or a global attributes file) cannot hide a MODIFIED test"
       " file's removed assertion either -> rc 1",
       _e2e(lambda r: (_w(r, "tests/x.rs", _tests_blob), _w(r, ".git/info/attributes", "*.rs binary\n")),
            lambda r: (_w(r, "tests/x.rs", "#[test]\nfn t() {\n}\n"), _git(r, "add", "-A")))[0] == 1)
record("run_diff E2E: `diff.suppressBlankEmpty=true` in the repo config cannot compress the tag window (three blank lines between the run"
       " and an unrelated tag) -> rc 1",
       _e2e(lambda r: _w(r, "tests/x.rs", "#[test]\nfn t() {\n    assert_eq!(1, 1);\n}\nfn a() {}\n\nfn b() {}\n\nfn c() {}\n\nfn d() {}\n"
                                           "// [Test-Change: P4.33 — old-obsolete+new-correct, §6.4: tests/zz.rs]\n"),
            lambda r: (_w(r, "tests/x.rs", "#[test]\nfn t() {\n}\nfn a() {}\n\nfn b() {}\n\nfn c() {}\n\nfn d() {}\n"
                                           "// [Test-Change: P4.33 — old-obsolete+new-correct, §6.4: tests/zz.rs]\n"), _git(r, "add", "-A")),
            config=[("diff.suppressBlankEmpty", "true")])[0] == 1)


def _with_env(k, v, fn):
    saved = os.environ.get(k)
    os.environ[k] = v
    try:
        return fn()
    finally:
        if saved is None:
            os.environ.pop(k, None)
        else:
            os.environ[k] = saved


record("run_diff E2E: GIT_DIFF_OPTS=-u0 in the environment (git honours it OVER an explicit --unified) is scrubbed from the git reads, so a"
       " tag three context lines above its run still justifies the deletion -> rc 0",
       _with_env("GIT_DIFF_OPTS", "-u0", lambda: _e2e(
           lambda r: _w(r, "tests/x.rs", "// [Test-Change: P4.33 — old-obsolete+new-correct, §6.4: retired]\n// pad\n// pad\n#[test]\nfn t() {\n    assert_eq!(1, 1);\n}\n"),
           lambda r: (_w(r, "tests/x.rs", "// [Test-Change: P4.33 — old-obsolete+new-correct, §6.4: retired]\n// pad\n// pad\n#[test]\nfn t() {\n}\n"),
                      _git(r, "add", "-A")))[0]) == 0)
record("run_diff E2E: a staged note whose ONE line carries a CR-joined tag (`// harmless \\r// [Test-Change: … tests/x.rs]`) justifies a"
       " real `git rm tests/x.rs` under git's line model - the tag IS on the added line (under a text-mode pipe's universal newlines the"
       " tag's half lost its `+` and the verdict flipped; the CR byte is G52's to reject) -> rc 0",
       _e2e(lambda r: _w(r, "tests/x.rs", _tests_blob),
            lambda r: (_git(r, "rm", "-q", "tests/x.rs"), _w(r, "tests/note.rs", "// harmless \r" + _note("tests/x.rs")),
                       _git(r, "add", "-A")))[0] == 0)
record("run_diff E2E: a staged icon (a PNG-like blob, binary to git) beside a justified deletion is never rendered - the diff is scoped"
       " to the parsed paths -> rc 0",
       _e2e(lambda r: _w(r, "tests/x.rs", _tests_blob),
            lambda r: (_git(r, "rm", "-q", "tests/x.rs"), _w(r, "tests/note.rs", _note("tests/x.rs")),
                       (r / "icon.png").write_bytes(b"\x89PNG\r\n\x1a\n" + bytes(range(256)) * 8), _git(r, "add", "-A")))[0] == 0)
def _e2e_full(setup, config=()) -> tuple[int, str]:
    """The fail-closed --full mirror, WIRED: a throwaway repo, `setup(repo)` writes + commits its files, then a COPY of
    the script placed at <repo>/scripts/ (so its ROOT resolves to the repo) runs `--full` from the repo."""
    with tempfile.TemporaryDirectory() as td:
        repo = Path(td)
        _git(repo, "init", "-q", "-b", "main"); _git(repo, "config", "user.email", "t@t.t"); _git(repo, "config", "user.name", "t")
        for k, v in config:
            _git(repo, "config", k, v)
        setup(repo)
        _git(repo, "add", "-A"); _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "base")
        (repo / "scripts").mkdir(exist_ok=True)
        (repo / "scripts" / SCRIPT.name).write_bytes(SCRIPT.read_bytes())
        r = subprocess.run([sys.executable, str(repo / "scripts" / SCRIPT.name), "--full"], cwd=str(repo), capture_output=True,
                           text=True, encoding="utf-8", errors="replace", env=dict(os.environ, PYTHONDONTWRITEBYTECODE="1"))
        return r.returncode, (r.stdout or "") + (r.stderr or "")


record("run_full E2E: a non-ASCII-named test file (`tests/ü.rs`) carrying an untagged #[ignore] is found by the fail-closed --full mirror"
       " even with core.quotepath on - its name is listed `-z`, never quoted -> rc 1",
       _e2e_full(lambda r: _w(r, "tests/ü.rs", "#[test]\n#[ignore]\nfn t() {\n    assert_eq!(1, 1);\n}\n"),
                 config=[("core.quotepath", "true")])[0] == 1)
record("run_full E2E: the same file with its #[ignore] tagged -> rc 0 (the mirror scans it and finds it justified)",
       _e2e_full(lambda r: _w(r, "tests/ü.rs", "#[test]\n// [Test-Change: P4.33 — new-test:harness, §6.4]\n#[ignore]\nfn t() {\n    assert_eq!(1, 1);\n}\n"),
                 config=[("core.quotepath", "true")])[0] == 0)
record("run_diff E2E: HEAD's tree is listed `--full-tree`, so the shared-basename rule sees BOTH `mod.rs` even when the gate runs from a"
       " sub-directory holding only one of them (the round-16 finding: a CWD-relative `ls-tree` called the basename unique and a bare"
       " `mod.rs` tag justified the deletion) -> rc 1",
       _e2e(lambda r: (_w(r, "tests/a/mod.rs", _tests_blob), _w(r, "tests/b/mod.rs", _tests_blob)),
            lambda r: (_git(r, "rm", "-q", "tests/a/mod.rs"), _w(r, "tests/note.rs", _note("mod.rs")), _git(r, "add", "-A")),
            cwd_sub="tests/b")[0] == 1)
record("run_diff E2E: deleting tests/b.rs while adding an EMPTY src/b.rs (a chunk with no `---`/`+++`) still makes `b.rs` ambiguous -> rc 1",
       _e2e(lambda r: _w(r, "tests/b.rs", _tests_blob),
            lambda r: (_git(r, "rm", "-q", "tests/b.rs"), _w(r, "src/b.rs", ""),
                       _w(r, "tests/note.rs", _note("b.rs")), _git(r, "add", "-A")))[0] == 1)
record("run_diff E2E: deleting tests/b.rs while ADDING src/b.rs makes `b.rs` ambiguous - a bare-basename tag no longer justifies -> rc 1",
       _e2e(lambda r: _w(r, "tests/b.rs", _tests_blob),
            lambda r: (_git(r, "rm", "-q", "tests/b.rs"), _w(r, "src/b.rs", "pub fn b() {}\n"),
                       _w(r, "tests/note.rs", _note("b.rs")), _git(r, "add", "-A")))[0] == 1)

# fail-open without a diff base (a fresh repo with no HEAD)
with tempfile.TemporaryDirectory() as td:
    repo = Path(td)
    _git(repo, "init", "-q", "-b", "main")
    cwd = os.getcwd(); os.chdir(repo)
    try:
        rc_open = m.main(["--diff"])
    finally:
        os.chdir(cwd)
    record("run_diff: no diff base (fresh repo) -> fail-open rc 0", rc_open == 0)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-test-suppression] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
