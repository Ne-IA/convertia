#!/usr/bin/env python3
"""g24-plan-lint.py - G24 self-test for plan-lint (P0.3.5, G7/G20).

FORMAT-check coverage: for each format check (_format.md §7), a CLEAN box yields no finding and a VIOLATING
box IS flagged (so no check is green-by-vacuity). Plus the base-case golden invariant: the real plan
passes (exit 0) and a deliberately-broken synthetic box-set exits non-empty. The doc-wide checks 1..33
get their own legs as they are built. stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import contextlib
import hashlib
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
# A git hook exports GIT_* variables (an absolute GIT_DIR in a linked worktree): under them, a git run from this
# file in a temp directory - its own, a gate's or a tool's - acts on the hooked repository. All but GIT_EXEC_PATH go.
for _k in [k for k in os.environ if k.startswith("GIT_") and k != "GIT_EXEC_PATH"]:
    os.environ.pop(_k)

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "plan-lint"
ROOT = Path(__file__).resolve().parents[2]
_loader = importlib.machinery.SourceFileLoader("pl", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("pl", _loader))
sys.modules["pl"] = m            # so the @dataclass annotations resolve under SourceFileLoader
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def box(bid="P0.1", marker="x", raw=None, indent=0, tags=None, title="Do the thing",
        refs="G7", needs=None, unlocked_by=None, notes=None, l_neg1=None, lineno=1, file="docs/plan/Px.md"):
    phase = int(bid[1:].split(".")[0])
    num = tuple(int(x) for x in bid[1:].split(".")[1:])
    return m.Box(box_id=bid, phase=phase, num=num, marker=(raw if raw is not None else marker),
                 raw_marker=(raw if raw is not None else marker), indent=indent,
                 tags=tags if tags is not None else ["GATE"], title=title, refs=refs,
                 file=file, lineno=lineno, needs=needs or [], unlocked_by=unlocked_by or [],
                 notes=notes or [], l_neg1=l_neg1 or [])


def ctx(boxes):
    return m.Ctx(root=ROOT, boxes=boxes, by_id={b.box_id: b for b in boxes}, plan_files=[])


# --- marker validity --------------------------------------------------------------------------
record("marker-validity: legal markers clean",
       m.fmt_marker_validity(ctx([box(raw=" "), box(raw="x"), box(raw="!", unlocked_by=["P0.1"]), box(raw="!extern")])) == [])
record("marker-validity: illegal [X]/[~]/[] flagged",
       len(m.fmt_marker_validity(ctx([box(bid="P0.1", raw="X"), box(bid="P0.2", raw="~"), box(bid="P0.3", raw="")]))) == 3)

# --- tag validity -----------------------------------------------------------------------------
record("tag-validity: [GATE] + [GATE,CI] clean",
       m.fmt_tag_validity(ctx([box(tags=["GATE"]), box(tags=["GATE", "CI"])])) == [])
record("tag-validity: bad tag / 3 tags / no tag flagged",
       len(m.fmt_tag_validity(ctx([box(bid="P0.1", tags=["WIP"]), box(bid="P0.2", tags=["A", "B", "C"]), box(bid="P0.3", tags=[])]))) >= 3)

# --- header well-formedness -------------------------------------------------------------------
record("header: a normal title clean", m.fmt_header_well_formedness(ctx([box(title="Build the gate")])) == [])
record("header: empty title + trailing period flagged",
       len(m.fmt_header_well_formedness(ctx([box(bid="P0.1", title=""), box(bid="P0.2", title="Ends badly.")]))) == 2)

# --- reference resolution (reads real spec/gates from ROOT) -----------------------------------
record("refs: a real Gnn ref clean", m.fmt_reference_resolution(ctx([box(refs="G7")])) == [])
record("refs: tooling-only clean", m.fmt_reference_resolution(ctx([box(refs="tooling-only")])) == [])
record("refs: a dangling §99.99 flagged",
       any("99.99" in f.msg for f in m.fmt_reference_resolution(ctx([box(refs="§99.99")]))))
record("refs: no ref AND no tooling-only flagged",
       len(m.fmt_reference_resolution(ctx([box(refs="")]))) >= 1)
record("refs: a real ref + tooling-only (mutually exclusive) flagged",
       any("mutually exclusive" in f.msg for f in m.fmt_reference_resolution(ctx([box(refs="G7 · tooling-only")]))))
# the §04/<file>#<slug> coverage-anchor leg (P4.60.3; _format.md §3.1/§7) - reads the REAL 04-formats tree
record("refs: §04 coverage anchor - a valid `§04/images.md#png` resolves to the `### PNG` heading -> clean",
       m.fmt_reference_resolution(ctx([box(refs="§04/images.md#png")])) == [])
record("refs: §04 coverage anchor - an absent slug (`§04/images.md#nonexistent`) -> flagged by the slug arm",
       any("slugs to 'nonexistent'" in f.msg
           for f in m.fmt_reference_resolution(ctx([box(refs="§04/images.md#nonexistent")]))))
record("refs: §04 coverage anchor - an absent file (`§04/nope.md#png`) -> flagged by the file arm",
       any("no docs/spec/04-formats/nope.md" in f.msg
           for f in m.fmt_reference_resolution(ctx([box(refs="§04/nope.md#png")]))))
# a bare or incomplete §04-shaped token never falls back to the `04` title anchor (the 04-formats title heading
# numbers itself `04`, so `§04` alone WOULD resolve without the arm) - one leg per shape
def _flagged(refs, needle):
    return any(needle in f.msg for f in m.fmt_reference_resolution(ctx([box(refs=refs)])))


record("refs: §04 coverage anchor - the bare token `§4` -> flagged as no numbered §4 tree", _flagged("§4", "is not a coverage anchor"))
record("refs: §04 coverage anchor - the bare token `§4.7` -> flagged as no numbered §4 tree", _flagged("§4.7", "is not a coverage anchor"))
record("refs: §04 coverage anchor - the bare token `§04` -> flagged, never resolved against the `04` title anchor", _flagged("§04", "is not a coverage anchor"))
record("refs: §04 coverage anchor - the bare token `§04.7` -> flagged, never resolved against the `04` title anchor", _flagged("§04.7", "is not a coverage anchor"))
record("refs: §04 coverage anchor - the incomplete token `§04/nope.md` (no slug) -> flagged as malformed", _flagged("§04/nope.md", "is not a well-formed reference token"))
record("refs: §04 coverage anchor - the incomplete token `§04/images.md` (no slug) -> flagged as malformed", _flagged("§04/images.md", "is not a well-formed reference token"))
record("refs: §04 coverage anchor - the incomplete token `§04/images.md#` (empty slug) -> flagged as malformed", _flagged("§04/images.md#", "is not a well-formed reference token"))
record("refs: §04 coverage anchor - the incomplete token `§04/#png` (no file) -> flagged as malformed", _flagged("§04/#png", "is not a well-formed reference token"))
record("refs: §04 coverage anchor - a valid anchor with a glued second anchor `§04/images.md#png#jpg` -> flagged as malformed", _flagged("§04/images.md#png#jpg", "is not a well-formed reference token"))
record("refs: §04 coverage anchor - a valid anchor with a glued tail `§04/images.md#png.md` -> flagged as malformed", _flagged("§04/images.md#png.md", "is not a well-formed reference token"))
record("refs: §04 coverage anchor - the numbered sibling with a glued tail `§1.7#foo` -> flagged as malformed", _flagged("§1.7#foo", "is not a well-formed reference token"))
record("refs: §04 coverage anchor - two glued refs `§1§04/images.md#png.7` never join into `§1.7` -> flagged as malformed", _flagged("§1§04/images.md#png.7", "is not a well-formed reference token"))
record("refs: §04 coverage anchor - a wrong-case slug `§04/images.md#PNG` -> flagged by the slug arm", _flagged("§04/images.md#PNG", "slugs to 'PNG'"))
record("refs: §04 coverage anchor - a wrong-case file name `§04/Images.md#png` -> flagged by the file arm on every OS", _flagged("§04/Images.md#png", "no docs/spec/04-formats/Images.md"))
record("refs: §04 coverage anchor - a real letter-suffixed numbered ref `§6.4.6a` stays clean", m.fmt_reference_resolution(ctx([box(refs="§6.4.6a · G33a")])) == [])
record("refs: §04 coverage anchor - a `§` not at the token start `(§1.7)` -> flagged as malformed", _flagged("(§1.7)", "is not a well-formed reference token"))
record("refs: §04 coverage anchor - a double letter suffix `§6.4.6ab` -> flagged as malformed", _flagged("§6.4.6ab", "is not a well-formed reference token"))
record("refs: §04 coverage anchor - an uppercase letter suffix `§6.4.6A` -> flagged as malformed", _flagged("§6.4.6A", "is not a well-formed reference token"))
record("refs: §04 coverage anchor - a zero-padded title number `§01` -> flagged, never resolved against the `01` title anchor", _flagged("§01", "is not a coverage anchor"))
record("refs: a gate id with a glued tail `G7#foo` -> flagged as malformed, never resolved as G7", _flagged("G7#foo", "is not a well-formed reference token"))
# the heading-level and fence rules, hermetically: a temp 04-formats file carrying a real `### `, a fenced `### `
# and a `## ` heading (the real tree has no fenced heading, so only a fixture can pin the fence rule)
with tempfile.TemporaryDirectory() as _tmp:
    _fmt = Path(_tmp) / "docs" / "spec" / "04-formats"
    _fmt.mkdir(parents=True)
    (_fmt / "t.md").write_bytes(b"## Level Two\n\n```\n### Fenced\n```\n\n### Real Entry\n")

    def _ctx_at(boxes):
        return m.Ctx(root=Path(_tmp), boxes=boxes, by_id={b.box_id: b for b in boxes}, plan_files=[])

    record("refs: §04 coverage anchor - hermetic: a `### ` heading resolves (`§04/t.md#real-entry`) -> clean",
           m.fmt_reference_resolution(_ctx_at([box(refs="§04/t.md#real-entry")])) == [])
    record("refs: §04 coverage anchor - hermetic: a `### ` inside a code fence is no heading (`#fenced`) -> flagged",
           any("slugs to 'fenced'" in f.msg for f in m.fmt_reference_resolution(_ctx_at([box(refs="§04/t.md#fenced")]))))
    record("refs: §04 coverage anchor - hermetic: a `## ` heading is not a format entry (`#level-two`) -> flagged",
           any("slugs to 'level-two'" in f.msg for f in m.fmt_reference_resolution(_ctx_at([box(refs="§04/t.md#level-two")]))))

# --- needs-targets + acyclic ------------------------------------------------------------------
record("needs: a resolvable target clean",
       m.fmt_needs_targets(ctx([box(bid="P0.1"), box(bid="P0.2", needs=["P0.1"])])) == [])
record("needs: a dangling target flagged",
       any("no such box" in f.msg for f in m.fmt_needs_targets(ctx([box(bid="P0.2", needs=["P9.9"])]))))
record("needs: a 2-cycle flagged",
       any("cycle" in f.msg for f in m.fmt_needs_targets(ctx([box(bid="P0.1", needs=["P0.2"]), box(bid="P0.2", needs=["P0.1"])]))))
# a top box's [x] is the AND of its sub-boxes: the historical P4.35 -> P4.35.1 -> P4.83 -> P4.35 deadlock replays as a
# cycle, the ordinary sub-box-needs-its-parent edge stays clean, and the 2026-09-15 re-cut shape is clean
record("needs: a sub-box needing a box that needs its parent -> a cycle (the P4.35.1 / P4.83 replay)",
       any("cycle" in f.msg for f in m.fmt_needs_targets(ctx([
           box(bid="P4.35", needs=["P4.34"]), box(bid="P4.35.1", indent=2, needs=["P4.35", "P4.83"]),
           box(bid="P4.34"), box(bid="P4.83", needs=["P4.35"])]))))
record("needs: a sub-box needing its own parent reads the parent's body -> clean",
       m.fmt_needs_targets(ctx([box(bid="P4.34"), box(bid="P4.35", needs=["P4.34"]),
                                box(bid="P4.35.1", indent=2, needs=["P4.35"])])) == [])
record("needs: a box needing a sub-box whose parent needs that box -> a cycle (the inherited-needs edge)",
       any("cycle" in f.msg for f in m.fmt_needs_targets(ctx([
           box(bid="P4.1", needs=["P4.2"]), box(bid="P4.1.1", indent=2), box(bid="P4.2", needs=["P4.1.1"])]))))
record("needs: the re-cut shape (the split before the parent, the sub-box after both) -> clean",
       m.fmt_needs_targets(ctx([box(bid="P4.34"), box(bid="P4.35", needs=["P4.34", "P4.83"]),
                                box(bid="P4.35.1", indent=2, needs=["P4.35", "P4.83"]),
                                box(bid="P4.83", needs=["P4.34"])])) == [])

# --- annotation pairing -----------------------------------------------------------------------
record("annot: unlocked-by under [!] clean",
       m.fmt_annotation_pairing(ctx([box(bid="P0.1"), box(bid="P0.2", raw="!", unlocked_by=["P0.1"])])) == [])
record("annot: unlocked-by under [x] flagged",
       any("only allowed under a [!]" in f.msg for f in m.fmt_annotation_pairing(ctx([box(bid="P0.1"), box(bid="P0.2", raw="x", unlocked_by=["P0.1"])]))))
record("annot: a [!] box with neither note nor unlocked-by flagged",
       any("needs a >-note" in f.msg for f in m.fmt_annotation_pairing(ctx([box(raw="!")]))))
record("annot: a [!extern] box with no note flagged",
       any("needs a >-note" in f.msg for f in m.fmt_annotation_pairing(ctx([box(raw="!extern")]))))

# --- sub-box consistency ----------------------------------------------------------------------
record("sub-box: a [x] parent with an [x] child clean",
       m.fmt_sub_box_consistency(ctx([box(bid="P0.1", raw="x"), box(bid="P0.1.1", raw="x", indent=2)])) == [])
record("sub-box: a [x] parent with an open [ ] child flagged",
       any("AND-of-children" in f.msg for f in m.fmt_sub_box_consistency(ctx([box(bid="P0.1", raw="x"), box(bid="P0.1.1", raw=" ", indent=2)]))))
record("sub-box: depth > one level flagged",
       any("deeper than one level" in f.msg for f in m.fmt_sub_box_consistency(ctx([box(bid="P0.1.2.3", indent=4)]))))
record("sub-box: odd indentation flagged",
       any("odd indentation" in f.msg for f in m.fmt_sub_box_consistency(ctx([box(bid="P0.1.1", indent=3)]))))

# --- numbering gap-free -----------------------------------------------------------------------
record("numbering: 1,2,3 clean",
       m.fmt_numbering_gap_free(ctx([box(bid="P0.1"), box(bid="P0.2"), box(bid="P0.3")])) == [])
record("numbering: a gap (1,3) flagged",
       any("not gap-free" in f.msg for f in m.fmt_numbering_gap_free(ctx([box(bid="P0.1"), box(bid="P0.3")]))))
record("numbering: a SUB-box gap (P0.1.1, P0.1.3) flagged",
       any("sub-boxes under" in f.msg for f in m.fmt_numbering_gap_free(
           ctx([box(bid="P0.1"), box(bid="P0.1.1", indent=2), box(bid="P0.1.3", indent=2)]))))
record("box-parse-completeness: a malformed box-id (no dotted segment) is a near-miss",
       bool(m._NEAR_BOX_RE.match("- [x] **P12** title")) and not m.BOX_RE.match("- [x] **P12** title"))
record("box-parse-completeness: a valid box-id parses (not a near-miss)",
       bool(m.BOX_RE.match("- [x] **P1.2** title")))

# --- the [!extern] sub-box rule and the l-neg1: route grammar (_format.md §2, §3.2, §5.3) --------------------
_EXT = {"raw": "!extern", "notes": ["an owner act"]}


def _open(bid, **kw):
    return box(bid=bid, raw=" ", **kw)


def _seq(*boxes):
    """The boxes in document order: each one's lineno is its position (the selection sorts by it)."""
    for i, b in enumerate(boxes, 1):
        b.lineno = i
    return list(boxes)


def _lneg1(boxes):
    return [f.msg for f in m.fmt_l_neg1_annotation(ctx(boxes))]


def _plan_root(tmp: Path, text: str, name: str = "P98-x.md") -> Path:
    d = tmp / "docs" / "plan"
    d.mkdir(parents=True, exist_ok=True)
    (d / name).write_bytes(text.encode("utf-8"))
    return tmp


def _mode(argv):
    """(exit code, stdout) of plan-lint's main on a mode argv; stderr is swallowed, an argparse exit is its code."""
    out = io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
        try:
            rc = m.main(argv)
        except SystemExit as e:
            rc = e.code
    return rc, out.getvalue()


record("sub-box: a [x] parent over an open [!extern] child flagged (the P4.56 / P4.56.3 shape)",
       any("AND-of-children" in f.msg for f in m.fmt_sub_box_consistency(
           ctx([box(bid="P4.56", raw="x"), box(bid="P4.56.3", indent=2, **_EXT)]))))
record("l-neg1 annot: the four routes are clean (same-push, sweep-tail, none, act on an [!extern] or [x] needs: target)",
       _lneg1([_open("P4.1", l_neg1=["same-push"]), _open("P4.2", l_neg1=["sweep-tail"]),
               _open("P4.3", l_neg1=["none"]), box(bid="P4.4", **_EXT),
               _open("P4.5", needs=["P4.4"], l_neg1=["act P4.4"]), box(bid="P4.6", raw="x"),
               _open("P4.7", needs=["P4.6"], l_neg1=["act P4.6"])]) == [])
record("l-neg1 annot: an unknown route (`same-box`, `same-commit`) flagged",
       len(_lneg1([_open("P4.1", l_neg1=["same-box"]), _open("P4.2", l_neg1=["same-commit"])])) == 2)
record("l-neg1 annot: an act target not in the box's needs: flagged",
       any("not in the box's needs:" in s for s in _lneg1([box(bid="P4.4", **_EXT), _open("P4.5", l_neg1=["act P4.4"])])))
