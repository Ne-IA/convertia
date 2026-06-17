#!/usr/bin/env python3
"""g24-dual-review.py - G24 self-test for check-dual-review (P0.3.3, G12).

Proves the Dual-Review-trailer gate: a well-formed trailer + narrative passes; a missing/ill-formed
trailer fails; a GO/GO trailer with NO review narrative fails; and a docs-`.md`-only `chore(todo): …
(abgehakt|done)` check-off is exempt (while the same subject with a non-.md file is NOT). stdlib-only.
Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import os
import subprocess
import sys
import tempfile
from pathlib import Path

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
SUBJECT = "feat(gates): a real box"

# a well-formed GO/GO trailer WITH a narrative -> OK
record("GO/GO + narrative -> OK", m.evaluate_commit(SUBJECT, GOOD_BODY, ["scripts/x"]) is None)

# missing trailer -> error
record("missing trailer -> error",
       m.evaluate_commit(SUBJECT, "feat(gates): x\n\nno trailer here\n", ["scripts/x"]) is not None)

# ill-formed trailer -> error
record("ill-formed trailer (opus=YES) -> error",
       m.evaluate_commit(SUBJECT, "x\n\nDual-Review: opus=YES sonnet=GO\n", ["scripts/x"]) is not None)

# GO/GO but NO narrative (bare trailer) -> error
record("GO/GO but bare body (no narrative) -> error",
       m.evaluate_commit(SUBJECT, "feat(gates): x\n\nDual-Review: opus=GO sonnet=GO\nCo-Authored-By: y\n",
                         ["scripts/x"]) is not None)

# GO/GO, a box-id in the SUBJECT but a bare body -> error (the subject's (P0.3.3) must NOT satisfy the
# P[0-3] marker — the findings-block scans the BODY only; this is the P1 regression guard)
record("GO/GO, box-id in subject but bare body -> error",
       m.evaluate_commit("feat(gates): commit-hygiene (P0.3.3)",
                         "feat(gates): commit-hygiene (P0.3.3)\n\nDual-Review: opus=GO sonnet=GO\nCo-Authored-By: y\n",
                         ["scripts/x"]) is not None)

# NOGO/NOGO well-formed trailer (no findings-block required) -> OK
record("well-formed NOGO trailer -> OK (no narrative requirement)",
       m.evaluate_commit(SUBJECT, "x\n\nDual-Review: opus=NOGO sonnet=NOGO\n", ["scripts/x"]) is None)

# check-off commit (chore(todo) + .md-only) -> EXEMPT even with no trailer
record("check-off (chore(todo)+md-only) -> exempt, no trailer needed",
       m.evaluate_commit("chore(todo): P0.3.3 abgehakt", "chore(todo): P0.3.3 abgehakt\n",
                         ["docs/plan/P0.md"]) is None)

# same check-off subject but a NON-.md file -> NOT exempt -> needs trailer -> error
record("check-off subject but a .rs file -> NOT exempt -> error",
       m.evaluate_commit("chore(todo): P0.3.3 abgehakt", "chore(todo): P0.3.3 abgehakt\n",
                         ["src/x.rs"]) is not None)

# check-off subject but EMPTY file list -> NOT exempt (no files = not a docs-only tick)
record("check-off subject + empty file list -> NOT exempt",
       m.evaluate_commit("chore(todo): abgehakt", "chore(todo): abgehakt\n", []) is not None)

# has_findings_block: subject + only-trailers body -> False (the subject is excluded)
record("has_findings_block: subject + trailers-only body -> False",
       not m.has_findings_block("feat: x (P0.3.3)\n\nDual-Review: opus=GO sonnet=GO\nCo-Authored-By: z\n"))
record("has_findings_block: marker in the BODY (not the subject) -> True",
       m.has_findings_block("feat: x\n\nfixed a P1 issue in review\nDual-Review: opus=GO sonnet=GO\n"))

# --- commit_shas range resolution (L4 --base) in real temp repos -----------------------------
def git(repo, *a):
    subprocess.run(["git", "-C", str(repo), *a], capture_output=True, text=True, check=True)


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

failed = [n for n, ok in results if not ok]
print(f"\n[g24-dual-review] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
