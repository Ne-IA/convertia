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
//! each block carrying a `// SAFETY:` justification. The Windows renames/locks/free-space ride the
//! `windows-sys` FFI, joined at **P4.17** by the §2.12.3 best-effort **Windows** privilege-drop tier —
//! Leg A the intermediate-integrity write confinement ([`label_confinement_sinks`] /
//! [`lower_child_to`] / [`strip_mandatory_label`]) and Leg B the own Job Object
//! ([`attach_confined_job`]), both applied parent-side on the still-suspended child; restricted-token /
//! AppContainer and the AppContainer/WFP net-deny are DECIDED unrealizable in the v1-portable build, so no
//! FFI for them exists here (the `no_appcontainer_or_spawn_token_ffi_in_the_core` source-scan pins it).
//! On **Linux** the §2.12.3 best-effort privilege-drop tier (P4.15, [`install_confinement`])
//! attaches its Landlock + seccomp legs through the one `unsafe` `CommandExt::pre_exec` closure (the safe
//! `landlock`/`seccompiler` crates do the syscalls inside it); macOS carries no `unsafe` — its renames ride
//! safe `rustix`, and the P4.16 macOS Seatbelt privilege-drop leg is **DECIDED cheap-tier only** (no apply,
//! no `unsafe` FFI): its only apply path is a private-libsandbox call in the post-fork/pre-exec child, which
//! is neither auditable fork-safe nor silent-skippable at its worst case (a hang, not an errno), so §2.12.3's
//! never-break floor forbids it — the same admission test that ADMITTED the Linux in-closure legs (Co-Pilot
//! ruling 2026-07-25, spec §2.12.3; macOS runs the P4.13 cheap-tier floor unconditionally).
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

// ============================================================================
// §2.12.3 best-effort Linux privilege-drop tier (P4.15) — Landlock FS-restrict
// (P4.15.1), network-namespace egress-deny (P4.15.2), seccomp-bpf exec/unexpected-
// syscall deny (P4.15.3). THREE INDEPENDENT, each-silent-degrading kernel-subsystem
// legs layered on the P4.13 cheap-tier floor. Best-effort defence-in-depth, NOT
// load-bearing (§0.11 T9b — the §3.5/§6.1.3 argv/build controls carry the guarantee):
// every leg degrades SILENTLY to the cheap tier where the kernel / portable build can't
// enable it and NEVER fails the conversion. All three legs live HERE (the sole
// `check-unsafe-policy` ALLOWED_UNSAFE_MODULES entry, G29) and attach as ONE `unsafe`
// `pre_exec` closure ([`install_confinement`]); `crate::isolation` stays
// `#![deny(unsafe_code)]`-clean and just calls it. Net-ns is applied via `libc::unshare`
// IN the post-fork child (single-threaded, where `CLONE_NEWUSER` is valid) rather than an
// `unshare(1)` argv-wrap — so a runtime namespace-setup failure just SKIPS the leg and the
// engine still runs (an argv-wrap would surface the failure as a non-zero exit = a broken
// conversion), and the identity uid/gid map is written BEFORE Landlock (which then needs
// only `/proc` read). The exact granted paths + the seccomp DENY-list are the §2.12.3
// `[DEFER: tuning]` residual; this box builds the tier MECHANISM. [Build-Session-Entscheidung: P4.15]
//
// PROFILE NOTE (§2.12.3 `[DEFER: tuning]`, refined by P4.24/P4.32/P4.37 + P5–P7): the
// Landlock read set grants the engine's own bundle dir (`program.parent()`) + the standard
// system runtime dirs so the decoder can launch + load its libs, and scratch (rw) for the
// output — but NOT yet a per-item INPUT-file grant: `run_confined` has no structured input
// path (it is flattened into `plan.args`), and NO real subprocess engine reads a real input
// through this seam before P4.37 (the imgworker wire) — so the `{input ro}` half is the
// real-engine consumer's additive grant, consistent with `run_subprocess` being production-
// dead until P4.32. Until then the leg is exercised by the `/bin/sh` integration tests below.
// ============================================================================

/// The applied-vs-degraded outcome of ONE §2.12.3 privilege-drop leg (P4.15) — INTERNAL
/// (no serde / IPC, `Debug` only; it never crosses the §0.4 wire). Consumed by the per-leg
/// tests (a Landlock failure does not imply a seccomp / net-ns failure, so each leg reports
/// independently — the P4.15 box's per-leg-independence contract) and, since **P4.18**, by
/// [`SpawnTier`] — the achieved-tier record every confined spawn hands out on
/// [`crate::engines::ConfinedRun::tier`]. That is the shaping choice P4.15 left to P4.18: the record
/// keeps the PER-LEG verdicts rather than collapsing them to one tier value, because the two Windows
/// legs degrade independently (a FAT/exFAT or SMB destination can leave Leg B applied while Leg A
/// degrades) and a collapsed value would hide exactly that.
///
/// `allow(dead_code)` (non-test), NOT `expect`: the reporter fn BODIES compile in the non-test build and
/// CONSTRUCT these variants, so an `expect` would flip to unfulfilled (the fs_guard forward-declared-item
/// precedent). The P4.18 production reader ([`crate::isolation::run_confined`] assembling the [`SpawnTier`])
/// exists, but rustc does not propagate liveness through a DEAD caller and that whole confined-spawn lane
/// stays dead until **P4.32** wires the subprocess dispatch arms — the same phenomenon the module-level
/// dead-code expectation in `crate::engines` records for `ConfinedRun` itself. The annotation therefore
/// stands until P4.32, then drops with its siblings.
///
/// Shared with the §2.12.3 WINDOWS tier (P4.17), whose Leg-A `lower_child_to` reports the same
/// three outcomes over the same grant-IS-the-enforcement model — one per-leg vocabulary across both
/// per-OS tiers, so the P4.18 record reads a single shape. It is deliberately declared on ALL platforms
/// (P4.18): macOS is DECIDED cheap-tier only (P4.16) and so constructs no verdict, but naming the same type
/// in the same [`SpawnTier`] shape everywhere is what lets one record, one accessor set and one test read
/// every platform — a macOS-only variant of the record would fork the shape the forward note asks to keep
/// single. `dead_code` is therefore allowed on macOS unconditionally (there is no leg to construct there),
/// where on Linux/Windows it is the non-test annotation above.
#[cfg_attr(any(not(test), target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LegOutcome {
    /// The leg's [`VERDICT_SOURCES`] reading says the restriction is in force (the
    /// grant-IS-enforcement model). Its STRENGTH is the source's: a `per-spawn` source answered about THIS
    /// spawn's own child, so `Applied` is a confirmation for that spawn; a `host-probe` source asked the
    /// kernel about this HOST, so `Applied` says the mechanism is in force here, not that this individual
    /// child received it (those legs' effect proofs are named on [`SpawnTier`]). Never read a bare `Applied`
    /// without its source.
    Applied,
    /// Silently degraded to the P4.13 cheap-tier floor; the reason distinguishes the classes.
    Degraded(DegradeReason),
}

/// Why a [`LegOutcome`] degraded. Declared on all platforms for the same single-shape reason as
/// [`LegOutcome`] (P4.18).
#[cfg_attr(any(not(test), target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DegradeReason {
    /// The restriction was never issued at all, so there is nothing to enforce — the OS feature is absent
    /// (Landlock ABI < 1 on a kernel < 5.13, seccomp unsupported on this arch/kernel), the mechanism could
    /// not be addressed (Windows, P4.17: the child's token or the confinement SID could not be obtained;
    /// P4.18: the child's namespace membership could not be read), or (P4.18) the leg's GRANT was never
    /// issued for this spawn — a Windows sink that could not be labelled, so the token was never lowered.
    /// The common thread is "not observed to be in force", never "observed to have failed".
    Unavailable,
    /// The grant call returned but did NOT enforce (Landlock `RulesetStatus::NotEnforced`; on
    /// Windows a `SetTokenInformation` whose integrity read-back does not show the requested
    /// level) — the "assert the grant applied, never assume it took" signal (P4.15.1 / P4.17).
    NotApplied,
}

// ============================================================================
// §2.12.3 ACHIEVED PRIVILEGE-DROP TIER — the G64 record's in-code half (P4.18)
//
// G64 (build-gates.md; the ratchet policy in docs/process/gate-status.md, P0.7.14) records the
// tier each platform ACHIEVES into the tracked `privilege-drop-coverage.toml` and guards it
// against a DECREASE, because the §2.12.3 tier is best-effort and silently degrades: a subsequent
// phase that quietly drops a platform from the privilege-drop tier to the cheap floor would
// otherwise be invisible (the T1 honest residual). This block is the half the code owns —
// the leg vocabulary the record is keyed by, the leg set THIS build attaches per platform, and
// the per-spawn verdict record. The `.toml` is the durable projection of it; the P4.18 record
// test binds the two so neither can drift from the other.
//
// [Build-Session-Entscheidung: P4.18] the record is keyed by STABLE STRING leg ids rather than a
// cross-platform enum: the legs are disjoint per OS (three on Linux, two on Windows, none on
// macOS), so one enum would carry variants that are unconstructible — and therefore dead — on
// every platform but one, while the ids have to survive verbatim into a `.toml` row anyway. The
// ids are `const`s, not literals at the use sites, so the record key and the verdict key are ONE
// decision that cannot drift.
// ============================================================================

/// The §2.12.3 tier NAMES the G64 `privilege-drop-coverage.toml` record uses, lowest first — the
/// ratchet's order (a platform moving from [`TIER_PRIVILEGE_DROP`] down to [`TIER_CHEAP`] is the NET
/// regression G64 exists to make visible). [`TIER_CHEAP`] is the §2.12.3 non-negotiable v1 floor that
/// ships unconditionally on all three OSes (P4.13). [Build-Session-Entscheidung: P4.18]
pub(crate) const TIER_CHEAP: &str = "cheap";

/// The §2.12.3 best-effort tier — reached when the platform attaches at least one privilege-drop leg
/// on top of the [`TIER_CHEAP`] floor. [Build-Session-Entscheidung: P4.18]
pub(crate) const TIER_PRIVILEGE_DROP: &str = "privilege-drop";

/// Leg id — the P4.15.2 network-namespace egress-deny leg (Linux).
#[cfg(target_os = "linux")]
pub(crate) const LEG_NETNS: &str = "netns";
/// Leg id — the P4.15.1 Landlock fs-restrict leg (Linux).
#[cfg(target_os = "linux")]
pub(crate) const LEG_LANDLOCK: &str = "landlock";
/// Leg id — the P4.15.3 seccomp-bpf syscall-deny leg (Linux).
#[cfg(target_os = "linux")]
pub(crate) const LEG_SECCOMP: &str = "seccomp";
/// Leg id — the P4.17 Leg-A intermediate-integrity write confinement (Windows).
#[cfg(windows)]
pub(crate) const LEG_INTEGRITY: &str = "integrity";
/// Leg id — the P4.17 Leg-B own kill-on-job-close Job Object (Windows).
#[cfg(windows)]
pub(crate) const LEG_JOB: &str = "job";

/// The §2.12.3 privilege-drop legs THIS build attaches on the current platform, in apply order — the
/// MECHANISM set the G64 record pins per platform. It is a compile-time fact, not a host reading: a
/// commit that removes a leg from the code changes this slice, and the P4.18 record test then reddens
/// against the unchanged `.toml` row. That is the host-independent half of the decrease guard (the
/// host-dependent half is the per-spawn [`SpawnTier`] verdicts).
///
/// Linux attaches all three P4.15 legs; Windows the two P4.17 legs; **macOS attaches none** — its tier
/// is DECIDED the cheap-tier floor only (P4.16), so the empty slice is the honest record, not a gap.
///
/// `allow(dead_code)` (non-test): this constant's consumer IS the record binding — the P4.18 test that
/// holds it and `privilege-drop-coverage.toml` identical. Same posture as its ratchet siblings
/// (`coverage-floors.toml` / `max_survived_mutants.toml` have no production reader either): the value is a
/// tracked FACT the gate layer compares, not an input the app branches on.
#[cfg_attr(not(test), allow(dead_code))]
#[cfg(target_os = "linux")]
pub(crate) const ATTACHED_LEGS: &[&str] = &[LEG_NETNS, LEG_LANDLOCK, LEG_SECCOMP];
/// The Windows leg set — see the Linux declaration above for the contract.
#[cfg_attr(not(test), allow(dead_code))]
#[cfg(windows)]
pub(crate) const ATTACHED_LEGS: &[&str] = &[LEG_INTEGRITY, LEG_JOB];
/// The macOS leg set: EMPTY by the P4.16 decision — see the Linux declaration above for the contract.
#[cfg_attr(not(test), allow(dead_code))]
#[cfg(not(any(target_os = "linux", windows)))]
pub(crate) const ATTACHED_LEGS: &[&str] = &[];

/// How EACH leg's verdict is OBSERVED, paired with its [`ATTACHED_LEGS`] id — recorded in
/// `privilege-drop-coverage.toml` beside the leg set so the asymmetry is a CHECKED fact, not prose that can
/// quietly stop being true. It is PER LEG, not per platform, because the strength of a verdict follows the
/// leg's apply point, and Linux mixes the two:
///
/// * `"per-spawn"` — the verdict is about THIS spawn's own child, so it is a real confirmation for it: the
///   Windows integrity leg re-reads the child's token (`GetTokenInformation`), the Windows job leg is
///   whether the assignment to ConvertIA's own job succeeded, and the Linux net-namespace leg compares the
///   child's `/proc/<pid>/ns/net` against the parent's.
/// * `"host-probe"` — the leg applies inside the pre-exec child, which has no channel back to the parent, so
///   the verdict is what the KERNEL answers for this host: it says the mechanism is in force on this machine,
///   it does NOT confirm this individual spawn. Landlock and seccomp read this way, and the per-spawn proof
///   that they took effect is the EFFECT the P4.18.1 regression measures.
///
/// EMPTY on macOS — no leg, so no verdict (P4.16). Same `allow(dead_code)` posture as [`ATTACHED_LEGS`] —
/// the record binding is its consumer. [Build-Session-Entscheidung: P4.18]
#[cfg_attr(not(test), allow(dead_code))]
#[cfg(target_os = "linux")]
pub(crate) const VERDICT_SOURCES: &[(&str, &str)] = &[
    (LEG_NETNS, "per-spawn"),
    (LEG_LANDLOCK, "host-probe"),
    (LEG_SECCOMP, "host-probe"),
];
/// The Windows verdict sources — see the Linux declaration above for the contract.
#[cfg_attr(not(test), allow(dead_code))]
#[cfg(windows)]
pub(crate) const VERDICT_SOURCES: &[(&str, &str)] =
    &[(LEG_INTEGRITY, "per-spawn"), (LEG_JOB, "per-spawn")];
/// The macOS verdict sources: EMPTY — see the Linux declaration above for the contract.
#[cfg_attr(not(test), allow(dead_code))]
#[cfg(not(any(target_os = "linux", windows)))]
pub(crate) const VERDICT_SOURCES: &[(&str, &str)] = &[];

/// The §2.12.3 tier [`ATTACHED_LEGS`] reaches on this platform — [`TIER_PRIVILEGE_DROP`] where the build
/// attaches at least one leg, else the [`TIER_CHEAP`] floor. This is the value the platform's
/// `privilege-drop-coverage.toml` row records. Same `allow(dead_code)` posture as [`ATTACHED_LEGS`].
/// [Build-Session-Entscheidung: P4.18]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const fn attached_tier() -> &'static str {
    if ATTACHED_LEGS.is_empty() {
        TIER_CHEAP
    } else {
        TIER_PRIVILEGE_DROP
    }
}

/// ONE leg's verdict inside a [`SpawnTier`] — the leg's [`ATTACHED_LEGS`] id plus its
/// applied-vs-degraded [`LegOutcome`].
#[cfg_attr(any(not(test), target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LegVerdict {
    /// The leg id — one of [`ATTACHED_LEGS`].
    pub(crate) leg: &'static str,
    /// What that leg achieved for this spawn.
    pub(crate) outcome: LegOutcome,
}

/// The §2.12.3 achieved privilege-drop tier of ONE confined spawn (P4.18) — the per-leg record
/// [`crate::isolation::run_confined`] assembles and hands out on [`crate::engines::ConfinedRun::tier`],
/// and the value the P4.18.1 per-spawn tier-APPLIED regression asserts against the platform's
/// `privilege-drop-coverage.toml` row.
///
/// **Per-leg, never collapsed.** Both Windows legs degrade INDEPENDENTLY — a FAT/exFAT stick or an SMB
/// destination fails Leg A's label-then-lower grant while Leg B's Job Object still attaches — so a single
/// collapsed tier value would report "privilege-drop" for a spawn whose write confinement never applied.
///
/// **A verdict is only ever as strong as its source, and [`VERDICT_SOURCES`] says which is which.** Both
/// Windows legs and the Linux net-namespace leg are `per-spawn` — each is about THIS spawn's own child (a
/// `GetTokenInformation` re-read of its token, whether the assignment to our own job succeeded, whether its
/// `/proc/<pid>/ns/net` differs from ours), so `Applied` there means this spawn really is confined. Landlock
/// and seccomp apply inside the pre-exec child, which has no channel back, so their verdicts are
/// `host-probe` — the kernel's answer for this HOST. Their EFFECT proofs live in different places:
/// Landlock's is the P4.18.1 regression (an APPLIED leg must deny an out-of-sandbox read through the
/// production `run_confined`); seccomp's is P4.15.3's own `seccomp_denies_a_listed_syscall_in_the_child`,
/// which applies a filter through the SAME `build_seccomp_program_for` + `seccompiler::apply_filter`
/// pre-exec mechanism `install_confinement` uses but with a TEST deny-list, because the production
/// deny-list (`ptrace`, `mount`, `bpf`, `kexec_load`, `setns`) is not reachable from a shell — so seccomp is
/// proven at the leg rather than on the spawn path. macOS records no verdict (P4.16 — no leg to report).
/// (`install_confinement` is named without a doc link on purpose: this type is declared on all three
/// platforms while that fn is Linux-only, so a link here would dangle on the Windows and macOS doc builds.)
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SpawnTier {
    verdicts: Vec<LegVerdict>,
}

#[cfg_attr(any(not(test), target_os = "macos"), allow(dead_code))]
impl SpawnTier {
    /// Record one leg's verdict. Called once per attached leg by the spawn that produced it.
    pub(crate) fn record(&mut self, leg: &'static str, outcome: LegOutcome) {
        self.verdicts.push(LegVerdict { leg, outcome });
    }

    /// Every recorded verdict, in the order the legs were recorded.
    pub(crate) fn verdicts(&self) -> &[LegVerdict] {
        &self.verdicts
    }

