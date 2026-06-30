//! `crate::ipc::intake` — the §0.4.1 intake command group (C1 / C2a / C13): the single §2.4 freeze point
//! for every intake origin (drop / picker / launch-arg) and the ingest-scoped cancel. P2.21 registered
//! these as the §0.4.1 command-surface interface shells; C1's typed request/response CONTRACT is authored
//! by P2.22 and C2a's by P2.23 (this file), C13's by P2.35. Each command's `crate::orchestrator` freeze
//! BODY is its own named fill-box (the C1 freeze funnel is P2.62; C2a's native-dialog pick is P2.70/P2.71 +
//! the `Picker`-origin stamp P2.63; the end-to-end walking-skeleton wiring is P3.49). Thin by design (§0.7):
//! the handler validates, delegates, and maps the `Result` onto §0.4.3 `IpcError`.

// §0.4 / T10: unchecked arithmetic on an untrusted wire field must be a compile error in every IPC handler
// (the `crate::ipc` arithmetic-overflow deny cascades here; restated at the T10 boundary so this group's
// real handlers carry it explicitly). The C1 contract handler below does no arithmetic; the deny bites the
// remaining fill-bodies (P2.23/P2.35 + P3.49).
#![deny(clippy::arithmetic_side_effects)]

use std::path::PathBuf;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::domain::{CollectedSet, CollectingId, IntakeOrigin, PickKind, ScanProgress};
use crate::orchestrator::{ingest, FrontendReady, IngestRegistry, PendingIntake};
use crate::outcome::{ConversionErrorKind, IpcError};