record("l-neg1 annot: an act target that is [ ] flagged",
       any("is [ ]" in s for s in _lneg1([_open("P4.4"), _open("P4.5", needs=["P4.4"], l_neg1=["act P4.4"])])))
record("l-neg1 annot: an act target that is no box flagged",
       any("no such box" in s for s in _lneg1([_open("P4.5", needs=["P4.9"], l_neg1=["act P4.9"])])))
record("l-neg1 annot: two l-neg1: lines flagged",
       any("2 l-neg1: lines" in s for s in _lneg1([_open("P4.1", l_neg1=["same-push", "sweep-tail"])])))
record("l-neg1 annot: a route under an [!extern] box flagged",
       any("carries no l-neg1:" in s for s in _lneg1([box(bid="P4.1", l_neg1=["none"], **_EXT)])))
with tempfile.TemporaryDirectory() as _tl:
    _lb = m.load_plan(_plan_root(Path(_tl), "- [ ] **P98.1** [DOC] A box · tooling-only\n  l-neg1: same-push, none\n"))[1]
    record("l-neg1 annot: a joined pair on one line parses as ONE raw entry and is flagged (never comma-split)",
           _lb["P98.1"].l_neg1 == ["same-push, none"] and any("is not a route" in s for s in _lneg1([_lb["P98.1"]])))
record("l-neg1 annot: registered as a format check and run before every mode",
       m.FORMAT_CHECKS.get("format:l-neg1-annotation") is m.fmt_l_neg1_annotation
       and "format:l-neg1-annotation" in m._MODE_PRECHECKS)

# --- --next: the _format.md §6 selection (select_next is pure; incident replays + the contract) ---------------
_n = m.select_next(ctx(_seq(box(bid="P4.1"), _open("P4.2", needs=["P4.5"]), _open("P4.3"), box(bid="P4.4", **_EXT),
                            _open("P4.5", needs=["P4.4"]))))
record("next: the P4.13 incident - a box whose needs: closure reaches an open [!extern] is never selected; the next "
       "box outside the closure is", _n.target == "P4.3" and ("P4.2", {"P4.4"}) in _n.blocked)
_n = m.select_next(ctx(_seq(_open("P4.1"), box(bid="P4.1.1", raw="x", indent=2), _open("P4.1.2", indent=2),
                            box(bid="P4.1.3", indent=2, **_EXT))))
_n2 = m.select_next(ctx(_seq(_open("P4.1"), box(bid="P4.1.1", indent=2, **_EXT), _open("P4.1.2", indent=2))))
record("next: an open [!extern] sub-box never blocks its buildable sibling, above or below it (the P4.56.2 shape)",
       (_n.target, _n.order) == ("P4.1", ["P4.1.2"]) and (_n2.target, _n2.order) == ("P4.1", ["P4.1.2"]))
_n = m.select_next(ctx(_seq(_open("P4.1"), box(bid="P4.1.1", raw="x", indent=2), box(bid="P4.1.2", indent=2, **_EXT),
                            _open("P4.2", needs=["P4.1"]), _open("P4.3"))))
record("next: a box needing a parent over an [!extern] sub-box is skipped with that sub-box as its root",
       _n.target == "P4.3" and ("P4.1", {"P4.1.2"}) in _n.blocked and ("P4.2", {"P4.1.2"}) in _n.blocked)
_n = m.select_next(ctx([_open("P10.1", file="docs/plan/P10-x.md"), _open("P2.1", file="docs/plan/P2-x.md")]))
record("next: numeric phase order - P2 before P10, never the file-name order", _n.target == "P2.1")
_sw = box(bid="P2.2", title=m._SWEEP_TITLE + " over P2", **_EXT)
_n = m.select_next(ctx(_seq(box(bid="P2.1"), _sw, _open("P3.1", needs=["P2.2"]), _open("P3.2"))))
_sw.raw_marker = "x"
_n2 = m.select_next(ctx(_seq(box(bid="P2.1"), _sw, _open("P3.1", needs=["P2.2"]), _open("P3.2"))))
record("next: no P(n+1) box while the P(n) sweep box is open; its check-off opens P(n+1)",
       _n.target is None and _n.waiting == ["P2.2"] and _n2.target == "P3.1")
_n = m.select_next(ctx(_seq(_open("P4.1", needs=["P4.3", "P5.2"]), _open("P4.2"), _open("P4.3", needs=["P5.2"]),
                            box(bid="P4.4", title=m._SWEEP_TITLE + " over P4", **_EXT), _open("P5.1", needs=["P4.4"]),
                            _open("P5.2"))))
record("next: DECISION C - every unmet prerequisite before the box needing it, followed across phases",
       (_n.target, _n.order) == ("P4.1", ["P5.2", "P4.3", "P4.1"]))
_n = m.select_next(ctx(_seq(_open("P4.1", needs=["P4.2"]), _open("P4.1.1", indent=2), _open("P4.2"))))
record("next: a sub-box step inherits its parent's unmet needs: into the build order",
       (_n.target, _n.order) == ("P4.1", ["P4.2", "P4.1.1"]))
_n = m.select_next(ctx(_seq(_open("P4.1"), box(bid="P4.1.1", raw="x", indent=2), _open("P4.2"))))
record("next: an open parent whose sub-boxes are all [x] is its own step, never blocked without a root",
       (_n.target, _n.order, _n.blocked) == ("P4.1", ["P4.1"], []))
_n = m.select_next(ctx(_seq(box(bid="P4.1"), box(bid="P4.2", raw="!", unlocked_by=["P4.1"]), _open("P4.3"))))
_n2 = m.select_next(ctx(_seq(box(bid="P4.2", raw="!", unlocked_by=["P4.3"]), _open("P4.3"))))
record("next: a [!] box whose unlocked-by: is [x] is an unlock and counts as open; one with an open releaser is not",
       _n.target == "P4.2" and _n.unlocks == [("P4.2", ["P4.1"])] and _n2.target == "P4.3" and _n2.unlocks == [])
_n = m.select_next(ctx(_seq(_open("P0.1"), _open("P1.1", needs=["P0.1"]), _open("P1.2"))))
record("next: an open P0 box reached through needs: is a root (out of the loop's range)",
       _n.target == "P1.2" and ("P1.1", {"P0.1"}) in _n.blocked)
_n = m.select_next(ctx(_seq(_open("P4.1"), _open("P4.2", needs=["P4.3"]), box(bid="P4.3"))), {"P4.2": True})
_n2 = m.select_next(ctx(_seq(_open("P4.1", needs=["P4.3"]), _open("P4.2"), box(bid="P4.3", **_EXT))), {"P4.1": True})
record("next: a resumable park is answered before an earlier open box when its own needs: closure is clear; when it "
       "is not, it is skipped once with its root and never resumed",
       (_n.resume, _n.target) == ("P4.2", "P4.2")
       and (_n2.resume, _n2.target, _n2.blocked) == (None, "P4.2", [("P4.1", {"P4.3"})]))
_n = m.select_next(ctx(_seq(_open("P4.1"), box(bid="P4.2", **_EXT))), {"P4.2": True})
_n2 = m.select_next(ctx(_seq(_open("P4.1"), box(bid="P4.2", raw="!", unlocked_by=["P4.3"]), _open("P4.3"))),
                    {"P4.2": True})
_n3 = m.select_next(ctx(_seq(_open("P0.1"), _open("P1.1"))), {"P0.1": True})
record("next: a resumable park whose box the scan never selects - [!extern], [!] with an open releaser, an open P0 "
       "box - is skipped with the box as its root and never resumed",
       (_n.resume, _n.target, _n.blocked) == (None, "P4.1", [("P4.2", {"P4.2"})])
       and (_n2.resume, _n2.target, _n2.blocked) == (None, "P4.1", [("P4.2", {"P4.2"})])
       and (_n3.resume, _n3.target, _n3.blocked) == (None, "P1.1", [("P0.1", {"P0.1"})]))


def _parent_over_extern():
    """A parent whose every open sub-box needs an [!extern] box: nothing under it is buildable."""
    return ctx(_seq(box(bid="P4.1"), _open("P4.2"), _open("P4.2.1", indent=2, needs=["P4.3"]),
                    _open("P4.2.2", indent=2, needs=["P4.3"]), box(bid="P4.3", **_EXT)))


_n, _n0 = m.select_next(_parent_over_extern(), {"P4.2": True}), m.select_next(_parent_over_extern())
record("next: a resumable park on a parent with no buildable sub-box is skipped with its sub-boxes' roots - the "
       "answer the same plan gives with no park",
       (_n.resume, _n.target, _n.blocked, _n.waiting) == (None, None, [("P4.2", {"P4.3"})], ["P4.3"])
       and (_n0.target, _n0.blocked, _n0.waiting) == (None, [("P4.2", {"P4.3"})], ["P4.3"]))
_n = m.select_next(ctx(_seq(_open("P4.1"), _open("P4.2", needs=["P4.3"]), _open("P4.3"))), {"P4.2": True})
_n2 = m.select_next(ctx(_seq(_open("P4.1"), _open("P4.2"), box(bid="P4.2.1", raw="x", indent=2),
                             _open("P4.2.2", indent=2))), {"P4.2": True})
record("next: a resumed park carries its build order - an unmet buildable prerequisite first (DECISION C), a "
       "parent's open sub-box as its step",
       (_n.resume, _n.order) == ("P4.2", ["P4.3", "P4.2"]) and (_n2.resume, _n2.order) == ("P4.2", ["P4.2.2"]))
_n = m.select_next(ctx(_seq(_open("P4.1", needs=["P4.2"]), box(bid="P4.2", **_EXT))))
record("next: all blocked - no target, the roots named",
       _n.target is None and not _n.converged and _n.blocked == [("P4.1", {"P4.2"})] and _n.waiting == ["P4.2"])
_n = m.select_next(ctx(_seq(_open("P0.9"), box(bid="P1.1"), box(bid="P2.1"))))
record("next: converged - every P1..P11 box is [x]", _n.converged and _n.target is None)
_real = m.build_ctx(ROOT)
_n, _rg = m.select_next(_real, {}), m._plan_graph(_real)
record("next: the REAL plan's answer is well-formed - each build-order step has a clear closure, or no target and "
       "the roots named", (_n.target is not None and not any(m._stop_roots(_real, _rg, s) for s in _n.order))
       or (_n.target is None and bool(_n.converged or _n.blocked or _n.waiting)))
with tempfile.TemporaryDirectory() as _tn:
    _r = _plan_root(Path(_tn), "- [ ] **P98.1** [DOC] A · tooling-only\n  needs: P98.9\n")
    record("next: a malformed plan (a dangling needs:) exits 2 - a mode never answers over it",
           _mode(["--next", "--root", str(_r)])[0] == 2)
    _r = _plan_root(Path(_tn), "- [ ] **P98.1** [DOC] A · tooling-only\n  needs: P98.2\n"
                               "- [!extern] **P98.2** [DOC] B · tooling-only\n  > an owner act\n")
    _rc, _out = _mode(["--next", "--root", str(_r)])
    record("next: nothing buildable exits 1 and names each root with what it blocks",
           _rc == 1 and _out.startswith("nothing buildable") and "skipped: P98.2 [!extern] blocks P98.1" in _out)

# --- --show: the box-unpack incident (read from the header line, never from a line offset) --------------------
_SHOW_PLAN = ("## P98\n\n"
              "- [ ] **P98.1** [DOC] First box · tooling-only\n"
              "  needs: P98.2\n"
              "  l-neg1: none\n"
              "  > note one\n"
              "    an indented continuation line\n"
              "  - [x] **P98.1.1** [DOC] Sub one · tooling-only\n"
              "    > sub note\n"
              "  - [ ] **P98.1.2** [DOC] Sub two · tooling-only\n"
              "\n"
              "  > a note after a blank line\n"
              "\n"
              "- [ ] **P98.2** [DOC] Second box · tooling-only\n"
              "  > second note\n")
_sl = _SHOW_PLAN.splitlines()
with tempfile.TemporaryDirectory() as _ts:
    _r = _plan_root(Path(_ts), _SHOW_PLAN)
    _sc = m.build_ctx(_r)
    record("show: read from the header line - the needs:/l-neg1: lines, every note and sub-box, nothing of the "
           "next box", m.box_scope_text(_sc, "P98.1") == _sl[2:12])
    record("show: a sub-box's scope ends at its next sibling",
           m.box_scope_text(_sc, "P98.1.1") == _sl[7:9])
    _rc, _out = _mode(["--show", "P98.1", "--root", str(_r)])
    record("show: main prints the scope verbatim, then the status line and the commit line",
           _rc == 0 and _out.splitlines()[:10] == _sl[2:12]
           and "status: [ ] · unmet: P98.2 · roots: none" in _out and "commits naming P98.1: (no git history)" in _out)
    record("show: an unknown id exits 2", _mode(["--show", "P98.9", "--root", str(_r)])[0] == 2)


# --- --next parks: the parked set a real git dir holds, written by the build-loop.md §6 park procedure ---------
def _pgit(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, encoding="utf-8",
                          errors="replace", check=True).stdout.strip()


with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as _tp:
    _repo = Path(_tp)
    _pgit(_repo, "init", "-q", "-b", "main")
    for _k, _v in (("user.email", "t@t.t"), ("user.name", "t"), ("core.autocrlf", "false"),
                   ("commit.gpgsign", "false"), ("core.hooksPath", str(_repo / ".no-hooks"))):
        _pgit(_repo, "config", _k, _v)
    _PARK_PLAN = ("- [{m}] **P1.1** [DOC] The parked box · tooling-only\n"
                  "- [ ] **P1.2** [DOC] Needs the parked box · tooling-only\n  needs: P1.1\n"
                  "- [ ] **P1.3** [DOC] Outside the closure · tooling-only\n")
    _plan_root(_repo, _PARK_PLAN.format(m=" "), "P1-x.md")
    (_repo / "a.txt").write_bytes(b"base\n")
    _pgit(_repo, "add", "-A")
    _pgit(_repo, "commit", "-q", "-m", "docs: P1.1 first")
    for _subject in ("docs: P1.10 other", "docs: P1.1.2 sub", "docs: close P1.1."):
        _pgit(_repo, "commit", "-q", "--allow-empty", "-m", _subject)
    # the §6 park procedure, step (1): mkdir -p <git common dir>/parked, git add -N the new files, then the header
    # line and `git diff --binary HEAD -- <the box's paths>` into parked/<box-id>.patch
    _base = _pgit(_repo, "rev-parse", "HEAD")
    _pdir = _repo / _pgit(_repo, "rev-parse", "--git-common-dir") / "parked"
    _pdir.mkdir(parents=True, exist_ok=True)
    (_repo / "a.txt").write_bytes(b"box work\n")
    (_repo / "new.txt").write_bytes(b"a new file\n")
    _pgit(_repo, "add", "-N", "new.txt")
    _diff = subprocess.run(["git", "-C", str(_repo), "diff", "--binary", "HEAD", "--", "a.txt", "new.txt"],
                           capture_output=True, check=True).stdout
    _patch = _pdir / "P1.1.patch"

    def _park(reason: str, form: str = "# base {sha} reason {reason}") -> None:
        _patch.write_bytes(form.format(sha=_base, reason=reason).encode("utf-8") + b"\n" + _diff)

    _park("caged")
    _parks, _probs = m._read_parks(_repo)
    record("next: parks - a caged park the §6 recipe wrote is read back (box, reason, base, not released)",
           _probs == [] and [(p.box_id, p.reason, p.base, p.released) for p in _parks] == [("P1.1", "caged", _base, False)])
    record("next: parks - the header line leaves the patch valid for the §6 step (2) `git apply --check -R`",
           subprocess.run(["git", "-C", str(_repo), "apply", "--check", "-R", str(_patch)],
                          capture_output=True).returncode == 0)
    _rc, _out = _mode(["--next", "--root", str(_repo)])
    record("next: parks - a caged park and its needs: closure are skipped; the next box outside it is the target",
           _rc == 0 and _out.startswith("target P1.3 ") and "skipped: P1.1 [parked caged] blocks P1.1, P1.2" in _out)
    _park("ruling")
    _rc, _out = _mode(["--next", "--root", str(_repo)])
    record("next: parks - a ruling park stays a stop root until the Co-Pilot releases it",
           _rc == 0 and _out.startswith("target P1.3 ") and "skipped: P1.1 [parked ruling]" in _out)
    _released = _pdir / "P1.1.patch.released"
    _patch.rename(_released)
    _rc, _out = _mode(["--next", "--root", str(_repo)])
    record("next: parks - a released park is answered first, with its reason, patch path and build order",
           _rc == 0 and _out.splitlines()[:2] == [f"resume P1.1 (ruling, released) patch {_released}",
                                                  "build order: P1.1"])
    _released.rename(_patch)
    _park("stop")
    _rc, _out = _mode(["--next", "--root", str(_repo)])
    record("next: parks - a stop park is resumed with no release (the owner's next start)",
           _rc == 0 and _out.startswith("resume P1.1 (stop) patch "))
    _plan_root(_repo, "- [!extern] **P1.1** [DOC] The parked box · tooling-only\n  > now an owner act\n"
               + _PARK_PLAN.split("\n", 1)[1], "P1-x.md")
    _rc, _out = _mode(["--next", "--root", str(_repo)])
    record("next: parks - a stop park whose box a landed commit made [!extern] is skipped with its closure, never "
           "resumed",
           _rc == 0 and _out.startswith("target P1.3 ") and "skipped: P1.1 [!extern] blocks P1.1, P1.2" in _out)
    _plan_root(_repo, _PARK_PLAN.format(m=" "), "P1-x.md")
    _park("caged", "# parked-at {sha} reason {reason}")
    record("next: parks - a first line off the §6 form (`# parked-at ...`) exits 2, never a dropped park",
           _mode(["--next", "--root", str(_repo)])[0] == 2)
    _park("caged")
    (_pdir / "notes.txt").write_bytes(b"x\n")
    _stray_rc = _mode(["--next", "--root", str(_repo)])[0]
    (_pdir / "notes.txt").unlink()
    record("next: parks - an entry that is no <id>.patch[.released] exits 2", _stray_rc == 2)
    # a resumed box parked again beside its released file: the §6 step (1) deletes the released file first
    (_pdir / "P1.1.patch.released").write_bytes(_patch.read_bytes())
    _twice_rc, _twice = _mode(["--next", "--root", str(_repo)])[0], m._read_parks(_repo)[1]
    (_pdir / "P1.1.patch.released").unlink()
    record("next: parks - a box with two park files (a re-park beside its released file) exits 2 and names the §6 "
           "step (1) rule", _twice_rc == 2 and len(_twice) == 1 and "parked twice" in _twice[0]
           and "step (1)" in _twice[0])
    _plan_root(_repo, _PARK_PLAN.format(m="x"), "P1-x.md")
    _rc, _out = _mode(["--next", "--root", str(_repo)])
    record("next: parks - a park whose box is [x] is reported stale and never resumed",
           _rc == 0 and _out.startswith("target P1.2 ") and "stale park: P1.1 is [x]" in _out)
    record("show: commits naming an id match it whole - P1.1 names neither P1.10 nor P1.1.2",
           [ln.partition("\t")[2] for ln in m._commits_naming(_repo, "P1.1") or []]
           == ["docs: P1.1 first", "docs: close P1.1."])

