#!/usr/bin/env python3
"""g24-compile-engine-asset.py - the G24/G10 canary over `scripts/compile-engine-asset` (P4.28.1).

The sibling of g24-fetch-engine-assets.py, and the pre-declared owner act (c) of the P4.28.1
tail: `run-gate-selftests` discovers only `scripts/gate-selftests/*.py`, so WITHOUT this file
the harness's `--selftest` legs would execute in NO gate and NO workflow - the reason the
act was pre-declared with the box. Two jobs, both from OUTSIDE the tool so a neutering edit to
compile-engine-asset cannot neuter its own check:

1. RUN the tool's fixture-driven `--selftest` under a CAPTURED stdout (the tool's `_record`
   prints each leg since P4.29.1, so the stream is production-time evidence - section 1) and
   PIN the tally at 257 - the host-stable-count claim, CI-checked at L2 (diff-scoped canary)
   and L4 (3-OS). (208 legs at delivery, P4.28.1/072a021; 257 since P4.29.1/8affa89 - the
   cross-fallback + producing-step-arch legs; each bump is that box's pre-declared owner-acked
   tail, and the number is READ from the committed tree's own `--selftest` run, never carried
   from prose.)

2. The skip INVENTORY (the g24-stage-engines per-OS model, the 2026-08-31 owner ruling: no
   silent skips): the suite injects every subprocess (no compiler, no `gpg`, no network), so on
   POSIX every leg RUNS - zero skips allowed. On Windows only the SYMLINK TRIO may skip
   (creating a real symlink is an OS privilege, not a subprocess - the one thing injection
   cannot fake; the classifier itself, `link_target_escapes`, is a pure predicate asserted on
   every platform), and the allowed set is any SUBSET of the trio's three pinned names - the
   trio sits behind two independent privilege-shaped probes in the tool, so partial skips are
   legitimate variance there. ANY other "(skipped" name fails here and must arrive as a
   declared, owner-acked inventory edit, never quietly.

Plus independent planted positives driven against the tool's PUBLIC seams with their OWN inputs
(never the suite's fixtures, so a suite edit cannot mask them): five message-discriminated
raiser probes over the install-redirect guard + the archive-member confinement grammar; three
pure-predicate probes over the signature-verdict matchers and the symlink-escape classifier;
the manifest->build DERIVATION triple (an inline `engines.lock` + inline `engine-configure.toml`
pair of this canary's own, driven through the real `plan_compiles`, so the binding "the flags
that reach a build are the DECLARED line, keyed on the row id, and an undeclared or
doubly-declared source is refused" is asserted from outside - the enforcement fns alone cannot
prove it); and the TWO load-bearing seams the cage rulings lean on, driven END-TO-END rather
than as pure functions (the r1 finding of this act's own review: a tally cannot see a leg
neutered in place) - the restored-source RE-HASH (`resolve_inputs` refuses bytes that are not
the pinned `tarball_sha256`; a cache is not a trust boundary) and the signature-verdict
ENFORCEMENT pair (`verify_signature` with an injected runner: a clean tool exit whose output
never names the pinned key REFUSES, and the same call ACCEPTS once it does - so the matcher is
consumed, not decorative). Plus the two P4.29.1 guards, driven with this canary's OWN header
bytes, triples and tempdir (the pre-declared clause (b) of that box's owner tail - both live in
the un-caged tool, whose only other legs live in that same un-caged file, exactly the
force-green shape this discipline exists for): `resolve_build` must REFUSE an undeclared
`(host, target)` cross pair while still resolving the one declared pair and the native case,
and `assert_target_arch` must refuse BOTH its enumerated shapes - a wrong-architecture thin
Mach-O and an already-fat one - while passing a correct prefix (the no-over-fire direction).
Plus the two 4a5f359-escalated catchers: the recorder-fidelity leg
(a FAIL fed to the tool's `_record` must be stored faithfully - a force-green recorder reds
this caged canary at a preserved tally) and the `assert_self_contained` stub probe (a
duck-typed prefix whose walk yields an escaping symlink, no link privilege needed, with the
walk pattern pinned at "*" - a third cage-ruling-leaning seam, driven from outside).

COUPLING, declared so it is planned and never a surprise mid-box hard-stop: this file is
L(-1)-caged while compile-engine-asset is not, and it PINS the tally (257), the empty-skip
inventory, and its OWN leg count - any box that adds a `--selftest` leg to the tool (P4.34's
pull-forward, the first seam fill per its 2026-09-03 attribution ruling, and the
P5.1.1/P5.5.1/P5.9.1/P6.1.1/P7.17.1 compile boxes filling the configure seam) carries the
matching bump HERE as a pre-planned owner-acked L(-1) tail of that box.

Run:  python3 scripts/gate-selftests/g24-compile-engine-asset.py   Exit 0 = every assertion held.
"""
import contextlib
import importlib.machinery
import importlib.util
import io
import struct
import sys
import tempfile
from pathlib import Path
from types import SimpleNamespace

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "compile-engine-asset"
# The NamedTuple-tool g24 idiom (g24-stage-engines / g24-fetch-engine-assets are the models):
# SourceFileLoader + module_from_spec with NO sys.modules entry. The absence is LOAD-BEARING:
# compile-engine-asset deliberately uses NamedTuples, so this canary is also the standing proof
# the tool stays importable this way; registering the module here would quietly unenforce that.
# (The idiom is per-tool, not universal - g24-plan-lint, whose tool uses @dataclass, registers.)
_loader = importlib.machinery.SourceFileLoader("cea", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cea", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


# Resolved ONCE at module scope so a renamed exception class becomes a named FAIL below, never an
# AttributeError escaping _refused's own except clause. The tool has TWO refusal types by design:
# CompileError (a verification/build violation, exit 1) and StructuralError (an unusable
# prerequisite, exit 2) - each probe names the one its seam documents.
_COMPILE_ERROR = getattr(m, "CompileError", None)
_STRUCTURAL_ERROR = getattr(m, "StructuralError", None)


def _refused(fn, kind) -> tuple[bool, str]:
    """Did fn raise exactly the tool's own `kind`? A generic exception is a named FAIL, never a
    dead canary; a raise of the SIBLING refusal type fails the leg too (type-discrimination is
    the point - the two exit codes carry different contracts)."""
    try:
        fn()
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        if kind is not None and isinstance(e, kind):
            return True, str(e)
        return False, f"WRONG exception {type(e).__name__}: {e}"
    return False, ""


def _probe(fn) -> bool:
    """Evaluate a tool-touching record() predicate fail-closed: a renamed private attribute must
    become a named FAIL, never a bare traceback."""
    try:
        return bool(fn())
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        print(f"[g24-compile-engine-asset] predicate probe raised: {type(e).__name__}: {e}")
        return False


# --- 1. the full suite, its tally, and the strict skip inventory --------------------------------
print("[g24-compile-engine-asset] running compile-engine-asset --selftest ...")
# The suite runs under a CAPTURED stdout: compile-engine-asset's _record PRINTS each leg as it
# is recorded (the P4.29.1 Loop half, closing the bound the P4.29 tail RECORDED at this exact
# site: "the equivalent closure here requires a tool-side print - a Loop edit at the next
# --selftest-touching box" - P4.29.1 was that box), so the captured stream is PRODUCTION-TIME
# evidence a post-hoc in-place rewrite of _results cannot forge (the r4 review's third
# force-green shape: `_results[:] = [(n, True)...]` at the end of selftest() keeps the tally,
# rc=0 AND a green post-hoc read - only what was already printed still carries the [FAIL]).
# The stream is replayed below so the log keeps it.
_suite_out = io.StringIO()
try:
    with contextlib.redirect_stdout(_suite_out):
        rc = m.selftest()
    suite_crashed = ""
except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
    rc, suite_crashed = 1, f"{type(e).__name__}: {e}"
print(_suite_out.getvalue(), end="")
if suite_crashed:
    print(f"[g24-compile-engine-asset] --selftest raised: {suite_crashed}")
record("the tool's --selftest completed without an unhandled exception", not suite_crashed)
record("the tool's full --selftest suite passes under the canary runner", rc == 0)
record("the leg tally is host-stable at 257 (the pinned count)", _probe(lambda: len(m._results) == 257))
# The INDEPENDENT verdict from the stored entries: `rc` comes from selftest()'s own
# aggregation, which lives in the same un-caged tool - force-greening it is a one-line edit
# the recorder-fidelity catcher cannot see. Both reporting halves are pinned from outside.
record("independent verdict: every recorded suite leg is green (read from _results, never the "
       "tool's own aggregation)", _probe(lambda: all(ok for _, ok in m._results)))
record(
    "production-time evidence: the captured suite stream carries one [PASS] line per recorded "
    "leg and NO [FAIL] line (what was printed at record time cannot be rewritten afterwards; "
    "the per-leg COUNT bound also reds a reworded FAIL marker beside a rewrite - the r1 opus P2)",
    _probe(lambda: _suite_out.getvalue().count("[PASS]") == len(m._results))
    and "[FAIL]" not in _suite_out.getvalue(),
)

# The one declared skip set: the symlink trio, Windows-only, all-or-none on the single
# privilege probe. The EXACT names are pinned so a renamed or added skip is a named FAIL.
_SYMLINK_TRIO_SKIPPED = {
    base + " (skipped: symlink creation is unprivileged here)"
    for base in (
        "an escaping symlink in the BUILT tree is refused before publish",
        "compile_entry REFUSES to publish a tree the build pointed out of",
        "...and publishes nothing when it does",
    )
}


def _inventory_ok() -> bool:
    skipped = {name for name, _ok in m._results if "(skipped" in name}
    if sys.platform != "win32":
        return not skipped
    return skipped <= _SYMLINK_TRIO_SKIPPED


record(
    "skip-inventory (per-OS): POSIX runs every leg; Windows may skip only within the pinned "
    "symlink trio (two independent privilege probes, so a subset is legitimate) - any other "
    "skip name is a FAIL",
    _probe(_inventory_ok),
)

# --- 2. independent planted positives (own inputs, public seams) --------------------------------
record("the tool still exposes CompileError (the verification-violation refusal type)",
       _COMPILE_ERROR is not None)
record("the tool still exposes StructuralError (the unusable-prerequisite refusal type)",
       _STRUCTURAL_ERROR is not None)
raised, msg = _refused(lambda: m._assert_confined("../evil", "g24-canary.tar"), _COMPILE_ERROR)
record("planted positive: a traversing archive member REFUSES as traversing",
       raised and "refusing traversing member path" in msg)
raised, msg = _refused(lambda: m._refuse_install_redirect("--prefix=/g24", "g24-canary"),
                       _STRUCTURAL_ERROR)
record("planted positive: a flag that SETS the install prefix REFUSES as an ambiguity",
       raised and "an ambiguity, not an override" in msg)
raised, msg = _refused(
    lambda: m._refuse_install_redirect("-DCMAKE_TOOLCHAIN_FILE=g24.cmake", "g24-canary"),
    _STRUCTURAL_ERROR,
)
record("planted positive: a file-import flag REFUSES (a file can reset the prefix)",
       raised and "imports build settings from a file" in msg)
raised, msg = _refused(lambda: m._refuse_install_redirect("--libdir=..{prefix}", "g24-canary"),
                       _STRUCTURAL_ERROR)
record("planted positive: a GLUED `{prefix}` in an install value REFUSES (must START the value)",
       raised and "has to START the value" in msg)
raised, msg = _refused(lambda: m._refuse_install_redirect("--libdir=/usr/lib64", "g24-canary"),
                       _STRUCTURAL_ERROR)
record("planted positive: an absolute install-directory value REFUSES under the confinement rule",
       raised and "does not stay under the prefix this script publishes" in msg)

_FPR = "1234ABCD" * 5  # 40 hex chars - this canary's own pin, not a suite fixture's
record(
    "verdict probe: _gpg_validsig accepts the PRIMARY fingerprint (VALIDSIG's last field) and "
    "rejects one appearing only in an echoed PATH",
    _probe(lambda: m._gpg_validsig(f"[GNUPG:] VALIDSIG {'F' * 40} 2026-01-01 {_FPR}", _FPR))
    and not _probe(lambda: m._gpg_validsig(f"gpg: reading /tmp/{_FPR}.tar.gz.asc", _FPR)),
)
record(
    "verdict probe: _sq_names_key strips the echoed argv paths first (a fingerprint that appears "
    "ONLY in a path this script passed in must not verify)",
    not _probe(lambda: m._sq_names_key(f"checking /cache/{_FPR}.tar.asc: no match",
                                       _FPR, echoed=(f"/cache/{_FPR}.tar.asc",)))
    and _probe(lambda: m._sq_names_key(f"Authenticated signature, key {_FPR}", _FPR,
                                       echoed=("/cache/x.tar.asc",))),
)
record(
    "predicate probe: link_target_escapes classifies absolute / traversing / drive-relative as "
    "escaping and a plain in-tree relative as confined",
    _probe(lambda: m.link_target_escapes("/abs") and m.link_target_escapes("../up")
           and m.link_target_escapes("C:/x") and not m.link_target_escapes("sub/ok")),
)

# --- 3. the manifest->build DERIVATION triple (this canary's own inline manifests) --------------
_CANARY_FPR = "AAAA1111" * 5
_SRC_A = "https://src-a.invalid/canary-av-2.0.tar.gz"
_SRC_B = "https://src-b.invalid/canary-lib-1.0.tar.gz"


def _canary_row(ident: str, url: str) -> str:
    return (
        "[[engine]]\n"
        f'id = "{ident}"\n'
        'version = "2.0"\n'
        'cache_engine = "canary-av"\n'
        'cache_version = "2.0"\n'
        'triples = ["x86_64-unknown-linux-gnu"]\n'
        'kind = "staged-artifact"\n'
        'acquisition = "from-source"\n'
        f'upstream_url = "{url}"\n'
        "[engine.from_source]\n"
        f'tarball_sha256 = "{"c" * 64}"\n'
        f'signature_url = "{url}.asc"\n'
        f'signing_key_fingerprint = "{_CANARY_FPR}"\n'
        'verified_with = "gpg"\n'
        f'toolchain_digest = "sha256:{"2" * 64}"\n'
    )


_LOCK_TOML = (
    _canary_row("canary-av", _SRC_A)
    + _canary_row("canary-probe", _SRC_A)  # same tarball: ONE build produces both rows
    + _canary_row("canary-lib", _SRC_B)
)
_SEAM_TOML = """
["canary-av".configure.flags]
"canary-av" = ["--enable-canary", "--extra-cflags=-I{prefix}/include"]
"canary-lib" = ["--disable-frontend"]
["canary-av".configure.system]
"canary-av" = "configure"
"canary-lib" = "autotools"
"""


def _plans(seam_text: str):
    rows = m.parse_lock(_LOCK_TOML, where="g24-canary-lock")
    seam = m.parse_configure(seam_text, where="g24-canary-seam")
    return m.plan_compiles(rows, seam, "x86_64-unknown-linux-gnu", Path("."))


def _derivation_holds() -> bool:
    plans = _plans(_SEAM_TOML)
    if len(plans) != 1 or plans[0].cache_engine != "canary-av" or len(plans[0].sources) != 2:
        return False
    first, second = plans[0].sources
    return (
        plans[0].entry_name == "canary-av-2.0-x86_64-unknown-linux-gnu"
        and first.produces == ("canary-av", "canary-probe")
        and first.system == "configure"
        and "--enable-canary" in first.flags
        and second.row.id == "canary-lib"
        and second.system == "autotools"
    )


record("derivation: plan_compiles binds each source to ITS declared line, keyed on the row id, "
       "with one shared-tarball build producing both its rows",
       _probe(_derivation_holds))
raised, msg = _refused(
    lambda: _plans(
        '["other-group".configure.flags]\n"x" = ["--enable-x"]\n'
        '["other-group".configure.system]\n"x" = "configure"\n'
    ),
    _COMPILE_ERROR,
)
record("planted positive: a group with NO declared configure table is refused, never defaulted",
       raised and "refusing to compile with an undeclared configure line" in msg)
raised, msg = _refused(
    lambda: _plans(
        '["canary-av".configure.flags]\n"canary-av" = ["--enable-av"]\n'
        '"canary-probe" = ["--enable-probe"]\n"canary-lib" = ["--disable-frontend"]\n'
        '["canary-av".configure.system]\n'
        '"canary-av" = "configure"\n"canary-probe" = "configure"\n"canary-lib" = "autotools"\n'
    ),
    _COMPILE_ERROR,
)
record("planted positive: TWO declared lines for one shared tarball are refused as ambiguous",
       raised and "one tarball is one build" in msg)

# --- 4. two of the three cage-ruling-leaning seams, end-to-end - the third,
# assert_self_contained, is section 5's stub probe (this act's r1 review finding:
# a suite tally cannot see a leg neutered in place, so the load-bearing seams get their own
# outside-driven positives - the restored-source re-hash and the verdict ENFORCEMENT) ------------


def _rehash_probe() -> None:
    plans = _plans(_SEAM_TOML)
    with tempfile.TemporaryDirectory() as td:
        entry = Path(td)
        for source in plans[0].sources:
            (entry / source.archive_name).write_bytes(b"g24-not-the-pinned-bytes")
            (entry / source.signature_name).write_bytes(b"g24-signature")
        m.resolve_inputs(plans[0], entry)


raised, msg = _refused(_rehash_probe, _COMPILE_ERROR)
record("planted positive: a RESTORED source whose bytes do not hash to the pin REFUSES "
       "(a cache is not a trust boundary)",
       raised and "is not the pinned source" in msg)

_SIG_ROW = m.parse_lock(_canary_row("canary-av", _SRC_A), where="g24-canary-sig")[0]


def _verdict_runner(output: str):
    return lambda _argv, _cwd: SimpleNamespace(returncode=0, stdout=output, stderr="")


raised, msg = _refused(
    lambda: m.verify_signature(_SIG_ROW, Path("g24") / "a.tar.gz", Path("g24") / "a.tar.gz.asc",
                               runner=_verdict_runner("gpg: Good signature")),
    _COMPILE_ERROR,
)
record("enforcement probe: a CLEAN tool exit whose output never names the pinned key REFUSES "
       "(a valid signature by the WRONG key)",
       raised and "not against the pinned key" in msg)
record(
    "enforcement probe: the SAME call ACCEPTS once the verdict names the pinned key - the "
    "matcher is consumed by verify_signature, not decorative",
    _probe(lambda: m.verify_signature(
        _SIG_ROW, Path("g24") / "a.tar.gz", Path("g24") / "a.tar.gz.asc",
        runner=_verdict_runner(f"[GNUPG:] VALIDSIG {_CANARY_FPR} 2026-01-01 {_CANARY_FPR}"),
    ) is None),
)

# --- 4b. the two P4.29.1 guards (the pre-declared clause (b) of that box's owner tail): both
# live in the un-caged tool, whose only other legs live in that same un-caged file - exactly
# the force-green shape the canary discipline exists for. Driven with this canary's OWN header
# bytes, triples and tempdir, never the suite's fixtures. ----------------------------------------

# This canary's own Mach-O spellings (independent pins, never read from the tool - a tool whose
# constants drift from these no longer parses real Apple headers): MH_MAGIC_64 written
# little-endian, FAT_MAGIC + its 20-byte arch records written big-endian - each the file's own
# byte order, per the Mach-O ABI the tool documents.
_G24_ARM64 = 0x0100000C
_G24_X86_64 = 0x01000007


def _g24_thin(cpu: int) -> bytes:
    return struct.pack("<II", 0xFEEDFACF, cpu)


def _g24_fat(cpus: tuple[int, ...]) -> bytes:
    head = struct.pack(">II", 0xCAFEBABE, len(cpus))
    return head + b"".join(struct.pack(">IIIII", cpu, 0, 0, 0, 0) for cpu in cpus)


raised, msg = _refused(lambda: m.resolve_build("x86_64-apple-darwin", "aarch64-apple-darwin"),
                       _STRUCTURAL_ERROR)
record("P4.29.1 guard: an UNDECLARED (host, target) cross pair is refused, never built natively "
       "(driven with the REVERSE of the declared pair, so a direction-blind table reds too)",
       raised and "is not a declared cross pair" in msg)
record(
    "P4.29.1 guard (no over-fire): the one declared pair resolves to its CrossBuild and a "
    "same-triple build resolves native (None)",
    _probe(lambda: m.resolve_build("aarch64-apple-darwin", "x86_64-apple-darwin").target
           == "x86_64-apple-darwin")
    and _probe(lambda: m.resolve_build("x86_64-unknown-linux-gnu",
                                       "x86_64-unknown-linux-gnu") is None),
)


def _arch_probe(payload: bytes) -> None:
    with tempfile.TemporaryDirectory() as td:
        prefix = Path(td)
        bin_dir = prefix / "bin"
        bin_dir.mkdir()
        (bin_dir / "g24-tool").write_bytes(payload)
        m.assert_target_arch(prefix, "x86_64-apple-darwin", "g24-canary-entry")


raised, msg = _refused(lambda: _arch_probe(_g24_thin(_G24_ARM64)), _COMPILE_ERROR)
record("P4.29.1 guard: a thin Mach-O of the WRONG architecture under a per-slice key is refused "
       "(the silent-native class G30 exists for, one step upstream)",
       raised and "refusing to publish the wrong architecture under a per-slice key" in msg)
raised, msg = _refused(lambda: _arch_probe(_g24_fat((_G24_ARM64, _G24_X86_64))), _COMPILE_ERROR)
record("P4.29.1 guard: an already-FAT Mach-O under a per-slice key is refused on ITS arm "
       "(`lipo -create` would meet it as a duplicate architecture)",
       raised and "refusing to publish a fat binary under a per-slice key" in msg)


def _arch_clean() -> bool:
    with tempfile.TemporaryDirectory() as td:
        prefix = Path(td)
        bin_dir = prefix / "bin"
        bin_dir.mkdir()
        (bin_dir / "g24-tool").write_bytes(_g24_thin(_G24_X86_64))
        share = prefix / "share"
        share.mkdir()
        (share / "g24.txt").write_text("not a Mach-O", encoding="utf-8")
        return m.assert_target_arch(prefix, "x86_64-apple-darwin", "g24-canary-entry") is None


record("P4.29.1 guard (no over-fire): a correct-architecture prefix with non-Mach-O data passes "
       "the producing-step assertion",
       _probe(_arch_clean))

# --- 5. the two 4a5f359-escalated catchers: recorder fidelity + the assert_self_contained
# stub probe. A forced-green _record disarms the whole suite at a PRESERVED tally; not closable
# from the un-caged tool, so it is caged here - a FAIL fed to the recorder must be stored
# faithfully. (This tool's recorder PRINTS its entries since P4.29.1 - the section-1 stream leg
# is the production-time half; this leg pins STORAGE, the other reporting half. The [FAIL] line
# the probe prints below is its own deliberate entry, not a suite failure - the name says so.)
_PROBE_NAME = "g24 recorder-fidelity probe (a deliberate FAIL entry; not a suite failure)"
record(
    "escalation catcher: the tool's recorder stores a FAIL faithfully "
    "(a force-green _record reds here)",
    _probe(lambda: (m._record(_PROBE_NAME, False) or True)
           and m._results[-1] == (_PROBE_NAME, False)),
)

# The SECOND 4a5f359-escalated closure: `assert_self_contained` had no outside-driven probe (its
# coverage lived in the tool's own suite, and the r7 call-site leg patches the function away).
# 4a5f359 made the walk drivable through the entry OBJECT, so this stub needs no link privilege;
# the `patterns` pin catches a narrowed walk (`rglob("*.so")`) that every real fixture would miss.


class _EscapingEntry:
    name = "escape.link"

    def is_symlink(self) -> bool:
        return True

    def readlink(self) -> Path:
        return Path("../../outside/evil")


class _StubPrefix:
    def __init__(self) -> None:
        self.patterns: list[str] = []

    def rglob(self, pattern: str):
        self.patterns.append(pattern)
        yield _EscapingEntry()


_STUB = _StubPrefix()
raised, msg = _refused(lambda: m.assert_self_contained(_STUB, "g24-canary-entry"), _COMPILE_ERROR)
record(
    "escalation catcher: an entry whose walk yields an ESCAPING symlink is refused as not "
    "self-contained (stub-driven, no privilege needed; the walk pattern pinned at '*')",
    raised and "has to be self-contained" in msg and _STUB.patterns == ["*"],
)

# --- 6. the canary's own leg count --------------------------------------------------------------
# (No section-level exception guard: every raising seam runs inside _refused's try, every
# tool-touching predicate inside _probe's, and a module-load failure tracebacks to exit 1 -
# fail-closed either way; this canary builds no real trees.)
record("the canary's own leg count is pinned (29 + this pin)", len(results) == 29)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-compile-engine-asset] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
