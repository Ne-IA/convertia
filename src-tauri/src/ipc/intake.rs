//! `crate::ipc::intake` — the §0.4.1 intake command group (C1 / C2a / C13): the single §2.4 freeze point
//! for every intake origin (drop / picker / launch-arg / second-instance) and the ingest-scoped cancel. P2.21
//! registered these as the §0.4.1 command-surface interface shells; C1's typed request/response CONTRACT is
//! authored by P2.22 and C2a's by P2.23 (this file), C13's by P2.35. Each command's `crate::orchestrator`
//! freeze BODY is its own named fill-box (the C1 freeze funnel is P2.62; C2a's native-dialog pick is
//! P2.70/P2.71 + the `Picker`-origin stamp P2.63; the end-to-end walking-skeleton wiring is P3.49). Thin by
//! design (§0.7): the handler validates, delegates, and maps the `Result` onto §0.4.3 `IpcError`.
//!
//! [Build-Session-Entscheidung: P3.78] The 2026-07-06 core-owned-path ruling reshaped this group's wire
//! surface: C1 `ingest_paths { paths, origin, collectingId, drainPending?, onScan }` → **C1 `drain_intake
//! { collectingId, onScan }`** (every call drains the core-side §7.8.1 `PendingIntake` buffer — the WebView
//! supplies no path, no origin, no drain flag), and C2a `pick_for_intake` sheds its `collectingId`/`onScan`
//! and returns `()` (the picker only FILLS the §7.8.1 funnel — stash + nudge — and the WebView completes with
//! C1 `drain_intake`, so C1 is the sole `onScan` carrier). §0.4.1 documents the target contract; this box
//! lands the code + golden + regenerated `bindings.ts` in one commit.

// §0.4 / T10: unchecked arithmetic on an untrusted wire field must be a compile error in every IPC handler
// (the `crate::ipc` arithmetic-overflow deny cascades here; restated at the T10 boundary so this group's
// real handlers carry it explicitly). The C1/C2a/C13 handlers below do no arithmetic; the deny bites the
// remaining fill-body (the §1.1 walk/freeze wired at P3.49).
#![deny(clippy::arithmetic_side_effects)]

use std::path::PathBuf;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::domain::{CollectedSet, CollectingId, IntakeOrigin, PickKind, ScanProgress};
use crate::orchestrator::{ingest, FrontendReady, IngestRegistry, PendingIntake};
use crate::outcome::{ConversionErrorKind, IpcError};

