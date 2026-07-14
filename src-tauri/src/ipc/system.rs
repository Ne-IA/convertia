//! `crate::ipc::system` — the §0.4.1 OS shell-out + app-info command group (C9 / C10 / C11 / C12): reveal /
//! open a recorded output, open the canonical project page, surface app info, and report engine health. P2.21
//! registered these as the §0.4.1 command-surface interface shells; C9 `open_path`'s typed request/response
//! CONTRACT is authored by P2.32 (this file) and RE-KEYED at P3.79 to the `{ target: OpenTarget }` id form,
//! C10's by P2.33, C11's by P2.34, and C12's wired by P2.113. Each command's `crate::orchestrator`/`OpenerExt`
//! delegation BODY is its own named fill-box (for C9 the id-resolution LOGIC — `resolve_open_target`, which
//! folds the former `OpenKind`→`OpenerOp` mapping + the §7.7.3 membership gate into ONE `OpenTarget`→`OpenerOp`
//! resolution over the `RunResultStore` paths — is this box P3.79, and only the live `AppHandle` +
//! `State<RunResultStore>` + `OpenerExt` reveal/open wire is P3.51). Thin by design (§0.7): the handler
//! validates, delegates, and maps the `Result` onto the §0.4.3 `IpcError`. No `opener:*` WebView grant exists
//! — every shell-out is Rust-side via `OpenerExt` (§0.10).

// §0.4 / T10: unchecked arithmetic on an untrusted wire field must be a compile error in every IPC handler
// (the `crate::ipc` arithmetic-overflow deny cascades here; restated at the T10 boundary). The C9 contract
// handler below + the remaining C10/C11/C12 shells do no arithmetic; the deny bites the fill-bodies.
#![deny(clippy::arithmetic_side_effects)]

use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use crate::domain::OpenTarget;
use crate::engines::{AppInfo, EngineHealth};
use crate::orchestrator::RunResultPaths;
use crate::outcome::{ConversionErrorKind, IpcError};

/// **C9 `open_path`** (§0.4.1) — the DoD "one-click open-folder / open-file" action: reveal or open a recorded
/// output in the OS file manager / default app. The WebView names an **`OpenTarget` id — never a filesystem
/// path** (the 2026-07-06 core-owned-paths owner ruling); the handler **resolves that id against the current
/// run's `State<RunResultStore>`** (§0.4.4) to the core's OWN recorded `PathBuf` — membership IS successful
/// resolution, an unresolvable id is the §7.7.3 refusal (§7.7.2: no WebView path exists to validate,
/// canonicalize or race) — then calls the opener plugin's `OpenerExt` internally (`reveal_item_in_dir` /
/// `open_path`, §7.7.1). There is **no `opener:*` WebView capability** (§0.10) — the Rust-side id-resolution,
/// not a static scope, is the real gate (§7.7.2: beside-source outputs routinely fall outside any OS-known
/// root, so a glob scope could never cover them). This box (P3.79) RE-KEYS the typed §0.4.1 wire CONTRACT — the
/// `{ target } -> Result<(), IpcError>` door (the §0.4 universal error shape) — so the generated `bindings.ts`
/// mirrors the C9 surface, pulling `OpenTarget` onto the wire and retiring `OpenKind` + the WebView `path`.
///
/// - `target` — the §0.6 `OpenTarget` (`CommonRoot` | `DivertRoot` | `Item(ItemId)` | `Residue(ItemId)`)
///   naming which recorded location to surface: a run ROOT (folder browse), a recorded OUTPUT file (launch), or
///   an item's cleanup-residue location (reveal). The §7.7.3 resolution admits only what the run recorded.
///
/// [Build-Session-Entscheidung: P3.79 → wired P3.51] **Shell returns `Err(IpcError{ kind: InternalError })` —
/// the C3/C4/C5/C6/C8 branch (the §7.7.3 target does not resolve), NOT C7's `Ok(())` no-op branch.** C9 is a
/// **gated side-effect**: it opens the resolved location *only if* the `OpenTarget` resolves against the run's
/// recorded paths, and an unresolvable target is **refused** (§7.7.2/§7.7.3). A refusal is an error, not a
/// successful no-op — returning `Ok(())` would falsely claim the open happened. This box RE-KEYS the WIRE
/// (`OpenTarget` in, the pure `resolve_open_target` id→`OpenerOp` resolution beside it) but does NOT yet inject
/// the `State<RunResultStore>` (that is P3.51) — so the shell has no recorded paths to resolve against and
/// **every** target fails resolution, exactly the `Err` the real body returns for an unresolvable target:
/// `Err(IpcError{ kind: ConversionErrorKind::InternalError, … })` (§2.13 catch-all; the §3.2 `PlanError`
/// precedent C3/C4/C5 cite). The named fill-boxes own the rest: (a) the §2.8 catalog box owns the FINAL message
/// — the string below is a PROVISIONAL neutral English one — and must add a COMMAND-level string (the §2.8
/// catalog is item-scoped); (b) the C9 resolution LOGIC re-keys the P2.100–103 build-vs-wire split
/// (Co-Pilot-ratified) onto the resolved entry — `resolve_open_target` folds the P2.100 `OpenerExt`-op mapping
/// and the P2.101–103 membership gate into ONE id-resolution (the §7.7.3 check IS the resolution), pure and
/// dead-until the wire box; only the LIVE WIRE — the `AppHandle`, the `State<RunResultStore>` paths fetch, the
/// §7.7.1 `OpenerExt` reveal/open call, the §7.5 refusal log, and the §0.6 SUCCESS path (`Ok(())` on a resolved
/// open) — belongs to the wire box P3.51; (c) `kind` is the CONCRETE `ConversionErrorKind`, not the `ErrorKind`
/// alias (the P2.19 convention).
#[tauri::command]
#[specta::specta]
pub async fn open_path(target: OpenTarget) -> Result<(), IpcError> {
    let _ = target;
    Err(IpcError {
        kind: ConversionErrorKind::InternalError,
        message: "Could not open the requested location.".into(),
        path_display: None,
        residue_display: None,
    })
}

