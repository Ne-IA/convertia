//! `crate::pool` — the §0.9 bounded engine-subprocess pool: the single owner of the global concurrency
//! degree, of the per-engine parallelism rules ([`EngineParallelism`] + its cap consts since P4.21;
//! LibreOffice serialised via the dedicated single-permit [`SerialisedLanes`] since P4.22) and of the
//! timeout / no-progress-watchdog parameters. A §0.7 **tier-3 leaf** — it
//! depends DOWN only, on `std`, the `tokio` runtime primitives and (since P4.20) the tier-3 SIBLING
//! `crate::platform` for the §1.10 available-memory read; it names **no** tier-2 type. A sibling edge is not
//! an upward one — §0.7 puts `pool`, `platform` and `domain` on the same tier as "the leaves: they depend on
//! nothing above" — and `platform` has no edge back, so the graph stays acyclic and strictly downward.
//! Unsafe-free (the crate-root `#![deny(unsafe_code)]` in `main.rs` covers it; the OS probe's own `unsafe`
//! is confined to `crate::platform`, the one G29-allow-listed FFI module); `tokio::sync::Semaphore` +
//! `tokio::task::spawn_blocking` add no FFI and no sockets, so the G29 rule (g)/(j) socket ban + the §3
//! zero-egress rule hold.
//!
//! ## LIVE in P3.3 (real, tested)
//!  - [`Pool`] carries the **global-degree** permit model: an `Arc<Semaphore>` sized to
//!    `clamp(available_parallelism − 1, 1, 4)` (§0.9; see the `available_parallelism` note on
//!    [`resolve_global_degree`]), the bound every engine job acquires.
//!  - [`Pool::run_in_core`] is the **`spawn_blocking`-style in-core worker-thread lane** the sole
//!    `EngineProgram::InProcessNative` engine (native CSV/TSV, §3.5.6) runs on: it acquires a slot (one
//!    global-degree permit at full effective degree, `slot_weight` of them under §1.10 memory pressure —
//!    P4.20), runs a synchronous closure on a dedicated `spawn_blocking` worker thread so the
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
//! ## The §1.7 wedged-read bound — the bounded-pool-headroom leg [Build-Session-Entscheidung: P3.45]
//! §1.7 requires "the abandoned thread MUST NOT exhaust the blocking pool" and offers its mechanism as an
//! explicit AND/OR: **either** a bounded pool with headroom **or** a per-read deadline. The P3 slice takes the
//! **FIRST leg — bounded-pool-with-headroom** — which IS the already-built architecture and needs no new code:
//! [`Pool::run_in_core`] frees its §0.9 global-degree permit the instant its future is dropped (the `_permit`
//! lives in the async frame, NOT the blocking closure — see [`Pool::run_in_core`]), so a §1.7-timed-out
//! (abandoned) worker parks in the pool's headroom holding **no** permit; and the Tauri-managed multi-thread
//! Tokio runtime's default blocking pool is **bounded** (512 threads at the pinned tokio 1.52.3) with ample
//! headroom above the §0.9 global degree (`clamp(cores − 1, 1, 4)`). A handful of wedged uninterruptible
//! reads therefore degrade gracefully — those items fail `Failed(EngineHang)`, the batch finishes — rather
//! than wedging the pool. The **per-read-deadline leg is deliberately NOT built** (a spec-sanctioned OR-choice,
//! §2.12.4 demands no per-read deadline; the freeze already stat-prechecks + resolves every source, P3.9, so a
//! wedge is rare post-freeze device death). The wall-clock timeout parameter is [`NATIVE_CSV_TSV_TIMEOUT`].
//!
//! ## LIVE in P4.20 (the subprocess half + the §1.10 memory factor)
//!  - [`Pool::run_subprocess`] is the **subprocess permit lane** — the half P3.3 shelled. It takes a slot
//!    (`slot_weight` permits — one at full effective degree, more under §1.10 memory pressure), awaits the
//!    caller's engine future and releases on every exit path (clean, failed/killed, watchdog-reaped,
//!    caller-dropped). The §1.10 watermark pause is deliberately NOT in either lane — see the bullet
//!    below. It takes a
//!    caller-supplied FUTURE rather than a `'static` closure because the tier-2 confined runner borrows its
//!    arguments — the same generic-`R` trick that keeps this leaf from naming a tier-2 type.
//!  - [`Pool::effective_degree`] is the §0.9/§1.10 three-term `min(global_degree, per_engine_cap,
//!    memory_based_cap)`. It REUSES [`clamp_global_degree`] verbatim through [`resolve_global_degree`] (it
//!    does not re-author the degree formula this module owns) and layers the §1.10 memory term on top. It is
//!    ENFORCED, not merely computed: `slot_weight` turns it into the number of global permits one slot takes
//!    (`ceil(degree / effective)`), so under memory pressure fewer lanes fit in the same fixed semaphore —
//!    down to exactly one, §1.10's "down to serial". Both lanes acquire by weight.
//!  - The §1.10 high-memory watermark: [`Pool::await_dispatch_headroom`] holds a NEW item back while
//!    available memory is under [`HIGH_MEMORY_WATERMARK_BYTES`], re-reading every
//!    [`MEMORY_WATERMARK_POLL`] and CEILED at [`MEMORY_PAUSE_MAX`] so the throttle can never wedge a batch.
//!    It is called by `crate::engines::dispatch` at the §1.7 dispatch entry — deliberately OUTSIDE the
//!    lanes, because §1.7 wraps each lane in a wall-clock timeout and a pause spent inside one would come
//!    out of the ENGINE's budget (§2.12.3 never-break). That placement also gives "in-flight items finish"
//!    for free, and makes the gate live in production today via the in-core CSV/TSV lane rather than staged
//!    for P4.32.
//!  - The reading comes from `crate::platform::available_memory_bytes`, injected as a fn pointer so
//!    [`Pool::with_degree`] stays deterministic for the §6.7.2 harness and the tests drive exact values.
//!
//! ## LIVE in P4.21 (the §0.9 per-engine caps)
//!  - [`EngineParallelism`] is the §0.9 engine table as a closed enum — the vocabulary a job's engine
//!    declares through the §3.2.2 `Engine::parallelism` seam, and the only way a caller obtains a
//!    `per_engine_cap`. [`EngineParallelism::per_engine_cap`] is the ONE place a §0.9 row becomes a number
//!    ([`MAX_VIDEO_REENCODE_CONCURRENCY`]), so no call site can invent a cap.
//!  - BOTH lanes now take the row and acquire by it: the per-engine term rides the SAME
//!    [`Pool::slot_weight`] mechanism the §1.10 memory term uses, so a capped job takes proportionally more
//!    of the fixed semaphore and proportionally fewer of its kind are admitted. It is ENFORCED, not merely
//!    computed.
//!  - LibreOffice's "serialised — exactly 1" row is deliberately NOT in this vocabulary: §0.9 gives the
//!    serialised engine its own BOTH-permits mechanism (P4.22) and says non-serialised jobs take only the
//!    global permit, so a serialised job takes ONE global permit — see [`EngineParallelism`] for the full
//!    argument.
//!
//! ## LIVE in P4.22 (the §0.9 `serialised_only` enforcement)
//!  - [`MAX_LO_CONCURRENCY`] `= 1` is the §0.9-owned single source of the serialisation degree (imported by
//!    the §6.7.2 harness, never hard-coded), and it is the number [`SerialisedLanes`] sizes each lane with.
//!  - [`SerialisedLanes`] is §0.9's `[DECIDED]` mechanism: one dedicated single-permit `Semaphore` per
//!    engine whose descriptor carries `serialised_only`, allocated at registry-build time; a job for such
//!    an engine acquires BOTH that permit and the global-degree one before spawn and releases both on exit,
//!    while a non-serialised engine has no lane and takes only the global permit.
//!  - It is GENERIC in the key, which is how the §0.7 tier holds: the pool owns the mechanism, the tier-2
//!    registry instantiates `SerialisedLanes<EngineId>` from its own `descriptor()` walk. This is the
//!    resolution P4.5 left open, taken WITHOUT re-homing `EngineId` — see the tier note below.
//!  - The §1.7 caller takes the ENGINE permit first and the global one second (the tier-1 conductor holds
//!    the guard across `dispatch`), so a job queued behind LibreOffice holds no global permit and an
//!    office-heavy batch cannot fill the §0.9 degree with waiters — the ordering argument lives on
//!    [`SerialisedLanes`].
//!
//! ## SHELLED — a doc-only contract map P4 EXPANDS (never rebuilds)
//!  - **P4.23** re-homes the native lane P3.3 built onto the now-real pool, unchanged.
//!  - The §0.9 per-engine timeout / watchdog-poll / no-progress `pub const`s are authored with their
//!    consumers: the §1.7 native wall-clock timeout [`NATIVE_CSV_TSV_TIMEOUT`] is now LIVE (authored P3.45,
//!    consumed by the §1.7 `bounded_lane` wrapper); the subprocess watchdog set — [`WATCHDOG_POLL_INTERVAL`] /
//!    [`NO_PROGRESS_TIMEOUT`] / [`SUBPROCESS_WALL_CLOCK_DEFAULT`] / [`VIDEO_WALL_CLOCK`] — is authored **at
//!    P4.12** with its consumer, the §1.7 `crate::engines::run_subprocess` no-progress/wall-clock watchdog
//!    (the §0.9 "authored with their consumers" principle; this reconciles the prior "authored with P4.20"
//!    forecast — the watchdog MECHANISM lands at P4.12, so its parameters do too, while P4.20 expands the pool
//!    *structure*). They stay dead in the production build until P4.32 wires `run_subprocess` live, exactly
//!    like [`GROUP_CONFIRM_WAIT`]. **P3.3 authored no `pub const`** (no P3.3 consumer imported one; P3.45 adds
//!    the first). P4.20's own §1.10 memory `pub const`s follow the same rule and ARE live: their consumers
//!    ([`Pool::effective_degree`] and the watermark gate) are in this module.
//!
//! ## Tier note (§0.7 tier-3 vs §0.9's `HashMap<EngineId, bool>`)
//! `EngineId` lives in the tier-2 `crate::engines` layer, so a tier-3 leaf cannot name it. P3.3's live
//! scope needs none (the native lane acquires only the global permit). §0.9's serialised-flag map is DATA
//! the tier-2 registry pre-computes from each `descriptor()` and hands the pool at registry-build time — the
//! pool never calls UP into the registry. **P4.22 CLOSED that open question with the FIRST of its two
//! options — a generic-keyed value the registry instantiates — so `EngineId` was NOT re-homed:**
//! [`SerialisedLanes<K>`] owns the semaphores, the permit count and the acquire; `EngineRegistry` holds the
//! one `SerialisedLanes<EngineId>` instance, built from the same `descriptor()` walk that yields the flag
//! map. Making [`Pool`] itself generic was rejected — `Pool` is the app-managed value every §1.7 signature
//! names, so a key parameter would spread a tier-2 type across the whole tier-3 surface to buy nothing.
//! This module still names no `crate::engines` type, keeping §0.7's downward-only tiering intact.
//! P4.20 kept that intact for the subprocess half too: the lane is generic in the caller's return type, so
//! `ConfinedRun`/`InvocationResult`/`EngineInvocation` are instantiated BY the tier-2 caller and named
//! nowhere here. P4.21 keeps it intact for the per-engine caps as well: [`EngineParallelism`] is keyed by
//! nothing at all — it is a plain §0.9 ROW the tier-2 engine declares per job and hands DOWN, so the cap
//! needs no `EngineId` key and this leaf still names no `crate::engines` type. So all three P4 boxes that
//! could have forced an `EngineId` re-home resolved without one, by three different tier-legal shapes:
//! generic in the caller's return type (P4.20), keyed by nothing (P4.21), generic in the key (P4.22).

