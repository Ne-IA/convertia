#!/usr/bin/env python3
"""g24-osv-advisories.py - G24 self-test for check-osv-advisories (P4.96 and push 4, G17's OSV half).

Replays three incidents against ONE-RECORD OSV databases built in a temp dir from committed records, so no planted
positive depends on a live database: the npm undici GHSA-4cwx-7wf7-3272 (CVE-2026-13697; affected 7.0.0 to 7.29.0
and 8.0.0 to 8.9.0, the fixed versions excluded; osv-fixture-undici.json), the PyPI certifi GHSA-6q48-6c7w-9gw2
(PYSEC-2026-4320, the root-CA advisory of 2026-10-20 that reached this repository only as a Dependabot alert;
osv-fixture-pypi.json) and the pnpm lockfile-integrity bypass GHSA-7vhp-vf5g-r2fw (CVE-2025-69263, fixed in 10.26.0;
osv-fixture-pnpm.json), plus a PyYAML record (GHSA-8q59-q68h-6hv4, fixed in 5.4; osv-fixture-pyyaml.json) for a
mixed-case PyPI name.

Pure legs (every plane): loading the script runs no entry point; the committed ignore file, its count and its
held_by pins against requirements-ci.txt; the ignore-file grammar (held_by included) and its growth guard; the
verdict evaluation (id and alias matches, held_by on package and version, a PyPI name in its PEP 503 form, stale
entries, the stale rule suspended); the scanner exit-code set and the JSON reader; the lockfile census (the
lockfile-name rules on the basename, case-insensitive, requirements/dev.txt; an unread lockfile; the declarations,
the Rust half's lock set read from check-rust-lint-contract and bound to that module's value, the presence list)
and `git ls-files` in a scratch repository; the packageManager pin (the plain and the +sha512 form, a range, another
manager, an absent value) and its one-package lock; the per-ecosystem database test; the database-age floor (a
fresh record, a failed or old or future refresh, a missing or malformed record, a swapped zip); the argv and child
environment; the scanner lookup; the mode postures that stop before the scanner runs; the children each mode
starts (argv over every source and the pin lock, its own temp cwd, the closed proxy for the check only, the
bound; a recording fake stands in for subprocess and net-retry's run_bounded) and the record the refresh writes; the
download's retry classifier over osv-scanner's measured words and the retry policy around it on a fake clock (one
transient failure then a download green, a transient failure through the whole window red, a final failure red at
once, an attempt past its bound retried); the check's fail-closed
arms after the database test; main(): each flag reaches its mode; and the script run as a process from a scratch
git repository: each mode's exit code and stream, which lefthook and ci.yml read. A check leg whose contract holds
in every mode runs in both check modes (CHECK_MODES), so an exit 0 or a child that only one mode gets reds a leg.
Whether this plane has an osv-scanner on PATH is never trusted: every leg that runs without the pinned binary plants
one first on PATH. Binary legs run the PINNED osv-scanner from .gate-tools/bin and skip with a notice when it is
absent (the g24-gitleaks precedent); the L4 gate-tooling job installs it on all three OS before the canary.
stdlib-only. Exit 0 = all held (binary legs possibly skipped with the notice); 1 = a self-test failed.
"""
import contextlib
import hashlib
import importlib.machinery
import importlib.util
import io
import json
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import types
import zipfile
from collections.abc import Callable
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")
# A git hook exports GIT_* variables (an absolute GIT_DIR in a linked worktree): under them, a git run from this
# file in a temp directory - the census legs' scratch repositories, the gate's own `git ls-files` there - acts on the
# hooked repository. All but GIT_EXEC_PATH go.
for _k in [k for k in os.environ if k.startswith("GIT_") and k != "GIT_EXEC_PATH"]:
    os.environ.pop(_k)

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check-osv-advisories"
RUST_HALF = ROOT / "scripts" / "check-rust-lint-contract"
FX = ROOT / "scripts" / "gate-selftests"
FIXTURES = {   # file: (id, alias, ecosystem, package)
    "osv-fixture-undici.json": ("GHSA-4cwx-7wf7-3272", "CVE-2026-13697", "npm", "undici"),
    "osv-fixture-pypi.json": ("GHSA-6q48-6c7w-9gw2", "PYSEC-2026-4320", "PyPI", "certifi"),
    "osv-fixture-pnpm.json": ("GHSA-7vhp-vf5g-r2fw", "CVE-2025-69263", "npm", "pnpm"),
    "osv-fixture-pyyaml.json": ("GHSA-8q59-q68h-6hv4", "CVE-2020-14343", "PyPI", "pyyaml"),
}
GHSA, CVE = "GHSA-4cwx-7wf7-3272", "CVE-2026-13697"
CERTIFI, PYSEC = "GHSA-6q48-6c7w-9gw2", "PYSEC-2026-4320"
PNPM_ID, PYYAML_ID = "GHSA-7vhp-vf5g-r2fw", "GHSA-8q59-q68h-6hv4"
# check()'s two modes, (leg label, require_db): the default is lefthook's pre-push leg, --require-db ci.yml's check step
CHECK_MODES = (("default", False), ("--require-db", True))
results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def _load(path: Path, name: str) -> tuple[types.ModuleType, bool]:
    loader = importlib.machinery.SourceFileLoader(name, str(path))
    mod = importlib.util.module_from_spec(importlib.util.spec_from_loader(name, loader))
    try:
        loader.exec_module(mod)
    except SystemExit:                    # an entry point run on import would end this self-test with main()'s exit code
        return mod, True
    return mod, False


m, _exited_on_import = _load(SCRIPT, "coa")
record("loading check-osv-advisories as a module runs no entry point (its `__main__` guard holds, no SystemExit)",
       not _exited_on_import)

# a minimal pnpm v9 lockfile pinning one undici version (the shape osv-scanner 2.6.0 reads; measured)
_LOCK = """lockfileVersion: '9.0'

settings:
  autoInstallPeers: true
  excludeLinksFromLockfile: false

importers:

  .:
    dependencies:
      undici:
        specifier: {v}
        version: {v}

packages:

  undici@{v}:
    resolution: {{integrity: sha512-AAAA}}
    engines: {{node: '>=20.18.1'}}

snapshots:

  undici@{v}: {{}}
"""
_EMPTY_LOCK = ("lockfileVersion: '9.0'\n\nsettings:\n  autoInstallPeers: true\n  excludeLinksFromLockfile: false\n"
               "\nimporters:\n\n  .: {}\n")
_HASH = "    --hash=sha256:" + "0" * 64 + "\n"
_RUST_HALF_STUB = 'CARGO_AUDIT_LOCKS = ("Cargo.lock", "fuzz/Cargo.lock")\n'
_PIN = "pnpm@10.34.7"


def write(path: Path, text: str) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("utf-8"))
    return path


def req(*pins: str) -> str:
    """A hash-pinned requirements body: one `name==version \\` line and its --hash line per pin."""
    return "".join(f"{p} \\\n{_HASH}" for p in pins)


def package_json(pin: str | None) -> str:
    return json.dumps({"name": "x", "private": True, **({"packageManager": pin} if pin is not None else {})}) + "\n"


def scratch_root(d: Path, *, undici: str = "7.29.1", ci: tuple[str, ...] = ("certifi==2026.10.23",),
                 yamllint: tuple[str, ...] = ("pyyaml==6.0.3",), pin: str | None = _PIN,
                 extra: dict[str, str] | None = None) -> tuple[Path, list[str]]:
    """A scratch checkout with every source the census requires: (the root, its tracked paths)."""
    files = {"package.json": package_json(pin), "pnpm-lock.yaml": _LOCK.format(v=undici),
             "requirements-ci.txt": req(*ci), "requirements-yamllint.txt": req(*yamllint),
             "Cargo.lock": "version = 4\n", "fuzz/Cargo.lock": "version = 4\n",
             "tests/g53-fixture/Cargo.lock": "version = 4\n", "src-tauri/engines.lock": "",
             "scripts/check-rust-lint-contract": _RUST_HALF_STUB, **(extra or {})}
    for rel, text in files.items():
        write(d / rel, text)
    return d, sorted(files)


def fixture_db(d: Path, *names: str, subdir: str = "osv-scalibr") -> Path:
    """<d>/<subdir>/<ecosystem>/all.zip holding the named committed fixtures; returns the cache dir `d`."""
    by_eco: dict[str, list[str]] = {}
    for n in names:
        by_eco.setdefault(FIXTURES[n][2], []).append(n)
    for eco, files in by_eco.items():
        z = d / subdir / eco / "all.zip"
        z.parent.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(z, "w", zipfile.ZIP_DEFLATED) as zf:
            for n in files:
                zf.writestr(f"{FIXTURES[n][0]}.json", (FX / n).read_bytes())
    return d


def zip_at(d: Path, eco: str, members: dict[str, bytes], compression: int = zipfile.ZIP_DEFLATED) -> Path:
    z = m.db_zip(d, eco)
    z.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(z, "w", compression) as zf:
        for name, data in members.items():
            zf.writestr(name, data)
    return d


def fresh_record(db_dir: Path, ecosystems: tuple[str, ...] = ("npm", "PyPI"), *, rc: int | None = 0,
                 now: float | None = None) -> Path:
    """Write the record a refresh at `now` (default: this moment) with exit `rc` would leave; returns `db_dir`."""
    rec = m.refresh_record(db_dir, ecosystems, rc, time.time() if now is None else now)
    write(db_dir / m.REFRESH_STATE, json.dumps(rec) + "\n")
    return db_dir


def ignores_text(*entries: tuple[str, ...]) -> str:
    """An osv-ignores.toml body: `schema_version = 1` and one [[ignore]] table per (id, reason[, held_by])."""
    body = "schema_version = 1\n"
    for e in entries:
        body += f'\n[[ignore]]\nid = "{e[0]}"\nreason = "{e[1]}"\n' + (f'held_by = "{e[2]}"\n' if len(e) > 2 else "")
    return body


def ignores_file(d: Path, *entries: tuple[str, ...]) -> Path:
    return write(d / "osv-ignores.toml", ignores_text(*entries))


def above_count() -> tuple[tuple[str, str], ...]:
    """EXPECTED_IGNORE_COUNT + 1 distinct valid entries: one above the frozen count, whatever its value."""
    return tuple((f"GHSA-test-{k:04d}", "reviewed") for k in range(m.EXPECTED_IGNORE_COUNT + 1))


@contextlib.contextmanager
def expected_count(n: int):
    """Raise the module's frozen count for one leg (a legitimate owner-acked growth), restored afterwards."""
    old = m.EXPECTED_IGNORE_COUNT
    m.EXPECTED_IGNORE_COUNT = n
    try:
        yield
    finally:
        m.EXPECTED_IGNORE_COUNT = old


def problems_of(text: str) -> list[str]:
    try:
        return m.load_ignores(text, limit=5)[1]
    except Exception as e:                       # a crash is a red leg with its reason, never a pass
        return [f"<load_ignores raised {type(e).__name__}: {e}>"]


def after(argv: list[str], flag: str) -> str | None:
    """The argument that follows `flag` in `argv`, or None when the flag is absent or last."""
    return argv[argv.index(flag) + 1] if flag in argv[:-1] else None


def lockfiles(argv: list[str]) -> list[str]:
    return [argv[i + 1] for i, a in enumerate(argv[:-1]) if a == "--lockfile"]