/// The concrete §7.7.1 `OpenerExt` operation a resolved `OpenTarget` maps to — a PURE descriptor that SELECTS
/// the shell-out call (and carries the core's OWN recorded path) without performing it. The live invocation
/// (`app.opener()` + the `OpenerExt` method, the §0.10 no-`opener:*`-grant Rust-side path) is the P3.51 wire
/// box; this box owns only the resolution.
///
/// Two variants, because §7.7.1 has exactly two concrete methods: `reveal_item_in_dir` (reveal-with-select)
/// and `open_path(_, None)` (open a file in its default app OR a directory in the file manager). A run ROOT
/// (`CommonRoot`/`DivertRoot`, folder browse) and an OUTPUT file (`Item`, file launch) both map to `OpenPath`
/// — the SAME `OpenerExt` call on a different subject; a `Residue` reveal maps to `RevealItemInDir` (reveal
/// only, never a launch, §7.7.1). (The type is referenced by `resolve_open_target`'s signature so it is not
/// itself dead; the resolution FN carries the dead-until-P3.51 `expect`.) [Build-Session-Entscheidung: P3.79]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OpenerOp {
    /// `OpenerExt::reveal_item_in_dir(path)` — open the OS file manager with `path` selected/highlighted.
    RevealItemInDir(PathBuf),
    /// `OpenerExt::open_path(path, None)` — open `path` (a file in its OS default app, or a directory in the
    /// file manager) with no explicit program override.
    OpenPath(PathBuf),
}

// [Test-Change: P3.79 — old-obsolete+new-correct, §7.7.2] the removed `opener_op_for` (P2.100 mapping) +
// `open_path_member` (P2.101–103 path-membership gate) — incl. their `#[cfg_attr(not(test), expect(dead_code))]`
// attributes — are FOLDED into `resolve_open_target` below: the 2026-07-06 core-owned-paths ruling re-keys the
// WebView-supplied-path validation to an `OpenTarget` id-resolution (the §0.4.1 C9 row + the §0.4 SUPERSEDED
// note), so the mapping+membership pair is obsolete and the single fold is correct (the removal is intentional,
// not a suppressed assertion — the FP is G70's `expect(`/`assert(`-token scan over the production removal).
/// Resolve a C9 `OpenTarget` id against the current run's OFF-WIRE `RunResultPaths` to the concrete §7.7.1
/// `OpenerExt` op on the core's OWN recorded path — `Some(op)` when the target names something the run
/// recorded, `None` when it does not (the §7.7.3 refusal the P3.51 wire logs, §7.5). This is the 2026-07-06
/// core-owned-paths RE-KEY of the P2.100–103 build-vs-wire split: it FOLDS the former `OpenKind`→`OpenerOp`
/// mapping (P2.100) and the path-membership gate (P2.101–103) into ONE id-resolution — "membership IS
/// successful resolution" (§7.7.2) — so the anti-TOCTOU / canonicalization surface of the old
/// validate-a-given-path gate dissolves: no WebView path exists to validate, canonicalize or race, the only
/// path in play is the core's own recorded one. PURE: no `AppHandle`, no filesystem touch, no `OpenerExt`
/// invoke — the live wire (which fetches the paths from `State<RunResultStore>::paths` and calls the mapped
/// `OpenerOp`) is P3.51. The §7.7.3 resolution rules, one per variant:
/// - **File launch** (`Item(id)`) resolves ONLY into the run's recorded OUTPUT files (`item_outputs`, §1.12 /
///   §2.1) — never a source, never an engine intermediate (neither is in the recorded output set); an
///   unknown / output-less id does not resolve. Opens via `OpenPath` (launch in the OS default app).
/// - **Folder browse** (`CommonRoot` / `DivertRoot`) resolves to a run ROOT — `common_root` (always) and,
///   for a split-output batch, `divert_root` (§2.7.3 / §7.7.3); a `DivertRoot` on an undiverted run does not
///   resolve. When a batch splits, BOTH roots resolve, so §5.3 `OpenActions` renders TWO open-folder buttons
///   (§7.7.1). Opens the folder via `OpenPath`.
/// - **Reveal** (`Residue(id)`) resolves to the item's recorded §2.6.4 cleanup-residue location
///   (`item_residues`); a residue-free id does not resolve. Reveals (never launches) via `RevealItemInDir`.
///
/// The `match` is wildcard-free, so a future `OpenTarget` variant fails to compile here until it is resolved
/// (the §0.6 / §0.7 exhaustive-dispatch discipline). [Build-Session-Entscheidung: P3.79] The two folder-browse
/// ROOTS map to the §7.7.1 GUARANTEED folder-open (`OpenPath(root)`) — the cross-platform base "Open folder"
/// affordance; the §7.7.1 `[REC]` reveal-with-highlight-a-single-subject enhancement is platform-conditional
/// and needs a subject output a ROOT target does not name, so the id-keyed resolution opens the folder
/// (complete for the "Open folder" DoD action). `Residue` is the sole `RevealItemInDir` producer (its §7.7.1
/// reveal-only mandate).
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the §7.7.2/§7.7.3 C9 id-resolution folds the OpenerExt-op mapping and the membership gate over a &RunResultPaths; its only production consumer is the P3.51 live-wire box (AppHandle + RunResultStore::paths fetch + OpenerExt invoke — the build-vs-wire split), so it is dead in the production build until then (the §1.1-walk / §7.8.1-funnel dead-until pattern)."
    )
)]
pub(crate) fn resolve_open_target(target: &OpenTarget, paths: &RunResultPaths) -> Option<OpenerOp> {
    match target {
        // Folder browse: a run ROOT opened via OpenPath (§7.7.1). CommonRoot always resolves; DivertRoot
        // resolves only on a split-output run (§2.7.3) — else the §7.7.3 refusal.
        OpenTarget::CommonRoot => Some(OpenerOp::OpenPath(paths.common_root.clone())),
        OpenTarget::DivertRoot => paths.divert_root.clone().map(OpenerOp::OpenPath),
        // File launch: only a recorded OUTPUT file (§7.7.3 — never a source/intermediate); an unknown id does
        // not resolve. The real output PathBufs live off-wire in RunResultPaths.item_outputs (§2.10.1).
        OpenTarget::Item(id) => paths.item_outputs.get(id).cloned().map(OpenerOp::OpenPath),
        // Reveal: the item's recorded §2.6.4 cleanup-residue location, reveal-only (§7.7.1); a residue-free id
        // does not resolve.
        OpenTarget::Residue(id) => paths
            .item_residues
            .get(id)
            .cloned()
            .map(OpenerOp::RevealItemInDir),
    }
}