/// **C1 `ingest_paths`** (§0.4.1) — the single §2.4 freeze point for every intake origin (drop / picker /
/// launch-arg / second-instance). This box (P2.22) authors the typed §0.4.1 wire CONTRACT — the
/// `{ paths, origin, collectingId, drainPending?, onScan } -> CollectedSet` door — so the generated
/// `bindings.ts` mirrors the C1 surface, pulling the whole `CollectedSet` graph + `IntakeOrigin` +
/// `ScanProgress` into the bindings as named types (the §0.6 defer-registration-to-the-consumer pattern).
///
/// - `paths` — the absolute FS paths to freeze (a drop / launch-arg / second-instance set; empty for the
///   `drain_pending` first-launch drain).
/// - `origin` — how the set entered intake (§7.8) — the §0.6 `IntakeOrigin`; a drop/launch-arg/second-instance
///   carries its origin here, while C2a's handler stamps `Picker` itself (§1.1).
/// - `collecting_id` — the frontend-generated ingest-scoped cancel handle (§0.4.4) so C13 `cancel_ingest`
///   can name this in-flight walk **before** C1's long await resolves (§1.1).
/// - `drain_pending` — `Some(true)` consumes the §7.8.1 first-launch `PendingIntake` buffer instead of
///   `paths` (mutually exclusive with a non-empty `paths`, §0.4.1); `None` / `Some(false)` = a normal intake.
///   Wire form `drainPending: boolean | null` (tauri-specta's `Option<bool>` arg form): the frontend passes
///   `null` for a normal intake, `true` to drain — `null` ≡ the spec's omittable `drainPending?` (serde maps
///   a missing/`null` `Option` arg to `None`).
/// - `on_scan` — the throttled scan-telemetry Channel (§0.4.2 `ScanProgress`, ≈2/s) driving the §5.2
///   *Collecting* "Scanning… N files" count; best-effort, monotonic, dies with the call. **Always passed**
///   (non-optional — see the forced-deviation note below); the frontend realises the §0.4.1 "optional" intent
///   by subscribing only for a long walk, never by omitting the argument.
///
/// [Build-Session-Entscheidung: P2.22] **The typed CONTRACT is the P2.22 deliverable.** P2.22 authors the
/// §0.4.1 wire signature above so the generated `bindings.ts` carries the full C1 door. The §2.4 freeze BODY
/// is its own set of named, scheduled boxes — the §1.1 recursive walk → §1.2 detect → §2.3 de-dup → §1.3
/// group freeze funnel (P2.62), the §0.4.4 `collecting_id` token registry (P2.45), and the `on_scan`
/// scan-telemetry emit (the throttled §0.4.2 `ScanProgress` count, part of the §1.1 walk P2.62/P2.64) — wired
/// end-to-end into this handler by P3.49 "Implement C1 `ingest_paths`" (the CSV→TSV walking-skeleton slice).
/// This is the sanctioned compile-time interface-shell pattern (CLAUDE §5 / the P3 `crate::isolation` shells
/// P4 expands), NOT a quiet deferral: a freeze seam that collects nothing returns the §0.6 zero-collection
/// `CollectedSet::Empty { skipped: [] }` until P3.49 fills it. The freeze funnel is now homed in
/// `crate::orchestrator::ingest` (P2.62 — the §0.7 §01-conductor's first act; no §0.7 tree edit, since
/// `orchestrator` already homes it like the §7.8.1 `PendingIntake`/`FrontendReady` machinery), so it was
/// not pre-created here.
///
/// [Build-Session-Entscheidung: P2.60] **The §7.8.1 `drainPending` drain dispatch is now WIRED** (no longer
/// an ignore-all-args shell). The handler binds an `AppHandle` (a Tauri-injected arg, NOT part of the §0.4.1
/// wire signature — the generated C1 command signature is unchanged) to reach the Rust-side `State<PendingIntake>` +
/// `State<FrontendReady>` (P2.58/P2.59), and dispatches via the pure-state `resolve_intake_source` helper:
/// a `drainPending: true` call MARKS the frontend ready (the §7.8.1 root-shell-mount readiness signal) and
/// CONSUMES `PendingIntake` exactly once with its stored origin (empty buffer → `CollectedSet::Empty`, the
/// ordinary first launch with no files); a normal intake passes its `paths` + `origin` through. The drained /
/// passed-through `Freeze` source then enters the SAME §1.1/§2.4 freeze seam above (the shell until P3.49) —
/// P2.60 owns the drain DISPATCH, P2.62 builds the freeze funnel, P3.49 wires it end-to-end. The `AppHandle`
/// makes this handler AppHandle-coupled boot-glue (§1.1a; G28 signature-exempt): its drain LOGIC is
/// unit-tested on `resolve_intake_source`, its WIRING is source-scan-pinned (this crate ships no `tauri::test`
/// mock BY DECISION). `collecting_id`/`on_scan` remain shell-accepted (`_`-bound) until P2.45/P3.49.
///
/// [Build-Session-Entscheidung: P2.22] **`on_scan` is NON-OPTIONAL — a FORCED deviation from the §0.4.1
/// `onScan?` `[DECIDED]`.** tauri 2.11.3's `Channel<T>` is `!Deserialize` (it carries its own `CommandArg`
/// impl, but `Option<Channel<T>>` routes through the `CommandArg for D: Deserialize` blanket impl → E0277),
/// so an optional channel argument cannot compile. No behaviour is lost: the wire-form optionality is realised
/// by the frontend subscribing only for a long walk, never by omitting the arg. The rejected alternative was a
/// custom `OptionalChannel<T>` wrapper replicating undocumented `__CHANNEL__:N` internals (version-fragile).
/// §0.4.1 (C1 + the sibling C2a) / §0.4.2 + the README / plan / §05 mirrors are spec-synced non-optional in
/// THIS same commit (DoD item 2 — the forced deviation is reflected in the spec, not silently absorbed).
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn ingest_paths(
    app: AppHandle,
    paths: Vec<PathBuf>,
    origin: IntakeOrigin,
    collecting_id: CollectingId,
    drain_pending: Option<bool>,
    on_scan: Channel<ScanProgress>,
) -> Result<CollectedSet, IpcError> {
    // §7.8.1 drain dispatch (P2.60): resolve what the §1.1/§2.4 freeze seam consumes. `app.state::<…>()` is
    // infallible by construction — both stores are `.manage()`'d in main()'s Builder chain before the event
    // loop (P2.58/P2.59), so no panic under the `crate::ipc` clippy::panic deny. `&State<T>` deref-coerces to
    // the `&T` the pure helper takes, keeping the AppHandle resolution in this thin (G28-exempt) wrapper.
    let pending = app.state::<PendingIntake>();
    let ready = app.state::<FrontendReady>();
    match resolve_intake_source(&pending, &ready, drain_pending, paths, origin) {
        // §7.8.1: a `drainPending` call whose `PendingIntake` was empty (the ordinary first launch with no
        // files) — the genuine §0.6 zero-collection result; the UI stays Idle (§0.4.1).
        ResolvedIntake::Empty => Ok(CollectedSet::Empty {
            skipped: Vec::new(),
        }),
        // The §1.1/§2.4 freeze seam — the single freeze funnel (P2.62), wired end-to-end into this handler by
        // P3.49 (the CSV→TSV walking-skeleton slice). Until then the seam collects nothing, so a freezable
        // source (a normal intake OR a non-empty drain) returns the shell's zero-collection
        // `CollectedSet::Empty`; P2.60 owns the drain DISPATCH that selects this seam, not the freeze body.
        ResolvedIntake::Freeze { paths, origin } => {
            let _ = (paths, origin, collecting_id, on_scan);
            Ok(CollectedSet::Empty {
                skipped: Vec::new(),
            })
        }
    }
}

/// The C1 intake source after the §7.8.1 drain dispatch (P2.60) — what the §1.1/§2.4 freeze seam consumes.
/// INTERNAL (not a wire type): the handler maps it onto the §0.4.1 `CollectedSet` return.
/// [Build-Session-Entscheidung: P2.60]
#[derive(Debug, PartialEq, Eq)]
enum ResolvedIntake {
    /// Freeze this set with its origin — a normal intake, or a non-empty §7.8.1 first-launch drain (whose
    /// paths + stored origin come from the `PendingIntake` buffer, not the passed args).
    Freeze {
        paths: Vec<PathBuf>,
        origin: IntakeOrigin,
    },
    /// Nothing to freeze → the genuine §0.6 zero-collection `CollectedSet::Empty`: a `drainPending` call whose
    /// `PendingIntake` buffer was empty (the ordinary first launch with no files, §7.8.1).
    Empty,
}

