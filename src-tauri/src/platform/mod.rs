//! `crate::platform` — the §0.7 OS-abstraction leaf (depends on no other module): path handling,
//! volume detection (§2.14), the OS shims (§7.7 reveal-in-folder), and the §7.2.4 portable-build
//! executable-permission helper (`ensure_executable`, landed P1.17). The one allow-listed `unsafe`
//! FFI surface is the §2.1.2 Windows-only `windows-sys` extern set: the `FileRenameInformationEx`-class
//! no-replace move (`rename_noreplace_at`, P3.14) via `NtSetInformationFile` (ntdll), the §2.6.3
//! run-lock `LockFileEx` exclusive advisory-lock acquire (`acquire_exclusive_lock`, P3.21) + its
//! non-blocking startup-sweep liveness probe (`try_acquire_exclusive_lock`, P3.23), and the §2.14.3
//! cross-volume free-space re-check `GetDiskFreeSpaceExW` (`available_bytes`, P3.17 — built at its
//! §2.14.3 first-need, consumed by the §1.10/§2.14.4 preflight P4.72/P4.73 in a subsequent phase), and the
//! §2.7.2 FAT/exFAT-class "no-atomic-publish" detection (`lacks_atomic_publish_primitive`, P3.18 — the
//! proactive per-location divert heuristic §2.7.2 `location_status` folds in; Unix `statfs`, a Windows no-op),
//! and the §2.7.2 ephemeral-output classification (`is_ephemeral_output_dir`, P3.33 — the known-temp-dir
//! divert heuristic `location_status` folds in beside the FAT test; per-OS well-known temp roots).
//! The Unix renames
//! **and the §2.6.3 run-lock** ride safe `rustix` (`flock`; the §2.14.3 free-space read rides safe
//! `rustix::fs::statvfs`; the §2.7.2 FAT/exFAT detection rides safe `rustix::fs::statfs`), the §2.3 identity
//! reads ride safe `winapi-util`, the §0.9
//! kill rides `process-wrap` (example list corrected 2026-07-07, the P3.12 ruling); the remaining per-OS
//! helpers are authored by their consuming boxes (P3+).
//!
//! **The one `unsafe` allow (G29):** this file carries the module-inner `#![allow(unsafe_code)]` that
//! overrides the crate-root `#![deny(unsafe_code)]` — `src-tauri/src/platform/*.rs` is the sole entry in
//! `check-unsafe-policy`'s `ALLOWED_UNSAFE_MODULES`, so the core's entire `unsafe` surface is confined here,
//! each block carrying a `// SAFETY:` justification. Empty on Unix (the renames ride safe `rustix`).
#![allow(unsafe_code)]

use std::io;
use std::path::{Path, PathBuf};

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

