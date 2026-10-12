// PLANTED-POSITIVE armed canary for G29 rule (b2), convertia-pandoc-unsafe-resource-path: an absolute or
// home path literal anywhere in the pandoc module. The engine argv is a Vec<OsString> built by plan(), so
// the rule matches the literal itself, never a spawn chain. Every planted form carries a `sast-must-fire`
// pin; the hardened sibling's flag and relative literals carry `sast-clean` pins. DO NOT "fix". L(-1).
// engines/<engine>.rs matches the rule paths: include glob.
use std::ffi::OsString;
use std::path::Path;

fn plan_hardened(work: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = Vec::new();
    a.push(OsString::from("--sandbox")); // sast-clean: convertia-pandoc-unsafe-resource-path
    a.push(OsString::from("--resource-path")); // sast-clean: convertia-pandoc-unsafe-resource-path
    a.push(work.as_os_str().to_owned());
    a.push(OsString::from("media/figure.png")); // sast-clean: convertia-pandoc-unsafe-resource-path
    a.push(OsString::from("./media")); // sast-clean: convertia-pandoc-unsafe-resource-path
    a.push(OsString::from("\n")); // sast-clean: convertia-pandoc-unsafe-resource-path
    a
}

fn plan_home_data_dir() -> Vec<OsString> {
    vec![OsString::from("--resource-path"), OsString::from("/home/user/.local/share/pandoc")] // sast-must-fire: convertia-pandoc-unsafe-resource-path
}

fn plan_other_roots() -> Vec<OsString> {
    let mut a: Vec<OsString> = Vec::new();
    a.push(OsString::from("~/.pandoc")); // sast-must-fire: convertia-pandoc-unsafe-resource-path
    a.push(OsString::from("%APPDATA%\\pandoc")); // sast-must-fire: convertia-pandoc-unsafe-resource-path
    a.push(OsString::from("%USERPROFILE%\\pandoc")); // sast-must-fire: convertia-pandoc-unsafe-resource-path
    a.push(OsString::from("C:\\Users\\me\\pandoc")); // sast-must-fire: convertia-pandoc-unsafe-resource-path
    a.push(OsString::from("D:/pandoc")); // sast-must-fire: convertia-pandoc-unsafe-resource-path
    a.push(OsString::from("\\\\server\\share\\pandoc")); // sast-must-fire: convertia-pandoc-unsafe-resource-path
    a.push(OsString::from(r"/usr/share/pandoc")); // sast-must-fire: convertia-pandoc-unsafe-resource-path
    a.push(OsString::from(r#"/etc/pandoc"#)); // sast-must-fire: convertia-pandoc-unsafe-resource-path
    a.push(OsString::from(r"\pandoc")); // sast-must-fire: convertia-pandoc-unsafe-resource-path
    a
}