// [Build-Session-Entscheidung: P3.3] dead_code expect — the §0.9 Pool + the §1.7 in-core spawn_blocking
// lane are authored ahead of their production consumers. P3.43 WIRED run_in_core + LaneError into the dispatch
// InProcessNative arm, but they stay dead until the P3.46 conductor makes dispatch a live root; Pool
// construction + the degree helpers stay dead until the P4 pool wiring. `expect` (not `allow`) auto-flags the
// moment the last of these consumers lands, matching crate::engines / crate::domain / crate::outcome.
// [Test-Change: P3.43 — old-obsolete+new-correct, §0.9] reason-string accuracy edit (the &Pool dispatch means P3.43 constructs no Pool, so "until P3.43" is obsolete); the removed line quotes the lint keyword before a paren — a production-.rs G70 --diff over-flag (P3.7/P3.8 precedent), no test assertion changed.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Items authored ahead of their production consumers, each named with the box that makes it live. DEAD TODAY: Pool::with_degree + no_memory_cap (the deterministic pinned-degree constructor path, used by the §6.7.2 harness and by tests only); Pool::run_subprocess, whose production consumers are the P4.32 subprocess dispatch arms (the §1.7 lane calls it once the program-path resolution supplies a resolved binary); the EngineParallelism::VideoReencode VARIANT (P4.21), which is the §0.9 'FFmpeg (video re-encode)' table row and is therefore first CONSTRUCTED by the P6 FFmpeg engine's Engine::parallelism impl — the enum itself, its per_engine_cap projection and MAX_VIDEO_REENCODE_CONCURRENCY are LIVE (the native lane acquires by row on every live dispatch, and the projection's match arm reads the const), only that one variant has no production constructor yet; and the §0.9 subprocess-watchdog consts WATCHDOG_POLL_INTERVAL / NO_PROGRESS_TIMEOUT / SUBPROCESS_WALL_CLOCK_DEFAULT / VIDEO_WALL_CLOCK (P4.12) plus GROUP_CONFIRM_WAIT (P4.11), whose consumers (crate::engines::run_subprocess, crate::isolation::run_confined) are themselves dead until P4.32. LIVE, hence deliberately NOT listed: Pool::new and the with_degree_and_memory it calls one hop down; Pool::run_in_core (the native CSV/TSV lane runs through the live §1.7 dispatch) and, through it, Pool::slot_weight and Pool::effective_degree — the weighted acquire enforces the §1.10 effective degree on every live acquire — plus the MEMORY_PER_SLOT_BYTES those read; LaneError, whose variants run_in_core constructs and the live crate::engines::bounded_lane matches; Pool::await_dispatch_headroom and HIGH_MEMORY_WATERMARK_BYTES / MEMORY_WATERMARK_POLL / MEMORY_PAUSE_MAX, called by crate::engines::dispatch at the §1.7 dispatch entry; NATIVE_CSV_TSV_TIMEOUT; and the clamp_global_degree/resolve_global_degree degree helpers Pool::new resolves through. The cfg(test) tests below construct every pool shape and exercise both lanes, so the test build is dead-code-clean. `expect` (not `allow`) auto-flags the moment the last consumer lands."
    )
)]

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Semaphore, SemaphorePermit};

/// The §0.9-owned per-engine **wall-clock timeout** for the §1.7 `InProcessNative` native CSV/TSV engine
/// (§3.5.6) — the single source of that engine's time bound, so test and prod can never drift (the §0.9
/// "named `pub const`s in this §0.9 pool module … imported by the §6.7.2 test harness" contract, co-located
/// with the P4.22 `MAX_LO_CONCURRENCY`). The §1.7 `bounded_lane` wrapper (`crate::engines`, P3.45) runs the
/// native lane under `tokio::time::timeout(NATIVE_CSV_TSV_TIMEOUT, …)`; on expiry the lane future is dropped
/// (its §0.9 permit freed at once, the blocking worker detached), the cooperative poll is tripped, and the
/// item is `Failed(EngineHang)` while the run CONTINUES (the §1.7 InProcessNative timeout sub-case; the
/// wedged-uninterruptible-read residue parks in the pool's bounded headroom, §2.12.4).
///
/// **Baseline (pre-calibration).** [Build-Session-Entscheidung: P3.45] `120s` — **tight for this light engine**
/// relative to the video engine's minutes-long budget (§0.9), yet generous enough that any legitimate native
/// CSV/TSV transform completes well within it: the transform is a bounded, whole-file-buffered, linear
/// re-encode/re-quote (§3.5.6), so even a multi-hundred-MB export on slow media finishes in seconds, and the
/// real trigger of this bound is a wedged/pathological stall, not a large-but-progressing file. The §1.10
/// input-size preflight (P4.72) is the primary size bound (it rejects an over-budget input before the engine);
/// this timeout is the stall backstop. v1 ships this as the pre-calibration baseline (the §0.9 "baseline values
/// calibrated against the §6 corpus"); **P3.61** authors the deterministic bound-firing fixture that exercises
/// THIS §1.7 wall-clock timeout — the §0.9 "timeout-sentinel case" for the P3 slice. (P9.41 calibrates the
/// separate §1.10 SIZE budgets over the P4.72 engine — NOT this §1.7 timeout.)
pub const NATIVE_CSV_TSV_TIMEOUT: Duration = Duration::from_secs(120);

/// The §0.9-owned **group-kill confirm-wait bound** for the §1.7 cancel ordering (P4.11) — the short cap the
/// §2.12 confined runner (`crate::isolation::run_confined`) waits, AFTER issuing the whole-group kill, for the
/// OS to reap the engine's process group before the tier-1 conductor removes the per-job `*.part`
/// (§1.7 step 2 → step 3). Homed here beside [`NATIVE_CSV_TSV_TIMEOUT`] so the one §0.9 time bound is the
/// single source of truth (test and prod can never drift — the §0.9 "named `pub const`s … imported by the
/// §6.7.2 test harness" contract). Its consumer is `crate::isolation` (a downward tier-2 → tier-3 edge, the
/// same shape `crate::engines` already has on `NATIVE_CSV_TSV_TIMEOUT`).
///
/// **Why bounded, and why it is a settle *window*, not a proof (P4.10 forward note).** The kill
/// (`TerminateJobObject` / `killpg`) is unrefusable, but on Windows an open descendant handle blocks the
/// `*.part` deletion, so the runner gives the OS up to this cap to release the handle so the conductor's
/// subsequent single removal succeeds on the normal path. Neither platform's `wait()` PROVES the group empty
/// (Windows `JobObjectChild::wait` returns on the FIRST completion-port message; POSIX `ProcessGroupChild::wait`
/// returns on `waitpid(-pgid)` → `ECHILD`, i.e. no children of *ours* remain), so the runner never asserts
/// emptiness — it waits up to the cap, then returns REGARDLESS. On a wedged descendant the cap is what keeps
/// §7.3.3 quit-while-converting and the §5.8 cancel round-trip from hanging; the still-held `*.part` is then a
/// §2.6.4-case-3 `CleanupResidue` reclaimed by the §2.6.3 sweep. The removal's own success/failure — not this
/// wait — is the honest residue signal (§2.6.4 single bounded attempt).
///
/// **Baseline (pre-calibration).** [Build-Session-Entscheidung: P4.11] `5s` — normal group teardown returns in
/// well under this (the runner returns as soon as `wait()` returns), so the cap bites ONLY on a genuinely
/// wedged descendant; `5s` is generous for that rare case yet short enough that a cancel/quit stays responsive
/// (v1 is sequential, so it is a per-cancel worst case, not per-item). The §0.9 "baseline values calibrated
/// against the §6 corpus" applies — this is the pre-calibration value, tunable like the other §0.9 bounds.
pub const GROUP_CONFIRM_WAIT: Duration = Duration::from_secs(5);

/// The §0.9-owned **watchdog poll interval** for the §1.7 subprocess no-progress/timeout watchdog (P4.12) —
/// how often the `crate::engines::run_subprocess` lane checks a running engine for progress/liveness while it
/// races the confined run. Homed here beside the other §0.9 time bounds so the pool is the single source of the
/// watchdog parameters (test and prod can never drift — the §0.9 "named `pub const`s … imported by the §6.7.2
/// test harness" contract). Engine-INDEPENDENT: the poll cadence is a mechanism property, not a per-engine
/// value, so the lane reads this const directly; the per-engine [`NO_PROGRESS_TIMEOUT`] /
/// [`SUBPROCESS_WALL_CLOCK_DEFAULT`] / [`VIDEO_WALL_CLOCK`] bounds are passed in per invocation (the caller
/// selects them, the way `dispatch` passes [`NATIVE_CSV_TSV_TIMEOUT`]). A hang is detected within
/// `no_progress + WATCHDOG_POLL_INTERVAL` of the last progress signal.
///
/// **Baseline (pre-calibration).** [Build-Session-Entscheidung: P4.12] `250ms` — four polls per second: snappy
/// enough that a hung engine is reaped within a quarter-second of its no-progress/wall-clock bound, cheap enough
/// (one async timer tick) to run per live subprocess. The §0.9 "baseline values calibrated against the §6
/// corpus" applies — tunable like the other §0.9 bounds.
pub const WATCHDOG_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// The §0.9-owned **no-progress threshold** for the §1.7 subprocess watchdog (P4.12) — the time a STREAMING
/// engine may produce no progress before it is treated as hung → killed → `Failed(EngineHang)` (§1.7
/// timeout/hang policy; §2.8). Measured from the last forwarded progress fraction (the `on_progress` sink the
/// §1.7 per-`ProgressModel` line-reader feeds). The §1.7 watchdog (`crate::engines::bounded_confined_run`)
/// applies this leg **only to a streaming model** (`FfmpegKeyValue`/`VipsStdout`), whose silence genuinely
/// signals a hang; a no-tick `CoarseSpawnDone` invocation (the ffprobe probe / a no-native-progress encode)
/// emits no fractions, so this leg is INERT for it and the wall-clock ([`SUBPROCESS_WALL_CLOCK_DEFAULT`]) is its
/// sole bound — a no-progress bound over a no-tick engine would falsely reap a live-but-quiet conversion
/// (`NO_PROGRESS_TIMEOUT < SUBPROCESS_WALL_CLOCK_DEFAULT`). (§1.7/§0.9 also name output-FILE-size growth as a
/// third no-progress signal; monitoring `out_tmp` size for a no-tick encode is a P5–P7 refinement, per the
/// `bounded_confined_run` FORWARD note.) Homed here beside [`WATCHDOG_POLL_INTERVAL`] so the §6.7.2 harness
/// imports the same value (no test≠prod drift).
///
/// **Baseline (pre-calibration).** [Build-Session-Entscheidung: P4.12] `90s` — generous enough that a
/// legitimately-slow-but-progressing engine (a large re-encode emitting `-progress` ticks, a document render
/// whose §1.11 heuristic ticks) is never falsely reaped, tight enough that a genuinely wedged decoder fails the
/// one item within a minute-and-a-half rather than leaving the user staring at a hang. The §0.9 "baseline values
/// calibrated against the §6 corpus" applies — tunable.
pub const NO_PROGRESS_TIMEOUT: Duration = Duration::from_secs(90);

/// The §0.9-owned **per-engine wall-clock timeout — the light-engine default** for the §1.7 subprocess watchdog
/// (P4.12): the maximum total runtime for a light/short-lived subprocess engine (poppler / pandoc / the
/// image-worker / LibreOffice) before it is killed → `Failed(EngineHang)` (§1.7), independent of whether it is
/// still emitting progress. The §0.9 per-engine wall-clock is "**tight for the light engines**"; the generous
/// video budget is [`VIDEO_WALL_CLOCK`]. The `EngineId → which wall-clock` selection is the caller's (the P4.32
/// dispatch-arm wiring / the P5–P7 engine adapters, which pass the chosen bound into `run_subprocess` the way
/// `dispatch` passes [`NATIVE_CSV_TSV_TIMEOUT`] to the native lane); this const is the §0.9-owned value they
/// select, single-sourced here so the §6.7.2 harness imports it rather than hard-coding.
///
/// **Baseline (pre-calibration).** [Build-Session-Entscheidung: P4.12] `300s` (5 min) — comfortably above any
/// legitimate light conversion (a document export, a PDF text extract, an image transcode finish in seconds),
/// so the bound bites only a runaway. The §0.9 "baseline values calibrated against the §6 corpus" applies —
/// tunable; §3.4-heavy per-engine calibration lands with each engine's staging box (P5–P7).
pub const SUBPROCESS_WALL_CLOCK_DEFAULT: Duration = Duration::from_secs(300);

/// The §0.9-owned **per-engine wall-clock timeout — the generous video budget** for the §1.7 subprocess watchdog
/// (P4.12): the maximum total runtime for an FFmpeg video re-encode before it is killed → `Failed(EngineHang)`.
/// The §0.9 per-engine wall-clock is "**generous for video — a long film legitimately takes minutes**", so a
/// video re-encode gets this longer budget instead of [`SUBPROCESS_WALL_CLOCK_DEFAULT`]; the no-progress leg
/// ([`NO_PROGRESS_TIMEOUT`]) still catches a *stalled* re-encode long before this. The `EngineId → wall-clock`
/// selection is the caller's (P4.32 / P6 FFmpeg staging), the same seam as [`SUBPROCESS_WALL_CLOCK_DEFAULT`].
///
/// **Baseline (pre-calibration).** [Build-Session-Entscheidung: P4.12] `3600s` (60 min) — generous enough that a
/// long, still-progressing film re-encode is never wall-clock-reaped (the no-progress leg handles a genuine
/// stall), and finite so a truly wedged encode cannot run unbounded. The §0.9 "baseline values calibrated
/// against the §6 corpus" applies — tunable; the FFmpeg-specific calibration lands with P6.
pub const VIDEO_WALL_CLOCK: Duration = Duration::from_secs(3600);

