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
    p = subprocess.run([sys.executable, str(CHECK), "--root", td], capture_output=True, text=True)
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

failed = [n for n, ok in results if not ok]
print(f"\n[g24-ci-supply-chain] {len(results) - len(failed)}/{len(results)} assertions passed.")
sys.exit(1 if failed else 0)