/// §2.6.3 startup-sweep NON-BLOCKING liveness probe (Unix) — the try-lock the sweep (`crate::run::sweep_stale`,
/// P3.23) probes a FOREIGN run's `.lock` with. Unlike the blocking [`acquire_exclusive_lock`] (which a run
/// holds for its whole lifetime), this attempts an **immediate** exclusive `flock(LOCK_EX | LOCK_NB)` and
/// reports the outcome WITHOUT ever blocking (the app must stay responsive at startup, §2.6.3):
/// **`Ok(true)`** = the lock was FREE and is now momentarily held by this probe ⇒ the owning run is
/// **dead/crashed** ⇒ its scratch is reclaimable; **`Ok(false)`** = the non-blocking acquire was REFUSED
/// (`EWOULDBLOCK`) ⇒ a live owner still holds it ⇒ **keep** the scratch. The held lock is the SOLE §2.6.3
/// delete gate — never mtime/PID. The caller drops `file` immediately after, releasing any momentarily-taken
/// lock (so the sweep can then remove the dead dir). Safe `rustix` — no `unsafe` on Unix (the crate-root deny
/// holds). [Build-Session-Entscheidung: P3.23]
#[cfg(unix)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.6.3 startup-sweep non-blocking try-lock; its only caller is the P3.23 \
                  `crate::run::sweep_stale` liveness probe — itself dead in the production build until the \
                  §7.2 startup sequence wires the sweep — so rustc walks that dead-but-present caller and \
                  marks this callee used; `allow` (permissive) covers the transitive dead-ness through the P3 \
                  wiring window (the `acquire_exclusive_lock` pattern)."
    )
)]
pub(crate) fn try_acquire_exclusive_lock(file: &std::fs::File) -> io::Result<bool> {
    use rustix::fs::{flock, FlockOperation};
    use rustix::io::Errno;
    // Non-blocking exclusive acquire: success ⇒ the lock was free ⇒ the owning run is dead (reclaimable);
    // `EWOULDBLOCK` ⇒ a live owner holds it ⇒ keep. Any other errno is a genuine I/O failure, propagated so
    // the caller can decide conservatively (never delete on a guess).
    match flock(file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(true),
        Err(e) if e == Errno::WOULDBLOCK => Ok(false),
        Err(e) => Err(io::Error::from(e)),
    }
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

/// §2.6.3 startup-sweep NON-BLOCKING liveness probe (Windows leg of [`try_acquire_exclusive_lock`]).
/// `LockFileEx` with `LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY` (the immediate-fail flag the P3.21
/// blocking acquire deliberately omits) over the whole possible byte range: success ⇒ **`Ok(true)`** (the lock
/// was free ⇒ the owning run is dead ⇒ reclaimable); an immediate **`ERROR_LOCK_VIOLATION`** ⇒ **`Ok(false)`**
/// (a live owner holds it ⇒ keep). Any other OS error is propagated. The caller drops `file` immediately
/// after, releasing any momentarily-taken lock (Windows releases a handle's locks on close). The held lock is
/// the SOLE §2.6.3 delete gate — never mtime/PID. [Build-Session-Entscheidung: P3.23]
#[cfg(windows)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.6.3 startup-sweep non-blocking try-lock (Windows); its only caller is the P3.23 \
                  `crate::run::sweep_stale` liveness probe — itself dead in the production build until the \
                  §7.2 startup sequence wires the sweep — so rustc walks that dead-but-present caller and \
                  marks this callee used; `allow` (permissive) covers the transitive dead-ness through the P3 \
                  wiring window (the `acquire_exclusive_lock` pattern)."
    )
)]
pub(crate) fn try_acquire_exclusive_lock(file: &std::fs::File) -> io::Result<bool> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::ERROR_LOCK_VIOLATION;
    use windows_sys::Win32::Storage::FileSystem::{
        LockFileEx, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
    };
    use windows_sys::Win32::System::IO::OVERLAPPED;

    let handle = file.as_raw_handle();
    // A default (all-zero) OVERLAPPED locks the whole u64 range from offset 0 (Low|High = u32::MAX), the
    // canonical whole-file lock valid even on the 0-byte `.lock`. SAFE construction (no `mem::zeroed`).
    let mut overlapped = OVERLAPPED::default();
    // SAFETY: `handle` is the live file-owned OS handle (outlives the call); `&mut overlapped` is the default
    // `OVERLAPPED` above, valid for the call; `LockFileEx` touches only them (immediate-fail exclusive lock).
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let ok = unsafe {
        LockFileEx(
            handle,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            u32::MAX,
            u32::MAX,
            &mut overlapped,
        )
    };
    if ok != 0 {
        return Ok(true);
    }
    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(ERROR_LOCK_VIOLATION as i32) {
        return Ok(false);
    }
    Err(err)
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

/// §2.14.3/§2.14.4 the volume free-space read: the bytes still available to the CALLING user on the
/// filesystem that hosts `dir` (respecting per-user quotas where the OS enforces them). The §2.14.3 EXDEV
/// cross-volume fallback (`fs_guard::atomic_publish`, P3.17) calls this to re-check `final`'s volume against
/// the ~output-sized intermediate BEFORE the copy — that copy makes the output's bytes exist a SECOND time on
/// `final`'s volume (peak ~2× output), which the §1.10/§2.14.4 up-front preflight does NOT model, so this
/// at-use re-check is the bound (mirroring §2.7.2's late-divert "never assume it fits"). It is the SAME
/// primitive the §1.10 resource pre-flight & budgets engine (P4.72/P4.73, §2.14.4) reads for its
/// per-physical-volume grouping — built HERE at its §2.14.3 first-need, consumed there by that subsequent-phase
/// engine, so the free-space read has ONE home (the `crate::platform` OS-shim, the module doc). [Build-Session-Entscheidung: P3.17]
///
/// Per OS: Unix `statvfs(dir)` → `f_bavail × f_frsize` (blocks available to a non-privileged process × the
/// fragment size) via SAFE `rustix::fs::statvfs` (no `unsafe`); Windows `GetDiskFreeSpaceExW(dir, &free, …)` →
/// `lpFreeBytesAvailableToCaller` (the one `unsafe` FFI, this module's allow-listed surface, G29). No panic
/// (G4/G14) — a bad path / OS failure is a clean `io::Error` the §2.8 caller maps (never a silently-assumed
/// "fits"). `saturating_mul` on the Unix product never overflow-panics: a `u64 × u64` byte count on a real
/// volume is far below the ceiling, and saturation is the total-order-preserving cap (a would-be overflow reads
/// as "effectively unlimited free space", the safe direction for a "does it fit?" gate).
///
/// No `dead_code` attribute (the `rename_noreplace_at` pattern): its only in-crate caller is
/// `fs_guard::atomic_publish`'s §2.14.3 branch — itself dead-code-suppressed until the §2.1.1 write sequence
/// (P3.38) — and rustc walks an allow/expect-dead fn's body, marking this callee USED, so a `dead_code`
/// expectation here would be unfulfilled. Exercised directly by `available_bytes_tests`.
#[cfg(unix)]
pub(crate) fn available_bytes(dir: &Path) -> io::Result<u64> {
    // SAFE `rustix::fs::statvfs` (feature `fs`, already enabled for the P3.12 publish primitive) — no `unsafe`
    // on Unix (the crate-root `#![deny(unsafe_code)]` holds; this module's `allow(unsafe_code)` is inert on the
    // Unix leg). `f_bavail` is the blocks available to an UNPRIVILEGED process (NOT `f_bfree`, which counts the
    // root-reserved reserve a normal user cannot use); `f_frsize` is the fragment size. Their product is the
    // usable free bytes. [Build-Session-Entscheidung: P3.17]
    let vfs = rustix::fs::statvfs(dir).map_err(io::Error::from)?;
    Ok(vfs.f_bavail.saturating_mul(vfs.f_frsize))
}

