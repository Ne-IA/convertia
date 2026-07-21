//! The §6.4.2 / G48 bound-firing FIXTURE-CONTENT proof (`crate::fuzz_bounds`, P3.73) — a `#[cfg(test)]`
//! guard that the eight committed `fuzz/corpus/fs_guard_*/` bound-firing fixtures actually CONTAIN their
//! intended dangerous shape, so they cannot silently rot into decorative duds.
//!
//! WHY THIS EXISTS. G48's mandate is "bound-firing must be STRUCTURALLY proven, not fuzzer-hoped": a
//! committed `nul_path` fixture that (through a fat-fingered edit) no longer holds a NUL byte would still
//! pass `check-fuzz-contract` (which checks the file EXISTS by name) AND `crate::fuzz_replay` (which asserts
//! only no-panic — a non-NUL path is equally no-panic), so the guard it is supposed to fire would go
//! unexercised while everything reported green. This module closes that gap: it reads each committed fixture
//! and asserts its byte content is the real dangerous input (a NUL byte; an over-`PATH_MAX` length; a `\\.\`
//! device prefix; a reserved name; a drive-relative `C:x`; a `\\server\` UNC; a trailing dot/space; an
//! interior-NUL OUTPUT path). The per-platform "returns `Err`" behaviour those bytes drive is
//! `crate::fs_guard`'s own unit tests (P3.6/P3.8, over equivalent inline literals) + the `#[cfg(windows)]`
//! `resolve_identity` legs (P3.73 P0 ruling, item 5); the no-panic replay is `crate::fuzz_replay`. This
//! module is the third leg: the fixtures are what they claim to be.
//!
//! HOMING (P3.73 P0 ruling, 2026-07-21). The five `win_*` Windows dangerous-path fixtures live under
//! `fuzz/corpus/fs_guard_resolve_identity/` — NOT `.../fs_guard_is_safe_output/` — because those classes are
//! rejected by the untrusted-PATH fn `resolve_identity` (the P0.4.3 `G48_FUZZ_TARGETS` prose had mis-attributed
//! them to `is_safe_output`, which is a §2.3.3 no-clobber-onto-source verdict that correctly returns `Ok(Safe)`
//! for a drive-relative path with an empty frozen set). `is_safe_output`'s own bound-firing seed is
//! `nul_output_path` (an interior-NUL OUTPUT path → `InvalidInput` → its non-fallback `Err` arm).
//!
//! CROSS-PLATFORM. The assertions are byte-shape only (no filesystem call, no `cfg`), so they hold identically
//! on every OS — the shape of a dangerous path is universal even where the guard that rejects it is
//! Windows-specific. Homed at the crate-root foot (a FILE, never a directory — the §1a/§0.7 map is unchanged,
//! G69) beside `crate::fuzz_replay` for the same reason: `#[cfg(test)]`-only fuzz infrastructure that reads
//! the workspace-root `fuzz/` tree. [Build-Session-Entscheidung: P3.73]

use std::path::{Path, PathBuf};

/// The `fuzz/corpus/` root, anchored at the workspace root exactly as `crate::fuzz_replay` anchors it
/// (`src-tauri/` manifest dir → `../fuzz`), so a mis-anchored root reddens rather than silently finding
/// nothing.
fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fuzz/corpus")
}

// Each test reads its own committed fixture with a fixture-specific fail-message. The read uses the
// test-allowed unwrap-panic form (a bare panic macro, or an eager-format fail-arg, would trip the crate
// deny set or a clippy style lint); a fixture-shaped path-string names which fixture failed. A missing
// fixture is `check-fuzz-contract`'s ABSENCE job — here the fixture must be present AND hold its dangerous
// byte shape.

#[test]
fn nul_path_fixture_contains_an_interior_nul() {
    let bytes = std::fs::read(
        corpus_dir()
            .join("fs_guard_resolve_identity")
            .join("nul_path"),
    )
    .expect("§6.4.2/G48: the committed nul_path bound-firing fixture must exist and be readable");
    assert!(
        bytes.contains(&0),
        "the nul_path fixture must hold a NUL byte (the T7+T2a 'never Ok on a null-byte path' guard, §2.3.1)"
    );
}

#[test]
fn path_max_plus_1_fixture_exceeds_every_per_os_ceiling() {
    let bytes = std::fs::read(
        corpus_dir()
            .join("fs_guard_resolve_identity")
            .join("path_max_plus_1"),
    )
    .expect(
        "§6.4.2/G48: the committed path_max_plus_1 bound-firing fixture must exist and be readable",
    );
    // The largest per-OS total-path ceiling is Linux's PATH_MAX (4096, §2.2.3); a fixture longer than that
    // over-runs the limit on EVERY platform, so the length guard is reachable regardless of the CI leg.
    assert!(
        bytes.len() > 4096,
        "the path_max_plus_1 fixture must exceed the largest per-OS path ceiling (Linux PATH_MAX 4096); got {}",
        bytes.len()
    );
}

