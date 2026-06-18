// PLANTED-POSITIVE armed canary for G29 — this file DELIBERATELY violates the named rule and
// MUST be flagged by it (the SAST self-test prelude asserts it). DO NOT "fix" it. This dir is L(-1).
// rule (a): convertia-tauri-command-missing-fs-guard
use std::path::PathBuf;

#[tauri::command]
fn open_doc(path: PathBuf) -> String {
    format!("{:?}", path)
}