/// **C1 `drain_intake`** (§0.4.1) — the universal intake-completion door: the single §2.4 freeze point for
/// **every** intake origin (the Rust `WindowEvent::DragDrop` native drop, the C2a picker, launch-arg /
/// second-instance / Open-with — §7.8). This box (P2.22, reshaped P3.78) authors the typed §0.4.1 wire
/// CONTRACT — the `{ collectingId, onScan } -> CollectedSet` door — so the generated `bindings.ts` mirrors the
/// C1 surface, pulling the whole `CollectedSet` graph + `ScanProgress` into the bindings as named types (the
/// §0.6 defer-registration-to-the-consumer pattern).
///
/// - `collecting_id` — the frontend-generated ingest-scoped cancel handle (§0.4.4) so C13 `cancel_ingest`
///   can name this in-flight walk **before** C1's long await resolves (§1.1).
/// - `on_scan` — the throttled scan-telemetry Channel (§0.4.2 `ScanProgress`, ≈2/s) driving the §5.2
///   *Collecting* "Scanning… N files" count; best-effort, monotonic, dies with the call. **Always passed**
///   (non-optional — see the forced-deviation note below); the frontend realises the §0.4.1 "optional" intent
///   by subscribing only for a long walk, never by omitting the argument. C1 is the **sole** `onScan` carrier
///   (C2a walks nothing, §0.4.1).
///
/// [Build-Session-Entscheidung: P3.78] **Every call drains — the P2.60 `drainPending` flag is retired.** The
/// 2026-07-06 core-owned-path ruling makes the drain the ONLY mode: the WebView no longer supplies `paths` /
/// `origin` / `drainPending` — every intake origin funnels core-side into `State<PendingIntake>` (the native
/// drop, the C2a picker, launch-arg / second-instance / Open-with), and this command DRAINS it. So the handler
/// binds an `AppHandle` (a Tauri-injected arg, NOT part of the §0.4.1 wire signature) to reach the Rust-side
/// `State<PendingIntake>` + `State<FrontendReady>` (P2.58/P2.59) and dispatches via the pure-state
/// `drain_to_collected_set` helper: MARK the frontend ready (the §7.8.1 readiness signal) + CONSUME
/// `PendingIntake` exactly once; a non-empty buffer enters the §1.1/§2.4 freeze funnel (`ingest`) with its
/// STORED origin, an empty buffer is the genuine §0.6 zero-collection `CollectedSet::Empty` (a raced/duplicate
/// drain, or the ordinary first launch with no files). **No FS path crosses this wire in either direction**
/// (§2.10.1); the `origin` travels inside the buffer (core-side `IntakeOrigin`), never on the wire.
///
/// [Build-Session-Entscheidung: P2.22] **The freeze BODY is P3.49.** The §1.1 recursive walk → §1.2 detect →
/// §2.3 de-dup → §1.3 group freeze funnel (`ingest`, homed in `crate::orchestrator`) is a compile-time
/// interface shell that returns the §0.6 zero-collection `CollectedSet::Empty { skipped: [] }` until P3.49
/// wires it end-to-end (CLAUDE §5 / the P3 `crate::isolation` shells P4 expands — NOT a quiet deferral). The
/// §0.4.4 `collecting_id` token registration + the `on_scan` throttled emit are that same P3.49 walk's, so
/// here `collecting_id`/`on_scan` are shell-accepted (`_`-bound).
///
/// [Build-Session-Entscheidung: P2.22] **`on_scan` is NON-OPTIONAL — a FORCED deviation from the §0.4.1
/// `onScan?` `[DECIDED]`.** tauri 2.11.3's `Channel<T>` is `!Deserialize` (it carries its own `CommandArg`
/// impl, but `Option<Channel<T>>` routes through the `CommandArg for D: Deserialize` blanket impl → E0277),
/// so an optional channel argument cannot compile. No behaviour is lost: the wire-form optionality is realised
/// by the frontend subscribing only for a long walk, never by omitting the arg. The rejected alternative was a
/// custom `OptionalChannel<T>` wrapper replicating undocumented `__CHANNEL__:N` internals (version-fragile).
/// §0.4.1 (C1) / §0.4.2 + the README / plan / §05 mirrors are spec-synced non-optional (DoD item 2).
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn drain_intake(
    app: AppHandle,
    collecting_id: CollectingId,
    on_scan: Channel<ScanProgress>,
) -> Result<CollectedSet, IpcError> {
    // §7.8.1/§0.4.1 (P3.78): every call drains. `app.state::<…>()` is infallible by construction — both stores
    // are `.manage()`'d in main()'s Builder chain before the event loop (P2.58/P2.59), so no panic under the
    // `crate::ipc` clippy::panic deny. `&State<T>` deref-coerces to the `&T` the pure helper takes, keeping the
    // AppHandle resolution in this thin (G28-exempt) wrapper. The §0.4.4 token registration + the `on_scan`
    // throttled emit are the §1.1 walk's (P3.49), so `collecting_id`/`on_scan` are shell-accepted here.
    let pending = app.state::<PendingIntake>();
    let ready = app.state::<FrontendReady>();
    let _ = (collecting_id, on_scan);
    Ok(drain_to_collected_set(&pending, &ready))
}

/// [Build-Session-Entscheidung: P3.78] The §7.8.1 / §0.4.1 C1 drain — resolve what THIS `drain_intake` call
/// yields from the §1.1/§2.4 freeze seam. Every call drains (the P2.60 `drainPending` flag is retired —
/// draining `PendingIntake` IS the command): the two cohesive effects — MARK the frontend ready + CONSUME the
/// buffer exactly once — run FUSED under the pending-slot `Mutex` (`take_marking_ready`, §7.8.1). A non-empty
/// buffer enters the single §1.1 freeze funnel (`ingest`, the interface shell until P3.49) with its STORED
/// origin (a first-launch set drains as `LaunchArg`, a picked set as `Picker` — never a hard-coded origin); an
/// empty buffer is the genuine §0.6 zero-collection `CollectedSet::Empty` (a raced/duplicate drain, or the
/// ordinary first launch with no files, §0.4.1). Mark-BEFORE-take inside the lock is the §7.8.1 no-loss
/// ordering's second rule (P3.77): an intake whose `stash` serializes after this section sees `ready == true`
/// and nudges → the next drain consumes it; every intake set is either observed by this drain or nudged for a
/// fresh one. Takes `&PendingIntake` / `&FrontendReady` (NOT the `AppHandle`) so it is fully unit-testable with
/// real state — the AppHandle resolution stays in the thin command wrapper (the §1.1a boot-glue split).
fn drain_to_collected_set(pending: &PendingIntake, ready: &FrontendReady) -> CollectedSet {
    match pending.take_marking_ready(ready) {
        // §1.1/§2.4: freeze the drained buffer with its STORED origin through the single freeze funnel
        // (`ingest`, the interface shell until P3.49 → the zero-collection `Empty`).
        Some(buffered) => ingest(buffered.paths, buffered.origin),
        // §7.8.1: nothing pending — the genuine §0.6 zero-collection `Empty` (a raced/duplicate drain, or the
        // ordinary first launch with no files). The UI stays put (§0.4.1).
        None => CollectedSet::Empty {
            skipped: Vec::new(),
        },
    }
}

