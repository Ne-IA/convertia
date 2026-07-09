//! `crate::platform` — the §0.7 OS-abstraction leaf (depends on no other module): path handling,
//! volume detection (§2.14), the OS shims (§7.7 reveal-in-folder), and the §7.2.4 portable-build
//! executable-permission helper (`ensure_executable`, landed P1.17). The one allow-listed `unsafe`
//! FFI surface is the §2.1.2 Windows-only `windows-sys` extern set: the `FileRenameInformationEx`-class
//! no-replace move (`rename_noreplace_at`, P3.14) via `NtSetInformationFile` (ntdll), and the §2.6.3
//! run-lock `LockFileEx` exclusive advisory-lock acquire (`acquire_exclusive_lock`, P3.21); the §2.14.4
//! `GetDiskFreeSpaceExW` free-space read joins at its own §2.14 box. The Unix renames **and the §2.6.3
//! run-lock** ride safe `rustix` (`flock`), the §2.3 identity reads ride safe `winapi-util`, the §0.9
//! kill rides `process-wrap` (example list corrected 2026-07-07, the P3.12 ruling); the remaining per-OS
//! helpers are authored by their consuming boxes (P3+).
//!
//! **The one `unsafe` allow (G29):** this file carries the module-inner `#![allow(unsafe_code)]` that
//! overrides the crate-root `#![deny(unsafe_code)]` — `src-tauri/src/platform/*.rs` is the sole entry in
//! `check-unsafe-policy`'s `ALLOWED_UNSAFE_MODULES`, so the core's entire `unsafe` surface is confined here,
//! each block carrying a `// SAFETY:` justification. Empty on Unix (the renames ride safe `rustix`).
#![allow(unsafe_code)]

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

/// §2.6.3 run-lifecycle EXCLUSIVE advisory-lock acquire (Unix) — the "held lock is the SOLE delete gate"
/// primitive the §2.6.3/§2.6.1 sweep relies on. `crate::run` (P3.21) opens `run-<RunId>/.lock`, calls this
/// to take a **blocking exclusive** lock, and **holds it for the whole run's lifetime** — the lock is
/// released automatically when the owning `File` handle is dropped/closed (Unix `flock` semantics), so a
/// crashed run's lock is provably free (⇒ dead ⇒ reclaimable) while a live run's is held (⇒ keep). The
/// run's `.lock` is a fresh, uniquely-named file (a fresh v4 `RunId`), so this uncontended acquire returns
/// immediately. rustix's **safe** `flock` — no `unsafe` on Unix (the crate-root deny holds); the
/// **non-blocking** try-lock the §2.6.3 startup sweep probes foreign locks with is P3.23's own primitive.
/// [Build-Session-Entscheidung: P3.21]
#[cfg(unix)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.6.3 run-start exclusive advisory-lock acquire; its only caller is the P3.21 \
                  run-lifecycle RunScratch::acquire — itself dead in the production build until its \
                  C6-accept run-start wiring lands (P3.46 / §2.1.1 write sequence P3.38) — and rustc walks \
                  that dead-but-present caller, marking this callee USED, so a dead_code EXPECTATION would \
                  be unfulfilled; `allow` (permissive) covers the transitive dead-ness through the P3 \
                  wiring window (the platform WindowsRenameOutcome pattern). The §2.6.3 sweep's non-blocking \
                  try-lock is the separate P3.23 primitive."
    )
)]
pub(crate) fn acquire_exclusive_lock(file: &std::fs::File) -> io::Result<()> {
    // Held for the run's lifetime; `flock` is released automatically when the fd is closed (drop of the
    // owning `File`), which is what makes "absent/free lock ⇒ dead ⇒ reclaimable" SAFE (§2.6.3). Safe
    // `rustix` — the one FFI-free lock path; no `unsafe` on Unix.
    rustix::fs::flock(file, rustix::fs::FlockOperation::LockExclusive).map_err(io::Error::from)
}

