#!/usr/bin/env python3
"""g24-js-advisories.py - G24 self-test for check-js-advisories (P4.96, G17's JS half).

Replays the undici GHSA-4cwx-7wf7-3272 incident (CVE-2026-13697; affected 7.0.0 to 7.29.0 and 8.0.0 to 8.9.0,
the fixed versions excluded) against a ONE-RECORD OSV database built in a temp dir from the committed record
scripts/gate-selftests/osv-fixture-undici.json, so the planted positive never depends on a live database.

Pure legs (every plane): loading the script runs no entry point, the ignore-file grammar and its growth guard,
the verdict evaluation (id and alias matches, stale entries), the scanner exit-code set, the JSON reader, the
database test (absent, not a zip, zero members, directory members only, a CRC failure, a corrupt deflate stream,
ok), the argv and child environment, the scanner lookup (the pinned binary under either name), the mode
postures that stop before the scanner runs (an unreadable ignore file among them), the child each mode starts
(argv, its own empty temp cwd, the closed proxy for the check only, the timeout; a recording fake stands in for
subprocess), the check's fail-closed arms after the database test (the scanner's finding, an exit code outside
its verdicts, a scan that does not finish, a scanner that cannot start), main(): each flag reaches its mode and
main hands the found binary, LOCK and DB_DIR on (the lookup and DB_DIR patched, so these legs never read this
plane's .gate-tools), and the script run as a process from a scratch root: each mode's exit code and stream,
which lefthook and ci.yml read, the default mode's red included (a partial database beside the pinned binary's
file). A check leg whose contract holds in every mode runs in both check modes (CHECK_MODES: the default, which
lefthook runs, and --require-db, ci.yml's check step), so an exit 0 or a child that only one mode gets reds a
leg. Whether this plane has an osv-scanner on PATH is never trusted: every leg that runs without the pinned
binary plants one first on PATH, so a PATH binary is never used at the lookup, in main(), check() and
refresh(), or by the process. Binary legs run the PINNED osv-scanner from .gate-tools/bin and skip with a notice
when it is absent (a dev box that did not run install-gate-tools - the g24-gitleaks precedent); the L4
gate-tooling job installs it on all three OS before the canary, so they run there.
stdlib-only. Exit 0 = all held (binary legs possibly skipped with the notice); 1 = a self-test failed.
"""
import contextlib
import importlib.machinery
import importlib.util
import io
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import types
import zipfile
from collections.abc import Callable
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check-js-advisories"
FIXTURE = ROOT / "scripts" / "gate-selftests" / "osv-fixture-undici.json"
GHSA, CVE = "GHSA-4cwx-7wf7-3272", "CVE-2026-13697"
# check()'s two modes, (leg label, require_db): the default is lefthook's pre-push leg, --require-db ci.yml's check step
CHECK_MODES = (("default", False), ("--require-db", True))
results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


