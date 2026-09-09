#!/usr/bin/env python3
"""_monotone_pin - the monotone leg-name pin shared by the tool canaries (2026-09-09).

Used by g24-compile-engine-asset.py, g24-fetch-engine-assets.py and g24-stage-engines.py - the
three caged canaries that run an un-caged tool's fixture-driven `--selftest` and pin what it ran.

WHY. Each of them used to pin the tool's leg COUNT exactly (`len(m._results) == N`). A Loop box
that added one leg to the un-caged tool therefore reddened the caged canary, and the bump was an
owner-acked L(-1) tail of every such box - five tails for one number across P4.27..P4.31, the
single largest per-box stop class (the owner's 2026-09-09 cage-by-direction decision). The exact
count was also weaker than it looked: a RENAME kept it green, and an add-plus-remove kept it
green.

WHAT. The canary pins the BLESSED LEG-NAME MULTISET instead: the sibling `g24-<tool>.legs` file,
one normalized leg name per line in record order, written by `--bless` on a green tree.
  * every blessed name must be present in the live run (multiset inclusion) - a REMOVED or
    RENAMED leg reds, on every OS (strictly stronger than the count);
  * an ADDED leg is reported as unblessed and never reds - the Loop adds legs freely, and the
    Co-Pilot re-blesses the set at each phase end (test-strategy §11.2), one owner-acked act per
    phase instead of one per box;
  * an absent or EMPTY blessed file is a failing pin - a vacuous inclusion is not a pin.
Names are NORMALIZED before comparison: the tools label an OS-impossible leg with a trailing
`(skipped: ...)` / `(skipped - ...)` suffix on the host that skips it, so the raw multiset is
not host-stable while the base names are - PROVIDED the skip arm spells the REAL leg's name plus
the suffix, never a placeholder of its own (the 2026-09-09 r1 P0: stage-engines' placeholders
would have left ten blessed names missing on POSIX; each canary now asserts every declared skip
name AND every skip literal in the tool's SOURCE normalizes to a blessed base - the source scan
reads the arms the running host never executes). The per-OS skip INVENTORY legs stay the police
of WHICH legs may skip where; this pin only asks that the leg still exists.

The `.legs` files are L(-1) with their canaries (`scripts/gate-selftests/**`). `run-gate-selftests`
discovers `*.py` only and skips `_`-prefixed files, so neither this module nor a `.legs` file runs
as a canary; plan-lint check 16 reads `*.py` only, so a gate id inside a leg NAME never registers
a data file as a self-test.

Bless (the Co-Pilot's phase-end act; refuses a red or empty suite):
  python3 scripts/gate-selftests/_monotone_pin.py --bless <tool>
      tool in {compile-engine-asset, fetch-engine-assets, stage-engines}
"""
from __future__ import annotations

import collections
import contextlib
import importlib.machinery
import importlib.util
import io
import re
import sys
from pathlib import Path

for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
TOOLS = ("compile-engine-asset", "fetch-engine-assets", "stage-engines")
# the two skip spellings the tools use: `... (skipped: reason)` and `... (skipped - reason)`
_SKIP_SUFFIX = re.compile(r"\s*\(skipped(?::| -) [^()]*(?:\([^()]*\)[^()]*)*\)\s*$")   # the FINAL balanced parenthetical
# a double-quoted literal carrying a skip suffix, as the tools spell it on a skip arm
_SKIP_LITERAL = re.compile(r'(["\'])([^"\'\n]*\(skipped(?::| -) [^"\'\n]*)\1')   # double- or single-quoted, one line


class MonotonePinError(Exception):
    """The blessed set is unusable (absent / empty / unreadable) - a failing pin, never a vacuous pass."""


def normalize(name: str) -> str:
    """(pure) The host-stable base name: the trailing skip suffix removed, outer whitespace stripped."""
    return _SKIP_SUFFIX.sub("", name).strip()


def legs_path(tool: str) -> Path:
    if tool not in TOOLS:
        raise MonotonePinError(f"unknown tool {tool!r} (expected one of {', '.join(TOOLS)})")
    return HERE / f"g24-{tool}.legs"


def load_blessed(tool: str, path: Path | None = None) -> list[str]:
    """The blessed names (record order, duplicates kept). Absent or empty is an error by design.
    `path` overrides the sibling file (the canaries' own legs drive the absent / empty arms with it)."""
    path = path or legs_path(tool)
    if not path.is_file():
        raise MonotonePinError(f"blessed set absent: {path.name} - run --bless {tool} on a green tree")
    names = [ln for ln in path.read_text(encoding="utf-8").split("\n") if ln.strip()]
    if not names:
        raise MonotonePinError(f"blessed set EMPTY: {path.name} - a vacuous pin is a failing pin")
    return names


