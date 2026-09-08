#!/usr/bin/env python3
"""g24-deferral.py - G24 self-test for check-deferral (P0.3.4, G8/G21).

Proves the deferral/dead-marker scan: HARD markers flag anywhere; SOFT deferral phrasings flag ONLY
in comments (a legit `placeholder=` attribute is NOT flagged); `[Build-Session-Entscheidung]` within
±6 lines suppresses; a bare `[!extern]` suppresses ONLY in docs/plan/*.md and NEVER in production code
(the row-mandated negative test); and the production-file selector excludes docs/tooling/tests.
stdlib-only. Exit 0 = all held; 1 = a self-test failed.
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

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-deferral"
_loader = importlib.machinery.SourceFileLoader("cd", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cd", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def hits(text: str, rel: str = "src-tauri/src/x.rs") -> list:
    return m.scan_text(text, rel)


# --- HARD markers flag anywhere ---------------------------------------------------------------
for marker_line in ("let x = todo!();", "unimplemented!()", "unreachable!()", "dbg!(x);", 'println!("hi");',
                    "compile_error!(\"x\");", "console.log(x)", "const y: any = 1;", "y as any",
                    '<div style="x">', "{ style: {} }", "// TODO fix this", "// FIXME later"):
    record(f"HARD marker flagged: {marker_line!r}", len(hits(marker_line, "src/x.ts")) >= 1)

# --- SOFT phrasings flag ONLY in comments -----------------------------------------------------
record("SOFT 'comes in P5' in a COMMENT -> flagged", len(hits("// the rest comes in P5", "src/x.ts")) == 1)
record("SOFT 'for now'/'stub' in a comment -> flagged", len(hits("// just a stub for now", "src/x.ts")) >= 1)
record("SOFT 'placeholder' as a JSX ATTRIBUTE (not a comment) -> NOT flagged",
       len(hits('<input placeholder="Name" />', "src/x.tsx")) == 0)
record("SOFT 'later' in prose CODE (string, not comment) -> NOT flagged",
       len(hits('const msg = "do it later";', "src/x.ts")) == 0)
record("SOFT 'currently absent' in a comment -> flagged",
       len(hits("// this feature is currently absent", "src/x.ts")) == 1)

# --- block-comment STATE (P1 fix) + string-awareness (URL P2 fix) -----------------------------
record("SOFT in a bare /* */ body line (no leading *) -> flagged",
       len(hits("/*\n  real impl comes in P5\n*/", "src-tauri/src/x.rs")) >= 1)
record("SOFT on a block-comment CLOSER line -> flagged",
       len(hits("let x = 1; /* body\n  stuff for now */", "src/x.ts")) >= 1)
record("SOFT word after // inside a URL STRING -> NOT flagged (string-aware)",
       len(hits('let url = "http://example.com/later";', "src/x.ts")) == 0)
record("SOFT after // with an UNBALANCED apostrophe before it (/don't/) -> flagged (re-review P1 fix)",
       len(hits("const re = /don't/; // stub for now", "src/x.ts")) >= 1)
record("SOFT after // with an UNTERMINATED string before it -> flagged (re-review P1 fix)",
       len(hits('const s = "oops; // stub for now', "src/x.ts")) >= 1)
record("identifier-form deferral (compute_later/stub_handler) -> NOT flagged (HARD macros cover dead code)",
       len(hits("let v = compute_later(); fn stub_handler() {}", "src/x.ts")) == 0)

# --- suppression ------------------------------------------------------------------------------
SUP = "// TODO real work\n// [Build-Session-Entscheidung: P0.3.4]\n"
record("[Build-Session-Entscheidung] within ±6 lines -> suppressed", len(hits(SUP, "src/x.ts")) == 0)
FAR = "// TODO real work\n" + "\n" * 8 + "// [Build-Session-Entscheidung: P0.3.4]\n"
record("[Build-Session-Entscheidung] >6 lines away -> NOT suppressed", len(hits(FAR, "src/x.ts")) >= 1)

# --- [!extern] restriction (the row-mandated negative test) -----------------------------------
EXT = "// comes in P5 [!extern]\n"
record("[!extern] beside a deferral in a .rs -> STILL FAILS (not suppressed in production code)",
       len(hits(EXT, "src-tauri/src/x.rs")) >= 1)
record("[!extern] beside a deferral in docs/plan/*.md -> suppressed",
       len(m.scan_text("- a box comes in P5 [!extern]\n", "docs/plan/P0.md")) == 0)

# --- production-file selector -----------------------------------------------------------------
record("src/x.ts is production", m.is_production_file("src/x.ts"))
record("src-tauri/src/main.rs is production", m.is_production_file("src-tauri/src/main.rs"))
record("docs/x.md is NOT production", not m.is_production_file("docs/spec/00-architecture.md"))
record("scripts/check-deferral (tooling) is NOT production", not m.is_production_file("scripts/check-deferral"))
record("src/x.test.ts (test) is NOT production", not m.is_production_file("src/x.test.ts"))
record("src-tauri/tests/y.rs (test) is NOT production", not m.is_production_file("src-tauri/tests/y.rs"))
record("src-tauri/build.rs is production (spec §0.7)", m.is_production_file("src-tauri/build.rs"))
record("index.html is production (spec §0.7)", m.is_production_file("index.html"))
record("vite.config.ts is production", m.is_production_file("vite.config.ts"))
record("tsconfig.json is NOT production (pure config)", not m.is_production_file("tsconfig.json"))

# --- run_diff reads the STAGED blob, not the worktree (P2 fix) --------------------------------
def _git(repo, *a):
    subprocess.run(["git", "-C", str(repo), *a], capture_output=True, text=True, encoding="utf-8", errors="replace", check=True)


with tempfile.TemporaryDirectory() as td:
    repo = Path(td)
    _git(repo, "init", "-q", "-b", "main"); _git(repo, "config", "user.email", "t@t.t"); _git(repo, "config", "user.name", "t")
    (repo / "README").write_text("x\n", encoding="utf-8")
    _git(repo, "add", "README"); _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "init")
    (repo / "src").mkdir()
    src = repo / "src" / "x.ts"
    src.write_text("// real work comes in P5\nexport const x = 1;\n", encoding="utf-8")
    _git(repo, "add", "src/x.ts")
    src.write_text("// pad\n// pad2\n// real work comes in P5\nexport const x = 1;\n", encoding="utf-8")  # worktree diverges
    cwd = os.getcwd(); os.chdir(repo)
    try:
        rc = m.main(["--diff"])
    finally:
        os.chdir(cwd)
    record("run_diff flags a STAGED production marker despite worktree divergence (P2)", rc == 1)

# --- live --full passes on the real repo (production source clean) ----------------------------
record("--full passes on the real repo (production source carries no unsuppressed marker — LIVE since P1)", m.main(["--full"]) == 0)

# --- the `+++`-as-header class (2026-09-08): inside a hunk EVERY `+` line is an added line -------------
record("diff: an ADDED line whose content begins `++` (rendered `+++…`) is an added line, not a header; the real"
       " `+++ b/…` header before the first @@ is not a line",
       m.added_lines_of("--- a/src/a.ts\n+++ b/src/a.ts\n@@ -1,0 +1,2 @@\n+++i; // TODO: wire this up\n+x\n") == {1, 2}
       and m.added_lines_of("--- a/src/a.ts\n+++ b/src/a.ts\n@@ -3,1 +3,2 @@\n y\n+z\n") == {4})
record("diff: the `\\ No newline at end of file` marker is not a new-side line - the `+` line after it keeps its number",
       m.added_lines_of("--- a/src/a.ts\n+++ b/src/a.ts\n@@ -5 +5 @@\n-old\n\\ No newline at end of file\n+new\n\\ No newline at end of file\n") == {5})


# --- the diff-SYNTAX class (2026-09-08, the round-12 G1 finding on the sibling gates): the staged file list is read `-z`
# (never quoted, so a non-ASCII NEW production file is in scope whatever core.quotepath says), and every diff read carries the
# syntax pins (`a/`/`b/` prefixes forced, no external driver, no textconv, non-ASCII literal) -----------------------------------
def _e2e_deferral(stage, config=(), attributes=None, base_files=None, cwd_sub=None) -> int:
    """A throwaway repo with the given config; `stage(repo)` stages the change; then the script's own --diff."""
    with tempfile.TemporaryDirectory() as td:
        repo = Path(td)
        _git(repo, "init", "-q", "-b", "main"); _git(repo, "config", "user.email", "t@t.t"); _git(repo, "config", "user.name", "t")
        for k, v in config:
            _git(repo, "config", k, v)
        if attributes is not None:
            (repo / ".git" / "info").mkdir(exist_ok=True)
            (repo / ".git" / "info" / "attributes").write_text(attributes, encoding="utf-8")
        (repo / "README").write_text("x\n", encoding="utf-8")
        for rel, text in (base_files or {}).items():
            (repo / rel).parent.mkdir(parents=True, exist_ok=True)
            (repo / rel).write_bytes(text.encode("utf-8"))
        _git(repo, "add", "-A"); _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "init")
        stage(repo)
        cwd = os.getcwd(); os.chdir(repo / cwd_sub if cwd_sub else repo)
        buf = io.StringIO()
        try:
            with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(buf):
                rc = m.main(["--diff"])
        finally:
            os.chdir(cwd)
        return rc


