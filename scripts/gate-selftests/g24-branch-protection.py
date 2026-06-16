#!/usr/bin/env python3
"""g24-branch-protection.py - G24 self-test for check-branch-protection (P0.2.8, G56a).

Drives the PURE evaluate_*() functions with fixture JSON (the live `gh api` calls are owner/CI-
verified - G56a is exempt from the plan-lint check-16 fixture requirement, an API-introspection
gate). Proves: the branch-protection core (required checks subset, enforce_admins, force-push/
deletion, required_signatures target-gating), the repo-security (d), workflow-perms (e) and the
T2-taint OR-gate (f) all classify correctly, and main() handles a missing repo per posture.
stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-branch-protection"
_loader = importlib.machinery.SourceFileLoader("cbp", str(SCRIPT))
_spec = importlib.util.spec_from_loader("cbp", _loader)
m = importlib.util.module_from_spec(_spec)
_loader.exec_module(m)

EXPECTED = m.EXPECTED_REQUIRED_CHECKS
results: list[tuple[str, bool]] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}{(' - ' + detail) if detail else ''}")


def bp(*, checks=None, contexts=None, admins=True, force=False, deletions=False, sig=False) -> dict:
    rsc: dict = {}
    if checks is not None:
        rsc["checks"] = [{"context": c} for c in checks]
    if contexts is not None:
        rsc["contexts"] = list(contexts)
    return {
        "required_status_checks": rsc,
        "enforce_admins": {"enabled": admins},
        "allow_force_pushes": {"enabled": force},
        "allow_deletions": {"enabled": deletions},
        "required_signatures": {"enabled": sig},
    }


# --- evaluate_branch_protection ---------------------------------------------------------------
h, s = m.evaluate_branch_protection(bp(checks=EXPECTED), EXPECTED, False)
record("BP valid (all checks, no admin-bypass, no force/del) -> no hard", not h)
record("BP valid + sig-not-required -> (g) soft note present", any("required_signatures" in x for x in s))

h, _ = m.evaluate_branch_protection(bp(checks=EXPECTED[1:]), EXPECTED, False)
record("BP missing a required check -> hard", any("present-and-required" in x for x in h))

h, _ = m.evaluate_branch_protection(bp(checks=EXPECTED, admins=False), EXPECTED, False)
record("BP enforce_admins false -> hard", any("admin-bypass" in x for x in h))

h, _ = m.evaluate_branch_protection(bp(checks=EXPECTED, force=True), EXPECTED, False)
record("BP allow_force_pushes true -> hard", any("force_pushes" in x for x in h))

h, _ = m.evaluate_branch_protection(bp(checks=EXPECTED, deletions=True), EXPECTED, False)
record("BP allow_deletions true -> hard", any("deletions" in x for x in h))

h, _ = m.evaluate_branch_protection(bp(checks=EXPECTED, sig=False), EXPECTED, True)
record("BP required_signatures off WHILE allowed-signers exists -> hard", any("required_signatures" in x for x in h))

h, _ = m.evaluate_branch_protection(bp(checks=EXPECTED, sig=True), EXPECTED, True)
record("BP required_signatures on + required -> no hard", not h)

h, _ = m.evaluate_branch_protection(bp(contexts=EXPECTED), EXPECTED, False)
record("BP legacy contexts[] shape recognized -> no hard", not h)

# --- evaluate_repo_security (d) ---------------------------------------------------------------
ok = {"security_and_analysis": {"secret_scanning": {"status": "enabled"},
                                "secret_scanning_push_protection": {"status": "enabled"}}}
record("(d) both enabled -> no hard", not m.evaluate_repo_security(ok))
record("(d) secret-scanning disabled -> hard", any("secret-scanning is not" in x for x in m.evaluate_repo_security(
    {"security_and_analysis": {"secret_scanning": {"status": "disabled"},
                               "secret_scanning_push_protection": {"status": "enabled"}}})))
record("(d) push-protection disabled -> hard", any("push-protection" in x for x in m.evaluate_repo_security(
    {"security_and_analysis": {"secret_scanning": {"status": "enabled"},
                               "secret_scanning_push_protection": {"status": "disabled"}}})))
record("(d) no security_and_analysis at all -> 2 hard", len(m.evaluate_repo_security({})) == 2)

# --- evaluate_workflow_perms (e) --------------------------------------------------------------
record("(e) read + can_approve false -> no hard",
       not m.evaluate_workflow_perms({"default_workflow_permissions": "read",
                                      "can_approve_pull_request_reviews": False}))
record("(e) write -> hard", any("must be 'read'" in x for x in m.evaluate_workflow_perms(
    {"default_workflow_permissions": "write", "can_approve_pull_request_reviews": False})))
record("(e) can_approve true -> hard", any("can_approve" in x for x in m.evaluate_workflow_perms(
    {"default_workflow_permissions": "read", "can_approve_pull_request_reviews": True})))

# --- taint_or_gate (f), returns (hard, soft) --------------------------------------------------
record("(f) CodeQL configured -> ok", m.taint_or_gate("ok", "configured", False, False) == (None, None))
record("(f) Semgrep ruleset present -> ok", m.taint_or_gate("notfound", None, True, False) == (None, None))
record("(f) CodeQL+Semgrep both -> ok", m.taint_or_gate("ok", "configured", True, False) == (None, None))
_r = m.taint_or_gate("notfound", None, False, False)
record("(f) CodeQL 404 (not configured) + no semgrep -> target-absent soft", _r[0] is None and _r[1] is not None)
_r = m.taint_or_gate("error", None, False, False)
record("(f) CodeQL READ-ERROR + no semgrep, P0 -> soft (unverifiable)",
       _r[0] is None and _r[1] is not None and "could not read" in _r[1])
_r = m.taint_or_gate("error", None, False, True)
record("(f) CodeQL READ-ERROR + no semgrep, --enforce -> HARD (unverifiable)", _r[0] is not None and _r[1] is None)
record("(f) CodeQL READ-ERROR but semgrep present -> ok (OR short-circuits)",
       m.taint_or_gate("error", None, True, True) == (None, None))

# --- semgrep_taint_ruleset_present ------------------------------------------------------------
with tempfile.TemporaryDirectory() as td:
    d = Path(td)
    (d / "r.yml").write_text("rules:\n  - id: x\n    mode: taint\n", encoding="utf-8")
    record("semgrep dir with a `mode: taint` rule -> present", m.semgrep_taint_ruleset_present(d))
with tempfile.TemporaryDirectory() as td:
    d = Path(td)
    (d / "r.yml").write_text("rules:\n  - id: x\n    pattern: foo\n", encoding="utf-8")
    record("semgrep dir without a taint rule -> absent", not m.semgrep_taint_ruleset_present(d))
with tempfile.TemporaryDirectory() as td:
    d = Path(td)
    (d / "decoy.yml").write_text("# a rule that mentions taint in a comment, no mode\n"
                                 "rules:\n  - id: taint-lookalike\n    pattern: foo\n", encoding="utf-8")
    record("semgrep DECOY (comment/id mention 'taint', no `mode: taint`) -> absent",
           not m.semgrep_taint_ruleset_present(d))
record("semgrep dir missing -> absent", not m.semgrep_taint_ruleset_present(Path(td) / "nope"))

# --- main() posture on a missing repo (no network needed) -------------------------------------
record("main no-repo, no --enforce -> fail-soft exit 0", m.main(["--repo", ""]) == 0)
record("main no-repo, --enforce -> bad-invocation exit 2", m.main(["--repo", "", "--enforce"]) == 2)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-branch-protection] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
