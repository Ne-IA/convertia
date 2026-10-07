#!/usr/bin/env python3
"""g24-record-action-pins.py - the G24/G10 canary over `scripts/record-action-pins` (P4.97; G56 leg (12)).

The recorder is a `[[loop_tool]]` escape (scripts/l-neg1-files.toml): uncaged, so an edit to it needs no owner-ack.
Its output - the caged scripts/action-pins.toml, every row reviewed at each action re-land - and leg (12) are the
trust root; this caged file drives the tool from OUTSIDE, so a recorder edit that changes its answer on one of the
shapes item 2 lists reds a caged file (the g24-fetch-engine-assets / g24-compile-engine-asset pattern):

1. RUN the tool's fixture-driven `--selftest` (no network: an injected HTTP function) under a CAPTURED stdout - the
   tool's `_record` prints each leg as it records it, so the stream is production-time evidence a post-hoc rewrite of
   `_results` cannot forge - and PIN the BLESSED LEG-NAME SET (`g24-record-action-pins.legs` via `_monotone_pin.py`:
   a removed or renamed leg reds, an added leg is reported; the Co-Pilot re-blesses at the phase-end sweep). No leg
   of the suite may skip: every leg runs on every platform.

2. Independent planted positives through the tool's PUBLIC seams with this canary's OWN inputs (never the suite's
   fixtures). The suite is uncaged and may be rewritten to print the blessed names alone, so the shapes listed below
   are pinned HERE, each by a fixture of this canary's own - those shapes and no other. The three modes the re-land
   and the sweep run - `--write`, `--check`, `--resolve-images` - are driven through the tool's own command line
   (`main`) with this canary's HTTP function in place of the tool's HTTPS one (offline), each leg asserting the rows
   as the checker reads them (the file `--write` wrote, parsed back) and the checker's own verdict. The pinned shapes:
   * agreement: a composite nesting two SHA pins, one a composite nesting a third - four rows, each nested list in
     order, and (12) green;
   * the nested list of a remote and of a local composite ((12) re-hashes a local action.yml but trusts the fields
     recorded from it), recorded whole and in order: two SHA pins, each repo nested once more by a tag (after its
     pin once, before it once), and a tag, a `docker://` image, a local path and a branch, the entries that are not
     SHA pins first, in the middle and last, a step's `uses:` on its `-` line and on the line below it; (12) reds
     each of the six that are not SHA pins by name;
   * images: scorecard-action v2.4.4's `runs.image` tag (action.yaml read, action.yml absent) and a local docker
     action's tag image, each recorded as written and red by (12); a Dockerfile's four external `FROM` refs -
     mutable, mutable, digest-pinned, mutable - recorded in order, (12) naming each mutable ref;
   * which metadata file: a path action (`owner/repo/path@sha`) read under its own directory - its metadata file and
     its Dockerfile, never the repo root's; action.yml read where action.yml and action.yaml both exist, remote and
     local;
   * `image_exempt_reason`: carried over for the same (uses, sha) only - never to another docker row, never across a
     re-pin;
   * `--check` (the only reproduction of a remote row): the file `--write` wrote for a composite, a Dockerfile and a
     `docker://` image row reproduces (exit 0), and a hand edit setting any of the five fields (12) trusts as
     recorded - `using`, `nested_uses`, `image`, `dockerfile_from`, `metadata_sha256` - to a value no row records
     (a fabricated pin or digest; `node20`) in the first, the middle or the last row differs (exit 1; fifteen
     edits, one at a time);
   * `--resolve-images` (the re-pin of a `docker://` image): a three-segment name's tag resolves through the
     anonymous Bearer round to the digest a registry of this canary's own serves; a reply without one is refused;
   * the byte and dialect bars on fetched third-party metadata: the digest covers the raw bytes of a CRLF file;
     refused - a `uses` key token the structural read did not see (a commented step), a flow `runs:`, the gate's
     dialect (a BOM, a quoted `uses` key, a U+2028 inside a comment), a lone CR, bytes that are not UTF-8, a file
     above the 1 MiB bound and an oversize reply through the tool's own network read (urllib's `urlopen` answered by
     this canary), a duplicate `runs.using` or `runs.image`, both metadata names absent, a status other than 200/404,
     a non-https URL; a refusal is exit 1 of `--write` and `--check` (never 0; `--write` leaves the committed file
     as it was);
   * the recorder fidelity catcher (a FAIL fed to the tool's `_record` is stored faithfully).
   Every other recorder behaviour - another input form, another fixture shape of these arms - is the recorder's own
   `--selftest` scope plus the Co-Pilot's review of every inventory row against the raw metadata lines the tool
   prints, at each action re-land (roles-and-escalation §5a); this canary does not claim to red a recorder edit that
   keeps every listed shape intact.

[Build-Session-Entscheidung: P4.97] a canary of its own, as scripts/l-neg1-files.toml asks of every `[[loop_tool]]` escape (a
g24-<tool>.py that pins the blessed leg names and drives planted positives from outside), rather than one
`--selftest` leg inside g24-ci-supply-chain.py.

COUPLING: this file is caged while record-action-pins is not, and it pins the blessed leg-name set; a removed or
renamed `--selftest` leg needs the owner-acked edit here, an added one only a re-bless at the sweep.

Run:  python3 -P scripts/gate-selftests/g24-record-action-pins.py   Exit 0 = every assertion held.
"""
import contextlib
import hashlib
import importlib.machinery
import importlib.util
import io
import subprocess
import sys
import tempfile
import tomllib
import urllib.request
from pathlib import Path