/// [Build-Session-Entscheidung: P2.60] The §7.8.1 / §0.4.1 C1 drain dispatch — resolve what the §1.1 freeze
/// seam consumes, handling the first-launch `drainPending` call. When `drain_pending == Some(true)` (the
/// frontend's root-shell-mount drain, fired AFTER it registered its `app://intake` listener) two cohesive
/// effects run: (1) MARK the frontend ready — the drain call IS the §7.8.1 readiness signal, so a subsequent
/// launch intake EMITs `app://intake` instead of buffering (P2.59's `FrontendReady`) — and (2) CONSUME
/// `PendingIntake` exactly once (§7.8.1 "consumes `PendingIntake` exactly once"), freezing THAT buffered set
/// with its STORED origin (typically `LaunchArg`), or — if the buffer is empty — returning `Empty` (the
/// ordinary first launch with no files → `CollectedSet::Empty`, §0.4.1). A normal intake (`drain_pending`
/// `None` / `Some(false)`) passes its own `paths` + `origin` through unchanged and NEVER marks ready
/// (readiness is the drain's signal alone). `drainPending` and a non-empty `paths` are mutually exclusive
/// (§0.4.1 C1): a drain IGNORES the passed `paths`. Takes `&PendingIntake` / `&FrontendReady` (NOT the
/// `AppHandle`) so it is fully unit-testable with real state — the AppHandle resolution stays in the thin
/// command wrapper (the §1.1a boot-glue split, mirroring main.rs's `intake_disposition`).
fn resolve_intake_source(
    pending: &PendingIntake,
    ready: &FrontendReady,
    drain_pending: Option<bool>,
    paths: Vec<PathBuf>,
    origin: IntakeOrigin,
) -> ResolvedIntake {
    if drain_pending == Some(true) {
        ready.mark_ready();
        match pending.take() {
            Some(buffered) => ResolvedIntake::Freeze {
                paths: buffered.paths,
                origin: buffered.origin,
            },
            None => ResolvedIntake::Empty,
        }
    } else {
        ResolvedIntake::Freeze { paths, origin }
    }
}

/// **C2a `pick_for_intake`** (§0.4.1) — the Rust-side `DialogExt` intake picker. This box (P2.23) authors the
/// typed §0.4.1 wire CONTRACT — the `{ kind, collectingId, onScan } -> CollectedSet` door — mirroring the C1
/// surface so the picker shares C1's freeze return and **no raw FS path ever reaches the WebView** (the WebView
/// only triggers the picker and receives the collected summary, §0.10 / §5.4).
///
/// - `kind` — the §0.6 `PickKind` (`Files` | `Folder`): open the native files-multiselect or the folder
///   dialog; a folder pick is recursively collected at the §1.1 freeze.
/// - `collecting_id` — the frontend-generated ingest-scoped cancel handle (§0.4.4), registered as the §1.1
///   token **before the dialog opens** so C13 `cancel_ingest` can trip the in-flight pick/walk (P2.70).
/// - `on_scan` — the throttled scan-telemetry Channel (§0.4.2 `ScanProgress`); **non-optional**, the *same*
///   `Channel<T>` `!Deserialize` forced deviation the C1 `ingest_paths` handler documents above — C2a takes it
///   identically (§0.4.1). The frontend always hands it and realises the "optional" intent by subscribing only
///   for a long walk, never by omitting the argument.
///
/// C2a carries **no `origin` field**: the picked set's origin is `Picker`, **stamped by this handler itself**
/// (P2.63), not supplied by the WebView (§1.1 / §5.4) — so a compromised WebView cannot forge the intake origin.
///
/// [Build-Session-Entscheidung: P2.70] **Native-dialog phase — the body is now built.** The handler binds an
/// `AppHandle` (a Tauri-injected arg, NOT part of the §0.4.1 wire signature — the generated C2a command stays
/// `{ kind, collectingId, onScan }`) to open the native `DialogExt` picker and reach the §0.4.4
/// `IngestRegistry`. Per §1.1: it registers the `CollectingId` token **before** the dialog opens — via the
/// RAII `IngestGuard` (P2.70), so the token is de-registered on **every** exit branch by construction (the
/// §1.1 "drop in every C2a return path"; the explicit per-branch + the C13 `.cancel()` trip are P2.71) — then
/// opens the picker on a **dedicated blocking thread** (`spawn_blocking` + `blocking_pick_*`, never a
/// synchronous `blocking_pick_*` on a Tokio worker), so the runtime stays free and a C13 during the modal is
/// serviceable. After the dialog it runs the **AppHandle-free `resolve_pick_outcome`** decision (§1.1a split,
/// unit-tested): a C13 trip during the modal **abandons** the pick → `Empty`; a user-dismissed dialog →
/// `Empty` (§5.4 clean no-op); otherwise the picked paths are **`Picker`-stamped core-side** (a compromised
/// WebView cannot forge the origin, §5.4 / §0.10) and funnelled into the single §1.1/§2.4 freeze
/// (`crate::orchestrator::ingest`, the interface shell until P3.49 — so today the funnel still yields `Empty`,
/// but the dialog→funnel path is real). The post-dialog token check is **live** (until P2.71 wires C13's
/// `.cancel()` it reads `false` — reachable-by-construction, no hole). This handler is AppHandle-coupled
/// boot-glue (§1.1a; G28 signature-exempt): the dialog open + the token registration are source-scan-pinned,
/// the outcome decision is `resolve_pick_outcome` (unit-tested + G27-counted). `on_scan` belongs to the §1.1
/// walk (the freeze funnel, P3.49), not the dialog phase, so it stays `_`-bound here.
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn pick_for_intake(
    app: AppHandle,
    kind: PickKind,
    collecting_id: CollectingId,
    on_scan: Channel<ScanProgress>,
) -> Result<CollectedSet, IpcError> {
    // §1.1 (P2.70): register the CollectingId token BEFORE the dialog opens, via the RAII guard so it is
    // de-registered on EVERY exit branch (drop-by-construction; no branch can leak it). Registering before the
    // modal keeps C13 honest — a cancel_ingest arriving while the dialog is up trips this token (its .cancel()
    // wiring is P2.71), read back post-dialog via guard.is_cancelled().
    let registry = app.state::<IngestRegistry>();
    let guard = registry.register_guard(collecting_id);

    // §1.1 (P2.70): open the native picker on a DEDICATED BLOCKING THREAD (spawn_blocking), never a synchronous
    // blocking_pick_* on a Tokio worker — so the async runtime stays free and C13 remains serviceable while the
    // modal is up. A spawn_blocking failure (the dialog thread panicked — should-never-happen) surfaces as an
    // InternalError, never a silent no-op.
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
        path: None,
        residue: None,
    })?;

    // FilePath -> PathBuf (a desktop pick is always a real path; drop a non-path entry defensively). This
    // conversion is the handler's boot-glue; the §1.1a pure decision is resolve_pick_outcome.
    let picked_paths: Option<Vec<PathBuf>> = picked.map(|files| {
        files
            .into_iter()
            .filter_map(|f| f.into_path().ok())
            .collect()
    });

    let _ = on_scan;
    // §1.1 (P2.70): the AppHandle-FREE post-dialog decision (§1.1a split, unit-tested). guard.is_cancelled() is
    // LIVE (false until P2.71 wires C13's trip — no hole). The guard drops at fn end on EVERY branch below,
    // de-registering the token.
    match resolve_pick_outcome(picked_paths, guard.is_cancelled()) {
        // §1.1/§5.4: a C13 trip during the modal OR a user-dismissed dialog → the genuine zero-collection
        // Empty (the UI stays Idle), no error.
        PickOutcome::Cancelled | PickOutcome::DialogCancelled => Ok(CollectedSet::Empty {
            skipped: Vec::new(),
        }),
        // §1.1: the happy path — stamp Picker core-side + funnel the picked set into the single §1.1/§2.4
        // freeze (`ingest`, the interface shell until P3.49).
        PickOutcome::Picked(picked_paths) => Ok(ingest(picked_paths, IntakeOrigin::Picker)),
    }
}