    /// The verdict recorded for `leg`, or `None` when this platform records none for it (macOS records
    /// none at all; a Linux/Windows leg is always recorded once the spawn reached its apply point).
    pub(crate) fn outcome_of(&self, leg: &str) -> Option<LegOutcome> {
        self.verdicts
            .iter()
            .find(|verdict| verdict.leg == leg)
            .map(|verdict| verdict.outcome)
    }

    /// The tier THIS SPAWN achieved: [`TIER_PRIVILEGE_DROP`] once at least one leg reports
    /// [`LegOutcome::Applied`], else the [`TIER_CHEAP`] floor. Distinct from [`attached_tier`], which is
    /// the compile-time mechanism set: a build that attaches legs still lands on the cheap floor for a
    /// spawn where every one of them degraded (the §2.12.3 silent-degrade semantics, made readable).
    pub(crate) fn tier(&self) -> &'static str {
        if self
            .verdicts
            .iter()
            .any(|verdict| verdict.outcome == LegOutcome::Applied)
        {
            TIER_PRIVILEGE_DROP
        } else {
            TIER_CHEAP
        }
    }
}

/// The standard system dirs the Landlock read set grants (read + traverse + EXECUTE — the
/// `AccessFs::from_read(ABI::V1)` group includes execute) so the confined decoder can launch
/// and resolve its shared libraries + read config (`/etc/ld.so.cache`, fontconfig, locale).
/// A dir that does not exist on the host is skipped (best-effort). §2.12.3 `[DEFER: tuning]`.
#[cfg(target_os = "linux")]
const SANDBOX_READ_DIRS: [&str; 6] = ["/usr", "/lib", "/lib64", "/bin", "/sbin", "/etc"];

/// The seccomp DENY-list (P4.15.3): syscalls never legitimately issued by a short-lived
/// decoder, denied `EPERM` by the otherwise-ALLOW-by-default filter (so libvips/ffmpeg's
/// large, glibc-version-dependent syscall set runs untouched — a default-deny allow-list
/// would randomly break decodes). Deliberately EXCLUDES `execve`/`execveat` — the filter is
/// installed pre-exec, so denying them would block the engine's OWN launch; "no arbitrary
/// exec" is the Landlock execute-right's job — and EXCLUDES `unshare` (the P4.15.2 net-ns
/// leg) + `setpgid` (the P4.10 group-leader). §2.12.3 `[DEFER: tuning]` — the exact set is a
/// tuning residual; every entry is a syscall whose only use in a decoder would be an exploit
/// primitive (debugger attach, mount, kernel-module/BPF load, key management, ns-join).
/// [Build-Session-Entscheidung: P4.15]
#[cfg(target_os = "linux")]
fn seccomp_denied_syscalls() -> Vec<i64> {
    let mut denied = vec![
        libc::SYS_ptrace,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
        libc::SYS_mount,
        libc::SYS_umount2,
        libc::SYS_pivot_root,
        libc::SYS_kexec_load,
        libc::SYS_kexec_file_load,
        libc::SYS_perf_event_open,
        libc::SYS_bpf,
        libc::SYS_add_key,
        libc::SYS_keyctl,
        libc::SYS_request_key,
        libc::SYS_setns,
    ];
    denied.sort_unstable(); // deterministic BPF program (byte-stable across runs)
    denied
}

/// Build the §2.12.3 Landlock ruleset (P4.15.1) in the PARENT — this opens the path fds and
/// allocates the ruleset fd, so ONLY the returned handle's `restrict_self()` (two async-signal-
/// safe syscalls) runs in the pre-exec child. `BestEffort` compatibility means a pre-5.13 kernel
/// yields a `NotEnforced` ruleset rather than an error, so the caller degrades silently. The
/// granted set: `scratch` (full `from_all` rw incl. create — the engine's output lands here), `/dev`
/// (`ReadFile|ReadDir|WriteFile` — read `/dev/urandom`, write `/dev/null`; NOT device-node create,
/// which `from_all` would grant via MakeChar/MakeSock/…), `/proc` (read — the P4.15.2 net-ns leg writes
/// `/proc/self/uid_map` in the SAME pre_exec closure BEFORE this Landlock applies, so read suffices),
/// the engine's own bundle dir `program.parent()` (rx), and [`SANDBOX_READ_DIRS`] (rx). Everything else
/// — the user's home + other files — is denied. A missing path is skipped (best-effort). Returns `None`
/// only if the ruleset could not be created at all (the leg then degrades). NOTE (§2.12.3
/// `[DEFER: tuning]`): the per-item INPUT-file grant is NOT here — see the PROFILE NOTE above (the
/// real-engine consumer P4.37 adds it, since run_confined has no structured input path yet).
/// [Build-Session-Entscheidung: P4.15]
#[cfg(target_os = "linux")]
fn build_landlock_ruleset(program: &Path, scratch: &Path) -> Option<landlock::RulesetCreated> {
    use landlock::{
        Access, AccessFs, CompatLevel, Compatible, PathBeneath, PathFd, Ruleset, RulesetAttr,
        RulesetCreatedAttr, ABI,
    };
    let abi = ABI::V1;
    let read = AccessFs::from_read(abi);
    let all = AccessFs::from_all(abi);
    // /dev needs read (/dev/urandom) + write (/dev/null) of EXISTING device files, never node-create —
    // so read (ReadFile|ReadDir|Execute) plus WriteFile, NOT `from_all` (which adds MakeChar/MakeSock/
    // RemoveFile/… a decoder never needs). Least-privilege over the "grant IS the enforcement" model.
    let dev = read | AccessFs::WriteFile;
    let mut rs = Ruleset::default()
        // BestEffort + silent-degrade: never hard-error on an old/Landlock-off kernel.
        .set_compatibility(CompatLevel::BestEffort)
        // handle_access MUST declare the superset (from_all) or the rw grants are silently dropped.
        .handle_access(all)
        .ok()?
        .create()
        .ok()?;
    // Add a `PathBeneath` rule for `path` (best-effort): a MISSING path (e.g. no /lib64 on a merged-usr
    // host) is SKIPPED — the leg keeps its other grants (`Some(rs)`); a landlock `add_rule` failure with a
    // valid fd is rare and degrades the WHOLE leg via `?` at the call site (`add_rule` consumes the ruleset
    // by value, so a failed add cannot return the original — `.ok()` maps it to `None`).
    let add =
        |rs: landlock::RulesetCreated, path: &Path, access| -> Option<landlock::RulesetCreated> {
            match PathFd::new(path) {
                Ok(fd) => rs.add_rule(PathBeneath::new(fd, access)).ok(),
                Err(_) => Some(rs),
            }
        };
    rs = add(rs, scratch, all)?;
    rs = add(rs, Path::new("/dev"), dev)?;
    rs = add(rs, Path::new("/proc"), read)?;
    if let Some(parent) = program.parent() {
        rs = add(rs, parent, read)?;
    }
    for dir in SANDBOX_READ_DIRS {
        rs = add(rs, Path::new(dir), read)?;
    }
    Some(rs)
}

/// Build the §2.12.3 seccomp-bpf program (P4.15.3) for `denied` in the PARENT (this allocates
/// the `BTreeMap` + the `Vec<sock_filter>`, so only the tiny `apply_filter` install runs in the
/// child). ALLOW-by-default (`mismatch_action`), `EPERM` for a listed syscall (`match_action` —
/// graceful, so a denied call surfaces as an engine error, never a spurious KILL; §2.12.3
/// `[DEFER: tuning]` could switch to `KillProcess`). Returns `None` on an unsupported arch
/// (`TargetArch` conversion fails) or a compile error — then the seccomp leg degrades silently.
/// An empty `denied` yields `None` (a no-op filter is not installed). [Build-Session-Entscheidung: P4.15]
#[cfg(target_os = "linux")]
fn build_seccomp_program_for(denied: &[i64]) -> Option<seccompiler::BpfProgram> {
    use seccompiler::{BpfProgram, SeccompAction, SeccompFilter, SeccompRule, TargetArch};
    use std::collections::BTreeMap;
    if denied.is_empty() {
        return None;
    }
    // The filter must be compiled for the RUNNING arch (a wrong-arch filter SIGSYSes at runtime).
    let arch: TargetArch = std::env::consts::ARCH.try_into().ok()?;
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    for &syscall in denied {
        // An empty rule Vec matches the syscall UNCONDITIONALLY → the match_action (EPERM) fires.
        rules.insert(syscall, vec![]);
    }
    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow, // mismatch: allow every other syscall
        SeccompAction::Errno(libc::EPERM as u32), // match: deny the listed ones with EPERM
        arch,
    )
    .ok()?;
    let program: BpfProgram = filter.try_into().ok()?;
    Some(program)
}

/// The production seccomp program — [`build_seccomp_program_for`] over [`seccomp_denied_syscalls`].
#[cfg(target_os = "linux")]
fn build_seccomp_program() -> Option<seccompiler::BpfProgram> {
    build_seccomp_program_for(&seccomp_denied_syscalls())
}

/// Attach the §2.12.3 privilege-drop tier (P4.15) to `command` as ONE best-effort `pre_exec` closure:
/// the network-namespace egress-deny leg (P4.15.2), the Landlock fs-restrict leg (P4.15.1) and the
/// seccomp-bpf exec-deny leg (P4.15.3), applied IN THAT ORDER in the post-fork / pre-exec child.
/// `crate::isolation::run_confined` calls this on the spawn's underlying `std::process::Command`
/// (`command.as_std_mut()`), so `crate::isolation` stays unsafe-free. The Landlock ruleset fd, the seccomp
/// BPF program and the identity uid/gid map bytes are built HERE in the PARENT and moved into the closure,
/// which issues ONLY async-signal-safe syscalls. `program` is the engine binary (the Landlock bundle-dir
/// grant keys on it); `scratch` is the per-run cwd (rw). BEST-EFFORT / NEVER-break: net-ns applies via
/// `unshare` IN the single-threaded child, so a namespace-setup failure just SKIPS net-ns and the engine
/// still runs; a `NotEnforced` Landlock ruleset or a failed `apply_filter` is swallowed to `Ok(())` — the
/// tier is non-load-bearing and must NEVER fail the spawn. ORDER: net-ns FIRST (its `/proc/self/uid_map`
/// write must precede Landlock, which grants `/proc` read-only), seccomp LAST (so it never gates the setup
/// syscalls). The closure is ALWAYS installed (net-ns is always attempted). [Build-Session-Entscheidung: P4.15]
#[cfg(target_os = "linux")]
pub(crate) fn install_confinement(
    command: &mut std::process::Command,
    program: &Path,
    scratch: &Path,
) {
    use std::os::unix::process::CommandExt;
    // Build everything the closure needs in the PARENT (fd open, allocation, formatting) — the child only
    // APPLIES it via syscalls. The identity uid/gid maps keep the engine's OWN uid inside the user namespace,
    // so it can still read/write the per-run scratch (the correct realization of the §2.12.3 net-ns leg — a
    // default `unshare --user` maps to nobody and would break scratch access).
    let mut ruleset = build_landlock_ruleset(program, scratch);
    let seccomp = build_seccomp_program();
    // SAFETY: getuid/getgid are argument-less, pointer-free syscalls that cannot fail. Parent-side.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
    let uid_map = format!("{uid} {uid} 1\n").into_bytes();
    let gid_map = format!("{gid} {gid} 1\n").into_bytes();

    // The `pre_exec` closure runs post-fork / pre-exec in the child. Every operation is async-signal-safe:
    //   - net-ns: raw `unshare` + `open`/`write`/`close` of static `/proc` control paths (no alloc/lock);
    //   - Landlock `restrict_self()`: prctl(NO_NEW_PRIVS) + landlock_restrict_self — the `landlock` crate
    //     resolves the BestEffort compat level in the PARENT at build time, so the child call neither
    //     allocates nor logs (verified against landlock 0.4.5's ruleset.rs — the fork-safety guarantee);
    //   - seccompiler `apply_filter()`: prctl + seccomp(SET_MODE_FILTER) on the parent-built BPF — stack-only.
    // No heap allocation, no lock, no panic (every arm swallows to Ok); the ruleset fd, BPF program and map
    // bytes are built in the PARENT and moved in by value, so no lock a sibling thread held across the fork
    // can deadlock the child.
    // SAFETY: the `pre_exec` contract (async-signal-safe only) is met per the rationale above.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    unsafe {
        command.pre_exec(move || {
            // 1. net-ns (P4.15.2) FIRST — the child is single-threaded post-fork, so CLONE_NEWUSER is valid.
            //    A FAILURE just skips (the engine still runs — never-break); the identity maps keep our uid.
            //    Async-signal-safe raw syscalls (covered by the outer `unsafe`): unshare + open/write/close of
            //    static NUL-terminated /proc paths + parent-built map bytes; every fd is closed; failure ignored.
            if libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWNET) == 0 {
                // setgroups=deny MUST precede gid_map for an unprivileged user namespace.
                for (path, content) in [
                    (&b"/proc/self/setgroups\0"[..], &b"deny"[..]),
                    (&b"/proc/self/uid_map\0"[..], uid_map.as_slice()),
                    (&b"/proc/self/gid_map\0"[..], gid_map.as_slice()),
                ] {
                    let fd = libc::open(path.as_ptr().cast::<libc::c_char>(), libc::O_WRONLY);
                    if fd >= 0 {
                        libc::write(fd, content.as_ptr().cast::<libc::c_void>(), content.len());
                        libc::close(fd);
                    }
                }
            }
            // 2. Landlock (P4.15.1) — restrict_self consumes the ruleset by value → Option::take (FnMut, once).
            if let Some(rs) = ruleset.take() {
                let _ = rs.restrict_self();
            }
            // 3. seccomp (P4.15.3) LAST — so it never gates the net-ns / Landlock setup syscalls above.
            if let Some(prog) = &seccomp {
                let _ = seccompiler::apply_filter(prog);
            }
            Ok(())
        });
    }
}

/// §2.12.3 Landlock availability probe (P4.15.1) — build a trivial `BestEffort` ruleset on a
/// THROWAWAY thread (so no parent thread is left restricted) and read the `RestrictionStatus`:
/// `NotEnforced` ⇒ the kernel lacks Landlock (< 5.13 / disabled) ⇒ `Degraded(Unavailable)`; a real
/// error ⇒ `Degraded(NotApplied)`; otherwise `Applied`. Used by the per-leg tests and reported into
/// the P4.18 record via [`spawn_leg_verdicts`]; the production apply in [`install_confinement`] needs no
/// probe — `BestEffort` self-degrades. [Build-Session-Entscheidung: P4.15]
///
/// `allow(dead_code)` (non-test): the P4.18 production consumer is [`spawn_leg_verdicts`], itself dead until
/// the confined-spawn lane becomes a live root at P4.32 — see [`LegOutcome`]. `allow` not `expect`: the body
/// constructs the enum in the non-test build, so `expect` would flip unfulfilled.
#[cfg(target_os = "linux")]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn landlock_probe() -> LegOutcome {
    use landlock::{
        Access, AccessFs, CompatLevel, Compatible, Ruleset, RulesetAttr, RulesetStatus, ABI,
    };
    std::thread::spawn(|| {
        let status = Ruleset::default()
            .set_compatibility(CompatLevel::BestEffort)
            .handle_access(AccessFs::from_all(ABI::V1))
            .and_then(|r| r.create())
            .and_then(|r| r.restrict_self());
        match status {
            Ok(s) if s.ruleset == RulesetStatus::NotEnforced => {
                LegOutcome::Degraded(DegradeReason::Unavailable)
            }
            Ok(_) => LegOutcome::Applied,
            Err(_) => LegOutcome::Degraded(DegradeReason::NotApplied),
        }
    })
    .join()
    .unwrap_or(LegOutcome::Degraded(DegradeReason::NotApplied))
}

/// §2.12.3 seccomp availability probe (P4.15.3) — compile the production DENY-list and install it on
/// a THROWAWAY thread (a seccomp filter is thread-local + never removable, so it must not touch a live
/// thread): a clean install ⇒ `Applied`; a compile miss (unsupported arch) or a kernel that rejects
/// `seccomp(SET_MODE_FILTER)` ⇒ `Degraded(Unavailable)`. [Build-Session-Entscheidung: P4.15]
///
/// `allow(dead_code)` (non-test): the P4.18 production consumer is [`spawn_leg_verdicts`], itself dead
/// until the confined-spawn lane becomes a live root at P4.32 — see [`LegOutcome`]. `allow` not `expect`
/// (the reporter body constructs the enum in the non-test build).
#[cfg(target_os = "linux")]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn seccomp_probe() -> LegOutcome {
    let Some(program) = build_seccomp_program() else {
        return LegOutcome::Degraded(DegradeReason::Unavailable);
    };
    std::thread::spawn(move || match seccompiler::apply_filter(&program) {
        Ok(()) => LegOutcome::Applied,
        Err(_) => LegOutcome::Degraded(DegradeReason::Unavailable),
    })
    .join()
    .unwrap_or(LegOutcome::Degraded(DegradeReason::Unavailable))
}

/// §2.12.3 net-namespace PER-SPAWN verdict (P4.15.2, recorded into the P4.18 tier record): does the child
/// `pid` really sit in a network namespace of its own?
///
/// This leg has **no throwaway-thread apply** the way its two siblings do — `unshare(CLONE_NEWUSER)` is
/// rejected outright in a MULTITHREADED process (EINVAL), which is exactly why [`install_confinement`]
/// applies it in the single-threaded post-fork child and nowhere else, and a parent that tried to "probe by
/// applying" would either fail for the wrong reason or move the HOST process into a new namespace. But the
/// result IS parent-observable: namespace membership is exposed as a symlink under `/proc`, so comparing the
/// child's `/proc/<pid>/ns/net` against our own answers the real question — DID this child get its own
/// namespace — instead of the weaker "does the kernel permit one".
///
/// That distinction is load-bearing. A capability reading over the `user.max_user_namespaces` /
/// `kernel.unprivileged_userns_clone` / `kernel.apparmor_restrict_unprivileged_userns` knobs cannot see a
/// container or LSM policy that denies `unshare` anyway, and cannot see the in-closure `unshare` failing at
/// runtime (which silently skips, by design) — so it would report `Applied` for a spawn that never got a
/// namespace. Over-reporting is the one direction the G64 record must never take, since the whole point of
/// the ratchet is to make a LOST restriction visible.
///
/// `Degraded(NotApplied)` = the child demonstrably shares our namespace (the leg was skipped or refused);
/// `Degraded(Unavailable)` = the membership could not be read at all — no `/proc`, no pid (the spawn
/// wrapper had none to give), or the child was already reaped — an honest "not observed", never a silent
/// `Applied`. [Build-Session-Entscheidung: P4.18]
#[cfg(target_os = "linux")]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn netns_verdict(pid: Option<u32>) -> LegOutcome {
    let (Some(pid), Ok(ours)) = (pid, std::fs::read_link("/proc/self/ns/net")) else {
        return LegOutcome::Degraded(DegradeReason::Unavailable);
    };
    let Ok(theirs) = std::fs::read_link(format!("/proc/{pid}/ns/net")) else {
        return LegOutcome::Degraded(DegradeReason::Unavailable);
    };
    if theirs == ours {
        return LegOutcome::Degraded(DegradeReason::NotApplied);
    }
    LegOutcome::Applied
}

