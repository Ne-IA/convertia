//! `crate::prefs` — the §7.4 persistence layer: the 3-key `settings.json` prefs blob, the ONLY state
//! ConvertIA persists (§7.4.1 `[DECIDED]`: cosmetic / convenience / diagnostic values only — never anything
//! derived from the user's files). Owned directly (§7.4.2: `std::fs` + `serde_json`, no plugin) — this module
//! is the file's sole owner. The blob is **best-effort and never load-bearing** (§7.4.2): an unreadable /
//! oversize / non-object file yields the defaults plus one log line (§7.5), an absent file (the first launch)
//! the defaults silently, and a wrong-typed key its own default; it NEVER blocks a conversion or surfaces an
//! error (§2).
//!
//! [Build-Session-Entscheidung: P2.85] Home — a binary-root LEAF module (`src-tauri/src/prefs.rs`), a
//! sibling of `main.rs`. §0.7 homes no app-shell / persistence tier module (the §7.2.1 startup "spine" is
//! `main.rs` wiring, and `StartupContext` sets the precedent of parking app-shell state at the binary
//! root); §7.4 persistence is app-shell state with no tier, so it lives here rather than being forced into
//! an unrelated tier module or bloating `main.rs`. A leaf FILE adds no directory, so it is inert to the
//! §1a / §0.7 structural map (G69) and needs no §0.7 physical-tree row (that tree lists dirs + notable
//! seams, not every module file).
//!
//! Scope of this box — the typed 3-key model, its defaults, the tolerant parse, and the config-dir-resolved
//! `load`. The downstream READERS are separate boxes: `lastDestinationMode` use + re-validation (P2.88 /
//! §2.7.2), the `verboseLog` startup read (P2.89 / P2.94), `theme` (§5.5, served to the WebView by a typed
//! core command, §7.4.2). No plugin is involved; the WebView holds no store grant (§0.10).

// [Test-Change: P2.94 — old-obsolete+new-correct, §7.5.3] The former module-level
// `#![cfg_attr(not(test), expect(dead_code, …))]` is REMOVED. P2.94's `resolve_log_verbosity` reads
// `load(app).verbose_log` (§7.5.3) — the first PRODUCTION reader the model was waiting on — so `load` and
// the whole 3-key model are now live in the non-test build (every field is read: `verbose_log` directly,
// `theme`/`last_destination_mode` via the derived `PartialEq`/`Debug`). Were it kept, the `dead_code`
// expectation would flip to "unfulfilled" — a hard error under `-D warnings`. G70 flags the removed
// `#![… expect(dead_code) …]` as a "removed assertion": a FALSE POSITIVE — it is a LINT attribute, never a
// test assertion, and the §6.4.1 tests below are entirely unchanged.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde_json::Value;
use tauri::{AppHandle, Manager};

/// The one prefs file — the single `settings.json` in the app config dir (§7.4.2); this module is its sole
/// owner.
const SETTINGS_FILE: &str = "settings.json";

/// §7.4.2 read bound: the most `settings.json` bytes a load accepts (64 KiB). The 3-key blob is a few
/// hundred bytes, so a larger file is treated as corrupt (the defaults plus one log line), and a planted or
/// runaway file can never make startup read an unbounded amount.
const SETTINGS_MAX_BYTES: u64 = 64 * 1024;

/// §7.4.1 blob key — the UI theme.
const KEY_THEME: &str = "theme";
/// §7.4.1 blob key — the re-usable chosen-destination hint.
const KEY_LAST_DESTINATION_MODE: &str = "lastDestinationMode";
/// §7.4.1 blob key — the diagnostic-logging opt-in.
const KEY_VERBOSE_LOG: &str = "verboseLog";

/// The `lastDestinationMode` "write beside each source" sentinel value (§7.4.1); any other string is a
/// chosen path.
const BESIDE_SOURCE_SENTINEL: &str = "beside-source";

/// §7.4.1 `theme` — the UI colour-scheme preference (a cosmetic value, not user data). §5.5 owns the theme
/// behaviour; this is only its persisted value. Default `System`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    /// Follow the OS setting — the default (persisted as `"system"`).
    #[default]
    System,
    /// Force the light scheme (persisted as `"light"`).
    Light,
    /// Force the dark scheme (persisted as `"dark"`).
    Dark,
}

