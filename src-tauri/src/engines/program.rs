//! `crate::engines::program` — the §3.3.3 runtime program-path resolution (§0.7 physical tree:
//! `engines/`, tier 2): how the Rust core turns a §3.2.2 [`EngineProgram`] into the ABSOLUTE binary path
//! [`crate::isolation::run_confined`] spawns. Two shapes, per §3.3.3 `[DECIDED]`:
//!
//! * **`externalBin` sidecars** resolve by their BARE name beside the app executable —
//!   `current_exe()?.parent()` joined with `ffmpeg` / `ffmpeg.exe`. The `-<target-triple>` suffix is a
//!   STAGE-time convention only (`scripts/stage-engines`, P4.27); Tauri STRIPS it when bundling, so
//!   resolving `current_exe()` + the triple would look for a file that does not exist in the shipped
//!   bundle. `BaseDirectory::Resource` is **wrong** for a sidecar — they sit next to the main exe, not in
//!   the resources tree ([`sidecar_path`] is the executable form of that rule).
//! * **resources-tree binaries** (the LibreOffice `program/soffice.bin`) resolve under the bundle's
//!   resource root, the `app.path().resolve(rel, BaseDirectory::Resource)` answer.
//!
//! `PATH` is never consulted and every result is absolute (§3.5's env note; the §2.12.3 wrapper strips the
//! loader-injection vars, so a bare name would be unresolvable by design).
//!
//! [Build-Session-Entscheidung: P4.32] **The Tauri path APIs stay OUT of this tier-2 module.** The two
//! ROOTS are resolved ONCE by the AppHandle-coupled §7.2.1 boot glue (`publish_program_roots`, the
//! readiness gate's first step) and handed DOWN as plain paths into [`init_program_roots`]; everything here is
//! pure path arithmetic over them. That mirrors the `crate::isolation` contract (its `program` is
//! caller-supplied precisely "so this tier-2 module never touches the Tauri path APIs") and it is what
//! makes the whole resolver unit-testable with no Tauri runtime. It is also the **one home** the §7.2.3
//! startup verification (P4.42+) consumes: the presence loop resolves through [`program_roots`], never a
//! second resolution, so the path the verifier checks and the path the spawn uses cannot diverge.
//!
//! A resolution FAILURE at boot is a §2.13 app-level fault (`EngineMissing` / `BundleDamaged`) raised by
//! that boot glue, never a panic here — this module runs on the in-core no-panic path (§1.2 discipline,
//! G4/G14), so every fallible edge returns a structured `Err`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::{EngineId, EngineProgram};

/// The `.exe` extension the §3.3.3 sidecar rule appends on Windows ("same rule as the app binary"), empty
/// elsewhere. Keyed off the COMPILE target because this resolves paths for the process we are running in.
const EXE_SUFFIX: &str = if cfg!(windows) { ".exe" } else { "" };

/// The two once-resolved bundle roots every §3.3.3 lookup is relative to.
///
/// [Build-Session-Entscheidung: P4.32] A plain owned struct rather than borrowed paths: it lives in a
/// process-wide [`OnceLock`] for the life of the app (the roots cannot change while it runs) — the "one
/// home, read from anywhere in the engine layer" shape `crate::engines::registry`'s `REGISTRY` static
/// already establishes, and the reason `dispatch`, which carries no `AppHandle`, can reach them at all.
/// It is deliberately NOT the same PRIMITIVE: `REGISTRY` is a [`std::sync::LazyLock`] and so is
/// order-independent by construction, while these roots need an `AppHandle` and therefore have to be
/// PUSHED in at boot. That difference is a real hazard, not a detail — a test that reaches a reader
/// before the boot glue has published sees a different branch than one that runs after it, which is
/// exactly the order-dependence the P4.32 review found in `dispatch`'s seam test. Every consumer must
/// therefore be written to be indifferent to the publish state, and the tests below say so explicitly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramRoots {
    /// The directory holding the app executable — where Tauri places every `externalBin` sidecar (§3.3.1).
    exe_dir: PathBuf,
    /// The bundle's resource root — the `BaseDirectory::Resource` base the §3.3.1 `bundle.resources`
    /// TARGET paths (`engines/libreoffice/`, `engines/image/`, `fonts/`) hang off.
    resource_root: PathBuf,
}

impl ProgramRoots {
    /// Build the roots from the boot glue's two resolved absolute paths.
    #[must_use]
    pub fn new(exe_dir: PathBuf, resource_root: PathBuf) -> Self {
        Self {
            exe_dir,
            resource_root,
        }
    }

