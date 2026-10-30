#!/usr/bin/env python3
"""g24-branch-protection.py - G24 self-test for check-branch-protection (P0.2.8, G56a).

Drives the PURE evaluate_*() functions with fixture JSON (the live `gh api` calls are owner/CI-
verified - an API-introspection gate, so this file is its planted positive). Proves: the ruleset core
(ruleset_targets_main; bypass_is_admin_only; evaluate_rulesets - required checks subset, admin-only
check-bypass, no-bypass history protection, required_signatures target-gating), the repo-security (d)
and workflow-perms (e) evaluators classify correctly, the transient-retry classifier, api_json under the
gate plane's network retry policy (scripts/net-retry, with a fake `gh` and a fake clock), and main()
handles a missing repo per posture. Sub-check (f) is retired (plan-lint check 21 asserts the G29
rule-(i) file). stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import subprocess
import sys
from pathlib import Path
from unittest import mock
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

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

# [Test-Change: G56a sub-check (f) retired — old-obsolete+new-correct, build-gates §6 item 21] the (f) taint
# OR-gate (CodeQL default setup or a Semgrep `mode: taint` file) and its Semgrep-dir probe are gone from the
# script: plan-lint check 21 asserts the G29 rule-(i) file (its comment-decoy legs moved there) and CodeQL is
# not adopted for v1 (gate-status.md), so the taint_or_gate and semgrep_taint_ruleset_present legs retire.

# --- api_json transient-retry classification (the spurious-red-main guard, G56a) --------------
# A transient gh/network failure (TLS/connection hiccup, 5xx/429) is retried before the gate
# fail-CLOSES; a 404 / non-transient HTTP error (403/auth) is NOT (it returns on the first try).
record("transient: TLS handshake timeout -> retried",
       bool(m._TRANSIENT_RE.search("Post https://api.github.com: net/http: TLS handshake timeout")))
record("transient: 502 Bad Gateway -> retried", bool(m._TRANSIENT_RE.search("HTTP 502: Bad Gateway")))
record("transient: 503 Service Unavailable -> retried", bool(m._TRANSIENT_RE.search("HTTP 503: Service Unavailable")))
record("transient: connection reset -> retried", bool(m._TRANSIENT_RE.search("read tcp: connection reset by peer")))
record("transient: 429 secondary rate limit -> retried",
       bool(m._TRANSIENT_RE.search("HTTP 429: too many requests (secondary rate limit)")))
record("non-transient: 404 NOT retried", not m._TRANSIENT_RE.search("HTTP 404: Not Found"))
record("non-transient: 403 forbidden NOT retried", not m._TRANSIENT_RE.search("HTTP 403: Forbidden (token lacks scope)"))


# --- api_json under the gate plane's one network retry policy (scripts/net-retry, build-gates §0) --------------
# Driven with a fake `run_bounded` of the gate's own policy module (patched only inside the context manager) and a
# fake-clock Retrier from it, so no leg waits: the pauses are the policy's (2 s doubling to a 60 s cap, until 300 s
# after the first failure) - a gate back on its own fixed pauses reds here. gh's status is read from its status line
# only (`gh_status`), so a `404` inside a request path names no status.
class _Clock:
    def __init__(self) -> None:
        self.now = 0.0
        self.pauses: list[float] = []

    def clock(self) -> float:
        return self.now

    def sleep(self, seconds: float) -> None:
        self.pauses.append(seconds)
        self.now += seconds


_BOUNDS_SEEN: list[float] = []      # the wall-clock bound of every gh run the fake answered


def _api(outcomes: list, fresh: bool = True) -> tuple[tuple, int, _Clock]:
    """api_json('repos/o/r') over a scripted gh: each entry is (returncode, stdout, stderr); the last entry repeats.
    `fresh` installs a new fake-clock Retrier first. Returns (the api_json result, the number of gh runs, the fake
    clock); every run's bound is kept in _BOUNDS_SEEN."""
    fc = _Clock()
    if fresh:
        m._RETRY = m._NET.Retrier("G56a", clock=fc.clock, sleep=fc.sleep, rng=lambda: 1.0, log=lambda _msg: None)
    calls = {"n": 0}

    def fake_run_bounded(cmd, seconds, *, env=None):
        _BOUNDS_SEEN.append(seconds)
        step = outcomes[min(calls["n"], len(outcomes) - 1)]
        calls["n"] += 1
        return subprocess.CompletedProcess(cmd, step[0], step[1], step[2])

    with mock.patch.object(m._NET, "run_bounded", fake_run_bounded):
        result = m.api_json("repos/o/r")
    return result, calls["n"], fc


