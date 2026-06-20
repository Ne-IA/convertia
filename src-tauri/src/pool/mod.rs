//! `crate::pool` — the §0.9 bounded engine-subprocess pool: the single owner of the global
//! concurrency degree and the per-engine parallelism rules (LibreOffice serialised via a dedicated
//! single-permit semaphore), plus the timeout / no-progress-watchdog parameters. The interface shell
//! lands in P3.3; the real pool in P4.20.