# --- --report owner-acts: the phase's [!extern] acts by needs: closure, then the l-neg1: routes ----------------
_acts, _routes = m.owner_acts(ctx(_seq(
    box(bid="P3.1", **_EXT), _open("P4.1", needs=["P3.1"]), box(bid="P4.2", **_EXT), _open("P4.3", needs=["P4.2"]),
    _open("P4.4", l_neg1=["same-push"]), _open("P4.5", needs=["P4.2"], l_neg1=["act P4.2"]),
    _open("P4.6", l_neg1=["sweep-tail"]), box(bid="P4.7", l_neg1=["same-push"]), _open("P4.8", l_neg1=["none"]))), 4)
record("report: owner-acts lists the phase's [!extern] boxes with the open boxes each blocks",
       ("P4.2", ["P4.3", "P4.5"]) in _acts)
record("report: owner-acts lists an earlier-phase [!extern] root reached through needs:, in plan order",
       _acts[0] == ("P3.1", ["P4.1"]))
record("report: the l-neg1: routes group same-push, act <id>, sweep-tail in that order ([x] and none left out)",
       _routes == [("same-push", ["P4.4"]), ("act P4.2", ["P4.5"]), ("sweep-tail", ["P4.6"])])
record("report: --report without --phase, and --phase without --report, exit 2",
       _mode(["--report", "owner-acts"])[0] == 2 and _mode(["--phase", "4"])[0] == 2)

# --- DOC checks: each catches its violation (negative fixtures; not green-by-vacuity) ---------
def dctx(docs):
    return m.Ctx(root=ROOT, boxes=[], by_id={}, plan_files=[], docs=docs, gate_ids=set())


record("2 cross-ref: a dangling §99.99 -> caught",
       any("99.99" in f.msg for f in m.doc2_cross_reference(
           dctx({"docs/spec/s.md": "# x\n## 5 foo\n", "docs/security/y.md": "ok §5, bad §99.99\n"}))))
record("3 heading-hierarchy: H1->H3 skip -> caught",
       m.doc3_heading_hierarchy(dctx({"a.md": "# T\n\n### skip\n"})) != [])
record("4 numbering: a gap (0.1,0.3) -> caught",
       m.doc4_numbering_gap_free(dctx({"docs/spec/a.md": "# t\n## 0.1 a\n## 0.3 b\n"})) != [])
record("5 gate-catalogue: a gate named in security-concept absent from build-gates -> caught",
       m.doc5_gate_catalogue(dctx({"docs/security/security-concept.md": "uses G999 here\n",
                                   "docs/security/build-gates.md": "| **G2** | x |\n"})) != [])
record("6 forbidden-tokens: strikethrough -> caught",
       m.doc6_forbidden_tokens(dctx({"a.md": "this is ~~struck~~ text\n"})) != [])
record("8 threat-parity: a §5 table missing classes -> caught",
       m.doc8_threat_parity(dctx({"docs/security/security-concept.md": "| **T1** d | c | G48 |\n"})) != [])
record("9 inventory: a C99 IPC command -> caught",
       m.doc9_inventory_parity(dctx({"docs/spec/a.md": "the C99 command\n"})) != [])
record("9 inventory: C17 (past the ruled set) -> caught",
       m.doc9_inventory_parity(dctx({"docs/spec/a.md": "the C17 command\n"})) != [])
record("11 span-bound: a frozen G2-G50 < max -> caught",
       m.doc11_span_bound(dctx({"docs/security/build-gates.md": "| **G2** | a |\n| **G72** | b |\nthe G2-G50 boundary\n"})) != [])
record("11 span-bound: 'rather than G2-G50' counter-example -> NOT caught (negative cue)",
       m.doc11_span_bound(dctx({"docs/security/build-gates.md": "| **G2** | a |\n| **G72** | b |\nrather than a frozen G2-G50\n"})) == [])
record("17 forward-idea: a live [DEFER] citing a catalogue row -> caught",
       m.doc17_forward_idea_status(dctx({"docs/security/build-gates.md": "| **G2** | a |\nG2 idea [DEFER]\n"})) != [])
record("17 forward-idea: 'promoted from [DEFER]' history -> NOT caught (resolved cue)",
       m.doc17_forward_idea_status(dctx({"docs/security/build-gates.md": "| **G2** | a |\nG2 promoted from [DEFER]\n"})) == [])
record("22 gate-id-gap: G3/G4 missing + not vacated -> caught",
       m.doc22_gate_id_gap_free(dctx({"docs/security/build-gates.md": "| **G2** | a |\n| **G5** | b |\n"})) != [])
record("22 gate-id-gap: the gap documented vacated -> NOT caught",
       m.doc22_gate_id_gap_free(dctx({"docs/security/build-gates.md": "| **G2** | a |\n| **G5** | b |\nG3, G4 vacated/reserved\n"})) == [])
record("25 doc-graph: a dangling cross-doc link -> caught",
       any("dangling" in f.msg for f in m.doc25_doc_graph(dctx({"docs/a.md": "[bad](nope.md)\n"}))))
record("25 freshness: a doc naming a non-documented (deleted/renamed) gate -> caught",
       any("freshness" in f.msg and "G9999" in f.msg for f in m.doc25_doc_graph(
           dctx({"docs/x.md": "we still use G9999 here\n", "docs/security/build-gates.md": "| **G2** | x |\n"}))))
# [Test-Change: G7 target-absent flip — old-obsolete+new-correct, build-gates §6] the forward-target allowlist
# is gone (its three docs landed), so a dangling link whose basename it once excused is a finding like any other.
record("25 forward-allowlist: a dangling link whose basename was on the retired allowlist -> caught (no allowlist)",
       any("dangling" in f.msg and "nowhere/vuln-response.md" in f.msg for f in m.doc25_doc_graph(
           dctx({"docs/spec/README.md": "[v](../nowhere/vuln-response.md)\n"}))))
record("25 forward-allowlist: a dangling link to an UNREGISTERED target -> still caught (no cue evasion)",
       any("dangling" in f.msg for f in m.doc25_doc_graph(
           dctx({"docs/spec/README.md": "[x](gone.md) — but this is planned for later\n"}))))
record("25 code-span: a link inside an inline `code` span -> NOT flagged",
       not any("dangling" in f.msg for f in m.doc25_doc_graph(dctx({"docs/a.md": "example `[x](nope.md)` here\n"}))))

# --- check 13: §0.10 plugin-surface — granted allowlist + the forced-transitive-INERT fs handling -----
record("13 plugin-surface: a granted plugin (dialog) -> clean",
       m._plugin_surface_drift('name = "tauri-plugin-dialog"\n', "") == [])
record("13 plugin-surface: tauri-plugin-fs present + NOT .plugin()-registered -> clean (forced-inert dialog dep)",
       m._plugin_surface_drift('name = "tauri-plugin-dialog"\nname = "tauri-plugin-fs"\n',
                               "fn main() { Builder::new().plugin(tauri_plugin_dialog::init()); }") == [])
record("13 plugin-surface: tauri-plugin-fs present AND .plugin()-registered -> caught (must stay type-only)",
       any("registered" in p for p in m._plugin_surface_drift(
           'name = "tauri-plugin-fs"\n', ".plugin(tauri_plugin_fs::init())")))
record("13 plugin-surface: fs registered via the Builder form -> caught (init|Builder both flagged)",
       any("registered" in p for p in m._plugin_surface_drift(
           'name = "tauri-plugin-fs"\n', "tauri_plugin_fs::Builder::new().build()")))
record("13 plugin-surface: an UNLISTED plugin (http) -> caught (neither granted nor forced-inert)",
       any("http" in p and "neither" in p for p in m._plugin_surface_drift('name = "tauri-plugin-http"\n', "")))
record("13 plugin-surface: tauri-plugin-store (retired, §7.4.2) -> caught (off the allowlist)",
       any("tauri-plugin-store" in p and "neither" in p
           for p in m._plugin_surface_drift('name = "tauri-plugin-store"\n', "")))

# --- check 28: §0.4.2 closed app:// event surface — exactly {fault,intake,close-requested}, const-homed ---
_AE_MOD = "src-tauri/src/ipc/mod.rs"
record("28 app-event: the three events declared in the events module -> clean",
       m._app_event_surface_drift({_AE_MOD: 'pub const A: &str = "app://fault";\n'
                                            'pub const B: &str = "app://intake";\n'
                                            'pub const C: &str = "app://close-requested";\n'}) == [])
record("28 app-event: a FOURTH app:// event -> caught (no fourth)",
       any("no fourth" in p for p in m._app_event_surface_drift({_AE_MOD: 'const X: &str = "app://bogus";\n'})))
record("28 app-event: a raw VALID literal OUTSIDE the events module -> caught (bypasses the const SSOT)",
       any("bypasses" in p for p in m._app_event_surface_drift({"src-tauri/src/main.rs": 'app.emit("app://fault", x);\n'})))
record("28 app-event: a backtick doc-comment mention (not a literal) -> NOT caught (negative cue)",
       m._app_event_surface_drift({"src-tauri/src/x.rs": "/// the `app://fault` event is emitted here\n"}) == [])
record("28 app-event: an assert message merely CONTAINING app:// mid-string -> NOT caught (negative cue)",
       m._app_event_surface_drift({"src-tauri/src/x.rs": '    msg = "§0.4.2: AppFault is the app://fault payload";\n'}) == [])
record("28 app-event: a dynamic format!(\"app://{}\") name -> caught (value not in the closed set)",
       any("no fourth" in p for p in m._app_event_surface_drift({"src-tauri/src/x.rs": 'format!("app://{}", k)\n'})))
record("28 app-event: the REAL committed src-tauri/src passes (the 6 literals are all const-homed + valid)",
       m.doc28_app_event_surface_drift(m.build_ctx(ROOT)) == [])

# --- check 29: stale liveness/futurity claims in .rs comments (the P4.23 L(-1) hand-off) -------
_done29 = {"P3.48", "P4.4"}
record("29 stale-liveness: dead-until bounded by a LANDED box -> caught (claim JOINED across two lines)",
       len(m._stale_liveness_problems(
           {"src-tauri/src/x/mod.rs":
            "// the whole hull\n// stays dead until P3.48 wires the\n// conductor\nfn f() {}\n"},
           _done29)) == 1)
record("29 stale-liveness: the same construction bounded by an OPEN box -> clean (the claim is still true)",
       m._stale_liveness_problems(
           {"src-tauri/src/x/mod.rs": "// stays dead until P9.99 wires it\n"}, _done29) == [])
record("29 stale-liveness: a lint reason = string is scanned too (the other rot carrier P4.23 found)",
       len(m._stale_liveness_problems(
           {"src-tauri/src/x/mod.rs":
            '#[cfg_attr(not(test), expect(dead_code, reason = "dead in the production build '
            'until P4.4 lands"))]\n'},
           _done29)) == 1)
record("29 stale-liveness: PAST-tense narration (stayed dead until X made it live) -> history, clean",
       m._stale_liveness_problems(
           {"src-tauri/src/x.rs":
            "// this graph stayed dead until the P3.48 conductor made it reachable\n"},
           _done29) == [])
record("29 stale-liveness: an id in a trailing parenthetical is NOT the bound (window stops at '(')",
       m._stale_liveness_problems(
           {"src-tauri/src/x.rs":
            "// has no production caller until P9.99 (`needs: P4.4`) wires it\n"},
           _done29) == [])
record("29 stale-liveness: a double-quoted MENTION of an old claim is stripped, not asserted",
       m._stale_liveness_problems(
           {"src-tauri/src/x.rs":
            '// the clause read "stays dead until P3.48 wires it" and was reworded\n'},
           _done29) == [])
record("29 stale-liveness: a bracket-tag span is attribution, not assertion (stripped before matching)",
       m._stale_liveness_problems(
           {"src-tauri/src/x.rs":
            "// [Corrected by P4.23: previously dead until P4.4 wires it] now live\n"},
           _done29) == [])
record("29 stale-liveness: 'once P<id> lands' with the id landed -> caught",
       len(m._stale_liveness_problems(
           {"src-tauri/src/x.rs": "// registered transitively once P4.4 lands\n"}, _done29)) == 1)
record("29 stale-liveness: 'once P<id> landed' (a PAST-tense post-id verb) -> history, clean",
       m._stale_liveness_problems(
           {"src-tauri/src/x.rs": "// the guard was added once P4.4 landed, closing the class\n"},
           _done29) == [])
record("29 stale-liveness: 'once P<id> shipped' (the R2 review's probe verb) -> history, clean",
       m._stale_liveness_problems(
           {"src-tauri/src/x.rs": "// the flag flips once P4.4 shipped the spawn\n"},
           _done29) == [])
record("29 stale-liveness: the REAL tree is clean today (the 15 calibration sites were fixed in the arming commit)",
       m.doc29_stale_liveness(m.build_ctx(ROOT)) == [])
# the real-entry planted positive (the g24-hermetic rule: the clean real-tree leg above would stay
# green if file DISCOVERY or the [x] parse broke, so the FIRING path is proven end-to-end through
# doc29_stale_liveness + build_ctx over a scratch root - never only through the pure helper)
with tempfile.TemporaryDirectory() as _d29:
    _root29 = Path(_d29)
    (_root29 / "docs" / "plan").mkdir(parents=True)
    (_root29 / "docs" / "plan" / "P0-x.md").write_text(
        "- [x] **P0.1** [GATE] Done thing · G7\n", encoding="utf-8")
    (_root29 / "src-tauri" / "src").mkdir(parents=True)
    (_root29 / "src-tauri" / "src" / "planted.rs").write_text(
        "// the hull stays dead until P0.1 wires it\n", encoding="utf-8")
    # [Test-Change: G7 target-absent flip — old-obsolete+new-correct, build-gates §6] the other two scope
    # roots exist too: a missing one is now its own finding (the `missing target` legs), not a silent skip.
    (_root29 / "crates").mkdir()
    (_root29 / "xtask" / "src").mkdir(parents=True)
    record("29 stale-liveness: the REAL entry point fires on a planted scratch root (discovery + [x] parse proven)",
           len(m.doc29_stale_liveness(m.build_ctx(_root29))) == 1)

# --- check 23: the owner-decidable / informational-then-ratcheted gate-status ledger (P0.4.5) --
_GS = "docs/process/gate-status.md"
_LEDGER_HEAD = "| Gate / tool | Status | Since | Activation | Contract |\n|---|---|---|---|---|\n"


def _ledger(rows):                                   # rows: list of (name, status, since)
    body = "".join(f"| {n} | {s} | {d} | P1 | x |\n" for (n, s, d) in rows)
    return {_GS: "# Ledger doc\n\n## Ledger\n\n" + _LEDGER_HEAD + body}


# The declined tools and G65 carry `decided` (the gate-status ledger).
_SEEDED = [("`cargo-acl`/cackle", "decided", "2026-06-18"),
           ("`cargo-careful`", "decided", "2026-06-18"),
           ("Kani", "decided", "2026-06-18"),
           ("`cargo-geiger`", "decided", "2026-06-18"),
           ("`cargo-mutants`", "informational", "2026-06-19"),   # P0.5.10 — the G15 mutation sub-leg
           ("`G17b`", "informational", "2026-06-19"),            # P0.7.7 — bundled-engine CVE awareness
           ("`G64`", "informational", "2026-06-19"),             # P0.7.14 — privilege-drop-tier ratchet
           ("`G65`", "decided", "2026-06-19")]                   # P0.7.15 — engine-subprocess coverage-guided fuzz
record("23 gate-status: a clean 8-row ledger (all registered gates) -> no finding",
       m.doc23_ratchet_log(dctx(_ledger(_SEEDED))) == [])
# [Test-Change: G7 target-absent flip — old-obsolete+new-correct, build-gates §6] the ledger landed (P0.4.5),
# so its absence is a finding, never a skip.
record("23 gate-status: absent ledger -> caught (a missing target is a finding)",
       [f.msg for f in m.doc23_ratchet_log(dctx({}))]
       == ["docs/process/gate-status.md is missing or empty — fail-closed"])
record("23 gate-status: a missing required gate (no Kani row) -> caught",
       any("kani" in f.msg.lower() for f in m.doc23_ratchet_log(dctx(_ledger([r for r in _SEEDED if r[0] != "Kani"])))))
record("23 gate-status: a malformed status ('maybe') -> caught",
       any("not one of" in f.msg for f in m.doc23_ratchet_log(
           dctx(_ledger([("`cargo-acl`/cackle", "maybe", "2026-06-18")] + _SEEDED[1:])))))
record("23 gate-status: a non-ISO 'Since' date -> caught",
       any("ISO" in f.msg for f in m.doc23_ratchet_log(
           dctx(_ledger([("`cargo-acl`/cackle", "informational", "June 18")] + _SEEDED[1:])))))
