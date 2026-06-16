#!/usr/bin/env python3
"""test-docs-only-fastpath-pattern.py - G10 fastpath smoke test for scripts/fastpath-docs-only.

The `test-*-fastpath-pattern` G10 self-test (build-gates G10): a POSITIVE + NEGATIVE proof that the
docs-only skip detector classifies a range correctly and DEFAULTS TO MUST-RUN on ambiguity. Drives
the pure is_skip_eligible / is_docs_only classifiers with fixture path lists (no git needed); the
live range path defaults-to-must-run when origin/main is unknown (the conservative direction).
stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import sys
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "fastpath-docs-only"
_loader = importlib.machinery.SourceFileLoader("fdo", str(SCRIPT))
_spec = importlib.util.spec_from_loader("fdo", _loader)
m = importlib.util.module_from_spec(_spec)
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


# --- is_skip_eligible: single-path POSITIVES (docs-only) --------------------------------------
# Any top-level *.md is documentation (README.md, CHANGELOG.md, a lowercase readme.md), as is
# anything under docs/ and the LICENSE/NOTICE files.
for p in ("docs/spec/00-architecture.md", "docs/SINGLE-SOURCE-OF-TRUTH.md", "README.md",
          "CHANGELOG.md", "readme.md", "LICENSE", "NOTICE", "docs/security/build-gates.md"):
    record(f"skip-eligible POSITIVE: {p}", m.is_skip_eligible(p))

# --- is_skip_eligible: single-path NEGATIVES (must-run) ---------------------------------------
# CRITICAL (build-gates §4 `.md`-only invariant): a NON-`.md` file even UNDER docs/ is NOT skip-
# eligible - a docs/evil.rs / docs/x.json must still force the heavy gates (a wrong skip = a hole).
for p in ("src/main.rs", "src-tauri/Cargo.toml", "scripts/check-branch-protection",
          ".github/workflows/ci.yml", "Cargo.toml", "Cargo.lock", "package.json",
          "lefthook.yml", "src/ui.ts", "docs-extra/x.md",  # "docs-extra/" must NOT match "docs/"
          "docs/evil.rs", "docs/build.py", "docs/x.json",  # §276: non-.md UNDER docs/ -> must-run
          "docs/Cargo.toml", "docs/sub/code.ts", "docs/.gitleaks.toml",
          "docs/../src/main.rs", "a/../docs/x.md",          # any `..` segment -> deny (defense-in-depth)
          "license.txt", "notes.txt",                       # top-level non-.md (NOT skip-eligible)
          "docs", "docs/", ""):                              # the bare dir / trailing-slash / empty
    record(f"must-run NEGATIVE: {p!r}", not m.is_skip_eligible(p))

# --- is_docs_only: range-level -----------------------------------------------------------------
record("docs-only range (all docs) -> True",
       m.is_docs_only(["docs/a.md", "README.md", "LICENSE"]))
record("MIXED range (docs + code) -> False (one code file forces must-run)",
       not m.is_docs_only(["docs/a.md", "src/main.rs"]))
record("MIXED range (docs/*.md + docs/*.rs UNDER docs/) -> False (§276: non-.md under docs/ forces run)",
       not m.is_docs_only(["docs/a.md", "docs/evil.rs"]))
record("code-only range -> False", not m.is_docs_only(["scripts/check-x", "Cargo.toml"]))
record("EMPTY range -> False (ambiguous, never silently skip)", not m.is_docs_only([]))
record("range of blank strings -> False", not m.is_docs_only(["", "  "]))
record("single docs file -> True", m.is_docs_only(["docs/plan/P0-build-and-security.md"]))

# --- live main(): the conservative must-run paths (exit 1) ------------------------------------
record("main: unresolvable base -> MUST-RUN (exit 1)",
       m.main(["--base", "refs/heads/__no_such_base__", "--head", "HEAD"]) == 1)
record("main: EMPTY range (HEAD..HEAD, 0 changed files) -> MUST-RUN (exit 1)",
       m.main(["--base", "HEAD", "--head", "HEAD"]) == 1)

failed = [n for n, ok in results if not ok]
print(f"\n[test-docs-only-fastpath-pattern] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