/// Windows leg of [`available_bytes`] — `GetDiskFreeSpaceExW` reports `lpFreeBytesAvailableToCaller`, the free
/// bytes available to the calling user on `dir`'s volume (respecting disk quotas), exactly what the §2.14.3
/// re-check needs. See the Unix leg's doc for the full contract. [Build-Session-Entscheidung: P3.17]
#[cfg(windows)]
pub(crate) fn available_bytes(dir: &Path) -> io::Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    // A wide, NUL-TERMINATED path (`GetDiskFreeSpaceExW` takes a `PCWSTR`). `dir` is our own resolved
    // destination dir (§2.3.1), not untrusted input.
    let wide: Vec<u16> = dir
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut free_to_caller: u64 = 0;
    // SAFETY: `wide` is a valid NUL-terminated UTF-16 buffer that outlives the call; `&mut free_to_caller` is a
    // valid `u64` out-param; the two total-size out-params are null; `GetDiskFreeSpaceExW` writes only through it.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_to_caller,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(free_to_caller)
}

/// §2.1.2/§2.7.2 the FAT/exFAT-class "no atomic-publish primitive" detector (Unix; a Windows no-op) — the
/// PROACTIVE per-location planning heuristic that §2.7.2 `fs_guard::location_status` (P3.33) folds into its
/// verdict so §1.8 output planning (P3.37) can DIVERT a source whose destination filesystem offers NEITHER a
/// `RENAME_NOREPLACE`-class no-replace rename NOR hardlinks (`link()` → `EPERM`/`ENOTSUP`) — the canonical case
/// being FAT32/exFAT (the §2.14.2 portable-USB destination). On such a volume neither half of the §2.1 publish
/// has a mechanised implementation, so the item's output diverts to the hardlink-capable system disk (§2.7.3)
/// where the full §2.1 chain holds; the divert there carries `DivertReason::NoAtomicPublish` (§0.6), mapped by
/// the higher planning tier — NOT here. This leaf returns only the boolean signal, keeping `crate::platform`
/// (a §0.7 tier-3 leaf) free of any `crate::domain` dependency, exactly as the REACTIVE §2.1.2 third-fallback
/// arm returns `fs_guard::PublishOutcome::NoAtomicPublishSupport` and defers the `DivertReason` mapping upward.
///
/// **READ-ONLY detection [Decision: P3.18, 2026-07-07 — the `statfs`-class realization]:** a `statfs`-class
/// query that WRITES NO FILE, so it leaves no unreclaimable probe residue (the defect of the discarded
/// write-probe alternative). Per OS:
///  - **Linux:** `rustix::fs::statfs(dir)` → `StatFs.f_type` (the superblock magic) is classified by
///    [`is_fat_class_magic`] against { `MSDOS_SUPER_MAGIC` `0x4d44` — the FAT driver reports one magic for
///    FAT12/16/32 incl. vfat, `EXFAT_SUPER_MAGIC` `0x2011_BAB0` }. Both are PROJECT constants (see their defs):
///    rustix exposes only `PROC`/`NFS_SUPER_MAGIC`, and `libc` is not a direct dependency — a raw magic value
///    needs no crate.
///  - **macOS:** `rustix::fs::statfs(dir)` → `StatFs` = `libc::statfs`, whose public `f_fstypename: [c_char; 16]`
///    is classified by `is_fat_class_name` (plain code-span, not an intra-doc link — that classifier is
///    `#[cfg(target_os = "macos")]`, absent from this Linux-gated doc's compilation) against { `"msdos"`
///    (uniform for FAT12/16/32), `"exfat"` } — read THROUGH the rustix `StatFs` alias, so `libc` is never named.
///  - **Windows (and any other target): `Ok(false)`** — `MoveFileExW`-without-`MOVEFILE_REPLACE_EXISTING`
///    (§2.1.2) is a true create-only move on FAT/exFAT too, so a Windows FAT/exFAT destination keeps the §2.1
///    guarantee and is NEVER diverted for this reason (§2.7.2). The leg exists (mirroring [`ensure_executable`])
///    only so `location_status` can call this unconditionally without a per-OS `cfg`.
///
/// `Err` = the `statfs` read itself failed (a missing / vanished directory). The §2.7.2 caller (P3.33) treats
/// an `Err` as "heuristic indeterminate → do NOT proactively divert" (logged, §7.5), because the REACTIVE
/// §2.1.2 third-fallback publish arm (`PublishOutcome::NoAtomicPublishSupport`) remains the correctness
/// backstop for any FAT/exFAT this magic/name list misses (Decision P3.18 "list-miss honesty" — the `statfs`
/// list is the proactive heuristic, not the backstop). SAFE `rustix` on Unix — no `unsafe` (the crate-root
/// `#![deny(unsafe_code)]` holds); no panic (G4/G14). [Build-Session-Entscheidung: P3.18]
#[cfg(target_os = "linux")]
// [Test-Change: P3.33 — old-obsolete+new-correct, §2.7.2] `expect`→`allow`: P3.33's `location_status` now
// calls this detector, so the P3.18 dead-code EXPECTATION is obsolete; `allow` (permissive) is correct — a
// lint-attribute flip, not a real assertion change.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.7.2 proactive FAT/exFAT-class detector (P3.18); now CALLED by P3.33's \
                  `fs_guard::location_status` divert classification (which folds it with the writable/ephemeral \
                  tests) — itself unbuilt in production until §1.8/C4 wiring (P3.34+), so `allow` covers the \
                  transitive dead-ness through the P3 wiring window (the `ensure_executable` pattern); the \
                  magic_tests below exercise the classifier boundary."
    )
)]
pub(crate) fn lacks_atomic_publish_primitive(dir: &Path) -> io::Result<bool> {
    // SAFE `rustix::fs::statfs` (feature `fs`, already enabled for the P3.17 free-space read) — no `unsafe` on
    // Unix. `f_type` is the superblock magic; cast to `u64` for an arch-independent magic compare (`f_type` is
    // `c_long` — i64 on the shipped x86_64 Linux target — so this is a real i64→u64 cast, never a lint-tripping
    // identity cast). READ-ONLY: `statfs` writes nothing (Decision P3.18).
    let sfs = rustix::fs::statfs(dir).map_err(io::Error::from)?;
    Ok(is_fat_class_magic(sfs.f_type as u64))
}