record("23 gate-status: a status disagreeing with the effective posture -> caught",
       any("disagrees" in f.msg for f in m.doc23_ratchet_log(
           dctx(_ledger([("`cargo-acl`/cackle", "required", "2026-06-18")] + _SEEDED[1:])))))
record("23 gate-status: a table without a Status+Since header is ignored -> required rows still missing",
       any("no dated status row" in f.msg for f in m.doc23_ratchet_log(
           dctx({_GS: "# L\n\n| a | b |\n|---|---|\n| x | y |\n"}))))
record("23 gate-status: a near-miss name (cargo-aclx) does NOT false-bind the cargo-acl key -> still missing",
       any("'cargo-acl'" in f.msg and "no dated status row" in f.msg for f in m.doc23_ratchet_log(
           dctx(_ledger([("`cargo-aclx`", "informational", "2026-06-18")] + _SEEDED[1:])))))
record("23 gate-status: an impossible-but-ISO-shaped 'Since' date (2026-13-99) -> caught",
       any("valid ISO" in f.msg for f in m.doc23_ratchet_log(
           dctx(_ledger([("`cargo-acl`/cackle", "informational", "2026-13-99")] + _SEEDED[1:])))))
# P0.5.10: the cargo-mutants registration is enforced both ways — a missing row is caught, and a
# posture flip away from `informational` without a matching _OWNER_DECIDABLE_GATES edit is caught.
# [Test-Change: P0.7.7 — old-obsolete+new-correct, gate-status.md ledger] both legs filter cargo-mutants
# out BY NAME (mirroring the Kani leg) instead of the old positional `_SEEDED[:-1]`: P0.7.7 appended the
# G17b row to _SEEDED, so the last element is no longer cargo-mutants — the positional slice would drop
# G17b and leave cargo-mutants present. The name-filter is robust to any future _SEEDED growth (G64/G65).
_SEEDED_NO_MUTANTS = [r for r in _SEEDED if "cargo-mutants" not in r[0]]
record("23 gate-status: a missing cargo-mutants row (P0.5.10) -> caught",
       any("cargo-mutants" in f.msg.lower() and "no dated status row" in f.msg
           for f in m.doc23_ratchet_log(dctx(_ledger(_SEEDED_NO_MUTANTS)))))
record("23 gate-status: a cargo-mutants row flipped to 'required' (no registry edit) -> disagrees caught",
       any("cargo-mutants" in f.msg.lower() and "disagrees" in f.msg for f in m.doc23_ratchet_log(
           dctx(_ledger(_SEEDED_NO_MUTANTS + [("`cargo-mutants`", "required", "2026-06-19")])))))

# --- check 19: the G1 reviewer-rubric fenced block in build-loop.md (P0.6.2) -------------------
_BL = "docs/process/build-loop.md"
_RUBRIC_OK = ("# Build-Loop\n\n```text\n=== ConvertIA dual-review rubric (canonical) ===\n"
              "Input: the STAGED diff (git diff --cached, inline).\n"
              "1. COMPLETENESS  2. CORRECTNESS  3. SPEC-CONFORMANCE  4. SECURITY (does it open a network surface?)  5. TEST-INTEGRITY\n"
              "is this SUPPRESSING A REAL REGRESSION? State convergence/divergence explicitly.\n"
              "6. CLASS-CLOSURE - class closed (sweep + permanent catcher) or the one-off recorded.\n"
              "SPEC-CONTRADICTION is a finding CLASS ABOVE P0.\n```\n")
record("_extract_fenced_block: returns the marked block's body",
       (m._extract_fenced_block("a\n```text\nMARK here\nbody line\n```\nb\n", "MARK here") or "").strip().endswith("body line"))
record("_extract_fenced_block: None when no fenced block carries the marker",
       m._extract_fenced_block("```\nunrelated\n```\n", "MARK here") is None)
record("19 reviewer-rubric: a complete fenced rubric block -> no finding",
       m.doc19_reviewer_rubric(dctx({_BL: _RUBRIC_OK})) == [])
# [Test-Change: G7 target-absent flip — old-obsolete+new-correct, build-gates §6] build-loop.md landed (P0.1.3),
# so its absence is a finding, never a skip (the same flip for checks 14/15/18/20 below).
_BL_MISSING = ["docs/process/build-loop.md is missing or empty — fail-closed"]
record("19 reviewer-rubric: absent build-loop.md -> caught (a missing target is a finding)",
       [f.msg for f in m.doc19_reviewer_rubric(dctx({}))] == _BL_MISSING)
record("19 reviewer-rubric: no fenced rubric block at all -> caught (absent/empty)",
       any("absent or empty" in f.msg for f in m.doc19_reviewer_rubric(dctx({_BL: "# bl\n\nprose, no rubric\n"}))))
record("19 reviewer-rubric: a rubric MISSING the SPEC-CONTRADICTION-above-P0 phrase -> caught",
       any("SPEC-CONTRADICTION is a finding CLASS ABOVE P0" in f.msg for f in m.doc19_reviewer_rubric(
           dctx({_BL: _RUBRIC_OK.replace("SPEC-CONTRADICTION is a finding CLASS ABOVE P0.", "")}))))
record("19 reviewer-rubric: a rubric MISSING the test-integrity item -> caught",
       any("SUPPRESSING A REAL REGRESSION" in f.msg for f in m.doc19_reviewer_rubric(
           dctx({_BL: _RUBRIC_OK.replace("is this SUPPRESSING A REAL REGRESSION?", "")}))))
# G1 P0.6.2 P3 hardening: the SECURITY dimension is pinned by its substance ("open a network surface"),
# not the bare word "SECURITY" (which collides with "security-critical" in-block) — so renaming the
# dimension away is now CAUGHT, the one phrase-drop the original legs did not exercise.
record("19 reviewer-rubric: a rubric MISSING the SECURITY dimension (open-a-network-surface) -> caught",
       any("open a network surface" in f.msg for f in m.doc19_reviewer_rubric(
           dctx({_BL: _RUBRIC_OK.replace("open a network surface", "")}))))
record("19 reviewer-rubric: a rubric MISSING the CLASS-CLOSURE dimension -> caught",
       any("CLASS-CLOSURE" in f.msg for f in m.doc19_reviewer_rubric(
           dctx({_BL: _RUBRIC_OK.replace("6. CLASS-CLOSURE", "6. ")}))))
record("19 reviewer-rubric: a rubric MISSING the class-closure SUBSTANCE (permanent catcher) -> caught",
       any("permanent catcher" in f.msg for f in m.doc19_reviewer_rubric(
           dctx({_BL: _RUBRIC_OK.replace("permanent catcher", "catcher")}))))
record("19 reviewer-rubric: the REAL committed build-loop.md rubric passes (no finding)",
       m.doc19_reviewer_rubric(m.build_ctx(ROOT)) == [])
# Pin-uniqueness (the 3x-recurred collision class, mechanized 2026-08-26): a substring pin is ARMED
# only while its phrase occurs EXACTLY ONCE in the block - a second (prose) twin keeps check 19 green
# while the guarded element is deleted/renamed (P0.6.2 bare-"SECURITY"; the 2026-08-26 G1 R2 caught
# "staged diff" + "class-closure" twins). This leg turns any future twin into a red canary.
def _real_rubric_block() -> str:
    bl_text = (ROOT / "docs" / "process" / "build-loop.md").read_text(encoding="utf-8")
    return m._extract_fenced_block(bl_text, m._RUBRIC_MARKER) or ""
record("19 pin-uniqueness: every _RUBRIC_PHRASES entry occurs EXACTLY ONCE in the real rubric block",
       (lambda blk: bool(blk) and all(blk.lower().count(ph.lower()) == 1 for ph in m._RUBRIC_PHRASES))(
           _real_rubric_block()))
record("19 pin-uniqueness: the leg ARMS - a planted prose twin of a pinned phrase is caught",
       (lambda blk: blk.lower().count("permanent catcher") == 2)(
           _real_rubric_block() + "a permanent catcher mentioned twice\n"))

# --- check 20: the reviewer-family owner decision + spot-audit cadence (P0.6.3) ----------------
_FAM_OK = ("# Build-Loop\n\nRecorded reviewer-family decision: the two reviewers share model lineage, so\n"
           "the correlated-lineage residual is explicitly ACCEPTED for v1 (the deterministic gates carry\n"
           "the real security weight), WITH a Co-Pilot spot-audit at every phase boundary AND a random\n"
           "1-in-10-box sample; the flip option remains open.\n")
record("20 reviewer-family: a build-loop.md with the full decision + cadence -> no finding",
       m.doc20_reviewer_family(dctx({_BL: _FAM_OK})) == [])
# [Test-Change: G7 target-absent flip — old-obsolete+new-correct, build-gates §6] see check 19 above.
record("20 reviewer-family: absent build-loop.md -> caught (a missing target is a finding)",
       [f.msg for f in m.doc20_reviewer_family(dctx({}))] == _BL_MISSING)
record("20 reviewer-family: a build-loop.md MISSING the spot-audit cadence -> caught",
       any("spot-audit" in f.msg for f in m.doc20_reviewer_family(dctx({_BL: _FAM_OK.replace("spot-audit", "review")}))))
record("20 reviewer-family: a build-loop.md MISSING the explicit acceptance -> caught",
       any("ACCEPTED for v1" in f.msg for f in m.doc20_reviewer_family(dctx({_BL: _FAM_OK.replace("ACCEPTED for v1", "left open")}))))
record("20 reviewer-family: a build-loop.md MISSING the flip-option phrase -> caught",
       any("flip option remains open" in f.msg for f in m.doc20_reviewer_family(
           dctx({_BL: _FAM_OK.replace("the flip option remains open", "")}))))
# G1 P0.6.3 P1: the cadence has TWO prongs (phase-boundary spot-audit + 1-in-10-box sample); dropping the
# phase-boundary prong must be caught — the phrase is pinned to "at every phase boundary" (unique-in-doc),
# not bare "phase boundary" (which collides with L571/L657 and let this prong be silently halved).
record("20 reviewer-family: a build-loop.md MISSING the phase-boundary cadence prong -> caught",
       any("at every phase boundary" in f.msg for f in m.doc20_reviewer_family(
           dctx({_BL: _FAM_OK.replace("at every phase boundary", "per box")}))))
record("20 reviewer-family: the REAL committed build-loop.md decision passes (no finding)",
       m.doc20_reviewer_family(m.build_ctx(ROOT)) == [])

# --- check 14: the 8-point DoD (a)-(h) tri-copy parity (P0.6.5) --------------------------------
# Three synthetic copies, each carrying the ordered (a)-(h) run in its OWN shape: build-loop.md §5
# (with the real doc's leading blockquote "item (c)'s" + trailing "Items (g) and (h)" noise — proving
# the greedy parser ignores non-a-first letter refs), the build-gates G1 `- **Definition-of-Done.**`
# bullet (preceded by an UNRELATED `- **G29 …**` (a)/(b) list AND followed by a `- **Skipped only**`
# (a)/(b) bullet — proving the region-slice reads ONLY the DoD bullet), and the P0.6.5 box notes.
_BL5_OK = ("# Build-Loop\n\n## 5. Definition of Done (canonical)\n\n"
           "> check 14 holds the copies identical; output-validity lives inside item (c)'s bar.\n\n"
           "A change is done only when:\n"
           "- **(a)** spec ref\n- **(b)** spec synced\n- **(c)** tests green\n- **(d)** hard gates green\n"
           "- **(e)** dual review\n- **(f)** decision tags\n- **(g)** engines.lock + SBOM\n"
           "- **(h)** threat row. (Items (g) and (h) fire independently.)\n\n## 6. Next\n")
_BG_OK = ("# Build gates\n\n## 1. L0\n\n"
          "- **G29 plugin surface:** **(a)** the lockfile set; **(b)** every capability entry.\n"
          "- **Definition-of-Done.** A box is done only when: (a) spec ref; (b) spec synced; "
          "(c) tests green; (d) hard gates green; (e) dual review; (f) decision tags; "
          "(g) engines.lock + SBOM row; (h) threat-class row.\n"
          "- **Skipped only** for: (a) check-off commits; (b) `[!extern]` boxes.\n")
_BOX_DOD = ("Conventional commit + the canonical 8-point DoD lives here: (a) spec ref; (b) spec synced; "
            "(c) tests green; (d) hard gates green; (e) dual review; (f) decision tags; "
            "(g) engines.lock+SBOM; (h) threat row. The 8-vs-9 derivation is recorded.")


def _c14(bl=_BL5_OK, bg=_BG_OK, box_notes=_BOX_DOD, delivered=None, with_bl=True, with_box=True):
    docs = {"docs/security/build-gates.md": bg}
    if with_bl:
        docs[_BL] = bl
    notes = [box_notes] + ([delivered] if delivered else [])
    boxes = [box(bid="P0.6.5", notes=notes)] if with_box else []
    return m.Ctx(root=ROOT, boxes=boxes, by_id={b.box_id: b for b in boxes}, plan_files=[],
                 docs=docs, gate_ids=set())


# greedy-parser unit: noise-tolerant, ORDERED extraction (not a set)
record("14 greedy: leading '(c)'s' + trailing '(g) and (h)' noise -> exactly a..h",
       m._greedy_letters("item (c)'s bar; (a)(b)(c)(d)(e)(f)(g)(h); Items (g) and (h)") == list("abcdefgh"))
record("14 greedy: a reorder (a)(c)(b)... -> NOT a..h (in-order, not the same SET)",
       m._greedy_letters("(a)(c)(b)(d)(e)(f)(g)(h)") != list("abcdefgh"))
record("14 greedy: an appended (i) -> 9 items (count drift visible)",
       m._greedy_letters("(a)(b)(c)(d)(e)(f)(g)(h)(i)") == list("abcdefghi"))
# region-slice unit: the G1 DoD bullet is read in ISOLATION from the neighbouring (a)/(b) lists
record("14 region: the G1 DoD bullet reads a..h, NOT the G29 / Skipped-only (a)/(b)",
       m._greedy_letters(m._gates_g1_dod_region(_BG_OK) or "") == list("abcdefgh"))
record("14 region: absent DoD bullet -> None",
       m._gates_g1_dod_region("# gates\n\n- **G1 row** only, no DoD bullet\n") is None)
# G1 P0.6.5 P3#2: the end-anchor is indentation-symmetric with the lstrip()-based start, so an INDENTED
# DoD bullet slices to its OWN next sibling — a following indented bullet (here carrying a stray (i)) is
# excluded, not bled in (which under the old col-0-only end anchor would have yielded a..i).
record("14 region: an INDENTED DoD bullet ends at its own sibling (a following indented (i)-bullet excluded)",
       m._greedy_letters(m._gates_g1_dod_region(
           "  - **Definition-of-Done.** (a) x;(b) x;(c) x;(d) x;(e) x;(f) x;(g) x;(h) x.\n"
           "  - **Note:** a ninth item (i) does not belong here.\n") or "") == list("abcdefgh"))
# integration: aligned -> clean; each source drifting -> caught; a missing build-loop.md -> caught; real docs pass
record("14 dod-parity: three aligned copies -> no finding", m.doc14_dod_parity(_c14()) == [])
# [Test-Change: G7 target-absent flip — old-obsolete+new-correct, build-gates §6] see check 19 above.
record("14 dod-parity: absent build-loop.md -> caught (a missing target is a finding)",
       [f.msg for f in m.doc14_dod_parity(_c14(with_bl=False))] == _BL_MISSING)
record("14 dod-parity: build-loop.md §5 dropping item (f) -> caught",
       any("build-loop.md" in f.file and "!= canonical" in f.msg
           for f in m.doc14_dod_parity(_c14(bl=_BL5_OK.replace("**(f)**", "**(x)**")))))
record("14 dod-parity: build-gates G1 region reordering (d)/(e) -> caught",
       any("build-gates" in f.file for f in m.doc14_dod_parity(
           _c14(bg=_BG_OK.replace("(d) hard gates green", "(e) hard gates green").replace("(e) dual review", "(d) dual review")))))
record("14 dod-parity: the P0.6.5 box dropping item (f) -> caught",
       any(f.check == "14:dod-parity" and "P0.6.5" in f.msg
           for f in m.doc14_dod_parity(_c14(box_notes=_BOX_DOD.replace("(f) decision tags; ", "")))))
# G1 P0.6.5 P3#1: the box leg reads ONLY the spec description (up to the first "**Delivered" note). A
# Delivered note carrying a full CONTIGUOUS a..h prose run must NOT re-acquire a letter dropped from the
# real list (the greedy false-pass) — and its own (a)-(h)/(c) refs must NOT trip a false count-drift.
record("14 dod-parity: a dropped (f) in the box list is NOT masked by a full a..h run in a Delivered note",
       any(f.check == "14:dod-parity" and "P0.6.5" in f.msg for f in m.doc14_dod_parity(
           _c14(box_notes=_BOX_DOD.replace("(f) decision tags; ", ""),
                delivered="**Delivered (P0.6.5):** the DoD is (a)(b)(c)(d)(e)(f)(g)(h), all present."))))
record("14 dod-parity: a Delivered note's own (a)-(h)/(c) refs are ignored (description-only) -> clean",
       m.doc14_dod_parity(_c14(delivered="**Delivered (P0.6.5):** see (a)-(h), item (c), Items (g) and (h).")) == [])
record("14 dod-parity: the P0.6.5 box absent (renumbered) -> caught",
       any("re-point check 14" in f.msg for f in m.doc14_dod_parity(_c14(with_box=False))))
record("14 dod-parity: the G1 `Definition-of-Done.` bullet absent -> caught",
       any("prose bullet is absent" in f.msg for f in m.doc14_dod_parity(
           _c14(bg="# gates\n\n## 1. L0\n\n- **G1 row** only\n"))))
record("14 dod-parity: the REAL committed docs (build-loop.md §5 / G1 bullet / P0.6.5 box) pass",
       m.doc14_dod_parity(m.build_ctx(ROOT)) == [])

