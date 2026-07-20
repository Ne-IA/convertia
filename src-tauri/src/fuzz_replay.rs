//! The §6.4.2 fuzz-crash REPLAY harness (G48) — every committed `fuzz/corpus/` + `fuzz/crashes/` byte fed
//! back through the REAL in-core fuzz-target functions as a plain `cargo test`, with **no libFuzzer
//! harness**. That is the whole point of the P0.5.8 convention: the instrumented `cargo-fuzz` run needs a
//! date-pinned nightly and is Linux/macOS-only (P3.73), so a crash it finds could otherwise regress
//! unnoticed on Windows. A plain integration test over the same committed bytes compiles and runs on ALL
//! three platforms under the STABLE toolchain, so a fixed crash cannot silently come back on any OS.
//!
//! The replay NEVER WRITES and never modifies anything: `detect`/`classify_encoding`/`classify_delimiter` are
//! pure over a byte slice, `resolve_identity`/`is_safe_output` only resolve/stat (on Windows `resolve_identity`
//! opens a READ handle to read the file index), and the CSV/TSV transform writes into an in-memory sink.
//! Replaying a hostile corpus therefore cannot damage anything on the machine that runs it.
//!
//! [Build-Session-Entscheidung: P3.67] **Homed at the crate root as `crate::fuzz_replay`, not as a cargo
//! `tests/` integration target.** G48 + P0.5.8 + test-strategy §1.5 name this harness
//! `tests/fuzz_replay.rs`; that name denotes **the plain `cargo test` suite as opposed to `fuzz/`** (the
//! contrast those §§ draw in every sentence: "a plain `cargo test` … with NO libFuzzer harness"), and the
//! physical home follows the crate's real shape. `convertia-core` is a BINARY crate (no `lib.rs`, main.rs
//! §0.7), so a cargo integration test under `src-tauri/tests/` links no library and cannot reach
//! `crate::detection` / `crate::fs_guard` / `crate::engines` at all — the harness would be unable to call the
//! very functions it exists to replay. The workspace-root `tests/` dir is likewise not a cargo target (the
//! P1.6 root manifest is VIRTUAL); it holds the §6.4.5 corpus data. **This is the P2.126 precedent applied
//! unchanged**: the P0.4.3 `IPC_PROPTEST_TARGETS`, contracted identically as "in `tests/`, NOT under
//! `fuzz/`", were delivered as a `#[cfg(test)]` module inside `src-tauri/src/ipc/mod.rs` and ratified there.
//! The crate-root PLACEMENT follows `crate::test_corpus` (P3.61) / `crate::test_volumes` (P3.65): this is
//! `#[cfg(test)]`-only infrastructure spanning three §0.7 tiers (`detection`, `fs_guard`, `engines`), so
//! homing it inside any one of them would invert the dependency direction the tiers express. It adds a FILE,
//! never a directory, so the §1a/§0.7 structure map (G69 asserts the DIRECTORY set) is untouched.
//!
//! **Which targets are replayed.** The G48 in-core target set is frozen at six keys in
//! `scripts/check-fuzz-contract` (`G48_FUZZ_TARGETS`). This harness drives the FOUR whose real function
//! bodies this phase delivered — `detect` (P3.29), `fs_guard_resolve_identity` (P3.6),
//! `fs_guard_is_safe_output` (P3.8), `csv_tsv` (P3.41). The remaining two are owned by the boxes that build
//! their surfaces: `imgworker_ffi` by P4.35.1 and `zip_slip` by P7.50.1, each of which extends
//! [`InCoreTarget`] with its key in the same commit that stands its target up. G48's replay sentence also
//! names the command-handler serde boundary; that surface is deliberately OUT of scope here because it is a
//! G16 `proptest` (P2.126, `crate::ipc`), not a libFuzzer target — it owns no `fuzz/` corpus to replay.
//!
//! **Forward note for P3.73 (the corpus LOCATION is load-bearing).** cargo-fuzz writes findings to
//! `fuzz/artifacts/<target>/` by default, but the P0.5.8 convention this harness implements walks
//! `fuzz/corpus/` + `fuzz/crashes/`. A crash left at the cargo-fuzz default would sit outside the replay set
//! WITHOUT tripping the vacuity guard below, so P3.73 must move each minimized crash into `fuzz/crashes/`
//! (or point `-artifact_prefix` there) as part of committing it.
//!
//! **The harness is ARMED, not a no-op.** `fuzz/` is P3.73-owned and holds no committed crash at the time
//! this harness lands, so the corpus walk legitimately finds zero files — which would make a naive replay
//! vacuously green forever. Two things close that: the G24 planted-positive below drives the SAME replay
//! engine with a target that crashes on its input and requires the engine to REPORT it (a swallowed crash
//! fails the harness itself), and the corpus walk requires a non-empty file set the moment either input
//! directory exists, so a mis-resolved root or an emptied corpus reddens instead of passing silently.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::detection::{classify_delimiter, classify_encoding, detect, ExtensionDelimiterHint};
use crate::engines::{csv_tsv_transform, CsvTsvTarget};
use crate::fs_guard::{is_safe_output, resolve_identity};

