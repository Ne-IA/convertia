#!/usr/bin/env python3
"""g24-commit-msg.py - G24 self-test for check-commit-msg (P0.3.3, G11).

Proves the conventional-commit subject gate ACCEPTS every valid type/scope/rollback/auto subject and
REJECTS non-conventional ones. stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import sys
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

failed = [n for n, k in results if not k]
print(f"\n[g24-commit-msg] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