/// macOS leg of [`lacks_atomic_publish_primitive`] — classify by `f_fstypename` (the fs type NAME), not a
/// superblock magic (BSD `statfs` carries the name; the Decision rules the name the reliable macOS signal). See
/// the Linux leg's doc for the full contract. [Build-Session-Entscheidung: P3.18]
#[cfg(target_os = "macos")]
// [Test-Change: P3.33 — old-obsolete+new-correct, §2.7.2] `expect`→`allow`: P3.33's `location_status` now
// calls this detector, so the P3.18 dead-code EXPECTATION is obsolete; `allow` (permissive) is correct — a
// lint-attribute flip, not a real assertion change.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.7.2 proactive FAT/exFAT-class detector (P3.18, macOS); now CALLED by P3.33's \
                  `fs_guard::location_status` divert classification — itself unbuilt until §1.8/C4 wiring \
                  (P3.34+), so `allow` covers the transitive dead-ness through the P3 wiring window (the \
                  `ensure_executable` pattern)."
    )
)]
pub(crate) fn lacks_atomic_publish_primitive(dir: &Path) -> io::Result<bool> {
    let sfs = rustix::fs::statfs(dir).map_err(io::Error::from)?;
    // `f_fstypename: [c_char; 16]` read THROUGH the rustix `StatFs` alias (= `libc::statfs`; `libc` never named).
    // NUL-terminated C string → `&str` WITHOUT `unsafe`: take bytes up to the first NUL, reinterpret each
    // `c_char` (ASCII fs-type names) to `u8`, and lossily map an invalid-UTF-8 name to `""` (no panic — the
    // crate-root `#![deny(clippy::unwrap_used)]` holds). READ-ONLY: `statfs` writes nothing (Decision P3.18).
    let bytes: Vec<u8> = sfs
        .f_fstypename
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    let name = std::str::from_utf8(&bytes).unwrap_or("");
    Ok(is_fat_class_name(name))
}

/// Windows (and any non-Linux/macOS target) leg of [`lacks_atomic_publish_primitive`]: always `Ok(false)`.
/// Windows' `MoveFileExW`-without-`MOVEFILE_REPLACE_EXISTING` (§2.1.2) is a true create-only move on FAT/exFAT,
/// so a Windows FAT/exFAT destination keeps the §2.1 guarantee and is NEVER diverted for `NoAtomicPublish`
/// (§2.7.2). Present (the [`ensure_executable`] precedent) only so `location_status` (P3.33) can call this
/// unconditionally without a per-OS `cfg`. [Build-Session-Entscheidung: P3.18]
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
// [Test-Change: P3.33 — old-obsolete+new-correct, §2.7.2] `expect`→`allow`: P3.33's `location_status` now
// calls this detector, so the P3.18 dead-code EXPECTATION is obsolete; `allow` (permissive) is correct — a
// lint-attribute flip, not a real assertion change.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "§2.7.2 FAT/exFAT-class detector (P3.18, Windows no-op); now CALLED by P3.33's \
                  `fs_guard::location_status` divert classification — itself unbuilt until §1.8/C4 wiring \
                  (P3.34+), so `allow` covers the transitive dead-ness through the P3 wiring window (the \
                  `ensure_executable` pattern)."
    )
)]
pub(crate) fn lacks_atomic_publish_primitive(_dir: &Path) -> io::Result<bool> {
    Ok(false)
}

