#!/usr/bin/env python3
"""g24-commit-msg.py - G24 self-test for check-commit-msg (P0.3.3, G11).

Proves the conventional-commit subject gate ACCEPTS every valid type/scope/rollback/auto subject and
REJECTS non-conventional ones, and the subject/body caps: the `cap:` legs pin the 100-char subject, the
120-line body and the 5-line trailer block at their boundaries (literal numbers, so a moved cap constant
reds), the counting rule (blank lines and a bounded final trailer block at both planes; `#` comment lines
and the scissors tail only in the L3 hook's reading, while the L4 `%B` reading counts them), git's line
model (a line ends only at LF, whitespace is space/TAB/CR, an undecodable byte is one character, the
subject is one line), the auto-subject exemption, the L3/L4 parity through a real `git commit -v -e`, the
L3 entry's verdict, the L4 range mirror over real `-F` commits (a subject holding a line break git does not
make is rejected at both planes) and the L4 reader under a hostile log config. stdlib-only. Exit 0 = all
held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
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

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-commit-msg"
_loader = importlib.machinery.SourceFileLoader("ccm", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("ccm", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def ok(subject: str) -> bool:
    return m.validate_subject(subject) is None


# --- accepted -------------------------------------------------------------------------------
for good in ("feat(gates): add the G11 gate", "fix: correct the off-by-one", "chore(todo): P0.3.3 abgehakt",
             "docs(spec): sync §0.10", "refactor: extract helper", "test: add a case", "perf: cache it",
             "ci: pin the action", "build: bump node", "chore(scope): roll back — bad deploy",
             "fix(my-scope.v2): hyphen+dot scope", "Merge branch 'main'", 'Revert "feat: x"', "fixup! feat: x",
             "squash! feat: x", "amend! fix: y"):
    record(f"accept {good!r}", ok(good))

# --- rejected -------------------------------------------------------------------------------
for bad in ("random subject line", "feat add the thing", "feature(x): wrong type", "feat(): empty scope",
            "Feat(gates): capitalised type", "wip: not a type", ": no type", "feat(gates):no-space",
            "feat(gates)!: no breaking-! in the spec regex", "feat(Gates): uppercase scope char",
            "feat:    "):
    record(f"reject {bad!r}", not ok(bad))

record("empty message -> error", m.validate_subject(None) is not None)

# --- subject extraction skips comment / blank lines -----------------------------------------
record("subject_of skips # comments + blanks",
       m.subject_of("\n# a comment\n\nfeat(gates): real subject\n", stored=False) == "feat(gates): real subject")

# --- L4 range-mirror: validate_messages (pure) + the file/range dispatch ---------------------
record("range: all-valid subjects -> no violations",
       m.validate_messages(["feat(x): a\n\nbody", "fix: b", "Merge branch 'main'"]) == [])
record("range: flags exactly the bad subjects at the right indices",
       [i for i, _s, _r in m.validate_messages(
           ["feat(x): ok", "bad subject line", "chore: ok", "feature(y): wrong"])] == [1, 3])
# [Test-Change: P0.3.3 — old-obsolete+new-correct, build-gates G11] the old leg read a `#` line of a
# stored message as a comment (subject None). %B is the message after git's cleanup, so a `#` line in it
# is text and its first line is git's subject: a range leg at the end of this file commits
# `# only a comment` with -F and reads it back from %s. Both messages stay flagged; the empty one reports
# no subject.
record("range: a `#`-only and an empty message in range are flagged (the stored `#` line is the subject)",
       [(i, s) for i, s, _r in m.validate_messages(["\n# only a comment\n", "\n\n"])]
       == [(0, "# only a comment"), (1, None)])
record("range: auto-subjects (revert/fixup) accepted in range",
       m.validate_messages(['Revert "feat: x"', "fixup! fix: y"]) == [])
record("dispatch: no file and no --base -> bad invocation (exit 2)", m.main([]) == 2)
record("dispatch: both a file and --base -> bad invocation (exit 2)",
       m.main(["msg.txt", "--base", "HEAD~1"]) == 2)


# --- the length caps: subject <= 100 characters, body <= 120 counted lines, a final trailer block of at
#     most 5 lines not counted. The boundaries are LITERAL here (never read from the module), so a moved
#     SUBJECT_MAX / BODY_MAX / TRAILER_MAX turns a boundary leg red. Unless a leg names one plane, it holds
#     at BOTH readings: the L3 hook file (stored=False) and the L4 `%B` (stored=True). ---
def verdicts(message: str) -> tuple[str | None, str | None]:
    return m.validate_message(message, stored=False), m.validate_message(message, stored=True)


def passes(message: str) -> bool:
    return verdicts(message) == (None, None)


def rejected_by(message: str, needle: str) -> bool:
    return all(reason is not None and needle in reason for reason in verdicts(message))


def counts(message: str) -> tuple[int, int]:
    return m.body_line_count(message, stored=False), m.body_line_count(message, stored=True)


def with_body(lines: list[str], subject: str = "feat(gates): cap leg") -> str:
    return subject + "\n\n" + "\n".join(lines) + "\n"


SCISSORS_LINE = "# ------------------------ >8 ------------------------"   # git's own cut line, spelled out
BODY_120 = [f"body line {i}" for i in range(120)]
TRAILERS = ["Dual-Review: opus=GO sonnet=GO", "L-neg1-ack: owner",
            "Co-Authored-By: Claude <noreply@anthropic.com>"]
TRAILERS_5 = TRAILERS + ["Signed-off-by: A Contributor <a@example.org>", "Co-Authored-By: B <b@example.org>"]
TRAILERS_6 = TRAILERS_5 + ["Signed-off-by: B <b@example.org>"]
s100 = "feat(gates): " + "x" * 87
s101 = s100 + "x"
s_dash = "chore(scope): roll back — " + "y" * 74
record("cap: a 100-char subject -> accepted", len(s100) == 100 and passes(s100))
record("cap: a 101-char subject -> rejected", len(s101) == 101 and rejected_by(s101, "the cap is 100"))
record("cap: a 100-char subject + 3 trailing spaces -> accepted (trailing whitespace stripped)",
       passes(s100 + "   \n\nbody\n"))
record("cap: a 100-code-point subject holding an em-dash (102 UTF-8 bytes) -> accepted",
       len(s_dash) == 100 and len(s_dash.encode("utf-8")) == 102 and passes(s_dash))
record("cap: an auto-subject `Revert \"` + 120 chars -> accepted (exempt)", passes('Revert "' + "z" * 120))
record("cap: a 120-line body -> accepted",
       counts(with_body(BODY_120)) == (120, 120) and passes(with_body(BODY_120)))
record("cap: a 121-line body -> rejected", rejected_by(with_body(BODY_120 + ["one more"]), "the cap is 120"))
spaced = [x for i, line in enumerate(BODY_120) for x in ([line, ""] if i % 3 == 2 else [line])]
record("cap: 120 lines + 40 interleaved blank lines -> accepted (blanks not counted)",
       spaced.count("") == 40 and passes(with_body(spaced)))
record("cap: 120 lines + a final Dual-Review/L-neg1-ack/Co-Authored-By block -> accepted",
       passes(with_body(BODY_120 + [""] + TRAILERS)))
record("cap: 120 lines + a final paragraph with one non-trailer line -> rejected",
       rejected_by(with_body(BODY_120 + ["", TRAILERS[0], "this line has no trailer shape"]), "the cap is 120"))
record("cap: 120 lines + trailer lines with no blank line before them -> rejected (the block is its own paragraph)",
       counts(with_body(BODY_120 + TRAILERS)) == (123, 123)
       and rejected_by(with_body(BODY_120 + TRAILERS), "the cap is 120"))
record("cap: 120 lines + a final 5-line trailer block -> accepted (the block bound is 5)",
       len(TRAILERS_5) == 5 and counts(with_body(BODY_120 + [""] + TRAILERS_5)) == (120, 120)
       and passes(with_body(BODY_120 + [""] + TRAILERS_5)))
record("cap: 120 lines + a final 6-line trailer-shaped paragraph -> rejected (a longer block counts in full)",
       counts(with_body(BODY_120 + [""] + TRAILERS_6)) == (126, 126)
       and rejected_by(with_body(BODY_120 + [""] + TRAILERS_6), "the cap is 120"))
fields = ["Box/§/Gates: none (Co-Pilot act) · G11", "What: a field", "Tests: a field", "Class: a field"]
padding = [f"Field-{i}: padding text" for i in range(200)]
record("cap: a 4-line field block + a final 200-line `Token: value` paragraph -> rejected (204 counted)",
       counts(with_body(fields + [""] + padding)) == (204, 204)
       and rejected_by(with_body(fields + [""] + padding), "the cap is 120"))
commented = [x for i, line in enumerate(BODY_120) for x in ([line, f"# comment {i}"] if i % 4 == 0 else [line])]
record("cap: 120 lines + 30 `#` lines -> the L3 hook reading drops them (accepted), %B counts them (rejected)",
       sum(1 for x in commented if x.startswith("#")) == 30 and counts(with_body(commented)) == (120, 150)
       and verdicts(with_body(commented))[0] is None and "the cap is 120" in (verdicts(with_body(commented))[1] or ""))
diff_lines = ["diff --git a/f b/f"] + [f"+added line {i}" for i in range(199)]
scissors_tail = [SCISSORS_LINE, "# Do not modify or remove the line above.",
                 "# Everything below it will be ignored."] + diff_lines
cut = with_body(BODY_120 + [""] + scissors_tail)
record("cap: 120 lines + the scissors line + 200 diff lines -> the L3 hook reading cuts the tail (accepted), "
       "%B counts it (rejected)",
       len(diff_lines) == 200 and counts(cut) == (120, 323)
       and verdicts(cut)[0] is None and "the cap is 120" in (verdicts(cut)[1] or ""))
lead_hash = "# a leading hash line\nfeat(gates): the next line\n"
record("cap: a `#` first line is the %B subject (rejected) and skipped by the L3 hook reading (accepted)",
       verdicts(lead_hash)[0] is None and "not a conventional commit" in (verdicts(lead_hash)[1] or ""))
record("cap: an auto-subject with a 200-line body -> accepted (exempt)",
       passes(with_body([f"reverted line {i}" for i in range(200)], subject='Revert "feat: x"')))


# --- cap: git's line model at both planes. git splits a message only at LF and its whitespace is space, TAB
#     and CR, so a character Python's bare splitlines()/strip() treat as a line break or a space stays text
#     inside git's line; git's subject (%s) is the whole first paragraph. ---
SEPARATORS = {"U+2028": " ", "U+2029": " ", "U+0085": "\x85", "FF": "\x0c", "VT": "\x0b",
              "U+001C": "\x1c", "U+001D": "\x1d", "U+001E": "\x1e", "a lone CR": "\r"}


def split_subject(sep: str) -> str:
    return "feat(x): " + "a" * 50 + sep + "b" * 200


record("cap: a line break git does not make (U+2028/U+2029/U+0085/FF/VT/U+001C-U+001E/a lone CR) stays in "
       "the subject line -> a 260-char subject, rejected at both planes",
       all(len(m.subject_of(split_subject(sep) + "\n\nbody\n", stored=s) or "") == 260
           and rejected_by(split_subject(sep) + "\n\nbody\n", "the cap is 100")
           for sep in SEPARATORS.values() for s in (False, True)))
record("cap: a trailing space/TAB/CR is git whitespace (100 -> accepted); a trailing FF/VT/NBSP/U+3000 is "
       "text git keeps (101 -> rejected)",
       passes(s100 + " \t\r\n\nbody\n")
       and all(rejected_by(s100 + ch + "\n\nbody\n", "subject is 101 characters")
               for ch in ("\x0c", "\x0b", "\xa0", "　")))
ff_first = "\x0c\n\nfeat(gates): after a form-feed line\n"
record("cap: a line of only FF or U+3000 is text git keeps - first, it is git's subject (not conventional); "
       "in the body it counts (121 -> rejected)",
       all("not a conventional commit" in (v or "") for v in verdicts(ff_first))
       and all(counts(with_body(BODY_120 + [ch])) == (121, 121)
               and rejected_by(with_body(BODY_120 + [ch]), "the cap is 120") for ch in ("\x0c", "　")))
record("cap: a CRLF message counts as its LF lines (the trailing CR is git whitespace)",
       counts(with_body(BODY_120).replace("\n", "\r\n")) == (120, 120)
       and passes(with_body(BODY_120).replace("\n", "\r\n"))
       and rejected_by(with_body(BODY_120 + ["one more"]).replace("\n", "\r\n"), "the cap is 120")
       and rejected_by(s101 + "\r\n", "the cap is 100"))
two_line = "feat(gates): the first line\nits second line\n\nbody\n"
record("cap: a subject that runs onto a second line -> rejected at both planes (git's %s joins the paragraph); "
       "an auto-subject's -> accepted",
       rejected_by(two_line, "runs onto a second line")
       and passes("Merge branch 'x'\nof y\n"))
undecodable = b"feat(x): " + b"\xe2\x80" * 46          # 46 truncated 3-byte sequences = 92 undecodable bytes
record("cap: an undecodable byte counts as one character - `feat(x): ` + 92 undecodable bytes -> 101 "
       "characters, rejected at both planes",
       len(m.decode_message(undecodable)) == 101
       and rejected_by(m.decode_message(undecodable + b"\n"), "subject is 101 characters"))


# --- L4 range resolution (check_range) in a real temp repo — the all-zeros / absent-base
#     degrade path is the security-critical leg (a fail-closed mirror must NOT exit-2-redden
#     main on an initial-push / force-push); mirrors g24-dual-review.py's commit_shas legs. ---


def _git(repo: Path, *a: str) -> None:
    subprocess.run(["git", "-C", str(repo), *a], capture_output=True, text=True, encoding="utf-8", errors="replace", check=True)


def _git_out(repo: Path, *a: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *a], capture_output=True, text=True, encoding="utf-8", errors="replace", check=True).stdout.strip()


def _in_repo(repo: Path, fn):
    cwd = os.getcwd()
    os.chdir(repo)
    try:
        return fn()
    finally:
        os.chdir(cwd)


with tempfile.TemporaryDirectory() as td:
    repo = Path(td)
    _git(repo, "init", "-q", "-b", "main")
    _git(repo, "config", "user.email", "t@t.t")
    _git(repo, "config", "user.name", "t")
    (repo / "a").write_text("1\n", encoding="utf-8"); _git(repo, "add", "-A"); _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "feat: one")
    c1 = _git_out(repo, "rev-parse", "HEAD")
    (repo / "a").write_text("2\n", encoding="utf-8"); _git(repo, "add", "-A"); _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "feat: two")
    c2 = _git_out(repo, "rev-parse", "HEAD")
    record("range: all-zeros base -> tip-only degrade -> exit 0 (no red-CI on a new-ref push)",
           _in_repo(repo, lambda: m.check_range("0" * 40, "HEAD")) == 0)
    record("range: absent 40-hex base -> tip-only degrade -> exit 0 (no rev-list crash on a force-push)",
           _in_repo(repo, lambda: m.check_range("deadbeef" * 5, "HEAD")) == 0)
    record("range: real base..HEAD all-conventional -> exit 0",
           _in_repo(repo, lambda: m.check_range(c1, "HEAD")) == 0)
    (repo / "a").write_text("3\n", encoding="utf-8"); _git(repo, "add", "-A"); _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-m", "broken subject")
    record("range: a non-conventional subject in range -> caught -> exit 1",
           _in_repo(repo, lambda: m.check_range(c2, "HEAD")) == 1)


# --- cap: L3/L4 parity + the L3 entry + the range mirror, in a temp repo whose core.hooksPath is a temp
#     hooks dir. A `#!/bin/sh` commit-msg hook copies the file git hands it (the L3 input); `git commit -v -e`
#     with a no-op editor writes the comment block + scissors + diff into that file and stores the cleaned
#     `%B` (the L4 input). The hook copy must be the uncleaned file AND count the same 6 lines as `%B`. The
#     commits after it use -F without -e (git's whitespace cleanup), which stores `#` lines as text. ---
PARITY_MSG = ("feat(gates): parity leg\n\n" + "\n".join(f"body line {i}" for i in range(5))
              + "\n# an own comment line\n\nbody line 5\n\nDual-Review: opus=GO sonnet=GO\n")
with tempfile.TemporaryDirectory() as td:
    repo, hooks = Path(td) / "repo", Path(td) / "hooks"
    repo.mkdir()
    hooks.mkdir()
    _git(repo, "init", "-q", "-b", "main")
    _git(repo, "config", "user.email", "t@t.t")
    _git(repo, "config", "user.name", "t")
    _git(repo, "config", "commit.gpgsign", "false")
    _git(repo, "config", "commit.cleanup", "default")      # git's own cleanup choice, whatever the host sets
    _git(repo, "config", "core.commentChar", "#")
    _git(repo, "config", "core.hooksPath", str(hooks))
    hook = hooks / "commit-msg"
    hook.write_bytes(b'#!/bin/sh\ncp "$1" "$(dirname "$1")/hook-copy"\n')
    hook.chmod(0o755)
    (repo / "f").write_bytes(b"".join(b"staged line %d\n" % i for i in range(200)))   # a -v diff over the cap
    _git(repo, "add", "f")
    msg_file = Path(td) / "msg"
    msg_file.write_bytes(PARITY_MSG.encode("utf-8"))
    committed = subprocess.run(["git", "-C", str(repo), "commit", "-q", "-v", "-e", "-F", str(msg_file)],
                               env={**os.environ, "GIT_EDITOR": "true"}, capture_output=True, text=True,
                               encoding="utf-8", errors="replace").returncode == 0
    copy_path = repo / ".git" / "hook-copy"
    copy = m.decode_message(copy_path.read_bytes()) if copy_path.is_file() else ""     # the L3 reader
    stored = _in_repo(repo, lambda: m.stored_message("HEAD")) if committed else ""      # the L4 reader
    uncleaned = "# an own comment line" in copy and SCISSORS_LINE in copy and "diff --git" in copy
    record("cap: L3/L4 parity - the hook gets the uncleaned file and counts the same lines as %B",
           committed and uncleaned
           and m.body_line_count(copy, stored=False) == m.body_line_count(stored, stored=True) == 6)
    record("cap: L3 entry - the real hook file (a 200-line -v diff below the scissors) -> check-commit-msg <file> exit 0",
           committed and m.main([str(copy_path)]) == 0 and m.body_line_count(copy, stored=True) > 200)
    parity_head = _git_out(repo, "rev-parse", "HEAD")
    (repo / "f").write_bytes(b"three\n")
    _git(repo, "add", "f")
    msg_file.write_bytes(with_body(BODY_120 + ["one more"]).encode("utf-8"))
    record("cap: L3 entry - a 121-line body file -> check-commit-msg <file> exit 1",
           m.main([str(msg_file)]) == 1)
    _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-F", str(msg_file))
    record("cap: range - an over-cap body in base..HEAD -> caught -> exit 1",
           _in_repo(repo, lambda: m.check_range(parity_head, "HEAD")) == 1)
    over_head = _git_out(repo, "rev-parse", "HEAD")
    (repo / "f").write_bytes(b"four\n")
    _git(repo, "add", "f")
    msg_file.write_bytes(with_body(BODY_120 + ["# a stored hash line"]).encode("utf-8"))
    _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-F", str(msg_file))
    stored_hash = _git_out(repo, "log", "-1", "--format=%B")
    record("cap: range - a -F commit stores its `#` line, %B counts it (121) -> exit 1",
           "# a stored hash line" in stored_hash.splitlines()
           and m.body_line_count(stored_hash, stored=True) == 121
           and _in_repo(repo, lambda: m.check_range(over_head, "HEAD")) == 1)
    hash_head = _git_out(repo, "rev-parse", "HEAD")
    (repo / "f").write_bytes(b"five\n")
    _git(repo, "add", "f")
    msg_file.write_bytes(b"# only a comment\n")
    _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-F", str(msg_file))
    record("range: a -F commit of `# only a comment` has it as git's subject (%s) -> caught -> exit 1",
           _git_out(repo, "log", "-1", "--format=%s") == "# only a comment"
           and _in_repo(repo, lambda: m.check_range(hash_head, "HEAD")) == 1)
    # the reviewer's incident, through real git: git stores the subject line whole (its first line holds
    # 260 code points), and both planes read that line - the L4 mirror from the stored commit, the L3 entry
    # from the message file (a lone CR included, which a text-mode reader turns into a line break).
    for name, sep in SEPARATORS.items():
        prev = _git_out(repo, "rev-parse", "HEAD")
        (repo / "f").write_bytes(f"separator {name}\n".encode("utf-8"))
        _git(repo, "add", "f")
        msg_file.write_bytes((split_subject(sep) + "\n\nbody line\n").encode("utf-8"))
        _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-F", str(msg_file))
        raw = subprocess.run(["git", "-C", str(repo), "cat-file", "commit", "HEAD"], capture_output=True,
                             check=True).stdout
        first = raw.split(b"\n\n", 1)[1].split(b"\n", 1)[0].decode("utf-8")
        record(f"cap: range + L3 entry - a stored `feat(x): <50>` + {name} + `<200>` subject (git's first "
               "line: 260 code points) -> check_range exit 1 and check-commit-msg <file> exit 1",
               len(first) == 260
               and _in_repo(repo, lambda: m.check_range(prev, "HEAD")) == 1
               and m.main([str(msg_file)]) == 1)
    prev = _git_out(repo, "rev-parse", "HEAD")
    (repo / "f").write_bytes(b"lone CR body\n")
    _git(repo, "add", "f")
    cr_lines = [f"body line {i}\rits tail after a lone CR" for i in range(120)]
    msg_file.write_bytes(with_body(cr_lines).encode("utf-8"))
    _git(repo, "-c", "core.hooksPath=", "commit", "-q", "-F", str(msg_file))
    raw = subprocess.run(["git", "-C", str(repo), "cat-file", "commit", "HEAD"], capture_output=True,
                         check=True).stdout
    record("cap: range + L3 entry - a 120-line body whose lines each hold a lone CR (git stores 122 message "
           "lines) -> check_range exit 0 and check-commit-msg <file> exit 0 (a lone CR ends no line)",
           raw.split(b"\n\n", 1)[1].count(b"\n") == 122
           and _in_repo(repo, lambda: m.check_range(prev, "HEAD")) == 0
           and m.main([str(msg_file)]) == 0)
    msg_file.write_bytes(undecodable + b"\n\nbody line\n")
    record("cap: L3 entry - a file whose subject holds 92 undecodable bytes (101 characters) -> exit 1",
           m.main([str(msg_file)]) == 1)


# --- cap: the L4 reader under a hostile log config. log.showSignature prints a signature check's output
#     above the message (a fake gpg.program stands in for gpg, over a commit object that carries a gpgsig
#     header) and i18n.logOutputEncoding=ISO-8859-1 re-encodes `é`; the reader must still return the stored
#     message byte for byte, a lone CR and a form feed included. ---
with tempfile.TemporaryDirectory() as td:
    repo = Path(td) / "repo"
    repo.mkdir()
    _git(repo, "init", "-q", "-b", "main")
    fake_gpg = Path(td) / "fake-gpg"
    fake_gpg.write_bytes(b'#!/bin/sh\necho "fake signature check output" >&2\nexit 1\n')
    fake_gpg.chmod(0o755)
    _git(repo, "config", "gpg.program", fake_gpg.as_posix())
    _git(repo, "config", "log.showSignature", "true")
    _git(repo, "config", "i18n.logOutputEncoding", "ISO-8859-1")
    tree = subprocess.run(["git", "-C", str(repo), "mktree"], input=b"", capture_output=True,
                          check=True).stdout.strip()
    hostile_msg = "feat(x): café\rtail\x0cform feed\n\nbody\n"
    commit_obj = (b"tree " + tree + b"\nauthor t <t@t.t> 1700000000 +0000\n"
                  b"committer t <t@t.t> 1700000000 +0000\n"
                  b"gpgsig -----BEGIN PGP SIGNATURE-----\n \n AAAA\n -----END PGP SIGNATURE-----\n\n"
                  + hostile_msg.encode("utf-8"))
    sha = subprocess.run(["git", "-C", str(repo), "hash-object", "-t", "commit", "-w", "--stdin"],
                         input=commit_obj, capture_output=True, check=True).stdout.decode().strip()
    plain = subprocess.run(["git", "-C", str(repo), "log", "-1", "--format=%B", sha], capture_output=True).stdout
    record("cap: the L4 reader returns the stored message exactly under log.showSignature + a latin-1 "
           "logOutputEncoding (a plain `git log --format=%B` prints the check output and re-encodes `é`)",
           plain.startswith(b"fake signature check output\n") and b"caf\xe9\r" in plain
           and _in_repo(repo, lambda: m.stored_message(sha)) == hostile_msg + "\n")

failed = [n for n, k in results if not k]
print(f"\n[g24-commit-msg] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
