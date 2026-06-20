#!/usr/bin/env python3
"""g24-supply-chain.py - G24 self-test for check-supply-chain (P0.3.6, G18/G18a/G18b).

Proves the structural supply-chain growth-guard catches every way the policy could be silently
WEAKENED: a forbidden crate un-denied, a copyleft license allow-listed, `yanked`/version downgraded,
the license confidence floor lowered, a per-crate license exception slipped in, an advisory-ignore-set
mismatch with cargo-audit, an unknown source allowed, crates.io dropped from the allow-list, the
cargo-vet import sources dropped below 2, or an exemption added past the frozen count. Also confirms
the REAL committed deny.toml + supply-chain/config.toml evaluate clean and main() is target-absent-OK.
stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import copy
import importlib.machinery
import importlib.util
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-supply-chain"
_loader = importlib.machinery.SourceFileLoader("csc", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("csc", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def good_deny() -> dict:
    return {
        "bans": {"wildcards": "deny", "deny": [{"crate": c} for c in sorted(m.FORBIDDEN_CRATES)]},
        "licenses": {"version": 2, "confidence-threshold": 0.93, "allow": ["MIT", "Apache-2.0"], "exceptions": []},
        "advisories": {"version": 2, "yanked": "deny", "ignore": []},
        "sources": {"unknown-registry": "deny", "unknown-git": "deny",
                    "allow-registry": ["https://github.com/rust-lang/crates.io-index"]},
    }


def good_vet() -> dict:
    return {"imports": {"mozilla": {"url": "u1"}, "google": {"url": "u2"}}, "exemptions": {}}


# --- the valid baseline passes ----------------------------------------------------------------
record("good deny.toml shape -> no problems", m.evaluate_deny(good_deny(), set()) == [])
record("good config.toml shape -> no problems", m.evaluate_vet(good_vet()) == [])

# --- [bans] -----------------------------------------------------------------------------------
d = good_deny(); d["bans"]["deny"] = [x for x in d["bans"]["deny"] if x["crate"] != "reqwest"]
record("a forbidden crate (reqwest) un-denied -> caught", any("reqwest" in p for p in m.evaluate_deny(d, set())))
d = good_deny(); d["bans"]["wildcards"] = "allow"
record("[bans].wildcards downgraded to allow -> caught", any("wildcards" in p for p in m.evaluate_deny(d, set())))

# --- [licenses] -------------------------------------------------------------------------------
d = good_deny(); d["licenses"]["allow"].append("GPL-3.0")
record("copyleft GPL-3.0 allow-listed -> caught", any("copyleft" in p for p in m.evaluate_deny(d, set())))
d = good_deny(); d["licenses"]["allow"].append("LGPL-2.1")
record("copyleft LGPL-2.1 allow-listed -> caught", any("copyleft" in p for p in m.evaluate_deny(d, set())))
d = good_deny(); d["licenses"]["version"] = 1
record("[licenses].version downgraded to 1 -> caught", any("licenses].version" in p for p in m.evaluate_deny(d, set())))
d = good_deny(); d["licenses"]["confidence-threshold"] = 0.5
record("license confidence floor lowered to 0.5 -> caught", any("confidence" in p for p in m.evaluate_deny(d, set())))
d = good_deny(); d["licenses"]["exceptions"] = [{"crate": "x", "allow": ["GPL-3.0"]}]
record("a per-crate license exception slipped in -> caught", any("exceptions" in p for p in m.evaluate_deny(d, set())))

# --- [advisories] -----------------------------------------------------------------------------
d = good_deny(); d["advisories"]["yanked"] = "warn"
record("[advisories].yanked downgraded to warn -> caught", any("yanked" in p for p in m.evaluate_deny(d, set())))
d = good_deny(); d["advisories"]["version"] = 1
record("[advisories].version downgraded to 1 -> caught", any("advisories].version" in p for p in m.evaluate_deny(d, set())))
d = good_deny(); d["advisories"]["ignore"] = ["RUSTSEC-2024-0001"]
record("advisory ignored in cargo-deny but NOT in cargo-audit -> caught (reconciliation)",
       any("ignore sets disagree" in p for p in m.evaluate_deny(d, set())))
d = good_deny(); d["advisories"]["ignore"] = ["RUSTSEC-2024-0001"]
record("the SAME advisory ignored in BOTH scanners -> NOT caught (sets agree)",
       m.evaluate_deny(d, {"RUSTSEC-2024-0001"}) == [])

# --- [sources] --------------------------------------------------------------------------------
d = good_deny(); d["sources"]["unknown-registry"] = "allow"
record("[sources].unknown-registry allowed -> caught", any("unknown-registry" in p for p in m.evaluate_deny(d, set())))
d = good_deny(); d["sources"]["allow-registry"] = []
record("crates.io dropped from allow-registry -> caught", any("crates.io" in p for p in m.evaluate_deny(d, set())))

# --- cargo-vet config (G18b) ------------------------------------------------------------------
v = good_vet(); v["imports"] = {"mozilla": {"url": "u1"}}
record("import sources dropped below 2 -> caught", any("DISTINCT" in p for p in m.evaluate_vet(v)))
v = good_vet(); v["exemptions"] = {"somecrate": [{"version": "1.0", "criteria": "safe-to-deploy"}]}
record("a cargo-vet exemption added past the frozen count -> caught", any("exemption set" in p for p in m.evaluate_vet(v)))

# --- G1-review fixes: distinct import URLs / frozen license set / workspace lock resolution -----
v = good_vet(); v["imports"] = {"mozilla": {"url": "https://same"}, "moz2": {"url": "https://same"}}
record("two import keys at the SAME url -> caught (distinct URLs, not key-count)",
       any("DISTINCT" in p for p in m.evaluate_vet(v)))
d = good_deny(); d["licenses"]["allow"].append("EUPL-1.2")
record("a non-GPL copyleft (EUPL-1.2) added to allow -> caught (frozen permissive set)",
       any("EUPL-1.2" in p for p in m.evaluate_deny(d, set())))
record("the REAL deny.toml allow-list is a subset of the frozen permissive set",
       set(m._load(m.DENY)["licenses"]["allow"]) <= m.EXPECTED_LICENSE_ALLOW)
with tempfile.TemporaryDirectory() as _td:
    base = Path(_td)
    lc = (base / "Cargo.lock", base / "src-tauri" / "Cargo.lock")
    tc = (base / "Cargo.toml", base / "src-tauri" / "Cargo.toml")
    record("workspace 'absent' when no manifest/lock exists -> live tier skips (P0 posture)",
           m._workspace_state(lc, tc)[0] == "absent")
    (base / "src-tauri").mkdir()
    (base / "src-tauri" / "Cargo.toml").write_text("[package]\n", encoding="utf-8")
    record("a src-tauri/Cargo.toml WITHOUT a lock -> 'lock-missing' (FAIL-closed, never silent skip)",
           m._workspace_state(lc, tc)[0] == "lock-missing")
    (base / "src-tauri" / "Cargo.lock").write_text("version = 3\n", encoding="utf-8")
    record("a src-tauri/Cargo.lock present -> 'ready' (found off-root; not the hard-coded path)",
           m._workspace_state(lc, tc)[0] == "ready")

# --- the REAL committed configs evaluate clean + main() is target-absent OK --------------------
record("the REAL committed deny.toml evaluates clean", m.evaluate_deny(m._load(m.DENY), set()) == [])
record("the REAL committed supply-chain/config.toml evaluates clean", m.evaluate_vet(m._load(m.VET_CONFIG)) == [])
record("main() exits 0 today (structural OK; live cargo-deny/vet target-absent until P1)", m.main() == 0)

# --- the P1-runway fix: workspace ready but cargo-deny/cargo-vet absent in this plane -> the live tier
# SKIPS (no binary-absent problems), not a fail (the frozen deny.toml/config.toml policy stays enforced) -
def _live_skip_when_binaries_absent() -> list:
    import tempfile
    saved = (m.shutil.which, m._workspace_state)
    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        m._workspace_state = lambda *a, **k: ("ready", root / "Cargo.toml", root / "Cargo.lock")
        m.shutil.which = lambda tool: None
        try:
            return m._live_checks()
        finally:
            m.shutil.which, m._workspace_state = saved


record("_live_checks(): workspace ready but cargo-deny/cargo-vet absent in this plane -> SKIP (no "
       "binary-absent problems), not a fail (P1-runway fix; live binaries enforce where present)",
       _live_skip_when_binaries_absent() == [])

failed = [n for n, ok in results if not ok]
print(f"\n[g24-supply-chain] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
