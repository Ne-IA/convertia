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
- act · improvement · gate scripts other than plan-lint still pass when a landed target vanishes (the target-absent branches of check-core-deps, check-csp-capabilities, check-corpus-integrity, check-fuzz-contract, check-lockfile-integrity, check-rs-test-refs, check-rust-lint-contract, check-deferral, check-sast, check-ts-gate, check-unsafe-policy, check-english-only and check-test-suppression; several only once every candidate target is gone); this act flipped plan-lint only · scripts/check-* · open
- act · P3 · the G8 row's first sentence names "a box-id or `[Build-Session-Entscheidung]`" as the suppressing marker, while `check-deferral` (`SUPPRESS_TAG_RE`) and the row's own later sentence accept only the `[Build-Session-Entscheidung: <box-id>]` tag · build-gates G8 row · open
- act · improvement · G71 audits every commit of a push against the tip's cage, so a caged edit earlier in the range needs no ack of its own when an owner-acked commit after it in the same push drops its pattern or declares its path a `[[loop_tool]]` escape, and a ratchet weakening earlier in the range needs none when a later owner-acked commit drops its `[[monotone]]` entry; auditing each commit against its parent's cage would close it · scripts/check-l-neg1-ack · open
- act · P3 · `security-concept §0` is cited as the pin-and-verify rule, but security-concept has no §0 (the rule is its §4 principles 5 and 10 and the build-gates §0 gate-toolchain bullet); plan-lint check 2 matches a § number against every spec, security and process heading, so a cite of a section its named doc lacks passes, and resolving a doc-qualified § against that doc would catch it; this act re-pointed the CLAUDE.md §7 cite only · .github/workflows/fuzz.yml, test-strategy.md, requirements-yamllint.txt, rust-toolchain.toml, scripts/check-js-supply-chain, scripts/gate-tools.toml, the P4.34 and P4.89 notes · open
- act · improvement · the skip notices of the supply-chain gates (check-supply-chain's "no Cargo workspace yet (lands in P1)" and "no Cargo.lock - skipping the tauri-plugin presence scan", check-js-supply-chain's "no pnpm-lock.yaml yet" and "no pnpm manifest yet") still print, while the §0.8 floor legs now fail the same run closed when the manifest or lockfile is missing; retiring those branches would make each gate's output match its verdict · scripts/check-supply-chain, scripts/check-js-supply-chain · open
