//! §6.4.3 per-pair integration E2E for the §0.4.1 C6 conversion command `start_conversion` (P3.63) — the
//! walking-skeleton's first per-pair integration RUNNER. It drives the REAL native CSV→TSV conversion
//! end-to-end through the §1.9 conductor [`run_conversion`](super::run_conversion) — the run path C6
//! `start_conversion` delegates to after resolving its managed State — against a real temp FS + the real
//! in-core engine (test-strategy §0.1: the conversion IS the product, never mock the no-harm/`fs_guard`
//! layer under test, P0.5.1).
//!
//! **Why this file is `*_tests.rs`:** it is the G23 conversion-command→test partner for `start_conversion`.
//! The G23 walk keys a `#[tauri::command]` conversion handler to a partner test that references it by name
//! in a `_TEST_PATH_RE`-matching file; this suite both NAMES `start_conversion` (the mechanical floor,
//! [`covers_the_c6_start_conversion_command`]) and genuinely DRIVES its run path (the box's own bar). Per
//! the 2026-07-17 P3.63 Decision note, this fill lands FIRST (green under the still-`convert_*`-keyed gate,
//! which does not match `start_conversion`); the owner-acked L(-1) re-key of G23 off `convert_*` onto the
//! §0.4.1 conversion-command set `{start_conversion}` then flips G23 live AND green against it.
//!
//! **§6.5 reliability:** these passing tests are what make the CSV→TSV pair `reliable` (§6.5.1); the §6.5.2
//! pair-status ledger (`reliability-report.json`) that RECORDS the cell is the **P4.61** generator — it does
//! not exist in P3 (roles §5a), so this box authors the test; the P4.61 generator marks the pair.
//!
//! Reuses the [`run_conversion_tests`](super::run_conversion_tests) `pub(super)` C6-path harness so the
//! conductor harness has one home. [Build-Session-Entscheidung: P3.63]

use super::run_conversion_tests::{
    capture_channel, deps, eligible, non_ephemeral_source_dir, registered, run,
};
use super::*;
use crate::domain::ItemId;
use crate::test_corpus::fixture;

/// Read the published output back with the REAL RFC-4180 `csv` reader at the TSV delimiter (the G31
/// structural-reader bar — not a byte-blob / magic-sniff) and return the record count; `expect` fails the
/// test if the produced file does not parse as valid tab-delimited RFC-4180.
fn tsv_record_count(output: &[u8]) -> usize {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(true)
        .from_reader(output);
    let mut record = csv::ByteRecord::new();
    let mut records = 0usize;
    while reader
        .read_byte_record(&mut record)
        .expect("the published output parses as valid RFC-4180 TSV")
    {
        records = records.saturating_add(1);
    }
    records
}

/// A fn-item reference PINNING the §0.4.1 C6 conversion command this suite is the partner test for:
/// `start_conversion` (the real `#[tauri::command]` in `crate::ipc::conversion`, whose run path is the
/// [`run_conversion`](super::run_conversion) conductor the tests below drive). A rename of the command fails
/// to compile here, and the reference is the G23 conversion-command→test name-match's mechanical floor once
/// G23 re-keys off `convert_*` (the 2026-07-17 P3.63 Decision note). [Build-Session-Entscheidung: P3.63]
#[test]
fn covers_the_c6_start_conversion_command() {
    let _c6_command_under_test = crate::ipc::conversion::start_conversion;
}