/// The compiled-in canonical Ne-IA GitHub Releases URL C10 opens in the default browser (§7.6.2 / §7.7.2) —
/// the single authentic-builds origin (SSOT *Distribution & download trust*). Compiling the URL in, rather
/// than taking one as a WebView argument, IS the §7.7.2 C10 gate: the command carries no `url` wire parameter,
/// so there is no URL-injection surface — the one permitted, user-initiated network origin is this fixed
/// `https` constant and nothing else. [Build-Session-Entscheidung: P2.104]
const PROJECT_PAGE_URL: &str = "https://github.com/Ne-IA/convertia/releases";

/// **C10 `open_project_page`** (§0.4.1) — the **only** permitted, user-initiated network action: opens the
/// fixed compiled-in canonical Ne-IA GitHub Releases URL (the `PROJECT_PAGE_URL` constant, §7.6.2) in the
/// default browser via `OpenerExt::open_url` (§7.7.1). The WebView supplies **no URL** — the handler opens the
/// compiled-in constant, so there is no URL-injection surface (§7.7.2); there is **no `opener:*` WebView
/// capability** (§0.10 — a Rust-internal `OpenerExt` call is not capability-gated), and no fetch/parse of the
/// page itself (§7.6.1 no phone-home). The typed §0.4.1 wire CONTRACT (`{} -> Result<(), IpcError>`, the §0.4
/// universal error shape) was authored by P2.33; the generated `bindings.ts` mirrors the **arg-less** C10
/// surface — the `AppHandle` is a Tauri-injected arg, NOT part of the wire signature.
///
/// [Build-Session-Entscheidung: P2.33 → filled P2.104] **The body now performs the real shell-out.** P2.33
/// authored the typed contract with an honest `Err(InternalError)` shell — returning `Ok(())` from a shell that
/// opened nothing would falsely claim the page opened (the fabricated success CLAUDE §5 forbids). **P2.104
/// replaces that shell** with the real open: the handler binds an `AppHandle`, so it is AppHandle-coupled
/// boot-glue (§1.1a; G28 signature-exempt — its wiring is source-scan-pinned, this crate ships no `tauri::test`
/// mock BY DECISION), and hands the compiled-in constant to the §7.7.1 opener — `Ok(())` on a successful
/// shell-out, `Err(IpcError{ kind: InternalError, … })` on a genuine `OpenerExt` failure (no browser / OS
/// error). Two facts stay owned by their named boxes: (a) the §2.8 catalog box owns the FINAL message — the
/// string below is a PROVISIONAL neutral English one — and must add a COMMAND-level string (the §2.8 catalog is
/// item-scoped); (b) `kind` is the CONCRETE `ConversionErrorKind`, not the `ErrorKind` alias (the P2.19
/// convention).
#[tauri::command]
#[specta::specta]
pub async fn open_project_page(app: AppHandle) -> Result<(), IpcError> {
    app.opener()
        .open_url(PROJECT_PAGE_URL, None::<&str>)
        .map_err(|_err| IpcError {
            kind: ConversionErrorKind::InternalError,
            message: "Could not open the project page.".into(),
            path_display: None,
            residue_display: None,
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
/// [Build-Session-Entscheidung: P2.34 → filled P2.98] **The body now assembles a real `Ok(AppInfo)`.** P2.34
/// authored the typed `{} -> Result<AppInfo, IpcError>` contract with an honest `Err` shell — `AppInfo` has
/// no honest zero value, so fabricating an `Ok(AppInfo)` with an empty `version`/`build_id` would LIE that
/// real app info exists (CLAUDE §5; the §5.9 About screen would render blanks). **P2.98 replaced that shell**
/// with `Ok(AppInfo::gather())` — the §7.2.3 producer in `crate::engines` gathering all four fields in-process
/// / in-bundle with NO network (§2.11): `version` (`CARGO_PKG_VERSION`), `build_id` (the `build.rs` §6
/// producer), `platform` (the running §3.2.2 target), and `third_party_notice` (the bundled §3.7 notice). C11
/// stays `AppHandle`-free — `version` via `CARGO_PKG_VERSION` is identical to `app.package_info().version`
/// (`tauri.conf.json` omits `version`, so Tauri inherits the Cargo version; §7.6.2 offers either) — so it
/// remains a pure, unit-testable command. It cannot fail: `get_app_info` returns `Ok` unconditionally (the
/// `Result` wrapper is the §0.4 universal command shape, not a runtime error path here).
#[tauri::command]
#[specta::specta]
pub async fn get_app_info() -> Result<AppInfo, IpcError> {
    Ok(AppInfo::gather())
}

/// **C12 `get_engine_health`** (§0.4.1) — the cached §7.2.3 startup self-check (which bundled engines are
/// present/runnable, which §3.4 patent-gated targets are available on this platform), feeding §5.2
/// (disable/omit unavailable targets) and the §7.2.4 startup-fault surface. This box (P2.113) WIRES the typed
/// §0.4.1 wire CONTRACT — the `{} -> Result<EngineHealth, IpcError>` door (the §0.4 universal error shape; the
/// §0.4.1 table Response column `EngineHealth` is the success `T`, wrapped in `Result` like every command) —
/// so the generated `bindings.ts` mirrors the C12 surface and **pulls the §7.2.3 `EngineHealth` graph
/// (`EngineHealth` → `EngineStatus` → `EngineId`, + the embedded §0.6 `TargetId`) onto the wire** via this
/// return: the §0.6 defer-registration-to-the-consumer pattern (the `AppInfo`/`Platform`-via-C11 precedent),
/// this being the first consumer of the `EngineStatus`/`EngineHealth` types authored at P2.110/P2.111.
///
/// [Build-Session-Entscheidung: P2.113] **Honest `Err` shell — the C3/C4/C5/C6/C8/C9 shell branch, NOT a
/// fabricated `Ok`.** The cached `EngineHealth` is produced by the §7.2.3 startup ENGINE PROBE, which is P4.45
/// (the §7.2.1 step-3 verifier body); with no probe having populated the cache, there is no honestly-probed
/// `EngineHealth` to return — fabricating an `Ok(EngineHealth{ … })` here would CLAIM a startup self-check
/// result that never ran (the fabricated success CLAUDE §5 forbids; the identical reason C11's P2.34 shell
/// returned `Err` before P2.98 assembled the real `AppInfo`). So the shell returns the genuine
/// `Err(IpcError{ kind: InternalError, … })` (§2.13 catch-all, the C9 shell precedent), and **P4.45 replaces
/// this shell with the real cached `Ok(EngineHealth)`** (populate the C12 contract from the startup probe —
/// the build-vs-wire split, the C9 → P3.51 precedent). Two facts stay owned by their named boxes: (a) the §2.8
/// catalog box owns the FINAL message — the string below is a PROVISIONAL neutral English one — and must add a
/// COMMAND-level string (the §2.8 catalog is item-scoped); (b) `kind` is the CONCRETE `ConversionErrorKind`,
/// not the `ErrorKind` alias (the P2.19 convention).
#[tauri::command]
#[specta::specta]
pub async fn get_engine_health() -> Result<EngineHealth, IpcError> {
    Err(IpcError {
        kind: ConversionErrorKind::InternalError,
        message: "Engine health is unavailable.".into(),
        path_display: None,
        residue_display: None,
    })
}

#[cfg(test)]
mod c9_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C9 `open_path` typed CONTRACT (re-keyed at P3.79) — same interface-shell
    //! pattern as C3/C4/C5/C6/C8: the handler carries its typed `{ target } -> Result<(), IpcError>` signature
    //! (the §0.6 `OpenTarget` id, retiring `OpenKind` + the WebView `path`). The shell returns the genuine
    //! §7.7.3-refused `Err(InternalError)` (no `State<RunResultStore>` injected to resolve against yet — that is
    //! the P3.51 wire); SHAPE is asserted, NOT the provisional message (owned by the §2.8 catalog box). The
    //! §7.7.3 id-resolution is `resolve_open_target` (exercised by `c9_resolution`); only the live `OpenerExt`
    //! reveal/open wire lands at P3.51. [Build-Session-Entscheidung: P3.79]
    use super::*;
    use tauri::async_runtime::block_on;

    // §6.4.1 unit (G15): the C9 contract is invocable with its re-keyed §0.4.1 typed arg ({ target: OpenTarget })
    // and returns a `Result<(), IpcError>` (the §0.4 universal error shape). The shell has no injected
    // `State<RunResultStore>` to resolve against yet (P3.51), so every target fails resolution — it returns the
    // genuine §7.7.3-refused `Err(InternalError)`, the same Err the real body returns for an unresolvable
    // target. SHAPE asserted (kind == InternalError), NOT the provisional message (owned by the §2.8 catalog
    // box); the pure `resolve_open_target` is exercised by `c9_resolution` and P3.51 wires it live (State fetch
    // → resolve → `OpenerExt` reveal/open).
    // [Test-Change: P3.79 — old-obsolete+new-correct, §0.4.1] the arg set changed with the spec-mandated re-key
    // (`{ kind, path }` → `{ target }`, the §0.4.1 C9 row + the §0.4 SUPERSEDED note); the invocation is updated
    // to `OpenTarget::CommonRoot` and the assertion (the honest §7.7.3-refused Err(InternalError) shell) is
    // unchanged and re-verified — no assertion is relaxed.
    #[test]
    fn c9_open_path_contract_is_invocable_and_typed() {
        let out = block_on(open_path(OpenTarget::CommonRoot));
        let err = out.expect_err(
            "§0.4.1/§0.4: the C9 shell has no State<RunResultStore> injected to resolve against yet (P3.51), so \
             the §7.7.3 resolution refuses every target → the genuine Err(InternalError); the typed \
             Result<(), IpcError> signature is the P3.79 re-key deliverable",
        );
        assert_eq!(
            err.kind,
            ConversionErrorKind::InternalError,
            "§2.13: the unresolvable-target shell outcome is the InternalError catch-all — SHAPE asserted, NOT \
             the provisional message (the §2.8 catalog box owns the final string)"
        );
    }
}

// [Test-Change: P3.79 — old-obsolete+new-correct, §7.7.2] the P2.100 `c9_opener_op` `OpenKind`→`OpenerOp`
// mapping module AND the P2.101/P2.102/P2.103/P2.137 `c9_membership` module BOTH stood here and are DELETED,
// REPLACED by the `c9_resolution` module at the end of this file: they validated a WebView-SUPPLIED `path` (the
// `opener_op_for` mapping + the `open_path_member` gate incl. the anti-TOCTOU `..`-perturbation /
// benign-lexical-no-op property). Under the 2026-07-06 core-owned-paths ruling the WebView can no longer NAME a
// path (the §0.4.1 C9 row + the §0.4 SUPERSEDED note), so there is NO WebView path to validate, canonicalize or
// race (§7.7.2) — the whole path-perturbation surface DISSOLVES. The new expectation (id → recorded `OpenerOp`,
// total by construction) is verified in `c9_resolution` by reading back each variant against a real
// `RunResultPaths` (test-strategy §0.2). This single tombstone justifies the whole deleted two-module block.

#[cfg(test)]
mod c10_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C10 `open_project_page` shell-out body (P2.104). The handler now binds an
    //! `AppHandle` (to reach the §7.7.1 opener and open the compiled-in URL), so it is AppHandle-coupled
    //! boot-glue (the §1.1a pattern — NOT cargo-test-invocable; this crate ships no `tauri::test` mock BY
    //! DECISION, G28 signature-exempt). The PURE part — the compiled-in `PROJECT_PAGE_URL` value — is
    //! unit-tested here; the handler WIRING (bind the AppHandle, open the compiled-in constant via the opener)
    //! is source-scan-pinned; the runtime open is the §1.6 E2E / §6.6 walkthrough. [Build-Session-Entscheidung: P2.104]
    //!
    //! [Test-Change: P2.104 — old-obsolete+new-correct, §7.7.2/§7.6.2] the P2.33 direct
    //! `block_on(open_project_page())` contract test is OBSOLETE — P2.104 filled the body and the handler now
    //! binds an `AppHandle`, uninvocable without a Tauri runtime (none in cargo-test, §1.1a). It is REPLACED by
    //! the `PROJECT_PAGE_URL` constant unit test (the pure value, read back directly) + the handler source-scan
    //! — the sanctioned boot-glue stratification (the C1 `drain_intake` P2.60 / C13 `cancel_ingest` P2.71
    //! precedent), NOT a dropped assertion.
    use super::*;

    /// The production prefix of `system.rs` (everything before the FIRST `#[cfg(test)]`), so a needle declared
    /// in this test can never self-match — mirroring the intake.rs `c1_contract`/`c13_contract` helpers (each
    /// contract module keeps its own copy, the established per-module test-helper pattern).
    fn production_system_source() -> &'static str {
        let full = include_str!("system.rs");
        full.split_once(concat!("#[cfg", "(test)]"))
            .map_or(full, |(prefix, _)| prefix)
    }

    // §6.4.1 unit (G15): the compiled-in §7.6.2 URL is the canonical Ne-IA GitHub Releases page over https — the
    // single authentic-builds origin. Read-back proof (test-strategy §0.2): the exact literal is asserted, plus
    // the two security invariants the §7.7.2 no-injection posture rests on — https (never a downgradeable http)
    // and the canonical Ne-IA/convertia host (never an arbitrary origin).
    #[test]
    fn project_page_url_is_the_canonical_https_ne_ia_releases_page() {
        assert_eq!(
            PROJECT_PAGE_URL, "https://github.com/Ne-IA/convertia/releases",
            "§7.6.2: C10 opens the canonical Ne-IA GitHub Releases page"
        );
        assert!(
            PROJECT_PAGE_URL.starts_with("https://"),
            "§7.7.2: the only permitted network action opens an https origin, never a downgradeable http one"
        );
        assert!(
            PROJECT_PAGE_URL.starts_with("https://github.com/Ne-IA/convertia"),
            "§7.6.2: the origin is the canonical Ne-IA/convertia GitHub project, not an arbitrary host"
        );
    }

    // §6.4.1 unit (G15): the C10 handler is AppHandle-coupled boot-glue — a source-scan pins that it binds an
    // `AppHandle` and opens the compiled-in `PROJECT_PAGE_URL` constant via `OpenerExt::open_url` (the §7.7.2
    // no-WebView-URL / no-injection wiring), rather than the P2.33 `Err` shell. Needles `concat!`-assembled
    // (self-match avoidance); the literal call form appears only in the handler body, never the doc prose, so
    // the scan pins the CALL, not a comment. [Build-Session-Entscheidung: P2.104]
    #[test]
    fn open_project_page_handler_opens_the_compiled_in_url_via_the_opener() {
        let src = production_system_source();
        for needle in [
            concat!("open_project_page(app: App", "Handle"),
            concat!("open_", "url(PROJECT_PAGE_URL"),
        ] {
            assert!(
                src.contains(needle),
                "§7.7.1/§7.7.2: C10 must bind an AppHandle and open the compiled-in PROJECT_PAGE_URL via \
                 OpenerExt::open_url (missing `{needle}`)"
            );
        }
    }
}

