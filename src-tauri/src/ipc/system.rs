//! `crate::ipc::system` — the §0.4.1 OS shell-out + app-info command group (C9 / C10 / C11 / C12): reveal /
//! open an output path, open the canonical project page, surface app info, and report engine health. P2.21
//! registered these as the §0.4.1 command-surface interface shells; C9 `open_path`'s typed request/response
//! CONTRACT is authored by P2.32 (this file), C10's by P2.33, C11's by P2.34, and C12's wired by P2.113. Each
//! command's `crate::orchestrator`/`OpenerExt` delegation BODY is its own named fill-box (the C9
//! membership-validate + `OpenerExt` reveal/open is P3.51). Thin by design (§0.7): the handler validates,
//! delegates, and maps the `Result` onto the §0.4.3 `IpcError`. No `opener:*` WebView grant exists — every
//! shell-out is Rust-side via `OpenerExt` (§0.10).

// §0.4 / T10: unchecked arithmetic on an untrusted wire field must be a compile error in every IPC handler
// (the `crate::ipc` arithmetic-overflow deny cascades here; restated at the T10 boundary). The C9 contract
// handler below + the remaining C10/C11/C12 shells do no arithmetic; the deny bites the fill-bodies.
#![deny(clippy::arithmetic_side_effects)]

use std::path::PathBuf;

use crate::domain::OpenKind;
use crate::engines::AppInfo;
use crate::outcome::{ConversionErrorKind, IpcError};

/// **C9 `open_path`** (§0.4.1) — the DoD "one-click open-folder / open-file" action: reveal or open an output
/// in the OS file manager / default app. The handler **validates `path` against the current §1.12 `RunResult`'s
/// recorded outputs (file-launch) or roots (folder-browse) — the §7.7.3 membership gate** — then calls the
/// opener plugin's `OpenerExt` internally (`reveal_item_in_dir` / `open_path`, §7.7.1); there is **no
/// `opener:*` WebView capability** (§0.10) — the Rust-side membership check, not a static scope, is the real
/// gate (§7.7.2: beside-source outputs routinely fall outside any OS-known root, so a glob scope could never
/// cover them). This box (P2.32) authors the typed §0.4.1 wire CONTRACT — the `{ kind, path } -> Result<(),
/// IpcError>` door (the §0.4 universal error shape) — so the generated `bindings.ts` mirrors the C9 surface
/// (pulling `OpenKind` into the bindings as a command-arg type).
///
/// - `kind` — the §0.6 `OpenKind` (`RevealInFolder` | `Folder` | `File`) selecting the §7.7.1 `OpenerExt` op:
///   reveal-with-select / open-containing-folder / open-file-in-default-app.
/// - `path` — the path to open; the §7.7.3 gate admits an *output file* for `File` and a run *root* for the
///   folder-browse kinds (`RevealInFolder`/`Folder`), refusing anything else (never a source, never an
///   arbitrary WebView path).
///
/// [Build-Session-Entscheidung: P2.32] **Shell returns `Err(IpcError{ kind: InternalError })` — the
/// C3/C4/C5/C6/C8 branch (the §7.7.3 gate refuses), NOT C7's `Ok(())` no-op branch.** Unlike C7's idempotent
/// fire-and-forget cancel (whose `()` zero value makes a tripped-nothing a genuine no-op success), C9 is a
/// **gated side-effect**: it opens `path` *only if* the §7.7.3 membership check passes, and a path not in the
/// set is **refused** (§7.7.2/§7.7.3 — "a path not in that set is refused and logged"). A refusal is an error,
/// not a successful no-op — returning `Ok(())` would falsely claim the open happened. The shell has no §1.12
/// `RunResult` to validate against (the §0.4.4 retention registry is P2.43), so **every** path fails the
/// membership check — exactly the `Err` the real body returns for a non-member path: `Err(IpcError{ kind:
/// ConversionErrorKind::InternalError, … })` (§2.13 catch-all; the §3.2 `PlanError` precedent C3/C4/C5 cite).
/// The named fill-boxes own the rest: (a) the §2.8 catalog box owns the FINAL message — the string below is a
/// PROVISIONAL neutral English one — and must add a COMMAND-level string (the §2.8 catalog is item-scoped); (b)
/// the §7.7.3 membership resolve (against the P2.43 `RunResult` retention) + the §7.7.1 `OpenerExt` reveal/open
/// call + the §7.5 refusal log + the §0.6 SUCCESS path (`Ok(())` on a validated open) belong to the body box
/// P3.51; (c) `kind` is the CONCRETE `ConversionErrorKind`, not the `ErrorKind` alias (the P2.19 convention).
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn open_path(kind: OpenKind, path: PathBuf) -> Result<(), IpcError> {
    let _ = (kind, path);
    Err(IpcError {
        kind: ConversionErrorKind::InternalError,
        message: "Could not open the requested location.".into(),
        path: None,
        residue: None,
    })
}

