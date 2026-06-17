#!/usr/bin/env python3
"""g24-plan-lint.py - G24 self-test for plan-lint (P0.3.5, G7/G20).

FORMAT-check coverage: for each of the 8 format checks, a CLEAN box yields no finding and a VIOLATING
box IS flagged (so no check is green-by-vacuity). Plus the base-case golden invariant: the real plan
passes (exit 0) and a deliberately-broken synthetic box-set exits non-empty. The doc-wide checks 1..26
get their own legs as they are built. stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import subprocess
import sys
from pathlib import Path

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
        refs="G7", needs=None, unlocked_by=None, notes=None):
    phase = int(bid[1:].split(".")[0])
    num = tuple(int(x) for x in bid[1:].split(".")[1:])
    return m.Box(box_id=bid, phase=phase, num=num, marker=(raw if raw is not None else marker),
                 raw_marker=(raw if raw is not None else marker), indent=indent,
                 tags=tags if tags is not None else ["GATE"], title=title, refs=refs,
                 file="docs/plan/Px.md", lineno=1, needs=needs or [], unlocked_by=unlocked_by or [],
                 notes=notes or [])


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

# --- needs-targets + acyclic ------------------------------------------------------------------
record("needs: a resolvable target clean",
       m.fmt_needs_targets(ctx([box(bid="P0.1"), box(bid="P0.2", needs=["P0.1"])])) == [])
record("needs: a dangling target flagged",
       any("no such box" in f.msg for f in m.fmt_needs_targets(ctx([box(bid="P0.2", needs=["P9.9"])]))))
record("needs: a 2-cycle flagged",
       any("cycle" in f.msg for f in m.fmt_needs_targets(ctx([box(bid="P0.1", needs=["P0.2"]), box(bid="P0.2", needs=["P0.1"])]))))

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
record("25 forward-allowlist: a dangling link to a registered forward-target -> NOT a dangling finding",
       not any("dangling" in f.msg for f in m.doc25_doc_graph(
           dctx({"docs/spec/README.md": "[v](../process/vuln-response.md)\n"}))))
record("25 forward-allowlist: a dangling link to an UNREGISTERED target -> still caught (no cue evasion)",
       any("dangling" in f.msg for f in m.doc25_doc_graph(
           dctx({"docs/spec/README.md": "[x](gone.md) — but this is planned for later\n"}))))
record("25 code-span: a link inside an inline `code` span -> NOT flagged",
       not any("dangling" in f.msg for f in m.doc25_doc_graph(dctx({"docs/a.md": "example `[x](nope.md)` here\n"}))))
record("8 threat-parity: a §5 row citing a non-catalogue gate -> caught",
       any("G9999" in f.msg for f in m.doc8_threat_parity(
           dctx({"docs/security/security-concept.md": "| **T1** d | c | G9999 |\n",
                 "docs/security/build-gates.md": "| **G2** | x |\n"}))))

# checks reading the filesystem (16/21): exercise on the REAL repo (built ctx) — both clean today
_real = m.build_ctx(ROOT)
record("16 planted-positive: clean on the real repo (every built fail-closed §5 gate self-tested)",
       m.doc16_planted_positive(_real) == [])
record("21 t2-taint-xor: pending (neither CodeQL nor Semgrep live) -> skip []",
       m.doc21_taint_xor(_real) == [])
# the 13 target-absent stubs all skip today (their P0.6/P1 targets are unauthored)
record("target-absent stubs (14/15/18/19/20/23/24/26) skip while their targets are absent",
       all(fn(_real) == [] for fn in (m.doc14_dod_parity, m.doc15_hard_stop_parity, m.doc18_named_procedure,
                                      m.doc19_reviewer_rubric, m.doc20_reviewer_family, m.doc23_ratchet_log,
                                      m.doc24_p0_completion, m.doc26_struct_map)))

# --- base-case golden invariant ---------------------------------------------------------------
rc_real = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True, text=True).returncode
record("base-case: the REAL plan passes the format checks (exit 0)", rc_real == 0)
broken = ctx([box(bid="P0.1", raw="X", tags=["NOPE"], title="bad.", refs="")])
record("base-case: a deliberately-broken box yields findings (would exit 1)", len(m.run(broken)) >= 1)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-plan-lint] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