#[cfg(test)]
mod c11_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C11 `get_app_info` typed CONTRACT (P2.34) + its filled body (P2.98). The
    //! handler returns `{} -> Result<AppInfo, IpcError>` (the §0.4 universal shape; `AppInfo` is the §0.4.1
    //! Response `T` pulled onto the wire via this return), so the P2.21 all-shells `block_on(get_app_info())`
    //! invocation in `crate::ipc` (mod.rs) lives here (mirroring the C2b/C10 move). P2.98 filled the body — it
    //! now assembles a real `Ok(AppInfo)` — so this test asserts the assembled payload's four fields, not the
    //! former `Err` shell. [Build-Session-Entscheidung: P2.34 → filled P2.98]
    use super::*;
    use tauri::async_runtime::block_on;

    // §6.4.1 unit (G15): the C11 contract is invocable with no args ({}) and, since P2.98 filled the body,
    // returns a real `Ok(AppInfo)` (§7.2.3) — the four fields assembled by `AppInfo::gather()`. Read-back proof
    // (test-strategy §0.2): version is CARGO_PKG_VERSION, build_id is the non-empty §6 build.rs id, platform is
    // the running target, and the §3.7 notice is embedded. [Test-Change: P2.98 — the old Err-shell expectation
    // is obsolete (P2.98 landed the real Ok(AppInfo) assembly per §7.2.3 / the P2.34 shell note), the new Ok
    // expectation is correct (verified by reading back the four real fields), §7.2.3]
    #[test]
    fn c11_get_app_info_contract_is_invocable_and_typed() {
        let out: Result<AppInfo, IpcError> = block_on(get_app_info());
        let info = out.expect(
            "§7.2.3/P2.98: C11 now assembles a real Ok(AppInfo) (version/build_id/platform/notice) — no \
             AppHandle, so it cannot fail; the typed Result<AppInfo, IpcError> signature is the §0.4 shape",
        );
        assert_eq!(
            info.version,
            env!("CARGO_PKG_VERSION"),
            "§7.2.3: version is the crate CARGO_PKG_VERSION (== app.package_info().version)"
        );
        assert!(
            !info.build_id.is_empty(),
            "§7.2.3: build_id is the §6 build.rs producer, never empty"
        );
        assert_eq!(
            info.platform,
            crate::engines::current_platform(),
            "§7.2.3: platform is the running compile target"
        );
        assert!(
            info.third_party_notice.contains("ConvertIA"),
            "§3.7: the bundled notice rides thirdPartyNotice"
        );
    }
}