impl Theme {
    /// (pure) Parse the persisted `theme` string (§7.4.1). An unrecognised or empty value tolerantly maps
    /// to the default `System` (the blob is best-effort, never load-bearing — §7.4.2), so only the two
    /// non-default values carry an explicit arm.
    fn parse(value: &str) -> Self {
        match value {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::System,
        }
    }
}

/// §7.4.1 `lastDestinationMode` — the re-usable chosen-destination hint (§2.7). A re-validated HINT, never a
/// guarantee: §2.7.2 / P2.88 re-check writability at use time and fall back per §2.7 if the chosen path is
/// gone or read-only. It stores a folder the user explicitly picked — never a source path or filename
/// (§7.4.1). Default `BesideSource`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LastDestinationMode {
    /// Write beside each source — the §2.7.1 default (persisted as `"beside-source"`).
    #[default]
    BesideSource,
    /// A user-chosen output root (persisted as its absolute-path string).
    ChosenPath(PathBuf),
}

impl LastDestinationMode {
    /// (pure) Parse the persisted `lastDestinationMode` string (§7.4.1): the `"beside-source"` sentinel (or
    /// an empty / malformed value) yields the default; any other string is taken as a chosen path. That path
    /// is a HINT re-validated as writable at use time (§2.7.2 / P2.88), so an invalid path here is harmless.
    fn parse(value: &str) -> Self {
        if value == BESIDE_SOURCE_SENTINEL || value.is_empty() {
            LastDestinationMode::BesideSource
        } else {
            LastDestinationMode::ChosenPath(PathBuf::from(value))
        }
    }
}

/// The §7.4 3-key `settings.json` prefs blob — the only state ConvertIA persists (§7.4.1 `[DECIDED]`).
///
/// [Build-Session-Entscheidung: P2.88 → P3.80] Consumer map — Rust reads all three keys into this complete typed
/// model (best-effort, §7.4.2). `verbose_log` is **Rust-consumed** (§7.5.3 — the P2.94 `resolve_log_verbosity`
/// startup read in `main`'s setup stage); `theme` (§5.5) is **served to the WebView** through a typed core
/// command (§7.4.2).
/// `last_destination_mode` is now **CORE-consumed** (P3.80 — the 2026-07-06 core-owned-paths ruling superseding
/// the P2.88 "frontend-consumed, mapped JS-side, never via Rust" split, the plan P2.88 `[Superseded]` note points
/// here): no FS path may cross the wire, so the WebView can NEVER hold the stored absolute path to map it.
/// Instead `crate::orchestrator::resolve_persisted_destination` reads this `LastDestinationMode` Rust-side,
/// re-validates a `ChosenPath` as writable (§2.7.2 `location_status`; §7.4.1 re-validate-at-use-time), and loads
/// a valid one into the §0.4.4 `DestinationRegistry` (a beside-source fallback registers nothing) — the frontend
/// then handles only the resulting `DestinationPicked` ID+display pair (its live consumers are the
/// P3.53+/P3.56 screens; P3.81 verifies — the 2026-07-15 re-ordering). The
/// HINT-not-a-guarantee semantics are encoded by the distinct `LastDestinationMode` type (P2.85).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Prefs {
    /// §7.4.1 `theme` — UI colour scheme (default `System`).
    pub theme: Theme,
    /// §7.4.1 `lastDestinationMode` — the re-usable chosen-destination hint (default `BesideSource`).
    pub last_destination_mode: LastDestinationMode,
    /// §7.4.1 `verboseLog` — the §7.5.3 / §5.9 diagnostic-logging opt-in (default `false`).
    pub verbose_log: bool,
}

impl Prefs {
    /// (pure) Build `Prefs` from the three raw settings values, already narrowed to primitives by
    /// [`from_store_values`](Prefs::from_store_values). ANY
    /// absent (`None`) or wrong-typed key (a non-string `theme` / `lastDestinationMode`, a non-bool
    /// `verboseLog` — all surface here as `None`) falls back to that key's default: the §7.4.2
    /// best-effort-never-load-bearing contract. Never fails.
    fn from_raw(
        theme: Option<&str>,
        last_destination_mode: Option<&str>,
        verbose_log: Option<bool>,
    ) -> Self {
        Self {
            theme: theme.map(Theme::parse).unwrap_or_default(),
            last_destination_mode: last_destination_mode
                .map(LastDestinationMode::parse)
                .unwrap_or_default(),
            verbose_log: verbose_log.unwrap_or_default(),
        }
    }