@contextlib.contextmanager
def osv_scanner_on_path():
    """An empty `osv-scanner` and `osv-scanner.exe` (execute bits set) in a fresh temp dir placed first on PATH (a child
    process inherits it); yields the dir; PATH is restored and the dir removed afterwards (mock the probe, don't trust
    the plane)."""
    with tempfile.TemporaryDirectory(prefix="on-path-") as td:
        d = Path(td)
        for name in ("osv-scanner", "osv-scanner.exe"):
            (d / name).write_bytes(b"")
            (d / name).chmod(0o755)
        old = os.environ.get("PATH")
        os.environ["PATH"] = os.pathsep.join([str(d), old] if old else [str(d)])
        try:
            yield d
        finally:
            if old is None:
                os.environ.pop("PATH", None)
            else:
                os.environ["PATH"] = old


def run_main(argv: list[str], *, binary: Path | None, db_dir: Path, root: Path) -> tuple[int | str | None, str]:
    """m.main(argv) with the scanner lookup answering `binary`, DB_DIR = `db_dir`, ROOT = `root` and IGNORES its
    scripts/osv-ignores.toml, an osv-scanner planted first on PATH, all restored afterwards: (the exit code,
    argparse's SystemExit code included; stdout + stderr). A crash is a red leg with its reason, never a pass."""
    saved = (m.osv_binary, m.DB_DIR, m.ROOT, m.IGNORES)

    def lookup(bin_dir: Path = m.BIN_DIR) -> Path | None:
        return binary

    m.osv_binary, m.DB_DIR, m.ROOT, m.IGNORES = lookup, db_dir, root, root / "scripts" / "osv-ignores.toml"
    out = io.StringIO()
    rc: int | str | None = None
    try:
        with osv_scanner_on_path(), contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
            rc = m.main(argv)
    except SystemExit as e:
        rc = e.code
    except Exception as e:                       # a crash is a red leg with its reason, never a pass
        out.write(f"<main raised {type(e).__name__}: {e}>")
    finally:
        m.osv_binary, m.DB_DIR, m.ROOT, m.IGNORES = saved
    return rc, out.getvalue()


def guarded(fn: Callable[..., tuple[int, list[str]]], **kw: object) -> tuple[int | None, list[str]]:
    """fn(**kw) -> (exit code, report lines); an exception it raises becomes (None, [its reason])."""
    try:
        return fn(**kw)
    except Exception as e:                       # a crash is a red leg with its reason, never a pass
        return None, [f"<{fn.__name__} raised {type(e).__name__}: {e}>"]


def without_binary(fn: Callable[..., tuple[int, list[str]]], **kw: object) -> tuple[int | None, list[str]]:
    """guarded(fn, binary=None, **kw) with an osv-scanner planted first on PATH."""
    with osv_scanner_on_path():
        return guarded(fn, binary=None, **kw)


def state_of(d: Path, eco: str = "npm") -> str:
    try:
        return m.db_state(d, eco)
    except Exception as e:                       # a crash is a red leg with its reason, never a pass
        return f"<db_state raised {type(e).__name__}: {e}>"


def clean_report(argv: list[str]) -> str:
    """The `--all-packages` report of a clean scan of argv's inputs: each source with one package and no advisory."""
    return json.dumps({"results": [{"source": {"path": lf.split(":", 1)[1], "type": "lockfile"}, "packages": [
        {"package": {"name": "p", "version": "1", "ecosystem": "npm"}, "vulnerabilities": []}]}
        for lf in lockfiles(argv)]})


@contextlib.contextmanager
def recording_subprocess(*, stdout: str | None = None, returncode: int = 0, raises: BaseException | None = None,
                         downloads: list[tuple[int, str]] | None = None):
    """The gate module's `subprocess` and net-retry's run_bounded (the refresh's download child) swapped for a recorder
    (restored afterwards), so no scanner child starts (`git` still runs for real). Each scanner call is recorded with
    what its cwd, its --config file and its pin lock hold at call time (the child's temp dir is gone once the call
    returns) and its bound (`timeout`: subprocess.run's, or run_bounded's seconds); it returns `returncode` with
    `stdout` (by default clean_report() of its argv), or raises `raises`. A download call takes the next (exit code,
    stderr) of `downloads` instead while any is left. Yields the list of calls."""
    calls: list[dict[str, object]] = []
    real, real_net = m.subprocess, m._NET
    left = list(downloads or [])

    def run(argv: list[str], **kw: object) -> subprocess.CompletedProcess:
        if argv and argv[0] == "git":
            return real.run(argv, **kw)
        cwd, env, cfg = kw.get("cwd"), kw.get("env"), after(list(argv), "--config")
        pins = [lf.split(":", 1)[1] for lf in lockfiles(list(argv)) if f"{m._PIN_DIR}" in lf]
        calls.append({"argv": list(argv), "cwd": cwd, "timeout": kw.get("timeout"),
                      "env": dict(env) if isinstance(env, dict) else {},
                      "cwd_files": (sorted(p.name for p in Path(cwd).iterdir())
                                    if isinstance(cwd, (str, os.PathLike)) else None),
                      "config": cfg, "config_bytes": Path(cfg).read_bytes() if cfg and Path(cfg).is_file() else None,
                      "pin_text": Path(pins[0]).read_text(encoding="utf-8") if pins and Path(pins[0]).is_file()
                      else None})
        if raises is not None:
            raise raises
        return subprocess.CompletedProcess(argv, returncode, clean_report(list(argv)) if stdout is None else stdout, "")

    def run_bounded(argv: list[str], seconds: float, *, env: dict[str, str] | None = None,
                    cwd: object = None) -> subprocess.CompletedProcess:
        if left:
            run(argv, cwd=cwd, env=env, timeout=seconds)
            rc, err = left.pop(0)
            return subprocess.CompletedProcess(argv, rc, "", err)
        return run(argv, cwd=cwd, env=env, timeout=seconds)

    m.subprocess = types.SimpleNamespace(run=run, TimeoutExpired=subprocess.TimeoutExpired)
    m._NET = types.SimpleNamespace(**{**vars(real_net), "run_bounded": run_bounded})
    try:
        yield calls
    finally:
        m.subprocess, m._NET = real, real_net


def fake_retrier(log: list[str] | None = None) -> tuple[object, list[float]]:
    """A net-retry Retrier on a fake clock that each pause advances (no leg waits), its jitter at the low end of each
    pause and its log lines into `log`: (the Retrier, the pauses it slept)."""
    now, pauses = [0.0], []

    def sleep(sec: float) -> None:
        pauses.append(sec)
        now[0] += sec

    return m._NET.Retrier("G17 OSV", clock=lambda: now[0], sleep=sleep, rng=lambda: 0.0,
                          log=(log if log is not None else []).append), pauses


def child_ok(call: dict[str, object], argv_of: Callable[..., list[str]], binary: Path, root: Path,
             sources: list[str], pin: str | None) -> bool:
    """The recorded child ran argv_of(binary, <the sources' parsers and absolute paths, then the pin lock>, <its
    --config file>) from its own temp dir, which held only that empty file and the pin lock's directory (holding the
    pin's one-package lock), and is gone once the call returned."""
    cwd, cfg, argv = call.get("cwd"), call.get("config"), call.get("argv")
    if not (isinstance(cwd, (str, os.PathLike)) and isinstance(cfg, str) and isinstance(argv, list)):
        return False
    inputs = [(m.osv_parser(s)[0], root / s) for s in sources]
    if pin is not None:
        inputs.append(("package-lock.json", Path(cwd) / m._PIN_DIR / "package-lock.json"))
    want_files = sorted([Path(cfg).name] + ([m._PIN_DIR] if pin is not None else []))
    return (argv == argv_of(binary, inputs, Path(cfg)) and Path(cfg).parent == Path(cwd)
            and call.get("cwd_files") == want_files and call.get("config_bytes") == b""
            and call.get("pin_text") == (m.pin_lock(pin) if pin is not None else None) and not Path(cwd).exists())


def git_repo(d: Path, files: list[str]) -> bool:
    """`git init` + `git add` of `files` in `d` (no commit, no identity needed); True when both exit 0."""
    env = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)
    ok = True
    for argv in (["git", "init", "-q", str(d)], ["git", "-C", str(d), "add", "--", *files]):
        try:
            ok &= subprocess.run(argv, capture_output=True, timeout=60, env=env).returncode == 0
        except (OSError, subprocess.TimeoutExpired):
            return False
    return ok


# --- the committed ignore file, its pins and the fixture records -------------------------------------------------
_real_ignores, _real_problems = m.load_ignores(m.IGNORES.read_text(encoding="utf-8"))
record("the committed scripts/osv-ignores.toml parses clean and holds exactly EXPECTED_IGNORE_COUNT entries",
       _real_problems == [] and len(_real_ignores) == m.EXPECTED_IGNORE_COUNT)
# [Test-Change: push 4 C2a - old-obsolete+new-correct, build-gates G17 row] old: the pin read 1, the minimatch entry
# alone, while the scan read pnpm-lock.yaml only. new: 6, the minimatch entry and the five advisories the pinned
# Semgrep holds in requirements-ci.txt (mcp 1.29.0 four times, opentelemetry-api 1.37.0 once), which the owner-acked
# commit that adds the PyPI sources accepts, with check-osv-advisories' count moved 1 -> 6 in the same commit;
# moving the value stays an owner-acked edit of this pin.
record("EXPECTED_IGNORE_COUNT is frozen at 6 (six accepted advisories; growth is an owner-acked edit of this pin)",
       m.EXPECTED_IGNORE_COUNT == 6)
_ci_pins = dict(re.findall(r"(?im)^([a-z0-9][a-z0-9._-]*)==([^\s\\;]+)", (ROOT / "requirements-ci.txt").read_text(
    encoding="utf-8")))
_ci_pins = {m._pypi_norm(k): v for k, v in _ci_pins.items()}
_held = [e.held_by for e in _real_ignores.values() if e.held_by is not None]
record("the committed held_by pins: five, each the vulnerable package's own exact pin in requirements-ci.txt, none "
       "naming Semgrep (plan-lint check 32 keeps its version in requirements-ci.txt alone)",
       len(_held) == 5 and all(_ci_pins.get(m._pypi_norm(n)) == v for n, v in _held)
       and not any(m._pypi_norm(n) == "semgrep" for n, _ in _held))
for _name, (_id, _alias, _eco, _pkg) in FIXTURES.items():
    _raw = (FX / _name).read_bytes()
    _fx = json.loads(_raw)
    record(f"the fixture {_name} is the trimmed {_id} record (six keys, the id, the {_alias} alias, {_eco} {_pkg}), "
           "LF-only",
           sorted(_fx) == ["affected", "aliases", "id", "modified", "published", "schema_version"]
           and _fx["id"] == _id and _alias in _fx["aliases"]
           and {(a["package"]["ecosystem"], a["package"]["name"]) for a in _fx["affected"]} == {(_eco, _pkg)}
           and b"\r" not in _raw and _raw.endswith(b"\n"))

# --- load_ignores: the grammar ------------------------------------------------------------------------------------
# [Test-Change: push 4 C2a - old-obsolete+new-correct, build-gates G17 row] old: a well-formed entry loaded as {id: reason}; new: as
# {id: Ignore(reason, held_by)}, the holder None without `held_by` - the same entry, the value now carries the pin.
_ok_text = f'schema_version = 1\n\n[[ignore]]\nid = "{CVE}"\nreason = "not reachable"\n'
record("load_ignores: a well-formed entry within the limit -> {id: Ignore(reason, no holder)}, no problem",
       m.load_ignores(_ok_text, limit=1) == ({CVE: m.Ignore("not reachable", None)}, []))
