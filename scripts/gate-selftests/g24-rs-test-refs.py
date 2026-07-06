#!/usr/bin/env python3
"""g24-rs-test-refs.py - G24 self-test for check-rs-test-refs (P2.136, G73).

Proves the `.rs` contract-test-reference resolver (the `.rs` analogue of the G68 doc-graph `.md`
cross-resolution net): a `…_contract` / `…_contract_is_invocable_and_typed` reference in an `ipc/`
COMMENT that names a `mod`/`fn` defined nowhere under `src-tauri/src/ipc/**` FAILS CLOSED; a resolving
reference passes; a rename that strands a reference reds; ordinary `::` code paths and string/identifier
tokens are ignored; and the `*_contract` family GLOB is NOT tokenised (the load-bearing planning.rs FP).
Includes the LIVE real-repo regression leg (every current ipc/** reference resolves). stdlib-only.
Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import sys
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-rs-test-refs"
_loader = importlib.machinery.SourceFileLoader("crtr", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("crtr", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def bad(*texts: str) -> list[str]:
    """Unresolved reference tokens across the pooled texts (defs collected tree-wide)."""
    return m.unresolved_in_texts(list(texts))


# --- (a) planted-positive: a comment naming an UNDEFINED module reds -------------------------------
record("(a) planted-positive: comment ref `c99_contract` with no such mod defined -> FAIL-CLOSED",
       bad("#[cfg(test)]\nmod c1_contract {}\n// mirrors c99_contract\n") == ["c99_contract"])

# --- (b) rename-strand: a resolving ref, then the mod renamed -> the ref reds ---------------------
record("(b) rename-strand: `c3_contract` ref resolves while the mod exists",
       bad("mod c3_contract {}\n/// mirrors c3_contract\n") == [])
record("(b) rename-strand: rename the mod to c3_renamed, the comment ref `c3_contract` now reds",
       bad("mod c3_renamed {}\n/// mirrors c3_contract\n") == ["c3_contract"])

# --- (c) negative all-resolve --------------------------------------------------------------------
record("(c) all-resolve: `c1_contract`/`c2a_contract` refs to defined mods -> pass",
       bad("mod c1_contract {}\nmod c2a_contract {}\n/// mirrors the c1_contract / c2a_contract helpers\n") == [])

# --- (d) discriminator: ordinary `::` code paths are ignored --------------------------------------
record("(d) discriminator: `crate::orchestrator` / `serde_json::from_value` in a comment are NOT refs",
       bad("mod c1_contract {}\n// delegates to crate::orchestrator and serde_json::from_value; mirrors c1_contract\n") == [])

# --- (e) comment-only: a `_contract` token in a STRING literal is not scanned ---------------------
record("(e) comment-only: an unresolved `_contract` token inside a string literal is ignored",
       bad('mod c1_contract {}\nlet s = "undefinedxyz_contract";\n// ref c1_contract\n') == [])
record("(e) comment-only: a production `fn helper_contract()` def is not itself a reference",
       bad("fn helper_contract() {}\nmod c1_contract {}\n// ref c1_contract\n") == [])

# --- (f) GLOB-guard: the `*_contract` family glob is NOT tokenised (planning.rs:159 replica) ------
record("(f) GLOB-guard: a `//!` comment with the family glob `*_contract` alone -> no ref, pass",
       bad("mod c1_contract {}\n//! Mirrors the C1/C2a `*_contract` tests\n") == [])
record("(f) GLOB-guard: the glob does not strand a co-located resolving ref",
       bad("mod c1_contract {}\n//! Mirrors the `*_contract` tests; see c1_contract\n") == [])

# --- (g) module-qualified resolution uses the FINAL segment ---------------------------------------
record("(g) module-qualified negative: `intake::c99_contract` tail is undefined -> FAIL-CLOSED",
       bad("mod c2a_contract {}\n// pinned by intake::c99_contract\n") == ["intake::c99_contract"])
record("(g') module-qualified positive: `intake::c2a_contract` tail resolves -> pass",
       bad("mod c2a_contract {}\n// pinned by intake::c2a_contract\n") == [])
record("(g'') deep path: `crate::ipc::intake::c2a_contract` resolves on its final segment",
       bad("mod c2a_contract {}\n// pinned by crate::ipc::intake::c2a_contract\n") == [])

# --- (h) long-form fn ref: the `_is_invocable_and_typed` arm is captured whole --------------------
record("(h) long-form fn ref resolves while the fn exists",
       bad("fn c6_start_conversion_contract_is_invocable_and_typed() {}\n// see c6_start_conversion_contract_is_invocable_and_typed\n") == [])
record("(h) long-form fn ref reds when the fn is renamed (the `_is_invocable_and_typed` arm matched)",
       bad("fn c6_renamed() {}\n// see c6_start_conversion_contract_is_invocable_and_typed\n")
       == ["c6_start_conversion_contract_is_invocable_and_typed"])

# --- (i) cross-file pooling: a def in file A resolves a ref in file B ------------------------------
record("(i) cross-file: `c3_contract` defined in file A resolves a ref in file B (tree-wide union)",
       bad("mod c3_contract {}\n", "// mirrors c3_contract\n") == [])

# --- (j) block-comment body (no leading `*`) is extracted (stateful) -------------------------------
record("(j) block-comment body: an undefined ref inside a /* … */ body reds (stateful extraction)",
       bad("mod c1_contract {}\n/*\n mirrors cNN_contract here\n*/\n") == ["cNN_contract"])

# --- (k) DEF_RE captures every visibility/async form ----------------------------------------------
record("(k) DEF_RE: `pub async fn foo_contract()` is collected as a def -> a ref to it resolves",
       bad("pub async fn foo_contract() {}\n// ref foo_contract\n") == [])
record("(k) DEF_RE: `pub(crate) mod bar_contract {}` is collected as a def",
       bad("pub(crate) mod bar_contract {}\n// ref bar_contract\n") == [])

# --- (l) prose plural is not a false-positive -----------------------------------------------------
record("(l) plural: `_contracts` prose is not a reference (trailing non-word-boundary guard)",
       bad("mod c1_contract {}\n// the per-command _contracts live here; see c1_contract\n") == [])

# --- (n)/(o) phantom-masking guard: a `fn`/`mod` in a comment/string body must NOT mask a strand ---
# (the G1 dual-review fail-open fix — DEF_RE now runs over the _strip_rust'd code projection, so
# commented-out or string-embedded code cannot forge a definition that resolves a stranded reference)
record("(n) block-comment phantom: a `fn` in commented-out /* */ code is NOT a def, so the strand STILL reds",
       "c99_contract" in bad("mod real_thing {}\n/// stranded ref c99_contract\n/*\nfn c99_contract() {}\n*/\n"))
record("(o) raw-string phantom: a strand whose name reappears inside a r#\"…\"# body STILL reds",
       bad('mod real_thing {}\n/// stranded ref c99_contract\nlet s = r#"\nfn c99_contract() {}\n"#;\n') == ["c99_contract"])
record("(o') plain-string phantom: a strand whose name reappears at line-start inside a multi-line \"…\" body STILL reds",
       bad('mod real_thing {}\n/// stranded ref c99_contract\nconst NEEDLE: &str = "\nfn c99_contract() {}\n";\n') == ["c99_contract"])

# --- (m) LIVE real-repo regression guard ----------------------------------------------------------
record("(m) LIVE: check-rs-test-refs passes on the real repo (every ipc/** ref resolves; the glob excluded)",
       m.main([]) == 0)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-rs-test-refs] {len(results) - len(failed)}/{len(results)} assertions passed (G73).")
sys.exit(1 if failed else 0)
