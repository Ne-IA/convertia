#!/usr/bin/env python3
"""g24-ci-supply-chain.py - G24 self-test for check-ci-supply-chain (P0.2.6).

Proves the G56 residual gate FAILS on each violation and PASSES a clean tree, using
throwaway temp .github/ fixture trees (invoking the real check with --root <tmp>).

Three leg families:
  * happy-path legs (the canonical 2-space block dialect the gate is written against);
  * ADVERSARIAL-DIALECT legs (F1-F6) - valid-but-exotic YAML a malicious/careless
    L(-1) `.github/**` diff could use to walk past a naive line scanner: quoted/dup
    write perms, comment-spoofed dependabot ecosystems, release/create polarity,
    flow-style + 4-space jobs, step-vs-job timeout, and `'on':`/scalar trigger keys.
    Each must FAIL; two robustness-positive legs prove a legitimate 4-space tree and a
    comment/run-string `id-token` mention do NOT over-fail;
  * the (11) legs - the actions/cache lockstep shape rule AND the YAML DIALECT the precheck
    enforces for every structural reader (every structural line a `key:` entry or a `- ` item,
    no backslash, no inline `jobs:` / job-id / `steps:` flow value, no spanning flow, no quoted
    node running off its line, no flow-position explicit key / anchor / alias / merge key / tag,
    no quoted mapping key, no BOM anywhere, no character Python treats as white space or a line break outside the four
    every YAML parser agrees on - the predicate Python's own, pinned over every code point and the
    refusal driven per character; the readers split at LF only) - one leg per refused form, the
    r4-r13 review spellings replayed, and no-over-fire guards for the forms the dialect keeps.

stdlib-only. Exit 0 = every assertion held; 1 = a self-test assertion FAILED.
"""
import subprocess
import sys
import tempfile
from pathlib import Path
for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

CHECK = Path(__file__).resolve().parents[2] / "scripts" / "check-ci-supply-chain"
ECOS = ["github-actions", "cargo", "npm", "pip"]
DEFAULT = object()
results: list[tuple[str, bool]] = []


def record(name: str, ok: bool, detail: str = "") -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}{(' - ' + detail) if detail else ''}")


def dependabot(ecos: list[str]) -> str:
    blocks = "\n".join(
        f'  - package-ecosystem: "{e}"\n    directory: "/"\n    schedule: {{ interval: "weekly" }}'
        for e in ecos
    )
    return "version: 2\nupdates:\n" + blocks + "\n"


def push_wf(concurrency: bool = True, timeout: bool = True, idtoken: bool = False, perms: str = "read") -> str:
    out = ["name: ci", "on:", "  push:", "    branches: [main]", "permissions:", f"  contents: {perms}"]
    if idtoken:
        out.append("  id-token: write")
    if concurrency:
        out += ["concurrency:", "  group: ci-x", "  cancel-in-progress: true"]
    out += ["jobs:", "  build:", "    runs-on: ubuntu-22.04"]
    if timeout:
        out.append("    timeout-minutes: 10")
    out += ["    steps:", "      - run: echo hi"]
    return "\n".join(out) + "\n"


def tag_wf(cancel: bool = False) -> str:
    out = ["name: release", "on:", "  push:", "    tags: ['v*']", "permissions:", "  contents: read"]
    if cancel:
        out += ["concurrency:", "  group: rel-x", "  cancel-in-progress: true"]
    out += ["jobs:", "  guard:", "    runs-on: ubuntu-22.04", "    timeout-minutes: 10",
            "    steps:", "      - run: echo hi"]
    return "\n".join(out) + "\n"


def _load_gate_module():
    """The gate as a module (its constants only; every verdict leg drives the real subprocess)."""
    import importlib.machinery
    import importlib.util
    loader = importlib.machinery.SourceFileLoader("check_ci_supply_chain", str(CHECK))
    spec = importlib.util.spec_from_loader("check_ci_supply_chain", loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


def run(td: str) -> tuple[int, str]:
    p = subprocess.run([sys.executable, str(CHECK), "--root", td], capture_output=True, text=True, encoding="utf-8", errors="replace")
    return p.returncode, p.stdout + p.stderr


def leg(name: str, want_sub: str, *, dependabot_yml=DEFAULT, workflows=DEFAULT) -> None:
    """want_sub == "" => expect exit 0 (pass); else expect exit 1 with the substring."""
    with tempfile.TemporaryDirectory() as td:
        gh = Path(td) / ".github"
        (gh / "workflows").mkdir(parents=True)
        if dependabot_yml is not None:  # None => omit the file entirely
            content = dependabot(ECOS) if dependabot_yml is DEFAULT else dependabot_yml
            (gh / "dependabot.yml").write_text(content, encoding="utf-8")
        wfs = {"ci.yml": push_wf(), "release.yml": tag_wf()} if workflows is DEFAULT else workflows
        for fn, body in (wfs or {}).items():
            (gh / "workflows" / fn).write_text(body, encoding="utf-8")
        rc, out = run(td)
        ok = (rc == 0) if not want_sub else (rc == 1 and want_sub in out.lower())
        record(name, ok, f"exit={rc}")


def _with_steps(wf: str, *steps: str) -> str:
    """The given workflow body with the step snippets inserted, in order, before its `echo hi` step."""
    assert "      - run: echo hi\n" in wf
    return wf.replace("      - run: echo hi\n", "".join(steps) + "      - run: echo hi\n", 1)


def _cache(half: str, sha: str, uses_line: str | None = None) -> str:
    """An actions/cache/<half> step; `uses_line` overrides the whole `uses:` line (quoted / folded forms)."""
    line = uses_line if uses_line is not None else f"      - uses: actions/cache/{half}@{sha}\n"
    return line + "        with:\n          path: x\n          key: k\n"


def _with_cache(wf: str, half: str, sha: str) -> str:
    return _with_steps(wf, _cache(half, sha))


def _inline_jobs(job_body: str) -> str:
    """push_wf() with its whole `jobs:` block written as ONE inline flow mapping (the r8 review's form G1)."""
    wf = push_wf()
    block = "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
    assert block in wf
    return wf.replace(block, "jobs: { build: { " + job_body + " } }\n", 1)


def _job_line(job_line: str) -> str:
    """push_wf() with its `build:` job written as the given single job-id line (an inline value)."""
    wf = push_wf()
    block = "  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
    assert block in wf
    return wf.replace(block, "  " + job_line + "\n", 1)


# ---------------------------------------------------------------------------
# happy-path legs (canonical 2-space block dialect)
# ---------------------------------------------------------------------------
leg("clean tree passes", "")
# --- (11) the actions/cache lockstep: one leg per arm; the mirror mutant battery in the commit body records which
# legs each mutant reds (the coverage claim is that measurement, never this banner) ---------------------------
leg("(11) actions/cache save + restore pinned at ONE sha across workflows -> passes (lockstep held)", "",
    workflows={"ci.yml": _with_cache(push_wf(), "save", "a" * 40),
               "release.yml": _with_cache(tag_wf(), "restore", "a" * 40)})
leg("(11) save + restore pinned at DIFFERENT shas across workflows -> fails (lockstep)", "not in lockstep",
    workflows={"ci.yml": _with_cache(push_wf(), "save", "a" * 40),
               "release.yml": _with_cache(tag_wf(), "restore", "b" * 40)})
leg("(11) REPLAY #30: restore@a then save@b inside ONE workflow (the shape the bot's save-only bump produces) "
    "-> fails (lockstep; a first-sha-per-file scan would miss it)", "not in lockstep",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40), _cache("save", "b" * 40)),
               "release.yml": tag_wf()})
leg("(11) the bare `actions/cache@b` form against `restore@a` -> fails (lockstep; the bare arm is read, not refused as unpinned)", "not in lockstep",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     _cache("", "", uses_line="      - uses: actions/cache@" + "b" * 40 + "\n")),
               "release.yml": tag_wf()})
leg("(11) a QUOTED `uses:` value at another sha -> read through its quotes, fails (lockstep, not refused as unpinned)", "not in lockstep",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     _cache("", "", uses_line="      - uses: 'actions/cache/save@" + "b" * 40 + "'\n")),
               "release.yml": tag_wf()})
leg("(11) NO OVER-FIRE: a `# uses: actions/cache/save@b…` comment and a `run: |` body that echoes the string "
    "beside a real restore@a -> passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      # uses: actions/cache/save@" + "b" * 40 + "\n",
                                     "      - run: |\n          echo \"uses: actions/cache/save@" + "b" * 40 + "\"\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL VIEW: a `run: |` body line that itself BEGINS with `uses: actions/cache/save@b…` beside restore@a "
    "-> excised with its block (GitHub reads no step from a string), passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - run: |\n          uses: actions/cache/save@" + "b" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) a FOLDED `uses: >-` value naming actions/cache -> not statically resolvable, fails closed", "outside the one shape",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("", "", uses_line="      - uses: >-\n          actions/cache/save@" + "a" * 40 + "\n")),
               "release.yml": tag_wf()})
leg("(11) a NEXT-LINE `uses:` value naming actions/cache -> fails closed", "outside the one shape",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("", "", uses_line="      - uses:\n          actions/cache/save@" + "a" * 40 + "\n")),
               "release.yml": tag_wf()})
leg("(11) an UPPERCASE sha on actions/cache -> not a lowercase 40-hex pin, fails closed", "lowercase 40-hex",
    workflows={"ci.yml": _with_cache(push_wf(), "save", "A" * 40), "release.yml": tag_wf()})
leg("(11) a double-quoted ESCAPE in the value (\"actions/cach\\x65/save@b…\") beside restore@a -> a backslash on a "
    "structural line is refused by the precheck (the dialect has no escapes)", "backslash",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     _cache("", "", uses_line='      - uses: "actions/cach\\x65/save@' + "b" * 40 + '"\n')),
               "release.yml": tag_wf()})
leg("(11) a FLOW-MAPPING step (`- { uses: actions/cache/save@b…, with: {...} }`) beside restore@a -> not statically "
    "resolvable, fails closed", "outside the one shape",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - { uses: actions/cache/save@" + "b" * 40 + ", with: { path: x, key: k } }\n"),
               "release.yml": tag_wf()})
leg("(11) an UPPERCASE owner/repo (`Actions/Cache/save@b…`) beside restore@a -> read case-insensitively, fails (lockstep)",
    "not in lockstep",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     _cache("", "", uses_line="      - uses: Actions/Cache/save@" + "b" * 40 + "\n")),
               "release.yml": tag_wf()})
leg("(11) NO OVER-FIRE: a local action named `./.github/actions/cache-engines` beside restore@a -> not the cache action, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - uses: ./.github/actions/cache-engines\n"),
               "release.yml": tag_wf()})
leg("(11) a next-line `uses:` value naming ANOTHER action -> refused too (the shape is unscoped: the value must be on "
    "the line)", "outside the one shape",
    workflows={"ci.yml": _with_steps(push_wf(), "      - uses:\n          actions/checkout@" + "c" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) a next-line `uses:` value behind a blank line and a comment line -> refused (the value must be on the line)",
    "outside the one shape",
    workflows={"ci.yml": _with_steps(push_wf(), "      - uses:\n\n          # the value follows\n          actions/cache/save@" + "a" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) a trailing `# v6.1.0` comment on the cache `uses:` line is stripped before the pin is read -> the split still fails (lockstep)",
    "not in lockstep",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     _cache("", "", uses_line="      - uses: actions/cache/save@" + "b" * 40 + "  # v6.1.0\n")),
               "release.yml": tag_wf()})
leg("(11) TOKEN BOUND (lookbehind): a local action named exactly `./.github/actions/cache` beside restore@a -> not the "
    "cache action, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - uses: ./.github/actions/cache\n"), "release.yml": tag_wf()})
leg("(11) TOKEN BOUND (lookahead): `actions/cache-foo@b…` beside restore@a -> not the cache action, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     _cache("", "", uses_line="      - uses: actions/cache-foo@" + "b" * 40 + "\n")),
               "release.yml": tag_wf()})
leg("(11) SHAPE: a flow-mapping step with `uses` as the SECOND key (`- { name: x, uses: actions/cache/save@b… }`) -> refused",
    "outside the one shape",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - { name: x, uses: actions/cache/save@" + "b" * 40 + ", with: { path: x, key: k } }\n"),
               "release.yml": tag_wf()})
leg("(11) SHAPE (r4 review): a flow step with a DOUBLE-QUOTED key (`- {\"uses\": ...}`) -> refused as a flow item", "flow collection",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     '      - {"uses": "actions/cache/save@' + "b" * 40 + '", with: {path: x, key: k}}\n'),
               "release.yml": tag_wf()})
leg("(11) SHAPE (r4 review): a flow step with a SINGLE-QUOTED key (`- {'uses': ...}`) -> refused as a flow item", "flow collection",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - {'uses': 'actions/cache/save@" + "b" * 40 + "', with: {path: x, key: k}}\n"),
               "release.yml": tag_wf()})
leg("(11) SHAPE (r4 review): a `steps: [{uses: ...}]` flow SEQUENCE -> refused", "outside the one shape",
    workflows={"ci.yml": push_wf().replace("    steps:\n      - run: echo hi\n",
                                            "    steps: [{uses: actions/cache/save@" + "b" * 40 + ", with: {path: x, key: k}}]\n"),
               "release.yml": tag_wf()})
leg("(11) SHAPE (r4 review): `steps: [{run: echo}, {uses: ...}]` (the item after a comma) -> refused", "outside the one shape",
    workflows={"ci.yml": push_wf().replace("    steps:\n      - run: echo hi\n",
                                            "    steps: [{run: echo}, {uses: actions/cache/save@" + "b" * 40 + ", with: {path: x, key: k}}]\n"),
               "release.yml": tag_wf()})
leg("(11) SHAPE (r4 review): a MULTI-LINE flow step with `uses:` on a continuation line -> refused", "outside the one shape",
    workflows={"ci.yml": _with_steps(push_wf(), "      - {\n          name: x, uses: actions/cache/save@" + "b" * 40 + ",\n"
                                                "          with: {path: x, key: k}}\n"),
               "release.yml": tag_wf()})
