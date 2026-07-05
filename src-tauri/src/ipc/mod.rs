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
mod responsiveness_contract {
    //! §6.4.1 unit (G15): the §0.4/§1.11 C-command-surface RESPONSIVENESS CONTRACT — the WebView-side analogue
    //! of the per-engine watchdog (which is P3.44/P4.12). A STRUCTURAL (source-scan) assertion that no
    //! synchronous C-command can wedge the WebView, in three parts: (1) UNIVERSAL-ASYNC — every §0.4.1 C1–C13
    //! handler is `pub async fn`, so its dispatch yields to the Tokio runtime rather than the WebView thread;
    //! (2) STREAMING-SEAM on the long-running commands — C1/C2a carry `on_scan: Channel<ScanProgress>` and C6
    //! carries `on_progress: Channel<ConversionEvent>`, so their long work streams instead of resolving one
    //! end-of-call Promise (§5.8 "respond immediately, stream the rest"); (3) BLOCKING-NATIVE-OFFLOAD — the C2a
    //! native dialog opens via `spawn_blocking`, pinned by the `intake::c2a_contract` test module.
    //!
    //! [Build-Session-Entscheidung: P2.125 — Co-Pilot scope DECISION, Reading B] A STRUCTURAL contract-assert
    //! (the P2.72 assert-now / wire-P3.49 precedent), NOT a runtime test. The RUNTIME behaviours the box once
    //! read as — a large-folder C1 that streams throttled `ScanProgress` without a freeze, and C3/C4 that
    //! return within a bounded budget under a real §1.10 preflight — need the P3.49 `on_scan` emit + the C3/C4
    //! slice bodies + the P4.72 §1.10 estimator, none of which the P2 tree carries (`intake::ingest_paths`
    //! `_`-binds `on_scan` → `Empty`; `planning::get_targets` / `plan_output` are instant-return `Err` shells).
    //! Those runtime end-to-end streaming / latency assertions belong to P3.49 (+ P4.72), not this box. The
    //! signature invariant pinned here is present + stable now, so a sync/blocking regression on the C-surface
    //! reddens the moment it is introduced — the early guard the assert-now half exists to give.

    /// A per-file production-source reader (one of the `*_src` fns below) — aliased so the handler table's
    /// element type stays simple (clippy::type_complexity).
    type SrcFn = fn() -> &'static str;

    /// Everything before a scanned file's first `#[cfg(test)]`, so a needle can never match a test's own
    /// source. `concat!`-split so the literal marker is absent from this scanning module too.
    fn production_prefix(full: &'static str) -> &'static str {
        full.split_once(concat!("#[cfg", "(test)]"))
            .map_or(full, |(prefix, _)| prefix)
    }

    fn intake_src() -> &'static str {
        production_prefix(include_str!("intake.rs"))
    }
    fn planning_src() -> &'static str {
        production_prefix(include_str!("planning.rs"))
    }
    fn conversion_src() -> &'static str {
        production_prefix(include_str!("conversion.rs"))
    }
    fn system_src() -> &'static str {
        production_prefix(include_str!("system.rs"))
    }

    // P2.125.1 — the STREAMING-SEAM leg. The long-running commands carry a `Channel<T>` seam + are async, so
    // their work streams rather than blocking the WebView on one end-of-call Promise: C1 `ingest_paths` + C2a
    // `pick_for_intake` carry `on_scan: Channel<ScanProgress>` (a large-folder walk streams over it — the
    // throttled emit is P3.49); C6 `start_conversion` carries `on_progress: Channel<ConversionEvent>` (§5.8).
    // The C2a native-dialog `spawn_blocking` offload is pinned by `intake::c2a_contract`; this leg pins the
    // async + Channel-seam signature. Needles use the trailing `(` so a doc-comment mention of the fn name
    // cannot false-match the signature scan.
    #[test]
    fn long_running_commands_are_async_and_carry_a_streaming_channel_seam() {
        let intake = intake_src();
        assert!(
            intake.contains("pub async fn ingest_paths("),
            "§0.4/§1.1: C1 ingest_paths must be `pub async fn` (its walk yields to the runtime, never the WebView thread)"
        );
        assert!(
            intake.contains("pub async fn pick_for_intake("),
            "§0.4/§1.1: C2a pick_for_intake must be `pub async fn`"
        );
        assert_eq!(
            intake.matches("on_scan: Channel<ScanProgress>").count(),
            2,
            "§1.1/§5.8: both C1 ingest_paths and C2a pick_for_intake carry the on_scan: Channel<ScanProgress> \
             streaming seam (a large-folder walk streams over it, never a whole-walk block)"
        );

        let conversion = conversion_src();
        assert!(
            conversion.contains("pub async fn start_conversion("),
            "§0.4: C6 start_conversion must be `pub async fn`"
        );
        assert!(
            conversion.contains("on_progress: Channel<ConversionEvent>"),
            "§5.8: C6 start_conversion carries the on_progress: Channel<ConversionEvent> run-telemetry \
             streaming seam (progress streams, not one end-of-run Promise)"
        );
    }

    // P2.125.2 — the UNIVERSAL-ASYNC leg. Every §0.4.1 C1–C13 handler is `pub async fn` (never a synchronous
    // `pub fn`), so no command can block the WebView thread — the §0.4 C6 "respond immediately" model applied
    // to the whole surface (§1.11). Names the planning commands C3 `get_targets` / C4 `plan_output` / C5
    // `set_destination` explicitly (the re-scoped Reading-B structural residuum of the bounded-budget leg) and
    // scans the entire C-surface so a future sync handler ANYWHERE reddens. The runtime bounded-budget-under-a-
    // real-§1.10-preflight test is P3.49 (C3/C4 slice bodies) / P4.72 (the §1.10 estimator).
    #[test]
    fn every_c_command_handler_is_async() {
        let handlers: [(&str, SrcFn); 14] = [
            ("get_targets", planning_src),
            ("plan_output", planning_src),
            ("set_destination", planning_src),
            ("pick_destination", planning_src),
            ("ingest_paths", intake_src),
            ("pick_for_intake", intake_src),
            ("cancel_ingest", intake_src),
            ("start_conversion", conversion_src),
            ("cancel_run", conversion_src),
            ("get_run_summary", conversion_src),
            ("open_path", system_src),
            ("open_project_page", system_src),
            ("get_app_info", system_src),
            ("get_engine_health", system_src),
        ];
        for (name, src) in handlers {
            let s = src();
            assert!(
                s.contains(&format!("pub async fn {name}(")),
                "§0.4/§1.11: C-command `{name}` must be `pub async fn` (cooperative-yield — no synchronous command wedges the WebView)"
            );
            assert!(
                !s.contains(&format!("pub fn {name}(")),
                "§0.4/§1.11: C-command `{name}` must NOT be a synchronous `pub fn` (it would block the WebView thread)"
            );
        }
    }
}