/// The C2a post-dialog outcome (§1.1a split, P2.70) — AppHandle-FREE so it is unit-tested + G27-counted (the
/// dialog open + the token registration are the AppHandle-coupled boot-glue, source-scan-pinned). INTERNAL:
/// the handler maps it onto the §0.4.1 `CollectedSet` return. [Build-Session-Entscheidung: P2.70]
#[derive(Debug, PartialEq, Eq)]
enum PickOutcome {
    /// A C13 `cancel_ingest` tripped the ingest token while the dialog was up (§1.1) → the picked paths are
    /// ABANDONED → `CollectedSet::Empty`. Distinct from `DialogCancelled` so the §1.1 per-branch matrix is
    /// explicit (P2.71); both map to `Empty`.
    Cancelled,
    /// The user dismissed the native dialog (§5.4 clean no-op) → `CollectedSet::Empty`, the UI stays Idle.
    DialogCancelled,
    /// The user picked these paths → stamp `Picker` and funnel them into the §1.1/§2.4 freeze.
    Picked(Vec<PathBuf>),
}

/// [Build-Session-Entscheidung: P2.70] The §1.1 C2a post-dialog decision — the §1.1a pure half of
/// `pick_for_intake`. Given the dialog's picked paths (`None` = the user dismissed it) and whether a C13
/// tripped the ingest token DURING the modal (`cancelled`), decide the outcome. Two §1.1 rules: (1) a C13 trip
/// ABANDONS the result even if the user had already picked — §1.1: the handler "checks the token after the
/// dialog returns and yields Empty rather than walking the picked paths" — so `cancelled` is tested FIRST;
/// (2) a user-dismissed dialog is a clean no-op (§5.4). Otherwise the picked paths funnel (Picker-stamped).
/// Takes already-converted `PathBuf`s (the `FilePath` -> `PathBuf` conversion is the handler's boot-glue), so
/// it is fully unit-testable with no Tauri runtime — mirroring `resolve_intake_source`.
fn resolve_pick_outcome(picked_paths: Option<Vec<PathBuf>>, cancelled: bool) -> PickOutcome {
    if cancelled {
        return PickOutcome::Cancelled;
    }
    match picked_paths {
        None => PickOutcome::DialogCancelled,
        Some(paths) => PickOutcome::Picked(paths),
    }
}

