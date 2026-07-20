//! §6.7.1/§6.1.3 (G15/G30) the CROSS-OS DE-RISK SMOKE (P3.66) — the C1→C6→C8 round trip driven end to end
//! on a real filesystem, so the Tauri + atomic-publish + IPC stack is proven on Windows, macOS AND Linux
//! before any heavy engine lands (the README walking-skeleton purpose).
//!
//! **Why it lives here.** The round trip SPANS three commands homed in two sibling modules (`ipc::intake`'s
//! C1, `ipc::conversion`'s C6/C8), so it belongs at the span's own level — beside the other cross-cutting
//! C-surface suites in `crate::ipc` (`c_surface_scan`, `responsiveness_contract`, `error_shape_contract`,
//! `camel_case_wire_contract`, `ipc_boundary_proptest`) rather than inside one participant. It is a FILE
//! rather than an inline `mod` (unlike those five) because it carries a real-FS fixture and a full run;
//! G69 asserts the DIRECTORY set, so a file adds nothing structural.
//! [Build-Session-Entscheidung: P3.66 — module name + file placement]
//!
//! **What it drives, and what it deliberately does not.** The three `#[tauri::command]` handlers are NOT
//! callable from `cargo test`: C1/C6 bind an `AppHandle` and C8 takes a `State<'_, …>` whose constructor is
//! private to tauri, and this crate ships NO `tauri::test` mock harness BY OWNER DECISION (test-strategy
//! §1.1a). The sanctioned substitute is that decision's own boot-glue split — the handlers are thin
//! AppHandle/State resolvers over AppHandle-free helpers, and those helpers are the test surface:
//!
//! | leg | driven here | the handler above it |
//! |---|---|---|
//! | C1 `drain_intake` | [`drain_to_collected_set`](super::intake::drain_to_collected_set) | resolves 5 stores, `spawn_blocking` |
//! | C6 `start_conversion` | [`run_conversion`](crate::orchestrator::run_conversion) | `start_run` — AppHandle boot-glue (G28-exempt) |
//! | C8 `get_run_summary` | [`resolve_run_summary`](super::conversion::resolve_run_summary) | one `State` extractor |
//!
//! So this is the widest slice `cargo test` can reach; the handler shells above it are pinned structurally by
//! the sibling `c_surface_scan` legs, and their live behaviour is the §1.6 E2E level.
//!
//! **The per-OS publish primitive (the box's second clause).** "Exercised and green" binds at THIS box's
//! level: each OS's primitive must be reached through the PRODUCT path on its own leg — which the round trip
//! below delivers, because C6 publishes through `fs_guard::atomic_publish` → `publish_once`, whose Unix arm
//! is `rustix::renameat_with(RenameFlags::NOREPLACE)` (Linux `renameat2` / macOS `renameatx_np`, one call
//! resolved at COMPILE time) and whose Windows arm is `FileRenameInformationEx`. Reading the output back
//! therefore proves that OS's primitive ran, on that OS's runner. Which of the two Unix spellings ran is not
//! runtime-observable and is not asserted; nor is single-call-vs-fallback, which `publish_once` collapses
//! into one `SinglePublish::Published` — so the single-call-FIRST ordering is pinned STRUCTURALLY instead
//! (the source-scan test in this module), the §1.1a pattern — cited by MODULE, never by `#[test] fn` name,
//! which a rename would silently strand (the P2.136/G73 convention).
//! [Decision: P3.66, 2026-07-18 — clause (2), reading (a) sharpened; no production change for observability]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tauri::ipc::{Channel, InvokeResponseBody};
use tokio_util::sync::CancellationToken;

use crate::domain::{
    CollectedSet, CollectingId, InstanceId, IntakeOrigin, OptionValues, RerunDecision,
    ResolvedDestination, RunId,
};
use crate::orchestrator::{
    build_batch, run_conversion, CollectedSetRegistry, EquivKeyComputer, FrontendReady,
    IngestRegistry, PendingIntake, RunRegistry, RunResultStore,
};
use crate::pool::Pool;
use crate::run::{RerunLedger, RunScratch};