/// The Linux FAT/exFAT superblock magics — PROJECT constants (rustix exposes only `PROC`/`NFS_SUPER_MAGIC`;
/// `libc` is not a direct dependency, so the raw values are inlined with their kernel-header citation).
/// `MSDOS_SUPER_MAGIC` = `0x4d44` (`include/uapi/linux/magic.h` — the FAT driver reports one magic for
/// FAT12/16/32, so vfat is covered); `EXFAT_SUPER_MAGIC` = `0x2011_BAB0` (`fs/exfat/exfat_fs.h`).
/// [Build-Session-Entscheidung: P3.18]
#[cfg(target_os = "linux")]
const MSDOS_SUPER_MAGIC: u64 = 0x4d44;
#[cfg(target_os = "linux")]
const EXFAT_SUPER_MAGIC: u64 = 0x2011_BAB0;

/// PURE §2.7.2 classifier (the testable core of the Linux [`lacks_atomic_publish_primitive`] leg + the G48
/// magic-boundary bound-firing target): is a `statfs` superblock magic one of the FAT/exFAT-class values that
/// lack BOTH a no-replace rename AND hardlinks? A real FAT/exFAT volume cannot be mounted on the CI runners, so
/// the classification boundary is proven HERE on the magic value directly (Decision P3.18). No I/O, no panic.
/// [Build-Session-Entscheidung: P3.18]
#[cfg(target_os = "linux")]
fn is_fat_class_magic(f_type: u64) -> bool {
    f_type == MSDOS_SUPER_MAGIC || f_type == EXFAT_SUPER_MAGIC
}

/// PURE §2.7.2 classifier (the testable core of the macOS [`lacks_atomic_publish_primitive`] leg + the G48
/// name-boundary bound-firing target): is a `statfs` `f_fstypename` one of the FAT/exFAT-class NAMES? `"msdos"`
/// is the uniform macOS name for FAT12/16/32; `"exfat"` is exFAT (case-sensitive — the kernel reports
/// lowercase). Proven at its boundaries in the tests (Decision P3.18). No I/O, no panic.
/// [Build-Session-Entscheidung: P3.18]
#[cfg(target_os = "macos")]
fn is_fat_class_name(fstype: &str) -> bool {
    matches!(fstype, "msdos" | "exfat")
}

/// §2.7.2 ephemeral-output classification: is `dir` inside a KNOWN-EPHEMERAL OS temp location the OS may
/// silently purge? Writing a conversion RESULT into such a place would silently lose the user's output, so
/// §2.7.2 treats an ephemeral destination like an unwritable one → **divert** (`DivertReason::Ephemeral` —
/// the §2.7.2 `location_status`, P3.33, folds this in beside the FAT/writable tests). Reading a SOURCE from a
/// temp dir is fine; only the OUTPUT diverts. The per-OS ephemeral roots (§2.7.2): every platform's
/// `std::env::temp_dir()` (Windows `GetTempPathW`, Unix `$TMPDIR`-or-`/tmp`) PLUS — Windows `%TEMP%`/`%TMP%`;
/// macOS `$TMPDIR` / `/tmp` / `/var/folders`; Linux `$TMPDIR` / `/tmp` / `/var/tmp` / `/run/user` (XDG
/// runtime). A dir is ephemeral iff its resolved path is at-or-under one of those roots (COMPONENT-wise
/// `starts_with`, so `/tmpfoo` is not under `/tmp`). Best-effort canonicalisation resolves a symlinked root
/// (macOS `/tmp` → `/private/tmp`); an absent/unreadable dir or root falls back to a LEXICAL compare —
/// `location_status` is a planning HINT, not a commitment (P3.36 re-checks at the real write). Panic-free
/// (the crate no-panic deny, G4/G14). [Build-Session-Entscheidung: P3.33]
pub(crate) fn is_ephemeral_output_dir(dir: &Path) -> bool {
    let target = canonical_or_lexical(dir);
    ephemeral_roots()
        .iter()
        .any(|root| target.starts_with(canonical_or_lexical(root)))
}

