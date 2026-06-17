#!/usr/bin/env python3
"""g24-repo-invariants.py - G24 self-test for check-repo-invariants (P0.3.10, G9).

Positive+negative legs PER invariant (a..f), the row-mandated machine proof of both the catch and
the carve-out. The load-bearing legs are invariant (e)'s prose-vs-invocation discrimination: a real
`run: cargo vet update` YAML step FAILS, while the same phrase as PROSE (a comment, a Python string, a
subprocess LIST, this gate's own pattern definition + docstring, and THIS self-test's own fixtures)
does NOT - proven by scanning the gate's own source AND this file's own source and asserting 0 hits.
stdlib-only. Exit 0 = all held; 1 = a self-test failed.
"""
import importlib.machinery
import importlib.util
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "check-repo-invariants"
_loader = importlib.machinery.SourceFileLoader("cri", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("cri", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def inv(id_: str):
    return next(i for i in m.INVARIANTS if i.id == id_)


def scan(id_: str, relpath: str, text: str):
    return inv(id_).scan(relpath, text)


# === (a) hardcoded colour outside design/tokens.css ===========================================
A = inv("a")
record("(a) design/tokens.css is the carved-out home (NOT in scope)", not A.in_scope("design/tokens.css"))
record("(a) src/ui/theme.css in scope", A.in_scope("src/ui/theme.css"))
record("(a) src/x.tsx in scope", A.in_scope("src/x.tsx"))
record("(a) docs/x.md NOT in scope (wrong ext)", not A.in_scope("docs/x.md"))
record("(a) src/x.test.ts NOT in scope (test)", not A.in_scope("src/x.test.ts"))
record("(a) #rrggbb colour flagged", len(scan("a", "src/x.css", "  color: #ff0000;")) == 1)
record("(a) #rgb + #rrggbbaa flagged", len(scan("a", "src/x.css", "a{color:#abc}\nb{color:#1a2b3c4d}")) == 2)
record("(a) a non-hex #word NOT flagged (#region)", len(scan("a", "src/x.ts", "// #region foo")) == 0)
record("(a) 0xFF00AA (no leading #) NOT flagged", len(scan("a", "src/x.ts", "const c = 0xFF00AA;")) == 0)

# === (b) process::Command::new outside crate::isolation =======================================
B = inv("b")
record("(b) std::process::Command::new flagged", len(scan("b", "src-tauri/src/x.rs", "std::process::Command::new(\"x\")")) == 1)
record("(b) tokio::process::Command::new flagged", len(scan("b", "src-tauri/src/x.rs", "tokio::process::Command::new(\"x\")")) == 1)
record("(b) bare `process::Command::new` flagged", len(scan("b", "src-tauri/src/x.rs", "let c = process::Command::new(p);")) == 1)
record("(b) a BARE Command::new NOT flagged (clap-builder carve; G29's import-resolving job)",
       len(scan("b", "src-tauri/src/x.rs", "let app = Command::new(\"cli\");")) == 0)
record("(b) src-tauri/src/exec.rs in scope", B.in_scope("src-tauri/src/exec.rs"))
record("(b) crate::isolation module carved out (isolation/mod.rs)", not B.in_scope("src-tauri/src/isolation/mod.rs"))
record("(b) crate::isolation.rs carved out", not B.in_scope("src-tauri/src/isolation.rs"))
record("(b) a .ts file NOT in scope", not B.in_scope("src/x.ts"))
record("(b) a test .rs NOT in scope", not B.in_scope("src-tauri/src/x.test.rs") and not B.in_scope("src-tauri/tests/x.rs"))

# === (c) unsafe impl Send/Sync outside the FFI module =========================================
C = inv("c")
record("(c) unsafe impl Send flagged", len(scan("c", "src-tauri/src/x.rs", "unsafe impl Send for W {}")) == 1)
record("(c) unsafe impl<T> Sync flagged", len(scan("c", "src-tauri/src/x.rs", "unsafe impl<T> Sync for W<T> {}")) == 1)
record("(c) `unsafe impl Display` (no Send/Sync) NOT flagged", len(scan("c", "src-tauri/src/x.rs", "unsafe impl Display for W {}")) == 0)
record("(c) a SAFE `impl Send` (no `unsafe`) NOT flagged", len(scan("c", "src-tauri/src/x.rs", "impl Send for W {}")) == 0)
record("(c) FFI module carved out (ffi/mod.rs)", not C.in_scope("src-tauri/src/ffi/mod.rs"))
record("(c) ffi.rs carved out", not C.in_scope("src-tauri/src/ffi.rs"))
record("(c) src-tauri/src/core.rs in scope", C.in_scope("src-tauri/src/core.rs"))

# === (d) raw localhost outside #[cfg(test)] (+ LibreOffice carve-out) =========================
D = inv("d")
record("(d) a 127.0.0.1 string literal in production .rs flagged",
       len(scan("d", "src-tauri/src/net.rs", "fn c() { let a = \"127.0.0.1\"; }")) == 1)
record("(d) localhost inside a \"http://localhost\" STRING flagged (the // is not a comment)",
       len(scan("d", "src/api.ts", "const u = \"http://localhost:1420\";")) == 1)
record("(d) localhost in a // COMMENT NOT flagged (literal invariant, not a prose ban)",
       len(scan("d", "src-tauri/src/net.rs", "// never bind to localhost in prod")) == 0)
record("(d) localhost in a /* block */ comment NOT flagged",
       len(scan("d", "src-tauri/src/net.rs", "/* localhost note */ let x = 1;")) == 0)
# #[cfg(test)] exemption + the println!("{}") format-brace confounder
RUST = (
    "fn connect() {\n"
    "    let prod = \"127.0.0.1\";\n"            # line 2: production -> FLAGGED
    "}\n"
    "#[cfg(test)]\n"
    "mod tests {\n"
    "    fn t() {\n"
    "        let s = \"127.0.0.1\";\n"           # inside cfg(test) -> exempt
    "        println!(\"{}\", s);\n"             # format brace must NOT break the block match
    "    }\n"
    "}\n"
    "fn after() { let again = \"localhost\"; }\n"  # line 11: AFTER the test block, production -> FLAGGED
)
d_hits = scan("d", "src-tauri/src/net.rs", RUST)
d_lines = sorted(ln for ln, _ in d_hits)
record("(d) #[cfg(test)] block exempt; production lines before AND after it flagged (brace-safe)",
       d_lines == [2, 11])
record("(d) cfg(test) exempt-line set covers the test module body", 7 in m._cfg_test_exempt_lines(RUST))
record("(d) LibreOffice bundle carve: bundle/LibreOffice/** excluded even for a .rs",
       not D.in_scope("bundle/LibreOffice/program/x.rs"))
record("(d) LibreOffice.app + lowercase libreoffice trees excluded",
       m._is_libreoffice_bundle("bundle/LibreOffice.app/x") and m._is_libreoffice_bundle("bundle/libreoffice/y"))
record("(d) a non-LibreOffice bundle path is NOT carved (would be scanned if first-party src)",
       not m._is_libreoffice_bundle("bundle/ffmpeg/x"))
record("(d) first-party src-tauri/src/x.rs in scope (banned, no exception)", D.in_scope("src-tauri/src/x.rs"))
record("(d) a .test.ts NOT in scope (test)", not D.in_scope("src/x.test.ts"))

# === (e) cargo vet update/sync/import in CI or scripts/ (LIVE) ================================
E = inv("e")
record("(e) .github/workflows/ci.yml in scope", E.in_scope(".github/workflows/ci.yml"))
record("(e) scripts/whatever in scope", E.in_scope("scripts/check-foo"))
record("(e) docs/*.md NOT in scope", not E.in_scope("docs/security/build-gates.md"))
record("(e) supply-chain/config.toml NOT in scope (its # comment mention is out of scope)",
       not E.in_scope("supply-chain/config.toml"))
# --- the REAL invocation vectors FAIL ---
record("(e) YAML `run: cargo vet update` FLAGGED (the realistic auto-refresh vector)",
       len(scan("e", ".github/workflows/x.yml", "      - run: cargo vet update")) == 1)
record("(e) YAML `cargo vet sync` / `cargo vet import` FLAGGED",
       len(scan("e", ".github/workflows/x.yml", "  - run: cargo vet sync\n  - run: cargo vet import mozilla")) == 2)
record("(e) YAML QUOTED `run: \"cargo vet update\"` FLAGGED (YAML strings are kept)",
       len(scan("e", ".github/workflows/x.yml", "    - run: \"cargo vet update\"")) == 1)
record("(e) a bare `cargo vet update` line in a scripts/*.sh FLAGGED",
       len(scan("e", "scripts/refresh.sh", "set -e\ncargo vet update\n")) == 1)
# --- PROSE does NOT fire ---
record("(e) a YAML `# cargo vet update` comment NOT flagged",
       len(scan("e", ".github/workflows/x.yml", "      # we used to cargo vet update here")) == 0)
record("(e) `cargo vet check` (read-only verb) NOT flagged",
       len(scan("e", ".github/workflows/x.yml", "      - run: cargo vet check")) == 0)
record("(e) `cargo-vet` (hyphenated tool name) NOT flagged",
       len(scan("e", "scripts/x.sh", "cargo-vet --version")) == 0)
record("(e) a Python STRING mention NOT flagged",
       len(scan("e", "scripts/check-x", "doc = \"see cargo vet update guide\"\n")) == 0)
record("(e) a Python TRIPLE-QUOTED docstring mention NOT flagged",
       len(scan("e", "scripts/check-x", "\"\"\"\nban cargo vet update / sync / import here\n\"\"\"\n")) == 0)
record("(e) a Python `# cargo vet update` comment NOT flagged",
       len(scan("e", "scripts/check-x", "# cargo vet update\n")) == 0)
record("(e) a subprocess LIST `[\"cargo\",\"vet\",\"update\"]` NOT flagged (not contiguous + blanked)",
       len(scan("e", "scripts/check-x", "subprocess.run([\"cargo\", \"vet\", \"update\"])\n")) == 0)
record("(e) a TOML `# comment` + string value mention NOT flagged",
       len(scan("e", "scripts/x.toml", "# no cargo vet update allowed\nk = \"cargo vet sync\"\n")) == 0)
# --- the LOAD-BEARING legs: the live gate must NOT self-match its own source or this self-test ---
GATE_SRC = SCRIPT.read_text(encoding="utf-8")
record("(e) the gate's OWN source (pattern def + docstring) yields 0 hits when scanned live",
       len(scan("e", "scripts/check-repo-invariants", GATE_SRC)) == 0)
SELFTEST_SRC = Path(__file__).read_text(encoding="utf-8")
record("(e) THIS self-test's OWN source (literal `cargo vet update` fixtures) yields 0 hits",
       len(scan("e", "scripts/gate-selftests/g24-repo-invariants.py", SELFTEST_SRC)) == 0)

# === (f) fc.gen( outside the approved shrink-wrapper ==========================================
F = inv("f")
record("(f) a .test.ts in scope", F.in_scope("src/x.test.ts"))
record("(f) the approved shrink-wrapper path carved out", not F.in_scope("tests/support/fc-arbitraries.ts"))
record("(f) a non-test .ts NOT in scope", not F.in_scope("src/x.ts"))
record("(f) `fc.gen(` flagged in a test file", len(scan("f", "src/x.test.ts", "const a = fc.gen(() => 1);")) == 1)
record("(f) `fc.gen (` with a space flagged", len(scan("f", "src/x.test.ts", "fc.gen ( g )")) == 1)
record("(f) `fc.sample(` (a different API) NOT flagged", len(scan("f", "src/x.test.ts", "fc.sample(arb)")) == 0)

# === helper-level confidence ==================================================================
record("_blank_code blanks a # comment + string but keeps bare code",
       "secret" not in m._blank_code("x = \"secret\"  # secret\ncargo vet update") and
       "cargo vet update" in m._blank_code("x = \"secret\"  # secret\ncargo vet update"))
record("_strip_command_comments keeps a quoted scalar, drops a # comment",
       m._strip_command_comments("run: \"a # b\"  # tail", is_yaml=True).strip() == "run: \"a # b\"")
record("_strip_command_comments is quote-aware ACROSS lines (# on a continuation of an open quote = data)",
       "cargo vet update" in m._strip_command_comments("run: \"open\n  b # c\" ; cargo vet update\n", is_yaml=True))
record("_logical_lines folds a more-indented YAML scalar body but NOT a same-indent sibling",
       any("cargo vet update" in t for _, t in m._logical_lines("run: cargo\n  vet update")) and
       not any("cargo vet update" in t for _, t in m._logical_lines("a: cargo\nb: vet update")))
record("_lex_blank(rust) keeps a string but blanks a // comment",
       "kept" in m._lex_blank("let s = \"kept\"; // gone", rust=True, blank_strings=False, blank_comments=True) and
       "gone" not in m._lex_blank("let s = \"kept\"; // gone", rust=True, blank_strings=False, blank_comments=True))
record("_lex_blank does NOT mistake a Rust lifetime 'a for a char/string",
       m._lex_blank("fn f<'a>(x: &'a str) {}", rust=True, blank_strings=True, blank_comments=True)
       == "fn f<'a>(x: &'a str) {}")

# === G1 review round-1 fixes (volle Härte) ====================================================
# --- P0: (e) a real `cargo vet update` SPLIT across physical lines must be caught ---
record("(e/P0) YAML folded `run: >-` split `cargo vet`/`update` FLAGGED (multiline reconstruction)",
       len(scan("e", ".github/workflows/x.yml", "      - run: >-\n          cargo vet\n          update\n")) == 1)
record("(e/P0) YAML plain multiline scalar `run: cargo`/`vet update` FLAGGED",
       len(scan("e", ".github/workflows/x.yml", "      - run: cargo\n          vet update\n")) == 1)
record("(e/P0) shell `\\`-line-continuation `cargo vet \\<nl>update` FLAGGED",
       len(scan("e", "scripts/refresh.sh", "set -e\ncargo vet \\\n    update\n")) == 1)
record("(e/P0) shell `eval \"cargo vet update\"` (string-wrapped) FLAGGED (shell = command-lang, strings kept)",
       len(scan("e", "scripts/refresh.sh", "eval \"cargo vet update\"\n")) == 1)
record("(e/P0) a multi-line COMMENT of the same split shape NOT flagged",
       len(scan("e", "scripts/refresh.sh", "# cargo vet \\\n# update\n")) == 0)
record("(e/P0) two UNRELATED lines (`echo cargo` / `vet update`) NOT joined into a false match",
       len(scan("e", ".github/workflows/x.yml", "      - run: echo cargo\n      - run: vet update\n")) == 0)
record("(e/P0) a YAML literal block `run: |` single-line invocation still FLAGGED",
       len(scan("e", ".github/workflows/x.yml", "      - run: |\n          cargo vet update\n")) == 1)

# --- P1: (d) #[cfg(not(test))] is NOT exempt; a non-block #[cfg(test)] does not leak ---
record("(d/P1) localhost under #[cfg(not(test))] FLAGGED (production-only cfg, NOT exempt)",
       len(scan("d", "src-tauri/src/s.rs", "#[cfg(not(test))]\nfn serve() { let a = \"127.0.0.1\"; }\n")) == 1)
record("(d/P1) localhost under #[cfg(all(not(test), unix))] FLAGGED",
       len(scan("d", "src-tauri/src/s.rs", "#[cfg(all(not(test), unix))]\nfn serve() { let a = \"127.0.0.1\"; }\n")) == 1)
record("(d/P1) #[cfg(any(test, feature=\"x\"))] still EXEMPT (a real test gate)",
       len(scan("d", "src-tauri/src/s.rs", "#[cfg(any(test, feature = \"x\"))]\nmod t { fn f(){ let a=\"127.0.0.1\"; } }\n")) == 0)
record("(d/P1) #[cfg(test)] on a `use` (non-block) then a PRODUCTION localhost FLAGGED (no pending-leak)",
       len(scan("d", "src-tauri/src/s.rs", "#[cfg(test)]\nuse mockito::Server;\nfn prod() { let e = \"127.0.0.1\"; }\n")) == 1)
record("(d/P1) #[cfg(test)] on a `const` (non-block) then a production localhost FLAGGED",
       len(scan("d", "src-tauri/src/s.rs", "#[cfg(test)]\nconst FIXTURE: &str = \"x\";\nfn prod() { let e = \"localhost\"; }\n")) == 1)
record("(d/P1) inline #[cfg(test)] mod tests {…} STILL exempt (regression guard)",
       len(scan("d", "src-tauri/src/s.rs", "#[cfg(test)]\nmod tests { fn t(){ let a=\"127.0.0.1\"; } }\nfn prod(){ let ok=1; }\n")) == 0)

# --- P1: (a)/(c) raw-text false-positives fixed (comment/string-aware) ---
record("(a/P1) `href=\"#fff\"` (anchor fragment, not a colour) NOT flagged",
       len(scan("a", "src/x.tsx", "<a href=\"#fff\">x</a>")) == 0)
record("(a/P1) `// fixes #1234` + `#abc123` SHA in a COMMENT NOT flagged",
       len(scan("a", "src/x.ts", "// fixes #1234 and see commit #abc123")) == 0)
record("(a/P1) a real `color: #ff0000` literal STILL flagged (string/value kept)",
       len(scan("a", "src/x.css", "color: #ff0000;")) == 1)
record("(a/P1) `url(#abc)` SVG fragment ref NOT flagged",
       len(scan("a", "src/x.css", "fill: url(#abc);")) == 0)
record("(c/P1) `unsafe impl<T: Send> Foo for W<T>` (Send is a BOUND, not the impl'd trait) NOT flagged",
       len(scan("c", "src-tauri/src/x.rs", "unsafe impl<T: Send> Foo for W<T> {}")) == 0)
record("(c/P1) `unsafe impl<T> Send for W<T>` (Send IS the impl'd trait) STILL flagged",
       len(scan("c", "src-tauri/src/x.rs", "unsafe impl<T> Send for W<T> {}")) == 1)
record("(c/P1) `unsafe impl<T: Iterator<Item=u8>> Sync for W` (nested generic) flagged",
       len(scan("c", "src-tauri/src/x.rs", "unsafe impl<T: Iterator<Item=u8>> Sync for W<T> {}")) == 1)
record("(c/P1) a `// unsafe impl Send` COMMENT NOT flagged (comment-blanked)",
       len(scan("c", "src-tauri/src/x.rs", "// unsafe impl Send for W {}")) == 0)
record("(b/P3) a `// std::process::Command::new` COMMENT NOT flagged (comment-blanked)",
       len(scan("b", "src-tauri/src/x.rs", "// avoid std::process::Command::new here")) == 0)

# --- P2: (f) scope broadened to .test.tsx + __tests__/ ---
record("(f/P2) a .test.tsx file in scope", inv("f").in_scope("src/c.test.tsx"))
record("(f/P2) a __tests__/ TS file in scope", inv("f").in_scope("src/__tests__/converters.ts"))
record("(f/P2) fc.gen( in a .test.tsx flagged", len(scan("f", "src/c.test.tsx", "const a = fc.gen(() => 1);")) == 1)
record("(f/P2) the wrapper path still carved even under __tests__-style naming", not inv("f").in_scope("tests/support/fc-arbitraries.ts"))

# === G1 re-review round-2 fixes (the round-1 multiline change introduced two NEW defects) =====
# --- P0: a real `cargo vet update` hidden behind a multiline-QUOTED scalar must STILL be caught
#         (the per-line comment-strip wrongly truncated the open quote's continuation line) ---
record("(e/P0-r2) shell: a `#` inside a multiline single-quote does NOT truncate a trailing `; cargo vet update`",
       len(scan("e", "scripts/r.sh", "echo 'banner one\nsecond # line' ; cargo vet update\n")) == 1)
record("(e/P0-r2) YAML: a `#` inside a multiline double-quoted scalar does NOT hide a trailing `; cargo vet update`",
       len(scan("e", ".github/workflows/x.yml", "      - run: \"echo 'a\n          b # c' ; cargo vet update\"\n")) == 1)
# --- P1: a cross-line FALSE-JOIN must NOT fire (unrelated sibling lines / a blanked comment line
#         must never fold `cargo vet` + a later standalone `update` into a phantom match) ---
record("(e/P1-r2) `cargo vet # verify` / `# refresh` / `update --force` (comment lines) NOT a false hit",
       len(scan("e", "scripts/r.sh", "cargo vet  # verify\n# refresh\nupdate --force\n")) == 0)
record("(e/P1-r2) `echo cargo` / `vet update` (same-indent siblings) NOT a false hit",
       len(scan("e", "scripts/r.sh", "echo cargo\nvet update\n")) == 0)
record("(e/P1-r2) bare `cargo` / `vet update` on separate same-indent lines NOT a false hit",
       len(scan("e", "scripts/r.sh", "cargo\nvet update\n")) == 0)
record("(e/P1-r2) regression: a GENUINE shell `\\`-continuation `cargo vet \\<nl>update` STILL flagged",
       len(scan("e", "scripts/r.sh", "cargo vet \\\n    update\n")) == 1)

# === G1 re-review round-3 fixes (the round-2 rewrite introduced two false-positives) ==========
# --- P1: a word-internal apostrophe in a YAML scalar (an English contraction) must NOT open a phantom
#         run-to-EOF quote that keeps a downstream prose `# … cargo vet … …` comment (a common CI edit).
#         (In SHELL a word-internal `'` is an UNBALANCED-quote SYNTAX ERROR, so the guard is YAML-only -
#         see the round-4 legs below; over-flagging invalid shell blocks no legitimate push.) ---
record("(e/P1-r3) YAML `name: it's CI` + a downstream prose `# … cargo vet update …` comment NOT flagged",
       len(scan("e", ".github/workflows/x.yml", "    - name: it's CI\n      # historically we ran cargo vet update by hand\n")) == 0)
record("(e/P1-r3) YAML `desc: hello it's me` + a downstream `# … cargo vet sync …` comment NOT flagged",
       len(scan("e", ".github/workflows/x.yml", "    desc: hello it's me\n    # do not cargo vet sync here\n")) == 0)
record("(e/P1-r3) YAML `note: don't run it` + a downstream `# … cargo vet import …` comment NOT flagged",
       len(scan("e", ".github/workflows/x.yml", "    note: don't run it\n    # never cargo vet import automatically\n")) == 0)
record("(e/P1-r3) REGRESSION: a GENUINE shell multiline single-quote hides nothing - trailing invocation FLAGGED",
       len(scan("e", "scripts/r.sh", "echo 'banner one\nsecond # line' ; cargo vet update\n")) == 1)
record("(e/P1-r3) REGRESSION: a normal single-line trailing `# comment` still strips (no false hit)",
       len(scan("e", "scripts/r.sh", "cargo vet check  # safe verb\n")) == 0)
# --- (my round-3 finding) shell is NOT indentation-sensitive: a cosmetically-indented next line is a
#     SEPARATE command, so `echo cargo` / `  vet update` must NOT false-join into a phantom match ---
record("(e/P1-r3) shell: a cosmetically-INDENTED next line is NOT a continuation (no phantom `cargo vet update`)",
       len(scan("e", "scripts/r.sh", "echo cargo\n  vet update\n")) == 0)
record("(e/P1-r3) REGRESSION: YAML indent-fold STILL works (a plain multiline scalar `run: cargo`/`  vet update`)",
       len(scan("e", ".github/workflows/x.yml", "    - run: cargo\n        vet update\n")) == 1)
record("(e/P1-r3) _logical_lines: fold_indent off (shell) does NOT join an indented line; on (YAML) does",
       not any("cargo vet update" in t for _, t in m._logical_lines("echo cargo\n  vet update", fold_indent=False))
       and any("cargo vet update" in t for _, t in m._logical_lines("run: cargo\n  vet update", fold_indent=True)))

# === G1 re-review round-4 fix (the round-3 apostrophe guard hid a real SHELL invocation) ======
# --- P0: in SHELL a quote opens word-adjacent (`X=a"b # c"` quotes `b # c`), so the `#` is DATA and a
#         trailing `; cargo vet update` after the closing quote EXECUTES and MUST be caught. The round-3
#         "not after a word char" guard wrongly suppressed the opener -> stripped the real invocation.
#         The guard is now YAML-only (shell quotes always open). ---
record("(e/P0-r4) shell `X=a\"b # c\" ; cargo vet update` (the `#` is QUOTED data) FLAGGED",
       len(scan("e", "scripts/r.sh", "X=a\"b # c\" ; cargo vet update\n")) == 1)
record("(e/P0-r4) shell `echo x'a # b' ; cargo vet update` (word-adjacent single-quote) FLAGGED",
       len(scan("e", "scripts/r.sh", "echo x'a # b' ; cargo vet update\n")) == 1)
record("(e/P0-r4) YAML `run: X=a\"b # c\" ; cargo vet update` is a YAML comment after `b` -> correctly NOT flagged",
       len(scan("e", ".github/workflows/x.yml", "    - run: X=a\"b # c\" ; cargo vet update\n")) == 0)
# --- P2: only an ODD number of trailing backslashes is a shell continuation (an even count is an
#         escaped literal backslash ending the command) ---
record("(e/P2-r4) shell: an EVEN (2) trailing-backslash does NOT fold (escaped literal backslash, not a continuation)",
       len(scan("e", "scripts/r.sh", "echo cargo vet \\\\\n update\n")) == 0)
record("(e/P2-r4) shell: a single (odd) trailing backslash STILL folds `cargo vet \\<nl>update`",
       len(scan("e", "scripts/r.sh", "cargo vet \\\n    update\n")) == 1)

# === live: the real repo is clean today =======================================================
record("main() exits 0 today (live (e) over real workflows+scripts clean; a-d,f target-absent)", m.main() == 0)

failed = [n for n, ok in results if not ok]
print(f"\n[g24-repo-invariants] {len(results) - len(failed)}/{len(results)} assertions passed.")
import sys
sys.exit(1 if failed else 0)