record("load_ignores: an entry with held_by -> its (package, version) pair",
       m.load_ignores(ignores_text((PYSEC, "r", "certifi@2026.7.22")), limit=1)
       == ({PYSEC: m.Ignore("r", ("certifi", "2026.7.22"))}, []))
record("load_ignores: a scoped npm held_by splits at its last @",
       m.load_ignores(ignores_text((GHSA, "r", "@scope/pkg@1.2.3")), limit=1)[0][GHSA].held_by == ("@scope/pkg", "1.2.3"))
for _bad in ("certifi", "@2026.7.22", "certifi@", "certifi 2@1", "certifi@@1"):
    record(f"load_ignores: held_by {_bad!r} -> problem naming `held_by`",
           any("`held_by` must be" in p for p in problems_of(ignores_text((GHSA, "r", _bad)))))
record("load_ignores: a held_by that is not a string -> problem naming `held_by`",
       any("`held_by` must be" in p for p in problems_of(
           f'schema_version = 1\n[[ignore]]\nid = "{GHSA}"\nreason = "r"\nheld_by = 7\n')))
record("load_ignores: an entry without a reason -> problem naming `reason`",
       any("`reason` is required" in p for p in problems_of(f'schema_version = 1\n[[ignore]]\nid = "{GHSA}"\n')))
record("load_ignores: a blank reason -> problem naming `reason`",
       any("`reason` is required" in p
           for p in problems_of(f'schema_version = 1\n[[ignore]]\nid = "{GHSA}"\nreason = "   "\n')))
record("load_ignores: a duplicate id -> problem",
       any("duplicate id" in p for p in problems_of(
           f'schema_version = 1\n[[ignore]]\nid = "{GHSA}"\nreason = "a"\n[[ignore]]\nid = "{GHSA}"\nreason = "b"\n')))
# [Test-Change: push 4 C2a - old-obsolete+new-correct, build-gates G17 row] old: `owner_ack` was the planted unknown key and
# the message said "exactly `id` and `reason`"; new: `held_by` is a known key, so `owner_ack` and the design's
# untaken `expires` are planted, and the message names the optional `held_by`.
record("load_ignores: an unknown key (a per-entry `owner_ack`, or an `expires`) -> problem",
       all(any("unknown key" in p and k in p for p in problems_of(
           f'schema_version = 1\n[[ignore]]\nid = "{GHSA}"\nreason = "a"\n{k} = "yes"\n')) for k in ("owner_ack", "expires")))
record("load_ignores: an id carrying white space -> problem",
       any("`id` must be one advisory id" in p
           for p in problems_of('schema_version = 1\n[[ignore]]\nid = "GHSA x"\nreason = "a"\n')))
record("load_ignores: an id that is not a string -> problem",
       any("`id` must be one advisory id" in p for p in problems_of('schema_version = 1\n[[ignore]]\nid = 7\nreason = "a"\n')))
record("load_ignores: bad TOML -> problem", any("not valid TOML" in p for p in problems_of("schema_version = \n")))
record("load_ignores: schema_version missing -> problem", any("schema_version must be 1" in p for p in problems_of("")))
record("load_ignores: schema_version 2 -> problem", any("schema_version must be 1" in p for p in problems_of("schema_version = 2\n")))
record("load_ignores: an unknown top-level key -> problem",
       any("unknown top-level key" in p for p in problems_of('schema_version = 1\nallow_all = true\n')))
record("load_ignores: a plain [ignore] table instead of [[ignore]] -> problem",
       any("array of tables" in p for p in problems_of('schema_version = 1\n[ignore]\nid = "x"\nreason = "y"\n')))
_over = ignores_text(*above_count())
record("load_ignores: one valid entry above EXPECTED_IGNORE_COUNT -> problem naming the pin to raise",
       any("raise EXPECTED_IGNORE_COUNT in scripts/check-osv-advisories" in p for p in m.load_ignores(_over)[1]))
with expected_count(m.EXPECTED_IGNORE_COUNT + 1):
    _raised = m.load_ignores(_over)[1]
record("load_ignores: the same entries with EXPECTED_IGNORE_COUNT raised by one -> clean (the reviewed growth path)",
       _raised == [])

# --- evaluate: findings against the ignore ids ---------------------------------------------------------------------
_incident = [m.Finding(GHSA, (CVE,), "undici", "7.28.0", "npm", "pnpm-lock.yaml")]
_certifi = [m.Finding(CERTIFI, (PYSEC,), "certifi", "2026.7.22", "PyPI", "requirements-ci.txt")]
_unignored = m.evaluate(_incident, {})
record("evaluate: an advisory no entry names -> problem naming the id, package@version and the source",
       len(_unignored) == 1 and GHSA in _unignored[0] and "undici@7.28.0 (pnpm-lock.yaml)" in _unignored[0])
record("evaluate: accepted by its OSV id -> clean", m.evaluate(_incident, {GHSA: m.Ignore("r", None)}) == [])
record("evaluate: accepted by its CVE alias -> clean", m.evaluate(_incident, {CVE: m.Ignore("r", None)}) == [])
record("evaluate: held_by on the finding's package and version -> clean",
       m.evaluate(_certifi, {PYSEC: m.Ignore("r", ("certifi", "2026.7.22"))}) == [])
_moved = m.evaluate(_certifi, {PYSEC: m.Ignore("r", ("certifi", "2026.7.21"))})
record("evaluate: held_by on another version (the holder moved) -> the finding reds as accepted only on another pin, "
       "and the entry is stale",
       len(_moved) == 2 and "accepted only on another pin" in _moved[0] and "stale ignore" in _moved[1])
record("evaluate: held_by on another package (the id reached a second package) -> problem",
       any("accepted only on another pin" in p
           for p in m.evaluate(_certifi, {CERTIFI: m.Ignore("r", ("requests", "2026.7.22"))})))
record("evaluate: a PyPI held_by in another spelling (PEP 503: case, `_`, `.`) -> clean",
       m.evaluate([m.Finding(PYYAML_ID, (), "PyYAML", "5.3.1", "PyPI", "r.txt")],
                  {PYYAML_ID: m.Ignore("r", ("pyyaml", "5.3.1"))}) == []
       and m.evaluate([m.Finding("X-1", (), "zope.interface", "1", "PyPI", "r.txt")],
                      {"X-1": m.Ignore("r", ("Zope_Interface", "1"))}) == [])
record("evaluate: an npm held_by in another case -> problem (npm names are compared exactly)",
       any("accepted only on another pin" in p
           for p in m.evaluate(_incident, {GHSA: m.Ignore("r", ("Undici", "7.28.0"))})))
_stale = m.evaluate([], {GHSA: m.Ignore("r", None)})
record("evaluate: an entry matching no finding -> stale problem", len(_stale) == 1 and "stale ignore" in _stale[0])
record("evaluate: judge_stale=False (an ecosystem skipped) -> no stale problem",
       m.evaluate([], {GHSA: m.Ignore("r", None)}, judge_stale=False) == [])
record("evaluate: the same advisory reported twice is one problem", len(m.evaluate(_incident + _incident, {})) == 1)

# --- interpret: the exit code before the JSON ----------------------------------------------------------------------
_CLEAN_JSON = '{"results": []}'
_finding_json = json.dumps({"results": [{"source": {"path": "/r/pnpm-lock.yaml", "type": "lockfile"}, "packages": [
    {"package": {"name": "undici", "version": "7.28.0", "ecosystem": "npm"},
     "vulnerabilities": [{"id": GHSA, "aliases": [CVE]}],
     "groups": [{"ids": [GHSA], "aliases": [GHSA, CVE]}]}]}]})
_found, _probs = m.interpret(127, _CLEAN_JSON)
record("interpret: exit 127 with a clean-looking {\"results\": []} (the absent-database print) -> problem, no pass",
       _found == [] and any("exited 127" in p for p in _probs))
record("interpret: exit 128 with empty stdout (a lockfile without packages) -> problem",
       any("exited 128" in p for p in m.interpret(128, "")[1]))
record("interpret: exit 1 whose JSON lists no finding -> problem",
       any("lists none" in p for p in m.interpret(1, _CLEAN_JSON)[1]))
record("interpret: exit 0 with unreadable stdout -> problem",
       any("no readable JSON verdict" in p for p in m.interpret(0, "not json")[1]))
record("interpret: exit 0 with a `results` value that is no list -> problem",
       any("no readable JSON verdict" in p for p in m.interpret(0, '{"results": null}')[1]))
record("interpret: exit 0 with {\"results\": []} -> clean", m.interpret(0, _CLEAN_JSON) == ([], []))
record("interpret/findings: the measured result shape -> (id, aliases, package, version, ecosystem, source path)",
       m.interpret(1, _finding_json) == ([m.Finding(GHSA, (CVE,), "undici", "7.28.0", "npm", "/r/pnpm-lock.yaml")], []))
record("findings: a vulnerability without an id -> ValueError (fail-closed)",
       any("no readable JSON verdict" in p for p in m.interpret(1, _finding_json.replace(f'"id": "{GHSA}", ', ""))[1]))
record("name_pin: a finding from the pin's temp lock is named `package.json packageManager`, another keeps its source",
       m.name_pin([m.Finding(PNPM_ID, (), "pnpm", "10.13.1", "npm", f"/tmp/x/{m._PIN_DIR}/package-lock.json"),
                   _incident[0]])
       == [m.Finding(PNPM_ID, (), "pnpm", "10.13.1", "npm", "package.json packageManager"), _incident[0]])

_two = [("pnpm-lock.yaml", Path("/r/pnpm-lock.yaml")), ("requirements.txt", Path("/r/requirements-ci.txt")),
        ("package-lock.json", Path(f"/tmp/x/{m._PIN_DIR}/package-lock.json"))]
_counts = m.source_packages(json.dumps({"results": [
    {"source": {"path": "/r/pnpm-lock.yaml"}, "packages": [{"package": {}}, {"package": {}}]},
    {"source": {"path": f"/tmp/x/{m._PIN_DIR}/package-lock.json"}, "packages": [{"package": {}}]}]}))
record("source_packages: the --all-packages report -> packages read per normalised source path",
       _counts == {os.path.normcase(os.path.normpath("/r/pnpm-lock.yaml")): 2,
                   os.path.normcase(os.path.normpath(f"/tmp/x/{m._PIN_DIR}/package-lock.json")): 1})
_unread = m.unread_sources(_two, _counts)
record("unread_sources: an input the report lists no package for -> one problem naming it; the read ones and the "
       "pin lock raise none", len(_unread) == 1 and "read no package from" in _unread[0]
       and str(Path("/r/requirements-ci.txt")) in _unread[0])
record("unread_sources: the pin lock missing from the report -> named `package.json packageManager`",
       any("package.json packageManager" in p for p in m.unread_sources(_two, {})))

# --- the census: the tracked tree is the scan set ------------------------------------------------------------------
_TREE = ["Cargo.lock", "fuzz/Cargo.lock", "pnpm-lock.yaml", "requirements-ci.txt", "requirements-yamllint.txt",
         "src-tauri/engines.lock", "tests/g53-fixture/Cargo.lock", "scripts/check-lockfile-integrity",
         "tests/engines-lock-valid.toml", "README.md", "package.json"]
