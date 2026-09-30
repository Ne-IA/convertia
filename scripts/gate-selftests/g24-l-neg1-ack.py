#!/usr/bin/env python3
"""g24-l-neg1-ack.py - G24 self-test for check-l-neg1-ack (P0.2.14, G71).

Proves the L(-1)-ack change-control gate: (1) the glob matcher caging the right paths (a wrong glob
is a cage GAP), (2) that the trailer is the ONLY escape - there is NO check-off / `[!extern]`
exemption, so a check-off / `[!extern]` commit that touches an L(-1) file still REQUIRES the ack
(security-concept §2; the §2-vs-plan conflict resolves to §2), (3) the ACK trailer regex, (4) the
end-to-end verdict in a REAL temp git repo - an L(-1)-touching commit WITHOUT the trailer fails under
--enforce (fail-soft without), WITH the trailer passes; a check-off / `[!extern]` commit OVER an
L(-1) file FAILS (no exemption), while a non-L(-1) commit (incl. a plan-only check-off) passes the
TRAILER audit (the P1.66 base-resolution hardening legs live here too) - (5) the P4.56.1
cage-liveness audit: dead / stale-declared / orphan-declared globs each red the gate (unit + E2E,
incl. the CWD-independence and non-ASCII-path legs), so a clean tip can still exit 1 on a sick cage
- and (6) the `scripts/**` cage with its `[[loop_tool]]` escapes: every scripts/ path but a declared
escape is caged, a gate-shaped / nested / extension-bearing escape is refused and stays caged, a
dead or orphan escape reds the gate, a malformed escape table makes the cage unreadable, and in a
real temp repo an escape edit needs no ack while a new scripts/ file does.
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

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-l-neg1-ack"
_loader = importlib.machinery.SourceFileLoader("clna", str(SCRIPT))
_spec = importlib.util.spec_from_loader("clna", _loader)
m = importlib.util.module_from_spec(_spec)
_loader.exec_module(m)

# the REAL committed cage (the gate's DEFAULT_CAGE) - so the glob matcher is tested against production
REGEXES = m.load_patterns(m.DEFAULT_CAGE)
CAGE = m._read_cage(m.DEFAULT_CAGE)
ESCAPES = m.honoured_escapes(CAGE.loop_tools) if CAGE is not None else frozenset()
results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


# --- glob matcher / is_l_neg1: POSITIVES (caged) ----------------------------------------------
record("cage loaded (non-empty)", isinstance(REGEXES, list) and len(REGEXES) > 0)
record("cage loaded with its [[loop_tool]] escapes (non-empty)", CAGE is not None and len(ESCAPES) > 0)
# [Test-Change: G71 scripts/** shape — old-obsolete+new-correct, security-concept §2] scripts/helper.py
# and scripts/gen.sh moved from the NEGATIVES: the old cage caged only gate-shaped scripts/ names, the
# `scripts/**` cage cages every scripts/ path but a declared [[loop_tool]] escape.
for p in ("lefthook.yml", "scripts/check-l-neg1-ack", "scripts/check-branch-protection",
          "scripts/fastpath-docs-only", "scripts/run-gate-selftests", "scripts/install-gate-tools",
          "scripts/setup-dev", "scripts/gate-selftests/g24-l-neg1-ack.py", "scripts/gate-planes.toml",
          "scripts/l-neg1-files.toml", ".github/workflows/ci.yml", ".github/allowed_signers",
          "deny.toml", ".gitleaks.toml", "supply-chain/config.toml", "supply-chain/imports.lock",
          ".gitattributes", ".lfsconfig", "requirements-ci.txt", "src-tauri/capabilities/default.json",
          "rust-toolchain.toml", "src-tauri/engines.lock", "docs/security/build-gates.md",
          "docs/security/security-concept.md", "docs/process/build-loop.md",
          "scripts/helper.py", "scripts/gen.sh",       # any scripts/ file, not only a gate-shaped name
          "scripts/newtool",                           # a future top-level tool is caged until declared
          "scripts/stage-corpus", "scripts/plan-lint", "scripts/doc-fingerprints.toml",
          "scripts/semgrep-rules/project/x.yaml", "scripts/deep/nested/file",
          "scripts/stage-engines.py",                  # an escape is an EXACT path: a sibling spelling stays caged
          "scripts/Stage-Engines"):                    # ...and so does a case variant (git paths are case-sensitive)
    record(f"L(-1) POSITIVE: {p}", m.is_l_neg1(p, REGEXES, ESCAPES))

# --- is_l_neg1: NEGATIVES (NOT caged) ---------------------------------------------------------
for p in ("README.md", "src/main.rs", "src/ui.ts", "Cargo.toml", "Cargo.lock", "package.json",
          "docs/plan/P0-build-and-security.md",        # the plan is NOT L(-1) (only security/process docs are)
          "docs/SINGLE-SOURCE-OF-TRUTH.md",            # the SSOT is NOT under security/process
          "scripts/stage-engines", "scripts/fetch-engine-assets",
          "scripts/compile-engine-asset",              # the declared [[loop_tool]] escapes (the Loop's build tools)
          "engines.lock",                              # patterns match ROOT-ANCHORED: the cage names the §3.7.2 home `src-tauri/engines.lock` (P4.56.1); a repo-root spelling matches no spec-sanctioned path
          ".githubfoo/x", "docs/securityfoo/x", "scriptsfoo/x", "tests/scripts/x", ""):   # prefix-confusion / anchoring / empty
    record(f"NOT-caged NEGATIVE: {p!r}", not m.is_l_neg1(p, REGEXES, ESCAPES))

# --- cage-liveness audit (P4.56.1 - the dead-glob class) --------------------------------------
# dead_globs is pure (patterns x tracked-paths in, dead patterns out), so the shapes are driven
# directly; the last leg runs the audit over the REAL committed cage + the REAL tracked tree, so
# a dead glob in production reds this self-test too, not only the gate run.
_TRACKED = ["lefthook.yml", "src-tauri/engines.lock", "scripts/check-l-neg1-ack", ".github/workflows/ci.yml"]
record("audit: an all-live pattern set has no dead globs",
       m.dead_globs(["lefthook.yml", "src-tauri/engines.lock", ".github/**"], _TRACKED) == [])
record("audit: a root-anchored mis-spelling IS dead (the bare-engines.lock bug shape)",
       m.dead_globs(["engines.lock"], _TRACKED) == ["engines.lock"])
record("audit: a glob whose target is absent is dead",
       m.dead_globs(["deny.toml"], _TRACKED) == ["deny.toml"])
record("audit: a declared AUDIT_DECLARED_TARGETLESS entry is skipped, not dead",
       m.dead_globs([".lfsconfig"], _TRACKED) == [])
record("audit: a declared glob whose target LANDED is a stale declaration",
       m.stale_declarations([".lfsconfig"], _TRACKED + [".lfsconfig"]) == [".lfsconfig"])
record("audit: a declared glob with no landed target is NOT stale",
       m.stale_declarations([".lfsconfig"], _TRACKED) == [])
record("audit: an undeclared live glob is neither dead nor stale",
       m.dead_globs(["lefthook.yml"], _TRACKED) == [] and m.stale_declarations(["lefthook.yml"], _TRACKED) == [])
record("audit: a declaration whose GLOB left the cage is an orphan",
       m.orphan_declarations(["lefthook.yml"]) == [".lfsconfig"])
record("audit: a declaration whose glob is present is NOT an orphan",
       m.orphan_declarations(["lefthook.yml", ".lfsconfig"]) == [])
_ls = subprocess.run(["git", "ls-files"], capture_output=True, text=True, encoding="utf-8",
                     cwd=SCRIPT.parents[1])
_real_pats = CAGE.patterns if CAGE is not None else []
record("audit: the REAL committed cage is fully live over the REAL tracked tree",
       _ls.returncode == 0 and m.dead_globs(_real_pats, _ls.stdout.splitlines()) == [])
record("audit: the REAL cage carries no stale and no orphan declarations",
       _ls.returncode == 0
       and m.stale_declarations(_real_pats, _ls.stdout.splitlines()) == []
       and m.orphan_declarations(_real_pats) == [])

# --- the [[loop_tool]] escape audit (the scripts/** cage) -------------------------------------
# The real cage first: every declared escape is honoured, tracked and caged by a pattern, and the
# escapes are exactly the tracked scripts/ files the cage leaves uncaged (the escape list and the
# real uncaged set cannot drift apart unnoticed).
_real_tools = CAGE.loop_tools if CAGE is not None else {}
record("escape: the REAL cage refuses none of its escapes (all honoured)",
       CAGE is not None and m.escape_refusals(_real_tools) == [] and ESCAPES == frozenset(_real_tools))
record("escape: the REAL cage has no dead and no orphan escape over the REAL tracked tree",
       _ls.returncode == 0 and m.dead_escapes(_real_tools, _ls.stdout.splitlines()) == []
       and m.orphan_escapes(_real_tools, _real_pats) == [])
record("escape: the REAL uncaged scripts/ files are exactly the declared escapes",
       _ls.returncode == 0 and REGEXES is not None
       and sorted(p for p in _ls.stdout.splitlines()
                  if p.startswith("scripts/") and not m.is_l_neg1(p, REGEXES, ESCAPES)) == sorted(_real_tools))
record("escape: every escape carries its reason",
       all(isinstance(r, str) and r.strip() for r in _real_tools.values()))
# The refusal shapes: nested, extension-bearing, non-slug, outside scripts/, a trailing newline (the
# `$`-matches-before-`\n` trap a fullmatch closes), and every gate-shaped name.
for p in ("scripts/sub/tool", "scripts/tool.py", "scripts/Tool", "scripts/-tool", "tools/x",
          "scripts/tool\n", "scripts/", "scripts/check-foo", "scripts/run-foo", "scripts/fastpath-foo",
          "scripts/plan-lint", "scripts/install-gate-tools", "scripts/setup-dev", "scripts/stage-corpus"):
    record(f"escape: {p!r} is REFUSED", [q for q, _ in m.escape_refusals({p: "r"})] == [p])
record("escape: a top-level, extension-less, non-gate name is NOT refused",
       m.escape_refusals({"scripts/stage-engines": "r", "scripts/build-tool2": "r"}) == [])
record("escape: a refused escape is not honoured, so its path stays caged",
       m.honoured_escapes({"scripts/check-x": "r", "scripts/tool": "r"}) == frozenset({"scripts/tool"})
       and m.is_l_neg1("scripts/check-x", [m.glob_to_regex("scripts/**")],
                       m.honoured_escapes({"scripts/check-x": "r"})))
record("escape: an honoured escape is uncaged, its caged neighbour is not",
       not m.is_l_neg1("scripts/tool", [m.glob_to_regex("scripts/**")], frozenset({"scripts/tool"}))
       and m.is_l_neg1("scripts/tool2", [m.glob_to_regex("scripts/**")], frozenset({"scripts/tool"})))
record("escape: an untracked escape is DEAD",
       m.dead_escapes({"scripts/gone": "r", "scripts/tool": "r"}, ["scripts/tool", "lefthook.yml"])
       == ["scripts/gone"])
record("escape: a tracked escape is not dead (a backslash-spelled listing normalizes)",
       m.dead_escapes({"scripts/tool": "r"}, ["scripts\\tool"]) == [])
record("escape: an escape no pattern matches is an ORPHAN",
       m.orphan_escapes({"scripts/tool": "r"}, ["lefthook.yml", "docs/security/**"]) == ["scripts/tool"])
record("escape: an escape a pattern matches is not an orphan",
       m.orphan_escapes({"scripts/tool": "r"}, ["lefthook.yml", "scripts/**"]) == [])


def _cage_from(text: str) -> "m.Cage | None":
    with tempfile.TemporaryDirectory() as ctd:
        p = Path(ctd) / "cage.toml"
        p.write_bytes(text.encode("utf-8"))
        return m._read_cage(p)


_PATS = 'patterns = ["scripts/**"]\n'
_LT = '\n[[loop_tool]]\npath = "{p}"\nreason = "{r}"\n'
_ok = _cage_from(_PATS + _LT.format(p="scripts/a", r="ra") + _LT.format(p="scripts/b", r="rb"))
record("cage: two well-formed [[loop_tool]] tables parse to {path: reason}",
       _ok is not None and _ok.loop_tools == {"scripts/a": "ra", "scripts/b": "rb"}
       and _ok.patterns == ["scripts/**"])
_none = _cage_from(_PATS)
record("cage: no [[loop_tool]] table parses to no escapes", _none is not None and _none.loop_tools == {})
for label, body in (
        ("a missing reason", '\n[[loop_tool]]\npath = "scripts/a"\n'),
        ("a missing path", '\n[[loop_tool]]\nreason = "r"\n'),
        ("an empty path", _LT.format(p="", r="r")),
        ("a whitespace-only reason", _LT.format(p="scripts/a", r="  ")),
        ("a non-string path", '\n[[loop_tool]]\npath = 7\nreason = "r"\n'),
        ("a non-string reason", '\n[[loop_tool]]\npath = "scripts/a"\nreason = 7\n'),
        ("an unknown key (a misspelled reason)", '\n[[loop_tool]]\npath = "scripts/a"\nreasons = "r"\n'),
        ("an extra key beside path and reason", _LT.format(p="scripts/a", r="r") + 'note = "x"\n'),
        ("a duplicate path", _LT.format(p="scripts/a", r="r1") + _LT.format(p="scripts/a", r="r2")),
        ("a single [loop_tool] table, not an array", '\n[loop_tool]\npath = "scripts/a"\nreason = "r"\n')):
    record(f"cage: {label} in [[loop_tool]] -> malformed (None)", _cage_from(_PATS + body) is None)

# --- ACK trailer regex ------------------------------------------------------------------------
record("ack regex: 'L-neg1-ack: owner' line -> match",
       bool(m.ACK_RE.search("subject\n\nbody\nL-neg1-ack: owner\nCo-Authored-By: x")))
record("ack regex: tab spacing tolerated", bool(m.ACK_RE.search("L-neg1-ack:\towner")))
record("ack regex: 'owner ' trailing ws tolerated", bool(m.ACK_RE.search("L-neg1-ack: owner ")))
record("ack regex: wrong value 'L-neg1-ack: co-pilot' -> no match",
       not m.ACK_RE.search("L-neg1-ack: co-pilot"))
record("ack regex: inline (not line-start) -> no match",
       not m.ACK_RE.search("see L-neg1-ack: owner here"))

# --- base resolution hardening (P1.66): an all-zeros / absent github.event.before must route to the
# tip-only fallback, NOT fatal-red under --enforce (the ^{commit}-peel + strip('0'), mirroring the
# sibling check-dual-review). Unit here; proven end-to-end in the temp repo below.
record("base: an all-zeros 40-hex base does NOT resolve (peeled to ^{commit})", m._resolves("0" * 40) is False)
record("base: HEAD still resolves (the peel does not break a real ref)", m._resolves("HEAD") is True)
record("base: resolve_base(all-zeros) -> None (strip('0') short-circuits before any rev-list)",
       m.resolve_base("0" * 40) is None)


# --- end-to-end in a REAL temp git repo -------------------------------------------------------
def _git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, encoding="utf-8", errors="replace",
                          check=True).stdout.strip()


def _commit(repo: Path, rel: str, content: str, message: str) -> str:
    f = repo / rel
    f.parent.mkdir(parents=True, exist_ok=True)
    f.write_text(content, encoding="utf-8")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-q", "-m", message)   # the fresh temp repo has core.hooksPath set to an empty dir
    return _git(repo, "rev-parse", "HEAD")


def run_gate(repo: Path, base: str, head: str, *, enforce: bool, cage: Path | None = None,
             subdir: str | None = None) -> int:
    """Run check-l-neg1-ack inside `repo` (the gate uses git in CWD), against the temp repo's OWN
    mini cage — hermetic against the production cage's CONTENT (P4.56.1: the cage-liveness audit
    measures every glob against `git ls-files`, so the real 35-glob cage would be almost entirely
    dead inside the throwaway repo; before the audit the coupling was latent, now it is load-bearing)."""
    cwd = os.getcwd()
    os.chdir(repo / subdir if subdir else repo)
    try:
        argv = ["--base", base, "--head", head, "--cage", str(cage if cage is not None else repo / "cage.toml")]
        if enforce:
            argv.append("--enforce")
        return m.main(argv)
    finally:
        os.chdir(cwd)


with tempfile.TemporaryDirectory() as td:
    repo = Path(td)
    _git(repo, "init", "-q", "-b", "main")
    _git(repo, "config", "user.email", "t@t.t")
    _git(repo, "config", "user.name", "t")
    (repo / ".nohooks").mkdir()
    _git(repo, "config", "core.hooksPath", str(repo / ".nohooks"))   # no hooks fire in the throwaway repo
    # the mini cage + every cage-target file land in the base commit, so the cage-liveness audit is
    # green inside the fixture from the start; the legs below then EDIT these files (diff semantics
    # identical - is_l_neg1 is add-vs-modify agnostic). Every fixture cage ALSO carries the
    # `.lfsconfig` glob: AUDIT_DECLARED_TARGETLESS is a module constant that applies to whichever
    # cage the gate runs, so a fixture cage WITHOUT the declared glob would trip the orphan leg.
    # The mini cage mirrors the production shape: `scripts/**` plus one [[loop_tool]] escape
    # (scripts/buildtool), with a gate-shaped scripts/check-x tracked beside it.
    (repo / "cage.toml").write_text(
        'patterns = ["lefthook.yml", "docs/security/**", "rust-toolchain.toml", ".lfsconfig", "scripts/**"]\n'
        '\n[[loop_tool]]\npath = "scripts/buildtool"\nreason = "the fixture build tool"\n', encoding="utf-8")
    (repo / "docs" / "security").mkdir(parents=True)
    (repo / "docs" / "security" / "build-gates.md").write_text("g\n", encoding="utf-8")
    (repo / "lefthook.yml").write_text("x: 0\n", encoding="utf-8")
    (repo / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "pinned"\n', encoding="utf-8")
    (repo / "scripts").mkdir()
    (repo / "scripts" / "buildtool").write_text("#!/bin/sh\necho 1\n", encoding="utf-8")
    (repo / "scripts" / "check-x").write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    base = _commit(repo, "README.md", "# base\n", "chore: base")

    # L(-1) edit (lefthook.yml) WITHOUT the trailer
    bad = _commit(repo, "lefthook.yml", "x: 1\n", "ci: tweak the hook plane")
    record("E2E: L(-1) edit, NO trailer, --enforce -> exit 1", run_gate(repo, base, bad, enforce=True) == 1)
    record("E2E: L(-1) edit, NO trailer, no --enforce -> exit 0 (fail-soft P0)",
           run_gate(repo, base, bad, enforce=False) == 0)

    # L(-1) edit WITH the trailer
    good = _commit(repo, "lefthook.yml", "x: 2\n", "ci: tweak the hook plane\n\nL-neg1-ack: owner")
    record("E2E: L(-1) edit, WITH trailer, --enforce -> exit 0", run_gate(repo, bad, good, enforce=True) == 0)

    # a check-off commit touching an L(-1) .md doc -> NOT exempt (no check-off escape: the gate
    # catalogue is the most enforcement-critical file; §2 sanctions only the trailer)
    chk = _commit(repo, "docs/security/build-gates.md", "doc\n", "chore(todo): P0.2.14 abgehakt")
    record("E2E: check-off over an L(-1) .md -> exit 1 (NO exemption; needs the ack)",
           run_gate(repo, good, chk, enforce=True) == 1)

    # a [!extern] commit touching lefthook.yml -> NOT exempt (no [!extern] escape for an L(-1) edit)
    ext = _commit(repo, "lefthook.yml", "x: 3\n", "chore: external action [!extern]")
    record("E2E: [!extern] over an L(-1) file -> exit 1 (NO exemption; needs the ack)",
           run_gate(repo, chk, ext, enforce=True) == 1)

    # a plan-only check-off (docs/plan is NOT L(-1)) -> exit 0 (a legit check-off passes via empty touched)
    plan = _commit(repo, "docs/plan/P0.md", "- [x] box\n", "chore(todo): box abgehakt")
    record("E2E: plan-only check-off (non-L(-1)) -> exit 0 (legit check-off passes)",
           run_gate(repo, ext, plan, enforce=True) == 0)

    # a non-L(-1) commit (README.md) WITHOUT trailer -> exit 0 (nothing caged touched)
    non = _commit(repo, "README.md", "# more\n", "docs: readme tweak")
    record("E2E: non-L(-1) edit, NO trailer, --enforce -> exit 0", run_gate(repo, plan, non, enforce=True) == 0)

    # the scripts/** cage: an edit to a declared escape needs no ack, a NEW scripts/ file does,
    # and so does an edit to a gate-shaped script beside the escape
    tool_edit = _commit(repo, "scripts/buildtool", "#!/bin/sh\necho 2\n", "build: tune the build tool")
    record("E2E: an edit to a declared [[loop_tool]] escape, NO trailer, --enforce -> exit 0 (uncaged)",
           run_gate(repo, non, tool_edit, enforce=True) == 0)
    new_tool = _commit(repo, "scripts/newtool", "#!/bin/sh\n", "build: add a new tool")
    record("E2E: a NEW scripts/ file, NO trailer, --enforce -> exit 1 (scripts/** cages it until declared)",
           run_gate(repo, tool_edit, new_tool, enforce=True) == 1)
    gate_edit = _commit(repo, "scripts/check-x", "#!/bin/sh\nexit 1\n", "build: tweak a gate")
    record("E2E: an edit to a gate script beside the escape, NO trailer, --enforce -> exit 1",
           run_gate(repo, new_tool, gate_edit, enforce=True) == 1)

    # the NEW cage entry: a rust-toolchain.toml channel bump WITHOUT the trailer -> exit 1
    rt = _commit(repo, "rust-toolchain.toml", "[toolchain]\nchannel = \"evil\"\n", "build: bump the toolchain")
    record("E2E: rust-toolchain.toml channel bump, NO trailer, --enforce -> exit 1 (the new cage entry)",
           run_gate(repo, gate_edit, rt, enforce=True) == 1)

    # a chore(todo) subject touching lefthook.yml -> exit 1 (no check-off escape for an L(-1) edit)
    fake = _commit(repo, "lefthook.yml", "x: 4\n", "chore(todo): sneaky abgehakt")
    record("E2E: chore(todo) subject over lefthook.yml -> exit 1 (no check-off escape for L(-1))",
           run_gate(repo, rt, fake, enforce=True) == 1)

    # an ALL-ZEROS base (a brand-new ref's github.event.before) must NOT fatal-red under --enforce: it
    # routes to the tip-only fallback (the P1.66 ^{commit}-peel + strip('0')), not a fatal `rev-list
    # 0000..HEAD` -> exit 1. With a CLEAN non-L(-1) tip -> exit 0; the fallback STILL audits the tip,
    # so an L(-1) tip lacking the trailer -> exit 1 (not a blanket pass).
    clean_tip = _commit(repo, "README.md", "# zzz\n", "docs: another readme tweak")
    record("E2E: all-zeros base, clean tip, --enforce -> exit 0 (tip-only fallback, no rev-list 0000.. fatal)",
           run_gate(repo, "0" * 40, clean_tip, enforce=True) == 0)
    dirty_tip = _commit(repo, "lefthook.yml", "x: 9\n", "ci: hook tweak (no ack)")
    record("E2E: all-zeros base, L(-1) tip, no trailer, --enforce -> exit 1 (the tip is still audited)",
           run_gate(repo, "0" * 40, dirty_tip, enforce=True) == 1)

    # the cage-liveness audit end-to-end (P4.56.1): a cage carrying a dead glob (the bare
    # `engines.lock`, which no tracked temp-repo path matches - the root-anchored-mis-spelling
    # shape) reds the gate under --enforce even though the audited tip commit itself is clean
    (repo / "cage-dead.toml").write_text(
        'patterns = ["lefthook.yml", "engines.lock", ".lfsconfig"]\n', encoding="utf-8")
    record("E2E: a dead cage glob (bare engines.lock) -> exit 1 under --enforce (clean tip)",
           run_gate(repo, "0" * 40, clean_tip, enforce=True, cage=repo / "cage-dead.toml") == 1)
    # the paired posture leg: the SAME dead cage without --enforce warns but exits 0 - this
    # discriminates the audit's fail-soft arm AND keeps the enforce leg honest (an unrelated
    # exit-1 source could not fake this pair, since it would red this soft leg too)
    record("E2E: the same dead cage glob, no --enforce -> exit 0 (fail-soft warns only)",
           run_gate(repo, "0" * 40, clean_tip, enforce=False, cage=repo / "cage-dead.toml") == 0)
    # the escape audit end-to-end, each shape isolated on the clean tip: the escaped path is tracked
    # and caged by scripts/** unless the leg is about exactly that
    _esc_pats = 'patterns = ["lefthook.yml", ".lfsconfig", "scripts/**"]\n'
    for leg, body in (("a dead escape (untracked path)", _esc_pats + _LT.format(p="scripts/gone", r="r")),
                      ("a refused escape (gate-shaped, tracked)", _esc_pats + _LT.format(p="scripts/check-x", r="r")),
                      ("an orphan escape (no pattern cages it)",
                       'patterns = ["lefthook.yml", ".lfsconfig"]\n' + _LT.format(p="scripts/buildtool", r="r"))):
        _c = repo / "cage-esc.toml"
        _c.write_text(body, encoding="utf-8")
        record(f"E2E: {leg} -> exit 1 under --enforce (clean tip)",
               run_gate(repo, "0" * 40, clean_tip, enforce=True, cage=_c) == 1)
        record(f"E2E: {leg}, no --enforce -> exit 0 (fail-soft warns only)",
               run_gate(repo, "0" * 40, clean_tip, enforce=False, cage=_c) == 0)
    # a refused escape never un-cages its path in the trailer audit: the fail-soft run over the
    # gate_edit commit (scripts/check-x, no trailer) under a cage that declares scripts/check-x an
    # escape still names that commit (the exit code alone cannot show it - the liveness block
    # returns 1 first under --enforce, and a fail-soft run exits 0 either way)
    (repo / "cage-esc.toml").write_text(_esc_pats + _LT.format(p="scripts/check-x", r="r"), encoding="utf-8")
    _err = io.StringIO()
    with contextlib.redirect_stderr(_err):
        _rc = run_gate(repo, new_tool, gate_edit, enforce=False, cage=repo / "cage-esc.toml")
    record("E2E: a refused escape stays caged in the trailer audit (the unacked edit to it is still named)",
           _rc == 0 and "[scripts/check-x] without an `L-neg1-ack: owner` trailer" in _err.getvalue())
    (repo / "cage-esc.toml").write_text(_esc_pats + '\n[[loop_tool]]\npath = "scripts/buildtool"\n',
                                        encoding="utf-8")
    record("E2E: a malformed [[loop_tool]] table (no reason) -> exit 2 under --enforce (cage unreadable)",
           run_gate(repo, "0" * 40, clean_tip, enforce=True, cage=repo / "cage-esc.toml") == 2)
    # the CWD-independence catcher: the audit's `--full-name -- :/` pathspec is load-bearing - a
    # bare `git ls-files` is subtree-scoped, so from a subdirectory every glob would read dead and
    # a correct cage would false-red; this leg reds exactly on that regression. (Runs BEFORE the
    # .lfsconfig landing below - afterwards the main cage would report a stale declaration.)
    (repo / "sub").mkdir()
    record("E2E: the audit is CWD-independent (gate run from a subdirectory -> exit 0)",
           run_gate(repo, "0" * 40, clean_tip, enforce=True, subdir="sub") == 0)
    # the quotePath-bypass catcher: git octal-escapes+quotes a non-ASCII path by default
    # ("docs/security/schl\\303\\274ssel..."), which matches NO glob - the ack would silently not
    # be required; with core.quotePath=false pinned in git(), the caged path matches and FAILS.
    # (Also BEFORE the .lfsconfig landing, so this exit 1 can ONLY come from the missing trailer.)
    uml = _commit(repo, "docs/security/schlüssel-custody.md", "doc\n", "docs: add custody doc (no ack)")
    record("E2E: a caged NON-ASCII path without the trailer -> exit 1 (the quotePath bypass is closed)",
           run_gate(repo, "0" * 40, uml, enforce=True) == 1)
    # the stale-declaration catcher end-to-end: land the declared-targetless .lfsconfig (WITH the
    # ack trailer, so the trailer audit stays green and exit 1 can ONLY come from the stale
    # declaration - never masked by a trailer violation on the same commit)
    lfs_tip = _commit(repo, ".lfsconfig", "[lfs]\n", "chore: land the lfs override\n\nL-neg1-ack: owner")
    (repo / "cage-stale.toml").write_text(
        'patterns = ["lefthook.yml", ".lfsconfig"]\n', encoding="utf-8")
    record("E2E: a declared-targetless glob whose target landed -> exit 1 under --enforce (stale declaration)",
           run_gate(repo, "0" * 40, lfs_tip, enforce=True, cage=repo / "cage-stale.toml") == 1)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-l-neg1-ack] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
