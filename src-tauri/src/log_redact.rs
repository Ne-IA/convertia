//! `crate::log_redact` — the §7.5.3 redaction stance made a primitive: the DEFAULT-level,
//! basename-only renderer for user file paths written to the local log (§7.5).
//!
//! §7.5.3 `[DECIDED]`: a log that recorded file **contents** or a user file's **full path** would
//! dent the §2.11 privacy invariant — a path can carry a username, project names, the user's whole
//! directory structure. At the default level (`info`/`warn`) the log records **structural facts
//! only**, and where a user file must be named, its **basename only** (`vacation.jpg`, never
//! `/home/alice/secret-project/vacation.jpg`). This module is that basename-only door.
//!
//! **The convention (§7.5.3).** Every default-level log site that would name a *user* file routes its
//! path through [`RedactedPath`]; the full path can then never reach the log by construction (a
//! `&Path` becomes a value whose only rendering is the basename). This is the *structural* half of
//! the stance. The *behavioural* proof — "a secret-shaped path stem is absent from the log output" —
//! is the separate P2.127 property gate (§7.5 · G31/G15), not this box. A Rust-log-sink taint SAST
//! rule forcing every log site through the door was considered and **declined for v1** (a possible §8
//! owner-decidable hardening), so this door plus the documented convention are the v1 structural
//! control.
//!
//! **Out of scope: ConvertIA's OWN diagnostic paths.** The app's config-dir location (logged by
//! `crate::prefs` on a store fallback) is **not** a user file — §7.4.2 explicitly permits recording
//! it — so it does not route through here. The stance covers *user* file paths only.
//!
//! **Verbose full paths are a subsequent, additive box.** The §7.5.3 verbose / diagnostic opt-in that
//! *additionally* records full paths for reproduction (read once at startup, effective next launch)
//! is **P2.94** (`needs: P2.93`). This box builds the basename-only default renderer only — there is
//! deliberately no verbose branch here (it would be dead + untested until P2.94 lands the
//! `--verbose` / `verboseLog` read).
//!
//! **Home.** A binary-root LEAF module (a sibling of `main.rs`), like `crate::prefs`: §0.7 homes no
//! §7.5 logging tier module — the log PLUGIN wiring lives in `main.rs` (`log_plugin`), and this is
//! the redaction POLICY — and an app-shell concern with no tier lives at the binary root rather than
//! being forced into an unrelated tier module. A leaf file adds no directory, so it is inert to the
//! §1a / §0.7 structural map (G69) and needs no §0.7 physical-tree row.

// The redaction door has no PRODUCTION caller yet — today the crate logs only ConvertIA's own config
// path (`crate::prefs`, out of the stance's scope above); the first *user*-path log sites land with
// their producers (P2.94 verbose diagnostics, the P3+ run / P4 engine-argv log sites). It is
// exercised by the §6.4.1 tests below now; `expect` (not `allow`) auto-flags the moment the first
// real caller lands, matching `crate::prefs` / `crate::platform` / `crate::domain`.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the §7.5.3 basename-only redaction door has no production caller yet — today the crate logs only ConvertIA's own config path (crate::prefs, out of the stance's scope); the first user-path log sites land with their producers (P2.94 verbose / P3+ run / P4 engine argv). Exercised by the §6.4.1 tests now."
    )
)]

use std::fmt;
use std::path::Path;

/// [Build-Session-Entscheidung: P2.93] The §7.5.3 no-basename fallback. A path with no final
/// component (a filesystem root, or a `..`-terminated path) has no basename to show — render this
/// fixed, non-leaking fallback rather than the directory path, so the "never the full path at the
/// default level" invariant (§2.11) holds even for the degenerate input.
const NO_BASENAME_FALLBACK: &str = "<no-basename>";

/// [Build-Session-Entscheidung: P2.93] The §7.5.3 default-level loggable form of a user file path:
/// its **basename only**, never the directory. Wrap a `&Path` at a log site —
/// `warn!("could not write {}", RedactedPath::new(&out))` — and BOTH its `Display` and `Debug` render
/// `vacation.jpg`, never `/home/alice/secret-project/vacation.jpg`. Making the redaction a *type* is
/// what turns the §7.5.3 convention structural: a value of this type cannot yield the full path
/// through any standard formatting, so a default-level user-path log site that routes through it
/// cannot leak the directory by construction (the §2.11 privacy invariant).
///
/// `Debug` is written by hand to also render the basename: a `#[derive(Debug)]` would print the
/// wrapped `&Path` in full (`RedactedPath("/home/alice/…")`) and so re-open the exact leak this type
/// exists to close, so `{:?}` is redacted to the same basename as `{}`.
///
/// The verbose / full-path override (§7.5.3 opt-in, read once at startup) is **P2.94** — it is
/// additive and does not change this default-level basename rendering; no verbose branch lives here.
#[derive(Clone, Copy)]
pub(crate) struct RedactedPath<'a>(&'a Path);

impl<'a> RedactedPath<'a> {
    /// Wrap a user file path for default-level logging (§7.5.3): the only thing it will render is the
    /// basename.
    pub(crate) fn new(path: &'a Path) -> Self {
        RedactedPath(path)
    }