# --- check 15: the operator-anchored cadence strings in build-loop.md (§7 iteration unit, §6 stop) -----
# Each canonical string is an EXACT integer + EXPLICIT operator; rewording one to a fuzzy prose form ("one
# box per iteration", a "~5 boxes" batch, "3 consecutive gate-red pushes") drops the verbatim match and is
# CAUGHT — so the loop cannot silently run on a cadence it could misread.
# [Test-Change: G7 cadence re-pin — old-obsolete+new-correct, build-loop §6/§7] the session counters
# (soft-stop >= 8, hard-stop == 12, cluster >= 5) are retired by the owner's unattended-loop decision; the
# legs pin the two live strings instead, and the absent-target / push-failure / REAL legs are unchanged.
_HS_OK = ("# Build-Loop\n\n## 6. Hard-stops\n\n"
          "- a hard-stop class: >= 3 consecutive push failures.\n\n"
          "## 7. Vocabulary\n\n"
          "Cadence: == 1 box per iteration.\n")
record("15 hard-stop: a build-loop.md with both operator-anchored cadence strings -> no finding",
       m.doc15_hard_stop_parity(dctx({_BL: _HS_OK})) == [])
# [Test-Change: G7 target-absent flip — old-obsolete+new-correct, build-gates §6] see check 19 above.
record("15 hard-stop: absent build-loop.md -> caught (a missing target is a finding)",
       [f.msg for f in m.doc15_hard_stop_parity(dctx({}))] == _BL_MISSING)
record("15 hard-stop: the == 1 box string reverted to the 'one box per iteration' prose form -> caught",
       any("== 1 box per iteration" in f.msg for f in m.doc15_hard_stop_parity(
           dctx({_BL: _HS_OK.replace("== 1 box per iteration", "one box per iteration")}))))
record("15 hard-stop: the == 1 box string widened to a '~5 boxes' batch -> caught",
       any("== 1 box per iteration" in f.msg for f in m.doc15_hard_stop_parity(
           dctx({_BL: _HS_OK.replace("== 1 box per iteration", "a batch of ~5 boxes per iteration")}))))
record("15 hard-stop: the >= 3 push-failures string reverted to 'gate-red pushes' -> caught",
       any(">= 3 consecutive push failures" in f.msg for f in m.doc15_hard_stop_parity(
           dctx({_BL: _HS_OK.replace(">= 3 consecutive push failures", "3 consecutive gate-red pushes")}))))
record("15 hard-stop: the REAL committed build-loop.md cadence strings pass (no finding)",
       m.doc15_hard_stop_parity(m.build_ctx(ROOT)) == [])
# Pin-uniqueness (the check-19 pattern): a substring pin is ARMED only while its string occurs EXACTLY ONCE
# in build-loop.md - a second copy would keep check 15 green after the canonical line is reworded.
def _hs_pins_unique(bl: str) -> bool:
    return bool(bl) and all(bl.lower().count(ph.lower()) == 1 for ph in m._HARDSTOP_PHRASES)
_REAL_BL = (ROOT / "docs" / "process" / "build-loop.md").read_text(encoding="utf-8")
record("15 pin-uniqueness: every _HARDSTOP_PHRASES entry occurs EXACTLY ONCE in the real build-loop.md",
       _hs_pins_unique(_REAL_BL))
record("15 pin-uniqueness: the leg ARMS - a planted second copy of each pinned string is caught",
       all(not _hs_pins_unique(_REAL_BL + f"a stray {ph} twin\n") for ph in m._HARDSTOP_PHRASES))

# --- check 18: the two named build-loop procedures present verbatim in build-loop.md (P0.6.8) ----------
# Each procedure (crash-recovery §9, divergence-resolution §3 Step 5) is pinned by its header + its
# load-bearing sub-rules; dropping ANY canonical phrase (gutting a procedure to a bare header, or removing
# its core rule) is CAUGHT — so "currently absent" cannot silently survive into the docs (build-gates §6
# check 18). The gate quarantine is authored in §6 but lies outside check 18's named set.
_NP_OK = ("# Build-Loop\n\n## 3. The loop\n\n"
          "Divergence-resolution rule (canonical): a P0/P1 GO-vs-NOGO is NOGO — the stricter reviewer wins.\n\n"
          "## 9. Crash-recovery\n\n"
          "Crash-recovery procedure (a mid-box crash is recoverable):\n"
          "- Committed-but-CI-red -> a NEW commit fixing it; never amend a pushed commit.\n"
          "- Push is idempotent on retry — a re-push is a safe no-op.\n")
record("18 named-proc: a build-loop.md with both procedures + their sub-rules -> no finding",
       m.doc18_named_procedure(dctx({_BL: _NP_OK})) == [])
# [Test-Change: G7 target-absent flip — old-obsolete+new-correct, build-gates §6] see check 19 above.
record("18 named-proc: absent build-loop.md -> caught (a missing target is a finding)",
       [f.msg for f in m.doc18_named_procedure(dctx({}))] == _BL_MISSING)
record("18 named-proc: the crash-recovery procedure header dropped -> caught",
       any("Crash-recovery procedure" in f.msg for f in m.doc18_named_procedure(
           dctx({_BL: _NP_OK.replace("Crash-recovery procedure", "Recovery steps")}))))
record("18 named-proc: the crash-recovery case (b) Committed-but-CI-red dropped -> caught",
       any("Committed-but-CI-red" in f.msg for f in m.doc18_named_procedure(
           dctx({_BL: _NP_OK.replace("Committed-but-CI-red", "Committed but red")}))))
record("18 named-proc: the crash-recovery case (d) push-idempotent dropped -> caught",
       any("Push is idempotent on retry" in f.msg for f in m.doc18_named_procedure(
           dctx({_BL: _NP_OK.replace("Push is idempotent on retry", "Re-push is safe")}))))
record("18 named-proc: the divergence-resolution rule header dropped -> caught",
       any("Divergence-resolution rule" in f.msg for f in m.doc18_named_procedure(
           dctx({_BL: _NP_OK.replace("Divergence-resolution rule", "Divergence handling")}))))
record("18 named-proc: the divergence 'stricter reviewer wins' core dropped -> caught",
       any("the stricter reviewer wins" in f.msg for f in m.doc18_named_procedure(
           dctx({_BL: _NP_OK.replace("the stricter reviewer wins", "the stricter one wins")}))))
record("18 named-proc: the REAL committed build-loop.md procedures pass (no finding)",
       m.doc18_named_procedure(m.build_ctx(ROOT)) == [])
# Pin-uniqueness for checks 18 and 20 (the check-15/19 pattern): a pin is ARMED only while no second copy
# of its phrase sits outside the element it guards - the file header naming these canonical homes carried
# case-insensitive twins of three pins, so deleting the guarded element kept the check green.
def _np_pins_unique(bl: str) -> bool:
    return bool(bl) and all(bl.lower().count(ph.lower()) == 1 for ph in m._NAMED_PROCEDURE_PHRASES)
record("18 pin-uniqueness: every _NAMED_PROCEDURE_PHRASES entry occurs EXACTLY ONCE in the real build-loop.md",
       _np_pins_unique(_REAL_BL))
record("18 pin-uniqueness: the leg ARMS - a planted second copy of each pinned phrase is caught",
       all(not _np_pins_unique(_REAL_BL + f"a stray {ph} twin\n") for ph in m._NAMED_PROCEDURE_PHRASES))
# Check 20's decision block holds "spot-audit" twice (plan-lint's check-20 comment), so its rule is the
# out-of-block one: every pinned phrase occurs inside the recorded-decision blockquote and nowhere else.
def _fam_block(bl: str) -> str:
    lines = m._lf(bl).split("\n")
    start = next((i for i, ln in enumerate(lines) if ln.lstrip().startswith(">")
                  and "recorded reviewer-family decision" in ln.lower()), None)
    if start is None:
        return ""
    end = start
    while end < len(lines) and lines[end].lstrip().startswith(">"):
        end += 1
    return "\n".join(lines[start:end])
def _fam_pins_in_block(bl: str) -> bool:
    blk = _fam_block(bl).lower()
    return bool(blk) and all(
        blk.count(ph.lower()) >= 1 and m._lf(bl).lower().count(ph.lower()) == blk.count(ph.lower())
        for ph in m._REVIEWER_FAMILY_PHRASES)
record("20 pin-uniqueness: every _REVIEWER_FAMILY_PHRASES entry occurs only inside the real decision block",
       _fam_pins_in_block(_REAL_BL))
record("20 pin-uniqueness: the leg ARMS - a planted out-of-block copy of each pinned phrase is caught",
       all(not _fam_pins_in_block(_REAL_BL + f"\na stray {ph} twin\n") for ph in m._REVIEWER_FAMILY_PHRASES))

# --- check 25 leg (c2): the per-source content-fingerprint freshness ledger (P0.3.12) ---------
# Each leg drives the PURE m._freshness_fingerprints(entries, root, docs) so a synthetic source can be
# supplied via the `docs` dict (no temp files): an entry whose `file` is in `docs` reads that content.
def _fp(text):
    return "sha256:" + hashlib.sha256(m._lf(text).encode()).hexdigest()


def _ff(entries, docs):
    return m._freshness_fingerprints(entries, ROOT, docs)


_DOC = "scripts/x.toml"
_CONTENT = 'patterns = [\n  "a/*",\n]\n'
record("25-fp: a matching file fingerprint is clean",
       _ff([{"id": "x", "kind": "file", "file": _DOC, "fingerprint": _fp(_CONTENT)}], {_DOC: _CONTENT}) == [])
record("25-fp: a stale file fingerprint is caught (same-name content drift)",
       any("freshness-fingerprint" in f.msg and "'x'" in f.msg
           for f in _ff([{"id": "x", "kind": "file", "file": _DOC, "fingerprint": "sha256:" + "0" * 64}],
                        {_DOC: _CONTENT})))

# kind="section": the fingerprint is scoped to the §0.7 region, so a §0.8 change must NOT trip it
_ARCH = "## 0.7 Tree\n\nsrc/ here\n\n## 0.8 Pins\n\npnpm 10.13.1\n"
_SE = [{"id": "s07", "kind": "section", "file": "docs/spec/a.md", "anchor": "0.7",
        "fingerprint": "sha256:" + hashlib.sha256(m._extract_section(_ARCH, "0.7").encode()).hexdigest()}]
record("25-fp: a section fingerprint matching its region is clean",
       _ff(_SE, {"docs/spec/a.md": _ARCH}) == [])
record("25-fp: a change OUTSIDE the fingerprinted section does NOT trip it (region scoping)",
       _ff(_SE, {"docs/spec/a.md": "## 0.7 Tree\n\nsrc/ here\n\n## 0.8 Pins\n\npnpm 10.99.9\n"}) == [])
record("25-fp: a change INSIDE the fingerprinted section IS caught",
       any("s07" in f.msg for f in _ff(_SE, {"docs/spec/a.md": "## 0.7 Tree\n\nsrc/ MOVED\n\n## 0.8 Pins\n\npnpm 10.13.1\n"})))

# region unresolvable -> fail-closed (a renamed/removed section, a missing file)
record("25-fp: a section anchor that no longer exists fails closed",
       any("not found" in f.msg for f in _ff(
           [{"id": "s", "kind": "section", "file": "docs/spec/a.md", "anchor": "9.9", "fingerprint": "sha256:" + "0" * 64}],
           {"docs/spec/a.md": _ARCH})))
record("25-fp: a source file that does not exist fails closed",
       any("not found" in f.msg for f in _ff(
           [{"id": "mm", "kind": "file", "file": "scripts/__nope__.toml", "fingerprint": "sha256:" + "0" * 64}], {})))

# [Test-Change: P0.3.12 G68 seed floor — old-obsolete+new-correct, build-gates §6 check 25(c)] the ledger's
# `dormant` skip is retired with its one user (the §0.7 -> CLAUDE.md §1a binding, replaced by G69's direct
# tracked-tree == §0.7 bind): the two dormancy legs are gone, and a leftover `dormant` table skips nothing.
_DORM = {"id": "d", "kind": "file", "file": _DOC, "fingerprint": "sha256:" + "0" * 64,  # deliberately WRONG
         "dormant": {"file": "CLAUDE.md", "contains": "P1.64"}}
record("25-fp: an entry carrying a leftover `dormant` table is checked like any other (no skip path) -> caught",
       any("'d'" in f.msg for f in _ff([_DORM], {_DOC: _CONTENT, "CLAUDE.md": "... finalized by the P1.64 box ..."})))

# malformed entries fail closed (never silently pass)
record("25-fp: a malformed fingerprint (not sha256:<64hex>) fails closed",
       any("malformed fingerprint" in f.msg for f in _ff(
           [{"id": "b", "kind": "file", "file": _DOC, "fingerprint": "deadbeef"}], {_DOC: _CONTENT})))
record("25-fp: an unknown kind fails closed",
       any("unknown kind" in f.msg for f in _ff(
           [{"id": "k", "kind": "blob", "file": _DOC, "fingerprint": "sha256:" + "0" * 64}], {_DOC: _CONTENT})))

# a missing committed ledger is fail-closed (the freshness axis cannot silently no-op)
with tempfile.TemporaryDirectory() as _td:
    _entries, _fatal = m._read_fp_ledger(Path(_td))
    record("25-fp: a missing committed ledger fails closed", _fatal is not None and _entries == [])

# the REAL ledger parses, carries the live seed, and is clean today (seed hashes match the real sources)
_RE, _RF = m._read_fp_ledger(ROOT)
record("25-fp: the real ledger parses + carries the live l-neg1-cage seed",
       _RF is None and any(e.get("id") == "l-neg1-cage" for e in _RE))
record("25-fp: the real ledger is clean today (every seed fingerprint matches its real source)",
       _RF is None and m._freshness_fingerprints(_RE, ROOT, m.load_docs(ROOT)) == [])


# --- G1-review fixes: the required-seed FLOOR + array-of-tables guard + list-dormancy (P0.3.12) ---
def _read_ledger_body(body):
    """Parse a synthetic ledger body through the real _read_fp_ledger over a temp root."""
    with tempfile.TemporaryDirectory() as td:
        sp = Path(td) / "scripts"
        sp.mkdir()
        (sp / "doc-fingerprints.toml").write_text(body, encoding="utf-8")
        return m._read_fp_ledger(Path(td))


_E64 = "0" * 64
# P2: an empty/seedless ledger (file present, zero [[source]]) FAILS the floor — no silent no-op
_e1, _f1 = _read_ledger_body("# only a comment, no [[source]] tables\n")
record("25-fp: an empty-but-present ledger fails the required-seed floor (no silent no-op)",
       _f1 is not None and "required seed" in _f1.msg)
# P2: dropping the required seed (another entry present, l-neg1-cage missing) FAILS closed
_e2, _f2 = _read_ledger_body(f'[[source]]\nid = "other"\nkind = "file"\nfile = "x"\nfingerprint = "sha256:{_E64}"\n')
record("25-fp: a ledger missing a required seed id fails closed (names the missing id)",
       _f2 is not None and "l-neg1-cage" in _f2.msg)
# P3: a plain string-array `source = [...]` fails closed CLEANLY (no AttributeError crash)
try:
    _e3, _f3 = _read_ledger_body('source = ["a", "b"]\n')
    _ok3 = _f3 is not None and "array of tables" in _f3.msg
except Exception:
    _ok3 = False
record("25-fp: a plain-array (non-array-of-tables) ledger fails closed, does NOT crash", _ok3)
# the floor is SATISFIED by a minimal ledger carrying the required id (not green-by-over-strictness)
_e4, _f4 = _read_ledger_body(
    f'[[source]]\nid = "l-neg1-cage"\nkind = "file"\nfile = "x"\nfingerprint = "sha256:{_E64}"\n')
record("25-fp: a ledger carrying every required seed id passes the floor (no fatal)", _f4 is None)
record("25-fp: the floor is exactly the cage binding (the retired §0.7 seed is not required)",
       m._REQUIRED_FP_IDS == {"l-neg1-cage"})
record("8 threat-parity: a §5 row citing a non-catalogue gate -> caught",
       any("G9999" in f.msg for f in m.doc8_threat_parity(
           dctx({"docs/security/security-concept.md": "| **T1** d | c | G9999 |\n",
                 "docs/security/build-gates.md": "| **G2** | x |\n"}))))

# checks reading the filesystem (16/21): exercise on the REAL repo (built ctx) — both clean today
_real = m.build_ctx(ROOT)
record("16 planted-positive: clean on the real repo (every built fail-closed §5 gate self-tested)",
       m.doc16_planted_positive(_real) == [])
# 16 reads the gate-selftests dir as `*.py` ONLY (2026-09-09): a `.legs` blessed leg-name set naming a
# gate id inside a leg NAME is data, not a self-test - the planted pair below discriminates the glob.
with tempfile.TemporaryDirectory() as _t16:
    _r16 = Path(_t16)
    (_r16 / "scripts" / "gate-selftests").mkdir(parents=True)
    (_r16 / "scripts" / "check-x").write_text("# implements G9999\n", encoding="utf-8")
    (_r16 / "scripts" / "gate-selftests" / "g24-x.legs").write_text("a leg naming G9999\n", encoding="utf-8")
    _c16 = m.Ctx(root=_r16, boxes=[], by_id={}, plan_files=[], gate_ids=set(),
                 docs={"docs/security/security-concept.md": "| **T1** d | c | G9999 |\n",
                       "docs/security/build-gates.md": "| **G9999** | x | fail-closed |\n"})
    record("16 planted-positive: a gate id inside a NON-.py data file under gate-selftests (a `.legs` "
           "blessed set) does NOT register as a self-test -> caught",
           any("G9999" in f.msg for f in m.doc16_planted_positive(_c16)))
    (_r16 / "scripts" / "gate-selftests" / "_helper.py").write_text("# a helper the runner never runs: G9999\n", encoding="utf-8")
    record("16 planted-positive: a gate id ONLY in a `_`-prefixed helper module (which run-gate-selftests never "
           "executes) does NOT register as a self-test -> still caught",
           any("G9999" in f.msg for f in m.doc16_planted_positive(_c16)))
    (_r16 / "scripts" / "gate-selftests" / "g24-x.py").write_text("# G9999 planted\n", encoding="utf-8")
    record("16 planted-positive: the same gate id in a .py self-test registers -> clean",
           m.doc16_planted_positive(_c16) == [])