    /// (pure) Narrow the three raw `settings.json` `serde_json::Value`s to their expected primitives, then build
    /// `Prefs` via [`from_raw`](Prefs::from_raw). A key whose JSON value is the WRONG type (a number / bool /
    /// array / object where a string is expected, or a non-bool `verboseLog`) narrows to `None` via
    /// `Value::as_str` / `as_bool` and so falls back to that key's default — the §7.4.2 best-effort tolerance.
    /// This narrowing is a PURE helper (no `AppHandle` in its signature, so it is coverage-counted and
    /// unit-tested with adversarial JSON types — test-strategy §1.1a); [`from_settings_bytes`] feeds it the
    /// parsed object's three keys.
    fn from_store_values(
        theme: Option<&Value>,
        last_destination_mode: Option<&Value>,
        verbose_log: Option<&Value>,
    ) -> Self {
        Prefs::from_raw(
            theme.and_then(|value| value.as_str()),
            last_destination_mode.and_then(|value| value.as_str()),
            verbose_log.and_then(|value| value.as_bool()),
        )
    }
}

/// (pure) §7.4.2 parse of the raw `settings.json` bytes: `Some` for a JSON object — each §7.4.1 key narrowed
/// on its own by [`Prefs::from_store_values`], unknown extra keys ignored — and `None` for anything else:
/// empty bytes, invalid JSON, or a top-level array / string / number / bool / null. `None` is the caller's
/// cue to run with the §7.4.1 defaults and log once (§7.5); it never reaches the user as an error.
fn from_settings_bytes(bytes: &[u8]) -> Option<Prefs> {
    let Ok(Value::Object(object)) = serde_json::from_slice::<Value>(bytes) else {
        return None;
    };
    Some(Prefs::from_store_values(
        object.get(KEY_THEME),
        object.get(KEY_LAST_DESTINATION_MODE),
        object.get(KEY_VERBOSE_LOG),
    ))
}

/// §7.4.2 bounded read of the settings file at `path`: `Ok(None)` when no file exists (the first launch),
/// `Ok(Some(bytes))` for a regular file of at most [`SETTINGS_MAX_BYTES`], and `Err` otherwise —
/// `InvalidData` for an oversize file or a non-regular one (a directory, FIFO or device at the path), the OS
/// error for an unreadable one. At most `SETTINGS_MAX_BYTES + 1` bytes are ever read, whatever the file's
/// size. The type check is a `metadata` stat taken BEFORE the open (a stat never blocks), so a FIFO planted
/// at the path can never hang startup in `File::open` — the stat-then-open precedent of the §1.2
/// `orchestrator::detect_candidate` read, with the same narrow swap-between-stat-and-open residual.
fn read_settings_bytes(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the settings path is not a regular file (§7.4.2)",
        ));
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(SETTINGS_MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    // usize → u64 widens losslessly on every shipped target (the `detection::read_header` cast precedent).
    if bytes.len() as u64 > SETTINGS_MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the settings file exceeds the §7.4.2 read bound",
        ));
    }
    Ok(Some(bytes))
}

/// §7.4.2 read rules over the settings file at `path` (host FS, no `AppHandle`, so unit-tested over a
/// `tempfile` dir): an absent file yields the §7.4.1 defaults silently (the first launch); an unreadable,
/// oversize or non-regular file, or a document that is not a JSON object, yields the defaults plus one
/// static log line (§7.5); a JSON object narrows each key on its own (a wrong-typed key falls back to its
/// default). Never fails. The log lines carry at most an `io::ErrorKind` — never a path (§7.5.3).
fn load_from(path: &Path) -> Prefs {
    match read_settings_bytes(path) {
        Ok(None) => Prefs::default(),
        Ok(Some(bytes)) => from_settings_bytes(&bytes).unwrap_or_else(|| {
            tauri_plugin_log::log::warn!(
                "prefs: settings file is not a JSON object; running with defaults (§7.4.2)"
            );
            Prefs::default()
        }),
        Err(err) => {
            tauri_plugin_log::log::warn!(
                "prefs: settings file unreadable ({:?}); running with defaults (§7.4.2)",
                err.kind()
            );
            Prefs::default()
        }
    }
}

