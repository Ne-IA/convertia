//! `crate::ipc::planning` — the §0.4.1 pre-convert planning command group (C2b / C3 / C4 / C5): the
//! target offer, the "will save to…" output plan, the destination picker, and the destination change +
//! re-validation (the §5.2 state-4 flow). P2.21 registers these as the §0.4.1 command-surface interface
//! shells; each command's full request/response contract + its `crate::orchestrator` delegation is authored
//! by its named fill-box. Thin by design (§0.7): validate, delegate, map onto the §0.4.3 `IpcError`.

// §0.4 / T10: unchecked arithmetic on an untrusted wire field must be a compile error in every IPC handler
// (the `crate::ipc` arithmetic-overflow deny cascades here; restated at the T10 boundary). The §1.10
// preflight estimates these handlers will carry are exactly the `width*height*bpp`-class arithmetic the
// deny guards. The shells below do no arithmetic; the deny bites the fill-bodies.
#![deny(clippy::arithmetic_side_effects)]

/// **C2b `pick_destination`** (§0.4.1) — the Rust-side `DialogExt` destination-folder picker; the one chosen
/// `PathBuf` it returns is a *write* destination that legitimately transits the WebView into C5 (§0.10 /
/// §2.1). Registered as the §0.4.1 interface shell (P2.21); the full `{} -> Option<PathBuf>` contract is
/// authored by P2.24. [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn pick_destination() {}

/// **C3 `get_targets`** (§0.4.1) — a pure function of the detected source type to the offered targets + the
/// one pre-highlighted default (§1.5); no engine spawned. Registered as the §0.4.1 interface shell (P2.21);
/// the full `{ collectedSetId } -> TargetOffer` contract is authored by P2.25. [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn get_targets() {}

/// **C4 `plan_output`** (§0.4.1) — computes the §1.8 output plan (resolved destination, divert preview,
/// §2.5 re-run, §1.10 preflight) that drives the "will save to…" line before convert. Registered as the
/// §0.4.1 interface shell (P2.21); the full
/// `{ collectedSetId, target, options, destination } -> OutputPlanPreview` contract is authored by P2.26.
/// [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn plan_output() {}

/// **C5 `set_destination`** (§0.4.1) — re-validates writability/divert and re-evaluates the
/// destination-dependent §2.14.4 preflight when the user changes the destination, carrying the §2.5 re-run
/// verdict through unchanged (§2.5.1). Registered as the §0.4.1 interface shell (P2.21); the full
/// `{ collectedSetId, target, options, destination } -> DestinationResolved` contract is authored by P2.27.
/// [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn set_destination() {}
