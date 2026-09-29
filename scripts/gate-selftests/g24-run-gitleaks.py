#!/usr/bin/env python3
"""g24-run-gitleaks.py - G24 self-test for the run-gitleaks G2 driver (P0.3.1).

Four parts, all in real throwaway git repos:
  (a) the L2 range-base fallback chain: the base is the first of @{u} -> origin/<branch> ->
      origin/main -> origin/HEAD that resolves, and on a FIRST PUSH (none resolve) resolve_base() is
      None so the driver FULL-scans every commit. No gitleaks binary needed.
  (b) the error-line classifier and the run_scan() verdict (no gitleaks binary): gitleaks logs a
      failure of its own `git` call at level ERR and still exits 0, so error_lines() must flag the
      recorded pinned-version log of that failure and pass a clean scan's log, and run_scan() over a
      stand-in scanner must fail an exit-0 scan closed (2) on an ERR line on either output stream
      while a finding (1) beside an error keeps its 1.
  (c) every mode end-to-end against the planted MINISIGN_SECRET_KEY fixture line (the pinned binary).
      The first legs replay the incident this file was extended for: the range leg passed
      `<base>..HEAD` in the repository slot of `gitleaks git`, scanned 0 commits and exited 0, so an
      unpushed commit carrying a secret went through the pre-push leg unread; the fixed leg must read
      exactly that one commit. Then the pre-fix argv against the same repo (it must fail closed, never
      exit 0), the range bound (a secret already pushed is the history leg's, not the range's), the
      first-push full scan (a key below a clean tip), --staged, --history and --dir.
  (d) the suppression channels outside the growth-guarded .gitleaks.toml allowlist, which let a
      finding through with no caged edit (the planted key line + ` # gitleaks:allow`, staged, passed
      `run-gitleaks --staged` with exit 0). Binary-free: every mode's argv carries
      --ignore-gitleaks-allow and a --gitleaks-ignore-path that no OS can hold (and the committed
      baseline, which every throwaway scan in this file omits: it lists this repository's findings,
      and gitleaks cannot resolve it across volumes), the git version floor of GIT_ATTR_SOURCE, and a
      `git` mode failing closed on an older git, and the `git` modes' environment (the empty tree as
      GIT_ATTR_SOURCE, a core.attributesFile holding `* diff`, appended after any GIT_CONFIG_*
      entries). With the pinned binary, per mode: an inline `gitleaks:allow` on the planted line reds;
      a `.gitleaksignore` listing the finding beside the scan source is refused (exit 2); a
      `.gitleaksignore` in the working directory of a `dir` scan is not read; a nested (uncaged)
      `.gitattributes` `-diff` mark and a NUL byte in the key's file do not hide it from the `git`
      modes (nor the NUL byte from `dir`). Each channel has a control leg on the argv or environment
      that honoured it (exit 0), so no leg can pass on a channel the binary never opened. Last, the
      named residual: the global allowlist of gitleaks' bundled rule set (appended by
      `[extend].useDefault`; the pinned 8.30.1 cannot keep the bundled rules without it) drops a
      finding of every rule by path, and a finding by its secret's value, which reaches the bundled
      rules only (each custom rule reports a fixed capture). Its reach is pinned from both sides - per
      mode, the planted key reds in a plain text path and passes in an `.svg`, and the planted
      MINISIGN_PASSWORD line with `False` in its value reds (its control: the rule reporting the value
      passes it); per path (the `dir` report), the key is reported in each of the repository's own
      text path shapes and in none of ten paths sampled across the list's classes; per value (the
      `dir` report), every planted secret with `False`, `null` or a stopword in its value is reported
      by a custom rule, and a GitHub token the bundled rule reports is not reported with `false`
      spliced in - so a gitleaks bump that moves either list reds a leg.
  Parts (c) and (d)'s binary legs SKIP with a notice if the pinned gitleaks binary is absent (a dev
  box that did not run install-gate-tools); the L4 gate-tooling job installs it before the canary.

stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import contextlib
import importlib.machinery
import importlib.util
import io
import json
import os
import string
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "run-gitleaks"
FIXTURE = ROOT / "scripts" / "gate-selftests" / "gitleaks-fixtures" / "planted-secrets.txt"
_loader = importlib.machinery.SourceFileLoader("rgl", str(SCRIPT))
_spec = importlib.util.spec_from_loader("rgl", _loader)
m = importlib.util.module_from_spec(_spec)
_loader.exec_module(m)

# The throwaway-repo git calls (and the driver's own git + gitleaks runs inside them) get an environment
# WITHOUT the GIT_* location variables a hook plane can carry (GIT_DIR / GIT_INDEX_FILE / GIT_WORK_TREE /
# ...): with one of them set, `git -C <tmp> commit` and gitleaks' `git log` would act on the OUTER
# repository. GIT_EXEC_PATH stays.
for _k in [k for k in os.environ if k.startswith("GIT_") and k != "GIT_EXEC_PATH"]:
    os.environ.pop(_k)

# [Build-Session-Entscheidung: P0.3.1] the throwaway scans run WITHOUT this repository's baseline: a baseline lists
# findings of THIS repository (its paths, its commits), and gitleaks resolves it relative to the scan source, which
# fails across volumes - a Windows runner checks the repository out on D: and makes temp dirs on C:, so gitleaks logged
# `ERR Could not load baseline` beside every throwaway scan and run_scan failed the exit-0 legs closed (the
# 2026-09-28 gate-tooling windows-2022 red of b9aa895). No leg's expectation changes; production scans the repository
# beside its own baseline, and a part (d) leg pins that common_args() passes it.
REAL_BASELINE = m.BASELINE
m.BASELINE = m.CONFIG / "no-baseline.json"          # a child of the config FILE: never exists, so no --baseline-path

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")
    if not ok and detail:
        print("    " + detail.strip().replace("\n", "\n    "))


def git(repo: Path, *args: str) -> None:
    subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, encoding="utf-8", errors="replace", check=True)


def in_repo(repo: Path, fn):
    cwd = os.getcwd()
    os.chdir(repo)
    try:
        return fn()
    finally:
        os.chdir(cwd)


def resolve_base_in(repo: Path):
    return in_repo(repo, m.resolve_base)


def new_repo(path: Path) -> Path:
    path.mkdir(parents=True)
    git(path, "init", "-q", "-b", "main")
    for key, value in (("user.email", "t@t.t"), ("user.name", "t"), ("core.autocrlf", "false"),
                       ("commit.gpgsign", "false"), ("core.hooksPath", str(path / ".no-hooks"))):
        git(path, "config", key, value)
    return path


def commit_file(repo: Path, name: str, text: str, msg: str) -> None:
    (repo / name).write_bytes(text.encode("utf-8"))
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", msg)


def delete_file(repo: Path, name: str, msg: str) -> None:
    git(repo, "rm", "-q", name)
    git(repo, "commit", "-q", "-m", msg)


def with_upstream(td: Path, repo: Path) -> None:
    """A real bare `origin`, the current branch pushed with -u: `@{u}` resolves as on a real pre-push."""
    remote = td / "remote.git"
    subprocess.run(["git", "init", "-q", "--bare", str(remote)], capture_output=True, check=True)
    git(repo, "remote", "add", "origin", remote.as_posix())
    git(repo, "push", "-q", "-u", "origin", "main")


def captured(fn) -> tuple[int, str]:
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(buf):
        rc = fn()
    return rc, buf.getvalue()


def gate(repo: Path, *argv: str) -> tuple[int, str]:
    """run-gitleaks main(argv) with the throwaway repo as the working directory; (exit code, output)."""
    return captured(lambda: in_repo(repo, lambda: m.main(list(argv))))


# --- (a) FIRST PUSH: no upstream, no origin/* -> resolve_base() is None (driver full-scans) -------
with tempfile.TemporaryDirectory() as td:
    repo = Path(td)
    git(repo, "init", "-q", "-b", "main")
    git(repo, "config", "user.email", "t@t.t")
    git(repo, "config", "user.name", "t")
    git(repo, "config", "commit.gpgsign", "false")
    (repo / "a.txt").write_text("x\n", encoding="utf-8")
    git(repo, "add", "-A")
    git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "init")
    record("first push (no upstream / no origin) -> resolve_base() is None (=> full-scan)",
           resolve_base_in(repo) is None)

    # --- with an origin/main ref present -> resolve_base() picks it -------------------------------
    git(repo, "update-ref", "refs/remotes/origin/main", "HEAD")
    record("origin/main present -> resolve_base() == 'origin/main'",
           resolve_base_in(repo) == "origin/main")

# --- (b) the error-line classifier over the pinned gitleaks 8.30.1 log of the incident ----------
# the log of `gitleaks git @{u}..HEAD` (the pre-fix range argv) as the pinned binary writes it: colour-coded
# even into a pipe, a git failure at ERR, then `0 commits scanned` and exit 0
_C = "\x1b[90m4:11PM\x1b[0m "
INCIDENT_LOG = (
    f"{_C}\x1b[31mERR\x1b[0m \x1b[1m[git] fatal: cannot change to '@{{u}}..HEAD': No such file or directory\x1b[0m\n"
    f"{_C}\x1b[31mERR\x1b[0m \x1b[36merror=\x1b[0m\x1b[31m\x1b[1m\"stderr is not empty\"\x1b[0m\x1b[0m\n"
    f"{_C}\x1b[32mINF\x1b[0m \x1b[1m0 commits scanned.\x1b[0m\n"
    f"{_C}\x1b[32mINF\x1b[0m \x1b[1mno leaks found\x1b[0m\n"
)
CLEAN_LOG = (
    "4:12PM INF Unknown SCM platform. Use --platform to include links in findings. host=\n"
    "4:12PM INF 3 commits scanned.\n"
    "4:12PM WRN exhaustive rename detection was skipped due to too many files.\n"
    "4:12PM INF skipped ERR.txt ERR FTL\n"
    "4:12PM WRN leaks found: 1\n"
)
flagged = m.error_lines(INCIDENT_LOG)
record("classifier: the incident log (colour-coded) -> its 2 ERR lines flagged, colour codes stripped",
       len(flagged) == 2 and flagged[0].startswith("4:11PM ERR [git] fatal:") and "\x1b" not in "".join(flagged),
       repr(flagged))
record("classifier: the same log without colour codes -> the same 2 ERR lines",
       m.error_lines(m._ANSI_RE.sub("", INCIDENT_LOG)) == flagged)
record("classifier: a clean scan log (INF/WRN only; `ERR` inside a message) -> no line flagged",
       m.error_lines(CLEAN_LOG) == [], repr(m.error_lines(CLEAN_LOG)))


# run_scan()'s verdict over a stand-in scanner (a Python child in place of the binary): an ERR line on EITHER output
# stream of an exit-0 scan is a failed scan (2) - the pinned binary logs to stderr, and a version that moves its log to
# stdout must not open the hole again - while a finding (exit 1) beside an error keeps its 1
def stand_in(stream: str, code: int) -> list[str]:
    return [sys.executable, "-P", "-c",
            f"import sys; sys.{stream}.write('4:11PM ERR [git] fatal: stand-in\\n'); sys.exit({code})"]


for stream in ("stdout", "stderr"):
    rc, log = captured(lambda: m.run_scan(stand_in(stream, 0)))
    record(f"run_scan: an ERR line on the scanner's {stream} beside exit 0 -> exit 2, never a clean 0",
           rc == 2 and "gitleaks logged an error" in log, f"rc={rc}\n{log}")
rc, log = captured(lambda: m.run_scan(stand_in("stderr", 1)))
record("run_scan: an ERR line beside a finding (exit 1) -> the finding's exit 1 is kept", rc == 1, f"rc={rc}\n{log}")

# --- (d) the suppression channels outside the allowlist: the binary-free legs ---------------------
ARGS = m.common_args()
IGNORE_PATH = ARGS[ARGS.index("--gitleaks-ignore-path") + 1] if "--gitleaks-ignore-path" in ARGS[:-1] else ""
record("common_args: every mode passes --ignore-gitleaks-allow (an inline `gitleaks:allow` comment is not honoured)",
       "--ignore-gitleaks-allow" in ARGS, repr(ARGS))
record("common_args: --gitleaks-ignore-path names a child of the config FILE, a path no OS can hold",
       IGNORE_PATH == str(m.NO_IGNORE_PATH) and Path(IGNORE_PATH).parent == m.CONFIG and m.CONFIG.is_file()
       and not os.path.lexists(IGNORE_PATH), repr(ARGS))
_throwaway_baseline, m.BASELINE = m.BASELINE, REAL_BASELINE
try:
    PROD_ARGS = m.common_args()
finally:
    m.BASELINE = _throwaway_baseline
if REAL_BASELINE.is_file():
    _baseline_ok = ("--baseline-path" in PROD_ARGS[:-1]
                    and PROD_ARGS[PROD_ARGS.index("--baseline-path") + 1] == str(REAL_BASELINE))
else:
    _baseline_ok = "--baseline-path" not in PROD_ARGS
record("common_args: the committed baseline joins as --baseline-path when present (the throwaway scans here omit it)",
       _baseline_ok, repr(PROD_ARGS))
VERSIONS = {"git version 2.41.0": True, "git version 2.53.0.windows.1": True, "git version 3.0.0": True,
            "git version 2.40.1": False, "git version 2.39.5 (Apple Git-154)": False, "git version 1.99.9": False,
            "git version": False, "": False}
record("git version floor: 2.41 and newer read GIT_ATTR_SOURCE; 2.40, Apple's 2.39 and an unparseable line do not",
       all(m.attr_source_supported(text) is want for text, want in VERSIONS.items()),
       repr({text: m.attr_source_supported(text) for text in VERSIONS}))
with tempfile.TemporaryDirectory() as td:
    (Path(td) / ".gitleaksignore").mkdir()
    record("the source-root refusal covers any entry by that name - a directory too (ignore_file_beside names it)",
           m.ignore_file_beside(Path(td)) == Path(td) / ".gitleaksignore")


def config_entry(env: dict[str, str]) -> tuple[dict[str, str], str]:
    """with_config_entry(env) for the attributes file; ({}, reason) when it refuses or raises (a failed leg, never an
    aborted run)."""
    try:
        got, why = m.with_config_entry(env, "core.attributesFile", "/a")
    except ValueError as exc:
        return {}, f"raised {exc!r}"
    return (got, "") if got is not None else ({}, why)


_env0, _ = config_entry({})
_env2, _ = config_entry({"GIT_CONFIG_COUNT": "2", "GIT_CONFIG_KEY_0": "a.b", "GIT_CONFIG_VALUE_0": "1",
                         "GIT_CONFIG_KEY_1": "c.d", "GIT_CONFIG_VALUE_1": "2"})
_bad, _why = config_entry({"GIT_CONFIG_COUNT": "two"})
record("the attributes-file setting joins after the GIT_CONFIG_* entries the environment carries (git applies the last "
       "value of a key), and a GIT_CONFIG_COUNT that is not a count fails closed",
       _env0 == {"GIT_CONFIG_COUNT": "1", "GIT_CONFIG_KEY_0": "core.attributesFile", "GIT_CONFIG_VALUE_0": "/a"}
       and _env2.get("GIT_CONFIG_COUNT") == "3" and _env2.get("GIT_CONFIG_KEY_2") == "core.attributesFile"
       and _env2.get("GIT_CONFIG_KEY_0") == "a.b" and _env2.get("GIT_CONFIG_KEY_1") == "c.d"
       and _bad == {} and "GIT_CONFIG_COUNT" in _why, repr((_env0, _env2, _bad, _why)))
with tempfile.TemporaryDirectory() as td:
    _env, _why = m.attribute_blind_env(Path(td))
    _env = _env or {}
    _empty_tree = subprocess.run(["git", "hash-object", "-t", "tree", "--stdin"], input=b"",
                                 capture_output=True).stdout.decode().strip()
    _n = int(_env.get("GIT_CONFIG_COUNT") or "0") - 1
    _attrs = Path(_env.get(f"GIT_CONFIG_VALUE_{_n}") or Path(td) / "missing")
    record("the `git` modes' environment: GIT_ATTR_SOURCE names the empty tree and core.attributesFile names a file "
           "holding exactly `* diff` (every path diffs as text)",
           _env.get("GIT_ATTR_SOURCE") == _empty_tree and _env.get(f"GIT_CONFIG_KEY_{_n}") == "core.attributesFile"
           and _attrs.is_file() and _attrs.read_bytes() == b"* diff\n",
           f"{_why}\n{ {k: v for k, v in _env.items() if 'GIT' in k} }")

# a `git` mode on a git that ignores GIT_ATTR_SOURCE fails closed before any scan: the stand-in `git` reports 2.40.1
# and the stand-in scanner path must never be started
_real_git, _real_bin = m.git, m.gitleaks_bin
m.git = lambda *a: (0, "git version 2.40.1") if a == ("version",) else _real_git(*a)
m.gitleaks_bin = lambda: "stand-in-scanner-that-must-not-start"
try:
    with tempfile.TemporaryDirectory() as td:
        rc, log = gate(Path(td), "--staged")
except OSError as exc:
    rc, log = -1, f"the scanner was started: {exc}"
finally:
    m.git, m.gitleaks_bin = _real_git, _real_bin
record("a git older than 2.41 (GIT_ATTR_SOURCE unread) -> a `git` mode exits 2 naming it, before any scan",
       rc == 2 and "GIT_ATTR_SOURCE" in log, f"rc={rc}\n{log}")

# --- (c) every mode end-to-end against the planted secret (the pinned binary) ---------------------
GL = m.gitleaks_bin()
if GL is None:
    print("[g24-run-gitleaks] SKIP the end-to-end and per-mode suppression-channel legs - pinned gitleaks binary not "
          "found (run scripts/install-gate-tools); the L4 gate-tooling canary installs it and runs them for real.")
else:
    KEY_LINE = next(line for line in FIXTURE.read_text(encoding="utf-8").splitlines()
                    if line.startswith("MINISIGN_SECRET_KEY="))

    # the incident replay: a pushed base (a real upstream) + ONE unpushed commit carrying the planted key
    with tempfile.TemporaryDirectory() as td:
        repo = new_repo(Path(td) / "work")
        commit_file(repo, "a.txt", "x\n", "base")
        with_upstream(Path(td), repo)
        commit_file(repo, "leak.txt", KEY_LINE + "\n", "unpushed leak")
        record("incident replay: the range base is the upstream `@{u}`, as on a real pre-push",
               resolve_base_in(repo) == "@{u}")
        rc, log = gate(repo, "--range")
        record("incident replay: an unpushed commit carrying the planted MINISIGN_SECRET_KEY line -> --range exit 1",
               rc == 1, f"rc={rc}\n{log}")
        record("incident replay: gitleaks reports `1 commits scanned` - exactly the unpushed commit (the pre-fix "
               "argv read 0), in the log run_scan relays", "INF 1 commits scanned." in m._ANSI_RE.sub("", log),
               f"rc={rc}\n{log}")
        # the pre-fix argv against the same repo: gitleaks reads the range as a repository path, scans 0
        # commits and exits 0; run_scan must turn that into a failed scan
        pre_fix = [GL, "git", "@{u}..HEAD", *m.common_args()]
        rc, log = captured(lambda: in_repo(repo, lambda: m.run_scan(pre_fix)))
        record("pre-fix argv (`gitleaks git @{u}..HEAD`, the range in the repository slot) -> exit 2 naming "
               "the logged error, never a clean 0",
               rc == 2 and "gitleaks logged an error" in log, f"rc={rc}\n{log}")

    # the range bound: the secret was committed, removed and PUSHED; one clean unpushed commit on top
    with tempfile.TemporaryDirectory() as td:
        repo = new_repo(Path(td) / "work")
        commit_file(repo, "leak.txt", KEY_LINE + "\n", "pushed leak")
        delete_file(repo, "leak.txt", "pushed removal")
        with_upstream(Path(td), repo)
        commit_file(repo, "b.txt", "clean\n", "clean unpushed")
        rc, log = gate(repo, "--range")
        record("range bound: a secret only in PUSHED history + a clean unpushed commit -> --range exit 0 "
               "(no error line either: a normal scan with a remote is not failed closed)", rc == 0, f"rc={rc}\n{log}")
        rc, log = gate(repo, "--history")
        record("--history: the same secret, removed at the tip -> exit 1 (the full history is read)",
               rc == 1, f"rc={rc}\n{log}")

    # the first push: no upstream, no origin/* -> the full-scan fallback reads every commit, so the planted key in
    # a commit BELOW a clean tip is still found (a scan of the tip alone would miss it)
    with tempfile.TemporaryDirectory() as td:
        repo = new_repo(Path(td) / "work")
        commit_file(repo, "a.txt", "x\n", "base")
        commit_file(repo, "leak.txt", KEY_LINE + "\n", "leak")
        commit_file(repo, "b.txt", "clean\n", "clean tip")
        rc, log = gate(repo, "--range")
        record("first push: no range base -> the full scan reads every commit (the planted key sits below a clean "
               "tip) -> --range exit 1", rc == 1 and "FULL-scanning" in log, f"rc={rc}\n{log}")

    # --staged: the planted key staged, not committed
    with tempfile.TemporaryDirectory() as td:
        repo = new_repo(Path(td) / "work")
        commit_file(repo, "a.txt", "x\n", "base")
        (repo / "leak.txt").write_bytes((KEY_LINE + "\n").encode("utf-8"))
        git(repo, "add", "leak.txt")
        rc, log = gate(repo, "--staged")
        record("--staged: the planted key in the staged diff -> exit 1", rc == 1, f"rc={rc}\n{log}")

    # --dir: the planted key in the scanned tree (the driver scans ROOT; pointed at the temp tree here)
    with tempfile.TemporaryDirectory() as td:
        tree = Path(td)
        (tree / "leak.txt").write_bytes((KEY_LINE + "\n").encode("utf-8"))
        saved_root, saved_bin = m.ROOT, m.gitleaks_bin
        m.ROOT, m.gitleaks_bin = tree, (lambda: GL)
        try:
            rc, log = gate(tree, "--dir")
        finally:
            m.ROOT, m.gitleaks_bin = saved_root, saved_bin
        record("--dir: the planted key in the scanned tree -> exit 1", rc == 1, f"rc={rc}\n{log}")

    # --- (d) the suppression channels, per mode, against the pinned binary -----------------------------
    GIT_MODES = ("--staged", "--range", "--history")
    ALLOW_LINE = KEY_LINE + "  # gitleaks:allow"
    FINGERPRINT = "leak.txt:minisign-secret-key:1"      # the planted line's global fingerprint in a `git` scan

    def build(td: Path, mode: str, files: dict[str, str]) -> Path:
        """A throwaway repo whose `mode` scan reads `files` (path -> text): staged for --staged, one unpushed
        commit over a pushed base for --range, committed for --history."""
        repo = new_repo(td / "work")
        commit_file(repo, "a.txt", "x\n", "base")
        if mode == "--range":
            with_upstream(td, repo)
        for name, text in files.items():
            (repo / name).parent.mkdir(parents=True, exist_ok=True)
            (repo / name).write_bytes(text.encode("utf-8"))
        git(repo, "add", "-A")
        if mode != "--staged":
            git(repo, "commit", "-q", "-m", "leak")
        return repo

    def dir_gate(tree: Path, cwd: Path) -> tuple[int, str]:
        """run-gitleaks --dir over `tree` (the driver scans ROOT; pointed at the temp tree) from `cwd`."""
        saved_root, saved_bin = m.ROOT, m.gitleaks_bin
        m.ROOT, m.gitleaks_bin = tree, (lambda: GL)
        try:
            return captured(lambda: in_repo(cwd, lambda: m.main(["--dir"])))
        finally:
            m.ROOT, m.gitleaks_bin = saved_root, saved_bin

    def dir_report(tree: Path) -> list[dict[str, object]]:
        """The findings the pinned binary reports (JSON) for a `dir` scan of `tree` (OS-specific absolute paths)."""
        with tempfile.TemporaryDirectory() as rd:
            report = Path(rd) / "report.json"
            subprocess.run([GL, "dir", str(tree), *m.common_args(), "--exit-code", "0", "--report-format", "json",
                            "--report-path", str(report)], capture_output=True)
            return json.loads(report.read_text(encoding="utf-8") or "[]") if report.is_file() else []

    def dir_fingerprints(tree: Path) -> list[str]:
        """The fingerprints the pinned binary reports for a `dir` scan of `tree` (OS-specific absolute paths)."""
        return [str(f["Fingerprint"]) for f in dir_report(tree)]

    def pre_fix_args() -> list[str]:
        """common_args() without the two suppression switches: the flags every mode passed before they joined."""
        args = m.common_args()
        if "--gitleaks-ignore-path" in args[:-1]:
            i = args.index("--gitleaks-ignore-path")
            del args[i:i + 2]
        return [a for a in args if a != "--ignore-gitleaks-allow"]

    def scan_in(repo: Path, cmd: list[str], env: dict[str, str] | None = None) -> tuple[int, str]:
        """run_scan(cmd) in `repo`, in `env` (else this environment: no GIT_ATTR_SOURCE); (exit code, output)."""
        return captured(lambda: in_repo(repo, lambda: m.run_scan(cmd, env)))

    # an inline `gitleaks:allow` on the planted key line - the incident: this line, staged, passed --staged (exit 0)
    for mode in GIT_MODES:
        with tempfile.TemporaryDirectory() as td:
            repo = build(Path(td), mode, {"leak.txt": ALLOW_LINE + "\n"})
            rc, log = gate(repo, mode)
            record(f"inline `gitleaks:allow` on the planted key line: {mode} -> exit 1 (the comment is not honoured)",
                   rc == 1, f"rc={rc}\n{log}")
            if mode == "--staged":
                rc, log = scan_in(repo, [GL, "git", "--staged", *pre_fix_args(), "."])
                record("incident replay: the same staged line under the argv without --ignore-gitleaks-allow -> "
                       "exit 0 (the comment dropped the finding)", rc == 0, f"rc={rc}\n{log}")
    with tempfile.TemporaryDirectory() as td:
        (Path(td) / "leak.txt").write_bytes((ALLOW_LINE + "\n").encode("utf-8"))
        rc, log = dir_gate(Path(td), Path(td))
        record("inline `gitleaks:allow` on the planted key line: --dir -> exit 1", rc == 1, f"rc={rc}\n{log}")

    # a `.gitleaksignore` listing the finding beside the scan source: gitleaks reads it whatever
    # --gitleaks-ignore-path says, so the driver refuses the scan
    for mode in GIT_MODES:
        with tempfile.TemporaryDirectory() as td:
            repo = build(Path(td), mode, {"leak.txt": KEY_LINE + "\n", ".gitleaksignore": FINGERPRINT + "\n"})
            rc, log = gate(repo, mode)
            record(f"a `.gitleaksignore` listing the finding beside the scan source: {mode} -> exit 2, refused before "
                   "the scan", rc == 2 and ".gitleaksignore exists" in log, f"rc={rc}\n{log}")
            if mode == "--staged":
                rc, log = scan_in(repo, m.git_scan(GL, "--staged"))
                record("control: the fixed argv alone still honours `<source>/.gitleaksignore` (exit 0) - no flag "
                       "closes it, the refusal does", rc == 0, f"rc={rc}\n{log}")
    with tempfile.TemporaryDirectory() as td:
        tree = Path(td)
        (tree / "leak.txt").write_bytes((KEY_LINE + "\n").encode("utf-8"))
        fingerprints = dir_fingerprints(tree)
        (tree / ".gitleaksignore").write_bytes("".join(f"{fp}\n" for fp in fingerprints).encode("utf-8"))
        rc, log = dir_gate(tree, tree)
        record("a `.gitleaksignore` listing the finding beside the scan source: --dir -> exit 2, refused before the "
               "scan", rc == 2 and ".gitleaksignore exists" in log and fingerprints != [], f"rc={rc}\n{log}")

    # a `.gitleaksignore` in the working directory of a `dir` scan whose root is elsewhere: read through the
    # --gitleaks-ignore-path default `.`, never through the driver's path
    with tempfile.TemporaryDirectory() as td:
        tree, cwd = Path(td) / "tree", Path(td) / "cwd"
        tree.mkdir()
        cwd.mkdir()
        (tree / "leak.txt").write_bytes((KEY_LINE + "\n").encode("utf-8"))
        fingerprints = dir_fingerprints(tree)
        (cwd / ".gitleaksignore").write_bytes("".join(f"{fp}\n" for fp in fingerprints).encode("utf-8"))
        rc, log = dir_gate(tree, cwd)
        record("a `.gitleaksignore` in the working directory, the scan root elsewhere: --dir -> exit 1 (the ignore "
               "path reads no file)", rc == 1 and fingerprints != [], f"rc={rc}\n{log}")
        rc, log = scan_in(cwd, [GL, "dir", str(tree), *pre_fix_args()])
        record("control: the same scan without --gitleaks-ignore-path (its default `.`) -> exit 0 (the working "
               "directory's file dropped the finding)", rc == 0, f"rc={rc}\n{log}")

    # a nested .gitattributes `-diff` mark (outside the root-anchored cage) on the planted key's file: git writes
    # `Binary files differ` in place of the diff and gitleaks skips it, unless git reads no attributes file
    ATTR_FILES = {"sub/.gitattributes": "leak.txt -diff\n", "sub/leak.txt": KEY_LINE + "\n"}
    for mode in GIT_MODES:
        with tempfile.TemporaryDirectory() as td:
            repo = build(Path(td), mode, ATTR_FILES)
            rc, log = gate(repo, mode)
            record(f"a nested `.gitattributes` `-diff` mark on the planted key's file: {mode} -> exit 1 (git reads "
                   "no attributes file)", rc == 1, f"rc={rc}\n{log}")
            if mode == "--staged":
                rc, log = scan_in(repo, m.git_scan(GL, "--staged"))
                record("control: the same staged scan without GIT_ATTR_SOURCE -> exit 0 (the mark hid the diff)",
                       rc == 0, f"rc={rc}\n{log}")

    # one NUL byte beside the planted key line: git's content check calls the file binary and writes `Binary files
    # differ`, whatever the attributes say, unless every path carries `diff` (core.attributesFile `* diff`)
    NUL_FILES = {"leak.txt": KEY_LINE + "\n\x00\n"}
    for mode in GIT_MODES:
        with tempfile.TemporaryDirectory() as td:
            repo = build(Path(td), mode, NUL_FILES)
            rc, log = gate(repo, mode)
            record(f"a NUL byte in the planted key's file: {mode} -> exit 1 (every path diffs as text)",
                   rc == 1, f"rc={rc}\n{log}")
            if mode == "--staged":
                empty_tree = subprocess.run(["git", "hash-object", "-t", "tree", "--stdin"], input=b"",
                                            capture_output=True).stdout.decode().strip()
                rc, log = scan_in(repo, m.git_scan(GL, "--staged"), {**os.environ, "GIT_ATTR_SOURCE": empty_tree})
                record("control: the same staged scan with GIT_ATTR_SOURCE alone (no attributes file) -> exit 0 (git's "
                       "content check hid the diff)", rc == 0, f"rc={rc}\n{log}")
    with tempfile.TemporaryDirectory() as td:
        (Path(td) / "leak.txt").write_bytes((KEY_LINE + "\n\x00\n").encode("utf-8"))
        rc, log = dir_gate(Path(td), Path(td))
        record("a NUL byte in the planted key's file: --dir -> exit 1", rc == 1, f"rc={rc}\n{log}")

    # the named residual: the global allowlist of gitleaks' bundled rule set (appended by `[extend].useDefault`; the
    # pinned 8.30.1 cannot keep the bundled rules without it) drops a finding of every rule by path, a custom rule's
    # too. Its reach is pinned from both sides, so a gitleaks bump that moves the list reds a leg and the G2 text is
    # re-read. Per mode, through the driver: the planted key reds in a plain text path and passes in an `.svg`.
    for mode in (*GIT_MODES, "--dir"):
        codes: dict[str, tuple[int, str]] = {}
        for name in ("leak.txt", "assets/icon.svg"):
            with tempfile.TemporaryDirectory() as td:
                if mode == "--dir":
                    tree = Path(td)
                    (tree / name).parent.mkdir(parents=True, exist_ok=True)
                    (tree / name).write_bytes((KEY_LINE + "\n").encode("utf-8"))
                    codes[name] = dir_gate(tree, tree)
                else:
                    codes[name] = gate(build(Path(td), mode, {name: KEY_LINE + "\n"}), mode)
        record(f"residual reach: the planted key in `leak.txt` -> {mode} exit 1; in `assets/icon.svg`, a path the "
               "bundled global allowlist names -> exit 0",
               codes["leak.txt"][0] == 1 and codes["assets/icon.svg"][0] == 0, repr(codes))

    # per path, from the pinned binary's `dir` report over one tree: the planted key is reported in each of the
    # repository's own text path shapes (Cargo's lock file and a script named after gitleaks among them) and in none of
    # ten paths sampled across the classes the list names (images, fonts, documents, a JS lock file, a dependency and a
    # vendored tree, Go module files, a build wrapper, and the unanchored `gitleaks\.toml` inside a longer name)
    REPO_TEXT_PATHS = ("leak.txt", "README.md", "docs/spec/00-architecture.md", "src-tauri/src/lib.rs", "src/App.tsx",
                       "Cargo.toml", "Cargo.lock", "package.json", ".github/workflows/ci.yml", "scripts/run-gitleaks",
                       ".env", "tests/corpus/manifest.toml")
    BUNDLED_ALLOWED = ("assets/branding/logo.svg", "src-tauri/icons/icon.png", "design/mock.pdf",
                       "src/styles/font.woff2", "pnpm-lock.yaml", "node_modules/pkg/index.js", "vendor/modules.txt",
                       "go.sum", "gradlew", "docs/gitleaks.toml.md")
    with tempfile.TemporaryDirectory() as td:
        tree = Path(td)
        for name in (*REPO_TEXT_PATHS, *BUNDLED_ALLOWED):
            (tree / name).parent.mkdir(parents=True, exist_ok=True)
            (tree / name).write_bytes((KEY_LINE + "\n").encode("utf-8"))
        files = [str(f.get("File", "")).replace("\\", "/") for f in dir_report(tree)]
        reported = {name for name in (*REPO_TEXT_PATHS, *BUNDLED_ALLOWED) if any(f.endswith("/" + name) for f in files)}
        record(f"residual reach, per path (the `dir` report): the planted key is reported in each of the "
               f"{len(REPO_TEXT_PATHS)} repository text path shapes and in none of the {len(BUNDLED_ALLOWED)} sampled "
               "paths the bundled global allowlist names",
               reported == set(REPO_TEXT_PATHS), repr(sorted(reported)))

    # the same allowlist drops a finding by its secret: one containing `false` in any case or a stopword, or ending in
    # `null` (its first value regex, `(?i)^true|false|null$`, is an unanchored alternation), or a placeholder shape.
    # Each custom rule reports a fixed capture (the key's `RW` prefix, the variable name, `sk-ant-`), which none of them
    # matches. Per mode, through the driver: the planted MINISIGN_PASSWORD line with `False` in its value reds - it
    # passed every mode (exit 0) while the rule reported the value, which the control leg replays.
    def spliced(line: str, word: str) -> str:
        """`line` with `word` inserted six characters before its end, inside the planted secret's value."""
        return line[:-6] + word + line[-6:]

    FIXTURE_LINES = FIXTURE.read_text(encoding="utf-8").splitlines()
    PW_FALSE = spliced(next(line for line in FIXTURE_LINES if line.startswith("MINISIGN_PASSWORD=")), "False")
    for mode in (*GIT_MODES, "--dir"):
        with tempfile.TemporaryDirectory() as td:
            if mode == "--dir":
                (Path(td) / "leak.txt").write_bytes((PW_FALSE + "\n").encode("utf-8"))
                rc, log = dir_gate(Path(td), Path(td))
            else:
                rc, log = gate(build(Path(td), mode, {"leak.txt": PW_FALSE + "\n"}), mode)
        record(f"the planted MINISIGN_PASSWORD line with `False` in its value: {mode} -> exit 1 (the rule reports the "
               "variable name, which no value regex or stopword of the bundled allowlist matches)",
               rc == 1, f"rc={rc}\n{log}")
    CUSTOM_RULES = [r for r in tomllib.loads(m.CONFIG.read_text(encoding="utf-8")).get("rules", [])
                    if isinstance(r, dict)]
    CUSTOM_IDS = {str(r.get("id")) for r in CUSTOM_RULES}
    PW_REGEX = str(next((r.get("regex") for r in CUSTOM_RULES if r.get("id") == "minisign-password-literal"), ""))
    VALUE_REGEX = PW_REGEX.replace("(MINISIGN_PASSWORD)", "MINISIGN_PASSWORD", 1)
    with tempfile.TemporaryDirectory() as td:
        tree = Path(td) / "tree"
        tree.mkdir()
        (tree / "leak.txt").write_bytes((PW_FALSE + "\n").encode("utf-8"))
        value_config = Path(td) / "value-rule.toml"
        value_config.write_bytes((f"[extend]\nuseDefault = true\n\n[[rules]]\nid = \"minisign-password-literal\"\n"
                                  f"regex = '''{VALUE_REGEX}'''\nkeywords = [\"minisign_password\"]\n").encode("utf-8"))
        args = m.common_args()
        args[args.index("--config") + 1] = str(value_config)
        rc, log = scan_in(tree, [GL, "dir", str(tree), *args])
        record("control: the same line under the committed password rule with its name capture removed (it reports the "
               "value) -> exit 0 (the bundled allowlist's `false` dropped it)",
               rc == 0 and VALUE_REGEX != PW_REGEX, f"rc={rc} regex={PW_REGEX!r}\n{log}")

    # per value, from the pinned binary's `dir` report over one tree: every planted secret of the fixture with `False`
    # spliced into its value, with `null` appended and with the alphabet stopword spliced in is reported by a custom
    # rule, and the custom rules reporting them are exactly the config's (a custom rule that reports its value, or has
    # no planted secret, reds here); a GitHub token built here is reported by the bundled rule and, with `false` spliced
    # in, is not - the value side of the residual, which reaches the bundled rules
    def section(lines: list[str], start: str, stop: str) -> list[str]:
        """The non-comment, non-blank lines between the fixture header lines starting `start` and `stop`."""
        i = next((n for n, line in enumerate(lines) if line.startswith(start)), len(lines))
        j = next((n for n, line in enumerate(lines) if line.startswith(stop)), len(lines))
        return [line for line in lines[i + 1:j] if line.strip() and not line.startswith("#")]

    SECRET_LINES = section(FIXTURE_LINES, "# === SECRETS THAT MUST BE CAUGHT", "# === LOOK-ALIKES")
    VARIANTS = {f"secret{i}-{tag}.txt": text for i, line in enumerate(SECRET_LINES)
                for tag, text in (("false", spliced(line, "False")), ("null", line + "null"),
                                  ("stopword", spliced(line, string.ascii_lowercase)))}
    TOKEN = (string.ascii_letters + string.digits)[::-1][:36]         # 36 distinct characters, no stopword run
    TOKENS = {"token.txt": "gh" + "p_" + TOKEN, "token-false.txt": "gh" + "p_" + TOKEN[:15] + "false" + TOKEN[20:]}
    with tempfile.TemporaryDirectory() as td:
        tree = Path(td)
        for name, text in {**VARIANTS, **TOKENS}.items():
            (tree / name).write_bytes((text + "\n").encode("utf-8"))
        rules_by_file: dict[str, set[str]] = {}
        for finding in dir_report(tree):
            name = str(finding.get("File", "")).replace("\\", "/").rsplit("/", 1)[-1]
            rules_by_file.setdefault(name, set()).add(str(finding.get("RuleID", "")))
    missed = sorted(name for name in VARIANTS if not rules_by_file.get(name, set()) & CUSTOM_IDS)
    reporting = set().union(*(rules_by_file.get(name, set()) & CUSTOM_IDS for name in VARIANTS))
    record(f"residual reach, per value (the `dir` report): each of the {len(SECRET_LINES)} planted secrets with "
           "`False` spliced in, `null` appended or a stopword spliced in is reported by a custom rule, and every "
           "custom rule reports one (each reports a fixed capture)",
           SECRET_LINES != [] and missed == [] and reporting == CUSTOM_IDS,
           f"missed={missed} reporting={sorted(reporting)} custom={sorted(CUSTOM_IDS)}")
    record("residual reach, per value: a GitHub token is reported by the bundled `github-pat` rule, and the same token "
           "with `false` spliced in is not (the bundled allowlist drops a bundled rule's finding by its value)",
           "github-pat" in rules_by_file.get("token.txt", set())
           and "github-pat" not in rules_by_file.get("token-false.txt", set()), repr(rules_by_file))

failed = [n for n, ok in results if not ok]
print(f"\n[g24-run-gitleaks] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