/// §2.6.3 run-lifecycle EXCLUSIVE advisory-lock acquire (Windows leg of [`acquire_exclusive_lock`]).
/// `LockFileEx` with `LOCKFILE_EXCLUSIVE_LOCK` (blocking, no `LOCKFILE_FAIL_IMMEDIATELY`) over the entire
/// possible byte range — a whole-file exclusive lock held until the owning `File` handle closes (Windows
/// releases a handle's locks on close), the same run-lifetime hold as the Unix leg. Uncontended (a fresh
/// unique `run-<RunId>/.lock`), so it returns immediately. [Build-Session-Entscheidung: P3.21]
#[cfg(windows)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.6.3 run-start exclusive advisory-lock acquire (Windows); its only caller is the P3.21 \
                  run-lifecycle RunScratch::acquire — itself dead in the production build until its \
                  C6-accept run-start wiring lands (P3.46 / §2.1.1 write sequence P3.38) — and rustc walks \
                  that dead-but-present caller, marking this callee USED, so a dead_code EXPECTATION would \
                  be unfulfilled; `allow` (permissive) covers the transitive dead-ness through the P3 \
                  wiring window (the platform WindowsRenameOutcome pattern). The §2.6.3 sweep's non-blocking \
                  try-lock is the separate P3.23 primitive."
    )
)]
pub(crate) fn acquire_exclusive_lock(file: &std::fs::File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{LockFileEx, LOCKFILE_EXCLUSIVE_LOCK};
    use windows_sys::Win32::System::IO::OVERLAPPED;

    let handle = file.as_raw_handle();
    // A default (all-zero) OVERLAPPED locks from offset 0; the whole u64 range (Low|High = u32::MAX) is the
    // canonical whole-file lock, valid even on the 0-byte `.lock` (byte-range locks may exceed EOF).
    // `OVERLAPPED` derives `Default` (Offset/OffsetHigh 0, hEvent null) — a SAFE construction, no `unsafe`
    // `std::mem::zeroed()` needed (mirroring the `IO_STATUS_BLOCK::default()` in `rename_noreplace_at`).
    let mut overlapped = OVERLAPPED::default();
    // SAFETY: `handle` is the live file-owned OS handle (outlives the call); `&mut overlapped` is the default
    // `OVERLAPPED` above, valid for the call; `LockFileEx` touches only them (blocking exclusive lock).
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let ok = unsafe {
        LockFileEx(
            handle,
            LOCKFILE_EXCLUSIVE_LOCK,
            0,
            u32::MAX,
            u32::MAX,
            &mut overlapped,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// §2.1.2/§2.3.3 the outcome of one Windows dir-handle-relative no-replace publish attempt
/// ([`rename_noreplace_at`], P3.14). Windows-only — `fs_guard::publish_rename_windows` (P3.14) maps it and
/// runs the §2.1.2 bounded AV-retry. Its own outcome type (like the Unix `fs_guard::PublishAttempt` /
/// `LinkPublishAttempt`), unified by the composite `atomic_publish` (P3.15+).
#[cfg(windows)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.1.2 rename_noreplace_at's outcome type (P3.14), constructed only by that fn — whose \
                  consumer is fs_guard::publish_rename_windows / the §2.1.1 write sequence (P3.15 / P3.38) — \
                  so it is dead-at-runtime during the P3 wiring window; `allow` (permissive) covers the \
                  ambiguous dead-ness. Exercised by rename_noreplace_at_tests."
    )
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsRenameOutcome {
    /// The no-replace move committed (`STATUS_SUCCESS`) — `leaf` now names the completed output (`tmp` was
    /// moved; never a 0-byte `final`).
    Renamed,
    /// `leaf` already exists (`STATUS_OBJECT_NAME_COLLISION`) — the move refused to clobber it (the SSOT
    /// never-harm guarantee); the §2.2.2 numbering loop (P3.15) re-picks, `tmp` untouched.
    TargetExists,
    /// A TRANSIENT lock (an AV scanner / indexer holding `tmp`) blocked the publish — `STATUS_ACCESS_DENIED` /
    /// `STATUS_SHARING_VIOLATION` at the NT move, or `ERROR_ACCESS_DENIED` / `ERROR_SHARING_VIOLATION` at the
    /// `tmp` open. The caller (`fs_guard::publish_rename_windows`) retries this with a bounded short-backoff
    /// before giving up to §2.8 `WriteFailed`; nothing was published, `tmp` untouched.
    Retryable,
}

/// §2.1.2/§2.3.3 the Windows dir-handle-relative, create-only publish primitive (never a 0-byte `final`, so no
/// empty name a crash could leave behind): atomically move `tmp` onto `leaf` RELATIVE to the P3.9-verified
/// parent dir handle `root_dir`, failing rather than replacing if `leaf` exists. The move is
/// `NtSetInformationFile(tmp, …, FileRenameInformationEx, FILE_RENAME_INFORMATION { Flags: 0, RootDirectory:
/// root_dir, FileName: leaf })` (ntdll) — the Ex-class `Flags` bitfield form (NOT the boolean `ReplaceIfExists`
/// of the non-Ex class) with `FILE_RENAME_REPLACE_IF_EXISTS` (0x1) omitted. Because the destination resolves
/// THROUGH the verified handle (not a re-parsed path string), the parent cannot be link-swapped between the
/// §2.3.3 verify and this publish (the §2.3.3 TOCTOU-closure).
///
/// **Why the NT API, not `SetFileInformationByHandle`** [Build-Session-Entscheidung: P3.14]: the Win32 shim
/// returns `ERROR_INVALID_PARAMETER` on a non-NULL `RootDirectory` HANDLE (verified locally), so the
/// RootDirectory-relative move the §2.3.3 TOCTOU-closure requires is available only via `NtSetInformationFile`
/// — exactly what spec §2.3.3 specifies.
///
/// **Outcome mapping (no panic):** `STATUS_SUCCESS` → [`WindowsRenameOutcome::Renamed`];
/// `STATUS_OBJECT_NAME_COLLISION` → [`WindowsRenameOutcome::TargetExists`] (re-pick, P3.15); the transient
/// `STATUS_ACCESS_DENIED` / `STATUS_SHARING_VIOLATION` (NT move) or `ERROR_ACCESS_DENIED` /
/// `ERROR_SHARING_VIOLATION` (`tmp` open) → [`WindowsRenameOutcome::Retryable`]; any other NTSTATUS maps
/// through `RtlNtStatusToDosError` to a §2.8 `io::Error`.
///
/// No `dead_code` attribute: its caller `fs_guard::publish_rename_windows` is itself allow-listed dead in the
/// P3-wiring window, and rustc walks an allowed-dead fn's body — marking this callee **used** — so a
/// `dead_code` expectation here would be unfulfilled. Exercised directly by `rename_noreplace_at_tests`.
#[cfg(windows)]
pub fn rename_noreplace_at(
    root_dir: std::os::windows::io::RawHandle,
    tmp: &Path,
    leaf: &std::ffi::OsStr,
) -> io::Result<WindowsRenameOutcome> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Wdk::Storage::FileSystem::{
        FileRenameInformationEx, NtSetInformationFile, FILE_RENAME_INFORMATION,
    };
    use windows_sys::Win32::Foundation::{
        RtlNtStatusToDosError, ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION, STATUS_ACCESS_DENIED,
        STATUS_OBJECT_NAME_COLLISION, STATUS_SHARING_VIOLATION, STATUS_SUCCESS,
    };
    use windows_sys::Win32::Storage::FileSystem::{DELETE, SYNCHRONIZE};
    use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

    // Open `tmp` with DELETE (the rename requires it) + SYNCHRONIZE (so `NtSetInformationFile` completes
    // synchronously on this non-overlapped handle). Safe std; the only `unsafe` is the FFI below. A transient
    // AV/indexer lock on `tmp` surfaces here as a Win32 SHARING_VIOLATION/ACCESS_DENIED → Retryable.
    let tmp_file = match std::fs::OpenOptions::new()
        .access_mode(DELETE | SYNCHRONIZE)
        .open(tmp)
    {
        Ok(f) => f,
        Err(e) if matches!(e.raw_os_error(), Some(c) if c == ERROR_ACCESS_DENIED as i32 || c == ERROR_SHARING_VIOLATION as i32) =>
        {
            return Ok(WindowsRenameOutcome::Retryable);
        }
        Err(e) => return Err(e),
    };

    // `leaf` → UTF-16, NO trailing NUL (`FileNameLength` is a BYTE count, not NUL-terminated).
    let name: Vec<u16> = leaf.encode_wide().collect();
    let name_bytes = name
        .len()
        .checked_mul(2)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "output leaf name too long"))?;
    let name_bytes_u32 = u32::try_from(name_bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "output leaf name too long"))?;
    // The kernel reads `size` meaningful bytes: the fixed header up to `FileName`, plus every name WCHAR.
    let size = std::mem::offset_of!(FILE_RENAME_INFORMATION, FileName)
        .checked_add(name_bytes)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "output leaf name too long"))?;
    let size_u32 = u32::try_from(size)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "output leaf name too long"))?;
    // Backing store: `size_of::<FILE_RENAME_INFORMATION>() + name_bytes` bytes, 8-byte-aligned via `Vec<u64>`
    // (matching the `HANDLE` field's alignment) so the `*mut FILE_RENAME_INFORMATION` cast is well-aligned and
    // the flexible `FileName[]` tail fits. Zeroed — the field-by-field writes below leave the inter-field
    // padding at that zero, so every byte the kernel reads within `size` is defined.
    let alloc = std::mem::size_of::<FILE_RENAME_INFORMATION>()
        .checked_add(name_bytes)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "output leaf name too long"))?;
    let mut buf = vec![0u64; alloc.div_ceil(std::mem::size_of::<u64>())];
    let info = buf.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();

    // SAFETY: `info` = zeroed, 8-byte-aligned `Vec<u64>` of `alloc` bytes (struct-aligned); each field is set in
    // place (padding stays zeroed) and `name.len()` WCHARs copied into `FileName[]` via `addr_of_mut!`, in-bounds.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    unsafe {
        (*info).Anonymous.Flags = 0; // no-replace: FILE_RENAME_REPLACE_IF_EXISTS (0x1) omitted
        (*info).RootDirectory = root_dir;
        (*info).FileNameLength = name_bytes_u32;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            std::ptr::addr_of_mut!((*info).FileName).cast::<u16>(),
            name.len(),
        );
    }

    let mut iosb = IO_STATUS_BLOCK::default();
    // SAFETY: `tmp_file`/`root_dir` handles are live; `info` = `size_u32` valid initialised bytes of the class-65
    // struct; the call keeps no pointer past it and completes synchronously (SYNCHRONIZE + non-overlapped).
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let status = unsafe {
        NtSetInformationFile(
            tmp_file.as_raw_handle(),
            &mut iosb,
            info.cast::<core::ffi::c_void>(),
            size_u32,
            FileRenameInformationEx,
        )
    };
    match status {
        STATUS_SUCCESS => Ok(WindowsRenameOutcome::Renamed),
        // The no-replace move refused an existing `leaf` — the SSOT never-harm guarantee (§2.1.2); re-pick.
        STATUS_OBJECT_NAME_COLLISION => Ok(WindowsRenameOutcome::TargetExists),
        // A transient AV/indexer lock on `tmp`/`leaf` — the caller retries (bounded), §2.1.2.
        STATUS_ACCESS_DENIED | STATUS_SHARING_VIOLATION => Ok(WindowsRenameOutcome::Retryable),
        // Any other NTSTATUS → a §2.8 `io::Error` via the NTSTATUS→Win32-code mapping.
        other => {
            // SAFETY: `RtlNtStatusToDosError` is a pure NTSTATUS→Win32-code mapping (no memory args).
            // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
            let win32 = unsafe { RtlNtStatusToDosError(other) };
            Err(io::Error::from_raw_os_error(win32 as i32))
        }
    }
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

