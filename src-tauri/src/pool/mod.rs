//! `crate::pool` — the §0.9 bounded engine-subprocess pool: the single owner of the global concurrency
//! degree and (from P4) the per-engine parallelism rules (LibreOffice serialised via a dedicated
//! single-permit semaphore) + the timeout / no-progress-watchdog parameters. A §0.7 **tier-3 leaf** — it
//! depends DOWN only, on `std` + the `tokio` runtime primitives, and names **no** tier-2 type. Unsafe-free
//! (the crate-root `#![deny(unsafe_code)]` in `main.rs` covers it); `tokio::sync::Semaphore` +
//! `tokio::task::spawn_blocking` add no FFI and no sockets, so the G29 rule (g)/(j) socket ban + the §3
//! zero-egress rule hold.
//!
//! ## LIVE in P3.3 (real, tested)
//!  - [`Pool`] carries the **global-degree** permit model: an `Arc<Semaphore>` sized to
//!    `clamp(available_parallelism − 1, 1, 4)` (§0.9; see the `available_parallelism` note on
//!    [`resolve_global_degree`]), the bound every engine job acquires.
//!  - [`Pool::run_in_core`] is the **`spawn_blocking`-style in-core worker-thread lane** the sole
//!    `EngineProgram::InProcessNative` engine (native CSV/TSV, §3.5.6) runs on: it acquires ONE
//!    global-degree permit, runs a synchronous closure on a dedicated `spawn_blocking` worker thread so the
//!    CSV loop **never blocks** the Tokio runtime that drives the subprocess engines + IPC (§0.9
//!    native-CSV/TSV row / §1.7 concurrency-permit model), and releases the permit on completion, on a
//!    worker panic (§0.9 panic isolation — a caught panic maps to a clean [`LaneError::Panicked`], never
//!    poisoning the pool), and on abandonment (the §1.7 timeout drops the lane future → the permit frees
//!    while the detached blocking thread parks in the pool's headroom, the §1.7 wedged-read bound). This
//!    engine holds a global-degree permit like any other job and has **no** `serialised_only` lane (§1.7).
//!
//! The lane is engine-agnostic on purpose: the §1.7 `mpsc::Sender<f32>` `progress_tx` (P3.43), the
//! cooperative `CancellationToken` chunk-boundary poll (P3.44), and the wall-clock timeout wrapper (P3.45)
//! are the **caller's** — the native engine captures `progress_tx` + the token inside the closure it hands
//! to `run_in_core`, and P3.45 wraps the lane future in `tokio::time::timeout`. §1.7 owns the `Receiver`, so
//! the lane stays a minimal permit + off-runtime primitive and references no type authored downstream.
//!
//! ## SHELLED — a doc-only contract map P4 EXPANDS (never rebuilds)
//!  - **P4.20** EXPANDS [`Pool`] onto the subprocess engines + the §1.10 memory-adaptive
//!    `effective = min(global_degree, per_engine_cap, memory_cap)` factor; it REUSES [`clamp_global_degree`]
//!    verbatim (it does not re-author the degree formula this module owns).
//!  - **P4.21** adds the per-engine caps (LibreOffice 1, video re-encode 1–2, the rest up to the degree).
//!  - **P4.22** adds the `serialised_only` enforcement — a dedicated single-permit `Semaphore` per
//!    serialised engine, allocated at registry-build time; a serialised job acquires BOTH the global permit
//!    AND that engine's single-permit before spawn, releasing both on exit — and authors `MAX_LO_CONCURRENCY
//!    = 1` (§0.9-owned, imported by the §6.7.2 harness). **P4.23** re-homes the native lane P3.3 built onto
//!    the now-real pool, unchanged.
//!  - The §0.9 per-engine timeout / watchdog-poll / no-progress `pub const`s are authored with their
//!    consumers: the §1.7 native wall-clock timeout with P3.45, the subprocess watchdog set with P4.20.
//!    **P3.3 authors no `pub const`** (no P3 consumer imports them).
//!
//! ## Tier note (§0.7 tier-3 vs §0.9's `HashMap<EngineId, bool>`)
//! `EngineId` lives in the tier-2 `crate::engines` layer, so a tier-3 leaf cannot name it. P3.3's live
//! scope needs none (the native lane acquires only the global permit). §0.9's serialised-flag map is DATA
//! the tier-2 registry pre-computes from each `descriptor()` and hands the pool at registry-build time — the
//! pool never calls UP into the registry. P4.22 realises it tier-legally (a generic-keyed map the registry
//! instantiates with `EngineId`, a legal downward edge — or a re-home of `EngineId` decided with its
//! consumer); this module names no `crate::engines` type, keeping §0.7's downward-only tiering intact.