_loader = importlib.machinery.SourceFileLoader("cja", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cja", _loader))
_exited_on_import = False
try:
    _loader.exec_module(m)
except SystemExit:                    # an entry point run on import would end this self-test with main()'s exit code
    _exited_on_import = True
record("loading check-js-advisories as a module runs no entry point (its `__main__` guard holds, no SystemExit)",
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


def write(path: Path, text: str) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("utf-8"))
    return path


def lock_for(d: Path, version: str) -> Path:
    return write(d / f"lock-{version}" / "pnpm-lock.yaml", _LOCK.format(v=version))


def fixture_db(d: Path, *, subdir: str = "osv-scalibr") -> Path:
    """<d>/<subdir>/npm/all.zip holding the committed one-record fixture; returns the cache dir `d`."""
    z = d / subdir / "npm" / "all.zip"
    z.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(z, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.writestr(f"{GHSA}.json", FIXTURE.read_bytes())
    return d


def zip_at(d: Path, members: dict[str, bytes], compression: int = zipfile.ZIP_DEFLATED) -> Path:
    z = m.db_zip(d)
    z.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(z, "w", compression) as zf:
        for name, data in members.items():
            zf.writestr(name, data)
    return d


def ignores_file(d: Path, *entries: tuple[str, str]) -> Path:
    body = "schema_version = 1\n" + "".join(f'\n[[ignore]]\nid = "{i}"\nreason = "{r}"\n' for i, r in entries)
    return write(d / "osv-ignores.toml", body)


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


@contextlib.contextmanager
def osv_scanner_on_path():
    """An empty `osv-scanner` and `osv-scanner.exe` (execute bits set) in a fresh temp dir placed first on PATH (a child
    process inherits it); yields the dir; PATH is restored and the dir removed afterwards. A leg that claims a PATH
    binary is never used plants one instead of trusting this plane to have none (mock the probe, don't trust the
    plane)."""
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


def run_main(argv: list[str], *, binary: Path | None, db_dir: Path) -> tuple[int | str | None, str]:
    """m.main(argv) with the scanner lookup answering `binary`, DB_DIR = `db_dir` and an osv-scanner planted first on
    PATH, all restored afterwards, so the leg never reads this plane's .gate-tools and a PATH scanner main() reached
    would change the leg's result: (the exit code, argparse's SystemExit code included; stdout + stderr). A crash is
    a red leg with its reason, never a pass."""
    real_lookup, real_db = m.osv_binary, m.DB_DIR

    def lookup(bin_dir: Path = m.BIN_DIR) -> Path | None:
        return binary

    m.osv_binary, m.DB_DIR = lookup, db_dir
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
        m.osv_binary, m.DB_DIR = real_lookup, real_db
    return rc, out.getvalue()


def guarded(fn: Callable[..., tuple[int, list[str]]], **kw: object) -> tuple[int | None, list[str]]:
    """fn(**kw) -> (exit code, report lines); an exception it raises becomes (None, [its reason])."""
    try:
        return fn(**kw)
    except Exception as e:                       # a crash is a red leg with its reason, never a pass
        return None, [f"<{fn.__name__} raised {type(e).__name__}: {e}>"]


def without_binary(fn: Callable[..., tuple[int, list[str]]], **kw: object) -> tuple[int | None, list[str]]:
    """guarded(fn, binary=None, **kw) with an osv-scanner planted first on PATH: a no-binary posture holds while a PATH
    scanner exists, not only because this plane has none."""
    with osv_scanner_on_path():
        return guarded(fn, binary=None, **kw)


def state_of(d: Path) -> str:
    """m.db_state(d), or the exception it raised as the red leg's reason (a corrupt database is reported, never a
    crash)."""
    try:
        return m.db_state(d)
    except Exception as e:                       # a crash is a red leg with its reason, never a pass
        return f"<db_state raised {type(e).__name__}: {e}>"


@contextlib.contextmanager
def recording_subprocess(*, stdout: str = "", returncode: int = 0, raises: BaseException | None = None):
    """The gate module's `subprocess` swapped for a recorder (restored afterwards), so no child process starts. Each
    run() call is recorded with what its cwd and its --config file hold at call time (the child's temp dir is gone
    once the call returns); it returns `returncode` with `stdout`, or raises `raises`. Yields the list of calls."""
    calls: list[dict[str, object]] = []

    def run(argv: list[str], **kw: object) -> subprocess.CompletedProcess:
        cwd, env, cfg = kw.get("cwd"), kw.get("env"), after(list(argv), "--config")
        calls.append({"argv": list(argv), "cwd": cwd, "timeout": kw.get("timeout"),
                      "env": dict(env) if isinstance(env, dict) else {},
                      "cwd_files": (sorted(p.name for p in Path(cwd).iterdir())
                                    if isinstance(cwd, (str, os.PathLike)) else None),
                      "config": cfg, "config_bytes": Path(cfg).read_bytes() if cfg and Path(cfg).is_file() else None})
        if raises is not None:
            raise raises
        return subprocess.CompletedProcess(argv, returncode, stdout, "")

    real = m.subprocess
    m.subprocess = types.SimpleNamespace(run=run, TimeoutExpired=subprocess.TimeoutExpired)
    try:
        yield calls
    finally:
        m.subprocess = real


def child_in_own_empty_cwd(call: dict[str, object], argv_of: Callable[[Path, Path, Path], list[str]],
                           binary: Path, lock: Path) -> bool:
    """The recorded child ran argv_of(binary, lock, <its --config file>) from its own temp dir, which held only that
    file, empty, and is gone once the call returned."""
    cwd, cfg = call.get("cwd"), call.get("config")
    return (isinstance(cwd, (str, os.PathLike)) and isinstance(cfg, str)
            and call.get("argv") == argv_of(binary, lock, Path(cfg)) and Path(cfg).parent == Path(cwd)
            and call.get("cwd_files") == [Path(cfg).name] and call.get("config_bytes") == b""
            and not Path(cwd).exists())


# --- the committed ignore file + the fixture record -------------------------------------------------------------
_real_ignores, _real_problems = m.load_ignores(m.IGNORES.read_text(encoding="utf-8"))
record("the committed scripts/osv-ignores.toml parses clean and holds exactly EXPECTED_IGNORE_COUNT entries",
       _real_problems == [] and len(_real_ignores) == m.EXPECTED_IGNORE_COUNT)
record("EXPECTED_IGNORE_COUNT is frozen at 0 (no accepted JS advisory; growth is an owner-acked edit of this pin)",
       m.EXPECTED_IGNORE_COUNT == 0)
_fx_bytes = FIXTURE.read_bytes()
_fx = json.loads(_fx_bytes)
record("the fixture is the trimmed incident record (six keys, the GHSA id, the CVE alias, npm undici), LF-only",
       sorted(_fx) == ["affected", "aliases", "id", "modified", "published", "schema_version"]
       and _fx["id"] == GHSA and _fx["aliases"] == [CVE]
       and {a["package"]["name"] for a in _fx["affected"]} == {"undici"}
       and b"\r" not in _fx_bytes and _fx_bytes.endswith(b"\n"))

# --- load_ignores: the grammar ------------------------------------------------------------------------------------
_ok_text = f'schema_version = 1\n\n[[ignore]]\nid = "{CVE}"\nreason = "not reachable"\n'
record("load_ignores: a well-formed entry within the limit -> {id: reason}, no problem",
       m.load_ignores(_ok_text, limit=1) == ({CVE: "not reachable"}, []))
record("load_ignores: an entry without a reason -> problem naming `reason`",
       any("`reason` is required" in p for p in problems_of(f'schema_version = 1\n[[ignore]]\nid = "{GHSA}"\n')))
record("load_ignores: a blank reason -> problem naming `reason`",
       any("`reason` is required" in p
           for p in problems_of(f'schema_version = 1\n[[ignore]]\nid = "{GHSA}"\nreason = "   "\n')))
record("load_ignores: a duplicate id -> problem",
       any("duplicate id" in p for p in problems_of(
           f'schema_version = 1\n[[ignore]]\nid = "{GHSA}"\nreason = "a"\n[[ignore]]\nid = "{GHSA}"\nreason = "b"\n')))
record("load_ignores: an unknown key (a per-entry `owner_ack`) -> problem",
       any("unknown key" in p and "owner_ack" in p for p in problems_of(
           f'schema_version = 1\n[[ignore]]\nid = "{GHSA}"\nreason = "a"\nowner_ack = "yes"\n')))
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
# the growth guard: the default limit is the frozen count (0) - one valid entry above it reds
_grown = m.load_ignores(_ok_text)[1]
record("load_ignores: one valid entry above EXPECTED_IGNORE_COUNT -> problem naming the pin to raise",
       any("raise EXPECTED_IGNORE_COUNT" in p for p in _grown))
with expected_count(1):
    _raised = m.load_ignores(_ok_text)[1]
record("load_ignores: the same entry with EXPECTED_IGNORE_COUNT raised to 1 -> clean (the reviewed growth path)",
       _raised == [])

# --- evaluate: findings against the ignore ids ---------------------------------------------------------------------
_incident = [(GHSA, (CVE,), "undici", "7.28.0")]
_unignored = m.evaluate(_incident, {})
record("evaluate: an advisory no entry names -> problem naming the id and package@version",
       len(_unignored) == 1 and GHSA in _unignored[0] and "undici@7.28.0" in _unignored[0])
record("evaluate: accepted by its OSV id -> clean", m.evaluate(_incident, {GHSA: "r"}) == [])
record("evaluate: accepted by its CVE alias -> clean", m.evaluate(_incident, {CVE: "r"}) == [])
_stale = m.evaluate([], {GHSA: "r"})
record("evaluate: an entry matching no finding -> stale problem", len(_stale) == 1 and "stale ignore" in _stale[0])
record("evaluate: the same advisory reported twice is one problem",
       len(m.evaluate(_incident + _incident, {})) == 1)

# --- interpret: the exit code before the JSON ----------------------------------------------------------------------
_CLEAN_JSON = '{"results": []}'
_finding_json = json.dumps({"results": [{"source": {"path": "pnpm-lock.yaml", "type": "lockfile"}, "packages": [
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
record("interpret/findings: the measured result shape -> (id, aliases, package, version)",
       m.interpret(1, _finding_json) == ([(GHSA, (CVE,), "undici", "7.28.0")], []))
record("findings: a vulnerability without an id -> ValueError (fail-closed)",
       any("no readable JSON verdict" in p for p in m.interpret(1, _finding_json.replace(f'"id": "{GHSA}", ', ""))[1]))

# --- argv + environment ---------------------------------------------------------------------------------------------
_argv = m.scan_argv(Path("osv"), Path("pnpm-lock.yaml"), Path("cfg.toml"))
record("scan_argv: `scan source --offline --format json --config <empty> --lockfile <lock>`, no download flag",
       _argv[1:4] == ["scan", "source", "--offline"] and after(_argv, "--config") == "cfg.toml"
       and after(_argv, "--lockfile") == "pnpm-lock.yaml" and after(_argv, "--format") == "json"
       and not any("download" in a or a == "--offline-vulnerabilities" for a in _argv))
_rargv = m.refresh_argv(Path("osv"), Path("pnpm-lock.yaml"), Path("cfg.toml"))
# no binary leg runs the download (it needs the network), so this leg holds the refresh's whole spelling
record("refresh_argv: exactly `scan source --offline-vulnerabilities --download-offline-databases --format json "
       "--config <empty> --lockfile <lock>` (it downloads, still with the empty --config)",
       _rargv == ["osv", "scan", "source", "--offline-vulnerabilities", "--download-offline-databases", "--format",
                  "json", "--config", "cfg.toml", "--lockfile", "pnpm-lock.yaml"])
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

# --- db_state, the scanner lookup and the mode postures that stop before the scanner ----------------------------------
with tempfile.TemporaryDirectory() as _td:
    T = Path(_td)
    record("db_state: no zip -> absent", m.db_state(T / "none") == "absent")
    record("db_state: a zip at the unmeasured osv-scanner/npm/all.zip layout -> absent (the scanner reads osv-scalibr/)",
           m.db_state(fixture_db(T / "wrong-layout", subdir="osv-scanner")) == "absent")
    write(m.db_zip(T / "notzip"), "PK but not a zip\n")
    record("db_state: bytes that are not a zip -> partial", m.db_state(T / "notzip") == "partial")
    record("db_state: a zero-member zip -> partial (the scanner passes it silently)",
           m.db_state(zip_at(T / "zero", {})) == "partial")
    record("db_state: a zip holding only a directory entry -> partial",
           m.db_state(zip_at(T / "dirs", {"npm/": b""})) == "partial")
    _crc = zip_at(T / "crc", {"x.json": b'{"id": "X-1"}' * 20}, zipfile.ZIP_STORED)
    _raw = m.db_zip(_crc).read_bytes()
    _at = _raw.index(b'{"id": "X-1"}')
    m.db_zip(_crc).write_bytes(_raw[:_at] + b"[" + _raw[_at + 1:])
    record("db_state: a member whose CRC fails -> partial", m.db_state(_crc) == "partial")
    _inflate = zip_at(T / "inflate", {"x.json": b'{"id": "X-1"}' * 20})
    _zraw = bytearray(m.db_zip(_inflate).read_bytes())
    with zipfile.ZipFile(m.db_zip(_inflate)) as _zf:
        _off = _zf.getinfo("x.json").header_offset
    _name_len, _extra_len = struct.unpack("<HH", bytes(_zraw[_off + 26:_off + 30]))
    _zraw[_off + 30 + _name_len + _extra_len] = 0x07    # BFINAL=1, BTYPE=11 (reserved): inflating raises zlib.error
    m.db_zip(_inflate).write_bytes(bytes(_zraw))
    record("db_state: a member whose deflate stream is corrupt -> partial (the inflater's zlib.error is caught)",
           state_of(_inflate) == "partial")
    record("db_state: the one-record fixture database -> ok", m.db_state(fixture_db(T / "fx")) == "ok")

    for _name in ("osv-scanner", "osv-scanner.exe"):
        _pinned_dir = T / f"bin-{_name}"
        write(_pinned_dir / _name, "")
        record(f"osv_binary: a bin dir holding the pinned {_name} -> that path",
               m.osv_binary(_pinned_dir) == _pinned_dir / _name)
    with osv_scanner_on_path() as _plant:
        _path_hit = shutil.which("osv-scanner")
        _lookup = m.osv_binary(T / "no-bin")
    record("osv_binary (non-vacuity): a PATH lookup finds the osv-scanner planted first on PATH",
           _path_hit is not None and Path(_path_hit).parent == _plant)
    record("osv_binary: an empty bin dir -> None with an osv-scanner first on PATH (a PATH binary is never used)",
           _lookup is None)
    _lock = lock_for(T, "7.28.0")
    _ign = ignores_file(T / "ign")
    _never_run = T / "never-run.exe"          # the postures below return before any scanner is executed
    # every no-binary posture below runs with an osv-scanner planted first on PATH (without_binary)
    rc_, out_ = without_binary(m.check, lock=_lock, ignores_file=_ign, db_dir=T / "none", require_db=False)
    record("mode default: no pinned binary -> exit 0 with the notice naming install-gate-tools",
           rc_ == 0 and "NOTICE" in out_[0] and "install-gate-tools" in out_[0])
    rc_, out_ = without_binary(m.check, lock=_lock, ignores_file=_ign, db_dir=T / "none", require_db=True)
    record("mode --require-db: no pinned binary -> exit 1 naming the absent pinned binary",
           rc_ == 1 and "FAIL: the pinned osv-scanner is not installed" in out_[0])
    rc_, out_ = m.check(binary=_never_run, lock=_lock, ignores_file=_ign, db_dir=T / "none", require_db=False)
    record("mode default: no local database -> exit 0 with the notice naming --refresh (never silent)",
           rc_ == 0 and "NOTICE" in out_[0] and "--refresh" in out_[0])
    rc_, out_ = m.check(binary=_never_run, lock=_lock, ignores_file=_ign, db_dir=T / "none", require_db=True)
    record("mode --require-db: no database -> exit 1", rc_ == 1 and "no OSV npm database" in out_[0])
    rc_, out_ = m.check(binary=_never_run, lock=_lock, ignores_file=_ign, db_dir=T / "zero", require_db=True)
    record("mode --require-db: a zero-member database -> exit 1 (partial)", rc_ == 1 and "partial" in out_[0])
    rc_, out_ = m.check(binary=_never_run, lock=_lock, ignores_file=_ign, db_dir=T / "zero", require_db=False)
    record("mode default: a partial database -> exit 1 too (not excusable as absent)", rc_ == 1 and "partial" in out_[0])
    # the arms before the scanner that fail whatever the mode, each red in both check modes (under --require-db an arm
    # that let the input through would still end in another FAIL, so each leg needs its own arm's message)
    _grown_file = ignores_file(T / "grown", (CVE, "reason"))
    for _mode, _rq in CHECK_MODES:
        rc_, out_ = without_binary(m.check, lock=_lock, ignores_file=_grown_file, db_dir=T / "none", require_db=_rq)
        record(f"mode {_mode}: a grown ignore file reds even with no binary and no database",
               rc_ == 1 and any("raise EXPECTED_IGNORE_COUNT" in ln for ln in out_))
        rc_, out_ = without_binary(m.check, lock=_lock, ignores_file=T / "absent.toml", db_dir=T / "none",
                                   require_db=_rq)
        record(f"mode {_mode}: an unreadable ignore file (absent) -> exit 1 even with no binary and no database",
               rc_ == 1 and "cannot read" in out_[0])
        rc_, out_ = guarded(m.check, binary=_never_run, lock=T / "no-lock.yaml", ignores_file=_ign, db_dir=T / "fx",
                            require_db=_rq)
        record(f"mode {_mode}: pnpm-lock.yaml missing -> exit 1", rc_ == 1 and "is missing" in out_[0])
    rc_, out_ = without_binary(m.refresh, lock=_lock, db_dir=T / "none")
    record("mode --refresh: no pinned binary -> exit 0 with the notice, nothing refreshed (fail-open download step)",
           rc_ == 0 and "NOTICE" in out_[0] and "nothing refreshed" in out_[0])

# --- the scanner's child: what check and refresh start, and how each answers a child that fails ---------------------
with tempfile.TemporaryDirectory() as _td:
    T = Path(_td)
    _db = fixture_db(T / "db")                 # `ok`, so the check reaches the scanner
    _lock = lock_for(T, "7.28.0")
    _ign = ignores_file(T / "ign")
    _bin = T / "osv-scanner.exe"               # recorded by the fake below, never started
    _closed = "http://127.0.0.1:9"
    _proxies = ("HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy")
    for _mode, _rq in CHECK_MODES:            # the one scan child, whatever the mode
        with recording_subprocess(stdout='{"results": []}') as _calls:
            rc_, out_ = guarded(m.check, binary=_bin, lock=_lock, ignores_file=_ign, db_dir=_db, require_db=_rq)
        _c = _calls[0] if len(_calls) == 1 else {}
        record(f"check, mode {_mode}: one scan child, scan_argv with an empty --config file alone in the child's own "
               "temp cwd -> exit 0",
               rc_ == 0 and len(_calls) == 1 and child_in_own_empty_cwd(_c, m.scan_argv, _bin, _lock))
        _cenv = _c.get("env", {})
        record(f"check, mode {_mode}: the scan child gets the closed proxy on every HTTP(S) proxy variable, no "
               "NO_PROXY or no_proxy, the database dir and SCAN_TIMEOUT",
               isinstance(_cenv, dict) and all(_cenv.get(v) == _closed for v in _proxies)
               and not {"NO_PROXY", "no_proxy"} & set(_cenv) and _c.get("timeout") == m.SCAN_TIMEOUT
               and _cenv.get("OSV_SCANNER_LOCAL_DB_CACHE_DIRECTORY") == str(_db))
    with recording_subprocess(stdout='{"results": []}') as _calls:
        rc_, out_ = guarded(m.refresh, binary=_bin, lock=_lock, db_dir=T / "refresh-db")
    _c = _calls[0] if len(_calls) == 1 else {}
    _renv = _c.get("env", {})
    record("refresh: one download child, refresh_argv with an empty --config file alone in its own temp cwd -> exit 0",
           rc_ == 0 and len(_calls) == 1 and child_in_own_empty_cwd(_c, m.refresh_argv, _bin, _lock))
    record("refresh: the download child keeps the network (no proxy variable is the closed port), the database dir "
           "and REFRESH_TIMEOUT",
           isinstance(_renv, dict) and not any(_renv.get(v) == _closed for v in _proxies)
           and _c.get("timeout") == m.REFRESH_TIMEOUT
           and _renv.get("OSV_SCANNER_LOCAL_DB_CACHE_DIRECTORY") == str(T / "refresh-db"))
    with recording_subprocess(raises=subprocess.TimeoutExpired(["osv-scanner"], m.REFRESH_TIMEOUT)):
        rc_, out_ = guarded(m.refresh, binary=_bin, lock=_lock, db_dir=T / "refresh-db")
    record("refresh: a download that does not finish -> exit 0 with the failure notice (fail-open)",
           rc_ == 0 and any("the download failed (fail-open)" in ln for ln in out_))
    # the check's fail-closed arms after the database test, each red in both check modes: the scanner's finding (the
    # incident as lefthook's leg meets it), an exit code outside its verdicts, and a scanner that never gives one
    for _mode, _rq in CHECK_MODES:
        with recording_subprocess(stdout=_finding_json, returncode=1):
            rc_, out_ = guarded(m.check, binary=_bin, lock=_lock, ignores_file=_ign, db_dir=_db, require_db=_rq)
        record(f"check, mode {_mode}: the scanner reports the incident advisory (exit 1 and its JSON) -> exit 1 naming "
               f"{GHSA} and undici@7.28.0", rc_ == 1 and any(GHSA in ln and "undici@7.28.0" in ln for ln in out_))
        with recording_subprocess(stdout='{"results": []}', returncode=127):
            rc_, out_ = guarded(m.check, binary=_bin, lock=_lock, ignores_file=_ign, db_dir=_db, require_db=_rq)
        record(f"check, mode {_mode}: the scanner exits 127 with a clean-looking result -> exit 1",
               rc_ == 1 and any("exited 127" in ln for ln in out_))
        with recording_subprocess(raises=subprocess.TimeoutExpired(["osv-scanner"], m.SCAN_TIMEOUT)):
            rc_, out_ = guarded(m.check, binary=_bin, lock=_lock, ignores_file=_ign, db_dir=_db, require_db=_rq)
        record(f"check, mode {_mode}: a scan that does not finish -> exit 1 (fail-closed)",
               rc_ == 1 and "did not finish" in out_[0])
        # a real spawn: the absent path fails with OSError, which the check answers fail-closed
        rc_, out_ = guarded(m.check, binary=T / "absent-scanner.exe", lock=_lock, ignores_file=_ign, db_dir=_db,
                            require_db=_rq)
        record(f"check, mode {_mode}: a scanner that cannot be started (a path that does not exist) -> exit 1 "
               "(fail-closed)", rc_ == 1 and "cannot run" in out_[0])

# --- main(): each flag reaches its mode (lefthook runs the default; ci.yml runs --refresh, then --require-db) ---------
with tempfile.TemporaryDirectory() as _td:
    T = Path(_td)
    _no_db = T / "no-db"                  # DB_DIR up to the recorded legs: no database there, never this plane's
    _found = T / "absent-scanner.exe"     # the lookup's found-binary answer: a path that does not exist, no scanner
    rc_, _ = run_main(["--require-db", "--refresh"], binary=None, db_dir=_no_db)
    record("usage: --require-db together with --refresh -> exit 2", rc_ == 2)
    rc_, out_ = run_main([], binary=None, db_dir=_no_db)
    record("main([]): no pinned binary -> exit 0 with the check's skip notice (no flag reaches the default mode)",
           rc_ == 0 and "the JS advisory scan is skipped here" in out_)
    rc_, out_ = run_main(["--require-db"], binary=None, db_dir=_no_db)
    record("main(['--require-db']): no pinned binary -> exit 1 FAIL (the flag reaches the check's require_db)",
           rc_ == 1 and "FAIL: the pinned osv-scanner is not installed" in out_)
    rc_, out_ = run_main(["--refresh"], binary=None, db_dir=_no_db)
    record("main(['--refresh']): no pinned binary -> exit 0 with the refresh notice (the flag reaches refresh)",
           rc_ == 0 and "nothing refreshed" in out_)
    rc_, out_ = run_main([], binary=_found, db_dir=_no_db)
    record("main([]): the found binary and DB_DIR reach the check -> exit 0, the no-database notice naming DB_DIR",
           rc_ == 0 and "NOTICE: no local OSV npm database" in out_ and str(m.db_zip(_no_db)) in out_)
    rc_, out_ = run_main(["--require-db"], binary=_found, db_dir=_no_db)
    record("main(['--require-db']): the found binary and DB_DIR reach the check -> exit 1 naming DB_DIR",
           rc_ == 1 and "FAIL: no OSV npm database" in out_ and str(m.db_zip(_no_db)) in out_)
    # a real spawn: the absent path fails with OSError, which the download step answers fail-open
    rc_, out_ = run_main(["--refresh"], binary=_found, db_dir=_no_db)
    record("main(['--refresh']): the found binary and DB_DIR reach refresh; a failed spawn -> exit 0 naming DB_DIR",
           rc_ == 0 and "the download failed (fail-open)" in out_ and str(m.db_zip(_no_db)) in out_)
    # the recorded legs (no child starts): what main hands each mode's child - the found binary, LOCK and DB_DIR (the
    # check's verdict is left out: it reads the committed ignore file, which the recorded empty scan would call stale
    # once it holds an entry)
    _db = fixture_db(T / "db")            # `ok`, so the check reaches the scanner
    _bin = T / "osv-scanner.exe"          # recorded by the fake, never started
    for _argv in ([], ["--require-db"]):  # both check modes
        with recording_subprocess(stdout='{"results": []}') as _calls:
            run_main(_argv, binary=_bin, db_dir=_db)
        _c = _calls[0] if len(_calls) == 1 else {}
        _cenv = _c.get("env", {})
        record(f"main({_argv!r}): the one scan child gets the found binary and LOCK in its argv, DB_DIR in its env",
               len(_calls) == 1 and child_in_own_empty_cwd(_c, m.scan_argv, _bin, m.LOCK)
               and isinstance(_cenv, dict) and _cenv.get("OSV_SCANNER_LOCAL_DB_CACHE_DIRECTORY") == str(_db))
    with recording_subprocess(stdout='{"results": []}') as _calls:
        rc_, out_ = run_main(["--refresh"], binary=_bin, db_dir=T / "refresh-db")
    _c = _calls[0] if len(_calls) == 1 else {}
    _renv = _c.get("env", {})
    record("main(['--refresh']): the one download child gets the found binary and LOCK in its argv, DB_DIR in its env "
           "-> exit 0",
           rc_ == 0 and len(_calls) == 1 and child_in_own_empty_cwd(_c, m.refresh_argv, _bin, m.LOCK)
           and isinstance(_renv, dict) and _renv.get("OSV_SCANNER_LOCAL_DB_CACHE_DIRECTORY") == str(T / "refresh-db"))

# --- the script as a process: the exit code and stream lefthook (the default) and ci.yml (--refresh, --require-db) read
with tempfile.TemporaryDirectory() as _td:
    T = Path(_td)
    _root = T / "root"                    # a scratch checkout: the script, a lock, an ignore file; .gate-tools comes last
    (_root / "scripts").mkdir(parents=True)
    shutil.copyfile(SCRIPT, _root / "scripts" / "check-js-advisories")
    write(_root / "pnpm-lock.yaml", _LOCK.format(v="7.28.0"))
    ignores_file(_root / "scripts")

    def cli(*flags: str) -> tuple[int | None, str, str]:
        """The copy run as `python3 -P scripts/check-js-advisories <flags>` from the scratch root: (exit code, stdout,
        stderr); a child that cannot start or does not finish is a red leg with its reason, never a pass."""
        try:
            p = subprocess.run([sys.executable, "-P", str(_root / "scripts" / "check-js-advisories"), *flags],
                               cwd=str(_root), capture_output=True, text=True, encoding="utf-8", errors="replace",
                               timeout=60)
        except (OSError, subprocess.TimeoutExpired) as e:
            return None, "", f"<the child failed: {type(e).__name__}: {e}>"
        return p.returncode, p.stdout, p.stderr

    with osv_scanner_on_path():           # the children inherit this PATH: their lookup must not take the plant
        _default, _require_db, _refresh = cli(), cli("--require-db"), cli("--refresh")
        # then the default mode's red as lefthook meets it: a binary in the scratch root's .gate-tools/bin (an empty
        # file under the installed name, never started) beside a partial database, which fails before any scan
        write(_root / ".gate-tools" / "bin" / ("osv-scanner.exe" if os.name == "nt" else "osv-scanner"), "")
        zip_at(_root / ".gate-tools" / "osv-db", {})
        _default_red = cli()
    rc_, so_, se_ = _require_db
    record("CLI --require-db (the ci.yml check step): no pinned binary -> exit 1, the FAIL on stderr",
           rc_ == 1 and "FAIL: the pinned osv-scanner is not installed" in se_ and "[G17 JS]" not in so_)
    rc_, so_, se_ = _default
    record("CLI default (the lefthook leg): no pinned binary -> exit 0, the skip notice on stdout",
           rc_ == 0 and "the JS advisory scan is skipped here" in so_ and "[G17 JS]" not in se_)
    rc_, so_, se_ = _refresh
    record("CLI --refresh (the ci.yml download step): no pinned binary -> exit 0, the notice on stdout",
           rc_ == 0 and "nothing refreshed" in so_ and "[G17 JS]" not in se_)
    rc_, so_, se_ = _default_red
    record("CLI default (the lefthook leg): a binary in .gate-tools/bin and a partial database -> exit 1, the FAIL on "
           "stderr (the leg reds)", rc_ == 1 and "is partial" in se_ and "[G17 JS]" not in so_)

# --- binary legs: the pinned scanner against the one-record database -------------------------------------------------
BIN = m.osv_binary()
if BIN is None:
    print("[g24-js-advisories] SKIP binary legs - the pinned osv-scanner is not in .gate-tools/bin (run "
          "scripts/install-gate-tools); the L4 gate-tooling canary installs it and runs them for real.")
else:
    with tempfile.TemporaryDirectory() as _td:
        T = Path(_td)
        db = fixture_db(T / "db")
        ign0 = ignores_file(T / "ign0")
        l728, l7291, l880 = lock_for(T, "7.28.0"), lock_for(T, "7.29.1"), lock_for(T, "8.8.0")

        def gate(lock: Path, ign: Path = ign0) -> tuple[int, list[str]]:
            return m.check(binary=BIN, lock=lock, ignores_file=ign, db_dir=db, require_db=True)

        rc_, out_ = gate(l728)
        record("binary: the incident replay - a lock pinning undici 7.28.0 -> exit 1 naming GHSA-4cwx-7wf7-3272",
               rc_ == 1 and any(GHSA in ln and "undici@7.28.0" in ln for ln in out_))
        rc_, out_ = gate(l7291)
        record("binary: undici 7.29.1 (the fixed version) -> exit 0", rc_ == 0 and out_[0].startswith("OK"))
        rc_, out_ = gate(l880)
        record("binary: undici 8.8.0 (the second affected range) -> exit 1", rc_ == 1 and any(GHSA in ln for ln in out_))
        rc_, out_ = gate(m.LOCK)
        record("binary: the committed pnpm-lock.yaml -> exit 0 against the fixture database", rc_ == 0)
        with expected_count(1):
            rc_alias, _ = gate(l728, ignores_file(T / "alias", (CVE, "reviewed: not reachable")))
            rc_stale, out_stale = gate(l7291, ignores_file(T / "stale", (GHSA, "reviewed")))
        record("binary: an entry naming the CVE alias with a reason accepts the GHSA finding -> exit 0", rc_alias == 0)
        record("binary: an entry whose advisory the scan does not report -> exit 1 (stale)",
               rc_stale == 1 and any("stale ignore" in ln for ln in out_stale))
        write(l728.parent / "osv-scanner.toml", f'[[IgnoredVulns]]\nid = "{GHSA}"\nreason = "planted"\n')
        rc_, out_ = gate(l728)
        record("binary: a planted osv-scanner.toml ignoring the GHSA beside the lock is never read -> still exit 1",
               rc_ == 1 and any(GHSA in ln for ln in out_))
        rc_, out_ = gate(write(T / "empty" / "pnpm-lock.yaml", _EMPTY_LOCK))
        record("binary: a lockfile without packages -> exit 1 (the scanner's 128)", rc_ == 1 and any("128" in ln for ln in out_))
        # the scanner's own verdicts that make the check's order load-bearing (raw argv, no check around it)
        _cfg = write(T / "empty-config.toml", "")
        _zero = zip_at(T / "zero", {})
        _raw_zero = subprocess.run(m.scan_argv(BIN, l728, _cfg), env=m.child_env(dict(os.environ), _zero, offline=True),
                                   capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=120)
        record("binary (raw scanner): a zero-member database passes the incident lock (exit 0) - db_state is load-bearing",
               _raw_zero.returncode == 0)
        _raw_absent = subprocess.run(m.scan_argv(BIN, l728, _cfg), env=m.child_env(dict(os.environ), T / "none", offline=True),
                                     capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=120)
        record("binary (raw scanner): no database -> exit 127 while stdout still reads clean - the exit code comes first",
               _raw_absent.returncode == 127 and m.findings(_raw_absent.stdout) == [])

failed = [n for n, ok in results if not ok]
print(f"\n[g24-js-advisories] {len(results) - len(failed)}/{len(results)} assertions passed (G17).")
sys.exit(1 if failed else 0)
