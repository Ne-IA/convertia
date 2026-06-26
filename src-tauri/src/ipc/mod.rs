//! `crate::ipc` — the §0.4 Tauri command/event surface; the only module the WebView reaches. Thin by
//! design: validate the request, delegate to `crate::orchestrator`, and map the `Result` onto the
//! §0.4.3 `IpcError`. No raw `127.0.0.1` / `localhost` ever appears here outside `#[cfg(test)]` (the
//! G9 repo-invariant scopes its grep here).
//!
//! P2.21 wires the §0.4.1 command surface: the C1–C13 handlers are registered on the shared
//! `ipc_specta_builder()` (the `collect_commands![]` in main.rs), so the generated `bindings.ts` carries
//! the whole IPC door from here on. They live one-file-per-command-group (§0.7) as registered interface
//! shells — `intake` (C1/C2a/C13), `planning` (C2b/C3/C4/C5), `conversion` (C6/C7/C8), `system`
//! (C9/C10/C11/C12). Each command's full request/response contract + its `crate::orchestrator` delegation
//! is authored by its named fill-box (intake → P2.22/P2.23/P2.35, planning → P2.24/P2.25/P2.26/P2.27,
//! conversion → P2.29/P2.30/P2.31, system → P2.32/P2.33/P2.34 + the C12 `EngineHealth` wiring P2.113);
//! the closed-set completeness + drift gate over the surface is P2.36 (G23).

// §0.4 IPC-handler arithmetic-overflow deny (T10): a `MAX_USIZE` wire field must not silently overflow
// a `width*height*bpp`-style preflight, so unchecked arithmetic is a compile error at this module root.
// The G4 REQUIRED_ATTRS contract makes this deny mandatory the moment this module exists; it cascades to
// the command-group submodules below (each also restates it at its own T10 boundary). The rule bites on
// the real command handlers authored in the fill-boxes.
#![deny(clippy::arithmetic_side_effects)]

pub mod conversion;
pub mod intake;
pub mod planning;
pub mod system;

#[cfg(test)]
mod command_surface {
    //! §6.4.1 unit (G15): the §0.4.1 command surface registered at P2.21 is real, INVOCABLE async commands,
    //! not merely registered symbols. As each fill-box (P2.22+) authors its command's typed signature, that
    //! command's invocation here is REPLACED by its own typed-contract test co-located with the command (the
    //! P2.21-scheduled transition); this test then exercises the REMAINING bare-`()` interface shells. The
    //! `main.rs` `bindings_codegen` read-back test still proves the generated TS surface lists all 14
    //! commands; this proves the Rust side: each remaining registered shell is a live `async fn` that runs to
    //! completion. P2.22 filled C1 `ingest_paths` + P2.23 filled C2a `pick_for_intake` (tested in
    //! `crate::ipc::intake`: `c1_contract` / `c2a_contract`); P2.24 filled C2b `pick_destination`, P2.25 filled
    //! C3 `get_targets`, P2.26 filled C4 `plan_output` (tested in `crate::ipc::planning`: `c2b_contract` /
    //! `c3_contract` / `c4_contract`), so the 9 remaining C5..C13 shells are exercised here.
    //! [Build-Session-Entscheidung: P2.21]
    use tauri::async_runtime::block_on;

    // §6.4.1 unit (G15): invoke every still-bare §0.4.1 command shell so the registered surface is EXERCISED
    // (the handler runs, not merely compiles + registers). Each empty shell body completes without panic —
    // the contract an interface shell carries until its fill-box authors the typed body. The remaining
    // invocations sit in ONE test fn so the Tauri async runtime is initialised once (no cross-test re-init).
    // Listed in §0.4.1 C5..C13 order; C1 `ingest_paths` (P2.22) + C2a `pick_for_intake` (P2.23) are filled and
    // tested in `crate::ipc::intake`, and C2b `pick_destination` (P2.24) + C3 `get_targets` (P2.25) + C4
    // `plan_output` (P2.26) in `crate::ipc::planning`.
    // [Test-Change: P2.22 — old-obsolete+new-correct, §0.4.1] old: the P2.21 all-shells test invoked the
    // bare-`()` C1 shell; new (verified by read-back — C1 now returns `Result<CollectedSet, IpcError>` over a
    // typed arg set, so the no-arg `()` invocation is obsolete and would no longer compile): C1's typed
    // contract is exercised by `intake::c1_contract::c1_ingest_paths_contract_is_invocable_and_typed`, so its
    // line moves there.
    // [Test-Change: P2.23 — old-obsolete+new-correct, §0.4.1] old: this test invoked the bare-`()` C2a
    // `pick_for_intake` shell; new (verified by read-back — C2a now returns `Result<CollectedSet, IpcError>`
    // over a typed arg set, so the no-arg `()` invocation is obsolete and would no longer compile): C2a's typed
    // contract is exercised by `intake::c2a_contract::c2a_pick_for_intake_contract_is_invocable_and_typed`, so
    // its line moves there.
    // [Test-Change: P2.24 — old-obsolete+new-correct, §0.4.1] old: this test invoked the bare-`()` C2b
    // `pick_destination` shell with no value assertion; new (verified by read-back — C2b now returns
    // `Result<Option<PathBuf>, IpcError>` (the §0.4 universal error shape), so the bare invocation no longer
    // asserts the typed contract): C2b's typed contract is exercised by
    // `planning::c2b_contract::c2b_pick_destination_contract_is_invocable_and_typed` (asserting the
    // cancelled/no-pick `Ok(None)`), so its line moves there.
    // (The no-arg call still compiled, but C2b is no longer a bare `()` shell — moving it keeps the
    // one-typed-contract-test-per-filled-command pattern of C1/C2a.)
    // [Test-Change: P2.25 — old-obsolete+new-correct, §0.4.1] old: this test invoked the bare-`()` C3
    // `get_targets` shell; new (verified by read-back — C3 now returns `Result<TargetOffer, IpcError>` over a
    // typed `collectedSetId` arg, so the no-arg `()` invocation is obsolete and would no longer compile): C3's
    // typed contract is exercised by `planning::c3_contract::c3_get_targets_contract_is_invocable_and_typed`
    // (asserting the genuine pre-registry `Err(InternalError)` SHAPE, not its provisional message), so its line
    // moves there.
    // [Test-Change: P2.26 — old-obsolete+new-correct, §0.4.1] old: this test invoked the bare-`()` C4
    // `plan_output` shell; new (verified by read-back — C4 now returns `Result<OutputPlanPreview, IpcError>`
    // over its typed arg set, so the no-arg `()` invocation is obsolete and would no longer compile): C4's typed
    // contract is exercised by `planning::c4_contract::c4_plan_output_contract_is_invocable_and_typed`
    // (asserting the genuine pre-registry `Err(InternalError)` SHAPE, not its provisional message), so its line
    // moves there and this test now covers the 9 still-bare C5..C13 shells.
    #[test]
    fn every_registered_command_shell_is_invocable() {
        block_on(super::planning::set_destination()); // C5
        block_on(super::conversion::start_conversion()); // C6
        block_on(super::conversion::cancel_run()); // C7
        block_on(super::conversion::get_run_summary()); // C8
        block_on(super::system::open_path()); // C9
        block_on(super::system::open_project_page()); // C10
        block_on(super::system::get_app_info()); // C11
        block_on(super::system::get_engine_health()); // C12
        block_on(super::intake::cancel_ingest()); // C13
    }
}
