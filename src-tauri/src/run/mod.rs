//! `crate::run` — the §2.6 per-run / per-instance scratch ownership + cleanup lifecycle, keyed on
//! `RunId` / `InstanceId` (§7.1): owned temp roots + names, the §2.6.3 startup orphan sweep, and
//! teardown. A §0.7 tier-2 trust-kernel LEAF: it depends DOWN only (on `crate::domain` for the
//! `RunId` / `InstanceId` / `ItemId` ids), never up on IPC / orchestrator / the engine registry (§2.0
//! dependency direction); it does NOT depend on `crate::fs_guard` to compile its root (the three
//! trust-kernel roots have no mutual dependency at scaffold time). Unsafe-free — the crate-root
//! `#![deny(unsafe_code)]` (main.rs) covers it; the §2.6.3 advisory-lock / try-lock FFI is homed in the
//! single allow-listed `crate::platform` shim (P3.21 / P3.23).
//!
//! ## P3.1.2 public-surface contract map — bodies authored by the named fill-boxes
//! [Build-Session-Entscheidung: P3.1.2] As in `crate::fs_guard` (P3.1.1), the surface is a documented
//! CONTRACT MAP, not callable bodies (the title's "function shells" = the public surface). Each
//! cleanup / sweep function does real filesystem work whose only honest value is the real one; a
//! permissive default would falsely claim "cleaned" / "swept", and a permissive `sweep_stale` could
//! remove a LIVE foreign temp (the §2.6.3 held-lock delete-gate the kernel exists to protect). No
//! run-owned temp even exists to clean ahead of the P3.20 naming model + the P3.21 lock-before-part
//! lifecycle, and no caller reaches these ahead of their fill-box (`cleanup_run` wires at P3.74, the
//! sweep at startup with P3.23). Signature AND body land together in each fill-box:
//!  - `cleanup_item` / `cleanup_run` — own-prefix-scoped cleanup on every exit path
//!    (`.convertia-<thisInstanceId>-<thisRunId>-*.part`, never a bare `*.part` glob, §2.6.2) — **P3.22**
//!    (the `CleanupResidue` honesty leg is **P3.25**).
//!  - `sweep_stale` — startup sweep, the held lock as the SOLE delete gate, non-blocking try-lock
//!    (§2.6.3) — **P3.23** (the opportunistic destination-resident `*.part` reclaim is **P3.24**).
//!  - the run-lifecycle ORDERING seam — the publish-temp naming + ownership model
//!    (`.convertia-<InstanceId>-<RunId>-<jobId>-<rand>.part`, on `final`'s volume, §2.6.1 / §2.14.1) is
//!    **P3.20**; the lock-before-part start ordering (mint `RunId` -> create `run-<RunId>/` -> OS-lock
//!    `.lock` -> only then the first `*.part`, the premise making "absent lock => dead => reclaimable"
//!    safe, §2.6.3) is **P3.21**. No ordering typestate is seated here — that shape is P3.21's own.
