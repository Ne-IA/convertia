#!/usr/bin/env python3
"""g24-fuzz-contract.py - G24 self-test for check-fuzz-contract (P0.4.3, G48/G16).

Proves the contract freeze CATCHES a gutted target/bounds/fixture/overflow set + a floating/impossible
nightly + a missing gate-tools.toml, SKIPS target-absent, and - the load-bearing part (the G1-r1 P0) -
the LIVE tier binds each target to a REAL artifact: it CATCHES a HOLLOW-STUB fuzz_targets/<key>.rs (no
fuzz_target! macro) and a COMMENT-ONLY mention of targets/bounds/fixtures, NOT just a bare stem/substring.
stdlib-only. Exit 0 = all held.
"""
import importlib.machinery
import importlib.util
import re
import shutil
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-fuzz-contract"
_loader = importlib.machinery.SourceFileLoader("cfc", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cfc", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


# --- freeze_contract -----------------------------------------------------------------------------
record("freeze: the real committed contract sets pass", m.freeze_contract() == 0)
for attr in ("G48_FUZZ_TARGETS", "IPC_PROPTEST_TARGETS", "LIBFUZZER_BOUNDS", "REQUIRED_FIXTURES",
             "NUMERIC_OVERFLOW_BOUNDARIES"):
    saved = getattr(m, attr)
    try:
        setattr(m, attr, [])
        record(f"freeze: an empty {attr} is caught", m.freeze_contract() >= 1)
    finally:
        setattr(m, attr, saved)
_st = m.G48_FUZZ_TARGETS
try:
    m.G48_FUZZ_TARGETS = [("detect", "x"), ("zip_slip", "y")]   # drop fs_guard_resolve_identity + csv_tsv
    record("freeze: a missing mandatory libFuzzer target (fs_guard/csv_tsv) is caught", m.freeze_contract() >= 1)
finally:
    m.G48_FUZZ_TARGETS = _st
_sp = m.IPC_PROPTEST_TARGETS
try:
    m.IPC_PROPTEST_TARGETS = [("ipc_serde_proptest", "x")]   # drop ipc_numeric_overflow
    record("freeze: a missing per-numeric-IPC-arg overflow proptest is caught", m.freeze_contract() >= 1)
finally:
    m.IPC_PROPTEST_TARGETS = _sp

# --- freeze_nightly_pin --------------------------------------------------------------------------
record("nightly: the real gate-tools.toml fuzz_nightly (date-pinned) passes", m.freeze_nightly_pin() == 0)


def nightly_with(body: str | None) -> int:
    td = Path(tempfile.mkdtemp(prefix="g24-fuzz-"))
    saved = m.GATE_TOOLS
    try:
        if body is None:
            m.GATE_TOOLS = td / "__absent__.toml"     # never created
        else:
            (td / "gt.toml").write_text(body, encoding="utf-8")
            m.GATE_TOOLS = td / "gt.toml"
        return m.freeze_nightly_pin()
    finally:
        m.GATE_TOOLS = saved
        shutil.rmtree(td, ignore_errors=True)


record("nightly: a DATE-pinned channel passes", nightly_with('[toolchain]\nfuzz_nightly = "nightly-2026-06-16"\n') == 0)
record("nightly: a bare floating `nightly` is caught", nightly_with('[toolchain]\nfuzz_nightly = "nightly"\n') >= 1)
record("nightly: a calendar-impossible date (2026-13-40) is caught",
       nightly_with('[toolchain]\nfuzz_nightly = "nightly-2026-13-40"\n') >= 1)
record("nightly: an absent fuzz_nightly is caught", nightly_with('[toolchain]\nrust_stable = "1.96.0"\n') >= 1)
record("nightly: a missing gate-tools.toml -> exit-2 signal", nightly_with(None) == 2)


def harness(tree: dict) -> int:
    """Run the live harness assertion against a synthetic fuzz/ tree (patching FUZZ_DIR)."""
    td = Path(tempfile.mkdtemp(prefix="g24-fuzz-h-"))
    saved = m.FUZZ_DIR
    try:
        for rel, body in tree.items():
            p = td / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(body, encoding="utf-8")
        m.FUZZ_DIR = td
        return m.assert_harness_live()
    finally:
        m.FUZZ_DIR = saved
        shutil.rmtree(td, ignore_errors=True)


def complete_tree() -> dict:
    """A COMPLETE, STRUCTURALLY-real synthetic fuzz/: a real fuzz_target! per fuzz_targets/<key>.rs, a
    committed corpus file per fixture, and the bounds in a NON-comment run-script (not a comment)."""
    tree = {f"fuzz_targets/{k}.rs": "fuzz_target!(|data: &[u8]| { let _ = run(data); });\n"
            for k, _ in m.G48_FUZZ_TARGETS}
    for fid, _ in m.REQUIRED_FIXTURES:
        tree[f"corpus/{fid}"] = "seed-bytes\n"
    tree["run.sh"] = "cargo fuzz run detect -- " + " ".join(f"{b}=1" for b in m.LIBFUZZER_BOUNDS) + "\n"
    return tree


# target-absent must be HERMETIC (patch FUZZ_DIR to a nonexistent path like every other leg): since
# P3.73 landed the real fuzz/, a bare `m.main([])` runs the LIVE tier against the REAL repo tree -
# the leg then tests "the committed tree is green" (accidentally green pre-P3.73-ruling, red the
# moment a new REQUIRED_FIXTURES row precedes its seed file), not "absent fuzz/ skips". [Fixed with
# the 2026-07-21 P3.73 P0 ruling's 9th fixture row, which exposed the non-hermetic call.]
def main_with_fuzz_dir(path: Path) -> int:
    saved = m.FUZZ_DIR
    try:
        m.FUZZ_DIR = path
        return m.main([])
    finally:
        m.FUZZ_DIR = saved


_absent_td = Path(tempfile.mkdtemp(prefix="g24-fuzz-absent-"))
try:
    record("target-absent: no fuzz/ -> main skips (exit 0)",
           main_with_fuzz_dir(_absent_td / "__no_fuzz__") == 0)
finally:
    shutil.rmtree(_absent_td, ignore_errors=True)
record("harness: a COMPLETE structurally-real fuzz/ passes", harness(complete_tree()) == 0)

# THE G1-r1 P0 regression guards: hollow-stub + comment-only false-passes MUST be CAUGHT.
t = complete_tree(); t["fuzz_targets/detect.rs"] = "fn detect(_d: &[u8]) {}\n"   # stem matches, NO fuzz_target!
record("harness: a HOLLOW-STUB target file (no fuzz_target! macro) is CAUGHT", harness(t) >= 1)

t = complete_tree(); t["fuzz_targets/detect.rs"] = "// fuzz_target!(|d: &[u8]| {});  (commented out)\n"
record("harness: a fuzz_target! only in a COMMENT is CAUGHT", harness(t) >= 1)

t = complete_tree(); t["fuzz_targets/detect.rs"] = 'fn main() { let _name = "fuzz_target!"; }\n'   # bare token in a STRING
record("harness: fuzz_target! only as a STRING-LITERAL token (no invocation) is CAUGHT", harness(t) >= 1)

t = complete_tree(); del t["fuzz_targets/detect.rs"]
t["notes.txt"] = "detect target harness lives elsewhere\n"   # prose mention must NOT satisfy it
record("harness: a MISSING (non-dormant) target file (mentioned only in prose) is CAUGHT", harness(t) >= 1)

t = complete_tree(); t["run.sh"] = "cargo fuzz run detect   # -rss_limit_mb=1 -max_len=1 (commented bounds)\n"
record("harness: bounds present only in a COMMENT are CAUGHT", harness(t) >= 1)

t = complete_tree()
for fid, _ in m.REQUIRED_FIXTURES[1:]:
    t.pop(f"corpus/{fid}", None)
record("harness: a fuzz/ missing G16 bound-firing corpus files is CAUGHT", harness(t) >= 1)

# --- per-target dormancy (the 2026-07-21 P3.73 fork ruling): absence of a DORMANT target/fixture is
# tolerated ONLY while its activating box is unchecked; the box's [x] arms it (self-healing); a PRESENT
# dormant harness is still validated; an unresolvable plan file/box id ARMS fail-closed. -------------


def dormancy(tree: dict, box_line: str, *, missing_plan: bool = False) -> int:
    """Run the harness with DORMANT_UNTIL/DORMANT_FIXTURES patched to a synthetic plan file whose
    P7.50.1/P4.35.1 boxes carry `box_line`'s checkbox state ([ ] / [x])."""
    td = Path(tempfile.mkdtemp(prefix="g24-fuzz-d-"))
    saved_u, saved_f = m.DORMANT_UNTIL, m.DORMANT_FIXTURES
    try:
        plan = td / "plan.md"
        if not missing_plan:
            plan.write_text(f"  - {box_line} **P7.50.1** [TEST] x · §1 · G48\n"
                            f"  - {box_line} **P4.35.1** [TEST] y · §1 · G48\n", encoding="utf-8")
        m.DORMANT_UNTIL = {"zip_slip": (str(plan), "P7.50.1"), "imgworker_ffi": (str(plan), "P4.35.1")}
        m.DORMANT_FIXTURES = {"zip_slip_entry": (str(plan), "P7.50.1")}
        return harness(tree)
    finally:
        m.DORMANT_UNTIL, m.DORMANT_FIXTURES = saved_u, saved_f
        shutil.rmtree(td, ignore_errors=True)


def p373_tree() -> dict:
    """The honest P3.73 shape: the four buildable targets + the seven buildable fixtures - NO
    zip_slip.rs / imgworker_ffi.rs / zip_slip_entry."""
    t = complete_tree()
    del t["fuzz_targets/zip_slip.rs"]
    del t["fuzz_targets/imgworker_ffi.rs"]
    del t["corpus/zip_slip_entry"]
    return t


record("dormancy: the P3.73-shape tree passes while the activating boxes are UNCHECKED",
       dormancy(p373_tree(), "[ ]") == 0)
record("dormancy: the SAME absences are CAUGHT once the activating boxes are [x] (the arming event)",
       dormancy(p373_tree(), "[x]") >= 3)
t = p373_tree(); t["fuzz_targets/zip_slip.rs"] = "fn zip_slip(_d: &[u8]) {}\n"   # present but hollow
record("dormancy: a PRESENT dormant harness is STILL validated (a hollow stub is CAUGHT)",
       dormancy(t, "[ ]") >= 1)
record("dormancy: a MISSING plan file ARMS the dormant targets (fail-closed, no silent waiver)",
       dormancy(p373_tree(), "[ ]", missing_plan=True) >= 3)
record("dormancy: _box_checked on an unfindable box id ARMS (fail-closed)",
       m._box_checked("docs/plan/P7-office.md", "P99.999") is True)
record("dormancy: a `[!]` (blocked) activating box is a LEGIBLE not-done state -> dormant tolerated",
       dormancy(p373_tree(), "[!]") == 0)
record("dormancy: the committed DORMANT_UNTIL/DORMANT_FIXTURES wiring resolves in the REAL plans "
       "(each activating box id is findable - a renumber would strand-and-arm, this leg makes it loud)",
       all((Path(m.ROOT) / pf).is_file()
           and re.search(r"^\s*- \[(x| |!)\] \*\*" + re.escape(box) + r"\*\*",
                         (Path(m.ROOT) / pf).read_text(encoding="utf-8"), flags=re.M)
           for pf, box in list(m.DORMANT_UNTIL.values()) + list(m.DORMANT_FIXTURES.values())))


# --- the 2026-07-22 sanitizer-posture leg (hermetic: patch m.FUZZ_WORKFLOW, never the real file) --
_POSTURE_OK = (
    "jobs:\n  fuzz:\n    steps:\n      - name: sweep\n        env:\n"
    "          SAN: ${{ runner.os == 'macOS' && 'none' || 'address' }}\n"
    "        run: |\n"
    '          cargo +nightly-2026-07-14 fuzz run "${t}" -s "${SAN}" -- ${BOUNDS}\n'
    "      - name: canary\n        run: |\n"
    '          if cargo +nightly-2026-07-14 fuzz run detect -s address -- ${BOUNDS}; then\n'
    '            echo "::warning title=ASAN healed on aarch64-apple-darwin::re-arm"\n'
    "          fi\n")


def posture(wf_body: str | None) -> int:
    """assert_sanitizer_posture() with FUZZ_WORKFLOW patched to a temp file (or an absent path)."""
    saved = m.FUZZ_WORKFLOW
    try:
        with tempfile.TemporaryDirectory() as td:
            p = Path(td) / "fuzz.yml"
            if wf_body is not None:
                p.write_text(wf_body, encoding="utf-8")
            m.FUZZ_WORKFLOW = p
            return m.assert_sanitizer_posture()
    finally:
        m.FUZZ_WORKFLOW = saved


record("posture: an absent fuzz.yml is target-absent (0 problems)", posture(None) == 0)
record("posture: the canonical split + SAN usage + canary passes", posture(_POSTURE_OK) == 0)
record("posture: the Linux arm flipped to 'none' is CAUGHT (the full-ASAN oracle drop)",
       posture(_POSTURE_OK.replace("&& 'none' || 'address'", "&& 'none' || 'none'")) >= 1)
record("posture: a literal un-split `-s none` on the sweep is CAUGHT",
       posture(_POSTURE_OK.replace('-s "${SAN}"', "-s none")) >= 1)  # the illegal token; expr+canary intact
record("posture: the canary (its ::warning:: marker) removed is CAUGHT",
       posture(_POSTURE_OK.replace("ASAN healed on aarch64-apple-darwin", "gone")) >= 1)
record("posture: a `-s address` mention in a COMMENT is not a violation (comment-stripped scan)",
       posture(_POSTURE_OK + "          # restore -s address someday\n") == 0)
record("posture: a `-s`-glued prose substring (a `dbus -sys stack` step name) is NOT a FP",
       posture(_POSTURE_OK + "      - name: deps (dbus -sys stack)\n        run: echo x\n") == 0)
record("posture: an unrelated `grep -s foo` in a run step is NOT a FP (value not forbidden)",
       posture(_POSTURE_OK + "      - name: x\n        run: grep -s foo bar\n") == 0)
# --- the R6-confirm bypass regressions (opus P2 spellings; sonnet P1 decoy; sonnet-2 P1 var-hoist) -
record("posture: the LONG form `--sanitizer none` is CAUGHT",
       posture(_POSTURE_OK.replace('-s "${SAN}"', "--sanitizer none")) >= 1)
record("posture: the `=`-glued `--sanitizer=none` is CAUGHT",
       posture(_POSTURE_OK.replace('-s "${SAN}"', "--sanitizer=none")) >= 1)
record("posture: the glued short form `-snone` is CAUGHT",
       posture(_POSTURE_OK.replace('-s "${SAN}"', "-snone")) >= 1)
record("posture: the single-quoted `-s 'none'` is CAUGHT (quote-normalized)",
       posture(_POSTURE_OK.replace('-s "${SAN}"', "-s 'none'")) >= 1)
record("posture: an exotic non-address sanitizer `-s thread` is CAUGHT (denylist, not just none)",
       posture(_POSTURE_OK.replace('-s "${SAN}"', "-s thread")) >= 1)
record("posture: a quoted `-s 'address'` PASSES (quote-normalized to a non-forbidden value)",
       posture(_POSTURE_OK.replace("-s address", "-s 'address'")) == 0)
record("posture: the VARIABLE-HOIST bypass is CAUGHT - `fuzz run` hidden in a shell var, `-s none` "
       "on a line with no literal `fuzz run` (whole-file denylist, no invocation-line scoping)",
       posture(_POSTURE_OK.replace(
           'cargo +nightly-2026-07-14 fuzz run "${t}" -s "${SAN}" -- ${BOUNDS}',
           'CMD="fuzz run"\n'
           '          cargo +nightly-2026-07-14 $CMD "${t}" -s none -- ${BOUNDS}')) >= 1)
record("posture: the DECOY bypass is CAUGHT - SAN rebound to unconditional none, the frozen "
       "expression parked in another key (binding-tied, not substring-presence)",
       posture(_POSTURE_OK.replace(
           "SAN: ${{ runner.os == 'macOS' && 'none' || 'address' }}",
           "SAN: ${{ 'none' }}\n"
           "          SAN_DECOY: \"runner.os == 'macOS' && 'none' || 'address'\"")) >= 1)
record("posture: a SECOND rebinding `SAN:` definition elsewhere is CAUGHT (exactly-one binding)",
       posture(_POSTURE_OK +
               "      - name: extra\n        env:\n          SAN: ${{ 'none' }}\n"
               "        run: echo x\n") >= 1)
record("posture: the REAL committed fuzz.yml satisfies the frozen posture",
       m.assert_sanitizer_posture() == 0)
_sp = m.SANITIZER_SPLIT_EXPR
try:
    m.SANITIZER_SPLIT_EXPR = "runner.os == 'macOS' && 'none' || 'none'"
    record("posture freeze: SANITIZER_SPLIT_EXPR without the 'address' fallback is caught",
           m.freeze_contract() >= 1)
finally:
    m.SANITIZER_SPLIT_EXPR = _sp
_fv = m.FORBIDDEN_SANITIZER_VALUES
try:
    m.FORBIDDEN_SANITIZER_VALUES = frozenset({"leak", "memory", "thread"})  # dropped `none`
    record("posture freeze: FORBIDDEN_SANITIZER_VALUES dropping `none` is caught",
           m.freeze_contract() >= 1)
    m.FORBIDDEN_SANITIZER_VALUES = frozenset({"none", "address"})  # forbids the ASAN leg itself
    record("posture freeze: FORBIDDEN_SANITIZER_VALUES forbidding `address` is caught",
           m.freeze_contract() >= 1)
finally:
    m.FORBIDDEN_SANITIZER_VALUES = _fv

failed = [n for n, ok in results if not ok]
print(f"\n{len(results) - len(failed)}/{len(results)} legs passed")
if failed:
    print("FAILED:", *failed, sep="\n  - ")
    sys.exit(1)
sys.exit(0)
