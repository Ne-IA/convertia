// PLANTED-POSITIVE armed canary for G29 rule (b3a), convertia-soffice-accept-socket: an accept-flag literal
// anywhere in the LibreOffice module (the flag opens the UNO loopback socket). The engine argv is a
// Vec<OsString> built by plan(), so the rule matches the literal itself, never a spawn chain. Every planted
// form carries a `sast-must-fire` pin; the hardened sibling's flag literals carry `sast-clean` pins.
// DO NOT "fix". L(-1). engines/<engine>.rs matches the rule paths: include glob.
use std::ffi::OsString;
use std::path::Path;

fn plan_hardened(work: &Path, profile_url: &str) -> Vec<OsString> {
    let mut a: Vec<OsString> = Vec::new();
    a.push(OsString::from("--headless")); // sast-clean: convertia-soffice-accept-socket
    a.push(OsString::from(format!("-env:UserInstallation={profile_url}"))); // sast-clean: convertia-soffice-accept-socket
    a.push(OsString::from("--convert-to")); // sast-clean: convertia-soffice-accept-socket
    a.push(OsString::from("--outdir")); // sast-clean: convertia-soffice-accept-socket
    a.push(work.as_os_str().to_owned());
    a
}

fn plan_uno_listener() -> Vec<OsString> {
    let mut a: Vec<OsString> = Vec::new();
    a.push(OsString::from("--accept=socket,host=localhost,port=2002;urp;")); // sast-must-fire: convertia-soffice-accept-socket
    a.push(OsString::from("-accept=pipe,name=convertia;urp;")); // sast-must-fire: convertia-soffice-accept-socket
    a.push(OsString::from(r"--accept=socket,host=localhost,port=2003;urp;")); // sast-must-fire: convertia-soffice-accept-socket
    a
}