for _stream in (sys.stdout, sys.stderr):          # the console's codepage is not this script's concern (G9 invariant i)
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "record-action-pins"
CHECK = REPO / "scripts" / "check-ci-supply-chain"
# SourceFileLoader + module_from_spec with NO sys.modules entry (the tool keeps NamedTuples, never @dataclass).
_loader = importlib.machinery.SourceFileLoader("rap", str(SCRIPT))
m = importlib.util.module_from_spec(importlib.util.spec_from_loader("rap", _loader))
_loader.exec_module(m)

results: list[tuple[str, bool]] = []


def record(name: str, ok: bool) -> None:
    results.append((name, ok))
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")


def _probe(fn) -> bool:
    """A tool-touching predicate, fail-closed: a renamed attribute is a named FAIL, never a dead canary."""
    try:
        return bool(fn())
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        print(f"[g24-record-action-pins] predicate probe raised: {type(e).__name__}: {e}")
        return False


_RECORD_ERROR = getattr(m, "RecordError", None)


def _refused(fn) -> str:
    """The tool's own RecordError message fn raised, or "" (a generic exception is a named FAIL, not a refusal)."""
    try:
        fn()
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        if _RECORD_ERROR is not None and isinstance(e, _RECORD_ERROR):
            return str(e) or "<empty RecordError>"
        print(f"[g24-record-action-pins] WRONG exception {type(e).__name__}: {e}")
    return ""


try:
    _mp_loader = importlib.machinery.SourceFileLoader(
        "_monotone_pin", str(Path(__file__).resolve().parent / "_monotone_pin.py"))
    _mp = importlib.util.module_from_spec(importlib.util.spec_from_loader("_monotone_pin", _mp_loader))
    _mp_loader.exec_module(_mp)
except Exception as _mp_err:  # noqa: BLE001 - a missing or broken pin module is a NAMED FAIL below
    print(f"[g24] monotone pin module failed to load: {type(_mp_err).__name__}: {_mp_err}")
    _mp = None


def _pin_probe() -> tuple[list[str], list[str], list[str]]:
    if _mp is None:
        return [], ["<pin module unusable>"], []
    try:
        return _mp.check("record-action-pins", [n for n, _ok in m._results])
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        print(f"[g24-record-action-pins] monotone pin probe raised: {type(e).__name__}: {e}")
        return [], ["<blessed set unusable>"], []


# --- 1. the suite, its printed stream, the blessed leg-name set, no skips ------------------------------------------
print("[g24-record-action-pins] running record-action-pins --selftest ...")
_suite_out = io.StringIO()
try:
    with contextlib.redirect_stdout(_suite_out):
        rc = m.selftest()
    suite_crashed = ""
except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
    rc, suite_crashed = 1, f"{type(e).__name__}: {e}"
print(_suite_out.getvalue(), end="")
record("the tool's --selftest completed without an unhandled exception", not suite_crashed)
record("the tool's full --selftest suite passes under the canary runner", rc == 0)
_blessed, _pin_missing, _pin_unblessed = _pin_probe()
record("monotone pin: every blessed leg name (g24-record-action-pins.legs) is in the live run and the blessed set "
       "is non-empty - a removed or renamed tool leg reds, an absent or empty blessed set is a failing pin",
       _blessed != [] and _pin_missing == [])
if _pin_missing:
    print("[g24-record-action-pins] monotone pin: MISSING blessed leg(s): " + "; ".join(_pin_missing[:5]))
if _pin_unblessed:
    print(f"[g24-record-action-pins] monotone pin: {len(_pin_unblessed)} unblessed live leg(s) - reported, never "
          "failed (re-bless at the phase-end sweep): " + "; ".join(_pin_unblessed[:5]))
record("independent verdict: every recorded suite leg is green (read from _results, never the tool's aggregation)",
       _probe(lambda: m._results != [] and all(ok for _, ok in m._results)))
record("production-time evidence: the captured stream carries one [PASS] line per recorded leg and no [FAIL] line",
       _probe(lambda: _suite_out.getvalue().count("[PASS]") == len(m._results)) and "[FAIL]" not in _suite_out.getvalue())
record("skip inventory (strict): no suite leg skips - every leg runs on every platform",
       _probe(lambda: not any("(skipped" in name for name, _ok in m._results)))

