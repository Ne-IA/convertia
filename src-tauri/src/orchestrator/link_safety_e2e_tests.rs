//! §2.3/§2.4 link-safety + no-self-feeding — the **end-to-end (G31 live-path) leg** (P3.64). The frozen-set
//! de-dup, `is_safe_output` link rejection, and the write-target divert primitives are proven at the
//! **G15 unit / freeze / publish layers** (see the coverage audit below); this module adds the
//! `run_conversion`-level END-TO-END proofs those unit tests do not exercise — the layer test-strategy §6
//! calls out ("G15 is the data-structure unit leg, THIS is the end-to-end proof"). It drives the REAL C6
//! `run_conversion` conductor over real symlinks + a real temp FS (test-strategy §0.1: never mock the
//! no-harm/`fs_guard` layer, P0.5.1). This is the T7/T8 home P0.5.9 points at (§2.4.2/§2.4.3 + §2.3.3).
//! [Build-Session-Entscheidung: P3.64]
//!
//! **Coverage audit — the §2.3 primitives are proven at their own layers (asserted here by REFERENCE, not
//! duplicated end-to-end):**
//! - **§2.3.2 frozen-set de-dup** (a symlink+target / hardlink+original collapse to ONE identity → convert
//!   once): the real-FS freeze tests `resolve_dedup_unix_realfs_tests::symlink_and_target_collapse_to_one_first_seen_survivor`
//!   and `freeze_tests::hardlinked_candidates_collapse_to_one_frozen_member` (`run_conversion` iterates the
//!   ALREADY-de-duped frozen snapshot — it never re-de-dups, so the de-dup home is the freeze layer).
//! - **§2.3.3 write-target divert** (an output resolving onto a source → diverted, never clobbered):
//!   `write_sequence_tests::a_parent_resolving_onto_a_frozen_source_diverts_and_never_publishes_onto_an_original`
//!   (the publish path) + `is_safe_output_unix_tests::an_output_symlink_onto_a_source_is_rejected`.
//! - **§2.3.4 hardlink/junction identity** (dev+inode / volume-serial+file-index): the hardlink case is
//!   `fs_guard::tests::hardlink_same_inode_different_path_is_one_identity` + `is_safe_output_tests::a_hardlink_to_a_source_is_rejected`.
//!   The **Windows junction reparse-follow has NO automated test**: `resolve_identity` follows it via the
//!   winapi-util file-index identity path — the SAME volume-serial+file-index mechanism the hardlink case
//!   above already exercises — so only the reparse-SPECIFIC end-to-end is unproven, and it is owned by the
//!   §6.6 human walkthrough **+ the P3 phase-end hardening sweep (test-strategy §11)**, matching the
//!   `fs_guard::windows_realfs_tests` symlink-or-skip precedent (P3.6).
//! - **T7 "a resolved non-convertible target fails clearly per §2.8"** (test-strategy §6's third input-side
//!   sub-assertion): a symlink resolving to a non-CSV/TSV file is detected on its RESOLVED target and
//!   skipped `Unsupported` like any non-convertible source (the §1.2 detection + §1.3 grouping + §2.8 skip
//!   path — the freeze-layer detection/skip tests + the P3.60 `Unsupported`/pre-flight-refusal screen); it
//!   is not symlink-specific, so it is not re-proven end-to-end here.

use super::run_conversion_tests::{
    capture_channel, deps, eligible, non_ephemeral_source_dir, registered, run,
};
use super::*;

/// Read a published output back with the real RFC-4180 `csv` reader at the TSV delimiter (the G31
/// structural-reader bar) → record count; `expect` fails if the output is not valid tab-delimited RFC-4180.
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

/// The `.tsv` files sitting directly in `dir` (the published conversion outputs).
fn tsv_outputs(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .expect("read the source dir")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "tsv"))
        .collect();
    out.sort();
    out
}

/// Create a file symlink `link` → `target` (Unix). See the Windows variant for the unprivileged-skip note.
#[cfg(unix)]
fn make_file_symlink(target: &std::path::Path, link: &std::path::Path) -> Option<()> {
    std::os::unix::fs::symlink(target, link).expect("create the unix file symlink");
    Some(())
}