/// **C2a `pick_for_intake`** (§0.4.1) — the Rust-side `DialogExt` intake picker. This box (P2.23, reshaped
/// P3.78) authors the typed §0.4.1 wire CONTRACT — the `{ kind } -> ()` door. The picker **only fills the
/// §7.8.1 funnel**: it opens the native dialog Rust-side, stamps the `Picker` origin core-side, routes the
/// picked paths through the SAME `forward_launch_intake` funnel every other intake source uses (uniform §7.1.1
/// refuse-busy → `State<PendingIntake>` → the payload-less `app://intake` nudge), and returns `()`. The WebView
/// then completes the intake with **C1 `drain_intake`** — so **no raw FS path ever reaches the WebView** and C1
/// is the sole walk/freeze/`onScan` carrier (§0.4.1).
///
/// - `kind` — the §0.6 `PickKind` (`Files` | `Folder`): open the native files-multiselect or the folder
///   dialog; a folder pick is recursively collected at the §1.1 freeze (inside the C1 drain, P3.49).
///
/// C2a carries **no `collectingId` / `onScan`** (P3.78): the picker walks nothing — the walk, the freeze,
/// `collectingId`, `onScan` and the `CollectedSet` return all live on C1 `drain_intake`. During the modal there
/// is no walk, so a C13-during-modal has nothing to cancel (§0.4.1). It carries **no `origin` field** either:
/// the picked set's origin is `Picker`, **stamped by this handler itself** (P2.63) — so a compromised WebView
/// cannot forge the intake origin (§1.1 / §5.4).
///
/// [Build-Session-Entscheidung: P3.78] **Fill-the-funnel phase.** The handler binds an `AppHandle` (a
/// Tauri-injected arg, NOT part of the §0.4.1 wire signature — the generated C2a command is `{ kind }`) to
/// open the native `DialogExt` picker and reach the §7.8.1 funnel. It opens the picker on a **dedicated
/// blocking thread** (`spawn_blocking` + `blocking_pick_*`, never a synchronous `blocking_pick_*` on a Tokio
/// worker), so the runtime stays free. After the dialog it runs the **AppHandle-free `resolve_pick_outcome`**
/// decision (§1.1a split, unit-tested): a user-dismissed dialog → a clean no-op (nothing buffered, no nudge,
/// the UI stays Idle, §5.4); otherwise the picked paths are **`Picker`-stamped core-side** and funnelled through
/// `crate::launch_intake::forward_launch_intake` (stash + nudge — the same §7.8.1 funnel the native drop /
/// launch-arg use), and the WebView drains via C1. This handler is AppHandle-coupled boot-glue (§1.1a; G28
/// signature-exempt): the dialog open is source-scan-pinned, the outcome decision is `resolve_pick_outcome`
/// (unit-tested + G27-counted). (The former P2.70 `collectingId` ingest-token registration is retired with the
/// picker's walk — there is no dialog-phase walk to cancel; only the C1 drain registers a token, P3.49.)
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn pick_for_intake(app: AppHandle, kind: PickKind) -> Result<(), IpcError> {
    // §1.1/§0.4.1 (P3.78): open the native picker on a DEDICATED BLOCKING THREAD (spawn_blocking), never a
    // synchronous blocking_pick_* on a Tokio worker — so the async runtime stays free. A spawn_blocking failure
    // (the dialog thread panicked — should-never-happen) surfaces as an InternalError, never a silent no-op.
    let dialog_app = app.clone();
    let picked: Option<Vec<FilePath>> = tauri::async_runtime::spawn_blocking(move || match kind {
        PickKind::Files => dialog_app.dialog().file().blocking_pick_files(),
        PickKind::Folder => dialog_app
            .dialog()
            .file()
            .blocking_pick_folder()
            .map(|f| vec![f]),
    })
    .await
    .map_err(|_| IpcError {
        kind: ConversionErrorKind::InternalError,
        message: "Could not open the file picker.".into(),
        path_display: None,
        residue_display: None,
    })?;

    // FilePath -> PathBuf (a desktop pick is always a real path; drop a non-path entry defensively). This
    // conversion is the handler's boot-glue; the §1.1a pure decision is resolve_pick_outcome.
    let picked_paths: Option<Vec<PathBuf>> = picked.map(|files| {
        files
            .into_iter()
            .filter_map(|f| f.into_path().ok())
            .collect()
    });

    // §1.1/§0.4.1 (P3.78): the AppHandle-FREE post-dialog decision (§1.1a split, unit-tested). A user-dismissed
    // dialog is a clean no-op — nothing buffered, no nudge, the UI stays Idle (§5.4). A pick funnels the picked
    // set through the SAME §7.8.1 funnel every other intake source uses (`forward_launch_intake`: uniform §7.1.1
    // refuse-busy → stash into `PendingIntake` with origin `Picker` → the payload-less `app://intake` nudge),
    // Picker-stamped CORE-SIDE (a compromised WebView cannot forge the origin, §5.4 / §0.10); the WebView then
    // completes the intake with C1 `drain_intake`. C2a itself walks nothing and returns `()`.
    match resolve_pick_outcome(picked_paths) {
        PickOutcome::DialogCancelled => {}
        PickOutcome::Picked(picked_paths) => {
            crate::launch_intake::forward_launch_intake(&app, picked_paths, IntakeOrigin::Picker);
        }
    }
    Ok(())
}