def _stage_new(rel, text):
    def stage(repo):
        p = repo / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(text.encode("utf-8"))
        _git(repo, "add", "-A")
    return stage


record("run_diff E2E: a NEW production file with a non-ASCII name is read `-z` (never quoted) - its TODO is caught even with"
       " core.quotepath on (the round-12 finding: the octal-quoted name failed the production-file test and the marker passed L1) -> rc 1",
       _e2e_deferral(_stage_new("src-tauri/src/über.rs", "// TODO: finish this\npub fn f() {}\n"), config=[("core.quotepath", "true")]) == 1)
record("run_diff E2E: a `diff.external` driver a user set (one that cannot run) does not blank the added-line map -> rc 1",
       _e2e_deferral(_stage_new("src-tauri/src/x.rs", "// TODO: finish this\npub fn f() {}\n"),
                     config=[("diff.external", "g24-no-such-diff-driver")]) == 1)
record("run_diff E2E: a user attribute marking `.rs` binary cannot hide a new production file's TODO behind `Binary files differ` - the"
       " per-path diff is read `--text` (the round-13 P0) -> rc 1",
       _e2e_deferral(_stage_new("src-tauri/src/x.rs", "// TODO: finish this\npub fn f() {}\n"), attributes="*.rs binary\n") == 1)