# --- 2. independent planted positives (own inputs, public seams) ---------------------------------------------------
record("the tool still exposes RecordError (the refusal type every probe discriminates on)", _RECORD_ERROR is not None)
_A, _B, _C, _D = "a" * 40, "b" * 40, "c" * 40, "d" * 40
_NODE = b"name: canary\nruns:\n  using: node20\n  main: main.js\n"
_DIGEST_FROM = "docker.io/library/alpine@sha256:" + "c" * 64


def _docker(image: str) -> bytes:
    return f"name: d\nruns:\n  using: docker\n  image: {image}\n".encode()


def _composite(*entries: str) -> bytes:
    """A composite action nesting `entries` in order; every second step names itself first, so a nested `uses:` sits
    on a step's `-` line and on a line below it alike."""
    steps = "".join(f"    - uses: {e}\n" if i % 2 == 0 else f"    - name: s{i}\n      uses: {e}\n"
                    for i, e in enumerate(entries))
    return f"name: c\nruns:\n  using: composite\n  steps:\n{steps}".encode()


def _files(remote: dict[str, bytes], statuses: dict[str, int] | None = None):
    def http(method: str, url: str, headers: dict[str, str]):
        if statuses and url in statuses:
            return m.Response(statuses[url], {}, b"")
        return m.Response(200, {}, remote[url]) if url in remote else m.Response(404, {}, b"")
    return http


def _url(repo: str, sha: str, rel: str) -> str:
    return f"https://raw.githubusercontent.com/{repo}/{sha}/{rel}"


def _tree(td: str, *uses: str, files: dict[str, bytes] | None = None) -> Path:
    root = Path(td)
    (root / ".github" / "workflows").mkdir(parents=True)
    (root / ".github" / "dependabot.yml").write_text(
        "version: 2\nupdates:\n" + "".join(
            f'  - package-ecosystem: "{e}"\n    directory: "/"\n    schedule:\n      interval: "weekly"\n'
            for e in ("github-actions", "cargo", "npm", "pip")), encoding="utf-8")
    steps = "".join(f"      - uses: {u}\n" for u in uses)
    (root / ".github" / "workflows" / "ci.yml").write_text(
        "name: ci\non:\n  push:\n    branches: [main]\npermissions:\n  contents: read\nconcurrency:\n"
        "  group: ci-x\n  cancel-in-progress: true\njobs:\n  build:\n    runs-on: ubuntu-22.04\n"
        "    timeout-minutes: 10\n    steps:\n" + steps + "      - run: echo canary\n", encoding="utf-8")
    (root / "scripts").mkdir()
    for rel, data in (files or {}).items():
        (root / rel).parent.mkdir(parents=True, exist_ok=True)
        (root / rel).write_bytes(data)
    return root


def _cli(argv: list[str], http) -> tuple[int, str, str]:
    """The tool's own command line (`main`) with this canary's HTTP function in place of the tool's HTTPS one, so the
    modes the re-land and the sweep run (`--write`, `--check`, `--resolve-images`) are driven whole and offline:
    (exit, stdout, stderr). A tool without the `main` / `real_http` seam is a named FAIL (exit -1), never a network
    call; the tool's own function is put back afterwards (the oversize-reply leg hands in that function itself, so
    the tool's own network read runs, under a patched urllib `urlopen`)."""
    if not callable(getattr(m, "main", None)) or not callable(getattr(m, "real_http", None)):
        print("[g24-record-action-pins] the tool lost its main / real_http seam")
        return -1, "", ""
    tool_http, out, err = m.real_http, io.StringIO(), io.StringIO()
    m.real_http = http
    try:
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            rc = m.main(argv)
    except SystemExit as e:
        rc = e.code if isinstance(e.code, int) else -1
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        print(f"[g24-record-action-pins] main({argv[0]} ...) raised: {type(e).__name__}: {e}")
        rc = -1
    finally:
        m.real_http = tool_http
    return rc if isinstance(rc, int) else -1, out.getvalue(), err.getvalue()


def _rows(root: Path) -> list | None:
    """The inventory's rows as the checker reads them (the file parsed back); None when it is absent or unparsable."""
    try:
        rows = tomllib.loads((root / "scripts" / "action-pins.toml").read_bytes().decode("utf-8")).get("action", [])
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as e:
        print(f"[g24-record-action-pins] the written inventory is unreadable: {type(e).__name__}: {e}")
        return None
    return rows if isinstance(rows, list) else [rows]