# --- check 31: the phase-boundary sweep binding (2026-09-09) -------------------------------------------------
_SW = "Run the phase-end Co-Pilot hardening sweep over the whole P2 delivery"
_SW3 = "Run the phase-end Co-Pilot hardening sweep over the whole P3 delivery"


def _sweep_plan(**kw):
    """A two-phase plan in the bound shape; kw overrides let each leg break exactly one clause."""
    p2_sweep = box(bid="P2.9", marker=kw.get("m2", "!extern"), title=_SW, notes=["> owner"])
    p3_first = box(bid=kw.get("first3", "P3.1"), marker=" ", needs=kw.get("n31", ["P2.9"]))
    p3_sweep = box(bid="P3.5", marker="!extern", title=kw.get("t35", _SW3), notes=["> owner"])
    p3_signoff = box(bid="P3.6", marker=" ", title="Sign off", needs=kw.get("n36", ["P3.5"]))
    boxes = [box(bid="P2.1"), p2_sweep, p3_first, p3_sweep, p3_signoff] + kw.get("extra", [])
    for i, b in enumerate(boxes):          # document order = list order (one file)
        b.lineno = i + 1
    return ctx(boxes)


record("31 sweep: the bound shape (one sweep per phase, [!extern], last, P<n+1>.1 needs it, the final "
       "sign-off needs it) -> clean", m.doc31_sweep_binding(_sweep_plan()) == [])
record("31 sweep: a phase with NO sweep box -> caught",
       any("0 phase-end sweep boxes" in f.msg for f in m.doc31_sweep_binding(_sweep_plan(t35="Some other box"))))
record("31 sweep: TWO sweep boxes in one phase -> caught",
       any("2 phase-end sweep boxes" in f.msg for f in m.doc31_sweep_binding(
           _sweep_plan(extra=[box(bid="P3.7", marker="!extern", title=_SW3, notes=["> dup"], needs=["P3.5"])]))))
record("31 sweep: P<n+1>.1 without `needs:` the sweep -> caught",
       any("P3.1: must carry `needs: P2.9`" in f.msg for f in m.doc31_sweep_binding(_sweep_plan(n31=["P2.1"]))))
record("31 sweep: a box AFTER the sweep that does not `needs:` it -> caught",
       any("P3.6: follows the phase-end sweep box P3.5" in f.msg
           for f in m.doc31_sweep_binding(_sweep_plan(n36=["P3.1"]))))
record("31 sweep: an OPEN `[ ]` sweep box (not [!extern]) -> caught",
       any("found [ ]" in f.msg for f in m.doc31_sweep_binding(_sweep_plan(m2=" "))))
record("31 sweep: a successor phase whose first box P<n+1>.1 is ABSENT -> caught (fail-closed, never a silent skip)",
       any("P3.1 is absent" in f.msg for f in m.doc31_sweep_binding(_sweep_plan(first3="P3.2"))))
record("31 sweep: clean on the real plan (P2..P11 each bind their sweep)", m.doc31_sweep_binding(_real) == [])
record("registry: every DOC_CHECKS key has its numbered item in build-gates.md §6 (a check without its §6 "
       "definition is a dangling cross-reference - the check-31 r1 P1 catcher)",
       all(m.re.search(rf"^{k.split(':')[0]}\. \*\*", _real.docs.get("docs/security/build-gates.md", ""), m.re.M)
           for k in m.DOC_CHECKS))
# [Test-Change: G7 target-absent flip — old-obsolete+new-correct, build-gates §6] the old leg read the real
# repo under a "neither live -> skip" label while the real repo has the Semgrep ruleset (exactly one live);
# neither live is now a finding (§6 item 21: "not both, not neither"), so each arm gets its own root.
record("21 t2-taint-xor: the real repo (the Semgrep taint ruleset live, no CodeQL marker) -> clean",
       m.doc21_taint_xor(_real) == [])
with tempfile.TemporaryDirectory() as _t21:
    _r21 = Path(_t21)
    _c21 = m.Ctx(root=_r21, boxes=[], by_id={}, plan_files=[], docs={}, gate_ids=set())
    record("21 t2-taint-xor: NEITHER a CodeQL marker NOR the Semgrep ruleset live -> caught",
           any("NEITHER" in f.msg for f in m.doc21_taint_xor(_c21)))
    (_r21 / ".github").mkdir()
    (_r21 / ".github" / "codeql-default-setup").write_text("CodeQL javascript-typescript\n", encoding="utf-8")
    (_r21 / "scripts" / "semgrep-rules").mkdir(parents=True)
    record("21 t2-taint-xor: BOTH a CodeQL marker AND the Semgrep ruleset live -> caught (XOR)",
           any("BOTH" in f.msg for f in m.doc21_taint_xor(_c21)))
# --- check 24: p0-completion.md run_url is an immutable Actions-run URL (P0.6.10) --------------------
# The stub is BORN-GREEN: run_url holds the pattern-valid placeholder run `0` until the P0-exit commit
# fills the real run id. check 24 reddens ANY run_url: token that is not an Actions-run URL, so a non-URL
# placeholder (or a stray "run_url:" colon in prose BEFORE the data line) would fail; the schema describes
# the field as `run_url` (no colon) so the regex's FIRST match is the data line.
_PC_OK = ("# ConvertIA — P0 Completion Record\n\n## Record\n\n"
          "run_url: https://github.com/Ne-IA/convertia/actions/runs/0\ndate: 2026-01-01\n")
_PC = "docs/process/p0-completion.md"
record("24 p0-completion: a pattern-valid Actions-run URL (the runs/0 placeholder) -> no finding",
       m.doc24_p0_completion(dctx({_PC: _PC_OK})) == [])
# [Test-Change: G7 target-absent flip — old-obsolete+new-correct, build-gates §6] the record landed (P0.6.10),
# so its absence is a finding, never a skip.
record("24 p0-completion: absent p0-completion.md -> caught (a missing target is a finding)",
       [f.msg for f in m.doc24_p0_completion(dctx({}))]
       == ["docs/process/p0-completion.md is missing or empty — fail-closed"])
record("24 p0-completion: a non-URL placeholder token -> caught",
       any("does not match" in f.msg for f in m.doc24_p0_completion(
           dctx({_PC: _PC_OK.replace("https://github.com/Ne-IA/convertia/actions/runs/0", "<pending-at-exit>")}))))
record("24 p0-completion: a filled real run id passes",
       m.doc24_p0_completion(dctx({_PC: _PC_OK.replace("runs/0", "runs/27820505219")})) == [])
record("24 p0-completion: the REAL committed p0-completion.md stub passes (born-green)",
       m.doc24_p0_completion(_real) == [])
# Every doc check over a landed target reports a missing target: the synthetic dctx({}) probes above assert
# the finding (each paired with its real-doc leg), and the `missing target` family below covers the checks
# that had no such probe plus the registry-wide posture.

# --- check 26 (G69) structural-map integrity — the real logic, driven by pure fns (P0.3.13) ------
# [Test-Change: P0.3.13 G69 re-key — old-obsolete+new-correct, build-gates §6 check 26] the bind moved from
# the CLAUDE.md §1a map (now a pointer) to spec §0.7 itself: the §1a projection leg and the two
# PLACEHOLDER-skip legs are gone with the skip branch, the relation / fence / active / fail-closed legs are
# re-keyed onto the two-set tracked-tree == §0.7 bind, and the Load-bearing-files legs are new. The logic
# is exercised via the pure parser/relations fns + doc26 runs against the real repo tree (both directions).
_TREE = [
    "convertia/                  -> root",
    "├── docs/                   -> docs",
    "│   ├── spec/               -> spec",
    "│   └── plan/               -> plan",
    "├── src/                    -> ui",
    "│   ├── lib/ipc/bindings.ts -> generated (embedded-path ancestors)",
    "│   └── components/  hooks/  state/   -> siblings on one line",
    "└── assets/                 -> assets",
]
_md = m._parse_tree_dirs(_TREE)
record("26 parse: nested dirs reconstructed from indent (docs, docs/spec, docs/plan, assets)",
       {"docs", "docs/spec", "docs/plan", "assets"} <= _md)
record("26 parse: same-line sibling dirs all captured (src/components, src/hooks, src/state)",
       {"src/components", "src/hooks", "src/state"} <= _md)
record("26 parse: embedded-path ancestor dirs captured (src/lib, src/lib/ipc) but NOT the file",
       {"src/lib", "src/lib/ipc"} <= _md and "src/lib/ipc/bindings.ts" not in _md)
record("26 parse: the repo-root line (convertia/) is not itself a mapped dir",
       "convertia" not in _md and "" not in _md)

# the 2 relations (pure, set-only)
record("26 rel: tracked dirs == §0.7 dirs is clean",
       m._struct_map_relations({"docs", "src"}, {"docs", "src"}) == [])
record("26 rel: a tracked dir with no §0.7 row is caught (a folder without a row)",
       m._struct_map_relations({"docs"}, {"docs", "newdir"}) == [("disk-not-in-spec07", "newdir")])
record("26 rel: a §0.7 dir with no tracked file beneath it is caught (a stale row)",
       m._struct_map_relations({"docs", "gone"}, {"docs"}) == [("spec07-not-on-disk", "gone")])

# _dirs_from_files: every ancestor of a tracked path, minus the out-of-scope trees
record("26 disk: ancestor dirs derived from tracked-file paths (a, a/b, scripts)",
       m._dirs_from_files(["a/b/c.txt", "a/d.txt", "scripts/x", "top.md"]) == {"a", "a/b", "scripts"})
record("26 disk: the out-of-scope trees (target/node_modules/dist/.git) are excluded",
       m._dirs_from_files(["target/x/y", "node_modules/p/i.js", "dist/a", ".git/z", "src/m.rs"]) == {"src"})

# _fenced_block_after: pulls the block under the §0.7 Physical tree header, None when no fence
_DOCT = "### Physical tree (mapping)\n\n```\nconvertia/\n├── docs/\n```\n\n## 2 Next\n"
record("26 fence: extracts the fenced block under the §0.7 Physical tree header",
       m._fenced_block_after(_DOCT, m._PHYSICAL_TREE_HDR) == ["convertia/", "├── docs/"])
record("26 fence: returns None when the header has no following fence",
       m._fenced_block_after("### Physical tree\n\njust prose, no fence\n", m._PHYSICAL_TREE_HDR) is None)

# _load_bearing_paths: one path per bullet that opens with a code span, section-scoped
_LBT = ("### Load-bearing files\n\nintro naming `not/a/bullet.rs`\n\n- `a.toml` — role\n* `b/c.rs` — role\n"
        "- plain bullet without a code span\n\n## 0.8 Next\n\n- `after/next-heading.rs` — out of the section\n")
record("26 load-bearing: `-` and `*` code-span bullets are listed in order; prose, a plain bullet and a bullet "
       "under the next heading are not",
       m._load_bearing_paths(_LBT) == ["a.toml", "b/c.rs"])
record("26 load-bearing: a missing section is None, a section without a code-span bullet is []",
       m._load_bearing_paths("## 0.7 x\n\nno such section\n") is None
       and m._load_bearing_paths("### Load-bearing files\n\n- plain\n") == [])

# doc26 over the REAL repo: tracked dirs == §0.7 dirs both ways, every load-bearing file tracked
_REAL_ARCH = m.load_docs(ROOT).get("docs/spec/00-architecture.md", "")
_REAL_LB = m._load_bearing_paths(_REAL_ARCH) or []
record("26 active: the real repo passes (tracked dirs == spec §0.7 dirs both ways; every load-bearing file tracked)",
       m.doc26_struct_map(_real) == [])
record("26 active: the real §0.7 load-bearing list is parsed (the workspace root, the Tauri config and the IPC door "
       "among its entries)",
       {"Cargo.toml", "src-tauri/tauri.conf.json", "src/lib/ipc/bindings.ts"} <= set(_REAL_LB))


def _doc26(arch_text):
    return m.doc26_struct_map(m.Ctx(root=ROOT, boxes=[], by_id={}, plan_files=[],
                                    docs={"docs/spec/00-architecture.md": arch_text}))


# both directions end to end through find + parse + real git ls-files + relations
_af = _doc26("### Physical tree\n\n```\nconvertia/\n├── docs/\n├── zzz-bogus/\n```\n\n"
             "### Load-bearing files\n\n- `Cargo.toml` — root\n")
record("26 active: a §0.7-only bogus dir is flagged (no tracked file beneath it)",
       any("'zzz-bogus'" in f.msg and "no tracked file beneath it" in f.msg for f in _af))
record("26 active: a real tracked dir absent from §0.7 IS flagged (disk -> §0.7 via real git)",
       any("'scripts/gate-selftests'" in f.msg and "no spec §0.7 Physical-tree row" in f.msg for f in _af))
_ARCH_MINUS = "\n".join(ln for ln in _REAL_ARCH.split("\n")
                        if not ln.lstrip("│├└─ ").startswith("typos-fixtures/"))
record("26 active: the real §0.7 tree with its typos-fixtures/ row deleted flags exactly that tracked dir",
       _ARCH_MINUS != _REAL_ARCH
       and [f.msg for f in _doc26(_ARCH_MINUS)]
       == [m._STRUCT_MSG["disk-not-in-spec07"].format(d="scripts/gate-selftests/typos-fixtures")])
_ARCH_PLANT = _REAL_ARCH.replace("- `tsconfig.json` —", "- `zzz/untracked.json` — planted\n- `tsconfig.json` —", 1)
record("26 load-bearing: an untracked path on the real list is caught, and only it",
       _ARCH_PLANT != _REAL_ARCH
       and [f.msg for f in _doc26(_ARCH_PLANT)] == ["load-bearing file 'zzz/untracked.json' is not tracked"])
record("26 load-bearing: every entry of the real list is a tracked file (clean)",
       len(_REAL_LB) > 0 and _doc26(_REAL_ARCH) == [])

# fail-CLOSED branches (never a silent [])
record("26 fail-closed: no §0.7 Physical tree block -> Finding",
       any("Physical tree" in f.msg for f in _doc26("no physical tree here\n")))
record("26 fail-closed: a Physical tree block naming no directory -> Finding",
       any("names no directory" in f.msg for f in _doc26("### Physical tree\n\n```\nconvertia/\n```\n")))
record("26 fail-closed: the real tree with no Load-bearing files section -> Finding",
       [f.msg for f in _doc26(_REAL_ARCH.replace("### Load-bearing files", "### Other files"))]
       == ["the §0.7 'Load-bearing files' section is missing — fail-closed"])
record("26 fail-closed: the real tree with an empty Load-bearing files section -> Finding",
       [f.msg for f in _doc26(_REAL_ARCH.replace("### Load-bearing files", "### Load-bearing files\n\n### Moved list"))]
       == ["the §0.7 'Load-bearing files' section lists no `path` bullet — fail-closed"])
_saved_ls = m._git_ls_files
try:
    m._git_ls_files = lambda root: None
    _git_down = _doc26(_REAL_ARCH)
finally:
    m._git_ls_files = _saved_ls
record("26 fail-closed: an unavailable git -> Finding (and the stub is restored)",
       any("git ls-files failed" in f.msg for f in _git_down) and m._git_ls_files is _saved_ls)

# G1 r1 parser hardening: annotation prose / sibling embedded-paths cannot inject phantom dirs
record("26 parse: a slash-word in an arrow/hash annotation is NOT mined as a dir (false-negative closed)",
       m._parse_tree_dirs(["convertia/", "└── docs/   → moved to src-tauri/src/ipc"]) == {"docs"})
record("26 parse: a same-line sibling must be a SIMPLE dir — an embedded-path sibling is not mined",
       m._parse_tree_dirs(["convertia/", "├── a/  lib/ipc/"]) == {"a"})
record("26 parse: a hash-annotation slash-word is not mined either (§0.7 # convention)",
       m._parse_tree_dirs(["convertia/", "└── src/   # see config/secrets/ for details"]) == {"src"})

# --- check 27 (the G71/P1 prevention): posture-flag fail-soft -> fail-closed transition wiring -----
# The pure core ARMS: a DUE-but-unwired flip / a DUE flip with a stale fail_open excuse / an
# unjustified soft posture are each CAUGHT; and the REAL committed registry passes (G71 wired) yet
# would FAIL if its wiring were stripped of --enforce (the exact original G71/P1 silent-no-flip). [P1.66]
_PF_ROWS = [{"gate": "G71", "script": "scripts/check-l-neg1-ack", "enforce_flag": "--enforce",
             "fail_closed_after_phase": "P1", "planes": ["lefthook.yml", ".github/workflows/ci.yml"]}]
_PF_WIRED = {"lefthook.yml": "run: python3 scripts/check-l-neg1-ack --enforce\n",
             ".github/workflows/ci.yml": 'run: python3 scripts/check-l-neg1-ack --base "$X" --enforce\n'}
_PF_UNWIRED = {"lefthook.yml": "run: python3 scripts/check-l-neg1-ack\n",
               ".github/workflows/ci.yml": 'run: python3 scripts/check-l-neg1-ack --base "$X" --enforce\n'}
record("27 posture: DUE (phase complete) + wired everywhere + no fail_open -> clean",
       m._gate_posture_findings(_PF_ROWS, set(), {"P1"}, _PF_WIRED) == [])
record("27 posture: DUE + a plane NOT wired -> caught (the exact G71/P1 unwired-flip bug)",
       any("NOT wired" in s for s in m._gate_posture_findings(_PF_ROWS, set(), {"P1"}, _PF_UNWIRED)))
record("27 posture: DUE + a stale [[fail_open]] row still excusing it -> caught",
       any("excuses" in s for s in m._gate_posture_findings(_PF_ROWS, {"G71"}, {"P1"}, _PF_WIRED)))
record("27 posture: phase NOT complete + no fail_open + not wired -> caught (unjustified soft)",
       any("fail-soft" in s for s in m._gate_posture_findings(_PF_ROWS, set(), set(), _PF_UNWIRED)))