#[test]
fn win_device_fixture_is_a_device_namespace_path() {
    let bytes = std::fs::read(
        corpus_dir()
            .join("fs_guard_resolve_identity")
            .join("win_device"),
    )
    .expect("§6.4.2/G48: the committed win_device bound-firing fixture must exist and be readable");
    // The Windows device namespace prefix is `\\.\` or `\\?\` (0x5c 0x5c 0x2e|0x3f 0x5c).
    let device = bytes.starts_with(br"\\.\") || bytes.starts_with(br"\\?\");
    assert!(
        device,
        r"the win_device fixture must start with the device-namespace prefix \\.\ or \\?\ (§2.3.1)"
    );
}

#[test]
fn win_reserved_fixture_is_a_reserved_device_name() {
    let bytes = std::fs::read(
        corpus_dir()
            .join("fs_guard_resolve_identity")
            .join("win_reserved"),
    )
    .expect(
        "§6.4.2/G48: the committed win_reserved bound-firing fixture must exist and be readable",
    );
    // The stem before the first `.` must be a reserved DOS device name (CON/PRN/AUX/NUL/COM1-9/LPT1-9),
    // case-insensitive — `CON.jpg` opens the console device, not a file.
    let stem: Vec<u8> = bytes
        .split(|&b| b == b'.')
        .next()
        .unwrap_or(&[])
        .to_ascii_uppercase();
    let reserved: &[&[u8]] = &[
        b"CON", b"PRN", b"AUX", b"NUL", b"COM1", b"COM2", b"COM3", b"COM4", b"COM5", b"COM6",
        b"COM7", b"COM8", b"COM9", b"LPT1", b"LPT2", b"LPT3", b"LPT4", b"LPT5", b"LPT6", b"LPT7",
        b"LPT8", b"LPT9",
    ];
    assert!(
        reserved.contains(&stem.as_slice()),
        "the win_reserved fixture's stem must be a reserved DOS device name (got {stem:?}) (§2.3.1)"
    );
}

#[test]
fn win_drive_relative_fixture_is_drive_relative_not_absolute() {
    let bytes = std::fs::read(corpus_dir().join("fs_guard_resolve_identity").join("win_drive_relative"))
        .expect("§6.4.2/G48: the committed win_drive_relative bound-firing fixture must exist and be readable");
    // Drive-relative = `<letter>:` NOT followed by a separator (`C:foo` resolves against the drive's CWD,
    // unlike the absolute `C:\foo`), so a fixed output dir can be silently escaped. `.get()` (not `[]`) —
    // `indexing_slicing` is in the crate deny set.
    let drive_relative = bytes.first().is_some_and(u8::is_ascii_alphabetic)
        && bytes.get(1) == Some(&b':')
        && bytes.get(2).is_some_and(|&b| b != b'\\' && b != b'/');
    assert!(
        drive_relative,
        r"the win_drive_relative fixture must be `<letter>:` with NO following separator (drive-relative, not C:\, §2.3.1)"
    );
}

#[test]
fn win_unc_fixture_is_a_unc_share_path() {
    let bytes = std::fs::read(
        corpus_dir()
            .join("fs_guard_resolve_identity")
            .join("win_unc"),
    )
    .expect("§6.4.2/G48: the committed win_unc bound-firing fixture must exist and be readable");
    // UNC = `\\server\...` — two leading backslashes then a server name (NOT the `\\.\`/`\\?\` device forms).
    let unc = bytes.starts_with(br"\\") && bytes.get(2).is_some_and(|&b| b != b'.' && b != b'?');
    assert!(
        unc,
        r"the win_unc fixture must be a UNC path (\\server\share…, two leading backslashes + a server name) (§2.3.1)"
    );
}

#[test]
fn win_trailing_fixture_ends_with_a_dot_or_space() {
    let bytes = std::fs::read(
        corpus_dir()
            .join("fs_guard_resolve_identity")
            .join("win_trailing"),
    )
    .expect(
        "§6.4.2/G48: the committed win_trailing bound-firing fixture must exist and be readable",
    );
    // A trailing dot/space is silently stripped by the Win32 path layer, so `evil.txt.` can alias `evil.txt`.
    assert!(
        bytes.last().is_some_and(|&b| b == b'.' || b == b' '),
        "the win_trailing fixture must end with a trailing dot or space (§2.3.1)"
    );
}

#[test]
fn nul_output_path_fixture_contains_an_interior_nul() {
    // `is_safe_output`'s OWN bound-firing seed (P3.73 P0 ruling): an interior-NUL OUTPUT path resolves to
    // `InvalidInput`, which is NEITHER NotFound NOR NotADirectory, so is_safe_output takes its non-fallback
    // reject arm and returns `Err` (never `Ok(Safe)`) — the fixture must hold the NUL that fires it.
    let bytes = std::fs::read(
        corpus_dir()
            .join("fs_guard_is_safe_output")
            .join("nul_output_path"),
    )
    .expect(
        "§6.4.2/G48: the committed nul_output_path bound-firing fixture must exist and be readable",
    );
    assert!(
        bytes.contains(&0),
        "the nul_output_path fixture must hold a NUL byte (fires is_safe_output's non-fallback Err arm, §2.3.3)"
    );
}
