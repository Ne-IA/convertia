#!/usr/bin/env python3
"""g24-ts-gate-contract.py - G24 self-test for check-ts-gate (P0.4.7, G5/G6/G13).

Proves the structural freeze CANNOT be gutted (the strict / no-`any` / tool-wiring contract stays
non-empty) and that every LIVE-tier assertion CATCHES its planted violation once the P1 frontend lands:
a relaxed tsconfig strict flag (G6/G13), a missing / switched-off eslint rule (G5), and a HALF-WIRED
tool — a config without its package.json script, or a script without its config (assert_toolchain_wiring,
the per-tool config<->script agreement) — while a BARE manifest with neither is TOLERATED (the P1.2
staggered-landing posture, the regression this gate-fix locks in). Plus the JSONC tolerance + the
multi-level `extends` follow + the comment/string-aware scans. The live tsc/eslint/prettier/vitest are
LIVE since P1 (package.json + the toolchain exist) so the real gate runs them. stdlib-only. Exit 0 = all held.
"""
import importlib.machinery
import importlib.util
import sys
import tempfile
from pathlib import Path

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

passed = sum(1 for _, ok in results if ok)
print(f"\n[g24-ts-gate-contract] {passed}/{len(results)} assertions passed.")
sys.exit(0 if passed == len(results) else 1)
