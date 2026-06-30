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

/// The §0.4.2 app-wide event NAMES — the closed `app://` event surface (P2.39). The three app-wide events
/// flow Rust→WebView via raw `app.emit(<name>, payload)` / TS `listen(<name>)` (§0.4.2 "App-wide events —
/// `app.emit` / TS `listen`"), NOT tauri-specta `collect_events!` typed events: rc.25's TS event codegen
/// unconditionally emits a `makeEvent` helper with an `any`-typed `payload` parameter, which would violate the no-`any`
/// rule frozen on the generated `bindings.ts` (G5/G8) — the same class of decision as P2.22's
/// `ErrorHandlingMode::Throw` (see `main.rs` `register_ipc_event_types`). Their payload types are authored +
/// `.types()`-registered (the §0.4.2 payloads: `AppFault` in `crate::outcome`, `IntakePayload` in
/// `crate::domain`; `app://close-requested` carries `()` — no payload type) so the TS `listen` side
/// type-checks. These constants are the SINGLE SOURCE of the names their emit sites (§2.13/§5.8,
/// §7.8.1, §7.3.2) and the P2.41/G23 closed-set gate reference.
pub mod events {
    // The names are referenced by their emit sites (§2.13/§7.8.1/§7.3.2) + the P2.41/G23 closed-set
    // gate; until those land they are dead in the PRODUCTION build, so `expect` (not `allow`) auto-flags the
    // moment the first emit site uses them — matching `crate::domain` / `crate::outcome`.
    #![cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the §0.4.2 app:// event-name constants are referenced by their emit sites (§2.13/§7.8.1/§7.3.2) + the P2.41/G23 closed-set gate, so they are dead in the production build until then."
        )
    )]

    /// `app://fault` — the §2.13 app-level fault (payload `AppFault`). Emit sites: §2.13.3 / §5.8 / §7.2.
    pub const APP_FAULT: &str = "app://fault";
    /// `app://intake` — the §7.8.1 launch-arg / second-instance IDLE-path hand-off (payload `IntakePayload`).
    pub const APP_INTAKE: &str = "app://intake";
    /// `app://close-requested` — the §7.3.2 mid-run window-close intercept (payload `()`; §7.3.2 emits a
    /// `serde_json::Value::Null`).
    pub const APP_CLOSE_REQUESTED: &str = "app://close-requested";
}

#[cfg(test)]
mod app_event_names {
    //! §6.4.1 unit (G15): the §0.4.2 app:// event NAMES (P2.39) are pinned to their exact fixed strings, so a
    //! typo / rename of an event name reddens at L2. The CLOSED-SET invariant (exactly these three, no fourth
    //! `app://` event) is the P2.41/G23 gate's job; this pins each name's literal value.
    use super::events;

    #[test]
    fn app_event_names_are_the_fixed_0_4_2_strings() {
        assert_eq!(events::APP_FAULT, "app://fault");
        assert_eq!(events::APP_INTAKE, "app://intake");
        assert_eq!(events::APP_CLOSE_REQUESTED, "app://close-requested");
    }
}

#[cfg(test)]
mod command_surface {
    //! §6.4.1 unit (G15): the §0.4.1 command surface registered at P2.21 is real, INVOCABLE async commands,
    //! not merely registered symbols. As each fill-box (P2.22+) authors its command's typed signature, that
    //! command's invocation here is REPLACED by its own typed-contract test co-located with the command (the
    //! P2.21-scheduled transition); this test then exercises the REMAINING bare-`()` interface shells. The
    //! `main.rs` `bindings_codegen` read-back test still proves the generated TS surface lists all 14
    //! commands; this proves the Rust side: each remaining registered shell is a live `async fn` that runs to
    //! completion. P2.22 filled C1 `ingest_paths` + P2.23 filled C2a `pick_for_intake` + P2.35 filled C13
    //! `cancel_ingest` (tested in `crate::ipc::intake`: `c1_contract` / `c2a_contract` / `c13_contract`); P2.24
    //! filled C2b `pick_destination`, P2.25 filled C3 `get_targets`, P2.26 filled C4 `plan_output`, P2.27 filled
    //! C5 `set_destination` (tested in `crate::ipc::planning`: `c2b_contract` / `c3_contract` / `c4_contract` /
    //! `c5_contract`); P2.29 filled C6 `start_conversion` + P2.30 filled C7 `cancel_run` + P2.31 filled C8
    //! `get_run_summary` (tested in `crate::ipc::conversion`: `c6_contract` / `c7_contract` / `c8_contract`);
    //! P2.32 filled C9 `open_path` + P2.33 filled C10 `open_project_page` + P2.34 filled C11 `get_app_info`
    //! (tested in `crate::ipc::system`: `c9_contract` / `c10_contract` / `c11_contract`), so the 1 remaining C12
    //! shell is exercised here. [Build-Session-Entscheidung: P2.21]
    use tauri::async_runtime::block_on;