record("line model: a lone CR inside a `+` line is ONE added line (git's `\\n` model; under `str.splitlines()` the added-line map and"
       " the blob's line numbers desynced, the round-14 finding)",
       m._lines("a\rb\n") == ["a\rb"]
       and m.added_lines_of("--- a/src/a.ts\n+++ b/src/a.ts\n@@ -1,0 +1,2 @@\n+// a \r// b\n+x\n") == {1, 2})
record("run_diff E2E: a committed production line carrying a lone CR (`// a \\r// b`) does not shift the numbering of a TODO added"
       " below it - the staged blob and the diff share git's line model -> rc 1",
       _e2e_deferral(_stage_new("src-tauri/src/x.rs", "// a \r// b\n// TODO: finish this\n"),
                     base_files={"src-tauri/src/x.rs": "// a \r// b\n"}) == 1)
record("source pin: the git reads return BYTES decoded as UTF-8 (no universal-newline translation); no `splitlines()` survives",
       'subprocess.run(["git", *args], env=_git_env(), capture_output=True)\n    return p.returncode, _decode(p.stdout)' in SCRIPT.read_text(encoding="utf-8")
       and SCRIPT.read_text(encoding="utf-8").replace("str.splitlines()", "").count("splitlines()") == 0)
record("source pin: the staged file list is read `--name-only -z`, the per-path diff carries the diff-SYNTAX pins (incl. `--text`), and the"
       " env knob that outranks the command line (GIT_DIFF_OPTS) is scrubbed",
       m.GIT_DIFF_SYNTAX == ("-c", "core.quotepath=false", "-c", "diff.noprefix=false", "-c", "diff.mnemonicPrefix=false",
                             "-c", "diff.srcPrefix=a/", "-c", "diff.dstPrefix=b/", "-c", "diff.suppressBlankEmpty=false",
                             "-c", "diff.relative=false", "-c", "diff.interHunkContext=0")
       and m.GIT_DIFF_FLAGS == ("--no-ext-diff", "--no-textconv", "--text", "--no-color")
       and m.SCRUBBED_GIT_ENV == ("GIT_DIFF_OPTS", "GIT_EXTERNAL_DIFF")
       and 'subprocess.run(["git", *args], env=_git_env(),' in SCRIPT.read_text(encoding="utf-8")
       and '"--name-only", "-z", "--diff-filter=ACMR"' in SCRIPT.read_text(encoding="utf-8")
       and 'diff_args = [*GIT_DIFF_SYNTAX, "diff", "--unified=0", *GIT_DIFF_FLAGS]' in SCRIPT.read_text(encoding="utf-8"))