/// The §2.12.3 Linux per-leg verdicts for the P4.18 achieved-tier record of ONE spawn, in [`ATTACHED_LEGS`]
/// order so the record and the apply order read the same. Every leg reports INDEPENDENTLY — the P4.15
/// per-leg-independence contract: an old kernel without Landlock says nothing about seccomp.
///
/// Net-ns is read PER SPAWN off the child itself ([`netns_verdict`]). Landlock and seccomp cannot be:
/// they apply inside the pre-exec child with no channel back, so their verdicts are the host-capability
/// readings [`landlock_probe`] / [`seccomp_probe`] produce — taken ONCE per process and cached, because
/// each costs a thread (Landlock restricts it, seccomp installs a filter on it) and the answer is a host
/// property that cannot change under a running app. The cache also keeps the cost off the async spawn path:
/// only the FIRST Linux spawn joins the two probe threads; every later one reads the memoised value.
/// [Build-Session-Entscheidung: P4.18]
#[cfg(target_os = "linux")]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn spawn_leg_verdicts(pid: Option<u32>) -> SpawnTier {
    static CACHED_HOST_LEGS: std::sync::OnceLock<(LegOutcome, LegOutcome)> =
        std::sync::OnceLock::new();
    let (landlock, seccomp) = *CACHED_HOST_LEGS.get_or_init(|| (landlock_probe(), seccomp_probe()));
    let mut tier = SpawnTier::default();
    tier.record(LEG_NETNS, netns_verdict(pid));
    tier.record(LEG_LANDLOCK, landlock);
    tier.record(LEG_SECCOMP, seccomp);
    tier
}

// ============================================================================
// §2.12.3 best-effort WINDOWS privilege-drop tier (P4.17) — the two mechanisms that ARE
// realizable in the v1-portable build (spec §2.12.3 `[DECIDED — P4.17, Co-Pilot ruling
// 2026-08-25]`): **Leg A**, an intermediate-integrity WRITE confinement (label-then-lower), and
// **Leg B**, an own Job Object carrying the `JOB_OBJECT_LIMIT` caps + kill-on-job-close.
// Restricted-token / AppContainer and the AppContainer/WFP net-deny are DECIDED unrealizable on
// this stack, so NO FFI for them enters the core — pinned by the cross-platform
// `no_appcontainer_or_spawn_token_ffi_in_the_core` source-scan at the end of this file (the
// `no_seatbelt_apply_callsite_in_the_core` sibling).
//
// Both legs apply PARENT-SIDE on the still-`CREATE_SUSPENDED` child, BEFORE its threads resume
// (`crate::isolation`'s own `process_wrap::CommandWrapper::post_spawn` hook, which the crate runs
// before every `wrap_child` — and `JobObject::wrap_child` is what resumes the threads). That
// satisfies the P4.16 admission test literally: parent-side, never a post-fork child, and every
// failure is an error the caller silently skips, never a hang.
//
// EMERGENT SIDE-EFFECT, recorded honestly — NOT a designed control: a child at the intermediate
// level is also refused Medium-labelled DEVICE objects, so it cannot open a SOCKET or a mailslot.
// Measured against a real confined child on this stack: a System32 `ping` fails, while a System32
// `waitfor` sleep, an `exec` of another System32 binary, a `>nul` redirect and every write into a
// granted sink all succeed. §2.12.3 is explicit that Windows has NO privilege-drop network-deny
// leg, and this must not be read as one — it disappears the moment the tier degrades, and the
// load-bearing offline gate is the §2.11.4 packet-monitor regardless of tier. What it IS, is
// exactly the class the ruling's PER-ENGINE tier opt-out exists for: the first candidate is
// LibreOffice headless, whose UNO IPC rides a NAMED PIPE, so the P7 office corpus decides that
// engine's tier rather than widening the grant.
//
// The §2.12.3 residual covers BOTH labelled sinks, not only the `.part`: the per-run scratch is
// labelled `(OI)(CI)` at the same level, so the same-user cross-integrity co-tenant the spec note
// records could equally write there — engine working files, and (were it not created before the
// label, §2.6.3 lock-before-part) the run lock. Same subject, same bound, same compensating
// controls; recorded here because the sink set is this module's, and flagged to the Co-Pilot for
// the §2.12.3 note, whose text is theirs.
//
// PER-ENGINE TIER OPT-OUT — decided up front (the ruling's Leg-A(iv)): Leg A is a GRANT, never a
// tuning dial. An engine that DELETES and RE-CREATES its output file mid-run (the FFmpeg
// truncate-vs-recreate shape) succeeds the unlink from the confined child but is DENIED the
// re-create in the Medium destination dir — and the unlink also RELEASES the §2.1.2 `create_new`'d
// reserved name mid-run. Closing that would mean labelling the USER'S WHOLE DESTINATION DIRECTORY,
// which is unacceptable, so the answer for such an engine is to SKIP Leg A for it — a per-engine
// tier decision the P5-P7 G31/G32 corpus verdict makes — never to widen the grant. No per-engine
// discriminant is planted here: the subprocess engines land P5-P7, so a possibly-unused one would
// be a premature type (CLAUDE §5); the plan carries the obligation (the P4.37 / P7 forward notes).
//
// BEST-EFFORT / NEVER-break: every fn here returns a verdict rather than an error, and every
// failure path leaves the P4.13 cheap-tier floor in place — the tier is defence-in-depth, NOT
// load-bearing (§0.11 T9b: the §3.5/§6.1.3 argv/build controls carry the guarantee). The exact
// label placement + cap values are the §2.12.3 `[DEFER: tuning]` residual; this box builds the
// tier MECHANISM. [Build-Session-Entscheidung: P4.17]
// ============================================================================

/// The ConvertIA-private mandatory integrity level the Leg-A confinement uses — `S-1-16-6144`
/// (`0x1800`), STRICTLY between Low (`0x1000`) and Medium (`0x2000`) (spec §2.12.3
/// `[DECIDED — P4.17]`). The intermediate level, NOT the well-known Low, is deliberate: a Low
/// (4096) co-tenant — an Acrobat renderer, Office Protected View, a browser content process,
/// i.e. exactly the sandboxes a hostile document compromises — is denied write-UP to a 6144
/// object by the MIC total order (`NO_WRITE_UP`), while the engine at 6144 still cannot write
/// Medium (8192) user files. Windows keeps the exact RID (it does not snap an intermediate value
/// to a well-known level), as its own Medium Plus (`0x2100`) shows.
#[cfg(windows)]
pub(crate) const CONFINED_INTEGRITY_RID: u32 = 0x1800;

/// The mandatory-integrity SID for `rid` — `S-1-16-<rid>`, the mandatory-label authority.
#[cfg(windows)]
fn integrity_sid(rid: u32) -> String {
    format!("S-1-16-{rid}")
}

/// The Leg-A label ACE for ONE FILE at `rid` (no inheritance) — the `.part` publish temp. Built from
/// [`integrity_sid`] rather than written out, so the level lives in exactly one place.
#[cfg(windows)]
fn label_sddl_file(rid: u32) -> String {
    format!("S:(ML;;NW;;;{})", integrity_sid(rid))
}

/// The Leg-A label ACE for a DIRECTORY at `rid`, `(OI)(CI)`-inheritable so everything the engine
/// creates inside the per-run scratch — a LibreOffice `--outdir` / profile / TMP subtree — inherits
/// the same level.
#[cfg(windows)]
fn label_sddl_dir(rid: u32) -> String {
    format!("S:(ML;OICI;NW;;;{})", integrity_sid(rid))
}

/// The STRIP: an explicit EMPTY SACL, which reads back `NO_ACCESS_CONTROL` — never "set Medium",
/// which would stamp a label the destination does not otherwise carry. Only
/// `LABEL_SECURITY_INFORMATION` is ever passed, so the DACL / owner / group stay untouched (§2.14.1:
/// the mandatory label is the orthogonal INTEGRITY dimension, not the confidentiality/DACL one).
#[cfg(windows)]
const LABEL_SDDL_NONE: &str = "S:";

/// The SDDL fragment that marks a mandatory-label ACE — the needle both the strip and the
/// read-blocking test key on.
#[cfg(windows)]
const LABEL_ACE_MARKER: &str = "(ML;";

/// The Leg-B `JOB_OBJECT_LIMIT` runaway caps (§2.12.3 `[DEFER: tuning]` — the per-OS profile
/// CONTENTS are the tier's one residual; the MECHANISM is what this box builds). Both are
/// deliberately GENEROUS so a cap can only ever catch a runaway, never a legitimate conversion
/// (never-break is absolute): 16 GiB of committed job memory is multiples of what the bundled
/// engines commit — they stream (FFmpeg) or tile (libvips) — and 64 active processes is far
/// above the deepest bundled tree (`soffice` → `soffice.bin`). These are NOT the §1.10 per-item
/// resource budgets (a different control with its own preflight); they are the OS-level backstop
/// underneath them.
#[cfg(windows)]
const JOB_MEMORY_CAP_BYTES: u64 = 16 << 30;
#[cfg(windows)]
const JOB_ACTIVE_PROCESS_CAP: u32 = 64;

/// The `JOB_OBJECT_LIMIT` values one [`ConfinedJob`] carries. Kept as a value rather than read
/// back from the job so [`ConfinedJob::stand_down`] can re-apply the SAME caps while dropping
/// only the kill-on-job-close flag, and so a test can attach a deliberately tiny cap to prove the
/// limit is ARMED rather than merely set. [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct JobLimits {
    /// `JOB_OBJECT_LIMIT_JOB_MEMORY` — the job-wide committed-memory cap.
    memory_bytes: u64,
    /// `JOB_OBJECT_LIMIT_ACTIVE_PROCESS` — the fork-bomb guard.
    active_processes: u32,
}

/// The production Leg-B caps ([`JOB_MEMORY_CAP_BYTES`] / [`JOB_ACTIVE_PROCESS_CAP`]).
#[cfg(windows)]
const PRODUCTION_JOB_LIMITS: JobLimits = JobLimits {
    memory_bytes: JOB_MEMORY_CAP_BYTES,
    active_processes: JOB_ACTIVE_PROCESS_CAP,
};

/// ConvertIA's OWN Job Object around one confined engine spawn (§2.12.3 Leg B, P4.17) — the
/// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` + memory / active-process / die-on-unhandled-exception
/// job the §1.7 P4.10 forward note assigns to this box. It is assigned in `post_spawn`, i.e.
/// BEFORE `process_wrap`'s own (limit-less) job is assigned in `wrap_child`, so ours is the OUTER
/// job of the Windows-8+ nested pair and its limits cover the whole tree; `process_wrap`'s
/// `TerminateJobObject` group-kill and its completion-port wait keep working unchanged (the
/// P4.10/P4.11 contract survives 1:1).
///
/// The handle is a `std::os::windows::io::OwnedHandle`, which closes on drop — and with
/// kill-on-job-close armed, that close IS the crash-time reap `process_wrap` 9.1.0 cannot deliver
/// (its `core`-is-empty defect leaves the flag off). `OwnedHandle` is `Send` + `Sync` by
/// construction, so the tier needs no `unsafe impl Send` — which the G9 repo-invariant (c)
/// forbids outside an `ffi` module anyway. [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
#[derive(Debug)]
pub(crate) struct ConfinedJob {
    job: std::os::windows::io::OwnedHandle,
    limits: JobLimits,
}

#[cfg(windows)]
impl ConfinedJob {
    /// Drop kill-on-job-close while KEEPING the resource caps, then let the handle close
    /// harmlessly — the CLEAN-exit arm only. `crate::isolation`'s `GroupKillGuard` stands its
    /// explicit group-kill down on a clean completed wait (killing there would truncate a launcher
    /// that legitimately exits before its worker finished writing valid output, §1.7); this is the
    /// Leg-B mirror of that decision, so the two teardown authorities agree. On the CRASH /
    /// reap-fault / cancel arms the flag is deliberately LEFT ARMED, so a ConvertIA host crash
    /// still reaps the engine tree (the P4.10 residual this box closes).
    ///
    /// If clearing the flag FAILS, the handle is deliberately LEAKED (`mem::forget`) instead of
    /// closed: closing it while armed would kill exactly the launcher-outlives-worker tree this
    /// stand-down exists to protect. One kernel handle then lives until the process exits — at
    /// which point a job teardown is the correct behaviour anyway — which is strictly better than
    /// a correctness regression on the success path. [Build-Session-Entscheidung: P4.17]
    pub(crate) fn stand_down(self) {
        use std::os::windows::io::AsRawHandle;
        if set_job_limits(self.job.as_raw_handle(), self.limits, false) {
            return;
        }
        std::mem::forget(self);
    }
}

/// Widen an `OsStr` into the NUL-terminated UTF-16 buffer a `PCWSTR` Win32 argument needs.
/// Returns `None` when the value already contains an interior NUL, which would silently truncate
/// the argument — the caller then degrades to the cheap tier rather than acting on a truncated
/// path. [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
fn wide_nul(text: &std::ffi::OsStr) -> Option<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;
    let mut buf: Vec<u16> = text.encode_wide().collect();
    if buf.contains(&0) {
        return None;
    }
    buf.push(0);
    Some(buf)
}

/// True when the volume holding `path` persists ACLs (`FILE_PERSISTENT_ACLS`) — i.e. it can
/// actually store the Leg-A mandatory label. Evaluated PER SINK (§2.14.1 puts the `.part` in the
/// destination dir while the §2.14.2 scratch lives under the user profile, and the §2.14.3
/// fallback can place them on different volumes), because a FAT/exFAT stick or any other
/// non-persistent volume silently DROPS the label — and lowering the engine's token against a sink
/// whose label was dropped would break the conversion. `GetVolumePathNameW` resolves the mount
/// point first, so a nested mount / UNC path is classified against the volume that really holds it.
/// Any failure reads as "cannot confirm" ⇒ `false` ⇒ cheap tier.
///
/// A **REMOTE** volume is excluded OUTRIGHT, ahead of the flag test, rather than trusted to report
/// honestly: an SMB share backed by NTFS DOES report `FILE_PERSISTENT_ACLS` and DOES accept the
/// label write over the redirector — yet MIC is a LOCAL-kernel mechanism, so whether a confined
/// subject can then open that redirector-served file for write is not something this build has
/// probed. §2.12.3's never-break floor is absolute, and this tier is non-load-bearing, so an
/// unprobed edge degrades rather than ships on an assumption. That also covers mapped drives and
/// UNC paths, which `GetVolumePathNameW` alone would happily resolve.
/// [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
fn volume_persists_acls(path: &Path) -> bool {
    use windows_sys::Win32::Storage::FileSystem::{
        GetDriveTypeW, GetVolumeInformationW, GetVolumePathNameW,
    };
    // Win32 `FILE_PERSISTENT_ACLS` / `DRIVE_REMOTE` (winnt.h / winbase.h), declared locally so the
    // `windows-sys` feature set stays at the four this tier needs (the `fs_guard`
    // `FILE_FLAG_BACKUP_SEMANTICS` precedent).
    const FILE_PERSISTENT_ACLS: u32 = 0x0000_0008;
    const DRIVE_REMOTE: u32 = 4;
    // The extended-path maximum, not `MAX_PATH`: a 260-element buffer would make
    // `GetVolumePathNameW` fail on a long destination and degrade the tier for a reason that has
    // nothing to do with the volume.
    const MOUNT_BUF: usize = 32_768;
    let Some(wide_path) = wide_nul(path.as_os_str()) else {
        return false;
    };
    let mut mount = vec![0u16; MOUNT_BUF];
    // SAFETY: `wide_path` is a NUL-terminated UTF-16 buffer alive across the call, and `mount` is
    // a caller-owned buffer whose true element count is passed as the length.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let resolved = unsafe {
        GetVolumePathNameW(
            wide_path.as_ptr(),
            mount.as_mut_ptr(),
            u32::try_from(mount.len()).unwrap_or(0),
        )
    };
    if resolved == 0 {
        return false;
    }
    // SAFETY: `mount` is the NUL-terminated mount point `GetVolumePathNameW` just wrote.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let drive_type = unsafe { GetDriveTypeW(mount.as_ptr()) };
    if drive_type == DRIVE_REMOTE {
        return false;
    }
    let mut flags: u32 = 0;
    // SAFETY: `mount` is the NUL-terminated mount point `GetVolumePathNameW` just wrote; every unwanted
    // out-param is null with a zero length, and `flags` is a valid `u32` out-param.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let read = unsafe {
        GetVolumeInformationW(
            mount.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut flags,
            std::ptr::null_mut(),
            0,
        )
    };
    read != 0 && flags & FILE_PERSISTENT_ACLS != 0
}

/// Apply an SDDL security descriptor's SACL to `path` as its mandatory integrity LABEL, or clear
/// it ([`LABEL_SDDL_NONE`]). Only `LABEL_SECURITY_INFORMATION` is passed and the owner / group /
/// DACL arguments are null, so nothing but the label changes. Needs no `SeSecurityPrivilege`
/// (that privilege gates the AUDIT half of the SACL, not the label) and no elevation — only
/// `WRITE_OWNER` on an object we created. Returns `true` on `ERROR_SUCCESS`.
/// [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
fn set_label_sddl(path: &Path, sddl: &str) -> bool {
    use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS};
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW,
        SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        GetSecurityDescriptorSacl, ACL, LABEL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    };
    let (Some(wide_path), Some(wide_sddl)) = (
        wide_nul(path.as_os_str()),
        wide_nul(std::ffi::OsStr::new(sddl)),
    ) else {
        return false;
    };
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: `wide_sddl` is a NUL-terminated UTF-16 buffer alive across the call; `descriptor` is
    // a valid out-param receiving a `LocalAlloc` block freed exactly once below.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide_sddl.as_ptr(),
            SDDL_REVISION_1,
            &raw mut descriptor,
            std::ptr::null_mut(),
        )
    };
    if converted == 0 {
        return false;
    }
    let mut present: i32 = 0;
    let mut sacl: *mut ACL = std::ptr::null_mut();
    let mut defaulted: i32 = 0;
    // SAFETY: `descriptor` is the descriptor the converter just produced and is still owned here;
    // the three out-params are valid locals.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let read = unsafe {
        GetSecurityDescriptorSacl(
            descriptor,
            &raw mut present,
            &raw mut sacl,
            &raw mut defaulted,
        )
    };
    let rc = if read == 0 {
        // A sentinel distinct from `ERROR_SUCCESS`: the verdict below is `== ERROR_SUCCESS`, so any
        // other value reads as "the label was not applied".
        u32::MAX
    } else {
        // SAFETY: `wide_path` is NUL-terminated and alive, `sacl` points INTO `descriptor` (freed only
        // after this returns), and owner / group / DACL are null — so only the label is written.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        unsafe {
            SetNamedSecurityInfoW(
                wide_path.as_ptr(),
                SE_FILE_OBJECT,
                LABEL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
                sacl,
            )
        }
    };
    // SAFETY: `descriptor` is the `LocalAlloc` block the converter returned, freed exactly once.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    unsafe { LocalFree(descriptor) };
    rc == ERROR_SUCCESS
}

