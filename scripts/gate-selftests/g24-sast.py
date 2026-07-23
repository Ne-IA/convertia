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
forms). The REAL semgrep-over-fixtures armed canary runs inside check-sast in the CI `sast` job
(semgrep present). stdlib-only. Exit 0 = all held.
"""
import importlib.machinery
import importlib.util
import sys
from pathlib import Path

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
record("rule discovery: 15 project rules found", len(ids) == 15)
record("rule discovery: the load-bearing rules are present",
       {"convertia-net-ban-std-tokio", "convertia-net-ban-raw-socket-ffi",
        "convertia-tauri-command-missing-fs-guard", "convertia-tauri-command-name-rename-forbidden",
        "convertia-webview-taint-to-dom-sink", "convertia-opener-egress-non-constant"} <= ids)
record("rule discovery: every NET_BAN_RULE_ID is a real discovered rule", m.NET_BAN_RULE_IDS <= ids)

# --- 2. planted-positive fixtures present, incl. the resolved-path engine fixtures at the module path ---
# (one .rs may cover >1 rule — libreoffice.rs feeds both b3a + b3b — so the per-RULE firing guarantee is
# the check-sast canary prelude, not a file count; here we assert the fixtures + the path-scoped engine
# fixtures exist at engines/<engine>.rs so the `paths: include` globs bind.)
fixtures = list(m.FIXTURES.rglob("*.rs")) + list(m.FIXTURES.rglob("*.ts"))
record("fixtures: present incl. the resolved-path engine fixtures under engines/",
       len(fixtures) >= 10 and all((m.FIXTURES / "engines" / e).is_file()
                                   for e in ("pandoc.rs", "libreoffice.rs", "ffmpeg.rs", "image.rs")))

# --- 2b. the contract a-j tag set is pinned INDEPENDENTLY of the discovered ids (not count-only) ---
id_tags = m.project_rule_id_tags()
record("tags: every project rule carries a convertia-rule tag", len(id_tags) == len(ids))
record("tags: declared tag set == the frozen contract a-j set EXPECTED_RULE_TAGS",
       set(id_tags.values()) == m.EXPECTED_RULE_TAGS)
record("tags: a renamed/dropped rule (tag set shrinks) != EXPECTED_RULE_TAGS (count-only would miss it)",
       (set(id_tags.values()) - {"b4"}) != m.EXPECTED_RULE_TAGS)

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

# --- 6. T2c store-load count -------------------------------------------------------------------
import tempfile, shutil
record("store-count: target-absent (no app source) -> 0", m.store_load_count([]) == 0)
_td = Path(tempfile.mkdtemp(prefix="g24-sast-"))
try:
    (_td / "a.rs").write_text('let s = app.store(STORE);\nlet t = Store::load(&app, STORE);\n', encoding="utf-8")
    record("store-count: two store-open call sites are counted (>1 -> the gate fails)", m.store_load_count([_td]) >= 2)
    (_td / "a.rs").write_text('let s = app.store(STORE);\n', encoding="utf-8")
    record("store-count: a single store-open call site -> 1 (allowed)", m.store_load_count([_td]) == 1)
    # P2.85 refinement — an atomic `x.store(val, Ordering::…)` is NOT a Tauri store-open -> not counted:
    (_td / "a.rs").write_text('self.ready.store(true, std::sync::atomic::Ordering::Release);\n', encoding="utf-8")
    record("store-count: an atomic .store(val, Ordering) is NOT counted (the P2.85 false-positive fix)", m.store_load_count([_td]) == 0)
    (_td / "a.rs").write_text('use std::sync::atomic::Ordering;\nself.f.store(1, Ordering::Relaxed);\n', encoding="utf-8")
    record("store-count: a short-form atomic .store(val, Ordering::Relaxed) is NOT counted", m.store_load_count([_td]) == 0)
    # a store-open MENTIONED in a comment/string is not counted (the _blank_rs_noncode blanking):
    (_td / "a.rs").write_text('// app.store(STORE) in a comment\nlet x = "app.store(STORE) in a string";\n', encoding="utf-8")
    record("store-count: a store-open in a comment/string is not counted (blanking)", m.store_load_count([_td]) == 0)
    # the real store-open ALONGSIDE an atomic counts EXACTLY the real one (=1, still allowed) — no false-negative:
    (_td / "a.rs").write_text('self.ready.store(true, Ordering::Release);\nlet s = app.store(&path);\n', encoding="utf-8")
    record("store-count: a real store-open + an atomic -> 1 (the atomic is excluded, the real one still counts)", m.store_load_count([_td]) == 1)
    # the marker is the specific `Ordering::` (not a bare `Ordering`), so a real store-open with a generic
    # arg named Ordering STILL counts (no false-negative on this contrived-but-real form; G1-review-pinned):
    (_td / "a.rs").write_text('let s = app.store(resolve::<Ordering>(&path));\n', encoding="utf-8")
    record("store-count: a real store-open with a `<Ordering>` generic arg still counts (Ordering:: marker, not bare)", m.store_load_count([_td]) == 1)

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

failed = [n for n, ok in results if not ok]
print(f"\n{len(results) - len(failed)}/{len(results)} legs passed")
if failed:
    print("FAILED:", *failed, sep="\n  - ")
    sys.exit(1)
sys.exit(0)