/// The workspace-root `fuzz/` tree (this crate's manifest dir is `src-tauri/`, so `fuzz/` is `../fuzz`) —
/// the P0.5.8 home of every minimized libFuzzer input, mirroring `crate::test_corpus::tests_dir`.
fn fuzz_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fuzz")
}

/// The retained libFuzzer input corpus — the coverage-growing inputs a run keeps.
fn corpus_dir() -> PathBuf {
    fuzz_dir().join("corpus")
}

/// The minimized crash artifacts — the inputs that once panicked/aborted an in-core target (P0.5.8).
fn crashes_dir() -> PathBuf {
    fuzz_dir().join("crashes")
}

/// The G48 in-core fuzz targets whose real bodies exist, keyed exactly as `check-fuzz-contract` freezes them
/// (`G48_FUZZ_TARGETS`) so a corpus laid out the cargo-fuzz way — `fuzz/corpus/<key>/…` — routes by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InCoreTarget {
    /// §1.2 content sniffing over untrusted bytes, in-core outside the §2.12 boundary.
    Detect,
    /// §2.3.1 identity resolution over an untrusted PATH (NUL bytes, overlong, symlink chains, `..`).
    FsGuardResolveIdentity,
    /// §2.3.3 write-target link-safety over an untrusted PATH (Windows device/reserved/UNC classes).
    FsGuardIsSafeOutput,
    /// §3.5.6 the in-core native CSV/TSV transform (memory-safe is not the same as panic/OOM-safe).
    CsvTsv,
}

impl InCoreTarget {
    /// Every target this harness drives. P4.35.1 (`imgworker_ffi`) and P7.50.1 (`zip_slip`) each add their
    /// key here in the commit that stands their target up.
    const ALL: [InCoreTarget; 4] = [
        InCoreTarget::Detect,
        InCoreTarget::FsGuardResolveIdentity,
        InCoreTarget::FsGuardIsSafeOutput,
        InCoreTarget::CsvTsv,
    ];

    /// The frozen `check-fuzz-contract` key — also the cargo-fuzz corpus sub-directory name.
    const fn key(self) -> &'static str {
        match self {
            InCoreTarget::Detect => "detect",
            InCoreTarget::FsGuardResolveIdentity => "fs_guard_resolve_identity",
            InCoreTarget::FsGuardIsSafeOutput => "fs_guard_is_safe_output",
            InCoreTarget::CsvTsv => "csv_tsv",
        }
    }
}

/// A libFuzzer input is a raw byte string; the two `fs_guard` targets interpret it as an OS PATH. On Unix a
/// path IS arbitrary bytes, so the conversion is BYTE-FAITHFUL — a lossy UTF-8 decode would rewrite exactly
/// the shapes G48 names (an overlong `C0 AF` becomes two `U+FFFD` replacement chars, three times the length
/// and different content), so a crash found on a non-UTF-8 path would not reproduce and this harness would
/// report green over a live regression. Windows paths are UTF-16 with no faithful byte mapping, so there the
/// lossy decode is the honest best effort. An interior NUL survives on both (the T7+T2a "never `Ok` on a
/// null-byte path" contract). P3.73's `fuzz_target!` body must use this same conversion, or the fuzzer and
/// the replay disagree about what a given corpus file means.
#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

