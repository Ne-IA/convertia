#!/usr/bin/env python3
"""g24-gitleaks-allowlist.py - G24 self-test for check-gitleaks-allowlist (P0.3.1, G2 growth-guard).

Proves the growth-guard FAILS on every way the gitleaks config could quietly SWALLOW a secret — a
widened/changed top-level allowlist path, a value-level regexes/stopwords blanket, gitleaks' bundled
defenses turned off (useDefault=false), a deleted/renamed custom rule, a per-rule allowlist, a
baseline holding a finding — and PASSES the committed config. stdlib-only. Exit 0 = all held; 1 = a fail.
"""
import importlib.machinery
import importlib.util
import subprocess
import sys
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-gitleaks-allowlist"
_loader = importlib.machinery.SourceFileLoader("cga", str(SCRIPT))
_spec = importlib.util.spec_from_loader("cga", _loader)
m = importlib.util.module_from_spec(_spec)
_loader.exec_module(m)

PATHS = set(m.EXPECTED_ALLOWLIST_PATHS)
RULE_IDS = set(m.EXPECTED_CUSTOM_RULE_IDS)
results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def base() -> dict:
    """A minimal VALID parsed .gitleaks.toml: useDefault on, all custom rules present, frozen paths."""
    return {
        "extend": {"useDefault": True},
        "rules": [{"id": rid} for rid in RULE_IDS],
        "allowlist": {"paths": list(PATHS)},
    }


record("valid config + empty baseline -> no problems", m.evaluate(base(), []) == [])

c = base(); c["allowlist"]["paths"] = list(PATHS) + ["(^|/)src/"]
record("ADDED allowlist path -> drift", any("drifted" in p for p in m.evaluate(c, [])))

c = base(); c["allowlist"]["paths"] = list(PATHS)[:1]
record("REMOVED allowlist path -> drift", any("drifted" in p for p in m.evaluate(c, [])))

c = base(); c["allowlist"]["regexes"] = ["sk-ant-.*"]
record("top-level `regexes` blanket -> forbidden", any("regexes" in p for p in m.evaluate(c, [])))

c = base(); c["allowlist"]["stopwords"] = ["x"]
record("top-level `stopwords` blanket -> forbidden", any("stopwords" in p for p in m.evaluate(c, [])))

c = base(); c["extend"]["useDefault"] = False
record("useDefault=false -> bundled defenses dropped", any("useDefault" in p for p in m.evaluate(c, [])))

c = base(); del c["extend"]
record("missing [extend] entirely -> useDefault not true", any("useDefault" in p for p in m.evaluate(c, [])))

c = base(); c["rules"] = [{"id": rid} for rid in list(RULE_IDS)[1:]]   # drop one custom rule
record("a custom rule deleted/renamed -> missing-rule", any("is missing" in p for p in m.evaluate(c, [])))

c = base(); c["rules"][0]["allowlist"] = {"regexes": ["sk-ant-.*"]}
record("a PER-RULE allowlist -> forbidden", any("per-rule allowlist" in p for p in m.evaluate(c, [])))

record("baseline with 1 accepted finding -> growth",
       any("baseline holds" in p for p in m.evaluate(base(), [{"RuleID": "x"}])))
record("baseline not a JSON array -> rejected",
       any("not a JSON array" in p for p in m.evaluate(base(), {"oops": 1})))

# the REAL committed config + baseline must pass (exit 0)
rc = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True, text=True, encoding="utf-8", errors="replace").returncode
record("committed .gitleaks.toml + baseline pass the growth-guard (exit 0)", rc == 0)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-gitleaks-allowlist] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
