#!/usr/bin/env python3
"""g24-ts-gate-contract.py - G24 self-test for check-ts-gate (P0.4.7, G5/G6/G13).

Proves the structural freeze CANNOT be gutted (the strict / no-`any` / tool-wiring contract stays
non-empty) and that every LIVE-tier assertion CATCHES its planted violation once the P1 frontend lands:
a relaxed tsconfig strict flag (G6/G13), a missing / switched-off eslint rule (G5), an absent
prettier/stylelint/vitest config, a manifest not wiring a tool, plus the JSONC tolerance + the
single-level `extends` follow + the comment/string-aware scans. The live tsc/eslint/prettier/vitest are
target-absent today (no package.json) so the real gate skips. stdlib-only. Exit 0 = all held.
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

# --- assert_configs_present (G5 prettier/stylelint + vitest) ----------------------------------
_CFG_FILES = {".prettierrc": "{}", ".stylelintrc.json": "{}", "vitest.config.ts": "export default {}"}
record("configs: prettier + stylelint + vitest present -> clean",
       in_root(_CFG_FILES, m.assert_configs_present) == 0)
record("configs: missing prettier -> caught",
       in_root({".stylelintrc.json": "{}", "vitest.config.ts": "export default {}"}, m.assert_configs_present) >= 1)
record("configs: vitest via a `test:` block in vite.config.ts -> accepted",
       in_root({".prettierrc": "{}", ".stylelintrc.json": "{}",
                "vite.config.ts": "export default {\n  test: { environment: 'jsdom' }\n}"},
               m.assert_configs_present) == 0)

# --- assert_pkg_scripts ------------------------------------------------------------------------
_GOOD_PKG = ('{ "scripts": { "typecheck": "tsc --noEmit", "lint": "eslint . && stylelint **/*.css", '
             '"fmt": "prettier --check .", "test": "vitest run" } }')
record("pkg-scripts: a manifest wiring all tools -> clean",
       m.assert_pkg_scripts(tmp_file("package.json", _GOOD_PKG)) == 0)
record("pkg-scripts: a manifest NOT wiring eslint -> caught",
       m.assert_pkg_scripts(tmp_file("package.json",
                                     '{ "scripts": { "typecheck": "tsc --noEmit", "fmt": "prettier -c .", "test": "vitest" } }')) >= 1)
record("pkg-scripts: malformed package.json -> exit-2 signal",
       m.assert_pkg_scripts(tmp_file("package.json", "{ not json")) == 2)

# --- target-absent + a full synthetic frontend E2E (asserts only; live runs skip if pnpm absent) ---
record("manifest: no package.json -> target-absent (None)", in_root({}, m._frontend_manifest) is None)
_FULL = dict(_CFG_FILES)
_FULL.update({"package.json": _GOOD_PKG, "tsconfig.json": _GOOD_TS, "eslint.config.js": _GOOD_ESLINT})
record("E2E: a complete synthetic frontend passes every static assertion (tsconfig+eslint+configs+scripts)",
       in_root(_FULL, lambda: m.assert_tsconfig_strict() + m.assert_eslint_rules()
               + m.assert_configs_present() + m.assert_pkg_scripts(m.ROOT / "package.json")) == 0)

# --- P0.4.7 G1 (volle Härte) P2/P3 fix legs ---------------------------------------------------
# P2: a formatter-wrapped MULTI-line array-form disable (the per-physical-line scan missed it) is caught
record("eslint: a multi-line `\"rule\":\\n[\"off\"]` disable -> caught (token-proximity, not per-line)",
       in_root({"eslint.config.js": 'export default [{ rules: {\n  "@typescript-eslint/no-explicit-any":\n    ["off"],\n  "no-restricted-syntax": ["error", { message: "fc.gen wrap" }],\n} }];'},
               m.assert_eslint_rules) >= 1)
# P2: an unrelated `: 0` for a DIFFERENT key on the same physical line does NOT cross-contaminate a valid rule
record("eslint: an unrelated `: 0` sharing a line with a valid rule does NOT false-flag it OFF",
       in_root({"eslint.config.js": 'export default [{ rules: { "@typescript-eslint/no-explicit-any": "error", "complexity": 0 }, settings: { note: "fc.gen" } }];'},
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