/// The non-Unix half of the byte→path conversion — see the `cfg(unix)` sibling above for the contract.
#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

/// Feed one input to one in-core target exactly as its libFuzzer target body does. Every result is
/// discarded on purpose: the replayed invariant is "no panic / no abort on arbitrary bytes", and a
/// structured `Err` IS the correct outcome for most of this corpus.
fn drive(target: InCoreTarget, file: &Path, bytes: &[u8]) {
    match target {
        InCoreTarget::Detect => {
            let _ = detect(bytes);
            if let Some(encoding) = classify_encoding(bytes) {
                let hint = file
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .and_then(ExtensionDelimiterHint::from_extension);
                let _ = classify_delimiter(bytes, encoding, hint);
            }
        }
        InCoreTarget::FsGuardResolveIdentity => {
            let _ = resolve_identity(&path_from_bytes(bytes));
        }
        InCoreTarget::FsGuardIsSafeOutput => {
            // An empty frozen set: the fuzzed dimension is the PATH, not the source list.
            let _ = is_safe_output(&path_from_bytes(bytes), &[]);
        }
        InCoreTarget::CsvTsv => {
            // §3.5.6's PUBLIC entry point takes the source PATH and reads it itself, so this arm re-reads
            // the file rather than reusing `bytes`. That duplicate read is deliberate: driving the same
            // entry point production drives keeps the replay honest (test-strategy §0.1 — never mock the
            // thing under test), and the alternative would be widening a private fn purely for a test. Both
            // directions run — a crash can live in either the comma or the tab writer.
            for direction in [CsvTsvTarget::Csv, CsvTsvTarget::Tsv] {
                let mut out = Vec::new();
                let _ = csv_tsv_transform(file, direction, &mut out, &mut |_| {}, &mut || false);
            }
        }
    }
}

/// One replayed input that did not come through cleanly.
#[derive(Debug)]
struct ReplayFailure {
    file: PathBuf,
    target: &'static str,
    what: String,
}

/// Replay ONE input through ONE target, converting a panic into a reported failure instead of unwinding out
/// of the harness. This is the engine both the live corpus walk and the G24 planted-positive drive, so the
/// planted positive proves the exact code path the real replay uses.
fn replay_one<F>(file: &Path, target: &'static str, mut drive_one: F) -> Option<ReplayFailure>
where
    F: FnMut(&Path, &[u8]),
{
    let bytes = match std::fs::read(file) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Some(ReplayFailure {
                file: file.to_path_buf(),
                target,
                what: format!("unreadable: {error}"),
            })
        }
    };
    if catch_unwind(AssertUnwindSafe(|| drive_one(file, &bytes))).is_err() {
        return Some(ReplayFailure {
            file: file.to_path_buf(),
            target,
            what: "panicked".to_owned(),
        });
    }
    None
}

/// Every committed input under `fuzz/corpus/` + `fuzz/crashes/`, recursively, in a stable order, together
/// with any directory entry the walk could not read. An ABSENT root is the expected state before P3.73 lands
/// the `fuzz/` tree and is skipped silently; an entry that exists but cannot be walked is REPORTED, on the
/// same reasoning that reports an unreadable file — a corpus entry that quietly drops out of the replay set
/// would shrink the replay while it still reported green, and the vacuity guard only catches TOTAL emptiness.
fn committed_fuzz_inputs() -> (Vec<PathBuf>, Vec<ReplayFailure>) {
    let mut files = Vec::new();
    let mut unwalkable = Vec::new();
    for root in [corpus_dir(), crashes_dir()] {
        if !root.is_dir() {
            continue;
        }
        for entry in WalkDir::new(&root) {
            match entry {
                Ok(entry) if entry.file_type().is_file() => files.push(entry.into_path()),
                Ok(_) => {}
                Err(error) => unwalkable.push(ReplayFailure {
                    file: error.path().map_or_else(|| root.clone(), Path::to_path_buf),
                    target: "corpus-walk",
                    what: format!("unreadable directory entry: {error}"),
                }),
            }
        }
    }
    files.sort();
    (files, unwalkable)
}

