//! `crate::ipc::system` — the §0.4.1 OS shell-out + app-info command group (C9 / C10 / C11 / C12): reveal /
//! open an output path, open the canonical project page, surface app info, and report engine health. P2.21
//! registered these as the §0.4.1 command-surface interface shells; C9 `open_path`'s typed request/response
//! CONTRACT is authored by P2.32 (this file), C10's by P2.33, C11's by P2.34, and C12's wired by P2.113. Each
//! command's `crate::orchestrator`/`OpenerExt` delegation BODY is its own named fill-box (for C9 the gate
//! LOGIC — the `OpenKind`→`OpenerOp` mapping + the §7.7.3 membership-validate — is P2.100–103, and only the
//! live `OpenerExt` reveal/open wire is P3.51). Thin by design (§0.7): the handler validates,
//! delegates, and maps the `Result` onto the §0.4.3 `IpcError`. No `opener:*` WebView grant exists — every
//! shell-out is Rust-side via `OpenerExt` (§0.10).

// §0.4 / T10: unchecked arithmetic on an untrusted wire field must be a compile error in every IPC handler
// (the `crate::ipc` arithmetic-overflow deny cascades here; restated at the T10 boundary). The C9 contract
// handler below + the remaining C10/C11/C12 shells do no arithmetic; the deny bites the fill-bodies.
#![deny(clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};

use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use crate::domain::OpenKind;
use crate::engines::{AppInfo, EngineHealth};
use crate::orchestrator::RunResultPaths;
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
/// the C9 LOGIC is built PURE in P2 (the P2↔P3 §7.7 build-vs-wire split, Co-Pilot-ratified): the
/// `OpenKind`→`OpenerOp` mapping is P2.100, the §7.7.3 membership validate over the real P2.43 `RunResultStore`
/// is P2.101, the two-rule split (file→output FILES / folder→run ROOTS) is P2.102, and the split-output
/// two-targets rule is P2.103 — all pure, dead-until the wire box; only the LIVE WIRE — the `AppHandle`, the
/// current-`RunResult` fetch from `State<RunResultStore>`, the §7.7.1 `OpenerExt` reveal/open call, the §7.5
/// refusal log, and the §0.6 SUCCESS path (`Ok(())` on a validated open) — belongs to the wire box P3.51; (c)
/// `kind` is the CONCRETE `ConversionErrorKind`, not the `ErrorKind` alias (the P2.19 convention).
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn open_path(kind: OpenKind, path: PathBuf) -> Result<(), IpcError> {
    let _ = (kind, path);
    Err(IpcError {
        kind: ConversionErrorKind::InternalError,
        message: "Could not open the requested location.".into(),
        path_display: None,
        residue_display: None,
    })
}

/// The concrete §7.7.1 `OpenerExt` operation a C9 `OpenKind` maps to — a PURE descriptor that SELECTS the
/// shell-out call without performing it. The live invocation (`app.opener()` + the `OpenerExt` method, the
/// §0.10 no-`opener:*`-grant Rust-side path) is the P3.51 wire box; P2.100 owns only this mapping.
///
/// Two variants, because §7.7.1 has exactly two concrete methods: `reveal_item_in_dir` (reveal-with-select)
/// and `open_path(_, None)` (open a file in its default app OR a directory in the file manager). `OpenKind::
/// Folder` and `OpenKind::File` both map to `OpenPath` — the SAME `OpenerExt` call on a different subject (a
/// run ROOT vs an output FILE); that source/root distinction is the §7.7.3 membership gate's job (P2.101–103),
/// not the mapping's. (The type is referenced by `opener_op_for`'s signature so it is not itself dead; the
/// mapping FN carries the dead-until-P3.51 `expect`.) [Build-Session-Entscheidung: P2.100]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OpenerOp {
    /// `OpenerExt::reveal_item_in_dir(path)` — open the OS file manager with `path` selected/highlighted.
    RevealItemInDir(PathBuf),
    /// `OpenerExt::open_path(path, None)` — open `path` (a file in its OS default app, or a directory in the
    /// file manager) with no explicit program override.
    OpenPath(PathBuf),
}