// [Build-Session-Entscheidung: P3.3] dead_code expect — the §0.9 Pool + the §1.7 in-core spawn_blocking
// lane are authored ahead of their production consumers. `expect` (not `allow`) auto-flags the moment the
// P3.4/P3.43 consumer lands, matching crate::engines / crate::domain / crate::outcome.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the §0.9 Pool + the §1.7 in-core spawn_blocking lane (Pool::new/with_degree/run_in_core), LaneError, and the clamp_global_degree/resolve_global_degree degree helpers are authored ahead of their production consumers: the §1.7 dispatch reaches the InProcessNative arm at P3.4 and the §3.5.6 native CSV/TSV executor runs on run_in_core from P3.43, and P4.20/P4.22 EXPAND the pool onto the subprocess engines + the serialised single-permit lane. Nothing constructs a Pool in the production build until P3.43; the cfg(test) tests below construct the pool and exercise the lane, so the test build is dead-code-clean. expect (not allow) auto-flags the moment the P3.4/P3.43 consumer lands — matching crate::engines/crate::domain/crate::outcome."
    )
)]

use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::sync::Semaphore;

/// The failure modes of the §0.9 in-core `spawn_blocking` lane. INTERNAL — never on the IPC wire (no
/// `serde`/`specta`); the §1.7/§2.8 caller (P3.46) maps it onto a per-item `Failed`, so a lane failure is
/// always ONE item's failure, never a pool-wide fault. [Build-Session-Entscheidung: P3.3] `Debug` + the
/// test-assertion `PartialEq`/`Eq`; NO `Clone` (the caller matches/maps it, never clones) — mirroring the
/// internal `crate::engines` descriptor types.
#[derive(Debug, PartialEq, Eq)]
pub enum LaneError {
    /// The global-degree semaphore was closed — no permit can be granted. Unreachable while the app runs
    /// (the pool lives for the process lifetime and is never closed); surfaced without a panic to keep the
    /// §0.9 no-panic pool path.
    PoolClosed,
    /// The worker closure panicked; `tokio::task::spawn_blocking` caught the unwind (§2.13 catch_unwind
    /// semantics — the worker is not killed). Because a `spawn_blocking` task cannot be abort-cancelled, a
    /// `JoinError` from this lane is ALWAYS a captured panic. The permit was released on unwind, so the pool
    /// is NOT poisoned — the next acquire succeeds. Rests on the workspace default `panic = "unwind"`.
    Panicked,
}

/// The §0.9 bounded pool. In P3 it carries the LIVE global-degree permit model + the in-core lane; P4.20/
/// P4.22 EXPAND it with the subprocess machinery (the per-engine serialised single-permit semaphores, the
/// §1.10 memory-adaptive effective degree). [Build-Session-Entscheidung: P3.3] `Clone` = a cheap `Arc`
/// bump sharing the SAME global semaphore, so the one app-wide pool is handed to every executor by value
/// (the tokio-pool convention); `Debug` for diagnostics. NOT `Copy` (owns an `Arc`); NOT `PartialEq` (a
/// semaphore is not comparable). `global` is `Arc<Semaphore>` because `acquire_owned` — needed to move a
/// `'static` permit into the `'static` blocking closure — is defined on `Arc<Semaphore>`.
#[derive(Debug, Clone)]
pub struct Pool {
    /// The global-degree permit source (§0.9): `degree` permits. Every job — subprocess (P4) or
    /// InProcessNative (P3) — acquires one permit here before running.
    global: Arc<Semaphore>,
    /// The resolved global degree (§0.9). Stored because `Semaphore::available_permits` fluctuates as
    /// permits are held; the P4.20/P4.21 effective-degree math + the §1.11 batch bar read this configured
    /// value.
    degree: usize,
}