/// The §1.10 **memory budget per concurrent engine slot** — the divisor that turns an available-memory
/// reading into the memory-based degree cap of `effective = min(global_degree, per_engine_cap,
/// memory_based_cap)`. Homed here with the other §0.9 bounds so the §6.7.2 harness imports the same number
/// production uses.
///
/// **Baseline (pre-calibration).** [Build-Session-Entscheidung: P4.20] `512 MiB` per concurrent slot, so
/// the cap reads: ~2 GB AVAILABLE → 4 slots (the §0.9 clamp ceiling), ~1 GB → 2, under 512 MiB → 1, i.e.
/// serial. The order of magnitude is motivated by §0.3.1's envelope ("2 GB minimum-supported … below 2 GB
/// it still launches + converts **serially**, slower") — but note the units differ: §0.3.1 speaks of
/// TOTAL installed RAM while this divisor consumes AVAILABLE memory, which on a 2 GB machine is far less
/// once the OS, the WebView and the app are resident. So this is a defensible placeholder, NOT a derivation
/// from §0.3.1: it puts a minimum-supported machine at or near serial, which is the right end of the curve,
/// while the exact digit is what P6.56.1's memory-constrained-host run calibrates (§1.10 marks the memory
/// numbers "corpus-calibrated starting values"). It is a per-SLOT budget, not a per-item ceiling — the
/// §1.10 per-item memory ceiling that kills one over-budget item is a separate control.
pub const MEMORY_PER_SLOT_BYTES: u64 = 512 * 1024 * 1024;

/// The §1.10 **high-memory watermark** — while available memory is below it, the §1.7 dispatch gate
/// ([`Pool::await_dispatch_headroom`]) holds a NEW item back before it is dispatched at all ("a high-memory
/// watermark pauses dispatch of NEW items … and resumes as memory frees"); items already dispatched are
/// untouched and run to completion. The hold is at the DISPATCH entry, never at the permit acquire — see
/// that fn's doc for why placing it any deeper would breach §2.12.3's never-break floor.
///
/// **Baseline (pre-calibration).** [Build-Session-Entscheidung: P4.20] `256 MiB` — half a slot budget: at
/// that point even one more engine would not fit its own [`MEMORY_PER_SLOT_BYTES`], so admitting it is the
/// step that risks the OOM §1.10 forbids. Deliberately BELOW the per-slot budget so the watermark is the
/// last line rather than a second, earlier throttle — the degree cap above already thins concurrency
/// gradually, and a watermark set at or above the slot budget would pause dispatch while the degree cap
/// was still granting slots, i.e. two controls fighting.
pub const HIGH_MEMORY_WATERMARK_BYTES: u64 = 256 * 1024 * 1024;

/// The §1.10 watermark **re-check cadence** — how often a paused dispatch re-reads available memory to see
/// whether it may proceed ("resumes as memory frees"). [Build-Session-Entscheidung: P4.20] `500ms`, twice
/// [`WATCHDOG_POLL_INTERVAL`]: memory pressure eases on a human/allocator timescale, not a syscall one, so
/// a gentler cadence costs nothing in responsiveness and keeps the probe off the hot path.
pub const MEMORY_WATERMARK_POLL: Duration = Duration::from_millis(500);

/// The §1.10 watermark **pause CEILING** — the longest the pool holds a new item back before dispatching it
/// regardless. The pause is a best-effort throttle, and §2.12.3's never-break floor governs it exactly as it
/// governs the privilege-drop legs: a defence-in-depth control must never be able to stop a conversion from
/// happening. Without a ceiling, a machine that simply never rises above the watermark (a small-RAM host
/// under steady foreign load) would hang the batch forever — the one outcome worse than converting under
/// pressure. On expiry the item proceeds and the OTHER half of the §1.10 policy takes over: a single item
/// that still exceeds its per-item memory ceiling is killed to a clean `Failed(TooBig)` while the batch
/// continues. [Build-Session-Entscheidung: P4.20] `30s` — long enough for a transient spike (another app's
/// large allocation, a GC pause) to clear, short enough that a user never reads it as a freeze.
pub const MEMORY_PAUSE_MAX: Duration = Duration::from_secs(30);

// The two §1.10 memory bounds are const-vs-const, so their ordering is a COMPILE-TIME invariant rather than
// a runtime test: a recalibration that inverted them could not build. (a) the watermark sits BELOW one
// slot's budget, so the degree cap thins concurrency first and the watermark stays the last line rather
// than a second, earlier throttle that fights it; (b) a zero slot budget would divide by zero in the memory
// cap. The Duration bounds' ordering is not const-foldable and is asserted in the test module beside the
// §0.9 watchdog set. [Build-Session-Entscheidung: P4.20]
const _: () = assert!(HIGH_MEMORY_WATERMARK_BYTES < MEMORY_PER_SLOT_BYTES);
const _: () = assert!(MEMORY_PER_SLOT_BYTES > 0);

/// The §0.9-owned **LibreOffice serialisation degree** — how many jobs of a `serialised_only` engine may
/// run at once. `[DECIDED]` **1**, and this is a *correctness* bound rather than a throughput one: §0.9
/// records that LibreOffice headless "is **NOT safely parallel under one user profile** — concurrent
/// `soffice` instances sharing a profile **lock/corrupt** it — a *correctness* issue, not just contention",
/// and that even with the §3.5.2 per-run isolated `-env:UserInstallation` profiles "the safe v1 stance is
/// **one office conversion at a time**".
///
/// It is a named `pub const` here because §0.9 makes it one: "**`MAX_LO_CONCURRENCY = 1` is a §0.9-owned
/// `pub const`** (the single source of the LibreOffice serialisation degree); the §6.7.2 test harness
/// **imports this same constant** rather than hard-coding `1`, so the test env can never drift from prod."
/// [`SerialisedLanes`] sizes every serialised engine's dedicated semaphore with it — the one place the
/// number is spent. [Build-Session-Entscheidung: P4.22]
pub const MAX_LO_CONCURRENCY: usize = 1;

/// A serialised lane must genuinely serialise: a zero-permit lane would deadlock every office job and any
/// value above 1 would re-open the profile-corruption §0.9 calls a correctness issue. Compile-enforced, so
/// a recalibration of the const cannot silently break the property it exists to guarantee.
/// [Build-Session-Entscheidung: P4.22]
const _: () = assert!(MAX_LO_CONCURRENCY == 1);

/// The §0.9 **serialised-engine lanes** — one dedicated [`MAX_LO_CONCURRENCY`]-permit `Semaphore` per
/// engine whose §0.6 descriptor carries `serialised_only` (LibreOffice today), allocated ONCE at
/// registry-build time. This is §0.9's `[DECIDED]` `serialised_only` enforcement mechanism: "the pool holds
/// a **dedicated single-permit semaphore** (one per serialised engine). A job for that engine must
/// **acquire BOTH** the global degree semaphore **and** that engine's single-permit semaphore **before
/// spawn**, and **releases both on subprocess exit** (success/fail/kill) … non-serialised engines acquire
/// only the global degree permit."
///
/// **Why it is GENERIC in the key, and who instantiates it** [Build-Session-Entscheidung: P4.22]. §0.9
/// keys the lanes by `EngineId`, which lives in the tier-2 `crate::engines` layer — a §0.7 tier-3 leaf may
/// not name it. P4.5 left the resolution open ("a generic-keyed map the registry instantiates with
/// `EngineId`, a legal downward edge — or the `EngineId` re-home the tier note leaves open, decided with
/// P4.22"); this box takes the FIRST option, which needs no re-home: the pool owns the mechanism (the
/// semaphores, the permit count, the acquire) as `SerialisedLanes<K>`, and the tier-2 registry instantiates
/// it as `SerialisedLanes<EngineId>` from the very `descriptor()` walk that already produces the P4.5
/// serialised-flag map. The alternative — making [`Pool`] itself generic — was rejected: `Pool` is the
/// app-managed value every §1.7 signature names, so a key parameter would spread a tier-2 type through the
/// whole tier-3 surface to buy nothing, whereas a standalone generic value keeps this module naming **no**
/// `crate::engines` type at all (the same reason [`Pool::run_subprocess`] stays generic in the caller's
/// return type).
///
/// **ORDERING — the engine permit is taken BEFORE the global one** (the §1.7 caller's contract; §0.9 fixes
/// no order, only "acquire BOTH … before spawn"). Both orders are deadlock-free while they are used
/// CONSISTENTLY, because a fixed global order over the two resources admits no cycle — but this order is
/// also the one that cannot wedge the pool's throughput. Taking the global permit first would let N queued
/// office jobs each hold a global permit while waiting on the single office permit, so a batch of office
/// items could occupy the whole §0.9 degree with waiters and starve every other engine (head-of-line
/// blocking). Taking the engine permit first means a job waiting for its turn at LibreOffice holds **no**
/// global permit, so the rest of the batch keeps flowing. It costs the office job nothing that matters: it
/// queues twice (engine lane, then global) rather than once, but it is serialised either way, so the added
/// hop can only delay a job that was already waiting on that same single permit.
///
/// **The acquire is deliberately NOT cancel-aware, and that is a NAMED forward obligation, not an
/// oversight** [Build-Session-Entscheidung: P4.22]. The §1.10 watermark gate in `crate::engines::dispatch`
/// wraps its wait in a `biased;` `select!` on the job token so a cancel is never swallowed by a stall;
/// this wait has no such arm, so a job queued on an engine lane ignores a tripped cancel until the
/// in-flight job of that engine ends. It is UNREACHABLE while the tier-1 conductor is sequential (nothing
/// ever queues on a lane), and the leg is homed on **P4.86** — the box that makes contention real — with
/// its test on **P7.10**, the box that registers the first `serialised_only` engine. Adding the arm here
/// now would land an UNVERIFIED cancel arm — not literally the P4.20 class (an unbiased `select!` silently
/// re-deciding existing semantics), but the same risk profile, and it cannot be verified before P7.10.
///
/// INTERNAL — no `serde`/`specta`; not `Clone` (it OWNS the semaphores, and a clone would silently create a
/// second, independent set of permits — the one thing that would break serialisation). The registry holds
/// the single instance behind its `&'static` handle, so a permit's lifetime is never the constraint.
pub struct SerialisedLanes<K> {
    /// One single-permit lane per SERIALISED engine. A non-serialised engine is deliberately ABSENT rather
    /// than present-with-many-permits, so a missing key means exactly "acquire only the global permit".
    lanes: HashMap<K, Semaphore>,
}

/// Manual `Debug` — a `Semaphore` renders as an opaque blob, so the lanes render as their key set (the
/// question a reader ever has is *which* engines are serialised). Mirrors [`EngineRegistry`]'s own manual
/// `Debug` over its engine map. [Build-Session-Entscheidung: P4.22]
impl<K: std::fmt::Debug> std::fmt::Debug for SerialisedLanes<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerialisedLanes")
            .field("lanes", &self.lanes.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl<K: Eq + std::hash::Hash> SerialisedLanes<K> {
    /// Allocate the lanes from a `(key, serialised_only)` flag set — §0.9's "the pool, at registry-build
    /// time, allocates a `Semaphore(MAX_LO_CONCURRENCY)` for **each engine flagged serialised**". A `false`
    /// flag allocates nothing at all, which is what makes [`Self::acquire`]'s absent-key answer exactly
    /// §0.9's "non-serialised engines acquire only the global degree permit".
    /// [Build-Session-Entscheidung: P4.22]
    #[must_use]
    pub fn build(flags: impl IntoIterator<Item = (K, bool)>) -> Self {
        SerialisedLanes {
            lanes: flags
                .into_iter()
                .filter(|(_, serialised_only)| *serialised_only)
                .map(|(key, _)| (key, Semaphore::new(MAX_LO_CONCURRENCY)))
                .collect(),
        }
    }

    /// Acquire this engine's dedicated single permit, if it has one — the §0.9 "acquire BOTH … before
    /// spawn" half that the global-degree lane does not cover.
    ///
    /// * `Ok(None)` — the engine is not serialised; nothing was acquired and nothing must be released
    ///   (§0.9: "non-serialised engines acquire only the global degree permit").
    /// * `Ok(Some(permit))` — the engine's ONE permit, held until the guard drops. The caller keeps it in
    ///   the frame that wraps the engine run, so it is released on EVERY exit path — success, failure,
    ///   kill, cancel and a dropped future alike (§0.9 "releases both on subprocess exit").
    /// * `Err(LaneError::PoolClosed)` — unreachable by construction: these semaphores are never closed
    ///   (this type exposes no close, unlike [`Pool`]'s `#[cfg(test)]` seam). It is surfaced rather than
    ///   unwrapped to keep the §0.9 no-panic pool path, and the caller fails that ONE item — fail-CLOSED,
    ///   because serialisation is a correctness control: running an office job whose serialisation could
    ///   not be established is precisely the profile corruption §0.9 forbids.
    ///
    /// [Build-Session-Entscheidung: P4.22]
    pub async fn acquire(&self, key: &K) -> Result<Option<SemaphorePermit<'_>>, LaneError> {
        let Some(lane) = self.lanes.get(key) else {
            return Ok(None);
        };
        lane.acquire()
            .await
            .map(Some)
            .map_err(|_closed| LaneError::PoolClosed)
    }

    /// The number of allocated lanes — the `#[cfg(test)]` observability seam the registry's own tests read
    /// (a `Semaphore` set has no other honest shape to assert on). Absent from production, so it needs no
    /// dead-code entry. [Build-Session-Entscheidung: P4.22]
    #[cfg(test)]
    pub(crate) fn lane_count(&self) -> usize {
        self.lanes.len()
    }
}

