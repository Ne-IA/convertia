// PLANTED-POSITIVE armed canary for G29 rule (b2), convertia-pandoc-unsafe-resource-path, under the pandoc
// module DIRECTORY: the rule's `paths: include` names engines/pandoc.rs and engines/pandoc/**, and only the
// directory glob binds this file, so its `sast-must-fire` pin stops firing - and the canary reds - when that
// glob no longer binds. DO NOT "fix". L(-1).
use std::ffi::OsString;

pub(crate) fn resource_args() -> Vec<OsString> {
    vec![OsString::from("--resource-path"), OsString::from("/usr/share/pandoc")] // sast-must-fire: convertia-pandoc-unsafe-resource-path
}
