#!/usr/bin/env python3
"""g24-js-supply-chain.py - G24 self-test for check-js-supply-chain (P0.3.8, G18c/G18d).

Proves the JS/WebView supply-chain posture guard: a foreign/unpinned registry, an enabled pre/post
script hook, unsafe-perm, or a relaxed frozen-lockfile in .npmrc is caught; a pnpm-lock.yaml resolution
URL from a non-allowed host is caught; the onlyBuiltDependencies allowlist count is read from both pnpm
manifest forms; the REAL committed .npmrc evaluates clean and main() is target-absent-OK. stdlib-only.
Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-js-supply-chain"
_loader = importlib.machinery.SourceFileLoader("cjs", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cjs", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def good() -> dict:
    return {"registry": "https://registry.npmjs.org/", "enable-pre-post-scripts": "false",
            "unsafe-perm": "false", "frozen-lockfile": "true"}


# --- .npmrc posture ---------------------------------------------------------------------------
record("good .npmrc -> no problems", m.evaluate_npmrc(good()) == [])
c = good(); c["registry"] = "https://evil.example.com/"
record("a foreign/unpinned registry -> caught", any("registry" in p for p in m.evaluate_npmrc(c)))
c = good(); del c["registry"]
record("a MISSING registry pin -> caught", any("registry" in p for p in m.evaluate_npmrc(c)))
c = good(); c["enable-pre-post-scripts"] = "true"
record("enable-pre-post-scripts=true -> caught", any("pre/post" in p for p in m.evaluate_npmrc(c)))
c = good(); c["unsafe-perm"] = "true"
record("unsafe-perm=true -> caught", any("unsafe-perm" in p for p in m.evaluate_npmrc(c)))
c = good(); c["frozen-lockfile"] = "false"
record("frozen-lockfile=false -> caught", any("frozen-lockfile" in p for p in m.evaluate_npmrc(c)))

# --- .npmrc parsing ---------------------------------------------------------------------------
parsed = m.parse_npmrc("# comment\n\nregistry=https://registry.npmjs.org/\n; semicolon comment\nunsafe-perm=false\n")
record("parse_npmrc ignores comments/blanks + lowercases keys",
       parsed.get("registry") == "https://registry.npmjs.org/" and parsed.get("unsafe-perm") == "false")

# --- pnpm-lock resolution-URL guard (G18c) ----------------------------------------------------
record("a lockfile URL from the allowed host -> clean",
       m.lockfile_url_problems("resolution: {tarball: https://registry.npmjs.org/foo/-/foo-1.0.0.tgz}") == [])
record("a lockfile URL from a FOREIGN host -> caught",
       any("non-allowed host" in p for p in
           m.lockfile_url_problems("resolution: {tarball: https://evil.example.com/foo.tgz}")))
record("integrity sha512 hashes (not URLs) are ignored",
       m.lockfile_url_problems("integrity: sha512-AAAA/BBBB==\n") == [])

# --- onlyBuiltDependencies count (G18d) -------------------------------------------------------
record("pnpm-workspace onlyBuiltDependencies: [] -> 0", m.workspace_onlybuilt_count("onlyBuiltDependencies: []\n") == 0)
record("pnpm-workspace onlyBuiltDependencies with 2 items -> 2",
       m.workspace_onlybuilt_count("packages:\n  - 'a'\nonlyBuiltDependencies:\n  - esbuild\n  - "
                                   "'@swc/core'\nother: x\n") == 2)
record("pnpm-workspace WITHOUT the key -> None (target-absent)",
       m.workspace_onlybuilt_count("packages:\n  - 'apps/*'\n") is None)
record("pnpm-workspace INLINE onlyBuiltDependencies: [a, b] -> 2 (a non-empty inline list is not missed)",
       m.workspace_onlybuilt_count("onlyBuiltDependencies: [esbuild, '@swc/core']\n") == 2)

# --- G1 review P1 fixes: scoped registry / resolution schemes / allowlist robustness ----------
c = good(); c["@evilscope:registry"] = "https://evil.example.com/"
record("a scoped @scope:registry override to a foreign origin -> caught (source-substitution)",
       any("scoped" in p for p in m.evaluate_npmrc(c)))
c = good(); c["@myscope:registry"] = "https://registry.npmjs.org/"
record("a scoped registry pointing at the ALLOWED origin -> NOT caught", m.evaluate_npmrc(c) == [])
record("a git+ssh foreign resolution -> caught (clone-and-build code-exec)",
       any("non-registry resolution scheme" in p for p in
           m.lockfile_url_problems("repo: git+ssh://git@evil.example.com/pkg.git")))
record("a protocol-relative //host resolution -> caught",
       m.lockfile_url_problems("tarball: //evil.example.com/x.tgz") != [])
record("a git+https from a FOREIGN host -> caught",
       m.lockfile_url_problems("repo: git+https://evil.example.com/pkg.git") != [])
record("a block allowlist with a # comment line mid-list -> counts correctly (not under-counted)",
       m._list_count_under("onlyBuiltDependencies:\n  - a\n  # note\n  - b\nother: x\n",
                           "onlyBuiltDependencies") == 2)
record("pnpm 11 allowBuilds list is counted",
       m._list_count_under("allowBuilds:\n  - esbuild\n", "allowBuilds") == 1)

# --- R2 fixes: pnpm-workspace registries: block + .pnpmfile.cjs (source-substitution / install code) -
record("pnpm-workspace registries.default off-origin -> caught",
       any("registries.default" in p for p in
           m.workspace_registries_problems("registries:\n  default: https://evil.example.com/\n")))
record("pnpm-workspace registries '@scope' off-origin -> caught",
       m.workspace_registries_problems('registries:\n  "@my-org": https://evil.example.com/\n') != [])
record("pnpm-workspace registries all pointing at the allowed origin -> clean",
       m.workspace_registries_problems('registries:\n  default: https://registry.npmjs.org/\n'
                                       '  "@my-org": https://registry.npmjs.org/\n') == [])
record("pnpm-workspace registries inline/anchor form -> fail-closed",
       m.workspace_registries_problems("registries: &r {default: https://x/}\n") != [])
record("no registries: block -> clean", m.workspace_registries_problems("packages:\n  - 'apps/*'\n") == [])

# --- R3 fixes: install-time source-mutation class + per-dir .npmrc -----------------------------
record("pnpm patchedDependencies (package.json) -> caught",
       m.install_mutation_problems({"pnpm": {"patchedDependencies": {"x@1": "patches/x.patch"}}}, "", False) != [])
record("a committed patches/ dir -> caught", m.install_mutation_problems({}, "", True) != [])
record("pnpm-workspace patchedDependencies -> caught",
       m.install_mutation_problems({}, "patchedDependencies:\n  x@1: patches/x.patch\n", False) != [])
record("a file:/link: source-redirect override -> caught",
       any("SOURCE-redirect" in p for p in
           m.install_mutation_problems({"pnpm": {"overrides": {"x": "file:../evil"}}}, "", False)))
record("a plain version override -> NOT caught (legit version pin)",
       m.install_mutation_problems({"pnpm": {"overrides": {"x": "^1.2.3"}}}, "", False) == [])
record("a root postinstall install-lifecycle script -> caught",
       any("postinstall" in p for p in m.install_mutation_problems({"scripts": {"postinstall": "node x"}}, "", False)))
record("a clean package.json (build script + version override) -> no problems",
       m.install_mutation_problems({"scripts": {"build": "vite build"}, "pnpm": {"overrides": {"x": "^1"}}}, "", False) == [])
# R4 fix: overrides/resolutions in pnpm-workspace.yaml (pnpm 11's primary location), + extra hooks
record("ws-yaml overrides: a file: source-redirect -> caught",
       any("redirects a dependency SOURCE" in p for p in
           m.install_mutation_problems({}, "overrides:\n  left-pad: file:../evil\n", False)))
record("ws-yaml overrides: a plain version pin -> NOT caught (block form not mis-read as inline)",
       m.install_mutation_problems({}, "overrides:\n  left-pad: ^1.2.3\n", False) == [])
record("ws-yaml resolutions: a git+ redirect -> caught",
       m.install_mutation_problems({}, "resolutions:\n  foo: git+https://evil.example.com/x.git\n", False) != [])
record("ws-yaml overrides: an inline {map} redirect -> caught",
       m.install_mutation_problems({}, "overrides: {left-pad: link:../evil}\n", False) != [])
record("ws-yaml overrides: an anchor form -> fail-closed",
       m.install_mutation_problems({}, "overrides: &o\n  x: ^1\n", False) != [])
record("a preprepare install-lifecycle hook -> caught",
       any("preprepare" in p for p in m.install_mutation_problems({"scripts": {"preprepare": "x"}}, "", False)))
# R5 fix: a DIRECT source-redirect dependency spec (the more direct door than overrides) + catalogs
record("a DIRECT file: dependency in package.json -> caught",
       any("SOURCE-redirect" in p for p in
           m.install_mutation_problems({"dependencies": {"evil": "file:../evil"}}, "", False)))
record("a DIRECT git+ devDependency -> caught",
       m.install_mutation_problems({"devDependencies": {"x": "git+https://evil.example.com/x.git"}}, "", False) != [])
record("a DIRECT link: optionalDependency -> caught",
       m.install_mutation_problems({"optionalDependencies": {"x": "link:../evil"}}, "", False) != [])
record("a plain-version direct dependency -> NOT caught",
       m.install_mutation_problems({"dependencies": {"react": "^18.2.0"}}, "", False) == [])
record("a workspace: monorepo dependency -> NOT caught (legit)",
       m.install_mutation_problems({"dependencies": {"x": "workspace:*"}}, "", False) == [])
record("ws-yaml catalog: a file: redirect -> caught",
       m.install_mutation_problems({}, "catalog:\n  react: file:../evil\n", False) != [])
record("ws-yaml catalogs: a NESTED file: redirect -> caught",
       m.install_mutation_problems({}, "catalogs:\n  r17:\n    react: file:../evil\n", False) != [])
record("ws-yaml catalog: a plain version -> NOT caught",
       m.install_mutation_problems({}, "catalog:\n  react: ^18.2.0\n", False) == [])
# R6 fix: configDependencies (registry-sourced install hooks/patches/allowlist) + onlyBuiltDependenciesFile + packageExtensions
record("pnpm configDependencies (package.json) -> caught",
       any("configDependencies" in p for p in
           m.install_mutation_problems({"pnpm": {"configDependencies": {"x": "1.0+sha512-y"}}}, "", False)))
record("ws-yaml configDependencies block -> caught",
       m.install_mutation_problems({}, "configDependencies:\n  x: 1.0+sha512-y\n", False) != [])
record("onlyBuiltDependenciesFile (a file-pointer allowlist) -> caught",
       any("onlyBuiltDependenciesFile" in p for p in
           m.install_mutation_problems({"pnpm": {"onlyBuiltDependenciesFile": "allow.json"}}, "", False)))
record("packageExtensions injecting a file: dependency -> caught",
       m.install_mutation_problems({"pnpm": {"packageExtensions": {"foo": {"dependencies": {"evil": "file:../e"}}}}}, "", False) != [])
record("packageExtensions with a plain-version injected dep -> NOT caught",
       m.install_mutation_problems({"pnpm": {"packageExtensions": {"foo": {"dependencies": {"bar": "^1"}}}}}, "", False) == [])
# R7 fix: .pnpmfile.mjs (pnpm's default ESM name) + dangerouslyAllowAllBuilds
record(".pnpmfile.mjs + pnpmfile.mjs are in the forbidden-pnpmfile set (the ESM default name)",
       all(any(p.name == n for p in m.PNPMFILE_CANDIDATES) for n in (".pnpmfile.mjs", "pnpmfile.mjs")))
record("dangerouslyAllowAllBuilds (package.json) -> caught",
       any("dangerouslyAllowAllBuilds" in p for p in
           m.install_mutation_problems({"pnpm": {"dangerouslyAllowAllBuilds": True}}, "", False)))
record("dangerouslyAllowAllBuilds (ws-yaml true) -> caught",
       m.install_mutation_problems({}, "dangerouslyAllowAllBuilds: true\n", False) != [])
record("dangerouslyAllowAllBuilds absent/false -> NOT caught",
       m.install_mutation_problems({"pnpm": {"dangerouslyAllowAllBuilds": False}}, "", False) == [])
record("dangerouslyAllowAllBuilds: every JS-truthy scalar pnpm honors -> caught (fail-closed)",
       all(m.install_mutation_problems({}, f"dangerouslyAllowAllBuilds: {v}\n", False) != [] for v in
           ("true", "True", "TRUE", "yes", "Yes", "on", "y", "enabled", "1", '"true"', "!!bool true")))
record("dangerouslyAllowAllBuilds: a falsy literal -> NOT caught (clean)",
       all(m.install_mutation_problems({}, f"dangerouslyAllowAllBuilds: {v}\n", False) == [] for v in
           ("false", "False", "FALSE", "no", "off", "0", "null", "~", "false  # default")))
record("dangerouslyAllowAllBuilds: a QUOTED-falsy (JS-truthy string pnpm honors) -> caught (R8 boundary lock)",
       all(m.install_mutation_problems({}, f"dangerouslyAllowAllBuilds: {v}\n", False) != [] for v in
           ("'false'", '"false"', "'0'", "'no'")))
record("dangerouslyAllowAllBuildsExtra (a longer key) -> NOT a false-positive (^-anchor + literal colon)",
       m.install_mutation_problems({}, "dangerouslyAllowAllBuildsExtra: true\n", False) == [])
with tempfile.TemporaryDirectory() as _td2:
    b = Path(_td2)
    (b / ".npmrc").write_text("registry=https://registry.npmjs.org/\n", encoding="utf-8")
    record("nonroot_npmrc: a repo with ONLY a root .npmrc -> none flagged", m.nonroot_npmrc(b) == [])
    (b / "src").mkdir()
    (b / "src" / ".npmrc").write_text("registry=https://evil.example.com/\n", encoding="utf-8")
    record("nonroot_npmrc: a committed subdir .npmrc is discovered (per-dir override surface)",
           len(m.nonroot_npmrc(b)) == 1)

# --- main() integration over a temp workspace (the lockfile/manifest path the suite missed) ----
with tempfile.TemporaryDirectory() as _td:
    base = Path(_td)
    (base / ".npmrc").write_text("registry=https://registry.npmjs.org/\nenable-pre-post-scripts=false\n"
                                 "unsafe-perm=false\nfrozen-lockfile=true\n", encoding="utf-8")
    _orig = (m.NPMRC, m.PNPM_LOCK, m.PNPM_WORKSPACE, m.PACKAGE_JSON, m.PNPMFILE_CANDIDATES, m.PINNED_FLOORS_JS)
    m.NPMRC, m.PNPM_LOCK = base / ".npmrc", base / "pnpm-lock.yaml"
    m.PNPM_WORKSPACE, m.PACKAGE_JSON = base / "pnpm-workspace.yaml", base / "package.json"
    m.PNPMFILE_CANDIDATES = (base / ".pnpmfile.cjs", base / "pnpmfile.cjs")
    m.PINNED_FLOORS_JS = {}    # isolate the §0.8 floor from the synthetic-lock URL-guard integration legs
    try:
        (base / "package.json").write_text('{"name":"x"}', encoding="utf-8")     # manifest, NO lock
        rc_nolock = m.main()
        (base / "pnpm-lock.yaml").write_text("resolution: {tarball: https://evil.example.com/x.tgz}\n",
                                             encoding="utf-8")
        rc_foreign = m.main()
        (base / "pnpm-lock.yaml").write_text("resolution: {tarball: https://registry.npmjs.org/x.tgz}\n",
                                             encoding="utf-8")
        rc_clean = m.main()
        (base / ".pnpmfile.cjs").write_text("module.exports = {}\n", encoding="utf-8")
        rc_pnpmfile = m.main()
        (base / ".pnpmfile.cjs").unlink()
    finally:
        (m.NPMRC, m.PNPM_LOCK, m.PNPM_WORKSPACE, m.PACKAGE_JSON, m.PNPMFILE_CANDIDATES,
         m.PINNED_FLOORS_JS) = _orig
    record("main(): a pnpm manifest WITHOUT a lockfile -> FAIL (not a silent skip)", rc_nolock == 1)
    record("main(): a lockfile with a FOREIGN resolution URL -> FAIL", rc_foreign == 1)
    record("main(): a lockfile with only allowed-registry resolutions -> pass", rc_clean == 0)
    record("main(): a committed .pnpmfile.cjs -> FAIL (no install-time code in a zero-egress product)",
           rc_pnpmfile == 1)

# --- the REAL committed .npmrc + main() -------------------------------------------------------
record("the REAL committed .npmrc evaluates clean",
       m.evaluate_npmrc(m.parse_npmrc(m.NPMRC.read_text(encoding="utf-8"))) == [])
record("main() exits 0 (.npmrc posture OK; lockfile resolution-URL + onlyBuilt + §0.8 floor live over the real lock)",
       m.main() == 0)

# --- §0.8 JS pinned-floor + its semver comparator (P1.60; mirrors g24-supply-chain) -----------
record("_version_ge: equal -> True", m._version_ge("2.11.3", "2.11.3") is True)
record("_version_ge: higher patch -> True", m._version_ge("2.11.4", "2.11.3") is True)
record("_version_ge: lower patch -> False", m._version_ge("2.11.2", "2.11.3") is False)
record("_version_ge: higher major -> True", m._version_ge("3.0.0", "2.11.3") is True)
record("_version_ge: a release outranks a pre-release floor -> True", m._version_ge("1.0.0", "1.0.0-rc.1") is True)
record("_version_ge: unparseable -> None (fail-closed)", m._version_ge("latest", "2.11.3") is None)
record("_direct_dep_versions: strips the pnpm v9 peer-context parens",
       m._direct_dep_versions("importers:\n\n  .:\n    dependencies:\n      zustand:\n        specifier: 5.0.14\n"
                              "        version: 5.0.14(react@19.2.7)\n", {"zustand"}) == {"zustand": ["5.0.14"]})
record("_pinned_floor_assertion(): the REAL pnpm-lock.yaml satisfies every §0.8 JS floor",
       m._pinned_floor_assertion() == [])


def _floor_with_temp_lock(body: str) -> list:
    saved = (m.PNPM_LOCK, m.PINNED_FLOORS_JS)
    with tempfile.TemporaryDirectory() as td:
        lock = Path(td) / "pnpm-lock.yaml"
        lock.write_text(body, encoding="utf-8")
        m.PNPM_LOCK = lock
        m.PINNED_FLOORS_JS = {"@tauri-apps/cli": "2.11.3", "zustand": "5.0.14"}
        try:
            return m._pinned_floor_assertion()
        finally:
            m.PNPM_LOCK, m.PINNED_FLOORS_JS = saved


_imp = ("importers:\n\n  .:\n    dependencies:\n      zustand:\n        specifier: 5.0.14\n        version: {z}\n"
        "    devDependencies:\n      '@tauri-apps/cli':\n        specifier: ^2.11.3\n        version: {c}\n")
record("_pinned_floor_assertion(): both JS floor crates AT floor -> clean",
       _floor_with_temp_lock(_imp.format(z="5.0.14", c="2.11.3")) == [])
record("_pinned_floor_assertion(): zustand BELOW floor (4.0.0 < 5.0.14) -> caught",
       any("zustand" in p and "below the relied-upon API floor" in p
           for p in _floor_with_temp_lock(_imp.format(z="4.0.0", c="2.11.3"))))
record("_pinned_floor_assertion(): a JS floor crate ABSENT from importers -> caught (relied-upon dep vanished)",
       any("@tauri-apps/cli" in p and "not a direct dep" in p
           for p in _floor_with_temp_lock("importers:\n\n  .:\n    dependencies:\n      zustand:\n"
                                          "        specifier: 5.0.14\n        version: 5.0.14\n")))
record("_pinned_floor_assertion(): a malformed resolved version (2.0) -> fail-closed (unparseable)",
       any("unparseable" in p for p in _floor_with_temp_lock(_imp.format(z="2.0", c="2.11.3"))))

failed = [n for n, ok in results if not ok]
print(f"\n[g24-js-supply-chain] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