record("27 posture: phase NOT complete + a fail_open row justifies the soft posture -> clean",
       m._gate_posture_findings(_PF_ROWS, {"G71"}, set(), _PF_UNWIRED) == [])
record("27 posture: an under-specified row (empty planes) -> caught",
       any("under-specified" in s for s in m._gate_posture_findings(
           [{"gate": "Gx", "script": "s", "enforce_flag": "--e", "fail_closed_after_phase": "P1", "planes": []}],
           set(), {"P1"}, {})))
record("27 wired-in: a '#'-commented invocation is NOT counted as wired",
       m._posture_wired_in("# run: scripts/check-l-neg1-ack --enforce\n", "scripts/check-l-neg1-ack", "--enforce") is False)
record("27 wired-in: a TRAILING inline comment (' # --enforce') is NOT counted (promised, not applied)",
       m._posture_wired_in("run: python3 scripts/check-l-neg1-ack  # --enforce\n", "scripts/check-l-neg1-ack", "--enforce") is False)
record("27 wired-in: a real run-line invocation IS counted",
       m._posture_wired_in("run: python3 scripts/check-l-neg1-ack --enforce\n", "scripts/check-l-neg1-ack", "--enforce") is True)
record("27 phases: an open [ ] box blocks completion; an all-[x] phase is complete",
       m._completed_phases([box(bid="P1.1", raw="x"), box(bid="P1.2", raw=" ")]) == set()
       and m._completed_phases([box(bid="P1.1", raw="x")]) == {"P1"})
record("27 doc27: the REAL repo passes (G71 wired, P1 complete, no fail_open excuse)",
       m.doc27_gate_posture_transition(_real) == [])
# the REAL committed registry ARMS: real posture rows + a wiring stripped of --enforce -> caught
_gp27 = m.tomllib.loads((ROOT / "scripts" / "gate-planes.toml").read_text(encoding="utf-8"))
_real_pf = _gp27.get("posture_flag", [])
_stripped27 = {p: "run: python3 scripts/check-l-neg1-ack\n" for pf in _real_pf for p in pf.get("planes", [])}
record("27 doc27: the REAL committed registry ARMS (real rows + stripped wiring -> caught)",
       bool(_real_pf) and len(m._gate_posture_findings(_real_pf, set(), {"P1"}, _stripped27)) > 0)

# --- check 30: spec-restatement fidelity (the P4.33 QuarantinedByOs reconcile's class) ----------------
# Own inline catalog: two §2.8.2 rows (one carrying inner quotes, the markdown `\"`-escape case) + a §2.8.3
# heading closing the section, so the section slice is exercised too. `_g30_run` scans the given files
# against it. The span-shape clauses (glued mark / emphasis mark / table cell / the 400-char cap) each get a
# leg that fails when ONLY that clause is dropped — the fixtures carry no other blocker.
_g30 = ("# t\n### 2.8.2 The message catalog\n"
        "| `Corrupt` | **\"This file looks damaged and couldn't be converted.\"** | — |\n"
        "| `Quarantined` | **\"Could not launch it - click \"Open Anyway\" next to it, then try again.\"** | — |\n"
        "### 2.8.3 next\n")


def _g30_run(files):
    return m.doc30_spec_restatement_fidelity(dctx({"docs/spec/02-guarantees.md": _g30, **files}))


record("30 restatement: a DRIFTED restatement (same opening words, different wording) -> caught",
       any(f.file == "docs/spec/05-ui-ux.md" for f in _g30_run({
           "docs/spec/05-ui-ux.md": "the toast reads *\"This file looks damaged and could not be converted.\"*\n"})))
record("30 restatement: a drift in the OPENING word only -> caught (the anchor slides, it is not a head)",
       _g30_run({"docs/spec/05-ui-ux.md": "*\"That file looks damaged and couldn't be converted.\"*\n"}) != [])
record("30 restatement: a VERBATIM restatement, hard-wrapped and `\\\"`-escaped -> NOT caught",
       _g30_run({"docs/spec/07-app-shell.md": "*\"Could not launch it - click \\\"Open Anyway\\\" next to it,\n"
                                              "  then try again.\"* and *\"This file looks damaged\n"
                                              "  and couldn't be converted.\"*\n"}) == [])
record("30 restatement: a DRIFTED quotation that KEEPS its inner quote (the QuarantinedByOs shape) -> caught",
       _g30_run({"docs/spec/07-app-shell.md": "*\"Could not launch it - click \\\"Open Anyway\\\" beside it,\n"
                                              "  then try once more.\"*\n"}) != [])
record("30 restatement: the HISTORICAL §2.8.2 <-> §7.2.4 QuarantinedByOs pair (92eeb2b^) replays as a finding",
       m.doc30_spec_restatement_fidelity(dctx({
           "docs/spec/02-guarantees.md": ("# t\n### 2.8.2 c\n| `QuarantinedByOs` | **\"macOS is blocking one of "
               "ConvertIA's built-in tools with a security check. Open System Settings → Privacy & Security and "
               "choose \"Open Anyway\", then try again.\"** | — |\n### 2.8.3 n\n"),
           "docs/spec/07-app-shell.md": ("  **Canonical `QuarantinedByOs` message:** *\"Could not launch {engine name} — blocked by macOS\n"
               "  security. Open System Settings → Privacy & Security and click \\\"Open Anyway\\\" next to\n"
               "  {engine name}, then try again.\"* The `{engine name}` is the friendly sidecar name.\n")})) != [])
record("30 restatement: a file quoting the string verbatim ONCE and drifted in a SECOND quotation -> caught",
       _g30_run({"docs/spec/05-ui-ux.md": "*\"This file looks damaged and couldn't be converted.\"* and later\n"
                                          "the calm form \"This file looks damaged and could not be converted.\"\n"}) != [])
record("30 restatement: a paraphrase sharing no five-word run, and a docs/plan file, are out of scope",
       _g30_run({"docs/spec/05-ui-ux.md": "a damaged file is refused with a calm line\n",
                 "docs/plan/P9.md": "*\"This file looks damaged and could not be converted.\"*\n"}) == [])
record("30 restatement: a run shared with ANOTHER canonical string quoted verbatim is not a drift",
       m.doc30_spec_restatement_fidelity(dctx({
           "docs/spec/02-guarantees.md": ("# t\n### 2.8.2 c\n"
               "| `A` | **\"This file couldn't be converted, and a temporary file may remain at {path}.\"** |\n"
               "| `B` | **\"Converted — a temporary file may remain at {path}.\"** |\n### 2.8.3 n\n"),
           "docs/spec/05-ui-ux.md": "the note reads \"Converted — a temporary file may remain at {path}.\"\n"})) == [])
record("30 restatement: bold prose carrying a run BETWEEN two unrelated quote marks (a mis-paired pair) -> NOT caught",
       _g30_run({"docs/spec/06-build-test-release.md": "on a 12\" display: **click the button next to it, then try again** —\n"
                                                       "  the dialog \"shows once more and then vanishes\" afterwards\n"}) == [])
record("30 restatement: span shape - a `12\"` mark glued to a word never OPENS a span",
       _g30_run({"docs/spec/06-build-test-release.md":
                 "a 12\" display; the notice next to it, then try again appears; then a lone \" mark\n"}) == [])
record("30 restatement: span shape - a span never crosses a markdown emphasis mark",
       _g30_run({"docs/spec/06-build-test-release.md":
                 "the notice reads \"ConvertIA can't be opened — **click next to it, then try again** — and stays\"\n"}) == [])
record("30 restatement: span shape - a span never crosses a table cell boundary",
       _g30_run({"docs/spec/06-build-test-release.md":
                 "| a lone \" mark | next to it, then try again | and a closing \" mark |\n"}) == [])
record("30 restatement: span shape - two marks over 400 characters apart bound a sweep, not a quotation",
       _g30_run({"docs/spec/06-build-test-release.md":
                 "a lone \" mark " + "filler " * 60 + "next to it, then try again " + "filler " * 5 + "and a closing \" mark\n"}) == [])
_g30n = ("# t\n### 2.8.2 c\n| `UnopenableOutputName` | **\"The output name \"{name}\" can't be used as a file on "
         "Windows, so this file was skipped.\"** | — |\n### 2.8.3 n\n")
record("30 restatement: a VERBATIM quotation whose inner fragment opens with a NON-word char (`\"{name}\"`, the span "
       "splits) -> NOT caught: the string is blanked before the spans are cut",
       m.doc30_spec_restatement_fidelity(dctx({
           "docs/spec/02-guarantees.md": _g30n,
           "docs/spec/05-ui-ux.md": "the row reads *\"The output name \\\"{name}\\\" can't be used as a file on Windows,\n"
                                    "  so this file was skipped.\"* and plain: \"The output name \"{name}\" can't be used as a\n"
                                    "  file on Windows, so this file was skipped.\"\n"})) == [])
record("30 restatement: a DRIFTED quotation of the `\"{name}\"` shape (the split halves still carry the runs) -> caught",
       m.doc30_spec_restatement_fidelity(dctx({
           "docs/spec/02-guarantees.md": _g30n,
           "docs/spec/05-ui-ux.md": "*\"The output name \"{name}\" cannot be used as a file on Windows, so this file was skipped.\"*\n"})) != [])
record("30 restatement: a drifted quotation keeping a 120-character inner fragment is still seen (the fragment bound is 200)",
       _g30_run({"docs/spec/07-app-shell.md": "*\"Could not launch it - click \"" + "x" * 120 + "\" beside it, then try once more.\"*\n"}) != [])
record("30 restatement: a MID-WORD drift of a SHORT (8-word) string, which no five-word run can see -> caught by word similarity",
       _g30_run({"docs/spec/05-ui-ux.md": "the toast reads \"This file looks broken and couldn't be converted.\"\n"}) != [])
record("30 restatement: a short quotation sharing only some words with a short string is prose, not a drift -> NOT caught",
       _g30_run({"docs/spec/05-ui-ux.md": "the toast reads \"This file is fine and was converted.\"\n"}) == [])
record("30 restatement: a quotation carrying the string VERBATIM plus authored text inside the same marks is not verbatim -> caught",
       _g30_run({"docs/spec/05-ui-ux.md": "the toast reads \"This file looks damaged and couldn't be converted. Try again.\"\n"}) != [])
record("30 restatement: a short string drifted by one word AND case AND punctuation (the 05-ui-ux:821 shape) -> caught "
       "(runs and similarity compare NORMALISED tokens)",
       _g30_run({"docs/spec/05-ui-ux.md": "the note reads (\"this file looks broken and couldn't be converted\")\n"}) != [])
record("30 restatement: a CURLY-quoted drifted quotation (U+201C/U+201D) is judged like a straight one -> caught",
       _g30_run({"docs/spec/05-ui-ux.md": "the toast reads \u201cThis file looks damaged and could not be converted.\u201d\n"}) != [])
_g30esc = ("# t\n### 2.8.2 c\n| `U` | **\"The output name \\\"{name}\\\" can't be used as a file on Windows.\"** | — |\n### 2.8.3 n\n")
record("30 restatement: a catalog row carrying a markdown-ESCAPED inner quote is unescaped like a quoting file - its verbatim quotation"
       " is clean, a drifted one is caught",
       m.doc30_spec_restatement_fidelity(dctx({"docs/spec/02-guarantees.md": _g30esc,
           "docs/spec/05-ui-ux.md": "*\"The output name \\\"{name}\\\" can't be used as a file on Windows.\"*\n"})) == []
       and m.doc30_spec_restatement_fidelity(dctx({"docs/spec/02-guarantees.md": _g30esc,
           "docs/spec/05-ui-ux.md": "*\"The output name \\\"{name}\\\" cannot be used as a file on Windows.\"*\n"})) != [])
_g30_real = m._canonical_strings((SCRIPT.parents[1] / "docs" / "spec" / "02-guarantees.md").read_text(encoding="utf-8"))
record("30 restatement: precondition on the REAL catalog - no canonical string of nine tokens or fewer carries an inner quote, so the"
       " split-span residual (a short string's quotation cut in two by its own inner fragment) stays dormant (the round-12 P3)",
       len(_g30_real) >= 50 and not any(len(m._tokens(s)) <= 9 and '"' in s for s in _g30_real))
_ssot = "docs/SINGLE-SOURCE-OF-TRUTH.md"
record("30 restatement: the SSOT is in scope (it OUTRANKS the spec - the round-15 finding: a drifted quotation survived there);"
       " a drifted SSOT quotation is caught, a verbatim one is clean",
       m.doc30_spec_restatement_fidelity(dctx({"docs/spec/02-guarantees.md": _g30,
           _ssot: "shown as a note (\"Could not launch it - click \"Open Anyway\" beside it, then try again.\")\n"})) != []
       and m.doc30_spec_restatement_fidelity(dctx({"docs/spec/02-guarantees.md": _g30,
           _ssot: "shown as a note (\"Could not launch it - click \"Open Anyway\" next to it, then try again.\")\n"})) == [])
record("30 restatement: docs/plan is OUT of scope (history quotes - the named residual)",
       m.doc30_spec_restatement_fidelity(dctx({"docs/spec/02-guarantees.md": _g30,
           "docs/plan/P5-images.md": "> \"Could not launch it - click \"Open Anyway\" beside it, then try again.\"\n"})) == [])
# [Test-Change: G7 target-absent flip — old-obsolete+new-correct, build-gates §6] the catalog landed, so a
# 02-guarantees.md yielding no canonical string is a finding, never a clean skip.
record("30 restatement: a missing catalog (no catalog section) -> caught (a missing target is a finding)",
       [f.msg for f in m.doc30_spec_restatement_fidelity(dctx({"docs/spec/05-ui-ux.md": "This file looks damaged and could not\n"}))]
       == ["the §2.8.2/§2.9.1 canonical-string catalog in docs/spec/02-guarantees.md is missing or empty — fail-closed"])
record("30 restatement: non-vacuity - the REAL 02-guarantees.md yields a full catalog (>= 50 canonical strings)",
       len(m._canonical_strings((ROOT / "docs/spec/02-guarantees.md").read_text(encoding="utf-8"))) >= 50)
record("30 restatement: the REAL spec set passes (every cross-file quotation is verbatim)",
       m.doc30_spec_restatement_fidelity(m.build_ctx(ROOT)) == [])

# --- base-case golden invariant ---------------------------------------------------------------
rc_real = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True, text=True, encoding="utf-8", errors="replace").returncode
record("base-case: the REAL plan passes the format checks (exit 0)", rc_real == 0)
broken = ctx([box(bid="P0.1", raw="X", tags=["NOPE"], title="bad.", refs="")])
record("base-case: a deliberately-broken box yields findings (would exit 1)", len(m.run(broken)) >= 1)

# --- check 32: the SAST pin is single-sourced (requirements-ci.txt vs its header + the G29 row + the P0.4.2 note)
_BG32 = "| **G29** | SAST | Semgrep `1.168.0` (bumped to `{v}` 2026-08-25) hash-pinned | push |\n"
_P032 = "- [x] **P0.4.2** [GATE] Wire SAST · G29\n  > **Delivered:** `semgrep==1.168.0` (bumped to `{v}`)\n- [x] **P0.4.3** [GATE] Next\n"
_REQ32 = "# Pinned EXACT-version `semgrep=={h}` + its tree\nsemgrep=={p} \\\n    --hash=sha256:aa\n"
with tempfile.TemporaryDirectory() as _d32:
    _r32 = Path(_d32)
    def _c32(req: str | None, v: str = "9.9.9", bg: str | None = None, p0: str | None = None):
        if req is None:
            (_r32 / "requirements-ci.txt").unlink(missing_ok=True)
        else:
            (_r32 / "requirements-ci.txt").write_text(req, encoding="utf-8")
        return m.Ctx(root=_r32, boxes=[], by_id={}, plan_files=[], gate_ids=set(),
                     docs={"docs/security/build-gates.md": _BG32.format(v=bg or v),
                           "docs/plan/P0-build-and-security.md": _P032.format(v=p0 or v)})
    record("32 sast-pin-sync: pin 9.9.9 named in the header, the G29 row AND the P0.4.2 note -> clean",
           m.doc32_sast_pin_sync(_c32(_REQ32.format(h="9.9.9", p="9.9.9"))) == [])
    _f32 = m.doc32_sast_pin_sync(_c32(_REQ32.format(h="9.9.10", p="9.9.10")))
    record("32 sast-pin-sync: the pin moved (9.9.10) but both doc notes still say 9.9.9 -> BOTH doc sites flagged",
           len(_f32) == 2 and {f.file for f in _f32} == {"docs/security/build-gates.md", "docs/plan/P0-build-and-security.md"})
    record("32 sast-pin-sync: only the P0.4.2 note stale -> exactly that site flagged",
           [f.file for f in m.doc32_sast_pin_sync(_c32(_REQ32.format(h="9.9.10", p="9.9.10"), bg="9.9.10", p0="9.9.9"))]
           == ["docs/plan/P0-build-and-security.md"])
    record("32 sast-pin-sync: the requirements HEADER line stale (`semgrep==9.9.9` above a 9.9.10 pin) -> flagged (the third site)",
           any(f.file == "requirements-ci.txt" and "header line" in f.msg
               for f in m.doc32_sast_pin_sync(_c32(_REQ32.format(h="9.9.9", p="9.9.10"), v="9.9.10"))))
    record("32 sast-pin-sync: a requirements file WITHOUT a semgrep== pin -> flagged (the file's reason to exist)",
           any("no `semgrep==" in f.msg for f in m.doc32_sast_pin_sync(_c32("requests==2.0.0\n"))))
    # [Test-Change: G7 target-absent flip — old-obsolete+new-correct, build-gates §6] requirements-ci.txt
    # landed, so its absence is a finding, never target-absent.
    record("32 sast-pin-sync: requirements-ci.txt absent -> caught (a missing target is a finding)",
           [f.msg for f in m.doc32_sast_pin_sync(_c32(None))] == ["requirements-ci.txt is missing or empty — fail-closed"])
record("32 sast-pin-sync: the REAL repo is clean (the live pin is named in all three sites)",
       m.doc32_sast_pin_sync(m.Ctx(root=ROOT, boxes=[], by_id={}, plan_files=[], gate_ids=set(), docs={})) == [])

