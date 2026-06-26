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

/// **C2b `pick_destination`** (§0.4.1) — the Rust-side `DialogExt` destination-folder picker. This box (P2.24)
/// authors the typed §0.4.1 wire CONTRACT — the `{} -> Option<PathBuf>` door — so the generated `bindings.ts`
/// carries the C2b surface. Unlike the C2a intake picker, the **one chosen `PathBuf` it returns legitimately
/// transits the WebView** into C5 `set_destination` (and then C6): it is a *write* destination, not a source
/// path, so it can never harm an original or read anything (§0.10 / §2.1 / §0.11 T2). `None` = the user
/// cancelled — a clean no-op; the held C4/C5 destination is unchanged.
///
/// [Build-Session-Entscheidung: P2.24] **`Option<PathBuf>` return, NO `IpcError` wrapper.** The §0.4.1 C2b
/// output column is a bare `Option<PathBuf>` (not `Result<_, IpcError>` like C1/C2a): a folder pick has no
/// user-facing failure mode — a cancel is `None`, not an error — so the contract carries no error arm. This is
/// the spec literal, not a deviation.
///
/// [Build-Session-Entscheidung: P2.24] **Interface-shell body — the typed CONTRACT is the deliverable.**
/// P2.24 authors the §0.4.1 wire signature; the native `DialogExt` folder-pick BODY (`app.dialog().file()
/// .pick_folder(..)`, opened async/`spawn_blocking` so it never blocks a Tokio worker, §7 app-shell) is wired
/// end-to-end at P3.56 — the DestinationBar, whose "Change destination" affordance drives C2b → C5 (P3.54 wires
/// the C2a *intake* picker, a distinct path; C2b is the *destination* picker). A native OS folder dialog is
/// **not unit-testable** (it needs a real OS dialog /
/// user interaction — the §6.6 walkthrough + the P9 E2E flow exercise it), so the testable P2 deliverable is
/// the typed contract; the shell returns `None` — the genuine cancelled/no-pick result. This is the sanctioned
/// compile-time interface-shell pattern (CLAUDE §5 / the P3 `crate::isolation` shells P4 expands), not a quiet
/// deferral.
#[tauri::command]
#[specta::specta]
pub async fn pick_destination() -> Option<PathBuf> {
    None
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
    //! `*_contract` tests — the handler now carries its typed `-> Option<PathBuf>` signature, so the P2.21
    //! all-shells `block_on(pick_destination())` invocation in `crate::ipc` (mod.rs) is REPLACED here by C2b's
    //! own typed-contract test (the fill-box transition the P2.21 note schedules). The native folder-dialog
    //! body is not unit-testable (it needs a real OS dialog) and lands at P3.56 (the DestinationBar "Change
    //! destination" path); this asserts the typed contract returns the cancelled/no-pick `None`.
    //! [Build-Session-Entscheidung: P2.24]
    use super::*;
    use tauri::async_runtime::block_on;

    // §6.4.1 unit (G15): the C2b contract is invocable and returns `Option<PathBuf>` (the wire door this box
    // authors). The shell opens no dialog yet (the DialogExt body is P3.56, the DestinationBar "Change
    // destination" path), so it returns `None` — which is ALSO the contract's genuine cancelled-dialog result
    // (§0.4.1: `None` = the user cancelled); P3.56 replaces it with the real folder pick whose `Some(path)`
    // carries into C5.
    #[test]
    fn c2b_pick_destination_contract_is_invocable_and_typed() {
        let out: Option<PathBuf> = block_on(pick_destination());
        assert_eq!(
            out, None,
            "§0.4.1: the C2b contract shell opens no dialog yet (the DialogExt body is P3.56), so it returns \
             None — also the cancelled-pick result; the typed Option<PathBuf> signature is the P2.24 deliverable"
        );
    }
}
