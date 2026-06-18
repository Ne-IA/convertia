#!/usr/bin/env python3
"""g24-build-matrix.py - G24 self-test for check-build-matrix (P0.4.10, G30).

Proves the structural freeze (the native-OS / required-arch / suffix constants) cannot be weakened,
the fat-Mach-O slice assertion CATCHES the silent single-arch case (a `*-universal-apple-darwin`
binary carrying only one slice / a thin Mach-O / a non-Mach-O) and PASSES a real arm64+x86_64 fat
binary (both the 32-bit `FAT_MAGIC` and 64-bit `FAT_MAGIC_64` forms), and the live tier is
target-absent today (no sidecar staged). Synthetic Mach-O byte fixtures, stdlib-only. Exit 0 = held.
"""
import importlib.machinery
import importlib.util
import struct
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-build-matrix"
_loader = importlib.machinery.SourceFileLoader("cbm", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cbm", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


X86, ARM = m.CPU_TYPE_X86_64, m.CPU_TYPE_ARM64


def fat(cputypes, magic=m.FAT_MAGIC) -> bytes:
    """A synthetic fat Mach-O header with one fat_arch per cputype (slice bodies omitted - the gate
    reads only the header). 32-bit fat_arch (20 B) or 64-bit fat_arch_64 (32 B)."""
    out = struct.pack(">II", magic, len(cputypes))
    for ct in cputypes:
        out += (struct.pack(">IIIII", ct, 0, 0, 0, 0) if magic == m.FAT_MAGIC
                else struct.pack(">IIQQII", ct, 0, 0, 0, 0, 0))
    return out


def _freeze_with(**overrides) -> list[str]:
    saved = {k: getattr(m, k) for k in overrides}
    for k, v in overrides.items():
        setattr(m, k, v)
    try:
        return m.frozen_contract()
    finally:
        for k, v in saved.items():
            setattr(m, k, v)


# --- the structural freeze ---------------------------------------------------------------------
record("freeze: the real frozen constants are internally consistent", m.frozen_contract() == [])
record("freeze: dropping an OS from REQUIRED_NATIVE_OS is caught (no cross-compile, §6.1.4)",
       len(_freeze_with(REQUIRED_NATIVE_OS=frozenset({"windows", "macos"}))) >= 1)
record("freeze: dropping x86_64 from UNIVERSAL_ARCHES is caught (a missing slice crashes that arch)",
       len(_freeze_with(UNIVERSAL_ARCHES=frozenset({"arm64"}))) >= 1)
record("freeze: a suffix that is not the universal target triple is caught",
       len(_freeze_with(UNIVERSAL_SUFFIX="-apple-darwin")) >= 1)
record("freeze: a cputype->arch map not covering the required arch set is caught (branch isolated)",
       len(_freeze_with(_ARCH_NAME={m.CPU_TYPE_ARM64: "arm64"})) >= 1)

# --- the fat-Mach-O slice assertion (the parser) -----------------------------------------------
record("parse: a fat arm64+x86_64 binary -> both slices recognized",
       m.fat_macho_arches(fat([X86, ARM])) == {"x86_64", "arm64"})
record("parse: a 64-bit fat (FAT_MAGIC_64) arm64+x86_64 binary -> both slices",
       m.fat_macho_arches(fat([X86, ARM], magic=m.FAT_MAGIC_64)) == {"x86_64", "arm64"})
record("parse: a single-arch (arm64-only) fat binary -> only arm64",
       m.fat_macho_arches(fat([ARM])) == {"arm64"})
record("parse: a thin Mach-O (MH_MAGIC_64 0xFEEDFACF) -> None (not a fat binary)",
       m.fat_macho_arches(struct.pack(">I", 0xFEEDFACF) + b"\x00" * 28) is None)
record("parse: a non-Mach-O (random/text bytes) -> None",
       m.fat_macho_arches(b"#!/bin/sh\necho hi\n") is None)
record("parse: an implausible nfat_arch (a Java .class-shaped CAFEBABE) -> None (bounded)",
       m.fat_macho_arches(struct.pack(">II", m.FAT_MAGIC, 9999) + b"\x00" * 40) is None)
record("parse: a truncated arch table (claims 2 slices, body cut) -> None (cannot trust)",
       m.fat_macho_arches(struct.pack(">II", m.FAT_MAGIC, 2) + b"\x00" * 10) is None)
record("parse: too-short input (<8 bytes) -> None", m.fat_macho_arches(b"\xca\xfe") is None)

# --- check_sidecar (the per-file verdict) ------------------------------------------------------
P = Path("src-tauri/binaries/ffmpeg-universal-apple-darwin")
record("sidecar: a valid universal (arm64+x86_64) sidecar passes", m.check_sidecar(P, fat([X86, ARM])) == [])
record("sidecar: a single-arch (arm64-only) universal-NAMED sidecar is caught",
       len(m.check_sidecar(P, fat([ARM]))) == 1)
record("sidecar: a single-arch (x86_64-only) universal-NAMED sidecar is caught",
       len(m.check_sidecar(P, fat([X86]))) == 1)
record("sidecar: a THIN Mach-O named `*-universal-apple-darwin` is caught (the silent-crash class)",
       len(m.check_sidecar(P, struct.pack(">I", 0xFEEDFACF) + b"\x00" * 28)) == 1)
record("sidecar: a fat binary of only UNrecognized arches (no arm64/x86_64) is caught",
       len(m.check_sidecar(P, fat([0x0000000C]))) == 1)

# --- end-to-end over fixture roots -------------------------------------------------------------
with tempfile.TemporaryDirectory() as td:
    root = Path(td)
    bindir = root / "src-tauri" / "binaries"
    bindir.mkdir(parents=True)
    # target-absent: a non-universal sidecar (per-arch suffixed) is NOT scanned
    (bindir / "ffmpeg-aarch64-apple-darwin").write_bytes(fat([ARM]))
    record("e2e: only per-arch (non-universal) sidecars present -> target-absent skip (exit 0)",
           m.main(["--root", str(root)]) == 0)
    # a valid universal sidecar -> pass
    (bindir / "ffmpeg-universal-apple-darwin").write_bytes(fat([X86, ARM]))
    record("e2e: a valid universal sidecar staged -> exit 0", m.main(["--root", str(root)]) == 0)
    # a single-arch universal-named sidecar -> fail
    (bindir / "pandoc-universal-apple-darwin").write_bytes(fat([ARM]))
    record("e2e: a single-arch universal-named sidecar staged -> exit 1", m.main(["--root", str(root)]) == 1)

record("e2e: the real repo passes (no universal sidecar staged yet -> target-absent)", m.main([]) == 0)

passed = sum(1 for _, ok in results if ok)
print(f"\n[g24-build-matrix] {passed}/{len(results)} assertions passed.")
sys.exit(0 if passed == len(results) else 1)
