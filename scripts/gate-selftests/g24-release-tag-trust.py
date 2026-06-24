#!/usr/bin/env python3
"""g24-release-tag-trust.py - G24 self-test for check-release-tag-trust (P0.2.9, G56b).

Proves the three trust legs behave fail-closed/fail-soft as specified, by importing the
gate module and driving its pure + git-based functions:
  - tag_name_from_ref: ref normalization (a non-tag ref must NOT pass);
  - evaluate_check_runs: the green-history parse (the leg-2 logic, on fixture JSON - the live
    `gh api` call is release/owner-verified, the G56a/P0.2.8 API-introspection exemption);
  - leg_ancestry: a tag on origin/main passes, a descendant-not-on-origin/main fails
    (real temp git repo);
  - leg_signed_tag: fail-SOFT when the allowed-signers file is absent, fail-CLOSED (unsigned
    tag rejected) once it is present.

stdlib-only. Exit 0 = every assertion held; 1 = a self-test assertion FAILED.
"""
import importlib.machinery
import importlib.util
import os
import subprocess
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-release-tag-trust"
_loader = importlib.machinery.SourceFileLoader("crt", str(SCRIPT))
_spec = importlib.util.spec_from_loader("crt", _loader)
crt = importlib.util.module_from_spec(_spec)
_loader.exec_module(crt)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}{(' - ' + detail) if detail else ''}")


def g(cwd: str, *args: str) -> None:
    subprocess.run(["git", *args], cwd=cwd, check=True,
                   capture_output=True, text=True, encoding="utf-8", errors="replace")


# --- tag_name_from_ref ------------------------------------------------------
record("ref: refs/tags/v1.2.3 -> v1.2.3", crt.tag_name_from_ref("refs/tags/v1.2.3") == "v1.2.3")
record("ref: bare v2 -> v2", crt.tag_name_from_ref("v2") == "v2")
record("ref: refs/heads/main -> None (non-tag rejected)", crt.tag_name_from_ref("refs/heads/main") is None)
record("ref: refs/pull/1/merge -> None", crt.tag_name_from_ref("refs/pull/1/merge") is None)

# --- evaluate_check_runs (leg 2 green-history parse) ------------------------
record("checks: all success -> green",
       crt.evaluate_check_runs({"check_runs": [
           {"name": "ci", "status": "completed", "conclusion": "success"},
           {"name": "wf-sec", "status": "completed", "conclusion": "success"}]})[0] is True)
record("checks: a failure -> NOT green",
       crt.evaluate_check_runs({"check_runs": [
           {"name": "ci", "status": "completed", "conclusion": "success"},
           {"name": "wf-sec", "status": "completed", "conclusion": "failure"}]})[0] is False)
record("checks: in_progress -> NOT green",
       crt.evaluate_check_runs({"check_runs": [
           {"name": "ci", "status": "in_progress", "conclusion": None}]})[0] is False)
record("checks: cancelled -> NOT green",
       crt.evaluate_check_runs({"check_runs": [
           {"name": "ci", "status": "completed", "conclusion": "cancelled"}]})[0] is False)
record("checks: empty -> NOT green (fail-closed)",
       crt.evaluate_check_runs({"check_runs": []})[0] is False)
record("checks: skipped+neutral -> green",
       crt.evaluate_check_runs({"check_runs": [
           {"name": "a", "status": "completed", "conclusion": "skipped"},
           {"name": "b", "status": "completed", "conclusion": "neutral"}]})[0] is True)
record("checks: completed+null conclusion -> NOT green",
       crt.evaluate_check_runs({"check_runs": [
           {"name": "ci", "status": "completed", "conclusion": None}]})[0] is False)
record("checks: timed_out -> NOT green",
       crt.evaluate_check_runs({"check_runs": [
           {"name": "ci", "status": "completed", "conclusion": "timed_out"}]})[0] is False)
record("checks: bare {} (key absent) -> NOT green",
       crt.evaluate_check_runs({})[0] is False)

# --- flatten_pages (gh --paginate --slurp multi-page normalization) ---------
record("flatten: --slurp list of 2 pages -> all runs merged",
       crt.flatten_pages([{"check_runs": [{"name": "a", "status": "completed", "conclusion": "success"}]},
                          {"check_runs": [{"name": "b", "status": "completed", "conclusion": "success"}]}]
                         )["check_runs"].__len__() == 2)
