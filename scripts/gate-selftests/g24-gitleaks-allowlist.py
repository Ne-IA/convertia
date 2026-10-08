#!/usr/bin/env python3
"""g24-gitleaks-allowlist.py - G24 self-test for check-gitleaks-allowlist (P0.3.1, G2 growth-guard).

Proves the growth-guard FAILS on every way the gitleaks config could quietly SWALLOW a secret — a
widened/changed top-level allowlist path, any allowlist key beside `paths` (a value-level
regexes/stopwords/commits blanket, a condition, targetRules), a second `[[allowlists]]` table or another
key outside the frozen top-level shape, gitleaks' bundled defenses turned off (useDefault=false) or cut
down (`disabledRules`) or a second config loaded (`[extend].path`/`url`), a deleted/renamed or an extra
custom rule, a per-rule allowlist (both spellings), `skipReport`, a `path`/`required` fire condition, a
baseline holding a finding, a tracked `.gitleaksignore` at any depth or letter case (and main() failing
closed when the tracked-file list cannot be read) — and PASSES the committed config and a rule using every
permitted key. stdlib-only. Exit 0 = all held; 1 = a fail.
"""
import contextlib
import importlib.machinery
import importlib.util
import io
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


def flags(cfg: dict, needle: str) -> bool:
    """Whether evaluate() over `cfg` (+ an empty baseline) reports a problem containing `needle`."""
    return any(needle in p for p in m.evaluate(cfg, []))


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

# --- the frozen SHAPE: every other key gitleaks reads that can drop a finding is refused --------------
c = base(); c["rules"][0].update({"description": "d", "regex": "x", "secretGroup": 1, "entropy": 3.5,
                                  "keywords": ["k"], "tags": ["t"]})
record("a custom rule using every permitted key (a pure pattern) -> no problems", m.evaluate(c, []) == [])

c = base(); c["allowlists"] = [{"targetRules": ["minisign-secret-key"], "paths": ["leak"]}]
record("a top-level `[[allowlists]]` table (targetRules) -> a second suppression table", flags(c, "`[[allowlists]]`"))

c = base(); c["unreviewed"] = True
record("an unknown top-level key -> outside the frozen config shape", flags(c, "`unreviewed` is outside the frozen"))

for key, value in (("disabledRules", ["github-pat"]), ("path", "other.toml"), ("url", "https://example.invalid/x")):
    c = base(); c["extend"][key] = value
    record(f"`[extend].{key}` beside useDefault=true -> refused", flags(c, f"`[extend].{key}` is set"))

c = base(); c["rules"].append({"id": "github-pat", "regex": "never"})
record("an extra custom rule (a bundled rule's id) -> outside the frozen id set", flags(c, "`github-pat` is outside"))

c = base(); c["rules"][0]["allowlists"] = [{"paths": ["leak"]}]
record("a PER-RULE `[[rules.allowlists]]` table (the plural spelling) -> forbidden", flags(c, "per-rule allowlist (`allowlists`)"))

for key, value in (("skipReport", True), ("path", "never-matches"), ("required", [{"id": "x"}])):
    c = base(); c["rules"][0][key] = value
    record(f"a rule declaring `{key}` -> not a pure pattern", flags(c, f"declares `{key}`"))

for key, value in (("commits", ["abc"]), ("regexTarget", "line"), ("condition", "AND"), ("targetRules", ["x"])):
    c = base(); c["allowlist"][key] = value
    record(f"allowlist declares `{key}` beside the frozen paths -> forbidden", flags(c, f"allowlist declares `{key}`"))

# --- a tracked `.gitleaksignore` at any depth or letter case --------------------------------------------
record("tracked-file scan: `.gitleaksignore` at the root, nested and in another letter case -> flagged",
       m.tracked_ignore_files([".gitleaksignore", "docs/.gitleaksignore", "a/b/.GitleaksIgnore"]) ==
       [".gitleaksignore", "docs/.gitleaksignore", "a/b/.GitleaksIgnore"])
record("tracked-file scan: a look-alike name (`.gitleaksignore.bak`, `x.gitleaksignore`, a directory prefix) -> "
       "not flagged",
       m.tracked_ignore_files([".gitleaksignore.bak", "x.gitleaksignore", ".gitleaksignore/readme", "gitleaksignore"])
       == [])


def main_in(root: Path) -> tuple[int, str]:
    """check-gitleaks-allowlist main() with `root` as the repository its `git ls-files` lists (the committed
    config + baseline stay the real ones); (exit code, output)."""
    buf = io.StringIO()
    saved = m.ROOT
    m.ROOT = root
    try:
        with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(buf):
            rc = m.main()
    finally:
        m.ROOT = saved
    return rc, buf.getvalue()


with tempfile.TemporaryDirectory() as td:
    repo = Path(td) / "work"
    (repo / "sub").mkdir(parents=True)
    (repo / "sub" / ".gitleaksignore").write_bytes(b"leak.txt:minisign-secret-key:1\n")
    for args in (("init", "-q", "-b", "main"), ("add", "-A")):
        subprocess.run(["git", "-C", str(repo), *args], capture_output=True, check=True)
    rc, out = main_in(repo)
    record("main(): a tracked nested `.gitleaksignore` -> exit 1 naming it", rc == 1 and "sub/.gitleaksignore" in out)
    subprocess.run(["git", "-C", str(repo), "rm", "-q", "--cached", "sub/.gitleaksignore"], capture_output=True, check=True)
    rc, out = main_in(repo)
    record("main(): the same file untracked -> exit 0 (the tracked list is this catcher's; run-gitleaks refuses the "
           "working-tree file)", rc == 0)
    rc, out = main_in(Path(td) / "no-such-repository")
    record("main(): the tracked-file list cannot be read -> exit 2 (fail closed)", rc == 2 and "git ls-files" in out)

# the REAL committed config + baseline must pass (exit 0)
rc = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True, text=True, encoding="utf-8", errors="replace").returncode
record("committed .gitleaks.toml + baseline pass the growth-guard (exit 0)", rc == 0)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-gitleaks-allowlist] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