def _e2e_full_deferral(setup, config=()) -> int:
    """The fail-closed --full mirror, WIRED: a throwaway repo with the given committed files, then a COPY of the script at
    <repo>/scripts/ (so its ROOT resolves to the repo) runs `--full` from the repo."""
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
        return r.returncode


def _write(rel, text):
    def setup(repo):
        p = repo / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(text.encode("utf-8"))
    return setup


record("run_full E2E: a non-ASCII-named production file (`src-tauri/src/ü.rs`) carrying a TODO is found by the fail-closed --full mirror"
       " even with core.quotepath on - its name is listed `-z`, never quoted (the round-15 finding) -> rc 1",
       _e2e_full_deferral(_write("src-tauri/src/ü.rs", "// TODO: finish this\npub fn f() {}\n"), config=[("core.quotepath", "true")]) == 1)
record("run_full E2E: the same file without a marker -> rc 0 (the mirror scans it)",
       _e2e_full_deferral(_write("src-tauri/src/ü.rs", "pub fn f() {}\n"), config=[("core.quotepath", "true")]) == 0)
record("run_diff E2E: the staged file list carries the syntax pins - `diff.relative=true` with the gate run from a sub-directory"
       " (`src-tauri/`) would list CWD-relative names that miss the production test and the blob read -> rc 1",
       _e2e_deferral(_stage_new("src-tauri/src/x.rs", "// TODO: finish this\npub fn f() {}\n"),
                     config=[("diff.relative", "true")], cwd_sub="src-tauri") == 1)
record("source pin: the tracked list is `ls-files -z` and the staged list carries the syntax pins",
       'git("ls-files", "-z", "--full-name", "--", ":/")' in SCRIPT.read_text(encoding="utf-8") and 'git("ls-files")' not in SCRIPT.read_text(encoding="utf-8")
       and 'diff_args += ["--", f":(top,literal){relpath}"]' in SCRIPT.read_text(encoding="utf-8")
       and 'git(*GIT_DIFF_SYNTAX, "diff", "--cached", "--name-only", "-z", "--diff-filter=ACMR")' in SCRIPT.read_text(encoding="utf-8"))

record("run_diff E2E: a tracked path carrying a glob character (`src-tauri/src/a[1].rs`) is diffed by its LITERAL name (`:(top,literal)`),"
       " so a TODO added to it is caught -> rc 1 (a bare `:/` pathspec reads the name as a wildmatch pattern, git-version-dependent; the source pin below is the mutant's catcher)",
       _e2e_deferral(_stage_new("src-tauri/src/a[1].rs", "// TODO: finish this\npub fn f() {}\n")) == 1)
record("source pin: the per-path diff uses the `:(top,literal)` pathspec magic (repo-root-anchored AND literal)",
       'diff_args += ["--", f":(top,literal){relpath}"]' in SCRIPT.read_text(encoding="utf-8"))

failed = [n for n, ok in results if not ok]
print(f"\n[g24-deferral] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
