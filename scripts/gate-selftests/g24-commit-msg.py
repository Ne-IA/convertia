#!/usr/bin/env python3
"""g24-commit-msg.py - G24 self-test for check-commit-msg (P0.3.3, G11).

Proves the conventional-commit subject gate ACCEPTS every valid type/scope/rollback/auto subject and
REJECTS non-conventional ones. stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import os
import subprocess
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-commit-msg"
_loader = importlib.machinery.SourceFileLoader("ccm", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("ccm", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def ok(subject: str) -> bool:
    return m.validate_subject(subject) is None


# --- accepted -------------------------------------------------------------------------------
for good in ("feat(gates): add the G11 gate", "fix: correct the off-by-one", "chore(todo): P0.3.3 abgehakt",
             "docs(spec): sync §0.10", "refactor: extract helper", "test: add a case", "perf: cache it",
             "ci: pin the action", "build: bump node", "chore(scope): roll back — bad deploy",
             "fix(my-scope.v2): hyphen+dot scope", "Merge branch 'main'", 'Revert "feat: x"', "fixup! feat: x",
             "squash! feat: x", "amend! fix: y"):
    record(f"accept {good!r}", ok(good))

# --- rejected -------------------------------------------------------------------------------
for bad in ("random subject line", "feat add the thing", "feature(x): wrong type", "feat(): empty scope",
            "Feat(gates): capitalised type", "wip: not a type", ": no type", "feat(gates):no-space",
            "feat(gates)!: no breaking-! in the spec regex", "feat(Gates): uppercase scope char",
            "feat:    "):
    record(f"reject {bad!r}", not ok(bad))

record("empty message -> error", m.validate_subject(None) is not None)

# --- subject extraction skips comment / blank lines -----------------------------------------
record("subject_of skips # comments + blanks",
       m.subject_of("\n# a comment\n\nfeat(gates): real subject\n") == "feat(gates): real subject")

# --- L4 range-mirror: validate_messages (pure) + the file/range dispatch ---------------------
record("range: all-valid subjects -> no violations",
       m.validate_messages(["feat(x): a\n\nbody", "fix: b", "Merge branch 'main'"]) == [])
record("range: flags exactly the bad subjects at the right indices",
       [i for i, _s, _r in m.validate_messages(
           ["feat(x): ok", "bad subject line", "chore: ok", "feature(y): wrong"])] == [1, 3])
record("range: an empty/comment-only message in range is flagged",
       any(s is None for _i, s, _r in m.validate_messages(["\n# only a comment\n"])))
record("range: auto-subjects (revert/fixup) accepted in range",
       m.validate_messages(['Revert "feat: x"', "fixup! fix: y"]) == [])
record("dispatch: no file and no --base -> bad invocation (exit 2)", m.main([]) == 2)
record("dispatch: both a file and --base -> bad invocation (exit 2)",
       m.main(["msg.txt", "--base", "HEAD~1"]) == 2)


# --- L4 range resolution (check_range) in a real temp repo — the all-zeros / absent-base
#     degrade path is the security-critical leg (a fail-closed mirror must NOT exit-2-redden
#     main on an initial-push / force-push); mirrors g24-dual-review.py's commit_shas legs. ---
def _git(repo: Path, *a: str) -> None:
    subprocess.run(["git", "-C", str(repo), *a], capture_output=True, text=True, check=True)


def _git_out(repo: Path, *a: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *a], capture_output=True, text=True, check=True).stdout.strip()


def _in_repo(repo: Path, fn):
    cwd = os.getcwd()
    os.chdir(repo)
    try:
        return fn()
    finally:
        os.chdir(cwd)


with tempfile.TemporaryDirectory() as td:
    repo = Path(td)
    _git(repo, "init", "-q", "-b", "main")
    _git(repo, "config", "user.email", "t@t.t")
    _git(repo, "config", "user.name", "t")
    (repo / "a").write_text("1\n", encoding="utf-8"); _git(repo, "add", "-A"); _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "feat: one")
    c1 = _git_out(repo, "rev-parse", "HEAD")
    (repo / "a").write_text("2\n", encoding="utf-8"); _git(repo, "add", "-A"); _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "feat: two")
    c2 = _git_out(repo, "rev-parse", "HEAD")
    record("range: all-zeros base -> tip-only degrade -> exit 0 (no red-CI on a new-ref push)",
           _in_repo(repo, lambda: m.check_range("0" * 40, "HEAD")) == 0)
    record("range: absent 40-hex base -> tip-only degrade -> exit 0 (no rev-list crash on a force-push)",
           _in_repo(repo, lambda: m.check_range("deadbeef" * 5, "HEAD")) == 0)
    record("range: real base..HEAD all-conventional -> exit 0",
           _in_repo(repo, lambda: m.check_range(c1, "HEAD")) == 0)
    (repo / "a").write_text("3\n", encoding="utf-8"); _git(repo, "add", "-A"); _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "broken subject")
    record("range: a non-conventional subject in range -> caught -> exit 1",
           _in_repo(repo, lambda: m.check_range(c2, "HEAD")) == 1)

failed = [n for n, k in results if not k]
print(f"\n[g24-commit-msg] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