_RUST = ("Cargo.lock", "fuzz/Cargo.lock")
_srcs, _cprobs = m.census(_TREE, _RUST)
record("census: this tree's shape -> exactly the three OSV sources (pnpm-lock.yaml npm, both requirements files "
       "PyPI), no problem; the Rust locks, the declared files and lock-named non-lockfiles raise nothing",
       _cprobs == [] and _srcs == [m.Source("pnpm-lock.yaml", "pnpm-lock.yaml", "npm"),
                                    m.Source("requirements-ci.txt", "requirements.txt", "PyPI"),
                                    m.Source("requirements-yamllint.txt", "requirements.txt", "PyPI")])
for _extra, _parser in (("requirements-extra.txt", "requirements.txt"), ("requirements/dev.txt", "requirements.txt"),
                        ("tools/Requirements.TXT", "requirements.txt"), ("constraints-dev.txt", "requirements.txt"),
                        ("web/pnpm-lock.yaml", "pnpm-lock.yaml"), ("tools/package-lock.json", "package-lock.json")):
    _s, _p = m.census(_TREE + [_extra], _RUST)
    record(f"census: a tracked {_extra} is discovered and scanned (parser {_parser}), not left unread",
           _p == [] and any(s.path == _extra and s.parser == _parser for s in _s))
for _extra in ("uv.lock", "docs/Pipfile.lock", "web/yarn.lock", "go.sum", "sub/Cargo.lock", "supply-chain/imports.lock",
               "x/poetry.LOCK", "tools/deps.lockfile"):
    _p = m.census(_TREE + [_extra], _RUST)[1]
    record(f"census: a tracked {_extra} no half reads and no declaration names -> 'unread lockfile'",
           len(_p) == 1 and f"unread lockfile {_extra}" in _p[0])
record("census: lock-like directory names never make a file lockfile-named (the basename decides)",
       m.census(_TREE + ["requirements-docs/index.md", "x.lock/readme.md", "pnpm-lock.yaml.d/a.md"], _RUST)[1] == [])
_p = m.census([p for p in _TREE if p != "fuzz/Cargo.lock"], _RUST)[1]
record("census: a CARGO_AUDIT_LOCKS path that is not tracked -> problem",
       len(_p) == 1 and "CARGO_AUDIT_LOCKS names fuzz/Cargo.lock" in _p[0])
_p = m.census([p for p in _TREE if p != "src-tauri/engines.lock"], _RUST)[1]
record("census: a DECLARED_UNSCANNED path that is not tracked -> problem",
       len(_p) == 1 and "DECLARED_UNSCANNED names src-tauri/engines.lock" in _p[0])
_p = m.census(_TREE + ["docs/notes.txt"], _RUST, {**m.DECLARED_UNSCANNED, "docs/notes.txt": "r"})[1]
record("census: a declaration of a file the census would not leave unread (not lockfile-named) -> problem",
       len(_p) == 1 and "would not leave unread" in _p[0])
_p = m.census(_TREE, _RUST, {**m.DECLARED_UNSCANNED, "requirements-ci.txt": "r"})[1]
record("census: a declaration of a scanned source -> problem (a declaration never excuses a readable file)",
       len(_p) == 1 and "would not leave unread" in _p[0])
_p = m.census(_TREE + ["uv.lock"], _RUST, {**m.DECLARED_UNSCANNED, "uv.lock": "  "})[1]
record("census: a declaration without a reason -> problem", len(_p) == 1 and "without a reason" in _p[0])
for _gone in m.REQUIRED_SOURCES:
    _p = m.census([p for p in _TREE if p != _gone], _RUST)[1]
    record(f"census: the required source {_gone} gone from the tree -> problem (the presence list)",
           len(_p) == 1 and f"the required source {_gone}" in _p[0])
record("census: the presence list is frozen at the three sources this tree scans",
       m.REQUIRED_SOURCES == ("pnpm-lock.yaml", "requirements-ci.txt", "requirements-yamllint.txt"))
record("census: DECLARED_UNSCANNED is frozen at the G53 fixture lock and engines.lock, each with a reason",
       sorted(m.DECLARED_UNSCANNED) == ["src-tauri/engines.lock", "tests/g53-fixture/Cargo.lock"]
       and all(r.strip() for r in m.DECLARED_UNSCANNED.values()))

# --- the Rust half's lock set: one copy, read from check-rust-lint-contract's syntax tree ------------------------------
_rust_mod, _ = _load(RUST_HALF, "crlc_for_osv")
record("rust_audit_locks: the committed check-rust-lint-contract -> its CARGO_AUDIT_LOCKS, equal to the loaded "
       "module's value (binds the census to the Rust half)",
       m.rust_audit_locks(RUST_HALF.read_text(encoding="utf-8")) == _rust_mod.CARGO_AUDIT_LOCKS
       == ("Cargo.lock", "fuzz/Cargo.lock"))
for _label, _src in (("no CARGO_AUDIT_LOCKS", "X = 1\n"), ("a list, not a tuple", 'CARGO_AUDIT_LOCKS = ["Cargo.lock"]\n'),
                     ("a computed element", 'CARGO_AUDIT_LOCKS = ("Cargo.lock", str(1))\n'),
                     ("an empty tuple", "CARGO_AUDIT_LOCKS = ()\n"), ("a syntax error", "CARGO_AUDIT_LOCKS = (\n")):
    record(f"rust_audit_locks: {_label} -> None (the check fails closed)", m.rust_audit_locks(_src) is None)

# --- the packageManager pin -----------------------------------------------------------------------------------------
_SHA = "+sha512." + "ab12" * 32
record("pnpm_pin: `pnpm@10.34.7` -> 10.34.7", m.pnpm_pin(package_json("pnpm@10.34.7")) == ("10.34.7", None))
record("pnpm_pin: corepack's `pnpm@10.34.7+sha512.<hex>` form -> 10.34.7 (the pin is scanned, not skipped)",
       m.pnpm_pin(package_json("pnpm@10.34.7" + _SHA)) == ("10.34.7", None))
for _label, _pin in (("a range `pnpm@^10`", "pnpm@^10"), ("a bare `pnpm`", "pnpm"), ("another manager", "yarn@4.5.0"),
                     ("an upper-case hash", "pnpm@10.34.7+sha512.AB12"), ("a two-part version", "pnpm@10.34"),
                     ("a trailing space", "pnpm@10.34.7 ")):
    record(f"pnpm_pin: {_label} -> problem (fail-closed)", m.pnpm_pin(package_json(_pin))[0] is None)
record("pnpm_pin: no packageManager -> problem", m.pnpm_pin(package_json(None))[0] is None)
record("pnpm_pin: package.json not JSON -> problem", m.pnpm_pin("{")[0] is None)
_pl = json.loads(m.pin_lock("10.34.7"))
record("pin_lock: a one-package npm v3 lock pinning pnpm at the version",
       _pl["lockfileVersion"] == 3 and _pl["packages"]["node_modules/pnpm"] == {"version": "10.34.7", "dev": True}
       and _pl["packages"][""]["devDependencies"] == {"pnpm": "10.34.7"})

# --- argv + environment ---------------------------------------------------------------------------------------------
# [Test-Change: push 4 C2a - old-obsolete+new-correct, build-gates G17 row] old: one `--lockfile <lock>` and no
# `--all-packages`; new: one `--lockfile <parser>:<path>` per source (the parser explicit, the census decides it) and
# `--all-packages`, so the check can red a source the scanner read no package from; the refresh argv likewise.
_in = [("pnpm-lock.yaml", Path("/r/pnpm-lock.yaml")), ("requirements.txt", Path("/r/requirements-ci.txt"))]
_argv = m.scan_argv(Path("osv"), _in, Path("cfg.toml"))
record("scan_argv: `scan source --offline --all-packages --format json --config <empty>` and one `--lockfile "
       "<parser>:<path>` per input, no download flag",
       _argv == ["osv", "scan", "source", "--offline", "--all-packages", "--format", "json", "--config", "cfg.toml",
                 "--lockfile", f"pnpm-lock.yaml:{Path('/r/pnpm-lock.yaml')}",
                 "--lockfile", f"requirements.txt:{Path('/r/requirements-ci.txt')}"])
record("refresh_argv: exactly `scan source --offline-vulnerabilities --download-offline-databases --format json "
       "--config <empty>` and the same --lockfile pairs (it downloads, still with the empty --config)",
       m.refresh_argv(Path("osv"), _in, Path("cfg.toml"))
       == ["osv", "scan", "source", "--offline-vulnerabilities", "--download-offline-databases", "--format", "json",
           "--config", "cfg.toml", "--lockfile", f"pnpm-lock.yaml:{Path('/r/pnpm-lock.yaml')}",
           "--lockfile", f"requirements.txt:{Path('/r/requirements-ci.txt')}"])
_env = m.child_env({"HTTPS_PROXY": "http://proxy.example:8080", "NO_PROXY": "*", "no_proxy": "*", "PATH": "p"},
                   Path("db"), offline=True)
record("child_env(offline): the database dir is set, every HTTP(S) proxy variable is the closed port, NO_PROXY and "
       "no_proxy are gone",
       _env["OSV_SCANNER_LOCAL_DB_CACHE_DIRECTORY"] == "db" and not {"NO_PROXY", "no_proxy"} & set(_env)
       and _env["PATH"] == "p"
       and all(_env[v] == "http://127.0.0.1:9" for v in ("HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy")))
_renv = m.child_env({"HTTPS_PROXY": "http://proxy.example:8080"}, Path("db"), offline=False)
record("child_env(refresh): the caller's proxy is kept (the download step may need it)",
       _renv["HTTPS_PROXY"] == "http://proxy.example:8080" and _renv["OSV_SCANNER_LOCAL_DB_CACHE_DIRECTORY"] == "db")

