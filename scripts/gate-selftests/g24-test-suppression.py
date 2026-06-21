#!/usr/bin/env python3
"""g24-test-suppression.py - G24 self-test for check-test-suppression (P0.3.14, G70).

Proves the "no green-by-rewrite" canary: a suppression MARKER (#[ignore]/it.skip/.only/#[should_panic])
or a removed/commented-out assertion in a TEST file FLAGS unless a [Test-Change] tag sits within ±6
lines; the #[cfg(test)] scoping (a marker in a src .rs counts ONLY inside its test block); both tag
shapes (old-obsolete+new-correct, new-test:<reason>) suppress; a clean test diff passes; and the
contract's enumerated cases (plant a marker/removed-assertion WITHOUT a tag ⇒ fail; WITH ⇒ pass).
Drives the pure fns + a real temp-git-repo E2E for --diff. stdlib-only. Exit 0 = all held; 1 = failed.
"""
import importlib.machinery
import importlib.util
import os
import subprocess
import sys
import tempfile
from pathlib import Path

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

# --- the live --full passes today (only the clean g53-fixture .rs files exist) ----------------
record("--full passes on the real repo today (no unjustified markers)", m.main(["--full"]) == 0)


# --- run_diff E2E in a real temp git repo (the staged-blob path + fail-open-without-base) ------
def _git(repo, *a):
    subprocess.run(["git", "-C", str(repo), *a], capture_output=True, text=True, encoding="utf-8", errors="replace", check=True)


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
