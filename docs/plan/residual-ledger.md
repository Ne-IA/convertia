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
> `<source>` is the short SHA of the commit whose `Open P2/P3:` line raised it, or the
> CI run id, `sweep` or `escalation` for one raised outside a commit. `<outcome>` is
> `open` until the sweep gives it exactly one of: `done <sha>` (mechanical and small; at
> most 10 per phase), `boxed P<n>.<m>` (a successor-phase box, with a `needs:` edge where
> it binds; one box may take several lines) or `declined: <reason>` (one line). A phase
> section is closed when no line reads `open`; triaged lines stay as the record.

## P4