/// **C10 `open_project_page`** (§0.4.1) — the **only** permitted, user-initiated network action: opens a fixed
/// compiled-in canonical Ne-IA GitHub Releases URL in the default browser via `OpenerExt::open_url` (§7.6.2 /
/// §7.7.1). The WebView supplies **no URL** — the handler opens a compiled-in constant, eliminating any
/// URL-injection surface (§7.7.2); there is **no `opener:*` WebView capability** (§0.10), and no fetch/parse of
/// the page itself (§7.6.1 no phone-home). This box (P2.33) authors the typed §0.4.1 wire CONTRACT — the `{} ->
/// Result<(), IpcError>` door (the §0.4 universal error shape) — so the generated `bindings.ts` mirrors the C10
/// surface.
///
/// [Build-Session-Entscheidung: P2.33] **Shell returns `Err(IpcError{ kind: InternalError })` — the
/// deferred-body branch (C8/C9), NOT C7's `Ok(())` no-op.** C10 is a **side-effect** command (open a URL); its
/// success type `()` has only one meaning — `Ok(())` = "the URL was opened". The real `OpenerExt::open_url`
/// wiring is the body box **P2.104** (it adds the `AppHandle` + the compiled-in §7.6.2 URL constant); this
/// contract shell performs no open, so returning `Ok(())` would **falsely claim the page opened** — the
/// fabricated success CLAUDE §5 forbids. (Unlike C7's idempotent cancel, where tripping nothing genuinely *is*
/// the desired "not running" state, an un-opened URL is *not* a desired state, so the C7 `Ok(())` no-op branch
/// does not apply.) The honest shell outcome is the `Err` the operation yields when it cannot complete:
/// `Err(IpcError{ kind: ConversionErrorKind::InternalError, … })` (§2.13 catch-all; the §3.2 `PlanError`
/// precedent C3/C4/C5 cite). P2.104 replaces this with the real open — `Ok(())` on a successful shell-out,
/// `Err` on a genuine `OpenerExt` failure (no browser / OS error). The named fill-boxes own the rest: (a) the
/// §2.8 catalog box owns the FINAL message — the string below is a PROVISIONAL neutral English one — and must
/// add a COMMAND-level string (the §2.8 catalog is item-scoped); (b) the compiled-in §7.6.2 URL constant + the
/// §7.7.1 `OpenerExt::open_url` call + the `AppHandle` belong to the body box P2.104; (c) `kind` is the CONCRETE
/// `ConversionErrorKind`, not the `ErrorKind` alias (the P2.19 convention).
#[tauri::command]
#[specta::specta]
pub async fn open_project_page() -> Result<(), IpcError> {
    Err(IpcError {
        kind: ConversionErrorKind::InternalError,
        message: "Could not open the project page.".into(),
        path: None,
        residue: None,
    })
}

