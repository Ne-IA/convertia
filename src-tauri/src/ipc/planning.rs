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

use std::path::PathBuf;

use crate::outcome::IpcError;

/// **C2b `pick_destination`** (§0.4.1) — the Rust-side `DialogExt` destination-folder picker. This box (P2.24)
/// authors the typed §0.4.1 wire CONTRACT — the `{} -> Result<Option<PathBuf>, IpcError>` door — so the
/// generated `bindings.ts` carries the C2b surface. Unlike the C2a intake picker, the **one chosen `PathBuf` it
/// returns legitimately transits the WebView** into C5 `set_destination` (and then C6): it is a *write*
/// destination, not a source path, so it can never harm an original or read anything (§0.10 / §2.1 / §0.11 T2).
/// `Ok(None)` = the user cancelled — a clean no-op; the held C4/C5 destination is unchanged.
///
/// [Build-Session-Entscheidung: P2.24] **`Result<Option<PathBuf>, IpcError>` return — the §0.4 universal
/// error-shape rule.** §0.4 "Error shape" is categorical: *every* command returns `Result<T, IpcError>`. The
/// §0.4.1 table's `Option<PathBuf>` output column is the SUCCESS type `T`, wrapped in `Result<T, IpcError>` at
/// the handler — exactly as C1's `CollectedSet` column maps to `Result<CollectedSet, IpcError>`. So the three
/// boundary outcomes are: `Ok(Some(path))` = the user picked a folder; `Ok(None)` = the user cancelled (a clean
/// no-op, the §5.4 cancelled-picker result); `Err(IpcError)` = the native dialog subsystem genuinely failed (a
/// folder pick has no *user-facing* failure, but the boundary still honours the universal Result shape rather
/// than panicking across it, §0.4 "No command ever panics across the boundary"). The wire/TS callsite is
/// unchanged (`Result<T, E>` renders as `__TAURI_INVOKE<T>` + a thrown `IpcError`, like C1).
///
/// [Build-Session-Entscheidung: P2.24] **Interface-shell body — the typed CONTRACT is the deliverable.**
/// P2.24 authors the §0.4.1 wire signature; the native `DialogExt` folder-pick BODY (`app.dialog().file()
/// .pick_folder(..)`, opened async/`spawn_blocking` so it never blocks a Tokio worker, §7 app-shell) is wired
/// end-to-end at P3.56 — the DestinationBar, whose "Change destination" affordance drives C2b → C5 (P3.54 wires
/// the C2a *intake* picker, a distinct path; C2b is the *destination* picker). A native OS folder dialog is
/// **not unit-testable** (it needs a real OS dialog /
/// user interaction — the §6.6 walkthrough + the P9 E2E flow exercise it), so the testable P2 deliverable is
/// the typed contract; the shell returns `Ok(None)` — the genuine cancelled/no-pick result. This is the
/// sanctioned compile-time interface-shell pattern (CLAUDE §5 / the P3 `crate::isolation` shells P4 expands),
/// not a quiet deferral.
#[tauri::command]
#[specta::specta]
pub async fn pick_destination() -> Result<Option<PathBuf>, IpcError> {
    Ok(None)
}

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

#[cfg(test)]
mod c2b_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C2b `pick_destination` typed CONTRACT (P2.24). Mirrors the C1/C2a
    //! `*_contract` tests — the handler now carries its typed `-> Result<Option<PathBuf>, IpcError>` signature
    //! (the §0.4 universal error shape), so the P2.21 all-shells `block_on(pick_destination())` invocation in
    //! `crate::ipc` (mod.rs) is REPLACED here by C2b's own typed-contract test (the fill-box transition the
    //! P2.21 note schedules). The native folder-dialog body is not unit-testable (it needs a real OS dialog) and
    //! lands at P3.56 (the DestinationBar "Change destination" path); this asserts the typed contract returns
    //! the cancelled/no-pick `Ok(None)`. [Build-Session-Entscheidung: P2.24]
    use super::*;
    use tauri::async_runtime::block_on;

    // §6.4.1 unit (G15): the C2b contract is invocable and returns `Result<Option<PathBuf>, IpcError>` (the wire
    // door this box authors, in the §0.4 universal error shape). The shell opens no dialog yet (the DialogExt
    // body is P3.56, the DestinationBar "Change destination" path), so it returns `Ok(None)` — which is ALSO the
    // contract's genuine cancelled-dialog result (§0.4.1: `Ok(None)` = the user cancelled); P3.56 replaces it
    // with the real folder pick whose `Ok(Some(path))` carries into C5, and an `Err(IpcError)` for a genuine
    // dialog-subsystem failure.
    #[test]
    fn c2b_pick_destination_contract_is_invocable_and_typed() {
        let out: Result<Option<PathBuf>, IpcError> = block_on(pick_destination());
        assert_eq!(
            out,
            Ok(None),
            "§0.4.1/§0.4: the C2b contract shell opens no dialog yet (the DialogExt body is P3.56), so it \
             returns Ok(None) — also the §5.4 cancelled-pick result; the typed Result<Option<PathBuf>, \
             IpcError> signature (the §0.4 universal error shape) is the P2.24 deliverable"
        );
    }
}