    /// The one rendering both `Display` and `Debug` share: the basename (§7.5.3), or the
    /// `NO_BASENAME_FALLBACK` text for a path with no final component. `Path::file_name` +
    /// `OsStr::to_string_lossy` are total (no panic — the crate-root `clippy::panic` deny), and the
    /// `Some`/`None` match is exhaustive, so no `_ =>` arm is needed (the §0.7 exhaustive-dispatch
    /// deny). A non-UTF-8 basename is rendered lossily (U+FFFD) — the name, not the directory, so the
    /// privacy invariant is unaffected.
    fn write_basename(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.file_name() {
            Some(name) => f.write_str(&name.to_string_lossy()),
            None => f.write_str(NO_BASENAME_FALLBACK),
        }
    }
}

impl fmt::Display for RedactedPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_basename(f)
    }
}

impl fmt::Debug for RedactedPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_basename(f)
    }
}

#[cfg(test)]
mod tests {
    use super::{RedactedPath, NO_BASENAME_FALLBACK};
    use std::path::{Path, PathBuf};

    // A representative user path built with `join`, so the platform-native separator is used on every
    // OS and `file_name()` splits it correctly on Win/Linux/macOS alike (a hard-coded `/`-literal
    // would not split on Windows' `\`).
    fn user_path() -> PathBuf {
        Path::new("home")
            .join("alice")
            .join("secret-project")
            .join("vacation.jpg")
    }

    // §6.4.1 unit (G15): the §7.5.3 core stance — a user file path renders as its BASENAME ONLY at the
    // default level; the directory is never in the output.
    #[test]
    fn redacts_to_basename_only() {
        assert_eq!(RedactedPath::new(&user_path()).to_string(), "vacation.jpg");
    }

    // §6.4.1 unit (G15): the privacy invariant stated NEGATIVELY (§7.5.3 / §2.11) — no directory
    // component of the source path appears in the rendered form, via either `{}` or `{:?}`. This is
    // the assertion that catches a regression to logging the full path (a `#[derive(Debug)]` leak, or
    // a Display that fell back to the whole path).
    #[test]
    fn never_leaks_a_directory_component() {
        let p = user_path();
        let shown = RedactedPath::new(&p).to_string();
        let debugged = format!("{:?}", RedactedPath::new(&p));
        for leaked in ["home", "alice", "secret-project"] {
            assert!(
                !shown.contains(leaked),
                "§7.5.3: Display leaked directory component `{leaked}`"
            );
            assert!(
                !debugged.contains(leaked),
                "§7.5.3: Debug leaked directory component `{leaked}`"
            );
        }
    }

    // §6.4.1 unit (G15): `Debug` is redacted to the SAME basename as `Display` — the hand-written impl,
    // NOT the derived one (which would print the full wrapped path) — so a `{:?}` log site is as safe
    // as a `{}` one.
    #[test]
    fn debug_matches_display_basename() {
        let p = Path::new("var").join("data").join("report.pdf");
        assert_eq!(format!("{:?}", RedactedPath::new(&p)), "report.pdf");
        assert_eq!(format!("{}", RedactedPath::new(&p)), "report.pdf");
    }

    // §6.4.1 unit (G15): a bare filename (already only a basename) renders unchanged — the door is a
    // no-op when there is nothing to strip, so a log site can route EVERY path through it uniformly.
    #[test]
    fn bare_filename_is_unchanged() {
        assert_eq!(
            RedactedPath::new(Path::new("photo.png")).to_string(),
            "photo.png"
        );
    }

    // §6.4.1 unit (G15): a non-ASCII basename survives intact — only the DIRECTORY is stripped, not
    // the name's characters (the basename is what the user recognises, §7.5.3 `vacation.jpg`).
    #[test]
    fn unicode_basename_preserved() {
        let p = Path::new("dir").join("Ferienfotos-Sommer-2026.jpeg");
        assert_eq!(
            RedactedPath::new(&p).to_string(),
            "Ferienfotos-Sommer-2026.jpeg"
        );
    }

    // §6.4.1 unit (G15): a path with no final component — BOTH a filesystem root and a `..`-terminated
    // path (the two cases the `NO_BASENAME_FALLBACK` doc names) — has no basename, so it renders the
    // fixed non-leaking fallback, never the directory path, and never a panic (the crate-root
    // `clippy::panic` deny; `file_name()` returns `None` for each, handled by the `None` arm).
    #[test]
    fn no_basename_path_renders_fallback() {
        // A bare filesystem root — `/` is a root on every shipped platform (a Windows drive root is
        // likewise root-only), so `file_name()` is `None`.
        assert_eq!(
            RedactedPath::new(Path::new("/")).to_string(),
            NO_BASENAME_FALLBACK
        );
        // A `..`-terminated path likewise has no final normal component.
        assert_eq!(
            RedactedPath::new(Path::new("..")).to_string(),
            NO_BASENAME_FALLBACK
        );
        let parent_terminated = Path::new("some").join("dir").join("..");
        assert_eq!(
            RedactedPath::new(&parent_terminated).to_string(),
            NO_BASENAME_FALLBACK
        );
    }
}