    /// The sidecar directory (§3.3.3) — the app executable's own parent.
    #[must_use]
    pub fn exe_dir(&self) -> &Path {
        &self.exe_dir
    }

    /// The `BaseDirectory::Resource` root (§3.3.3).
    #[must_use]
    pub fn resource_root(&self) -> &Path {
        &self.resource_root
    }
}

/// The process-wide home. Set exactly once by the §7.2.1 boot glue; read by `dispatch` (the spawn path)
/// and, from P4.42, by the §7.2.3 startup presence/integrity verification — one resolution, two readers.
static PROGRAM_ROOTS: OnceLock<ProgramRoots> = OnceLock::new();

/// Why a [`program_roots`] read or an [`init_program_roots`] write could not be served.
///
/// [Build-Session-Entscheidung: P4.32] A structured `Err` rather than a panic on either edge: this type
/// sits on the in-core path, and the boot glue projects `NotInitialised` onto the §2.13 app-level
/// `BundleDamaged` fault while `dispatch` projects it onto a per-item §2.8 `InternalError` (an app-level
/// `EngineMissing` is NOT a per-item conversion row — §2.8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramRootsError {
    /// Read before the §7.2.1 boot glue resolved the roots — a boot-ordering defect, never user input.
    NotInitialised,
    /// A second initialisation attempted DIFFERENT roots; the first ones stand (they are immutable for
    /// the process, so a re-point would silently redirect every subsequent spawn). Re-publishing the
    /// SAME roots is idempotent and returns `Ok` — see [`init_program_roots`].
    AlreadyInitialised,
}

/// Why an [`EngineProgram`] could not be turned into a spawnable path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramResolveError {
    /// The engine has no `externalBin` binary name in the §3.3.3 table — `ImageMagick` (a delegate linked
    /// INSIDE the image-worker, never spawned as its own sidecar) and `NativeCsvTsv` (`InProcessNative`,
    /// no binary at all) are deliberately absent from it.
    NoSidecarBinary(EngineId),
    /// An `InProcessNative` program was handed to the resolver — the one §3.5.6 in-core engine has no
    /// binary to resolve; reaching here means a dispatch arm routed the wrong lane.
    NotASubprocessProgram(EngineId),
    /// A `ResourceBin.rel` carrying any component that is not `Normal` — `..`, a root, or a Windows
    /// drive prefix — each of which `Path::join` uses to REPLACE the base rather than extend it. `rel` is
    /// engine-authored in-core data, so this is defence in depth, not untrusted-input handling: the
    /// resource root is a bundle boundary and such a join would silently spawn something outside it.
    ResourcePathEscapesRoot(PathBuf),
}

/// Publish the two roots the §7.2.1 boot glue resolved. Called exactly once, before any spawn.
///
/// Re-publishing the SAME roots is idempotent and returns `Ok` — only a RE-POINT (different roots) is an
/// error, because only that could silently redirect a subsequent spawn. The distinction matters at the
/// call site: the boot glue turns the error into a user-facing "your app folder looks incomplete" screen,
/// which would be a lie for a benign identical re-entry. [Build-Session-Entscheidung: P4.32]
///
/// # Errors
/// [`ProgramRootsError::AlreadyInitialised`] if DIFFERENT roots were already published.
pub fn init_program_roots(roots: ProgramRoots) -> Result<(), ProgramRootsError> {
    match PROGRAM_ROOTS.set(roots) {
        Ok(()) => Ok(()),
        Err(rejected) if PROGRAM_ROOTS.get() == Some(&rejected) => Ok(()),
        Err(_different) => Err(ProgramRootsError::AlreadyInitialised),
    }
}

/// The once-resolved roots.
///
/// # Errors
/// [`ProgramRootsError::NotInitialised`] if read before the §7.2.1 boot glue published them.
pub fn program_roots() -> Result<&'static ProgramRoots, ProgramRootsError> {
    PROGRAM_ROOTS.get().ok_or(ProgramRootsError::NotInitialised)
}