def missing(blessed: list[str], live_names: list[str]) -> list[str]:
    """(pure) Blessed names the live run lacks (multiset difference over normalized names)."""
    return sorted((collections.Counter(blessed) - collections.Counter(normalize(n) for n in live_names)).elements())


def unblessed(blessed: list[str], live_names: list[str]) -> list[str]:
    """(pure) Live names not (yet) blessed - reported, never failed."""
    return sorted((collections.Counter(normalize(n) for n in live_names) - collections.Counter(blessed)).elements())


def check(tool: str, live_names: list[str]) -> tuple[list[str], list[str], list[str]]:
    """(blessed, missing, unblessed) for the tool's live run against its blessed set - the canaries' one
    call; raises MonotonePinError when the blessed set is unusable, so the fail-closed probe reds."""
    blessed = load_blessed(tool)
    return blessed, missing(blessed, live_names), unblessed(blessed, live_names)


def skip_literals(source: str) -> list[str]:
    """(pure) Every double-quoted skip-arm literal in a tool's SOURCE whose normalized base is non-empty -
    a bare suffix literal (`" (skipped: ...)"`, the concatenation idiom) is not a name and is ignored;
    comment lines are ignored. Host-independent: it reads the arms this host never runs."""
    out: list[str] = []
    for line in source.split("\n"):
        if line.lstrip().startswith("#"):
            continue
        for _quote, lit in _SKIP_LITERAL.findall(line):
            if normalize(lit):
                out.append(lit)
    return out


def unaccounted_skip_mentions(source: str) -> int:
    """(pure) `(skipped` mentions on non-comment lines that are NOT a recognized one-line quoted literal (a
    bare-suffix concatenation literal counts as recognized). Non-zero means a NEW spelling - an implicitly
    concatenated multi-line name, an f-string - that the scan would silently pass; the canaries pin zero."""
    unaccounted = 0
    for line in source.split("\n"):
        if line.lstrip().startswith("#"):
            continue
        # recognized: a literal with a base, or the concatenation idiom's bare suffix - which always starts
        # with the joining space (`" (skipped: ...)"`); a bare `"(skipped ..."` with no leading space is the
        # second half of an implicit multi-line concatenation and is NOT recognized.
        recognized = sum(1 for _quote, lit in _SKIP_LITERAL.findall(line) if normalize(lit) or lit[:1].isspace())
        unaccounted += line.count("(skipped") - recognized
    return unaccounted


def unblessed_skip_literals(source: str, blessed: list[str]) -> list[str]:
    """(pure) The skip-arm literals whose base is NOT a blessed name - a placeholder spelling its own name
    instead of the real leg's (the 2026-09-09 r1 P0), caught on every host because the SOURCE is read."""
    names = set(blessed)
    return [lit for lit in skip_literals(source) if normalize(lit) not in names]


def _load_tool(tool: str):
    # the NamedTuple-tool idiom the canaries use: SourceFileLoader + module_from_spec, NO sys.modules entry
    loader = importlib.machinery.SourceFileLoader(tool.replace("-", "_"), str(REPO / "scripts" / tool))
    module = importlib.util.module_from_spec(importlib.util.spec_from_loader(loader.name, loader))
    loader.exec_module(module)
    return module


def bless(tool: str, module=None, out: Path | None = None) -> int:
    """Run the tool's --selftest and write its normalized leg names as the blessed set. Refuses a red
    suite and an empty suite; writes LF + a trailing newline, UTF-8, byte-stable across hosts.
    `module` / `out` override the loaded tool and the sibling file (the canaries' own legs)."""
    if tool not in TOOLS:
        raise MonotonePinError(f"unknown tool {tool!r} (expected one of {', '.join(TOOLS)}) - no guessed path")
    module = module or _load_tool(tool)
    captured = io.StringIO()
    with contextlib.redirect_stdout(captured):
        rc = module.selftest()
    names = [normalize(n) for n, _ok in module._results]
    if rc != 0 or not all(ok for _n, ok in module._results):
        print(f"[_monotone_pin] refusing to bless {tool}: its --selftest is RED (rc={rc})", file=sys.stderr)
        return 1
    if not names:
        print(f"[_monotone_pin] refusing to bless {tool}: its --selftest recorded no legs", file=sys.stderr)
        return 1
    path = out or legs_path(tool)
    path.write_bytes(("\n".join(names) + "\n").encode("utf-8"))
    shown = path.relative_to(REPO).as_posix() if path.is_relative_to(REPO) else str(path)
    print(f"[_monotone_pin] blessed {len(names)} leg names -> {shown}")
    return 0


def main(argv: list[str]) -> int:
    if len(argv) == 2 and argv[0] == "--bless":
        return bless(argv[1])
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
