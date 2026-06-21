#!/usr/bin/env python3
"""g24-ci-supply-chain.py - G24 self-test for check-ci-supply-chain (P0.2.6).

Proves the G56 residual gate FAILS on each violation and PASSES a clean tree, using
throwaway temp .github/ fixture trees (invoking the real check with --root <tmp>).

Two leg families:
  * happy-path legs (the canonical 2-space block dialect the gate is written against);
  * ADVERSARIAL-DIALECT legs (F1-F6) - valid-but-exotic YAML a malicious/careless
    L(-1) `.github/**` diff could use to walk past a naive line scanner: quoted/dup
    write perms, comment-spoofed dependabot ecosystems, release/create polarity,
    flow-style + 4-space jobs, step-vs-job timeout, and `'on':`/scalar trigger keys.
    Each must FAIL; two robustness-positive legs prove a legitimate 4-space tree and a
    comment/run-string `id-token` mention do NOT over-fail.

stdlib-only. Exit 0 = every assertion held; 1 = a self-test assertion FAILED.
"""
import subprocess
import sys
import tempfile
from pathlib import Path

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


# ---------------------------------------------------------------------------
# happy-path legs (canonical 2-space block dialect)
# ---------------------------------------------------------------------------
leg("clean tree passes", "")
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
leg("R3c quoted 'id-token' key fails", "id-token", workflows={"ci.yml": (
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
    raw = wf.splitlines()
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

failed = [n for n, ok in results if not ok]
print(f"\n[g24-ci-supply-chain] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
