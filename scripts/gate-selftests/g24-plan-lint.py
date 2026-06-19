#!/usr/bin/env python3
"""g24-plan-lint.py - G24 self-test for plan-lint (P0.3.5, G7/G20).

FORMAT-check coverage: for each of the 8 format checks, a CLEAN box yields no finding and a VIOLATING
box IS flagged (so no check is green-by-vacuity). Plus the base-case golden invariant: the real plan
passes (exit 0) and a deliberately-broken synthetic box-set exits non-empty. The doc-wide checks 1..26
get their own legs as they are built. stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import hashlib
import importlib.machinery
import importlib.util
import subprocess
import sys
import tempfile
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

# --- check 23: the owner-decidable / informational-then-ratcheted gate-status ledger (P0.4.5) --
_GS = "docs/process/gate-status.md"
_LEDGER_HEAD = "| Gate / tool | Status | Since | Activation | Contract |\n|---|---|---|---|---|\n"


def _ledger(rows):                                   # rows: list of (name, status, since)
    body = "".join(f"| {n} | {s} | {d} | P1 | x |\n" for (n, s, d) in rows)
    return {_GS: "# Ledger doc\n\n## Ledger\n\n" + _LEDGER_HEAD + body}


_SEEDED = [("`cargo-acl`/cackle", "informational", "2026-06-18"),
           ("`cargo-careful`", "informational", "2026-06-18"),
           ("Kani", "informational", "2026-06-18"),
           ("`cargo-geiger`", "informational", "2026-06-18"),
           ("`cargo-mutants`", "informational", "2026-06-19"),   # P0.5.10 — the G15 mutation sub-leg
           ("`G17b`", "informational", "2026-06-19"),            # P0.7.7 — bundled-engine CVE awareness
           ("`G64`", "informational", "2026-06-19"),             # P0.7.14 — privilege-drop-tier ratchet
           ("`G65`", "informational", "2026-06-19")]             # P0.7.15 — engine-subprocess coverage-guided fuzz
record("23 gate-status: a clean 8-row ledger (all registered gates) -> no finding",
       m.doc23_ratchet_log(dctx(_ledger(_SEEDED))) == [])
record("23 gate-status: absent ledger -> skip (target-absent, not a finding)",
       m.doc23_ratchet_log(dctx({})) == [])
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
              "SPEC-CONTRADICTION is a finding CLASS ABOVE P0.\n```\n")
record("_extract_fenced_block: returns the marked block's body",
       (m._extract_fenced_block("a\n```text\nMARK here\nbody line\n```\nb\n", "MARK here") or "").strip().endswith("body line"))
record("_extract_fenced_block: None when no fenced block carries the marker",
       m._extract_fenced_block("```\nunrelated\n```\n", "MARK here") is None)
record("19 reviewer-rubric: a complete fenced rubric block -> no finding",
       m.doc19_reviewer_rubric(dctx({_BL: _RUBRIC_OK})) == [])
record("19 reviewer-rubric: absent build-loop.md -> skip (target-absent, not a finding)",
       m.doc19_reviewer_rubric(dctx({})) == [])
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
record("19 reviewer-rubric: the REAL committed build-loop.md rubric passes (no finding)",
       m.doc19_reviewer_rubric(m.build_ctx(ROOT)) == [])

# --- check 20: the reviewer-family owner decision + spot-audit cadence (P0.6.3) ----------------
_FAM_OK = ("# Build-Loop\n\nRecorded reviewer-family decision: the two reviewers share model lineage, so\n"
           "the correlated-lineage residual is explicitly ACCEPTED for v1 (the deterministic gates carry\n"
           "the real security weight), WITH a Co-Pilot spot-audit at every phase boundary AND a random\n"
           "1-in-10-box sample; the flip option remains open.\n")
record("20 reviewer-family: a build-loop.md with the full decision + cadence -> no finding",
       m.doc20_reviewer_family(dctx({_BL: _FAM_OK})) == [])
record("20 reviewer-family: absent build-loop.md -> skip (target-absent, not a finding)",
       m.doc20_reviewer_family(dctx({})) == [])
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
# integration: aligned -> clean; each source drifting -> caught; target-absent -> skip; real docs pass
record("14 dod-parity: three aligned copies -> no finding", m.doc14_dod_parity(_c14()) == [])
record("14 dod-parity: absent build-loop.md -> skip (target-absent, not a finding)",
       m.doc14_dod_parity(_c14(with_bl=False)) == [])
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

# --- check 15: the operator-anchored hard-stop / Notbremse thresholds in build-loop.md §6 (P0.6.7) ------
# Each canonical string is an EXACT integer + EXPLICIT operator; reverting any one to a fuzzy prose form
# (~8 / ~12 / ~5 / "3 consecutive gate-red pushes") drops the verbatim match and is CAUGHT — so the loop
# cannot silently run on a threshold it could misread.
_HS_OK = ("# Build-Loop\n\n## 6. Hard-stops\n\n"
          "- soft-stop fires when committed-box-count >= 8 in one session.\n"
          "- hard-stop at == 12 committed boxes in one session.\n"
          "- cluster soft-stop at >= 5 committed boxes since the last soft-stop.\n"
          "- >= 3 consecutive push failures = hard-stop + escalate.\n")
record("15 hard-stop: a build-loop.md with all four operator-anchored thresholds -> no finding",
       m.doc15_hard_stop_parity(dctx({_BL: _HS_OK})) == [])
record("15 hard-stop: absent build-loop.md -> skip (target-absent, not a finding)",
       m.doc15_hard_stop_parity(dctx({})) == [])
record("15 hard-stop: the soft-stop >= 8 string reverted to the '~8' prose form -> caught",
       any(">= 8" in f.msg for f in m.doc15_hard_stop_parity(
           dctx({_BL: _HS_OK.replace("soft-stop fires when committed-box-count >= 8", "soft-stop ~8 boxes")}))))
record("15 hard-stop: the hard-stop == 12 string reverted to '~12' -> caught",
       any("== 12" in f.msg for f in m.doc15_hard_stop_parity(
           dctx({_BL: _HS_OK.replace("hard-stop at == 12", "hard-stop ~12 boxes")}))))
record("15 hard-stop: the cluster >= 5 string reverted to '~5' -> caught",
       any(">= 5" in f.msg for f in m.doc15_hard_stop_parity(
           dctx({_BL: _HS_OK.replace("cluster soft-stop at >= 5 committed boxes", "cluster soft-stop at ~5 boxes")}))))
record("15 hard-stop: the >= 3 push-failures string reverted to 'gate-red pushes' -> caught",
       any(">= 3 consecutive push failures" in f.msg for f in m.doc15_hard_stop_parity(
           dctx({_BL: _HS_OK.replace(">= 3 consecutive push failures", "3 consecutive gate-red pushes")}))))
record("15 hard-stop: the REAL committed build-loop.md §6 thresholds pass (no finding)",
       m.doc15_hard_stop_parity(m.build_ctx(ROOT)) == [])

# --- check 18: the two named build-loop procedures present verbatim in build-loop.md (P0.6.8) ----------
# Each procedure (crash-recovery §9, divergence-resolution §3 Step 5) is pinned by its header + its
# load-bearing sub-rules; dropping ANY canonical phrase (gutting a procedure to a bare header, or removing
# its core rule) is CAUGHT — so "currently absent" cannot silently survive into the docs (build-gates §6
# check 18). gate-quarantine + suppression-ledger are authored in §6 but lie outside check 18's named set.
_NP_OK = ("# Build-Loop\n\n## 3. The loop\n\n"
          "Divergence-resolution rule (canonical): a P0/P1 GO-vs-NOGO is NOGO — the stricter reviewer wins.\n\n"
          "## 9. Crash-recovery\n\n"
          "Crash-recovery procedure (a mid-box crash is recoverable):\n"
          "- Committed-but-CI-red -> a NEW commit fixing it; never amend a pushed commit.\n"
          "- Push is idempotent on retry — a re-push is a safe no-op.\n")
record("18 named-proc: a build-loop.md with both procedures + their sub-rules -> no finding",
       m.doc18_named_procedure(dctx({_BL: _NP_OK})) == [])
record("18 named-proc: absent build-loop.md -> skip (target-absent, not a finding)",
       m.doc18_named_procedure(dctx({})) == [])
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

# dormancy works BOTH ways (skip while the describing doc is unauthored; activate once it is)
_DORM = {"id": "d", "kind": "file", "file": _DOC, "fingerprint": "sha256:" + "0" * 64,  # deliberately WRONG
         "dormant": {"file": "CLAUDE.md", "contains": "P1.64"}}
record("25-fp: a dormant entry (marker present) is skipped even with a wrong fingerprint",
       _ff([_DORM], {_DOC: _CONTENT, "CLAUDE.md": "... finalized by the P1.64 box ..."}) == [])
record("25-fp: the SAME entry is NOT skipped once the dormancy marker is gone (then caught)",
       any("'d'" in f.msg for f in _ff([_DORM], {_DOC: _CONTENT, "CLAUDE.md": "no marker here\n"})))

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
# P2: dropping a required seed (only l-neg1-cage present, spec-0.7 missing) FAILS closed
_e2, _f2 = _read_ledger_body(f'[[source]]\nid = "l-neg1-cage"\nkind = "file"\nfile = "x"\nfingerprint = "sha256:{_E64}"\n')
record("25-fp: a ledger missing a required seed id fails closed (names the missing id)",
       _f2 is not None and "spec-0.7-physical-tree" in _f2.msg)
# P3: a plain string-array `source = [...]` fails closed CLEANLY (no AttributeError crash)
try:
    _e3, _f3 = _read_ledger_body('source = ["a", "b"]\n')
    _ok3 = _f3 is not None and "array of tables" in _f3.msg
except Exception:
    _ok3 = False
record("25-fp: a plain-array (non-array-of-tables) ledger fails closed, does NOT crash", _ok3)
# the floor is SATISFIED by a minimal ledger carrying both required ids (not green-by-over-strictness)
_e4, _f4 = _read_ledger_body(
    f'[[source]]\nid = "l-neg1-cage"\nkind = "file"\nfile = "x"\nfingerprint = "sha256:{_E64}"\n'
    f'[[source]]\nid = "spec-0.7-physical-tree"\nkind = "file"\nfile = "y"\nfingerprint = "sha256:{_E64}"\n')
record("25-fp: a ledger carrying every required seed id passes the floor (no fatal)", _f4 is None)
# P3: list `contains` is the exact G69 OR-dormancy — dormant if ANY marker present, active if NONE
_DL = {"id": "spec-0.7-physical-tree", "kind": "file", "file": _DOC, "fingerprint": "sha256:" + _E64,
       "dormant": {"file": "CLAUDE.md", "contains": ["PLACEHOLDER", "P1.64"]}}
record("25-fp: list `contains` is dormant when ANY marker is present (PLACEHOLDER alone)",
       _ff([_DL], {_DOC: _CONTENT, "CLAUDE.md": "this is a PLACEHOLDER stub\n"}) == [])
record("25-fp: list `contains` is ACTIVE (caught) when NO marker is present",
       any("'spec-0.7" in f.msg for f in _ff([_DL], {_DOC: _CONTENT, "CLAUDE.md": "real finalized map\n"})))
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
# the target-absent stubs all skip today (their P0.6/P1 targets are unauthored). NB check 23 is NO LONGER
# here: P0.4.5 created docs/process/gate-status.md, so doc23 is now ACTIVE and is exercised by its own
# dedicated legs above (the gate-status block) — keeping it in this "stubs skip" tuple would pass for the
# wrong reason (the real ledger is clean, the active path) under a misleading label (P0.4.5 G1 P2 fix).
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
record("24 p0-completion: absent p0-completion.md -> skip (target-absent, not a finding)",
       m.doc24_p0_completion(dctx({})) == [])
record("24 p0-completion: a non-URL placeholder token -> caught",
       any("does not match" in f.msg for f in m.doc24_p0_completion(
           dctx({_PC: _PC_OK.replace("https://github.com/Ne-IA/convertia/actions/runs/0", "<pending-at-exit>")}))))
record("24 p0-completion: a filled real run id passes",
       m.doc24_p0_completion(dctx({_PC: _PC_OK.replace("runs/0", "runs/27820505219")})) == [])
record("24 p0-completion: the REAL committed p0-completion.md stub passes (born-green)",
       m.doc24_p0_completion(_real) == [])
# With p0-completion.md authored (P0.6.10), doc24 is now ACTIVE too — so ALL SIX P0.6/P0.3.5 doc-checks
# (14/15/18/19/20/24) are live with dedicated real-doc legs and NONE remains a target-absent skip under a
# misleading label. The only target-absent skips that survive are the per-check synthetic dctx({}) probes
# (each paired with its real-doc leg), never a real check passing for the wrong reason — the P0.4.5-G1-P2
# principle (see check 23 above), now fully discharged across the P0.6 activations.

# --- check 26 (G69) structural-map integrity — the real logic, driven by pure fns (P0.3.13) ------
# doc26 SKIPS while the §1a map is the P1.64 placeholder, so the active path is exercised via the pure
# parser/relations fns + one synthetic-active doc26 run against the real repo tree.
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

# the 3 relations (pure, set-only)
record("26 rel: map==disk==§0.7 projection is clean",
       m._struct_map_relations({"docs", "src"}, {"docs", "src"}, {"docs", "src", "extra"}) == [])
record("26 rel: an on-disk dir absent from the map is caught (folder without a map row)",
       ("disk-not-in-map", "newdir") in m._struct_map_relations({"docs"}, {"docs", "newdir"}, {"docs", "newdir"}))
record("26 rel: a mapped dir not on disk is caught (stale map entry)",
       ("map-not-on-disk", "gone") in m._struct_map_relations({"docs", "gone"}, {"docs"}, {"docs", "gone"}))
record("26 rel: a §1a dir the §0.7 tree does not home is caught (projection bind)",
       ("map-not-in-spec07", "invented") in m._struct_map_relations({"docs", "invented"}, {"docs", "invented"}, {"docs"}))

# _dirs_from_files: every ancestor of a tracked path, minus the out-of-scope trees
record("26 disk: ancestor dirs derived from tracked-file paths (a, a/b, scripts)",
       m._dirs_from_files(["a/b/c.txt", "a/d.txt", "scripts/x", "top.md"]) == {"a", "a/b", "scripts"})
record("26 disk: the out-of-scope trees (target/node_modules/dist/.git) are excluded",
       m._dirs_from_files(["target/x/y", "node_modules/p/i.js", "dist/a", ".git/z", "src/m.rs"]) == {"src"})

# _fenced_block_after: pulls the block under the named header, None when no fence
_DOCT = "## 1a Map\n\n```\nconvertia/\n├── docs/\n```\n\n## 2 Next\n"
record("26 fence: extracts the fenced block under the §1a header",
       m._fenced_block_after(_DOCT, __import__("re").compile(r"^#{1,6}\s+1a\b")) == ["convertia/", "├── docs/"])
record("26 fence: returns None when the header has no following fence",
       m._fenced_block_after("## 1a\n\njust prose, no fence\n", __import__("re").compile(r"1a")) is None)

# the skip signal is 'PLACEHOLDER' WITHIN the §1a section (not a whole-file scan) — G1 r1 hardening
record("26 skip: the real repo skips today (the §1a section still carries 'PLACEHOLDER')",
       m.doc26_struct_map(_real) == [])
record("26 skip: a synthetic §1a section carrying 'PLACEHOLDER' skips even with a broken map",
       m.doc26_struct_map(m.Ctx(root=ROOT, boxes=[], by_id={}, plan_files=[],
                                docs={"CLAUDE.md": "## 1a Map\n\n> PLACEHOLDER stub\n\n```\nbogus/\n```\n"})) == [])
record("26 skip: a bare 'P1.64' provenance mention with NO 'PLACEHOLDER' does NOT keep it dormant",
       m.doc26_struct_map(m.Ctx(root=ROOT, boxes=[], by_id={}, plan_files=[],
                                docs={"CLAUDE.md": "## 1a Map\n\nsee [P1.64](x)\n\n```\nconvertia/\n└── zzz/\n```\n",
                                      "docs/spec/00-architecture.md": "### Physical tree\n```\nconvertia/\n└── zzz/\n```\n"})) != [])
# doc26 ACTIVE end-to-end against the real tree: BOTH directions exercised through real git ls-files
_active_claude = ("## 1a Repo layout\n\n```\nconvertia/\n├── docs/\n├── zzz-bogus/\n```\n\n## 2 x\n")
_active = m.Ctx(root=ROOT, boxes=[], by_id={}, plan_files=[],
                docs={"CLAUDE.md": _active_claude,
                      "docs/spec/00-architecture.md": m.load_docs(ROOT).get("docs/spec/00-architecture.md", "")})
_af = m.doc26_struct_map(_active)
record("26 active: a map-only bogus dir is flagged (map->disk + map->§0.7 via find+parse+git+rel)",
       any("zzz-bogus" in f.msg for f in _af))
record("26 active: a real on-disk dir absent from the map IS flagged disk-not-in-map (disk->map via real git)",
       any("scripts/gate-selftests" in f.msg and "absent from" in f.msg for f in _af))

# active fail-CLOSED branches once the §1a section is placeholder-free (never a silent [])
def _doc26(claude_text, arch_text=""):
    return m.doc26_struct_map(m.Ctx(root=ROOT, boxes=[], by_id={}, plan_files=[],
                                    docs={"CLAUDE.md": claude_text, "docs/spec/00-architecture.md": arch_text}))
record("26 fail-closed: an active §1a with NO map fence -> Finding (not silent [])",
       any("fenced block is absent" in f.msg for f in _doc26("## 1a Map\n\njust prose, no fence\n\n## 2 x\n")))
record("26 fail-closed: an active map but a missing §0.7 Physical tree block -> Finding",
       any("Physical tree" in f.msg for f in _doc26("## 1a\n```\nconvertia/\n└── docs/\n```\n", "no physical tree here\n")))

# G1 r1 parser hardening: annotation prose / sibling embedded-paths cannot inject phantom dirs
record("26 parse: a slash-word in an arrow/hash annotation is NOT mined as a dir (false-negative closed)",
       m._parse_tree_dirs(["convertia/", "└── docs/   → moved to src-tauri/src/ipc"]) == {"docs"})
record("26 parse: a same-line sibling must be a SIMPLE dir — an embedded-path sibling is not mined",
       m._parse_tree_dirs(["convertia/", "├── a/  lib/ipc/"]) == {"a"})
record("26 parse: a hash-annotation slash-word is not mined either (§0.7 # convention)",
       m._parse_tree_dirs(["convertia/", "└── src/   # see config/secrets/ for details"]) == {"src"})

# --- base-case golden invariant ---------------------------------------------------------------
rc_real = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True, text=True).returncode
record("base-case: the REAL plan passes the format checks (exit 0)", rc_real == 0)
broken = ctx([box(bid="P0.1", raw="X", tags=["NOPE"], title="bad.", refs="")])
record("base-case: a deliberately-broken box yields findings (would exit 1)", len(m.run(broken)) >= 1)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-plan-lint] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