leg("(11) SHAPE (r4 review): an ESCAPED double-quoted key (`\"u\\x73es\": actions/cache/save@b…`) -> refused", "backslash",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     '      - "u\\x73es": actions/cache/save@' + "b" * 40 + "\n        with:\n          path: x\n          key: k\n"),
               "release.yml": tag_wf()})
leg("(11) SHAPE (r4 review): a next-line value with an escaped line break (`uses:` / `\"actions/\\` / `cache/save@b…\"`) -> "
    "refused (the empty value AND the unterminated scalar)", "outside the one shape",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     '      - uses:\n          "actions/\\\n          cache/save@' + "b" * 40 + '"\n'),
               "release.yml": tag_wf()})
leg("(11) SHAPE: an UNTERMINATED double-quoted scalar on a structural line -> refused", "unterminated",
    workflows={"ci.yml": _with_steps(push_wf(), '      - name: "a step\n          continued"\n        run: echo\n'),
               "release.yml": tag_wf()})
leg("(11) SHAPE: a value-position `uses:` with a SECOND key token on the line -> refused", "outside the one shape",
    workflows={"ci.yml": _with_steps(push_wf(), "      - uses: actions/checkout@" + "c" * 40 + " uses: x\n"),
               "release.yml": tag_wf()})
leg("(11) the `- name: x` + `uses:` (no dash) step form beside restore@a -> read, fails (lockstep)", "not in lockstep",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - name: save\n        uses: actions/cache/save@" + "b" * 40 + "\n        with:\n          path: x\n          key: k\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL VIEW (r5 review): a step with a FOLDED `- name: >-` and its `uses: actions/cache/save@b…` sibling "
    "beside restore@a -> the sibling is read (the header's KEY column bounds the excision), fails (lockstep)", "not in lockstep",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - name: >-\n          save the\n          cache\n        uses: actions/cache/save@" + "b" * 40
                                     + "\n        with:\n          path: x\n          key: k\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL VIEW (r5 review): a LITERAL `- if: |` header with the cache `uses:` sibling -> read, fails (lockstep)",
    "not in lockstep",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - if: |\n          true\n        uses: actions/cache/save@" + "b" * 40
                                     + "\n        with:\n          path: x\n          key: k\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL VIEW (r5 review): a plain scalar ending in `:>` (`- name: save:>`) is no block-scalar header -> the "
    "sibling `uses:` is read, fails (lockstep)", "not in lockstep",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - name: save:>\n        uses: actions/cache/save@" + "b" * 40
                                     + "\n        with:\n          path: x\n          key: k\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL VIEW: a real `run: |` body under a step is still excised (the body line more indented than the key column)",
    "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - run: |\n          echo uses: actions/cache/save@" + "b" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) PRECHECK (r5 review): an explicit key after the dash (`- ? uses` / `: actions/cache/save@b…`) -> refused (complex key)",
    "complex key",
    workflows={"ci.yml": _with_steps(push_wf(), "      - ? uses\n        : actions/cache/save@" + "b" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) PRECHECK (r5 review): an explicit key in FLOW position with an escaped name (`- { ? \"u\\x73es\" : ... }`) -> refused",
    "complex key",
    workflows={"ci.yml": _with_steps(push_wf(), '      - { ? "u\\x73es" : actions/cache/save@' + "b" * 40 + ", with: {path: x, key: k} }\n"),
               "release.yml": tag_wf()})
leg("(11) PRECHECK (r5 review): an anchor whose name starts with `-` (`- name: &-c actions/cache/save@b…` + `uses: *-c`) -> "
    "refused (anchor)", "anchor",
    workflows={"ci.yml": _with_steps(push_wf(), "      - name: &-c actions/cache/save@" + "b" * 40 + "\n        uses: *-c\n"),
               "release.yml": tag_wf()})
leg("(11) PRECHECK (r5 review): a merge key after the dash (`- <<: *x`) -> refused (merge key)", "merge key",
    workflows={"ci.yml": _with_steps(push_wf(), "      - <<: *x\n"), "release.yml": tag_wf()})
leg("(11) PRECHECK (r5 review): a flow mapping spanning lines (`- {` / `uses: ...` / `, with: ...}`) -> refused (flow collection)",
    "flow collection",
    workflows={"ci.yml": _with_steps(push_wf(), "      - {\n          uses: actions/cache/save@" + "b" * 40
                                                + "\n          , with: {path: x, key: k}}\n"),
               "release.yml": tag_wf()})
leg("(11) SHAPE (r5 review, M3): an ESCAPED double-quoted key in FLOW position (`- {\"u\\x73es\": ...}`) -> refused (backslash)",
    "backslash",
    workflows={"ci.yml": _with_steps(push_wf(), '      - {"u\\x73es": "actions/cache/save@' + "b" * 40 + '", with: {path: x, key: k}}\n'),
               "release.yml": tag_wf()})
leg("(11) NO OVER-FIRE (r5 review): a plain `run:` value with an apostrophe and a single-quoted word (`echo \"don't\" 'stop'`) "
    "-> no quoted node opens mid-scalar, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - run: echo \"don't\" 'stop'\n"), "release.yml": tag_wf()})
leg("(11) DIALECT: a double-quoted value with an escaped quote (`name: \"a \\\"b\\\" c\"`) -> a backslash on a structural "
    "line is refused (write it single-quoted)", "backslash",
    workflows={"ci.yml": _with_steps(push_wf(), '      - name: "a \\"b\\" c"\n        run: echo\n'), "release.yml": tag_wf()})
leg("(11) NO OVER-FIRE (r5 review): a single-quoted value with the `''` escape (`name: 'it''s'`) -> closed on its line, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - name: 'it''s'\n        run: echo\n"), "release.yml": tag_wf()})
leg("(11) NO OVER-FIRE: a single-quoted value holding double quotes (`name: 'a \"b\" c'`, the backslash-free spelling) -> "
    "passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - name: 'a \"b\" c'\n        run: echo\n"), "release.yml": tag_wf()})
leg("(11) DIALECT (the r8 review, B2 on a block line): `- name: \"${{ !cancelled() \\x7d}\"` + `uses: actions/cache/save@b…` "
    "beside restore@a -> the backslash is refused before the escape-blind expression pre-pass can desynchronise", "backslash",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - name: \"${{ !cancelled() \\x7d}\"\n        uses: actions/cache/save@" + "b" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) DIALECT: a backslash in a single-line `run:` value (`run: echo a\\b`) -> refused (the named residual: write "
    "`run: |`)", "backslash",
    workflows={"ci.yml": _with_steps(push_wf(), "      - run: echo a\\b\n"), "release.yml": tag_wf()})
leg("(11) DIALECT NO OVER-FIRE: a backslash inside a `run: |` BODY (excised) and one inside a comment (cut) -> passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - run: |\n          echo a\\b\n      - run: echo hi # C:\\x\n"),
               "release.yml": tag_wf()})
leg("(11) DIALECT: a backslash in a quoted `name:` value that is neither a key nor a `uses` value (`name: \"a\\tb\"`) -> "
    "refused all the same (the rule reads the line, not the node)", "backslash",
    workflows={"ci.yml": _with_steps(push_wf(), "      - name: \"a\\tb\"\n        run: echo\n"), "release.yml": tag_wf()})
leg("(11) NO OVER-FIRE (r5 review): a `with:` input named `reuses:` beside a plain `uses:` -> not a `uses` key token, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - uses: actions/checkout@" + "c" * 40 + "\n        with:\n          reuses: x\n"),
               "release.yml": tag_wf()})
leg("(11) NO OVER-FIRE (r5 review): a plain `run:` value with a LONE apostrophe (`echo it's fine`) -> a quote inside a plain "
    "scalar opens no node, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - run: echo it's fine\n"), "release.yml": tag_wf()})
leg("(11) DIALECT: a double-quoted node whose closing-looking quote is ESCAPED (`name: \"a \\\"` + next line) -> the "
    "backslash is refused; no escape model decides whether the node closed", "backslash",
    workflows={"ci.yml": _with_steps(push_wf(), '      - name: "a \\"\n          continued"\n        run: echo\n'), "release.yml": tag_wf()})
leg("(11) SHAPE: an unterminated single-quoted node whose only closing-looking quote is the `''` escape (`name: 'it''s` + next "
    "line) -> the escape is honoured, refused (unterminated)", "unterminated",
    workflows={"ci.yml": _with_steps(push_wf(), "      - name: 'it''s\n          more'\n        run: echo\n"), "release.yml": tag_wf()})
leg("(11) STRUCTURAL VIEW: a `:>` plain scalar (`- name: save:>`) opens no block scalar, so a deeper line DIRECTLY under it "
    "stays in view (a header would have excised it - a line-scan pin: YAML itself rejects the continuation) - the cache "
    "`uses:` there is read, fails (lockstep)", "not in lockstep",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - name: save:>\n          uses: actions/cache/save@" + "b" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) PRECHECK: a tag after a DOUBLE dash (`- - !!str x`) -> refused (tag) at any node-start depth", "tag",
    workflows={"ci.yml": _with_steps(push_wf(), "      - - !!str x\n"), "release.yml": tag_wf()})
leg("(11) PRECHECK (r6 review, A): a flow step whose plain scalar `it's` used to hide a flow-position explicit key "
    "(`- { with: {path: x, key: it's}, ? \"u\\x73es\" : actions/cache/save@b…, env: {A: y'} }`) -> the node-aware view keeps "
    "the `?` visible, refused (complex key)", "complex key",
    workflows={"ci.yml": _with_steps(push_wf(), "      - { with: {path: x, key: it's}, ? \"u\\x73es\" : actions/cache/save@"
                                                + "b" * 40 + ", env: {A: y'} }\n"), "release.yml": tag_wf()})
leg("(11) PRECHECK (r6 review, A'): the same with a tag (`!!str \"u\\x73es\" :`) -> refused (tag)", "tag",
    workflows={"ci.yml": _with_steps(push_wf(), "      - { with: {path: x, key: it's}, !!str \"u\\x73es\" : actions/cache/save@"
                                                + "b" * 40 + ", env: {A: y'} }\n"), "release.yml": tag_wf()})
leg("(11) SHAPE (r6 review, B): a flow spanning lines whose apostrophes made each line look balanced (`\"name\":\"v'} }` / "
    "`x\", ? \"u\\x73es\" : …}`) -> the JSON-like `\"k\":\"v` opens a node that runs off the line, refused (unterminated)",
    "unterminated",
    workflows={"ci.yml": _with_steps(push_wf(), "      - { with: {path: x, key: it's}, \"name\":\"v'} }\n"
                                                "          x\", ? \"u\\x73es\" : actions/cache/save@" + "b" * 40 + " }\n"),
               "release.yml": tag_wf()})
leg("(11) SHAPE (r6 review): an escaped double-quoted key after `? ` in flow position (`? \"u\\x73es\" :`) -> the "
    "backslash is refused by the dialect (any position; the r6 arm that read the preceding token is gone)", "backslash",
    workflows={"ci.yml": _with_steps(push_wf(), "      - { ? \"u\\x73es\" : actions/cache/save@" + "b" * 40 + " }\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL VIEW: a `- |` sequence item that is itself the block scalar - a body line ONE column past the dash "
    "(the minimum YAML allows) is excised from the last dash column, passes beside restore@a (a key-column or dash+1 bound "
    "would read it and red the lockstep)", "",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - |\n       uses: actions/cache/save@" + "b" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL VIEW: a header behind a DOUBLE dash (`- - run: |`) - its body is excised from the key column, passes "
    "beside restore@a (a line-scan pin: GitHub rejects a nested sequence in steps)", "",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - - run: |\n            uses: actions/cache/save@" + "b" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL VIEW: a header behind a DOUBLE dash - a SIBLING key at the key column (`- - run: |` / `uses:` at that "
    "column) stays in view and is read, fails (lockstep; a one-dash prefix would excise it)", "not in lockstep",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - - run: |\n          uses: actions/cache/save@" + "b" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) NO OVER-FIRE: an expression whose quoted string holds a brace (`if: ${{ contains(x, '{') }}`) -> the expression "
    "is blanked before the scan, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - if: ${{ contains(github.ref, '{') }}\n        run: echo\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL VIEW: text after the indicator (`- run: |x`) makes no header (the `$` anchor) - the deeper line stays in "
    "view and its `uses:` is read, fails (lockstep; a line-scan pin: YAML rejects `|x`)", "not in lockstep",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - run: |x\n          uses: actions/cache/save@" + "b" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL VIEW: a flow line ending in `: |` (`- {run: |`) makes no header (the flow-start exclusion) - the deeper "
    "`uses:` is read, fails (lockstep; the unbalanced `{` reds too)", "not in lockstep",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - {run: |\n          uses: actions/cache/save@" + "b" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) PRECHECK: a flow SEQUENCE spanning lines (`- [x,` / `y]`) -> refused (flow collection; the `[`/`]` half)",
    "flow collection",
    workflows={"ci.yml": _with_steps(push_wf(), "      - [x,\n          y]\n"), "release.yml": tag_wf()})
leg("(11) PRECHECK: an alias whose name starts with `-` used as a value (`uses: *-c`, no anchor line) -> refused (alias)",
    "alias",
    workflows={"ci.yml": _with_steps(push_wf(), "      - uses: *-c\n"), "release.yml": tag_wf()})
leg("(11) SHAPE: an unterminated node opened after the DASH (`- \"a`) -> refused (unterminated)", "unterminated",
    workflows={"ci.yml": _with_steps(push_wf(), '      - "a\n          b"\n'), "release.yml": tag_wf()})
leg("(11) SHAPE: an unterminated node opened after a flow COMMA (`- {a: 1, \"b`) -> refused (unterminated)", "unterminated",
    workflows={"ci.yml": _with_steps(push_wf(), '      - {a: 1, "b\n          c"}\n'), "release.yml": tag_wf()})
leg("(11) SHAPE: an unterminated node opened at the LINE START (a quoted key `\"uses: x` under a step) -> refused "
    "(unterminated)", "unterminated",
    workflows={"ci.yml": _with_steps(push_wf(), '      - name: x\n        "uses: actions/cache/save@' + "b" * 40 + '\n'),
               "release.yml": tag_wf()})
leg("(11) SHAPE: an unterminated node opened after `? ` (`- ? \"a`) -> refused (unterminated; the explicit key reds too)",
    "unterminated",
    workflows={"ci.yml": _with_steps(push_wf(), '      - ? "a\n          b"\n        : x\n'), "release.yml": tag_wf()})
leg("(11) NO OVER-FIRE (r6 review): a plain `run:` with an apostrophe followed by a `# uses: …` comment -> the comment is "
    "cut by the same node-aware tokenizer, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - run: echo it's # uses: actions/cache/save@" + "b" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) NO OVER-FIRE: the common `if: ${{ !cancelled() }}` idiom -> the expression is blanked before the scan, so its "
    "`!` is no YAML tag, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - if: ${{ !cancelled() }}\n        run: echo\n"), "release.yml": tag_wf()})