/// **C11 `get_app_info`** (§0.4.1) — version, build id, platform, and the §3.7 third-party-licenses / NOTICE
/// data for the §5.9 About screen (§7.2.3); no network — every field is gathered in-process / in-bundle. This
/// box (P2.34) authors the typed §0.4.1 wire CONTRACT — the `{} -> Result<AppInfo, IpcError>` door (the §0.4
/// universal error shape; the §0.4.1 table Response column `AppInfo` is the success `T`, wrapped in `Result`
/// like every command) — so the generated `bindings.ts` mirrors the C11 surface and **pulls the §7.2.3
/// `AppInfo` graph (and its embedded §3.2.2 `Platform`) onto the wire** via this return: the §0.6
/// defer-registration-to-the-consumer pattern (the `EngineId`/`ScanProgress`/`ConversionEvent` precedent),
/// the first consumer of the `AppInfo`/`Platform` types authored at P2.112/P2.132.
///
/// [Build-Session-Entscheidung: P2.34] **Shell returns `Err(IpcError{ kind: InternalError })` — the C3/C4/C5/
/// C6/C8 interface-shell pattern (success type has no honest zero value), NOT the C7 `Ok(())` no-op branch.**
/// `AppInfo` carries four real fields (`version`/`build_id`/`platform`/`third_party_notice`); the version +
/// `build_id` data sources are the RELEASE-BLOCKING **P2.98 PRODUCER** (the `package_info()` version source +
/// the §6 CI build id, neither of which may ship empty), assembled there, and `third_party_notice` is the
/// bundled §3.7 THIRD-PARTY-LICENSES.txt resource (§3.7 generation). This contract box assembles none of
/// them, so the shell cannot produce an HONEST `AppInfo` — fabricating an `Ok(AppInfo)` with empty
/// `version`/`build_id` (or an invented notice) would LIE that real app info exists (CLAUDE §5; the §5.9
/// About screen would render blanks). So the honest shell outcome is exactly the `Err` the operation yields
/// when it cannot complete: `Err(IpcError{ kind: ConversionErrorKind::InternalError, … })` (§2.13 catch-all;
/// the §3.2 `PlanError` precedent C3/C4/C5 cite). P2.98 replaces this with the real assembly —
/// `Ok(AppInfo{ … })` gathered from `package_info()` (version), the §6 build-id producer, the §3.2.2
/// `Platform`, and the §3.7 notice resource (a non-wire `AppHandle` param the body adds does NOT change the
/// `{}` wire signature — Tauri injects it). The named fill-boxes own the rest: (a) the §2.8 catalog box owns
/// the FINAL message — the string below is a PROVISIONAL neutral English one — and must add a COMMAND-level
/// string (the §2.8 catalog is item-scoped); (b) the version / `build_id` producers + the §0.6 SUCCESS path
/// belong to P2.98; (c) `kind` is the CONCRETE `ConversionErrorKind`, not the `ErrorKind` alias (the P2.19
/// convention).
#[tauri::command]
#[specta::specta]
pub async fn get_app_info() -> Result<AppInfo, IpcError> {
    Err(IpcError {
        kind: ConversionErrorKind::InternalError,
        message: "Could not retrieve the app info.".into(),
        path: None,
        residue: None,
    })
}

/// **C12 `get_engine_health`** (§0.4.1) — the cached §7.2.3 startup self-check (which bundled engines are
/// present/runnable, which §3.4 patent-gated targets are available), feeding §5.2. The `EngineHealth` type
/// is authored at P2.111 and the cache is populated by the P4 probe. Registered as the §0.4.1 interface
/// shell (P2.21); the full `{} -> EngineHealth` contract is wired by P2.113. [Build-Session-Entscheidung: P2.21]
#[tauri::command]
#[specta::specta]
pub async fn get_engine_health() {}

#[cfg(test)]
mod c9_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C9 `open_path` typed CONTRACT (P2.32) — same interface-shell pattern as
    //! C3/C4/C5/C6/C8: the handler carries its typed `{ kind, path } -> Result<(), IpcError>` signature, so the
    //! P2.21 all-shells `block_on(open_path())` invocation in `crate::ipc` (mod.rs) is REPLACED here by C9's own
    //! typed-contract test. The shell returns the genuine §7.7.3-refused `Err(InternalError)` (no `RunResult` to
    //! validate against yet, P2.43); SHAPE is asserted, NOT the provisional message (owned by the §2.8 catalog
    //! box). The §7.7.3 membership resolve + the §7.7.1 `OpenerExt` reveal/open land at P3.51.
    //! [Build-Session-Entscheidung: P2.32]
    use super::*;
    use tauri::async_runtime::block_on;

    // §6.4.1 unit (G15): the C9 contract is invocable with its full §0.4.1 typed arg set ({ kind, path }) and
    // returns a `Result<(), IpcError>` (the §0.4 universal error shape). The shell has no §1.12 `RunResult` to
    // membership-check against yet (P2.43), so every path is refused — it returns the genuine §7.7.3-refused
    // `Err(InternalError)`, the same Err the real body returns for a non-member path. SHAPE asserted (kind ==
    // InternalError), NOT the provisional message (owned by the §2.8 catalog box); P3.51 replaces the shell
    // with the real membership validate → `OpenerExt` reveal/open.
    #[test]
    fn c9_open_path_contract_is_invocable_and_typed() {
        let out = block_on(open_path(
            OpenKind::RevealInFolder,
            PathBuf::from("/runs/out/data.tsv"),
        ));
        let err = out.expect_err(
            "§0.4.1/§0.4: the C9 shell has no RunResult to validate against yet (P2.43), so the §7.7.3 gate \
             refuses every path → the genuine Err(InternalError); the typed Result<(), IpcError> signature is \
             the P2.32 deliverable",
        );
        assert_eq!(
            err.kind,
            ConversionErrorKind::InternalError,
            "§2.13: the non-member-path shell outcome is the InternalError catch-all — SHAPE asserted, NOT \
             the provisional message (the §2.8 catalog box owns the final string)"
        );
    }
}

