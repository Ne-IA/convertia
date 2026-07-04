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
