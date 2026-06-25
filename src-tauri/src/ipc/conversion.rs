//! `crate::ipc::conversion` — the §0.4.1 run-lifecycle command group (C6 / C7 / C8): start a run, cancel a
//! run, and re-fetch a run summary. P2.21 registers these as the §0.4.1 command-surface interface shells;
//! each command's full request/response contract + its `crate::orchestrator` delegation is authored by its
//! named fill-box. Thin by design (§0.7): validate, delegate, map onto the §0.4.3 `IpcError`.

// §0.4 / T10: unchecked arithmetic on an untrusted wire field must be a compile error in every IPC handler
// (the `crate::ipc` arithmetic-overflow deny cascades here; restated at the T10 boundary). The shells below
// do no arithmetic; the deny bites the fill-bodies.
#![deny(clippy::arithmetic_side_effects)]

/// **C6 `start_conversion`** (§0.4.1) — mints the `RunId`, enqueues the batch (§1.9), and streams
/// `ConversionEvent`s over the handed `onProgress` Channel; returns immediately (the run proceeds async).
/// The `destination` argument is authoritative (§0.4.1). Registered as the §0.4.1 interface shell (P2.21);
/// the full
/// `{ collectedSetId, target, options, destination, rerunDecision, onProgress } -> RunId` contract is
/// authored by P2.29. [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn start_conversion() {}

/// **C7 `cancel_run`** (§0.4.1) — trips the §0.4.4 cancellation token for the run (finished items kept, the
/// in-progress item discarded cleanly, §2.1/§2.6). Registered as the §0.4.1 interface shell (P2.21); the
/// full `{ runId } -> ()` contract is authored by P2.30. [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn cancel_run() {}

/// **C8 `get_run_summary`** (§0.4.1) — the idempotent re-fetch of the retained §1.12 `RunResult` (also
/// delivered as the terminal `RunFinished` event), e.g. after a WebView reload. Registered as the §0.4.1
/// interface shell (P2.21); the full `{ runId } -> RunResult` contract is authored by P2.31.
/// [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn get_run_summary() {}