def _write_and_check(root: Path, http) -> tuple[int, str, list]:
    """Record the tree's inventory through the tool's own `--write` (the re-land's write path), read it back and run
    the real checker over the tree: (exit, output, the rows parsed back). A write that fails or a file that does not
    parse is a named FAIL of the calling leg (exit -1), never a dead canary."""
    rc, out, err = _cli(["--write", "--root", str(root)], http)
    rows = _rows(root) if rc == 0 else None
    if rows is None:
        print(f"[g24-record-action-pins] --write exited {rc}: {(out + err).strip()[-300:]}")
        return -1, "", []
    p = subprocess.run([sys.executable, "-P", str(CHECK), "--root", str(root)], capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    return p.returncode, p.stdout + p.stderr, rows


def _view(rows: list, *keys: str) -> list:
    """The rows' named fields in file order, a list field as a tuple; a row that is not a table reads as None."""
    return [tuple(tuple(r.get(k)) if isinstance(r.get(k), list) else r.get(k) for k in keys)
            if isinstance(r, dict) else None for r in rows]


with tempfile.TemporaryDirectory() as td:
    rc12, out12, rows12 = _write_and_check(
        _tree(td, f"canary/comp@{_A}"),
        _files({_url("canary/comp", _A, "action.yml"): _composite(f"canary/leaf@{_B}", f"canary/mid@{_C}"),
                _url("canary/mid", _C, "action.yml"): _composite(f"canary/deep@{_D}"),
                _url("canary/leaf", _B, "action.yml"): _NODE, _url("canary/deep", _D, "action.yml"): _NODE}))
    record("recorder and checker agree: a composite nesting two SHA pins, one a composite nesting a third, records "
           "four rows with each nested list in order, and they pass (12)",
           rc12 == 0 and "+ 4 action-pin row(s)" in out12
           and _view(rows12, "uses", "sha", "nested_uses") == [
               ("canary/comp", _A, (f"canary/leaf@{_B}", f"canary/mid@{_C}")), ("canary/deep", _D, ()),
               ("canary/leaf", _B, ()), ("canary/mid", _C, (f"canary/deep@{_D}",))])
# one nested list for both arms: the entries that are not SHA pins sit first, in the middle and last, around two pins,
# and each pinned repo is nested once more by a tag - after its pin once, before it once
_NESTED = ("canary/t1@v1", f"canary/leaf@{_B}", "canary/leaf@v3", "docker://ghcr.io/canary/x:1",
           "./.github/actions/y", "other/mid@v2", f"other/mid@{_C}", "other/br@main")
_NOT_PINS = ("canary/t1@v1", "canary/leaf@v3", "docker://ghcr.io/canary/x:1", "./.github/actions/y",
             "other/mid@v2", "other/br@main")
_NESTED_PINS = {_url("canary/leaf", _B, "action.yml"): _NODE, _url("other/mid", _C, "action.yml"): _NODE}
with tempfile.TemporaryDirectory() as td:
    rc12, out12, rows12 = _write_and_check(
        _tree(td, f"canary/comp@{_A}"),
        _files({_url("canary/comp", _A, "action.yml"): _composite(*_NESTED), **_NESTED_PINS}))
    record("the remote arm records a composite's whole nested list in order - two SHA pins, a tag of each pinned repo "
           "after and before its pin, a tag, a branch, a docker:// image and a local path, first, between and last - "
           "and (12) reds each entry that is not a SHA pin",
           rc12 == 1 and _view(rows12, "uses", "nested_uses") == [("canary/comp", _NESTED), ("canary/leaf", ()),
                                                                 ("other/mid", ())]
           and all(f"canary/comp@{_A} nests `uses: {e}`" in out12 for e in _NOT_PINS))
with tempfile.TemporaryDirectory() as td:
    rc12, out12, rows12 = _write_and_check(
        _tree(td, "./.github/actions/comp", "./.github/actions/img",
              files={".github/actions/comp/action.yml": _composite(*_NESTED),
                     ".github/actions/img/action.yml": _docker("docker://ghcr.io/canary/img:1")}),
        _files(_NESTED_PINS))
    record("the local arm records a local composite's whole nested list in order (the remote arm's list) and a local "
           "docker action's tag image as written ((12) re-hashes a local action.yml but trusts the fields recorded "
           "from it), and (12) reds each",
           rc12 == 1 and _view(rows12, "uses", "nested_uses", "image") == [
               ("./.github/actions/comp", _NESTED, ""), ("./.github/actions/img", (), "docker://ghcr.io/canary/img:1"),
               ("canary/leaf", (), ""), ("other/mid", (), "")]
           and all(f"./.github/actions/comp@(local) nests `uses: {e}`" in out12 for e in _NOT_PINS)
           and "./.github/actions/img@(local) runs ghcr.io/canary/img:1 by a mutable reference" in out12)
with tempfile.TemporaryDirectory() as td:
    scorecard = b'name: s\nruns:\n  using: "docker"\n  image: "docker://ghcr.io/ossf/scorecard-action:v2.4.4"\n'
    rc12, out12, rows12 = _write_and_check(_tree(td, f"ossf/scorecard-action@{_A}"),
                                           _files({_url("ossf/scorecard-action", _A, "action.yaml"): scorecard}))
    record("the scorecard-action v2.4.4 replay through the recorder: action.yml absent, action.yaml read and named, "
           "the mutable runs.image recorded as written, and (12) reds it",
           rc12 == 1 and _view(rows12, "metadata_file", "metadata_sha256", "using", "image") == [
               ("action.yaml", hashlib.sha256(scorecard).hexdigest(), "docker",
                "docker://ghcr.io/ossf/scorecard-action:v2.4.4")]
           and f"ossf/scorecard-action@{_A} runs ghcr.io/ossf/scorecard-action:v2.4.4 by a mutable reference" in out12)
with tempfile.TemporaryDirectory() as td:
    dockerfile = (b"FROM debian:bookworm AS base\nRUN true\nFROM ghcr.io/canary/build:1\nFROM "
                  + _DIGEST_FROM.encode() + b" AS mid\nFROM ubuntu:24.04\n")
    rc12, out12, rows12 = _write_and_check(_tree(td, f"canary/dk@{_A}"),
                                           _files({_url("canary/dk", _A, "action.yml"): _docker("Dockerfile"),
                                                   _url("canary/dk", _A, "Dockerfile"): dockerfile}))
    record("the Dockerfile arm records every external FROM in order - mutable refs before and after a digest-pinned "
           "one - and (12) names each mutable ref",
           rc12 == 1 and _view(rows12, "dockerfile_from") == [
               (("debian:bookworm", "ghcr.io/canary/build:1", _DIGEST_FROM, "ubuntu:24.04"),)]
           and f"canary/dk@{_A} runs debian:bookworm, ghcr.io/canary/build:1, ubuntu:24.04 by a mutable reference"
           in out12)
with tempfile.TemporaryDirectory() as td:
    scan_meta, build_meta = _docker("docker://ghcr.io/canary/scan:latest"), _docker("Dockerfile")
    rc12, out12, rows12 = _write_and_check(
        _tree(td, f"canary/tools/sub/scan@{_A}", f"canary/tools/build@{_A}"),
        _files({_url("canary/tools", _A, "action.yml"): _NODE,
                _url("canary/tools", _A, "Dockerfile"): b"FROM " + _DIGEST_FROM.encode() + b"\n",
                _url("canary/tools", _A, "sub/scan/action.yml"): scan_meta,
                _url("canary/tools", _A, "build/action.yml"): build_meta,
                _url("canary/tools", _A, "build/Dockerfile"): b"FROM debian:bookworm\n"}))
    record("a path action (owner/repo/path@sha) is read under its own directory - its metadata file and its "
           "Dockerfile, never the repo root's action or Dockerfile - and (12) reds what it runs",
           rc12 == 1
           and _view(rows12, "uses", "metadata_file", "metadata_sha256", "using", "image", "dockerfile_from") == [
               ("canary/tools/build", "build/action.yml", hashlib.sha256(build_meta).hexdigest(), "docker",
                "Dockerfile", ("debian:bookworm",)),
               ("canary/tools/sub/scan", "sub/scan/action.yml", hashlib.sha256(scan_meta).hexdigest(), "docker",
                "docker://ghcr.io/canary/scan:latest", ())]
           and f"canary/tools/build@{_A} runs debian:bookworm by a mutable reference" in out12
           and f"canary/tools/sub/scan@{_A} runs ghcr.io/canary/scan:latest by a mutable reference" in out12)
with tempfile.TemporaryDirectory() as td:
    yml_remote, yml_local = _docker("docker://ghcr.io/canary/both:1"), _docker("docker://ghcr.io/canary/lboth:1")
    rc12, out12, rows12 = _write_and_check(
        _tree(td, f"canary/both@{_A}", "./.github/actions/both",
              files={".github/actions/both/action.yml": yml_local, ".github/actions/both/action.yaml": _NODE}),
        _files({_url("canary/both", _A, "action.yml"): yml_remote, _url("canary/both", _A, "action.yaml"): _NODE}))
    record("where action.yml and action.yaml both exist, action.yml is the one read (GitHub's order), remote and local "
           "alike, and (12) reds what it runs",
           rc12 == 1 and _view(rows12, "uses", "metadata_file", "metadata_sha256", "image") == [
               ("./.github/actions/both", ".github/actions/both/action.yml", hashlib.sha256(yml_local).hexdigest(),
                "docker://ghcr.io/canary/lboth:1"),
               ("canary/both", "action.yml", hashlib.sha256(yml_remote).hexdigest(), "docker://ghcr.io/canary/both:1")]
           and "./.github/actions/both@(local) runs ghcr.io/canary/lboth:1 by a mutable reference" in out12
           and f"canary/both@{_A} runs ghcr.io/canary/both:1 by a mutable reference" in out12)
with tempfile.TemporaryDirectory() as td:
    crlf = _NODE.replace(b"\n", b"\r\n")
    crlf_root = _tree(td, f"canary/crlf@{_A}")
    record("the digest covers the RAW bytes of a CRLF metadata file (CRs kept)",
           _probe(lambda: [r.metadata_sha256 for r in m.record_inventory(
               crlf_root, _files({_url("canary/crlf", _A, "action.yml"): crlf})).rows] == [hashlib.sha256(crlf).hexdigest()]))
for title, data, needle in (
    ("a `uses` key token the structural read did not see (a commented step) is refused",
     _NODE + b"#    - uses: canary/evil@v1\n", "key token(s)"),
    ("a flow `runs:` mapping is refused", b"name: f\nruns: {using: node20, main: main.js}\n", "inline or flow"),
    ("a BOM in fetched metadata is refused (the gate's dialect)", b"\xef\xbb\xbf" + _NODE, "dialect"),
    ("a lone CR in fetched metadata is refused", _NODE.replace(b"\n", b"\r", 1), "lone CR"),
    ("fetched metadata that is not UTF-8 is refused, never read with replacement characters",
     _NODE.replace(b"canary", b"can\xffary"), "not UTF-8"),
    ("fetched metadata above the 1 MiB bound is refused, never truncated", _NODE + b"#" * (1 << 20), "larger than"),
    ("a quoted `uses` key (the token regex does not see it, GitHub's parser does) is refused by the gate's dialect",
     b"name: c\nruns:\n  using: composite\n  steps:\n    - \"uses\": canary/evil@v1\n", "dialect"),
    ("a U+2028 line break inside a comment (a real line to a YAML 1.1 reader) is refused by the gate's dialect",
     "name: c\nruns:\n  using: composite\n  steps:\n    - run: echo hi  # a\u2028    - \"uses\": canary/evil@v1\n"
     .encode(), "dialect"),
    ("a duplicate `runs.using` is refused, never read as its first value (a node row hiding a docker image)",
     b"name: d\nruns:\n  using: node20\n  using: docker\n  image: docker://ghcr.io/canary/x:1\n", "`runs.using` appears 2"),
    ("a duplicate docker `runs.image` is refused, never read as its first value (a digest hiding a tag)",
     b"name: d\nruns:\n  using: docker\n  image: docker://ghcr.io/canary/x@sha256:" + b"d" * 64
     + b"\n  image: docker://ghcr.io/canary/x:1\n", "`runs.image` appears 2"),
):
    with tempfile.TemporaryDirectory() as td:
        msg = _refused(lambda: m.record_inventory(_tree(td, f"canary/x@{_A}"), _files({_url("canary/x", _A, "action.yml"): data})))
        record(title, needle in msg)
with tempfile.TemporaryDirectory() as td:
    msg = _refused(lambda: m.record_inventory(_tree(td, f"canary/none@{_A}"), _files({})))
    record("both metadata names absent (404, 404) is refused, never an empty row", "both 404" in msg)
with tempfile.TemporaryDirectory() as td:
    msg = _refused(lambda: m.record_inventory(_tree(td, f"canary/x@{_A}"),
                                              _files({}, {_url("canary/x", _A, "action.yml"): 403})))
    record("a status other than 200/404 is refused, never read as absent", "HTTP 403" in msg)
with tempfile.TemporaryDirectory() as td:
    refused_root = _tree(td, f"canary/x@{_A}")
    refused_http = _files({_url("canary/x", _A, "action.yml"): b"name: f\nruns: {using: node20, main: main.js}\n"})
    (refused_root / "scripts" / "action-pins.toml").write_bytes(b"schema_version = 1\n")
    rc_w, _out_w, err_w = _cli(["--write", "--root", str(refused_root)], refused_http)
    rc_c, _out_c, err_c = _cli(["--check", "--root", str(refused_root)], refused_http)
    record("--write and --check exit 1 on a refusal, naming it (REFUSED) - never 0 - and --write leaves the "
           "committed file as it was",
           rc_w == 1 and "REFUSED" in err_w and "inline or flow" in err_w
           and rc_c == 1 and "REFUSED" in err_c and "inline or flow" in err_c
           and (refused_root / "scripts" / "action-pins.toml").read_bytes() == b"schema_version = 1\n")
with tempfile.TemporaryDirectory() as td, tempfile.TemporaryDirectory() as td2:
    carry_http = _files({_url("ossf/sc", _A, "action.yml"): _docker("docker://ghcr.io/o/s:1"),
                         _url("ossf/sc", _B, "action.yml"): _docker("docker://ghcr.io/o/s:1"),
                         _url("ossf/other", _A, "action.yml"): _docker("docker://ghcr.io/o/other:1")})
    carry_root = _tree(td, f"ossf/sc@{_A}", f"ossf/other@{_A}")
    repin_root = _tree(td2, f"ossf/sc@{_B}", f"ossf/other@{_A}")

    def _carried() -> tuple[list, list, int, str]:
        """`--write` over a committed file whose ossf/sc@A row alone carries a reason (set by hand, as the owner sets
        it): (the reasons it writes, read back; the reasons it writes after ossf/sc's re-pin to B; the checker's exit
        and output over the first)."""
        if _cli(["--write", "--root", str(carry_root)], carry_http)[0] != 0:
            return [], [], -1, ""
        blocks = (carry_root / "scripts" / "action-pins.toml").read_bytes().decode("utf-8").split("[[action]]")
        if sum('uses = "ossf/sc"\n' in b for b in blocks) != 1:
            return [], [], -1, ""
        committed = "[[action]]".join(b.replace('image_exempt_reason = ""', 'image_exempt_reason = "kept"')
                                      if 'uses = "ossf/sc"\n' in b else b for b in blocks).encode("utf-8")
        for root in (carry_root, repin_root):
            (root / "scripts" / "action-pins.toml").write_bytes(committed)
        repinned = (_view(_rows(repin_root) or [], "uses", "sha", "image_exempt_reason")
                    if _cli(["--write", "--root", str(repin_root)], carry_http)[0] == 0 else [])
        rc, out, kept = _write_and_check(carry_root, carry_http)
        return _view(kept, "uses", "sha", "image_exempt_reason"), repinned, rc, out

    try:
        kept12, repinned12, rc12, out12 = _carried()
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        print(f"[g24-record-action-pins] the carry-over probe raised: {type(e).__name__}: {e}")
        kept12, repinned12, rc12, out12 = [], [], -1, ""
    record("--write carries a committed image_exempt_reason over for the same (uses, sha) only - never to another "
           "docker row, never across a re-pin - and (12) still reds the row that has none",
           kept12 == [("ossf/other", _A, ""), ("ossf/sc", _A, "kept")]
           and repinned12 == [("ossf/other", _A, ""), ("ossf/sc", _B, "")]
           and rc12 == 1 and f"ossf/other@{_A} runs ghcr.io/o/other:1 by a mutable reference" in out12
           and f"ossf/sc@{_A}" not in out12)
with tempfile.TemporaryDirectory() as td:
    check_root = _tree(td, f"canary/comp@{_A}", f"canary/dk@{_A}", f"canary/img@{_A}")
    check_http = _files({
        _url("canary/comp", _A, "action.yml"): _composite("canary/t1@v1", "docker://ghcr.io/canary/x:1"),
        _url("canary/dk", _A, "action.yml"): _docker("Dockerfile"),
        _url("canary/dk", _A, "Dockerfile"): b"FROM debian:bookworm\n",
        _url("canary/img", _A, "action.yml"): _docker("docker://ghcr.io/canary/img:1")})
    # the five fields (12) trusts as recorded, each hand-set to a value no row of this tree records: `node20`, then a
    # fabricated nested SHA pin, image digest, FROM digest and file digest
    _PLANTED = (("using", '"node20"'), ("nested_uses", f'["canary/t1@{_D}"]'),
                ("image", '"docker://ghcr.io/canary/img@sha256:' + "e" * 64 + '"'),
                ("dockerfile_from", '["docker.io/library/debian@sha256:' + "e" * 64 + '"]'),
                ("metadata_sha256", '"' + "e" * 64 + '"'))

    def _check_verdicts() -> tuple[int, list, dict]:
        """`--check` over the file `--write` wrote for three rows - a composite, a Dockerfile and a `docker://` image
        row: the first, the middle and the last - then over fifteen hand edits of it, one at a time, each `_PLANTED`
        field set in each row: (the exit as written, the rows read back, {(row index, field): exit}); -1 = a write
        that failed, or an edit that does not hit exactly one line of its row or leaves that line as it was."""
        pins = check_root / "scripts" / "action-pins.toml"
        if _cli(["--write", "--root", str(check_root)], check_http)[0] != 0:
            return -1, [], {}
        text = pins.read_bytes().decode("utf-8")
        rows = _rows(check_root) or []
        as_written = _cli(["--check", "--root", str(check_root)], check_http)[0]
        lines = text.split("\n")
        starts = [i for i, ln in enumerate(lines) if ln == "[[action]]"] + [len(lines)]
        verdicts: dict[tuple[int, str], int] = {}
        for row in range(len(starts) - 1):
            span = range(starts[row], starts[row + 1])
            for field, planted in _PLANTED:
                hits = [i for i in span if lines[i].startswith(f"{field} = ")]
                edited = list(lines)
                if len(hits) == 1:
                    edited[hits[0]] = f"{field} = {planted}"
                if len(hits) != 1 or edited == lines:
                    verdicts[(row, field)] = -1
                    continue
                pins.write_bytes("\n".join(edited).encode("utf-8"))
                verdicts[(row, field)] = _cli(["--check", "--root", str(check_root)], check_http)[0]
        pins.write_bytes(text.encode("utf-8"))
        return as_written, rows, verdicts

    try:
        as_written, check_rows, verdicts = _check_verdicts()
    except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
        print(f"[g24-record-action-pins] the --check probe raised: {type(e).__name__}: {e}")
        as_written, check_rows, verdicts = -1, [], {}
    record("--check (the sweep's reproduction) from outside: the file --write wrote for a composite, a Dockerfile and "
           "a docker:// image row reproduces (exit 0), and a hand edit setting using, nested_uses, image, "
           "dockerfile_from or metadata_sha256 to a value no row records in the first, the middle or the last row "
           "differs (exit 1, each of the fifteen edits)",
           as_written == 0
           and _view(check_rows, "uses", "using", "nested_uses", "image", "dockerfile_from") == [
               ("canary/comp", "composite", ("canary/t1@v1", "docker://ghcr.io/canary/x:1"), "", ()),
               ("canary/dk", "docker", (), "Dockerfile", ("debian:bookworm",)),
               ("canary/img", "docker", (), "docker://ghcr.io/canary/img:1", ())]
           and len(verdicts) == 15 and all(v == 1 for v in verdicts.values()))
    if as_written != 0 or len(verdicts) != 15 or any(v != 1 for v in verdicts.values()):
        print(f"[g24-record-action-pins] --check exits: as written {as_written}; every edit that did not exit 1: "
              + ("; ".join(f"row {r} {f} -> {v}" for (r, f), v in verdicts.items() if v != 1) or "none"))
_IMG_DIGEST = "sha256:" + hashlib.sha256(b"canary image manifest").hexdigest()
_IMG_REF = "docker://registry.canary.example/canary/sub/img:2.0"
_heads: list[str] = []


def _registry(method: str, url: str, headers: dict[str, str]):
    """A registry of this canary's own: a 401 Bearer challenge, an anonymous token, then the manifest digest."""
    if url.startswith("https://auth.canary.example/token?"):
        return m.Response(200, {}, b'{"token": "canary-token"}')
    if headers.get("Authorization") != "Bearer canary-token":
        return m.Response(401, {"www-authenticate": 'Bearer realm="https://auth.canary.example/token",'
                                                     'service="registry.canary.example",'
                                                     'scope="repository:canary/sub/img:pull"'}, b"")
    _heads.append(f"{method} {url}")
    return m.Response(200, {"docker-content-digest": _IMG_DIGEST}, b"")


_rc_img, _out_img, _err_img = _cli(["--resolve-images", _IMG_REF], _registry)
_rc_bare, _out_bare, _err_bare = _cli(["--resolve-images", _IMG_REF], lambda _m, _u, _h: m.Response(200, {}, b""))
record("--resolve-images (the sweep's re-pin) from outside: a three-segment name's tag resolves through the "
       "anonymous Bearer round to the digest its manifest HEAD answers, printed as the reference to write, and a "
       "reply without a digest is refused (exit 1)",
       _rc_img == 0 and _out_img == f"docker://registry.canary.example/canary/sub/img@{_IMG_DIGEST}  # 2.0\n"
       and _heads == ["HEAD https://registry.canary.example/v2/canary/sub/img/manifests/2.0"]
       and _rc_bare == 1 and "Docker-Content-Digest" in _err_bare)
record("the real HTTP function refuses a non-https URL", "non-https" in _refused(lambda: m.real_http("GET", "http://canary.invalid/", {})))


class _Reply:
    """A urlopen reply of this canary's own: `read(n)` honours its bound, as an HTTP response does."""

    status = 200
    headers: dict[str, str] = {}

    def __init__(self, body: bytes) -> None:
        self._body = io.BytesIO(body)

    def read(self, n: int = -1) -> bytes:
        return self._body.read(n)

    def __enter__(self):
        return self

    def __exit__(self, *_exc) -> bool:
        return False


def _oversize_write() -> tuple[int, str, bool]:
    """`--write` through the tool's OWN network read (its `real_http` handed to `_cli`, so it stays in place), urllib's
    `urlopen` answering every request with a 200 reply of 1 MiB + 51 bytes (`_NODE`, then 1 MiB of comment): (exit,
    stderr, whether an inventory file was written). The real `urlopen` is put back afterwards."""
    real_urlopen = urllib.request.urlopen
    urllib.request.urlopen = lambda *_args, **_kwargs: _Reply(_NODE + b"#" * (1 << 20))
    try:
        with tempfile.TemporaryDirectory() as td:
            root = _tree(td, f"canary/x@{_A}")
            rc, _out, err = _cli(["--write", "--root", str(root)], getattr(m, "real_http", None))
            return rc, err, (root / "scripts" / "action-pins.toml").exists()
    finally:
        urllib.request.urlopen = real_urlopen


try:
    _rc_big, _err_big, _wrote_big = _oversize_write()
except Exception as e:  # noqa: BLE001 - a named FAIL beats a dead canary
    print(f"[g24-record-action-pins] the oversize-reply probe raised: {type(e).__name__}: {e}")
    _rc_big, _err_big, _wrote_big = -1, "", True
record("an oversize reply through the tool's own network read is refused, never truncated to the bound: --write "
       "exits 1 naming it (REFUSED, larger than) and writes no inventory",
       _rc_big == 1 and "REFUSED" in _err_big and "larger than" in _err_big and not _wrote_big)

_PROBE_NAME = "g24 recorder-fidelity probe (a deliberate FAIL entry; not a suite failure)"
record("escalation catcher: the tool's recorder stores a FAIL faithfully (a force-green _record reds here)",
       _probe(lambda: (m._record(_PROBE_NAME, False) or True) and m._results[-1] == (_PROBE_NAME, False)))

failed = [n for n, ok in results if not ok]
print(f"\n[g24-record-action-pins] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