/// Best-effort canonical form of `p` for the §2.7.2 ephemeral prefix compare. Uses **`dunce::canonicalize`**
/// (the `fs_guard::resolve_identity` §2.3.1 choice — off-Windows a `std::fs::canonicalize` passthrough; on
/// Windows it strips the verbatim `\\?\` UNC prefix to the most-compatible NON-UNC form) so a canonicalised
/// EXISTING dir and the lexical fallback for a NOT-YET-CREATED one compare in the SAME form — a bare
/// `std::fs::canonicalize` returns the `\\?\`-verbatim form for the existing roots, whose `Path` prefix
/// component (`VerbatimDisk`) never `starts_with`-matches a plain-`Disk` lexical target.
///
/// **Not-yet-created dir (the correctness-critical case):** a §2.7.1 mode-2 user-chosen-root SUBTREE dir does
/// not exist at §1.8/C4 planning time, so `canonicalize(p)` fails. Falling straight back to the fully-lexical
/// `p` would MISS a temp subtree whose ancestor is symlinked (macOS `/tmp` → `/private/tmp`) or whose root
/// canonicalises differently — a false "not ephemeral" that lets a result be written into a purgeable dir the
/// P3.36 late-divert (write-FAILURE-only) can never rescue → silent data loss. So the nearest EXISTING
/// ANCESTOR is canonicalised and the not-yet-created tail re-appended, resolving to the SAME form the
/// canonicalised ephemeral roots use. No panic — every step is a fallible short-circuit, the fully-lexical
/// `p` the final fallback. [Build-Session-Entscheidung: P3.33]
fn canonical_or_lexical(p: &Path) -> PathBuf {
    if let Ok(real) = dunce::canonicalize(p) {
        return real;
    }
    // `p` does not exist yet (a not-yet-created subtree): canonicalise the nearest EXISTING ancestor + re-append
    // the not-yet-created tail, so a symlinked ancestor / verbatim-prefix root still matches the roots' form.
    for ancestor in p.ancestors().skip(1) {
        if let Ok(real) = dunce::canonicalize(ancestor) {
            return match p.strip_prefix(ancestor) {
                Ok(tail) => real.join(tail),
                Err(_) => real,
            };
        }
    }
    p.to_path_buf()
}

/// The §2.7.2 per-OS known-ephemeral temp roots (see [`is_ephemeral_output_dir`]). `std::env::temp_dir()` is
/// always included (the OS primary temp); the rest are the platform-specific well-known roots the primary
/// may not cover. Env-derived roots (`%TEMP%`/`%TMP%`/`$TMPDIR`) are read via `var_os` so a non-UTF-8 temp
/// path is kept verbatim, never lossily dropped. Only Win/macOS/Linux ship (§1), so exactly one cfg block is
/// active per build and `roots` is always mutated (no `unused_mut`). [Build-Session-Entscheidung: P3.33]
fn ephemeral_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    // The §2.7.2 detector READ-ONLY enumerates the OS primary temp root — the path is compared
    // against, never created/written/handed out as a work dir, so the temp-dir rule's
    // predictable-shared-path concern does not apply here. Statement-level (never macro-nested)
    // so the G29 rule SEES this use (the check-sast temp-dir macro-arg backstop bars the
    // semgrep-invisible `vec![..]` form). [Build-Session-Entscheidung: P3.33 — re-shaped by the
    // 2026-07-16 Co-Pilot backstop commit]
    // nosemgrep: rust.lang.security.temp-dir.temp-dir
    roots.push(std::env::temp_dir());
    #[cfg(windows)]
    for var in ["TEMP", "TMP"] {
        if let Some(v) = std::env::var_os(var) {
            roots.push(PathBuf::from(v));
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(v) = std::env::var_os("TMPDIR") {
            roots.push(PathBuf::from(v));
        }
        roots.push(PathBuf::from("/tmp"));
        roots.push(PathBuf::from("/var/folders"));
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(v) = std::env::var_os("TMPDIR") {
            roots.push(PathBuf::from(v));
        }
        for r in ["/tmp", "/var/tmp", "/run/user"] {
            roots.push(PathBuf::from(r));
        }
    }
    roots
}

#[cfg(test)]
mod ephemeral_tests {
    use super::is_ephemeral_output_dir;
    use std::path::Path;

    // §6.4.1 unit (G15) / §2.7.2: a real subdir of the OS temp root IS ephemeral — a writability-passing temp
    // destination §2.7.2 diverts so a silent OS purge never loses the user's output. Real-FS
    // (test-strategy §0.1): a real dir under `std::env::temp_dir()` gives the canonicalising prefix compare a
    // real target (and is exactly why `location_status`'s writable/unwritable legs use a NON-temp dir).
    #[test]
    fn a_temp_dir_subdir_is_classified_ephemeral() {
        let dir = tempfile::tempdir().expect("a real temp dir under the OS temp root");
        assert!(
            is_ephemeral_output_dir(dir.path()),
            "§2.7.2: a dir under the OS temp root ({:?}) is ephemeral → divert",
            dir.path()
        );
    }