/// The §3.3.3 `[DECIDED]` `EngineId → binary-name` table — the SINGLE source of the §3.3.1 `externalBin`
/// names, so a sidecar name exists in exactly one place.
///
/// `ImageMagick` and `NativeCsvTsv` return `None` BY DESIGN, and the match is exhaustive with no wildcard
/// arm so a new [`EngineId`] variant fails to COMPILE here rather than silently resolving to `None` (the
/// G4/G14 exhaustive-dispatch discipline).
#[must_use]
pub fn sidecar_binary_name(id: EngineId) -> Option<&'static str> {
    match id {
        EngineId::FFmpeg => Some("ffmpeg"),
        EngineId::FFprobe => Some("ffprobe"),
        EngineId::LibreOffice => Some("soffice"),
        EngineId::Poppler => Some("pdftotext"),
        EngineId::Pandoc => Some("pandoc"),
        EngineId::ImageCore => Some("convertia-imgworker"),
        // NOT sidecars (§3.3.3): a delegate linked inside the image-worker, and the in-core engine.
        EngineId::ImageMagick | EngineId::NativeCsvTsv => None,
    }
}

/// The shipped file name of a sidecar — the bare §3.3.3 name plus `.exe` on Windows.
#[must_use]
pub fn sidecar_file_name(id: EngineId) -> Option<String> {
    sidecar_binary_name(id).map(|name| format!("{name}{EXE_SUFFIX}"))
}

/// The absolute path of a sidecar: BESIDE THE APP EXE, never under the resource root (§3.3.3).
///
/// # Errors
/// [`ProgramResolveError::NoSidecarBinary`] for an engine absent from the §3.3.3 table.
pub fn sidecar_path(id: EngineId, roots: &ProgramRoots) -> Result<PathBuf, ProgramResolveError> {
    let file = sidecar_file_name(id).ok_or(ProgramResolveError::NoSidecarBinary(id))?;
    Ok(roots.exe_dir().join(file))
}

/// The absolute path of a resources-tree binary: `<resource root>/<rel>` (§3.3.3).
///
/// # Errors
/// [`ProgramResolveError::ResourcePathEscapesRoot`] if `rel` carries any component that is not `Normal`
/// — a `..`, a root, or a Windows drive prefix, each of which `Path::join` uses to REPLACE the base.
pub fn resource_bin_path(rel: &Path, roots: &ProgramRoots) -> Result<PathBuf, ProgramResolveError> {
    // Every component must be `Normal`. `is_absolute()` is NOT sufficient on Windows: both a `Prefix`
    // (`C:cmd.exe` — drive-relative) and a bare `RootDir` (`\Windows\…`) report `is_absolute() == false`
    // there, yet `Path::join` REPLACES the base for each of them, so a check keyed on `is_absolute()`
    // alone would let the join escape the resource root on exactly one of the three §6.4.4 legs. This
    // mirrors `crate::fs_guard`'s §2.7.1 predicate, which rejects every non-`Normal` component for the
    // same reason. [Build-Session-Entscheidung: P4.32]
    let escapes = rel
        .components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)));
    if escapes {
        return Err(ProgramResolveError::ResourcePathEscapesRoot(
            rel.to_path_buf(),
        ));
    }
    Ok(roots.resource_root().join(rel))
}