    // §6.4.1 unit (G15): invoke every still-bare §0.4.1 command shell so the registered surface is EXERCISED
    // (the handler runs, not merely compiles + registers). Each empty shell body completes without panic —
    // the contract an interface shell carries until its fill-box authors the typed body. The remaining
    // invocations sit in ONE test fn so the Tauri async runtime is initialised once (no cross-test re-init).
    // Only C12 `get_engine_health` remains bare; C1 `ingest_paths` (P2.22) + C2a `pick_for_intake` (P2.23) +
    // C13 `cancel_ingest` (P2.35) are filled and tested in `crate::ipc::intake`, C2b `pick_destination` (P2.24)
    // + C3 `get_targets` (P2.25) + C4 `plan_output` (P2.26) + C5 `set_destination` (P2.27) in
    // `crate::ipc::planning`, C6 `start_conversion` (P2.29) + C7 `cancel_run` (P2.30) + C8 `get_run_summary`
    // (P2.31) in `crate::ipc::conversion`, and C9 `open_path` (P2.32) + C10 `open_project_page` (P2.33) + C11
    // `get_app_info` (P2.34) in `crate::ipc::system`.
    // [Test-Change: P2.22 — old-obsolete+new-correct, §0.4.1] old: the P2.21 all-shells test invoked the
    // bare-`()` C1 shell; new (verified by read-back — C1 now returns `Result<CollectedSet, IpcError>` over a
    // typed arg set, so the no-arg `()` invocation is obsolete and would no longer compile): C1's typed
    // contract is exercised in the `intake::c1_contract` module (since P2.60 the AppHandle-bound handler is the
    // §1.1a boot-glue split — pure-helper unit tests + a handler source-scan), so its line moves there.
    // [Test-Change: P2.23 — old-obsolete+new-correct, §0.4.1] old: this test invoked the bare-`()` C2a
    // `pick_for_intake` shell; new (verified by read-back — C2a now returns `Result<CollectedSet, IpcError>`
    // over a typed arg set, so the no-arg `()` invocation is obsolete and would no longer compile): C2a's typed
    // contract is exercised in the `intake::c2a_contract` module (since P2.70 the AppHandle-bound handler is the
    // §1.1a boot-glue split — pure-helper unit tests + handler source-scans), so its line moves there.
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
    // moves there.
    // [Test-Change: P2.27 — old-obsolete+new-correct, §0.4.1] old: this test invoked the bare-`()` C5
    // `set_destination` shell; new (verified by read-back — C5 now returns `Result<DestinationResolved,
    // IpcError>` over its typed arg set, so the no-arg `()` invocation is obsolete and would no longer compile):
    // C5's typed contract is exercised by `planning::c5_contract::c5_set_destination_contract_is_invocable_and_typed`
    // (asserting the genuine pre-registry `Err(InternalError)` SHAPE, not its provisional message), so its line
    // moves there.
    // [Test-Change: P2.29 — old-obsolete+new-correct, §0.4.1] old: this test invoked the bare-`()` C6
    // `start_conversion` shell; new (verified by read-back — C6 now returns `Result<RunId, IpcError>` over its
    // typed arg set incl. the non-optional `onProgress: Channel<ConversionEvent>` arg, so the no-arg `()`
    // invocation is obsolete and would no longer compile): C6's typed contract is exercised by
    // `conversion::c6_contract::c6_start_conversion_contract_is_invocable_and_typed` (asserting the genuine
    // pre-registry `Err(InternalError)` SHAPE, not its provisional message), so its line moves there.
    // [Test-Change: P2.30 — old-obsolete+new-correct, §0.4.1] old: this test invoked the bare-`()` C7
    // `cancel_run` shell with no value assertion; new (verified by read-back — C7 now returns `Result<(),
    // IpcError>` over a typed `runId` arg, so the no-arg `()` invocation is obsolete and would no longer
    // compile): C7's typed contract is exercised by
    // `conversion::c7_contract::c7_cancel_run_contract_is_invocable_and_typed` (asserting the genuine
    // idempotent no-op-cancel `Ok(())`), so its line moves there.
    // [Test-Change: P2.31 — old-obsolete+new-correct, §0.4.1] old: this test invoked the bare-`()` C8
    // `get_run_summary` shell; new (verified by read-back — C8 now returns `Result<RunResult, IpcError>` over a
    // typed `runId` arg, so the no-arg `()` invocation is obsolete and would no longer compile): C8's typed
    // contract is exercised by `conversion::c8_contract::c8_get_run_summary_contract_is_invocable_and_typed`
    // (asserting the genuine pre-retention `Err(InternalError)` SHAPE, not its provisional message), so its line
    // moves there.
    // [Test-Change: P2.32 — old-obsolete+new-correct, §0.4.1] old: this test invoked the bare-`()` C9
    // `open_path` shell; new (verified by read-back — C9 now returns `Result<(), IpcError>` over a typed
    // `{ kind, path }` arg set, so the no-arg `()` invocation is obsolete and would no longer compile): C9's
    // typed contract is exercised by `system::c9_contract::c9_open_path_contract_is_invocable_and_typed`
    // (asserting the genuine §7.7.3-refused `Err(InternalError)` SHAPE, not its provisional message), so its
    // line moves there.
    // [Test-Change: P2.33 — old-obsolete+new-correct, §0.4.1] old: this test invoked the bare-`()` C10
    // `open_project_page` shell with no value assertion; new (verified by read-back — C10 now returns
    // `Result<(), IpcError>` (the §0.4 universal error shape), so the bare invocation no longer asserts the
    // typed contract): C10's typed contract is exercised by
    // `system::c10_contract::c10_open_project_page_contract_is_invocable_and_typed` (asserting the genuine
    // deferred-body `Err(InternalError)` SHAPE, not its provisional message), so its line moves there.
    // (The no-arg call still compiled, but C10 is no longer a bare `()` shell — moving it keeps the
    // one-typed-contract-test-per-filled-command pattern, mirroring the C2b move.)
    // [Test-Change: P2.34 — old-obsolete+new-correct, §0.4.1] old: this test invoked the bare-`()` C11
    // `get_app_info` shell; new (verified by read-back — C11 now returns `Result<AppInfo, IpcError>`, so the
    // bare statement is an unused-`must_use` and no longer asserts the typed contract): C11's typed contract is
    // exercised by `system::c11_contract::c11_get_app_info_contract_is_invocable_and_typed` (asserting the
    // genuine deferred-body `Err(InternalError)` SHAPE, not its provisional message), so its line moves there.
    // [Test-Change: P2.35 — old-obsolete+new-correct, §0.4.1] old: this test invoked the bare-`()` C13
    // `cancel_ingest` shell; new (verified by read-back — C13 now takes a typed `collectingId` arg and returns
    // `Result<(), IpcError>`, so the no-arg `()` invocation is obsolete and would no longer compile): C13's
    // typed contract is exercised in the `intake::c13_contract` module (since P2.71 the AppHandle-bound handler
    // trips the §0.4.4 token via `IngestRegistry::cancel`, the §1.1a boot-glue split — an end-to-end
    // token-trip→`Empty` test + a handler source-scan), so its line moves there. This test now covers the 1
    // still-bare C12 shell.
    #[test]
    fn every_registered_command_shell_is_invocable() {
        block_on(super::system::get_engine_health()); // C12
    }
}