/// Read `path`'s mandatory-label SACL back as SDDL (`S:AI(ML;;NW;;;S-1-16-6144)`,
/// `S:AINO_ACCESS_CONTROL`, …), or `None` when it cannot be read. Reading with
/// `LABEL_SECURITY_INFORMATION` alone needs no `SeSecurityPrivilege`, unlike a full SACL read, so
/// this works from an ordinary non-elevated ConvertIA process. It is the grant-IS-the-enforcement
/// read-back the Leg-A strip and the tier tests assert against, never an assumption that the
/// write took. [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
fn read_label_sddl(path: &Path) -> Option<String> {
    use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW,
        SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{ACL, LABEL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR};
    let wide_path = wide_nul(path.as_os_str())?;
    let mut sacl: *mut ACL = std::ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: `wide_path` is NUL-terminated and alive; the unrequested out-params are null, and `sacl` /
    // `descriptor` are valid out-params — `descriptor` gets a `LocalAlloc` block freed once below.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let rc = unsafe {
        GetNamedSecurityInfoW(
            wide_path.as_ptr(),
            SE_FILE_OBJECT,
            LABEL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut sacl,
            &raw mut descriptor,
        )
    };
    if rc != ERROR_SUCCESS {
        return None;
    }
    let mut text: windows_sys::core::PWSTR = std::ptr::null_mut();
    let mut len: u32 = 0;
    // SAFETY: `descriptor` is the descriptor just returned and still owned here; `text` receives a
    // `LocalAlloc` UTF-16 string freed exactly once below.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let converted = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            LABEL_SECURITY_INFORMATION,
            &raw mut text,
            &raw mut len,
        )
    };
    let sddl = if converted == 0 || text.is_null() {
        None
    } else {
        // SAFETY: `text` is the NUL-terminated UTF-16 string the converter allocated and `len` is its
        // character count, so the slice stays inside the allocation.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        let chars = unsafe { std::slice::from_raw_parts(text, len as usize) };
        // `len` COUNTS the terminator, so the lossy conversion would carry a trailing NUL into the
        // String — measured, not assumed. Trim it, so every consumer sees the bare SDDL text.
        Some(
            String::from_utf16_lossy(chars)
                .trim_end_matches('\0')
                .to_owned(),
        )
    };
    // SAFETY: both pointers are `LocalAlloc` blocks owned here, each freed exactly once; a null
    // `text` (the not-converted arm) is a documented no-op for `LocalFree`.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    unsafe {
        LocalFree(text.cast());
        LocalFree(descriptor);
    }
    sddl
}

/// The mandatory-integrity RID an SDDL label ACE names, or `None` when the SDDL carries no label ACE
/// or a level this cannot resolve. Lets the read-blocking test below reason about the LEVEL rather
/// than the mere presence of an `NR`/`NX` flag — a `NO_READ_UP` ACE BELOW our own level cannot block
/// us (that is a read DOWN).
///
/// Both SDDL spellings are handled, because Windows CHOOSES between them on read-back: a WELL-KNOWN
/// level comes back as its two-letter alias (`S:AI(ML;;NWNR;;;LW)`), while an intermediate level with
/// no alias — ConvertIA's own [`CONFINED_INTEGRITY_RID`] — comes back as the verbatim SID
/// (`S:AI(ML;;NW;;;S-1-16-6144)`). Reading only the numeric form would silently resolve every
/// well-known level to `None`, which the caller's never-break bias then treats as blocking — i.e. the
/// tier would degrade on a label that cannot restrict it. The ACE's account-SID is its 6th
/// `;`-separated field. [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
fn label_ace_rid(sddl: &str) -> Option<u32> {
    let (_rights, sid) = label_ace_fields(sddl)?;
    if let Some(rid) = sid.strip_prefix("S-1-16-") {
        return rid.parse().ok();
    }
    // The `SDDL_ML_*` aliases (sddl.h). An unknown one stays `None` — the caller's never-break bias.
    match sid {
        "LW" => Some(0x1000),
        "ME" => Some(0x2000),
        "MP" => Some(0x2100),
        "HI" => Some(0x3000),
        "SI" => Some(0x4000),
        _ => None,
    }
}

/// The `(rights, account_sid)` of the FIRST mandatory-label ACE in `sddl`. An ACE is
/// `(type;flags;rights;object_guid;inherit_object_guid;account_sid)`; splitting at the `(ML;` marker
/// consumes the type, so the remaining fields are `flags` (0), `rights` (1), the two GUIDs (2, 3) and
/// the SID (4). Returning BOTH is what keeps the level test and the rights test keyed to the SAME ace —
/// each resolves the FIRST `(ML;` ACE, so they can never describe two different subjects. (They are two
/// calls, not one parse: `label_ace_rid` re-enters here for the SID. Reading `NR`/`NX` off the whole
/// descriptor while reading the RID off one ACE is what would mix subjects, and that is what this
/// replaces.) [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
fn label_ace_fields(sddl: &str) -> Option<(&str, &str)> {
    let mut fields = sddl.split_once(LABEL_ACE_MARKER)?.1.split(';');
    let _flags = fields.next()?;
    let rights = fields.next()?;
    let _object_guid = fields.next()?;
    let _inherit_object_guid = fields.next()?;
    let sid = fields.next()?.trim_end_matches([')', '\0']);
    Some((rights, sid))
}

/// True when `path` carries a label that would BLOCK a child confined to `rid` from reading or
/// executing it — an ACE **at or above** `rid` carrying `NO_READ_UP` (`NR`) or `NO_EXECUTE_UP`
/// (`NX`). Lowering the token against such an engine binary would break the conversion, so the
/// caller degrades to the cheap tier instead. The labels real files carry in practice (a browser
/// download at Low) are `NW`-only AND below `0x1800`, so neither applies — and the level test is
/// what makes that claim true of the CODE, not only of the prose. An unreadable label, or a label ACE
/// whose RID does not parse, reads as blocking — the never-break bias.
/// [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
fn label_blocks_lowered_access(path: &Path, rid: u32) -> bool {
    let Some(sddl) = read_label_sddl(path) else {
        return true;
    };
    // No label ACE at all — nothing can block. Both tests below read the SAME ace, never the whole
    // descriptor, so the rights and the level can never describe two different subjects.
    let Some((rights, _sid)) = label_ace_fields(&sddl) else {
        return false;
    };
    if !(rights.contains("NR") || rights.contains("NX")) {
        return false;
    }
    label_ace_rid(&sddl).is_none_or(|ace_rid| ace_rid >= rid)
}

/// The Leg-A grant for one confined spawn: label EVERY write sink the §2.14.1/§2.14.3 placement
/// actually chose at [`CONFINED_INTEGRITY_RID`], and report whether the child's token may
/// therefore be lowered to it. LABEL-THEN-LOWER, never the reverse — the Windows analogue of the
/// Landlock `{scratch rw}` grant (P4.15.1): every engine write sink is OURS (the parent
/// `create_new`s the §2.14.1 `.part`; the §2.14.2 per-run scratch is app-owned), so the parent
/// grants them to the lowered child BEFORE lowering it.
///
/// `scratch` is the per-run cwd, labelled `(OI)(CI)` so the engine's own subtree (a LibreOffice
/// `--outdir` / profile / TMP tree) inherits the level; `out_tmp` is the `.part` publish temp,
/// `None` for a read-only sub-invocation such as the §3.2.1 probe, which writes no artifact. The
/// label is set NON-RECURSIVELY, so the pre-existing `run-<RunId>/.lock` (§2.6.3
/// lock-before-part) keeps its implicit level — and a Medium parent reaches a lower-integrity
/// object regardless, so nothing breaks either way.
///
/// Returns `false` — cheap tier for THIS spawn — unless every sink sits on a
/// `FILE_PERSISTENT_ACLS` volume AND every label call succeeded AND the engine binary carries no
/// read/execute-blocking label. A `false` verdict does NOT unwind the labels already applied: the
/// scratch is labelled before `out_tmp` is evaluated, so a spawn that degrades on the `.part` leaves
/// the per-run scratch (and everything the engine creates inside it) labelled for the rest of the
/// run. That is deliberate and harmless — the token was never lowered, a Medium parent reads, writes
/// and deletes DOWN freely, and the §2.14.3 cross-volume `std::fs::copy` carries no label — but it is
/// a persistent effect of a not-issued grant, so it is stated rather than left to be rediscovered. That covers a FAT/exFAT-stick destination, a `Modify`-only folder
/// without `WRITE_OWNER`, and an SMB share, so no integrity-enforcement assumption about those
/// filesystems is ever load-bearing.
///
/// PROFILE NOTE (§2.12.3 `[DEFER: tuning]`, additive at P4.37): the per-item INPUT file is not in
/// the checked set — `run_confined` has no structured input path (§3.5 flattens it into
/// `plan.args`), exactly as the P4.15.1 Landlock leg has no `{input ro}` grant yet, and no real
/// subprocess engine reads a real input through this seam before the P4.37 image-worker wire. The
/// input's read-blocking-label check joins that box together with the structured path. The
/// residual is bounded: the labels real inputs carry (a browser download at Low) are `NW`-only and
/// below `0x1800`, so reading them from the confined child is unaffected.
/// [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
pub(crate) fn label_confinement_sinks(
    scratch: &Path,
    out_tmp: Option<&Path>,
    program: &Path,
) -> bool {
    if label_blocks_lowered_access(program, CONFINED_INTEGRITY_RID) {
        return false;
    }
    if !(volume_persists_acls(scratch)
        && set_label_sddl(scratch, &label_sddl_dir(CONFINED_INTEGRITY_RID)))
    {
        return false;
    }
    match out_tmp {
        Some(tmp) => {
            volume_persists_acls(tmp)
                && set_label_sddl(tmp, &label_sddl_file(CONFINED_INTEGRITY_RID))
        }
        None => true,
    }
}

/// The verdict of [`strip_mandatory_label`]. `Failed` is the only arm the caller must act on: the
/// §2.1.1 publish then republishes the bytes through a FRESH exclusively-created sibling (a copy
/// carries no label) rather than publishing a still-labelled `final` — never the source, never an
/// existing `final`.
///
/// `allow(dead_code)` OFF WINDOWS: the type is cross-platform (the §2.1.1 publish calls the strip
/// with no per-OS `cfg` at the call site), but only `Absent` is ever CONSTRUCTED elsewhere — the
/// `#[cfg(not(windows))]` [`strip_mandatory_label`] returns exactly that. Matching a variant does not
/// count as constructing it, so `Stripped`/`Failed` read as dead on the Linux + macOS legs and would
/// fail their `clippy -D warnings` (G4/G14). `allow`, never `expect`: on Windows all three ARE
/// constructed, so an `expect` would flip to unfulfilled there (the recorded `expect`→`allow` trap).
#[cfg_attr(
    not(windows),
    allow(
        dead_code,
        reason = "§2.12.3 Leg A is Windows-only (P4.17): the cross-platform strip returns only `Absent` \
                  off Windows, and a matched-but-never-constructed variant reads as dead there"
    )
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LabelStrip {
    /// No explicit mandatory label to remove — every non-Windows publish, and every Windows
    /// publish whose §2.12.3 Leg A degraded to the cheap tier.
    Absent,
    /// The explicit label was removed, and the removal was read back.
    Stripped,
    /// The label is still there, or its state could not be read at all — the caller republishes
    /// through a fresh unlabelled sibling.
    Failed,
}

/// Remove the explicit §2.12.3 mandatory label from the `.part` before the §2.1.2 create-only
/// move, so `final` carries the destination's implicit level. The label TRAVELS with
/// `MoveFileEx`, so without this strip a published output would keep ConvertIA's private
/// `0x1800` level for the rest of its life. Called after the engine exit and the §1.7 non-empty
/// verification, before the publish — ONE site covering every engine and both publish shapes (the
/// same-volume rename and the §2.14.3 cross-volume `std::fs::copy`, which both consume this
/// `.part`). An UNLABELLED `.part` — every spawn whose Leg A degraded — is [`LabelStrip::Absent`]:
/// a cheap read-back, no write.
///
/// Every uncertainty resolves to [`LabelStrip::Failed`], never to `Absent`: a label state that
/// cannot be READ is not evidence that there is none, and the fallback the `Failed` arm routes to
/// (republish through a fresh sibling) is unlabelled by construction. That is the same
/// never-break/never-assume bias [`label_blocks_lowered_access`] carries, applied to the other end
/// of the leg — the grant-IS-the-enforcement doctrine cuts both ways.
/// [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
pub(crate) fn strip_mandatory_label(path: &Path) -> LabelStrip {
    let Some(before) = read_label_sddl(path) else {
        return LabelStrip::Failed;
    };
    if !before.contains(LABEL_ACE_MARKER) {
        return LabelStrip::Absent;
    }
    if !set_label_sddl(path, LABEL_SDDL_NONE) {
        return LabelStrip::Failed;
    }
    match read_label_sddl(path) {
        Some(after) if !after.contains(LABEL_ACE_MARKER) => LabelStrip::Stripped,
        _ => LabelStrip::Failed,
    }
}

/// The non-Windows sibling of [`strip_mandatory_label`]: no other OS's §2.12.3 tier labels a
/// publish temp, so there is never a label to remove. Present unconditionally so the §2.1.1
/// publish sequence calls it with no per-OS `cfg` at the call site (the [`ensure_executable`]
/// precedent). [Build-Session-Entscheidung: P4.17]
#[cfg(not(windows))]
pub(crate) fn strip_mandatory_label(_path: &Path) -> LabelStrip {
    LabelStrip::Absent
}

/// Open the just-spawned, still-suspended child by PID with exactly the rights the two legs need
/// — `PROCESS_SET_QUOTA` + `PROCESS_TERMINATE` for `AssignProcessToJobObject`, and
/// `PROCESS_QUERY_INFORMATION` for `OpenProcessToken`. The PID is taken rather than the child's
/// raw handle so no raw pointer crosses the `crate::isolation` → `crate::platform` boundary
/// (`crate::isolation` stays unsafe-free per its P3.2 contract map), and PID REUSE is impossible
/// here: the caller still holds the child's own process handle, which pins the PID for the whole
/// call. [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
fn open_child(pid: u32) -> Option<std::os::windows::io::OwnedHandle> {
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };
    // SAFETY: an argument-less-by-value call whose only pointer is the returned handle; a failure
    // yields a null handle, which the check below rejects before it is ever adopted.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let raw = unsafe {
        OpenProcess(
            PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_INFORMATION,
            0,
            pid,
        )
    };
    if raw.is_null() {
        return None;
    }
    // SAFETY: `raw` is a non-null process handle this call just opened and nothing else owns, so
    // adopting it into an `OwnedHandle` transfers the single close responsibility exactly once.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    Some(unsafe { OwnedHandle::from_raw_handle(raw) })
}

/// Write one `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` onto `job`: the [`JobLimits`] caps plus
/// `JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION` (so a crashed engine cannot sit in a Windows
/// Error Reporting dialog until the §1.7 watchdog reaps it), and `KILL_ON_JOB_CLOSE` iff
/// `kill_on_close`. Re-callable, so [`ConfinedJob::stand_down`] re-applies the same caps with the
/// flag dropped. [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
fn set_job_limits(
    job: std::os::windows::io::RawHandle,
    limits: JobLimits,
    kill_on_close: bool,
) -> bool {
    use windows_sys::Win32::System::JobObjects::{
        JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
        JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION, JOB_OBJECT_LIMIT_JOB_MEMORY,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    let mut flags = JOB_OBJECT_LIMIT_JOB_MEMORY
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
        | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
    if kill_on_close {
        flags |= JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    }
    info.BasicLimitInformation.LimitFlags = flags;
    info.BasicLimitInformation.ActiveProcessLimit = limits.active_processes;
    // A cap wider than the address space is not expressible; saturating keeps the widest
    // expressible cap (still a runaway guard) instead of degrading the whole leg.
    info.JobMemoryLimit = usize::try_from(limits.memory_bytes).unwrap_or(usize::MAX);
    let size = u32::try_from(std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
        .unwrap_or(u32::MAX);
    // SAFETY: `job` is a live job handle owned by the caller; `info` is a fully-initialised,
    // correctly-typed `repr(C)` struct whose own `size_of` is passed as the length.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let ok = unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&raw const info).cast(),
            size,
        )
    };
    ok != 0
}

/// Create ConvertIA's own Job Object with `limits` + kill-on-job-close and assign the still-
/// suspended child `pid` to it (§2.12.3 Leg B, P4.17). The child is already destined for
/// `process_wrap`'s own job in `wrap_child`; Windows 8+ NESTED jobs make ours the outer one, so
/// both coexist and the P4.10 `TerminateJobObject` group-kill keeps working on the inner job while
/// our caps + kill-on-close cover the whole tree. Every failure yields `None` ⇒ cheap tier for
/// this spawn, never an error the caller must handle.
///
/// **Nested-job support is LOAD-BEARING ON THE SPAWN PATH, not merely on the caps.** Unlike every
/// other failure here, a refusal of the SECOND assignment is not ours to swallow: it would surface
/// inside `process_wrap`'s `JobObject::wrap_child`, whose `?` turns it into a `spawn()` error — i.e.
/// EVERY Windows conversion would fail. Windows 8 introduced nesting and §0.8 floors the product at
/// Windows 10, so the assumption holds on the whole supported range; it is pinned directly rather
/// than by inference by `a_second_job_assignment_nests_rather_than_failing`.
/// [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
fn attach_job_with(pid: u32, limits: JobLimits) -> Option<ConfinedJob> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::System::JobObjects::{AssignProcessToJobObject, CreateJobObjectW};
    let child = open_child(pid)?;
    // SAFETY: both arguments are the documented "no attributes / unnamed" nulls; the returned
    // handle is null on failure, which the check below rejects before it is adopted.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let raw_job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if raw_job.is_null() {
        return None;
    }
    // SAFETY: `raw_job` is a non-null job handle this call just created and nothing else owns, so adopting
    // it transfers the single close responsibility exactly once — and that close is the reap.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let job = unsafe { OwnedHandle::from_raw_handle(raw_job) };
    if !set_job_limits(job.as_raw_handle(), limits, true) {
        return None;
    }
    // SAFETY: both handles are live and owned here — the job just created, the child just opened.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let assigned = unsafe { AssignProcessToJobObject(job.as_raw_handle(), child.as_raw_handle()) };
    if assigned == 0 {
        return None;
    }
    Some(ConfinedJob { job, limits })
}