# --- the r7 review: the RAW view of checks (6), (8), (10) keeps the shell-quote-aware cut; the structural closure
# refuses flow-collection steps; the tokenizer opens `{`/`[` and starts `-`/`?` nodes only at a node start ----------
leg("(6) RAW VIEW (r7 review P0): a self-hosted secret job whose `run: |` body reads the secret after a shell-quoted "
    "`\"a #\"` -> the body line keeps its tail (HEAD's cut), the secret read is seen, fails (self-hosted)", "self-hosted",
    workflows={"ci.yml": push_wf() + "  evil:\n    runs-on: self-hosted\n    timeout-minutes: 5\n    steps:\n      - run: |\n"
                                     "          echo \"a #\" \"${{ secrets.MINISIGN_SECRET_KEY }}\" > k\n",
               "release.yml": tag_wf()})
leg("(8) RAW VIEW (r7 review P0): a secret-reading hosted job that compiles after a shell-quoted `\"a #\"` -> the compile "
    "is seen, fails (CARGO_NET_OFFLINE)", "cargo_net_offline",
    workflows={"ci.yml": push_wf() + "  evil:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 5\n    steps:\n      - run: |\n"
                                     "          echo \"${{ secrets.MINISIGN_PASSWORD }}\" > p\n"
                                     "          echo \"a #\" && cargo build --release\n",
               "release.yml": tag_wf()})
leg("(10) RAW VIEW (r7 review P0): a restore job that invokes stage-engines after a shell-quoted `\"stage #1\"` -> the "
    "invocation is seen, fails (the restore + stage-engines pair)", "and invokes",
    workflows={"ci.yml": _with_steps(push_wf(), "      - uses: actions/cache/restore@" + "a" * 40 + "\n        with:\n"
                                                "          path: .engine-cache/x\n          key: k\n"
                                                "      - run: |\n          echo \"stage #1\" && python3 scripts/stage-engines --check\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE (r7 review P1, A): a flow step whose plain scalar ` - ` faked a node start (`- { name: x - "
    "\"b, \"u\\x73es\": actions/cache/save@b…, with: {…} }`) -> a sequence item written as a flow collection is refused",
    "flow collection",
    workflows={"ci.yml": _with_steps(push_wf(), "      - { name: x - \"b, \"u\\x73es\": actions/cache/save@" + "b" * 40 + ", with: {path: x, key: k} }\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE (r7 review P1, B2): a flow step whose `if:` expression hides its end behind an escaped `\\x7d` "
    "-> refused as a flow item", "flow collection",
    workflows={"ci.yml": _with_steps(push_wf(), "      - { with: {path: x, key: k}, if: \"${{ !cancelled() \\x7d}\", \"u\\x73es\": actions/cache/save@"
                                                + "b" * 40 + ", name: \"}}\" }\n"), "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE (r7 review P1, C): a flow step with a JSON-like key, whitespace before its `:` and a `#` in "
    "its value (`- { \"name\" :\"a} # \", uses: actions/cache/save@b…, … }`) -> refused as a flow item", "flow collection",
    workflows={"ci.yml": _with_steps(push_wf(), "      - { \"name\" :\"a} # \", uses: actions/cache/save@" + "b" * 40 + ", with: {path: x, key: k} }\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE (r7 review P1, C6): a flow step with a plain key adjacent to its quoted value (`name:\"a} # \"`) "
    "-> refused as a flow item", "flow collection",
    workflows={"ci.yml": _with_steps(push_wf(), "      - { name:\"a} # \", uses: actions/cache/save@" + "b" * 40 + ", with: {path: x, key: k} }\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE: an inline flow `steps:` value -> refused (write the steps in block style)", "inline flow",
    workflows={"ci.yml": push_wf().replace("    steps:\n      - run: echo hi\n",
                                            "    steps: [{run: echo hi}]\n"), "release.yml": tag_wf()})
leg("(11) TOKENIZER (r7 review, D): a JSON-like block key `- \"name\":\"save\"` followed by a cache `uses:` beside restore@a "
    "-> both quoted nodes read, the step's `uses:` read, fails (lockstep)", "not in lockstep",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - \"name\":\"save\"\n        uses: actions/cache/save@" + "b" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) TOKENIZER (r7 review, C-block): a quoted block key with whitespace before its `:` (`- \"name\" : \"a # b\"`) -> refused "
    "by the dialect (a quoted mapping key; the C-block cut leg below keeps the tokenizer's whitespace-skip pinned)", "quoted mapping key",
    workflows={"ci.yml": _with_steps(push_wf(), "      - \"name\" : \"a # b\"\n        run: echo\n"), "release.yml": tag_wf()})
leg("(11) TOKENIZER (r7 review, over-fires gone): `run: echo }` / `run: echo ]` / `run: echo \"}\"` / `ls [ab` -> a brace "
    "inside a plain scalar is content, no flow collection opens, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - run: echo }\n      - run: echo ]\n      - run: echo \"}\"\n      - run: ls [ab\n"),
               "release.yml": tag_wf()})
leg("(11) TOKENIZER (r7 review, over-fires gone): `name: print '{' char` and `if: contains(github.ref, '[')` -> a mid-scalar "
    "quote opens nothing and a mid-scalar brace counts nothing, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - name: print '{' char\n        run: echo x\n"
                                                "      - if: contains(github.ref, '[')\n        run: echo x\n"),
               "release.yml": tag_wf()})
leg("(11) TOKENIZER (r7 review, sonnet P2): a plain block value holding an expression whose single-quoted JSON literal "
    "carries nested closing braces (`V: ${{ toJSON(fromJSON('{\"a\":{\"b\":1}}')) }}`) -> the quote-aware scan ends at the "
    "real `}}`, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - env:\n          V: ${{ toJSON(fromJSON('{\"a\":{\"b\":1}}')) }}\n        run: echo\n"),
               "release.yml": tag_wf()})
leg("(11) TOKENIZER: a ` - ` inside a plain block scalar starts no node (`name: a - 'b` + next line) -> the apostrophe is "
    "content, no unterminated node, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - name: a - 'b\n        run: echo\n"), "release.yml": tag_wf()})
leg("(11) TOKENIZER (r7 review, C-block cut): a quoted block key with whitespace before its `:` whose VALUE holds "
    "`# uses: …` -> the value is a node, its `#` is no comment, and the `uses` token inside reds as outside the shape "
    "(the named name-value residual; a comment cut there would hide it)", "outside the one shape",
    workflows={"ci.yml": _with_steps(push_wf(), "      - \"name\" : \"a # uses: actions/cache/save@" + "b" * 40 + "\"\n        run: echo\n"),
               "release.yml": tag_wf()})
leg("(11) TOKENIZER (r7 review, sonnet P2): an expression whose single-quoted string holds `}}` "
    "(`${{ contains(github.ref, '}}') }}`) -> the quote-aware scan ends at the real `}}`, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - if: ${{ contains(github.ref, '}}') }}\n        run: echo\n"),
               "release.yml": tag_wf()})
leg("(11) PRECHECK: a flow VALUE spanning lines (`with: {path: x,` / `key: k}`) -> refused (the spanning refusal, "
    "independent of the flow-item one)", "flow collection",
    workflows={"ci.yml": _with_steps(push_wf(), "      - uses: actions/checkout@" + "c" * 40 + "\n        with: {path: x,\n          key: k}\n"),
               "release.yml": tag_wf()})
leg("(11) TOKENIZER (r7 review, C on a block line): a quoted key with a space BEFORE its `:` and none after "
    "(`- \"name\" :\"a # uses: …\"`) -> the value node still opens (the whitespace skip, not the `: ` rule), its `#` "
    "is no comment, the `uses` token inside reds as outside the shape", "outside the one shape",
    workflows={"ci.yml": _with_steps(push_wf(), "      - \"name\" :\"a # uses: actions/cache/save@" + "b" * 40 + "\"\n        run: echo\n"),
               "release.yml": tag_wf()})
