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
committed deny.toml + supply-chain/config.toml evaluate clean and main() exits 0 over the real tree.
The §0.8 floor legs read the floors as data (the root Cargo.toml
`[workspace.metadata.convertia.pinned-floors]`): a missing, unparseable or malformed table and a
missing Cargo.lock each fail closed. Their verdict legs run over a synthetic table, so a floor row
raised as box work (§0.8) leaves this canary green.
stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import contextlib
import copy
import importlib.machinery
import importlib.util
import io
import sys
import tempfile
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

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
record("main() exits 0 (structural OK + live cargo-deny/audit where present + §0.8 floor; binary-absent "
       "legs skip-with-notice)", m.main() == 0)

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

# --- cargo-deny transient-fetch retry classification (the spurious-red guard, G18) ------------
# A transient advisory-db FETCH failure is retried before fail-closing; a real policy violation
# (banned crate / disallowed license / a live advisory) is NOT a fetch error -> no transient match
# -> it fails fast on the first run (and even if mis-matched it still fails-closed after the retries).
record("deny transient: 'failed to fetch advisory database' (TLS timeout) -> retried",
       bool(m._DENY_TRANSIENT_RE.search(
           "error: failed to fetch advisory database https://github.com/RustSec/advisory-db: TLS handshake timeout")))
record("deny transient: git 'could not read from remote' -> retried",
       bool(m._DENY_TRANSIENT_RE.search("fatal: unable to access ...: could not read from remote repository")))
record("deny transient: HTTP 503 -> retried",
       bool(m._DENY_TRANSIENT_RE.search("error fetching index: HTTP 503 Service Unavailable")))
record("deny NON-transient: a banned-crate violation -> NOT retried (fails fast)",
       not m._DENY_TRANSIENT_RE.search("error[banned]: the crate 'reqwest' is explicitly banned"))
record("deny NON-transient: a license violation -> NOT retried",
       not m._DENY_TRANSIENT_RE.search("error[rejected]: crate 'foo' license 'GPL-3.0' is not in the allow list"))
record("deny NON-transient: a live vulnerability advisory -> NOT retried",
       not m._DENY_TRANSIENT_RE.search("error[vulnerability]: RUSTSEC-2024-1234 in crate foo 1.2.3"))

failed = [n for n, ok in results if not ok]
print(f"\n[g24-supply-chain] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
