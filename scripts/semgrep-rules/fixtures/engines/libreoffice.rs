// PLANTED-POSITIVE armed canary for G29 (engine-argv) — RESOLVED-SIDECAR-PATH spawns + SIBLING-ESCAPE
// shape (a hardened sibling next to an unhardened one), so the canary catches both the binary-literal
// silent-never-bind class AND the whole-fn over-suppression (fail-OPEN) class. DO NOT "fix". L(-1).
// engines/<engine>.rs matches the rule paths: include glob.
// rules (b3a) convertia-soffice-accept-socket + (b3b) convertia-soffice-missing-user-installation
use std::process::Command;

fn run_soffice(&self, scratch: &str) {
    let _good = Command::new(resolve_sidecar("soffice")).arg(format!("-env:UserInstallation=file://{}", scratch)).env_clear();
    // unhardened sibling: --accept (b3a) AND no -env:UserInstallation (b3b) -> both MUST fire
    let _bad = Command::new(resolve_sidecar("soffice")).arg("--accept=socket,host=localhost,port=2002;urp;").arg("--headless").env_clear();
}