record("flatten: 2-page with a 2nd-page failure -> NOT green (no silent drop)",
       crt.evaluate_check_runs(crt.flatten_pages([
           {"check_runs": [{"name": "a", "status": "completed", "conclusion": "success"}]},
           {"check_runs": [{"name": "b", "status": "completed", "conclusion": "failure"}]}]))[0] is False)
record("flatten: single object (no pagination) -> runs preserved",
       crt.flatten_pages({"check_runs": [{"name": "a", "status": "completed", "conclusion": "success"}]}
                         )["check_runs"].__len__() == 1)
record("flatten: empty list -> no runs (fail-closed downstream)",
       crt.evaluate_check_runs(crt.flatten_pages([]))[0] is False)

# --- gh_api transient-retry classification (the spurious-red guard, G56b) ---
# A transient gh/network failure (TLS/connection hiccup, 5xx/429) is retried before the gate
# fails a release; a 404 / non-transient HTTP error (403/auth) is NOT (it returns on the first try).
record("transient: TLS handshake timeout -> retried",
       bool(crt._TRANSIENT_RE.search("Post https://api.github.com: net/http: TLS handshake timeout")))
record("transient: 502 Bad Gateway -> retried", bool(crt._TRANSIENT_RE.search("HTTP 502: Bad Gateway")))
record("transient: connection reset -> retried", bool(crt._TRANSIENT_RE.search("read tcp: connection reset by peer")))
record("transient: 429 secondary rate limit -> retried",
       bool(crt._TRANSIENT_RE.search("HTTP 429: too many requests (secondary rate limit)")))
record("non-transient: 404 NOT retried", not crt._TRANSIENT_RE.search("HTTP 404: Not Found"))
record("non-transient: 403 forbidden NOT retried", not crt._TRANSIENT_RE.search("HTTP 403: Forbidden"))

# --- leg_ancestry + leg_signed_tag (real temp git repo) ---------------------
with tempfile.TemporaryDirectory() as td:
    g(td, "init", "-b", "main")
    g(td, "config", "user.email", "t@t.t")
    g(td, "config", "user.name", "t")
    g(td, "config", "commit.gpgsign", "false")
    g(td, "config", "tag.gpgsign", "false")
    (Path(td) / "a.txt").write_text("1", encoding="utf-8")
    g(td, "add", "-A")
    g(td, "commit", "-m", "c1")
    c1 = subprocess.run(["git", "rev-parse", "HEAD"], cwd=td, capture_output=True, text=True, encoding="utf-8", errors="replace").stdout.strip()
    # origin/main := c1 (the released history)
    g(td, "update-ref", "refs/remotes/origin/main", c1)
    g(td, "tag", "v1")  # lightweight tag at c1 (on origin/main, unsigned)
    g(td, "tag", "-a", "v2", "-m", "annotated release")  # annotated (unsigned) tag at c1
    # a second commit that never landed on origin/main
    (Path(td) / "a.txt").write_text("2", encoding="utf-8")
    g(td, "add", "-A")
    g(td, "commit", "-m", "c2")
    c2 = subprocess.run(["git", "rev-parse", "HEAD"], cwd=td, capture_output=True, text=True, encoding="utf-8", errors="replace").stdout.strip()

    cwd = os.getcwd()
    os.chdir(td)
    try:
        record("leg1: tag commit on origin/main -> ancestry ok", crt.leg_ancestry(c1) is True)
        record("leg1: commit not on origin/main -> ancestry FAILS", crt.leg_ancestry(c2) is False)
        # leg 3: allowed-signers absent -> fail-soft (skip, returns True)
        record("leg3: allowed-signers absent -> fail-soft pass",
               crt.leg_signed_tag("v1", Path(td) / "no-such-allowed-signers") is True)
        # leg 3: allowed-signers present + UNSIGNED tag -> fail-closed (verify-tag fails)
        signers = Path(td) / "allowed-signers"
        signers.write_text("owner@ne-ia ssh-ed25519 AAAAfake\n", encoding="utf-8")
        record("leg3: allowed-signers present + unsigned tag -> fail-closed",
               crt.leg_signed_tag("v1", signers) is False)
        # resolve_tag_sha dereferences both a lightweight and an annotated tag to the commit
        record("resolve: lightweight tag -> commit sha", crt.resolve_tag_sha("v1") == c1)
        record("resolve: annotated tag -> commit sha (dereferenced)", crt.resolve_tag_sha("v2") == c1)
    finally:
        os.chdir(cwd)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-release-tag-trust] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