/// A writable, NON-ephemeral directory under the crate source root — a realistic user-source placement. A
/// plain `tempfile::tempdir()` lives under the OS temp root, which the conductor's §2.7.2 `location_status`
/// correctly classifies `Ephemeral` → DIVERT, so a beside-source publish must run from a non-ephemeral dir.
/// `None` on the pathological environment where the crate root is itself under an OS temp root (a clean skip,
/// never a false pass).
///
/// Re-stated here rather than reused. TWO twins exist and both are closed — for DIFFERENT reasons, stated
/// precisely because they are not interchangeable:
///
/// - `crate::orchestrator`'s twin: cross-tier TEST-helper coupling. Importing an orchestrator test-module
///   item into `crate::ipc` would make an ipc suite fail whenever an orchestrator fixture is refactored. It
///   is `#[cfg(test)]`, so widening it would open no production surface — the ruling's cap, which governs
///   production IPC-funnel internals, never counted it either way.
/// - `ipc::planning`'s twin: it sits in a PRIVATE `mod support`, so reuse needs a further visibility grant
///   INSIDE `crate::ipc` — and it is that IN-TIER count the ruling fixed when it found "exactly these two
///   widenings suffice … there is no hidden third". A third in-tier grant would falsify that finding.
///
/// Six lines of duplication, no coupling and no re-opened ruling. [Build-Session-Entscheidung: P3.66]
fn non_ephemeral_source_dir() -> Option<tempfile::TempDir> {
    let dir = tempfile::Builder::new()
        .prefix("convertia-p366-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .expect("create a temp dir in the crate source root");
    (!crate::platform::is_ephemeral_output_dir(dir.path())).then_some(dir)
}

/// Parse `bytes` with the REAL RFC-4180 reader at `delimiter` and return every record's FIELD BYTES — the
/// G31 structural-reader bar, in its falsifiable form.
///
/// Two weaker forms were tried and rejected, both recorded so the bar is not silently walked back:
/// a bare "it parsed / record count > 0" is NOT falsifiable here (`has_headers(false)` + `flexible(true)`
/// make `read_byte_record` succeed on ANY non-empty stream, so an engine regressing to a byte-through copy
/// would read at the TAB delimiter as one single-field record per line and still pass); and comparing only
/// per-record FIELD COUNTS proves the geometry survived but says nothing about content, so a value mangled
/// inside a field would pass. Comparing the parsed FIELDS makes "only the delimiter changed" literally what
/// is asserted — and, unlike a `!contains(b',')` byte scan, it stays correct when the corpus grows a quoted
/// field holding a literal comma (both sides parse that field to the same bytes).
///
/// SCOPE, so a future reader cannot misread a legitimate red: field-BYTE equality is sound only for a UTF-8,
/// BOM-free source. The §2.10.2 transform decodes to UTF-8 and strips a BOM, so pointing this smoke at
/// `utf16le_bom.csv` / `cp1252.csv` (both in the corpus) would compare PRE-transcode source bytes against
/// POST-transcode output bytes and fail on a CORRECT conversion — such a row must compare post-transcode.
/// [Build-Session-Entscheidung: P3.66]
fn record_fields(bytes: &[u8], delimiter: u8) -> Vec<Vec<Vec<u8>>> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(bytes);
    let mut record = csv::ByteRecord::new();
    let mut records = Vec::new();
    while reader
        .read_byte_record(&mut record)
        .expect("the parsed file is valid RFC-4180 at this delimiter")
    {
        records.push(record.iter().map(<[u8]>::to_vec).collect());
    }
    records
}

