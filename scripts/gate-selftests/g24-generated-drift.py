#!/usr/bin/env python3
"""g24-generated-drift.py - G24 self-test for check-generated-drift (P0.3.9, G19).

Proves the drift-check framework: the structural validators (json / text / ts-bindings) catch an empty
or malformed generated artifact; and check_artifact (over a real temp git repo) returns 0 when
regeneration leaves the committed file byte-identical, 1 when regeneration DRIFTS it / the regen command
fails / structural sanity fails, and 2 when the artifact path is missing after regen. main() is
target-absent (empty registry) today. stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import subprocess
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-generated-drift"
_loader = importlib.machinery.SourceFileLoader("cgd", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cgd", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


# --- structural validators (pure) -------------------------------------------------------------
record("json validator: valid + non-empty -> OK", m.validate_json_nonempty('{"a": 1}') is None)
record("json validator: not JSON -> caught", m.validate_json_nonempty("not json") is not None)
record("json validator: empty {} -> caught", m.validate_json_nonempty("{}") is not None)
record("json validator: [] / null -> caught",
       m.validate_json_nonempty("[]") is not None and m.validate_json_nonempty("null") is not None)
record("text validator: non-blank -> OK", m.validate_text_nonempty("Usage: convertia ...") is None)
record("text validator: blank -> caught", m.validate_text_nonempty("   \n  ") is not None)
record("ts-bindings validator: has export -> OK", m.validate_ts_bindings("export const commands = {}") is None)
record("ts-bindings validator: no export (truncated emit) -> caught",
       m.validate_ts_bindings("const commands = {}") is not None)
record("ts-bindings validator: empty -> caught", m.validate_ts_bindings("") is not None)


# --- check_artifact over a real temp git repo -------------------------------------------------
def _git(repo: Path, *a: str) -> None:
    subprocess.run(["git", "-C", str(repo), *a], capture_output=True, text=True, check=True)


def _write_argv(path: str, content: str) -> list[str]:
    # a regen command that (re)writes `path` with `content`, run with cwd=repo
    return [sys.executable, "-c",
            f"import io; io.open({path!r},'w',encoding='utf-8').write({content!r})"]


with tempfile.TemporaryDirectory() as td:
    repo = Path(td)
    _git(repo, "init", "-q", "-b", "main")
    _git(repo, "config", "user.email", "t@t.t")
    _git(repo, "config", "user.name", "t")
    (repo / "gen.json").write_text('{"x": 1}\n', encoding="utf-8")
    _git(repo, "add", "gen.json")
    _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "init")
    orig_root = m.ROOT
    m.ROOT = repo
    try:
        # regen reproduces the committed bytes exactly -> no drift -> 0
        code, _ = m.check_artifact({"name": "g", "regen": _write_argv("gen.json", '{"x": 1}\n'), "path": "gen.json", "validator": "json"})
        record("check_artifact: regen byte-identical -> 0 (no drift)", code == 0)
        # regen changes the bytes -> drift -> 1
        code, msg = m.check_artifact({"name": "g", "regen": _write_argv("gen.json", '{"x": 2}\n'), "path": "gen.json", "validator": "json"})
        record("check_artifact: regen DRIFTS the file -> 1", code == 1 and "DRIFTED" in msg)
        _git(repo, "checkout", "--", "gen.json")  # restore for the next legs
        # a CLEAN but structurally-empty committed artifact (regen reproduces {} -> no drift) -> sanity fails
        (repo / "empty.json").write_text("{}\n", encoding="utf-8")
        _git(repo, "add", "empty.json"); _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "empty")
        code, msg = m.check_artifact({"name": "g", "regen": _write_argv("empty.json", "{}\n"), "path": "empty.json", "validator": "json"})
        record("check_artifact: a clean-but-empty {} artifact -> 1 (structural sanity, not drift)",
               code == 1 and "sanity" in msg)
        # a failing regen command -> 1
        code, msg = m.check_artifact({"name": "g", "regen": [sys.executable, "-c", "import sys; sys.exit(3)"], "path": "gen.json", "validator": "json"})
        record("check_artifact: a failing regen command -> 1", code == 1 and "failed" in msg)
        # a path missing after regen -> 2 (misconfigured)
        code, _ = m.check_artifact({"name": "g", "regen": [sys.executable, "-c", "pass"], "path": "nope.json", "validator": "json"})
        record("check_artifact: artifact path missing after regen -> 2 (misconfig)", code == 2)
        # an UNTRACKED artifact (regen writes a never-committed file) -> 2, not a false-green 0
        code, msg = m.check_artifact({"name": "g", "regen": _write_argv("untracked.json", '{"x": 1}\n'), "path": "untracked.json", "validator": "json"})
        record("check_artifact: an UNTRACKED artifact -> 2 (not a false-green; git diff is blind to it)",
               code == 2 and "not git-tracked" in msg)
        # an unknown validator key -> 2 (registry misconfig)
        code, msg = m.check_artifact({"name": "g", "regen": _write_argv("gen.json", '{"x": 1}\n'), "path": "gen.json", "validator": "xml"})
        record("check_artifact: an unknown validator key -> 2 (misconfig)", code == 2 and "validator" in msg)
    finally:
        m.ROOT = orig_root

# --- main() target-absent today ---------------------------------------------------------------
record("main() exits 0 today (empty ARTIFACTS registry -> target-absent until P1)", m.main() == 0)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-generated-drift] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