#[cfg(test)]
mod c12_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C12 `get_engine_health` typed CONTRACT (P2.113). The handler returns
    //! `{} -> Result<EngineHealth, IpcError>` (the §0.4 universal shape; `EngineHealth` is the §0.4.1 Response
    //! `T` pulled onto the wire via this return, registering the §7.2.3 `EngineHealth`/`EngineStatus`/`EngineId`
    //! graph (plus the embedded `TargetId`) into `bindings.ts`), so the P2.21 all-shells
    //! `block_on(get_engine_health())` invocation in `crate::ipc` (the now-removed `command_surface` mod — C12
    //! was its last bare shell) is
    //! REPLACED here by C12's own typed-contract test, completing the P2.21-scheduled per-command move. The
    //! shell returns the genuine §7.2.3 pre-probe `Err(InternalError)` (no populated cache to read — the §7.2.1
    //! step-3 probe is P4.45); SHAPE is asserted, NOT the provisional message (owned by the §2.8 catalog box);
    //! P4.45 replaces the shell with the real cached `Ok(EngineHealth)`. [Build-Session-Entscheidung: P2.113]
    //!
    //! [Test-Change: P2.113 — old-obsolete+new-correct, §0.4.1] the P2.21 `command_surface` all-shells
    //! `every_registered_command_shell_is_invocable` exerciser is REMOVED: it existed to invoke the still-bare
    //! `()` interface shells, and with C12 filled here there are ZERO bare shells left (all 14 commands carry
    //! their own typed-contract test — the P2.22–P2.35/P2.98/P2.104 per-command moves), so the all-bare-shells
    //! exerciser is obsolete (old-obsolete). Its invocability coverage is fully subsumed by the 14 per-command
    //! contract tests (new-correct — verified: C12's line moves here). Not a dropped assertion — the removed
    //! test carried a bare `block_on(...)` statement, no assertion.
    use super::*;
    use tauri::async_runtime::block_on;

    // §6.4.1 unit (G15): the C12 contract is invocable with no args ({}) and returns a `Result<EngineHealth,
    // IpcError>` (the §0.4 universal error shape). The shell has no populated §7.2.3 cache to read (the step-3
    // startup probe is P4.45), so it returns the genuine pre-probe `Err(InternalError)` — the same honest-shell
    // branch as C3/C4/C5/C6/C8/C9 (no fabricated Ok). SHAPE asserted (kind == InternalError), NOT the
    // provisional message (owned by the §2.8 catalog box); P4.45 replaces the shell with the real cached
    // `Ok(EngineHealth)`. [Build-Session-Entscheidung: P2.113]
    #[test]
    fn c12_get_engine_health_contract_is_invocable_and_typed() {
        let out: Result<EngineHealth, IpcError> = block_on(get_engine_health());
        let err = out.expect_err(
            "§0.4.1/§7.2.3: the C12 shell has no populated cache to read (the step-3 startup probe is P4.45), \
             so the §7.2.3 self-check returns the genuine pre-probe Err(InternalError); the typed \
             Result<EngineHealth, IpcError> signature is the P2.113 deliverable",
        );
        assert_eq!(
            err.kind,
            ConversionErrorKind::InternalError,
            "§2.13: the pre-probe shell outcome is the InternalError catch-all — SHAPE asserted, NOT the \
             provisional message (the §2.8 catalog box owns the final string)"
        );
    }
}