/// Which targets an input is replayed through. A cargo-fuzz corpus is laid out per target
/// (`fuzz/corpus/<key>/…`), so a path component naming a known key routes precisely to it. Anything else —
/// a file at the root of `crashes/`, or one under a key this harness does not drive — is replayed through
/// EVERY driven target rather than skipped: over-driving a byte string is harmless (the invariant is
/// universal), whereas skipping one would silently drop a committed crash from the replay.
fn routed_targets(file: &Path) -> Vec<InCoreTarget> {
    let named: Vec<InCoreTarget> = InCoreTarget::ALL
        .into_iter()
        .filter(|target| {
            file.components()
                .any(|component| component.as_os_str() == target.key())
        })
        .collect();
    if named.is_empty() {
        InCoreTarget::ALL.to_vec()
    } else {
        named
    }
}

/// Replay every input through its routed targets; returns the number of (input, target) runs performed and
/// every failure found.
fn replay_all(files: &[PathBuf]) -> (usize, Vec<ReplayFailure>) {
    let mut runs = 0_usize;
    let mut failures = Vec::new();
    for file in files {
        for target in routed_targets(file) {
            runs = runs.saturating_add(1);
            if let Some(failure) = replay_one(file, target.key(), |input, bytes| {
                drive(target, input, bytes)
            }) {
                failures.push(failure);
            }
        }
    }
    (runs, failures)
}

/// The G48 cross-platform replay itself: every committed corpus/crash byte, through the in-core target
/// functions it routes to, on the stable toolchain — Windows included. A panic here means a previously-fixed
/// crash has regressed (or a newly-committed crash is still open).
#[test]
fn every_committed_fuzz_input_replays_without_a_panic() {
    let (files, unwalkable) = committed_fuzz_inputs();
    let (runs, mut failures) = replay_all(&files);
    failures.extend(unwalkable);

    assert!(
        failures.is_empty(),
        "§6.4.2/G48: {} of {runs} in-core target run(s) over {} committed fuzz input(s) still crash (or the \
         corpus walk could not read an entry): {failures:#?}",
        failures.len(),
        files.len()
    );

    // The corpus walk must not silently resolve to nothing once the tree exists: a renamed root or an
    // emptied corpus would make this replay vacuous while still reporting green.
    if corpus_dir().is_dir() || crashes_dir().is_dir() {
        assert!(
            !files.is_empty(),
            "§6.4.2/G48: fuzz/corpus or fuzz/crashes exists but the replay found no input under {} — the \
             replay would be vacuous",
            fuzz_dir().display()
        );
    }
}

/// The **G24 planted-positive**: a target that crashes on its input MUST surface as a reported replay
/// failure. Without this the replay could swallow an unwind and report green over a live crash — exactly the
/// "armed, not a no-op" property G48 requires of this harness. It drives the real [`replay_one`] engine, so
/// the arming proof covers the same code path the corpus walk uses.
#[test]
fn a_crashing_target_is_reported_as_a_replay_failure() {
    let dir = tempfile::tempdir().expect("temp dir");
    let planted = dir.path().join("crash-planted");
    std::fs::write(&planted, b"planted crash input").expect("write the planted input");

    let crashed = replay_one(&planted, "planted", |_, _| {
        // The two-lint-safe panic trigger (the P3.3 precedent): a bare `panic!` or a literal `None.unwrap()`
        // both red the deny set, so the value is laundered through `black_box` first.
        let _: u8 = std::hint::black_box(Option::<u8>::None).unwrap();
    });
    let crashed = crashed.expect("§6.4.2/G48: a crashing target must be REPORTED, never swallowed");
    assert_eq!(
        crashed.file, planted,
        "the report names the input that crashed"
    );
    assert_eq!(
        crashed.target, "planted",
        "the report names the target that crashed"
    );
    assert_eq!(
        crashed.what, "panicked",
        "the report distinguishes a panic from an unreadable input"
    );

    // The negative half — otherwise a harness that reported EVERY input as a failure would pass the leg
    // above while being equally useless.
    let clean = replay_one(&planted, "planted", |_, _| {});
    assert!(
        clean.is_none(),
        "a target that comes through cleanly must not be reported: {clean:#?}"
    );
}

