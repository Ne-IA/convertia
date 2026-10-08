#!/usr/bin/env python3
"""g24-dual-review.py - G24 self-test for check-dual-review (P0.3.3, G12).

Proves the Dual-Review-trailer gate: a well-formed trailer + narrative passes; a missing/ill-formed
trailer fails; a GO/GO trailer with NO review narrative fails; and no commit is exempt — the retired
check-off shape (`chore(todo): … (abgehakt|done)`, docs-`.md`-only) without a trailer fails, both in
`evaluate_commit` and through the real CLI over a range in a temp repo. stdlib-only.
Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import os
import subprocess
import sys
import tempfile
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")
# A git hook exports GIT_* variables (an absolute GIT_DIR in a linked worktree): under them, a git run from this
# file in a temp directory - its own, a gate's or a tool's - acts on the hooked repository. All but GIT_EXEC_PATH go.
for _k in [k for k in os.environ if k.startswith("GIT_") and k != "GIT_EXEC_PATH"]:
    os.environ.pop(_k)

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-dual-review"
_loader = importlib.machinery.SourceFileLoader("cdr", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cdr", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


GOOD_BODY = ("feat(gates): a real box\n\nG1 review found 2 P2 findings, fixed.\n\n"
             "Dual-Review: opus=GO sonnet=GO\nL-neg1-ack: owner\n")

# a well-formed GO/GO trailer WITH a narrative -> OK
record("GO/GO + narrative -> OK", m.evaluate_commit(GOOD_BODY) is None)

# missing trailer -> error
record("missing trailer -> error", m.evaluate_commit("feat(gates): x\n\nno trailer here\n") is not None)

# ill-formed trailer -> error
record("ill-formed trailer (opus=YES) -> error",
       m.evaluate_commit("x\n\nDual-Review: opus=YES sonnet=GO\n") is not None)

# GO/GO but NO narrative (bare trailer) -> error
record("GO/GO but bare body (no narrative) -> error",
       m.evaluate_commit("feat(gates): x\n\nDual-Review: opus=GO sonnet=GO\nCo-Authored-By: y\n") is not None)

# GO/GO, a box-id in the SUBJECT but a bare body -> error (the subject's (P0.3.3) must NOT satisfy the
# P[0-3] marker — the findings-block scans the BODY only; this is the P1 regression guard)
record("GO/GO, box-id in subject but bare body -> error",
       m.evaluate_commit("feat(gates): commit-hygiene (P0.3.3)\n\nDual-Review: opus=GO sonnet=GO\n"
                         "Co-Authored-By: y\n") is not None)

# NOGO/NOGO well-formed trailer (no findings-block required) -> OK
record("well-formed NOGO trailer -> OK (no narrative requirement)",
       m.evaluate_commit("x\n\nDual-Review: opus=NOGO sonnet=NOGO\n") is None)

# [Test-Change: P0.3.3 — old-obsolete+new-correct, build-gates G12 row] old: a docs-`.md`-only
# `chore(todo): … (abgehakt|done)` commit was exempt (a `.rs` file or an empty file list was not);
# obsolete: the check-off rides in the box commit (build-loop.md Step 7), so the exemption and its
# double predicate retire and `evaluate_commit` takes the message alone (the legs above only drop the
# unused subject/file arguments). New: the old skip shape without a trailer is an ordinary commit ->
# error, under both keywords the retired subject regex accepted, and through the real CLI below.
record("check-off shape `chore(todo): … abgehakt` without a trailer -> error (no check-off exemption)",
       m.evaluate_commit("chore(todo): P0.3.3 abgehakt\n") is not None)
record("check-off shape `chore(todo): … done` without a trailer -> error (no check-off exemption)",
       m.evaluate_commit("chore(todo): P0.3.3 done\n") is not None)

# has_findings_block: subject + only-trailers body -> False (the subject is excluded)
record("has_findings_block: subject + trailers-only body -> False",
       not m.has_findings_block("feat: x (P0.3.3)\n\nDual-Review: opus=GO sonnet=GO\nCo-Authored-By: z\n"))
record("has_findings_block: marker in the BODY (not the subject) -> True",
       m.has_findings_block("feat: x\n\nfixed a P1 issue in review\nDual-Review: opus=GO sonnet=GO\n"))

# --- commit_shas range resolution (L4 --base) + the CLI over a range, in real temp repos -----------


def git(repo, *a):
    return subprocess.run(["git", "-C", str(repo), *a], capture_output=True, text=True, encoding="utf-8",
                          errors="replace", check=True).stdout.strip()


def in_repo(repo, fn):
    cwd = os.getcwd(); os.chdir(repo)
    try:
        return fn()
    finally:
        os.chdir(cwd)


with tempfile.TemporaryDirectory() as td:
    repo = Path(td)
    git(repo, "init", "-q", "-b", "main"); git(repo, "config", "user.email", "t@t.t"); git(repo, "config", "user.name", "t")
    (repo / "a").write_text("1\n", encoding="utf-8"); git(repo, "add", "-A"); git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "feat: one")
    (repo / "a").write_text("2\n", encoding="utf-8"); git(repo, "add", "-A"); git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "feat: two")
    # an ABSENT well-formed 40-hex base must route to tip-only (NOT crash rc=128) — the P2 ^{commit} fix
    rc, shas, rng = in_repo(repo, lambda: m.commit_shas("deadbeef" * 5))
    record("commit_shas(absent 40-hex base) -> tip-only, rc 0 (no rev-list crash)", rc == 0 and len(shas) == 1)
    # no upstream / no base -> tip-only
    rc, shas, rng = in_repo(repo, lambda: m.commit_shas(None))
    record("commit_shas(None, no upstream) -> tip-only", rc == 0 and len(shas) == 1)
    # the retired exemption end to end (the P0.3.3 Test-Change above): a docs-`.md`-only `chore(todo): …
    # abgehakt` commit without a trailer, read by the real CLI over `--base <parent>..HEAD` (the L4 mirror's
    # form) -> exit 1, naming that commit
    base = git(repo, "rev-parse", "HEAD")
    (repo / "docs").mkdir(); (repo / "docs" / "plan.md").write_text("- [x] box\n", encoding="utf-8")
    git(repo, "add", "-A"); git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "chore(todo): box abgehakt")
    cli = subprocess.run([sys.executable, "-P", str(SCRIPT), "--base", base], cwd=str(repo), capture_output=True,
                         text=True, encoding="utf-8", errors="replace")
    record("CLI: a docs-only `chore(todo): … abgehakt` commit without a trailer -> exit 1 (no exemption)",
           cli.returncode == 1 and "missing a well-formed" in cli.stderr and "chore(todo): box abgehakt" in cli.stderr)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-dual-review] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
