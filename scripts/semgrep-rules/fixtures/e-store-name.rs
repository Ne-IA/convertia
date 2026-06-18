// PLANTED-POSITIVE armed canary for G29 — this file DELIBERATELY violates the named rule and
// MUST be flagged by it (the SAST self-test prelude asserts it). DO NOT "fix" it. This dir is L(-1).
// rule (e): convertia-store-name-not-constant (string-literal store name)
fn open_store(app: &tauri::AppHandle) {
    let _s = app.store("settings.json");
}