/// §7.4.2 — best-effort load of the 3-key prefs blob from `<app_config_dir>/settings.json` (P2.85.1: the
/// per-OS config dir is resolved via `app.path().app_config_dir()` — `dev.ne-ia.convertia/settings.json` —
/// and the file is addressed by that ABSOLUTE path). The file-level read rules are `load_from`'s.
///
/// P2.85.2 tolerance — never load-bearing (§7.4.2 / §2): a config dir that cannot be resolved yields
/// `Prefs::default()` plus one log line (§7.5); an absent, unreadable, oversize or non-object file and a
/// wrong-typed key are handled by `load_from`. It NEVER blocks a conversion or surfaces an error to the user.
///
/// The `&AppHandle` signature is the boot-glue seam (G28): this host-coupled body is verified by the
/// boot-stage pattern (the signature pin below + the §1.6 E2E real-window run), not `cargo test` execution
/// (no `tauri::test` mock harness by decision, test-strategy §1.1a); the read and parse it delegates to
/// (`load_from`, `read_settings_bytes`, `from_settings_bytes`, `from_store_values`) are unit-tested. The
/// §7.5.3 redaction stance holds — every `warn!` line on this path logs a static message, at most with an
/// `io::ErrorKind`, and never a path.
pub fn load(app: &AppHandle) -> Prefs {
    let Ok(config_dir) = app.path().app_config_dir() else {
        tauri_plugin_log::log::warn!(
            "prefs: could not resolve the app config dir; running with defaults (§7.4.2)"
        );
        return Prefs::default();
    };
    load_from(&config_dir.join(SETTINGS_FILE))
}

#[cfg(test)]
mod prefs_blob {
    //! §6.4.1 unit (G15): the §7.4.1 defaults + the §7.4.2 best-effort-never-load-bearing tolerance of the
    //! pure parse (an absent / valid / unrecognised / empty value each yields a well-defined result, never
    //! an error), plus the boot-glue signature pin for the host-coupled `load` (test-strategy §1.1a), and
    //! the §7.4.2 bounded file read + parse (`read_settings_bytes` / `from_settings_bytes` / `load_from`)
    //! over a `tempfile` dir.
    use super::*;

    #[test]
    fn defaults_are_system_beside_source_and_quiet() {
        // §7.4.1 defaults: theme "system", lastDestinationMode "beside-source", verboseLog false.
        let prefs = Prefs::default();
        assert_eq!(prefs.theme, Theme::System);
        assert_eq!(
            prefs.last_destination_mode,
            LastDestinationMode::BesideSource
        );
        assert!(!prefs.verbose_log);
    }

    #[test]
    fn absent_keys_yield_the_defaults() {
        // §7.4.2: a missing store / missing key → every default (never an error).
        assert_eq!(Prefs::from_raw(None, None, None), Prefs::default());
    }

    #[test]
    fn valid_values_parse() {
        let prefs = Prefs::from_raw(Some("dark"), Some("/home/u/out"), Some(true));
        assert_eq!(prefs.theme, Theme::Dark);
        assert_eq!(
            prefs.last_destination_mode,
            LastDestinationMode::ChosenPath(PathBuf::from("/home/u/out"))
        );
        assert!(prefs.verbose_log);
    }

    #[test]
    fn theme_parse_is_tolerant() {
        assert_eq!(Theme::parse("system"), Theme::System);
        assert_eq!(Theme::parse("light"), Theme::Light);
        assert_eq!(Theme::parse("dark"), Theme::Dark);
        // §7.4.2: case-sensitive; an unrecognised or empty value → the default, never an error.
        assert_eq!(Theme::parse("Dark"), Theme::System);
        assert_eq!(Theme::parse("purple"), Theme::System);
        assert_eq!(Theme::parse(""), Theme::System);
    }