/// §2.12.3 Leg B (P4.17) with the production caps — see [`attach_job_with`].
#[cfg(windows)]
pub(crate) fn attach_confined_job(pid: u32) -> Option<ConfinedJob> {
    attach_job_with(pid, PRODUCTION_JOB_LIMITS)
}

/// Read the integrity RID of `pid`'s primary token — the grant-IS-the-enforcement read-back that
/// turns "we called `SetTokenInformation`" into "the child really runs at this level". Used to
/// verify the Leg-A lowering and, at P4.18, to feed the achieved-tier record.
/// [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
pub(crate) fn child_integrity_rid(pid: u32) -> Option<u32> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::Security::{
        GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TokenIntegrityLevel,
        TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::OpenProcessToken;
    let child = open_child(pid)?;
    let handle = child.as_raw_handle();
    let mut raw_token: windows_sys::Win32::Foundation::HANDLE = std::ptr::null_mut();
    // SAFETY: `handle` is a live process handle opened with `PROCESS_QUERY_INFORMATION`; the token
    // out-param is a valid local receiving a handle adopted exactly once below.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let opened = unsafe { OpenProcessToken(handle, TOKEN_QUERY, &raw mut raw_token) };
    if opened == 0 || raw_token.is_null() {
        return None;
    }
    // SAFETY: `raw_token` is the non-null token handle just opened and owned by nobody else.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let token = unsafe { OwnedHandle::from_raw_handle(raw_token) };
    let mut needed: u32 = 0;
    // SAFETY: the documented size-probe form — a null buffer with a zero length makes the call fail
    // with the required byte count written to `needed`.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenIntegrityLevel,
            std::ptr::null_mut(),
            0,
            &raw mut needed,
        );
    }
    if needed == 0 {
        return None;
    }
    // Backing store: a `Vec<u64>`, NOT a `Vec<u8>` — `TOKEN_MANDATORY_LABEL` embeds a `PSID`, so it is
    // 8-byte-aligned, and a `Vec<u8>` guarantees only alignment 1; the `*const TOKEN_MANDATORY_LABEL` cast
    // below must be well-aligned by construction, not by the allocator's habit. Mirrors the
    // `FILE_RENAME_INFORMATION` backing store the §2.1.2 publish uses above.
    let mut buffer = vec![0u64; (needed as usize).div_ceil(std::mem::size_of::<u64>())];
    // SAFETY: `buffer` is a caller-owned allocation of at least `needed` bytes (the `div_ceil` rounds UP)
    // whose true byte length is passed, so the call writes strictly inside it.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let read = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenIntegrityLevel,
            buffer.as_mut_ptr().cast(),
            needed,
            &raw mut needed,
        )
    };
    if read == 0 {
        return None;
    }
    // The `Vec<u64>` store makes the cast below well-aligned for `TOKEN_MANDATORY_LABEL` by construction.
    // SAFETY: the call above filled `buffer` with a label whose `Label.Sid` points at the SID inside that
    // same 8-byte-aligned buffer; the LAST sub-authority of a valid SID is the integrity RID.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    unsafe {
        let label = buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>();
        let sid = (*label).Label.Sid;
        let count = *GetSidSubAuthorityCount(sid);
        if count == 0 {
            return None;
        }
        Some(*GetSidSubAuthority(sid, u32::from(count - 1)))
    }
}

/// §2.12.3 Leg A (P4.17) — lower the still-suspended child `pid`'s primary token to the mandatory
/// integrity level `rid` and READ THE LEVEL BACK. Production always passes
/// [`CONFINED_INTEGRITY_RID`], which `crate::isolation` supplies from the grant; the level is a PARAMETER rather than a
/// second hardcoded literal so the tier tests can stand a co-tenant up at a different level and prove
/// the enforcement claim from the other side. Called only after [`label_confinement_sinks`] granted
/// every write sink at the same level (label-then-lower, never the reverse). Lowering needs no
/// privilege; RAISING is refused by the kernel (`ERROR_INVALID_LABEL`), so this can only ever
/// restrict. Returns the per-leg outcome the P4.18 achieved-tier record consumes: `Applied` when the
/// read-back confirms the level, `Degraded(NotApplied)` when the call SUCCEEDED but the read-back does
/// not show it, `Degraded(Unavailable)` when the token / the SID / the write itself could not be
/// obtained at all. Never an error — the caller silently keeps the cheap tier.
/// [Build-Session-Entscheidung: P4.17]
#[cfg(windows)]
pub(crate) fn lower_child_to(pid: u32, rid: u32) -> LegOutcome {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
    use windows_sys::Win32::Security::{
        GetLengthSid, SetTokenInformation, TokenIntegrityLevel, PSID, SID_AND_ATTRIBUTES,
        TOKEN_ADJUST_DEFAULT, TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::OpenProcessToken;
    // Win32 `SE_GROUP_INTEGRITY` (winnt.h) — declared locally so the `windows-sys` feature set
    // stays at the four this tier needs (the `FILE_PERSISTENT_ACLS` precedent above).
    const SE_GROUP_INTEGRITY: u32 = 0x0000_0020;
    let Some(sid_text) = wide_nul(std::ffi::OsStr::new(&integrity_sid(rid))) else {
        return LegOutcome::Degraded(DegradeReason::Unavailable);
    };
    let Some(child) = open_child(pid) else {
        return LegOutcome::Degraded(DegradeReason::Unavailable);
    };
    let mut raw_token: windows_sys::Win32::Foundation::HANDLE = std::ptr::null_mut();
    // SAFETY: `child` is a live process handle opened with `PROCESS_QUERY_INFORMATION`; the token
    // out-param is a valid local receiving a handle adopted exactly once below.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let opened = unsafe {
        OpenProcessToken(
            child.as_raw_handle(),
            TOKEN_ADJUST_DEFAULT | TOKEN_QUERY,
            &raw mut raw_token,
        )
    };
    if opened == 0 || raw_token.is_null() {
        return LegOutcome::Degraded(DegradeReason::Unavailable);
    }
    // SAFETY: `raw_token` is the non-null token handle just opened and owned by nobody else.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let token = unsafe { OwnedHandle::from_raw_handle(raw_token) };
    let mut sid: PSID = std::ptr::null_mut();
    // SAFETY: `sid_text` is a NUL-terminated UTF-16 SID string alive across the call; `sid` is a
    // valid out-param receiving a `LocalAlloc` block freed exactly once below.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let converted = unsafe { ConvertStringSidToSidW(sid_text.as_ptr(), &raw mut sid) };
    if converted == 0 || sid.is_null() {
        return LegOutcome::Degraded(DegradeReason::Unavailable);
    }
    let label = TOKEN_MANDATORY_LABEL {
        Label: SID_AND_ATTRIBUTES {
            Sid: sid,
            Attributes: SE_GROUP_INTEGRITY,
        },
    };
    // SAFETY: `sid` is the valid SID just converted; `GetLengthSid` reads only its own header. The
    // documented length for a `TokenIntegrityLevel` write is the struct plus the SID it points at.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let length = unsafe { GetLengthSid(sid) }
        .saturating_add(u32::try_from(std::mem::size_of::<TOKEN_MANDATORY_LABEL>()).unwrap_or(0));
    // SAFETY: `token` is a live token opened with `TOKEN_ADJUST_DEFAULT`; `label` is a fully-initialised
    // `repr(C)` struct whose `Sid` outlives the call, and `length` is the documented struct-plus-SID size.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let set = unsafe {
        SetTokenInformation(
            token.as_raw_handle(),
            TokenIntegrityLevel,
            (&raw const label).cast(),
            length,
        )
    };
    // SAFETY: `sid` is the `LocalAlloc` block `ConvertStringSidToSidW` returned, freed exactly once
    // and only after the `SetTokenInformation` that read through it has returned.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    unsafe { LocalFree(sid) };
    if set == 0 {
        // The WRITE ITSELF was refused (a policy, a token we cannot adjust, an out-of-range level) — the
        // grant could not be obtained at all, which is `Unavailable` in the shared per-OS vocabulary.
        // `NotApplied` is reserved for the sharper signal below: the call SUCCEEDED yet the kernel does
        // not show the level, i.e. a grant that returned without enforcing.
        return LegOutcome::Degraded(DegradeReason::Unavailable);
    }
    match child_integrity_rid(pid) {
        Some(read_back) if read_back == rid => LegOutcome::Applied,
        Some(_) => LegOutcome::Degraded(DegradeReason::NotApplied),
        None => LegOutcome::Degraded(DegradeReason::Unavailable),
    }
}

#[cfg(test)]
mod ephemeral_tests {
    use super::is_ephemeral_output_dir;

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