# --- db_state per ecosystem, the age floor, the scanner lookup ----------------------------------------------------------
with tempfile.TemporaryDirectory() as _td:
    T = Path(_td)
    for _eco in ("npm", "PyPI"):
        record(f"db_state({_eco}): no zip -> absent", m.db_state(T / "none", _eco) == "absent")
    record("db_state: a zip at the unmeasured osv-scanner/<eco>/all.zip layout -> absent (the scanner reads osv-scalibr/)",
           m.db_state(fixture_db(T / "wrong-layout", "osv-fixture-undici.json", subdir="osv-scanner"), "npm") == "absent")
    record("db_state: the npm database present, the PyPI one absent -> each ecosystem answers for itself",
           m.db_state(fixture_db(T / "npm-only", "osv-fixture-undici.json"), "npm") == "ok"
           and m.db_state(T / "npm-only", "PyPI") == "absent")
    write(m.db_zip(T / "notzip", "PyPI"), "PK but not a zip\n")
    record("db_state: bytes that are not a zip -> partial", m.db_state(T / "notzip", "PyPI") == "partial")
    record("db_state: a zero-member zip -> partial (the scanner passes it silently)",
           m.db_state(zip_at(T / "zero", "npm", {}), "npm") == "partial")
    record("db_state: a zip holding only a directory entry -> partial",
           m.db_state(zip_at(T / "dirs", "npm", {"npm/": b""}), "npm") == "partial")
    _crc = zip_at(T / "crc", "npm", {"x.json": b'{"id": "X-1"}' * 20}, zipfile.ZIP_STORED)
    _raw = m.db_zip(_crc, "npm").read_bytes()
    _at = _raw.index(b'{"id": "X-1"}')
    m.db_zip(_crc, "npm").write_bytes(_raw[:_at] + b"[" + _raw[_at + 1:])
    record("db_state: a member whose CRC fails -> partial", m.db_state(_crc, "npm") == "partial")
    _inflate = zip_at(T / "inflate", "npm", {"x.json": b'{"id": "X-1"}' * 20})
    _zraw = bytearray(m.db_zip(_inflate, "npm").read_bytes())
    with zipfile.ZipFile(m.db_zip(_inflate, "npm")) as _zf:
        _off = _zf.getinfo("x.json").header_offset
    _name_len, _extra_len = struct.unpack("<HH", bytes(_zraw[_off + 26:_off + 30]))
    _zraw[_off + 30 + _name_len + _extra_len] = 0x07    # BFINAL=1, BTYPE=11 (reserved): inflating raises zlib.error
    m.db_zip(_inflate, "npm").write_bytes(bytes(_zraw))
    record("db_state: a member whose deflate stream is corrupt -> partial (the inflater's zlib.error is caught)",
           state_of(_inflate) == "partial")
    _both = fixture_db(T / "both", "osv-fixture-undici.json", "osv-fixture-pypi.json")
    record("db_state: the one-record fixture databases -> ok", m.db_state(_both, "npm") == m.db_state(_both, "PyPI") == "ok")

    # the database-age floor (--require-db): the refresh record dates the bytes the scan reads
    _now = time.time()
    _day = 86400
    fresh_record(_both, now=_now - 3600)
    record("age floor: a record of a completed refresh an hour ago, both databases ok at their zip's sha256 -> clean",
           m.age_problems(_both, ("npm", "PyPI"), _now) == [])
    record("age floor: the same record 3 days and 1 s later -> problem (older than MAX_DB_AGE_S = 72 h)",
           m.MAX_DB_AGE_S == 3 * _day
           and any("age floor" in p for p in m.age_problems(_both, ("npm", "PyPI"), _now - 3600 + 3 * _day + 1)))
    fresh_record(_both, now=_now + 3600)
    record("age floor: a record from the future (beyond the 300 s skew) -> problem",
           any("age floor" in p for p in m.age_problems(_both, ("npm", "PyPI"), _now)))
    fresh_record(_both, rc=None, now=_now)
    record("age floor: a failed refresh (no scanner verdict: a download timeout) -> problem",
           any("did not complete its download" in p for p in m.age_problems(_both, ("npm", "PyPI"), _now)))
    fresh_record(_both, ("npm",), now=_now)
    record("age floor: a record without the PyPI database -> problem naming PyPI",
           any("PyPI database ok (unrecorded)" in p for p in m.age_problems(_both, ("npm", "PyPI"), _now)))
    fresh_record(_both, now=_now)
    shutil.copyfile(m.db_zip(fixture_db(T / "other", "osv-fixture-pyyaml.json"), "PyPI"), m.db_zip(_both, "PyPI"))
    record("age floor: a zip swapped after the refresh (another sha256) -> problem (the record cannot date it)",
           any("is not the one the last refresh recorded" in p for p in m.age_problems(_both, ("npm", "PyPI"), _now)))
    (_both / m.REFRESH_STATE).unlink()
    record("age floor: no refresh record -> problem", any("no readable refresh record" in p
                                                          for p in m.age_problems(_both, ("npm",), _now)))
    write(_both / m.REFRESH_STATE, '{"refreshed_at": "yesterday", "ok": true, "ecosystems": {}}\n')
    record("age floor: a malformed record -> problem", any("malformed" in p for p in m.age_problems(_both, ("npm",), _now)))
    _r = m.refresh_record(fixture_db(T / "rec", "osv-fixture-undici.json"), ("npm", "PyPI"), 0, _now)
    record("refresh_record: a scanner verdict with the PyPI database absent -> ok false, PyPI absent without a sha256",
           _r["ok"] is False and _r["ecosystems"]["PyPI"] == {"state": "absent", "sha256": None}
           and _r["ecosystems"]["npm"]["sha256"]
           == hashlib.sha256(m.db_zip(T / "rec", "npm").read_bytes()).hexdigest())

    for _name in ("osv-scanner", "osv-scanner.exe"):
        _pinned_dir = T / f"bin-{_name}"
        write(_pinned_dir / _name, "")
        record(f"osv_binary: a bin dir holding the pinned {_name} -> that path", m.osv_binary(_pinned_dir) == _pinned_dir / _name)
    with osv_scanner_on_path() as _plant:
        _path_hit = shutil.which("osv-scanner")
        _lookup = m.osv_binary(T / "no-bin")
    record("osv_binary (non-vacuity): a PATH lookup finds the osv-scanner planted first on PATH",
           _path_hit is not None and Path(_path_hit).parent == _plant)
    record("osv_binary: an empty bin dir -> None with an osv-scanner first on PATH (a PATH binary is never used)",
           _lookup is None)

# --- tracked_files: git ls-files in a scratch repository ---------------------------------------------------------------
with tempfile.TemporaryDirectory() as _td:
    T = Path(_td)
    _root, _files = scratch_root(T / "repo", extra={"uv.lock": "version = 1\n", "a dir/requirements-x.txt": req()})
    _made = git_repo(_root, _files)
    _listed, _why = m.tracked_files(_root)
    record("tracked_files: a scratch repository with its files added -> exactly them (an index entry counts, a path "
           "with a space too)", _made and _listed is not None and sorted(_listed) == _files)
    write(_root / "untracked.lock", "x\n")
    record("tracked_files: an untracked lockfile is not listed (the census reads the tracked tree)",
           "untracked.lock" not in (m.tracked_files(_root)[0] or []))
    _none, _why = m.tracked_files(T / "not-a-repo-" / "x")
    record("tracked_files: no repository -> None with the reason (the census fails closed)", _none is None and bool(_why))
    rc_, out_ = without_binary(m.check, root=_root, ignores_file=ignores_file(T / "ign"), db_dir=T / "none",
                               require_db=False)
    record("check (git census, scratch repository): a tracked uv.lock -> exit 1 'unread lockfile uv.lock'; the "
           "requirements file in a directory with a space is scanned, not unread",
           rc_ == 1 and any("unread lockfile uv.lock" in ln for ln in out_)
           and not any("requirements-x.txt" in ln for ln in out_))

# --- the mode postures that stop before the scanner --------------------------------------------------------------------
with tempfile.TemporaryDirectory() as _td:
    T = Path(_td)
    _root, _tracked = scratch_root(T / "root", undici="7.28.0")
    _ign = ignores_file(T / "ign")
    _never_run = T / "never-run.exe"          # the postures below return before any scanner is executed
    _ck = dict(root=_root, ignores_file=_ign, tracked=_tracked)
    rc_, out_ = without_binary(m.check, db_dir=T / "none", require_db=False, **_ck)
    record("mode default: no pinned binary -> exit 0 with the notice naming install-gate-tools",
           rc_ == 0 and "NOTICE" in out_[0] and "install-gate-tools" in out_[0])
    rc_, out_ = without_binary(m.check, db_dir=T / "none", require_db=True, **_ck)
    record("mode --require-db: no pinned binary -> exit 1 naming the absent pinned binary",
           rc_ == 1 and "FAIL: the pinned osv-scanner is not installed" in out_[0])
    rc_, out_ = guarded(m.check, binary=_never_run, db_dir=T / "none", require_db=False, **_ck)
    record("mode default: no local database at all -> exit 0, a notice per ecosystem naming --refresh (never silent)",
           rc_ == 0 and sum("NOTICE: no local OSV" in ln and "--refresh" in ln for ln in out_) == 2)
    rc_, out_ = guarded(m.check, binary=_never_run, db_dir=T / "none", require_db=True, **_ck)
    record("mode --require-db: no database -> exit 1 for each ecosystem",
           rc_ == 1 and any("no OSV npm database" in ln for ln in out_) and any("no OSV PyPI database" in ln for ln in out_))
    _pypi_partial = zip_at(fixture_db(T / "pp", "osv-fixture-undici.json"), "PyPI", {})
    for _mode, _rq in CHECK_MODES:
        rc_, out_ = guarded(m.check, binary=_never_run, db_dir=_pypi_partial, require_db=_rq, **_ck)
        record(f"mode {_mode}: a partial PyPI database beside an ok npm one -> exit 1 (partial, in every mode)",
               rc_ == 1 and "PyPI database" in out_[0] and "partial" in out_[0])
    _ok_db = fixture_db(T / "okdb", "osv-fixture-undici.json", "osv-fixture-pypi.json")
    rc_, out_ = guarded(m.check, binary=_never_run, db_dir=_ok_db, require_db=True, **_ck)
    record("mode --require-db: both databases ok but no refresh record -> exit 1 (the age floor), before any scan",
           rc_ == 1 and any("no readable refresh record" in ln for ln in out_))
    fresh_record(_ok_db, rc=None)
    rc_, out_ = guarded(m.check, binary=_never_run, db_dir=_ok_db, require_db=True, **_ck)
    record("mode --require-db: the last refresh failed -> exit 1 (a failed refresh reds L4)",
           rc_ == 1 and any("did not complete its download" in ln for ln in out_))
    fresh_record(_ok_db, now=time.time() - 4 * 86400)
    rc_, out_ = guarded(m.check, binary=_never_run, db_dir=_ok_db, require_db=True, **_ck)
    record("mode --require-db: the last refresh is 4 days old -> exit 1 (the age floor)",
           rc_ == 1 and any("age floor" in ln for ln in out_))
    # the arms before the scanner that fail whatever the mode, each red in both check modes with no binary
    _grown = ignores_file(T / "grown", *above_count())
    for _mode, _rq in CHECK_MODES:
        def _red(label: str, needle: str, **over: object) -> None:
            kw = {**_ck, "db_dir": T / "none", "require_db": _rq, **over}
            rc, out = without_binary(m.check, **kw)
            record(f"mode {_mode}: {label} -> exit 1 even with no binary and no database",
                   rc == 1 and any(needle in ln for ln in out))
        _red("a grown ignore file", "raise EXPECTED_IGNORE_COUNT", ignores_file=_grown)
        _red("an unreadable ignore file (absent)", "cannot read", ignores_file=T / "absent.toml")
        _red("an unread lockfile in the tracked tree", "unread lockfile uv.lock", tracked=_tracked + ["uv.lock"])
        _red("requirements-yamllint.txt gone from the tracked tree", "the required source requirements-yamllint.txt",
             tracked=[p for p in _tracked if p != "requirements-yamllint.txt"])
        _red("a tracked source absent from the working tree", "absent from the working tree",
             tracked=_tracked + ["requirements-dev.txt"])
        for _pin in ("pnpm@^10", None):
            _r2, _t2 = scratch_root(T / f"pin-{_mode}-{_pin}", pin=_pin)
            _red(f"packageManager {_pin!r}", "packageManager", root=_r2, tracked=_t2)
        _r3, _t3 = scratch_root(T / f"rust-{_mode}", extra={"scripts/check-rust-lint-contract": "X = 1\n"})
        _red("a Rust half without CARGO_AUDIT_LOCKS", "assigns no CARGO_AUDIT_LOCKS", root=_r3, tracked=_t3)
    rc_, out_ = without_binary(m.refresh, root=_root, db_dir=T / "none", tracked=_tracked)
    record("mode --refresh: no pinned binary -> exit 0 with the notice, nothing refreshed (the download step exits 0)",
           rc_ == 0 and "NOTICE" in out_[0] and "nothing refreshed" in out_[0])

