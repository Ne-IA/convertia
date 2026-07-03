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
    tauri_build::build();
    emit_build_id();
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