    // §6.4.1 unit (G15) / §2.7.2: a dir NOT under any known temp root is NOT ephemeral — the negative
    // branch is proven against a real, canonicalisable directory, not a fabricated one. [Test-Change:
    // P3.72 — old-obsolete+new-correct, §2.7.2] the former specimen (`CARGO_MANIFEST_DIR`, "the CI
    // workspace is never under the OS temp root") is obsolete: the P3.72 `cargo-mutants` gate runs this
    // suite from a tree COPY under the OS temp root, where the source root genuinely IS ephemeral — a
    // checkout-location assumption, not a §2.7.2 fact. The home dir is never a temp root on any OS, so
    // the same negative branch holds in every execution environment (repo checkout, CI, a temp-dir copy).
    #[test]
    fn a_non_temp_dir_is_not_ephemeral() {
        let home = std::env::home_dir().expect("a real home dir on every dev/CI environment");
        assert!(
            !is_ephemeral_output_dir(&home),
            "§2.7.2: the user home dir ({home:?}) is not under any OS temp root → not ephemeral"
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
// exercised end-to-end on a real temp dir (never mock the FS under test, test-strategy §0.1). P3.65 adds the
// converse leg: wherever a host DOES have a FAT/exFAT filesystem mounted, the detector is asserted against it
// using the kernel's own mount table as the independent oracle. STANDALONE `#[cfg(test)]` (clippy
// `is_cfg_test` recognition, P1.17).
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

    // §2.7.2 (G15/G31, P3.65) THE DETECTOR AGAINST A REAL FAT/exFAT FILESYSTEM, wherever the host has one
    // mounted. Both the mount point and its TYPE come from the kernel's own mount table (`/proc/self/mounts`,
    // read by `crate::test_volumes::fat_class_mount`), NOT from this detector — so the assertion is an
    // independent check of the classifier rather than a restatement of it (asking the detector to find its own
    // subject would assert nothing). `None` = no such filesystem mounted here (the ordinary case, including the
    // CI runners) → a clean skip; where the substrate DOES exist, a miss is exactly the P3.18 "list-miss" the
    // reactive §2.1.2 backstop covers, and catching it is the point. READ-ONLY — nothing is written to what may
    // be removable media. [Build-Session-Entscheidung: P3.65]
    #[cfg(target_os = "linux")]
    #[test]
    fn a_real_kernel_reported_fat_mount_classifies_as_fat_class() {
        let Some(mount_point) = crate::test_volumes::fat_class_mount() else {
            return; // no FAT/exFAT filesystem mounted on this host — the magic-boundary proof above stands.
        };
        // `!= Some(false)` on purpose, not `== Some(true)`: a `statfs` READ FAILURE (a permission-restricted
        // `/boot/efi`, a vanished mount) is a spec-sanctioned outcome the §2.7.2 caller treats as
        // heuristic-indeterminate (the P3.18 list-miss honesty), so it must not red the build. Only a genuine
        // CLASSIFICATION MISS — the read succeeded and said "not FAT-class" — is the defect this pins.
        assert_ne!(
            lacks_atomic_publish_primitive(&mount_point).ok(),
            Some(false),
            "§2.7.2: a filesystem the kernel reports as vfat/exfat/msdos at {mount_point:?} MUST NOT classify \
             as non-FAT-class"
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

// §2.12.3 best-effort Linux privilege-drop tier (P4.15) — per-leg tests. Linux-only + real subprocess:
// each leg's enforcement is only observable through a REAL child under a REAL kernel (grant-is-enforcement),
// never a mock (test-strategy §0.1). TWO STACKED cfg attrs (`#[cfg(test)]` then `#[cfg(target_os = "linux")]`)
// — NOT a compound `all(test, target_os = "linux")` (the clippy `is_cfg_test` trap; the P1.17 precedent).
#[cfg(test)]
#[cfg(target_os = "linux")]
mod privilege_drop_tests {
    use super::{
        build_seccomp_program_for, install_confinement, landlock_probe, seccomp_denied_syscalls,
        seccomp_probe, DegradeReason, LegOutcome,
    };
    use std::os::unix::process::CommandExt;
    use std::path::Path;
    use std::process::Command;

    // Run `/bin/sh -c <script>` in `scratch` under the FULL P4.15 tier (net-ns + Landlock + seccomp) via the
    // SAME `install_confinement` the production spawn uses — not a re-implementation.
    fn confined_sh(scratch: &Path, script: &str) -> std::process::Output {
        let sh = Path::new("/bin/sh");
        // A TEST-ONLY spawn of /bin/sh to OBSERVE the P4.15 confinement — NOT a production engine spawn (those
        // route through crate::isolation::run_confined, the G29-sanctioned site). It deliberately keeps the
        // inherited env so the shell can resolve `cat`/`mkdir` via PATH (env_clear would break the observation),
        // and homes in crate::platform because the sibling seccomp test needs the allow-listed `pre_exec`.
        // nosemgrep: convertia-command-outside-isolation, convertia-command-missing-env-clear
        let mut cmd = Command::new(sh);
        cmd.arg("-c").arg(script).current_dir(scratch);
        install_confinement(&mut cmd, sh, scratch);
        cmd.output().expect("spawn /bin/sh")
    }

    // §2.12.3 P4.15.3 (deny-list CONTENT): the seccomp deny-list must EXCLUDE execve/execveat + unshare +
    // setpgid (denying them would break the engine launch / the net-ns leg / the P4.10 group-leader) and
    // INCLUDE the exploit primitives; and be deterministic (sorted → byte-stable BPF).
    #[test]
    fn seccomp_denylist_excludes_launch_syscalls_and_covers_exploit_primitives() {
        let denied = seccomp_denied_syscalls();
        for allowed in [
            libc::SYS_execve,
            libc::SYS_execveat,
            libc::SYS_unshare,
            libc::SYS_setpgid,
        ] {
            assert!(
                !denied.contains(&allowed),
                "syscall {allowed} must NOT be denied (engine launch / net-ns / group-leader): {denied:?}"
            );
        }
        for primitive in [
            libc::SYS_ptrace,
            libc::SYS_mount,
            libc::SYS_bpf,
            libc::SYS_kexec_load,
            libc::SYS_setns,
        ] {
            assert!(
                denied.contains(&primitive),
                "exploit-primitive syscall {primitive} must be denied: {denied:?}"
            );
        }
        let mut sorted = denied.clone();
        sorted.sort_unstable();
        assert_eq!(
            denied, sorted,
            "the deny-list must be sorted for a byte-stable BPF program"
        );
    }

    // §2.12.3 P4.15.1 (grant-is-enforcement): a Landlock-confined child CANNOT read a file OUTSIDE
    // {scratch, /dev, /proc, its bundle dir, the system dirs}, yet CAN still read its own scratch. Where
    // the kernel lacks Landlock (probe = Degraded) the leg silent-degrades and the read is NOT gated — the
    // test asserts the arm matching the runner, so it is honest on a 5.13+ CI runner AND an older one.
    #[test]
    fn landlock_denies_out_of_sandbox_reads_when_enforced() {
        let scratch = tempfile::tempdir().expect("scratch dir");
        std::fs::write(scratch.path().join("ok.txt"), b"ok").expect("write a scratch file");
        let secret_dir = tempfile::tempdir().expect("a sibling dir NOT under scratch");
        let secret = secret_dir.path().join("secret.txt");
        std::fs::write(&secret, b"TOP SECRET").expect("write the out-of-sandbox file");
        let script = format!(
            "cat ok.txt >/dev/null 2>&1 && echo scratch=ok || echo scratch=FAIL; \
             cat '{}' >/dev/null 2>&1 && echo secret=LEAK || echo secret=denied",
            secret.display()
        );
        let out = confined_sh(scratch.path(), &script);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("scratch=ok"),
            "the confined child must still read its own scratch (never break the conversion): {stdout}"
        );
        match landlock_probe() {
            LegOutcome::Applied => assert!(
                stdout.contains("secret=denied"),
                "Landlock enforced — the out-of-sandbox read MUST be denied: {stdout}"
            ),
            LegOutcome::Degraded(_) => assert!(
                stdout.contains("secret=LEAK"),
                "Landlock degraded on this kernel — the read is not gated (silent-degrade): {stdout}"
            ),
        }
    }

    // §2.12.3 P4.15.3 (seccomp mechanism): a syscall in a filter's deny-list returns EPERM in the child.
    // A CUSTOM filter denying BOTH mkdir variants (glibc uses mkdir(2) on some arches, mkdirat on others)
    // makes `mkdir` fail regardless — the mechanism, distinct from the production deny-list content above.
    #[test]
    fn seccomp_denies_a_listed_syscall_in_the_child() {
        let scratch = tempfile::tempdir().expect("scratch dir");
        // Deny BOTH mkdir variants so `mkdir` fails regardless of which glibc issues. `SYS_mkdir` exists only
        // on x86_64 (aarch64/riscv define only `SYS_mkdirat`), so it is arch-guarded to keep this test portable
        // if a non-x86_64 Linux target is ever added (v1 Linux is x86_64-only). [Build-Session-Entscheidung: P4.15]
        let mut denied = vec![libc::SYS_mkdirat];
        #[cfg(target_arch = "x86_64")]
        denied.push(libc::SYS_mkdir);
        let Some(program) = build_seccomp_program_for(&denied) else {
            return; // unsupported arch — the leg degrades; nothing to assert.
        };
        // A TEST-ONLY spawn of /bin/sh to OBSERVE that a seccomp-denied syscall returns EPERM in the child —
        // NOT a production engine spawn (crate::isolation owns those); it keeps the inherited env (the shell
        // resolves `mkdir` via PATH) and homes here because the `pre_exec` below needs the allow-listed unsafe.
        // nosemgrep: convertia-command-outside-isolation, convertia-command-missing-env-clear
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c")
            .arg("mkdir denied_dir 2>/dev/null && echo mkdir=ALLOWED || echo mkdir=denied")
            .current_dir(scratch.path());
        // SAFETY: post-fork / pre-exec, async-signal-safe only — seccompiler `apply_filter` (prctl + seccomp
        // syscalls) on a BPF program built in the parent; no alloc / no panic. Mirrors install_confinement.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        unsafe {
            cmd.pre_exec(move || {
                let _ = seccompiler::apply_filter(&program);
                Ok(())
            });
        }
        let out = cmd.output().expect("spawn /bin/sh");
        let stdout = String::from_utf8_lossy(&out.stdout);
        match seccomp_probe() {
            LegOutcome::Applied => assert!(
                stdout.contains("mkdir=denied"),
                "seccomp enforced — the denied syscall MUST return EPERM in the child: {stdout}"
            ),
            LegOutcome::Degraded(_) => { /* seccomp unavailable on this kernel — silent-degrade, no assertion */
            }
        }
    }

    // §2.12.3: the tier is NON-load-bearing — applying it must NEVER break a benign conversion. A confined
    // child touching only its scratch + system paths runs to a clean exit on EVERY kernel (enforced or
    // degraded), so the P4.13 cheap-tier floor is never regressed.
    #[test]
    fn confinement_never_breaks_a_benign_confined_spawn() {
        let scratch = tempfile::tempdir().expect("scratch dir");
        let out = confined_sh(scratch.path(), "echo hello > out.txt");
        assert!(
            out.status.success(),
            "a benign confined spawn must succeed (best-effort, never breaks): {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            std::fs::read_to_string(scratch.path().join("out.txt"))
                .unwrap_or_default()
                .trim(),
            "hello",
            "the confined child wrote its output into the scratch dir"
        );
    }

    // Each leg's availability probe returns a well-formed LegOutcome (never a panic), on any runner.
    #[test]
    fn the_leg_probes_report_a_well_formed_outcome() {
        assert!(matches!(
            landlock_probe(),
            LegOutcome::Applied
                | LegOutcome::Degraded(DegradeReason::Unavailable | DegradeReason::NotApplied)
        ));
        assert!(matches!(
            seccomp_probe(),
            LegOutcome::Applied | LegOutcome::Degraded(DegradeReason::Unavailable)
        ));
    }

    // §2.12.3 P4.15.2 (net-ns): a confined child is placed in a FRESH, isolated network namespace when
    // unprivileged user+net namespaces are available, else the leg silently degrades (the child shares the
    // host netns). Observed via the child's `/proc/self/ns/net` inode vs the parent's. This ALSO proves the
    // never-break property: the net-ns setup runs INSIDE the pre_exec closure, so a failure just skips and the
    // child still runs. The egress-deny enforcement itself is environment-gated (userns is restricted on many
    // CI runners) + non-load-bearing (§0.11 T9b) — so the applied arm is asserted only WHEN observable.
    #[test]
    fn net_ns_isolates_the_child_or_degrades() {
        let scratch = tempfile::tempdir().expect("scratch dir");
        let out = confined_sh(
            scratch.path(),
            "readlink /proc/self/ns/net > netns.txt 2>/dev/null; echo done",
        );
        assert!(
            out.status.success(),
            "the confined child must run — the net-ns leg never breaks the spawn: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let child_ns =
            std::fs::read_to_string(scratch.path().join("netns.txt")).unwrap_or_default();
        assert!(
            !child_ns.trim().is_empty(),
            "the confined child must read its own /proc/self/ns/net (proc granted read): {child_ns:?}"
        );
        let parent_ns = std::fs::read_link("/proc/self/ns/net")
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        if child_ns.trim() != parent_ns.trim() {
            // net-ns APPLIED: the child is in a different (fresh, empty-but-loopback) network namespace.
            assert!(
                child_ns.trim().starts_with("net:["),
                "an isolated network namespace link: {child_ns:?}"
            );
        }
        // else: SAME namespace = silent-degrade (unprivileged userns unavailable on this runner) — accepted.
    }
}

// §2.12.3 macOS privilege-drop DECISION pin (P4.16, Co-Pilot ruling 2026-07-25 — anchor: never-break >
// non-load-bearing defence-in-depth). The macOS Seatbelt tier is DECIDED cheap-tier only: no private-libsandbox
// apply FFI enters the core, because its only apply path (a call in the post-fork/pre-exec child of the
// multithreaded host) is neither auditable fork-safe nor silent-skippable at its worst case (a hang, not an
// errno), so §2.12.3's never-break floor forbids it. This CROSS-PLATFORM source-scan (runs on ALL THREE CI
// legs incl. macOS — the macOS apply path is untestable-on-host, so the decision is pinned structurally, not
// at runtime) walks the TWO directories that could home a Seatbelt apply RECURSIVELY, so a future submodule
// (e.g. the P4.24 `isolation/macos.rs` the P4.85 homing contract designates) is covered AUTOMATICALLY — the
// scan does not silently stop enforcing when its target grows (the g24 target-absent-leg lesson). It FAILS if
// any edit reintroduces such a call/FFI into `crate::platform` (the sole G29 ALLOWED_UNSAFE_MODULES entry,
// where a `sandbox_*` FFI may be DECLARED) or `crate::isolation` (the spawn path that would CALL it — a call
// in any third module would have to call a `platform`-declared FFI, which the `platform` scan catches at its
// declaration). [Build-Session-Entscheidung: P4.16]

#[cfg(test)]
mod macos_seatbelt_decision_tests {
    // Everything before a scanned file's FIRST `#[cfg(test)]`, so a needle can never match a test's own
    // source (this module names the forbidden tokens in its assertions). `concat!`-split so the literal
    // marker is absent from this scanning module too (the `c_surface_scan::production_prefix` precedent).
    fn production_prefix(full: &str) -> &str {
        full.split_once(concat!("#[cfg", "(test)]"))
            .map_or(full, |(prefix, _)| prefix)
    }

    // The private libsandbox apply/compile/init family — the only way to APPLY a Seatbelt profile from Rust,
    // and (per the P4.16 ruling) exactly what must NOT exist in the core. Substrings so `_with_parameters`
    // and `_bytecode` variants are covered too.
    const FORBIDDEN_APPLY_TOKENS: [&str; 3] = ["sandbox_init", "sandbox_apply", "sandbox_compile"];

    // §2.12.3 / P4.16 Co-Pilot ruling: no private-libsandbox apply call or FFI in the core's isolation surface.
    // Walks `src/platform/**` + `src/isolation/**` (from the compile-time crate root) recursively, so a new
    // file under either — the P4.24 `isolation/macos.rs`, an `isolation/macos/` subdir — is scanned WITHOUT a
    // future editor having to extend a hardcoded file list.
    #[test]
    fn no_seatbelt_apply_callsite_in_the_core() {
        use walkdir::WalkDir;
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut scanned = 0usize;
        let (mut saw_platform_root, mut saw_isolation_root) = (false, false);
        for dir in ["platform", "isolation"] {
            for entry in WalkDir::new(src.join(dir)) {
                let entry = entry.expect("walk the core platform/isolation source tree");
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                    continue;
                }
                let full = std::fs::read_to_string(path).expect("read a core source file");
                let prod = production_prefix(&full);
                for token in FORBIDDEN_APPLY_TOKENS {
                    assert!(
                        !prod.contains(token),
                        "§2.12.3 P4.16 decision (never-break > non-load-bearing DiD): a `{token}` call/FFI \
                         reappeared in {}'s production source. The macOS Seatbelt apply leg is DECIDED \
                         cheap-tier only — its worst-case fork-child hang is not silent-skippable, so it cannot \
                         ship. If a signed/notarized build with a safe apply path is being added, revisit the \
                         Co-Pilot ruling (2026-07-25) + spec §2.12.3 FIRST.",
                        path.display()
                    );
                }
                scanned += 1;
                saw_platform_root |= path.ends_with("platform/mod.rs");
                saw_isolation_root |= path.ends_with("isolation/mod.rs");
            }
        }
        // Hermetic guard (the g24 lesson — never silently watch nothing): the scan MUST have read the two
        // known homes. If the tree layout ever changes so they are not found, FAIL loudly rather than pass vacuously.
        assert!(
            saw_platform_root && saw_isolation_root && scanned >= 2,
            "the source-scan must cover crate::platform + crate::isolation (found platform={saw_platform_root}, \
             isolation={saw_isolation_root}, {scanned} .rs files) — the P4.16 decision is not being enforced"
        );
    }
}

// §2.12.3 / G64 (G15, P4.18): the ACHIEVED-TIER RECORD BINDING. The tracked
// `privilege-drop-coverage.toml` and this module's `ATTACHED_LEGS` / `attached_tier()` / `VERDICT_SOURCES`
// are ONE fact stated in two places, so they are held identical here. `include_str!` pins the REAL
// committed file (never a copy), so a leg removed from the code reddens against the unchanged row, and a
// row edited in the file reddens against the unchanged code — on every leg of the 3-OS CI matrix. That is
// the host-INDEPENDENT half of the G64 decrease guard (the host-dependent half is the per-spawn
// `SpawnTier` the P4.18.1 regression asserts through a real confined spawn).
#[cfg(test)]
mod privilege_drop_record_tests {
    use super::{attached_tier, ATTACHED_LEGS, TIER_CHEAP, TIER_PRIVILEGE_DROP, VERDICT_SOURCES};

    /// The REAL tracked record — the file the G64 ratchet reads, never a fixture copy.
    const RECORD: &str = include_str!("../../../privilege-drop-coverage.toml");

    /// The three platforms ConvertIA ships (CLAUDE.md §1: one artifact per platform, no fourth target).
    const PLATFORMS: [&str; 3] = ["linux", "macos", "windows"];

    // A deliberately tiny TOML reader. The record is a flat table-per-platform file of string scalars and
    // string arrays, and the MIT core ships no `toml` crate — pulling one in (plus its §0.8 floor row) to
    // read four keys would be a dependency bought for a test's convenience. It understands exactly the two
    // shapes the record uses and answers `None` for anything else. [Build-Session-Entscheidung: P4.18]
    fn section(name: &str) -> String {
        let head = format!("[{name}]");
        let mut body = String::new();
        let mut inside = false;
        for line in RECORD.lines() {
            let trimmed = line.trim();
            // A comment can legitimately contain a key-looking or table-looking word, so comments are
            // dropped BEFORE the table-header test — otherwise a commented-out header would flip sections.
            if trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with('[') {
                inside = trimmed == head;
                continue;
            }
            if inside {
                body.push_str(trimmed);
                body.push('\n');
            }
        }
        body
    }

    fn scalar(body: &str, key: &str) -> Option<String> {
        body.lines()
            .filter_map(|line| line.split_once('='))
            .find(|(name, _)| name.trim() == key)
            .map(|(_, value)| value.trim().trim_matches('"').to_owned())
    }

    fn array(body: &str, key: &str) -> Option<Vec<String>> {
        let raw = scalar(body, key)?;
        let inner = raw.trim().strip_prefix('[')?.strip_suffix(']')?;
        Some(
            inner
                .split(',')
                .map(|item| item.trim().trim_matches('"').to_owned())
                .filter(|item| !item.is_empty())
                .collect(),
        )
    }

    fn this_platform() -> &'static str {
        if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(windows) {
            "windows"
        } else {
            ""
        }
    }

    // The load-bearing binding: this build's leg set, tier and verdict source ARE the row. A phase that
    // drops a leg — the exact NET regression G64 exists to surface — changes `ATTACHED_LEGS` and fails here
    // against the unchanged record, so re-blessing the record becomes a visible, reviewable diff.
    #[test]
    fn the_record_row_for_this_platform_matches_the_legs_this_build_attaches() {
        let platform = this_platform();
        assert!(
            PLATFORMS.contains(&platform),
            "§2.12.3/G64: the build targets one of the three shipped platforms, got {:?}",
            std::env::consts::OS
        );
        let row = section(&format!("platform.{platform}"));
        let legs = array(&row, "legs").expect("the platform row carries a `legs` array");
        assert_eq!(
            legs, ATTACHED_LEGS,
            "§2.12.3/G64: `privilege-drop-coverage.toml` [platform.{platform}].legs must equal the legs \
             this build attaches — a leg added or removed in code is a tier change and is re-blessed in \
             the record (decrease-guarded)"
        );
        assert_eq!(
            scalar(&row, "tier").expect("the platform row carries a `tier`"),
            attached_tier(),
            "§2.12.3/G64: the recorded tier must equal the tier this build's leg set reaches"
        );
        let sources = section(&format!("platform.{platform}.verdict_source"));
        let recorded: Vec<(String, String)> = legs
            .iter()
            .map(|leg| {
                (
                    leg.clone(),
                    scalar(&sources, leg)
                        .unwrap_or_else(|| format!("<no verdict_source recorded for `{leg}`>")),
                )
            })
            .collect();
        let expected: Vec<(String, String)> = VERDICT_SOURCES
            .iter()
            .map(|(leg, source)| ((*leg).to_owned(), (*source).to_owned()))
            .collect();
        assert_eq!(
            recorded, expected,
            "§2.12.3/G64: [platform.{platform}.verdict_source] must name, PER LEG, where that leg's verdict \
             actually comes from — a parent-side read-back of the running child (`per-spawn`) or the \
             kernel's answer for this host (`host-probe`). The strength of an `Applied` is its source's, so \
             a leg that quietly loses its per-spawn read-back must show up as a record change"
        );
    }

    // The record is read by a release-tier gate for EVERY platform, so every row must parse and mean
    // something on the two legs where the code binding above does not run.
    #[test]
    fn every_shipped_platform_has_a_well_formed_row() {
        let meta = section("meta");
        assert_eq!(
            scalar(&meta, "schema").expect("[meta].schema"),
            "1",
            "§2.12.3/G64: the record schema this test knows how to read. A bump means the shape changed — \
             re-read the parser and the assertions below BEFORE re-blessing this number, or the binding \
             silently starts checking the wrong keys"
        );
        let tier_order = array(&meta, "tier_order").expect("[meta].tier_order");
        let verdict_sources = array(&meta, "verdict_sources").expect("[meta].verdict_sources");
        assert_eq!(
            tier_order,
            vec![TIER_CHEAP, TIER_PRIVILEGE_DROP],
            "§2.12.3/G64: the record's tier vocabulary IS the code's, lowest first — the ratchet's order"
        );
        for platform in PLATFORMS {
            let row = section(&format!("platform.{platform}"));
            let tier = scalar(&row, "tier").expect("every platform row carries a `tier`");
            let legs = array(&row, "legs").expect("every platform row carries a `legs` array");
            let sources = section(&format!("platform.{platform}.verdict_source"));
            assert!(
                tier_order.contains(&tier),
                "§2.12.3/G64: [platform.{platform}].tier {tier:?} is outside the recorded tier vocabulary"
            );
            assert_eq!(
                tier == TIER_PRIVILEGE_DROP,
                !legs.is_empty(),
                "§2.12.3/G64: [platform.{platform}] reaches the privilege-drop tier EXACTLY when it \
                 attaches at least one leg — a tier claimed without a leg, or a leg without the tier, \
                 makes the record unreadable"
            );
            // Every leg names a source from the vocabulary, and NOTHING else does: a leg-less platform
            // carries no verdict-source table at all, and a source for a leg the platform does not attach
            // is an orphan the next reader would trust.
            let named: Vec<String> = sources
                .lines()
                .filter_map(|line| line.split_once('='))
                .map(|(key, _)| key.trim().to_owned())
                .collect();
            assert_eq!(
                named, legs,
                "§2.12.3/G64: [platform.{platform}.verdict_source] names EXACTLY this platform's legs, in \
                 the same order"
            );
            for leg in &legs {
                let source = scalar(&sources, leg)
                    .unwrap_or_else(|| format!("<no verdict_source recorded for `{leg}`>"));
                assert!(
                    verdict_sources.contains(&source),
                    "§2.12.3/G64: [platform.{platform}.verdict_source].{leg} = {source:?} is outside the \
                     recorded verdict-source vocabulary"
                );
            }
        }
    }

    // The forward note P4.17 left for this box, made mechanical: the Windows tier has TWO legs that
    // degrade INDEPENDENTLY (a FAT/exFAT or SMB destination fails the label-then-lower grant while the Job
    // Object still attaches), so the record must never collapse them — a collapsed row would report a
    // write confinement that never applied.
    #[test]
    fn the_windows_row_keeps_its_two_legs_separate() {
        let legs = array(&section("platform.windows"), "legs").expect("the Windows row's legs");
        assert_eq!(
            legs,
            ["integrity", "job"],
            "§2.12.3 (the P4.17 forward note): the Windows record names Leg A (intermediate-integrity \
             write confinement) and Leg B (the own kill-on-job-close Job Object) SEPARATELY"
        );
    }

    // macOS is the cheap-tier floor by DECISION (P4.16), not by degradation — so an empty leg list is the
    // honest record, and a commit adding a leg row here would be a spec change, never a ratchet raise.
    #[test]
    fn the_macos_row_records_the_decided_cheap_floor() {
        let row = section("platform.macos");
        assert_eq!(
            scalar(&row, "tier").expect("the macOS row's tier"),
            TIER_CHEAP,
            "§2.12.3 [DECIDED — P4.16]: v1-portable macOS runs the cheap-tier floor"
        );
        assert!(
            array(&row, "legs")
                .expect("the macOS row's legs")
                .is_empty(),
            "§2.12.3 [DECIDED — P4.16]: no Seatbelt profile is applied, so macOS attaches no leg"
        );
    }
}

/// Read `path`'s mandatory-label SDDL for the tier tests — the crate-internal window onto
/// [`read_label_sddl`], so a test in `crate::isolation` can assert the LEVEL a sink actually carries
/// rather than inferring it from a denial (an unlabelled sink denies a Low writer just as well, by
/// implicit Medium). Test-only.
#[cfg(windows)]
#[cfg(test)]
pub(crate) fn read_label_sddl_for_test(path: &Path) -> Option<String> {
    read_label_sddl(path)
}

/// Label `path` at the mandatory integrity level `rid`, `(OI)(CI)`-inheritably when `inheritable`
/// (§2.12.3 Leg A, P4.17). Test-only: production always labels at [`CONFINED_INTEGRITY_RID`] through
/// [`label_confinement_sinks`], but the tier tests need a sink at a DIFFERENT level — a co-tenant
/// standing in for a Low sandbox can only report into a sink at its own level, which is what makes
/// its denial against the 6144 `.part` non-vacuous.
#[cfg(windows)]
#[cfg(test)]
pub(crate) fn label_at_level(path: &Path, rid: u32, inheritable: bool) -> bool {
    let sddl = if inheritable {
        label_sddl_dir(rid)
    } else {
        label_sddl_file(rid)
    };
    set_label_sddl(path, &sddl)
}

/// Read one job's `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` back (§2.12.3 Leg B, P4.17) — the
/// grant-IS-the-enforcement read-back for the caps, mirroring the Linux legs' probe reporters: the tier tests
/// assert what the KERNEL holds, never what [`set_job_limits`] was asked to write. Test-only: production never
/// re-reads its own limits (it re-writes them on stand-down instead).
#[cfg(windows)]
#[cfg(test)]
fn query_job_limits(
    job: std::os::windows::io::RawHandle,
) -> Option<windows_sys::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION> {
    use windows_sys::Win32::System::JobObjects::{
        JobObjectExtendedLimitInformation, QueryInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    };
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    let size = u32::try_from(std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).ok()?;
    let mut returned: u32 = 0;
    // SAFETY: `job` is a live job handle owned by the caller; `info` is a correctly-typed `repr(C)` buffer
    // whose own `size_of` is passed as the length, and `returned` is a valid out-param.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
    let ok = unsafe {
        QueryInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&raw mut info).cast(),
            size,
            &raw mut returned,
        )
    };
    (ok != 0).then_some(info)
}