# --- the download's retry classifier: osv-scanner 2.6.0's measured words for each failure (push 4 R2, the gate
# container: a refused and a reset connection, no DNS, a fake HTTP proxy's CONNECT answers, a fake TLS host's statuses
# behind a CA trusted for that run, a transfer cut short, an empty trust store) -----------------------------------------
_U = 'Get "https://osv-vulnerabilities.storage.googleapis.com/npm/all.zip": '
_H = 'Head "https://osv-vulnerabilities.storage.googleapis.com/npm/all.zip": '
_F = "could not load db for npm ecosystem: unable to fetch OSV database: "
_A = "could not retrieve OSV database archive: "
_CLASSES = [   # (case, exit code, stderr, transient?)
    ("a refused connection (warm)", 127, _F + _H + "proxyconnect tcp: dial tcp 127.0.0.1:9: connect: connection refused",
     True),
    ("a refused connection (cold)", 127, _F + _A + _U + "proxyconnect tcp: dial tcp 127.0.0.1:9: connect: connection "
     "refused", True),
    ("a reset connection", 127, _F + _A + _U + "read tcp 127.0.0.1:42746->127.0.0.1:18999: read: connection reset by "
     "peer", True),
    ("no DNS", 127, _F + _H + "dial tcp: lookup osv-vulnerabilities.storage.googleapis.com on 127.0.0.11:53: server "
     "misbehaving", True),
    ("a transfer cut short", 127, _F + _A + _U + "EOF", True),
    ("an attempt ended at its bound", 124, "osv-scanner: timed out - no result within the 300 s bound of one attempt",
     True),
    ("a proxy's 429 to the CONNECT", 127, _F + _A + _U + "Too Many Requests", True),
    ("a proxy's 502 to the CONNECT", 127, _F + _A + _U + "Bad Gateway", True),
    ("a proxy's 503 to the CONNECT", 127, _F + _A + _U + "Service Unavailable", True),
    ("a proxy's 504 to the CONNECT", 127, _F + _A + _U + "Gateway Timeout", True),
    ("the host's 429", 127, _F + "db host returned 429 Too Many Requests", True),
    ("the host's 500", 127, _F + "db host returned 500 Internal Server Error", True),
    ("the host's 502", 127, _F + "db host returned 502 Bad Gateway", True),
    ("the host's 503", 127, _F + "db host returned 503 Service Unavailable", True),
    ("a proxy's 403 to the CONNECT (a refusal)", 127, _F + _A + _U + "Forbidden", False),
    ("a proxy's 407 to the CONNECT", 127, _F + _A + _U + "Proxy Authentication Required", False),
    ("the host's 403", 127, _F + "db host returned 403 Forbidden", False),
    ("the host's 404", 127, _F + "db host returned 404 Not Found", False),
    ("a rejected certificate", 127, _F + _H + "tls: failed to verify certificate: x509: certificate signed by unknown "
     "authority", False),
    ("a failure that names no download error", 127, "Error: open /x/osv-scalibr/npm/all.zip: permission denied", False),
    ("an exit 2 without the line", 2, "unknown flag: --nope", False),
]
_wrong = [f"{case}: {m.refresh_transient(rc, err)!r}" for case, rc, err, want in _CLASSES
          if (m.refresh_transient(rc, err) is not None) != want]
record("refresh_transient: the measured download failures - a refused or reset connection, no DNS, a transfer cut "
       "short, an attempt past its bound, a proxy's or the host's 429 and 5xx - are transient; a proxy's or the "
       "host's 403, 404 or 407, a rejected certificate and a failure without osv-scanner's `unable to fetch OSV "
       "database` line are final", not _wrong)
if _wrong:
    print("  " + "; ".join(_wrong))
record("refresh_transient: a transient reason names what failed (the host's status, the proxy's answer, the bound)",
       m.refresh_transient(127, _F + "db host returned 503 Service Unavailable") == "db host returned 503"
       and m.refresh_transient(127, _F + _A + _U + "Bad Gateway") == "proxy CONNECT Bad Gateway"
       and m.refresh_transient(124, "") == f"past its {m.REFRESH_ATTEMPT_S} s bound")

