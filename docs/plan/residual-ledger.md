# Residual ledger

> What a box or a Co-Pilot act deliberately left outside its work at hand
> ([CLAUDE.md](../../CLAUDE.md) §6): every P2/P3 review finding it did not apply and every
> improvement outside its scope, plus residuals raised outside a commit (a CI run, an
> escalation, a sweep). The intake is each commit body's `Open P2/P3:` lines (build-loop
> Step 5); the Co-Pilot copies those lines here and triages them at the phase-end sweep
> ([test-strategy §11.2](../process/test-strategy.md)). The Build-Loop never edits this
> file and never builds from it; nothing here is a plan box.
>
> **Line format** — one plain bullet per residual, never a `[ ]` marker:
> `- <source> · <P2 | P3 | improvement> · <what, one line> · <file or §> · <outcome>`.
> `<source>` is the short SHA of the commit whose `Open P2/P3:` line raised it, the CI
> run id, `sweep` or `escalation` for one raised outside a commit, or `act` for one a
> Co-Pilot act records here in its own commit (`git log -- docs/plan/residual-ledger.md`
> names that commit). `<outcome>` is `open` until the sweep gives it exactly one of:
> `done <sha>` (mechanical and small; at most 10 per phase), `boxed P<n>.<m>` (a
> successor-phase box, with a `needs:` edge where it binds; one box may take several
> lines) or `declined: <reason>` (one line). A phase section is closed when no line reads
> `open`; triaged lines stay as the record.

## P4

- act · P3 · plan-lint check 1 is a registered no-op, while build-gates §6 item 1 describes an enforcing membership and matrix-parity check (format membership is G22's) · scripts/plan-lint check 1, build-gates §6 item 1 · open
- act · P3 · build-gates §6 item 6 names banned stamps, strikethrough and stale dates, but plan-lint check 6 enforces strikethrough only · scripts/plan-lint check 6, build-gates §6 item 6 · open
- act · improvement · gate scripts other than plan-lint still pass when a landed target vanishes (the target-absent branches of check-supply-chain's tauri-plugin scan, check-core-deps, check-csp-capabilities, check-corpus-integrity, check-fuzz-contract, check-js-supply-chain, check-lockfile-integrity, check-rs-test-refs, check-rust-lint-contract, check-deferral, check-sast, check-ts-gate, check-unsafe-policy, check-english-only and check-test-suppression; several only once every candidate target is gone); this act flipped plan-lint only · scripts/check-* · open