    #[test]
    fn last_destination_mode_parse_is_tolerant() {
        assert_eq!(
            LastDestinationMode::parse("beside-source"),
            LastDestinationMode::BesideSource
        );
        // an empty / malformed value → the default sentinel.
        assert_eq!(
            LastDestinationMode::parse(""),
            LastDestinationMode::BesideSource
        );
        assert_eq!(
            LastDestinationMode::parse("/mnt/exports"),
            LastDestinationMode::ChosenPath(PathBuf::from("/mnt/exports"))
        );
    }

    #[test]
    fn verbose_log_passthrough_and_default() {
        assert!(Prefs::from_raw(None, None, Some(true)).verbose_log);
        assert!(!Prefs::from_raw(None, None, Some(false)).verbose_log);
        // absent (or a non-bool JSON value, which `from_store_values` maps to `None`) → default `false`.
        assert!(!Prefs::from_raw(None, None, None).verbose_log);
    }

    #[test]
    fn from_store_values_narrows_and_tolerates_wrong_json_types() {
        use serde_json::json;
        // Correct JSON types narrow + parse (theme/mode are strings, verboseLog is a bool).
        let prefs = Prefs::from_store_values(
            Some(&json!("dark")),
            Some(&json!("/home/u/out")),
            Some(&json!(true)),
        );
        assert_eq!(prefs.theme, Theme::Dark);
        assert_eq!(
            prefs.last_destination_mode,
            LastDestinationMode::ChosenPath(PathBuf::from("/home/u/out"))
        );
        assert!(prefs.verbose_log);

        // §7.4.2 tolerance: a WRONG JSON type per key narrows to `None` (via `as_str`/`as_bool`) → that
        // key's default, never an error — a number/array/object where a string is expected, a non-bool
        // `verboseLog`, and JSON `null` all fall back. This is the adversarial path `load` feeds from the
        // store; it runs through the real narrowing here (not merely asserted by inspection).
        assert_eq!(
            Prefs::from_store_values(Some(&json!(42)), Some(&json!(["x"])), Some(&json!("yes"))),
            Prefs::default()
        );
        assert_eq!(
            Prefs::from_store_values(
                Some(&json!(true)),
                Some(&json!({ "k": 1 })),
                Some(&json!(0))
            ),
            Prefs::default()
        );
        assert_eq!(
            Prefs::from_store_values(Some(&json!(null)), None, None),
            Prefs::default()
        );
    }

    #[test]
    fn load_has_its_boot_glue_signature() {
        // Boot-stage signature pin (test-strategy §1.1a): `load` is `AppHandle`-coupled (no `tauri::test`
        // mock harness by decision), so it is verified by its fn-pointer SIGNATURE here + the §1.6 E2E run,
        // not cargo-test execution — G28 exempts its body from the diff floor by this same signature.
        let _pinned: fn(&AppHandle) -> Prefs = load;
    }

