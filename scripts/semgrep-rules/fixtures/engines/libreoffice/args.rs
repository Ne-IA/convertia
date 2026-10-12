// PLANTED-POSITIVE armed canary for G29 rule (b3a), convertia-soffice-accept-socket, under the LibreOffice
// module DIRECTORY: the rule's `paths: include` names engines/libreoffice.rs and engines/libreoffice/**, and
// only the directory glob binds this file, so its `sast-must-fire` pin stops firing - and the canary reds -
// when that glob no longer binds. DO NOT "fix". L(-1).
use std::ffi::OsString;

pub(crate) fn listener_args() -> Vec<OsString> {
    vec![OsString::from("--headless"), OsString::from("--accept=socket,host=localhost,port=2002;urp;")] // sast-must-fire: convertia-soffice-accept-socket
}