/// An input the harness cannot read is reported rather than skipped — a corpus entry that vanished or is
/// unreadable must not quietly shrink the replay set.
#[test]
fn an_unreadable_input_is_reported_rather_than_skipped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let missing = dir.path().join("was-never-written");

    let failure = replay_one(&missing, "detect", |_, _| {})
        .expect("§6.4.2: an unreadable corpus entry must be reported");
    assert!(
        failure.what.starts_with("unreadable"),
        "the report distinguishes an unreadable input from a panic: {failure:#?}"
    );
}

/// The replay roots are the committed `fuzz/corpus/` + `fuzz/crashes/` dirs of the P0.5.8 convention, anchored
/// at the real workspace root. A typo'd root would find zero files on every platform and pass forever, so the
/// resolution is pinned against the filesystem — NOT against the same helpers under test, which would restate
/// their own definitions and hold for `../fuzzz` just as happily.
#[test]
fn the_replay_roots_are_the_committed_fuzz_corpus_and_crash_dirs() {
    // The anchor is real: the parent of `fuzz/` is the directory that actually holds the workspace manifest.
    let workspace_root = fuzz_dir()
        .parent()
        .map(Path::to_path_buf)
        .expect("fuzz/ has a parent");
    assert!(
        workspace_root.join("Cargo.toml").is_file(),
        "fuzz/ must be anchored at the workspace root (no Cargo.toml under {})",
        workspace_root.display()
    );
    assert!(
        workspace_root.join("tests").is_dir(),
        "fuzz/ and the §6.4.5 tests/ corpus root are siblings at the workspace root"
    );

    // The directory NAMES are pinned against literals, so a typo in any of the three helpers reddens.
    assert_eq!(
        fuzz_dir().file_name().and_then(|name| name.to_str()),
        Some("fuzz"),
        "the fuzz tree is `fuzz/` (P0.5.8)"
    );
    assert_eq!(
        corpus_dir().strip_prefix(&workspace_root).ok(),
        Some(Path::new("fuzz/corpus")),
        "the retained-input root is fuzz/corpus (P0.5.8)"
    );
    assert_eq!(
        crashes_dir().strip_prefix(&workspace_root).ok(),
        Some(Path::new("fuzz/crashes")),
        "the minimized-crash root is fuzz/crashes (P0.5.8)"
    );
}

/// The driven target keys are a real subset of the `check-fuzz-contract` `G48_FUZZ_TARGETS` freeze, read out
/// of the gate script itself — so a rename on EITHER side reddens instead of silently un-routing a corpus
/// sub-directory. Pinning only this side would leave an L(-1) key rename undetected. Reading the gate's own
/// source is the source-scan-assertion pattern the P3.66 ruling sanctioned for facts `cargo test` cannot
/// otherwise reach.
#[test]
fn the_driven_target_keys_match_the_frozen_g48_contract() {
    let gate = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/check-fuzz-contract");
    let source = std::fs::read_to_string(&gate).expect("read scripts/check-fuzz-contract");
    let list_start = source
        .find("G48_FUZZ_TARGETS = [")
        .expect("check-fuzz-contract declares G48_FUZZ_TARGETS");
    let list = source
        .get(list_start..)
        .and_then(|rest| rest.split_once("\n]"))
        .map(|(list, _)| list)
        .expect("the G48_FUZZ_TARGETS list literal terminates");
    // Each row is `("<key>", "<description>"),` — take the first quoted field of every row.
    let frozen: Vec<String> = list
        .lines()
        .skip(1)
        .filter_map(|line| line.trim().strip_prefix("(\""))
        .filter_map(|rest| rest.split_once('"'))
        .map(|(key, _)| key.to_owned())
        .collect();
    assert_eq!(
        frozen.len(),
        6,
        "G48 freezes SIX in-core targets; parsed {frozen:?} from {}",
        gate.display()
    );

    let driven: Vec<String> = InCoreTarget::ALL
        .iter()
        .map(|target| target.key().to_owned())
        .collect();
    for key in &driven {
        assert!(
            frozen.contains(key),
            "driven target {key:?} is not in the frozen G48_FUZZ_TARGETS set {frozen:?} — a key rename on \
             either side would silently un-route its corpus sub-directory"
        );
    }

    // The complement is named, so a target gaining a body without joining this harness is caught too.
    let undriven: Vec<&String> = frozen.iter().filter(|key| !driven.contains(key)).collect();
    assert_eq!(
        undriven,
        vec!["zip_slip", "imgworker_ffi"],
        "the only G48 targets this harness does not drive are the two whose bodies their own boxes build \
         (zip_slip = P7.50.1, imgworker_ffi = P4.35.1)"
    );
}

