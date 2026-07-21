//! ConvertIA build script — the Tauri v2 build-time setup (P1.12) + the §7.2.3 `AppInfo.build_id` producer
//! (P2.98).
//!
//! `tauri_build::build()` is mandatory for every Tauri app: it reads `tauri.conf.json`, generates the
//! permission/capability + context artifacts the runtime `tauri::generate_context!()` consumes
//! (P1.13), and re-runs when the conf / capabilities change (it emits its own `rerun-if-changed`). §0.4.0 / §0.4.5.

// §2.12 / G29: deny unsafe at this build-script crate root (first-party); build.rs has no FFI surface.
#![deny(unsafe_code)]

use std::env;

// [Build-Session-Entscheidung: P1.12] The §0.4.5 tauri-specta `bindings.ts` codegen is NOT driven from
// this build script: it is owned by the xtask `codegen` task — `cargo run -p xtask -- codegen` (wired
// P1.26, registered with the G19 generated-drift check at P1.28) — emitting the single tracked
// `src/lib/ipc/bindings.ts`. This build script drives only `tauri_build::build()` (the §0.4.5 build-time
// generation seam) + the §7.2.3 build-id injection below; codegen stays in xtask (one command, one tracked
// artifact), not a build-script hook.
fn main() {
    // [Build-Session-Entscheidung: P3.87] `new_without_app_manifest()` + the linker-embedded manifest
    // below replace the default `tauri_build::build()` manifest packaging — see
    // `emit_windows_manifest_link_args` for the full rationale (the bin+lib split's lib-test harness needs
    // the same Common-Controls-v6 manifest the bin gets, and cargo has no lib-test-scoped resource hook).
    // Icon / version-info / rc packaging are untouched (only the manifest member leaves `resource.lib`).
    if let Err(error) = tauri_build::try_build(
        tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest()),
    ) {
        // `cargo::error` fails the build with the diagnostic — the panic-free build-script idiom
        // (equivalent outcome to the panicking `tauri_build::build()` wrapper this call replaces).
        println!("cargo::error=tauri-build failed: {error}");
    }
    emit_build_id();
    emit_windows_manifest_link_args();
}

// [Build-Session-Entscheidung: P3.87] The Windows application manifest, embedded by the LINKER for EVERY
// target of this crate (bin AND the lib unit-test harness). Since the bin+lib split the test suites compile
// into the LIB test harness, and tauri-build's default packaging ships the app manifest inside the
// bins-only `resource.lib` — an un-manifested test executable then loads the WinSxS comctl32 **5.82**,
// whose export table lacks the v6-only entry points the linked tauri/dialog surface imports
// (`TaskDialogIndirect`, `SetWindowSubclass`/`DefSubclassProc`/`RemoveWindowSubclass`), so the loader
// aborts the whole harness with `STATUS_ENTRYPOINT_NOT_FOUND` before any test runs. Cargo offers no
// lib-test-scoped link/resource directive (`rustc-link-arg-tests` reaches only `tests/`-dir integration
// targets — probed empirically), so the manifest moves to the plain `rustc-link-arg` channel, which DOES
// reach the lib test harness: `new_without_app_manifest()` (above) removes the manifest member from
// `resource.lib` (icon/version stay), and these directives embed the byte-identical manifest —
// `windows-app-manifest.xml` is a committed copy of tauri-build 2.6.3's default
// `src/windows-app-manifest.xml` (the Common-Controls v6 dependency, verbatim; re-verify on a tauri-build
// major bump) — into every linked target. `/MANIFESTUAC:NO` keeps the linker from adding a `trustInfo`
// fragment the tauri default does not carry (launch semantics stay byte-equivalent; no-manifest-fragment
// drift). Windows-only via `CARGO_CFG_WINDOWS` (the TARGET cfg, correct under cross-compilation); the
// pinned Windows toolchain is msvc (§0.8), so MSVC-style args are the only dialect needed.
fn emit_windows_manifest_link_args() {
    if env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let manifest =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        // [Build-Session-Entscheidung: P3.87] the four println!("cargo:...") lines below are build-script
        // cargo DIRECTIVES (the only channel for link args / rerun rules — the P2.98 precedent), not
        // runtime debug prints; the fn doc above records the full manifest rationale.
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo::rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo::rustc-link-arg=/MANIFESTUAC:NO");
        println!(
            "cargo::rustc-link-arg=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
}

// [Build-Session-Entscheidung: P2.98] Inject the §7.2.3 `AppInfo.build_id` — the §6 CI build identifier —
// as a `rustc-env` the core reads via `env!("CONVERTIA_BUILD_ID")` (so it is compile-time-guaranteed present,
// never an empty string, honoring §7.2.3 "neither field may silently ship empty"). In CI, GitHub Actions
// auto-sets `GITHUB_SHA` + `GITHUB_RUN_ID` on every runner, so NO `.github`/workflow (L(-1)) change is needed
// to produce the real §6 id `<short-sha>-<run-id>`; the release build (Lane-B, P10) picks up the same vars.
// In a local `tauri dev`/`cargo` build both are absent → the deterministic literal `"dev"` marker (the box's
// sanctioned non-empty dev fallback; a git subprocess is deliberately avoided — no build-time process spawn).
// The `rerun-if-env-changed` lines refresh the embedded id when the CI vars change (they ADD to the
// `rerun-if-changed` set `tauri_build::build()` already emitted for the conf/capabilities — additive, so the
// conf-driven regeneration is unaffected).
fn emit_build_id() {
    let build_id = match (env::var("GITHUB_SHA"), env::var("GITHUB_RUN_ID")) {
        (Ok(sha), Ok(run)) if !sha.is_empty() && !run.is_empty() => {
            let short = sha.get(..9).unwrap_or(sha.as_str());
            format!("{short}-{run}")
        }
        _ => "dev".to_owned(),
    };
    // [Build-Session-Entscheidung: P2.98] the three `println!("cargo:…")` below are build-script cargo
    // DIRECTIVES (the only mechanism to emit a rustc-env / rerun rule), not runtime debug prints — G8's
    // println! ban targets runtime code, so these documented directive lines carry the choice-site tag.
    println!("cargo:rustc-env=CONVERTIA_BUILD_ID={build_id}");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    println!("cargo:rerun-if-env-changed=GITHUB_RUN_ID");
}