// §2.12.3 best-effort WINDOWS privilege-drop tier (P4.17) — per-leg tests. Windows-only + REAL objects: a
// mandatory label and a Job Object are only observable through the real kernel (grant-IS-enforcement), never a
// mock (test-strategy §0.1). TWO STACKED cfg attrs (`#[cfg(test)]` then `#[cfg(windows)]`) — NOT a compound
// `all(test, windows)` (the P1.17 clippy `is_cfg_test` trap). The cross-module ENFORCEMENT half — a lowered
// child writing its own labelled sink and being denied a Medium one — lives in `crate::isolation`, which owns
// the spawn composition these legs attach to.
#[cfg(test)]
#[cfg(windows)]
mod windows_privilege_drop_tests {
    use super::{
        attach_confined_job, attach_job_with, label_ace_rid, label_blocks_lowered_access,
        label_confinement_sinks, label_sddl_dir, label_sddl_file, query_job_limits,
        read_label_sddl, set_job_limits, set_label_sddl, strip_mandatory_label,
        volume_persists_acls, ConfinedJob, JobLimits, LabelStrip, CONFINED_INTEGRITY_RID,
        LABEL_SDDL_NONE, PRODUCTION_JOB_LIMITS,
    };
    use std::os::windows::io::AsRawHandle;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::System::JobObjects::{
        JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION,
        JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    // The SID string the constants are built from, derived from the RID rather than repeated, so the
    // duplication between `CONFINED_INTEGRITY_RID` and the SDDL literals cannot silently drift.
    fn confined_sid() -> String {
        format!("S-1-16-{CONFINED_INTEGRITY_RID}")
    }

    // An absolute System32 executable — never a PATH lookup (the confined child runs env-cleared, and a CI
    // runner's PATH is not ours to rely on).
    fn system32(exe: &str) -> PathBuf {
        let root = std::env::var_os("SystemRoot").expect("SystemRoot is set on Windows");
        Path::new(&root).join("System32").join(exe)
    }

    // A real, long-lived child to attach a job to. `PING.EXE -n <n> 127.0.0.1` is the System32 sleep that
    // needs no shell, no PATH and no console input.
    fn long_lived_child() -> std::process::Child {
        // A TEST-ONLY spawn to OBSERVE the P4.17 Leg-B job semantics — NOT a production engine spawn (those
        // route through crate::isolation::run_confined, the G29-sanctioned site). It keeps the inherited env
        // deliberately: this child observes nothing about the env, and env_clear would add no signal. The
        // IMPORTED `Command::new` form is the P4.15 `confined_sh` precedent (the G9 invariant-(b) grep is
        // scoped to the qualified spelling; the import-resolving SAST job is the real net, suppressed here).
        // nosemgrep: convertia-command-outside-isolation, convertia-command-missing-env-clear
        Command::new(system32("PING.EXE"))
            .args(["-n", "60", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn the System32 ping child")
    }

    // Poll a child for up to `bound` for its exit — the deterministic replacement for a fixed sleep: it
    // returns as soon as the state is observed, and the generous bound absorbs a loaded CI runner.
    fn exited_within(child: &mut std::process::Child, bound: Duration) -> bool {
        status_within(child, bound).is_some()
    }

    // The same poll, keeping the STATUS — a cap-breach regression has to tell "the child finished its work"
    // apart from "the child was stopped", and a bare did-it-exit answer cannot.
    // [Build-Session-Entscheidung: P4.18.2]
    fn status_within(
        child: &mut std::process::Child,
        bound: Duration,
    ) -> Option<std::process::ExitStatus> {
        let deadline = Instant::now() + bound;
        while Instant::now() < deadline {
            if let Ok(Some(status)) = child.try_wait() {
                return Some(status);
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        None
    }

    // §6.4.1 unit (G15) / §2.12.3 Leg A: the SDDL literals and the RID constant are ONE decision — a drift
    // between them would silently label at a different level than the tier reasons about.
    #[test]
    fn the_label_constants_all_name_the_confined_integrity_level() {
        let sid = confined_sid();
        assert_eq!(
            sid, "S-1-16-6144",
            "the P4.17 ruling fixes the level at 0x1800"
        );
        let file_ace = label_sddl_file(CONFINED_INTEGRITY_RID);
        let dir_ace = label_sddl_dir(CONFINED_INTEGRITY_RID);
        assert!(
            file_ace.contains(&sid) && dir_ace.contains(&sid),
            "both label ACEs must name {sid}: file={file_ace} dir={dir_ace}"
        );
        assert_eq!(
            label_ace_rid(&file_ace),
            Some(CONFINED_INTEGRITY_RID),
            "the ACE's RID must parse back to the level the tier reasons about: {file_ace}"
        );
        assert!(
            dir_ace.contains("OICI"),
            "the directory ACE must be object+container inheritable so the engine's own subtree inherits it"
        );
        assert!(
            !LABEL_SDDL_NONE.contains("ML"),
            "the strip must be an EMPTY SACL, never a re-label: {LABEL_SDDL_NONE}"
        );
    }

    // §6.4.1 unit (G15) / §2.12.3 Leg A: the label round-trips VERBATIM at the intermediate level (Windows
    // keeps the exact RID rather than snapping it to a well-known one) and the strip removes it — the two
    // halves the §2.1.2 publish depends on. Real FS, real kernel (test-strategy §0.1).
    #[test]
    fn the_intermediate_label_round_trips_and_the_strip_removes_it() {
        let dir = tempfile::tempdir().expect("a real temp dir");
        let file = dir.path().join("out.part");
        std::fs::write(&file, b"bytes").expect("write the publish-temp stand-in");
        assert_eq!(
            strip_mandatory_label(&file),
            LabelStrip::Absent,
            "an unlabelled temp needs no strip — the every-publish fast path"
        );
        assert!(
            set_label_sddl(&file, &label_sddl_file(CONFINED_INTEGRITY_RID)),
            "labelling a file we own needs no privilege and no elevation"
        );
        let labelled =
            read_label_sddl(&file).expect("the label reads back without SeSecurityPrivilege");
        assert!(
            labelled.contains(&confined_sid()),
            "Windows must keep the exact intermediate RID: {labelled}"
        );
        assert_eq!(
            strip_mandatory_label(&file),
            LabelStrip::Stripped,
            "the strip must remove the label AND read the removal back"
        );
        let stripped = read_label_sddl(&file).unwrap_or_default();
        assert!(
            !stripped.contains("(ML;"),
            "after the strip no label ACE may remain, so `final` carries the destination's implicit level: {stripped}"
        );
    }

    // §6.4.1 unit (G15) / §2.12.3 Leg A: the `(OI)(CI)` scratch ACE PROPAGATES to files the engine creates
    // inside the per-run scratch — the LibreOffice `--outdir` / profile / TMP subtree case, which is why the
    // directory sink is labelled inheritably rather than once.
    #[test]
    fn a_labelled_scratch_propagates_the_level_to_engine_created_files() {
        let scratch = tempfile::tempdir().expect("a real scratch dir");
        assert!(
            set_label_sddl(scratch.path(), &label_sddl_dir(CONFINED_INTEGRITY_RID)),
            "labelling the per-run scratch dir"
        );
        let inherited = scratch.path().join("engine-working-file.tmp");
        std::fs::write(&inherited, b"engine bytes")
            .expect("create a file inside the labelled scratch");
        let sddl = read_label_sddl(&inherited).expect("read the inherited label");
        assert!(
            sddl.contains(&confined_sid()),
            "a file created inside the labelled scratch must inherit the level: {sddl}"
        );
    }

    // §6.4.1 unit (G15) / §2.12.3 Leg A: the grant covers BOTH sinks the §2.14.1 placement chose, and a sink
    // that cannot be labelled DEGRADES the whole grant (never-break: the token is then not lowered, so the
    // engine keeps full write access to a sink whose label was dropped).
    #[test]
    fn the_sink_grant_covers_both_sinks_and_degrades_when_one_cannot_be_labelled() {
        let scratch = tempfile::tempdir().expect("a real scratch dir");
        let dest = tempfile::tempdir().expect("a real destination dir");
        let part = dest.path().join("item.part");
        std::fs::write(&part, b"bytes").expect("create the publish temp");
        let program = system32("cmd.exe");
        assert!(
            label_confinement_sinks(scratch.path(), Some(&part), &program),
            "both sinks are on a real NTFS temp volume and are ours to label"
        );
        for sink in [scratch.path(), part.as_path()] {
            let sddl = read_label_sddl(sink).unwrap_or_default();
            assert!(
                sddl.contains(&confined_sid()),
                "every granted sink must carry the level ({}): {sddl}",
                sink.display()
            );
        }
        assert!(
            !label_confinement_sinks(
                scratch.path(),
                Some(&dest.path().join("absent.part")),
                &program
            ),
            "a sink that cannot be labelled degrades the grant to the cheap tier"
        );
    }

    // §6.4.1 unit (G15) / §2.12.3 Leg A: the per-sink volume test is NOT vacuous — a real local NTFS temp
    // dir must report persistent ACLs, or every grant above would degrade for the wrong reason.
    #[test]
    fn a_local_temp_volume_reports_persistent_acls() {
        let dir = tempfile::tempdir().expect("a real temp dir");
        assert!(
            volume_persists_acls(dir.path()),
            "the local NTFS temp volume must persist ACLs, else the tier degrades vacuously everywhere"
        );
    }

    // §6.4.1 unit (G15) / §2.12.3 Leg A: an UNREADABLE label state is never mistaken for "no label". The
    // strip is what keeps ConvertIA's private level off `final` (§2.1.1 step-2 Windows tail), so a read it
    // cannot perform must route to the fallback, not to the silent fast path — the same never-assume bias
    // `label_blocks_lowered_access` carries at the other end of the leg.
    #[test]
    fn an_unreadable_label_state_routes_to_the_fallback_not_to_absent() {
        let dir = tempfile::tempdir().expect("a real temp dir");
        assert_eq!(
            strip_mandatory_label(&dir.path().join("vanished.part")),
            LabelStrip::Failed,
            "a label state that cannot be read is not evidence that there is none"
        );
    }

    // §6.4.1 unit (G15) / §2.12.3 Leg A: the read-blocking test reasons about the LEVEL, not merely about the
    // presence of an `NR`/`NX` flag. A `NO_READ_UP` ACE BELOW the confined level cannot block our child (that
    // is a read DOWN), so degrading the tier for it would be a needless loss; one AT or ABOVE it genuinely
    // would, so the tier must step aside.
    #[test]
    fn a_read_blocking_label_only_counts_at_or_above_the_confined_level() {
        let dir = tempfile::tempdir().expect("a real temp dir");
        let below = dir.path().join("low-nr.bin");
        std::fs::write(&below, b"x").expect("create the low-labelled file");
        assert!(
            set_label_sddl(&below, "S:(ML;;NRNW;;;S-1-16-4096)"),
            "labelling a file we own at Low"
        );
        assert_eq!(
            label_ace_rid(&read_label_sddl(&below).unwrap_or_default()),
            Some(0x1000),
            "the Low ACE must resolve to its level — Windows reads a well-known level back as its SDDL ALIAS"
        );
        assert!(
            !label_blocks_lowered_access(&below, CONFINED_INTEGRITY_RID),
            "a Low `NO_READ_UP` ACE cannot block a {CONFINED_INTEGRITY_RID}-level reader — that is a read DOWN"
        );
        let at_level = dir.path().join("medium-nr.bin");
        std::fs::write(&at_level, b"x").expect("create the medium-labelled file");
        assert!(
            set_label_sddl(&at_level, "S:(ML;;NRNW;;;S-1-16-8192)"),
            "labelling a file we own at Medium"
        );
        assert!(
            label_blocks_lowered_access(&at_level, CONFINED_INTEGRITY_RID),
            "a Medium `NO_READ_UP` ACE DOES block a lowered child — the tier must degrade rather than break it"
        );
    }

    // §6.4.1 unit (G15) / §2.12.3 Leg B: the own job is created with the caps AND kill-on-job-close ARMED —
    // read back from the kernel, not assumed from the write.
    #[test]
    fn the_own_job_arms_the_caps_and_kill_on_job_close() {
        let mut child = long_lived_child();
        let job = attach_confined_job(child.id()).expect("assign our own job to the child");
        let info = query_job_limits(job.job.as_raw_handle()).expect("read the job limits back");
        let flags = info.BasicLimitInformation.LimitFlags;
        for (flag, name) in [
            (JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, "KILL_ON_JOB_CLOSE"),
            (JOB_OBJECT_LIMIT_JOB_MEMORY, "JOB_MEMORY"),
            (JOB_OBJECT_LIMIT_ACTIVE_PROCESS, "ACTIVE_PROCESS"),
            (
                JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION,
                "DIE_ON_UNHANDLED_EXCEPTION",
            ),
        ] {
            assert!(
                flags & flag != 0,
                "§2.12.3 Leg B must arm {name}: flags={flags:#x}"
            );
        }
        assert_eq!(
            u64::try_from(info.JobMemoryLimit).unwrap_or(0),
            PRODUCTION_JOB_LIMITS.memory_bytes,
            "the committed-memory runaway cap must be the production value"
        );
        assert_eq!(
            info.BasicLimitInformation.ActiveProcessLimit, PRODUCTION_JOB_LIMITS.active_processes,
            "the fork-bomb cap must be the production value"
        );
        drop(job);
        child.kill().ok();
        child.wait().ok();
    }

    // §6.4.1 unit (G15) / §2.12.3 Leg B: the stand-down clears ONLY kill-on-job-close — the resource caps
    // survive it, so a stood-down job is still a runaway guard for whatever of the tree outlives the launcher.
    #[test]
    fn the_stand_down_clears_only_kill_on_job_close() {
        let mut child = long_lived_child();
        let job = attach_confined_job(child.id()).expect("assign our own job to the child");
        let handle = job.job.as_raw_handle();
        assert!(
            set_job_limits(handle, PRODUCTION_JOB_LIMITS, false),
            "re-writing the limits without kill-on-close is what stand_down does"
        );
        let info = query_job_limits(handle).expect("read the job limits back");
        let flags = info.BasicLimitInformation.LimitFlags;
        assert!(
            flags & JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE == 0,
            "the clean-exit stand-down must drop kill-on-job-close: flags={flags:#x}"
        );
        for (flag, name) in [
            (JOB_OBJECT_LIMIT_JOB_MEMORY, "JOB_MEMORY"),
            (JOB_OBJECT_LIMIT_ACTIVE_PROCESS, "ACTIVE_PROCESS"),
            (
                JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION,
                "DIE_ON_UNHANDLED_EXCEPTION",
            ),
        ] {
            assert!(
                flags & flag != 0,
                "the stand-down must KEEP {name}: flags={flags:#x}"
            );
        }
        drop(job);
        child.kill().ok();
        child.wait().ok();
    }

    // §6.4.1 unit (G15) / §2.12.3 Leg B — the ENFORCEMENT half (the P4.10 crash-time-reap residual this box
    // closes): dropping an ARMED job reaps the child, which is exactly what the OS does for us when ConvertIA
    // itself dies. The armed arm is every non-clean exit.
    #[test]
    fn dropping_an_armed_job_reaps_the_child() {
        let mut child = long_lived_child();
        let job = attach_confined_job(child.id()).expect("assign our own job to the child");
        assert!(
            !exited_within(&mut child, Duration::from_millis(200)),
            "non-vacuity: the child must still be alive while the job handle is held"
        );
        drop(job);
        assert!(
            exited_within(&mut child, Duration::from_secs(10)),
            "closing the last handle of a kill-on-job-close job must reap the tree"
        );
        child.wait().ok();
    }

    // §6.4.1 unit (G15) / §2.12.3 Leg B — the other half of the same decision: a STOOD-DOWN job leaves the
    // tree alone, so a launcher that legitimately exits before its worker finished writing is never truncated
    // (the P4.12 clean-exit rationale, mirrored).
    #[test]
    fn a_stood_down_job_leaves_the_child_running() {
        let mut child = long_lived_child();
        let job = attach_confined_job(child.id()).expect("assign our own job to the child");
        job.stand_down();
        assert!(
            !exited_within(&mut child, Duration::from_secs(2)),
            "a stood-down job must NOT reap the tree on close — that would truncate valid in-flight output"
        );
        child.kill().ok();
        child.wait().ok();
    }

    // §6.4.1 unit (G15) / §2.12.3 Leg B: our job NESTS rather than colliding — the load-bearing assumption of
    // attaching in `post_spawn` while `process_wrap` assigns its own job in `wrap_child`. A second assignment
    // succeeding IS the Windows-8+ nested-job behaviour; if it ever failed, the spawn itself would fail and
    // the P4.10 group-kill contract would break, so this pins it directly rather than by inference.
    #[test]
    fn a_second_job_assignment_nests_rather_than_failing() {
        let mut child = long_lived_child();
        let ours: ConfinedJob = attach_confined_job(child.id()).expect("our job assigns first");
        let theirs: ConfinedJob = attach_job_with(
            child.id(),
            JobLimits {
                memory_bytes: PRODUCTION_JOB_LIMITS.memory_bytes,
                active_processes: PRODUCTION_JOB_LIMITS.active_processes,
            },
        )
        .expect(
            "a SECOND job assignment must nest (Windows 8+), as process-wrap's does after ours",
        );
        ours.stand_down();
        theirs.stand_down();
        child.kill().ok();
        child.wait().ok();
    }

    // ─── P4.18.2 / P4.18.3: the §2.12.3 Leg-B cap + reap REGRESSIONS (the P0.5.9 homes) ──────────────
    //
    // `the_own_job_arms_the_caps_and_kill_on_job_close` above proves the caps are SET — read back from the
    // kernel rather than assumed from the write. What it cannot prove is that a cap actually BITES, or that
    // the reap reaches past the immediate child. Those are the two P0.5.9 regressions this box instantiates,
    // and they are deliberately separate tests: they exercise different OS subsystems (job memory accounting
    // vs job teardown) and fail independently. Both drive the PRODUCTION `set_job_limits` / `attach_job_with`
    // path — only the cap VALUE is a test value, which is exactly what `JobLimits` carries limits as data for.

    // A child that COMMITS a large allocation shortly after start, so a job attached in between decides
    // whether it can finish. PowerShell is the only System32 tool that allocates on demand without an input
    // file; `-NoProfile -NonInteractive` keeps it deterministic and `-Command` is not gated by the machine's
    // script execution policy. [Build-Session-Entscheidung: P4.18.2]
    fn allocating_child(megabytes: u32) -> std::process::Child {
        let shell =
            Path::new(&std::env::var_os("SystemRoot").expect("SystemRoot is set on Windows"))
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe");
        // A TEST-ONLY spawn OBSERVING the P4.17 Leg-B cap semantics — not a production engine spawn (those
        // route through crate::isolation::run_confined, the G29-sanctioned site). The imported `Command::new`
        // form is the P4.15 `confined_sh` / P4.17 `long_lived_child` precedent in this module.
        // nosemgrep: convertia-command-outside-isolation, convertia-command-missing-env-clear
        Command::new(shell)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                // The sleep is the attach window: the job must be assigned BEFORE the allocation, or the
                // test would measure nothing. `AllocHGlobal` rather than a managed `byte[]`: the CLR
                // RESERVES a large-object segment and commits it lazily as pages are touched, so a managed
                // array of this size charges the job almost nothing until it is written page by page
                // (measured — a 768 MB `[byte[]]` sailed through a 128 MiB cap). `AllocHGlobal` commits the
                // whole block up front, which is what `JOB_OBJECT_LIMIT_JOB_MEMORY` accounts.
                // [Build-Session-Entscheidung: P4.18.2]
                &format!(
                    "Start-Sleep -Milliseconds 400; \
                     $p = [System.Runtime.InteropServices.Marshal]::AllocHGlobal({megabytes}MB); \
                     [System.Runtime.InteropServices.Marshal]::WriteByte($p, 0, 1); \
                     [System.Runtime.InteropServices.Marshal]::FreeHGlobal($p); \
                     exit 0"
                ),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn the System32 PowerShell allocating child")
    }

    // §6.4.2 fault-injection (G16/G31) / §2.12.3 Leg B (P4.18.2): the memory cap is ARMED, not merely set —
    // a child that breaches it CANNOT run on. Asserted as a red-green PAIR so it can never pass vacuously:
    // the very same child under the PRODUCTION cap completes, and under a deliberately tiny cap it does not.
    // Without the control leg a child that died for an unrelated reason would look like a firing cap.
    //
    // This is the §2.12.3 memory-cap arm of the P0.5.9 isolation/privilege-drop home. It is Windows-only by
    // construction: the Job-Object `JOB_OBJECT_LIMIT_JOB_MEMORY` is the ONLY §2.12.3 memory cap in the
    // product — the Linux and macOS tiers carry none, so there is nothing to regress there (their per-item
    // memory kill is the §1.10/§1.7 preflight path, a different control with its own home).
    #[test]
    fn the_job_memory_cap_is_armed_and_stops_a_breaching_engine() {
        const ALLOCATE_MB: u32 = 768;
        // Generously below the allocation and generously above a PowerShell start-up, so the breach is what
        // the tiny-cap leg measures. Wherever the runtime happens to fail first, the assertion holds: under
        // this cap the child CANNOT complete a 768 MB commit.
        const TINY_CAP_BYTES: u64 = 128 << 20;

        let mut capped = allocating_child(ALLOCATE_MB);
        let capped_job = attach_job_with(
            capped.id(),
            JobLimits {
                memory_bytes: TINY_CAP_BYTES,
                active_processes: PRODUCTION_JOB_LIMITS.active_processes,
            },
        )
        .expect("assign a tiny-memory-cap job to the allocating child");
        let capped_status = status_within(&mut capped, Duration::from_secs(60));
        drop(capped_job);
        capped.kill().ok();
        capped.wait().ok();

        let mut uncapped = allocating_child(ALLOCATE_MB);
        let uncapped_job = attach_confined_job(uncapped.id())
            .expect("assign the PRODUCTION-cap job to the same allocating child");
        let uncapped_status = status_within(&mut uncapped, Duration::from_secs(60));
        drop(uncapped_job);
        uncapped.kill().ok();
        uncapped.wait().ok();

        // The CONTROL leg first: it is what makes the capped leg mean anything. If the allocating child
        // could not complete even UNCAPPED, the capped leg would be measuring a broken environment rather
        // than a firing cap — the exact silent vacuity this red-green pair exists to rule out.
        assert!(
            uncapped_status.is_some_and(|status| status.success()),
            "control leg: under the production cap ({} GiB) the child COMPLETES its {ALLOCATE_MB} MB \
             allocation and exits 0 — got {uncapped_status:?}",
            PRODUCTION_JOB_LIMITS.memory_bytes >> 30
        );
        let capped_status =
            capped_status.expect("a capped child must not run on past its cap (it never exited)");
        assert!(
            !capped_status.success(),
            "§2.12.3 Leg B: a child breaching its job memory cap must FAIL rather than complete the same \
             {ALLOCATE_MB} MB allocation the control leg just completed — the cap is ARMED, not merely SET \
             (the runaway guard the §1.7 watchdog sits above); got {capped_status:?}"
        );

        // §1.9 batch-continues, at the layer this box owns: a cap breach kills ONE spawn's job and leaves
        // the tier able to confine the next item. Each spawn gets its OWN job, so a killed one must not
        // wedge the mechanism — the property that makes "the offending item is reported Failed while the
        // batch continues" true at the Leg-B layer. (The §2.8 mapping of an abnormally-terminated engine to
        // `Failed` is `crate::isolation`'s
        // `a_clean_exit_maps_to_succeeded_and_a_nonzero_exit_to_engine_crash`, and the many-items-in-one-
        // process property is its `many_concurrent_cheap_tier_spawns_all_complete_under_timeout` — both
        // proven over the real wrapper, so re-asserting them here would only duplicate. The §0.9 permit that
        // the failing item returns is acquired by the subprocess lane the pool-wiring boxes build; the
        // pool's own release-on-failure guarantee is proven by `crate::pool`'s permit tests.)
        // [Build-Session-Entscheidung: P4.18.2]
        let mut next = long_lived_child();
        let next_job = attach_confined_job(next.id()).expect(
            "the tier still confines the NEXT item after a cap breach killed the previous one",
        );
        drop(next_job);
        assert!(
            exited_within(&mut next, Duration::from_secs(30)),
            "§1.9/§2.12.3: the next item's own job is fully functional after the cap breach — kill-on-job-\
             close still reaps it"
        );
        next.kill().ok();
        next.wait().ok();
    }

    // A child that leaves a DETACHED descendant behind and exits at once, so what survives the teardown is
    // the grandchild, never the direct child. The descendant writes an early marker (non-vacuity: it really
    // ran), then APPENDS a heartbeat once a second for far longer than the test's own horizon.
    //
    // A HEARTBEAT rather than a one-shot "an orphan writes `alive.txt` after N seconds" marker, and for the
    // same reason its engines-side sibling uses one: a one-shot marker makes the assertion depend on a
    // wall-clock guess (the test must wait past `descendant_start + N`), and here that guess fails in the
    // WORST direction — a loaded runner that stalls a genuinely-orphaned descendant past the wait window
    // would read as "reaped" and pass SILENTLY. A frozen-length observation cannot: it proves the process is
    // not executing, whenever it happened to start. [Build-Session-Entscheidung: P4.18.3]
    //
    // `ping.exe` is a valid tick sleep HERE and would not be under `run_confined`: this child is never
    // integrity-lowered, whereas a §2.12.3 Leg-A-confined child runs below Medium and is refused the device
    // objects a socket needs, so `ping` would return instantly and the heartbeat would spin at full speed.
    // The engines-side sibling (`the_watchdog_reap_leaves_no_orphaned_descendant`) DOES run confined and
    // uses `waitfor.exe` for exactly that reason; the rationale is cross-referenced here so re-homing this
    // test under a confined spawn cannot re-open the vacuity.
    fn descendant_leaving_child(scratch: &Path) -> std::process::Child {
        std::fs::write(
            scratch.join("descendant.cmd"),
            "@echo off\r\n\
             echo x> started.txt\r\n\
             for /L %%i in (1,1,60) do (\r\n\
             echo x>> ticks.txt\r\n\
             %SystemRoot%\\System32\\ping.exe -n 2 127.0.0.1 > nul\r\n\
             )\r\n",
        )
        .expect("write the descendant script into the scratch dir");
        // A TEST-ONLY spawn OBSERVING the P4.17 Leg-B teardown — see `long_lived_child`.
        // nosemgrep: convertia-command-outside-isolation, convertia-command-missing-env-clear
        let mut command = Command::new(system32("cmd.exe"));
        command
            .args(["/d", "/c"])
            .current_dir(scratch)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        // `cmd.exe` parses `/c`'s tail with its OWN quoting rules, which do not understand the MSVCRT
        // backslash-escaping `Command::arg` applies — the same reason `crate::isolation`'s confined-cmd
        // helper reaches for `raw_arg`. The `.\` prefix is load-bearing: a bare `descendant.cmd` is not
        // resolved from the current directory on a host with `NoDefaultCurrentDirectoryInExePath` set
        // (measured — the inner shell reports "command not found" and the descendant never runs).
        {
            use std::os::windows::process::CommandExt;
            command.raw_arg("start /b cmd /d /c .\\descendant.cmd");
        }
        command
            .spawn()
            .expect("spawn the descendant-leaving cmd child")
    }

    // Poll for a marker, bounded — every teardown assertion is armed off the EARLY marker, never off a
    // wall-clock guess that could fire before the descendant even existed.
    fn appeared_within(marker: &Path, bound: Duration) -> bool {
        let deadline = Instant::now() + bound;
        while Instant::now() < deadline {
            if marker.exists() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        marker.exists()
    }

    // The heartbeat file's length — `0` while it does not exist yet, so a descendant reaped before its
    // first tick reads as "not growing" exactly like one reaped further along.
    fn heartbeat_len(path: &Path) -> u64 {
        std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
    }

    // §6.4.3 integration (G31) / §2.12.3 Leg B + §1.7 (P4.18.3): the kill-on-job-close reap reaches the whole
    // TREE, not just the process we assigned. `dropping_an_armed_job_reaps_the_child` above proves the direct
    // child dies; this proves the case that actually leaks in production — a launcher that exits immediately
    // and leaves a worker behind (the `soffice` → `soffice.bin` class). An orphaned descendant would keep
    // appending to its heartbeat; the reap is what freezes it.
    //
    // This is the process-group / Job-Object REAP arm of the P0.5.9 home. Its cross-platform sibling — the
    // §1.7 watchdog-timeout trigger through the production `run_confined` — lives with the watchdog in
    // `crate::engines`; the cancel trigger is `crate::isolation`'s `a_cancel_group_kills_the_engines_descendants`.
    #[test]
    fn dropping_an_armed_job_reaps_the_childs_descendants_too() {
        let scratch = tempfile::tempdir().expect("a real scratch dir for the descendant markers");
        let ticks = scratch.path().join("ticks.txt");
        let mut child = descendant_leaving_child(scratch.path());
        let job = attach_confined_job(child.id()).expect("assign our own job to the child");
        assert!(
            appeared_within(&scratch.path().join("started.txt"), Duration::from_secs(30)),
            "non-vacuity: the descendant must really have run, or the frozen heartbeat below would prove \
             nothing — it would just be a process that never started"
        );
        // The launcher exits at once; the descendant is what the job still holds.
        child.wait().ok();
        drop(job);
        // A short settle before the snapshot: an append already in flight when the kill lands may still
        // complete, and reading across that would be a one-off FALSE RED. Bounded and one-directional — it
        // can never hide a survivor, only mis-time the baseline.
        std::thread::sleep(Duration::from_millis(250));
        let at_reap = heartbeat_len(&ticks);
        // Several of the descendant's own tick intervals: a survivor appends throughout this window.
        std::thread::sleep(Duration::from_secs(5));
        assert_eq!(
            heartbeat_len(&ticks),
            at_reap,
            "§2.12.3 Leg B / §1.7: closing the kill-on-job-close job must reap the engine's DESCENDANT too \
             — a direct-child-only teardown would have left it appending to its heartbeat throughout this \
             window"
        );
    }
}

// §2.12.3 WINDOWS privilege-drop DECISION pin (P4.17, Co-Pilot ruling 2026-08-25 — the
// `macos_seatbelt_decision_tests` sibling). The restricted-token / AppContainer leg and the AppContainer /
// WFP network-deny leg are DECIDED unrealizable in the v1-portable build (stable `CommandExt` carries no
// spawn-token / process-creation-attribute path, `tokio::process::Child` cannot be built from a raw handle,
// an AppContainer additionally needs `ALL APPLICATION PACKAGES` DACL grants on the portable bundle dir and on
// every input, and a WFP/firewall rule needs elevation plus a persistent machine-global mutation), so NO FFI
// for them may enter the core — no dead code for a decided-not-applied mechanism (CLAUDE §5). This
// CROSS-PLATFORM source-scan runs on ALL THREE CI legs and walks the two directories that could home such a
// call RECURSIVELY, so a future submodule is covered automatically (the g24 target-absent-leg lesson).
// [Build-Session-Entscheidung: P4.17]
#[cfg(test)]
mod windows_tier_decision_tests {
    // Everything before a scanned file's FIRST `#[cfg(test)]`, so a needle can never match a test's own
    // source (this module names the forbidden tokens in its assertions). `concat!`-split so the literal
    // marker is absent from this scanning module too (the `macos_seatbelt_decision_tests` precedent).
    fn production_prefix(full: &str) -> &str {
        full.split_once(concat!("#[cfg", "(test)]"))
            .map_or(full, |(prefix, _)| prefix)
    }

    // The spawn-token / AppContainer / WFP API families — API IDENTIFIERS, never the English words, so the
    // decision PROSE that necessarily names the rejected mechanisms (this file's module doc, the §2.12.3
    // rationale in `crate::isolation`) does not trip its own pin. `CreateProcessAsUser`/`CreateProcessWithToken`
    // are the only ways to spawn with a foreign token; `CreateRestrictedToken` mints one; the AppContainer
    // family creates/derives a profile SID and `SECURITY_CAPABILITIES` is the attribute that would carry it;
    // `Fwpm`/`INetFw` are the WFP and COM-firewall surfaces. Substrings, so the `W`/`A`/`0` suffixed variants
    // are covered.
    const FORBIDDEN_SPAWN_TOKEN_TOKENS: [&str; 8] = [
        "CreateProcessAsUser",
        "CreateProcessWithToken",
        "CreateRestrictedToken",
        "CreateAppContainerProfile",
        "DeriveAppContainerSid",
        "SECURITY_CAPABILITIES",
        "Fwpm",
        "INetFw",
    ];

    // The scanned PREFIX must be non-trivial, per file — the g24 never-silently-watch-nothing lesson applied
    // one level deeper than "the file was read". `production_prefix` truncates at the first `#[cfg(test)]`, so
    // a file whose production half shrank (or that grew production code BELOW its first test module, where the
    // scan cannot see it) would pass vacuously while enforcing nothing. Each needle is a production symbol the
    // scanned file cannot lose without the tier itself changing, and each lives in that file's production half
    // by construction.
    const PREFIX_NEEDLES: [(&str, &str); 2] = [
        ("platform/mod.rs", "SetTokenInformation"),
        ("isolation/mod.rs", "post_spawn"),
    ];

    // §2.12.3 / the P4.17 Co-Pilot ruling: no restricted-token / AppContainer / WFP call or FFI in the core's
    // isolation surface. Walks `src/platform/**` + `src/isolation/**` from the compile-time crate root.
    #[test]
    fn no_appcontainer_or_spawn_token_ffi_in_the_core() {
        use walkdir::WalkDir;
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut scanned = 0usize;
        let (mut saw_platform_root, mut saw_isolation_root) = (false, false);
        for dir in ["platform", "isolation"] {
            for entry in WalkDir::new(src.join(dir)) {
                let entry = entry.expect("walk the core platform/isolation source tree");
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                    continue;
                }
                let full = std::fs::read_to_string(path).expect("read a core source file");
                let prod = production_prefix(&full);
                for token in FORBIDDEN_SPAWN_TOKEN_TOKENS {
                    assert!(
                        !prod.contains(token),
                        "§2.12.3 P4.17 decision (v1-portable Windows = intermediate-IL write confinement + an \
                         own Job Object): a `{token}` call/FFI reappeared in {}'s production source. \
                         Restricted-token / AppContainer and the AppContainer/WFP net-deny are DECIDED \
                         unrealizable here — an installer-build epoch plus a brokered/staged input model is the \
                         revisit anchor. Revisit the Co-Pilot ruling (2026-08-25) + spec §2.12.3 FIRST.",
                        path.display()
                    );
                }
                scanned += 1;
                saw_platform_root |= path.ends_with("platform/mod.rs");
                saw_isolation_root |= path.ends_with("isolation/mod.rs");
                // Hermetic guard, per file: the scanned PREFIX must still contain that file's production
                // needle. "The file was read" is not the same as "a production half was scanned" — a
                // truncated-to-nothing prefix would enforce nothing while passing.
                for (suffix, needle) in PREFIX_NEEDLES {
                    if path.ends_with(suffix) {
                        assert!(
                            prod.contains(needle),
                            "the scanned production prefix of {} no longer contains `{needle}` — the \
                             `#[cfg(test)]` truncation is scanning (nearly) nothing, so the P4.17 decision \
                             pin would pass vacuously",
                            path.display()
                        );
                    }
                }
            }
        }
        // Hermetic guard (the g24 lesson — never silently watch nothing): the scan MUST have read the two
        // known homes, else FAIL loudly rather than pass vacuously.
        assert!(
            saw_platform_root && saw_isolation_root && scanned >= 2,
            "the source-scan must cover crate::platform + crate::isolation (found platform={saw_platform_root}, \
             isolation={saw_isolation_root}, {scanned} .rs files) — the P4.17 decision is not being enforced"
        );
    }
}
