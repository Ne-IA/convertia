// PLANTED-POSITIVE armed canary for G29 — this file DELIBERATELY violates the named rule and
// MUST be flagged by it (the SAST self-test prelude asserts it). DO NOT "fix" it. This dir is L(-1).
// rule (f): convertia-tauri-command-name-rename-forbidden — a `name=` override changes the
// registered IPC name (defeating plan-lint check 12). (`rename_all=` is ALLOWED, NOT a violation.)
#[tauri::command(name = "renamed_command")]
fn start_conversion(in_path: String) -> String {
    in_path
}
