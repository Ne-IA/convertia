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
        # [Test-Change: P0.3.8 - old-obsolete+new-correct, G18c 2026-09-24] the clean lock is the dumper's
        # canonical registry entry; a `tarball:` (even from the allowed host) is refused by shape now.
        (base / "pnpm-lock.yaml").write_text("packages:\n\n  x@1.0.0:\n    resolution: {integrity: sha512-AAAA}\n",
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