/// The §0.9-owned **video re-encode** parallelism cap — the concrete number behind the §0.9 engine table's
/// "**FFmpeg** (video re-encode) | **low — 1–2**" row, and the single source of it: a named `pub const` in
/// this §0.9 pool module, co-located with the timeout / watchdog / memory set, so the §6.7.2 test harness
/// imports the same number production uses rather than hard-coding a literal (the `MAX_LO_CONCURRENCY`
/// "single source … never hard-coded" convention §0.9 states for the serialised degree).
///
/// **Why `2` and not `1`** [Build-Session-Entscheidung: P4.21] — §0.9 states the band as "low — 1–2" and
/// then works the example itself one bullet down: "video re-encode runs at `min(global_degree, 2)`". So `2`
/// is §0.9's own resolution of its band, not a free pick. The rationale is §0.9's too: a video re-encode is
/// "already the slowest op" and libx264/libvpx saturate most cores from INSIDE one process, so one or two
/// of them already is the machine — "video re-encode is effectively serial-ish on typical machines, by
/// design — not a bug". On a small host the global degree caps it further; every term of
/// [`Pool::effective_degree`] only ever caps downward.
pub const MAX_VIDEO_REENCODE_CONCURRENCY: usize = 2;

/// The §0.9 **per-engine parallelism rule** for one job — the engine-table row the job's engine declares
/// for it (the §3.2.2 `Engine::parallelism` seam), and the ONLY way a caller obtains a `per_engine_cap` for
/// [`Pool::effective_degree`]. Modelling §0.9's rows as a closed enum instead of a bare number is what
/// keeps §0.9 the single source: a lane cannot be handed an invented cap, and a new row is a §0.9 edit plus
/// a variant here, never a literal at a call site.
///
/// **The §0.9 engine table, in full, and where each row is realised**
/// [Build-Session-Entscheidung: P4.21]:
///
/// | §0.9 table row | realised as |
/// |---|---|
/// | FFmpeg (video re-encode) — "low — 1–2" | [`EngineParallelism::VideoReencode`] → [`MAX_VIDEO_REENCODE_CONCURRENCY`] |
/// | FFmpeg (audio / extract-audio / remux) — "up to global degree" | [`EngineParallelism::UpToGlobalDegree`] |
/// | image core (§3.5.5) — "up to global degree" | [`EngineParallelism::UpToGlobalDegree`] |
/// | poppler / pandoc — "up to global degree" | [`EngineParallelism::UpToGlobalDegree`] |
/// | native CSV/TSV (§3.5.6) — "up to global degree (worker threads)" | [`EngineParallelism::UpToGlobalDegree`] |
/// | LibreOffice — "serialised — exactly 1" | **deliberately NOT a variant here** — see below |
///
/// **Why LibreOffice's row is deliberately absent.** §0.9 gives the serialised engine its own `[DECIDED]`
/// enforcement mechanism — a dedicated single-permit semaphore per serialised engine, where such a job
/// "must **acquire BOTH** the global degree semaphore **and** that engine's single-permit semaphore",
/// while "non-serialised engines acquire only the global degree permit". A serialised job therefore takes
/// exactly ONE global permit like any other job, and its serialisation is the second, engine-private
/// semaphore (built at P4.22) reached through `EngineDescriptor.serialised_only` (the P4.5 data path).
/// Routing it through THIS term instead would give it `per_engine_cap = Some(1)`, hence a
/// [`Pool::slot_weight`] of the whole degree — one office conversion would consume every global permit and
/// nothing else could run beside it. That is neither what §0.9's mechanism says nor what §0.9's "a batch
/// mixing engines respects each engine's own cap **within the shared global bound**" allows. So LibreOffice
/// is `UpToGlobalDegree` on THIS axis and serialised on THAT one — two orthogonal axes, one source each.
///
/// INTERNAL — never on the IPC wire (no `serde`/`specta`), mirroring the sibling fieldless engine-seam
/// enums (`crate::engines::EngineKind`); `Copy` is free for a fieldless enum, `PartialEq`/`Eq` for the
/// dispatch + test assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineParallelism {
    /// §0.9 "up to global degree" — the table's majority row: FFmpeg audio / extract-audio / remux, the
    /// image worker, poppler, pandoc and the native CSV/TSV engine. Imposes NO per-engine term, so the job
    /// is bounded by the global degree and the §1.10 memory factor alone. Also the row of a light
    /// sub-invocation of an otherwise-capped engine (the §3.2.1 `ffprobe` probe).
    UpToGlobalDegree,
    /// §0.9 "low — 1–2" — a CPU-bound video RE-ENCODE (never a remux, which §0.9 puts on the row above).
    /// Caps the job at [`MAX_VIDEO_REENCODE_CONCURRENCY`] concurrent runs.
    VideoReencode,
}

impl EngineParallelism {
    /// Project the §0.9 row onto [`Pool::effective_degree`]'s `per_engine_cap` term — `None` for "up to
    /// global degree" (no per-engine bound at all, so the `min` is decided by the global degree and the
    /// §1.10 memory cap), a concrete cap otherwise. This is the ONE place a §0.9 row becomes a number.
    /// [Build-Session-Entscheidung: P4.21]
    #[must_use]
    pub const fn per_engine_cap(self) -> Option<usize> {
        match self {
            EngineParallelism::UpToGlobalDegree => None,
            EngineParallelism::VideoReencode => Some(MAX_VIDEO_REENCODE_CONCURRENCY),
        }
    }
}

/// The failure modes of the §0.9 permit paths. Authored (P3.3) for the in-core `spawn_blocking` lane
/// alone, it has since OUTGROWN that scope twice — [`Pool::run_subprocess`] (P4.20) and
/// [`SerialisedLanes::acquire`] (P4.22) both produce it — so the type doc names the general shape and
/// each variant states which producers can actually reach it. INTERNAL — never on the IPC wire (no
/// `serde`/`specta`); the §1.7/§2.8 caller (P3.46) maps it onto a per-item `Failed`, so a lane failure is
/// always ONE item's failure, never a pool-wide fault. [Build-Session-Entscheidung: P3.3] `Debug` + the
/// test-assertion `PartialEq`/`Eq`; NO `Clone` (the caller matches/maps it, never clones) — mirroring the
/// internal `crate::engines` descriptor types.
#[derive(Debug, PartialEq, Eq)]
pub enum LaneError {
    /// A semaphore this path acquires from was closed, so no permit can be granted — the GLOBAL-degree one
    /// on either [`Pool`] lane, or a [`SerialisedLanes`] engine lane (P4.22). Unreachable while the app
    /// runs in both cases: the pool lives for the process lifetime and is closed only by a `#[cfg(test)]`
    /// seam, and `SerialisedLanes` exposes no close at all. Surfaced without a panic to keep the §0.9
    /// no-panic pool path; the §1.7/§2.8 caller maps it to ONE item's failure — fail-CLOSED on the
    /// serialised lane, where the permit IS the correctness control.
    PoolClosed,
    /// The worker closure panicked; `tokio::task::spawn_blocking` caught the unwind (§2.13 catch_unwind
    /// semantics — the worker is not killed). Because a `spawn_blocking` task cannot be abort-cancelled, a
    /// `JoinError` from this lane is ALWAYS a captured panic. The permit was released on unwind, so the pool
    /// is NOT poisoned — the next acquire succeeds. Rests on the workspace default `panic = "unwind"`.
    Panicked,
}

/// The §0.9 bounded pool. In P3 it carries the LIVE global-degree permit model + the in-core lane; P4.20
/// EXPANDED it with the subprocess lane + the §1.10 memory-adaptive effective degree, and P4.21 with the
/// §0.9 per-engine cap term. **P4.22 deliberately did NOT expand it**: the `serialised_only` single-permit
/// semaphores live in the standalone [`SerialisedLanes`] the tier-2 registry owns, precisely so this
/// app-managed type stays un-generic and names no `crate::engines` key (the §0.9 `[DECIDED — P4.22]`
/// decision (a); see the module tier note). [Build-Session-Entscheidung: P3.3] `Clone` = a cheap `Arc`
/// bump sharing the SAME global semaphore, so the one app-wide pool is handed to every executor by value
/// (the tokio-pool convention); `Debug` for diagnostics. NOT `Copy` (owns an `Arc`); NOT `PartialEq` (a
/// semaphore is not comparable). `global` is `Arc<Semaphore>` because `acquire_owned` — needed to move a
/// `'static` permit into the `'static` blocking closure — is defined on `Arc<Semaphore>`.
#[derive(Debug, Clone)]
pub struct Pool {
    /// The global-degree permit source (§0.9): `degree` permits. Every job — subprocess (P4) or
    /// InProcessNative (P3) — takes a SLOT here before running, which is [`Pool::slot_weight`] permits:
    /// one at full effective degree, more under §1.10 memory pressure (P4.20).
    global: Arc<Semaphore>,
    /// The resolved global degree (§0.9). Stored because `Semaphore::available_permits` fluctuates as
    /// permits are held; the P4.20/P4.21 effective-degree math + the §1.11 batch bar read this configured
    /// value.
    degree: usize,
    /// The §1.10 available-memory reading behind the memory-adaptive factor — a plain fn pointer, not a
    /// captured closure, so [`Pool`] stays `Clone`/`Debug` and allocation-free. `Pool::new` wires the real
    /// `crate::platform::available_memory_bytes`; [`Pool::with_degree`] wires [`no_memory_cap`] so a pinned
    /// harness degree is not perturbed by the host's live memory, and the tests inject deterministic
    /// readings. A probe returning `None` means "unknown ⇒ impose no memory cap" (the never-break bias the
    /// probe's own doc states). [Build-Session-Entscheidung: P4.20]
    memory: fn() -> Option<u64>,
}

impl Pool {
    /// Construct the pool sized to this machine's §0.9 global concurrency degree, with the §1.10
    /// memory-adaptive factor reading real host memory. This is the PRODUCTION constructor.
    /// [Build-Session-Entscheidung: P3.3] [Build-Session-Entscheidung: P4.20]
    pub fn new() -> Self {
        Self::with_degree_and_memory(
            resolve_global_degree(),
            crate::platform::available_memory_bytes,
        )
    }

    /// Construct the pool at an explicit degree — the §6.7.2 harness pins a deterministic degree. The
    /// degree is floored at 1 (`max(1)`) so the global `Semaphore` always has ≥1 permit (a zero-permit pool
    /// would deadlock every job). [Build-Session-Entscheidung: P3.3]
    ///
    /// **The §1.10 memory factor is OFF here, deliberately** [Build-Session-Entscheidung: P4.20]: a
    /// constructor whose whole purpose is a *pinned, deterministic* degree must not have that degree
    /// silently re-capped — or a watermark pause introduced — by whatever the host's memory happens to be
    /// while a test runs (test-strategy §7: engineer determinism, never absorb it with retries). The real
    /// probe is wired by [`Pool::new`]; the memory path has its own tests, which inject deterministic
    /// readings.
    pub fn with_degree(degree: usize) -> Self {
        Self::with_degree_and_memory(degree, no_memory_cap)
    }

    /// The shared constructor behind [`Pool::new`] and [`Pool::with_degree`], parameterised on the §1.10
    /// memory probe so the memory-adaptive paths are testable against deterministic readings.
    /// [Build-Session-Entscheidung: P4.20]
    fn with_degree_and_memory(degree: usize, memory: fn() -> Option<u64>) -> Self {
        let degree = degree.max(1);
        Pool {
            global: Arc::new(Semaphore::new(degree)),
            degree,
            memory,
        }
    }

    /// The `#[cfg(test)]` door onto [`Self::with_degree_and_memory`] for tests in OTHER modules — the
    /// `crate::engines` cancel-during-pause regression drives the gate through the real `dispatch`, so it
    /// needs a pool with a pinned memory reading. Placed AFTER the fn it wraps, never between that fn and
    /// its doc block (the `inserting-a-module-hijacks-the-preceding-doc-comment` class).
    /// [Build-Session-Entscheidung: P4.20]
    #[cfg(test)]
    pub(crate) fn with_degree_and_memory_for_test(
        degree: usize,
        memory: fn() -> Option<u64>,
    ) -> Self {
        Self::with_degree_and_memory(degree, memory)
    }