/// **C13 `cancel_ingest`** (§0.4.1) — trips the ingest-scoped `CollectingId` token to cancel an in-flight
/// C1/C2a walk **before** its long await resolves (§1.1): the frontend mints the `CollectingId`, hands it to
/// C1/C2a, and names it here to abort a deep recursive collect that would otherwise run to completion. This
/// box (P2.35) authors the typed §0.4.1 wire CONTRACT — the `{ collectingId } -> Result<(), IpcError>` door
/// (the §0.4 universal error shape) — so the generated `bindings.ts` mirrors the C13 surface.
///
/// - `collecting_id` — the §0.4.4 frontend-generated ingest-scoped cancel handle (registered by C1/C2a at
///   handler entry, P2.45) whose token to trip.
///
/// [Build-Session-Entscheidung: P2.71] **The `.cancel()` trip is now WIRED** (no longer the P2.35 `Ok(())`
/// shell). The handler binds an `AppHandle` (Tauri-injected — the §0.4.1 wire signature stays
/// `{ collectingId }`) to reach the §0.4.4 `IngestRegistry` (`.manage`d in main, P2.70) and **trips the
/// ingest-scoped token** via `IngestRegistry::cancel(collecting_id)`. The cancel EFFECT is then observed by
/// the in-flight ingest — the §1.1 walk-loop poll (P2.69) returns `WalkAbort::Cancelled`, or the C2a
/// post-dialog check (P2.70) reads the tripped token via its RAII guard — each yielding the §0.6
/// zero-collection `CollectedSet::Empty` (§1.1); C13's own return never carries the effect. **Idempotent
/// `Ok(())` `[DECIDED]`:** a cancel of an unknown / already-finished ingest finds no live token
/// (`IngestRegistry::cancel` returns `false`) — the genuine "not collecting" end-state (§1.1), the C7
/// `cancel_run` mirror — so the result is ALWAYS `Ok(())` (the §0.4.1 C13 idempotent contract), NEVER an
/// `Err`. This handler is AppHandle-coupled boot-glue (§1.1a; G28 signature-exempt): the trip LOGIC is
/// `IngestRegistry::cancel` (unit-tested at P2.45, + the end-to-end token-trip→guard→`Empty` chain proven in
/// the C13 tests here), the WIRING source-scan-pinned. `app.state::<…>()` is infallible by construction (the
/// registry is `.manage`d in main()'s Builder chain before the event loop, P2.70 — no panic under the
/// `crate::ipc` clippy::panic deny).
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn cancel_ingest(app: AppHandle, collecting_id: CollectingId) -> Result<(), IpcError> {
    // §1.1/§0.4.4 (P2.71): trip the ingest-scoped token so the in-flight C1/C2a ingest observes the cancel —
    // the §1.1 walk poll (P2.69) returns WalkAbort::Cancelled, or the C2a post-dialog guard check (P2.70) reads
    // it — each yielding CollectedSet::Empty. Idempotent: a cancel of an unknown/already-finished ingest finds
    // no token (cancel returns false) — the genuine "not collecting" no-op (§1.1) — so the result is ALWAYS
    // Ok(()) (the §0.4.1 C13 contract), never an error.
    let _ = app.state::<IngestRegistry>().cancel(collecting_id);
    Ok(())
}

