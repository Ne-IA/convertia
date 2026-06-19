# ConvertIA — P0 Completion Record

> **The durable, tamper-evident proof that P0 exited green.** P0 — the bootstrap that
> creates the loop, the gate system, and the dual review that every later phase runs
> under — is "done" when the **first push to `main` whose L4 CI run completed green**
> lands. That proof must **outlive any single commit body** (r7: a commit message is
> overwritable; a tracked, committed file is an append-only record), so this file IS
> the record. It is **stubbed now** (P0.6.10) so `plan-lint` check 24 has a committed
> shape to validate against; the live values are filled in the **P0-exit-recording
> commit**.
>
> **Conflict order:** SSOT > spec > security/process docs > plan > code > conversation.

---

## Schema

> **Editor note — do not break check 24.** `plan-lint` check 24 scans this whole file
> for the **first** occurrence of the `run_url` field in its *colon-prefixed data form*
> and requires that occurrence to be a valid `…/actions/runs/<id>` URL. Keep that single
> occurrence the **Record line at the bottom**; everywhere else (this Schema included)
> refer to the field as the backticked `run_url` key **without a trailing colon**, so a
> prose mention can never shadow the data line and spuriously redden the gate.

This record carries three fields:

- **`run_url`** — the GitHub Actions run URL of the **first push to `main` whose L4 CI
  run completed green** (the P0 exit criterion). `plan-lint` check 24 asserts it matches
  the immutable Actions-run shape `https://github.com/Ne-IA/convertia/actions/runs/<id>`.
  Until P0 exits, the field holds the **placeholder run `0`**; the exit-recording commit
  replaces `0` with the real run id. (A non-URL placeholder marker is **not** an option —
  check 24 reddens any `run_url` value that is not an Actions-run URL, which is exactly
  why the placeholder is the pattern-valid run `0`, not a free-text token.)
- **`date`** — the P0-exit date (ISO 8601), filled at exit.
- **`box_state_at_exit`** — the box-state summary the [build-loop.md](build-loop.md) §9
  convergence report names — boxes completed + their commit SHAs + the consolidated
  `[!extern]` owner-action list — filled at exit.

**P0-exit obligation.** When the first green L4 run on `main` lands, the exit-recording
commit MUST (1) replace the placeholder run `0` with the real run id and (2) fill `date`
and `box_state_at_exit`. check 24 guards the `run_url` **shape if present** — it does not
assert presence/non-deletion, nor that the placeholder was replaced — so the
exit-recording is a deliberate fill-all-three step (and the P0-exit box should consider
extending check 24 to reject a still-`0` run once `box_state_at_exit` is filled). The
record's non-deletion is held by the L(-1) `docs/process/**` cage + its tracked-file
status, not by check 24.

---

## Record

run_url: https://github.com/Ne-IA/convertia/actions/runs/0
date: (filled in the P0-exit-recording commit)
box_state_at_exit: (filled in the P0-exit-recording commit)