    /// The settings file inside a fresh `tempfile` dir (the dir guard is returned so it lives as long as
    /// the test needs the path).
    fn settings_path() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join(SETTINGS_FILE);
        (dir, path)
    }

    #[test]
    fn from_settings_bytes_parses_a_json_object() {
        // §7.4.2: a JSON object parses; each key narrows through the real `from_store_values`.
        let bytes = br#"{"theme":"dark","lastDestinationMode":"/home/u/out","verboseLog":true}"#;
        assert_eq!(
            from_settings_bytes(bytes),
            Some(Prefs {
                theme: Theme::Dark,
                last_destination_mode: LastDestinationMode::ChosenPath(PathBuf::from(
                    "/home/u/out"
                )),
                verbose_log: true,
            })
        );
        // An empty object is a valid document with every key absent: the defaults, as `Some`.
        assert_eq!(from_settings_bytes(b"{}"), Some(Prefs::default()));
    }

    #[test]
    fn from_settings_bytes_keeps_the_known_keys_and_ignores_unknown_ones() {
        // Unknown extra keys (a future or foreign key) never spoil the three known ones.
        let bytes =
            br#"{"theme":"light","windowSize":[800,600],"future":{"x":1},"verboseLog":true}"#;
        assert_eq!(
            from_settings_bytes(bytes),
            Some(Prefs {
                theme: Theme::Light,
                last_destination_mode: LastDestinationMode::BesideSource,
                verbose_log: true,
            })
        );
        // A wrong-typed key inside a valid object falls back to its own default; the object still parses.
        assert_eq!(
            from_settings_bytes(br#"{"theme":42,"verboseLog":"yes"}"#),
            Some(Prefs::default())
        );
    }

    #[test]
    fn from_settings_bytes_rejects_every_non_object_document() {
        // §7.4.2: anything but a JSON object is `None` (the caller runs with the defaults and logs once).
        for bytes in [
            &b""[..],
            b"   ",
            b"not json",
            b"{\"theme\":",
            b"[\"dark\"]",
            b"\"dark\"",
            b"42",
            b"true",
            b"null",
        ] {
            assert_eq!(
                from_settings_bytes(bytes),
                None,
                "a non-object document must yield None: {:?}",
                String::from_utf8_lossy(bytes)
            );
        }
    }

    #[test]
    fn read_settings_bytes_absent_file_is_none() {
        // The first launch: no file at the path is `Ok(None)`, not an error.
        let (_dir, path) = settings_path();
        assert!(matches!(read_settings_bytes(&path), Ok(None)));
    }

    #[test]
    fn read_settings_bytes_returns_a_small_file_verbatim() {
        let (_dir, path) = settings_path();
        let content = br#"{"theme":"dark"}"#;
        std::fs::write(&path, content).expect("write settings");
        assert_eq!(
            read_settings_bytes(&path).expect("a small file reads"),
            Some(content.to_vec())
        );
    }

    #[test]
    fn read_settings_bytes_accepts_exactly_the_bound_and_refuses_one_byte_more() {
        let (_dir, path) = settings_path();
        let bound = usize::try_from(SETTINGS_MAX_BYTES).expect("the bound fits usize");
        // Exactly the §7.4.2 bound still reads (the bound is inclusive).
        std::fs::write(&path, vec![b' '; bound]).expect("write settings");
        let at_bound = read_settings_bytes(&path).expect("a file at the bound reads");
        assert_eq!(at_bound.map(|bytes| bytes.len()), Some(bound));
        // One byte over the bound is refused as `InvalidData` (treated as corrupt, never read in full).
        std::fs::write(&path, vec![b' '; bound + 1]).expect("write settings");
        let over = read_settings_bytes(&path).expect_err("a file over the bound is refused");
        assert_eq!(over.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn read_settings_bytes_refuses_a_directory_at_the_path() {
        // A non-regular file at the path is refused by the pre-open type check, on every OS alike.
        let (_dir, path) = settings_path();
        std::fs::create_dir(&path).expect("create a directory at the settings path");
        let err = read_settings_bytes(&path).expect_err("a directory is not a settings file");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn load_from_reads_a_valid_file_and_defaults_when_absent() {
        let (_dir, path) = settings_path();
        // Absent: the §7.4.1 defaults (the first launch).
        assert_eq!(load_from(&path), Prefs::default());
        std::fs::write(&path, br#"{"verboseLog":true,"theme":"light"}"#).expect("write settings");
        assert_eq!(
            load_from(&path),
            Prefs {
                theme: Theme::Light,
                last_destination_mode: LastDestinationMode::BesideSource,
                verbose_log: true,
            }
        );
    }

    #[test]
    fn load_from_degrades_to_the_defaults_on_every_bad_file() {
        // §7.4.2 never load-bearing: a non-object document, an oversize file and a directory at the path
        // each yield the defaults, never an error.
        let (_dir, path) = settings_path();
        std::fs::write(&path, b"[\"dark\"]").expect("write settings");
        assert_eq!(load_from(&path), Prefs::default());
        let bound = usize::try_from(SETTINGS_MAX_BYTES).expect("the bound fits usize");
        let mut oversize = br#"{"theme":"dark"}"#.to_vec();
        oversize.resize(bound + 1, b' ');
        std::fs::write(&path, &oversize).expect("write settings");
        assert_eq!(load_from(&path), Prefs::default());
        std::fs::remove_file(&path).expect("remove settings");
        std::fs::create_dir(&path).expect("create a directory at the settings path");
        assert_eq!(load_from(&path), Prefs::default());
    }
}
