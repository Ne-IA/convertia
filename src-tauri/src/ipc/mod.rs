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
mod c_surface_scan {
    //! Shared test-support for the STRUCTURAL C-surface scans (`responsiveness_contract`,
    //! `error_shape_contract`, `camel_case_wire_contract`, and the `ipc_boundary_proptest` completeness
    //! bind): ONE handler table + ONE set of production-prefix source readers, so the legs can never drift
    //! apart on what the §0.4.1 surface IS. Hoisted verbatim out of `responsiveness_contract` (P2.125)
    //! when P2.137 added the sibling legs — the assertions those helpers feed are unchanged.
    //! [Build-Session-Entscheidung: P2.137]

    /// A per-file production-source reader (one of the `*_src` fns below) — aliased so the handler table's
    /// element type stays simple (clippy::type_complexity).
    pub(super) type SrcFn = fn() -> &'static str;

    /// Everything before a scanned file's first `#[cfg(test)]`, so a needle can never match a test's own
    /// source. `concat!`-split so the literal marker is absent from this scanning module too.
    pub(super) fn production_prefix(full: &'static str) -> &'static str {
        full.split_once(concat!("#[cfg", "(test)]"))
            .map_or(full, |(prefix, _)| prefix)
    }

    pub(super) fn intake_src() -> &'static str {
        production_prefix(include_str!("intake.rs"))
    }
    pub(super) fn planning_src() -> &'static str {
        production_prefix(include_str!("planning.rs"))
    }
    pub(super) fn conversion_src() -> &'static str {
        production_prefix(include_str!("conversion.rs"))
    }
    pub(super) fn system_src() -> &'static str {
        production_prefix(include_str!("system.rs"))
    }

    /// The complete §0.4.1 C1–C13 command surface — every handler name paired with its file's
    /// production-prefix reader (14 handlers: C2 splits into C2a/C2b). The single table every structural
    /// leg iterates; a new command must join this table to be covered by the legs (the registration-side
    /// closed set is the P2.36/G23 gate's job).
    pub(super) const HANDLERS: [(&str, SrcFn); 14] = [
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

    /// Everything between `pub async fn {name}(` and the body's opening `{` — the remainder of the
    /// handler's signature (multi-line arg list + return type; no C-handler signature carries an interior
    /// `{`). Slicing the signature out means a scan needle can only ever match the declaration itself,
    /// never a doc-comment mention of the same tokens. [Build-Session-Entscheidung: P2.137]
    pub(super) fn signature_rest_of(name: &str, src: &'static str) -> &'static str {
        let marker = format!("pub async fn {name}(");
        let (_, after) = src
            .split_once(marker.as_str())
            .expect("every §0.4.1 handler declaration is present in its file's production prefix");
        let (sig_rest, _) = after
            .split_once('{')
            .expect("a handler signature terminates at its body's opening brace");
        sig_rest
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

    // The per-file production-prefix readers + the 14-handler table live in the shared `c_surface_scan`
    // support module (hoisted verbatim at P2.137 for the sibling structural legs — `error_shape_contract`,
    // `camel_case_wire_contract`, the `ipc_boundary_proptest` completeness bind; assertions unchanged).
    use super::c_surface_scan::{conversion_src, intake_src, HANDLERS};

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
    // real-§1.10-preflight test is P3.49 (C3/C4 slice bodies) / P4.72 (the §1.10 estimator). (The handler
    // table is the shared `c_surface_scan::HANDLERS` as of P2.137.)
    #[test]
    fn every_c_command_handler_is_async() {
        for (name, src) in HANDLERS {
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

#[cfg(test)]
mod error_shape_contract {
    //! §6.4.1 unit (G15): the §0.4 UNIVERSAL ERROR SHAPE over the whole C-surface — "every command returns
    //! `Result<T, IpcError>`" (§0.4 "Error shape"; the §0.4.1 table's Response column lists only the
    //! success `T`). A STRUCTURAL (source-scan) leg beside `responsiveness_contract`, over the SAME
    //! `c_surface_scan` 14-handler table + per-file production-prefix sources: each handler's signature
    //! carries `-> Result<` and its return type terminates in `IpcError>`. This class ALREADY SHIPPED once
    //! — C2b `pick_destination` landed returning a bare `Option<PathBuf>` (no `Result`, no `IpcError`; both
    //! G1 reviewers missed it) and was fixed at fb685df — so the universal shape is pinned structurally: a
    //! new or regressed handler returning a bare `T` reds at L2 before `bindings.ts` can mirror the wrong
    //! door. [Build-Session-Entscheidung: P2.137]

    use super::c_surface_scan::{signature_rest_of, HANDLERS};

    // §6.4.1 unit (G15): every §0.4.1 C1–C13 handler signature carries the `-> Result<` opener AND ends in
    // `IpcError>` — the two needles that together pin `-> Result<T, IpcError>` (the §0.4.3 error arm is the
    // fixed tail; `T` varies per the §0.4.1 Response column). The signature slice runs to the body's
    // opening brace, so the tail check sees the full return type and nothing of the body.
    #[test]
    fn every_c_command_returns_result_t_ipc_error() {
        for (name, src) in HANDLERS {
            let sig = signature_rest_of(name, src());
            assert!(
                sig.contains("-> Result<"),
                "§0.4: C-command `{name}` must return the universal `Result<T, IpcError>` shape — its \
                 signature carries no `-> Result<` (the C2b bare-`Option` class, fixed fb685df)"
            );
            assert!(
                sig.trim_end().ends_with("IpcError>"),
                "§0.4: C-command `{name}`'s return type must terminate in `IpcError>` (the §0.4.3 error \
                 arm of the universal shape); signature rest: `{sig}`"
            );
        }
    }
}

#[cfg(test)]
mod camel_case_wire_contract {
    //! §6.4.1 unit (G15): the §0.4.1 camelCase WIRE-KEY invariant over the C-surface ("field naming is
    //! `camelCase` on the wire", §0.4.1). tauri-specta rc.25 camelCases the generated invoke keys
    //! UNCONDITIONALLY, while tauri itself resolves each command arg by its Rust snake_case name unless the
    //! handler carries `#[tauri::command(rename_all = "camelCase")]` — so a multi-word arg without the
    //! attribute type-checks, generates clean `bindings.ts`, AND compiles, then fails only at RUNTIME as an
    //! invalid-args invoke error (the d832698 C1/C2a/C13 incident). That runtime-only failure mode is
    //! exactly why this STRUCTURAL scan exists: every handler whose signature carries an
    //! underscore-bearing WIRE arg must carry the rename_all attribute form directly above its
    //! `pub async fn`.
    //!
    //! [Build-Session-Entscheidung: P2.137] WIRE args here = every deserialized arg INCLUDING `Channel<T>`
    //! (the channel id rides the invoke payload under the arg's key, so `on_scan`/`on_progress` are renamed
    //! too); only the runtime-injected `AppHandle` is excluded (tauri supplies it — it has no wire key).
    //! This widens the sweep-finding's AppHandle+Channel exclusion on the DETECTION side: excluding Channel
    //! args could leave a handler whose only snake_case arg is a channel unguarded, and admitting them
    //! costs nothing (today's required handler set is identical either way).

    use super::c_surface_scan::{signature_rest_of, HANDLERS};

    /// The underscore-bearing wire-arg names of a handler's signature rest (`args…) -> ret`): each
    /// `name: Type` pair except an `AppHandle`-typed one. The identifier filter skips any fragment a comma
    /// split could produce inside a hypothetical multi-parameter generic type.
    fn snake_case_wire_args(sig_rest: &str) -> Vec<String> {
        let (params, _ret) = sig_rest
            .split_once("->")
            .expect("a C-handler signature carries a return arrow");
        let (args, _) = params
            .rsplit_once(')')
            .expect("the arg list closes before the return arrow");
        args.split(',')
            .filter_map(|part| {
                let (name, ty) = part.split_once(':')?;
                let name = name.trim();
                if ty.contains("AppHandle") {
                    return None;
                }
                (!name.is_empty()
                    && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && name.contains('_'))
                .then(|| name.to_owned())
            })
            .collect()
    }

    // §6.4.1 unit (G15): every snake_case-wire-arg handler carries the rename_all attribute directly above
    // its declaration — the LAST `#[tauri::command` occurrence before the `pub async fn` is the handler's
    // own attribute (attributes sit immediately above their item). Needles `concat!`-split so this
    // scanning module never self-matches.
    #[test]
    fn every_snake_case_wire_arg_handler_carries_the_rename_all_attribute() {
        let attr_form = concat!("#[tauri::command(rename_all = ", "\"camelCase\")]");
        let attr_start = concat!("#[tauri::", "command");
        let mut covered: Vec<&str> = Vec::new();
        for (name, src) in HANDLERS {
            let s = src();
            let snake = snake_case_wire_args(signature_rest_of(name, s));
            if snake.is_empty() {
                continue;
            }
            covered.push(name);
            let marker = format!("pub async fn {name}(");
            let (before, _) = s.split_once(marker.as_str()).expect(
                "every §0.4.1 handler declaration is present in its file's production prefix",
            );
            let own_attr_at = before
                .rfind(attr_start)
                .expect("every C-handler carries a #[tauri::command] attribute");
            assert!(
                before[own_attr_at..].starts_with(attr_form),
                "§0.4.1: C-command `{name}` carries snake_case wire arg(s) {snake:?} but its \
                 #[tauri::command] attribute is not the rename_all = \"camelCase\" form — tauri-specta \
                 camelCases the invoke keys unconditionally, so the mismatch fails only at runtime (the \
                 d832698 incident class)"
            );
        }
        // The scan must actually bite: the surface carries 9 snake-arg handlers (C1/C2a/C13, C3/C4/C5,
        // C6/C7/C8) — a parser regression that silently matched none would leave the invariant unasserted.
        assert!(
            covered.len() >= 9,
            "§0.4.1: the snake_case wire-arg scan covered fewer handlers than the known surface: {covered:?}"
        );
    }
}

#[cfg(test)]
mod ipc_boundary_proptest {
    //! §6.4.2 property tests (G16) — the two IPC boundary proptest legs the P0.4.3 `IPC_PROPTEST_TARGETS`
    //! contract freezes to P2, instantiated over the now-real §0.4.1 / §1.1 C1–C13 command surface (P2.126):
    //! `ipc_serde` and `ipc_numeric_overflow`. These are the P2-scoped PROPTEST half of the in-core fuzz
    //! plane; they live in the test suite (`#[cfg(test)]`), NOT the `fuzz/` libFuzzer tree — the 6
    //! coverage-guided G48 `fuzz_target!` in-core targets live in that separate tree (built across P3–P9),
    //! and `scripts/check-fuzz-contract` places these two IPC legs in `tests/` by design (its docstring:
    //! "the IPC proptest legs … land in P2 (tests/, NOT under fuzz/)"). The P2.126 box and the frozen
    //! `check-fuzz-contract` home the IPC serde boundary in `IPC_PROPTEST_TARGETS` — a G16 proptest in the
    //! test suite, NOT a `fuzz/` libFuzzer target; this module IS that proptest.
    //!
    //! Leg (a) `ipc_serde` (test-strategy §1.5 pt.5): tauri deserializes each non-runtime command arg via
    //! `serde_json::from_value`, so feeding arbitrary / malformed JSON to EVERY C1–C13 inbound arg type must
    //! yield a structured `Result`, never a panic across the Tauri boundary (§0.4.3 `IpcError` is the `Err`
    //! arm). The runtime-injected `AppHandle` / `Channel<T>` args are not deserialized and are excluded.
    //!
    //! Leg (b) `ipc_numeric_overflow` (§0.4 T10): the single inbound numeric IPC arg — `OptionValue::Int(i64)`
    //! carried in `options: OptionValues` (C4/C5/C6) — is exercised at the P0.4.3 boundary set
    //! (`u32::MAX` / `i32::MIN` / 0 / 1 / 65535): each round-trips with its value preserved exactly, and an
    //! out-of-i64-range integer is a structured `Err`, never a silent wrap. This is the runtime companion to
    //! the IPC module-root `#![deny(clippy::arithmetic_side_effects)]` (mod.rs), which forbids the unchecked
    //! `width*height*bpp`-style arithmetic at compile time; this proves the wire boundary that feeds it is
    //! overflow-safe.
    //!
    //! All G16 determinism knobs are set (test-strategy §1.3): a case-count floor of 512 (above proptest's
    //! 256 default) and a `deterministic_rng`-pinned seed, so every case is identical each run and a
    //! counterexample is reproducible and NEVER retried-to-pass.
    //!
    //! [Build-Session-Entscheidung: P2.126] Co-located with `responsiveness_contract` in the IPC module root
    //! (the P2.125 sibling home for IPC-surface contracts); the runner replicates the P2.14 pinned-seed /
    //! 512-case pattern; the enumerated inbound-type list models tauri's per-arg `from_value` boundary, so a
    //! new inbound arg type must be added here to stay covered — structurally ENFORCED since P2.137 by the
    //! completeness bind below: the pinned `INBOUND_WIRE_ARG_TYPES` list is held set-equal to the real
    //! handler signatures AND to both feed-helper bodies.

    use crate::domain::{
        CollectedSetId, CollectingId, DestinationChoice, IntakeOrigin, OpenKind, OptionValue,
        OptionValues, PickKind, RerunDecision, RunId, TargetId,
    };
    use proptest::prelude::*;
    use proptest::test_runner::{RngAlgorithm, TestRng, TestRunner};
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::path::PathBuf;

    /// The IPC-boundary property-test case-count floor (test-strategy §1.3 / G16: above proptest's 256
    /// default; matches the P2.14 §0.6-invariant floor). [Build-Session-Entscheidung: P2.126]
    const P2_126_CASES: u32 = 512;

    /// A PINNED-SEED proptest runner (test-strategy §1.3 / G16: "a pinned CI seed"). The `proptest!` macro
    /// seeds its forward run from entropy, so the boundary exploration drives a `TestRunner` with a
    /// `deterministic_rng` directly — making all 512 cases identical every run, so a counterexample is
    /// reproducible and NEVER retried-to-pass (§7). Replicates the P2.14 runner.
    /// [Build-Session-Entscheidung: P2.126]
    fn pinned_runner() -> TestRunner {
        TestRunner::new_with_rng(
            ProptestConfig::with_cases(P2_126_CASES),
            TestRng::deterministic_rng(RngAlgorithm::ChaCha),
        )
    }

    /// A bounded-depth arbitrary `serde_json::Value` — the shape tauri parses an invoke payload into before
    /// deserializing each command arg. Depth/size are bounded so the STRATEGY never exhausts the stack; the
    /// unbounded-depth case is covered by the deterministic over-limit fixture in
    /// `serde_boundary_rejects_malformed_with_structured_err`.
    fn arb_json_value() -> impl Strategy<Value = serde_json::Value> {
        let leaf = prop_oneof![
            Just(serde_json::Value::Null),
            any::<bool>().prop_map(serde_json::Value::from),
            any::<i64>().prop_map(serde_json::Value::from),
            any::<u64>().prop_map(serde_json::Value::from),
            any::<f64>().prop_map(serde_json::Value::from),
            ".*".prop_map(serde_json::Value::from),
        ];
        leaf.prop_recursive(4, 32, 8, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..8).prop_map(serde_json::Value::Array),
                prop::collection::vec((".*", inner), 0..8)
                    .prop_map(|entries| serde_json::Value::Object(entries.into_iter().collect())),
            ]
        })
    }

    /// The CLOSED SET of §0.4.1 wire-DESERIALIZED inbound arg types across the C1–C13 surface
    /// (whitespace-normalized; the runtime-supplied `AppHandle` / `Channel<T>` args are never deserialized
    /// and are excluded, per the leg-(a) module docs). This single pinned list is what BOTH
    /// `feed_every_ipc_input_type_*` legs enumerate. Rust generics are monomorphized at compile time, so
    /// the feed helpers cannot literally iterate a name list; instead the two P2.137 completeness tests
    /// below bind (a) the real handler signatures to this list and (b) this list to the feed-helper bodies
    /// — so an inbound arg type added to any handler reds at L2 until BOTH feed legs cover it.
    /// [Build-Session-Entscheidung: P2.137]
    const INBOUND_WIRE_ARG_TYPES: [&str; 13] = [
        "CollectedSetId",
        "CollectingId",
        "DestinationChoice",
        "IntakeOrigin",
        "OpenKind",
        "Option<bool>",
        "OptionValues",
        "PathBuf",
        "PickKind",
        "RerunDecision",
        "RunId",
        "TargetId",
        "Vec<PathBuf>",
    ];

    /// Deserialize the given JSON `Value` into EVERY §0.4.1 C1–C13 inbound argument type — the exact per-arg
    /// `serde_json::from_value` step tauri runs at the command boundary. The runtime-injected `AppHandle` /
    /// `Channel<T>` args are supplied by tauri, never deserialized, so they are absent here. Each result is
    /// discarded — the property under exercise is that none of these calls PANICS. Enumerates exactly the
    /// pinned `INBOUND_WIRE_ARG_TYPES` (the P2.137 completeness bind holds it to that list).
    fn feed_every_ipc_input_type_from_value(v: &serde_json::Value) {
        // C1 ingest_paths — paths / origin / collectingId / drainPending
        let _ = serde_json::from_value::<Vec<PathBuf>>(v.clone());
        let _ = serde_json::from_value::<IntakeOrigin>(v.clone());
        let _ = serde_json::from_value::<CollectingId>(v.clone());
        let _ = serde_json::from_value::<Option<bool>>(v.clone());
        // C2a pick_for_intake — kind (collectingId shared with C1/C13)
        let _ = serde_json::from_value::<PickKind>(v.clone());
        // C3/C4/C5/C6 — collectedSetId / target / options / destination
        let _ = serde_json::from_value::<CollectedSetId>(v.clone());
        let _ = serde_json::from_value::<TargetId>(v.clone());
        let _ = serde_json::from_value::<OptionValues>(v.clone());
        let _ = serde_json::from_value::<DestinationChoice>(v.clone());
        // C6 start_conversion — rerunDecision
        let _ = serde_json::from_value::<RerunDecision>(v.clone());
        // C7/C8 — runId
        let _ = serde_json::from_value::<RunId>(v.clone());
        // C9 open_path — kind / path
        let _ = serde_json::from_value::<OpenKind>(v.clone());
        let _ = serde_json::from_value::<PathBuf>(v.clone());
    }

    /// The `from_str` twin of `feed_every_ipc_input_type_from_value` — exercises the raw-parse path (an
    /// arbitrary, possibly syntactically-malformed string) into every C1–C13 inbound arg type. Same property:
    /// a structured `Result`, never a panic. Enumerates exactly the pinned `INBOUND_WIRE_ARG_TYPES` (the
    /// P2.137 completeness bind holds it to that list).
    fn feed_every_ipc_input_type_from_str(s: &str) {
        let _ = serde_json::from_str::<Vec<PathBuf>>(s);
        let _ = serde_json::from_str::<IntakeOrigin>(s);
        let _ = serde_json::from_str::<CollectingId>(s);
        let _ = serde_json::from_str::<Option<bool>>(s);
        let _ = serde_json::from_str::<PickKind>(s);
        let _ = serde_json::from_str::<CollectedSetId>(s);
        let _ = serde_json::from_str::<TargetId>(s);
        let _ = serde_json::from_str::<OptionValues>(s);
        let _ = serde_json::from_str::<DestinationChoice>(s);
        let _ = serde_json::from_str::<RerunDecision>(s);
        let _ = serde_json::from_str::<RunId>(s);
        let _ = serde_json::from_str::<OpenKind>(s);
        let _ = serde_json::from_str::<PathBuf>(s);
    }

    /// The body of one of the two module-level feed helpers: from its `fn …(` marker to its
    /// 4-space-indented closing brace (the helpers nest no braced blocks, so the first `\n    }` is the
    /// helper's own close). [Build-Session-Entscheidung: P2.137]
    fn fn_body_of<'a>(src: &'a str, marker: &str) -> &'a str {
        let (_, after) = src
            .split_once(marker)
            .expect("the feed-helper declaration is present in this module");
        let (body, _) = after
            .split_once("\n    }")
            .expect("the feed helper closes at 4-space indent");
        body
    }

    /// P2.137 completeness bind, SIGNATURE side: the wire-deserialized arg types extracted from the four
    /// handler files' production-prefix signatures (via the shared `c_surface_scan` table) are EXACTLY the
    /// pinned `INBOUND_WIRE_ARG_TYPES` set. Extraction mirrors tauri's boundary: every `name: Type` arg
    /// except the runtime-supplied `AppHandle` / `Channel<T>` / `State<T>`, whitespace-normalized — so an arg
    /// type added to any §0.4.1 handler without extending the pinned list (and with it both feed legs) reds here.
    /// [Test-Change: P3.50 — old-obsolete+new-correct, §0.4.1] the C8 `get_run_summary` body (P3.50) is the
    /// FIRST handler with a direct `State<T>` arg (the §0.4.4 `State<RunResultStore>`); like `AppHandle` /
    /// `Channel<T>` it is RUNTIME-INJECTED by tauri, never deserialized from the wire, so it is EXCLUDED from
    /// the inbound-wire set (extending the test's existing runtime-supplied filter, not relaxing the pin).
    #[test]
    fn handler_signatures_carry_exactly_the_pinned_inbound_arg_types() {
        use super::c_surface_scan::{signature_rest_of, HANDLERS};
        use std::collections::BTreeSet;
        let mut extracted: BTreeSet<String> = BTreeSet::new();
        for (name, src) in HANDLERS {
            let sig_rest = signature_rest_of(name, src());
            let (params, _ret) = sig_rest
                .split_once("->")
                .expect("a C-handler signature carries a return arrow");
            let (args, _) = params
                .rsplit_once(')')
                .expect("the arg list closes before the return arrow");
            for part in args.split(',') {
                let Some((_arg, ty)) = part.split_once(':') else {
                    continue;
                };
                let ty: String = ty.chars().filter(|c| !c.is_whitespace()).collect();
                // Skip tauri's runtime-injected args (never deserialized from the wire): AppHandle, the
                // progress Channel<T>, and the §0.4.4 State<T> store injections (C8's RunResultStore, P3.50).
                if ty.is_empty()
                    || ty.contains("AppHandle")
                    || ty.contains("Channel<")
                    || ty.contains("State<")
                {
                    continue;
                }
                extracted.insert(ty);
            }
        }
        let pinned: BTreeSet<String> = INBOUND_WIRE_ARG_TYPES
            .iter()
            .map(|t| (*t).to_owned())
            .collect();
        assert_eq!(
            extracted, pinned,
            "§1.5/§0.4.1: the C-surface's deserialized arg types and the pinned proptest feed list must be \
             the same closed set — extend INBOUND_WIRE_ARG_TYPES AND both feed_every_ipc_input_type_* legs \
             in the same change"
        );
    }

    /// P2.137 completeness bind, FEED side: each feed-helper body enumerates EXACTLY the pinned list —
    /// every pinned type appears as a deserialize call, and the per-helper call COUNT equals the list
    /// length (so an extra un-pinned feed line reds too). Scans this file in FULL (the helpers live inside
    /// this `#[cfg(test)]` module, past the production-prefix cut); needles are `concat!`-split so this
    /// test's own source never matches.
    #[test]
    fn feed_helpers_enumerate_exactly_the_pinned_inbound_arg_types() {
        let src = include_str!("mod.rs");
        let value_body = fn_body_of(src, concat!("fn feed_every_ipc_input_type_", "from_value("));
        let str_body = fn_body_of(src, concat!("fn feed_every_ipc_input_type_", "from_str("));
        let value_call = concat!("serde_json::from_value", "::<");
        let str_call = concat!("serde_json::from_str", "::<");
        for ty in INBOUND_WIRE_ARG_TYPES {
            assert!(
                value_body.contains(&format!("{value_call}{ty}>")),
                "the from_value feed leg must exercise the pinned inbound type `{ty}`"
            );
            assert!(
                str_body.contains(&format!("{str_call}{ty}>")),
                "the from_str feed leg must exercise the pinned inbound type `{ty}`"
            );
        }
        assert_eq!(
            value_body.matches(value_call).count(),
            INBOUND_WIRE_ARG_TYPES.len(),
            "the from_value feed leg must enumerate EXACTLY the pinned list — no extra un-pinned line"
        );
        assert_eq!(
            str_body.matches(str_call).count(),
            INBOUND_WIRE_ARG_TYPES.len(),
            "the from_str feed leg must enumerate EXACTLY the pinned list — no extra un-pinned line"
        );
    }

    /// Leg (a) `ipc_serde`: over arbitrary structurally-valid JSON (the tauri invoke-payload shape),
    /// deserializing EVERY inbound arg type never panics — malformed input becomes a structured `Err`; a
    /// panic would be a caught, shrunk counterexample. proptest captures any panic in the closure; the
    /// explicit `catch_unwind` makes the "no panic across the Tauri boundary" property self-evident.
    #[test]
    fn serde_boundary_from_value_never_panics() {
        pinned_runner()
            .run(&arb_json_value(), |v| {
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    feed_every_ipc_input_type_from_value(&v);
                }));
                prop_assert!(
                    outcome.is_ok(),
                    "§1.5: deserializing an IPC arg from arbitrary JSON must never panic across the Tauri \
                     boundary (value: {v})"
                );
                Ok(())
            })
            .unwrap();
    }

    /// Leg (a) `ipc_serde`, raw-parse twin: over an arbitrary string (frequently invalid JSON syntax),
    /// parsing+deserializing every inbound arg type never panics — the parser returns a structured `Err`.
    #[test]
    fn serde_boundary_from_str_never_panics() {
        pinned_runner()
            .run(&any::<String>(), |s| {
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    feed_every_ipc_input_type_from_str(&s);
                }));
                prop_assert!(
                    outcome.is_ok(),
                    "§1.5: parsing+deserializing an IPC arg from an arbitrary string must never panic (len {})",
                    s.len()
                );
                Ok(())
            })
            .unwrap();
    }

    /// Leg (a) `ipc_serde`, the positive-teeth half: representative malformed inputs deserialize to a
    /// structured `Err` (never a silent `Ok`, never a panic), including serde_json's recursion-limit guard
    /// that turns a pathologically nested payload into an `Err` rather than a stack overflow.
    #[test]
    fn serde_boundary_rejects_malformed_with_structured_err() {
        // an enum fed the wrong JSON kind or an unknown variant
        assert!(
            serde_json::from_str::<OpenKind>("42").is_err(),
            "OpenKind rejects a bare number"
        );
        assert!(
            serde_json::from_str::<OpenKind>(r#""notAVariant""#).is_err(),
            "OpenKind rejects an unknown variant"
        );
        assert!(
            serde_json::from_str::<PickKind>("[]").is_err(),
            "PickKind rejects an array"
        );
        assert!(
            serde_json::from_str::<IntakeOrigin>("{}").is_err(),
            "IntakeOrigin rejects an object"
        );
        assert!(
            serde_json::from_str::<RerunDecision>("true").is_err(),
            "RerunDecision rejects a bool"
        );
        assert!(
            serde_json::from_str::<DestinationChoice>("0").is_err(),
            "DestinationChoice rejects a number"
        );
        // a Uuid newtype fed a non-uuid or wrong JSON kind
        assert!(
            serde_json::from_str::<RunId>(r#""not-a-uuid""#).is_err(),
            "RunId rejects a non-uuid string"
        );
        assert!(
            serde_json::from_str::<CollectingId>("123").is_err(),
            "CollectingId rejects a number"
        );
        assert!(
            serde_json::from_str::<CollectedSetId>("null").is_err(),
            "CollectedSetId rejects null"
        );
        // OptionValue — an unknown externally-tagged variant, the §1.6 no-floats invariant, a bare string
        assert!(
            serde_json::from_str::<OptionValue>(r#"{"unknownTag":1}"#).is_err(),
            "OptionValue rejects an unknown tag"
        );
        assert!(
            serde_json::from_str::<OptionValue>(r#"{"int":1.5}"#).is_err(),
            "OptionValue::Int rejects a float (§1.6 no-floats)"
        );
        assert!(
            serde_json::from_str::<OptionValue>(r#""bareString""#).is_err(),
            "OptionValue rejects a bare string (it is externally tagged)"
        );
        // a pathologically nested payload — serde_json's recursion limit yields Err, not a stack overflow
        let deep = format!("{}{}", "[".repeat(300), "]".repeat(300));
        assert!(
            serde_json::from_str::<serde_json::Value>(&deep).is_err(),
            "serde_json enforces a recursion limit (no stack overflow on deep nesting)"
        );
        assert!(
            serde_json::from_str::<Vec<PathBuf>>(&deep).is_err(),
            "a deeply nested payload into Vec<PathBuf> is a structured Err, never a crash"
        );
    }

    /// Leg (b) `ipc_numeric_overflow`: the P0.4.3 boundary set for the one inbound numeric IPC arg
    /// (`OptionValue::Int(i64)`) round-trips through the wire form with the value preserved EXACTLY — no
    /// truncation, no wrap. Runtime companion to the module-root `deny(clippy::arithmetic_side_effects)`.
    #[test]
    fn numeric_ipc_arg_boundary_values_deserialize_exactly() {
        // the P0.4.3 NUMERIC_OVERFLOW_BOUNDARIES set: u32::MAX, i32::MIN, 0, 1, 2^16-1 (all in i64 range)
        let boundaries: [i64; 5] = [i64::from(u32::MAX), i64::from(i32::MIN), 0, 1, 65535];
        for b in boundaries {
            let wire = format!(r#"{{"int":{b}}}"#);
            let got: OptionValue =
                serde_json::from_str(&wire).expect("an in-i64-range boundary value deserializes");
            assert_eq!(
                got,
                OptionValue::Int(b),
                "OptionValue::Int preserves the boundary value {b} exactly (no wrap/truncation)"
            );
            let back = serde_json::to_string(&got).expect("OptionValue serializes");
            assert_eq!(
                back, wire,
                "OptionValue::Int({b}) re-serializes to its canonical wire form"
            );
        }
    }

    /// Leg (b) `ipc_numeric_overflow`: an integer beyond the i64 wire type is a structured `Err`, never a
    /// silent wrap or a panic — the boundary rejects it before any downstream arithmetic runs.
    #[test]
    fn numeric_ipc_arg_out_of_i64_range_is_structured_err() {
        // i64::MAX + 1, i64::MIN - 1, and a far-out-of-range literal (written literally — no arithmetic)
        for lit in [
            "9223372036854775808",
            "-9223372036854775809",
            "99999999999999999999999999",
        ] {
            let wire = format!(r#"{{"int":{lit}}}"#);
            assert!(
                serde_json::from_str::<OptionValue>(&wire).is_err(),
                "an out-of-i64-range integer literal {lit} is a structured Err, never a wrap/panic"
            );
        }
    }

    /// Leg (b) `ipc_numeric_overflow`: over the FULL i64 range, an `OptionValue::Int` deserializes and
    /// re-serializes with the value preserved exactly — proving no value in the range wraps or is rejected.
    #[test]
    fn numeric_ipc_arg_roundtrips_over_full_i64_range() {
        pinned_runner()
            .run(&any::<i64>(), |n| {
                let wire = format!(r#"{{"int":{n}}}"#);
                let parsed = serde_json::from_str::<OptionValue>(&wire);
                prop_assert!(
                    parsed.is_ok(),
                    "every i64 must deserialize into OptionValue::Int (no in-range value is rejected): {n}"
                );
                prop_assert_eq!(
                    parsed.unwrap(),
                    OptionValue::Int(n),
                    "OptionValue::Int preserves n across the wire round-trip"
                );
                Ok(())
            })
            .unwrap();
    }
}
