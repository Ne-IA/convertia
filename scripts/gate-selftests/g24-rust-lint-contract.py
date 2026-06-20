#!/usr/bin/env python3
"""g24-rust-lint-contract.py - G24 self-test for check-rust-lint-contract (P0.4.1, G3/G4/G14/G17).

Proves the structural freeze CATCHES a relaxed rustfmt.toml/clippy.toml (a missing newline_style /
test-allowance / MSRV / a wrong value) and that the deny-set CONTRACT is non-empty + the per-module
crate-attr assertion flags a crate root missing its required deny. The live fmt/clippy/test/audit are
target-absent today (no workspace Cargo.toml) so the real gate skips. stdlib-only. Exit 0 = all held.
"""
import importlib.machinery
import importlib.util
import sys
import tempfile
from pathlib import Path

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

# --- the REAL gate: configs freeze + target-absent skip (no workspace Cargo.toml today) -------
record("real gate: structural freeze passes + skips live tools today (target-absent, rc 0)", m.main([]) == 0)
record("real gate: --full also passes today (target-absent)", m.main(["--full"]) == 0)
record("real configs: the committed rustfmt.toml + clippy.toml pass their freeze",
       m.freeze_config(m.RUSTFMT, m.RUSTFMT_REQUIRED, "rustfmt") == 0 and
       m.freeze_config(m.CLIPPY, m.CLIPPY_REQUIRED, "clippy") == 0)
record("workspace: the tests/g53-fixture Cargo.tomls are NOT mistaken for the first-party workspace",
       m._workspace_manifest() is None)

# --- the P1-runway fix: cargo absent in this plane -> run_live_tools SKIPS (0), not a fmt/clippy fail -
def _live_skip_when_cargo_absent() -> int:
    saved = m.shutil.which
    m.shutil.which = lambda tool: None
    try:
        return m.run_live_tools(False)
    finally:
        m.shutil.which = saved


record("run_live_tools(): cargo absent in this plane -> SKIP (0), not a fmt/clippy fail "
       "(P1-runway fix; live fmt/clippy/test enforce where cargo is present)",
       _live_skip_when_cargo_absent() == 0)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-rust-lint-contract] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