#[cfg(test)]
mod c9_resolution {
    //! §6.4.1 unit (G15): the P3.79 C9 `resolve_open_target` id-resolution — pure resolution of an `OpenTarget`
    //! id against the off-wire `&RunResultPaths` to a concrete §7.7.1 `OpenerExt` op (no AppHandle / FS /
    //! OpenerExt; the live wire is P3.51). It FOLDS the former P2.100 `OpenKind`→`OpenerOp` mapping and the
    //! P2.101–103 path-membership gate into ONE resolution — "membership IS successful resolution" (§7.7.2).
    //! Covers the §7.7.3 rules the old gate covered (output-file launch, root folder-browse incl. split-output,
    //! residue reveal) PLUS the refusal cases the id form introduces (unknown id, undiverted `DivertRoot`,
    //! residue-free `Residue`, disjoint launch/reveal tables). [Build-Session-Entscheidung: P3.79]
    //! [Test-Change: P3.79 — old-obsolete+new-correct, §7.7.2] this module REPLACES the deleted `c9_opener_op`
    //! (P2.100 mapping) + `c9_membership` (P2.101–103 path-membership incl. the P2.137 anti-traversal property):
    //! the 2026-07-06 core-owned-paths ruling retires the WebView path (the §0.4.1 C9 row), so the
    //! path-perturbation surface those covered dissolves and id-resolution is the correct successor.
    use super::*;
    use crate::domain::ItemId;