    /// The §0.9/§1.10 **effective parallelism** for one engine — `min(global_degree, per_engine_cap,
    /// memory_based_cap)`, the three-term form §1.10 mandates ("the effective §0.9 concurrency degree
    /// adapts to available memory … down to serial"). Each term only ever caps DOWNWARD, never upward, and
    /// the result is floored at 1 (a zero degree would dispatch nothing at all).
    ///
    /// * `global_degree` — this pool's configured §0.9 degree.
    /// * `per_engine_cap` — the §0.9 per-engine term, LIVE since P4.21: it arrives as an
    ///   [`EngineParallelism`] row (the §0.9 engine table) projected through
    ///   [`EngineParallelism::per_engine_cap`], so the number always comes from §0.9 and never from a call
    ///   site. `None` is "up to global degree" — no per-engine bound. (LibreOffice's "serialised — exactly
    ///   1" row is NOT this term; see [`EngineParallelism`] for why.)
    /// * `memory_based_cap` — `available_memory / MEMORY_PER_SLOT_BYTES`, or NO cap when the probe cannot
    ///   read a value (the never-break bias).
    ///
    /// Pure over the probe's reading, so the formula is unit-tested against deterministic memory values.
    /// [Build-Session-Entscheidung: P4.20]
    #[must_use]
    pub fn effective_degree(&self, per_engine_cap: Option<usize>) -> usize {
        let memory_cap = (self.memory)().map(|available| {
            // integer division floors, so a machine with less than one slot's budget yields 0 → the
            // `max(1)` below makes that SERIAL, which is exactly §1.10's "down to serial"
            usize::try_from(available / MEMORY_PER_SLOT_BYTES).unwrap_or(usize::MAX)
        });
        [Some(self.degree), per_engine_cap, memory_cap]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(self.degree)
            .max(1)
    }

    /// How many of the `degree` global permits ONE slot must take so that at most
    /// [`Self::effective_degree`] slots can be held at once — the mechanism that makes the effective degree
    /// actually BITE rather than merely be computed.
    ///
    /// The global semaphore is fixed at `degree` permits (resizing it under a live pool would race with
    /// in-flight holders), so the effective degree is enforced by WEIGHT instead: a job takes
    /// `ceil(degree / effective)` permits, which admits `floor(degree / weight) ≤ effective` of them
    /// concurrently. At full effective degree the weight is 1 and nothing changes; at effective 1 the weight
    /// is the whole degree, i.e. SERIAL — §1.10's "down to serial", enforced. Where `degree` is not a
    /// multiple of `effective` the integer weight admits slightly FEWER than the cap (degree 4, effective 3
    /// ⇒ weight 2 ⇒ 2 concurrent): under-admitting is the safe direction for a memory gate, and the
    /// alternative — a resizable permit pool — would buy one extra slot at the price of a race.
    ///
    /// **The same weight now carries the §0.9 PER-ENGINE term (P4.21), and its two side effects are
    /// deliberate** [Build-Session-Entscheidung: P4.21]. (1) A capped job takes MORE than one permit, so a
    /// video re-encode at degree 4 holds 2 of them and leaves 2 for everything else. That is not a
    /// distortion of §0.9 but its own rationale made mechanical: the 1–2 cap exists "*because* one or two
    /// FFmpeg processes already use most cores", so charging such a job ~half the machine's slot budget is
    /// exactly the pressure §0.9 describes. No cap is ever exceeded (`min` is downward-only, and
    /// `floor(degree / weight) ≤ effective` holds term-independently) — only the total in-flight count is
    /// conservative, the safe direction for a CPU gate as much as for a memory one. (2) At a degree that is
    /// not a multiple of the cap the same rounding bites: degree 3 with a cap of 2 ⇒ weight 2 ⇒ ONE
    /// concurrent re-encode. Under-admitting again, and squarely inside §0.9's stated intent that "video
    /// re-encode is effectively serial-ish on typical machines, by design — not a bug".
    ///
    /// Never zero and never above `degree`: `effective_degree` is floored at 1, so the weight is in
    /// `1..=degree` and the semaphore can always satisfy it (a weight above the total would deadlock).
    /// [Build-Session-Entscheidung: P4.20]
    fn slot_weight(&self, per_engine_cap: Option<usize>) -> u32 {
        let weight = self.degree.div_ceil(self.effective_degree(per_engine_cap));
        u32::try_from(weight).unwrap_or(u32::MAX).max(1)
    }

    /// The §1.10 **high-memory watermark gate** — hold a NEW item back while available memory is below
    /// [`HIGH_MEMORY_WATERMARK_BYTES`], re-reading every [`MEMORY_WATERMARK_POLL`], and let it through as
    /// soon as memory frees. Items already dispatched are never touched, so "in-flight items finish" is
    /// satisfied by construction: this gate sits at the dispatch entry, not around the work.
    ///
    /// **Bounded by [`MEMORY_PAUSE_MAX`], never open-ended** — §2.12.3's never-break floor applies to this
    /// throttle exactly as it does to the privilege-drop legs: a host that simply never rises above the
    /// watermark must not hang the batch forever. On expiry the item dispatches anyway and the other half
    /// of the §1.10 policy (the per-item memory ceiling → `Failed(TooBig)`, batch continues) takes over.
    /// An unreadable probe imposes no pause at all.
    ///
    /// **The caller invokes this BEFORE it starts the item's timed lane — it is deliberately NOT inside
    /// [`Pool::run_in_core`]/[`Pool::run_subprocess`].** §1.7 wraps each lane in a wall-clock timeout
    /// (`NATIVE_CSV_TSV_TIMEOUT`, and the P4.12 subprocess watchdog bounds), so a pause *inside* the lane
    /// would be spent out of the ENGINE's budget: a low-memory host could silently turn a slow-but-
    /// progressing conversion into `Failed(EngineHang)` — a defence-in-depth throttle becoming the reason a
    /// conversion fails, precisely what §2.12.3's never-break floor forbids and what the ceiling above
    /// exists to prevent. Placing it at the §1.7 dispatch entry also matches §1.10's own wording: the
    /// watermark pauses **dispatch** of new items, it does not shorten a running engine.
    /// [Build-Session-Entscheidung: P4.20]
    pub async fn await_dispatch_headroom(&self) {
        let deadline = tokio::time::Instant::now() + MEMORY_PAUSE_MAX;
        while (self.memory)().is_some_and(|available| available < HIGH_MEMORY_WATERMARK_BYTES) {
            if tokio::time::Instant::now() >= deadline {
                return;
            }
            tokio::time::sleep(MEMORY_WATERMARK_POLL).await;
        }
    }

    /// The §0.9 native-CSV/TSV / §1.7 InProcessNative in-core permit lane. Acquire a slot — `slot_weight`
    /// global-degree permits, which is ONE at full effective degree and more under §1.10 memory pressure
    /// (P4.20) or the job's §0.9 per-engine cap (P4.21) — run `task` on a dedicated `spawn_blocking` worker
    /// thread (so the synchronous loop never blocks the Tokio runtime that drives the subprocess engines +
    /// IPC), and release the permit on completion, on a worker panic, AND on abandonment. A caught worker
    /// panic → `Err(LaneError::Panicked)`
    /// (never re-raised: re-raising would panic the pool-driver task and violate §0.9 panic isolation). The
    /// caller captures its own `progress_tx` (P3.43) + `CancellationToken` (P3.44) inside `task`, and P3.45
    /// wraps this future in `tokio::time::timeout` (§1.7). [Build-Session-Entscheidung: P3.3]
    ///
    /// **`parallelism`** is the job's §0.9 engine-table row (P4.21), taken as an [`EngineParallelism`]
    /// rather than a bare `Option<usize>` so a caller cannot invent a cap §0.9 does not state. The sole
    /// engine that reaches this lane — native CSV/TSV (§3.5.6) — is `UpToGlobalDegree` per §0.9, but the
    /// parameter is not hard-coded here: the §1.7 dispatch passes what the job's `Engine::parallelism`
    /// declared, so this lane never has to know which engine it is running.
    /// [Build-Session-Entscheidung: P4.21]
    pub async fn run_in_core<F, R>(
        &self,
        parallelism: EngineParallelism,
        task: F,
    ) -> Result<R, LaneError>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        // The permit lives in THIS async frame (not moved into the closure): dropping the future then
        // releases the permit at once while the abandoned blocking task detaches and runs on — the §1.7
        // wedged-uninterruptible-read design (the abandoned thread must not hold a global-degree permit, or
        // a handful of wedges would starve the pool). Moving it into the closure would keep a wedged
        // thread's permit held until that thread finishes. [Build-Session-Entscheidung: P3.3]
        // The §1.10 effective degree AND the §0.9 per-engine cap are enforced by WEIGHT (see
        // `slot_weight`): under memory pressure — or for a capped engine — one slot takes several global
        // permits, so fewer run at once. The row comes from the caller's `Engine::parallelism` (P4.21),
        // never from a literal here.
        let _permit = self
            .global
            .clone()
            .acquire_many_owned(self.slot_weight(parallelism.per_engine_cap()))
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

    /// The §0.9 **subprocess** permit lane — the half of the pool P3.3 shelled and P4.20 filled. Acquire
    /// a slot — `slot_weight` permits, which is ONE at full effective degree and more under §1.10 memory
    /// pressure or the job's §0.9 per-engine cap (P4.21) — await the caller's engine run, and release on
    /// EVERY exit path. It is the bound behind
    /// §0.9's "a bounded engine-subprocess pool governs how many engine processes run at once".
    ///
    /// **The §1.10 watermark pause is NOT here**, and a P4.32 caller must not add it: it lives at the §1.7
    /// dispatch entry ([`Pool::await_dispatch_headroom`], called by `crate::engines::dispatch`) precisely
    /// so it stays outside each lane's wall-clock timeout. Pausing inside this lane would spend the
    /// throttle out of the ENGINE's budget — the §2.12.3 never-break defect the placement exists to avoid.
    ///
    /// **Why a caller-supplied future rather than a closure like [`Pool::run_in_core`]** — the §0.7 tier
    /// rule, not a style choice. `crate::isolation::run_confined` and the §1.7 `bounded_confined_run` that
    /// wraps it are tier-2 and borrow their arguments (`&EngineInvocation`, `&Path`, a non-`Send`
    /// `on_progress`), so the `F: FnOnce() -> R + Send + 'static` shape the in-core lane needs cannot hold
    /// here. Taking `impl Future<Output = R>` and returning the caller's own `R` keeps this tier-3 leaf
    /// from naming a single tier-2 type, exactly as `run_in_core<F, R>` does — the caller instantiates `R`
    /// as its own `ConfinedRun`/`InvocationResult`. [Build-Session-Entscheidung: P4.20]
    ///
    /// **The permit is released on all four failure shapes, which is the P4.18.2 forward obligation.**
    /// Because `_permit` is bound in THIS async frame rather than inside `task`, it drops when the lane
    /// returns (clean exit or engine failure), when the §2.12.3 memory cap kills the engine, when the §1.7
    /// watchdog reaps a hang, and when the caller DROPS the lane future (cancel / quit). No path can leave
    /// the pool a permit down — the `pool::tests` permit regressions assert over all four.
    ///
    /// **`LaneError::Panicked` is unreachable on this lane** and that is by construction, not an oversight:
    /// it denotes a `spawn_blocking` `JoinError`, and an awaited future has no `catch_unwind` boundary — a
    /// panic inside `task` propagates to the caller's own §2.13 per-item boundary. The only error this lane
    /// produces is `PoolClosed`. [Build-Session-Entscheidung: P4.20]
    ///
    /// **`parallelism`** is the job's §0.9 engine-table row (P4.21) — an [`EngineParallelism`] rather than
    /// the raw `Option<usize>` P4.20 shipped, so §0.9 stays the only place a cap number exists. A P4.32
    /// caller passes what the job's `Engine::parallelism` declared; the LibreOffice "serialised — exactly
    /// 1" row is NOT expressed through this argument (its dedicated single-permit semaphore is P4.22 — see
    /// [`EngineParallelism`]). [Build-Session-Entscheidung: P4.21]
    pub async fn run_subprocess<F, R>(
        &self,
        parallelism: EngineParallelism,
        task: F,
    ) -> Result<R, LaneError>
    where
        F: std::future::Future<Output = R>,
    {
        // The §1.10 effective degree AND the §0.9 per-engine cap are enforced by WEIGHT (see
        // `slot_weight`); the row arrives from the caller's `Engine::parallelism`, never a literal here.
        let _permit = self
            .global
            .acquire_many(self.slot_weight(parallelism.per_engine_cap()))
            .await
            .map_err(|_closed| LaneError::PoolClosed)?;
        Ok(task.await)
    }

