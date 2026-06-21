#!/usr/bin/env python3
"""g24-deferral.py - G24 self-test for check-deferral (P0.3.4, G8/G21).

Proves the deferral/dead-marker scan: HARD markers flag anywhere; SOFT deferral phrasings flag ONLY
in comments (a legit `placeholder=` attribute is NOT flagged); `[Build-Session-Entscheidung]` within
±6 lines suppresses; a bare `[!extern]` suppresses ONLY in docs/plan/*.md and NEVER in production code
(the row-mandated negative test); and the production-file selector excludes docs/tooling/tests.
stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import os
import subprocess
import sys
import tempfile
from pathlib import Path

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

# --- live --full passes today (no production source yet) --------------------------------------
record("--full passes today (no production files — target-absent)", m.main(["--full"]) == 0)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-deferral] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