/// Create a file symlink `link` → `target`, returning `None` on an UNPRIVILEGED Windows runner (Windows
/// symlink creation needs the `SeCreateSymbolicLink` privilege / Developer Mode; `ERROR_PRIVILEGE_NOT_HELD`
/// (1314) → skip). Mirrors the established `fs_guard::windows_realfs_tests` symlink-or-skip pattern (P3.6),
/// so the symlink-source E2E COMPILES on every platform (caught by local CI/clippy) and RUNS wherever
/// symlinks are permitted (every Unix leg + a privileged Windows leg; an unprivileged Windows leg skips).
#[cfg(windows)]
fn make_file_symlink(target: &std::path::Path, link: &std::path::Path) -> Option<()> {
    let made = std::os::windows::fs::symlink_file(target, link);
    if matches!(&made, Err(e) if e.raw_os_error() == Some(1314) || e.kind() == std::io::ErrorKind::PermissionDenied)
    {
        return None; // unprivileged Windows runner → skip (the reparse-follow proof is §6.6 + §11's).
    }
    made.expect("create the windows file symlink (a non-privilege error is a real failure)");
    Some(())
}

/// §2.3 T7 (input-side symlink): a dropped SYMLINK source is resolved to its real target BEFORE the engine
/// sees it (`run_conversion` converts `ItemPaths.resolved_path` — the canonical target, §2.10.1), so the
/// conversion reads the real target, publishes a valid TSV beside it, and leaves BOTH the real target
/// (no-harm on the original, G32(a)) and the source symlink itself untouched.
#[tokio::test]
async fn a_symlink_source_is_converted_through_its_resolved_target_and_leaves_it_unchanged() {
    use crate::domain::{
        Confidence, DetectionOutcome, DroppedItem, ItemId, ItemPaths, UserFacingFormat,
    };
    use crate::fs_guard::resolve_identity;

    let Some(src_dir) = non_ephemeral_source_dir() else {
        return; // the crate root is itself ephemeral — no realistic non-ephemeral source dir to test.
    };
    let d = deps();
    let source_bytes =
        std::fs::read(crate::test_corpus::fixture("canonical.csv")).expect("read the corpus");
    // The REAL target file + a symlink to it (the "dropped" path the user selected).
    let target = src_dir.path().join("target.csv");
    std::fs::write(&target, &source_bytes).expect("write the real target file");
    let link = src_dir.path().join("link.csv");
    let Some(()) = make_file_symlink(&target, &link) else {
        return; // an unprivileged Windows runner cannot create a symlink — skip the symlink-source proof.
    };

    // A frozen item whose RAW path is the symlink but whose RESOLVED path is the real target — the §2.3
    // resolve-before-the-engine-sees-it contract (the freeze sets `resolved_path = identity.canonical_path`).
    let identity = resolve_identity(&link).expect("resolve the symlink source identity");
    let resolved = identity.canonical_path.clone();
    let item_id = ItemId::from_index(0);
    let dropped = DroppedItem {
        item: item_id,
        display_name: "link.csv".to_owned(),
        rel_path_display: None,
        size_bytes: source_bytes.len() as u64,
        detected: DetectionOutcome::Recognized {
            format: UserFacingFormat::Csv,
            confidence: Confidence::High,
            dims: None,
        },
    };
    let paths = ItemPaths {
        raw_path: link.clone(),
        resolved_path: resolved.clone(),
    };
    let set = registered(src_dir.path(), vec![(dropped, paths, identity)], Vec::new());
    let (channel, _events) = capture_channel();

    let run_id = run(&d, &set, RerunDecision::FreshCopy, src_dir.path(), &channel).await;

    // Exactly one valid TSV output was published (the conversion of the resolved target).
    let outputs = tsv_outputs(src_dir.path());
    assert_eq!(outputs.len(), 1, "exactly one TSV output was published");
    let out = std::fs::read(&outputs[0]).expect("read the published output");
    assert!(
        tsv_record_count(&out) > 0 && !out.contains(&b','),
        "G31: a valid tab-delimited TSV converted from the resolved target"
    );
    // T7 / G32(a): the REAL target is byte-identical (the engine read + converted the resolved original,
    // never harming it), and the source symlink is still a symlink pointing at the target.
    assert_eq!(
        std::fs::read(&target).expect("read the target after the run"),
        source_bytes,
        "T7/G32(a): the real symlink target is byte-identical (no-harm on the resolved original)"
    );
    assert!(
        std::fs::symlink_metadata(&link)
            .expect("stat the source symlink")
            .file_type()
            .is_symlink(),
        "the source symlink itself is untouched"
    );
    // §1.12: the summary maps the one frozen item to a Succeeded conversion.
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
    assert_eq!(
        result.items[0].item, item_id,
        "the summary maps to the frozen source item"
    );
}