    /// Test-only seam: close the global semaphore so the next acquire fails — exercises the `PoolClosed`
    /// arm (unreachable in the running app). `cfg(test)`, so it is absent from production.
    /// [Build-Session-Entscheidung: P3.3]
    #[cfg(test)]
    fn close(&self) {
        self.global.close();
    }
}

/// The §1.10 probe [`Pool::with_degree`] wires: "no reading available" — hence no memory cap and no
/// watermark pause, the never-break default. A pinned-degree pool is deterministic by construction.
/// [Build-Session-Entscheidung: P4.20]
fn no_memory_cap() -> Option<u64> {
    None
}

/// A §1.10 probe pinned permanently BELOW the watermark — the deterministic "memory-constrained host" the
/// cross-module tests need (`crate::engines`' cancel-during-pause regression). `#[cfg(test)]`, so it cannot
/// reach production. [Build-Session-Entscheidung: P4.20]
#[cfg(test)]
pub(crate) fn below_watermark_for_test() -> Option<u64> {
    Some(HIGH_MEMORY_WATERMARK_BYTES / 2)
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
/// the clamp to `[1, 4]` + the per-engine caps (§0.9 table, P4.21) keep heavy engines conservative regardless
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
                pool.run_in_core(EngineParallelism::UpToGlobalDegree, move || {
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
            .run_in_core(EngineParallelism::UpToGlobalDegree, || {
                std::thread::current().id()
            })
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
            .run_in_core(EngineParallelism::UpToGlobalDegree, || {
                std::hint::black_box(Option::<u32>::None).unwrap()
            })
            .await;
        assert_eq!(
            panicked,
            Err(LaneError::Panicked),
            "§0.9/§2.13: a worker panic surfaces as a clean LaneError::Panicked, never a re-raised pool-path panic"
        );
        // [Test-Change: P4.21 — old-obsolete+new-correct, §0.9] the OLD call shape is obsolete (both pool
        // lanes now take the job's §0.9 `EngineParallelism` row) and the NEW one is correct (this lane
        // carries no per-engine cap, §0.9's native-CSV/TSV row); rustfmt then split the chain, so what G70
        // sees as removed is the chain OPENER line, not an assertion — and the message argument below is
        // replaced too, its TEXT unchanged with only its indentation shifted by the reflow (verified
        // byte-exactly against HEAD, not assumed). Nothing was removed, relaxed, skipped or flipped
        // red→green (the P3.43 G70 over-flag precedent) — the permit-release property this test proves is
        // untouched.
        let recovered = pool
            .run_in_core(EngineParallelism::UpToGlobalDegree, || 42_u32)
            .await
            .expect(
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
            pool.run_in_core(EngineParallelism::UpToGlobalDegree, || 1_u32).await,
            Err(LaneError::PoolClosed),
            "§0.9: acquiring on a closed semaphore maps to PoolClosed, never an unwrap/panic on the no-panic pool path"
        );
    }

    // §6.4.1 (G15): the §0.9 subprocess-watchdog baseline bounds (P4.12) hold their ordering invariants — the
    // poll cadence is far below the no-progress threshold (so a hang is detected within ~one poll of the
    // threshold, never masked by a coarse poll), the no-progress threshold is below the light-engine wall-clock
    // (a stalled light engine is reaped by no-progress before its total budget), and video's wall-clock is the
    // most generous ("generous for video, tight for the light engines", §0.9). This references every watchdog
    // const so they stay non-dead in the test build and pins the pre-calibration ordering the §6.7.2 harness
    // will later import. [Build-Session-Entscheidung: P4.12]
    #[test]
    fn subprocess_watchdog_baseline_bounds_hold_their_ordering() {
        assert!(
            WATCHDOG_POLL_INTERVAL < NO_PROGRESS_TIMEOUT,
            "§0.9: the poll cadence must be well under the no-progress threshold (else a hang is only \
             detected a full threshold-plus-poll late)"
        );
        assert!(
            NO_PROGRESS_TIMEOUT < SUBPROCESS_WALL_CLOCK_DEFAULT,
            "§0.9: a stalled light engine is reaped by the no-progress leg before its total wall-clock budget"
        );
        assert!(
            SUBPROCESS_WALL_CLOCK_DEFAULT < VIDEO_WALL_CLOCK,
            "§0.9: the video wall-clock is 'generous for video', above the tight light-engine default"
        );
        assert!(
            WATCHDOG_POLL_INTERVAL > Duration::ZERO,
            "§0.9: a zero poll interval would busy-spin the watchdog"
        );
    }

    // ─── P4.20: the §1.10 memory-adaptive factor + the subprocess lane ──
    //
    // Every memory-dependent test injects its OWN probe fn (each with its own `static`, so the suite's
    // parallel tests never share a reading) — the §1.10 numbers are then exact inputs, not whatever the CI
    // host happens to have free. The pauses run under a PAUSED tokio clock, so a 30 s ceiling costs no wall
    // time and no test depends on a real sleep landing (test-strategy §7: engineer determinism).

    const GIB: u64 = 1024 * 1024 * 1024;

    fn mem_unknown() -> Option<u64> {
        None
    }
    fn mem_4gib() -> Option<u64> {
        Some(4 * GIB)
    }
    fn mem_1gib() -> Option<u64> {
        Some(GIB)
    }
    fn mem_quarter_slot() -> Option<u64> {
        Some(MEMORY_PER_SLOT_BYTES / 4)
    }

    // §6.4.1 (G15): the §0.9/§1.10 three-term formula `min(global_degree, per_engine_cap, memory_cap)`.
    // Each term is proven to cap DOWNWARD and only downward, the memory term is proven to be the
    // `available / MEMORY_PER_SLOT_BYTES` floor, and an unknown reading is proven to impose NO cap (the
    // never-break bias). The last row is §1.10's headline: too little memory for even one slot ⇒ SERIAL.
    #[test]
    fn effective_degree_is_the_three_term_minimum() {
        // (degree, per-engine cap, probe, expected, why)
        type DegreeCase = (
            usize,
            Option<usize>,
            fn() -> Option<u64>,
            usize,
            &'static str,
        );
        let cases: [DegreeCase; 8] = [
            (
                4,
                None,
                mem_unknown,
                4,
                "no cap + unknown memory ⇒ the global degree stands",
            ),
            (
                4,
                None,
                mem_4gib,
                4,
                "4 GiB ⇒ 8 slots, above the degree ⇒ the degree still stands",
            ),
            (
                4,
                Some(2),
                mem_4gib,
                2,
                "the per-engine cap is the smallest term",
            ),
            (
                4,
                None,
                mem_1gib,
                2,
                "1 GiB / 512 MiB ⇒ 2 ⇒ the memory cap is the smallest term",
            ),
            (
                2,
                Some(3),
                mem_4gib,
                2,
                "a per-engine cap ABOVE the degree never raises it",
            ),
            (
                4,
                Some(3),
                mem_1gib,
                2,
                "all three terms present ⇒ the minimum wins",
            ),
            (
                4,
                None,
                mem_quarter_slot,
                1,
                "§1.10: under one slot's budget ⇒ serial, never 0",
            ),
            (
                1,
                Some(4),
                mem_4gib,
                1,
                "a degree-1 pool stays serial whatever the other terms say",
            ),
        ];
        for (degree, cap, probe, want, why) in cases {
            let pool = Pool::with_degree_and_memory(degree, probe);
            assert_eq!(
                pool.effective_degree(cap),
                want,
                "§0.9/§1.10: {why} (degree {degree}, cap {cap:?})"
            );
        }
    }

    // §6.4.1 (G15): `Pool::with_degree` — the deterministic harness constructor — imposes NO memory cap, so
    // a pinned degree is never silently re-capped by the host's live memory while a test runs. The
    // production `Pool::new` is the constructor that reads real memory.
    #[test]
    fn with_degree_pins_the_degree_without_a_memory_cap() {
        let pool = Pool::with_degree(4);
        assert_eq!(
            pool.effective_degree(None),
            4,
            "a pinned-degree pool is deterministic — the §1.10 factor is off"
        );
        assert_eq!((pool.memory)(), None, "with_degree wires the no-cap probe");
        // Compared as fn POINTERS, never by calling both and comparing readings: two live memory reads
        // legitimately differ (the host allocates between them), so a value comparison would be a
        // nondeterministic test of a deterministic property (test-strategy §7 — engineer determinism out,
        // never absorb it). The pointer identity IS the property: which probe the constructor wired.
        assert!(
            std::ptr::fn_addr_eq(
                Pool::new().memory,
                crate::platform::available_memory_bytes as fn() -> Option<u64>
            ),
            "§1.10: the PRODUCTION constructor wires the real platform probe"
        );
    }

    static LOW_THEN_HIGH_READS: AtomicUsize = AtomicUsize::new(0);
    /// Below the watermark for the first three reads, then above it — so "the pause ENDED when memory
    /// freed" is proven by the read count, with no cross-task timing to race on.
    fn mem_low_then_high() -> Option<u64> {
        let n = LOW_THEN_HIGH_READS.fetch_add(1, Ordering::SeqCst);
        Some(if n < 3 {
            HIGH_MEMORY_WATERMARK_BYTES / 2
        } else {
            4 * GIB
        })
    }

    // §6.4.1 (G15): the §1.10 effective degree is ENFORCED, not merely computed — `slot_weight` is the
    // mechanism. One slot takes `ceil(degree / effective)` of the `degree` global permits, so the admitted
    // concurrency `floor(degree / weight)` never exceeds the effective degree and reaches 1 (serial) when
    // memory allows only one slot. The last column pins the deliberate under-admission where `degree` is not
    // a multiple of `effective` (the safe direction for a memory gate).
    #[test]
    fn the_effective_degree_is_enforced_by_the_slot_weight() {
        // (degree, probe, expected effective, expected weight, expected admitted concurrency)
        type WeightCase = (usize, fn() -> Option<u64>, usize, u32, usize);
        let cases: [WeightCase; 5] = [
            (4, mem_unknown, 4, 1, 4),
            (4, mem_4gib, 4, 1, 4),
            (4, mem_1gib, 2, 2, 2),
            (4, mem_quarter_slot, 1, 4, 1),
            (3, mem_1gib, 2, 2, 1),
        ];
        for (degree, probe, effective, weight, admitted) in cases {
            let pool = Pool::with_degree_and_memory(degree, probe);
            assert_eq!(
                pool.effective_degree(None),
                effective,
                "degree {degree}: effective"
            );
            assert_eq!(
                pool.slot_weight(None),
                weight,
                "degree {degree}: slot weight"
            );
            assert_eq!(
                degree / weight as usize,
                admitted,
                "§1.10: degree {degree} at effective {effective} admits {admitted} concurrent slot(s)"
            );
            assert!(
                degree / weight as usize <= effective,
                "§1.10: the weight never admits MORE than the effective degree"
            );
        }
    }

    // §6.4.x (G15): the enforcement, end to end on a real semaphore — under memory pressure that allows one
    // slot, a degree-4 pool admits exactly ONE lane: the first takes all four permits and a second cannot
    // enter until it releases. This is §1.10's "down to serial" as observable behaviour rather than
    // arithmetic. Deterministic: proven by permit accounting + a pending second lane, never by a sleep race.
    #[tokio::test]
    async fn under_memory_pressure_a_degree_four_pool_admits_one_lane_at_a_time() {
        let pool = Pool::with_degree_and_memory(4, mem_quarter_slot);
        let mut first = Box::pin(pool.run_subprocess(
            EngineParallelism::UpToGlobalDegree,
            std::future::pending::<u32>(),
        ));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut first)
                .await
                .is_err(),
            "the first lane is running (its engine future never completes)"
        );
        assert_eq!(
            pool.global.available_permits(),
            0,
            "§1.10: at effective degree 1 ONE slot consumes the whole degree — that IS the enforcement"
        );
        // The second lane's engine future completes INSTANTLY, so "still pending" can only mean it is
        // blocked acquiring — that is what distinguishes "not admitted" from "admitted and running".
        let mut second =
            Box::pin(pool.run_subprocess(EngineParallelism::UpToGlobalDegree, async { 7_u32 }));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut second)
                .await
                .is_err(),
            "a second lane cannot be admitted while the first holds the whole degree — serial under pressure"
        );
        drop(first);
        assert_eq!(
            second.await.expect("§0.9: the second lane runs once the first releases"),
            7,
            "§1.10: releasing the weighted permits admits the next item — the pause is a throttle, not a wedge"
        );
        assert_eq!(
            pool.global.available_permits(),
            4,
            "both weighted acquisitions returned every permit"
        );
    }

    // §6.4.x (G15): the §1.10 watermark HOLDS a new item while memory is below the mark and RELEASES it the
    // moment memory frees. Proven by the probe's read count (it polled, it did not pass straight through)
    // plus the gate returning — never by a wall-clock sleep. Under a paused clock the polls cost no time.
    #[tokio::test(start_paused = true)]
    async fn the_watermark_pauses_a_new_item_and_resumes_when_memory_frees() {
        LOW_THEN_HIGH_READS.store(0, Ordering::SeqCst);
        let pool = Pool::with_degree_and_memory(2, mem_low_then_high);
        pool.await_dispatch_headroom().await;
        assert!(
            LOW_THEN_HIGH_READS.load(Ordering::SeqCst) >= 4,
            "§1.10: the gate re-read while below the watermark and only proceeded once memory freed — {} \
             read(s) means it passed straight through",
            LOW_THEN_HIGH_READS.load(Ordering::SeqCst)
        );
    }

    // §6.4.x (G15): NEVER-BREAK — a host that stays below the watermark forever must not hang the batch.
    // The gate gives up after MEMORY_PAUSE_MAX and dispatches anyway (the §1.10 per-item ceiling is then
    // the control that bites). Asserted on the PAUSED clock's own elapsed virtual time, so it is exact.
    #[tokio::test(start_paused = true)]
    async fn a_permanently_low_watermark_still_dispatches_within_the_ceiling() {
        let pool = Pool::with_degree_and_memory(2, mem_quarter_slot);
        let start = tokio::time::Instant::now();
        pool.await_dispatch_headroom().await;
        let waited = start.elapsed();
        assert!(
            waited >= MEMORY_PAUSE_MAX,
            "§1.10: the gate genuinely waited for headroom (waited {waited:?})"
        );
        assert!(
            waited < MEMORY_PAUSE_MAX + MEMORY_WATERMARK_POLL * 2,
            "§2.12.3 never-break: the pause is CEILED at MEMORY_PAUSE_MAX, never open-ended (waited \
             {waited:?})"
        );
    }

    // §6.4.x (G15): an unreadable probe imposes NO pause at all — the never-break bias end to end. On a
    // paused clock a single elapsed poll interval would be visible, so zero elapsed proves no wait.
    #[tokio::test(start_paused = true)]
    async fn an_unknown_memory_reading_never_pauses_dispatch() {
        let pool = Pool::with_degree_and_memory(2, mem_unknown);
        let start = tokio::time::Instant::now();
        pool.await_dispatch_headroom().await;
        assert_eq!(
            start.elapsed(),
            Duration::ZERO,
            "§1.10: unknown ⇒ no cap AND no pause; the app runs as if the factor were absent"
        );
    }

    // §6.4.x (G15) PERMIT BOUNDING for the SUBPROCESS lane — the mirror of the in-core lane's barrier test:
    // a Barrier(DEGREE) proves permits are granted up to the full degree (liveness) while the degree bound
    // caps the peak (safety). This is §0.9's "a bounded engine-subprocess pool governs how many engine
    // processes run at once" made executable. Deterministic — no sleep window to overlap.
    #[tokio::test]
    async fn run_subprocess_bounds_concurrency_to_the_global_degree() {
        const DEGREE: usize = 3;
        let pool = Pool::with_degree(DEGREE);
        let barrier = Arc::new(tokio::sync::Barrier::new(DEGREE));
        let concurrent = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..(2 * DEGREE) {
            let pool = pool.clone();
            let barrier = Arc::clone(&barrier);
            let concurrent = Arc::clone(&concurrent);
            let peak = Arc::clone(&peak);
            handles.push(tokio::spawn(async move {
                pool.run_subprocess(EngineParallelism::UpToGlobalDegree, async {
                    let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    barrier.wait().await; // permit-bounded: exactly DEGREE can be here at once
                    concurrent.fetch_sub(1, Ordering::SeqCst);
                })
                .await
                .expect("§0.9: the subprocess lane runs the engine future to completion");
            }));
        }
        for handle in handles {
            handle.await.expect("each spawned lane task joins");
        }

        assert_eq!(
            peak.load(Ordering::SeqCst),
            DEGREE,
            "§0.9: exactly `degree` engine futures run concurrently — the Barrier forces all degree \
             permit-holders to rendezvous (liveness) and the degree-permit bound caps the peak (safety)"
        );
        assert_eq!(
            pool.global.available_permits(),
            DEGREE,
            "§0.9: all global-degree permits are released after the batch drains"
        );
    }

    // §6.4.x (G15) THE P4.18.2 FORWARD OBLIGATION — "a killed/failed engine returns its permit; the pool is
    // not left a permit down after a cap breach, a crash exit or a watchdog hang". Every shape a failing
    // engine reaches the pool through is driven against a DEGREE-1 pool, where a leaked permit is instantly
    // fatal (the next acquire would hang forever) — so the follow-up acquire is the real proof, not the
    // permit count alone. The shapes are exactly those the §2.12.3 memory cap (P4.18.2), a crash exit and
    // the §1.7 watchdog reap produce at this boundary: the lane future RETURNS a failure value, or the lane
    // future is DROPPED mid-flight (cancel / quit / watchdog).
    #[tokio::test]
    async fn a_failing_or_killed_engine_always_returns_its_permit() {
        let pool = Pool::with_degree(1);

        // (1) clean completion
        pool.run_subprocess(EngineParallelism::UpToGlobalDegree, async { 1_u32 })
            .await
            .expect("a clean engine run completes");
        assert_eq!(
            pool.global.available_permits(),
            1,
            "clean exit returns the permit"
        );

        // (2) the engine FAILED — a non-zero exit, a §2.12.3 memory-cap kill, a §1.7 watchdog reap: at this
        // boundary they are all "the future resolved to a failure value", which must not change the permit
        // accounting one bit.
        let failed: Result<Result<u32, &str>, LaneError> = pool
            .run_subprocess(EngineParallelism::UpToGlobalDegree, async {
                Err("engine killed at its memory cap")
            })
            .await;
        assert_eq!(
            failed.expect("the lane itself succeeded — the ENGINE failed"),
            Err("engine killed at its memory cap")
        );
        assert_eq!(
            pool.global.available_permits(),
            1,
            "§0.9/P4.18.2: a failed/killed engine returns its permit — the pool is not left a permit down"
        );

        // (3) the lane future is DROPPED mid-flight — the cancel / quit / watchdog-reap shape. The engine
        // future here never completes, exactly like a wedged engine, so only the frame-bound permit can
        // save the pool. `Box::pin` (not `tokio::pin!`) because the drop must destroy the FUTURE: `pin!`
        // yields a `Pin<&mut _>`, and dropping a reference leaves the referent — and its permit — alive
        // until end of scope, which would make this leg silently vacuous.
        let mut abandoned = Box::pin(pool.run_subprocess(
            EngineParallelism::UpToGlobalDegree,
            std::future::pending::<u32>(),
        ));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut abandoned)
                .await
                .is_err(),
            "the wedged engine future is still pending, as the shape requires"
        );
        assert_eq!(
            pool.global.available_permits(),
            0,
            "the wedged engine is HOLDING the single permit — so the drop below is what must return it"
        );
        drop(abandoned);
        assert_eq!(
            pool.global.available_permits(),
            1,
            "§1.7/P4.18.2: dropping the lane future (cancel / quit / watchdog reap) releases the permit"
        );

        // The load-bearing proof: on a DEGREE-1 pool the next item can only run if every shape above
        // genuinely returned its permit.
        let recovered = pool
            .run_subprocess(EngineParallelism::UpToGlobalDegree, async { 42_u32 })
            .await
            .expect("§0.9: the single permit survived every failure shape");
        assert_eq!(recovered, 42);
    }

    // §6.4.x (G15): the subprocess lane maps a closed pool onto the same clean `PoolClosed` the in-core lane
    // does — never an unwrap/panic on the §0.9 no-panic pool path.
    #[tokio::test]
    async fn a_closed_pool_surfaces_pool_closed_on_the_subprocess_lane_too() {
        let pool = Pool::with_degree(2);
        pool.close();
        assert_eq!(
            pool.run_subprocess(EngineParallelism::UpToGlobalDegree, async { 1_u32 })
                .await,
            Err(LaneError::PoolClosed),
            "§0.9: acquiring on a closed semaphore maps to PoolClosed on both lanes"
        );
    }

    // §6.4.1 (G15): the §1.10 memory bounds hold their ordering — the watermark sits BELOW one slot's
    // budget (so the degree cap thins concurrency first and the watermark is the last line, not a second
    // earlier throttle), the poll cadence is far under the pause ceiling (so a freed machine resumes
    // promptly rather than serving out the ceiling), and neither bound is zero (a zero poll would busy-spin,
    // a zero ceiling would disable the pause). Pins the pre-calibration relationships the §6.7.2 harness
    // imports. [Build-Session-Entscheidung: P4.20]
    #[test]
    fn memory_adaptive_baseline_bounds_hold_their_ordering() {
        // The byte bounds' ordering is compile-enforced beside their definitions (a `const` assertion, so
        // an inverted recalibration cannot build); these are the Duration relationships, which are not
        // const-foldable.
        assert!(
            MEMORY_WATERMARK_POLL < MEMORY_PAUSE_MAX,
            "§1.10: a freed machine resumes on a poll, long before the ceiling"
        );
        assert!(
            MEMORY_WATERMARK_POLL > Duration::ZERO && MEMORY_PAUSE_MAX > Duration::ZERO,
            "§1.10: a zero poll would busy-spin the gate; a zero ceiling would disable the pause"
        );
    }

    // ─── P4.21: the §0.9 per-engine parallelism caps ──
    //
    // The rows are pure data over the same `effective_degree`/`slot_weight` frame P4.20 built, so the
    // arithmetic legs are exact unit pins and the behavioural leg reuses the established Barrier shape
    // (a Barrier sized to the expected peak proves liveness, the permit bound proves safety) — no sleep
    // window to race on.

    // §6.4.1 (G15): the §0.9 engine table transcribed — each [`EngineParallelism`] row projects onto exactly
    // the `per_engine_cap` term of §0.9's `min(global_degree, per_engine_cap, memory_based_cap)`, and
    // MAX_VIDEO_REENCODE_CONCURRENCY is pinned to the number §0.9 works out itself one bullet below its
    // table ("video re-encode runs at `min(global_degree, 2)`") — so a recalibration of the band has to
    // change §0.9 and this pin together, never silently drift from the spec's own worked example.
    #[test]
    fn the_engine_parallelism_rows_project_onto_the_per_engine_cap_term() {
        assert_eq!(
            MAX_VIDEO_REENCODE_CONCURRENCY, 2,
            "§0.9: the 'low — 1–2' band is resolved by §0.9's own worked example, min(global_degree, 2)"
        );
        assert_eq!(
            EngineParallelism::UpToGlobalDegree.per_engine_cap(),
            None,
            "§0.9: 'up to global degree' imposes NO per-engine term — the min is left to the global degree \
             and the §1.10 memory cap"
        );
        assert_eq!(
            EngineParallelism::VideoReencode.per_engine_cap(),
            Some(MAX_VIDEO_REENCODE_CONCURRENCY),
            "§0.9: the video-re-encode row is the one row carrying a number, and it carries the §0.9-owned \
             const rather than a literal (the MAX_LO_CONCURRENCY 'never hard-coded' convention)"
        );
    }

    // The COMPILE-TIME variant lock (the established dependency-free exhaustive-match pattern, cf.
    // `crate::engines`' `engine_id_exhaustive`): adding a §0.9 table row without deciding its
    // `per_engine_cap` projection in the test above fails to compile here.
    fn engine_parallelism_exhaustive(row: &EngineParallelism) {
        match row {
            EngineParallelism::UpToGlobalDegree | EngineParallelism::VideoReencode => {}
        }
    }

    #[test]
    fn engine_parallelism_exhaustive_match_is_exercised() {
        engine_parallelism_exhaustive(&EngineParallelism::VideoReencode);
    }

    // §6.4.1 (G15): the §0.9 per-engine cap enters `effective_degree` as a DOWNWARD-only term, exactly like
    // the other two ("the per-engine caps above OVERRIDE the global degree downward, never upward"). Pinned
    // across the whole §0.9 degree range 1..=4 plus the cases where the §1.10 memory term is the smaller
    // one — a per-engine cap never RAISES a degree the memory factor lowered.
    #[test]
    fn the_video_reencode_cap_bounds_the_effective_degree_downward_only() {
        // (degree, probe, expected effective degree for the capped row, why)
        type CapCase = (usize, fn() -> Option<u64>, usize, &'static str);
        let cases: [CapCase; 6] = [
            (
                1,
                mem_unknown,
                1,
                "a degree-1 host is already serial — a cap can never raise it",
            ),
            (
                2,
                mem_unknown,
                2,
                "at degree 2 the cap equals the degree, so nothing changes",
            ),
            (
                3,
                mem_unknown,
                2,
                "at degree 3 the cap is the smallest term",
            ),
            (
                4,
                mem_unknown,
                2,
                "§0.9's own example: video re-encode runs at min(global_degree, 2)",
            ),
            (4, mem_1gib, 2, "cap and memory term agree at 2"),
            (
                4,
                mem_quarter_slot,
                1,
                "§1.10: under one slot's budget the memory term is smaller still — serial",
            ),
        ];
        for (degree, probe, want, why) in cases {
            let pool = Pool::with_degree_and_memory(degree, probe);
            let capped = pool.effective_degree(EngineParallelism::VideoReencode.per_engine_cap());
            let uncapped =
                pool.effective_degree(EngineParallelism::UpToGlobalDegree.per_engine_cap());
            assert_eq!(capped, want, "§0.9/§1.10: {why} (degree {degree})");
            assert!(
                capped <= uncapped,
                "§0.9: a per-engine cap overrides the global degree DOWNWARD, never upward (degree \
                 {degree}: capped {capped}, uncapped {uncapped})"
            );
        }
    }

    // §6.4.1 (G15): the §0.9 per-engine cap is ENFORCED by the same `slot_weight` mechanism the §1.10 memory
    // term uses — a capped job takes `ceil(degree / cap)` global permits, so `floor(degree / weight)` of them
    // are admitted, never more than the cap. The degree-3 row pins the DOCUMENTED under-admission (weight 2
    // ⇒ ONE concurrent re-encode where the cap says 2), which is the safe direction and squarely inside
    // §0.9's own intent that "video re-encode is effectively serial-ish on typical machines, by design — not
    // a bug". The uncapped assertion in the same loop proves the WEIGHT is what the cap changed, not the
    // mechanism.
    #[test]
    fn the_video_reencode_cap_is_enforced_by_the_slot_weight() {
        // (degree, expected capped slot weight, expected admitted concurrent re-encodes)
        type WeightCase = (usize, u32, usize);
        let cases: [WeightCase; 4] = [(1, 1, 1), (2, 1, 2), (3, 2, 1), (4, 2, 2)];
        for (degree, weight, admitted) in cases {
            let pool = Pool::with_degree(degree);
            let capped = EngineParallelism::VideoReencode.per_engine_cap();
            assert_eq!(
                pool.slot_weight(capped),
                weight,
                "degree {degree}: the capped slot weight"
            );
            assert_eq!(
                degree / weight as usize,
                admitted,
                "§0.9: degree {degree} admits {admitted} concurrent video re-encode(s)"
            );
            assert!(
                admitted <= MAX_VIDEO_REENCODE_CONCURRENCY,
                "§0.9: never MORE than the video-re-encode cap (degree {degree})"
            );
            assert_eq!(
                pool.slot_weight(EngineParallelism::UpToGlobalDegree.per_engine_cap()),
                1,
                "§0.9: an uncapped row still takes exactly ONE permit at degree {degree} — the §0.9 row is \
                 what changed the weight, not the pool"
            );
        }
    }

    // §6.4.x (G15) THE CAP AS OBSERVABLE BEHAVIOUR, on a real semaphore: at the §0.9 clamp ceiling of 4 a
    // `VideoReencode` row admits exactly MAX_VIDEO_REENCODE_CONCURRENCY lanes at once, while an
    // `UpToGlobalDegree` row on an identical pool reaches the full degree — so the difference is provably
    // the §0.9 row and not the pool. Driven on the SUBPROCESS lane, the one §0.9's capped engine (FFmpeg
    // video re-encode) will run on; that choice costs the LIVE lane nothing, because BOTH lanes make the
    // IDENTICAL `slot_weight(parallelism.per_engine_cap())` call — an in-core cap leg would be synthetic
    // (§0.9 puts the sole in-core engine on the uncapped row), and the shared call is what carries this
    // behavioural proof across to `run_in_core`, on top of the `slot_weight` unit pins above.
    // Deterministic in the established shape: a Barrier sized to the expected
    // peak forces that many holders to rendezvous (liveness) while the permit bound caps the peak (safety),
    // and the lane count is a multiple of the barrier size so no partial final group can strand.
    #[tokio::test]
    async fn a_capped_row_admits_only_its_cap_while_an_uncapped_row_reaches_the_degree() {
        const DEGREE: usize = 4;
        for (row, expected_peak) in [
            (
                EngineParallelism::VideoReencode,
                MAX_VIDEO_REENCODE_CONCURRENCY,
            ),
            (EngineParallelism::UpToGlobalDegree, DEGREE),
        ] {
            let pool = Pool::with_degree(DEGREE);
            let barrier = Arc::new(tokio::sync::Barrier::new(expected_peak));
            let concurrent = Arc::new(AtomicUsize::new(0));
            let peak = Arc::new(AtomicUsize::new(0));

            let mut handles = Vec::new();
            for _ in 0..(2 * expected_peak) {
                let pool = pool.clone();
                let barrier = Arc::clone(&barrier);
                let concurrent = Arc::clone(&concurrent);
                let peak = Arc::clone(&peak);
                handles.push(tokio::spawn(async move {
                    pool.run_subprocess(row, async {
                        let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(now, Ordering::SeqCst);
                        barrier.wait().await; // permit-bounded: exactly `expected_peak` can be here
                        concurrent.fetch_sub(1, Ordering::SeqCst);
                    })
                    .await
                    .expect("§0.9: the subprocess lane runs the engine future to completion");
                }));
            }
            for handle in handles {
                handle.await.expect("each spawned lane task joins");
            }

            assert_eq!(
                peak.load(Ordering::SeqCst),
                expected_peak,
                "§0.9: the {row:?} row admits exactly {expected_peak} concurrent engine future(s) at \
                 global degree {DEGREE}"
            );
            assert_eq!(
                concurrent.load(Ordering::SeqCst),
                0,
                "every permit's critical section exited ({row:?})"
            );
            assert_eq!(
                pool.global.available_permits(),
                DEGREE,
                "§0.9: every weighted acquisition returned all its permits ({row:?})"
            );
        }
    }

    // ─── P4.22: the §0.9 `serialised_only` dedicated single-permit lanes ──
    //
    // The lanes are keyed GENERICALLY (§0.7: this tier-3 leaf may not name `EngineId`), so the tests key on
    // their own local engine set — which also proves the generic actually is generic. The tier-2 side (the
    // registry instantiating `SerialisedLanes<EngineId>` from its `descriptor()` walk) is asserted in
    // `crate::engines::registry`'s own tests.

    /// A local stand-in for the tier-2 `EngineId` key: one serialised engine, a SECOND serialised engine
    /// (§0.9 says "one per serialised engine", so two must not share a lane) and one that is not.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum TestEngineKey {
        Office,
        SecondOffice,
        Light,
    }

    fn office_lanes() -> SerialisedLanes<TestEngineKey> {
        SerialisedLanes::build([
            (TestEngineKey::Office, true),
            (TestEngineKey::SecondOffice, true),
            (TestEngineKey::Light, false),
        ])
    }

    // §6.4.1 (G15): `MAX_LO_CONCURRENCY` is the §0.9-owned single source of the serialisation degree, and
    // it is the number a lane is actually SIZED with — the const and the mechanism cannot drift apart
    // (§0.9: "the single source of the LibreOffice serialisation degree; the §6.7.2 test harness imports
    // this same constant rather than hard-coding 1"). The `== 1` value itself is compile-enforced beside the
    // definition, so this asserts the LINK: whatever the const says, that many holders fit.
    #[tokio::test]
    async fn max_lo_concurrency_is_the_number_a_serialised_lane_is_sized_with() {
        let lanes = office_lanes();
        let mut held = Vec::new();
        for _ in 0..MAX_LO_CONCURRENCY {
            held.push(
                lanes
                    .acquire(&TestEngineKey::Office)
                    .await
                    .expect("§0.9: the lane is open")
                    .expect("§0.9: a serialised engine HAS a lane"),
            );
        }
        assert_eq!(
            held.len(),
            MAX_LO_CONCURRENCY,
            "§0.9: exactly MAX_LO_CONCURRENCY holders fit the lane"
        );
        let mut queued = Box::pin(lanes.acquire(&TestEngineKey::Office));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut queued)
                .await
                .is_err(),
            "§0.9: the MAX_LO_CONCURRENCY-th+1 office job WAITS — the lane is sized by the const, not by a \
             hard-coded literal"
        );
        drop(held);
        assert!(
            queued.await.expect("§0.9: the lane is open").is_some(),
            "§0.9: releasing admits the next office job — the permit is returned, never leaked"
        );
    }

    // §6.4.1 (G15): §0.9's allocation rule — "a `Semaphore(MAX_LO_CONCURRENCY)` for EACH ENGINE FLAGGED
    // SERIALISED" — and its mirror image, "non-serialised engines acquire only the global degree permit".
    // A `false` flag allocates NOTHING, so an unflagged engine's acquire is a genuine no-op (`Ok(None)`),
    // not a permit it must remember to release.
    #[tokio::test]
    async fn lanes_are_allocated_for_exactly_the_flagged_engines() {
        let lanes = office_lanes();
        assert_eq!(
            lanes.lane_count(),
            2,
            "§0.9: one lane per SERIALISED engine — the unflagged one allocates nothing"
        );
        assert!(
            lanes
                .acquire(&TestEngineKey::Light)
                .await
                .expect("§0.9: the lane set is open")
                .is_none(),
            "§0.9: a non-serialised engine acquires only the global degree permit — nothing is taken here"
        );
        assert!(
            SerialisedLanes::build([(TestEngineKey::Light, false)]).lane_count() == 0,
            "§0.9: an engine set with no serialised engine allocates no lane at all"
        );
    }

    // §6.4.x (G15): §0.9 says "one per serialised engine", not one shared lane — two serialised engines
    // must not serialise against EACH OTHER (that would be a global office lock, which §0.9 does not ask
    // for and which would cost throughput for no correctness gain: the profile corruption is per-engine).
    #[tokio::test]
    async fn each_serialised_engine_gets_its_own_lane() {
        let lanes = office_lanes();
        let _first = lanes
            .acquire(&TestEngineKey::Office)
            .await
            .expect("§0.9: the lane is open")
            .expect("§0.9: a serialised engine HAS a lane");
        let second = tokio::time::timeout(
            Duration::from_millis(50),
            lanes.acquire(&TestEngineKey::SecondOffice),
        )
        .await
        .expect("§0.9: a DIFFERENT serialised engine has its OWN lane and is admitted at once")
        .expect("§0.9: the lane is open");
        assert!(
            second.is_some(),
            "§0.9: the second serialised engine took its own permit"
        );
    }

    // §6.4.x (G15) THE ORDERING DECISION, as observable behaviour: the §1.7 caller takes the ENGINE permit
    // before the §0.9 GLOBAL one, so an office job queued behind another holds NO global permit and the rest
    // of the batch keeps flowing. Under the opposite order the queued job would be sitting on a global
    // permit and an office-heavy batch could fill the whole §0.9 degree with waiters (head-of-line
    // blocking). Deterministic: proven by permit accounting plus a pending future, never by a sleep race.
    #[tokio::test]
    async fn a_queued_office_job_holds_no_global_permit_so_other_engines_keep_running() {
        const DEGREE: usize = 2;
        let pool = Pool::with_degree(DEGREE);
        let lanes = office_lanes();

        // Office job A: engine permit FIRST, then the global lane — the caller's fixed order.
        let _office_permit = lanes
            .acquire(&TestEngineKey::Office)
            .await
            .expect("§0.9: the lane is open")
            .expect("§0.9: a serialised engine HAS a lane");
        let mut running = Box::pin(pool.run_subprocess(
            EngineParallelism::UpToGlobalDegree,
            std::future::pending::<u32>(),
        ));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut running)
                .await
                .is_err(),
            "office job A is running (its engine future never completes)"
        );
        assert_eq!(
            pool.global.available_permits(),
            DEGREE - 1,
            "office job A holds exactly one global permit"
        );

        // Office job B queues on the ENGINE lane, before it ever reaches the pool.
        let mut queued_office = Box::pin(lanes.acquire(&TestEngineKey::Office));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut queued_office)
                .await
                .is_err(),
            "§0.9: office job B waits for the single office permit"
        );
        assert_eq!(
            pool.global.available_permits(),
            DEGREE - 1,
            "§0.9: the QUEUED office job holds no global permit — the engine-then-global order is what \
             keeps the pool free for other engines"
        );

        // ...which is exactly what lets a non-office job run beside them, on the permit B did not take.
        assert_eq!(
            pool.run_subprocess(EngineParallelism::UpToGlobalDegree, async { 7_u32 })
                .await
                .expect("§0.9: the light engine takes the free global permit and runs"),
            7,
            "§0.9: an office backlog does not starve the other engines"
        );

        drop(_office_permit);
        assert!(
            queued_office
                .await
                .expect("§0.9: the lane is open")
                .is_some(),
            "§0.9: releasing the engine permit on job A's exit admits job B"
        );
    }
}
