#!/usr/bin/env python3
"""g24-sast.py - G24 self-test for check-sast (P0.4.2, G29 Semgrep leg).

Runs on all 3 OS in the continuous-armed-canary WITHOUT semgrep installed, so it proves check-sast's
pure, importable LOGIC: (1) the project rule-ids auto-discover from scripts/semgrep-rules/project/ and
match the committed rule set; (2) every project rule has a planted-positive fixture; (3) the
armed-canary detector (missing_canaries) flags a rule that stopped firing; (4) the net-allow-list
parser ignores comments/blanks; (5) the net-ban allow-list FILTER drops only allow-listed net findings;
(6, P4.85) the `sast-clean`/`sast-must-fire` per-line pin marker parse + the both-direction
detectors (broken_suppressions / missing_must_fires), the text<->matcher command-census
counter/comparator, the macOS homing scan (cfg + runtime consts::OS forms), and the structural
pins on the refined (b1)/(d) rule text (paths scope + fixture binding + the adjacent suppression
forms); (7, the 2026-08-30 P4.26 adjudication) the T11 seam pin `t11_seam_pin` (the engine_input
macOS arm's own span calls the literal standalone stage_for_tcc; a dropped call, a vanished or
not()-only arm, a call in the wrong arm, or a rename of either load-bearing name is flagged);
(8) the engine argv contract, the G29 (b) split: the (b2)/(b3a) absence rules are whole-module
`pattern-regex` rules whose one `paths:` key includes exactly the module's file and directory
globs, and whose regex, run here through Python's `re`, fires on every `sast-must-fire` line of
the engine fixtures (the module file's and the one under the module directory) and on no
`sast-clean` line; and `argv_contract_pin` over hermetic temp roots (a `[ ]`/`[!]`/`[!extern]`
box leaves its engine dormant, `[x]` or an unresolvable box arms it, an armed engine needs
exactly one inline `mod argv_contract` block, in any file of the module at any depth, holding a
string literal for every flag - not in a comment, not outside the block, not moved by a `//` or a
brace inside a literal, a raw literal closed only by its own hash count), the live tree, and
main()'s wiring of the leg (over a temp root).
The REAL semgrep-over-fixtures armed canary
runs inside check-sast in the CI `sast` job (semgrep present). stdlib-only. Exit 0 = all held.
"""
import contextlib
import importlib.machinery
import importlib.util
import io
import re
import sys
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-sast"
_loader = importlib.machinery.SourceFileLoader("csast", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("csast", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


# --- 1. project rule-ids auto-discover ---------------------------------------------------------
ids = m.project_rule_ids()
# [Test-Change: P0.4.2 — old-obsolete+new-correct, §0.10 rule (e) retired 2026-09-29] 15 -> 14: the
# single-store-name rule left with the WebView store grant (G47 exact tokens); measured 14 rule ids.
# [Test-Change: P0.4.2 — old-obsolete+new-correct, §3.5 G29 (b) argv contract] 14 -> 11: the chain-shaped
# presence rules b3b, b4 and b5 bound no spawn (engine argv is a plan()-built Vec<OsString>); the presence
# half is check-sast's argv-contract leg (section 13); measured 11 rule ids.
record("rule discovery: 11 project rules found", len(ids) == 11)
record("rule discovery: the load-bearing rules are present",
       {"convertia-net-ban-std-tokio", "convertia-net-ban-raw-socket-ffi",
        "convertia-tauri-command-missing-fs-guard", "convertia-tauri-command-name-rename-forbidden",
        "convertia-webview-taint-to-dom-sink", "convertia-opener-egress-non-constant"} <= ids)
record("rule discovery: every NET_BAN_RULE_ID is a real discovered rule", m.NET_BAN_RULE_IDS <= ids)

# --- 2. planted-positive fixtures present, incl. the engine-module fixtures at the module path ---
# (the per-RULE firing guarantee is the check-sast canary prelude, not a file count; here we assert the
# fixtures + the path-scoped engine fixtures exist at engines/<engine>.rs and engines/<engine>/args.rs, so
# both `paths: include` globs of each engine rule bind. The engine argv absence rules are (b2) pandoc and
# (b3a) LibreOffice only.)
# [Test-Change: P0.4.2 — old-obsolete+new-correct, §3.5 G29 (b) argv contract] ffmpeg.rs and image.rs
# planted the retired b4/b5 spawn-chain rules and left with them.
fixtures = list(m.FIXTURES.rglob("*.rs")) + list(m.FIXTURES.rglob("*.ts"))
record("fixtures: present incl. the engine-module fixtures under engines/ (pandoc.rs, libreoffice.rs, and "
       "pandoc/args.rs, libreoffice/args.rs for the module directories)",
       len(fixtures) >= 10 and all((m.FIXTURES / "engines" / e).is_file()
                                   for e in ("pandoc.rs", "libreoffice.rs", "pandoc/args.rs", "libreoffice/args.rs")))

# --- 2b. the contract a-j tag set is pinned INDEPENDENTLY of the discovered ids (not count-only) ---
id_tags = m.project_rule_id_tags()
record("tags: every project rule carries a convertia-rule tag", len(id_tags) == len(ids))
record("tags: declared tag set == the frozen contract a-j set EXPECTED_RULE_TAGS",
       set(id_tags.values()) == m.EXPECTED_RULE_TAGS)
# [Test-Change: P0.4.2 — old-obsolete+new-correct, §3.5 G29 (b) argv contract] the dropped tag is b3a: b4 is
# retired, so subtracting it no longer shrinks the set.
record("tags: a renamed/dropped rule (tag set shrinks) != EXPECTED_RULE_TAGS (count-only would miss it)",
       (set(id_tags.values()) - {"b3a"}) != m.EXPECTED_RULE_TAGS)
record("tags: the retired e, b3b, b4 and b5 stay retired (in neither EXPECTED_RULE_TAGS nor a rule file)",
       not ({"e", "b3b", "b4", "b5"} & (m.EXPECTED_RULE_TAGS | set(id_tags.values()))))

# --- 3. armed-canary detector ------------------------------------------------------------------
def res(rids):  # synthetic semgrep result list
    return [{"check_id": "rules." + r, "path": "scripts/semgrep-rules/fixtures/x.rs"} for r in rids]

all_fired = res(sorted(ids))
record("canary: when ALL rules fire, missing_canaries is empty", m.missing_canaries(all_fired, ids) == set())
one_missing = res(sorted(ids - {"convertia-net-ban-std-tokio"}))
record("canary: a rule that STOPPED firing is flagged by missing_canaries",
       m.missing_canaries(one_missing, ids) == {"convertia-net-ban-std-tokio"})
record("canary: an empty result set flags ALL rules", m.missing_canaries([], ids) == ids)
record("canary: rules_fired extracts the last check_id segment",
       m.rules_fired([{"check_id": "scripts.semgrep-rules.project.x.convertia-net-ban-std-tokio"}]) ==
       {"convertia-net-ban-std-tokio"})

# --- 4. net-allow-list parser ------------------------------------------------------------------
sample = "# a comment\n\n   \nsrc-tauri/src/platform/net_probe.rs\n# another\n  crates/x/src/y.rs  \n"
parsed = m.parse_net_allow_list(sample)
record("allow-list: comments + blank lines ignored, paths kept",
       parsed == {"src-tauri/src/platform/net_probe.rs", "crates/x/src/y.rs"})
record("allow-list: the committed net-allow-list.txt is EMPTY (any net match fails)",
       m.parse_net_allow_list(m.NET_ALLOW_LIST.read_text(encoding="utf-8")) == set())

# --- 5. net-ban finding filter -----------------------------------------------------------------
findings = [
    {"check_id": "r.convertia-net-ban-std-tokio", "path": "src-tauri/src/platform/net_probe.rs"},  # allow-listed -> drop
    {"check_id": "r.convertia-net-ban-std-tokio", "path": "src-tauri/src/ipc/mod.rs"},              # NOT allow-listed -> keep
    {"check_id": "r.convertia-net-ban-raw-socket-ffi", "path": "src-tauri/src/ipc/mod.rs"},         # NOT allow-listed -> keep
    {"check_id": "r.convertia-tauri-command-missing-fs-guard", "path": "src-tauri/src/platform/net_probe.rs"},  # non-net -> always keep
]
allow = {"src-tauri/src/platform/net_probe.rs"}
kept = m.filter_net_findings(findings, allow)
record("net-filter: an allow-listed net finding is dropped", len(kept) == 3)
record("net-filter: a non-allow-listed net finding is kept",
       any(f["path"] == "src-tauri/src/ipc/mod.rs" and "net-ban-std-tokio" in f["check_id"] for f in kept))
record("net-filter: a non-net finding in an allow-listed file is ALWAYS kept",
       any("fs-guard" in f["check_id"] for f in kept))
record("net-filter: with an EMPTY allow-list, all net findings are kept (unconditional fail posture)",
       len(m.filter_net_findings(findings, set())) == 4)

# --- 5b. ABSOLUTE path normalisation (the G1 P0 fix: real_scan emits absolute paths) -----------
abs_path = str(m.ROOT / "src-tauri" / "src" / "platform" / "net_probe.rs")
record("net-filter: to_repo_rel normalises an ABSOLUTE finding path to repo-relative POSIX",
       m.to_repo_rel(abs_path) == "src-tauri/src/platform/net_probe.rs")
abs_findings = [{"check_id": "r.convertia-net-ban-std-tokio", "path": abs_path}]
record("net-filter: an ABSOLUTE-path net finding IS dropped when its repo-relative form is allow-listed "
       "(the absolute-vs-relative mismatch is fixed)",
       m.filter_net_findings(abs_findings, {"src-tauri/src/platform/net_probe.rs"}) == [])
record("net-filter: the same ABSOLUTE finding is KEPT against the empty allow-list",
       len(m.filter_net_findings(abs_findings, set())) == 1)

# [Test-Change: P0.4.2 — old-obsolete+new-correct, §0.10 rule (e) retired 2026-09-29] the 8
# store_load_count legs left with the count: no WebView store grant remains for it to guard.
import tempfile, shutil
_td = Path(tempfile.mkdtemp(prefix="g24-sast-"))
try:
    # --- 7. matcher-gap backstop: a Command::new nested in a macro arg (semgrep-invisible) is flagged ---
    record("macro-backstop: target-absent (no app source) -> []", m.macro_nested_commands([]) == [])
    (_td / "b.rs").write_text('fn f(){ let _ = vec![Command::new(a()).spawn()]; }\n', encoding="utf-8")
    record("macro-backstop: Command::new inside vec![..] is flagged", len(m.macro_nested_commands([_td])) >= 1)
    (_td / "b.rs").write_text('fn f(){ let s = format!("{:?}", Command::new(a()).output()); }\n', encoding="utf-8")
    record("macro-backstop: Command::new inside format!(..) is flagged", len(m.macro_nested_commands([_td])) >= 1)
    (_td / "b.rs").write_text('fn f(){ let mut c = Command::new(a()); c.env_clear(); c.spawn(); }\n', encoding="utf-8")
    record("macro-backstop: a statement-level Command::new is NOT flagged", m.macro_nested_commands([_td]) == [])
    (_td / "b.rs").write_text('fn f(){ assert_eq!(x, y); let _ = Command::new(a()).spawn(); }\n', encoding="utf-8")
    record("macro-backstop: a Command::new AFTER a macro's `;` (statement-level) is NOT flagged",
           m.macro_nested_commands([_td]) == [])
    (_td / "b.rs").write_text('fn f(){ // vec![Command::new(a())] in a comment\n let s = "vec![Command::new(x)]"; }\n', encoding="utf-8")
    record("macro-backstop: a Command::new in a comment/string does NOT count (blanking)", m.macro_nested_commands([_td]) == [])
    # DOCUMENTED boundary (pinned, not silent): an internal `;` inside a macro arg defeats the `;`-bounded
    # grep — this is the P1 reconciliation residual recorded in the check-sast comment + the G29 row.
    (_td / "b.rs").write_text('fn f(){ let _ = vec![{ let z = 1; Command::new(a()).spawn() }]; }\n', encoding="utf-8")
    record("macro-backstop: the DOCUMENTED internal-`;` macro form is a known miss (P1 reconciliation, not silent)",
           m.macro_nested_commands([_td]) == [])

    # --- 7b. the temp-dir sibling backstop: a temp_dir() nested in a macro arg (semgrep-invisible) is flagged ---
    record("tmpdir-backstop: target-absent (no app source) -> []", m.macro_nested_temp_dirs([]) == [])
    (_td / "b.rs").write_text('fn f(){ let v = vec![std::env::temp_dir()]; }\n', encoding="utf-8")
    record("tmpdir-backstop: std::env::temp_dir() inside vec![..] is flagged (the escaped 91d1975 form)",
           len(m.macro_nested_temp_dirs([_td])) >= 1)
    (_td / "b.rs").write_text('fn f(){ let s = format!("{:?}", env::temp_dir()); }\n', encoding="utf-8")
    record("tmpdir-backstop: env::temp_dir() inside format!(..) is flagged", len(m.macro_nested_temp_dirs([_td])) >= 1)
    (_td / "b.rs").write_text('fn f(){ let v = vec![temp_dir()]; }\n', encoding="utf-8")
    record("tmpdir-backstop: a BARE imported temp_dir() in a macro arg is flagged (the _MACRO_CMD_RE optional-qualifier posture)",
           len(m.macro_nested_temp_dirs([_td])) >= 1)
    (_td / "b.rs").write_text('fn f(){ let mut v = Vec::new(); v.push(std::env::temp_dir()); }\n', encoding="utf-8")
    record("tmpdir-backstop: a statement-level push-arg temp_dir is NOT flagged (semgrep sees + nosemgrep audits it)",
           m.macro_nested_temp_dirs([_td]) == [])
    (_td / "b.rs").write_text('fn f(){ assert_eq!(x, y); let _ = std::env::temp_dir(); }\n', encoding="utf-8")
    record("tmpdir-backstop: a temp_dir AFTER a macro's `;` (statement-level) is NOT flagged",
           m.macro_nested_temp_dirs([_td]) == [])
    (_td / "b.rs").write_text('fn f(){ // vec![std::env::temp_dir()] in a comment\n let s = "vec![env::temp_dir()]"; }\n', encoding="utf-8")
    record("tmpdir-backstop: a temp_dir in a comment/string does NOT count (blanking)", m.macro_nested_temp_dirs([_td]) == [])
    (_td / "b.rs").write_text('fn f(){ let p = make_temp_dir_for_job(); let v = vec![p]; }\n', encoding="utf-8")
    record("tmpdir-backstop: a `..temp_dir..`-substring identifier is NOT matched (word-boundary + call-paren anchors)",
           m.macro_nested_temp_dirs([_td]) == [])
    # the same DOCUMENTED internal-`;` residual as the Command sibling (leg-for-leg parity):
    (_td / "b.rs").write_text('fn f(){ let v = vec![{ let z = 1; std::env::temp_dir() }]; }\n', encoding="utf-8")
    record("tmpdir-backstop: the DOCUMENTED internal-`;` macro form is a known miss (the shared-walker residual, not silent)",
           m.macro_nested_temp_dirs([_td]) == [])
finally:
    shutil.rmtree(_td, ignore_errors=True)

# --- 8. the P4.85 suppression pins: `sast-clean` marker parse + broken-suppression detector -----
mk = m.parse_clean_markers('fn f(){}\nlet x = 1; // sast-clean: rule-a\nlet y = 2; // sast-clean: rule-a, rule-b\n')
record("clean-markers: a trailing `// sast-clean:` marker parses to its 1-based line + rule-id", mk.get(2) == {"rule-a"})
record("clean-markers: a comma-separated multi-id marker parses to the full id set", mk.get(3) == {"rule-a", "rule-b"})
record("clean-markers: text without markers parses to {}", m.parse_clean_markers("fn f(){}\nno markers here\n") == {})

_MPATH = "scripts/semgrep-rules/fixtures/x.rs"
_markers = {(_MPATH, 5): {"convertia-x"}}
def _f(rid, path, line):  # a synthetic semgrep finding with a start line
    return {"check_id": "rules." + rid, "path": path, "start": {"line": line}}
record("broken-suppressions: a finding of the pinned rule on the pinned line is flagged",
       m.broken_suppressions([_f("convertia-x", _MPATH, 5)], _markers) == [(_MPATH, 5, "convertia-x")])
record("broken-suppressions: a DIFFERENT rule's finding on the pinned line is NOT flagged (per-rule pins)",
       m.broken_suppressions([_f("convertia-y", _MPATH, 5)], _markers) == [])
record("broken-suppressions: the pinned rule on an UNpinned line is NOT flagged",
       m.broken_suppressions([_f("convertia-x", _MPATH, 7)], _markers) == [])
record("broken-suppressions: an ABSOLUTE finding path is normalised before the pin lookup",
       m.broken_suppressions([_f("convertia-x", str(m.ROOT / "scripts" / "semgrep-rules" / "fixtures" / "x.rs"), 5)],
                             _markers) == [(_MPATH, 5, "convertia-x")])

fm = m.fixture_clean_markers()
record("clean-markers: the committed b1+d fixtures pin >=5 refined-suppression lines clean (armed, not hoped)",
       len(fm) >= 5 and {"convertia-command-missing-env-clear", "convertia-macos-command-missing-stage-for-tcc"}
       <= {i for ids in fm.values() for i in ids})

# --- 8b. the sast-must-fire twin (the G1 round-1 Opus P2: per-line positives, fail-OPEN direction) ---
fk = m.parse_fire_markers("fn f(){}\nlet a = spawn(); // sast-must-fire: rule-a\n"
                          "let b = spawn(); // prose first // sast-must-fire: rule-a, rule-b\n")
record("fire-markers: a trailing `// sast-must-fire:` marker parses to its 1-based line + rule-id",
       fk.get(2) == {"rule-a"})
record("fire-markers: a marker AFTER a prose comment on the same line still parses (multi-id)",
       fk.get(3) == {"rule-a", "rule-b"})
record("fire-markers: text without markers parses to {}", m.parse_fire_markers("fn f(){}\n") == {})
record("must-fire: a pinned positive WITH its finding is not reported",
       m.missing_must_fires([_f("convertia-x", _MPATH, 5)], _markers) == [])
record("must-fire: a pinned positive with NO finding is reported (the fail-OPEN direction)",
       m.missing_must_fires([], _markers) == [(_MPATH, 5, "convertia-x")])
record("must-fire: a finding of a DIFFERENT rule on the pinned line does not satisfy the pin",
       m.missing_must_fires([_f("convertia-y", _MPATH, 5)], _markers) == [(_MPATH, 5, "convertia-x")])
record("must-fire: an ABSOLUTE finding path is normalised before the pin lookup",
       m.missing_must_fires([_f("convertia-x", str(m.ROOT / "scripts" / "semgrep-rules" / "fixtures" / "x.rs"), 5)],
                            _markers) == [])
ff = m.fixture_fire_markers()
record("fire-markers: the committed b1+d fixtures pin >=7 must-fire positives (both refined rules armed)",
       len(ff) >= 7 and {"convertia-command-missing-env-clear", "convertia-macos-command-missing-stage-for-tcc"}
       <= {i for ids in ff.values() for i in ids})

# --- 9. the P4.85 text<->matcher command census (counter + comparator) --------------------------
record("census-text: target-absent (no app source) -> {}", m.command_census_text([]) == {})
_td2 = Path(tempfile.mkdtemp(prefix="g24-sast-p485-"))
try:
    _cdir = _td2 / "census"
    _cdir.mkdir(parents=True)
    (_cdir / "c.rs").write_text(
        "fn f(){ let a = Command::new(x); let b = std::process::Command::new(y); let c = Command :: new(z); }\n",
        encoding="utf-8")
    record("census-text: bare + qualified + spaced `Command::new(` all count (3 in one file)",
           list(m.command_census_text([_cdir]).values()) == [3])
    (_cdir / "c.rs").write_text('// Command::new(a) in a comment\nlet s = "Command::new(b) in a string";\n',
                                encoding="utf-8")
    record("census-text: a Command::new in a comment/string does NOT count (blanking)",
           m.command_census_text([_cdir]) == {})
    (_cdir / "c.rs").write_text("fn f(){ let t = TokioCommand::new(x); }\n", encoding="utf-8")
    record("census-text: an identifier-tail form (TokioCommand::new) does NOT count (word boundary)",
           m.command_census_text([_cdir]) == {})

    record("census-gaps: text == matched -> no gap", m.census_gaps({"a.rs": 2}, {"a.rs": 2}) == [])
    record("census-gaps: text > matched -> the file is flagged (a matcher-invisible spawn)",
           m.census_gaps({"a.rs": 2}, {"a.rs": 1}) == ["a.rs"])
    record("census-gaps: matched > text is benign (a qualified call can match twice)",
           m.census_gaps({"a.rs": 1}, {"a.rs": 3}) == [])
    record("census-gaps: a file the matcher never reported at all is flagged",
           m.census_gaps({"a.rs": 1}, {}) == ["a.rs"])

    # --- 10. the P4.85 macOS-cfg homing scan ----------------------------------------------------
    record("macos-homing: target-absent (no app source) -> []", m.misplaced_macos_cfg([]) == [])
    _iso = _td2 / "homing" / "src" / "isolation"
    (_iso / "macos").mkdir(parents=True)
    (_iso / "mod.rs").write_text('#[cfg(target_os = "macos")]\nfn mac_leak() {}\n', encoding="utf-8")
    record("macos-homing: a mac-cfg in isolation/mod.rs (outside the macos module) is flagged",
           len(m.misplaced_macos_cfg([_td2 / "homing"])) == 1)
    (_iso / "mod.rs").write_text("fn cross_platform() {}\n", encoding="utf-8")
    (_iso / "macos.rs").write_text('#[cfg(target_os = "macos")]\nfn staged() {}\n', encoding="utf-8")
    (_iso / "macos" / "tcc.rs").write_text('#[cfg(target_os = "macos")]\nfn stage_for_tcc() {}\n', encoding="utf-8")
    record("macos-homing: mac-cfg in isolation/macos.rs + isolation/macos/ is the sanctioned home (not flagged)",
           m.misplaced_macos_cfg([_td2 / "homing"]) == [])
    (_iso / "mod.rs").write_text('// on target_os = "macos" we stage first\nfn cross_platform() {}\n',
                                 encoding="utf-8")
    record("macos-homing: a mac-cfg mention in a COMMENT does not count (comment blanking, strings kept)",
           m.misplaced_macos_cfg([_td2 / "homing"]) == [])
    _other = _td2 / "homing" / "src" / "platform"
    _other.mkdir(parents=True)
    (_other / "mac.rs").write_text('#[cfg(target_os = "macos")]\nfn os_shim() {}\n', encoding="utf-8")
    record("macos-homing: a mac-cfg OUTSIDE the isolation tree (crate::platform) is not this scan's business",
           m.misplaced_macos_cfg([_td2 / "homing"]) == [])
    (_iso / "mod.rs").write_text('fn f() { if std::env::consts::OS == "macos" { stage(); } }\n',
                                 encoding="utf-8")
    record("macos-homing: a RUNTIME os-check (consts::OS == \"macos\") outside the macos module is flagged too",
           len(m.misplaced_macos_cfg([_td2 / "homing"])) == 1)
    (_iso / "mod.rs").write_text("fn cross_platform() {}\n", encoding="utf-8")
finally:
    shutil.rmtree(_td2, ignore_errors=True)

# --- 11. structural pins on the refined (b1)/(d) rule text --------------------------------------
_yaml = (m.PROJECT_RULES / "process-isolation.yaml").read_text(encoding="utf-8")
_blocks = m._RULE_SPLIT_RE.split(_yaml)
_d_block = next((b for b in _blocks if "convertia-macos-command-missing-stage-for-tcc" in b), "")
_b1_block = next((b for b in _blocks if "convertia-command-missing-env-clear" in b), "")
record("yaml-pin: rule (d) is paths-scoped to the macOS isolation module AND fixture-bound",
       all(g in _d_block for g in ('"**/isolation/macos.rs"', '"**/isolation/macos/**"',
                                   '"**/fixtures/d-stage-tcc.rs"')))
record("yaml-pin: rule (d) keys on the literal standalone stage_for_tcc call (inline + adjacent-arg forms)",
       ".arg(stage_for_tcc(...))" in _d_block and "let $S = stage_for_tcc(...);" in _d_block)
record("yaml-pin: rule (d) carries the stage-then-build adjacency arm (the split-builder shape)",
       "let mut $C = Command::new(...);" in _d_block)
record("yaml-pin: rule (b1) carries the split-builder scrub-FIRST adjacent suppression",
       "let mut $C = Command::new(...);" in _b1_block and "$C.env_clear();" in _b1_block)


# --- 12. the T11 seam pin (the 2026-08-30 P4.26 adjudication) -----------------------------------
_seam_ok = (
    'pub(crate) fn stage_for_tcc(a: &P) -> R { s(a) }\n'
    'pub(crate) fn engine_input(a: &P) -> R {\n'
    '    #[cfg(target_os = "macos")]\n'
    '    { stage_for_tcc(a).map(C) }\n'
    '    #[cfg(not(target_os = "macos"))]\n'
    '    { Ok(b(a)) }\n'
    '}\n'
)
record("t11-seam-pin: the LIVE isolation/macos.rs satisfies the pin (the real-file leg)",
       m.t11_seam_pin() == [])
record("t11-seam-pin: a well-formed seam (macOS arm + standalone call) passes",
       m.t11_seam_pin(_seam_ok) == [])
record("t11-seam-pin: engine_input WITHOUT the stage_for_tcc call is flagged (the dropped-plumbing revert)",
       any("never calls" in p for p in m.t11_seam_pin(_seam_ok.replace("stage_for_tcc(a).map(C)", "Ok(b(a))"))))
record("t11-seam-pin: a renamed engine_input is flagged (the seam-rename class)",
       any("engine_input" in p for p in m.t11_seam_pin(_seam_ok.replace("fn engine_input", "fn engine_source"))))
record("t11-seam-pin: a renamed stage_for_tcc is flagged (the load-bearing-name class)",
       any("absent/renamed" in p for p in m.t11_seam_pin(_seam_ok.replace("stage_for_tcc", "stage_tcc"))))
record("t11-seam-pin: a macOS arm + call living only in a COMMENT does not satisfy (comment-blanked)",
       any("no POSITIVE macOS-cfg arm" in p
           for p in m.t11_seam_pin('fn stage_for_tcc(a: &P) -> R { s(a) }\n'
                                   'fn engine_input(a: &P) -> R {\n'
                                   '    // #[cfg(target_os = "macos")] stage_for_tcc(a)\n'
                                   '    Ok(b(a))\n'
                                   '}\n')))
record("t11-seam-pin: a VANISHED macOS arm with a surviving not()-arm is flagged (the R2 opus probe 1)",
       any("no POSITIVE macOS-cfg arm" in p
           for p in m.t11_seam_pin(_seam_ok.replace('#[cfg(target_os = "macos")]', '#[cfg(feature = "x")]'))))
record("t11-seam-pin: the call sitting ONLY in the not()-arm is flagged (the R2 opus probe 2, arm-scoped)",
       any("macOS arm never calls" in p
           for p in m.t11_seam_pin('pub(crate) fn stage_for_tcc(a: &P) -> R { s(a) }\n'
                                   'pub(crate) fn engine_input(a: &P) -> R {\n'
                                   '    #[cfg(target_os = "macos")]\n'
                                   '    { Ok(o(a)) }\n'
                                   '    #[cfg(not(target_os = "macos"))]\n'
                                   '    { let _x = stage_for_tcc(a); Ok(b(a)) }\n'
                                   '}\n')))


# --- 13. the engine argv contract, the G29 (b) split (§3.5) ---------------------------------------------------
# ABSENCE: (b2)/(b3a) are whole-module `pattern-regex` rules. A rule's one `paths:` key must include exactly the
# module's file and directory globs and exclude nothing - pinned line for line, since a glob moved from the include
# to an exclude list still appears in the block. Each regex is read from the YAML and run through Python's `re`
# (the 3-OS stand-in for the CI prelude's Semgrep run): over its engine fixtures - fixtures/engines/<engine>.rs, and
# fixtures/engines/<engine>/args.rs under the module directory, whose firing in the prelude proves the directory
# glob binds - it must hit exactly the lines pinned `sast-must-fire` for its rule - every planted form fires, no
# `sast-clean` line and no unpinned line does.
def _paths_lines(blk: str) -> list[str] | None:
    """The stripped, non-comment lines of a rule block's `paths:` mapping, or None unless the block holds exactly one
    `paths:` key, written in block style (a flow-style, anchored, aliased or repeated key fails the pin closed)."""
    lines = blk.split("\n")
    keys = [i for i, ln in enumerate(lines) if re.match(r"[ \t]*paths:", ln)]
    if len(keys) != 1 or not re.fullmatch(r"[ \t]*paths:[ \t]*", lines[keys[0]]):
        return None
    indent = len(lines[keys[0]]) - len(lines[keys[0]].lstrip())
    out = []
    for ln in lines[keys[0] + 1:]:
        s = ln.strip()
        if not s or s.startswith("#"):
            continue
        if len(ln) - len(ln.lstrip()) <= indent:
            break
        out.append(s)
    return out


def _fixture_lines(rel: str, rx: re.Pattern | None, rid: str) -> tuple[set[int], set[int], set[int]]:
    """(the lines `rx` hits, the lines pinned `sast-must-fire` for `rid`, the lines pinned `sast-clean` for `rid`)
    of the fixture at FIXTURES/`rel`; an absent fixture reads empty, so its leg fails."""
    p = m.FIXTURES / rel
    fx = p.read_text(encoding="utf-8") if p.is_file() else ""
    return ({i for i, ln in enumerate(fx.split("\n"), 1) if rx is not None and rx.search(ln)},
            {ln for ln, rids in m.parse_fire_markers(fx).items() if rid in rids},
            {ln for ln, rids in m.parse_clean_markers(fx).items() if rid in rids})


_ea_yaml = (m.PROJECT_RULES / "engine-argv.yaml").read_text(encoding="utf-8")
_ea_blocks = {}
for _blk in m._RULE_SPLIT_RE.split(_ea_yaml):
    _mid = re.search(r"(?m)^\s*-\s*id:\s*(\S+)", _blk)
    if _mid:
        _ea_blocks[_mid.group(1)] = _blk
_EA_RULES = {"convertia-pandoc-unsafe-resource-path": ("b2", "pandoc"),
             "convertia-soffice-accept-socket": ("b3a", "libreoffice")}
record("engine-argv: the rule file holds exactly the (b2) and (b3a) absence rules", set(_ea_blocks) == set(_EA_RULES))
for _rid, (_tag, _eng) in _EA_RULES.items():
    _blk = _ea_blocks.get(_rid, "")
    record(f"engine-argv yaml-pin: ({_tag}) is a pattern-regex rule over the whole {_eng} module - its one paths: key "
           f"includes exactly engines/{_eng}.rs and engines/{_eng}/** and excludes nothing - no spawn-chain pattern",
           "pattern-regex:" in _blk
           and _paths_lines(_blk) == [f'include: ["**/engines/{_eng}.rs", "**/engines/{_eng}/**"]']
           and "pattern-either" not in _blk and "Command::new" not in _blk)
    _mrx = re.search(r"(?m)^\s*pattern-regex:\s*'((?:[^']|'')*)'\s*$", _blk)
    _rx = re.compile(_mrx.group(1).replace("''", "'")) if _mrx else None
    _hits, _fire, _clean = _fixture_lines(f"engines/{_eng}.rs", _rx, _rid)
    record(f"engine-argv regex: ({_tag}) hits exactly the must-fire lines of fixtures/engines/{_eng}.rs "
           "(>= 3 planted forms; none of its >= 3 sast-clean lines)",
           _rx is not None and len(_fire) >= 3 and len(_clean) >= 3 and _hits == _fire)
    _hits, _fire, _ = _fixture_lines(f"engines/{_eng}/args.rs", _rx, _rid)
    record(f"engine-argv regex: ({_tag}) hits exactly the must-fire line of fixtures/engines/{_eng}/args.rs, the "
           "fixture under the module directory",
           _rx is not None and len(_fire) >= 1 and _hits == _fire)

# PRESENCE: argv_contract_pin. The table itself is the contract the G29 row states.
record("argv-contract: ARGV_CONTRACT names the §3.5 flags of LibreOffice, pandoc and FFmpeg at their argv boxes",
       m.ARGV_CONTRACT == {
           "libreoffice": ("docs/plan/P7-office.md", "P7.8", ("-env:UserInstallation=",)),
           "pandoc": ("docs/plan/P7-office.md", "P7.25", ("--sandbox", "--resource-path")),
           "ffmpeg": ("docs/plan/P6-av-crosscat.md", "P6.9", ("-protocol_whitelist",)),
       })

# Hermetic temp roots with a one-engine contract; these legs never run main([]) on the real repo.
_td3 = Path(tempfile.mkdtemp(prefix="g24-sast-argv-"))
try:
    _C = {"ffmpeg": ("docs/plan/P6-x.md", "P6.9", ("-protocol_whitelist", "-nostdin"))}
    _MOD = "src-tauri/src/engines/ffmpeg.rs"
    _GOOD = ("pub(crate) fn plan() -> Vec<OsString> {\n"
             "    Vec::new()\n"
             "}\n"
             "#[cfg(test)]\n"
             "mod argv_contract {\n"
             "    #[test]\n"
             "    fn flags() {\n"
             "        let a = super::plan();\n"
             '        assert!(a.contains(&"-protocol_whitelist".into()));\n'
             '        assert!(a.contains(&"-nostdin".into()));\n'
             "    }\n"
             "}\n")
    _FLAGS_OUTSIDE = 'fn plan() -> Vec<OsString> { vec!["-protocol_whitelist".into(), "-nostdin".into()] }\n'
    _roots = [0]

    def _box(marker: str) -> str:
        return f"- [{marker}] **P6.9** [RUST] Apply the global FFmpeg flags · §3.5.1 · G38\n  needs: P6.8\n"

    def _pin(plan, files):
        """argv_contract_pin over a fresh root: docs/plan/P6-x.md holds `plan` (None: no plan file), plus `files`."""
        _roots[0] += 1
        root = _td3 / f"r{_roots[0]}"
        root.mkdir()
        if plan is not None:
            (root / "docs" / "plan").mkdir(parents=True)
            (root / "docs" / "plan" / "P6-x.md").write_text(plan, encoding="utf-8", newline="\n")
        for rel, body in files.items():
            (root / rel).parent.mkdir(parents=True, exist_ok=True)
            (root / rel).write_text(body, encoding="utf-8", newline="\n")
        return m.argv_contract_pin(root, _C)

    _p, _n = _pin(_box(" "), {})
    record("argv-contract: a [ ] box leaves its engine dormant (no module needed; a notice)",
           _p == [] and any("ffmpeg dormant" in x for x in _n))
    _p1, _n1 = _pin(_box("!"), {})
    _p2, _n2 = _pin(_box("!extern"), {})
    record("argv-contract: a [!] or [!extern] box leaves its engine dormant too",
           _p1 == [] and _p2 == [] and any("[!]" in x for x in _n1) and any("[!extern]" in x for x in _n2))
    _p, _ = _pin(_box("x"), {})
    record("argv-contract: a [x] box with the engine module absent FAILS",
           len(_p) == 1 and "no file of its engine module" in _p[0])
    _p, _ = _pin("- [x] **P6.90** [RUST] Another box · §3.5.1 · G38\n- [x] **P619** [RUST] A third box · G38\n", {})
    record("argv-contract: a box id the plan file does not carry ARMS the engine (fail-closed; module absent FAILS)",
           len(_p) == 1 and "does not resolve" in _p[0])
    _p, _ = _pin(None, {})
    record("argv-contract: an absent plan file ARMS the engine (fail-closed)",
           len(_p) == 1 and "does not resolve" in _p[0])
    _p, _ = _pin(_box(" ") + _box("x"), {})
    record("argv-contract: two header lines for the box id ARM the engine (fail-closed, never the first match)",
           len(_p) == 1 and "does not resolve" in _p[0])
    _p, _n = _pin(_box("x"), {_MOD: _GOOD})
    record("argv-contract: an armed engine whose argv_contract block carries every flag passes",
           _p == [] and any("ffmpeg armed" in x for x in _n))
    _p, _n = _pin(None, {_MOD: _GOOD})
    record("argv-contract: an unresolvable box arms, and a satisfied module passes armed",
           _p == [] and any("ffmpeg armed" in x and "does not resolve" in x for x in _n))
    _p, _ = _pin(_box("x"), {_MOD: _GOOD.replace('assert!(a.contains(&"-nostdin".into()));',
                                                 '// "-nostdin" is asserted by the caller')})
    record("argv-contract: a flag only in a COMMENT inside the block FAILS",
           len(_p) == 1 and "`-nostdin`" in _p[0])
    _p, _ = _pin(_box("x"), {_MOD: _GOOD.replace('assert!(a.contains(&"-nostdin".into()));',
                                                 '/* outer /* inner */ "-nostdin" */')})
    record("argv-contract: a flag inside a NESTED block comment FAILS (block comments nest, as in Rust)",
           len(_p) == 1 and "`-nostdin`" in _p[0])
    _p, _ = _pin(_box("x"), {_MOD: _GOOD.replace('        assert!(a.contains(&"-nostdin".into()));\n', "")
                                         .replace("    Vec::new()\n", '    vec!["-nostdin".into()]\n')})
    record("argv-contract: a flag only OUTSIDE the block (in plan() itself) FAILS",
           len(_p) == 1 and "`-nostdin`" in _p[0])
    _p, _ = _pin(_box("x"), {_MOD: "fn plan() -> Vec<OsString> { Vec::new() }\n"
                                   'mod argv_contract { fn t() { let _ = ("-protocol_whitelist", "-nostdin", '
                                   '"file://x"); } }\n'})
    record("argv-contract: a `//` inside a literal (`file://`) is no comment - the block closes on its own line",
           _p == [])
    _p, _ = _pin(_box("x"), {_MOD: 'mod argv_contract { fn t() { let _ = "file://x"; } }\n' + _FLAGS_OUTSIDE})
    record("argv-contract: ... so flags after that one-line block are OUTSIDE it (both FAIL)",
           len(_p) == 2 and all("carries no string literal" in x for x in _p))
    _p, _ = _pin(_box("x"), {_MOD: "mod argv_contract { fn t() { let _ = (\"{\", '{', '\\u{7B}'); } }\n"
                                   + _FLAGS_OUTSIDE})
    record("argv-contract: a brace inside a string or char literal does not move the block end (flags outside FAIL)",
           len(_p) == 2 and all("carries no string literal" in x for x in _p))
    _p, _ = _pin(_box("x"), {_MOD: _GOOD.replace('"-nostdin"', '"see \\"-nostdin"')})
    record("argv-contract: a flag after an escaped quote inside a literal does not count",
           len(_p) == 1 and "`-nostdin`" in _p[0])
    _p, _ = _pin(_box("x"), {_MOD: _GOOD.replace('"-nostdin"', '"x-nostdin"')})
    record("argv-contract: a flag in the middle of a longer literal does not count (a literal must START with it)",
           len(_p) == 1 and "`-nostdin`" in _p[0])
    _p, _ = _pin(_box("x"), {_MOD: _GOOD.replace('"-protocol_whitelist"', 'r##"-protocol_whitelist "#}"##')
                                         .replace('"-nostdin"', 'r"-nostdin\\"')})
    record("argv-contract: raw string literals count, read raw (only the opening's own hash count closes one - an "
           "inner `\"#}` ends neither r##\"…\"## nor the block; a trailing backslash escapes nothing)",
           _p == [])
    _p, _ = _pin(_box("x"), {_MOD: "mod argv_contract {\n    fn a() {}\n"
                                   '    fn b() { let _ = ("-protocol_whitelist", "-nostdin"); }\n}\n'})
    record("argv-contract: a nested item's closing brace does not end the block early", _p == [])
    _p, _ = _pin(_box("x"), {_MOD: _GOOD.replace("    fn flags() {\n",
                                                 "    fn flags() {\n        let _q = ('\"', '\\\"');\n")})
    record("argv-contract: a '\"' or '\\\"' char literal opens no string (the flags after it still count)", _p == [])
    _p, _ = _pin(_box("x"), {_MOD: _GOOD.replace("    #[test]\n",
                                                 "    fn s(v: &'static str) { let _w: &'static str = v; }\n"
                                                 "    #[test]\n")})
    record("argv-contract: a lifetime ('static) opens no char literal (a brace between two stays a brace)",
           _p == [])
    _p, _ = _pin(_box("x"), {_MOD: _GOOD, "src-tauri/src/engines/ffmpeg/extra.rs": "mod argv_contract {}\n"})
    record("argv-contract: two `mod argv_contract` blocks across the engine module FAIL (exactly one)",
           len(_p) == 1 and "2 `mod argv_contract` blocks" in _p[0])
    _p, _ = _pin(_box("x"), {_MOD: "fn plan() {}\n#[cfg(test)]\nmod argv_contract;\n",
                             "src-tauri/src/engines/ffmpeg/argv_contract.rs":
                                 '#[test]\nfn t() { let _ = ("-protocol_whitelist", "-nostdin"); }\n'})
    record("argv-contract: an out-of-line `mod argv_contract;` FAILS, naming the inline block it needs",
           len(_p) == 1 and "out of line" in _p[0])
    _p, _ = _pin(_box("x"), {"src-tauri/src/engines/ffmpeg/mod.rs": "mod plan;\n",
                             "src-tauri/src/engines/ffmpeg/plan/mod.rs": "mod args;\n",
                             "src-tauri/src/engines/ffmpeg/plan/args.rs": _GOOD})
    record("argv-contract: a directory engine module is read whole, at any depth - the block may sit in any file "
           "under it (here ffmpeg/plan/args.rs, two levels down)",
           _p == [])
    _p, _ = _pin(_box("x"), {"src-tauri/src/engines/pandoc.rs": _GOOD, "src-tauri/src/engines/ffmpegx.rs": _GOOD})
    record("argv-contract: a block in another engine's module (pandoc.rs, ffmpegx.rs) does not count",
           len(_p) == 1 and "no file of its engine module" in _p[0])
    _p, _ = _pin(_box("x"), {_MOD: '// mod argv_contract { "-protocol_whitelist" "-nostdin" }\n'
                                   'const DOC: &str = "mod argv_contract { }";\n'})
    record("argv-contract: `mod argv_contract {` inside a comment or a string is no block (FAILS)",
           len(_p) == 1 and "no file of its engine module" in _p[0])
    _p, _ = _pin(_box("x"), {_MOD: _GOOD[: _GOOD.rindex("}")]})
    record("argv-contract: a block that never closes FAILS", len(_p) == 1 and "never closes" in _p[0])
finally:
    shutil.rmtree(_td3, ignore_errors=True)

# The live tree: the pin holds (each engine dormant until its argv box is [x], then satisfied), and every box the
# table names resolves to exactly one plan box - a stale id would arm its engine fail-closed.
_lp, _ = m.argv_contract_pin()
record("argv-contract: the LIVE tree passes the pin, and every ARGV_CONTRACT box resolves to one plan box",
       _lp == [] and all(m._box_marker(m.ROOT / pf, bx) is not None for pf, bx, _ in m.ARGV_CONTRACT.values()))

# The main() wiring: driven over a temp root with the module's ROOT, contract, Semgrep probe and shellcheck leg
# swapped for the two calls and restored after, so the real repo is never scanned.
_saved = (m.ROOT, m.ARGV_CONTRACT, m._semgrep, m.shellcheck_scan)
_td4 = Path(tempfile.mkdtemp(prefix="g24-sast-main-"))
try:
    (_td4 / "docs" / "plan").mkdir(parents=True)
    m.ROOT = _td4
    m.ARGV_CONTRACT = {"ffmpeg": ("docs/plan/P6-x.md", "P6.9", ("-protocol_whitelist",))}
    m._semgrep = lambda: None
    m.shellcheck_scan = lambda: 0
    _runs = []
    for _mk in (" ", "x"):
        (_td4 / "docs" / "plan" / "P6-x.md").write_text(f"- [{_mk}] **P6.9** [RUST] Apply the flags · G38\n",
                                                        encoding="utf-8", newline="\n")
        _out = io.StringIO()
        with contextlib.redirect_stdout(_out), contextlib.redirect_stderr(_out):
            _runs.append((m.main([]), _out.getvalue()))
    record("argv-contract: main() runs the pin - dormant exits 0 naming the leg, an armed engine without its "
           "module exits 1",
           _runs[0][0] == 0 and "argv-contract" in _runs[0][1] and "ffmpeg dormant" in _runs[0][1]
           and _runs[1][0] == 1 and "argv contract: ffmpeg is armed" in _runs[1][1])
finally:
    m.ROOT, m.ARGV_CONTRACT, m._semgrep, m.shellcheck_scan = _saved
    shutil.rmtree(_td4, ignore_errors=True)

failed = [n for n, ok in results if not ok]
print(f"\n{len(results) - len(failed)}/{len(results)} legs passed")
if failed:
    print("FAILED:", *failed, sep="\n  - ")
    sys.exit(1)
sys.exit(0)