/// The C2a post-dialog outcome (§1.1a split, P2.70; reshaped P3.78) — AppHandle-FREE so it is unit-tested +
/// G27-counted (the dialog open is the AppHandle-coupled boot-glue, source-scan-pinned). INTERNAL: the handler
/// maps it onto the funnel (Picked) or a no-op (DialogCancelled). [Build-Session-Entscheidung: P3.78]
#[derive(Debug, PartialEq, Eq)]
enum PickOutcome {
    /// The user dismissed the native dialog (§5.4 clean no-op) → nothing is buffered, no nudge, the UI stays Idle.
    DialogCancelled,
    /// The user picked these paths → funnel them (Picker-stamped) through the §7.8.1 funnel.
    Picked(Vec<PathBuf>),
}

/// [Build-Session-Entscheidung: P3.78] The §1.1 C2a post-dialog decision — the §1.1a pure half of
/// `pick_for_intake`. Given the dialog's picked paths (`None` = the user dismissed it), decide the outcome: a
/// dismissed dialog is a clean no-op (§5.4), a pick funnels the paths (Picker-stamped, at the handler). Takes
/// already-converted `PathBuf`s (the `FilePath` -> `PathBuf` conversion is the handler's boot-glue), so it is
/// fully unit-testable with no Tauri runtime — mirroring `drain_to_collected_set`. (The former P2.70
/// C13-during-modal `cancelled` branch is retired with C2a's walk — the picker now only fills the §7.8.1 funnel
/// and walks nothing, so the dialog wait has no walk to cancel, §0.4.1; only the C1 drain does.)
fn resolve_pick_outcome(picked_paths: Option<Vec<PathBuf>>) -> PickOutcome {
    match picked_paths {
        None => PickOutcome::DialogCancelled,
        Some(paths) => PickOutcome::Picked(paths),
    }
}

/// **C13 `cancel_ingest`** (§0.4.1) — trips the ingest-scoped `CollectingId` token to cancel an in-flight
/// C1 `drain_intake` walk **before** its long await resolves (§1.1): the frontend mints the `CollectingId`,
/// hands it to C1, and names it here to abort a deep recursive collect that would otherwise run to completion.
/// This box (P2.35) authors the typed §0.4.1 wire CONTRACT — the `{ collectingId } -> Result<(), IpcError>` door
/// (the §0.4 universal error shape) — so the generated `bindings.ts` mirrors the C13 surface.
///
/// - `collecting_id` — the §0.4.4 frontend-generated ingest-scoped cancel handle (registered by C1's drain at
///   handler entry, P3.49) whose token to trip.
///
/// [Build-Session-Entscheidung: P2.71] **The `.cancel()` trip is WIRED** (no longer the P2.35 `Ok(())`
/// shell). The handler binds an `AppHandle` (Tauri-injected — the §0.4.1 wire signature stays
/// `{ collectingId }`) to reach the §0.4.4 `IngestRegistry` (`.manage`d in main, P2.70) and **trips the
/// ingest-scoped token** via `IngestRegistry::cancel(collecting_id)`. The cancel EFFECT is then observed by
/// the in-flight C1 `drain_intake` walk — the §1.1 walk-loop poll (P2.69) returns `WalkAbort::Cancelled`,
/// yielding the §0.6 zero-collection `CollectedSet::Empty` (§1.1); C13's own return never carries the effect.
/// (The former C2a-during-modal leg is retired with C2a's walk — the picker now only fills the §7.8.1 funnel
/// and walks nothing, §0.4.1, so the dialog wait has no token to trip; only the C1 drain registers one, P3.49.)
/// **Idempotent `Ok(())` `[DECIDED]`:** a cancel of an unknown / already-finished ingest finds no live token
/// (`IngestRegistry::cancel` returns `false`) — the genuine "not collecting" end-state (§1.1), the C7
/// `cancel_run` mirror — so the result is ALWAYS `Ok(())` (the §0.4.1 C13 idempotent contract), NEVER an
/// `Err`. This handler is AppHandle-coupled boot-glue (§1.1a; G28 signature-exempt): the trip LOGIC is
/// `IngestRegistry::cancel` (unit-tested at P2.45, + the end-to-end token-trip chain proven in the C13 tests
/// here), the WIRING source-scan-pinned. `app.state::<…>()` is infallible by construction (the registry is
/// `.manage`d in main()'s Builder chain before the event loop, P2.70 — no panic under the `crate::ipc`
/// clippy::panic deny).
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn cancel_ingest(app: AppHandle, collecting_id: CollectingId) -> Result<(), IpcError> {
    // §1.1/§0.4.4 (P2.71): trip the ingest-scoped token so the in-flight C1 drain observes the cancel — the
    // §1.1 walk poll (P2.69) returns WalkAbort::Cancelled, yielding CollectedSet::Empty. Idempotent: a cancel of
    // an unknown/already-finished ingest finds no token (cancel returns false) — the genuine "not collecting"
    // no-op (§1.1) — so the result is ALWAYS Ok(()) (the §0.4.1 C13 contract), never an error.
    let _ = app.state::<IngestRegistry>().cancel(collecting_id);
    Ok(())
}

