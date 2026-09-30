#!/usr/bin/env python3
"""g24-js-supply-chain.py - G24 self-test for check-js-supply-chain (P0.3.8, G18c/G18d).

Proves the JS/WebView supply-chain posture guard: a foreign/unpinned registry, an enabled pre/post
script hook, unsafe-perm (in .npmrc, or re-set in pnpm-workspace.yaml), an .npmrc key the guard does not
evaluate (`userconfig` and its siblings load a further config file), or ANY committed frozen-lockfile setting
(.npmrc key or pnpm-workspace.yaml) is caught - the dead-Dependabot-watch incident replayed, and every
.npmrc / pnpm-workspace.yaml spelling measured through pnpm 10.13.1 read the way pnpm reads it; in
pnpm-workspace.yaml every refused setting is caught in any key form and a `${` or a source-redirect
prefix anywhere; a
pnpm-lock.yaml resolution URL from a non-allowed host is caught; the onlyBuiltDependencies allowlist
count is read from both pnpm manifest forms; the REAL committed .npmrc evaluates clean and main()
exits 0 over the real tree; the §0.8 floor legs read the floors as data (package.json
`convertia.pinned-floors`): a missing, unparseable or malformed block and a missing pnpm-lock.yaml
each fail closed. stdlib-only.
Exit 0 = all held; 1 = a self-test failed.
"""
import contextlib
import importlib.machinery
import importlib.util
import io
import sys
import tempfile
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-js-supply-chain"
_loader = importlib.machinery.SourceFileLoader("cjs", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cjs", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


# [Test-Change: P0.3.8 - old-obsolete+new-correct, build-gates G18a row] old: `frozen-lockfile=true` was the
# clean posture; new: the key is refused at any value, because it froze Dependabot's lockfile-only resolve
# (ERR_PNPM_OUTDATED_LOCKFILE in every npm update job) and CI's pin is the G18a `--frozen-lockfile` flag-scan.
def good() -> dict:
    return {"registry": "https://registry.npmjs.org/", "enable-pre-post-scripts": "false",
            "unsafe-perm": "false"}


# the committed .npmrc as it stood before the key's retirement - the incident text, embedded, never read from git
_INCIDENT_NPMRC = (
    "# .npmrc — JS/WebView supply-chain lockdown (G18c registry pin + G18d lifecycle-script posture).\n"
    "# Authored P0.3.8; the live pnpm-lock-URL guard + the onlyBuiltDependencies allowlist activate in P1\n"
    "# once the pnpm workspace + lockfile exist. The WebView is the ENTIRE T2 attack surface, so its supply\n"
    "# chain matches the Rust-side discipline (the JS analogue of deny.toml [sources] + [bans]).\n"
    "# L(-1) security-critical (scripts/l-neg1-files.toml, G71) — structurally guarded by\n"
    "# scripts/check-js-supply-chain.\n"
    "\n"
    "# G18c — pin the registry: every dependency resolves from the public npm registry, nowhere else\n"
    "# (dependency-confusion / source-substitution defence; check-js-supply-chain asserts every\n"
    "# pnpm-lock.yaml resolution URL is from this origin once the lockfile lands).\n"
    "registry=https://registry.npmjs.org/\n"
    "\n"
    "# G18d — install-lifecycle-script lockdown. pnpm 10 already blocks dependency build/lifecycle scripts\n"
    "# by default (only an explicit onlyBuiltDependencies allowlist may run them); these pin that posture so\n"
    "# a malicious dep cannot run arbitrary code via postinstall the moment `pnpm install` runs in CI (which\n"
    "# holds the signing secrets at release time).\n"
    "enable-pre-post-scripts=false\n"
    "unsafe-perm=false\n"
    "\n"
    "# G18a (JS analogue) — CI installs against the committed lockfile; a drifted lockfile fails rather than\n"
    "# silently resolving a different graph than the audited/SBOM'd one.\n"
    "frozen-lockfile=true\n"
)


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
# [Test-Change: P0.3.8 - old-obsolete+new-correct, build-gates G18a row] the `=false` leg keeps its assertion
# under the any-value refusal; the `=true` key, the camel key and the incident text are refused alongside it.
c = good(); c["frozen-lockfile"] = "false"
record("a committed frozen-lockfile=false -> caught (any value)",
       any("frozen-lockfile" in p for p in m.evaluate_npmrc(c)))


def _frozen_refusals(problems: list) -> list:
    return [p for p in problems if "is committed in .npmrc" in p]


c = good(); c["frozen-lockfile"] = "true"
record("a committed frozen-lockfile=true -> caught (the key that froze the Dependabot resolve)",
       [p.startswith("`frozen-lockfile=true`") for p in _frozen_refusals(m.evaluate_npmrc(c))] == [True])
record("a committed frozenLockfile=true (camel key; pnpm 10.13.1 ignores it, refused fail-closed) -> caught",
       [p.startswith("`frozenLockfile=true`") for p in
        _frozen_refusals(m.evaluate_npmrc({**good(), **m.parse_npmrc("frozenLockfile=true\n")}))] == [True])
_incident = m.evaluate_npmrc(m.parse_npmrc(_INCIDENT_NPMRC))
record("the incident replay: the pre-retirement .npmrc text -> caught, the one problem names frozen-lockfile",
       len(_incident) == 1 and _incident[0].startswith("`frozen-lockfile=true` is committed in .npmrc"))


# every spelling below was measured through pnpm 10.13.1 (`pnpm install --lockfile-only` froze, or the root
# project's `pre<script>` ran): the .npmrc reader must read each one the way pnpm does
_CLEAN_NPMRC = "registry=https://registry.npmjs.org/\nenable-pre-post-scripts=false\nunsafe-perm=false\n"


def _npmrc_problems(text: str) -> list:
    """What main() raises for an .npmrc text: the evaluated map plus the unmodelled spellings."""
    return m.evaluate_npmrc(m.parse_npmrc(text)) + m.npmrc_unmodelled_problems(text)


def _frozen_named(extra: str, value: str = "true") -> bool:
    """The clean .npmrc plus `extra` raises exactly one frozen-lockfile refusal, naming the key and the value
    pnpm reads."""
    return ([p.startswith(f"`frozen-lockfile={value}`") for p in _frozen_refusals(_npmrc_problems(_CLEAN_NPMRC + extra))]
            == [True])


def _unmodelled(extra: str, needle: str) -> bool:
    return any(needle in p for p in m.npmrc_unmodelled_problems(_CLEAN_NPMRC + extra))


record("the clean .npmrc fixture -> no problem at all (the spelling legs below start from zero)",
       _npmrc_problems(_CLEAN_NPMRC) == [])
record(".npmrc a bare `frozen-lockfile` line -> caught, read as `true`", _frozen_named("frozen-lockfile\n"))
record(".npmrc a double-quoted key \"frozen-lockfile\"=true -> caught", _frozen_named('"frozen-lockfile"=true\n'))
record(".npmrc a single-quoted key 'frozen-lockfile'=true -> caught", _frozen_named("'frozen-lockfile'=true\n"))
record(".npmrc an array key frozen-lockfile[]=true -> caught, and the array key is refused",
       _frozen_named("frozen-lockfile[]=true\n") and _unmodelled("frozen-lockfile[]=true\n", "ini array key"))
record(".npmrc a bare key with an inline comment `frozen-lockfile ;c` -> caught, read as `true`",
       _frozen_named("frozen-lockfile ;c\n"))
record(".npmrc a value with an inline comment `frozen-lockfile=true #c` -> caught, the comment dropped",
       _frozen_named("frozen-lockfile=true #c\n"))
record(".npmrc a JSON-escaped quoted key \"frozen\\u002dlockfile\"=true -> caught",
       _frozen_named('"frozen\\u002dlockfile"=true\n'))
record(".npmrc a BOM before the key (JavaScript's trim drops U+FEFF) -> caught",
       _frozen_named("\ufefffrozen-lockfile=true\n"))
record(".npmrc a BOM before a quoted key (trimmed before the quote test, as JavaScript does) -> caught",
       _frozen_named("\ufeff\"frozen-lockfile\"=true\n"))
record(".npmrc an empty value `frozen-lockfile=` -> caught (any value)", _frozen_named("frozen-lockfile=\n", ""))
record(".npmrc a `[frozen-lockfile]` section header -> refused (pnpm reads it as that key)",
       _unmodelled("[frozen-lockfile]\n", "`[section]`"))
record(".npmrc an env-fallback key ${UNSET-frozen-lockfile}=true -> refused (npm-conf substitutes keys)",
       _unmodelled("${UNSET-frozen-lockfile}=true\n", "holds `${`"))
record(".npmrc a key decoding to a JSON array '[\"unsafe-perm\"]'=true -> refused (pnpm reads `unsafe-perm`)",
       _unmodelled("'[\"unsafe-perm\"]'=true\n", "decodes to the JSON value"))
record(".npmrc a 3050-deep JSON array key around \"unsafe-perm\" (pnpm reads `unsafe-perm`, measured) -> refused",
       _unmodelled("'" + "[" * 3050 + '"unsafe-perm"' + "]" * 3050 + "'=true\n", "decodes to the JSON value"))
record(".npmrc a 200000-deep JSON array key (past every Python decoder: the RecursionError path) -> refused, no crash",
       _unmodelled("'" + "[" * 200000 + '"unsafe-perm"' + "]" * 200000 + "'=true\n", "decodes to the JSON value"))
record(".npmrc FROZEN-LOCKFILE=true and frozen_lockfile=true (pnpm 10.13.1 ignores both) -> caught fail-closed",
       all([p.startswith(f"`{k}=true`") for p in _frozen_refusals(_npmrc_problems(_CLEAN_NPMRC + f"{k}=true\n"))]
           == [True] for k in ("FROZEN-LOCKFILE", "frozen_lockfile")))
# [Test-Change: P0.3.8 - old-obsolete+new-correct, build-gates G18d row] old: the spelling stayed clean; new:
# the .npmrc may carry only the keys the gate evaluates (a key such as `userconfig` loads a further config
# file, measured), so this key reds as unlisted - and still never as the frozen setting, which pnpm does not
# read it as (measured: not frozen).
_BACKSLASH_KEY = _npmrc_problems(_CLEAN_NPMRC + "frozen\\-lockfile=true\n")
record(".npmrc `frozen\\-lockfile=true` (a backslash ini keeps, pnpm does not freeze) -> refused as an unlisted "
       "key, never as the frozen setting",
       _frozen_refusals(_BACKSLASH_KEY) == []
       and [p.startswith(".npmrc key `frozen\\-lockfile` is not one this guard evaluates") for p in _BACKSLASH_KEY]
       == [True])
record(".npmrc a `;frozen-lockfile=true` comment line -> NOT caught",
       _npmrc_problems(_CLEAN_NPMRC + ";frozen-lockfile=true\n") == [])
_EVIL = "registry=https://evil.example.com/\n"
_PINS = "enable-pre-post-scripts=false\nunsafe-perm=false\n"
for _label, _text in (("a case variant REGISTRY= (pnpm keys are case-sensitive)",
                       _EVIL + "REGISTRY=https://registry.npmjs.org/\n" + _PINS),
                      ("a `[x]` section moving the second registry out of the top level",
                       _PINS + _EVIL + "[x]\nregistry=https://registry.npmjs.org/\n"),
                      ("an array key registry[]= in front of the pin",
                       _PINS + "registry[]=https://evil.example.com/\nregistry=https://registry.npmjs.org/\n"),
                      ("a U+2028 in the second value (pnpm skips that line)",
                       _PINS + _EVIL + "registry=https://registry.npmjs.org/\u2028\n")):
    record(f".npmrc a foreign registry masked by {_label} -> caught", _npmrc_problems(_text) != [])
_epps = "`enable-pre-post-scripts`"
for _label, _text in (("a bare `enable-pre-post-scripts` line after the pin", _CLEAN_NPMRC + "enable-pre-post-scripts\n"),
                      ("`enable-pre-post-scripts=FALSE`", _CLEAN_NPMRC.replace("scripts=false", "scripts=FALSE")),
                      ("an empty `enable-pre-post-scripts=`", _CLEAN_NPMRC.replace("scripts=false", "scripts=")),
                      ("`enable-pre-post-scripts=yes`", _CLEAN_NPMRC.replace("scripts=false", "scripts=yes")),
                      ("an absent `enable-pre-post-scripts` (pnpm defaults it to true)",
                       _CLEAN_NPMRC.replace("enable-pre-post-scripts=false\n", ""))):
    record(f".npmrc {_label} -> caught (pnpm runs the pre/post hooks)",
           any(p.startswith(_epps) and "pre/post" in p for p in _npmrc_problems(_text)))
record(".npmrc an absent `unsafe-perm` (pnpm defaults it to true unless running as root) -> caught",
       any(p.startswith("`unsafe-perm` is absent") for p in
           _npmrc_problems(_CLEAN_NPMRC.replace("unsafe-perm=false\n", ""))))
record(".npmrc `enable-pre-post-scripts=\"false\"` and `=false ;c` (pnpm reads false) -> clean",
       all(_npmrc_problems(_CLEAN_NPMRC.replace("scripts=false", v)) == [] for v in
           ('scripts="false"', "scripts=false ;c")))


def _unlisted(extra: str, key: str) -> bool:
    """The clean .npmrc plus `extra` raises exactly one problem: the unlisted-key refusal naming `key`."""
    return ([p.startswith(f".npmrc key `{key}` is not one this guard evaluates")
             for p in _npmrc_problems(_CLEAN_NPMRC + extra)] == [True])


# each key below makes pnpm 10.13.1 load a further config file: with that file committed holding
# `frozen-lockfile=true`, `install --lockfile-only` froze (ERR_PNPM_NO_LOCKFILE) and `config get` read true;
# through `userconfig` / `globalconfig` a foreign `@s:registry` was read the same way (measured)
record(".npmrc `userconfig=./rc2` (pnpm loads ./rc2 as a config file) -> refused",
       _unlisted("userconfig=./rc2\n", "userconfig"))
record(".npmrc `globalconfig=./rc2` (pnpm loads ./rc2 as a config file) -> refused",
       _unlisted("globalconfig=./rc2\n", "globalconfig"))
record(".npmrc `prefix=./p` (pnpm loads ./p/etc/npmrc as a config file) -> refused",
       _unlisted("prefix=./p\n", "prefix"))
record(".npmrc `workspace-prefix=./w` (pnpm loads ./w/.npmrc as a config file) -> refused",
       _unlisted("workspace-prefix=./w\n", "workspace-prefix"))
record(".npmrc a quoted key `\"userconfig\"=./rc2` (pnpm loads ./rc2, measured) -> refused under the decoded name",
       _unlisted("\"userconfig\"=./rc2\n", "userconfig"))
record(".npmrc `pnpmfile=tools/h.cjs` (pnpm runs the hook file at that path, measured) -> refused",
       _unlisted("pnpmfile=tools/h.cjs\n", "pnpmfile"))
record(".npmrc `USERCONFIG=./rc2` and `frozen-lockfile-if-exists=true` (pnpm 10.13.1 reads neither, measured) "
       "-> refused fail-closed",
       _unlisted("USERCONFIG=./rc2\n", "USERCONFIG")
       and _unlisted("frozen-lockfile-if-exists=true\n", "frozen-lockfile-if-exists"))
record(".npmrc a scoped `@myscope:registry` on the allowed origin -> clean (a scoped key is the override check's)",
       _npmrc_problems(_CLEAN_NPMRC + "@myscope:registry=https://registry.npmjs.org/\n") == [])

_WS_REFUSAL = "carries a `frozenLockfile` token"
record("pnpm-workspace.yaml frozenLockfile: true (flush or indented) -> caught",
       all(any(_WS_REFUSAL in p for p in m.workspace_frozen_lockfile_problems(t)) for t in
           ("packages:\n  - '.'\nfrozenLockfile: true\n", "  frozenLockfile: true\n")))
record("pnpm-workspace.yaml frozenLockfile: false -> caught (any value)",
       any(_WS_REFUSAL in p for p in m.workspace_frozen_lockfile_problems("frozenLockfile: false\n")))
record("pnpm-workspace.yaml a quoted key (\"frozenLockfile\" / 'frozenLockfile') -> caught",
       all(any(_WS_REFUSAL in p for p in m.workspace_frozen_lockfile_problems(t)) for t in
           ('"frozenLockfile": true\n', "'frozenLockfile': true\n")))
record("pnpm-workspace.yaml the kebab key frozen-lockfile: and the snake key frozen_lockfile: -> caught fail-closed",
       all(any(_WS_REFUSAL in p for p in m.workspace_frozen_lockfile_problems(t)) for t in
           ("frozen-lockfile: true\n", "frozen_lockfile: true\n")))
record("pnpm-workspace.yaml frozenLockfileExtra: true (a longer key) -> NOT caught",
       m.workspace_frozen_lockfile_problems("frozenLockfileExtra: true\n") == [])
record("pnpm-workspace.yaml preferFrozenLockfile: true (a different setting) -> NOT caught",
       m.workspace_frozen_lockfile_problems("preferFrozenLockfile: true\n") == [])
_WS_PKGS = "packages:\n  - '.'\n"
for _label, _text in (("a flow mapping {packages: ['.'], frozenLockfile: true} and its JSON twin",
                       ("{packages: ['.'], frozenLockfile: true}\n", '{"packages": ["."], "frozenLockfile": true}\n')),
                      ("an explicit key `? frozenLockfile` / `: true`", (_WS_PKGS + "? frozenLockfile\n: true\n",)),
                      ("a hex escape \"frozen\\x4Cockfile\"", (_WS_PKGS + '"frozen\\x4Cockfile": true\n',)),
                      ("a unicode escape \"frozen\\u004Cockfile\"", (_WS_PKGS + '"frozen\\u004Cockfile": true\n',)),
                      ("an escaped line break inside the quoted key",
                       (_WS_PKGS + '? "frozen\\\n  Lockfile"\n: true\n',)),
                      ("a `#` line that is live YAML inside a multi-line quoted flow scalar",
                       ("{packages: ['.'], \"x\n#\", frozenLockfile: true}\n",)),
                      ("an anchored value used as an aliased key", (_WS_PKGS + "x: &k frozenLockfile\n? *k\n: true\n",)),
                      ("a merge key <<: {frozenLockfile: true}", (_WS_PKGS + "<<: {frozenLockfile: true}\n",)),
                      ("an env-fallback key ${UNSET-frozenLockfile}", (_WS_PKGS + "${UNSET-frozenLockfile}: true\n",)),
                      ("a comment naming the setting (nothing is excised)", (_WS_PKGS + "# frozenLockfile: true\n",)),
                      ("a plain value ending in a backslash on the line before (only the raw view keeps the key)",
                       (_WS_PKGS + "x: a\\\nfrozenLockfile: true\n",))):
    record(f"pnpm-workspace.yaml {_label} -> caught",
           all(any(_WS_REFUSAL in p for p in m.workspace_frozen_lockfile_problems(t)) for t in _text))
record("REPLAY (R2 review): pnpm-workspace.yaml frozenLockfileIfExists: true (froze the lockfile-only resolve once a "
       "lockfile exists, measured) -> caught",
       any(_WS_REFUSAL in p for p in m.workspace_frozen_lockfile_problems(_WS_PKGS + "frozenLockfileIfExists: true\n")))
record("pnpm-workspace.yaml frozenLockfileIfExists as a hex escape (froze, measured), kebab, snake or case variant "
       "-> caught",
       all(any(_WS_REFUSAL in p for p in m.workspace_frozen_lockfile_problems(_WS_PKGS + t)) for t in
           ('"frozenLockfile\\x49fExists": true\n', "frozen-lockfile-if-exists: true\n",
            "frozen_lockfile_if_exists: true\n", "FrozenLockfileIfExists: true\n")))
record("pnpm-workspace.yaml frozenLockfileIfExistsExtra: true (a longer key) -> NOT caught",
       m.workspace_frozen_lockfile_problems(_WS_PKGS + "frozenLockfileIfExistsExtra: true\n") == [])
_WS_LIFECYCLE = "carries an `enablePrePostScripts` / `unsafePerm` token"
record("pnpm-workspace.yaml enablePrePostScripts: true (ran the root `pre<script>` over the .npmrc pin, measured) "
       "-> caught",
       any(_WS_LIFECYCLE in p for p in m.workspace_lifecycle_problems(_WS_PKGS + "enablePrePostScripts: true\n")))
record("pnpm-workspace.yaml unsafePerm: true (`config get unsafe-perm` read true over the .npmrc pin, measured) "
       "-> caught",
       any(_WS_LIFECYCLE in p for p in m.workspace_lifecycle_problems(_WS_PKGS + "unsafePerm: true\n")))
record("pnpm-workspace.yaml a lifecycle token at `false`, kebab, snake, flow, hex-escaped or in a comment -> caught "
       "(any value, nothing excised)",
       all(any(_WS_LIFECYCLE in p for p in m.workspace_lifecycle_problems(t)) for t in
           (_WS_PKGS + "enablePrePostScripts: false\n", _WS_PKGS + "enable-pre-post-scripts: true\n",
            _WS_PKGS + "unsafe_perm: true\n", "{packages: ['.'], unsafePerm: true}\n",
            _WS_PKGS + '"enable\\x50rePostScripts": true\n', _WS_PKGS + "# unsafePerm: true\n")))
record("pnpm-workspace.yaml a longer key either side (unsafePermExtra, xUnsafePerm) and the plain packages list "
       "-> NOT caught",
       all(m.workspace_lifecycle_problems(_WS_PKGS + t) == [] for t in
           ("unsafePermExtra: true\n", "xUnsafePerm: true\n", "")))

# pnpm 10.13.1 substitutes the environment into every top-level pnpm-workspace.yaml key and string value, so a key
# composed across `${` carries no token: each form below took effect through pnpm (measured) while every token
# search stayed clean - one leg per composed form, each also proving the token search alone misses it
_WS_ENV = "carries `${`"
_WS_REGISTRIES = "carries a `registries` token"
for _label, _text, _token_fn in (
        ("${UNSET-frozen}Lockfile: true (froze the lockfile-only resolve)", "${UNSET-frozen}Lockfile: true\n",
         m.workspace_frozen_lockfile_problems),
        ("frozen${UNSET-Lockfile}: true (froze)", "frozen${UNSET-Lockfile}: true\n", m.workspace_frozen_lockfile_problems),
        ("${UNSET-frozen}LockfileIfExists: true (froze a seeded lock)", "${UNSET-frozen}LockfileIfExists: true\n",
         m.workspace_frozen_lockfile_problems),
        ("${UNSET-enable}PrePostScripts: true (ran the root `pre<script>` over the .npmrc pin)",
         "${UNSET-enable}PrePostScripts: true\n", m.workspace_lifecycle_problems),
        ("${UNSET-unsafe}Perm: true (`config get unsafe-perm` read true)", "${UNSET-unsafe}Perm: true\n",
         m.workspace_lifecycle_problems),
        ("${UNSET-dangerously}AllowAllBuilds: true (ran a dependency's postinstall)",
         "${UNSET-dangerously}AllowAllBuilds: true\n", lambda t: m.install_mutation_problems({}, t, False)),
        ("${UNSET-regis}tries: {default: <foreign>} (re-pointed resolution)",
         "${UNSET-regis}tries: {default: 'https://evil.invalid/'}\n", m.workspace_registries_problems),
        ("\"\\x24{UNSET-frozen}Lockfile\": true (froze; only the escape-decoded view shows the `${`)",
         '"\\x24{UNSET-frozen}Lockfile": true\n', m.workspace_frozen_lockfile_problems)):
    record(f"pnpm-workspace.yaml {_label} -> caught by the `${{` refusal (its token search alone misses it)",
           any(_WS_ENV in p for p in m.workspace_env_problems(_WS_PKGS + _text))
           and _token_fn(_WS_PKGS + _text) == [])
record("pnpm-workspace.yaml a `${` in a string value or a comment -> caught (fail-closed, nothing excised)",
       all(any(_WS_ENV in p for p in m.workspace_env_problems(_WS_PKGS + t)) for t in
           ("x: '${HOME}'\n", "# ${UNSET-x}\n")))
record("pnpm-workspace.yaml a `$` not followed by `{` ($HOME, `$ {x}`) and the plain packages list -> NOT caught",
       all(m.workspace_env_problems(_WS_PKGS + t) == [] for t in ("x: '$HOME'\n", "x: '$ {y}'\n", "")))


# every other pnpm-workspace.yaml setting the guard refuses outright: pnpm 10.13.1 read each from the block,
# quoted, flow and explicit key forms alike (`pnpm config get`, measured), so each is a token at any value
def _ws_key_forms(key: str, val: str) -> tuple[str, ...]:
    return (_WS_PKGS + f"{key}: {val}\n", _WS_PKGS + f'"{key}": {val}\n', f"{{packages: ['.'], {key}: {val}}}\n",
            _WS_PKGS + f"? {key}\n: {val}\n")


for _key, _kebab, _val, _needle in (
        ("configDependencies", "config-dependencies", "{x: '1.0.0+sha512-AAAA'}", "`configDependencies` exists"),
        ("onlyBuiltDependenciesFile", "only-built-dependencies-file", "allow.json",
         "`onlyBuiltDependenciesFile` points"),
        ("patchedDependencies", "patched-dependencies", "{x@1.0.0: patches/x.patch}",
         "`patchedDependencies` / `patches/`"),
        ("dangerouslyAllowAllBuilds", "dangerously-allow-all-builds", "true", "`dangerouslyAllowAllBuilds` is set")):
    record(f"pnpm-workspace.yaml {_key} in the block, quoted, flow and explicit key forms (pnpm read each, measured) "
           "-> caught",
           all(any(_needle in p for p in m.install_mutation_problems({}, t, False)) for t in _ws_key_forms(_key, _val)))
    _hex = _key[:4] + f"\\x{ord(_key[4]):02X}" + _key[5:]
    record(f"pnpm-workspace.yaml {_key} as a hex-escaped key, a kebab or snake key or in a comment -> caught (raw or "
           "decoded, fail-closed, nothing excised)",
           all(any(_needle in p for p in m.install_mutation_problems({}, _WS_PKGS + t, False)) for t in
               (f'"{_hex}": {_val}\n', f"{_kebab}: {_val}\n", _kebab.replace("-", "_") + f": {_val}\n",
                f"# {_key}: {_val}\n")))
record("REPLAY (R3 review): pnpm-workspace.yaml dangerouslyAllowAllBuilds as an explicit key or a hex-escaped key "
       "(each ran a dependency's postinstall, measured) -> caught",
       all(any("`dangerouslyAllowAllBuilds` is set" in p for p in m.install_mutation_problems({}, t, False)) for t in
           (_WS_PKGS + "? dangerouslyAllowAllBuilds\n: true\n", _WS_PKGS + '"dangerously\\x41llowAllBuilds": true\n')))
record("pnpm-workspace.yaml dangerouslyAllowAllBuilds: false plus a more-indented `on` line (the truthy plain scalar "
       "`false on`; pnpm ran a dependency's postinstall, measured) -> caught",
       any("`dangerouslyAllowAllBuilds` is set" in p
           for p in m.install_mutation_problems({}, _WS_PKGS + "dangerouslyAllowAllBuilds: false\n  on\n", False)))
record("pnpm-workspace.yaml registries in the flow, quoted and explicit key forms (the first two re-pointed "
       "resolution over the .npmrc pin, measured) -> caught",
       all(any(_WS_REGISTRIES in p for p in m.workspace_registries_problems(t)) for t in
           _ws_key_forms("registries", "{default: 'https://evil.invalid/'}")[1:]))
record("pnpm-workspace.yaml registries as a hex-escaped key, a case variant or in a comment -> caught (fail-closed, "
       "nothing excised)",
       all(any(_WS_REGISTRIES in p for p in m.workspace_registries_problems(_WS_PKGS + t)) for t in
           ('"regis\\x74ries": {}\n', "Registries: {}\n", "# registries: {}\n")))
record("pnpm-workspace.yaml a longer key either side of each refused setting, and onlyBuiltDependencies (not the "
       "File pointer) -> NOT caught",
       all(m.install_mutation_problems({}, _WS_PKGS + t, False) == [] and m.workspace_registries_problems(_WS_PKGS + t)
           == [] for t in ("configDependenciesX: 1\n", "xConfigDependencies: 1\n", "patchedDependenciesX: 1\n",
                           "xPatchedDependencies: 1\n", "onlyBuiltDependenciesFileX: 1\n",
                           "xOnlyBuiltDependenciesFile: 1\n", "dangerouslyAllowAllBuildsX: 1\n",
                           "xDangerouslyAllowAllBuilds: 1\n", "registriesX: 1\n", "xRegistries: 1\n",
                           "onlyBuiltDependencies: []\n")))

# a source-redirect spec prefix is refused anywhere in pnpm-workspace.yaml: pnpm reads a dependency spec from any
# key form and through an escape, which the `^overrides:` block reader does not
_WS_REDIRECT = "carries a source-redirect spec prefix"
record("REPLAY: pnpm-workspace.yaml a flow-form {packages: ['.'], overrides: {is-number: 'link:./y'}} (the lock "
       "recorded `link:y`, measured) -> caught, where the block reader sees nothing",
       any(_WS_REDIRECT in p for p in
           m.install_mutation_problems({}, "{packages: ['.'], overrides: {is-number: 'link:./y'}}\n", False))
       and m._yaml_redirect_block("{packages: ['.'], overrides: {is-number: 'link:./y'}}\n", "overrides") == [])
record("REPLAY: pnpm-workspace.yaml a block-form override with the hex-escaped value \"\\x6cink:./y\" (the lock "
       "recorded `link:y`, measured) -> caught, where the block reader sees nothing",
       any(_WS_REDIRECT in p for p in
           m.install_mutation_problems({}, _WS_PKGS + 'overrides:\n  is-number: "\\x6cink:./y"\n', False))
       and m._yaml_redirect_block(_WS_PKGS + 'overrides:\n  is-number: "\\x6cink:./y"\n', "overrides") == [])
record("pnpm-workspace.yaml each source-redirect prefix (file: link: portal: git+ git: ssh: http: https:, any case) "
       "in a nested flow catalog -> caught",
       all(any(_WS_REDIRECT in p for p in
               m.install_mutation_problems({}, _WS_PKGS + f"catalogs:\n  r: {{react: '{v}x'}}\n", False)) for v in
           ("file:", "link:", "portal:", "git+", "git:", "ssh:", "http:", "https:", "LINK:")))
record("pnpm-workspace.yaml a version pin, an npm: alias, workspace:, `profile:` and `unlink:` -> NOT caught",
       all(m.install_mutation_problems({}, _WS_PKGS + t, False) == [] for t in
           ("overrides:\n  x: ^1.2.3\n", "overrides:\n  x: npm:y@^1\n", "catalog:\n  x: workspace:*\n",
            "profile: x\n", "unlink: x\n")))

# --- .npmrc parsing ---------------------------------------------------------------------------
parsed = m.parse_npmrc("# comment\n\nregistry=https://registry.npmjs.org/\n; semicolon comment\nunsafe-perm=false\n")
# [Test-Change: P0.3.8 - old-obsolete+new-correct, build-gates G18d row] old label: "+ lowercases keys" - the
# reader no longer folds case, because pnpm 10.13.1 reads keys case-sensitively (a `REGISTRY=` line after a
# foreign `registry=` masked it under the fold, measured); the assertion is unchanged and holds.
record("parse_npmrc ignores comments/blanks",
       parsed.get("registry") == "https://registry.npmjs.org/" and parsed.get("unsafe-perm") == "false")

# --- pnpm-lock resolution-URL guard (G18c) ----------------------------------------------------
# [Test-Change: P0.3.8 - old-obsolete+new-correct, G18c 2026-09-24] the old leg passed a `tarball:` resolution from
# the allowed host; the shape control refuses EVERY non-`{integrity}` resolution now (measured through pnpm's
# loader + URL parser: pnpm fetches `{integrity}` from the pinned registry; `tarball` / `repo` / `directory` are
# the keys that could point elsewhere - `tarball` the registry-package one; the older `registry` key the pinned
# pnpm never reads), so the canonical entry is the clean case and the allowed-host token case moves to a URL
# outside a resolution.
record("the canonical registry entry (`resolution: {integrity: sha512-...}`) -> clean",
       m.lockfile_url_problems("packages:\n\n  foo@1.0.0:\n    resolution: {integrity: sha512-AAAA/BBBB==}\n") == [])
record("a URL from the allowed host outside a resolution (a notice) -> clean under the token scan",
       m.lockfile_url_problems("    deprecated: see https://registry.npmjs.org/package/foo\n") == [])
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

# --- G18c (2026-09-24; the gate's module comment): every spelling the reviews measured through pnpm 10.13.1's
# actual loader (@zkochan/js-yaml 0.0.7; the repo's js-yaml 4.3.2 agrees on every form but `!!binary`) + Node's
# URL parser is replayed here; the resolution SHAPE control is the closure, the YAML-indicator refusal guards the
# key, and the posture stays: a deprecation notice's URL is a RED by design, never excised. Every leg drives
# lockfile_url_problems (the function main() calls), never a helper alone.
_PKG = "packages:\n\n  foo@1.0.0:\n"


def _lock(res: str) -> str:
    return _PKG + "    resolution: " + res + "\n"


def _tb(esc: str) -> str:
    """A tarball whose host carries `esc` between the allowed host and `.evil...` - the r4 P0 shape."""
    return '{integrity: sha512-AAAA, tarball: "https://registry.npmjs.org' + esc + '.evil.example.com/x.tgz"}'


def _host_red(lock: str, host: str) -> bool:
    return any("non-allowed host" in p and host in p for p in m.lockfile_url_problems(lock))


def _shape_red(lock: str) -> bool:
    return any("refused by shape" in p for p in m.lockfile_url_problems(lock))


def _indicator_red(lock: str, ch: str) -> bool:
    return any("YAML indicator " + repr(ch) in p for p in m.lockfile_url_problems(lock))


_JOINED = "registry.npmjs.org.evil.example.com"
_ESC = _lock('{integrity: sha512-AAAA, tarball: "https:\\x2f\\x2fevil.example.com/foo.tgz"}')
record("REPLAY (r2 review): a double-quoted `\\x2f` escape spelling a foreign tarball -> decoded, the token names the host",
       _host_red(_ESC, "evil.example.com/foo.tgz"))
record("a `\\u002f` escape spelling the slashes -> decoded, the token names the host",
       _host_red(_ESC.replace("\\x2f", "\\u002f"), "evil.example.com"))
record("a `\\/` escape spelling the slashes -> decoded, the token names the host",
       _host_red(_ESC.replace("\\x2f", "\\/"), "evil.example.com"))
record("a `\\U0000002f` escape spelling the slashes -> decoded, the token names the host",
       _host_red(_ESC.replace("\\x2f", "\\U0000002f"), "evil.example.com"))
record("REPLAY (r4 review, P0): a `\\t` escape inside the host is DELETED as the URL parser does -> the token names "
       "registry.npmjs.org.evil.example.com (never a separator)",
       _host_red(_lock(_tb("\\t")), _JOINED))
record("a `\\n` escape inside the host -> deleted, the joined host is named", _host_red(_lock(_tb("\\n")), _JOINED))
record("a `\\r` escape inside the host -> deleted, the joined host is named", _host_red(_lock(_tb("\\r")), _JOINED))
record("a `\\x09` (tab) escape inside the host -> deleted, the joined host is named",
       _host_red(_lock(_tb("\\x09")), _JOINED))
record("a `\\u000A` (LF) escape inside the host -> deleted, the joined host is named",
       _host_red(_lock(_tb("\\u000A")), _JOINED))
record("a `\\U0000000D` (CR) escape inside the host -> deleted, the joined host is named",
       _host_red(_lock(_tb("\\U0000000D")), _JOINED))
record("a `\\` + TAB escape (js-yaml reads a tab) inside the host -> deleted, the joined host is named",
       _host_red(_lock(_tb("\\\t")), _JOINED))
record("a LITERAL tab inside the host -> deleted, the joined host is named",
       _host_red(_lock(_tb("\t")), _JOINED))
record("REPLAY (r4 review, P1): an ESCAPED LINE BREAK (`https:/\\` + newline + `/evil...`) -> joined as the loader "
       "does, the token names evil.example.com",
       _host_red(_lock('{integrity: sha512-AAAA, tarball: "https:/\\\n      /evil.example.com/x.tgz"}'), "evil.example.com"))
record("REPLAY (r4 review, sonnet P0): a `\\N` (U+0085) right after `://` stays IN the host (ASCII-only token "
       "boundaries) -> the token reds instead of vanishing",
       _host_red(_lock('{integrity: sha512-AAAA, tarball: "https://\\Nevil.example.com/x.tgz"}'), "evil.example.com"))
record("a `\\_` (NBSP) right after `://` -> stays in the host, reds",
       _host_red(_lock('{integrity: sha512-AAAA, tarball: "https://\\_evil.example.com/x.tgz"}'), "evil.example.com"))
record("a `\\L` (U+2028) right after `://` -> stays in the host, reds",
       _host_red(_lock('{integrity: sha512-AAAA, tarball: "https://\\Levil.example.com/x.tgz"}'), "evil.example.com"))
record("a `\\P` (U+2029) right after `://` -> stays in the host, reds",
       _host_red(_lock('{integrity: sha512-AAAA, tarball: "https://\\Pevil.example.com/x.tgz"}'), "evil.example.com"))
record("a `\\x20` (space) right after `://` -> an EMPTY-host token, reds (fail-closed), never a non-match",
       any("non-allowed host: https:// " in p for p in m.lockfile_url_problems(
           _lock('{integrity: sha512-AAAA, tarball: "https://\\x20evil.example.com/x.tgz"}'))))
record("a `git+ssh://\\N...` repo -> the scheme arm refuses it (the git/ssh refusal never vanishes either)",
       any("non-registry resolution scheme" in p for p in m.lockfile_url_problems(
           _lock('{type: git, repo: "git+ssh://\\Nevil.example.com/x.git", commit: abc}'))))
record("an out-of-range `\\U00110000` escape -> a fail-closed problem naming it, no crash (js-yaml accepts it)",
       any("out-of-range escape `\\U00110000`" in p for p in m.lockfile_url_problems(
           _lock('{integrity: sha512-AAAA, tarball: "\\U00110000"}'))))
record("an out-of-range `\\UFFFFFFFF` escape -> a fail-closed problem, no crash",
       any("out-of-range escape" in p for p in m.lockfile_url_problems(_lock('{integrity: sha512-AAAA, tarball: "\\UFFFFFFFF"}'))))
record("decode_yaml_escapes: `\\\\` -> one backslash, `\\t` -> deleted, `\\N` -> U+0085, an unknown `\\x i` sequence in "
       "prose is kept",
       m.decode_yaml_escapes("a\\\\b\\tc\\Nd C:\\x is gone") == "a\\bc\x85d C:\\x is gone")
# the SHAPE control: pnpm reads a fetch location only from a `resolution` key, and only the dumper's registry
# shape passes - a fetch location is refused without the scan having to see its URL
record("SHAPE: the canonical registry entry -> clean, and COUNTED as canonical (not skipped)",
       m.lockfile_url_problems(_lock("{integrity: sha512-AAAA/BBBB==}")) == []
       and m.resolution_shape_problems(_lock("{integrity: sha512-AAAA/BBBB==}")) == ([], 1))
record("SHAPE: the sha1- and sha256- integrity forms are canonical too",
       m.resolution_shape_problems(_lock("{integrity: sha1-AAAA}") + "  bar@1.0.0:\n    resolution: {integrity: sha256-BBBB=}\n")
       == ([], 2))
record("SHAPE: a `tarball:` beside the integrity (the sonnet r4 form, its host hidden by `\\N`) -> refused by shape, "
       "no URL read needed",
       _shape_red(_lock('{integrity: sha512-AAAA, tarball: "https://\\Nevil.example.com/x.tgz"}')))
record("SHAPE: a `tarball:` from the ALLOWED host -> refused too (`tarball` is the registry-package key that could point elsewhere)",
       _shape_red(_lock("{integrity: sha512-AAAA, tarball: https://registry.npmjs.org/foo/-/foo-1.0.0.tgz}")))
record("SHAPE: `http:evil.example.com/x.tgz` (no `//`; the URL parser reads http://evil...) -> refused by shape",
       _shape_red(_lock("{integrity: sha512-AAAA, tarball: http:evil.example.com/x.tgz}")))
record("SHAPE: `http:\\\\evil...` (the URL parser reads `\\` as `/`) -> refused by shape",
       _shape_red(_lock("{integrity: sha512-AAAA, tarball: 'http:\\\\evil.example.com/x.tgz'}")))
record("SHAPE: a `tarball: |-` block scalar with the URL split over two lines -> the resolution line is not canonical, "
       "refused",
       _shape_red(_PKG + "    resolution:\n      integrity: sha512-AAAA\n      tarball: |-\n        https:/\n"
                  "        /evil.example.com/x.tgz\n"))
record("SHAPE: a double-quoted tarball folded over a BLANK line (the loader emits an LF the URL parser deletes: "
       "registry.npmjs.org.evil...) -> the resolution line is not canonical, refused",
       _shape_red(_lock('{integrity: sha512-AAAA, tarball: "https://registry.npmjs.org\n\n      .evil.example.com/x.tgz"}')))
record("SHAPE: a block-form resolution (`resolution:` with its keys on the next lines) -> refused",
       _shape_red(_PKG + "    resolution:\n      integrity: sha512-AAAA\n"))
record("SHAPE: a multi-line flow resolution (`{integrity: ...,` newline `tarball: ...}`) -> refused",
       _shape_red(_PKG + "    resolution: {integrity: sha512-AAAA,\n      tarball: https://evil.example.com/x.tgz}\n"))
record("SHAPE: a git resolution (`{type: git, repo: ..., commit: ...}`) -> refused by shape",
       _shape_red(_lock("{type: git, repo: git+https://github.com/x/y.git, commit: abc}")))
record("SHAPE: a directory resolution -> refused by shape", _shape_red(_lock("{type: directory, directory: ../x}")))
record("SHAPE: a `file:` tarball -> refused by shape",
       _shape_red(_lock("{integrity: sha512-AAAA, tarball: file:../x.tgz}")))
record("SHAPE: a double-quoted `\"resolution\":` key -> refused (the dumper writes the bare key)",
       _shape_red(_PKG + '    "resolution": {integrity: sha512-AAAA, tarball: https://evil.example.com/x.tgz}\n'))
record("SHAPE: an escaped key `\"re\\x73olution\":` -> decoded, then refused",
       _shape_red(_PKG + '    "re\\x73olution": {integrity: sha512-AAAA, tarball: https://evil.example.com/x.tgz}\n'))
record("SHAPE: an escaped-line-break key (`\"resolu\\` newline `tion\":`; js-yaml rejects the indentation, refused "
       "here fail-closed regardless) -> joined, then refused",
       _shape_red(_PKG + '    "resolu\\\n      tion": {integrity: sha512-AAAA, tarball: https://evil.example.com/x.tgz}\n'))
record("SHAPE: a flow-context key/colon split (`{resolution` newline `: {tarball: http:evil...}}`; js-yaml rejects "
       "it, refused here fail-closed regardless) -> the token on the entry line is not canonical, refused",
       _shape_red("packages:\n\n  foo@1.0.0: {resolution\n    : {tarball: http:evil.example.com/x.tgz}}\n"))
record("SHAPE: a merge key carrying a resolution (`<<: {resolution: {tarball: ...}}`; measured: js-yaml reads the "
       "merged key) -> the token on the merge line is not canonical, refused",
       _shape_red(_PKG + "    <<: {resolution: {tarball: http:evil.example.com/x.tgz}}\n"))
record("SHAPE: a resolution at a non-dumper indent (6 spaces) -> refused (the shape is pinned whole)",
       _shape_red(_PKG + "      resolution: {integrity: sha512-AAAA}\n"))
record("SHAPE: a package literally named `resolution@1.0.0` / `@x/resolution` is not the token (no false red)",
       m.resolution_shape_problems("  resolution@1.0.0:\n    resolution: {integrity: sha512-AAAA}\n"
                                   "  '@x/resolution@1.0.0':\n    resolution: {integrity: sha512-BBBB}\n") == ([], 2))
record("SHAPE (named residual, fail-closed): a notice carrying the bare word `resolution` -> reds, never silent",
       _shape_red(_lock("{integrity: sha512-AAAA}") + "    deprecated: no resolution for this, use bar\n"))
record("INDICATOR: an explicit key `? resolution` (measured: js-yaml reads it as the key) -> refused ('?')",
       _indicator_red(_PKG + "    ? resolution\n    : {tarball: http:evil.example.com/x.tgz}\n", "?"))
record("INDICATOR: an anchored key `&r resolution: ...` -> refused ('&')",
       _indicator_red(_PKG + "    &r resolution: {tarball: http:evil.example.com/x.tgz}\n", "&"))
record("INDICATOR: an alias key `*r : ...` -> refused ('*')",
       _indicator_red(_PKG + "    *r : {tarball: http:evil.example.com/x.tgz}\n", "*"))
record("INDICATOR: a tagged key `!!binary cmVzb2x1dGlvbg==: ...` (a YAML error in pnpm's zkochan loader, byte numbers "
       "in js-yaml 4.3.2 - never the key; refused fail-closed regardless) -> refused ('!')",
       _indicator_red(_PKG + "    !!binary cmVzb2x1dGlvbg==: {tarball: http:evil.example.com/x.tgz}\n", "!"))
record("INDICATOR: a `%TAG` directive -> refused ('%')",
       _indicator_red("%TAG ! tag:evil,2026:\n" + _lock("{integrity: sha512-AAAA}"), "%"))
_live = m.decode_yaml_escapes(m.PNPM_LOCK.read_text(encoding="utf-8")) if m.PNPM_LOCK.is_file() else ""
_live_tokens = len(m._RESOLUTION_TOKEN_RE.findall(_live))
# the r5 review's key-gluing forms (measured end to end through pnpm's loader + Node's URL: pnpm read the key and
# fetched): the decoder's deletions and joins OUTSIDE a double-quoted scalar hid the bare `resolution` token from a
# decoded-only shape pass. The shape control reads the RAW text too, and the two spellings are refused outright.
_A = "packages:\n\n  foo@1.0.0: {&z\tresolution: {tarball: http:evil.example.com/x.tgz}}\n"
record("REPLAY (r5 review, A): `{&z<TAB>resolution: {tarball: http:evil...}}` (the deleted tab glues the key onto the "
       "anchor) -> refused twice: the literal-tab refusal AND the RAW-text shape pass",
       any("literal tab" in p for p in m.lockfile_url_problems(_A)) and _shape_red(_A))
record("REPLAY (r5 review, A'): `{!!str<TAB>resolution: ...}` (a tag instead of the anchor) -> refused",
       any("literal tab" in p for p in m.lockfile_url_problems(_A.replace("&z", "!!str"))))
_B = "packages:\n\n  foo@1.0.0:\n    # pinned\\\n    resolution: {tarball: http:evil.example.com/x.tgz}\n"
record("REPLAY (r5 review, B): a comment ending in `\\` + newline before `resolution:` (the join glues the key onto the "
       "comment) -> refused twice: the line-end-backslash refusal AND the RAW-text shape pass",
       any("ends with a backslash" in p for p in m.lockfile_url_problems(_B)) and _shape_red(_B))
record("REPLAY (r5 review, B'): a plain scalar ending in `\\` (`x: a\\` + newline) before `resolution:` -> refused",
       any("ends with a backslash" in p for p in m.lockfile_url_problems(_B.replace("    # pinned\\\n", "    x: a\\\n"))))
_C = _lock("{integrity: sha512-X\\x7d\\N, tarball: http:evil.example.com/x.tgz}")
record("REPLAY (r5 review, C): `sha512-X\\x7d\\N, tarball: ...` (a decoded `}` + U+0085 forged a canonical line under a "
       "universal line split) -> refused by shape",
       _shape_red(_C))
record("the DECODED pass alone on C: lines split at LF only, so the decoded U+0085 stays inside the line and the line is "
       "not canonical",
       m.resolution_shape_problems(m.decode_yaml_escapes(_C))[0] != [])
record("each pass owns one spelling: `\"re\\x73olution\":` is bare only DECODED; `x: a\\` + newline + `resolution:` is bare "
       "only RAW (the join glues it in the decoded text)",
       m.resolution_shape_problems(_PKG + '    "re\\x73olution": {tarball: x}\n')[0] == []
       and m.resolution_shape_problems(m.decode_yaml_escapes(_PKG + '    "re\\x73olution": {tarball: x}\n'))[0] != []
       and m.resolution_shape_problems(m.decode_yaml_escapes(_PKG + "    x: a\\\n    resolution: {tarball: x}\n"))[0] == []
       and m.resolution_shape_problems(_PKG + "    x: a\\\n    resolution: {tarball: x}\n")[0] != [])
record("RESIDUAL (no fetch): a backslash-t ESCAPE in plain context (`{&z\\tresolution: ...}`) glues the key in BOTH "
       "views - and pnpm reads no `resolution` key from it (measured through pnpm's loader: the anchor swallows "
       "`z\\tresolution:`, a plain `\\nresolution` key keeps its backslash), so nothing is hidden",
       m.lockfile_url_problems("packages:\n\n  foo@1.0.0: {&z\\tresolution: {tarball: http:evil.example.com/x.tgz}}\n") == [])
record("a CRLF + BOM lock with the A form -> refused (the literal tab reds on the raw text; CRLF itself is normalized "
       "by main()'s read_text - universal newlines - before the scan, and .gitattributes `* text=auto eol=lf` keeps "
       "the committed lock LF-only; editorconfig-checker, G52, excludes pnpm-lock.yaml by default)",
       any("literal tab" in p for p in m.lockfile_url_problems("\ufeff" + _A.replace("\n", "\r\n"))))
record("the LIVE lock carries no literal tab and no line-end backslash (the two raw refusals cost no false red on the "
       "live lock; a value line ending in a backslash, plain or block-scalar, would red by design)",
       m.PNPM_LOCK.is_file() and m.raw_text_problems(m.PNPM_LOCK.read_text(encoding="utf-8")) == [])
record("the two-pass DEDUPE: a `tarball:` resolution reds by shape once, not once per pass (the raw and the decoded "
       "line are identical)",
       sum("refused by shape" in p for p in m.lockfile_url_problems(_lock("{tarball: http:evil.example.com/x.tgz}"))) == 1)
# the TOKEN scan's named fetch-free gaps, pinned as what they are: a notice fetches nothing, a resolution is caught by shape
record("RESIDUAL (token scan, fetch-free): `http:evil.example.com/x` in a notice carries no `://`, the token scan does not "
       "see it; the same spelling inside a resolution is refused by shape",
       m.lockfile_url_problems("    deprecated: see http:evil.example.com/x\n") == []
       and _shape_red(_lock("{integrity: sha512-AAAA, tarball: http:evil.example.com/x}")))
record("RESIDUAL (token scan, fetch-free): a host cut by a decoded `\\\"` inside a double-quoted notice ends the token at the "
       "quote; inside a resolution the shape refuses it",
       m.lockfile_url_problems('    deprecated: "see https://registry.npmjs.org\\".evil.example.com/x"\n') == []
       and _shape_red(_lock('{integrity: sha512-AAAA, tarball: "https://registry.npmjs.org\\".evil.example.com/x"}')))
record("LIVE: every `resolution` token of the committed pnpm-lock.yaml is canonical, and the canonical count equals "
       "the token count (non-vacuous: at least one)",
       _live_tokens > 0 and m.resolution_shape_problems(_live) == ([], _live_tokens))
record("POSTURE: a `deprecated:` notice naming a foreign host (the 2026-09-24 eslint@9.39.4 entry) is a RED by design, "
       "never excised - the signal to move off the deprecated package",
       any("eslint.org" in p for p in m.lockfile_url_problems(
           "packages:\n\n  eslint@9.39.4:\n    resolution: {integrity: sha512-XoMj}\n"
           "    deprecated: This version is no longer supported. Please see https://eslint.org/version-support for other options.\n")))

record("a block allowlist with a # comment line mid-list -> counts correctly (not under-counted)",
       m._list_count_under("onlyBuiltDependencies:\n  - a\n  # note\n  - b\nother: x\n",
                           "onlyBuiltDependencies") == 2)
record("pnpm 11 allowBuilds list is counted",
       m._list_count_under("allowBuilds:\n  - esbuild\n", "allowBuilds") == 1)

# --- R2 fixes: pnpm-workspace registries: block + .pnpmfile.cjs (source-substitution / install code) -
# [Test-Change: P0.3.8 - old-obsolete+new-correct, build-gates G18c row] old: the refusal named the entry
# (`registries.default`) and a block on the allowed origin was clean; new: the registry pin lives in .npmrc only,
# so a `registries` token is refused at any value under one message - the per-entry block read was a `^registries:`
# line read, and pnpm re-pointed resolution through the flow and the quoted-key map it never saw (measured).
record("pnpm-workspace registries.default off-origin -> caught",
       any(_WS_REGISTRIES in p for p in
           m.workspace_registries_problems("registries:\n  default: https://evil.example.com/\n")))
record("pnpm-workspace registries '@scope' off-origin -> caught",
       m.workspace_registries_problems('registries:\n  "@my-org": https://evil.example.com/\n') != [])
record("pnpm-workspace registries all pointing at the allowed origin -> caught (the pin lives in .npmrc only)",
       any(_WS_REGISTRIES in p for p in
           m.workspace_registries_problems('registries:\n  default: https://registry.npmjs.org/\n'
                                           '  "@my-org": https://registry.npmjs.org/\n')))
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
# [Test-Change: P0.3.8 - old-obsolete+new-correct, build-gates G18d row] old: a falsy literal on the key's line
# was clean; new: the pnpm-workspace.yaml token is refused at any value, because a line read cannot settle the
# value - `dangerouslyAllowAllBuilds: false` plus a more-indented `on` line is the truthy plain scalar `false on`,
# and pnpm ran the dependency's postinstall under it (measured). The package.json `False` leg above stays clean.
record("dangerouslyAllowAllBuilds: a falsy literal -> caught (the pnpm-workspace.yaml token at any value)",
       all(any("`dangerouslyAllowAllBuilds` is set" in p
               for p in m.install_mutation_problems({}, f"dangerouslyAllowAllBuilds: {v}\n", False)) for v in
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
                                 "unsafe-perm=false\n", encoding="utf-8")
    _orig = (m.NPMRC, m.PNPM_LOCK, m.PNPM_WORKSPACE, m.PACKAGE_JSON, m.PNPMFILE_CANDIDATES)
    m.NPMRC, m.PNPM_LOCK = base / ".npmrc", base / "pnpm-lock.yaml"
    m.PNPM_WORKSPACE, m.PACKAGE_JSON = base / "pnpm-workspace.yaml", base / "package.json"
    m.PNPMFILE_CANDIDATES = (base / ".pnpmfile.cjs", base / "pnpmfile.cjs")
    try:
        # [Test-Change: G18 floors as data — old-obsolete+new-correct, §0.8] the synthetic manifest carries an
        # EMPTY floor block (the gate reads the floors from package.json now) instead of patching the retired
        # PINNED_FLOORS_JS constant to {}; it isolates the §0.8 floor from the URL-guard legs the same way.
        (base / "package.json").write_text('{"name":"x","convertia":{"pinned-floors":{}}}',
                                           encoding="utf-8")                   # manifest, NO lock
        rc_nolock = m.main()
        (base / "pnpm-lock.yaml").write_text("resolution: {tarball: https://evil.example.com/x.tgz}\n",
                                             encoding="utf-8")
        rc_foreign = m.main()
        # [Test-Change: P0.3.8 - old-obsolete+new-correct, G18c 2026-09-24] the clean lock is the dumper's
        # canonical registry entry; a `tarball:` (even from the allowed host) is refused by shape now.
        (base / "pnpm-lock.yaml").write_text("packages:\n\n  x@1.0.0:\n    resolution: {integrity: sha512-AAAA}\n",
                                             encoding="utf-8")
        rc_clean = m.main()
        (base / ".pnpmfile.cjs").write_text("module.exports = {}\n", encoding="utf-8")
        rc_pnpmfile = m.main()
        (base / ".pnpmfile.cjs").unlink()
        (base / "pnpm-workspace.yaml").write_text("packages:\n  - '.'\nfrozenLockfile: true\n", encoding="utf-8")
        _ws_err = io.StringIO()
        with contextlib.redirect_stderr(_ws_err):
            rc_ws_frozen = m.main()
        (base / "pnpm-workspace.yaml").write_text("packages:\n  - '.'\nenablePrePostScripts: true\n",
                                                  encoding="utf-8")
        _wl_err = io.StringIO()
        with contextlib.redirect_stderr(_wl_err):
            rc_ws_lifecycle = m.main()
        (base / "pnpm-workspace.yaml").write_text("packages:\n  - '.'\n${UNSET-frozen}Lockfile: true\n",
                                                  encoding="utf-8")
        _we_err = io.StringIO()
        with contextlib.redirect_stderr(_we_err):
            rc_ws_env = m.main()
        (base / "pnpm-workspace.yaml").unlink()
        _npmrc_clean = (base / ".npmrc").read_text(encoding="utf-8")
        (base / ".npmrc").write_text(_npmrc_clean + "[x]\n", encoding="utf-8")
        _rc_err = io.StringIO()
        with contextlib.redirect_stderr(_rc_err):
            rc_npmrc_section = m.main()
        (base / ".npmrc").write_text(_npmrc_clean + "userconfig=./rc2\n", encoding="utf-8")
        _uc_err = io.StringIO()
        with contextlib.redirect_stderr(_uc_err):
            rc_npmrc_unlisted = m.main()
        (base / ".npmrc").write_text(_npmrc_clean, encoding="utf-8")
    finally:
        m.NPMRC, m.PNPM_LOCK, m.PNPM_WORKSPACE, m.PACKAGE_JSON, m.PNPMFILE_CANDIDATES = _orig
    record("main(): a pnpm manifest WITHOUT a lockfile -> FAIL (not a silent skip)", rc_nolock == 1)
    record("main(): a lockfile with a FOREIGN resolution URL -> FAIL", rc_foreign == 1)
    record("main(): a lockfile with only allowed-registry resolutions -> pass", rc_clean == 0)
    record("main(): a committed .pnpmfile.cjs -> FAIL (no install-time code in a zero-egress product)",
           rc_pnpmfile == 1)
    record("main(): a pnpm-workspace.yaml frozenLockfile: true over the clean fixture -> FAIL, naming the setting "
           "(the workspace refusal is wired)", rc_ws_frozen == 1 and _WS_REFUSAL in _ws_err.getvalue())
    record("main(): a `[x]` section header in the clean fixture's .npmrc -> FAIL, naming it (the unmodelled-"
           "spelling refusal is wired)", rc_npmrc_section == 1 and "`[section]`" in _rc_err.getvalue())
    record("main(): a pnpm-workspace.yaml enablePrePostScripts: true over the clean fixture -> FAIL, naming it (the "
           "workspace lifecycle refusal is wired)", rc_ws_lifecycle == 1 and _WS_LIFECYCLE in _wl_err.getvalue())
    record("main(): a pnpm-workspace.yaml `${UNSET-frozen}Lockfile: true` over the clean fixture -> FAIL, naming the "
           "`${` (the workspace env refusal is wired)", rc_ws_env == 1 and _WS_ENV in _we_err.getvalue())
    record("main(): a `userconfig=./rc2` line in the clean fixture's .npmrc -> FAIL, naming the key (the unlisted-key "
           "refusal is wired)",
           rc_npmrc_unlisted == 1 and ".npmrc key `userconfig` is not one this guard evaluates" in _uc_err.getvalue())

# --- the REAL committed .npmrc + main() -------------------------------------------------------
record("the REAL committed .npmrc evaluates clean",
       m.evaluate_npmrc(m.parse_npmrc(m.NPMRC.read_text(encoding="utf-8"))) == [])
record("the REAL committed .npmrc has no spelling the reader cannot settle",
       m.npmrc_unmodelled_problems(m.NPMRC.read_text(encoding="utf-8")) == [])
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


# [Test-Change: G18 floors as data — old-obsolete+new-correct, §0.8] the two test floors are written into a
# temp package.json floor block (the gate's data home) instead of patching the retired PINNED_FLOORS_JS
# constant; the four assertions below are unchanged.
_TWO_FLOORS = '{"convertia": {"pinned-floors": {"@tauri-apps/cli": "2.11.3", "zustand": "5.0.14"}}}'


def _floor_with(manifest: "str | bytes | None", body: "str | None") -> list:
    """_pinned_floor_assertion over a temp package.json (None = absent) and a temp lock (None = absent)."""
    saved = (m.PNPM_LOCK, m.PACKAGE_JSON)
    with tempfile.TemporaryDirectory() as td:
        lock, pkg = Path(td) / "pnpm-lock.yaml", Path(td) / "package.json"
        if body is not None:
            lock.write_text(body, encoding="utf-8")
        if manifest is not None:
            pkg.write_bytes(manifest if isinstance(manifest, bytes) else manifest.encode("utf-8"))
        m.PNPM_LOCK, m.PACKAGE_JSON = lock, pkg
        try:
            return m._pinned_floor_assertion()
        finally:
            m.PNPM_LOCK, m.PACKAGE_JSON = saved


def _floor_with_temp_lock(body: str) -> list:
    return _floor_with(_TWO_FLOORS, body)


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

# --- the floor block is data: every way it can be missing or malformed fails closed ----------------
_REAL_JS_FLOORS, _REAL_JS_WHY = m._load_floors(m.PACKAGE_JSON)
record("_load_floors(): the REAL package.json floor block loads, non-empty, every floor a semver string",
       _REAL_JS_WHY is None and bool(_REAL_JS_FLOORS)
       and all(m._parse_ver(v) is not None for v in _REAL_JS_FLOORS.values()))


def _js_floors_from(text: "str | bytes | None") -> tuple:
    with tempfile.TemporaryDirectory() as td:
        pkg = Path(td) / "package.json"
        if text is not None:
            pkg.write_bytes(text if isinstance(text, bytes) else text.encode("utf-8"))
        return m._load_floors(pkg)


record("_load_floors(): a well-formed block -> the rows",
       _js_floors_from(_TWO_FLOORS) == ({"@tauri-apps/cli": "2.11.3", "zustand": "5.0.14"}, None))
record("_load_floors(): an EMPTY block -> no rows, not a finding (G71's monotone rule guards the emptying)",
       _js_floors_from('{"convertia": {"pinned-floors": {}}}') == ({}, None))
for _label, _text, _needle in (
        ("package.json missing", None, "missing"),
        ("the convertia block absent", '{"name": "x"}', "no `convertia.pinned-floors` object"),
        ("the floor key absent", '{"convertia": {}}', "no `convertia.pinned-floors`"),
        ("convertia not an object", '{"convertia": ["pinned-floors"]}', "no `convertia.pinned-floors`"),
        ("the floors an array", '{"convertia": {"pinned-floors": ["zustand"]}}', "no `convertia.pinned-floors`"),
        ("a top-level array", '[1]', "no `convertia.pinned-floors`"),
        ("unparseable JSON", '{"convertia": {"pinned-floors": {', "unreadable"),
        ("non-UTF-8 bytes", b'\xff{"convertia": {"pinned-floors": {}}}', "unreadable"),
        ("a non-string floor", '{"convertia": {"pinned-floors": {"zustand": 5}}}', "['zustand']"),
        ("a floor that is no semver version", '{"convertia": {"pinned-floors": {"zustand": "^5"}}}', "['zustand']")):
    _floors, _why = _js_floors_from(_text)
    record(f"_load_floors(): {_label} -> fail-closed (no floors, the reason named)",
           _floors is None and isinstance(_why, str) and _needle in _why)
_AT = _imp.format(z="5.0.14", c="2.11.3")
record("_pinned_floor_assertion(): the floor comes from package.json (a raised row reds the same lock)",
       any("zustand" in p and "below the relied-upon API floor" in p for p in _floor_with(
           '{"convertia": {"pinned-floors": {"@tauri-apps/cli": "2.11.3", "zustand": "5.1.0"}}}', _AT)))
record("_pinned_floor_assertion(): the floor block ABSENT -> caught (fail-closed, the home named)",
       any("cannot be asserted" in p and "convertia.pinned-floors" in p for p in _floor_with('{"name": "x"}', _AT)))
record("_pinned_floor_assertion(): package.json ABSENT -> caught (fail-closed)",
       any("cannot be asserted" in p for p in _floor_with(None, _AT)))
record("_pinned_floor_assertion(): pnpm-lock.yaml ABSENT -> caught (was a silent pass before the floors moved to data)",
       any("pnpm-lock.yaml is missing" in p for p in _floor_with(_TWO_FLOORS, None)))
record("_pinned_floor_assertion(): an EMPTY block over a lock -> clean (no rows to assert)",
       _floor_with('{"convertia": {"pinned-floors": {}}}', _AT) == [])

failed = [n for n, ok in results if not ok]
print(f"\n[g24-js-supply-chain] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
