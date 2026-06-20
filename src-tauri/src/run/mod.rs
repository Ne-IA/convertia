//! `crate::run` — the §2.6 per-run / per-instance scratch ownership + cleanup lifecycle, keyed on
//! `RunId` / `InstanceId` (§7.1): owned temp roots, the §2.6.3 startup orphan sweep, and teardown.
//! Filled by P3.1.2.