#[cfg(test)]
mod c1_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C1 `drain_intake` contract — the §7.8.1 ALWAYS-drain (P3.78; the P2.60
    //! `drainPending` flag is retired — draining `PendingIntake` IS the command). The handler binds an
    //! `AppHandle` (to reach the §7.8.1 `State<PendingIntake>` / `State<FrontendReady>`), so it is
    //! AppHandle-coupled boot-glue (the §1.1a pattern — NOT cargo-test-invocable; this crate ships no
    //! `tauri::test` mock BY DECISION). Its drain LOGIC lives in the `drain_to_collected_set` helper (taking
    //! `&State`-deref refs, not the `AppHandle`), unit-tested here with real `PendingIntake` / `FrontendReady`;
    //! the handler's WIRING (resolve the two States + dispatch via the helper) is source-scan-pinned. The
    //! §0.4.1 typed wire surface stays asserted by the bindings.ts golden (`bindings_codegen` in main.rs).
    //! [Build-Session-Entscheidung: P3.78]
    //!
    //! [Test-Change: P3.78 — old-obsolete+new-correct, §7.8.1/§0.4.1] the P2.60 `resolve_intake_source`
    //! passthrough / `drainPending` unit tests are OBSOLETE — C1 sheds `paths`/`origin`/`drainPending` entirely
    //! (every call drains, §0.4.1), so there is no passthrough branch and no `drainPending` flag to test. They
    //! are REPLACED by the `drain_to_collected_set` unit tests (the always-drain logic, read back via real
    //! `PendingIntake`/`FrontendReady` state) + the renamed handler source-scan — the sanctioned boot-glue
    //! stratification, NOT a dropped assertion.
    use super::*;

    // [Test-Change: P3.78 — old-obsolete+new-correct, §7.8.1/§0.4.1] (rationale in the module doc above) — the
    // P2.60 `resolve_intake_source` passthrough (`drain_pending = None` / `Some(false)`) tests are obsolete:
    // C1 has no `paths`/`origin`/`drainPending` args, so there is no passthrough branch. Replaced by the
    // `drain_to_collected_set` empty/non-empty drain tests below.
    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    // [Test-Change: P3.78 — old-obsolete+new-correct, §7.8.1/§0.4.1] replaces the obsolete P2.60
    // `resolve_intake_source` passthrough / `drain_pending = Some(false)` tests — C1 shed `paths`/`origin`/
    // `drainPending`, so every call drains and there is no passthrough branch left to test.
    // §6.4.1 unit (G15): a `drain_intake` call over an EMPTY `PendingIntake` returns the genuine zero-collection
    // `Empty` (a raced/duplicate drain, or the ordinary first launch with no files) AND marks the frontend
    // ready — the drain call IS the §7.8.1 root-shell-mount readiness signal, which fires even when no files
    // were buffered (§7.8.1 / §0.4.1).
    #[test]
    fn drain_of_empty_buffer_is_empty_and_marks_ready() {
        // [Test-Change: P3.78 — old-obsolete+new-correct, §7.8.1/§0.4.1] this + the non-empty test below fold in
        // the obsolete P2.60 `resolve_drain_false` / `resolve_drain_empty` `drainPending`-flag cases — the flag
        // is retired (every call drains, §0.4.1), so the flag-branch assertions have no counterpart.
        let pending = PendingIntake::default();
        let ready = FrontendReady::default();
        let out = drain_to_collected_set(&pending, &ready);
        assert_eq!(
            out,
            CollectedSet::Empty {
                skipped: Vec::new(),
            },
            "§7.8.1: a drain of an empty PendingIntake is the genuine zero-collection Empty (first launch, no files)"
        );
        // [Test-Change: P3.78 — old-obsolete+new-correct, §7.8.1/§0.4.1] folds in the obsolete P2.60
        // `resolve_drain_false` (drainPending=false passthrough) + `resolve_drain_empty` (drainPending=true drain)
        // cases — the `drainPending` flag is retired (every call drains, §0.4.1), so those flag branches are gone.
        assert!(
            ready.is_ready(),
            "§7.8.1: the drain call marks the frontend ready EVEN when the buffer is empty (the drain IS the readiness signal)"
        );
    }

    // [Test-Change: P3.78 — old-obsolete+new-correct, §7.8.1/§0.4.1] replaces the obsolete P2.60
    // `resolve_drain_empty` / `resolve_drain_nonempty` `drainPending`-flag tests — the flag is retired (every
    // call drains, §0.4.1); this asserts the same mark-ready + consume-once contract without the flag.
    // §6.4.1 unit (G15): a `drain_intake` call over a NON-empty `PendingIntake` freezes the BUFFERED set with
    // its STORED origin (§7.8.1 "using its stored origin"), marks ready, and consumes the buffer exactly once.
    // The §1.1 freeze funnel (`ingest`) is the interface shell until P3.49, so the returned `CollectedSet` is
    // the zero-collection `Empty`; the drain's OBSERVABLE contract here is mark-ready + consume-once +
    // route-through-the-freeze-funnel (pinned by the handler source-scan below; the P3.49 fill upgrades `ingest`).
    #[test]
    fn drain_of_nonempty_buffer_freezes_with_stored_origin_marks_ready_and_drains_once() {
        let pending = PendingIntake::default();
        let ready = FrontendReady::default();
        pending.stash(
            paths(&["/launch/x.png", "/launch/y.jpg"]),
            IntakeOrigin::LaunchArg,
        );
        let out = drain_to_collected_set(&pending, &ready);
        assert_eq!(
            out,
            CollectedSet::Empty {
                skipped: Vec::new(),
            },
            "§1.1: the drained buffer enters the freeze funnel (`ingest`, the interface shell until P3.49 → Empty)"
        );
        assert!(
            ready.is_ready(),
            "§7.8.1: the drain marks the frontend ready"
        );
        assert!(
            pending.take_marking_ready(&ready).is_none(),
            "§7.8.1: the drain consumed PendingIntake exactly once (the buffer is now empty)"
        );
    }

    /// The production prefix of `intake.rs` — everything before the FIRST `#[cfg(test)]` module (this one), so
    /// a sentinel needle declared here can never self-match the scan. Needle `concat!`-assembled so the literal
    /// `#[cfg(test)]` does not appear in this file's test source.
    fn production_intake_source() -> &'static str {
        let full = include_str!("intake.rs");
        full.split_once(concat!("#[cfg", "(test)]"))
            .map_or(full, |(prefix, _)| prefix)
    }

    // §6.4.1 unit (G15): the C1 `drain_intake` handler is AppHandle-coupled boot-glue — a source-scan pins it
    // binds an `AppHandle`, resolves the two §7.8.1 States, and DISPATCHES via `drain_to_collected_set` (the
    // testable always-drain logic), rather than a WebView-supplied-paths passthrough. The dispatch needle
    // carries the call-site args (`&pending, &ready`) so it matches the CALL, not merely the fn definition
    // (non-blind). Needles `concat!`-assembled (self-match avoidance). [Build-Session-Entscheidung: P3.78]
    #[test]
    fn drain_intake_handler_dispatches_via_the_drain_helper() {
        let src = production_intake_source();
        for needle in [
            concat!("pub async fn drain_", "intake("),
            concat!("app: App", "Handle"),
            concat!("state::<Pending", "Intake>()"),
            concat!("state::<Frontend", "Ready>()"),
            concat!("drain_to_collected_", "set(&pending, &ready"),
        ] {
            assert!(
                src.contains(needle),
                "§7.8.1/§0.4.1: the C1 drain_intake handler must bind an AppHandle, resolve the §7.8.1 States, \
                 and dispatch via drain_to_collected_set (missing `{needle}`)"
            );
        }
    }
}