/// §2.4.2 / §2.4.3 T8 (no self-feeding, live-path leg): the run iterates the FROZEN snapshot, never the live
/// directory — so files that appear in the source folder AFTER the freeze (a concurrent instance's drop; the
/// run's own beside-source output) are foreign to the batch and are NEVER ingested as new sources. The run
/// processes exactly the one frozen item, even though the source folder holds more files at conversion time.
#[tokio::test]
async fn the_run_processes_only_the_frozen_snapshot_not_files_appearing_after_the_freeze() {
    let Some(src_dir) = non_ephemeral_source_dir() else {
        return; // the crate root is itself ephemeral — no realistic non-ephemeral source dir to test.
    };
    let d = deps();
    let source_bytes =
        std::fs::read(crate::test_corpus::fixture("canonical.csv")).expect("read the corpus");
    let (dropped, paths, identity) = eligible(src_dir.path(), "data.csv", 0, &source_bytes);
    let source = paths.resolved_path.clone();
    // The FREEZE: the batch's frozen set is captured here (one item), before any conversion.
    let set = registered(src_dir.path(), vec![(dropped, paths, identity)], Vec::new());

    // §2.4.3: a concurrent instance drops a NEW, well-formed CSV into the SAME source folder AFTER the freeze.
    // Being absent from this run's snapshot, it must never be ingested as a source (SSOT).
    let concurrent = src_dir.path().join("dropped-by-another-instance.csv");
    let concurrent_bytes: &[u8] = b"x,y\n9,9\n";
    std::fs::write(&concurrent, concurrent_bytes)
        .expect("a concurrent instance drops a file post-freeze");
    let (channel, _events) = capture_channel();

    let run_id = run(&d, &set, RerunDecision::FreshCopy, src_dir.path(), &channel).await;

    // Non-vacuity: the source folder holds MORE than the one frozen source at conversion time (the concurrent
    // CSV + the run's own beside-source `.tsv` output) — a live re-walk would see extra convertible files.
    let csv_count = std::fs::read_dir(src_dir.path())
        .expect("read the source dir")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "csv"))
        .count();
    assert!(
        csv_count >= 2,
        "the source folder holds the frozen source AND the post-freeze concurrent CSV at run time"
    );

    // §2.4.2/§2.4.3: the batch did NOT expand — exactly the ONE frozen source was processed. The output
    // landing beside the source (its own `.tsv`) and the concurrent CSV are foreign to the frozen snapshot.
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
        },
        "§2.4.2/§2.4.3: the snapshot batch did not expand — the post-freeze files were never ingested"
    );
    assert_eq!(
        result.items.len(),
        1,
        "exactly the one frozen source is in the summary"
    );
    // G31: the one frozen source produced exactly one valid TSV output (the concurrent CSV produced none).
    let outputs = tsv_outputs(src_dir.path());
    assert_eq!(
        outputs.len(),
        1,
        "the one frozen source produced exactly one TSV output"
    );
    assert!(
        tsv_record_count(&std::fs::read(&outputs[0]).expect("read the published output")) > 0,
        "G31: the published output is valid tab-delimited RFC-4180"
    );
    // The concurrent file was never read/converted/harmed, and the frozen source is byte-identical.
    assert_eq!(
        std::fs::read(&concurrent).expect("read the concurrent file after the run"),
        concurrent_bytes,
        "the foreign concurrent file was never ingested or harmed"
    );
    assert_eq!(
        std::fs::read(&source).expect("read the frozen source after the run"),
        source_bytes,
        "G32(a): the frozen source is byte-identical"
    );
}
