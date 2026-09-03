#!/usr/bin/env python3
"""g24-stage-engines.py - the independent armed canary for `scripts/stage-engines` (P4.27's
planned L(-1) hand-off, landed 2026-08-31; G37's staging half, G24 discipline).

Two jobs, both from OUTSIDE the tool so a neutering edit to stage-engines cannot neuter its own
check (the P4.27 box's hand-off note):

1. RUN the tool's fixture-driven `--selftest` (its 124 legs have a CI runner: this file is
   discovered by `run-gate-selftests`, so the suite executes at L2 (diff-scoped canary) and L4
   (3-OS)) and PIN the tally at 124 - the host-stable-count claim, CI-checked. (35 at delivery,
   P4.27; 53 since P4.28/17808aa; 124 since P4.29/e9dcb14 - the universal lipo/merge legs; each
   bump is that box's pre-declared owner-acked tail, and the number is READ from the committed
   tree's own `--selftest` run, never carried from prose.)

2. The per-OS skip INVENTORY (the 2026-08-31 owner ruling: no silent skips - every OS-gated leg
   is loud, pinned, and genuinely executes on at least one CI platform). stage-engines records an
   OS-impossible leg as a labeled `(skipped - ...)` PASS; this canary asserts the skipped-name set
   per OS fail-closed: on POSIX exactly the junction trio is skipped and every symlink-gated leg
   RAN; on Windows the junction trio MUST run (`mklink /J` needs no privilege - a junction skip
   there is a real failure, not an environment fact), the symlink trio and the P4.29 universal
   symlink QUARTET each skip all-or-none (a privilege probe gates each group), and the two
   ANY-LINK legs MUST run (they plant a symlink where the host allows one and fall back to a
   junction - they skip only on a host that can express NEITHER shape, which no CI platform is).
   Any leg name carrying `(skipped` outside the declared names FAILS - a NEW silently-skipping
   leg cannot appear unnoticed. The 3-OS canary is what makes "every leg executes somewhere" a
   CI fact: ubuntu/macos run the symlink-gated legs, windows runs the junction-gated ones.

Plus independent planted positives against the tool's guards (own fixtures, never the tool's
`_fixture_tree`): the dash-prefix collision, the two-restored-versions refusal, the member-path
escape, the non-3.3.1 resource subdir, the ONE-slice universal refusal (P4.29 landed the
universal path and retired the old universal-triple-refused positive; its equivalent-strength
successor is asserted on the MESSAGE naming P4.29.1 + the absent slice - the near sibling
raiser is the VERSION-MISMATCH arm, which raises the same StagingError type through the same
stage_all call once both slices exist, so a type-only assert could pass on the wrong arm; an
engine absent from BOTH slices raises nothing at all - it is reported as absent),
the absent-cache-root refusal (structural), a missing member, the
check-mode no-write guarantee, the per-destination replace-vs-accumulate clear (the policy.xml
casualty class), the triple-keyed suffix + exec-bit properties, and the OS-conditional link
probes (POSIX: a RELATIVE escaping symlink - relative on purpose, since an absolute escape
target is refused by the absolute-symlink rule even with the containment guard deleted, the
same masking class - plus an absolute in-entry symlink; Windows: escaping junction +
ancestor-cycle junction; each a MUST-run probe on its OS, never a skip, message-discriminated
where a sibling rule could raise the same type; each guard is thereby independently covered on
the one platform its link shape is constructible on, every push, via the 3-OS runs). The suite
call AND the planted-positive section are both exception-guarded: an unhandled traceback out of
either is a NAMED failing leg, never a dead canary.

COUPLING, declared so it is planned and never a surprise mid-box hard-stop (the P4.56.3 pattern):
this file is L(-1)-caged while stage-engines is not, and it PINS the tally (124), the skip-name
inventory, and its OWN leg count - so any box that adds a `--selftest` leg to stage-engines (P4.30
relocation, P4.41 manifest, P4.51 assertions, the P5-P7 staging boxes) carries the matching
tally/inventory bump HERE as a pre-planned owner-acked L(-1) tail of that box. It also carries
the recorder-fidelity catcher (the 4a5f359-escalated _record force-green class - see section 3)
plus the independent suite verdict re-derived from _results (section 1).

Run:  python3 scripts/gate-selftests/g24-stage-engines.py   Exit 0 = every assertion held.
"""
import contextlib
import importlib.machinery
import importlib.util
import io
import os
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "stage-engines"
_loader = importlib.machinery.SourceFileLoader("stg", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("stg", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def _refused(fn) -> tuple[bool, bool, str]:
    """(raised, structural, message) for a callable expected to raise StagingError.

    Any OTHER exception is caught and reported as not-raised (with its text printed), so a
    neutered guard that lets a raw OSError through fails its leg BY NAME instead of killing the
    whole canary mid-run with an unreported traceback.
    """
    try:
        fn()
    except m.StagingError as e:
        return True, bool(e.structural), str(e)
    except Exception as e:  # noqa: BLE001 - diagnosability: a named FAIL beats a dead canary
        print(f"[g24-stage-engines] non-StagingError escaped: {type(e).__name__}: {e}")
        return False, False, f"{type(e).__name__}: {e}"
    return False, False, ""


_LINUX = "x86_64-unknown-linux-gnu"
_WIN = "x86_64-pc-windows-msvc"

# The declared OS-gated leg names, verbatim from stage-engines' selftest. A rename there without
# a matching edit here fails the inventory legs - deliberate: the skip surface is pinned.
POSIX_JUNCTION_SKIPS = {
    "fail-closed: junction escape leg (skipped - Windows-only)",
    "junction allow leg (skipped - junctions are Windows-only)",
    "junction cycle leg (skipped - junctions are Windows-only)",
}
WIN_SYMLINK_SKIPS = {
    "stage: symlink preserve leg (skipped - host cannot create symlinks)",
    "fail-closed: symlink escape leg (skipped - host cannot create symlinks)",
    "fail-closed: absolute-symlink leg (skipped - host cannot create symlinks)",
}
# The junction trio ALSO has an on-Windows fallback spelling ("mklink unavailable"); it may never
# fire on a supported host (mklink /J is unprivileged), so it is in the DECLARED set only for the
# subset check - the Windows must-run leg below fails if any junction-flavoured skip appears.
WIN_JUNCTION_FALLBACK_SKIPS = {
    "fail-closed: junction escape leg (skipped - mklink unavailable)",
    "junction allow leg (skipped - mklink unavailable)",
    "junction cycle leg (skipped - mklink unavailable)",
}
# The P4.29 universal symlink QUARTET: the same all-or-none shape as the trio (a symlink-privilege
# probe gates the group) - never skipped on POSIX, all four skipped on an unprivileged Windows.
WIN_UNIVERSAL_SYMLINK_SKIPS = {
    "universal staging: symlink-vs-file leg (skipped - host cannot create symlinks)",
    "universal staging: dir-vs-dirlink leg (skipped - host cannot create symlinks)",
    "universal staging: symlink-disagreement leg (skipped - host cannot create symlinks)",
    "universal staging: symlink-preserve leg (skipped - host cannot create symlinks)",
}
# The two ANY-LINK legs plant a symlink where the host allows one and fall back to a junction
# where it does not - declared (name-legitimate) but MUST-RUN on both CI platforms, since they
# skip only on a host that can express NEITHER link shape.
ANY_LINK_SKIPS = {
    "universal staging: escaping-link leg (skipped - host cannot create any link)",
    "universal staging: tree-walk descent leg (skipped - host cannot create any link)",
}
DECLARED_SKIP_NAMES = (POSIX_JUNCTION_SKIPS | WIN_SYMLINK_SKIPS | WIN_JUNCTION_FALLBACK_SKIPS
                       | WIN_UNIVERSAL_SYMLINK_SKIPS | ANY_LINK_SKIPS)

# --- 1. the full suite, its tally, and the per-OS skip inventory --------------------------------
print("[g24-stage-engines] running stage-engines --selftest ...")
# The suite runs under a CAPTURED stdout: stage-engines' _record PRINTS each leg as it is
# recorded, so the captured stream is PRODUCTION-TIME evidence a post-hoc in-place rewrite of
# _results cannot forge (the r4 review's third force-green shape: `_results[:] = [(n, True)...]`
# at the end of selftest() keeps the tally, rc=0 AND a green post-hoc read - only what was
# already printed still carries the [FAIL]). The stream is replayed below so the log keeps it.
_suite_out = io.StringIO()
try:
    with contextlib.redirect_stdout(_suite_out):
        rc = m.selftest()
    suite_crashed = ""
except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary, here as in _refused
    rc, suite_crashed = 1, f"{type(e).__name__}: {e}"
print(_suite_out.getvalue(), end="")
if suite_crashed:
    print(f"[g24-stage-engines] --selftest raised: {suite_crashed}")
record("the tool's --selftest completed without an unhandled exception", not suite_crashed)
record("the tool's full --selftest suite passes under the canary runner", rc == 0)
record("the leg tally is host-stable at 124 (the pinned count)", len(m._results) == 124)
# The INDEPENDENT verdict, derived from the stored entries rather than the tool's own
# aggregation: `rc` above comes from selftest()'s `return 1 if failed else 0`, which lives in
# the same un-caged function - force-greening THAT is a one-line edit the recorder-fidelity
# catcher below cannot see (it pins storage, not aggregation). This leg re-derives the verdict
# from _results itself, so both halves of the reporting path are pinned from outside.
record("independent verdict: every recorded suite leg is green (read from _results, never the "
       "tool's own aggregation)", all(ok for _, ok in m._results))
record(
    "production-time evidence: the captured suite stream carries PASS lines and NO [FAIL] line "
    "(what was printed at record time cannot be rewritten afterwards)",
    "[PASS]" in _suite_out.getvalue() and "[FAIL]" not in _suite_out.getvalue(),
)

skipped = {name for name, ok in m._results if "(skipped" in name}
record(
    "skip-inventory: every skipped leg is one of the declared OS-gated names "
    "(a NEW silent skip fails here)",
    skipped <= DECLARED_SKIP_NAMES,
)
if os.name == "nt":
    record(
        "skip-inventory (Windows): the junction trio RAN - mklink /J is unprivileged, "
        "a junction skip on Windows is a failure",
        not (skipped & (POSIX_JUNCTION_SKIPS | WIN_JUNCTION_FALLBACK_SKIPS)),
    )
    record(
        "skip-inventory (Windows): the symlink trio skips all-or-none "
        "(one privilege probe gates the trio)",
        len(skipped & WIN_SYMLINK_SKIPS) in (0, 3),
    )
    record(
        "skip-inventory (Windows): the P4.29 universal symlink quartet skips all-or-none "
        "(a privilege probe gates the group)",
        len(skipped & WIN_UNIVERSAL_SYMLINK_SKIPS) in (0, 4),
    )
    record(
        "skip-inventory (Windows): the two ANY-LINK legs RAN - a junction satisfies them, "
        "so a skip on Windows is a failure",
        not (skipped & ANY_LINK_SKIPS),
    )
else:
    record(
        "skip-inventory (POSIX): exactly the junction trio is skipped, nothing else",
        skipped == POSIX_JUNCTION_SKIPS,
    )
    record(
        "skip-inventory (POSIX): the symlink trio genuinely RAN",
        not (skipped & WIN_SYMLINK_SKIPS),
    )
    record(
        "skip-inventory (POSIX): the P4.29 universal symlink quartet genuinely RAN",
        not (skipped & WIN_UNIVERSAL_SYMLINK_SKIPS),
    )
    record(
        "skip-inventory (POSIX): the two ANY-LINK legs RAN - a symlink satisfies them, "
        "so a skip on POSIX is a failure",
        not (skipped & ANY_LINK_SKIPS),
    )

# --- 2. independent planted positives (own fixtures, public API) --------------------------------
fixture_crashed = ""
try:
    with tempfile.TemporaryDirectory() as tmp:
        base = Path(tmp)
        root, cache = base / "own-root", base / "own-cache"
        # The version deliberately carries a dash ("3.6-full"): the collision probe below adds a
        # declared engine "pandoc-3.6", and only a non-empty middle after BOTH prefixes makes the
        # entry genuinely claimable by both parses (engine "pandoc" @ "3.6-full" vs "pandoc-3.6" @
        # "full") - the first cut of this probe used a dash-free version and proved nothing.
        entry = cache / f"pandoc-3.6-full-{_LINUX}"
        entry.mkdir(parents=True)
        (entry / "pandoc").write_bytes(b"canary sidecar bytes")
        tree_entry = cache / f"libreoffice-25.2-{_LINUX}"
        (tree_entry / "program").mkdir(parents=True)
        (tree_entry / "program" / "soffice.bin").write_bytes(b"canary launcher")
        rows = (
            m.EngineStaging("pandoc", m.StagingKind.SIDECAR, "pandoc"),
            m.EngineStaging("libreoffice", m.StagingKind.RESOURCE_TREE, "libreoffice"),
        )

        staged = m.stage_all(root, cache, _LINUX, rows)
        sidecar = root / m.BINARIES_REL / f"pandoc-{_LINUX}"
        launcher = root / m.RESOURCES_REL / "libreoffice" / "program" / "soffice.bin"
        record(
            "independent positive: an outside-authored fixture stages to the two 3.3.1 shapes",
            len(staged) == 2 and sidecar.is_file() and launcher.is_file(),
        )
        record(
            "independent positive: the sidecar is triple-suffixed exactly as Tauri requires",
            sidecar.name == m.sidecar_filename("pandoc", _LINUX),
        )
        # The suffix keys off the TRIPLE, never the host: this leg and the next are each REAL on both
        # OSes (POSIX additionally proves the exec bit the tool must set on a non-Windows sidecar).
        win_entry = cache / f"pandoc-3.6-full-{_WIN}"
        win_entry.mkdir(parents=True)
        (win_entry / "pandoc").write_bytes(b"pe-ish bytes")
        m.stage_all(root, cache, _WIN, rows[:1])
        win_sidecar = root / m.BINARIES_REL / f"pandoc-{_WIN}.exe"
        record(
            "independent positive: a windows-triple stage lands .exe-suffixed on ANY host "
            "(suffix keys off the triple)",
            win_sidecar.is_file(),
        )
        record(
            "independent positive: the exec-bit/suffix property holds (POSIX: x-bit on the "
            "linux-triple sidecar; Windows: no .exe-suffixed linux-triple sidecar exists - the "
            "host-keyed-suffix regression, not a tautology over this canary's own constant)",
            bool(sidecar.stat().st_mode & 0o111) if os.name != "nt"
            else not (root / m.BINARIES_REL / f"pandoc-{_LINUX}.exe").exists(),
        )

        # The per-destination clear: shared rows ACCUMULATE while a re-stage still REPLACES stale
        # files - the guard whose per-ROW regression would silently drop e.g. ImageMagick's
        # policy.xml (the casualty stage-engines' own comment names). Independent probe: neutering
        # clear_resource_destinations leaves the stale file; a per-ROW clear drops libvips.so.
        for eng, lib in (("libvips", "libvips.so"), ("libheif", "libheif.so")):
            libdir = cache / f"{eng}-1.0-{_LINUX}" / "lib"
            libdir.mkdir(parents=True)
            (libdir / lib).write_bytes(b"image-stack component")
        shared_rows = (
            m.EngineStaging("libvips", m.StagingKind.RESOURCE_TREE, "image"),
            m.EngineStaging("libheif", m.StagingKind.RESOURCE_TREE, "image"),
        )
        m.stage_all(root, cache, _LINUX, shared_rows)
        stale = root / m.RESOURCES_REL / "image" / "lib" / "stale-from-the-old-pin.so"
        stale.write_bytes(b"stale")
        m.stage_all(root, cache, _LINUX, shared_rows)
        image_libs = sorted(p.name for p in (root / m.RESOURCES_REL / "image" / "lib").iterdir())
        record(
            "independent positive: the per-DESTINATION clear replaces stale files while shared rows "
            "accumulate (the policy.xml casualty class)",
            not stale.exists() and image_libs == ["libheif.so", "libvips.so"],
        )

        # --check writes NOTHING: a fresh root stays untouched (the no-write guard, independently).
        dry_root = base / "own-dry-root"
        m.stage_all(dry_root, cache, _LINUX, rows, write=False)
        record(
            "independent positive: write=False resolves + validates but creates nothing",
            not (dry_root / m.BINARIES_REL).exists() and not (dry_root / m.RESOURCES_REL).exists(),
        )

        # The colliding row names an EXISTING member ("pandoc") on purpose: with a bogus member, a
        # neutered collision guard would be masked by the missing-member raise and this leg would stay
        # green - proven by patching the revert (the guard is the ONLY raiser on this shape).
        raised, _, _ = _refused(
            lambda: m.stage_all(
                root, cache, _LINUX,
                rows + (m.EngineStaging("pandoc-3.6", m.StagingKind.SIDECAR, "pandoc"),),
                write=False,
            )
        )
        record("planted: the dash-prefix engine collision is refused (ambiguous_engine_entries)", raised)

        (cache / f"pandoc-3.5-{_LINUX}").mkdir()
        raised, _, _ = _refused(lambda: m.select_cache_entry(cache, "pandoc", _LINUX))
        record("planted: two restored pinned versions of one engine are refused", raised)
        (cache / f"pandoc-3.5-{_LINUX}").rmdir()

        raised, _, _ = _refused(lambda: m._member_path(tree_entry, f"../pandoc-3.6-full-{_LINUX}"))
        record("planted: a member path escaping its cache entry is refused", raised)

        raised, _, _ = _refused(lambda: m.resource_dest(root, "ghostscript"))
        record("planted: a resource subdir outside the closed 3.3.1 key set is refused", raised)

        # The P4.29 replacement for the obsolete universal-refused positive (P4.29 landed the
        # universal path, removing that boundary): the equivalent-strength fail-closed edge is a
        # cache holding ONE per-arch slice. Asserted on the MESSAGE because the SIBLING raiser is
        # near: the VERSION-MISMATCH arm raises the same StagingError type through the same
        # stage_all call once a second slice exists (one fixture edit away), so a type-only
        # assert could pass on the wrong arm. P4.29.1 is the ARM-EXCLUSIVE needle (the mismatch
        # message carries the triple too); the triple needle adds WHICH slice is absent. (An
        # engine absent from BOTH slices raises nothing - it is REPORTED as absent - so that
        # shape cannot mask anything; it would fail a raise-required leg outright.)
        (cache / "pandoc-3.6-full-aarch64-apple-darwin").mkdir()
        (cache / "pandoc-3.6-full-aarch64-apple-darwin" / "pandoc").write_bytes(b"arm64 bytes")
        raised, _, msg = _refused(
            lambda: m.stage_all(root, cache, m.UNIVERSAL_TRIPLE, rows[:1], write=False)
        )
        record(
            "planted: a ONE-slice cache is refused for a universal stage, the message carrying "
            "P4.29.1 + the absent slice's triple (message-discriminated)",
            raised and "P4.29.1" in msg and "x86_64-apple-darwin" in msg,
        )

        raised, structural, _ = _refused(
            lambda: m.stage_all(root, base / "no-such-cache", _LINUX, rows, write=False)
        )
        record("planted: an absent cache root is refused as STRUCTURAL (exit-2 semantics)",
               raised and structural)

        raised, _, _ = _refused(
            lambda: m.stage_all(
                root, cache, _LINUX,
                rows + (m.EngineStaging("libreoffice", m.StagingKind.SIDECAR, "soffice"),),
                write=False,
            )
        )
        record("planted: a declared member missing from its cache entry is refused", raised)

        # --- 3. the OS-conditional link probes: MUST run on every platform, never skip -------------
        # Two probes per OS, covering each link guard on the one platform its shape is constructible
        # on: POSIX covers the escape + absolute-symlink rules (every push, on the ubuntu/macos
        # legs); Windows covers the junction escape + ancestor-cycle rules (on the windows leg).
        outside = base / "outside-sentinel"
        outside.mkdir()
        (outside / "secret.txt").write_bytes(b"out-of-entry")
        program = tree_entry / "program"
        if os.name == "nt":
            junction = program / "escape.link"
            made = subprocess.run(
                ["cmd", "/c", "mklink", "/J", str(junction), str(outside)],
                capture_output=True, check=False,
            )
            probe_ready = made.returncode == 0
            raised, msg = False, ""
            if probe_ready:
                raised, _, msg = _refused(
                    lambda: m.stage_all(root, cache, _LINUX, rows, write=False)
                )
                junction.rmdir()
            record(
                "planted (Windows): an out-of-entry junction is refused by the CONTAINMENT rule "
                "itself (message-discriminated; the probe must succeed - no skip)",
                probe_ready and raised and "resolving outside the staged tree" in msg,
            )
            cycle = program / "cycle.link"
            made = subprocess.run(
                ["cmd", "/c", "mklink", "/J", str(cycle), str(tree_entry)],
                capture_output=True, check=False,
            )
            probe_ready = made.returncode == 0
            raised = False
            if probe_ready:
                raised, _, _ = _refused(
                    lambda: m.stage_all(root, cache, _LINUX, rows, write=False)
                )
                cycle.rmdir()
            record(
                "planted (Windows): a junction at its own ancestor is refused "
                "(the materialisation-loop guard; the probe must succeed - no skip)",
                probe_ready and raised,
            )
        else:
            link = program / "escape.link"
            # RELATIVE on purpose: an ABSOLUTE escape target is refused by the absolute-symlink rule
            # even with the containment guard deleted (the same masking class as the universal
            # probe's), so only a relative escape isolates the containment rule as the sole raiser -
            # found by the R2 review's own revert probe, closed with the message discrimination.
            link.symlink_to(Path("..") / ".." / ".." / "outside-sentinel" / "secret.txt")
            raised, _, msg = _refused(
                lambda: m.stage_all(root, cache, _LINUX, rows, write=False)
            )
            link.unlink()
            record(
                "planted (POSIX): a RELATIVE out-of-entry symlink is refused by the CONTAINMENT "
                "rule itself (message-discriminated)",
                raised and "resolving outside the staged tree" in msg,
            )
            absolute = program / "absolute.link"
            absolute.symlink_to(program / "soffice.bin")
            raised, _, _ = _refused(
                lambda: m.stage_all(root, cache, _LINUX, rows, write=False)
            )
            absolute.unlink()
            record(
                "planted (POSIX): an ABSOLUTE symlink, even in-entry, is refused "
                "(copytree preserves it verbatim - the dangling-bundle-path class)",
                raised,
            )

except Exception as e:  # noqa: BLE001 - same rationale as the suite guard
    fixture_crashed = f"{type(e).__name__}: {e}"
    print(f"[g24-stage-engines] the planted-positive section raised: {fixture_crashed}")
record("the planted-positive section completed without an unhandled exception", not fixture_crashed)

# --- 3. the recorder-fidelity catcher (the 4a5f359-escalated _record force-green class) ---------
# Forcing the tool's _record green disarms the whole suite at a PRESERVED tally, and every leg
# above reads _results, which would look healthy. Not closable from the un-caged tool; caged
# here: feed the recorder a FAIL and assert it is stored faithfully - a coerced-True or
# dropped-FAIL recorder reds this leg mechanically. (The [FAIL] line it prints is the probe's
# own deliberate entry, not a suite failure - the name says so.)
_PROBE_NAME = "g24 recorder-fidelity probe (a deliberate FAIL entry; not a suite failure)"
try:
    m._record(_PROBE_NAME, False)
    _fidelity = m._results[-1] == (_PROBE_NAME, False)
except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
    print(f"[g24-stage-engines] recorder probe raised: {type(e).__name__}: {e}")
    _fidelity = False
record("escalation catcher: the tool's recorder stores a FAIL faithfully "
       "(a force-green _record reds here)", _fidelity)

# --- 4. the canary's own leg count --------------------------------------------------------------
record("the canary's own leg count is pinned (27 + this pin)", len(results) == 27)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-stage-engines] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
