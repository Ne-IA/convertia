//! `crate::platform` — the §0.7 OS-abstraction leaf (depends on no other module): path handling,
//! volume detection (§2.14) and the OS shims (§7.7 reveal-in-folder). The per-OS helpers and the one
//! allow-listed `unsafe` FFI surface (the §2.1/§2.3 `renameat2` / `MoveFileExW` /
//! `GetFileInformationByHandle` primitives + the §0.9 Job-Object kill) are authored by their
//! consuming boxes (P3+).
