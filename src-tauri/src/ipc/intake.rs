//! `crate::ipc::intake` — the §0.4.1 intake command group (C1 / C2a / C13): the single §2.4 freeze point
//! for every intake origin (drop / picker / launch-arg) and the ingest-scoped cancel. P2.21 registers
//! these as the §0.4.1 command-surface interface shells; each command's full request/response contract +
//! its `crate::orchestrator` delegation is authored by its named fill-box. Thin by design (§0.7): the
//! handler validates, delegates, and maps the `Result` onto the §0.4.3 `IpcError`.

// §0.4 / T10: unchecked arithmetic on an untrusted wire field must be a compile error in every IPC handler
// (the `crate::ipc` arithmetic-overflow deny cascades here; restated at the T10 boundary so this group's
// real handlers carry it explicitly). The shells below do no arithmetic; the deny bites the fill-bodies.
#![deny(clippy::arithmetic_side_effects)]

/// **C1 `ingest_paths`** (§0.4.1) — the single §2.4 freeze point for every intake origin. Registered here
/// as the §0.4.1 command-surface interface shell (P2.21); the full
/// `{ paths, origin, collectingId, drainPending?, onScan? } -> CollectedSet` contract + the orchestrator
/// freeze delegation are authored by P2.22. [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn ingest_paths() {}

/// **C2a `pick_for_intake`** (§0.4.1) — the Rust-side `DialogExt` intake picker that funnels straight into
/// the C1 freeze, so no raw FS path ever reaches the WebView (§0.10 / §5.4). Registered as the §0.4.1
/// interface shell (P2.21); the full `{ kind, collectingId, onScan? } -> CollectedSet` contract is authored
/// by P2.23. [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn pick_for_intake() {}

/// **C13 `cancel_ingest`** (§0.4.1) — trips the ingest-scoped `CollectingId` token to cancel an in-flight
/// C1/C2a walk before its long await resolves (§1.1). Registered as the §0.4.1 interface shell (P2.21); the
/// full `{ collectingId } -> ()` contract is authored by P2.35. [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn cancel_ingest() {}