impl Pool {
    /// Construct the pool sized to this machine's §0.9 global concurrency degree.
    /// [Build-Session-Entscheidung: P3.3]
    pub fn new() -> Self {
        Self::with_degree(resolve_global_degree())
    }

    /// Construct the pool at an explicit degree — the §6.7.2 harness pins a deterministic degree, and the
    /// P4.20 §1.10 memory-adaptive factor re-sizes against it. The degree is floored at 1 (`max(1)`) so the
    /// global `Semaphore` always has ≥1 permit (a zero-permit pool would deadlock every job).
    /// [Build-Session-Entscheidung: P3.3]
    pub fn with_degree(degree: usize) -> Self {
        let degree = degree.max(1);
        Pool {
            global: Arc::new(Semaphore::new(degree)),
            degree,
        }
    }

    /// The §0.9 native-CSV/TSV / §1.7 InProcessNative in-core permit lane. Acquire ONE global-degree
    /// permit, run `task` on a dedicated `spawn_blocking` worker thread (so the synchronous loop never
    /// blocks the Tokio runtime that drives the subprocess engines + IPC), and release the permit on
    /// completion, on a worker panic, AND on abandonment. A caught worker panic → `Err(LaneError::Panicked)`
    /// (never re-raised: re-raising would panic the pool-driver task and violate §0.9 panic isolation). The
    /// caller captures its own `progress_tx` (P3.43) + `CancellationToken` (P3.44) inside `task`, and P3.45
    /// wraps this future in `tokio::time::timeout` (§1.7). [Build-Session-Entscheidung: P3.3]
    pub async fn run_in_core<F, R>(&self, task: F) -> Result<R, LaneError>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        // The permit lives in THIS async frame (not moved into the closure): dropping the future then
        // releases the permit at once while the abandoned blocking task detaches and runs on — the §1.7
        // wedged-uninterruptible-read design (the abandoned thread must not hold a global-degree permit, or
        // a handful of wedges would starve the pool). Moving it into the closure would keep a wedged
        // thread's permit held until that thread finishes. [Build-Session-Entscheidung: P3.3]
        let _permit = self
            .global
            .clone()
            .acquire_owned()
            .await
            .map_err(|_closed| LaneError::PoolClosed)?;
        // spawn_blocking gives the §2.13 catch_unwind boundary for free: a panic → JoinError (never
        // is_cancelled — we never abort the handle). We deliberately do NOT resume_unwind it (that would
        // panic the pool-driver task); it surfaces as a clean per-item LaneError the §1.7/§2.8 caller maps
        // to Failed. `_permit` drops when this fn returns (Ok or panic-mapped Err) or when the future is
        // dropped (abandon) — released on all three paths, so the pool is never poisoned or down a permit.
        tokio::task::spawn_blocking(task)
            .await
            .map_err(|_join_err| LaneError::Panicked)
    }

    /// Test-only seam: close the global semaphore so the next acquire fails — exercises the `PoolClosed`
    /// arm (unreachable in the running app). `cfg(test)`, so it is absent from production.
    /// [Build-Session-Entscheidung: P3.3]
    #[cfg(test)]
    fn close(&self) {
        self.global.close();
    }
}

impl Default for Pool {
    fn default() -> Self {
        Self::new()
    }
}