#[cfg(test)]
mod c2a_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C2a `pick_for_intake` fill-the-funnel phase (P2.70, reshaped P3.78). The
    //! handler binds an `AppHandle` (to open the native `DialogExt` picker + reach the §7.8.1 funnel), so it is
    //! AppHandle-coupled boot-glue (the §1.1a pattern — NOT cargo-test-invocable; this crate ships no
    //! `tauri::test` mock BY DECISION, G28 signature-exempt). Its post-dialog OUTCOME logic lives in the
    //! AppHandle-free `resolve_pick_outcome` helper, unit-tested here; the handler's WIRING (open the dialog on
    //! a blocking thread, funnel the picked set) is source-scan-pinned. The §0.4.1 typed wire surface stays
    //! asserted by the bindings.ts golden (`bindings_codegen` in main.rs). [Build-Session-Entscheidung: P3.78]
    //!
    //! [Test-Change: P3.78 — old-obsolete+new-correct, §1.1/§0.4.1] C2a sheds `collectingId`/`onScan` and no
    //! longer registers an ingest token (the picker walks nothing — the walk/token/`onScan` all live on C1
    //! `drain_intake`, §0.4.1). So the P2.70 `resolve_pick_outcome(.., cancelled)` C13-during-modal test + the
    //! `c2a_handler_registers_the_token_before_opening_the_dialog` order-scan are OBSOLETE and removed; the
    //! remaining post-dialog decision (dismiss → no-op, pick → funnel) + the `spawn_blocking` offload scan stay.
    use super::*;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    /// The production prefix of `intake.rs` (everything before the FIRST `#[cfg(test)]`), so a needle declared
    /// in this test can never self-match — mirroring the `c1_contract` helper (each contract module keeps its
    /// own copy, the established per-module test-helper pattern).
    fn production_intake_source() -> &'static str {
        let full = include_str!("intake.rs");
        full.split_once(concat!("#[cfg", "(test)]"))
            .map_or(full, |(prefix, _)| prefix)
    }

    // [Test-Change: P3.78 — old-obsolete+new-correct, §1.1/§0.4.1] replaces the obsolete P2.70
    // `resolve_pick_outcome_abandons_on_a_c13_trip` test — the `cancelled`-first argument is retired with the
    // C13-during-modal leg (the picker walks nothing, only fills the §7.8.1 funnel, §0.4.1), so
    // `resolve_pick_outcome` takes only the picked paths now.
    // §6.4.1 unit (G15): `resolve_pick_outcome` — a user-DISMISSED dialog (`None`) is a clean no-op →
    // `DialogCancelled` (→ nothing buffered, no nudge, the UI stays Idle, §5.4).
    #[test]
    fn resolve_pick_outcome_dialog_cancelled_when_user_dismisses() {
        assert_eq!(
            resolve_pick_outcome(None),
            PickOutcome::DialogCancelled,
            "§5.4: a user-dismissed dialog (None) is a clean no-op → nothing buffered, no nudge"
        );
    }

    // §6.4.1 unit (G15): `resolve_pick_outcome` happy path — a pick funnels the picked paths (Picker-stamped at
    // the handler, then routed through the §7.8.1 funnel).
    #[test]
    fn resolve_pick_outcome_picked_when_paths() {
        assert_eq!(
            resolve_pick_outcome(Some(paths(&["/picked/a.png", "/picked/b.jpg"]))),
            PickOutcome::Picked(paths(&["/picked/a.png", "/picked/b.jpg"])),
            "§1.1: a successful pick funnels the picked paths through the §7.8.1 funnel"
        );
    }

    // [Test-Change: P3.78 — old-obsolete+new-correct, §0.4.1] the P2.70
    // `c2a_handler_registers_the_token_before_opening_the_dialog` order-scan is removed — C2a sheds
    // `collectingId` and no longer registers an ingest token (only the C1 drain registers one, §0.4.1); the
    // `spawn_blocking` offload scan below stays.
    // §6.4.1 unit (G15): the native picker opens on a DEDICATED BLOCKING THREAD (spawn_blocking +
    // blocking_pick_*), never a synchronous blocking_pick_* on a Tokio worker (§1.1: the runtime stays free).
    // Needles concat!-assembled.
    #[test]
    fn c2a_handler_opens_the_dialog_off_the_async_runtime() {
        let src = production_intake_source();
        for needle in [
            concat!("spawn_", "blocking(move"),
            concat!("blocking_pick_", "files"),
            concat!("blocking_pick_", "folder"),
        ] {
            assert!(
                src.contains(needle),
                "§1.1/P2.70: the C2a dialog opens on a blocking thread (spawn_blocking + blocking_pick_*), never a Tokio worker (missing `{needle}`)"
            );
        }
    }

    // §6.4.1 unit (G15): the C2a handler dispatches the post-dialog decision via the AppHandle-free
    // `resolve_pick_outcome`, then on the happy branch stamps the §1.1 `Picker` origin CORE-SIDE and funnels
    // the picked set through the SAME §7.8.1 funnel every other intake source uses
    // (`crate::launch_intake::forward_launch_intake`) — the §1.1 anti-origin-forgery property (a compromised
    // WebView cannot forge the intake origin, §5.4 / §0.10) + the single-funnel uniformity (§7.8.1). Needles
    // concat!-assembled. [Build-Session-Entscheidung: P2.63/P3.78]
    #[test]
    fn c2a_handler_stamps_picker_and_funnels_through_the_intake_funnel() {
        let src = production_intake_source();
        assert!(
            src.contains(concat!("resolve_pick_", "outcome(picked_paths)")),
            "§1.1/P3.78: the handler dispatches the post-dialog decision via the AppHandle-free resolve_pick_outcome"
        );
        assert!(
            src.contains(concat!(
                "forward_launch_",
                "intake(&app, picked_paths, IntakeOrigin::Picker)"
            )),
            "§1.1/§7.8.1: the C2a handler stamps Picker core-side and funnels the picked set through forward_launch_intake"
        );
    }
}

