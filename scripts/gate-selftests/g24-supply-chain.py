#!/usr/bin/env python3
"""g24-supply-chain.py - G24 self-test for check-supply-chain (P0.3.6, G18/G18a/G18b).

Proves the structural supply-chain growth-guard catches every way the policy could be silently
WEAKENED: a forbidden crate un-denied, a copyleft license allow-listed, `yanked`/version downgraded,
the license confidence floor lowered, a per-crate license exception slipped in, an advisory-ignore-set
mismatch with cargo-audit, an unknown source allowed, crates.io dropped from the allow-list, the
cargo-vet import sources dropped below 2, an exemption added past the frozen count, the deny.toml
`[graph].targets` scope drifting from the frozen shipped desktop-triple set (a DROPPED shipped triple
blinds cargo-deny to an on-that-triple dep = fail-OPEN; an ADDED non-shipped triple re-masks tauri's
mobile-only reqwest/hyper into scope = spurious fail), or `[graph].all-features` flipped off (a banned
crate reachable only via a non-default feature would escape [bans]), or an unexpected `tauri-plugin-*`
entering Cargo.lock (the structural PRESENCE scan `_tauri_plugin_drift` — the 2nd enforcer beside plan-lint
check 13 — accepts only the §0.10-granted set + the forced-transitive-inert `tauri-plugin-fs`). Also
confirms the REAL committed deny.toml `[graph].targets` equals EXPECTED_GRAPH_TARGETS, and that the REAL
committed deny.toml + supply-chain/config.toml evaluate clean and main() exits 0 over the real tree, its live
cargo-deny tier answered without the network (the canary is deterministic and offline).
The §0.8 floor legs read the floors as data (the root Cargo.toml
`[workspace.metadata.convertia.pinned-floors]`): a missing, unparseable or malformed table and a
missing Cargo.lock each fail closed. Their verdict legs run over a synthetic table, so a floor row
raised as box work (§0.8) leaves this canary green. The live cargo-deny tier's fetches run under the gate
plane's one network retry policy (scripts/net-retry, build-gates §0); with a scripted `run_bounded` that
records each command, its bound and its environment, and a fake clock, the legs pin its steps: every
cargo-deny command with `--color never`, every step under its own bound, an environment without the
repository-selecting GIT_* variables and with git's network settings of each class unchanged (GIT_CONFIG_GLOBAL,
GIT_CONFIG_PARAMETERS, GIT_CONFIG_COUNT with its keys and values, GIT_SSL_*, GIT_HTTP_*), CARGO_TERM_COLOR=never,
LC_ALL=C and GIT_TERMINAL_PROMPT=0, the ls-remote, probe and ancestry argv, and a coloured log read like a plain
one; a clone left behind the remote head (its ancestry check exiting 1, or 128), a step past its bound, an
ls-remote with no route or answered 503, a cold fetch whose ls-remote probe finds no route, runs past its bound or
meets an answering remote, and a `cargo fetch` with no route, each retried through the 300 s window; a rejected
certificate (in git's line, the probe's or cargo's), an ls-remote or a proxy CONNECT answered 403, a gone
repository (on the warm and on the cold path), a head that is no SHA-1, a fetch log that names no URL, no clone,
more URLs than clones or a clone G18 does not name, and a stale lock failing closed at once; a failure that names
its step; and a `check --disable-fetch` verdict that is never retried, the live tier's finding failing main(). The
clone cargo-deny 0.19.9 names (its XXH64 of the URL, its database root) and its removal - after a fetch past its
bound or failed and after a clone found behind the remote head, so the next attempt clones it fresh - are pinned
on scratch trees: a clone removed whole, nothing followed through a link inside it; a link, a clone outside the
root, a name not cargo-deny's, a file, a mount point and another user's directory refused with nothing deleted; a
clone another cargo-deny run holds left in place. Two legs run real git in throwaway repositories (the GIT_* scrub
of the header): a half clone of a killed fetch, and the stale clone a fetch onto it leaves, each removed and
cloned fresh.
stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import contextlib
import copy
import importlib.machinery
import importlib.util
import io
import os
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from unittest import mock
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")
# The real-git legs below run git in throwaway repositories: a hook's GIT_DIR must not point them at the hooked one.
for _k in [k for k in os.environ if k.startswith("GIT_") and k != "GIT_EXEC_PATH"]:
    os.environ.pop(_k)

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-supply-chain"
_loader = importlib.machinery.SourceFileLoader("csc", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("csc", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}{(' - ' + detail) if detail else ''}")


def good_deny() -> dict:
    return {
        "bans": {"wildcards": "deny", "deny": [{"crate": c} for c in sorted(m.FORBIDDEN_CRATES)]},
        "licenses": {"version": 2, "confidence-threshold": 0.93, "allow": ["MIT", "Apache-2.0"], "exceptions": []},
        "advisories": {"version": 2, "yanked": "deny", "ignore": []},
        "sources": {"unknown-registry": "deny", "unknown-git": "deny",
                    "allow-registry": ["https://github.com/rust-lang/crates.io-index"]},
        "graph": {"targets": sorted(m.EXPECTED_GRAPH_TARGETS), "all-features": True},
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

# --- [graph] freeze (P1.12 — the cargo-deny eval graph == the shipped graph: targets + all-features) ---
d = good_deny(); d["graph"]["targets"] = [t for t in d["graph"]["targets"] if t != "x86_64-unknown-linux-gnu"]
record("a SHIPPED desktop triple dropped from [graph].targets -> caught (fail-OPEN blind-spot to an "
       "on-Linux network/copyleft dep)", any("graph].targets" in p for p in m.evaluate_deny(d, set())))
d = good_deny(); d["graph"]["targets"] = d["graph"]["targets"] + ["aarch64-linux-android"]
record("a NON-shipped (mobile) triple added to [graph].targets -> caught (re-masks tauri's mobile-only "
       "reqwest/hyper back into scope)", any("graph].targets" in p for p in m.evaluate_deny(d, set())))
d = good_deny(); d["graph"]["targets"] = [{"triple": t} for t in sorted(m.EXPECTED_GRAPH_TARGETS)]
record("[graph].targets in the cargo-deny {triple=...} table form is parsed (equals frozen) -> clean",
       m.evaluate_deny(d, set()) == [])
d = good_deny(); d["graph"]["all-features"] = False
record("[graph].all-features downgraded to false -> caught (a feature-gated banned dep would escape "
       "[bans] eval)", any("all-features" in p for p in m.evaluate_deny(d, set())))
d = good_deny(); del d["graph"]["all-features"]
record("[graph].all-features removed entirely -> caught (default-features-only eval is the same "
       "weakening)", any("all-features" in p for p in m.evaluate_deny(d, set())))
record("the REAL committed deny.toml [graph].targets equals the frozen shipped-triple set (both entry "
       "forms via _graph_targets)", m._graph_targets(m._load(m.DENY)) == m.EXPECTED_GRAPH_TARGETS)

# --- tauri-plugin presence scan (the 2nd structural enforcer beside plan-lint check 13; P1.14) -------
# [Test-Change: P0.3.6 — old-obsolete+new-correct, §7.4.2 store plugin retired 2026-09-29] the granted-clean
# leg used tauri-plugin-store, which left the allowlist with the plugin; log is a granted plugin.
record("tauri-plugin presence: a granted plugin (log) -> clean",
       m._tauri_plugin_drift('name = "tauri-plugin-log"\n') == [])
record("tauri-plugin presence: tauri-plugin-store (retired, §7.4.2) -> caught (off the allowlist)",
       any("tauri-plugin-store" in p and "neither" in p
           for p in m._tauri_plugin_drift('name = "tauri-plugin-store"\n')))
record("tauri-plugin presence: tauri-plugin-fs (forced-transitive-inert dialog dep) -> clean",
       m._tauri_plugin_drift('name = "tauri-plugin-dialog"\nname = "tauri-plugin-fs"\n') == [])
record("tauri-plugin presence: an UNLISTED plugin (http) -> caught (unexpected surface, T2/T2c)",
       any("http" in p and "neither" in p for p in m._tauri_plugin_drift('name = "tauri-plugin-http"\n')))
record("tauri-plugin presence: every granted plugin + forced-inert fs together -> clean",
       m._tauri_plugin_drift("".join(f'name = "tauri-plugin-{n}"\n'
                                     for n in sorted(m.TAURI_PLUGIN_ALLOWLIST | m.TAURI_PLUGIN_INERT))) == [])
record("tauri-plugin presence: the granted allowlist + the forced-inert set are DISJOINT (fs is not granted)",
       not (m.TAURI_PLUGIN_ALLOWLIST & m.TAURI_PLUGIN_INERT))

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

# --- the REAL committed configs evaluate clean + main() is OK ----------------------------------
_real_audit_ig = m._ignore_set(m._load(m.AUDIT_TOML).get("advisories")) if m.AUDIT_TOML.is_file() else set()
record("the REAL committed deny.toml evaluates clean (reconciled vs the real audit.toml ignore set)",
       m.evaluate_deny(m._load(m.DENY), _real_audit_ig) == [])
record("the REAL committed supply-chain/config.toml evaluates clean", m.evaluate_vet(m._load(m.VET_CONFIG)) == [])
record("the REAL deny.toml + audit.toml advisory-ignore sets AGREE + are non-empty (G18 two-scanner "
       "reconciliation; P1.59)",
       m._ignore_set(m._load(m.DENY).get("advisories")) == _real_audit_ig and len(_real_audit_ig) > 0)


# [Test-Change: G24 network retry policy — old-obsolete+new-correct, build-gates §0] the leg ran G18's live tier
# over the network without git's network settings (this file's header and the runner drop them), so it failed closed
# after the 300 s window where git's proxy sits only in GIT_CONFIG_GLOBAL; main() still runs over the real tree, its
# live tier answered here and pinned by the deny-tier legs below.
def _main_with_tier_answered() -> tuple:
    """(main()'s exit, the manifests the live cargo-deny tier was asked for) over the real tree, with that tier answered
    as passed without the network: this canary runs offline and without git's network settings (its header and
    run-gate-selftests' child environment drop every GIT_* variable but GIT_EXEC_PATH), so the live tier here would
    fail closed after the retry window for no defect. The scripted deny-tier legs below pin the tier, and G18's own
    L1/L2/L4 runs exercise it live."""
    asked: list = []
    with mock.patch.object(m, "_run_cargo_deny", lambda manifest: asked.append(manifest)):
        return m.main(), asked


_rc, _asked = _main_with_tier_answered()
_tier_runs = m._workspace_state()[0] == "ready" and m.shutil.which("cargo-deny") is not None
record("main() exits 0 (structural OK + the live cargo-deny tier answered without the network, asked once for the "
       "real manifest where cargo-deny is present + §0.8 floor; binary-absent legs skip-with-notice)",
       _rc == 0 and len(_asked) == (1 if _tier_runs else 0), f"exit {_rc}, tier asked {len(_asked)}x")

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


def _live_checks_with(finding: "str | None") -> list:
    """_live_checks() with the workspace ready, cargo-deny present (cargo-vet absent) and the live cargo-deny tier
    returning `finding`."""
    saved = (m.shutil.which, m._workspace_state, m._run_cargo_deny)
    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        m._workspace_state = lambda *a, **k: ("ready", root / "Cargo.toml", root / "Cargo.lock")
        m.shutil.which = lambda tool: str(root / tool) if tool == "cargo-deny" else None
        m._run_cargo_deny = lambda manifest: finding
        try:
            with contextlib.redirect_stdout(io.StringIO()):
                return m._live_checks()
        finally:
            m.shutil.which, m._workspace_state, m._run_cargo_deny = saved


def _main_with_live(problems: list) -> tuple:
    """(main()'s exit, its stderr) with the live tier returning `problems` and every other check over the real tree."""
    saved = m._live_checks
    m._live_checks = lambda: list(problems)
    err = io.StringIO()
    try:
        with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
            rc = m.main()
    finally:
        m._live_checks = saved
    return rc, err.getvalue()


_LIVE_FINDING = "the advisory database fetch failed at once (a final failure) - G18 fails closed"
record("_live_checks(): what the live cargo-deny tier found is the gate's problem, and a tier that passed adds none",
       _live_checks_with(_LIVE_FINDING) == [_LIVE_FINDING] and _live_checks_with(None) == [])
_rc, _err = _main_with_live([_LIVE_FINDING])
record("main(): a finding of the live tier fails G18 (exit 1, the finding printed as a FAIL)",
       _rc == 1 and f"[G18] FAIL: {_LIVE_FINDING}" in _err, f"exit {_rc}")

# --- §0.8 pinned-floor assertion + its semver comparator (P1.59) ------------------------------
record("_version_ge: equal pre-releases (rc.25 >= rc.25) -> True", m._version_ge("2.0.0-rc.25", "2.0.0-rc.25") is True)
record("_version_ge: a HIGHER pre-release (rc.26 >= rc.25) -> True", m._version_ge("2.0.0-rc.26", "2.0.0-rc.25") is True)
record("_version_ge: a LOWER pre-release (rc.24 >= rc.25) -> False", m._version_ge("2.0.0-rc.24", "2.0.0-rc.25") is False)
record("_version_ge: a release outranks its own pre-release (2.0.0 >= 2.0.0-rc.25) -> True", m._version_ge("2.0.0", "2.0.0-rc.25") is True)
record("_version_ge: a pre-release is BELOW the release floor (2.0.0-rc.25 >= 2.0.0) -> False", m._version_ge("2.0.0-rc.25", "2.0.0") is False)
record("_version_ge: a higher patch (2.5.1 >= 2.5.0) -> True", m._version_ge("2.5.1", "2.5.0") is True)
record("_version_ge: a lower minor (2.4.0 >= 2.5.0) -> False", m._version_ge("2.4.0", "2.5.0") is False)
record("_version_ge: a higher major (3.0.0 >= 2.5.0) -> True", m._version_ge("3.0.0", "2.5.0") is True)
record("_version_ge: numeric pre-segments sort NUMERICALLY not lexically (rc.10 >= rc.2) -> True", m._version_ge("1.0.0-rc.10", "1.0.0-rc.2") is True)
record("_version_ge: an unparseable version -> None (the caller fails closed)", m._version_ge("not.a.version", "2.5.0") is None)
record("_pinned_floor_assertion(): the REAL Cargo.lock satisfies every §0.8 floor", m._pinned_floor_assertion() == [])


def _floor_with_temp_lock(body: str) -> list:
    """_pinned_floor_assertion over the REAL floor table (the root Cargo.toml) and a temp lock carrying `body`."""
    saved = m.CARGO_LOCK_CANDIDATES
    with tempfile.TemporaryDirectory() as td:
        lock = Path(td) / "Cargo.lock"
        lock.write_text(body, encoding="utf-8")
        m.CARGO_LOCK_CANDIDATES = (lock,)
        try:
            return m._pinned_floor_assertion()
        finally:
            m.CARGO_LOCK_CANDIDATES = saved


_FLOOR_HDR = "[workspace.metadata.convertia.pinned-floors]\n"


def _assert_with(manifest_text: "str | None", lock_body: "str | None") -> list:
    """_pinned_floor_assertion over a temp manifest (None = absent) and a temp lock (None = absent)."""
    saved = (m.FLOORS_MANIFEST, m.CARGO_LOCK_CANDIDATES)
    with tempfile.TemporaryDirectory() as td:
        man, lock = Path(td) / "Cargo.toml", Path(td) / "Cargo.lock"
        if manifest_text is not None:
            man.write_text(manifest_text, encoding="utf-8")
        if lock_body is not None:
            lock.write_text(lock_body, encoding="utf-8")
        m.FLOORS_MANIFEST, m.CARGO_LOCK_CANDIDATES = man, (lock,)
        try:
            return m._pinned_floor_assertion()
        finally:
            m.FLOORS_MANIFEST, m.CARGO_LOCK_CANDIDATES = saved


def _pkg(name: str, version: str) -> str:
    """One Cargo.lock [[package]] entry."""
    return f'[[package]]\nname = "{name}"\nversion = "{version}"\n\n'


# [Test-Change: G18 floors as data — old-obsolete+new-correct, §0.8] the at-floor leg and the five verdict
# legs after it were judged against the retired PINNED_FLOORS constant (the at-floor lock generated from it,
# three verdict locks made by find-and-replace of its specta and walkdir values). The floors are data now, and
# raising a row is ordinary box work (§0.8) that must leave this caged canary green, so no leg keys on a real
# row's value or presence: the at-floor lock is generated from the loaded real table, and the verdict legs run
# over a synthetic two-row table (_TWO_FLOORS, the floors the old fixtures assumed) with the same versions
# under test and the same assertions.
_REAL_FLOORS, _REAL_FLOORS_WHY = m._load_floors(m.FLOORS_MANIFEST)
record("_load_floors(): the REAL root Cargo.toml floor table loads, non-empty, every floor a semver string",
       _REAL_FLOORS_WHY is None and bool(_REAL_FLOORS)
       and all(m._parse_ver(v) is not None for v in _REAL_FLOORS.values()))
_all_at_floor = "".join(_pkg(c, v) for c, v in (_REAL_FLOORS or {}).items())
record("_pinned_floor_assertion(): a temp lock with every §0.8 crate AT its floor -> clean",
       _floor_with_temp_lock(_all_at_floor) == [])
_TWO_FLOORS = _FLOOR_HDR + 'specta = "2.0.0-rc.25"\nwalkdir = "2.5.0"\n'
_SPECTA_AT, _WALKDIR_AT = _pkg("specta", "2.0.0-rc.25"), _pkg("walkdir", "2.5.0")
_below = _pkg("specta", "2.0.0-rc.2") + _WALKDIR_AT
record("_pinned_floor_assertion(): specta DOWN to rc.2 (< the rc.25 floor) -> caught (below the API floor)",
       any("specta" in p and "below the relied-upon API floor" in p for p in _assert_with(_TWO_FLOORS, _below)))
record("_pinned_floor_assertion(): a §0.8 floor crate MISSING from the lock -> caught (relied-upon dep vanished)",
       any("walkdir" in p and "not in Cargo.lock" in p for p in _assert_with(_TWO_FLOORS, _SPECTA_AT)))
_garbage = _SPECTA_AT + _pkg("walkdir", "garbage")   # walkdir floor 2.5.0 -> unparseable
record("_pinned_floor_assertion(): an unparseable lock version -> fail-closed (caught, not silently passed)",
       any("unparseable" in p for p in _assert_with(_TWO_FLOORS, _garbage)))
# multi-version robustness (Cargo.lock may carry duplicate-version crates): pass if ANY copy >= floor.
_multi_ok = _SPECTA_AT + _WALKDIR_AT + _pkg("walkdir", "2.4.0")   # walkdir 2.5.0 (floor) + an older 2.4.0
record("_pinned_floor_assertion(): a floor crate present at TWO versions (2.5.0 + older 2.4.0) -> clean (a copy >= floor)",
       _assert_with(_TWO_FLOORS, _multi_ok) == [])
_multi_mid = _SPECTA_AT + _pkg("walkdir", "2.4.0") + _WALKDIR_AT + _pkg("walkdir", "2.3.0")   # floor copy in the middle
record("_pinned_floor_assertion(): the at-floor copy BETWEEN two older ones (2.4.0, 2.5.0, 2.3.0) -> clean (every copy "
       "is judged, not only the first or the last)", _assert_with(_TWO_FLOORS, _multi_mid) == [])
_multi_below = _SPECTA_AT + _pkg("walkdir", "2.3.0") + _pkg("walkdir", "2.4.0")   # both copies below the 2.5.0 floor
record("_pinned_floor_assertion(): a floor crate present ONLY at versions below floor (2.3.0 + 2.4.0) -> caught",
       any("walkdir" in p and "below the relied-upon API floor" in p
           for p in _assert_with(_TWO_FLOORS, _multi_below)))


# --- the floor table is data: every way it can be missing or malformed fails closed ---------------
def _floors_from(text: "str | bytes | None") -> tuple:
    """_load_floors over a temp manifest carrying `text` (None = no file at all)."""
    with tempfile.TemporaryDirectory() as td:
        man = Path(td) / "Cargo.toml"
        if text is not None:
            man.write_bytes(text if isinstance(text, bytes) else text.encode("utf-8"))
        return m._load_floors(man)


record("_load_floors(): a well-formed table -> the rows",
       _floors_from(_FLOOR_HDR + 'walkdir = "2.5.0"\nspecta = "2.0.0-rc.25"\n')
       == ({"walkdir": "2.5.0", "specta": "2.0.0-rc.25"}, None))
record("_load_floors(): an EMPTY table -> no rows, not a finding (G71's monotone rule guards the emptying)",
       _floors_from(_FLOOR_HDR) == ({}, None))
for _label, _text, _needle in (
        ("the manifest missing", None, "missing"),
        ("the table absent", '[workspace]\nmembers = []\n', "no [workspace.metadata.convertia.pinned-floors] table"),
        ("a parent key that is not a table", '[workspace]\nmetadata = "x"\n', "no [workspace.metadata"),
        ("the table spelled as a string", '[workspace.metadata.convertia]\npinned-floors = "x"\n', "no [workspace"),
        ("unparseable TOML", _FLOOR_HDR + 'walkdir = \n', "unreadable"),
        ("non-UTF-8 bytes", b"\xff\xfe" + _FLOOR_HDR.encode(), "unreadable"),
        ("a non-string floor", _FLOOR_HDR + "walkdir = 2\n", "['walkdir']"),
        ("a floor that is no semver version", _FLOOR_HDR + 'walkdir = "latest"\n', "['walkdir']"),
        ("a two-part floor", _FLOOR_HDR + 'walkdir = "2.5"\n', "['walkdir']"),
        ("a sub-table row", _FLOOR_HDR + '[workspace.metadata.convertia.pinned-floors.walkdir]\nv = "1.0.0"\n',
         "['walkdir']")):
    _floors, _why = _floors_from(_text)
    record(f"_load_floors(): {_label} -> fail-closed (no floors, the reason named)",
           _floors is None and isinstance(_why, str) and _needle in _why)


_ONE_ROW = _FLOOR_HDR + 'walkdir = "2.5.0"\n'
_ONE_PKG = '[[package]]\nname = "walkdir"\nversion = "2.5.0"\n'
record("_pinned_floor_assertion(): a temp manifest row AT floor over a temp lock -> clean",
       _assert_with(_ONE_ROW, _ONE_PKG) == [])
record("_pinned_floor_assertion(): the floor comes from the manifest (a raised row reds the same lock)",
       any("walkdir" in p and "below the relied-upon API floor" in p
           for p in _assert_with(_FLOOR_HDR + 'walkdir = "2.6.0"\n', _ONE_PKG)))
record("_pinned_floor_assertion(): the floor table ABSENT -> caught (fail-closed, the home named)",
       any("cannot be asserted" in p and "pinned-floors" in p
           for p in _assert_with("[workspace]\n", _ONE_PKG)))
record("_pinned_floor_assertion(): the manifest ABSENT -> caught (fail-closed)",
       any("cannot be asserted" in p for p in _assert_with(None, _ONE_PKG)))
record("_pinned_floor_assertion(): Cargo.lock ABSENT -> caught (was a silent pass before the floors moved to data)",
       any("no Cargo.lock" in p for p in _assert_with(_ONE_ROW, None)))
record("_pinned_floor_assertion(): an EMPTY table over a lock -> clean (no rows to assert)",
       _assert_with(_FLOOR_HDR, _ONE_PKG) == [])


def _main_with_manifest(text: str) -> tuple:
    """(exit code, stderr) of main() over a temp floor manifest, the live cargo tier stubbed out so the
    verdict can only come from the frozen policy and the floor leg (no network, no binary)."""
    saved = (m.FLOORS_MANIFEST, m._live_checks)
    err = io.StringIO()
    with tempfile.TemporaryDirectory() as td:
        man = Path(td) / "Cargo.toml"
        man.write_text(text, encoding="utf-8")
        m.FLOORS_MANIFEST, m._live_checks = man, lambda: []
        try:
            with contextlib.redirect_stderr(err):
                rc = m.main()
        finally:
            m.FLOORS_MANIFEST, m._live_checks = saved
    return rc, err.getvalue()


_rc_nofloor, _err_nofloor = _main_with_manifest("[workspace]\n")
record("main(): a manifest WITHOUT the floor table -> exit 1 naming the floor finding (the floor leg is "
       "wired, fail-closed)", _rc_nofloor == 1 and "relied-upon floors cannot be asserted" in _err_nofloor)

# --- widened malformed-config guard (a NON-UTF-8 / unreadable config must FAIL-CLOSED, not crash with an
# uncaught UnicodeDecodeError — it is a ValueError, NOT an OSError, so the old `except TOMLDecodeError` alone
# let it through; mirrors freeze_config / check-rust-lint's _audit_ignores). ---------------------------------
def _main_with_override(attr: str) -> int:
    saved = getattr(m, attr)
    with tempfile.TemporaryDirectory() as td:
        bad = Path(td) / "bad.toml"
        bad.write_bytes(b'\xff\xfe version = 2\n')          # non-UTF-8 -> UnicodeDecodeError, not TOMLDecodeError
        setattr(m, attr, bad)
        try:
            return m.main()
        finally:
            setattr(m, attr, saved)


record("guard: a NON-UTF-8 deny.toml -> main() returns 2 (fail-CLOSED, not an uncaught UnicodeDecodeError)",
       _main_with_override("DENY") == 2)
record("guard: a NON-UTF-8 audit.toml -> main() returns 2 (fail-CLOSED, not a crash)",
       _main_with_override("AUDIT_TOML") == 2)


def _floor_with_nonutf8_lock() -> list:
    saved = m.CARGO_LOCK_CANDIDATES
    with tempfile.TemporaryDirectory() as td:
        lock = Path(td) / "Cargo.lock"
        lock.write_bytes(b'\xff\xfe [[package]]\n')
        m.CARGO_LOCK_CANDIDATES = (lock,)
        try:
            return m._pinned_floor_assertion()
        finally:
            m.CARGO_LOCK_CANDIDATES = saved


record("guard: a NON-UTF-8 Cargo.lock -> _pinned_floor_assertion() fail-CLOSED (caught, not a crash)",
       any("unreadable" in p or "UTF-8" in p for p in _floor_with_nonutf8_lock()))

# --- the classifiers of the live tier's failures (git's own words, cargo's, the ANSI-free reading) -------------
# [Test-Change: G24 network retry policy — old-obsolete+new-correct, build-gates §0] the six legs here pinned
# _DENY_TRANSIENT_RE, whose blanket `failed to fetch` read a gone advisory repository on a cold cache as an outage
# (G1 R1 of the act); a failed fetch is now classified by git's own `unable to access` line (_git_transient) and a
# failed `cargo fetch` by curl's code or an HTTP status (_cargo_transient). The legs below feed both lines of each
# case - those git 2.43, cargo 1.96 and cargo-deny 0.19.9 printed in the gate container marked `measured` - the
# transport failures transient, a rejected certificate, a 4xx, a proxy's refusal and a gone repository final - and
# keep the old legs' second half: cargo-deny's own words and a policy verdict are no transient failure.
_DB_URL = "https://github.com/RustSec/advisory-db"
_UNABLE = f"fatal: unable to access '{_DB_URL}/': "
_GIT_TRANSIENT_LINES = (
    _UNABLE + "Could not resolve host: github.com",                                    # measured, no route out
    _UNABLE + "Failed to connect to github.com port 443 after 133 ms: Couldn't connect to server",
    _UNABLE + "Recv failure: Connection reset by peer",
    _UNABLE + "The requested URL returned error: 503",
    _UNABLE + "The requested URL returned error: 429",
    _UNABLE + "CONNECT tunnel failed, response 502",
)
_GIT_FINAL_LINES = (
    _UNABLE + "server certificate verification failed. CAfile: /tmp/ca.pem CRLfile: none",   # measured, a foreign CA
    _UNABLE + "SSL certificate problem: self-signed certificate in certificate chain",
    _UNABLE + "SSL: no alternative certificate subject name matches target host name 'github.com'",
    # git for Windows over schannel and git over macOS Secure Transport (curl's wordings for those back ends)
    _UNABLE + "schannel: SEC_E_UNTRUSTED_ROOT (0x80090325) - The certificate chain was issued by an authority that is "
              "not trusted.",
    _UNABLE + "schannel: SEC_E_CERT_UNKNOWN (0x80090327) - An unknown error occurred while processing the certificate.",
    _UNABLE + "schannel: SEC_E_WRONG_PRINCIPAL (0x80090322) - The target principal name is incorrect.",
    _UNABLE + "SSL: the server certificate is not trusted",
    _UNABLE + "The requested URL returned error: 403",
    _UNABLE + "The requested URL returned error: 404",
    "fatal: unable to access 'https://gitlab.com/x.git/': CONNECT tunnel failed, response 403",   # measured, off-list
    "fatal: could not read Username for 'https://github.com': No such device or address",       # measured, gone repo
    "2026-10-29 17:08:34 [ERROR] failed to fetch database: failed to get 'FETCH_HEAD' metadata",  # cargo-deny's own
    "error[banned]: crate 'reqwest' is explicitly banned",
    "error[vulnerability]: RUSTSEC-2024-1234 in crate foo 1.2.3 (timed out 503)",
)
_wrong = [ln for ln in _GIT_TRANSIENT_LINES if m._git_transient(ln + "\n") is None]
_wrong += [ln for ln in _GIT_FINAL_LINES if m._git_transient(ln + "\n") is not None]
record("_git_transient: git's `unable to access` line naming no route, a failed connect, a reset, HTTP 503 or 429, "
       "or a proxy's CONNECT 502 is transient; a rejected certificate (OpenSSL's three wordings, schannel's "
       "SEC_E_UNTRUSTED_ROOT, SEC_E_CERT_UNKNOWN and SEC_E_WRONG_PRINCIPAL, Secure Transport's `certificate is not "
       "trusted`), HTTP 403 or 404, a proxy's CONNECT 403, a gone repository, cargo-deny's own words and a policy "
       "verdict are final", not _wrong,
       "; ".join(_wrong) or f"{len(_GIT_TRANSIENT_LINES) + len(_GIT_FINAL_LINES)} lines")
record("_git_transient: a line wrapped in ANSI colour codes is read like the plain line",
       m._git_transient("\x1b[31m" + _GIT_TRANSIENT_LINES[0] + "\x1b[0m\n") == "Could not resolve host: github.com")
_CAUSED = "error: failed to get `chardetng` as a dependency of package `convertia-core v0.1.0`\n\nCaused by:\n  curl failed\n\nCaused by:\n"
_CARGO_TRANSIENT_OUT = (
    _CAUSED + "  [6] Could not resolve hostname (Could not resolve host: index.crates.io)\n",       # measured
    _CAUSED + "  [7] Could not connect to server (Failed to connect to index.crates.io port 443 after 1 ms: Could not "
              "connect to server)\n",                                    # a refused connection (the R2 review measured)
    _CAUSED + "  [28] Timeout was reached (Operation timed out after 30000 milliseconds)\n",
    _CAUSED + "  [56] Failure when receiving data from the peer (CONNECT tunnel failed, response 502)\n",
    "error: failed to download from `https://static.crates.io/crates/x/x-1.0.0.crate`\n\nCaused by:\n  failed to get "
    "successful HTTP response from `https://static.crates.io/crates/x/x-1.0.0.crate` (151.101.1.1), got 503\n",
    "error: failed to get successful HTTP response from `https://index.crates.io/config.json` (151.101.1.1), "
    "got 429\n",
)
_CARGO_FINAL_OUT = (
    _CAUSED + "  [56] Failure when receiving data from the peer (CONNECT tunnel failed, response 403)\n",   # measured
    _CAUSED + "  [60] SSL peer certificate or SSH remote key was not OK (SSL certificate problem)\n",
    _CAUSED + "  [35] schannel: next InitializeSecurityContext failed: SEC_E_UNTRUSTED_ROOT (0x80090325) - The "
              "certificate chain was issued by an authority that is not trusted.\n",   # a transient code, a rejected CA
    "error: failed to download from `https://static.crates.io/crates/x/x-1.0.0.crate`\n\nCaused by:\n  failed to get "
    "successful HTTP response from `https://static.crates.io/crates/x/x-1.0.0.crate` (151.101.1.1), got 404\n",
    "error: cannot update the lock file /w/Cargo.lock because --locked was passed to prevent this\n",       # measured
)
_wrong = [o.splitlines()[-1] for o in _CARGO_TRANSIENT_OUT if m._cargo_transient(o) is None]
_wrong += [o.splitlines()[-1] for o in _CARGO_FINAL_OUT if m._cargo_transient(o) is not None]
record("_cargo_transient: curl's no-host, no-connection, timeout and receive failures, an HTTP or CONNECT 5xx and an "
       "HTTP 429 are transient; a proxy's CONNECT 403, a certificate error (under any curl code, schannel's among "
       "them), an HTTP 404 and a stale lock are final", not _wrong,
       "; ".join(_wrong) or f"{len(_CARGO_TRANSIENT_OUT) + len(_CARGO_FINAL_OUT)} outputs")
_REPO_ENV = {"GIT_DIR": "/hooked/.git", "GIT_WORK_TREE": "/hooked", "GIT_INDEX_FILE": "/hooked/.git/index",
             "GIT_COMMON_DIR": "/hooked/.git", "GIT_OBJECT_DIRECTORY": "/hooked/.git/objects",
             "GIT_ALTERNATE_OBJECT_DIRECTORIES": "/alt", "GIT_NAMESPACE": "ns", "GIT_PREFIX": "sub/",
             "GIT_CEILING_DIRECTORIES": "/hooked"}
_NETWORK_ENV = {"GIT_CONFIG_GLOBAL": "/cfg/gitconfig", "GIT_CONFIG_PARAMETERS": "'http.proxy'='http://p:3128'",
                "GIT_SSL_CAINFO": "/cfg/ca.pem", "GIT_SSH_COMMAND": "ssh -F /cfg/ssh", "GIT_HTTP_USER_AGENT": "probe",
                "GIT_EXEC_PATH": "/usr/lib/git-core", "HTTPS_PROXY": "http://p:3128"}


@contextlib.contextmanager
def _caller_env(extra: dict):
    """os.environ with `extra` set for the block (the environment a hook or a dev shell hands the gate); a value of None
    unsets that variable for the block."""
    saved = {k: os.environ.get(k) for k in extra}
    for k, v in extra.items():
        if v is None:
            os.environ.pop(k, None)
        else:
            os.environ[k] = v
    try:
        yield
    finally:
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v


def _live_env(env: "dict | None") -> bool:
    """The environment the live tier hands every command: no repository-selecting GIT_* variable, git's network
    settings kept, CARGO_TERM_COLOR=never, LC_ALL=C and GIT_TERMINAL_PROMPT=0."""
    return (env is not None and not set(_REPO_ENV) & set(env) and all(env.get(k) == v for k, v in _NETWORK_ENV.items())
            and env.get("CARGO_TERM_COLOR") == "never" and env.get("LC_ALL") == "C"
            and env.get("GIT_TERMINAL_PROMPT") == "0")


with _caller_env({**_REPO_ENV, **_NETWORK_ENV, "CARGO_TERM_COLOR": "always", "LC_ALL": "de_DE.UTF-8",
                  "GIT_TERMINAL_PROMPT": "1"}):
    _env = m._git_env()
record("_git_env: the nine repository-selecting GIT_* variables dropped; git's network settings, GIT_EXEC_PATH and "
       "the proxy kept; CARGO_TERM_COLOR=never, LC_ALL=C and GIT_TERMINAL_PROMPT=0 over the caller's",
       _live_env(_env), f"kept repo vars {sorted(set(_REPO_ENV) & set(_env))}")


# --- the advisory database clone as cargo-deny 0.19.9 names it, and its removal (the comment above _ANSI_RE) ----------
record("_xxh64: the XXH64 test vectors (empty, `a`, `abc` and a sentence, seed 0)",
       [m._xxh64(d, 0) for d in (b"", b"a", b"abc", b"Nobody inspects the spammish repetition")]
       == [0xEF46DB3751D8E999, 0xD24EC4F1A98C6E5B, 0x44BC2CF5AD770999, 0xFBCEA83C8A378BF1])
_DEFAULT_CLONE = "advisory-db-3157b0e258782691"       # the directory the pinned cargo-deny made (measured)
record("_db_clone_name: the default database's clone is advisory-db-3157b0e258782691, the name cargo-deny 0.19.9 gave "
       "it (measured), whatever the URL's case",
       m._db_clone_name(_DB_URL) == _DEFAULT_CLONE == m._db_clone_name(_DB_URL.lower())
       == m._db_clone_name("HTTPS://GITHUB.COM/RUSTSEC/ADVISORY-DB"), str(m._db_clone_name(_DB_URL)))
_UNNAMED_URLS = (_DB_URL + "/", _DB_URL + ".git/", "http://github.com/RustSec/advisory-db",
                 "https://github.com:443/RustSec/advisory-db", "https://user@github.com/RustSec/advisory-db",
                 _DB_URL + "?ref=main", _DB_URL + "#main", "https://github.com/RustSec/../advisory-db",
                 "https://github.com/RustSec/advisory%2Ddb", "https://localhost/advisory-db", "https://github.com")
record("_db_clone_name: a URL outside the form G18 names exactly (a trailing slash, http, a port, a user, a query, a "
       "fragment, a dot segment, a percent escape, a dotless host, no path) gets no name",
       all(m._db_clone_name(u) is None for u in _UNNAMED_URLS),
       ", ".join(u for u in _UNNAMED_URLS if m._db_clone_name(u) is not None))
_MANIFEST = m.ROOT / "Cargo.toml"


def _root_with(env: dict, files: "dict | None" = None, manifest_sub: str = "") -> tuple:
    """_deny_db_root for a manifest in a scratch tree holding `files` (relative path -> text), or for the real manifest
    when `files` is None, under the caller environment `env` (a None value unsets)."""
    with tempfile.TemporaryDirectory() as td, _caller_env(env):
        if files is None:
            return m._deny_db_root(_MANIFEST)
        for rel, text in files.items():
            (Path(td) / rel).parent.mkdir(parents=True, exist_ok=True)
            (Path(td) / rel).write_text(text, encoding="utf-8")
        got = m._deny_db_root(Path(td) / manifest_sub / "Cargo.toml")
        return tuple(x.replace(td, "<td>") if isinstance(x, str) else x for x in got)


_HOME_VAR = "USERPROFILE" if os.name == "nt" else "HOME"
_ABS = os.path.abspath(os.sep + "abs")
_ADV = "[advisories]\nversion = 2\n"


def _db_path(sub: str) -> str:
    """A deny config's [advisories] section with db-path <_ABS>/<sub> (a TOML literal string: no escape is read)."""
    return _ADV + f"db-path = '{os.path.join(_ABS, sub)}'\n"


_roots = {
    "CARGO_HOME absolute": _root_with({"CARGO_HOME": _ABS}),
    "CARGO_HOME relative": _root_with({"CARGO_HOME": "rel-home"}),
    "no CARGO_HOME": _root_with({"CARGO_HOME": None, _HOME_VAR: _ABS}),
    "neither": _root_with({"CARGO_HOME": None, _HOME_VAR: None}),
    "db-path": _root_with({"CARGO_HOME": _ABS}, {"deny.toml": _db_path("dbs")}),
    "db-path ~": _root_with({"CARGO_HOME": _ABS}, {"deny.toml": _ADV + "db-path = '~/dbs'\n"}),
    "db-path $": _root_with({"CARGO_HOME": _ABS}, {"deny.toml": _ADV + "db-path = '$CARGO_HOME/dbs'\n"}),
    "absolute db-path with ~": _root_with({"CARGO_HOME": _ABS}, {"deny.toml": _db_path(os.path.join("~", "dbs"))}),
    "absolute db-path with $": _root_with({"CARGO_HOME": _ABS}, {"deny.toml": _db_path(os.path.join("$HOME", "dbs"))}),
    "db-path relative": _root_with({"CARGO_HOME": _ABS}, {"deny.toml": _ADV + "db-path = 'dbs'\n"}),
    ".deny.toml": _root_with({"CARGO_HOME": _ABS}, {".deny.toml": _db_path("h")}),
    ".cargo/deny.toml": _root_with({"CARGO_HOME": _ABS}, {".cargo/deny.toml": _db_path("c")}),
    "a parent's": _root_with({"CARGO_HOME": _ABS}, {"deny.toml": _db_path("p"), "sub/Cargo.toml": ""}, "sub"),
    "deny.toml first": _root_with({"CARGO_HOME": _ABS}, {"deny.toml": _ADV, ".deny.toml": _db_path("h")}),
}
_want = {
    "CARGO_HOME absolute": (os.path.join(_ABS, "advisory-dbs"), ""),
    "CARGO_HOME relative": (os.path.join(os.getcwd(), "rel-home", "advisory-dbs"), ""),
    "no CARGO_HOME": (os.path.join(_ABS, ".cargo", "advisory-dbs"), ""),
    "db-path": (os.path.join(_ABS, "dbs"), ""), ".deny.toml": (os.path.join(_ABS, "h"), ""),
    ".cargo/deny.toml": (os.path.join(_ABS, "c"), ""), "a parent's": (os.path.join(_ABS, "p"), ""),
    "deny.toml first": (os.path.join(_ABS, "advisory-dbs"), ""),
}
_wrong = [k for k, want in _want.items() if _roots[k] != want]
_wrong += [k for k in ("neither", "db-path ~", "db-path $", "absolute db-path with ~", "absolute db-path with $",
                       "db-path relative") if _roots[k][0] is not None]
record("_deny_db_root: cargo-deny's database root - the db-path of the first deny.toml, .deny.toml or .cargo/deny.toml "
       "from the manifest's directory up, else $CARGO_HOME/advisory-dbs (a relative CARGO_HOME under the working "
       "directory, none: the home's .cargo); a db-path with `~` or `$` or relative, or no home at all, is not named",
       not _wrong, "; ".join(f"{k}: {_roots[k]}" for k in _wrong) or f"{len(_roots)} cases")


def _make_clone(path: Path, sentinel: "Path | None" = None) -> None:
    """A database clone's look-alike at `path`: a nested tree, a read-only file (git makes its objects read-only) and,
    with `sentinel`, a link to the sentinel's directory outside (a symlink, or a junction on a Windows host without
    the symlink privilege)."""
    (path / ".git" / "objects" / "pack").mkdir(parents=True)
    pack = path / ".git" / "objects" / "pack" / "pack-1.pack"
    pack.write_bytes(b"PACK")
    os.chmod(pack, stat.S_IREAD)
    (path / "crates").mkdir()
    (path / "crates" / "RUSTSEC-0000-0000.md").write_text("x", encoding="utf-8")
    if sentinel is not None:
        _plant_link(path / "outside-link", sentinel.parent)


def _plant_link(link: Path, target: Path) -> str:
    """A directory link at `link` to `target`: a symlink where the host allows one, else a Windows junction."""
    try:
        os.symlink(target, link, target_is_directory=True)
        return "symlink"
    except OSError:
        if os.name != "nt":
            raise
    subprocess.run(["cmd", "/c", "mklink", "/J", str(link), str(target)], check=True, capture_output=True,
                   stdin=subprocess.DEVNULL, timeout=60)
    return "junction"


def _sentinel(base: Path) -> Path:
    s = base / "outside" / "keep.txt"
    s.parent.mkdir(parents=True)
    s.write_text("keep", encoding="utf-8")
    return s


with tempfile.TemporaryDirectory() as _td:
    _base = Path(_td)
    _root = _base / "advisory-dbs"
    _root.mkdir()
    _keep = _sentinel(_base)
    _clone = _root / _DEFAULT_CLONE
    _make_clone(_clone, _keep)
    _outcome = m._remove_db_clone(str(_clone), str(_root))
    _left = sorted(p.name for p in _root.iterdir())
    record("_remove_db_clone: a real clone directory named advisory-db-<16 hex digits> directly inside the database "
           "root and owned by this user is removed whole - its read-only files too - with no quarantine left behind "
           "(only cargo-deny's lock file, which cargo-deny makes too) and nothing followed through the link inside it",
           _outcome[0] == "removed" and _left == ["db.lock"] and _keep.read_text(encoding="utf-8") == "keep",
           f"{_outcome}; left {_left}")
    record("_remove_db_clone: nothing at the path, or no database root at all, is nothing to remove",
           m._remove_db_clone(str(_clone), str(_root))[0] == "absent"
           and m._remove_db_clone(str(_base / "none" / _DEFAULT_CLONE), str(_base / "none"))[0] == "absent")


def _refused_with(setup, patch=None) -> tuple:
    """(the outcome, the text, the tree under the scratch dir before == after) of a removal that must be refused:
    `setup(base, root)` plants the case and returns the clone path to remove; `patch` a context for the call."""
    with tempfile.TemporaryDirectory() as td:
        base = Path(td)
        root = base / "advisory-dbs"
        root.mkdir()
        _sentinel(base)
        clone = setup(base, root)
        before = sorted(str(p.relative_to(base)) for p in base.rglob("*"))
        with (patch or contextlib.nullcontext)():
            outcome, text = m._remove_db_clone(str(clone), str(root))
        after = sorted(str(p.relative_to(base)) for p in base.rglob("*"))
        return outcome, text, before == after and (base / "outside" / "keep.txt").is_file()


def _linked(base: Path, root: Path) -> Path:
    _plant_link(root / _DEFAULT_CLONE, base / "outside")
    return root / _DEFAULT_CLONE


def _outside(base: Path, root: Path) -> Path:
    _make_clone(base / "elsewhere" / _DEFAULT_CLONE)
    return base / "elsewhere" / _DEFAULT_CLONE


def _named(name: str):
    def setup(base: Path, root: Path) -> Path:
        _make_clone(root / name)
        return root / name
    return setup


def _plain_file(base: Path, root: Path) -> Path:
    (root / _DEFAULT_CLONE).write_text("not a clone", encoding="utf-8")
    return root / _DEFAULT_CLONE


_REFUSALS = {
    "a link (symlink or junction) in the clone's place": (_linked, None, "is a link"),
    "a clone directory outside the database root": (_outside, None, "not directly inside"),
    "a name with upper-case hex digits": (_named("advisory-db-3157B0E258782691"), None, "not named like"),
    "a name with 15 hex digits": (_named("advisory-db-3157b0e25878269"), None, "not named like"),
    "a quarantine name": (_named(_DEFAULT_CLONE + ".removing-1-2"), None, "not named like"),
    "another directory's name": (_named("registry"), None, "not named like"),
    "a file in the clone's place": (_plain_file, None, "not a directory"),
    "another user's directory": (_named(_DEFAULT_CLONE),
                                 lambda: mock.patch.object(m, "_owned_by_user", lambda _p, _s: False), "not owned"),
    "a mount point": (_named(_DEFAULT_CLONE),
                      lambda: mock.patch.object(m.os.path, "ismount", lambda p: os.path.basename(p) == _DEFAULT_CLONE),
                      "mount point"),
}
_wrong = []
for _label, (_setup, _patch, _needle) in _REFUSALS.items():
    _o, _t, _kept = _refused_with(_setup, _patch)
    if not (_o == "refused" and _needle in _t and _kept):
        _wrong.append(f"{_label}: {_o} {_t[:120]} kept={_kept}")
record("_remove_db_clone: a link in the clone's place, a clone outside the database root, a name not cargo-deny's "
       "(upper-case or short hex, a quarantine name, another directory), a file, another user's directory and a mount "
       "point are each refused, naming the path, and nothing under the scratch tree is deleted or changed",
       not _wrong, "; ".join(_wrong) or f"{len(_REFUSALS)} cases")


def _hold_lock(root: Path):
    """Another process's hold on cargo-deny's lock (<root>/db.lock), as a second open of it with the lock taken."""
    fd = os.open(root / "db.lock", os.O_RDWR | os.O_CREAT, 0o644)
    if os.name == "nt":
        import msvcrt
        msvcrt.locking(fd, msvcrt.LK_NBLCK, 1)
    else:
        import fcntl
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    return fd


with tempfile.TemporaryDirectory() as _td:
    _root = Path(_td) / "advisory-dbs"
    _root.mkdir()
    _make_clone(_root / _DEFAULT_CLONE)
    _held = _hold_lock(_root)
    try:
        _busy = m._remove_db_clone(str(_root / _DEFAULT_CLONE), str(_root))
    finally:
        os.close(_held)
    _free = m._remove_db_clone(str(_root / _DEFAULT_CLONE), str(_root))
    record("_remove_db_clone: a clone whose cargo-deny lock another run holds is left in place (busy, not waited "
           "for); once the lock is free it is removed",
           _busy[0] == "busy" and _free[0] == "removed" and not (_root / _DEFAULT_CLONE).exists(), f"{_busy} {_free}")
with tempfile.TemporaryDirectory() as _td:
    _root = Path(_td) / "advisory-dbs"
    _root.mkdir()
    _make_clone(_root / _DEFAULT_CLONE)
    _real_lstat = os.lstat

    def _other_inode(path, *a, **k):
        st = _real_lstat(path, *a, **k)
        if ".removing-" in os.path.basename(path):
            return os.stat_result((st.st_mode, st.st_ino + 1) + tuple(st)[2:])
        return st

    with mock.patch.object(m.os, "lstat", _other_inode):
        _changed = m._remove_db_clone(str(_root / _DEFAULT_CLONE), str(_root))
    _moved = [p.name for p in _root.iterdir() if p.name != "db.lock"]
    record("_remove_db_clone: a clone that is not the checked directory once renamed (another inode) is refused and "
           "left at its quarantine name, never removed",
           _changed[0] == "refused" and "changed while it was checked" in _changed[1] and len(_moved) == 1
           and ".removing-" in _moved[0] and (_root / _moved[0] / "crates" / "RUSTSEC-0000-0000.md").is_file(),
           f"{_changed} {_moved}")
with tempfile.TemporaryDirectory() as _td:
    _own = Path(_td) / "mine"
    _own.mkdir()
    _st = os.lstat(_own)
    if os.name == "nt":
        _foreign = os.environ.get("SystemRoot", r"C:\Windows")          # owned by TrustedInstaller
        _foreign_mine = m._owned_by_user(_foreign, os.lstat(_foreign))
    else:
        with mock.patch.object(m.os, "geteuid", lambda: _st.st_uid + 1):
            _foreign_mine = m._owned_by_user(str(_own), _st)
    record("_owned_by_user: a directory this canary made is its user's; one owned by another user is not (POSIX: "
           "another effective uid; Windows: the system directory)",
           m._owned_by_user(str(_own), _st) is True and _foreign_mine is False)
    _ro = Path(_td) / "read-only"
    _ro.write_text("x", encoding="utf-8")
    os.chmod(_ro, stat.S_IREAD)
    with mock.patch.object(m.os, "chmod", wraps=os.chmod) as _chmod:
        m._clear_readonly(os.unlink, str(_ro), PermissionError(13, "refused"))
    _cleared = _chmod.call_args_list == [mock.call(str(_ro), stat.S_IREAD | stat.S_IWRITE)] and not _ro.exists()
    _link = Path(_td) / "link"
    _plant_link(_link, _own)
    _raised = []
    for _exc, _path in ((OSError(5, "other"), str(_own)), (PermissionError(13, "refused"), str(_link))):
        try:
            m._clear_readonly(os.unlink, _path, _exc)
        except OSError as _e:
            _raised.append(_e is _exc)
    record("_clear_readonly: a delete refused for a read-only file clears that bit on the file itself and is tried "
           "once more; any other error, and a refused delete of a link, is raised as it was, the path untouched",
           _cleared and _raised == [True, True] and _own.is_dir() and os.path.lexists(_link),
           f"chmod {_chmod.call_args_list} raised {_raised}")

# --- the G18 live tier under the gate plane's one network retry policy (scripts/net-retry, build-gates §0) ------
# _run_cargo_deny is driven with a fake `run_bounded` of the gate's own policy module (patched only inside the
# context manager) that answers each command the steps issue - `cargo deny fetch db`, `git ls-remote` (after the
# fetch, or as the probe of a failed one), `git merge-base --is-ancestor`, `cargo fetch --locked`, `cargo deny check
# --disable-fetch` - and records each command with its bound and its environment, and a fake-clock Retrier, so no
# leg waits: the schedule is the policy's (2 s doubling to a 60 s cap, until 300 s after the first failure), and a
# clone left behind the remote head (cargo-deny 0.19.9 logs a failed git fetch as fetched while a clone is cached)
# is a transient failure, so offline G18 fails closed after the window instead of passing on the cache.
class _Clock:
    def __init__(self) -> None:
        self.now = 0.0
        self.pauses: list[float] = []

    def clock(self) -> float:
        return self.now

    def sleep(self, seconds: float) -> None:
        self.pauses.append(seconds)
        self.now += seconds


# Every _deny_run sets CARGO_HOME to a scratch directory of this canary's, so the clone G18 names and may remove -
# <it>/advisory-dbs/advisory-db-3157b0e258782691 - is never under the caller's own cargo home.
_CARGO_HOME_TD = tempfile.TemporaryDirectory(prefix="g24-supply-chain-")
_CARGO_HOME = _CARGO_HOME_TD.name
_DB_CLONE = os.path.join(_CARGO_HOME, "advisory-dbs", _DEFAULT_CLONE)


def _fetched(clone: str = _DB_CLONE) -> tuple:
    """`cargo deny fetch db`'s log of a fetch that passed, naming the database URL and the clone it loaded."""
    return (0, f"2026-10-29 13:46:47 [DEBUG] Fetching advisory database with git cli from '{_DB_URL}'\n"
               f"2026-10-29 13:46:47 [DEBUG] fetched {_DB_URL}\n"
               f"2026-10-29 13:46:47 [DEBUG] loading advisory database from {clone}\n")


_FETCHED = _fetched()
# the run a fetch past its bound leaves: cargo-deny named the URL before run_bounded ended it
_KILLED = (124, f"2026-10-29 19:45:17 [DEBUG] Fetching advisory database with git cli from '{_DB_URL}'\n"
                "cargo: timed out - no result within the 120 s bound of one attempt")
# cargo-deny's log under CARGO_TERM_COLOR=always: the level wrapped in colour codes (the shape the R1 review measured)
_FETCHED_COLOURED = (0, _FETCHED[1].replace("[DEBUG]", "[\x1b[34mDEBUG\x1b[0m]"))
# a failed fetch with no clone cached: the same words for no route, a rejected certificate and a gone repository
_COLD_FAILED = (1, f"2026-10-29 17:08:34 [DEBUG] Fetching advisory database with git cli from '{_DB_URL}'\n"
                   f"2026-10-29 17:08:34 [DEBUG] cloned {_DB_URL}\n"
                   "2026-10-29 17:08:34 [ERROR] failed to fetch database: failed to get 'FETCH_HEAD' metadata: failed "
                   "to parse ISO-8601 timestamp 'fatal: cannot change to '/x/advisory-db-3157b0e258782691': No such file "
                   "or directory'\n")
_REMOTE_HEAD = (0, "acf7eedad03d9200e8e3e66331e89af25b072541\tHEAD\n", "")
_NO_ROUTE = (128, "", _UNABLE + "Could not resolve host: github.com\n")
_GONE = (128, "", "fatal: could not read Username for 'https://github.com': No such device or address\n")
_BAD_CA = (128, "", _UNABLE + "server certificate verification failed. CAfile: /tmp/ca.pem CRLfile: none\n")
_CRATES_NO_ROUTE = (101, _CAUSED + "  [6] Could not resolve hostname (Could not resolve host: index.crates.io)\n")
_STALE_LOCK = (101, "error: cannot update the lock file /w/Cargo.lock because --locked was passed to prevent this\n")
_BOUND = {"fetch": m._DENY_FETCH_ATTEMPT_S, "ls-remote": m._LS_REMOTE_ATTEMPT_S, "ancestry": m._MERGE_BASE_S,
          "crates": m._CARGO_FETCH_ATTEMPT_S, "check": m._DENY_CHECK_S}


_REAL_RUN_BOUNDED = m._NET.run_bounded


def _deny_run(fetches: list, heads: list, ancestry: list, crates: list = ((0, ""),),
              check: tuple = (0, ""), *, cargo_home: "str | None" = _CARGO_HOME, caller: "dict | None" = None,
              on_fetch=None, real_ancestry: bool = False,
              logs: "list | None" = None) -> tuple[object, dict, _Clock, list]:
    """_run_cargo_deny over scripted runs, in the caller environment of _REPO_ENV and _NETWORK_ENV (or `caller`) with
    CARGO_HOME `cargo_home` (None: no CARGO_HOME and no home directory); each list holds one entry per run of its
    command, the last repeating: `fetches` (exit,
    stderr) of `cargo deny fetch db`, `heads` (exit, stdout, stderr) of every `git ls-remote` (after a fetch and as a
    probe alike), `ancestry` the exit of `git merge-base --is-ancestor` (with `real_ancestry`, the real command runs
    instead), `crates` (exit, stderr) of `cargo fetch --locked`; `check` the (exit, stderr) of `cargo deny check
    --disable-fetch`. `on_fetch(index, exit)` runs at each fetch (what cargo-deny would do to its clone), and the
    Retrier's lines go to `logs`. Returns (the finding or None, the runs per command, the fake clock, every recorded
    (kind, command, bound, environment))."""
    fc = _Clock()
    m._RETRY = m._NET.Retrier("G18", clock=fc.clock, sleep=fc.sleep, rng=lambda: 1.0,
                              log=logs.append if logs is not None else lambda _msg: None)
    runs = {"fetch": 0, "ls-remote": 0, "ancestry": 0, "crates": 0, "check": 0}
    calls: list[tuple[str, list, float, dict]] = []

    def take(script, kind: str):
        step = script[min(runs[kind], len(script) - 1)]
        runs[kind] += 1
        return step

    def fake_run_bounded(cmd, seconds, *, env=None):
        # routed by subcommand: this file spawns no git of its own (the G24 direct-run rule reads an exact program
        # name literal as a spawn)
        if cmd[1:2] == ["ls-remote"]:
            calls.append(("ls-remote", cmd, seconds, env))
            code, out, err = take(heads, "ls-remote")
            return subprocess.CompletedProcess(cmd, code, out, err)
        if "--is-ancestor" in cmd:
            calls.append(("ancestry", cmd, seconds, env))
            if real_ancestry:
                runs["ancestry"] += 1
                return _REAL_RUN_BOUNDED(cmd, seconds, env=env)
            return subprocess.CompletedProcess(cmd, take(ancestry, "ancestry"), "", "")
        if cmd[-2:] == ["fetch", "db"]:
            calls.append(("fetch", cmd, seconds, env))
            index = runs["fetch"]
            code, err = take(fetches, "fetch")
            if on_fetch is not None:
                on_fetch(index, code)
            return subprocess.CompletedProcess(cmd, code, "", err)
        if cmd[1:3] == ["fetch", "--locked"]:
            calls.append(("crates", cmd, seconds, env))
            code, err = take(crates, "crates")
            return subprocess.CompletedProcess(cmd, code, "", err)
        if cmd[-2:] == ["check", "--disable-fetch"]:
            calls.append(("check", cmd, seconds, env))
            runs["check"] += 1
            return subprocess.CompletedProcess(cmd, check[0], "", check[1])
        raise AssertionError(f"the G18 live tier ran an unexpected command: {cmd}")

    env = {**(caller if caller is not None else {**_REPO_ENV, **_NETWORK_ENV}), "CARGO_HOME": cargo_home}
    if cargo_home is None:
        env.update({"HOME": None, "USERPROFILE": None})      # no cargo home at all
    with _caller_env(env), mock.patch.object(m._NET, "run_bounded", fake_run_bounded):
        result = m._run_cargo_deny(_MANIFEST)
    return result, runs, fc, calls


_REAL_RETRY = m._RETRY
_WINDOW_PAUSES = [2.0, 4.0, 8.0, 16.0, 32.0, 60.0, 60.0, 60.0, 58.0]
r, runs, fc, calls = _deny_run([_FETCHED], [_REMOTE_HEAD], [0])
record("deny tier: a refreshed database and fetched crates -> one fetch, one ls-remote, one ancestry check, one "
       "`cargo fetch --locked`, then the check's own result (no pause)",
       r is None and runs == {"fetch": 1, "ls-remote": 1, "ancestry": 1, "crates": 1, "check": 1} and fc.pauses == [])
_deny_cmds = [cmd for kind, cmd, _s, _e in calls if kind in ("fetch", "check")]
record("deny tier: every cargo-deny command asks for `--color never` before its subcommand, and `cargo fetch` runs "
       "`--locked --color never`",
       len(_deny_cmds) == 2 and all(cmd[1:4] == ["deny", "--color", "never"] for cmd in _deny_cmds)
       and all(cmd[1:5] == ["fetch", "--locked", "--color", "never"] for kind, cmd, _s, _e in calls if kind == "crates"),
       "; ".join(" ".join(cmd) for _k, cmd, _s, _e in calls))
record("deny tier: every step runs under its own module bound (fetch db 120 s, ls-remote 60 s, merge-base 60 s, "
       "cargo fetch 300 s, check 120 s)",
       len(calls) == 5 and all(seconds == _BOUND[kind] for kind, _c, seconds, _e in calls)
       and (_BOUND["fetch"], _BOUND["ls-remote"], _BOUND["ancestry"], _BOUND["crates"], _BOUND["check"])
       == (120, 60, 60, 300, 120),
       "; ".join(f"{kind} {seconds}" for kind, _c, seconds, _e in calls))
record("deny tier: every step's environment lacks the repository-selecting GIT_* variables (a hook's GIT_DIR) and "
       "keeps git's network settings (GIT_CONFIG_GLOBAL, GIT_SSL_CAINFO, the proxy), with CARGO_TERM_COLOR=never, "
       "LC_ALL=C and GIT_TERMINAL_PROMPT=0",
       len(calls) == 5 and all(_live_env(env) for _k, _c, _s, env in calls))
# git's network settings by class, as a contributor's shell or CI hands them to the gate: each one G18 drops would make
# its git and cargo-deny runs miss the proxy or CA a contributor set only there (the R3 review's scenario: git's proxy
# only in GIT_CONFIG_GLOBAL).
_NETWORK_CLASSES = {
    "GIT_CONFIG_GLOBAL": {"GIT_CONFIG_GLOBAL": "/cfg/gitconfig"},
    "GIT_CONFIG_PARAMETERS": {"GIT_CONFIG_PARAMETERS": "'http.proxy'='http://p:3128'"},
    "GIT_CONFIG_COUNT": {"GIT_CONFIG_COUNT": "2", "GIT_CONFIG_KEY_0": "http.proxy",
                         "GIT_CONFIG_VALUE_0": "http://p:3128", "GIT_CONFIG_KEY_1": "http.sslCAInfo",
                         "GIT_CONFIG_VALUE_1": "/cfg/ca.pem"},
    "GIT_SSL_*": {"GIT_SSL_CAINFO": "/cfg/ca.pem", "GIT_SSL_CAPATH": "/cfg/certs", "GIT_SSL_CERT": "/cfg/client.pem"},
    "GIT_HTTP_*": {"GIT_HTTP_USER_AGENT": "probe", "GIT_HTTP_PROXY_AUTHMETHOD": "basic",
                   "GIT_HTTP_LOW_SPEED_LIMIT": "1000", "GIT_HTTP_LOW_SPEED_TIME": "30"},
}
_all_network = {k: v for cls in _NETWORK_CLASSES.values() for k, v in cls.items()}
# a cold fetch that failed (its ls-remote probe answers), then a fetch that passed: every command the tier issues runs
_r_net, _runs_net, _fc_net, _calls_net = _deny_run([_COLD_FAILED, _FETCHED], [_REMOTE_HEAD], [0],
                                                   caller={**_REPO_ENV, **_all_network})
_kinds = sorted({kind for kind, _c, _s, _e in _calls_net})
_dropped = sorted({cls for _k, _c, _s, env in _calls_net for cls, kv in _NETWORK_CLASSES.items()
                   if any((env or {}).get(k) != v for k, v in kv.items())})
_leaked = sorted({k for _k, _c, _s, env in _calls_net for k in set(_REPO_ENV) & set(env or {})})
record("_git_env: every command of the live tier (the fetch, the probe and the ls-remote after the fetch, the ancestry "
       "check, the crate fetch, the check) receives git's network settings of each class unchanged - "
       "GIT_CONFIG_GLOBAL, GIT_CONFIG_PARAMETERS, GIT_CONFIG_COUNT with its GIT_CONFIG_KEY_n/GIT_CONFIG_VALUE_n, "
       "GIT_SSL_*, GIT_HTTP_* - and none of the repository-selecting variables",
       _r_net is None and _runs_net["ls-remote"] == 2
       and _kinds == ["ancestry", "check", "crates", "fetch", "ls-remote"] and not _dropped and not _leaked, f"classes dropped {_dropped}, repo vars kept {_leaked}, commands {_kinds}")
_HEAD_SHA = _REMOTE_HEAD[1].split()[0]
_ls_cmds = [cmd for kind, cmd, _s, _e in calls if kind == "ls-remote"]
_mb_cmds = [cmd for kind, cmd, _s, _e in calls if kind == "ancestry"]
record("deny tier: the fetch step asks the database URL cargo-deny logged for its HEAD (`git ls-remote <url> HEAD`) "
       "and checks that head in the clone G18 names for it (`git -C <clone> merge-base --is-ancestor <head> HEAD`)",
       _ls_cmds == [["git", "ls-remote", _DB_URL, "HEAD"]]
       and _mb_cmds == [["git", "-C", _DB_CLONE, "merge-base", "--is-ancestor", _HEAD_SHA, "HEAD"]],
       f"{_ls_cmds} {_mb_cmds}")
r, runs, fc, calls = _deny_run([_FETCHED_COLOURED], [_REMOTE_HEAD], [0])
record("deny tier: a fetch log whose level is wrapped in ANSI colour codes is read like the plain one (the clone "
       "named, the check run)", r is None and runs["ancestry"] == 1 and runs["check"] == 1,
       str(r)[:160] if r else "")
r, runs, fc, _calls = _deny_run([_FETCHED], [_REMOTE_HEAD], [1, 1, 0])
record("deny tier: a clone left behind the remote head (a fetch logged as fetched that failed) is retried with the "
       "policy's pauses (2 s, 4 s) until it is current",
       r is None and runs["fetch"] == 3 and runs["check"] == 1 and fc.pauses == [2.0, 4.0])
r, runs, fc, _calls = _deny_run([_FETCHED], [_REMOTE_HEAD], [1])
record("deny tier: a clone that stays behind the remote head fails closed through the window, naming `git merge-base "
       "--is-ancestor` in the clone; the crate fetch and the check never run",
       r is not None and "advisory database fetch failed through its 300 s retry window" in r
       and f"`git -C {_DB_CLONE} merge-base --is-ancestor" in r and runs["crates"] == 0 and runs["check"] == 0
       and fc.now == 300.0, (r or "")[:200])
r, runs, fc, _calls = _deny_run([_FETCHED], [_NO_ROUTE], [0])
record("deny tier: no route out (ls-remote `unable to access`) is retried through the 300 s window, then G18 fails "
       "closed naming the step - the crate fetch and the check never run",
       r is not None and "advisory database fetch failed through its 300 s retry window" in r
       and "`git ls-remote https://github.com/RustSec/advisory-db HEAD` failed" in r
       and "Could not resolve host" in r and "cargo deny check" not in r
       and runs["fetch"] == 10 and runs["crates"] == 0 and runs["check"] == 0 and fc.now == 300.0
       and fc.pauses == _WINDOW_PAUSES, (r or "")[:200])
r, runs, fc, _calls = _deny_run([_COLD_FAILED, _FETCHED], [_NO_ROUTE, _REMOTE_HEAD], [0])
record("deny tier: a cold fetch that failed is classified by git's own words - its ls-remote probe finds no route, "
       "so it is retried, and the next attempt fetches",
       r is None and runs["fetch"] == 2 and runs["ls-remote"] == 2 and fc.pauses == [2.0])
r, runs, fc, _calls = _deny_run([_COLD_FAILED], [_GONE], [0])
record("deny tier: a cold fetch of a repository that is gone (the probe asks for credentials) fails closed at once, "
       "naming `cargo deny fetch db` and the probe's words",
       r is not None and "failed at once" in r and "`cargo deny fetch db` failed (exit 1)" in r
       and "could not read Username" in r and runs["fetch"] == 1 and runs["crates"] == 0 and fc.pauses == [],
       (r or "")[:200])
r, runs, fc, _calls = _deny_run([_COLD_FAILED], [_BAD_CA], [0])
record("deny tier: a cold fetch under a CA bundle without the server's root (the probe's rejected certificate) "
       "fails closed at once", r is not None and "failed at once" in r and runs["fetch"] == 1 and fc.pauses == [])
r, runs, fc, _calls = _deny_run([_COLD_FAILED, _FETCHED], [_REMOTE_HEAD], [0])
record("deny tier: a cold fetch that failed while its ls-remote probe answers is retried (cargo-deny's words do not "
       "tell a cut transfer from a local error)",
       r is None and runs["fetch"] == 2 and fc.pauses == [2.0])
r, runs, fc, _calls = _deny_run([(124, "cargo: timed out - no result within the 120 s bound of one attempt"),
                                 _FETCHED], [_REMOTE_HEAD], [0])
record("deny tier: a fetch past its wall-clock bound is a transient timeout, retried",
       r is None and runs["fetch"] == 2 and runs["ls-remote"] == 1 and fc.pauses == [2.0])
r, runs, fc, _calls = _deny_run([_FETCHED], [(128, "", _UNABLE + "The requested URL returned error: 503\n"),
                                             _REMOTE_HEAD], [0])
record("deny tier: an ls-remote answered 503 is retried", r is None and runs["ls-remote"] == 2 and fc.pauses == [2.0])
for _label, _head in (("a gone repository (asks for credentials)", _GONE),
                      ("an ls-remote answered 403", (128, "", _UNABLE + "The requested URL returned error: 403\n")),
                      ("a rejected certificate", _BAD_CA),
                      ("a proxy that refuses the CONNECT (403)", (128, "", _UNABLE + "CONNECT tunnel failed, response "
                                                                                    "403\n"))):
    r, runs, fc, _calls = _deny_run([_FETCHED], [_head], [0])
    record(f"deny tier: {_label} at the ls-remote fails closed at once (only 429 and 5xx are transient)",
           r is not None and "failed at once" in r and runs["ls-remote"] == 1 and runs["check"] == 0
           and fc.pauses == [], (r or "")[:160])
r, runs, fc, _calls = _deny_run([_FETCHED], [(0, "ref: refs/heads/main\tHEAD\n", "")], [0])
record("deny tier: an ls-remote whose first word is no SHA-1 names no head and fails closed at once (no ancestry "
       "check runs)", r is not None and "named no head" in r and runs["ancestry"] == 0 and fc.pauses == [],
       (r or "")[:160])
r, runs, fc, _calls = _deny_run([(0, "2026-10-29 13:46:47 [INFO] advisory database fetched\n")], [_REMOTE_HEAD], [0])
record("deny tier: a fetch log that names no database clone fails closed at once (its outcome cannot be read)",
       r is not None and "cannot be read" in r and runs["ls-remote"] == 0 and fc.pauses == [])
r, runs, fc, _calls = _deny_run([_FETCHED], [_REMOTE_HEAD], [0], crates=[_CRATES_NO_ROUTE, (0, "")])
record("deny tier: a `cargo fetch` with no route (curl's [6]) is retried, then the check runs",
       r is None and runs["crates"] == 2 and runs["check"] == 1 and fc.pauses == [2.0])
r, runs, fc, _calls = _deny_run([_FETCHED], [_REMOTE_HEAD], [0], crates=[_STALE_LOCK])
record("deny tier: a stale lock (`cargo fetch --locked` cannot update it) fails closed at once, naming `cargo fetch "
       "--locked`; the check never runs",
       r is not None and "crate fetch failed at once" in r and "`cargo fetch --locked --manifest-path" in r
       and runs["crates"] == 1 and runs["check"] == 0 and fc.pauses == [], (r or "")[:200])
r, runs, fc, _calls = _deny_run([_FETCHED], [_REMOTE_HEAD], [0],
                                crates=[(124, "cargo: timed out - no result within the 300 s bound"), (0, "")])
record("deny tier: a `cargo fetch` past its wall-clock bound is a transient timeout, retried",
       r is None and runs["crates"] == 2 and fc.pauses == [2.0])
r, runs, fc, _calls = _deny_run([_FETCHED], [_REMOTE_HEAD], [0], crates=[_CRATES_NO_ROUTE])
record("deny tier: a `cargo fetch` that stays without a route fails closed through the window, naming `cargo fetch "
       "--locked`; the check never runs",
       r is not None and "crate fetch failed through its 300 s retry window" in r and "[6] Could not resolve" in r
       and runs["check"] == 0 and fc.now == 300.0, (r or "")[:200])
_COLD_CERT = (1, _COLD_FAILED[1] + _UNABLE + "server certificate verification failed. CAfile: /tmp/ca.pem CRLfile: none\n")
_COLD_ROUTE = (1, _COLD_FAILED[1] + _UNABLE + "Could not resolve host: github.com\n")
r, runs, fc, _calls = _deny_run([_COLD_CERT], [_REMOTE_HEAD], [0])
r2, runs2, fc2, _calls2 = _deny_run([_COLD_ROUTE, _FETCHED], [_REMOTE_HEAD], [0])
record("deny tier: a failed fetch whose own output carries git's `unable to access` line is classified by that line, "
       "with no probe - a rejected certificate fails closed at once, no route is retried",
       r is not None and "failed at once" in r and runs["fetch"] == 1 and runs["ls-remote"] == 0
       and r2 is None and runs2["fetch"] == 2 and runs2["ls-remote"] == 1 and fc2.pauses == [2.0], (r or "")[:160])
r, runs, fc, _calls = _deny_run([_COLD_FAILED, _FETCHED],
                                [(124, "", "git: timed out - no result within the 60 s bound of one attempt"),
                                 _REMOTE_HEAD], [0])
record("deny tier: a probe ls-remote past its bound is a transient timeout, so the failed cold fetch is retried",
       r is None and runs["fetch"] == 2 and fc.pauses == [2.0], (r or "")[:160])
r, runs, fc, _calls = _deny_run([(1, "2026-10-29 17:08:34 [ERROR] failed to acquire advisory database lock\n")],
                                [_REMOTE_HEAD], [0])
record("deny tier: a failed fetch whose output names no database URL (cargo-deny reached no fetch) fails closed at "
       "once, with no probe", r is not None and "failed at once" in r and runs["ls-remote"] == 0 and fc.pauses == [],
       (r or "")[:160])
r, runs, fc, calls = _deny_run([_COLD_FAILED, _FETCHED], [_NO_ROUTE, _REMOTE_HEAD], [0])
_probe = next((c for c in calls if c[0] == "ls-remote"), None)
record("deny tier: the probe of a failed cold fetch is `git ls-remote <url> HEAD` in the live tier's environment",
       _probe is not None and _probe[1] == ["git", "ls-remote", _DB_URL, "HEAD"] and _live_env(_probe[3]),
       str(_probe[1]) if _probe else "no probe")
_URL_LINE = f"2026-10-29 13:46:47 [DEBUG] Fetching advisory database with git cli from '{_DB_URL}'\n"
_LOADED_LINE = f"2026-10-29 13:46:47 [DEBUG] loading advisory database from {_DB_CLONE}\n"
r, runs, fc, _calls = _deny_run([(0, _URL_LINE * 2 + _LOADED_LINE)], [_REMOTE_HEAD], [0])
r2, runs2, fc2, _calls2 = _deny_run([_fetched(os.path.join(_CARGO_HOME, "elsewhere", _DEFAULT_CLONE))],
                                    [_REMOTE_HEAD], [0])
record("deny tier: a fetch log that names more database URLs than clones, or a clone other than the one G18 names for "
       "the URL, fails closed at once (its outcome cannot be read; nothing runs in that clone)",
       r is not None and "cannot be read" in r and runs["ls-remote"] == 0
       and r2 is not None and "cannot be read" in r2 and runs2["ls-remote"] == 0, f"{(r or '')[:100]} | {(r2 or '')[:100]}")
r, runs, fc, _calls = _deny_run([_KILLED], [_REMOTE_HEAD], [0], cargo_home=None)
r2, runs2, fc2, _calls2 = _deny_run([_FETCHED], [_REMOTE_HEAD], [0], cargo_home=None)
record("deny tier: with no CARGO_HOME and no home directory G18 cannot name the clone, and fails closed at once - a "
       "fetch past its bound (its clone not removed) as one that passed (no check in an unnamed clone)",
       r is not None and "failed at once" in r and "cannot be named" in r and fc.pauses == []
       and r2 is not None and "cannot name the clone" in r2 and runs2["ls-remote"] == 0, f"{(r or '')[:120]}")


def _clone_seen(seen: list):
    """An on_fetch that records whether the clone G18 names is there when each fetch starts."""
    def on_fetch(_index: int, _code: int) -> None:
        seen.append(os.path.isdir(_DB_CLONE))
    return on_fetch


def _drop_scratch(path: str) -> None:
    """Remove a scratch tree this canary made (its read-only files too); a link in its place is unlinked."""
    if os.path.islink(path) or (os.name == "nt" and os.path.isjunction(path)):
        with contextlib.suppress(OSError):
            os.unlink(path)
        with contextlib.suppress(OSError):
            os.rmdir(path)
    elif os.path.isdir(path):
        shutil.rmtree(path, onexc=lambda func, p, _e: (os.chmod(p, stat.S_IREAD | stat.S_IWRITE), func(p)))


_scenarios = {
    "a fetch past its bound": ([_KILLED, _FETCHED], [_REMOTE_HEAD], [0]),
    "a failed fetch": ([_COLD_FAILED, _FETCHED], [_NO_ROUTE, _REMOTE_HEAD], [0]),
    "a clone the currency check finds behind the remote head": ([_FETCHED], [_REMOTE_HEAD], [1, 0]),
}
_wrong = []
for _label, (_f, _h, _a) in _scenarios.items():
    _make_clone(Path(_DB_CLONE))
    _seen: list = []
    _logs: list = []
    r, runs, fc, _calls = _deny_run(_f, _h, _a, on_fetch=_clone_seen(_seen), logs=_logs)
    if not (r is None and _seen == [True, False] and fc.pauses == [2.0] and any("the clone removed" in ln for ln in _logs)):
        _wrong.append(f"{_label}: {r!r:.80} seen={_seen} pauses={fc.pauses} logs={_logs}")
    _drop_scratch(_DB_CLONE)
record("deny tier: a fetch past its bound, a failed fetch and a clone the currency check finds behind the remote head "
       "each have that database's clone removed before the retry, which then clones it fresh (the attempt's log line "
       "says so)", not _wrong, "; ".join(_wrong)[:300])
_make_clone(Path(_DB_CLONE))
_seen = []
_logs = []
r, runs, fc, _calls = _deny_run([_FETCHED], [_REMOTE_HEAD], [128, 0], on_fetch=_clone_seen(_seen), logs=_logs)
_drop_scratch(_DB_CLONE)
record("deny tier: an ancestry check that exits neither 0 nor 1 (128: git's answer when the remote head is not in the "
       "clone's object store, or HEAD is unborn) counts as a clone not refreshed - retried, the clone removed before "
       "the retry", r is None and _seen == [True, False] and runs["ancestry"] == 2 and fc.pauses == [2.0]
       and any("the clone removed" in ln for ln in _logs), f"{r!r:.80} seen={_seen} pauses={fc.pauses}")
with tempfile.TemporaryDirectory() as _td:
    _target = Path(_td) / "target"
    _target.mkdir()
    (_target / "keep.txt").write_text("keep", encoding="utf-8")
    os.makedirs(os.path.dirname(_DB_CLONE), exist_ok=True)
    _plant_link(Path(_DB_CLONE), _target)
    try:
        r, runs, fc, _calls = _deny_run([_FETCHED], [_REMOTE_HEAD], [1])
    finally:
        _drop_scratch(_DB_CLONE)
    record("deny tier: a clone the removal may not touch (a link in its place) fails G18 closed at once, naming its "
           "path and why, and nothing is deleted",
           r is not None and "failed at once" in r and "was not removed" in r and "is a link" in r
           and _DEFAULT_CLONE in r and (_target / "keep.txt").is_file() and runs["fetch"] == 1 and fc.pauses == [],
           (r or "")[-200:])
_make_clone(Path(_DB_CLONE))
_held = _hold_lock(Path(_DB_CLONE).parent)
_logs = []
try:
    r, runs, fc, _calls = _deny_run([_FETCHED], [_REMOTE_HEAD], [1, 0], logs=_logs)
finally:
    os.close(_held)
record("deny tier: a clone another cargo-deny run holds (its db.lock) is left in place and the attempt retried",
       r is None and os.path.isdir(_DB_CLONE) and fc.pauses == [2.0]
       and any("the clone left to another cargo-deny run" in ln for ln in _logs), "; ".join(_logs)[:200])
_drop_scratch(_DB_CLONE)


def _git_fixture(td: str) -> tuple:
    """(run, upstream, commit 1, commit 2): a throwaway upstream whose `main` is at commit 2 and whose `a-stale` is at
    commit 1 (it sorts first, as the measured stale branch sorted before main), and `run` - git with no hook, no
    signing, a fixed identity and none of the caller's GIT_* variables."""
    env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
    base = ["git", "-c", f"core.hooksPath={os.path.join(td, 'no-hooks')}", "-c", "commit.gpgsign=false",
            "-c", "user.name=g24", "-c", "user.email=g24@invalid", "-c", "init.defaultBranch=main"]

    def run(*args: str) -> str:
        return subprocess.run(base + list(args), env=env, capture_output=True, text=True, check=True,
                              stdin=subprocess.DEVNULL, timeout=120).stdout.strip()

    upstream = os.path.join(td, "upstream")
    run("init", "-q", upstream)
    run("-C", upstream, "commit", "-q", "--allow-empty", "-m", "c1")
    c1 = run("-C", upstream, "rev-parse", "HEAD")
    run("-C", upstream, "branch", "a-stale")
    run("-C", upstream, "commit", "-q", "--allow-empty", "-m", "c2")
    return run, upstream, c1, run("-C", upstream, "rev-parse", "HEAD")


def _cargo_deny_fetch(run, upstream: str, clone: str) -> None:
    """cargo-deny 0.19.9's fetch_via_cli on `clone` (read from its source): a clone that is there gets `reset --hard`,
    `fetch` and `reset --hard FETCH_HEAD`, each failure ignored as cargo-deny does; a clone that is not is cloned."""
    if os.path.isdir(clone):
        for args in (("reset", "-q", "--hard"), ("fetch", "-q"), ("reset", "-q", "--hard", "FETCH_HEAD")):
            with contextlib.suppress(subprocess.CalledProcessError):
                run("-C", clone, *args)
    else:
        run("clone", "-q", upstream, clone)


def _real_git_run(half_clone: bool, fetches: list) -> tuple:
    """A _deny_run in a scratch cargo home whose clone is left by a fetch killed mid-clone (`half_clone`: its remote
    set, no ref) or stuck on the stale branch (as the next fetch onto such a half clone leaves it); each fetch that
    passes runs cargo-deny's own fetch on the clone, and the currency check runs real git in it. Returns (the
    finding, runs, the fake clock, the stale head before, the head after, commit 1, commit 2)."""
    with tempfile.TemporaryDirectory() as td:
        run, upstream, c1, c2 = _git_fixture(td)
        home = os.path.join(td, "cargo-home")
        clone = os.path.join(home, "advisory-dbs", _DEFAULT_CLONE)
        run("init", "-q", clone)
        run("-C", clone, "remote", "add", "origin", upstream)
        if not half_clone:
            _cargo_deny_fetch(run, upstream, clone)
        before = run("-C", clone, "rev-parse", "--verify", "-q", "HEAD") if not half_clone else ""

        def on_fetch(_index: int, code: int) -> None:
            if code == 0:
                _cargo_deny_fetch(run, upstream, clone)

        script = [_fetched(clone) if f == "fetched" else f for f in fetches]
        r, runs, fc, _calls = _deny_run(script, [(0, f"{c2}\tHEAD\n", "")], [0], cargo_home=home, caller=dict(_REPO_ENV),
                                        on_fetch=on_fetch, real_ancestry=True)
        after = run("-C", clone, "rev-parse", "HEAD") if os.path.isdir(clone) else ""
        return r, runs, fc, before, after, c1, c2


r, runs, fc, _before, _after, _c1, _c2 = _real_git_run(True, [_KILLED, "fetched"])
record("deny tier, real git: a fetch killed mid-clone leaves a half clone (its remote set, no ref); G18 removes it, so "
       "the next fetch clones it fresh and the clone is current (cargo-deny's own fetch onto that half clone checks "
       "out the stale branch: the next leg)",
       r is None and _after == _c2 and runs["fetch"] == 2 and runs["ancestry"] == 1 and fc.pauses == [2.0],
       f"{(r or '')[:120]} head={_after[:12]}")
r, runs, fc, _before, _after, _c1, _c2 = _real_git_run(False, ["fetched"])
record("deny tier, real git: a clone stuck on the stale branch (cargo-deny's fetch onto a half clone checked it out, "
       "and every fetch keeps it there) fails `git merge-base --is-ancestor` in it; G18 removes the clone, and the "
       "next attempt clones it fresh and passes",
       r is None and _before == _c1 and _after == _c2 and runs["fetch"] == 2 and runs["ancestry"] == 2
       and fc.pauses == [2.0], f"{(r or '')[:120]} before={_before[:12]} after={_after[:12]}")
r, runs, fc, _calls = _deny_run([_FETCHED], [_NO_ROUTE], [0])
_spent = m._RETRY.spent


def _no_command(cmd, seconds, *, env=None):
    raise AssertionError(f"a spent run ran {cmd}")


with _caller_env({"CARGO_HOME": _CARGO_HOME}), mock.patch.object(m._NET, "run_bounded", _no_command):
    _again = m._run_cargo_deny(_MANIFEST)
record("deny tier: once the advisory fetch spent the window, a later run attempts no step (WindowSpent) and fails "
       "closed", _spent and _again is not None and "not attempted" in _again, (_again or "")[:160])
r, runs, fc, _calls = _deny_run([_FETCHED], [_REMOTE_HEAD], [0], check=(2, "error[banned]: crate 'reqwest' is "
                                                                         "explicitly banned\ntimed out 503"))
record("deny check: a policy verdict of `check --disable-fetch` is returned at once and never retried, whatever its "
       "text holds, and the finding names `cargo deny check`",
       r is not None and r.startswith("`cargo deny check --disable-fetch` failed (exit 2") and "banned" in r
       and runs["check"] == 1 and runs["fetch"] == 1 and runs["crates"] == 1 and fc.pauses == [], (r or "")[:160])
m._RETRY = _REAL_RETRY
_CARGO_HOME_TD.cleanup()

failed = [n for n, ok in results if not ok]
print(f"\n[g24-supply-chain] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