#[cfg(test)]
mod c1_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C1 `ingest_paths` contract + the §7.8.1 `drainPending` drain dispatch
    //! (P2.60). The handler gained an `AppHandle` (to reach the §7.8.1 `State<PendingIntake>` /
    //! `State<FrontendReady>`), so it is now AppHandle-coupled boot-glue (the §1.1a pattern — NOT
    //! cargo-test-invocable; this crate ships no `tauri::test` mock BY DECISION). Its drain LOGIC lives in the
    //! `resolve_intake_source` helper (taking `&State`-deref refs, not the `AppHandle`), unit-tested here with
    //! real `PendingIntake` / `FrontendReady`; the handler's WIRING (resolve the two States + dispatch via the
    //! helper) is source-scan-pinned. The §0.4.1 typed wire surface stays asserted by the bindings.ts golden
    //! (`bindings_codegen` in main.rs). [Build-Session-Entscheidung: P2.60]
    //!
    //! [Test-Change: P2.60 — old-obsolete+new-correct, §7.8.1/§0.4.1] the P2.22 direct-`block_on(ingest_paths(
    //! …))` contract test is OBSOLETE — the handler now binds an `AppHandle`, so it is uninvocable without a
    //! Tauri runtime (none in cargo-test, §1.1a). It is REPLACED by the executable `resolve_intake_source`
    //! unit tests (the real drain logic, read back via real `PendingIntake`/`FrontendReady` state) + the
    //! handler source-scan — the sanctioned boot-glue stratification, NOT a dropped assertion.
    use super::*;

    // [Test-Change: P2.60 — old-obsolete+new-correct, §7.8.1/§0.4.1] (rationale in the module doc above) —
    // the removed P2.22 `collecting_id()` helper (its `.expect`) + the direct-`block_on(ingest_paths)`
    // contract test are obsolete: the handler now binds an `AppHandle`, uninvocable without a Tauri runtime
    // (none in cargo-test, §1.1a). Replaced by the `resolve_intake_source` unit tests + the handler source-scan.
    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    // §6.4.1 unit (G15): a NORMAL intake (`drain_pending = None`) passes its own paths + origin through to the
    // freeze seam, NEVER marks the frontend ready (readiness is the drain's signal alone), and never touches
    // `PendingIntake` (§7.8.1 / §0.4.1).
    #[test]
    fn resolve_passthrough_freezes_args_and_does_not_mark_ready() {
        let pending = PendingIntake::default();
        let ready = FrontendReady::default();
        let out = resolve_intake_source(
            &pending,
            &ready,
            None,
            paths(&["/drop/data.csv"]),
            IntakeOrigin::Drop,
        );
        assert_eq!(
            out,
            ResolvedIntake::Freeze {
                paths: paths(&["/drop/data.csv"]),
                origin: IntakeOrigin::Drop,
            },
            "§0.4.1: a normal intake freezes its own passed paths + origin"
        );
        assert!(
            !ready.is_ready(),
            "§7.8.1: a normal intake never marks the frontend ready (readiness is the drainPending signal alone)"
        );
        assert!(
            pending.take().is_none(),
            "§7.8.1: a normal intake never touches PendingIntake"
        );
    }

    // §6.4.1 unit (G15): `drainPending = Some(false)` is a NORMAL intake, not a drain — only `Some(true)`
    // drains (§0.4.1: "omits drainPending (or false) and uses its paths").
    #[test]
    fn resolve_drain_false_is_passthrough() {
        let pending = PendingIntake::default();
        let ready = FrontendReady::default();
        let out = resolve_intake_source(
            &pending,
            &ready,
            Some(false),
            paths(&["/drop/b.csv"]),
            IntakeOrigin::Drop,
        );
        assert_eq!(
            out,
            ResolvedIntake::Freeze {
                paths: paths(&["/drop/b.csv"]),
                origin: IntakeOrigin::Drop,
            },
            "§0.4.1: drainPending=false is a normal intake (passes its paths), not a drain"
        );
        assert!(
            !ready.is_ready(),
            "§0.4.1: drainPending=false does not mark the frontend ready"
        );
    }

    // §6.4.1 unit (G15): a `drainPending: true` drain of an EMPTY `PendingIntake` returns the genuine
    // zero-collection `Empty` (the ordinary first launch with no files) AND marks the frontend ready — the
    // drain call IS the §7.8.1 root-shell-mount readiness signal, which fires even when no files were buffered.
    #[test]
    fn resolve_drain_empty_buffer_is_empty_and_marks_ready() {
        let pending = PendingIntake::default();
        let ready = FrontendReady::default();
        let out = resolve_intake_source(
            &pending,
            &ready,
            Some(true),
            Vec::new(),
            IntakeOrigin::LaunchArg,
        );
        assert_eq!(
            out,
            ResolvedIntake::Empty,
            "§7.8.1: a drain of an empty PendingIntake is the genuine zero-collection Empty (first launch, no files)"
        );
        assert!(
            ready.is_ready(),
            "§7.8.1: the drainPending call marks the frontend ready EVEN when the buffer is empty (the drain IS the readiness signal)"
        );
    }

    // §6.4.1 unit (G15): a `drainPending: true` drain of a NON-empty `PendingIntake` freezes the BUFFERED set
    // with its STORED origin (§7.8.1 "using its stored origin"), marks ready, and consumes the buffer exactly
    // once. The passed `paths`/`origin` are IGNORED (§0.4.1 mutual exclusivity) — a decoy proves it.
    #[test]
    fn resolve_drain_nonempty_freezes_buffer_with_stored_origin_and_drains_once() {
        let pending = PendingIntake::default();
        pending.stash(
            paths(&["/launch/x.png", "/launch/y.jpg"]),
            IntakeOrigin::LaunchArg,
        );
        let ready = FrontendReady::default();
        let out = resolve_intake_source(
            &pending,
            &ready,
            Some(true),
            paths(&["/decoy.csv"]),
            IntakeOrigin::Drop,
        );
        assert_eq!(
            out,
            ResolvedIntake::Freeze {
                paths: paths(&["/launch/x.png", "/launch/y.jpg"]),
                origin: IntakeOrigin::LaunchArg,
            },
            "§7.8.1/§0.4.1: a drain freezes the BUFFERED set with its STORED origin, ignoring the passed args"
        );
        assert!(
            ready.is_ready(),
            "§7.8.1: the drain marks the frontend ready"
        );
        assert!(
            pending.take().is_none(),
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

    // §6.4.1 unit (G15): the C1 handler is now AppHandle-coupled boot-glue — a source-scan pins it binds an
    // `AppHandle`, resolves the two §7.8.1 States, and DISPATCHES via `resolve_intake_source` (the testable
    // drain logic), rather than the P2.22 ignore-all-args shell. The dispatch needle carries the call-site
    // args (`&pending, &ready`) so it matches the CALL, not merely the fn definition (non-blind). Needles
    // `concat!`-assembled (self-match avoidance). [Build-Session-Entscheidung: P2.60]
    #[test]
    fn ingest_paths_handler_dispatches_via_the_drain_resolver() {
        let src = production_intake_source();
        for needle in [
            concat!("app: App", "Handle"),
            concat!("state::<Pending", "Intake>()"),
            concat!("state::<Frontend", "Ready>()"),
            concat!("resolve_intake_", "source(&pending, &ready"),
        ] {
            assert!(
                src.contains(needle),
                "§7.8.1/§0.4.1: the C1 handler must bind an AppHandle, resolve the §7.8.1 States, and dispatch \
                 via resolve_intake_source (missing `{needle}`)"
            );
        }
    }
}

#[cfg(test)]
mod c2a_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C2a `pick_for_intake` native-dialog phase (P2.70). The handler now binds
    //! an `AppHandle` (to open the native `DialogExt` picker + reach the §0.4.4 `IngestRegistry`), so it is
    //! AppHandle-coupled boot-glue (the §1.1a pattern — NOT cargo-test-invocable; this crate ships no
    //! `tauri::test` mock BY DECISION, G28 signature-exempt). Its post-dialog OUTCOME logic lives in the
    //! AppHandle-free `resolve_pick_outcome` helper, unit-tested here; the handler's WIRING (register the token
    //! BEFORE the dialog, open it on a blocking thread, map the outcome) is source-scan-pinned. The §0.4.1
    //! typed wire surface stays asserted by the bindings.ts golden (`bindings_codegen` in main.rs).
    //! [Build-Session-Entscheidung: P2.70]
    //!
    //! [Test-Change: P2.70 — old-obsolete+new-correct, §1.1/§0.4.1] the P2.23 direct
    //! `block_on(pick_for_intake(…))` contract test (+ its `collecting_id()` helper) is OBSOLETE — the handler
    //! now binds an `AppHandle`, uninvocable without a Tauri runtime (none in cargo-test, §1.1a). It is
    //! REPLACED by the executable `resolve_pick_outcome` unit tests (the real post-dialog decision, read back
    //! directly) + the handler source-scans — the sanctioned boot-glue stratification (the C1 `ingest_paths` /
    //! P2.60 precedent), NOT a dropped assertion.
    use super::*;

    // [Test-Change: P2.70 — old-obsolete+new-correct, §1.1/§0.4.1] (rationale in the module doc above) — the
    // removed P2.23 `collecting_id()` helper (its `.expect`) + the direct `block_on(pick_for_intake(…))`
    // contract test are obsolete: the handler now binds an `AppHandle`, uninvocable without a Tauri runtime
    // (none in cargo-test, §1.1a). Replaced by the `resolve_pick_outcome` unit tests + the handler source-scans.
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

    // §6.4.1 unit (G15): `resolve_pick_outcome` rule (1) — a C13 trip during the modal ABANDONS the result
    // even if the user had already picked (§1.1: the handler checks the token AFTER the dialog and yields
    // Empty rather than walking the picked paths), so `cancelled` is tested FIRST.
    #[test]
    fn resolve_pick_outcome_abandons_on_a_c13_trip_even_with_picked_paths() {
        assert_eq!(
            resolve_pick_outcome(Some(paths(&["/picked/a.png"])), true),
            PickOutcome::Cancelled,
            "§1.1: a C13 trip during the modal abandons even a successful pick (→ Empty), checked before the pick"
        );
        assert_eq!(
            resolve_pick_outcome(None, true),
            PickOutcome::Cancelled,
            "§1.1: a C13 trip with no pick is also Cancelled"
        );
    }

    // §6.4.1 unit (G15): `resolve_pick_outcome` rule (2) — a user-DISMISSED dialog (None, no C13) is a clean
    // no-op → DialogCancelled (→ Empty, §5.4).
    #[test]
    fn resolve_pick_outcome_dialog_cancelled_when_user_dismisses() {
        assert_eq!(
            resolve_pick_outcome(None, false),
            PickOutcome::DialogCancelled,
            "§5.4: a user-dismissed dialog (None) with no C13 is a clean no-op → Empty"
        );
    }

    // §6.4.1 unit (G15): `resolve_pick_outcome` happy path — a pick with no C13 funnels the picked paths
    // (Picker-stamped at the handler).
    #[test]
    fn resolve_pick_outcome_picked_when_paths_and_no_cancel() {
        assert_eq!(
            resolve_pick_outcome(Some(paths(&["/picked/a.png", "/picked/b.jpg"])), false),
            PickOutcome::Picked(paths(&["/picked/a.png", "/picked/b.jpg"])),
            "§1.1: a successful pick (no C13) funnels the picked paths into the freeze"
        );
    }

    // §6.4.1 unit (G15): the C2a handler registers the ingest token via the RAII guard BEFORE opening the
    // dialog (§1.1: "registers the CollectingId token at handler entry — before opening the dialog"), so a C13
    // during the modal is honoured. The source-scan pins the ORDER. Needles concat!-assembled (self-match
    // avoidance).
    #[test]
    fn c2a_handler_registers_the_token_before_opening_the_dialog() {
        let src = production_intake_source();
        // Code-specific needles (the handler's DOC prose also names "spawn_blocking"/"register…", so match the
        // literal call forms — `register_guard(collecting_id)` and `spawn_blocking(move` — which appear only in
        // the body, never the prose, so the order check pins the real call sites).
        let reg_at = src
            .find(concat!("register_", "guard(collecting_id)"))
            .expect("§1.1/P2.70: the handler registers the ingest token via register_guard");
        let dialog_at = src
            .find(concat!("spawn_", "blocking(move"))
            .expect("§1.1/P2.70: the handler opens the native dialog on a blocking thread");
        assert!(
            reg_at < dialog_at,
            "§1.1/P2.70: the CollectingId token is registered BEFORE the dialog opens (so a C13 during the modal is honoured)"
        );
    }

    // §6.4.1 unit (G15): the native picker opens on a DEDICATED BLOCKING THREAD (spawn_blocking +
    // blocking_pick_*), never a synchronous blocking_pick_* on a Tokio worker (§1.1: the runtime stays free so
    // C13 remains serviceable while the modal is up). Needles concat!-assembled.
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
    // the picked set into the single freeze (`ingest`) — the §1.1 anti-origin-forgery property (a compromised
    // WebView cannot forge the intake origin, §5.4 / §0.10). Needles concat!-assembled.
    // [Build-Session-Entscheidung: P2.63/P2.70]
    #[test]
    fn c2a_handler_stamps_picker_and_funnels_into_ingest() {
        let src = production_intake_source();
        assert!(
            src.contains(concat!(
                "resolve_pick_",
                "outcome(picked_paths, guard.is_cancelled())"
            )),
            "§1.1/P2.70: the handler dispatches the post-dialog decision via the AppHandle-free resolve_pick_outcome"
        );
        assert!(
            src.contains(concat!("ingest(picked_", "paths, IntakeOrigin::Picker)")),
            "§1.1: the C2a handler stamps Picker core-side and funnels the picked set into ingest"
        );
    }
}

