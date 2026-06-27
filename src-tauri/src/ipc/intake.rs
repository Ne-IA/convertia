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

use crate::domain::{CollectedSet, CollectingId, IntakeOrigin, PickKind, ScanProgress};
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
/// [Build-Session-Entscheidung: P2.22] **Interface-shell body — the typed CONTRACT is the deliverable.**
/// P2.22 authors the §0.4.1 wire signature above so the generated `bindings.ts` carries the full C1 door;
/// the §2.4 freeze BODY is its own set of named, scheduled boxes — the §1.1 recursive walk → §1.2 detect →
/// §2.3 de-dup → §1.3 group freeze funnel (P2.62), the §0.4.4 `collecting_id` token registry (P2.45), the
/// `drain_pending` `PendingIntake` drain (P2.60), and the `on_scan` scan-telemetry pump (P2.69) — wired
/// end-to-end into this handler by P3.49 "Implement C1 `ingest_paths`" (the CSV→TSV walking-skeleton slice)
/// once those layers exist. This is the sanctioned compile-time interface-shell pattern (CLAUDE §5 / the P3
/// `crate::isolation` shells P4 expands), NOT a quiet deferral: a shell that performs no freeze collects
/// nothing, so it returns the §0.6 zero-collection `CollectedSet::Empty { skipped: [] }` (the genuinely-zero
/// case — cancelled dialog / drained-empty `PendingIntake` / all-hidden-filtered). The five contract args
/// are accepted so the wire signature is complete and bound to `_` to mark them shell-accepted (no
/// fabricated handling) until their named boxes consume them. The freeze funnel's own §0.7 module home is
/// P2.62's to fix (the §1.1/§2.4 freeze is not yet placed in the §0.7 tree), so P2.22 does not pre-create it.
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
    paths: Vec<PathBuf>,
    origin: IntakeOrigin,
    collecting_id: CollectingId,
    drain_pending: Option<bool>,
    on_scan: Channel<ScanProgress>,
) -> Result<CollectedSet, IpcError> {
    let _ = (paths, origin, collecting_id, drain_pending, on_scan);
    Ok(CollectedSet::Empty {
        skipped: Vec::new(),
    })
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
    //! §6.4.1 unit (G15): the §0.4.1 C1 `ingest_paths` typed CONTRACT (P2.22). The handler now carries its
    //! full typed signature, so the P2.21 all-shells `block_on(ingest_paths())` invocation in `crate::ipc`
    //! (mod.rs) is REPLACED here by C1's own typed-contract test — the fill-box transition the P2.21 note
    //! schedules ("replace each invocation … with that command's typed-contract test"). It invokes the
    //! contract with the full typed arg set and asserts the shell-stage return; the §2.4 freeze body + its
    //! real-slice assertions land at P2.62 / P3.49. [Build-Session-Entscheidung: P2.22]
    use super::*;
    use tauri::async_runtime::block_on;

    /// A `CollectingId` for the contract call — minted through its PUBLIC bare-uuid `Deserialize` wire form
    /// (the inner `Uuid` is private to `crate::domain`; the frontend mints the id, §0.4 C13), never a
    /// back-door constructor — mirroring the `crate::orchestrator` test helpers.
    fn collecting_id() -> CollectingId {
        serde_json::from_str(r#""22222222-2222-4222-8222-222222222222""#)
            .expect("CollectingId deserializes from a uuid string")
    }

    // §6.4.1 unit (G15): the C1 contract is invocable with its full §0.4.1 typed arg set and returns a
    // `CollectedSet` (the wire door this box authors). `on_scan` is a real `Channel::new(|_| Ok(()))` — the
    // non-optional contract (there is no `None` arm; see the handler's forced-deviation note); `drain_pending
    // = None` is a normal intake. The freeze seam performs no walk yet (P2.62), so the handler returns the
    // zero-collection `CollectedSet::Empty` — the shell-stage contract assertion P3.49 replaces with the real
    // CSV→TSV walking-skeleton slice.
    #[test]
    fn c1_ingest_paths_contract_is_invocable_and_typed() {
        let out = block_on(ingest_paths(
            vec![PathBuf::from("/drop/data.csv")],
            IntakeOrigin::Drop,
            collecting_id(),
            None,
            Channel::new(|_| Ok(())),
        ));
        assert_eq!(
            out,
            Ok(CollectedSet::Empty {
                skipped: Vec::new()
            }),
            "§0.4.1: the C1 contract shell freezes nothing yet (the §2.4 funnel body is P2.62), so it \
             returns the zero-collection CollectedSet::Empty; the typed signature is the P2.22 deliverable"
        );
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
