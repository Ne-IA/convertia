#!/usr/bin/env python3
"""g24-run-gitleaks.py - G24 self-test for the run-gitleaks G2 driver (P0.3.1).

Three parts, all in real throwaway git repos:
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
      first-push full scan (a key below a clean tip), --staged, --history and --dir. Part (c) SKIPS
      with a notice if the pinned gitleaks binary is absent (a dev box that did not run
      install-gate-tools); the L4 gate-tooling job installs it before the canary.

stdlib-only. Exit 0 = all held; 1 = a self-test failed.
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

# --- (c) every mode end-to-end against the planted secret (the pinned binary) ---------------------
GL = m.gitleaks_bin()
if GL is None:
    print("[g24-run-gitleaks] SKIP the end-to-end legs - pinned gitleaks binary not found (run "
          "scripts/install-gate-tools); the L4 gate-tooling canary installs it and runs them for real.")
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

failed = [n for n, ok in results if not ok]
print(f"\n[g24-run-gitleaks] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
