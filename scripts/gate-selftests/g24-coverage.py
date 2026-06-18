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
       _freeze_with({"meta": {"diff_floor": 80, "target_line_floor": 70}, "branch": {"crate::detect": True}}) >= 1)
record("freeze: BRANCH_FLOOR_DOMAINS is the no-harm/detect kernel",
       set(m.BRANCH_FLOOR_DOMAINS) == {"crate::detect", "crate::fs_guard", "crate::isolation"})

# --- the increase-only decrease-guard ----------------------------------------------------------
record("decrease-guard: a LOWERED line floor (70 -> 50) is caught",
       _dg({"line": {"convertia-core": 50}}, {"line": {"convertia-core": 70}}) >= 1)
record("decrease-guard: a LOWERED branch floor (80 -> 79) is caught",
       _dg({"branch": {"crate::detect": 79}}, {"branch": {"crate::detect": 80}}) >= 1)
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

# --- the live tier is target-absent today ------------------------------------------------------
record("run_live: no cargo-llvm-cov/vitest report yet -> target-absent skip (0)",
       m.run_live({"line": {}, "branch": {}}) == 0)

passed = sum(1 for _, ok in results if ok)
print(f"\n[g24-coverage] {passed}/{len(results)} assertions passed.")
sys.exit(0 if passed == len(results) else 1)
