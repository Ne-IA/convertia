#!/usr/bin/env python3
"""g24-selftest-runner-fastpath.py - G24 self-test for run-gate-selftests' --changed fastpath.

The L2 slot the G24 row declared from P0 landed 2026-08-30 as a --changed gate on the full-canary
runner. These legs pin the DECISION (pure fn - hermetic, no live git state) and the CROSS-PLANE
WIRING as PER-LINE rules over every non-comment `run-gate-selftests` invocation - lefthook.yml:
every one carries --changed; ci.yml: every one carries --require-network and NONE carries
--changed - flag-order- and second-call-site-robust, mutation-tested against the appended
`--require-network --changed` form (the R1 opus P1: a literal-adjacency substring pin passed that
mutation green). The fail-safe direction is pinned both ways: a canary may over-run, never
under-run. Named with the g24- prefix DELIBERATELY: it is a G24 wiring/planted-positive self-test
for an internal fastpath of the G24 runner, not a G10 `test-*-fastpath-pattern` detector.

Run:  python3 scripts/gate-selftests/g24-selftest-runner-fastpath.py
Exit: 0 = every assertion held; 1 = a self-test assertion FAILED.
"""
import importlib.machinery
import importlib.util
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "run-gate-selftests"
_loader = importlib.machinery.SourceFileLoader("rgs", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("rgs", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def _runner_lines(text: str) -> list[str]:
    """Every NON-comment line invoking run-gate-selftests (a flag in a comment does not count -
    the check-27 'promised, not applied' discipline)."""
    out = []
    for line in text.splitlines():
        s = line.split(" #", 1)[0].strip()
        if s and not s.startswith("#") and "run-gate-selftests" in s:
            out.append(s)
    return out


def lefthook_ok(text: str) -> bool:
    """L2 rule: EVERY invocation file-wide carries --changed (flag order irrelevant), and at least
    one sits UNDER the `pre-push:` key — the hook-TIER bind (the R2 opus P3: a slot relocated into
    pre-commit would blow the L1 budget while a whole-file rule stayed green). pre-push is the last
    top-level hook section in this file; the slice-to-end is exact for that layout."""
    lines = _runner_lines(text)
    idx = 0 if text.startswith("pre-push:") else text.find("\npre-push:")
    prepush = _runner_lines(text[idx:]) if idx != -1 else []
    return bool(lines) and all("--changed" in ln for ln in lines) and bool(prepush)


def ci_ok(text: str) -> bool:
    """L4 rule: >=1 invocation, EVERY one carries --require-network, NONE carries --changed."""
    lines = _runner_lines(text)
    return (bool(lines) and all("--require-network" in ln for ln in lines)
            and not any("--changed" in ln for ln in lines))


# --- the pure decision (fail-safe toward RUN) --------------------------------------------------
record("no resolvable base (first push / no git) -> RUN (fail-safe)",
       m.fastpath_decision(False, None)[0] == "run")
record("base resolved but the range diff unreadable -> RUN (fail-safe)",
       m.fastpath_decision(True, None)[0] == "run")
record("an empty push range -> SKIP", m.fastpath_decision(True, [])[0] == "skip")
record("a docs/src-only range -> SKIP (the ~0 s normal-push path)",
       m.fastpath_decision(True, ["docs/plan/P4-engine-framework.md", "src-tauri/src/pool/mod.rs"])[0] == "skip")
record("a gate SCRIPT in the range -> RUN",
       m.fastpath_decision(True, ["docs/x.md", "scripts/check-doc-links"])[0] == "run")
record("a gate SELF-TEST in the range -> RUN (scripts/** is wholesale)",
       m.fastpath_decision(True, ["scripts/gate-selftests/g24-plan-lint.py"])[0] == "run")
record("a gate-config TOML in the range -> RUN (scripts/** is wholesale)",
       m.fastpath_decision(True, ["scripts/gate-planes.toml"])[0] == "run")
record("lefthook.yml in the range -> RUN (a wiring-only push arms the pins at L2)",
       m.fastpath_decision(True, ["lefthook.yml"])[0] == "run")
record("a .github/workflows/ change -> RUN (the L4 plane is gate tooling too)",
       m.fastpath_decision(True, [".github/workflows/ci.yml"])[0] == "run")
record("requirements-ci.txt in the range -> RUN (the pip gate-toolchain pin)",
       m.fastpath_decision(True, ["requirements-ci.txt"])[0] == "run")
record("a backslash-separated path still matches the scope (Windows diff hygiene)",
       m.fastpath_decision(True, ["scripts\\check-x"])[0] == "run")
record("a QUOTED path shape still matches (the quotepath under-run closed both ways)",
       m.fastpath_decision(True, ['"scripts/check-ä.py"'])[0] == "run")
record("a NEAR-MISS prefix (scripts-extra/) does NOT arm the canary",
       m.fastpath_decision(True, ["scripts-extra/x"])[0] == "skip")
record("the runner's git wrapper pins core.quotepath=false (source pin)",
       '"core.quotepath=false"' in SCRIPT.read_text(encoding="utf-8"))

# --- the cross-plane wiring rules: mutation-tested pure legs -----------------------------------
record("ci rule: the APPENDED `--require-network --changed` mutation is RED (the R1 opus P1 probe)",
       not ci_ok("      - name: x\n        run: python3 scripts/run-gate-selftests --require-network --changed\n"))
record("ci rule: a clean full-prelude line is GREEN",
       ci_ok("        run: python3 scripts/run-gate-selftests --require-network\n"))
record("ci rule: a SECOND call site lacking --require-network is RED (per-line, not first-match)",
       not ci_ok("        run: python3 scripts/run-gate-selftests --require-network\n"
                 "        run: python3 scripts/run-gate-selftests\n"))
record("ci rule: zero invocations is RED (the prelude cannot silently vanish)", not ci_ok("jobs: {}\n"))
record("lefthook rule: flag-order-robust - `--require-network --changed` under pre-push is GREEN "
       "(no false red)",
       lefthook_ok("pre-push:\n  commands:\n    g:\n"
                   "      run: python3 scripts/run-gate-selftests --require-network --changed\n"))
record("lefthook rule: a slot line MISSING --changed is RED",
       not lefthook_ok("      run: python3 scripts/run-gate-selftests\n"))
record("lefthook rule: a flag only in a COMMENT does not count as wired",
       not lefthook_ok("    # run: python3 scripts/run-gate-selftests --changed\n"))
record("lefthook rule: a --changed slot ONLY under pre-commit is RED (the hook-TIER bind)",
       not lefthook_ok("pre-commit:\n  commands:\n    x:\n"
                       "      run: python3 scripts/run-gate-selftests --changed\n"
                       "\npre-push:\n  commands:\n    y:\n      run: echo ok\n"))

# --- the real plane files satisfy the rules ----------------------------------------------------
record("lefthook.yml wires `run-gate-selftests --changed` per the L2 rule",
       lefthook_ok((REPO / "lefthook.yml").read_text(encoding="utf-8")))
record("ci.yml keeps the FULL --require-network prelude and never passes --changed (L4 rule)",
       ci_ok((REPO / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")))

failed = [n for n, ok in results if not ok]
print(f"\n[g24-selftest-runner-fastpath] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