// §6.4.1/§6.4.3 real-FS Windows (G15/G31) for the §2.1.2/§2.3.3 `rename_noreplace_at` FFI (P3.14) — the one
// `unsafe` surface. Never mock the FS under test (test-strategy §0.1): a REAL temp dir + a REAL directory
// HANDLE + the REAL `NtSetInformationFile` move. TWO STACKED cfg attrs (`#[cfg(test)]` then
// `#[cfg(windows)]`) — NOT a compound `all(test, windows)` (the P1.17 clippy `is_cfg_test` trap).
#[cfg(test)]
#[cfg(windows)]
mod rename_noreplace_at_tests {
    use super::{rename_noreplace_at, WindowsRenameOutcome};
    use std::ffi::OsStr;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;

    /// Open a real directory HANDLE — Windows requires `FILE_FLAG_BACKUP_SEMANTICS` to open a directory as a
    /// `File`. This is the `RootDirectory` the create-only rename resolves the leaf against (§2.3.3).
    fn dir_handle(dir: &Path) -> std::fs::File {
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(dir)
            .expect("open a directory handle")
    }

    // §2.1.2 (G15/G31): a fresh leaf renames — the tmp moves onto `leaf` relative to the dir handle, the bytes
    // land exact, and the tmp is gone (moved, no residual; never a 0-byte final).
    #[test]
    fn a_fresh_leaf_renames() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir_handle(dir.path());
        let tmp = dir.path().join("out.part");
        std::fs::write(&tmp, b"payload").expect("write the tmp");
        let outcome =
            rename_noreplace_at(root.as_raw_handle(), &tmp, OsStr::new("out.tsv")).expect("rename");
        assert_eq!(
            outcome,
            WindowsRenameOutcome::Renamed,
            "§2.1.2: a fresh leaf renames"
        );
        assert_eq!(
            std::fs::read(dir.path().join("out.tsv")).expect("read the leaf"),
            b"payload",
            "§2.1.2: the tmp's bytes land exact at the leaf, resolved through the dir handle"
        );
        assert!(!tmp.exists(), "§2.1.2: the tmp was moved (create-only)");
    }

    // §2.1.2 NO-HARM (G15/G31): an existing leaf → TargetExists (ERROR_ALREADY_EXISTS), never clobbered; the
    // existing file is byte-identical and the tmp is untouched.
    #[test]
    fn a_collision_reports_target_exists_and_never_clobbers() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir_handle(dir.path());
        let existing = dir.path().join("taken.tsv");
        std::fs::write(&existing, b"must survive").expect("write the existing target");
        let tmp = dir.path().join("out.part");
        std::fs::write(&tmp, b"new").expect("write the tmp");
        let outcome = rename_noreplace_at(root.as_raw_handle(), &tmp, OsStr::new("taken.tsv"))
            .expect("rename attempt");
        assert_eq!(
            outcome,
            WindowsRenameOutcome::TargetExists,
            "§2.1.2: an existing leaf is TargetExists (ERROR_ALREADY_EXISTS)"
        );
        assert_eq!(
            std::fs::read(&existing).expect("read the existing target"),
            b"must survive",
            "§2.1.2 no-harm: the existing target is byte-identical — the no-replace move NEVER clobbered it"
        );
        assert_eq!(
            std::fs::read(&tmp).expect("read the tmp"),
            b"new",
            "§2.1.2: the tmp is untouched on collision"
        );
    }

    // §2.1.2 a PERSISTENT lock on the tmp (a second handle NOT sharing DELETE, exactly as an AV scanner /
    // indexer holds) makes the DELETE-access open raise SHARING_VIOLATION → the primitive reports Retryable
    // (the caller then retries), NEVER a panic and NEVER a clobber.
    #[test]
    fn a_locked_tmp_reports_retryable() {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir_handle(dir.path());
        let tmp = dir.path().join("out.part");
        std::fs::write(&tmp, b"payload").expect("write the tmp");
        let blocker = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&tmp)
            .expect("hold a no-delete-share handle on the tmp");
        let outcome = rename_noreplace_at(root.as_raw_handle(), &tmp, OsStr::new("out.tsv"));
        drop(blocker);
        assert_eq!(
            outcome.expect("a locked tmp is a clean Retryable, not an Err"),
            WindowsRenameOutcome::Retryable,
            "§2.1.2: a no-delete-share lock on the tmp → Retryable (SHARING_VIOLATION at the DELETE-access open)"
        );
        assert!(
            !dir.path().join("out.tsv").exists(),
            "§2.1.2 no-harm: nothing was published on the retryable path"
        );
    }
}
