#!/usr/bin/env python3
"""g24-fetch-engine-assets.py - the G24/G10 canary over `scripts/fetch-engine-assets` (P4.28).

Two jobs, both from OUTSIDE the tool so a neutering edit to fetch-engine-assets cannot neuter its
own check (the g24-stage-engines pattern; that file's COUPLING note is the model):

1. RUN the tool's fixture-driven `--selftest` (125 legs at delivery, P4.28/17808aa) and PIN the
   tally at 125 - the host-stable-count claim, CI-checked at L2 (diff-scoped canary) and L4 (3-OS).

2. The skip INVENTORY, in its strictest form: no leg of the fetch suite is OS-SKIPPED - every
   leg RUNS on every platform (one leg deliberately accepts two OS-divergent OUTCOMES, the
   separator-in-bare-asset-name leg; it still runs everywhere - the 2026-08-31 owner ruling: no
   silent skips), so ANY "(skipped" leg name failing here is a new silent skip that must arrive
   as a declared, owner-acked inventory edit, never quietly.

Plus independent planted positives driven against the tool's PUBLIC seams with their OWN inputs
(never the suite's fixtures, so a suite edit cannot mask them): six message-discriminated raiser
probes (a sibling raiser cannot satisfy the leg), one STRUCTURAL assertion (the per-hop redirect
pin installed in the real opener - no raiser exists there to discriminate), a FetchError-existence
leg (the refusal type the probes discriminate on, resolved once at module scope so a rename is a
named FAIL), and the manifest->egress DERIVATION pair (allowed_hosts built from the row's own
URLs + an off-row URL refused under that derived list).

COUPLING, declared so it is planned and never a surprise mid-box hard-stop: this file is
L(-1)-caged while fetch-engine-assets is not, and it PINS the tally (125), the empty-skip
inventory, and its OWN leg count - any box that adds a `--selftest` leg to THIS tool (the P5-P7
row boxes exercising real rows; P4.28.1 IF its from-source harness extends this script rather
than shipping its own - in the latter case that box ships its own sibling g24 canary) carries
the matching bump HERE as a pre-planned owner-acked L(-1) tail of that box.

Run:  python3 scripts/gate-selftests/g24-fetch-engine-assets.py   Exit 0 = every assertion held.
"""
import importlib.machinery
import importlib.util
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "fetch-engine-assets"
# The NamedTuple-tool g24 idiom (g24-stage-engines is the model): SourceFileLoader +
# module_from_spec with NO sys.modules entry. That absence is LOAD-BEARING here, not an omission:
# fetch-engine-assets deliberately uses NamedTuples (its own [Build-Session-Entscheidung: P4.28]
# note) BECAUSE a @dataclass cannot be constructed under this idiom — so this canary is also the
# standing proof that the tool stays importable this way; registering the module here would
# quietly unenforce that decision. (Canaries that DO register exist — g24-plan-lint, whose tool
# legitimately uses @dataclass, and g24-doc-splice — so the idiom is per-tool, not universal.)
_loader = importlib.machinery.SourceFileLoader("fea", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("fea", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


# Resolved ONCE at module scope so a renamed exception class becomes a named FAIL below, never an
# AttributeError escaping _refused's own except clause (that lookup happens DURING handling and is
# not covered by the try).
_FETCH_ERROR = getattr(m, "FetchError", None)


def _refused(fn) -> tuple[bool, str]:
    """Did fn raise the tool's own FetchError? Returns (raised, message) - a generic exception is
    a named FAIL, never a dead canary."""
    try:
        fn()
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        if _FETCH_ERROR is not None and isinstance(e, _FETCH_ERROR):
            return True, str(e)
        return False, f"WRONG exception {type(e).__name__}: {e}"
    return False, ""


def _probe(fn) -> bool:
    """Evaluate a tool-touching record() predicate fail-closed: a renamed private attribute
    (`_results`, `_PinnedRedirectHandler`, ...) must become a named FAIL, never a bare traceback
    - the same dead-canary discipline _refused applies to the raising seams."""
    try:
        return bool(fn())
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        print(f"[g24-fetch-engine-assets] predicate probe raised: {type(e).__name__}: {e}")
        return False


# --- 1. the full suite, its tally, and the strict skip inventory --------------------------------
print("[g24-fetch-engine-assets] running fetch-engine-assets --selftest ...")
try:
    rc = m.selftest()
    suite_crashed = ""
except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
    rc, suite_crashed = 1, f"{type(e).__name__}: {e}"
    print(f"[g24-fetch-engine-assets] --selftest raised: {suite_crashed}")
record("the tool's --selftest completed without an unhandled exception", not suite_crashed)
record("the tool's full --selftest suite passes under the canary runner", rc == 0)
record("the leg tally is host-stable at 125 (the pinned count)", _probe(lambda: len(m._results) == 125))
record(
    "skip-inventory (strict): every fetch-suite leg RUNS on every platform - NO leg may skip",
    _probe(lambda: not any("(skipped" in name for name, _ok in m._results)),
)

# --- 2. independent planted positives (own inputs, public seams) --------------------------------
record("the tool still exposes FetchError (the refusal type every raiser probe discriminates on)",
       _FETCH_ERROR is not None)
raised, msg = _refused(lambda: m.verify_sha256(b"canary-bytes", "0" * 64, url="https://x.invalid/a"))
record("planted positive: a pinned-hash mismatch REFUSES, naming the mismatch",
       raised and "SHA-256 mismatch" in msg)
raised, msg = _refused(lambda: m.verify_sha256(b"canary-bytes", "not-a-hash", url="https://x.invalid/a"))
record("planted positive: a malformed pin REFUSES as malformed (not as a mismatch)",
       raised and "malformed `asset_sha256` pin" in msg)
raised, msg = _refused(lambda: m.assert_confined_member("../evil", archive="canary.tar"))
record("planted positive: a traversing archive member REFUSES as traversing",
       raised and "traversing member path" in msg)
raised, msg = _refused(lambda: m.assert_confined_member("/abs/evil", archive="canary.tar"))
record("planted positive: an absolute archive member REFUSES as absolute",
       raised and "absolute member path" in msg)
raised, msg = _refused(lambda: m.assert_fetchable_url("http://plain.invalid/a"))
record("planted positive: a plaintext URL scheme REFUSES at the scheme pin",
       raised and "refusing URL scheme" in msg)
raised, msg = _refused(
    lambda: m.assert_fetchable_url("https://evil.invalid/a", allowed_hosts=frozenset({"good.invalid"}))
)
record("planted positive: an off-allow-list host REFUSES, naming the allow-list rule",
       raised and "not an origin this row names" in msg)
# The redirect pin is one build_opener ARGUMENT (fetch-engine-assets' own factoring note): losing
# it is a silent downgrade to urllib's default policy, and the suite tally (125) only catches a
# DELETED leg, never one neutered in place — so the installation is asserted here independently.
record("planted positive: the per-hop redirect pin is INSTALLED in the real opener",
       _probe(lambda: any(isinstance(h, m._PinnedRedirectHandler)
                          for h in m.build_pinned_opener(frozenset()).handlers)))
# The manifest→egress DERIVATION, asserted from OUTSIDE with this canary's own row (the [DECIDED]
# note's off-engines.lock-URL positive): the allow-list a real fetch runs under must be derived
# from the row's own URLs — the enforcement fn alone (probed above) cannot prove that binding,
# and a derivation neutered in place would keep the suite tally AND the fn probes green.
_ROW_TOML = """
[[engine]]
id = "canary-tool"
version = "1.0"
source_ref = "v1.0"
triples = ["x86_64-unknown-linux-gnu"]
kind = "staged-artifact"
upstream_url = "https://pin.invalid/canary-tool-1.0.tar.gz"
licence = "MIT"
linkage = "invoked"
acquisition = "prebuilt"
purl = "pkg:generic/canary-tool@1.0"
sha256 = "%s"
asset_sha256 = "%s"
prebuilt_corroboration = "mirrors"
corroboration_urls = ["https://mirror-a.invalid/sums", "https://mirror-b.invalid/sums"]
""" % ("a" * 64, "b" * 64)


def _derived_hosts():
    rows = m.parse_lock(_ROW_TOML, where="g24-canary-inline")
    return m.group_plans(rows, "x86_64-unknown-linux-gnu", Path("."))[0].allowed_hosts


record("derivation: FetchPlan.allowed_hosts is built from the row's OWN pin+corroboration hosts",
       _probe(lambda: _derived_hosts()
              == frozenset({"pin.invalid", "mirror-a.invalid", "mirror-b.invalid"})))
raised, msg = _refused(
    lambda: m.fetch_asset(
        "https://evil.invalid/x",
        opener=lambda *_a, **_k: (_ for _ in ()).throw(AssertionError("opener must not fire")),
        allowed_hosts=_derived_hosts(),
    )
)
record("planted positive: an off-row URL is refused UNDER THE DERIVED allow-list, before any I/O",
       raised and "not an origin this row names" in msg)

# --- 3. the canary's own leg count --------------------------------------------------------------
# (No section-level exception guard like g24-stage-engines' fixture block: every raising seam
# runs inside _refused's try, every tool-touching record() predicate inside _probe's, and a
# module-load failure tracebacks to exit 1 — fail-closed either way; the template's guard covers
# a fixture SECTION that builds real trees, which this canary deliberately has none of.)
record("the canary's own leg count is pinned (14 + this pin)", len(results) == 14)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-fetch-engine-assets] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
