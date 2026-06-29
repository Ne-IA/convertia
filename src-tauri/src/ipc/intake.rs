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

use crate::domain::{CollectedSet, CollectingId, IntakeOrigin, PickKind, ScanProgress};
use crate::orchestrator::{FrontendReady, PendingIntake};
use crate::outcome::IpcError;

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
/// [Build-Session-Entscheidung: P2.23] **Interface-shell body — the typed CONTRACT is the deliverable.**
/// P2.23 authors the §0.4.1 wire signature above so the generated `bindings.ts` carries the full C2a door
/// (pulling `PickKind` into the bindings as a command-arg type). The native-dialog BODY is its own set of
/// named, scheduled boxes — the async/`spawn_blocking` `DialogExt` pick with the ingest token registered
/// before the dialog opens (P2.70), the token-drop-on-every-exit-branch rule (P2.71), and the `Picker`-origin
/// stamp + funnel into the C1 `ingest_paths` freeze (P2.63 / P2.62). This is the sanctioned compile-time
/// interface-shell pattern (CLAUDE §5 / the P3 `crate::isolation` shells P4 expands), NOT a quiet deferral: a
/// shell that opens no dialog and freezes nothing returns the §0.6 zero-collection `CollectedSet::Empty {
/// skipped: [] }` — which is **also the contract's genuine cancelled-dialog result** (a cancelled pick is a
/// clean no-op that returns `Empty`, no error, the UI stays Idle, §5.4). The three contract args are accepted
/// so the wire signature is complete and bound to `_` (no fabricated handling) until P2.70/P2.71/P2.63 consume
/// them.
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn pick_for_intake(
    kind: PickKind,
    collecting_id: CollectingId,
    on_scan: Channel<ScanProgress>,
) -> Result<CollectedSet, IpcError> {
    let _ = (kind, collecting_id, on_scan);
    Ok(CollectedSet::Empty {
        skipped: Vec::new(),
    })
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
/// [Build-Session-Entscheidung: P2.35] **Shell returns `Ok(())` — the genuine idempotent no-op-cancel
/// outcome, the C7 `cancel_run` "zero-valued result" branch of the interface-shell pattern, NOT the
/// C3/C4/C5/C6/C8 `Err(InternalError)` branch.** C13 is the ingest-side mirror of C7: an idempotent
/// fire-and-forget side-effect that trips a token and returns. Its success type `()` has a zero value, and a
/// cancel of a non-existent / already-finished ingest is the desired "not collecting" end-state (§1.1) — so
/// tripping *no* token (the shell has no §0.4.4 `CollectingId` registry — P2.45 — yet) is genuinely `Ok(())`,
/// NOT a fabricated success: it claims nothing positive happened (unlike a fabricated C6 `Ok(RunId)`, which
/// would lie that a run started). The cancel *effect* is observed by C1/C2a returning the §0.6 zero-collection
/// `CollectedSet::Empty` once its token is tripped (§1.1), never C13's return. The real registry resolve +
/// `.cancel()` wiring lands at P2.45 (the `CollectingId` → ingest-scoped token registry) / P2.69 (the
/// cooperative ingest-cancellation poll) / P2.71 (the C2a token-drop-on-every-exit-branch); the contract is
/// unchanged by it (cancel stays `Ok(())`).
#[tauri::command(rename_all = "camelCase")]
#[specta::specta]
pub async fn cancel_ingest(collecting_id: CollectingId) -> Result<(), IpcError> {
    let _ = collecting_id;
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
    //! §6.4.1 unit (G15): the §0.4.1 C2a `pick_for_intake` typed CONTRACT (P2.23). Mirrors the C1
    //! `c1_contract` test — the handler now carries its full typed signature, so the P2.21 all-shells
    //! `block_on(pick_for_intake())` invocation in `crate::ipc` (mod.rs) is REPLACED here by C2a's own
    //! typed-contract test (the fill-box transition the P2.21 note schedules). It invokes the contract with
    //! the full typed arg set and asserts the shell-stage return; the native-dialog pick body + its real-pick
    //! assertions land at P2.70/P2.71/P2.63. [Build-Session-Entscheidung: P2.23]
    use super::*;
    use tauri::async_runtime::block_on;

    /// A `CollectingId` for the contract call — minted through its PUBLIC bare-uuid `Deserialize` wire form
    /// (the inner `Uuid` is private to `crate::domain`; the frontend mints the id, §0.4 C13), never a
    /// back-door constructor — mirroring the `c1_contract` helper.
    fn collecting_id() -> CollectingId {
        serde_json::from_str(r#""33333333-3333-4333-8333-333333333333""#)
            .expect("CollectingId deserializes from a uuid string")
    }

    // §6.4.1 unit (G15): the C2a contract is invocable with its full §0.4.1 typed arg set (`kind`,
    // `collectingId`, the non-optional `onScan` Channel) and returns a `CollectedSet` (the wire door this box
    // authors). `on_scan` is a real `Channel::new(|_| Ok(()))` — the non-optional contract (there is no `None`
    // arm; see the handler's forced-deviation note). The native dialog is not opened yet (P2.70), so the
    // handler returns the zero-collection `CollectedSet::Empty` — which is ALSO the contract's §5.4
    // cancelled-dialog no-op; P2.70/P2.71 replace it with the real DialogExt pick funnelled into the C1 freeze.
    #[test]
    fn c2a_pick_for_intake_contract_is_invocable_and_typed() {
        let out = block_on(pick_for_intake(
            PickKind::Files,
            collecting_id(),
            Channel::new(|_| Ok(())),
        ));
        assert_eq!(
            out,
            Ok(CollectedSet::Empty {
                skipped: Vec::new()
            }),
            "§0.4.1: the C2a contract shell opens no dialog yet (the native-pick body is P2.70/P2.71), so it \
             returns the zero-collection CollectedSet::Empty — also the §5.4 cancelled-dialog result; the \
             typed signature is the P2.23 deliverable"
        );
    }
}

#[cfg(test)]
mod c13_contract {
    //! §6.4.1 unit (G15): the §0.4.1 C13 `cancel_ingest` typed CONTRACT (P2.35). The handler now carries its
    //! typed `{ collectingId } -> Result<(), IpcError>` signature, so the P2.21 all-shells
    //! `block_on(cancel_ingest())` invocation in `crate::ipc` (mod.rs) is REPLACED here by C13's own
    //! typed-contract test (the fill-box transition the P2.21 note schedules — the LAST such move, leaving only
    //! C12 bare). The shell returns the genuine idempotent no-op-cancel `Ok(())` (the C7 `cancel_run` branch);
    //! the §0.4.4 token registry resolve + `.cancel()` land at P2.45 / P2.69. [Build-Session-Entscheidung: P2.35]
    use super::*;
    use tauri::async_runtime::block_on;

    /// A `CollectingId` for the contract call — minted through its PUBLIC bare-uuid `Deserialize` wire form
    /// (the frontend mints the ingest id, §0.4.4), mirroring the `c1_contract`/`c2a_contract` helpers.
    fn collecting_id() -> CollectingId {
        serde_json::from_str(r#""44444444-4444-4444-8444-444444444444""#)
            .expect("CollectingId deserializes from a uuid string")
    }

    // §6.4.1 unit (G15): the C13 contract is invocable with its §0.4.1 typed `collectingId` arg and returns a
    // `Result<(), IpcError>` (the §0.4 universal error shape). The shell trips no token yet (no ingest registry
    // — P2.45), so it returns the genuine idempotent no-op-cancel `Ok(())` (a cancel of a non-existent /
    // finished ingest is the desired "not collecting" end-state, §1.1); P2.45/P2.69 wire the real registry
    // resolve + cooperative-poll cancel.
    #[test]
    fn c13_cancel_ingest_contract_is_invocable_and_typed() {
        let out = block_on(cancel_ingest(collecting_id()));
        assert_eq!(
            out,
            Ok(()),
            "§0.4.1/§0.4: the C13 contract shell trips no token yet (the §0.4.4 ingest registry is P2.45), so \
             it returns the genuine idempotent no-op-cancel Ok(()); the typed Result<(), IpcError> signature \
             is the P2.35 deliverable"
        );
    }
}