# --- the scanner's child: what check and refresh start, and how each answers a child that fails ---------------------
with tempfile.TemporaryDirectory() as _td:
    T = Path(_td)
    _root, _tracked = scratch_root(T / "root", undici="7.28.0", pin="pnpm@10.34.7" + _SHA)
    _srcs = ["pnpm-lock.yaml", "requirements-ci.txt", "requirements-yamllint.txt"]
    _db = fresh_record(fixture_db(T / "db", "osv-fixture-undici.json", "osv-fixture-pypi.json"))
    _ign = ignores_file(T / "ign")
    _bin = T / "osv-scanner.exe"               # recorded by the fake below, never started
    _closed = "http://127.0.0.1:9"
    _proxies = ("HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy")
    _ck = dict(binary=_bin, root=_root, ignores_file=_ign, tracked=_tracked)
    for _mode, _rq in CHECK_MODES:            # the one scan child, whatever the mode
        with recording_subprocess() as _calls:
            rc_, out_ = guarded(m.check, db_dir=_db, require_db=_rq, **_ck)
        _c = _calls[0] if len(_calls) == 1 else {}
        record(f"check, mode {_mode}: one scan child over the three sources and the pin's one-package lock (the "
               "+sha512 pin scanned at its version), its empty --config file and the pin lock alone in the child's own "
               "temp cwd -> exit 0 naming every source",
               rc_ == 0 and len(_calls) == 1 and child_ok(_c, m.scan_argv, _bin, _root, _srcs, "10.34.7")
               and all(s in out_[-1] for s in _srcs + ["package.json packageManager"]))
        _cenv = _c.get("env", {})
        record(f"check, mode {_mode}: the scan child gets the closed proxy on every HTTP(S) proxy variable, no "
               "NO_PROXY or no_proxy, the database dir and SCAN_TIMEOUT",
               isinstance(_cenv, dict) and all(_cenv.get(v) == _closed for v in _proxies)
               and not {"NO_PROXY", "no_proxy"} & set(_cenv) and _c.get("timeout") == m.SCAN_TIMEOUT
               and _cenv.get("OSV_SCANNER_LOCAL_DB_CACHE_DIRECTORY") == str(_db))
    _npm_only = fixture_db(T / "npm-only", "osv-fixture-undici.json")
    with recording_subprocess() as _calls:
        rc_, out_ = guarded(m.check, db_dir=_npm_only, require_db=False, **_ck)
    record("check, mode default: the PyPI database absent -> the child scans the npm sources and the pin only, exit 0 "
           "with the skip notice and the stale rule's suspension notice",
           rc_ == 0 and len(_calls) == 1 and child_ok(_calls[0], m.scan_argv, _bin, _root, ["pnpm-lock.yaml"], "10.34.7")
           and any("PyPI database" in ln for ln in out_) and any("suspended" in ln for ln in out_))
    _pypi_only = fixture_db(T / "pypi-only", "osv-fixture-pypi.json")
    with recording_subprocess() as _calls:
        rc_, out_ = guarded(m.check, db_dir=_pypi_only, require_db=False, **_ck)
    record("check, mode default: the npm database absent -> the child scans the PyPI sources only (no pin lock)",
           rc_ == 0 and len(_calls) == 1
           and child_ok(_calls[0], m.scan_argv, _bin, _root, ["requirements-ci.txt", "requirements-yamllint.txt"], None))
    with recording_subprocess() as _calls:
        rc_, out_ = guarded(m.check, db_dir=_npm_only, require_db=False, binary=_bin, root=_root,
                            ignores_file=ignores_file(T / "ign-one", (GHSA, "r")), tracked=_tracked)
    record("check, mode default: an entry whose advisory the partial scan does not report is not called stale while "
           "an ecosystem is skipped", rc_ == 0)
    with recording_subprocess() as _calls:
        rc_, out_ = guarded(m.refresh, binary=_bin, root=_root, db_dir=_db, tracked=_tracked, now=1_700_000_000.0)
    _c = _calls[0] if len(_calls) == 1 else {}
    _renv = _c.get("env", {})
    _rec = json.loads((_db / m.REFRESH_STATE).read_text(encoding="utf-8"))
    record("refresh: one download child over the three sources and the pin lock, refresh_argv, its empty --config file "
           "and the pin lock alone in its own temp cwd -> exit 0",
           rc_ == 0 and len(_calls) == 1 and child_ok(_c, m.refresh_argv, _bin, _root, _srcs, "10.34.7"))
    # [Test-Change: push 4 C2a - old-obsolete+new-correct, build-gates §0 *Network fetches*] old: the one download
    # ran under subprocess.run's REFRESH_TIMEOUT; new: each attempt runs under net-retry's run_bounded with
    # REFRESH_ATTEMPT_S, the bound g24-net-retry pins and the job timeouts are computed from
    record("refresh: the download child keeps the network (no proxy variable is the closed port), the database dir "
           "and run_bounded's REFRESH_ATTEMPT_S bound",
           isinstance(_renv, dict) and not any(_renv.get(v) == _closed for v in _proxies)
           and _c.get("timeout") == m.REFRESH_ATTEMPT_S and _renv.get("OSV_SCANNER_LOCAL_DB_CACHE_DIRECTORY") == str(_db))
    record("refresh: a verdict exit with both databases ok -> the record says ok, at `now`, each database with its "
           "zip's sha256",
           _rec["ok"] is True and _rec["refreshed_at"] == "2023-11-14T22:13:20Z" and _rec["scanner_exit"] == 0
           and all(_rec["ecosystems"][e]["sha256"] == hashlib.sha256(m.db_zip(_db, e).read_bytes()).hexdigest()
                   for e in ("npm", "PyPI")))
    # The retry policy over the download (build-gates §0 *Network fetches*), on a fake clock: the scanner's measured
    # words for a refused connection (transient) and for the host's 404 (final).
    _refused = ('could not load db for npm ecosystem: unable to fetch OSV database: Head "https://osv-vulnerabilities'
                '.storage.googleapis.com/npm/all.zip": proxyconnect tcp: dial tcp 127.0.0.1:9: connect: connection '
                'refused\n')
    _gone = "could not load db for npm ecosystem: unable to fetch OSV database: db host returned 404 Not Found\n"
    _retry, _pauses = fake_retrier()
    with recording_subprocess(downloads=[(127, _refused)]) as _calls:
        rc_, out_ = guarded(m.refresh, binary=_bin, root=_root, db_dir=_db, tracked=_tracked, retrier=_retry)
    _rec = json.loads((_db / m.REFRESH_STATE).read_text(encoding="utf-8"))
    with recording_subprocess():
        _after = guarded(m.check, db_dir=_db, require_db=True, **_ck)[0]
    record("refresh: one transient failure (a refused connection) then a download -> two attempts, one pause, the "
           "record ok and the --require-db check after it green",
           rc_ == 0 and len(_calls) == 2 and len(_pauses) == 1 and _rec["ok"] is True and _rec["scanner_exit"] == 0
           and "after 2 attempt(s)" in out_[0] and _after == 0 and not _retry.spent)
    _retry, _pauses = fake_retrier()
    with recording_subprocess(downloads=[(127, _refused)] * 50) as _calls:
        rc_, out_ = guarded(m.refresh, binary=_bin, root=_root, db_dir=_db, tracked=_tracked, retrier=_retry)
    _rec = json.loads((_db / m.REFRESH_STATE).read_text(encoding="utf-8"))
    with recording_subprocess():
        _after = guarded(m.check, db_dir=_db, require_db=True, **_ck)[0]
    record("refresh: a transient failure through the whole retry window -> the pauses fill exactly WINDOW_S, the "
           "Retrier is spent, exit 0 with the notice naming the window, the record not ok and the --require-db check "
           "after it red",
           rc_ == 0 and len(_calls) == len(_pauses) + 1 > 2 and abs(sum(_pauses) - m._NET.WINDOW_S) < 1e-6
           and _retry.spent and any(f"through its {m._NET.WINDOW_S} s retry window" in ln for ln in out_)
           and _rec["ok"] is False and _rec["scanner_exit"] == 127 and _after == 1)
    _retry, _pauses = fake_retrier()
    with recording_subprocess(downloads=[(127, _gone), (0, "")]) as _calls:
        rc_, out_ = guarded(m.refresh, binary=_bin, root=_root, db_dir=_db, tracked=_tracked, retrier=_retry)
    _rec = json.loads((_db / m.REFRESH_STATE).read_text(encoding="utf-8"))
    with recording_subprocess():
        _after = guarded(m.check, db_dir=_db, require_db=True, **_ck)[0]
    record("refresh: a final failure (the host's 404) -> one attempt, no pause, exit 0 with the notice `at once`, the "
           "record not ok and the --require-db check after it red",
           rc_ == 0 and len(_calls) == 1 and not _pauses and any("at once" in ln for ln in out_)
           and _rec["ok"] is False and _after == 1)
    # [Test-Change: push 4 C2a - old-obsolete+new-correct, build-gates §0 *Network fetches*] old: a download past
    # REFRESH_TIMEOUT raised TimeoutExpired, one attempt; new: run_bounded ends an attempt at its bound with exit 124,
    # a transient timeout retried through the window (measured: a stalled transfer ended by `timeout` exits 124)
    _retry, _pauses = fake_retrier()
    with recording_subprocess(downloads=[(124, "osv-scanner: timed out")] * 50) as _calls:
        rc_, out_ = guarded(m.refresh, binary=_bin, root=_root, db_dir=_db, tracked=_tracked, retrier=_retry)
    _rec = json.loads((_db / m.REFRESH_STATE).read_text(encoding="utf-8"))
    record("refresh: a download that never finishes inside its bound -> retried through the window, exit 0 with the "
           "failure notice, and the record says not ok, so the --require-db check after it reds",
           rc_ == 0 and len(_calls) > 2 and any("the download failed through its" in ln for ln in out_)
           and _rec["ok"] is False and guarded(m.check, db_dir=_db, require_db=True, **_ck)[0] == 1)
    # The last record goes before the download (push 4 R3 review): a record the refresh cannot write leaves none, so
    # the --require-db check after it reds instead of dating the database by the last good refresh.

    class _RecordBlocked:
        """A net-retry Retrier that, once the last record is gone and before the first attempt, puts a directory
        where the refresh writes its record (a record that cannot be written)."""

        def __init__(self, inner: object) -> None:
            self.inner = inner

        def run(self, fn: Callable[[], object], **kw: object) -> object:
            (_db / m.REFRESH_STATE).mkdir()
            return self.inner.run(fn, **kw)

    fresh_record(_db)
    _retry, _pauses = fake_retrier()
    with recording_subprocess(downloads=[(127, _gone)]) as _calls:
        rc_, out_ = guarded(m.refresh, binary=_bin, root=_root, db_dir=_db, tracked=_tracked,
                            retrier=_RecordBlocked(_retry))
    with recording_subprocess():
        _after = guarded(m.check, db_dir=_db, require_db=True, **_ck)[0]
    record("refresh: a good last record, then a final failure whose record cannot be written -> exit 0, the last "
           "record removed (none is left), the notices `cannot write the refresh record` and `not recorded`, and "
           "the --require-db check after it red",
           rc_ == 0 and len(_calls) == 1 and (_db / m.REFRESH_STATE).is_dir()
           and any("cannot write the refresh record" in ln for ln in out_)
           and any("at once" in ln and "not recorded" in ln for ln in out_) and _after == 1)
    shutil.rmtree(_db / m.REFRESH_STATE)
    write(_db / m.REFRESH_STATE / "keep", "")      # a last record that cannot be removed: a non-empty directory
    with recording_subprocess() as _calls:
        rc_, out_ = guarded(m.refresh, binary=_bin, root=_root, db_dir=_db, tracked=_tracked, retrier=fake_retrier()[0])
    record("refresh: a last record that cannot be removed -> exit 1 `cannot remove the last refresh record` before any "
           "download child",
           rc_ == 1 and not _calls and len(out_) == 1 and "FAIL: cannot remove the last refresh record" in out_[0])
    shutil.rmtree(_db / m.REFRESH_STATE)
    with recording_subprocess() as _calls:
        rc_, out_ = guarded(m.refresh, binary=_bin, root=_root, db_dir=_db, tracked=_tracked + ["uv.lock"])
    record("refresh: a census problem (an unread uv.lock) never stops the download of the readable sources",
           rc_ == 0 and len(_calls) == 1 and child_ok(_calls[0], m.refresh_argv, _bin, _root, _srcs, "10.34.7"))
    fresh_record(_db)
    # the check's fail-closed arms after the database test, each red in both check modes
    _pin_json = json.dumps({"results": [{"source": {"path": f"/tmp/x/{m._PIN_DIR}/package-lock.json"}, "packages": [
        {"package": {"name": "pnpm", "version": "10.13.1", "ecosystem": "npm"},
         "vulnerabilities": [{"id": PNPM_ID, "aliases": ["CVE-2025-69263"]}]}]}]})
    for _mode, _rq in CHECK_MODES:
        with recording_subprocess(stdout=_finding_json, returncode=1):
            rc_, out_ = guarded(m.check, db_dir=_db, require_db=_rq, **_ck)
        record(f"check, mode {_mode}: the scanner reports the incident advisory (exit 1 and its JSON) -> exit 1 naming "
               f"{GHSA} and undici@7.28.0", rc_ == 1 and any(GHSA in ln and "undici@7.28.0" in ln for ln in out_))
        with recording_subprocess(stdout=_pin_json, returncode=1):
            rc_, out_ = guarded(m.check, db_dir=_db, require_db=_rq, **_ck)
        record(f"check, mode {_mode}: an advisory on the pin's lock -> exit 1 naming pnpm@10.13.1 and `package.json "
               "packageManager`", rc_ == 1 and any("pnpm@10.13.1 (package.json packageManager)" in ln for ln in out_))
        with recording_subprocess(stdout='{"results": []}', returncode=127):
            rc_, out_ = guarded(m.check, db_dir=_db, require_db=_rq, **_ck)
        record(f"check, mode {_mode}: the scanner exits 127 with a clean-looking result -> exit 1",
               rc_ == 1 and any("exited 127" in ln for ln in out_))
        with recording_subprocess(raises=subprocess.TimeoutExpired(["osv-scanner"], m.SCAN_TIMEOUT)):
            rc_, out_ = guarded(m.check, db_dir=_db, require_db=_rq, **_ck)
        record(f"check, mode {_mode}: a scan that does not finish -> exit 1 (fail-closed)",
               rc_ == 1 and "did not finish" in out_[0])
        rc_, out_ = guarded(m.check, db_dir=_db, require_db=_rq, **{**_ck, "binary": T / "absent-scanner.exe"})
        record(f"check, mode {_mode}: a scanner that cannot be started (a path that does not exist) -> exit 1 "
               "(fail-closed)", rc_ == 1 and "cannot run" in out_[0])

# --- main(): each flag reaches its mode (lefthook runs the default; ci.yml runs --refresh, then --require-db) ---------
with tempfile.TemporaryDirectory() as _td:
    T = Path(_td)
    _root, _files = scratch_root(T / "root")
    ignores_file(_root / "scripts")
    _files.append("scripts/osv-ignores.toml")
    _git_ok = git_repo(_root, _files)
    _no_db = T / "no-db"
    _found = T / "absent-scanner.exe"     # the lookup's found-binary answer: a path that does not exist, no scanner
    rc_, _ = run_main(["--require-db", "--refresh"], binary=None, db_dir=_no_db, root=_root)
    record("usage: --require-db together with --refresh -> exit 2", rc_ == 2)
    rc_, out_ = run_main([], binary=None, db_dir=_no_db, root=_root)
    record("main([]): no pinned binary -> exit 0 with the check's skip notice (the census over ROOT's git index passed)",
           _git_ok and rc_ == 0 and "the OSV advisory scan is skipped here" in out_)
    rc_, out_ = run_main(["--require-db"], binary=None, db_dir=_no_db, root=_root)
    record("main(['--require-db']): no pinned binary -> exit 1 FAIL (the flag reaches the check's require_db)",
           rc_ == 1 and "FAIL: the pinned osv-scanner is not installed" in out_)
    rc_, out_ = run_main(["--refresh"], binary=None, db_dir=_no_db, root=_root)
    record("main(['--refresh']): no pinned binary -> exit 0 with the refresh notice (the flag reaches refresh)",
           rc_ == 0 and "nothing refreshed" in out_)
    rc_, out_ = run_main([], binary=_found, db_dir=_no_db, root=_root)
    record("main([]): the found binary and DB_DIR reach the check -> exit 0, the no-database notices naming DB_DIR",
           rc_ == 0 and f"NOTICE: no local OSV npm database at {m.db_zip(_no_db, 'npm')}" in out_)
    rc_, out_ = run_main(["--require-db"], binary=_found, db_dir=_no_db, root=_root)
    record("main(['--require-db']): the found binary and DB_DIR reach the check -> exit 1 naming DB_DIR",
           rc_ == 1 and f"FAIL: no OSV npm database at {m.db_zip(_no_db, 'npm')}" in out_)
    rc_, out_ = run_main(["--refresh"], binary=_found, db_dir=_no_db, root=_root)
    # [Test-Change: push 4 C2a - old-obsolete+new-correct, build-gates §0 *Network fetches*] old: the notice read
    # "the download failed (fail-open)"; new: a scanner that cannot be started is a final failure, "failed at once"
    record("main(['--refresh']): the found binary and DB_DIR reach refresh; a failed spawn -> exit 0 naming DB_DIR, "
           "the failed refresh recorded as a final failure",
           rc_ == 0 and "the download failed at once" in out_ and str(m.db_zip(_no_db, "npm")) in out_
           and json.loads((_no_db / m.REFRESH_STATE).read_text(encoding="utf-8"))["ok"] is False)