# --- check 33: a gate row never defers a leg to an unowned box (shape rule, word window, real-box resolution) --
_B33 = [box("P4.96"), box("P0.4.1"), box("P4.50")]
def _c33(bg: str):
    return m.Ctx(root=ROOT, boxes=_B33, by_id={b.box_id: b for b in _B33}, plan_files=[], gate_ids=set(),
                 docs={"docs/security/build-gates.md": bg})
record("33 gate-row-promise: 'the JS leg + the floor are later boxes' without a box id -> caught",
       any("unowned box" in f.msg for f in m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg + the floor are later boxes | push |\n"))))
record("33 gate-row-promise: 'a later box, P4.96 (authored 2026-09-24)' - the owner named right after it -> clean",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a later box, P4.96 (authored 2026-09-24). | push |\n")) == [])
record("33 gate-row-promise: the id in ANOTHER sentence of the row does not own the promise -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | Delivered by P0.4.1. The JS leg is a later box. | push |\n")) != [])
record("33 gate-row-promise: an id ONE word past a period ('...a later box. P4.96 owns the next one.') is in another "
       "sentence -> caught (the sentence split, not the window, decides)",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a later box. P4.96 owns the next one. | push |\n")) != [])
record("33 gate-row-promise: an id 9 words BEFORE the phrase in a comma-joined run (the r2 review's evasion) -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | Delivered per P4.50, with various other details noted, the JS leg is a later box, more details | push |\n")) != [])
record("33 gate-row-promise: an em-dash-joined run without a nearby id -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | Delivered per P4.50 — various details — the JS leg is a later box — more | push |\n")) != [])
record("33 gate-row-promise: an id that resolves to NO plan box (P99.99) does not own the promise -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a later box, P99.99. | push |\n")) != [])
record("33 gate-row-promise: the window counts WORDS - a comma token plus nine words then the id is still within 10 -> clean",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a later box , w1 w2 w3 w4 w5 w6 w7 w8 w9 P4.96 owns it. | push |\n")) == [])
record("33 gate-row-promise: the deferral adjective 'later' before 'box' without an owner -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | this leg lives in its later box. | push |\n")) != [])
record("33 gate-row-promise: the deferral adjective 'future' before 'box' without an owner -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | this leg lives in its future box. | push |\n")) != [])
record("33 gate-row-promise: the deferral adjective 'subsequent' before 'box' without an owner -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | this leg lives in its subsequent box. | push |\n")) != [])
record("33 gate-row-promise: the deferral adjective 'next' before 'box' without an owner -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | this leg lives in its next box. | push |\n")) != [])
record("33 gate-row-promise: the deferral adjective 'follow-on' before 'box' without an owner -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | this leg lives in its follow-on box. | push |\n")) != [])
record("33 gate-row-promise: the deferral adjective 'follow-up' before 'box' without an owner -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | this leg lives in its follow-up box. | push |\n")) != [])
record("33 gate-row-promise: the deferral adjective 'later-phase' before 'box' without an owner -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | this leg lives in its later-phase box. | push |\n")) != [])
record("33 gate-row-promise: the deferral adjective 'own' before 'box' without an owner -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | this leg lives in its own box. | push |\n")) != [])
record("33 gate-row-promise: the deferral adjective 'separate' before 'box' without an owner -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | this leg lives in its separate box. | push |\n")) != [])
record("33 gate-row-promise: the deferral adjective 'dedicated' before 'box' without an owner -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | this leg lives in its dedicated box. | push |\n")) != [])
record("33 gate-row-promise: the deferral adjective 'owning' before 'box' without an owner -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | this leg lives in its owning box. | push |\n")) != [])
record("33 gate-row-promise: the deferral adjective 'acquisition' before 'box' without an owner -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | this leg lives in its acquisition box. | push |\n")) != [])
record("33 gate-row-promise: the deferral adjective 'staging' before 'box' without an owner -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | this leg lives in its staging box. | push |\n")) != [])
record("33 gate-row-promise: the deferral adjective 'stage-slot' before 'box' without an owner -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | this leg lives in its stage-slot box. | push |\n")) != [])
record("33 gate-row-promise: 'in later phases' (plural, the G19 shape) -> caught",
       m.doc33_gate_row_promise(_c33("| **G19** | x | the CLI --help + asset manifest in later phases. | push |\n")) != [])
record("33 gate-row-promise: 'a **later** box' (markdown emphasis inside the phrase) -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | the leg is a **later** box. | push |\n")) != [])
record("33 gate-row-promise: 'the next phase' -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | the release leg lands in the next phase. | push |\n")) != [])
record("33 gate-row-promise: 'lands in a later phase' -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | the release leg lands in a later phase. | push |\n")) != [])
record("33 gate-row-promise: 'its own box-id' (a hyphenated noun) is NOT a deferral -> clean",
       m.doc33_gate_row_promise(_c33("| **G9** | x | each entry carries its own box-id and its own separate box-ids. | push |\n")) == [])
record("33 gate-row-promise: 'its own acquisition box' (the G56 pinact shape) -> caught",
       m.doc33_gate_row_promise(_c33("| **G56** | x | the transitive half remains with its own acquisition box. | push |\n")) != [])
record("33 gate-row-promise: 'the Lane-B staging box extends check (10)' (the G37 shape) -> caught",
       m.doc33_gate_row_promise(_c33("| **G37** | x | the Lane-B staging box extends check (10) into the verify step. | push |\n")) != [])
record("33 gate-row-promise: a hard-wrapped §6 paragraph with the phrase on one line and its owner on the NEXT line -> joined, clean",
       m.doc33_gate_row_promise(_c33("31. **Title** — the JS leg is a later\n    box, P4.96 (authored 2026-09-24); more text.\n")) == [])
record("33 gate-row-promise: a hard-wrapped UNOWNED promise ('...is a later' / '    box; more') -> joined, caught "
       "(without the join the two words never meet)",
       m.doc33_gate_row_promise(_c33("31. **Title** — the JS leg is a later\n    box; more text.\n")) != [])
record("33 gate-row-promise: 'future boxes' is the same promise -> caught",
       m.doc33_gate_row_promise(_c33("| **G9** | x | the rest are future boxes | push |\n")) != [])
record("33 gate-row-promise: an id at word 11 AFTER the phrase (one past the 10-word window) does not own it -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a later box w1 w2 w3 w4 w5 w6 w7 w8 w9 w10 P4.96 owns it | push |\n")) != [])
record("33 gate-row-promise: an id past a SEMICOLON ('...a later box; P4.96 owns it') is in another sentence -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a later box; P4.96 owns it | push |\n")) != [])
record("33 gate-row-promise: punctuation tokens before the phrase do not consume the 4-word window ('P4.96 , — , JS leg "
       "is a later box') -> clean",
       m.doc33_gate_row_promise(_c33("| **G17** | x | P4.96 , — , JS leg is a later box | push |\n")) == [])
record("33 gate-row-promise: 'a later P4 box' (a phase number between the adjective and the noun) is the same promise -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a later P4 box | push |\n")) != [])
record("33 gate-row-promise: an id exactly FIVE words before the phrase (one past the 4-word window) does not own it -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | P4.96 w1 w2 w3 w4 a later box | push |\n")) != [])
record("33 gate-row-promise: emphasis on the NOUN ('a later **box**') is the same promise -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a later **box** | push |\n")) != [])
record("33 gate-row-promise: backticks on the noun ('a later `box`') is the same promise -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a later `box` | push |\n")) != [])
record("33 gate-row-promise: a parenthesised adjective ('a (later) box') is the same promise -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a (later) box | push |\n")) != [])
record("33 gate-row-promise: the article before an EMPHASISED adjective belongs to the phrase ('P4.96 w1 w2 w3 a **later** box': the id is the 4th word before the phrase, not the 5th) -> clean",
       m.doc33_gate_row_promise(_c33("| **G17** | x | P4.96 w1 w2 w3 a **later** box | push |\n")) == [])
record("33 gate-row-promise: backticks on the ADJECTIVE ('a `later` box') is the same promise -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a `later` box | push |\n")) != [])
record("33 gate-row-promise: parentheses on the NOUN ('a later (box)') is the same promise -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a later (box) | push |\n")) != [])
record("33 gate-row-promise: underscore emphasis ('a _later_ box') is the same promise -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a _later_ box | push |\n")) != [])
record("33 gate-row-promise: underscore emphasis on the NOUN ('a later _box_') -> caught (the r8 review: the trailing `_` is "
       "a word character, so a bare `(?![\\w-])` lookahead let it slip)",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a later _box_ | push |\n")) != [])
record("33 gate-row-promise: double-underscore emphasis on the noun ('a later __box__') -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a later __box__ | push |\n")) != [])
record("33 gate-row-promise: underscore emphasis on the plural phase noun ('in later _phases_') -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg lands in later _phases_ | push |\n")) != [])
record("33 gate-row-promise: bold-italic on the adjective ('a ***later*** box', three markers a side) -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a ***later*** box | push |\n")) != [])
record("33 gate-row-promise: mixed bold-italic ('a **_later_** box') -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a **_later_** box | push |\n")) != [])
record("33 gate-row-promise: bold-italic on the NOUN ('a later ***box***') -> caught (a leading noun class bounded to two "
       "markers would miss it)",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a later ***box*** | push |\n")) != [])
record("33 gate-row-promise: the article before a BOLD-ITALIC adjective belongs to the phrase ('P4.96 w1 w2 w3 a ***later*** "
       "box': a leading class bounded to two markers re-anchors at `later` and counts the article) -> clean",
       m.doc33_gate_row_promise(_c33("| **G17** | x | P4.96 w1 w2 w3 a ***later*** box | push |\n")) == [])
record("33 gate-row-promise: the article before a BACKTICKED adjective belongs to the phrase ('P4.96 w1 w2 w3 a `later` box': the "
       "id is the 4th word before the phrase; a leading class without the backtick re-anchors at `later` and counts the article) "
       "-> clean",
       m.doc33_gate_row_promise(_c33("| **G17** | x | P4.96 w1 w2 w3 a `later` box | push |\n")) == [])
record("33 gate-row-promise: the article before a PARENTHESISED adjective belongs to the phrase ('P4.96 w1 w2 w3 a (later) box') "
       "-> clean",
       m.doc33_gate_row_promise(_c33("| **G17** | x | P4.96 w1 w2 w3 a (later) box | push |\n")) == [])
record("33 gate-row-promise: underscore bold-italic on the NOUN ('a later ___box___', three trailing underscores) -> caught",
       m.doc33_gate_row_promise(_c33("| **G17** | x | the JS leg is a later ___box___ | push |\n")) != [])
record("33 gate-row-promise: the article `the` belongs to the phrase ('P4.96 w1 w2 w3 the later box': the id is the 4th word "
       "before) -> clean",
       m.doc33_gate_row_promise(_c33("| **G17** | x | P4.96 w1 w2 w3 the later box | push |\n")) == [])
record("33 gate-row-promise: the article `its` belongs to the phrase ('P4.96 w1 w2 w3 its own box') -> clean",
       m.doc33_gate_row_promise(_c33("| **G17** | x | P4.96 w1 w2 w3 its own box | push |\n")) == [])
record("33 gate-row-promise: the article `an` belongs to the phrase ('P4.96 w1 w2 w3 an acquisition box') -> clean",
       m.doc33_gate_row_promise(_c33("| **G17** | x | P4.96 w1 w2 w3 an acquisition box | push |\n")) == [])
record("33 gate-row-promise: generic process prose outside a row / §6 item ('a follow-up box if structural') is NOT scanned",
       m.doc33_gate_row_promise(_c33("Note the residual in the commit body + a follow-up box if structural.\n")) == [])
_bs33, _by33, _pf33 = m.load_plan(ROOT)
record("33 gate-row-promise: the REAL build-gates.md carries no unowned promise (ids resolved against the real plan)",
       m.doc33_gate_row_promise(m.Ctx(root=ROOT, boxes=_bs33, by_id=_by33, plan_files=_pf33, gate_ids=set(), docs={})) == [])

# --- missing target: a doc check over a landed target reports its absence (build-gates §6, G7) ------------------
# Each leg removes exactly one target and expects exactly the finding that names it; the registry leg then pins
# the posture for every check at once, so a new check that passes silently on a missing target reds here.
_SEC, _BGD = "docs/security/security-concept.md", "docs/security/build-gates.md"


def _miss(rel: str, what: str = "") -> str:
    return f"{what or rel} is missing or empty — fail-closed"


def _msgs(findings) -> list[str]:
    return [f.msg for f in findings]


record("missing target: 2 cross-reference - no numbered heading in docs/spec/, docs/security/ or docs/process/ -> caught",
       _msgs(m.doc2_cross_reference(dctx({"docs/a.md": "# x\n"})))
       == [_miss("", "the numbered-heading set of docs/spec/, docs/security/ and docs/process/ (the § targets)")])
record("missing target: 5 gate-catalogue - security-concept.md alone missing, then build-gates.md alone -> each caught",
       _msgs(m.doc5_gate_catalogue(dctx({_BGD: "| **G2** | x |\n"}))) == [_miss(_SEC)]
       and _msgs(m.doc5_gate_catalogue(dctx({_SEC: "uses G2\n"}))) == [_miss(_BGD)])
record("missing target: 8 threat-parity - security-concept.md missing -> caught; build-gates.md missing -> caught beside the row checks",
       _msgs(m.doc8_threat_parity(dctx({_BGD: "| **G2** | x |\n"}))) == [_miss(_SEC)]
       and _miss(_BGD) in _msgs(m.doc8_threat_parity(dctx({_SEC: "| **T1** d | c | G48 |\n"}))))
_CAT = "the build-gates `| **Gnn** |` catalogue"
record("missing target: 11 span-bound / 17 forward-idea / 22 gate-id-gap - a build-gates.md with no Gnn row -> each caught",
       all(_msgs(fn(dctx({_BGD: "# gates, no rows\n"}))) == [_miss(_BGD, _CAT)]
           for fn in (m.doc11_span_bound, m.doc17_forward_idea_status, m.doc22_gate_id_gap_free)))
record("missing target: 16 planted-positive - security-concept.md alone missing, then build-gates.md alone -> each caught",
       _msgs(m.doc16_planted_positive(dctx({_BGD: "| **G2** | x | fail-closed |\n"}))) == [_miss(_SEC)]
       and _msgs(m.doc16_planted_positive(dctx({_SEC: "| **T1** d | c | G2 |\n"}))) == [_miss(_BGD)])
record("missing target: 24 p0-completion - a record without a `run_url:` field -> caught",
       _msgs(m.doc24_p0_completion(dctx({"docs/process/p0-completion.md": "# P0 record\n\ndate: 2026-01-01\n"})))
       == [_miss("", "the `run_url:` field of docs/process/p0-completion.md")])
record("missing target: 25 doc-graph - no build-gates catalogue for the gate-name freshness axis -> caught",
       _miss(_BGD) in _msgs(m.doc25_doc_graph(dctx({"docs/a.md": "names G7\n"}))))
with tempfile.TemporaryDirectory() as _tmt:
    _rmt = Path(_tmt)

    def _cmt(docs=None):
        return m.Ctx(root=_rmt, boxes=[], by_id={}, plan_files=[], docs=docs or {}, gate_ids=set())

    record("missing target: 12 ipc-surface - no src-tauri/src and no golden -> both caught",
           _msgs(m.doc12_ipc_surface_drift(_cmt())) == [_miss("src-tauri/src"), _miss("src-tauri/ipc-commands.golden")])
    record("missing target: 13 plugin-surface - no Cargo.lock and no src-tauri/src -> both caught",
           _msgs(m.doc13_plugin_surface_drift(_cmt())) == [_miss("Cargo.lock"), _miss("src-tauri/src")])
    record("missing target: 28 app-event - no src-tauri/src -> caught",
           _msgs(m.doc28_app_event_surface_drift(_cmt())) == [_miss("src-tauri/src")])
    record("missing target: 29 stale-liveness - each of the three scope roots missing -> caught",
           _msgs(m.doc29_stale_liveness(_cmt())) == [_miss("src-tauri/src"), _miss("crates"), _miss("xtask/src")])
    record("missing target: 27 posture - no gate-planes.toml -> caught",
           _msgs(m.doc27_gate_posture_transition(_cmt())) == [_miss("scripts/gate-planes.toml")])
    record("missing target: 33 gate-row-promise - no build-gates.md -> caught",
           _msgs(m.doc33_gate_row_promise(_cmt())) == [_miss(_BGD)])
    record("missing target: 10 manifest-currency - no ffmpeg-*.lock -> NO finding (the one target-absent check, P6.4/P6.5)",
           m.doc10_manifest_currency(_cmt()) == [])
    # the posture over the whole registry: an empty root with no docs and no boxes leaves exactly the documented
    # no-target set silent - 1 (registered no-op), 3/4/6/7/9 (scope scans), 10 (target-absent), 31 (the plan,
    # whose absence exits 2 in main) - and every other doc check reports a missing target
    _silent = {cid for cid, fn in m.DOC_CHECKS.items() if not fn(_cmt())}
    record("missing target: registry - on an empty root only checks 1, 3, 4, 6, 7, 9, 10 and 31 report nothing",
           _silent == {"1:matrix-parity", "3:heading-hierarchy", "4:numbering-gap-free", "6:forbidden-tokens",
                       "7:generated-file-sanity", "9:inventory-parity", "10:manifest-currency", "31:sweep-binding"})
    (_rmt / "scripts").mkdir()
    (_rmt / "scripts" / "gate-planes.toml").write_text('[[fail_open]]\ngate = "G71"\n', encoding="utf-8")
    record("missing target: 27 posture - a gate-planes.toml without a [[posture_flag]] row -> caught",
           _msgs(m.doc27_gate_posture_transition(_cmt()))
           == [_miss("scripts/gate-planes.toml", "the [[posture_flag]] registry in scripts/gate-planes.toml")])

failed = [n for n, ok in results if not ok]
print(f"\n[g24-plan-lint] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