    // §6.4.1 unit (G15) / §2.7.2: a dir NOT under any known temp root is NOT ephemeral — the crate source
    // root (`CARGO_MANIFEST_DIR`) is a real, canonicalisable, non-temp path (the CI workspace is never under
    // the OS temp root), so the negative branch is proven against a real directory, not a fabricated one.
    #[test]
    fn a_non_temp_dir_is_not_ephemeral() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(
            !is_ephemeral_output_dir(manifest),
            "§2.7.2: the crate source root ({manifest:?}) is not under any OS temp root → not ephemeral"
        );
    }

    // §6.4.1 unit (G15) / §2.7.2 + §2.7.1 mode-2 (REGRESSION guard): a user-chosen-root SUBTREE dir that does
    // NOT exist yet at §1.8/C4 planning time must STILL classify ephemeral when it is under an OS temp root —
    // else a result written there is silently purged (the P3.36 late-divert only catches write FAILURES, not
    // OS purges). This is the nearest-existing-ancestor canonicalisation: without it, a bare `canonicalize` of
    // the existing temp root returns a form (Windows `\\?\`-verbatim / macOS `/private/tmp`-symlink) that the
    // fully-lexical not-yet-created target never `starts_with`-matches — a false "not ephemeral" data-loss class.
    #[test]
    fn a_not_yet_created_subtree_under_the_temp_root_is_still_ephemeral() {
        let base = tempfile::tempdir().expect("a real temp dir under the OS temp root");
        let not_yet_created = base.path().join("sub").join("dir"); // never created on disk
        assert!(
            !not_yet_created.exists(),
            "precondition: the nested subtree dir does not exist yet"
        );
        assert!(
            is_ephemeral_output_dir(&not_yet_created),
            "§2.7.2: a not-yet-created subtree dir under the OS temp root ({not_yet_created:?}) is STILL ephemeral (nearest-existing-ancestor resolution)"
        );
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

// §2.14.3/§2.14.4 free-space read (G15) — cross-OS (`available_bytes` compiles on both). A STANDALONE
// `#[cfg(test)]` mod (clippy `is_cfg_test` recognises the test context, so the crate-root expect_used deny is
// lifted for the test's expect-calls); the per-OS behaviour is exercised on a REAL temp filesystem + a REAL
// statvfs/GetDiskFreeSpaceExW read (never mock the FS under test, test-strategy §0.1).
#[cfg(test)]
mod available_bytes_tests {
    use super::available_bytes;

    // §2.14.3/§2.14.4 (G15): the free-space read returns a plausible POSITIVE byte count for a real temp dir on a
    // real volume — so the §2.14.3 re-check has a live number to compare the intermediate against. A real writable
    // temp dir always has SOME free space; a 0 would signal the statvfs/GetDiskFreeSpaceExW read is broken (e.g.
    // f_bavail × f_frsize mis-multiplied, or the wrong out-param read on Windows).
    #[test]
    fn available_bytes_on_a_real_dir_is_positive() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let free = available_bytes(dir.path()).expect("§2.14.3: read the volume's free space");
        assert!(
            free > 0,
            "§2.14.3: a real writable temp dir reports a positive free-byte count (statvfs/GetDiskFreeSpaceExW)"
        );
    }

    // §2.8/G4/G14 (G15): a missing path is a clean Err (never a panic) — the §2.8 caller maps it, never a
    // silently-assumed "fits". Unix-gated: `statvfs(missing)` deterministically fails `ENOENT`; the Windows leg's
    // error path (`GetDiskFreeSpaceExW` → `last_os_error`) is a trivial map covered by the positive test's success
    // path + the §6.4.4 cross-OS matrix (a bare-metal `GetDiskFreeSpaceExW` on a nonexistent dir is not portably
    // deterministic — some builds resolve to the volume root — so it is not asserted per-push). The individual
    // `#[cfg(unix)]` fn inside a STANDALONE `#[cfg(test)]` mod keeps clippy's test-context recognition (only the
    // MODULE-level compound `cfg(all(test, unix))` trips `is_cfg_test`, the P1.17 trap). [Build-Session-Entscheidung: P3.17]
    #[cfg(unix)]
    #[test]
    fn available_bytes_on_a_missing_path_is_err() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let missing = dir.path().join("no-such-subdir");
        assert!(
            available_bytes(&missing).is_err(),
            "§2.8: a missing path is a clean Err (the §2.8 caller maps it), never a panic"
        );
    }
}

// §2.1.2/§2.7.2 FAT/exFAT-class detection (G15/G48 bound-firing, P3.18) — the pure magic/name classifiers
// proven at their BOUNDARIES + the impure `statfs` read smoke-tested on the REAL CI temp filesystem (ext4 on
// Linux, APFS on macOS, NTFS on Windows — none FAT/exFAT-class → `Ok(false)`). A real FAT/exFAT volume cannot
// be mounted on the CI runners, so the classification boundary is proven on the magic value / name directly
// (Decision P3.18 "magic/name-list boundary fixtures instead of probe-error fixtures"), and the read is
// exercised end-to-end on a real temp dir (never mock the FS under test, test-strategy §0.1). STANDALONE
// `#[cfg(test)]` (clippy `is_cfg_test` recognition, P1.17).
#[cfg(test)]
mod lacks_atomic_publish_primitive_tests {
    use super::lacks_atomic_publish_primitive;