/// The fan-out layer propagates what [`replay_one`] reports: every routed (input, target) pair is counted,
/// and a failure on any of them reaches the caller rather than being lost in the loop.
#[test]
fn replay_all_counts_every_routed_run_and_propagates_its_failures() {
    let dir = tempfile::tempdir().expect("temp dir");
    let good = dir.path().join("csv_tsv").join("real-input");
    std::fs::create_dir_all(good.parent().expect("the routed parent"))
        .expect("create the routed dir");
    std::fs::write(&good, b"a,b\n1,2\n").expect("write the routed input");
    let missing = dir.path().join("never-written");

    let (runs, failures) = replay_all(&[good, missing.clone()]);
    // The routed file drives its one target; the unrouted (and absent) one drives all four.
    assert_eq!(runs, 5, "every routed (input, target) pair is counted");
    assert_eq!(
        failures.len(),
        4,
        "each of the absent input's four target runs is reported: {failures:#?}"
    );
    assert!(
        failures.iter().all(|failure| failure.file == missing),
        "the propagated failures name the input that failed: {failures:#?}"
    );
}

/// Routing: a cargo-fuzz per-target corpus path routes to that one target; anything else is replayed through
/// every driven target rather than dropped.
#[test]
fn routing_is_precise_by_directory_and_never_skips_an_input() {
    let routed = routed_targets(Path::new("fuzz/corpus/csv_tsv/input-1"));
    assert_eq!(
        routed,
        vec![InCoreTarget::CsvTsv],
        "a per-target corpus dir routes to exactly that target"
    );

    let unrouted = routed_targets(Path::new("fuzz/crashes/crash-deadbeef"));
    assert_eq!(
        unrouted,
        InCoreTarget::ALL.to_vec(),
        "an input outside a known per-target dir is replayed through every driven target, never skipped"
    );

    // A corpus dir belonging to a target this harness does not drive still replays through all four rather
    // than vanishing from the run.
    let foreign = routed_targets(Path::new("fuzz/corpus/zip_slip/entry-1"));
    assert_eq!(
        foreign,
        InCoreTarget::ALL.to_vec(),
        "a corpus dir for an undriven target is replayed, never skipped"
    );
}

/// The in-core targets come through a hostile-shaped byte set without panicking. This is the harness's own
/// teeth while `fuzz/` is P3.73-owned: it drives the same [`drive`] dispatch the corpus walk uses over the
/// adversarial shapes G48 names — an interior NUL, an over-long path, a Windows device/reserved/UNC/
/// drive-relative/trailing-dot path, and the empty input.
#[test]
fn the_driven_targets_survive_the_g48_adversarial_path_and_byte_shapes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let shapes: Vec<(&str, Vec<u8>)> = vec![
        ("empty", Vec::new()),
        ("nul_path", b"/tmp/a\0b".to_vec()),
        ("path_max_plus_1", vec![b'a'; 4097]),
        ("win_device", br"\\.\CON".to_vec()),
        ("win_reserved", b"CON.jpg".to_vec()),
        ("win_drive_relative", b"C:foo".to_vec()),
        ("win_unc", br"\\server\share\x".to_vec()),
        ("win_trailing", b"trailing. ".to_vec()),
        ("overlong_utf8", vec![0xC0, 0xAF, 0xE0, 0x80, 0xAF]),
        ("csv_recursive_quotes", br#""a""""""","b"#.to_vec()),
    ];

    for (name, bytes) in shapes {
        let file = dir.path().join(name);
        std::fs::write(&file, &bytes).expect("write the shape fixture");
        for target in InCoreTarget::ALL {
            let failure = replay_one(&file, target.key(), |input, input_bytes| {
                drive(target, input, input_bytes)
            });
            assert!(
                failure.is_none(),
                "§6.4.2/G48: the {} target must come through the {name} shape without a panic: {failure:#?}",
                target.key()
            );
        }
    }
}
