//! `crate::platform` — the §0.7 OS-abstraction leaf (depends on no other module): path handling,
//! volume detection (§2.14), the OS shims (§7.7 reveal-in-folder), and the §7.2.4 portable-build
//! executable-permission helper (`ensure_executable`, landed P1.17). The one allow-listed `unsafe`
//! FFI surface is the §2.1.2 Windows-only `windows-sys` extern set (the `FileRenameInfoEx`-class
//! no-replace move + `GetDiskFreeSpaceExW`, arriving with P3.14) — the Unix renames ride safe
//! `rustix`, the §2.3 identity reads ride safe `winapi-util`, the §0.9 kill rides `process-wrap`
//! (example list corrected 2026-07-07, the P3.12 ruling); the remaining per-OS helpers are authored
//! by their consuming boxes (P3+).

use std::io;
use std::path::Path;

/// §7.2.4 portable-build executable-permission setup (Unix). Files extracted from a portable archive
/// (the macOS `.zip` / the Linux AppImage) may lack the execute bit, and a bundled sidecar that is not
/// `+x` cannot be spawned. On every launch — **idempotently** — the core ensures each engine binary is
/// executable: when NO execute bit is set (`mode & 0o111 == 0`) the mode is widened to at least `0o755`
/// (`rwxr-xr-x`) and written back; an already-executable file is left **untouched** (the no-write fast
/// path — no needless metadata write on every launch). The first caller is the §7.2.1 step-4 startup
/// spine (P2) / the P4 engine staging; P1 lands the helper only.
///
/// [Build-Session-Entscheidung: P1.17] `pub(crate)` (the crate-internal OS-shim API): the §7.2.4
/// reference impl is module-private, but ConvertIA's call site is another module (the P2 spine), so it
/// is crate-visible here. The `not(test)` dead-code attribute below mirrors the `crate::domain`
/// identity-spine pattern — the unix test exercises the helper now, but it is dead in the non-test bin
/// build until the P2 spine calls it; using `#[expect]` rather than `#[allow]` auto-flags the moment a
/// real caller lands, so the annotation cannot silently outlive the scaffolding phase.
#[cfg(unix)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "§7.2.4 executable-permission helper; first caller is the P2 §7.2.1 step-4 startup spine / P4 engine staging (P1 lands the helper only)"
    )
)]
pub(crate) fn ensure_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(path)?.permissions();
    // Idempotent + no-harm: only touch the file when it carries no execute bit at all (§7.2.4). An
    // already-`+x` sidecar is left byte-for-byte — no needless `set_permissions` write on every launch.
    if perm.mode() & 0o111 == 0 {
        perm.set_mode(perm.mode() | 0o755);
        std::fs::set_permissions(path, perm)?;
    }
    Ok(())
}

/// §7.2.4 Windows leg: Windows has no execute-bit concept — a bundled `.exe` sidecar runs as-is — so
/// this is a deliberate **no-op**, present only so the P2/P4 call sites can invoke `ensure_executable`
/// unconditionally without a per-OS `cfg`. (SmartScreen is the analogous unsigned-build friction,
/// surfaced honestly on the §6.2.4 download page, not here.) [Build-Session-Entscheidung: P1.17]
#[cfg(not(unix))]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "§7.2.4 executable-permission helper (Windows no-op); first caller is the P2 §7.2.1 step-4 startup spine (P1 lands the helper only)"
    )
)]
pub(crate) fn ensure_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}

// Two separate cfg attributes (NOT `cfg(all(test, unix))`): clippy's `allow-expect-in-tests` only
// recognises a STANDALONE `#[cfg(test)]` as a test context (its `is_cfg_test` matches a single-item
// `cfg(test)`, not a compound `all(test, unix)`), so the compound form would wrongly trip the crate-root
// `#![deny(clippy::expect_used)]` on the test's expect-calls. [Build-Session-Entscheidung: P1.17]
#[cfg(test)]
#[cfg(unix)]
mod unix_tests {
    use super::ensure_executable;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn mode_of(p: &std::path::Path) -> u32 {
        std::fs::metadata(p)
            .expect("stat the test file")
            .permissions()
            .mode()
            & 0o777
    }