#[cfg(test)]
mod c13_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C13 `cancel_ingest` `.cancel()` trip wiring (P2.71). The handler binds
    //! an `AppHandle` (to reach the §0.4.4 `IngestRegistry` and trip the token), so it is AppHandle-coupled
    //! boot-glue (the §1.1a pattern — NOT cargo-test-invocable; this crate ships no `tauri::test` mock BY
    //! DECISION, G28 signature-exempt). The trip LOGIC is `IngestRegistry::cancel` (unit-tested at P2.45); this
    //! module pins the handler WIRING by source-scan + proves the END-TO-END token-trip chain at the
    //! registry/guard level (no runtime). [Build-Session-Entscheidung: P2.71]
    //!
    //! [Test-Change: P3.78 — old-obsolete+new-correct, §0.4.1] the former end-to-end test routed the trip
    //! through the C2a pick's `resolve_pick_outcome(.., cancelled)` — the C2a-during-modal leg is RETIRED with
    //! C2a's walk (the picker now only fills the §7.8.1 funnel and walks nothing, §0.4.1), so that coupling is
    //! obsolete. It is REPLACED by the registry-level trip mechanism (register → cancel → is_cancelled) — what
    //! C13 does and what the C1 `drain_intake` walk-loop (P3.49) polls — asserted directly.
    use super::*;

    /// The production prefix of `intake.rs` (everything before the FIRST `#[cfg(test)]`), so a needle declared
    /// here can never self-match — mirroring the `c1_contract`/`c2a_contract` helpers.
    fn production_intake_source() -> &'static str {
        let full = include_str!("intake.rs");
        full.split_once(concat!("#[cfg", "(test)]"))
            .map_or(full, |(prefix, _)| prefix)
    }

    /// A `CollectingId` for the registry-level end-to-end test — its PUBLIC bare-uuid wire form (the frontend
    /// mints the ingest id, §0.4.4), mirroring the sibling helpers.
    fn collecting_id() -> CollectingId {
        serde_json::from_str(r#""44444444-4444-4444-8444-444444444444""#)
            .expect("CollectingId deserializes from a uuid string")
    }

    // [Test-Change: P3.78 — old-obsolete+new-correct, §0.4.1] rewritten from the P2.71 end-to-end test that
    // routed the trip through the C2a pick's `resolve_pick_outcome(.., cancelled)` — that C2a-during-modal leg is
    // retired (the picker walks nothing, §0.4.1); the registry-level trip mechanism (register → cancel →
    // is_cancelled) is what C13 does and the C1 drain observes, asserted directly.
    // §6.4.1 unit (G15): the END-TO-END C13-tripped→observed chain (P2.71), proven at the registry/guard level
    // with NO Tauri runtime. The C1 `drain_intake` walk registers its ingest token (register_guard, P3.49); a
    // C13 cancel trips it (`IngestRegistry::cancel` — the wiring this handler adds); the in-flight walk then
    // reads the tripped token (`guard.is_cancelled()`) and stops cooperatively, discarding its partial set →
    // `CollectedSet::Empty` (§1.1). This is the live, reachable C13-tripped→observed mechanism the C1 drain's
    // P3.49 walk-loop polls.
    #[test]
    fn c13_cancel_trips_the_ingest_token_so_the_in_flight_drain_observes_it() {
        let registry = IngestRegistry::default();
        // The C1 drain registered its ingest token at handler entry (P3.49):
        let guard = registry.register_guard(collecting_id());
        // C13 cancel_ingest trips it (the wiring P2.71 adds to the handler: app.state::<IngestRegistry>().cancel):
        assert!(
            registry.cancel(collecting_id()),
            "§0.4.4: C13 finds the in-flight ingest's token and trips it"
        );
        // The in-flight walk's cooperative-cancel poll (P2.69/P3.49) now reads the tripped token and stops.
        // [Test-Change: P3.78 — old-obsolete+new-correct, §0.4.1] the former end-to-end test appended a
        // `resolve_pick_outcome(.., cancelled)` assert here (the C2a-during-modal leg, retired with C2a's walk,
        // §0.4.1); the registry-level trip → observe chain is the mechanism C13 drives and the C1 drain polls.
        assert!(
            guard.is_cancelled(),
            "§1.1: the in-flight drain observes the C13 trip (the walk-loop poll reads it → discards the partial set → Empty)"
        );
    }

    // §6.4.1 unit (G15): the C13 handler binds an AppHandle and TRIPS the token via `IngestRegistry::cancel`
    // (the P2.71 wiring), no longer the P2.35 `let _ = collecting_id; Ok(())` shell. Source-scan (AppHandle
    // boot-glue, not cargo-test-runnable; the cancel LOGIC is unit-tested on `IngestRegistry` at P2.45). Needles
    // `concat!`-assembled (self-match avoidance); the literal call forms appear only in the handler body, never
    // the doc prose.
    #[test]
    fn c13_handler_trips_the_ingest_token_via_the_registry() {
        let src = production_intake_source();
        for needle in [
            concat!("cancel_ingest(app: App", "Handle"),
            concat!("state::<Ingest", "Registry>().cancel(collecting_id)"),
        ] {
            assert!(
                src.contains(needle),
                "§0.4.4/§1.1: cancel_ingest must bind an AppHandle and trip the token via IngestRegistry::cancel (missing `{needle}`)"
            );
        }
    }
}