#[cfg(test)]
mod c13_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C13 `cancel_ingest` `.cancel()` trip wiring (P2.71). The handler now binds
    //! an `AppHandle` (to reach the §0.4.4 `IngestRegistry` and trip the token), so it is AppHandle-coupled
    //! boot-glue (the §1.1a pattern — NOT cargo-test-invocable; this crate ships no `tauri::test` mock BY
    //! DECISION, G28 signature-exempt). The trip LOGIC is `IngestRegistry::cancel` (unit-tested at P2.45); this
    //! module pins the handler WIRING by source-scan + proves the END-TO-END token-trip→guard→`Empty` chain at
    //! the registry/helper level (no runtime). [Build-Session-Entscheidung: P2.71]
    //!
    //! [Test-Change: P2.71 — old-obsolete+new-correct, §0.4.1] the P2.35 direct `block_on(cancel_ingest(…))`
    //! contract test is OBSOLETE — the handler now binds an `AppHandle`, uninvocable without a Tauri runtime
    //! (none in cargo-test, §1.1a). It is REPLACED by the end-to-end token-trip→`Empty` test + the handler
    //! source-scan — the sanctioned boot-glue stratification (the C1 `ingest_paths` P2.60 / C2a
    //! `pick_for_intake` P2.70 precedent), NOT a dropped assertion (the `collecting_id()` helper is KEPT, reused
    //! by the end-to-end test).
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

    // §6.4.1 unit (G15): the END-TO-END C13-tripped→Empty chain (P2.71), proven at the registry/helper level
    // with NO Tauri runtime. A C2a ingest registers its token (register_guard, P2.70); a C13 cancel trips it
    // (`IngestRegistry::cancel` — the wiring this box adds to the handler); the C2a post-dialog check then reads
    // the tripped token (`guard.is_cancelled()`) and `resolve_pick_outcome` ABANDONS the pick → `Empty` — even
    // though the user had picked real paths (§1.1: the token check wins over a successful pick). This is the
    // live, reachable C13-tripped→Empty path P2.70's post-dialog check was waiting on.
    #[test]
    fn c13_cancel_trips_the_ingest_token_so_the_c2a_pick_is_abandoned_to_empty() {
        let registry = IngestRegistry::default();
        // C2a registered its ingest token before opening the dialog (P2.70):
        let guard = registry.register_guard(collecting_id());
        // C13 cancel_ingest trips it (the wiring P2.71 adds to the handler: app.state::<IngestRegistry>().cancel):
        assert!(
            registry.cancel(collecting_id()),
            "§0.4.4: C13 finds the in-flight ingest's token and trips it"
        );
        // The C2a post-dialog check (P2.70) now reads the tripped token...
        assert!(
            guard.is_cancelled(),
            "§1.1: the C2a guard observes the C13 trip (the post-dialog check is now reachable-true)"
        );
        // ...and resolve_pick_outcome abandons even a successful pick → Empty:
        assert_eq!(
            resolve_pick_outcome(Some(vec![PathBuf::from("/picked/a.png")]), guard.is_cancelled()),
            PickOutcome::Cancelled,
            "§1.1: C13-tripped → the picked paths are abandoned → CollectedSet::Empty (end-to-end reachable)"
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
