#!/usr/bin/env python3
"""g24-sast.py - G24 self-test for check-sast (P0.4.2, G29 Semgrep leg).

Runs on all 3 OS in the continuous-armed-canary WITHOUT semgrep installed, so it proves check-sast's
pure, importable LOGIC: (1) the project rule-ids auto-discover from scripts/semgrep-rules/project/ and
match the committed rule set; (2) every project rule has a planted-positive fixture; (3) the
armed-canary detector (missing_canaries) flags a rule that stopped firing; (4) the net-allow-list
parser ignores comments/blanks; (5) the net-ban allow-list FILTER drops only allow-listed net findings.
The REAL semgrep-over-fixtures armed canary runs inside check-sast in the CI `sast` job (semgrep
present). stdlib-only. Exit 0 = all held.
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
finally:
    shutil.rmtree(_td, ignore_errors=True)

failed = [n for n, ok in results if not ok]
print(f"\n{len(results) - len(failed)}/{len(results)} legs passed")
if failed:
    print("FAILED:", *failed, sep="\n  - ")
    sys.exit(1)
sys.exit(0)
