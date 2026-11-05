#!/usr/bin/env python3
"""g24-rust-lint-contract.py - G24 self-test for check-rust-lint-contract (P0.4.1, G3/G4/G14/G17).

Proves the structural freeze CATCHES a relaxed rustfmt.toml/clippy.toml (a missing newline_style /
test-allowance / MSRV / a wrong value) and that the deny-set CONTRACT is non-empty + the per-module
crate-attr assertion flags a crate root missing its required deny. Since P1.6 a real workspace
Cargo.toml exists, so the real-gate legs pin the TARGET-PRESENT + cargo-absent-plane skip path
(the GitHub ubuntu/windows runners ship a cargo, so keying liveness on which('cargo') is fooled —
the cargo-absent skip is mocked explicitly) + the G17 audit-db-present/absent guard. stdlib-only.
Exit 0 = all held.
"""
import importlib.machinery
import importlib.util
import os
import sys
import tempfile
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-rust-lint-contract"
_loader = importlib.machinery.SourceFileLoader("crlc", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("crlc", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def freeze(body: str, required: dict) -> int:
    with tempfile.TemporaryDirectory() as td:
        p = Path(td) / "x.toml"
        p.write_text(body, encoding="utf-8")
        return m.freeze_config(p, required, "test")


# --- rustfmt.toml freeze ----------------------------------------------------------------------
GOOD_FMT = 'edition = "2021"\nnewline_style = "Unix"\nmax_width = 100\n'
record("rustfmt freeze: the real committed rustfmt.toml passes", freeze(GOOD_FMT, m.RUSTFMT_REQUIRED) == 0)
record("rustfmt freeze: a missing newline_style is caught",
       freeze('edition = "2021"\nmax_width = 100\n', m.RUSTFMT_REQUIRED) >= 1)
record("rustfmt freeze: newline_style relaxed to Windows is caught (value mismatch)",
       freeze('edition = "2021"\nnewline_style = "Windows"\nmax_width = 100\n', m.RUSTFMT_REQUIRED) >= 1)
record("rustfmt freeze: a missing edition is caught", freeze('newline_style = "Unix"\nmax_width = 100\n', m.RUSTFMT_REQUIRED) >= 1)

# --- clippy.toml freeze -----------------------------------------------------------------------
GOOD_CLIPPY = 'allow-unwrap-in-tests = true\nallow-expect-in-tests = true\nmsrv = "1.96.0"\n'
record("clippy freeze: a complete clippy.toml passes", freeze(GOOD_CLIPPY, m.CLIPPY_REQUIRED) == 0)
record("clippy freeze: a missing allow-unwrap-in-tests is caught",
       freeze('allow-expect-in-tests = true\nmsrv = "1.96.0"\n', m.CLIPPY_REQUIRED) >= 1)
record("clippy freeze: allow-unwrap-in-tests flipped to false is caught (value mismatch)",
       freeze('allow-unwrap-in-tests = false\nallow-expect-in-tests = true\nmsrv = "1.96.0"\n', m.CLIPPY_REQUIRED) >= 1)
record("clippy freeze: a missing msrv is caught",
       freeze('allow-unwrap-in-tests = true\nallow-expect-in-tests = true\n', m.CLIPPY_REQUIRED) >= 1)
record("freeze: a missing config file -> exit-2 signal",
       m.freeze_config(Path(tempfile.gettempdir()) / "__no_such_lint__.toml", m.CLIPPY_REQUIRED, "x") == 2)

# --- the deny-set CONTRACT (REQUIRED_ATTRS) ---------------------------------------------------
record("contract: REQUIRED_ATTRS is non-empty + well-formed (freeze_contract clean)", m.freeze_contract() == 0)
record("contract: the no-panic + indexing + arithmetic + exhaustive-match denies are all present",
       {tok for _, _, toks in m.REQUIRED_ATTRS for tok in toks} >=
       {"clippy::unwrap_used", "clippy::expect_used", "clippy::panic", "clippy::indexing_slicing",
        "clippy::arithmetic_side_effects", "clippy::wildcard_enum_match_arm"})

# the detect-path glob must be §0.7's `detection` (NOT `detect` — the G1-r1 silent-target-absent fix)
record("contract: the indexing_slicing entry globs §0.7 `detection` (not `detect`)",
       all("detection" in g for _, globs, toks in m.REQUIRED_ATTRS if "clippy::indexing_slicing" in toks for g in globs))
record("contract: every REQUIRED_ATTRS glob is a src-tauri/src/*.rs module path",
       all(g.startswith("src-tauri/src/") and g.endswith(".rs") for _, globs, _ in m.REQUIRED_ATTRS for g in globs))

# --- the deny-context-aware per-module crate-attr assertion core (the P1 logic, G1-r1 hardened) ---
record("crate-attr: a crate root WITH the required deny -> nothing missing",
       m._attrs_missing("#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]\n",
                        ["clippy::unwrap_used", "clippy::panic"]) == [])
record("crate-attr: a crate root MISSING a required deny -> flagged",
       m._attrs_missing("#![deny(clippy::unwrap_used)]\n", ["clippy::unwrap_used", "clippy::panic"]) == ["clippy::panic"])
record("crate-attr: `clippy::panic` is NOT satisfied by `clippy::panic_fmt` (delimiter-bound, no substring false-pass)",
       m._attrs_missing("#![deny(clippy::panic_fmt, clippy::unwrap_used)]\n",
                        ["clippy::unwrap_used", "clippy::panic"]) == ["clippy::panic"])
record("crate-attr: a #![allow(...)] RELAXATION does NOT satisfy the deny (the literal opposite)",
       m._attrs_missing("#![allow(clippy::unwrap_used)]\n", ["clippy::unwrap_used"]) == ["clippy::unwrap_used"])
record("crate-attr: a deny token mentioned only in a // comment does NOT satisfy it",
       m._attrs_missing("// we deny clippy::unwrap_used elsewhere\nfn x() {}\n", ["clippy::unwrap_used"]) == ["clippy::unwrap_used"])
record("crate-attr: a deny token in a /* */ block comment does NOT satisfy it",
       m._attrs_missing("/* deny(clippy::panic) is a goal */\nfn x() {}\n", ["clippy::panic"]) == ["clippy::panic"])
record("crate-attr: #![forbid(...)] also satisfies the deny (forbid is stricter than deny)",
       m._attrs_missing("#![forbid(clippy::unwrap_used)]\n", ["clippy::unwrap_used"]) == [])
# G1 round 2: a deny token in a STRING LITERAL or mid-line is NOT an in-force attribute
record("crate-attr: a deny token inside a STRING LITERAL does NOT satisfy it",
       m._attrs_missing('pub const P: &str = "#![deny(clippy::unwrap_used)]";\nfn main() {}\n', ["clippy::unwrap_used"]) == ["clippy::unwrap_used"])
record("crate-attr: a deny token in a multi-line string (line-starting inside the string) does NOT satisfy it",
       m._attrs_missing('const X: &str = "\n#![deny(clippy::panic)]\n";\n', ["clippy::panic"]) == ["clippy::panic"])
# G1 round 2: only a module-inner #![allow] disqualifies; an item-level #[allow] (justified escape) does NOT
record("crate-attr: an ITEM-level #[allow(...)] (the // PANIC: justified escape) does NOT disqualify the module deny",
       m._attrs_missing("#![deny(clippy::unwrap_used)]\n#[allow(clippy::unwrap_used)] // PANIC: known-good fixture\nfn t() {}\n", ["clippy::unwrap_used"]) == [])
record("crate-attr: a module-inner #![allow(...)] DOES disqualify (the dangerous module-wide relaxation)",
       m._attrs_missing("#![deny(clippy::unwrap_used)]\n#![allow(clippy::unwrap_used)]\n", ["clippy::unwrap_used"]) == ["clippy::unwrap_used"])
# G1 round 3: a cfg_attr-wrapped allow (always-on / production-affecting) DISQUALIFIES; test-only does NOT
record("crate-attr: #![cfg_attr(all(), allow(...))] (always-on crate-wide relaxation) DOES disqualify",
       m._attrs_missing("#![deny(clippy::unwrap_used)]\n#![cfg_attr(all(), allow(clippy::unwrap_used))]\n", ["clippy::unwrap_used"]) == ["clippy::unwrap_used"])
record("crate-attr: #![cfg_attr(not(test), allow(...))] (relaxes production) DOES disqualify",
       m._attrs_missing("#![deny(clippy::unwrap_used)]\n#![cfg_attr(not(test), allow(clippy::unwrap_used))]\n", ["clippy::unwrap_used"]) == ["clippy::unwrap_used"])
record("crate-attr: #![cfg_attr(feature = \"x\", allow(...))] (feature-gated relaxation) DOES disqualify",
       m._attrs_missing('#![deny(clippy::unwrap_used)]\n#![cfg_attr(feature = "x", allow(clippy::unwrap_used))]\n', ["clippy::unwrap_used"]) == ["clippy::unwrap_used"])
record("crate-attr: #![cfg_attr(test, allow(...))] (test-ONLY) does NOT disqualify (the legitimate case)",
       m._attrs_missing("#![deny(clippy::unwrap_used)]\n#![cfg_attr(test, allow(clippy::unwrap_used))]\n", ["clippy::unwrap_used"]) == [])
record("crate-attr: a same-line `#![deny(...)] #![allow(...)]` is caught (allow not line-anchored)",
       m._attrs_missing("#![deny(clippy::unwrap_used)] #![allow(clippy::unwrap_used)]\n", ["clippy::unwrap_used"]) == ["clippy::unwrap_used"])
record("crate-attr: a rustfmt-WRAPPED multi-line #![cfg_attr(\\n all(),\\n allow(...)\\n)] is still caught (re.S)",
       m._attrs_missing("#![deny(clippy::unwrap_used)]\n#![cfg_attr(\n    all(),\n    allow(clippy::unwrap_used)\n)]\n", ["clippy::unwrap_used"]) == ["clippy::unwrap_used"])
record("crate-attr: a spaced `#![allow(clippy :: unwrap_used)]` relaxation is caught (:: whitespace normalised)",
       m._attrs_missing("#![deny(clippy::unwrap_used)]\n#![allow(clippy :: unwrap_used)]\n", ["clippy::unwrap_used"]) == ["clippy::unwrap_used"])
record("crate-attr: a spaced `#![deny(clippy :: panic)]` still SATISFIES (:: whitespace normalised both ways)",
       m._attrs_missing("#![deny(clippy :: panic)]\n", ["clippy::panic"]) == [])
# G1 round 2: a non-UTF-8 config fails closed (exit-2), not an uncaught UnicodeDecodeError crash
with tempfile.TemporaryDirectory() as _td:
    _bad = Path(_td) / "bad.toml"
    _bad.write_bytes(b'\xff\xfe newline_style = not-utf8')
    record("freeze: a non-UTF-8 config -> exit-2 (UnicodeDecodeError guarded, not a crash)",
           m.freeze_config(_bad, m.RUSTFMT_REQUIRED, "x") == 2)

# --- freeze uses tomllib: a DUPLICATE key is REJECTED (a hand last-wins reader would false-pass) ---
record("freeze: a duplicate-key config (relaxed-then-good) is REJECTED, not last-wins-accepted",
       freeze('newline_style = "Windows"\nnewline_style = "Unix"\nedition = "2021"\nmax_width = 100\n', m.RUSTFMT_REQUIRED) == 2)
record("freeze: a duplicate allow-unwrap-in-tests (false-then-true) is REJECTED",
       freeze('allow-unwrap-in-tests = false\nallow-unwrap-in-tests = true\nallow-expect-in-tests = true\nmsrv = "1.96.0"\n', m.CLIPPY_REQUIRED) == 2)
record("freeze: a malformed (non-TOML) config -> exit-2 signal", freeze("this is not = = toml [[\n", m.RUSTFMT_REQUIRED) == 2)

# --- the REAL gate: target-present (the P1.6 workspace) + cargo-absent-plane skip ---------------
# Liveness must NOT be keyed on which('cargo') ALONE: the GitHub ubuntu/windows runners ship a cargo,
# so the gate-tooling (L4-structural) plane would otherwise run the live tier (and `cargo audit
# --no-fetch` would fail cold on the db-less runner). These legs pin the deterministic cargo-absent
# skip path by mocking shutil.which, so they hold on every runner regardless of a pre-installed cargo
# (the P1.6 self-test-staleness fix; live fmt/clippy/test enforce at L1/L2 + the equipped Rust CI job).
def _with_cargo_absent(fn):
    saved = m.shutil.which
    m.shutil.which = lambda tool: None
    try:
        return fn()
    finally:
        m.shutil.which = saved


def _with_no_manifest(fn):
    saved = m._workspace_manifest
    m._workspace_manifest = lambda: None
    try:
        return fn()
    finally:
        m._workspace_manifest = saved


record("real gate: target-present + cargo-absent plane -> structural freeze + crate-attrs pass, live tools skip (rc 0)",
       _with_cargo_absent(lambda: m.main([])) == 0)
record("real gate: --full in the cargo-absent plane also passes (rc 0)",
       _with_cargo_absent(lambda: m.main(["--full"])) == 0)
record("real gate: the manifest-absent skip branch still returns 0 (defensive — the workspace could be removed)",
       _with_no_manifest(lambda: m.main([])) == 0)
record("real configs: the committed rustfmt.toml + clippy.toml pass their freeze",
       m.freeze_config(m.RUSTFMT, m.RUSTFMT_REQUIRED, "rustfmt") == 0 and
       m.freeze_config(m.CLIPPY, m.CLIPPY_REQUIRED, "clippy") == 0)
record("workspace: _workspace_manifest() resolves the real first-party ROOT workspace (P1.6), "
       "not None and not a tests/g53-fixture manifest",
       m._workspace_manifest() == m.ROOT / "Cargo.toml")

# --- run_live_tools: cargo absent in this plane -> SKIPS (0), not a fmt/clippy fail (P1-runway fix) -
record("run_live_tools(): cargo absent in this plane -> SKIP (0), not a fmt/clippy fail "
       "(live fmt/clippy/test enforce where cargo is present)",
       _with_cargo_absent(lambda: m.run_live_tools(False)) == 0)

# --- the G17 audit-db guard: `cargo audit --no-fetch` skips gracefully when the advisory-db is absent
# (a fresh machine / clean CI runner that ships cargo-audit but never fetched the RustSec db) ----------
with tempfile.TemporaryDirectory() as _td:
    _saved_ch = os.environ.get("CARGO_HOME")
    os.environ["CARGO_HOME"] = _td                  # a fresh CARGO_HOME with no advisory-db subdir
    try:
        record("audit-db: an absent advisory-db -> _advisory_db_present() False (the --no-fetch skip path arms)",
               m._advisory_db_present() is False)
        (Path(_td) / "advisory-db").mkdir()
        record("audit-db: a present advisory-db -> _advisory_db_present() True (the audit leg is NOT silently disarmed)",
               m._advisory_db_present() is True)
    finally:
        if _saved_ch is None:
            os.environ.pop("CARGO_HOME", None)
        else:
            os.environ["CARGO_HOME"] = _saved_ch

# --- the G17 `--ignore` sourcing (the fix: cargo-audit does NOT auto-read a repo-root audit.toml — its
# default config path is .cargo/audit.toml — so the live leg passes the ignore set on the CLI, sourced from
# audit.toml (== deny.toml). Without this the audit leg is blind to the ignore set and a REAL, fatal
# vulnerability advisory reddens G17 despite an owner-acked ignore — the quick-xml RUSTSEC-2026-0194/0195
# incident. Tested on SYNTHETIC configs (data-independent — survives a re-triage of any real id) + a
# real-file smoke.) --------------------------------------------------------------------------------------
with tempfile.TemporaryDirectory() as _td:
    _saved_at = m.AUDIT_TOML
    try:
        _syn = Path(_td) / "audit.toml"
        _syn.write_text('[advisories]\nignore = ["RUSTSEC-2099-0001", "RUSTSEC-2099-0002"]\n', encoding="utf-8")
        m.AUDIT_TOML = _syn
        record("audit-ignore: _audit_ignores() reads [advisories].ignore verbatim",
               m._audit_ignores() == ["RUSTSEC-2099-0001", "RUSTSEC-2099-0002"])
        # [Test-Change: push 4 C2a - old-obsolete+new-correct, build-gates G17 row] old: one argv, `cargo audit
        # --no-fetch` + the ignores, read Cargo.lock alone. new: one argv per CARGO_AUDIT_LOCKS lock, `--file <lock>`
        # after `--no-fetch`, the same ignore pairs after it, so fuzz/Cargo.lock is audited with the same ignore set.
        record("audit-argv: _audit_argv(lock) = `cargo audit --no-fetch --file <lock>` + a `--ignore <id>` pair per "
               "ignore, the same pairs for each lock",
               all(m._audit_argv(_l) == ["cargo", "audit", "--no-fetch", "--file", _l,
                                         "--ignore", "RUSTSEC-2099-0001", "--ignore", "RUSTSEC-2099-0002"]
                   for _l in ("Cargo.lock", "fuzz/Cargo.lock")))
        m.AUDIT_TOML = Path(_td) / "__absent__.toml"
        # [Test-Change: push 4 C2a - old-obsolete+new-correct, build-gates G17 row] old: the argv without ignores was
        # `cargo audit --no-fetch`; new: it is `cargo audit --no-fetch --file <lock>`, still with no --ignore.
        record("audit-ignore: an ABSENT audit.toml -> [] + argv has NO --ignore (target-absent, graceful)",
               m._audit_ignores() == []
               and m._audit_argv("Cargo.lock") == ["cargo", "audit", "--no-fetch", "--file", "Cargo.lock"])
        _bad = Path(_td) / "bad.toml"; _bad.write_text("this = = not toml [[\n", encoding="utf-8")
        m.AUDIT_TOML = _bad
        record("audit-ignore: a MALFORMED audit.toml -> [] (tomllib guarded, not a crash)",
               m._audit_ignores() == [])
        _noadv = Path(_td) / "noadv.toml"; _noadv.write_text("[other]\nx = 1\n", encoding="utf-8")
        m.AUDIT_TOML = _noadv
        record("audit-ignore: an audit.toml with no [advisories].ignore -> [] (no KeyError)",
               m._audit_ignores() == [])
        _nonutf8 = Path(_td) / "nonutf8.toml"; _nonutf8.write_bytes(b'\xff\xfe [advisories]\nignore = ["RUSTSEC-2099-0001"]')
        m.AUDIT_TOML = _nonutf8
        record("audit-ignore: a NON-UTF-8 audit.toml -> [] (UnicodeDecodeError guarded, not a crash — mirrors freeze_config)",
               m._audit_ignores() == [])
    finally:
        m.AUDIT_TOML = _saved_at
# [Test-Change: push 4 C2a - old-obsolete+new-correct, build-gates G17 row] old: the one argv carried every ignore;
# new: the argv of each CARGO_AUDIT_LOCKS lock carries every ignore.
record("audit-ignore: the REAL committed audit.toml -> non-empty set, every id passed as a --ignore value for each lock",
       len(m._audit_ignores()) > 0
       and all(a in m._audit_argv(_l) for a in m._audit_ignores() for _l in m.CARGO_AUDIT_LOCKS))

# --- the G17 lock set: the workspace lock and the fuzz harness's lock, each audited (push 4 C2a) -------------------
_ROOT = SCRIPT.parents[1]
record("CARGO_AUDIT_LOCKS is frozen at (Cargo.lock, fuzz/Cargo.lock), each a file of this tree (the fuzz-only "
       "crates are read by no other advisory scan; check-osv-advisories' census reads this tuple)",
       m.CARGO_AUDIT_LOCKS == ("Cargo.lock", "fuzz/Cargo.lock")
       and all((_ROOT / _l).is_file() for _l in m.CARGO_AUDIT_LOCKS))


def _audits_run(*, db_present: bool) -> list[list[str]] | str:
    """run_live_tools(False) with cargo present, cargo-audit answering `--version`, the advisory-db present or not and
    every live tool recorded instead of run (no child starts): the argvs of the G17 audit legs it ran."""
    calls: list[list[str]] = []
    saved = (m._run, m.shutil.which, m.subprocess.run, m._advisory_db_present)

    def fake_run(label: str, argv: list[str]) -> int:
        calls.append(list(argv))
        return 0

    m._run = fake_run
    m.shutil.which = lambda name, *a, **k: "/planted/" + name
    m.subprocess.run = lambda argv, **kw: m.subprocess.CompletedProcess(argv, 0, b"", b"")
    m._advisory_db_present = lambda: db_present
    try:
        m.run_live_tools(False)
    except Exception as e:                       # a crash is a red leg with its reason, never a pass
        return f"<run_live_tools raised {type(e).__name__}: {e}>"
    finally:
        m._run, m.shutil.which, m.subprocess.run, m._advisory_db_present = saved
    return [c for c in calls if c[:2] == ["cargo", "audit"]]


_ran = _audits_run(db_present=True)
record("run_live_tools: with cargo-audit and a local advisory-db, one audit per CARGO_AUDIT_LOCKS lock, in order, "
       "each the lock's _audit_argv",
       _ran == [m._audit_argv(_l) for _l in m.CARGO_AUDIT_LOCKS])
record("run_live_tools: without a local advisory-db, no audit runs (the --no-fetch skip with its notice)",
       _audits_run(db_present=False) == [])

# --- binary leg: the pinned cargo-audit honours --file against a one-advisory RustSec database ---------------------
# A planted two-crate lock and a database holding one advisory for its second crate: the lock's _audit_argv (plus
# `--db` for the planted database) must exit 1 naming the advisory, so a cargo-audit that ignored `--file` (and read
# the empty temp dir's absent Cargo.lock, exit 2) or the database reds this leg; the fixed version exits 0. Skips
# with a notice when the pinned cargo-audit or cargo is absent (the L4 Linux gate-tooling leg has both).
_AUDIT_BIN = next((p for p in (_ROOT / ".gate-tools" / "bin" / n for n in ("cargo-audit", "cargo-audit.exe"))
                   if p.is_file()), None)
_ADVISORY = """```toml
[advisory]
id = "RUSTSEC-2099-0001"
package = "g24-fixture-vuln"
date = "2099-01-01"
url = "https://example.invalid/RUSTSEC-2099-0001"
categories = ["memory-corruption"]

[versions]
patched = [">= 0.2.0"]
```

# A planted advisory for the G24 self-test

One advisory, one crate, so the binary leg never depends on the live RustSec database.
"""
_FIXTURE_LOCK = """# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "g24-fixture-root"
version = "0.1.0"
dependencies = [
 "g24-fixture-vuln",
]

[[package]]
name = "g24-fixture-vuln"
version = "{v}"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "0000000000000000000000000000000000000000000000000000000000000000"
"""
if _AUDIT_BIN is None or m.shutil.which("cargo") is None:
    print("[g24-rust-lint-contract] SKIP the cargo-audit binary leg - the pinned cargo-audit (.gate-tools/bin) or "
          "cargo is absent here; the L4 Linux gate-tooling leg runs it.")
else:
    with tempfile.TemporaryDirectory() as _td:
        _T = Path(_td)
        (_T / "db" / "crates" / "g24-fixture-vuln").mkdir(parents=True)
        (_T / "db" / "crates" / "g24-fixture-vuln" / "RUSTSEC-2099-0001.md").write_text(_ADVISORY, encoding="utf-8")
        (_T / "empty").mkdir()
        _env = dict(os.environ, PATH=os.pathsep.join([str(_AUDIT_BIN.parent), os.environ.get("PATH", "")]))

        def _audit(version: str) -> tuple[int | None, str]:
            lock = _T / f"lock-{version}" / "Cargo.lock"
            lock.parent.mkdir(exist_ok=True)
            lock.write_text(_FIXTURE_LOCK.format(v=version), encoding="utf-8")
            try:
                p = m.subprocess.run(m._audit_argv(str(lock)) + ["--db", str(_T / "db")], cwd=str(_T / "empty"),
                                     env=_env, capture_output=True, text=True, encoding="utf-8", errors="replace",
                                     timeout=300)
            except (OSError, m.subprocess.TimeoutExpired) as e:
                return None, f"{type(e).__name__}: {e}"
            return p.returncode, p.stdout + p.stderr

        _rc, _out = _audit("0.1.0")
        record("binary: the pinned cargo-audit over a planted lock pinning g24-fixture-vuln 0.1.0 (--file, from an "
               "empty cwd) against the one-advisory database -> exit 1 naming RUSTSEC-2099-0001",
               _rc == 1 and "RUSTSEC-2099-0001" in _out)
        _rc, _out = _audit("0.2.0")
        record("binary: the same lock at the patched 0.2.0 -> exit 0", _rc == 0)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-rust-lint-contract] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