# --- the script as a process: the exit code and stream lefthook (the default) and ci.yml (--refresh, --require-db) read
with tempfile.TemporaryDirectory() as _td:
    T = Path(_td)
    _root, _files = scratch_root(T / "root", undici="7.28.0")
    # [Test-Change: push 4 C2a - old-obsolete+new-correct, build-gates §0 *Network fetches*] old: the script alone was
    # copied; new: it compiles scripts/net-retry from its own tree on load, as check-supply-chain does, so the copy
    # carries it
    shutil.copyfile(SCRIPT, _root / "scripts" / "check-osv-advisories")
    shutil.copyfile(ROOT / "scripts" / "net-retry", _root / "scripts" / "net-retry")
    ignores_file(_root / "scripts")
    _files += ["scripts/check-osv-advisories", "scripts/net-retry", "scripts/osv-ignores.toml"]
    _git_ok = git_repo(_root, _files)

    def cli(*flags: str) -> tuple[int | None, str, str]:
        """The copy run as `python3 -P scripts/check-osv-advisories <flags>` from the scratch root: (exit code, stdout,
        stderr); a child that cannot start or does not finish is a red leg with its reason, never a pass."""
        try:
            p = subprocess.run([sys.executable, "-P", str(_root / "scripts" / "check-osv-advisories"), *flags],
                               cwd=str(_root), capture_output=True, text=True, encoding="utf-8", errors="replace",
                               timeout=60)
        except (OSError, subprocess.TimeoutExpired) as e:
            return None, "", f"<the child failed: {type(e).__name__}: {e}>"
        return p.returncode, p.stdout, p.stderr

    with osv_scanner_on_path():           # the children inherit this PATH: their lookup must not take the plant
        _default, _require_db, _refresh = cli(), cli("--require-db"), cli("--refresh")
        write(_root / ".gate-tools" / "bin" / ("osv-scanner.exe" if os.name == "nt" else "osv-scanner"), "")
        zip_at(_root / ".gate-tools" / "osv-db", "npm", {})
        _default_red = cli()
    rc_, so_, se_ = _require_db
    record("CLI --require-db (the ci.yml check step): no pinned binary -> exit 1, the FAIL on stderr",
           _git_ok and rc_ == 1 and "FAIL: the pinned osv-scanner is not installed" in se_ and "[G17 OSV]" not in so_)
    rc_, so_, se_ = _default
    record("CLI default (the lefthook leg): no pinned binary -> exit 0, the skip notice on stdout",
           rc_ == 0 and "the OSV advisory scan is skipped here" in so_ and "[G17 OSV]" not in se_)
    rc_, so_, se_ = _refresh
    record("CLI --refresh (the ci.yml download step): no pinned binary -> exit 0, the notice on stdout",
           rc_ == 0 and "nothing refreshed" in so_ and "[G17 OSV]" not in se_)
    rc_, so_, se_ = _default_red
    record("CLI default (the lefthook leg): a binary in .gate-tools/bin and a partial npm database -> exit 1, the FAIL "
           "on stderr (the leg reds)", rc_ == 1 and "is partial" in se_ and "[G17 OSV]" not in so_)

# --- binary legs: the pinned scanner against the one-record databases --------------------------------------------------
BIN = m.osv_binary()
if BIN is None:
    print("[g24-osv-advisories] SKIP binary legs - the pinned osv-scanner is not in .gate-tools/bin (run "
          "scripts/install-gate-tools); the L4 gate-tooling canary installs it and runs them for real.")
else:
    with tempfile.TemporaryDirectory() as _td:
        T = Path(_td)
        db = fresh_record(fixture_db(T / "db", "osv-fixture-undici.json", "osv-fixture-pypi.json"))
        db_pnpm = fresh_record(fixture_db(T / "db-pnpm", "osv-fixture-pnpm.json", "osv-fixture-pypi.json"))
        db_yaml = fresh_record(fixture_db(T / "db-yaml", "osv-fixture-undici.json", "osv-fixture-pyyaml.json"))
        ign0 = ignores_file(T / "ign0")
        _n = iter(range(1000))

        def gate(*, db_dir: Path = db, ign: Path = ign0, require_db: bool = True, **root_kw: object) -> tuple[int, list[str]]:
            root, tracked = scratch_root(T / f"r{next(_n)}", **root_kw)
            return m.check(binary=BIN, root=root, ignores_file=ign, db_dir=db_dir, require_db=require_db,
                           tracked=tracked)

        for _mode, _rq in CHECK_MODES:
            rc_, out_ = gate(undici="7.28.0", require_db=_rq)
            record(f"binary, mode {_mode}: the incident replay - a lock pinning undici 7.28.0 -> exit 1 naming "
                   f"{GHSA}", rc_ == 1 and any(GHSA in ln and "undici@7.28.0" in ln for ln in out_))
            rc_, out_ = gate(ci=("certifi==2026.7.22",), require_db=_rq)
            record(f"binary, mode {_mode}: the certifi replay - requirements-ci.txt pinning certifi 2026.7.22 (with "
                   f"--hash lines) against the one-record PyPI database -> exit 1 naming {CERTIFI}",
                   rc_ == 1 and any(CERTIFI in ln and "certifi@2026.7.22 (requirements-ci.txt)" in ln for ln in out_))
            rc_, out_ = gate(db_dir=db_pnpm, pin="pnpm@10.13.1", require_db=_rq)
            record(f"binary, mode {_mode}: the pnpm pin replay - packageManager pnpm@10.13.1 against the one-record "
                   f"pnpm database -> exit 1 naming {PNPM_ID} and package.json packageManager",
                   rc_ == 1 and any(PNPM_ID in ln and "pnpm@10.13.1 (package.json packageManager)" in ln for ln in out_))
        rc_, out_ = gate()
        record("binary: undici 7.29.1, certifi 2026.10.23 and pyyaml 6.0.3 (the fixed versions) -> exit 0 over every "
               "source", rc_ == 0 and out_[-1].startswith("OK"))
        rc_, out_ = gate(undici="8.8.0")
        record("binary: undici 8.8.0 (the second affected range) -> exit 1", rc_ == 1 and any(GHSA in ln for ln in out_))
        rc_, out_ = gate(db_dir=db_pnpm, pin="pnpm@10.13.1" + _SHA)
        record("binary: the pin in corepack's +sha512 form, pnpm@10.13.1+sha512.<hex> -> exit 1 (scanned at its "
               "version, not skipped)", rc_ == 1 and any(PNPM_ID in ln for ln in out_))
        rc_, out_ = gate(db_dir=db_pnpm, pin="pnpm@10.34.7")
        record("binary: the pin at pnpm 10.34.7 against the pnpm database -> exit 0", rc_ == 0)
        rc_, out_ = gate(db_dir=db_yaml, yamllint=("PyYAML==5.3.1",))
        record(f"binary: a mixed-case PyPI name, PyYAML==5.3.1 in requirements-yamllint.txt -> exit 1 naming {PYYAML_ID} "
               "(the scanner matches the record's pyyaml)", rc_ == 1 and any(PYYAML_ID in ln for ln in out_))
        rc_, out_ = gate(db_dir=db_yaml, yamllint=("PyYAML==5.4",))
        record("binary: PyYAML==5.4 (the fixed version) -> exit 0", rc_ == 0)
        with expected_count(1):
            rc_held, _ = gate(db_dir=db_yaml, yamllint=("PyYAML==5.3.1",),
                              ign=ignores_file(T / "held-yaml", (PYYAML_ID, "reviewed", "pyyaml@5.3.1")))
            rc_alias, _ = gate(ci=("certifi==2026.7.22",),
                               ign=ignores_file(T / "alias", (PYSEC, "reviewed: not reachable", "certifi@2026.7.22")))
            rc_moved, out_moved = gate(ci=("certifi==2026.7.21",),
                                       ign=ignores_file(T / "moved", (PYSEC, "reviewed", "certifi@2026.7.22")))
            rc_stale, out_stale = gate(ign=ignores_file(T / "stale", (GHSA, "reviewed")))
        record("binary: held_by pyyaml@5.3.1 accepts the PyYAML==5.3.1 finding (PEP 503) -> exit 0", rc_held == 0)
        record("binary: an entry naming the PYSEC alias, held_by certifi@2026.7.22, accepts the GHSA finding -> exit 0",
               rc_alias == 0)
        record("binary: the same entry once requirements-ci.txt moved to certifi 2026.7.21 (still affected) -> exit 1, "
               "the holder moved", rc_moved == 1 and any("accepted only on another pin" in ln for ln in out_moved))
        record("binary: an entry whose advisory the scan does not report -> exit 1 (stale)",
               rc_stale == 1 and any("stale ignore" in ln for ln in out_stale))
        # [Test-Change: push 4 C2a - old-obsolete+new-correct, build-gates G17 row]
        # old: the committed pnpm-lock.yaml alone against the undici database; new: this tree as the gate reads it
        # (the census over git, every source and the pin) against all four fixture records.
        rc_, out_ = m.check(binary=BIN, root=ROOT, ignores_file=ign0, db_dir=fresh_record(fixture_db(
            T / "db-all", *FIXTURES)), require_db=True)
        record("binary: this tree (its census over git, every source and the pin) -> exit 0 against all four fixture "
               "records", rc_ == 0 and out_[-1].startswith("OK"))
        _planted, _pt = scratch_root(T / "planted", undici="7.28.0")
        write(_planted / "osv-scanner.toml", f'[[IgnoredVulns]]\nid = "{GHSA}"\nreason = "planted"\n')
        rc_, out_ = m.check(binary=BIN, root=_planted, ignores_file=ign0, db_dir=db, require_db=True, tracked=_pt)
        record("binary: a planted osv-scanner.toml ignoring the GHSA in the root is never read -> still exit 1",
               rc_ == 1 and any(GHSA in ln for ln in out_))
        _empty, _et = scratch_root(T / "empty")
        write(_empty / "pnpm-lock.yaml", _EMPTY_LOCK)
        rc_, out_ = m.check(binary=BIN, root=_empty, ignores_file=ign0, db_dir=db, require_db=True, tracked=_et)
        # [Test-Change: push 4 C2a - old-obsolete+new-correct, build-gates G17 row]
        # old: a lockfile without packages, scanned alone, -> exit 1 on the scanner's 128; new: scanned beside the
        # other sources the scanner exits 0 on it, and the --all-packages count reds it by name.
        record("binary: an emptied pnpm-lock.yaml beside the other sources -> exit 1 naming it (the scanner skips it and "
               "exits 0, no 128 any more)", rc_ == 1 and any("read no package from" in ln and "pnpm-lock.yaml" in ln
                                                            for ln in out_))
        # the scanner's own verdicts that make the check's order load-bearing (raw argv, no check around it)
        _cfg = write(T / "empty-config.toml", "")
        _l728 = write(T / "l728" / "pnpm-lock.yaml", _LOCK.format(v="7.28.0"))
        _zero = zip_at(T / "zero", "npm", {})
        _raw_zero = subprocess.run(m.scan_argv(BIN, [("pnpm-lock.yaml", _l728)], _cfg),
                                   env=m.child_env(dict(os.environ), _zero, offline=True),
                                   capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=120)
        record("binary (raw scanner): a zero-member database passes the incident lock (exit 0) - db_state is load-bearing",
               _raw_zero.returncode == 0)
        _raw_absent = subprocess.run(m.scan_argv(BIN, [("pnpm-lock.yaml", _l728)], _cfg),
                                     env=m.child_env(dict(os.environ), T / "none", offline=True),
                                     capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=120)
        record("binary (raw scanner): no database -> exit 127 while stdout still reads clean - the exit code comes first",
               _raw_absent.returncode == 127 and m.findings(_raw_absent.stdout) == [])

failed = [n for n, ok in results if not ok]
print(f"\n[g24-osv-advisories] {len(results) - len(failed)}/{len(results)} assertions passed (G17).")
sys.exit(1 if failed else 0)