    /// A `RunResultPaths` builder for the resolution tests: outputs + residues keyed by explicit `ItemId`
    /// index, the two roots given directly. Shared by the cases below.
    fn paths_with(
        outputs: &[(u32, &str)],
        common_root: &str,
        divert_root: Option<&str>,
        residues: &[(u32, &str)],
    ) -> RunResultPaths {
        RunResultPaths {
            common_root: PathBuf::from(common_root),
            divert_root: divert_root.map(PathBuf::from),
            item_outputs: outputs
                .iter()
                .map(|(i, p)| (ItemId::from_index(*i), PathBuf::from(p)))
                .collect(),
            item_residues: residues
                .iter()
                .map(|(i, p)| (ItemId::from_index(*i), PathBuf::from(p)))
                .collect(),
        }
    }

    // §6.4.1 unit (G15): the §7.7.3 folder-browse rule — CommonRoot ALWAYS resolves to the run's common root
    // opened via `OpenPath` (the §7.7.1 guaranteed "Open folder"), the primary DoD affordance.
    #[test]
    fn common_root_resolves_to_the_folder_open() {
        let paths = paths_with(&[(0, "/out/data.tsv")], "/out", None, &[]);
        assert_eq!(
            resolve_open_target(&OpenTarget::CommonRoot, &paths),
            Some(OpenerOp::OpenPath(PathBuf::from("/out"))),
            "§7.7.1/§7.7.3: CommonRoot resolves to OpenPath on the run's common root (folder browse)"
        );
    }