# --- the r8 review's structural closure: the DIALECT refusals (a flow collection at the start of a line's content, an
# inline `jobs:` / job-id / `steps:` value) - one leg per named form + one per job-level check the inline `jobs:` form
# would skip (the r8 P2, present at HEAD) ------------------------------------------------------------------------
leg("(11) STRUCTURAL CLOSURE (r8 review, N1): a bare `-` with the flow mapping on the NEXT line (`-` / `{ uses: "
    "actions/cache/save@b… }`) beside restore@a -> a flow collection at the start of a line's content is refused",
    "flow collection at the start",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      -\n        { uses: actions/cache/save@" + "b" * 40 + " }\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE (r8 review, N2): `- # comment` with the flow mapping on the next line -> refused (the "
    "comment is cut, the item is a bare `-`)", "flow collection at the start",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - # a step\n        { uses: actions/cache/save@" + "b" * 40 + " }\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE (r8 review, N6/G4): `steps:` with the flow sequence on the NEXT line (`[ { uses: … }, "
    "{ run: echo hi } ]`) -> refused (a flow collection at the start of a line's content)", "flow collection at the start",
    workflows={"ci.yml": push_wf().replace("    steps:\n      - run: echo hi\n",
                                            "    steps:\n      [ { uses: actions/cache/save@" + "b" * 40 + " }, { run: echo hi } ]\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE (r8 review): `jobs:` with the flow mapping on the NEXT line -> refused (a flow collection "
    "at the start of a line's content)", "flow collection at the start",
    workflows={"ci.yml": push_wf().replace("jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n",
                                            "jobs:\n  { build: { runs-on: ubuntu-22.04, timeout-minutes: 10, steps: [ { run: echo hi } ] } }\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE (r8 review, G1): an inline `jobs: { build: { …, steps: [ { uses: actions/cache/save@b… } ] } }` "
    "-> the inline `jobs:` value is refused", "inline `jobs:`",
    workflows={"ci.yml": _inline_jobs("runs-on: ubuntu-22.04, timeout-minutes: 10, steps: [ { uses: actions/cache/save@" + "b" * 40 + " } ]"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE (r8 review P2, check (6)): an inline `jobs:` whose job runs on `self-hosted` and reads a secret "
    "-> refused as inline (HEAD's parse_jobs saw no job and passed it)", "inline `jobs:`",
    workflows={"ci.yml": _inline_jobs("runs-on: self-hosted, timeout-minutes: 10, steps: [ { run: echo ${{ secrets.MINISIGN_SECRET_KEY }} } ]"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE (r8 review P2, check (4)): an inline `jobs:` whose job has NO timeout-minutes -> refused as "
    "inline", "inline `jobs:`",
    workflows={"ci.yml": _inline_jobs("runs-on: ubuntu-22.04, steps: [ { run: echo hi } ]"), "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE (r8 review P2, check (5)): an inline `jobs:` whose job sets `permissions: { contents: write }` "
    "-> refused as inline", "inline `jobs:`",
    workflows={"ci.yml": _inline_jobs("runs-on: ubuntu-22.04, timeout-minutes: 10, permissions: { contents: write }, steps: [ { run: echo hi } ]"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE (r8 review P2, the pin): an inline `jobs:` whose job uses `evil/action@main` (unpinned) -> "
    "refused as inline", "inline `jobs:`",
    workflows={"ci.yml": _inline_jobs("runs-on: ubuntu-22.04, timeout-minutes: 10, steps: [ { uses: evil/action@main } ]"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE: a SCALAR on the top-level `jobs:` line (`jobs: x`) -> refused as an inline `jobs:` value "
    "(the refusal reads any value, not only a flow opener)", "inline `jobs:`",
    workflows={"ci.yml": push_wf().replace("jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n",
                                            "jobs: x\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE: a job-id line with an inline flow SEQUENCE (`build: [ x ]`) -> refused as a job written "
    "inline", "flow-style",
    workflows={"ci.yml": _job_line("build: [ x ]"), "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE: a job-id line with an inline SCALAR (`build: x`) -> refused as a job written inline",
    "flow-style", workflows={"ci.yml": _job_line("build: x"), "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE: a `steps:` flow value in the MIDDLE of a line (`- with: { steps: [ { uses: … } ] }`) -> "
    "refused wherever it sits (the r8 line-anchored form read only `^steps:`)", "inline flow `steps:`",
    workflows={"ci.yml": _with_steps(push_wf(), "      - with: { steps: [ { uses: actions/cache/save@" + "b" * 40 + " } ] }\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE NO OVER-FIRE: a flow value inline after a key that is not `jobs` / a job id / `steps` "
    "(`branches: [main]`, `with: {path: x, key: k}`, a matrix `os: [ubuntu-22.04]`) -> read, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - uses: actions/checkout@" + "c" * 40 + "\n        with: {path: x, key: k}\n")
               .replace("    timeout-minutes: 10\n", "    timeout-minutes: 10\n    strategy:\n      matrix:\n        os: [ubuntu-22.04]\n"),
               "release.yml": tag_wf()})
leg("(11) STRUCTURAL CLOSURE (r8 review P3, the named residual): a matrix `include:` entry written as a flow mapping "
    "(`- { os: ubuntu-22.04, lane: x }`) -> refused like any flow-collection sequence item (write block style)",
    "flow collection at the start",
    workflows={"ci.yml": push_wf().replace("    timeout-minutes: 10\n",
                                            "    timeout-minutes: 10\n    strategy:\n      matrix:\n        include:\n          - { os: ubuntu-22.04, lane: x }\n"),
               "release.yml": tag_wf()})
# --- the r9 review: the line SHAPE (a `?` at a line end with the key on the next line, a `:`-led value line, a
# plain-scalar continuation) -----------------------------------------------------------------------------------------
leg("(11) LINE SHAPE (r9 review, EK1): `?` alone on a line, `uses` on the next, `: actions/cache/save@b…` on the third, "
    "beside restore@a -> the `?`-led line is outside the shape, refused", "outside the dialect's shape",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - name: save\n        ?\n          uses\n        : actions/cache/save@" + "b" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) LINE SHAPE (r9 review, EK2): `- ?` at a line end with the key on the next line -> refused (an item led by "
    "`?`)", "outside the dialect's shape",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - ?\n          uses\n        : actions/cache/save@" + "b" * 40 + "\n        name: save\n"),
               "release.yml": tag_wf()})
leg("(11) LINE SHAPE (r9 review, EK3): `? # the key` (the comment cut leaves a bare `?`) -> refused", "outside the dialect's shape",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                     "      - name: save\n        ? # the key\n          uses\n        : actions/cache/save@" + "b" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) LINE SHAPE (r9 review, the job-level sibling): top-level `?` / `jobs` / `:` hiding a self-hosted secret job with no "
    "timeout -> refused (HEAD and the r9 gate passed it)", "outside the dialect's shape",
    workflows={"ci.yml": push_wf().replace("jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n",
                                            "?\n  jobs\n:\n  build:\n    runs-on: self-hosted\n    steps:\n      - run: echo ${{ secrets.MINISIGN_SECRET_KEY }}\n"),
               "release.yml": tag_wf()})
leg("(11) LINE SHAPE (r9 review): job-level `?` / `permissions` / `:` + `contents: write` -> refused", "outside the dialect's shape",
    workflows={"ci.yml": push_wf().replace("    timeout-minutes: 10\n", "    timeout-minutes: 10\n    ?\n      permissions\n    :\n      contents: write\n"),
               "release.yml": tag_wf()})
leg("(11) LINE SHAPE: a `:`-led value line after a key alone on the previous line (`- uses` / `: actions/cache/save@b…`) -> "
    "refused (the key without its `:`, then the explicit-value indicator)", "outside the dialect's shape",
    workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40), "      - uses\n        : actions/cache/save@" + "b" * 40 + "\n"),
               "release.yml": tag_wf()})
leg("(11) LINE SHAPE (r9 review, the named residual): a plain-scalar continuation line starting with `[` (`- name: save the` / "
    "`[populated] entry`) -> refused (write the value on one line or as a block scalar)", "outside the dialect's shape",
    workflows={"ci.yml": _with_steps(push_wf(), "      - name: save the\n          [populated] entry\n        run: echo\n"),
               "release.yml": tag_wf()})
leg("(11) LINE SHAPE: a plain-scalar continuation line without a bracket (`- name: save the` / `entry`) -> refused all "
    "the same (a line that is neither a `key:` entry nor a `- ` item)", "outside the dialect's shape",
    workflows={"ci.yml": _with_steps(push_wf(), "      - name: save the\n          entry\n        run: echo\n"),
               "release.yml": tag_wf()})
leg("(11) LINE SHAPE: a scalar item led by `-` (`- --locked`) -> refused (an item never starts with an indicator; quote it)",
    "outside the dialect's shape",
    workflows={"ci.yml": _with_steps(push_wf(), "      - name: x\n        with:\n          args:\n            - --locked\n        run: echo\n"),
               "release.yml": tag_wf()})
leg("(11) LINE SHAPE NO OVER-FIRE: a `?` and a `: ` inside values (`run: echo what?`, `run: 'echo a: b'`, `run: echo a:b`), a plain key "
    "holding an underscore (`A_B: x`), a quoted item (`- 'v*'`), a `- |` item and a bare `-` item -> all inside the shape, passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - run: echo what?\n      - run: 'echo a: b'\n      - run: echo a:b\n      - env:\n          A_B: x\n"
                                                "        run: echo\n      - name: x\n        with:\n          args:\n            - 'v*'\n            - |\n              body\n            -\n              k: v\n"
                                                "        run: echo\n"),
               "release.yml": tag_wf()})
leg("(11) LINE SHAPE (r9 review, the O2 residual): `- name: 'steps: [a]'` -> the `steps` flow refusal reads the raw line, reds "
    "(named, fail-closed)", "inline flow `steps:`",
    workflows={"ci.yml": _with_steps(push_wf(), "      - name: 'steps: [a]'\n        run: echo\n"), "release.yml": tag_wf()})
leg("(11) LINE SHAPE (r9 review, the O1 residual): a literal backslash in a SINGLE-quoted value (`- name: 'C:\\x'`) -> refused "
    "(single quotes clear only an escaped quote; a literal backslash needs a block scalar)", "backslash",
    workflows={"ci.yml": _with_steps(push_wf(), "      - name: 'C:\\x'\n        run: echo\n"), "release.yml": tag_wf()})
# --- the r9-r13 reviews' white-space / line-break class: a character Python treats as white space or a line break
# outside the four every YAML parser agrees on is refused anywhere in the file (the 8 splitlines breaks - NEL / U+2028 /
# U+2029 breaks to GitHub's YAML 1.1 parser too - and the 17 further isspace() members such as U+00A0); the predicate
# is Python's own, pinned below over every code point, and the refusal itself is driven with each of the 25 ----------
for _sep_name, _sep in (("VT U+000B", "\x0b"), ("FF U+000C", "\x0c"), ("FS U+001C", "\x1c"), ("GS U+001D", "\x1d"),
                        ("RS U+001E", "\x1e"), ("NEL U+0085", "\x85"), ("U+2028", "\u2028"), ("U+2029", "\u2029")):
    leg(f"(11) LINE BREAK (r9-r11 reviews): {_sep_name} inside a comment forging a block-scalar header "
        "(`- name: 'save' # c<sep>       z: |` + `uses: actions/cache/save@b…` beside restore@a) -> the file is refused: to "
        "str.splitlines() and (for NEL / U+2028 / U+2029) to GitHub's YAML 1.1 parser the header is a real line and the "
        "step's `uses:` its body, to an LF reader it is comment", "treats as whitespace",
        workflows={"ci.yml": _with_steps(push_wf(), _cache("restore", "a" * 40),
                                         "      - name: 'save' # c" + _sep + "       z: |\n        uses: actions/cache/save@" + "b" * 40 + "\n"),
                   "release.yml": tag_wf()})
leg("(11) LINE BREAK (r11 review, S2): on a self-hosted job, `- name: hello  # c<U+2028>        env:<U+2028>          K: ${{ "
    "secrets.MINISIGN_SECRET_KEY }}` -> refused (GitHub reads the env secret behind the comment; check (6) would never see it)",
    "treats as whitespace",
    workflows={"ci.yml": push_wf().replace("    runs-on: ubuntu-22.04\n", "    runs-on: self-hosted\n")
               .replace("      - run: echo hi\n", "      - name: hello  # c\u2028        env:\u2028          K: ${{ secrets.MINISIGN_SECRET_KEY }}\n        run: echo hi\n"),
               "release.yml": tag_wf()})
leg("(11) LINE BREAK (r11 review, R10): beside an engine-cache restore, the `run: |` body line `echo staged # c<NEL>          "
    "python3 scripts/stage-engines --check` -> refused (PyYAML reads the stage call as its own body line; check (10) would not)",
    "treats as whitespace",
    workflows={"ci.yml": _with_steps(push_wf(), "      - uses: actions/cache/restore@" + "a" * 40 + "\n        with:\n"
                                                "          path: .engine-cache/x\n          key: k\n"
                                                "      - run: |\n          echo staged # c\x85          python3 scripts/stage-engines --check\n"),
               "release.yml": tag_wf()})
leg("(11) LINE BREAK: a VT inside a comment on the `runs-on:` line (a forged header above `timeout-minutes:`) -> refused "
    "(str.splitlines() would have excised the timeout and check (4) would red; libyaml rejects the control, YamlDotNet "
    "reads it as content)",
    "treats as whitespace",
    workflows={"ci.yml": push_wf().replace("    runs-on: ubuntu-22.04\n", "    runs-on: ubuntu-22.04 # c\x0b   z: |\n"),
               "release.yml": tag_wf()})
leg("(11) LINE BREAK: dependabot.yml with U+2028 inside a comment on the `updates:` line forging a header above every entry "
    "-> refused (a YAML 1.1 parser reads `updates` as {z: …} and zero ecosystems)", "treats as whitespace",
    dependabot_yml=dependabot(ECOS).replace("updates:\n", "updates: # x\u2028 z: |\n"))
leg("(11) WHITE SPACE (r12 review, a): dependabot.yml `- package-ecosystem:<U+00A0>\"npm\"` -> refused (a `\\s` regex read the "
    "ecosystem, Dependabot's YAML parser cannot load the file)", "treats as whitespace",
    dependabot_yml=dependabot(ECOS).replace('  - package-ecosystem: "npm"', '  - package-ecosystem:\u00a0"npm"'))
leg("(11) WHITE SPACE (r12 review, b): a `services:` entry indented six spaces + four U+00A0 after a folded `options: >-` "
    "(indent 10 > the header's key column 8 to lstrip(), so the entry with its mutable `image:` was excised from check (9); "
    "a registry-qualified benign image beside it, so nothing else reds) -> refused; with the refusal off the gate passed",
    "treats as whitespace",
    workflows={"ci.yml": push_wf().replace("    timeout-minutes: 10\n",
                                            "    timeout-minutes: 10\n    services:\n      a:\n        image: docker.io/library/x@sha256:" + "0" * 64 + "\n"
                                            "        options: >-\n          --health-cmd x\n      \u00a0\u00a0\u00a0\u00a0evil:\n          image: evil:latest\n"),
               "release.yml": tag_wf()})
leg("(11) WHITE SPACE: a key spelled with a trailing U+00A0 (`egress-policy<U+00A0>: block`) -> refused (YAML reads a different "
    "key)", "treats as whitespace",
    workflows={"ci.yml": _with_steps(push_wf(), "      - uses: step-security/harden-runner@" + "d" * 40 + "\n        with:\n"
                                                "          egress-policy\u00a0: block\n"),
               "release.yml": tag_wf()})
_gate_mod = _load_gate_module()
for _c in ("\x0b", "\x0c", "\x1c", "\x1d", "\x1e", "\x1f", "\x85", "\xa0", "\u1680", "\u2000", "\u2001", "\u2002",
           "\u2003", "\u2004", "\u2005", "\u2006", "\u2007", "\u2008", "\u2009", "\u200a", "\u2028", "\u2029", "\u202f",
           "\u205f", "\u3000"):
    record(f"(11) WHITE SPACE (r13 review): the refusal itself fires on U+{ord(_c):04X} inside a file (the application, not "
           "only the predicate)", _gate_mod.foreign_spaces("t", "a" + _c + "b") == 1)
record("(11) WHITE SPACE (r13 review): the refusal does not fire on a zero-width space (U+200B is no white space to "
       "Python either)", _gate_mod.foreign_spaces("t", "a\u200bb") == 0)
leg("(11) DIALECT (r13 review): a U+FEFF at column 0 of a LATER line (`env:` / `<U+FEFF>FOO: bar`) -> refused (libyaml / "
    "yaml.v3 skip it and read a nested key, YamlDotNet and this reader keep it as a top-level key)", "bom",
    workflows={"ci.yml": push_wf().replace("jobs:\n", "env:\n\ufeffFOO: bar\njobs:\n"), "release.yml": tag_wf()})
leg("(11) DIALECT (r13 review): dependabot.yml with a U+FEFF at column 0 of a later line (`<U+FEFF>x: 1` appended: libyaml "
    "cannot load the file; without the refusal check (1) reads all four ecosystems and passes) -> refused", "bom",
    dependabot_yml=dependabot(ECOS) + "\ufeffx: 1\n")
leg("(1) DEPENDABOT (r13 review): an ecosystem value with a suffix (`package-ecosystem: \"npm.x\"`) -> not read as npm "
    "(the value is read to its end; named unreadable, fail-closed)", "unreadable",
    dependabot_yml=dependabot(ECOS).replace('  - package-ecosystem: "npm"', '  - package-ecosystem: "npm.x"'))
_derived = frozenset(chr(c) for c in range(0x110000) if _gate_mod._foreign_space(chr(c)))
_breaks = frozenset(chr(c) for c in range(0x110000) if chr(c) not in "\r\n" and len(("a" + chr(c) + "b").splitlines()) == 2)
record("(11) WHITE SPACE: the gate's predicate, applied over every code point, yields exactly the 25 characters Python treats "
       "as white space beyond YAML's space / tab / LF / CR, and the 8 str.splitlines() breaks are among them",
       _derived == frozenset("\x0b\x0c\x1c\x1d\x1e\x1f\x85\xa0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008"
                             "\u2009\u200a\u2028\u2029\u202f\u205f\u3000")
       and _breaks <= _derived and len(_breaks) == 8)
leg("(11) DIALECT (r10 review): a workflow starting with a UTF-8 BOM (`\ufeffjobs:` hiding a self-hosted secret job with no "
    "timeout) -> refused (YAML strips the BOM; the reader would have read `\ufeffjobs` as a key)", "bom",
    workflows={"ci.yml": "\ufeff" + push_wf().replace("    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n",
                                                        "    runs-on: self-hosted\n    steps:\n      - run: echo ${{ secrets.MINISIGN_SECRET_KEY }}\n"),
               "release.yml": tag_wf()})
leg("(11) DIALECT (r10 review): dependabot.yml starting with a UTF-8 BOM -> refused", "bom",
    dependabot_yml="\ufeff" + dependabot(ECOS))
leg("(11) DIALECT (r10 review, DB2): dependabot.yml whose quoted `prefix: \"deps(cargo)` runs off its line and swallows the "
    "npm entry's lines (`x: y\"` closes it three lines later; YAML folds them into the string, check (1) read a phantom "
    "ecosystem at HEAD) -> the unterminated node is refused in the precheck now", "unterminated",
    dependabot_yml=dependabot(["github-actions", "cargo", "pip"]).replace(
        '  - package-ecosystem: "pip"', '    commit-message:\n      prefix: "deps(cargo)\n  - package-ecosystem: npm\n    directory: /\n    schedule:\n      interval: weekly\n    x: y"\n  - package-ecosystem: "pip"'))
leg("(11) LINE SHAPE (r10 review): a `--` continuation line (`- name: a` / `--`) -> refused (the dash arm admits a run of "
    "dashes separated by whitespace ending in a bare `-`, never `--`)", "outside the dialect's shape",
    workflows={"ci.yml": _with_steps(push_wf(), "      - name: a\n        --\n        run: echo\n"), "release.yml": tag_wf()})
leg("(11) LINE SHAPE: a leading document marker (`---`) -> admitted by the shape, left to the multi-document check, passes", "",
    workflows={"ci.yml": "---\n" + push_wf(), "release.yml": tag_wf()})
leg("missing dependabot.yml fails", "dependabot", dependabot_yml=None)
leg("missing cargo ecosystem fails", "cargo", dependabot_yml=dependabot(["github-actions", "npm", "pip"]))
leg("push wf missing concurrency fails", "concurrency",
    workflows={"ci.yml": push_wf(concurrency=False), "release.yml": tag_wf()})
leg("release wf with cancel-in-progress fails", "must not",
    workflows={"ci.yml": push_wf(), "release.yml": tag_wf(cancel=True)})
leg("job missing timeout-minutes fails", "timeout",
    workflows={"ci.yml": push_wf(timeout=False), "release.yml": tag_wf()})
leg("workflow-level id-token fails", "id-token",
    workflows={"ci.yml": push_wf(idtoken=True), "release.yml": tag_wf()})
leg("top-level write permissions fails", "write",
    workflows={"ci.yml": push_wf(perms="write"), "release.yml": tag_wf()})

# ---------------------------------------------------------------------------
# F1 - top-level write via quoted scalar / duplicate-key last-wins
# ---------------------------------------------------------------------------
leg("F1a quoted top-level write fails", "write", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    'permissions:\n  contents: read\n  packages: "write"\n'
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})
# NB: caught by the permissions BODY parser (last dict value wins), NOT the top-level
# dup-key precheck - both `contents` lines are indented inside the one `permissions:` block.
leg("F1b per-block dup contents last-wins write fails", "write", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    'permissions:\n  contents: read\n  contents: "write"\n'
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# ---------------------------------------------------------------------------
# F2 - dependabot ecosystem coverage spoofed by comments / per-block fields
# ---------------------------------------------------------------------------
leg("F2 comment-spoofed ecosystems fail", "missing", dependabot_yml=(
    "version: 2\nupdates:\n"
    '  # - package-ecosystem: "cargo"\n'
    '  # - package-ecosystem: "npm"\n'
    '  # - package-ecosystem: "pip"\n'
    '  - package-ecosystem: "github-actions"\n    directory: "/"\n    schedule: { interval: "weekly" }\n'
))
leg("F2b ecosystem block missing schedule fails", "schedule", dependabot_yml=(
    "version: 2\nupdates:\n"
    '  - package-ecosystem: "github-actions"\n    directory: "/"\n'
    '  - package-ecosystem: "cargo"\n    directory: "/"\n    schedule: { interval: "weekly" }\n'
    '  - package-ecosystem: "npm"\n    directory: "/"\n    schedule: { interval: "weekly" }\n'
    '  - package-ecosystem: "pip"\n    directory: "/"\n    schedule: { interval: "weekly" }\n'
))

# ---------------------------------------------------------------------------
# F3 - release-class polarity escape (on: release / create, no push.tags token)
# ---------------------------------------------------------------------------
leg("F3 on:release with cancel fails", "must not", workflows={"release.yml": (
    "name: rel\non:\n  release:\n    types: [published]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: rel-x\n  cancel-in-progress: true\n"
    "jobs:\n  guard:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# ---------------------------------------------------------------------------
# F4 - jobs invisible to a hard indent==2 / endswith(":") scan
# ---------------------------------------------------------------------------
leg("F4a flow-style job fails", "flow-style", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build: { runs-on: ubuntu-22.04, timeout-minutes: 10 }\n"
)})
leg("F4b 4-space job without timeout still caught", "timeout", workflows={"ci.yml": (
    "name: ci\non:\n    push:\n        branches: [main]\n"
    "permissions:\n    contents: read\n"
    "concurrency:\n    group: ci-x\n    cancel-in-progress: true\n"
    "jobs:\n    build:\n        runs-on: ubuntu-22.04\n        steps:\n            - run: echo hi\n"
)})
leg("F4c 4-space job WITH timeout passes (robustness)", "", workflows={"ci.yml": (
    "name: ci\non:\n    push:\n        branches: [main]\n"
    "permissions:\n    contents: read\n"
    "concurrency:\n    group: ci-x\n    cancel-in-progress: true\n"
    "jobs:\n    build:\n        runs-on: ubuntu-22.04\n        timeout-minutes: 10\n"
    "        steps:\n            - run: echo hi\n"
)})

# ---------------------------------------------------------------------------
# F5 - step-level timeout-minutes must NOT satisfy the job-level requirement
# ---------------------------------------------------------------------------
leg("F5 step-only timeout fails", "timeout", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    steps:\n      - run: echo hi\n        timeout-minutes: 5\n"
)})

# ---------------------------------------------------------------------------
# F6 - quoted/scalar `on` key disabling the polarity checks
# ---------------------------------------------------------------------------
leg("F6a 'on'-quoted-key release cancel fails", "must not", workflows={"release.yml": (
    "name: rel\n'on':\n  release:\n    types: [published]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: rel-x\n  cancel-in-progress: true\n"
    "jobs:\n  guard:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})
leg("F6b scalar on:push without concurrency fails", "concurrency", workflows={"ci.yml": (
    "name: ci\non: push\n"
    "permissions:\n  contents: read\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# ---------------------------------------------------------------------------
# Round-2 adversarial legs (valid-but-exotic YAML a real parser resolves differently
# than a line scan) - each must FAIL fail-closed.
# ---------------------------------------------------------------------------

# duplicate top-level BLOCK key: YAML resolves to the LAST occurrence
leg("R2a duplicate permissions block (2nd=write) fails", "duplicate", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "permissions:\n  contents: write\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})
leg("R2b duplicate on: block (last=push, no concurrency) fails", "duplicate", workflows={"ci.yml": (
    "name: ci\non:\n  release:\n    types: [published]\n"
    "on:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# permission value on the NEXT line (block/next-line form) - cannot prove read-only
leg("R2c next-line permission write fails", "non-inline", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n  packages:\n    write\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# YAML anchor + merge key (<<:) hiding a write the line scan never resolves.
# Split into single-construct legs so anchor / alias / merge regress INDEPENDENTLY.
leg("R2d anchor + merge-key write fails (combined)", "not statically", workflows={"ci.yml": (
    "name: ci\nx-perm: &wp\n  packages: write\n"
    "on:\n  push:\n    branches: [main]\n"
    "permissions:\n  <<: *wp\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})
leg("R2d1 anchor-only fails", "not statically", workflows={"ci.yml": (
    "name: ci\nenv:\n  X: &a value\n"
    "on:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})
leg("R2d2 alias-only fails", "not statically", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: *grp\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})
leg("R2d3 merge-key-only fails", "not statically", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  <<: base\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# depth-blind trigger: deep workflow_dispatch.inputs.tags must NOT mask a push-branch
leg("R2e workflow_dispatch.inputs.tags + push, no cancel fails", "concurrency", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "  workflow_dispatch:\n    inputs:\n      tags:\n        description: x\n        required: false\n"
    "permissions:\n  contents: read\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# aliased on: trigger - unresolvable by a line scan
leg("R2f aliased on: trigger fails", "not statically", workflows={"release.yml": (
    "name: rel\nx-trig: &t\n  release:\n    types: [published]\n"
    "on: *t\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: rel-x\n  cancel-in-progress: true\n"
    "jobs:\n  guard:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# multi-document file - a second document could override the first
leg("R2g multi-document workflow fails", "multiple", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
    "---\npermissions:\n  contents: write\n"
)})

# ---------------------------------------------------------------------------
# robustness positives - legitimate-but-uncommon dialects must NOT over-fail
# ---------------------------------------------------------------------------
leg("comment/run-string id-token does not false-positive", "", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "# permissions: id-token: write   <- a comment, must be ignored\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n"
    '    steps:\n      - run: echo "id-token: write"\n'
)})

# block-scalar body (run: |) must be excised: no id-token / && / write-all false-positive
leg("R2h block-scalar run body does not false-positive", "", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n"
    "    steps:\n      - name: print\n        run: |\n"
    '          echo "id-token: write"\n          echo "permissions: write-all"\n          echo "a && b"\n'
)})

# reusable-workflow call job (uses:) is exempt from the job-level timeout requirement
leg("R2i uses: job without timeout passes", "", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  call:\n    uses: ./.github/workflows/reusable.yml\n"
)})

# ---------------------------------------------------------------------------
# Round-3 adversarial legs (flow-style values, YAML-1.1 booleans, quoted keys) -
# the valid-but-exotic forms a real YAML parser resolves differently than a line scan.
# ---------------------------------------------------------------------------

# flow-MAPPING trigger `push: { tags: [...] }` hides the release-class tags from the scan
leg("R3a flow-map trigger (push:{tags}) + cancel fails", "classifiable", workflows={"release.yml": (
    "name: rel\non:\n  push: { tags: ['v*'] }\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: rel-x\n  cancel-in-progress: true\n"
    "jobs:\n  guard:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# job-level flow-style permissions granting id-token
leg("R3b job-level flow permissions id-token fails", "flow-style", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n"
    "    permissions: { id-token: write }\n    steps:\n      - run: echo hi\n"
)})

# quoted id-token key bypasses a line-start-anchored scan
# [Test-Change: P0.4.4 (G56 (11) r16) — old-obsolete+new-correct, §6.7.1] the want moves from `id-token` to `quoted mapping
# key`: the dialect now refuses a quoted key BEFORE check (2) reads the grant (old-obsolete: a plain-only read would have
# let the quoted key hide the grant, so the check-(2) message can no longer be the signal); the refusal fires with
# exit 1 (new-correct: the leg below drives it; the r19 review).
leg("R3c quoted 'id-token' key is refused by the dialect (a quoted key would hide the grant from a plain read)", "quoted mapping key",
    workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n  'id-token': write\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# YAML-1.1 truthy `True` on a release workflow must still trip the must-NOT-cancel rule
leg("R3d release wf cancel-in-progress: True fails", "must not", workflows={"release.yml": (
    "name: rel\non:\n  push:\n    tags: ['v*']\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: rel-x\n  cancel-in-progress: True\n"
    "jobs:\n  guard:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})
# ... and the same truthy spelling on a push workflow correctly SATISFIES the push rule
leg("R3d2 push wf cancel-in-progress: True passes", "", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: True\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# robustness: a single-line run: with mid-scalar *glob / &bg shell tokens is NOT a YAML anchor
leg("R3e run: with *glob and &bg shell tokens passes", "", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n"
    "    steps:\n      - run: cp -r dist/ *staging && echo $FOO &bg\n"
)})

# block-list-of-mappings on: (`- push:` with nested tags) - non-GHA-valid, fail-closed
leg("R3g block-list-of-maps on: trigger fails", "classifiable", workflows={"release.yml": (
    "name: rel\non:\n  - push:\n      tags: ['v*']\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: rel-x\n  cancel-in-progress: true\n"
    "jobs:\n  guard:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})
# robustness: a list of event-name SCALARS under on: stays classifiable (must NOT over-fail)
leg("R3h block-list-of-scalars on: passes", "", workflows={"ci.yml": (
    "name: ci\non:\n  - push\n  - pull_request\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# flow-style dependabot item -> one clean diagnostic (not 8-10 misleading ones)
leg("R3f flow-style dependabot item fails", "flow-style", dependabot_yml=(
    "version: 2\nupdates:\n"
    '  - {package-ecosystem: "github-actions", directory: "/", schedule: {interval: "weekly"}}\n'
    '  - package-ecosystem: "cargo"\n    directory: "/"\n    schedule: { interval: "weekly" }\n'
    '  - package-ecosystem: "npm"\n    directory: "/"\n    schedule: { interval: "weekly" }\n'
    '  - package-ecosystem: "pip"\n    directory: "/"\n    schedule: { interval: "weekly" }\n'
))

# explicit YAML tag !!bool changes how `True` resolves - reject fail-closed
leg("R3i !!bool tag on release cancel fails", "tag", workflows={"release.yml": (
    "name: rel\non:\n  push:\n    tags: ['v*']\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: rel-x\n  cancel-in-progress: !!bool true\n"
    "jobs:\n  guard:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})
leg("R3j !!str tag on permission write fails", "tag", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n  packages: !!str write\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})
# robustness: `!` and `&` inside a quoted scalar value must NOT trip the tag/anchor detectors
leg("R3k bang/amp inside quoted value passes", "", workflows={"ci.yml": (
    'name: "Build & Deploy!"\non:\n  push:\n    branches: [main]\n'
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# ---------------------------------------------------------------------------
# Round-4 adversarial legs: per-job permission escalation, block-scalar permission
# values, scheduled-trigger over-fail, expression cancel, complex keys, uses-shape.
# ---------------------------------------------------------------------------

# per-job permissions OVERRIDE the read-only default - a per-job block write must FAIL
leg("R4a per-job block permissions write fails", "per-job", workflows={"release.yml": (
    "name: rel\non:\n  push:\n    tags: ['v*']\n"
    "permissions:\n  contents: read\n"
    "jobs:\n  publish:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n"
    "    permissions:\n      contents: write\n      packages: write\n"
    "    steps:\n      - run: echo hi\n"
)})
# per-job inline write-all must FAIL
leg("R4b per-job inline write-all fails", "write-all", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n"
    "    permissions: write-all\n    steps:\n      - run: echo hi\n"
)})
# robustness: a per-job READ-ONLY block is legitimate and must PASS
leg("R4f per-job read-only block passes", "", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n"
    "    permissions:\n      contents: read\n      pull-requests: read\n"
    "    steps:\n      - run: echo hi\n"
)})

# block-scalar permission value (packages: >- then write) smuggles a write past the line scan
leg("R4c block-scalar permission value fails", "block-scalar", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n  packages: >-\n    write\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# canonical scheduled trigger (on: schedule: - cron:) must NOT over-fail (it had been wrongly rejected)
leg("R4d scheduled-only on: schedule:-cron passes", "", workflows={"ci.yml": (
    "name: nightly\non:\n  schedule:\n    - cron: '0 0 * * *'\n"
    "permissions:\n  contents: read\n"
    "jobs:\n  audit:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})
leg("R4e push + schedule (with cancel) passes", "", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n  schedule:\n    - cron: '0 0 * * *'\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# release-class with an unresolvable ${{ }} cancel expression must fail closed
leg("R4g release cancel ${{ expr }} fails", "must not", workflows={"release.yml": (
    "name: rel\non:\n  push:\n    tags: ['v*']\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: rel-x\n  cancel-in-progress: ${{ true }}\n"
    "jobs:\n  guard:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# robustness: a release workflow may explicitly DISABLE cancel (cancel-in-progress: false)
leg("R4j release cancel-in-progress: false passes", "", workflows={"release.yml": (
    "name: rel\non:\n  push:\n    tags: ['v*']\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: rel-x\n  cancel-in-progress: false\n"
    "jobs:\n  guard:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# YAML complex/explicit key (? key) - statically unverifiable, reject with an honest message
leg("R4h complex key (? permissions) fails", "complex", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "? permissions\n: write-all\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# a non-ref `uses:` value must NOT buy a timeout exemption
leg("R4i non-ref uses: without timeout fails", "timeout", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    uses: total-garbage-not-a-ref\n    steps:\n      - run: echo hi\n"
)})

# robustness: a single-line run: with a !!str shell token is NOT a YAML tag false-positive
leg("R3e2 run: with !!str shell token passes", "", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n"
    "    steps:\n      - run: echo !!str value\n"
)})

# ---------------------------------------------------------------------------
# Round-5 adversarial legs: key-level YAML tag, per-job inline non-read-all scalar,
# and a non-reusable-workflow `uses:` value.
# ---------------------------------------------------------------------------

# a YAML tag on a mapping KEY (`!!str packages: write`) - evaded the value-position tag
# scan AND the perms parser; PyYAML + actionlint both accept it, so it must fail closed here
leg("R5a key-level YAML tag in permissions fails", "tag", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n  !!str packages: write\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# per-job inline `permissions: write` (a non-read-all inline scalar) must fail closed
leg("R5b per-job inline permissions: write fails", "read-only", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n"
    "    permissions: write\n    steps:\n      - run: echo hi\n"
)})

# a `uses:` value with an `@ref` but no `.github/workflows/` path buys no timeout exemption
leg("R5c non-workflow uses: @ref without timeout fails", "timeout", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    uses: some-garbage@v1\n    steps:\n      - run: echo hi\n"
)})

# ---------------------------------------------------------------------------
# Round-6 adversarial legs: dependabot ecosystem-coverage spoof via a field VALUE,
# and permissions body lines a scalar-key:value parse cannot resolve (must fail closed).
# ---------------------------------------------------------------------------

# a decoy `package-ecosystem: cargo` embedded in an earlier field VALUE must NOT shadow the
# item's real ecosystem (docker) - cargo stays uncovered, so coverage must FAIL
leg("R6a dependabot value-spoofed ecosystem fails", "cargo", dependabot_yml=(
    "version: 2\nupdates:\n"
    '  - package-ecosystem: "npm"\n    directory: "/"\n    schedule: { interval: "weekly" }\n'
    '  - package-ecosystem: "pip"\n    directory: "/"\n    schedule: { interval: "weekly" }\n'
    '  - package-ecosystem: "github-actions"\n    directory: "/"\n    schedule: { interval: "weekly" }\n'
    '  - target-branch: "package-ecosystem: cargo"\n    package-ecosystem: "docker"\n'
    '    directory: "/"\n    schedule: { interval: "weekly" }\n'
))

# a quoted permission key with an embedded space (`"!!str packages": write`) the scalar parse
# cannot resolve must fail closed, not be silently skipped
leg("R6b unparseable quoted-key permission fails", "unparseable", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    'permissions:\n  contents: read\n  "!!str packages": write\n'
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# a flow-SEQUENCE permission value (`packages: [write]`) cannot be proven read-only -> fail closed
leg("R6c flow-sequence permission value fails", "flow value", workflows={"ci.yml": (
    "name: ci\non:\n  push:\n    branches: [main]\n"
    "permissions:\n  contents: read\n  packages: [write]\n"
    "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
    "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n      - run: echo hi\n"
)})

# ---------------------------------------------------------------------------
# P0.2.7 runner-host integrity (check 6): a secret-reading job must not be self-hosted,
# a GitHub-hosted secret job must use harden-runner BLOCK, and a non-literal-hosted
# runs-on for a secret job is rejected fail-closed. Non-secret jobs are unconstrained.
# ---------------------------------------------------------------------------

def _sign_wf(runs_on, *, harden=None, harden_if=False, secret=True, multiline=False,
             wf_env=False, secret_expr="${{ secrets.MINISIGN_SECRET_KEY }}",
             run="minisign -Sm SHA256SUMS"):
    """A release-class (tags) workflow with one job. harden=None|'block'|'audit';
    harden_if gates the harden step with `if:`; multiline uses a `run: |` block scalar;
    wf_env puts the secret in a WORKFLOW-level env; secret_expr is the secret reference."""
    out = ["name: release", "on:", "  push:", "    tags: ['v*']", "permissions:", "  contents: read"]
    if wf_env:
        out += ["env:", f"  MK: {secret_expr}"]
    out += ["jobs:", "  sign:", f"    runs-on: {runs_on}", "    timeout-minutes: 10", "    steps:"]
    if harden:
        out += ["      - uses: step-security/harden-runner@abc123"]
        if harden_if:
            out += ["        if: ${{ false }}"]
        out += ["        with:", f"          egress-policy: {harden}"]
    if multiline:
        out += ["      - run: |", f"          {run}"]
    else:
        out += [f"      - run: {run}"]
    if secret and not wf_env:
        out += ["        env:", f"          MK: {secret_expr}"]
    return "\n".join(out) + "\n"


leg("RH1 secret job on self-hosted fails", "self-hosted",
    workflows={"release.yml": _sign_wf("self-hosted")})
leg("RH2 hosted secret job without harden-runner fails", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04")})
leg("RH3 hosted secret job with harden-runner block passes", "",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block")})
leg("RH4 hosted secret job with harden-runner audit (not block) fails", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="audit")})
leg("RH4a (r13 review, pre-existing) harden-runner block + `use-policy-store: true` fails (the dashboard policy "
    "replaces the pinned mode, audit by default)", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace(
        "          egress-policy: block\n", "          egress-policy: block\n          use-policy-store: true\n")})
leg("RH4b (r13 review, pre-existing) harden-runner block + a stored `policy:` name fails", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace(
        "          egress-policy: block\n", "          egress-policy: block\n          policy: my-policy\n")})
leg("RH4c (r13 review) harden-runner `egress-policy: block-x` (a suffixed value harden-runner rejects) is not `block`",
    "harden-runner", workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block-x")})
leg("RH4d harden-runner `egress-policy: block  # pinned` (a trailing comment) still passes", "",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block  # pinned")})
# --- the r14 / r15 reviews' quoted-key class: a quoted mapping key is refused by the dialect (every reader one plain
# spelling) - one leg per read a quoted key had hidden from, plus the case-variant and continue-on-error polarity legs
def _harden_with(extra: str) -> str:
    """The RH3 fixture with `extra` (a `with:` line) appended after `egress-policy: block`."""
    return _sign_wf("ubuntu-22.04", harden="block").replace("          egress-policy: block\n",
                                                         "          egress-policy: block\n" + extra)


leg("RH4e (r14 review) `\"use-policy-store\": true` (a double-quoted key) beside block -> refused by the dialect", "quoted mapping key",
    workflows={"release.yml": _harden_with('          "use-policy-store": true\n')})
leg("RH4f (r14 review) `'policy': my-policy` (a single-quoted key) beside block -> refused by the dialect", "quoted mapping key",
    workflows={"release.yml": _harden_with("          'policy': my-policy\n")})
leg("RH4g (r14 review) `denied-endpoints:` beside block fails (v2.21.0's deny-list mode replaces block-with-allow-list)",
    "harden-runner", workflows={"release.yml": _harden_with("          denied-endpoints: evil.example:443\n")})
leg("RH4h (r14 review) a `\"if\":` (double-quoted key) on the harden step -> refused by the dialect", "quoted mapping key",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace(
        "        with:\n", '        "if": ${{ github.event_name == \'schedule\' }}\n        with:\n')})
leg("RH4i (r14 review) a `'continue-on-error': true` (single-quoted key) on the harden step -> refused by the dialect", "quoted mapping key",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace(
        "        with:\n", "        'continue-on-error': true\n        with:\n")})
leg("RH4j (r14 review) `\"egress-policy\": block` (a quoted key) -> refused by the dialect, never read as enforcement", "quoted mapping key",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace(
        "          egress-policy: block\n", '          "egress-policy": block\n')})
leg("RH4k (r14 review) `egress-policy: 'block'` (a balanced-quoted value) passes", "",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="'block'")})
leg("RH4l (r14 review) `egress-policy: Block` (another case; harden-runner compares case-sensitively) fails", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="Block")})
leg("RH4m (r14 review) `egress-policy: block\"` (an unbalanced quote) fails", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden='block"')})
leg("RH4n (r14 review) `'uses': step-security/harden-runner@…` (a quoted uses key) -> refused by the dialect", "quoted mapping key",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace(
        "      - uses: step-security", "      - 'uses': step-security")})
leg("(3) CANCEL (r14 review) a release workflow's `\"cancel-in-progress\": true` (quoted key) -> refused by the dialect", "quoted mapping key",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace(
        "permissions:\n", 'concurrency:\n  group: rel-x\n  "cancel-in-progress": true\npermissions:\n', 1)})
leg("(3) CANCEL (r14 review) a push workflow's `'cancel-in-progress': true` (quoted key) -> refused by the dialect", "quoted mapping key",
    workflows={"ci.yml": push_wf().replace("  cancel-in-progress: true\n", "  'cancel-in-progress': true\n"),
               "release.yml": tag_wf()})
leg("(1) DEPENDABOT (r14 review) `\"package-ecosystem\": \"npm\"` (a quoted key) -> refused by the dialect", "quoted mapping key",
    dependabot_yml=dependabot(ECOS).replace('  - package-ecosystem: "npm"', '  - "package-ecosystem": "npm"'))
leg("(11) DIALECT (r15 review): a quoted key under a job's `env:` (`\"FOO\": bar`) -> refused (the dialect reads plain keys only)",
    "quoted mapping key", workflows={"ci.yml": _with_steps(push_wf(), "      - env:\n          \"FOO\": bar\n        run: echo\n"),
                                    "release.yml": tag_wf()})
leg("(10) (r15 review) a `\"uses\": actions/cache/restore@…` (quoted key) beside `scripts/stage-engines` -> refused by the "
    "dialect (the restore read never sees a quoted key)", "quoted mapping key",
    workflows={"ci.yml": _with_steps(push_wf(), "      - \"uses\": actions/cache/restore@" + "a" * 40 + "\n        with:\n"
                                                "          path: .engine-cache/x\n          key: x\n      - run: python3 scripts/stage-engines --target t\n"),
               "release.yml": tag_wf()})
leg("RH4o (r15 review) `Use-Policy-Store: true` (another case; the runner matches input names case-insensitively) beside "
    "block fails", "harden-runner", workflows={"release.yml": _harden_with("          Use-Policy-Store: true\n")})
leg("RH4p (r15 review) `Egress-Policy: block` (the key in another case, the value exact) passes", "",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace("          egress-policy: block\n",
                                                                                    "          Egress-Policy: block\n")})
leg("RH4q (r15 review, pre-existing) `continue-on-error: |-` with a `${{ }}` expression on the next line (a block scalar the "
    "structural view excises) on the harden step fails - a present continue-on-error counts only when provably false", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace(
        "        with:\n", "        continue-on-error: |-\n          ${{ github.event_name == 'push' }}\n        with:\n")})
leg("RH4r (r15 review) `continue-on-error: false` on the harden step passes (provably false)", "",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace(
        "        with:\n", "        continue-on-error: false\n        with:\n")})
leg("RH4s (r16 review) a decoy `continue-on-error: false` as the harden step's FIRST `with:` input beside a step-level "
    "`continue-on-error: true` after the block -> the step's own key is read, fails", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace(
        "        with:\n          egress-policy: block\n",
        "        with:\n          continue-on-error: false\n          egress-policy: block\n        continue-on-error: true\n")})
leg("RH4t (r16 review) a decoy `continue-on-error: false` under the harden step's `env:` beside a step-level `${{ }}` "
    "expression -> fails", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace(
        "        with:\n", "        env:\n          continue-on-error: false\n        continue-on-error: ${{ github.event_name == 'push' }}\n        with:\n")})
leg("RH4u (r16 review, pre-existing) `uses: step-security/harden-runner@…` + `egress-policy: block` written as `with:` INPUTS "
    "of actions/checkout, no harden step -> fails", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04").replace(
        "      - run: minisign", "      - uses: actions/checkout@" + "c" * 40 + "\n        with:\n          uses: step-security/harden-runner@"
        + "d" * 40 + "\n          egress-policy: block\n      - run: minisign")})
leg("RH4v (r17 review) a harden step written `-   uses: …` (a multi-space dash) with a step-level `if:` at its real content "
    "indent (dash+4) fails - the own-key read follows the prefix, not dash+2", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace(
        "      - uses: step-security/harden-runner@abc123\n        with:\n          egress-policy: block\n",
        "      -   uses: step-security/harden-runner@abc123\n          if: ${{ github.event_name == 'push' }}\n          with:\n            egress-policy: block\n")})
leg("RH4w (r17 review) the multi-space-dash harden step with a step-level `continue-on-error: true` at dash+4 fails", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace(
        "      - uses: step-security/harden-runner@abc123\n        with:\n          egress-policy: block\n",
        "      -   uses: step-security/harden-runner@abc123\n          continue-on-error: true\n          with:\n            egress-policy: block\n")})
leg("RH4x (r17 review) the multi-space-dash harden step, clean, passes (the with: block at dash+6 is read at its real indent)", "",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace(
        "      - uses: step-security/harden-runner@abc123\n        with:\n          egress-policy: block\n",
        "      -   uses: step-security/harden-runner@abc123\n          with:\n            egress-policy: block\n")})
leg("B1 (r18 review, pre-existing) a with-less harden step followed by a BARE-DASH checkout step carrying `egress-policy: block` "
    "as an input -> the bare dash opens its own step, the harden step has no with: block, fails", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04").replace(
        "      - run: minisign", "      - uses: step-security/harden-runner@abc123\n      -\n        name: Checkout\n"
        "        uses: actions/checkout@" + "c" * 40 + "\n        with:\n          egress-policy: block\n          persist-credentials: false\n"
        "      - run: minisign")})
leg("B3 (r18 review) a bare-dash harden step with an `if:` among its keys (content on the next lines) fails", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04").replace(
        "      - run: minisign", "      -\n        uses: step-security/harden-runner@abc123\n        if: ${{ false }}\n"
        "        with:\n          egress-policy: block\n      - run: minisign")})
leg("B4 (r18 review) a bare-dash harden step, clean (content at dash+2 on the next lines) passes", "",
    workflows={"release.yml": _sign_wf("ubuntu-22.04").replace(
        "      - run: minisign", "      -\n        uses: step-security/harden-runner@abc123\n        with:\n          egress-policy: block\n"
        "      - run: minisign")})
leg("B6 (r18 review) a bare-dash harden step whose content sits DEEPER (dash+4) passes - the next line's indent, not a guess", "",
    workflows={"release.yml": _sign_wf("ubuntu-22.04").replace(
        "      - run: minisign", "      -\n          uses: step-security/harden-runner@abc123\n          with:\n            egress-policy: block\n"
        "      - run: minisign")})
leg("B7 (r19 battery) a harden step whose dash line carries `env:` with a nested mapping and whose `uses:` follows at the content "
    "indent passes - the content indent is the dash-line prefix, not the deeper next line (a next-line-only rule reds it)", "",
    workflows={"release.yml": _sign_wf("ubuntu-22.04").replace(
        "      - run: minisign", "      - env:\n          FOO: bar\n        uses: step-security/harden-runner@abc123\n        with:\n"
        "          egress-policy: block\n      - run: minisign")})
leg("(3) CANCEL (r16 review) a release workflow's flow `concurrency: {group: rel-x, \"cancel-in-progress\": true}` (a quoted key "
    "in FLOW position) -> refused by the dialect (the tokenizer's node start, not a line-leading regex)", "quoted mapping key",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block").replace(
        "permissions:\n", 'concurrency: {group: rel-x, "cancel-in-progress": true}\npermissions:\n', 1)})
leg("(11) DIALECT (r16 review) a quoted key in a flow map under a step's `with:` (`with: {\"path\": x, key: k}`) -> refused",
    "quoted mapping key",
    workflows={"ci.yml": _with_steps(push_wf(), "      - uses: actions/cache/restore@" + "a" * 40 + "\n        with: {\"path\": x, key: k}\n"),
               "release.yml": tag_wf()})
leg("(11) DIALECT NO OVER-FIRE (r16 review): a JSON-like quoted key INSIDE a quoted value (`run: 'echo {\"a\": 1}'`) is not a "
    "key (the value node is blanked first) -> passes", "",
    workflows={"ci.yml": _with_steps(push_wf(), "      - run: 'echo {\"a\": 1}'\n"), "release.yml": tag_wf()})
leg("(1) DEPENDABOT (r14 review) `package-ecosystem: npm\"` (an unbalanced quote) is unreadable - fails naming the value",
    "unreadable", dependabot_yml=dependabot(ECOS).replace('  - package-ecosystem: "npm"', '  - package-ecosystem: npm"'))
leg("RH5 secret job with matrix/expression runs-on fails", "recognized",
    workflows={"release.yml": _sign_wf("${{ matrix.os }}")})
# a NON-secret job on self-hosted is unconstrained (fail-open) - must PASS
leg("RH6 non-secret self-hosted job passes", "",
    workflows={"release.yml": _sign_wf("self-hosted", secret=False, run="echo hi")})
# a literal `secrets.txt` in a run-string is NOT a ${{ secrets }} reference - must PASS
leg("RH7 secrets.txt run-string is not a secret ref (passes)", "",
    workflows={"release.yml": _sign_wf("self-hosted", secret=False, run="cp secrets.txt /tmp/out")})

# --- round-2 runner-host adversarial legs (the 6 reviewer bypasses) ---
# (A) a multi-line `run: |` step's sibling env secret must still be SEEN (not excised)
leg("RH8 multi-line run + env secret on self-hosted fails", "self-hosted",
    workflows={"release.yml": _sign_wf("self-hosted", multiline=True)})
leg("RH9 multi-line run + env secret hosted-no-harden fails", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", multiline=True)})
# (B) bracket-index, uppercase, and workflow-level secret references must be detected
leg("RH10 bracket-index secret on self-hosted fails", "self-hosted",
    workflows={"release.yml": _sign_wf("self-hosted", secret_expr="${{ secrets['MINISIGN_SECRET_KEY'] }}")})
leg("RH11 uppercase SECRETS. on self-hosted fails", "self-hosted",
    workflows={"release.yml": _sign_wf("self-hosted", secret_expr="${{ SECRETS.MINISIGN_SECRET_KEY }}")})
leg("RH12 workflow-level env secret + self-hosted job fails", "self-hosted",
    workflows={"release.yml": _sign_wf("self-hosted", wf_env=True, secret=False, run="echo build")})
# (C) harden-runner must be a REAL unconditional uses-step in BLOCK mode
leg("RH13 harden-runner decoy in a run-string does not satisfy (fails)", "harden-runner",
    workflows={"release.yml": (
        "name: release\non:\n  push:\n    tags: ['v*']\n"
        "permissions:\n  contents: read\n"
        "jobs:\n  sign:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n    steps:\n"
        '      - run: echo "uses: step-security/harden-runner egress-policy: block"\n'
        "      - run: minisign -Sm SHA256SUMS\n        env:\n          MK: ${{ secrets.MINISIGN_SECRET_KEY }}\n"
    )})
leg("RH14 if-gated harden-runner does not satisfy (fails)", "harden-runner",
    workflows={"release.yml": _sign_wf("ubuntu-22.04", harden="block", harden_if=True)})
# (P2) a step-level `with: runs-on:` must not shadow the job-level self-hosted runs-on
leg("RH15 step-level runs-on does not shadow job-level self-hosted (fails)", "self-hosted",
    workflows={"release.yml": (
        "name: release\non:\n  push:\n    tags: ['v*']\n"
        "permissions:\n  contents: read\n"
        "jobs:\n  sign:\n    runs-on: self-hosted\n    timeout-minutes: 10\n    steps:\n"
        "      - uses: some/action@v1\n        with:\n          runs-on: ubuntu-22.04\n"
        "      - run: minisign -Sm SHA256SUMS\n        env:\n          MK: ${{ secrets.MINISIGN_SECRET_KEY }}\n"
    )})
# robustness: an ARM/large hosted label is still a hosted image (must PASS with harden block)
leg("RH16 ARM hosted label secret + harden block passes", "",
    workflows={"release.yml": _sign_wf("ubuntu-24.04-arm", harden="block")})

# --- round-3 runner-host adversarial legs (the 3 reviewer bypasses) ---
_HDR = ("name: release\non:\n  push:\n    tags: ['v*']\n"
        "permissions:\n  contents: read\n"
        "jobs:\n  sign:\n    runs-on: {ro}\n    timeout-minutes: 10\n    steps:\n")

# (1) secret read via toJSON(secrets) / split-delimiter ${{ ... secrets.K }} must be SEEN
leg("RH17 toJSON(secrets) on self-hosted fails", "self-hosted",
    workflows={"release.yml": _sign_wf("self-hosted", secret_expr="${{ toJSON(secrets) }}")})
leg("RH18 split-delimiter secret in run body on self-hosted fails", "self-hosted",
    workflows={"release.yml": _HDR.format(ro="self-hosted") +
        "      - run: |\n          echo ${{\n            secrets.MINISIGN_SECRET_KEY }}\n"})
leg("RH19 folded env split-secret on self-hosted fails", "self-hosted",
    workflows={"release.yml": _HDR.format(ro="self-hosted") +
        "      - run: echo build\n        env:\n          MK: >-\n            ${{\n              secrets.MINISIGN_SECRET_KEY }}\n"})

# (2) a block-scalar (run: | / name: |) decoy must NOT forge a harden-runner block step
leg("RH20 block-scalar run decoy harden does not satisfy (fails)", "harden-runner",
    workflows={"release.yml": _HDR.format(ro="ubuntu-22.04") +
        "      - name: fake\n        run: |\n          uses: step-security/harden-runner@v2\n"
        "          egress-policy: block\n"
        "      - run: minisign -Sm SHA256SUMS\n        env:\n          MK: ${{ secrets.MINISIGN_SECRET_KEY }}\n"})
leg("RH21 name-block-scalar decoy with real audit step (fails)", "harden-runner",
    workflows={"release.yml": _HDR.format(ro="ubuntu-22.04") +
        "      - uses: step-security/harden-runner@v2\n        name: |\n          egress-policy: block\n"
        "        with:\n          egress-policy: audit\n"
        "      - run: minisign -Sm SHA256SUMS\n        env:\n          MK: ${{ secrets.MINISIGN_SECRET_KEY }}\n"})

# (3) a lookalike action name (harden-runner-fork) must NOT satisfy the harden requirement
leg("RH22 lookalike harden-runner-fork does not satisfy (fails)", "harden-runner",
    workflows={"release.yml": _HDR.format(ro="ubuntu-22.04") +
        "      - uses: step-security/harden-runner-fork@v2\n        with:\n          egress-policy: block\n"
        "      - run: minisign -Sm SHA256SUMS\n        env:\n          MK: ${{ secrets.MINISIGN_SECRET_KEY }}\n"})

# robustness: a real harden block step alongside an innocuous run: | must still PASS
leg("RH23 real harden block + innocuous run-block passes", "",
    workflows={"release.yml": _HDR.format(ro="ubuntu-22.04") +
        "      - uses: step-security/harden-runner@v2\n        with:\n          egress-policy: block\n"
        '      - run: |\n          echo "building"\n          echo "done"\n'
        "      - run: minisign -Sm SHA256SUMS\n        env:\n          MK: ${{ secrets.MINISIGN_SECRET_KEY }}\n"})

# --- round-4 runner-host adversarial legs ---
# (P0) egress-policy:block must be a direct child of the harden step's OWN with: mapping -
# not an env var named egress-policy, not a bare deeper key, not a flow-style with:.
leg("RH24 egress-policy under env: (not with:) does not satisfy (fails)", "harden-runner",
    workflows={"release.yml": _HDR.format(ro="ubuntu-22.04") +
        "      - uses: step-security/harden-runner@v2\n        env:\n          egress-policy: block\n"
        "      - run: minisign -Sm SHA256SUMS\n        env:\n          MK: ${{ secrets.MINISIGN_SECRET_KEY }}\n"})
leg("RH25 bare egress-policy step key (no with:) does not satisfy (fails)", "harden-runner",
    workflows={"release.yml": _HDR.format(ro="ubuntu-22.04") +
        "      - uses: step-security/harden-runner@v2\n        egress-policy: block\n"
        "      - run: minisign -Sm SHA256SUMS\n        env:\n          MK: ${{ secrets.MINISIGN_SECRET_KEY }}\n"})
leg("RH26 flow-style with: harden is fail-closed (fails)", "harden-runner",
    workflows={"release.yml": _HDR.format(ro="ubuntu-22.04") +
        "      - uses: step-security/harden-runner@v2\n        with: { egress-policy: block }\n"
        "      - run: minisign -Sm SHA256SUMS\n        env:\n          MK: ${{ secrets.MINISIGN_SECRET_KEY }}\n"})
# (P1) format('{0}', secrets.X) / fromJSON('{}').secrets.X secret reads must be SEEN
leg("RH27 format('{0}', secrets.X) on self-hosted fails", "self-hosted",
    workflows={"release.yml": _sign_wf("self-hosted", secret_expr="${{ format('{0}', secrets.MINISIGN_SECRET_KEY) }}")})
leg("RH28 fromJSON('{}').secrets.X on self-hosted fails", "self-hosted",
    workflows={"release.yml": _sign_wf("self-hosted", secret_expr="${{ fromJSON('{}').secrets.MINISIGN_SECRET_KEY }}")})

# --- round-5 runner-host adversarial legs ---
# (P0) a multi-element runs-on list pulling in a self-hosted CONVENTION label (linux/x64)
# must NOT be classified as hosted on element 0 alone - it can only run self-hosted.
leg("RH29 list runs-on [ubuntu-latest, linux] secret fails", "recognized",
    workflows={"release.yml": _sign_wf("[ubuntu-latest, linux]", harden="block")})
leg("RH30 list runs-on [ubuntu-22.04, x64] secret fails", "recognized",
    workflows={"release.yml": _sign_wf("[ubuntu-22.04, x64]", harden="block")})
# robustness: a single-element all-hosted list is genuinely hosted (must PASS with harden)
leg("RH31 single-element [ubuntu-latest] list + harden passes", "",
    workflows={"release.yml": _sign_wf("[ubuntu-latest]", harden="block")})
# (P1) an escaped-brace format string before the secret token must still be SEEN
leg("RH32 escaped-brace format secret on self-hosted fails", "self-hosted",
    workflows={"release.yml": _sign_wf("self-hosted", secret_expr="${{ format('{{x}}', secrets.MINISIGN_SECRET_KEY) }}")})

# --- round-6 runner-host adversarial legs ---
# a continue-on-error harden step is conditional enforcement -> does NOT satisfy
leg("RH33 continue-on-error harden step does not satisfy (fails)", "harden-runner",
    workflows={"release.yml": _HDR.format(ro="ubuntu-22.04") +
        "      - uses: step-security/harden-runner@v2\n        continue-on-error: true\n"
        "        with:\n          egress-policy: block\n"
        "      - run: minisign -Sm SHA256SUMS\n        env:\n          MK: ${{ secrets.MINISIGN_SECRET_KEY }}\n"})
leg("RH37 continue-on-error: ${{ expr }} harden step does not satisfy (fails)", "harden-runner",
    workflows={"release.yml": _HDR.format(ro="ubuntu-22.04") +
        "      - uses: step-security/harden-runner@v2\n        continue-on-error: ${{ true }}\n"
        "        with:\n          egress-policy: block\n"
        "      - run: minisign -Sm SHA256SUMS\n        env:\n          MK: ${{ secrets.MINISIGN_SECRET_KEY }}\n"})
# block-SEQUENCE runs-on must be resolved like a flow list (every label must be hosted)
_BSEQ = ("name: release\non:\n  push:\n    tags: ['v*']\n"
         "permissions:\n  contents: read\n"
         "jobs:\n  sign:\n    runs-on:\n{labels}    timeout-minutes: 10\n    steps:\n")
leg("RH34 block-seq runs-on with self-hosted fails", "self-hosted",
    workflows={"release.yml": _BSEQ.format(labels="      - self-hosted\n      - linux\n") +
        "      - run: minisign -Sm SHA256SUMS\n        env:\n          MK: ${{ secrets.MINISIGN_SECRET_KEY }}\n"})
leg("RH35 block-seq all-hosted [ubuntu-latest] + harden passes", "",
    workflows={"release.yml": _BSEQ.format(labels="      - ubuntu-latest\n") +
        "      - uses: step-security/harden-runner@v2\n        with:\n          egress-policy: block\n"
        "      - run: minisign -Sm SHA256SUMS\n        env:\n          MK: ${{ secrets.MINISIGN_SECRET_KEY }}\n"})
leg("RH36 block-seq [ubuntu-latest, linux] convention label fails", "recognized",
    workflows={"release.yml": _BSEQ.format(labels="      - ubuntu-latest\n      - linux\n") +
        "      - uses: step-security/harden-runner@v2\n        with:\n          egress-policy: block\n"
        "      - run: minisign -Sm SHA256SUMS\n        env:\n          MK: ${{ secrets.MINISIGN_SECRET_KEY }}\n"})

# ---------------------------------------------------------------------------
# P0.2.10 check 7: cargo-fuzz must name the date-pinned nightly channel
# ---------------------------------------------------------------------------
_FUZZ = ("name: ci\non:\n  push:\n    branches: [main]\n"
         "permissions:\n  contents: read\n"
         "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
         "jobs:\n  fuzz:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n"
         "    steps:\n      - run: {cmd}\n")
leg("FP1 cargo +nightly fuzz (bare) fails", "date-pinned",
    workflows={"ci.yml": _FUZZ.format(cmd="cargo +nightly fuzz run detect")})
leg("FP2 cargo fuzz (no channel) fails", "date-pinned",
    workflows={"ci.yml": _FUZZ.format(cmd="cargo fuzz run detect")})
leg("FP3 cargo +nightly-2026-06-16 fuzz passes", "",
    workflows={"ci.yml": _FUZZ.format(cmd="cargo +nightly-2026-06-16 fuzz run detect")})
leg("FP4 cargo-fuzz binary form fails", "binary form",
    workflows={"ci.yml": _FUZZ.format(cmd="cargo-fuzz run detect")})
# a `run: |` block whose cargo-fuzz spans a shell `\`-continuation must still be seen
_FUZZML = ("name: ci\non:\n  push:\n    branches: [main]\n"
           "permissions:\n  contents: read\n"
           "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
           "jobs:\n  fuzz:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n"
           "    steps:\n      - run: |\n{body}")
leg("FP5 backslash-continuation bare +nightly fuzz fails", "date-pinned",
    workflows={"ci.yml": _FUZZML.format(body="          cargo \\\n            +nightly fuzz run detect\n")})
leg("FP6 flag between cargo and fuzz (cargo +nightly --quiet fuzz) fails", "date-pinned",
    workflows={"ci.yml": _FUZZ.format(cmd="cargo +nightly --quiet fuzz run detect")})
# installing the cargo-fuzz tool is NOT an unpinned invocation -> must PASS
leg("FP7 cargo install cargo-fuzz passes (not an invocation)", "",
    workflows={"ci.yml": _FUZZ.format(cmd="cargo install --locked cargo-fuzz")})
leg("FP8 date-pinned fuzz across a backslash-continuation passes", "",
    workflows={"ci.yml": _FUZZML.format(body="          cargo \\\n            +nightly-2026-06-16 fuzz run detect\n")})
# binary-form cargo-fuzz at the FULL set of shell command positions must all be caught
leg("FP9 env-prefixed cargo-fuzz fails", "binary form",
    workflows={"ci.yml": _FUZZ.format(cmd="RUST_BACKTRACE=1 cargo-fuzz run detect")})
leg("FP10 time-prefixed cargo-fuzz fails", "binary form",
    workflows={"ci.yml": _FUZZ.format(cmd="time cargo-fuzz run detect")})
leg("FP11 cargo-fuzz after loop `do` fails", "binary form",
    workflows={"ci.yml": _FUZZML.format(body="          for t in detect; do cargo-fuzz run; done\n")})
leg("FP12 cargo-fuzz in a subshell fails", "binary form",
    workflows={"ci.yml": _FUZZML.format(body="          (cargo-fuzz run detect)\n")})
leg("FP13 cargo-fuzz in a brace group fails", "binary form",
    workflows={"ci.yml": _FUZZML.format(body="          { cargo-fuzz run detect; }\n")})
leg("FP14 env VAR=val cargo-fuzz fails", "binary form",
    workflows={"ci.yml": _FUZZ.format(cmd="env RUST_BACKTRACE=1 cargo-fuzz run detect")})
leg("FP15 nohup cargo-fuzz fails", "binary form",
    workflows={"ci.yml": _FUZZ.format(cmd="nohup cargo-fuzz run detect")})

# ---------------------------------------------------------------------------
# (8) build.rs/proc-macro execution-isolation for the secret job (P0.4.4) - import-based unit legs
# (call check_secret_offline_build directly so rule 8 is isolated from rules 1-7).
# ---------------------------------------------------------------------------
import importlib.machinery
import importlib.util

_l = importlib.machinery.SourceFileLoader("ccsc", str(CHECK))
_mod = importlib.util.module_from_spec(importlib.util.spec_from_loader("ccsc", _l))
_l.exec_module(_mod)


def rule8(wf: str) -> int:
    raw = wf.split("\n")
    return _mod.check_secret_offline_build("t.yml", _mod.structural_lines(raw),
                                           [_mod.strip_comment(x) for x in raw])


_SEC = "${{ secrets.MINISIGN_SECRET_KEY }}"


def sjob(steps: list[str], *, offline: bool, secret: bool = True, top_env_secret: bool = False) -> str:
    out = []
    if top_env_secret:
        out += ["env:", f"  TOK: {_SEC}"]
    out += ["jobs:", "  sign:", "    runs-on: ubuntu-22.04", "    timeout-minutes: 30", "    env:"]
    if offline:
        out.append('      CARGO_NET_OFFLINE: "true"')
    if secret:
        out.append(f"      KEY: {_SEC}")
    if not offline and not secret:
        out[-1:] = []  # drop the empty `env:` if nothing under it
    out += ["    steps:"] + ["      - run: " + s for s in steps]
    return "\n".join(out) + "\n"


FETCH, BUILD = "cargo fetch --locked", "cargo build --release"
record("(8) secret job: fetch --locked + offline + build AFTER fetch -> clean",
       rule8(sjob([FETCH, BUILD], offline=True)) == 0)
record("(8) secret job: cargo build with NO `cargo fetch --locked` is caught",
       rule8(sjob([BUILD], offline=True)) >= 1)
record("(8) secret job: cargo build without CARGO_NET_OFFLINE=true is caught",
       rule8(sjob([FETCH, BUILD], offline=False)) >= 1)
record("(8) secret job: a cargo build BEFORE the fetch is caught (ordering)",
       rule8(sjob([BUILD, FETCH], offline=True)) >= 1)
record("(8) secret job with NO compiling cargo command -> nothing to assert (clean)",
       rule8(sjob(["echo signing"], offline=False)) == 0)
record("(8) a NON-secret job running cargo build is unconstrained (fail-open until P10)",
       rule8(sjob([BUILD], offline=False, secret=False)) == 0)
record("(8) a workflow-LEVEL env secret makes the job secret-reading (build w/o offline caught)",
       rule8(sjob([FETCH, BUILD], offline=False, secret=False, top_env_secret=True)) >= 1)
# broadened compile detection (P0.4.4 G1 P2 fix): tauri-build wrappers + other build.rs-running subcommands
record("(8) a `pnpm tauri build` in a secret job (no offline/fetch) is CAUGHT",
       rule8(sjob(["pnpm tauri build"], offline=False)) >= 1)
record("(8) a `cargo install <tool>` in a secret job (no offline/fetch) is CAUGHT",
       rule8(sjob(["cargo install some-tool"], offline=False)) >= 1)
record("(8) a toolchain-override `cargo +nightly build` is detected as a compile (no fetch -> caught)",
       rule8(sjob(["cargo +nightly build --release"], offline=True)) >= 1)
# the compile scan is scoped to the steps: section -> a 'cargo build' in a job-level env VALUE is not a
# phantom compile (would otherwise fire a spurious ordering error before the real fetch).
_PHANTOM = "\n".join(["jobs:", "  sign:", "    runs-on: ubuntu-22.04", "    timeout-minutes: 30",
                      "    env:", '      CARGO_NET_OFFLINE: "true"',
                      "      NOTE: rebuild the cargo build artifacts dir", f"      KEY: {_SEC}",
                      "    steps:", "      - run: cargo fetch --locked", "      - run: cargo build --release"]) + "\n"
record("(8) a 'cargo build' embedded in a job-level env VALUE is NOT a phantom compile (clean)",
       rule8(_PHANTOM) == 0)

# ---------------------------------------------------------------------------
# (9) image digests + (10) engine-cache restore/stage ordering (the P4.28.1 tail's sub-rules)
# ---------------------------------------------------------------------------
def _container_wf(image_line: str, extra_steps: str = "      - run: echo hi\n") -> str:
    return (
        "name: ci\non:\n  push:\n    branches: [main]\n"
        "permissions:\n  contents: read\n"
        "concurrency:\n  group: ci-x\n  cancel-in-progress: true\n"
        "jobs:\n  build:\n    runs-on: ubuntu-22.04\n    timeout-minutes: 10\n"
        + image_line
        + "    steps:\n" + extra_steps
    )


leg("(9) a mutable-tag container image fails", "digest-pinned", workflows={
    "ci.yml": _container_wf("    container:\n      image: debian:bookworm-slim\n"),
    "release.yml": tag_wf()})
leg("(9) a digest-pinned container image passes", "", workflows={
    "ci.yml": _container_wf(
        "    container:\n      image: docker.io/library/debian:bookworm-slim@sha256:" + "8" * 64 + "\n"),
    "release.yml": tag_wf()})
_RESTORE = ("      - uses: actions/cache/restore@" + "0" * 40 + "\n"
            "        with:\n"
            "          path: .engine-cache/x\n"
            "          key: x\n")
leg("(9) a value-less `image:` (next-line value) fails closed", "no value", workflows={
    "ci.yml": _container_wf("    container:\n      image:\n        debian:bookworm-slim\n"),
    "release.yml": tag_wf()})
leg("(9) a `container:` block with NO resolvable image child fails closed", "no resolvable",
    workflows={"ci.yml": _container_wf("    container:\n      options: --cpus 1\n"),
               "release.yml": tag_wf()})
leg("(9) a flow-style `container: {...}` fails closed", "not a digest-pinned", workflows={
    "ci.yml": _container_wf("    container: { image: debian }\n"),
    "release.yml": tag_wf()})
leg("(9) an expression image fails closed", "not a digest-pinned", workflows={
    "ci.yml": _container_wf("    container:\n      image: ${{ matrix.image }}\n"),
    "release.yml": tag_wf()})
leg("(10) an engine-cache restore + stage-engines in ONE job fails (no verify actor between)",
    "restore-re-verify", workflows={
        "ci.yml": _container_wf("", _RESTORE + "      - run: python3 scripts/stage-engines --check\n"),
        "release.yml": tag_wf()})
# The r2 review's P1: the rule must see through a BLOCK SCALAR - the repo's dominant idiom for
# a multi-command step, and the one shape the excised structural view is blind to.
leg("(10) the SAME pair with stage-engines inside a `run: |` block scalar STILL fails",
    "restore-re-verify", workflows={
        "ci.yml": _container_wf("", _RESTORE
                                + "      - run: |\n"
                                + "          echo populate\n"
                                + "          python3 scripts/stage-engines --check\n"),
        "release.yml": tag_wf()})
leg("(10) a restore-only job (the populate shape) passes", "", workflows={
    "ci.yml": _container_wf("", _RESTORE + "      - run: python3 scripts/fetch-engine-assets --all --check\n"),
    "release.yml": tag_wf()})
# Pins rule (10)'s SCOPE in the over-fire direction (the r3 review's unguarded-mutant finding):
# a cache restore under a NON-engine path beside stage-engines must PASS - dropping the
# `.engine-cache` conjunct would wrongly refuse e.g. a Cargo-cache-restoring staging job.
_OTHER_RESTORE = ("      - uses: actions/cache/restore@" + "0" * 40 + "\n"
                  "        with:\n"
                  "          path: target/some-cache/x\n"
                  "          key: x\n")
leg("(10) a NON-engine cache restore beside stage-engines passes (the scope pin)", "",
    workflows={"ci.yml": _container_wf("", _OTHER_RESTORE
                                       + "      - run: python3 scripts/stage-engines --check\n"),
               "release.yml": tag_wf()})

failed = [n for n, ok in results if not ok]
print(f"\n[g24-ci-supply-chain] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