/// A `CollectingId` for this suite — its PUBLIC bare-uuid wire form (the FRONTEND mints the ingest id,
/// §0.4.4, so there is no core-side `mint`), mirroring the `c1_contract`/`c13_contract` helpers.
fn collecting_id() -> CollectingId {
    serde_json::from_str(r#""33333333-3333-4333-8333-333333333333""#)
        .expect("CollectingId deserializes from a uuid string")
}

/// A `Channel<T>` that records every JSON body the run emits — an intentional independent copy of the
/// orchestrator suite's twin, for the same cross-tier TEST-helper-coupling reason recorded on
/// [`non_ephemeral_source_dir`]; `record_fields` above is likewise independent of (and a strictly stronger bar
/// than) that suite's `tsv_record_count`. [Build-Session-Entscheidung: P3.66]
///
/// Records every JSON body the run emits — the §0.4.2 event sink the commands would hand
/// the WebView. The round trip asserts on BOTH the returned summary and this stream: the Channel is part of
/// the C6 product path (§1.11 — the run returns immediately and carries all telemetry here), so a round trip
/// that ignored it would leave half of C6's contract unproven.
fn capture_channel<T: serde::Serialize + Send + Sync + 'static>(
) -> (Channel<T>, Arc<Mutex<Vec<String>>>) {
    let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    let channel = Channel::new(move |body: InvokeResponseBody| {
        if let InvokeResponseBody::Json(json) = body {
            sink.lock().expect("event sink lock").push(json);
        }
        Ok(())
    });
    (channel, events)
}

