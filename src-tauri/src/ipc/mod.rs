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
