#!/usr/bin/env python3
"""g24-ts-gate-contract.py - G24 self-test for check-ts-gate (P0.4.7, G5/G6/G13).

Proves the structural freeze CANNOT be gutted (the strict / no-`any` / tool-wiring contract stays
non-empty) and that every LIVE-tier assertion CATCHES its planted violation once the P1 frontend lands:
a relaxed tsconfig strict flag (G6/G13), a missing / switched-off eslint rule (G5), and a HALF-WIRED
tool — a config without its package.json script, or a script without its config (assert_toolchain_wiring,
the per-tool config<->script agreement) — while a BARE manifest with neither is TOLERATED (the P1.2
staggered-landing posture, the regression this gate-fix locks in). Plus the JSONC tolerance + the
multi-level `extends` follow + the comment/string-aware scans. The live tsc/eslint/prettier/vitest are
LIVE since P1 (package.json + the toolchain exist) so the real gate runs them. The T2 DOM/eval sink ban
(security-concept §5 T2): the freeze (each sink rule declared once at "error", `noInlineConfig: true`)
catches a dropped, doubled, downgraded or comment-only rule and a missing inline lock; the armed canary's
verdict over synthetic eslint JSON catches a silent pin, a warn-only pin, a fatal parse and an unpinned
contract rule, and the committed fixture pins every contract rule plus the inline-directive decoy; the
scope verdict over synthetic per-file configs catches an ignored file, a missing, weaker or warn-level
sink rule, a differing, extra or missing language, language-option or settings entry, an inline config
left on, a processor and an absent, unarmed or context-less reference, and its probe runner fails closed
on every malformed result; the real run's verdict catches any message and an unlinted src/ source; the
src/ enumeration reaches nested directories for the scope probe and the real run alike; the canary, the
scope probe and the real run are pinned to the eslint input each reads, and the live tier and main() to
call the canary, the scope leg, the real run and the freeze. stdlib-only.
Exit 0 = all held.
"""
import copy
import importlib.machinery
import importlib.util
import json
import subprocess
import sys
import tempfile
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-ts-gate"
_loader = importlib.machinery.SourceFileLoader("ctg", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("ctg", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def in_root(files: dict, fn):
    """Write {relpath: content} into a temp dir, point m.ROOT at it, call fn() (which reads m.ROOT-relative
    files at call-time), restore m.ROOT, return the result."""
    saved = m.ROOT
    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        for rel, content in files.items():
            p = root / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(content, encoding="utf-8")
        m.ROOT = root
        try:
            return fn()
        finally:
            m.ROOT = saved


def tmp_file(name: str, content: str) -> Path:
    td = tempfile.mkdtemp()
    p = Path(td) / name
    p.write_text(content, encoding="utf-8")
    return p


# --- the frozen CONTRACT (freeze_contract over the module constants) --------------------------
record("freeze_contract: the real committed contract is clean", m.freeze_contract() == 0)
record("contract: TSCONFIG_REQUIRED pins `strict` + the index/override/fallthrough extras",
       m.TSCONFIG_REQUIRED.get("strict") is True
       and {"noUncheckedIndexedAccess", "noImplicitOverride", "noFallthroughCasesInSwitch"} <= set(m.TSCONFIG_REQUIRED))
record("contract: ESLINT_REQUIRED_RULES carries no-explicit-any + the fc.gen ban",
       any("no-explicit-any" in tok for _, tok in m.ESLINT_REQUIRED_RULES)
       and any("fc.gen" in tok for _, tok in m.ESLINT_REQUIRED_RULES))
record("contract: PKG_REQUIRED_SCRIPTS wires tsc/eslint/prettier/vitest",
       set(m.PKG_REQUIRED_SCRIPTS) >= {"tsc", "eslint", "prettier", "vitest"})


def _gutted_tsconfig_required():
    saved = m.TSCONFIG_REQUIRED
    m.TSCONFIG_REQUIRED = {}
    try:
        return m.freeze_contract()
    finally:
        m.TSCONFIG_REQUIRED = saved


record("freeze_contract: a gutted TSCONFIG_REQUIRED is caught", _gutted_tsconfig_required() >= 1)

# --- _read_jsonc: JSONC tolerance (comments + trailing commas) without mis-stripping strings --
JSONC = '{\n  // a line comment\n  "compilerOptions": { /* block */ "strict": true, },\n}\n'
record("_read_jsonc: // + /* */ comments + a trailing comma parse", m._read_jsonc(tmp_file("t.json", JSONC))[1] is None)
record("_read_jsonc: a `//` inside a string value is NOT mis-stripped",
       (lambda o: o is not None and o.get("u") == "http://x/y")(m._read_jsonc(tmp_file("u.json", '{ "u": "http://x/y" }'))[0]))
record("_read_jsonc: malformed JSON -> error signal", m._read_jsonc(tmp_file("b.json", "{ not json "))[1] is not None)

# --- _tsconfig_options: a single-level local `extends` is followed ----------------------------
def _extends_case():
    return in_root({"base.json": '{ "compilerOptions": { "strict": true } }',
                    "tsconfig.json": '{ "extends": "./base.json", "compilerOptions": { "noImplicitOverride": true } }'},
                   lambda: m._tsconfig_options(m.ROOT / "tsconfig.json")[0])


_eo = _extends_case()
record("_tsconfig_options: a flag set only in the extended base is picked up (no extends false-negative)",
       _eo.get("strict") is True and _eo.get("noImplicitOverride") is True)

# --- assert_tsconfig_strict (G6/G13) ----------------------------------------------------------
_GOOD_TS = ('{ "compilerOptions": { "strict": true, "noUncheckedIndexedAccess": true, '
            '"noImplicitOverride": true, "noFallthroughCasesInSwitch": true } }')
record("tsconfig: all required strict flags set -> clean",
       in_root({"tsconfig.json": _GOOD_TS}, m.assert_tsconfig_strict) == 0)
record("tsconfig: strict relaxed to false -> caught",
       in_root({"tsconfig.json": _GOOD_TS.replace('"strict": true', '"strict": false')}, m.assert_tsconfig_strict) >= 1)
record("tsconfig: a missing required flag (noUncheckedIndexedAccess) -> caught",
       in_root({"tsconfig.json": '{ "compilerOptions": { "strict": true, "noImplicitOverride": true, "noFallthroughCasesInSwitch": true } }'},
               m.assert_tsconfig_strict) >= 1)
record("tsconfig: absent -> target-absent skip (no finding)",
       in_root({}, m.assert_tsconfig_strict) == 0)
record("tsconfig: malformed -> exit-2 signal",
       in_root({"tsconfig.json": "{ not json"}, m.assert_tsconfig_strict) == 2)

# --- assert_eslint_rules (G5) -----------------------------------------------------------------
_GOOD_ESLINT = (
    'export default [{ rules: {\n'
    '  "@typescript-eslint/no-explicit-any": "error",\n'
    '  "no-restricted-syntax": ["error", { selector: "X", message: "fc.gen() needs a shrink wrapper" }],\n'
    '  "no-restricted-imports": ["error", { patterns: [{ group: ["@tauri-apps/api", "@tauri-apps/plugin-*"] }] }],\n'
    '} }];\n')
record("eslint: both required rules present + on -> clean",
       in_root({"eslint.config.js": _GOOD_ESLINT}, m.assert_eslint_rules) == 0)
record("eslint: missing no-explicit-any -> caught",
       in_root({"eslint.config.js": 'export default [{ rules: { "no-restricted-syntax": ["error", { message: "fc.gen wrap" }] } }];'},
               m.assert_eslint_rules) >= 1)
record("eslint: no-explicit-any switched to \"off\" -> caught",
       in_root({"eslint.config.js": _GOOD_ESLINT.replace('"@typescript-eslint/no-explicit-any": "error"',
                                                          '"@typescript-eslint/no-explicit-any": "off"')},
               m.assert_eslint_rules) >= 1)
record("eslint: a rule only in a // comment -> counted MISSING (comment-stripped)",
       in_root({"eslint.config.js": 'export default [{ rules: {\n  // "@typescript-eslint/no-explicit-any": "error" fc.gen\n} }];'},
               m.assert_eslint_rules) >= 1)
record("eslint: absent -> target-absent skip (no finding)", in_root({}, m.assert_eslint_rules) == 0)

# --- _strip_js_comments: keeps strings, drops comments ----------------------------------------
record("_strip_js_comments: a // inside a string is kept, a real // comment is dropped",
       'http://keep/me' in m._strip_js_comments('const u = "http://keep/me"; // drop this fc.gen note')
       and 'drop this' not in m._strip_js_comments('const u = "http://keep/me"; // drop this fc.gen note'))
# HARDENING (P2.106 G1 Opus P0): a lone possessive apostrophe/contraction in a comment must NOT open a
# phantom quote span that swallows a later quoted rule token (the strings-first-regex bug). The single-pass
# scanner recognizes the comment first, so the apostrophe inside it never opens a string.
record("_strip_js_comments: a lone apostrophe in a comment does NOT corrupt a later quoted rule token",
       '@typescript-eslint/no-explicit-any' in m._strip_js_comments(
           "// the plugin's error() and the shell's cmd -- two contractions\n"
           "const r = '@typescript-eslint/no-explicit-any';\n")
       and "plugin" not in m._strip_js_comments(
           "// the plugin's error() and the shell's cmd -- two contractions\n"
           "const r = '@typescript-eslint/no-explicit-any';\n"))
record("_strip_js_comments: a block comment /* it's */ with an apostrophe is dropped, later token kept",
       '@typescript-eslint/no-explicit-any' in m._strip_js_comments(
           "/* it's a note */ const r = '@typescript-eslint/no-explicit-any';")
       and "note" not in m._strip_js_comments("/* it's a note */ const r = '@typescript-eslint/no-explicit-any';"))
# CAGE-B FREEZE (P2.106): the §5.1 single-IPC-consumer import-ban tokens are frozen present.
record("eslint: the two @tauri-apps IPC-surface ban tokens present -> clean (part of the _GOOD_ESLINT clean leg)",
       in_root({"eslint.config.js": _GOOD_ESLINT}, m.assert_eslint_rules) == 0)
record("eslint: the @tauri-apps/plugin-* IPC-ban token dropped -> caught",
       in_root({"eslint.config.js": _GOOD_ESLINT.replace(', "@tauri-apps/plugin-*"', '')},
               m.assert_eslint_rules) >= 1)
record("eslint: the @tauri-apps/api IPC-ban token dropped -> caught",
       in_root({"eslint.config.js": _GOOD_ESLINT.replace('"@tauri-apps/api", ', '')},
               m.assert_eslint_rules) >= 1)
# THE message-defeat regression (G1 Sonnet P0): the ban's `message:` string legitimately NAMES the banned
# packages, so the prior bare-substring freeze passed even with the functional `group` array GUTTED. This
# fixture mirrors the REAL config's redundancy (tokens in the array AND the message); the quote-delimited
# freeze must still catch a gutted array. Both legs would have (wrongly) passed before the quote fix.
_ESLINT_MSG_REDUNDANT = (
    'export default [{ rules: {\n'
    '  "@typescript-eslint/no-explicit-any": "error",\n'
    '  "no-restricted-syntax": ["error", { message: "fc.gen wrap" }],\n'
    '  "no-restricted-imports": ["error", { patterns: [{ group: ["@tauri-apps/api", "@tauri-apps/plugin-*"],\n'
    '    message: "Only src/lib/ipc may import @tauri-apps/api or a @tauri-apps/plugin-* package" }] }],\n'
    '} }];\n')
record("eslint: tokens in BOTH the group array AND the message -> clean (the array element is present)",
       in_root({"eslint.config.js": _ESLINT_MSG_REDUNDANT}, m.assert_eslint_rules) == 0)
record("eslint: the group array GUTTED but the message still NAMES the packages -> STILL caught (no message-defeat)",
       in_root({"eslint.config.js": _ESLINT_MSG_REDUNDANT.replace(
           'group: ["@tauri-apps/api", "@tauri-apps/plugin-*"],', 'group: [],')},
               m.assert_eslint_rules) >= 1)
def _gutted_import_bans():
    saved = m.ESLINT_REQUIRED_IMPORT_BANS
    m.ESLINT_REQUIRED_IMPORT_BANS = [("x", "@tauri-apps/api")]  # plugin-* dropped
    try:
        return m.freeze_contract()
    finally:
        m.ESLINT_REQUIRED_IMPORT_BANS = saved


record("freeze_contract: a gutted ESLINT_REQUIRED_IMPORT_BANS (plugin-* dropped) is caught",
       _gutted_import_bans() >= 1)

# --- assert_toolchain_wiring: per-tool config<->script AGREEMENT (the P1.2 staggered-landing fix) -----
# Each tool's config presence and its package.json script must AGREE: NEITHER = target-absent (tolerated);
# a config without its script, or a script without its config, = a half-wired tool (caught). The
# stylelint config is coupled to the eslint flat config (they land together in P1.33).
_FULL_PKG = ('{ "scripts": { "typecheck": "tsc --noEmit", "lint": "eslint . && stylelint **/*.css", '
             '"fmt": "prettier --check .", "test": "vitest run" } }')
_FULL_CFG = {".prettierrc": "{}", ".stylelintrc.json": "{}", "vitest.config.ts": "export default {}",
             "tsconfig.json": _GOOD_TS, "eslint.config.js": _GOOD_ESLINT}


def _wiring(files):
    return in_root(files, lambda: m.assert_toolchain_wiring(m.ROOT / "package.json"))


# THE regression test for the P1.2 escalation: a BARE manifest (no toolchain configs, no toolchain
# scripts -- only the `tauri` script P1.2.3 wires) is TOLERATED (0), not 7-violations-red as it was
# before the fix (package.json alone armed the whole prettier/stylelint/vitest + scripts contract).
record("wiring: a bare P1.2 manifest (no configs, no toolchain scripts) -> target-absent (0)",
       _wiring({"package.json": '{ "scripts": { "tauri": "tauri" } }'}) == 0)
record("wiring: an empty-scripts manifest -> target-absent (0)",
       _wiring({"package.json": '{ "scripts": {} }'}) == 0)
record("wiring: every tool config present + every script wired -> clean (0)",
       _wiring(dict(_FULL_CFG, **{"package.json": _FULL_PKG})) == 0)
# ARM proof 1: a config present but its script NOT wired -> caught (half-wired)
record("wiring: prettier config present but no prettier script -> caught (half-wired)",
       _wiring({".prettierrc": "{}", "package.json": '{ "scripts": { "tauri": "tauri" } }'}) >= 1)
record("wiring: tsconfig present but no tsc script -> caught (half-wired)",
       _wiring({"tsconfig.json": _GOOD_TS, "package.json": '{ "scripts": { "tauri": "tauri" } }'}) >= 1)
# ARM proof 2: a script wired but its config absent -> caught (half-wired; ALSO the anti-gaming leg --
# a wired script whose config is deleted does NOT silently open the gate, unlike a single-trigger fix)
record("wiring: a vitest script wired but no vitest config -> caught (half-wired / anti-gaming)",
       _wiring({"package.json": '{ "scripts": { "test": "vitest run" } }'}) >= 1)
# stylelint coupling: eslint flat config present but no stylelint config -> caught
record("wiring: eslint config present but stylelint config absent -> caught (stylelint coupled to eslint)",
       _wiring({"eslint.config.js": _GOOD_ESLINT,
                "package.json": '{ "scripts": { "lint": "eslint ." } }'}) >= 1)
# stylelint coupling satisfied: eslint + stylelint both present -> no stylelint finding
record("wiring: eslint + stylelint configs both present (lint script wired) -> clean for the lint layer",
       _wiring({"eslint.config.js": _GOOD_ESLINT, ".stylelintrc.json": "{}",
                "package.json": '{ "scripts": { "lint": "eslint . && stylelint **/*.css" } }'}) == 0)
record("wiring: malformed package.json -> exit-2 signal",
       _wiring({"package.json": "{ not json"}) == 2)
record("manifest: no package.json -> target-absent (None)", in_root({}, m._frontend_manifest) is None)


# detector-coverage: freeze_contract catches a PKG_REQUIRED_SCRIPTS tool with no _TOOL_CONFIG detector
def _drift_detector():
    saved = m.PKG_REQUIRED_SCRIPTS
    m.PKG_REQUIRED_SCRIPTS = list(saved) + ["webpack"]
    try:
        return m.freeze_contract()
    finally:
        m.PKG_REQUIRED_SCRIPTS = saved


record("freeze_contract: a PKG_REQUIRED_SCRIPTS tool with no _TOOL_CONFIG detector is caught (drift)",
       _drift_detector() >= 1)


# the drift guard must SHORT-CIRCUIT main() before the live tier: with the drift present AND a manifest
# on disk, main() exits 1 cleanly via the early structural bail -- it does NOT reach (and KeyError-crash
# in) the assert_toolchain_wiring loop (G1 dual-review P1 fix; without the early bail this leg crashes).
def _drift_with_manifest():
    saved = m.PKG_REQUIRED_SCRIPTS
    m.PKG_REQUIRED_SCRIPTS = list(saved) + ["webpack"]
    try:
        return in_root({"package.json": '{ "scripts": {} }'}, lambda: m.main([]))
    finally:
        m.PKG_REQUIRED_SCRIPTS = saved


record("main: contract/detector drift WITH a manifest present -> clean exit 1, not a KeyError crash",
       _drift_with_manifest() == 1)

# --- a full synthetic frontend E2E (static asserts only; live runs skip if pnpm absent) -------------
_FULL = dict(_FULL_CFG, **{"package.json": _FULL_PKG})
record("E2E: a complete synthetic frontend passes every static assertion (tsconfig+eslint+wiring)",
       in_root(_FULL, lambda: m.assert_tsconfig_strict() + m.assert_eslint_rules()
               + m.assert_toolchain_wiring(m.ROOT / "package.json")) == 0)

# --- P0.4.7 G1 (volle Härte) P2/P3 fix legs ---------------------------------------------------
# P2: a formatter-wrapped MULTI-line array-form disable (the per-physical-line scan missed it) is caught
record("eslint: a multi-line `\"rule\":\\n[\"off\"]` disable -> caught (token-proximity, not per-line)",
       in_root({"eslint.config.js": 'export default [{ rules: {\n  "@typescript-eslint/no-explicit-any":\n    ["off"],\n  "no-restricted-syntax": ["error", { message: "fc.gen wrap" }],\n} }];'},
               m.assert_eslint_rules) >= 1)
# P2: an unrelated `: 0` for a DIFFERENT key on the same physical line does NOT cross-contaminate a valid rule
record("eslint: an unrelated `: 0` sharing a line with a valid rule does NOT false-flag it OFF",
       in_root({"eslint.config.js": 'export default [{ rules: { "@typescript-eslint/no-explicit-any": "error", "complexity": 0, "no-restricted-imports": ["error", { patterns: [{ group: ["@tauri-apps/api", "@tauri-apps/plugin-*"] }] }] }, settings: { note: "fc.gen" } }];'},
               m.assert_eslint_rules) == 0)
# P2: the TS5 extends-ARRAY form is followed
record("_tsconfig_options: the TS5 extends-ARRAY form is followed (no false-negative)",
       (lambda o: o.get("strict") is True and o.get("noImplicitOverride") is True)(
           in_root({"strict.json": '{ "compilerOptions": { "strict": true } }',
                    "tsconfig.json": '{ "extends": ["./strict.json"], "compilerOptions": { "noImplicitOverride": true } }'},
                   lambda: m._tsconfig_options(m.ROOT / "tsconfig.json")[0])))
# P3: a MULTI-level extends chain resolves a flag from the grandparent base (docstring now says multi-level)
record("_tsconfig_options: a multi-level extends chain resolves a grandparent-base flag",
       (lambda o: o.get("strict") is True and o.get("noImplicitOverride") is True and o.get("noUncheckedIndexedAccess") is True)(
           in_root({"grand.json": '{ "compilerOptions": { "strict": true } }',
                    "base.json": '{ "extends": "./grand.json", "compilerOptions": { "noImplicitOverride": true } }',
                    "tsconfig.json": '{ "extends": "./base.json", "compilerOptions": { "noUncheckedIndexedAccess": true } }'},
                   lambda: m._tsconfig_options(m.ROOT / "tsconfig.json")[0])))


def _gutted_eslint(rules):
    saved = m.ESLINT_REQUIRED_RULES
    m.ESLINT_REQUIRED_RULES = rules
    try:
        return m.freeze_contract()
    finally:
        m.ESLINT_REQUIRED_RULES = saved


# P3: dropping the fc.gen rule from the contract is caught (freeze_contract guards BOTH named rules)
record("freeze_contract: dropping the fc.gen ban from the contract is caught (symmetric guard)",
       _gutted_eslint([("only-any", "@typescript-eslint/no-explicit-any")]) >= 1)

# --- the T2 DOM/eval sink ban (security-concept §5 T2): the freeze, the armed canary, the real run ----------
_SINK_RULES = list(m.ESLINT_SINK_RULES)


def _sink_cfg(rules=None, inline="linterOptions: { noInlineConfig: true },", sev='"error"', extra=""):
    """A synthetic flat config declaring `rules` (default: every contract sink rule) at `sev`."""
    body = "".join(f'    "{r}": {sev},\n' for r in (_SINK_RULES if rules is None else rules))
    return f"export default [{{ {inline} rules: {{\n{body}}} }}{extra}];\n"


def _freeze(cfg_text):
    return in_root({"eslint.config.js": cfg_text}, m.assert_eslint_sink_ban)


record("T2 freeze: the REAL committed eslint.config.mjs declares each sink rule once at error with noInlineConfig",
       m.assert_eslint_sink_ban() == 0)
record("T2 freeze: every sink rule once at \"error\" + `noInlineConfig: true` -> clean", _freeze(_sink_cfg()) == 0)
record("T2 freeze: the options-array form [\"error\", {...}] and the numeric 2 both read as error -> clean",
       _freeze(_sink_cfg(sev='["error", { allowIndirect: false }]')) == 0 and _freeze(_sink_cfg(sev="2")) == 0)
record("T2 freeze: each contract sink rule dropped in turn -> caught (every rule of ESLINT_SINK_RULES)",
       all(_freeze(_sink_cfg([x for x in _SINK_RULES if x != r])) >= 1 for r in _SINK_RULES))
record("T2 freeze: a sink rule declared again in a files-scoped block (its options replaced there) -> caught",
       _freeze(_sink_cfg(extra=', { files: ["src/x/**"], rules: { "no-restricted-properties": '
                                '["error", { property: "x" }] } }')) >= 1)
record("T2 freeze: a sink rule at \"warn\", \"off\", 1, 0 or a variable severity -> caught (each form)",
       all(_freeze(_sink_cfg(sev=s)) >= 1 for s in ('"warn"', "'off'", "1", "0", "SEVERITY", '["warn"]')))
record("T2 freeze: \"no-implied-eval\" dropped while \"@typescript-eslint/no-implied-eval\" stays -> caught "
       "(quote-delimited count, no substring credit)",
       _freeze(_sink_cfg([x for x in _SINK_RULES if x != "no-implied-eval"])) >= 1)
record("T2 freeze: a sink rule named only in a // comment -> counted missing (comment-stripped scan)",
       _freeze(_sink_cfg([x for x in _SINK_RULES if x != "no-eval"]).replace(
           "rules: {\n", 'rules: {\n    // "no-eval": "error",\n')) >= 1)
record("T2 freeze: `noInlineConfig` absent, false, or declared twice -> caught (each form)",
       all(_freeze(c) >= 1 for c in (
           _sink_cfg(inline=""),
           _sink_cfg(inline="linterOptions: { noInlineConfig: false },"),
           _sink_cfg(extra=", { linterOptions: { noInlineConfig: true } }"))))


def _gutted_sink(rules=None, fixture=None):
    saved = (m.ESLINT_SINK_RULES, m.SINK_FIXTURE)
    m.ESLINT_SINK_RULES = saved[0] if rules is None else rules
    m.SINK_FIXTURE = saved[1] if fixture is None else fixture
    try:
        return m.freeze_contract()
    finally:
        m.ESLINT_SINK_RULES, m.SINK_FIXTURE = saved


record("freeze_contract: ESLINT_SINK_RULES without a rule-(i) sink rule (no-eval dropped) -> caught",
       _gutted_sink(rules=[x for x in _SINK_RULES if x != "no-eval"]) >= 1)
record("contract: _RULE_I_SINK_FLOOR is the five rules that ban the G29 rule-(i) sinks",
       m._RULE_I_SINK_FLOOR == {"no-restricted-properties", "no-restricted-syntax", "no-eval", "no-new-func",
                                "react/no-danger"})
record("freeze_contract: an empty SINK_FIXTURE -> caught", _gutted_sink(fixture="") >= 1)


def _gutted_probe_as():
    saved = m.SINK_PROBE_AS
    m.SINK_PROBE_AS = ""
    try:
        return m.freeze_contract()
    finally:
        m.SINK_PROBE_AS = saved


record("freeze_contract: an empty SINK_PROBE_AS -> caught", _gutted_probe_as() >= 1)

# the fixture's pin grammar and the COMMITTED fixture
record("parse_must_fire: a line-end marker pins scoped and comma-listed ids; a marker followed by code does not",
       m.parse_must_fire("a(); // eslint-must-fire: no-eval, @typescript-eslint/no-implied-eval\n"
                         "b(); // eslint-must-fire: no-eval; c();\nd();\n")
       == {1: {"no-eval", "@typescript-eslint/no-implied-eval"}})
_FIXTURE_TEXT = (m.ROOT / m.SINK_FIXTURE).read_text(encoding="utf-8")
_FIXTURE_PINS = m.parse_must_fire(_FIXTURE_TEXT)
record("the committed fixture pins every ESLINT_SINK_RULES rule on at least one line",
       set(_SINK_RULES) <= set().union(*_FIXTURE_PINS.values()))
_FIXTURE_LINES = _FIXTURE_TEXT.split("\n")
record("the committed fixture carries the inline-directive decoy: an eslint-disable-next-line no-eval right "
       "above a pinned no-eval line (it must still fire under noInlineConfig)",
       any("eslint-disable-next-line no-eval" in ln and "no-eval" in _FIXTURE_PINS.get(i + 2, set())
           for i, ln in enumerate(_FIXTURE_LINES)))


def _msgs(*triples, fatal_line=None):
    """A one-file eslint JSON result list from (line, ruleId, severity) triples."""
    msgs = [{"line": ln, "ruleId": rid, "severity": sev, "message": "m"} for ln, rid, sev in triples]
    if fatal_line is not None:
        msgs.append({"line": fatal_line, "ruleId": None, "severity": 2, "fatal": True, "message": "Parsing error"})
    return [{"filePath": str(m.ROOT / m.SINK_PROBE_AS), "messages": msgs}]


_ALL_FIRED = _msgs(*[(ln, rid, 2) for ln, ids in _FIXTURE_PINS.items() for rid in ids])
record("canary verdict: every pin of the committed fixture reported at severity 2 -> clean",
       m.sink_canary_problems(_ALL_FIRED, _FIXTURE_PINS) == [])
_ONE_PIN = {7: {"no-eval"}}
record("unfired_pins: the pinned rule silent on its line -> 'not reported'",
       m.unfired_pins([], _ONE_PIN) == [(7, "no-eval", "not reported")])
record("unfired_pins: the pinned rule only at severity 1 -> 'reported below error severity'",
       m.unfired_pins(m._messages(_msgs((7, "no-eval", 1))), _ONE_PIN)
       == [(7, "no-eval", "reported below error severity")])
record("unfired_pins: the pinned rule reported on ANOTHER line does not satisfy the pin",
       m.unfired_pins(m._messages(_msgs((8, "no-eval", 2))), _ONE_PIN) == [(7, "no-eval", "not reported")])
record("canary verdict: a fatal parse message (the fixture no longer lints as src/main.tsx) -> caught",
       any("did not lint" in p for p in m.sink_canary_problems(
           _msgs(*[(ln, rid, 2) for ln, ids in _FIXTURE_PINS.items() for rid in ids], fatal_line=1),
           _FIXTURE_PINS)))
record("canary verdict: a contract rule the pins never name -> caught (its ban unproven)",
       any("pins no line for the T2 sink rule 'react/no-danger-with-children'" in p
           for p in m.sink_canary_problems(_ALL_FIRED, {ln: ids - {"react/no-danger-with-children"}
                                                        for ln, ids in _FIXTURE_PINS.items()})))
record("canary verdict: an ignored-file result (one rule-less warning) -> every pin unfired",
       len(m.sink_canary_problems(_msgs((0, None, 1)), _FIXTURE_PINS))
       >= sum(len(ids) for ids in _FIXTURE_PINS.values()))

# the real run: src/ coverage and the verdict
record("unlinted_sources: a src/ .ts left out of the linted set, an unlinted .jsx and a nested-directory .tsx -> all "
       "three; .d.ts/.css never",
       in_root({"src/a.tsx": "", "src/b.ts": "", "src/c.jsx": "", "src/d.d.ts": "", "src/e.css": "",
                "src/n/m/f.tsx": "", "src/n/m/g.d.ts": ""},
               lambda: m.unlinted_sources({"src/a.tsx"})) == ["src/b.ts", "src/c.jsx", "src/n/m/f.tsx"])
record("unlinted_sources: no src/ -> none (target-absent)", in_root({}, lambda: m.unlinted_sources(set())) == [])
_EVERY_EXT = {f"src/x{ext}": "" for ext in (".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs")}
_NESTED = {f"src/n/m/x{ext}": "" for ext in (".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs")}
record("_src_sources: every JS-family extension counts, at the top of src/ and in a nested directory; a .d.ts, "
       ".d.mts, .d.cts or .css file never does",
       in_root({**_EVERY_EXT, **_NESTED, "src/y.d.ts": "", "src/y.d.mts": "", "src/y.d.cts": "", "src/y.css": "",
                "src/n/m/y.d.ts": "", "src/n/m/y.css": ""},
               m._src_sources) == sorted([*_EVERY_EXT, *_NESTED]))
record("_messages: a non-object message entry is skipped, not passed on",
       m._messages([{"messages": ["x", {"line": 1}]}, {"messages": None}]) == [{"line": 1}])
record("_repo_rel: an absolute eslint filePath under ROOT -> its repo-relative POSIX path",
       m._repo_rel(str(m.ROOT / "src" / "App.tsx")) == "src/App.tsx")


def _with_eslint(fake, fn, files=None):
    """Run fn() with m._eslint_json replaced by `fake` (and, given `files`, m.ROOT on a temp tree)."""
    saved = m._eslint_json
    m._eslint_json = fake
    try:
        return in_root(files, fn) if files is not None else fn()
    finally:
        m._eslint_json = saved


def _tree_results(root_rel_messages):
    return lambda args, stdin=None: ([{"filePath": str(m.ROOT / rel), "messages": msgs}
                                      for rel, msgs in root_rel_messages], "")


_TREE = {"src/a.tsx": "", "src/b.ts": "", "src/n/m/c.tsx": ""}
record("run_eslint_tree: every src/ file linted (a nested one too), no message -> 0",
       _with_eslint(_tree_results([("src/a.tsx", []), ("src/b.ts", []), ("src/n/m/c.tsx", [])]),
                    m.run_eslint_tree, _TREE) == 0)
record("run_eslint_tree: a WARNING (an inline directive under noInlineConfig) -> 1",
       _with_eslint(_tree_results([("src/a.tsx", [{"line": 1, "ruleId": None, "severity": 1, "message": "w"}]),
                                   ("src/b.ts", []), ("src/n/m/c.tsx", [])]), m.run_eslint_tree, _TREE) == 1)
record("run_eslint_tree: a src/ file eslint never reported (ignored) -> 1",
       _with_eslint(_tree_results([("src/a.tsx", []), ("src/n/m/c.tsx", [])]), m.run_eslint_tree, _TREE) == 1)
record("run_eslint_tree: a src/ file in a nested directory eslint never reported -> 1",
       _with_eslint(_tree_results([("src/a.tsx", []), ("src/b.ts", [])]), m.run_eslint_tree, _TREE) == 1)
record("run_eslint_tree: eslint produced no result list -> 1 (fail-closed)",
       _with_eslint(lambda args, stdin=None: (None, "eslint exited 2"), m.run_eslint_tree, _TREE) == 1)
_CALLS: list = []


def _recording(results):
    def fake(args, stdin=None):
        _CALLS.append((list(args), stdin))
        return results, ""
    return fake


_with_eslint(_recording([{"filePath": str(m.ROOT / "src/a.tsx"), "messages": []}]), m.run_eslint_tree, {"src/a.tsx": ""})
record("run_eslint_tree: eslint lints the whole tree (.), no stdin", _CALLS[-1] == (["."], None))
_with_eslint(_recording(_ALL_FIRED), m.run_sink_canary)
record("run_sink_canary: eslint reads the committed fixture text from stdin, as SINK_PROBE_AS",
       _CALLS[-1] == (["--stdin", "--stdin-filename", m.SINK_PROBE_AS], _FIXTURE_TEXT))
record("run_sink_canary: the committed fixture with every pin fired -> 0",
       _with_eslint(lambda args, stdin=None: (_ALL_FIRED, ""), m.run_sink_canary) == 0)
_PIN_PAIRS = sorted((ln, rid) for ln, ids in _FIXTURE_PINS.items() for rid in ids)
record("run_sink_canary: the committed fixture with one pin silent -> 1",
       _with_eslint(lambda args, stdin=None: (_msgs(*[(ln, rid, 2) for ln, rid in _PIN_PAIRS[1:]]), ""),
                    m.run_sink_canary) == 1)
record("run_sink_canary: eslint produced no result list -> 1 (fail-closed)",
       _with_eslint(lambda args, stdin=None: (None, "eslint exited 2"), m.run_sink_canary) == 1)
record("run_sink_canary: the planted fixture missing, or the path it is linted as missing -> 1 (fail-closed)",
       _with_eslint(lambda args, stdin=None: (_ALL_FIRED, ""), m.run_sink_canary, {}) == 1
       and _with_eslint(lambda args, stdin=None: (_ALL_FIRED, ""), m.run_sink_canary,
                        {m.SINK_FIXTURE: _FIXTURE_TEXT}) == 1)


# the scope leg: every src/ source file carries the evaluated sink rules, language, language options and settings
# of the canary's path
_REF_ENTRY = {"rules": {r: [2, {"pinned": r}] for r in _SINK_RULES}, "noInlineConfig": True, "processor": False,
              "context": {key: f"digest-of-{key}" for key in m._SINK_SCOPE_CONTEXT}}
_SCOPE_FILES = ["src/a.tsx", "src/b.ts"]


def _scope_cfgs(changes=None):
    """Synthetic per-file configs: the probe path and every _SCOPE_FILES entry carry _REF_ENTRY, then
    `changes` ({path: entry or a function editing a copy of _REF_ENTRY}) is applied."""
    cfgs = {f: copy.deepcopy(_REF_ENTRY) for f in [m.SINK_PROBE_AS, *_SCOPE_FILES]}
    for rel, change in (changes or {}).items():
        cfgs[rel] = change(copy.deepcopy(_REF_ENTRY)) if callable(change) else change
    return cfgs


_DROP = object()                                  # _set's marker: remove the key instead of setting it


def _edit_rule(rule, entry):
    def edit(cfg):
        cfg["rules"][rule] = entry
        return cfg
    return edit


def _set(key, value):
    def edit(cfg):
        if value is _DROP:
            cfg.pop(key)
        else:
            cfg[key] = value
        return cfg
    return edit


def _scope_catches(changes, needle):
    return any(needle in p for p in m.sink_scope_problems(_scope_cfgs(changes), _SCOPE_FILES))


record("sink_scope_problems: every src/ file carries the probe path's sink entries, noInlineConfig on, no "
       "processor -> clean", m.sink_scope_problems(_scope_cfgs(), _SCOPE_FILES) == [])
record("sink_scope_problems: a src/ file eslint ignores (no config) -> caught",
       _scope_catches({"src/b.ts": None}, "src/b.ts: eslint applies no config"))
record("sink_scope_problems: a sink rule absent from one file's config (a narrowed block) -> caught",
       _scope_catches({"src/b.ts": _edit_rule("no-eval", None)}, "src/b.ts: the T2 sink rules ['no-eval'] differ"))
record("sink_scope_problems: one file's sink rule with fewer banned names, or at warn -> caught (each form)",
       _scope_catches({"src/a.tsx": _edit_rule("no-restricted-properties", [2, {"property": "x"}])},
                      "src/a.tsx: the T2 sink rules ['no-restricted-properties'] differ")
       and _scope_catches({"src/a.tsx": _edit_rule("no-script-url", [1])},
                          "src/a.tsx: the T2 sink rules ['no-script-url'] differ"))
record("sink_scope_problems: a file entry without a rules object -> every sink rule differs -> caught",
       _scope_catches({"src/a.tsx": {"noInlineConfig": True, "processor": False}}, "src/a.tsx: the T2 sink rules"))
record("sink_scope_problems: noInlineConfig false or absent in one file -> caught (each form)",
       all(_scope_catches({"src/b.ts": _set("noInlineConfig", v)}, "src/b.ts: noInlineConfig is not on")
           for v in (False, None, _DROP)))
record("sink_scope_problems: a processor on one file, or the processor field absent -> caught (each form)",
       all(_scope_catches({"src/a.tsx": _set("processor", v)}, "src/a.tsx: a processor") for v in (True, _DROP)))
record("sink_scope_problems: no evaluated config for the probe path (absent or null) -> caught (fail-closed)",
       all(any("no evaluated config for" in p
               for p in m.sink_scope_problems({k: v for k, v in _scope_cfgs().items() if k != m.SINK_PROBE_AS}
                                              | extra, _SCOPE_FILES))
           for extra in ({}, {m.SINK_PROBE_AS: None})))
record("sink_scope_problems: the reference unarmed (a sink rule at warn in every file alike) -> caught",
       any(f"{m.SINK_PROBE_AS}: the T2 sink rule 'no-eval' is not at error severity" in p
           for p in m.sink_scope_problems(
               {f: _edit_rule("no-eval", [1, {"allowIndirect": False}])(copy.deepcopy(_REF_ENTRY))
                for f in [m.SINK_PROBE_AS, *_SCOPE_FILES]}, _SCOPE_FILES)))
record("sink_scope_problems: the probe path's own noInlineConfig off -> caught (the reference is checked too)",
       _scope_catches({m.SINK_PROBE_AS: _set("noInlineConfig", False)}, f"{m.SINK_PROBE_AS}: noInlineConfig"))


def _edit_context(key, value):
    def edit(cfg):
        if value is _DROP:
            cfg["context"].pop(key)
        else:
            cfg["context"][key] = value
        return cfg
    return edit


record("contract: _SINK_SCOPE_CONTEXT is the language, the settings and the five evaluated language options",
       set(m._SINK_SCOPE_CONTEXT) == {"language", "settings", "languageOptions.ecmaVersion",
                                      "languageOptions.sourceType", "languageOptions.parser",
                                      "languageOptions.parserOptions", "languageOptions.globals"})
record("sink_scope_problems: one file's evaluated globals differ (a scoped block turning the eval global off) -> "
       "caught", _scope_catches({"src/b.ts": _edit_context("languageOptions.globals", "digest-with-eval-off")},
                                "src/b.ts: its evaluated ['languageOptions.globals'] differ"))
record("sink_scope_problems: each context entry differing in one file -> caught (the language, the settings, each "
       "language option)",
       all(_scope_catches({"src/a.tsx": _edit_context(key, "other")}, f"src/a.tsx: its evaluated [{key!r}] differ")
           for key in m._SINK_SCOPE_CONTEXT))
record("sink_scope_problems: one file with an extra context entry, one entry missing, or no context at all -> "
       "caught (each form)",
       _scope_catches({"src/a.tsx": _edit_context("languageOptions.extra", "x")},
                      "src/a.tsx: its evaluated ['languageOptions.extra'] differ")
       and _scope_catches({"src/a.tsx": _edit_context("settings", _DROP)}, "src/a.tsx: its evaluated ['settings'] differ")
       and _scope_catches({"src/b.ts": _set("context", _DROP)}, "src/b.ts: its evaluated ['language',"))
record("sink_scope_problems: the reference without a context, or with a required entry absent or empty (in every "
       "file alike) -> caught (fail-closed, each form)",
       all(any(f"{m.SINK_PROBE_AS}: the probe reports no evaluated" in p
               for p in m.sink_scope_problems({f: edit(copy.deepcopy(_REF_ENTRY))
                                               for f in [m.SINK_PROBE_AS, *_SCOPE_FILES]}, _SCOPE_FILES))
           for edit in (_set("context", _DROP), _set("context", None),
                        _edit_context("languageOptions.globals", _DROP), _edit_context("language", ""))))


def _with_scope_probe(fake, files=None):
    """run_sink_scope with m._sink_scope_configs replaced by `fake` (m.ROOT on a temp tree of `files`)."""
    saved = m._sink_scope_configs
    m._sink_scope_configs = fake
    try:
        return in_root(files or {}, m.run_sink_scope)
    finally:
        m._sink_scope_configs = saved


_SEEN: list = []


def _probe_all_equal(files):
    _SEEN.append(list(files))
    return {f: copy.deepcopy(_REF_ENTRY) for f in files}, ""


_SCOPE_TREE = {"src/a.tsx": "", "src/b.ts": "", "src/c.d.ts": "", "src/d.css": "", "src/n/m/e.tsx": "",
               "src/n/m/f.d.ts": ""}
record("run_sink_scope: the probe gets the probe path plus every src/ source, a nested one too (.d.ts/.css never); "
       "all equal -> 0",
       _with_scope_probe(_probe_all_equal, _SCOPE_TREE) == 0
       and _SEEN[-1] == sorted({"src/a.tsx", "src/b.ts", "src/n/m/e.tsx", m.SINK_PROBE_AS}))
record("run_sink_scope: one src/ file with a differing sink rule -> 1",
       _with_scope_probe(lambda files: ({**{f: copy.deepcopy(_REF_ENTRY) for f in files},
                                         "src/b.ts": _edit_rule("no-new-func", None)(copy.deepcopy(_REF_ENTRY))}, ""),
                         _SCOPE_TREE) == 1)
record("run_sink_scope: a nested src/ file with differing evaluated globals -> 1",
       _with_scope_probe(lambda files: ({**{f: copy.deepcopy(_REF_ENTRY) for f in files},
                                         "src/n/m/e.tsx": _edit_context("languageOptions.globals", "other")(
                                             copy.deepcopy(_REF_ENTRY))}, ""),
                         _SCOPE_TREE) == 1)
record("run_sink_scope: the probe produced no mapping -> 1 (fail-closed)",
       _with_scope_probe(lambda files: (None, "node exited 1"), _SCOPE_TREE) == 1)


def _probe_run(stdout, returncode=0, have_pnpm=True):
    """_sink_scope_configs over ["src/a.tsx"] with subprocess.run and shutil.which stubbed; returns
    (result, captured argv, captured stdin)."""
    seen = {}

    def fake_run(argv, **kw):
        seen["argv"], seen["input"] = argv, kw.get("input")
        return subprocess.CompletedProcess(argv, returncode, stdout=stdout, stderr="")
    saved = (m.subprocess.run, m.shutil.which)
    m.subprocess.run = fake_run
    m.shutil.which = lambda tool: ("/bin/" + tool) if have_pnpm else None
    try:
        return m._sink_scope_configs(["src/a.tsx"]), seen.get("argv"), seen.get("input")
    finally:
        m.subprocess.run, m.shutil.which = saved


_GOOD_OUT = json.dumps({"src/a.tsx": _REF_ENTRY, "src/x.ts": None})
(_res, _why), _argv, _stdin = _probe_run(_GOOD_OUT)
record("_sink_scope_configs: the program goes in on stdin to `pnpm exec node --input-type=module -`, its "
       "@INPUT@ replaced by the files and the sink rules; a JSON object of objects and nulls is returned",
       _res == json.loads(_GOOD_OUT) and _why == "" and _argv[1:] == ["exec", "node", "--input-type=module", "-"]
       and "@INPUT@" not in _stdin
       and json.dumps({"files": ["src/a.tsx"], "rules": m.ESLINT_SINK_RULES}) in _stdin)
record("_sink_scope_configs: pnpm absent, a non-zero exit, non-JSON, a JSON list, or a non-object entry -> None "
       "(each form, fail-closed)",
       all(_probe_run(out, rc, have)[0][0] is None for out, rc, have in (
           (_GOOD_OUT, 0, False), (_GOOD_OUT, 1, True), ("Error: config failed", 0, True), ("[]", 0, True),
           (json.dumps({"src/a.tsx": [2]}), 0, True))))
record("_SINK_SCOPE_JS: the probe resolves each input file's own config and prints the fields the verdict "
       "reads (one @INPUT@ slot)",
       m._SINK_SCOPE_JS.count("@INPUT@") == 1
       and "for (const file of input.files)" in m._SINK_SCOPE_JS
       and "eslint.calculateConfigForFile(file)" in m._SINK_SCOPE_JS
       and all(f in m._SINK_SCOPE_JS for f in ("config.rules?.[rule]", "linterOptions?.noInlineConfig",
                                               "config.processor !== undefined", "JSON.stringify(out)")))
record("_SINK_SCOPE_JS: the context map carries the language and the parser by identity, the settings and every "
       "other evaluated language option by a digest of key-sorted JSON with each function by identity",
       all(f in m._SINK_SCOPE_JS for f in (
           "    context: Object.fromEntries([\n",
           '["language", identity(config.language)]', '["settings", digest(config.settings)]',
           "Object.entries(config.languageOptions ?? {})", "`languageOptions.${key}`",
           'key === "parser" ? identity(value) : digest(value)', 'typeof v === "function"', "? identity(v)",
           "Object.keys(v).sort()", 'createHash("sha256").update(canonical(value))',
           "if (!ids.has(value)) ids.set(value, `object#${ids.size}`);")))


def _live_calls():
    """run_live_tools with pnpm present and every runner stubbed: which G5 legs does it call?"""
    called = []
    saved = (m._have, m._run, m.run_sink_canary, m.run_sink_scope, m.run_eslint_tree)
    m._have = lambda tool: True
    m._run = lambda label, argv: 0
    m.run_sink_canary = lambda: called.append("canary") or 0
    m.run_sink_scope = lambda: called.append("scope") or 0
    m.run_eslint_tree = lambda: called.append("tree") or 0
    try:
        m.run_live_tools(False)
        return called
    finally:
        m._have, m._run, m.run_sink_canary, m.run_sink_scope, m.run_eslint_tree = saved


record("run_live_tools: the armed canary, then its scope over src/, then the real eslint run",
       _live_calls() == ["canary", "scope", "tree"])


def _main_with_sink_freeze(verdict):
    """main() over the clean synthetic frontend, the live tier stubbed green, the sink freeze stubbed."""
    saved = (m.assert_eslint_sink_ban, m.run_live_tools)
    m.assert_eslint_sink_ban = lambda: verdict
    m.run_live_tools = lambda full: 0
    try:
        return in_root(_FULL, lambda: m.main([]))
    finally:
        m.assert_eslint_sink_ban, m.run_live_tools = saved


record("main: a red T2 sink freeze fails the gate and a green one passes (assert_eslint_sink_ban is wired)",
       _main_with_sink_freeze(1) == 1 and _main_with_sink_freeze(0) == 0)

passed = sum(1 for _, ok in results if ok)
print(f"\n[g24-ts-gate-contract] {passed}/{len(results)} assertions passed.")
sys.exit(0 if passed == len(results) else 1)
