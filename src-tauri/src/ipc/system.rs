//! `crate::ipc::system` — the §0.4.1 OS shell-out + app-info command group (C9 / C10 / C11 / C12): reveal /
//! open an output path, open the canonical project page, surface app info, and report engine health. P2.21
//! registers these as the §0.4.1 command-surface interface shells; each command's full request/response
//! contract + its `crate::orchestrator`/`OpenerExt` delegation is authored by its named fill-box. Thin by
//! design (§0.7): validate, delegate, map onto the §0.4.3 `IpcError`. No `opener:*` WebView grant exists —
//! every shell-out is Rust-side via `OpenerExt` (§0.10).

// §0.4 / T10: unchecked arithmetic on an untrusted wire field must be a compile error in every IPC handler
// (the `crate::ipc` arithmetic-overflow deny cascades here; restated at the T10 boundary). The shells below
// do no arithmetic; the deny bites the fill-bodies.
#![deny(clippy::arithmetic_side_effects)]

/// **C9 `open_path`** (§0.4.1) — the "one-click open/reveal" action; the handler validates `path` against the
/// current `RunResult`'s recorded outputs (§7.7.3) before calling `OpenerExt` internally (no `opener:*`
/// grant, §0.10). Registered as the §0.4.1 interface shell (P2.21); the full `{ kind, path } -> ()` contract
/// is authored by P2.32. [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn open_path() {}

/// **C10 `open_project_page`** (§0.4.1) — the only permitted, user-initiated network action: opens a fixed
/// compiled-in canonical URL constant via `OpenerExt::open_url` (the WebView supplies no URL, §7.6.2 / §7.7.2).
/// Registered as the §0.4.1 interface shell (P2.21); the full `{} -> ()` contract is authored by P2.33.
/// [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn open_project_page() {}

/// **C11 `get_app_info`** (§0.4.1) — version, build id, platform, and the third-party-licenses / NOTICE data
/// for the §5.9 About screen (§7.2.3); no network. Registered as the §0.4.1 interface shell (P2.21); the full
/// `{} -> AppInfo` contract is authored by P2.34. [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn get_app_info() {}

/// **C12 `get_engine_health`** (§0.4.1) — the cached §7.2.3 startup self-check (which bundled engines are
/// present/runnable, which §3.4 patent-gated targets are available), feeding §5.2. The `EngineHealth` type
/// is authored at P2.111 and the cache is populated by the P4 probe. Registered as the §0.4.1 interface
/// shell (P2.21); the full `{} -> EngineHealth` contract is wired by P2.113. [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn get_engine_health() {}