_REAL_RETRY = m._RETRY
_GH_502 = (1, "", "gh: HTTP 502: Bad Gateway (https://api.github.com/repos/o/r)")
res, n, fc = _api([_GH_502, _GH_502, (0, '{"a": 1}', "")])
record("api_json retry: a transient 502 is retried with the policy's pauses (2 s, 4 s) until the read succeeds",
       res == ("ok", {"a": 1}, "") and n == 3 and fc.pauses == [2.0, 4.0])
res, n, fc = _api([(1, "", "gh: Not Found (HTTP 404)")])
record("api_json retry: a 404 returns 'notfound' on the first read (no pause)", res[0] == "notfound" and n == 1
       and fc.pauses == [])
res, n, fc = _api([(1, "", "gh: HTTP 403: Resource not accessible by integration")])
record("api_json retry: a 403 returns 'error' on the first read (no pause)", res[0] == "error" and n == 1
       and fc.pauses == [])
res, n, fc = _api([(124, "", "gh: timed out - no result within the 60 s bound of one attempt"), (0, "[]", "")])
record("api_json retry: a read past its wall-clock bound is a transient timeout, retried",
       res == ("ok", [], "") and n == 2 and fc.pauses == [2.0])
res, n, fc = _api([(1, "", "gh: HTTP 502 (https://api.github.com/repos/o/r/commits/a404b8e/check-runs)"),
                   (0, "{}", "")])
record("api_json retry: a 502 whose request path holds `404` is retried, never read as 'notfound' (the status "
       "comes from gh's status line)", res == ("ok", {}, "") and n == 2 and fc.pauses == [2.0])
res, n, fc = _api([(1, "", "gh: You have exceeded a secondary rate limit (HTTP 403)"), (0, "{}", "")])
record("api_json retry: a 403 naming a secondary rate limit is retried", res == ("ok", {}, "") and n == 2)
res, n, fc = _api([(1, "", 'Post "https://api.github.com/graphql": dial tcp 140.82.112.6:443: connect: '
                           "connection refused"), (0, "{}", "")])
record("api_json retry: a request that got no answer (a refused connection) is retried",
       res == ("ok", {}, "") and n == 2)
record("gh_status: gh's two status-line forms and none", (m.gh_status("gh: Not Found (HTTP 404)"),
       m.gh_status("gh: HTTP 502"), m.gh_status("error: path a404b/HTTP 404x")) == (404, 502, None))
res, n, fc = _api([_GH_502])
record("api_json retry: a transient that outlasts the 300 s window returns 'error' (fail-closed under --enforce)",
       res[0] == "error" and "502" in res[2] and n == 10 and fc.now == 300.0)
res, n, fc = _api([(0, "{}", "")], fresh=False)
record("api_json retry: after a read spent the window, the run's next read is not attempted ('error')",
       res[0] == "error" and "not attempted" in res[2] and n == 0)
record("api_json: every gh read runs under the module's wall-clock bound (_GH_ATTEMPT_TIMEOUT_S, 60 s)",
       bool(_BOUNDS_SEEN) and all(s == m._GH_ATTEMPT_TIMEOUT_S == 60 for s in _BOUNDS_SEEN),
       f"bounds seen {sorted(set(_BOUNDS_SEEN))}")
m._RETRY = _REAL_RETRY

# --- main() posture on a missing repo (no network needed) -------------------------------------
record("main no-repo, no --enforce -> fail-soft exit 0", m.main(["--repo", ""]) == 0)
record("main no-repo, --enforce -> bad-invocation exit 2", m.main(["--repo", "", "--enforce"]) == 2)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-branch-protection] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
