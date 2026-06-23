#!/usr/bin/env python3
"""g24-l-neg1-ack.py - G24 self-test for check-l-neg1-ack (P0.2.14, G71).

Proves the L(-1)-ack change-control gate: (1) the glob matcher caging the right paths (a wrong glob
is a cage GAP), (2) that the trailer is the ONLY escape - there is NO check-off / `[!extern]`
exemption, so a check-off / `[!extern]` commit that touches an L(-1) file still REQUIRES the ack
(security-concept §2; the §2-vs-plan conflict resolves to §2), (3) the ACK trailer regex, and (4) the
end-to-end verdict in a REAL temp git repo - an L(-1)-touching commit WITHOUT the trailer fails under
--enforce (fail-soft without), WITH the trailer passes; a check-off / `[!extern]` commit OVER an
L(-1) file FAILS (no exemption), while a non-L(-1) commit (incl. a plan-only check-off) passes.
stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import os
import subprocess
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-l-neg1-ack"
_loader = importlib.machinery.SourceFileLoader("clna", str(SCRIPT))
_spec = importlib.util.spec_from_loader("clna", _loader)
m = importlib.util.module_from_spec(_spec)
_loader.exec_module(m)

# the REAL committed cage (the gate's DEFAULT_CAGE) - so the glob matcher is tested against production
REGEXES = m.load_patterns(m.DEFAULT_CAGE)
results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


# --- glob matcher / is_l_neg1: POSITIVES (caged) ----------------------------------------------
record("cage loaded (non-empty)", isinstance(REGEXES, list) and len(REGEXES) > 0)
for p in ("lefthook.yml", "scripts/check-l-neg1-ack", "scripts/check-branch-protection",
          "scripts/fastpath-docs-only", "scripts/run-gate-selftests", "scripts/install-gate-tools",
          "scripts/setup-dev", "scripts/gate-selftests/g24-l-neg1-ack.py", "scripts/gate-planes.toml",
          "scripts/l-neg1-files.toml", ".github/workflows/ci.yml", ".github/allowed_signers",
          "deny.toml", ".gitleaks.toml", "supply-chain/config.toml", "supply-chain/imports.lock",
          ".gitattributes", ".lfsconfig", "requirements-ci.txt", "src-tauri/capabilities/default.json",
          "rust-toolchain.toml", "engines.lock", "docs/security/build-gates.md",
          "docs/security/security-concept.md", "docs/process/build-loop.md"):
    record(f"L(-1) POSITIVE: {p}", m.is_l_neg1(p, REGEXES))

# --- is_l_neg1: NEGATIVES (NOT caged) ---------------------------------------------------------
for p in ("README.md", "src/main.rs", "src/ui.ts", "Cargo.toml", "Cargo.lock", "package.json",
          "docs/plan/P0-build-and-security.md",        # the plan is NOT L(-1) (only security/process docs are)
          "docs/SINGLE-SOURCE-OF-TRUTH.md",            # the SSOT is NOT under security/process
          "scripts/helper.py", "scripts/gen.sh",       # a non-gate script is not caged
          ".githubfoo/x", "docs/securityfoo/x", ""):   # prefix-confusion / empty
    record(f"NOT-caged NEGATIVE: {p!r}", not m.is_l_neg1(p, REGEXES))

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


def run_gate(repo: Path, base: str, head: str, *, enforce: bool) -> int:
    """Run check-l-neg1-ack inside `repo` (the gate uses git in CWD)."""
    cwd = os.getcwd()
    os.chdir(repo)
    try:
        argv = ["--base", base, "--head", head]
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

    # the NEW cage entry: a rust-toolchain.toml channel bump WITHOUT the trailer -> exit 1
    rt = _commit(repo, "rust-toolchain.toml", "[toolchain]\nchannel = \"evil\"\n", "build: bump the toolchain")
    record("E2E: rust-toolchain.toml channel bump, NO trailer, --enforce -> exit 1 (the new cage entry)",
           run_gate(repo, non, rt, enforce=True) == 1)

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

failed = [n for n, ok in results if not ok]
print(f"\n[g24-l-neg1-ack] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
