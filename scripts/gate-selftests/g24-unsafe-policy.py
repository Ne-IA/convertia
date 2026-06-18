#!/usr/bin/env python3
"""g24-unsafe-policy.py - G24 self-test for check-unsafe-policy (P0.4.2, G29).

Proves the unsafe-policy gate (1) freezes a narrow allow-list (an empty / catch-all allow-list is
rejected), (2) skips target-absent when no crate root exists, and (3) on a synthetic crate tree
CATCHES every violation class: a crate root missing `#![deny(unsafe_code)]`, an `allow(unsafe_code)`
outside the allow-listed FFI module, a `#![forbid(unsafe_code)]` crate that still has an allow, two
allow modules in one crate, and an `unsafe` block without a `// SAFETY:` justification - while the
clean tree passes. A commented/stringed allow must NOT count. stdlib-only. Exit 0 = all held.
"""
import importlib.machinery
import importlib.util
import shutil
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-unsafe-policy"
_loader = importlib.machinery.SourceFileLoader("cup", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cup", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def build(tree: dict) -> Path:
    """Materialise {repo-relative-path: contents} into a fresh temp dir; return it."""
    td = Path(tempfile.mkdtemp(prefix="g24-unsafe-"))
    for rel, body in tree.items():
        p = td / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(body, encoding="utf-8")
    return td


def scan(tree: dict) -> int:
    """Run the live scan against a synthetic repo (patching the module ROOT; temp dir cleaned up)."""
    saved = m.ROOT
    td = build(tree)
    try:
        m.ROOT = td
        return m.scan_live()
    finally:
        m.ROOT = saved
        shutil.rmtree(td, ignore_errors=True)


def crate_roots(tree: dict) -> list:
    saved = m.ROOT
    td = build(tree)
    try:
        m.ROOT = td
        return m._crate_roots()
    finally:
        m.ROOT = saved
        shutil.rmtree(td, ignore_errors=True)


CARGO = '[package]\nname = "x"\n'
DENY = "#![deny(unsafe_code)]\n"

# --- freeze_contract: the real allow-list passes; empty + catch-all are rejected --------------
record("freeze: the real committed ALLOWED_UNSAFE_MODULES passes", m.freeze_contract() == 0)

_saved = m.ALLOWED_UNSAFE_MODULES
try:
    m.ALLOWED_UNSAFE_MODULES = []
    record("freeze: an empty allow-list -> exit-2 signal", m.freeze_contract() == 2)
    m.ALLOWED_UNSAFE_MODULES = ["**/*.rs"]
    record("freeze: a catch-all `**/*.rs` (matches main.rs) is rejected", m.freeze_contract() == 2)
    m.ALLOWED_UNSAFE_MODULES = ["src-tauri/**/*.rs"]
    record("freeze: a crate-wide `src-tauri/**/*.rs` is rejected (covers the core)", m.freeze_contract() == 2)
    m.ALLOWED_UNSAFE_MODULES = ["crates/imgworker/**/*.rs"]
    record("freeze: a crate-wide `crates/imgworker/**/*.rs` is rejected (covers the densest-unsafe crate)",
           m.freeze_contract() == 2)
finally:
    m.ALLOWED_UNSAFE_MODULES = _saved

# --- _is_allow_listed: the FFI module yes, a core module no -----------------------------------
record("allow-listed: src-tauri/src/platform/os.rs is allowed", m._is_allow_listed("src-tauri/src/platform/os.rs"))
record("allow-listed: imgworker ffi.rs is allowed", m._is_allow_listed("crates/imgworker/src/ffi.rs"))
record("allow-listed: a core module (ipc/mod.rs) is NOT allowed", not m._is_allow_listed("src-tauri/src/ipc/mod.rs"))

# --- target-absent: empty repo -> no crate roots ----------------------------------------------
record("target-absent: an empty repo has no crate roots", crate_roots({}) == [])

# --- PASS: deny at root, allow only in platform with SAFETY, no other unsafe ------------------
PASS_TREE = {
    "src-tauri/Cargo.toml": CARGO,
    "src-tauri/src/main.rs": DENY + "fn main() {}\n",
    "src-tauri/src/platform/os.rs": (
        "#![allow(unsafe_code)]\n"
        "pub fn rename_atomic() {\n"
        "    // SAFETY: renameat2 is called with valid, owned fds per §2.1.\n"
        "    unsafe { libc_renameat2(); }\n"
        "}\n"
    ),
    "src-tauri/src/ipc/mod.rs": "pub fn handler() {}\n",
}
record("PASS: deny-at-root + allow only in platform (with SAFETY) -> 0 problems", scan(PASS_TREE) == 0)

# --- FAIL: crate root missing deny ------------------------------------------------------------
T = dict(PASS_TREE); T["src-tauri/src/main.rs"] = "fn main() {}\n"
record("FAIL: crate root missing #![deny(unsafe_code)] is caught", scan(T) >= 1)

# --- FAIL: allow(unsafe_code) outside the allow-listed module ---------------------------------
T = dict(PASS_TREE); T["src-tauri/src/ipc/mod.rs"] = "#![allow(unsafe_code)]\npub fn handler() {}\n"
record("FAIL: allow(unsafe_code) in a non-FFI module (ipc) is caught", scan(T) >= 1)

# --- FAIL: forbid crate that still carries an allow (would not compile) ------------------------
T = dict(PASS_TREE); T["src-tauri/src/main.rs"] = "#![forbid(unsafe_code)]\nfn main() {}\n"
record("FAIL: a #![forbid(unsafe_code)] crate with an allow is caught", scan(T) >= 1)

# --- FAIL: two allow modules in one crate (exactly-one rule) ----------------------------------
T = dict(PASS_TREE)
T["src-tauri/src/platform/extra.rs"] = "#![allow(unsafe_code)]\n// SAFETY: x\nfn f() { unsafe {} }\n"
record("FAIL: two allow(unsafe_code) modules in one crate is caught", scan(T) >= 1)

# --- FAIL: unsafe block in the allow-listed module without a // SAFETY: ------------------------
T = dict(PASS_TREE)
T["src-tauri/src/platform/os.rs"] = "#![allow(unsafe_code)]\npub fn f() { unsafe { libc_x(); } }\n"
record("FAIL: unsafe block without a // SAFETY: justification is caught", scan(T) >= 1)

# --- a commented / stringed allow must NOT count ----------------------------------------------
T = dict(PASS_TREE)
T["src-tauri/src/ipc/mod.rs"] = '// #![allow(unsafe_code)] (a note, not real)\nconst S: &str = "#![allow(unsafe_code)]";\npub fn h() {}\n'
record("robustness: an allow(unsafe_code) in a comment/string does NOT count", scan(T) == 0)

# --- delimiter-bound deny token (unsafe_codex must not satisfy unsafe_code) --------------------
T = dict(PASS_TREE); T["src-tauri/src/main.rs"] = "#![deny(unsafe_codex)]\nfn main() {}\n"
record("robustness: a look-alike deny token (unsafe_codex) does NOT satisfy the deny", scan(T) >= 1)

failed = [n for n, ok in results if not ok]
print(f"\n{len(results) - len(failed)}/{len(results)} legs passed")
if failed:
    print("FAILED:", *failed, sep="\n  - ")
    sys.exit(1)
sys.exit(0)