/// The §0.9 global-degree clamp — `clamp(cores − 1, 1, 4)`: leave a core free (`saturating_sub(1)`, never
/// underflows), cap at 4 so a many-core machine cannot spawn a thrashing number of engines, floor at 1 so a
/// single-core host still runs (§0.9 "everyday default 2–4"). Pure over the passed core count so the §0.9
/// formula is unit-tested machine-independently, and so P4.20 REUSES it verbatim rather than re-inlining the
/// formula. [Build-Session-Entscheidung: P3.3]
fn clamp_global_degree(cores: usize) -> usize {
    cores.saturating_sub(1).clamp(1, 4)
}

/// This machine's §0.9 global degree — `clamp(available_parallelism − 1, 1, 4)`.
/// [Build-Session-Entscheidung: P3.3] v1 resolves the core count via `std::thread::available_parallelism()`
/// (std-native, no added dependency; it respects OS affinity / cgroup limits, which serves the §0.9 "keep
/// the machine usable" intent better than a raw physical count). `available_parallelism` is fallible — an
/// unknowable count on an exotic platform falls back to 1 core → degree 1 (`unwrap_or`, never a panic on the
/// §0.9 no-panic pool path). The §0.9 literal says `physical_cores`; std exposes no physical-core API, and
/// the clamp to [1,4] + the per-engine caps (§0.9 table, P4.21) keep heavy engines conservative regardless
/// of the logical-vs-physical difference — the §0.9 spec is reconciled to this in the same commit (DoD
/// item 2). Physical-core precision (a dedicated crate) is an unadopted refinement.
fn resolve_global_degree() -> usize {
    let cores = std::thread::available_parallelism()
        .map(NonZeroUsize::get)
        .unwrap_or(1);
    clamp_global_degree(cores)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Barrier;

    // §6.4.1 (G15): the §0.9 formula clamp(cores − 1, 1, 4), pinned machine-independently.
    #[test]
    fn clamp_global_degree_matches_the_spec_formula() {
        let cases = [
            (0, 1),
            (1, 1),
            (2, 1),
            (3, 2),
            (4, 3),
            (5, 4),
            (8, 4),
            (16, 4),
        ];
        for (cores, want) in cases {
            assert_eq!(
                clamp_global_degree(cores),
                want,
                "§0.9: clamp({cores} − 1, 1, 4) == {want}"
            );
        }
    }

    // §6.4.1 (G15): the machine read stays in the clamped range and matches the pure formula.
    #[test]
    fn resolve_global_degree_is_in_the_clamped_range_and_matches_the_formula() {
        let degree = resolve_global_degree();
        assert!(
            (1..=4).contains(&degree),
            "§0.9: the resolved global degree is always in 1..=4; got {degree}"
        );
        let cores = std::thread::available_parallelism()
            .map(NonZeroUsize::get)
            .unwrap_or(1);
        assert_eq!(
            degree,
            clamp_global_degree(cores),
            "§0.9: resolve == clamp_global_degree(available_parallelism)"
        );
    }

    // §6.4.1 (G15): Pool::new sizes the semaphore to the resolved degree.
    #[test]
    fn new_sizes_the_semaphore_to_the_resolved_global_degree() {
        let pool = Pool::new();
        assert_eq!(
            pool.degree,
            resolve_global_degree(),
            "the constructed pool stores the resolved global degree"
        );
        assert_eq!(
            pool.global.available_permits(),
            pool.degree,
            "§0.9: the global-degree semaphore starts with exactly `degree` permits"
        );
    }

    // §6.4.x (G15) PERMIT BOUNDING: a Barrier(degree) forces all `degree` permit-holders to rendezvous
    // (liveness — proves permits are granted up to the full degree) while the degree-permit bound caps the
    // peak (safety — a broken/over-permitting lane would push more than `degree` into the closure and the
    // peak past `degree`). N = 2·degree is a multiple of the Barrier size, so the reusable Barrier never
    // strands a partial final group. Deterministic — no reliance on a sleep window overlapping.
    #[tokio::test]
    async fn run_in_core_bounds_concurrency_to_the_global_degree() {
        const DEGREE: usize = 3;
        let pool = Pool::with_degree(DEGREE);
        let barrier = Arc::new(Barrier::new(DEGREE));
        let concurrent = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..(2 * DEGREE) {
            let pool = pool.clone();
            let barrier = Arc::clone(&barrier);
            let concurrent = Arc::clone(&concurrent);
            let peak = Arc::clone(&peak);
            handles.push(tokio::spawn(async move {
                pool.run_in_core(move || {
                    let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    barrier.wait(); // permit-bounded: exactly DEGREE can be here at once
                    concurrent.fetch_sub(1, Ordering::SeqCst);
                })
                .await
                .expect("§0.9: the in-core lane runs the closure to completion");
            }));
        }
        for handle in handles {
            handle.await.expect("each spawned lane task joins");
        }

        assert_eq!(
            peak.load(Ordering::SeqCst),
            DEGREE,
            "§0.9: exactly `degree` in-core closures run concurrently — the Barrier forces all degree \
             permit-holders to rendezvous (liveness) and the degree-permit bound caps the peak (safety)"
        );
        assert_eq!(
            concurrent.load(Ordering::SeqCst),
            0,
            "every permit's critical section exited"
        );
        assert_eq!(
            pool.global.available_permits(),
            DEGREE,
            "§0.9: all global-degree permits are released after the batch drains"
        );
    }

    // §6.4.x (G15) OFF THE RUNTIME: the closure runs on a spawn_blocking worker thread, never the async
    // runtime thread. Deterministic via a ThreadId inequality (no timing).
    #[tokio::test]
    async fn run_in_core_runs_the_closure_off_the_async_runtime_thread() {
        let pool = Pool::with_degree(2);
        let runtime_thread = std::thread::current().id();
        let worker_thread = pool
            .run_in_core(|| std::thread::current().id())
            .await
            .expect("§0.9: the in-core lane returns the closure's value");
        assert_ne!(
            runtime_thread, worker_thread,
            "§0.9/§1.7: the synchronous closure runs on a dedicated spawn_blocking worker thread, never the \
             Tokio runtime thread (so the CSV loop never blocks the runtime driving the subprocess engines + IPC)"
        );
    }

    // §6.4.x (G15) PANIC RELEASE + NO POISON: a panicking closure surfaces Panicked and releases its permit;
    // a subsequent acquire succeeds. degree 1 makes the release load-bearing (the second run reuses the SAME
    // single permit). The panic is induced by `unwrap`-on-a-`black_box`ed-None: `unwrap` is test-allow-listed
    // (unlike the deny-listed `panic!` macro, no test exception in this crate), and `black_box` hides the
    // `None` from `clippy::unnecessary_literal_unwrap` — which also fires under `-D warnings` on a bare
    // `None.unwrap()`. The caught-panic backtrace on stderr is EXPECTED, not a failure.
    // [Build-Session-Entscheidung: P3.3]
    #[tokio::test]
    async fn a_panicking_closure_releases_its_permit_and_does_not_poison_the_pool() {
        let pool = Pool::with_degree(1);
        let panicked: Result<u32, LaneError> = pool
            .run_in_core(|| std::hint::black_box(Option::<u32>::None).unwrap())
            .await;
        assert_eq!(
            panicked,
            Err(LaneError::Panicked),
            "§0.9/§2.13: a worker panic surfaces as a clean LaneError::Panicked, never a re-raised pool-path panic"
        );
        let recovered = pool.run_in_core(|| 42_u32).await.expect(
            "§0.9: the single permit was released despite the panic — the pool is not poisoned",
        );
        assert_eq!(recovered, 42);
    }

    // §6.4.x (G15): a closed pool surfaces PoolClosed, never an unwrap/panic (the no-panic acquire-error map).
    #[tokio::test]
    async fn a_closed_pool_surfaces_pool_closed_without_a_panic() {
        let pool = Pool::with_degree(2);
        pool.close();
        assert_eq!(
            pool.run_in_core(|| 1_u32).await,
            Err(LaneError::PoolClosed),
            "§0.9: acquiring on a closed semaphore maps to PoolClosed, never an unwrap/panic on the no-panic pool path"
        );
    }
}
