#!/usr/bin/env python3
"""g24-generated-drift.py - G24 self-test for check-generated-drift (P0.3.9, G19).

Proves the drift-check framework: the structural validators (json / text / ts-bindings) catch an empty
or malformed generated artifact; and check_artifact (over a real temp git repo) returns 0 when
regeneration leaves the committed file byte-identical, 1 when regeneration DRIFTS it / the regen command
fails / structural sanity fails, and 2 when the artifact path is missing after regen. main() over the
populated registry (the ts-bindings artifact since P1.28) skips gracefully where the regen toolchain is
absent in this plane (plane-independent). stdlib-only. Exit 0 = all held; 1 = a self-test failed.
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
    subprocess.run(["git", "-C", str(repo), *a], capture_output=True, text=True, encoding="utf-8", errors="replace", check=True)


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

# --- main() with the populated P1 registry: plane-independent (toolchain absent -> graceful skip) ----
# Since P1.28 the ARTIFACTS registry is non-empty (the ts-bindings artifact). main()'s exit must not
# depend on whether `cargo` happens to be installed in THIS plane (GitHub runners ship it, the L4
# gate-tooling plane may not), so we force the regen-toolchain pre-check to skip and assert main()
# still exits 0 — i.e. a populated registry skips gracefully where the toolchain is absent (the live
# regen+diff enforces at L1/L2 + the equipped Rust CI leg; the planted-violation arming is P1.62.2).
def _main_skips_when_toolchain_absent():
    saved = m.shutil.which
    m.shutil.which = lambda tool: None
    try:
        return m.main()
    finally:
        m.shutil.which = saved


record("main() exits 0 when the registered artifacts' regen toolchain is absent in this plane "
       "(populated ARTIFACTS registry since P1.28 -> graceful skip, plane-independent)",
       _main_skips_when_toolchain_absent() == 0)

# --- the P1-runway fix: an artifact whose regen toolchain is absent in this plane -> check_artifact
# SKIPS (0), not a FileNotFoundError crash (regen+diff enforces where the toolchain is present) ---------
def _artifact_skip_when_toolchain_absent():
    saved = m.shutil.which
    m.shutil.which = lambda tool: None
    try:
        return m.check_artifact({"name": "b", "path": "x.ts", "regen": ["cargo", "xtask", "codegen"],
                                 "validator": "ts-bindings"})
    finally:
        m.shutil.which = saved


_gd_code, _gd_msg = _artifact_skip_when_toolchain_absent()
record("check_artifact(): regen toolchain absent in this plane -> SKIP (0), not a FileNotFoundError "
       "crash (P1-runway fix; regen+diff enforces where the toolchain is present)", _gd_code == 0)


# the which-found-but-unrunnable anomaly (wrong arch / not executable / exec race) -> FAIL-CLOSED (1),
# NOT a skip: a genuinely-absent tool is the which() pre-check's skip; an on-PATH-but-broken tool is a
# real anomaly (G1 dual-review [P2]: the OSError path must fail-closed, not silently skip)
def _artifact_fail_when_exec_anomaly():
    saved = (m.shutil.which, m.subprocess.run)
    m.shutil.which = lambda tool: "/usr/bin/" + tool          # the tool IS on PATH...
    def _boom(*a, **k):
        raise OSError("Exec format error")                    # ...but exec fails
    m.subprocess.run = _boom
    try:
        return m.check_artifact({"name": "b", "path": "x.ts", "regen": ["cargo", "xtask"],
                                 "validator": "ts-bindings"})
    finally:
        m.shutil.which, m.subprocess.run = saved


_fc_code, _ = _artifact_fail_when_exec_anomaly()
record("check_artifact(): regen tool on PATH but exec fails -> FAIL (1) fail-closed, not a skip "
       "(the which-found-but-unrunnable anomaly; G1 review P2)", _fc_code == 1)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-generated-drift] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