// §6.7.1/§6.1.3 (G15/G30) THE CROSS-OS SLICE ROUND TRIP: a real CSV is stashed into the §7.8.1 intake buffer
// exactly as a drop/launch/picker would, C1 drains + freezes it, C6's conductor converts + publishes it
// through the §2.1 atomic publish, and C8 re-serves the §1.12 wire summary. Every leg is real — real temp FS,
// real freeze, real in-core CSV→TSV engine, real per-OS publish primitive, real store retention.
//
// This is the box's per-OS proof: the SAME test body runs on all three `gate-tooling` legs
// [ubuntu-22.04, macos-14, windows-2022], so a green here on each runner means that OS's create-only publish
// primitive was reached THROUGH THE PRODUCT PATH and produced a readable output — not merely that a
// primitive-level unit test compiled there. [Build-Session-Entscheidung: P3.66]
#[tokio::test]
async fn the_slice_round_trips_from_c1_intake_through_c6_conversion_to_the_c8_summary() {
    let Some(src_dir) = non_ephemeral_source_dir() else {
        return; // the crate root is itself ephemeral — no realistic non-ephemeral source placement.
    };
    // A REAL §6.4.5 corpus source, copied to the realistic source location.
    let source_bytes = std::fs::read(crate::test_corpus::fixture("canonical.csv"))
        .expect("read the corpus source");
    let source = src_dir.path().join("canonical.csv");
    std::fs::write(&source, &source_bytes).expect("place the real source file");

    // ── C1 `drain_intake` — the §7.8.1 funnel + the §1.1/§2.4 freeze ──
    let pending = PendingIntake::default();
    let ready = FrontendReady::default();
    let ingest_registry = IngestRegistry::default();
    let collected_sets = CollectedSetRegistry::default();
    let instance = InstanceId::mint();
    // What a drop / launch-argv / picker stashes, verbatim: the paths + their origin.
    pending.stash(vec![source.clone()], IntakeOrigin::Drop);
    ready.mark_ready();
    let (scan_channel, scan_events) = capture_channel();
    let collected = crate::ipc::intake::drain_to_collected_set(
        &pending,
        &ready,
        &ingest_registry,
        &collected_sets,
        collecting_id(),
        &scan_channel,
        instance,
    );
    // Bind via match→Option→`expect` — never a hard-fail macro: the crate's no-panic policy denies `panic!`
    // even in tests (G4/G14), and this is the established `fs_guard::atomic_publish_tests::verified` idiom.
    let set_id = match collected {
        CollectedSet::Single { id, .. } => Some(id),
        // Enumerated, never a wildcard (the crate denies `clippy::wildcard_enum_match_arm`): a new §0.6
        // funnel outcome must be classified here deliberately, not silently swallowed as "not Single".
        CollectedSet::Mixed { .. }
        | CollectedSet::Unsupported { .. }
        | CollectedSet::Uncertain { .. }
        | CollectedSet::Empty { .. } => None,
    }
    .expect("§1.3: one real CSV, freshly frozen, yields a Single collected set");
    let registered = collected_sets
        .take(set_id)
        .expect("§0.4.4: C1 registered the frozen set under its CollectedSetId");

    // §0.4.1/§0.4.2: C1 is the sole walk/freeze/`onScan` carrier, and `ScanThrottle::finish` emits
    // unconditionally, so the drain must have streamed at least the terminal count.
    assert!(
        !scan_events.lock().expect("scan sink lock").is_empty(),
        "§0.4.2: the C1 drain streamed its ScanProgress count over the onScan Channel"
    );

    // ── C6 `start_conversion` — the §1.9 conductor over the frozen set ──
    let target = crate::engines::slice_target(crate::domain::UserFacingFormat::Csv)
        .expect("§1.5: the CSV slice source resolves its TSV target");
    let batch = build_batch(
        &registered.frozen,
        target,
        OptionValues(BTreeMap::new()),
        ResolvedDestination::BesideSource,
    );
    let scratch_base = tempfile::tempdir().expect("scratch base dir");
    let run_id = RunId::mint();
    let scratch = RunScratch::acquire(scratch_base.path(), instance, std::process::id(), run_id)
        .expect("acquire the run scratch (lock held)");
    let results = RunResultStore::default();
    let runs = RunRegistry::default();
    let (progress_channel, events) = capture_channel();
    run_conversion(
        batch,
        &registered,
        run_id,
        CancellationToken::new(),
        scratch,
        instance,
        src_dir.path().to_path_buf(),
        RerunDecision::FreshCopy,
        &Pool::new(),
        &RerunLedger::default(),
        &EquivKeyComputer::default(),
        &results,
        &runs,
        &progress_channel,
    )
    .await;

    // §2.1/§2.2: the output was PUBLISHED beside the source at `stem.tsv` — on this runner's OS, through this
    // OS's create-only publish primitive (the box's "exercised and green" clause).
    let output = src_dir.path().join("canonical.tsv");
    let out = std::fs::read(&output).expect(
        "§2.1.2: the per-OS create-only publish primitive published the output beside the source",
    );
    // G31 output-validity: read the published bytes back with the REAL RFC-4180 reader and compare the parsed
    // FIELDS against the source CSV's. This is the falsifiable form — a byte-through copy parses at the tab
    // delimiter as one single-field record per line and fails here, which a record-count-only check would wave
    // through, and a value mangled inside a field fails too (see `record_fields`).
    let output_fields = record_fields(&out, b'\t');
    assert!(
        !output_fields.is_empty(),
        "G31: the published TSV parses to at least one record"
    );
    assert_eq!(
        output_fields,
        record_fields(&source_bytes, b','),
        "G31: the published TSV carries the SAME records and field values as the source CSV — only the delimiter changed"
    );
    // G32(a) no-harm: the source survives the round trip byte-identical.
    assert_eq!(
        std::fs::read(&source).expect("read the source after the run"),
        source_bytes,
        "G32(a): the source file is byte-identical after the slice round trip"
    );

    // §0.4.2/§1.11: the run's telemetry Channel carried the ordered event stream C6 promises — RunStarted
    // first, a terminal RunFinished last, with the per-item ItemFinished between them. Half of C6's contract
    // lives on this Channel (the command returns the RunId immediately), so the round trip closes on it too.
    let events = events.lock().expect("event sink lock").clone();
    let tag = |needle: &str| events.iter().position(|e| e.contains(needle));
    let run_started = tag(r#""type":"runStarted""#).expect("§0.4.2: a RunStarted event");
    let item_finished = tag(r#""type":"itemFinished""#).expect("§0.4.2: an ItemFinished event");
    let run_finished = tag(r#""type":"runFinished""#).expect("§0.4.2: a RunFinished event");
    assert_eq!(run_started, 0, "§0.4.2: RunStarted is the FIRST event");
    assert_eq!(
        run_finished,
        events.len().saturating_sub(1),
        "§0.4.2: RunFinished is the LAST event"
    );
    assert!(
        run_started < item_finished && item_finished < run_finished,
        "§0.4.2: the per-item outcome is streamed between the run's start and its terminal event"
    );

    // ── C8 `get_run_summary` — the §1.12 wire projection, re-served from the §0.4.4 store ──
    let summary = crate::ipc::conversion::resolve_run_summary(&results, run_id)
        .expect("§0.4.4: C8 re-serves the retained summary for a finished run");
    assert_eq!(
        summary.totals.succeeded, 1,
        "§1.12: the one converted item is reported succeeded, got {:?}",
        summary.totals
    );
    assert_eq!(
        summary.totals.failed + summary.totals.cancelled + summary.totals.skipped,
        0,
        "§1.12: nothing failed, was cancelled or skipped, got {:?}",
        summary.totals
    );
    // §1.12/§2.7: the run published BESIDE THE SOURCE — not diverted. Without this, the output-path assertion
    // could not tell a beside-source publish from a divert that happened to land in the same directory (the
    // divert root handed to the conductor IS the source dir, mirroring the P3.63 harness).
    assert!(
        summary.divert_root_display.is_none(),
        "§1.12: nothing diverted — the output was published beside its source, got {:?}",
        summary.divert_root_display
    );
    assert!(
        summary.items.len() == 1
            && summary.items[0]
                .output_display
                .as_deref()
                .is_some_and(|display| display.ends_with("canonical.tsv")),
        "§1.12: the summary maps the one item to the published output, got {:?}",
        summary.items
    );

    // C8 on an UNKNOWN run id is the §2.13 InternalError not-available result, never a panic — the same
    // resolve the handler wraps, so the round trip closes on both of C8's arms.
    assert!(
        crate::ipc::conversion::resolve_run_summary(&results, RunId::mint()).is_err(),
        "§0.4.3: an unresolvable run id resolves to a structured IpcError"
    );
}

// §2.1.2 (G15/G30, P3.66) THE SINGLE-CALL-FIRST ORDERING, PINNED STRUCTURALLY. `publish_once` must try the
// single-call no-replace primitive FIRST and reach the portable `link`+`unlink` fallback ONLY on its
// `Unsupported` verdict (§2.1.2: the fallback exists for filesystems lacking the flag, not as a co-equal
// path). That ordering is invisible at runtime — `publish_once` collapses both successes into the same
// `SinglePublish::Published { residual_tmp: false }` — and adding a discriminating arm purely for a test
// would be test-driven production complexity for a distinction the product does not act on. So it is pinned
// on the SOURCE, the §1.1a pattern sanctioned for surfaces `cargo test` cannot reach: a silent re-order to
// fallback-first reddens here, which is the invariant a runtime bar was really after.
//
// Scanned over the PRODUCTION PREFIX of `fs_guard` (everything before its first `#[cfg(test)]`), so a needle
// can never match one of that module's own tests. Runs on EVERY platform even though the chain it pins is
// Unix-only (Windows' `publish_once` has no `Unsupported` arm at all, §2.1.2): the assertion reads SOURCE
// TEXT, which is identical on every runner, so cfg-gating it to Unix would only make the invariant
// unverifiable on a Windows workstation while proving nothing extra on Linux.
// [Build-Session-Entscheidung: P3.66]
#[test]
fn the_publish_primitive_tries_the_single_call_before_the_link_fallback() {
    let src = super::c_surface_scan::production_prefix(include_str!("../fs_guard/mod.rs"));
    let single_call = src
        .find("publish_noreplace(parent, tmp, leaf)?")
        .expect("§2.1.2: publish_once calls the single-call no-replace primitive");
    let fallback = src
        .find("publish_link_fallback(parent, tmp, leaf)?")
        .expect("§2.1.2: publish_once carries the portable link+unlink fallback");
    assert!(
        single_call < fallback,
        "§2.1.2: the single-call no-replace primitive is tried FIRST; the link+unlink fallback follows it"
    );
    assert!(
        src[single_call..fallback].contains("PublishAttempt::Unsupported =>"),
        "§2.1.2: the link+unlink fallback is reached ONLY from the single-call `Unsupported` verdict, \
         never as a co-equal first choice"
    );
}