    // §6.4.1 unit (G15): the §7.7.3 DivertRoot rule — resolves to the divert root (`OpenPath`) ONLY on a
    // split-output run (§2.7.3); an undiverted run has no divert root, so the target is the §7.7.3 refusal
    // (`None`). Also pins the split-output "two open-folder targets" (§7.7.1): BOTH roots resolve. This is the
    // divert disjunct the old P2.103 test covered, re-cut to id-resolution.
    #[test]
    fn divert_root_resolves_only_on_a_split_output_run() {
        let split = paths_with(
            &[(0, "/out/data.tsv"), (1, "/dl/other.tsv")],
            "/out",
            Some("/dl"),
            &[],
        );
        assert_eq!(
            resolve_open_target(&OpenTarget::DivertRoot, &split),
            Some(OpenerOp::OpenPath(PathBuf::from("/dl"))),
            "§7.7.1/§7.7.3: a split-output run resolves DivertRoot to OpenPath on the divert root"
        );
        assert_eq!(
            resolve_open_target(&OpenTarget::CommonRoot, &split),
            Some(OpenerOp::OpenPath(PathBuf::from("/out"))),
            "§7.7.1: a split-output run ALSO resolves the beside-source common root (two open-folder buttons)"
        );
        let undiverted = paths_with(&[(0, "/out/data.tsv")], "/out", None, &[]);
        assert_eq!(
            resolve_open_target(&OpenTarget::DivertRoot, &undiverted),
            None,
            "§7.7.3: DivertRoot on an undiverted run does not resolve — the refusal"
        );
    }

    // §6.4.1 unit (G15): the §7.7.3 file-launch rule — Item(id) resolves to that item's recorded OUTPUT file
    // opened via `OpenPath` (launch in the OS default app); an unknown id does not resolve. The lookup is over
    // `item_outputs` ONLY, so a source/intermediate — never recorded there — is structurally unreachable (the
    // WebView cannot even NAME one, §7.7.2 core-owned paths).
    #[test]
    fn item_resolves_to_the_recorded_output_file_launch() {
        let paths = paths_with(
            &[(0, "/out/data.tsv"), (2, "/out/more.tsv")],
            "/out",
            None,
            &[],
        );
        assert_eq!(
            resolve_open_target(&OpenTarget::Item(ItemId::from_index(2)), &paths),
            Some(OpenerOp::OpenPath(PathBuf::from("/out/more.tsv"))),
            "§7.7.3: Item resolves to the recorded output file (file launch via OpenPath)"
        );
        assert_eq!(
            resolve_open_target(&OpenTarget::Item(ItemId::from_index(9)), &paths),
            None,
            "§7.7.3: Item with an id absent from item_outputs does not resolve — the refusal"
        );
    }

    // §6.4.1 unit (G15): the §7.7.3 reveal rule — Residue(id) resolves to that item's recorded §2.6.4
    // cleanup-residue location REVEALED via `RevealItemInDir` (never a launch, §7.7.1); a residue-free id does
    // not resolve.
    #[test]
    fn residue_resolves_to_the_reveal_of_the_recorded_residue() {
        let paths = paths_with(
            &[(0, "/out/data.tsv")],
            "/out",
            None,
            &[(0, "/out/.convertia-residue")],
        );
        assert_eq!(
            resolve_open_target(&OpenTarget::Residue(ItemId::from_index(0)), &paths),
            Some(OpenerOp::RevealItemInDir(PathBuf::from("/out/.convertia-residue"))),
            "§7.7.1/§7.7.3: Residue resolves to RevealItemInDir on the recorded residue location (reveal only)"
        );
        assert_eq!(
            resolve_open_target(
                &OpenTarget::Residue(ItemId::from_index(0)),
                &paths_with(&[], "/out", None, &[]),
            ),
            None,
            "§7.7.3: Residue on an item with no recorded residue does not resolve — the refusal"
        );
    }

    // §6.4.1 unit (G15): the §7.7.3 two-rule DISJOINTNESS, re-cut to id-resolution — the file-launch table
    // (`item_outputs`) and the reveal table (`item_residues`) are SEPARATE lookups keyed by the same id space,
    // so an id present ONLY in `item_residues` does not resolve as an Item file-launch (and vice versa). This is
    // the old P2.102 exclusivity ("never a source/root for file-launch") in the id-keyed form: the tables never
    // overlap, and no path can be named to cross them (§7.7.2).
    #[test]
    fn file_launch_and_reveal_tables_are_disjoint_lookups() {
        // Item id 5 has a RESIDUE but no OUTPUT; id 0 has an OUTPUT but no residue.
        let paths = paths_with(
            &[(0, "/out/data.tsv")],
            "/out",
            None,
            &[(5, "/out/.convertia-residue")],
        );
        assert_eq!(
            resolve_open_target(&OpenTarget::Item(ItemId::from_index(5)), &paths),
            None,
            "§7.7.3: an id present only in item_residues does not resolve as an Item file-launch (disjoint tables)"
        );
        assert_eq!(
            resolve_open_target(&OpenTarget::Residue(ItemId::from_index(0)), &paths),
            None,
            "§7.7.3: an id present only in item_outputs does not resolve as a Residue reveal (disjoint tables)"
        );
    }
}