    // §2.7.2 (G48 magic bound-firing): the Linux superblock-magic classifier fires ON exactly the FAT/exFAT
    // magics and off every neighbour + common non-FAT filesystem — so a real FAT/exFAT volume would divert and
    // an ext4/btrfs volume never spuriously would.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_magic_classifier_matches_only_fat_and_exfat() {
        use super::{is_fat_class_magic, EXFAT_SUPER_MAGIC, MSDOS_SUPER_MAGIC};
        // The two in-class magics fire (both via the named constant and its literal value).
        assert!(
            is_fat_class_magic(MSDOS_SUPER_MAGIC),
            "§2.7.2: MSDOS/vfat (0x4d44) is FAT-class"
        );
        assert!(
            is_fat_class_magic(EXFAT_SUPER_MAGIC),
            "§2.7.2: exFAT (0x2011BAB0) is FAT-class"
        );
        assert_eq!(
            MSDOS_SUPER_MAGIC, 0x4d44,
            "the MSDOS magic constant is 0x4d44"
        );
        assert_eq!(
            EXFAT_SUPER_MAGIC, 0x2011_BAB0,
            "the exFAT magic constant is 0x2011BAB0"
        );
        // Off-by-one boundaries + common non-FAT magics are NOT FAT-class (never a spurious divert).
        assert!(
            !is_fat_class_magic(0x4d43),
            "boundary: 0x4d43 (one below MSDOS) is not FAT-class"
        );
        assert!(
            !is_fat_class_magic(0x4d45),
            "boundary: 0x4d45 (one above MSDOS) is not FAT-class"
        );
        assert!(
            !is_fat_class_magic(0x2011_BAB1),
            "boundary: one above exFAT is not FAT-class"
        );
        assert!(
            !is_fat_class_magic(0xEF53),
            "ext2/3/4 (0xEF53) is not FAT-class"
        );
        assert!(
            !is_fat_class_magic(0x9123_683E),
            "btrfs (0x9123683E) is not FAT-class"
        );
        assert!(!is_fat_class_magic(0), "a zero magic is not FAT-class");
    }

    // §2.7.2 (G48 name bound-firing): the macOS `f_fstypename` classifier fires ON exactly the FAT/exFAT names
    // and off every neighbour + common macOS filesystem, case-sensitively (the kernel reports lowercase).
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_name_classifier_matches_only_msdos_and_exfat() {
        use super::is_fat_class_name;
        assert!(
            is_fat_class_name("msdos"),
            "§2.7.2: 'msdos' (FAT12/16/32) is FAT-class"
        );
        assert!(is_fat_class_name("exfat"), "§2.7.2: 'exfat' is FAT-class");
        // Boundaries + common macOS filesystems are NOT FAT-class.
        assert!(!is_fat_class_name("apfs"), "APFS is not FAT-class");
        assert!(!is_fat_class_name("hfs"), "HFS+ is not FAT-class");
        assert!(!is_fat_class_name(""), "an empty name is not FAT-class");
        assert!(
            !is_fat_class_name("msdo"),
            "boundary: a truncated 'msdo' is not FAT-class"
        );
        assert!(
            !is_fat_class_name("exfatx"),
            "boundary: 'exfatx' is not FAT-class"
        );
        assert!(
            !is_fat_class_name("MSDOS"),
            "case-sensitive: uppercase 'MSDOS' is not the kernel name"
        );
    }

    // §2.7.2 (G15/G31): the impure `statfs` read on the REAL CI temp filesystem (ext4/APFS/NTFS — none
    // FAT/exFAT-class) is a clean `Ok(false)`: the proactive heuristic does NOT fire. READ-ONLY (Decision P3.18):
    // the detection wrote nothing, so the temp dir is still empty afterwards.
    #[test]
    fn real_temp_dir_is_not_fat_class_and_writes_nothing() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let lacks = lacks_atomic_publish_primitive(dir.path())
            .expect("§2.7.2: the statfs read on a real temp dir succeeds");
        assert!(
            !lacks,
            "§2.7.2: a normal CI temp filesystem (ext4/APFS/NTFS) is NOT FAT/exFAT-class → no proactive divert"
        );
        assert_eq!(
            std::fs::read_dir(dir.path())
                .expect("read the temp dir")
                .count(),
            0,
            "Decision P3.18: the statfs detection is READ-ONLY — it left no probe residue"
        );
    }

    // §2.8/G4/G14: a missing directory is a clean `Err` (the §2.7.2 caller treats it as heuristic-indeterminate
    // and does not divert), never a panic. Unix-gated: `statfs(missing)` deterministically fails `ENOENT`; the
    // Windows leg is a const `Ok(false)` with no read to fail (its no-op contract is the positive test above).
    #[cfg(unix)]
    #[test]
    fn missing_dir_is_err_not_panic() {
        let dir = tempfile::tempdir().expect("create a real temp dir");
        let missing = dir.path().join("no-such-subdir");
        assert!(
            lacks_atomic_publish_primitive(&missing).is_err(),
            "§2.8: a missing directory is a clean Err (heuristic-indeterminate), never a panic"
        );
    }
}
