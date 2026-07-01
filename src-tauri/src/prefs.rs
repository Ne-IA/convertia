//! `crate::prefs` — the §7.4 persistence layer: the 3-key `settings.json` prefs blob, the ONLY state
//! ConvertIA persists (§7.4.1 `[DECIDED]`: cosmetic / convenience / diagnostic values only — never anything
//! derived from the user's files). Read Rust-side via `tauri-plugin-store`. The blob is **best-effort and
//! never load-bearing** (§7.4.2): an unreadable / corrupt / absent store, or a wrong-typed key, falls back
//! to that key's default and logs (§7.5); it NEVER blocks a conversion or surfaces an error (§2).
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
//! §2.7.2), the `verboseLog` startup read (P2.89 / P2.94), `theme` (§5.5, frontend). The store PLUGIN is
//! already registered on the Builder (`main.rs`); the structural one-store-name gate is P2.86.

// The typed model + `load` are referenced only by their downstream readers (P2.88 / P2.89 / P2.94, §5.5)
// and the §6.4.1 tests below, so every item is dead in the PRODUCTION (non-test) build until a reader is
// wired — `expect` (not `allow`) auto-flags the moment the last one lands, matching `crate::domain` /
// `crate::outcome` / the §0.4.2 event-name constants.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the §7.4 prefs model + tolerant `load` are referenced by their downstream readers (P2.88 lastDestinationMode, P2.89/P2.94 verboseLog, §5.5 theme) + the §6.4.1 tests, so they are dead in the production build until a reader is wired."
    )
)]

use std::path::PathBuf;

use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

/// The one store file — the single `settings.json` (§7.4.2). ConvertIA opens exactly this store, by
/// convention (its only store name; the structural one-call-site gate is P2.86).
const SETTINGS_FILE: &str = "settings.json";

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
/// [Build-Session-Entscheidung: P2.88] Consumer map — Rust reads all three keys into this complete typed
/// model (best-effort, §7.4.2), but only `verbose_log` is **Rust-consumed** (§7.5.3 `tauri-plugin-log` init,
/// P2.94). `theme` (§5.5) and `last_destination_mode` (the C4 destination hint) are **frontend-consumed** —
/// read JS-side from the store (05-ui-ux "Persisted `lastDestinationMode`"), never via Rust — modelled here
/// for the complete-blob representation + its §7.4.2 tolerance. The `last_destination_mode`
/// re-validate-as-writable / beside-source-fallback ENFORCEMENT lives in P3 (C4 §1.10 preflight + §2.7.2
/// `location_status` + §2.7 divert), not here; a Rust `LastDestinationMode` → `DestinationChoice` mapping
/// would have no Rust consumer (C4 receives the `DestinationChoice` already mapped JS-side, 05-ui-ux). The
/// HINT-not-a-guarantee semantics are already encoded by the distinct `LastDestinationMode` type (P2.85).
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
    /// (pure) Build `Prefs` from the three raw store values, already extracted to primitives by `load`. ANY
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

    /// (pure) Narrow the three raw store `serde_json::Value`s to their expected primitives, then build
    /// `Prefs` via [`from_raw`](Prefs::from_raw). A key whose JSON value is the WRONG type (a number / bool /
    /// array / object where a string is expected, or a non-bool `verboseLog`) narrows to `None` via
    /// `Value::as_str` / `as_bool` and so falls back to that key's default — the §7.4.2 best-effort tolerance.
    /// This narrowing is a PURE helper (no `AppHandle` in its signature, so it is coverage-counted and
    /// unit-tested with adversarial JSON types — test-strategy §1.1a); `load` is only the store plumbing.
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

/// §7.4.2 — best-effort load of the 3-key prefs blob from `<app_config_dir>/settings.json` (P2.85.1: the
/// per-OS config dir is resolved via `app.path().app_config_dir()` — `dev.ne-ia.convertia/settings.json`;
/// the store plugin resolves a RELATIVE name against `BaseDirectory::AppData`, which diverges from the
/// §7.4.2-mandated config dir on Linux, so the store is opened by ABSOLUTE config-dir path).
///
/// P2.85.2 tolerance — never load-bearing (§7.4.2 / §2): a failure to resolve the config dir or open the
/// store falls back to `Prefs::default()` and logs (§7.5); `tauri-plugin-store` itself already tolerates a
/// corrupt / absent file (its build swallows the read error → an empty cache → every key `None` → the
/// defaults), and a wrong-typed key defaults via `from_store_values`. It NEVER blocks a conversion or
/// surfaces an error to the user.
///
/// The `&AppHandle` signature is the boot-glue seam (G28): this host-coupled body is verified by the
/// boot-stage pattern (the signature pin below + the §1.6 E2E real-window run), not `cargo test` execution
/// (no `tauri::test` mock harness by decision, test-strategy §1.1a); the PURE narrowing/parse it delegates
/// to (`from_store_values`) is unit-tested. The §7.5.3 redaction stance holds — the two `warn!` lines log a
/// static message plus the store error's `Display`; any path it might surface is ConvertIA's OWN config
/// location (never a user file), which §7.4.2 / §6.10-row-21 explicitly permit.
pub fn load(app: &AppHandle) -> Prefs {
    let Ok(config_dir) = app.path().app_config_dir() else {
        tauri_plugin_log::log::warn!(
            "prefs: could not resolve the app config dir; running with defaults (§7.4.2)"
        );
        return Prefs::default();
    };
    let path = config_dir.join(SETTINGS_FILE);
    let store = match app.store(&path) {
        Ok(store) => store,
        Err(err) => {
            tauri_plugin_log::log::warn!(
                "prefs: settings store unavailable ({err}); running with defaults (§7.4.2)"
            );
            return Prefs::default();
        }
    };
    // Read the three raw store values and hand them to the pure `from_store_values` narrower — the §7.4.2
    // wrong-type→default tolerance lives THERE (unit-tested), not in this AppHandle-coupled plumbing.
    // (`Store::get` returns an owned `Option<Value>`; the locals keep them alive for the borrowed narrow.)
    let theme = store.get(KEY_THEME);
    let last_destination_mode = store.get(KEY_LAST_DESTINATION_MODE);
    let verbose_log = store.get(KEY_VERBOSE_LOG);
    Prefs::from_store_values(
        theme.as_ref(),
        last_destination_mode.as_ref(),
        verbose_log.as_ref(),
    )
}

#[cfg(test)]
mod prefs_blob {
    //! §6.4.1 unit (G15): the §7.4.1 defaults + the §7.4.2 best-effort-never-load-bearing tolerance of the
    //! pure parse (an absent / valid / unrecognised / empty value each yields a well-defined result, never
    //! an error), plus the boot-glue signature pin for the host-coupled `load` (test-strategy §1.1a).
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
}