    // §6.4.2 fault-injection on a REAL temp filesystem (test-strategy §0.1: never mock the FS under
    // test): a non-executable extracted sidecar is made `+x` to at least 0o755, and a second call is a
    // no-op — the §7.2.4 "idempotent on every launch" contract read back from the real file.
    #[test]
    fn ensure_executable_sets_x_then_is_idempotent() {
        let dir = tempdir().expect("create a real temp dir");
        let bin = dir.path().join("sidecar");
        std::fs::write(&bin, b"#!/bin/sh\n").expect("write the fake sidecar");
        // Start non-executable: 0o644 (rw-r--r--), no execute bit at all.
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o644))
            .expect("set the initial non-executable mode");
        assert_eq!(
            mode_of(&bin) & 0o111,
            0,
            "precondition: the staged sidecar has no execute bit"
        );

        // First call widens to at least 0o755 (§7.2.4 `mode | 0o755`; 0o644 | 0o755 == 0o755).
        ensure_executable(&bin).expect("ensure_executable on a non-executable file");
        assert_eq!(
            mode_of(&bin),
            0o755,
            "§7.2.4: a non-executable sidecar is widened to 0o755 (rwxr-xr-x)"
        );

        // Idempotent: a second call leaves the now-executable file unchanged.
        ensure_executable(&bin).expect("ensure_executable is idempotent");
        assert_eq!(
            mode_of(&bin),
            0o755,
            "§7.2.4: a re-run leaves an already-executable file untouched"
        );
    }

    // §7.2.4 no-write fast path: a file already carrying an execute bit is left at its EXACT mode (the
    // `mode & 0o111 == 0` guard skips the write) — it is not needlessly widened to 0o755.
    #[test]
    fn ensure_executable_preserves_already_executable_mode() {
        let dir = tempdir().expect("create a real temp dir");
        let bin = dir.path().join("already-exec");
        std::fs::write(&bin, b"x").expect("write");
        // 0o700 already has the owner-execute bit → the guard must skip, preserving 0o700.
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o700))
            .expect("set an already-executable mode");
        ensure_executable(&bin).expect("ensure_executable on an already-executable file");
        assert_eq!(
            mode_of(&bin),
            0o700,
            "§7.2.4: an already-executable file keeps its exact mode (no needless widen/write)"
        );
    }

    // §2.8/§7.2.4 error path: a missing target surfaces a structured `Err` from the metadata read,
    // never a panic — the helper returns `io::Result`, and the §2.8 caller maps it to the taxonomy.
    #[test]
    fn ensure_executable_missing_file_is_err_not_panic() {
        let dir = tempdir().expect("create a real temp dir");
        let missing = dir.path().join("does-not-exist");
        assert!(
            ensure_executable(&missing).is_err(),
            "§7.2.4: a missing target is a clean Err (the §2.8 caller maps it), never a panic"
        );
    }
}

// Two separate cfg attributes (NOT `cfg(all(test, not(unix)))`) — same clippy `is_cfg_test`
// standalone-`cfg(test)` recognition reason as `unix_tests` above. [Build-Session-Entscheidung: P1.17]
#[cfg(test)]
#[cfg(not(unix))]
mod windows_tests {
    use super::ensure_executable;
    use std::path::Path;

    // §7.2.4 Windows leg: no execute-bit concept — `ensure_executable` is a no-op that always returns
    // Ok and never touches the path, so a bundled `.exe` sidecar runs as-is. Asserting the no-op keeps
    // the cross-platform call site honest (the P2 spine invokes it unconditionally).
    #[test]
    fn ensure_executable_is_ok_noop_on_windows() {
        // The no-op ignores its argument; even a non-existent path returns Ok (no metadata read).
        assert!(
            ensure_executable(Path::new("C:/nonexistent/sidecar.exe")).is_ok(),
            "§7.2.4: the Windows leg is a no-op that always succeeds (no execute-bit concept)"
        );
    }
}