/// §6.4.3 (G31/G32(a)/§1.12): the real CSV→TSV vertical slice publishes a VALID tab-delimited TSV BESIDE the
/// source at the expected `stem.tsv` name (§2.2), never harms the source, and the §1.12 `RunResult` summary
/// maps the output back to its source — the wire `ItemResult.item` → the frozen source item, `output_display`
/// → the published output, and the off-wire `RunResultPaths.item_outputs` → the real beside-source path.
#[tokio::test]
async fn slice_publishes_beside_source_and_the_summary_maps_output_to_source() {
    let Some(src_dir) = non_ephemeral_source_dir() else {
        return; // the crate root is itself ephemeral — no realistic non-ephemeral source dir to test.
    };
    let d = deps();
    // A REAL §6.4.5 corpus source (canonical.csv) driven through the full C6 run path.
    let source_bytes = std::fs::read(fixture("canonical.csv")).expect("read the corpus source");
    let (dropped, paths, identity) = eligible(src_dir.path(), "canonical.csv", 0, &source_bytes);
    let source = paths.resolved_path.clone();
    let item_id: ItemId = dropped.item;
    let set = registered(src_dir.path(), vec![(dropped, paths, identity)], Vec::new());
    let (channel, _events) = capture_channel();

    let run_id = run(&d, &set, RerunDecision::FreshCopy, src_dir.path(), &channel).await;

    // §2.2: the output is published BESIDE the source at the expected `stem.tsv` name.
    let output = src_dir.path().join("canonical.tsv");
    let out =
        std::fs::read(&output).expect("the TSV output is published beside the source at stem.tsv");
    // G31: read back with the real RFC-4180 reader — a valid tab-delimited TSV with at least one record.
    assert!(
        tsv_record_count(&out) > 0,
        "G31: the published TSV decodes to at least one record via the real RFC-4180 reader"
    );
    assert!(
        !out.contains(&b','),
        "the output is tab-delimited (the CSV comma delimiter is gone)"
    );
    // G32(a) no-harm: the source is byte-identical after the conversion.
    assert_eq!(
        std::fs::read(&source).expect("read the source after the run"),
        source_bytes,
        "G32(a): the source file is byte-identical (no-harm)"
    );

    // §1.12: the summary maps output→source. One Succeeded row whose `item` is the frozen source item and
    // whose `output_display` names the published output.
    let result = d
        .results
        .get(run_id)
        .expect("the terminal RunResult is retained for the C8 re-serve");
    assert_eq!(
        result.totals,
        Totals {
            succeeded: 1,
            failed: 0,
            cancelled: 0,
            skipped: 0,
        }
    );
    assert_eq!(result.items.len(), 1, "one item in the summary");
    let row = &result.items[0];
    assert_eq!(
        row.item, item_id,
        "§1.12: the summary row maps to the source item (the output→source anchor)"
    );
    assert!(
        matches!(row.state, JobState::Succeeded),
        "the converted item is Succeeded"
    );
    assert!(
        row.output_display
            .as_deref()
            .is_some_and(|display| display.ends_with("canonical.tsv")),
        "output_display names the published beside-source output, got {:?}",
        row.output_display
    );
    // Off-wire real-path mapping (§0.4.4): `item_outputs` maps the source item → the REAL beside-source path
    // (the C9 `OpenTarget::Item` file-launch target), completing the output→source map at the path level.
    let run_paths = d
        .results
        .current_paths()
        .expect("the off-wire RunResultPaths are retained alongside the wire summary");
    assert_eq!(
        run_paths.item_outputs.get(&item_id),
        Some(&output),
        "item_outputs maps the source item to the real published beside-source output path"
    );
}

/// §2.1/§2.2.1 no-clobber: a pre-existing, UNRELATED file at the beside-source output name is NEVER
/// overwritten — the exclusive publish falls through to the space-paren `stem (1).tsv` numbered variant
/// (§2.2.1), the pre-existing file stays byte-identical (no-harm), and the summary maps the source item to
/// the numbered output.
#[tokio::test]
async fn a_pre_existing_output_collision_is_never_clobbered_and_numbers() {
    let Some(src_dir) = non_ephemeral_source_dir() else {
        return; // the crate root is itself ephemeral — no realistic non-ephemeral source dir to test.
    };
    let d = deps();
    let source_bytes = std::fs::read(fixture("canonical.csv")).expect("read the corpus source");
    let (dropped, paths, identity) = eligible(src_dir.path(), "canonical.csv", 0, &source_bytes);
    let item_id: ItemId = dropped.item;
    let set = registered(src_dir.path(), vec![(dropped, paths, identity)], Vec::new());
    // Seed a pre-existing UNRELATED file at the beside-source output name. The §2.1 exclusive
    // create-new-or-fail publish must never clobber it (nothing on disk is a re-run of THIS session's
    // ledger, §2.5.2, so it is an ordinary §2.2 collision → silent numbering, never an overwrite).
    let pre_existing = src_dir.path().join("canonical.tsv");
    let pre_existing_bytes: &[u8] = b"pre-existing unrelated content\tnot the conversion output\n";
    std::fs::write(&pre_existing, pre_existing_bytes)
        .expect("seed the pre-existing collision file");
    let (channel, _events) = capture_channel();

    let run_id = run(&d, &set, RerunDecision::FreshCopy, src_dir.path(), &channel).await;

    // No-harm: the pre-existing beside-source file is byte-identical (never clobbered).
    assert_eq!(
        std::fs::read(&pre_existing).expect("read the pre-existing file after the run"),
        pre_existing_bytes,
        "§2.1: the pre-existing beside-source file was NOT clobbered by the exclusive publish"
    );
    // §2.2.1: the conversion output took the space-paren numbered variant.
    let numbered = src_dir.path().join("canonical (1).tsv");
    let out = std::fs::read(&numbered)
        .expect("the output published to the `stem (1).tsv` numbered variant, not by overwriting");
    assert!(
        tsv_record_count(&out) > 0 && !out.contains(&b','),
        "the numbered output is a valid tab-delimited TSV"
    );
    // The summary maps the source item to the numbered output (both wire display + off-wire real path).
    let result = d
        .results
        .get(run_id)
        .expect("the terminal RunResult is retained");
    assert_eq!(
        result.totals,
        Totals {
            succeeded: 1,
            failed: 0,
            cancelled: 0,
            skipped: 0,
        }
    );
    let row = &result.items[0];
    assert_eq!(row.item, item_id, "the summary row maps to the source item");
    assert!(
        row.output_display
            .as_deref()
            .is_some_and(|display| display.ends_with("canonical (1).tsv")),
        "output_display maps to the numbered variant, got {:?}",
        row.output_display
    );
    let run_paths = d
        .results
        .current_paths()
        .expect("the off-wire RunResultPaths are retained");
    assert_eq!(
        run_paths.item_outputs.get(&item_id),
        Some(&numbered),
        "item_outputs maps the source item to the real numbered output path"
    );
}
