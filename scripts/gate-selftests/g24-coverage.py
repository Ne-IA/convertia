#!/usr/bin/env python3
"""g24-coverage.py - G24 self-test for check-coverage (P0.4.8, G27/G28).

Proves the structural freeze CANNOT be relaxed (a wrong diff_floor / target_line_floor / missing file),
the INCREASE-ONLY decrease-guard CATCHES a lowered floor (and passes a raise), and the per-domain
floor-comparison core fails a domain below its floor (NEVER averaged). The live per-domain/diff tier is
target-absent today (no cargo-llvm-cov/vitest report) so the real gate skips. stdlib-only. Exit 0 = held.
"""
import importlib.machinery
import importlib.util
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-coverage"
_loader = importlib.machinery.SourceFileLoader("ccov", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("ccov", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def _freeze_with(cfg, err=None):
    """freeze_floors() over a synthetic config (monkeypatch _load_floors — its default arg binds the real
    FLOORS path at def-time, so we replace the function, not the global)."""
    saved = m._load_floors
    m._load_floors = lambda path=None: (cfg, err)
    try:
        return m.freeze_floors()
    finally:
        m._load_floors = saved


def _dg(cur, prior):
    """decrease_guard() over synthetic current + prior configs (no git needed)."""
    sl, sp = m._load_floors, m._prior_floors
    m._load_floors = lambda path=None: (cur, None)
    m._prior_floors = lambda ref: prior
    try:
        return m.decrease_guard(None)
    finally:
        m._load_floors, m._prior_floors = sl, sp


# --- the structural freeze ---------------------------------------------------------------------
record("freeze: the real committed coverage-floors.toml passes", m.freeze_floors() == 0)
record("freeze: a clean synthetic floors config passes",
       _freeze_with({"meta": {"diff_floor": 80, "target_line_floor": 70}}) == 0)
record("freeze: a relaxed diff_floor (50, not 80) is caught",
       _freeze_with({"meta": {"diff_floor": 50, "target_line_floor": 70}}) >= 1)
record("freeze: a relaxed target_line_floor (60, not 70) is caught",
       _freeze_with({"meta": {"diff_floor": 80, "target_line_floor": 60}}) >= 1)
record("freeze: a missing coverage-floors.toml -> exit-2 signal", _freeze_with({}, "missing") == 2)
record("freeze: a non-numeric [line] floor (retype-to-ungate) is caught",
       _freeze_with({"meta": {"diff_floor": 80, "target_line_floor": 70}, "line": {"x": "lol"}}) >= 1)
record("freeze: a [branch] floor retyped to a bool is caught",
       _freeze_with({"meta": {"diff_floor": 80, "target_line_floor": 70}, "branch": {"crate::detection": True}}) >= 1)
record("freeze: BRANCH_FLOOR_DOMAINS is the no-harm/detect kernel",
       set(m.BRANCH_FLOOR_DOMAINS) == {"crate::detection", "crate::fs_guard", "crate::isolation"})

# --- the increase-only decrease-guard ----------------------------------------------------------
record("decrease-guard: a LOWERED line floor (70 -> 50) is caught",
       _dg({"line": {"convertia-core": 50}}, {"line": {"convertia-core": 70}}) >= 1)
record("decrease-guard: a LOWERED branch floor (80 -> 79) is caught",
       _dg({"branch": {"crate::detection": 79}}, {"branch": {"crate::detection": 80}}) >= 1)
record("decrease-guard: a RAISED floor (50 -> 70) is clean (raises are deliberate)",
       _dg({"line": {"convertia-core": 70}}, {"line": {"convertia-core": 50}}) == 0)
record("decrease-guard: an unchanged floor is clean",
       _dg({"line": {"convertia-core": 70}}, {"line": {"convertia-core": 70}}) == 0)
record("decrease-guard: BOTH empty (genuine P0) -> no-op", _dg({}, {}) == 0)
record("decrease-guard: a REMOVED floor (cur empty, prior had it) -> caught (removal un-gates the domain)",
       _dg({}, {"line": {"x": 70}}) >= 1)
record("decrease-guard: a REMOVED floor while another survives -> caught",
       _dg({"line": {"o": 70}}, {"line": {"o": 70, "x": 70}}) >= 1)
record("decrease-guard: no prior version (first landing / no base) -> no-op (fail-open w/o base)",
       _dg({"line": {"x": 70}}, None) == 0)

# --- _floor_map + the per-domain floor-comparison core (G27, never averaged) -------------------
record("_floor_map: flattens [line]/[branch] into kind:domain keys",
       m._floor_map({"line": {"x": 50}, "branch": {"y": 60}}) == {"line:x": 50.0, "branch:y": 60.0})
record("check_floor: a domain BELOW its floor is caught",
       m.check_floor({"a": 60.0}, {"a": 70.0}, "line") >= 1)
record("check_floor: a domain AT/ABOVE its floor is clean",
       m.check_floor({"a": 80.0}, {"a": 70.0}, "line") == 0)
record("check_floor: NEVER averaged — one domain above, one below -> the below one is caught",
       m.check_floor({"a": 95.0, "b": 40.0}, {"a": 70.0, "b": 70.0}, "line") >= 1)
record("check_floor: a floor for a not-yet-measured domain is not gated (added in P1)",
       m.check_floor({}, {"a": 70.0}, "line") == 0)

# --- the live per-domain tier (G27, P1.54): _rust_domain + _extract_measured + run_live ---------
# _rust_domain: product crates mapped, tooling -> None (excluded from the floors AND the diff).
record("_rust_domain: src-tauri/src/* -> convertia-core",
       m._rust_domain("/abs/src-tauri/src/run/mod.rs") == "convertia-core")
record("_rust_domain: crates/imgworker/src/* -> convertia-imgworker",
       m._rust_domain("crates/imgworker/src/ffi.rs") == "convertia-imgworker")
record("_rust_domain: xtask is tooling -> None (excluded)",
       m._rust_domain("xtask/src/main.rs") is None)
record("_rust_domain: a Windows-backslash path normalises",
       m._rust_domain("C:" + chr(92) + "r" + chr(92) + "src-tauri" + chr(92) + "src" + chr(92) + "main.rs")
       == "convertia-core")
# _rust_branch_domain: the security-kernel modules -> crate::<module>, else None.
record("_rust_branch_domain: detection module -> crate::detection",
       m._rust_branch_domain("src-tauri/src/detection/mod.rs") == "crate::detection")
record("_rust_branch_domain: a non-kernel module -> None",
       m._rust_branch_domain("src-tauri/src/run/mod.rs") is None)
record("SECURITY_BRANCH_MODULES mirrors BRANCH_FLOOR_DOMAINS",
       set(m.SECURITY_BRANCH_MODULES) == {"detection", "fs_guard", "isolation"})


def _extract(rust, ts):
    """_extract_measured() over synthetic LLVM-json (rust) + vitest-summary (ts) reports (monkeypatch
    _read_report — it is keyed on the RUST_COV / TS_COV paths)."""
    saved = m._read_report
    m._read_report = lambda p: rust if p == m.RUST_COV else (ts if p == m.TS_COV else None)
    try:
        return m._extract_measured()
    finally:
        m._read_report = saved


_RUST_FIX = {"data": [{"files": [
    {"filename": "/x/src-tauri/src/run/mod.rs", "summary": {"lines": {"covered": 8, "count": 10}, "branches": {"covered": 0, "count": 0}}},
    {"filename": "/x/src-tauri/src/detection/mod.rs", "summary": {"lines": {"covered": 4, "count": 4}, "branches": {"covered": 3, "count": 6}}},
    {"filename": "/x/xtask/src/main.rs", "summary": {"lines": {"covered": 0, "count": 99}, "branches": {"covered": 0, "count": 0}}},
    {"filename": "/x/crates/imgworker/src/main.rs", "summary": {"lines": {"covered": 0, "count": 2}, "branches": {"covered": 0, "count": 0}}},
]}]}
_TS_FIX = {"total": {"lines": {"covered": 10, "total": 20}},
           "/x/src/a.ts": {"lines": {"covered": 9, "total": 10}, "branches": {"covered": 1, "total": 2}},
           "/x/src/b.ts": {"lines": {"covered": 1, "total": 10}, "branches": {"covered": 0, "total": 0}}}
_ml, _mb = _extract(_RUST_FIX, _TS_FIX)
record("_extract_measured: convertia-core rolls up covered/count (NOT a per-file avg) = 12/14 = 85.7%",
       abs(_ml.get("convertia-core", 0) - 12 / 14 * 100) < 0.05)
record("_extract_measured: convertia-imgworker = 0/2 = 0.0%", _ml.get("convertia-imgworker") == 0.0)
record("_extract_measured: the xtask dev-bin is excluded (tooling)", "xtask" not in _ml)
record("_extract_measured: TS rolls up into the single `ui` domain = 10/20 = 50.0%", _ml.get("ui") == 50.0)
record("_extract_measured: branch maps the kernel module = crate::detection 3/6 = 50.0%",
       _mb.get("crate::detection") == 50.0)
record("_extract_measured: TS branch measured (ui 1/2 = 50.0%) — measured, floored only if a row exists",
       _mb.get("ui") == 50.0)


def _live(measured_line, measured_branch, cfg, present=True):
    """run_live() with a synthetic measured set (monkeypatch _extract_measured) + a forced report-presence
    (RUST_COV -> an existing file = present; a non-existent path = target-absent)."""
    se, sr, st = m._extract_measured, m.RUST_COV, m.TS_COV
    from pathlib import Path as _P
    m._extract_measured = lambda: (measured_line, measured_branch)
    m.RUST_COV = SCRIPT if present else _P("/no/such/coverage.json")
    m.TS_COV = _P("/no/such/coverage-summary.json")
    try:
        return m.run_live(cfg)
    finally:
        m._extract_measured, m.RUST_COV, m.TS_COV = se, sr, st


record("run_live: NO report on this leg -> target-absent skip (0)",
       _live({}, {}, {"line": {}, "branch": {}}, present=False) == 0)
record("run_live: a domain BELOW its line floor MUST fail (G27 planted violation)",
       _live({"convertia-core": 60.0}, {}, {"line": {"convertia-core": 70}}) >= 1)
record("run_live: every domain at/above its floor -> clean",
       _live({"convertia-core": 80.0, "ui": 95.0}, {}, {"line": {"convertia-core": 70, "ui": 90}}) == 0)
record("run_live: a measured branch-floor-domain with NO [branch] floor is caught (kernel contract)",
       _live({}, {"crate::detection": 50.0}, {"line": {}, "branch": {}}) >= 1)

# --- the live diff tier (G28, P1.54): _parse_lcov + _diff_counts + _diff_verdict ----------------
_LCOV = (
    f"SF:{str(m.ROOT).replace(chr(92), '/')}/src-tauri/src/run/mod.rs\n"
    "DA:10,1\nDA:11,0\nDA:12,5\nend_of_record\n"
    f"SF:{str(m.ROOT).replace(chr(92), '/')}/xtask/src/main.rs\n"
    "DA:3,0\nend_of_record\n"
    "SF:src/state/store.ts\nDA:69,0\nDA:83,1\nend_of_record\n"
)
_hits = m._parse_lcov(_LCOV)
record("_parse_lcov: an ABSOLUTE Rust SF is made repo-relative",
       "src-tauri/src/run/mod.rs" in _hits)
record("_parse_lcov: a tooling (.rs xtask) SF is dropped", "xtask/src/main.rs" not in _hits)
record("_parse_lcov: a relative TS SF is kept", "src/state/store.ts" in _hits)
record("_parse_lcov: DA hit counts are captured (10:1, 11:0, 12:5)",
       _hits.get("src-tauri/src/run/mod.rs") == {10: 1, 11: 0, 12: 5})
# _diff_counts: only EXECUTABLE PRODUCT lines (in the hit map) count; a non-executable / non-product
# changed line is ignored.
_changed = {"src-tauri/src/run/mod.rs": {10, 11, 12, 99}, "src/state/store.ts": {69, 83}, "README.md": {1}}
record("_diff_counts: executable product lines only -> 3 covered / 5 total",
       m._diff_counts(_changed, _hits) == (3, 5))
# _diff_verdict: the G28 planted violation — below floor MUST fail.
record("_diff_verdict: 3/5 = 60% < 80% floor MUST fail (G28 planted violation)",
       m._diff_verdict(3, 5, 80.0) >= 1)
record("_diff_verdict: 5/5 = 100% >= 80% -> clean", m._diff_verdict(5, 5, 80.0) == 0)
record("_diff_verdict: no changed executable product lines (0/0) -> vacuous pass",
       m._diff_verdict(0, 0, 80.0) == 0)
record("run_diff: no base -> fail-open skip (the G8/G70 diff-base posture)",
       m.run_diff({"meta": {"diff_floor": 80}}, None) == 0)

# --- the boot-glue diff exemption (G28, P2.135): _strip_rust + _apphandle_fn_ranges + _boot_glue_exempt --
# _strip_rust: comments/strings/chars are blanked so their `{`/`}`/`(` never miscount, length + newlines kept.
record("_strip_rust: a `{` in a line comment is blanked", "{" not in m._strip_rust("x // {\n"))
record("_strip_rust: a `}` in a block comment is blanked", "}" not in m._strip_rust("/* } */ y"))
record("_strip_rust: a NESTED block comment is fully blanked (Rust allows nesting)",
       "}" not in m._strip_rust("/* a /* b } */ c } */ x"))
record("_strip_rust: braces in a normal string are blanked", "{" not in m._strip_rust('let s = "{}";'))
record("_strip_rust: braces in a raw string are blanked", "{" not in m._strip_rust('let s = r#"a{b}c"#;'))
record("_strip_rust: a `'{'` char literal is blanked", "{" not in m._strip_rust("let c = '{';"))
record("_strip_rust: a `'a` lifetime + its real braces survive", m._strip_rust("fn f<'a>() {}").count("{") == 1)
record("_strip_rust: length is preserved (offset-stable)",
       len(m._strip_rust('a "str{}" // c\n/* b */ x')) == len('a "str{}" // c\n/* b */ x'))
record("_strip_rust: newline count is preserved",
       m._strip_rust('"a\nb" /* c\nd */\n').count("\n") == 3)

# _apphandle_fn_ranges: a fn whose SIGNATURE binds AppHandle is a range; a pure fn / a comment-or-string
# mention / a no-body decl is NOT (fail-closed). Generics + `-> T` + format!-braces never miscount.
_PURE = "fn pure(busy: bool, ready: bool) -> X {\n    if busy { 1 } else { 2 }\n}\n"
_GLUE = 'fn glue(app: &AppHandle, p: Vec<P>) {\n    let _ = format!("{}-{}", 1, 2);\n}\n'
record("_apphandle_fn_ranges: an AppHandle fn is a range",
       [n for n, _, _ in m._apphandle_fn_ranges(_GLUE)] == ["glue"])
record("_apphandle_fn_ranges: a pure (non-AppHandle) fn is NOT a range",
       m._apphandle_fn_ranges(_PURE) == [])
record("_apphandle_fn_ranges: AppHandle only in a string/comment (not the sig) is NOT a range",
       m._apphandle_fn_ranges('fn f() {\n    let _ = "AppHandle";  // AppHandle\n}\n') == [])
record("_apphandle_fn_ranges: a generic `fn g<R: Runtime>(app: &AppHandle<R>)` is detected (parens, not angles)",
       [n for n, _, _ in m._apphandle_fn_ranges("fn g<R: Runtime>(app: &AppHandle<R>) -> bool {\n    true\n}\n")] == ["g"])
record("_apphandle_fn_ranges: a `;`-terminated decl (no body) is NOT a range (fail-closed)",
       m._apphandle_fn_ranges("fn d(app: &AppHandle);\n") == [])
record("_apphandle_fn_ranges: an unbalanced body is NOT a range (fail-closed)",
       m._apphandle_fn_ranges("fn u(app: &AppHandle) {\n    if x {\n") == [])
# a format!-brace body ends at its OWN close, so a following pure fn is not swallowed
_TWO = _GLUE + _PURE
_two_ranges = m._apphandle_fn_ranges(_TWO)
record("_apphandle_fn_ranges: a format!-brace body ends correctly (the following pure fn is not swallowed)",
       [n for n, _, _ in _two_ranges] == ["glue"] and _two_ranges[0][2] == 3)
# the REAL main.rs: the 5 launch-funnel AppHandle fns are ranges; the pure helpers are NOT (the partition)
_main_src = (m.ROOT / "src-tauri" / "src" / "main.rs").read_text(encoding="utf-8")
_main_fns = {n for n, _, _ in m._apphandle_fn_ranges(_main_src)}
record("_apphandle_fn_ranges: real main.rs - the funnel + shells are exempt",
       {"forward_launch_intake", "converter_is_busy", "frontend_ready", "buffer_pending_intake"} <= _main_fns)
record("_apphandle_fn_ranges: real main.rs - the PURE helpers (intake_disposition/parse_path_args) are NOT exempt",
       "intake_disposition" not in _main_fns and "parse_path_args" not in _main_fns)
record("_apphandle_fn_ranges: real main.rs - fn main (no AppHandle in its sig) is NOT exempt (boot_invariants source-scan covers its body, not this diff exemption)",
       "main" not in _main_fns)

# _boot_glue_exempt: reads the HEAD tree; a product Rust file's AppHandle-fn lines are exempt, a non-product
# file contributes nothing; _diff_counts honours the exempt set (an uncovered exempt line stops counting).
_first = m._apphandle_fn_ranges(_main_src)[0]
_inside = _first[1] + 1                                # a line inside the first AppHandle fn body
_ex, _labels = m._boot_glue_exempt({"src-tauri/src/main.rs": {_inside}, "README.md": {1}})
record("_boot_glue_exempt: a product Rust AppHandle-fn line is exempt",
       _inside in _ex.get("src-tauri/src/main.rs", set()))
record("_boot_glue_exempt: a non-product file contributes nothing + a label is logged",
       "README.md" not in _ex and len(_labels) >= 1)
_eh = {"f.rs": {10: 1, 11: 0, 12: 0}}
record("_diff_counts: exempt=None is backward-compatible (an uncovered line still counts)",
       m._diff_counts({"f.rs": {10, 11, 12}}, _eh) == (1, 3))
record("_diff_counts: an uncovered EXEMPT boot-glue line stops counting (only the non-exempt line remains)",
       m._diff_counts({"f.rs": {10, 11, 12}}, _eh, {"f.rs": {11, 12}}) == (1, 1))

passed = sum(1 for _, ok in results if ok)
print(f"\n[g24-coverage] {passed}/{len(results)} assertions passed.")
sys.exit(0 if passed == len(results) else 1)