#[cfg(test)]
mod c10_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C10 `open_project_page` typed CONTRACT (P2.33). The handler now returns
    //! its typed `{} -> Result<(), IpcError>` (the §0.4 universal error shape), so the P2.21 all-shells
    //! `block_on(open_project_page())` invocation in `crate::ipc` (mod.rs) MOVES here (the no-arg call still
    //! compiles, but the bare invocation no longer asserts the typed contract — mirroring the C2b move). The
    //! shell returns the genuine deferred-body `Err(InternalError)` (the real `OpenerExt::open_url` body is
    //! P2.104); SHAPE is asserted, NOT the provisional message (owned by the §2.8 catalog box).
    //! [Build-Session-Entscheidung: P2.33]
    use super::*;
    use tauri::async_runtime::block_on;

    // §6.4.1 unit (G15): the C10 contract is invocable and returns `Result<(), IpcError>` (the §0.4 universal
    // error shape). The shell opens no URL yet (the `OpenerExt::open_url` body is P2.104), so it returns the
    // genuine deferred-body `Err(InternalError)` — returning `Ok(())` would falsely claim the page opened
    // (§7.6.2/§7.7.2). SHAPE asserted (kind == InternalError), NOT the provisional message (owned by the §2.8
    // catalog box); P2.104 replaces the shell with the real compiled-in-URL open.
    #[test]
    fn c10_open_project_page_contract_is_invocable_and_typed() {
        let out: Result<(), IpcError> = block_on(open_project_page());
        let err = out.expect_err(
            "§0.4.1/§0.4: the C10 contract shell opens no URL yet (the OpenerExt::open_url body is P2.104), so \
             it returns the genuine deferred-body Err(InternalError); the typed Result<(), IpcError> signature \
             is the P2.33 deliverable",
        );
        assert_eq!(
            err.kind,
            ConversionErrorKind::InternalError,
            "§2.13: the deferred-body shell outcome is the InternalError catch-all — SHAPE asserted, NOT the \
             provisional message (the §2.8 catalog box owns the final string)"
        );
    }
}

#[cfg(test)]
mod c11_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C11 `get_app_info` typed CONTRACT (P2.34). The handler now returns its
    //! typed `{} -> Result<AppInfo, IpcError>` (the §0.4 universal error shape; `AppInfo` is the §0.4.1 Response
    //! `T` pulled onto the wire via this return), so the P2.21 all-shells `block_on(get_app_info())` invocation
    //! in `crate::ipc` (mod.rs) MOVES here (the now-`Result` return makes the bare statement an unused-`must_use`,
    //! and the move keeps the one-typed-contract-test-per-filled-command pattern — mirroring the C2b/C10 move).
    //! The shell returns the genuine deferred-body `Err(InternalError)` (the real version + §6 build-id assembly
    //! is P2.98, the §3.7 notice resource is §3.7); SHAPE is asserted, NOT the provisional message (owned by the
    //! §2.8 catalog box). [Build-Session-Entscheidung: P2.34]
    use super::*;
    use tauri::async_runtime::block_on;

    // §6.4.1 unit (G15): the C11 contract is invocable with no args ({}) and returns `Result<AppInfo, IpcError>`
    // (the §0.4 universal error shape; AppInfo is the §0.4.1 Response T pulled onto the wire via this return).
    // The shell cannot assemble an honest AppInfo yet — the version/build_id producers (P2.98) + the §3.7 notice
    // resource are not wired — so it returns the genuine deferred-body `Err(InternalError)`; fabricating an
    // Ok(AppInfo) with empty version/build_id would lie that real app info exists (CLAUDE §5). SHAPE asserted
    // (kind == InternalError), NOT the provisional message (owned by the §2.8 catalog box); P2.98 replaces the
    // shell with the real package_info/build-id/Platform/notice assembly.
    #[test]
    fn c11_get_app_info_contract_is_invocable_and_typed() {
        let out: Result<AppInfo, IpcError> = block_on(get_app_info());
        let err = out.expect_err(
            "§0.4.1/§0.4: the C11 contract shell has no version/build_id producer (P2.98) or §3.7 notice yet, \
             so it returns the genuine deferred-body Err(InternalError); the typed Result<AppInfo, IpcError> \
             signature is the P2.34 deliverable",
        );
        assert_eq!(
            err.kind,
            ConversionErrorKind::InternalError,
            "§2.13: the deferred-body shell outcome is the InternalError catch-all — SHAPE asserted, NOT the \
             provisional message (the §2.8 catalog box owns the final string)"
        );
    }
}
