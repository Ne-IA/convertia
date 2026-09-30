#!/usr/bin/env python3
"""test-docs-only-fastpath-pattern.py - G10 fastpath smoke test for scripts/fastpath-docs-only.

The `test-*-fastpath-pattern` G10 self-test (build-gates G10): a POSITIVE + NEGATIVE proof that the
docs-only skip detector classifies a range correctly and DEFAULTS TO MUST-RUN on ambiguity. Drives
the pure is_skip_eligible / is_docs_only classifiers with fixture path lists (no git needed); the
live range path defaults-to-must-run when origin/main is unknown (the conservative direction), and
in a throwaway repo a docs-only range skips only while the index and working tree change no code (a
staged change counts even with its working copy back at HEAD's bytes, and a code file renamed to a
`.md` path counts as code).
It also pins the detector's lefthook wiring (build-gates §4): exactly the six heavy toolchain
pre-push legs run behind `fastpath-docs-only ||`, each wrap runs its own gate script in the one
accepted shape, and no pre-commit, commit-msg or other pre-push command consults the detector.
The planted legs rewrite the real lefthook.yml text in memory, so they track the live file.
stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "fastpath-docs-only"
_loader = importlib.machinery.SourceFileLoader("fdo", str(SCRIPT))
_spec = importlib.util.spec_from_loader("fdo", _loader)
m = importlib.util.module_from_spec(_spec)
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


# --- is_skip_eligible: single-path POSITIVES (docs-only) --------------------------------------
# Any top-level *.md is documentation (README.md, CHANGELOG.md, a lowercase readme.md), as is
# anything under docs/ and the LICENSE/NOTICE files.
for p in ("docs/spec/00-architecture.md", "docs/SINGLE-SOURCE-OF-TRUTH.md", "README.md",
          "CHANGELOG.md", "readme.md", "LICENSE", "NOTICE", "docs/security/build-gates.md"):
    record(f"skip-eligible POSITIVE: {p}", m.is_skip_eligible(p))

# --- is_skip_eligible: single-path NEGATIVES (must-run) ---------------------------------------
# CRITICAL (build-gates §4 `.md`-only invariant): a NON-`.md` file even UNDER docs/ is NOT skip-
# eligible - a docs/evil.rs / docs/x.json must still force the heavy gates (a wrong skip = a hole).
for p in ("src/main.rs", "src-tauri/Cargo.toml", "scripts/check-branch-protection",
          ".github/workflows/ci.yml", "Cargo.toml", "Cargo.lock", "package.json",
          "lefthook.yml", "src/ui.ts", "docs-extra/x.md",  # "docs-extra/" must NOT match "docs/"
          "docs/evil.rs", "docs/build.py", "docs/x.json",  # §276: non-.md UNDER docs/ -> must-run
          "docs/Cargo.toml", "docs/sub/code.ts", "docs/.gitleaks.toml",
          "docs/../src/main.rs", "a/../docs/x.md",          # any `..` segment -> deny (defense-in-depth)
          "license.txt", "notes.txt",                       # top-level non-.md (NOT skip-eligible)
          "docs", "docs/", ""):                              # the bare dir / trailing-slash / empty
    record(f"must-run NEGATIVE: {p!r}", not m.is_skip_eligible(p))

# --- is_docs_only: range-level -----------------------------------------------------------------
record("docs-only range (all docs) -> True",
       m.is_docs_only(["docs/a.md", "README.md", "LICENSE"]))
record("MIXED range (docs + code) -> False (one code file forces must-run)",
       not m.is_docs_only(["docs/a.md", "src/main.rs"]))
record("MIXED range (docs/*.md + docs/*.rs UNDER docs/) -> False (§276: non-.md under docs/ forces run)",
       not m.is_docs_only(["docs/a.md", "docs/evil.rs"]))
record("code-only range -> False", not m.is_docs_only(["scripts/check-x", "Cargo.toml"]))
record("EMPTY range -> False (ambiguous, never silently skip)", not m.is_docs_only([]))
record("range of blank strings -> False", not m.is_docs_only(["", "  "]))
record("single docs file -> True", m.is_docs_only(["docs/plan/P0-build-and-security.md"]))

# --- live main(): the conservative must-run paths (exit 1) ------------------------------------
record("main: unresolvable base -> MUST-RUN (exit 1)",
       m.main(["--base", "refs/heads/__no_such_base__", "--head", "HEAD"]) == 1)
record("main: EMPTY range (HEAD..HEAD, 0 changed files) -> MUST-RUN (exit 1)",
       m.main(["--base", "HEAD", "--head", "HEAD"]) == 1)

# --- live main() in a throwaway repo: the range verdict joined by the index / working tree --------
# The six wrapped legs read the working tree, so code staged on top of a docs-only unpushed range (the
# build-loop Step 4a pre-review sweep runs `lefthook run pre-push --force` over a staged box) must run
# them. The throwaway-repo git calls get an environment WITHOUT the GIT_* location variables a hook
# plane could carry (with GIT_DIR absolute, `git -C <tmp>` would reach the OUTER repository);
# GIT_EXEC_PATH stays.
for _k in [k for k in os.environ if k.startswith("GIT_") and k != "GIT_EXEC_PATH"]:
    os.environ.pop(_k)


def _git(repo: Path, *a: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *a], capture_output=True, text=True, encoding="utf-8",
                          errors="replace", check=True).stdout


def _commit(repo: Path, msg: str) -> None:
    _git(repo, "add", "-A")
    _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", msg)


def _verdict(repo: Path, base: str) -> int:
    cwd = os.getcwd()
    os.chdir(repo)
    try:
        return m.main(["--base", base])
    finally:
        os.chdir(cwd)


with tempfile.TemporaryDirectory() as _td:
    _repo = Path(_td)
    _git(_repo, "init", "-q", "-b", "main")
    _git(_repo, "config", "user.email", "t@t.t")
    _git(_repo, "config", "user.name", "t")
    (_repo / "README.md").write_bytes(b"r\n")            # bytes: no host newline translation reaches git
    (_repo / "src").mkdir()
    (_repo / "src" / "a.rs").write_bytes(b"fn a() {}\n")
    _commit(_repo, "feat: base")
    _base = _git(_repo, "rev-parse", "HEAD").strip()
    (_repo / "docs").mkdir()
    (_repo / "docs" / "x.md").write_bytes(b"x\n")
    _commit(_repo, "docs: x")
    record("main (live): a docs-only range over a clean tree -> skip-eligible (exit 0)", _verdict(_repo, _base) == 0)
    (_repo / "docs" / "y.md").write_bytes(b"y\n")
    _git(_repo, "add", "-A")
    record("main (live): a docs-only range + a STAGED docs change -> skip-eligible (exit 0)", _verdict(_repo, _base) == 0)
    _commit(_repo, "docs: y")                       # the code legs below then see exactly one local change
    (_repo / "src" / "a.rs").write_bytes(b"fn a() { let _ = 1; }\n")
    record("main (live): a docs-only range + an UNSTAGED code change -> MUST-RUN (exit 1; the legs read the tree)",
           _verdict(_repo, _base) == 1)
    _git(_repo, "add", "-A")
    record("main (live): a docs-only range + a STAGED code change (the Step 4a sweep) -> MUST-RUN (exit 1)",
           _verdict(_repo, _base) == 1)
    # The index half: a staged change whose working copy is back at HEAD's bytes (a codegen re-run over a
    # staged hand edit) is invisible to the working-tree read, replayed first; generated-drift and the
    # lockfile-integrity drift guard diff the working tree against the index, so it must still run them.
    (_repo / "src" / "a.rs").write_bytes(b"fn a() {}\n")          # HEAD's bytes; the index keeps the edit
    record("main (live): a docs-only range + a STAGED code change whose working copy is back at HEAD's bytes"
           " -> MUST-RUN (exit 1; the index read)",
           _git(_repo, "diff", "--name-only", "HEAD").split() == []
           and _git(_repo, "diff", "--cached", "--name-only", "HEAD").split() == ["src/a.rs"]
           and _verdict(_repo, _base) == 1)
    (_repo / "src" / "a.rs").write_bytes(b"fn a() { let _ = 1; }\n")   # the staged bytes again, for the commit
    _commit(_repo, "feat: a")
    record("main (live): a range touching code over a clean tree -> MUST-RUN (exit 1)", _verdict(_repo, _base) == 1)
    # A rename is a deletion at its source path. The incident: git's rename detection (`diff.renames`, set
    # here so a host config cannot mask it) lists only the destination, so `git mv` of code to a `.md`
    # path read as docs-only. Each leg replays that read first, then needs the MUST-RUN verdict.
    _git(_repo, "config", "diff.renames", "true")
    (_repo / "src" / "b.rs").write_bytes(b"fn b() { let _ = 2; }\n")
    _commit(_repo, "feat: b")
    _base2 = _git(_repo, "rev-parse", "HEAD").strip()
    (_repo / "docs" / "z.md").write_bytes(b"z\n")
    _commit(_repo, "docs: z")
    _git(_repo, "mv", "src/b.rs", "docs/b.md")
    record("main (live): a docs-only range + a STAGED rename of code to a .md path -> MUST-RUN (exit 1)",
           _git(_repo, "diff", "--name-only", "HEAD").split() == ["docs/b.md"] and _verdict(_repo, _base2) == 1)
    _commit(_repo, "docs: b")
    record("main (live): a range renaming code to a .md path, clean tree -> MUST-RUN (exit 1)",
           _git(_repo, "diff", "--name-only", f"{_base2}..HEAD").split() == ["docs/b.md", "docs/z.md"]
           and _verdict(_repo, _base2) == 1)

# --- the lefthook wiring (build-gates §4) ------------------------------------------------------
# The wrapped set is keyed on the command id AND on the gate script the wrap runs: a gate moved under
# a wrapped id would otherwise turn skippable while every id still looked right.
LEFTHOOK = REPO / "lefthook.yml"
DETECTOR = "fastpath-docs-only"
SIX = {
    "rust-lint-contract": "check-rust-lint-contract",
    "ts-gate": "check-ts-gate",
    "lockfile-integrity": "check-lockfile-integrity",
    "generated-drift": "check-generated-drift",
    "doc-links": "check-doc-links",
    "coverage": "check-coverage",
}
EXPECTED = {"pre-push": frozenset(SIX)}
_HOOK_RE = re.compile(r"^([A-Za-z0-9_-]+):")
_CMD_RE = re.compile(r"^    ([A-Za-z0-9_.-]+):")
# The one accepted wrap: the detector first, then `||` (the gate runs unless the detector exits 0),
# then one gate invocation with plain arguments. `&&` would invert the skip, `;` would drop it, and a
# chained `|| true`, pipe, redirect, quote or substitution could swallow the gate's exit; `python3 -P`
# on both sides is the plane-line spelling (a `-h`/`-V` there would exit 0 without running the gate).
_WRAP_RE = re.compile(r"^run: python3 -P scripts/fastpath-docs-only \|\| "
                      r"python3 -P scripts/([A-Za-z0-9._-]+)(?: [^|&;<>`$\"'\\]*)?$")


def _code(line: str) -> str:
    """The non-comment part of a lefthook.yml line: a full-line `#` comment is empty and a trailing
    ` # ...` comment is cut, so a wrap mentioned only in a comment is never counted."""
    s = line.split(" #", 1)[0]
    return "" if s.lstrip().startswith("#") else s.rstrip()


def _sites(text: str):
    """(hook, command id, line number, code) for every non-comment line that names the detector,
    under the nearest top-level hook key and 4-space command id. A mention outside any command id
    keeps its line number as the id, so it still counts (default-deny) instead of vanishing."""
    hook, cmd = "<top level>", None
    for n, line in enumerate(text.splitlines(), 1):
        code = _code(line)
        if not code.strip():
            continue
        h = _HOOK_RE.match(code)
        if h:
            hook, cmd = h.group(1), None
        else:
            c = _CMD_RE.match(code)
            if c:
                cmd = c.group(1)
        if DETECTOR in code:
            yield hook, cmd or f"<line {n}>", n, code.strip()


def wrapped(text: str) -> dict[str, set[str]]:
    """{hook: {command id}} of every command that consults the detector on a non-comment line."""
    out: dict[str, set[str]] = {}
    for hook, cmd, _n, _c in _sites(text):
        out.setdefault(hook, set()).add(cmd)
    return out


def wiring_findings(text: str) -> list[str]:
    """Every deviation from the build-gates §4 wiring: a wrapped set other than the six under pre-push
    (or any wrap under another hook), a wrap outside the one accepted shape, or a wrapped id that runs
    a gate script other than its own."""
    found = wrapped(text)
    out: list[str] = []
    for hook in sorted(set(found) | set(EXPECTED)):
        got, want = found.get(hook, set()), EXPECTED.get(hook, frozenset())
        out += [f"{hook}: `{c}` consults the docs-only detector but is not a heavy toolchain pre-push leg"
                for c in sorted(got - want)]
        out += [f"{hook}: `{c}` is a heavy toolchain pre-push leg but is not wrapped" for c in sorted(want - got)]
    for hook, cmd, n, code in _sites(text):
        w = _WRAP_RE.match(code)
        if not w:
            out.append(f"line {n}: `{code}` is not the `run: python3 -P scripts/fastpath-docs-only || "
                       "python3 -P scripts/<gate> <args>` shape")
        elif hook == "pre-push" and cmd in SIX and w.group(1) != SIX[cmd]:
            out.append(f"line {n}: `{cmd}` wraps `{w.group(1)}`, not its own gate `{SIX[cmd]}`")
    return out


def _set_run(text: str, hook: str, cmd: str, run: str) -> str | None:
    """`text` with the `run:` line of `hook`'s command `cmd` replaced by `run` (its indentation kept),
    or None when no such line exists - a planted leg on an absent site would pass vacuously."""
    lines = text.split("\n")
    cur_hook, cur_cmd = None, None
    for i, line in enumerate(lines):
        code = _code(line)
        h = _HOOK_RE.match(code)
        if h:
            cur_hook, cur_cmd = h.group(1), None
            continue
        c = _CMD_RE.match(code)
        if c:
            cur_cmd = c.group(1)
            continue
        if cur_hook == hook and cur_cmd == cmd and code.strip().startswith("run:"):
            lines[i] = line[: len(line) - len(line.lstrip())] + run
            return "\n".join(lines)
    return None


def _reds(text: str | None, needle: str) -> bool:
    """A planted leg holds only when its site existed AND a finding carries the expected needle."""
    return text is not None and any(needle in f for f in wiring_findings(text))


REAL = LEFTHOOK.read_text(encoding="utf-8")
WRAP = "run: python3 -P scripts/fastpath-docs-only || python3 -P scripts/"
_real_findings = wiring_findings(REAL)
for f in _real_findings:
    print(f"  finding: {f}")
record("wiring (a): the real lefthook.yml wraps exactly the six heavy toolchain pre-push legs, each running its own"
       " gate in the accepted shape; pre-commit and commit-msg wrap nothing",
       wrapped(REAL) == {"pre-push": set(SIX)} and _real_findings == [])
record("wiring (b): a planted wrap on the pre-push `l-neg1-ack` (the G71 cage) -> RED",
       _reds(_set_run(REAL, "pre-push", "l-neg1-ack", WRAP + "check-l-neg1-ack --enforce"),
             "pre-push: `l-neg1-ack` consults the docs-only detector"))
record("wiring (c): a planted wrap on a pre-commit (L1) command, even one of the six ids -> RED",
       _reds(_set_run(REAL, "pre-commit", "rust-lint-contract", WRAP + "check-rust-lint-contract"),
             "pre-commit: `rust-lint-contract` consults the docs-only detector"))
_commented = _set_run(REAL, "pre-push", "l-neg1-ack",
                      "run: python3 -P scripts/check-l-neg1-ack --enforce  # not fastpath-docs-only ||")
record("wiring (d): a wrap that appears only in a comment (full-line or trailing) is not counted",
       _commented is not None and wrapped(_commented) == wrapped(REAL)
       and wrapped(REAL + "\n    #   run: " + WRAP[5:] + "check-l-neg1-ack --enforce\n") == wrapped(REAL))
_trailing = _set_run(REAL, "pre-push", "plan-lint", WRAP + "plan-lint  # G7")
record("wiring (d'): a real wrap followed by a trailing comment still counts (the comment cut is not a blind spot)",
       _trailing is not None and "plan-lint" in wrapped(_trailing).get("pre-push", set())
       and _reds(_trailing, "pre-push: `plan-lint` consults the docs-only detector"))
record("wiring (e): one of the six left unwrapped -> RED (the set is exact both ways)",
       _reds(_set_run(REAL, "pre-push", "coverage", "run: python3 -P scripts/check-coverage --full"),
             "pre-push: `coverage` is a heavy toolchain pre-push leg but is not wrapped"))
_off_shape = ("run: python3 -P scripts/fastpath-docs-only && python3 -P scripts/check-ts-gate --full",
              "run: python3 -P scripts/fastpath-docs-only; python3 -P scripts/check-ts-gate --full",
              WRAP + "check-ts-gate --full || true",
              "run: python3 -P scripts/fastpath-docs-only || python3 -h scripts/check-ts-gate --full")
record("wiring (f): an off-shape wrap (`&&` inverts the skip, `;` drops it, a `|| true` tail swallows the gate,"
       " `python3 -h` never runs it) -> RED each",
       all(_reds(_set_run(REAL, "pre-push", "ts-gate", r), "is not the `run: python3 -P scripts/fastpath-docs-only")
           for r in _off_shape))
record("wiring (g): a wrapped id running another gate (the G71 cage moved under `coverage`) -> RED",
       _reds(_set_run(REAL, "pre-push", "coverage", WRAP + "check-l-neg1-ack --enforce"),
             "`coverage` wraps `check-l-neg1-ack`, not its own gate `check-coverage`"))
_block = _set_run(REAL, "pre-push", "plan-lint", "run: >\n        python3 -P scripts/fastpath-docs-only || "
                                                  "python3 -P scripts/plan-lint")
record("wiring (h): a wrap inside a folded block scalar under another pre-push command is counted -> RED",
       _reds(_block, "pre-push: `plan-lint` consults the docs-only detector"))
record("wiring (i): a detector mention outside any command id counts under its hook (default-deny)",
       wrapped("pre-push:\n  jobs: python3 -P scripts/fastpath-docs-only\n") == {"pre-push": {"<line 2>"}}
       and _reds("pre-push:\n  jobs: python3 -P scripts/fastpath-docs-only\n", "pre-push: `<line 2>` consults"))

failed = [n for n, ok in results if not ok]
print(f"\n[test-docs-only-fastpath-pattern] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
