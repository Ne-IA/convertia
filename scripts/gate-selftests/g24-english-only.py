#!/usr/bin/env python3
"""g24-english-only.py - G24 self-test for check-english-only (P0.4.6, G57).

Proves the structural freeze cannot be gutted and every LIVE-tier leg CATCHES its planted violation once
the P1 frontend lands: a banned i18n dependency (package.json) / import (src) / missing eslint rule (a),
an empty / missing / drifted strings/ui.ts value (b), and that the eslint react/jsx-no-literals rule is
required (c). The live legs are target-absent today (no package.json/src/eslint/ui.ts) so the real gate
skips. stdlib-only. Exit 0 = all held.
"""
import importlib.machinery
import importlib.util
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-english-only"
_loader = importlib.machinery.SourceFileLoader("ceo", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("ceo", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def with_root(files: dict, fn):
    """Write {relpath: content} into a temp dir, repoint m.ROOT/PKG/SRC/STRINGS_MODULE at it, call fn(),
    restore. (PKG/SRC/STRINGS_MODULE are module-level Paths fixed at import, so each must be repointed.)"""
    saved = (m.ROOT, m.PKG, m.SRC, m.STRINGS_MODULE)
    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        for rel, content in files.items():
            p = root / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(content, encoding="utf-8")
        m.ROOT, m.PKG, m.SRC, m.STRINGS_MODULE = root, root / "package.json", root / "src", root / "src" / "strings" / "ui.ts"
        try:
            return fn()
        finally:
            m.ROOT, m.PKG, m.SRC, m.STRINGS_MODULE = saved


# --- the frozen CONTRACT --------------------------------------------------------------------------
record("freeze_contract: the real committed contract is clean", m.freeze_contract() == 0)
record("_banned: i18next / @lingui/core / i18next-http-backend (prefix) banned; react NOT",
       m._banned("i18next") and m._banned("@lingui/core") and m._banned("i18next-http-backend") and not m._banned("react"))


def _gutted_banned():
    se, sp = m.BANNED_I18N_EXACT, m.BANNED_I18N_PREFIX
    m.BANNED_I18N_EXACT, m.BANNED_I18N_PREFIX = set(), ()
    try:
        return m.freeze_contract()
    finally:
        m.BANNED_I18N_EXACT, m.BANNED_I18N_PREFIX = se, sp


record("freeze_contract: a gutted banned-i18n set is caught", _gutted_banned() >= 1)

# --- (a) package.json dependency scan -------------------------------------------------------------
record("pkg-deps: a clean package.json (no i18n dep) -> clean",
       with_root({"package.json": '{ "dependencies": { "react": "19.0.0" } }'}, m.scan_pkg_deps) == 0)
record("pkg-deps: i18next in dependencies -> caught",
       with_root({"package.json": '{ "dependencies": { "i18next": "23.0.0" } }'}, m.scan_pkg_deps) >= 1)
record("pkg-deps: @lingui/react in devDependencies -> caught",
       with_root({"package.json": '{ "devDependencies": { "@lingui/react": "4.0.0" } }'}, m.scan_pkg_deps) >= 1)
record("pkg-deps: malformed package.json -> exit-2 signal",
       with_root({"package.json": "{ not json"}, m.scan_pkg_deps) == 2)
record("pkg-deps: absent package.json -> target-absent skip", with_root({}, m.scan_pkg_deps) == 0)

# --- (a) src import grep --------------------------------------------------------------------------
record("src-imports: a clean component (no i18n import) -> clean",
       with_root({"src/App.tsx": 'import React from "react";\nexport const App = () => null;\n'}, m.scan_src_imports) == 0)
record("src-imports: `import i18next from \"i18next\"` -> caught",
       with_root({"src/i.ts": 'import i18next from "i18next";\n'}, m.scan_src_imports) >= 1)
record("src-imports: a COMMENTED-OUT i18n import -> NOT caught (comment-stripped)",
       with_root({"src/i.ts": '// import i18next from "i18next";\nexport const x = 1;\n'}, m.scan_src_imports) == 0)
record("src-imports: `from \"@formatjs/intl\"` (scoped prefix) -> caught",
       with_root({"src/i.ts": 'import { x } from "@formatjs/intl";\n'}, m.scan_src_imports) >= 1)
record("src-imports: a deep import `from \"i18next/foo\"` -> caught (bare-name split)",
       with_root({"src/i.ts": 'import x from "i18next/foo";\n'}, m.scan_src_imports) >= 1)

# --- (a)/(c) eslint flat config rules -------------------------------------------------------------
_GOOD_ESLINT = ('export default [{ rules: {\n  "react/jsx-no-literals": "error",\n'
                '  "no-restricted-imports": ["error", { paths: ["i18next", "react-intl"] }],\n} }];\n')
record("eslint: jsx-no-literals + no-restricted-imports naming i18n -> clean",
       with_root({"eslint.config.js": _GOOD_ESLINT}, m.assert_eslint_rules) == 0)
record("eslint: missing react/jsx-no-literals -> caught",
       with_root({"eslint.config.js": 'export default [{ rules: { "no-restricted-imports": ["error", { paths: ["i18next"] }] } }];'},
                 m.assert_eslint_rules) >= 1)
record("eslint: no-restricted-imports present but naming NO i18n lib -> caught (empty restriction)",
       with_root({"eslint.config.js": 'export default [{ rules: { "react/jsx-no-literals": "error", "no-restricted-imports": ["error"] } }];'},
                 m.assert_eslint_rules) >= 1)
record("eslint: absent flat config -> target-absent skip", with_root({}, m.assert_eslint_rules) == 0)

# --- (b) strings/ui.ts non-empty + the §6.10 pinned-key drift check -------------------------------
_PINNED = m.PINNED_KEYS["idle_reassurance"]
_GOOD_UI = ('export const ui = {\n  idle_reassurance: "' + _PINNED + '",\n  open_folder: "Open folder",\n} as const;\n')
record("strings: all keys non-empty + idle_reassurance exact -> clean",
       with_root({"src/strings/ui.ts": _GOOD_UI}, m.assert_strings_values) == 0)
record("strings: an EMPTY value -> caught",
       with_root({"src/strings/ui.ts": 'export const ui = {\n  idle_reassurance: "' + _PINNED + '",\n  empty_one: "",\n};\n'},
                 m.assert_strings_values) >= 1)
record("strings: the spec-DECIDED idle_reassurance MISSING -> caught",
       with_root({"src/strings/ui.ts": 'export const ui = {\n  open_folder: "Open folder",\n};\n'},
                 m.assert_strings_values) >= 1)
record("strings: idle_reassurance text DRIFTED from the spec pin -> caught",
       with_root({"src/strings/ui.ts": 'export const ui = {\n  idle_reassurance: "Everything is local.",\n};\n'},
                 m.assert_strings_values) >= 1)
record("strings: absent module -> target-absent skip", with_root({}, m.assert_strings_values) == 0)

# --- target-absent + a full clean synthetic frontend E2E ------------------------------------------
record("target-absent: no targets at all -> every live leg skips (0)",
       with_root({}, lambda: m.scan_pkg_deps() + m.scan_src_imports() + m.assert_eslint_rules() + m.assert_strings_values()) == 0)
record("E2E: a complete clean English-only frontend passes every live leg",
       with_root({"package.json": '{ "dependencies": { "react": "19.0.0" } }',
                  "src/App.tsx": 'import React from "react";\nexport const App = () => null;\n',
                  "eslint.config.js": _GOOD_ESLINT,
                  "src/strings/ui.ts": _GOOD_UI},
                 lambda: m.scan_pkg_deps() + m.scan_src_imports() + m.assert_eslint_rules() + m.assert_strings_values()) == 0)

# --- P0.4.6 G1 (volle Härte) P1/P2 fix legs ---------------------------------------------------
# P1: an EMPTY value on a SECOND entry sharing a physical line (the line-anchor fail-open) is now caught
record("strings: an empty value on a SECOND-on-the-line entry -> caught (no line-anchor fail-open)",
       with_root({"src/strings/ui.ts": 'export const ui = { idle_reassurance: "' + _PINNED + '", open_folder: "" };\n'},
                 m.assert_strings_values) >= 1)
# P1: an empty entry on the OPENING-BRACE line is caught
record("strings: an empty entry on the `{`-line is caught",
       with_root({"src/strings/ui.ts": 'export const ui = { sneaky: "",\n  idle_reassurance: "' + _PINNED + '" };\n'},
                 m.assert_strings_values) >= 1)
# P1: a MINIFIED single-line ui.ts with an embedded empty value is caught
record("strings: a minified single-line ui.ts with an embedded empty value is caught",
       with_root({"src/strings/ui.ts": 'export const ui={idle_reassurance:"' + _PINNED + '",a:"",b:"x"};\n'},
                 m.assert_strings_values) >= 1)
# P2: a react/jsx-no-literals rule switched "off" is caught (the off-detection, not token-presence only)
record("eslint: react/jsx-no-literals switched \"off\" -> caught (off-detection)",
       with_root({"eslint.config.js": 'export default [{ rules: { "react/jsx-no-literals": "off", "no-restricted-imports": ["error", { paths: ["i18next"] }] } }];'},
                 m.assert_eslint_rules) >= 1)
# P2: a bare side-effect `import "i18next";` is caught
record("src-imports: a bare side-effect `import \"i18next\";` -> caught",
       with_root({"src/i.ts": 'import "i18next";\nexport const x = 1;\n'}, m.scan_src_imports) >= 1)

passed = sum(1 for _, ok in results if ok)
print(f"\n[g24-english-only] {passed}/{len(results)} assertions passed.")
sys.exit(0 if passed == len(results) else 1)