/// Turn a §3.2.2 [`EngineProgram`] into the absolute binary path the §2.12 wrapper spawns (§3.3.3).
///
/// # Errors
/// See [`ProgramResolveError`] — an engine with no sidecar name, an `InProcessNative` program handed to
/// the subprocess path, or a `ResourceBin.rel` that escapes the resource root.
pub fn resolve_program(
    program: &EngineProgram,
    roots: &ProgramRoots,
) -> Result<PathBuf, ProgramResolveError> {
    match program {
        EngineProgram::Sidecar(id) => sidecar_path(*id, roots),
        EngineProgram::ResourceBin { rel, .. } => resource_bin_path(rel, roots),
        EngineProgram::InProcessNative(id) => Err(ProgramResolveError::NotASubprocessProgram(*id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots() -> ProgramRoots {
        ProgramRoots::new(
            PathBuf::from("/opt/convertia"),
            PathBuf::from("/opt/convertia/resources"),
        )
    }

    /// The §3.3.3 `[DECIDED]` table, spelled out per variant — a rename or a dropped row fails here, and
    /// the two deliberate `None`s are asserted as such rather than left to a wildcard.
    #[test]
    fn the_engine_id_to_binary_name_table_is_the_spec_table() {
        assert_eq!(sidecar_binary_name(EngineId::FFmpeg), Some("ffmpeg"));
        assert_eq!(sidecar_binary_name(EngineId::FFprobe), Some("ffprobe"));
        assert_eq!(sidecar_binary_name(EngineId::LibreOffice), Some("soffice"));
        assert_eq!(sidecar_binary_name(EngineId::Poppler), Some("pdftotext"));
        assert_eq!(sidecar_binary_name(EngineId::Pandoc), Some("pandoc"));
        assert_eq!(
            sidecar_binary_name(EngineId::ImageCore),
            Some("convertia-imgworker")
        );
        // §3.3.3: a delegate inside the image-worker, and the in-core engine — neither is a sidecar.
        assert_eq!(sidecar_binary_name(EngineId::ImageMagick), None);
        assert_eq!(sidecar_binary_name(EngineId::NativeCsvTsv), None);
    }

    /// The `.exe` rule keys off the COMPILE target, so this asserts the shape both ways rather than
    /// hard-coding one platform's answer (the leg that would otherwise be vacuous off Windows).
    #[test]
    fn the_shipped_file_name_carries_the_platform_exe_rule() {
        let ffmpeg = sidecar_file_name(EngineId::FFmpeg);
        if cfg!(windows) {
            assert_eq!(ffmpeg.as_deref(), Some("ffmpeg.exe"));
        } else {
            assert_eq!(ffmpeg.as_deref(), Some("ffmpeg"));
        }
        assert_eq!(sidecar_file_name(EngineId::NativeCsvTsv), None);
    }

    /// The §3.3.3 rule that is easiest to get wrong made executable: a sidecar resolves BESIDE THE EXE,
    /// and must NOT land under the resource root (`BaseDirectory::Resource` is for resource-tree binaries
    /// only — "resolving `current_exe()` + the triple suffix would look for a file that does not exist").
    #[test]
    fn a_sidecar_resolves_beside_the_exe_and_never_under_the_resource_root() {
        let r = roots();
        let path = sidecar_path(EngineId::FFmpeg, &r).expect("ffmpeg is a §3.3.3 sidecar");
        assert_eq!(path.parent(), Some(r.exe_dir()));
        assert!(!path.starts_with(r.resource_root()));
        // The staged `-<triple>` suffix is a stage-time convention Tauri strips: the resolved name is bare.
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        assert!(!name.contains("x86_64"), "resolved a staged triple: {name}");
    }

    #[test]
    fn an_engine_absent_from_the_table_has_no_sidecar_path() {
        assert_eq!(
            sidecar_path(EngineId::ImageMagick, &roots()),
            Err(ProgramResolveError::NoSidecarBinary(EngineId::ImageMagick))
        );
    }

    #[test]
    fn a_resource_bin_resolves_under_the_resource_root() {
        let r = roots();
        let rel = Path::new("engines/libreoffice/program/soffice.bin");
        let path = resource_bin_path(rel, &r).expect("a relative in-root path resolves");
        assert_eq!(path, r.resource_root().join(rel));
        assert!(path.starts_with(r.resource_root()));
    }

    #[test]
    fn a_resource_path_that_escapes_the_root_is_refused() {
        let r = roots();
        for rel in ["../outside/soffice", "engines/../../outside/soffice"] {
            assert_eq!(
                resource_bin_path(Path::new(rel), &r),
                Err(ProgramResolveError::ResourcePathEscapesRoot(PathBuf::from(
                    rel
                ))),
                "a `..` component must not join out of the resource root: {rel}"
            );
        }
        let absolute = if cfg!(windows) {
            r"C:\Windows\System32\cmd.exe"
        } else {
            "/bin/sh"
        };
        assert_eq!(
            resource_bin_path(Path::new(absolute), &r),
            Err(ProgramResolveError::ResourcePathEscapesRoot(PathBuf::from(
                absolute
            ))),
            "an absolute rel would replace the root entirely"
        );
    }

    /// The two Windows shapes `is_absolute()` does NOT catch and which `Path::join` nevertheless uses to
    /// replace the base: a drive-relative `Prefix` (`C:cmd.exe`) and a bare `RootDir`
    /// (`\Windows\System32\cmd.exe`). Windows-only BY CONSTRUCTION — on POSIX a backslash is not a
    /// separator and a leading `C:` is not a prefix, so both parse as ordinary `Normal` components there
    /// and asserting a refusal off Windows would assert a falsehood (the same platform-shaped trap as an
    /// `is_absolute()` fixture). Each case proves it escapes an UNGUARDED join FIRST, so the leg cannot go
    /// vacuous. [Build-Session-Entscheidung: P4.32]
    #[test]
    #[cfg(windows)]
    fn the_windows_drive_relative_and_rooted_shapes_are_refused() {
        let r = roots();
        for rel in [
            r"C:cmd.exe",
            r"\Windows\System32\cmd.exe",
            r"C:\Windows\System32\cmd.exe",
        ] {
            let p = Path::new(rel);
            let unguarded = r.resource_root().join(p);
            assert!(
                !unguarded.starts_with(r.resource_root()),
                "{rel} must genuinely escape an unguarded join, else this leg proves nothing"
            );
            assert_eq!(
                resource_bin_path(p, &r),
                Err(ProgramResolveError::ResourcePathEscapesRoot(
                    p.to_path_buf()
                )),
                "the §3.3.3 resource root must not be replaceable by {rel}"
            );
        }
    }

    /// `Path::join` REPLACES the base on an absolute argument, so without the guard above an absolute
    /// `rel` would spawn straight out of the bundle. Pinned as its own leg because it is the reason the
    /// guard exists, not merely a variant of it.
    #[test]
    fn the_escape_guard_is_what_stops_join_from_replacing_the_root() {
        let r = roots();
        let absolute = if cfg!(windows) {
            r"C:\Windows\System32\cmd.exe"
        } else {
            "/bin/sh"
        };
        let unguarded = r.resource_root().join(absolute);
        assert!(
            !unguarded.starts_with(r.resource_root()),
            "join must be shown to escape, else the guard's leg is vacuous"
        );
    }

    #[test]
    fn resolve_program_routes_each_engine_program_shape() {
        let r = roots();
        assert_eq!(
            resolve_program(&EngineProgram::Sidecar(EngineId::Pandoc), &r),
            Ok(r.exe_dir().join(format!("pandoc{EXE_SUFFIX}")))
        );
        let rel = PathBuf::from("engines/libreoffice/program/soffice.bin");
        assert_eq!(
            resolve_program(
                &EngineProgram::ResourceBin {
                    engine: EngineId::LibreOffice,
                    rel: rel.clone(),
                },
                &r
            ),
            Ok(r.resource_root().join(rel))
        );
        // The in-core engine has no binary — the subprocess path must never be handed one.
        assert_eq!(
            resolve_program(&EngineProgram::InProcessNative(EngineId::NativeCsvTsv), &r),
            Err(ProgramResolveError::NotASubprocessProgram(
                EngineId::NativeCsvTsv
            ))
        );
    }

    /// The process-wide home. Deliberately ORDER-INDEPENDENT: `OnceLock` is per-process and the test
    /// binary shares it, so this asserts the post-publish invariant (a read succeeds and returns the
    /// published roots) rather than an uninitialised-first sequence another test could win.
    /// [Build-Session-Entscheidung: P4.32]
    #[test]
    fn the_roots_home_serves_reads_once_published() {
        // An absolute path is platform-shaped: `/app` is NOT `is_absolute()` on Windows (it is
        // root-relative, drive-less), so the fixture has to carry a drive there or the leg asserts a
        // falsehood on one of the three §6.4.4 legs.
        let base = if cfg!(windows) {
            PathBuf::from(r"C:\app")
        } else {
            PathBuf::from("/app")
        };
        let published = ProgramRoots::new(base.clone(), base.join("resources"));
        // Either this call publishes, or a sibling test already did — both are the same post-state.
        // A matches-based assertion rather than a panic-macro arm: the no-panic policy denies the
        // panic-macro family on this crate's paths, `#[cfg(test)]` included — G4/G14.
        let outcome = init_program_roots(published);
        assert!(
            matches!(outcome, Ok(()) | Err(ProgramRootsError::AlreadyInitialised)),
            "unexpected init outcome: {outcome:?}"
        );
        let roots = program_roots().expect("published roots are readable");
        assert!(roots.exe_dir().is_absolute());
        assert!(roots.resource_root().is_absolute());
        // Re-publishing the SAME roots is idempotent — a benign re-entry must not become a user-facing
        // "your app folder looks incomplete" fault (the boot glue's projection of the error).
        assert_eq!(init_program_roots(roots.clone()), Ok(()));
        // A second publish never silently re-points a subsequent spawn.
        let other = base.join("other");
        assert_eq!(
            init_program_roots(ProgramRoots::new(other.clone(), other.join("resources"))),
            Err(ProgramRootsError::AlreadyInitialised)
        );
    }
}