/// Map a C9 `OpenKind` + `path` to its concrete §7.7.1 `OpenerExt` operation (P2.100). PURE dispatch: it
/// SELECTS the op, it does not invoke it — the live `OpenerExt` call is the P3.51 wire box. The `match` is
/// wildcard-free, so a future `OpenKind` variant fails to compile here until it is mapped (the §0.6 / §0.7
/// exhaustive-dispatch discipline). [Build-Session-Entscheidung: P2.100]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the §7.7.1 OpenKind→OpenerOp mapping is the pure C9 dispatch; its only production consumer is the P3.51 live-wire box (AppHandle + OpenerExt invoke — the build-vs-wire split), so it is dead in the production build until then (the §1.1-walk / §7.8.1-funnel dead-until pattern)."
    )
)]
pub(crate) fn opener_op_for(kind: OpenKind, path: PathBuf) -> OpenerOp {
    match kind {
        OpenKind::RevealInFolder => OpenerOp::RevealItemInDir(path),
        OpenKind::Folder | OpenKind::File => OpenerOp::OpenPath(path),
    }
}

/// Whether `path` is an allowed C9 open target for `kind` against the current run's OFF-WIRE
/// `RunResultPaths` — the §7.7.2 Rust-side membership gate that REPLACES a static opener scope (§0.10 carries
/// no `opener:*` grant, so a glob could never cover the §2.7 beside-source outputs). PURE validation over a
/// borrowed `&RunResultPaths` (the real `PathBuf`s the display-only wire `RunResult` shed, 2026-07-06 ruling
/// / §2.10.1): no `AppHandle`, no filesystem touch, no `OpenerExt` invoke — the live wire (which fetches the
/// paths from `State<RunResultStore>::paths` and calls the mapped `OpenerOp`) is P3.51/P3.79. The two §7.7.3
/// rules:
/// - **File launch** (`OpenKind::File`) admits ONLY a recorded OUTPUT file (`RunResultPaths.item_outputs` — a
///   succeeded item's real published path, §1.12/§2.1) — never a source, never an engine intermediate.
/// - **Folder browse** (`OpenKind::Folder` / `RevealInFolder`) admits ONLY a run ROOT — `common_root`
///   (beside-source) and, for a split-output batch, `divert_root` (§7.7.3). When a batch splits (§2.7.3), BOTH
///   roots are members, so §5.3 `OpenActions` renders TWO open-folder buttons (§7.7.1); a `divert_root` of
///   `None` leaves a single target.
///
/// The two rules are DISJOINT (§7.7.3 "two distinct membership rules"): a `File` request for a ROOT, or a
/// folder-browse request for an OUTPUT file, is REFUSED — the file-launch set and the folder-browse set never
/// overlap (asserted by the P2.102 exclusivity tests). Membership is EXACT equality against the run's
/// already-resolved recorded paths (`crate::fs_guard` writes the resolved real path, §2.3); the gate never
/// canonicalizes the WebView-supplied `path` (a TOCTOU footgun), so a `..`-containing or symlinked input that
/// would resolve INTO the recorded set is REFUSED, not admitted (Rust `Path` equality normalizes benign
/// lexical no-ops — an interior `.`, repeated separators, a trailing slash — each denoting the same file, but
/// never a `..` or a link). [Build-Session-Entscheidung: P2.101]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the §7.7.2/§7.7.3 C9 membership gate is pure validation over a &RunResultPaths; its only production consumer is the P3.51/P3.79 live-wire box (AppHandle + RunResultStore::paths fetch + OpenerExt invoke — the build-vs-wire split), so it is dead in the production build until then (the §1.1-walk / §7.8.1-funnel dead-until pattern)."
    )
)]
pub(crate) fn open_path_member(kind: OpenKind, path: &Path, paths: &RunResultPaths) -> bool {
    match kind {
        // File launch: only a path in the run's recorded OUTPUT files (§7.7.3 — never a source/intermediate).
        // The real output PathBufs live off-wire in RunResultPaths.item_outputs (2026-07-06 ruling, §2.10.1).
        OpenKind::File => paths.item_outputs.values().any(|out| out.as_path() == path),
        // Folder browse: only a run ROOT — common_root, plus divert_root for a split-output batch (§7.7.3).
        OpenKind::Folder | OpenKind::RevealInFolder => {
            path == paths.common_root.as_path() || paths.divert_root.as_deref() == Some(path)
        }
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
    //! §6.4.1 unit (G15): the §0.4.1 C9 `open_path` typed CONTRACT (P2.32) — same interface-shell pattern as
    //! C3/C4/C5/C6/C8: the handler carries its typed `{ kind, path } -> Result<(), IpcError>` signature, so the
    //! P2.21 all-shells `block_on(open_path())` invocation in `crate::ipc` (mod.rs) is REPLACED here by C9's own
    //! typed-contract test. The shell returns the genuine §7.7.3-refused `Err(InternalError)` (no `RunResult` to
    //! validate against yet, P2.43); SHAPE is asserted, NOT the provisional message (owned by the §2.8 catalog
    //! box). The §7.7.3 membership resolve is P2.101–103; only the `OpenerExt` reveal/open wire lands at P3.51.
    //! [Build-Session-Entscheidung: P2.32]
    use super::*;
    use tauri::async_runtime::block_on;

    // §6.4.1 unit (G15): the C9 contract is invocable with its full §0.4.1 typed arg set ({ kind, path }) and
    // returns a `Result<(), IpcError>` (the §0.4 universal error shape). The shell has no §1.12 `RunResult` to
    // membership-check against yet (P2.43), so every path is refused — it returns the genuine §7.7.3-refused
    // `Err(InternalError)`, the same Err the real body returns for a non-member path. SHAPE asserted (kind ==
    // InternalError), NOT the provisional message (owned by the §2.8 catalog box); the P2.101–103 gate is the
    // pure membership validate and P3.51 wires it live (RunResult fetch → gate → `OpenerExt` reveal/open).
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
mod c9_opener_op {
    //! §6.4.1 unit (G15): the P2.100 §7.7.1 `OpenKind`→`OpenerOp` mapping — the pure C9 dispatch (mapping
    //! only; no `OpenerExt` invoke — the live wire is P3.51). Pins each `OpenKind` variant to its §7.7.1 row
    //! (`RevealInFolder`→`reveal_item_in_dir`; `Folder` and `File`→`open_path`) and that `path` is carried
    //! through verbatim. The wildcard-free `match` in `opener_op_for` locks membership at compile time; this
    //! test pins the per-variant target. [Build-Session-Entscheidung: P2.100]
    use super::*;

    // §6.4.1 unit (G15): every §0.6 `OpenKind` maps to its §7.7.1 `OpenerExt` op, and the `path` (here a
    // non-trivial spaces+parens path) is moved into the op verbatim. `Folder` and `File` share `OpenPath` —
    // the SAME §7.7.1 call on a different subject; the source/root distinction is the §7.7.3 membership gate's
    // job (P2.101–103), not the mapping's.
    #[test]
    fn open_kind_maps_to_its_7_7_1_opener_op() {
        let p = PathBuf::from("/runs/out (1)/data.tsv");
        assert_eq!(
            opener_op_for(OpenKind::RevealInFolder, p.clone()),
            OpenerOp::RevealItemInDir(p.clone()),
            "§7.7.1: RevealInFolder → OpenerExt::reveal_item_in_dir(path)"
        );
        assert_eq!(
            opener_op_for(OpenKind::Folder, p.clone()),
            OpenerOp::OpenPath(p.clone()),
            "§7.7.1: Folder → OpenerExt::open_path(dir, None)"
        );
        assert_eq!(
            opener_op_for(OpenKind::File, p.clone()),
            OpenerOp::OpenPath(p.clone()),
            "§7.7.1: File → OpenerExt::open_path(path, None)"
        );
    }
}

#[cfg(test)]
mod c9_membership {
    //! §6.4.1 unit (G15): the P2.101 §7.7.2/§7.7.3 C9 `open_path_member` membership gate — pure validation over
    //! the off-wire `&RunResultPaths` (no AppHandle / FS / OpenerExt; the live wire is P3.51/P3.79). Asserts the
    //! two §7.7.3 rules: File-launch admits a recorded OUTPUT file; folder-browse admits a run ROOT
    //! (common_root / divert_root).
    //! The two-rule EXCLUSIVITY negatives are P2.102 and the split-output two-targets are P2.103 — both add
    //! cases to this module against the shared `run_with` builder. [Build-Session-Entscheidung: P2.101]
    //! P2.137 (phase-end hardening) adds the §7.7.3 ANTI-TRAVERSAL legs against the same builder: the
    //! `..`-perturbation negatives, the benign-lexical-no-op pins (each grounded on std `Path` equality
    //! first), and a §6.4.2 (G16) pinned-seed 512-case perturbation property.
    use super::*;
    use crate::domain::ItemId;
    use crate::orchestrator::RunResultPaths;
    use proptest::prelude::*;
    use proptest::test_runner::{RngAlgorithm, TestRng, TestRunner};
    use std::collections::BTreeMap;

    // A minimal OFF-WIRE `RunResultPaths` for the membership tests: one recorded output `PathBuf` per
    // `outputs` entry (keyed by a synthetic `ItemId`), the given real roots, no residues. This IS what C9
    // resolves against post-2026-07-06-ruling — the display-only wire `RunResult` shed its paths (§2.10.1),
    // so the gate reads only these real paths and the wire summary is not needed here. The prior `run_with`
    // built a full wire `RunResult` with uuid-deserialized `collected_set_id`/`run_id` — those ids (+ their
    // uuid-deserialization lines) are gone because C9 now reads `RunResultPaths`, not the wire `RunResult`;
    // that removal is obsolete fixture plumbing, NOT a suppressed assertion (the §7.7.3 cases below assert
    // membership verbatim). Shared by the P2.101 / P2.102 / P2.103 / P2.137 cases in this module.
    fn run_with(outputs: &[&str], common_root: &str, divert_root: Option<&str>) -> RunResultPaths {
        let item_outputs = outputs
            .iter()
            .enumerate()
            .map(|(i, out)| {
                let id = ItemId::from_index(u32::try_from(i).expect("test output index fits u32"));
                (id, PathBuf::from(out))
            })
            // [Test-Change: P3.76 — old-obsolete+new-correct, §2.10.1] the prior wire-`RunResult` fixture's removed uuid-deserialization lines are obsolete plumbing (see the fn doc above), not suppressed assertions.
            .collect();
        RunResultPaths {
            common_root: PathBuf::from(common_root),
            divert_root: divert_root.map(PathBuf::from),
            item_outputs,
            item_residues: BTreeMap::new(),
        }
    }

    // §6.4.1 unit (G15): the §7.7.3 File-launch rule — File admits a recorded OUTPUT file, and only that.
    #[test]
    fn file_launch_admits_a_recorded_output() {
        let run = run_with(&["/out/data.tsv"], "/out", None);
        assert!(
            open_path_member(OpenKind::File, Path::new("/out/data.tsv"), &run),
            "§7.7.3: File launch admits a recorded output file"
        );
        assert!(
            !open_path_member(OpenKind::File, Path::new("/out/other.tsv"), &run),
            "§7.7.3: File launch refuses a path that is not a recorded output"
        );
    }

    // §6.4.1 unit (G15): the §7.7.3 folder-browse rule — Folder / RevealInFolder admit the run's common_root.
    #[test]
    fn folder_browse_admits_the_common_root() {
        let run = run_with(&["/out/data.tsv"], "/out", None);
        for kind in [OpenKind::Folder, OpenKind::RevealInFolder] {
            assert!(
                open_path_member(kind, Path::new("/out"), &run),
                "§7.7.3: folder-browse admits the run's common_root"
            );
            assert!(
                !open_path_member(kind, Path::new("/elsewhere"), &run),
                "§7.7.3: folder-browse refuses a directory that is not a run root"
            );
        }
    }

    // §6.4.1 unit (G15): the §7.7.3 two-rule EXCLUSIVITY (P2.102) — the file-launch and folder-browse sets are
    // DISJOINT. A `File` request for a ROOT (or a SOURCE) is refused — never a source, never an intermediate,
    // never a directory. This is the security-critical "never a source (file-launch)" half of §7.7.3.
    // [Build-Session-Entscheidung: P2.102]
    #[test]
    fn file_launch_refuses_a_root_or_source() {
        let run = run_with(&["/out/data.tsv"], "/out", None);
        assert!(
            !open_path_member(OpenKind::File, Path::new("/out"), &run),
            "§7.7.3: File launch refuses the run's common_root — a directory root is never a launchable file"
        );
        assert!(
            !open_path_member(OpenKind::File, Path::new("/in/data.csv"), &run),
            "§7.7.3: File launch refuses a SOURCE path — never a source, never an engine intermediate"
        );
    }

    // §6.4.1 unit (G15): the other half of the §7.7.3 EXCLUSIVITY (P2.102) — a folder-browse request
    // (`Folder` / `RevealInFolder`) for an OUTPUT file is refused; folder-browse admits ONLY run ROOTS.
    // [Build-Session-Entscheidung: P2.102]
    #[test]
    fn folder_browse_refuses_an_output_file() {
        let run = run_with(&["/out/data.tsv"], "/out", None);
        for kind in [OpenKind::Folder, OpenKind::RevealInFolder] {
            assert!(
                !open_path_member(kind, Path::new("/out/data.tsv"), &run),
                "§7.7.3: folder-browse refuses an output FILE — it admits only run ROOTS, not individual outputs"
            );
        }
    }

    // §6.4.1 unit (G15): the split-output two-targets contract (P2.103) — a DIVERTED run (§2.7.3) carries BOTH
    // `common_root` AND `divert_root`, and folder-browse admits BOTH (§7.7.1 "two open-folder buttons" /
    // §7.7.3); File-launch still admits only the recorded outputs, never either root. This exercises the
    // `divert_root` disjunct of the folder-browse arm (untested until now — the P2.101 review flagged it).
    // [Build-Session-Entscheidung: P2.103]
    #[test]
    fn split_output_folder_browse_admits_both_roots() {
        let run = run_with(&["/out/data.tsv", "/dl/other.tsv"], "/out", Some("/dl"));
        for kind in [OpenKind::Folder, OpenKind::RevealInFolder] {
            assert!(
                open_path_member(kind, Path::new("/out"), &run),
                "§7.7.1/§7.7.3: a split-output run admits the beside-source common_root"
            );
            assert!(
                open_path_member(kind, Path::new("/dl"), &run),
                "§7.7.1/§7.7.3: a split-output run ALSO admits the divert_root (the second open-folder target)"
            );
            assert!(
                !open_path_member(kind, Path::new("/nope"), &run),
                "§7.7.3: a directory that is neither root is still refused"
            );
        }
        // File-launch admits neither root even when split-output — only the recorded OUTPUT files (§7.7.3).
        assert!(
            !open_path_member(OpenKind::File, Path::new("/dl"), &run),
            "§7.7.3: File launch refuses the divert_root — a directory root is never a launchable file"
        );
        assert!(
            open_path_member(OpenKind::File, Path::new("/dl/other.tsv"), &run),
            "§7.7.3: File launch admits the diverted item's recorded OUTPUT file"
        );
    }

    // §6.4.1 unit (G15): the §7.7.3 ANTI-TRAVERSAL negatives (P2.137) — the gate never canonicalizes the
    // WebView-supplied path (the TOCTOU footgun the P2.101 doc bans), and Rust `Path` equality PRESERVES a
    // `..` component (components() drops only `.` / repeated separators — no other normalization), so a
    // `..`-perturbed query is REFUSED even when it lexically resolves to a recorded member.
    // [Build-Session-Entscheidung: P2.137]
    #[test]
    fn dot_dot_perturbed_member_paths_are_refused() {
        let run = run_with(&["/out/data.tsv"], "/out", None);
        assert!(
            !open_path_member(OpenKind::File, Path::new("/out/../out/data.tsv"), &run),
            "§7.7.3: a `..`-perturbed File query is refused even though it lexically resolves to the \
             recorded output — membership is exact component equality, never canonicalization"
        );
        for kind in [OpenKind::Folder, OpenKind::RevealInFolder] {
            assert!(
                !open_path_member(kind, Path::new("/out/x/.."), &run),
                "§7.7.3: a `..`-perturbed folder-browse query is refused even though it lexically resolves \
                 to the run root — a `..` component never survives the membership gate"
            );
        }
    }

    // §6.4.1 unit (G15): the BENIGN lexical no-ops the P2.101 doc names — an interior `.`, a repeated
    // separator, a trailing slash — each denote the same file/dir under Rust `Path` component equality, so
    // the gate ADMITS them. Each std semantic is grounded on `Path` equality directly first (read-back
    // proof, test-strategy §0.2), then the gate is held to it: components() drops an interior `.` and a
    // repeated interior separator and ignores a trailing slash, but NEVER a `..`.
    // [Build-Session-Entscheidung: P2.137]
    #[test]
    fn benign_lexical_no_ops_are_admitted_by_component_equality() {
        assert_eq!(
            Path::new("/out/./data.tsv"),
            Path::new("/out/data.tsv"),
            "std: components() drops an interior `.`"
        );
        assert_eq!(
            Path::new("/out//data.tsv"),
            Path::new("/out/data.tsv"),
            "std: components() drops a repeated interior separator"
        );
        assert_eq!(
            Path::new("/out/"),
            Path::new("/out"),
            "std: a trailing slash does not change the component sequence"
        );
        assert_ne!(
            Path::new("/out/../out/data.tsv"),
            Path::new("/out/data.tsv"),
            "std: a `..` component is PRESERVED — no lexical resolution"
        );
        let run = run_with(&["/out/data.tsv"], "/out", None);
        assert!(
            open_path_member(OpenKind::File, Path::new("/out/./data.tsv"), &run),
            "§7.7.3: an interior-`.` no-op of a recorded output stays a File-launch member"
        );
        assert!(
            open_path_member(OpenKind::File, Path::new("/out//data.tsv"), &run),
            "§7.7.3: a repeated-separator no-op of a recorded output stays a File-launch member"
        );
        for kind in [OpenKind::Folder, OpenKind::RevealInFolder] {
            assert!(
                open_path_member(kind, Path::new("/out/"), &run),
                "§7.7.3: a trailing-slash no-op of the run root stays a folder-browse member"
            );
        }
    }

    /// The §7.7.3 anti-traversal property case-count floor (test-strategy §1.3 / G16: above proptest's 256
    /// default; matches the P2.126 `crate::ipc` boundary floor). [Build-Session-Entscheidung: P2.137]
    const P2_137_CASES: u32 = 512;

    /// A PINNED-SEED proptest runner — the per-module copy of the `crate::ipc` `ipc_boundary_proptest`
    /// P2.126 runner (test-strategy §1.3 / G16): `proptest!` seeds its forward run from entropy, so the
    /// perturbation exploration drives a `TestRunner` with a `deterministic_rng` directly — all 512 cases
    /// identical every run, a counterexample reproducible and never retried-to-pass.
    /// [Build-Session-Entscheidung: P2.137]
    fn pinned_runner() -> TestRunner {
        TestRunner::new_with_rng(
            ProptestConfig::with_cases(P2_137_CASES),
            TestRng::deterministic_rng(RngAlgorithm::ChaCha),
        )
    }

    /// Build a query path from the member's two segments (`/out/data.tsv`) with `infix` inserted as an
    /// extra component at slot `at` (0 = before `out`, 1 = between, 2 = after `data.tsv`).
    fn inject(infix: &str, at: usize) -> String {
        let mut parts = vec!["out", "data.tsv"];
        parts.insert(at, infix);
        format!("/{}", parts.join("/"))
    }

    /// A lexical perturbation of the member path `/out/data.tsv`, paired with its GROUND-TRUTH membership
    /// verdict derived from the perturbation's CONSTRUCTION (not from `Path` equality — that is the gate
    /// under test): a `..`-injecting perturbation must be REFUSED (components() preserves `..` — even one
    /// that lexically resolves straight back to the member), while a pure lexical no-op (interior `.`,
    /// repeated interior separator, trailing slash) stays a member. The repeated-separator case doubles
    /// only an INTERIOR separator — a leading `//` is a UNC prefix on Windows, a genuinely different path,
    /// not a no-op. [Build-Session-Entscheidung: P2.137]
    fn perturbed_member() -> impl Strategy<Value = (String, bool)> {
        let up_detour = ("[a-z0-9]{1,8}", 0usize..=2usize)
            .prop_map(|(seg, at)| (inject(&format!("{seg}/.."), at), false));
        let raw_parent = (0usize..=2usize).prop_map(|at| (inject("..", at), false));
        let cur_dir = (0usize..=2usize).prop_map(|at| (inject(".", at), true));
        let benign = prop_oneof![
            Just(("/out//data.tsv".to_owned(), true)),
            Just(("/out/data.tsv/".to_owned(), true)),
        ];
        prop_oneof![up_detour, raw_parent, cur_dir, benign]
    }

    // §6.4.2 property (G16): the pinned-seed 512-case anti-traversal sweep (P2.137) — over `..`-injected
    // and no-op lexical perturbations of a recorded output, the File-launch gate refuses every perturbation
    // UNLESS it is a pure lexical no-op. The expected verdict comes from the perturbation's construction,
    // so the property is not a tautology over the same `Path` equality the gate itself uses.
    // [Build-Session-Entscheidung: P2.137]
    #[test]
    fn traversal_perturbations_are_refused_unless_pure_lexical_no_ops() {
        let run = run_with(&["/out/data.tsv"], "/out", None);
        assert!(
            open_path_member(OpenKind::File, Path::new("/out/data.tsv"), &run),
            "§7.7.3: the unperturbed recorded output is a File-launch member (the property's baseline)"
        );
        pinned_runner()
            .run(&perturbed_member(), |(query, expect_member)| {
                let admitted = open_path_member(OpenKind::File, Path::new(&query), &run);
                prop_assert_eq!(
                    admitted,
                    expect_member,
                    "§7.7.3: `{}` — a `..`-injected perturbation must be refused (exact component \
                     equality, no canonicalization); a pure lexical no-op stays a member",
                    query
                );
                Ok(())
            })
            .expect("§7.7.3: the pinned-seed anti-traversal property holds over all 512 cases");
    }
}

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
    //! — the sanctioned boot-glue stratification (the C1 `ingest_paths` P2.60 / C13 `cancel_ingest` P2.71
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
