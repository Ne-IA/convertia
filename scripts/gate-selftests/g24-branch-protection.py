#!/usr/bin/env python3
"""g24-branch-protection.py - G24 self-test for check-branch-protection (P0.2.8, G56a).

Drives the PURE evaluate_*() functions with fixture JSON (the live `gh api` calls are owner/CI-
verified - G56a is exempt from the plan-lint check-16 fixture requirement, an API-introspection
gate). Proves: the ruleset core (ruleset_targets_main; bypass_is_admin_only; evaluate_rulesets -
required checks subset, admin-only check-bypass, no-bypass history protection, required_signatures
target-gating), the repo-security (d), workflow-perms (e) and the T2-taint OR-gate (f) all classify
correctly, and main() handles a missing repo per posture.
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


ADMIN_BYPASS = [{"actor_type": "RepositoryRole", "actor_id": 5, "bypass_mode": "always"}]


def rs_checks(contexts=EXPECTED, bypass="admin", sig=False):
    """A ruleset hosting a required_status_checks rule (+ optionally a required_signatures rule)."""
    by = ADMIN_BYPASS if bypass == "admin" else (bypass or [])
    rules = [{"type": "required_status_checks",
              "parameters": {"required_status_checks": [{"context": c} for c in contexts]}}]
    if sig:
        rules.append({"type": "required_signatures"})
    return {"name": "checks", "target": "branch", "enforcement": "active",
            "conditions": {"ref_name": {"include": ["refs/heads/main"], "exclude": []}},
            "bypass_actors": by, "rules": rules}


def rs_history(bypass=None, rules=("non_fast_forward", "deletion")):
    """A ruleset hosting the history-protection rules (force-push / deletion)."""
    return {"name": "history", "target": "branch", "enforcement": "active",
            "conditions": {"ref_name": {"include": ["refs/heads/main"], "exclude": []}},
            "bypass_actors": bypass or [], "rules": [{"type": t} for t in rules]}


# --- ruleset_targets_main ---------------------------------------------------------------------
def _rs(include, exclude=None, target="branch"):
    return {"target": target, "conditions": {"ref_name": {"include": include, "exclude": exclude or []}}}


record("targets_main: include refs/heads/main -> True", m.ruleset_targets_main(_rs(["refs/heads/main"])))
record("targets_main: include ~ALL -> True", m.ruleset_targets_main(_rs(["~ALL"])))
record("targets_main: include ~DEFAULT_BRANCH -> True", m.ruleset_targets_main(_rs(["~DEFAULT_BRANCH"])))
record("targets_main: main excluded -> False",
       not m.ruleset_targets_main(_rs(["~ALL"], exclude=["refs/heads/main"])))
record("targets_main: tag target -> False", not m.ruleset_targets_main(_rs(["refs/heads/main"], target="tag")))

# --- bypass_is_admin_only ---------------------------------------------------------------------
record("bypass admin-only -> True", m.bypass_is_admin_only({"bypass_actors": ADMIN_BYPASS}))
record("bypass empty -> True (vacuous)", m.bypass_is_admin_only({"bypass_actors": []}))
record("bypass a user actor -> False",
       not m.bypass_is_admin_only({"bypass_actors": [{"actor_type": "User", "actor_id": 42}]}))
record("bypass a wider role (Write=3) -> False",
       not m.bypass_is_admin_only({"bypass_actors": [{"actor_type": "RepositoryRole", "actor_id": 3}]}))

# --- bypass_unreadable (absent/null vs genuinely-empty) ---------------------------------------
record("bypass_unreadable: present empty [] -> False (readable, no bypass)",
       not m.bypass_unreadable({"bypass_actors": []}))
record("bypass_unreadable: absent key -> True", m.bypass_unreadable({}))
record("bypass_unreadable: null value -> True", m.bypass_unreadable({"bypass_actors": None}))

# --- evaluate_rulesets ------------------------------------------------------------------------
h, s = m.evaluate_rulesets([rs_checks(), rs_history()], EXPECTED, False)
record("RS valid (admin-bypass checks + no-bypass history) -> no hard", not h)
record("RS valid + sig-not-required -> (g) soft note", any("required_signatures" in x for x in s))

h, _ = m.evaluate_rulesets([rs_history()], EXPECTED, False)
record("RS missing the checks ruleset -> hard", any("required_status_checks rule" in x for x in h))

h, _ = m.evaluate_rulesets([rs_checks(contexts=EXPECTED[1:]), rs_history()], EXPECTED, False)
record("RS missing a required check -> hard", any("present-and-required" in x for x in h))

h, _ = m.evaluate_rulesets(
    [rs_checks(bypass=[{"actor_type": "User", "actor_id": 42}]), rs_history()], EXPECTED, False)
record("RS non-admin bypass on the checks ruleset -> hard", any("non-admin bypass" in x for x in h))

h, _ = m.evaluate_rulesets([rs_checks(), rs_history(rules=("deletion",))], EXPECTED, False)
record("RS missing the force-push block -> hard", any("force-push" in x for x in h))

h, _ = m.evaluate_rulesets([rs_checks(), rs_history(rules=("non_fast_forward",))], EXPECTED, False)
record("RS missing the deletion block -> hard", any("deletion" in x for x in h))

h, _ = m.evaluate_rulesets([rs_checks(), rs_history(bypass=ADMIN_BYPASS)], EXPECTED, False)
record("RS history ruleset WITH a bypass actor -> hard", any("history protection must apply" in x for x in h))

h, _ = m.evaluate_rulesets([rs_checks(sig=True), rs_history()], EXPECTED, True)
record("RS required_signatures present + required -> no hard", not h)

h, _ = m.evaluate_rulesets([rs_checks(sig=False), rs_history()], EXPECTED, True)
record("RS required_signatures MISSING while allowed-signers exists -> hard",
       any("required_signatures rule" in x for x in h))

_no_by = rs_checks()
del _no_by["bypass_actors"]
h, _ = m.evaluate_rulesets([_no_by, rs_history()], EXPECTED, False)
record("RS checks ruleset bypass_actors UNREADABLE (omitted) -> hard", any("not readable" in x for x in h))

_hist_no_by = rs_history()
del _hist_no_by["bypass_actors"]
h, _ = m.evaluate_rulesets([rs_checks(), _hist_no_by], EXPECTED, False)
record("RS history ruleset bypass_actors UNREADABLE (omitted) -> hard", any("not readable" in x for x in h))

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
